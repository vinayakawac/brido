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
You are a smart assistant that reads laptop screenshots and responds helpfully.

RULES:
1. If you see a QUIZ or MCQ question with options (A/B/C/D): pick the CORRECT answer. Format:
   Answer: B. Stack
   Explanation: Stacks follow LIFO — the last element pushed is the first popped.

2. If you see a CODING PROBLEM or code to write: write the solution code. Format:
   ```python
   def solution():
       pass
   ```
   Explanation: Brief description of approach.

3. If you see a MATH PROBLEM: solve it. Format:
   Answer: 42
   Steps: 1. First step  2. Second step

4. For ANYTHING ELSE: write a short 2-3 sentence description of what's on screen.

CRITICAL:
- READ the text in the image carefully before answering
- Give the CORRECT answer, not just any answer
- Do NOT describe the screen layout or UI elements
- Do NOT say what you see — just answer/solve what's asked
- Keep responses SHORT and DIRECT";

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
        // Try primary vision model, then fallback
        let models = ["qwen3-vl:8b", "gemma3:4b"];

        for model in models {
            let result = self.run_vision(image_base64, model, custom_prompt).await;
            match result {
                Ok(text) if !text.trim().is_empty() => {
                    tracing::info!("Got response from {}", model);
                    let clean = strip_think_tags(&text);
                    return Ok(format!("[{}]\n{}", model, clean));
                }
                Ok(_) => tracing::warn!("{} returned empty, trying next", model),
                Err(e) => tracing::warn!("{} failed: {}, trying next", model, e),
            }
        }

        Err(anyhow!("All models failed"))
    }

    async fn run_vision(
        &self,
        image_base64: &str,
        model: &str,
        custom_prompt: Option<&str>,
    ) -> Result<String> {
        let user_msg = custom_prompt
            .unwrap_or("Read this screenshot carefully and respond according to your instructions.");

        let request = ChatRequest {
            model: model.to_string(),
            messages: vec![
                ChatMessage {
                    role: "system".to_string(),
                    content: SYSTEM_PROMPT.to_string(),
                    images: None,
                },
                ChatMessage {
                    role: "user".to_string(),
                    content: user_msg.to_string(),
                    images: Some(vec![image_base64.to_string()]),
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
