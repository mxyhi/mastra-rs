# mastra-rs

Rust workspace aiming for high-fidelity Mastra parity on the core runtime surface.

## Current Scope

This repository already ships a real Rust subset for:

- core agent, tool, workflow, and memory abstractions
- HTTP server routes for agents, tools, workflows, and memory
- Rust client SDK for the current server surface
- `mastra` CLI commands: `create`, `init`, `dev`, `start`, `routes`
- `create-mastra` starter scaffolding
- `mastracode` persistent headless runner

The current workspace does **not** claim full parity for the larger Mastra product surface. Still-structural gaps include:

- workflow `resume` / `resume-async` / `cancel` / time travel
- working memory / observational memory
- vectors / logs / telemetry
- CLI `build` / `studio` / `lint` / `scorers` / `migrate`
- full docs site, template library, and TUI-grade MastraCode UX

## Quick Start

```bash
cargo test --workspace
```

Generate a starter app:

```bash
cargo run -p create-mastra -- new ./demo-app
```

Or scaffold through the Rust CLI wrapper:

```bash
cargo run -p mastra-cli -- create demo-app --dir .
cargo run -p mastra-cli -- init --dir ./existing-app
```

Inspect the current server route surface:

```bash
cargo run -p mastra-cli -- routes
```

Run the headless MastraCode subset:

```bash
cargo run -p mastracode -- run --prompt "hello rust" --continue-latest --format json
```

## CLI Status

Current Rust CLI behavior is intentionally narrower than the upstream TypeScript CLI:

- `create` creates a new Rust starter under `<dir>/<project-name>` and defaults the project name to `mastra-app`
- `init` writes the same starter into an existing directory and aborts if `Cargo.toml` or `src/main.rs` already exists
- `routes` prints the current `mastra-server` route catalog
- `dev` and `start` bind an HTTP server on `127.0.0.1:4111` by default

Current parity gaps that are documented on purpose:

- `dev` parses `--dir`, `--env`, and `--debug`, but the current Rust implementation still serves `MastraHttpServer::new()` instead of loading a project graph from disk
- `start` parses `--dir` and `--env`, but does not yet boot from built `.mastra/output` artifacts
- upstream commands such as `build`, `studio`, `lint`, `scorers`, and `migrate` are not implemented yet

## Workspace Entry Points

- [`packages/core/README.md`](./packages/core/README.md)
- [`packages/server/README.md`](./packages/server/README.md)
- [`packages/cli/README.md`](./packages/cli/README.md)
- [`packages/create-mastra/README.md`](./packages/create-mastra/README.md)
- [`packages/memory/README.md`](./packages/memory/README.md)
- [`client-sdks/client-js/README.md`](./client-sdks/client-js/README.md)
- [`mastracode/README.md`](./mastracode/README.md)
- [`docs/reference`](./docs/reference)
- [`examples/README.md`](./examples/README.md)

## Validation

The main acceptance bar for this repository is:

- code compiles
- targeted route and client parity tests pass
- `cargo test --workspace` stays green
- docs only describe currently implemented behavior
