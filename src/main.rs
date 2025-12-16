use std::path::PathBuf;

use clap::{Parser, Subcommand};

pub mod build;
pub mod commands;
pub mod config;
pub mod git;
pub mod theme;
pub mod util;

#[derive(Parser)]
struct Args {
    /// The command to execute
    #[command(subcommand)]
    command: UndoxCommand,
}

#[derive(Parser)]
pub struct InitArgs {
    /// The path to initialize the project in
    path: PathBuf,

    /// Whether to create the directory if it doesn't exist
    #[arg(short, long, default_value = "false")]
    create: bool,

    /// Whether to overwrite files in the directory if they exist
    #[arg(short, long, default_value = "false")]
    force: bool,
}

#[derive(Parser)]
pub struct BuildArgs {
    /// The path to the configuration file
    #[arg(short, long, alias = "config", default_value = "undox.yaml")]
    config_file: Option<PathBuf>,
}

#[derive(Parser)]
pub struct ServeArgs {
    /// The address to bind to
    #[arg(short, long, default_value = "0.0.0.0")]
    bind: String,

    /// The port to bind to
    #[arg(short, long, default_value = "3000")]
    port: u16,

    /// Open the project in the default browser
    #[arg(short, long, default_value = "false")]
    open: bool,

    /// The path to the configuration file
    #[arg(short, long, alias = "config", default_value = "undox.yaml")]
    config_file: Option<PathBuf>,

    /// Whether to watch for changes and rebuild automatically (default: true)
    #[arg(short, long, default_value = "true")]
    watch: bool,
}

#[derive(Parser)]
pub struct CleanArgs {
    /// The path to the configuration file
    #[arg(short, long, alias = "config", default_value = "undox.yaml")]
    config_file: Option<PathBuf>,

    /// Show what will be deleted, but don't delete anything
    #[arg(short, long, default_value = "false")]
    dry_run: bool,
}

#[derive(Subcommand)]
enum UndoxCommand {
    /// Initialize a new undox project
    Init(InitArgs),

    /// Build the undox project
    Build(BuildArgs),

    /// Serve the undox project on a local port
    Serve(ServeArgs),

    /// Delete the generated site folder and the undox cache folder
    Clean(CleanArgs),
}

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let args = Args::parse();

    match args.command {
        UndoxCommand::Init(args) => {
            commands::init::run(&args).await?;
        }
        UndoxCommand::Build(args) => {
            commands::build::run(&args).await?;
        }
        UndoxCommand::Serve(args) => {
            commands::serve::run(&args).await?;
        }
        UndoxCommand::Clean(args) => {
            commands::clean::run(&args).await?;
        }
    }

    Ok(())
}
