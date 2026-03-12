use futures::stream::BoxStream;
use indexmap::IndexMap;
use mastra_client_sdks_client_js::{FinishReason, GenerateResponse, UsageStats};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AiSdkRole {
    System,
    User,
    Assistant,
    Tool,
}

impl AiSdkRole {
    pub fn as_api_role(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::User => "user",
            Self::Assistant => "assistant",
            Self::Tool => "tool",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AiSdkMessage {
    pub id: String,
    pub role: AiSdkRole,
    pub content: String,
}

impl AiSdkMessage {
    pub fn new(role: AiSdkRole, content: impl Into<String>) -> Self {
        Self {
            id: Uuid::now_v7().to_string(),
            role,
            content: content.into(),
        }
    }

    pub fn with_id(id: impl Into<String>, role: AiSdkRole, content: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            role,
            content: content.into(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct AiSdkGenerateRequest {
    #[serde(default)]
    pub messages: Vec<AiSdkMessage>,
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
pub struct AiSdkStartEvent {
    pub run_id: String,
    pub message_id: String,
    pub thread_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AiSdkTextDeltaEvent {
    pub run_id: String,
    pub message_id: String,
    pub delta: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AiSdkFinishEvent {
    pub run_id: String,
    pub message: AiSdkMessage,
    pub finish_reason: FinishReason,
    pub usage: Option<UsageStats>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AiSdkEvent {
    Start(AiSdkStartEvent),
    TextDelta(AiSdkTextDeltaEvent),
    Finish(AiSdkFinishEvent),
}

pub type AiSdkEventStream = BoxStream<'static, Result<AiSdkEvent, AiSdkError>>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AiSdkRun {
    pub run_id: String,
    pub assistant_message: AiSdkMessage,
    pub events: Vec<AiSdkEvent>,
    pub raw: GenerateResponse,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct AssistantMessageAccumulator {
    current_message: Option<AiSdkMessage>,
    run_id: Option<String>,
    finish_reason: Option<FinishReason>,
    usage: Option<UsageStats>,
}

impl AssistantMessageAccumulator {
    pub fn apply(&mut self, event: &AiSdkEvent) {
        match event {
            AiSdkEvent::Start(start) => {
                self.run_id = Some(start.run_id.clone());
                self.current_message = Some(AiSdkMessage::with_id(
                    start.message_id.clone(),
                    AiSdkRole::Assistant,
                    String::new(),
                ));
            }
            AiSdkEvent::TextDelta(delta) => {
                self.run_id = Some(delta.run_id.clone());
                let message = self.current_message.get_or_insert_with(|| {
                    AiSdkMessage::with_id(
                        delta.message_id.clone(),
                        AiSdkRole::Assistant,
                        String::new(),
                    )
                });
                if message.id != delta.message_id {
                    *message = AiSdkMessage::with_id(
                        delta.message_id.clone(),
                        AiSdkRole::Assistant,
                        String::new(),
                    );
                }
                message.content.push_str(&delta.delta);
            }
            AiSdkEvent::Finish(finish) => {
                self.run_id = Some(finish.run_id.clone());
                self.finish_reason = Some(finish.finish_reason.clone());
                self.usage = finish.usage.clone();
                self.current_message = Some(finish.message.clone());
            }
        }
    }

    pub fn current_message(&self) -> Option<&AiSdkMessage> {
        self.current_message.as_ref()
    }

    pub fn into_message(self) -> Option<AiSdkMessage> {
        self.current_message
    }

    pub fn usage(&self) -> Option<&UsageStats> {
        self.usage.as_ref()
    }

    pub fn finish_reason(&self) -> Option<&FinishReason> {
        self.finish_reason.as_ref()
    }

    pub fn run_id(&self) -> Option<&str> {
        self.run_id.as_deref()
    }
}

#[derive(Debug, Error)]
pub enum AiSdkError {
    #[error(transparent)]
    Client(#[from] mastra_client_sdks_client_js::MastraClientError),
    #[error("{0}")]
    Validation(String),
}
