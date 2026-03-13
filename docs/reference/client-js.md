# Rust Client SDK Surface

Current convenience and resource APIs exposed by `mastra-client-sdks-client-js`.

## Top-Level Client

```rust
use mastra_client_sdks_client_js::MastraClient;

let client = MastraClient::new("http://127.0.0.1:4111")?;
let agents = client.list_agents().await?;
let workflows = client.list_workflows().await?;
let thread = client.create_memory_thread(/* ... */).await?;
let thread_client = client.get_memory_thread(thread.thread.id.clone());
# Ok::<(), mastra_client_sdks_client_js::MastraClientError>(())
```

## Resource Clients

- `client.get_agent(id)` / `client.agent(id)`
- `client.get_workflow(id)` / `client.workflow(id)`
- `client.get_tool(id)` / `client.tool(id)`
- `client.get_memory(id)` / `client.memory(id)`
- `client.default_memory()`

## Workflow Support

- create run
- start
- start async
- observe
- restart
- restart async
- restart all active
- restart all active async
- resume
- resume async
- resume stream
- stream
- cancel run by id
- list runs with filters
- get run by id
- delete run by id

### Workflow Lifecycle Control Methods

Current lifecycle methods mirror the current Rust server surface:

- `WorkflowClient::start(run_id, request)` -> `{ message }`
- `WorkflowClient::observe(run_id)` -> SSE `WorkflowStreamEvent`
- `WorkflowClient::restart(run_id, request)` -> `{ message }`
- `WorkflowClient::restart_async(run_id, request)` -> `StartWorkflowRunResponse`
- `WorkflowClient::restart_all_active()` -> `{ message }`
- `WorkflowClient::restart_all_active_async()` -> `{ message }`

Notes:

- `start` and `restart` send `runId` as a query parameter, matching the Rust
  server's control-route shape.
- `observe` consumes the same cached-first SSE stream exposed by
  `/api/workflows/{workflow_id}/observe`.
- `RestartWorkflowRunRequest.tracing_options` is currently only wire
  compatibility state; the Rust runtime does not act on it yet.

## Memory Support

- list threads with pagination and ordering
- create, fetch, update, clone, and delete threads
- append, list, and delete messages
- get and update working memory
- list and append observations
- top-level default-memory helpers

### Working Memory And Observation Methods

Current Rust client parity matches the current Rust server routes, not the
larger upstream TypeScript client shape.

Top-level named/default memory helpers:

- `MemoryClient::get_working_memory(thread_id)`
- `MemoryClient::update_working_memory(thread_id, request)`
- `MemoryClient::observations(thread_id)`
- `MemoryClient::observations_with_query(thread_id, query)`
- `MemoryClient::append_observation(thread_id, request)`

Per-thread helpers:

- `MemoryThreadClient::get_working_memory()`
- `MemoryThreadClient::update_working_memory(request)`
- `MemoryThreadClient::observations()`
- `MemoryThreadClient::observations_with_query(query)`
- `MemoryThreadClient::append_observation(request)`

Example:

```rust
use mastra_client_sdks_client_js::{
    AppendObservationInput, ListObservationsQuery, MastraClient, UpdateWorkingMemoryInput,
};
use mastra_core::MemoryScope;
use serde_json::json;

let client = MastraClient::new("http://127.0.0.1:4111")?;
let memory = client.memory("chat");

let updated = memory
    .update_working_memory(
        "thread-1",
        UpdateWorkingMemoryInput {
            resource_id: Some("user-123".to_owned()),
            scope: Some(MemoryScope::Resource),
            format: None,
            template: None,
            content: json!("# User Profile\n- Name: Ada\n- Preferences: rust, cli\n"),
        },
    )
    .await?;

let observations = memory
    .observations_with_query(
        "thread-1",
        ListObservationsQuery {
            page: Some(0),
            per_page: Some("20".to_owned()),
            resource_id: Some("user-123".to_owned()),
            scope: Some(MemoryScope::Resource),
        },
    )
    .await?;

let appended = memory
    .append_observation(
        "thread-1",
        AppendObservationInput {
            resource_id: Some("user-123".to_owned()),
            scope: Some(MemoryScope::Resource),
            content: "User prefers concise Rust examples.".to_owned(),
            observed_message_ids: Vec::new(),
            metadata: json!({"source": "manual"}),
        },
    )
    .await?;
# let _ = (updated, observations, appended);
# Ok::<(), mastra_client_sdks_client_js::MastraClientError>(())
```

Notes:

- working-memory `format` is optional; the server infers markdown for string content and JSON for structured content
- observations use the same `page/perPage/resourceId/scope` query shape as the Rust server contract, with `perPage` encoded as a string query value
- this is a manual read/write API slice; automatic working-memory tools and observational-memory processors are still out of scope

## Agent Generate / Stream Request Shape

Current `GenerateRequest` serializes upstream-style camelCase wire keys.

- top-level execution keys:
  `runId`, `maxSteps`, `requestContext`, `activeTools`, `toolChoice`, `output`
- prompt overrides:
  `instructions`, `system`, `context`
- memory options:
  `GenerateMemoryConfig::Options(GenerateMemoryOptions { thread, resource, options, read_only, .. })`
- compatibility alias still supported:
  `GenerateMemoryConfig::Enabled(false)` disables recall and persistence for the request

The Rust client now shares the server contract types for `GenerateMemoryConfig` and `ToolChoice`, so request builders and server handlers use the same wire model.

## Not Yet Implemented

- semantic recall clients
- automatic working-memory or observational-memory processors
- vectors, logs, and telemetry clients
- full upstream structured-output/runtime-processor semantics
