//! Path and URL conversion utilities.
//!
//! This module handles conversions between:
//! - Source file paths (relative paths within a documentation source)
//! - URL paths (the URL at which content will be served)
//! - Output file paths (where files are written in the output directory)

use std::path::{Path, PathBuf};

/// Convert a markdown file path to a URL path.
///
/// Takes a source-relative path and a URL prefix, produces a URL path.
///
/// # Examples
/// ```ignore
/// path_to_url("installation.md", "/cli") => "/cli/installation"
/// path_to_url("getting-started/quickstart.md", "/cli") => "/cli/getting-started/quickstart"
/// path_to_url("index.md", "/cli") => "/cli"
/// path_to_url("index.md", "") => "/"
/// ```
pub fn source_path_to_url(path: &Path, url_prefix: &str) -> String {
    let mut url = url_prefix.to_string();
    // Ensure we have a trailing slash for appending the path
    // (empty prefix becomes "/" which is then trimmed if needed)
    if url.is_empty() || !url.ends_with('/') {
        url.push('/');
    }

    // Remove .md extension and convert path separators
    let path_str = path.with_extension("").to_string_lossy().to_string();
    let path_str = path_str.replace('\\', "/");

    // Handle index files - they become the directory URL
    let path_str = if path_str.ends_with("/index") || path_str == "index" {
        path_str
            .trim_end_matches("/index")
            .trim_end_matches("index")
            .to_string()
    } else {
        path_str
    };

    url.push_str(&path_str);

    // Normalize: remove trailing slash unless it's the root
    if url.len() > 1 && url.ends_with('/') {
        url.pop();
    }

    // Ensure we have at least a slash
    if url.is_empty() {
        url = "/".to_string();
    }

    url
}

/// Convert a static file path to a URL path.
///
/// Unlike markdown files, static files keep their extension.
///
/// # Examples
/// ```ignore
/// static_path_to_url("images/screenshot.png", "/cli") => "/cli/images/screenshot.png"
/// static_path_to_url("styles.css", "/cli") => "/cli/styles.css"
/// ```
pub fn static_path_to_url(path: &Path, url_prefix: &str) -> String {
    let mut url = url_prefix.to_string();
    // Ensure we have a trailing slash for appending the path
    if url.is_empty() || !url.ends_with('/') {
        url.push('/');
    }

    let path_str = path.to_string_lossy().to_string();
    let path_str = path_str.replace('\\', "/");

    url.push_str(&path_str);
    url
}

/// Convert a URL path to an output file path.
///
/// Documents (no extension) become `path/index.html`.
/// Static files (with extension) keep their path.
///
/// # Examples
/// ```ignore
/// url_to_output_path("/cli/installation", output_dir) => output_dir/cli/installation/index.html
/// url_to_output_path("/", output_dir) => output_dir/index.html
/// url_to_output_path("/cli/style.css", output_dir) => output_dir/cli/style.css
/// ```
pub fn url_to_output_path(url_path: &str, output_dir: &Path) -> PathBuf {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_source_path_to_url_simple() {
        assert_eq!(
            source_path_to_url(Path::new("installation.md"), "/cli"),
            "/cli/installation"
        );
    }

    #[test]
    fn test_source_path_to_url_nested() {
        assert_eq!(
            source_path_to_url(Path::new("getting-started/quickstart.md"), "/cli"),
            "/cli/getting-started/quickstart"
        );
    }

    #[test]
    fn test_source_path_to_url_index() {
        assert_eq!(source_path_to_url(Path::new("index.md"), "/cli"), "/cli");
        assert_eq!(source_path_to_url(Path::new("index.md"), ""), "/");
    }

    #[test]
    fn test_source_path_to_url_nested_index() {
        assert_eq!(
            source_path_to_url(Path::new("guides/index.md"), "/cli"),
            "/cli/guides"
        );
    }

    #[test]
    fn test_source_path_to_url_root_source() {
        // Source with empty url_prefix (root source)
        assert_eq!(
            source_path_to_url(Path::new("installation.md"), ""),
            "/installation"
        );
        assert_eq!(
            source_path_to_url(Path::new("guides/quickstart.md"), ""),
            "/guides/quickstart"
        );
    }

    #[test]
    fn test_static_path_to_url() {
        assert_eq!(
            static_path_to_url(Path::new("images/screenshot.png"), "/cli"),
            "/cli/images/screenshot.png"
        );
        assert_eq!(
            static_path_to_url(Path::new("style.css"), "/cli"),
            "/cli/style.css"
        );
    }

    #[test]
    fn test_url_to_output_path_document() {
        let output = Path::new("/site");
        assert_eq!(
            url_to_output_path("/cli/installation", output),
            PathBuf::from("/site/cli/installation/index.html")
        );
    }

    #[test]
    fn test_url_to_output_path_root() {
        let output = Path::new("/site");
        assert_eq!(
            url_to_output_path("/", output),
            PathBuf::from("/site/index.html")
        );
    }

    #[test]
    fn test_url_to_output_path_static() {
        let output = Path::new("/site");
        assert_eq!(
            url_to_output_path("/cli/style.css", output),
            PathBuf::from("/site/cli/style.css")
        );
    }

    #[test]
    fn test_base_path_from_config() {
        assert_eq!(
            base_path_from_config(Path::new("/project/undox.yaml")),
            PathBuf::from("/project")
        );
        assert_eq!(
            base_path_from_config(Path::new("undox.yaml")),
            PathBuf::from("")
        );
    }
}
