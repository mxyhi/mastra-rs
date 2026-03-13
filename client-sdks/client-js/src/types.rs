use chrono::{DateTime, Utc};
use indexmap::IndexMap;
use mastra_core::{MemoryMessage, Thread};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

pub use mastra_server::{
    AgentMessages, AgentSummary, ChatMessage, ErrorResponse, FinishReason, GenerateResponse,
    GenerateStreamEvent, GenerateStreamFinishEvent, GenerateStreamStartEvent,
    GenerateStreamTextDeltaEvent, GenerateStreamToolCallEvent, GenerateStreamToolResultEvent,
    ListWorkflowRunsResponse, UsageStats, WorkflowRunRecord, WorkflowRunStatus,
    WorkflowStreamEvent, WorkflowStreamFinishEvent, WorkflowStreamStartEvent,
    WorkflowStreamStepEvent, WorkflowSummary,
};

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MemorySummary {
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ListAgentsResponse {
    pub agents: Vec<AgentSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ListWorkflowsResponse {
    pub workflows: Vec<WorkflowSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ListMemoriesResponse {
    pub memories: Vec<MemorySummary>,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StartWorkflowRunResponse {
    pub run: WorkflowRunRecord,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CreateMemoryThreadRequest {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub resource_id: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CreateMemoryThreadResponse {
    pub thread: Thread,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ListThreadsResponse {
    pub threads: Vec<Thread>,
    pub total: usize,
    pub page: usize,
    pub per_page: usize,
    pub has_more: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListThreadsQuery {
    #[serde(default)]
    #[serde(rename = "page")]
    pub page: Option<usize>,
    #[serde(default)]
    #[serde(rename = "perPage")]
    pub per_page: Option<usize>,
    #[serde(default)]
    #[serde(rename = "resourceId")]
    pub resource_id: Option<String>,
    #[serde(default)]
    pub metadata: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MemoryMessageRole {
    System,
    User,
    Assistant,
    Tool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MemoryMessageInput {
    pub role: MemoryMessageRole,
    pub content: String,
    #[serde(default)]
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AppendMemoryMessagesRequest {
    pub messages: Vec<MemoryMessageInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AppendMemoryMessagesResponse {
    pub thread_id: String,
    pub appended: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ListMemoryMessagesResponse {
    pub messages: Vec<MemoryMessage>,
    pub total: usize,
    pub page: usize,
    pub per_page: usize,
    pub has_more: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ListMessagesQuery {
    #[serde(default)]
    #[serde(rename = "page")]
    pub page: Option<usize>,
    #[serde(default)]
    #[serde(rename = "perPage")]
    pub per_page: Option<usize>,
    #[serde(default)]
    #[serde(rename = "resourceId")]
    pub resource_id: Option<String>,
    #[serde(default)]
    #[serde(rename = "messageIds")]
    pub message_ids: Option<Vec<String>>,
    #[serde(default)]
    #[serde(rename = "startDate")]
    pub start_date: Option<DateTime<Utc>>,
    #[serde(default)]
    #[serde(rename = "endDate")]
    pub end_date: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CloneMemoryThreadOptions {
    #[serde(default)]
    #[serde(rename = "messageLimit")]
    pub message_limit: Option<usize>,
    #[serde(default)]
    #[serde(rename = "messageFilter")]
    pub message_filter: Option<CloneMemoryThreadMessageFilter>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CloneMemoryThreadMessageFilter {
    #[serde(default)]
    #[serde(rename = "startDate")]
    pub start_date: Option<DateTime<Utc>>,
    #[serde(default)]
    #[serde(rename = "endDate")]
    pub end_date: Option<DateTime<Utc>>,
    #[serde(default)]
    #[serde(rename = "messageIds")]
    pub message_ids: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CloneMemoryThreadRequest {
    #[serde(default)]
    #[serde(rename = "newThreadId")]
    pub new_thread_id: Option<String>,
    #[serde(default)]
    #[serde(rename = "resourceId")]
    pub resource_id: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub metadata: Option<Value>,
    #[serde(default)]
    #[serde(rename = "messageLimit")]
    pub message_limit: Option<usize>,
    #[serde(default)]
    pub options: Option<CloneMemoryThreadOptions>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CloneMemoryThreadResponse {
    pub thread: Thread,
    pub cloned_messages: Vec<MemoryMessage>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MessageIdReference {
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum DeleteMemoryMessagesInput {
    MessageId(String),
    MessageIds(Vec<String>),
    Message(MessageIdReference),
    Messages(Vec<MessageIdReference>),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeleteMemoryMessagesRequest {
    #[serde(rename = "messageIds")]
    pub message_ids: DeleteMemoryMessagesInput,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeleteMemoryMessagesResponse {
    pub success: bool,
    pub message: String,
    pub deleted: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RouteDescription {
    pub method: &'static str,
    pub path: String,
    pub summary: &'static str,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SystemPackage {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SystemPackagesResponse {
    pub packages: Vec<SystemPackage>,
    pub is_dev: bool,
    pub cms_enabled: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkflowStreamQuery {
    #[serde(default)]
    #[serde(rename = "runId")]
    pub run_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkflowRunRef {
    pub run_id: Uuid,
}
