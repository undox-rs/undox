//! Tera template processing stage.
//!
//! Processes Tera syntax in markdown content, expanding macros,
//! variables, and control structures before markdown rendering.

use crate::build::pipeline::{PipelineContext, PipelineError, ProcessingDocument, Stage};
use crate::build::render::{ContentRenderContext, PageInfo};

/// Stage that processes Tera syntax in markdown content.
///
/// This stage runs Tera templating on the raw markdown, allowing
/// content authors to use:
/// - Macros: `{{ macros::note(content="...") }}`
/// - Variables: `{{ page.title }}`
/// - Control flow: `{% if ... %}...{% endif %}`
///
/// The processed markdown (with Tera syntax expanded) replaces
/// the document's content for the next stage.
pub struct TeraStage;

impl Stage for TeraStage {
    fn name(&self) -> &'static str {
        "tera"
    }

    fn process(
        &self,
        docs: &mut [ProcessingDocument],
        ctx: &mut PipelineContext,
    ) -> Result<(), PipelineError> {
        for doc in docs {
            // Build page info for template context
            let page_info = PageInfo {
                title: doc.title(),
                url: doc.doc.url_path.clone(),
                description: doc.doc.front_matter.description.clone(),
                extra: doc.doc.front_matter.extra.clone(),
            };

            // Create context for Tera rendering
            let content_context = ContentRenderContext {
                site: ctx.site.clone(),
                page: page_info,
                theme: ctx.theme_settings.clone(),
                undox: ctx.undox.clone(),
            };

            // Process Tera syntax in the markdown
            let processed = ctx
                .renderer
                .render_content(&doc.content, &content_context)?;

            // Update document content with processed result
            doc.content = processed;
        }

        Ok(())
    }
}
