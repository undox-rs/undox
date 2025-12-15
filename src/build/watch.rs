//! File watching for automatic rebuilds.
//!
//! Uses `notify-debouncer-full` to watch source directories, templates,
//! and config files for changes.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

use notify::event::ModifyKind;
use notify::{
    Config as NotifyConfig, EventKind, PollWatcher, RecommendedWatcher, RecursiveMode, Watcher,
};
use notify_debouncer_full::{
    DebounceEventResult, Debouncer, RecommendedCache, new_debouncer, new_debouncer_opt,
};

use super::cache::ChangeKind;
use crate::config::WatchConfig;

// =============================================================================
// Errors
// =============================================================================

#[derive(thiserror::Error, Debug)]
pub enum WatchError {
    #[error("notify error: {0}")]
    Notify(#[from] notify::Error),

    #[allow(dead_code)]
    #[error("failed to create watcher channel")]
    ChannelCreate,
}

// =============================================================================
// Watch events
// =============================================================================

/// Events sent from the file watcher.
#[derive(Debug)]
pub enum WatchEvent {
    /// Files changed, rebuild needed.
    FilesChanged(Vec<ChangeKind>),
    /// Watcher error occurred.
    Error(String),
}

// =============================================================================
// Path classification
// =============================================================================

/// Paths to watch for changes.
pub struct WatchPaths {
    /// Source directories to watch (source_name -> directory path).
    pub source_dirs: HashMap<String, PathBuf>,
    /// Theme directory (for template changes).
    pub theme_dir: PathBuf,
    /// Config file path.
    pub config_path: PathBuf,
}

/// Classifies file paths into change types.
#[derive(Clone)]
pub struct PathClassifier {
    /// Source name -> source directory mapping.
    source_dirs: HashMap<String, PathBuf>,
    /// Theme directory path.
    theme_dir: PathBuf,
    /// Config file path.
    config_path: PathBuf,
    /// Theme config file path.
    theme_config_path: PathBuf,
}

impl PathClassifier {
    /// Create a new path classifier.
    pub fn new(
        source_dirs: HashMap<String, PathBuf>,
        theme_dir: PathBuf,
        config_path: PathBuf,
    ) -> Self {
        let theme_config_path = theme_dir.join("undox-theme.yaml");

        Self {
            source_dirs,
            theme_dir,
            config_path,
            theme_config_path,
        }
    }

    /// Classify a changed path into a ChangeKind.
    pub fn classify(&self, path: &Path, deleted: bool) -> Option<ChangeKind> {
        // Skip hidden files and directories
        if path
            .components()
            .any(|c| c.as_os_str().to_string_lossy().starts_with('.'))
        {
            return None;
        }

        // Check if it's the main config
        if path == self.config_path {
            return Some(ChangeKind::Config);
        }

        // Check if it's the theme config
        if path == self.theme_config_path {
            return Some(ChangeKind::ThemeConfig);
        }

        // Check if it's a template
        if path.starts_with(&self.theme_dir) {
            if path.extension().is_some_and(|e| e == "html") {
                return Some(ChangeKind::Template {
                    path: path.to_path_buf(),
                });
            }
            // Other theme files (CSS, etc.) - ignore for now
            return None;
        }

        // Check which source it belongs to
        for (source_name, source_dir) in &self.source_dirs {
            if path.starts_with(source_dir) {
                let ext = path.extension().and_then(|e| e.to_str());

                return match ext {
                    Some("md") | Some("markdown") => Some(ChangeKind::Document {
                        source_name: source_name.clone(),
                        path: path.to_path_buf(),
                        deleted,
                    }),
                    _ => Some(ChangeKind::StaticFile {
                        source_name: source_name.clone(),
                        path: path.to_path_buf(),
                        deleted,
                    }),
                };
            }
        }

        None // Unknown path, ignore
    }
}

// =============================================================================
// File watcher
// =============================================================================

/// A file watcher that can use either native or polling backend.
pub enum FileWatcher {
    /// Native file system watcher (recommended for local development).
    Native {
        _debouncer: Debouncer<RecommendedWatcher, RecommendedCache>,
        rx: Receiver<WatchEvent>,
    },
    /// Polling-based watcher (for network filesystems, Docker, etc.).
    Polling {
        _debouncer: Debouncer<PollWatcher, RecommendedCache>,
        rx: Receiver<WatchEvent>,
    },
}

impl FileWatcher {
    /// Create a new file watcher.
    pub fn new(
        config: &WatchConfig,
        paths: &WatchPaths,
        classifier: PathClassifier,
    ) -> Result<Self, WatchError> {
        let debounce_timeout = Duration::from_millis(config.debounce_ms);

        // Create channel for events
        let (tx, rx) = mpsc::channel();

        // Callback to convert notify events to our WatchEvent type
        let classifier = classifier.clone();
        let callback = move |result: DebounceEventResult| {
            match result {
                Ok(events) => {
                    let changes: Vec<ChangeKind> = events
                        .iter()
                        .filter_map(|event| {
                            let deleted = matches!(event.kind, EventKind::Remove(_));
                            // Only process events for actual file changes
                            if !is_relevant_event(&event.kind) {
                                return None;
                            }
                            // Classify the first path (usually there's only one)
                            event
                                .paths
                                .first()
                                .and_then(|p| classifier.classify(p, deleted))
                        })
                        .collect();

                    if !changes.is_empty() {
                        let _ = tx.send(WatchEvent::FilesChanged(changes));
                    }
                }
                Err(errors) => {
                    for e in errors {
                        let _ = tx.send(WatchEvent::Error(e.to_string()));
                    }
                }
            }
        };

        if config.poll {
            // Use polling watcher
            let poll_interval = Duration::from_millis(config.poll_interval_ms);
            let notify_config = NotifyConfig::default().with_poll_interval(poll_interval);

            let mut debouncer = new_debouncer_opt::<_, PollWatcher, RecommendedCache>(
                debounce_timeout,
                None,
                callback,
                RecommendedCache::default(),
                notify_config,
            )
            .map_err(WatchError::Notify)?;

            add_watch_paths_to_debouncer(&mut debouncer, paths)?;

            Ok(FileWatcher::Polling {
                _debouncer: debouncer,
                rx,
            })
        } else {
            // Use native watcher
            let mut debouncer =
                new_debouncer(debounce_timeout, None, callback).map_err(WatchError::Notify)?;

            add_watch_paths_to_debouncer(&mut debouncer, paths)?;

            Ok(FileWatcher::Native {
                _debouncer: debouncer,
                rx,
            })
        }
    }

    /// Receive the next watch event (blocking).
    pub fn recv(&self) -> Option<WatchEvent> {
        match self {
            FileWatcher::Native { rx, .. } => rx.recv().ok(),
            FileWatcher::Polling { rx, .. } => rx.recv().ok(),
        }
    }
}

/// Add watch paths to a debouncer.
fn add_watch_paths_to_debouncer<W: Watcher, C: notify_debouncer_full::FileIdCache>(
    debouncer: &mut Debouncer<W, C>,
    paths: &WatchPaths,
) -> Result<(), WatchError> {
    // Watch all source directories
    for source_dir in paths.source_dirs.values() {
        if source_dir.exists() {
            debouncer.watch(source_dir, RecursiveMode::Recursive)?;
        }
    }

    // Watch theme directory for template changes
    if paths.theme_dir.exists() {
        debouncer.watch(&paths.theme_dir, RecursiveMode::Recursive)?;
    }

    // Watch config file's parent directory (to catch config changes)
    if let Some(parent) = paths.config_path.parent()
        && parent.exists()
    {
        debouncer.watch(parent, RecursiveMode::NonRecursive)?;
    }

    Ok(())
}

/// Check if an event kind is relevant for rebuilds.
fn is_relevant_event(kind: &EventKind) -> bool {
    matches!(
        kind,
        EventKind::Create(_)
            | EventKind::Remove(_)
            | EventKind::Modify(ModifyKind::Data(_))
            | EventKind::Modify(ModifyKind::Name(_))
    )
}
