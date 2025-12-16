//! Child configuration resolution.
//!
//! This module handles resolving child configs by fetching their parent
//! and creating a synthetic root config for building.

use std::path::{Path, PathBuf};

use crate::git::GitFetcher;

use super::types::{ChildConfig, Location, RootConfig, SourceLocation};
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
        let parent_location = self
            .dev
            .as_ref()
            .and_then(|d| d.parent.as_ref())
            .unwrap_or(&self.parent);

        // Resolve parent location to a local path
        let parent_path = resolve_location(parent_location, child_base_path, cache_dir)?;

        // Load parent config
        let parent_config_path = parent_path.join("undox.yaml");
        let parent_config_str =
            std::fs::read_to_string(&parent_config_path).map_err(ConfigError::ReadParent)?;

        // Parse parent config - it must be a root config
        let parent_config: Config = serde_yaml::from_str(&parent_config_str).map_err(|e| {
            ConfigError::Validation(format!("failed to parse parent config: {}", e))
        })?;

        let mut parent_root = match parent_config {
            Config::Root(root) => root,
            Config::Child(_) => return Err(ConfigError::ParentNotRoot),
        };

        // Find our source in the parent to verify it exists
        let source_index = parent_root
            .sources
            .iter()
            .position(|s| s.name == self.name)
            .ok_or_else(|| ConfigError::SourceNotFound(self.name.clone()))?;

        // Resolve the content path from child config
        let content_path = self.content.require_path().map_err(|git_url| {
            ConfigError::Validation(format!(
                "child config 'content' must be a path, not git: {}",
                &git_url
            ))
        })?;
        let resolved_content_path = if content_path.is_relative() {
            child_base_path.join(content_path)
        } else {
            content_path.clone()
        };

        // Clone all sources, fixing paths relative to parent for non-child sources
        let mut sources = parent_root.sources.clone();

        for (i, source) in sources.iter_mut().enumerate() {
            if i == source_index {
                // This is the child's source - point to local content
                source.location = SourceLocation::Local {
                    local: Location::Path {
                        path: resolved_content_path.clone(),
                    },
                };

                // Apply nav from child config
                if let Some(ref nav) = self.nav {
                    source.nav = Some(nav.clone());
                }
            } else {
                // Other sources - fix local paths to be absolute relative to parent
                match &source.location {
                    SourceLocation::Local { local } => {
                        if let Some(path) = local.as_path()
                            && path.is_relative()
                        {
                            source.location = SourceLocation::Local {
                                local: Location::Path {
                                    path: parent_path.join(path),
                                },
                            };
                        }
                    }
                    SourceLocation::Remote { location } => {
                        if let Some(path) = location.as_path()
                            && path.is_relative()
                        {
                            source.location = SourceLocation::Remote {
                                location: Location::Path {
                                    path: parent_path.join(path),
                                },
                            };
                        }
                    }
                }
            }
        }

        // Apply overrides from child config
        let mut theme = parent_root.theme;
        if let Some(ref overrides) = self.overrides {
            if let Some(ref site_overrides) = overrides.site {
                if let Some(ref repository) = site_overrides.repository {
                    parent_root.site.repository = Some(repository.clone());
                }
                if let Some(ref edit_path) = site_overrides.edit_path {
                    parent_root.site.edit_path = Some(edit_path.clone());
                }
            }
            if let Some(ref theme_override) = overrides.theme {
                theme = theme_override.clone();
            }
        }

        // Create root config with all sources (our source now points to local content)
        let synthetic_root = RootConfig {
            site: parent_root.site,
            sources,
            theme,
            markdown: parent_root.markdown,
            dev: parent_root.dev,
        };

        Ok(ResolvedChildConfig {
            config: synthetic_root,
            parent_path,
        })
    }
}

/// Resolve a Location to a local filesystem path.
/// For git locations, this fetches the repository to the cache.
/// For path locations, this resolves relative to base_path.
fn resolve_location(
    location: &Location,
    base_path: &Path,
    cache_dir: &Path,
) -> Result<PathBuf, ConfigError> {
    match location {
        Location::Path { path } => {
            if path.is_relative() {
                Ok(base_path.join(path))
            } else {
                Ok(path.clone())
            }
        }
        Location::Git { git } => {
            let git_loc = git.to_location();
            eprintln!("Fetching parent config from {}...", git_loc.url);
            let fetcher = GitFetcher::new(cache_dir.to_path_buf());
            let repo_path = fetcher.fetch_location(&git_loc)?;

            // Apply path if specified
            if let Some(ref path) = git_loc.path {
                Ok(repo_path.join(path))
            } else {
                Ok(repo_path)
            }
        }
    }
}
