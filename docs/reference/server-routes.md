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
- `POST /api/memory/threads/{thread_id}/clone`
- `GET /api/memory/threads/{thread_id}/messages`
- `POST /api/memory/threads/{thread_id}/messages`
- `POST /api/memory/messages/delete`
- named-memory equivalents under `/api/memory/{memory_id}/...`

## Workflows

- `GET /api/workflows`
- `GET /api/workflows/{workflow_id}`
- `POST /api/workflows/{workflow_id}/create-run`
- `POST /api/workflows/{workflow_id}/start-async`
- `POST /api/workflows/{workflow_id}/stream`
- `GET /api/workflows/{workflow_id}/runs`
- `GET /api/workflows/{workflow_id}/runs/{run_id}`
- `DELETE /api/workflows/{workflow_id}/runs/{run_id}`

## Misc

- `GET /api/health`
- `GET /api/routes`
- `GET /api/memories`
- `GET /api/system/packages`

## Not Yet Implemented

These upstream route families are still structural gaps in the Rust port:

- workflow `resume` / `resume-async`
- workflow cancel / time-travel
- vectors
- logs
- telemetry
- stored MCP clients
- tool providers
