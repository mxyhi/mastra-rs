use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "mastracode")]
#[command(about = "Rust port placeholder for MastraCode")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    Run {
        #[arg(long)]
        prompt: String,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Some(Command::Run { prompt }) => {
            println!("mastracode received prompt: {prompt}");
        }
        None => {
            println!("mastracode ready");
        }
    }
}
