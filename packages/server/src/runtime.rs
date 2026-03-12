use async_stream::stream;
use async_trait::async_trait;
use futures::{StreamExt, stream::BoxStream};
use mastra_core::{
    Agent, AgentGenerateRequest, AgentStreamRequest, FinishReason as CoreFinishReason, ModelEvent,
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
        let response = self
            .agent
            .generate(AgentGenerateRequest {
                prompt: request.messages.flatten_text(),
                thread_id: request.thread_id,
                resource_id: request.resource_id,
                run_id: request.run_id,
                max_steps: request.max_steps,
                request_context: RequestContext::from_value_map(request.request_context),
            })
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
        let run_id = request
            .run_id
            .clone()
            .unwrap_or_else(|| Uuid::now_v7().to_string());
        let message_id = Uuid::now_v7().to_string();
        let upstream = self.agent.stream(AgentStreamRequest {
            prompt: request.messages.flatten_text(),
            thread_id: request.thread_id,
            resource_id: request.resource_id,
            run_id: request.run_id,
            max_steps: request.max_steps,
            request_context: RequestContext::from_value_map(request.request_context),
        });

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
