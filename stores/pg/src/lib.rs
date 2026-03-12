#[path = "../../_test-utils/src/provider_support.rs"]
mod provider_support;

use async_trait::async_trait;
use mastra_memory::{
    AppendMessageRequest, CloneThreadRequest, CreateThreadRequest, DeleteMessagesRequest,
    HistoryQuery, InMemoryMemoryStore, ListMessagesQuery, ListThreadsQuery, MemoryStore,
    MemoryStoreResult, Message, MessagePage, Thread, ThreadPage,
};
use provider_support::ensure_not_blank;
pub use provider_support::{
    ProviderBinding, ProviderBridge, ProviderCapability, ProviderConfigError, ProviderDescriptor,
    ProviderKind,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

const PG_CAPABILITIES: &[ProviderCapability] = &[ProviderCapability::MemoryStore];
const PG_DESCRIPTOR: ProviderDescriptor =
    ProviderDescriptor::new("pg", ProviderKind::Storage, PG_CAPABILITIES);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PgStoreConfig {
    pub connection_string: String,
    pub schema: Option<String>,
}

impl Default for PgStoreConfig {
    fn default() -> Self {
        Self {
            connection_string: "postgres://localhost/mastra".to_string(),
            schema: None,
        }
    }
}

impl PgStoreConfig {
    pub fn validate(&self) -> Result<(), ProviderConfigError> {
        ensure_not_blank(&self.connection_string, "connection_string")?;
        if let Some(schema) = &self.schema {
            ensure_not_blank(schema, "schema")?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PgProvider {
    config: PgStoreConfig,
}

impl PgProvider {
    pub fn new(config: PgStoreConfig) -> Result<Self, ProviderConfigError> {
        config.validate()?;
        Ok(Self { config })
    }

    pub fn config(&self) -> &PgStoreConfig {
        &self.config
    }

    pub fn descriptor(&self) -> ProviderDescriptor {
        PG_DESCRIPTOR
    }

    pub fn storage_bridge(&self) -> ProviderBridge {
        let mut bridge = ProviderBridge::new(
            PG_DESCRIPTOR,
            self.config
                .schema
                .clone()
                .unwrap_or_else(|| "public".to_string()),
        )
        .with_binding(ProviderBinding::plain(
            "connection_string",
            self.config.connection_string.clone(),
        ));

        if let Some(schema) = &self.config.schema {
            bridge = bridge.with_binding(ProviderBinding::plain("schema", schema.clone()));
        }

        bridge
    }

    pub fn build_store(&self) -> PgStore {
        PgStore::new(self.config.clone())
    }
}

#[derive(Debug, Clone, Default)]
pub struct PgStore {
    config: PgStoreConfig,
    inner: InMemoryMemoryStore,
}

impl PgStore {
    pub fn new(config: PgStoreConfig) -> Self {
        Self {
            config,
            inner: InMemoryMemoryStore::default(),
        }
    }

    pub fn config(&self) -> &PgStoreConfig {
        &self.config
    }

    pub fn inner(&self) -> &InMemoryMemoryStore {
        &self.inner
    }
}

#[async_trait]
impl MemoryStore for PgStore {
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

    async fn clone_thread(&self, input: CloneThreadRequest) -> MemoryStoreResult<Thread> {
        self.inner.clone_thread(input).await
    }

    async fn delete_messages(&self, input: DeleteMessagesRequest) -> MemoryStoreResult<usize> {
        self.inner.delete_messages(input).await
    }

    async fn delete_thread(&self, thread_id: Uuid) -> MemoryStoreResult<()> {
        self.inner.delete_thread(thread_id).await
    }
}

#[cfg(test)]
mod tests {
    use mastra_memory::{CloneThreadRequest, HistoryQuery, MessageRole};

    use super::{
        CreateThreadRequest, MemoryStore, PgProvider, PgStore, PgStoreConfig, ProviderCapability,
        ProviderConfigError, ProviderKind,
    };

    #[test]
    fn pg_provider_exposes_storage_bridge() {
        let provider = PgProvider::new(PgStoreConfig {
            connection_string: "postgres://localhost/test".into(),
            schema: Some("mastra".into()),
        })
        .expect("pg config should be valid");

        let descriptor = provider.descriptor();
        let bridge = provider.storage_bridge();
        let store = provider.build_store();

        assert_eq!(descriptor.id, "pg");
        assert_eq!(descriptor.kind, ProviderKind::Storage);
        assert!(bridge.supports(ProviderCapability::MemoryStore));
        assert_eq!(bridge.target, "mastra");
        assert_eq!(store.config().schema.as_deref(), Some("mastra"));
    }

    #[test]
    fn pg_provider_rejects_blank_connection_string() {
        let error = PgProvider::new(PgStoreConfig {
            connection_string: " ".into(),
            schema: None,
        })
        .expect_err("blank connection string should be rejected");

        assert_eq!(error, ProviderConfigError::EmptyField("connection_string"));
    }

    #[tokio::test]
    async fn pg_store_uses_in_memory_backend_for_now() {
        let store = PgStore::new(PgStoreConfig {
            connection_string: "postgres://localhost/test".into(),
            schema: Some("mastra".into()),
        });
        let thread = store
            .create_thread(CreateThreadRequest::new("resource-1", "chat"))
            .await
            .expect("thread should be created");

        store
            .append_message(mastra_memory::AppendMessageRequest::new(
                thread.id,
                MessageRole::Assistant,
                "hello pg",
            ))
            .await
            .expect("message should be written");

        let cloned = store
            .clone_thread(CloneThreadRequest::new(thread.id).with_title("copy"))
            .await
            .expect("thread should be cloned");
        let history = store
            .history(HistoryQuery {
                thread_id: cloned.id,
                limit: None,
            })
            .await
            .expect("history should be available");

        assert_eq!(store.config().schema.as_deref(), Some("mastra"));
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].text, "hello pg");
    }
}
