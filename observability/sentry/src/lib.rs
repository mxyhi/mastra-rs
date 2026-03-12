use std::collections::BTreeMap;

use async_trait::async_trait;
use mastra_observability_mastra::{
    ExportError, HttpExporter, HttpMethod, HttpRequest, HttpRequestBuilder, ObservabilityExporter,
    SpanKind, SpanStatus, TraceBatch, TraceSpan,
};
use serde_json::{Value, json};
use url::Url;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SentryConfig {
    pub dsn: String,
    pub environment: Option<String>,
    pub release: Option<String>,
}

#[derive(Clone, Debug)]
struct SentryRequestBuilder {
    config: SentryConfig,
}

#[derive(Clone, Debug)]
pub struct SentryExporter {
    inner: HttpExporter<SentryRequestBuilder>,
}

impl SentryExporter {
    pub fn new(config: SentryConfig) -> Self {
        Self {
            inner: HttpExporter::new(SentryRequestBuilder { config }),
        }
    }

    pub fn build_requests(&self, batch: &TraceBatch) -> Result<Vec<HttpRequest>, ExportError> {
        self.inner.build_requests(batch)
    }

    pub async fn export(&self, batch: &TraceBatch) -> Result<(), ExportError> {
        self.inner.export(batch).await
    }
}

impl HttpRequestBuilder for SentryExporter {
    fn build_requests(&self, batch: &TraceBatch) -> Result<Vec<HttpRequest>, ExportError> {
        self.inner.build_requests(batch)
    }
}

#[async_trait]
impl ObservabilityExporter for SentryExporter {
    async fn export(&self, batch: &TraceBatch) -> Result<(), ExportError> {
        self.inner.export(batch).await
    }
}

impl HttpRequestBuilder for SentryRequestBuilder {
    fn build_requests(&self, batch: &TraceBatch) -> Result<Vec<HttpRequest>, ExportError> {
        validate_config(&self.config)?;

        let url = envelope_url(&self.config.dsn)?;
        let headers = BTreeMap::from([(
            "content-type".to_string(),
            "application/x-sentry-envelope".to_string(),
        )]);

        let mut items = vec![summary_item(batch, &self.config)];
        for span in prioritized_spans(batch) {
            items.push(span_item(span, &self.config, false));
        }
        let payload = json!({ "items": items });
        let envelope = format!(
            "{}\n{}\n{}",
            serde_json::to_string(&json!({ "dsn": self.config.dsn }))?,
            serde_json::to_string(&json!({
                "type": "span",
                "content_type": "application/vnd.sentry.items.span.v2+json"
            }))?,
            serde_json::to_string(&payload)?,
        );

        Ok(vec![HttpRequest {
            method: HttpMethod::Post,
            url,
            headers,
            body: envelope.into_bytes(),
        }])
    }
}

fn validate_config(config: &SentryConfig) -> Result<(), ExportError> {
    if config.dsn.trim().is_empty() {
        return Err(ExportError::InvalidConfiguration(
            "sentry dsn must not be empty".to_string(),
        ));
    }
    Ok(())
}

fn envelope_url(dsn: &str) -> Result<Url, ExportError> {
    let dsn = Url::parse(dsn)?;
    let host = dsn
        .host_str()
        .ok_or_else(|| ExportError::InvalidConfiguration("sentry dsn host is missing".to_string()))?;
    let project_id = dsn
        .path_segments()
        .and_then(|segments| segments.filter(|segment| !segment.is_empty()).next_back())
        .ok_or_else(|| {
            ExportError::InvalidConfiguration("sentry dsn project id is missing".to_string())
        })?;

    let port = dsn.port().map(|port| format!(":{port}")).unwrap_or_default();
    Url::parse(&format!(
        "{}://{}{}/api/{}/envelope/",
        dsn.scheme(),
        host,
        port,
        project_id
    ))
    .map_err(ExportError::from)
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

fn summary_item(batch: &TraceBatch, config: &SentryConfig) -> Value {
    let root = batch.root_span();
    json!({
        "type": "trace",
        "span_id": root.map(|span| span.span_id.clone()),
        "trace_id": root.map(|span| span.trace_id.clone()),
        "parent_span_id": Value::Null,
        "op": "trace",
        "description": root.map(|span| span.name.clone()).unwrap_or_else(|| batch.service_name.clone()),
        "start_timestamp": root.map(|span| span.started_at.to_rfc3339()),
        "timestamp": root.and_then(|span| span.ended_at.map(|time| time.to_rfc3339())),
        "status": root.map(sentry_status),
        "is_segment": true,
        "origin": "auto.ai.mastra-rs",
        "environment": config.environment,
        "release": config.release,
    })
}

fn span_item(span: &TraceSpan, config: &SentryConfig, is_segment: bool) -> Value {
    json!({
        "type": "span",
        "span_id": span.span_id,
        "trace_id": span.trace_id,
        "parent_span_id": span.parent_span_id,
        "op": sentry_op(span),
        "description": span.name,
        "start_timestamp": span.started_at.to_rfc3339(),
        "timestamp": span.ended_at.map(|time| time.to_rfc3339()),
        "status": sentry_status(span),
        "is_segment": is_segment,
        "origin": "auto.ai.mastra-rs",
        "environment": config.environment,
        "release": config.release,
        "data": {
            "input": span.input,
            "output": span.output,
            "attributes": span.attributes,
            "metadata": span.metadata,
        }
    })
}

fn sentry_op(span: &TraceSpan) -> &'static str {
    match span.kind {
        SpanKind::ModelGeneration => "ai.generation",
        SpanKind::ToolCall => "ai.tool_call",
        SpanKind::AgentRun => "ai.agent",
        SpanKind::WorkflowRun => "ai.workflow",
        SpanKind::WorkflowStep => "ai.workflow.step",
        SpanKind::Generic => "ai.span",
    }
}

fn sentry_status(span: &TraceSpan) -> &'static str {
    match span.status {
        SpanStatus::Ok => "ok",
        SpanStatus::Error => "internal_error",
        SpanStatus::InProgress => "unknown",
    }
}
