package com.example.brido.viewmodel

import android.graphics.Bitmap
import android.util.Base64
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.example.brido.models.ApiError
import com.example.brido.models.AnalyseResponse
import com.example.brido.models.AnalyseRequest
import com.example.brido.models.ConnectRequest
import com.example.brido.models.ServerInfo
import com.example.brido.network.BridoApiService
import com.example.brido.network.RetrofitClient
import com.google.gson.Gson
import com.google.gson.JsonSyntaxException
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

class BridoViewModel : ViewModel() {

    // ── Connection state ─────────────────────────────────────────────────
    var serverIp by mutableStateOf("")
    var serverPort by mutableStateOf(8080)
    var pin by mutableStateOf("")
    var isConnecting by mutableStateOf(false)
    var isConnected by mutableStateOf(false)
    var connectionError by mutableStateOf<String?>(null)
    var token by mutableStateOf("")
    var serverInfo by mutableStateOf<ServerInfo?>(null)
    var trustDevice by mutableStateOf(false)

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
    private var streamManager: StreamManager? = null
    private val gson = Gson()
    private var streamReconnectAttempts = 0
    private val maxStreamReconnectAttempts = 3
    private val baseReconnectDelayMs = 1_500L

    fun connect(onSuccess: () -> Unit) {
        if (serverIp.isBlank() || pin.isBlank()) return

        viewModelScope.launch {
            isConnecting = true
            connectionError = null

            try {
                val service = RetrofitClient.getService(serverIp, serverPort)
                apiService = service

                val response = withContext(Dispatchers.IO) {
                    service.connect(ConnectRequest(pin))
                }

                token = response.token
                serverInfo = response.systemInfo
                isConnected = true
                connectionError = null

                // Start stream automatically after successful connection
                startStream()

                onSuccess()
            } catch (e: retrofit2.HttpException) {
                connectionError = when (e.code()) {
                    401 -> "Invalid PIN"
                    else -> "Server error: ${e.code()}"
                }
            } catch (e: Exception) {
                connectionError = classifyConnectionError(e)
            } finally {
                isConnecting = false
            }
        }
    }

    private fun startStream() {
        streamManager?.disconnect()

        streamManager = StreamManager(
            onFrame = { bitmap ->
                viewModelScope.launch(Dispatchers.Main) {
                    currentFrame = bitmap
                }
            },
            onConnected = {
                viewModelScope.launch(Dispatchers.Main) {
                    isStreaming = true
                    streamReconnectAttempts = 0
                }
            },
            onDisconnected = { reason ->
                viewModelScope.launch(Dispatchers.Main) {
                    isStreaming = false
                    if (reason == "Client closing") {
                        return@launch
                    }

                    if (reason.contains("401") || reason.contains("403") || reason.contains("Unauthorized", ignoreCase = true)) {
                        terminalLines.add("> stream disconnected: unauthorized session")
                        invalidateSession("Session expired. Reconnect required.")
                        return@launch
                    }

                    if (reason.isNotBlank()) {
                        terminalLines.add("> stream disconnected: $reason")
                    }

                    if (!isConnected || token.isBlank()) {
                        return@launch
                    }

                    if (streamReconnectAttempts >= maxStreamReconnectAttempts) {
                        terminalLines.add("> stream reconnect failed after $maxStreamReconnectAttempts attempts")
                        terminalLines.add("> hint: tap disconnect and connect again")
                        return@launch
                    }

                    val delayMs = baseReconnectDelayMs * (1 shl streamReconnectAttempts)
                    streamReconnectAttempts += 1
                    terminalLines.add("> reconnecting stream in ${delayMs / 1000.0}s...")
                    delay(delayMs)

                    if (isConnected && token.isNotBlank()) {
                        streamManager?.connect(serverIp, serverPort, token)
                    }
                }
            },
        )

        streamManager?.connect(serverIp, serverPort, token)
    }

    fun analyse() {
        val frame = streamManager?.latestFrame ?: currentFrame
        if (frame == null || isAnalysing) return

        // Set immediately to avoid double-tap races creating overlapping requests.
        isAnalysing = true

        viewModelScope.launch {
            terminalLines.add("> analysing frame...")

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

                        terminalLines.add("> retrying with smaller frame (${retryReasonForStatus(e.code())})...")
                    } catch (e: Exception) {
                        lastError = e
                        if (index == presets.lastIndex) {
                            throw e
                        }

                        terminalLines.add("> transient network issue, retrying...")
                    }
                }

                val resolvedResponse = response ?: throw lastError
                    ?: IllegalStateException("Analysis failed without a response")

                // Add full response as one block (server prefixes with [model-name])
                terminalLines.add(resolvedResponse.result.trim())
                terminalLines.add("")
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

                terminalLines.add("> error: $errorText")
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
            is SSLException -> "TLS handshake failed. Reconnect and accept the local certificate."
            else -> "Cannot reach server: ${error.message ?: "Unknown error"}"
        }
    }

    private fun invalidateSession(reason: String) {
        streamManager?.disconnect()
        streamManager = null
        isStreaming = false
        isConnected = false
        token = ""
        apiService = null
        connectionError = reason
    }

    fun disconnect() {
        streamManager?.disconnect()
        streamManager = null
        isStreaming = false
        isConnected = false
        currentFrame = null
        token = ""
        terminalLines.clear()
        connectionError = null
        apiService = null
    }

    override fun onCleared() {
        super.onCleared()
        streamManager?.disconnect()
    }
}
