use std::sync::Arc;

use mastra_core::{Agent, AgentConfig, MemoryConfig, StaticModel, Step, Workflow};
use mastra_server::MastraHttpServer;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let server = MastraHttpServer::new();

    server.register_agent(Agent::new(AgentConfig {
        id: "echo".to_owned(),
        name: "Echo".to_owned(),
        instructions: "Echo the incoming prompt.".to_owned(),
        description: Some("Minimal mastra-server example".to_owned()),
        model: Arc::new(StaticModel::echo()),
        tools: Vec::new(),
        memory: None,
        memory_config: MemoryConfig::default(),
    }));

    server.register_workflow(Workflow::new("demo").then(Step::new(
        "shape",
        |input, _context| async move { Ok(input) },
    )));

    server.serve("127.0.0.1:4111".parse()?).await?;
    Ok(())
}
