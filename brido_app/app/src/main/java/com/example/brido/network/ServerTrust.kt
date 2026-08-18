package com.example.brido.network

import android.util.Log
import java.security.MessageDigest
import java.security.SecureRandom
import java.security.cert.CertificateException
import java.security.cert.X509Certificate
import javax.net.ssl.SSLContext
import javax.net.ssl.SSLSocketFactory
import javax.net.ssl.TrustManager
import javax.net.ssl.X509TrustManager

/**
 * Certificate pinning for the Brido server's self-signed certificate.
 *
 * The server generates its own certificate, so no public CA can vouch for it.
 * Instead of trusting *anything* (which leaves the connection open to a
 * man-in-the-middle on the same network), we pin the exact certificate:
 *
 *  - When paired by QR, the expected SHA-256 fingerprint travels in the payload
 *    and is enforced from the very first connection.
 *  - When paired manually there is no fingerprint to compare against, so the
 *    first certificate seen is recorded (trust on first use) and every later
 *    connection to that server must present the same one.
 *
 * A mismatch fails the handshake rather than warning, because by then an
 * attacker would already be positioned between the phone and the laptop.
 */
object ServerTrust {

    private const val TAG = "ServerTrust"

    /** SHA-256 of a DER-encoded certificate, as lowercase hex. */
    fun fingerprintOf(certificate: X509Certificate): String =
        MessageDigest.getInstance("SHA-256")
            .digest(certificate.encoded)
            .joinToString("") { "%02x".format(it) }

    /** Normalises user/QR-supplied fingerprints for comparison. */
    fun normalise(raw: String?): String? =
        raw?.replace(":", "")
            ?.replace(" ", "")
            ?.trim()
            ?.lowercase()
            ?.takeIf { it.length == 64 && it.all { c -> c.isDigit() || c in 'a'..'f' } }

    /**
     * Trust manager that accepts exactly one leaf certificate.
     *
     * @param expected pinned fingerprint, or null to learn one on first use.
     * @param onPinned invoked with the fingerprint actually accepted, so the
     *   caller can persist it for next time.
     */
    class PinnedTrustManager(
        private val expected: String?,
        private val onPinned: (String) -> Unit,
    ) : X509TrustManager {

        override fun checkClientTrusted(chain: Array<out X509Certificate>?, authType: String?) {
            // This app is never a TLS server; nothing to verify.
            throw CertificateException("Client authentication is not supported")
        }

        override fun checkServerTrusted(chain: Array<out X509Certificate>?, authType: String?) {
            val leaf = chain?.firstOrNull()
                ?: throw CertificateException("Server presented no certificate")

            val actual = fingerprintOf(leaf)
            val pinned = normalise(expected)

            if (pinned == null) {
                // Trust on first use: remember what we saw so a swapped
                // certificate is rejected from the second connection onward.
                Log.i(TAG, "Pinning server certificate on first use: ${actual.take(16)}…")
                onPinned(actual)
                return
            }

            if (actual != pinned) {
                throw CertificateException(
                    "Server certificate does not match the pinned fingerprint. " +
                        "Expected ${pinned.take(16)}…, got ${actual.take(16)}…"
                )
            }
        }

        override fun getAcceptedIssuers(): Array<X509Certificate> = emptyArray()
    }

    /** Builds an SSL socket factory bound to [trustManager]. */
    fun socketFactory(trustManager: X509TrustManager): SSLSocketFactory =
        SSLContext.getInstance("TLS").apply {
            init(null, arrayOf<TrustManager>(trustManager), SecureRandom())
        }.socketFactory
}
