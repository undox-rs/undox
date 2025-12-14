use crate::{BuildArgs, build::{Builder, base_path_from_config, build_search_index}, config::Config, theme::ThemeConfig};

pub async fn run(args: &BuildArgs) -> Result<(), anyhow::Error> {
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

    // Extract root config (child config resolution is TODO)
    let root_config = match config {
        Config::Root(root) => root,
        Config::Child(_child) => {
            // TODO: Resolve child config by loading parent and merging
            return Err(anyhow::anyhow!(
                "Child config support not yet implemented. Use a root config file."
            ));
        }
    };

    // Get the base path for resolving relative paths
    let base_path = base_path_from_config(&config_path);

    // Build the site
    // Future: Using notify, we can invalidate certain files and rebuild
    // incrementally. We should be able to register callbacks for changes.
    let builder = Builder::new(root_config, base_path);
    let result = builder.build().await?;

    println!(
        "Built site to {} ({} documents, {} static files)",
        result.output_dir.display(),
        result.documents,
        result.static_files
    );

    // Load theme config for pagefind settings
    let theme_config = ThemeConfig::load(&result.theme_path)?;

    // Build search index
    print!("Building search index...");
    let page_count = build_search_index(&result.output_dir, &theme_config.pagefind).await?;
    println!(" indexed {} pages", page_count);

    Ok(())
}
