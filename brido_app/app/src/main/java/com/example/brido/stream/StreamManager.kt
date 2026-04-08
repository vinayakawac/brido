package com.example.brido.stream

import android.graphics.Bitmap
import android.graphics.BitmapFactory
import com.example.brido.network.RetrofitClient
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.Response
import okhttp3.WebSocket
import okhttp3.WebSocketListener
import okio.ByteString
import java.util.concurrent.TimeUnit

class StreamManager(
    private val onFrame: (Bitmap) -> Unit,
    private val onConnected: () -> Unit,
    private val onDisconnected: (reason: String) -> Unit,
) {
    private val client = OkHttpClient.Builder()
        .sslSocketFactory(
            RetrofitClient.okHttpClient.sslSocketFactory,
            RetrofitClient.trustManager
        )
        .hostnameVerifier { _, _ -> true }
        .connectTimeout(5, TimeUnit.SECONDS)
        .readTimeout(0, TimeUnit.MILLISECONDS)
        .pingInterval(10, TimeUnit.SECONDS)
        .build()

    private var webSocket: WebSocket? = null

    @Volatile
    var latestFrame: Bitmap? = null
        private set

    fun connect(serverIp: String, port: Int, token: String) {
        disconnect()

        val url = "wss://$serverIp:$port/ws/stream?token=$token"
        val request = Request.Builder().url(url).build()

        webSocket = client.newWebSocket(request, object : WebSocketListener() {
            override fun onOpen(webSocket: WebSocket, response: Response) {
                onConnected()
            }

            override fun onMessage(webSocket: WebSocket, bytes: ByteString) {
                val data = bytes.toByteArray()
                val bitmap = BitmapFactory.decodeByteArray(data, 0, data.size)
                if (bitmap != null) {
                    latestFrame = bitmap
                    onFrame(bitmap)
                }
            }

            override fun onFailure(webSocket: WebSocket, t: Throwable, response: Response?) {
                val responseCode = response?.code
                val detail = if (responseCode != null) {
                    "${t.message ?: "Connection failed"} (http $responseCode)"
                } else {
                    t.message ?: "Connection failed"
                }
                onDisconnected(detail)
            }

            override fun onClosed(webSocket: WebSocket, code: Int, reason: String) {
                val detail = if (reason.isBlank()) "code=$code" else "code=$code reason=$reason"
                onDisconnected(detail)
            }
        })
    }

    fun disconnect() {
        webSocket?.close(1000, "Client closing")
        webSocket = null
    }
}
