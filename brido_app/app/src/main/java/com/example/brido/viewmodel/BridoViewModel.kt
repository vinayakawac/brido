package com.example.brido.viewmodel

import android.app.Application
import android.graphics.Bitmap
import android.util.Base64
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import com.example.brido.data.ConnectionStore
import com.example.brido.network.ServerTrust
import okhttp3.OkHttpClient
import com.example.brido.models.ApiError
import com.example.brido.models.AnalyseResponse
import com.example.brido.models.AnalyseRequest
import com.example.brido.models.ConnectRequest
import com.example.brido.models.ProviderOption
import com.example.brido.models.ServerInfo
import com.example.brido.models.SettingsPayload
import com.example.brido.network.BridoApiService
import com.example.brido.network.RetrofitClient
import com.google.gson.Gson
import com.google.gson.JsonSyntaxException
import com.example.brido.stream.StreamKeepAliveService
import com.example.brido.stream.StreamManager
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.io.ByteArrayOutputStream
import java.net.ConnectException
import java.net.SocketTimeoutException
import java.net.UnknownHostException
import javax.net.ssl.SSLException

class BridoViewModel(application: Application) : AndroidViewModel(application) {
    companion object {
        /** Terminal history cap, so a long session cannot grow without bound. */
        private const val MAX_TERMINAL_LINES = 500
    }

    private val store = ConnectionStore(application)

    // ── Connection state ─────────────────────────────────────────────────
    // Prefilled from the last successful connection so the user is not
    // retyping an IP address every launch.
    var serverIp by mutableStateOf(store.lastIp)
    var serverPort by mutableStateOf(store.lastPort)
    var pin by mutableStateOf("")
    var isConnecting by mutableStateOf(false)
    var isConnected by mutableStateOf(false)
    var connectionError by mutableStateOf<String?>(null)
    var token by mutableStateOf("")
    var serverInfo by mutableStateOf<ServerInfo?>(null)
    var trustDevice by mutableStateOf(false)

    /** Certificate fingerprint to enforce, from a QR scan or a past pairing. */
    private var expectedFingerprint: String? = null

    /** Model the server said it will use; shown so the user knows what ran. */
    var serverDefaultModel by mutableStateOf<String?>(null)
        private set

    /** True when a stored trusted token means the PIN can be skipped. */
    val hasTrustedSession: Boolean
        get() = store.trustedTokenFor(serverIp, serverPort) != null

    // ── Stream state ─────────────────────────────────────────────────────
    var currentFrame by mutableStateOf<Bitmap?>(null)
        private set
    var isStreaming by mutableStateOf(false)
        private set

    // ── Analysis state ───────────────────────────────────────────────────
    var terminalLines = mutableStateListOf<String>()
        private set
    var isAnalysing by mutableStateOf(false)
        private set

    // ── Internal ─────────────────────────────────────────────────────────
    private var apiService: BridoApiService? = null
    private var httpClient: OkHttpClient? = null
    private var streamManager: StreamManager? = null
    private val gson = Gson()
    private var streamReconnectAttempts = 0
    private val maxStreamReconnectAttempts = 3
    private val baseReconnectDelayMs = 1_500L

    /** Set when reconnect attempts are exhausted, so the UI can offer Retry. */
    var canRetryStream by mutableStateOf(false)
        private set

    // ── Synced desktop settings ──────────────────────────────────────────
    // Deliberately in-memory only. These carry live API keys, so they arrive
    // with the handshake, are never written to disk on the phone, and are
    // wiped on disconnect — including when the device is marked trusted.
    var settings by mutableStateOf<SettingsPayload?>(null)
        private set
    var providers by mutableStateOf<List<ProviderOption>>(emptyList())
        private set
    var isSavingSettings by mutableStateOf(false)
        private set
    var settingsMessage by mutableStateOf<String?>(null)

    /** Clears every synced credential from memory. */
    private fun clearSyncedSettings() {
        settings = null
        providers = emptyList()
        settingsMessage = null
        serverDefaultModel = null
    }

    /** Re-reads settings from the desktop (after an edit made there). */
    fun refreshSettings() {
        val service = apiService ?: return
        if (token.isBlank()) return
        viewModelScope.launch {
            try {
                val response = withContext(Dispatchers.IO) {
                    service.getSettings("Bearer $token")
                }
                settings = response.toPayload()
                providers = response.providers
            } catch (e: Exception) {
                settingsMessage = "Could not load settings: ${e.message ?: "unknown error"}"
            }
        }
    }

    /** Pushes edited settings to the desktop, which applies and persists them. */
    fun saveSettings(updated: SettingsPayload, onSaved: () -> Unit = {}) {
        val service = apiService ?: return
        if (token.isBlank()) return

        viewModelScope.launch {
            isSavingSettings = true
            settingsMessage = null
            try {
                val response = withContext(Dispatchers.IO) {
                    service.putSettings("Bearer $token", updated)
                }
                settings = response.toPayload()
                providers = response.providers
                serverDefaultModel = response.toPayload()
                    .modelFor(response.activeProvider)
                settingsMessage = "Saved to desktop"
                onSaved()
            } catch (e: Exception) {
                settingsMessage = "Save failed: ${e.message ?: "unknown error"}"
            } finally {
                isSavingSettings = false
            }
        }
    }

    /**
     * Appends a line to the terminal, trimming the oldest entries once the cap
     * is reached so a long-running session cannot exhaust memory.
     */
    private fun addTerminalLine(line: String) {
        terminalLines.add(line)
        while (terminalLines.size > MAX_TERMINAL_LINES) {
            terminalLines.removeAt(0)
        }
    }

    /** Retries the stream after reconnect attempts were exhausted. */
    fun retryStream() {
        if (!isConnected || token.isBlank()) return
        streamReconnectAttempts = 0
        canRetryStream = false
        addTerminalLine("> retrying stream...")
        startStream()
    }

    /** Applies connection details from a scanned QR code, including the pin. */
    fun applyScannedData(ip: String, port: Int, scannedPin: String, fingerprint: String?) {
        serverIp = ip
        serverPort = port
        pin = scannedPin
        // A fingerprint from the QR is authoritative — it pins the certificate
        // before the very first request rather than trusting on first use.
        expectedFingerprint = ServerTrust.normalise(fingerprint)
            ?: store.fingerprintFor(ip)
    }

    /**
     * Builds a pinned client for the current server.
     *
     * Any fingerprint learned during the handshake is persisted so a swapped
     * certificate is refused next time.
     */
    private fun buildSession(): RetrofitClient.Session {
        val pinned = expectedFingerprint ?: store.fingerprintFor(serverIp)
        val host = serverIp
        return RetrofitClient.createSession(
            serverIp = serverIp,
            port = serverPort,
            expectedFingerprint = pinned,
        ) { accepted ->
            store.setFingerprintFor(host, accepted)
            expectedFingerprint = accepted
        }
    }

    fun connect(onSuccess: () -> Unit) {
        if (serverIp.isBlank() || pin.isBlank()) return
        performConnect(usePin = true, onSuccess = onSuccess)
    }

    /**
     * Reconnects with the stored trusted token, skipping PIN entry.
     *
     * If the server has forgotten the device (restarted past the token's life,
     * or the user revoked it) the token is dropped and the PIN is required.
     */
    fun connectWithTrustedToken(onSuccess: () -> Unit) {
        if (store.trustedTokenFor(serverIp, serverPort) == null) return
        performConnect(usePin = false, onSuccess = onSuccess)
    }

    private fun performConnect(usePin: Boolean, onSuccess: () -> Unit) {
        viewModelScope.launch {
            isConnecting = true
            connectionError = null

            try {
                val session = withContext(Dispatchers.IO) { buildSession() }
                apiService = session.service
                httpClient = session.client

                if (usePin) {
                    val response = withContext(Dispatchers.IO) {
                        session.service.connect(
                            ConnectRequest(pin = pin, trustDevice = trustDevice)
                        )
                    }
                    token = response.token
                    serverInfo = response.systemInfo
                    serverDefaultModel = response.defaultModel
                    // Settings ride along with the handshake, so the settings
                    // screen is populated with zero extra round trips.
                    settings = response.settings
                    providers = response.providers

                    if (trustDevice) {
                        store.saveTrustedSession(serverIp, serverPort, response.token)
                    } else {
                        store.clearTrustedSession()
                    }
                } else {
                    val saved = store.trustedTokenFor(serverIp, serverPort)
                        ?: throw IllegalStateException("No trusted session stored")
                    // Validate the stored token before treating it as connected.
                    serverInfo = withContext(Dispatchers.IO) {
                        session.service.getSystemInfo("Bearer $saved")
                    }
                    token = saved

                    // A trusted reconnect skips the PIN but still re-syncs the
                    // credentials, because they were wiped on the last
                    // disconnect and are never stored on the device.
                    val synced = withContext(Dispatchers.IO) {
                        session.service.getSettings("Bearer $saved")
                    }
                    settings = synced.toPayload()
                    providers = synced.providers
                    serverDefaultModel = synced.toPayload().modelFor(synced.activeProvider)
                }

                store.rememberLast(serverIp, serverPort)
                isConnected = true
                connectionError = null

                // Start stream automatically after successful connection
                startStream()

                onSuccess()
            } catch (e: retrofit2.HttpException) {
                connectionError = when (e.code()) {
                    401 -> if (usePin) {
                        "Invalid PIN"
                    } else {
                        // Stored token is no longer good; fall back to the PIN.
                        store.clearTrustedSession()
                        "Saved session expired. Enter the PIN again."
                    }
                    429 -> "Too many failed attempts. Wait a moment and retry."
                    else -> "Server error: ${e.code()}"
                }
            } catch (e: Exception) {
                connectionError = classifyConnectionError(e)
            } finally {
                isConnecting = false
            }
        }
    }

    /** Forgets the saved token and pinned certificate for this server. */
    fun forgetThisDevice() {
        store.clearTrustedSession()
        store.clearFingerprintFor(serverIp)
        expectedFingerprint = null
        trustDevice = false
    }

    private fun startStream() {
        streamManager?.disconnect()
        canRetryStream = false

        val client = httpClient ?: return

        streamManager = StreamManager(
            httpClient = client,
            onFrame = { bitmap ->
                viewModelScope.launch(Dispatchers.Main) {
                    currentFrame = bitmap
                }
            },
            onConnected = {
                viewModelScope.launch(Dispatchers.Main) {
                    isStreaming = true
                    streamReconnectAttempts = 0
                    // Foreground service keeps the process alive once the user
                    // switches away from the app.
                    StreamKeepAliveService.start(getApplication())
                }
            },
            onDisconnected = { reason ->
                viewModelScope.launch(Dispatchers.Main) {
                    isStreaming = false
                    if (reason == "Client closing") {
                        return@launch
                    }

                    if (reason.contains("401") || reason.contains("403") || reason.contains("Unauthorized", ignoreCase = true)) {
                        addTerminalLine("> stream disconnected: unauthorized session")
                        invalidateSession("Session expired. Reconnect required.")
                        return@launch
                    }

                    if (reason.isNotBlank()) {
                        addTerminalLine("> stream disconnected: $reason")
                    }

                    if (!isConnected || token.isBlank()) {
                        return@launch
                    }

                    if (streamReconnectAttempts >= maxStreamReconnectAttempts) {
                        addTerminalLine("> stream reconnect failed after $maxStreamReconnectAttempts attempts")
                        addTerminalLine("> hint: tap Retry to try again")
                        // Surfaces a Retry button rather than making the user
                        // guess that disconnect-then-connect is the fix.
                        canRetryStream = true
                        return@launch
                    }

                    val delayMs = baseReconnectDelayMs * (1 shl streamReconnectAttempts)
                    streamReconnectAttempts += 1
                    addTerminalLine("> reconnecting stream in ${delayMs / 1000.0}s...")
                    delay(delayMs)

                    if (isConnected && token.isNotBlank()) {
                        streamManager?.connect(serverIp, serverPort, token)
                    }
                }
            },
        )

        streamManager?.connect(serverIp, serverPort, token)
    }

    /**
     * Captures the latest frame and asks the server to analyse it.
     *
     * @param question optional free-text prompt; when omitted the server uses
     *   its own default prompt, exactly as the desktop overlay does.
     */
    fun analyse(question: String? = null) {
        val frame = streamManager?.latestFrame ?: currentFrame
        if (frame == null || isAnalysing) return

        val prompt = question?.trim()?.takeIf { it.isNotEmpty() }

        // Set immediately to avoid double-tap races creating overlapping requests.
        isAnalysing = true

        viewModelScope.launch {
            if (prompt != null) {
                addTerminalLine("> Q: $prompt")
            }
            addTerminalLine("> analysing frame...")

            try {
                suspend fun encodeFrame(maxWidth: Int, quality: Int): String = withContext(Dispatchers.Default) {
                    val scaled = if (frame.width > maxWidth) {
                        val scale = maxWidth.toFloat() / frame.width
                        Bitmap.createScaledBitmap(
                            frame,
                            maxWidth,
                            (frame.height * scale).toInt(),
                            true,
                        )
                    } else frame
                    val stream = ByteArrayOutputStream()
                    scaled.compress(Bitmap.CompressFormat.JPEG, quality, stream)
                    Base64.encodeToString(stream.toByteArray(), Base64.NO_WRAP)
                }

                val service = apiService ?: return@launch

                suspend fun runAnalyse(maxWidth: Int, quality: Int) = withContext(Dispatchers.IO) {
                    val imageBase64 = encodeFrame(maxWidth, quality)
                    service.analyse(
                        token = "Bearer $token",
                        request = AnalyseRequest(
                            imageBase64 = imageBase64,
                            // No model is named on purpose: the server uses the
                            // provider and model configured on the desktop, so
                            // there is one source of truth and no hardcoded
                            // free-tier fallback.
                            model = null,
                            prompt = prompt,
                        ),
                    )
                }

                val presets = listOf(1440 to 88, 1024 to 80, 768 to 65)
                var response: AnalyseResponse? = null
                var lastError: Exception? = null

                for ((index, preset) in presets.withIndex()) {
                    val (maxWidth, quality) = preset
                    try {
                        response = runAnalyse(maxWidth, quality)
                        break
                    } catch (e: retrofit2.HttpException) {
                        lastError = e
                        if (index == presets.lastIndex || !shouldRetryWithNextPreset(e.code())) {
                            throw e
                        }

                        addTerminalLine("> retrying with smaller frame (${retryReasonForStatus(e.code())})...")
                    } catch (e: Exception) {
                        lastError = e
                        if (index == presets.lastIndex) {
                            throw e
                        }

                        addTerminalLine("> transient network issue, retrying...")
                    }
                }

                val resolvedResponse = response ?: throw lastError
                    ?: IllegalStateException("Analysis failed without a response")

                // Add full response as one block (server prefixes with [model-name])
                addTerminalLine(resolvedResponse.result.trim())
                addTerminalLine("")
            } catch (e: Exception) {
                val errorText = when (e) {
                    is retrofit2.HttpException -> {
                        val parsed = parseHttpError(e)
                        if (parsed.statusCode == 401 || parsed.statusCode == 403 || parsed.apiError?.code == "unauthorized") {
                            invalidateSession("Session expired. Reconnect required.")
                        }

                        formatApiError(parsed)
                    }
                    else -> e.message ?: "Unknown error"
                }

                addTerminalLine("> error: $errorText")
            } finally {
                isAnalysing = false
            }
        }
    }

    private data class ParsedHttpError(
        val statusCode: Int,
        val rawBody: String,
        val apiError: ApiError?,
    )

    private fun parseHttpError(error: retrofit2.HttpException): ParsedHttpError {
        val body = error.response()?.errorBody()?.string()?.trim().orEmpty()
        return ParsedHttpError(
            statusCode = error.code(),
            rawBody = body,
            apiError = parseApiError(body),
        )
    }

    private fun parseApiError(raw: String): ApiError? {
        if (raw.isBlank()) return null
        return try {
            gson.fromJson(raw, ApiError::class.java)
        } catch (_: JsonSyntaxException) {
            null
        } catch (_: Exception) {
            null
        }
    }

    private fun shouldRetryWithNextPreset(statusCode: Int): Boolean {
        return statusCode == 413 || statusCode == 429 || statusCode == 502 || statusCode == 503 || statusCode == 504 || statusCode >= 500
    }

    private fun retryReasonForStatus(statusCode: Int): String {
        return when (statusCode) {
            413 -> "payload too large"
            429 -> "provider rate limit"
            502 -> "provider unavailable"
            503 -> "queue busy"
            504 -> "provider timeout"
            else -> "server error"
        }
    }

    private fun formatApiError(parsed: ParsedHttpError): String {
        val code = parsed.apiError?.code.orEmpty()
        val message = parsed.apiError?.message?.takeIf { it.isNotBlank() }
        val hint = parsed.apiError?.hint?.takeIf { it.isNotBlank() }
        val requestIdSuffix = parsed.apiError?.requestId?.takeIf { it.isNotBlank() }
            ?.let { " [req:${it.take(8)}]" }
            .orEmpty()

        val base = when {
            code == "model_unsupported_image_input" || parsed.statusCode == 422 ->
                "Selected model does not support image input. Choose a vision-capable model/provider."
            parsed.statusCode == 401 || parsed.statusCode == 403 ->
                "Session invalid or expired. Reconnect and try again."
            code == "image_payload_too_large" || parsed.statusCode == 413 ->
                "Frame payload too large. Try smaller frame settings or lower quality."
            code == "provider_rate_limited" || parsed.statusCode == 429 ->
                "Provider rate limited this request. Wait briefly and retry."
            code == "provider_timeout" || parsed.statusCode == 504 ->
                "Provider timed out. Retry or switch provider."
            parsed.statusCode == 503 ->
                "Service temporarily unavailable or analysis queue busy."
            parsed.statusCode == 502 ->
                "Upstream provider error. Check provider key/model and retry."
            else -> message ?: "HTTP ${parsed.statusCode}"
        }

        val detail = if (message != null && message != base) " details: $message" else ""
        val hintText = hint?.let { " hint: $it" }.orEmpty()
        val fallbackRaw = if (parsed.apiError == null && parsed.rawBody.isNotBlank()) " details: ${parsed.rawBody}" else ""

        return "$base$detail$hintText$fallbackRaw$requestIdSuffix"
    }

    private fun classifyConnectionError(error: Exception): String {
        return when (error) {
            is UnknownHostException -> "Cannot resolve server address. Check IP and network."
            is ConnectException -> "Server refused connection. Confirm server is running and port is correct."
            is SocketTimeoutException -> "Connection timed out. Check network quality and server responsiveness."
            is SSLException -> {
                // A pin mismatch surfaces here. Say so plainly: the usual cause
                // is a restarted server (new certificate), but it is also what
                // an interception attempt looks like.
                val detail = error.message.orEmpty()
                if (detail.contains("pinned", ignoreCase = true) ||
                    detail.contains("fingerprint", ignoreCase = true)
                ) {
                    "Server certificate changed. If you restarted the server, tap " +
                        "\"Forget this device\" and pair again — otherwise the connection " +
                        "may be intercepted."
                } else {
                    "TLS handshake failed: ${detail.ifBlank { "could not establish a secure connection" }}"
                }
            }
            else -> "Cannot reach server: ${error.message ?: "Unknown error"}"
        }
    }

    private fun invalidateSession(reason: String) {
        StreamKeepAliveService.stop(getApplication())
        streamManager?.disconnect()
        streamManager = null
        isStreaming = false
        isConnected = false
        canRetryStream = false
        token = ""
        apiService = null
        httpClient = null
        clearSyncedSettings()
        // The server already refused this token, so drop any saved copy.
        store.clearTrustedSession()
        connectionError = reason
    }

    fun disconnect() {
        // Revoke server-side first, otherwise the token stays valid for its
        // full lifetime and signing out would be cosmetic only.
        val currentToken = token
        val service = apiService
        val keepTrusted = trustDevice && store.trustedTokenFor(serverIp, serverPort) != null

        if (service != null && currentToken.isNotBlank() && !keepTrusted) {
            viewModelScope.launch {
                try {
                    withContext(Dispatchers.IO) {
                        service.disconnect("Bearer $currentToken")
                    }
                } catch (_: Exception) {
                    // Best effort: if the server is already gone the token dies
                    // with it, and it expires on its own regardless.
                }
            }
        }

        StreamKeepAliveService.stop(getApplication())
        streamManager?.disconnect()
        streamManager = null
        isStreaming = false
        isConnected = false
        canRetryStream = false
        currentFrame = null
        token = ""
        terminalLines.clear()
        connectionError = null
        apiService = null
        httpClient = null
        // Synced credentials never outlive the connection.
        clearSyncedSettings()
    }

    override fun onCleared() {
        super.onCleared()
        StreamKeepAliveService.stop(getApplication())
        streamManager?.disconnect()
        clearSyncedSettings()
    }
}
