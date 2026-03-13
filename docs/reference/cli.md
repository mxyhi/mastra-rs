# CLI Reference

Current Rust CLI surface in this repository.

## Binaries

- `mastra-cli`: manifest-driven Rust wrapper for the current `mastra` command subset
- `create-mastra`: starter generator for a minimal Rust Mastra app
- `mastracode`: persistent headless runner

Examples in this repository use `cargo run -p ... -- ...` so they stay accurate without a separate install step.

## `mastra-cli`

```bash
cargo run -p mastra-cli -- --help
```

Supported commands today:

- `create`
- `init`
- `lint`
- `dev`
- `build`
- `start`
- `studio`
- `migrate`
- `scorers`
- `routes`

### `create`

Create a new starter project below `--dir`.

```bash
cargo run -p mastra-cli -- create demo-app --dir .
```

Current behavior:

- target path is `<dir>/<project-name>`
- `project-name` defaults to `mastra-app`
- fails if the target directory already exists
- delegates to the local `create-mastra` crate

### `init`

Scaffold the same starter in place.

```bash
cargo run -p mastra-cli -- init --dir ./existing-app
```

Current behavior:

- writes starter files into `--dir`
- fails if `Cargo.toml` or `src/main.rs` already exists
- does not merge into an existing Rust app

### `lint`

Validate a project manifest without starting a server.

```bash
cargo run -p mastra-cli -- lint --root ./demo-app --dir src/mastra
```

Current behavior:

- loads `src/mastra/mastra.json`
- accepts both the local single-file manifest and the `create-mastra` graph manifest with `schema_version`
- validates duplicate ids plus agent/workflow references
- reports a short project summary

### `dev`

```bash
cargo run -p mastra-cli -- dev --root ./demo-app --dir src/mastra --addr 127.0.0.1:4111
```

Current behavior:

- loads `.env.development`, `.env.local`, `.env`, then an optional custom `--env`
- loads request-context presets when `--request-context-presets` is provided
- reads the project manifest from `--root/--dir`
- builds a real `MastraHttpServer` from registered memories, tools, agents, and workflows
- still warns and ignores upstream-only flags such as `--tools`, `--inspect`, `--inspect-brk`, `--custom-args`, and `--https`

### `build`

```bash
cargo run -p mastra-cli -- build --root ./demo-app --dir src/mastra --studio
```

Current behavior:

- validates and normalizes the project manifest
- writes `.mastra/output/bundle.json`
- writes `.mastra/output/routes.txt`
- optionally writes `.mastra/output/studio/index.html` when `--studio` is enabled

### `start`

```bash
cargo run -p mastra-cli -- start --dir .mastra/output
```

Current behavior:

- loads `.env.production`, `.env`, then an optional custom `--env`
- reads `.mastra/output/bundle.json`
- boots the same runtime from the built bundle instead of reparsing source files
- still warns and ignores upstream-only `--custom-args`

### `studio`

```bash
cargo run -p mastra-cli -- studio --port 3000 --server-port 4111
```

Current behavior:

- serves a lightweight static HTML shell on `127.0.0.1:<port>`
- points the shell at the configured Mastra server URL
- can embed request-context preset JSON into the page
- is not yet the upstream Studio application

### `migrate`

```bash
cargo run -p mastra-cli -- migrate --root ./demo-app --dir src/mastra
```

Current behavior:

- loads the project manifest
- initializes every `libsql` memory referenced by the manifest
- validates connectivity by listing threads
- reports which memory ids were touched

### `scorers`

List built-in scorer templates or scaffold one into the project.

```bash
cargo run -p mastra-cli -- scorers list
cargo run -p mastra-cli -- scorers add answer-relevancy --root ./demo-app --dir src/mastra
```

Current behavior:

- `list` prints the built-in template catalog
- `add` writes a Rust scorer stub under `<root>/<dir>/scorers`

### `routes`

```bash
cargo run -p mastra-cli -- routes
```

Current behavior:

- prints one line per `mastra-server` route
- is the fastest way to inspect the current HTTP surface from the CLI layer

## `create-mastra`

```bash
cargo run -p create-mastra -- new ./demo-app
```

Generates a starter Rust app plus a graph manifest under `src/mastra/`. See [create-mastra](./create-mastra.md) for the generated file layout.

## `mastracode`

```bash
cargo run -p mastracode -- run --prompt "hello rust" --continue --format json
```

Current behavior:

- persists history into `~/.mastracode/memory.db`
- resumes the latest thread with `--continue`
- still accepts `--continue-latest` as a compatibility alias
- supports `--prompt -` to read stdin
- supports `--format default|json`
- supports `--timeout <seconds>` and exits with code `2` on timeout

See [mastracode](./mastracode.md) for details.

## Current Boundary

The Rust CLI now exposes the major command names, but several commands are intentionally slimmed-down implementations compared with upstream:

- `build` writes a normalized bundle and routes snapshot, not the upstream bundler output
- `studio` is a static shell, not the full Studio product
- `migrate` only initializes `libsql` memories
- `scorers` ships a tiny built-in template set
