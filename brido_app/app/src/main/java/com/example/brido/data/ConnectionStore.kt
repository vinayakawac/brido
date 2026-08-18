package com.example.brido.data

import android.content.Context
import android.content.SharedPreferences

/**
 * Remembers where the user last connected, the certificate pinned for that
 * server, and (when they asked for it) a reusable session token.
 *
 * Backed by app-private [SharedPreferences]: no extra dependency, and other
 * apps cannot read it. The token is a bearer credential, so it is only written
 * when the user explicitly ticks "Trust this device", and it is cleared as soon
 * as the server rejects it.
 */
class ConnectionStore(context: Context) {

    private val prefs: SharedPreferences =
        context.applicationContext.getSharedPreferences(PREFS, Context.MODE_PRIVATE)

    var lastIp: String
        get() = prefs.getString(KEY_IP, "").orEmpty()
        set(value) = prefs.edit().putString(KEY_IP, value).apply()

    var lastPort: Int
        get() = prefs.getInt(KEY_PORT, DEFAULT_PORT)
        set(value) = prefs.edit().putInt(KEY_PORT, value).apply()

    /** Pinned certificate fingerprint for [lastIp], if one has been seen. */
    fun fingerprintFor(ip: String): String? =
        prefs.getString(fingerprintKey(ip), null)

    fun setFingerprintFor(ip: String, fingerprint: String) {
        prefs.edit().putString(fingerprintKey(ip), fingerprint).apply()
    }

    fun clearFingerprintFor(ip: String) {
        prefs.edit().remove(fingerprintKey(ip)).apply()
    }

    /** Long-lived token from a "trusted" pairing, if the user opted in. */
    var trustedToken: String?
        get() = prefs.getString(KEY_TOKEN, null)?.takeIf { it.isNotBlank() }
        set(value) {
            prefs.edit().apply {
                if (value.isNullOrBlank()) remove(KEY_TOKEN) else putString(KEY_TOKEN, value)
            }.apply()
        }

    /** Server the trusted token belongs to, so it is never sent elsewhere. */
    var trustedHost: String?
        get() = prefs.getString(KEY_TOKEN_HOST, null)
        set(value) {
            prefs.edit().apply {
                if (value.isNullOrBlank()) remove(KEY_TOKEN_HOST) else putString(KEY_TOKEN_HOST, value)
            }.apply()
        }

    fun saveTrustedSession(ip: String, port: Int, token: String) {
        prefs.edit()
            .putString(KEY_TOKEN, token)
            .putString(KEY_TOKEN_HOST, hostKey(ip, port))
            .apply()
    }

    /** Returns the stored token only if it was issued by this exact server. */
    fun trustedTokenFor(ip: String, port: Int): String? =
        trustedToken?.takeIf { trustedHost == hostKey(ip, port) }

    fun clearTrustedSession() {
        prefs.edit().remove(KEY_TOKEN).remove(KEY_TOKEN_HOST).apply()
    }

    fun rememberLast(ip: String, port: Int) {
        prefs.edit().putString(KEY_IP, ip).putInt(KEY_PORT, port).apply()
    }

    private fun fingerprintKey(ip: String) = "$KEY_FINGERPRINT_PREFIX$ip"

    private fun hostKey(ip: String, port: Int) = "$ip:$port"

    companion object {
        const val DEFAULT_PORT = 8080
        private const val PREFS = "brido_connection"
        private const val KEY_IP = "last_ip"
        private const val KEY_PORT = "last_port"
        private const val KEY_TOKEN = "trusted_token"
        private const val KEY_TOKEN_HOST = "trusted_token_host"
        private const val KEY_FINGERPRINT_PREFIX = "fingerprint_"
    }
}
