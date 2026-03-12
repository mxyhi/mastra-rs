use async_trait::async_trait;
use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::Result;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub enum MemoryRole {
    System,
    User,
    Assistant,
    Tool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct MemoryMessage {
    pub id: String,
    pub thread_id: String,
    pub role: MemoryRole,
    pub content: String,
    pub created_at: DateTime<Utc>,
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct Thread {
    pub id: String,
    pub resource_id: Option<String>,
    pub title: Option<String>,
    pub created_at: DateTime<Utc>,
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CreateThreadRequest {
    pub id: Option<String>,
    pub resource_id: Option<String>,
    pub title: Option<String>,
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MemoryRecallRequest {
    pub thread_id: String,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MemoryConfig {
    pub last_messages: Option<usize>,
    pub read_only: bool,
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            last_messages: Some(20),
            read_only: false,
        }
    }
}

#[async_trait]
pub trait MemoryEngine: Send + Sync {
    async fn create_thread(&self, request: CreateThreadRequest) -> Result<Thread>;

    async fn get_thread(&self, thread_id: &str) -> Result<Option<Thread>>;

    async fn list_threads(&self, resource_id: Option<&str>) -> Result<Vec<Thread>>;

    async fn append_messages(&self, thread_id: &str, messages: Vec<MemoryMessage>) -> Result<()>;

    async fn list_messages(&self, request: MemoryRecallRequest) -> Result<Vec<MemoryMessage>>;
}
