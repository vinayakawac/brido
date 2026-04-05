use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

use crate::config::Config;

const ANALYZE_PROMPT: &str = "Analyze this screenshot and follow this strict priority order:\n\n1) QUESTION ANSWERING (highest priority)\n- If the screen contains direct question(s) where answers can be provided from visible context, answer them first.\n- Use this format:\nQuestion: <short restatement>\nAnswer: <best direct answer>\nWhy: <1-3 short lines>\n\n2) QUIZ / MCQ\n- If it is a multiple-choice quiz, answer using:\nAnswer: <LETTER>. <FULL_OPTION_TEXT>\n\nExplanation:\n<2-4 short lines>\n\n3) CODING PROBLEM\n- If it is a coding task, return exactly one runnable code block only and no extra text.\n\n4) SCREEN DESCRIPTION (fallback only)\n- Only when none of the above apply, briefly describe what the screen is showing in 2-4 lines.\n\nGlobal rules:\n- Pick exactly one mode from the priority list.\n- Prefer helping with visible question content over generic description.\n- If text is not readable, state that briefly and ask for a clearer frame.\n- Keep output concise and actionable.";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ProviderKind {
    OpenAI,
    Anthropic,
    Gemini,
    OpenRouter,
}

impl ProviderKind {
    fn as_str(self) -> &'static str {
        match self {
            ProviderKind::OpenAI => "openai",
            ProviderKind::Anthropic => "anthropic",
            ProviderKind::Gemini => "gemini",
            ProviderKind::OpenRouter => "openrouter",
        }
    }

    fn rank(self) -> u8 {
        match self {
            ProviderKind::OpenAI => 100,
            ProviderKind::Anthropic => 95,
            ProviderKind::Gemini => 90,
            ProviderKind::OpenRouter => 85,
        }
    }
}

#[derive(Clone)]
struct ProviderConfig {
    kind: ProviderKind,
    api_key: String,
    base_url: String,
    model: String,
}

pub struct ModelManager<'a> {
    client: &'a reqwest::Client,
    providers: Vec<ProviderConfig>,
}

impl<'a> ModelManager<'a> {
    pub fn new(config: &Config, client: &'a reqwest::Client) -> Self {
        let mut providers = Vec::new();

        push_provider(
            &mut providers,
            ProviderKind::OpenAI,
            &config.openai_api_key,
            &config.openai_base_url,
            &config.openai_model,
        );
        push_provider(
            &mut providers,
            ProviderKind::Anthropic,
            &config.anthropic_api_key,
            &config.anthropic_base_url,
            &config.anthropic_model,
        );
        push_provider(
            &mut providers,
            ProviderKind::Gemini,
            &config.gemini_api_key,
            &config.gemini_base_url,
            &config.gemini_model,
        );
        push_provider(
            &mut providers,
            ProviderKind::OpenRouter,
            &config.openrouter_api_key,
            &config.openrouter_base_url,
            &config.openrouter_model,
        );

        providers.sort_by(|a, b| b.kind.rank().cmp(&a.kind.rank()));

        Self { client, providers }
    }

    pub fn available_models(config: &Config) -> Vec<(String, String, String, f64)> {
        let mut entries = Vec::new();

        if !config.openai_api_key.trim().is_empty() {
            entries.push((
                format!("openai:{}", config.openai_model),
                config.openai_model.clone(),
                "cloud vision+reasoning".to_string(),
                10.0,
            ));
        }

        if !config.anthropic_api_key.trim().is_empty() {
            entries.push((
                format!("anthropic:{}", config.anthropic_model),
                config.anthropic_model.clone(),
                "cloud vision+reasoning".to_string(),
                9.5,
            ));
        }

        if !config.gemini_api_key.trim().is_empty() {
            entries.push((
                format!("gemini:{}", config.gemini_model),
                config.gemini_model.clone(),
                "cloud vision+reasoning".to_string(),
                9.0,
            ));
        }

        if !config.openrouter_api_key.trim().is_empty() {
            entries.push((
                format!("openrouter:{}", config.openrouter_model),
                config.openrouter_model.clone(),
                "cloud gateway multi-model".to_string(),
                8.5,
            ));
        }

        if entries.is_empty() {
            entries.push((
                "no-provider-configured".to_string(),
                "set OPENAI_API_KEY / ANTHROPIC_API_KEY / GEMINI_API_KEY / OPENROUTER_API_KEY"
                    .to_string(),
                "missing api key".to_string(),
                0.0,
            ));
        }

        entries
    }

    pub async fn analyse_image(
        &self,
        image_base64: &str,
        requested_model: Option<&str>,
        custom_prompt: Option<&str>,
    ) -> Result<(String, String)> {
        let image_base64 = normalize_image_base64(image_base64);
        if image_base64.is_empty() {
            return Err(anyhow!("Image payload is empty"));
        }

        if self.providers.is_empty() {
            return Err(anyhow!(
                "No AI provider configured. Set one of: OPENAI_API_KEY, ANTHROPIC_API_KEY, GEMINI_API_KEY, OPENROUTER_API_KEY"
            ));
        }

        let mut candidate_providers = self.providers.clone();

        if let Some(requested) = requested_model.and_then(parse_provider_hint) {
            candidate_providers.sort_by_key(|provider| {
                if provider.kind == requested {
                    0u8
                } else {
                    1u8
                }
            });
        }

        let prompt = merge_prompt(custom_prompt);
        let mut last_error: Option<anyhow::Error> = None;

        for provider in candidate_providers {
            let attempt = self
                .analyse_with_provider(&provider, &image_base64, &prompt)
                .await;

            match attempt {
                Ok(result) if !result.trim().is_empty() => {
                    let model_used = format!("{}:{}", provider.kind.as_str(), provider.model);
                    return Ok((result.trim().to_string(), model_used));
                }
                Ok(_) => {
                    last_error = Some(anyhow!(
                        "{} returned an empty response",
                        provider.kind.as_str()
                    ));
                }
                Err(err) => {
                    last_error = Some(anyhow!(
                        "{} failed: {}",
                        provider.kind.as_str(),
                        err
                    ));
                }
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow!("All providers failed")))
    }

    async fn analyse_with_provider(
        &self,
        provider: &ProviderConfig,
        image_base64: &str,
        prompt: &str,
    ) -> Result<String> {
        match provider.kind {
            ProviderKind::OpenAI => self.call_openai(provider, image_base64, prompt).await,
            ProviderKind::Anthropic => self.call_anthropic(provider, image_base64, prompt).await,
            ProviderKind::Gemini => self.call_gemini(provider, image_base64, prompt).await,
            ProviderKind::OpenRouter => self.call_openrouter(provider, image_base64, prompt).await,
        }
    }

    async fn call_openai(
        &self,
        provider: &ProviderConfig,
        image_base64: &str,
        prompt: &str,
    ) -> Result<String> {
        let url = format!("{}/chat/completions", trim_trailing_slash(&provider.base_url));
        let body = OpenAiChatRequest {
            model: provider.model.clone(),
            messages: vec![OpenAiMessage {
                role: "user".to_string(),
                content: OpenAiContent::Parts(vec![
                    OpenAiPart::Text {
                        text: prompt.to_string(),
                    },
                    OpenAiPart::ImageUrl {
                        image_url: OpenAiImageUrl {
                            url: format!("data:image/jpeg;base64,{}", image_base64),
                        },
                    },
                ]),
            }],
            temperature: 0.1,
            max_tokens: 900,
        };

        let response = self
            .client
            .post(url)
            .bearer_auth(&provider.api_key)
            .json(&body)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow!("OpenAI error {}: {}", status, error_text));
        }

        let payload = response.json::<OpenAiChatResponse>().await?;
        let content = payload
            .choices
            .into_iter()
            .next()
            .map(|choice| choice.message.content)
            .unwrap_or_default();

        Ok(content)
    }

    async fn call_openrouter(
        &self,
        provider: &ProviderConfig,
        image_base64: &str,
        prompt: &str,
    ) -> Result<String> {
        let url = format!("{}/chat/completions", trim_trailing_slash(&provider.base_url));
        let body = OpenAiChatRequest {
            model: provider.model.clone(),
            messages: vec![OpenAiMessage {
                role: "user".to_string(),
                content: OpenAiContent::Parts(vec![
                    OpenAiPart::Text {
                        text: prompt.to_string(),
                    },
                    OpenAiPart::ImageUrl {
                        image_url: OpenAiImageUrl {
                            url: format!("data:image/jpeg;base64,{}", image_base64),
                        },
                    },
                ]),
            }],
            temperature: 0.1,
            max_tokens: 900,
        };

        let response = self
            .client
            .post(url)
            .header("Authorization", format!("Bearer {}", provider.api_key))
            .header("HTTP-Referer", "https://brido.local")
            .header("X-Title", "brido")
            .json(&body)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow!("OpenRouter error {}: {}", status, error_text));
        }

        let payload = response.json::<OpenAiChatResponse>().await?;
        let content = payload
            .choices
            .into_iter()
            .next()
            .map(|choice| choice.message.content)
            .unwrap_or_default();

        Ok(content)
    }

    async fn call_anthropic(
        &self,
        provider: &ProviderConfig,
        image_base64: &str,
        prompt: &str,
    ) -> Result<String> {
        let url = format!("{}/messages", trim_trailing_slash(&provider.base_url));
        let body = AnthropicRequest {
            model: provider.model.clone(),
            max_tokens: 900,
            temperature: 0.1,
            messages: vec![AnthropicMessage {
                role: "user".to_string(),
                content: vec![
                    AnthropicContent::Text {
                        text: prompt.to_string(),
                    },
                    AnthropicContent::Image {
                        source: AnthropicImageSource {
                            source_type: "base64".to_string(),
                            media_type: "image/jpeg".to_string(),
                            data: image_base64.to_string(),
                        },
                    },
                ],
            }],
        };

        let response = self
            .client
            .post(url)
            .header("x-api-key", &provider.api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&body)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow!("Anthropic error {}: {}", status, error_text));
        }

        let payload = response.json::<AnthropicResponse>().await?;
        let content = payload
            .content
            .into_iter()
            .filter_map(|item| match item {
                AnthropicTextBlock::Text { text } => Some(text),
            })
            .collect::<Vec<_>>()
            .join("\n");

        Ok(content)
    }

    async fn call_gemini(
        &self,
        provider: &ProviderConfig,
        image_base64: &str,
        prompt: &str,
    ) -> Result<String> {
        let url = format!(
            "{}/models/{}:generateContent?key={}",
            trim_trailing_slash(&provider.base_url),
            provider.model,
            provider.api_key
        );

        let body = GeminiRequest {
            contents: vec![GeminiContent {
                parts: vec![
                    GeminiPart {
                        text: Some(prompt.to_string()),
                        inline_data: None,
                    },
                    GeminiPart {
                        text: None,
                        inline_data: Some(GeminiInlineData {
                            mime_type: "image/jpeg".to_string(),
                            data: image_base64.to_string(),
                        }),
                    },
                ],
            }],
            generation_config: GeminiGenerationConfig {
                temperature: 0.1,
                max_output_tokens: 900,
            },
        };

        let response = self.client.post(url).json(&body).send().await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow!("Gemini error {}: {}", status, error_text));
        }

        let payload = response.json::<GeminiResponse>().await?;

        let content = payload
            .candidates
            .unwrap_or_default()
            .into_iter()
            .flat_map(|candidate| candidate.content.parts)
            .filter_map(|part| part.text)
            .collect::<Vec<_>>()
            .join("\n");

        Ok(content)
    }
}

fn push_provider(
    providers: &mut Vec<ProviderConfig>,
    kind: ProviderKind,
    api_key: &str,
    base_url: &str,
    model: &str,
) {
    if api_key.trim().is_empty() {
        return;
    }

    providers.push(ProviderConfig {
        kind,
        api_key: api_key.trim().to_string(),
        base_url: base_url.trim().to_string(),
        model: model.trim().to_string(),
    });
}

fn merge_prompt(custom_prompt: Option<&str>) -> String {
    match custom_prompt {
        Some(extra) if !extra.trim().is_empty() => {
            format!("{}\n\nExtra instruction:\n{}", ANALYZE_PROMPT, extra)
        }
        _ => ANALYZE_PROMPT.to_string(),
    }
}

fn parse_provider_hint(model_hint: &str) -> Option<ProviderKind> {
    let lower = model_hint.to_lowercase();

    if lower.starts_with("openai:") || lower.contains("gpt") {
        Some(ProviderKind::OpenAI)
    } else if lower.starts_with("anthropic:") || lower.contains("claude") {
        Some(ProviderKind::Anthropic)
    } else if lower.starts_with("gemini:") || lower.contains("gemini") {
        Some(ProviderKind::Gemini)
    } else if lower.starts_with("openrouter:") || lower.contains("openrouter") {
        Some(ProviderKind::OpenRouter)
    } else {
        None
    }
}

fn trim_trailing_slash(value: &str) -> &str {
    value.trim_end_matches('/')
}

fn normalize_image_base64(raw: &str) -> String {
    raw.trim()
        .strip_prefix("data:image/jpeg;base64,")
        .or_else(|| raw.trim().strip_prefix("data:image/png;base64,"))
        .unwrap_or(raw.trim())
        .to_string()
}

#[derive(Serialize)]
struct OpenAiChatRequest {
    model: String,
    messages: Vec<OpenAiMessage>,
    temperature: f32,
    max_tokens: u32,
}

#[derive(Serialize)]
struct OpenAiMessage {
    role: String,
    content: OpenAiContent,
}

#[derive(Serialize)]
#[serde(untagged)]
enum OpenAiContent {
    Parts(Vec<OpenAiPart>),
}

#[derive(Serialize)]
#[serde(tag = "type")]
enum OpenAiPart {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image_url")]
    ImageUrl { image_url: OpenAiImageUrl },
}

#[derive(Serialize)]
struct OpenAiImageUrl {
    url: String,
}

#[derive(Deserialize)]
struct OpenAiChatResponse {
    choices: Vec<OpenAiChoice>,
}

#[derive(Deserialize)]
struct OpenAiChoice {
    message: OpenAiChoiceMessage,
}

#[derive(Deserialize)]
struct OpenAiChoiceMessage {
    content: String,
}

#[derive(Serialize)]
struct AnthropicRequest {
    model: String,
    max_tokens: u32,
    temperature: f32,
    messages: Vec<AnthropicMessage>,
}

#[derive(Serialize)]
struct AnthropicMessage {
    role: String,
    content: Vec<AnthropicContent>,
}

#[derive(Serialize)]
#[serde(tag = "type")]
enum AnthropicContent {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image")]
    Image { source: AnthropicImageSource },
}

#[derive(Serialize)]
struct AnthropicImageSource {
    #[serde(rename = "type")]
    source_type: String,
    media_type: String,
    data: String,
}

#[derive(Deserialize)]
struct AnthropicResponse {
    content: Vec<AnthropicTextBlock>,
}

#[derive(Deserialize)]
#[serde(tag = "type")]
enum AnthropicTextBlock {
    #[serde(rename = "text")]
    Text { text: String },
}

#[derive(Serialize)]
struct GeminiRequest {
    contents: Vec<GeminiContent>,
    #[serde(rename = "generationConfig")]
    generation_config: GeminiGenerationConfig,
}

#[derive(Serialize)]
struct GeminiContent {
    parts: Vec<GeminiPart>,
}

#[derive(Serialize, Deserialize)]
struct GeminiPart {
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    inline_data: Option<GeminiInlineData>,
}

#[derive(Serialize, Deserialize)]
struct GeminiInlineData {
    mime_type: String,
    data: String,
}

#[derive(Serialize)]
struct GeminiGenerationConfig {
    temperature: f32,
    #[serde(rename = "maxOutputTokens")]
    max_output_tokens: u32,
}

#[derive(Deserialize)]
struct GeminiResponse {
    candidates: Option<Vec<GeminiCandidate>>,
}

#[derive(Deserialize)]
struct GeminiCandidate {
    content: GeminiCandidateContent,
}

#[derive(Deserialize)]
struct GeminiCandidateContent {
    parts: Vec<GeminiPart>,
}
