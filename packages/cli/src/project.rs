use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

use mastra_core::{
    Agent, AgentConfig, FinishReason, LanguageModel, MemoryConfig, MemoryEngine, ModelResponse,
    StaticModel, Step, Tool, Workflow,
};
use mastra_memory::{ListThreadsQuery, Memory};
use mastra_server::MastraHttpServer;
use mastra_store_libsql::{LibSqlStore, LibSqlStoreConfig};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

pub const DEFAULT_MASTRA_DIR: &str = "src/mastra";
pub const DEFAULT_BUILD_DIR: &str = ".mastra/output";
pub const MANIFEST_FILE_NAME: &str = "mastra.json";
pub const BUNDLE_FILE_NAME: &str = "bundle.json";
pub const ROUTES_FILE_NAME: &str = "routes.txt";
pub const DEFAULT_SCORERS_DIR: &str = "scorers";

pub type CliResult<T> = Result<T, Box<dyn Error>>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProjectManifest {
    pub app_name: String,
    #[serde(default)]
    pub memories: Vec<MemorySpec>,
    #[serde(default)]
    pub tools: Vec<ToolSpec>,
    #[serde(default)]
    pub agents: Vec<AgentSpec>,
    #[serde(default)]
    pub workflows: Vec<WorkflowSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MemorySpec {
    pub id: String,
    #[serde(flatten)]
    pub kind: MemoryKind,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MemoryKind {
    InMemory,
    Libsql { url: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolSpec {
    pub id: String,
    pub description: String,
    #[serde(default)]
    pub require_approval: bool,
    #[serde(flatten)]
    pub kind: ToolKind,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ToolKind {
    EchoInput,
    StaticJson { output: Value },
    Sum,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentSpec {
    pub id: String,
    pub name: String,
    pub instructions: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub tools: Vec<String>,
    #[serde(default)]
    pub memory: Option<String>,
    #[serde(flatten)]
    pub model: ModelSpec,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "model_kind", rename_all = "snake_case")]
pub enum ModelSpec {
    Echo,
    PrefixedEcho { prefix: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkflowSpec {
    pub id: String,
    #[serde(default)]
    pub steps: Vec<WorkflowStepSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WorkflowStepSpec {
    Identity { id: String },
    StaticJson { id: String, output: Value },
    Tool { id: String, tool: String },
    Agent { id: String, agent: String },
}

#[derive(Debug, Deserialize)]
struct GraphProjectManifest {
    pub app_name: String,
    #[serde(default)]
    pub memories: Vec<GraphNodeRef>,
    #[serde(default)]
    pub tools: Vec<GraphNodeRef>,
    #[serde(default)]
    pub agents: Vec<GraphNodeRef>,
    #[serde(default)]
    pub workflows: Vec<GraphNodeRef>,
}

#[derive(Debug, Deserialize)]
struct GraphNodeRef {
    pub id: String,
    pub path: String,
}

#[derive(Debug, Deserialize)]
struct GraphMemoryNode {
    pub id: String,
    #[serde(flatten)]
    pub kind: MemoryKind,
}

#[derive(Debug, Deserialize)]
struct GraphToolNode {
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub require_approval: bool,
    #[serde(flatten)]
    pub kind: ToolKind,
}

#[derive(Debug, Deserialize)]
struct GraphAgentNode {
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub instructions: Option<String>,
    #[serde(default)]
    pub instructions_path: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub tools: Vec<String>,
    #[serde(default)]
    pub memory: Option<String>,
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub model: Option<GraphModelNode>,
}

#[derive(Debug, Deserialize)]
struct GraphModelNode {
    pub kind: String,
    #[serde(default)]
    pub prefix: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GraphWorkflowNode {
    pub id: String,
    #[serde(default)]
    pub steps: Vec<GraphWorkflowStepNode>,
}

#[derive(Debug, Deserialize)]
struct GraphWorkflowStepNode {
    pub id: String,
    pub kind: String,
    #[serde(default)]
    pub output: Option<Value>,
    #[serde(default)]
    pub tool: Option<String>,
    #[serde(default)]
    pub agent: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProjectBundle {
    pub manifest: ProjectManifest,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectSummary {
    pub app_name: String,
    pub memories: usize,
    pub tools: usize,
    pub agents: usize,
    pub workflows: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildSummary {
    pub output_dir: PathBuf,
    pub manifest_path: PathBuf,
    pub routes_path: PathBuf,
    pub project: ProjectSummary,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationSummary {
    pub migrated_memory_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScorerTemplate {
    pub id: &'static str,
    pub description: &'static str,
}

const BUILTIN_SCORERS: &[ScorerTemplate] = &[
    ScorerTemplate {
        id: "answer-relevancy",
        description: "Evaluate whether an answer directly addresses the prompt.",
    },
    ScorerTemplate {
        id: "groundedness",
        description: "Evaluate whether an answer stays grounded in the supplied context.",
    },
    ScorerTemplate {
        id: "faithfulness",
        description: "Evaluate whether an answer preserves important source facts.",
    },
];

impl ProjectManifest {
    pub fn validate(&self) -> CliResult<()> {
        if self.app_name.trim().is_empty() {
            return Err("project manifest app_name must not be blank".into());
        }

        validate_unique_ids(
            "memory",
            self.memories.iter().map(|memory| memory.id.as_str()),
        )?;
        validate_unique_ids("tool", self.tools.iter().map(|tool| tool.id.as_str()))?;
        validate_unique_ids("agent", self.agents.iter().map(|agent| agent.id.as_str()))?;
        validate_unique_ids(
            "workflow",
            self.workflows.iter().map(|workflow| workflow.id.as_str()),
        )?;

        let memory_ids = self
            .memories
            .iter()
            .map(|memory| memory.id.as_str())
            .collect::<BTreeSet<_>>();
        let tool_ids = self
            .tools
            .iter()
            .map(|tool| tool.id.as_str())
            .collect::<BTreeSet<_>>();
        let agent_ids = self
            .agents
            .iter()
            .map(|agent| agent.id.as_str())
            .collect::<BTreeSet<_>>();

        for agent in &self.agents {
            if let Some(memory_id) = agent.memory.as_deref() {
                if !memory_ids.contains(memory_id) {
                    return Err(format!(
                        "agent '{}' references unknown memory '{}'",
                        agent.id, memory_id
                    )
                    .into());
                }
            }

            for tool_id in &agent.tools {
                if !tool_ids.contains(tool_id.as_str()) {
                    return Err(format!(
                        "agent '{}' references unknown tool '{}'",
                        agent.id, tool_id
                    )
                    .into());
                }
            }
        }

        for workflow in &self.workflows {
            validate_unique_ids(
                &format!("workflow '{}' step", workflow.id),
                workflow.steps.iter().map(WorkflowStepSpec::id),
            )?;

            for step in &workflow.steps {
                match step {
                    WorkflowStepSpec::Identity { .. } => {}
                    WorkflowStepSpec::StaticJson { .. } => {}
                    WorkflowStepSpec::Tool { tool, .. } => {
                        if !tool_ids.contains(tool.as_str()) {
                            return Err(format!(
                                "workflow '{}' references unknown tool '{}'",
                                workflow.id, tool
                            )
                            .into());
                        }
                    }
                    WorkflowStepSpec::Agent { agent, .. } => {
                        if !agent_ids.contains(agent.as_str()) {
                            return Err(format!(
                                "workflow '{}' references unknown agent '{}'",
                                workflow.id, agent
                            )
                            .into());
                        }
                    }
                }
            }
        }

        Ok(())
    }

    pub fn summary(&self) -> ProjectSummary {
        ProjectSummary {
            app_name: self.app_name.clone(),
            memories: self.memories.len(),
            tools: self.tools.len(),
            agents: self.agents.len(),
            workflows: self.workflows.len(),
        }
    }
}

impl WorkflowStepSpec {
    pub fn id(&self) -> &str {
        match self {
            Self::Identity { id }
            | Self::StaticJson { id, .. }
            | Self::Tool { id, .. }
            | Self::Agent { id, .. } => id,
        }
    }
}

pub fn default_mastra_dir() -> PathBuf {
    PathBuf::from(DEFAULT_MASTRA_DIR)
}

pub fn default_build_dir() -> PathBuf {
    PathBuf::from(DEFAULT_BUILD_DIR)
}

pub fn manifest_path(root: &Path, dir: &Path) -> PathBuf {
    resolve_path(root, dir).join(MANIFEST_FILE_NAME)
}

pub fn bundle_path(output_dir: &Path) -> PathBuf {
    output_dir.join(BUNDLE_FILE_NAME)
}

pub fn routes_path(output_dir: &Path) -> PathBuf {
    output_dir.join(ROUTES_FILE_NAME)
}

pub fn load_project_manifest(root: &Path, dir: &Path) -> CliResult<ProjectManifest> {
    let path = manifest_path(root, dir);
    let raw = fs::read_to_string(path)?;
    let value = serde_json::from_str::<Value>(&raw)?;
    let manifest = if value.get("schema_version").is_some() {
        normalize_graph_manifest(root, dir, value)?
    } else {
        serde_json::from_value::<ProjectManifest>(value)?
    };
    manifest.validate()?;
    Ok(manifest)
}

pub fn load_project_bundle(output_dir: &Path) -> CliResult<ProjectBundle> {
    let raw = fs::read_to_string(bundle_path(output_dir))?;
    let bundle = serde_json::from_str::<ProjectBundle>(&raw)?;
    bundle.manifest.validate()?;
    Ok(bundle)
}

pub fn build_server_from_manifest(
    root: &Path,
    manifest: &ProjectManifest,
) -> CliResult<MastraHttpServer> {
    manifest.validate()?;

    let server = MastraHttpServer::new();
    let memories = build_memories(root, &manifest.memories)?;
    let tools = build_tools(&manifest.tools);
    let agents = build_agents(&manifest.agents, &tools, &memories);
    let workflows = build_workflows(&manifest.workflows, &tools, &agents);

    for (memory_id, memory) in memories {
        server.register_memory(memory_id, memory);
    }
    for tool in tools.into_values() {
        server.register_tool(tool);
    }
    for agent in agents.into_values() {
        server.register_agent(agent);
    }
    for workflow in workflows.into_values() {
        server.register_workflow(workflow);
    }

    Ok(server)
}

pub fn lint_project(root: &Path, dir: &Path) -> CliResult<ProjectSummary> {
    load_project_manifest(root, dir).map(|manifest| manifest.summary())
}

pub fn build_project(root: &Path, dir: &Path, output_dir: &Path) -> CliResult<BuildSummary> {
    let manifest = load_project_manifest(root, dir)?;
    let _server = build_server_from_manifest(root, &manifest)?;
    fs::create_dir_all(output_dir)?;

    let manifest_path = bundle_path(output_dir);
    let routes_path = routes_path(output_dir);
    let bundle = ProjectBundle {
        manifest: manifest.clone(),
    };
    fs::write(
        &manifest_path,
        serde_json::to_string_pretty(&bundle).expect("bundle should serialize"),
    )?;
    fs::write(
        &routes_path,
        MastraHttpServer::route_descriptions()
            .iter()
            .map(|route| format!("{} {}  # {}", route.method, route.path, route.summary))
            .collect::<Vec<_>>()
            .join("\n"),
    )?;

    Ok(BuildSummary {
        output_dir: output_dir.to_path_buf(),
        manifest_path,
        routes_path,
        project: manifest.summary(),
    })
}

pub async fn migrate_project(root: &Path, dir: &Path) -> CliResult<MigrationSummary> {
    let manifest = load_project_manifest(root, dir)?;
    let mut migrated_memory_ids = Vec::new();

    for memory in &manifest.memories {
        if let MemoryKind::Libsql { .. } = memory.kind {
            let memory_engine = build_memory(root, memory)?;
            memory_engine
                .list_threads(ListThreadsQuery::default())
                .await
                .map_err(|error| format!("migrate memory '{}': {error}", memory.id))?;
            migrated_memory_ids.push(memory.id.clone());
        }
    }

    Ok(MigrationSummary {
        migrated_memory_ids,
    })
}

pub fn list_builtin_scorers() -> &'static [ScorerTemplate] {
    BUILTIN_SCORERS
}

pub fn add_scorer(root: &Path, dir: &Path, scorer_name: &str) -> CliResult<PathBuf> {
    let scorer = BUILTIN_SCORERS
        .iter()
        .find(|candidate| candidate.id == scorer_name)
        .ok_or_else(|| format!("unknown scorer template '{scorer_name}'"))?;
    let scorers_dir = resolve_path(root, dir).join(DEFAULT_SCORERS_DIR);
    fs::create_dir_all(&scorers_dir)?;
    let destination = scorers_dir.join(format!("{scorer_name}.rs"));
    if destination.exists() {
        return Err(format!("scorer file {} already exists", destination.display()).into());
    }

    fs::write(
        &destination,
        render_scorer_template(scorer.id, scorer.description),
    )?;
    Ok(destination)
}

fn validate_unique_ids<'a>(label: &str, ids: impl Iterator<Item = &'a str>) -> CliResult<()> {
    let mut seen = BTreeSet::new();
    for id in ids {
        if id.trim().is_empty() {
            return Err(format!("{label} id must not be blank").into());
        }
        if !seen.insert(id.to_owned()) {
            return Err(format!("duplicate {label} id '{id}'").into());
        }
    }
    Ok(())
}

fn resolve_path(root: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    }
}

fn normalize_graph_manifest(root: &Path, dir: &Path, value: Value) -> CliResult<ProjectManifest> {
    let manifest = serde_json::from_value::<GraphProjectManifest>(value)?;
    let graph_root = resolve_path(root, dir);

    Ok(ProjectManifest {
        app_name: manifest.app_name,
        memories: manifest
            .memories
            .iter()
            .map(|node_ref| load_graph_memory(&graph_root, node_ref))
            .collect::<CliResult<Vec<_>>>()?,
        tools: manifest
            .tools
            .iter()
            .map(|node_ref| load_graph_tool(&graph_root, node_ref))
            .collect::<CliResult<Vec<_>>>()?,
        agents: manifest
            .agents
            .iter()
            .map(|node_ref| load_graph_agent(&graph_root, node_ref))
            .collect::<CliResult<Vec<_>>>()?,
        workflows: manifest
            .workflows
            .iter()
            .map(|node_ref| load_graph_workflow(&graph_root, node_ref))
            .collect::<CliResult<Vec<_>>>()?,
    })
}

fn load_graph_memory(root: &Path, node_ref: &GraphNodeRef) -> CliResult<MemorySpec> {
    let node = load_graph_node::<GraphMemoryNode>(root, node_ref)?;
    Ok(MemorySpec {
        id: node.id,
        kind: node.kind,
    })
}

fn load_graph_tool(root: &Path, node_ref: &GraphNodeRef) -> CliResult<ToolSpec> {
    let node = load_graph_node::<GraphToolNode>(root, node_ref)?;
    Ok(ToolSpec {
        id: node.id.clone(),
        description: node
            .description
            .or(node.name)
            .unwrap_or_else(|| format!("Tool {}", node.id)),
        require_approval: node.require_approval,
        kind: node.kind,
    })
}

fn load_graph_agent(root: &Path, node_ref: &GraphNodeRef) -> CliResult<AgentSpec> {
    let node = load_graph_node::<GraphAgentNode>(root, node_ref)?;
    let instructions = if let Some(instructions) = node.instructions {
        instructions
    } else if let Some(path) = node.instructions_path.as_deref() {
        fs::read_to_string(resolve_path(root, Path::new(path)))?
    } else {
        return Err(format!("agent '{}' is missing instructions", node.id).into());
    };

    let model = if let Some(model) = node.model {
        normalize_model(model)?
    } else if let Some(kind) = node.kind.as_deref() {
        normalize_model(GraphModelNode {
            kind: kind.to_owned(),
            prefix: None,
        })?
    } else {
        return Err(format!("agent '{}' is missing model configuration", node.id).into());
    };

    Ok(AgentSpec {
        id: node.id.clone(),
        name: node.name.unwrap_or_else(|| node.id.clone()),
        instructions: instructions.trim().to_owned(),
        description: node.description,
        tools: node.tools,
        memory: node.memory,
        model,
    })
}

fn load_graph_workflow(root: &Path, node_ref: &GraphNodeRef) -> CliResult<WorkflowSpec> {
    let node = load_graph_node::<GraphWorkflowNode>(root, node_ref)?;
    Ok(WorkflowSpec {
        id: node.id,
        steps: node
            .steps
            .into_iter()
            .map(normalize_workflow_step)
            .collect::<CliResult<Vec<_>>>()?,
    })
}

fn load_graph_node<T>(root: &Path, node_ref: &GraphNodeRef) -> CliResult<T>
where
    T: for<'de> Deserialize<'de> + GraphNodeId,
{
    let raw = fs::read_to_string(resolve_path(root, Path::new(&node_ref.path)))?;
    let node = serde_json::from_str::<T>(&raw)?;
    if node.graph_id() != node_ref.id {
        return Err(format!(
            "graph node reference '{}' does not match file id '{}'",
            node_ref.id,
            node.graph_id()
        )
        .into());
    }
    Ok(node)
}

trait GraphNodeId {
    fn graph_id(&self) -> &str;
}

impl GraphNodeId for GraphMemoryNode {
    fn graph_id(&self) -> &str {
        &self.id
    }
}

impl GraphNodeId for GraphToolNode {
    fn graph_id(&self) -> &str {
        &self.id
    }
}

impl GraphNodeId for GraphAgentNode {
    fn graph_id(&self) -> &str {
        &self.id
    }
}

impl GraphNodeId for GraphWorkflowNode {
    fn graph_id(&self) -> &str {
        &self.id
    }
}

fn normalize_model(model: GraphModelNode) -> CliResult<ModelSpec> {
    match model.kind.as_str() {
        "echo" => Ok(ModelSpec::Echo),
        "prefixed_echo" => Ok(ModelSpec::PrefixedEcho {
            prefix: model
                .prefix
                .ok_or_else(|| "prefixed_echo model requires prefix".to_owned())?,
        }),
        other => Err(format!("unsupported graph model kind '{other}'").into()),
    }
}

fn normalize_workflow_step(step: GraphWorkflowStepNode) -> CliResult<WorkflowStepSpec> {
    match step.kind.as_str() {
        "identity" => Ok(WorkflowStepSpec::Identity { id: step.id }),
        "static_json" => Ok(WorkflowStepSpec::StaticJson {
            id: step.id,
            output: step
                .output
                .ok_or_else(|| "static_json workflow step requires output".to_owned())?,
        }),
        "tool" => Ok(WorkflowStepSpec::Tool {
            id: step.id,
            tool: step
                .tool
                .ok_or_else(|| "tool workflow step requires tool".to_owned())?,
        }),
        "agent" => Ok(WorkflowStepSpec::Agent {
            id: step.id,
            agent: step
                .agent
                .ok_or_else(|| "agent workflow step requires agent".to_owned())?,
        }),
        other => Err(format!("unsupported workflow step kind '{other}'").into()),
    }
}

fn build_memories(
    root: &Path,
    specs: &[MemorySpec],
) -> CliResult<BTreeMap<String, Arc<dyn MemoryEngine>>> {
    specs
        .iter()
        .try_fold(BTreeMap::new(), |mut memories, spec| {
            let memory = build_memory(root, spec)?;
            memories.insert(spec.id.clone(), Arc::new(memory) as Arc<dyn MemoryEngine>);
            Ok(memories)
        })
}

fn build_memory(root: &Path, spec: &MemorySpec) -> CliResult<Memory> {
    match &spec.kind {
        MemoryKind::InMemory => Ok(Memory::in_memory()),
        MemoryKind::Libsql { url } => {
            let resolved_url = resolve_relative_url(root, url);
            ensure_libsql_parent(&resolved_url)?;
            Ok(Memory::new(LibSqlStore::new(LibSqlStoreConfig {
                url: resolved_url,
            })))
        }
    }
}

fn resolve_relative_url(root: &Path, url: &str) -> String {
    if let Some(path) = url.strip_prefix("file:") {
        let path = PathBuf::from(path);
        if path.is_relative() {
            return format!("file:{}", root.join(path).display());
        }
    }
    url.to_owned()
}

fn ensure_libsql_parent(url: &str) -> CliResult<()> {
    if let Some(path) = url.strip_prefix("file:") {
        let path = Path::new(path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
    }
    Ok(())
}

fn build_tools(specs: &[ToolSpec]) -> BTreeMap<String, Tool> {
    specs
        .iter()
        .map(|spec| {
            let base = match &spec.kind {
                ToolKind::EchoInput => Tool::new(
                    spec.id.clone(),
                    spec.description.clone(),
                    |input, _context| async move { Ok(input) },
                ),
                ToolKind::StaticJson { output } => {
                    let output = output.clone();
                    Tool::new(
                        spec.id.clone(),
                        spec.description.clone(),
                        move |_input, _context| {
                            let output = output.clone();
                            async move { Ok(output) }
                        },
                    )
                }
                ToolKind::Sum => Tool::new(
                    spec.id.clone(),
                    spec.description.clone(),
                    |input, _context| async move {
                        let (left, right) = extract_sum_operands(&input)?;
                        Ok(json!({
                            "left": left,
                            "right": right,
                            "result": left + right,
                        }))
                    },
                ),
            };
            let tool = if spec.require_approval {
                base.requiring_approval()
            } else {
                base
            };
            (spec.id.clone(), tool)
        })
        .collect()
}

fn extract_sum_operands(input: &Value) -> Result<(f64, f64), mastra_core::MastraError> {
    if let Some(array) = input.as_array() {
        if array.len() == 2 {
            let left = array[0].as_f64().ok_or_else(|| {
                mastra_core::MastraError::validation(
                    "sum tool expects array operands to be numbers".to_owned(),
                )
            })?;
            let right = array[1].as_f64().ok_or_else(|| {
                mastra_core::MastraError::validation(
                    "sum tool expects array operands to be numbers".to_owned(),
                )
            })?;
            return Ok((left, right));
        }
    }

    let left = input
        .get("a")
        .or_else(|| input.get("left"))
        .and_then(Value::as_f64)
        .ok_or_else(|| {
            mastra_core::MastraError::validation(
                "sum tool expects an object with numeric 'a'/'left'".to_owned(),
            )
        })?;
    let right = input
        .get("b")
        .or_else(|| input.get("right"))
        .and_then(Value::as_f64)
        .ok_or_else(|| {
            mastra_core::MastraError::validation(
                "sum tool expects an object with numeric 'b'/'right'".to_owned(),
            )
        })?;
    Ok((left, right))
}

fn build_agents(
    specs: &[AgentSpec],
    tools: &BTreeMap<String, Tool>,
    memories: &BTreeMap<String, Arc<dyn MemoryEngine>>,
) -> BTreeMap<String, Agent> {
    specs
        .iter()
        .map(|spec| {
            let agent_tools = spec
                .tools
                .iter()
                .filter_map(|tool_id| tools.get(tool_id).cloned())
                .collect::<Vec<_>>();
            let memory = spec
                .memory
                .as_ref()
                .and_then(|memory_id| memories.get(memory_id))
                .cloned();

            let agent = Agent::new(AgentConfig {
                id: spec.id.clone(),
                name: spec.name.clone(),
                instructions: spec.instructions.clone(),
                description: spec.description.clone(),
                model: build_model(&spec.model),
                tools: agent_tools,
                memory,
                memory_config: MemoryConfig::default(),
            });
            (spec.id.clone(), agent)
        })
        .collect()
}

fn build_model(spec: &ModelSpec) -> Arc<dyn LanguageModel> {
    match spec {
        ModelSpec::Echo => Arc::new(StaticModel::echo()),
        ModelSpec::PrefixedEcho { prefix } => {
            let prefix = prefix.clone();
            Arc::new(StaticModel::new(move |request| {
                let prefix = prefix.clone();
                async move {
                    let memory_prefix = if request.memory.is_empty() {
                        String::new()
                    } else {
                        format!("{}\n", request.memory.join("\n"))
                    };
                    Ok(ModelResponse {
                        text: format!("{memory_prefix}{prefix}{}", request.prompt),
                        data: Value::Null,
                        finish_reason: FinishReason::Stop,
                        usage: None,
                        tool_calls: Vec::new(),
                    })
                }
            }))
        }
    }
}

fn build_workflows(
    specs: &[WorkflowSpec],
    tools: &BTreeMap<String, Tool>,
    agents: &BTreeMap<String, Agent>,
) -> BTreeMap<String, Workflow> {
    specs
        .iter()
        .map(|spec| {
            let workflow =
                spec.steps
                    .iter()
                    .fold(Workflow::new(spec.id.clone()), |workflow, step| {
                        let next = match step {
                            WorkflowStepSpec::Identity { id } => {
                                Step::new(id.clone(), |input, _context| async move { Ok(input) })
                            }
                            WorkflowStepSpec::StaticJson { id, output } => {
                                let output = output.clone();
                                Step::new(id.clone(), move |_input, _context| {
                                    let output = output.clone();
                                    async move { Ok(output) }
                                })
                            }
                            WorkflowStepSpec::Tool { tool, .. } => Step::from_tool(
                                tools.get(tool).cloned().expect("validated tool reference"),
                            ),
                            WorkflowStepSpec::Agent { agent, .. } => Step::from_agent(
                                agents
                                    .get(agent)
                                    .cloned()
                                    .expect("validated agent reference"),
                            ),
                        };
                        workflow.then(next)
                    });
            (spec.id.clone(), workflow)
        })
        .collect()
}

fn render_scorer_template(id: &str, description: &str) -> String {
    format!(
        r#"// Generated by `mastra scorers add {id}`.
// {description}

pub fn score(input: &str, output: &str) -> f64 {{
    if input.trim().is_empty() || output.trim().is_empty() {{
        return 0.0;
    }}

    1.0
}}
"#
    )
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use create_mastra::{default_repo_root, scaffold};
    use tempfile::tempdir;

    use super::{
        BUNDLE_FILE_NAME, DEFAULT_SCORERS_DIR, ProjectBundle, ProjectManifest, add_scorer,
        build_project, build_server_from_manifest, bundle_path, default_build_dir,
        default_mastra_dir, lint_project, load_project_bundle, load_project_manifest,
        manifest_path, migrate_project, routes_path,
    };

    fn demo_manifest() -> ProjectManifest {
        serde_json::from_value(serde_json::json!({
            "app_name": "demo-app",
            "memories": [
                {
                    "id": "default",
                    "kind": "in_memory"
                },
                {
                    "id": "disk",
                    "kind": "libsql",
                    "url": "file:.mastra/memory.db"
                }
            ],
            "tools": [
                {
                    "id": "sum",
                    "description": "Add numbers",
                    "kind": "sum"
                },
                {
                    "id": "static",
                    "description": "Return static JSON",
                    "kind": "static_json",
                    "output": {
                        "ok": true
                    }
                }
            ],
            "agents": [
                {
                    "id": "echo",
                    "name": "Echo",
                    "instructions": "Echo the incoming prompt.",
                    "description": "Demo agent",
                    "tools": ["sum"],
                    "memory": "default",
                    "model_kind": "prefixed_echo",
                    "prefix": "agent: "
                }
            ],
            "workflows": [
                {
                    "id": "demo",
                    "steps": [
                        {
                            "id": "sum-step",
                            "kind": "tool",
                            "tool": "sum"
                        },
                        {
                            "id": "echo-step",
                            "kind": "agent",
                            "agent": "echo"
                        }
                    ]
                }
            ]
        }))
        .expect("manifest")
    }

    #[test]
    fn defaults_match_documented_paths() {
        assert_eq!(default_mastra_dir(), Path::new("src/mastra"));
        assert_eq!(default_build_dir(), Path::new(".mastra/output"));
    }

    #[test]
    fn manifest_round_trip_and_validation_succeed() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path();
        let mastra_dir = root.join("src/mastra");
        std::fs::create_dir_all(&mastra_dir).expect("mastra dir");
        std::fs::write(
            mastra_dir.join("mastra.json"),
            serde_json::to_string_pretty(&demo_manifest()).expect("json"),
        )
        .expect("manifest write");

        let manifest = load_project_manifest(root, Path::new("src/mastra")).expect("manifest");
        assert_eq!(manifest.app_name, "demo-app");
        assert_eq!(manifest.summary().agents, 1);

        let server = build_server_from_manifest(root, &manifest).expect("server");
        assert_eq!(server.registry().list_agents().len(), 1);
        assert_eq!(server.registry().list_workflows().len(), 1);
        assert_eq!(server.registry().list_memory().len(), 2);
    }

    #[test]
    fn lint_and_build_write_bundle_and_routes() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path();
        let mastra_dir = root.join("src/mastra");
        std::fs::create_dir_all(&mastra_dir).expect("mastra dir");
        std::fs::write(
            manifest_path(root, Path::new("src/mastra")),
            serde_json::to_string_pretty(&demo_manifest()).expect("json"),
        )
        .expect("manifest write");

        let summary = lint_project(root, Path::new("src/mastra")).expect("lint");
        assert_eq!(summary.tools, 2);

        let output_dir = root.join(".mastra/output");
        let build = build_project(root, Path::new("src/mastra"), &output_dir).expect("build");
        assert_eq!(build.project.workflows, 1);
        assert!(bundle_path(&output_dir).exists());
        assert!(routes_path(&output_dir).exists());
        assert_eq!(build.manifest_path, output_dir.join(BUNDLE_FILE_NAME));
    }

    #[test]
    fn bundle_loader_round_trips_manifest() {
        let temp = tempdir().expect("tempdir");
        let output_dir = temp.path().join(".mastra/output");
        std::fs::create_dir_all(&output_dir).expect("output dir");
        let bundle = ProjectBundle {
            manifest: demo_manifest(),
        };
        std::fs::write(
            bundle_path(&output_dir),
            serde_json::to_string_pretty(&bundle).expect("json"),
        )
        .expect("bundle write");

        let loaded = load_project_bundle(&output_dir).expect("bundle");
        assert_eq!(loaded.manifest.app_name, "demo-app");
    }

    #[tokio::test]
    async fn migrate_initializes_libsql_memory() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path();
        let mastra_dir = root.join("src/mastra");
        std::fs::create_dir_all(&mastra_dir).expect("mastra dir");
        std::fs::write(
            manifest_path(root, Path::new("src/mastra")),
            serde_json::to_string_pretty(&demo_manifest()).expect("json"),
        )
        .expect("manifest write");

        let summary = migrate_project(root, Path::new("src/mastra"))
            .await
            .expect("migrate");

        assert_eq!(summary.migrated_memory_ids, vec!["disk".to_owned()]);
        assert!(root.join(".mastra/memory.db").exists());
    }

    #[test]
    fn add_scorer_creates_template_file() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path();
        let path = add_scorer(root, Path::new("src/mastra"), "answer-relevancy").expect("scorer");
        let body = std::fs::read_to_string(&path).expect("scorer body");
        assert!(body.contains("score(input"));
        assert!(path.ends_with(format!("{DEFAULT_SCORERS_DIR}/answer-relevancy.rs")));
    }

    #[test]
    fn starter_graph_manifest_from_create_mastra_loads_and_builds() {
        let temp = tempdir().expect("tempdir");
        let project_root = temp.path().join("starter-app");
        scaffold(&project_root, &default_repo_root()).expect("scaffold");

        let manifest =
            load_project_manifest(&project_root, Path::new("src/mastra")).expect("manifest");
        assert_eq!(manifest.app_name, "starter-app");
        assert_eq!(manifest.memories.len(), 1);
        assert_eq!(manifest.tools.len(), 1);
        assert_eq!(manifest.agents.len(), 1);
        assert_eq!(manifest.workflows.len(), 1);

        let build_dir = project_root.join(".mastra/output");
        let summary =
            build_project(&project_root, Path::new("src/mastra"), &build_dir).expect("build");
        assert!(summary.manifest_path.exists());
        assert!(summary.routes_path.exists());
    }
}
