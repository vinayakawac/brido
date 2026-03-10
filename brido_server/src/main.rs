#![windows_subsystem = "windows"]

mod ai_server;
mod capture;
mod config;
mod encoder;
mod model_manager;
mod stream_server;
mod tls;
mod ui;

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
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
use ui::window::BridoApp;

pub struct AppState {
    pub config: Config,
    pub frame_tx: broadcast::Sender<Vec<u8>>,
    pub active_tokens: RwLock<HashSet<String>>,
    pub http_client: reqwest::Client,
    pub connected_count: Arc<AtomicUsize>,
    /// Keeps one receiver alive so the capture thread doesn't exit when no WebSocket clients are connected.
    _keep_alive_rx: broadcast::Receiver<Vec<u8>>,
}

/// Spawns the axum server + screen capture on a background thread with its own tokio runtime.
/// Accepts shared `server_ready` and `connected_count` so the GUI can track state across restarts.
/// Returns an `axum_server::Handle` that can be used to shut the server down.
pub fn start_server(
    config: Config,
    server_ready: Arc<AtomicBool>,
    connected_count: Arc<AtomicUsize>,
) -> axum_server::Handle {
    server_ready.store(false, Ordering::SeqCst);
    connected_count.store(0, Ordering::SeqCst);

    let ready_clone = server_ready;
    let count_clone = connected_count;
    let handle = axum_server::Handle::new();
    let handle_for_server = handle.clone();

    let ip = local_ip_address::local_ip()
        .map(|ip| ip.to_string())
        .unwrap_or_else(|_| "127.0.0.1".to_string());

    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("Failed to create tokio runtime");

        rt.block_on(async move {
            let port = config.port;
            let fps = config.capture_fps;
            let target_w = config.target_width;
            let target_h = config.target_height;
            let quality = config.capture_quality;

            let (frame_tx, keep_alive_rx) = broadcast::channel::<Vec<u8>>(8);
            let tx = frame_tx.clone();

            // Screen capture in a dedicated OS thread (scrap types are !Send)
            std::thread::spawn(move || {
                let mut cap = match ScreenCapture::new() {
                    Ok(c) => c,
                    Err(e) => {
                        eprintln!("Screen capture init failed: {e}");
                        return;
                    }
                };

                let encoder = FrameEncoder::new(target_w, target_h, quality);
                let interval = Duration::from_millis(1000 / fps as u64);

                loop {
                    let start = Instant::now();
                    if let Ok(rgb) = cap.capture_frame() {
                        if let Ok(jpeg) =
                            encoder.encode(&rgb, cap.width() as u32, cap.height() as u32)
                        {
                            // Exit if no receivers (server was shut down)
                            if tx.send(jpeg).is_err() {
                                break;
                            }
                        }
                    }
                    let elapsed = start.elapsed();
                    if elapsed < interval {
                        std::thread::sleep(interval - elapsed);
                    }
                }
            });

            let connected_count_clone = count_clone;

            let state = Arc::new(AppState {
                config,
                frame_tx,
                active_tokens: RwLock::new(HashSet::new()),
                http_client: reqwest::Client::new(),
                connected_count: connected_count_clone,
                _keep_alive_rx: keep_alive_rx,
            });

            let app = Router::new()
                .route("/api/connect", post(ai_server::handle_connect))
                .route("/api/qr-info", get(ai_server::handle_qr_info))
                .route("/api/system-info", get(ai_server::handle_system_info))
                .route("/api/models", get(ai_server::handle_models))
                .route("/api/analyse", post(ai_server::handle_analyse))
                .route("/ws/stream", get(stream_server::ws_handler))
                .layer(CorsLayer::permissive())
                .with_state(state);

            let listener = loop {
                match tokio::net::TcpListener::bind(format!("0.0.0.0:{port}")).await {
                    Ok(l) => break l,
                    Err(e) => {
                        eprintln!("Port {port} busy, retrying… ({e})");
                        tokio::time::sleep(Duration::from_millis(300)).await;
                    }
                }
            };

            ready_clone.store(true, Ordering::SeqCst);
            println!("  Server ready — listening on https://{ip}:{port}");
            tracing::info!("Listening (HTTPS) on 0.0.0.0:{port}");

            // Generate self-signed TLS certificate
            let tls_cert = tls::generate_self_signed_cert(&ip);
            let rustls_config = axum_server::tls_rustls::RustlsConfig::from_pem(
                tls_cert.cert_pem,
                tls_cert.key_pem,
            )
            .await
            .expect("Failed to create TLS config");

            axum_server::from_tcp_rustls(listener.into_std().unwrap(), rustls_config)
                .handle(handle_for_server)
                .serve(app.into_make_service())
                .await
                .ok();

            println!("  Server stopped.");
        });
    });

    handle
}

fn main() {
    tracing_subscriber::fmt::init();

    let config = Config::default();
    let ip = local_ip_address::local_ip()
        .map(|ip| ip.to_string())
        .unwrap_or_else(|_| "unknown".to_string());
    let pin = config.pin.clone();
    let port = config.port;

    println!();
    println!("╔═══════════════════════════════════════╗");
    println!("║           Brido Server                ║");
    println!("╠═══════════════════════════════════════╣");
    println!("║  IP Address : {:<23} ║", ip);
    println!("║  Port       : {:<23} ║", port);
    println!("║  PIN        : {:<23} ║", pin);
    println!("╚═══════════════════════════════════════╝");
    println!();

    // Flags shared between UI and server lifecycle
    let shutdown_flag = Arc::new(AtomicBool::new(false));
    let restart_flag = Arc::new(AtomicBool::new(false));
    let server_ready = Arc::new(AtomicBool::new(false));
    let connected_count = Arc::new(AtomicUsize::new(0));

    // Start the network server + capture on a background thread
    let axum_handle = start_server(config, server_ready.clone(), connected_count.clone());

    // Build the egui application
    let app = BridoApp::new(
        ip,
        pin,
        port,
        shutdown_flag.clone(),
        restart_flag.clone(),
        server_ready.clone(),
        connected_count,
        axum_handle,
    );

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([400.0, 600.0])
            .with_min_inner_size([360.0, 520.0])
            .with_title("Brido Server")
            .with_decorations(true),
        ..Default::default()
    };

    // Wait a moment for the server to bind
    let start = Instant::now();
    while !server_ready.load(Ordering::SeqCst) && start.elapsed() < Duration::from_secs(5) {
        std::thread::sleep(Duration::from_millis(50));
    }

    eframe::run_native(
        "Brido Server",
        native_options,
        Box::new(|_cc| Ok(Box::new(app))),
    )
    .ok();

    // When the window closes, make sure everything shuts down
    shutdown_flag.store(true, Ordering::SeqCst);
    std::process::exit(0);
}
