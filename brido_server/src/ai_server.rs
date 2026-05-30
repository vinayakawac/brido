use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use uuid::Uuid;

use brido_server::model_manager::{AnalyseError, AnalyseErrorCode, ModelManager, ProviderAttempt};
use crate::server::AppState;

// ── Request / Response types ────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct ConnectRequest {
    pub pin: String,
}

#[derive(Serialize)]
pub struct ConnectResponse {
    pub token: String,
    pub system_info: SystemInfo,
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

#[derive(Serialize)]
pub struct QrInfoResponse {
    pub ip: String,
    pub port: u16,
    pub pin: String,
}

// ── Handlers ────────────────────────────────────────────────────────────────

/// No auth required — returns info for QR code generation
pub async fn handle_qr_info(
    State(state): State<Arc<AppState>>,
) -> Json<QrInfoResponse> {
    let ip = local_ip_address::local_ip()
        .map(|ip| ip.to_string())
        .unwrap_or_else(|_| "0.0.0.0".to_string());
    Json(QrInfoResponse {
        ip,
        port: state.config.port,
        pin: state.config.pin.clone(),
    })
}

pub async fn handle_connect(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ConnectRequest>,
) -> Result<Json<ConnectResponse>, StatusCode> {
    if req.pin != state.config.pin {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let token = Uuid::new_v4().to_string();
    state.active_tokens.write().await.insert(token.clone());
    state.connected_count.fetch_add(1, Ordering::SeqCst);

    let system_info = get_system_info();
    Ok(Json(ConnectResponse { token, system_info }))
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
    Ok(Json(get_supported_models(&state.config)))
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

    let manager = ModelManager::new(&state.config, &state.http_client);

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
    let token = headers
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or(StatusCode::UNAUTHORIZED)?;

    if !state.active_tokens.read().await.contains(token) {
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
