use rand::Rng;

#[derive(Clone)]
pub struct Config {
    pub port: u16,
    pub pin: String,
    pub capture_fps: u32,
    pub capture_quality: u8,
    pub target_width: u32,
    pub target_height: u32,
    pub ollama_url: String,
    pub default_vision_model: String,
    pub default_reasoning_model: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            port: 8080,
            pin: generate_pin(),
            capture_fps: 15,
            capture_quality: 65,
            target_width: 1280,
            target_height: 720,
            ollama_url: "http://localhost:11434".to_string(),
            default_vision_model: "qwen3-vl:4b".to_string(),
            default_reasoning_model: "deepseek-r1:8b".to_string(),
        }
    }
}

fn generate_pin() -> String {
    let mut rng = rand::thread_rng();
    format!("{:06}", rng.gen_range(0..1_000_000u32))
}
