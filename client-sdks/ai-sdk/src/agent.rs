use async_trait::async_trait;
use mastra_client_sdks_client_js::{AgentClient, AgentMessages, ChatMessage, GenerateRequest};
use uuid::Uuid;

use crate::{
    AiSdkError, AiSdkEvent, AiSdkFinishEvent, AiSdkGenerateRequest, AiSdkMessage, AiSdkRole,
    AiSdkRun, AiSdkStartEvent, AiSdkTextDeltaEvent,
};

#[async_trait]
pub trait AiSdkEventSource: Send + Sync {
    async fn generate(&self, request: AiSdkGenerateRequest) -> Result<AiSdkRun, AiSdkError>;
}

#[derive(Debug, Clone)]
pub struct AiSdkAgent {
    agent: AgentClient,
}

impl AiSdkAgent {
    pub fn new(agent: AgentClient) -> Self {
        Self { agent }
    }

    pub async fn generate(&self, request: AiSdkGenerateRequest) -> Result<AiSdkRun, AiSdkError> {
        <Self as AiSdkEventSource>::generate(self, request).await
    }

    pub async fn generate_text(&self, prompt: impl Into<String>) -> Result<AiSdkRun, AiSdkError> {
        self.generate(AiSdkGenerateRequest {
            messages: vec![AiSdkMessage::new(AiSdkRole::User, prompt)],
            ..Default::default()
        })
        .await
    }
}

#[async_trait]
impl AiSdkEventSource for AiSdkAgent {
    async fn generate(&self, request: AiSdkGenerateRequest) -> Result<AiSdkRun, AiSdkError> {
        if request.messages.is_empty() {
            return Err(AiSdkError::Validation(
                "at least one message is required".to_owned(),
            ));
        }

        let run_id = request.run_id.unwrap_or_else(|| Uuid::now_v7().to_string());
        let assistant_message_id = Uuid::now_v7().to_string();
        let response = self
            .agent
            .generate(GenerateRequest {
                messages: AgentMessages::Messages(
                    request
                        .messages
                        .iter()
                        .map(|message| ChatMessage {
                            role: message.role.as_api_role().to_owned(),
                            content: message.content.clone(),
                        })
                        .collect(),
                ),
                resource_id: request.resource_id.clone(),
                thread_id: request.thread_id.clone(),
                run_id: Some(run_id.clone()),
                max_steps: request.max_steps,
                request_context: request.request_context.clone(),
            })
            .await
            .map_err(AiSdkError::Client)?;

        let assistant_message = AiSdkMessage::with_id(
            assistant_message_id.clone(),
            AiSdkRole::Assistant,
            response.text.clone(),
        );
        let mut events = vec![AiSdkEvent::Start(AiSdkStartEvent {
            run_id: run_id.clone(),
            message_id: assistant_message_id.clone(),
            thread_id: request.thread_id,
        })];

        if !assistant_message.content.is_empty() {
            events.push(AiSdkEvent::TextDelta(AiSdkTextDeltaEvent {
                run_id: run_id.clone(),
                message_id: assistant_message_id,
                delta: assistant_message.content.clone(),
            }));
        }

        events.push(AiSdkEvent::Finish(AiSdkFinishEvent {
            run_id: run_id.clone(),
            message: assistant_message.clone(),
            finish_reason: response.finish_reason.clone(),
            usage: response.usage.clone(),
        }));

        Ok(AiSdkRun {
            run_id,
            assistant_message,
            events,
            raw: response,
        })
    }
}
