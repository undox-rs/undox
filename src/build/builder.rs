use std::path::{Path, PathBuf};

use crate::config::{Location, RootConfig};
use crate::git::GitFetcher;
use crate::util::title_case;

use super::document::ContentItem;
use super::format::FormatRegistry;
use super::highlight::SyntaxHighlighter;
use super::nav::build_navigation_by_source;
use super::paths::url_to_output_path;
use super::pipeline::{Pipeline, PipelineContext, PipelineError, ProcessingDocument};
use super::render::{RenderError, Renderer, SiteContext, SourceTab};
use super::source::{ResolvedSource, SourceError};

#[derive(thiserror::Error, Debug)]
pub enum BuildError {
    #[error("source error: {0}")]
    Source(#[from] SourceError),

    #[error("render error: {0}")]
    Render(#[from] RenderError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("pipeline error: {0}")]
    Pipeline(#[from] PipelineError),

    #[error("git fetch error: {0}")]
    Git(#[from] crate::git::GitError),

    #[error("theme error: {0}")]
    Theme(String),
}

pub struct BuildResult {
    pub output_dir: PathBuf,
    pub theme_path: PathBuf,
    pub documents: usize,
    pub static_files: usize,
}

pub struct Builder {
    config: RootConfig,
    /// Base path for resolving relative paths (typically the config file's directory)
    base_path: PathBuf,
    /// Optional path for theme resolution (used when building child configs)
    /// If set, themes are resolved relative to this path instead of base_path
    theme_base_path: Option<PathBuf>,
    /// Whether we're in development mode (enables live reload script, etc.)
    dev_mode: bool,
    /// Whether live reload is enabled (only relevant in dev mode)
    live_reload: bool,
}

impl Builder {
    pub fn new(config: RootConfig, base_path: PathBuf) -> Self {
        Self {
            config,
            base_path,
            theme_base_path: None,
            dev_mode: false,
            live_reload: false,
        }
    }

    /// Set a custom base path for theme resolution.
    /// Used when building child configs where the theme is in the parent repo.
    pub fn with_theme_base_path(mut self, path: PathBuf) -> Self {
        self.theme_base_path = Some(path);
        self
    }

    /// Enable development mode (live reload, etc.)
    pub fn with_dev_mode(mut self, dev_mode: bool) -> Self {
        self.dev_mode = dev_mode;
        self
    }

    /// Enable live reload in dev mode
    pub fn with_live_reload(mut self, live_reload: bool) -> Self {
        self.live_reload = live_reload;
        self
    }

    pub async fn build(&self) -> Result<BuildResult, BuildError> {
        // Build pipeline:
        // 1. Resolve sources -> ResolvedSource[]
        // 2. Discover content -> ContentItem[]
        // 3. Load renderer (templates)
        // 4. Build navigation
        // 5. Render and write each document
        // 6. Copy static files

        // Step 1: Resolve all sources
        let resolved_sources = self.resolve_sources()?;
        println!("Resolved {} source(s)", resolved_sources.len());

        // Step 2: Create format registry (needed for content discovery)
        let format_registry = FormatRegistry::with_defaults();

        // Step 3: Discover and collect content from all sources
        let mut all_items: Vec<(ContentItem, PathBuf)> = Vec::new();
        for source in &resolved_sources {
            let content = source.discover_content(&format_registry)?;
            let display_path = source
                .local_path
                .canonicalize()
                .unwrap_or(source.local_path.clone());
            println!(
                "  - {}: {} item(s) in {}",
                source.config.name,
                content.len(),
                display_path.display()
            );
            for item in content {
                all_items.push((item, source.local_path.clone()));
            }
        }

        // Count documents vs static files
        let doc_count = all_items
            .iter()
            .filter(|(item, _)| matches!(item, ContentItem::Document(_)))
            .count();
        let static_count = all_items.len() - doc_count;
        println!(
            "Found {} document(s) and {} static file(s)",
            doc_count, static_count
        );

        // Step 4: Load renderer
        let theme_path = self.resolve_theme_path()?;
        let mut renderer = Renderer::new(&theme_path)?;

        // Step 5: Build source tabs for top-level navigation
        let source_tabs: Vec<SourceTab> = resolved_sources
            .iter()
            .map(|source| {
                let url_prefix = source.url_prefix();
                let is_top_level = url_prefix == "/";
                // Use title if set, otherwise title-case the name
                let display_name = source
                    .config
                    .title
                    .clone()
                    .unwrap_or_else(|| title_case(&source.config.name));
                SourceTab {
                    name: display_name,
                    source_id: source.config.name.clone(),
                    url: if is_top_level {
                        "/".to_string()
                    } else {
                        format!("{}/", url_prefix)
                    },
                    is_current: false, // Will be set per-page
                    is_top_level,
                }
            })
            .collect();

        // Step 6: Build per-source navigation
        let nav_by_source = build_navigation_by_source(&all_items, &resolved_sources);

        // Step 7: Create output directory
        let output_dir = self.output_dir();
        std::fs::create_dir_all(&output_dir)?;

        // Step 8: Copy theme static files to _theme/
        let theme_static = theme_path.join("static");
        if theme_static.exists() {
            let theme_output = output_dir.join("_theme");
            copy_dir_recursive(&theme_static, &theme_output)?;
        }

        // Step 9: Create syntax highlighter
        let highlighter = SyntaxHighlighter::default();

        // Step 10: Build site context (shared across all pages)
        let site_context = SiteContext {
            name: self.config.site.name.clone(),
            url: self.config.site.url.clone(),
            favicon: self.config.site.favicon.clone(),
        };

        // Step 11: Separate documents from static files
        let mut documents: Vec<ProcessingDocument> = Vec::new();
        let mut static_files: Vec<(&super::document::StaticFile, &PathBuf)> = Vec::new();

        for (item, source_path) in &all_items {
            match item {
                ContentItem::Document(doc) => {
                    documents.push(ProcessingDocument::new(doc.clone(), source_path.clone()));
                }
                ContentItem::Static(file) => {
                    static_files.push((file, source_path));
                }
            }
        }

        // Step 12: Create pipeline context
        let mut ctx = PipelineContext::new(
            &output_dir,
            &site_context,
            &self.config.theme.settings,
            &self.config.markdown,
            &nav_by_source,
            &source_tabs,
            &highlighter,
            &mut renderer,
            &format_registry,
            self.dev_mode,
            self.live_reload,
        );

        // Step 13: Run the document pipeline
        let pipeline = Pipeline::default_pipeline();
        pipeline.run(&mut documents, &mut ctx)?;

        // Step 14: Copy static files
        for (file, source_path) in static_files {
            let input_path = source_path.join(&file.source_path);
            let output_path = url_to_output_path(&file.output_path, &output_dir);

            if let Some(parent) = output_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(&input_path, &output_path)?;
        }

        let display_output = output_dir.canonicalize().unwrap_or(output_dir.clone());
        println!(
            "Wrote {} file(s) to {}",
            all_items.len(),
            display_output.display()
        );

        Ok(BuildResult {
            output_dir,
            theme_path,
            documents: doc_count,
            static_files: static_count,
        })
    }

    /// Resolve all source configurations to local paths.
    fn resolve_sources(&self) -> Result<Vec<ResolvedSource>, SourceError> {
        let cache_dir = self.base_path.join(".undox/cache/git");
        self.config
            .sources
            .iter()
            .map(|source_config| {
                ResolvedSource::resolve(source_config.clone(), &self.base_path, &cache_dir)
            })
            .collect()
    }

    /// Get the output directory path, resolved against base_path.
    fn output_dir(&self) -> PathBuf {
        let output = &self.config.site.output;
        if output.is_relative() {
            self.base_path.join(output)
        } else {
            output.clone()
        }
    }

    /// Resolve the theme location to a local path.
    fn resolve_theme_path(&self) -> Result<PathBuf, BuildError> {
        let cache_dir = self.base_path.join(".undox/cache/git");
        // Use theme_base_path if set (for child configs), otherwise base_path
        let theme_base = self.theme_base_path.as_ref().unwrap_or(&self.base_path);

        match self.config.theme.resolved_location() {
            Location::Path { path } => {
                // Resolve relative paths against theme_base
                let resolved = if path.is_relative() {
                    theme_base.join(&path)
                } else {
                    path
                };

                if !resolved.exists() {
                    return Err(BuildError::Theme(format!(
                        "theme path does not exist: {}",
                        resolved.display()
                    )));
                }

                Ok(resolved)
            }
            Location::Git { git } => {
                // Fetch theme from git
                let git_loc = git.to_location();
                eprintln!("Fetching theme from {}...", git_loc.url);
                let fetcher = GitFetcher::new(cache_dir);
                let repo_path = fetcher.fetch_location(&git_loc)?;

                // Apply path if specified
                let resolved = if let Some(ref path) = git_loc.path {
                    repo_path.join(path)
                } else {
                    repo_path
                };

                if !resolved.exists() {
                    return Err(BuildError::Theme(format!(
                        "theme path does not exist after git fetch: {}",
                        resolved.display()
                    )));
                }

                Ok(resolved)
            }
        }
    }
}

/// Recursively copy a directory to a destination.
fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    if !src.exists() {
        return Ok(());
    }

    std::fs::create_dir_all(dst)?;

    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path)?;
        }
    }

    Ok(())
}
