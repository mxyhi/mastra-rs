# create-mastra

Scaffold a manifest-driven Rust Mastra starter against the local workspace crates.

## Usage

```bash
cargo run -p create-mastra -- demo-app --default --llm openai
```

`create-mastra` now supports the upstream-style root invocation surface:

- `[project-name]` or `--project-name`
- `--default`
- `--components agents,tools,workflows,scorers`
- `--llm <provider>` / `--llm-api-key <key>`
- `--example` / `--no-example`
- `--dir <path>` for the generated `mastra_dir`
- `--mcp <editor>`
- `--template <name>`

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
- `src/mastra/scorers/answer-relevancy.rs` when `scorers` is selected
- `README.md`
- `.env.example`

## Generated Runtime Shape

The starter deliberately stays inside the subset already implemented by this workspace:

- `Agent` from `mastra-core`
- `Memory::in_memory()` from `mastra-memory`
- `StaticModel::echo()` as the sample model
- `init_tracing("info")` from `mastra-loggers`
- `mastra.json` as a `schema_version` project graph manifest with per-node path references
- one default memory, one sum tool, one echo agent, and one static_json workflow

## CLI-Consumed Graph Subset

The generated `schema_version` graph is intentionally larger than what the Rust
CLI currently executes. Today `mastra lint/dev/build/start` only consumes:

- top-level `app_name`
- `memories/tools/agents/workflows` entries shaped as `{ id, path }`
- agent `instructions` or `instructions_path`
- model kinds `echo` and `prefixed_echo`
- workflow step kinds `identity`, `static_json`, `tool`, and `agent`

Generated metadata such as `entrypoint`, `mastra_dir`, and `resources` is kept
for starter parity, but the Rust CLI/runtime does not execute those fields yet.
The generated manifest now also records starter metadata for provider hint,
template hint, MCP target, example toggle, and selected components.

## Current Boundary

This is a single built-in starter generator, not the full upstream template catalog.
It is now shaped so `mastra lint/dev/build/start` can consume a non-empty project graph end to end.

Not implemented yet:

- interactive prompts
- real template materialization beyond metadata
- GitHub-template sources
- dependency installation or package-manager bootstrap
