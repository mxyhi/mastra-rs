use std::sync::Arc;

use futures::{stream, StreamExt};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::{
  error::{MastraError, Result},
  memory::{CreateThreadRequest, MemoryConfig, MemoryEngine, MemoryMessage, MemoryRecallRequest, MemoryRole},
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
    self.tools.iter().map(|tool| tool.id().to_string()).collect()
  }

  pub async fn generate(&self, request: AgentGenerateRequest) -> Result<AgentResponse> {
    let (thread_id, memory_context) = self.prepare_memory(&request.prompt, request.thread_id, request.resource_id).await?;
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

    self.persist_response(&thread_id, &request.prompt, &response).await?;

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
      let stream = agent.model.stream(ModelRequest {
        prompt: request.prompt,
        instructions: agent.instructions.clone(),
        memory: memory_context,
        tool_names: agent.tool_names(),
        request_context: request.request_context,
      });

      Ok::<_, MastraError>((agent.id.clone(), thread_id, stream))
    })
    .flat_map(|result| match result {
      Ok((agent_id, thread_id, stream)) => stream
        .map(move |event| {
          event.map(|event| AgentStreamResponse {
            id: agent_id.clone(),
            event,
            thread_id: thread_id.clone(),
          })
        })
        .boxed(),
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
