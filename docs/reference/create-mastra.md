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
- `README.md`
- `.env.example`

## Starter Shape

The generated project is intentionally small and only uses crates that already exist in this workspace:

- `mastra-core`
- `mastra-memory`
- `mastra-loggers`
- `tokio`

Generated `src/main.rs` currently:

- creates a real `Agent`
- uses `StaticModel::echo()`
- enables `Memory::in_memory()`
- executes one sample `agent.generate(...)` call

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
