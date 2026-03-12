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

use crate::{
    MastraClientError,
    types::{
        AppendMemoryMessagesRequest, AppendMemoryMessagesResponse, CreateMemoryThreadRequest,
        CreateMemoryThreadResponse, CreateWorkflowRunRequest, ErrorResponse, GenerateRequest,
        GenerateResponse, GenerateStreamEvent, ListAgentsResponse, ListMemoriesResponse,
        ListMemoryMessagesResponse, ListThreadsResponse, ListWorkflowsResponse,
        StartWorkflowRunRequest, StartWorkflowRunResponse, WorkflowRunRecord,
    },
};

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
    memory_id: String,
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

    pub fn memories(&self) -> MemoriesClient {
        MemoriesClient {
            inner: Arc::clone(&self.inner),
        }
    }

    pub fn memory(&self, memory_id: impl Into<String>) -> MemoryClient {
        MemoryClient {
            inner: Arc::clone(&self.inner),
            memory_id: memory_id.into(),
        }
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
    ) -> Result<impl Stream<Item = Result<GenerateStreamEvent, MastraClientError>> + Send + 'static, MastraClientError>
    {
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
            resource_id: None,
            thread_id: None,
            run_id: None,
            max_steps: Some(1),
            request_context: Default::default(),
        })
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
}

impl MemoriesClient {
    pub async fn list(&self) -> Result<ListMemoriesResponse, MastraClientError> {
        self.inner
            .request(Method::GET, "/memories", Option::<&()>::None)
            .await
    }
}

impl MemoryClient {
    pub async fn threads(&self) -> Result<ListThreadsResponse, MastraClientError> {
        self.inner
            .request(
                Method::GET,
                &format!("/memory/{}/threads", self.memory_id),
                Option::<&()>::None,
            )
            .await
    }

    pub async fn create_thread(
        &self,
        request: CreateMemoryThreadRequest,
    ) -> Result<CreateMemoryThreadResponse, MastraClientError> {
        self.inner
            .request(
                Method::POST,
                &format!("/memory/{}/threads", self.memory_id),
                Some(&request),
            )
            .await
    }

    pub async fn append_messages(
        &self,
        thread_id: &str,
        request: AppendMemoryMessagesRequest,
    ) -> Result<AppendMemoryMessagesResponse, MastraClientError> {
        self.inner
            .request(
                Method::POST,
                &format!("/memory/{}/threads/{thread_id}/messages", self.memory_id),
                Some(&request),
            )
            .await
    }

    pub async fn messages(
        &self,
        thread_id: &str,
    ) -> Result<ListMemoryMessagesResponse, MastraClientError> {
        self.inner
            .request(
                Method::GET,
                &format!("/memory/{}/threads/{thread_id}/messages", self.memory_id),
                Option::<&()>::None,
            )
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
        let response = self.send(method, path, body).await?;
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
        let response = self.send(method, path, body).await?;
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
        let url = self.endpoint(path)?;
        let request = self.http.request(method, url);
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

fn decode_event_stream(
    response: Response,
) -> impl Stream<Item = Result<GenerateStreamEvent, MastraClientError>> + Send + 'static {
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
                    if let Some(event) = flush_event(&mut current_event, &mut data_lines)? {
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

        if let Some(event) = flush_event(&mut current_event, &mut data_lines)? {
            yield event;
        }
    }
}

fn flush_event(
    current_event: &mut Option<String>,
    data_lines: &mut Vec<String>,
) -> Result<Option<GenerateStreamEvent>, MastraClientError> {
    if data_lines.is_empty() {
        *current_event = None;
        return Ok(None);
    }

    let payload = data_lines.join("\n");
    data_lines.clear();
    let parsed = serde_json::from_str::<GenerateStreamEvent>(&payload)
        .map_err(|error| MastraClientError::StreamProtocol(error.to_string()))?;
    *current_event = None;

    match parsed {
        GenerateStreamEvent::Error(error) => Err(MastraClientError::StreamProtocol(error.error)),
        event => Ok(Some(event)),
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
