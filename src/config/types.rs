//! Configuration type definitions.
//!
//! This module contains all the data structures used in undox configuration files.
//! These types are pure data - no I/O or complex logic.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

// =============================================================================
// Root and child configs
// =============================================================================

/// Root site configuration - defines the full documentation site.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RootConfig {
    pub site: SiteConfig,
    pub sources: Vec<SourceConfig>,
    #[serde(default)]
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
    /// URL or local path to the parent site's undox.yaml
    pub parent: String,
    /// Branch, tag, or commit to checkout when parent is a git URL
    #[serde(rename = "ref")]
    pub parent_ref: Option<String>,
    /// Which source in the parent config this repo corresponds to
    pub source: String,
    /// Optional overrides for source-specific settings
    #[serde(default)]
    pub overrides: SourceOverrides,
    /// Development-specific settings
    pub dev: Option<DevConfig>,
}

/// Settings that can be overridden in a child config.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SourceOverrides {
    pub repo_url: Option<String>,
    pub edit_path: Option<String>,
    pub nav: Option<NavConfig>,
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
}

fn default_output() -> PathBuf {
    PathBuf::from("_site")
}

// =============================================================================
// Theme configuration
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeConfig {
    /// Theme name (built-in) or path to custom theme
    #[serde(default = "default_theme_name")]
    pub name: String,
    /// Arbitrary settings passed to templates as `theme.*`
    #[serde(default)]
    pub settings: serde_json::Value,
}

fn default_theme_name() -> String {
    "default".to_string()
}

impl Default for ThemeConfig {
    fn default() -> Self {
        Self {
            name: default_theme_name(),
            settings: serde_json::Value::Object(Default::default()),
        }
    }
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
    /// Repository URL for "edit on GitHub" links
    pub repo_url: Option<String>,
    /// Path within the repo where docs live (for edit links)
    pub edit_path: Option<String>,
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
    /// Local content directory (content belongs to the root config)
    ContentPath { content_path: PathBuf },
    /// Local sub-docs directory (has its own undox.yaml, like git but local)
    LocalPath { path: PathBuf },
    /// Remote git repository (sub-docs with their own config)
    Git { git: GitConfig },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitConfig {
    /// Repository URL
    pub url: String,
    /// Branch, tag, or commit (defaults to default branch)
    #[serde(rename = "ref")]
    pub git_ref: Option<String>,
    /// Path within the repository to the docs
    pub path: Option<PathBuf>,
    /// Whether to use sparse checkout
    #[serde(default)]
    pub sparse: bool,
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
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum NavItem {
    /// A section with nested items
    /// Must come first so serde tries it before the map variant
    Section {
        section: String,
        items: Vec<NavItem>,
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
    /// Override parent path for local development
    pub parent: Option<String>,
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
