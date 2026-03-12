mod client;
mod error;
mod types;

pub use client::{
    AgentClient, AgentsClient, MastraClient, MastraClientBuilder, MemoriesClient, MemoryClient,
    WorkflowClient, WorkflowsClient,
};
pub use error::MastraClientError;
pub use types::{
    AgentMessages, AgentSummary, AppendMemoryMessagesRequest, AppendMemoryMessagesResponse,
    ChatMessage, CreateMemoryThreadRequest, CreateMemoryThreadResponse, CreateWorkflowRunRequest,
    ErrorResponse, FinishReason, GenerateRequest, GenerateResponse, GenerateStreamEvent,
    GenerateStreamFinishEvent, GenerateStreamStartEvent, GenerateStreamTextDeltaEvent,
    ListAgentsResponse, ListMemoriesResponse, ListMemoryMessagesResponse, ListThreadsResponse,
    ListWorkflowsResponse, MemoryMessageInput, MemoryMessageRole, MemorySummary, RouteDescription,
    StartWorkflowRunRequest, StartWorkflowRunResponse, UsageStats, WorkflowRunRecord,
    WorkflowRunRef, WorkflowRunStatus, WorkflowSummary,
};

#[cfg(test)]
mod tests {
    use std::{sync::Arc, time::Duration};

    use axum::serve;
    use mastra_core::{
        Agent, AgentConfig, MemoryConfig, ModelRequest, ModelResponse, StaticModel, Step, Workflow,
    };
    use mastra_memory::Memory;
    use mastra_server::{MastraRuntimeRegistry, MastraServer};
    use serde_json::{Value, json};
    use tokio::{net::TcpListener, task::JoinHandle};

    use super::{
        AgentMessages, AppendMemoryMessagesRequest, CreateMemoryThreadRequest,
        CreateWorkflowRunRequest, GenerateRequest, MastraClient, MastraClientBuilder,
        MastraClientError, MemoryMessageInput, MemoryMessageRole, StartWorkflowRunRequest,
        WorkflowRunStatus,
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

            server.register_workflow(Workflow::new("demo").then(Step::new(
                "shape",
                |input, _context| async move {
                    Ok(json!({
                        "accepted": true,
                        "input": input,
                    }))
                },
            )));
            server.register_memory("chat", Arc::new(Memory::in_memory()));

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
    async fn exercises_real_agent_workflow_and_memory_routes() {
        let harness = TestHarness::spawn().await;
        let client = MastraClientBuilder::new(harness.base_url.clone())
            .timeout(Duration::from_secs(3))
            .build()
            .unwrap();

        let agents = client.agents().list().await.unwrap();
        assert_eq!(agents.agents.len(), 1);
        assert_eq!(agents.agents[0].id, "echo");

        let generated = client
            .agent("echo")
            .generate(GenerateRequest {
                messages: AgentMessages::Text("hello".to_owned()),
                resource_id: None,
                thread_id: None,
                run_id: Some("run-1".to_owned()),
                max_steps: Some(1),
                request_context: Default::default(),
            })
            .await
            .unwrap();
        assert_eq!(generated.text, "echo: hello");

        let created_run = client
            .workflow("demo")
            .create_run(CreateWorkflowRunRequest {
                resource_id: Some("resource-1".to_owned()),
                input_data: Some(json!({"topic": "rust"})),
                request_context: Default::default(),
            })
            .await
            .unwrap();
        assert_eq!(created_run.status, WorkflowRunStatus::Created);

        let started = client
            .workflow("demo")
            .start_async(StartWorkflowRunRequest {
                resource_id: Some("resource-1".to_owned()),
                input_data: Some(json!({"topic": "rust"})),
                request_context: Default::default(),
            })
            .await
            .unwrap();
        assert_eq!(started.run.status, WorkflowRunStatus::Success);
        assert_eq!(
            started.run.result,
            Some(json!({
                "accepted": true,
                "input": {"topic": "rust"}
            }))
        );

        let fetched = client
            .workflow("demo")
            .run(started.run.run_id)
            .await
            .unwrap();
        assert_eq!(fetched.run_id, started.run.run_id);
        assert_eq!(fetched.result, started.run.result);

        let memories = client.memories().list().await.unwrap();
        assert_eq!(memories.memories.len(), 1);
        assert_eq!(memories.memories[0].id, "chat");

        let thread = client
            .memory("chat")
            .create_thread(CreateMemoryThreadRequest {
                id: None,
                resource_id: Some("resource-1".to_owned()),
                title: Some("Support thread".to_owned()),
                metadata: json!({"scope": "tests"}),
            })
            .await
            .unwrap()
            .thread;
        assert_eq!(thread.title.as_deref(), Some("Support thread"));

        let append_result = client
            .memory("chat")
            .append_messages(
                &thread.id,
                AppendMemoryMessagesRequest {
                    messages: vec![MemoryMessageInput {
                        role: MemoryMessageRole::User,
                        content: "hello memory".to_owned(),
                        metadata: json!({"kind": "greeting"}),
                    }],
                },
            )
            .await
            .unwrap();
        assert_eq!(append_result.appended, 1);

        let messages = client.memory("chat").messages(&thread.id).await.unwrap();
        assert_eq!(messages.messages.len(), 1);
        assert_eq!(messages.messages[0].content, "hello memory");
    }

    #[tokio::test]
    async fn surfaces_api_errors_with_status_and_server_message() {
        let harness = TestHarness::spawn().await;
        let client = MastraClient::builder(harness.base_url.clone())
            .build()
            .unwrap();

        let error = client
            .agent("missing")
            .generate_text("hello")
            .await
            .unwrap_err();
        match error {
            MastraClientError::Api { status, body, .. } => {
                assert_eq!(status.as_u16(), 404);
                assert!(body.contains("agent 'missing' was not found"));
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }
}
