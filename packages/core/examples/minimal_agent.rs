use mastra_core::{
    Agent, AgentConfig, AgentGenerateRequest, MemoryConfig, RequestContext, StaticModel,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let agent = Agent::new(AgentConfig {
        id: "demo-agent".to_owned(),
        name: "Demo Agent".to_owned(),
        instructions: "Echo the prompt.".to_owned(),
        description: Some("Minimal mastra-core example".to_owned()),
        model: std::sync::Arc::new(StaticModel::echo()),
        tools: Vec::new(),
        memory: None,
        memory_config: MemoryConfig::default(),
    });

    let response = agent
        .generate(AgentGenerateRequest {
            prompt: "hello from mastra-core".to_owned(),
            thread_id: None,
            resource_id: Some("example".to_owned()),
            run_id: None,
            max_steps: Some(1),
            request_context: RequestContext::new().with_resource_id("example"),
        })
        .await?;

    println!("{}", response.text);
    Ok(())
}
