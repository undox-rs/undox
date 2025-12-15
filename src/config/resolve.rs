//! Child configuration resolution.
//!
//! This module handles resolving child configs by fetching their parent
//! and creating a synthetic root config for building.

use std::path::{Path, PathBuf};

use crate::git::GitFetcher;

use super::types::{ChildConfig, GitConfig, RootConfig, SourceLocation};
use super::{Config, ConfigError};

/// Result of resolving a child config, containing the synthetic root config
/// and the path to the parent repository (for theme resolution).
pub struct ResolvedChildConfig {
    /// The synthetic root config with all sources (child's source points to local content)
    pub config: RootConfig,
    /// Path to the parent repository (for theme resolution)
    pub parent_path: PathBuf,
}

impl ChildConfig {
    /// Resolve this child config by fetching the parent and creating a synthetic RootConfig.
    ///
    /// The resulting config will include all sources from the parent, but the child's
    /// source will point to local content instead of whatever was specified in the parent.
    pub fn resolve(
        &self,
        child_base_path: &Path,
        cache_dir: &Path,
    ) -> Result<ResolvedChildConfig, ConfigError> {
        // Use dev.parent override if set, otherwise use parent
        let parent_url = self
            .dev
            .as_ref()
            .and_then(|d| d.parent.as_ref())
            .unwrap_or(&self.parent);

        // Determine if parent is a git URL or local path
        let parent_path = if parent_url.starts_with("http://")
            || parent_url.starts_with("https://")
            || parent_url.starts_with("git@")
        {
            // It's a git URL - fetch it
            eprintln!("Fetching parent config from {}...", parent_url);
            let fetcher = GitFetcher::new(cache_dir.to_path_buf());
            let git_config = GitConfig {
                url: parent_url.clone(),
                git_ref: self.parent_ref.clone(),
                path: None,
                sparse: false,
            };
            fetcher.fetch(&git_config)?
        } else {
            // It's a local path
            let path = PathBuf::from(parent_url);
            if path.is_relative() {
                child_base_path.join(path)
            } else {
                path
            }
        };

        // Load parent config
        let parent_config_path = parent_path.join("undox.yaml");
        let parent_config_str =
            std::fs::read_to_string(&parent_config_path).map_err(ConfigError::ReadParent)?;

        // Parse parent config - it must be a root config
        let parent_config: Config = serde_yaml::from_str(&parent_config_str).map_err(|e| {
            ConfigError::Validation(format!("failed to parse parent config: {}", e))
        })?;

        let parent_root = match parent_config {
            Config::Root(root) => root,
            Config::Child(_) => return Err(ConfigError::ParentNotRoot),
        };

        // Find our source in the parent to verify it exists
        let source_index = parent_root
            .sources
            .iter()
            .position(|s| s.name == self.source)
            .ok_or_else(|| ConfigError::SourceNotFound(self.source.clone()))?;

        // Clone all sources, fixing paths relative to parent for non-child sources
        let mut sources = parent_root.sources.clone();

        for (i, source) in sources.iter_mut().enumerate() {
            if i == source_index {
                // This is the child's source - point to local content
                let resolved_content_path = if child_base_path.join("content").is_dir() {
                    child_base_path.join("content")
                } else {
                    child_base_path.to_path_buf()
                };
                source.location = SourceLocation::ContentPath {
                    content_path: resolved_content_path,
                };

                // Apply overrides from child config
                if let Some(repo_url) = &self.overrides.repo_url {
                    source.repo_url = Some(repo_url.clone());
                }
                if let Some(edit_path) = &self.overrides.edit_path {
                    source.edit_path = Some(edit_path.clone());
                }
                if let Some(nav) = &self.overrides.nav {
                    source.nav = Some(nav.clone());
                }
            } else {
                // Other sources - fix local paths to be absolute relative to parent
                if let SourceLocation::ContentPath { content_path } = &source.location
                    && content_path.is_relative()
                {
                    source.location = SourceLocation::ContentPath {
                        content_path: parent_path.join(content_path),
                    };
                }
            }
        }

        // Create root config with all sources (our source now points to local content)
        let synthetic_root = RootConfig {
            site: parent_root.site.clone(),
            sources,
            theme: parent_root.theme.clone(),
            markdown: parent_root.markdown.clone(),
            dev: parent_root.dev.clone(),
        };

        Ok(ResolvedChildConfig {
            config: synthetic_root,
            parent_path,
        })
    }
}
