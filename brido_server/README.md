# Brido Server

> Rust-based laptop server for the Brido system. Captures the screen, streams JPEG frames over WSS, and runs on-demand AI analysis through a local Ollama instance. Ships with a native egui GUI — no terminal window.

---

## Table of Contents

- [Overview](#overview)
- [Architecture](#architecture)
- [Modules](#modules)
- [Prerequisites](#prerequisites)
- [Build & Run](#build--run)
- [Configuration](#configuration)
- [API Reference](#api-reference)
- [AI Pipeline](#ai-pipeline)
- [TLS & Security](#tls--security)
- [GUI Window](#gui-window)
- [Dependencies](#dependencies)

---

## Overview

The Brido server runs on Windows and handles:

1. **Screen capture** — continuous DXGI capture at 15 fps via the `scrap` crate
2. **Frame encoding** — resize to 1280×720 (Nearest filter), JPEG compress at quality 65
3. **HTTPS / WSS server** — TLS-secured axum server on port 8080
4. **AI analysis** — sends base64 JPEG frames to a local Ollama instance and returns structured results
5. **PIN authentication** — 6-digit random PIN generated on every start/restart
6. **egui GUI** — native window showing QR code, connection details, status, and controls

---

## Architecture

```
main.rs
  │
  ├── OS thread: screen capture (scrap, !Send)
  │     ├── ScreenCapture::frame()  →  raw BGRA Vec<u8>
  │     └── FrameEncoder::encode()  →  JPEG Vec<u8>  →  broadcast::Sender
  │
  ├── tokio runtime (inside OS thread)
  │     ├── axum HTTPS router :8080  (self-signed TLS via rcgen)
  │     │     ├── POST /api/connect
  │     │     ├── GET  /api/qr-info
  │     │     ├── GET  /api/system-info
  │     │     ├── GET  /api/models
  │     │     ├── POST /api/analyse
  │     │     └── WS   /ws/stream?token=<token>
  │     └── axum_server::Handle  →  stored for graceful restart
  │
  └── eframe::run_native()  →  egui event loop
        └── BridoApp::update()  →  renders GUI every frame
```

The capture loop runs in a dedicated `std::thread` because `scrap` types are `!Send`. A `_keep_alive_rx` broadcast receiver is held in `AppState` so the capture thread stays alive even with no WebSocket clients connected.

---

## Modules

| File | Responsibility |
|------|---------------|
| `main.rs` | Entry point — router, capture thread, server lifecycle, egui launch |
| `config.rs` | `Config` struct — port, PIN, fps, quality, Ollama URL, model names |
| `capture.rs` | `ScreenCapture` — DXGI screen capture via `scrap` |
| `encoder.rs` | `FrameEncoder` — resize to 720p (Nearest), JPEG encode with `image` crate |
| `stream_server.rs` | WebSocket handler — subscribes to broadcast, sends binary JPEG frames |
| `ai_server.rs` | REST handlers: connect, qr-info, system-info, models, analyse |
| `model_manager.rs` | Ollama HTTP client — universal system-prompt approach, try-fallback chain |
| `tls.rs` | Self-signed certificate + key generation via `rcgen` (IP SAN included) |
| `ui/window.rs` | egui main window — status indicators, phone connection count, controls |
| `ui/header.rs` | Typing animation for the "brido" title |
| `ui/controls.rs` | `ControlAction` enum (Restart, StopServer, Minimize, Shutdown) |
| `ui/qr_panel.rs` | QR code texture generation and rendering in egui |

---

## Prerequisites

| Tool | Version | Purpose | Install |
|------|---------|---------|---------|
| Rust | stable | Build toolchain | https://rustup.rs |
| Ollama | latest | Local AI inference | https://ollama.com |

### Pull required Ollama models

```bash
ollama pull qwen3-vl:8b     # Primary vision model  (6.19 GB)
ollama pull gemma3:4b        # Fallback vision model (3.34 GB)
```

Ollama must be running before the server starts (`ollama serve` or via the Ollama app). The server calls `http://localhost:11434`.

---

## Build & Run

```bash
cd brido_server

# Development build (console visible)
cargo run

# Release build (optimised, no console window)
cargo build --release
.\target\release\brido-server.exe
```

> `#![windows_subsystem = "windows"]` is set — the binary opens the egui GUI window directly with no terminal.

On first launch the server will:
1. Generate a self-signed TLS certificate (stored in memory, not on disk)
2. Generate a 6-digit PIN
3. Start the HTTPS + WSS server on port `8080`
4. Open the GUI window

---

## Configuration

Defaults are in `src/config.rs`:

| Field | Default | Description |
|-------|---------|-------------|
| `port` | `8080` | HTTPS / WSS listening port |
| `capture_fps` | `15` | Screen capture target FPS |
| `capture_quality` | `65` | JPEG quality for stream frames (0–100) |
| `target_width` | `1280` | Resize target width in pixels |
| `target_height` | `720` | Resize target height in pixels |
| `ollama_url` | `http://localhost:11434` | Ollama base URL |
| `default_vision_model` | `gemma3:4b` | Vision model name |

---

## API Reference

**Base URL:** `https://<SERVER_IP>:8080`  
**Auth header:** `Authorization: Bearer <token>` (obtain from `/api/connect`)

### POST `/api/connect`
Authenticate with PIN, receive session token and hardware info.
```json
// Request
{ "pin": "482901" }

// Response 200
{
  "token": "abc123...",
  "systemInfo": {
    "ram": "15.3 GB",
    "processor": "AMD Ryzen 7 7840HS",
    "processorSpeed": "3.80 GHz",
    "gpu": "GPU info unavailable",
    "storage": "952 GB",
    "storageUsed": "674 of 952 GB used"
  }
}
```

### GET `/api/qr-info`
Returns QR pairing data (no auth required).
```json
{ "ip": "192.168.0.5", "port": 8080, "pin": "482901" }
```
QR value encoded as: `brido://IP:PORT:PIN`

### GET `/api/system-info` *(auth required)*
Returns live hardware info (same shape as `systemInfo` above).

### GET `/api/models` *(auth required)*
Returns list of Ollama models available locally.
```json
{ "models": ["qwen3-vl:8b", "gemma3:4b"] }
```

### POST `/api/analyse` *(auth required)*
Send a frame for AI analysis.
```json
// Request
{ "imageBase64": "<base64 JPEG>", "model": "qwen3-vl:8b" }

// Response 200
{ "result": "Answer: B. Stack\nExplanation: ...", "modelUsed": "qwen3-vl:8b", "processingTimeMs": 4200 }
```

### WS `/ws/stream?token=<token>`
WebSocket upgrade — server pushes binary JPEG frames at ~15 fps over WSS.

---

## AI Pipeline

`model_manager.rs` uses a **single universal system prompt** approach — no classification stage, no routing, no post-processing:

```
analyse_image(base64, model, prompt)
  │
  ├── try qwen3-vl:8b
  │     └── run_vision(image, system_prompt + user_msg) → Ollama /api/chat
  │           OK, non-empty → strip_think_tags() → return "[qwen3-vl:8b]\n<result>"
  │
  └── fallback: gemma3:4b
        └── same pipeline

Ollama options: num_predict=512, num_ctx=4096, temperature=0.1, keep_alive=5m
```

**System prompt rules** (built into the model at request time):
- **Quiz / MCQ** → `Answer: B. Stack` + one-line explanation
- **Coding problem** → solution in a fenced code block + brief approach note
- **Math problem** → `Answer: 42` + numbered steps
- **Anything else** → 2–3 sentence description

Images are sent as base64 JPEG at 1024 px max width, quality 80 (resized in the Android app before upload).

---

## TLS & Security

- Self-signed certificate generated at startup via `rcgen` — includes the server's LAN IP as a Subject Alternative Name (IP SAN)
- Certificate and private key are held in memory only — never written to disk
- Android client uses a trust-all `X509TrustManager` so it accepts the cert without a CA chain
- Session tokens are UUID v4, stored in an `Arc<RwLock<HashSet<String>>>` in memory
- Tokens are invalidated on server restart (new PIN + new token set)
- No tokens or PINs are logged to disk

---

## GUI Window

The egui window (`ui/window.rs`) shows:

| Section | Content |
|---------|---------|
| Header | Animated "brido" title |
| Status | Server state (Starting / Running), phone connection count |
| QR Code | Rendered QR for `brido://IP:PORT:PIN` |
| Connection Info | IP address, port, PIN (copyable via arboard) |
| Controls | Restart, Stop Server, Minimise, Shutdown |

The window icon is embedded from `brido.png` at compile time via `include_bytes!`.

---

## Dependencies

| Crate | Version | Purpose |
|-------|---------|---------|
| `tokio` | 1 (full) | Async runtime |
| `axum` | 0.7 | HTTP + WebSocket router |
| `axum-server` | 0.7 (tls-rustls) | TLS-enabled listener |
| `tower-http` | 0.6 | CORS middleware |
| `rcgen` | 0.13 | Self-signed TLS cert generation |
| `scrap` | 0.5 | DXGI screen capture |
| `image` | 0.25 | Frame resize + JPEG encode |
| `reqwest` | 0.12 | Ollama HTTP client |
| `sysinfo` | 0.32 | CPU / RAM / storage info |
| `eframe` / `egui` | 0.31 | Native GUI window |
| `qrcode` | 0.14 | QR code generation |
| `arboard` | 3 | Clipboard access |
| `serde` / `serde_json` | 1 | JSON serialisation |
| `anyhow` | 1 | Error handling |
| `tracing` | 0.1 | Structured logging |
| `local-ip-address` | 0.6 | LAN IP detection |
| `base64` | 0.22 | Base64 encode/decode |
| `uuid` | 1 (v4) | Session token generation |
