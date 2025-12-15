//! Pluggable content format system.
//!
//! This module provides a registry of content formats that can render
//! different file types to HTML. The default format is Markdown, but
//! additional formats (AsciiDoc, reStructuredText, etc.) can be added.
//!
//! # Adding a New Format
//!
//! ```ignore
//! struct AsciidocFormat;
//!
//! impl ContentFormat for AsciidocFormat {
//!     fn name(&self) -> &'static str { "asciidoc" }
//!     fn extensions(&self) -> &[&'static str] { &["adoc", "asciidoc"] }
//!     fn render(&self, content: &str, ctx: &FormatContext) -> Result<FormatOutput, FormatError> {
//!         // Convert AsciiDoc to HTML...
//!     }
//! }
//!
//! registry.register(AsciidocFormat);
//! ```

use std::path::Path;

use crate::build::highlight::SyntaxHighlighter;
use crate::build::markdown::render_markdown;
use crate::build::render::TocEntry;
use crate::config::MarkdownConfig;

/// Output from rendering a content format.
#[derive(Debug, Clone)]
pub struct FormatOutput {
    /// The rendered HTML content.
    pub html: String,
    /// Table of contents extracted from headings.
    pub toc: Vec<TocEntry>,
}

/// Context available during format rendering.
pub struct FormatContext<'a> {
    /// Syntax highlighter for code blocks.
    pub highlighter: &'a SyntaxHighlighter,
    /// Markdown-specific configuration (also used by other formats for consistency).
    pub markdown_config: &'a MarkdownConfig,
}

/// Error during format rendering.
#[derive(thiserror::Error, Debug)]
pub enum FormatError {
    /// Generic render error for custom formats.
    #[allow(dead_code)]
    #[error("render error: {0}")]
    Render(String),

    #[error("markdown error: {0}")]
    Markdown(#[from] crate::build::markdown::MarkdownError),
}

/// A content format that can render files to HTML.
///
/// Implement this trait to add support for new file formats like
/// AsciiDoc, reStructuredText, or custom formats.
#[allow(dead_code)]
pub trait ContentFormat: Send + Sync {
    /// The name of this format (e.g., "markdown", "asciidoc").
    fn name(&self) -> &'static str;

    /// File extensions this format handles (lowercase, without dot).
    ///
    /// Example: `&["md", "markdown"]` for Markdown.
    fn extensions(&self) -> &[&'static str];

    /// Render content to HTML.
    ///
    /// Returns the HTML output and extracted table of contents.
    fn render(&self, content: &str, ctx: &FormatContext) -> Result<FormatOutput, FormatError>;
}

/// Markdown format implementation.
///
/// Uses pulldown-cmark for parsing and the syntax highlighter for code blocks.
pub struct MarkdownFormat;

impl ContentFormat for MarkdownFormat {
    fn name(&self) -> &'static str {
        "markdown"
    }

    fn extensions(&self) -> &[&'static str] {
        &["md", "markdown"]
    }

    fn render(&self, content: &str, ctx: &FormatContext) -> Result<FormatOutput, FormatError> {
        let output = render_markdown(content, ctx.highlighter, ctx.markdown_config)?;
        Ok(FormatOutput {
            html: output.html,
            toc: output.toc,
        })
    }
}

/// Registry of content formats.
///
/// The registry determines which format to use based on file extension
/// and provides access to format implementations for rendering.
pub struct FormatRegistry {
    formats: Vec<Box<dyn ContentFormat>>,
}

impl FormatRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            formats: Vec::new(),
        }
    }

    /// Create a registry with the default formats (Markdown).
    pub fn with_defaults() -> Self {
        let mut registry = Self::new();
        registry.register(MarkdownFormat);
        registry
    }

    /// Register a new format.
    ///
    /// Later registrations take precedence for overlapping extensions.
    pub fn register<F: ContentFormat + 'static>(&mut self, format: F) {
        self.formats.push(Box::new(format));
    }

    /// Find the format for a file extension.
    ///
    /// Returns `None` if no format handles this extension.
    pub fn for_extension(&self, ext: &str) -> Option<&dyn ContentFormat> {
        let ext_lower = ext.to_lowercase();
        // Search in reverse so later registrations take precedence
        self.formats
            .iter()
            .rev()
            .find(|f| f.extensions().iter().any(|e| *e == ext_lower))
            .map(|f| f.as_ref())
    }

    /// Find the format for a file path based on its extension.
    pub fn for_path(&self, path: &Path) -> Option<&dyn ContentFormat> {
        path.extension()
            .and_then(|ext| ext.to_str())
            .and_then(|ext| self.for_extension(ext))
    }

    /// Check if a path is a document (has a registered format).
    pub fn is_document(&self, path: &Path) -> bool {
        self.for_path(path).is_some()
    }

    /// Get all registered extensions.
    #[allow(dead_code)]
    pub fn all_extensions(&self) -> Vec<&'static str> {
        self.formats
            .iter()
            .flat_map(|f| f.extensions().iter().copied())
            .collect()
    }
}

impl Default for FormatRegistry {
    fn default() -> Self {
        Self::with_defaults()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_default_formats() {
        let registry = FormatRegistry::with_defaults();

        assert!(registry.for_extension("md").is_some());
        assert!(registry.for_extension("markdown").is_some());
        assert!(registry.for_extension("MD").is_some()); // Case insensitive
        assert!(registry.for_extension("txt").is_none());
    }

    #[test]
    fn test_registry_is_document() {
        let registry = FormatRegistry::with_defaults();

        assert!(registry.is_document(Path::new("docs/intro.md")));
        assert!(registry.is_document(Path::new("guide.markdown")));
        assert!(!registry.is_document(Path::new("image.png")));
        assert!(!registry.is_document(Path::new("style.css")));
    }

    #[test]
    fn test_registry_all_extensions() {
        let registry = FormatRegistry::with_defaults();
        let exts = registry.all_extensions();

        assert!(exts.contains(&"md"));
        assert!(exts.contains(&"markdown"));
    }

    struct MockFormat;
    impl ContentFormat for MockFormat {
        fn name(&self) -> &'static str {
            "mock"
        }
        fn extensions(&self) -> &[&'static str] {
            &["mock", "test"]
        }
        fn render(
            &self,
            _content: &str,
            _ctx: &FormatContext,
        ) -> Result<FormatOutput, FormatError> {
            Ok(FormatOutput {
                html: "<p>mock</p>".to_string(),
                toc: vec![],
            })
        }
    }

    #[test]
    fn test_registry_custom_format() {
        let mut registry = FormatRegistry::with_defaults();
        registry.register(MockFormat);

        assert!(registry.for_extension("mock").is_some());
        assert!(registry.for_extension("test").is_some());
        assert_eq!(registry.for_extension("mock").unwrap().name(), "mock");
    }
}
