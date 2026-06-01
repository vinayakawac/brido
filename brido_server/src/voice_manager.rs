use anyhow::{anyhow, Result};
use reqwest::Client;
use serde::Deserialize;
use tracing::{error, info};

pub struct VoiceCopilot {
    client: Client,
    nvidia_api_key: String,
    deepgram_api_key: String,
    resume_text: String,
    job_description: String,
    pub transcript_history: String,
}

#[derive(Deserialize)]
struct NvidiaAsrResponse {
    text: String,
}

#[derive(Deserialize)]
struct DeepgramAsrResponse {
    results: DeepgramResults,
}

#[derive(Deserialize)]
struct DeepgramResults {
    channels: Vec<DeepgramChannel>,
}

#[derive(Deserialize)]
struct DeepgramChannel {
    alternatives: Vec<DeepgramAlternative>,
}

#[derive(Deserialize)]
struct DeepgramAlternative {
    transcript: String,
}

impl VoiceCopilot {
    pub fn new(nvidia_api_key: String, deepgram_api_key: String, resume_text: String, job_description: String) -> Self {
        Self {
            client: Client::new(),
            nvidia_api_key,
            deepgram_api_key,
            resume_text,
            job_description,
            transcript_history: String::new(),
        }
    }

    pub async fn transcribe_chunk(&self, wav_data: &[u8], asr_provider: &str, asr_model: &str) -> Result<String> {
        if asr_provider == "Deepgram" {
            self.transcribe_deepgram(wav_data, asr_model).await
        } else {
            self.transcribe_nvidia(wav_data, asr_model).await
        }
    }

    async fn transcribe_nvidia(&self, wav_data: &[u8], asr_model: &str) -> Result<String> {
        if self.nvidia_api_key.is_empty() {
            return Err(anyhow!("Nvidia API key is missing"));
        }

        let url = "https://integrate.api.nvidia.com/v1/audio/transcriptions";

        let part = reqwest::multipart::Part::bytes(wav_data.to_vec())
            .file_name("audio.wav")
            .mime_str("audio/wav")?;

        let model_name = if asr_model.is_empty() { "nvidia/parakeet-tdt-0.6b-v2" } else { asr_model };

        let form = reqwest::multipart::Form::new()
            .part("file", part)
            .text("model", model_name.to_string())
            .text("language", "en-US");

        let response = self.client.post(url)
            .bearer_auth(&self.nvidia_api_key)
            .multipart(form)
            .send()
            .await?;

        let status = response.status();
        let body_text = response.text().await?;

        if !status.is_success() {
            error!("Nvidia ASR error: {}", body_text);
            return Err(anyhow!("Nvidia ASR error: {}", status));
        }

        let parsed: NvidiaAsrResponse = serde_json::from_str(&body_text)?;
        Ok(parsed.text)
    }

    async fn transcribe_deepgram(&self, wav_data: &[u8], asr_model: &str) -> Result<String> {
        if self.deepgram_api_key.is_empty() {
            return Err(anyhow!("Deepgram API key is missing"));
        }

        let model_name = if asr_model.is_empty() { "nova-2" } else { asr_model };
        let url = format!("https://api.deepgram.com/v1/listen?model={}&smart_format=true", model_name);

        let response = self.client.post(&url)
            .header("Authorization", format!("Token {}", self.deepgram_api_key))
            .header("Content-Type", "audio/wav")
            .body(wav_data.to_vec())
            .send()
            .await?;

        let status = response.status();
        let body_text = response.text().await?;

        if !status.is_success() {
            error!("Deepgram ASR error: {}", body_text);
            return Err(anyhow!("Deepgram ASR error: {}", status));
        }

        let parsed: DeepgramAsrResponse = serde_json::from_str(&body_text)?;
        
        let transcript = parsed.results.channels
            .first()
            .and_then(|c| c.alternatives.first())
            .map(|a| a.transcript.clone())
            .unwrap_or_default();

        Ok(transcript)
    }

    pub async fn append_and_generate(&mut self, new_transcript: &str, config: &crate::config::Config) -> Result<Option<String>> {
        if new_transcript.trim().is_empty() {
            return Ok(None);
        }

        info!("New transcript segment: {}", new_transcript);
        
        self.transcript_history.push_str(new_transcript);
        self.transcript_history.push_str(" ");

        // Keep history reasonably bounded (last 3000 chars)
        if self.transcript_history.len() > 3000 {
            let offset = self.transcript_history.len() - 3000;
            self.transcript_history = self.transcript_history[offset..].to_string();
        }

        self.generate_answer(config).await
    }

    async fn generate_answer(&self, config: &crate::config::Config) -> Result<Option<String>> {
        let system_prompt = format!(
            "You are an expert interview copilot assisting a candidate in real-time. \
             Read the ongoing conversation transcript. Identify if an interview question is being asked or a technical topic is being discussed. \
             Provide a highly concise, bulleted hint (max 3 bullets) to help the candidate answer the question perfectly based on their resume and the job description. \
             If no direct question is asked, simply summarize the topic or stay silent. Do not write full paragraphs. Keep it under 50 words.\n\n\
             Resume Context:\n{}\n\n\
             Job Description:\n{}",
            self.resume_text, self.job_description
        );

        let prompt = format!(
            "System:\n{}\n\nUser: Current transcript:\n{}",
            system_prompt, self.transcript_history
        );

        let manager = crate::model_manager::ModelManager::new(config, &self.client);

        match manager.generate_text(&prompt).await {
            Ok((response, _model_used)) => Ok(Some(response)),
            Err(e) => Err(anyhow!("AI error: {}", e.message)),
        }
    }
}
