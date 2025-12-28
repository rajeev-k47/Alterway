use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub listen_addr: String,
    pub max_connections: usize,
    pub blocked_domains_file: String,
    pub log_file: String,
    pub request_timeout_secs: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            listen_addr: "127.0.0.1:8888".to_string(),
            max_connections: 100,
            blocked_domains_file: "config/blocked_domains.txt".to_string(),
            log_file: "logs/proxy.log".to_string(),
            request_timeout_secs: 30,
        }
    }
}

impl Config {
    pub fn from_file(path: &str) -> Result<Self> {
        let contents = fs::read_to_string(path)?;
        let config: Config = toml::from_str(&contents)?;
        Ok(config)
    }

    pub fn save(&self, path: &str) -> Result<()> {
        let contents = toml::to_string_pretty(self)?;
        fs::write(path, contents)?;
        Ok(())
    }
}
