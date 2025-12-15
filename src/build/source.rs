use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::config::{NavConfig, SourceConfig, SourceLocation};
use crate::git::GitFetcher;

use super::document::{ContentItem, Document, StaticFile, parse_front_matter};

/// Partial config for local sub-docs (just the fields we need)
#[derive(Deserialize)]
struct LocalSubdocsConfig {
    content_path: Option<PathBuf>,
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
            SourceLocation::ContentPath { content_path } => {
                // Resolve relative paths against base_path
                let resolved = if content_path.is_relative() {
                    base_path.join(content_path)
                } else {
                    content_path.clone()
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
            SourceLocation::LocalPath { path } => {
                // Local sub-docs - resolve the path and look for content
                // Similar to git sources but without fetching
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

                // Check for undox.yaml to get content_path and nav
                let child_config_path = resolved.join("undox.yaml");
                if child_config_path.exists() {
                    if let Ok(content) = std::fs::read_to_string(&child_config_path) {
                        if let Ok(subdocs_config) =
                            serde_yaml::from_str::<LocalSubdocsConfig>(&content)
                        {
                            // Apply nav from child config if not set in parent
                            if config.nav.is_none() {
                                if let Some(nav) = subdocs_config.nav {
                                    config.nav = Some(nav);
                                }
                            }

                            // Use content_path from child config
                            if let Some(cp) = subdocs_config.content_path {
                                let content_dir = resolved.join(&cp);
                                if content_dir.exists() && content_dir.is_dir() {
                                    return Ok(Self {
                                        config,
                                        local_path: content_dir,
                                    });
                                }
                            }
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
            SourceLocation::Git { git } => {
                // Fetch/update the git repository
                let fetcher = GitFetcher::new(cache_dir.to_path_buf());
                let repo_path = fetcher.fetch(git)?;

                // If git.path is set, use that subdirectory within the repo
                match &git.path {
                    Some(subpath) => {
                        let full_path = repo_path.join(subpath);
                        if !full_path.exists() {
                            return Err(SourceError::PathNotFound(full_path));
                        }
                        if !full_path.is_dir() {
                            return Err(SourceError::NotADirectory(full_path));
                        }
                        full_path
                    }
                    None => repo_path,
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
    pub fn discover_content(&self) -> Result<Vec<ContentItem>, SourceError> {
        let mut items = Vec::new();
        self.walk_directory(&self.local_path, &PathBuf::new(), &mut items)?;
        Ok(items)
    }

    /// Recursively walk a directory and collect content items.
    fn walk_directory(
        &self,
        dir: &Path,
        relative_path: &Path,
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
            if path.is_dir() && matches!(file_name_str.as_ref(), "node_modules" | "__pycache__" | "target" | ".git") {
                continue;
            }

            let item_relative_path = relative_path.join(&file_name);

            if path.is_dir() {
                // Recurse into subdirectory
                self.walk_directory(&path, &item_relative_path, items)?;
            } else if path.is_file() {
                // Determine if this is a document or static file
                let item = self.classify_file(&path, &item_relative_path);
                items.push(item);
            }
        }

        Ok(())
    }

    /// Classify a file as either a Document or StaticFile.
    fn classify_file(&self, full_path: &Path, relative_path: &Path) -> ContentItem {
        let extension = relative_path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase());

        let url_prefix = self.url_prefix();

        match extension.as_deref() {
            Some("md" | "markdown") => {
                // It's a markdown document - read and parse frontmatter for title
                let url_path = self.path_to_url(relative_path, &url_prefix);

                // Read file and parse frontmatter to get title
                let front_matter = std::fs::read_to_string(full_path)
                    .ok()
                    .map(|content| parse_front_matter(&content).front_matter)
                    .unwrap_or_default();

                ContentItem::Document(Document::discovered_with_front_matter(
                    self.config.name.clone(),
                    relative_path.to_path_buf(),
                    url_path,
                    front_matter,
                ))
            }
            _ => {
                // It's a static file
                let output_path = self.path_to_static_url(relative_path, &url_prefix);
                ContentItem::Static(StaticFile::new(
                    self.config.name.clone(),
                    relative_path.to_path_buf(),
                    output_path,
                ))
            }
        }
    }

    /// Convert a file path to a URL path for documents.
    /// "getting-started/installation.md" -> "/cli/getting-started/installation"
    fn path_to_url(&self, path: &Path, url_prefix: &str) -> String {
        let mut url = url_prefix.to_string();
        if !url.ends_with('/') {
            url.push('/');
        }

        // Remove .md extension and convert path separators
        let path_str = path.with_extension("").to_string_lossy().to_string();
        let path_str = path_str.replace('\\', "/");

        // Handle index files - they become the directory URL
        let path_str = if path_str.ends_with("/index") || path_str == "index" {
            path_str.trim_end_matches("/index").trim_end_matches("index").to_string()
        } else {
            path_str
        };

        url.push_str(&path_str);

        // Normalize: remove trailing slash unless it's the root
        if url.len() > 1 && url.ends_with('/') {
            url.pop();
        }

        // Ensure we have at least a slash
        if url.is_empty() {
            url = "/".to_string();
        }

        url
    }

    /// Convert a file path to an output path for static files.
    /// "images/screenshot.png" -> "/cli/images/screenshot.png"
    fn path_to_static_url(&self, path: &Path, url_prefix: &str) -> String {
        let mut url = url_prefix.to_string();
        if !url.ends_with('/') {
            url.push('/');
        }

        let path_str = path.to_string_lossy().to_string();
        let path_str = path_str.replace('\\', "/");

        url.push_str(&path_str);
        url
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_path_to_url() {
        let config = SourceConfig {
            name: "cli".to_string(),
            url_prefix: Some("/cli".to_string()),
            repo_url: None,
            edit_path: None,
            nav: None,
            location: SourceLocation::ContentPath {
                content_path: PathBuf::from("./docs"),
            },
        };

        let source = ResolvedSource {
            config,
            local_path: PathBuf::from("/tmp/docs"),
        };

        let prefix = source.url_prefix();

        assert_eq!(
            source.path_to_url(Path::new("installation.md"), &prefix),
            "/cli/installation"
        );
        assert_eq!(
            source.path_to_url(Path::new("getting-started/quickstart.md"), &prefix),
            "/cli/getting-started/quickstart"
        );
        assert_eq!(
            source.path_to_url(Path::new("index.md"), &prefix),
            "/cli"
        );
        assert_eq!(
            source.path_to_url(Path::new("commands/index.md"), &prefix),
            "/cli/commands"
        );
    }

    #[test]
    fn test_path_to_url_root_source() {
        let config = SourceConfig {
            name: "docs".to_string(),
            url_prefix: Some("/".to_string()),
            repo_url: None,
            edit_path: None,
            nav: None,
            location: SourceLocation::ContentPath {
                content_path: PathBuf::from("./docs"),
            },
        };

        let source = ResolvedSource {
            config,
            local_path: PathBuf::from("/tmp/docs"),
        };

        let prefix = source.url_prefix();

        assert_eq!(
            source.path_to_url(Path::new("installation.md"), &prefix),
            "/installation"
        );
        assert_eq!(
            source.path_to_url(Path::new("index.md"), &prefix),
            "/"
        );
    }

    #[test]
    fn test_path_to_static_url() {
        let config = SourceConfig {
            name: "cli".to_string(),
            url_prefix: Some("/cli".to_string()),
            repo_url: None,
            edit_path: None,
            nav: None,
            location: SourceLocation::ContentPath {
                content_path: PathBuf::from("./docs"),
            },
        };

        let source = ResolvedSource {
            config,
            local_path: PathBuf::from("/tmp/docs"),
        };

        let prefix = source.url_prefix();

        assert_eq!(
            source.path_to_static_url(Path::new("images/screenshot.png"), &prefix),
            "/cli/images/screenshot.png"
        );
    }
}
