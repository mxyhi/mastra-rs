use async_trait::async_trait;
use mastra_memory::{
    AppendMessageRequest, CreateThreadRequest, HistoryQuery, InMemoryMemoryStore, ListMessagesQuery, ListThreadsQuery,
    MemoryStore, MemoryStoreResult, Message, MessagePage, Thread, ThreadPage,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LibSqlStoreConfig {
    pub url: String,
}

impl Default for LibSqlStoreConfig {
    fn default() -> Self {
        Self {
            url: "file::memory:".to_string(),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct LibSqlStore {
    config: LibSqlStoreConfig,
    inner: InMemoryMemoryStore,
}

impl LibSqlStore {
    pub fn new(config: LibSqlStoreConfig) -> Self {
        Self {
            config,
            inner: InMemoryMemoryStore::default(),
        }
    }

    pub fn config(&self) -> &LibSqlStoreConfig {
        &self.config
    }

    pub fn inner(&self) -> &InMemoryMemoryStore {
        &self.inner
    }
}

#[async_trait]
impl MemoryStore for LibSqlStore {
    async fn create_thread(&self, input: CreateThreadRequest) -> MemoryStoreResult<Thread> {
        self.inner.create_thread(input).await
    }

    async fn get_thread(&self, thread_id: Uuid) -> MemoryStoreResult<Option<Thread>> {
        self.inner.get_thread(thread_id).await
    }

    async fn list_threads(&self, query: ListThreadsQuery) -> MemoryStoreResult<ThreadPage> {
        self.inner.list_threads(query).await
    }

    async fn append_message(&self, input: AppendMessageRequest) -> MemoryStoreResult<Message> {
        self.inner.append_message(input).await
    }

    async fn list_messages(&self, query: ListMessagesQuery) -> MemoryStoreResult<MessagePage> {
        self.inner.list_messages(query).await
    }

    async fn history(&self, query: HistoryQuery) -> MemoryStoreResult<Vec<Message>> {
        self.inner.history(query).await
    }
}

#[cfg(test)]
mod tests {
    use mastra_memory::{MessageRole, Pagination};

    use super::{CreateThreadRequest, LibSqlStore, LibSqlStoreConfig, MemoryStore};

    #[tokio::test]
    async fn libsql_store_uses_in_memory_backend_for_now() {
        let store = LibSqlStore::new(LibSqlStoreConfig {
            url: "file:test.db".into(),
        });
        let thread = store
            .create_thread(CreateThreadRequest::new("resource-1", "chat"))
            .await
            .expect("thread should be created");

        store
            .append_message(mastra_memory::AppendMessageRequest::new(
                thread.id,
                MessageRole::User,
                "hello libsql",
            ))
            .await
            .expect("message should be written");

        let messages = store
            .list_messages(mastra_memory::ListMessagesQuery {
                thread_id: thread.id,
                pagination: Pagination::new(0, 10),
            })
            .await
            .expect("messages should be listed");

        assert_eq!(store.config().url, "file:test.db");
        assert_eq!(messages.total, 1);
        assert_eq!(messages.items[0].text, "hello libsql");
    }
}
