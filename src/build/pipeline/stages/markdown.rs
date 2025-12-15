//! Content rendering stage.
//!
//! Renders document content to HTML using the appropriate format
//! from the format registry (Markdown, AsciiDoc, etc.).

use crate::build::format::FormatContext;
use crate::build::pipeline::{PipelineContext, PipelineError, ProcessingDocument, Stage};

/// Stage that renders content to HTML using the format registry.
///
/// This stage:
/// - Looks up the appropriate format based on file extension
/// - Renders content to HTML (with syntax highlighting for code blocks)
/// - Extracts heading structure for table of contents
///
/// After this stage, `doc.content` contains HTML and `doc.toc`
/// contains the extracted headings.
///
/// Note: This stage is named "markdown" for backwards compatibility
/// with pipeline extension points, even though it now handles all formats.
pub struct MarkdownStage;

impl Stage for MarkdownStage {
    fn name(&self) -> &'static str {
        "markdown"
    }

    fn process(
        &self,
        docs: &mut [ProcessingDocument],
        ctx: &mut PipelineContext,
    ) -> Result<(), PipelineError> {
        // Create format context for rendering
        let format_ctx = FormatContext {
            highlighter: ctx.highlighter,
            markdown_config: ctx.markdown_config,
        };

        for doc in docs {
            // Look up format based on file extension
            let format = ctx
                .format_registry
                .for_path(&doc.doc.source_path)
                .ok_or_else(|| {
                    PipelineError::stage(
                        "markdown",
                        format!(
                            "no format registered for extension: {}",
                            doc.doc
                                .source_path
                                .extension()
                                .and_then(|e| e.to_str())
                                .unwrap_or("(none)")
                        ),
                    )
                })?;

            // Render content using the format
            let output = format.render(&doc.content, &format_ctx).map_err(|e| {
                PipelineError::stage(
                    "markdown",
                    format!("failed to render {}: {}", doc.url_path(), e),
                )
            })?;

            // Update document with rendered HTML and TOC
            doc.content = output.html;
            doc.toc = output.toc;
        }

        Ok(())
    }
}
