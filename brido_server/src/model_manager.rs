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

#[derive(Debug)]
enum ContentType {
    Code,
    Math,
    Quiz,
    General,
}

impl<'a> ModelManager<'a> {
    pub fn new(ollama_url: &'a str, client: &'a reqwest::Client) -> Self {
        Self { ollama_url, client }
    }

    /// Agent pipeline:
    /// 1. Classify content type using qwen3-vl
    /// 2. Route to best model for that type
    /// 3. Fallback chain if primary fails or returns empty
    pub async fn analyse_image(
        &self,
        image_base64: &str,
        _model: &str,
        custom_prompt: Option<&str>,
    ) -> Result<String> {
        // Stage 1: classify
        let content_type = self.classify(image_base64).await.unwrap_or(ContentType::General);
        tracing::info!("Content classified as: {:?}", content_type);

        // Stage 2: primary model based on type
        let (primary, fallbacks): (&str, &[&str]) = match content_type {
            ContentType::Code => ("deepseek-r1:8b", &["qwen3-vl:8b", "gemma3:4b"]),
            ContentType::Math => ("deepseek-r1:8b", &["qwen3-vl:8b", "gemma3:4b"]),
            ContentType::Quiz => ("qwen3-vl:8b", &["gemma3:4b", "deepseek-r1:8b"]),
            ContentType::General => ("qwen3-vl:8b", &["gemma3:4b", "deepseek-r1:8b"]),
        };

        // Stage 3: try primary, then fallbacks
        let all_models = std::iter::once(primary).chain(fallbacks.iter().copied());

        for model in all_models {
            let result = self
                .run_model(image_base64, model, custom_prompt, &content_type)
                .await;
            match result {
                Ok(text) if !text.trim().is_empty() => {
                    tracing::info!("Got response from model: {}", model);
                    return Ok(format!("[{}]\n{}", model, text));
                }
                Ok(_) => {
                    tracing::warn!("Model {} returned empty, trying next", model);
                }
                Err(e) => {
                    tracing::warn!("Model {} failed: {}, trying next", model, e);
                }
            }
        }

        Err(anyhow!("All models failed to produce a response"))
    }

    async fn classify(&self, image_base64: &str) -> Result<ContentType> {
        let prompt = "Look at this screenshot. Reply with exactly one word:\n\
                      - CODE if it shows programming code or a coding problem\n\
                      - MATH if it shows equations or mathematical problems\n\
                      - QUIZ if it shows a multiple choice or written question\n\
                      - GENERAL for anything else\n\
                      Reply with only that one word.";

        let request = ChatRequest {
            model: "qwen3-vl:8b".to_string(),
            messages: vec![ChatMessage {
                role: "user".to_string(),
                content: prompt.to_string(),
                images: Some(vec![image_base64.to_string()]),
            }],
            stream: false,
            options: Some(ChatOptions {
                num_predict: 16,
                num_ctx: 1024,
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

        let label = response.message.content.trim().to_uppercase();
        Ok(match label.as_str() {
            l if l.contains("CODE") => ContentType::Code,
            l if l.contains("MATH") => ContentType::Math,
            l if l.contains("QUIZ") => ContentType::Quiz,
            _ => ContentType::General,
        })
    }

    async fn run_model(
        &self,
        image_base64: &str,
        model: &str,
        custom_prompt: Option<&str>,
        content_type: &ContentType,
    ) -> Result<String> {
        let is_vision = model.contains("vl") || model.contains("gemma");

        if is_vision {
            self.vision_analysis(image_base64, model, custom_prompt, content_type)
                .await
        } else {
            // Reasoning model: extract text first, then reason
            let extracted = self
                .vision_analysis(
                    image_base64,
                    "qwen3-vl:8b",
                    Some("Extract all visible text from this screenshot exactly as shown. Preserve structure, options, and formatting."),
                    &ContentType::General,
                )
                .await?;

            if extracted.trim().is_empty() {
                return Err(anyhow!("Vision extraction returned empty"));
            }

            self.text_analysis(&extracted, model, custom_prompt, content_type)
                .await
        }
    }

    async fn vision_analysis(
        &self,
        image_base64: &str,
        model: &str,
        custom_prompt: Option<&str>,
        content_type: &ContentType,
    ) -> Result<String> {
        let default_prompt = match content_type {
            ContentType::Code => {
                "You are analyzing a code problem on a laptop screen.\n\
                 Identify the problem, provide the solution with explanation.\n\
                 Format:\nAnswer: <solution code>\nExplanation: <why it works>"
            }
            ContentType::Math => {
                "You are analyzing a math problem on a laptop screen.\n\
                 Solve it step by step.\n\
                 Format:\nAnswer: <final answer>\nExplanation: <steps>"
            }
            ContentType::Quiz => {
                "You are analyzing a quiz question on a laptop screen.\n\
                 Identify the correct answer from the options.\n\
                 Format:\nAnswer: <correct option>\nExplanation: <why it is correct>"
            }
            ContentType::General => {
                "You are analyzing a laptop screen screenshot.\n\
                 Identify any questions, tasks, or content visible and respond helpfully.\n\
                 Format:\nAnswer: <your answer>\nExplanation: <your explanation>"
            }
        };

        let prompt = custom_prompt.unwrap_or(default_prompt);

        let request = ChatRequest {
            model: model.to_string(),
            messages: vec![ChatMessage {
                role: "user".to_string(),
                content: prompt.to_string(),
                images: Some(vec![image_base64.to_string()]),
            }],
            stream: false,
            options: Some(ChatOptions {
                num_predict: 512,
                num_ctx: 2048,
                temperature: 0.3,
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

    async fn text_analysis(
        &self,
        text: &str,
        model: &str,
        custom_prompt: Option<&str>,
        content_type: &ContentType,
    ) -> Result<String> {
        let default_prompt = match content_type {
            ContentType::Code => format!(
                "Solve this coding problem:\n\n{}\n\nFormat:\nAnswer: <solution>\nExplanation: <explanation>",
                text
            ),
            ContentType::Math => format!(
                "Solve this math problem step by step:\n\n{}\n\nFormat:\nAnswer: <answer>\nExplanation: <steps>",
                text
            ),
            _ => format!(
                "Answer this question:\n\n{}\n\nFormat:\nAnswer: <answer>\nExplanation: <explanation>",
                text
            ),
        };

        let content = custom_prompt
            .map(|p| format!("{}\n\nContent:\n{}", p, text))
            .unwrap_or(default_prompt);

        let request = ChatRequest {
            model: model.to_string(),
            messages: vec![ChatMessage {
                role: "user".to_string(),
                content,
                images: None,
            }],
            stream: false,
            options: Some(ChatOptions {
                num_predict: 512,
                num_ctx: 2048,
                temperature: 0.3,
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
