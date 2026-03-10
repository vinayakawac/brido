use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use crate::model_manager::ModelManager;
use crate::AppState;

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

// ── Handlers ────────────────────────────────────────────────────────────────

pub async fn handle_connect(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ConnectRequest>,
) -> Result<Json<ConnectResponse>, StatusCode> {
    if req.pin != state.config.pin {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let token = Uuid::new_v4().to_string();
    state.active_tokens.write().await.insert(token.clone());

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
    Ok(Json(get_supported_models()))
}

pub async fn handle_analyse(
    headers: HeaderMap,
    State(state): State<Arc<AppState>>,
    Json(req): Json<AnalyseRequest>,
) -> Result<Json<AnalyseResponse>, StatusCode> {
    verify_token(&headers, &state).await?;

    let model = req
        .model
        .unwrap_or_else(|| state.config.default_vision_model.clone());

    let manager = ModelManager::new(&state.config.ollama_url, &state.http_client);

    let result = manager
        .analyse_image(&req.image_base64, &model, req.prompt.as_deref())
        .await
        .map_err(|e| {
            tracing::error!("Analysis failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(AnalyseResponse {
        result,
        model_used: model,
    }))
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

fn get_supported_models() -> Vec<ModelInfo> {
    vec![
        ModelInfo {
            name: "qwen/qwen3-vl-8b".to_string(),
            file: "qwen3-VL-8B-Instruct-Q4_K_M.gguf".to_string(),
            capability: "vision, tool use".to_string(),
            size_gb: 6.19,
        },
        ModelInfo {
            name: "google/gemma-3-4b".to_string(),
            file: "gemma-3-4b-it-Q4_K_M.gguf".to_string(),
            capability: "vision".to_string(),
            size_gb: 3.34,
        },
        ModelInfo {
            name: "deepseek/deepseek-r1-0528-qwen3-8b".to_string(),
            file: "deepseek-r1-0528-qwen3-8b-Q4_K_M.gguf".to_string(),
            capability: "reasoning".to_string(),
            size_gb: 5.03,
        },
    ]
}
