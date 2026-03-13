# mastra-rs

Rust workspace aiming for high-fidelity Mastra parity on the core runtime surface.

## Current Scope

This repository already ships a real Rust subset for:

- core agent, tool, workflow, and memory abstractions
- HTTP server routes for agents, tools, workflows, and memory
- Rust client SDK for the current server surface
- `mastra` CLI commands: `create`, `init`, `lint`, `dev`, `build`, `start`, `studio`, `migrate`, `scorers`, `routes`
- `create-mastra` starter scaffolding
- `mastracode` persistent headless runner

The current workspace does **not** claim full parity for the larger Mastra product surface. Still-structural gaps include:

- workflow time travel
- working memory / observational memory
- vectors / logs / telemetry
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

Validate and build the generated project graph:

```bash
cargo run -p mastra-cli -- lint --root ./demo-app --dir src/mastra
cargo run -p mastra-cli -- build --root ./demo-app --dir src/mastra --studio
```

Inspect the current server route surface:

```bash
cargo run -p mastra-cli -- routes
```

Run the headless MastraCode subset:

```bash
cargo run -p mastracode -- run --prompt "hello rust" --continue --format json
```

## CLI Status

Current Rust CLI behavior is intentionally narrower than the upstream TypeScript CLI:

- `create` creates a new Rust starter under `<dir>/<project-name>` and defaults the project name to `mastra-app`
- `init` writes the same starter into an existing directory and aborts if `Cargo.toml` or `src/main.rs` already exists
- `lint` validates either a single-file manifest or the `create-mastra` graph manifest
- `dev` loads the project graph from disk and serves a real `MastraHttpServer`
- `build` writes `.mastra/output/bundle.json`, `routes.txt`, and an optional static Studio shell
- `start` boots from the built bundle under `.mastra/output`
- `studio` serves a lightweight HTML shell wired to the configured server URL
- `migrate` initializes `libsql` memories declared in the manifest
- `scorers` lists built-in templates or scaffolds one into `src/mastra/scorers`
- `routes` prints the current `mastra-server` route catalog
- `dev` and `start` bind an HTTP server on `127.0.0.1:4111` by default

Current parity gaps that are documented on purpose:

- `build` is still a normalized bundle writer, not the upstream bundler/runtime pipeline
- `studio` is still a static shell, not the upstream Studio application
- `migrate` is limited to `libsql`-backed memories
- upstream-only flags like `--inspect`, `--custom-args`, or `--https` are warned about and ignored

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
