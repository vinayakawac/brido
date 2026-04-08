# Brido Server

Rust desktop server for Brido. It captures the laptop screen, streams JPEG frames over WSS, and handles AI analysis requests through configured provider APIs.

---

## Overview

Server responsibilities:

1. Screen capture and JPEG encoding.
2. HTTPS/WSS API hosting on port `8080`.
3. Session authentication via PIN and bearer token.
4. AI analysis using configured providers.
5. Native egui GUI with system tray support.

---

## AI Provider Configuration

On startup, the server loads `.env.local` from the same folder as `brido-server.exe`.

If that folder is not writable, it falls back to `%APPDATA%/Brido/.env.local`.

Precedence rules:

1. `.env.local` is always authoritative.
2. If both `.env` and `.env.local` exist, `.env.local` wins.
3. Legacy `.env` values are migrated once when `.env.local` is first created.

At least one provider API key must be set.

On first run with no provider key, the GUI opens a **Configure AI** prompt.
Choose provider + API key there, and Brido writes it only to `.env.local`.

After save, Brido shows a restart-required message so changes can be reloaded safely.

### Example `.env.local` (OpenRouter)

```dotenv
# do not commit this file
OPENROUTER_API_KEY=<YOUR_API_KEY>
OPENROUTER_BASE_URL=https://openrouter.ai/api/v1
OPENROUTER_MODEL=openrouter/free
```

You can also start from `brido_server/.env.local.template`.

### Supported provider environment variables

| Provider | API key | Base URL variable | Model variable | Default model |
|----------|---------|-------------------|----------------|---------------|
| OpenAI | `OPENAI_API_KEY` | `OPENAI_BASE_URL` | `OPENAI_MODEL` | `gpt-4.1-mini` |
| Anthropic | `ANTHROPIC_API_KEY` | `ANTHROPIC_BASE_URL` | `ANTHROPIC_MODEL` | `claude-3-5-sonnet-latest` |
| Gemini | `GEMINI_API_KEY` | `GEMINI_BASE_URL` | `GEMINI_MODEL` | `gemini-2.0-flash` |
| OpenRouter | `OPENROUTER_API_KEY` | `OPENROUTER_BASE_URL` | `OPENROUTER_MODEL` | `google/gemini-2.5-flash` |

Provider selection priority in code is:

1. OpenAI
2. Anthropic
3. Gemini
4. OpenRouter

If a model hint is supplied in request payload, matching provider is prioritized for that request.

---

## Build and Run

```bash
cd brido_server
cargo run --release
```

Release build binary:

```bash
cargo build --release
.\target\release\brido-server.exe
```

The app is built with `windows_subsystem = "windows"`, so it launches as a desktop app without a terminal window.

---

## System Tray and Window Behavior

While the process is running:

- The tray icon is always present.
- GUI `minimize` hides the window to tray.
- OS close button (`X`) hides to tray instead of exiting.
- Tray menu has exactly two actions: `Open` and `Quit`.
- Double-clicking tray icon restores and focuses the window.
- `Quit` triggers the same clean shutdown path as GUI shutdown.

---

## API Endpoints

Base URL: `https://<SERVER_IP>:8080`

| Method | Path | Auth required | Description |
|--------|------|---------------|-------------|
| `POST` | `/api/connect` | No | Validate PIN, return token and system info |
| `GET` | `/api/qr-info` | No | Return IP, port, and PIN for QR flow |
| `GET` | `/api/system-info` | Yes | Return system information |
| `GET` | `/api/models` | Yes | Return configured/available AI model entries |
| `POST` | `/api/analyse` | Yes | Analyze a frame and return result with `model_used` |
| `GET` | `/ws/stream?token=<token>` | Yes (query token) | Stream binary JPEG frames over WSS |

---

## Project Structure

| File | Purpose |
|------|---------|
| `main.rs` | Entry point, server lifecycle, GUI launch |
| `config.rs` | Runtime configuration and environment variables |
| `capture.rs` | Screen capture |
| `encoder.rs` | Frame resize and JPEG encoding |
| `ai_server.rs` | REST handlers |
| `stream_server.rs` | WebSocket stream handling |
| `model_manager.rs` | Provider client calls and failover logic |
| `tls.rs` | Self-signed cert generation |
| `tray.rs` | Tray icon creation, menu wiring, tray event routing |
| `ui/window.rs` | Main GUI, controls, close/minimize tray behavior |
| `ui/controls.rs` | `ControlAction` enum |
| `ui/header.rs` | Header animation |
| `ui/qr_panel.rs` | QR texture rendering |

---

## Security Notes

- Server traffic is HTTPS/WSS with a self-signed certificate generated on startup.
- Session tokens are UUID-based and kept in memory.
- PIN is regenerated on restart.
- Provider API keys are read from environment variables.
- Keep `.env.local` out of source control.

---

## Troubleshooting

| Problem | Check |
|---------|-------|
| App cannot connect | Same Wi-Fi, firewall rules, server is running |
| `401 Unauthorized` | PIN mismatch or missing/invalid token |
| Analyse fails | Provider API key missing or provider endpoint error |
| No model list | No provider keys configured |
| Window not visible | Use tray menu `Open` or double-click tray icon |
