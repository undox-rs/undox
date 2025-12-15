//! Build cache for incremental rebuilds.
//!
//! Tracks file state and rendered output to determine what needs rebuilding
//! when source files change.
//!
//! Note: This module is prepared for future incremental build support.

#![allow(dead_code)]

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use super::render::NavSection;

// =============================================================================
// Change detection types
// =============================================================================

/// What kind of change was detected in the filesystem.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChangeKind {
    /// A document (markdown file) was added, modified, or deleted.
    Document {
        source_name: String,
        path: PathBuf,
        deleted: bool,
    },
    /// A static file was added, modified, or deleted.
    StaticFile {
        source_name: String,
        path: PathBuf,
        deleted: bool,
    },
    /// A template file changed.
    Template { path: PathBuf },
    /// The main config file changed.
    Config,
    /// The theme config changed.
    ThemeConfig,
}

/// What scope of rebuild is needed based on the changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InvalidationScope {
    /// Rebuild only the specific file(s).
    Files(Vec<PathBuf>),
    /// Rebuild all documents in a source (navigation changed).
    Source(String),
    /// Rebuild all documents (template changed).
    AllDocuments,
    /// Full rebuild (config changed).
    Full,
}

// =============================================================================
// Cached data types
// =============================================================================

/// Cached information about a built document.
#[derive(Debug, Clone)]
pub struct CachedDocument {
    /// Source name this document belongs to.
    pub source_name: String,
    /// Absolute source path.
    pub source_path: PathBuf,
    /// URL path for output.
    pub url_path: String,
    /// Output file path (absolute).
    pub output_path: PathBuf,
    /// Last modified time of source file when cached.
    pub source_mtime: SystemTime,
}

/// Cached information about a static file.
#[derive(Debug, Clone)]
pub struct CachedStaticFile {
    /// Source name this file belongs to.
    pub source_name: String,
    /// Absolute source path.
    pub source_path: PathBuf,
    /// Output file path (absolute).
    pub output_path: PathBuf,
    /// Last modified time when copied.
    pub source_mtime: SystemTime,
}

// =============================================================================
// Build cache
// =============================================================================

/// Tracks the state of all files for incremental rebuilds.
#[derive(Debug, Default)]
pub struct BuildCache {
    /// Document entries: source path -> cached document info.
    documents: HashMap<PathBuf, CachedDocument>,
    /// Static file entries: source path -> cached static info.
    static_files: HashMap<PathBuf, CachedStaticFile>,
    /// Template modification times (path -> mtime).
    template_mtimes: HashMap<PathBuf, SystemTime>,
    /// Navigation cache per source.
    nav_by_source: HashMap<String, Vec<NavSection>>,
    /// Set of known document paths per source (for detecting additions/deletions).
    documents_by_source: HashMap<String, Vec<PathBuf>>,
}

impl BuildCache {
    /// Create a new empty build cache.
    pub fn new() -> Self {
        Self::default()
    }

    /// Determine the invalidation scope based on a list of changes.
    pub fn invalidation_scope(&self, changes: &[ChangeKind]) -> InvalidationScope {
        let mut files_to_rebuild: Vec<PathBuf> = Vec::new();
        let mut sources_needing_nav_rebuild: Vec<String> = Vec::new();
        let mut needs_all_documents = false;

        for change in changes {
            match change {
                ChangeKind::Config | ChangeKind::ThemeConfig => {
                    return InvalidationScope::Full;
                }
                ChangeKind::Template { .. } => {
                    needs_all_documents = true;
                }
                ChangeKind::Document {
                    source_name,
                    path,
                    deleted,
                } => {
                    if *deleted || !self.documents.contains_key(path) {
                        // Document added or deleted - nav changes
                        if !sources_needing_nav_rebuild.contains(source_name) {
                            sources_needing_nav_rebuild.push(source_name.clone());
                        }
                    } else {
                        // Document modified - just rebuild that file
                        files_to_rebuild.push(path.clone());
                    }
                }
                ChangeKind::StaticFile { path, .. } => {
                    files_to_rebuild.push(path.clone());
                }
            }
        }

        if needs_all_documents {
            return InvalidationScope::AllDocuments;
        }

        if !sources_needing_nav_rebuild.is_empty() {
            // For simplicity, if any source needs nav rebuild, rebuild that source
            // In the future, we could handle multiple sources
            return InvalidationScope::Source(sources_needing_nav_rebuild.remove(0));
        }

        if !files_to_rebuild.is_empty() {
            return InvalidationScope::Files(files_to_rebuild);
        }

        // No meaningful changes
        InvalidationScope::Files(vec![])
    }

    /// Check if a document needs rebuilding based on mtime.
    pub fn document_needs_rebuild(&self, path: &Path) -> bool {
        match self.documents.get(path) {
            Some(cached) => std::fs::metadata(path)
                .and_then(|m| m.modified())
                .map(|mtime| mtime > cached.source_mtime)
                .unwrap_or(true),
            None => true,
        }
    }

    /// Check if a static file needs copying based on mtime.
    pub fn static_file_needs_copy(&self, path: &Path) -> bool {
        match self.static_files.get(path) {
            Some(cached) => std::fs::metadata(path)
                .and_then(|m| m.modified())
                .map(|mtime| mtime > cached.source_mtime)
                .unwrap_or(true),
            None => true,
        }
    }

    /// Update cache after building a document.
    pub fn update_document(&mut self, doc: CachedDocument) {
        let source_name = doc.source_name.clone();
        let source_path = doc.source_path.clone();

        self.documents.insert(doc.source_path.clone(), doc);

        // Track document in source's document list
        self.documents_by_source
            .entry(source_name)
            .or_default()
            .push(source_path);
    }

    /// Update cache after copying a static file.
    pub fn update_static_file(&mut self, file: CachedStaticFile) {
        self.static_files.insert(file.source_path.clone(), file);
    }

    /// Remove a document from cache (file deleted).
    pub fn remove_document(&mut self, path: &Path) -> Option<CachedDocument> {
        if let Some(doc) = self.documents.remove(path) {
            // Also remove from source's document list
            if let Some(docs) = self.documents_by_source.get_mut(&doc.source_name) {
                docs.retain(|p| p != path);
            }
            Some(doc)
        } else {
            None
        }
    }

    /// Remove a static file from cache.
    pub fn remove_static_file(&mut self, path: &Path) -> Option<CachedStaticFile> {
        self.static_files.remove(path)
    }

    /// Get all cached documents for a source.
    pub fn documents_for_source(&self, source_name: &str) -> Vec<&CachedDocument> {
        self.documents
            .values()
            .filter(|d| d.source_name == source_name)
            .collect()
    }

    /// Get cached navigation for a source.
    pub fn get_nav(&self, source_name: &str) -> Option<&Vec<NavSection>> {
        self.nav_by_source.get(source_name)
    }

    /// Store computed navigation for a source.
    pub fn set_nav(&mut self, source_name: String, nav: Vec<NavSection>) {
        self.nav_by_source.insert(source_name, nav);
    }

    /// Invalidate navigation for a source.
    pub fn invalidate_nav(&mut self, source_name: &str) {
        self.nav_by_source.remove(source_name);
    }

    /// Update template mtime tracking.
    pub fn update_template_mtime(&mut self, path: PathBuf, mtime: SystemTime) {
        self.template_mtimes.insert(path, mtime);
    }

    /// Check if any template has changed since last check.
    pub fn any_template_changed(&self, template_dir: &Path) -> bool {
        // Walk template directory and check mtimes
        if let Ok(entries) = std::fs::read_dir(template_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().is_some_and(|e| e == "html")
                    && let Ok(meta) = std::fs::metadata(&path)
                    && let Ok(mtime) = meta.modified()
                {
                    match self.template_mtimes.get(&path) {
                        Some(&cached_mtime) if mtime > cached_mtime => return true,
                        None => return true,
                        _ => {}
                    }
                }
            }
        }
        false
    }

    /// Clear the entire cache.
    pub fn clear(&mut self) {
        self.documents.clear();
        self.static_files.clear();
        self.template_mtimes.clear();
        self.nav_by_source.clear();
        self.documents_by_source.clear();
    }

    /// Get the number of cached documents.
    pub fn document_count(&self) -> usize {
        self.documents.len()
    }

    /// Get the number of cached static files.
    pub fn static_file_count(&self) -> usize {
        self.static_files.len()
    }
}
