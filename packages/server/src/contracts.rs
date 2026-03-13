use indexmap::IndexMap;
use mastra_core::{MemoryMessage, Thread, Tool};
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
pub struct MemorySummary {
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolSummary {
    pub id: String,
    pub description: String,
    #[serde(default)]
    pub require_approval: bool,
    #[serde(default)]
    pub input_schema: Option<Value>,
    #[serde(default)]
    pub output_schema: Option<Value>,
}

impl ToolSummary {
    pub fn from_tool(tool: &Tool) -> Self {
        Self {
            id: tool.id().to_owned(),
            description: tool.description().to_owned(),
            require_approval: tool.requires_approval(),
            input_schema: tool.input_schema().cloned(),
            output_schema: tool.output_schema().cloned(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentDetail {
    pub id: String,
    pub name: String,
    pub instructions: String,
    pub description: Option<String>,
    pub tools: Vec<ToolSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentDetailResponse {
    pub agent: AgentDetail,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkflowStepSummary {
    pub id: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkflowDetail {
    pub id: String,
    pub description: Option<String>,
    pub steps: Vec<WorkflowStepSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkflowDetailResponse {
    pub workflow: WorkflowDetail,
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
    #[serde(alias = "resourceId")]
    pub resource_id: Option<String>,
    #[serde(default)]
    #[serde(alias = "threadId")]
    pub thread_id: Option<String>,
    #[serde(default)]
    #[serde(alias = "runId")]
    pub run_id: Option<String>,
    #[serde(default)]
    #[serde(alias = "maxSteps")]
    pub max_steps: Option<u32>,
    #[serde(default)]
    #[serde(alias = "requestContext")]
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
pub struct GenerateStreamStartEvent {
    pub run_id: String,
    pub message_id: String,
    #[serde(default)]
    pub thread_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GenerateStreamTextDeltaEvent {
    pub run_id: String,
    pub message_id: String,
    pub delta: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GenerateStreamToolCallEvent {
    pub run_id: String,
    pub message_id: String,
    pub tool_call_id: String,
    pub tool_name: String,
    pub input: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GenerateStreamToolResultEvent {
    pub run_id: String,
    pub message_id: String,
    pub tool_call_id: String,
    pub tool_name: String,
    pub output: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GenerateStreamFinishEvent {
    pub run_id: String,
    pub message_id: String,
    #[serde(default)]
    pub thread_id: Option<String>,
    pub text: String,
    #[serde(default)]
    pub finish_reason: FinishReason,
    #[serde(default)]
    pub usage: Option<UsageStats>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GenerateStreamEvent {
    Start(GenerateStreamStartEvent),
    TextDelta(GenerateStreamTextDeltaEvent),
    ToolCall(GenerateStreamToolCallEvent),
    ToolResult(GenerateStreamToolResultEvent),
    Finish(GenerateStreamFinishEvent),
    Error(ErrorResponse),
}

impl GenerateStreamEvent {
    pub fn event_name(&self) -> &'static str {
        match self {
            Self::Start(_) => "start",
            Self::TextDelta(_) => "text_delta",
            Self::ToolCall(_) => "tool_call",
            Self::ToolResult(_) => "tool_result",
            Self::Finish(_) => "finish",
            Self::Error(_) => "error",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CreateWorkflowRunRequest {
    #[serde(default)]
    #[serde(alias = "resourceId")]
    pub resource_id: Option<String>,
    #[serde(default)]
    #[serde(alias = "inputData")]
    pub input_data: Option<Value>,
    #[serde(default)]
    #[serde(alias = "requestContext")]
    pub request_context: IndexMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StartWorkflowRunRequest {
    #[serde(default)]
    #[serde(alias = "resourceId")]
    pub resource_id: Option<String>,
    #[serde(default)]
    #[serde(alias = "inputData")]
    pub input_data: Option<Value>,
    #[serde(default)]
    #[serde(alias = "requestContext")]
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ListMemoriesResponse {
    pub memories: Vec<MemorySummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ListToolsResponse {
    pub tools: Vec<ToolSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExecuteToolRequest {
    pub data: Value,
    #[serde(default)]
    pub approved: bool,
    #[serde(default)]
    pub run_id: Option<String>,
    #[serde(default)]
    pub thread_id: Option<String>,
    #[serde(default)]
    pub request_context: IndexMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExecuteToolResponse {
    pub tool_id: String,
    pub output: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StartWorkflowRunResponse {
    pub run: WorkflowRunRecord,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ListWorkflowRunsResponse {
    pub runs: Vec<WorkflowRunRecord>,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CreateMemoryThreadRequest {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    #[serde(alias = "resourceId")]
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
pub struct GetMemoryThreadResponse {
    pub thread: Thread,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CloneMemoryThreadRequest {
    #[serde(default)]
    #[serde(alias = "newThreadId")]
    pub new_thread_id: Option<String>,
    #[serde(default)]
    #[serde(alias = "resourceId")]
    pub resource_id: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub metadata: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CloneMemoryThreadResponse {
    pub thread: Thread,
    pub cloned_messages: Vec<MemoryMessage>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ListThreadsResponse {
    pub threads: Vec<Thread>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MemoryMessageRole {
    System,
    User,
    Assistant,
    Tool,
}

impl From<MemoryMessageRole> for mastra_core::MemoryRole {
    fn from(value: MemoryMessageRole) -> Self {
        match value {
            MemoryMessageRole::System => Self::System,
            MemoryMessageRole::User => Self::User,
            MemoryMessageRole::Assistant => Self::Assistant,
            MemoryMessageRole::Tool => Self::Tool,
        }
    }
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

impl DeleteMemoryMessagesInput {
    pub fn into_ids(self) -> Vec<String> {
        match self {
            Self::MessageId(id) => vec![id],
            Self::MessageIds(ids) => ids,
            Self::Message(reference) => vec![reference.id],
            Self::Messages(references) => references
                .into_iter()
                .map(|reference| reference.id)
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeleteMemoryMessagesRequest {
    #[serde(alias = "messageIds")]
    pub message_ids: DeleteMemoryMessagesInput,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeleteMemoryMessagesResponse {
    pub success: bool,
    pub message: String,
    pub deleted: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ListMemoryMessagesResponse {
    pub messages: Vec<MemoryMessage>,
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{DeleteMemoryMessagesRequest, GenerateRequest, StartWorkflowRunRequest};

    #[test]
    fn generate_request_deserializes_official_camel_case_fields() {
        let request: GenerateRequest = serde_json::from_value(json!({
            "messages": [{ "role": "user", "content": "hello" }],
            "resourceId": "resource-1",
            "threadId": "thread-1",
            "runId": "run-1",
            "maxSteps": 3,
            "requestContext": {
                "tenant": "acme"
            }
        }))
        .expect("request should deserialize");

        assert_eq!(request.resource_id.as_deref(), Some("resource-1"));
        assert_eq!(request.thread_id.as_deref(), Some("thread-1"));
        assert_eq!(request.run_id.as_deref(), Some("run-1"));
        assert_eq!(request.max_steps, Some(3));
        assert_eq!(request.request_context["tenant"], json!("acme"));
    }

    #[test]
    fn workflow_request_deserializes_official_camel_case_fields() {
        let request: StartWorkflowRunRequest = serde_json::from_value(json!({
            "resourceId": "resource-7",
            "inputData": { "topic": "rust" },
            "requestContext": {
                "trace_id": "trace-1"
            }
        }))
        .expect("request should deserialize");

        assert_eq!(request.resource_id.as_deref(), Some("resource-7"));
        assert_eq!(request.input_data, Some(json!({ "topic": "rust" })));
        assert_eq!(request.request_context["trace_id"], json!("trace-1"));
    }

    #[test]
    fn delete_memory_messages_request_accepts_string_or_object_collections() {
        let single: DeleteMemoryMessagesRequest = serde_json::from_value(json!({
            "messageIds": "message-1"
        }))
        .expect("single string id should deserialize");

        assert_eq!(single.message_ids.into_ids(), vec!["message-1".to_owned()]);

        let objects: DeleteMemoryMessagesRequest = serde_json::from_value(json!({
            "messageIds": [
                { "id": "message-2" },
                { "id": "message-3" }
            ]
        }))
        .expect("object list should deserialize");

        assert_eq!(
            objects.message_ids.into_ids(),
            vec!["message-2".to_owned(), "message-3".to_owned()]
        );
    }
}
