use std::collections::HashMap;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use axum::Router;
use axum::extract::State;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::routing::get;
use futures_util::stream::Stream;
use tokio::sync::broadcast;
use tower_http::services::ServeDir;

use crate::{
    ServeArgs,
    build::{
        Builder, FileWatcher, PathClassifier, WatchEvent, WatchPaths, base_path_from_config,
        build_search_index,
    },
    config::{Config, RootConfig},
    theme::ThemeConfig,
};

/// SSE handler for live reload notifications.
async fn live_reload_handler(
    State(tx): State<broadcast::Sender<()>>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let rx = tx.subscribe();
    let stream = async_stream::stream! {
        let mut rx = rx;
        loop {
            match rx.recv().await {
                Ok(_) => {
                    yield Ok(Event::default().event("reload").data("reload"));
                }
                Err(broadcast::error::RecvError::Lagged(_)) => {
                    // Missed some messages, but that's fine - we just need the latest
                    continue;
                }
                Err(broadcast::error::RecvError::Closed) => {
                    break;
                }
            }
        }
    };
    Sse::new(stream).keep_alive(KeepAlive::default())
}

pub async fn run(args: &ServeArgs) -> Result<(), anyhow::Error> {
    // Determine the config file path
    let config_path = args
        .config_file
        .clone()
        .unwrap_or_else(|| "undox.yaml".into());
    let config_path = if config_path.is_relative() {
        std::env::current_dir()?.join(&config_path)
    } else {
        config_path
    };

    let config = Config::load_from_arg(Some(config_path.as_path())).await?;

    // Get the base path for resolving relative paths
    let base_path = base_path_from_config(&config_path);

    // Resolve config to root config and optional parent path
    let (root_config, parent_path) = match config {
        Config::Root(root) => (root, None),
        Config::Child(child) => {
            // Resolve child config by fetching parent
            let cache_dir = base_path.join(".undox/cache/git");
            let resolved = child.resolve(&base_path, &cache_dir)?;
            (resolved.config, Some(resolved.parent_path))
        }
    };

    // Create broadcast channel for live reload
    let (reload_tx, _) = broadcast::channel::<()>(16);

    // Build the site first
    println!("Building site...");
    let result = do_build(&root_config, &base_path, parent_path.as_deref(), true).await?;

    println!(
        "Built {} documents, {} static files",
        result.documents, result.static_files
    );

    // Build search index
    let theme_config = ThemeConfig::load(&result.theme_path)?;
    print!("Building search index...");
    let page_count = build_search_index(&result.output_dir, &theme_config.pagefind).await?;
    println!(" indexed {} pages", page_count);

    // Set up file watcher if enabled
    let _watcher_handle = if args.watch {
        // Collect source directories to watch
        let cache_dir = base_path.join(".undox/cache/git");
        let source_dirs: HashMap<String, PathBuf> = root_config
            .sources
            .iter()
            .filter_map(|source| {
                use crate::build::source::ResolvedSource;
                ResolvedSource::resolve(source.clone(), &base_path, &cache_dir)
                    .ok()
                    .map(|resolved| {
                        // Canonicalize the path to ensure consistent matching with file events
                        let canonical = resolved
                            .local_path
                            .canonicalize()
                            .unwrap_or(resolved.local_path);
                        (source.name.clone(), canonical)
                    })
            })
            .collect();

        let watch_paths = WatchPaths {
            source_dirs: source_dirs.clone(),
            theme_dir: result.theme_path.clone(),
            config_path: config_path.clone(),
        };

        let classifier =
            PathClassifier::new(source_dirs, result.theme_path.clone(), config_path.clone());

        let watch_config = root_config.dev.watch.clone();
        match FileWatcher::new(&watch_config, &watch_paths, classifier) {
            Ok(watcher) => {
                println!("Watching for changes...");

                // Spawn rebuild task
                let rebuild_config = root_config.clone();
                let rebuild_base = base_path.clone();
                let rebuild_parent = parent_path.clone();
                let rebuild_output = result.output_dir.clone();
                let pagefind_config = theme_config.pagefind.clone();
                let watcher_reload_tx = reload_tx.clone();

                Some(tokio::task::spawn_blocking(move || {
                    while let Some(event) = watcher.recv() {
                        match event {
                            WatchEvent::FilesChanged(changes) => {
                                println!("\nDetected {} change(s), rebuilding...", changes.len());

                                // Create a new runtime for the rebuild
                                let rt = tokio::runtime::Builder::new_current_thread()
                                    .enable_all()
                                    .build()
                                    .expect("Failed to create runtime");

                                let rebuild_succeeded = rt.block_on(async {
                                    match do_build(
                                        &rebuild_config,
                                        &rebuild_base,
                                        rebuild_parent.as_deref(),
                                        true,
                                    )
                                    .await
                                    {
                                        Ok(result) => {
                                            println!(
                                                "Rebuilt {} documents, {} static files",
                                                result.documents, result.static_files
                                            );
                                            // Rebuild search index
                                            match build_search_index(
                                                &rebuild_output,
                                                &pagefind_config,
                                            )
                                            .await
                                            {
                                                Ok(count) => println!("Re-indexed {} pages", count),
                                                Err(e) => eprintln!("Search index error: {}", e),
                                            }
                                            true
                                        }
                                        Err(e) => {
                                            eprintln!("Build error: {}", e);
                                            false
                                        }
                                    }
                                });

                                // Notify connected browsers to reload
                                if rebuild_succeeded {
                                    let _ = watcher_reload_tx.send(());
                                }
                            }
                            WatchEvent::Error(e) => {
                                eprintln!("Watch error: {}", e);
                            }
                        }
                    }
                }))
            }
            Err(e) => {
                eprintln!("Warning: Failed to start file watcher: {}", e);
                None
            }
        }
    } else {
        None
    };

    // Create the static file server
    let serve_dir = ServeDir::new(&result.output_dir).append_index_html_on_directories(true);

    // Build router with SSE endpoint for live reload
    let app = Router::new()
        .route("/_undox/live-reload", get(live_reload_handler))
        .with_state(reload_tx)
        .fallback_service(serve_dir);

    // Parse the address
    let addr: SocketAddr = format!("{}:{}", args.bind, args.port).parse()?;

    // Determine the URL to display
    let display_host = if args.bind == "0.0.0.0" {
        "localhost"
    } else {
        &args.bind
    };
    let url = format!("http://{}:{}", display_host, args.port);

    println!("\nServing site at {}", url);
    println!("Press Ctrl+C to stop\n");

    // Open browser if requested
    if args.open
        && let Err(e) = open::that(&url)
    {
        eprintln!("Failed to open browser: {}", e);
    }

    // Start the server
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

/// Helper function to run the build
async fn do_build(
    config: &RootConfig,
    base_path: &Path,
    parent_path: Option<&Path>,
    dev_mode: bool,
) -> Result<crate::build::BuildResult, anyhow::Error> {
    let mut builder = Builder::new(config.clone(), base_path.to_path_buf())
        .with_dev_mode(dev_mode)
        .with_live_reload(config.dev.live_reload);
    if let Some(parent_path) = parent_path {
        builder = builder.with_theme_base_path(parent_path.to_path_buf());
    }
    Ok(builder.build().await?)
}
