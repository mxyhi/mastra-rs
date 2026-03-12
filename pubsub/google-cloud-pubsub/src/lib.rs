use std::sync::Arc;

use chrono::{DateTime, Utc};
use indexmap::{IndexMap, IndexSet};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub type Result<T> = std::result::Result<T, PubSubError>;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PubSubError {
    #[error("topic '{0}' was not found")]
    TopicNotFound(String),
    #[error("subscription '{0}' was not found")]
    SubscriptionNotFound(String),
    #[error("topic '{0}' already exists")]
    TopicAlreadyExists(String),
    #[error("subscription '{0}' already exists")]
    SubscriptionAlreadyExists(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GoogleCloudPubSubConfig {
    pub project_id: String,
    pub emulator_host: Option<String>,
}

impl GoogleCloudPubSubConfig {
    pub fn new(project_id: impl Into<String>) -> Self {
        Self {
            project_id: project_id.into(),
            emulator_host: None,
        }
    }

    pub fn with_emulator_host(mut self, emulator_host: impl Into<String>) -> Self {
        self.emulator_host = Some(emulator_host.into());
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PubSubMessage {
    pub id: String,
    pub data: Vec<u8>,
    pub attributes: IndexMap<String, String>,
    pub published_at: DateTime<Utc>,
}

impl PubSubMessage {
    pub fn new(data: Vec<u8>) -> Self {
        Self {
            id: Uuid::now_v7().to_string(),
            data,
            attributes: IndexMap::new(),
            published_at: Utc::now(),
        }
    }

    pub fn text(text: impl Into<String>) -> Self {
        Self::new(text.into().into_bytes())
    }

    pub fn with_attribute(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.attributes.insert(key.into(), value.into());
        self
    }

    pub fn as_text(&self) -> Option<&str> {
        std::str::from_utf8(&self.data).ok()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PulledMessage {
    pub ack_id: String,
    pub message: PubSubMessage,
}

#[derive(Debug, Clone)]
struct TopicState {
    messages: Vec<PubSubMessage>,
}

#[derive(Debug, Clone)]
struct SubscriptionState {
    topic: String,
    delivered_message_ids: IndexSet<String>,
    acked_message_ids: IndexSet<String>,
}

#[derive(Debug, Default)]
struct PubSubState {
    topics: IndexMap<String, TopicState>,
    subscriptions: IndexMap<String, SubscriptionState>,
}

#[derive(Clone)]
pub struct GoogleCloudPubSub {
    config: GoogleCloudPubSubConfig,
    state: Arc<RwLock<PubSubState>>,
}

impl GoogleCloudPubSub {
    pub fn new(config: GoogleCloudPubSubConfig) -> Self {
        Self {
            config,
            state: Arc::new(RwLock::new(PubSubState::default())),
        }
    }

    pub fn config(&self) -> &GoogleCloudPubSubConfig {
        &self.config
    }

    pub fn create_topic(&self, topic: impl Into<String>) -> Result<()> {
        let topic = topic.into();
        let mut state = self.state.write();
        if state.topics.contains_key(&topic) {
            return Err(PubSubError::TopicAlreadyExists(topic));
        }
        state.topics.insert(topic, TopicState { messages: Vec::new() });
        Ok(())
    }

    pub fn create_subscription(
        &self,
        subscription: impl Into<String>,
        topic: impl Into<String>,
    ) -> Result<()> {
        let subscription = subscription.into();
        let topic = topic.into();
        let mut state = self.state.write();
        if !state.topics.contains_key(&topic) {
            return Err(PubSubError::TopicNotFound(topic));
        }
        if state.subscriptions.contains_key(&subscription) {
            return Err(PubSubError::SubscriptionAlreadyExists(subscription));
        }
        state.subscriptions.insert(
            subscription,
            SubscriptionState {
                topic,
                delivered_message_ids: IndexSet::new(),
                acked_message_ids: IndexSet::new(),
            },
        );
        Ok(())
    }

    pub fn publish(&self, topic: &str, message: PubSubMessage) -> Result<String> {
        let mut state = self.state.write();
        let topic_state = state
            .topics
            .get_mut(topic)
            .ok_or_else(|| PubSubError::TopicNotFound(topic.to_owned()))?;
        let message_id = message.id.clone();
        topic_state.messages.push(message);
        Ok(message_id)
    }

    pub fn pull(&self, subscription: &str, max_messages: usize) -> Result<Vec<PulledMessage>> {
        let mut state = self.state.write();
        let topic_name = state
            .subscriptions
            .get(subscription)
            .ok_or_else(|| PubSubError::SubscriptionNotFound(subscription.to_owned()))?
            .topic
            .clone();
        let topic_messages = state
            .topics
            .get(&topic_name)
            .ok_or_else(|| PubSubError::TopicNotFound(topic_name.clone()))?
            .messages
            .clone();
        let subscription_state = state
            .subscriptions
            .get_mut(subscription)
            .ok_or_else(|| PubSubError::SubscriptionNotFound(subscription.to_owned()))?;

        let mut batch = Vec::new();
        for message in topic_messages {
            if subscription_state.acked_message_ids.contains(&message.id) {
                continue;
            }
            if subscription_state.delivered_message_ids.contains(&message.id) {
                continue;
            }
            subscription_state
                .delivered_message_ids
                .insert(message.id.clone());
            batch.push(PulledMessage {
                ack_id: message.id.clone(),
                message,
            });
            if batch.len() == max_messages {
                break;
            }
        }
        Ok(batch)
    }

    pub fn ack<I>(&self, subscription: &str, ack_ids: I) -> Result<()>
    where
        I: IntoIterator<Item = String>,
    {
        let mut state = self.state.write();
        let subscription_state = state
            .subscriptions
            .get_mut(subscription)
            .ok_or_else(|| PubSubError::SubscriptionNotFound(subscription.to_owned()))?;
        for ack_id in ack_ids {
            subscription_state.acked_message_ids.insert(ack_id);
        }
        Ok(())
    }
}
