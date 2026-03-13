# create-mastra Starter

`create-mastra` currently ships one built-in Rust starter.

## Usage

```bash
cargo run -p create-mastra -- new ./demo-app
```

Current command surface:

- only subcommand: `new <PATH>`
- no interactive prompt
- no template selection flags

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

## Starter Shape

The generated project is intentionally small and only uses crates that already exist in this workspace:

- `mastra-core`
- `mastra-memory`
- `mastra-loggers`
- `tokio`

Generated `src/mastra/mastra.json` is a graph manifest with `schema_version`, path references, and per-node JSON assets. The current Rust CLI loader can consume that graph directly for `mastra lint`, `mastra dev`, and `mastra build`.

Generated `src/main.rs` currently:

- creates a real `Agent`
- uses `StaticModel::echo()`
- enables `Memory::in_memory()`
- executes one sample `agent.generate(...)` call
- embeds the generated graph assets with `include_str!`

Generated graph assets currently include:

- one default in-memory memory node
- one sum tool node
- one echo agent whose instructions live in `demo-agent.md`
- one workflow with `static_json` and `tool` steps

## Current Graph Subset Consumed By Rust CLI

The generated `schema_version` graph intentionally looks broader than the
runtime that consumes it. Today the Rust `mastra-cli` line only executes:

- top-level `app_name`
- `memories/tools/agents/workflows` arrays of `{ id, path }`
- agent `instructions` or `instructions_path`
- model kinds `echo` and `prefixed_echo`
- workflow step kinds `identity`, `static_json`, `tool`, and `agent`

Generated metadata such as `entrypoint`, `mastra_dir`, and `resources` is
preserved so the starter layout resembles upstream graph projects, but those
fields are not executed by the Rust loader/runtime yet.

## CLI Compatibility

The generated starter is intentionally aligned with the current Rust CLI subset:

- `mastra create` and `mastra init` delegate to this crate
- `mastra lint` validates the generated graph
- `mastra build` writes `.mastra/output/bundle.json` and `routes.txt`
- `mastra start` can boot from the produced bundle

## What This Is Not Yet

This Rust crate is still much narrower than the upstream JavaScript `create-mastra` flow:

- no template catalog or GitHub template ingestion
- no `--default`, `--components`, `--llm`, `--llm-api-key`, `--example`, `--mcp`, or `--skills`
- no package-manager detection or dependency installation

## Pending Alignment Notes

When the main CLI line grows beyond the current starter flow, the most likely follow-up documentation updates here are:

- how Rust-side scaffolding should map to future `mastra-cli create/init` flags
- whether multiple starter variants will be supported
- whether generated code should move beyond the current echo-model baseline
