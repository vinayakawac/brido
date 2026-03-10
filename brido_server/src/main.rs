mod ai_server;
mod capture;
mod config;
mod encoder;
mod model_manager;
mod stream_server;

use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::{
    routing::{get, post},
    Router,
};
use tokio::sync::{broadcast, RwLock};
use tower_http::cors::CorsLayer;

use capture::ScreenCapture;
use config::Config;
use encoder::FrameEncoder;

pub struct AppState {
    pub config: Config,
    pub frame_tx: broadcast::Sender<Vec<u8>>,
    pub active_tokens: RwLock<HashSet<String>>,
    pub http_client: reqwest::Client,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let config = Config::default();
    let port = config.port;
    let fps = config.capture_fps;
    let target_w = config.target_width;
    let target_h = config.target_height;
    let quality = config.capture_quality;

    let local_ip = local_ip_address::local_ip()
        .map(|ip| ip.to_string())
        .unwrap_or_else(|_| "unknown".to_string());

    println!();
    println!("╔═══════════════════════════════════════╗");
    println!("║           Brido Server                ║");
    println!("╠═══════════════════════════════════════╣");
    println!("║  IP Address : {:<23} ║", local_ip);
    println!("║  Port       : {:<23} ║", port);
    println!("║  PIN        : {:<23} ║", config.pin);
    println!("╚═══════════════════════════════════════╝");
    println!();

    let (frame_tx, _) = broadcast::channel::<Vec<u8>>(2);
    let tx = frame_tx.clone();

    // Screen capture runs in a dedicated OS thread (scrap types are !Send)
    std::thread::spawn(move || {
        let mut capture = match ScreenCapture::new() {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Screen capture init failed: {e}");
                return;
            }
        };

        let encoder = FrameEncoder::new(target_w, target_h, quality);
        let interval = Duration::from_millis(1000 / fps as u64);

        tracing::info!(
            "Capturing {}×{} → {}×{} @ {} fps",
            capture.width(),
            capture.height(),
            target_w,
            target_h,
            fps
        );

        loop {
            let start = Instant::now();
            if let Ok(rgb) = capture.capture_frame() {
                if let Ok(jpeg) =
                    encoder.encode(&rgb, capture.width() as u32, capture.height() as u32)
                {
                    let _ = tx.send(jpeg);
                }
            }
            let elapsed = start.elapsed();
            if elapsed < interval {
                std::thread::sleep(interval - elapsed);
            }
        }
    });

    let state = Arc::new(AppState {
        config,
        frame_tx,
        active_tokens: RwLock::new(HashSet::new()),
        http_client: reqwest::Client::new(),
    });

    let app = Router::new()
        .route("/api/connect", post(ai_server::handle_connect))
        .route("/api/system-info", get(ai_server::handle_system_info))
        .route("/api/models", get(ai_server::handle_models))
        .route("/api/analyse", post(ai_server::handle_analyse))
        .route("/ws/stream", get(stream_server::ws_handler))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}")).await?;
    tracing::info!("Listening on 0.0.0.0:{port}");
    axum::serve(listener, app).await?;

    Ok(())
}
