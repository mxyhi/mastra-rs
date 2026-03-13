# mastra-client-sdks-client-js

Rust HTTP client for the current Mastra server subset.

Despite the upstream-inspired package name, this crate is implemented in Rust and targets the Rust server routes in this repository.

## Included Today

- collection clients for agents, workflows, tools, and memories
- resource clients for agent, workflow, tool, memory, and memory thread operations
- workflow run listing, lookup, deletion, and streaming
- top-level convenience methods for collections and default memory threads

## Example

See [`docs/reference/client-js.md`](../../docs/reference/client-js.md).

## Current Boundary

This crate does not yet expose working memory, observational memory, vectors, logs, telemetry, or workflow resume/cancel APIs because the server/runtime surface is not present yet.
