use crate::ai_server;
use crate::auth::AuthState;
use crate::stream_server;
use crate::tls;

// Shared modules come from the library crate.
use brido_server::capture;
use brido_server::config;
use brido_server::encoder;

use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::{
    routing::{get, post},
    Router,
};
use tokio::sync::{broadcast, Semaphore};

use capture::{CaptureMethod, ScreenCapture};
use config::Config;
use encoder::FrameEncoder;


pub struct AppState {
    /// Live configuration shared with the desktop overlay.
    ///
    /// This used to be a snapshot taken at startup, which meant provider keys
    /// edited in the GUI never reached the HTTP API — the phone kept
    /// authenticating with whatever key existed when the process launched.
    /// Sharing the value keeps both paths on the same settings.
    pub config: Arc<std::sync::RwLock<Config>>,
    /// Bumped whenever settings change, so the overlay can pick up edits made
    /// from the phone.
    pub settings_version: Arc<AtomicU64>,
    /// Where to persist settings when the phone updates them.
    pub runtime_env: config::RuntimeEnvPaths,
    pub frame_tx: broadcast::Sender<Vec<u8>>,
    /// Token issuing/verification and PIN throttling.
    pub auth: AuthState,
    pub http_client: reqwest::Client,
    pub analysis_gate: Semaphore,
    pub connected_count: Arc<AtomicUsize>,
    /// Keeps one receiver alive so the capture thread doesn't exit when no WebSocket clients are connected.
    _keep_alive_rx: broadcast::Receiver<Vec<u8>>,
}

impl AppState {
    /// Cheap clone of the current settings.
    ///
    /// Handlers take a snapshot rather than holding the lock, so the guard is
    /// never held across an await point.
    pub fn config(&self) -> Config {
        self.config
            .read()
            .map(|c| c.clone())
            .unwrap_or_else(|e| e.into_inner().clone())
    }
}

/// Spawns the axum server + screen capture on a background thread with its own tokio runtime.
/// Accepts shared `server_ready` and `connected_count` so the GUI can track state across restarts.
/// Returns an `axum_server::Handle` that can be used to shut the server down.
///
/// `cert_fingerprint` is filled in once TLS is up, so the GUI can put the
/// certificate hash into the pairing QR code.
pub fn start_server(
    config: Arc<std::sync::RwLock<Config>>,
    settings_version: Arc<AtomicU64>,
    runtime_env: config::RuntimeEnvPaths,
    server_ready: Arc<AtomicBool>,
    connected_count: Arc<AtomicUsize>,
    cert_fingerprint: Arc<std::sync::RwLock<Option<String>>>,
    trusted_store: std::path::PathBuf,
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
            // Capture parameters are fixed for the lifetime of the server, so
            // a snapshot is fine here; only provider settings change at runtime.
            let (port, fps, target_w, target_h, quality) = {
                let cfg = config.read().unwrap_or_else(|e| e.into_inner());
                (
                    cfg.port,
                    cfg.capture_fps,
                    cfg.target_width,
                    cfg.target_height,
                    cfg.capture_quality,
                )
            };

            let (frame_tx, keep_alive_rx) = broadcast::channel::<Vec<u8>>(8);
            let tx = frame_tx.clone();

            // Screen capture in a dedicated OS thread (scrap types are !Send)
            std::thread::spawn(move || {
                let capture_method = CaptureMethod::from_env();
                tracing::info!("Capture method: {:?}", capture_method);

                let mut cap = match ScreenCapture::new(capture_method) {
                    Ok(c) => c,
                    Err(e) => {
                        eprintln!("Screen capture init failed: {e}");
                        return;
                    }
                };
                tracing::info!("Active capture backend: {}", cap.backend_label());

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
            let http_client = reqwest::Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .expect("Failed to create HTTP client");

            let state = Arc::new(AppState {
                config,
                settings_version,
                runtime_env,
                frame_tx,
                auth: AuthState::new(trusted_store),
                http_client,
                analysis_gate: Semaphore::new(1),
                connected_count: connected_count_clone,
                _keep_alive_rx: keep_alive_rx,
            });

            // Drop expired tokens and stale throttle entries periodically so
            // neither map grows without bound on a long-running server.
            let sweeper_state = state.clone();
            tokio::spawn(async move {
                let mut ticker = tokio::time::interval(Duration::from_secs(300));
                loop {
                    ticker.tick().await;
                    sweeper_state.auth.sweep().await;
                }
            });

            // No CORS layer: the only clients are the native apps, so there is
            // no reason to let a web page in a browser reach these endpoints.
            let app = Router::new()
                .route("/api/connect", post(ai_server::handle_connect))
                .route("/api/disconnect", post(ai_server::handle_disconnect))
                .route("/api/system-info", get(ai_server::handle_system_info))
                .route("/api/models", get(ai_server::handle_models))
                .route(
                    "/api/settings",
                    get(ai_server::handle_get_settings).put(ai_server::handle_put_settings),
                )
                .route("/api/analyse", post(ai_server::handle_analyse))
                .route("/api/type", post(ai_server::handle_type))
                .route("/ws/stream", get(stream_server::ws_handler))
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

            // Publish the fingerprint so the QR code can carry it and the app
            // can pin against this exact certificate.
            if let Ok(mut slot) = cert_fingerprint.write() {
                *slot = Some(tls_cert.fingerprint.clone());
            }
            tracing::info!("TLS certificate fingerprint: {}", tls_cert.fingerprint);

            let rustls_config = axum_server::tls_rustls::RustlsConfig::from_pem(
                tls_cert.cert_pem,
                tls_cert.key_pem,
            )
            .await
            .expect("Failed to create TLS config");

            // `with_connect_info` gives handlers the peer address, which the
            // PIN throttle needs to rate-limit per client.
            axum_server::from_tcp_rustls(listener.into_std().unwrap(), rustls_config)
                .handle(handle_for_server)
                .serve(app.into_make_service_with_connect_info::<SocketAddr>())
                .await
                .ok();

            println!("  Server stopped.");
        });
    });

    handle
}


