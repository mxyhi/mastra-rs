use std::sync::Arc;

use indexmap::IndexMap;
use serde_json::Value;
use uuid::Uuid;

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

  pub fn list_agents(&self) -> Vec<String> {
    self.agents.keys().cloned().collect()
  }

  pub fn list_tools(&self) -> Vec<String> {
    self.tools.keys().cloned().collect()
  }

  pub fn list_workflows(&self) -> Vec<String> {
    self.workflows.keys().cloned().collect()
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

  pub async fn generate(&self, agent_id: &str, prompt: impl Into<String>) -> Result<AgentResponse> {
    let agent = self
      .get_agent(agent_id)
      .ok_or_else(|| MastraError::not_found(format!("agent '{}' was not registered", agent_id)))?;

    agent
      .generate(AgentGenerateRequest {
        prompt: prompt.into(),
        thread_id: Some(Uuid::now_v7().to_string()),
        resource_id: None,
        request_context: RequestContext::new(),
      })
      .await
  }

  pub async fn run_workflow(
    &self,
    workflow_id: &str,
    input: Value,
    request_context: RequestContext,
  ) -> Result<WorkflowRunResult> {
    let workflow = self
      .get_workflow(workflow_id)
      .ok_or_else(|| MastraError::not_found(format!("workflow '{}' was not registered", workflow_id)))?;
    workflow.run(input, request_context).await
  }
}

#[cfg(test)]
mod tests {
  use std::{collections::HashMap, sync::Arc};

  use async_trait::async_trait;
  use chrono::Utc;
  use parking_lot::RwLock;
  use serde_json::json;

  use crate::{
    agent::{Agent, AgentConfig},
    memory::{CreateThreadRequest, MemoryConfig, MemoryEngine, MemoryMessage, MemoryRecallRequest, MemoryRole, Thread},
    model::StaticModel,
    request_context::RequestContext,
    tool::Tool,
    workflow::{Step, Workflow},
    Mastra,
  };

  #[derive(Default)]
  struct TestMemory {
    threads: RwLock<HashMap<String, Thread>>,
    messages: RwLock<HashMap<String, Vec<MemoryMessage>>>,
  }

  #[async_trait]
  impl MemoryEngine for TestMemory {
    async fn create_thread(&self, request: CreateThreadRequest) -> crate::Result<Thread> {
      let thread = Thread {
        id: request.id.unwrap_or_else(|| uuid::Uuid::now_v7().to_string()),
        resource_id: request.resource_id,
        title: request.title,
        created_at: Utc::now(),
        metadata: request.metadata,
      };
      self.threads.write().insert(thread.id.clone(), thread.clone());
      Ok(thread)
    }

    async fn get_thread(&self, thread_id: &str) -> crate::Result<Option<Thread>> {
      Ok(self.threads.read().get(thread_id).cloned())
    }

    async fn list_threads(&self, _resource_id: Option<&str>) -> crate::Result<Vec<Thread>> {
      Ok(self.threads.read().values().cloned().collect())
    }

    async fn append_messages(&self, thread_id: &str, messages: Vec<MemoryMessage>) -> crate::Result<()> {
      self
        .messages
        .write()
        .entry(thread_id.to_string())
        .or_default()
        .extend(messages);
      Ok(())
    }

    async fn list_messages(&self, request: MemoryRecallRequest) -> crate::Result<Vec<MemoryMessage>> {
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

  #[tokio::test]
  async fn tool_and_workflow_round_trip() {
    let tool = Tool::new("sum", "add two numbers", |input, _| async move {
      let left = input["left"].as_i64().unwrap_or_default();
      let right = input["right"].as_i64().unwrap_or_default();
      Ok(json!({ "value": left + right }))
    });

    let workflow = Workflow::new("calc").then(Step::from_tool(tool.clone()));
    let mastra = Mastra::builder().with_tool(tool).with_workflow(workflow).build();

    let result = mastra
      .run_workflow("calc", json!({ "left": 2, "right": 3 }), RequestContext::new())
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
        request_context: RequestContext::new(),
      })
      .await
      .expect("agent should respond");

    assert!(response.text.contains("Yesterday was rainy"));
    assert!(response.text.contains("How is today?"));
  }
}
