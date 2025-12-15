//! Markdown rendering with syntax highlighting and TOC extraction.

use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd, html};

use super::highlight::SyntaxHighlighter;
use super::render::TocEntry;
use crate::config::MarkdownConfig;

#[derive(thiserror::Error, Debug)]
pub enum MarkdownError {
    #[error("invalid markdown extension: {0}")]
    InvalidExtension(String),
}

/// Result of rendering markdown, containing both HTML and table of contents.
pub struct MarkdownOutput {
    pub html: String,
    pub toc: Vec<TocEntry>,
}

/// Render markdown to HTML using pulldown-cmark with syntax highlighting.
pub fn render_markdown(
    markdown: &str,
    highlighter: &SyntaxHighlighter,
    markdown_config: &MarkdownConfig,
) -> Result<MarkdownOutput, MarkdownError> {
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
            other => return Err(MarkdownError::InvalidExtension(other.to_string())),
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
            Event::Start(Tag::Heading {
                level,
                ref id,
                ref classes,
                ref attrs,
            }) => {
                // If heading already has an id, just pass it through
                if let Some(existing_id) = id {
                    used_heading_ids.insert(existing_id.to_string());
                    return vec![event];
                }
                // Otherwise, capture the heading to generate an id
                in_heading = Some(HeadingState {
                    level,
                    classes: classes.iter().map(|c| c.to_string()).collect(),
                    attrs: attrs
                        .iter()
                        .map(|(k, v)| (k.to_string(), v.as_ref().map(|v| v.to_string())))
                        .collect(),
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
                let extra_attrs = state
                    .attrs
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

/// Convert a string to a slug suitable for use as an HTML id.
fn slugify(s: &str) -> String {
    s.to_lowercase()
        .replace(' ', "-")
        .replace(|c: char| !c.is_alphanumeric() && c != '-', "")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slugify() {
        assert_eq!(slugify("Hello World"), "hello-world");
        assert_eq!(slugify("What's New?"), "whats-new");
        assert_eq!(slugify("API Reference"), "api-reference");
    }

    #[test]
    fn test_render_basic_markdown() {
        let highlighter = SyntaxHighlighter::default();
        let config = MarkdownConfig::default();

        let output = render_markdown("# Hello\n\nWorld", &highlighter, &config).unwrap();

        assert!(output.html.contains("Hello"));
        assert!(output.html.contains("<p>World</p>"));
        assert_eq!(output.toc.len(), 1);
        assert_eq!(output.toc[0].text, "Hello");
        assert_eq!(output.toc[0].level, 1);
    }

    #[test]
    fn test_render_code_block() {
        let highlighter = SyntaxHighlighter::default();
        let config = MarkdownConfig::default();

        let output = render_markdown("```rust\nlet x = 1;\n```", &highlighter, &config).unwrap();

        assert!(output.html.contains("let"));
        assert!(output.html.contains("<pre"));
    }

    #[test]
    fn test_invalid_extension() {
        let highlighter = SyntaxHighlighter::default();
        let config = MarkdownConfig {
            extensions: vec!["not_a_real_extension".to_string()],
        };

        let result = render_markdown("# Test", &highlighter, &config);
        assert!(result.is_err());
    }
}
