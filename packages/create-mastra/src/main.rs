use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};
use create_mastra::{ScaffoldOptions, default_repo_root, scaffold_with_options};

#[derive(Parser)]
#[command(name = "create-mastra")]
#[command(about = "Scaffold a Rust Mastra project")]
struct Cli {
    #[arg(value_name = "project-name")]
    project_name_arg: Option<String>,

    #[arg(short = 'p', long)]
    project_name: Option<String>,

    #[arg(long, default_value_t = false)]
    default: bool,

    #[command(flatten)]
    scaffold: ScaffoldCliOptions,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Clone, Args)]
struct ScaffoldCliOptions {
    #[arg(short = 'c', long)]
    components: Option<String>,

    #[arg(short = 'l', long)]
    llm: Option<String>,

    #[arg(short = 'k', long)]
    llm_api_key: Option<String>,

    #[arg(short = 'e', long, default_value_t = false)]
    example: bool,

    #[arg(short = 'n', long = "no-example", default_value_t = false)]
    no_example: bool,

    #[arg(short = 'd', long)]
    dir: Option<PathBuf>,

    #[arg(short = 'm', long)]
    mcp: Option<String>,

    #[arg(long)]
    template: Option<String>,
}

#[derive(Debug, Clone, Subcommand)]
enum Command {
    New {
        path: PathBuf,

        #[command(flatten)]
        scaffold: ScaffoldCliOptions,
    },
}

fn build_options(
    project_name: Option<String>,
    default: bool,
    options: ScaffoldCliOptions,
) -> ScaffoldOptions {
    let mut scaffold_options = ScaffoldOptions {
        project_name,
        directory: options.dir.unwrap_or_else(|| PathBuf::from("src")),
        components: options
            .components
            .unwrap_or_default()
            .split(',')
            .map(str::trim)
            .filter(|component| !component.is_empty())
            .map(ToString::to_string)
            .collect(),
        llm_provider: options.llm,
        llm_api_key: options.llm_api_key,
        add_example: !options.no_example,
        mcp_server: options.mcp,
        template: options.template,
    };

    if options.example {
        scaffold_options.add_example = true;
    }

    if default {
        scaffold_options = scaffold_options.apply_default_quickstart();
    }

    scaffold_options
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let project_name = cli.project_name.or(cli.project_name_arg);

    match cli.command {
        Some(Command::New { path, scaffold }) => {
            let options = build_options(project_name.clone(), cli.default, scaffold);
            scaffold_with_options(&path, &default_repo_root(), &options)?;
            println!("created {}", path.display());
        }
        None => {
            let target = PathBuf::from(project_name.clone().as_deref().unwrap_or("mastra-app"));
            let options = build_options(project_name, cli.default, cli.scaffold);
            scaffold_with_options(&target, &default_repo_root(), &options)?;
            println!("created {}", target.display());
        }
    };

    Ok(())
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::Cli;

    #[test]
    fn root_invocation_accepts_upstream_style_flags_without_subcommand() {
        let cli = Cli::try_parse_from([
            "create-mastra",
            "demo-app",
            "--default",
            "--components",
            "agents,tools,workflows",
            "--llm",
            "openai",
            "--dir",
            "src",
            "--template",
            "template-deep-search",
        ]);

        assert!(
            cli.is_ok(),
            "create-mastra should parse upstream-style root flags"
        );
    }

    #[test]
    fn root_invocation_accepts_project_name_option_without_subcommand() {
        let cli = Cli::try_parse_from([
            "create-mastra",
            "--project-name",
            "demo-app",
            "--no-example",
            "--mcp",
            "vscode",
        ]);

        assert!(
            cli.is_ok(),
            "create-mastra should accept --project-name without requiring `new`"
        );
    }
}
