use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use axum::Router;
use tokio::sync::Mutex;
use tower_http::services::ServeDir;

use crate::{
    build::{
        base_path_from_config, build_search_index, Builder, FileWatcher, PathClassifier,
        WatchEvent, WatchPaths,
    },
    config::{Config, RootConfig},
    theme::ThemeConfig,
    ServeArgs,
};

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

    // Build the site first
    println!("Building site...");
    let result = do_build(&root_config, &base_path, parent_path.as_ref()).await?;

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
                    .map(|resolved| (source.name.clone(), resolved.local_path))
            })
            .collect();

        let watch_paths = WatchPaths {
            source_dirs: source_dirs.clone(),
            theme_dir: result.theme_path.clone(),
            config_path: config_path.clone(),
        };

        let classifier = PathClassifier::new(source_dirs, result.theme_path.clone(), config_path.clone());

        let watch_config = root_config.dev.watch.clone();
        match FileWatcher::new(&watch_config, &watch_paths, classifier) {
            Ok(watcher) => {
                println!("Watching for changes...");

                // Spawn rebuild task
                let rebuild_config = root_config.clone();
                let rebuild_base = base_path.clone();
                let rebuild_parent = parent_path.clone();
                let rebuild_output = result.output_dir.clone();
                let rebuild_theme = result.theme_path.clone();
                let pagefind_config = theme_config.pagefind.clone();

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

                                rt.block_on(async {
                                    match do_build(&rebuild_config, &rebuild_base, rebuild_parent.as_ref()).await {
                                        Ok(result) => {
                                            println!(
                                                "Rebuilt {} documents, {} static files",
                                                result.documents, result.static_files
                                            );
                                            // Rebuild search index
                                            match build_search_index(&rebuild_output, &pagefind_config).await {
                                                Ok(count) => println!("Re-indexed {} pages", count),
                                                Err(e) => eprintln!("Search index error: {}", e),
                                            }
                                        }
                                        Err(e) => {
                                            eprintln!("Build error: {}", e);
                                        }
                                    }
                                });
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

    let app = Router::new().fallback_service(serve_dir);

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
    if args.open {
        if let Err(e) = open::that(&url) {
            eprintln!("Failed to open browser: {}", e);
        }
    }

    // Start the server
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

/// Helper function to run the build
async fn do_build(
    config: &RootConfig,
    base_path: &PathBuf,
    parent_path: Option<&PathBuf>,
) -> Result<crate::build::BuildResult, anyhow::Error> {
    let mut builder = Builder::new(config.clone(), base_path.clone());
    if let Some(parent_path) = parent_path {
        builder = builder.with_theme_base_path(parent_path.clone());
    }
    Ok(builder.build().await?)
}
