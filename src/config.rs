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
}

/// Child configuration - used in source repos to point back to the parent site.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChildConfig {
    /// URL or local path to the parent site's undox.yaml
    pub parent: String,
    /// Which source in the parent config this repo corresponds to
    pub source: String,
    /// Optional overrides for source-specific settings
    #[serde(default)]
    pub overrides: SourceOverrides,
    /// Development-specific settings
    pub dev: Option<DevConfig>,
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
    /// Local filesystem path
    Local { path: PathBuf },
    /// Remote git repository
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
