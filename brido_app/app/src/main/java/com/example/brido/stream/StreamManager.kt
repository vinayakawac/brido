package com.example.brido.stream

import android.graphics.Bitmap
import android.graphics.BitmapFactory
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.Response
import okhttp3.WebSocket
import okhttp3.WebSocketListener
import okio.ByteString
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicBoolean

/**
 * @param httpClient the pinned client built during connect, so the stream is
 *   held to the same certificate as the REST calls.
 */
class StreamManager(
    httpClient: OkHttpClient,
    private val onFrame: (Bitmap) -> Unit,
    private val onConnected: () -> Unit,
    private val onDisconnected: (reason: String) -> Unit,
) {
    // Reuses the pinned TLS configuration; only the timeouts differ, because a
    // stream should never time out on read the way a request does.
    private val client = httpClient.newBuilder()
        .connectTimeout(5, TimeUnit.SECONDS)
        .readTimeout(0, TimeUnit.MILLISECONDS)
        .pingInterval(10, TimeUnit.SECONDS)
        .build()

    /**
     * One live socket plus a flag marking whether its callbacks still matter.
     *
     * The flag is cleared *before* we close a socket ourselves, so the close
     * callback that follows is ignored. Without this, an intentional
     * reconnect/disconnect fired `onDisconnected("Client closing")`, which the
     * ViewModel mistook for a dropout and reconnected — an endless loop.
     */
    private class Conn(val ws: WebSocket, val active: AtomicBoolean)

    @Volatile
    private var current: Conn? = null

    @Volatile
    var latestFrame: Bitmap? = null
        private set

    fun connect(serverIp: String, port: Int, token: String) {
        // Silence the previous socket before opening a new one.
        closeCurrent(intentional = true, reason = "reconnect")

        val url = "wss://$serverIp:$port/ws/stream?token=$token"
        val request = Request.Builder().url(url).build()

        val active = AtomicBoolean(true)
        val listener = object : WebSocketListener() {
            override fun onOpen(webSocket: WebSocket, response: Response) {
                if (active.get()) onConnected()
            }

            override fun onMessage(webSocket: WebSocket, bytes: ByteString) {
                if (!active.get()) return
                val data = bytes.toByteArray()
                val bitmap = BitmapFactory.decodeByteArray(data, 0, data.size)
                if (bitmap != null) {
                    latestFrame = bitmap
                    onFrame(bitmap)
                }
            }

            override fun onFailure(webSocket: WebSocket, t: Throwable, response: Response?) {
                // `getAndSet(false)` ensures a single disconnect callback per
                // socket, and never for one we already retired.
                if (!active.getAndSet(false)) return
                val responseCode = response?.code
                val detail = if (responseCode != null) {
                    "${t.message ?: "Connection failed"} (http $responseCode)"
                } else {
                    t.message ?: "Connection failed"
                }
                onDisconnected(detail)
            }

            override fun onClosed(webSocket: WebSocket, code: Int, reason: String) {
                if (!active.getAndSet(false)) return
                val detail = if (reason.isBlank()) "code=$code" else "code=$code reason=$reason"
                onDisconnected(detail)
            }
        }

        val ws = client.newWebSocket(request, listener)
        current = Conn(ws, active)
    }

    /** Closes the stream for good (user left, session invalidated). */
    fun disconnect() {
        closeCurrent(intentional = true, reason = "Client closing")
    }

    private fun closeCurrent(intentional: Boolean, reason: String) {
        val conn = current ?: return
        current = null
        // Retiring the socket first means its onClosed is a no-op, so an
        // intentional close never looks like a dropout.
        if (intentional) conn.active.set(false)
        conn.ws.close(1000, reason)
    }
}
