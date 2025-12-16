//! Configuration loading and types for undox.
//!
//! This module handles all aspects of configuration:
//! - Type definitions for config structures (`types`)
//! - Loading configs from files (`load`)
//! - Resolving child configs to synthetic root configs (`resolve`)

mod load;
mod resolve;
mod types;

use serde::{Deserialize, Deserializer, Serialize};

// Re-export all types for convenient access
pub use types::{
    ChildConfig, DevConfig, GitLocation, GitValue, Location, MarkdownConfig, NavConfig, NavItem,
    RootConfig, SiteConfig, SourceConfig, SourceLocation, ThemeConfig, WatchConfig,
};

// =============================================================================
// Errors
// =============================================================================

#[derive(thiserror::Error, Debug)]
pub enum ConfigError {
    #[error("failed to deserialize config: {0}")]
    Deserialize(#[from] config::ConfigError),

    #[error("failed to get current working directory: {0}")]
    CwdFailure(std::io::Error),

    #[error("{0}")]
    Validation(String),

    #[error("failed to fetch parent repository: {0}")]
    GitFetch(#[from] crate::git::GitError),

    #[error("failed to read parent config: {0}")]
    ReadParent(std::io::Error),

    #[error("parent config is not a root config")]
    ParentNotRoot,

    #[error("source '{0}' not found in parent config")]
    SourceNotFound(String),
}

// =============================================================================
// Top-level config enum
// =============================================================================

/// The top-level configuration, which can be either a root site config
/// or a child config that points to a parent site.
#[derive(Debug, Clone, Serialize)]
pub enum Config {
    Root(RootConfig),
    Child(ChildConfig),
}

impl<'de> Deserialize<'de> for Config {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        use serde::de::Error;

        // First deserialize into a generic Value to inspect the structure
        let value = serde_yaml::Value::deserialize(deserializer)?;

        let obj = value.as_mapping().ok_or_else(|| {
            D::Error::custom("config must be a YAML/JSON object, not a scalar or array")
        })?;

        // Determine which variant based on distinguishing fields
        let has_site = obj.contains_key(&serde_yaml::Value::String("site".to_string()));
        let has_parent = obj.contains_key(&serde_yaml::Value::String("parent".to_string()));

        match (has_site, has_parent) {
            (true, false) => {
                // Root config
                serde_yaml::from_value::<RootConfig>(value)
                    .map(Config::Root)
                    .map_err(|e| D::Error::custom(format_root_error(e)))
            }
            (false, true) => {
                // Child config
                serde_yaml::from_value::<ChildConfig>(value)
                    .map(Config::Child)
                    .map_err(|e| D::Error::custom(format!("invalid child config: {e}")))
            }
            (true, true) => Err(D::Error::custom(
                "config cannot have both 'site' (root config) and 'parent' (child config) fields",
            )),
            (false, false) => Err(D::Error::custom(
                "invalid config: must have either 'site' field (for root config) or 'parent' field (for child config)",
            )),
        }
    }
}

/// Format a root config deserialization error with helpful context
fn format_root_error(e: serde_yaml::Error) -> String {
    let msg = e.to_string();

    // serde_json errors use "at line X column Y" for syntax errors
    // and path info for structural errors
    // Check for common issues and provide specific guidance
    if msg.contains("missing field `sources`") {
        return "invalid config: 'sources' list is required\n\nExample:\n  sources:\n    - name: docs\n      local:\n        path: ./content".to_string();
    }
    if msg.contains("missing field `name`") {
        // Could be site.name or source[].name - provide both possibilities
        return "invalid config: missing required 'name' field (check 'site.name' and each source's 'name')".to_string();
    }
    if msg.contains("missing field `theme`") {
        return "invalid config: 'theme' is required\n\nExample:\n  theme:\n    location:\n      path: ./themes/default".to_string();
    }
    if msg.contains("location must have either") {
        return "invalid config: each source must have either 'local: { path: ... }' for inline content or 'location: { path/git: ... }' for external sources".to_string();
    }

    format!("invalid config: {msg}")
}
