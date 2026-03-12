use clap::{Parser, Subcommand};
use mastracode::{RunOptions, ready_message, render_output, run_headless};

#[derive(Parser)]
#[command(name = "mastracode")]
#[command(about = "Minimal headless MastraCode runner")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    Run {
        #[arg(long)]
        prompt: String,
        #[arg(long)]
        thread_id: Option<String>,
        #[arg(long)]
        resource_id: Option<String>,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        Some(Command::Run {
            prompt,
            thread_id,
            resource_id,
            json,
        }) => {
            let output = run_headless(RunOptions {
                prompt,
                thread_id,
                resource_id,
                json,
            })
            .await?;
            println!("{}", render_output(&output, json));
        }
        None => {
            println!("{}", ready_message());
        }
    }

    Ok(())
}
