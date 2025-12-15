use std::path::{Path, PathBuf};

use serde::{Deserialize, Deserializer, Serialize};

#[derive(thiserror::Error, Debug)]
pub enum ConfigError {
    #[error("failed to encode config file path as a unicode string: {0}")]
    EncodePath(PathBuf),

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
// Top-level config: either a root site config or a child repo config
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
        let value = serde_json::Value::deserialize(deserializer)?;

        let obj = value.as_object().ok_or_else(|| {
            D::Error::custom("config must be a YAML/JSON object, not a scalar or array")
        })?;

        // Determine which variant based on distinguishing fields
        let has_site = obj.contains_key("site");
        let has_parent = obj.contains_key("parent");

        match (has_site, has_parent) {
            (true, false) => {
                // Root config
                serde_json::from_value::<RootConfig>(value)
                    .map(Config::Root)
                    .map_err(|e| D::Error::custom(format_root_error(e)))
            }
            (false, true) => {
                // Child config
                serde_json::from_value::<ChildConfig>(value)
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
fn format_root_error(e: serde_json::Error) -> String {
    let msg = e.to_string();

    // serde_json errors use "at line X column Y" for syntax errors
    // and path info for structural errors
    // Check for common issues and provide specific guidance
    if msg.contains("missing field `sources`") {
        return "invalid config: 'sources' list is required\n\nExample:\n  sources:\n    - name: docs\n      path: ./content".to_string();
    }
    if msg.contains("missing field `name`") {
        // Could be site.name or source[].name - provide both possibilities
        return "invalid config: missing required 'name' field (check 'site.name' and each source's 'name')".to_string();
    }
    if msg.contains("missing field `path`") {
        return "invalid config: each source must have a 'path' field".to_string();
    }

    format!("invalid config: {msg}")
}

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

/// Result of resolving a child config, containing the synthetic root config
/// and the path to the parent repository (for theme resolution).
pub struct ResolvedChildConfig {
    /// The synthetic root config with just this source
    pub config: RootConfig,
    /// Path to the parent repository (for theme resolution)
    pub parent_path: PathBuf,
}

impl ChildConfig {
    /// Resolve this child config by fetching the parent and creating a synthetic RootConfig.
    ///
    /// The resulting config will only include this child's source, with the local
    /// content path instead of whatever was specified in the parent.
    pub fn resolve(
        &self,
        child_base_path: &Path,
        cache_dir: &Path,
    ) -> Result<ResolvedChildConfig, ConfigError> {
        use crate::git::GitFetcher;

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
                if let SourceLocation::ContentPath { content_path } = &source.location {
                    if content_path.is_relative() {
                        source.location = SourceLocation::ContentPath {
                            content_path: parent_path.join(content_path),
                        };
                    }
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

impl NavItem {
    /// Returns true if this is a directory auto-expand item (path ends with /).
    pub fn is_directory(&self) -> bool {
        matches!(self, NavItem::Path(p) if p.ends_with('/'))
    }

    /// Get the file path for this nav item, if it's a page.
    pub fn page_path(&self) -> Option<&str> {
        match self {
            NavItem::Path(p) if !p.ends_with('/') => Some(p),
            NavItem::Titled(map) => map.values().next().map(|s| s.as_str()),
            _ => None,
        }
    }

    /// Get the display title for this nav item.
    /// Returns None for sections and directories.
    pub fn title(&self) -> Option<String> {
        match self {
            NavItem::Path(p) if !p.ends_with('/') => {
                // Derive title from filename
                std::path::Path::new(p)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .map(|s| title_from_slug(s))
            }
            NavItem::Titled(map) => map.keys().next().cloned(),
            _ => None,
        }
    }
}

/// Convert a filename slug to a display title.
/// "getting-started" -> "Getting started"
/// "installation" -> "Installation"
fn title_from_slug(s: &str) -> String {
    let mut result = s.replace(['-', '_'], " ");
    if let Some(first) = result.get_mut(0..1) {
        first.make_ascii_uppercase();
    }
    result
}

// =============================================================================
// Child config types
// =============================================================================

/// Settings that can be overridden in a child config.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SourceOverrides {
    pub repo_url: Option<String>,
    pub edit_path: Option<String>,
    pub nav: Option<NavConfig>,
}

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

// =============================================================================
// Config loading
// =============================================================================

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
    async fn load_from_file(path: &Path) -> Result<Self, ConfigError> {
        let path_str = path
            .as_os_str()
            .to_str()
            .ok_or_else(|| ConfigError::EncodePath(path.to_path_buf()))?;

        Ok(config::Config::builder()
            .add_source(config::File::new(path_str, config::FileFormat::Yaml))
            .build()?
            .try_deserialize::<Config>()?)
    }

    /// Returns the root config, or an error if this is a child config.
    pub fn as_root(&self) -> Option<&RootConfig> {
        match self {
            Config::Root(root) => Some(root),
            Config::Child(_) => None,
        }
    }

    /// Returns the child config, or an error if this is a root config.
    pub fn as_child(&self) -> Option<&ChildConfig> {
        match self {
            Config::Root(_) => None,
            Config::Child(child) => Some(child),
        }
    }
}
