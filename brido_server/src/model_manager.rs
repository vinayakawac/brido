use reqwest::StatusCode;
use serde::{Deserialize, Serialize};

use crate::config::Config;

const ANALYZE_PROMPT: &str = "Analyze this screenshot and follow this strict priority order:\n\n1) QUESTION ANSWERING (highest priority)\n- If the screen contains direct question(s) where answers can be provided from visible context, answer them first.\n- Use this format:\nQuestion: <short restatement>\nAnswer: <best direct answer>\nWhy: <1-3 short lines>\n\n2) QUIZ / MCQ\n- If it is a multiple-choice quiz, answer using:\nAnswer: <LETTER>. <FULL_OPTION_TEXT>\n\nExplanation:\n<2-4 short lines>\n\n3) CODING PROBLEM\n- If it is a coding task, return exactly one runnable code block only and no extra text.\n\n4) SCREEN DESCRIPTION (fallback only)\n- Only when none of the above apply, briefly describe what the screen is showing in 2-4 lines.\n\nGlobal rules:\n- Pick exactly one mode from the priority list.\n- Prefer helping with visible question content over generic description.\n- If text is not readable, state that briefly and ask for a clearer frame.\n- Keep output concise and actionable.";
const MAX_ANALYZE_IMAGE_BASE64_BYTES: usize = 5 * 1024 * 1024;

#[derive(Clone, Debug, Serialize)]
pub struct ProviderAttempt {
    pub provider: String,
    pub model: String,
    pub code: String,
    pub message: String,
}

#[derive(Clone, Copy, Debug)]
pub enum AnalyseErrorCode {
    ImagePayloadEmpty,
    ImagePayloadTooLarge,
    NoProviderConfigured,
    ModelUnsupportedImageInput,
    ProviderAuthFailed,
    ProviderRateLimited,
    ProviderTimeout,
    ProviderUnavailable,
    ProviderOutOfMemory,
    NetworkError,
    ProviderReturnedEmpty,
    AllProvidersFailed,
}

impl AnalyseErrorCode {
    pub fn as_str(self) -> &'static str {
        match self {
            AnalyseErrorCode::ImagePayloadEmpty => "image_payload_empty",
            AnalyseErrorCode::ImagePayloadTooLarge => "image_payload_too_large",
            AnalyseErrorCode::NoProviderConfigured => "no_provider_configured",
            AnalyseErrorCode::ModelUnsupportedImageInput => "model_unsupported_image_input",
            AnalyseErrorCode::ProviderAuthFailed => "provider_auth_failed",
            AnalyseErrorCode::ProviderRateLimited => "provider_rate_limited",
            AnalyseErrorCode::ProviderTimeout => "provider_timeout",
            AnalyseErrorCode::ProviderUnavailable => "provider_unavailable",
            AnalyseErrorCode::ProviderOutOfMemory => "provider_out_of_memory",
            AnalyseErrorCode::NetworkError => "network_error",
            AnalyseErrorCode::ProviderReturnedEmpty => "provider_returned_empty",
            AnalyseErrorCode::AllProvidersFailed => "all_providers_failed",
        }
    }
}

#[derive(Clone, Debug)]
pub struct AnalyseError {
    pub code: AnalyseErrorCode,
    pub message: String,
    pub retryable: bool,
    pub hint: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub attempts: Vec<ProviderAttempt>,
}

impl AnalyseError {
    fn new(code: AnalyseErrorCode, message: impl Into<String>, retryable: bool) -> Self {
        Self {
            code,
            message: message.into(),
            retryable,
            hint: None,
            provider: None,
            model: None,
            attempts: Vec::new(),
        }
    }

    fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    fn with_provider(mut self, provider: &str, model: &str) -> Self {
        self.provider = Some(provider.to_string());
        self.model = Some(model.to_string());
        self
    }

    fn with_attempts(mut self, attempts: Vec<ProviderAttempt>) -> Self {
        self.attempts = attempts;
        self
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ProviderKind {
    OpenAI,
    Anthropic,
    Gemini,
    OpenRouter,
    Ollama,
}

impl ProviderKind {
    fn as_str(self) -> &'static str {
        match self {
            ProviderKind::OpenAI => "openai",
            ProviderKind::Anthropic => "anthropic",
            ProviderKind::Gemini => "gemini",
            ProviderKind::OpenRouter => "openrouter",
            ProviderKind::Ollama => "ollama",
        }
    }

    fn rank(self) -> u8 {
        match self {
            ProviderKind::OpenRouter => 100,
            ProviderKind::OpenAI => 95,
            ProviderKind::Anthropic => 90,
            ProviderKind::Gemini => 85,
            ProviderKind::Ollama => 80,
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

        if let Some(kind) = crate::config::ProviderKind::from_label(&config.active_provider) {
            match kind {
                crate::config::ProviderKind::OpenAI => {
                    push_provider(&mut providers, ProviderKind::OpenAI, &config.openai_api_key, &config.openai_base_url, &config.openai_model);
                }
                crate::config::ProviderKind::Anthropic => {
                    push_provider(&mut providers, ProviderKind::Anthropic, &config.anthropic_api_key, &config.anthropic_base_url, &config.anthropic_model);
                }
                crate::config::ProviderKind::Gemini => {
                    push_provider(&mut providers, ProviderKind::Gemini, &config.gemini_api_key, &config.gemini_base_url, &config.gemini_model);
                }
                crate::config::ProviderKind::OpenRouter => {
                    push_provider(&mut providers, ProviderKind::OpenRouter, &config.openrouter_api_key, &config.openrouter_base_url, &config.openrouter_model);
                }
                crate::config::ProviderKind::Ollama => {
                    push_provider(&mut providers, ProviderKind::Ollama, &config.ollama_api_key, &config.ollama_base_url, &config.ollama_model);
                }
            }
        }

        providers.sort_by(|a, b| b.kind.rank().cmp(&a.kind.rank()));

        Self { client, providers }
    }

    pub fn available_models(config: &Config) -> Vec<(String, String, String, f64)> {
        let mut entries = Vec::new();

        if !config.openrouter_api_key.trim().is_empty() {
            entries.push((
                format!("openrouter:{}", config.openrouter_model),
                config.openrouter_model.clone(),
                "cloud gateway multi-model".to_string(),
                8.5,
            ));
        }

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

        if !config.ollama_base_url.trim().is_empty() {
            entries.push((
                format!("ollama:{}", config.ollama_model),
                config.ollama_model.clone(),
                "local vision+reasoning".to_string(),
                8.0,
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
    ) -> std::result::Result<(String, String), AnalyseError> {
        let image_base64 = normalize_image_base64(image_base64);
        if image_base64.is_empty() {
            return Err(
                AnalyseError::new(
                    AnalyseErrorCode::ImagePayloadEmpty,
                    "Image payload is empty.",
                    false,
                )
                .with_hint("Capture a frame first, then try analysing again."),
            );
        }

        if image_base64.len() > MAX_ANALYZE_IMAGE_BASE64_BYTES {
            return Err(
                AnalyseError::new(
                    AnalyseErrorCode::ImagePayloadTooLarge,
                    format!(
                        "Image payload too large ({} bytes).",
                        image_base64.len()
                    ),
                    true,
                )
                .with_hint("Use a smaller frame or lower JPEG quality and retry."),
            );
        }

        if self.providers.is_empty() {
            return Err(
                AnalyseError::new(
                    AnalyseErrorCode::NoProviderConfigured,
                    "No AI provider configured.",
                    false,
                )
                .with_hint(
                    "Set one of OPENAI_API_KEY, ANTHROPIC_API_KEY, GEMINI_API_KEY, or OPENROUTER_API_KEY and restart server.",
                ),
            );
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
        let mut last_error: Option<AnalyseError> = None;
        let mut attempts: Vec<ProviderAttempt> = Vec::new();

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
                    let err = AnalyseError::new(
                        AnalyseErrorCode::ProviderReturnedEmpty,
                        format!("{} returned an empty response", provider.kind.as_str()),
                        true,
                    )
                    .with_provider(provider.kind.as_str(), &provider.model)
                    .with_hint("Try again or switch to another provider/model.");

                    attempts.push(ProviderAttempt {
                        provider: provider.kind.as_str().to_string(),
                        model: provider.model.clone(),
                        code: err.code.as_str().to_string(),
                        message: err.message.clone(),
                    });
                    last_error = Some(err);
                }
                Err(err) => {
                    attempts.push(ProviderAttempt {
                        provider: provider.kind.as_str().to_string(),
                        model: provider.model.clone(),
                        code: err.code.as_str().to_string(),
                        message: err.message.clone(),
                    });
                    last_error = Some(err);
                }
            }
        }

        if attempts
            .iter()
            .all(|attempt| attempt.code == AnalyseErrorCode::ModelUnsupportedImageInput.as_str())
        {
            return Err(
                AnalyseError::new(
                    AnalyseErrorCode::ModelUnsupportedImageInput,
                    "No configured provider/model currently supports image input for this request.",
                    false,
                )
                .with_hint("Select a vision-capable model or configure another provider.")
                .with_attempts(attempts),
            );
        }

        let fallback_msg = last_error
            .as_ref()
            .map(|err| err.message.clone())
            .unwrap_or_else(|| "All providers failed.".to_string());

        Err(
            AnalyseError::new(
                AnalyseErrorCode::AllProvidersFailed,
                format!("All providers failed. Last error: {}", fallback_msg),
                true,
            )
            .with_hint("Check provider credentials, model capability, and network connectivity.")
            .with_attempts(attempts),
        )
    }

    async fn analyse_with_provider(
        &self,
        provider: &ProviderConfig,
        image_base64: &str,
        prompt: &str,
    ) -> std::result::Result<String, AnalyseError> {
        match provider.kind {
            ProviderKind::OpenAI => self.call_openai(provider, image_base64, prompt).await,
            ProviderKind::Anthropic => self.call_anthropic(provider, image_base64, prompt).await,
            ProviderKind::Gemini => self.call_gemini(provider, image_base64, prompt).await,
            ProviderKind::OpenRouter => self.call_openrouter(provider, image_base64, prompt).await,
            ProviderKind::Ollama => self.call_openai(provider, image_base64, prompt).await,
        }
    }

    async fn call_openai(
        &self,
        provider: &ProviderConfig,
        image_base64: &str,
        prompt: &str,
    ) -> std::result::Result<String, AnalyseError> {
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
            .await
            .map_err(|err| classify_transport_error(provider, err))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(classify_http_error(provider, status, &error_text));
        }

        let payload = response
            .json::<OpenAiChatResponse>()
            .await
            .map_err(|err| {
                AnalyseError::new(
                    AnalyseErrorCode::ProviderUnavailable,
                    format!("Invalid OpenAI response: {}", err),
                    true,
                )
                .with_provider(provider.kind.as_str(), &provider.model)
            })?;
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
    ) -> std::result::Result<String, AnalyseError> {
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
            .await
            .map_err(|err| classify_transport_error(provider, err))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(classify_http_error(provider, status, &error_text));
        }

        let payload = response
            .json::<OpenAiChatResponse>()
            .await
            .map_err(|err| {
                AnalyseError::new(
                    AnalyseErrorCode::ProviderUnavailable,
                    format!("Invalid OpenRouter response: {}", err),
                    true,
                )
                .with_provider(provider.kind.as_str(), &provider.model)
            })?;
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
    ) -> std::result::Result<String, AnalyseError> {
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
            .await
            .map_err(|err| classify_transport_error(provider, err))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(classify_http_error(provider, status, &error_text));
        }

        let payload = response
            .json::<AnthropicResponse>()
            .await
            .map_err(|err| {
                AnalyseError::new(
                    AnalyseErrorCode::ProviderUnavailable,
                    format!("Invalid Anthropic response: {}", err),
                    true,
                )
                .with_provider(provider.kind.as_str(), &provider.model)
            })?;
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
    ) -> std::result::Result<String, AnalyseError> {
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

        let response = self
            .client
            .post(url)
            .json(&body)
            .send()
            .await
            .map_err(|err| classify_transport_error(provider, err))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(classify_http_error(provider, status, &error_text));
        }

        let payload = response
            .json::<GeminiResponse>()
            .await
            .map_err(|err| {
                AnalyseError::new(
                    AnalyseErrorCode::ProviderUnavailable,
                    format!("Invalid Gemini response: {}", err),
                    true,
                )
                .with_provider(provider.kind.as_str(), &provider.model)
            })?;

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
    if kind != ProviderKind::Ollama && api_key.trim().is_empty() {
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
    } else if lower.starts_with("ollama:") || lower.contains("ollama") {
        Some(ProviderKind::Ollama)
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

fn classify_transport_error(provider: &ProviderConfig, error: reqwest::Error) -> AnalyseError {
    if error.is_timeout() {
        return AnalyseError::new(
            AnalyseErrorCode::ProviderTimeout,
            format!("{} request timed out", provider.kind.as_str()),
            true,
        )
        .with_provider(provider.kind.as_str(), &provider.model)
        .with_hint("Retry in a few seconds or switch provider.");
    }

    AnalyseError::new(
        AnalyseErrorCode::NetworkError,
        format!("{} network error: {}", provider.kind.as_str(), error),
        true,
    )
    .with_provider(provider.kind.as_str(), &provider.model)
    .with_hint("Check internet access and provider endpoint configuration.")
}

fn classify_http_error(
    provider: &ProviderConfig,
    status: StatusCode,
    error_text: &str,
) -> AnalyseError {
    let body_lower = error_text.to_lowercase();
    let provider_name = provider.kind.as_str();

    if provider.kind == ProviderKind::OpenRouter
        && status == StatusCode::NOT_FOUND
        && (body_lower.contains("support image input")
            || body_lower.contains("no endpoints found"))
    {
        return AnalyseError::new(
            AnalyseErrorCode::ModelUnsupportedImageInput,
            format!(
                "Model '{}' on {} does not support image input.",
                provider.model, provider_name
            ),
            false,
        )
        .with_provider(provider_name, &provider.model)
        .with_hint("Pick a vision-capable model/provider for screenshot analysis.");
    }

    if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
        return AnalyseError::new(
            AnalyseErrorCode::ProviderAuthFailed,
            format!("{} authentication failed", provider_name),
            false,
        )
        .with_provider(provider_name, &provider.model)
        .with_hint("Check API key and base URL in server configuration.");
    }

    if status == StatusCode::TOO_MANY_REQUESTS {
        return AnalyseError::new(
            AnalyseErrorCode::ProviderRateLimited,
            format!("{} rate limit reached", provider_name),
            true,
        )
        .with_provider(provider_name, &provider.model)
        .with_hint("Wait briefly and retry, or switch provider.");
    }

    if status == StatusCode::REQUEST_TIMEOUT || status == StatusCode::GATEWAY_TIMEOUT {
        return AnalyseError::new(
            AnalyseErrorCode::ProviderTimeout,
            format!("{} timed out", provider_name),
            true,
        )
        .with_provider(provider_name, &provider.model)
        .with_hint("Retry with a smaller frame or switch provider.");
    }

    if body_lower.contains("out of memory")
        || body_lower.contains("insufficient memory")
        || body_lower.contains("requires more system memory")
    {
        return AnalyseError::new(
            AnalyseErrorCode::ProviderOutOfMemory,
            format!("{} ran out of memory", provider_name),
            true,
        )
        .with_provider(provider_name, &provider.model)
        .with_hint("Retry later or reduce frame resolution/quality.");
    }

    let retryable = status.is_server_error();
    AnalyseError::new(
        AnalyseErrorCode::ProviderUnavailable,
        format!("{} error {}: {}", provider_name, status.as_u16(), error_text),
        retryable,
    )
    .with_provider(provider_name, &provider.model)
    .with_hint("Check provider status and configuration.")
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
