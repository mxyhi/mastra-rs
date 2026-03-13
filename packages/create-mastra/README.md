# create-mastra

Scaffold a manifest-driven Rust Mastra starter against the local workspace crates.

## Usage

```bash
cargo run -p create-mastra -- new ./demo-app
```

`create-mastra` currently exposes only one command shape:

- `new <PATH>`

## Generated Files

- `Cargo.toml`
- `src/main.rs`
- `src/mastra/mastra.json`
- `src/mastra/memories/default-memory.json`
- `src/mastra/tools/demo-sum.json`
- `src/mastra/agents/demo-agent.json`
- `src/mastra/agents/demo-agent.md`
- `src/mastra/workflows/demo-workflow.json`
- `src/mastra/resources/hello.txt`
- `README.md`
- `.env.example`

## Generated Runtime Shape

The starter deliberately stays inside the subset already implemented by this workspace:

- `Agent` from `mastra-core`
- `Memory::in_memory()` from `mastra-memory`
- `StaticModel::echo()` as the sample model
- `init_tracing("info")` from `mastra-loggers`
- `mastra.json` as a snake_case project graph manifest for CLI-side loading
- one default memory, one sum tool, one echo agent, and one static_json workflow

## Current Boundary

This is a single built-in starter generator, not the full upstream template catalog.
It is now shaped so `mastra dev/build/start` can consume a non-empty project graph once the CLI-side loader lands.

Not implemented yet:

- interactive prompts
- selectable templates
- GitHub-template sources
- provider/bootstrap flags such as `--llm` or `--components`
