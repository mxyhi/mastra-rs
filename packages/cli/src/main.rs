use clap::Parser;
use mastra_cli::{Cli, run};
use mastra_loggers::init_tracing;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_tracing("info");
    run(Cli::parse()).await
}
