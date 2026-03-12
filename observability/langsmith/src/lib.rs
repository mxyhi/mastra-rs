use std::collections::BTreeMap;

use async_trait::async_trait;
use mastra_observability_mastra::{
    ExportError, HttpExporter, HttpMethod, HttpRequest, HttpRequestBuilder, ObservabilityExporter,
    SpanKind, SpanStatus, TraceBatch, TraceSpan,
};
use serde_json::{Value, json};
use url::Url;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LangSmithConfig {
    pub api_key: String,
    pub api_url: String,
    pub project_name: String,
}

#[derive(Clone, Debug)]
struct LangSmithRequestBuilder {
    config: LangSmithConfig,
}

#[derive(Clone, Debug)]
pub struct LangSmithExporter {
    inner: HttpExporter<LangSmithRequestBuilder>,
}

impl LangSmithExporter {
    pub fn new(config: LangSmithConfig) -> Self {
        Self {
            inner: HttpExporter::new(LangSmithRequestBuilder { config }),
        }
    }

    pub fn build_requests(&self, batch: &TraceBatch) -> Result<Vec<HttpRequest>, ExportError> {
        self.inner.build_requests(batch)
    }

    pub async fn export(&self, batch: &TraceBatch) -> Result<(), ExportError> {
        self.inner.export(batch).await
    }
}

impl HttpRequestBuilder for LangSmithExporter {
    fn build_requests(&self, batch: &TraceBatch) -> Result<Vec<HttpRequest>, ExportError> {
        self.inner.build_requests(batch)
    }
}

#[async_trait]
impl ObservabilityExporter for LangSmithExporter {
    async fn export(&self, batch: &TraceBatch) -> Result<(), ExportError> {
        self.inner.export(batch).await
    }
}

impl HttpRequestBuilder for LangSmithRequestBuilder {
    fn build_requests(&self, batch: &TraceBatch) -> Result<Vec<HttpRequest>, ExportError> {
        validate_config(&self.config)?;

        let url = endpoint_url(&self.config.api_url, "/runs/batch")?;
        let headers = BTreeMap::from([
            ("content-type".to_string(), "application/json".to_string()),
            ("x-api-key".to_string(), self.config.api_key.clone()),
        ]);

        let mut post = vec![summary_run(batch, &self.config.project_name)];
        let mut patch = vec![summary_patch(batch)];
        for span in prioritized_spans(batch) {
            post.push(span_post(span, &self.config.project_name));
            patch.push(span_patch(span));
        }

        Ok(vec![HttpRequest {
            method: HttpMethod::Post,
            url,
            headers,
            body: serde_json::to_vec(&json!({ "post": post, "patch": patch }))?,
        }])
    }
}

fn validate_config(config: &LangSmithConfig) -> Result<(), ExportError> {
    if config.api_key.trim().is_empty() {
        return Err(ExportError::InvalidConfiguration(
            "langsmith api_key must not be empty".to_string(),
        ));
    }
    if config.api_url.trim().is_empty() {
        return Err(ExportError::InvalidConfiguration(
            "langsmith api_url must not be empty".to_string(),
        ));
    }
    if config.project_name.trim().is_empty() {
        return Err(ExportError::InvalidConfiguration(
            "langsmith project_name must not be empty".to_string(),
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

fn summary_run(batch: &TraceBatch, project_name: &str) -> Value {
    let root = batch.root_span();
    json!({
        "id": format!("trace:{}", root.map(|span| span.trace_id.clone()).unwrap_or_else(|| batch.service_name.clone())),
        "trace_id": root.map(|span| span.trace_id.clone()),
        "name": root.map(|span| span.name.clone()).unwrap_or_else(|| batch.service_name.clone()),
        "run_type": "chain",
        "session_name": project_name,
        "start_time": root.map(|span| span.started_at.to_rfc3339()),
        "end_time": root.and_then(|span| span.ended_at.map(|time| time.to_rfc3339())),
        "extra": {
            "metadata": batch.metadata,
            "service_name": batch.service_name,
            "environment": batch.environment,
        }
    })
}

fn summary_patch(batch: &TraceBatch) -> Value {
    let root = batch.root_span();
    json!({
        "id": format!("trace:{}", root.map(|span| span.trace_id.clone()).unwrap_or_else(|| batch.service_name.clone())),
        "trace_id": root.map(|span| span.trace_id.clone()),
        "end_time": root.and_then(|span| span.ended_at.map(|time| time.to_rfc3339())),
        "outputs": root.and_then(|span| span.output.clone()),
        "error": root
            .filter(|span| matches!(span.status, SpanStatus::Error))
            .map(|span| span.name.clone()),
    })
}

fn span_post(span: &TraceSpan, project_name: &str) -> Value {
    json!({
        "id": span.span_id,
        "trace_id": span.trace_id,
        "parent_run_id": span.parent_span_id,
        "name": span.name,
        "run_type": run_type(span),
        "session_name": project_name,
        "start_time": span.started_at.to_rfc3339(),
        "inputs": span.input,
        "extra": {
            "metadata": span.metadata,
            "attributes": span.attributes,
            "tags": span.tags,
        }
    })
}

fn span_patch(span: &TraceSpan) -> Value {
    json!({
        "id": span.span_id,
        "trace_id": span.trace_id,
        "end_time": span.ended_at.map(|time| time.to_rfc3339()),
        "outputs": span.output,
        "error": matches!(span.status, SpanStatus::Error).then(|| span.name.clone()),
    })
}

fn run_type(span: &TraceSpan) -> &'static str {
    match span.kind {
        SpanKind::ModelGeneration => "llm",
        SpanKind::ToolCall => "tool",
        SpanKind::AgentRun | SpanKind::WorkflowRun | SpanKind::WorkflowStep => "chain",
        SpanKind::Generic => "chain",
    }
}
