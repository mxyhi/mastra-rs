use std::path::PathBuf;

use clap::{Parser, Subcommand};
use create_mastra::{default_repo_root, scaffold};

#[derive(Parser)]
#[command(name = "create-mastra")]
#[command(about = "Scaffold a Rust Mastra project")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    New { path: PathBuf },
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        Command::New { path } => {
            scaffold(&path, &default_repo_root())?;
            println!("created {}", path.display());
        }
    }

    Ok(())
}
