use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

pub struct ModelManager<'a> {
    ollama_url: &'a str,
    client: &'a reqwest::Client,
}

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    options: Option<ChatOptions>,
    #[serde(skip_serializing_if = "Option::is_none")]
    keep_alive: Option<String>,
}

#[derive(Serialize)]
struct ChatOptions {
    num_predict: u32,
    num_ctx: u32,
    temperature: f32,
}

#[derive(Serialize)]
struct ChatMessage {
    role: String,
    content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    images: Option<Vec<String>>,
}

#[derive(Deserialize)]
struct ChatResponse {
    message: ResponseMessage,
}

#[derive(Deserialize)]
struct ResponseMessage {
    content: String,
}

const SYSTEM_PROMPT: &str = "\
You are an expert assistant answering questions visible on a laptop screenshot.

WHEN YOU SEE MCQ / QUIZ QUESTIONS:
- There may be multiple numbered questions. Answer ALL of them.
- Use your own knowledge to pick the CORRECT answer — do NOT guess from the image alone.
- Output one line per question in this exact format:
  1. B. Queue
  2. C. O(log n)
  3. B. HTML
- No explanations unless there is only one question.

WHEN YOU SEE A CODING PROBLEM:
- Write the solution code.
- Add a one-line explanation of the approach.

WHEN YOU SEE A MATH PROBLEM:
- Solve it. Show the answer and brief steps.

ANYTHING ELSE:
- Write 2-3 sentences describing what's on screen.

CRITICAL RULES:
- ALWAYS use your knowledge to verify the correct answer — never pick an option just because it appears first.
- READ every option carefully before deciding.
- Do NOT describe UI, windows, or layout.
- Keep responses SHORT and DIRECT.";

impl<'a> ModelManager<'a> {
    pub fn new(ollama_url: &'a str, client: &'a reqwest::Client) -> Self {
        Self { ollama_url, client }
    }

    pub async fn analyse_image(
        &self,
        image_base64: &str,
        _model: &str,
        custom_prompt: Option<&str>,
    ) -> Result<String> {
        // Step 1: vision model reads the screen
        let vision_text: String = self.run_vision(image_base64, "qwen3-vl:4b", custom_prompt)
            .await
            .map_err(|e| anyhow!("Vision model failed: {}", e))?;

        if vision_text.trim().is_empty() {
            return Err(anyhow!("Vision model returned empty"));
        }

        let clean_vision = strip_think_tags(&vision_text);

        // Step 2: if it looks like code/algorithm/reasoning, send to DeepSeek
        let needs_reasoning = looks_like_reasoning(&clean_vision);

        if needs_reasoning {
            let reasoning_result = self.run_reasoning(&clean_vision).await;
            match reasoning_result {
                Ok(r) if !r.trim().is_empty() => {
                    let clean = strip_think_tags(&r);
                    return Ok(format!("[qwen3-vl:4b + deepseek-r1:8b]\n{}", clean));
                }
                _ => {} // fall through to vision-only result
            }
        }

        Ok(format!("[qwen3-vl:4b]\n{}", clean_vision))
    }

    async fn run_reasoning(&self, extracted_text: &str) -> Result<String> {
        let prompt = format!(
            "Solve this problem concisely:\n\n{}\n\nAnswer directly with solution and brief explanation.",
            extracted_text
        );

        let request = ChatRequest {
            model: "deepseek-r1:8b".to_string(),
            messages: vec![
                ChatMessage {
                    role: "user".to_string(),
                    content: prompt,
                    images: None,
                },
            ],
            stream: false,
            options: Some(ChatOptions {
                num_predict: 512,
                num_ctx: 4096,
                temperature: 0.1,
            }),
            keep_alive: Some("5m".to_string()),
        };

        let response = self
            .client
            .post(format!("{}/api/chat", self.ollama_url))
            .json(&request)
            .send()
            .await?
            .error_for_status()?
            .json::<ChatResponse>()
            .await?;

        Ok(response.message.content)
    }

    async fn run_vision(
        &self,
        image_base64: &str,
        model: &str,
        custom_prompt: Option<&str>,
    ) -> Result<String> {
        // Fold system instructions into the user message — qwen3-vl does not
        // support a separate "system" role in Ollama's chat API.
        let user_content = match custom_prompt {
            Some(p) => format!("{}", p),
            None => format!("{}", SYSTEM_PROMPT),
        };

        let request = ChatRequest {
            model: model.to_string(),
            messages: vec![
                ChatMessage {
                    role: "user".to_string(),
                    content: user_content,
                    images: Some(vec![image_base64.to_string()]),
                },
            ],
            stream: false,
            options: Some(ChatOptions {
                num_predict: 512,
                num_ctx: 8192,
                temperature: 0.1,
            }),
            keep_alive: Some("5m".to_string()),
        };

        let response = self
            .client
            .post(format!("{}/api/chat", self.ollama_url))
            .json(&request)
            .send()
            .await?
            .error_for_status()?
            .json::<ChatResponse>()
            .await?;

        Ok(response.message.content)
    }
}

fn looks_like_reasoning(text: &str) -> bool {
    let lower = text.to_lowercase();
    let triggers = [
        "complexity", "algorithm", "function", "code", "program",
        "output", "implement", "recursion", "loop", "array",
        "prove", "derive", "calculate", "equation", "solve",
    ];
    triggers.iter().filter(|&&t| lower.contains(t)).count() >= 2
}

fn strip_think_tags(text: &str) -> String {
    let mut result = text.to_string();
    while let Some(start) = result.find("<think>") {
        if let Some(end) = result.find("</think>") {
            result = format!("{}{}", &result[..start], &result[end + 8..]);
        } else {
            result = result[..start].to_string();
            break;
        }
    }
    result.trim().to_string()
}
