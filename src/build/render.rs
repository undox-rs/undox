use std::path::Path;

use serde::Serialize;
use tera::{Context, Tera};

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
}

impl Renderer {
    /// Create a new renderer loading templates from the given theme directory.
    pub fn new(theme_path: &Path) -> Result<Self, RenderError> {
        let templates_path = theme_path.join("templates");
        if !templates_path.exists() {
            return Err(RenderError::ThemeNotFound(
                theme_path.display().to_string(),
            ));
        }

        let glob = templates_path.join("**/*.html");
        let glob_str = glob.to_string_lossy();
        let tera = Tera::new(&glob_str)?;

        Ok(Self { tera })
    }

    /// Render a page with the given context.
    pub fn render_page(&self, context: &PageContext) -> Result<String, RenderError> {
        let mut tera_context = Context::new();
        tera_context.insert("site", &context.site);
        tera_context.insert("page", &context.page);
        tera_context.insert("content", &context.content);
        tera_context.insert("nav", &context.nav);
        tera_context.insert("toc", &context.toc);
        tera_context.insert("theme", &context.theme);

        Ok(self.tera.render("page.html", &tera_context)?)
    }
}

/// Context passed to page templates.
#[derive(Debug, Serialize)]
pub struct PageContext {
    pub site: SiteContext,
    pub page: PageInfo,
    pub content: String,
    pub nav: Vec<NavSection>,
    /// Table of contents for the current page
    pub toc: Vec<TocEntry>,
    /// Theme settings from config, accessible as `theme.*` in templates
    pub theme: serde_json::Value,
}

/// Site-level information.
#[derive(Debug, Clone, Serialize)]
pub struct SiteContext {
    pub name: String,
    pub url: Option<String>,
}

/// Information about the current page.
#[derive(Debug, Serialize)]
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
