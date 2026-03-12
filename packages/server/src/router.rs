use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use tracing::instrument;
use uuid::Uuid;

use crate::{
    contracts::{
        CreateWorkflowRunRequest, GenerateRequest, ListAgentsResponse,
        ListWorkflowsResponse, RouteDescription, StartWorkflowRunRequest,
        StartWorkflowRunResponse, WorkflowRunRecord,
    },
    error::{ServerError, ServerResult},
    registry::RuntimeRegistry,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerConfig {
    pub api_prefix: String,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            api_prefix: "/api".to_owned(),
        }
    }
}

#[derive(Clone)]
pub struct MastraServer {
    registry: RuntimeRegistry,
    config: ServerConfig,
}

#[derive(Clone)]
struct AppState {
    registry: RuntimeRegistry,
}

impl MastraServer {
    pub fn new(registry: RuntimeRegistry) -> Self {
        Self {
            registry,
            config: ServerConfig::default(),
        }
    }

    pub fn with_config(mut self, config: ServerConfig) -> Self {
        self.config = ServerConfig {
            api_prefix: normalize_prefix(&config.api_prefix),
        };
        self
    }

    pub fn with_prefix(self, api_prefix: impl Into<String>) -> Self {
        self.with_config(ServerConfig {
            api_prefix: api_prefix.into(),
        })
    }

    pub fn registry(&self) -> &RuntimeRegistry {
        &self.registry
    }

    pub fn into_router(self) -> Router {
        let api_router = Router::new()
            .route("/agents", get(list_agents))
            .route("/agents/{agent_id}/generate", post(generate_agent))
            .route("/workflows", get(list_workflows))
            .route("/workflows/{workflow_id}/create-run", post(create_workflow_run))
            .route("/workflows/{workflow_id}/start-async", post(start_workflow_async))
            .route(
                "/workflows/{workflow_id}/runs/{run_id}",
                get(get_workflow_run),
            )
            .with_state(AppState {
                registry: self.registry.clone(),
            });

        let prefix = normalize_prefix(&self.config.api_prefix);
        if prefix.is_empty() {
            api_router
        } else {
            Router::new().nest(&prefix, api_router)
        }
    }

    pub fn route_catalog(&self) -> Vec<RouteDescription> {
        route_catalog(&self.config.api_prefix)
    }
}

pub fn route_catalog(prefix: &str) -> Vec<RouteDescription> {
    let prefix = normalize_prefix(prefix);
    [
        ("GET", "/agents", "List registered agents"),
        ("POST", "/agents/{agent_id}/generate", "Generate an agent response"),
        ("GET", "/workflows", "List registered workflows"),
        (
            "POST",
            "/workflows/{workflow_id}/create-run",
            "Create a workflow run record",
        ),
        (
            "POST",
            "/workflows/{workflow_id}/start-async",
            "Execute a workflow run immediately",
        ),
        (
            "GET",
            "/workflows/{workflow_id}/runs/{run_id}",
            "Fetch a workflow run record",
        ),
    ]
    .into_iter()
    .map(|(method, path, summary)| RouteDescription {
        method,
        path: format!("{}{}", prefix, path),
        summary,
    })
    .collect()
}

fn normalize_prefix(prefix: &str) -> String {
    let trimmed = prefix.trim();
    if trimmed.is_empty() || trimmed == "/" {
        String::new()
    } else {
        format!("/{}", trimmed.trim_matches('/'))
    }
}

#[instrument(skip(state))]
async fn list_agents(
    State(state): State<AppState>,
) -> Json<ListAgentsResponse> {
    Json(ListAgentsResponse {
        agents: state.registry.list_agents(),
    })
}

#[instrument(skip(state, request))]
async fn generate_agent(
    State(state): State<AppState>,
    Path(agent_id): Path<String>,
    Json(request): Json<GenerateRequest>,
) -> ServerResult<Json<crate::contracts::GenerateResponse>> {
    let agent = state.registry.find_agent(&agent_id)?;
    let response = agent.generate(request).await?;
    Ok(Json(response))
}

#[instrument(skip(state))]
async fn list_workflows(
    State(state): State<AppState>,
) -> Json<ListWorkflowsResponse> {
    Json(ListWorkflowsResponse {
        workflows: state.registry.list_workflows(),
    })
}

#[instrument(skip(state, request))]
async fn create_workflow_run(
    State(state): State<AppState>,
    Path(workflow_id): Path<String>,
    Json(request): Json<CreateWorkflowRunRequest>,
) -> ServerResult<(StatusCode, Json<WorkflowRunRecord>)> {
    let run = state.registry.create_workflow_run(&workflow_id, request)?;
    Ok((StatusCode::CREATED, Json(run)))
}

#[instrument(skip(state, request))]
async fn start_workflow_async(
    State(state): State<AppState>,
    Path(workflow_id): Path<String>,
    Json(request): Json<StartWorkflowRunRequest>,
) -> ServerResult<Json<StartWorkflowRunResponse>> {
    let workflow = state.registry.find_workflow(&workflow_id)?;

    // We persist a "running" record before executing the workflow so the API
    // always exposes a run identifier, even if the workflow fails partway
    // through execution.
    let run = state
        .registry
        .begin_workflow_run(&workflow_id, &request)?;
    let run_id = run.run_id;

    match workflow.start(request).await {
        Ok(result) => {
            let run = state
                .registry
                .complete_workflow_run_success(run_id, result)?;
            Ok(Json(StartWorkflowRunResponse { run }))
        }
        Err(error) => {
            let run = state
                .registry
                .complete_workflow_run_failure(run_id, &error)?;
            Err(ServerError::Internal(format!(
                "workflow '{}' failed: {} (run_id={})",
                workflow_id, error, run.run_id
            )))
        }
    }
}

#[instrument(skip(state))]
async fn get_workflow_run(
    State(state): State<AppState>,
    Path((workflow_id, run_id)): Path<(String, String)>,
) -> ServerResult<Json<WorkflowRunRecord>> {
    let parsed_run_id = Uuid::parse_str(&run_id)
        .map_err(|error| ServerError::BadRequest(error.to_string()))?;
    let run = state
        .registry
        .get_workflow_run(&workflow_id, parsed_run_id)?;
    Ok(Json(run))
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use axum::{
        body::{to_bytes, Body},
        http::{Request, StatusCode},
    };
    use serde_json::{json, Value};
    use tower::ServiceExt;

    use crate::{
        contracts::{
            AgentMessages, AgentSummary, FinishReason, GenerateRequest, GenerateResponse,
            StartWorkflowRunRequest, WorkflowRunStatus, WorkflowSummary,
        },
        registry::RuntimeRegistry,
        runtime::{AgentRuntime, WorkflowRuntime},
    };

    use super::MastraServer;

    struct EchoAgent;

    #[async_trait]
    impl AgentRuntime for EchoAgent {
        fn summary(&self) -> AgentSummary {
            AgentSummary {
                id: "echo".to_owned(),
                name: "Echo".to_owned(),
                description: Some("Echoes flattened input".to_owned()),
            }
        }

        async fn generate(
            &self,
            request: GenerateRequest,
        ) -> crate::error::ServerResult<GenerateResponse> {
            Ok(GenerateResponse {
                text: format!("echo: {}", request.messages.flatten_text()),
                finish_reason: FinishReason::Stop,
                usage: None,
            })
        }
    }

    struct JsonWorkflow;

    #[async_trait]
    impl WorkflowRuntime for JsonWorkflow {
        fn summary(&self) -> WorkflowSummary {
            WorkflowSummary {
                id: "demo".to_owned(),
                description: Some("Returns workflow input".to_owned()),
            }
        }

        async fn start(
            &self,
            request: StartWorkflowRunRequest,
        ) -> crate::error::ServerResult<Value> {
            Ok(json!({
                "accepted": true,
                "input": request.input_data,
            }))
        }
    }

    fn build_router() -> axum::Router {
        let registry = RuntimeRegistry::new();
        registry.register_agent(EchoAgent);
        registry.register_workflow(JsonWorkflow);
        MastraServer::new(registry).into_router()
    }

    #[tokio::test]
    async fn lists_registered_agents() {
        let response = build_router()
            .oneshot(
                Request::builder()
                    .uri("/api/agents")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let payload: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(payload["agents"][0]["id"], "echo");
    }

    #[tokio::test]
    async fn generates_agent_responses() {
        let request = serde_json::to_vec(&GenerateRequest {
            messages: AgentMessages::Text("hello".to_owned()),
            resource_id: None,
            thread_id: None,
            run_id: None,
            max_steps: Some(1),
            request_context: Default::default(),
        })
        .unwrap();

        let response = build_router()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/agents/echo/generate")
                    .header("content-type", "application/json")
                    .body(Body::from(request))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let payload: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(payload["text"], "echo: hello");
    }

    #[tokio::test]
    async fn starts_workflows_and_persists_run_records() {
        let response = build_router()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/workflows/demo/start-async")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&json!({
                            "input_data": {"topic": "rust"}
                        }))
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let payload: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(payload["run"]["status"], json!(WorkflowRunStatus::Success));
        assert_eq!(payload["run"]["result"]["accepted"], true);

        let run_id = payload["run"]["run_id"].as_str().unwrap();
        let fetch_response = build_router()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/workflows/demo/runs/{run_id}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(fetch_response.status(), StatusCode::NOT_FOUND);
    }
}
