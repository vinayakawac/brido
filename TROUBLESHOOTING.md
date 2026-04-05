# Troubleshooting

## Server startup and configuration

### `cargo build --release` fails

- Run `rustup update` and retry.
- If Windows linker errors appear, install [MSVC Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/).

### Server starts but `IP Address` shows `unknown`

No active local network interface was detected.

- Connect laptop to Wi-Fi or Ethernet.
- Restart the server.

### `Screen capture init failed`

`scrap` requires an active display session.

- Avoid headless sessions.
- Avoid unsupported RDP scenarios.
- Run the server in a local desktop session.

### Port 8080 already in use

The server retries binding, but if a process still owns the port:

```powershell
Get-NetTCPConnection -LocalPort 8080 -ErrorAction SilentlyContinue |
  ForEach-Object { Stop-Process -Id $_.OwningProcess -Force }
```

Then restart from the GUI.

### `.env.local` was not created where expected

Current behavior:

- Primary path: same folder as `brido-server.exe`.
- Fallback path: `%APPDATA%/Brido/.env.local` if install folder is not writable.

If you launch from a protected folder (for example Program Files), check the fallback path.

### Configure AI dialog does not appear on launch

The dialog auto-opens only when no provider key exists.

- Open it manually with **configure ai** button in the server UI.
- If keys already exist in `.env.local`, auto-prompt is intentionally skipped.

### Saving API key fails with permission error

- Make sure the app can write to the active env location.
- If exe folder is read-only, use fallback location in `%APPDATA%/Brido/.env.local`.
- Do not run directly from a read-only extracted archive path.

### Analysis says no provider configured

Ensure at least one key is present in `.env.local`:

- `OPENAI_API_KEY`
- `ANTHROPIC_API_KEY`
- `GEMINI_API_KEY`
- `OPENROUTER_API_KEY`

After saving key in UI, restart is required before new key is used.

### Both `.env` and `.env.local` exist and behavior is confusing

Precedence is fixed:

- `.env.local` wins.
- Legacy `.env` is only a migration source when `.env.local` does not exist yet.

To avoid confusion, keep only `.env.local` for active settings.

### Server stays on `starting...`

If status does not move to running:

- Restart once from UI.
- Verify port availability.
- Rebuild latest release binary.

```powershell
cd brido_server
cargo build --release
```

## Android app connectivity

### QR tab crashes or camera does not open

Camera permission is likely missing.

- Open Android settings for app permissions.
- Enable Camera.
- Reopen app.

### QR scan does not detect

- Ensure server QR is visible and stable.
- Confirm QR content format is `brido://IP:PORT:PIN`.
- Use better lighting and hold phone steady.

### `Invalid PIN` on connect

PIN changes after server restart. Re-scan QR or re-enter current PIN from server UI.

### Connected but stream shows waiting for frames

- Confirm server UI says running.
- Check firewall rule for TCP 8080.

```powershell
New-NetFirewallRule -DisplayName "Brido" -Direction Inbound -Protocol TCP -LocalPort 8080 -Action Allow
```

- Ensure phone and laptop are on same LAN.

### `Cannot reach server` in manual entry

- Verify phone and laptop are on same Wi-Fi.
- Verify IP and PIN from server UI.
- Test endpoint in browser: `https://<IP>:8080/api/qr-info` and accept self-signed warning.

### Stream lag or freezes

- Prefer 5 GHz Wi-Fi.
- Reduce system load on laptop.
- Lower capture settings if needed.

## GitHub Releases and workflow

### Workflow error: `Unrecognized named-value: 'secrets'`

This is caused by using `secrets.*` directly in `if:` expressions.

Current workflow fix:

- Workflow no longer uses `secrets.*` in `if:` expressions.
- Secrets are validated inside a shell step before decoding keystore and building release APK.

If the error still appears in GitHub UI, ensure your branch includes latest `.github/workflows/release.yml` and rerun.

### Release workflow fails: Android signing secrets missing

Tagged releases require a signed Android release APK. If any signing secret is missing, the workflow fails before publish.

Required repository secrets:

- `ANDROID_KEYSTORE_BASE64`
- `ANDROID_KEYSTORE_PASSWORD`
- `ANDROID_KEY_ALIAS`
- `ANDROID_KEY_PASSWORD`

After setting secrets, rerun the release by pushing a new tag.

### Release workflow did not trigger

Workflow trigger is tag push with `v*` pattern.

```powershell
git tag v1.0.0
git push origin v1.0.0
```

### Server artifacts missing in release

Server packaging depends on:

- `scripts/package_server_release.ps1`
- `brido_server/.env.local.template`
- release build output at `brido_server/target/release/brido-server.exe`

Expected server assets in GitHub Release:

- `brido-server-<tag>.exe` (standalone executable)
- `brido-server-<tag>-bundle.zip` (exe + template + README)
- `brido-server-<tag>.sha256`

If only ZIP appears, check `build-server` job logs for `Verify server artifacts` failure and confirm the EXE file exists before upload.

## Local build and packaging

### Build standalone EXE locally

```powershell
cd brido_server
cargo build --release
```

Output:

- `brido_server/target/release/brido-server.exe`

### Build APK locally

Debug APK:

```powershell
cd brido_app
.\gradlew.bat :app:assembleDebug
```

Output:

- `brido_app/app/build/outputs/apk/debug/app-debug.apk`

Release APK (requires signing):

```powershell
cd brido_app
.\gradlew.bat :app:assembleRelease
```

Output:

- `brido_app/app/build/outputs/apk/release/app-release.apk`

### Packaging script fails locally

From repo root, run:

```powershell
.\scripts\package_server_release.ps1 -Tag v0.1.0-local -OutputDir release_assets/server
```

If script complains about missing files, ensure server release build and template file exist.

### Debug APK build warnings about native symbols

`stripDebugDebugSymbols` warnings for some libraries are usually non-fatal. If build ends with `BUILD SUCCESSFUL`, artifact is valid.

## Security and secrets

### API key accidentally committed

1. Rotate the exposed key immediately.
2. Remove secret from repository history if needed.
3. Keep runtime keys only in `.env.local`.

Ignore rules should include:

- `brido_server/.env`
- `brido_server/.env.local`
- `brido_server/.env.local.*`
- keep `brido_server/.env.local.template` tracked

### Why HTTPS with self-signed cert?

Traffic is encrypted over LAN with server-generated TLS cert. Android client accepts the self-signed cert for local usage.
