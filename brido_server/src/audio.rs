use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::SampleFormat;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use tracing::{error, info};

#[derive(Clone, Debug)]
pub enum AudioSource {
    Microphone,
    System,
}

#[derive(Clone)]
pub struct AudioChunk {
    pub pcm_data: Vec<u8>,
    pub sample_rate: u32,
    pub channels: u16,
    pub source: AudioSource,
}

struct AudioBuffer {
    buffer: Vec<u8>,
    source: AudioSource,
    sample_rate: u32,
    channels: u16,
    tx: mpsc::Sender<AudioChunk>,
}

impl AudioBuffer {
    fn new(source: AudioSource, sample_rate: u32, channels: u16, tx: mpsc::Sender<AudioChunk>) -> Self {
        Self {
            buffer: Vec::new(),
            source,
            sample_rate,
            channels,
            tx,
        }
    }

    fn process_samples(&mut self, data: &[f32]) {
        for &sample in data {
            let sample_i16 = (sample * i16::MAX as f32).clamp(i16::MIN as f32, i16::MAX as f32) as i16;
            let bytes = sample_i16.to_le_bytes();
            self.buffer.push(bytes[0]);
            self.buffer.push(bytes[1]);
        }

        // Send a chunk approximately every 250ms
        let bytes_per_sec = self.sample_rate as usize * self.channels as usize * 2;
        let chunk_size = bytes_per_sec / 4; 

        if self.buffer.len() >= chunk_size {
            let chunk = AudioChunk {
                pcm_data: std::mem::take(&mut self.buffer),
                sample_rate: self.sample_rate,
                channels: self.channels,
                source: self.source.clone(),
            };
            let _ = self.tx.try_send(chunk);
        }
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
