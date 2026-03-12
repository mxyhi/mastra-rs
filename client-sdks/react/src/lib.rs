mod chat;
mod state;

pub use chat::ChatController;
pub use state::{ChatAction, ChatState, ChatStatus};

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use axum::serve;
    use mastra_client_sdks_ai_sdk::{AiSdkAgent, AiSdkMessage, AiSdkRole};
    use mastra_client_sdks_client_js::MastraClient;
    use mastra_core::{Agent, AgentConfig, MemoryConfig, ModelRequest, ModelResponse, StaticModel};
    use mastra_server::{MastraRuntimeRegistry, MastraServer};
    use serde_json::Value;
    use tokio::{net::TcpListener, task::JoinHandle};

    use super::{ChatAction, ChatController, ChatState, ChatStatus};

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

    #[test]
    fn reduces_chat_events_into_predictable_state() {
        let mut state = ChatState::default();
        state.apply(ChatAction::PushMessage(AiSdkMessage::new(
            AiSdkRole::User,
            "hello",
        )));
        assert_eq!(state.messages.len(), 1);
        assert_eq!(state.status, ChatStatus::Idle);
    }

    #[tokio::test]
    async fn controller_runs_agent_and_materializes_assistant_message() {
        let harness = TestHarness::spawn().await;
        let client = MastraClient::builder(harness.base_url.clone())
            .build()
            .unwrap();
        let adapter = AiSdkAgent::new(client.agent("echo"));
        let mut controller = ChatController::new(adapter);

        let run = controller.send_message("hello").await.unwrap();

        assert_eq!(run.assistant_message.content, "echo: hello");
        assert_eq!(controller.state().status, ChatStatus::Complete);
        assert_eq!(controller.state().messages.len(), 2);
        assert_eq!(controller.state().messages[0].content, "hello");
        assert_eq!(controller.state().messages[1].content, "echo: hello");
    }
}
