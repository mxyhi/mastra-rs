use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use clap::ValueEnum;
use mastra_core::{
    Agent, AgentConfig, AgentGenerateRequest, MemoryConfig, MemoryEngine, RequestContext,
    StaticModel,
};
use mastra_memory::{ListThreadsQuery, Memory};
use mastra_store_libsql::{LibSqlStore, LibSqlStoreConfig};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum OutputFormat {
    Default,
    Json,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunOptions {
    pub prompt: String,
    pub thread_id: Option<String>,
    pub continue_latest: bool,
    pub resource_id: Option<String>,
    pub format: OutputFormat,
    pub storage_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RunOutput {
    pub text: String,
    pub thread_id: Option<String>,
    pub resource_id: Option<String>,
}

pub fn default_storage_path() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".mastracode")
        .join("memory.db")
}

fn prepare_storage_path(storage_path: &Path) -> mastra_core::Result<()> {
    if let Some(parent) = storage_path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| {
            mastra_core::MastraError::storage(format!(
                "create mastracode storage directory '{}': {error}",
                parent.display()
            ))
        })?;
    }

    Ok(())
}

fn storage_url(storage_path: &Path) -> String {
    format!("file:{}", storage_path.display())
}

fn open_memory(storage_path: &Path) -> mastra_core::Result<Memory> {
    prepare_storage_path(storage_path)?;

    Ok(Memory::new(LibSqlStore::new(LibSqlStoreConfig {
        url: storage_url(storage_path),
    })))
}

fn build_agent(memory: Arc<dyn MemoryEngine>) -> Agent {
    Agent::new(AgentConfig {
        id: "code-agent".to_owned(),
        name: "Code Agent".to_owned(),
        instructions: "You are the Rust port of MastraCode.".to_owned(),
        description: Some("Persistent headless MastraCode runner".to_owned()),
        model: Arc::new(StaticModel::echo()),
        tools: Vec::new(),
        memory: Some(memory),
        memory_config: MemoryConfig::default(),
    })
}

async fn resolve_thread_id(
    memory: &Memory,
    requested_thread_id: Option<String>,
    continue_latest: bool,
) -> mastra_core::Result<Option<String>> {
    if requested_thread_id.is_some() {
        return Ok(requested_thread_id);
    }

    if !continue_latest {
        return Ok(None);
    }

    let threads = memory
        .list_threads(ListThreadsQuery::default())
        .await
        .map_err(|error| mastra_core::MastraError::storage(error.to_string()))?;

    Ok(threads.items.first().map(|thread| thread.id.to_string()))
}

pub async fn run_headless(options: RunOptions) -> mastra_core::Result<RunOutput> {
    let storage_path = options.storage_path.unwrap_or_else(default_storage_path);
    let memory = Arc::new(open_memory(&storage_path)?);
    let thread_id =
        resolve_thread_id(&memory, options.thread_id.clone(), options.continue_latest).await?;
    let request_context = options
        .resource_id
        .clone()
        .map_or_else(RequestContext::new, |resource_id| {
            RequestContext::new().with_resource_id(resource_id)
        });
    let response = build_agent(memory)
        .generate(AgentGenerateRequest {
            prompt: options.prompt,
            thread_id,
            resource_id: options.resource_id.clone(),
            run_id: None,
            max_steps: None,
            request_context,
            ..Default::default()
        })
        .await?;

    Ok(RunOutput {
        text: response.text,
        thread_id: response.thread_id,
        resource_id: options.resource_id,
    })
}

pub fn render_output(output: &RunOutput, format: OutputFormat) -> String {
    match format {
        OutputFormat::Json => serde_json::to_string_pretty(output).expect("serialize output"),
        OutputFormat::Default => {
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
}

pub fn ready_message() -> &'static str {
    "mastracode headless runner is available via `mastracode run --prompt <text> --continue --format json`"
}

#[cfg(test)]
mod tests {
    use mastra_memory::MessageRole;
    use tempfile::tempdir;
    use uuid::Uuid;

    use super::{
        ListThreadsQuery, OutputFormat, RunOptions, RunOutput, default_storage_path, open_memory,
        ready_message, render_output, run_headless,
    };
    use mastra_memory::HistoryQuery;

    #[tokio::test]
    async fn run_headless_uses_persistent_memory_and_continue_reuses_latest_thread() {
        let temp = tempdir().expect("tempdir");
        let storage_path = temp.path().join("mastracode.db");

        let first = run_headless(RunOptions {
            prompt: "hello rust".to_owned(),
            thread_id: None,
            continue_latest: false,
            resource_id: Some("workspace-1".to_owned()),
            format: OutputFormat::Default,
            storage_path: Some(storage_path.clone()),
        })
        .await
        .expect("first headless run should succeed");

        let second = run_headless(RunOptions {
            prompt: "continue please".to_owned(),
            thread_id: None,
            continue_latest: true,
            resource_id: Some("workspace-1".to_owned()),
            format: OutputFormat::Default,
            storage_path: Some(storage_path.clone()),
        })
        .await
        .expect("second headless run should succeed");

        assert_eq!(first.resource_id.as_deref(), Some("workspace-1"));
        assert_eq!(second.resource_id.as_deref(), Some("workspace-1"));
        assert_eq!(second.thread_id, first.thread_id);

        let memory = open_memory(&storage_path).expect("persistent memory should open");
        let threads = memory
            .list_threads(ListThreadsQuery::default())
            .await
            .expect("threads should load");
        assert_eq!(threads.items.len(), 1);

        let thread_id = Uuid::parse_str(
            first
                .thread_id
                .as_deref()
                .expect("thread id should be persisted"),
        )
        .expect("uuid");
        let history = memory
            .history(HistoryQuery {
                thread_id,
                limit: None,
            })
            .await
            .expect("history should load");

        assert_eq!(history.len(), 4);
        assert_eq!(history[0].role, MessageRole::User);
        assert_eq!(history[0].text, "hello rust");
        assert_eq!(history[1].role, MessageRole::Assistant);
        assert_eq!(history[2].role, MessageRole::User);
        assert_eq!(history[2].text, "continue please");
        assert_eq!(history[3].role, MessageRole::Assistant);
    }

    #[test]
    fn render_output_supports_json_mode() {
        let rendered = render_output(
            &RunOutput {
                text: "hello".to_owned(),
                thread_id: Some("thread-1".to_owned()),
                resource_id: Some("resource-1".to_owned()),
            },
            OutputFormat::Json,
        );

        let payload: serde_json::Value = serde_json::from_str(&rendered).expect("json output");
        assert_eq!(payload["text"], "hello");
        assert_eq!(payload["thread_id"], "thread-1");
        assert_eq!(payload["resource_id"], "resource-1");
    }

    #[test]
    fn ready_message_points_to_official_continue_flag() {
        assert!(ready_message().contains("--continue"));
        assert!(ready_message().contains("--format json"));
    }

    #[test]
    fn default_storage_path_uses_mastracode_directory() {
        let path = default_storage_path();
        assert!(path.to_string_lossy().contains(".mastracode"));
        assert!(path.to_string_lossy().ends_with("memory.db"));
    }
}
