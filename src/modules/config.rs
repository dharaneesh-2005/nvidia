use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub groq_api_key: String,
    pub server: ServerConfig,
    pub hotkey: String,
    pub project_path: String,
    pub audio: AudioConfig,
    pub models: ModelsConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioConfig {
    pub sample_rate: u32,
    pub channels: u16,
    pub chunk_duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelsConfig {
    pub whisper: String,
    pub text: String,
    pub vision: String,
}

impl Config {
    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        let content = fs::read_to_string("config.json")?;
        let config: Config = serde_json::from_str(&content)?;
        Ok(config)
    }
}
