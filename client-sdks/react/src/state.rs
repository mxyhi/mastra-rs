use mastra_client_sdks_ai_sdk::{AiSdkEvent, AiSdkMessage, AiSdkRole};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum ChatStatus {
    #[default]
    Idle,
    Running,
    Complete,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChatState {
    pub status: ChatStatus,
    pub messages: Vec<AiSdkMessage>,
    pub pending_assistant: Option<AiSdkMessage>,
    pub run_id: Option<String>,
    pub thread_id: Option<String>,
    pub last_error: Option<String>,
}

impl Default for ChatState {
    fn default() -> Self {
        Self {
            status: ChatStatus::Idle,
            messages: Vec::new(),
            pending_assistant: None,
            run_id: None,
            thread_id: None,
            last_error: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ChatAction {
    PushMessage(AiSdkMessage),
    ApplyEvent(AiSdkEvent),
    Fail(String),
    Reset,
}

impl ChatState {
    pub fn apply(&mut self, action: ChatAction) {
        match action {
            ChatAction::PushMessage(message) => {
                self.last_error = None;
                self.status = ChatStatus::Idle;
                self.messages.push(message);
            }
            ChatAction::ApplyEvent(event) => self.apply_event(event),
            ChatAction::Fail(error) => {
                self.status = ChatStatus::Failed;
                self.pending_assistant = None;
                self.last_error = Some(error);
            }
            ChatAction::Reset => *self = Self::default(),
        }
    }

    fn apply_event(&mut self, event: AiSdkEvent) {
        match event {
            AiSdkEvent::Start(start) => {
                self.status = ChatStatus::Running;
                self.run_id = Some(start.run_id);
                self.thread_id = start.thread_id;
                self.last_error = None;
                self.pending_assistant = Some(AiSdkMessage::with_id(
                    start.message_id,
                    AiSdkRole::Assistant,
                    String::new(),
                ));
            }
            AiSdkEvent::TextDelta(delta) => {
                self.status = ChatStatus::Running;
                let pending = self.pending_assistant.get_or_insert_with(|| {
                    AiSdkMessage::with_id(
                        delta.message_id.clone(),
                        AiSdkRole::Assistant,
                        String::new(),
                    )
                });
                if pending.id != delta.message_id {
                    *pending = AiSdkMessage::with_id(
                        delta.message_id,
                        AiSdkRole::Assistant,
                        String::new(),
                    );
                }
                pending.content.push_str(&delta.delta);
            }
            AiSdkEvent::Finish(finish) => {
                self.status = ChatStatus::Complete;
                self.run_id = Some(finish.run_id);
                self.pending_assistant = None;
                self.messages.push(finish.message);
            }
        }
    }
}
