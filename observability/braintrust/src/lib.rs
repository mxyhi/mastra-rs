use async_trait::async_trait;
use mastra_observability_mastra::{
    ExportError, HttpExporter, HttpMethod, HttpRequest, HttpRequestBuilder, ObservabilityExporter,
    SpanEvent, SpanKind, TraceBatch, TraceSpan,
};
use serde_json::{json, Map, Value};
use url::Url;

#[derive(Clone, Debug)]
pub struct BraintrustConfig {
    pub api_key: String,
    pub endpoint: String,
    pub project_id: String,
}

#[derive(Clone, Debug)]
struct BraintrustRequestBuilder {
    config: BraintrustConfig,
}

impl BraintrustRequestBuilder {
    fn insert_url(&self) -> Result<Url, ExportError> {
        Url::parse(&format!(
            "{}/v1/project_logs/{}/insert",
            self.config.endpoint.trim_end_matches('/'),
            self.config.project_id
        ))
        .map_err(ExportError::from)
    }
}

impl HttpRequestBuilder for BraintrustRequestBuilder {
    fn build_requests(&self, batch: &TraceBatch) -> Result<Vec<HttpRequest>, ExportError> {
        if self.config.api_key.is_empty() || self.config.project_id.is_empty() {
            return Err(ExportError::InvalidConfiguration(
                "braintrust api_key and project_id must not be empty".to_string(),
            ));
        }

        let mut events = batch
            .ordered_spans()
            .into_iter()
            .map(|span| braintrust_event(batch, span))
            .collect::<Vec<_>>();
        events.extend(
            batch.ordered_spans()
                .into_iter()
                .flat_map(|span| {
                    span.events
                        .iter()
                        .enumerate()
                        .map(|(index, event)| braintrust_span_event(batch, span, event, index))
                        .collect::<Vec<_>>()
                }),
        );

        Ok(vec![HttpRequest {
            method: HttpMethod::Post,
            url: self.insert_url()?,
            headers: std::collections::BTreeMap::from([
                (
                    "authorization".to_string(),
                    format!("Bearer {}", self.config.api_key),
                ),
                ("content-type".to_string(), "application/json".to_string()),
            ]),
            body: serde_json::to_vec(&json!({ "events": events }))?,
        }])
    }
}

#[derive(Clone, Debug)]
pub struct BraintrustExporter {
    http: HttpExporter<BraintrustRequestBuilder>,
}

impl BraintrustExporter {
    pub fn new(config: BraintrustConfig) -> Self {
        Self {
            http: HttpExporter::new(BraintrustRequestBuilder { config }),
        }
    }

    pub fn build_requests(&self, batch: &TraceBatch) -> Result<Vec<HttpRequest>, ExportError> {
        self.http.build_requests(batch)
    }

    pub async fn export(&self, batch: &TraceBatch) -> Result<(), ExportError> {
        self.http.export(batch).await
    }
}

#[async_trait]
impl ObservabilityExporter for BraintrustExporter {
    async fn export(&self, batch: &TraceBatch) -> Result<(), ExportError> {
        self.export(batch).await
    }
}

fn braintrust_event(batch: &TraceBatch, span: &TraceSpan) -> Value {
    let mut metadata = Map::new();
    metadata.insert("service_name".to_string(), json!(batch.service_name));
    metadata.insert("span_kind".to_string(), json!(kind_name(&span.kind)));
    for (key, value) in &span.metadata {
        metadata.insert(key.clone(), value.clone());
    }
    for (key, value) in &span.attributes {
        metadata.insert(key.clone(), value.clone());
    }

    json!({
        "id": span.span_id,
        "span_id": span.span_id,
        "trace_id": span.trace_id,
        "parent_id": span.parent_span_id,
        "span_attributes": metadata,
        "input": span.input,
        "output": span.output,
        "expected": Value::Null,
        "tags": span.tags,
        "scores": {},
        "metrics": {
            "duration_ms": span.duration_ms(),
        },
        "created": span.started_at.to_rfc3339(),
        "ended": span.ended_at.unwrap_or(span.started_at).to_rfc3339(),
    })
}

fn braintrust_span_event(
    batch: &TraceBatch,
    span: &TraceSpan,
    event: &SpanEvent,
    index: usize,
) -> Value {
    json!({
        "id": format!("{}:event:{index}", span.span_id),
        "span_id": format!("{}:event:{index}", span.span_id),
        "trace_id": span.trace_id,
        "parent_id": span.span_id,
        "span_attributes": {
            "service_name": batch.service_name,
            "span_kind": "event",
            "event_name": event.name,
            "attributes": event.attributes,
        },
        "input": event.attributes,
        "output": Value::Null,
        "tags": [],
        "scores": {},
        "metrics": {},
        "created": event.timestamp.to_rfc3339(),
        "ended": event.timestamp.to_rfc3339(),
    })
}

fn kind_name(kind: &SpanKind) -> &'static str {
    match kind {
        SpanKind::AgentRun => "agent",
        SpanKind::ModelGeneration => "llm",
        SpanKind::ToolCall => "tool",
        SpanKind::WorkflowRun => "workflow",
        SpanKind::WorkflowStep => "workflow_step",
        SpanKind::Generic => "generic",
    }
}
