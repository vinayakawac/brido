package com.example.brido.network

import com.example.brido.models.AnalyseRequest
import com.example.brido.models.AnalyseResponse
import com.example.brido.models.ConnectRequest
import com.example.brido.models.ConnectResponse
import com.example.brido.models.ModelInfo
import com.example.brido.models.ServerInfo
import com.example.brido.models.SettingsPayload
import com.example.brido.models.SettingsResponse
import retrofit2.http.Body
import retrofit2.http.GET
import retrofit2.http.Header
import retrofit2.http.POST
import retrofit2.http.PUT

interface BridoApiService {

    @POST("api/connect")
    suspend fun connect(@Body request: ConnectRequest): ConnectResponse

    /** Revokes the current token so it cannot be replayed after sign-out. */
    @POST("api/disconnect")
    suspend fun disconnect(@Header("Authorization") token: String)

    @GET("api/system-info")
    suspend fun getSystemInfo(@Header("Authorization") token: String): ServerInfo

    @GET("api/models")
    suspend fun getModels(@Header("Authorization") token: String): List<ModelInfo>

    /** Current provider settings from the desktop. */
    @GET("api/settings")
    suspend fun getSettings(@Header("Authorization") token: String): SettingsResponse

    /** Pushes edited settings back to the desktop, which persists them. */
    @PUT("api/settings")
    suspend fun putSettings(
        @Header("Authorization") token: String,
        @Body settings: SettingsPayload,
    ): SettingsResponse

    @POST("api/analyse")
    suspend fun analyse(
        @Header("Authorization") token: String,
        @Body request: AnalyseRequest,
    ): AnalyseResponse
}
