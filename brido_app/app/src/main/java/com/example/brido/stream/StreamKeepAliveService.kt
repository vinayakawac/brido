package com.example.brido.stream

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.Build
import android.os.IBinder
import androidx.core.app.NotificationCompat
import androidx.core.content.ContextCompat
import com.example.brido.MainActivity
import com.example.brido.R

/**
 * Keeps the app's process in the foreground while a stream is running.
 *
 * The WebSocket itself lives in the ViewModel, but without a foreground service
 * Android is free to freeze or kill the process as soon as the user switches
 * away — which is exactly when someone would want to keep watching their
 * laptop screen. The notification is the price of that guarantee, and it
 * doubles as the way back into the app.
 */
class StreamKeepAliveService : android.app.Service() {

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        if (intent?.action == ACTION_STOP) {
            stopSelf()
            return START_NOT_STICKY
        }

        createChannel()
        val notification = buildNotification()

        // Android 10+ wants the service type declared at start time; the stream
        // is a long-lived data transfer, so dataSync is the matching type.
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            startForeground(
                NOTIFICATION_ID,
                notification,
                ServiceInfo.FOREGROUND_SERVICE_TYPE_DATA_SYNC,
            )
        } else {
            startForeground(NOTIFICATION_ID, notification)
        }

        // Not sticky: if the system kills us, silently resurrecting a stream
        // the user can no longer see would be worse than stopping.
        return START_NOT_STICKY
    }

    private fun buildNotification(): Notification {
        val openApp = PendingIntent.getActivity(
            this,
            0,
            Intent(this, MainActivity::class.java).apply {
                flags = Intent.FLAG_ACTIVITY_SINGLE_TOP or Intent.FLAG_ACTIVITY_CLEAR_TOP
            },
            PendingIntent.FLAG_IMMUTABLE,
        )

        return NotificationCompat.Builder(this, CHANNEL_ID)
            .setContentTitle("Brido is streaming")
            .setContentText("Connected to your laptop screen")
            .setSmallIcon(R.mipmap.ic_launcher)
            .setContentIntent(openApp)
            .setOngoing(true)
            .setPriority(NotificationCompat.PRIORITY_LOW)
            .setCategory(NotificationCompat.CATEGORY_SERVICE)
            .build()
    }

    private fun createChannel() {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O) return
        val manager = getSystemService(NotificationManager::class.java) ?: return
        if (manager.getNotificationChannel(CHANNEL_ID) != null) return

        manager.createNotificationChannel(
            NotificationChannel(
                CHANNEL_ID,
                "Screen stream",
                // Low importance: this is a status indicator, not an alert.
                NotificationManager.IMPORTANCE_LOW,
            ).apply {
                description = "Shown while Brido is receiving your laptop screen"
                setShowBadge(false)
            }
        )
    }

    companion object {
        private const val CHANNEL_ID = "brido_stream"
        private const val NOTIFICATION_ID = 1001
        private const val ACTION_STOP = "com.example.brido.STOP_STREAM_SERVICE"

        fun start(context: Context) {
            val intent = Intent(context, StreamKeepAliveService::class.java)
            ContextCompat.startForegroundService(context, intent)
        }

        fun stop(context: Context) {
            context.stopService(Intent(context, StreamKeepAliveService::class.java))
        }
    }
}
