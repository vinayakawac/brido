# Brido App

> Kotlin/Compose Android application for the Brido system. Receives a live JPEG stream from the laptop server over WSS, displays it full-screen, and sends frames to the server for on-demand AI analysis — all over a local Wi-Fi TLS connection.

---

## Table of Contents

- [Overview](#overview)
- [Screens](#screens)
- [Project Structure](#project-structure)
- [Navigation Flow](#navigation-flow)
- [Networking](#networking)
- [Frame Streaming](#frame-streaming)
- [AI Analysis](#ai-analysis)
- [Markdown Terminal](#markdown-terminal)
- [Prerequisites](#prerequisites)
- [Build & Run](#build--run)
- [Gradle & Dependencies](#gradle--dependencies)
- [Security Notes](#security-notes)

---

## Overview

The Android app is the **lightweight client** — all heavy work (screen capture, JPEG encoding, AI inference) stays on the laptop. The phone's role is:

1. **Pairing** — scan a QR code (or enter IP + PIN manually) to connect
2. **Streaming** — display the live JPEG stream from the laptop
3. **Analysis** — on button press, capture the current frame, upload it to the server, and show the AI result in a styled terminal panel

CPU usage on the phone stays below ~15%.

---

## Screens

### WelcomeScreen
Full-screen splash with the Brido logo. Auto-advances to Connection after 2.5 s, or immediately on tap.

### ConnectionScreen
Two-tab interface:

| Tab | Description |
|-----|-------------|
| **QR Scanner** | CameraX + ML Kit barcode scanner. Reads `brido://IP:PORT:PIN` format and auto-connects |
| **Manual Entry** | Text fields for IP address and PIN. Tap **Connect** to authenticate |

On successful connection the server returns hardware info (CPU, RAM, storage, GPU) which is shown in cards on the screen before navigating to the stream.

### StreamScreen
Main screen — split into two vertical panels:

| Panel | Weight | Content |
|-------|--------|---------|
| **Stream Viewer** | 0.45 | Live JPEG frames rendered as a `Bitmap` via `Image()` composable |
| **Terminal Panel** | 0.55 | Scrollable styled output — AI results, status messages, markdown rendered |

Action buttons:
- **anAlyse** — captures the latest frame, resizes to 1024 px width (JPEG quality 80), POSTs to `/api/analyse`, appends result to terminal
- **diScoNnect** — closes the WebSocket, clears state, navigates back to ConnectionScreen

---

## Project Structure

```
app/src/main/java/com/example/brido/
├── MainActivity.kt                  Entry point — sets up Compose + navigation
├── navigation/
│   └── BridoNavigation.kt           NavHost — welcome → connection → stream
├── screens/
│   ├── WelcomeScreen.kt             Animated splash screen
│   ├── ConnectionScreen.kt          QR + manual entry, hardware info cards
│   ├── StreamScreen.kt              Stream viewer + terminal panel + buttons
│   └── QrScannerTab.kt              CameraX + ML Kit barcode integration
├── viewmodel/
│   └── BridoViewModel.kt            Connection, streaming, and analysis state
├── network/
│   ├── BridoApiService.kt           Retrofit service interface
│   └── RetrofitClient.kt            OkHttp singleton with trust-all TLS
├── stream/
│   └── StreamManager.kt             OkHttp WSS client — delivers Bitmap frames
├── models/
│   └── ApiModels.kt                 Request/response data classes
└── ui/theme/
    ├── Color.kt                     Dark theme colour palette
    ├── Theme.kt                     MaterialTheme setup
    └── Type.kt                      Typography
```

---

## Navigation Flow

```
WelcomeScreen
    │  2.5 s or tap
    ▼
ConnectionScreen
    ├── Tab 0: QrScannerTab
    │       CameraX preview → ML Kit BarcodeScanning
    │       detect "brido://IP:PORT:PIN" → call connect()
    └── Tab 1: ManualEntryTab
            IP + PIN fields → tap Connect → call connect()
    │
    │  POST /api/connect → 200 OK + token
    ▼
StreamScreen
    ├── StreamManager connects WSS → frames → currentFrame
    ├── anAlyse → POST /api/analyse → terminal entry
    └── diScoNnect → viewModel.disconnect() → popBackStack()
```

The back arrow on ConnectionScreen also uses `popBackStack()` to go back to Welcome.

---

## Networking

All network operations use **HTTPS / WSS** on port `8080`. The server uses a self-signed TLS certificate, so the app uses a custom `X509TrustManager` that accepts all certificates — appropriate for a local LAN-only tool.

### RetrofitClient (`network/RetrofitClient.kt`)

Singleton that creates:
- `OkHttpClient` with a trust-all `SSLContext` and `HostnameVerifier`
- `Retrofit` instance pointed at `https://<SERVER_IP>:8080`
- Moshi JSON converter

### BridoApiService (`network/BridoApiService.kt`)

```kotlin
interface BridoApiService {
    @POST("/api/connect")       suspend fun connect(body: ConnectRequest): ConnectResponse
    @GET("/api/qr-info")        suspend fun getQrInfo(): QrInfoResponse
    @GET("/api/system-info")    suspend fun getSystemInfo(): SystemInfoResponse
    @GET("/api/models")         suspend fun getModels(): ModelsResponse
    @POST("/api/analyse")       suspend fun analyse(body: AnalyseRequest): AnalyseResponse
}
```

All calls are `suspend` functions called from the ViewModel's `viewModelScope`.

---

## Frame Streaming

`StreamManager` opens an OkHttp WebSocket to `wss://<IP>:8080/ws/stream?token=<token>`.

```
StreamManager.connect()
    └── OkHttp WebSocket listener
          onMessage(bytes: ByteString)
            → BitmapFactory.decodeByteArray()     // JPEG → Bitmap
            → latestFrame = bitmap                // stored for Analyse
            → onFrame(bitmap) callback
              → BridoViewModel.currentFrame       // StateFlow
                → StreamScreen recomposition → Image()
```

- Frames arrive at ~15 fps
- The latest frame is stored in `latestFrame` so `analyse()` always uses the most recent visible frame
- Connection errors and close events update `connectionState` in the ViewModel

---

## AI Analysis

When the user taps **anAlyse**:

```kotlin
// BridoViewModel.kt
fun analyse() {
    val bitmap = streamManager.latestFrame ?: return
    val scaled = Bitmap.createScaledBitmap(bitmap, 1024, scaledHeight, true)
    val jpeg = ByteArrayOutputStream()
    scaled.compress(Bitmap.CompressFormat.JPEG, 80, jpeg)
    val base64 = Base64.encodeToString(jpeg.toByteArray(), Base64.NO_WRAP)

    viewModelScope.launch {
        val response = api.analyse(AnalyseRequest(imageBase64 = base64, model = selectedModel))
        addTerminalEntry(response.result.trim())   // full block, no line-splitting
    }
}
```

Image sent: **1024 px max width**, **JPEG quality 80**.  
Model selected in ViewModel: `gemma3:4b` (overridden by server to `qwen3-vl:8b` if available).

---

## Markdown Terminal

`StreamScreen.kt` renders each terminal entry through a full **markdown parser** — `parseMarkdown(block)`.

### Supported syntax

| Syntax | Render |
|--------|--------|
| ```` ```lang … ``` ```` | Green text on dark `#1A1A1A` background, monospace |
| `## Heading` / `# H1` / `### H3` | Bold green, size scaled by level |
| `[model-name]` | Bold accent colour (model tag) |
| `---` | Horizontal rule line |
| `> blockquote` | `│ ` prefix in accent colour |
| `- item` / `* item` | `•` bullet in accent colour |
| `1. item` | Numbered list with bold accent number |
| `**bold**` | Bright white `#E0E0E0` |
| `*italic*` | Italic span |
| `***bold italic***` | Bold + italic combined |
| `` `inline code` `` | Green on dark `#2A2A2A` background |
| `~~strikethrough~~` | Strikethrough |

Compiled regex patterns process inline spans with `DOT_MATCHES_ALL` for robustness.

### Terminal entry types

| Prefix / Pattern | Colour | Meaning |
|-----------------|--------|---------|
| Starts with `>` | Green | Status / connection message |
| Matches `[...]` | Accent | Model tag from server response |
| Everything else | `parseMarkdown()` | AI result with full markdown |

---

## Prerequisites

| Tool | Version | Notes |
|------|---------|-------|
| Android Studio | Ladybug / Meerkat+ | Gradle sync, device run |
| JDK | 17+ | Bundled with recent Android Studio |
| Android device | API 24+ (Android 7.0+) | Physical device on same Wi-Fi |
| Brido server | running on laptop | Same local network |

> The app does **not** work on an emulator for streaming — screen capture and local network access need a real device.

---

## Build & Run

1. Open `brido_app/` in Android Studio (not the root folder)
2. Let Gradle sync complete
3. Ensure your Android device is connected via USB with USB debugging enabled
4. Tap **Run ▶** or press `Shift+F10`

### Gradle commands

```bash
# From brido_app/
./gradlew assembleDebug          # Build debug APK
./gradlew assembleRelease        # Build release APK (needs signing config)
./gradlew installDebug           # Build + install on connected device
./gradlew lint                   # Run lint checks
```

---

## Gradle & Dependencies

**`app/build.gradle.kts`:**

| Dependency | Version | Purpose |
|------------|---------|---------|
| Compose BOM | latest | Compose UI framework |
| Material 3 | latest | MD3 components and theming |
| `androidx.navigation:navigation-compose` | 2.8+ | Screen navigation |
| `retrofit2:retrofit` | 2.11.0 | HTTP client + JSON |
| `com.squareup.okhttp3:okhttp` | 4.12.0 | WebSocket + custom TLS |
| `androidx.camera:camera-*` | 1.4.1 | CameraX capture pipeline |
| `com.google.mlkit:barcode-scanning` | 17.3.0 | QR code decoding |
| `androidx.lifecycle:lifecycle-viewmodel-compose` | 2.9.0 | ViewModel integration |

**SDK targets:**

| Setting | Value |
|---------|-------|
| `compileSdk` | 36 |
| `targetSdk` | 36 |
| `minSdk` | 24 |
| `jvmTarget` | 11 |

---

## Security Notes

- **Trust-all TLS** — the app accepts any server certificate. This is intentional for a LAN-only tool with a self-signed cert. Do not use this pattern for internet-facing apps.
- **PIN required** — every connection requires the 6-digit PIN displayed on the server GUI. The PIN changes on every server restart.
- **Token in memory only** — the session token is never written to disk or SharedPreferences.
- **No analytics / telemetry** — the app makes no network requests outside the local Brido server.
