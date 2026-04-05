<div align="center">

<img src="brido.png" alt="Brido Logo" width="100"/>

# brido

Stream your laptop screen to your phone and run on-demand AI analysis.

[![Rust](https://img.shields.io/badge/server-Rust-orange?style=flat-square&logo=rust)](brido_server/)
[![Android](https://img.shields.io/badge/app-Kotlin%20%2F%20Compose-green?style=flat-square&logo=android)](brido_app/)
[![License](https://img.shields.io/badge/license-MIT-blue?style=flat-square)](#)

</div>

---

## Overview

Brido is a two-part system:

- A Windows desktop server written in Rust that captures the laptop screen and streams frames over HTTPS/WSS.
- An Android app written in Kotlin/Compose that receives the stream and requests AI analysis for the latest frame.
- AI analysis is provider-based and configured with environment variables (OpenAI, Anthropic, Gemini, or OpenRouter).

---

## Repository Layout

| Path | Description |
|------|-------------|
| `brido_server/` | Rust server, HTTPS/WSS APIs, desktop GUI, system tray integration |
| `brido_app/` | Android client app (Jetpack Compose) |
| `ARCHITECTURE.md` | Cross-project architecture notes |
| `API.md` | API notes and request/response reference |
| `SETUP.md` | Setup instructions |
| `TROUBLESHOOTING.md` | Common issues and fixes |

---

## Quick Start

### 1. Configure AI provider

Create `brido_server/.env.local` with at least one provider key.

```dotenv
# Example: OpenRouter
OPENROUTER_API_KEY=<YOUR_API_KEY>
OPENROUTER_BASE_URL=https://openrouter.ai/api/v1
OPENROUTER_MODEL=qwen/qwen3.6-plus:free
```

### 2. Start the server

```bash
cd brido_server
cargo run --release
```

The server GUI opens and shows QR code, IP, and PIN.

### 3. Run the Android app

Open `brido_app/` in Android Studio and run on a physical device connected to the same Wi-Fi.

### 4. Pair and analyse

- Scan QR (or enter IP and PIN manually) in the app.
- Press `anAlyse` on the phone to analyze the current frame.

---

## Desktop Tray Behavior

While the server is running:

- Clicking the GUI `minimize` button hides the window to system tray.
- Clicking the OS close button (`X`) also hides to tray instead of exiting.
- Tray menu contains `Open` and `Quit`.
- Double-clicking the tray icon restores the window.

---

## Documentation

- [Server README](brido_server/README.md)
- [Android App README](brido_app/README.md)
- [Architecture](ARCHITECTURE.md)
- [API](API.md)
- [Setup](SETUP.md)
- [Troubleshooting](TROUBLESHOOTING.md)

---

## Troubleshooting

| Problem | What to check |
|---------|---------------|
| App cannot connect | Laptop and phone are on same Wi-Fi, firewall allows server port (default 8080) |
| PIN rejected | PIN changes on server restart; use the PIN currently shown in GUI |
| Analyse fails | Verify at least one AI provider API key is configured in `.env.local` |
| Window seems gone | Check system tray and use `Open` |

---

<div align="center">

Built with Rust and Kotlin.

</div>
