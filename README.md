# Brido — Laptop Screen Stream + On-Demand AI Analysis

Stream your laptop screen to an Android phone and analyse visible content (code, quizzes, text) with local AI models — only when you press the **Analyse** button.

---

## Architecture

```
┌──────────────┐        WebSocket (JPEG)       ┌──────────────────┐
│  Laptop      │ ────────────────────────────▶  │  Android Phone   │
│  (Rust)      │                                │  (Kotlin)        │
│              │  ◀── POST /api/analyse ──────  │                  │
│  Screen Cap  │        (JPEG frame)            │  Stream Viewer   │
│  AI (Ollama) │  ──── JSON response ────────▶  │  Terminal Panel  │
└──────────────┘                                └──────────────────┘
```

**Phone is lightweight (< 15% CPU).** All heavy work (capture, encoding, OCR, AI) runs on the laptop.

---

## Prerequisites

### Laptop

| Tool      | Purpose            | Install                            |
| --------- | ------------------ | ---------------------------------- |
| Rust      | Build server       | https://rustup.rs                  |
| Ollama    | Run local AI       | https://ollama.com                 |

### Android

- Android Studio (Ladybug+)
- Physical device or emulator on the **same Wi-Fi network**

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

On startup the server prints:

```
╔═══════════════════════════════════════╗
║           Brido Server                ║
╠═══════════════════════════════════════╣
║  IP Address : 192.168.0.6            ║
║  Port       : 8080                   ║
║  PIN        : 482901                 ║
╚═══════════════════════════════════════╝
```

Note the **IP Address** and **PIN**.

### 3. Build and run the Android app

Open `brido_app/` in Android Studio, sync Gradle, then run on your device.

1. **Welcome screen** — tap or wait 2.5 s.
2. **Connection screen — Manual Entry** tab:
   - Enter the server IP address
   - Enter the 6-digit PIN
   - Tap **Connect**
3. Hardware info cards confirm the correct machine.
4. You are taken to the **Stream screen** automatically.

### 4. Analyse content

1. Navigate to the content you want analysed on the laptop.
2. Tap **Analyse** on the phone.
3. The current frame is captured and sent to the server.
4. The server runs the AI model and returns the result.
5. The result appears in the **terminal panel**.

---

## Server API Reference

| Method | Endpoint           | Auth            | Body                                    | Description                       |
| ------ | ------------------ | --------------- | --------------------------------------- | --------------------------------- |
| POST   | `/api/connect`     | —               | `{ "pin": "123456" }`                   | Authenticate, get token + HW info |
| GET    | `/api/system-info` | Bearer token    | —                                       | Laptop hardware information       |
| GET    | `/api/models`      | Bearer token    | —                                       | List supported AI models          |
| POST   | `/api/analyse`     | Bearer token    | `{ "image_base64": "...", "model": "qwen3-vl:8b" }` | Send frame for AI analysis |
| GET    | `/ws/stream`       | `?token=<uuid>` | WebSocket upgrade → binary JPEG frames  | Live screen stream                |

---

## Supported AI Models

| Model                                | File                                   | Capability        | Size     |
| ------------------------------------ | -------------------------------------- | ----------------- | -------- |
| qwen/qwen3-vl-8b                    | qwen3-VL-8B-Instruct-Q4_K_M.gguf      | vision, tool use  | 6.19 GB  |
| google/gemma-3-4b                    | gemma-3-4b-it-Q4_K_M.gguf             | vision            | 3.34 GB  |
| deepseek/deepseek-r1-0528-qwen3-8b  | deepseek-r1-0528-qwen3-8b-Q4_K_M.gguf | reasoning         | 5.03 GB  |

### Model selection strategy

- **Qwen3-VL-8B** — default for screen understanding, image reasoning, UI content
- **Gemma-3-4B** — fast, lightweight general explanations
- **DeepSeek-R1-Qwen3-8B** — complex reasoning, coding problems, algorithms

---

## Project Structure

```
brido/
├── brido_server/           # Rust laptop agent
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs         # Entry point, router, capture loop
│       ├── config.rs       # Server configuration, PIN generation
│       ├── capture.rs      # Screen capture (scrap / DXGI)
│       ├── encoder.rs      # JPEG frame encoding
│       ├── stream_server.rs# WebSocket frame streaming
│       ├── ai_server.rs    # REST API handlers
│       └── model_manager.rs# Ollama API integration
│
├── brido_app/              # Android application
│   └── app/src/main/java/com/example/brido/
│       ├── MainActivity.kt
│       ├── models/         # API data classes
│       ├── network/        # Retrofit + OkHttp
│       ├── stream/         # WebSocket stream manager
│       ├── screens/        # Compose UI screens
│       ├── viewmodel/      # App ViewModel
│       ├── navigation/     # Compose Navigation
│       └── ui/theme/       # Dark theme colors
│
└── models.md               # Reference AI model table
```

---

## Performance Targets

| Metric              | Target    |
| ------------------- | --------- |
| Stream latency      | < 300 ms  |
| AI response time    | < 2 s     |
| Phone CPU usage     | < 15%     |
| Phone memory        | < 300 MB  |
| Battery drain       | < 10%/hr  |
| Stream resolution   | 720p      |
| Stream FPS          | 20–30     |

---

## Troubleshooting

- **Cannot connect**: Ensure both devices are on the same Wi-Fi. Check firewall allows port 8080.
- **No stream frames**: Verify the server console shows "Capturing ..." log.
- **Analysis error**: Ensure Ollama is running (`ollama serve`) and the requested model is pulled.
- **PIN rejected**: The PIN changes each time the server starts. Re-check the server console.
