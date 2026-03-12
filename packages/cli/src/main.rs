use clap::{Parser, Subcommand};
use mastra_cli::{default_bind_addr, render_routes, serve_banner};
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
        #[arg(long, default_value_t = default_bind_addr())]
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
            eprintln!("{}", serve_banner(addr));
            MastraHttpServer::new().serve(addr).await?;
        }
        Command::Routes => {
            println!("{}", render_routes(&MastraHttpServer::route_descriptions()));
        }
    }

    Ok(())
}
