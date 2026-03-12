#[path = "../../_test-utils/src/provider_support.rs"]
mod provider_support;

use std::str::FromStr;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use mastra_memory::{
    AppendMessageRequest, CloneThreadRequest, CreateThreadRequest, DeleteMessagesRequest,
    HistoryQuery, ListMessagesQuery, ListThreadsQuery, MemoryStore, MemoryStoreError,
    MemoryStoreResult, Message, MessagePage, MessageRole, Thread, ThreadPage,
    ensure_valid_pagination,
};
use provider_support::ensure_not_blank;
pub use provider_support::{
    ProviderBinding, ProviderBridge, ProviderCapability, ProviderConfigError, ProviderDescriptor,
    ProviderKind,
};
use serde::{Deserialize, Serialize};
use sqlx::{
    Row, SqlitePool,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions, SqliteRow},
};
use tokio::sync::OnceCell;
use uuid::Uuid;

const LIBSQL_CAPABILITIES: &[ProviderCapability] = &[ProviderCapability::MemoryStore];
const LIBSQL_DESCRIPTOR: ProviderDescriptor =
    ProviderDescriptor::new("libsql", ProviderKind::Storage, LIBSQL_CAPABILITIES);

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

impl LibSqlStoreConfig {
    pub fn validate(&self) -> Result<(), ProviderConfigError> {
        ensure_not_blank(&self.url, "url")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibSqlProvider {
    config: LibSqlStoreConfig,
}

impl LibSqlProvider {
    pub fn new(config: LibSqlStoreConfig) -> Result<Self, ProviderConfigError> {
        config.validate()?;
        Ok(Self { config })
    }

    pub fn config(&self) -> &LibSqlStoreConfig {
        &self.config
    }

    pub fn descriptor(&self) -> ProviderDescriptor {
        LIBSQL_DESCRIPTOR
    }

    pub fn storage_bridge(&self) -> ProviderBridge {
        ProviderBridge::new(LIBSQL_DESCRIPTOR, self.config.url.clone())
            .with_binding(ProviderBinding::plain("url", self.config.url.clone()))
    }

    pub fn build_store(&self) -> LibSqlStore {
        LibSqlStore::new(self.config.clone())
    }
}

#[derive(Debug, Default)]
pub struct LibSqlStore {
    config: LibSqlStoreConfig,
    pool: OnceCell<SqlitePool>,
}

impl Clone for LibSqlStore {
    fn clone(&self) -> Self {
        let clone = Self::new(self.config.clone());
        if let Some(pool) = self.pool.get() {
            let _ = clone.pool.set(pool.clone());
        }
        clone
    }
}

impl LibSqlStore {
    pub fn new(config: LibSqlStoreConfig) -> Self {
        Self {
            config,
            pool: OnceCell::new(),
        }
    }

    pub fn config(&self) -> &LibSqlStoreConfig {
        &self.config
    }

    async fn pool(&self) -> MemoryStoreResult<&SqlitePool> {
        self.pool
            .get_or_try_init(|| async {
                let connect_options =
                    SqliteConnectOptions::from_str(&normalize_sqlite_url(&self.config.url))
                        .map_err(|error| {
                            MemoryStoreError::Backend(format!(
                                "parse libsql store url '{}': {error}",
                                self.config.url
                            ))
                        })?
                        .create_if_missing(true);
                let pool = SqlitePoolOptions::new()
                    .max_connections(1)
                    .connect_with(connect_options)
                    .await
                    .map_err(map_sqlx_error)?;
                initialize_schema(&pool).await?;
                Ok(pool)
            })
            .await
    }
}

#[async_trait]
impl MemoryStore for LibSqlStore {
    async fn create_thread(&self, input: CreateThreadRequest) -> MemoryStoreResult<Thread> {
        let pool = self.pool().await?;
        let now = Utc::now();
        let thread = Thread {
            id: input.thread_id.unwrap_or_else(Uuid::new_v4),
            resource_id: input.resource_id,
            title: input.title,
            metadata: input.metadata,
            created_at: now,
            updated_at: now,
        };

        sqlx::query(
            "INSERT INTO threads (id, resource_id, title, metadata, created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(thread.id.to_string())
        .bind(&thread.resource_id)
        .bind(&thread.title)
        .bind(json_to_string(&thread.metadata)?)
        .bind(timestamp_to_string(thread.created_at))
        .bind(timestamp_to_string(thread.updated_at))
        .execute(pool)
        .await
        .map_err(map_sqlx_error)?;

        Ok(thread)
    }

    async fn get_thread(&self, thread_id: Uuid) -> MemoryStoreResult<Option<Thread>> {
        let pool = self.pool().await?;
        let row = sqlx::query(
            "SELECT id, resource_id, title, metadata, created_at, updated_at \
             FROM threads WHERE id = ?",
        )
        .bind(thread_id.to_string())
        .fetch_optional(pool)
        .await
        .map_err(map_sqlx_error)?;

        row.map(thread_from_row).transpose()
    }

    async fn list_threads(&self, query: ListThreadsQuery) -> MemoryStoreResult<ThreadPage> {
        ensure_valid_pagination(query.pagination)?;
        let pool = self.pool().await?;
        let total = if let Some(resource_id) = &query.resource_id {
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM threads WHERE resource_id = ?")
                .bind(resource_id)
                .fetch_one(pool)
                .await
                .map_err(map_sqlx_error)? as usize
        } else {
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM threads")
                .fetch_one(pool)
                .await
                .map_err(map_sqlx_error)? as usize
        };

        let rows = if let Some(resource_id) = &query.resource_id {
            sqlx::query(
                "SELECT id, resource_id, title, metadata, created_at, updated_at \
                 FROM threads WHERE resource_id = ? \
                 ORDER BY updated_at DESC, id ASC LIMIT ? OFFSET ?",
            )
            .bind(resource_id)
            .bind(query.pagination.per_page as i64)
            .bind(query.pagination.offset() as i64)
            .fetch_all(pool)
            .await
            .map_err(map_sqlx_error)?
        } else {
            sqlx::query(
                "SELECT id, resource_id, title, metadata, created_at, updated_at \
                 FROM threads ORDER BY updated_at DESC, id ASC LIMIT ? OFFSET ?",
            )
            .bind(query.pagination.per_page as i64)
            .bind(query.pagination.offset() as i64)
            .fetch_all(pool)
            .await
            .map_err(map_sqlx_error)?
        };

        let items = rows
            .into_iter()
            .map(thread_from_row)
            .collect::<MemoryStoreResult<Vec<_>>>()?;

        Ok(ThreadPage {
            items,
            total,
            page: query.pagination.page,
            per_page: query.pagination.per_page,
        })
    }

    async fn append_message(&self, input: AppendMessageRequest) -> MemoryStoreResult<Message> {
        let pool = self.pool().await?;
        if self.get_thread(input.thread_id).await?.is_none() {
            return Err(MemoryStoreError::ThreadNotFound(input.thread_id));
        }

        let message = Message {
            id: input.message_id.unwrap_or_else(Uuid::new_v4),
            thread_id: input.thread_id,
            role: input.role,
            text: input.text,
            metadata: input.metadata,
            created_at: input.created_at.unwrap_or_else(Utc::now),
        };

        sqlx::query(
            "INSERT INTO messages (id, thread_id, role, text, metadata, created_at) \
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(message.id.to_string())
        .bind(message.thread_id.to_string())
        .bind(role_to_string(message.role.clone()))
        .bind(&message.text)
        .bind(json_to_string(&message.metadata)?)
        .bind(timestamp_to_string(message.created_at))
        .execute(pool)
        .await
        .map_err(map_sqlx_error)?;

        sqlx::query("UPDATE threads SET updated_at = ? WHERE id = ?")
            .bind(timestamp_to_string(message.created_at))
            .bind(message.thread_id.to_string())
            .execute(pool)
            .await
            .map_err(map_sqlx_error)?;

        Ok(message)
    }

    async fn list_messages(&self, query: ListMessagesQuery) -> MemoryStoreResult<MessagePage> {
        ensure_valid_pagination(query.pagination)?;
        let pool = self.pool().await?;
        if self.get_thread(query.thread_id).await?.is_none() {
            return Err(MemoryStoreError::ThreadNotFound(query.thread_id));
        }

        let total =
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM messages WHERE thread_id = ?")
                .bind(query.thread_id.to_string())
                .fetch_one(pool)
                .await
                .map_err(map_sqlx_error)? as usize;

        let rows = sqlx::query(
            "SELECT id, thread_id, role, text, metadata, created_at \
             FROM messages WHERE thread_id = ? \
             ORDER BY created_at ASC, id ASC LIMIT ? OFFSET ?",
        )
        .bind(query.thread_id.to_string())
        .bind(query.pagination.per_page as i64)
        .bind(query.pagination.offset() as i64)
        .fetch_all(pool)
        .await
        .map_err(map_sqlx_error)?;

        let items = rows
            .into_iter()
            .map(message_from_row)
            .collect::<MemoryStoreResult<Vec<_>>>()?;

        Ok(MessagePage {
            items,
            total,
            page: query.pagination.page,
            per_page: query.pagination.per_page,
        })
    }

    async fn history(&self, query: HistoryQuery) -> MemoryStoreResult<Vec<Message>> {
        let pool = self.pool().await?;
        if self.get_thread(query.thread_id).await?.is_none() {
            return Err(MemoryStoreError::ThreadNotFound(query.thread_id));
        }

        let limit = query.limit.unwrap_or(usize::MAX);
        let rows = sqlx::query(
            "SELECT id, thread_id, role, text, metadata, created_at \
             FROM messages WHERE thread_id = ? ORDER BY created_at DESC, id DESC LIMIT ?",
        )
        .bind(query.thread_id.to_string())
        .bind(limit as i64)
        .fetch_all(pool)
        .await
        .map_err(map_sqlx_error)?;

        let mut items = rows
            .into_iter()
            .map(message_from_row)
            .collect::<MemoryStoreResult<Vec<_>>>()?;
        items.reverse();
        Ok(items)
    }

    async fn clone_thread(&self, input: CloneThreadRequest) -> MemoryStoreResult<Thread> {
        let pool = self.pool().await?;
        let source_thread = self
            .get_thread(input.source_thread_id)
            .await?
            .ok_or(MemoryStoreError::ThreadNotFound(input.source_thread_id))?;
        let source_messages = self
            .history(HistoryQuery {
                thread_id: input.source_thread_id,
                limit: None,
            })
            .await?;
        let created_at = Utc::now();
        let updated_at = source_messages
            .last()
            .map(|message| message.created_at)
            .unwrap_or(created_at);
        let cloned_thread = Thread {
            id: input.new_thread_id.unwrap_or_else(Uuid::new_v4),
            resource_id: input.resource_id.unwrap_or(source_thread.resource_id),
            title: input
                .title
                .unwrap_or_else(|| format!("{} (copy)", source_thread.title)),
            metadata: input.metadata.unwrap_or(source_thread.metadata),
            created_at,
            updated_at,
        };

        let mut tx = pool.begin().await.map_err(map_sqlx_error)?;
        sqlx::query(
            "INSERT INTO threads (id, resource_id, title, metadata, created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(cloned_thread.id.to_string())
        .bind(&cloned_thread.resource_id)
        .bind(&cloned_thread.title)
        .bind(json_to_string(&cloned_thread.metadata)?)
        .bind(timestamp_to_string(cloned_thread.created_at))
        .bind(timestamp_to_string(cloned_thread.updated_at))
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx_error)?;

        for message in source_messages {
            sqlx::query(
                "INSERT INTO messages (id, thread_id, role, text, metadata, created_at) \
                 VALUES (?, ?, ?, ?, ?, ?)",
            )
            .bind(Uuid::new_v4().to_string())
            .bind(cloned_thread.id.to_string())
            .bind(role_to_string(message.role.clone()))
            .bind(message.text)
            .bind(json_to_string(&message.metadata)?)
            .bind(timestamp_to_string(message.created_at))
            .execute(&mut *tx)
            .await
            .map_err(map_sqlx_error)?;
        }

        tx.commit().await.map_err(map_sqlx_error)?;
        Ok(cloned_thread)
    }

    async fn delete_messages(&self, input: DeleteMessagesRequest) -> MemoryStoreResult<usize> {
        let pool = self.pool().await?;
        let thread = self
            .get_thread(input.thread_id)
            .await?
            .ok_or(MemoryStoreError::ThreadNotFound(input.thread_id))?;
        if input.message_ids.is_empty() {
            return Ok(0);
        }

        let mut tx = pool.begin().await.map_err(map_sqlx_error)?;
        let mut deleted = 0usize;
        for message_id in input.message_ids {
            deleted += sqlx::query("DELETE FROM messages WHERE thread_id = ? AND id = ?")
                .bind(input.thread_id.to_string())
                .bind(message_id.to_string())
                .execute(&mut *tx)
                .await
                .map_err(map_sqlx_error)?
                .rows_affected() as usize;
        }

        if deleted > 0 {
            let latest_created_at = sqlx::query_scalar::<_, String>(
                "SELECT created_at FROM messages WHERE thread_id = ? \
                 ORDER BY created_at DESC, id DESC LIMIT 1",
            )
            .bind(input.thread_id.to_string())
            .fetch_optional(&mut *tx)
            .await
            .map_err(map_sqlx_error)?;
            let updated_at = match latest_created_at {
                Some(timestamp) => parse_timestamp(&timestamp, "message created_at")?,
                None => thread.created_at,
            };

            sqlx::query("UPDATE threads SET updated_at = ? WHERE id = ?")
                .bind(timestamp_to_string(updated_at))
                .bind(input.thread_id.to_string())
                .execute(&mut *tx)
                .await
                .map_err(map_sqlx_error)?;
        }

        tx.commit().await.map_err(map_sqlx_error)?;
        Ok(deleted)
    }

    async fn delete_thread(&self, thread_id: Uuid) -> MemoryStoreResult<()> {
        let pool = self.pool().await?;
        if self.get_thread(thread_id).await?.is_none() {
            return Err(MemoryStoreError::ThreadNotFound(thread_id));
        }

        let mut tx = pool.begin().await.map_err(map_sqlx_error)?;
        sqlx::query("DELETE FROM messages WHERE thread_id = ?")
            .bind(thread_id.to_string())
            .execute(&mut *tx)
            .await
            .map_err(map_sqlx_error)?;
        sqlx::query("DELETE FROM threads WHERE id = ?")
            .bind(thread_id.to_string())
            .execute(&mut *tx)
            .await
            .map_err(map_sqlx_error)?;
        tx.commit().await.map_err(map_sqlx_error)?;
        Ok(())
    }
}

async fn initialize_schema(pool: &SqlitePool) -> MemoryStoreResult<()> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS threads (
            id TEXT PRIMARY KEY,
            resource_id TEXT NOT NULL,
            title TEXT NOT NULL,
            metadata TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        )",
    )
    .execute(pool)
    .await
    .map_err(map_sqlx_error)?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS messages (
            id TEXT PRIMARY KEY,
            thread_id TEXT NOT NULL,
            role TEXT NOT NULL,
            text TEXT NOT NULL,
            metadata TEXT NOT NULL,
            created_at TEXT NOT NULL,
            FOREIGN KEY(thread_id) REFERENCES threads(id) ON DELETE CASCADE
        )",
    )
    .execute(pool)
    .await
    .map_err(map_sqlx_error)?;

    Ok(())
}

fn normalize_sqlite_url(url: &str) -> String {
    if url == "file::memory:" {
        return "sqlite::memory:".to_string();
    }

    if let Some(path) = url.strip_prefix("file:") {
        return format!("sqlite://{path}");
    }

    if url.starts_with("sqlite:") {
        return url.to_string();
    }

    format!("sqlite://{url}")
}

fn json_to_string(value: &serde_json::Value) -> MemoryStoreResult<String> {
    serde_json::to_string(value)
        .map_err(|error| MemoryStoreError::Backend(format!("serialize json payload: {error}")))
}

fn role_to_string(role: MessageRole) -> &'static str {
    match role {
        MessageRole::System => "system",
        MessageRole::User => "user",
        MessageRole::Assistant => "assistant",
        MessageRole::Tool => "tool",
    }
}

fn role_from_str(role: &str) -> MemoryStoreResult<MessageRole> {
    match role {
        "system" => Ok(MessageRole::System),
        "user" => Ok(MessageRole::User),
        "assistant" => Ok(MessageRole::Assistant),
        "tool" => Ok(MessageRole::Tool),
        other => Err(MemoryStoreError::Backend(format!(
            "unknown message role '{other}' in libsql store"
        ))),
    }
}

fn timestamp_to_string(value: DateTime<Utc>) -> String {
    value.to_rfc3339()
}

fn parse_timestamp(value: &str, field: &str) -> MemoryStoreResult<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .map(|timestamp| timestamp.with_timezone(&Utc))
        .map_err(|error| {
            MemoryStoreError::Backend(format!(
                "parse timestamp for {field} in libsql store: {error}"
            ))
        })
}

fn thread_from_row(row: SqliteRow) -> MemoryStoreResult<Thread> {
    Ok(Thread {
        id: parse_uuid(row.get::<String, _>("id"), "thread id")?,
        resource_id: row.get("resource_id"),
        title: row.get("title"),
        metadata: parse_json(row.get::<String, _>("metadata"), "thread metadata")?,
        created_at: parse_timestamp(&row.get::<String, _>("created_at"), "thread created_at")?,
        updated_at: parse_timestamp(&row.get::<String, _>("updated_at"), "thread updated_at")?,
    })
}

fn message_from_row(row: SqliteRow) -> MemoryStoreResult<Message> {
    Ok(Message {
        id: parse_uuid(row.get::<String, _>("id"), "message id")?,
        thread_id: parse_uuid(row.get::<String, _>("thread_id"), "message thread_id")?,
        role: role_from_str(&row.get::<String, _>("role"))?,
        text: row.get("text"),
        metadata: parse_json(row.get::<String, _>("metadata"), "message metadata")?,
        created_at: parse_timestamp(&row.get::<String, _>("created_at"), "message created_at")?,
    })
}

fn parse_uuid(value: String, field: &str) -> MemoryStoreResult<Uuid> {
    Uuid::parse_str(&value)
        .map_err(|error| MemoryStoreError::Backend(format!("parse uuid for {field}: {error}")))
}

fn parse_json(value: String, field: &str) -> MemoryStoreResult<serde_json::Value> {
    serde_json::from_str(&value)
        .map_err(|error| MemoryStoreError::Backend(format!("parse json for {field}: {error}")))
}

fn map_sqlx_error(error: sqlx::Error) -> MemoryStoreError {
    MemoryStoreError::Backend(format!("libsql store operation failed: {error}"))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use mastra_memory::{HistoryQuery, MessageRole, Pagination};
    use uuid::Uuid;

    use super::{
        CreateThreadRequest, LibSqlProvider, LibSqlStore, LibSqlStoreConfig, MemoryStore,
        ProviderCapability, ProviderConfigError, ProviderKind,
    };

    #[test]
    fn libsql_provider_exposes_storage_bridge() {
        let provider = LibSqlProvider::new(LibSqlStoreConfig {
            url: "file:provider-test?mode=memory&cache=shared".into(),
        })
        .expect("libsql config should be valid");

        let descriptor = provider.descriptor();
        let bridge = provider.storage_bridge();
        let store = provider.build_store();

        assert_eq!(descriptor.id, "libsql");
        assert_eq!(descriptor.kind, ProviderKind::Storage);
        assert!(bridge.supports(ProviderCapability::MemoryStore));
        assert_eq!(bridge.target, "file:provider-test?mode=memory&cache=shared");
        assert_eq!(
            store.config().url,
            "file:provider-test?mode=memory&cache=shared"
        );
    }

    #[test]
    fn libsql_provider_rejects_blank_url() {
        let error = LibSqlProvider::new(LibSqlStoreConfig { url: " ".into() })
            .expect_err("blank url should be rejected");

        assert_eq!(error, ProviderConfigError::EmptyField("url"));
    }

    #[tokio::test]
    async fn libsql_store_persists_messages_across_instances() {
        let database_path = unique_database_path();
        let store = LibSqlStore::new(LibSqlStoreConfig {
            url: format!("file:{}", database_path.display()),
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

        drop(store);

        let reopened = LibSqlStore::new(LibSqlStoreConfig {
            url: format!("file:{}", database_path.display()),
        });
        let history = reopened
            .history(HistoryQuery {
                thread_id: thread.id,
                limit: None,
            })
            .await
            .expect("reopened store should load history");

        assert_eq!(
            reopened.config().url,
            format!("file:{}", database_path.display())
        );
        assert_eq!(messages.total, 1);
        assert_eq!(messages.items[0].text, "hello libsql");
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].text, "hello libsql");

        let _ = tokio::fs::remove_file(database_path).await;
    }

    fn unique_database_path() -> PathBuf {
        std::env::temp_dir().join(format!("mastra-libsql-{}.db", Uuid::new_v4()))
    }
}
