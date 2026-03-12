<div align="center">

<img src="brido.png" alt="Brido Logo" width="100"/>

# brido

**Stream your laptop screen to your phone.**  
**Tap once to get an AI answer — locally, instantly, privately.**

[![Rust](https://img.shields.io/badge/server-Rust-orange?style=flat-square&logo=rust)](brido_server/)
[![Android](https://img.shields.io/badge/app-Kotlin%20%2F%20Compose-green?style=flat-square&logo=android)](brido_app/)
[![License](https://img.shields.io/badge/license-MIT-blue?style=flat-square)](#)

</div>

---

## What it does

| | |
|--|--|
| 📡 | Streams your laptop screen to your Android phone over local Wi-Fi |
| 🔍 | Press **anAlyse** on the phone — the current frame goes to a local AI model |
| 🧠 | Returns quiz answers, code solutions, or descriptions — no cloud, no tracking |
| 🔒 | All traffic is TLS-encrypted on your LAN with a self-signed certificate |

---

## Quick Start

### 1 — Pull AI models

```bash
ollama pull qwen3-vl:8b
ollama pull gemma3:4b
```

### 2 — Start the server

```bash
cd brido_server
cargo run --release
```

A GUI window opens with a QR code, IP, and PIN.

### 3 — Connect the phone

Open the Android app → scan the QR code → stream starts automatically.

### 4 — Analyse

Navigate to whatever you want analysed on the laptop, then tap **anAlyse** on the phone.

---

## Architecture

```
Laptop (Rust)                          Android Phone (Kotlin)
┌──────────────┐   WSS (JPEG/TLS)     ┌──────────────────┐
│  Screen Cap  │ ───────────────────▶  │  Stream Viewer   │
│  egui GUI    │                       │  Terminal Panel   │
│  AI (Ollama) │ ◀── POST /analyse ── │  anAlyse button  │
│  TLS (rcgen) │ ─── JSON result ──▶  │                  │
└──────────────┘                       └──────────────────┘
```

All traffic is **HTTPS / WSS** — self-signed TLS cert generated at startup via `rcgen`.  
All AI inference is local via **Ollama**. No cloud. No data leaves your network.

---

## Components

| | Component | Stack |
|--|-----------|-------|
| 💻 | [`brido_server/`](brido_server/) | Rust · axum · egui · Ollama |
| 📱 | [`brido_app/`](brido_app/) | Kotlin · Jetpack Compose · OkHttp |

---

## AI Models

| Model | Use | Size |
|-------|-----|------|
| `qwen3-vl:8b` | Primary vision — quiz, code, UI | 6.19 GB |
| `gemma3:4b` | Fallback vision — fast, lightweight | 3.34 GB |

---

## Requirements

- **Laptop** — Windows, Rust toolchain, Ollama
- **Phone** — Android 7.0+ (API 24), same Wi-Fi network
- **Android Studio** — Ladybug / Meerkat or newer

---

## Troubleshooting

| Problem | Fix |
|---------|-----|
| Can't connect | Same Wi-Fi? Firewall allows port 8080? |
| PIN rejected | PIN resets on every server restart — check GUI |
| No AI result | Is `ollama serve` running? Model pulled? |
| No frames | Restart server from GUI, reconnect app |

---

<div align="center">

Made with Rust + Kotlin · Runs 100% locally

</div>
