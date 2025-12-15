use std::path::{Path, PathBuf};

use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd, html};

use crate::config::{MarkdownConfig, RootConfig};

use super::document::{ContentItem, Document, parse_front_matter};
use super::highlight::SyntaxHighlighter;
use super::render::{
    ContentRenderContext, NavLink, NavSection, PageContext, PageInfo, RenderError, Renderer,
    SiteContext, SourceTab, TocEntry, UndoxContext,
};
use super::source::{ResolvedSource, SourceError};

#[derive(thiserror::Error, Debug)]
pub enum BuildError {
    #[error("source error: {0}")]
    Source(#[from] SourceError),

    #[error("render error: {0}")]
    Render(#[from] RenderError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("invalid markdown extension: {0}")]
    InvalidMarkdownExtension(String),
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

        // Step 2: Discover and collect content from all sources
        let mut all_items: Vec<(ContentItem, PathBuf)> = Vec::new();
        for source in &resolved_sources {
            let content = source.discover_content()?;
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

        // Step 3: Load renderer
        let theme_path = self.theme_path();
        let mut renderer = Renderer::new(&theme_path)?;

        // Step 4: Build source tabs for top-level navigation
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

        // Step 5: Build per-source navigation
        let nav_by_source = self.build_navigation_by_source(&all_items, &resolved_sources);

        // Step 6: Create output directory
        let output_dir = self.output_dir();
        std::fs::create_dir_all(&output_dir)?;

        // Step 6.5: Copy theme static files to _theme/
        let theme_static = theme_path.join("static");
        if theme_static.exists() {
            let theme_output = output_dir.join("_theme");
            copy_dir_recursive(&theme_static, &theme_output)?;
        }

        // Step 7: Create syntax highlighter
        let highlighter = SyntaxHighlighter::default();

        // Step 8: Render and write each item
        let site_context = SiteContext {
            name: self.config.site.name.clone(),
            url: self.config.site.url.clone(),
            favicon: self.config.site.favicon.clone(),
        };

        // Get theme settings for templates
        let theme_settings = self.config.theme.settings.clone();

        // Get markdown config for markdown processing
        let markdown_config = self.config.markdown.clone();

        for (item, source_path) in &all_items {
            self.write_item(
                item,
                source_path,
                &output_dir,
                &mut renderer,
                &site_context,
                &nav_by_source,
                &source_tabs,
                &highlighter,
                &theme_settings,
                &markdown_config,
            )?;
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

    /// Write a content item to the output directory.
    fn write_item(
        &self,
        item: &ContentItem,
        source_path: &Path,
        output_dir: &Path,
        renderer: &mut Renderer,
        site: &SiteContext,
        nav_by_source: &std::collections::HashMap<String, Vec<NavSection>>,
        source_tabs: &[SourceTab],
        highlighter: &SyntaxHighlighter,
        theme_settings: &serde_json::Value,
        markdown_config: &MarkdownConfig,
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

                // Build undox context
                let undox_context = UndoxContext {
                    dev: self.dev_mode,
                    live_reload: self.dev_mode && self.live_reload,
                    version: env!("CARGO_PKG_VERSION").to_string(),
                };

                // First pass: Render markdown content through Tera for macros/loops/variables
                let content_context = ContentRenderContext {
                    site: site.clone(),
                    page: PageInfo {
                        title: title.clone(),
                        url: doc.url_path.clone(),
                        description: parsed.front_matter.description.clone(),
                        extra: parsed.front_matter.extra.clone(),
                    },
                    theme: theme_settings.clone(),
                    undox: undox_context.clone(),
                };
                let tera_processed_content =
                    renderer.render_content(&parsed.content, &content_context)?;

                // Second pass: Render markdown to HTML with syntax highlighting
                let markdown_output =
                    render_markdown(&tera_processed_content, highlighter, markdown_config)?;

                // Get navigation for this document's source
                let nav = nav_by_source
                    .get(&doc.source_name)
                    .cloned()
                    .unwrap_or_default();

                // Build source tabs with current source highlighted
                let sources: Vec<SourceTab> = source_tabs
                    .iter()
                    .map(|tab| SourceTab {
                        name: tab.name.clone(),
                        source_id: tab.source_id.clone(),
                        url: tab.url.clone(),
                        is_current: tab.source_id == doc.source_name,
                        is_top_level: tab.is_top_level,
                    })
                    .collect();

                // Build page context
                let context = PageContext {
                    site: site.clone(),
                    page: PageInfo {
                        title,
                        url: doc.url_path.clone(),
                        description: parsed.front_matter.description.clone(),
                        extra: parsed.front_matter.extra.clone(),
                    },
                    content: markdown_output.html,
                    nav,
                    sources,
                    toc: markdown_output.toc,
                    theme: theme_settings.clone(),
                    undox: undox_context,
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

    /// Get the theme path.
    /// For now, always uses the built-in default theme.
    fn theme_path(&self) -> PathBuf {
        // TODO: Support custom themes from config
        // Use theme_base_path if set (for child configs), otherwise base_path
        let theme_base = self.theme_base_path.as_ref().unwrap_or(&self.base_path);

        // For now, look for themes relative to the theme base or executable
        let exe_path = std::env::current_exe().ok();
        let possible_paths = [
            // Development: relative to theme base (parent repo for child configs)
            theme_base.join("themes/default"),
            // Also try base_path in case theme_base_path doesn't have themes
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
        theme_base.join("themes/default")
    }

    /// Build navigation structure grouped by source.
    ///
    /// Returns a map from source name to that source's navigation.
    /// Each source gets its own isolated navigation containing only its documents.
    /// If a source has configured nav, that is used; otherwise, nav is auto-generated.
    fn build_navigation_by_source(
        &self,
        items: &[(ContentItem, PathBuf)],
        resolved_sources: &[ResolvedSource],
    ) -> std::collections::HashMap<String, Vec<NavSection>> {
        // Group documents by source
        let mut docs_by_source: std::collections::HashMap<String, Vec<&Document>> =
            std::collections::HashMap::new();

        for (item, _) in items {
            if let ContentItem::Document(doc) = item {
                docs_by_source
                    .entry(doc.source_name.clone())
                    .or_default()
                    .push(doc);
            }
        }

        // Build navigation for each source
        let mut nav_by_source = std::collections::HashMap::new();

        for source in resolved_sources {
            let source_name = &source.config.name;
            let docs = docs_by_source.get(source_name).cloned().unwrap_or_default();

            // Check if source has configured nav
            if let Some(nav_config) = &source.config.nav {
                // Build lookup from relative source path to document URL
                let path_to_doc: std::collections::HashMap<String, &Document> = docs
                    .iter()
                    .map(|doc| {
                        let path_str = doc.source_path.to_string_lossy().to_string();
                        (path_str, *doc)
                    })
                    .collect();

                // Convert NavConfig to Vec<NavSection>
                let nav = self.convert_nav_config(nav_config, &path_to_doc);
                nav_by_source.insert(source_name.clone(), nav);
            } else {
                // Auto-generate navigation from documents
                let nav = self.auto_generate_nav(docs);
                nav_by_source.insert(source_name.clone(), nav);
            }
        }

        nav_by_source
    }

    /// Convert a NavConfig to Vec<NavSection> using document lookup.
    fn convert_nav_config(
        &self,
        nav_config: &[crate::config::NavItem],
        path_to_doc: &std::collections::HashMap<String, &Document>,
    ) -> Vec<NavSection> {
        use crate::config::NavItem;

        let mut result = Vec::new();

        for item in nav_config {
            match item {
                NavItem::Section { section, items } => {
                    // Convert section items to NavLinks
                    let nav_links: Vec<NavLink> = items
                        .iter()
                        .filter_map(|item| self.nav_item_to_link(item, path_to_doc))
                        .collect();

                    if !nav_links.is_empty() {
                        result.push(NavSection::Section {
                            section: section.clone(),
                            items: nav_links,
                        });
                    }
                }
                NavItem::Titled(map) => {
                    // Single titled link
                    if let Some((title, path)) = map.iter().next() {
                        if let Some(doc) = path_to_doc.get(path) {
                            result.push(NavSection::Link(NavLink {
                                title: title.clone(),
                                url: doc.url_path.clone(),
                            }));
                        }
                    }
                }
                NavItem::Path(path) => {
                    if !path.ends_with('/') {
                        // It's a file path
                        if let Some(doc) = path_to_doc.get(path) {
                            result.push(NavSection::Link(NavLink {
                                title: doc.title(),
                                url: doc.url_path.clone(),
                            }));
                        }
                    }
                    // Directory paths (ending with /) are not supported in this simple conversion
                }
            }
        }

        result
    }

    /// Convert a single NavItem to a NavLink.
    fn nav_item_to_link(
        &self,
        item: &crate::config::NavItem,
        path_to_doc: &std::collections::HashMap<String, &Document>,
    ) -> Option<NavLink> {
        use crate::config::NavItem;

        match item {
            NavItem::Titled(map) => {
                if let Some((title, path)) = map.iter().next() {
                    path_to_doc.get(path).map(|doc| NavLink {
                        title: title.clone(),
                        url: doc.url_path.clone(),
                    })
                } else {
                    None
                }
            }
            NavItem::Path(path) => {
                if !path.ends_with('/') {
                    path_to_doc.get(path).map(|doc| NavLink {
                        title: doc.title(),
                        url: doc.url_path.clone(),
                    })
                } else {
                    None
                }
            }
            NavItem::Section { .. } => {
                // Nested sections within a section not supported for now
                None
            }
        }
    }

    /// Auto-generate navigation from a list of documents.
    fn auto_generate_nav(&self, mut docs: Vec<&Document>) -> Vec<NavSection> {
        // Sort by source path (relative path within the source) for consistent ordering
        docs.sort_by(|a, b| a.source_path.cmp(&b.source_path));

        // Group by directory within the source
        // Store (is_index, NavLink) tuples for sorting
        let mut sections: std::collections::HashMap<String, Vec<(bool, NavLink)>> =
            std::collections::HashMap::new();
        let mut root_links: Vec<(bool, NavLink)> = Vec::new();

        for doc in docs {
            let is_index = doc
                .source_path
                .file_stem()
                .map_or(false, |s| s == "index");

            let link = NavLink {
                title: doc.title(),
                url: doc.url_path.clone(),
            };

            // Use source_path (relative to source root) for sectioning
            let path_str = doc.source_path.to_string_lossy();
            let path_parts: Vec<&str> = path_str.trim_matches('/').split('/').collect();

            if path_parts.len() <= 1 {
                // Root level document within this source
                root_links.push((is_index, link));
            } else {
                // Nested document - use first directory as section
                let section_name = path_parts[0].to_string();
                sections.entry(section_name).or_default().push((is_index, link));
            }
        }

        // Build the nav structure for this source
        let mut nav: Vec<NavSection> = Vec::new();

        // Sort root links: index first, then alphabetically by title
        root_links.sort_by(|a, b| {
            match (a.0, b.0) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.1.title.cmp(&b.1.title),
            }
        });

        // Add root links first (extract NavLink from tuple)
        for (_, link) in root_links {
            nav.push(NavSection::Link(link));
        }

        // Add sections (sorted by name)
        let mut section_names: Vec<_> = sections.keys().cloned().collect();
        section_names.sort();

        for section_name in section_names {
            if let Some(mut links) = sections.remove(&section_name) {
                // Sort section links: index first, then alphabetically by title
                links.sort_by(|a, b| {
                    match (a.0, b.0) {
                        (true, false) => std::cmp::Ordering::Less,
                        (false, true) => std::cmp::Ordering::Greater,
                        _ => a.1.title.cmp(&b.1.title),
                    }
                });
                nav.push(NavSection::Section {
                    section: title_case(&section_name),
                    items: links.into_iter().map(|(_, link)| link).collect(),
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
/// Result of rendering markdown, containing both HTML and table of contents.
struct MarkdownOutput {
    html: String,
    toc: Vec<TocEntry>,
}

fn render_markdown(
    markdown: &str,
    highlighter: &SyntaxHighlighter,
    markdown_config: &MarkdownConfig,
) -> Result<MarkdownOutput, BuildError> {
    let mut options = Options::empty();
    for extension in &markdown_config.extensions {
        match extension.as_str() {
            "definition_lists" => options.insert(Options::ENABLE_DEFINITION_LIST),
            "footnotes" => options.insert(Options::ENABLE_FOOTNOTES),
            "gfm" => options.insert(Options::ENABLE_GFM),
            "heading_attributes" => options.insert(Options::ENABLE_HEADING_ATTRIBUTES),
            "strikethrough" => options.insert(Options::ENABLE_STRIKETHROUGH),
            "tables" => options.insert(Options::ENABLE_TABLES),
            "tasklists" => options.insert(Options::ENABLE_TASKLISTS),
            other => return Err(BuildError::InvalidMarkdownExtension(other.to_string())),
        }
    }

    let parser = Parser::new_ext(markdown, options);

    // Process events, intercepting code blocks for syntax highlighting
    let mut in_code_block = false;
    let mut code_language = String::new();
    let mut code_content = String::new();
    // Intercept headings to add id attributes for permalinks
    struct HeadingState {
        level: pulldown_cmark::HeadingLevel,
        classes: Vec<String>,
        attrs: Vec<(String, Option<String>)>,
    }
    let mut in_heading: Option<HeadingState> = None;
    let mut used_heading_ids: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut heading_text = String::new();
    let mut toc_entries: Vec<TocEntry> = Vec::new();

    let events: Vec<Event> = parser
        .flat_map(|event| match event {
            Event::Start(Tag::Heading { level, ref id, ref classes, ref attrs }) => {
                // If heading already has an id, just pass it through
                if let Some(existing_id) = id {
                    used_heading_ids.insert(existing_id.to_string());
                    return vec![event];
                }
                // Otherwise, capture the heading to generate an id
                in_heading = Some(HeadingState {
                    level,
                    classes: classes.iter().map(|c| c.to_string()).collect(),
                    attrs: attrs.iter().map(|(k, v)| (k.to_string(), v.as_ref().map(|v| v.to_string()))).collect(),
                });
                heading_text.clear();
                vec![]
            }
            Event::End(TagEnd::Heading(_)) if in_heading.is_some() => {
                let state = in_heading.take().unwrap();

                // Generate a unique id from the heading text
                let base_id = slugify(&heading_text);
                let mut id = base_id.clone();
                let mut suffix = 1;
                while used_heading_ids.contains(&id) {
                    id = format!("{}-{}", base_id, suffix);
                    suffix += 1;
                }
                used_heading_ids.insert(id.clone());

                // Add to table of contents
                toc_entries.push(TocEntry {
                    text: heading_text.clone(),
                    id: id.clone(),
                    level: state.level as u8,
                });

                // Build class attribute if there are classes
                let class_attr = if state.classes.is_empty() {
                    String::new()
                } else {
                    format!(" class=\"{}\"", state.classes.join(" "))
                };

                // Build extra attributes
                let extra_attrs = state.attrs
                    .iter()
                    .map(|(k, v)| match v {
                        Some(val) => format!(" {}=\"{}\"", k, val),
                        None => format!(" {}", k),
                    })
                    .collect::<String>();

                // Emit the heading with id and permalink
                let permalink = format!(
                    "<a class=\"header-anchor\" href=\"#{}\" aria-label=\"Link to this heading\">#</a>",
                    id
                );
                vec![Event::Html(
                    format!(
                        "<h{} id=\"{}\"{}{}>{} {}</h{}>",
                        state.level as usize,
                        id,
                        class_attr,
                        extra_attrs,
                        heading_text,
                        permalink,
                        state.level as usize,
                    )
                    .into(),
                )]
            }
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
                vec![]
            }
            Event::Text(text) if in_heading.is_some() => {
                heading_text.push_str(&text);
                vec![]
            }
            _ => vec![event],
        })
        .collect();

    let mut html_output = String::new();
    html::push_html(&mut html_output, events.into_iter());

    Ok(MarkdownOutput {
        html: html_output,
        toc: toc_entries,
    })
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

/// Convert a string to a slug.
fn slugify(s: &str) -> String {
    s.to_lowercase()
        .replace(" ", "-")
        .replace(|c: char| !c.is_alphanumeric() && c != '-', "")
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
