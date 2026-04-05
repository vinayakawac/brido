use rand::Rng;
use std::env;

#[derive(Clone)]
pub struct Config {
    pub port: u16,
    pub pin: String,
    pub capture_fps: u32,
    pub capture_quality: u8,
    pub target_width: u32,
    pub target_height: u32,
    pub openai_api_key: String,
    pub openai_base_url: String,
    pub openai_model: String,
    pub anthropic_api_key: String,
    pub anthropic_base_url: String,
    pub anthropic_model: String,
    pub gemini_api_key: String,
    pub gemini_base_url: String,
    pub gemini_model: String,
    pub openrouter_api_key: String,
    pub openrouter_base_url: String,
    pub openrouter_model: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            port: 8080,
            pin: generate_pin(),
            capture_fps: 15,
            capture_quality: 65,
            target_width: 1280,
            target_height: 720,
            openai_api_key: env::var("OPENAI_API_KEY").unwrap_or_default(),
            openai_base_url: env_or_default("OPENAI_BASE_URL", "https://api.openai.com/v1"),
            openai_model: env_or_default("OPENAI_MODEL", "gpt-4.1-mini"),
            anthropic_api_key: env::var("ANTHROPIC_API_KEY").unwrap_or_default(),
            anthropic_base_url: env_or_default("ANTHROPIC_BASE_URL", "https://api.anthropic.com/v1"),
            anthropic_model: env_or_default("ANTHROPIC_MODEL", "claude-3-5-sonnet-latest"),
            gemini_api_key: env::var("GEMINI_API_KEY").unwrap_or_default(),
            gemini_base_url: env_or_default("GEMINI_BASE_URL", "https://generativelanguage.googleapis.com/v1beta"),
            gemini_model: env_or_default("GEMINI_MODEL", "gemini-2.0-flash"),
            openrouter_api_key: env::var("OPENROUTER_API_KEY").unwrap_or_default(),
            openrouter_base_url: env_or_default("OPENROUTER_BASE_URL", "https://openrouter.ai/api/v1"),
            openrouter_model: env_or_default("OPENROUTER_MODEL", "google/gemini-2.5-flash"),
        }
    }
}

fn env_or_default(key: &str, fallback: &str) -> String {
    env::var(key)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| fallback.to_string())
}

fn generate_pin() -> String {
    let mut rng = rand::thread_rng();
    format!("{:06}", rng.gen_range(0..1_000_000u32))
}
