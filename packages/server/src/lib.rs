mod contracts;
mod error;
mod registry;
mod router;
mod runtime;

use std::{net::SocketAddr, sync::Arc};

use axum::{
    Json, Router,
    extract::{Path, State},
    routing::{get, post},
};
use chrono::Utc;
use contracts::{
    AppendMemoryMessagesRequest, AppendMemoryMessagesResponse, CreateMemoryThreadRequest,
    CreateMemoryThreadResponse, CreateWorkflowRunRequest, GenerateRequest, ListAgentsResponse,
    ListMemoriesResponse, ListMemoryMessagesResponse, ListThreadsResponse, ListWorkflowsResponse,
    MemorySummary, StartWorkflowRunRequest, StartWorkflowRunResponse,
};
use error::{ServerError, ServerResult};
use indexmap::IndexMap;
use mastra_core::{
    Agent, CreateThreadRequest as CoreCreateThreadRequest, MemoryEngine, MemoryMessage,
    MemoryRecallRequest, Workflow,
};
use parking_lot::RwLock;
use registry::RuntimeRegistry;
use runtime::{CoreAgentRuntime, CoreWorkflowRuntime};
use uuid::Uuid;

pub use contracts::{
    AgentMessages, AgentSummary, ChatMessage, ErrorResponse, FinishReason, GenerateResponse,
    RouteDescription, StartWorkflowRunResponse as WorkflowRunResponse, UsageStats,
    WorkflowRunRecord, WorkflowRunStatus, WorkflowSummary,
};
pub use error::ServerError as MastraServerError;
pub use registry::RuntimeRegistry as MastraRuntimeRegistry;
pub use router::{MastraServer, ServerConfig, route_catalog};
pub use runtime::{AgentRuntime, WorkflowRuntime};

#[derive(Clone)]
struct ServerState {
    registry: RuntimeRegistry,
    memory: Arc<RwLock<IndexMap<String, Arc<dyn MemoryEngine>>>>,
}

#[derive(Clone, Default)]
pub struct MastraHttpServer {
    registry: RuntimeRegistry,
    memory: Arc<RwLock<IndexMap<String, Arc<dyn MemoryEngine>>>>,
}

impl MastraHttpServer {
    pub fn new() -> Self {
        Self {
            registry: RuntimeRegistry::new(),
            memory: Arc::new(RwLock::new(IndexMap::new())),
        }
    }

    pub fn registry(&self) -> RuntimeRegistry {
        self.registry.clone()
    }

    pub fn register_agent(&self, agent: Agent) {
        self.registry.register_agent(CoreAgentRuntime::new(agent));
    }

    pub fn register_workflow(&self, workflow: Workflow) {
        self.registry
            .register_workflow(CoreWorkflowRuntime::new(workflow));
    }

    pub fn register_memory(&self, id: impl Into<String>, memory: Arc<dyn MemoryEngine>) {
        self.memory.write().insert(id.into(), memory);
    }

    pub fn router(&self) -> Router {
        let state = ServerState {
            registry: self.registry.clone(),
            memory: Arc::clone(&self.memory),
        };

        Router::new()
            .route("/health", get(health))
            .route("/routes", get(routes))
            .route("/agents", get(list_agents))
            .route("/agents/{agent_id}/generate", post(generate_agent))
            .route("/memories", get(list_memories))
            .route(
                "/memory/{memory_id}/threads",
                get(list_memory_threads).post(create_memory_thread),
            )
            .route(
                "/memory/{memory_id}/threads/{thread_id}/messages",
                get(list_memory_messages).post(append_memory_messages),
            )
            .route("/workflows", get(list_workflows))
            .route("/workflows/{workflow_id}/runs", post(create_workflow_run))
            .route(
                "/workflows/{workflow_id}/runs/{run_id}",
                get(get_workflow_run),
            )
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
                path: "/memories".into(),
                summary: "list registered memories",
            },
            RouteDescription {
                method: "GET",
                path: "/memory/{memory_id}/threads".into(),
                summary: "list memory threads",
            },
            RouteDescription {
                method: "POST",
                path: "/memory/{memory_id}/threads".into(),
                summary: "create a memory thread",
            },
            RouteDescription {
                method: "GET",
                path: "/memory/{memory_id}/threads/{thread_id}/messages".into(),
                summary: "list memory messages",
            },
            RouteDescription {
                method: "POST",
                path: "/memory/{memory_id}/threads/{thread_id}/messages".into(),
                summary: "append memory messages",
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

async fn list_memories(State(state): State<ServerState>) -> Json<ListMemoriesResponse> {
    Json(ListMemoriesResponse {
        memories: state
            .memory
            .read()
            .keys()
            .cloned()
            .map(|id| MemorySummary { id })
            .collect(),
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

async fn create_memory_thread(
    Path(memory_id): Path<String>,
    State(state): State<ServerState>,
    Json(request): Json<CreateMemoryThreadRequest>,
) -> ServerResult<Json<CreateMemoryThreadResponse>> {
    let memory = resolve_memory(&state, &memory_id)?;
    let thread = memory
        .create_thread(CoreCreateThreadRequest {
            id: request.id,
            resource_id: request.resource_id,
            title: request.title,
            metadata: request.metadata,
        })
        .await
        .map_err(ServerError::internal)?;

    Ok(Json(CreateMemoryThreadResponse { thread }))
}

async fn list_memory_threads(
    Path(memory_id): Path<String>,
    State(state): State<ServerState>,
) -> ServerResult<Json<ListThreadsResponse>> {
    let memory = resolve_memory(&state, &memory_id)?;
    let threads = memory
        .list_threads(None)
        .await
        .map_err(ServerError::internal)?;

    Ok(Json(ListThreadsResponse { threads }))
}

async fn append_memory_messages(
    Path((memory_id, thread_id)): Path<(String, String)>,
    State(state): State<ServerState>,
    Json(request): Json<AppendMemoryMessagesRequest>,
) -> ServerResult<Json<AppendMemoryMessagesResponse>> {
    let memory = resolve_memory(&state, &memory_id)?;
    let appended = request.messages.len();
    let messages = request
        .messages
        .into_iter()
        .map(|message| MemoryMessage {
            id: Uuid::now_v7().to_string(),
            thread_id: thread_id.clone(),
            role: message.role.into(),
            content: message.content,
            created_at: Utc::now(),
            metadata: message.metadata,
        })
        .collect();

    memory
        .append_messages(&thread_id, messages)
        .await
        .map_err(ServerError::internal)?;

    Ok(Json(AppendMemoryMessagesResponse {
        thread_id,
        appended,
    }))
}

async fn list_memory_messages(
    Path((memory_id, thread_id)): Path<(String, String)>,
    State(state): State<ServerState>,
) -> ServerResult<Json<ListMemoryMessagesResponse>> {
    let memory = resolve_memory(&state, &memory_id)?;
    let messages = memory
        .list_messages(MemoryRecallRequest {
            thread_id,
            limit: None,
        })
        .await
        .map_err(ServerError::internal)?;

    Ok(Json(ListMemoryMessagesResponse { messages }))
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
            Err(ServerError::internal(
                run.error.unwrap_or_else(|| error.to_string()),
            ))
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

fn resolve_memory(state: &ServerState, memory_id: &str) -> ServerResult<Arc<dyn MemoryEngine>> {
    state
        .memory
        .read()
        .get(memory_id)
        .cloned()
        .ok_or_else(|| ServerError::NotFound {
            resource: "memory",
            id: memory_id.to_owned(),
        })
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, sync::Arc};

    use async_trait::async_trait;
    use axum::{
        body::{Body, to_bytes},
        http::{Request, StatusCode},
    };
    use chrono::Utc;
    use mastra_core::{
        Agent, AgentConfig, CreateThreadRequest, MemoryConfig, MemoryEngine, MemoryMessage,
        MemoryRecallRequest, RequestContext, StaticModel, Step, Thread, Workflow,
    };
    use parking_lot::RwLock;
    use serde_json::{Value, json};
    use tower::ServiceExt;
    use uuid::Uuid;

    use crate::MastraHttpServer;

    #[derive(Default)]
    struct TestMemory {
        threads: RwLock<HashMap<String, Thread>>,
        messages: RwLock<HashMap<String, Vec<MemoryMessage>>>,
    }

    #[async_trait]
    impl MemoryEngine for TestMemory {
        async fn create_thread(&self, request: CreateThreadRequest) -> mastra_core::Result<Thread> {
            let thread = Thread {
                id: request.id.unwrap_or_else(|| Uuid::now_v7().to_string()),
                resource_id: request.resource_id,
                title: request.title,
                created_at: Utc::now(),
                metadata: request.metadata,
            };
            self.threads
                .write()
                .insert(thread.id.clone(), thread.clone());
            self.messages.write().entry(thread.id.clone()).or_default();
            Ok(thread)
        }

        async fn get_thread(&self, thread_id: &str) -> mastra_core::Result<Option<Thread>> {
            Ok(self.threads.read().get(thread_id).cloned())
        }

        async fn list_threads(
            &self,
            resource_id: Option<&str>,
        ) -> mastra_core::Result<Vec<Thread>> {
            Ok(self
                .threads
                .read()
                .values()
                .filter(|thread| {
                    resource_id
                        .map(|resource_id| thread.resource_id.as_deref() == Some(resource_id))
                        .unwrap_or(true)
                })
                .cloned()
                .collect())
        }

        async fn append_messages(
            &self,
            thread_id: &str,
            messages: Vec<MemoryMessage>,
        ) -> mastra_core::Result<()> {
            self.messages
                .write()
                .entry(thread_id.to_string())
                .or_default()
                .extend(messages);
            Ok(())
        }

        async fn list_messages(
            &self,
            request: MemoryRecallRequest,
        ) -> mastra_core::Result<Vec<MemoryMessage>> {
            let mut messages = self
                .messages
                .read()
                .get(&request.thread_id)
                .cloned()
                .unwrap_or_default();
            if let Some(limit) = request.limit {
                let start = messages.len().saturating_sub(limit);
                messages = messages[start..].to_vec();
            }
            Ok(messages)
        }
    }

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
            Workflow::new("workflow-1")
                .then(Step::new("step-1", |input, _| async move { Ok(input) })),
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

    #[tokio::test]
    async fn server_exposes_memory_thread_and_message_routes() {
        let server = MastraHttpServer::new();
        server.register_memory("default", Arc::new(TestMemory::default()));

        let create_thread = server
            .router()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/memory/default/threads")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "resource_id": "resource-1",
                            "title": "Chat thread",
                            "metadata": { "source": "test" },
                        })
                        .to_string(),
                    ))
                    .expect("request should build"),
            )
            .await
            .expect("route should respond");

        assert_eq!(create_thread.status(), StatusCode::OK);
        let create_payload: Value = serde_json::from_slice(
            &to_bytes(create_thread.into_body(), usize::MAX)
                .await
                .expect("thread body should be readable"),
        )
        .expect("thread response should be valid json");
        let thread_id = create_payload["thread"]["id"]
            .as_str()
            .expect("thread id should be present")
            .to_string();

        let append_message = server
            .router()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/memory/default/threads/{thread_id}/messages"))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "messages": [
                                { "role": "user", "content": "hello memory" },
                                { "role": "assistant", "content": "hello back" }
                            ]
                        })
                        .to_string(),
                    ))
                    .expect("request should build"),
            )
            .await
            .expect("route should respond");

        assert_eq!(append_message.status(), StatusCode::OK);

        let list_messages = server
            .router()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/memory/default/threads/{thread_id}/messages"))
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("route should respond");

        assert_eq!(list_messages.status(), StatusCode::OK);
        let list_payload: Value = serde_json::from_slice(
            &to_bytes(list_messages.into_body(), usize::MAX)
                .await
                .expect("messages body should be readable"),
        )
        .expect("messages response should be valid json");

        assert_eq!(list_payload["messages"].as_array().map(Vec::len), Some(2));
        assert_eq!(list_payload["messages"][0]["content"], "hello memory");
        assert_eq!(list_payload["messages"][1]["content"], "hello back");
    }
}
