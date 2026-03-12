use indexmap::IndexMap;
use mastra_core::{MastraError, RequestContext, Workflow, WorkflowRunResult};
use serde::{Deserialize, Serialize};

pub type Result<T> = std::result::Result<T, MastraError>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InngestEvent {
    pub name: String,
    pub payload: serde_json::Value,
}

impl InngestEvent {
    pub fn new(name: impl Into<String>, payload: serde_json::Value) -> Self {
        Self {
            name: name.into(),
            payload,
        }
    }
}

#[derive(Clone)]
pub struct RegisteredWorkflow {
    pub event_name: String,
    pub workflow_id: String,
    workflow: Workflow,
}

impl RegisteredWorkflow {
    pub fn new(event_name: impl Into<String>, workflow: Workflow) -> Self {
        Self {
            event_name: event_name.into(),
            workflow_id: workflow.id().to_owned(),
            workflow,
        }
    }
}

#[derive(Clone, Default)]
pub struct InngestRuntime {
    workflows: IndexMap<String, RegisteredWorkflow>,
}

impl InngestRuntime {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(mut self, workflow: RegisteredWorkflow) -> Self {
        self.workflows
            .insert(workflow.event_name.clone(), workflow);
        self
    }

    pub fn bindings(&self) -> Vec<RegisteredWorkflow> {
        self.workflows.values().cloned().collect()
    }

    pub async fn dispatch(
        &self,
        event: InngestEvent,
        request_context: RequestContext,
    ) -> Result<WorkflowRunResult> {
        let workflow = self
            .workflows
            .get(&event.name)
            .ok_or_else(|| MastraError::not_found(format!("no workflow registered for event '{}'", event.name)))?;
        workflow.workflow.run(event.payload, request_context).await
    }
}
