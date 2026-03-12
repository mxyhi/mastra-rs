use std::{io::Read, path::PathBuf, process::ExitCode, time::Duration};

use clap::{Parser, Subcommand};
use mastracode::{OutputFormat, RunOptions, ready_message, render_output, run_headless};

#[derive(Parser)]
#[command(name = "mastracode")]
#[command(about = "Persistent headless MastraCode runner")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    Run {
        #[arg(long, short = 'p')]
        prompt: String,
        #[arg(long, short = 'c', default_value_t = false)]
        continue_latest: bool,
        #[arg(long)]
        thread_id: Option<String>,
        #[arg(long)]
        resource_id: Option<String>,
        #[arg(long, value_enum, default_value_t = OutputFormat::Default)]
        format: OutputFormat,
        #[arg(long)]
        timeout: Option<u64>,
        #[arg(long, hide = true)]
        storage_path: Option<PathBuf>,
    },
}

fn resolve_prompt(prompt: String) -> Result<String, String> {
    if prompt != "-" {
        return Ok(prompt);
    }

    let mut buffer = String::new();
    std::io::stdin()
        .read_to_string(&mut buffer)
        .map_err(|error| format!("read prompt from stdin: {error}"))?;

    let prompt = buffer.trim().to_owned();
    if prompt.is_empty() {
        return Err("stdin prompt was empty".to_owned());
    }

    Ok(prompt)
}

#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();

    match cli.command {
        Some(Command::Run {
            prompt,
            continue_latest,
            thread_id,
            resource_id,
            format,
            timeout,
            storage_path,
        }) => {
            let prompt = match resolve_prompt(prompt) {
                Ok(prompt) => prompt,
                Err(error) => {
                    eprintln!("{error}");
                    return ExitCode::from(1);
                }
            };

            let run = run_headless(RunOptions {
                prompt,
                thread_id,
                continue_latest,
                resource_id,
                format,
                storage_path,
            });

            let result = if let Some(timeout) = timeout {
                match tokio::time::timeout(Duration::from_secs(timeout), run).await {
                    Ok(result) => result,
                    Err(_) => {
                        eprintln!("mastracode run timed out after {timeout}s");
                        return ExitCode::from(2);
                    }
                }
            } else {
                run.await
            };

            match result {
                Ok(output) => {
                    println!("{}", render_output(&output, format));
                    ExitCode::SUCCESS
                }
                Err(error) => {
                    eprintln!("{error}");
                    ExitCode::from(1)
                }
            }
        }
        None => {
            println!("{}", ready_message());
            ExitCode::SUCCESS
        }
    }
}
