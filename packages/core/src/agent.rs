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
    model::{
        FinishReason, LanguageModel, ModelEvent, ModelRequest, ModelResponse, ModelToolCall,
        ModelToolResult, UsageStats,
    },
    request_context::RequestContext,
    tool::{Tool, ToolExecutionContext},
};

const DEFAULT_AGENT_MAX_STEPS: u32 = 8;

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
    pub run_id: Option<String>,
    pub max_steps: Option<u32>,
    pub request_context: RequestContext,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AgentResponse {
    pub id: String,
    pub text: String,
    pub data: Value,
    pub run_id: String,
    pub finish_reason: FinishReason,
    pub usage: Option<UsageStats>,
    pub thread_id: Option<String>,
    pub tool_names: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct AgentStreamRequest {
    pub prompt: String,
    pub thread_id: Option<String>,
    pub resource_id: Option<String>,
    pub run_id: Option<String>,
    pub max_steps: Option<u32>,
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
        let AgentGenerateRequest {
            prompt,
            thread_id,
            resource_id,
            run_id,
            max_steps,
            request_context,
        } = request;
        let (thread_id, memory_context) =
            self.prepare_memory(&prompt, thread_id, resource_id).await?;
        let run_id = run_id.unwrap_or_else(|| Uuid::now_v7().to_string());
        let max_steps = max_steps.unwrap_or(DEFAULT_AGENT_MAX_STEPS).max(1);
        let tool_names = self.tool_names();
        let mut tool_results = Vec::new();

        for step in 0..max_steps {
            let response = self
                .model
                .generate(ModelRequest {
                    prompt: prompt.clone(),
                    instructions: self.instructions.clone(),
                    memory: memory_context.clone(),
                    tool_names: tool_names.clone(),
                    tool_results: tool_results.clone(),
                    run_id: Some(run_id.clone()),
                    thread_id: thread_id.clone(),
                    max_steps: Some(max_steps),
                    request_context: request_context.clone(),
                })
                .await?;
            let finish_reason = response.normalized_finish_reason();
            let tool_calls = response.normalized_tool_calls();

            if tool_calls.is_empty() {
                if finish_reason == FinishReason::ToolCall {
                    return Err(MastraError::tool(format!(
                        "agent '{}' received tool_call finish reason without tool payload",
                        self.id
                    )));
                }
                self.persist_response(&thread_id, &prompt, &response)
                    .await?;
                return Ok(self.to_agent_response(response, run_id, thread_id, tool_names));
            }

            if step + 1 >= max_steps {
                return Err(MastraError::tool(format!(
                    "agent '{}' exhausted max_steps ({max_steps}) before finishing tool loop",
                    self.id
                )));
            }

            let mut round_results = self
                .execute_tool_calls(&tool_calls, &request_context, &run_id, &thread_id)
                .await?;
            tool_results.append(&mut round_results);
        }

        Err(MastraError::tool(format!(
            "agent '{}' failed to complete within {max_steps} steps",
            self.id
        )))
    }

    pub fn stream(
        &self,
        request: AgentStreamRequest,
    ) -> futures::stream::BoxStream<'static, Result<AgentStreamResponse>> {
        if !self.tools.is_empty() {
            let agent = self.clone();
            return stream::once(async move {
                let response = agent
                    .generate(AgentGenerateRequest {
                        prompt: request.prompt,
                        thread_id: request.thread_id,
                        resource_id: request.resource_id,
                        run_id: request.run_id,
                        max_steps: request.max_steps,
                        request_context: request.request_context,
                    })
                    .await?;

                Ok(AgentStreamResponse {
                    id: agent.id.clone(),
                    event: ModelEvent::Done(ModelResponse {
                        text: response.text,
                        data: response.data,
                        finish_reason: response.finish_reason,
                        usage: response.usage,
                        tool_calls: Vec::new(),
                    }),
                    thread_id: response.thread_id,
                })
            })
            .boxed();
        }

        let agent = self.clone();
        stream::once(async move {
            let run_id = request.run_id.unwrap_or_else(|| Uuid::now_v7().to_string());
            let max_steps = request.max_steps.unwrap_or(DEFAULT_AGENT_MAX_STEPS).max(1);
            let (thread_id, memory_context) = agent
                .prepare_memory(&request.prompt, request.thread_id, request.resource_id)
                .await?;
            let prompt = request.prompt;
            let stream = agent.model.stream(ModelRequest {
                prompt: prompt.clone(),
                instructions: agent.instructions.clone(),
                memory: memory_context,
                tool_names: agent.tool_names(),
                tool_results: Vec::new(),
                run_id: Some(run_id),
                thread_id: thread_id.clone(),
                max_steps: Some(max_steps),
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
            return Ok((thread_id, Vec::new()));
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

    async fn execute_tool_calls(
        &self,
        tool_calls: &[ModelToolCall],
        request_context: &RequestContext,
        run_id: &str,
        thread_id: &Option<String>,
    ) -> Result<Vec<ModelToolResult>> {
        let mut results = Vec::with_capacity(tool_calls.len());

        for call in tool_calls {
            let tool = self
                .tools
                .iter()
                .find(|tool| tool.id() == call.name)
                .ok_or_else(|| {
                    MastraError::tool(format!(
                        "agent '{}' received unknown tool call '{}'",
                        self.id, call.name
                    ))
                })?;
            let output = tool
                .execute(
                    call.input.clone(),
                    ToolExecutionContext {
                        request_context: request_context.clone(),
                        run_id: Some(run_id.to_owned()),
                        thread_id: thread_id.clone(),
                        approved: false,
                    },
                )
                .await?;
            results.push(ModelToolResult {
                id: call.id.clone(),
                name: call.name.clone(),
                output,
            });
        }

        Ok(results)
    }

    fn to_agent_response(
        &self,
        response: ModelResponse,
        run_id: String,
        thread_id: Option<String>,
        tool_names: Vec<String>,
    ) -> AgentResponse {
        let finish_reason = response.normalized_finish_reason();
        let usage = response.normalized_usage();
        AgentResponse {
            id: self.id.clone(),
            text: response.text,
            data: response.data,
            run_id,
            finish_reason,
            usage,
            thread_id,
            tool_names,
        }
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
    use serde_json::{Value, json};

    use crate::{
        memory::{
            CreateThreadRequest, MemoryConfig, MemoryEngine, MemoryMessage, MemoryRecallRequest,
            MemoryRole, Thread,
        },
        model::{
            FinishReason, ModelEvent, ModelResponse, ModelToolCall, ModelToolResult, StaticModel,
            UsageStats,
        },
        request_context::RequestContext,
        tool::Tool,
    };

    use super::{Agent, AgentConfig, AgentGenerateRequest, AgentStreamRequest};

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
                run_id: None,
                max_steps: None,
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

    #[tokio::test]
    async fn generate_executes_tool_calls_until_model_returns_final_response() {
        let seen_tool_contexts = Arc::new(RwLock::new(Vec::new()));
        let model_requests = Arc::new(RwLock::new(Vec::new()));

        let recording_requests = Arc::clone(&model_requests);
        let model = StaticModel::new(move |request| {
            let recording_requests = Arc::clone(&recording_requests);
            async move {
                let step = {
                    let mut requests = recording_requests.write();
                    let step = requests.len();
                    requests.push(request.clone());
                    step
                };

                match step {
                    0 => Ok(ModelResponse {
                        text: String::new(),
                        data: Value::Null,
                        finish_reason: FinishReason::ToolCall,
                        usage: Some(UsageStats {
                            prompt_tokens: 3,
                            completion_tokens: 1,
                        }),
                        tool_calls: vec![ModelToolCall {
                            id: "call-1".into(),
                            name: "sum".into(),
                            input: json!({ "a": 2, "b": 3 }),
                        }],
                    }),
                    1 => {
                        assert_eq!(
                            request.tool_results,
                            vec![ModelToolResult {
                                id: "call-1".into(),
                                name: "sum".into(),
                                output: json!(5),
                            }]
                        );
                        Ok(ModelResponse {
                            text: "5".into(),
                            data: json!({ "source": "tool-loop" }),
                            finish_reason: FinishReason::Stop,
                            usage: Some(UsageStats {
                                prompt_tokens: 5,
                                completion_tokens: 2,
                            }),
                            tool_calls: Vec::new(),
                        })
                    }
                    other => panic!("unexpected model step {other}"),
                }
            }
        });

        let tool_contexts = Arc::clone(&seen_tool_contexts);
        let sum_tool = Tool::new("sum", "add numbers", move |input, context| {
            let tool_contexts = Arc::clone(&tool_contexts);
            async move {
                tool_contexts.write().push(context);
                let a = input.get("a").and_then(Value::as_i64).unwrap_or_default();
                let b = input.get("b").and_then(Value::as_i64).unwrap_or_default();
                Ok(json!(a + b))
            }
        });

        let mut request_context = RequestContext::new();
        request_context.insert("trace_id", "trace-123");

        let agent = Agent::new(AgentConfig {
            id: "tool-loop".into(),
            name: "Tool Loop".into(),
            instructions: "Use tools when helpful".into(),
            description: None,
            model: Arc::new(model),
            tools: vec![sum_tool],
            memory: None,
            memory_config: MemoryConfig::default(),
        });

        let response = agent
            .generate(AgentGenerateRequest {
                prompt: "2 + 3 = ?".into(),
                thread_id: Some("thread-123".into()),
                resource_id: None,
                run_id: Some("run-123".into()),
                max_steps: Some(4),
                request_context: request_context.clone(),
            })
            .await
            .expect("agent should resolve tool loop");

        assert_eq!(response.text, "5");
        assert_eq!(response.data, json!({ "source": "tool-loop" }));
        assert_eq!(response.run_id, "run-123");
        assert_eq!(response.finish_reason, FinishReason::Stop);
        assert_eq!(
            response.usage,
            Some(UsageStats {
                prompt_tokens: 5,
                completion_tokens: 2,
            })
        );

        let seen_tool_contexts = seen_tool_contexts.read();
        assert_eq!(seen_tool_contexts.len(), 1);
        assert_eq!(seen_tool_contexts[0].run_id.as_deref(), Some("run-123"));
        assert_eq!(
            seen_tool_contexts[0].thread_id.as_deref(),
            Some("thread-123")
        );
        assert_eq!(
            seen_tool_contexts[0].request_context.get("trace_id"),
            Some(&json!("trace-123"))
        );

        let model_requests = model_requests.read();
        assert_eq!(model_requests.len(), 2);
        assert!(model_requests[0].tool_results.is_empty());
        assert_eq!(model_requests[0].run_id.as_deref(), Some("run-123"));
        assert_eq!(model_requests[0].thread_id.as_deref(), Some("thread-123"));
    }
}
