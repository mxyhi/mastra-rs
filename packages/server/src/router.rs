use std::{convert::Infallible, fs, net::SocketAddr, sync::Arc, time::Duration};

use async_stream::stream;
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::sse::{Event, KeepAlive, Sse},
    routing::{get, post},
};
use chrono::Utc;
use futures::StreamExt;
use mastra_core::{
    Agent, AppendObservationRequest as CoreAppendObservationRequest, CloneThreadRequest,
    CreateThreadRequest, MemoryEngine, MemoryMessage, MemoryMessageOrder,
    MemoryMessageOrderField, MemoryOrderDirection, MemoryRecallRequest, MemoryScope,
    MemoryThreadOrder, MemoryThreadOrderField, ObservationQuery, Tool,
    UpdateThreadRequest, UpdateWorkingMemoryRequest as CoreUpdateWorkingMemoryRequest, Workflow,
    WorkingMemoryFormat,
};
use tracing::instrument;
use uuid::Uuid;

use crate::{
    contracts::{
        AgentDetailResponse, AppendMemoryMessagesRequest, AppendMemoryMessagesResponse,
        AppendObservationInput, CancelWorkflowRunResponse, CloneMemoryThreadRequest,
        CloneMemoryThreadResponse, CreateMemoryThreadRequest, CreateMemoryThreadResponse,
        CreateWorkflowRunRequest, DeleteMemoryMessagesRequest, DeleteMemoryMessagesResponse,
        DeleteWorkflowRunResponse, ErrorResponse, ExecuteToolRequest, ExecuteToolResponse,
        GenerateRequest, GenerateStreamEvent, GetMemoryThreadResponse,
        GetWorkingMemoryResponse, ListAgentsResponse, ListMemoriesResponse,
        ListMemoryMessagesResponse, ListMessagesQuery, ListObservationsQuery,
        ListObservationsResponse, ListThreadsQuery, ListThreadsResponse, ListToolsResponse,
        ListWorkflowRunsQuery, ListWorkflowRunsResponse, ListWorkflowsResponse, MessageOrderBy,
        MessageOrderField, OrderDirection, ResumeWorkflowRunRequest,
        ResumeWorkflowRunResponse, RouteDescription, StartWorkflowRunRequest,
        StartWorkflowRunResponse, SystemPackage, SystemPackagesResponse, ThreadOrderBy,
        ThreadOrderField, UpdateMemoryThreadRequest, UpdateWorkingMemoryInput,
        WorkflowDetailResponse, WorkflowRunRecord, WorkflowStreamEvent,
        WorkflowStreamFinishEvent, WorkflowStreamQuery, WorkflowStreamStartEvent,
        WorkflowStreamStepEvent,
    },
    error::{ServerError, ServerResult},
    registry::RuntimeRegistry,
    runtime::{CoreAgentRuntime, CoreWorkflowRuntime},
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
    api_prefix: String,
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

    pub fn register_memory(&self, memory_id: impl Into<String>, memory: Arc<dyn MemoryEngine>) {
        self.registry.register_memory(memory_id, memory);
    }

    pub fn registry(&self) -> &RuntimeRegistry {
        &self.registry
    }

    pub fn into_router(self) -> Router {
        let api_router = Router::new()
            .route("/health", get(health))
            .route("/routes", get(routes))
            .route("/system/packages", get(system_packages))
            .route("/agents", get(list_agents))
            .route("/agents/{agent_id}", get(get_agent))
            .route("/agents/{agent_id}/generate", post(generate_agent))
            .route("/agents/{agent_id}/stream", post(stream_agent))
            .route("/agents/{agent_id}/tools", get(list_agent_tools))
            .route(
                "/agents/{agent_id}/tools/{tool_id}/execute",
                post(execute_agent_tool),
            )
            .route("/memories", get(list_memories))
            .route("/tools", get(list_tools))
            .route("/tools/{tool_id}", get(get_tool))
            .route("/tools/{tool_id}/execute", post(execute_tool))
            .route(
                "/memory/threads",
                get(list_default_memory_threads).post(create_default_memory_thread),
            )
            .route(
                "/memory/threads/{thread_id}",
                get(get_default_memory_thread)
                    .patch(update_default_memory_thread)
                    .delete(delete_default_memory_thread),
            )
            .route(
                "/memory/threads/{thread_id}/working-memory",
                get(get_default_working_memory).put(update_default_working_memory),
            )
            .route(
                "/memory/threads/{thread_id}/observations",
                get(list_default_memory_observations).post(append_default_memory_observation),
            )
            .route(
                "/memory/threads/{thread_id}/clone",
                post(clone_default_memory_thread),
            )
            .route(
                "/memory/threads/{thread_id}/messages",
                post(append_default_memory_messages).get(list_default_memory_messages),
            )
            .route(
                "/memory/messages/delete",
                post(delete_default_memory_messages),
            )
            .route(
                "/memory/{memory_id}/threads",
                get(list_memory_threads).post(create_memory_thread),
            )
            .route(
                "/memory/{memory_id}/threads/{thread_id}",
                get(get_memory_thread)
                    .patch(update_memory_thread)
                    .delete(delete_memory_thread),
            )
            .route(
                "/memory/{memory_id}/threads/{thread_id}/working-memory",
                get(get_memory_working_memory).put(update_memory_working_memory),
            )
            .route(
                "/memory/{memory_id}/threads/{thread_id}/observations",
                get(list_memory_observations).post(append_memory_observation),
            )
            .route(
                "/memory/{memory_id}/threads/{thread_id}/clone",
                post(clone_memory_thread),
            )
            .route(
                "/memory/{memory_id}/threads/{thread_id}/messages",
                post(append_memory_messages).get(list_memory_messages),
            )
            .route(
                "/memory/{memory_id}/messages/delete",
                post(delete_memory_messages),
            )
            .route("/workflows", get(list_workflows))
            .route(
                "/workflows/{workflow_id}/create-run",
                post(create_workflow_run),
            )
            .route("/workflows/{workflow_id}", get(get_workflow))
            .route(
                "/workflows/{workflow_id}/start-async",
                post(start_workflow_async),
            )
            .route("/workflows/{workflow_id}/resume", post(resume_workflow))
            .route(
                "/workflows/{workflow_id}/resume-async",
                post(resume_workflow_async),
            )
            .route(
                "/workflows/{workflow_id}/resume-stream",
                post(stream_resumed_workflow),
            )
            .route("/workflows/{workflow_id}/stream", post(stream_workflow))
            .route("/workflows/{workflow_id}/runs", get(list_workflow_runs))
            .route(
                "/workflows/{workflow_id}/runs/{run_id}",
                get(get_workflow_run).delete(delete_workflow_run),
            )
            .route(
                "/workflows/{workflow_id}/runs/{run_id}/cancel",
                post(cancel_workflow_run),
            )
            .with_state(AppState {
                registry: self.registry.clone(),
                api_prefix: normalize_prefix(&self.config.api_prefix),
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

    pub async fn serve(self, address: SocketAddr) -> std::io::Result<()> {
        let listener = tokio::net::TcpListener::bind(address).await?;
        axum::serve(listener, self.into_router()).await
    }
}

pub fn route_catalog(prefix: &str) -> Vec<RouteDescription> {
    let prefix = normalize_prefix(prefix);
    [
        ("GET", "/health", "Health check"),
        ("GET", "/routes", "List registered routes"),
        ("GET", "/system/packages", "List installed system packages"),
        ("GET", "/agents", "List registered agents"),
        ("GET", "/agents/{agent_id}", "Get a registered agent"),
        (
            "POST",
            "/agents/{agent_id}/generate",
            "Generate an agent response",
        ),
        (
            "POST",
            "/agents/{agent_id}/stream",
            "Stream an agent response",
        ),
        (
            "GET",
            "/agents/{agent_id}/tools",
            "List the tools registered on an agent",
        ),
        (
            "POST",
            "/agents/{agent_id}/tools/{tool_id}/execute",
            "Execute a tool from an agent",
        ),
        ("GET", "/memories", "List registered memories"),
        ("GET", "/tools", "List registered tools"),
        ("GET", "/tools/{tool_id}", "Get a registered tool"),
        (
            "POST",
            "/tools/{tool_id}/execute",
            "Execute a registered tool",
        ),
        ("GET", "/memory/threads", "List memory threads"),
        ("POST", "/memory/threads", "Create a memory thread"),
        ("GET", "/memory/threads/{thread_id}", "Get a memory thread"),
        (
            "PATCH",
            "/memory/threads/{thread_id}",
            "Update a memory thread",
        ),
        (
            "DELETE",
            "/memory/threads/{thread_id}",
            "Delete a memory thread",
        ),
        (
            "GET",
            "/memory/threads/{thread_id}/working-memory",
            "Get working memory for a thread",
        ),
        (
            "PUT",
            "/memory/threads/{thread_id}/working-memory",
            "Update working memory for a thread",
        ),
        (
            "GET",
            "/memory/threads/{thread_id}/observations",
            "List observations for a thread",
        ),
        (
            "POST",
            "/memory/threads/{thread_id}/observations",
            "Append an observation to a thread",
        ),
        (
            "POST",
            "/memory/threads/{thread_id}/clone",
            "Clone a memory thread",
        ),
        (
            "GET",
            "/memory/threads/{thread_id}/messages",
            "List messages for a memory thread",
        ),
        (
            "POST",
            "/memory/threads/{thread_id}/messages",
            "Append messages to a memory thread",
        ),
        (
            "POST",
            "/memory/messages/delete",
            "Delete messages from memory",
        ),
        ("GET", "/memory/{memory_id}/threads", "List memory threads"),
        (
            "POST",
            "/memory/{memory_id}/threads",
            "Create a memory thread",
        ),
        (
            "GET",
            "/memory/{memory_id}/threads/{thread_id}",
            "Get a memory thread",
        ),
        (
            "PATCH",
            "/memory/{memory_id}/threads/{thread_id}",
            "Update a memory thread",
        ),
        (
            "DELETE",
            "/memory/{memory_id}/threads/{thread_id}",
            "Delete a memory thread",
        ),
        (
            "GET",
            "/memory/{memory_id}/threads/{thread_id}/working-memory",
            "Get working memory for a thread",
        ),
        (
            "PUT",
            "/memory/{memory_id}/threads/{thread_id}/working-memory",
            "Update working memory for a thread",
        ),
        (
            "GET",
            "/memory/{memory_id}/threads/{thread_id}/observations",
            "List observations for a thread",
        ),
        (
            "POST",
            "/memory/{memory_id}/threads/{thread_id}/observations",
            "Append an observation to a thread",
        ),
        (
            "POST",
            "/memory/{memory_id}/threads/{thread_id}/clone",
            "Clone a memory thread",
        ),
        (
            "GET",
            "/memory/{memory_id}/threads/{thread_id}/messages",
            "List messages for a memory thread",
        ),
        (
            "POST",
            "/memory/{memory_id}/threads/{thread_id}/messages",
            "Append messages to a memory thread",
        ),
        (
            "POST",
            "/memory/{memory_id}/messages/delete",
            "Delete messages from memory",
        ),
        ("GET", "/workflows", "List registered workflows"),
        (
            "GET",
            "/workflows/{workflow_id}",
            "Get a registered workflow",
        ),
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
            "POST",
            "/workflows/{workflow_id}/resume",
            "Resume a workflow run and wait for completion",
        ),
        (
            "POST",
            "/workflows/{workflow_id}/resume-async",
            "Resume a workflow run asynchronously",
        ),
        (
            "POST",
            "/workflows/{workflow_id}/resume-stream",
            "Resume a workflow run as an event stream",
        ),
        (
            "POST",
            "/workflows/{workflow_id}/stream",
            "Stream workflow execution events",
        ),
        (
            "GET",
            "/workflows/{workflow_id}/runs",
            "List workflow run records",
        ),
        (
            "GET",
            "/workflows/{workflow_id}/runs/{run_id}",
            "Fetch a workflow run record",
        ),
        (
            "DELETE",
            "/workflows/{workflow_id}/runs/{run_id}",
            "Delete a workflow run record",
        ),
        (
            "POST",
            "/workflows/{workflow_id}/runs/{run_id}/cancel",
            "Cancel a workflow run record",
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

async fn health() -> &'static str {
    "ok"
}

async fn routes(State(state): State<AppState>) -> Json<Vec<RouteDescription>> {
    Json(route_catalog(&state.api_prefix))
}

#[instrument]
async fn system_packages() -> Json<SystemPackagesResponse> {
    Json(SystemPackagesResponse {
        packages: load_system_packages(),
        is_dev: std::env::var("MASTRA_DEV").is_ok_and(|value| value == "true"),
        cms_enabled: std::env::var("MASTRA_CMS_ENABLED").is_ok_and(|value| value == "true"),
    })
}

#[instrument(skip(state))]
async fn list_agents(State(state): State<AppState>) -> Json<ListAgentsResponse> {
    Json(ListAgentsResponse {
        agents: state.registry.list_agents(),
    })
}

#[instrument(skip(state))]
async fn get_agent(
    State(state): State<AppState>,
    Path(agent_id): Path<String>,
) -> ServerResult<Json<AgentDetailResponse>> {
    let agent = state.registry.find_agent(&agent_id)?;
    Ok(Json(AgentDetailResponse {
        agent: agent.detail(),
    }))
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

#[instrument(skip(state, request))]
async fn stream_agent(
    State(state): State<AppState>,
    Path(agent_id): Path<String>,
    Json(request): Json<GenerateRequest>,
) -> ServerResult<Sse<impl futures::Stream<Item = Result<Event, Infallible>>>> {
    let agent = state.registry.find_agent(&agent_id)?;
    let stream = agent.stream(request).map(|result| {
        let event = match result {
            Ok(event) => encode_stream_event(event),
            Err(error) => encode_stream_event(GenerateStreamEvent::Error(ErrorResponse {
                error: error.to_string(),
            })),
        };
        Ok::<_, Infallible>(event)
    });

    Ok(Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15))))
}

#[instrument(skip(state))]
async fn list_agent_tools(
    State(state): State<AppState>,
    Path(agent_id): Path<String>,
) -> ServerResult<Json<ListToolsResponse>> {
    let agent = state.registry.find_agent(&agent_id)?;
    Ok(Json(ListToolsResponse {
        tools: agent.tool_summaries(),
    }))
}

#[instrument(skip(state, request))]
async fn execute_agent_tool(
    State(state): State<AppState>,
    Path((agent_id, tool_id)): Path<(String, String)>,
    Json(request): Json<ExecuteToolRequest>,
) -> ServerResult<Json<ExecuteToolResponse>> {
    let agent = state.registry.find_agent(&agent_id)?;
    let response = agent.execute_tool(&tool_id, request).await?;
    Ok(Json(response))
}

#[instrument(skip(state))]
async fn list_workflows(State(state): State<AppState>) -> Json<ListWorkflowsResponse> {
    Json(ListWorkflowsResponse {
        workflows: state.registry.list_workflows(),
    })
}

#[instrument(skip(state))]
async fn get_workflow(
    State(state): State<AppState>,
    Path(workflow_id): Path<String>,
) -> ServerResult<Json<WorkflowDetailResponse>> {
    let workflow = state.registry.find_workflow(&workflow_id)?;
    Ok(Json(WorkflowDetailResponse {
        workflow: workflow.detail(),
    }))
}

#[instrument(skip(state))]
async fn list_memories(State(state): State<AppState>) -> Json<ListMemoriesResponse> {
    Json(ListMemoriesResponse {
        memories: state.registry.list_memory(),
    })
}

#[instrument(skip(state))]
async fn list_tools(State(state): State<AppState>) -> Json<ListToolsResponse> {
    Json(ListToolsResponse {
        tools: state.registry.list_tools(),
    })
}

#[instrument(skip(state))]
async fn get_tool(
    State(state): State<AppState>,
    Path(tool_id): Path<String>,
) -> ServerResult<Json<crate::contracts::ToolSummary>> {
    Ok(Json(state.registry.get_tool_summary(&tool_id)?))
}

#[instrument(skip(state, request))]
async fn execute_tool(
    State(state): State<AppState>,
    Path(tool_id): Path<String>,
    Json(request): Json<ExecuteToolRequest>,
) -> ServerResult<Json<ExecuteToolResponse>> {
    let tool = state.registry.find_tool(&tool_id)?;
    let output = tool
        .execute(
            request.data,
            mastra_core::ToolExecutionContext {
                request_context: mastra_core::RequestContext::from_value_map(
                    request.request_context,
                ),
                run_id: request.run_id,
                thread_id: request.thread_id,
                approved: request.approved,
            },
        )
        .await
        .map_err(ServerError::internal)?;

    Ok(Json(ExecuteToolResponse { tool_id, output }))
}

#[instrument(skip(state))]
async fn list_default_memory_threads(
    State(state): State<AppState>,
    Query(query): Query<ListThreadsQuery>,
) -> ServerResult<Json<ListThreadsResponse>> {
    let memory = state.registry.find_default_memory()?;
    list_memory_threads_for(memory, query).await
}

#[instrument(skip(state))]
async fn list_memory_threads(
    State(state): State<AppState>,
    Path(memory_id): Path<String>,
    Query(query): Query<ListThreadsQuery>,
) -> ServerResult<Json<ListThreadsResponse>> {
    let memory = state.registry.find_memory(&memory_id)?;
    list_memory_threads_for(memory, query).await
}

async fn list_memory_threads_for(
    memory: Arc<dyn MemoryEngine>,
    query: ListThreadsQuery,
) -> ServerResult<Json<ListThreadsResponse>> {
    let metadata = query
        .parsed_metadata()
        .map_err(|error| ServerError::BadRequest(format!("invalid metadata filter: {error}")))?;
    let per_page = query.parsed_per_page().map_err(ServerError::BadRequest)?;
    let order_by = query
        .parsed_order_by()
        .map_err(|error| ServerError::BadRequest(format!("invalid orderBy: {error}")))?;
    let threads = memory
        .list_threads_page(mastra_core::MemoryThreadQuery {
            resource_id: query.resource_id.clone(),
            metadata,
            page: query.page,
            per_page,
            order_by: order_by.map(thread_order_to_core),
        })
        .await
        .map_err(ServerError::internal)?;

    Ok(Json(ListThreadsResponse {
        threads: threads.threads,
        total: threads.total,
        page: threads.page,
        per_page: query.response_per_page(threads.per_page),
        has_more: threads.has_more,
    }))
}

#[instrument(skip(state, request))]
async fn create_default_memory_thread(
    State(state): State<AppState>,
    Json(request): Json<CreateMemoryThreadRequest>,
) -> ServerResult<Json<CreateMemoryThreadResponse>> {
    let memory = state.registry.find_default_memory()?;
    create_memory_thread_for(memory, request).await
}

#[instrument(skip(state, request))]
async fn create_memory_thread(
    State(state): State<AppState>,
    Path(memory_id): Path<String>,
    Json(request): Json<CreateMemoryThreadRequest>,
) -> ServerResult<Json<CreateMemoryThreadResponse>> {
    let memory = state.registry.find_memory(&memory_id)?;
    create_memory_thread_for(memory, request).await
}

async fn create_memory_thread_for(
    memory: Arc<dyn MemoryEngine>,
    request: CreateMemoryThreadRequest,
) -> ServerResult<Json<CreateMemoryThreadResponse>> {
    let thread = memory
        .create_thread(CreateThreadRequest {
            id: request.id,
            resource_id: request.resource_id,
            title: request.title,
            metadata: request.metadata,
        })
        .await
        .map_err(ServerError::internal)?;

    Ok(Json(CreateMemoryThreadResponse { thread }))
}

#[instrument(skip(state))]
async fn get_default_memory_thread(
    State(state): State<AppState>,
    Path(thread_id): Path<String>,
) -> ServerResult<Json<GetMemoryThreadResponse>> {
    let memory = state.registry.find_default_memory()?;
    get_memory_thread_for(memory, thread_id).await
}

#[instrument(skip(state))]
async fn get_memory_thread(
    State(state): State<AppState>,
    Path((memory_id, thread_id)): Path<(String, String)>,
) -> ServerResult<Json<GetMemoryThreadResponse>> {
    let memory = state.registry.find_memory(&memory_id)?;
    get_memory_thread_for(memory, thread_id).await
}

async fn get_memory_thread_for(
    memory: Arc<dyn MemoryEngine>,
    thread_id: String,
) -> ServerResult<Json<GetMemoryThreadResponse>> {
    let thread = memory
        .get_thread(&thread_id)
        .await
        .map_err(ServerError::internal)?
        .ok_or_else(|| ServerError::NotFound {
            resource: "memory thread",
            id: thread_id,
        })?;

    Ok(Json(GetMemoryThreadResponse { thread }))
}

#[instrument(skip(state, request))]
async fn update_default_memory_thread(
    State(state): State<AppState>,
    Path(thread_id): Path<String>,
    Json(request): Json<UpdateMemoryThreadRequest>,
) -> ServerResult<Json<GetMemoryThreadResponse>> {
    let memory = state.registry.find_default_memory()?;
    update_memory_thread_for(memory, thread_id, request).await
}

#[instrument(skip(state, request))]
async fn update_memory_thread(
    State(state): State<AppState>,
    Path((memory_id, thread_id)): Path<(String, String)>,
    Json(request): Json<UpdateMemoryThreadRequest>,
) -> ServerResult<Json<GetMemoryThreadResponse>> {
    let memory = state.registry.find_memory(&memory_id)?;
    update_memory_thread_for(memory, thread_id, request).await
}

async fn update_memory_thread_for(
    memory: Arc<dyn MemoryEngine>,
    thread_id: String,
    request: UpdateMemoryThreadRequest,
) -> ServerResult<Json<GetMemoryThreadResponse>> {
    let thread = memory
        .update_thread(
            &thread_id,
            UpdateThreadRequest {
                resource_id: request.resource_id,
                title: request.title,
                metadata: request.metadata,
            },
        )
        .await
        .map_err(ServerError::internal)?;

    Ok(Json(GetMemoryThreadResponse { thread }))
}

#[instrument(skip(state))]
async fn get_default_working_memory(
    State(state): State<AppState>,
    Path(thread_id): Path<String>,
) -> ServerResult<Json<GetWorkingMemoryResponse>> {
    let memory = state.registry.find_default_memory()?;
    get_working_memory_for(memory, thread_id).await
}

#[instrument(skip(state))]
async fn get_memory_working_memory(
    State(state): State<AppState>,
    Path((memory_id, thread_id)): Path<(String, String)>,
) -> ServerResult<Json<GetWorkingMemoryResponse>> {
    let memory = state.registry.find_memory(&memory_id)?;
    get_working_memory_for(memory, thread_id).await
}

async fn get_working_memory_for(
    memory: Arc<dyn MemoryEngine>,
    thread_id: String,
) -> ServerResult<Json<GetWorkingMemoryResponse>> {
    let working_memory = memory
        .get_working_memory(&thread_id, None)
        .await
        .map_err(ServerError::internal)?;

    Ok(Json(GetWorkingMemoryResponse { working_memory }))
}

#[instrument(skip(state, request))]
async fn update_default_working_memory(
    State(state): State<AppState>,
    Path(thread_id): Path<String>,
    Json(request): Json<UpdateWorkingMemoryInput>,
) -> ServerResult<Json<GetWorkingMemoryResponse>> {
    let memory = state.registry.find_default_memory()?;
    update_working_memory_for(memory, thread_id, request).await
}

#[instrument(skip(state, request))]
async fn update_memory_working_memory(
    State(state): State<AppState>,
    Path((memory_id, thread_id)): Path<(String, String)>,
    Json(request): Json<UpdateWorkingMemoryInput>,
) -> ServerResult<Json<GetWorkingMemoryResponse>> {
    let memory = state.registry.find_memory(&memory_id)?;
    update_working_memory_for(memory, thread_id, request).await
}

async fn update_working_memory_for(
    memory: Arc<dyn MemoryEngine>,
    thread_id: String,
    request: UpdateWorkingMemoryInput,
) -> ServerResult<Json<GetWorkingMemoryResponse>> {
    let working_memory = memory
        .update_working_memory(CoreUpdateWorkingMemoryRequest {
            thread_id,
            resource_id: request.resource_id,
            scope: request.scope.unwrap_or(MemoryScope::Thread),
            format: request.format.unwrap_or_else(|| {
                if request.content.is_string() {
                    WorkingMemoryFormat::Markdown
                } else {
                    WorkingMemoryFormat::Json
                }
            }),
            template: request.template,
            content: request.content,
        })
        .await
        .map_err(ServerError::internal)?;

    Ok(Json(GetWorkingMemoryResponse {
        working_memory: Some(working_memory),
    }))
}

#[instrument(skip(state))]
async fn list_default_memory_observations(
    State(state): State<AppState>,
    Path(thread_id): Path<String>,
    Query(query): Query<ListObservationsQuery>,
) -> ServerResult<Json<ListObservationsResponse>> {
    let memory = state.registry.find_default_memory()?;
    list_memory_observations_for(memory, thread_id, query).await
}

#[instrument(skip(state))]
async fn list_memory_observations(
    State(state): State<AppState>,
    Path((memory_id, thread_id)): Path<(String, String)>,
    Query(query): Query<ListObservationsQuery>,
) -> ServerResult<Json<ListObservationsResponse>> {
    let memory = state.registry.find_memory(&memory_id)?;
    list_memory_observations_for(memory, thread_id, query).await
}

async fn list_memory_observations_for(
    memory: Arc<dyn MemoryEngine>,
    thread_id: String,
    query: ListObservationsQuery,
) -> ServerResult<Json<ListObservationsResponse>> {
    let per_page = query.parsed_per_page().map_err(ServerError::BadRequest)?;
    let observations = memory
        .list_observations(ObservationQuery {
            thread_id,
            resource_id: query.resource_id.clone(),
            scope: query.scope,
            page: query.page,
            per_page,
        })
        .await
        .map_err(ServerError::internal)?;

    Ok(Json(ListObservationsResponse {
        observations: observations.observations,
        total: observations.total,
        page: observations.page,
        per_page: query.response_per_page(observations.per_page),
        has_more: observations.has_more,
    }))
}

#[instrument(skip(state, request))]
async fn append_default_memory_observation(
    State(state): State<AppState>,
    Path(thread_id): Path<String>,
    Json(request): Json<AppendObservationInput>,
) -> ServerResult<Json<mastra_core::ObservationRecord>> {
    let memory = state.registry.find_default_memory()?;
    append_memory_observation_for(memory, thread_id, request).await
}

#[instrument(skip(state, request))]
async fn append_memory_observation(
    State(state): State<AppState>,
    Path((memory_id, thread_id)): Path<(String, String)>,
    Json(request): Json<AppendObservationInput>,
) -> ServerResult<Json<mastra_core::ObservationRecord>> {
    let memory = state.registry.find_memory(&memory_id)?;
    append_memory_observation_for(memory, thread_id, request).await
}

async fn append_memory_observation_for(
    memory: Arc<dyn MemoryEngine>,
    thread_id: String,
    request: AppendObservationInput,
) -> ServerResult<Json<mastra_core::ObservationRecord>> {
    let observation = memory
        .append_observation(CoreAppendObservationRequest {
            thread_id,
            resource_id: request.resource_id,
            scope: request.scope.unwrap_or(MemoryScope::Thread),
            content: request.content,
            observed_message_ids: request.observed_message_ids,
            metadata: request.metadata,
        })
        .await
        .map_err(ServerError::internal)?;

    Ok(Json(observation))
}

#[instrument(skip(state))]
async fn delete_default_memory_thread(
    State(state): State<AppState>,
    Path(thread_id): Path<String>,
) -> ServerResult<StatusCode> {
    let memory = state.registry.find_default_memory()?;
    delete_memory_thread_for(memory, thread_id).await
}

#[instrument(skip(state))]
async fn delete_memory_thread(
    State(state): State<AppState>,
    Path((memory_id, thread_id)): Path<(String, String)>,
) -> ServerResult<StatusCode> {
    let memory = state.registry.find_memory(&memory_id)?;
    delete_memory_thread_for(memory, thread_id).await
}

async fn delete_memory_thread_for(
    memory: Arc<dyn MemoryEngine>,
    thread_id: String,
) -> ServerResult<StatusCode> {
    memory
        .delete_thread(&thread_id)
        .await
        .map_err(ServerError::internal)?;
    Ok(StatusCode::NO_CONTENT)
}

#[instrument(skip(state, request))]
async fn clone_default_memory_thread(
    State(state): State<AppState>,
    Path(thread_id): Path<String>,
    Json(request): Json<CloneMemoryThreadRequest>,
) -> ServerResult<Json<CloneMemoryThreadResponse>> {
    let memory = state.registry.find_default_memory()?;
    clone_memory_thread_for(memory, thread_id, request).await
}

#[instrument(skip(state, request))]
async fn clone_memory_thread(
    State(state): State<AppState>,
    Path((memory_id, thread_id)): Path<(String, String)>,
    Json(request): Json<CloneMemoryThreadRequest>,
) -> ServerResult<Json<CloneMemoryThreadResponse>> {
    let memory = state.registry.find_memory(&memory_id)?;
    clone_memory_thread_for(memory, thread_id, request).await
}

async fn clone_memory_thread_for(
    memory: Arc<dyn MemoryEngine>,
    source_thread_id: String,
    request: CloneMemoryThreadRequest,
) -> ServerResult<Json<CloneMemoryThreadResponse>> {
    let message_limit = request.requested_message_limit();
    let message_ids = request.requested_message_ids();
    let start_date = request.requested_start_date();
    let end_date = request.requested_end_date();
    let thread = memory
        .clone_thread(CloneThreadRequest {
            source_thread_id,
            new_thread_id: request.new_thread_id,
            resource_id: request.resource_id,
            title: request.title,
            metadata: request.metadata,
            message_limit,
            message_ids,
            start_date,
            end_date,
        })
        .await
        .map_err(ServerError::internal)?;

    let cloned_messages = memory
        .list_messages(MemoryRecallRequest {
            thread_id: thread.id.clone(),
            limit: None,
            resource_id: None,
            page: None,
            per_page: None,
            message_ids: None,
            start_date: None,
            end_date: None,
            order_by: None,
        })
        .await
        .map_err(ServerError::internal)?;

    Ok(Json(CloneMemoryThreadResponse {
        thread,
        cloned_messages,
    }))
}

#[instrument(skip(state, request))]
async fn append_default_memory_messages(
    State(state): State<AppState>,
    Path(thread_id): Path<String>,
    Json(request): Json<AppendMemoryMessagesRequest>,
) -> ServerResult<Json<AppendMemoryMessagesResponse>> {
    let memory = state.registry.find_default_memory()?;
    append_memory_messages_for(memory, thread_id, request).await
}

#[instrument(skip(state, request))]
async fn append_memory_messages(
    State(state): State<AppState>,
    Path((memory_id, thread_id)): Path<(String, String)>,
    Json(request): Json<AppendMemoryMessagesRequest>,
) -> ServerResult<Json<AppendMemoryMessagesResponse>> {
    let memory = state.registry.find_memory(&memory_id)?;
    append_memory_messages_for(memory, thread_id, request).await
}

async fn append_memory_messages_for(
    memory: Arc<dyn MemoryEngine>,
    thread_id: String,
    request: AppendMemoryMessagesRequest,
) -> ServerResult<Json<AppendMemoryMessagesResponse>> {
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
        .collect::<Vec<_>>();
    let appended = messages.len();

    memory
        .append_messages(&thread_id, messages)
        .await
        .map_err(ServerError::internal)?;

    Ok(Json(AppendMemoryMessagesResponse {
        thread_id,
        appended,
    }))
}

#[instrument(skip(state))]
async fn list_default_memory_messages(
    State(state): State<AppState>,
    Path(thread_id): Path<String>,
    Query(query): Query<ListMessagesQuery>,
) -> ServerResult<Json<ListMemoryMessagesResponse>> {
    let memory = state.registry.find_default_memory()?;
    list_memory_messages_for(memory, thread_id, query).await
}

#[instrument(skip(state))]
async fn list_memory_messages(
    State(state): State<AppState>,
    Path((memory_id, thread_id)): Path<(String, String)>,
    Query(query): Query<ListMessagesQuery>,
) -> ServerResult<Json<ListMemoryMessagesResponse>> {
    let memory = state.registry.find_memory(&memory_id)?;
    list_memory_messages_for(memory, thread_id, query).await
}

async fn list_memory_messages_for(
    memory: Arc<dyn MemoryEngine>,
    thread_id: String,
    query: ListMessagesQuery,
) -> ServerResult<Json<ListMemoryMessagesResponse>> {
    let per_page = query.parsed_per_page().map_err(ServerError::BadRequest)?;
    let order_by = query
        .parsed_order_by()
        .map_err(|error| ServerError::BadRequest(format!("invalid orderBy: {error}")))?;
    let messages = memory
        .list_messages_page(MemoryRecallRequest {
            thread_id,
            limit: None,
            resource_id: query.resource_id.clone(),
            page: query.page,
            per_page,
            message_ids: query.message_ids.clone(),
            start_date: query.start_date,
            end_date: query.end_date,
            order_by: order_by.map(message_order_to_core),
        })
        .await
        .map_err(ServerError::internal)?;

    Ok(Json(ListMemoryMessagesResponse {
        messages: messages.messages,
        total: messages.total,
        page: messages.page,
        per_page: query.response_per_page(messages.per_page),
        has_more: messages.has_more,
    }))
}

#[instrument(skip(state, request))]
async fn delete_default_memory_messages(
    State(state): State<AppState>,
    Json(request): Json<DeleteMemoryMessagesRequest>,
) -> ServerResult<Json<DeleteMemoryMessagesResponse>> {
    let memory = state.registry.find_default_memory()?;
    delete_memory_messages_for(memory, request).await
}

#[instrument(skip(state, request))]
async fn delete_memory_messages(
    State(state): State<AppState>,
    Path(memory_id): Path<String>,
    Json(request): Json<DeleteMemoryMessagesRequest>,
) -> ServerResult<Json<DeleteMemoryMessagesResponse>> {
    let memory = state.registry.find_memory(&memory_id)?;
    delete_memory_messages_for(memory, request).await
}

async fn delete_memory_messages_for(
    memory: Arc<dyn MemoryEngine>,
    request: DeleteMemoryMessagesRequest,
) -> ServerResult<Json<DeleteMemoryMessagesResponse>> {
    let deleted = memory
        .delete_messages(request.message_ids.into_ids())
        .await
        .map_err(ServerError::internal)?;

    Ok(Json(DeleteMemoryMessagesResponse {
        success: true,
        message: format!(
            "{deleted} message{} deleted successfully",
            if deleted == 1 { "" } else { "s" }
        ),
        deleted,
    }))
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
    let run = state.registry.begin_workflow_run(&workflow_id, &request)?;
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

fn parse_resume_run_id(request: &ResumeWorkflowRunRequest) -> ServerResult<Uuid> {
    let run_id = request
        .run_id
        .as_deref()
        .ok_or_else(|| ServerError::BadRequest("runId required to resume workflow".to_owned()))?;
    Uuid::parse_str(run_id)
        .map_err(|error| ServerError::BadRequest(format!("invalid runId '{run_id}': {error}")))
}

fn start_request_from_resume(
    existing_run: &WorkflowRunRecord,
    request: ResumeWorkflowRunRequest,
) -> StartWorkflowRunRequest {
    StartWorkflowRunRequest {
        resource_id: existing_run.resource_id.clone(),
        input_data: request
            .resume_data
            .or_else(|| existing_run.input_data.clone()),
        request_context: request.request_context,
    }
}

#[instrument(skip(state, request))]
async fn resume_workflow_async(
    State(state): State<AppState>,
    Path(workflow_id): Path<String>,
    Json(request): Json<ResumeWorkflowRunRequest>,
) -> ServerResult<Json<StartWorkflowRunResponse>> {
    let workflow = state.registry.find_workflow(&workflow_id)?;
    let run_id = parse_resume_run_id(&request)?;
    let existing_run = state.registry.get_workflow_run(&workflow_id, run_id)?;
    let start_request = start_request_from_resume(&existing_run, request);

    state
        .registry
        .restart_workflow_run(&workflow_id, run_id, &start_request)?;

    match workflow.start(start_request).await {
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
                "workflow '{}' failed to resume: {} (run_id={})",
                workflow_id, error, run.run_id
            )))
        }
    }
}

#[instrument(skip(state, request))]
async fn resume_workflow(
    State(state): State<AppState>,
    Path(workflow_id): Path<String>,
    Json(request): Json<ResumeWorkflowRunRequest>,
) -> ServerResult<Json<ResumeWorkflowRunResponse>> {
    let response = resume_workflow_async(State(state), Path(workflow_id), Json(request)).await?;

    Ok(Json(ResumeWorkflowRunResponse {
        message: "Workflow run resumed".to_owned(),
        run: Some(response.0.run),
    }))
}

#[instrument(skip(state, request))]
async fn stream_workflow(
    State(state): State<AppState>,
    Path(workflow_id): Path<String>,
    Query(query): Query<WorkflowStreamQuery>,
    Json(request): Json<StartWorkflowRunRequest>,
) -> ServerResult<Sse<impl futures::Stream<Item = Result<Event, Infallible>>>> {
    let workflow = state.registry.find_workflow(&workflow_id)?;
    let run_id = query.run_id;
    let run_uuid = Uuid::parse_str(&run_id)
        .map_err(|error| ServerError::BadRequest(format!("invalid runId '{run_id}': {error}")))?;
    let run = state
        .registry
        .begin_workflow_run_with_id(&workflow_id, &request, run_uuid)?;
    let mut step_ids = workflow
        .detail()
        .steps
        .into_iter()
        .map(|step| step.id)
        .collect::<Vec<_>>();
    if step_ids.is_empty() {
        step_ids.push("workflow".to_owned());
    }
    let registry = state.registry.clone();
    let workflow_id_for_events = workflow_id.clone();

    let event_stream = stream! {
        yield Ok(encode_workflow_stream_event(WorkflowStreamEvent::Start(
            WorkflowStreamStartEvent {
                run_id: run_id.clone(),
                workflow_id: workflow_id_for_events.clone(),
                resource_id: run.resource_id.clone(),
            },
        )));

        for step_id in &step_ids {
            yield Ok(encode_workflow_stream_event(WorkflowStreamEvent::StepStart(
                WorkflowStreamStepEvent {
                    run_id: run_id.clone(),
                    workflow_id: workflow_id_for_events.clone(),
                    step_id: step_id.clone(),
                },
            )));
        }

        match workflow.start(request).await {
            Ok(result) => {
                for step_id in &step_ids {
                    yield Ok(encode_workflow_stream_event(WorkflowStreamEvent::StepFinish(
                        WorkflowStreamStepEvent {
                            run_id: run_id.clone(),
                            workflow_id: workflow_id_for_events.clone(),
                            step_id: step_id.clone(),
                        },
                    )));
                }

                match registry.complete_workflow_run_success(run_uuid, result.clone()) {
                    Ok(_) => {
                        yield Ok(encode_workflow_stream_event(WorkflowStreamEvent::Finish(
                            WorkflowStreamFinishEvent {
                                run_id: run_id.clone(),
                                workflow_id: workflow_id_for_events.clone(),
                                status: "success".to_owned(),
                                result,
                            },
                        )));
                    }
                    Err(error) => {
                        yield Ok(encode_workflow_stream_event(WorkflowStreamEvent::Error(
                            ErrorResponse {
                                error: error.to_string(),
                            },
                        )));
                    }
                }
            }
            Err(error) => {
                let _ = registry.complete_workflow_run_failure(run_uuid, &error);
                yield Ok(encode_workflow_stream_event(WorkflowStreamEvent::Error(
                    ErrorResponse {
                        error: error.to_string(),
                    },
                )));
            }
        }
    };

    Ok(Sse::new(event_stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15))))
}

#[instrument(skip(state, request))]
async fn stream_resumed_workflow(
    State(state): State<AppState>,
    Path(workflow_id): Path<String>,
    Json(request): Json<ResumeWorkflowRunRequest>,
) -> ServerResult<Sse<impl futures::Stream<Item = Result<Event, Infallible>>>> {
    let workflow = state.registry.find_workflow(&workflow_id)?;
    let run_uuid = parse_resume_run_id(&request)?;
    let existing_run = state.registry.get_workflow_run(&workflow_id, run_uuid)?;
    let start_request = start_request_from_resume(&existing_run, request);
    let resumed_run =
        state
            .registry
            .restart_workflow_run(&workflow_id, run_uuid, &start_request)?;
    let run_id = resumed_run.run_id.to_string();
    let mut step_ids = workflow
        .detail()
        .steps
        .into_iter()
        .map(|step| step.id)
        .collect::<Vec<_>>();
    if step_ids.is_empty() {
        step_ids.push("workflow".to_owned());
    }
    let registry = state.registry.clone();
    let workflow_id_for_events = workflow_id.clone();

    let event_stream = stream! {
        yield Ok(encode_workflow_stream_event(WorkflowStreamEvent::Start(
            WorkflowStreamStartEvent {
                run_id: run_id.clone(),
                workflow_id: workflow_id_for_events.clone(),
                resource_id: resumed_run.resource_id.clone(),
            },
        )));

        for step_id in &step_ids {
            yield Ok(encode_workflow_stream_event(WorkflowStreamEvent::StepStart(
                WorkflowStreamStepEvent {
                    run_id: run_id.clone(),
                    workflow_id: workflow_id_for_events.clone(),
                    step_id: step_id.clone(),
                },
            )));
        }

        match workflow.start(start_request).await {
            Ok(result) => {
                for step_id in &step_ids {
                    yield Ok(encode_workflow_stream_event(WorkflowStreamEvent::StepFinish(
                        WorkflowStreamStepEvent {
                            run_id: run_id.clone(),
                            workflow_id: workflow_id_for_events.clone(),
                            step_id: step_id.clone(),
                        },
                    )));
                }

                match registry.complete_workflow_run_success(run_uuid, result.clone()) {
                    Ok(_) => {
                        yield Ok(encode_workflow_stream_event(WorkflowStreamEvent::Finish(
                            WorkflowStreamFinishEvent {
                                run_id: run_id.clone(),
                                workflow_id: workflow_id_for_events.clone(),
                                status: "success".to_owned(),
                                result,
                            },
                        )));
                    }
                    Err(error) => {
                        yield Ok(encode_workflow_stream_event(WorkflowStreamEvent::Error(
                            ErrorResponse {
                                error: error.to_string(),
                            },
                        )));
                    }
                }
            }
            Err(error) => {
                let _ = registry.complete_workflow_run_failure(run_uuid, &error);
                yield Ok(encode_workflow_stream_event(WorkflowStreamEvent::Error(
                    ErrorResponse {
                        error: error.to_string(),
                    },
                )));
            }
        }
    };

    Ok(Sse::new(event_stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15))))
}

#[instrument(skip(state))]
async fn list_workflow_runs(
    State(state): State<AppState>,
    Path(workflow_id): Path<String>,
    Query(query): Query<ListWorkflowRunsQuery>,
) -> ServerResult<Json<ListWorkflowRunsResponse>> {
    Ok(Json(
        state.registry.list_workflow_runs(&workflow_id, &query)?,
    ))
}

#[instrument(skip(state))]
async fn get_workflow_run(
    State(state): State<AppState>,
    Path((workflow_id, run_id)): Path<(String, String)>,
) -> ServerResult<Json<WorkflowRunRecord>> {
    let parsed_run_id =
        Uuid::parse_str(&run_id).map_err(|error| ServerError::BadRequest(error.to_string()))?;
    let run = state
        .registry
        .get_workflow_run(&workflow_id, parsed_run_id)?;
    Ok(Json(run))
}

#[instrument(skip(state))]
async fn delete_workflow_run(
    State(state): State<AppState>,
    Path((workflow_id, run_id)): Path<(String, String)>,
) -> ServerResult<Json<DeleteWorkflowRunResponse>> {
    let parsed_run_id =
        Uuid::parse_str(&run_id).map_err(|error| ServerError::BadRequest(error.to_string()))?;
    state
        .registry
        .delete_workflow_run(&workflow_id, parsed_run_id)?;
    Ok(Json(DeleteWorkflowRunResponse {
        message: "Workflow run deleted".to_owned(),
    }))
}

#[instrument(skip(state))]
async fn cancel_workflow_run(
    State(state): State<AppState>,
    Path((workflow_id, run_id)): Path<(String, String)>,
) -> ServerResult<Json<CancelWorkflowRunResponse>> {
    let parsed_run_id =
        Uuid::parse_str(&run_id).map_err(|error| ServerError::BadRequest(error.to_string()))?;
    state
        .registry
        .cancel_workflow_run(&workflow_id, parsed_run_id)?;

    Ok(Json(CancelWorkflowRunResponse {
        message: "Workflow run cancelled".to_owned(),
    }))
}

fn encode_stream_event(event: GenerateStreamEvent) -> Event {
    let data = serde_json::to_string(&event).unwrap_or_else(|error| {
        serde_json::json!({
            "type": "error",
            "error": {
                "error": error.to_string(),
            },
        })
        .to_string()
    });

    Event::default().event(event.event_name()).data(data)
}

fn thread_order_to_core(order: ThreadOrderBy) -> MemoryThreadOrder {
    MemoryThreadOrder {
        field: match order.field {
            ThreadOrderField::CreatedAt => MemoryThreadOrderField::CreatedAt,
            ThreadOrderField::UpdatedAt => MemoryThreadOrderField::UpdatedAt,
        },
        direction: match order.direction {
            OrderDirection::Asc => MemoryOrderDirection::Asc,
            OrderDirection::Desc => MemoryOrderDirection::Desc,
        },
    }
}

fn message_order_to_core(order: MessageOrderBy) -> MemoryMessageOrder {
    MemoryMessageOrder {
        field: match order.field {
            MessageOrderField::CreatedAt => MemoryMessageOrderField::CreatedAt,
        },
        direction: match order.direction {
            OrderDirection::Asc => MemoryOrderDirection::Asc,
            OrderDirection::Desc => MemoryOrderDirection::Desc,
        },
    }
}

fn encode_workflow_stream_event(event: WorkflowStreamEvent) -> Event {
    let data = serde_json::to_string(&event).unwrap_or_else(|error| {
        serde_json::json!({
            "type": "error",
            "error": {
                "error": error.to_string(),
            },
        })
        .to_string()
    });

    Event::default().event(event.event_name()).data(data)
}

fn load_system_packages() -> Vec<SystemPackage> {
    let Some(path) = std::env::var_os("MASTRA_PACKAGES_FILE") else {
        return Vec::new();
    };

    let Ok(content) = fs::read_to_string(path) else {
        return Vec::new();
    };

    serde_json::from_str(&content).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use std::{
        collections::{HashMap, HashSet},
        sync::Arc,
    };

    use async_trait::async_trait;
    use axum::{
        body::{Body, to_bytes},
        http::{Request, StatusCode},
    };
    use chrono::Utc;
    use futures::{StreamExt, stream::BoxStream};
    use mastra_core::{
        Agent, AgentConfig, AppendObservationRequest as CoreAppendObservationRequest,
        CloneThreadRequest, CreateThreadRequest, FinishReason as CoreFinishReason, MemoryConfig,
        MemoryEngine, MemoryMessage, MemoryRecallRequest, MemoryRole, ModelRequest,
        ModelResponse, ModelToolCall, ObservationPage, ObservationQuery, ObservationRecord,
        StaticModel, Thread, Tool, UpdateThreadRequest,
        UpdateWorkingMemoryRequest as CoreUpdateWorkingMemoryRequest, WorkingMemoryState,
    };
    use parking_lot::RwLock;
    use serde_json::{Value, json};
    use tokio::time::{Duration, sleep};
    use tower::ServiceExt;
    use uuid::Uuid;

    use crate::{
        contracts::{
            AgentMessages, AgentSummary, FinishReason, GenerateRequest, GenerateResponse,
            GenerateStreamEvent, GenerateStreamFinishEvent, GenerateStreamStartEvent,
            GenerateStreamTextDeltaEvent, ResumeWorkflowRunRequest, StartWorkflowRunRequest,
            WorkflowRunStatus, WorkflowSummary,
        },
        registry::RuntimeRegistry,
        runtime::{AgentRuntime, WorkflowRuntime},
    };

    use super::{MastraServer, list_memory_messages_for};

    struct EchoAgent;

    #[async_trait]
    impl AgentRuntime for EchoAgent {
        fn summary(&self) -> AgentSummary {
            AgentSummary {
                id: "echo".to_owned(),
                name: "Echo Agent".to_owned(),
                description: Some("Echoes flattened input".to_owned()),
            }
        }

        fn detail(&self) -> crate::contracts::AgentDetail {
            crate::contracts::AgentDetail {
                id: "echo".to_owned(),
                name: "Echo Agent".to_owned(),
                instructions: "Echoes the prompt".to_owned(),
                description: Some("Echoes flattened input".to_owned()),
                tools: Vec::new(),
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

        fn stream(
            &self,
            request: GenerateRequest,
        ) -> BoxStream<'static, crate::error::ServerResult<GenerateStreamEvent>> {
            let text = format!("echo: {}", request.messages.flatten_text());
            let run_id = request.run_id.unwrap_or_else(|| "test-run".to_owned());
            let message_id = "message-1".to_owned();

            futures::stream::iter(vec![
                Ok(GenerateStreamEvent::Start(GenerateStreamStartEvent {
                    run_id: run_id.clone(),
                    message_id: message_id.clone(),
                    thread_id: None,
                })),
                Ok(GenerateStreamEvent::TextDelta(
                    GenerateStreamTextDeltaEvent {
                        run_id: run_id.clone(),
                        message_id: message_id.clone(),
                        delta: text.clone(),
                    },
                )),
                Ok(GenerateStreamEvent::Finish(GenerateStreamFinishEvent {
                    run_id,
                    message_id,
                    thread_id: None,
                    text,
                    finish_reason: FinishReason::Stop,
                    usage: None,
                })),
            ])
            .boxed()
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

        fn detail(&self) -> crate::contracts::WorkflowDetail {
            crate::contracts::WorkflowDetail {
                id: "demo".to_owned(),
                description: Some("Returns workflow input".to_owned()),
                steps: Vec::new(),
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

    #[derive(Default)]
    struct TestMemory {
        threads: RwLock<HashMap<String, Thread>>,
        messages: RwLock<HashMap<String, Vec<MemoryMessage>>>,
        working_memory: RwLock<HashMap<String, WorkingMemoryState>>,
        observations: RwLock<HashMap<String, Vec<ObservationRecord>>>,
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

        async fn update_thread(
            &self,
            thread_id: &str,
            request: UpdateThreadRequest,
        ) -> mastra_core::Result<Thread> {
            let mut threads = self.threads.write();
            let thread = threads.get_mut(thread_id).ok_or_else(|| {
                mastra_core::MastraError::not_found(format!("thread '{thread_id}' was not found"))
            })?;

            if let Some(resource_id) = request.resource_id {
                thread.resource_id = Some(resource_id);
            }
            if let Some(title) = request.title {
                thread.title = Some(title);
            }
            if let Some(metadata) = request.metadata {
                thread.metadata = metadata;
            }
            thread.updated_at = Utc::now();

            Ok(thread.clone())
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
                        .map(|expected| thread.resource_id.as_deref() == Some(expected))
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
                .entry(thread_id.to_owned())
                .or_default()
                .extend(messages);
            Ok(())
        }

        async fn list_messages(
            &self,
            request: MemoryRecallRequest,
        ) -> mastra_core::Result<Vec<MemoryMessage>> {
            if let Some(resource_id) = request.resource_id.as_deref() {
                let thread = self
                    .threads
                    .read()
                    .get(&request.thread_id)
                    .cloned()
                    .ok_or_else(|| {
                        mastra_core::MastraError::not_found(format!(
                            "thread '{}' was not found",
                            request.thread_id
                        ))
                    })?;
                if thread.resource_id.as_deref() != Some(resource_id) {
                    return Ok(Vec::new());
                }
            }

            let mut messages = self
                .messages
                .read()
                .get(&request.thread_id)
                .cloned()
                .unwrap_or_default();

            if let Some(message_ids) = request.message_ids.as_ref() {
                let allowed = message_ids.iter().collect::<HashSet<_>>();
                messages.retain(|message| allowed.contains(&message.id));
            }

            if let Some(start_date) = request.start_date {
                messages.retain(|message| message.created_at >= start_date);
            }

            if let Some(end_date) = request.end_date {
                messages.retain(|message| message.created_at <= end_date);
            }

            if let Some(limit) = request.limit {
                let start = messages.len().saturating_sub(limit);
                messages = messages[start..].to_vec();
            }
            Ok(messages)
        }

        async fn get_working_memory(
            &self,
            thread_id: &str,
            _resource_id: Option<&str>,
        ) -> mastra_core::Result<Option<WorkingMemoryState>> {
            Ok(self.working_memory.read().get(thread_id).cloned())
        }

        async fn update_working_memory(
            &self,
            request: CoreUpdateWorkingMemoryRequest,
        ) -> mastra_core::Result<WorkingMemoryState> {
            let state = WorkingMemoryState {
                thread_id: request.thread_id.clone(),
                resource_id: request.resource_id.clone(),
                scope: request.scope,
                format: request.format,
                template: request.template,
                content: request.content,
                updated_at: Utc::now(),
            };
            self.working_memory
                .write()
                .insert(request.thread_id.clone(), state.clone());
            Ok(state)
        }

        async fn list_observations(
            &self,
            request: ObservationQuery,
        ) -> mastra_core::Result<ObservationPage> {
            let observations = self
                .observations
                .read()
                .get(&request.thread_id)
                .cloned()
                .unwrap_or_default();
            Ok(ObservationPage {
                total: observations.len(),
                page: request.page.unwrap_or(0),
                per_page: request.per_page.unwrap_or_else(|| observations.len().max(1)),
                has_more: false,
                observations,
            })
        }

        async fn append_observation(
            &self,
            request: CoreAppendObservationRequest,
        ) -> mastra_core::Result<ObservationRecord> {
            let observation = ObservationRecord {
                id: Uuid::now_v7().to_string(),
                thread_id: request.thread_id.clone(),
                resource_id: request.resource_id.clone(),
                scope: request.scope,
                content: request.content,
                observed_message_ids: request.observed_message_ids,
                metadata: request.metadata,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            };
            self.observations
                .write()
                .entry(request.thread_id)
                .or_default()
                .push(observation.clone());
            Ok(observation)
        }

        async fn clone_thread(&self, request: CloneThreadRequest) -> mastra_core::Result<Thread> {
            let source_thread = self
                .threads
                .read()
                .get(&request.source_thread_id)
                .cloned()
                .ok_or_else(|| {
                    mastra_core::MastraError::not_found(format!(
                        "thread '{}' was not found",
                        request.source_thread_id
                    ))
                })?;

            let now = Utc::now();
            let cloned_thread = Thread {
                id: request
                    .new_thread_id
                    .unwrap_or_else(|| Uuid::now_v7().to_string()),
                resource_id: request.resource_id.or(source_thread.resource_id),
                title: request.title.or(source_thread.title),
                created_at: now,
                updated_at: now,
                metadata: request.metadata.unwrap_or(source_thread.metadata),
            };

            let mut source_messages = self
                .messages
                .read()
                .get(&request.source_thread_id)
                .cloned()
                .unwrap_or_default();

            if let Some(message_ids) = request.message_ids.as_ref() {
                let allowed = message_ids.iter().collect::<HashSet<_>>();
                source_messages.retain(|message| allowed.contains(&message.id));
            }

            if let Some(start_date) = request.start_date {
                source_messages.retain(|message| message.created_at >= start_date);
            }

            if let Some(end_date) = request.end_date {
                source_messages.retain(|message| message.created_at <= end_date);
            }

            if let Some(message_limit) = request.message_limit {
                let start = source_messages.len().saturating_sub(message_limit);
                source_messages = source_messages.into_iter().skip(start).collect();
            }

            let cloned_messages = source_messages
                .into_iter()
                .map(|message| MemoryMessage {
                    id: Uuid::now_v7().to_string(),
                    thread_id: cloned_thread.id.clone(),
                    ..message
                })
                .collect::<Vec<_>>();

            self.threads
                .write()
                .insert(cloned_thread.id.clone(), cloned_thread.clone());
            self.messages
                .write()
                .insert(cloned_thread.id.clone(), cloned_messages);

            Ok(cloned_thread)
        }

        async fn delete_messages(&self, message_ids: Vec<String>) -> mastra_core::Result<usize> {
            let message_ids = message_ids.into_iter().collect::<HashSet<_>>();
            let mut deleted = 0;
            let mut messages = self.messages.write();

            for thread_messages in messages.values_mut() {
                let before = thread_messages.len();
                thread_messages.retain(|message| !message_ids.contains(&message.id));
                deleted += before - thread_messages.len();
            }

            Ok(deleted)
        }

        async fn delete_thread(&self, thread_id: &str) -> mastra_core::Result<()> {
            let removed = self.threads.write().remove(thread_id);
            self.messages.write().remove(thread_id);

            if removed.is_none() {
                return Err(mastra_core::MastraError::not_found(format!(
                    "thread '{thread_id}' was not found"
                )));
            }

            Ok(())
        }
    }

    fn build_router() -> axum::Router {
        let registry = RuntimeRegistry::new();
        registry.register_agent(EchoAgent);
        registry.register_workflow(JsonWorkflow);
        MastraServer::new(registry).into_router()
    }

    fn build_tool_stream_router() -> axum::Router {
        let server = MastraServer::new(RuntimeRegistry::new());
        let steps = Arc::new(RwLock::new(0usize));
        let model_steps = Arc::clone(&steps);

        server.register_agent(Agent::new(AgentConfig {
            id: "tool-stream".to_owned(),
            name: "Tool Stream".to_owned(),
            instructions: "Use tools when helpful".to_owned(),
            description: Some("Streams tool lifecycle events".to_owned()),
            model: Arc::new(StaticModel::new(move |_request: ModelRequest| {
                let model_steps = Arc::clone(&model_steps);
                async move {
                    let step = {
                        let mut step_count = model_steps.write();
                        let current = *step_count;
                        *step_count += 1;
                        current
                    };

                    match step {
                        0 => Ok(ModelResponse {
                            text: String::new(),
                            data: Value::Null,
                            finish_reason: CoreFinishReason::ToolCall,
                            usage: None,
                            tool_calls: vec![ModelToolCall {
                                id: "call-http".to_owned(),
                                name: "sum".to_owned(),
                                input: json!({ "a": 2, "b": 5 }),
                            }],
                        }),
                        1 => Ok(ModelResponse {
                            text: "7".to_owned(),
                            data: Value::Null,
                            finish_reason: CoreFinishReason::Stop,
                            usage: None,
                            tool_calls: Vec::new(),
                        }),
                        other => panic!("unexpected model step {other}"),
                    }
                }
            })),
            tools: vec![Tool::new(
                "sum",
                "add numbers",
                |input, _context| async move {
                    let a = input.get("a").and_then(Value::as_i64).unwrap_or_default();
                    let b = input.get("b").and_then(Value::as_i64).unwrap_or_default();
                    Ok(json!(a + b))
                },
            )],
            memory: None,
            memory_config: MemoryConfig::default(),
        }));

        server.into_router()
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
    async fn exposes_agent_and_workflow_details() {
        let agent_response = build_router()
            .oneshot(
                Request::builder()
                    .uri("/api/agents/echo")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(agent_response.status(), StatusCode::OK);
        let agent_body = to_bytes(agent_response.into_body(), usize::MAX)
            .await
            .unwrap();
        let agent_payload: Value = serde_json::from_slice(&agent_body).unwrap();
        assert_eq!(agent_payload["agent"]["id"], "echo");
        assert_eq!(agent_payload["agent"]["name"], "Echo Agent");
        assert_eq!(agent_payload["agent"]["instructions"], "Echoes the prompt");

        let workflow_response = build_router()
            .oneshot(
                Request::builder()
                    .uri("/api/workflows/demo")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(workflow_response.status(), StatusCode::OK);
        let workflow_body = to_bytes(workflow_response.into_body(), usize::MAX)
            .await
            .unwrap();
        let workflow_payload: Value = serde_json::from_slice(&workflow_body).unwrap();
        assert_eq!(workflow_payload["workflow"]["id"], "demo");
        assert_eq!(
            workflow_payload["workflow"]["description"],
            "Returns workflow input"
        );
    }

    #[tokio::test]
    async fn lists_and_executes_agent_and_global_tools() {
        let server = MastraServer::new(RuntimeRegistry::new());
        server.register_agent(Agent::new(AgentConfig {
            id: "calculator".to_owned(),
            name: "Calculator".to_owned(),
            instructions: "Use arithmetic tools".to_owned(),
            description: Some("Provides arithmetic helpers".to_owned()),
            model: Arc::new(StaticModel::echo()),
            tools: vec![Tool::new(
                "sum",
                "add numbers",
                |input, _context| async move {
                    let a = input.get("a").and_then(Value::as_i64).unwrap_or_default();
                    let b = input.get("b").and_then(Value::as_i64).unwrap_or_default();
                    Ok(json!(a + b))
                },
            )],
            memory: None,
            memory_config: MemoryConfig::default(),
        }));
        server.register_tool(Tool::new(
            "ping",
            "ping the service",
            |_input, _context| async move { Ok(json!({ "pong": true })) },
        ));
        let router = server.into_router();

        let tools_response = router
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/tools")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(tools_response.status(), StatusCode::OK);
        let tools_body = to_bytes(tools_response.into_body(), usize::MAX)
            .await
            .unwrap();
        let tools_payload: Value = serde_json::from_slice(&tools_body).unwrap();
        assert_eq!(tools_payload["tools"].as_array().unwrap().len(), 2);
        assert!(
            tools_payload["tools"]
                .as_array()
                .unwrap()
                .iter()
                .any(|tool| tool["id"] == "sum")
        );
        assert!(
            tools_payload["tools"]
                .as_array()
                .unwrap()
                .iter()
                .any(|tool| tool["id"] == "ping")
        );

        let agent_tools_response = router
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/agents/calculator/tools")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(agent_tools_response.status(), StatusCode::OK);
        let agent_tools_body = to_bytes(agent_tools_response.into_body(), usize::MAX)
            .await
            .unwrap();
        let agent_tools_payload: Value = serde_json::from_slice(&agent_tools_body).unwrap();
        assert_eq!(agent_tools_payload["tools"][0]["id"], "sum");

        let execute_agent_tool = router
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/agents/calculator/tools/sum/execute")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "data": { "a": 20, "b": 22 }
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(execute_agent_tool.status(), StatusCode::OK);
        let execute_agent_tool_body = to_bytes(execute_agent_tool.into_body(), usize::MAX)
            .await
            .unwrap();
        let execute_agent_tool_payload: Value =
            serde_json::from_slice(&execute_agent_tool_body).unwrap();
        assert_eq!(execute_agent_tool_payload["tool_id"], "sum");
        assert_eq!(execute_agent_tool_payload["output"], json!(42));

        let execute_global_tool = router
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/tools/ping/execute")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "data": { "source": "test" }
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(execute_global_tool.status(), StatusCode::OK);
        let execute_global_tool_body = to_bytes(execute_global_tool.into_body(), usize::MAX)
            .await
            .unwrap();
        let execute_global_tool_payload: Value =
            serde_json::from_slice(&execute_global_tool_body).unwrap();
        assert_eq!(execute_global_tool_payload["tool_id"], "ping");
        assert_eq!(
            execute_global_tool_payload["output"],
            json!({ "pong": true })
        );
    }

    #[tokio::test]
    async fn generates_agent_responses() {
        let request = serde_json::to_vec(&GenerateRequest {
            messages: AgentMessages::Text("hello".to_owned()),
            instructions: None,
            system: None,
            context: Vec::new(),
            memory: None,
            resource_id: None,
            thread_id: None,
            run_id: None,
            max_steps: Some(1),
            active_tools: None,
            tool_choice: None,
            output: None,
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
    async fn streams_agent_responses_as_sse_events() {
        let request = serde_json::to_vec(&GenerateRequest {
            messages: AgentMessages::Text("hello".to_owned()),
            instructions: None,
            system: None,
            context: Vec::new(),
            memory: None,
            resource_id: None,
            thread_id: None,
            run_id: Some("run-123".to_owned()),
            max_steps: Some(1),
            active_tools: None,
            tool_choice: None,
            output: None,
            request_context: Default::default(),
        })
        .unwrap();

        let response = build_router()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/agents/echo/stream")
                    .header("content-type", "application/json")
                    .body(Body::from(request))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let body = String::from_utf8(body.to_vec()).unwrap();
        assert!(body.contains("event: start"));
        assert!(body.contains("event: text_delta"));
        assert!(body.contains("event: finish"));
        assert!(body.contains("\"run_id\":\"run-123\""));
    }

    #[tokio::test]
    async fn streams_tool_lifecycle_events_as_sse() {
        let request = serde_json::to_vec(&GenerateRequest {
            messages: AgentMessages::Text("2 + 5 = ?".to_owned()),
            instructions: None,
            system: None,
            context: Vec::new(),
            memory: None,
            resource_id: None,
            thread_id: Some("thread-http".to_owned()),
            run_id: Some("run-tool".to_owned()),
            max_steps: Some(4),
            active_tools: None,
            tool_choice: None,
            output: None,
            request_context: Default::default(),
        })
        .unwrap();

        let response = build_tool_stream_router()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/agents/tool-stream/stream")
                    .header("content-type", "application/json")
                    .body(Body::from(request))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let body = String::from_utf8(body.to_vec()).unwrap();
        assert!(body.contains("event: start"));
        assert!(body.contains("event: tool_call"));
        assert!(body.contains("event: tool_result"));
        assert!(body.contains("event: finish"));
        assert!(body.contains("\"tool_call_id\":\"call-http\""));
        assert!(body.contains("\"tool_name\":\"sum\""));
        assert!(body.contains("\"text\":\"7\""));
    }

    #[tokio::test]
    async fn streams_workflows_as_sse_and_persists_requested_run_id() {
        let registry = RuntimeRegistry::new();
        registry.register_workflow(JsonWorkflow);

        let response = MastraServer::new(registry.clone())
            .into_router()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/workflows/demo/stream?runId=018f7f26-8b7e-7c9d-b145-2c3d4e5f6789")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&json!({
                            "resourceId": "resource-stream",
                            "inputData": {"topic": "rust"}
                        }))
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let body = String::from_utf8(body.to_vec()).unwrap();
        assert!(body.contains("event: start"));
        assert!(body.contains("event: step_start"));
        assert!(body.contains("event: step_finish"));
        assert!(body.contains("event: finish"));
        assert!(body.contains("\"run_id\":\"018f7f26-8b7e-7c9d-b145-2c3d4e5f6789\""));
        assert!(body.contains("\"workflow_id\":\"demo\""));

        let run = registry
            .get_workflow_run(
                "demo",
                Uuid::parse_str("018f7f26-8b7e-7c9d-b145-2c3d4e5f6789").unwrap(),
            )
            .expect("streamed workflow run should be recorded");
        assert_eq!(run.status, WorkflowRunStatus::Success);
        assert_eq!(run.resource_id.as_deref(), Some("resource-stream"));
    }

    #[tokio::test]
    async fn rejects_workflow_stream_requests_without_run_id() {
        let registry = RuntimeRegistry::new();
        registry.register_workflow(JsonWorkflow);

        let response = MastraServer::new(registry)
            .into_router()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/workflows/demo/stream")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&json!({
                            "resourceId": "resource-stream",
                            "inputData": {"topic": "rust"}
                        }))
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
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

    #[tokio::test]
    async fn resumes_workflow_runs_asynchronously() {
        let registry = RuntimeRegistry::new();
        registry.register_workflow(JsonWorkflow);
        let router = MastraServer::new(registry.clone()).into_router();
        let run_id = "018f7f26-8b7e-7c9d-b145-2c3d4e5f6791";

        let created = router
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/workflows/demo/create-run")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&json!({
                            "runId": run_id,
                            "resourceId": "resource-resume",
                            "inputData": {"topic": "draft"}
                        }))
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(created.status(), StatusCode::CREATED);

        let resumed = router
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/workflows/demo/resume-async")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&ResumeWorkflowRunRequest {
                            run_id: Some(run_id.to_owned()),
                            step: None,
                            resume_data: Some(json!({"topic": "resumed"})),
                            request_context: Default::default(),
                        })
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resumed.status(), StatusCode::OK);
        let payload: Value =
            serde_json::from_slice(&to_bytes(resumed.into_body(), usize::MAX).await.unwrap())
                .unwrap();
        assert_eq!(payload["run"]["status"], json!(WorkflowRunStatus::Success));
        assert_eq!(payload["run"]["resource_id"], json!("resource-resume"));
        assert_eq!(
            payload["run"]["result"]["input"],
            json!({"topic": "resumed"})
        );

        let stored_run = registry
            .get_workflow_run("demo", Uuid::parse_str(run_id).unwrap())
            .unwrap();
        assert_eq!(stored_run.status, WorkflowRunStatus::Success);
        assert_eq!(stored_run.input_data, Some(json!({"topic": "resumed"})));
    }

    #[tokio::test]
    async fn resumes_workflow_runs_synchronously() {
        let registry = RuntimeRegistry::new();
        registry.register_workflow(JsonWorkflow);
        let router = MastraServer::new(registry.clone()).into_router();
        let run_id = "018f7f26-8b7e-7c9d-b145-2c3d4e5f6792";

        let _ = router
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/workflows/demo/create-run")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&json!({
                            "runId": run_id,
                            "resourceId": "resource-sync-resume",
                            "inputData": {"topic": "draft"}
                        }))
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        let resumed = router
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/workflows/demo/resume")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&ResumeWorkflowRunRequest {
                            run_id: Some(run_id.to_owned()),
                            step: None,
                            resume_data: Some(json!({"topic": "sync"})),
                            request_context: Default::default(),
                        })
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resumed.status(), StatusCode::OK);
        let payload: Value =
            serde_json::from_slice(&to_bytes(resumed.into_body(), usize::MAX).await.unwrap())
                .unwrap();
        assert_eq!(payload["message"], json!("Workflow run resumed"));
        assert_eq!(payload["run"]["status"], json!(WorkflowRunStatus::Success));
    }

    #[tokio::test]
    async fn cancels_workflow_runs() {
        let registry = RuntimeRegistry::new();
        registry.register_workflow(JsonWorkflow);
        let router = MastraServer::new(registry.clone()).into_router();
        let run_id = "018f7f26-8b7e-7c9d-b145-2c3d4e5f6793";

        let _ = router
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/workflows/demo/create-run")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&json!({
                            "runId": run_id,
                            "resourceId": "resource-cancel",
                            "inputData": {"topic": "draft"}
                        }))
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        let cancelled = router
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/workflows/demo/runs/{run_id}/cancel"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(cancelled.status(), StatusCode::OK);
        let payload: Value =
            serde_json::from_slice(&to_bytes(cancelled.into_body(), usize::MAX).await.unwrap())
                .unwrap();
        assert_eq!(payload["message"], json!("Workflow run cancelled"));

        let stored_run = registry
            .get_workflow_run("demo", Uuid::parse_str(run_id).unwrap())
            .unwrap();
        assert_eq!(stored_run.status, WorkflowRunStatus::Cancelled);
    }

    #[tokio::test]
    async fn exposes_default_memory_routes_with_official_resource_id_shape() {
        let server = MastraServer::new(RuntimeRegistry::new());
        server.register_memory("default", Arc::new(TestMemory::default()));
        let router = server.into_router();

        let create_response = router
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/memory/threads")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "resourceId": "resource-1",
                            "title": "Default thread",
                            "metadata": { "source": "test" },
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(create_response.status(), StatusCode::OK);
        let create_body = to_bytes(create_response.into_body(), usize::MAX)
            .await
            .unwrap();
        let create_payload: Value = serde_json::from_slice(&create_body).unwrap();
        let thread_id = create_payload["thread"]["id"].as_str().unwrap().to_owned();
        assert_eq!(create_payload["thread"]["resource_id"], "resource-1");

        let get_thread_response = router
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/memory/threads/{thread_id}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(get_thread_response.status(), StatusCode::OK);

        let append_response = router
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/memory/threads/{thread_id}/messages"))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "messages": [
                                {
                                    "role": "user",
                                    "content": "hello from default memory",
                                }
                            ]
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(append_response.status(), StatusCode::OK);

        let list_messages_response = router
            .oneshot(
                Request::builder()
                    .uri(format!("/api/memory/threads/{thread_id}/messages"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(list_messages_response.status(), StatusCode::OK);
        let list_messages_body = to_bytes(list_messages_response.into_body(), usize::MAX)
            .await
            .unwrap();
        let list_messages_payload: Value = serde_json::from_slice(&list_messages_body).unwrap();
        assert_eq!(
            list_messages_payload["messages"][0]["content"],
            "hello from default memory"
        );
    }

    #[tokio::test]
    async fn clones_default_memory_threads_and_keeps_history() {
        let server = MastraServer::new(RuntimeRegistry::new());
        server.register_memory("default", Arc::new(TestMemory::default()));
        let router = server.into_router();

        let create_response = router
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/memory/threads")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "resourceId": "resource-1",
                            "title": "Original thread",
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(create_response.status(), StatusCode::OK);
        let create_body = to_bytes(create_response.into_body(), usize::MAX)
            .await
            .unwrap();
        let create_payload: Value = serde_json::from_slice(&create_body).unwrap();
        let source_thread_id = create_payload["thread"]["id"].as_str().unwrap().to_owned();

        let append_response = router
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/memory/threads/{source_thread_id}/messages"))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "messages": [
                                { "role": "user", "content": "hello" },
                                { "role": "assistant", "content": "world" }
                            ]
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(append_response.status(), StatusCode::OK);

        let clone_response = router
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/memory/threads/{source_thread_id}/clone"))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "title": "Cloned thread",
                            "metadata": { "cloned": true }
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(clone_response.status(), StatusCode::OK);
        let clone_body = to_bytes(clone_response.into_body(), usize::MAX)
            .await
            .unwrap();
        let clone_payload: Value = serde_json::from_slice(&clone_body).unwrap();
        assert_ne!(clone_payload["thread"]["id"], json!(source_thread_id));
        assert_eq!(clone_payload["thread"]["title"], "Cloned thread");
        assert_eq!(
            clone_payload["cloned_messages"].as_array().unwrap().len(),
            2
        );
        assert_eq!(clone_payload["cloned_messages"][0]["content"], "hello");
        assert_eq!(clone_payload["cloned_messages"][1]["content"], "world");
    }

    #[tokio::test]
    async fn clones_only_requested_message_subset() {
        let server = MastraServer::new(RuntimeRegistry::new());
        server.register_memory("default", Arc::new(TestMemory::default()));
        let router = server.into_router();

        let create_response = router
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/memory/threads")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "resourceId": "resource-clone-limit",
                            "title": "Clone subset",
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        let create_body = to_bytes(create_response.into_body(), usize::MAX)
            .await
            .unwrap();
        let create_payload: Value = serde_json::from_slice(&create_body).unwrap();
        let source_thread_id = create_payload["thread"]["id"].as_str().unwrap().to_owned();

        let append_response = router
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/memory/threads/{source_thread_id}/messages"))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "messages": [
                                { "role": "user", "content": "first" },
                                { "role": "assistant", "content": "second" },
                                { "role": "assistant", "content": "third" }
                            ]
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(append_response.status(), StatusCode::OK);

        let clone_response = router
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/memory/threads/{source_thread_id}/clone"))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "title": "Last message only",
                            "messageLimit": 1
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(clone_response.status(), StatusCode::OK);
        let clone_body = to_bytes(clone_response.into_body(), usize::MAX)
            .await
            .unwrap();
        let clone_payload: Value = serde_json::from_slice(&clone_body).unwrap();
        assert_eq!(
            clone_payload["cloned_messages"].as_array().unwrap().len(),
            1
        );
        assert_eq!(clone_payload["cloned_messages"][0]["content"], "third");
    }

    #[tokio::test]
    async fn clones_requested_message_ids_from_nested_filter_options() {
        let server = MastraServer::new(RuntimeRegistry::new());
        server.register_memory("default", Arc::new(TestMemory::default()));
        let router = server.into_router();

        let create_response = router
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/memory/threads")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "resourceId": "resource-clone-filter",
                            "title": "Clone by message id",
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        let create_body = to_bytes(create_response.into_body(), usize::MAX)
            .await
            .unwrap();
        let create_payload: Value = serde_json::from_slice(&create_body).unwrap();
        let source_thread_id = create_payload["thread"]["id"].as_str().unwrap().to_owned();

        let append_response = router
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/memory/threads/{source_thread_id}/messages"))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "messages": [
                                { "role": "user", "content": "first" },
                                { "role": "assistant", "content": "second" },
                                { "role": "assistant", "content": "third" }
                            ]
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(append_response.status(), StatusCode::OK);

        let messages_response = router
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/memory/threads/{source_thread_id}/messages"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let messages_body = to_bytes(messages_response.into_body(), usize::MAX)
            .await
            .unwrap();
        let messages_payload: Value = serde_json::from_slice(&messages_body).unwrap();
        let second_message_id = messages_payload["messages"][1]["id"]
            .as_str()
            .unwrap()
            .to_owned();

        let clone_response = router
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/memory/threads/{source_thread_id}/clone"))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "title": "Specific clone",
                            "options": {
                                "messageFilter": {
                                    "messageIds": [second_message_id]
                                }
                            }
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(clone_response.status(), StatusCode::OK);
        let clone_body = to_bytes(clone_response.into_body(), usize::MAX)
            .await
            .unwrap();
        let clone_payload: Value = serde_json::from_slice(&clone_body).unwrap();
        assert_eq!(
            clone_payload["cloned_messages"].as_array().unwrap().len(),
            1
        );
        assert_eq!(clone_payload["cloned_messages"][0]["content"], "second");
    }

    #[tokio::test]
    async fn clones_only_messages_selected_by_filter() {
        let server = MastraServer::new(RuntimeRegistry::new());
        server.register_memory("default", Arc::new(TestMemory::default()));
        let router = server.into_router();

        let create_response = router
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/memory/threads")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "resourceId": "resource-clone-filter",
                            "title": "Clone filter",
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        let create_body = to_bytes(create_response.into_body(), usize::MAX)
            .await
            .unwrap();
        let create_payload: Value = serde_json::from_slice(&create_body).unwrap();
        let source_thread_id = create_payload["thread"]["id"].as_str().unwrap().to_owned();

        let append_response = router
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/memory/threads/{source_thread_id}/messages"))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "messages": [
                                { "role": "user", "content": "first" },
                                { "role": "assistant", "content": "second" },
                                { "role": "assistant", "content": "third" }
                            ]
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(append_response.status(), StatusCode::OK);

        let list_response = router
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/memory/threads/{source_thread_id}/messages"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let list_body = to_bytes(list_response.into_body(), usize::MAX)
            .await
            .unwrap();
        let list_payload: Value = serde_json::from_slice(&list_body).unwrap();
        let selected_message_id = list_payload["messages"][1]["id"]
            .as_str()
            .unwrap()
            .to_owned();

        let clone_response = router
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/memory/threads/{source_thread_id}/clone"))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "title": "Filtered clone",
                            "options": {
                                "messageFilter": {
                                    "messageIds": [selected_message_id]
                                }
                            }
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(clone_response.status(), StatusCode::OK);
        let clone_body = to_bytes(clone_response.into_body(), usize::MAX)
            .await
            .unwrap();
        let clone_payload: Value = serde_json::from_slice(&clone_body).unwrap();
        assert_eq!(
            clone_payload["cloned_messages"].as_array().unwrap().len(),
            1
        );
        assert_eq!(clone_payload["cloned_messages"][0]["content"], "second");
    }

    #[tokio::test]
    async fn deletes_default_memory_threads() {
        let server = MastraServer::new(RuntimeRegistry::new());
        server.register_memory("default", Arc::new(TestMemory::default()));
        let router = server.into_router();

        let create_response = router
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/memory/threads")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "resourceId": "resource-delete",
                            "title": "Delete me",
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(create_response.status(), StatusCode::OK);
        let create_body = to_bytes(create_response.into_body(), usize::MAX)
            .await
            .unwrap();
        let create_payload: Value = serde_json::from_slice(&create_body).unwrap();
        let thread_id = create_payload["thread"]["id"].as_str().unwrap().to_owned();

        let delete_response = router
            .clone()
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri(format!("/api/memory/threads/{thread_id}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(delete_response.status(), StatusCode::NO_CONTENT);

        let get_response = router
            .oneshot(
                Request::builder()
                    .uri(format!("/api/memory/threads/{thread_id}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(get_response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn deletes_messages_from_default_memory_with_official_route_shape() {
        let server = MastraServer::new(RuntimeRegistry::new());
        server.register_memory("default", Arc::new(TestMemory::default()));
        let router = server.into_router();

        let create_response = router
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/memory/threads")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "resourceId": "resource-delete-messages",
                            "title": "Delete some messages",
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(create_response.status(), StatusCode::OK);
        let create_body = to_bytes(create_response.into_body(), usize::MAX)
            .await
            .unwrap();
        let create_payload: Value = serde_json::from_slice(&create_body).unwrap();
        let thread_id = create_payload["thread"]["id"].as_str().unwrap().to_owned();

        let append_response = router
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/memory/threads/{thread_id}/messages"))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "messages": [
                                { "role": "user", "content": "keep me" },
                                { "role": "assistant", "content": "delete me" }
                            ]
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(append_response.status(), StatusCode::OK);

        let list_before_response = router
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/memory/threads/{thread_id}/messages"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(list_before_response.status(), StatusCode::OK);
        let list_before_body = to_bytes(list_before_response.into_body(), usize::MAX)
            .await
            .unwrap();
        let list_before_payload: Value = serde_json::from_slice(&list_before_body).unwrap();
        let deleted_message_id = list_before_payload["messages"]
            .as_array()
            .unwrap()
            .iter()
            .find(|message| message["content"] == "delete me")
            .and_then(|message| message["id"].as_str())
            .unwrap()
            .to_owned();

        let delete_response = router
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/memory/messages/delete")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "messageIds": deleted_message_id,
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(delete_response.status(), StatusCode::OK);
        let delete_body = to_bytes(delete_response.into_body(), usize::MAX)
            .await
            .unwrap();
        let delete_payload: Value = serde_json::from_slice(&delete_body).unwrap();
        assert_eq!(delete_payload["success"], true);
        assert_eq!(delete_payload["message"], "1 message deleted successfully");
        assert_eq!(delete_payload["deleted"], 1);

        let list_after_response = router
            .oneshot(
                Request::builder()
                    .uri(format!("/api/memory/threads/{thread_id}/messages"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(list_after_response.status(), StatusCode::OK);
        let list_after_body = to_bytes(list_after_response.into_body(), usize::MAX)
            .await
            .unwrap();
        let list_after_payload: Value = serde_json::from_slice(&list_after_body).unwrap();
        assert_eq!(list_after_payload["messages"].as_array().unwrap().len(), 1);
        assert_eq!(list_after_payload["messages"][0]["content"], "keep me");
    }

    #[tokio::test]
    async fn deletes_messages_from_named_memory_with_object_message_ids() {
        let server = MastraServer::new(RuntimeRegistry::new());
        server.register_memory("archive", Arc::new(TestMemory::default()));
        let router = server.into_router();

        let create_response = router
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/memory/archive/threads")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "resourceId": "resource-archive",
                            "title": "Archive thread",
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(create_response.status(), StatusCode::OK);
        let create_body = to_bytes(create_response.into_body(), usize::MAX)
            .await
            .unwrap();
        let create_payload: Value = serde_json::from_slice(&create_body).unwrap();
        let thread_id = create_payload["thread"]["id"].as_str().unwrap().to_owned();

        let append_response = router
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/memory/archive/threads/{thread_id}/messages"))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "messages": [
                                { "role": "user", "content": "first" },
                                { "role": "assistant", "content": "second" },
                                { "role": "assistant", "content": "third" }
                            ]
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(append_response.status(), StatusCode::OK);

        let list_before_response = router
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/memory/archive/threads/{thread_id}/messages"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(list_before_response.status(), StatusCode::OK);
        let list_before_body = to_bytes(list_before_response.into_body(), usize::MAX)
            .await
            .unwrap();
        let list_before_payload: Value = serde_json::from_slice(&list_before_body).unwrap();
        let deleted_message_id = list_before_payload["messages"]
            .as_array()
            .unwrap()
            .iter()
            .find(|message| message["content"] == "first")
            .and_then(|message| message["id"].as_str())
            .unwrap()
            .to_owned();

        let delete_response = router
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/memory/archive/messages/delete")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "messageIds": [{ "id": deleted_message_id }],
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(delete_response.status(), StatusCode::OK);
        let delete_body = to_bytes(delete_response.into_body(), usize::MAX)
            .await
            .unwrap();
        let delete_payload: Value = serde_json::from_slice(&delete_body).unwrap();
        assert_eq!(delete_payload["success"], true);
        assert_eq!(delete_payload["message"], "1 message deleted successfully");
        assert_eq!(delete_payload["deleted"], 1);

        let list_after_response = router
            .oneshot(
                Request::builder()
                    .uri(format!("/api/memory/archive/threads/{thread_id}/messages"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(list_after_response.status(), StatusCode::OK);
        let list_after_body = to_bytes(list_after_response.into_body(), usize::MAX)
            .await
            .unwrap();
        let list_after_payload: Value = serde_json::from_slice(&list_after_body).unwrap();
        assert_eq!(list_after_payload["messages"].as_array().unwrap().len(), 2);
        assert_eq!(list_after_payload["messages"][0]["content"], "third");
        assert_eq!(list_after_payload["messages"][1]["content"], "second");
    }

    #[tokio::test]
    async fn updates_default_memory_threads_with_patch_route() {
        let server = MastraServer::new(RuntimeRegistry::new());
        server.register_memory("default", Arc::new(TestMemory::default()));
        let router = server.into_router();

        let create_response = router
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/memory/threads")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "resourceId": "resource-before",
                            "title": "Before",
                            "metadata": { "scope": "before" }
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(create_response.status(), StatusCode::OK);

        let create_body = to_bytes(create_response.into_body(), usize::MAX)
            .await
            .unwrap();
        let create_payload: Value = serde_json::from_slice(&create_body).unwrap();
        let thread_id = create_payload["thread"]["id"].as_str().unwrap().to_owned();
        let created_at = create_payload["thread"]["created_at"].clone();

        let update_response = router
            .clone()
            .oneshot(
                Request::builder()
                    .method("PATCH")
                    .uri(format!("/api/memory/threads/{thread_id}"))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "resourceId": "resource-after",
                            "title": "After",
                            "metadata": { "scope": "after" }
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(update_response.status(), StatusCode::OK);
        let update_body = to_bytes(update_response.into_body(), usize::MAX)
            .await
            .unwrap();
        let update_payload: Value = serde_json::from_slice(&update_body).unwrap();
        assert_eq!(update_payload["thread"]["id"], thread_id);
        assert_eq!(update_payload["thread"]["resource_id"], "resource-after");
        assert_eq!(update_payload["thread"]["title"], "After");
        assert_eq!(update_payload["thread"]["metadata"]["scope"], "after");
        assert_eq!(update_payload["thread"]["created_at"], created_at);
        assert!(update_payload["thread"]["updated_at"].is_string());
    }

    #[tokio::test]
    async fn exposes_working_memory_and_observation_routes_for_default_memory() {
        let server = MastraServer::new(RuntimeRegistry::new());
        server.register_memory("default", Arc::new(TestMemory::default()));
        let router = server.into_router();

        let create_response = router
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/memory/threads")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "resourceId": "resource-memory",
                            "title": "Memory thread",
                            "metadata": {}
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        let create_body = to_bytes(create_response.into_body(), usize::MAX)
            .await
            .unwrap();
        let create_payload: Value = serde_json::from_slice(&create_body).unwrap();
        let thread_id = create_payload["thread"]["id"].as_str().unwrap().to_owned();

        let update_working_memory = router
            .clone()
            .oneshot(
                Request::builder()
                    .method("PUT")
                    .uri(format!("/api/memory/threads/{thread_id}/working-memory"))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "resourceId": "resource-memory",
                            "scope": "thread",
                            "format": "json",
                            "template": "# User Profile",
                            "content": { "name": "Sam" }
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(update_working_memory.status(), StatusCode::OK);
        let update_body = to_bytes(update_working_memory.into_body(), usize::MAX)
            .await
            .unwrap();
        let update_payload: Value = serde_json::from_slice(&update_body).unwrap();
        assert_eq!(update_payload["working_memory"]["content"]["name"], "Sam");
        assert_eq!(update_payload["working_memory"]["format"], "json");

        let fetch_working_memory = router
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/memory/threads/{thread_id}/working-memory"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(fetch_working_memory.status(), StatusCode::OK);
        let fetch_body = to_bytes(fetch_working_memory.into_body(), usize::MAX)
            .await
            .unwrap();
        let fetch_payload: Value = serde_json::from_slice(&fetch_body).unwrap();
        assert_eq!(fetch_payload["working_memory"]["template"], "# User Profile");

        let append_observation = router
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/memory/threads/{thread_id}/observations"))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "resourceId": "resource-memory",
                            "scope": "thread",
                            "content": "User likes Rust",
                            "observedMessageIds": ["00000000-0000-7000-8000-000000000001"],
                            "metadata": { "kind": "summary" }
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(append_observation.status(), StatusCode::OK);

        let list_observations = router
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/api/memory/threads/{thread_id}/observations?resourceId=resource-memory&perPage=false"
                    ))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(list_observations.status(), StatusCode::OK);
        let list_body = to_bytes(list_observations.into_body(), usize::MAX)
            .await
            .unwrap();
        let list_payload: Value = serde_json::from_slice(&list_body).unwrap();
        assert_eq!(list_payload["observations"].as_array().unwrap().len(), 1);
        assert_eq!(list_payload["observations"][0]["content"], "User likes Rust");
        assert_eq!(list_payload["per_page"], false);
    }

    #[tokio::test]
    async fn lists_memory_threads_with_official_pagination_shape() {
        let server = MastraServer::new(RuntimeRegistry::new());
        server.register_memory("default", Arc::new(TestMemory::default()));
        let router = server.into_router();

        for title in ["Thread A", "Thread B"] {
            let response = router
                .clone()
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri("/api/memory/threads")
                        .header("content-type", "application/json")
                        .body(Body::from(
                            json!({
                                "resourceId": "resource-page",
                                "title": title,
                            })
                            .to_string(),
                        ))
                        .unwrap(),
                )
                .await
                .unwrap();

            assert_eq!(response.status(), StatusCode::OK);
        }

        let list_response = router
            .oneshot(
                Request::builder()
                    .uri("/api/memory/threads?resourceId=resource-page&page=0&perPage=1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(list_response.status(), StatusCode::OK);
        let list_body = to_bytes(list_response.into_body(), usize::MAX)
            .await
            .unwrap();
        let list_payload: Value = serde_json::from_slice(&list_body).unwrap();
        assert_eq!(list_payload["threads"].as_array().unwrap().len(), 1);
        assert_eq!(list_payload["total"], 2);
        assert_eq!(list_payload["page"], 0);
        assert_eq!(list_payload["per_page"], 1);
        assert_eq!(list_payload["has_more"], true);
    }

    #[tokio::test]
    async fn filters_memory_threads_by_metadata_query() {
        let server = MastraServer::new(RuntimeRegistry::new());
        server.register_memory("default", Arc::new(TestMemory::default()));
        let router = server.into_router();

        for (title, scope) in [("Keep", "keep"), ("Skip", "skip")] {
            let response = router
                .clone()
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri("/api/memory/threads")
                        .header("content-type", "application/json")
                        .body(Body::from(
                            json!({
                                "resourceId": "resource-metadata",
                                "title": title,
                                "metadata": { "scope": scope }
                            })
                            .to_string(),
                        ))
                        .unwrap(),
                )
                .await
                .unwrap();

            assert_eq!(response.status(), StatusCode::OK);
        }

        let list_response = router
            .oneshot(
                Request::builder()
                    .uri("/api/memory/threads?resourceId=resource-metadata&metadata=%7B%22scope%22%3A%22keep%22%7D")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(list_response.status(), StatusCode::OK);
        let list_body = to_bytes(list_response.into_body(), usize::MAX)
            .await
            .unwrap();
        let list_payload: Value = serde_json::from_slice(&list_body).unwrap();
        assert_eq!(list_payload["threads"].as_array().unwrap().len(), 1);
        assert_eq!(list_payload["threads"][0]["title"], "Keep");
        assert_eq!(list_payload["total"], 1);
    }

    #[tokio::test]
    async fn lists_memory_messages_with_pagination_shape() {
        let server = MastraServer::new(RuntimeRegistry::new());
        server.register_memory("default", Arc::new(TestMemory::default()));
        let router = server.into_router();

        let create_response = router
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/memory/threads")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "resourceId": "resource-message-page",
                            "title": "Message pagination",
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        let create_body = to_bytes(create_response.into_body(), usize::MAX)
            .await
            .unwrap();
        let create_payload: Value = serde_json::from_slice(&create_body).unwrap();
        let thread_id = create_payload["thread"]["id"].as_str().unwrap().to_owned();

        let append_response = router
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/memory/threads/{thread_id}/messages"))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "messages": [
                                { "role": "user", "content": "first" },
                                { "role": "assistant", "content": "second" }
                            ]
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(append_response.status(), StatusCode::OK);

        let list_response = router
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/api/memory/threads/{thread_id}/messages?page=1&perPage=1"
                    ))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(list_response.status(), StatusCode::OK);
        let list_body = to_bytes(list_response.into_body(), usize::MAX)
            .await
            .unwrap();
        let list_payload: Value = serde_json::from_slice(&list_body).unwrap();
        assert_eq!(list_payload["messages"].as_array().unwrap().len(), 1);
        assert_eq!(list_payload["messages"][0]["content"], "first");
        assert_eq!(list_payload["total"], 2);
        assert_eq!(list_payload["page"], 1);
        assert_eq!(list_payload["per_page"], 1);
        assert_eq!(list_payload["has_more"], false);
    }

    #[tokio::test]
    async fn lists_memory_threads_with_ordering_and_per_page_false() {
        let server = MastraServer::new(RuntimeRegistry::new());
        server.register_memory("default", Arc::new(TestMemory::default()));
        let router = server.into_router();

        let first_create = router
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/memory/threads")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "resourceId": "resource-order",
                            "title": "Older but updated later",
                            "metadata": { "scope": "ordered" }
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        let first_body = to_bytes(first_create.into_body(), usize::MAX)
            .await
            .unwrap();
        let first_payload: Value = serde_json::from_slice(&first_body).unwrap();
        let first_thread_id = first_payload["thread"]["id"].as_str().unwrap().to_owned();

        let second_create = router
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/memory/threads")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "resourceId": "resource-order",
                            "title": "Newer but not updated",
                            "metadata": { "scope": "ordered" }
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(second_create.status(), StatusCode::OK);

        let updated = router
            .clone()
            .oneshot(
                Request::builder()
                    .method("PATCH")
                    .uri(format!("/api/memory/threads/{first_thread_id}"))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "title": "Older but updated later",
                            "metadata": { "scope": "ordered", "status": "updated" }
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(updated.status(), StatusCode::OK);

        let ordered = router
            .oneshot(
                Request::builder()
                    .uri("/api/memory/threads?resourceId=resource-order&metadata=%7B%22scope%22%3A%22ordered%22%7D&perPage=false&orderBy=%7B%22field%22%3A%22updatedAt%22%2C%22direction%22%3A%22ASC%22%7D")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(ordered.status(), StatusCode::OK);

        let ordered_body = to_bytes(ordered.into_body(), usize::MAX).await.unwrap();
        let ordered_payload: Value = serde_json::from_slice(&ordered_body).unwrap();
        let titles = ordered_payload["threads"]
            .as_array()
            .unwrap()
            .iter()
            .map(|thread| thread["title"].as_str().unwrap().to_owned())
            .collect::<Vec<_>>();
        assert_eq!(
            titles,
            vec![
                "Newer but not updated".to_owned(),
                "Older but updated later".to_owned()
            ]
        );
        assert_eq!(ordered_payload["per_page"], false);
        assert_eq!(ordered_payload["total"], 2);
    }

    #[tokio::test]
    async fn lists_memory_messages_with_ordering_and_per_page_false() {
        let server = MastraServer::new(RuntimeRegistry::new());
        server.register_memory("default", Arc::new(TestMemory::default()));
        let router = server.into_router();

        let create_response = router
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/memory/threads")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "resourceId": "resource-message-order",
                            "title": "Message ordering",
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        let create_body = to_bytes(create_response.into_body(), usize::MAX)
            .await
            .unwrap();
        let create_payload: Value = serde_json::from_slice(&create_body).unwrap();
        let thread_id = create_payload["thread"]["id"].as_str().unwrap().to_owned();

        let append_response = router
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/memory/threads/{thread_id}/messages"))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "messages": [
                                { "role": "user", "content": "first" },
                                { "role": "assistant", "content": "second" }
                            ]
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(append_response.status(), StatusCode::OK);

        let ordered = router
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/api/memory/threads/{thread_id}/messages?perPage=false&orderBy=%7B%22field%22%3A%22createdAt%22%2C%22direction%22%3A%22ASC%22%7D"
                    ))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(ordered.status(), StatusCode::OK);

        let ordered_body = to_bytes(ordered.into_body(), usize::MAX).await.unwrap();
        let ordered_payload: Value = serde_json::from_slice(&ordered_body).unwrap();
        let contents = ordered_payload["messages"]
            .as_array()
            .unwrap()
            .iter()
            .map(|message| message["content"].as_str().unwrap().to_owned())
            .collect::<Vec<_>>();
        assert_eq!(contents, vec!["first".to_owned(), "second".to_owned()]);
        assert_eq!(ordered_payload["per_page"], false);
        assert_eq!(ordered_payload["total"], 2);
    }

    #[tokio::test]
    async fn filters_memory_messages_by_resource_and_message_ids() {
        let memory = Arc::new(TestMemory::default());
        let thread = memory
            .create_thread(CreateThreadRequest {
                id: None,
                resource_id: Some("resource-message-filter".to_owned()),
                title: Some("Message filter thread".to_owned()),
                metadata: json!({}),
            })
            .await
            .unwrap();
        let first_id = Uuid::now_v7().to_string();
        let second_id = Uuid::now_v7().to_string();
        memory
            .append_messages(
                &thread.id,
                vec![
                    MemoryMessage {
                        id: first_id.clone(),
                        thread_id: thread.id.clone(),
                        role: MemoryRole::User,
                        content: "first".to_owned(),
                        created_at: Utc::now(),
                        metadata: json!({}),
                    },
                    MemoryMessage {
                        id: second_id.clone(),
                        thread_id: thread.id.clone(),
                        role: MemoryRole::Assistant,
                        content: "second".to_owned(),
                        created_at: Utc::now(),
                        metadata: json!({}),
                    },
                ],
            )
            .await
            .unwrap();

        let response = list_memory_messages_for(
            memory,
            thread.id.clone(),
            crate::contracts::ListMessagesQuery {
                page: None,
                per_page: None,
                resource_id: Some("resource-message-filter".to_owned()),
                message_ids: Some(vec![second_id.clone()]),
                start_date: None,
                end_date: None,
                order_by: None,
            },
        )
        .await
        .unwrap()
        .0;

        assert_eq!(response.total, 1);
        assert_eq!(response.messages.len(), 1);
        assert_eq!(response.messages[0].id, second_id);
        assert_eq!(response.messages[0].content, "second");
    }

    #[tokio::test]
    async fn exposes_system_packages_route() {
        let response = build_router()
            .oneshot(
                Request::builder()
                    .uri("/api/system/packages")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let payload: Value = serde_json::from_slice(&body).unwrap();
        assert!(payload["packages"].is_array());
        assert_eq!(payload["is_dev"], false);
        assert_eq!(payload["cms_enabled"], false);
    }

    #[tokio::test]
    async fn lists_workflow_runs_after_starting_a_workflow_with_official_field_names() {
        let registry = RuntimeRegistry::new();
        registry.register_workflow(JsonWorkflow);
        let router = MastraServer::new(registry).into_router();

        let start_response = router
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/workflows/demo/start-async")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "resourceId": "resource-9",
                            "inputData": { "topic": "rust" },
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(start_response.status(), StatusCode::OK);

        let list_response = router
            .oneshot(
                Request::builder()
                    .uri("/api/workflows/demo/runs")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(list_response.status(), StatusCode::OK);
        let list_body = to_bytes(list_response.into_body(), usize::MAX)
            .await
            .unwrap();
        let list_payload: Value = serde_json::from_slice(&list_body).unwrap();
        assert_eq!(list_payload["runs"].as_array().unwrap().len(), 1);
        assert_eq!(list_payload["total"], 1);
        assert_eq!(list_payload["runs"][0]["workflow_id"], "demo");
        assert_eq!(list_payload["runs"][0]["resource_id"], "resource-9");
        assert_eq!(list_payload["runs"][0]["status"], "success");
    }

    #[tokio::test]
    async fn filters_workflow_runs_with_official_query_parameters() {
        let registry = RuntimeRegistry::new();
        registry.register_workflow(JsonWorkflow);
        let router = MastraServer::new(registry).into_router();

        let started = router
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/workflows/demo/start-async")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "resourceId": "resource-success",
                            "inputData": { "topic": "rust" },
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(started.status(), StatusCode::OK);

        let cutoff = Utc::now();
        sleep(Duration::from_millis(5)).await;

        let created = router
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/workflows/demo/create-run")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "resourceId": "resource-created",
                            "inputData": { "topic": "draft" },
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(created.status(), StatusCode::CREATED);

        let filtered = router
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/api/workflows/demo/runs?status=created&resourceId=resource-created&fromDate={}&page=0&perPage=10",
                        cutoff.to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
                    ))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(filtered.status(), StatusCode::OK);

        let filtered_body = to_bytes(filtered.into_body(), usize::MAX).await.unwrap();
        let filtered_payload: Value = serde_json::from_slice(&filtered_body).unwrap();
        assert_eq!(filtered_payload["total"], 1);
        assert_eq!(filtered_payload["runs"].as_array().unwrap().len(), 1);
        assert_eq!(
            filtered_payload["runs"][0]["resource_id"],
            "resource-created"
        );
        assert_eq!(filtered_payload["runs"][0]["status"], "created");
        assert!(filtered_payload["runs"][0]["created_at"].is_string());
        assert!(filtered_payload["runs"][0]["updated_at"].is_string());

        let paged = router
            .oneshot(
                Request::builder()
                    .uri("/api/workflows/demo/runs?page=1&perPage=1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(paged.status(), StatusCode::OK);

        let paged_body = to_bytes(paged.into_body(), usize::MAX).await.unwrap();
        let paged_payload: Value = serde_json::from_slice(&paged_body).unwrap();
        assert_eq!(paged_payload["total"], 2);
        assert_eq!(paged_payload["runs"].as_array().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn creates_and_deletes_workflow_runs_with_explicit_run_id() {
        let registry = RuntimeRegistry::new();
        registry.register_workflow(JsonWorkflow);
        let router = MastraServer::new(registry).into_router();
        let run_id = "018f7f26-8b7e-7c9d-b145-2c3d4e5f6790";

        let created = router
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/workflows/demo/create-run")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "runId": run_id,
                            "resourceId": "resource-explicit",
                            "inputData": { "topic": "custom" },
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(created.status(), StatusCode::CREATED);

        let created_body = to_bytes(created.into_body(), usize::MAX).await.unwrap();
        let created_payload: Value = serde_json::from_slice(&created_body).unwrap();
        assert_eq!(created_payload["run_id"], run_id);
        assert_eq!(created_payload["status"], "created");

        let deleted = router
            .clone()
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri(format!("/api/workflows/demo/runs/{run_id}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(deleted.status(), StatusCode::OK);

        let deleted_body = to_bytes(deleted.into_body(), usize::MAX).await.unwrap();
        let deleted_payload: Value = serde_json::from_slice(&deleted_body).unwrap();
        assert_eq!(deleted_payload["message"], "Workflow run deleted");

        let missing = router
            .oneshot(
                Request::builder()
                    .uri(format!("/api/workflows/demo/runs/{run_id}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(missing.status(), StatusCode::NOT_FOUND);
    }
}
