#![allow(non_snake_case)]

use std::collections::BTreeMap;

use chrono::{Duration, TimeZone, Utc};
use mastra_observability_mastra::{SpanKind, SpanStatus, TraceBatch, TraceSpan};
use serde_json::json;

pub fn example_trace_batch(service_name: &str) -> TraceBatch {
    let started_at = Utc
        .with_ymd_and_hms(2026, 3, 12, 12, 0, 0)
        .single()
        .expect("example timestamp should be valid");

    TraceBatch {
        service_name: service_name.to_string(),
        environment: Some("example".to_string()),
        metadata: BTreeMap::from([
            ("user_id".to_string(), json!("example-user")),
            ("session_id".to_string(), json!("example-session")),
        ]),
        resource_attributes: BTreeMap::new(),
        spans: vec![TraceSpan {
            trace_id: "example-trace".to_string(),
            span_id: "example-root".to_string(),
            parent_span_id: None,
            name: "example.agent".to_string(),
            kind: SpanKind::AgentRun,
            status: SpanStatus::Ok,
            started_at,
            ended_at: Some(started_at + Duration::seconds(1)),
            tags: vec!["example".to_string()],
            metadata: BTreeMap::new(),
            attributes: BTreeMap::new(),
            input: Some(json!({ "prompt": "hello" })),
            output: Some(json!({ "text": "world" })),
            usage: None,
            events: Vec::new(),
        }],
    }
}

pub fn provider_example_endpoints() -> BTreeMap<&'static str, &'static str> {
    BTreeMap::from([
        ("langfuse", "https://cloud.langfuse.com/api/public/ingestion"),
        ("langsmith", "https://api.smith.langchain.com/runs/batch"),
        ("posthog", "https://us.i.posthog.com/batch/"),
        ("datadog", "https://api.datadoghq.com/api/intake/llm-obs/v1/trace/spans"),
        ("sentry", "https://<host>/api/<project_id>/envelope/"),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_example_trace_batch() {
        let batch = example_trace_batch("demo-service");
        assert_eq!(batch.service_name, "demo-service");
        assert_eq!(batch.spans.len(), 1);
        assert_eq!(batch.spans[0].name, "example.agent");
    }
}
