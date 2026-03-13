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

Broader registries from upstream Mastra, such as gateways, telemetry, vectors, scorers, or deployer orchestration, are still outside the current Rust core subset.
