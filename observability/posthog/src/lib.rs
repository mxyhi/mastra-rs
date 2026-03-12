use std::collections::BTreeMap;

use async_trait::async_trait;
use mastra_observability_mastra::{
    ExportError, HttpExporter, HttpMethod, HttpRequest, HttpRequestBuilder, ObservabilityExporter,
    SpanKind, TraceBatch, TraceSpan,
};
use serde_json::{Value, json};
use url::Url;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PostHogConfig {
    pub api_key: String,
    pub host: String,
    pub default_distinct_id: String,
}

#[derive(Clone, Debug)]
struct PostHogRequestBuilder {
    config: PostHogConfig,
}

#[derive(Clone, Debug)]
pub struct PostHogExporter {
    inner: HttpExporter<PostHogRequestBuilder>,
}

impl PostHogExporter {
    pub fn new(config: PostHogConfig) -> Self {
        Self {
            inner: HttpExporter::new(PostHogRequestBuilder { config }),
        }
    }

    pub fn build_requests(&self, batch: &TraceBatch) -> Result<Vec<HttpRequest>, ExportError> {
        self.inner.build_requests(batch)
    }

    pub async fn export(&self, batch: &TraceBatch) -> Result<(), ExportError> {
        self.inner.export(batch).await
    }
}

impl HttpRequestBuilder for PostHogExporter {
    fn build_requests(&self, batch: &TraceBatch) -> Result<Vec<HttpRequest>, ExportError> {
        self.inner.build_requests(batch)
    }
}

#[async_trait]
impl ObservabilityExporter for PostHogExporter {
    async fn export(&self, batch: &TraceBatch) -> Result<(), ExportError> {
        self.inner.export(batch).await
    }
}

impl HttpRequestBuilder for PostHogRequestBuilder {
    fn build_requests(&self, batch: &TraceBatch) -> Result<Vec<HttpRequest>, ExportError> {
        validate_config(&self.config)?;

        let url = endpoint_url(&self.config.host, "/batch/")?;
        let headers = BTreeMap::from([(
            "content-type".to_string(),
            "application/json".to_string(),
        )]);
        let distinct_id = batch
            .metadata_string("user_id")
            .unwrap_or(self.config.default_distinct_id.as_str());

        let mut events = vec![trace_event(batch, distinct_id)];
        for span in prioritized_spans(batch) {
            events.push(span_event(span, distinct_id));
        }

        Ok(vec![HttpRequest {
            method: HttpMethod::Post,
            url,
            headers,
            body: serde_json::to_vec(&json!({
                "api_key": self.config.api_key,
                "batch": events,
            }))?,
        }])
    }
}

fn validate_config(config: &PostHogConfig) -> Result<(), ExportError> {
    if config.api_key.trim().is_empty() {
        return Err(ExportError::InvalidConfiguration(
            "posthog api_key must not be empty".to_string(),
        ));
    }
    if config.host.trim().is_empty() {
        return Err(ExportError::InvalidConfiguration(
            "posthog host must not be empty".to_string(),
        ));
    }
    if config.default_distinct_id.trim().is_empty() {
        return Err(ExportError::InvalidConfiguration(
            "posthog default_distinct_id must not be empty".to_string(),
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

fn trace_event(batch: &TraceBatch, distinct_id: &str) -> Value {
    let root = batch.root_span();
    json!({
        "event": "$ai_trace",
        "distinct_id": distinct_id,
        "properties": {
            "$ai_trace_id": root.map(|span| span.trace_id.clone()),
            "$ai_service": batch.service_name,
            "$ai_environment": batch.environment,
            "$ai_session_id": batch.metadata.get("session_id").cloned(),
            "$ai_user_id": batch.metadata.get("user_id").cloned(),
        }
    })
}

fn span_event(span: &TraceSpan, distinct_id: &str) -> Value {
    json!({
        "event": posthog_event_name(span),
        "distinct_id": distinct_id,
        "properties": {
            "$ai_trace_id": span.trace_id,
            "$ai_span_id": span.span_id,
            "$ai_parent_span_id": span.parent_span_id,
            "$ai_name": span.name,
            "$ai_model": span.attributes.get("model").cloned(),
            "$ai_provider": span.attributes.get("provider").cloned(),
            "$ai_tool_name": span.attributes.get("tool_name").cloned(),
            "$ai_input": span.input,
            "$ai_output": span.output,
            "$ai_input_tokens": span.usage.as_ref().map(|usage| usage.input_tokens),
            "$ai_output_tokens": span.usage.as_ref().map(|usage| usage.output_tokens),
        }
    })
}

fn posthog_event_name(span: &TraceSpan) -> &'static str {
    match span.kind {
        SpanKind::ModelGeneration => "$ai_generation",
        SpanKind::ToolCall => "$ai_tool_call",
        SpanKind::AgentRun | SpanKind::WorkflowRun | SpanKind::WorkflowStep => "$ai_span",
        SpanKind::Generic => "$ai_span",
    }
}
