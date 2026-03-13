use std::{io::Read, path::PathBuf, process::ExitCode, time::Duration};

use clap::{Args, Parser, Subcommand};
use mastracode::{OutputFormat, RunOptions, ready_message, render_output, run_headless};

#[derive(Parser)]
#[command(name = "mastracode")]
#[command(about = "Persistent headless MastraCode runner")]
struct Cli {
    #[command(flatten)]
    run: RunCommand,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Clone, Args)]
struct RunCommand {
    #[arg(long, short = 'p')]
    prompt: Option<String>,
    // Upstream headless mode uses `--continue` / `-c`; keep `--continue-latest`
    // as a compatibility alias for the Rust port's earlier wording.
    #[arg(
        long = "continue",
        visible_alias = "continue-latest",
        short = 'c',
        default_value_t = false
    )]
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
}

#[derive(Debug, Subcommand)]
enum Command {
    Run(RunCommand),
}

impl RunCommand {
    fn has_any_input(&self) -> bool {
        self.prompt.is_some()
            || self.continue_latest
            || self.thread_id.is_some()
            || self.resource_id.is_some()
            || !matches!(self.format, OutputFormat::Default)
            || self.timeout.is_some()
            || self.storage_path.is_some()
    }
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

    let run = match cli.command {
        Some(Command::Run(command)) => Some(command),
        None if cli.run.prompt.is_some() => Some(cli.run),
        None if cli.run.has_any_input() => {
            eprintln!("mastracode requires --prompt when headless flags are provided");
            return ExitCode::from(1);
        }
        None => None,
    };

    match run {
        Some(command) => {
            let prompt = match resolve_prompt(command.prompt.expect("prompt")) {
                Ok(prompt) => prompt,
                Err(error) => {
                    eprintln!("{error}");
                    return ExitCode::from(1);
                }
            };

            let run = run_headless(RunOptions {
                prompt,
                thread_id: command.thread_id,
                continue_latest: command.continue_latest,
                resource_id: command.resource_id,
                format: command.format,
                storage_path: command.storage_path,
            });

            let result = if let Some(timeout) = command.timeout {
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
                    println!("{}", render_output(&output, command.format));
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

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::{Cli, Command};

    #[test]
    fn run_command_accepts_official_continue_flag() {
        let cli = Cli::try_parse_from(["mastracode", "run", "--prompt", "hello", "--continue"])
            .expect("run command should parse");

        match cli.command {
            Some(Command::Run(command)) => assert!(command.continue_latest),
            other => panic!("expected run command, got {other:?}"),
        }
    }

    #[test]
    fn run_command_keeps_continue_latest_as_compatibility_alias() {
        let cli = Cli::try_parse_from([
            "mastracode",
            "run",
            "--prompt",
            "hello",
            "--continue-latest",
        ])
        .expect("run command should parse");

        match cli.command {
            Some(Command::Run(command)) => assert!(command.continue_latest),
            other => panic!("expected run command, got {other:?}"),
        }
    }

    #[test]
    fn top_level_prompt_invocation_matches_upstream_headless_mode() {
        let cli = Cli::try_parse_from([
            "mastracode",
            "--prompt",
            "hello",
            "--continue",
            "--format",
            "json",
        ]);

        assert!(
            cli.is_ok(),
            "top-level headless invocation should parse like upstream"
        );
    }
}
