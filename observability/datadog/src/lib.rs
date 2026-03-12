use std::collections::BTreeMap;

use async_trait::async_trait;
use mastra_observability_mastra::{
    ExportError, HttpExporter, HttpMethod, HttpRequest, HttpRequestBuilder, ObservabilityExporter,
    SpanKind, SpanStatus, TraceBatch, TraceSpan,
};
use serde_json::{Value, json};
use url::Url;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DatadogConfig {
    pub api_key: String,
    pub site: String,
    pub ml_app: String,
    pub env: Option<String>,
}

#[derive(Clone, Debug)]
struct DatadogRequestBuilder {
    config: DatadogConfig,
}

#[derive(Clone, Debug)]
pub struct DatadogExporter {
    inner: HttpExporter<DatadogRequestBuilder>,
}

impl DatadogExporter {
    pub fn new(config: DatadogConfig) -> Self {
        Self {
            inner: HttpExporter::new(DatadogRequestBuilder { config }),
        }
    }

    pub fn build_requests(&self, batch: &TraceBatch) -> Result<Vec<HttpRequest>, ExportError> {
        self.inner.build_requests(batch)
    }

    pub async fn export(&self, batch: &TraceBatch) -> Result<(), ExportError> {
        self.inner.export(batch).await
    }
}

impl HttpRequestBuilder for DatadogExporter {
    fn build_requests(&self, batch: &TraceBatch) -> Result<Vec<HttpRequest>, ExportError> {
        self.inner.build_requests(batch)
    }
}

#[async_trait]
impl ObservabilityExporter for DatadogExporter {
    async fn export(&self, batch: &TraceBatch) -> Result<(), ExportError> {
        self.inner.export(batch).await
    }
}

impl HttpRequestBuilder for DatadogRequestBuilder {
    fn build_requests(&self, batch: &TraceBatch) -> Result<Vec<HttpRequest>, ExportError> {
        validate_config(&self.config)?;

        let url = datadog_url(&self.config.site)?;
        let headers = datadog_headers(&self.config.api_key);
        let mut requests = vec![json_request(
            url.clone(),
            headers.clone(),
            json!({
                "data": {
                    "type": "trace",
                    "attributes": {
                        "trace_id": batch.root_span().map(|span| span.trace_id.clone()),
                        "name": batch
                            .root_span()
                            .map(|span| span.name.clone())
                            .unwrap_or_else(|| batch.service_name.clone()),
                        "service": batch.service_name,
                        "env": resolved_environment(batch, &self.config),
                        "ml_app": self.config.ml_app,
                        "user_id": batch.metadata.get("user_id").cloned(),
                        "session_id": batch.metadata.get("session_id").cloned(),
                    }
                }
            }),
        )?];

        for span in prioritized_spans(batch) {
            requests.push(json_request(
                url.clone(),
                headers.clone(),
                json!({
                    "data": {
                        "type": "span",
                        "attributes": {
                            "trace_id": span.trace_id,
                            "span_id": span.span_id,
                            "parent_id": span.parent_span_id,
                            "name": span.name,
                            "kind": datadog_kind(span),
                            "status": datadog_status(span),
                            "service": batch.service_name,
                            "env": resolved_environment(batch, &self.config),
                            "ml_app": self.config.ml_app,
                            "start_ms": span.started_at.timestamp_millis(),
                            "duration_ms": span.duration_ms(),
                            "input": span.input,
                            "output": span.output,
                            "tags": span.tags,
                            "attributes": span.attributes,
                            "metadata": span.metadata,
                        }
                    }
                }),
            )?);
        }

        Ok(requests)
    }
}

fn validate_config(config: &DatadogConfig) -> Result<(), ExportError> {
    if config.api_key.trim().is_empty() {
        return Err(ExportError::InvalidConfiguration(
            "datadog api_key must not be empty".to_string(),
        ));
    }
    if config.site.trim().is_empty() {
        return Err(ExportError::InvalidConfiguration(
            "datadog site must not be empty".to_string(),
        ));
    }
    if config.ml_app.trim().is_empty() {
        return Err(ExportError::InvalidConfiguration(
            "datadog ml_app must not be empty".to_string(),
        ));
    }
    Ok(())
}

fn datadog_url(site: &str) -> Result<Url, ExportError> {
    Url::parse(&format!(
        "https://api.{}/api/intake/llm-obs/v1/trace/spans",
        site.trim()
    ))
    .map_err(ExportError::from)
}

fn datadog_headers(api_key: &str) -> BTreeMap<String, String> {
    BTreeMap::from([
        ("content-type".to_string(), "application/json".to_string()),
        ("dd-api-key".to_string(), api_key.to_string()),
    ])
}

fn json_request(
    url: Url,
    headers: BTreeMap<String, String>,
    body: Value,
) -> Result<HttpRequest, ExportError> {
    Ok(HttpRequest {
        method: HttpMethod::Post,
        url,
        headers,
        body: serde_json::to_vec(&body)?,
    })
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

fn datadog_kind(span: &TraceSpan) -> &'static str {
    match span.kind {
        SpanKind::ModelGeneration => "llm",
        SpanKind::ToolCall => "tool",
        SpanKind::AgentRun => "agent",
        SpanKind::WorkflowRun | SpanKind::WorkflowStep => "workflow",
        SpanKind::Generic => "span",
    }
}

fn datadog_status(span: &TraceSpan) -> &'static str {
    match span.status {
        SpanStatus::Ok => "ok",
        SpanStatus::Error => "error",
        SpanStatus::InProgress => "in_progress",
    }
}

fn resolved_environment(batch: &TraceBatch, config: &DatadogConfig) -> Option<String> {
    config.env.clone().or_else(|| batch.environment.clone())
}
