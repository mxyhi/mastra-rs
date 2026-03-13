use async_stream::stream;
use async_trait::async_trait;
use futures::{StreamExt, stream::BoxStream};
use mastra_core::{
    Agent, AgentContextMessage, AgentGenerateRequest, AgentStreamRequest, AgentToolChoice,
    AgentToolChoiceMode, FinishReason as CoreFinishReason, ModelEvent,
    ModelResponse as CoreModelResponse, RequestContext, Tool, ToolExecutionContext,
    UsageStats as CoreUsageStats, Workflow,
};
use serde_json::Value;
use uuid::Uuid;

use crate::{
    contracts::{
        AgentDetail, AgentSummary, ExecuteToolRequest, ExecuteToolResponse, FinishReason,
        GenerateRequest, GenerateResponse, GenerateStreamEvent, GenerateStreamFinishEvent,
        GenerateStreamStartEvent, GenerateStreamTextDeltaEvent, GenerateStreamToolCallEvent,
        GenerateStreamToolResultEvent, StartWorkflowRunRequest, ToolSummary, UsageStats,
        WorkflowDetail, WorkflowStepSummary, WorkflowSummary,
    },
    error::{ServerError, ServerResult},
};

#[async_trait]
pub trait AgentRuntime: Send + Sync {
    fn summary(&self) -> AgentSummary;

    fn detail(&self) -> AgentDetail {
        let summary = self.summary();
        AgentDetail {
            id: summary.id,
            name: summary.name,
            instructions: String::new(),
            description: summary.description,
            tools: self.tool_summaries(),
        }
    }

    fn tools(&self) -> Vec<Tool> {
        Vec::new()
    }

    fn tool_summaries(&self) -> Vec<ToolSummary> {
        self.tools().iter().map(ToolSummary::from_tool).collect()
    }

    async fn generate(&self, request: GenerateRequest) -> ServerResult<GenerateResponse>;

    fn stream(
        &self,
        request: GenerateRequest,
    ) -> BoxStream<'static, ServerResult<GenerateStreamEvent>>;

    async fn execute_tool(
        &self,
        tool_id: &str,
        request: ExecuteToolRequest,
    ) -> ServerResult<ExecuteToolResponse> {
        let tool = self
            .tools()
            .into_iter()
            .find(|tool| tool.id() == tool_id)
            .ok_or_else(|| ServerError::NotFound {
                resource: "tool",
                id: tool_id.to_owned(),
            })?;

        let output = tool
            .execute(
                request.data,
                ToolExecutionContext {
                    request_context: RequestContext::from_value_map(request.request_context),
                    run_id: request.run_id,
                    thread_id: request.thread_id,
                    approved: request.approved,
                },
            )
            .await
            .map_err(ServerError::internal)?;

        Ok(ExecuteToolResponse {
            tool_id: tool_id.to_owned(),
            output,
        })
    }
}

#[async_trait]
pub trait WorkflowRuntime: Send + Sync {
    fn summary(&self) -> WorkflowSummary;

    fn detail(&self) -> WorkflowDetail {
        let summary = self.summary();
        WorkflowDetail {
            id: summary.id,
            description: summary.description,
            steps: Vec::new(),
        }
    }

    async fn start(&self, request: StartWorkflowRunRequest) -> ServerResult<Value>;
}

#[derive(Clone)]
pub struct CoreAgentRuntime {
    agent: Agent,
}

impl CoreAgentRuntime {
    pub fn new(agent: Agent) -> Self {
        Self { agent }
    }

    fn map_generate_request(&self, request: GenerateRequest) -> ServerResult<AgentGenerateRequest> {
        let mapped = map_request_parts(request)?;
        Ok(AgentGenerateRequest {
            prompt: mapped.prompt,
            thread_id: mapped.thread_id,
            resource_id: mapped.resource_id,
            run_id: mapped.run_id,
            max_steps: mapped.max_steps,
            instructions_override: mapped.instructions_override,
            system: mapped.system,
            context: mapped.context,
            disable_memory: mapped.disable_memory,
            memory_read_only: mapped.memory_read_only,
            active_tools: mapped.active_tools,
            tool_choice: mapped.tool_choice,
            output_schema: mapped.output_schema,
            request_context: mapped.request_context,
        })
    }

    fn map_stream_request(&self, request: GenerateRequest) -> ServerResult<AgentStreamRequest> {
        let mapped = map_request_parts(request)?;
        Ok(AgentStreamRequest {
            prompt: mapped.prompt,
            thread_id: mapped.thread_id,
            resource_id: mapped.resource_id,
            run_id: mapped.run_id,
            max_steps: mapped.max_steps,
            instructions_override: mapped.instructions_override,
            system: mapped.system,
            context: mapped.context,
            disable_memory: mapped.disable_memory,
            memory_read_only: mapped.memory_read_only,
            active_tools: mapped.active_tools,
            tool_choice: mapped.tool_choice,
            output_schema: mapped.output_schema,
            request_context: mapped.request_context,
        })
    }
}

#[async_trait]
impl AgentRuntime for CoreAgentRuntime {
    fn summary(&self) -> AgentSummary {
        AgentSummary {
            id: self.agent.id().to_string(),
            name: self.agent.name().to_string(),
            description: self.agent.description().map(str::to_string),
        }
    }

    fn detail(&self) -> AgentDetail {
        AgentDetail {
            id: self.agent.id().to_string(),
            name: self.agent.name().to_string(),
            instructions: self.agent.instructions().to_string(),
            description: self.agent.description().map(str::to_string),
            tools: self.tool_summaries(),
        }
    }

    fn tools(&self) -> Vec<Tool> {
        self.agent.tools().to_vec()
    }

    async fn generate(&self, request: GenerateRequest) -> ServerResult<GenerateResponse> {
        let request = self.map_generate_request(request)?;
        let response = self
            .agent
            .generate(request)
            .await
            .map_err(ServerError::internal)?;

        Ok(GenerateResponse {
            text: response.text,
            finish_reason: map_finish_reason(response.finish_reason),
            usage: map_usage(response.usage),
        })
    }

    fn stream(
        &self,
        request: GenerateRequest,
    ) -> BoxStream<'static, ServerResult<GenerateStreamEvent>> {
        let request = match self.map_stream_request(request) {
            Ok(request) => request,
            Err(error) => return futures::stream::once(async { Err(error) }).boxed(),
        };
        let run_id = request
            .run_id
            .clone()
            .unwrap_or_else(|| Uuid::now_v7().to_string());
        let message_id = Uuid::now_v7().to_string();
        let upstream = self.agent.stream(request);

        stream! {
            let mut emitted_start = false;
            let mut emitted_delta = false;
            let mut last_thread_id = None;
            tokio::pin!(upstream);

            while let Some(event) = upstream.next().await {
                let event = event.map_err(ServerError::internal)?;
                if !emitted_start {
                    emitted_start = true;
                    last_thread_id = event.thread_id.clone();
                    yield Ok(GenerateStreamEvent::Start(GenerateStreamStartEvent {
                        run_id: run_id.clone(),
                        message_id: message_id.clone(),
                        thread_id: event.thread_id.clone(),
                    }));
                }

                match event.event {
                    ModelEvent::TextDelta(delta) => {
                        if delta.is_empty() {
                            continue;
                        }
                        emitted_delta = true;
                        yield Ok(GenerateStreamEvent::TextDelta(GenerateStreamTextDeltaEvent {
                            run_id: run_id.clone(),
                            message_id: message_id.clone(),
                            delta,
                        }));
                    }
                    ModelEvent::ToolCall(call) => {
                        yield Ok(GenerateStreamEvent::ToolCall(GenerateStreamToolCallEvent {
                            run_id: run_id.clone(),
                            message_id: message_id.clone(),
                            tool_call_id: call.id,
                            tool_name: call.name,
                            input: call.input,
                        }));
                    }
                    ModelEvent::ToolResult(result) => {
                        yield Ok(GenerateStreamEvent::ToolResult(GenerateStreamToolResultEvent {
                            run_id: run_id.clone(),
                            message_id: message_id.clone(),
                            tool_call_id: result.id,
                            tool_name: result.name,
                            output: result.output,
                        }));
                    }
                    ModelEvent::Done(response) => {
                        let normalized = generate_response_from_model_response(&response);
                        if !emitted_delta && !normalized.text.is_empty() {
                            yield Ok(GenerateStreamEvent::TextDelta(GenerateStreamTextDeltaEvent {
                                run_id: run_id.clone(),
                                message_id: message_id.clone(),
                                delta: normalized.text.clone(),
                            }));
                        }
                        yield Ok(GenerateStreamEvent::Finish(GenerateStreamFinishEvent {
                            run_id: run_id.clone(),
                            message_id: message_id.clone(),
                            thread_id: event.thread_id.clone().or(last_thread_id.clone()),
                            text: normalized.text,
                            finish_reason: normalized.finish_reason,
                            usage: normalized.usage,
                        }));
                    }
                }
            }
        }
        .boxed()
    }
}

#[derive(Clone)]
pub struct CoreWorkflowRuntime {
    workflow: Workflow,
}

impl CoreWorkflowRuntime {
    pub fn new(workflow: Workflow) -> Self {
        Self { workflow }
    }
}

#[async_trait]
impl WorkflowRuntime for CoreWorkflowRuntime {
    fn summary(&self) -> WorkflowSummary {
        WorkflowSummary {
            id: self.workflow.id().to_string(),
            description: None,
        }
    }

    fn detail(&self) -> WorkflowDetail {
        WorkflowDetail {
            id: self.workflow.id().to_string(),
            description: None,
            steps: self
                .workflow
                .steps()
                .iter()
                .map(|step| WorkflowStepSummary {
                    id: step.id().to_string(),
                    description: step.description().map(str::to_string),
                })
                .collect(),
        }
    }

    async fn start(&self, request: StartWorkflowRunRequest) -> ServerResult<Value> {
        let input = request.input_data.unwrap_or(Value::Null);
        let result = self
            .workflow
            .run(
                input,
                RequestContext::from_value_map(request.request_context),
            )
            .await
            .map_err(ServerError::internal)?;
        Ok(result.output)
    }
}

fn generate_response_from_model_response(response: &CoreModelResponse) -> GenerateResponse {
    GenerateResponse {
        text: response.text.clone(),
        finish_reason: map_finish_reason(extract_finish_reason(response)),
        usage: map_usage(extract_usage(response)),
    }
}

struct CoreAgentRequestParts {
    prompt: String,
    thread_id: Option<String>,
    resource_id: Option<String>,
    run_id: Option<String>,
    max_steps: Option<u32>,
    instructions_override: Option<String>,
    system: Option<String>,
    context: Vec<AgentContextMessage>,
    disable_memory: bool,
    memory_read_only: bool,
    active_tools: Option<Vec<String>>,
    tool_choice: Option<AgentToolChoice>,
    output_schema: Option<Value>,
    request_context: RequestContext,
}

fn map_request_parts(request: GenerateRequest) -> ServerResult<CoreAgentRequestParts> {
    let GenerateRequest {
        messages,
        instructions,
        system,
        context,
        memory,
        resource_id,
        thread_id,
        run_id,
        max_steps,
        active_tools,
        tool_choice,
        output,
        request_context,
    } = request;

    if let Some(key) = memory.as_ref().and_then(|config| config.key()) {
        return Err(ServerError::BadRequest(format!(
            "memory.key override is not supported in the current Rust runtime: {key}"
        )));
    }

    let memory_thread_id = memory
        .as_ref()
        .and_then(crate::contracts::GenerateMemoryConfig::thread_id)
        .map(str::to_owned);
    let memory_resource_id = memory
        .as_ref()
        .and_then(crate::contracts::GenerateMemoryConfig::resource)
        .map(str::to_owned);

    Ok(CoreAgentRequestParts {
        prompt: messages.flatten_text(),
        thread_id: memory_thread_id.or(thread_id),
        resource_id: memory_resource_id.or(resource_id),
        run_id,
        max_steps,
        instructions_override: instructions,
        system,
        context: context
            .into_iter()
            .map(|message| AgentContextMessage {
                role: message.role,
                content: message.content,
            })
            .collect(),
        disable_memory: memory
            .as_ref()
            .is_some_and(crate::contracts::GenerateMemoryConfig::disables_memory),
        memory_read_only: memory
            .as_ref()
            .is_some_and(crate::contracts::GenerateMemoryConfig::read_only),
        active_tools,
        tool_choice: tool_choice.map(map_tool_choice),
        output_schema: output,
        request_context: RequestContext::from_value_map(request_context),
    })
}

fn map_tool_choice(choice: crate::contracts::ToolChoice) -> AgentToolChoice {
    match choice {
        crate::contracts::ToolChoice::Mode(crate::contracts::ToolChoiceMode::Auto) => {
            AgentToolChoice::Mode(AgentToolChoiceMode::Auto)
        }
        crate::contracts::ToolChoice::Mode(crate::contracts::ToolChoiceMode::None) => {
            AgentToolChoice::Mode(AgentToolChoiceMode::None)
        }
        crate::contracts::ToolChoice::Mode(crate::contracts::ToolChoiceMode::Required) => {
            AgentToolChoice::Mode(AgentToolChoiceMode::Required)
        }
        crate::contracts::ToolChoice::Tool(tool) => AgentToolChoice::tool(tool.tool_name),
    }
}

fn extract_finish_reason(response: &CoreModelResponse) -> CoreFinishReason {
    response.normalized_finish_reason()
}

fn extract_usage(response: &CoreModelResponse) -> Option<CoreUsageStats> {
    response.normalized_usage()
}

fn map_finish_reason(value: CoreFinishReason) -> FinishReason {
    match value {
        CoreFinishReason::Stop => FinishReason::Stop,
        CoreFinishReason::ToolCall => FinishReason::ToolCall,
        CoreFinishReason::Length => FinishReason::Length,
    }
}

fn map_usage(value: Option<CoreUsageStats>) -> Option<UsageStats> {
    let usage = value?;
    Some(UsageStats {
        prompt_tokens: usage.prompt_tokens,
        completion_tokens: usage.completion_tokens,
    })
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, sync::Arc};

    use async_trait::async_trait;
    use chrono::Utc;
    use indexmap::IndexMap;
    use mastra_core::{
        Agent, AgentConfig, CreateThreadRequest, MemoryConfig, MemoryEngine, MemoryMessage,
        MemoryRecallRequest, MemoryRole, ModelRequest, ModelResponse, StaticModel, Thread, Tool,
    };
    use parking_lot::RwLock;
    use serde_json::{Value, json};

    use super::{AgentRuntime, CoreAgentRuntime};
    use crate::contracts::{ChatMessage, GenerateMemoryConfig, GenerateRequest, ToolChoice};
    use crate::error::ServerError;

    #[derive(Default)]
    struct TestMemory {
        threads: RwLock<HashMap<String, Thread>>,
        messages: RwLock<HashMap<String, Vec<MemoryMessage>>>,
    }

    impl TestMemory {
        fn message_count(&self, thread_id: &str) -> usize {
            self.messages
                .read()
                .get(thread_id)
                .map(Vec::len)
                .unwrap_or_default()
        }
    }

    #[async_trait]
    impl MemoryEngine for TestMemory {
        async fn create_thread(&self, request: CreateThreadRequest) -> mastra_core::Result<Thread> {
            let now = Utc::now();
            let thread = Thread {
                id: request
                    .id
                    .unwrap_or_else(|| uuid::Uuid::now_v7().to_string()),
                resource_id: request.resource_id,
                title: request.title,
                created_at: now,
                updated_at: now,
                metadata: request.metadata,
            };
            self.threads
                .write()
                .insert(thread.id.clone(), thread.clone());
            self.messages.write().entry(thread.id.clone()).or_default();
            Ok(thread)
        }

        async fn get_thread(&self, thread_id: &str) -> mastra_core::Result<Option<Thread>> {
            Ok(self.threads.read().get(thread_id).cloned())
        }

        async fn list_threads(
            &self,
            _resource_id: Option<&str>,
        ) -> mastra_core::Result<Vec<Thread>> {
            Ok(self.threads.read().values().cloned().collect())
        }

        async fn append_messages(
            &self,
            thread_id: &str,
            messages: Vec<MemoryMessage>,
        ) -> mastra_core::Result<()> {
            self.messages
                .write()
                .entry(thread_id.to_owned())
                .or_default()
                .extend(messages);
            Ok(())
        }

        async fn list_messages(
            &self,
            request: MemoryRecallRequest,
        ) -> mastra_core::Result<Vec<MemoryMessage>> {
            Ok(self
                .messages
                .read()
                .get(&request.thread_id)
                .cloned()
                .unwrap_or_default())
        }
    }

    #[tokio::test]
    async fn generate_maps_official_agent_request_fields_into_core_runtime() {
        let memory = Arc::new(TestMemory::default());
        let thread = memory
            .create_thread(CreateThreadRequest {
                id: Some("thread-1".to_owned()),
                resource_id: Some("resource-1".to_owned()),
                title: Some("Existing".to_owned()),
                metadata: Value::Null,
            })
            .await
            .expect("thread should be created");
        memory
            .append_messages(
                &thread.id,
                vec![MemoryMessage {
                    id: "message-1".to_owned(),
                    thread_id: thread.id.clone(),
                    role: MemoryRole::Assistant,
                    content: "stored memory".to_owned(),
                    created_at: Utc::now(),
                    metadata: Value::Null,
                }],
            )
            .await
            .expect("message should seed memory");

        let agent = Agent::new(AgentConfig {
            id: "echo".to_owned(),
            name: "Echo".to_owned(),
            instructions: "Base instructions".to_owned(),
            description: None,
            model: Arc::new(StaticModel::new(|request: ModelRequest| async move {
                Ok(ModelResponse {
                    text: json!({
                        "prompt": request.prompt,
                        "instructions": request.instructions,
                        "memory": request.memory,
                        "tools": request.tool_names,
                        "requestContext": request.request_context.values(),
                    })
                    .to_string(),
                    data: Value::Null,
                    finish_reason: mastra_core::FinishReason::Stop,
                    usage: None,
                    tool_calls: Vec::new(),
                })
            })),
            tools: vec![
                Tool::new("sum", "add numbers", |_input, _context| async move {
                    Ok(json!({ "ok": true }))
                }),
                Tool::new("ping", "ping tool", |_input, _context| async move {
                    Ok(json!({ "pong": true }))
                }),
            ],
            memory: Some(memory.clone()),
            memory_config: MemoryConfig::default(),
        });
        let runtime = CoreAgentRuntime::new(agent);

        let response = runtime
            .generate(GenerateRequest {
                messages: crate::contracts::AgentMessages::Text("hello".to_owned()),
                instructions: Some("Follow the request override".to_owned()),
                system: Some("System prompt".to_owned()),
                context: vec![ChatMessage {
                    role: "assistant".to_owned(),
                    content: "prior context".to_owned(),
                }],
                memory: Some(GenerateMemoryConfig::Enabled(false)),
                resource_id: Some("resource-1".to_owned()),
                thread_id: Some("thread-1".to_owned()),
                run_id: Some("run-1".to_owned()),
                max_steps: Some(2),
                active_tools: Some(vec!["sum".to_owned(), "ping".to_owned()]),
                tool_choice: Some(ToolChoice::tool("sum")),
                output: Some(json!({
                    "type": "object",
                    "properties": {
                        "answer": { "type": "string" }
                    }
                })),
                request_context: IndexMap::from([("tenant".to_owned(), json!("acme"))]),
            })
            .await
            .expect("generate should succeed");

        let payload: Value =
            serde_json::from_str(&response.text).expect("response text should be JSON");
        let prompt = payload["prompt"]
            .as_str()
            .expect("prompt should be serialized");
        assert!(prompt.contains("prior context"));
        assert!(prompt.contains("hello"));

        let instructions = payload["instructions"]
            .as_str()
            .expect("instructions should be serialized");
        assert!(instructions.contains("Base instructions"));
        assert!(instructions.contains("Follow the request override"));
        assert!(instructions.contains("System prompt"));
        assert!(instructions.contains("\"answer\""));
        assert_eq!(payload["tools"], json!(["sum"]));
        assert_eq!(payload["memory"], json!([]));
        assert_eq!(payload["requestContext"]["tenant"], json!("acme"));
        assert_eq!(memory.message_count("thread-1"), 1);
    }

    #[tokio::test]
    async fn generate_rejects_named_memory_override_until_runtime_supports_it() {
        let agent = Agent::new(AgentConfig {
            id: "echo".to_owned(),
            name: "Echo".to_owned(),
            instructions: "Base instructions".to_owned(),
            description: None,
            model: Arc::new(StaticModel::echo()),
            tools: Vec::new(),
            memory: None,
            memory_config: MemoryConfig::default(),
        });
        let runtime = CoreAgentRuntime::new(agent);

        let error = runtime
            .generate(GenerateRequest {
                messages: crate::contracts::AgentMessages::Text("hello".to_owned()),
                instructions: None,
                system: None,
                context: Vec::new(),
                memory: Some(GenerateMemoryConfig::Options(
                    crate::contracts::GenerateMemoryOptions {
                        key: Some("chat".to_owned()),
                        thread: None,
                        resource: None,
                        options: None,
                        read_only: None,
                        extra: IndexMap::new(),
                    },
                )),
                resource_id: None,
                thread_id: None,
                run_id: None,
                max_steps: Some(1),
                active_tools: None,
                tool_choice: None,
                output: None,
                request_context: IndexMap::new(),
            })
            .await
            .expect_err("named memory override should be rejected");

        assert!(
            matches!(error, ServerError::BadRequest(message) if message.contains("memory.key override is not supported"))
        );
    }

    #[tokio::test]
    async fn generate_maps_live_memory_thread_shape_and_honors_read_only() {
        let memory = Arc::new(TestMemory::default());
        let thread = memory
            .create_thread(CreateThreadRequest {
                id: Some("thread-1".to_owned()),
                resource_id: Some("resource-1".to_owned()),
                title: Some("Existing".to_owned()),
                metadata: Value::Null,
            })
            .await
            .expect("thread should be created");
        memory
            .append_messages(
                &thread.id,
                vec![MemoryMessage {
                    id: "message-1".to_owned(),
                    thread_id: thread.id.clone(),
                    role: MemoryRole::Assistant,
                    content: "stored memory".to_owned(),
                    created_at: Utc::now(),
                    metadata: Value::Null,
                }],
            )
            .await
            .expect("message should seed memory");

        let agent = Agent::new(AgentConfig {
            id: "echo".to_owned(),
            name: "Echo".to_owned(),
            instructions: "Base instructions".to_owned(),
            description: None,
            model: Arc::new(StaticModel::new(|request: ModelRequest| async move {
                Ok(ModelResponse {
                    text: json!({
                        "threadId": request.thread_id,
                        "resourceId": request.request_context.resource_id(),
                        "memory": request.memory,
                    })
                    .to_string(),
                    data: Value::Null,
                    finish_reason: mastra_core::FinishReason::Stop,
                    usage: None,
                    tool_calls: Vec::new(),
                })
            })),
            tools: Vec::new(),
            memory: Some(memory.clone()),
            memory_config: MemoryConfig::default(),
        });
        let runtime = CoreAgentRuntime::new(agent);

        let response = runtime
            .generate(GenerateRequest {
                messages: crate::contracts::AgentMessages::Text("hello".to_owned()),
                instructions: None,
                system: None,
                context: Vec::new(),
                memory: Some(GenerateMemoryConfig::Options(
                    crate::contracts::GenerateMemoryOptions {
                        key: None,
                        thread: Some(crate::contracts::GenerateMemoryThreadRef::Thread(
                            crate::contracts::GenerateMemoryThreadObject {
                                id: "thread-1".to_owned(),
                                extra: IndexMap::from([("title".to_owned(), json!("Existing"))]),
                            },
                        )),
                        resource: Some("resource-1".to_owned()),
                        options: Some(IndexMap::from([("readOnly".to_owned(), json!(true))])),
                        read_only: Some(true),
                        extra: IndexMap::new(),
                    },
                )),
                resource_id: None,
                thread_id: None,
                run_id: Some("run-2".to_owned()),
                max_steps: Some(1),
                active_tools: None,
                tool_choice: None,
                output: None,
                request_context: IndexMap::new(),
            })
            .await
            .expect("generate should succeed");

        let payload: Value =
            serde_json::from_str(&response.text).expect("response text should be JSON");
        assert_eq!(payload["threadId"], json!("thread-1"));
        assert_eq!(payload["memory"], json!(["Assistant: stored memory"]));
        assert_eq!(memory.message_count("thread-1"), 1);
    }
}
