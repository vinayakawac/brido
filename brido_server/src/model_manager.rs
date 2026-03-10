use anyhow::Result;
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

impl<'a> ModelManager<'a> {
    pub fn new(ollama_url: &'a str, client: &'a reqwest::Client) -> Self {
        Self { ollama_url, client }
    }

    /// Analyses an image using the appropriate model.
    /// Vision models receive the image directly.
    /// Reasoning-only models first use a vision model to extract text,
    /// then process the extracted text.
    pub async fn analyse_image(
        &self,
        image_base64: &str,
        model: &str,
        custom_prompt: Option<&str>,
    ) -> Result<String> {
        let is_vision = model.contains("vl") || model.contains("gemma");

        if is_vision {
            self.vision_analysis(image_base64, model, custom_prompt)
                .await
        } else {
            // Two-stage pipeline for reasoning models:
            // 1. Extract text with a vision model
            // 2. Send text to the reasoning model
            let extracted = self
                .vision_analysis(
                    image_base64,
                    "qwen3-vl:8b",
                    Some("Extract all visible text from this screenshot. Return only the raw text, preserving structure."),
                )
                .await?;

            self.text_analysis(&extracted, model, custom_prompt).await
        }
    }

    async fn vision_analysis(
        &self,
        image_base64: &str,
        model: &str,
        custom_prompt: Option<&str>,
    ) -> Result<String> {
        let prompt = custom_prompt.unwrap_or(
            "You are analyzing a screenshot from a laptop screen. \
             Identify any questions, code, or problems visible. \
             Provide a clear answer and explanation.\n\n\
             Format your response as:\n\
             Answer: <your answer>\n\
             Explanation: <your explanation>",
        );

        let request = ChatRequest {
            model: model.to_string(),
            messages: vec![ChatMessage {
                role: "user".to_string(),
                content: prompt.to_string(),
                images: Some(vec![image_base64.to_string()]),
            }],
            stream: false,
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
    ) -> Result<String> {
        let content = if let Some(p) = custom_prompt {
            format!("{}\n\nText from screenshot:\n{}", p, text)
        } else {
            format!(
                "You are solving a programming or technical quiz.\n\n\
                 Question:\n{}\n\n\
                 Return:\n\
                 Answer: <your answer>\n\
                 Explanation: <your explanation>",
                text
            )
        };

        let request = ChatRequest {
            model: model.to_string(),
            messages: vec![ChatMessage {
                role: "user".to_string(),
                content,
                images: None,
            }],
            stream: false,
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
