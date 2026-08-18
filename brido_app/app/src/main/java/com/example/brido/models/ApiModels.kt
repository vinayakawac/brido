package com.example.brido.models

import com.google.gson.annotations.SerializedName

data class ConnectRequest(
    val pin: String,
    /** Ask the server for a long-lived token so the PIN can be skipped later. */
    @SerializedName("trust_device") val trustDevice: Boolean = false,
)

data class ConnectResponse(
    val token: String,
    @SerializedName("system_info") val systemInfo: ServerInfo,
    /** Seconds until the token expires. */
    @SerializedName("expires_in") val expiresIn: Long = 0,
    /** Model the server uses when a request does not name one. */
    @SerializedName("default_model") val defaultModel: String? = null,
    /**
     * Desktop provider settings, delivered with the handshake so the settings
     * screen is ready immediately. Held in memory only — never persisted.
     */
    val settings: SettingsPayload? = null,
    val providers: List<ProviderOption> = emptyList(),
)

/**
 * Provider credentials mirrored from the desktop.
 *
 * These are real API keys. They arrive over the certificate-pinned connection,
 * live only in memory, and are dropped on disconnect — including for devices
 * marked as trusted.
 */
data class SettingsPayload(
    @SerializedName("active_provider") val activeProvider: String = "",
    @SerializedName("gemini_api_key") val geminiApiKey: String = "",
    @SerializedName("gemini_model") val geminiModel: String = "",
    @SerializedName("openrouter_api_key") val openrouterApiKey: String = "",
    @SerializedName("openrouter_model") val openrouterModel: String = "",
    @SerializedName("ollama_api_key") val ollamaApiKey: String = "",
    @SerializedName("ollama_base_url") val ollamaBaseUrl: String = "",
    @SerializedName("ollama_model") val ollamaModel: String = "",
    @SerializedName("deepgram_api_key") val deepgramApiKey: String = "",
    @SerializedName("asr_model") val asrModel: String = "",
    @SerializedName("resume_text") val resumeText: String = "",
    @SerializedName("job_description_text") val jobDescriptionText: String = "",
) {
    /** Model belonging to whichever provider is active. */
    fun modelFor(provider: String): String = when (provider) {
        "OpenRouter" -> openrouterModel
        "Ollama" -> ollamaModel
        else -> geminiModel
    }

    fun withModelFor(provider: String, model: String): SettingsPayload = when (provider) {
        "OpenRouter" -> copy(openrouterModel = model)
        "Ollama" -> copy(ollamaModel = model)
        else -> copy(geminiModel = model)
    }

    fun keyFor(provider: String): String = when (provider) {
        "OpenRouter" -> openrouterApiKey
        "Ollama" -> ollamaApiKey
        else -> geminiApiKey
    }

    fun withKeyFor(provider: String, key: String): SettingsPayload = when (provider) {
        "OpenRouter" -> copy(openrouterApiKey = key)
        "Ollama" -> copy(ollamaApiKey = key)
        else -> copy(geminiApiKey = key)
    }
}

data class ProviderOption(
    val label: String = "",
    val models: List<String> = emptyList(),
    @SerializedName("default_model") val defaultModel: String = "",
)

/** GET/PUT /api/settings response: payload fields are flattened alongside providers. */
data class SettingsResponse(
    @SerializedName("active_provider") val activeProvider: String = "",
    @SerializedName("gemini_api_key") val geminiApiKey: String = "",
    @SerializedName("gemini_model") val geminiModel: String = "",
    @SerializedName("openrouter_api_key") val openrouterApiKey: String = "",
    @SerializedName("openrouter_model") val openrouterModel: String = "",
    @SerializedName("ollama_api_key") val ollamaApiKey: String = "",
    @SerializedName("ollama_base_url") val ollamaBaseUrl: String = "",
    @SerializedName("ollama_model") val ollamaModel: String = "",
    @SerializedName("deepgram_api_key") val deepgramApiKey: String = "",
    @SerializedName("asr_model") val asrModel: String = "",
    @SerializedName("resume_text") val resumeText: String = "",
    @SerializedName("job_description_text") val jobDescriptionText: String = "",
    val providers: List<ProviderOption> = emptyList(),
) {
    fun toPayload() = SettingsPayload(
        activeProvider = activeProvider,
        geminiApiKey = geminiApiKey,
        geminiModel = geminiModel,
        openrouterApiKey = openrouterApiKey,
        openrouterModel = openrouterModel,
        ollamaApiKey = ollamaApiKey,
        ollamaBaseUrl = ollamaBaseUrl,
        ollamaModel = ollamaModel,
        deepgramApiKey = deepgramApiKey,
        asrModel = asrModel,
        resumeText = resumeText,
        jobDescriptionText = jobDescriptionText,
    )
}

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

data class ProviderAttemptInfo(
    val provider: String? = null,
    val model: String? = null,
    val code: String? = null,
    val message: String? = null,
)

data class ApiError(
    val code: String? = null,
    val message: String? = null,
    val retryable: Boolean? = null,
    val hint: String? = null,
    val provider: String? = null,
    val model: String? = null,
    val attempts: List<ProviderAttemptInfo>? = null,
    @SerializedName("request_id") val requestId: String? = null,
)
