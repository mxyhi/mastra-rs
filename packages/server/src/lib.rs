mod contracts;
mod error;
mod registry;
mod runtime;

use std::net::SocketAddr;

use axum::{
    extract::{Path, State},
    routing::{get, post},
    Json, Router,
};
use contracts::{
    CreateWorkflowRunRequest, GenerateRequest, ListAgentsResponse, ListWorkflowsResponse,
    RouteDescription, StartWorkflowRunRequest, StartWorkflowRunResponse,
};
use error::{ServerError, ServerResult};
use mastra_core::{Agent, Workflow};
use registry::RuntimeRegistry;
use runtime::{CoreAgentRuntime, CoreWorkflowRuntime};
use uuid::Uuid;

pub use contracts::{
    AgentMessages, AgentSummary, ChatMessage, ErrorResponse, FinishReason, GenerateResponse,
    StartWorkflowRunResponse as WorkflowRunResponse, UsageStats, WorkflowRunRecord, WorkflowRunStatus,
    WorkflowSummary,
};
pub use error::ServerError as MastraServerError;
pub use registry::RuntimeRegistry as MastraRuntimeRegistry;
pub use runtime::{AgentRuntime, WorkflowRuntime};

#[derive(Clone)]
struct ServerState {
    registry: RuntimeRegistry,
}

#[derive(Clone, Default)]
pub struct MastraHttpServer {
    registry: RuntimeRegistry,
}

impl MastraHttpServer {
    pub fn new() -> Self {
        Self {
            registry: RuntimeRegistry::new(),
        }
    }

    pub fn registry(&self) -> RuntimeRegistry {
        self.registry.clone()
    }

    pub fn register_agent(&self, agent: Agent) {
        self.registry.register_agent(CoreAgentRuntime::new(agent));
    }

    pub fn register_workflow(&self, workflow: Workflow) {
        self.registry.register_workflow(CoreWorkflowRuntime::new(workflow));
    }

    pub fn router(&self) -> Router {
        let state = ServerState {
            registry: self.registry.clone(),
        };

        Router::new()
            .route("/health", get(health))
            .route("/routes", get(routes))
            .route("/agents", get(list_agents))
            .route("/agents/{agent_id}/generate", post(generate_agent))
            .route("/workflows", get(list_workflows))
            .route("/workflows/{workflow_id}/runs", post(create_workflow_run))
            .route("/workflows/{workflow_id}/runs/{run_id}", get(get_workflow_run))
            .route("/workflows/{workflow_id}/start", post(start_workflow_run))
            .with_state(state)
    }

    pub fn route_descriptions() -> Vec<RouteDescription> {
        vec![
            RouteDescription {
                method: "GET",
                path: "/health".into(),
                summary: "health check",
            },
            RouteDescription {
                method: "GET",
                path: "/routes".into(),
                summary: "list routes",
            },
            RouteDescription {
                method: "GET",
                path: "/agents".into(),
                summary: "list registered agents",
            },
            RouteDescription {
                method: "POST",
                path: "/agents/{agent_id}/generate".into(),
                summary: "generate an agent response",
            },
            RouteDescription {
                method: "GET",
                path: "/workflows".into(),
                summary: "list registered workflows",
            },
            RouteDescription {
                method: "POST",
                path: "/workflows/{workflow_id}/runs".into(),
                summary: "create a workflow run record",
            },
            RouteDescription {
                method: "GET",
                path: "/workflows/{workflow_id}/runs/{run_id}".into(),
                summary: "fetch a workflow run record",
            },
            RouteDescription {
                method: "POST",
                path: "/workflows/{workflow_id}/start".into(),
                summary: "start a workflow run",
            },
        ]
    }

    pub async fn serve(self, address: SocketAddr) -> std::io::Result<()> {
        let listener = tokio::net::TcpListener::bind(address).await?;
        axum::serve(listener, self.router()).await
    }
}

async fn health() -> &'static str {
    "ok"
}

async fn routes() -> Json<Vec<RouteDescription>> {
    Json(MastraHttpServer::route_descriptions())
}

async fn list_agents(State(state): State<ServerState>) -> Json<ListAgentsResponse> {
    Json(ListAgentsResponse {
        agents: state.registry.list_agents(),
    })
}

async fn generate_agent(
    Path(agent_id): Path<String>,
    State(state): State<ServerState>,
    Json(request): Json<GenerateRequest>,
) -> ServerResult<Json<contracts::GenerateResponse>> {
    let agent = state.registry.find_agent(&agent_id)?;
    let response = agent.generate(request).await?;
    Ok(Json(response))
}

async fn list_workflows(State(state): State<ServerState>) -> Json<ListWorkflowsResponse> {
    Json(ListWorkflowsResponse {
        workflows: state.registry.list_workflows(),
    })
}

async fn create_workflow_run(
    Path(workflow_id): Path<String>,
    State(state): State<ServerState>,
    Json(request): Json<CreateWorkflowRunRequest>,
) -> ServerResult<Json<StartWorkflowRunResponse>> {
    let run = state.registry.create_workflow_run(&workflow_id, request)?;
    Ok(Json(StartWorkflowRunResponse { run }))
}

async fn start_workflow_run(
    Path(workflow_id): Path<String>,
    State(state): State<ServerState>,
    Json(request): Json<StartWorkflowRunRequest>,
) -> ServerResult<Json<StartWorkflowRunResponse>> {
    let workflow = state.registry.find_workflow(&workflow_id)?;
    let pending = state.registry.begin_workflow_run(&workflow_id, &request)?;

    match workflow.start(request).await {
        Ok(result) => {
            let run = state
                .registry
                .complete_workflow_run_success(pending.run_id, result)?;
            Ok(Json(StartWorkflowRunResponse { run }))
        }
        Err(error) => {
            let run = state
                .registry
                .complete_workflow_run_failure(pending.run_id, &error)?;
            Err(ServerError::internal(run.error.unwrap_or_else(|| error.to_string())))
        }
    }
}

async fn get_workflow_run(
    Path((workflow_id, run_id)): Path<(String, Uuid)>,
    State(state): State<ServerState>,
) -> ServerResult<Json<StartWorkflowRunResponse>> {
    let run = state.registry.get_workflow_run(&workflow_id, run_id)?;
    Ok(Json(StartWorkflowRunResponse { run }))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use mastra_core::{
        Agent, AgentConfig, MemoryConfig, RequestContext, StaticModel, Step, Workflow,
    };
    use serde_json::json;

    use crate::MastraHttpServer;

    #[tokio::test]
    async fn server_registers_core_primitives() {
        let server = MastraHttpServer::new();
        server.register_agent(Agent::new(AgentConfig {
            id: "agent-1".into(),
            name: "Agent 1".into(),
            instructions: "Echo".into(),
            description: Some("example".into()),
            model: Arc::new(StaticModel::echo()),
            tools: Vec::new(),
            memory: None,
            memory_config: MemoryConfig::default(),
        }));
        server.register_workflow(
            Workflow::new("workflow-1").then(Step::new("step-1", |input, _| async move { Ok(input) })),
        );

        assert_eq!(server.registry().list_agents().len(), 1);
        assert_eq!(server.registry().list_workflows().len(), 1);

        let run = server
            .registry()
            .create_workflow_run(
                "workflow-1",
                crate::contracts::CreateWorkflowRunRequest {
                    resource_id: None,
                    input_data: Some(json!({"hello":"world"})),
                    request_context: RequestContext::new().values().clone(),
                },
            )
            .expect("run should be created");
        assert_eq!(run.workflow_id, "workflow-1");
    }
}
