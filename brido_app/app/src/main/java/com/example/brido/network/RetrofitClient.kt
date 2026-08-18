package com.example.brido.network

import okhttp3.OkHttpClient
import okhttp3.logging.HttpLoggingInterceptor
import retrofit2.Retrofit
import retrofit2.converter.gson.GsonConverterFactory
import java.util.concurrent.TimeUnit
import javax.net.ssl.X509TrustManager

/**
 * Builds HTTP clients that trust exactly one certificate — the one belonging to
 * the paired Brido server. See [ServerTrust] for how the pin is established.
 */
object RetrofitClient {

    /** A client plus the Retrofit service sharing its pinned TLS settings. */
    class Session(
        val service: BridoApiService,
        val client: OkHttpClient,
    )

    /**
     * @param expectedFingerprint pinned SHA-256 from the QR code or a previous
     *   connection; null means trust-on-first-use.
     * @param onPinned receives the fingerprint that was accepted, for storage.
     */
    fun createSession(
        serverIp: String,
        port: Int,
        expectedFingerprint: String?,
        onPinned: (String) -> Unit,
    ): Session {
        val trustManager: X509TrustManager =
            ServerTrust.PinnedTrustManager(expectedFingerprint, onPinned)

        val client = OkHttpClient.Builder()
            .sslSocketFactory(ServerTrust.socketFactory(trustManager), trustManager)
            // The certificate is pinned by fingerprint, which is a stronger
            // check than matching a hostname on a self-signed cert whose SANs
            // we already control. Only the paired server can complete the
            // handshake, so hostname matching adds nothing here.
            .hostnameVerifier { _, _ -> true }
            .connectTimeout(10, TimeUnit.SECONDS)
            .readTimeout(60, TimeUnit.SECONDS)
            .writeTimeout(30, TimeUnit.SECONDS)
            .addInterceptor(
                HttpLoggingInterceptor().apply {
                    level = HttpLoggingInterceptor.Level.BASIC
                }
            )
            .build()

        val retrofit = Retrofit.Builder()
            .baseUrl("https://$serverIp:$port/")
            .client(client)
            .addConverterFactory(GsonConverterFactory.create())
            .build()

        return Session(retrofit.create(BridoApiService::class.java), client)
    }
}
