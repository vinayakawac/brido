package com.example.brido.models

import com.google.gson.annotations.SerializedName

data class ConnectRequest(
    val pin: String,
)

data class ConnectResponse(
    val token: String,
    @SerializedName("system_info") val systemInfo: ServerInfo,
)

data class ServerInfo(
    val storage: String,
    @SerializedName("storage_used") val storageUsed: String,
    val gpu: String,
    @SerializedName("gpu_detail") val gpuDetail: String,
    val ram: String,
    @SerializedName("ram_speed") val ramSpeed: String,
    val processor: String,
    @SerializedName("processor_speed") val processorSpeed: String,
)

data class ModelInfo(
    val name: String,
    val file: String,
    val capability: String,
    @SerializedName("size_gb") val sizeGb: Double,
)

data class AnalyseRequest(
    @SerializedName("image_base64") val imageBase64: String,
    val model: String? = null,
    val prompt: String? = null,
)

data class AnalyseResponse(
    val result: String,
    @SerializedName("model_used") val modelUsed: String,
)
