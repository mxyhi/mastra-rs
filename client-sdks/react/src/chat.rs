use futures::StreamExt;
use mastra_client_sdks_ai_sdk::{
    AiSdkAgent, AiSdkError, AiSdkEvent, AiSdkEventSource, AiSdkGenerateRequest, AiSdkMessage,
    AiSdkRole, AiSdkRun, AssistantMessageAccumulator,
};

use crate::{ChatAction, ChatState};

#[derive(Debug)]
pub struct ChatController<S = AiSdkAgent> {
    source: S,
    state: ChatState,
}

impl<S> ChatController<S> {
    pub fn new(source: S) -> Self {
        Self {
            source,
            state: ChatState::default(),
        }
    }

    pub fn with_state(source: S, state: ChatState) -> Self {
        Self { source, state }
    }

    pub fn state(&self) -> &ChatState {
        &self.state
    }

    pub fn source(&self) -> &S {
        &self.source
    }

    pub fn apply(&mut self, action: ChatAction) {
        self.state.apply(action);
    }
}

impl<S> ChatController<S>
where
    S: AiSdkEventSource,
{
    pub async fn send_message(
        &mut self,
        content: impl Into<String>,
    ) -> Result<AiSdkRun, AiSdkError> {
        let user_message = AiSdkMessage::new(AiSdkRole::User, content);
        self.state.apply(ChatAction::PushMessage(user_message));

        let request = AiSdkGenerateRequest {
            messages: self.state.messages.clone(),
            thread_id: self.state.thread_id.clone(),
            ..Default::default()
        };
        self.run(request).await
    }

    pub async fn run(&mut self, request: AiSdkGenerateRequest) -> Result<AiSdkRun, AiSdkError> {
        match self.source.stream(request).await {
            Ok(mut stream) => {
                let mut events = Vec::new();
                let mut accumulator = AssistantMessageAccumulator::default();

                while let Some(event) = stream.next().await {
                    let event = event?;
                    accumulator.apply(&event);
                    self.state.apply(ChatAction::ApplyEvent(event.clone()));
                    events.push(event);
                }

                let assistant_message = accumulator
                    .clone()
                    .into_message()
                    .ok_or_else(|| AiSdkError::Validation("assistant stream returned no final message".to_owned()))?;
                let run_id = accumulator
                    .run_id()
                    .map(str::to_owned)
                    .unwrap_or_default();

                Ok(AiSdkRun {
                    run_id,
                    assistant_message: assistant_message.clone(),
                    raw: mastra_client_sdks_ai_sdk::GenerateResponse {
                        text: assistant_message.content.clone(),
                        finish_reason: accumulator.finish_reason().cloned().unwrap_or_default(),
                        usage: accumulator.usage().cloned(),
                    },
                    events,
                })
            }
            Err(error) => {
                self.state.apply(ChatAction::Fail(error.to_string()));
                Err(error)
            }
        }
    }
}

impl From<AiSdkEvent> for ChatAction {
    fn from(value: AiSdkEvent) -> Self {
        Self::ApplyEvent(value)
    }
}
