use std::{fs, path::PathBuf};

use clap::{Parser, Subcommand};

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
        Command::New { path } => scaffold(path)?,
    }

    Ok(())
}

fn scaffold(path: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    fs::create_dir_all(path.join("src"))?;
    fs::write(
        path.join("Cargo.toml"),
        "[package]\nname = \"my-mastra-app\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[dependencies]\nmastra-core = \"0.1.0\"\n",
    )?;
    fs::write(
        path.join("src/main.rs"),
        "fn main() {\n    println!(\"Welcome to Mastra Rust\");\n}\n",
    )?;
    println!("created {}", path.display());
    Ok(())
}
