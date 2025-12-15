//! Page template rendering stage.
//!
//! Wraps rendered HTML content in the page template,
//! adding navigation, site chrome, and other page elements.

use crate::build::pipeline::{PipelineContext, PipelineError, ProcessingDocument, Stage};
use crate::build::render::{PageContext, PageInfo};

/// Stage that applies the page template to rendered content.
///
/// This stage wraps the HTML content (from the markdown stage) in
/// the full page template, adding:
/// - Site header and navigation
/// - Sidebar navigation for the current source
/// - Table of contents
/// - Footer and other chrome
///
/// After this stage, `doc.output_html` contains the complete HTML page.
pub struct TemplateStage;

impl Stage for TemplateStage {
    fn name(&self) -> &'static str {
        "template"
    }

    fn process(
        &self,
        docs: &mut [ProcessingDocument],
        ctx: &mut PipelineContext,
    ) -> Result<(), PipelineError> {
        for doc in docs {
            // Build page info
            let page_info = PageInfo {
                title: doc.title(),
                url: doc.doc.url_path.clone(),
                description: doc.doc.front_matter.description.clone(),
                extra: doc.doc.front_matter.extra.clone(),
            };

            // Build full page context
            let page_context = PageContext {
                site: ctx.site.clone(),
                page: page_info,
                content: doc.content.clone(),
                nav: ctx.nav_for_source(doc.source_name()),
                sources: ctx.source_tabs_for(doc.source_name()),
                toc: doc.toc.clone(),
                theme: ctx.theme_settings.clone(),
                undox: ctx.undox.clone(),
            };

            // Render with page template
            let html = ctx.renderer.render_page(&page_context)?;

            // Store final output
            doc.output_html = Some(html);
        }

        Ok(())
    }
}
