//! File writing stage.
//!
//! Writes the final HTML output to the filesystem.

use crate::build::paths::url_to_output_path;
use crate::build::pipeline::{PipelineContext, PipelineError, ProcessingDocument, Stage};

/// Stage that writes rendered documents to the output directory.
///
/// This stage takes the final HTML from `doc.output_html` and writes
/// it to the appropriate location in the output directory, creating
/// any necessary parent directories.
pub struct WriteStage;

impl Stage for WriteStage {
    fn name(&self) -> &'static str {
        "write"
    }

    fn process(
        &self,
        docs: &mut [ProcessingDocument],
        ctx: &mut PipelineContext,
    ) -> Result<(), PipelineError> {
        for doc in docs {
            // Get the final HTML output
            let html = doc.output_html.as_ref().ok_or_else(|| {
                PipelineError::stage(
                    "write",
                    format!(
                        "document '{}' has no output HTML (was template stage run?)",
                        doc.url_path()
                    ),
                )
            })?;

            // Determine output path
            let output_path = url_to_output_path(doc.url_path(), ctx.output_dir);

            // Create parent directories if needed
            if let Some(parent) = output_path.parent() {
                std::fs::create_dir_all(parent)?;
            }

            // Write the file
            std::fs::write(&output_path, html)?;
        }

        Ok(())
    }
}
