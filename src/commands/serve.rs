use std::net::SocketAddr;

use axum::Router;
use tower_http::services::ServeDir;

use crate::{
    build::{base_path_from_config, build_search_index, Builder},
    config::Config,
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
    let mut builder = Builder::new(root_config, base_path);
    if let Some(parent_path) = parent_path {
        builder = builder.with_theme_base_path(parent_path);
    }
    let result = builder.build().await?;

    println!(
        "Built {} documents, {} static files",
        result.documents, result.static_files
    );

    // Build search index
    let theme_config = ThemeConfig::load(&result.theme_path)?;
    print!("Building search index...");
    let page_count = build_search_index(&result.output_dir, &theme_config.pagefind).await?;
    println!(" indexed {} pages", page_count);

    // Note about watch mode
    if args.watch {
        println!("Note: Live reload is not yet implemented. Restart the server to see changes.");
    }

    // Create the static file server
    let serve_dir = ServeDir::new(&result.output_dir)
        .append_index_html_on_directories(true);

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
