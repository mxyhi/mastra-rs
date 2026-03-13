use async_trait::async_trait;
use chrono::Utc;
use serde_json::Value;
use thiserror::Error;
use uuid::Uuid;

use crate::model::{
    AppendMessageRequest, AppendObservationRequest, CloneThreadRequest, CreateThreadRequest,
    DeleteMessagesRequest, HistoryQuery, ListMessagesQuery, ListObservationsQuery,
    ListThreadsQuery, Message, MessagePage, Observation, Pagination, Thread, ThreadPage,
    UpdateThreadRequest, UpdateWorkingMemoryRequest, WorkingMemory,
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

pub(crate) const WORKING_MEMORY_METADATA_KEY: &str = "workingMemory";
pub(crate) const OBSERVATIONAL_MEMORY_METADATA_KEY: &str = "observationalMemory";

#[async_trait]
pub trait MemoryStore: Send + Sync {
    async fn create_thread(&self, input: CreateThreadRequest) -> MemoryStoreResult<Thread>;

    async fn get_thread(&self, thread_id: Uuid) -> MemoryStoreResult<Option<Thread>>;

    async fn update_thread(&self, input: UpdateThreadRequest) -> MemoryStoreResult<Thread>;

    async fn list_threads(&self, query: ListThreadsQuery) -> MemoryStoreResult<ThreadPage>;

    async fn append_message(&self, input: AppendMessageRequest) -> MemoryStoreResult<Message>;

    async fn list_messages(&self, query: ListMessagesQuery) -> MemoryStoreResult<MessagePage>;

    async fn history(&self, query: HistoryQuery) -> MemoryStoreResult<Vec<Message>>;

    async fn clone_thread(&self, input: CloneThreadRequest) -> MemoryStoreResult<Thread>;

    async fn get_working_memory(
        &self,
        thread_id: Uuid,
        resource_id: Option<&str>,
    ) -> MemoryStoreResult<Option<WorkingMemory>> {
        let thread = self
            .get_thread(thread_id)
            .await?
            .ok_or(MemoryStoreError::ThreadNotFound(thread_id))?;
        if let Some(working_memory) = working_memory_from_metadata(&thread.metadata) {
            return Ok(Some(working_memory));
        }

        let Some(resource_id) = resource_id.or(Some(thread.resource_id.as_str())) else {
            return Ok(None);
        };

        let threads = self
            .list_threads(ListThreadsQuery {
                resource_id: Some(resource_id.to_owned()),
                pagination: Pagination::new(0, usize::MAX / 2),
            })
            .await?;
        Ok(threads.items.into_iter().find_map(|thread| {
            let working_memory = working_memory_from_metadata(&thread.metadata)?;
            (working_memory.resource_id.as_deref() == Some(resource_id)
                && matches!(working_memory.scope, mastra_core::MemoryScope::Resource))
            .then_some(working_memory)
        }))
    }

    async fn update_working_memory(
        &self,
        input: UpdateWorkingMemoryRequest,
    ) -> MemoryStoreResult<WorkingMemory> {
        let thread = self
            .get_thread(input.thread_id)
            .await?
            .ok_or(MemoryStoreError::ThreadNotFound(input.thread_id))?;
        let resource_id = input
            .resource_id
            .clone()
            .or_else(|| Some(thread.resource_id.clone()));
        let state = WorkingMemory {
            thread_id: input.thread_id,
            resource_id: resource_id.clone(),
            scope: input.scope,
            format: input.format,
            template: input.template.clone(),
            content: input.content,
            updated_at: Utc::now(),
        };

        let target_thread_ids = if matches!(state.scope, mastra_core::MemoryScope::Resource) {
            self.list_threads(ListThreadsQuery {
                resource_id: resource_id.clone(),
                pagination: Pagination::new(0, usize::MAX / 2),
            })
            .await?
            .items
            .into_iter()
            .map(|thread| thread.id)
            .collect::<Vec<_>>()
        } else {
            vec![input.thread_id]
        };

        for thread_id in target_thread_ids {
            let existing = self
                .get_thread(thread_id)
                .await?
                .ok_or(MemoryStoreError::ThreadNotFound(thread_id))?;
            self.update_thread(UpdateThreadRequest {
                thread_id,
                resource_id: None,
                title: None,
                metadata: Some(metadata_with_working_memory(&existing.metadata, &state)),
            })
            .await?;
        }

        Ok(state)
    }

    async fn list_observations(
        &self,
        query: ListObservationsQuery,
    ) -> MemoryStoreResult<Vec<Observation>> {
        let thread = self
            .get_thread(query.thread_id)
            .await?
            .ok_or(MemoryStoreError::ThreadNotFound(query.thread_id))?;
        let mut observations = observations_from_metadata(&thread.metadata);
        if let Some(resource_id) = query.resource_id.as_deref() {
            observations
                .retain(|observation| observation.resource_id.as_deref() == Some(resource_id));
        }
        if let Some(page) = query.page {
            let per_page = query.per_page.unwrap_or_else(|| observations.len().max(1));
            if per_page == 0 {
                return Err(MemoryStoreError::InvalidPagination);
            }
            let start = page.saturating_mul(per_page);
            observations = observations
                .into_iter()
                .skip(start)
                .take(per_page)
                .collect();
        }
        Ok(observations)
    }

    async fn append_observation(
        &self,
        input: AppendObservationRequest,
    ) -> MemoryStoreResult<Observation> {
        let thread = self
            .get_thread(input.thread_id)
            .await?
            .ok_or(MemoryStoreError::ThreadNotFound(input.thread_id))?;
        let now = Utc::now();
        let observation = Observation {
            id: Uuid::now_v7(),
            thread_id: input.thread_id,
            resource_id: input
                .resource_id
                .or_else(|| Some(thread.resource_id.clone())),
            scope: input.scope,
            content: input.content,
            observed_message_ids: input.observed_message_ids,
            metadata: input.metadata,
            created_at: now,
            updated_at: now,
        };
        let mut observations = observations_from_metadata(&thread.metadata);
        observations.push(observation.clone());
        self.update_thread(UpdateThreadRequest {
            thread_id: thread.id,
            resource_id: None,
            title: None,
            metadata: Some(metadata_with_observations(&thread.metadata, &observations)),
        })
        .await?;

        Ok(observation)
    }

    async fn delete_messages(&self, input: DeleteMessagesRequest) -> MemoryStoreResult<usize>;

    async fn delete_thread(&self, thread_id: Uuid) -> MemoryStoreResult<()>;
}

pub fn ensure_valid_pagination(pagination: Pagination) -> MemoryStoreResult<()> {
    if pagination.per_page == 0 {
        return Err(MemoryStoreError::InvalidPagination);
    }

    Ok(())
}

pub(crate) fn working_memory_from_metadata(metadata: &Value) -> Option<WorkingMemory> {
    metadata
        .as_object()
        .and_then(|object| object.get(WORKING_MEMORY_METADATA_KEY))
        .and_then(|value| serde_json::from_value(value.clone()).ok())
}

pub(crate) fn metadata_with_working_memory(
    metadata: &Value,
    working_memory: &WorkingMemory,
) -> Value {
    let mut object = metadata.as_object().cloned().unwrap_or_default();
    object.insert(
        WORKING_MEMORY_METADATA_KEY.to_owned(),
        serde_json::to_value(working_memory).expect("working memory should serialize"),
    );
    Value::Object(object)
}

pub(crate) fn observations_from_metadata(metadata: &Value) -> Vec<Observation> {
    metadata
        .as_object()
        .and_then(|object| object.get(OBSERVATIONAL_MEMORY_METADATA_KEY))
        .and_then(|value| serde_json::from_value(value.clone()).ok())
        .unwrap_or_default()
}

pub(crate) fn metadata_with_observations(metadata: &Value, observations: &[Observation]) -> Value {
    let mut object = metadata.as_object().cloned().unwrap_or_default();
    object.insert(
        OBSERVATIONAL_MEMORY_METADATA_KEY.to_owned(),
        serde_json::to_value(observations).expect("observations should serialize"),
    );
    Value::Object(object)
}
