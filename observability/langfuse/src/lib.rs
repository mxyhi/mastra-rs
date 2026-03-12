use std::collections::BTreeMap;

use async_trait::async_trait;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use mastra_observability_mastra::{
    ExportError, HttpExporter, HttpMethod, HttpRequest, HttpRequestBuilder, ObservabilityExporter,
    SpanKind, TraceBatch, TraceSpan,
};
use serde_json::{Value, json};
use url::Url;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LangfuseConfig {
    pub public_key: String,
    pub secret_key: String,
    pub base_url: String,
}

#[derive(Clone, Debug)]
struct LangfuseRequestBuilder {
    config: LangfuseConfig,
}

#[derive(Clone, Debug)]
pub struct LangfuseExporter {
    inner: HttpExporter<LangfuseRequestBuilder>,
}

impl LangfuseExporter {
    pub fn new(config: LangfuseConfig) -> Self {
        Self {
            inner: HttpExporter::new(LangfuseRequestBuilder { config }),
        }
    }

    pub fn build_requests(&self, batch: &TraceBatch) -> Result<Vec<HttpRequest>, ExportError> {
        self.inner.build_requests(batch)
    }

    pub async fn export(&self, batch: &TraceBatch) -> Result<(), ExportError> {
        self.inner.export(batch).await
    }
}

impl HttpRequestBuilder for LangfuseExporter {
    fn build_requests(&self, batch: &TraceBatch) -> Result<Vec<HttpRequest>, ExportError> {
        self.inner.build_requests(batch)
    }
}

#[async_trait]
impl ObservabilityExporter for LangfuseExporter {
    async fn export(&self, batch: &TraceBatch) -> Result<(), ExportError> {
        self.inner.export(batch).await
    }
}

impl HttpRequestBuilder for LangfuseRequestBuilder {
    fn build_requests(&self, batch: &TraceBatch) -> Result<Vec<HttpRequest>, ExportError> {
        validate_config(&self.config)?;

        let url = endpoint_url(&self.config.base_url, "/api/public/ingestion")?;
        let mut headers = BTreeMap::from([(
            "content-type".to_string(),
            "application/json".to_string(),
        )]);
        headers.insert(
            "authorization".to_string(),
            format!(
                "Basic {}",
                STANDARD.encode(format!(
                    "{}:{}",
                    self.config.public_key, self.config.secret_key
                ))
            ),
        );

        let mut items = vec![trace_item(batch)];
        for span in prioritized_spans(batch) {
            items.push(span_item(span));
            for event in &span.events {
                items.push(json!({
                    "type": "event-create",
                    "id": format!("{}:{}", span.span_id, event.name),
                    "traceId": span.trace_id,
                    "name": event.name,
                    "timestamp": event.timestamp.to_rfc3339(),
                    "metadata": event.attributes,
                }));
            }
        }

        Ok(vec![HttpRequest {
            method: HttpMethod::Post,
            url,
            headers,
            body: serde_json::to_vec(&json!({ "batch": items }))?,
        }])
    }
}

fn validate_config(config: &LangfuseConfig) -> Result<(), ExportError> {
    if config.public_key.trim().is_empty() || config.secret_key.trim().is_empty() {
        return Err(ExportError::InvalidConfiguration(
            "langfuse public_key and secret_key must not be empty".to_string(),
        ));
    }
    if config.base_url.trim().is_empty() {
        return Err(ExportError::InvalidConfiguration(
            "langfuse base_url must not be empty".to_string(),
        ));
    }
    Ok(())
}

fn endpoint_url(base_url: &str, path: &str) -> Result<Url, ExportError> {
    Url::parse(base_url)?.join(path).map_err(ExportError::from)
}

fn prioritized_spans(batch: &TraceBatch) -> Vec<&TraceSpan> {
    let mut spans = batch.spans.iter().collect::<Vec<_>>();
    spans.sort_by(|left, right| {
        span_priority(left)
            .cmp(&span_priority(right))
            .then_with(|| left.started_at.cmp(&right.started_at))
            .then_with(|| left.span_id.cmp(&right.span_id))
    });
    spans
}

fn span_priority(span: &TraceSpan) -> u8 {
    match span.kind {
        SpanKind::ModelGeneration => 0,
        SpanKind::AgentRun => 1,
        SpanKind::ToolCall => 2,
        SpanKind::WorkflowRun | SpanKind::WorkflowStep => 3,
        SpanKind::Generic => 4,
    }
}

fn trace_item(batch: &TraceBatch) -> Value {
    let root = batch.root_span();
    json!({
        "type": "trace-create",
        "id": root.map(|span| span.trace_id.clone()).unwrap_or_else(|| batch.service_name.clone()),
        "name": root.map(|span| span.name.clone()).unwrap_or_else(|| batch.service_name.clone()),
        "timestamp": root.map(|span| span.started_at.to_rfc3339()),
        "environment": batch.environment,
        "userId": batch.metadata.get("user_id").cloned(),
        "sessionId": batch.metadata.get("session_id").cloned(),
        "metadata": batch.metadata,
    })
}

fn span_item(span: &TraceSpan) -> Value {
    match span.kind {
        SpanKind::ModelGeneration => json!({
            "type": "generation-create",
            "id": span.span_id,
            "traceId": span.trace_id,
            "parentObservationId": span.parent_span_id,
            "name": span.name,
            "startTime": span.started_at.to_rfc3339(),
            "endTime": span.ended_at.map(|time| time.to_rfc3339()),
            "model": span.attributes.get("model").cloned(),
            "input": span.input,
            "output": span.output,
            "usage": span.usage,
            "metadata": span.attributes,
        }),
        _ => json!({
            "type": "span-update",
            "id": span.span_id,
            "traceId": span.trace_id,
            "parentObservationId": span.parent_span_id,
            "name": span.name,
            "startTime": span.started_at.to_rfc3339(),
            "endTime": span.ended_at.map(|time| time.to_rfc3339()),
            "input": span.input,
            "output": span.output,
            "metadata": span.attributes,
        }),
    }
}
