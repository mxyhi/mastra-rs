# mastra-server

Axum-based HTTP surface for the current Rust Mastra runtime, not the upstream
framework-agnostic Mastra server handler package.

## Route Groups

- agents
- tools
- memory threads/messages, working memory, and observations
- workflows
- system packages
- health and route catalog

## Example

```bash
cargo run -p mastra-server --example minimal_server
```

Then inspect:

```bash
curl http://127.0.0.1:4111/api/routes
```

## CLI Touchpoints

`mastra-cli` currently wraps this crate for:

- `mastra-cli dev`
- `mastra-cli start`
- `mastra-cli routes`
- `mastra-cli build`

Current behavior:

- `dev` loads a project graph from `src/mastra/mastra.json`, normalizes it, registers the graph into `MastraHttpServer`, and serves it
- `build` validates the same graph and writes a normalized `.mastra/output/bundle.json` plus `routes.txt`
- `start` loads the built bundle from `.mastra/output` and boots the same runtime from bundled data
- `routes` prints this crate's route catalog without starting a server

This is still a simplified Rust port: the CLI path now loads project graphs end to end, but it is not yet the upstream bundler/runtime/studio stack.

## Agent Execution Compatibility

- accepts camelCase generate/stream wire fields such as `runId`, `maxSteps`, `requestContext`, `activeTools`, and `toolChoice`
- accepts live memory request shape under `memory.thread/resource/options/readOnly`
- still accepts compatibility aliases `resourceId`, `threadId`, and `memory: false`
- rejects `memory.key` with `400` until the Rust runtime can switch named memories per request

## Current Non-Goals

This crate does not yet provide the larger upstream control plane:

- workflow time-travel
- workflow time-travel stream routes
- semantic recall
- vector routes
- logs routes
- telemetry routes
- voice routes
- network / A2A routes
- stored MCP client management

## Newly Aligned Workflow Control Routes

The Rust server now exposes the main upstream workflow lifecycle routes:

- `POST /api/workflows/{workflow_id}/start?runId=...`
- `POST /api/workflows/{workflow_id}/resume`
- `POST /api/workflows/{workflow_id}/resume-async`
- `POST /api/workflows/{workflow_id}/resume-stream`
- `POST /api/workflows/{workflow_id}/observe?runId=...`
- `POST /api/workflows/{workflow_id}/restart?runId=...`
- `POST /api/workflows/{workflow_id}/restart-async?runId=...`
- `POST /api/workflows/{workflow_id}/restart-all-active-workflow-runs`
- `POST /api/workflows/{workflow_id}/restart-all-active-workflow-runs-async`
- `POST /api/workflows/{workflow_id}/runs/{run_id}/cancel`

Current resume semantics are intentionally simple: the server restarts the
stored run with `resumeData` as the new `inputData` payload when provided,
otherwise it reuses the last persisted `input_data`.

The request contract still accepts an optional `step` field for wire
compatibility, but the current Rust runtime ignores it during resume.

Current `start` / `observe` / `restart*` semantics are still intentionally
simple compared with upstream:

- `start` and `restart` are control routes that return `{ message }` after
  scheduling a background task
- `observe` replays cached workflow events first and then tails the live event
  channel for the run
- `restart*` reuses the stored run's last `resource_id` and `input_data`
- `restart-all-active*` only targets runs currently marked `Running` or
  `Suspended`

Current cancel semantics are still narrower than upstream time-travel capable
engines, but no longer status-only: the cancel route marks the stored run
record as `Cancelled` and aborts a tracked background workflow task when one is
registered.

## Working Memory And Observations

The current Rust server now exposes manual memory state endpoints for both the
default memory and named memories:

- `GET/PUT /api/memory/threads/{thread_id}/working-memory`
- `GET/PUT /api/memory/{memory_id}/threads/{thread_id}/working-memory`
- `GET/POST /api/memory/threads/{thread_id}/observations`
- `GET/POST /api/memory/{memory_id}/threads/{thread_id}/observations`

Current behavior:

- `scope` defaults to `thread` for both updates and observation writes
- `resourceId` can be supplied to read or write resource-scoped state
- omitted working-memory `format` is inferred from `content`: strings become markdown, non-strings become JSON
- this is still a manual API surface, not the upstream automatic working-memory / observational-memory processor pipeline
