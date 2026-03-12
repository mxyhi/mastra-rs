use async_trait::async_trait;
use async_stream::stream;
use futures::{StreamExt, stream::BoxStream};
use mastra_core::{
    Agent, AgentGenerateRequest, AgentStreamRequest, ModelEvent, RequestContext, Workflow,
};
use serde_json::Value;
use uuid::Uuid;

use crate::{
    contracts::{
        AgentSummary, FinishReason, GenerateRequest, GenerateResponse, GenerateStreamEvent,
        GenerateStreamFinishEvent, GenerateStreamStartEvent, GenerateStreamTextDeltaEvent,
        StartWorkflowRunRequest, UsageStats, WorkflowSummary,
    },
    error::{ServerError, ServerResult},
};

#[async_trait]
pub trait AgentRuntime: Send + Sync {
    fn summary(&self) -> AgentSummary;

    async fn generate(&self, request: GenerateRequest) -> ServerResult<GenerateResponse>;

    fn stream(
        &self,
        request: GenerateRequest,
    ) -> BoxStream<'static, ServerResult<GenerateStreamEvent>>;
}

#[async_trait]
pub trait WorkflowRuntime: Send + Sync {
    fn summary(&self) -> WorkflowSummary;

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

    async fn generate(&self, request: GenerateRequest) -> ServerResult<GenerateResponse> {
        let response = self
            .agent
            .generate(AgentGenerateRequest {
                prompt: request.messages.flatten_text(),
                thread_id: request.thread_id,
                resource_id: request.resource_id,
                request_context: RequestContext::from_value_map(request.request_context),
            })
            .await
            .map_err(ServerError::internal)?;

        Ok(generate_response_from_parts(response.text, &response.data))
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
                    ModelEvent::Done(response) => {
                        let normalized = generate_response_from_parts(
                            response.text.clone(),
                            &response.data,
                        );
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

fn generate_response_from_parts(text: String, data: &Value) -> GenerateResponse {
    GenerateResponse {
        text,
        finish_reason: extract_finish_reason(data),
        usage: extract_usage(data),
    }
}

fn extract_finish_reason(data: &Value) -> FinishReason {
    let Some(value) = data.get("finish_reason").and_then(Value::as_str) else {
        return FinishReason::Stop;
    };

    match value {
        "tool_call" | "tool_calls" => FinishReason::ToolCall,
        "length" => FinishReason::Length,
        _ => FinishReason::Stop,
    }
}

fn extract_usage(data: &Value) -> Option<UsageStats> {
    let usage = data.get("usage")?;
    let prompt_tokens = usage.get("prompt_tokens")?.as_u64()?;
    let completion_tokens = usage.get("completion_tokens")?.as_u64()?;
    Some(UsageStats {
        prompt_tokens: prompt_tokens as u32,
        completion_tokens: completion_tokens as u32,
    })
}
