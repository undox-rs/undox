//! Pipeline context for sharing state across stages.

use std::collections::HashMap;
use std::path::Path;

use crate::build::format::FormatRegistry;
use crate::build::highlight::SyntaxHighlighter;
use crate::build::render::{NavSection, Renderer, SiteContext, SourceTab, UndoxContext};
use crate::config::MarkdownConfig;

/// Shared context for pipeline stages.
///
/// Contains all resources and configuration needed by stages during processing.
/// This is similar to `BuildContext` but designed for the pipeline architecture.
pub struct PipelineContext<'a> {
    // === Output configuration ===
    /// Directory where output files are written
    pub output_dir: &'a Path,

    // === Site-level data ===
    /// Site metadata (name, URL, favicon)
    pub site: &'a SiteContext,

    /// Theme settings passed to templates
    pub theme_settings: &'a serde_json::Value,

    /// Markdown processing configuration
    pub markdown_config: &'a MarkdownConfig,

    // === Navigation ===
    /// Per-source navigation structure
    pub nav_by_source: &'a HashMap<String, Vec<NavSection>>,

    /// Source tabs for top-level navigation
    pub source_tabs: &'a [SourceTab],

    // === Services ===
    /// Syntax highlighter for code blocks
    pub highlighter: &'a SyntaxHighlighter,

    /// Template renderer (needs mutable access for render_content)
    pub renderer: &'a mut Renderer,

    /// Content format registry for rendering different file types
    pub format_registry: &'a FormatRegistry,

    // === Mode flags ===
    /// Undox context (dev mode, live reload, version)
    pub undox: UndoxContext,
}

impl<'a> PipelineContext<'a> {
    /// Create a new pipeline context.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        output_dir: &'a Path,
        site: &'a SiteContext,
        theme_settings: &'a serde_json::Value,
        markdown_config: &'a MarkdownConfig,
        nav_by_source: &'a HashMap<String, Vec<NavSection>>,
        source_tabs: &'a [SourceTab],
        highlighter: &'a SyntaxHighlighter,
        renderer: &'a mut Renderer,
        format_registry: &'a FormatRegistry,
        dev_mode: bool,
        live_reload: bool,
    ) -> Self {
        Self {
            output_dir,
            site,
            theme_settings,
            markdown_config,
            nav_by_source,
            source_tabs,
            highlighter,
            renderer,
            format_registry,
            undox: UndoxContext {
                dev: dev_mode,
                live_reload: dev_mode && live_reload,
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
        }
    }

    /// Get navigation for a specific source.
    pub fn nav_for_source(&self, source_name: &str) -> Vec<NavSection> {
        self.nav_by_source
            .get(source_name)
            .cloned()
            .unwrap_or_default()
    }

    /// Build source tabs with the current source highlighted.
    pub fn source_tabs_for(&self, current_source: &str) -> Vec<SourceTab> {
        self.source_tabs
            .iter()
            .map(|tab| SourceTab {
                name: tab.name.clone(),
                source_id: tab.source_id.clone(),
                url: tab.url.clone(),
                is_current: tab.source_id == current_source,
                is_top_level: tab.is_top_level,
            })
            .collect()
    }
}
