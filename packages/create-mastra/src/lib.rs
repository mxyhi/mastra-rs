use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
const STARTER_MANIFEST_PATH: &str = "src/mastra/mastra.json";
#[cfg(test)]
const STARTER_MEMORY_PATH: &str = "src/mastra/memories/default-memory.json";
#[cfg(test)]
const STARTER_TOOL_PATH: &str = "src/mastra/tools/demo-sum.json";
#[cfg(test)]
const STARTER_AGENT_SPEC_PATH: &str = "src/mastra/agents/demo-agent.json";
#[cfg(test)]
const STARTER_AGENT_INSTRUCTIONS_PATH: &str = "src/mastra/agents/demo-agent.md";
#[cfg(test)]
const STARTER_WORKFLOW_PATH: &str = "src/mastra/workflows/demo-workflow.json";
#[cfg(test)]
const STARTER_PROMPT_PATH: &str = "src/mastra/resources/hello.txt";
const DEFAULT_MASTRA_PARENT_DIR: &str = "src";
const DEFAULT_COMPONENTS: &[&str] = &["agents", "tools", "workflows", "scorers"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScaffoldOptions {
    pub project_name: Option<String>,
    pub directory: PathBuf,
    pub components: Vec<String>,
    pub llm_provider: Option<String>,
    pub llm_api_key: Option<String>,
    pub add_example: bool,
    pub mcp_server: Option<String>,
    pub template: Option<String>,
}

impl Default for ScaffoldOptions {
    fn default() -> Self {
        Self {
            project_name: None,
            directory: PathBuf::from(DEFAULT_MASTRA_PARENT_DIR),
            components: Vec::new(),
            llm_provider: None,
            llm_api_key: None,
            add_example: true,
            mcp_server: None,
            template: None,
        }
    }
}

impl ScaffoldOptions {
    pub fn apply_default_quickstart(mut self) -> Self {
        if self.components.is_empty() {
            self.components = DEFAULT_COMPONENTS.iter().map(ToString::to_string).collect();
        }

        if self.llm_provider.is_none() {
            self.llm_provider = Some("openai".to_owned());
        }

        self.add_example = true;
        self
    }

    fn resolved_project_name(&self, path: &Path) -> String {
        self.project_name.clone().unwrap_or_else(|| {
            path.file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("mastra-app")
                .to_owned()
        })
    }

    fn selected_components(&self) -> BTreeSet<String> {
        if self.components.is_empty() {
            return ["agents", "tools", "workflows"]
                .into_iter()
                .map(ToString::to_string)
                .collect();
        }

        self.components
            .iter()
            .map(|component| component.trim().to_ascii_lowercase())
            .filter(|component| !component.is_empty())
            .collect()
    }

    fn mastra_dir(&self) -> PathBuf {
        if self
            .directory
            .iter()
            .last()
            .and_then(|segment| segment.to_str())
            == Some("mastra")
        {
            return self.directory.clone();
        }

        self.directory.join("mastra")
    }

    fn llm_provider(&self) -> &str {
        self.llm_provider.as_deref().unwrap_or("openai")
    }

    fn llm_api_key_env_name(&self) -> &'static str {
        match self.llm_provider() {
            "anthropic" => "ANTHROPIC_API_KEY",
            "groq" => "GROQ_API_KEY",
            "google" => "GOOGLE_API_KEY",
            "cerebras" => "CEREBRAS_API_KEY",
            _ => "OPENAI_API_KEY",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct StarterPaths {
    manifest: PathBuf,
    memory: PathBuf,
    tool: PathBuf,
    agent_spec: PathBuf,
    agent_instructions: PathBuf,
    workflow: PathBuf,
    prompt: PathBuf,
    scorer: PathBuf,
}

fn starter_paths(options: &ScaffoldOptions) -> StarterPaths {
    let mastra_dir = options.mastra_dir();
    StarterPaths {
        manifest: mastra_dir.join("mastra.json"),
        memory: mastra_dir.join("memories/default-memory.json"),
        tool: mastra_dir.join("tools/demo-sum.json"),
        agent_spec: mastra_dir.join("agents/demo-agent.json"),
        agent_instructions: mastra_dir.join("agents/demo-agent.md"),
        workflow: mastra_dir.join("workflows/demo-workflow.json"),
        prompt: mastra_dir.join("resources/hello.txt"),
        scorer: mastra_dir.join("scorers/answer-relevancy.rs"),
    }
}

pub fn render_manifest(project_name: &str, repo_root: &Path) -> String {
    format!(
        r#"[package]
name = "{project_name}"
version = "0.1.0"
edition = "2024"

[dependencies]
mastra-core = {{ path = "{core_path}" }}
mastra-memory = {{ path = "{memory_path}" }}
mastra-loggers = {{ path = "{loggers_path}" }}
tokio = {{ version = "1.47.1", features = ["full"] }}
"#,
        core_path = repo_root.join("packages/core").display(),
        memory_path = repo_root.join("packages/memory").display(),
        loggers_path = repo_root.join("packages/loggers").display(),
    )
}

fn include_path(options: &ScaffoldOptions, asset: &str) -> String {
    let asset_path = options.mastra_dir().join(asset);

    if let Ok(stripped) = asset_path.strip_prefix("src") {
        return stripped.display().to_string();
    }

    PathBuf::from("..").join(asset_path).display().to_string()
}

pub fn render_main_rs(options: &ScaffoldOptions) -> String {
    r#"use std::sync::Arc;

use mastra_core::{Agent, AgentConfig, AgentGenerateRequest, MemoryConfig, RequestContext, StaticModel};
use mastra_loggers::init_tracing;
use mastra_memory::Memory;

const STARTER_MANIFEST: &str = include_str!("__MANIFEST_PATH__");
const DEMO_TOOL_SPEC: &str = include_str!("__TOOL_PATH__");
const DEMO_WORKFLOW_SPEC: &str = include_str!("__WORKFLOW_PATH__");
const DEMO_AGENT_INSTRUCTIONS: &str = include_str!("__INSTRUCTIONS_PATH__");
const DEMO_PROMPT: &str = include_str!("__PROMPT_PATH__");

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_tracing("info");

    let agent = Agent::new(AgentConfig {
        id: "demo-agent".to_owned(),
        name: "Demo Agent".to_owned(),
        instructions: DEMO_AGENT_INSTRUCTIONS.trim().to_owned(),
        description: Some("Starter agent generated by create-mastra".to_owned()),
        model: Arc::new(StaticModel::echo()),
        tools: Vec::new(),
        memory: Some(Arc::new(Memory::in_memory())),
        memory_config: MemoryConfig::default(),
    });

    let response = agent.generate(AgentGenerateRequest {
        prompt: DEMO_PROMPT.trim().to_owned(),
        thread_id: None,
        resource_id: Some("starter".to_owned()),
        run_id: None,
        max_steps: None,
        request_context: RequestContext::new().with_resource_id("starter"),
        ..Default::default()
    }).await?;

    println!(
        "starter graph assets loaded: manifest={} tool={} workflow={}",
        STARTER_MANIFEST.lines().count(),
        DEMO_TOOL_SPEC.lines().count(),
        DEMO_WORKFLOW_SPEC.lines().count(),
    );
    println!("agent response: {}", response.text);
    Ok(())
}
"#
    .replace("__MANIFEST_PATH__", &include_path(options, "mastra.json"))
    .replace("__TOOL_PATH__", &include_path(options, "tools/demo-sum.json"))
    .replace(
        "__WORKFLOW_PATH__",
        &include_path(options, "workflows/demo-workflow.json"),
    )
    .replace(
        "__INSTRUCTIONS_PATH__",
        &include_path(options, "agents/demo-agent.md"),
    )
    .replace("__PROMPT_PATH__", &include_path(options, "resources/hello.txt"))
}

pub fn render_starter_manifest(project_name: &str, options: &ScaffoldOptions) -> String {
    let template = options
        .template
        .as_ref()
        .map(|value| format!("\"{value}\""))
        .unwrap_or_else(|| "null".to_owned());
    let mcp_server = options
        .mcp_server
        .as_ref()
        .map(|value| format!("\"{value}\""))
        .unwrap_or_else(|| "null".to_owned());
    let components = options
        .selected_components()
        .into_iter()
        .map(|component| format!("\"{component}\""))
        .collect::<Vec<_>>()
        .join(", ");

    format!(
        r#"{{
  "schema_version": 1,
  "app_name": "{project_name}",
  "entrypoint": "src/main.rs",
  "mastra_dir": "{mastra_dir}",
  "starter_metadata": {{
    "template": {template},
    "mcp_server": {mcp_server},
    "llm_provider": "{llm_provider}",
    "add_example": {add_example},
    "components": [{components}]
  }},
  "resources": [
    {{
      "id": "starter-prompt",
      "kind": "text",
      "path": "resources/hello.txt"
    }}
  ],
  "memories": [
    {{
      "id": "default-memory",
      "path": "memories/default-memory.json"
    }}
  ],
  "tools": [
    {{
      "id": "demo-sum",
      "path": "tools/demo-sum.json"
    }}
  ],
  "agents": [
    {{
      "id": "demo-agent",
      "path": "agents/demo-agent.json"
    }}
  ],
  "workflows": [
    {{
      "id": "demo-workflow",
      "path": "workflows/demo-workflow.json"
    }}
  ]
}}
"#,
        mastra_dir = options.mastra_dir().display(),
        template = template,
        mcp_server = mcp_server,
        llm_provider = options.llm_provider(),
        add_example = options.add_example,
        components = components,
    )
}

pub fn render_memory_manifest() -> String {
    r#"{
  "id": "default-memory",
  "kind": "in_memory",
  "name": "Default Memory",
  "description": "Starter in-memory conversation store used by the demo agent."
}
"#
    .to_owned()
}

pub fn render_tool_manifest() -> String {
    r#"{
  "id": "demo-sum",
  "kind": "sum",
  "name": "Demo Sum Tool",
  "description": "Adds two integers so the CLI loader has a concrete tool node to register.",
  "input": {
    "left": 2,
    "right": 3
  },
  "output": {
    "sum": 5
  }
}
"#
    .to_owned()
}

pub fn render_agent_manifest() -> String {
    r#"{
  "id": "demo-agent",
  "kind": "echo",
  "name": "Demo Agent",
  "instructions_path": "agents/demo-agent.md",
  "memory": "default-memory",
  "tools": [
    "demo-sum"
  ],
  "model": {
    "kind": "echo"
  }
}
"#
    .to_owned()
}

pub fn render_workflow_manifest() -> String {
    r#"{
  "id": "demo-workflow",
  "kind": "static_json",
  "name": "Demo Workflow",
  "description": "Starter workflow graph for CLI project loading.",
  "steps": [
    {
      "id": "prepare_input",
      "kind": "static_json",
      "output": {
        "left": 2,
        "right": 3
      }
    },
    {
      "id": "call_demo_tool",
      "kind": "tool",
      "tool": "demo-sum"
    },
    {
      "id": "finish",
      "kind": "static_json",
      "output": {
        "status": "ok",
        "message": "starter workflow completed"
      }
    }
  ]
}
"#
    .to_owned()
}

pub fn render_agent_instructions() -> String {
    r#"# Demo Agent

You are the starter agent generated by `create-mastra`.

Keep answers concise and grounded in the prompt.
Echo the request in a friendly sentence so the default `StaticModel::echo()` run is easy to verify.
"#
    .to_owned()
}

pub fn render_prompt_example(options: &ScaffoldOptions) -> String {
    if options.add_example {
        "Hello from create-mastra and src/mastra resources.\n".to_owned()
    } else {
        "Starter prompt for the Rust Mastra scaffold.\n".to_owned()
    }
}

pub fn render_readme(project_name: &str, options: &ScaffoldOptions) -> String {
    let components = options
        .selected_components()
        .into_iter()
        .collect::<Vec<_>>()
        .join(", ");
    let template_line = options
        .template
        .as_ref()
        .map(|template| format!("- template hint: `{template}`\n"))
        .unwrap_or_default();
    let mcp_line = options
        .mcp_server
        .as_ref()
        .map(|mcp_server| format!("- MCP target: `{mcp_server}`\n"))
        .unwrap_or_default();
    let example_line = if options.add_example {
        "- example assets: included\n".to_owned()
    } else {
        "- example assets: disabled\n".to_owned()
    };

    format!(
        r#"# {project_name}

Manifest-driven Rust starter generated by `create-mastra`.

## Run

```bash
cargo run
```

## What It Does

- boots a real `mastra-core` agent
- enables in-memory message persistence through `mastra-memory`
- initializes tracing via `mastra-loggers`
- stores a loader-friendly project graph in `{mastra_dir}/mastra.json`
- ships starter assets under `{mastra_dir}/`
- selected component set: `{components}`
- selected provider hint: `{llm_provider}`
{example_line}{template_line}{mcp_line}

## Starter Layout

- `src/main.rs`: runnable bootstrap
- `{mastra_dir}/mastra.json`: starter manifest for CLI/project loaders
- `{mastra_dir}/memories/default-memory.json`: default memory node
- `{mastra_dir}/tools/demo-sum.json`: demo tool node
- `{mastra_dir}/agents/demo-agent.json`: demo agent node
- `{mastra_dir}/agents/demo-agent.md`: agent instructions
- `{mastra_dir}/workflows/demo-workflow.json`: demo workflow node
- `{mastra_dir}/resources/hello.txt`: example prompt

Edit `src/main.rs` to replace the echo model with your own agent logic, or grow the manifest/resources layout as your project evolves.
"#,
        mastra_dir = options.mastra_dir().display(),
        components = components,
        llm_provider = options.llm_provider(),
        example_line = example_line,
        template_line = template_line,
        mcp_line = mcp_line,
    )
}

pub fn render_env_example(options: &ScaffoldOptions) -> String {
    let key_value = options.llm_api_key.as_deref().unwrap_or("your-key");

    format!(
        "# Add provider credentials here when you move beyond the echo model.\n# Selected provider: {}\n# {}={}\n",
        options.llm_provider(),
        options.llm_api_key_env_name(),
        key_value,
    )
}

pub fn render_scorer_template() -> String {
    r#"// Generated by `create-mastra --components scorers`.

pub fn answer_relevancy(_prompt: &str, _response: &str) -> f32 {
    1.0
}
"#
    .to_owned()
}

pub fn default_repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("workspace root")
        .to_path_buf()
}

pub fn scaffold_with_options(
    path: &Path,
    repo_root: &Path,
    options: &ScaffoldOptions,
) -> Result<(), Box<dyn std::error::Error>> {
    let project_name = options.resolved_project_name(path);
    let paths = starter_paths(options);
    let src_dir = path.join("src");
    let mastra_dir = path.join(options.mastra_dir());
    let memory_dir = mastra_dir.join("memories");
    let tool_dir = mastra_dir.join("tools");
    let agent_dir = mastra_dir.join("agents");
    let workflow_dir = mastra_dir.join("workflows");
    let resource_dir = mastra_dir.join("resources");
    let scorer_dir = mastra_dir.join("scorers");

    fs::create_dir_all(&src_dir)?;
    fs::create_dir_all(&memory_dir)?;
    fs::create_dir_all(&tool_dir)?;
    fs::create_dir_all(&agent_dir)?;
    fs::create_dir_all(&workflow_dir)?;
    fs::create_dir_all(&resource_dir)?;
    if options.selected_components().contains("scorers") {
        fs::create_dir_all(&scorer_dir)?;
    }
    fs::write(
        path.join("Cargo.toml"),
        render_manifest(&project_name, repo_root),
    )?;
    fs::write(path.join("src/main.rs"), render_main_rs(options))?;
    fs::write(
        path.join(paths.manifest),
        render_starter_manifest(&project_name, options),
    )?;
    fs::write(path.join(paths.memory), render_memory_manifest())?;
    fs::write(path.join(paths.tool), render_tool_manifest())?;
    fs::write(path.join(paths.agent_spec), render_agent_manifest())?;
    fs::write(
        path.join(paths.agent_instructions),
        render_agent_instructions(),
    )?;
    fs::write(path.join(paths.workflow), render_workflow_manifest())?;
    fs::write(path.join(paths.prompt), render_prompt_example(options))?;
    if options.selected_components().contains("scorers") {
        fs::write(path.join(paths.scorer), render_scorer_template())?;
    }
    fs::write(
        path.join("README.md"),
        render_readme(&project_name, options),
    )?;
    fs::write(path.join(".env.example"), render_env_example(options))?;
    Ok(())
}

pub fn scaffold(path: &Path, repo_root: &Path) -> Result<(), Box<dyn std::error::Error>> {
    scaffold_with_options(path, repo_root, &ScaffoldOptions::default())
}

#[cfg(test)]
mod tests {
    use std::{fs, path::Path};

    use tempfile::tempdir;

    use super::{
        STARTER_AGENT_INSTRUCTIONS_PATH, STARTER_AGENT_SPEC_PATH, STARTER_MANIFEST_PATH,
        STARTER_MEMORY_PATH, STARTER_PROMPT_PATH, STARTER_TOOL_PATH, STARTER_WORKFLOW_PATH,
        ScaffoldOptions, render_agent_instructions, render_agent_manifest, render_env_example,
        render_main_rs, render_manifest, render_memory_manifest, render_prompt_example,
        render_readme, render_scorer_template, render_starter_manifest, render_tool_manifest,
        render_workflow_manifest, scaffold, scaffold_with_options,
    };

    #[test]
    fn render_manifest_points_to_local_workspace_crates() {
        let repo_root = Path::new("/tmp/mastra-rs");
        let manifest = render_manifest("demo-app", repo_root);

        assert!(manifest.contains("name = \"demo-app\""));
        assert!(manifest.contains("mastra-core = { path = \"/tmp/mastra-rs/packages/core\" }"));
        assert!(manifest.contains("mastra-memory = { path = \"/tmp/mastra-rs/packages/memory\" }"));
        assert!(
            manifest.contains("mastra-loggers = { path = \"/tmp/mastra-rs/packages/loggers\" }")
        );
    }

    #[test]
    fn render_main_rs_bootstraps_a_real_agent() {
        let main_rs = render_main_rs(&ScaffoldOptions::default());

        assert!(main_rs.contains("StaticModel::echo()"));
        assert!(main_rs.contains("Memory::in_memory()"));
        assert!(main_rs.contains("include_str!(\"mastra/mastra.json\")"));
        assert!(main_rs.contains("DEMO_TOOL_SPEC"));
        assert!(main_rs.contains("DEMO_WORKFLOW_SPEC"));
        assert!(main_rs.contains("DEMO_AGENT_INSTRUCTIONS"));
        assert!(main_rs.contains("DEMO_PROMPT"));
    }

    #[test]
    fn render_starter_manifest_points_to_generated_project_graph_assets() {
        let manifest = render_starter_manifest("demo-app", &ScaffoldOptions::default());

        assert!(manifest.contains("\"app_name\": \"demo-app\""));
        assert!(manifest.contains("\"memories\""));
        assert!(manifest.contains("\"tools\""));
        assert!(manifest.contains("\"mastra_dir\": \"src/mastra\""));
        assert!(manifest.contains("\"path\": \"agents/demo-agent.json\""));
        assert!(manifest.contains("\"path\": \"workflows/demo-workflow.json\""));
    }

    #[test]
    fn render_static_component_manifests_cover_memory_tool_agent_and_workflow_nodes() {
        let memory = render_memory_manifest();
        let tool = render_tool_manifest();
        let agent = render_agent_manifest();
        let workflow = render_workflow_manifest();

        assert!(memory.contains("\"kind\": \"in_memory\""));
        assert!(tool.contains("\"kind\": \"sum\""));
        assert!(agent.contains("\"kind\": \"echo\""));
        assert!(agent.contains("\"memory\": \"default-memory\""));
        assert!(workflow.contains("\"kind\": \"static_json\""));
        assert!(workflow.contains("\"tool\": \"demo-sum\""));
    }

    #[test]
    fn render_agent_instructions_and_prompt_example_are_human_editable_assets() {
        let instructions = render_agent_instructions();
        let prompt = render_prompt_example(&ScaffoldOptions::default());

        assert!(instructions.contains("# Demo Agent"));
        assert!(instructions.contains("starter agent"));
        assert!(prompt.contains("src/mastra resources"));
    }

    #[test]
    fn render_readme_mentions_generated_starter_and_run_command() {
        let readme = render_readme("demo-app", &ScaffoldOptions::default());

        assert!(readme.contains("# demo-app"));
        assert!(readme.contains("cargo run"));
        assert!(readme.contains("create-mastra"));
        assert!(readme.contains("src/mastra/mastra.json"));
        assert!(readme.contains("demo workflow node"));
    }

    #[test]
    fn render_env_example_documents_provider_placeholder() {
        let env_example = render_env_example(&ScaffoldOptions::default());

        assert!(env_example.contains("OPENAI_API_KEY"));
        assert!(env_example.contains("echo model"));
    }

    #[test]
    fn render_scorer_template_is_rust_source() {
        let scorer = render_scorer_template();

        assert!(scorer.contains("answer_relevancy"));
        assert!(scorer.contains("Generated by `create-mastra --components scorers`"));
    }

    #[test]
    fn scaffold_writes_runnable_starter_files() {
        let temp = tempdir().expect("tempdir");
        let target = temp.path().join("demo-app");
        let repo_root = Path::new("/repo/mastra-rs");

        scaffold(&target, repo_root).expect("scaffold should succeed");

        let manifest = fs::read_to_string(target.join("Cargo.toml")).expect("manifest");
        let main_rs = fs::read_to_string(target.join("src/main.rs")).expect("main");
        let starter_manifest =
            fs::read_to_string(target.join(STARTER_MANIFEST_PATH)).expect("starter manifest");
        let memory_manifest =
            fs::read_to_string(target.join(STARTER_MEMORY_PATH)).expect("memory manifest");
        let tool_manifest =
            fs::read_to_string(target.join(STARTER_TOOL_PATH)).expect("tool manifest");
        let agent_manifest =
            fs::read_to_string(target.join(STARTER_AGENT_SPEC_PATH)).expect("agent manifest");
        let instructions = fs::read_to_string(target.join(STARTER_AGENT_INSTRUCTIONS_PATH))
            .expect("agent instructions");
        let workflow_manifest =
            fs::read_to_string(target.join(STARTER_WORKFLOW_PATH)).expect("workflow manifest");
        let prompt = fs::read_to_string(target.join(STARTER_PROMPT_PATH)).expect("prompt example");
        let readme = fs::read_to_string(target.join("README.md")).expect("readme");
        let env_example = fs::read_to_string(target.join(".env.example")).expect("env example");

        assert!(manifest.contains("/repo/mastra-rs/packages/core"));
        assert!(main_rs.contains("starter graph assets loaded"));
        assert!(starter_manifest.contains("\"app_name\": \"demo-app\""));
        assert!(starter_manifest.contains("\"memories\""));
        assert!(memory_manifest.contains("\"default-memory\""));
        assert!(tool_manifest.contains("\"demo-sum\""));
        assert!(agent_manifest.contains("\"demo-agent\""));
        assert!(instructions.contains("Demo Agent"));
        assert!(workflow_manifest.contains("\"demo-workflow\""));
        assert!(prompt.contains("Hello from create-mastra"));
        assert!(readme.contains("# demo-app"));
        assert!(env_example.contains("OPENAI_API_KEY"));
    }

    #[test]
    fn scaffold_with_options_writes_selected_provider_metadata_and_scorer() {
        let temp = tempdir().expect("tempdir");
        let target = temp.path().join("demo-app");
        let repo_root = Path::new("/repo/mastra-rs");
        let options = ScaffoldOptions {
            llm_provider: Some("anthropic".to_owned()),
            llm_api_key: Some("secret-key".to_owned()),
            components: vec!["agents".to_owned(), "scorers".to_owned()],
            template: Some("template-deep-search".to_owned()),
            mcp_server: Some("vscode".to_owned()),
            add_example: false,
            ..Default::default()
        };

        scaffold_with_options(&target, repo_root, &options).expect("scaffold should succeed");

        let starter_manifest =
            fs::read_to_string(target.join(STARTER_MANIFEST_PATH)).expect("starter manifest");
        let readme = fs::read_to_string(target.join("README.md")).expect("readme");
        let env_example = fs::read_to_string(target.join(".env.example")).expect("env example");
        let scorer = fs::read_to_string(target.join("src/mastra/scorers/answer-relevancy.rs"))
            .expect("scorer");
        let prompt = fs::read_to_string(target.join(STARTER_PROMPT_PATH)).expect("prompt");

        assert!(starter_manifest.contains("\"template\": \"template-deep-search\""));
        assert!(starter_manifest.contains("\"mcp_server\": \"vscode\""));
        assert!(starter_manifest.contains("\"llm_provider\": \"anthropic\""));
        assert!(readme.contains("selected provider hint: `anthropic`"));
        assert!(readme.contains("example assets: disabled"));
        assert!(env_example.contains("ANTHROPIC_API_KEY=secret-key"));
        assert!(scorer.contains("answer_relevancy"));
        assert!(prompt.contains("Starter prompt"));
    }
}
