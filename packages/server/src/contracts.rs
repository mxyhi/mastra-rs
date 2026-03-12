use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentSummary {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkflowSummary {
    pub id: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum AgentMessages {
    Text(String),
    Messages(Vec<ChatMessage>),
}

impl AgentMessages {
    pub fn flatten_text(&self) -> String {
        match self {
            Self::Text(text) => text.clone(),
            Self::Messages(messages) => messages
                .iter()
                .map(|message| message.content.as_str())
                .collect::<Vec<_>>()
                .join("\n"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UsageStats {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum FinishReason {
    #[default]
    Stop,
    ToolCall,
    Length,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GenerateRequest {
    pub messages: AgentMessages,
    #[serde(default)]
    pub resource_id: Option<String>,
    #[serde(default)]
    pub thread_id: Option<String>,
    #[serde(default)]
    pub run_id: Option<String>,
    #[serde(default)]
    pub max_steps: Option<u32>,
    #[serde(default)]
    pub request_context: IndexMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GenerateResponse {
    pub text: String,
    #[serde(default)]
    pub finish_reason: FinishReason,
    #[serde(default)]
    pub usage: Option<UsageStats>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CreateWorkflowRunRequest {
    #[serde(default)]
    pub resource_id: Option<String>,
    #[serde(default)]
    pub input_data: Option<Value>,
    #[serde(default)]
    pub request_context: IndexMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StartWorkflowRunRequest {
    #[serde(default)]
    pub resource_id: Option<String>,
    #[serde(default)]
    pub input_data: Option<Value>,
    #[serde(default)]
    pub request_context: IndexMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowRunStatus {
    Created,
    Running,
    Success,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkflowRunRecord {
    pub run_id: Uuid,
    pub workflow_id: String,
    pub status: WorkflowRunStatus,
    #[serde(default)]
    pub resource_id: Option<String>,
    #[serde(default)]
    pub input_data: Option<Value>,
    #[serde(default)]
    pub result: Option<Value>,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ListAgentsResponse {
    pub agents: Vec<AgentSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ListWorkflowsResponse {
    pub workflows: Vec<WorkflowSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StartWorkflowRunResponse {
    pub run: WorkflowRunRecord,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ErrorResponse {
    pub error: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RouteDescription {
    pub method: &'static str,
    pub path: String,
    pub summary: &'static str,
}
