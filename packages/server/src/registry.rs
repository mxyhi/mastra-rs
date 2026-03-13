use std::sync::Arc;

use indexmap::IndexMap;
use mastra_core::{MemoryEngine, Tool};
use parking_lot::RwLock;
use uuid::Uuid;

use crate::{
    contracts::{
        AgentSummary, CreateWorkflowRunRequest, MemorySummary, StartWorkflowRunRequest,
        ToolSummary, WorkflowRunRecord, WorkflowRunStatus, WorkflowSummary,
    },
    error::{ServerError, ServerResult},
    runtime::{AgentRuntime, WorkflowRuntime},
};

#[derive(Clone, Default)]
pub struct RuntimeRegistry {
    agents: Arc<RwLock<IndexMap<String, Arc<dyn AgentRuntime>>>>,
    memory: Arc<RwLock<IndexMap<String, Arc<dyn MemoryEngine>>>>,
    tools: Arc<RwLock<IndexMap<String, Tool>>>,
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

    pub fn register_memory(&self, memory_id: impl Into<String>, memory: Arc<dyn MemoryEngine>) {
        self.memory.write().insert(memory_id.into(), memory);
    }

    pub fn register_tool(&self, tool: Tool) {
        self.tools.write().insert(tool.id().to_owned(), tool);
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

    pub fn list_memory(&self) -> Vec<MemorySummary> {
        self.memory
            .read()
            .keys()
            .cloned()
            .map(|id| MemorySummary { id })
            .collect()
    }

    pub fn list_tools(&self) -> Vec<ToolSummary> {
        let mut tools = IndexMap::new();

        for tool in self.tools.read().values() {
            tools
                .entry(tool.id().to_owned())
                .or_insert_with(|| ToolSummary::from_tool(tool));
        }

        for agent in self.agents.read().values() {
            for tool in agent.tool_summaries() {
                tools.entry(tool.id.clone()).or_insert(tool);
            }
        }

        tools.into_values().collect()
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

    pub fn find_memory(&self, memory_id: &str) -> ServerResult<Arc<dyn MemoryEngine>> {
        self.memory
            .read()
            .get(memory_id)
            .cloned()
            .ok_or_else(|| ServerError::NotFound {
                resource: "memory",
                id: memory_id.to_owned(),
            })
    }

    pub fn find_default_memory(&self) -> ServerResult<Arc<dyn MemoryEngine>> {
        let memories = self.memory.read();
        if let Some(memory) = memories.get("default").cloned() {
            return Ok(memory);
        }

        if memories.len() == 1 {
            return Ok(memories.values().next().cloned().expect("single memory"));
        }

        Err(ServerError::BadRequest(
            "default memory is not available; register a `default` memory or expose a single memory instance"
                .to_owned(),
        ))
    }

    pub fn find_tool(&self, tool_id: &str) -> ServerResult<Tool> {
        if let Some(tool) = self.tools.read().get(tool_id).cloned() {
            return Ok(tool);
        }

        for agent in self.agents.read().values() {
            if let Some(tool) = agent.tools().into_iter().find(|tool| tool.id() == tool_id) {
                return Ok(tool);
            }
        }

        Err(ServerError::NotFound {
            resource: "tool",
            id: tool_id.to_owned(),
        })
    }

    pub fn get_tool_summary(&self, tool_id: &str) -> ServerResult<ToolSummary> {
        Ok(ToolSummary::from_tool(&self.find_tool(tool_id)?))
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
        self.begin_workflow_run_with_id(workflow_id, request, Uuid::now_v7())
    }

    pub fn begin_workflow_run_with_id(
        &self,
        workflow_id: &str,
        request: &StartWorkflowRunRequest,
        run_id: Uuid,
    ) -> ServerResult<WorkflowRunRecord> {
        self.ensure_workflow_exists(workflow_id)?;

        let run = WorkflowRunRecord {
            run_id,
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
        let record = runs.get_mut(&run_id).ok_or_else(|| ServerError::NotFound {
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
        let record = runs.get_mut(&run_id).ok_or_else(|| ServerError::NotFound {
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

    pub fn list_workflow_runs(&self, workflow_id: &str) -> ServerResult<Vec<WorkflowRunRecord>> {
        self.ensure_workflow_exists(workflow_id)?;

        let mut runs = self
            .workflow_runs
            .read()
            .values()
            .filter(|run| run.workflow_id == workflow_id)
            .cloned()
            .collect::<Vec<_>>();
        runs.sort_by_key(|run| run.run_id);
        Ok(runs)
    }

    fn ensure_workflow_exists(&self, workflow_id: &str) -> ServerResult<()> {
        self.find_workflow(workflow_id).map(|_| ())
    }
}
