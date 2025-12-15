//! Navigation building for documentation sites.
//!
//! Handles both configured navigation (from undox.yaml) and auto-generated
//! navigation based on document structure.

use std::collections::HashMap;
use std::path::PathBuf;

use crate::config::NavItem;
use crate::util::title_case;

use super::document::{ContentItem, Document};
use super::render::{NavLink, NavSection};
use super::source::ResolvedSource;

/// Build navigation structure grouped by source.
///
/// Returns a map from source name to that source's navigation.
/// Each source gets its own isolated navigation containing only its documents.
/// If a source has configured nav, that is used; otherwise, nav is auto-generated.
pub fn build_navigation_by_source(
    items: &[(ContentItem, PathBuf)],
    resolved_sources: &[ResolvedSource],
) -> HashMap<String, Vec<NavSection>> {
    // Group documents by source
    let mut docs_by_source: HashMap<String, Vec<&Document>> = HashMap::new();

    for (item, _) in items {
        if let ContentItem::Document(doc) = item {
            docs_by_source
                .entry(doc.source_name.clone())
                .or_default()
                .push(doc);
        }
    }

    // Build navigation for each source
    let mut nav_by_source = HashMap::new();

    for source in resolved_sources {
        let source_name = &source.config.name;
        let docs = docs_by_source.get(source_name).cloned().unwrap_or_default();

        // Check if source has configured nav
        if let Some(nav_config) = &source.config.nav {
            // Build lookup from relative source path to document URL
            let path_to_doc: HashMap<String, &Document> = docs
                .iter()
                .map(|doc| {
                    let path_str = doc.source_path.to_string_lossy().to_string();
                    (path_str, *doc)
                })
                .collect();

            // Convert NavConfig to Vec<NavSection>
            let nav = convert_nav_config(nav_config, &path_to_doc);
            nav_by_source.insert(source_name.clone(), nav);
        } else {
            // Auto-generate navigation from documents
            let nav = auto_generate_nav(docs);
            nav_by_source.insert(source_name.clone(), nav);
        }
    }

    nav_by_source
}

/// Convert a NavConfig to Vec<NavSection> using document lookup.
fn convert_nav_config(
    nav_config: &[NavItem],
    path_to_doc: &HashMap<String, &Document>,
) -> Vec<NavSection> {
    let mut result = Vec::new();

    for item in nav_config {
        match item {
            NavItem::Section { section, items } => {
                // Convert section items to NavLinks
                let nav_links: Vec<NavLink> = items
                    .iter()
                    .filter_map(|item| nav_item_to_link(item, path_to_doc))
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
                if let Some((title, path)) = map.iter().next()
                    && let Some(doc) = path_to_doc.get(path)
                {
                    result.push(NavSection::Link(NavLink {
                        title: title.clone(),
                        url: doc.url_path.clone(),
                    }));
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
fn nav_item_to_link(item: &NavItem, path_to_doc: &HashMap<String, &Document>) -> Option<NavLink> {
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
///
/// Documents are organized by directory structure:
/// - Root-level documents appear as top-level links
/// - Documents in subdirectories are grouped into sections
/// - Index files are sorted first within their level
/// - Section names are derived from directory names using title case
fn auto_generate_nav(mut docs: Vec<&Document>) -> Vec<NavSection> {
    // Sort by source path (relative path within the source) for consistent ordering
    docs.sort_by(|a, b| a.source_path.cmp(&b.source_path));

    // Group by directory within the source
    // Store (is_index, NavLink) tuples for sorting
    let mut sections: HashMap<String, Vec<(bool, NavLink)>> = HashMap::new();
    let mut root_links: Vec<(bool, NavLink)> = Vec::new();

    for doc in docs {
        let is_index = doc.source_path.file_stem().is_some_and(|s| s == "index");

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
            sections
                .entry(section_name)
                .or_default()
                .push((is_index, link));
        }
    }

    // Build the nav structure for this source
    let mut nav: Vec<NavSection> = Vec::new();

    // Sort root links: index first, then alphabetically by title
    root_links.sort_by(|a, b| match (a.0, b.0) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.1.title.cmp(&b.1.title),
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
            links.sort_by(|a, b| match (a.0, b.0) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.1.title.cmp(&b.1.title),
            });
            nav.push(NavSection::Section {
                section: title_case(&section_name),
                items: links.into_iter().map(|(_, link)| link).collect(),
            });
        }
    }

    nav
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::build::document::FrontMatter;
    use std::path::PathBuf;

    fn make_doc(source_name: &str, source_path: &str, url_path: &str) -> Document {
        Document {
            source_name: source_name.to_string(),
            source_path: PathBuf::from(source_path),
            url_path: url_path.to_string(),
            front_matter: FrontMatter::default(),
            raw_content: String::new(),
        }
    }

    #[test]
    fn test_auto_generate_nav_simple() {
        let docs = vec![
            make_doc("cli", "index.md", "/cli"),
            make_doc("cli", "installation.md", "/cli/installation"),
            make_doc("cli", "usage.md", "/cli/usage"),
        ];
        let doc_refs: Vec<&Document> = docs.iter().collect();

        let nav = auto_generate_nav(doc_refs);

        assert_eq!(nav.len(), 3);
        // Index should be first
        if let NavSection::Link(link) = &nav[0] {
            assert_eq!(link.url, "/cli");
        } else {
            panic!("Expected Link");
        }
    }

    #[test]
    fn test_auto_generate_nav_with_sections() {
        let docs = vec![
            make_doc("cli", "index.md", "/cli"),
            make_doc("cli", "commands/build.md", "/cli/commands/build"),
            make_doc("cli", "commands/serve.md", "/cli/commands/serve"),
        ];
        let doc_refs: Vec<&Document> = docs.iter().collect();

        let nav = auto_generate_nav(doc_refs);

        assert_eq!(nav.len(), 2); // One root link, one section
        if let NavSection::Section { section, items } = &nav[1] {
            assert_eq!(section, "Commands");
            assert_eq!(items.len(), 2);
        } else {
            panic!("Expected Section");
        }
    }
}
