//! Document types for pipeline processing.

use std::path::PathBuf;

use crate::build::document::Document;
use crate::build::render::TocEntry;

/// A document being processed through the pipeline.
///
/// Wraps the original `Document` with mutable state that evolves
/// through pipeline stages:
///
/// 1. Initially: `content` = raw markdown, `toc` = empty
/// 2. After tera: `content` = processed markdown (macros expanded)
/// 3. After markdown: `content` = HTML, `toc` = populated
/// 4. After template: `output_html` = final page HTML
#[derive(Debug)]
pub struct ProcessingDocument {
    /// The original document (metadata and raw content)
    pub doc: Document,

    /// Path to the source directory (for resolving relative paths)
    #[allow(dead_code)]
    pub source_path: PathBuf,

    /// Content being processed.
    ///
    /// Starts as the raw markdown content from `doc.raw_content`.
    /// After tera stage: markdown with macros expanded.
    /// After markdown stage: HTML fragment (just the content, no page wrapper).
    pub content: String,

    /// Table of contents extracted during markdown rendering.
    ///
    /// Empty until the markdown stage populates it.
    pub toc: Vec<TocEntry>,

    /// Final HTML output after template rendering.
    ///
    /// None until the template stage populates it.
    pub output_html: Option<String>,
}

impl ProcessingDocument {
    /// Create a new processing document from a discovered document.
    pub fn new(doc: Document, source_path: PathBuf) -> Self {
        let content = doc.raw_content.clone();
        Self {
            doc,
            source_path,
            content,
            toc: Vec::new(),
            output_html: None,
        }
    }

    /// Get the document's URL path (for output location).
    pub fn url_path(&self) -> &str {
        &self.doc.url_path
    }

    /// Get the document's source name.
    pub fn source_name(&self) -> &str {
        &self.doc.source_name
    }

    /// Get the document title.
    pub fn title(&self) -> String {
        self.doc.title()
    }
}
