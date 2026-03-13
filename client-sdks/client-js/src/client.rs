use std::{sync::Arc, time::Duration};

use async_stream::try_stream;
use futures::{Stream, StreamExt};
use reqwest::{
    Method, Response, StatusCode,
    header::{AUTHORIZATION, HeaderMap, HeaderName, HeaderValue},
};
use serde::Serialize;
use serde::de::DeserializeOwned;
use url::Url;
use uuid::Uuid;

use crate::{
    MastraClientError,
    types::{
        AgentDetailResponse, AppendMemoryMessagesRequest, AppendMemoryMessagesResponse,
        CancelWorkflowRunResponse, CreateMemoryThreadRequest, CreateMemoryThreadResponse,
        CreateWorkflowRunRequest, DeleteMemoryMessagesRequest, DeleteMemoryMessagesResponse,
        DeleteWorkflowRunResponse, ErrorResponse, ExecuteToolRequest, ExecuteToolResponse,
        GenerateRequest, GenerateResponse, GenerateStreamEvent, GetMemoryThreadResponse,
        ListAgentsResponse, ListMemoriesResponse, ListMemoryMessagesResponse, ListMessagesQuery,
        ListThreadsQuery, ListThreadsResponse, ListToolsResponse, ListWorkflowRunsQuery,
        ListWorkflowRunsResponse, ListWorkflowsResponse, MessageOrderBy, PaginationSizeValue,
        ResumeWorkflowRunRequest, ResumeWorkflowRunResponse, StartWorkflowRunRequest,
        StartWorkflowRunResponse, SystemPackagesResponse, ThreadOrderBy, ToolSummary,
        UpdateMemoryThreadRequest, WorkflowDetailResponse, WorkflowRunRecord, WorkflowStreamEvent,
    },
};

trait StreamProtocolEvent: DeserializeOwned {
    fn error_message(&self) -> Option<String>;
}

impl StreamProtocolEvent for GenerateStreamEvent {
    fn error_message(&self) -> Option<String> {
        match self {
            Self::Error(error) => Some(error.error.clone()),
            _ => None,
        }
    }
}

impl StreamProtocolEvent for WorkflowStreamEvent {
    fn error_message(&self) -> Option<String> {
        match self {
            Self::Error(error) => Some(error.error.clone()),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct MastraClient {
    inner: Arc<ClientInner>,
}

#[derive(Debug)]
struct ClientInner {
    http: reqwest::Client,
    base_url: Url,
    api_prefix: String,
}

#[derive(Debug)]
pub struct MastraClientBuilder {
    base_url: String,
    api_prefix: String,
    timeout: Duration,
    connect_timeout: Option<Duration>,
    default_headers: HeaderMap,
    bearer_token: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AgentsClient {
    inner: Arc<ClientInner>,
}

#[derive(Debug, Clone)]
pub struct AgentClient {
    inner: Arc<ClientInner>,
    agent_id: String,
}

#[derive(Debug, Clone)]
pub struct ToolsClient {
    inner: Arc<ClientInner>,
}

#[derive(Debug, Clone)]
pub struct ToolClient {
    inner: Arc<ClientInner>,
    tool_id: String,
}

#[derive(Debug, Clone)]
pub struct WorkflowsClient {
    inner: Arc<ClientInner>,
}

#[derive(Debug, Clone)]
pub struct WorkflowClient {
    inner: Arc<ClientInner>,
    workflow_id: String,
}

#[derive(Debug, Clone)]
pub struct MemoriesClient {
    inner: Arc<ClientInner>,
}

#[derive(Debug, Clone)]
pub struct MemoryClient {
    inner: Arc<ClientInner>,
    memory_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct MemoryThreadClient {
    inner: Arc<ClientInner>,
    memory_id: Option<String>,
    thread_id: String,
}

impl MastraClientBuilder {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            api_prefix: "/api".to_owned(),
            timeout: Duration::from_secs(30),
            connect_timeout: None,
            default_headers: HeaderMap::new(),
            bearer_token: None,
        }
    }

    pub fn api_prefix(mut self, api_prefix: impl Into<String>) -> Self {
        self.api_prefix = normalize_api_prefix(&api_prefix.into());
        self
    }

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn connect_timeout(mut self, timeout: Duration) -> Self {
        self.connect_timeout = Some(timeout);
        self
    }

    pub fn default_header(mut self, name: HeaderName, value: HeaderValue) -> Self {
        self.default_headers.insert(name, value);
        self
    }

    pub fn bearer_auth(mut self, token: impl Into<String>) -> Self {
        self.bearer_token = Some(token.into());
        self
    }

    pub fn build(mut self) -> Result<MastraClient, MastraClientError> {
        if let Some(token) = self.bearer_token.take() {
            let value = HeaderValue::from_str(&format!("Bearer {token}")).map_err(|error| {
                MastraClientError::Api {
                    status: StatusCode::BAD_REQUEST,
                    body: error.to_string(),
                    error: None,
                }
            })?;
            self.default_headers.insert(AUTHORIZATION, value);
        }

        let mut base_url = Url::parse(&self.base_url)?;
        if !base_url.path().ends_with('/') {
            let normalized = if base_url.path().is_empty() {
                "/".to_owned()
            } else {
                format!("{}/", base_url.path().trim_end_matches('/'))
            };
            base_url.set_path(&normalized);
        }

        let mut builder = reqwest::Client::builder()
            .default_headers(self.default_headers)
            .timeout(self.timeout);
        if let Some(connect_timeout) = self.connect_timeout {
            builder = builder.connect_timeout(connect_timeout);
        }

        let http = builder.build().map_err(MastraClientError::Build)?;

        Ok(MastraClient {
            inner: Arc::new(ClientInner {
                http,
                base_url,
                api_prefix: self.api_prefix,
            }),
        })
    }
}

impl MastraClient {
    pub fn builder(base_url: impl Into<String>) -> MastraClientBuilder {
        MastraClientBuilder::new(base_url)
    }

    pub fn new(base_url: impl Into<String>) -> Result<Self, MastraClientError> {
        Self::builder(base_url).build()
    }

    pub fn base_url(&self) -> &Url {
        &self.inner.base_url
    }

    pub fn api_prefix(&self) -> &str {
        &self.inner.api_prefix
    }

    pub fn agents(&self) -> AgentsClient {
        AgentsClient {
            inner: Arc::clone(&self.inner),
        }
    }

    pub fn agent(&self, agent_id: impl Into<String>) -> AgentClient {
        AgentClient {
            inner: Arc::clone(&self.inner),
            agent_id: agent_id.into(),
        }
    }

    pub fn get_agent(&self, agent_id: impl Into<String>) -> AgentClient {
        self.agent(agent_id)
    }

    pub fn tools(&self) -> ToolsClient {
        ToolsClient {
            inner: Arc::clone(&self.inner),
        }
    }

    pub fn tool(&self, tool_id: impl Into<String>) -> ToolClient {
        ToolClient {
            inner: Arc::clone(&self.inner),
            tool_id: tool_id.into(),
        }
    }

    pub fn get_tool(&self, tool_id: impl Into<String>) -> ToolClient {
        self.tool(tool_id)
    }

    pub fn workflows(&self) -> WorkflowsClient {
        WorkflowsClient {
            inner: Arc::clone(&self.inner),
        }
    }

    pub fn workflow(&self, workflow_id: impl Into<String>) -> WorkflowClient {
        WorkflowClient {
            inner: Arc::clone(&self.inner),
            workflow_id: workflow_id.into(),
        }
    }

    pub fn get_workflow(&self, workflow_id: impl Into<String>) -> WorkflowClient {
        self.workflow(workflow_id)
    }

    pub fn memories(&self) -> MemoriesClient {
        MemoriesClient {
            inner: Arc::clone(&self.inner),
        }
    }

    pub fn memory(&self, memory_id: impl Into<String>) -> MemoryClient {
        MemoryClient {
            inner: Arc::clone(&self.inner),
            memory_id: Some(memory_id.into()),
        }
    }

    pub fn get_memory(&self, memory_id: impl Into<String>) -> MemoryClient {
        self.memory(memory_id)
    }

    pub fn default_memory(&self) -> MemoryClient {
        MemoryClient {
            inner: Arc::clone(&self.inner),
            memory_id: None,
        }
    }

    pub async fn list_agents(&self) -> Result<ListAgentsResponse, MastraClientError> {
        self.agents().list().await
    }

    pub async fn list_workflows(&self) -> Result<ListWorkflowsResponse, MastraClientError> {
        self.workflows().list().await
    }

    pub async fn list_memories(&self) -> Result<ListMemoriesResponse, MastraClientError> {
        self.memories().list().await
    }

    pub async fn list_memory_threads(
        &self,
        query: ListThreadsQuery,
    ) -> Result<ListThreadsResponse, MastraClientError> {
        self.default_memory().threads_with_query(query).await
    }

    pub async fn create_memory_thread(
        &self,
        request: CreateMemoryThreadRequest,
    ) -> Result<CreateMemoryThreadResponse, MastraClientError> {
        self.default_memory().create_thread(request).await
    }

    pub fn get_memory_thread(&self, thread_id: impl Into<String>) -> MemoryThreadClient {
        self.default_memory().thread(thread_id)
    }

    pub async fn system_packages(&self) -> Result<SystemPackagesResponse, MastraClientError> {
        self.inner
            .request(Method::GET, "/system/packages", Option::<&()>::None)
            .await
    }
}

impl AgentsClient {
    pub async fn list(&self) -> Result<ListAgentsResponse, MastraClientError> {
        self.inner
            .request(Method::GET, "/agents", Option::<&()>::None)
            .await
    }
}

impl AgentClient {
    pub async fn details(&self) -> Result<AgentDetailResponse, MastraClientError> {
        self.inner
            .request(
                Method::GET,
                &format!("/agents/{}", self.agent_id),
                Option::<&()>::None,
            )
            .await
    }

    pub async fn tools(&self) -> Result<ListToolsResponse, MastraClientError> {
        self.inner
            .request(
                Method::GET,
                &format!("/agents/{}/tools", self.agent_id),
                Option::<&()>::None,
            )
            .await
    }

    pub async fn execute_tool(
        &self,
        tool_id: &str,
        request: ExecuteToolRequest,
    ) -> Result<ExecuteToolResponse, MastraClientError> {
        self.inner
            .request(
                Method::POST,
                &format!("/agents/{}/tools/{tool_id}/execute", self.agent_id),
                Some(&request),
            )
            .await
    }

    pub async fn generate(
        &self,
        request: GenerateRequest,
    ) -> Result<GenerateResponse, MastraClientError> {
        self.inner
            .request(
                Method::POST,
                &format!("/agents/{}/generate", self.agent_id),
                Some(&request),
            )
            .await
    }

    pub async fn stream(
        &self,
        request: GenerateRequest,
    ) -> Result<
        impl Stream<Item = Result<GenerateStreamEvent, MastraClientError>> + Send + 'static,
        MastraClientError,
    > {
        let response = self
            .inner
            .stream_request(
                Method::POST,
                &format!("/agents/{}/stream", self.agent_id),
                Some(&request),
            )
            .await?;
        Ok(decode_event_stream(response))
    }

    pub async fn generate_text(
        &self,
        prompt: impl Into<String>,
    ) -> Result<GenerateResponse, MastraClientError> {
        self.generate(GenerateRequest {
            messages: crate::AgentMessages::Text(prompt.into()),
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
        .await
    }
}

impl ToolsClient {
    pub async fn list(&self) -> Result<ListToolsResponse, MastraClientError> {
        self.inner
            .request(Method::GET, "/tools", Option::<&()>::None)
            .await
    }
}

impl ToolClient {
    pub async fn details(&self) -> Result<ToolSummary, MastraClientError> {
        self.inner
            .request(
                Method::GET,
                &format!("/tools/{}", self.tool_id),
                Option::<&()>::None,
            )
            .await
    }

    pub async fn execute(
        &self,
        request: ExecuteToolRequest,
    ) -> Result<ExecuteToolResponse, MastraClientError> {
        self.inner
            .request(
                Method::POST,
                &format!("/tools/{}/execute", self.tool_id),
                Some(&request),
            )
            .await
    }
}

impl WorkflowsClient {
    pub async fn list(&self) -> Result<ListWorkflowsResponse, MastraClientError> {
        self.inner
            .request(Method::GET, "/workflows", Option::<&()>::None)
            .await
    }
}

impl WorkflowClient {
    pub async fn details(&self) -> Result<WorkflowDetailResponse, MastraClientError> {
        self.inner
            .request(
                Method::GET,
                &format!("/workflows/{}", self.workflow_id),
                Option::<&()>::None,
            )
            .await
    }

    pub async fn create_run(
        &self,
        request: CreateWorkflowRunRequest,
    ) -> Result<WorkflowRunRecord, MastraClientError> {
        self.inner
            .request(
                Method::POST,
                &format!("/workflows/{}/create-run", self.workflow_id),
                Some(&request),
            )
            .await
    }

    pub async fn start_async(
        &self,
        request: StartWorkflowRunRequest,
    ) -> Result<StartWorkflowRunResponse, MastraClientError> {
        self.inner
            .request(
                Method::POST,
                &format!("/workflows/{}/start-async", self.workflow_id),
                Some(&request),
            )
            .await
    }

    pub async fn resume_async(
        &self,
        request: ResumeWorkflowRunRequest,
    ) -> Result<StartWorkflowRunResponse, MastraClientError> {
        self.inner
            .request(
                Method::POST,
                &format!("/workflows/{}/resume-async", self.workflow_id),
                Some(&request),
            )
            .await
    }

    pub async fn resume(
        &self,
        request: ResumeWorkflowRunRequest,
    ) -> Result<ResumeWorkflowRunResponse, MastraClientError> {
        self.inner
            .request(
                Method::POST,
                &format!("/workflows/{}/resume", self.workflow_id),
                Some(&request),
            )
            .await
    }

    pub async fn run(
        &self,
        run_id: impl std::fmt::Display,
    ) -> Result<WorkflowRunRecord, MastraClientError> {
        self.inner
            .request(
                Method::GET,
                &format!("/workflows/{}/runs/{}", self.workflow_id, run_id),
                Option::<&()>::None,
            )
            .await
    }

    pub async fn run_by_id(
        &self,
        run_id: impl std::fmt::Display,
    ) -> Result<WorkflowRunRecord, MastraClientError> {
        self.run(run_id).await
    }

    pub async fn runs(&self) -> Result<ListWorkflowRunsResponse, MastraClientError> {
        self.runs_with_query(ListWorkflowRunsQuery::default()).await
    }

    pub async fn runs_with_query(
        &self,
        query: ListWorkflowRunsQuery,
    ) -> Result<ListWorkflowRunsResponse, MastraClientError> {
        self.inner
            .request_with_query(
                Method::GET,
                &format!("/workflows/{}/runs", self.workflow_id),
                Some(&query),
                Option::<&()>::None,
            )
            .await
    }

    pub async fn delete_run_by_id(
        &self,
        run_id: impl std::fmt::Display,
    ) -> Result<DeleteWorkflowRunResponse, MastraClientError> {
        self.inner
            .request(
                Method::DELETE,
                &format!("/workflows/{}/runs/{}", self.workflow_id, run_id),
                Option::<&()>::None,
            )
            .await
    }

    pub async fn cancel_run_by_id(
        &self,
        run_id: impl std::fmt::Display,
    ) -> Result<CancelWorkflowRunResponse, MastraClientError> {
        self.inner
            .request(
                Method::POST,
                &format!("/workflows/{}/runs/{}/cancel", self.workflow_id, run_id),
                Option::<&()>::None,
            )
            .await
    }

    pub async fn stream(
        &self,
        request: StartWorkflowRunRequest,
    ) -> Result<
        impl Stream<Item = Result<WorkflowStreamEvent, MastraClientError>> + Send + 'static,
        MastraClientError,
    > {
        let run_id = Uuid::now_v7().to_string();
        self.stream_internal(Some(run_id), request).await
    }

    pub async fn stream_with_run_id(
        &self,
        run_id: &str,
        request: StartWorkflowRunRequest,
    ) -> Result<
        impl Stream<Item = Result<WorkflowStreamEvent, MastraClientError>> + Send + 'static,
        MastraClientError,
    > {
        self.stream_internal(Some(run_id.to_owned()), request).await
    }

    pub async fn resume_stream(
        &self,
        request: ResumeWorkflowRunRequest,
    ) -> Result<
        impl Stream<Item = Result<WorkflowStreamEvent, MastraClientError>> + Send + 'static,
        MastraClientError,
    > {
        let response = self
            .inner
            .stream_request(
                Method::POST,
                &format!("/workflows/{}/resume-stream", self.workflow_id),
                Some(&request),
            )
            .await?;
        Ok(decode_event_stream(response))
    }

    async fn stream_internal(
        &self,
        run_id: Option<String>,
        request: StartWorkflowRunRequest,
    ) -> Result<
        impl Stream<Item = Result<WorkflowStreamEvent, MastraClientError>> + Send + 'static,
        MastraClientError,
    > {
        let query = run_id.map(|run_id| [("runId", run_id)]);
        let response = self
            .inner
            .stream_request_with_query(
                Method::POST,
                &format!("/workflows/{}/stream", self.workflow_id),
                query.as_ref(),
                Some(&request),
            )
            .await?;
        Ok(decode_event_stream(response))
    }
}

impl MemoriesClient {
    pub async fn list(&self) -> Result<ListMemoriesResponse, MastraClientError> {
        self.inner
            .request(Method::GET, "/memories", Option::<&()>::None)
            .await
    }
}

impl MemoryClient {
    pub fn thread(&self, thread_id: impl Into<String>) -> MemoryThreadClient {
        MemoryThreadClient {
            inner: Arc::clone(&self.inner),
            memory_id: self.memory_id.clone(),
            thread_id: thread_id.into(),
        }
    }

    pub async fn threads(&self) -> Result<ListThreadsResponse, MastraClientError> {
        self.threads_with_query(ListThreadsQuery::default()).await
    }

    pub async fn threads_with_query(
        &self,
        query: ListThreadsQuery,
    ) -> Result<ListThreadsResponse, MastraClientError> {
        let query = ThreadQueryWire::try_from(query)?;
        self.inner
            .request_with_query(
                Method::GET,
                &self.threads_path(),
                Some(&query),
                Option::<&()>::None,
            )
            .await
    }

    pub async fn create_thread(
        &self,
        request: CreateMemoryThreadRequest,
    ) -> Result<CreateMemoryThreadResponse, MastraClientError> {
        self.inner
            .request(Method::POST, &self.threads_path(), Some(&request))
            .await
    }

    pub async fn update_thread(
        &self,
        thread_id: &str,
        request: UpdateMemoryThreadRequest,
    ) -> Result<mastra_core::Thread, MastraClientError> {
        self.thread(thread_id).update(request).await
    }

    pub async fn append_messages(
        &self,
        thread_id: &str,
        request: AppendMemoryMessagesRequest,
    ) -> Result<AppendMemoryMessagesResponse, MastraClientError> {
        self.thread(thread_id).append_messages(request).await
    }

    pub async fn messages(
        &self,
        thread_id: &str,
    ) -> Result<ListMemoryMessagesResponse, MastraClientError> {
        self.messages_with_query(thread_id, ListMessagesQuery::default())
            .await
    }

    pub async fn messages_with_query(
        &self,
        thread_id: &str,
        query: ListMessagesQuery,
    ) -> Result<ListMemoryMessagesResponse, MastraClientError> {
        self.thread(thread_id).messages_with_query(query).await
    }

    pub async fn clone_thread(
        &self,
        thread_id: &str,
        request: crate::CloneMemoryThreadRequest,
    ) -> Result<crate::CloneMemoryThreadResponse, MastraClientError> {
        self.thread(thread_id).clone(request).await
    }

    pub async fn delete_thread(&self, thread_id: &str) -> Result<(), MastraClientError> {
        self.thread(thread_id).delete().await
    }

    pub async fn delete_messages(
        &self,
        request: DeleteMemoryMessagesRequest,
    ) -> Result<DeleteMemoryMessagesResponse, MastraClientError> {
        self.inner
            .request(Method::POST, &self.delete_messages_path(), Some(&request))
            .await
    }

    fn base_path(&self) -> String {
        self.memory_id
            .as_ref()
            .map(|memory_id| format!("/memory/{memory_id}"))
            .unwrap_or_else(|| "/memory".to_owned())
    }

    fn threads_path(&self) -> String {
        format!("{}/threads", self.base_path())
    }

    fn delete_messages_path(&self) -> String {
        format!("{}/messages/delete", self.base_path())
    }
}

impl MemoryThreadClient {
    fn thread_path(&self) -> String {
        self.memory_id
            .as_ref()
            .map(|memory_id| format!("/memory/{memory_id}/threads/{}", self.thread_id))
            .unwrap_or_else(|| format!("/memory/threads/{}", self.thread_id))
    }

    fn messages_path(&self) -> String {
        format!("{}/messages", self.thread_path())
    }

    fn clone_path(&self) -> String {
        format!("{}/clone", self.thread_path())
    }

    fn delete_messages_path(&self) -> String {
        self.memory_id
            .as_ref()
            .map(|memory_id| format!("/memory/{memory_id}/messages/delete"))
            .unwrap_or_else(|| "/memory/messages/delete".to_owned())
    }

    pub async fn get(&self) -> Result<mastra_core::Thread, MastraClientError> {
        let response: GetMemoryThreadResponse = self
            .inner
            .request(Method::GET, &self.thread_path(), Option::<&()>::None)
            .await?;
        Ok(response.thread)
    }

    pub async fn update(
        &self,
        request: UpdateMemoryThreadRequest,
    ) -> Result<mastra_core::Thread, MastraClientError> {
        let response: GetMemoryThreadResponse = self
            .inner
            .request(Method::PATCH, &self.thread_path(), Some(&request))
            .await?;
        Ok(response.thread)
    }

    pub async fn append_messages(
        &self,
        request: AppendMemoryMessagesRequest,
    ) -> Result<AppendMemoryMessagesResponse, MastraClientError> {
        self.inner
            .request(Method::POST, &self.messages_path(), Some(&request))
            .await
    }

    pub async fn messages(&self) -> Result<ListMemoryMessagesResponse, MastraClientError> {
        self.messages_with_query(ListMessagesQuery::default()).await
    }

    pub async fn messages_with_query(
        &self,
        query: ListMessagesQuery,
    ) -> Result<ListMemoryMessagesResponse, MastraClientError> {
        let query = MessageQueryWire::try_from(query)?;
        self.inner
            .request_with_query(
                Method::GET,
                &self.messages_path(),
                Some(&query),
                Option::<&()>::None,
            )
            .await
    }

    pub async fn clone(
        &self,
        request: crate::CloneMemoryThreadRequest,
    ) -> Result<crate::CloneMemoryThreadResponse, MastraClientError> {
        self.inner
            .request(Method::POST, &self.clone_path(), Some(&request))
            .await
    }

    pub async fn delete(&self) -> Result<(), MastraClientError> {
        let response = self
            .inner
            .send(Method::DELETE, &self.thread_path(), None::<&()>)
            .await?;
        ensure_success(response).await?;
        Ok(())
    }

    pub async fn delete_messages(
        &self,
        request: DeleteMemoryMessagesRequest,
    ) -> Result<DeleteMemoryMessagesResponse, MastraClientError> {
        self.inner
            .request(Method::POST, &self.delete_messages_path(), Some(&request))
            .await
    }
}

impl ClientInner {
    async fn request<ResponseBody, RequestBody>(
        &self,
        method: Method,
        path: &str,
        body: Option<&RequestBody>,
    ) -> Result<ResponseBody, MastraClientError>
    where
        ResponseBody: DeserializeOwned,
        RequestBody: Serialize + ?Sized,
    {
        self.request_with_query(method, path, None::<&()>, body)
            .await
    }

    async fn request_with_query<ResponseBody, Query, RequestBody>(
        &self,
        method: Method,
        path: &str,
        query: Option<&Query>,
        body: Option<&RequestBody>,
    ) -> Result<ResponseBody, MastraClientError>
    where
        ResponseBody: DeserializeOwned,
        Query: Serialize + ?Sized,
        RequestBody: Serialize + ?Sized,
    {
        let response = self.send_with_query(method, path, query, body).await?;
        decode_response(response).await
    }

    async fn stream_request<RequestBody>(
        &self,
        method: Method,
        path: &str,
        body: Option<&RequestBody>,
    ) -> Result<Response, MastraClientError>
    where
        RequestBody: Serialize + ?Sized,
    {
        self.stream_request_with_query(method, path, None::<&()>, body)
            .await
    }

    async fn stream_request_with_query<Query, RequestBody>(
        &self,
        method: Method,
        path: &str,
        query: Option<&Query>,
        body: Option<&RequestBody>,
    ) -> Result<Response, MastraClientError>
    where
        Query: Serialize + ?Sized,
        RequestBody: Serialize + ?Sized,
    {
        let response = self.send_with_query(method, path, query, body).await?;
        ensure_success(response).await
    }

    async fn send<RequestBody>(
        &self,
        method: Method,
        path: &str,
        body: Option<&RequestBody>,
    ) -> Result<Response, MastraClientError>
    where
        RequestBody: Serialize + ?Sized,
    {
        self.send_with_query(method, path, None::<&()>, body).await
    }

    async fn send_with_query<Query, RequestBody>(
        &self,
        method: Method,
        path: &str,
        query: Option<&Query>,
        body: Option<&RequestBody>,
    ) -> Result<Response, MastraClientError>
    where
        Query: Serialize + ?Sized,
        RequestBody: Serialize + ?Sized,
    {
        let url = self.endpoint(path)?;
        let request = self.http.request(method, url);
        let request = if let Some(query) = query {
            request.query(query)
        } else {
            request
        };
        let request = if let Some(body) = body {
            request.json(body)
        } else {
            request
        };
        request.send().await.map_err(MastraClientError::Transport)
    }

    fn endpoint(&self, path: &str) -> Result<Url, MastraClientError> {
        let normalized_path = format!(
            "{}{}",
            self.api_prefix,
            if path.starts_with('/') {
                path.to_owned()
            } else {
                format!("/{path}")
            }
        );
        self.base_url
            .join(normalized_path.trim_start_matches('/'))
            .map_err(MastraClientError::InvalidBaseUrl)
    }
}

async fn decode_response<ResponseBody>(
    response: Response,
) -> Result<ResponseBody, MastraClientError>
where
    ResponseBody: DeserializeOwned,
{
    let response = ensure_success(response).await?;
    response.json().await.map_err(MastraClientError::Decode)
}

async fn ensure_success(response: Response) -> Result<Response, MastraClientError> {
    let status = response.status();
    if status.is_success() {
        return Ok(response);
    }

    let body = response.text().await.map_err(MastraClientError::Decode)?;
    let error = serde_json::from_str::<ErrorResponse>(&body).ok();
    let message = error
        .as_ref()
        .map(|payload| payload.error.clone())
        .unwrap_or_else(|| body.clone());

    Err(MastraClientError::Api {
        status,
        body: message,
        error,
    })
}

fn decode_event_stream<EventType>(
    response: Response,
) -> impl Stream<Item = Result<EventType, MastraClientError>> + Send + 'static
where
    EventType: StreamProtocolEvent + Send + 'static,
{
    try_stream! {
        let mut stream = response.bytes_stream();
        let mut buffer = String::new();
        let mut current_event = None::<String>;
        let mut data_lines = Vec::<String>::new();

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(MastraClientError::Transport)?;
            buffer.push_str(&String::from_utf8_lossy(&chunk));

            while let Some(index) = buffer.find('\n') {
                let mut line = buffer[..index].to_owned();
                buffer.drain(..=index);

                if line.ends_with('\r') {
                    line.pop();
                }

                if line.is_empty() {
                    if let Some(event) = flush_event::<EventType>(&mut current_event, &mut data_lines)? {
                        yield event;
                    }
                    continue;
                }

                if line.starts_with(':') {
                    continue;
                }

                if let Some(rest) = line.strip_prefix("event:") {
                    current_event = Some(rest.trim().to_owned());
                    continue;
                }

                if let Some(rest) = line.strip_prefix("data:") {
                    data_lines.push(rest.trim_start().to_owned());
                }
            }
        }

        if let Some(event) = flush_event::<EventType>(&mut current_event, &mut data_lines)? {
            yield event;
        }
    }
}

fn flush_event<EventType>(
    current_event: &mut Option<String>,
    data_lines: &mut Vec<String>,
) -> Result<Option<EventType>, MastraClientError>
where
    EventType: StreamProtocolEvent,
{
    if data_lines.is_empty() {
        *current_event = None;
        return Ok(None);
    }

    let payload = data_lines.join("\n");
    data_lines.clear();
    let parsed = serde_json::from_str::<EventType>(&payload)
        .map_err(|error| MastraClientError::StreamProtocol(error.to_string()))?;
    *current_event = None;

    if let Some(error) = parsed.error_message() {
        Err(MastraClientError::StreamProtocol(error))
    } else {
        Ok(Some(parsed))
    }
}

fn normalize_api_prefix(prefix: &str) -> String {
    let trimmed = prefix.trim();
    if trimmed.is_empty() || trimmed == "/" {
        String::new()
    } else {
        format!("/{}", trimmed.trim_matches('/'))
    }
}

#[derive(Debug, Serialize)]
struct ThreadQueryWire {
    #[serde(skip_serializing_if = "Option::is_none", rename = "page")]
    page: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "perPage")]
    per_page: Option<PaginationSizeValue>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "resourceId")]
    resource_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    metadata: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "orderBy")]
    order_by: Option<String>,
}

impl TryFrom<ListThreadsQuery> for ThreadQueryWire {
    type Error = MastraClientError;

    fn try_from(query: ListThreadsQuery) -> Result<Self, Self::Error> {
        let metadata = query
            .metadata
            .map(|value| {
                serde_json::to_string(&value).map_err(|error| MastraClientError::Api {
                    status: StatusCode::BAD_REQUEST,
                    body: error.to_string(),
                    error: None,
                })
            })
            .transpose()?;
        let order_by = query.order_by.map(serialize_thread_order_by).transpose()?;

        Ok(Self {
            page: query.page,
            per_page: query.per_page,
            resource_id: query.resource_id,
            metadata,
            order_by,
        })
    }
}

#[derive(Debug, Serialize)]
struct MessageQueryWire {
    #[serde(skip_serializing_if = "Option::is_none", rename = "page")]
    page: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "perPage")]
    per_page: Option<PaginationSizeValue>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "resourceId")]
    resource_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "messageIds")]
    message_ids: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "startDate")]
    start_date: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "endDate")]
    end_date: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "orderBy")]
    order_by: Option<String>,
}

impl TryFrom<ListMessagesQuery> for MessageQueryWire {
    type Error = MastraClientError;

    fn try_from(query: ListMessagesQuery) -> Result<Self, Self::Error> {
        Ok(Self {
            page: query.page,
            per_page: query.per_page,
            resource_id: query.resource_id,
            message_ids: query.message_ids,
            start_date: query.start_date,
            end_date: query.end_date,
            order_by: query.order_by.map(serialize_message_order_by).transpose()?,
        })
    }
}

fn serialize_thread_order_by(order_by: ThreadOrderBy) -> Result<String, MastraClientError> {
    serde_json::to_string(&order_by).map_err(|error| MastraClientError::Api {
        status: StatusCode::BAD_REQUEST,
        body: error.to_string(),
        error: None,
    })
}

fn serialize_message_order_by(order_by: MessageOrderBy) -> Result<String, MastraClientError> {
    serde_json::to_string(&order_by).map_err(|error| MastraClientError::Api {
        status: StatusCode::BAD_REQUEST,
        body: error.to_string(),
        error: None,
    })
}
