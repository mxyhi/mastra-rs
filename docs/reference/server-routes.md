# Server Routes

Current public route surface implemented by `mastra-server`.

## Agents

- `GET /api/agents`
- `GET /api/agents/{agent_id}`
- `POST /api/agents/{agent_id}/generate`
- `POST /api/agents/{agent_id}/stream`
- `GET /api/agents/{agent_id}/tools`
- `POST /api/agents/{agent_id}/tools/{tool_id}/execute`

### Agent Request Body Notes

- `generate` and `stream` accept upstream-style camelCase fields:
  `runId`, `maxSteps`, `requestContext`, `activeTools`, `toolChoice.toolName`.
- Current live memory shape is accepted:
  `memory.thread`, `memory.resource`, `memory.options`, `memory.readOnly`.
- Compatibility aliases are also accepted for the current Rust port:
  top-level `resourceId` and `threadId`, plus `memory: false`.
- `memory.key` is rejected with `400 Bad Request` because per-request memory registry switching is not implemented in the current Rust runtime.
- `instructions`, `system`, `context`, and `output` are wired through, but today they are still normalized onto the Rust runtime's simpler `prompt + instructions + tool filter` execution model rather than the upstream full message-graph / structured-output engine.

## Tools

- `GET /api/tools`
- `GET /api/tools/{tool_id}`
- `POST /api/tools/{tool_id}/execute`

## Memory

- `GET /api/memory/threads`
- `POST /api/memory/threads`
- `GET /api/memory/threads/{thread_id}`
- `PATCH /api/memory/threads/{thread_id}`
- `DELETE /api/memory/threads/{thread_id}`
- `GET /api/memory/threads/{thread_id}/working-memory`
- `PUT /api/memory/threads/{thread_id}/working-memory`
- `GET /api/memory/threads/{thread_id}/observations`
- `POST /api/memory/threads/{thread_id}/observations`
- `POST /api/memory/threads/{thread_id}/clone`
- `GET /api/memory/threads/{thread_id}/messages`
- `POST /api/memory/threads/{thread_id}/messages`
- `POST /api/memory/messages/delete`
- named-memory equivalents under `/api/memory/{memory_id}/...`

### Working Memory Request Notes

- `GET .../working-memory` returns `{ working_memory: Option<WorkingMemoryState> }`
- `PUT .../working-memory` accepts `resourceId`, optional `scope`, optional `format`, optional `template`, and required `content`
- `scope` defaults to `thread`
- omitted `format` is inferred from `content`: strings become markdown, non-strings become JSON
- resource-scoped writes require `resourceId` if the state should be shared across threads for the same resource

### Observation Request And Query Notes

- `GET .../observations` accepts `page`, `perPage`, optional `resourceId`, and optional `scope`
- `POST .../observations` accepts `resourceId`, optional `scope`, `content`, `observedMessageIds`, and free-form `metadata`
- `scope` defaults to `thread`
- list responses return `observations`, `total`, `page`, `per_page`, and `has_more`
- this is a manual API surface for persisted observations, not the upstream automatic observer/reflector pipeline

## Workflows

- `GET /api/workflows`
- `GET /api/workflows/{workflow_id}`
- `POST /api/workflows/{workflow_id}/create-run`
- `POST /api/workflows/{workflow_id}/start?runId=...`
- `POST /api/workflows/{workflow_id}/start-async`
- `POST /api/workflows/{workflow_id}/observe?runId=...`
- `POST /api/workflows/{workflow_id}/restart?runId=...`
- `POST /api/workflows/{workflow_id}/restart-async?runId=...`
- `POST /api/workflows/{workflow_id}/restart-all-active-workflow-runs`
- `POST /api/workflows/{workflow_id}/restart-all-active-workflow-runs-async`
- `POST /api/workflows/{workflow_id}/resume`
- `POST /api/workflows/{workflow_id}/resume-async`
- `POST /api/workflows/{workflow_id}/resume-stream`
- `POST /api/workflows/{workflow_id}/stream`
- `GET /api/workflows/{workflow_id}/runs`
- `GET /api/workflows/{workflow_id}/runs/{run_id}`
- `DELETE /api/workflows/{workflow_id}/runs/{run_id}`
- `POST /api/workflows/{workflow_id}/runs/{run_id}/cancel`

### Start / Observe / Restart Notes

- `start` requires `runId` in the query string and starts an existing stored
  run in the background, returning `{ message }`.
- `observe` requires `runId` in the query string and streams the run's cached
  workflow events first, then live events from the current background task.
- `start` reuses the stored run's `resource_id`; if the body omits
  `inputData`, the server also reuses the stored `input_data`.
- `restart` / `restart-async` currently restart the stored run using the last
  persisted `input_data` and `resource_id`.
- `restart-all-active-workflow-runs` and `...-async` target runs whose current
  status is `running` or `suspended`.
- `tracingOptions` is currently accepted only for wire compatibility; the Rust
  runtime does not consume it yet.

### Resume Request Notes

- `resume` / `resume-async` / `resume-stream` accept `runId`, optional `step`,
  optional `resumeData`, and optional `requestContext`.
- `step` is currently accepted only for wire compatibility; the Rust runtime
  does not consume it when rebuilding the run.
- The current Rust runtime does not implement upstream suspend/resume
  checkpoints yet; it restarts the stored run using `resumeData` as the next
  `inputData` payload when supplied, otherwise it reuses the persisted
  `input_data`.

### Cancel Notes

- `POST .../runs/{run_id}/cancel` updates the persisted run record to
  `Cancelled`.
- If the run was started through a background lifecycle route (`start`,
  `restart`, `resume-stream`, or `stream`), the server also aborts the tracked
  async task and emits a terminal workflow error event with
  `"workflow run cancelled"`.
- This is still narrower than upstream interruptibility because the Rust core
  runtime has no step checkpoint or time-travel model.

## Misc

- `GET /api/health`
- `GET /api/routes`
- `GET /api/memories`
- `GET /api/system/packages`

## Not Yet Implemented

These upstream route families are still structural gaps in the Rust port:

- workflow time-travel
- workflow time-travel stream routes
- semantic recall / vector-backed memory
- automatic working-memory / observational-memory processors
- vectors
- logs
- telemetry
- voice
- networks / A2A
- stored MCP clients
- tool providers
