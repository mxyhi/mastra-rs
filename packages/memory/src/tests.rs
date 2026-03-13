use std::sync::Arc;

use chrono::Utc;
use mastra_core::{
    CreateThreadRequest as CoreCreateThreadRequest, MemoryConfig, MemoryEngine, MemoryMessage,
    MemoryRole,
};
use serde_json::json;
use uuid::Uuid;

use crate::{
    AppendMessageRequest, CloneThreadRequest, CreateThreadRequest, DeleteMessagesRequest,
    HistoryQuery, InMemoryMemoryStore, ListMessagesQuery, ListThreadsQuery, Memory, MemoryStore,
    MessageRole, Pagination,
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
    let thread = MemoryEngine::create_thread(
        &memory,
        CoreCreateThreadRequest {
            id: None,
            resource_id: Some("resource-bridge".into()),
            title: Some("Bridge".into()),
            metadata: json!({ "source": "test" }),
        },
    )
    .await
    .expect("thread should be created");

    MemoryEngine::append_messages(
        &memory,
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

    let messages = MemoryEngine::list_messages(
        &memory,
        mastra_core::MemoryRecallRequest {
            thread_id: thread.id.clone(),
            limit: Some(10),
        },
    )
    .await
    .expect("messages should be listed");

    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].content, "hello bridge");
    assert_eq!(messages[0].role, MemoryRole::User);

    let _ = Arc::new(memory) as Arc<dyn MemoryEngine>;
    let _ = MemoryConfig::default();
}

#[tokio::test]
async fn bridge_deletes_messages_by_id_without_thread_context() {
    let memory = Memory::new(InMemoryMemoryStore::default());
    let first_thread = MemoryEngine::create_thread(
        &memory,
        CoreCreateThreadRequest {
            id: None,
            resource_id: Some("resource-bridge-delete".into()),
            title: Some("Bridge delete".into()),
            metadata: json!({}),
        },
    )
    .await
    .expect("first thread should be created");
    let second_thread = MemoryEngine::create_thread(
        &memory,
        CoreCreateThreadRequest {
            id: None,
            resource_id: Some("resource-bridge-delete".into()),
            title: Some("Bridge delete 2".into()),
            metadata: json!({}),
        },
    )
    .await
    .expect("second thread should be created");

    MemoryEngine::append_messages(
        &memory,
        &first_thread.id,
        vec![MemoryMessage {
            id: Uuid::new_v4().to_string(),
            thread_id: first_thread.id.clone(),
            role: MemoryRole::User,
            content: "keep first".into(),
            created_at: Utc::now(),
            metadata: json!({}),
        }],
    )
    .await
    .expect("first thread message should be stored");
    let deleted_message_id = Uuid::new_v4().to_string();
    MemoryEngine::append_messages(
        &memory,
        &second_thread.id,
        vec![MemoryMessage {
            id: deleted_message_id.clone(),
            thread_id: second_thread.id.clone(),
            role: MemoryRole::Assistant,
            content: "delete second".into(),
            created_at: Utc::now(),
            metadata: json!({}),
        }],
    )
    .await
    .expect("second thread message should be stored");

    let deleted = MemoryEngine::delete_messages(&memory, vec![deleted_message_id.clone()])
        .await
        .expect("message should be deleted by id");
    let second_thread_messages = MemoryEngine::list_messages(
        &memory,
        mastra_core::MemoryRecallRequest {
            thread_id: second_thread.id.clone(),
            limit: Some(10),
        },
    )
    .await
    .expect("second thread messages should be listed");
    let first_thread_messages = MemoryEngine::list_messages(
        &memory,
        mastra_core::MemoryRecallRequest {
            thread_id: first_thread.id.clone(),
            limit: Some(10),
        },
    )
    .await
    .expect("first thread messages should be listed");

    assert_eq!(deleted, 1);
    assert!(second_thread_messages.is_empty());
    assert_eq!(first_thread_messages.len(), 1);
    assert_eq!(first_thread_messages[0].content, "keep first");
}

#[tokio::test]
async fn memory_facade_lists_threads_and_messages() {
    let memory = Memory::in_memory();
    let thread = memory
        .create_thread(CreateThreadRequest::new("resource-list", "List demo"))
        .await
        .expect("thread should be created");

    memory
        .append_message(AppendMessageRequest::new(
            thread.id,
            MessageRole::User,
            "hello facade",
        ))
        .await
        .expect("message should be appended");

    let threads = memory
        .list_threads(ListThreadsQuery {
            resource_id: Some("resource-list".into()),
            pagination: Pagination::new(0, 10),
        })
        .await
        .expect("threads should be listed");
    let messages = memory
        .list_messages_page(ListMessagesQuery {
            thread_id: thread.id,
            pagination: Pagination::new(0, 10),
        })
        .await
        .expect("messages should be listed");

    assert_eq!(threads.total, 1);
    assert_eq!(threads.items[0].title, "List demo");
    assert_eq!(messages.total, 1);
    assert_eq!(messages.items[0].text, "hello facade");
}

#[tokio::test]
async fn clone_thread_copies_history_and_applies_overrides() {
    let memory = Memory::in_memory();
    let mut thread_request = CreateThreadRequest::new("resource-clone", "Original");
    thread_request.metadata = json!({ "scope": "seed" });
    let thread = memory
        .create_thread(thread_request)
        .await
        .expect("thread should be created");

    memory
        .append_message(AppendMessageRequest::new(
            thread.id,
            MessageRole::User,
            "message-one",
        ))
        .await
        .expect("first message should be appended");
    memory
        .append_message(AppendMessageRequest::new(
            thread.id,
            MessageRole::Assistant,
            "message-two",
        ))
        .await
        .expect("second message should be appended");

    let cloned = memory
        .clone_thread(
            CloneThreadRequest::new(thread.id)
                .with_title("Cloned")
                .with_resource_id("resource-copy"),
        )
        .await
        .expect("thread should be cloned");
    let cloned_messages = memory
        .list_messages_page(ListMessagesQuery {
            thread_id: cloned.id,
            pagination: Pagination::new(0, 10),
        })
        .await
        .expect("cloned messages should be listed");

    assert_ne!(cloned.id, thread.id);
    assert_eq!(cloned.resource_id, "resource-copy");
    assert_eq!(cloned.title, "Cloned");
    assert_eq!(cloned.metadata, json!({ "scope": "seed" }));
    assert_eq!(
        cloned_messages
            .items
            .into_iter()
            .map(|message| message.text)
            .collect::<Vec<_>>(),
        vec!["message-one".to_string(), "message-two".to_string()]
    );
}

#[tokio::test]
async fn delete_messages_prunes_only_selected_entries() {
    let memory = Memory::in_memory();
    let thread = memory
        .create_thread(CreateThreadRequest::new(
            "resource-delete",
            "Delete messages",
        ))
        .await
        .expect("thread should be created");

    let first = memory
        .append_message(AppendMessageRequest::new(
            thread.id,
            MessageRole::User,
            "keep me?",
        ))
        .await
        .expect("first message should be appended");
    let second = memory
        .append_message(AppendMessageRequest::new(
            thread.id,
            MessageRole::Assistant,
            "delete me",
        ))
        .await
        .expect("second message should be appended");

    let deleted = memory
        .delete_messages(DeleteMessagesRequest::new(thread.id, vec![second.id]))
        .await
        .expect("messages should be deleted");
    let remaining = memory
        .list_messages_page(ListMessagesQuery {
            thread_id: thread.id,
            pagination: Pagination::new(0, 10),
        })
        .await
        .expect("messages should be listed");

    assert_eq!(deleted, 1);
    assert_eq!(remaining.total, 1);
    assert_eq!(remaining.items[0].id, first.id);
}

#[tokio::test]
async fn delete_thread_removes_thread_and_history() {
    let memory = Memory::in_memory();
    let thread = memory
        .create_thread(CreateThreadRequest::new(
            "resource-thread-delete",
            "Delete thread",
        ))
        .await
        .expect("thread should be created");

    memory
        .append_message(AppendMessageRequest::new(
            thread.id,
            MessageRole::User,
            "gone soon",
        ))
        .await
        .expect("message should be appended");

    memory
        .delete_thread(thread.id)
        .await
        .expect("thread should be deleted");

    let thread = memory
        .get_thread(thread.id)
        .await
        .expect("thread lookup should succeed");
    let threads = memory
        .list_threads(ListThreadsQuery {
            resource_id: Some("resource-thread-delete".into()),
            pagination: Pagination::new(0, 10),
        })
        .await
        .expect("threads should be listed");

    assert!(thread.is_none());
    assert_eq!(threads.total, 0);
}
