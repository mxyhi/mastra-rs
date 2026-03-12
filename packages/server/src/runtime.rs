use async_trait::async_trait;
use mastra_core::{Agent, AgentGenerateRequest, RequestContext, Workflow};
use serde_json::Value;

use crate::{
    contracts::{
        AgentSummary, GenerateRequest, GenerateResponse, StartWorkflowRunRequest, WorkflowSummary,
    },
    error::{ServerError, ServerResult},
};

#[async_trait]
pub trait AgentRuntime: Send + Sync {
    fn summary(&self) -> AgentSummary;

    async fn generate(&self, request: GenerateRequest) -> ServerResult<GenerateResponse>;
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

        Ok(GenerateResponse {
            text: response.text,
            finish_reason: crate::contracts::FinishReason::Stop,
            usage: Some(crate::contracts::UsageStats {
                prompt_tokens: 0,
                completion_tokens: 0,
            }),
        })
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
