use std::{
    error::Error,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::{Path, PathBuf},
};

use clap::{Args, Parser, Subcommand};
use create_mastra::{default_repo_root, scaffold};
use mastra_server::{MastraHttpServer, RouteDescription};

pub const DEFAULT_SERVER_PORT: u16 = 4111;
const DEFAULT_CREATE_DIR: &str = ".";
const DEFAULT_START_DIR: &str = ".mastra/output";
const DEFAULT_MASTRA_DIR: &str = "src/mastra";

#[derive(Debug, Parser)]
#[command(name = "mastra")]
#[command(about = "Rust port of the Mastra CLI")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Create(CreateCommand),
    Dev(DevCommand),
    Start(StartCommand),
    Routes,
}

#[derive(Debug, Clone, Args)]
pub struct CreateCommand {
    #[arg(value_name = "project-name")]
    pub project_name: Option<String>,

    #[arg(long, default_value = DEFAULT_CREATE_DIR)]
    pub dir: PathBuf,
}

#[derive(Debug, Clone, Args)]
pub struct DevCommand {
    #[arg(long, default_value_t = default_bind_addr())]
    pub addr: SocketAddr,

    #[arg(long, default_value = DEFAULT_MASTRA_DIR)]
    pub dir: PathBuf,

    #[arg(long)]
    pub env: Option<PathBuf>,

    #[arg(long, default_value_t = false)]
    pub debug: bool,
}

#[derive(Debug, Clone, Args)]
pub struct StartCommand {
    #[arg(long, default_value_t = default_bind_addr())]
    pub addr: SocketAddr,

    #[arg(long, default_value = DEFAULT_START_DIR)]
    pub dir: PathBuf,

    #[arg(long)]
    pub env: Option<PathBuf>,
}

pub fn default_bind_addr() -> SocketAddr {
    SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), DEFAULT_SERVER_PORT)
}

pub fn start_output_dir() -> PathBuf {
    PathBuf::from(DEFAULT_START_DIR)
}

pub fn render_routes(routes: &[RouteDescription]) -> String {
    routes
        .iter()
        .map(|route| format!("{} {}  # {}", route.method, route.path, route.summary))
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn serve_banner(addr: SocketAddr) -> String {
    format!("starting mastra server on {addr}")
}

pub fn start_banner(addr: SocketAddr, dir: &Path) -> String {
    format!(
        "starting mastra production server on {addr} using {}",
        dir.display()
    )
}

pub fn create_success_banner(path: &Path) -> String {
    format!("created mastra project at {}", path.display())
}

pub fn scaffold_create_project(
    command: &CreateCommand,
    repo_root: &Path,
) -> Result<PathBuf, Box<dyn Error>> {
    let project_name = command.project_name.as_deref().unwrap_or("mastra-app");
    let target = command.dir.join(project_name);

    if target.exists() {
        return Err(format!("target directory {} already exists", target.display()).into());
    }

    scaffold(&target, repo_root)?;
    Ok(target)
}

pub async fn run(cli: Cli) -> Result<(), Box<dyn Error>> {
    match cli.command {
        Command::Create(command) => {
            let target = scaffold_create_project(&command, &default_repo_root())?;
            println!("{}", create_success_banner(&target));
        }
        Command::Dev(command) => {
            eprintln!("{}", serve_banner(command.addr));
            MastraHttpServer::new().serve(command.addr).await?;
        }
        Command::Start(command) => {
            eprintln!("{}", start_banner(command.addr, &command.dir));
            MastraHttpServer::new().serve(command.addr).await?;
        }
        Command::Routes => {
            println!("{}", render_routes(&MastraHttpServer::route_descriptions()));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use clap::Parser;
    use mastra_server::RouteDescription;
    use tempfile::tempdir;

    use super::{
        Cli, Command, CreateCommand, create_success_banner, default_bind_addr, render_routes,
        scaffold_create_project, serve_banner, start_banner, start_output_dir,
    };

    #[test]
    fn default_bind_addr_matches_expected_local_endpoint() {
        assert_eq!(default_bind_addr().to_string(), "127.0.0.1:4111");
    }

    #[test]
    fn render_routes_formats_each_route_on_its_own_line() {
        let rendered = render_routes(&[
            RouteDescription {
                method: "GET",
                path: "/health".to_owned(),
                summary: "health check",
            },
            RouteDescription {
                method: "POST",
                path: "/agents/weather/generate".to_owned(),
                summary: "generate",
            },
        ]);

        assert_eq!(
            rendered,
            "GET /health  # health check\nPOST /agents/weather/generate  # generate"
        );
    }

    #[test]
    fn serve_banner_mentions_bind_address() {
        assert!(serve_banner(default_bind_addr()).contains("127.0.0.1:4111"));
    }

    #[test]
    fn cli_parses_create_command_with_project_name_and_parent_dir() {
        let temp = tempdir().expect("tempdir");
        let cli = Cli::try_parse_from([
            "mastra",
            "create",
            "demo-app",
            "--dir",
            temp.path().to_str().expect("utf8 path"),
        ])
        .expect("create command should parse");

        match cli.command {
            Command::Create(command) => {
                assert_eq!(command.project_name.as_deref(), Some("demo-app"));
                assert_eq!(command.dir, temp.path());
            }
            other => panic!("expected create command, got {other:?}"),
        }
    }

    #[test]
    fn scaffold_create_project_writes_starter_files_via_create_mastra() {
        let temp = tempdir().expect("tempdir");
        let repo_root = PathBuf::from("/repo/mastra-rs");
        let target = scaffold_create_project(
            &CreateCommand {
                project_name: Some("demo-app".into()),
                dir: temp.path().to_path_buf(),
            },
            &repo_root,
        )
        .expect("scaffold should succeed");

        let manifest = std::fs::read_to_string(target.join("Cargo.toml")).expect("manifest");
        let main_rs = std::fs::read_to_string(target.join("src/main.rs")).expect("main");

        assert_eq!(target, temp.path().join("demo-app"));
        assert!(manifest.contains("/repo/mastra-rs/packages/core"));
        assert!(main_rs.contains("StaticModel::echo()"));
    }

    #[test]
    fn cli_parses_dev_command_with_official_default_port() {
        let cli = Cli::try_parse_from(["mastra", "dev"]).expect("dev command should parse");

        match cli.command {
            Command::Dev(command) => {
                assert_eq!(command.addr.to_string(), "127.0.0.1:4111");
                assert_eq!(command.dir, PathBuf::from("src/mastra"));
            }
            other => panic!("expected dev command, got {other:?}"),
        }
    }

    #[test]
    fn cli_parses_start_command_with_output_dir_default() {
        let cli = Cli::try_parse_from(["mastra", "start"]).expect("start command should parse");

        match cli.command {
            Command::Start(command) => {
                assert_eq!(command.addr.to_string(), "127.0.0.1:4111");
                assert_eq!(command.dir, start_output_dir());
            }
            other => panic!("expected start command, got {other:?}"),
        }
    }

    #[test]
    fn create_and_start_banners_expose_target_paths() {
        assert_eq!(
            create_success_banner(Path::new("/tmp/demo-app")),
            "created mastra project at /tmp/demo-app"
        );
        assert_eq!(
            start_banner(default_bind_addr(), Path::new(".mastra/output")),
            "starting mastra production server on 127.0.0.1:4111 using .mastra/output"
        );
    }
}
