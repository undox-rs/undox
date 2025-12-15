//! Build pipeline for document processing.
//!
//! The pipeline transforms documents through a series of stages:
//! 1. Tera processing (macro expansion in markdown)
//! 2. Markdown rendering (to HTML with TOC)
//! 3. Template rendering (page template wrapper)
//! 4. File writing (output to disk)
//!
//! Custom stages can be inserted before or after any named stage.
//! Build-wide stages run after all documents are processed.

mod context;
mod document;
mod error;
mod stages;

pub use context::PipelineContext;
pub use document::ProcessingDocument;
pub use error::PipelineError;

use stages::{MarkdownStage, TemplateStage, TeraStage, WriteStage};

/// A stage in the document processing pipeline.
///
/// Stages transform documents sequentially. Each stage receives all documents
/// and can modify them in place before passing to the next stage.
#[allow(dead_code)]
pub trait Stage: Send + Sync {
    /// Unique name for this stage (used for insertion points).
    fn name(&self) -> &'static str;

    /// Process documents through this stage.
    ///
    /// Documents are passed by mutable reference so stages can transform
    /// their content in place. The `ctx` provides access to shared resources
    /// like the renderer and highlighter.
    fn process(
        &self,
        docs: &mut [ProcessingDocument],
        ctx: &mut PipelineContext,
    ) -> Result<(), PipelineError>;
}

/// A stage that runs once after all documents are processed.
///
/// Use this for build-wide operations like:
/// - CSS aggregation
/// - Sitemap generation
/// - Search index building
/// - Asset optimization
#[allow(dead_code)]
pub trait FinalizeStage: Send + Sync {
    /// Unique name for this stage.
    fn name(&self) -> &'static str;

    /// Run finalization after all documents are processed and written.
    fn finalize(&self, ctx: &PipelineContext) -> Result<(), PipelineError>;
}

/// The document processing pipeline.
///
/// Orchestrates document transformation through a series of stages.
/// The default pipeline includes: tera → markdown → template → write.
///
/// # Extension Points
///
/// Insert custom stages using `insert_before` or `insert_after`:
///
/// ```ignore
/// pipeline.insert_after("tera", MyCustomStage);
/// ```
///
/// Add build-wide stages using `add_finalize_stage`:
///
/// ```ignore
/// pipeline.add_finalize_stage(CssAggregationStage::new(config));
/// ```
pub struct Pipeline {
    /// Document processing stages (run for each document batch)
    stages: Vec<Box<dyn Stage>>,
    /// Build-wide stages (run once after all documents)
    finalize_stages: Vec<Box<dyn FinalizeStage>>,
}

impl Pipeline {
    /// Create an empty pipeline with no stages.
    pub fn new() -> Self {
        Self {
            stages: Vec::new(),
            finalize_stages: Vec::new(),
        }
    }

    /// Create the default pipeline with standard stages.
    ///
    /// Stages: tera → markdown → template → write
    pub fn default_pipeline() -> Self {
        let mut pipeline = Self::new();
        pipeline.add_stage(TeraStage);
        pipeline.add_stage(MarkdownStage);
        pipeline.add_stage(TemplateStage);
        pipeline.add_stage(WriteStage);
        pipeline
    }

    /// Add a stage to the end of the pipeline.
    pub fn add_stage<S: Stage + 'static>(&mut self, stage: S) -> &mut Self {
        self.stages.push(Box::new(stage));
        self
    }

    /// Insert a stage before the named stage.
    ///
    /// # Panics
    ///
    /// Panics if no stage with the given name exists.
    #[allow(dead_code)]
    pub fn insert_before<S: Stage + 'static>(&mut self, name: &str, stage: S) -> &mut Self {
        let pos = self
            .stages
            .iter()
            .position(|s| s.name() == name)
            .unwrap_or_else(|| panic!("stage '{}' not found in pipeline", name));
        self.stages.insert(pos, Box::new(stage));
        self
    }

    /// Insert a stage after the named stage.
    ///
    /// # Panics
    ///
    /// Panics if no stage with the given name exists.
    #[allow(dead_code)]
    pub fn insert_after<S: Stage + 'static>(&mut self, name: &str, stage: S) -> &mut Self {
        let pos = self
            .stages
            .iter()
            .position(|s| s.name() == name)
            .unwrap_or_else(|| panic!("stage '{}' not found in pipeline", name));
        self.stages.insert(pos + 1, Box::new(stage));
        self
    }

    /// Add a finalize stage (runs after all documents are processed).
    #[allow(dead_code)]
    pub fn add_finalize_stage<S: FinalizeStage + 'static>(&mut self, stage: S) -> &mut Self {
        self.finalize_stages.push(Box::new(stage));
        self
    }

    /// Run the pipeline on a set of documents.
    pub fn run(
        &self,
        docs: &mut [ProcessingDocument],
        ctx: &mut PipelineContext,
    ) -> Result<(), PipelineError> {
        // Run each stage in sequence
        for stage in &self.stages {
            stage.process(docs, ctx)?;
        }

        // Run finalize stages
        for stage in &self.finalize_stages {
            stage.finalize(ctx)?;
        }

        Ok(())
    }

    /// Get the names of all stages in order.
    #[allow(dead_code)]
    pub fn stage_names(&self) -> Vec<&'static str> {
        self.stages.iter().map(|s| s.name()).collect()
    }
}

impl Default for Pipeline {
    fn default() -> Self {
        Self::default_pipeline()
    }
}
