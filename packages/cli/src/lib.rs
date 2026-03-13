mod project;
mod studio;

use std::{
    env,
    error::Error,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::{Path, PathBuf},
};

use clap::{Args, Parser, Subcommand};
use create_mastra::{default_repo_root, scaffold};
use mastra_server::{MastraHttpServer, RouteDescription};
use project::{
    DEFAULT_BUILD_DIR, DEFAULT_MASTRA_DIR, ProjectSummary, add_scorer, build_project,
    build_server_from_manifest, default_build_dir, default_mastra_dir, lint_project,
    list_builtin_scorers, load_project_bundle, load_project_manifest, migrate_project,
};
use studio::{StudioConfig, load_request_context_presets, render_studio_html, serve_studio};

pub const DEFAULT_SERVER_PORT: u16 = 4111;
const DEFAULT_CREATE_DIR: &str = ".";
const DEFAULT_STUDIO_PORT: u16 = 3000;
const DEFAULT_SERVER_HOST: &str = "localhost";
const DEFAULT_SERVER_PROTOCOL: &str = "http";

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
    Init(InitCommand),
    Lint(LintCommand),
    Dev(DevCommand),
    Build(BuildCommand),
    Start(StartCommand),
    Studio(StudioCommand),
    Migrate(MigrateCommand),
    Scorers(ScorersCommand),
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
pub struct InitCommand {
    #[arg(long, default_value = DEFAULT_CREATE_DIR)]
    pub dir: PathBuf,
}

#[derive(Debug, Clone, Args)]
pub struct LintCommand {
    #[arg(long, default_value = DEFAULT_MASTRA_DIR)]
    pub dir: PathBuf,

    #[arg(long, default_value = DEFAULT_CREATE_DIR)]
    pub root: PathBuf,

    #[arg(long)]
    pub tools: Option<String>,
}

#[derive(Debug, Clone, Args)]
pub struct DevCommand {
    #[arg(long, default_value_t = default_bind_addr())]
    pub addr: SocketAddr,

    #[arg(long, default_value = DEFAULT_MASTRA_DIR)]
    pub dir: PathBuf,

    #[arg(long, default_value = DEFAULT_CREATE_DIR)]
    pub root: PathBuf,

    #[arg(long)]
    pub tools: Option<String>,

    #[arg(long)]
    pub env: Option<PathBuf>,

    #[arg(long)]
    pub inspect: Option<String>,

    #[arg(long)]
    pub inspect_brk: Option<String>,

    #[arg(long)]
    pub custom_args: Option<String>,

    #[arg(long, default_value_t = false)]
    pub https: bool,

    #[arg(long)]
    pub request_context_presets: Option<PathBuf>,

    #[arg(long, default_value_t = false)]
    pub debug: bool,
}

#[derive(Debug, Clone, Args)]
pub struct BuildCommand {
    #[arg(long, default_value = DEFAULT_MASTRA_DIR)]
    pub dir: PathBuf,

    #[arg(long, default_value = DEFAULT_CREATE_DIR)]
    pub root: PathBuf,

    #[arg(long)]
    pub tools: Option<String>,

    #[arg(long, default_value_t = false)]
    pub studio: bool,

    #[arg(long, default_value_t = false)]
    pub debug: bool,
}

#[derive(Debug, Clone, Args)]
pub struct StartCommand {
    #[arg(long, default_value_t = default_bind_addr())]
    pub addr: SocketAddr,

    #[arg(long, default_value = DEFAULT_BUILD_DIR)]
    pub dir: PathBuf,

    #[arg(long)]
    pub env: Option<PathBuf>,

    #[arg(long)]
    pub custom_args: Option<String>,
}

#[derive(Debug, Clone, Args)]
pub struct StudioCommand {
    #[arg(long, default_value_t = DEFAULT_STUDIO_PORT)]
    pub port: u16,

    #[arg(long)]
    pub env: Option<PathBuf>,

    #[arg(long, default_value = DEFAULT_SERVER_HOST)]
    pub server_host: String,

    #[arg(long, default_value_t = DEFAULT_SERVER_PORT)]
    pub server_port: u16,

    #[arg(long, default_value = DEFAULT_SERVER_PROTOCOL)]
    pub server_protocol: String,

    #[arg(long, default_value = "/api")]
    pub server_api_prefix: String,

    #[arg(long)]
    pub request_context_presets: Option<PathBuf>,
}

#[derive(Debug, Clone, Args)]
pub struct MigrateCommand {
    #[arg(long, default_value = DEFAULT_MASTRA_DIR)]
    pub dir: PathBuf,

    #[arg(long, default_value = DEFAULT_CREATE_DIR)]
    pub root: PathBuf,

    #[arg(long)]
    pub env: Option<PathBuf>,

    #[arg(long, default_value_t = false)]
    pub debug: bool,

    #[arg(long, default_value_t = false)]
    pub yes: bool,
}

#[derive(Debug, Clone, Args)]
pub struct ScorersCommand {
    #[command(subcommand)]
    pub command: ScorersSubcommand,
}

#[derive(Debug, Clone, Subcommand)]
pub enum ScorersSubcommand {
    Add(ScorerAddCommand),
    List,
}

#[derive(Debug, Clone, Args)]
pub struct ScorerAddCommand {
    #[arg(value_name = "scorer-name")]
    pub scorer_name: Option<String>,

    #[arg(long, default_value = DEFAULT_MASTRA_DIR)]
    pub dir: PathBuf,

    #[arg(long, default_value = DEFAULT_CREATE_DIR)]
    pub root: PathBuf,
}

pub fn default_bind_addr() -> SocketAddr {
    SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), DEFAULT_SERVER_PORT)
}

pub fn start_output_dir() -> PathBuf {
    default_build_dir()
}

pub fn mastra_dir() -> PathBuf {
    default_mastra_dir()
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

pub fn dev_banner(addr: SocketAddr, dir: &Path) -> String {
    format!(
        "starting mastra dev server on {addr} using manifest {}",
        dir.display()
    )
}

pub fn create_success_banner(path: &Path) -> String {
    format!("created mastra project at {}", path.display())
}

pub fn init_success_banner(path: &Path) -> String {
    format!("initialized mastra project in {}", path.display())
}

pub fn lint_success_banner(summary: &ProjectSummary) -> String {
    format!(
        "validated {} (agents: {}, tools: {}, workflows: {}, memories: {})",
        summary.app_name, summary.agents, summary.tools, summary.workflows, summary.memories
    )
}

pub fn build_success_banner(output_dir: &Path) -> String {
    format!("built Mastra project into {}", output_dir.display())
}

pub fn migrate_success_banner(memory_ids: &[String]) -> String {
    if memory_ids.is_empty() {
        "no libsql-backed memories required migration".to_owned()
    } else {
        format!("migrated memories: {}", memory_ids.join(", "))
    }
}

pub fn scorer_add_banner(path: &Path) -> String {
    format!("added scorer template at {}", path.display())
}

pub fn scorer_list_output() -> String {
    list_builtin_scorers()
        .iter()
        .map(|scorer| format!("{}  # {}", scorer.id, scorer.description))
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn studio_server_url(command: &StudioCommand) -> String {
    format!(
        "{}://{}:{}{}",
        command.server_protocol,
        command.server_host,
        command.server_port,
        normalize_api_prefix(&command.server_api_prefix)
    )
}

pub fn studio_bind_addr(port: u16) -> SocketAddr {
    SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port)
}

pub fn ignored_option_warnings(
    options: &[(&str, Option<String>)],
    toggles: &[(&str, bool)],
) -> Vec<String> {
    let mut warnings = options
        .iter()
        .filter_map(|(name, value)| {
            value
                .as_ref()
                .map(|value| format!("ignoring upstream-only option --{name}={value}"))
        })
        .collect::<Vec<_>>();
    warnings.extend(
        toggles
            .iter()
            .filter(|(_, enabled)| *enabled)
            .map(|(name, _)| format!("ignoring upstream-only option --{name}")),
    );
    warnings
}

pub fn load_env_files(
    current_dir: &Path,
    defaults: &[&str],
    custom_env: Option<&Path>,
) -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let mut loaded = Vec::new();

    for relative in defaults {
        let path = current_dir.join(relative);
        if path.is_file() {
            let _ = std::fs::read_to_string(&path)?;
            loaded.push(path);
        }
    }

    if let Some(custom_env) = custom_env {
        let path = if custom_env.is_absolute() {
            custom_env.to_path_buf()
        } else {
            current_dir.join(custom_env)
        };
        let _ = std::fs::read_to_string(&path)?;
        loaded.push(path);
    }

    Ok(loaded)
}

fn normalize_api_prefix(prefix: &str) -> String {
    let trimmed = prefix.trim();
    if trimmed.is_empty() || trimmed == "/" {
        String::new()
    } else {
        format!("/{}", trimmed.trim_matches('/'))
    }
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

pub fn scaffold_init_project(
    command: &InitCommand,
    repo_root: &Path,
) -> Result<PathBuf, Box<dyn Error>> {
    let target = command.dir.clone();

    if target.join("Cargo.toml").exists() || target.join("src/main.rs").exists() {
        return Err(format!(
            "target directory {} already contains a Rust starter",
            target.display()
        )
        .into());
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
        Command::Init(command) => {
            let target = scaffold_init_project(&command, &default_repo_root())?;
            println!("{}", init_success_banner(&target));
        }
        Command::Lint(command) => {
            if let Some(tools) = &command.tools {
                eprintln!(
                    "{}",
                    ignored_option_warnings(&[("tools", Some(tools.clone()))], &[]).join("\n")
                );
            }

            let summary = lint_project(&command.root, &command.dir)?;
            println!("{}", lint_success_banner(&summary));
        }
        Command::Dev(command) => {
            for warning in ignored_option_warnings(
                &[
                    ("tools", command.tools.clone()),
                    ("inspect", command.inspect.clone()),
                    ("inspect-brk", command.inspect_brk.clone()),
                    ("custom-args", command.custom_args.clone()),
                ],
                &[("https", command.https)],
            ) {
                eprintln!("{warning}");
            }

            let _ = load_env_files(
                &command.root,
                &[".env.development", ".env.local", ".env"],
                command.env.as_deref(),
            )?;
            if let Some(presets_path) = command.request_context_presets.as_deref() {
                let _ = load_request_context_presets(presets_path)?;
            }

            let manifest = load_project_manifest(&command.root, &command.dir)?;
            let server = build_server_from_manifest(&command.root, &manifest)?;
            eprintln!("{}", dev_banner(command.addr, &command.dir));
            server.serve(command.addr).await?;
        }
        Command::Build(command) => {
            if let Some(tools) = &command.tools {
                eprintln!(
                    "{}",
                    ignored_option_warnings(&[("tools", Some(tools.clone()))], &[]).join("\n")
                );
            }

            let output_dir = command.root.join(DEFAULT_BUILD_DIR);
            let summary = build_project(&command.root, &command.dir, &output_dir)?;
            if command.studio {
                let studio_dir = output_dir.join("studio");
                std::fs::create_dir_all(&studio_dir)?;
                std::fs::write(
                    studio_dir.join("index.html"),
                    render_studio_html("http://localhost:4111/api", None),
                )?;
            }

            println!("{}", build_success_banner(&summary.output_dir));
            println!("{}", summary.routes_path.display());
            println!("{}", summary.manifest_path.display());
        }
        Command::Start(command) => {
            if let Some(custom_args) = &command.custom_args {
                eprintln!(
                    "{}",
                    ignored_option_warnings(&[("custom-args", Some(custom_args.clone()))], &[])
                        .join("\n")
                );
            }

            let current_dir = env::current_dir()?;
            let _ = load_env_files(
                &current_dir,
                &[".env.production", ".env"],
                command.env.as_deref(),
            )?;
            let bundle = load_project_bundle(&command.dir)?;
            let server = build_server_from_manifest(&current_dir, &bundle.manifest)?;
            eprintln!("{}", start_banner(command.addr, &command.dir));
            server.serve(command.addr).await?;
        }
        Command::Studio(command) => {
            let current_dir = env::current_dir()?;
            let _ = load_env_files(
                &current_dir,
                &[".env.production", ".env"],
                command.env.as_deref(),
            )?;
            let presets = command
                .request_context_presets
                .as_deref()
                .map(load_request_context_presets)
                .transpose()?;
            let config = StudioConfig {
                address: studio_bind_addr(command.port),
                server_url: studio_server_url(&command),
                presets,
            };
            eprintln!(
                "starting mastra studio on {} for {}",
                config.address, config.server_url
            );
            serve_studio(config).await?;
        }
        Command::Migrate(command) => {
            let _ = load_env_files(
                &command.root,
                &[".env.production", ".env"],
                command.env.as_deref(),
            )?;
            let summary = migrate_project(&command.root, &command.dir).await?;
            println!("{}", migrate_success_banner(&summary.migrated_memory_ids));
        }
        Command::Scorers(command) => match command.command {
            ScorersSubcommand::Add(add) => {
                let scorer_name = add
                    .scorer_name
                    .unwrap_or_else(|| "answer-relevancy".to_owned());
                let path = add_scorer(&add.root, &add.dir, &scorer_name)?;
                println!("{}", scorer_add_banner(&path));
            }
            ScorersSubcommand::List => {
                println!("{}", scorer_list_output());
            }
        },
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
        Cli, Command, StudioCommand, build_success_banner, create_success_banner,
        default_bind_addr, dev_banner, init_success_banner, lint_success_banner, load_env_files,
        mastra_dir, migrate_success_banner, render_routes, scaffold_create_project,
        scaffold_init_project, scorer_add_banner, scorer_list_output, serve_banner, start_banner,
        start_output_dir, studio_bind_addr, studio_server_url,
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
    fn banners_include_expected_paths() {
        assert!(serve_banner(default_bind_addr()).contains("127.0.0.1:4111"));
        assert!(dev_banner(default_bind_addr(), Path::new("src/mastra")).contains("src/mastra"));
        assert!(build_success_banner(Path::new(".mastra/output")).contains(".mastra/output"));
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
            &super::CreateCommand {
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
    fn cli_parses_init_command_with_default_directory() {
        let cli = Cli::try_parse_from(["mastra", "init"]).expect("init command should parse");

        match cli.command {
            Command::Init(command) => {
                assert_eq!(command.dir, PathBuf::from("."));
            }
            other => panic!("expected init command, got {other:?}"),
        }
    }

    #[test]
    fn scaffold_init_project_writes_starter_files_in_place() {
        let temp = tempdir().expect("tempdir");
        let repo_root = PathBuf::from("/repo/mastra-rs");

        let target = scaffold_init_project(
            &super::InitCommand {
                dir: temp.path().to_path_buf(),
            },
            &repo_root,
        )
        .expect("init scaffold should succeed");

        let manifest = std::fs::read_to_string(target.join("Cargo.toml")).expect("manifest");
        let main_rs = std::fs::read_to_string(target.join("src/main.rs")).expect("main");

        assert_eq!(target, temp.path());
        assert!(manifest.contains("/repo/mastra-rs/packages/core"));
        assert!(main_rs.contains("Starter agent generated by create-mastra"));
    }

    #[test]
    fn cli_parses_expanded_command_surface() {
        let build = Cli::try_parse_from(["mastra", "build"]).expect("build");
        assert!(matches!(build.command, Command::Build(_)));

        let lint = Cli::try_parse_from(["mastra", "lint"]).expect("lint");
        assert!(matches!(lint.command, Command::Lint(_)));

        let studio = Cli::try_parse_from(["mastra", "studio"]).expect("studio");
        assert!(matches!(studio.command, Command::Studio(_)));

        let migrate = Cli::try_parse_from(["mastra", "migrate"]).expect("migrate");
        assert!(matches!(migrate.command, Command::Migrate(_)));

        let scorers = Cli::try_parse_from(["mastra", "scorers", "list"]).expect("scorers list");
        assert!(matches!(scorers.command, Command::Scorers(_)));
    }

    #[test]
    fn cli_parses_dev_command_with_official_default_port() {
        let cli = Cli::try_parse_from(["mastra", "dev"]).expect("dev command should parse");

        match cli.command {
            Command::Dev(command) => {
                assert_eq!(command.addr.to_string(), "127.0.0.1:4111");
                assert_eq!(command.dir, mastra_dir());
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
            init_success_banner(Path::new("/tmp/existing-app")),
            "initialized mastra project in /tmp/existing-app"
        );
        assert_eq!(
            start_banner(default_bind_addr(), Path::new(".mastra/output")),
            "starting mastra production server on 127.0.0.1:4111 using .mastra/output"
        );
    }

    #[test]
    fn studio_url_and_bind_addr_follow_expected_defaults() {
        let command = StudioCommand {
            port: 3000,
            env: None,
            server_host: "localhost".to_owned(),
            server_port: 4111,
            server_protocol: "http".to_owned(),
            server_api_prefix: "/api".to_owned(),
            request_context_presets: None,
        };
        assert_eq!(studio_bind_addr(command.port).to_string(), "127.0.0.1:3000");
        assert_eq!(studio_server_url(&command), "http://localhost:4111/api");
    }

    #[test]
    fn helper_output_is_human_readable() {
        let summary = super::ProjectSummary {
            app_name: "demo".to_owned(),
            memories: 1,
            tools: 2,
            agents: 3,
            workflows: 4,
        };
        assert!(lint_success_banner(&summary).contains("validated demo"));
        assert!(migrate_success_banner(&[]).contains("no libsql"));
        assert!(scorer_add_banner(Path::new("src/mastra/scorers/a.rs")).contains("scorer"));
        assert!(scorer_list_output().contains("answer-relevancy"));
    }

    #[test]
    fn load_env_files_accepts_defaults_and_custom_override() {
        let temp = tempdir().expect("tempdir");
        std::fs::write(temp.path().join(".env"), "OPENAI_API_KEY=test").expect("env");
        std::fs::write(temp.path().join("extra.env"), "FOO=bar").expect("extra env");

        let loaded = load_env_files(temp.path(), &[".env"], Some(Path::new("extra.env")))
            .expect("env files");
        assert_eq!(loaded.len(), 2);
    }
}
