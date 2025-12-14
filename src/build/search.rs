use std::path::Path;

use pagefind::api::PagefindIndex;
use pagefind::options::PagefindServiceConfig;

use crate::theme::PagefindConfig;

#[derive(thiserror::Error, Debug)]
pub enum SearchError {
    #[error("failed to create search index: {0}")]
    IndexCreation(String),

    #[error("failed to index directory: {0}")]
    Indexing(String),

    #[error("failed to write search files: {0}")]
    WriteFiles(String),
}

/// Build a search index for the output directory using pagefind.
pub async fn build_search_index(
    output_dir: &Path,
    pagefind_config: &PagefindConfig,
) -> Result<usize, SearchError> {
    // Configure pagefind from theme settings
    let language = pagefind_config
        .force_language
        .clone()
        .unwrap_or_else(|| "en".to_string());

    let config = PagefindServiceConfig::builder()
        .keep_index_url(false)
        .root_selector(pagefind_config.root_selector.clone())
        .exclude_selectors(pagefind_config.exclude_selectors.clone())
        .force_language(language)
        .build();

    // Create the index
    let mut index = PagefindIndex::new(Some(config))
        .map_err(|e| SearchError::IndexCreation(e.to_string()))?;

    // Index the output directory
    let output_dir_str = output_dir.to_string_lossy().to_string();
    let page_count = index
        .add_directory(output_dir_str, Some("**/*.html".to_string()))
        .await
        .map_err(|e| SearchError::Indexing(e.to_string()))?;

    // Write the search files to output_dir/_pagefind/
    let pagefind_dir = output_dir.join("_pagefind");
    index
        .write_files(Some(pagefind_dir.to_string_lossy().to_string()))
        .await
        .map_err(|e| SearchError::WriteFiles(e.to_string()))?;

    Ok(page_count)
}
