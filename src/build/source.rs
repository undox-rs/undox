use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::config::{Location, NavConfig, SourceConfig, SourceLocation};
use crate::git::GitFetcher;

use super::document::{ContentItem, Document, FrontMatter, StaticFile, parse_front_matter};
use super::format::FormatRegistry;
use super::paths::{source_path_to_url, static_path_to_url};

/// Partial config for local sub-docs (just the fields we need)
#[derive(Deserialize)]
struct LocalSubdocsConfig {
    content: Option<Location>,
    nav: Option<NavConfig>,
}

// =============================================================================
// Errors
// =============================================================================

#[derive(thiserror::Error, Debug)]
pub enum SourceError {
    #[error("source path does not exist: {0}")]
    PathNotFound(PathBuf),

    #[error("source path is not a directory: {0}")]
    NotADirectory(PathBuf),

    #[error("source location must be a path, not git: {0}")]
    LocalMustBePath(String),

    #[error("failed to read directory {path}: {source}")]
    ReadDir {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("failed to read directory entry in {path}: {source}")]
    ReadEntry {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("git fetch error: {0}")]
    Git(#[from] crate::git::GitError),
}

// =============================================================================
// Resolved source
// =============================================================================

/// A source after resolution - we now have a local path to its content.
#[derive(Debug, Clone)]
pub struct ResolvedSource {
    /// The source configuration
    pub config: SourceConfig,
    /// The resolved local path to the content directory
    pub local_path: PathBuf,
}

impl ResolvedSource {
    /// Resolve a source configuration to a local path.
    ///
    /// For local sources, this validates the path exists.
    /// For git sources, this clones/fetches the repo to the cache directory.
    pub fn resolve(
        mut config: SourceConfig,
        base_path: &Path,
        cache_dir: &Path,
    ) -> Result<Self, SourceError> {
        let local_path = match &config.location {
            SourceLocation::Local { local } => {
                // Local content - must be a path, not git
                let path = local.require_path().map_err(|git_url| {
                    SourceError::LocalMustBePath(git_url)
                })?;

                // Resolve relative paths against base_path
                let resolved = if path.is_relative() {
                    base_path.join(path)
                } else {
                    path.clone()
                };

                // Validate the path exists and is a directory
                if !resolved.exists() {
                    return Err(SourceError::PathNotFound(resolved));
                }
                if !resolved.is_dir() {
                    return Err(SourceError::NotADirectory(resolved));
                }

                resolved
            }
            SourceLocation::Remote { location } => {
                match location {
                    Location::Path { path } => {
                        // Remote source with local path (has its own undox.yaml)
                        let resolved = if path.is_relative() {
                            base_path.join(path)
                        } else {
                            path.clone()
                        };

                        // Validate the path exists and is a directory
                        if !resolved.exists() {
                            return Err(SourceError::PathNotFound(resolved));
                        }
                        if !resolved.is_dir() {
                            return Err(SourceError::NotADirectory(resolved));
                        }

                        // Check for undox.yaml to get content path and nav
                        let child_config_path = resolved.join("undox.yaml");
                        if child_config_path.exists()
                            && let Ok(content) = std::fs::read_to_string(&child_config_path)
                            && let Ok(subdocs_config) =
                                serde_yaml::from_str::<LocalSubdocsConfig>(&content)
                        {
                            // Apply nav from child config if not set in parent
                            if config.nav.is_none()
                                && let Some(nav) = subdocs_config.nav
                            {
                                config.nav = Some(nav);
                            }

                            // Use content path from child config
                            if let Some(content_location) = subdocs_config.content
                                && let Some(cp) = content_location.as_path()
                            {
                                let content_dir = resolved.join(cp);
                                if content_dir.exists() && content_dir.is_dir() {
                                    return Ok(Self {
                                        config,
                                        local_path: content_dir,
                                    });
                                }
                            }
                        }

                        // Fallback: look for content directory
                        let content_dir = resolved.join("content");
                        if content_dir.exists() && content_dir.is_dir() {
                            content_dir
                        } else {
                            resolved
                        }
                    }
                    Location::Git { git } => {
                        // Remote git source
                        let git_loc = git.to_location();
                        let fetcher = GitFetcher::new(cache_dir.to_path_buf());
                        let repo_path = fetcher.fetch_location(&git_loc)?;

                        // Apply subpath if specified
                        let resolved = if let Some(ref subpath) = git_loc.subpath {
                            repo_path.join(subpath)
                        } else {
                            repo_path
                        };

                        if !resolved.exists() {
                            return Err(SourceError::PathNotFound(resolved));
                        }
                        if !resolved.is_dir() {
                            return Err(SourceError::NotADirectory(resolved));
                        }

                        // Check for undox.yaml to get content path and nav
                        let child_config_path = resolved.join("undox.yaml");
                        if child_config_path.exists()
                            && let Ok(content) = std::fs::read_to_string(&child_config_path)
                            && let Ok(subdocs_config) =
                                serde_yaml::from_str::<LocalSubdocsConfig>(&content)
                        {
                            // Apply nav from child config if not set in parent
                            if config.nav.is_none()
                                && let Some(nav) = subdocs_config.nav
                            {
                                config.nav = Some(nav);
                            }

                            // Use content path from child config
                            if let Some(content_location) = subdocs_config.content
                                && let Some(cp) = content_location.as_path()
                            {
                                let content_dir = resolved.join(cp);
                                if content_dir.exists() && content_dir.is_dir() {
                                    return Ok(Self {
                                        config,
                                        local_path: content_dir,
                                    });
                                }
                            }
                        }

                        // Fallback: look for content directory
                        let content_dir = resolved.join("content");
                        if content_dir.exists() && content_dir.is_dir() {
                            content_dir
                        } else {
                            resolved
                        }
                    }
                }
            }
        };

        Ok(Self { config, local_path })
    }

    /// Get the URL prefix for this source, defaulting to /{name}
    pub fn url_prefix(&self) -> String {
        self.config
            .url_prefix
            .clone()
            .unwrap_or_else(|| format!("/{}", self.config.name))
    }

    /// Discover all content in this source.
    ///
    /// Walks the directory tree and returns all documents and static files found.
    /// Uses the format registry to determine which files are documents.
    pub fn discover_content(
        &self,
        format_registry: &FormatRegistry,
    ) -> Result<Vec<ContentItem>, SourceError> {
        let mut items = Vec::new();
        self.walk_directory(
            &self.local_path,
            &PathBuf::new(),
            format_registry,
            &mut items,
        )?;
        Ok(items)
    }

    /// Recursively walk a directory and collect content items.
    fn walk_directory(
        &self,
        dir: &Path,
        relative_path: &Path,
        format_registry: &FormatRegistry,
        items: &mut Vec<ContentItem>,
    ) -> Result<(), SourceError> {
        let entries = std::fs::read_dir(dir).map_err(|e| SourceError::ReadDir {
            path: dir.to_path_buf(),
            source: e,
        })?;

        for entry in entries {
            let entry = entry.map_err(|e| SourceError::ReadEntry {
                path: dir.to_path_buf(),
                source: e,
            })?;

            let path = entry.path();
            let file_name = entry.file_name();
            let file_name_str = file_name.to_string_lossy();

            // Skip hidden files and directories
            if file_name_str.starts_with('.') {
                continue;
            }

            // Skip common non-content directories
            if path.is_dir()
                && matches!(
                    file_name_str.as_ref(),
                    "node_modules" | "__pycache__" | "target" | ".git"
                )
            {
                continue;
            }

            let item_relative_path = relative_path.join(&file_name);

            if path.is_dir() {
                // Recurse into subdirectory
                self.walk_directory(&path, &item_relative_path, format_registry, items)?;
            } else if path.is_file() {
                // Determine if this is a document or static file
                let item = self.classify_file(&path, &item_relative_path, format_registry);
                items.push(item);
            }
        }

        Ok(())
    }

    /// Classify a file as either a Document or StaticFile.
    ///
    /// Uses the format registry to determine if a file is a document based on
    /// its extension. Files with registered format extensions are documents;
    /// all others are static files.
    fn classify_file(
        &self,
        full_path: &Path,
        relative_path: &Path,
        format_registry: &FormatRegistry,
    ) -> ContentItem {
        let url_prefix = self.url_prefix();

        if format_registry.is_document(relative_path) {
            // It's a document - read and parse front matter + content
            let url_path = source_path_to_url(relative_path, &url_prefix);

            // Read file and parse front matter, storing both metadata and content
            let (front_matter, raw_content) = match std::fs::read_to_string(full_path) {
                Ok(content) => {
                    let parsed = parse_front_matter(&content);
                    (parsed.front_matter, parsed.content)
                }
                Err(e) => {
                    eprintln!("Warning: Failed to read {}: {}", full_path.display(), e);
                    (FrontMatter::default(), String::new())
                }
            };

            ContentItem::Document(Document::new(
                self.config.name.clone(),
                relative_path.to_path_buf(),
                url_path,
                front_matter,
                raw_content,
            ))
        } else {
            // It's a static file
            let output_path = static_path_to_url(relative_path, &url_prefix);
            ContentItem::Static(StaticFile::new(
                self.config.name.clone(),
                relative_path.to_path_buf(),
                output_path,
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_path_to_url() {
        // Tests for source_path_to_url are in paths.rs
        // This test verifies the url_prefix method works correctly
        let config = SourceConfig {
            name: "cli".to_string(),
            title: Some("CLI".to_string()),
            url_prefix: Some("/cli".to_string()),
            nav: None,
            location: SourceLocation::Local {
                local: Location::Path {
                    path: PathBuf::from("./docs"),
                },
            },
        };

        let source = ResolvedSource {
            config,
            local_path: PathBuf::from("/tmp/docs"),
        };

        assert_eq!(source.url_prefix(), "/cli");

        // Verify integration with path functions
        assert_eq!(
            source_path_to_url(Path::new("installation.md"), &source.url_prefix()),
            "/cli/installation"
        );
    }

    #[test]
    fn test_path_to_url_root_source() {
        let config = SourceConfig {
            name: "docs".to_string(),
            title: Some("Docs".to_string()),
            url_prefix: Some("/".to_string()),
            nav: None,
            location: SourceLocation::Local {
                local: Location::Path {
                    path: PathBuf::from("./docs"),
                },
            },
        };

        let source = ResolvedSource {
            config,
            local_path: PathBuf::from("/tmp/docs"),
        };

        // Root source has "/" prefix
        let prefix = source.url_prefix();
        assert_eq!(prefix, "/");

        // With "/" prefix, paths should work correctly
        assert_eq!(
            source_path_to_url(Path::new("installation.md"), &prefix),
            "/installation"
        );
        assert_eq!(source_path_to_url(Path::new("index.md"), &prefix), "/");
    }

    #[test]
    fn test_path_to_static_url() {
        assert_eq!(
            static_path_to_url(Path::new("images/screenshot.png"), "/cli"),
            "/cli/images/screenshot.png"
        );
    }
}
