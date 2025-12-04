use std::fs;
use std::path::Path;

use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct AppConfig {
    pub mqtt: MqttConfig,
    pub database: DatabaseConfig,
    pub logging: LoggingConfig,
}

#[derive(Debug, Deserialize)]
pub struct MqttConfig {
    pub broker_ip: String,
    pub broker_port: u16,
    pub topic_base: String,
    pub client_id: String,
    pub tele_period: u64,
}

#[derive(Debug, Deserialize)]
pub struct DatabaseConfig {
    pub filename: String,
}

#[derive(Debug, Deserialize)]
pub struct LoggingConfig {
    pub config_file: String,
}

/// Load configuration from a TOML file.
pub fn load_config(path: impl AsRef<Path>) -> Result<AppConfig, ConfigError> {
    let path = path.as_ref();
    let content = fs::read_to_string(path).map_err(|e| ConfigError::ReadError {
        path: path.display().to_string(),
        source: e,
    })?;
    let config: AppConfig = toml::from_str(&content).map_err(|e| ConfigError::ParseError {
        path: path.display().to_string(),
        source: e,
    })?;
    Ok(config)
}

#[derive(Debug)]
pub enum ConfigError {
    ReadError {
        path: String,
        source: std::io::Error,
    },
    ParseError {
        path: String,
        source: toml::de::Error,
    },
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::ReadError { path, source } => {
                write!(f, "Failed to read config file '{}': {}", path, source)
            }
            ConfigError::ParseError { path, source } => {
                write!(f, "Failed to parse config file '{}': {}", path, source)
            }
        }
    }
}

impl std::error::Error for ConfigError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ConfigError::ReadError { source, .. } => Some(source),
            ConfigError::ParseError { source, .. } => Some(source),
        }
    }
}
