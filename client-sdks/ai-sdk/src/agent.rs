use async_trait::async_trait;
use futures::{StreamExt, stream::BoxStream};
use mastra_client_sdks_client_js::{
    AgentClient, AgentMessages, ChatMessage, GenerateRequest, GenerateResponse,
    GenerateStreamEvent,
};
use uuid::Uuid;

use crate::{
    AiSdkError, AiSdkEvent, AiSdkEventStream, AiSdkFinishEvent, AiSdkGenerateRequest,
    AiSdkMessage, AiSdkRole, AiSdkRun, AiSdkStartEvent, AiSdkTextDeltaEvent,
    AssistantMessageAccumulator,
};

#[async_trait]
pub trait AiSdkEventSource: Send + Sync {
    async fn stream(&self, request: AiSdkGenerateRequest) -> Result<AiSdkEventStream, AiSdkError>;

    async fn generate(&self, request: AiSdkGenerateRequest) -> Result<AiSdkRun, AiSdkError> {
        let mut stream = self.stream(request).await?;
        let mut events = Vec::new();
        let mut accumulator = AssistantMessageAccumulator::default();

        while let Some(event) = stream.next().await {
            let event = event?;
            accumulator.apply(&event);
            events.push(event);
        }

        let run_id = accumulator
            .run_id()
            .map(str::to_owned)
            .unwrap_or_else(|| Uuid::now_v7().to_string());
        let assistant_message = accumulator
            .current_message()
            .cloned()
            .ok_or_else(|| AiSdkError::Validation("assistant stream returned no final message".to_owned()))?;
        let raw = GenerateResponse {
            text: assistant_message.content.clone(),
            finish_reason: accumulator
                .finish_reason()
                .cloned()
                .unwrap_or_default(),
            usage: accumulator.usage().cloned(),
        };

        Ok(AiSdkRun {
            run_id,
            assistant_message,
            events,
            raw,
        })
    }
}

#[derive(Debug, Clone)]
pub struct AiSdkAgent {
    agent: AgentClient,
}

impl AiSdkAgent {
    pub fn new(agent: AgentClient) -> Self {
        Self { agent }
    }

    pub async fn stream(
        &self,
        request: AiSdkGenerateRequest,
    ) -> Result<AiSdkEventStream, AiSdkError> {
        <Self as AiSdkEventSource>::stream(self, request).await
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
    async fn stream(&self, request: AiSdkGenerateRequest) -> Result<AiSdkEventStream, AiSdkError> {
        if request.messages.is_empty() {
            return Err(AiSdkError::Validation(
                "at least one message is required".to_owned(),
            ));
        }

        let stream = self
            .agent
            .stream(GenerateRequest {
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
                run_id: request.run_id.clone(),
                max_steps: request.max_steps,
                request_context: request.request_context.clone(),
            })
            .await
            .map_err(AiSdkError::Client)?;

        let mapped: BoxStream<'static, Result<AiSdkEvent, AiSdkError>> = stream
            .map(|event| {
                event
                    .map_err(AiSdkError::Client)
                    .and_then(map_stream_event)
            })
            .boxed();

        Ok(mapped)
    }
}

fn map_stream_event(event: GenerateStreamEvent) -> Result<AiSdkEvent, AiSdkError> {
    match event {
        GenerateStreamEvent::Start(start) => Ok(AiSdkEvent::Start(AiSdkStartEvent {
            run_id: start.run_id,
            message_id: start.message_id,
            thread_id: start.thread_id,
        })),
        GenerateStreamEvent::TextDelta(delta) => Ok(AiSdkEvent::TextDelta(AiSdkTextDeltaEvent {
            run_id: delta.run_id,
            message_id: delta.message_id,
            delta: delta.delta,
        })),
        GenerateStreamEvent::Finish(finish) => Ok(AiSdkEvent::Finish(AiSdkFinishEvent {
            run_id: finish.run_id,
            message: AiSdkMessage::with_id(finish.message_id, AiSdkRole::Assistant, finish.text),
            finish_reason: finish.finish_reason,
            usage: finish.usage,
        })),
        GenerateStreamEvent::Error(error) => Err(AiSdkError::Validation(error.error)),
    }
}
