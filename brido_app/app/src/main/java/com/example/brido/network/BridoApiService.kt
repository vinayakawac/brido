package com.example.brido.network

import com.example.brido.models.AnalyseRequest
import com.example.brido.models.AnalyseResponse
import com.example.brido.models.ConnectRequest
import com.example.brido.models.ConnectResponse
import com.example.brido.models.ModelInfo
import com.example.brido.models.ServerInfo
import retrofit2.http.Body
import retrofit2.http.GET
import retrofit2.http.Header
import retrofit2.http.POST

interface BridoApiService {

    @POST("api/connect")
    suspend fun connect(@Body request: ConnectRequest): ConnectResponse

    @GET("api/system-info")
    suspend fun getSystemInfo(@Header("Authorization") token: String): ServerInfo

    @GET("api/models")
    suspend fun getModels(@Header("Authorization") token: String): List<ModelInfo>

    @POST("api/analyse")
    suspend fun analyse(
        @Header("Authorization") token: String,
        @Body request: AnalyseRequest,
    ): AnalyseResponse
}
