# mastra-server

Axum-based HTTP surface for the current Rust Mastra runtime.

## Route Groups

- agents
- tools
- memory
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

## Agent Execution Compatibility

- accepts camelCase generate/stream wire fields such as `runId`, `maxSteps`, `requestContext`, `activeTools`, and `toolChoice`
- accepts live memory request shape under `memory.thread/resource/options/readOnly`
- still accepts compatibility aliases `resourceId`, `threadId`, and `memory: false`
- rejects `memory.key` with `400` until the Rust runtime can switch named memories per request

## Current Non-Goals

This crate does not yet provide the larger upstream control plane:

- workflow resume or cancel routes
- workflow time-travel
- vector routes
- logs routes
- telemetry routes
- stored MCP client management
