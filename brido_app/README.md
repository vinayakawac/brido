# Brido App

Android client application for Brido, built with Kotlin and Jetpack Compose.

It connects to the Brido server, displays a live screen stream, and sends the latest frame for AI analysis on demand.

---

## Overview

The app handles:

1. Pairing with server using QR or manual IP/PIN.
2. Receiving JPEG frames over secure WebSocket.
3. Displaying stream in real time.
4. Triggering analysis requests for the current frame.
5. Rendering server responses in the terminal panel.

---

## Screens

### WelcomeScreen

Splash screen shown at startup.

### ConnectionScreen

Two connection modes:

- QR Scanner tab (CameraX + ML Kit)
- Manual entry tab (IP + PIN)

On successful connect, token and system information are stored and stream starts.

### StreamScreen

Main screen with:

- Stream viewer panel.
- Terminal panel for analysis output and status lines.
- `anAlyse` and `diScoNnect` actions.

---

## Networking

Base URL pattern:

- `https://<SERVER_IP>:8080`
- `wss://<SERVER_IP>:8080/ws/stream?token=<token>`

API calls:

| Method | Path | Purpose |
|--------|------|---------|
| `POST` | `api/connect` | Validate PIN and return token/system info |
| `GET` | `api/system-info` | Fetch server hardware info |
| `GET` | `api/models` | Fetch model/provider entries from server |
| `POST` | `api/analyse` | Analyze current frame |

`RetrofitClient` config uses a trust-all TLS setup to accept server self-signed certs on local LAN.

---

## Analysis Flow

When `anAlyse` is pressed:

1. Take latest frame from stream manager.
2. Resize/compress frame to JPEG (first pass: width 1024, quality 80).
3. Send `/api/analyse` with base64 image.
4. If server returns 5xx, retry once with smaller image (width 768, quality 65).
5. Append server `result` text to terminal.

Returned `model_used` is supplied by server and may vary by provider availability and request routing.

---

## Project Structure

| Path | Purpose |
|------|---------|
| `MainActivity.kt` | App entry and navigation host setup |
| `navigation/BridoNavigation.kt` | Screen navigation graph |
| `screens/` | UI screens and scanner tab |
| `viewmodel/BridoViewModel.kt` | Connection, stream, analysis state and actions |
| `network/RetrofitClient.kt` | Retrofit + OkHttp setup |
| `network/BridoApiService.kt` | API interface |
| `stream/StreamManager.kt` | WSS frame stream handling |
| `models/ApiModels.kt` | API request/response models |

---

## Build and Run

1. Open `brido_app/` in Android Studio.
2. Run on a physical Android device on same network as server.

Gradle commands:

```bash
cd brido_app
./gradlew assembleDebug
./gradlew installDebug
./gradlew lint
```

## Release Artifacts

The GitHub release workflow publishes APK artifacts on `v*` tags:

- `brido-android-debug-<tag>.apk` is always generated.
- `brido-android-release-<tag>.apk` is required for tagged releases.

Signing secret names expected by CI:

- `ANDROID_KEYSTORE_BASE64`
- `ANDROID_KEYSTORE_PASSWORD`
- `ANDROID_KEY_ALIAS`
- `ANDROID_KEY_PASSWORD`

If any signing secret is missing, the release workflow fails before publishing assets.

---

## Requirements

- Android Studio (recent stable)
- Android device API 24+
- Brido server running on same local network

---

## Troubleshooting

| Problem | Check |
|---------|-------|
| Cannot connect | Verify IP/PIN and same Wi-Fi |
| Stream disconnects | Check server process and firewall |
| Analyse shows error | Verify server provider configuration/API key |
| QR does not scan | Improve lighting/focus or use manual connect |
