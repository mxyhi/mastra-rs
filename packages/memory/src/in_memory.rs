use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use parking_lot::RwLock;
use uuid::Uuid;

use crate::model::{
    AppendMessageRequest, CreateThreadRequest, HistoryQuery, ListMessagesQuery, ListThreadsQuery,
    Message, MessagePage, Thread, ThreadPage,
};
use crate::store::{MemoryStore, MemoryStoreError, MemoryStoreResult, ensure_valid_pagination};

#[derive(Debug, Clone, Default)]
pub struct InMemoryMemoryStore {
    state: Arc<RwLock<MemoryState>>,
}

#[derive(Debug, Default)]
struct MemoryState {
    threads: HashMap<Uuid, Thread>,
    messages: HashMap<Uuid, Vec<Message>>,
}

#[async_trait]
impl MemoryStore for InMemoryMemoryStore {
    async fn create_thread(&self, input: CreateThreadRequest) -> MemoryStoreResult<Thread> {
        let mut state = self.state.write();
        let now = Utc::now();
        let thread = Thread {
            id: input.thread_id.unwrap_or_else(Uuid::new_v4),
            resource_id: input.resource_id,
            title: input.title,
            metadata: input.metadata,
            created_at: now,
            updated_at: now,
        };

        state.messages.entry(thread.id).or_default();
        state.threads.insert(thread.id, thread.clone());

        Ok(thread)
    }

    async fn get_thread(&self, thread_id: Uuid) -> MemoryStoreResult<Option<Thread>> {
        let state = self.state.read();
        Ok(state.threads.get(&thread_id).cloned())
    }

    async fn list_threads(&self, query: ListThreadsQuery) -> MemoryStoreResult<ThreadPage> {
        ensure_valid_pagination(query.pagination)?;

        let state = self.state.read();
        let mut threads = state
            .threads
            .values()
            .filter(|thread| {
                query
                    .resource_id
                    .as_ref()
                    .map(|resource_id| thread.resource_id == *resource_id)
                    .unwrap_or(true)
            })
            .cloned()
            .collect::<Vec<_>>();

        threads.sort_by(|left, right| {
            right
                .updated_at
                .cmp(&left.updated_at)
                .then_with(|| left.id.cmp(&right.id))
        });

        let total = threads.len();
        let start = query.pagination.offset();
        let items = threads
            .into_iter()
            .skip(start)
            .take(query.pagination.per_page)
            .collect();

        Ok(ThreadPage {
            items,
            total,
            page: query.pagination.page,
            per_page: query.pagination.per_page,
        })
    }

    async fn append_message(&self, input: AppendMessageRequest) -> MemoryStoreResult<Message> {
        let mut state = self.state.write();
        let thread = state
            .threads
            .get_mut(&input.thread_id)
            .ok_or(MemoryStoreError::ThreadNotFound(input.thread_id))?;

        let created_at = input.created_at.unwrap_or_else(Utc::now);
        let message = Message {
            id: input.message_id.unwrap_or_else(Uuid::new_v4),
            thread_id: input.thread_id,
            role: input.role,
            text: input.text,
            metadata: input.metadata,
            created_at,
        };

        thread.updated_at = message.created_at;
        state
            .messages
            .entry(input.thread_id)
            .or_default()
            .push(message.clone());

        Ok(message)
    }

    async fn list_messages(&self, query: ListMessagesQuery) -> MemoryStoreResult<MessagePage> {
        ensure_valid_pagination(query.pagination)?;

        let state = self.state.read();
        let messages = state
            .messages
            .get(&query.thread_id)
            .ok_or(MemoryStoreError::ThreadNotFound(query.thread_id))?;

        let total = messages.len();
        let items = messages
            .iter()
            .skip(query.pagination.offset())
            .take(query.pagination.per_page)
            .cloned()
            .collect();

        Ok(MessagePage {
            items,
            total,
            page: query.pagination.page,
            per_page: query.pagination.per_page,
        })
    }

    async fn history(&self, query: HistoryQuery) -> MemoryStoreResult<Vec<Message>> {
        let state = self.state.read();
        let messages = state
            .messages
            .get(&query.thread_id)
            .ok_or(MemoryStoreError::ThreadNotFound(query.thread_id))?;

        let limit = query.limit.unwrap_or(messages.len());
        let start = messages.len().saturating_sub(limit);

        Ok(messages[start..].to_vec())
    }
}
