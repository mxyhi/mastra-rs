use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Thread {
    pub id: Uuid,
    pub resource_id: String,
    pub title: String,
    pub metadata: Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum MessageRole {
    System,
    User,
    Assistant,
    Tool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Message {
    pub id: Uuid,
    pub thread_id: Uuid,
    pub role: MessageRole,
    pub text: String,
    pub metadata: Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateThreadRequest {
    pub thread_id: Option<Uuid>,
    pub resource_id: String,
    pub title: String,
    pub metadata: Value,
}

impl CreateThreadRequest {
    pub fn new(resource_id: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            thread_id: None,
            resource_id: resource_id.into(),
            title: title.into(),
            metadata: Value::Object(Default::default()),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AppendMessageRequest {
    pub message_id: Option<Uuid>,
    pub thread_id: Uuid,
    pub role: MessageRole,
    pub text: String,
    pub metadata: Value,
    pub created_at: Option<DateTime<Utc>>,
}

impl AppendMessageRequest {
    pub fn new(thread_id: Uuid, role: MessageRole, text: impl Into<String>) -> Self {
        Self {
            message_id: None,
            thread_id,
            role,
            text: text.into(),
            metadata: Value::Object(Default::default()),
            created_at: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CloneThreadRequest {
    pub source_thread_id: Uuid,
    pub new_thread_id: Option<Uuid>,
    pub resource_id: Option<String>,
    pub title: Option<String>,
    pub metadata: Option<Value>,
    pub message_limit: Option<usize>,
    pub message_ids: Option<Vec<Uuid>>,
    pub start_date: Option<DateTime<Utc>>,
    pub end_date: Option<DateTime<Utc>>,
}

impl CloneThreadRequest {
    pub fn new(source_thread_id: Uuid) -> Self {
        Self {
            source_thread_id,
            new_thread_id: None,
            resource_id: None,
            title: None,
            metadata: None,
            message_limit: None,
            message_ids: None,
            start_date: None,
            end_date: None,
        }
    }

    pub fn with_thread_id(mut self, thread_id: Uuid) -> Self {
        self.new_thread_id = Some(thread_id);
        self
    }

    pub fn with_resource_id(mut self, resource_id: impl Into<String>) -> Self {
        self.resource_id = Some(resource_id.into());
        self
    }

    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn with_metadata(mut self, metadata: Value) -> Self {
        self.metadata = Some(metadata);
        self
    }

    pub fn with_message_limit(mut self, message_limit: usize) -> Self {
        self.message_limit = Some(message_limit);
        self
    }

    pub fn with_message_ids(mut self, message_ids: Vec<Uuid>) -> Self {
        self.message_ids = Some(message_ids);
        self
    }

    pub fn starting_from(mut self, start_date: DateTime<Utc>) -> Self {
        self.start_date = Some(start_date);
        self
    }

    pub fn ending_at(mut self, end_date: DateTime<Utc>) -> Self {
        self.end_date = Some(end_date);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeleteMessagesRequest {
    pub thread_id: Uuid,
    pub message_ids: Vec<Uuid>,
}

impl DeleteMessagesRequest {
    pub fn new(thread_id: Uuid, message_ids: Vec<Uuid>) -> Self {
        Self {
            thread_id,
            message_ids,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListThreadsQuery {
    pub resource_id: Option<String>,
    pub pagination: Pagination,
}

impl Default for ListThreadsQuery {
    fn default() -> Self {
        Self {
            resource_id: None,
            pagination: Pagination::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListMessagesQuery {
    pub thread_id: Uuid,
    pub pagination: Pagination,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryQuery {
    pub thread_id: Uuid,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ThreadPage {
    pub items: Vec<Thread>,
    pub total: usize,
    pub page: usize,
    pub per_page: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MessagePage {
    pub items: Vec<Message>,
    pub total: usize,
    pub page: usize,
    pub per_page: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pagination {
    pub page: usize,
    pub per_page: usize,
}

impl Pagination {
    pub const fn new(page: usize, per_page: usize) -> Self {
        Self { page, per_page }
    }

    pub const fn offset(self) -> usize {
        self.page.saturating_mul(self.per_page)
    }
}

impl Default for Pagination {
    fn default() -> Self {
        Self {
            page: 0,
            per_page: 50,
        }
    }
}
