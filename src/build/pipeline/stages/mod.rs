//! Default pipeline stages.
//!
//! The standard document processing pipeline consists of:
//!
//! 1. **TeraStage** - Process Tera syntax in markdown (macros, variables, loops)
//! 2. **MarkdownStage** - Convert markdown to HTML with syntax highlighting
//! 3. **TemplateStage** - Wrap content in the page template
//! 4. **WriteStage** - Write final HTML to output directory

mod markdown;
mod template;
mod tera;
mod write;

pub use markdown::MarkdownStage;
pub use template::TemplateStage;
pub use tera::TeraStage;
pub use write::WriteStage;
