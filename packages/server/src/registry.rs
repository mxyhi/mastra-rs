use std::sync::Arc;

use chrono::Utc;
use indexmap::IndexMap;
use mastra_core::{MemoryEngine, Tool};
use parking_lot::RwLock;
use tokio::{sync::broadcast, task::AbortHandle};
use uuid::Uuid;

use crate::{
    contracts::{
        AgentSummary, CreateWorkflowRunRequest, ListWorkflowRunsQuery, ListWorkflowRunsResponse,
        MemorySummary, StartWorkflowRunRequest, ToolSummary, WorkflowRunRecord, WorkflowRunStatus,
        WorkflowStreamEvent, WorkflowSummary,
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
    workflow_run_events: Arc<RwLock<IndexMap<Uuid, Vec<WorkflowStreamEvent>>>>,
    workflow_run_channels: Arc<RwLock<IndexMap<Uuid, broadcast::Sender<WorkflowStreamEvent>>>>,
    workflow_run_tasks: Arc<RwLock<IndexMap<Uuid, AbortHandle>>>,
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
        let now = Utc::now();
        let run_id = request
            .run_id
            .as_deref()
            .map(|value| {
                Uuid::parse_str(value).map_err(|error| {
                    ServerError::BadRequest(format!("invalid runId '{value}': {error}"))
                })
            })
            .transpose()?
            .unwrap_or_else(Uuid::now_v7);

        let run = WorkflowRunRecord {
            run_id,
            workflow_id: workflow_id.to_owned(),
            status: WorkflowRunStatus::Created,
            created_at: now,
            updated_at: now,
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
        let now = Utc::now();

        let run = WorkflowRunRecord {
            run_id,
            workflow_id: workflow_id.to_owned(),
            status: WorkflowRunStatus::Running,
            created_at: now,
            updated_at: now,
            resource_id: request.resource_id.clone(),
            input_data: request.input_data.clone(),
            result: None,
            error: None,
        };

        self.workflow_runs.write().insert(run.run_id, run.clone());
        Ok(run)
    }

    pub fn restart_workflow_run(
        &self,
        workflow_id: &str,
        run_id: Uuid,
        request: &StartWorkflowRunRequest,
    ) -> ServerResult<WorkflowRunRecord> {
        self.ensure_workflow_exists(workflow_id)?;
        let mut runs = self.workflow_runs.write();
        let record = runs.get_mut(&run_id).ok_or_else(|| ServerError::NotFound {
            resource: "workflow run",
            id: run_id.to_string(),
        })?;

        if record.workflow_id != workflow_id {
            return Err(ServerError::NotFound {
                resource: "workflow run",
                id: run_id.to_string(),
            });
        }

        if let Some(handle) = self.workflow_run_tasks.write().shift_remove(&run_id) {
            handle.abort();
        }

        record.status = WorkflowRunStatus::Running;
        record.updated_at = Utc::now();
        record.resource_id = request
            .resource_id
            .clone()
            .or_else(|| record.resource_id.clone());
        record.input_data = request
            .input_data
            .clone()
            .or_else(|| record.input_data.clone());
        record.result = None;
        record.error = None;
        Ok(record.clone())
    }

    pub fn register_workflow_task(&self, run_id: Uuid, handle: AbortHandle) {
        self.workflow_run_tasks.write().insert(run_id, handle);
    }

    pub fn clear_workflow_task(&self, run_id: Uuid) {
        self.workflow_run_tasks.write().shift_remove(&run_id);
    }

    pub fn subscribe_workflow_events(
        &self,
        run_id: Uuid,
    ) -> broadcast::Receiver<WorkflowStreamEvent> {
        let sender = {
            let mut channels = self.workflow_run_channels.write();
            channels
                .entry(run_id)
                .or_insert_with(|| {
                    let (sender, _receiver) = broadcast::channel(64);
                    sender
                })
                .clone()
        };
        sender.subscribe()
    }

    pub fn reset_workflow_events(&self, run_id: Uuid) {
        self.workflow_run_events.write().shift_remove(&run_id);
        self.workflow_run_channels.write().shift_remove(&run_id);
    }

    pub fn record_workflow_event(&self, run_id: Uuid, event: WorkflowStreamEvent) {
        self.workflow_run_events
            .write()
            .entry(run_id)
            .or_default()
            .push(event.clone());

        let sender = {
            let mut channels = self.workflow_run_channels.write();
            channels
                .entry(run_id)
                .or_insert_with(|| {
                    let (sender, _receiver) = broadcast::channel(64);
                    sender
                })
                .clone()
        };
        let _ = sender.send(event);
    }

    pub fn workflow_events(&self, run_id: Uuid) -> Vec<WorkflowStreamEvent> {
        self.workflow_run_events
            .read()
            .get(&run_id)
            .cloned()
            .unwrap_or_default()
    }

    pub fn list_active_workflow_run_ids(&self, workflow_id: &str) -> ServerResult<Vec<Uuid>> {
        self.ensure_workflow_exists(workflow_id)?;
        Ok(self
            .workflow_runs
            .read()
            .values()
            .filter(|run| run.workflow_id == workflow_id)
            .filter(|run| {
                matches!(
                    run.status,
                    WorkflowRunStatus::Running | WorkflowRunStatus::Suspended
                )
            })
            .map(|run| run.run_id)
            .collect())
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
        record.updated_at = Utc::now();
        record.result = Some(result);
        record.error = None;
        self.workflow_run_tasks.write().shift_remove(&run_id);
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
        record.updated_at = Utc::now();
        record.error = Some(error.to_string());
        record.result = None;
        self.workflow_run_tasks.write().shift_remove(&run_id);
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

    pub fn delete_workflow_run(&self, workflow_id: &str, run_id: Uuid) -> ServerResult<()> {
        self.ensure_workflow_exists(workflow_id)?;
        let mut runs = self.workflow_runs.write();
        let run = runs.get(&run_id).ok_or_else(|| ServerError::NotFound {
            resource: "workflow run",
            id: run_id.to_string(),
        })?;
        if run.workflow_id != workflow_id {
            return Err(ServerError::NotFound {
                resource: "workflow run",
                id: run_id.to_string(),
            });
        }

        runs.shift_remove(&run_id);
        self.workflow_run_tasks.write().shift_remove(&run_id);
        self.workflow_run_events.write().shift_remove(&run_id);
        self.workflow_run_channels.write().shift_remove(&run_id);
        Ok(())
    }

    pub fn cancel_workflow_run(
        &self,
        workflow_id: &str,
        run_id: Uuid,
    ) -> ServerResult<WorkflowRunRecord> {
        self.ensure_workflow_exists(workflow_id)?;
        let mut runs = self.workflow_runs.write();
        let run = runs.get_mut(&run_id).ok_or_else(|| ServerError::NotFound {
            resource: "workflow run",
            id: run_id.to_string(),
        })?;
        if run.workflow_id != workflow_id {
            return Err(ServerError::NotFound {
                resource: "workflow run",
                id: run_id.to_string(),
            });
        }

        run.status = WorkflowRunStatus::Cancelled;
        run.updated_at = Utc::now();
        run.error = None;
        if let Some(handle) = self.workflow_run_tasks.write().shift_remove(&run_id) {
            handle.abort();
        }
        Ok(run.clone())
    }

    pub fn list_workflow_runs(
        &self,
        workflow_id: &str,
        query: &ListWorkflowRunsQuery,
    ) -> ServerResult<ListWorkflowRunsResponse> {
        self.ensure_workflow_exists(workflow_id)?;
        if matches!(query.per_page, Some(0)) {
            return Err(ServerError::BadRequest(
                "perPage must be greater than zero".to_owned(),
            ));
        }

        let mut runs = self
            .workflow_runs
            .read()
            .values()
            .filter(|run| run.workflow_id == workflow_id)
            .filter(|run| {
                query
                    .resource_id
                    .as_deref()
                    .map(|resource_id| run.resource_id.as_deref() == Some(resource_id))
                    .unwrap_or(true)
            })
            .filter(|run| {
                query
                    .status
                    .as_ref()
                    .map(|status| &run.status == status)
                    .unwrap_or(true)
            })
            .filter(|run| {
                query
                    .from_date
                    .map(|from| run.created_at >= from)
                    .unwrap_or(true)
            })
            .filter(|run| query.to_date.map(|to| run.created_at <= to).unwrap_or(true))
            .cloned()
            .collect::<Vec<_>>();
        runs.sort_by_key(|run| (run.created_at, run.run_id));

        let total = runs.len();
        let page = query.page.unwrap_or(0);
        let per_page = query.per_page.unwrap_or_else(|| total.max(1));
        let start = page.saturating_mul(per_page);
        let runs = runs.into_iter().skip(start).take(per_page).collect();

        Ok(ListWorkflowRunsResponse { runs, total })
    }

    fn ensure_workflow_exists(&self, workflow_id: &str) -> ServerResult<()> {
        self.find_workflow(workflow_id).map(|_| ())
    }
}
