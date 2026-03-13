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

## Current Non-Goals

This crate does not yet provide the larger upstream control plane:

- workflow resume or cancel routes
- vector routes
- logs routes
- telemetry routes
- stored MCP client management
