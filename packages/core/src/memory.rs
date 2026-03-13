use async_trait::async_trait;
use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::error::{MastraError, Result};

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
    pub resource_id: Option<String>,
    pub page: Option<usize>,
    pub per_page: Option<usize>,
    pub message_ids: Option<Vec<String>>,
    pub start_date: Option<DateTime<Utc>>,
    pub end_date: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
pub struct MemoryThreadQuery {
    pub resource_id: Option<String>,
    pub metadata: Option<Value>,
    pub page: Option<usize>,
    pub per_page: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct MemoryThreadPage {
    pub threads: Vec<Thread>,
    pub total: usize,
    pub page: usize,
    pub per_page: usize,
    pub has_more: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct MemoryMessagePage {
    pub messages: Vec<MemoryMessage>,
    pub total: usize,
    pub page: usize,
    pub per_page: usize,
    pub has_more: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CloneThreadRequest {
    pub source_thread_id: String,
    pub new_thread_id: Option<String>,
    pub resource_id: Option<String>,
    pub title: Option<String>,
    pub metadata: Option<Value>,
    pub message_limit: Option<usize>,
    pub message_ids: Option<Vec<String>>,
    pub start_date: Option<DateTime<Utc>>,
    pub end_date: Option<DateTime<Utc>>,
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

    async fn list_threads_page(&self, request: MemoryThreadQuery) -> Result<MemoryThreadPage> {
        let mut threads = self.list_threads(request.resource_id.as_deref()).await?;
        if let Some(metadata) = request.metadata.as_ref() {
            threads.retain(|thread| metadata_matches(metadata, &thread.metadata));
        }

        paginate_threads(threads, request.page, request.per_page)
    }

    async fn append_messages(&self, thread_id: &str, messages: Vec<MemoryMessage>) -> Result<()>;

    async fn list_messages(&self, request: MemoryRecallRequest) -> Result<Vec<MemoryMessage>>;

    async fn list_messages_page(&self, request: MemoryRecallRequest) -> Result<MemoryMessagePage> {
        if request.page.is_none() && request.per_page.is_none() {
            let messages = self.list_messages(request).await?;
            let per_page = messages.len().max(1);
            return Ok(MemoryMessagePage {
                total: messages.len(),
                page: 0,
                per_page,
                has_more: false,
                messages,
            });
        }

        let mut unbounded_request = request.clone();
        unbounded_request.page = None;
        unbounded_request.per_page = None;
        unbounded_request.limit = None;

        let messages = self.list_messages(unbounded_request).await?;
        paginate_messages(messages, request.page, request.per_page)
    }

    async fn clone_thread(&self, _request: CloneThreadRequest) -> Result<Thread> {
        Err(MastraError::storage(
            "memory engine does not support thread cloning",
        ))
    }

    async fn delete_messages(&self, _message_ids: Vec<String>) -> Result<usize> {
        Err(MastraError::storage(
            "memory engine does not support deleting messages",
        ))
    }

    async fn delete_thread(&self, _thread_id: &str) -> Result<()> {
        Err(MastraError::storage(
            "memory engine does not support thread deletion",
        ))
    }
}

fn paginate_threads(
    threads: Vec<Thread>,
    page: Option<usize>,
    per_page: Option<usize>,
) -> Result<MemoryThreadPage> {
    let page = page.unwrap_or(0);
    let per_page = per_page.unwrap_or_else(|| threads.len().max(1));
    if per_page == 0 {
        return Err(MastraError::validation(
            "per_page must be greater than zero",
        ));
    }

    let total = threads.len();
    let start = page.saturating_mul(per_page);
    let page_threads = threads
        .into_iter()
        .skip(start)
        .take(per_page)
        .collect::<Vec<_>>();

    Ok(MemoryThreadPage {
        has_more: start.saturating_add(page_threads.len()) < total,
        threads: page_threads,
        total,
        page,
        per_page,
    })
}

fn paginate_messages(
    messages: Vec<MemoryMessage>,
    page: Option<usize>,
    per_page: Option<usize>,
) -> Result<MemoryMessagePage> {
    let page = page.unwrap_or(0);
    let per_page = per_page.unwrap_or_else(|| messages.len().max(1));
    if per_page == 0 {
        return Err(MastraError::validation(
            "per_page must be greater than zero",
        ));
    }

    let total = messages.len();
    let start = page.saturating_mul(per_page);
    let page_messages = messages
        .into_iter()
        .skip(start)
        .take(per_page)
        .collect::<Vec<_>>();

    Ok(MemoryMessagePage {
        has_more: start.saturating_add(page_messages.len()) < total,
        messages: page_messages,
        total,
        page,
        per_page,
    })
}

fn metadata_matches(filter: &Value, candidate: &Value) -> bool {
    match (filter, candidate) {
        (Value::Object(filter_map), Value::Object(candidate_map)) => {
            object_contains(candidate_map, filter_map)
        }
        _ => filter == candidate,
    }
}

fn object_contains(candidate: &Map<String, Value>, filter: &Map<String, Value>) -> bool {
    filter.iter().all(|(key, expected)| {
        let Some(actual) = candidate.get(key) else {
            return false;
        };

        match (expected, actual) {
            (Value::Object(expected_object), Value::Object(actual_object)) => {
                object_contains(actual_object, expected_object)
            }
            _ => actual == expected,
        }
    })
}
