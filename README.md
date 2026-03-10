# Brido — Laptop Screen Stream + On-Demand AI Analysis

Stream your laptop screen to an Android phone and analyse visible content (code, quizzes, text) with local AI models — only when you press the **Analyse** button.

---

## Architecture

```
Laptop (Rust)                          Android Phone (Kotlin)
┌──────────────┐   WSS (JPEG/TLS)     ┌──────────────────┐
│  Screen Cap  │ ───────────────────▶  │  Stream Viewer   │
│  egui GUI    │                       │  Terminal Panel   │
│  AI (Ollama) │ ◀── POST /analyse ── │  Analyse / DC    │
│  TLS (rcgen) │ ─── JSON result ──▶  │                  │
└──────────────┘                       └──────────────────┘
```

All communication uses **HTTPS / WSS** with a self-signed TLS certificate generated at startup via `rcgen`. The phone trusts all certificates for LAN connections.

**Phone is lightweight (< 15% CPU).** All heavy work (capture, encoding, AI) runs on the laptop.

---

## Prerequisites

### Laptop

| Tool   | Purpose          | Install                |
|--------|------------------|------------------------|
| Rust   | Build server     | https://rustup.rs      |
| Ollama | Run local AI     | https://ollama.com     |

### Android

- Android Studio (Ladybug+)
- Physical device on the **same Wi-Fi network**

---

## Setup

### 1. Pull AI models via Ollama

```bash
ollama pull qwen3-vl:8b          # Vision + tool use (6.19 GB)
ollama pull gemma3:4b             # Vision, lightweight (3.34 GB)
ollama pull deepseek-r1:8b        # Reasoning (5.03 GB)
```

> Models are GGUF Q4_K_M quantizations. See `models.md` for details.

### 2. Build and run the Rust server

```bash
cd brido_server
cargo build --release
cargo run --release
```

A **GUI window** opens showing the IP, PIN, QR code, and server status. No console window appears.

### 3. Build and run the Android app

Open `brido_app/` in Android Studio, sync Gradle, then run on your device.

1. **Welcome screen** — tap or wait 2.5 s.
2. **Connection screen** — scan the QR code from the server GUI, or switch to **Manual Entry** and type the IP + PIN.
3. Hardware info cards confirm the correct machine.
4. You are taken to the **Stream screen** automatically.

### 4. Analyse content

1. Navigate to the content you want analysed on the laptop.
2. Tap **anAlyse** on the phone.
3. The current frame is captured and sent to the server.
4. The server runs the AI model and returns the result.
5. The result appears in the **terminal panel**.

### 5. Disconnect

Tap **diSConnecT** on the stream screen to disconnect and return to the connection screen.

---

## Server GUI

The server launches an **egui** desktop window with:

- **Connection details** — IP address and PIN (click to copy)
- **QR code** — scan from the phone to auto-connect
- **Status indicators** — server running/starting/stopped + phone connected/disconnected
- **Control buttons** — restart server, stop server, minimize, shutdown

Restart generates a new PIN and TLS certificate, rebinds the port, and updates the QR code.

---

## Server API Reference

Base URL: `https://<SERVER_IP>:8080` (self-signed TLS)

| Method | Endpoint           | Auth            | Description                       |
|--------|--------------------|-----------------|-----------------------------------|
| POST   | `/api/connect`     | —               | Authenticate, get token + HW info |
| GET    | `/api/qr-info`     | —               | IP/port/PIN for QR generation     |
| GET    | `/api/system-info` | Bearer token    | Laptop hardware information       |
| GET    | `/api/models`      | Bearer token    | List supported AI models          |
| POST   | `/api/analyse`     | Bearer token    | Send frame for AI analysis        |
| GET    | `/ws/stream`       | `?token=<uuid>` | Live screen stream (WSS)          |

---

## Supported AI Models

| Model                                | Capability        | Size     |
|--------------------------------------|-------------------|----------|
| qwen/qwen3-vl-8b                    | vision, tool use  | 6.19 GB  |
| google/gemma-3-4b                    | vision            | 3.34 GB  |
| deepseek/deepseek-r1-0528-qwen3-8b  | reasoning         | 5.03 GB  |

### Model selection strategy

- **Qwen3-VL-8B** — default for screen understanding, image reasoning, UI content
- **Gemma-3-4B** — fast, lightweight general explanations
- **DeepSeek-R1-Qwen3-8B** — complex reasoning, coding problems, algorithms

---

## Project Structure

```
brido/
├── brido_server/              # Rust laptop server
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs            # Entry point, router, capture loop
│       ├── config.rs          # Configuration, PIN generation
│       ├── capture.rs         # Screen capture (scrap / DXGI)
│       ├── encoder.rs         # JPEG frame encoding + resize
│       ├── stream_server.rs   # WebSocket frame streaming
│       ├── ai_server.rs       # REST API handlers
│       ├── model_manager.rs   # Ollama API integration
│       ├── tls.rs             # Self-signed TLS cert (rcgen)
│       └── ui/
│           ├── window.rs      # egui main window + app state
│           ├── header.rs      # Typing animation header
│           ├── controls.rs    # Control button actions
│           └── qr_panel.rs    # QR code texture generation
│
├── brido_app/                 # Android application
│   └── app/src/main/java/com/example/brido/
│       ├── MainActivity.kt
│       ├── models/            # API data classes
│       ├── network/           # Retrofit + OkHttp (trust-all TLS)
│       ├── stream/            # WebSocket stream manager (WSS)
│       ├── screens/           # Compose UI screens
│       ├── viewmodel/         # App ViewModel
│       ├── navigation/        # Compose Navigation
│       └── ui/theme/          # Dark theme colors
│
└── models.md                  # Reference AI model table
```

---

## Performance Targets

| Metric            | Target    |
|-------------------|-----------|
| Stream latency    | < 300 ms  |
| AI response time  | < 2 s     |
| Phone CPU usage   | < 15%     |
| Phone memory      | < 300 MB  |
| Stream resolution | 720p      |
| Stream FPS        | 15        |

---

## Troubleshooting

- **Cannot connect**: Ensure both devices are on the same Wi-Fi. Check firewall allows port 8080.
- **No stream frames**: Restart the server from the GUI and reconnect.
- **Analysis error**: Ensure Ollama is running (`ollama serve`) and the requested model is pulled.
- **PIN rejected**: The PIN changes on every server start/restart. Check the server GUI.
