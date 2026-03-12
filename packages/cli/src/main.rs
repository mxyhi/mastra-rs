use clap::{Parser, Subcommand};
use mastra_loggers::init_tracing;
use mastra_server::MastraHttpServer;

#[derive(Parser)]
#[command(name = "mastra")]
#[command(about = "Rust port of the Mastra CLI")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Serve {
        #[arg(long, default_value = "127.0.0.1:3000")]
        addr: std::net::SocketAddr,
    },
    Routes,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_tracing("info");
    let cli = Cli::parse();

    match cli.command {
        Command::Serve { addr } => {
            MastraHttpServer::new().serve(addr).await?;
        }
        Command::Routes => {
            for route in MastraHttpServer::route_descriptions() {
                println!("{} {}", route.method, route.path);
            }
        }
    }

    Ok(())
}
