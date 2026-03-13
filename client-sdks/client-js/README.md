# mastra-client-sdks-client-js

Rust HTTP client for the current Mastra server subset.

Despite the upstream-inspired package name, this crate is implemented in Rust and targets the Rust server routes in this repository.

## Included Today

- collection clients for agents, workflows, tools, and memories
- resource clients for agent, workflow, tool, memory, and memory thread operations
- workflow run listing, lookup, deletion, resume/cancel, and streaming
- top-level convenience methods for collections and default memory threads
- camelCase request serialization for generate/stream, workflow run, and memory thread operations
- shared `GenerateMemoryConfig` / `ToolChoice` contract types with the Rust server crate

## Example

See [`docs/reference/client-js.md`](../../docs/reference/client-js.md).

## Current Boundary

This crate does not yet expose working memory, observational memory, vectors,
logs, or telemetry APIs. Workflow lifecycle coverage now includes
`resume`, `resume_async`, `resume_stream`, and `cancel_run_by_id`.
