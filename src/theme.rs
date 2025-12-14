use std::path::Path;

use serde::{Deserialize, Serialize};

/// Theme configuration loaded from undox-theme.yaml
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ThemeConfig {
    /// Theme metadata
    #[serde(default)]
    pub name: Option<String>,

    /// Pagefind search configuration
    #[serde(default)]
    pub pagefind: PagefindConfig,
}

/// Pagefind-specific configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PagefindConfig {
    /// CSS selector for the root element to index (default: "main")
    #[serde(default = "default_root_selector")]
    pub root_selector: String,

    /// CSS selectors to exclude from indexing
    #[serde(default = "default_exclude_selectors")]
    pub exclude_selectors: Vec<String>,

    /// Force a specific language for indexing (ISO 639-1 code)
    #[serde(default)]
    pub force_language: Option<String>,
}

fn default_root_selector() -> String {
    "main".to_string()
}

fn default_exclude_selectors() -> Vec<String> {
    vec![
        "nav".to_string(),
        ".sidebar".to_string(),
        ".site-header".to_string(),
    ]
}

impl Default for PagefindConfig {
    fn default() -> Self {
        Self {
            root_selector: default_root_selector(),
            exclude_selectors: default_exclude_selectors(),
            force_language: None,
        }
    }
}

impl ThemeConfig {
    /// Load theme config from a theme directory.
    /// Returns default config if the file doesn't exist.
    pub fn load(theme_path: &Path) -> Result<Self, ThemeConfigError> {
        let config_path = theme_path.join("undox-theme.yaml");

        if !config_path.exists() {
            return Ok(Self::default());
        }

        let content = std::fs::read_to_string(&config_path)
            .map_err(|e| ThemeConfigError::Io(config_path.clone(), e))?;

        let config: ThemeConfig = serde_yaml::from_str(&content)
            .map_err(|e| ThemeConfigError::Parse(config_path.clone(), e))?;

        Ok(config)
    }
}

#[derive(thiserror::Error, Debug)]
pub enum ThemeConfigError {
    #[error("failed to read theme config {0}: {1}")]
    Io(std::path::PathBuf, std::io::Error),

    #[error("failed to parse theme config {0}: {1}")]
    Parse(std::path::PathBuf, serde_yaml::Error),
}
