mod agent;
mod types;

pub use agent::{AiSdkAgent, AiSdkEventSource};
pub use mastra_client_sdks_client_js::GenerateResponse;
pub use types::{
    AiSdkError, AiSdkEvent, AiSdkEventStream, AiSdkFinishEvent, AiSdkGenerateRequest, AiSdkMessage,
    AiSdkRole, AiSdkRun, AiSdkStartEvent, AiSdkTextDeltaEvent, AssistantMessageAccumulator,
};

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use axum::serve;
    use mastra_client_sdks_client_js::MastraClient;
    use mastra_core::{Agent, AgentConfig, MemoryConfig, ModelRequest, ModelResponse, StaticModel};
    use mastra_server::{MastraRuntimeRegistry, MastraServer};
    use serde_json::Value;
    use tokio::{net::TcpListener, task::JoinHandle};

    use super::{
        AiSdkAgent, AiSdkEvent, AiSdkGenerateRequest, AiSdkMessage, AiSdkRole,
        AssistantMessageAccumulator,
    };

    struct TestHarness {
        base_url: String,
        task: JoinHandle<()>,
    }

    impl TestHarness {
        async fn spawn() -> Self {
            let server = MastraServer::new(MastraRuntimeRegistry::new());
            server.register_agent(Agent::new(AgentConfig {
                id: "echo".to_owned(),
                name: "Echo".to_owned(),
                instructions: "Echo prompt".to_owned(),
                description: Some("Echo test agent".to_owned()),
                model: Arc::new(StaticModel::new(|request: ModelRequest| async move {
                    Ok(ModelResponse {
                        text: format!("echo: {}", request.prompt),
                        data: Value::Null,
                        finish_reason: mastra_core::FinishReason::Stop,
                        usage: None,
                        tool_calls: Vec::new(),
                    })
                })),
                tools: Vec::new(),
                memory: None,
                memory_config: MemoryConfig::default(),
            }));

            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let address = listener.local_addr().unwrap();
            let task = tokio::spawn(async move {
                serve(listener, server.into_router()).await.unwrap();
            });

            Self {
                base_url: format!("http://{address}"),
                task,
            }
        }
    }

    impl Drop for TestHarness {
        fn drop(&mut self) {
            self.task.abort();
        }
    }

    #[tokio::test]
    async fn adapts_generate_responses_into_ai_sdk_events() {
        let harness = TestHarness::spawn().await;
        let client = MastraClient::builder(harness.base_url.clone())
            .build()
            .unwrap();
        let adapter = AiSdkAgent::new(client.agent("echo"));

        let run = adapter.generate_text("hello").await.unwrap();
        assert_eq!(run.assistant_message.role, AiSdkRole::Assistant);
        assert_eq!(run.assistant_message.content, "echo: hello");
        assert_eq!(run.events.len(), 3);

        let mut accumulator = AssistantMessageAccumulator::default();
        for event in &run.events {
            accumulator.apply(event);
        }

        assert_eq!(
            accumulator.current_message().unwrap().content,
            "echo: hello"
        );

        match &run.events[1] {
            AiSdkEvent::TextDelta(delta) => assert_eq!(delta.delta, "echo: hello"),
            other => panic!("unexpected event: {other:?}"),
        }
    }

    #[tokio::test]
    async fn converts_message_history_into_agent_generate_requests() {
        let harness = TestHarness::spawn().await;
        let client = MastraClient::builder(harness.base_url.clone())
            .build()
            .unwrap();
        let adapter = AiSdkAgent::new(client.agent("echo"));

        let run = adapter
            .generate(AiSdkGenerateRequest {
                messages: vec![
                    AiSdkMessage::new(AiSdkRole::System, "follow the plan"),
                    AiSdkMessage::new(AiSdkRole::User, "hello"),
                ],
                ..Default::default()
            })
            .await
            .unwrap();

        assert_eq!(
            run.assistant_message.content,
            "echo: follow the plan\nhello"
        );
    }
}
