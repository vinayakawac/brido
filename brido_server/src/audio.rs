use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, SampleRate, SupportedStreamConfig};
use hound::{WavSpec, WavWriter};
use std::io::Cursor;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tracing::{error, info};

#[derive(Clone, Debug)]
pub enum AudioSource {
    Microphone,
    System,
}

#[derive(Clone)]
pub struct AudioChunk {
    pub wav_data: Vec<u8>,
    pub source: AudioSource,
}

struct AudioBuffer {
    samples: Vec<f32>,
    last_speech: Instant,
    is_speaking: bool,
    source: AudioSource,
    spec: WavSpec,
    tx: mpsc::Sender<AudioChunk>,
}

impl AudioBuffer {
    fn new(source: AudioSource, sample_rate: u32, channels: u16, tx: mpsc::Sender<AudioChunk>) -> Self {
        let spec = WavSpec {
            channels,
            sample_rate,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };
        Self {
            samples: Vec::new(),
            last_speech: Instant::now(),
            is_speaking: false,
            source,
            spec,
            tx,
        }
    }

    fn process_samples(&mut self, data: &[f32]) {
        // Calculate RMS
        let mut sum_sq = 0.0;
        for &sample in data {
            sum_sq += sample * sample;
        }
        let rms = (sum_sq / data.len() as f32).sqrt();

        // Simple VAD threshold (needs tuning based on actual mic/system volume)
        let threshold = 0.005;

        if rms > threshold {
            self.is_speaking = true;
            self.last_speech = Instant::now();
        }

        if self.is_speaking {
            self.samples.extend_from_slice(data);
        }

        // If silence for more than 1.5 seconds, flush
        if self.is_speaking && self.last_speech.elapsed() > Duration::from_millis(1500) {
            self.flush();
        }
        
        // Also flush if buffer gets too large (e.g., 15 seconds)
        let max_samples = (self.spec.sample_rate * self.spec.channels as u32 * 15) as usize;
        if self.samples.len() > max_samples {
            self.flush();
        }
    }

    fn flush(&mut self) {
        if self.samples.is_empty() {
            return;
        }

        // Require at least 0.5s of audio to send a chunk, otherwise discard
        let min_samples = (self.spec.sample_rate * self.spec.channels as u32) as usize / 2;
        if self.samples.len() < min_samples {
            self.samples.clear();
            self.is_speaking = false;
            return;
        }

        let mut cursor = Cursor::new(Vec::new());
        {
            let mut writer = WavWriter::new(&mut cursor, self.spec).unwrap();
            for &sample in &self.samples {
                writer.write_sample(sample).unwrap();
            }
            writer.finalize().unwrap();
        }

        let wav_data = cursor.into_inner();
        let chunk = AudioChunk {
            wav_data,
            source: self.source.clone(),
        };

        // try_send is non-blocking, good for audio thread
        let _ = self.tx.try_send(chunk);

        self.samples.clear();
        self.is_speaking = false;
    }
}

pub struct AudioCapture {
    _mic_stream: Option<cpal::Stream>,
    _sys_stream: Option<cpal::Stream>,
}

impl AudioCapture {
    pub fn new(tx: mpsc::Sender<AudioChunk>) -> anyhow::Result<Self> {
        let host = cpal::default_host();

        let mic_device = host.default_input_device();
        let sys_device = host.default_output_device();

        let _mic_stream = if let Some(dev) = mic_device {
            info!("Microphone found: {}", dev.name().unwrap_or_default());
            match Self::start_stream(dev, AudioSource::Microphone, tx.clone()) {
                Ok(stream) => Some(stream),
                Err(e) => {
                    error!("Failed to start microphone stream: {}", e);
                    None
                }
            }
        } else {
            info!("No microphone found.");
            None
        };

        let _sys_stream = if let Some(dev) = sys_device {
            info!("System audio found: {}", dev.name().unwrap_or_default());
            match Self::start_stream(dev, AudioSource::System, tx.clone()) {
                Ok(stream) => Some(stream),
                Err(e) => {
                    error!("Failed to start system audio stream: {}", e);
                    None
                }
            }
        } else {
            info!("No system audio found.");
            None
        };

        Ok(Self {
            _mic_stream,
            _sys_stream,
        })
    }

    fn start_stream(
        device: cpal::Device,
        source: AudioSource,
        tx: mpsc::Sender<AudioChunk>,
    ) -> anyhow::Result<cpal::Stream> {
        let config = match source {
            AudioSource::Microphone => device.default_input_config()?,
            AudioSource::System => device.default_output_config()?,
        };
        
        let sample_rate = config.sample_rate().0;
        let channels = config.channels();
        
        let buffer = Arc::new(Mutex::new(AudioBuffer::new(
            source.clone(),
            sample_rate,
            channels,
            tx,
        )));

        let err_fn = |err| error!("an error occurred on stream: {}", err);

        let stream = match config.sample_format() {
            SampleFormat::F32 => {
                let buf = buffer.clone();
                device.build_input_stream(
                    &config.into(),
                    move |data: &[f32], _: &_| {
                        if let Ok(mut b) = buf.lock() {
                            b.process_samples(data);
                        }
                    },
                    err_fn,
                    None,
                )?
            }
            SampleFormat::I16 => {
                let buf = buffer.clone();
                device.build_input_stream(
                    &config.into(),
                    move |data: &[i16], _: &_| {
                        let f32_data: Vec<f32> = data.iter().map(|&s| s as f32 / i16::MAX as f32).collect();
                        if let Ok(mut b) = buf.lock() {
                            b.process_samples(&f32_data);
                        }
                    },
                    err_fn,
                    None,
                )?
            }
            SampleFormat::U16 => {
                let buf = buffer.clone();
                device.build_input_stream(
                    &config.into(),
                    move |data: &[u16], _: &_| {
                        let f32_data: Vec<f32> = data.iter().map(|&s| (s as f32 - u16::MAX as f32 / 2.0) / (u16::MAX as f32 / 2.0)).collect();
                        if let Ok(mut b) = buf.lock() {
                            b.process_samples(&f32_data);
                        }
                    },
                    err_fn,
                    None,
                )?
            }
            _ => return Err(anyhow::anyhow!("Unsupported sample format")),
        };

        stream.play()?;
        Ok(stream)
    }
}
