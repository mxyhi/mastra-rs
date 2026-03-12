use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub type Attributes = BTreeMap<String, Value>;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SpanKind {
    AgentRun,
    ModelGeneration,
    ToolCall,
    WorkflowRun,
    WorkflowStep,
    Generic,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SpanStatus {
    Ok,
    Error,
    InProgress,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct TokenUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub reasoning_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct SpanEvent {
    pub name: String,
    pub timestamp: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub attributes: Attributes,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct TraceSpan {
    pub trace_id: String,
    pub span_id: String,
    pub parent_span_id: Option<String>,
    pub name: String,
    pub kind: SpanKind,
    pub status: SpanStatus,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub metadata: Attributes,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub attributes: Attributes,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<TokenUsage>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub events: Vec<SpanEvent>,
}

impl TraceSpan {
    pub fn is_root(&self) -> bool {
        self.parent_span_id.is_none()
    }

    pub fn duration_ms(&self) -> Option<i64> {
        self.ended_at
            .map(|ended_at| ended_at.signed_duration_since(self.started_at).num_milliseconds())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct TraceBatch {
    pub service_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub environment: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub metadata: Attributes,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub resource_attributes: Attributes,
    pub spans: Vec<TraceSpan>,
}

impl TraceBatch {
    pub fn root_span(&self) -> Option<&TraceSpan> {
        self.spans.iter().find(|span| span.is_root())
    }

    pub fn metadata_string(&self, key: &str) -> Option<&str> {
        self.metadata.get(key).and_then(Value::as_str)
    }

    pub fn ordered_spans(&self) -> Vec<&TraceSpan> {
        let mut spans = self.spans.iter().collect::<Vec<_>>();
        spans.sort_by(|left, right| {
            left.started_at
                .cmp(&right.started_at)
                .then_with(|| left.parent_span_id.is_some().cmp(&right.parent_span_id.is_some()))
                .then_with(|| left.span_id.cmp(&right.span_id))
        });
        spans
    }
}
