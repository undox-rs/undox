//! Configuration loading from files.
//!
//! This module handles reading and parsing configuration files.

use std::path::Path;

use super::{Config, ConfigError};

impl Config {
    /// Load the config from the command line argument, defaulting to `undox.yaml`
    pub async fn load_from_arg(config_file: Option<&Path>) -> Result<Self, ConfigError> {
        let config_file = config_file.unwrap_or(Path::new("undox.yaml"));
        let config_file = if config_file.is_relative() {
            std::env::current_dir()
                .map_err(ConfigError::CwdFailure)?
                .join(config_file)
        } else {
            config_file.to_path_buf()
        };

        Self::load_from_file(&config_file).await
    }

    /// Load the config from a file path
    pub(crate) async fn load_from_file(path: &Path) -> Result<Self, ConfigError> {
        let content = std::fs::read_to_string(path).map_err(|e| {
            ConfigError::Validation(format!("failed to read config file: {}", e))
        })?;

        serde_yaml::from_str(&content).map_err(|e| {
            ConfigError::Validation(format!("failed to parse config: {}", e))
        })
    }
}
