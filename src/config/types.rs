//! Configuration type definitions.
//!
//! This module contains all the data structures used in undox configuration files.
//! These types are pure data - no I/O or complex logic.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

// =============================================================================
// Location types - unified path/git reference
// =============================================================================

/// A unified location specifier for content.
/// Can be either a local path or a git repository reference.
///
/// YAML formats:
/// ```yaml
/// # Path variant
/// path: ./local/path
///
/// # Git variant - compact string format
/// git: https://repo#ref
///
/// # Git variant - expanded format
/// git:
///   url: https://repo
///   ref: main
///   subpath: docs
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Location {
    /// A local filesystem path
    Path { path: PathBuf },
    /// A git repository reference (compact string or expanded object)
    Git { git: GitValue },
}

/// Git value that can be either a compact string or expanded object.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum GitValue {
    /// Compact format: "https://repo#ref"
    Compact(String),
    /// Expanded format with url, ref, and subpath
    Expanded(GitLocation),
}

/// Git repository location details (expanded format).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitLocation {
    /// Repository URL
    pub url: String,
    /// Branch, tag, or commit (defaults to default branch)
    #[serde(rename = "ref", default, skip_serializing_if = "Option::is_none")]
    pub git_ref: Option<String>,
    /// Subdirectory within the repository
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subpath: Option<PathBuf>,
}

impl GitLocation {
    /// Parse a compact git string like "https://repo#ref" into GitLocation
    pub fn from_compact(s: &str) -> Self {
        if let Some((url, git_ref)) = s.split_once('#') {
            GitLocation {
                url: url.to_string(),
                git_ref: Some(git_ref.to_string()),
                subpath: None,
            }
        } else {
            GitLocation {
                url: s.to_string(),
                git_ref: None,
                subpath: None,
            }
        }
    }
}

impl GitValue {
    /// Get the GitLocation, parsing compact format if needed
    pub fn to_location(&self) -> GitLocation {
        match self {
            GitValue::Compact(s) => GitLocation::from_compact(s),
            GitValue::Expanded(loc) => loc.clone(),
        }
    }
}

#[allow(dead_code)]
impl Location {
    /// Returns the path if this is a Path location
    pub fn as_path(&self) -> Option<&PathBuf> {
        match self {
            Location::Path { path } => Some(path),
            Location::Git { .. } => None,
        }
    }

    /// Returns the git location if this is a Git location
    pub fn as_git(&self) -> Option<GitLocation> {
        match self {
            Location::Path { .. } => None,
            Location::Git { git } => Some(git.to_location()),
        }
    }

    /// Returns the path, or an error message if this is a Git location
    pub fn require_path(&self) -> Result<&PathBuf, String> {
        match self {
            Location::Path { path } => Ok(path),
            Location::Git { git } => Err(match git {
                GitValue::Compact(s) => s.clone(),
                GitValue::Expanded(loc) => loc.url.clone(),
            }),
        }
    }

    /// Check if this location is a git URL
    pub fn is_git(&self) -> bool {
        matches!(self, Location::Git { .. })
    }

    /// Check if this location is a local path
    pub fn is_path(&self) -> bool {
        matches!(self, Location::Path { .. })
    }

    /// Resolve a relative path against a base path
    pub fn resolve_path(&self, base_path: &std::path::Path) -> Option<PathBuf> {
        match self {
            Location::Path { path } => {
                if path.is_relative() {
                    Some(base_path.join(path))
                } else {
                    Some(path.clone())
                }
            }
            Location::Git { .. } => None,
        }
    }
}

// =============================================================================
// Root and child configs
// =============================================================================

/// Root site configuration - defines the full documentation site.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RootConfig {
    pub site: SiteConfig,
    pub sources: Vec<SourceConfig>,
    pub theme: ThemeConfig,
    #[serde(default)]
    pub markdown: MarkdownConfig,
    /// Development-specific settings (watch mode, etc.)
    #[serde(default)]
    pub dev: DevConfig,
}

/// Child configuration - used in source repos to point back to the parent site.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChildConfig {
    /// Which source in the parent config this repo corresponds to
    pub name: String,
    /// Location of the parent site's undox.yaml
    pub parent: Location,
    /// Path to the content directory
    pub content: Location,
    /// Optional overrides for root config settings
    #[serde(default)]
    pub overrides: Option<RootConfigOverrides>,
    /// Development-specific settings
    pub dev: Option<DevConfig>,
}

/// Partial root config that can be used as overrides in child config.
/// Runtime validation will ensure only allowed fields are set.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RootConfigOverrides {
    /// Site config overrides
    #[serde(default)]
    pub site: Option<SiteConfigOverrides>,
    /// Navigation override for this source
    #[serde(default)]
    pub nav: Option<NavConfig>,
    // Note: sources, theme, markdown are intentionally not included
    // and will be validated at runtime if present
}

/// Partial site config for overrides
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SiteConfigOverrides {
    pub repository: Option<String>,
    pub edit_path: Option<String>,
}

// =============================================================================
// Site configuration
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SiteConfig {
    pub name: String,
    pub url: Option<String>,
    #[serde(default = "default_output")]
    pub output: PathBuf,
    /// Path to the site favicon (relative to config file)
    pub favicon: Option<String>,
    /// Repository URL for "edit on GitHub" links
    pub repository: Option<String>,
    /// Path within the repo where docs live (for edit links)
    pub edit_path: Option<String>,
}

fn default_output() -> PathBuf {
    PathBuf::from("_site")
}

// =============================================================================
// Theme configuration
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeConfig {
    /// Location of the theme (path or git)
    pub location: Location,
    /// Arbitrary settings passed to templates as `theme.*`
    #[serde(default)]
    pub settings: serde_json::Value,
}

// =============================================================================
// Source configuration
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceConfig {
    /// Unique identifier for this source
    pub name: String,
    /// Display title for source tabs (defaults to name if not set)
    pub title: Option<String>,
    /// URL path prefix (e.g., "/cli" -> site.com/cli/...)
    pub url_prefix: Option<String>,
    /// Navigation structure (auto-generated if omitted)
    pub nav: Option<NavConfig>,
    /// Where the content comes from
    #[serde(flatten)]
    pub location: SourceLocation,
}

/// Where a source's content is located.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SourceLocation {
    /// External source (local or remote) with its own undox.yaml config
    Remote { location: Location },
    /// Local content directory (content belongs to the root config)
    Local { local: Location },
}

#[allow(dead_code)]
impl SourceLocation {
    /// Check if this is a local (inline) source
    pub fn is_local(&self) -> bool {
        matches!(self, SourceLocation::Local { .. })
    }

    /// Check if this is a remote (external) source
    pub fn is_remote(&self) -> bool {
        matches!(self, SourceLocation::Remote { .. })
    }

    /// Get the inner location
    pub fn location(&self) -> &Location {
        match self {
            SourceLocation::Remote { location } => location,
            SourceLocation::Local { local } => local,
        }
    }
}

// =============================================================================
// Markdown configuration
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarkdownConfig {
    /// Extensions to enable for markdown processing
    #[serde(default = "default_markdown_extensions")]
    pub extensions: Vec<String>,
}

fn default_markdown_extensions() -> Vec<String> {
    vec![
        "definition_lists".to_string(),
        "footnotes".to_string(),
        "gfm".to_string(),
        "heading_attributes".to_string(),
        "strikethrough".to_string(),
        "tables".to_string(),
        "tasklists".to_string(),
    ]
}

impl Default for MarkdownConfig {
    fn default() -> Self {
        Self {
            extensions: default_markdown_extensions(),
        }
    }
}

// =============================================================================
// Navigation configuration
// =============================================================================

/// Navigation structure for a source's sidebar.
pub type NavConfig = Vec<NavItem>;

/// A navigation item in the sidebar.
///
/// Supports multiple formats in YAML:
/// ```yaml
/// nav:
///   - installation.md                    # Simple path, title from filename
///   - Getting Started: getting-started.md  # Explicit title
///   - section: Commands                  # Section with nested items
///     items:
///       - sync.md
///       - Search: search.md
///   - guides/                            # Auto-expand directory
///   - path: configuration.md             # Link with children
///     children:
///       - configuration/root.md
///       - configuration/sub.md
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum NavItem {
    /// A section with nested items (no link, just a heading)
    /// Must come first so serde tries it before the map variant
    Section {
        section: String,
        items: Vec<NavItem>,
    },
    /// A link with nested children
    /// Use this when a page has sub-pages underneath it
    LinkWithChildren {
        path: String,
        #[serde(default)]
        title: Option<String>,
        children: Vec<NavItem>,
    },
    /// A titled page: { "Display Title": "path/to/file.md" }
    Titled(std::collections::HashMap<String, String>),
    /// A simple path: "file.md" or "dir/" for auto-expand
    Path(String),
}

// =============================================================================
// Development configuration
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DevConfig {
    /// Override parent location for local development
    pub parent: Option<Location>,
    /// File watching configuration
    #[serde(default)]
    pub watch: WatchConfig,
    /// Enable live reload in the browser when files change (default: true)
    #[serde(default = "default_live_reload")]
    pub live_reload: bool,
}

impl Default for DevConfig {
    fn default() -> Self {
        Self {
            parent: None,
            watch: WatchConfig::default(),
            live_reload: true,
        }
    }
}

fn default_live_reload() -> bool {
    true
}

/// Configuration for file watching during development.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchConfig {
    /// Use polling-based watcher instead of native file system events.
    /// Useful for network filesystems, Docker volumes, or other situations
    /// where native events are unreliable.
    #[serde(default)]
    pub poll: bool,
    /// Poll interval in milliseconds (only used if poll=true).
    #[serde(default = "default_poll_interval_ms")]
    pub poll_interval_ms: u64,
    /// Debounce timeout in milliseconds.
    /// Changes within this window are batched together.
    #[serde(default = "default_debounce_ms")]
    pub debounce_ms: u64,
}

fn default_poll_interval_ms() -> u64 {
    500
}

fn default_debounce_ms() -> u64 {
    100
}

impl Default for WatchConfig {
    fn default() -> Self {
        Self {
            poll: false,
            poll_interval_ms: default_poll_interval_ms(),
            debounce_ms: default_debounce_ms(),
        }
    }
}
