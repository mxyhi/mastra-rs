use indexmap::IndexMap;
use mastra_core::{
    Agent, AgentGenerateRequest, RequestContext, Tool, ToolExecutionContext, Workflow,
};

use crate::{
    error::{McpError, Result},
    types::{McpPrompt, McpResource, McpTool},
};

#[derive(Clone)]
pub struct McpServerConfig {
    pub name: String,
    pub version: String,
    pub tools: IndexMap<String, Tool>,
    pub resources: IndexMap<String, McpResource>,
    pub prompts: IndexMap<String, McpPrompt>,
}

impl McpServerConfig {
    pub fn new(name: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            version: version.into(),
            tools: IndexMap::new(),
            resources: IndexMap::new(),
            prompts: IndexMap::new(),
        }
    }

    pub fn with_tool(mut self, tool: Tool) -> Self {
        self.tools.insert(tool.id().to_owned(), tool);
        self
    }

    pub fn with_resource(mut self, resource: McpResource) -> Self {
        self.resources.insert(resource.uri.clone(), resource);
        self
    }

    pub fn with_prompt(mut self, prompt: McpPrompt) -> Self {
        self.prompts.insert(prompt.name.clone(), prompt);
        self
    }

    pub fn with_agent(self, agent: Agent) -> Self {
        self.with_tool(agent_as_tool(agent))
    }

    pub fn with_workflow(self, workflow: Workflow) -> Self {
        self.with_tool(workflow_as_tool(workflow))
    }
}

#[derive(Clone)]
pub struct McpServer {
    config: McpServerConfig,
}

impl McpServer {
    pub fn new(config: McpServerConfig) -> Self {
        Self { config }
    }

    pub fn name(&self) -> &str {
        &self.config.name
    }

    pub fn version(&self) -> &str {
        &self.config.version
    }

    pub async fn list_tools(&self) -> Result<Vec<McpTool>> {
        Ok(self
            .config
            .tools
            .values()
            .map(|tool| McpTool {
                name: tool.id().to_owned(),
                description: tool.description().to_owned(),
                input_schema: match &tool.schema_snapshot()["input"] {
                    serde_json::Value::Null => None,
                    value => Some(value.clone()),
                },
                output_schema: match &tool.schema_snapshot()["output"] {
                    serde_json::Value::Null => None,
                    value => Some(value.clone()),
                },
            })
            .collect())
    }

    pub async fn call_tool(
        &self,
        tool_name: &str,
        input: serde_json::Value,
        request_context: RequestContext,
    ) -> Result<serde_json::Value> {
        let tool = self
            .config
            .tools
            .get(tool_name)
            .cloned()
            .ok_or_else(|| McpError::ToolNotFound(tool_name.to_owned()))?;
        tool.execute(
            input,
            ToolExecutionContext {
                thread_id: request_context.thread_id().map(str::to_owned),
                request_context,
                run_id: None,
                approved: true,
            },
        )
        .await
        .map_err(|error| McpError::Execution(error.message))
    }

    pub async fn list_resources(&self) -> Result<Vec<McpResource>> {
        Ok(self.config.resources.values().cloned().collect())
    }

    pub async fn read_resource(&self, uri: &str) -> Result<McpResource> {
        self.config
            .resources
            .get(uri)
            .cloned()
            .ok_or_else(|| McpError::ResourceNotFound(uri.to_owned()))
    }

    pub async fn list_prompts(&self) -> Result<Vec<McpPrompt>> {
        Ok(self.config.prompts.values().cloned().collect())
    }

    pub async fn get_prompt(&self, name: &str) -> Result<McpPrompt> {
        self.config
            .prompts
            .get(name)
            .cloned()
            .ok_or_else(|| McpError::PromptNotFound(name.to_owned()))
    }
}

fn agent_as_tool(agent: Agent) -> Tool {
    let tool_name = format!("ask_{}", agent.id());
    let description = agent
        .description()
        .map(str::to_owned)
        .unwrap_or_else(|| format!("Ask agent '{}'", agent.name()));

    Tool::new(tool_name, description, move |input, context| {
        let agent = agent.clone();
        async move {
            let prompt = extract_prompt(&input)?;
            let response = agent
                .generate(AgentGenerateRequest {
                    prompt,
                    thread_id: context.thread_id,
                    resource_id: context.request_context.resource_id().map(str::to_owned),
                    request_context: context.request_context,
                })
                .await?;
            serde_json::to_value(response)
                .map_err(|error| mastra_core::MastraError::tool(error.to_string()))
        }
    })
}

fn workflow_as_tool(workflow: Workflow) -> Tool {
    let workflow_id = workflow.id().to_owned();
    let tool_name = format!("run_{workflow_id}");
    Tool::new(
        tool_name,
        format!("Run workflow '{workflow_id}'"),
        move |input, context| {
            let workflow = workflow.clone();
            async move {
                let result = workflow.run(input, context.request_context).await?;
                Ok(result.output)
            }
        },
    )
}

fn extract_prompt(input: &serde_json::Value) -> mastra_core::Result<String> {
    input
        .get("prompt")
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned)
        .or_else(|| input.as_str().map(str::to_owned))
        .ok_or_else(|| mastra_core::MastraError::validation("agent MCP tools expect a prompt"))
}
