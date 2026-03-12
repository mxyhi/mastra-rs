use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use parking_lot::RwLock;
use uuid::Uuid;

use crate::model::{
    AppendMessageRequest, CloneThreadRequest, CreateThreadRequest, DeleteMessagesRequest,
    HistoryQuery, ListMessagesQuery, ListThreadsQuery, Message, MessagePage, Thread, ThreadPage,
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

    async fn clone_thread(&self, input: CloneThreadRequest) -> MemoryStoreResult<Thread> {
        let mut state = self.state.write();
        let source_thread = state
            .threads
            .get(&input.source_thread_id)
            .cloned()
            .ok_or(MemoryStoreError::ThreadNotFound(input.source_thread_id))?;
        let source_messages = state
            .messages
            .get(&input.source_thread_id)
            .cloned()
            .unwrap_or_default();
        let now = Utc::now();
        let cloned_thread = Thread {
            id: input.new_thread_id.unwrap_or_else(Uuid::new_v4),
            resource_id: input.resource_id.unwrap_or(source_thread.resource_id),
            title: input
                .title
                .unwrap_or_else(|| format!("{} (copy)", source_thread.title)),
            metadata: input.metadata.unwrap_or(source_thread.metadata),
            created_at: now,
            updated_at: now,
        };

        let cloned_messages = source_messages
            .into_iter()
            .map(|message| Message {
                id: Uuid::new_v4(),
                thread_id: cloned_thread.id,
                role: message.role,
                text: message.text,
                metadata: message.metadata,
                created_at: message.created_at,
            })
            .collect::<Vec<_>>();
        let updated_at = cloned_messages
            .last()
            .map(|message| message.created_at)
            .unwrap_or(cloned_thread.created_at);
        let cloned_thread = Thread {
            updated_at,
            ..cloned_thread
        };

        state
            .threads
            .insert(cloned_thread.id, cloned_thread.clone());
        state.messages.insert(cloned_thread.id, cloned_messages);

        Ok(cloned_thread)
    }

    async fn delete_messages(&self, input: DeleteMessagesRequest) -> MemoryStoreResult<usize> {
        let mut state = self.state.write();
        let created_at = state
            .threads
            .get(&input.thread_id)
            .map(|thread| thread.created_at)
            .ok_or(MemoryStoreError::ThreadNotFound(input.thread_id))?;
        let messages = state
            .messages
            .get_mut(&input.thread_id)
            .ok_or(MemoryStoreError::ThreadNotFound(input.thread_id))?;
        let delete_ids = input.message_ids.into_iter().collect::<HashSet<_>>();
        let original_len = messages.len();

        messages.retain(|message| !delete_ids.contains(&message.id));
        let deleted = original_len.saturating_sub(messages.len());
        if deleted > 0 {
            let updated_at = messages
                .last()
                .map(|message| message.created_at)
                .unwrap_or(created_at);
            if let Some(thread) = state.threads.get_mut(&input.thread_id) {
                thread.updated_at = updated_at;
            }
        }

        Ok(deleted)
    }

    async fn delete_thread(&self, thread_id: Uuid) -> MemoryStoreResult<()> {
        let mut state = self.state.write();
        if state.threads.remove(&thread_id).is_none() {
            return Err(MemoryStoreError::ThreadNotFound(thread_id));
        }
        state.messages.remove(&thread_id);
        Ok(())
    }
}
