use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use async_trait::async_trait;
use chrono::Utc;
use mastra_core::{
    Agent, AgentConfig, AgentGenerateRequest, CreateThreadRequest, MemoryConfig, MemoryEngine,
    MemoryMessage, MemoryRecallRequest, RequestContext, StaticModel, Thread,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunOptions {
    pub prompt: String,
    pub thread_id: Option<String>,
    pub resource_id: Option<String>,
    pub json: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RunOutput {
    pub text: String,
    pub thread_id: Option<String>,
    pub resource_id: Option<String>,
}

#[derive(Default)]
struct HeadlessMemory {
    threads: Mutex<HashMap<String, Thread>>,
    messages: Mutex<HashMap<String, Vec<MemoryMessage>>>,
}

#[async_trait]
impl MemoryEngine for HeadlessMemory {
    async fn create_thread(&self, request: CreateThreadRequest) -> mastra_core::Result<Thread> {
        let thread = Thread {
            id: request.id.unwrap_or_else(|| Uuid::now_v7().to_string()),
            resource_id: request.resource_id,
            title: request.title,
            created_at: Utc::now(),
            metadata: request.metadata,
        };
        self.threads
            .lock()
            .expect("thread mutex")
            .insert(thread.id.clone(), thread.clone());
        self.messages
            .lock()
            .expect("message mutex")
            .entry(thread.id.clone())
            .or_default();
        Ok(thread)
    }

    async fn get_thread(&self, thread_id: &str) -> mastra_core::Result<Option<Thread>> {
        Ok(self
            .threads
            .lock()
            .expect("thread mutex")
            .get(thread_id)
            .cloned())
    }

    async fn list_threads(&self, resource_id: Option<&str>) -> mastra_core::Result<Vec<Thread>> {
        let mut threads = self
            .threads
            .lock()
            .expect("thread mutex")
            .values()
            .filter(|thread| {
                resource_id
                    .map(|expected| thread.resource_id.as_deref() == Some(expected))
                    .unwrap_or(true)
            })
            .cloned()
            .collect::<Vec<_>>();
        threads.sort_by(|left, right| left.created_at.cmp(&right.created_at));
        Ok(threads)
    }

    async fn append_messages(
        &self,
        thread_id: &str,
        messages: Vec<MemoryMessage>,
    ) -> mastra_core::Result<()> {
        self.messages
            .lock()
            .expect("message mutex")
            .entry(thread_id.to_owned())
            .or_default()
            .extend(messages);
        Ok(())
    }

    async fn list_messages(
        &self,
        request: MemoryRecallRequest,
    ) -> mastra_core::Result<Vec<MemoryMessage>> {
        let mut messages = self
            .messages
            .lock()
            .expect("message mutex")
            .get(&request.thread_id)
            .cloned()
            .unwrap_or_default();
        if let Some(limit) = request.limit {
            let start = messages.len().saturating_sub(limit);
            messages = messages[start..].to_vec();
        }
        Ok(messages)
    }
}

fn build_agent(memory: Arc<dyn MemoryEngine>) -> Agent {
    Agent::new(AgentConfig {
        id: "code-agent".to_owned(),
        name: "Code Agent".to_owned(),
        instructions: "You are the Rust port of MastraCode.".to_owned(),
        description: Some("Minimal headless MastraCode runner".to_owned()),
        model: Arc::new(StaticModel::echo()),
        tools: Vec::new(),
        memory: Some(memory),
        memory_config: MemoryConfig::default(),
    })
}

pub async fn run_headless(options: RunOptions) -> mastra_core::Result<RunOutput> {
    let memory: Arc<dyn MemoryEngine> = Arc::new(HeadlessMemory::default());
    let agent = build_agent(Arc::clone(&memory));
    let request_context = options
        .resource_id
        .clone()
        .map_or_else(RequestContext::new, |resource_id| {
            RequestContext::new().with_resource_id(resource_id)
        });
    let response = agent
        .generate(AgentGenerateRequest {
            prompt: options.prompt,
            thread_id: options.thread_id,
            resource_id: options.resource_id.clone(),
            run_id: None,
            max_steps: None,
            request_context,
        })
        .await?;

    Ok(RunOutput {
        text: response.text,
        thread_id: response.thread_id,
        resource_id: options.resource_id,
    })
}

pub fn render_output(output: &RunOutput, json: bool) -> String {
    if json {
        serde_json::to_string_pretty(output).expect("serialize output")
    } else {
        let mut lines = vec![format!("text: {}", output.text)];
        if let Some(thread_id) = &output.thread_id {
            lines.push(format!("thread_id: {thread_id}"));
        }
        if let Some(resource_id) = &output.resource_id {
            lines.push(format!("resource_id: {resource_id}"));
        }
        lines.join("\n")
    }
}

pub fn ready_message() -> &'static str {
    "mastracode headless runner is available via `mastracode run --prompt <text>`"
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::{RunOptions, RunOutput, ready_message, render_output, run_headless};

    #[tokio::test]
    async fn run_headless_uses_agent_runtime_and_returns_thread() {
        let output = run_headless(RunOptions {
            prompt: "hello rust".to_owned(),
            thread_id: None,
            resource_id: Some("workspace-1".to_owned()),
            json: false,
        })
        .await
        .expect("headless run should succeed");

        assert_eq!(output.text, "hello rust");
        assert_eq!(output.resource_id.as_deref(), Some("workspace-1"));
        assert!(output.thread_id.is_some());
    }

    #[test]
    fn render_output_supports_json_mode() {
        let rendered = render_output(
            &RunOutput {
                text: "hello".to_owned(),
                thread_id: Some("thread-1".to_owned()),
                resource_id: Some("resource-1".to_owned()),
            },
            true,
        );

        let payload: Value = serde_json::from_str(&rendered).expect("json output");
        assert_eq!(payload["text"], "hello");
        assert_eq!(payload["thread_id"], "thread-1");
        assert_eq!(payload["resource_id"], "resource-1");
    }

    #[test]
    fn ready_message_points_to_headless_command() {
        assert!(ready_message().contains("mastracode run --prompt"));
    }
}
