//! Pipeline error types.

use crate::build::markdown::MarkdownError;
use crate::build::render::RenderError;

/// Errors that can occur during pipeline processing.
#[derive(thiserror::Error, Debug)]
pub enum PipelineError {
    #[error("tera rendering error: {0}")]
    Tera(#[from] RenderError),

    #[error("markdown rendering error: {0}")]
    Markdown(#[from] MarkdownError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("stage '{stage}' failed: {message}")]
    Stage { stage: String, message: String },
}

impl PipelineError {
    /// Create a stage-specific error.
    pub fn stage(stage: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Stage {
            stage: stage.into(),
            message: message.into(),
        }
    }
}
