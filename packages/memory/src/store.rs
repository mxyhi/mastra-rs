use async_trait::async_trait;
use thiserror::Error;
use uuid::Uuid;

use crate::model::{
    AppendMessageRequest, CloneThreadRequest, CreateThreadRequest, DeleteMessagesRequest,
    HistoryQuery, ListMessagesQuery, ListThreadsQuery, Message, MessagePage, Pagination, Thread,
    ThreadPage,
};

pub type MemoryStoreResult<T> = Result<T, MemoryStoreError>;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MemoryStoreError {
    #[error("thread `{0}` was not found")]
    ThreadNotFound(Uuid),
    #[error("pagination per_page must be greater than zero")]
    InvalidPagination,
    #[error("{0}")]
    Backend(String),
}

#[async_trait]
pub trait MemoryStore: Send + Sync {
    async fn create_thread(&self, input: CreateThreadRequest) -> MemoryStoreResult<Thread>;

    async fn get_thread(&self, thread_id: Uuid) -> MemoryStoreResult<Option<Thread>>;

    async fn list_threads(&self, query: ListThreadsQuery) -> MemoryStoreResult<ThreadPage>;

    async fn append_message(&self, input: AppendMessageRequest) -> MemoryStoreResult<Message>;

    async fn list_messages(&self, query: ListMessagesQuery) -> MemoryStoreResult<MessagePage>;

    async fn history(&self, query: HistoryQuery) -> MemoryStoreResult<Vec<Message>>;

    async fn clone_thread(&self, input: CloneThreadRequest) -> MemoryStoreResult<Thread>;

    async fn delete_messages(&self, input: DeleteMessagesRequest) -> MemoryStoreResult<usize>;

    async fn delete_thread(&self, thread_id: Uuid) -> MemoryStoreResult<()>;
}

pub fn ensure_valid_pagination(pagination: Pagination) -> MemoryStoreResult<()> {
    if pagination.per_page == 0 {
        return Err(MemoryStoreError::InvalidPagination);
    }

    Ok(())
}
