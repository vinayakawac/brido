use anyhow::{anyhow, Result};
use reqwest::Client;
use serde::Deserialize;
use tracing::{error, info};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;
use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;

pub struct VoiceCopilot {
    client: Client,
    deepgram_api_key: String,
    resume_text: String,
    job_description: String,
    pub transcript_history: String,
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

#[derive(Deserialize)]
struct DeepgramWsResponse {
    #[serde(rename = "type")]
    msg_type: Option<String>,
    is_final: Option<bool>,
    speech_final: Option<bool>,
    channel: Option<DeepgramWsChannel>,
}

#[derive(Deserialize)]
struct DeepgramWsChannel {
    alternatives: Option<Vec<DeepgramWsAlternative>>,
}

#[derive(Deserialize)]
struct DeepgramWsAlternative {
    transcript: Option<String>,
}

impl VoiceCopilot {
    pub fn new(deepgram_api_key: String, resume_text: String, job_description: String) -> Self {
        Self {
            client: Client::new(),
            deepgram_api_key,
            resume_text,
            job_description,
            transcript_history: String::new(),
        }
    }

    pub async fn transcribe_chunk(&self, wav_data: &[u8], asr_model: &str) -> Result<String> {
        self.transcribe_deepgram(wav_data, asr_model).await
    }

    pub async fn start_streaming_session(
        &self,
        mut audio_rx: mpsc::Receiver<crate::audio::AudioChunk>,
        tx_transcript: mpsc::Sender<(String, bool)>, // (transcript, is_final)
    ) -> Result<()> {
        // Wait for the first chunk to know sample_rate and channels
        let first_chunk = match audio_rx.recv().await {
            Some(chunk) => chunk,
            None => return Err(anyhow!("Audio channel closed before starting")),
        };

        let url = format!(
            "wss://api.deepgram.com/v1/listen?interim_results=true&endpointing=300&language=en&model=nova-3&encoding=linear16&sample_rate={}&channels={}",
            first_chunk.sample_rate, first_chunk.channels
        );

        let mut request = url.into_client_request()?;
        request.headers_mut().insert(
            "Authorization",
            format!("Token {}", self.deepgram_api_key).parse()?,
        );

        let (ws_stream, _) = connect_async(request).await?;
        let (mut write, mut read) = ws_stream.split();

        // Send the first chunk
        write.send(Message::Binary(first_chunk.pcm_data.into())).await?;

        // Spawn a task to forward audio chunks
        tokio::spawn(async move {
            while let Some(chunk) = audio_rx.recv().await {
                if let Err(e) = write.send(Message::Binary(chunk.pcm_data.into())).await {
                    error!("Failed to send audio to Deepgram: {}", e);
                    break;
                }
            }
        });

        // Spawn a task to read responses
        tokio::spawn(async move {
            let mut utterance_buffer = String::new();

            while let Some(msg) = read.next().await {
                match msg {
                    Ok(Message::Text(text)) => {
                        let text_str = text.to_string(); 
                        if let Ok(parsed) = serde_json::from_str::<DeepgramWsResponse>(&text_str) {
                            let msg_type = parsed.msg_type.as_deref().unwrap_or("");
                            if msg_type != "Results" {
                                continue;
                            }

                            let is_final = parsed.is_final.unwrap_or(false);
                            let speech_final = parsed.speech_final.unwrap_or(false);
                            let transcript = parsed.channel
                                .and_then(|c| c.alternatives)
                                .and_then(|a| a.into_iter().next())
                                .and_then(|a| a.transcript)
                                .unwrap_or_default();
                            
                            if !is_final {
                                if !transcript.is_empty() {
                                    let display_text = if utterance_buffer.is_empty() {
                                        transcript.clone()
                                    } else {
                                        format!("{} {}", utterance_buffer, transcript)
                                    };
                                    let _ = tx_transcript.send((display_text, false)).await;
                                }
                            } else {
                                if !transcript.is_empty() {
                                    if !utterance_buffer.is_empty() {
                                        utterance_buffer.push(' ');
                                    }
                                    utterance_buffer.push_str(&transcript);
                                }
                                
                                if speech_final {
                                    if !utterance_buffer.is_empty() {
                                        let _ = tx_transcript.send((utterance_buffer.clone(), true)).await;
                                        utterance_buffer.clear();
                                    }
                                } else {
                                    // It's a final chunk of a longer sentence, send interim update
                                    if !utterance_buffer.is_empty() {
                                        let _ = tx_transcript.send((utterance_buffer.clone(), false)).await;
                                    }
                                }
                            }
                        }
                    }
                    Ok(Message::Close(_)) => {
                        info!("Deepgram connection closed");
                        break;
                    }
                    Err(e) => {
                        error!("Deepgram read error: {}", e);
                        break;
                    }
                    _ => {}
                }
            }
        });

        Ok(())
    }

    async fn transcribe_deepgram(&self, wav_data: &[u8], asr_model: &str) -> Result<String> {
        if self.deepgram_api_key.is_empty() {
            return Err(anyhow!("Deepgram API key is missing"));
        }

        let model_name = if asr_model.is_empty() { "nova-3" } else { asr_model };
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

    pub async fn append_and_generate(&mut self, new_transcript: &str, config: &crate::config::Config, tx: tokio::sync::mpsc::Sender<String>) -> Result<Option<String>> {
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

        self.generate_answer(config, tx).await
    }

    async fn generate_answer(&self, config: &crate::config::Config, tx: tokio::sync::mpsc::Sender<String>) -> Result<Option<String>> {
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

        match manager.generate_text_stream(&prompt, tx).await {
            Ok((response, _model_used)) => Ok(Some(response)),
            Err(e) => Err(anyhow!("AI error: {}", e.message)),
        }
    }
}
