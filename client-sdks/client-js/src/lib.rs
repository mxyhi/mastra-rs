mod client;
mod error;
mod types;

pub use client::{
    AgentClient, AgentsClient, MastraClient, MastraClientBuilder, MemoriesClient, MemoryClient,
    MemoryThreadClient, ToolClient, ToolsClient, WorkflowClient, WorkflowsClient,
};
pub use error::MastraClientError;
pub use types::{
    AgentDetail, AgentDetailResponse, AgentMessages, AgentSummary, AppendMemoryMessagesRequest,
    AppendMemoryMessagesResponse, ChatMessage, CloneMemoryThreadMessageFilter,
    CloneMemoryThreadOptions, CloneMemoryThreadRequest, CloneMemoryThreadResponse,
    CreateMemoryThreadRequest, CreateMemoryThreadResponse, CreateWorkflowRunRequest,
    DeleteMemoryMessagesInput, DeleteMemoryMessagesRequest, DeleteMemoryMessagesResponse,
    ErrorResponse, ExecuteToolRequest, ExecuteToolResponse, FinishReason, GenerateRequest,
    GenerateResponse, GenerateStreamEvent, GenerateStreamFinishEvent, GenerateStreamStartEvent,
    GenerateStreamTextDeltaEvent, GenerateStreamToolCallEvent, GenerateStreamToolResultEvent,
    ListAgentsResponse, ListMemoriesResponse, ListMemoryMessagesResponse, ListMessagesQuery,
    ListThreadsQuery, ListThreadsResponse, ListToolsResponse, ListWorkflowRunsQuery,
    ListWorkflowRunsResponse, ListWorkflowsResponse, MemoryMessageInput, MemoryMessageRole,
    MemorySummary, RouteDescription, StartWorkflowRunRequest, StartWorkflowRunResponse,
    SystemPackage, SystemPackagesResponse, ToolSummary, UsageStats, WorkflowDetail,
    WorkflowDetailResponse, WorkflowRunRecord, WorkflowRunRef, WorkflowRunStatus,
    WorkflowStreamEvent, WorkflowStreamFinishEvent, WorkflowStreamQuery, WorkflowStreamStartEvent,
    WorkflowStreamStepEvent, WorkflowSummary,
};

#[cfg(test)]
mod tests {
    use std::{sync::Arc, time::Duration};

    use axum::serve;
    use futures::StreamExt;
    use mastra_core::{
        Agent, AgentConfig, MemoryConfig, ModelRequest, ModelResponse, StaticModel, Step, Tool,
        Workflow,
    };
    use mastra_memory::Memory;
    use mastra_server::{MastraRuntimeRegistry, MastraServer};
    use serde_json::{Value, json};
    use tokio::{net::TcpListener, task::JoinHandle};

    use super::{
        AgentMessages, AppendMemoryMessagesRequest, CloneMemoryThreadRequest,
        CreateMemoryThreadRequest, CreateWorkflowRunRequest, DeleteMemoryMessagesInput,
        DeleteMemoryMessagesRequest, ExecuteToolRequest, GenerateRequest, ListMessagesQuery,
        ListThreadsQuery, ListWorkflowRunsQuery, MastraClient, MastraClientBuilder,
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
            let ping_tool = Tool::new("ping", "Ping test tool", |input, _context| async move {
                Ok(json!({
                    "echo": input,
                    "ok": true,
                }))
            });

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
                tools: vec![ping_tool.clone()],
                memory: None,
                memory_config: MemoryConfig::default(),
            }));
            server.register_tool(ping_tool);

            server.register_workflow(Workflow::new("demo").then(Step::new(
                "shape",
                |input, _context| async move {
                    Ok(json!({
                        "accepted": true,
                        "input": input,
                    }))
                },
            )));
            server.register_memory("default", Arc::new(Memory::in_memory()));
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
        assert_eq!(memories.memories.len(), 2);
        assert!(
            memories
                .memories
                .iter()
                .any(|memory| memory.id == "default")
        );
        assert!(memories.memories.iter().any(|memory| memory.id == "chat"));

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

    #[tokio::test]
    async fn supports_memory_pagination_clone_delete_and_workflow_stream_routes() {
        let harness = TestHarness::spawn().await;
        let client = MastraClientBuilder::new(harness.base_url.clone())
            .timeout(Duration::from_secs(3))
            .build()
            .unwrap();
        let memory = client.memory("chat");

        let first_thread = memory
            .create_thread(CreateMemoryThreadRequest {
                id: None,
                resource_id: Some("resource-1".to_owned()),
                title: Some("Thread A".to_owned()),
                metadata: json!({}),
            })
            .await
            .unwrap()
            .thread;
        let second_thread = memory
            .create_thread(CreateMemoryThreadRequest {
                id: None,
                resource_id: Some("resource-1".to_owned()),
                title: Some("Thread B".to_owned()),
                metadata: json!({}),
            })
            .await
            .unwrap()
            .thread;

        memory
            .append_messages(
                &first_thread.id,
                AppendMemoryMessagesRequest {
                    messages: vec![
                        MemoryMessageInput {
                            role: MemoryMessageRole::User,
                            content: "first".to_owned(),
                            metadata: json!({}),
                        },
                        MemoryMessageInput {
                            role: MemoryMessageRole::Assistant,
                            content: "second".to_owned(),
                            metadata: json!({}),
                        },
                    ],
                },
            )
            .await
            .unwrap();

        let paged_threads = memory
            .threads_with_query(ListThreadsQuery {
                page: Some(0),
                per_page: Some(1),
                resource_id: Some("resource-1".to_owned()),
                metadata: None,
            })
            .await
            .unwrap();
        assert_eq!(paged_threads.threads.len(), 1);
        assert_eq!(paged_threads.total, 2);
        assert_eq!(paged_threads.page, 0);
        assert_eq!(paged_threads.per_page, 1);
        assert!(paged_threads.has_more);

        let paged_messages = memory
            .messages_with_query(
                &first_thread.id,
                ListMessagesQuery {
                    page: Some(1),
                    per_page: Some(1),
                    resource_id: None,
                    message_ids: None,
                    start_date: None,
                    end_date: None,
                },
            )
            .await
            .unwrap();
        assert_eq!(paged_messages.messages.len(), 1);
        assert_eq!(paged_messages.messages[0].content, "second");
        assert_eq!(paged_messages.total, 2);

        let cloned = memory
            .clone_thread(
                &first_thread.id,
                CloneMemoryThreadRequest {
                    new_thread_id: None,
                    resource_id: None,
                    title: Some("Thread A clone".to_owned()),
                    metadata: None,
                    message_limit: Some(1),
                    options: None,
                },
            )
            .await
            .unwrap();
        assert_eq!(cloned.cloned_messages.len(), 1);
        assert_eq!(cloned.cloned_messages[0].content, "second");

        let deleted = memory
            .delete_messages(DeleteMemoryMessagesRequest {
                message_ids: DeleteMemoryMessagesInput::MessageId(
                    paged_messages.messages[0].id.clone(),
                ),
            })
            .await
            .unwrap();
        assert_eq!(deleted.deleted, 1);
        assert!(deleted.success);

        let remaining = memory.messages(&first_thread.id).await.unwrap();
        assert_eq!(remaining.messages.len(), 1);
        assert_eq!(remaining.messages[0].content, "first");

        memory.delete_thread(&cloned.thread.id).await.unwrap();
        let threads_after_delete = memory.threads().await.unwrap();
        assert_eq!(threads_after_delete.total, 2);
        assert!(
            threads_after_delete
                .threads
                .iter()
                .all(|thread| thread.id != cloned.thread.id)
        );
        assert!(
            threads_after_delete
                .threads
                .iter()
                .any(|thread| thread.id == second_thread.id)
        );

        let workflow_events = client
            .workflow("demo")
            .stream_with_run_id(
                "018f7f26-8b7e-7c9d-b145-2c3d4e5f6789",
                StartWorkflowRunRequest {
                    resource_id: Some("resource-stream".to_owned()),
                    input_data: Some(json!({"topic": "rust"})),
                    request_context: Default::default(),
                },
            )
            .await
            .unwrap()
            .collect::<Vec<_>>()
            .await;
        assert!(
            workflow_events
                .iter()
                .any(|event| matches!(event, Ok(crate::WorkflowStreamEvent::Start(_))))
        );
        assert!(
            workflow_events
                .iter()
                .any(|event| matches!(event, Ok(crate::WorkflowStreamEvent::StepStart(_))))
        );
        assert!(
            workflow_events
                .iter()
                .any(|event| matches!(event, Ok(crate::WorkflowStreamEvent::Finish(_))))
        );

        let workflow_runs = client.workflow("demo").runs().await.unwrap();
        assert_eq!(workflow_runs.total, 1);
        assert_eq!(workflow_runs.runs.len(), 1);

        let packages = client.system_packages().await.unwrap();
        assert!(packages.packages.is_empty());
        assert!(!packages.is_dev);
        assert!(!packages.cms_enabled);
    }

    #[tokio::test]
    async fn supports_resource_wrappers_default_memory_and_filtered_workflow_runs() {
        let harness = TestHarness::spawn().await;
        let client = MastraClientBuilder::new(harness.base_url.clone())
            .timeout(Duration::from_secs(3))
            .build()
            .unwrap();

        let agent_detail = client.agent("echo").details().await.unwrap();
        assert_eq!(agent_detail.agent.instructions, "Echo prompt");
        assert_eq!(agent_detail.agent.tools.len(), 1);
        assert_eq!(agent_detail.agent.tools[0].id, "ping");

        let agent_tools = client.agent("echo").tools().await.unwrap();
        assert_eq!(agent_tools.tools.len(), 1);
        assert_eq!(agent_tools.tools[0].id, "ping");

        let global_tools = client.tools().list().await.unwrap();
        assert_eq!(global_tools.tools.len(), 1);
        assert_eq!(global_tools.tools[0].id, "ping");

        let tool_detail = client.tool("ping").details().await.unwrap();
        assert_eq!(tool_detail.id, "ping");

        let tool_result = client
            .tool("ping")
            .execute(ExecuteToolRequest {
                data: json!({ "value": "pong" }),
                approved: false,
                run_id: None,
                thread_id: None,
                request_context: Default::default(),
            })
            .await
            .unwrap();
        assert_eq!(tool_result.tool_id, "ping");
        assert_eq!(tool_result.output["ok"], true);

        let workflow_detail = client.workflow("demo").details().await.unwrap();
        assert_eq!(workflow_detail.workflow.id, "demo");
        assert_eq!(workflow_detail.workflow.steps.len(), 1);
        assert_eq!(workflow_detail.workflow.steps[0].id, "shape");

        let created = client
            .workflow("demo")
            .create_run(CreateWorkflowRunRequest {
                resource_id: Some("resource-created".to_owned()),
                input_data: Some(json!({"topic": "draft"})),
                request_context: Default::default(),
            })
            .await
            .unwrap();
        assert_eq!(created.status, WorkflowRunStatus::Created);

        let started = client
            .workflow("demo")
            .start_async(StartWorkflowRunRequest {
                resource_id: Some("resource-success".to_owned()),
                input_data: Some(json!({"topic": "rust"})),
                request_context: Default::default(),
            })
            .await
            .unwrap();
        assert_eq!(started.run.status, WorkflowRunStatus::Success);

        let filtered_runs = client
            .workflow("demo")
            .runs_with_query(ListWorkflowRunsQuery {
                page: Some(0),
                per_page: Some(10),
                resource_id: Some("resource-success".to_owned()),
                status: Some(WorkflowRunStatus::Success),
                from_date: None,
                to_date: None,
            })
            .await
            .unwrap();
        assert_eq!(filtered_runs.total, 1);
        assert_eq!(filtered_runs.runs.len(), 1);
        assert_eq!(
            filtered_runs.runs[0].resource_id.as_deref(),
            Some("resource-success")
        );
        assert_eq!(filtered_runs.runs[0].status, WorkflowRunStatus::Success);

        let default_thread = client
            .default_memory()
            .create_thread(CreateMemoryThreadRequest {
                id: None,
                resource_id: Some("resource-default".to_owned()),
                title: Some("Default thread".to_owned()),
                metadata: json!({"scope": "default"}),
            })
            .await
            .unwrap()
            .thread;
        assert_eq!(default_thread.title.as_deref(), Some("Default thread"));

        let default_thread_client = client.default_memory().thread(default_thread.id.clone());
        let fetched_default_thread = default_thread_client.get().await.unwrap();
        assert_eq!(fetched_default_thread.id, default_thread.id);

        default_thread_client
            .append_messages(AppendMemoryMessagesRequest {
                messages: vec![MemoryMessageInput {
                    role: MemoryMessageRole::User,
                    content: "default hello".to_owned(),
                    metadata: json!({}),
                }],
            })
            .await
            .unwrap();

        let default_messages = default_thread_client.messages().await.unwrap();
        assert_eq!(default_messages.messages.len(), 1);
        assert_eq!(default_messages.messages[0].content, "default hello");
    }
}
