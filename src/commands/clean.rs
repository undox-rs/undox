use crate::{CleanArgs, build::base_path_from_config, config::Config};

pub async fn run(args: &CleanArgs) -> Result<(), anyhow::Error> {
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

    // Look up the name of the build folder
    let build_folder = match config {
        Config::Root(root) => root.site.output,
        Config::Child(child) => {
            todo!()
        }
    };

    // Delete the generated site folder
    let site_path = base_path
        .join(build_folder)
        .canonicalize()
        .ok()
        .unwrap_or("_site".into());
    if site_path.exists() {
        if args.dry_run {
            println!("Would delete {}", site_path.display());
        } else {
            tokio::fs::remove_dir_all(&site_path).await?;
            println!("Deleted {}", site_path.display());
        }
    }

    // Delete the undox cache folder
    let cache_path = base_path
        .join(".undox/cache")
        .canonicalize()
        .ok()
        .unwrap_or(base_path.join(".undox/cache").into());
    if cache_path.exists() {
        if args.dry_run {
            println!("Would delete {}", cache_path.display());
        } else {
            tokio::fs::remove_dir_all(&cache_path).await?;
            println!("Deleted {}", cache_path.display());
        }
    }

    Ok(())
}
