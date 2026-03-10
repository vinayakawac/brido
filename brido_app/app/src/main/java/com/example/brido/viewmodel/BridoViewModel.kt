package com.example.brido.viewmodel

import android.graphics.Bitmap
import android.util.Base64
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.example.brido.models.AnalyseRequest
import com.example.brido.models.ConnectRequest
import com.example.brido.models.ServerInfo
import com.example.brido.network.BridoApiService
import com.example.brido.network.RetrofitClient
import com.example.brido.stream.StreamManager
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.io.ByteArrayOutputStream

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
    var selectedModel by mutableStateOf("qwen3-vl:8b")

    // ── Internal ─────────────────────────────────────────────────────────
    private var apiService: BridoApiService? = null
    private var streamManager: StreamManager? = null

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
                connectionError = "Cannot reach server: ${e.message}"
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
                }
            },
            onDisconnected = { reason ->
                viewModelScope.launch(Dispatchers.Main) {
                    isStreaming = false
                    if (reason.isNotBlank() && reason != "Client closing") {
                        terminalLines.add("> stream disconnected: $reason")
                    }
                }
            },
        )

        streamManager?.connect(serverIp, serverPort, token)
    }

    fun analyse() {
        val frame = streamManager?.latestFrame ?: currentFrame
        if (frame == null || isAnalysing) return

        viewModelScope.launch {
            isAnalysing = true
            terminalLines.add("> analysing frame...")

            try {
                // Compress the current frame to JPEG on a background thread
                val imageBase64 = withContext(Dispatchers.Default) {
                    val stream = ByteArrayOutputStream()
                    frame.compress(Bitmap.CompressFormat.JPEG, 85, stream)
                    Base64.encodeToString(stream.toByteArray(), Base64.NO_WRAP)
                }

                val service = apiService ?: return@launch
                val response = withContext(Dispatchers.IO) {
                    service.analyse(
                        token = "Bearer $token",
                        request = AnalyseRequest(
                            imageBase64 = imageBase64,
                            model = selectedModel,
                        ),
                    )
                }

                // Parse and display the result line by line
                terminalLines.add("> model: ${response.modelUsed}")
                response.result.lines().forEach { line ->
                    terminalLines.add(line)
                }
                terminalLines.add("")
            } catch (e: Exception) {
                terminalLines.add("> error: ${e.message}")
            } finally {
                isAnalysing = false
            }
        }
    }

    override fun onCleared() {
        super.onCleared()
        streamManager?.disconnect()
    }
}
