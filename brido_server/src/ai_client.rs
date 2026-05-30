//! Direct AI provider client for the overlay.
//!
//! Calls `ModelManager` in-process instead of going through HTTP.
//! Preserves the full provider failover chain.

use brido_server::config::Config;
use brido_server::model_manager::ModelManager;
use std::time::Duration;

/// Analyse an image via the configured AI providers.
///
/// Returns `(response_text, model_used)` on success, or an error string.
pub async fn analyse(
    config: &Config,
    image_base64: &str,
    custom_prompt: Option<&str>,
) -> Result<(String, String), String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(60))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {e}"))?;

    let manager = ModelManager::new(config, &client);

    manager
        .analyse_image(image_base64, None, custom_prompt)
        .await
        .map_err(|e| {
            let hint = e.hint.as_deref().unwrap_or("");
            format!("[{}] {} {}", e.code.as_str(), e.message, hint)
        })
}
