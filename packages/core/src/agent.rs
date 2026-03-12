use std::sync::Arc;

use futures::{StreamExt, stream};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::{
    error::{MastraError, Result},
    memory::{
        CreateThreadRequest, MemoryConfig, MemoryEngine, MemoryMessage, MemoryRecallRequest,
        MemoryRole,
    },
    model::{LanguageModel, ModelEvent, ModelRequest, ModelResponse},
    request_context::RequestContext,
    tool::Tool,
};

#[derive(Clone)]
pub struct AgentConfig {
    pub id: String,
    pub name: String,
    pub instructions: String,
    pub description: Option<String>,
    pub model: Arc<dyn LanguageModel>,
    pub tools: Vec<Tool>,
    pub memory: Option<Arc<dyn MemoryEngine>>,
    pub memory_config: MemoryConfig,
}

#[derive(Clone)]
pub struct Agent {
    id: String,
    name: String,
    instructions: String,
    description: Option<String>,
    model: Arc<dyn LanguageModel>,
    tools: Vec<Tool>,
    memory: Option<Arc<dyn MemoryEngine>>,
    memory_config: MemoryConfig,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct AgentGenerateRequest {
    pub prompt: String,
    pub thread_id: Option<String>,
    pub resource_id: Option<String>,
    pub request_context: RequestContext,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AgentResponse {
    pub id: String,
    pub text: String,
    pub data: Value,
    pub thread_id: Option<String>,
    pub tool_names: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct AgentStreamRequest {
    pub prompt: String,
    pub thread_id: Option<String>,
    pub resource_id: Option<String>,
    pub request_context: RequestContext,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AgentStreamResponse {
    pub id: String,
    pub event: ModelEvent,
    pub thread_id: Option<String>,
}

impl Agent {
    pub fn new(config: AgentConfig) -> Self {
        Self {
            id: config.id,
            name: config.name,
            instructions: config.instructions,
            description: config.description,
            model: config.model,
            tools: config.tools,
            memory: config.memory,
            memory_config: config.memory_config,
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn instructions(&self) -> &str {
        &self.instructions
    }

    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    pub fn tool_names(&self) -> Vec<String> {
        self.tools
            .iter()
            .map(|tool| tool.id().to_string())
            .collect()
    }

    pub async fn generate(&self, request: AgentGenerateRequest) -> Result<AgentResponse> {
        let (thread_id, memory_context) = self
            .prepare_memory(&request.prompt, request.thread_id, request.resource_id)
            .await?;
        let response = self
            .model
            .generate(ModelRequest {
                prompt: request.prompt.clone(),
                instructions: self.instructions.clone(),
                memory: memory_context,
                tool_names: self.tool_names(),
                request_context: request.request_context,
            })
            .await?;

        self.persist_response(&thread_id, &request.prompt, &response)
            .await?;

        Ok(AgentResponse {
            id: self.id.clone(),
            text: response.text,
            data: response.data,
            thread_id,
            tool_names: self.tool_names(),
        })
    }

    pub fn stream(
        &self,
        request: AgentStreamRequest,
    ) -> futures::stream::BoxStream<'static, Result<AgentStreamResponse>> {
        let agent = self.clone();
        stream::once(async move {
            let (thread_id, memory_context) = agent
                .prepare_memory(&request.prompt, request.thread_id, request.resource_id)
                .await?;
            let prompt = request.prompt;
            let stream = agent.model.stream(ModelRequest {
                prompt: prompt.clone(),
                instructions: agent.instructions.clone(),
                memory: memory_context,
                tool_names: agent.tool_names(),
                request_context: request.request_context,
            });

            Ok::<_, MastraError>((agent, prompt, thread_id, stream))
        })
        .flat_map(|result| match result {
            Ok((agent, prompt, thread_id, stream)) => {
                let agent_id = agent.id.clone();
                stream
                    .then(move |event| {
                        let agent = agent.clone();
                        let prompt = prompt.clone();
                        let thread_id = thread_id.clone();
                        let agent_id = agent_id.clone();
                        async move {
                            let event = event?;
                            if let ModelEvent::Done(response) = &event {
                                agent
                                    .persist_response(&thread_id, &prompt, response)
                                    .await?;
                            }

                            Ok(AgentStreamResponse {
                                id: agent_id,
                                event,
                                thread_id,
                            })
                        }
                    })
                    .boxed()
            }
            Err(error) => stream::once(async { Err(error) }).boxed(),
        })
        .boxed()
    }

    async fn prepare_memory(
        &self,
        prompt: &str,
        thread_id: Option<String>,
        resource_id: Option<String>,
    ) -> Result<(Option<String>, Vec<String>)> {
        let Some(memory) = &self.memory else {
            return Ok((None, Vec::new()));
        };

        let thread_id = match thread_id {
            Some(thread_id) => thread_id,
            None => {
                let thread = memory
                    .create_thread(CreateThreadRequest {
                        id: Some(Uuid::now_v7().to_string()),
                        resource_id,
                        title: Some(prompt.chars().take(32).collect()),
                        metadata: Value::Null,
                    })
                    .await?;
                thread.id
            }
        };

        let history = memory
            .list_messages(MemoryRecallRequest {
                thread_id: thread_id.clone(),
                limit: self.memory_config.last_messages,
            })
            .await?
            .into_iter()
            .map(|message| format!("{:?}: {}", message.role, message.content))
            .collect();

        Ok((Some(thread_id), history))
    }

    async fn persist_response(
        &self,
        thread_id: &Option<String>,
        prompt: &str,
        response: &ModelResponse,
    ) -> Result<()> {
        let Some(memory) = &self.memory else {
            return Ok(());
        };

        if self.memory_config.read_only {
            return Ok(());
        }

        let Some(thread_id) = thread_id else {
            return Ok(());
        };

        // Persist both sides of the exchange together so recall order remains stable.
        memory
            .append_messages(
                thread_id,
                vec![
                    MemoryMessage {
                        id: Uuid::now_v7().to_string(),
                        thread_id: thread_id.clone(),
                        role: MemoryRole::User,
                        content: prompt.to_string(),
                        created_at: chrono::Utc::now(),
                        metadata: Value::Null,
                    },
                    MemoryMessage {
                        id: Uuid::now_v7().to_string(),
                        thread_id: thread_id.clone(),
                        role: MemoryRole::Assistant,
                        content: response.text.clone(),
                        created_at: chrono::Utc::now(),
                        metadata: response.data.clone(),
                    },
                ],
            )
            .await
    }

    pub fn snapshot(&self) -> Value {
        json!({
          "id": self.id,
          "name": self.name,
          "description": self.description,
          "instructions": self.instructions,
          "tools": self.tool_names(),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, sync::Arc};

    use async_trait::async_trait;
    use chrono::Utc;
    use futures::StreamExt;
    use parking_lot::RwLock;
    use serde_json::Value;

    use crate::{
        memory::{
            CreateThreadRequest, MemoryConfig, MemoryEngine, MemoryMessage, MemoryRecallRequest,
            MemoryRole, Thread,
        },
        model::{ModelEvent, StaticModel},
        request_context::RequestContext,
    };

    use super::{Agent, AgentConfig, AgentStreamRequest};

    #[derive(Default)]
    struct RecordingMemory {
        threads: RwLock<HashMap<String, Thread>>,
        messages: RwLock<HashMap<String, Vec<MemoryMessage>>>,
    }

    #[async_trait]
    impl MemoryEngine for RecordingMemory {
        async fn create_thread(&self, request: CreateThreadRequest) -> crate::Result<Thread> {
            let thread = Thread {
                id: request
                    .id
                    .unwrap_or_else(|| uuid::Uuid::now_v7().to_string()),
                resource_id: request.resource_id,
                title: request.title,
                created_at: Utc::now(),
                metadata: request.metadata,
            };
            self.threads
                .write()
                .insert(thread.id.clone(), thread.clone());
            self.messages.write().entry(thread.id.clone()).or_default();
            Ok(thread)
        }

        async fn get_thread(&self, thread_id: &str) -> crate::Result<Option<Thread>> {
            Ok(self.threads.read().get(thread_id).cloned())
        }

        async fn list_threads(&self, _resource_id: Option<&str>) -> crate::Result<Vec<Thread>> {
            Ok(self.threads.read().values().cloned().collect())
        }

        async fn append_messages(
            &self,
            thread_id: &str,
            messages: Vec<MemoryMessage>,
        ) -> crate::Result<()> {
            self.messages
                .write()
                .entry(thread_id.to_string())
                .or_default()
                .extend(messages);
            Ok(())
        }

        async fn list_messages(
            &self,
            request: MemoryRecallRequest,
        ) -> crate::Result<Vec<MemoryMessage>> {
            Ok(self
                .messages
                .read()
                .get(&request.thread_id)
                .cloned()
                .unwrap_or_default())
        }
    }

    #[tokio::test]
    async fn stream_persists_exchange_when_model_finishes() {
        let memory = Arc::new(RecordingMemory::default());
        let agent = Agent::new(AgentConfig {
            id: "streamer".into(),
            name: "Streamer".into(),
            instructions: "Echo".into(),
            description: None,
            model: Arc::new(StaticModel::echo()),
            tools: Vec::new(),
            memory: Some(memory.clone()),
            memory_config: MemoryConfig::default(),
        });

        let events = agent
            .stream(AgentStreamRequest {
                prompt: "persist me".into(),
                thread_id: None,
                resource_id: Some("resource-1".into()),
                request_context: RequestContext::new(),
            })
            .collect::<Vec<_>>()
            .await;

        assert_eq!(events.len(), 1);
        let event = events
            .into_iter()
            .next()
            .expect("stream event")
            .expect("stream ok");
        match &event.event {
            ModelEvent::Done(response) => {
                assert_eq!(response.text, "persist me");
            }
            other => panic!("expected final response event, got {other:?}"),
        }

        let thread_id = event.thread_id.expect("thread id should exist");
        let persisted = memory
            .list_messages(MemoryRecallRequest {
                thread_id,
                limit: None,
            })
            .await
            .expect("messages should be persisted");

        assert_eq!(persisted.len(), 2);
        assert_eq!(persisted[0].role, MemoryRole::User);
        assert_eq!(persisted[0].content, "persist me");
        assert_eq!(persisted[1].role, MemoryRole::Assistant);
        assert_eq!(persisted[1].content, "persist me");
        assert_eq!(persisted[1].metadata, Value::Null);
    }
}
