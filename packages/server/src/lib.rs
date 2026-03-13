mod contracts;
mod error;
mod registry;
mod router;
mod runtime;

use std::{net::SocketAddr, sync::Arc};

use axum::Router;
use mastra_core::{Agent, MemoryEngine, Tool, Workflow};
use registry::RuntimeRegistry;
use runtime::{CoreAgentRuntime, CoreWorkflowRuntime};

pub use contracts::{
    AgentDetail, AgentDetailResponse, AgentMessages, AgentSummary, AppendObservationInput,
    CancelWorkflowRunResponse, ChatMessage, DeleteWorkflowRunResponse, ErrorResponse,
    ExecuteToolRequest, ExecuteToolResponse, FinishReason, GenerateMemoryConfig,
    GenerateMemoryOptions, GenerateMemoryThreadObject, GenerateMemoryThreadRef, GenerateResponse,
    GenerateStreamEvent, GenerateStreamFinishEvent, GenerateStreamStartEvent,
    GenerateStreamTextDeltaEvent, GenerateStreamToolCallEvent, GenerateStreamToolResultEvent,
    GetMemoryThreadResponse, GetWorkingMemoryResponse, ListObservationsQuery,
    ListObservationsResponse, ListToolsResponse, ListWorkflowRunsQuery, ListWorkflowRunsResponse,
    ResumeWorkflowRunRequest, ResumeWorkflowRunResponse,
    RouteDescription, StartWorkflowRunResponse as WorkflowRunResponse, SystemPackage,
    SystemPackagesResponse, ToolChoice, ToolChoiceMode, ToolSummary, UpdateMemoryThreadRequest,
    UpdateWorkingMemoryInput, UsageStats, WorkflowDetail, WorkflowDetailResponse,
    WorkflowRunRecord, WorkflowRunStatus, WorkflowStepSummary, WorkflowStreamEvent,
    WorkflowStreamFinishEvent, WorkflowStreamStartEvent, WorkflowStreamStepEvent, WorkflowSummary,
};
pub use error::ServerError as MastraServerError;
pub use registry::RuntimeRegistry as MastraRuntimeRegistry;
pub use router::{MastraServer, ServerConfig, route_catalog};
pub use runtime::{AgentRuntime, WorkflowRuntime};

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
        self.registry
            .register_workflow(CoreWorkflowRuntime::new(workflow));
    }

    pub fn register_tool(&self, tool: Tool) {
        self.registry.register_tool(tool);
    }

    pub fn register_memory(&self, id: impl Into<String>, memory: Arc<dyn MemoryEngine>) {
        self.registry.register_memory(id, memory);
    }

    pub fn router(&self) -> Router {
        MastraServer::new(self.registry.clone()).into_router()
    }

    pub fn route_descriptions() -> Vec<RouteDescription> {
        route_catalog("/api")
    }

    pub async fn serve(self, address: SocketAddr) -> std::io::Result<()> {
        let listener = tokio::net::TcpListener::bind(address).await?;
        axum::serve(listener, self.router()).await
    }
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
            let now = Utc::now();
            let thread = Thread {
                id: request.id.unwrap_or_else(|| Uuid::now_v7().to_string()),
                resource_id: request.resource_id,
                title: request.title,
                created_at: now,
                updated_at: now,
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
                        .map(|value| thread.resource_id.as_deref() == Some(value))
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
    async fn compatibility_wrapper_registers_core_primitives() {
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
    }

    #[tokio::test]
    async fn compatibility_wrapper_uses_prefixed_memory_routes() {
        let server = MastraHttpServer::new();
        server.register_memory("default", Arc::new(TestMemory::default()));

        let create_thread = server
            .router()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/memory/default/threads")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "resource_id": "resource-1",
                            "title": "Chat thread",
                            "metadata": { "source": "test" },
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(create_thread.status(), StatusCode::OK);
        let payload: Value = serde_json::from_slice(
            &to_bytes(create_thread.into_body(), usize::MAX)
                .await
                .unwrap(),
        )
        .unwrap();
        let thread_id = payload["thread"]["id"].as_str().unwrap().to_owned();

        let messages = server
            .router()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/memory/default/threads/{thread_id}/messages"))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "messages": [
                                {
                                    "role": "user",
                                    "content": "hello",
                                    "metadata": {"kind": "test"},
                                }
                            ]
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(messages.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn compatibility_wrapper_exposes_unified_route_catalog() {
        let response = MastraHttpServer::new()
            .router()
            .oneshot(
                Request::builder()
                    .uri("/api/routes")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let payload: Value = serde_json::from_slice(&body).unwrap();
        assert!(
            payload
                .as_array()
                .unwrap()
                .iter()
                .any(|route| route["path"] == "/api/agents/{agent_id}/stream")
        );
    }

    #[test]
    fn compatibility_wrapper_route_descriptions_follow_api_prefix() {
        assert!(
            MastraHttpServer::route_descriptions()
                .iter()
                .any(|route| route.path == "/api/health")
        );
    }

    #[test]
    fn registry_remains_usable_for_direct_run_creation() {
        let server = MastraHttpServer::new();
        server.register_workflow(
            Workflow::new("workflow-1")
                .then(Step::new("step-1", |input, _| async move { Ok(input) })),
        );
        let run = server
            .registry()
            .create_workflow_run(
                "workflow-1",
                crate::contracts::CreateWorkflowRunRequest {
                    run_id: None,
                    resource_id: None,
                    input_data: Some(json!({"hello": "world"})),
                    request_context: RequestContext::new().values().clone(),
                },
            )
            .unwrap();

        assert_eq!(run.workflow_id, "workflow-1");
    }
}
