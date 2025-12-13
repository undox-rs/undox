use std::path::PathBuf;

use serde::{Deserialize, Serialize};

// =============================================================================
// Content items (documents and static files)
// =============================================================================

/// A content item discovered in a source.
/// Can be either a document (markdown) or a static file (images, etc.).
#[derive(Debug, Clone)]
pub enum ContentItem {
    /// A markdown document that will be rendered to HTML
    Document(Document),
    /// A static file that will be copied (possibly transformed)
    Static(StaticFile),
}

impl ContentItem {
    /// Get the source name this content belongs to.
    pub fn source_name(&self) -> &str {
        match self {
            ContentItem::Document(doc) => &doc.source_name,
            ContentItem::Static(file) => &file.source_name,
        }
    }

    /// Get the source-relative path.
    pub fn source_path(&self) -> &PathBuf {
        match self {
            ContentItem::Document(doc) => &doc.source_path,
            ContentItem::Static(file) => &file.source_path,
        }
    }

    /// Get the output path.
    pub fn output_path(&self) -> &str {
        match self {
            ContentItem::Document(doc) => &doc.url_path,
            ContentItem::Static(file) => &file.output_path,
        }
    }
}

// =============================================================================
// Static files
// =============================================================================

/// A static file (image, CSS, JS, etc.) that gets copied to output.
///
/// Static files may optionally go through transformers (e.g., image optimization),
/// but by default are just copied as-is.
#[derive(Debug, Clone)]
pub struct StaticFile {
    /// Which source this file belongs to
    pub source_name: String,
    /// Path relative to the source root (e.g., "images/screenshot.png")
    pub source_path: PathBuf,
    /// The output path this file will be written to (e.g., "/cli/images/screenshot.png")
    pub output_path: String,
    /// The file's MIME type, if known
    pub mime_type: Option<String>,
}

impl StaticFile {
    /// Create a new static file.
    pub fn new(source_name: String, source_path: PathBuf, output_path: String) -> Self {
        let mime_type = mime_from_path(&source_path);
        Self {
            source_name,
            source_path,
            output_path,
            mime_type,
        }
    }

    /// Returns true if this is an image file.
    pub fn is_image(&self) -> bool {
        self.mime_type
            .as_ref()
            .map(|m| m.starts_with("image/"))
            .unwrap_or(false)
    }
}

/// Guess MIME type from file extension.
fn mime_from_path(path: &PathBuf) -> Option<String> {
    let ext = path.extension()?.to_str()?;
    let mime = match ext.to_lowercase().as_str() {
        // Images
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        "webp" => "image/webp",
        "ico" => "image/x-icon",
        // Web assets
        "css" => "text/css",
        "js" => "application/javascript",
        "json" => "application/json",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "eot" => "application/vnd.ms-fontobject",
        // Documents
        "pdf" => "application/pdf",
        "xml" => "application/xml",
        "txt" => "text/plain",
        _ => return None,
    };
    Some(mime.to_string())
}

// =============================================================================
// Documents
// =============================================================================

/// A document flowing through the build pipeline.
///
/// Documents progress through stages:
/// 1. Discovered: file path known, content not yet read
/// 2. Loaded: raw content read from disk
/// 3. Parsed: front matter extracted, content ready for transformation
/// 4. Rendered: HTML output generated
#[derive(Debug, Clone)]
pub struct Document {
    /// Which source this document belongs to
    pub source_name: String,
    /// Path relative to the source root (e.g., "getting-started/installation.md")
    pub source_path: PathBuf,
    /// The URL path this document will be served at (e.g., "/cli/getting-started/installation")
    pub url_path: String,
    /// Front matter metadata
    pub front_matter: FrontMatter,
    /// The document content (progresses through stages)
    pub content: DocumentContent,
}

/// Front matter metadata parsed from the document.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FrontMatter {
    /// Page title (can override filename-derived title)
    pub title: Option<String>,
    /// Page description for SEO/previews
    pub description: Option<String>,
    /// Hide from navigation
    #[serde(default)]
    pub hidden: bool,
    /// Custom slug override
    pub slug: Option<String>,
    /// Additional arbitrary metadata (available in templates at top level, e.g., `page.author`)
    #[serde(flatten)]
    pub extra: std::collections::HashMap<String, serde_yaml::Value>,
}

/// Result of parsing front matter from markdown content.
#[derive(Debug)]
pub struct ParsedContent {
    /// The parsed front matter (empty if none found)
    pub front_matter: FrontMatter,
    /// The markdown content without the front matter block
    pub content: String,
}

/// Parse front matter from markdown content.
///
/// Front matter is a YAML block delimited by `---` at the start of the file:
///
/// ```markdown
/// ---
/// title: My Page
/// description: A description
/// custom_field: custom value
/// ---
///
/// # Content starts here
/// ```
///
/// Returns the parsed front matter and the remaining content.
pub fn parse_front_matter(content: &str) -> ParsedContent {
    let content = content.trim_start();

    // Check if content starts with front matter delimiter
    if !content.starts_with("---") {
        return ParsedContent {
            front_matter: FrontMatter::default(),
            content: content.to_string(),
        };
    }

    // Find the closing delimiter
    let after_opening = &content[3..];
    let closing_pos = after_opening.find("\n---");

    let Some(closing_pos) = closing_pos else {
        // No closing delimiter found, treat entire content as markdown
        return ParsedContent {
            front_matter: FrontMatter::default(),
            content: content.to_string(),
        };
    };

    // Extract the YAML content (skip the opening newline if present)
    let yaml_content = after_opening[..closing_pos].trim_start_matches('\n');

    // Extract the markdown content (skip the closing delimiter and newline)
    let markdown_start = 3 + closing_pos + 4; // "---" + yaml + "\n---"
    let markdown_content = if markdown_start < content.len() {
        content[markdown_start..].trim_start_matches('\n').to_string()
    } else {
        String::new()
    };

    // Parse the YAML
    let front_matter = match serde_yaml::from_str(yaml_content) {
        Ok(fm) => fm,
        Err(e) => {
            // Log warning but continue with default front matter
            eprintln!("Warning: Failed to parse front matter: {}", e);
            FrontMatter::default()
        }
    };

    ParsedContent {
        front_matter,
        content: markdown_content,
    }
}

/// The content of a document at various stages of processing.
#[derive(Debug, Clone)]
pub enum DocumentContent {
    /// Just discovered, content not loaded
    Discovered,
    /// Raw content loaded from disk
    Raw(String),
    /// Parsed but not yet transformed
    /// (markdown without front matter, ready for transformation)
    Parsed(String),
    /// Rendered to HTML
    Rendered(String),
}

impl Document {
    /// Create a new discovered document (content not yet loaded).
    pub fn discovered(source_name: String, source_path: PathBuf, url_path: String) -> Self {
        Self {
            source_name,
            source_path,
            url_path,
            front_matter: FrontMatter::default(),
            content: DocumentContent::Discovered,
        }
    }

    /// Get the document title, falling back to filename if not in front matter.
    pub fn title(&self) -> String {
        self.front_matter.title.clone().unwrap_or_else(|| {
            self.source_path
                .file_stem()
                .and_then(|s| s.to_str())
                .map(|s| title_case(s))
                .unwrap_or_else(|| "Untitled".to_string())
        })
    }

    /// Returns true if this document has been loaded from disk.
    pub fn is_loaded(&self) -> bool {
        !matches!(self.content, DocumentContent::Discovered)
    }

    /// Returns true if this document has been rendered to HTML.
    pub fn is_rendered(&self) -> bool {
        matches!(self.content, DocumentContent::Rendered(_))
    }

    /// Get the raw content, if available.
    pub fn raw_content(&self) -> Option<&str> {
        match &self.content {
            DocumentContent::Raw(s) => Some(s),
            _ => None,
        }
    }

    /// Get the parsed content (markdown without front matter), if available.
    pub fn parsed_content(&self) -> Option<&str> {
        match &self.content {
            DocumentContent::Parsed(s) => Some(s),
            _ => None,
        }
    }

    /// Get the rendered HTML, if available.
    pub fn rendered_content(&self) -> Option<&str> {
        match &self.content {
            DocumentContent::Rendered(s) => Some(s),
            _ => None,
        }
    }
}

/// Convert a filename slug to title case.
/// "getting-started" -> "Getting Started"
/// "installation" -> "Installation"
fn title_case(s: &str) -> String {
    s.split(|c| c == '-' || c == '_')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_title_case() {
        assert_eq!(title_case("getting-started"), "Getting Started");
        assert_eq!(title_case("installation"), "Installation");
        assert_eq!(title_case("api_reference"), "Api Reference");
        assert_eq!(title_case("README"), "README");
    }

    #[test]
    fn test_document_title_fallback() {
        let doc = Document::discovered(
            "cli".to_string(),
            PathBuf::from("getting-started/installation.md"),
            "/cli/getting-started/installation".to_string(),
        );
        assert_eq!(doc.title(), "Installation");
    }

    #[test]
    fn test_document_title_from_front_matter() {
        let mut doc = Document::discovered(
            "cli".to_string(),
            PathBuf::from("intro.md"),
            "/cli/intro".to_string(),
        );
        doc.front_matter.title = Some("Welcome to the CLI".to_string());
        assert_eq!(doc.title(), "Welcome to the CLI");
    }

    #[test]
    fn test_parse_front_matter_basic() {
        let content = r#"---
title: My Page
description: A test page
---

# Hello World
"#;
        let parsed = parse_front_matter(content);
        assert_eq!(parsed.front_matter.title, Some("My Page".to_string()));
        assert_eq!(parsed.front_matter.description, Some("A test page".to_string()));
        assert_eq!(parsed.content.trim(), "# Hello World");
    }

    #[test]
    fn test_parse_front_matter_with_custom_fields() {
        let content = r#"---
title: Custom Page
author: John Doe
tags:
  - rust
  - documentation
---

Content here
"#;
        let parsed = parse_front_matter(content);
        assert_eq!(parsed.front_matter.title, Some("Custom Page".to_string()));
        assert!(parsed.front_matter.extra.contains_key("author"));
        assert!(parsed.front_matter.extra.contains_key("tags"));
    }

    #[test]
    fn test_parse_front_matter_no_front_matter() {
        let content = "# Just Markdown\n\nNo front matter here.";
        let parsed = parse_front_matter(content);
        assert_eq!(parsed.front_matter.title, None);
        assert!(parsed.content.starts_with("# Just Markdown"));
    }

    #[test]
    fn test_parse_front_matter_empty_front_matter() {
        let content = "---\n---\n\n# Content";
        let parsed = parse_front_matter(content);
        assert_eq!(parsed.front_matter.title, None);
        assert!(parsed.content.starts_with("# Content"));
    }
}
