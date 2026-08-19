use axum::{
    extract::{ConnectInfo, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use uuid::Uuid;

use crate::auth::{PinOutcome, TokenKind};
use crate::server::AppState;
use brido_server::model_manager::{AnalyseError, AnalyseErrorCode, ModelManager, ProviderAttempt};

// ── Request / Response types ────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct ConnectRequest {
    pub pin: String,
    /// When set, the issued token is long-lived and persisted, so this device
    /// can reconnect later without the PIN.
    #[serde(default)]
    pub trust_device: bool,
}

#[derive(Serialize)]
pub struct ConnectResponse {
    pub token: String,
    pub system_info: SystemInfo,
    /// Seconds until this token expires, so the app knows when to re-pair.
    pub expires_in: u64,
    /// Model the server will use when a request does not name one.
    pub default_model: String,
    /// Provider settings, delivered with the handshake rather than as a second
    /// round trip — the phone's settings screen is populated the instant it
    /// connects, and this payload is never written to storage on the device.
    pub settings: SettingsPayload,
    pub providers: Vec<ProviderOption>,
}

#[derive(Serialize, Clone)]
pub struct SystemInfo {
    pub storage: String,
    pub storage_used: String,
    pub gpu: String,
    pub gpu_detail: String,
    pub ram: String,
    pub ram_speed: String,
    pub processor: String,
    pub processor_speed: String,
}

#[derive(Serialize)]
pub struct ModelInfo {
    pub name: String,
    pub file: String,
    pub capability: String,
    pub size_gb: f64,
}

#[derive(Deserialize)]
pub struct AnalyseRequest {
    pub image_base64: String,
    pub model: Option<String>,
    pub prompt: Option<String>,
}

/// Remote-keyboard request: text to type into the focused window on the PC.
#[derive(Deserialize)]
pub struct TypeRequest {
    pub text: String,
    /// Number of backspaces to send before typing (phone-side edits).
    #[serde(default)]
    pub backspaces: usize,
}

#[derive(Serialize)]
pub struct TypeResponse {
    pub typed: usize,
}

#[derive(Serialize)]
pub struct AnalyseResponse {
    pub result: String,
    pub model_used: String,
}

#[derive(Serialize)]
pub struct AnalyseErrorResponse {
    pub code: String,
    pub message: String,
    pub retryable: bool,
    pub hint: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub attempts: Vec<ProviderAttempt>,
    pub request_id: String,
}

/// Provider settings mirrored to the phone.
///
/// This carries real API keys, which is why it is only ever returned over the
/// certificate-pinned connection to an authenticated client, and why the app
/// holds it in memory only and drops it the moment it disconnects.
#[derive(Serialize, Clone, Default)]
pub struct SettingsPayload {
    pub active_provider: String,
    pub gemini_api_key: String,
    pub gemini_model: String,
    pub openrouter_api_key: String,
    pub openrouter_model: String,
    pub ollama_api_key: String,
    pub ollama_base_url: String,
    pub ollama_model: String,
    pub deepgram_api_key: String,
    pub asr_model: String,
    pub resume_text: String,
    pub job_description_text: String,
}

/// Incoming settings update.
///
/// Every field is optional and **absent fields are left untouched**. An earlier
/// version reused the full payload here, so a partial update silently erased
/// every field the caller omitted — a one-line curl wiped stored API keys.
/// Clearing a value now requires explicitly sending an empty string.
#[derive(Deserialize, Default)]
#[serde(default)]
pub struct SettingsUpdate {
    pub active_provider: Option<String>,
    pub gemini_api_key: Option<String>,
    pub gemini_model: Option<String>,
    pub openrouter_api_key: Option<String>,
    pub openrouter_model: Option<String>,
    pub ollama_api_key: Option<String>,
    pub ollama_base_url: Option<String>,
    pub ollama_model: Option<String>,
    pub deepgram_api_key: Option<String>,
    pub asr_model: Option<String>,
    pub resume_text: Option<String>,
    pub job_description_text: Option<String>,
}

impl SettingsUpdate {
    /// Applies only the fields the caller actually sent.
    fn merge_into(self, cfg: &mut brido_server::config::Config) {
        fn set(target: &mut String, value: Option<String>) {
            if let Some(v) = value {
                *target = v;
            }
        }
        set(&mut cfg.active_provider, self.active_provider);
        set(&mut cfg.gemini_api_key, self.gemini_api_key);
        set(&mut cfg.gemini_model, self.gemini_model);
        set(&mut cfg.openrouter_api_key, self.openrouter_api_key);
        set(&mut cfg.openrouter_model, self.openrouter_model);
        set(&mut cfg.ollama_api_key, self.ollama_api_key);
        set(&mut cfg.ollama_base_url, self.ollama_base_url);
        set(&mut cfg.ollama_model, self.ollama_model);
        set(&mut cfg.deepgram_api_key, self.deepgram_api_key);
        set(&mut cfg.asr_model, self.asr_model);
        set(&mut cfg.resume_text, self.resume_text);
        set(&mut cfg.job_description_text, self.job_description_text);
    }
}

/// Everything the phone's settings screen needs, in one payload.
#[derive(Serialize)]
pub struct SettingsResponse {
    #[serde(flatten)]
    pub settings: SettingsPayload,
    /// Selectable providers and their known models, so the phone does not have
    /// to hardcode a list that would drift from the desktop's.
    pub providers: Vec<ProviderOption>,
}

#[derive(Serialize, Clone)]
pub struct ProviderOption {
    pub label: String,
    pub models: Vec<String>,
    pub default_model: String,
}

impl SettingsPayload {
    fn from_config(cfg: &brido_server::config::Config) -> Self {
        Self {
            active_provider: cfg.active_provider.clone(),
            gemini_api_key: cfg.gemini_api_key.clone(),
            gemini_model: cfg.gemini_model.clone(),
            openrouter_api_key: cfg.openrouter_api_key.clone(),
            openrouter_model: cfg.openrouter_model.clone(),
            ollama_api_key: cfg.ollama_api_key.clone(),
            ollama_base_url: cfg.ollama_base_url.clone(),
            ollama_model: cfg.ollama_model.clone(),
            deepgram_api_key: cfg.deepgram_api_key.clone(),
            asr_model: cfg.asr_model.clone(),
            resume_text: cfg.resume_text.clone(),
            job_description_text: cfg.job_description_text.clone(),
        }
    }

}

fn provider_options() -> Vec<ProviderOption> {
    use brido_server::config::ProviderKind;
    ProviderKind::ALL
        .iter()
        .map(|kind| ProviderOption {
            label: kind.label().to_string(),
            models: kind.available_models().iter().map(|m| m.to_string()).collect(),
            default_model: kind.default_model().to_string(),
        })
        .collect()
}

// ── Handlers ────────────────────────────────────────────────────────────────

/// Returns the desktop's current provider settings.
pub async fn handle_get_settings(
    headers: HeaderMap,
    State(state): State<Arc<AppState>>,
) -> Result<Json<SettingsResponse>, StatusCode> {
    verify_token(&headers, &state).await?;
    let cfg = state.config();
    Ok(Json(SettingsResponse {
        settings: SettingsPayload::from_config(&cfg),
        providers: provider_options(),
    }))
}

/// Applies settings edited on the phone, persisting them like the GUI does.
///
/// The body is taken as a raw string and parsed *after* the token check, so an
/// unauthenticated caller cannot probe the payload schema by comparing a
/// "malformed body" rejection against an "unauthorized" one.
pub async fn handle_put_settings(
    headers: HeaderMap,
    State(state): State<Arc<AppState>>,
    body: String,
) -> Result<Json<SettingsResponse>, StatusCode> {
    verify_token(&headers, &state).await?;

    let req: SettingsUpdate =
        serde_json::from_str(&body).map_err(|_| StatusCode::UNPROCESSABLE_ENTITY)?;

    // Update the shared config first so in-flight requests pick it up
    // immediately, then persist to disk.
    let updated = {
        let mut guard = state
            .config
            .write()
            .unwrap_or_else(|e| e.into_inner());
        req.merge_into(&mut guard);
        guard.clone()
    };

    let models = {
        use brido_server::config::ProviderKind;
        let mut map = std::collections::HashMap::new();
        map.insert(ProviderKind::Gemini, updated.gemini_model.clone());
        map.insert(ProviderKind::OpenRouter, updated.openrouter_model.clone());
        map.insert(ProviderKind::Ollama, updated.ollama_model.clone());
        map
    };

    if let Err(e) = brido_server::config::save_overlay_settings(
        &state.runtime_env,
        &updated.active_provider,
        &updated.asr_model,
        &updated.gemini_api_key,
        &updated.openrouter_api_key,
        &updated.ollama_api_key,
        &updated.ollama_base_url,
        &updated.deepgram_api_key,
        &updated.resume_text,
        &updated.job_description_text,
        &updated.overlay_hotkey_capture,
        &updated.overlay_hotkey_toggle,
        &updated.overlay_hotkey_settings,
        &updated.overlay_hotkey_stealth,
        &updated.overlay_hotkey_direct_type,
        updated.strict_stealth_mode,
        &models,
    ) {
        tracing::error!("Failed to persist settings from phone: {e}");
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }

    // Tell the overlay to reload so the desktop UI reflects the change.
    state.settings_version.fetch_add(1, Ordering::SeqCst);
    tracing::info!("Settings updated from paired device");

    Ok(Json(SettingsResponse {
        settings: SettingsPayload::from_config(&updated),
        providers: provider_options(),
    }))
}

/// Exchanges the pairing PIN for a bearer token.
///
/// The PIN is compared in constant time and every attempt is throttled per
/// client address, so a six-digit PIN cannot be brute-forced over the network.
pub async fn handle_connect(
    State(state): State<Arc<AppState>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    Json(req): Json<ConnectRequest>,
) -> Result<Json<ConnectResponse>, axum::response::Response> {
    use axum::response::IntoResponse;

    let cfg = state.config();

    match state
        .auth
        .check_pin(peer.ip(), &req.pin, &cfg.pin)
        .await
    {
        PinOutcome::Ok => {}
        PinOutcome::Invalid => return Err(StatusCode::UNAUTHORIZED.into_response()),
        PinOutcome::LockedOut { retry_after } => {
            // Tell the client exactly how long to back off.
            let secs = retry_after.as_secs().max(1);
            return Err((
                StatusCode::TOO_MANY_REQUESTS,
                [("Retry-After", secs.to_string())],
                format!("Too many failed PIN attempts. Try again in {secs}s."),
            )
                .into_response());
        }
    }

    let kind = if req.trust_device {
        TokenKind::Trusted
    } else {
        TokenKind::Session
    };
    let token = state.auth.issue_token(kind).await;
    state.connected_count.fetch_add(1, Ordering::SeqCst);

    let ttl = match kind {
        TokenKind::Trusted => crate::auth::TRUSTED_TTL,
        TokenKind::Session => crate::auth::SESSION_TTL,
    };

    Ok(Json(ConnectResponse {
        token,
        system_info: get_system_info(),
        expires_in: ttl.as_secs(),
        default_model: cfg.active_model().to_string(),
        settings: SettingsPayload::from_config(&cfg),
        providers: provider_options(),
    }))
}

/// Revokes the caller's token so it cannot be replayed after sign-out.
pub async fn handle_disconnect(
    headers: HeaderMap,
    State(state): State<Arc<AppState>>,
) -> StatusCode {
    let Some(token) = bearer_token(&headers) else {
        return StatusCode::BAD_REQUEST;
    };

    if state.auth.revoke(&token).await {
        // Saturating so a double disconnect cannot wrap the counter.
        let _ = state
            .connected_count
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| {
                Some(n.saturating_sub(1))
            });
        StatusCode::NO_CONTENT
    } else {
        StatusCode::UNAUTHORIZED
    }
}

/// Types text into whatever window has focus on the PC (remote keyboard).
///
/// This is a plain input-injection endpoint, like a Bluetooth keyboard: it
/// synthesises normal key events for the focused app and does nothing to hide
/// itself. Runs on a blocking thread because `SendInput` is a blocking Win32
/// call.
pub async fn handle_type(
    headers: HeaderMap,
    State(state): State<Arc<AppState>>,
    Json(req): Json<TypeRequest>,
) -> Result<Json<TypeResponse>, StatusCode> {
    verify_token(&headers, &state).await?;

    // Bound the payload so a single request cannot flood the input queue.
    if req.text.chars().count() > 10_000 || req.backspaces > 1_000 {
        return Err(StatusCode::PAYLOAD_TOO_LARGE);
    }

    let text = req.text.clone();
    let backspaces = req.backspaces;

    let typed = tokio::task::spawn_blocking(move || {
        if backspaces > 0 {
            crate::remote_type::backspace(backspaces);
        }
        crate::remote_type::type_text(&text)
    })
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(TypeResponse { typed }))
}

/// Extracts a `Bearer <token>` value from the Authorization header.
fn bearer_token(headers: &HeaderMap) -> Option<String> {
    headers
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
}

pub async fn handle_system_info(
    headers: HeaderMap,
    State(state): State<Arc<AppState>>,
) -> Result<Json<SystemInfo>, StatusCode> {
    verify_token(&headers, &state).await?;
    Ok(Json(get_system_info()))
}

pub async fn handle_models(
    headers: HeaderMap,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<ModelInfo>>, StatusCode> {
    verify_token(&headers, &state).await?;
    Ok(Json(get_supported_models(&state.config())))
}

pub async fn handle_analyse(
    headers: HeaderMap,
    State(state): State<Arc<AppState>>,
    Json(req): Json<AnalyseRequest>,
) -> Result<Json<AnalyseResponse>, (StatusCode, Json<AnalyseErrorResponse>)> {
    let request_id = Uuid::new_v4().to_string();

    verify_token(&headers, &state)
        .await
        .map_err(|_| {
            (
                StatusCode::UNAUTHORIZED,
                Json(AnalyseErrorResponse {
                    code: "unauthorized".to_string(),
                    message: "Unauthorized request.".to_string(),
                    retryable: false,
                    hint: Some("Reconnect from the app to obtain a new token.".to_string()),
                    provider: None,
                    model: None,
                    attempts: Vec::new(),
                    request_id: request_id.clone(),
                }),
            )
        })?;

    let _permit = state
        .analysis_gate
        .acquire()
        .await
        .map_err(|_| {
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(AnalyseErrorResponse {
                    code: "analysis_queue_unavailable".to_string(),
                    message: "Analysis queue unavailable.".to_string(),
                    retryable: true,
                    hint: Some("Wait a moment and retry.".to_string()),
                    provider: None,
                    model: None,
                    attempts: Vec::new(),
                    request_id: request_id.clone(),
                }),
            )
        })?;

    // Snapshot the *current* settings for this request, so provider keys
    // edited in the GUI (or from the phone) take effect immediately.
    let cfg = state.config();
    let manager = ModelManager::new(&cfg, &state.http_client);

    let (result, model_used) = manager
        .analyse_image(&req.image_base64, req.model.as_deref(), req.prompt.as_deref())
        .await
        .map_err(|e| {
            tracing::error!(
                request_id = %request_id,
                error_code = e.code.as_str(),
                provider = ?e.provider,
                model = ?e.model,
                retryable = e.retryable,
                message = %e.message,
                "Analysis failed"
            );

            map_analyse_error(e, request_id.clone())
        })?;

    Ok(Json(AnalyseResponse {
        result,
        model_used,
    }))
}

fn map_analyse_error(
    error: AnalyseError,
    request_id: String,
) -> (StatusCode, Json<AnalyseErrorResponse>) {
    let status = match error.code {
        AnalyseErrorCode::ImagePayloadEmpty => StatusCode::BAD_REQUEST,
        AnalyseErrorCode::ImagePayloadTooLarge => StatusCode::PAYLOAD_TOO_LARGE,
        AnalyseErrorCode::NoProviderConfigured => StatusCode::SERVICE_UNAVAILABLE,
        AnalyseErrorCode::ModelUnsupportedImageInput => StatusCode::UNPROCESSABLE_ENTITY,
        AnalyseErrorCode::ProviderAuthFailed => StatusCode::BAD_GATEWAY,
        AnalyseErrorCode::ProviderRateLimited => StatusCode::TOO_MANY_REQUESTS,
        AnalyseErrorCode::ProviderTimeout => StatusCode::GATEWAY_TIMEOUT,
        AnalyseErrorCode::ProviderUnavailable => StatusCode::BAD_GATEWAY,
        AnalyseErrorCode::ProviderOutOfMemory => StatusCode::SERVICE_UNAVAILABLE,
        AnalyseErrorCode::NetworkError => StatusCode::BAD_GATEWAY,
        AnalyseErrorCode::ProviderReturnedEmpty => StatusCode::BAD_GATEWAY,
        AnalyseErrorCode::AllProvidersFailed => StatusCode::BAD_GATEWAY,
    };

    (
        status,
        Json(AnalyseErrorResponse {
            code: error.code.as_str().to_string(),
            message: error.message,
            retryable: error.retryable,
            hint: error.hint,
            provider: error.provider,
            model: error.model,
            attempts: error.attempts,
            request_id,
        }),
    )
}

// ── Helpers ─────────────────────────────────────────────────────────────────

async fn verify_token(headers: &HeaderMap, state: &Arc<AppState>) -> Result<(), StatusCode> {
    let token = bearer_token(headers).ok_or(StatusCode::UNAUTHORIZED)?;

    // Rejects unknown *and* expired tokens.
    if !state.auth.verify(&token).await {
        return Err(StatusCode::UNAUTHORIZED);
    }

    Ok(())
}

fn get_system_info() -> SystemInfo {
    use sysinfo::{Disks, System};

    let mut sys = System::new_all();
    sys.refresh_all();

    let total_ram_gb = sys.total_memory() as f64 / (1024.0 * 1024.0 * 1024.0);
    let cpu_name = sys
        .cpus()
        .first()
        .map(|c| c.brand().to_string())
        .unwrap_or_else(|| "Unknown".to_string());
    let cpu_freq = sys.cpus().first().map(|c| c.frequency()).unwrap_or(0);

    let disks = Disks::new_with_refreshed_list();
    let total_storage: u64 = disks.iter().map(|d| d.total_space()).sum();
    let used_storage: u64 = disks
        .iter()
        .map(|d| d.total_space() - d.available_space())
        .sum();
    let total_gb = total_storage / (1024 * 1024 * 1024);
    let used_gb = used_storage / (1024 * 1024 * 1024);

    SystemInfo {
        storage: format!("{} GB", total_gb),
        storage_used: format!("{} GB of {} GB used", used_gb, total_gb),
        gpu: detect_gpu(),
        gpu_detail: String::new(),
        ram: format!("{:.1} GB", total_ram_gb),
        ram_speed: String::new(),
        processor: cpu_name,
        processor_speed: format!("{:.2} GHz", cpu_freq as f64 / 1000.0),
    }
}

fn detect_gpu() -> String {
    #[cfg(target_os = "windows")]
    {
        // Try to read GPU name via Windows WMI
        if let Ok(output) = std::process::Command::new("wmic")
            .args(["path", "win32_VideoController", "get", "Name"])
            .output()
        {
            let text = String::from_utf8_lossy(&output.stdout);
            if let Some(line) = text.lines().nth(1) {
                let name = line.trim();
                if !name.is_empty() {
                    return name.to_string();
                }
            }
        }
    }
    "GPU info unavailable".to_string()
}

fn get_supported_models(config: &crate::config::Config) -> Vec<ModelInfo> {
    ModelManager::available_models(config)
        .into_iter()
        .map(|(name, file, capability, size_gb)| ModelInfo {
            name,
            file,
            capability,
            size_gb,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use brido_server::config::Config;
    use std::sync::RwLock;

    /// The bug this guards against: the HTTP server held a Config cloned at
    /// startup, so a key entered in the desktop Settings panel never reached
    /// phone requests — analysis failed with "authentication failed" while the
    /// desktop's own capture worked with the new key.
    #[test]
    fn shared_config_reflects_later_edits() {
        let shared = Arc::new(RwLock::new(Config::default()));

        // Snapshot taken the way a handler takes one.
        let before = shared.read().unwrap().clone();
        assert_ne!(before.openrouter_api_key, "sk-or-new-key");

        // Simulate the GUI publishing edited settings.
        shared.write().unwrap().openrouter_api_key = "sk-or-new-key".to_string();

        let after = shared.read().unwrap().clone();
        assert_eq!(after.openrouter_api_key, "sk-or-new-key");
    }

    #[test]
    fn settings_payload_reads_every_field_from_config() {
        let mut cfg = Config::default();
        cfg.active_provider = "OpenRouter".to_string();
        cfg.openrouter_api_key = "sk-or-secret".to_string();
        cfg.openrouter_model = "some/model".to_string();

        let payload = SettingsPayload::from_config(&cfg);
        assert_eq!(payload.openrouter_api_key, "sk-or-secret");
        assert_eq!(payload.active_provider, "OpenRouter");
        assert_eq!(payload.openrouter_model, "some/model");
    }

    /// Regression: a partial update once cleared every omitted field, which
    /// erased stored API keys. Omitted fields must survive untouched.
    #[test]
    fn partial_update_preserves_unmentioned_fields() {
        let mut cfg = Config::default();
        cfg.gemini_api_key = "gemini-secret".to_string();
        cfg.openrouter_api_key = "openrouter-secret".to_string();
        cfg.deepgram_api_key = "deepgram-secret".to_string();
        cfg.resume_text = "my resume".to_string();

        // Only the model is being changed.
        let update: SettingsUpdate =
            serde_json::from_str(r#"{"openrouter_model":"new/model"}"#).expect("parses");
        update.merge_into(&mut cfg);

        assert_eq!(cfg.openrouter_model, "new/model");
        assert_eq!(cfg.gemini_api_key, "gemini-secret");
        assert_eq!(cfg.openrouter_api_key, "openrouter-secret");
        assert_eq!(cfg.deepgram_api_key, "deepgram-secret");
        assert_eq!(cfg.resume_text, "my resume");
    }

    /// Clearing a value must still be possible — but only explicitly.
    #[test]
    fn explicit_empty_string_clears_a_field() {
        let mut cfg = Config::default();
        cfg.gemini_api_key = "gemini-secret".to_string();

        let update: SettingsUpdate =
            serde_json::from_str(r#"{"gemini_api_key":""}"#).expect("parses");
        update.merge_into(&mut cfg);

        assert_eq!(cfg.gemini_api_key, "");
    }

    /// A request that names no model must resolve to the active provider's
    /// model, never to a hardcoded free-tier one.
    #[test]
    fn active_model_follows_selected_provider() {
        let mut cfg = Config::default();
        cfg.gemini_model = "gemini-x".to_string();
        cfg.openrouter_model = "openrouter-x".to_string();
        cfg.ollama_model = "ollama-x".to_string();

        cfg.active_provider = "Gemini".to_string();
        assert_eq!(cfg.active_model(), "gemini-x");

        cfg.active_provider = "OpenRouter".to_string();
        assert_eq!(cfg.active_model(), "openrouter-x");

        cfg.active_provider = "Ollama".to_string();
        assert_eq!(cfg.active_model(), "ollama-x");
    }

    /// Partial payloads must deserialize, since the handler parses the body
    /// itself after authenticating.
    #[test]
    fn partial_settings_payload_deserialises() {
        let parsed: SettingsUpdate =
            serde_json::from_str(r#"{"active_provider":"Gemini"}"#).expect("should parse");
        assert_eq!(parsed.active_provider.as_deref(), Some("Gemini"));
        assert!(parsed.openrouter_api_key.is_none());
    }
}
