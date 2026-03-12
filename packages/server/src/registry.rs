use std::sync::Arc;

use indexmap::IndexMap;
use parking_lot::RwLock;
use uuid::Uuid;

use crate::{
    contracts::{
        AgentSummary, CreateWorkflowRunRequest, StartWorkflowRunRequest, WorkflowRunRecord,
        WorkflowRunStatus, WorkflowSummary,
    },
    error::{ServerError, ServerResult},
    runtime::{AgentRuntime, WorkflowRuntime},
};

#[derive(Clone, Default)]
pub struct RuntimeRegistry {
    agents: Arc<RwLock<IndexMap<String, Arc<dyn AgentRuntime>>>>,
    workflows: Arc<RwLock<IndexMap<String, Arc<dyn WorkflowRuntime>>>>,
    workflow_runs: Arc<RwLock<IndexMap<Uuid, WorkflowRunRecord>>>,
}

impl RuntimeRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_agent<A>(&self, agent: A)
    where
        A: AgentRuntime + 'static,
    {
        let summary = agent.summary();
        self.agents
            .write()
            .insert(summary.id.clone(), Arc::new(agent));
    }

    pub fn register_workflow<W>(&self, workflow: W)
    where
        W: WorkflowRuntime + 'static,
    {
        let summary = workflow.summary();
        self.workflows
            .write()
            .insert(summary.id.clone(), Arc::new(workflow));
    }

    pub fn list_agents(&self) -> Vec<AgentSummary> {
        self.agents
            .read()
            .values()
            .map(|agent| agent.summary())
            .collect()
    }

    pub fn list_workflows(&self) -> Vec<WorkflowSummary> {
        self.workflows
            .read()
            .values()
            .map(|workflow| workflow.summary())
            .collect()
    }

    pub fn find_agent(&self, agent_id: &str) -> ServerResult<Arc<dyn AgentRuntime>> {
        self.agents
            .read()
            .get(agent_id)
            .cloned()
            .ok_or_else(|| ServerError::NotFound {
                resource: "agent",
                id: agent_id.to_owned(),
            })
    }

    pub fn find_workflow(&self, workflow_id: &str) -> ServerResult<Arc<dyn WorkflowRuntime>> {
        self.workflows
            .read()
            .get(workflow_id)
            .cloned()
            .ok_or_else(|| ServerError::NotFound {
                resource: "workflow",
                id: workflow_id.to_owned(),
            })
    }

    pub fn create_workflow_run(
        &self,
        workflow_id: &str,
        request: CreateWorkflowRunRequest,
    ) -> ServerResult<WorkflowRunRecord> {
        self.ensure_workflow_exists(workflow_id)?;

        let run = WorkflowRunRecord {
            run_id: Uuid::now_v7(),
            workflow_id: workflow_id.to_owned(),
            status: WorkflowRunStatus::Created,
            resource_id: request.resource_id,
            input_data: request.input_data,
            result: None,
            error: None,
        };

        self.workflow_runs.write().insert(run.run_id, run.clone());
        Ok(run)
    }

    pub fn begin_workflow_run(
        &self,
        workflow_id: &str,
        request: &StartWorkflowRunRequest,
    ) -> ServerResult<WorkflowRunRecord> {
        self.ensure_workflow_exists(workflow_id)?;

        let run = WorkflowRunRecord {
            run_id: Uuid::now_v7(),
            workflow_id: workflow_id.to_owned(),
            status: WorkflowRunStatus::Running,
            resource_id: request.resource_id.clone(),
            input_data: request.input_data.clone(),
            result: None,
            error: None,
        };

        self.workflow_runs.write().insert(run.run_id, run.clone());
        Ok(run)
    }

    pub fn complete_workflow_run_success(
        &self,
        run_id: Uuid,
        result: serde_json::Value,
    ) -> ServerResult<WorkflowRunRecord> {
        let mut runs = self.workflow_runs.write();
        let record = runs
            .get_mut(&run_id)
            .ok_or_else(|| ServerError::NotFound {
                resource: "workflow run",
                id: run_id.to_string(),
            })?;

        record.status = WorkflowRunStatus::Success;
        record.result = Some(result);
        record.error = None;
        Ok(record.clone())
    }

    pub fn complete_workflow_run_failure<E>(
        &self,
        run_id: Uuid,
        error: E,
    ) -> ServerResult<WorkflowRunRecord>
    where
        E: std::fmt::Display,
    {
        let mut runs = self.workflow_runs.write();
        let record = runs
            .get_mut(&run_id)
            .ok_or_else(|| ServerError::NotFound {
                resource: "workflow run",
                id: run_id.to_string(),
            })?;

        record.status = WorkflowRunStatus::Failed;
        record.error = Some(error.to_string());
        record.result = None;
        Ok(record.clone())
    }

    pub fn get_workflow_run(
        &self,
        workflow_id: &str,
        run_id: Uuid,
    ) -> ServerResult<WorkflowRunRecord> {
        let run = self
            .workflow_runs
            .read()
            .get(&run_id)
            .cloned()
            .ok_or_else(|| ServerError::NotFound {
                resource: "workflow run",
                id: run_id.to_string(),
            })?;

        if run.workflow_id != workflow_id {
            return Err(ServerError::NotFound {
                resource: "workflow run",
                id: run_id.to_string(),
            });
        }

        Ok(run)
    }

    fn ensure_workflow_exists(&self, workflow_id: &str) -> ServerResult<()> {
        self.find_workflow(workflow_id).map(|_| ())
    }
}
