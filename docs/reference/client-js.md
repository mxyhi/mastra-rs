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
- start async
- stream
- list runs with filters
- get run by id
- delete run by id

## Memory Support

- list threads with pagination and ordering
- create, fetch, update, clone, and delete threads
- append, list, and delete messages
- top-level default-memory helpers

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

- working memory APIs
- observational memory APIs
- vectors, logs, and telemetry clients
- workflow resume or cancel APIs
- full upstream structured-output/runtime-processor semantics
