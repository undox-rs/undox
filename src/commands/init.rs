use indoc::indoc;

use crate::InitArgs;

const DEFAULT_CONFIG: &str = indoc! {r#"
site:
  name: My Undox Site
  url: "https://my-undox-site.com"
  output: _site

sources:
  - name: Docs
    title: Docs
    url_prefix: /
    local:
      path: ./content
"#};

const GITIGNORE_CONTENT: &str = indoc! {r#"
_site/
.undox/
"#};

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
                "Directory does not exist: {path}\nUse --create to create it",
                path = path.display()
            ));
        }
    }

    let mut read_dir = tokio::fs::read_dir(path.clone()).await?;
    let is_empty = read_dir.next_entry().await?.is_none();

    if !is_empty && !args.force {
        return Err(anyhow::anyhow!(
            "Directory is not empty: {path}\nUse --force to overwrite",
            path = path.display()
        ));
    }

    println!("Initializing project in {}", path.display());

    let config_text = DEFAULT_CONFIG.to_string();
    tokio::fs::write(path.join("undox.yaml"), config_text).await?;
    tokio::fs::write(path.join(".gitignore"), GITIGNORE_CONTENT).await?;

    tokio::fs::create_dir_all(path.join("content")).await?;
    tokio::fs::write(path.join("content/index.md"), "# Hello, World!").await?;

    println!(
        "Created undox site in {path}",
        path = path.canonicalize().ok().unwrap_or(path).display()
    );

    Ok(())
}
