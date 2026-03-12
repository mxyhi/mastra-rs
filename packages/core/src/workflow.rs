use std::{future::Future, pin::Pin, sync::Arc};

use futures::FutureExt;
use indexmap::IndexMap;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
  agent::{Agent, AgentGenerateRequest},
  error::Result,
  request_context::RequestContext,
  tool::{Tool, ToolExecutionContext},
};

type StepFuture = Pin<Box<dyn Future<Output = Result<Value>> + Send + 'static>>;
type StepHandler = Arc<dyn Fn(Value, StepExecutionContext) -> StepFuture + Send + Sync>;

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct StepExecutionContext {
  pub request_context: RequestContext,
  pub run_id: Option<String>,
  pub state: Value,
}

#[derive(Clone)]
pub struct StepConfig {
  pub id: String,
  pub description: Option<String>,
  pub input_schema: Option<Value>,
  pub output_schema: Option<Value>,
  pub handler: StepHandler,
}

#[derive(Clone)]
pub struct Step {
  id: String,
  description: Option<String>,
  input_schema: Option<Value>,
  output_schema: Option<Value>,
  handler: StepHandler,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub enum WorkflowRunStatus {
  Success,
  Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct WorkflowRunResult {
  pub workflow_id: String,
  pub status: WorkflowRunStatus,
  pub output: Value,
  pub step_outputs: IndexMap<String, Value>,
}

#[derive(Clone)]
pub struct Workflow {
  id: String,
  steps: Vec<Step>,
}

impl Step {
  pub fn new<F, Fut>(id: impl Into<String>, handler: F) -> Self
  where
    F: Fn(Value, StepExecutionContext) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<Value>> + Send + 'static,
  {
    Self {
      id: id.into(),
      description: None,
      input_schema: None,
      output_schema: None,
      handler: Arc::new(move |input, context| Box::pin(handler(input, context))),
    }
  }

  pub fn from_config(config: StepConfig) -> Self {
    Self {
      id: config.id,
      description: config.description,
      input_schema: config.input_schema,
      output_schema: config.output_schema,
      handler: config.handler,
    }
  }

  pub fn from_tool(tool: Tool) -> Self {
    let tool_id = tool.id().to_string();
    Self {
      id: tool_id.clone(),
      description: Some(tool.description().to_string()),
      input_schema: Some(tool.schema_snapshot()["input"].clone()),
      output_schema: Some(tool.schema_snapshot()["output"].clone()),
      handler: Arc::new(move |input, context| {
        let tool = tool.clone();
        async move {
          tool
            .execute(
              input,
              ToolExecutionContext {
                request_context: context.request_context,
                run_id: context.run_id,
                thread_id: None,
                approved: true,
              },
            )
            .await
        }
        .boxed()
      }),
    }
  }

  pub fn from_agent(agent: Agent) -> Self {
    let id = agent.id().to_string();
    Self {
      id,
      description: agent.description().map(str::to_string),
      input_schema: None,
      output_schema: None,
      handler: Arc::new(move |input, context| {
        let agent = agent.clone();
        async move {
          let prompt = input
            .get("prompt")
            .and_then(Value::as_str)
            .map(str::to_string)
            .or_else(|| input.as_str().map(str::to_string))
            .unwrap_or_else(|| input.to_string());

          let response = agent
            .generate(AgentGenerateRequest {
              prompt,
              thread_id: None,
              resource_id: None,
              request_context: context.request_context,
            })
            .await?;
          Ok(serde_json::to_value(response).unwrap_or(Value::Null))
        }
        .boxed()
      }),
    }
  }

  pub async fn execute(&self, input: Value, context: StepExecutionContext) -> Result<Value> {
    (self.handler)(input, context).await
  }

  pub fn id(&self) -> &str {
    &self.id
  }

  pub fn description(&self) -> Option<&str> {
    self.description.as_deref()
  }

  pub fn input_schema(&self) -> Option<&Value> {
    self.input_schema.as_ref()
  }

  pub fn output_schema(&self) -> Option<&Value> {
    self.output_schema.as_ref()
  }
}

impl Workflow {
  pub fn new(id: impl Into<String>) -> Self {
    Self {
      id: id.into(),
      steps: Vec::new(),
    }
  }

  pub fn id(&self) -> &str {
    &self.id
  }

  pub fn then(mut self, step: Step) -> Self {
    self.steps.push(step);
    self
  }

  pub fn steps(&self) -> &[Step] {
    &self.steps
  }

  pub async fn run(&self, input: Value, request_context: RequestContext) -> Result<WorkflowRunResult> {
    let mut current = input;
    let mut step_outputs = IndexMap::new();

    for step in &self.steps {
      let output = step
        .execute(
          current,
          StepExecutionContext {
            request_context: request_context.clone(),
            run_id: Some(uuid::Uuid::now_v7().to_string()),
            state: Value::Null,
          },
        )
        .await?;
      current = output.clone();
      step_outputs.insert(step.id().to_string(), output);
    }

    Ok(WorkflowRunResult {
      workflow_id: self.id.clone(),
      status: WorkflowRunStatus::Success,
      output: current,
      step_outputs,
    })
  }
}
