use std::path::PathBuf;

use clap::{Parser, Subcommand};

mod build;
mod commands;
mod config;

#[derive(Parser)]
struct Args {
    /// The command to execute
    #[command(subcommand)]
    command: UndoxCommand,
}

#[derive(Parser)]
struct InitArgs {
    /// The path to initialize the project in
    path: PathBuf,

    /// Whether to create the directory if it doesn't exist
    #[arg(short, long, default_value = "false")]
    create: bool,
}

#[derive(Parser)]
struct BuildArgs {
    /// The path to the configuration file
    #[arg(short, long, default_value = "undox.yaml")]
    config_file: Option<PathBuf>,
}

#[derive(Parser)]
struct ServeArgs {
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
    #[arg(short, long, default_value = "undox.yaml")]
    config_file: Option<PathBuf>,

    /// Whether to watch for changes and rebuild automatically
    #[arg(short, long, default_value = "true")]
    watch: bool,
}

#[derive(Subcommand)]
enum UndoxCommand {
    /// Initialize a new Undox project
    Init(InitArgs),

    /// Build the Undox project
    Build(BuildArgs),

    /// Serve the Undox project on a local port
    Serve(ServeArgs),
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
    }

    Ok(())
}
