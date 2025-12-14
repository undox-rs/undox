use crate::{
    InitArgs,
    config::{Config, RootConfig, SiteConfig, ThemeConfig},
};

pub async fn run(args: &InitArgs) -> Result<(), anyhow::Error> {
    let path = if args.path.is_relative() {
        std::env::current_dir()?.join(&args.path)
    } else {
        args.path.clone()
    };

    if !path.exists() {
        if args.create {
            tokio::fs::create_dir_all(&path).await?;
            println!("Created directory {path}", path = path.display());
        } else {
            return Err(anyhow::anyhow!(
                "Directory does not exist: {path}",
                path = path.display()
            ));
        }
    }

    let default_config = Config::Root(RootConfig {
        site: SiteConfig {
            name: "My Undox Site".into(),
            url: Some("https://my-undox-site.com".into()),
            output: "_site".into(),
        },
        sources: vec![],
        theme: ThemeConfig::default(),
    });

    println!("Initializing project in {}", path.display());

    let config_text = serde_yaml::to_string(&default_config)?;
    tokio::fs::write(path.join("undox.yaml"), config_text).await?;

    println!(
        "Created config file {config_file}",
        config_file = path.join("undox.yaml").display()
    );

    Ok(())
}
