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
        if let Some(nav_section) = convert_nav_item(item, path_to_doc) {
            result.push(nav_section);
        }
    }

    result
}

/// Convert a single NavItem to a NavSection (recursively handles children).
fn convert_nav_item(
    item: &NavItem,
    path_to_doc: &HashMap<String, &Document>,
) -> Option<NavSection> {
    match item {
        NavItem::Section { section, items } => {
            // Convert section items recursively
            let nav_items: Vec<NavSection> = items
                .iter()
                .filter_map(|item| convert_nav_item(item, path_to_doc))
                .collect();

            if !nav_items.is_empty() {
                Some(NavSection::Section {
                    section: section.clone(),
                    items: nav_items,
                })
            } else {
                None
            }
        }
        NavItem::LinkWithChildren {
            path,
            title,
            children,
        } => {
            // A link with nested children
            if let Some(doc) = path_to_doc.get(path) {
                let child_sections: Vec<NavSection> = children
                    .iter()
                    .filter_map(|child| convert_nav_item(child, path_to_doc))
                    .collect();

                Some(NavSection::Link(NavLink {
                    title: title.clone().unwrap_or_else(|| doc.title()),
                    url: doc.url_path.clone(),
                    children: child_sections,
                }))
            } else {
                None
            }
        }
        NavItem::Titled(map) => {
            // Single titled link
            if let Some((title, path)) = map.iter().next()
                && let Some(doc) = path_to_doc.get(path)
            {
                Some(NavSection::Link(NavLink {
                    title: title.clone(),
                    url: doc.url_path.clone(),
                    children: vec![],
                }))
            } else {
                None
            }
        }
        NavItem::Path(path) => {
            if !path.ends_with('/') {
                // It's a file path
                path_to_doc.get(path).map(|doc| {
                    NavSection::Link(NavLink {
                        title: doc.title(),
                        url: doc.url_path.clone(),
                        children: vec![],
                    })
                })
            } else {
                // Directory paths (ending with /) are not supported in this simple conversion
                None
            }
        }
    }
}

/// A tree node for building hierarchical navigation.
#[derive(Default)]
struct NavTreeNode {
    /// Documents at this level: (is_index, link)
    links: Vec<(bool, NavLink)>,
    /// Subdirectories
    children: HashMap<String, NavTreeNode>,
}

impl NavTreeNode {
    /// Insert a document into the tree at the appropriate depth.
    fn insert(&mut self, path_parts: &[&str], is_index: bool, link: NavLink) {
        if path_parts.len() <= 1 {
            // This is a file at the current level
            self.links.push((is_index, link));
        } else {
            // Navigate into subdirectory
            let dir_name = path_parts[0].to_string();
            self.children
                .entry(dir_name)
                .or_default()
                .insert(&path_parts[1..], is_index, link);
        }
    }

    /// Convert this tree node into a Vec<NavSection>.
    ///
    /// When a link's filename stem matches a child directory name,
    /// the directory contents are merged into the link's `children` field
    /// instead of creating a separate section.
    fn into_nav_sections(mut self) -> Vec<NavSection> {
        let mut result = Vec::new();

        // Sort links (index files come first, then alphabetically)
        self.links.sort_by(|a, b| match (a.0, b.0) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.1.title.cmp(&b.1.title),
        });

        // Build a set of link stems to check for matching directories
        // The stem is derived from the link's URL (last path component)
        let link_stems: std::collections::HashSet<String> = self
            .links
            .iter()
            .filter_map(|(_, link)| {
                link.url
                    .trim_end_matches('/')
                    .rsplit('/')
                    .next()
                    .map(|s| s.to_lowercase())
            })
            .collect();

        // Process links, merging matching directory children
        for (is_index, mut link) in self.links {
            // Find matching child directory by checking the link's URL stem
            let link_stem = link
                .url
                .trim_end_matches('/')
                .rsplit('/')
                .next()
                .map(|s| s.to_lowercase());

            if let Some(stem) = link_stem
                && let Some(child) = self.children.remove(&stem)
            {
                // Merge directory contents into link's children
                link.children = child.into_nav_sections();
            }

            // Skip adding index files as standalone links if they're the only
            // thing at this level (they'll be shown via section navigation)
            if is_index && result.is_empty() && self.children.is_empty() && link.children.is_empty()
            {
                // Still add it - it's the only item
            }

            result.push(NavSection::Link(link));
        }

        // Add remaining children (directories without matching files) as sections
        let mut remaining_children: Vec<_> = self.children.into_iter().collect();
        remaining_children.sort_by(|a, b| a.0.cmp(&b.0));

        for (name, child) in remaining_children {
            // Skip if this directory was already merged with a link
            if link_stems.contains(&name.to_lowercase()) {
                continue;
            }

            let items = child.into_nav_sections();
            if !items.is_empty() {
                result.push(NavSection::Section {
                    section: title_case(&name),
                    items,
                });
            }
        }

        result
    }
}

/// Auto-generate navigation from a list of documents.
///
/// Documents are organized by directory structure:
/// - Root-level documents appear as top-level links
/// - Documents in subdirectories are grouped into sections
/// - Nested directories create nested sections
/// - Index files are sorted first within their level
/// - Section names are derived from directory names using title case
fn auto_generate_nav(mut docs: Vec<&Document>) -> Vec<NavSection> {
    // Sort by source path for consistent ordering
    docs.sort_by(|a, b| a.source_path.cmp(&b.source_path));

    // Build the navigation tree
    let mut root = NavTreeNode::default();

    for doc in docs {
        let is_index = doc.source_path.file_stem().is_some_and(|s| s == "index");
        let link = NavLink {
            title: doc.title(),
            url: doc.url_path.clone(),
            children: vec![],
        };

        let path_str = doc.source_path.to_string_lossy();
        let path_parts: Vec<&str> = path_str.trim_matches('/').split('/').collect();

        root.insert(&path_parts, is_index, link);
    }

    // Convert tree to Vec<NavSection>
    root.into_nav_sections()
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

    #[test]
    fn test_autogenerate_nav_nested() {
        let docs = vec![
            make_doc("cli", "index.md", "/cli"),
            make_doc("cli", "commands/build.md", "/cli/commands/build"),
            make_doc(
                "cli",
                "commands/build/details.md",
                "/cli/commands/build/details",
            ),
        ];
        let doc_refs: Vec<&Document> = docs.iter().collect();

        let nav = auto_generate_nav(doc_refs);

        // Expected structure (with file/directory merging):
        // - Link: Index (/cli)
        // - Section: Commands
        //   - Link: Build (/cli/commands/build)
        //     - children:
        //       - Link: Details (/cli/commands/build/details)
        //
        // The build/ directory contents are merged into the build.md link's children
        // since they share the same name.

        assert_eq!(nav.len(), 2); // One root link, one section

        // First item is the index link
        if let NavSection::Link(link) = &nav[0] {
            assert_eq!(link.title, "Index");
        } else {
            panic!("Expected Link at nav[0]");
        }

        // Second item is the Commands section
        if let NavSection::Section { section, items } = &nav[1] {
            assert_eq!(section, "Commands");
            assert_eq!(items.len(), 1); // Just the Build link (with children)

            // The Build link has the build/ directory contents as children
            if let NavSection::Link(link) = &items[0] {
                assert_eq!(link.title, "Build");
                assert_eq!(link.url, "/cli/commands/build");
                assert_eq!(link.children.len(), 1);

                // The child is the Details link
                if let NavSection::Link(child_link) = &link.children[0] {
                    assert_eq!(child_link.title, "Details");
                    assert_eq!(child_link.url, "/cli/commands/build/details");
                } else {
                    panic!("Expected Link in children");
                }
            } else {
                panic!("Expected Link at items[0]");
            }
        } else {
            panic!("Expected Section at nav[1]");
        }
    }

    #[test]
    fn test_convert_nav_config_link_with_children() {
        // Create documents
        let docs = vec![
            make_doc("docs", "configuration.md", "/docs/configuration"),
            make_doc("docs", "configuration/root.md", "/docs/configuration/root"),
            make_doc("docs", "configuration/sub.md", "/docs/configuration/sub"),
        ];

        // Build path_to_doc lookup
        let path_to_doc: HashMap<String, &Document> = docs
            .iter()
            .map(|doc| {
                let path_str = doc.source_path.to_string_lossy().to_string();
                (path_str, doc)
            })
            .collect();

        // Create nav config with LinkWithChildren
        let nav_config: Vec<NavItem> = vec![NavItem::LinkWithChildren {
            path: "configuration.md".to_string(),
            title: Some("Configuration".to_string()),
            children: vec![
                NavItem::Path("configuration/root.md".to_string()),
                NavItem::Path("configuration/sub.md".to_string()),
            ],
        }];

        let nav = convert_nav_config(&nav_config, &path_to_doc);

        // Should have one link with two children
        assert_eq!(nav.len(), 1);

        if let NavSection::Link(link) = &nav[0] {
            assert_eq!(link.title, "Configuration");
            assert_eq!(link.url, "/docs/configuration");
            assert_eq!(link.children.len(), 2);

            // First child
            if let NavSection::Link(child) = &link.children[0] {
                assert_eq!(child.url, "/docs/configuration/root");
            } else {
                panic!("Expected Link for first child");
            }

            // Second child
            if let NavSection::Link(child) = &link.children[1] {
                assert_eq!(child.url, "/docs/configuration/sub");
            } else {
                panic!("Expected Link for second child");
            }
        } else {
            panic!("Expected Link at nav[0]");
        }
    }
}
