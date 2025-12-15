use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::util::title_case;

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

// =============================================================================
// Static files
// =============================================================================

/// A static file (image, CSS, JS, etc.) that gets copied to output.
#[derive(Debug, Clone)]
pub struct StaticFile {
    /// Path relative to the source root (e.g., "images/screenshot.png")
    pub source_path: PathBuf,
    /// The output path this file will be written to (e.g., "/cli/images/screenshot.png")
    pub output_path: String,
}

impl StaticFile {
    /// Create a new static file.
    pub fn new(_source_name: String, source_path: PathBuf, output_path: String) -> Self {
        Self {
            source_path,
            output_path,
        }
    }
}

// =============================================================================
// Documents
// =============================================================================

/// A document discovered in a source directory.
///
/// Contains all information needed to render the document:
/// - Location info (source, paths)
/// - Front matter metadata (title, description, etc.)
/// - Raw markdown content (without front matter block)
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
    /// Raw markdown content (front matter already stripped)
    pub raw_content: String,
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
        content[markdown_start..]
            .trim_start_matches('\n')
            .to_string()
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

impl Document {
    /// Create a new document with all fields.
    pub fn new(
        source_name: String,
        source_path: PathBuf,
        url_path: String,
        front_matter: FrontMatter,
        raw_content: String,
    ) -> Self {
        Self {
            source_name,
            source_path,
            url_path,
            front_matter,
            raw_content,
        }
    }

    /// Get the document title, falling back to filename if not in front matter.
    pub fn title(&self) -> String {
        self.front_matter.title.clone().unwrap_or_else(|| {
            self.source_path
                .file_stem()
                .and_then(|s| s.to_str())
                .map(title_case)
                .unwrap_or_else(|| "Untitled".to_string())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_document_title_fallback() {
        let doc = Document::new(
            "cli".to_string(),
            PathBuf::from("getting-started/installation.md"),
            "/cli/getting-started/installation".to_string(),
            FrontMatter::default(),
            "# Installation".to_string(),
        );
        assert_eq!(doc.title(), "Installation");
    }

    #[test]
    fn test_document_title_from_front_matter() {
        let doc = Document::new(
            "cli".to_string(),
            PathBuf::from("intro.md"),
            "/cli/intro".to_string(),
            FrontMatter {
                title: Some("Welcome to the CLI".to_string()),
                ..Default::default()
            },
            "# Welcome".to_string(),
        );
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
        assert_eq!(
            parsed.front_matter.description,
            Some("A test page".to_string())
        );
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
