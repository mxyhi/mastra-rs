mod in_memory;
mod model;
mod store;

use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use mastra_core::{
    AppendObservationRequest as CoreAppendObservationRequest,
    CloneThreadRequest as CoreCloneThreadRequest, CreateThreadRequest as CoreCreateThreadRequest,
    MastraError, MemoryEngine, MemoryMessage, MemoryRecallRequest, MemoryRole,
    ObservationPage as CoreObservationPage, ObservationQuery as CoreObservationQuery,
    ObservationRecord as CoreObservationRecord, Thread as CoreThread,
    UpdateWorkingMemoryRequest as CoreUpdateWorkingMemoryRequest,
    WorkingMemoryState as CoreWorkingMemoryState,
};
use serde_json::{Map, Value};
use uuid::Uuid;

pub use in_memory::InMemoryMemoryStore;
pub use mastra_core::{MemoryScope, WorkingMemoryFormat};
pub use model::{
    AppendMessageRequest, AppendObservationRequest, CloneThreadRequest, CreateThreadRequest,
    DeleteMessagesRequest, HistoryQuery, ListMessagesQuery, ListObservationsQuery,
    ListThreadsQuery, Message, MessagePage, MessageRole, Observation, ObservationPage,
    Pagination, Thread, ThreadPage, UpdateThreadRequest, UpdateWorkingMemoryRequest, WorkingMemory,
};
pub use store::{MemoryStore, MemoryStoreError, MemoryStoreResult, ensure_valid_pagination};

const WORKING_MEMORY_METADATA_KEY: &str = "workingMemory";
const OBSERVATIONS_METADATA_KEY: &str = "observationalMemory";

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
        let thread = self.store.create_thread(input).await?;
        if working_memory_from_thread(&thread).is_none() {
            if let Some(working_memory) = self
                .resource_scoped_working_memory(&thread.resource_id)
                .await?
            {
                self.persist_working_memory_on_thread(thread.id, &working_memory)
                    .await?;
                return self
                    .store
                    .get_thread(thread.id)
                    .await?
                    .ok_or(MemoryStoreError::ThreadNotFound(thread.id));
            }
        }

        Ok(thread)
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
        let source_thread = self
            .store
            .get_thread(input.source_thread_id)
            .await?
            .ok_or(MemoryStoreError::ThreadNotFound(input.source_thread_id))?;
        let source_messages = self
            .store
            .list_messages(ListMessagesQuery {
                thread_id: input.source_thread_id,
                pagination: unbounded_pagination(),
            })
            .await?
            .items;
        let filtered_source_messages = filter_cloned_messages_facade(source_messages, &input);
        let cloned_thread = self.store.clone_thread(input).await?;

        if let Some(working_memory) = working_memory_from_thread(&source_thread) {
            self.persist_working_memory_on_thread(cloned_thread.id, &working_memory)
                .await?;
        }

        let source_observations = observations_from_thread(&source_thread);
        if !source_observations.is_empty() {
            let cloned_messages = self
                .store
                .list_messages(ListMessagesQuery {
                    thread_id: cloned_thread.id,
                    pagination: unbounded_pagination(),
                })
                .await?
                .items;
            let message_id_map = filtered_source_messages
                .iter()
                .zip(cloned_messages.iter())
                .map(|(source, cloned)| (source.id, cloned.id))
                .collect::<HashMap<_, _>>();

            let remapped_observations = source_observations
                .into_iter()
                .map(|observation| Observation {
                    id: Uuid::new_v4(),
                    thread_id: cloned_thread.id,
                    resource_id: Some(cloned_thread.resource_id.clone()),
                    scope: observation.scope,
                    content: observation.content,
                    observed_message_ids: observation
                        .observed_message_ids
                        .iter()
                        .filter_map(|message_id| message_id_map.get(message_id).copied())
                        .collect::<Vec<_>>(),
                    metadata: observation.metadata,
                    created_at: observation.created_at,
                    updated_at: Utc::now(),
                })
                .collect::<Vec<_>>();
            self.replace_observations_on_thread(cloned_thread.id, &remapped_observations)
                .await?;
        }

        self.store
            .get_thread(cloned_thread.id)
            .await?
            .ok_or(MemoryStoreError::ThreadNotFound(cloned_thread.id))
    }

    pub async fn delete_messages(&self, input: DeleteMessagesRequest) -> MemoryStoreResult<usize> {
        self.store.delete_messages(input).await
    }

    pub async fn delete_thread(&self, thread_id: Uuid) -> MemoryStoreResult<()> {
        self.store.delete_thread(thread_id).await
    }

    pub async fn working_memory(
        &self,
        thread_id: Uuid,
    ) -> MemoryStoreResult<Option<WorkingMemory>> {
        let thread = self
            .store
            .get_thread(thread_id)
            .await?
            .ok_or(MemoryStoreError::ThreadNotFound(thread_id))?;

        if let Some(working_memory) = working_memory_from_thread(&thread) {
            return Ok(Some(working_memory));
        }

        self.resource_scoped_working_memory(&thread.resource_id)
            .await
            .map(|memory| {
                memory.map(|mut memory| {
                    memory.thread_id = thread_id;
                    memory
                })
            })
    }

    pub async fn update_working_memory(
        &self,
        request: UpdateWorkingMemoryRequest,
    ) -> MemoryStoreResult<WorkingMemory> {
        let format = request.format;
        self.update_working_memory_with_format(request, format)
            .await
    }

    async fn update_working_memory_with_format(
        &self,
        request: UpdateWorkingMemoryRequest,
        format: WorkingMemoryFormat,
    ) -> MemoryStoreResult<WorkingMemory> {
        let thread = self
            .store
            .get_thread(request.thread_id)
            .await?
            .ok_or(MemoryStoreError::ThreadNotFound(request.thread_id))?;
        let resource_id = request
            .resource_id
            .unwrap_or_else(|| thread.resource_id.clone());
        let working_memory = WorkingMemory {
            thread_id: request.thread_id,
            resource_id: Some(resource_id.clone()),
            scope: request.scope,
            format,
            template: request.template,
            content: request.content,
            updated_at: Utc::now(),
        };

        match working_memory.scope {
            MemoryScope::Thread => {
                self.persist_working_memory_on_thread(request.thread_id, &working_memory)
                    .await?;
            }
            MemoryScope::Resource => {
                let threads = self
                    .store
                    .list_threads(ListThreadsQuery {
                        resource_id: Some(resource_id.clone()),
                        pagination: unbounded_pagination(),
                    })
                    .await?;
                for thread in threads.items {
                    self.persist_working_memory_on_thread(thread.id, &working_memory)
                        .await?;
                }
            }
        }

        Ok(working_memory)
    }

    pub async fn observations(
        &self,
        query: ListObservationsQuery,
    ) -> MemoryStoreResult<Vec<Observation>> {
        let thread = self
            .store
            .get_thread(query.thread_id)
            .await?
            .ok_or(MemoryStoreError::ThreadNotFound(query.thread_id))?;
        let mut observations = observations_from_thread(&thread);
        if let Some(page) = query.page {
            let per_page = query.per_page.unwrap_or_else(|| observations.len().max(1));
            let start = page.saturating_mul(per_page);
            observations = observations
                .into_iter()
                .skip(start)
                .take(per_page)
                .collect();
        } else if let Some(per_page) = query.per_page {
            observations = observations.into_iter().take(per_page).collect();
        }
        Ok(observations)
    }

    pub async fn append_observation(
        &self,
        request: AppendObservationRequest,
    ) -> MemoryStoreResult<Observation> {
        let thread = self
            .store
            .get_thread(request.thread_id)
            .await?
            .ok_or(MemoryStoreError::ThreadNotFound(request.thread_id))?;
        let observation = Observation {
            id: Uuid::new_v4(),
            thread_id: request.thread_id,
            resource_id: Some(
                request
                    .resource_id
                    .unwrap_or_else(|| thread.resource_id.clone()),
            ),
            scope: request.scope,
            content: request.content,
            observed_message_ids: request.observed_message_ids,
            metadata: request.metadata,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        match observation.scope {
            MemoryScope::Thread => {
                self.persist_observation_on_thread(request.thread_id, &observation)
                    .await?;
            }
            MemoryScope::Resource => {
                let threads = self
                    .store
                    .list_threads(ListThreadsQuery {
                        resource_id: observation.resource_id.clone(),
                        pagination: unbounded_pagination(),
                    })
                    .await?;
                for thread in threads.items {
                    self.persist_observation_on_thread(thread.id, &observation)
                        .await?;
                }
            }
        }

        Ok(observation)
    }

    async fn persist_working_memory_on_thread(
        &self,
        thread_id: Uuid,
        working_memory: &WorkingMemory,
    ) -> MemoryStoreResult<()> {
        let thread = self
            .store
            .get_thread(thread_id)
            .await?
            .ok_or(MemoryStoreError::ThreadNotFound(thread_id))?;
        let mut metadata = thread.metadata;
        set_working_memory_metadata(&mut metadata, working_memory)?;
        self.store
            .update_thread(UpdateThreadRequest {
                thread_id,
                resource_id: None,
                title: None,
                metadata: Some(metadata),
            })
            .await?;
        Ok(())
    }

    async fn persist_observation_on_thread(
        &self,
        thread_id: Uuid,
        observation: &Observation,
    ) -> MemoryStoreResult<()> {
        let thread = self
            .store
            .get_thread(thread_id)
            .await?
            .ok_or(MemoryStoreError::ThreadNotFound(thread_id))?;
        let mut metadata = thread.metadata;
        let mut observations = observations_from_metadata(&metadata)?;
        let mut persisted = observation.clone();
        persisted.thread_id = thread_id;
        persisted.resource_id = Some(thread.resource_id);
        observations.push(persisted);
        set_observations_metadata(&mut metadata, &observations)?;
        self.store
            .update_thread(UpdateThreadRequest {
                thread_id,
                resource_id: None,
                title: None,
                metadata: Some(metadata),
            })
            .await?;
        Ok(())
    }

    async fn replace_observations_on_thread(
        &self,
        thread_id: Uuid,
        observations: &[Observation],
    ) -> MemoryStoreResult<()> {
        let thread = self
            .store
            .get_thread(thread_id)
            .await?
            .ok_or(MemoryStoreError::ThreadNotFound(thread_id))?;
        let mut metadata = thread.metadata;
        set_observations_metadata(&mut metadata, observations)?;
        self.store
            .update_thread(UpdateThreadRequest {
                thread_id,
                resource_id: None,
                title: None,
                metadata: Some(metadata),
            })
            .await?;
        Ok(())
    }

    async fn resource_scoped_working_memory(
        &self,
        resource_id: &str,
    ) -> MemoryStoreResult<Option<WorkingMemory>> {
        let threads = self
            .store
            .list_threads(ListThreadsQuery {
                resource_id: Some(resource_id.to_owned()),
                pagination: unbounded_pagination(),
            })
            .await?;
        Ok(threads
            .items
            .into_iter()
            .filter_map(|thread| working_memory_from_thread(&thread))
            .find(|working_memory| working_memory.scope == MemoryScope::Resource))
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

        if working_memory_from_thread(&thread).is_none() {
            if let Some(working_memory) = self
                .resource_scoped_working_memory(&thread.resource_id)
                .await
                .map_err(map_store_error)?
            {
                self.persist_working_memory_on_thread(thread.id, &working_memory)
                    .await
                    .map_err(map_store_error)?;
                let thread = self
                    .store
                    .get_thread(thread.id)
                    .await
                    .map_err(map_store_error)?
                    .ok_or_else(|| {
                        MastraError::not_found(format!("thread '{}' was not found", thread.id))
                    })?;
                return Ok(thread_to_core(thread));
            }
        }

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

    async fn get_working_memory(
        &self,
        thread_id: &str,
        _resource_id: Option<&str>,
    ) -> mastra_core::Result<Option<CoreWorkingMemoryState>> {
        let thread_id = parse_uuid(thread_id, "thread id")?;
        let working_memory = self.working_memory(thread_id).await.map_err(map_store_error)?;

        Ok(working_memory.map(working_memory_to_core))
    }

    async fn update_working_memory(
        &self,
        request: CoreUpdateWorkingMemoryRequest,
    ) -> mastra_core::Result<CoreWorkingMemoryState> {
        let thread_id = parse_uuid(&request.thread_id, "thread id")?;
        let working_memory = self
            .update_working_memory_with_format(
                UpdateWorkingMemoryRequest {
                    thread_id,
                    resource_id: request.resource_id,
                    scope: request.scope,
                    format: request.format,
                    template: request.template,
                    content: request.content,
                },
                request.format,
            )
            .await
            .map_err(map_store_error)?;

        Ok(working_memory_to_core(working_memory))
    }

    async fn list_observations(
        &self,
        request: CoreObservationQuery,
    ) -> mastra_core::Result<CoreObservationPage> {
        let thread_id = parse_uuid(&request.thread_id, "thread id")?;
        let observations = self
            .observations(ListObservationsQuery {
                thread_id,
                resource_id: request.resource_id,
                page: request.page,
                per_page: request.per_page,
            })
            .await
            .map_err(map_store_error)?;
        let page = request.page.unwrap_or(0);
        let per_page = request
            .per_page
            .unwrap_or_else(|| observations.len().max(1));
        if per_page == 0 {
            return Err(MastraError::validation(
                "per_page must be greater than zero",
            ));
        }
        let total = observations.len();
        let start = page.saturating_mul(per_page);
        let page_observations = observations
            .into_iter()
            .skip(start)
            .take(per_page)
            .collect::<Vec<_>>();
        let has_more = start.saturating_add(page_observations.len()) < total;

        Ok(CoreObservationPage {
            observations: page_observations
                .into_iter()
                .map(observation_to_core)
                .collect(),
            total,
            page,
            per_page,
            has_more,
        })
    }

    async fn append_observation(
        &self,
        request: CoreAppendObservationRequest,
    ) -> mastra_core::Result<CoreObservationRecord> {
        let thread_id = parse_uuid(&request.thread_id, "thread id")?;
        let observation = self
            .append_observation(AppendObservationRequest {
                thread_id,
                resource_id: request.resource_id,
                scope: request.scope,
                content: request.content,
                observed_message_ids: request
                    .observed_message_ids
                    .iter()
                    .map(|message_id| parse_uuid(message_id, "message id"))
                    .collect::<mastra_core::Result<Vec<_>>>()?,
                metadata: request.metadata,
            })
            .await
            .map_err(map_store_error)?;

        Ok(observation_to_core(observation))
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

        let source_thread = self
            .store
            .get_thread(source_thread_id)
            .await
            .map_err(map_store_error)?
            .ok_or_else(|| {
                MastraError::not_found(format!("thread '{source_thread_id}' was not found"))
            })?;
        let source_messages = self
            .store
            .list_messages(ListMessagesQuery {
                thread_id: source_thread_id,
                pagination: unbounded_pagination(),
            })
            .await
            .map_err(map_store_error)?
            .items;
        let filtered_source_messages = filter_cloned_messages(source_messages, &request)?;
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

        if let Some(working_memory) = working_memory_from_thread(&source_thread) {
            self.persist_working_memory_on_thread(thread.id, &working_memory)
                .await
                .map_err(map_store_error)?;
        }

        let source_observations = observations_from_thread(&source_thread);
        if !source_observations.is_empty() {
            let cloned_messages = self
                .store
                .list_messages(ListMessagesQuery {
                    thread_id: thread.id,
                    pagination: unbounded_pagination(),
                })
                .await
                .map_err(map_store_error)?
                .items;
            let message_id_map = filtered_source_messages
                .iter()
                .zip(cloned_messages.iter())
                .map(|(source, cloned)| (source.id, cloned.id))
                .collect::<HashMap<_, _>>();
            let remapped_observations = source_observations
                .into_iter()
                .map(|observation| Observation {
                    id: Uuid::new_v4(),
                    thread_id: thread.id,
                    resource_id: Some(thread.resource_id.clone()),
                    scope: observation.scope,
                    content: observation.content,
                    observed_message_ids: observation
                        .observed_message_ids
                        .iter()
                        .filter_map(|message_id| message_id_map.get(message_id).copied())
                        .collect::<Vec<_>>(),
                    metadata: observation.metadata,
                    created_at: observation.created_at,
                    updated_at: Utc::now(),
                })
                .collect::<Vec<_>>();
            self.replace_observations_on_thread(thread.id, &remapped_observations)
                .await
                .map_err(map_store_error)?;
        }

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

fn working_memory_to_core(working_memory: WorkingMemory) -> CoreWorkingMemoryState {
    CoreWorkingMemoryState {
        thread_id: working_memory.thread_id.to_string(),
        resource_id: working_memory.resource_id,
        scope: working_memory.scope,
        format: working_memory.format,
        template: working_memory.template,
        content: working_memory.content,
        updated_at: working_memory.updated_at,
    }
}

fn observation_to_core(observation: Observation) -> CoreObservationRecord {
    CoreObservationRecord {
        id: observation.id.to_string(),
        thread_id: observation.thread_id.to_string(),
        resource_id: observation.resource_id,
        scope: observation.scope,
        content: observation.content,
        observed_message_ids: observation
            .observed_message_ids
            .into_iter()
            .map(|message_id| message_id.to_string())
            .collect(),
        metadata: observation.metadata,
        created_at: observation.created_at,
        updated_at: observation.updated_at,
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

fn set_working_memory_metadata(
    metadata: &mut Value,
    working_memory: &WorkingMemory,
) -> MemoryStoreResult<()> {
    let object = ensure_object(metadata);
    object.insert(
        WORKING_MEMORY_METADATA_KEY.to_owned(),
        serde_json::to_value(working_memory)
            .map_err(|error| MemoryStoreError::Backend(error.to_string()))?,
    );
    Ok(())
}

fn set_observations_metadata(
    metadata: &mut Value,
    observations: &[Observation],
) -> MemoryStoreResult<()> {
    let object = ensure_object(metadata);
    object.insert(
        OBSERVATIONS_METADATA_KEY.to_owned(),
        serde_json::to_value(observations)
            .map_err(|error| MemoryStoreError::Backend(error.to_string()))?,
    );
    Ok(())
}

fn observations_from_thread(thread: &Thread) -> Vec<Observation> {
    observations_from_metadata(&thread.metadata)
        .unwrap_or_default()
        .into_iter()
        .map(|mut observation| {
            observation.thread_id = thread.id;
            observation.resource_id = Some(thread.resource_id.clone());
            observation
        })
        .collect()
}

fn observations_from_metadata(metadata: &Value) -> MemoryStoreResult<Vec<Observation>> {
    let Some(value) = metadata
        .as_object()
        .and_then(|object| object.get(OBSERVATIONS_METADATA_KEY))
    else {
        return Ok(Vec::new());
    };
    serde_json::from_value(value.clone())
        .map_err(|error| MemoryStoreError::Backend(error.to_string()))
}

fn working_memory_from_thread(thread: &Thread) -> Option<WorkingMemory> {
    working_memory_from_metadata(&thread.metadata).map(|mut working_memory| {
        working_memory.thread_id = thread.id;
        working_memory.resource_id = Some(thread.resource_id.clone());
        working_memory
    })
}

fn working_memory_from_metadata(metadata: &Value) -> Option<WorkingMemory> {
    metadata
        .as_object()
        .and_then(|object| object.get(WORKING_MEMORY_METADATA_KEY))
        .and_then(|value| serde_json::from_value(value.clone()).ok())
}

fn ensure_object(metadata: &mut Value) -> &mut Map<String, Value> {
    if !metadata.is_object() {
        *metadata = Value::Object(Map::new());
    }

    metadata
        .as_object_mut()
        .expect("metadata should be an object after initialization")
}

fn filter_cloned_messages(
    messages: Vec<Message>,
    request: &CoreCloneThreadRequest,
) -> mastra_core::Result<Vec<Message>> {
    let mut filtered = messages;

    if let Some(message_ids) = request.message_ids.as_ref() {
        let allowed = parse_uuid_list(message_ids, "message id")?
            .into_iter()
            .collect::<HashSet<_>>();
        filtered.retain(|message| allowed.contains(&message.id));
    }

    if let Some(start_date) = request.start_date {
        filtered.retain(|message| message.created_at >= start_date);
    }

    if let Some(end_date) = request.end_date {
        filtered.retain(|message| message.created_at <= end_date);
    }

    if let Some(limit) = request.message_limit {
        let start = filtered.len().saturating_sub(limit);
        filtered = filtered.into_iter().skip(start).collect();
    }

    Ok(filtered)
}

fn filter_cloned_messages_facade(
    messages: Vec<Message>,
    request: &CloneThreadRequest,
) -> Vec<Message> {
    let mut filtered = messages;

    if let Some(message_ids) = request.message_ids.as_ref() {
        let allowed = message_ids.iter().copied().collect::<HashSet<_>>();
        filtered.retain(|message| allowed.contains(&message.id));
    }

    if let Some(start_date) = request.start_date {
        filtered.retain(|message| message.created_at >= start_date);
    }

    if let Some(end_date) = request.end_date {
        filtered.retain(|message| message.created_at <= end_date);
    }

    if let Some(limit) = request.message_limit {
        let start = filtered.len().saturating_sub(limit);
        filtered = filtered.into_iter().skip(start).collect();
    }

    filtered
}

#[cfg(test)]
mod tests;
