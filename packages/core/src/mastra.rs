use std::sync::Arc;

use indexmap::IndexMap;
use serde_json::{Value, json};

use crate::{
    agent::{Agent, AgentGenerateRequest, AgentResponse},
    error::{MastraError, Result},
    memory::MemoryEngine,
    request_context::RequestContext,
    tool::Tool,
    workflow::{Workflow, WorkflowRunResult},
};

#[derive(Default)]
pub struct MastraBuilder {
    agents: IndexMap<String, Agent>,
    tools: IndexMap<String, Tool>,
    workflows: IndexMap<String, Workflow>,
    memory: IndexMap<String, Arc<dyn MemoryEngine>>,
}

#[derive(Clone, Default)]
pub struct Mastra {
    agents: IndexMap<String, Agent>,
    tools: IndexMap<String, Tool>,
    workflows: IndexMap<String, Workflow>,
    memory: IndexMap<String, Arc<dyn MemoryEngine>>,
}

impl MastraBuilder {
    pub fn with_agent(mut self, agent: Agent) -> Self {
        self.agents.insert(agent.id().to_string(), agent);
        self
    }

    pub fn with_tool(mut self, tool: Tool) -> Self {
        self.tools.insert(tool.id().to_string(), tool);
        self
    }

    pub fn with_workflow(mut self, workflow: Workflow) -> Self {
        self.workflows.insert(workflow.id().to_string(), workflow);
        self
    }

    pub fn with_memory(mut self, id: impl Into<String>, memory: Arc<dyn MemoryEngine>) -> Self {
        self.memory.insert(id.into(), memory);
        self
    }

    pub fn build(self) -> Mastra {
        Mastra {
            agents: self.agents,
            tools: self.tools,
            workflows: self.workflows,
            memory: self.memory,
        }
    }
}

impl Mastra {
    pub fn builder() -> MastraBuilder {
        MastraBuilder::default()
    }

    pub fn register_agent(&mut self, agent: Agent) -> &mut Self {
        self.agents.insert(agent.id().to_string(), agent);
        self
    }

    pub fn register_tool(&mut self, tool: Tool) -> &mut Self {
        self.tools.insert(tool.id().to_string(), tool);
        self
    }

    pub fn register_workflow(&mut self, workflow: Workflow) -> &mut Self {
        self.workflows.insert(workflow.id().to_string(), workflow);
        self
    }

    pub fn register_memory(
        &mut self,
        id: impl Into<String>,
        memory: Arc<dyn MemoryEngine>,
    ) -> &mut Self {
        self.memory.insert(id.into(), memory);
        self
    }

    pub fn list_agents(&self) -> Vec<String> {
        self.agents.keys().cloned().collect()
    }

    pub fn list_tools(&self) -> Vec<String> {
        self.tools.keys().cloned().collect()
    }

    pub fn list_workflows(&self) -> Vec<String> {
        self.workflows.keys().cloned().collect()
    }

    pub fn list_memory(&self) -> Vec<String> {
        self.memory.keys().cloned().collect()
    }

    pub fn get_agent(&self, id: &str) -> Option<&Agent> {
        self.agents.get(id)
    }

    pub fn get_workflow(&self, id: &str) -> Option<&Workflow> {
        self.workflows.get(id)
    }

    pub fn get_tool(&self, id: &str) -> Option<&Tool> {
        self.tools.get(id)
    }

    pub fn get_memory(&self, id: &str) -> Option<Arc<dyn MemoryEngine>> {
        self.memory.get(id).cloned()
    }

    pub fn snapshot(&self) -> Value {
        json!({
          "agents": self.list_agents(),
          "tools": self.list_tools(),
          "workflows": self.list_workflows(),
          "memory": self.list_memory(),
        })
    }

    pub async fn generate_with_request(
        &self,
        agent_id: &str,
        request: AgentGenerateRequest,
    ) -> Result<AgentResponse> {
        let agent = self.get_agent(agent_id).ok_or_else(|| {
            MastraError::not_found(format!("agent '{}' was not registered", agent_id))
        })?;

        agent.generate(request).await
    }

    pub async fn generate(
        &self,
        agent_id: &str,
        prompt: impl Into<String>,
    ) -> Result<AgentResponse> {
        self.generate_with_request(
            agent_id,
            AgentGenerateRequest {
                prompt: prompt.into(),
                thread_id: None,
                resource_id: None,
                run_id: None,
                max_steps: None,
                request_context: RequestContext::new(),
                ..Default::default()
            },
        )
        .await
    }

    pub async fn run_workflow(
        &self,
        workflow_id: &str,
        input: Value,
        request_context: RequestContext,
    ) -> Result<WorkflowRunResult> {
        let workflow = self.get_workflow(workflow_id).ok_or_else(|| {
            MastraError::not_found(format!("workflow '{}' was not registered", workflow_id))
        })?;
        workflow.run(input, request_context).await
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, sync::Arc};

    use async_trait::async_trait;
    use chrono::Utc;
    use futures::{StreamExt, TryStreamExt, stream};
    use parking_lot::RwLock;
    use serde_json::{Value, json};

    use crate::{
        Mastra,
        agent::{Agent, AgentConfig},
        memory::{
            CreateThreadRequest, MemoryConfig, MemoryEngine, MemoryMessage, MemoryRecallRequest,
            MemoryRole, Thread,
        },
        model::{LanguageModel, ModelEvent, ModelRequest, ModelResponse, StaticModel},
        request_context::RequestContext,
        tool::Tool,
        workflow::{Step, Workflow},
    };

    #[derive(Default)]
    struct TestMemory {
        threads: RwLock<HashMap<String, Thread>>,
        messages: RwLock<HashMap<String, Vec<MemoryMessage>>>,
    }

    #[derive(Clone)]
    struct StreamingModel;

    #[async_trait]
    impl MemoryEngine for TestMemory {
        async fn create_thread(&self, request: CreateThreadRequest) -> crate::Result<Thread> {
            let now = Utc::now();
            let thread = Thread {
                id: request
                    .id
                    .unwrap_or_else(|| uuid::Uuid::now_v7().to_string()),
                resource_id: request.resource_id,
                title: request.title,
                created_at: now,
                updated_at: now,
                metadata: request.metadata,
            };
            self.threads
                .write()
                .insert(thread.id.clone(), thread.clone());
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
            let mut messages = self
                .messages
                .read()
                .get(&request.thread_id)
                .cloned()
                .unwrap_or_default();
            if let Some(limit) = request.limit {
                let start = messages.len().saturating_sub(limit);
                messages = messages[start..].to_vec();
            }
            Ok(messages)
        }
    }

    #[async_trait]
    impl LanguageModel for StreamingModel {
        async fn generate(&self, _request: ModelRequest) -> crate::Result<ModelResponse> {
            Ok(ModelResponse {
                text: "stream-complete".into(),
                data: json!({ "source": "generate" }),
                finish_reason: crate::FinishReason::Stop,
                usage: None,
                tool_calls: Vec::new(),
            })
        }

        fn stream(
            &self,
            _request: ModelRequest,
        ) -> futures::stream::BoxStream<'static, crate::Result<ModelEvent>> {
            stream::iter(vec![
                Ok(ModelEvent::TextDelta("stream".into())),
                Ok(ModelEvent::TextDelta("-complete".into())),
                Ok(ModelEvent::Done(ModelResponse {
                    text: "stream-complete".into(),
                    data: json!({ "source": "stream" }),
                    finish_reason: crate::FinishReason::Stop,
                    usage: None,
                    tool_calls: Vec::new(),
                })),
            ])
            .boxed()
        }

        fn clone_box(&self) -> Box<dyn LanguageModel> {
            Box::new(self.clone())
        }
    }

    #[derive(Default)]
    struct StrictMemory {
        threads: RwLock<HashMap<String, Thread>>,
        messages: RwLock<HashMap<String, Vec<MemoryMessage>>>,
    }

    #[async_trait]
    impl MemoryEngine for StrictMemory {
        async fn create_thread(&self, request: CreateThreadRequest) -> crate::Result<Thread> {
            let now = Utc::now();
            let thread = Thread {
                id: request
                    .id
                    .unwrap_or_else(|| uuid::Uuid::now_v7().to_string()),
                resource_id: request.resource_id,
                title: request.title,
                created_at: now,
                updated_at: now,
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
            let mut store = self.messages.write();
            let entry = store.get_mut(thread_id).ok_or_else(|| {
                crate::MastraError::not_found(format!("thread '{}' missing", thread_id))
            })?;
            entry.extend(messages);
            Ok(())
        }

        async fn list_messages(
            &self,
            request: MemoryRecallRequest,
        ) -> crate::Result<Vec<MemoryMessage>> {
            self.messages
                .read()
                .get(&request.thread_id)
                .cloned()
                .ok_or_else(|| {
                    crate::MastraError::not_found(format!("thread '{}' missing", request.thread_id))
                })
        }
    }

    #[tokio::test]
    async fn tool_and_workflow_round_trip() {
        let tool = Tool::new("sum", "add two numbers", |input, _| async move {
            let left = input["left"].as_i64().unwrap_or_default();
            let right = input["right"].as_i64().unwrap_or_default();
            Ok(json!({ "value": left + right }))
        });

        let workflow = Workflow::new("calc").then(Step::from_tool(tool.clone()));
        let mastra = Mastra::builder()
            .with_tool(tool)
            .with_workflow(workflow)
            .build();

        let result = mastra
            .run_workflow(
                "calc",
                json!({ "left": 2, "right": 3 }),
                RequestContext::new(),
            )
            .await
            .expect("workflow should succeed");

        assert_eq!(result.output["value"], 5);
    }

    #[tokio::test]
    async fn agent_generate_uses_memory() {
        let memory = Arc::new(TestMemory::default());
        let agent = Agent::new(AgentConfig {
            id: "weather".into(),
            name: "Weather".into(),
            instructions: "You answer weather questions.".into(),
            description: None,
            model: Arc::new(StaticModel::echo()),
            tools: Vec::new(),
            memory: Some(memory.clone()),
            memory_config: MemoryConfig::default(),
        });

        memory
            .append_messages(
                "thread-1",
                vec![MemoryMessage {
                    id: "msg-1".into(),
                    thread_id: "thread-1".into(),
                    role: MemoryRole::User,
                    content: "Yesterday was rainy".into(),
                    created_at: Utc::now(),
                    metadata: serde_json::Value::Null,
                }],
            )
            .await
            .expect("seed memory");

        let response = agent
            .generate(crate::AgentGenerateRequest {
                prompt: "How is today?".into(),
                thread_id: Some("thread-1".into()),
                resource_id: None,
                run_id: None,
                max_steps: None,
                request_context: RequestContext::new(),
                ..Default::default()
            })
            .await
            .expect("agent should respond");

        assert!(response.text.contains("Yesterday was rainy"));
        assert!(response.text.contains("How is today?"));
    }

    #[tokio::test]
    async fn agent_stream_persists_messages_after_done_event() {
        let memory = Arc::new(TestMemory::default());
        let agent = Agent::new(AgentConfig {
            id: "streamer".into(),
            name: "Streamer".into(),
            instructions: "Stream answers.".into(),
            description: Some("streaming test".into()),
            model: Arc::new(StreamingModel),
            tools: Vec::new(),
            memory: Some(memory.clone()),
            memory_config: MemoryConfig::default(),
        });

        let events = agent
            .stream(crate::AgentStreamRequest {
                prompt: "remember streamed output".into(),
                thread_id: Some("thread-stream".into()),
                resource_id: Some("resource-stream".into()),
                run_id: None,
                max_steps: None,
                request_context: RequestContext::new(),
                ..Default::default()
            })
            .try_collect::<Vec<_>>()
            .await
            .expect("stream should succeed");

        assert_eq!(events.len(), 3);

        let payload = memory
            .list_messages(MemoryRecallRequest {
                thread_id: "thread-stream".into(),
                limit: None,
                resource_id: None,
                page: None,
                per_page: None,
                message_ids: None,
                start_date: None,
                end_date: None,
                order_by: None,
            })
            .await
            .expect("stream should persist memory")
            .into_iter()
            .map(|message| (message.role, message.content, message.metadata))
            .collect::<Vec<_>>();

        assert_eq!(
            payload,
            vec![
                (
                    MemoryRole::User,
                    "remember streamed output".into(),
                    Value::Null
                ),
                (
                    MemoryRole::Assistant,
                    "stream-complete".into(),
                    json!({ "source": "stream" }),
                ),
            ]
        );
    }

    #[tokio::test]
    async fn mastra_generate_creates_a_thread_when_memory_requires_it() {
        let memory = Arc::new(StrictMemory::default());
        let agent = Agent::new(AgentConfig {
            id: "strict".into(),
            name: "Strict".into(),
            instructions: "Echo".into(),
            description: None,
            model: Arc::new(StaticModel::echo()),
            tools: Vec::new(),
            memory: Some(memory.clone()),
            memory_config: MemoryConfig::default(),
        });

        let mastra = Mastra::builder()
            .with_agent(agent)
            .with_memory("default", memory.clone())
            .build();
        let response = mastra
            .generate("strict", "hello strict memory")
            .await
            .expect("mastra should auto-create a thread for registered memory");

        assert_eq!(memory.list_threads(None).await.expect("threads").len(), 1);
        assert!(response.thread_id.is_some());
        assert_eq!(mastra.snapshot()["memory"][0], "default");
    }
}
