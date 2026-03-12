use std::sync::Arc;

use chrono::Utc;
use mastra_core::{
    CreateThreadRequest as CoreCreateThreadRequest, MemoryConfig, MemoryEngine, MemoryMessage,
    MemoryRole,
};
use serde_json::json;
use uuid::Uuid;

use crate::{
    AppendMessageRequest, CreateThreadRequest, HistoryQuery, InMemoryMemoryStore, Memory,
    ListMessagesQuery, ListThreadsQuery, MemoryStore, MessageRole, Pagination,
};

#[tokio::test]
async fn in_memory_store_tracks_threads_and_messages() {
    let store = InMemoryMemoryStore::default();

    let mut first_thread = CreateThreadRequest::new("user-1", "First");
    first_thread.metadata = json!({ "kind": "primary" });
    let first_thread = store
        .create_thread(first_thread)
        .await
        .expect("thread should be created");

    let second_thread = store
        .create_thread(CreateThreadRequest::new("user-1", "Second"))
        .await
        .expect("thread should be created");

    let list = store
        .list_threads(ListThreadsQuery {
            resource_id: Some("user-1".into()),
            pagination: Pagination::new(0, 10),
        })
        .await
        .expect("threads should be listed");

    assert_eq!(list.total, 2);
    assert_eq!(list.items[0].id, second_thread.id);
    assert_eq!(list.items[1].metadata, json!({ "kind": "primary" }));

    let first_message = store
        .append_message(AppendMessageRequest::new(
            first_thread.id,
            MessageRole::User,
            "hello",
        ))
        .await
        .expect("message should be written");
    store
        .append_message(AppendMessageRequest::new(
            first_thread.id,
            MessageRole::Assistant,
            "world",
        ))
        .await
        .expect("message should be written");

    let page = store
        .list_messages(ListMessagesQuery {
            thread_id: first_thread.id,
            pagination: Pagination::new(0, 1),
        })
        .await
        .expect("messages should be listed");

    assert_eq!(page.total, 2);
    assert_eq!(page.items, vec![first_message]);
}

#[tokio::test]
async fn history_returns_latest_messages_in_original_order() {
    let store = InMemoryMemoryStore::default();
    let thread = store
        .create_thread(CreateThreadRequest::new("user-2", "History"))
        .await
        .expect("thread should be created");

    for index in 0..4 {
        store
            .append_message(AppendMessageRequest::new(
                thread.id,
                MessageRole::User,
                format!("message-{index}"),
            ))
            .await
            .expect("message should be written");
    }

    let history = store
        .history(HistoryQuery {
            thread_id: thread.id,
            limit: Some(2),
        })
        .await
        .expect("history should be available");

    let texts = history
        .into_iter()
        .map(|message| message.text)
        .collect::<Vec<_>>();
    assert_eq!(
        texts,
        vec!["message-2".to_string(), "message-3".to_string()]
    );
}

#[tokio::test]
async fn missing_thread_returns_error() {
    let store = InMemoryMemoryStore::default();
    let missing_thread_id = Uuid::new_v4();

    let error = store
        .history(HistoryQuery {
            thread_id: missing_thread_id,
            limit: None,
        })
        .await
        .expect_err("history without a thread should fail");

    assert_eq!(
        error.to_string(),
        format!("thread `{missing_thread_id}` was not found")
    );
}

#[tokio::test]
async fn bridge_implements_core_memory_engine() {
    let memory = Memory::new(InMemoryMemoryStore::default());
    let thread = memory
        .create_thread(CoreCreateThreadRequest {
            id: None,
            resource_id: Some("resource-bridge".into()),
            title: Some("Bridge".into()),
            metadata: json!({ "source": "test" }),
        })
        .await
        .expect("thread should be created");

    memory
        .append_messages(
            &thread.id,
            vec![MemoryMessage {
                id: Uuid::new_v4().to_string(),
                thread_id: thread.id.clone(),
                role: MemoryRole::User,
                content: "hello bridge".into(),
                created_at: Utc::now(),
                metadata: json!({}),
            }],
        )
        .await
        .expect("message should be stored");

    let messages = memory
        .list_messages(mastra_core::MemoryRecallRequest {
            thread_id: thread.id.clone(),
            limit: Some(10),
        })
        .await
        .expect("messages should be listed");

    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].content, "hello bridge");
    assert_eq!(messages[0].role, MemoryRole::User);

    let _ = Arc::new(memory) as Arc<dyn MemoryEngine>;
    let _ = MemoryConfig::default();
}
