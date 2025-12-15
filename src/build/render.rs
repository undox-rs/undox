use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::Serialize;
use tera::{Context, Tera, Value};

#[derive(thiserror::Error, Debug)]
pub enum RenderError {
    #[error("template error: {0}")]
    Template(#[from] tera::Error),

    #[error("theme not found: {0}")]
    ThemeNotFound(String),
}

/// The template renderer, wrapping Tera.
pub struct Renderer {
    tera: Tera,
    #[allow(dead_code)]
    theme_path: PathBuf,
}

impl Renderer {
    /// Create a new renderer loading templates from the given theme directory.
    pub fn new(theme_path: &Path) -> Result<Self, RenderError> {
        let templates_path = theme_path.join("templates");
        if !templates_path.exists() {
            return Err(RenderError::ThemeNotFound(theme_path.display().to_string()));
        }

        let glob = templates_path.join("**/*.html");
        let glob_str = glob.to_string_lossy();
        let mut tera = Tera::new(&glob_str)?;

        // Register the icon() function for inlining SVG icons
        // Usage: {{ icon(name="search") }} or {{ icon(name="menu", class="nav-icon", size=20) }}
        let icons_path = Arc::new(theme_path.join("static/icons"));
        tera.register_function("icon", MakeIconFunction(icons_path));

        Ok(Self {
            tera,
            theme_path: theme_path.to_path_buf(),
        })
    }

    /// Render a page with the given context.
    pub fn render_page(&self, context: &PageContext) -> Result<String, RenderError> {
        let mut tera_context = Context::new();
        tera_context.insert("site", &context.site);
        tera_context.insert("page", &context.page);
        tera_context.insert("content", &context.content);
        tera_context.insert("nav", &context.nav);
        tera_context.insert("sources", &context.sources);
        tera_context.insert("toc", &context.toc);
        tera_context.insert("theme", &context.theme);
        tera_context.insert("undox", &context.undox);

        Ok(self.tera.render("page.html", &tera_context)?)
    }

    /// Render raw content (markdown) through Tera before markdown processing.
    /// This allows markdown files to use Tera syntax like macros, loops, and variables.
    ///
    /// Unlike `render_str`, this method gives the content access to macros defined
    /// in template files by dynamically adding the content as a template.
    pub fn render_content(
        &mut self,
        content: &str,
        context: &ContentRenderContext,
    ) -> Result<String, RenderError> {
        let mut tera_context = Context::new();
        tera_context.insert("site", &context.site);
        tera_context.insert("page", &context.page);
        tera_context.insert("theme", &context.theme);
        tera_context.insert("undox", &context.undox);

        // Prepend import for macros so content can use them as `macros::name(...)`
        // The macros.html file should exist in the theme's templates directory
        let content_with_imports = format!("{{% import \"macros.html\" as macros %}}\n{}", content);

        // Add the content as a temporary template so it has access to macros
        // defined in other template files
        const TEMP_TEMPLATE_NAME: &str = "__content_render__";
        self.tera
            .add_raw_template(TEMP_TEMPLATE_NAME, &content_with_imports)?;

        let result = self.tera.render(TEMP_TEMPLATE_NAME, &tera_context);

        // Clean up the temporary template
        self.tera.templates.remove(TEMP_TEMPLATE_NAME);

        Ok(result?)
    }
}

/// Context available during content (markdown) rendering.
/// This is a subset of PageContext since nav/toc aren't available yet.
#[derive(Debug, Serialize)]
pub struct ContentRenderContext {
    pub site: SiteContext,
    pub page: PageInfo,
    pub theme: serde_json::Value,
    pub undox: UndoxContext,
}

/// Context passed to page templates.
#[derive(Debug, Serialize)]
pub struct PageContext {
    pub site: SiteContext,
    pub page: PageInfo,
    pub content: String,
    /// Navigation for the current source only
    pub nav: Vec<NavSection>,
    /// All sources/projects for top-level tabs
    pub sources: Vec<SourceTab>,
    /// Table of contents for the current page
    pub toc: Vec<TocEntry>,
    /// Theme settings from config, accessible as `theme.*` in templates
    pub theme: serde_json::Value,
    /// Undox-specific context (dev mode, version, etc.)
    pub undox: UndoxContext,
}

/// Information about a source/project for top-level navigation tabs.
#[derive(Debug, Clone, Serialize)]
pub struct SourceTab {
    /// Display name for the source (uses title if set, otherwise name)
    pub name: String,
    /// Source identifier (the config name, used for matching)
    #[serde(skip_serializing)]
    pub source_id: String,
    /// URL to the source's root page
    pub url: String,
    /// Whether this is the currently active source
    pub is_current: bool,
    /// Whether this is a top-level source (url_prefix is "/")
    pub is_top_level: bool,
}

/// Site-level information.
#[derive(Debug, Clone, Serialize)]
pub struct SiteContext {
    pub name: String,
    pub url: Option<String>,
    pub favicon: Option<String>,
}

/// Information about the current page.
#[derive(Debug, Clone, Serialize)]
pub struct PageInfo {
    pub title: String,
    pub url: String,
    pub description: Option<String>,
    /// Custom front matter fields (flattened to top level, e.g., `page.author`)
    #[serde(flatten)]
    pub extra: std::collections::HashMap<String, serde_yaml::Value>,
}

/// A navigation section (group of links).
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum NavSection {
    /// A section with a title and nested items
    Section {
        section: String,
        items: Vec<NavLink>,
    },
    /// A standalone link (no section header)
    Link(NavLink),
}

/// A single navigation link.
#[derive(Debug, Clone, Serialize)]
pub struct NavLink {
    pub title: String,
    pub url: String,
}

/// A table of contents entry for the current page.
#[derive(Debug, Clone, Serialize)]
pub struct TocEntry {
    /// The heading text
    pub text: String,
    /// The heading id (for anchor links)
    pub id: String,
    /// The heading level (1-6)
    pub level: u8,
}

/// Undox-specific context available in templates.
#[derive(Debug, Clone, Default, Serialize)]
pub struct UndoxContext {
    /// Whether we're in development/serve mode
    pub dev: bool,
    /// Whether live reload is enabled (only true in dev mode with live_reload config enabled)
    pub live_reload: bool,
    /// The undox version
    pub version: String,
}

struct MakeIconFunction(Arc<PathBuf>);

impl tera::Function for MakeIconFunction {
    fn call(&self, args: &std::collections::HashMap<String, Value>) -> tera::Result<Value> {
        Self::make_icon(&self.0, args)
    }

    fn is_safe(&self) -> bool {
        true
    }
}

impl MakeIconFunction {
    /// Create the icon() Tera function for inlining SVG icons.
    ///
    /// Usage in templates:
    ///   {{ icon(name="search") }}
    ///   {{ icon(name="menu", class="nav-icon") }}
    ///   {{ icon(name="star", size=20) }}
    fn make_icon(
        icons_path: &Arc<PathBuf>,
        args: &std::collections::HashMap<String, Value>,
    ) -> tera::Result<Value> {
        // Get required "name" argument
        let name = args
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| tera::Error::msg("icon() requires a 'name' argument"))?;

        // Get optional "class" argument
        let class = args.get("class").and_then(|v| v.as_str());

        // Get optional "size" argument (defaults to 24)
        let size = args.get("size").and_then(|v| v.as_i64()).map(|s| s as u32);

        // Read the SVG file
        let svg_path = icons_path.join(format!("{}.svg", name));
        let svg_content = match std::fs::read_to_string(&svg_path) {
            Ok(content) => content,
            Err(_) => {
                eprintln!("Warning: icon '{}' not found at {:?}", name, svg_path);
                return Ok(Value::String(String::new()));
            }
        };

        // Modify the SVG if needed
        let mut svg = svg_content;

        // Add class attribute if specified
        if let Some(class_value) = class {
            svg = svg.replacen("<svg", &format!("<svg class=\"{}\"", class_value), 1);
        }

        // Update size if specified
        if let Some(size_value) = size {
            svg = svg
                .replace("width=\"24\"", &format!("width=\"{}\"", size_value))
                .replace("height=\"24\"", &format!("height=\"{}\"", size_value));
        }

        Ok(Value::String(svg))
    }
}
