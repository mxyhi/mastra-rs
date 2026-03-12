#![allow(non_snake_case)]

use std::{collections::BTreeMap, sync::Arc};

use axum::{
    body::{to_bytes, Body},
    extract::State,
    http::{Request, StatusCode},
    response::IntoResponse,
    routing::any,
    Router,
};
use chrono::{Duration, TimeZone, Utc};
use mastra_observability_mastra::{
    Attributes, SpanEvent, SpanKind, SpanStatus, TokenUsage, TraceBatch, TraceSpan,
};
use serde_json::json;
use tokio::{
    net::TcpListener,
    sync::{mpsc, Mutex},
    task::JoinHandle,
};
use url::Url;
use uuid::Uuid;

#[derive(Clone)]
struct ServerState {
    sender: mpsc::Sender<CapturedRequest>,
    status: StatusCode,
    body: Arc<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapturedRequest {
    pub method: String,
    pub path: String,
    pub headers: BTreeMap<String, String>,
    pub body: Vec<u8>,
}

impl CapturedRequest {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .get(&name.to_ascii_lowercase())
            .map(String::as_str)
    }
}

pub struct TestHttpServer {
    base_url: Url,
    receiver: Mutex<mpsc::Receiver<CapturedRequest>>,
    handle: JoinHandle<()>,
}

impl Drop for TestHttpServer {
    fn drop(&mut self) {
        self.handle.abort();
    }
}

impl TestHttpServer {
    pub async fn start(status_code: u16, body: impl Into<String>) -> Self {
        let (sender, receiver) = mpsc::channel(16);
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("test server listener should bind");
        let addr = listener
            .local_addr()
            .expect("test server should expose a local address");
        let state = ServerState {
            sender,
            status: StatusCode::from_u16(status_code).expect("status code should be valid"),
            body: Arc::new(body.into()),
        };
        let app = Router::new()
            .fallback(any(capture_request))
            .with_state(state);
        let handle = tokio::spawn(async move {
            axum::serve(listener, app)
                .await
                .expect("test server should stay healthy");
        });

        Self {
            base_url: Url::parse(&format!("http://{addr}"))
                .expect("test server base url should be valid"),
            receiver: Mutex::new(receiver),
            handle,
        }
    }

    pub fn url(&self, path: &str) -> Url {
        self.base_url.join(path).expect("path should be joinable")
    }

    pub async fn recv(&self) -> CapturedRequest {
        self.receiver
            .lock()
            .await
            .recv()
            .await
            .expect("request should arrive")
    }
}

async fn capture_request(
    State(state): State<ServerState>,
    request: Request<Body>,
) -> impl IntoResponse {
    let (parts, body) = request.into_parts();
    let body = to_bytes(body, usize::MAX)
        .await
        .expect("request body should be readable");

    let headers = parts
        .headers
        .iter()
        .map(|(name, value)| {
            (
                name.as_str().to_ascii_lowercase(),
                value.to_str().unwrap_or_default().to_string(),
            )
        })
        .collect::<BTreeMap<_, _>>();

    state
        .sender
        .send(CapturedRequest {
            method: parts.method.to_string(),
            path: parts.uri.path().to_string(),
            headers,
            body: body.to_vec(),
        })
        .await
        .expect("request should be recorded");

    (state.status, state.body.as_str().to_string())
}

pub fn sample_trace_batch() -> TraceBatch {
    let started_at = Utc
        .with_ymd_and_hms(2026, 3, 12, 10, 0, 0)
        .single()
        .expect("sample timestamp should be valid");
    let root_started_at = started_at;
    let model_started_at = root_started_at + Duration::milliseconds(200);
    let tool_started_at = root_started_at + Duration::milliseconds(1200);

    let mut batch_metadata = Attributes::new();
    batch_metadata.insert("user_id".to_string(), json!("user-123"));
    batch_metadata.insert("session_id".to_string(), json!("session-456"));

    let mut resource_attributes = Attributes::new();
    resource_attributes.insert("service.version".to_string(), json!("0.1.0"));

    let mut root_metadata = Attributes::new();
    root_metadata.insert("agent_name".to_string(), json!("researcher"));

    let mut model_attributes = Attributes::new();
    model_attributes.insert("model".to_string(), json!("gpt-4.1-mini"));
    model_attributes.insert("provider".to_string(), json!("openai"));
    model_attributes.insert("temperature".to_string(), json!(0.2));

    let mut tool_attributes = Attributes::new();
    tool_attributes.insert("tool_name".to_string(), json!("web_search"));

    let root_span = TraceSpan {
        trace_id: "trace-agent-123".to_string(),
        span_id: "span-root-123".to_string(),
        parent_span_id: None,
        name: "agent.run".to_string(),
        kind: SpanKind::AgentRun,
        status: SpanStatus::Ok,
        started_at: root_started_at,
        ended_at: Some(root_started_at + Duration::seconds(3)),
        tags: vec!["env:test".to_string(), "team:core".to_string()],
        metadata: root_metadata,
        attributes: Attributes::new(),
        input: Some(json!({
            "messages": [
                { "role": "user", "content": "Summarize the weather" }
            ]
        })),
        output: Some(json!({
            "text": "It is sunny today."
        })),
        usage: None,
        events: Vec::new(),
    };

    let model_span = TraceSpan {
        trace_id: "trace-agent-123".to_string(),
        span_id: "span-model-123".to_string(),
        parent_span_id: Some("span-root-123".to_string()),
        name: "model.generate".to_string(),
        kind: SpanKind::ModelGeneration,
        status: SpanStatus::Ok,
        started_at: model_started_at,
        ended_at: Some(model_started_at + Duration::milliseconds(900)),
        tags: vec!["provider:openai".to_string()],
        metadata: Attributes::new(),
        attributes: model_attributes,
        input: Some(json!({
            "messages": [
                { "role": "system", "content": "You are concise." },
                { "role": "user", "content": "Summarize the weather" }
            ]
        })),
        output: Some(json!({
            "choices": [
                { "text": "It is sunny today." }
            ]
        })),
        usage: Some(TokenUsage {
            input_tokens: 21,
            output_tokens: 9,
            reasoning_tokens: 0,
            cache_read_tokens: 0,
            cache_write_tokens: 0,
        }),
        events: vec![SpanEvent {
            name: "first_token".to_string(),
            timestamp: model_started_at + Duration::milliseconds(150),
            attributes: BTreeMap::from([
                ("token_index".to_string(), json!(0)),
                ("content".to_string(), json!("It")),
            ]),
        }],
    };

    let tool_span = TraceSpan {
        trace_id: "trace-agent-123".to_string(),
        span_id: format!("span-tool-{}", Uuid::new_v4()),
        parent_span_id: Some("span-root-123".to_string()),
        name: "tool.execute".to_string(),
        kind: SpanKind::ToolCall,
        status: SpanStatus::Ok,
        started_at: tool_started_at,
        ended_at: Some(tool_started_at + Duration::milliseconds(200)),
        tags: vec!["tool:web_search".to_string()],
        metadata: Attributes::new(),
        attributes: tool_attributes,
        input: Some(json!({
            "query": "weather in Shanghai"
        })),
        output: Some(json!({
            "results": ["Sunny"]
        })),
        usage: None,
        events: Vec::new(),
    };

    TraceBatch {
        service_name: "mastra-rs-tests".to_string(),
        environment: Some("test".to_string()),
        metadata: batch_metadata,
        resource_attributes,
        spans: vec![root_span, model_span, tool_span],
    }
}
