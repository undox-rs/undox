use std::path::{Path, PathBuf};

use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd, html};

use crate::config::RootConfig;

use super::document::{parse_front_matter, ContentItem, Document};
use super::highlight::SyntaxHighlighter;
use super::render::{NavLink, NavSection, PageContext, PageInfo, RenderError, Renderer, SiteContext};
use super::source::{ResolvedSource, SourceError};

#[derive(thiserror::Error, Debug)]
pub enum BuildError {
    #[error("source error: {0}")]
    Source(#[from] SourceError),

    #[error("render error: {0}")]
    Render(#[from] RenderError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
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
}

impl Builder {
    pub fn new(config: RootConfig, base_path: PathBuf) -> Self {
        Self { config, base_path }
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

        // Step 2: Discover and collect content from all sources
        let mut all_items: Vec<(ContentItem, PathBuf)> = Vec::new();
        for source in &resolved_sources {
            let content = source.discover_content()?;
            let display_path = source.local_path.canonicalize().unwrap_or(source.local_path.clone());
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
        println!("Found {} document(s) and {} static file(s)", doc_count, static_count);

        // Step 3: Load renderer
        let theme_path = self.theme_path();
        let renderer = Renderer::new(&theme_path)?;

        // Step 4: Build navigation from documents
        let nav = self.build_navigation(&all_items);

        // Step 5: Create output directory
        let output_dir = self.output_dir();
        std::fs::create_dir_all(&output_dir)?;

        // Step 6: Create syntax highlighter
        let highlighter = SyntaxHighlighter::default();

        // Step 7: Render and write each item
        let site_context = SiteContext {
            name: self.config.site.name.clone(),
            url: self.config.site.url.clone(),
        };

        // Get theme settings for templates
        let theme_settings = self.config.theme.settings.clone();

        for (item, source_path) in &all_items {
            self.write_item(item, source_path, &output_dir, &renderer, &site_context, &nav, &highlighter, &theme_settings)?;
        }

        let display_output = output_dir.canonicalize().unwrap_or(output_dir.clone());
        println!("Wrote {} file(s) to {}", all_items.len(), display_output.display());

        Ok(BuildResult {
            output_dir,
            theme_path,
            documents: doc_count,
            static_files: static_count,
        })
    }

    /// Write a content item to the output directory.
    fn write_item(
        &self,
        item: &ContentItem,
        source_path: &Path,
        output_dir: &Path,
        renderer: &Renderer,
        site: &SiteContext,
        nav: &[NavSection],
        highlighter: &SyntaxHighlighter,
        theme_settings: &serde_json::Value,
    ) -> Result<(), BuildError> {
        match item {
            ContentItem::Document(doc) => {
                // Read the markdown file
                let input_path = source_path.join(&doc.source_path);
                let raw_content = std::fs::read_to_string(&input_path)?;

                // Parse front matter
                let parsed = parse_front_matter(&raw_content);

                // Get title: prefer front matter, fall back to filename
                let title = parsed
                    .front_matter
                    .title
                    .clone()
                    .unwrap_or_else(|| doc.title());

                // Render markdown to HTML (without front matter) with syntax highlighting
                let content_html = render_markdown(&parsed.content, highlighter);

                // Build page context
                let context = PageContext {
                    site: site.clone(),
                    page: PageInfo {
                        title,
                        url: doc.url_path.clone(),
                        description: parsed.front_matter.description.clone(),
                        extra: parsed.front_matter.extra.clone(),
                    },
                    content: content_html,
                    nav: nav.to_vec(),
                    theme: theme_settings.clone(),
                };

                // Render with template
                let html = renderer.render_page(&context)?;

                // Write to output
                let output_path = url_to_file_path(&doc.url_path, output_dir);
                if let Some(parent) = output_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(&output_path, html)?;
            }
            ContentItem::Static(file) => {
                // Copy the static file
                let input_path = source_path.join(&file.source_path);
                let output_path = url_to_file_path(&file.output_path, output_dir);

                if let Some(parent) = output_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::copy(&input_path, &output_path)?;
            }
        }

        Ok(())
    }

    /// Resolve all source configurations to local paths.
    fn resolve_sources(&self) -> Result<Vec<ResolvedSource>, SourceError> {
        self.config
            .sources
            .iter()
            .map(|source_config| {
                ResolvedSource::resolve(source_config.clone(), &self.base_path)
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

    /// Get the theme path.
    /// For now, always uses the built-in default theme.
    fn theme_path(&self) -> PathBuf {
        // TODO: Support custom themes from config
        // For now, look for themes relative to the executable or use a fallback
        let exe_path = std::env::current_exe().ok();
        let possible_paths = [
            // Development: relative to project root
            self.base_path.join("themes/default"),
            // Installed: relative to executable
            exe_path
                .as_ref()
                .and_then(|p| p.parent())
                .map(|p| p.join("themes/default"))
                .unwrap_or_default(),
        ];

        for path in &possible_paths {
            if path.exists() {
                return path.clone();
            }
        }

        // Fallback - will error when renderer tries to load
        self.base_path.join("themes/default")
    }

    /// Build navigation structure from discovered documents.
    fn build_navigation(&self, items: &[(ContentItem, PathBuf)]) -> Vec<NavSection> {
        // Collect all documents
        let mut docs: Vec<&Document> = items
            .iter()
            .filter_map(|(item, _)| match item {
                ContentItem::Document(doc) => Some(doc),
                _ => None,
            })
            .collect();

        // Sort by URL path for consistent ordering
        docs.sort_by(|a, b| a.url_path.cmp(&b.url_path));

        // Group by top-level directory
        let mut sections: std::collections::HashMap<String, Vec<NavLink>> = std::collections::HashMap::new();
        let mut root_links: Vec<NavLink> = Vec::new();

        for doc in docs {
            let link = NavLink {
                title: doc.title(),
                url: doc.url_path.clone(),
            };

            // Determine the section from the path
            let path_parts: Vec<&str> = doc.url_path.trim_matches('/').split('/').collect();

            if path_parts.len() <= 1 {
                // Root level document
                root_links.push(link);
            } else {
                // Nested document - use first directory as section
                let section_name = path_parts[0].to_string();
                sections.entry(section_name).or_default().push(link);
            }
        }

        // Build the final nav structure
        let mut nav: Vec<NavSection> = Vec::new();

        // Add root links first
        for link in root_links {
            nav.push(NavSection::Link(link));
        }

        // Add sections (sorted by name)
        let mut section_names: Vec<_> = sections.keys().collect();
        section_names.sort();

        for section_name in section_names {
            if let Some(links) = sections.get(section_name) {
                nav.push(NavSection::Section {
                    section: title_case(section_name),
                    items: links.clone(),
                });
            }
        }

        nav
    }
}

/// Convert a slug to title case.
fn title_case(s: &str) -> String {
    s.split('-')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Render markdown to HTML using pulldown-cmark with syntax highlighting.
fn render_markdown(markdown: &str, highlighter: &SyntaxHighlighter) -> String {
    // Enable common extensions
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_HEADING_ATTRIBUTES;

    let parser = Parser::new_ext(markdown, options);

    // Process events, intercepting code blocks for syntax highlighting
    let mut in_code_block = false;
    let mut code_language = String::new();
    let mut code_content = String::new();

    let events: Vec<Event> = parser
        .flat_map(|event| match event {
            Event::Start(Tag::CodeBlock(kind)) => {
                in_code_block = true;
                code_language = match kind {
                    CodeBlockKind::Fenced(lang) => lang.to_string(),
                    CodeBlockKind::Indented => String::new(),
                };
                code_content.clear();
                vec![] // Don't emit the start tag yet
            }
            Event::End(TagEnd::CodeBlock) => {
                in_code_block = false;
                // Apply syntax highlighting and emit as raw HTML
                let highlighted = highlighter.highlight(&code_content, &code_language);
                vec![Event::Html(highlighted.into())]
            }
            Event::Text(text) if in_code_block => {
                code_content.push_str(&text);
                vec![] // Accumulate, don't emit yet
            }
            _ => vec![event],
        })
        .collect();

    let mut html_output = String::new();
    html::push_html(&mut html_output, events.into_iter());

    html_output
}

/// Convert a URL path to a file path in the output directory.
/// "/cli/installation" -> "output_dir/cli/installation/index.html"
/// "/" -> "output_dir/index.html"
fn url_to_file_path(url_path: &str, output_dir: &Path) -> PathBuf {
    let url_path = url_path.trim_start_matches('/');

    if url_path.is_empty() {
        // Root path
        output_dir.join("index.html")
    } else if url_path.contains('.') {
        // Already has extension (static file)
        output_dir.join(url_path)
    } else {
        // Document - create directory with index.html
        output_dir.join(url_path).join("index.html")
    }
}

/// Get the base path from a config file path (its parent directory).
pub fn base_path_from_config(config_path: &Path) -> PathBuf {
    config_path
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."))
}
