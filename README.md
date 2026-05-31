<div align="center">

<img src="brido.png" alt="Brido Logo" width="100"/>

# brido

Stream your laptop screen to your phone and run on-demand AI analysis.

[![Rust](https://img.shields.io/badge/server-Rust-orange?style=flat-square&logo=rust)](brido_server/)
[![Android](https://img.shields.io/badge/app-Kotlin%20%2F%20Compose-green?style=flat-square&logo=android)](brido_app/)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue?style=flat-square)](#)

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

### 1. Start the server

```bash
cd brido_server
cargo run --release
```

The server GUI opens and shows QR code, IP, and PIN.
On first run, Brido automatically creates `.env.local` beside the running executable and opens a setup prompt to choose provider + API key.

If the executable directory is read-only (for example, Program Files), Brido falls back to `%APPDATA%/Brido/.env.local`.

### 2. Run the Android app

Open `brido_app/` in Android Studio and run on a physical device connected to the same Wi-Fi.

### 3. Pair and analyse

- Scan QR (or enter IP and PIN manually) in the app.
- Press `anAlyse` on the phone to analyze the current frame.

### 4. Release downloads

GitHub Releases publish these artifacts on every `v*` tag:

- `brido-server-<tag>.exe`
- `brido-server-<tag>-bundle.zip` (exe + `.env.local.template` + server README)
- `brido-android-debug-<tag>.apk`
- `brido-android-release-<tag>.apk` (required for tagged releases)

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
| Window seems gone | Press the toggle hotkey (same key used to hide) to bring it back; also check system tray and use `Open` |

---

<div align="center">

Built with Rust and Kotlin.

</div>
