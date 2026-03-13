# mastra-core

Core runtime primitives for the Rust Mastra port.

## Included Today

- `Agent`
- `Tool`
- `Workflow`
- `MemoryEngine`
- `RequestContext`
- static test model helpers such as `StaticModel::echo()`

## Example

Run the minimal agent example:

```bash
cargo run -p mastra-core --example minimal_agent
```

## Notes

`Mastra` currently acts as a registry for:

- agents
- tools
- workflows
- memory instances

When `MemoryConfig` enables them, agents can now hydrate prompt context from
stored working memory and persisted observations before appending thread
history.

## CLI And Scaffolding Touchpoints

The current Rust scaffolding surface depends on this crate in two places:

- `create-mastra` generates a starter that boots `Agent`, `AgentGenerateRequest`, `MemoryConfig`, `RequestContext`, and `StaticModel::echo()`
- `mastracode` uses the same core request/runtime types for its headless runner

Those entry points are intentionally documented because they are part of the current user-facing CLI path, even though the broader upstream CLI product surface is not implemented yet.

Broader registries from upstream Mastra, such as gateways, telemetry, vectors, scorers, or deployer orchestration, are still outside the current Rust core subset.

This does not yet include upstream automatic working-memory updates,
observational-memory processors, or semantic recall.
