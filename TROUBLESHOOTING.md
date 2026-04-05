# Troubleshooting

## Server

### `cargo build` fails — "could not compile"

- Run `rustup update` to get the latest stable toolchain.
- If you see a linker error on Windows, install the [MSVC Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/).

### Server starts but shows `IP Address: unknown`

Your machine has no active local network interface. Connect to Wi-Fi or Ethernet and restart the server.

### `Screen capture init failed`

`scrap` requires a display to be attached. This will fail on headless / RDP sessions. Run the server from a physically connected or direct-display session.

### Port 8080 already in use

The server retries binding automatically on restart. If it persists:

```powershell
Get-NetTCPConnection -LocalPort 8080 -ErrorAction SilentlyContinue |
  ForEach-Object { Stop-Process -Id $_.OwningProcess -Force }
```

Then restart via the GUI or re-run the exe.

### Ollama not reachable / `error sending AI request`

1. Make sure Ollama is running: `ollama list`
2. Verify the URL in `brido_server/src/config.rs` matches your Ollama port (default `http://localhost:11434`).
3. Check the model name matches exactly what `ollama list` shows.

### AI analysis returns an empty result

The model may not be pulled yet:

```bash
ollama pull qwen3-vl:8b
ollama pull gemma3:4b
ollama pull deepseek-r1:8b
```

### Server status stays "starting" after restart

This was fixed — the GUI now monitors the `server_ready` flag and transitions to "running" automatically. If you built from an older version, rebuild:

```powershell
cd brido_server
cargo build --release
```

### GUI shows "phone disconnected" even when connected

The server tracks the connected phone count via a shared atomic counter. Make sure you are running the latest build. The counter increments when `POST /api/connect` succeeds.

---

## Android app

### App crashes on the Connection screen (QR tab)

Camera permission was likely denied. Go to **Settings > Apps > Brido > Permissions > Camera** and enable it, then reopen the app.

### QR scan does not detect the code

- The QR must encode the exact string `brido://IP:PORT:PIN` (no spaces, no extra characters).
- The server GUI displays a scannable QR code — point the phone camera at it.
- Ensure good lighting and hold the phone ~20-30 cm from the screen.

### "Invalid PIN" when connecting

The PIN changes every time the server starts or restarts. Check the current PIN in the server GUI and re-enter it (or scan the new QR code).

### App connects but stream shows "Waiting for frames..."

1. Confirm the server GUI shows "server running".
2. Check that port 8080 is not blocked by Windows Firewall:

```powershell
New-NetFirewallRule -DisplayName "Brido" -Direction Inbound -Protocol TCP -LocalPort 8080 -Action Allow
```

3. The stream uses WSS (secure WebSocket). Ensure no proxy or VPN is intercepting TLS traffic on the LAN.
4. Try restarting the server from the GUI and reconnecting.

> **Root cause (fixed):** An earlier version broke the stream by using `if tx.send(jpeg).is_err() { break; }` in the capture thread. Since the initial broadcast receiver was immediately dropped, `send()` returned `Err` before any WebSocket client connected, exiting the capture loop immediately. Fixed by storing a keep-alive receiver in `AppState._keep_alive_rx` so there is always at least one subscriber until the server shuts down.

### Stream is laggy or freezes

- Switch to a 5 GHz Wi-Fi band if possible.
- The default is 15 fps at quality 65. You can adjust in `brido_server/src/config.rs`.
- Close other CPU-heavy applications on the laptop.

### "Cannot reach server" on Manual Entry

- Confirm phone and laptop are on the **same** Wi-Fi network.
- Ping the laptop IP from a network tool app on the phone.
- Confirm the server GUI shows "server running".
- Try `https://<IP>:8080/api/qr-info` in a browser (accept the self-signed cert warning).

### UI is cut off by navigation bar or status bar

All screens use `WindowInsets.systemBars` to avoid both the top status bar and bottom navigation bar. If content is still clipped, ensure the app has `enableEdgeToEdge()` in `MainActivity.kt` (it does by default).

---

## Gradle / Android Studio

### Gradle sync fails after adding dependencies

1. **File > Sync Project with Gradle Files**.
2. If that fails, **File > Invalidate Caches > Invalidate and Restart**.
3. Check `brido_app/gradle/libs.versions.toml` — version strings must not contain spaces.

### `CameraX` or `ML Kit` classes not found

Ensure the dependencies exist in `brido_app/app/build.gradle.kts` and run a Gradle sync.

---

## General FAQ

### Can I change the port?

Edit `port` in `brido_server/src/config.rs` and rebuild.

### Can I run without Ollama?

Yes — the server, GUI, and stream work without Ollama. Only the **Analyse** feature requires Ollama to be running with at least one model pulled.

### Why self-signed TLS?

Android requires HTTPS for modern network security. The server generates a self-signed certificate at startup via `rcgen`, and the Android app trusts all certificates for LAN connections. This avoids the need for a real CA while keeping traffic encrypted.
