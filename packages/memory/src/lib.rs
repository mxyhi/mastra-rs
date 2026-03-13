mod in_memory;
mod model;
mod store;

use std::{collections::HashSet, sync::Arc};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use mastra_core::{
    CloneThreadRequest as CoreCloneThreadRequest, CreateThreadRequest as CoreCreateThreadRequest,
    MastraError, MemoryEngine, MemoryMessage, MemoryRecallRequest, MemoryRole,
    Thread as CoreThread,
};
use uuid::Uuid;

pub use in_memory::InMemoryMemoryStore;
pub use model::{
    AppendMessageRequest, CloneThreadRequest, CreateThreadRequest, DeleteMessagesRequest,
    HistoryQuery, ListMessagesQuery, ListThreadsQuery, Message, MessagePage, MessageRole,
    Pagination, Thread, ThreadPage, UpdateThreadRequest,
};
pub use store::{MemoryStore, MemoryStoreError, MemoryStoreResult, ensure_valid_pagination};

#[derive(Clone)]
pub struct Memory {
    store: Arc<dyn MemoryStore>,
}

impl Memory {
    pub fn new<S>(store: S) -> Self
    where
        S: MemoryStore + 'static,
    {
        Self {
            store: Arc::new(store),
        }
    }

    pub fn in_memory() -> Self {
        Self::new(InMemoryMemoryStore::default())
    }

    pub fn store(&self) -> Arc<dyn MemoryStore> {
        Arc::clone(&self.store)
    }

    pub async fn create_thread(&self, input: CreateThreadRequest) -> MemoryStoreResult<Thread> {
        self.store.create_thread(input).await
    }

    pub async fn get_thread(&self, thread_id: Uuid) -> MemoryStoreResult<Option<Thread>> {
        self.store.get_thread(thread_id).await
    }

    pub async fn update_thread(&self, input: UpdateThreadRequest) -> MemoryStoreResult<Thread> {
        self.store.update_thread(input).await
    }

    pub async fn list_threads(&self, query: ListThreadsQuery) -> MemoryStoreResult<ThreadPage> {
        self.store.list_threads(query).await
    }

    pub async fn append_message(&self, input: AppendMessageRequest) -> MemoryStoreResult<Message> {
        self.store.append_message(input).await
    }

    pub async fn list_messages_page(
        &self,
        query: ListMessagesQuery,
    ) -> MemoryStoreResult<MessagePage> {
        self.store.list_messages(query).await
    }

    pub async fn history(&self, query: HistoryQuery) -> MemoryStoreResult<Vec<Message>> {
        self.store.history(query).await
    }

    pub async fn clone_thread(&self, input: CloneThreadRequest) -> MemoryStoreResult<Thread> {
        self.store.clone_thread(input).await
    }

    pub async fn delete_messages(&self, input: DeleteMessagesRequest) -> MemoryStoreResult<usize> {
        self.store.delete_messages(input).await
    }

    pub async fn delete_thread(&self, thread_id: Uuid) -> MemoryStoreResult<()> {
        self.store.delete_thread(thread_id).await
    }
}

#[async_trait]
impl MemoryEngine for Memory {
    async fn create_thread(
        &self,
        request: CoreCreateThreadRequest,
    ) -> mastra_core::Result<CoreThread> {
        let thread = self
            .store
            .create_thread(CreateThreadRequest {
                thread_id: request.id.and_then(|id| Uuid::parse_str(&id).ok()),
                resource_id: request.resource_id.unwrap_or_else(|| "default".to_string()),
                title: request.title.unwrap_or_else(|| "New thread".to_string()),
                metadata: request.metadata,
            })
            .await
            .map_err(map_store_error)?;

        Ok(thread_to_core(thread))
    }

    async fn get_thread(&self, thread_id: &str) -> mastra_core::Result<Option<CoreThread>> {
        let thread_id = parse_uuid(thread_id, "thread id")?;
        self.store
            .get_thread(thread_id)
            .await
            .map(|thread| thread.map(thread_to_core))
            .map_err(map_store_error)
    }

    async fn update_thread(
        &self,
        thread_id: &str,
        request: mastra_core::UpdateThreadRequest,
    ) -> mastra_core::Result<CoreThread> {
        let thread_id = parse_uuid(thread_id, "thread id")?;
        let thread = self
            .store
            .update_thread(UpdateThreadRequest {
                thread_id,
                resource_id: request.resource_id,
                title: request.title,
                metadata: request.metadata,
            })
            .await
            .map_err(map_store_error)?;

        Ok(thread_to_core(thread))
    }

    async fn list_threads(
        &self,
        resource_id: Option<&str>,
    ) -> mastra_core::Result<Vec<CoreThread>> {
        self.store
            .list_threads(ListThreadsQuery {
                resource_id: resource_id.map(str::to_string),
                pagination: unbounded_pagination(),
            })
            .await
            .map(|page| page.items.into_iter().map(thread_to_core).collect())
            .map_err(map_store_error)
    }

    async fn append_messages(
        &self,
        thread_id: &str,
        messages: Vec<MemoryMessage>,
    ) -> mastra_core::Result<()> {
        let thread_id = parse_uuid(thread_id, "thread id")?;

        for message in messages {
            self.store
                .append_message(AppendMessageRequest {
                    message_id: Some(
                        Uuid::parse_str(&message.id).unwrap_or_else(|_| Uuid::new_v4()),
                    ),
                    thread_id,
                    role: role_from_core(message.role),
                    text: message.content,
                    metadata: message.metadata,
                    created_at: Some(message.created_at),
                })
                .await
                .map_err(map_store_error)?;
        }

        Ok(())
    }

    async fn list_messages(
        &self,
        request: MemoryRecallRequest,
    ) -> mastra_core::Result<Vec<MemoryMessage>> {
        let thread_id = parse_uuid(&request.thread_id, "thread id")?;
        if let Some(resource_id) = request.resource_id.as_deref() {
            let thread = self
                .store
                .get_thread(thread_id)
                .await
                .map_err(map_store_error)?
                .ok_or_else(|| {
                    MastraError::not_found(format!("thread '{thread_id}' was not found"))
                })?;
            if thread.resource_id != resource_id {
                return Ok(Vec::new());
            }
        }

        let mut messages = self
            .store
            .list_messages(ListMessagesQuery {
                thread_id,
                pagination: unbounded_pagination(),
            })
            .await
            .map_err(map_store_error)?
            .items;
        messages = filter_messages(
            messages,
            request.message_ids.as_ref(),
            request.start_date,
            request.end_date,
        );

        if let Some(limit) = request.limit {
            let start = messages.len().saturating_sub(limit);
            messages = messages.into_iter().skip(start).collect();
        }

        Ok(messages.into_iter().map(message_to_core).collect())
    }

    async fn clone_thread(
        &self,
        request: CoreCloneThreadRequest,
    ) -> mastra_core::Result<CoreThread> {
        let source_thread_id = parse_uuid(&request.source_thread_id, "source thread id")?;
        let new_thread_id = request
            .new_thread_id
            .as_deref()
            .map(|value| parse_uuid(value, "new thread id"))
            .transpose()?;

        let thread = self
            .store
            .clone_thread(CloneThreadRequest {
                source_thread_id,
                new_thread_id,
                resource_id: request.resource_id,
                title: request.title,
                metadata: request.metadata,
                message_limit: request.message_limit,
                message_ids: request
                    .message_ids
                    .as_ref()
                    .map(|ids| parse_uuid_list(ids, "message id"))
                    .transpose()?,
                start_date: request.start_date,
                end_date: request.end_date,
            })
            .await
            .map_err(map_store_error)?;

        Ok(thread_to_core(thread))
    }

    async fn delete_messages(&self, message_ids: Vec<String>) -> mastra_core::Result<usize> {
        if message_ids.is_empty() {
            return Ok(0);
        }

        let mut remaining = message_ids
            .into_iter()
            .map(|message_id| parse_uuid(&message_id, "message id"))
            .collect::<mastra_core::Result<HashSet<_>>>()?;
        let threads = self
            .store
            .list_threads(ListThreadsQuery {
                resource_id: None,
                pagination: unbounded_pagination(),
            })
            .await
            .map_err(map_store_error)?;
        let mut deleted = 0;

        // The core trait deletes by message id only, so we resolve message ownership here and
        // then delegate to the existing per-thread store deletion API.
        for thread in threads.items {
            if remaining.is_empty() {
                break;
            }

            let messages = self
                .store
                .list_messages(ListMessagesQuery {
                    thread_id: thread.id,
                    pagination: unbounded_pagination(),
                })
                .await
                .map_err(map_store_error)?;
            let matched = messages
                .items
                .into_iter()
                .filter_map(|message| remaining.remove(&message.id).then_some(message.id))
                .collect::<Vec<_>>();

            if matched.is_empty() {
                continue;
            }

            deleted += self
                .store
                .delete_messages(DeleteMessagesRequest::new(thread.id, matched))
                .await
                .map_err(map_store_error)?;
        }

        Ok(deleted)
    }

    async fn delete_thread(&self, thread_id: &str) -> mastra_core::Result<()> {
        let thread_id = parse_uuid(thread_id, "thread id")?;
        self.store
            .delete_thread(thread_id)
            .await
            .map_err(map_store_error)
    }
}

fn parse_uuid(value: &str, label: &str) -> mastra_core::Result<Uuid> {
    Uuid::parse_str(value)
        .map_err(|error| MastraError::validation(format!("invalid {label} '{value}': {error}")))
}

fn parse_uuid_list(values: &[String], label: &str) -> mastra_core::Result<Vec<Uuid>> {
    values
        .iter()
        .map(|value| parse_uuid(value, label))
        .collect::<mastra_core::Result<Vec<_>>>()
}

fn map_store_error(error: MemoryStoreError) -> MastraError {
    match error {
        MemoryStoreError::ThreadNotFound(thread_id) => {
            MastraError::not_found(format!("thread '{thread_id}' was not found"))
        }
        MemoryStoreError::InvalidPagination => MastraError::validation("invalid pagination"),
        MemoryStoreError::Backend(message) => MastraError::storage(message),
    }
}

fn thread_to_core(thread: Thread) -> CoreThread {
    CoreThread {
        id: thread.id.to_string(),
        resource_id: Some(thread.resource_id),
        title: Some(thread.title),
        created_at: thread.created_at,
        updated_at: thread.updated_at,
        metadata: thread.metadata,
    }
}

fn message_to_core(message: Message) -> MemoryMessage {
    MemoryMessage {
        id: message.id.to_string(),
        thread_id: message.thread_id.to_string(),
        role: role_to_core(message.role),
        content: message.text,
        created_at: message.created_at,
        metadata: message.metadata,
    }
}

fn role_to_core(role: MessageRole) -> MemoryRole {
    match role {
        MessageRole::System => MemoryRole::System,
        MessageRole::User => MemoryRole::User,
        MessageRole::Assistant => MemoryRole::Assistant,
        MessageRole::Tool => MemoryRole::Tool,
    }
}

fn role_from_core(role: MemoryRole) -> MessageRole {
    match role {
        MemoryRole::System => MessageRole::System,
        MemoryRole::User => MessageRole::User,
        MemoryRole::Assistant => MessageRole::Assistant,
        MemoryRole::Tool => MessageRole::Tool,
    }
}

fn filter_messages(
    messages: Vec<Message>,
    message_ids: Option<&Vec<String>>,
    start_date: Option<DateTime<Utc>>,
    end_date: Option<DateTime<Utc>>,
) -> Vec<Message> {
    let mut filtered = messages;

    if let Some(message_ids) = message_ids {
        let allowed = message_ids
            .iter()
            .filter_map(|id| Uuid::parse_str(id).ok())
            .collect::<HashSet<_>>();
        filtered.retain(|message| allowed.contains(&message.id));
    }

    if let Some(start_date) = start_date {
        filtered.retain(|message| message.created_at >= start_date);
    }

    if let Some(end_date) = end_date {
        filtered.retain(|message| message.created_at <= end_date);
    }

    filtered
}

fn unbounded_pagination() -> Pagination {
    Pagination::new(0, i32::MAX as usize)
}

#[cfg(test)]
mod tests;
