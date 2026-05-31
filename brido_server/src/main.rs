//! brido-overlay — Stealth desktop AI overlay.
//!
//! A standalone Windows binary that floats an always-on-top panel for
//! on-demand screen capture + AI analysis.  No HTTP server, no ports.
//!
//! Shares config, capture, encoder, and model_manager with brido-server
//! via the brido_server library crate.

#![windows_subsystem = "windows"]

mod ai_client;
mod capture_trigger;
mod hotkey;
mod stealth;
mod window;
mod server;
mod ai_server;
mod stream_server;
mod tls;
mod ui;

use std::path::PathBuf;

use brido_server::config;

fn load_icon() -> egui::IconData {
    let png_bytes = include_bytes!("../../brido.png");
    let img = image::load_from_memory(png_bytes)
        .expect("brido.png not found")
        .into_rgba8();
    let (width, height) = img.dimensions();
    egui::IconData {
        rgba: img.into_raw(),
        width,
        height,
    }
}

fn main() {
    tracing_subscriber::fmt::init();

    // ── Bootstrap env config (same logic as brido-server main.rs) ────
    let (runtime_env, runtime_loaded) = match config::bootstrap_runtime_env() {
        Ok(runtime) => (runtime, true),
        Err(err) => {
            tracing::error!("Failed to bootstrap env configuration: {}", err);
            let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            (
                config::RuntimeEnvPaths {
                    active_env_path: cwd.join(".env.local"),
                    primary_env_path: cwd.join(".env.local"),
                    fallback_env_path: cwd.join(".env.local"),
                    legacy_env_path: cwd.join(".env"),
                    is_using_fallback: false,
                    migrated_legacy_env: false,
                },
                false,
            )
        }
    };

    if !runtime_loaded {
        if let Err(err) = config::load_runtime_env(&runtime_env) {
            tracing::error!("Failed to load runtime env: {}", err);
        }
    }

    let cfg = config::Config::default();

    if !cfg.has_any_provider_key() {
        eprintln!("╔═══════════════════════════════════════════╗");
        eprintln!("║  No AI provider key configured.           ║");
        eprintln!("║  Set one of:                              ║");
        eprintln!("║    OPENAI_API_KEY                         ║");
        eprintln!("║    ANTHROPIC_API_KEY                      ║");
        eprintln!("║    GEMINI_API_KEY                         ║");
        eprintln!("║    OPENROUTER_API_KEY                     ║");
        eprintln!("║  in .env.local next to this executable.   ║");
        eprintln!("╚═══════════════════════════════════════════╝");
    }

    println!();
    println!("╔═══════════════════════════════════════╗");
    println!("║        Brido (Unified)                ║");
    println!("╠═══════════════════════════════════════╣");
    println!("║  Hotkeys configured in settings       ║");
    println!("╚═══════════════════════════════════════╝");
    println!();

    // ── Tokio runtime for async AI calls ─────────────────────────────
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .worker_threads(2)
        .build()
        .expect("Failed to create tokio runtime");

    // ── Hotkey listener ──────────────────────────────────────────────
    let (hotkey_tx, hotkey_rx) = std::sync::mpsc::channel();
    let (_jh, hotkey_handle) = hotkey::start_hotkey_listener(
        hotkey_tx.clone(),
        &cfg.overlay_hotkey_capture,
        &cfg.overlay_hotkey_toggle,
        &cfg.overlay_hotkey_settings,
    );

    // ── Background Server ────────────────────────────────────────────
    let server_ready = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let connected_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let _axum_handle = server::start_server(cfg.clone(), server_ready.clone(), connected_count.clone());

    let ip = local_ip_address::local_ip()
        .map(|ip| ip.to_string())
        .unwrap_or_else(|_| "unknown".to_string());
    let pin = cfg.pin.clone();
    let port = cfg.port;

    // ── Build the overlay app ────────────────────────────────────────
    let app = window::OverlayApp::new(
        hotkey_tx,
        hotkey_rx,
        hotkey_handle,
        rt.handle().clone(),
        cfg,
        runtime_env,
        ip,
        pin,
        port,
        server_ready,
        connected_count,
    );

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([380.0, 620.0])
            .with_min_inner_size([340.0, 400.0])
            .with_always_on_top()
            .with_decorations(false)
            .with_transparent(true)
            .with_taskbar(false)
            .with_title("Brido Overlay")
            .with_icon(std::sync::Arc::new(load_icon())),
        ..Default::default()
    };

    eframe::run_native(
        "Brido Overlay",
        native_options,
        Box::new(move |_cc| {
            // Apply stealth after the window is created.
            // eframe doesn't expose the raw HWND directly, so we find
            // our window by title using FindWindowW.
            apply_stealth_by_title();

            Ok(Box::new(app))
        }),
    )
    .ok();

    std::process::exit(0);
}

/// Find our overlay window by title and apply stealth.
fn apply_stealth_by_title() {
    use windows::core::PCWSTR;
    use windows::Win32::UI::WindowsAndMessaging::FindWindowW;

    unsafe {
        let title: Vec<u16> = "Brido Overlay\0".encode_utf16().collect();
        let result = FindWindowW(PCWSTR::null(), PCWSTR(title.as_ptr()));
        match result {
            Ok(hwnd) if !hwnd.is_invalid() => {
                stealth::apply_stealth(hwnd.0 as isize);
            }
            _ => {
                tracing::warn!(
                    "Could not find overlay window for stealth — will retry after delay"
                );
                // Retry after a short delay (window might not be fully created yet)
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_millis(500));
                    let title: Vec<u16> = "Brido Overlay\0".encode_utf16().collect();
                    let result = FindWindowW(PCWSTR::null(), PCWSTR(title.as_ptr()));
                    match result {
                        Ok(hwnd) if !hwnd.is_invalid() => {
                            stealth::apply_stealth(hwnd.0 as isize);
                        }
                        _ => {
                            tracing::error!("Stealth: could not find window after retry");
                        }
                    }
                });
            }
        }
    }
}
