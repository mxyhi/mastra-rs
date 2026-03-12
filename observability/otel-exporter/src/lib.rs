use std::collections::BTreeMap;

use async_trait::async_trait;
use mastra_observability_mastra::{
    Attributes, ExportError, HttpExporter, HttpMethod, HttpRequest, HttpRequestBuilder,
    ObservabilityExporter, SpanEvent, SpanKind, SpanStatus, TraceBatch, TraceSpan,
};
use serde_json::{json, Value};
use url::Url;

#[derive(Clone, Debug, Default)]
pub struct OtelConfig {
    pub endpoint: String,
    pub headers: BTreeMap<String, String>,
    pub resource_attributes: Attributes,
}

#[derive(Clone, Debug)]
struct OtelRequestBuilder {
    config: OtelConfig,
}

impl HttpRequestBuilder for OtelRequestBuilder {
    fn build_requests(&self, batch: &TraceBatch) -> Result<Vec<HttpRequest>, ExportError> {
        if self.config.endpoint.is_empty() {
            return Err(ExportError::InvalidConfiguration(
                "otel endpoint must not be empty".to_string(),
            ));
        }

        let mut headers = self.config.headers.clone();
        headers
            .entry("content-type".to_string())
            .or_insert_with(|| "application/json".to_string());

        Ok(vec![HttpRequest {
            method: HttpMethod::Post,
            url: Url::parse(&self.config.endpoint)?,
            headers,
            body: serde_json::to_vec(&build_otel_payload(batch, &self.config.resource_attributes))?,
        }])
    }
}

#[derive(Clone, Debug)]
pub struct OtelExporter {
    http: HttpExporter<OtelRequestBuilder>,
}

impl OtelExporter {
    pub fn new(config: OtelConfig) -> Self {
        Self {
            http: HttpExporter::new(OtelRequestBuilder { config }),
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
impl ObservabilityExporter for OtelExporter {
    async fn export(&self, batch: &TraceBatch) -> Result<(), ExportError> {
        self.export(batch).await
    }
}

pub fn build_otel_payload(batch: &TraceBatch, extra_resource_attributes: &Attributes) -> Value {
    let mut resource_attributes = vec![key_value(
        "service.name",
        Value::String(batch.service_name.clone()),
    )];

    if let Some(environment) = &batch.environment {
        resource_attributes.push(key_value(
            "deployment.environment",
            Value::String(environment.clone()),
        ));
    }

    for (key, value) in &batch.resource_attributes {
        resource_attributes.push(key_value(key, value.clone()));
    }
    for (key, value) in extra_resource_attributes {
        resource_attributes.push(key_value(key, value.clone()));
    }

    json!({
        "resourceSpans": [
            {
                "resource": {
                    "attributes": resource_attributes,
                },
                "scopeSpans": [
                    {
                        "scope": {
                            "name": "mastra-rs",
                            "version": "0.1.0",
                        },
                        "spans": batch.ordered_spans().into_iter().map(otel_span).collect::<Vec<_>>(),
                    }
                ],
            }
        ]
    })
}

fn otel_span(span: &TraceSpan) -> Value {
    let mut attributes = Vec::new();
    attributes.push(key_value(
        "mastra.span.kind",
        Value::String(kind_name(&span.kind).to_string()),
    ));
    for tag in &span.tags {
        attributes.push(key_value("mastra.tag", Value::String(tag.clone())));
    }
    for (key, value) in &span.metadata {
        attributes.push(key_value(key, value.clone()));
    }
    for (key, value) in &span.attributes {
        attributes.push(key_value(key, value.clone()));
    }
    if let Some(input) = &span.input {
        attributes.push(key_value("mastra.input", input.clone()));
    }
    if let Some(output) = &span.output {
        attributes.push(key_value("mastra.output", output.clone()));
    }
    if let Some(usage) = &span.usage {
        attributes.push(key_value("gen_ai.usage.input_tokens", json!(usage.input_tokens)));
        attributes.push(key_value("gen_ai.usage.output_tokens", json!(usage.output_tokens)));
        attributes.push(key_value(
            "gen_ai.usage.reasoning_tokens",
            json!(usage.reasoning_tokens),
        ));
    }

    json!({
        "traceId": span.trace_id,
        "spanId": span.span_id,
        "parentSpanId": span.parent_span_id,
        "name": span.name,
        "kind": kind_name(&span.kind),
        "startTimeUnixNano": timestamp_nanos(span.started_at),
        "endTimeUnixNano": timestamp_nanos(span.ended_at.unwrap_or(span.started_at)),
        "attributes": attributes,
        "events": span.events.iter().map(otel_event).collect::<Vec<_>>(),
        "status": {
            "code": status_code(&span.status),
        },
    })
}

fn otel_event(event: &SpanEvent) -> Value {
    json!({
        "name": event.name,
        "timeUnixNano": timestamp_nanos(event.timestamp),
        "attributes": event
            .attributes
            .iter()
            .map(|(key, value)| key_value(key, value.clone()))
            .collect::<Vec<_>>(),
    })
}

fn key_value(key: &str, value: Value) -> Value {
    json!({
        "key": key,
        "value": otel_any_value(value),
    })
}

// OTLP AnyValue 有多种变体，这里显式展开，避免 provider wrapper 自己重复序列化逻辑。
fn otel_any_value(value: Value) -> Value {
    match value {
        Value::Null => json!({ "stringValue": "null" }),
        Value::Bool(boolean) => json!({ "boolValue": boolean }),
        Value::Number(number) => {
            if let Some(integer) = number.as_i64() {
                json!({ "intValue": integer.to_string() })
            } else if let Some(unsigned) = number.as_u64() {
                json!({ "intValue": unsigned.to_string() })
            } else {
                json!({ "doubleValue": number.as_f64().unwrap_or_default() })
            }
        }
        Value::String(string) => json!({ "stringValue": string }),
        Value::Array(array) => json!({
            "arrayValue": {
                "values": array.into_iter().map(otel_any_value).collect::<Vec<_>>(),
            }
        }),
        Value::Object(object) => {
            let fields = object
                .into_iter()
                .map(|(key, nested)| {
                    json!({
                        "key": key,
                        "value": otel_any_value(nested),
                    })
                })
                .collect::<Vec<_>>();
            json!({
                "kvlistValue": {
                    "values": fields,
                }
            })
        }
    }
}

fn kind_name(kind: &SpanKind) -> &'static str {
    match kind {
        SpanKind::AgentRun => "SPAN_KIND_INTERNAL",
        SpanKind::ModelGeneration => "SPAN_KIND_CLIENT",
        SpanKind::ToolCall => "SPAN_KIND_INTERNAL",
        SpanKind::WorkflowRun => "SPAN_KIND_SERVER",
        SpanKind::WorkflowStep => "SPAN_KIND_INTERNAL",
        SpanKind::Generic => "SPAN_KIND_INTERNAL",
    }
}

fn status_code(status: &SpanStatus) -> &'static str {
    match status {
        SpanStatus::Ok => "STATUS_CODE_OK",
        SpanStatus::Error => "STATUS_CODE_ERROR",
        SpanStatus::InProgress => "STATUS_CODE_UNSET",
    }
}

fn timestamp_nanos(value: chrono::DateTime<chrono::Utc>) -> String {
    value
        .timestamp_nanos_opt()
        .unwrap_or_default()
        .to_string()
}
