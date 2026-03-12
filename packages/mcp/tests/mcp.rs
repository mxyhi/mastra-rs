use std::sync::Arc;

use mastra_core::{
    Agent, AgentConfig, MemoryConfig, RequestContext, StaticModel, Step, Tool, Workflow,
};
use mastra_packages_mcp::{
    McpClient, McpPrompt, McpPromptArgument, McpPromptMessage, McpResource, McpResourceContents,
    McpServer, McpServerConfig,
};
use serde_json::json;

fn build_agent() -> Agent {
    Agent::new(AgentConfig {
        id: "helper".to_owned(),
        name: "Helper".to_owned(),
        instructions: "Answer briefly".to_owned(),
        description: Some("Agent exposed via MCP".to_owned()),
        model: Arc::new(StaticModel::new(|request| async move {
            Ok(mastra_core::ModelResponse {
                text: format!("agent: {}", request.prompt),
                data: json!({ "echo": request.prompt }),
                finish_reason: mastra_core::FinishReason::Stop,
                usage: None,
                tool_calls: Vec::new(),
            })
        })),
        tools: Vec::new(),
        memory: None,
        memory_config: MemoryConfig::default(),
    })
}

#[tokio::test]
async fn local_client_lists_tools_and_calls_tool() {
    let weather_tool = Tool::new("weather", "Return weather", |input, _| async move {
        let location = input
            .get("location")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("unknown");
        Ok(json!({
            "forecast": format!("sunny in {location}"),
        }))
    });
    let server =
        McpServer::new(McpServerConfig::new("test-server", "1.0.0").with_tool(weather_tool));
    let client = McpClient::local(server);

    let tools = client.list_tools().await.expect("tools should list");
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0].name, "weather");

    let result = client
        .call_tool("weather", json!({ "location": "Shanghai" }))
        .await
        .expect("tool should execute");

    assert_eq!(result, json!({ "forecast": "sunny in Shanghai" }));
}

#[tokio::test]
async fn local_client_reads_resources_and_prompts() {
    let server = McpServer::new(
        McpServerConfig::new("test-server", "1.0.0")
            .with_resource(McpResource::new(
                "file:///notes/today.md",
                "today",
                McpResourceContents::text("# Notes"),
            ))
            .with_prompt(McpPrompt::new(
                "daily-summary",
                "Daily summary prompt",
                vec![McpPromptArgument::new("topic", true)],
                vec![McpPromptMessage::user("Summarize the work for {{topic}}")],
            )),
    );
    let client = McpClient::local(server);

    let resources = client
        .list_resources()
        .await
        .expect("resources should list");
    assert_eq!(resources.len(), 1);
    assert_eq!(resources[0].uri, "file:///notes/today.md");

    let contents = client
        .read_resource("file:///notes/today.md")
        .await
        .expect("resource should exist");
    assert_eq!(contents.as_text(), Some("# Notes"));

    let prompts = client.list_prompts().await.expect("prompts should list");
    assert_eq!(prompts.len(), 1);
    assert_eq!(prompts[0].name, "daily-summary");

    let prompt = client
        .get_prompt("daily-summary")
        .await
        .expect("prompt should exist");
    assert_eq!(prompt.messages.len(), 1);
    assert_eq!(
        prompt.messages[0].content,
        "Summarize the work for {{topic}}"
    );
}

#[tokio::test]
async fn server_can_expose_agents_and_workflows_as_tools() {
    let agent = build_agent();
    let workflow = Workflow::new("echo-workflow")
        .then(Step::new("echo-step", |input, _| async move {
            Ok(json!({ "workflow": input }))
        }));

    let server = McpServer::new(
        McpServerConfig::new("test-server", "1.0.0")
            .with_agent(agent)
            .with_workflow(workflow),
    );
    let client = McpClient::local(server);

    let tools = client.list_tools().await.expect("tools should list");
    let tool_names = tools
        .iter()
        .map(|tool| tool.name.as_str())
        .collect::<Vec<_>>();
    assert!(tool_names.contains(&"ask_helper"));
    assert!(tool_names.contains(&"run_echo-workflow"));

    let agent_response = client
        .call_tool("ask_helper", json!({ "prompt": "hello" }))
        .await
        .expect("agent tool should execute");
    assert_eq!(agent_response["text"], "agent: hello");

    let workflow_response = client
        .call_tool("run_echo-workflow", json!({ "value": 42 }))
        .await
        .expect("workflow tool should execute");
    assert_eq!(workflow_response["workflow"]["value"], 42);
}

#[tokio::test]
async fn agent_tool_forwards_request_context_values() {
    let agent = build_agent();
    let server = McpServer::new(McpServerConfig::new("test-server", "1.0.0").with_agent(agent));
    let client = McpClient::local(server);

    let result = client
        .call_tool_with_context(
            "ask_helper",
            json!({ "prompt": "hello" }),
            RequestContext::new().with_resource_id("workspace-1"),
        )
        .await
        .expect("agent tool should execute");

    assert_eq!(result["text"], "agent: hello");
}
