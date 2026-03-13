# CLI Reference

Current Rust CLI surface in this repository.

## Binaries

- `mastra-cli`: Rust wrapper that exposes the current `mastra` command subset
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
- `dev`
- `start`
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

### `dev`

```bash
cargo run -p mastra-cli -- dev --addr 127.0.0.1:4111
```

Parsed flags:

- `--addr`
- `--dir` with current default `src/mastra`
- `--env`
- `--debug`

Current behavior:

- starts `MastraHttpServer::new()` on the requested address
- does not yet load a project definition from `--dir`
- does not yet apply `--env` loading or `--debug` runtime changes
- does not expose upstream Studio or build pipeline behavior

### `start`

```bash
cargo run -p mastra-cli -- start --dir .mastra/output
```

Parsed flags:

- `--addr`
- `--dir` with current default `.mastra/output`
- `--env`

Current behavior:

- starts the same server wrapper used by `dev`
- reports the chosen output directory in the startup banner
- does not yet boot from built production artifacts under `.mastra/output`

### `routes`

```bash
cargo run -p mastra-cli -- routes
```

Current behavior:

- prints one line per `mastra-server` route
- is the most reliable way to inspect the current HTTP surface from the CLI layer

## `create-mastra`

```bash
cargo run -p create-mastra -- new ./demo-app
```

Generates:

- `Cargo.toml`
- `src/main.rs`
- `README.md`
- `.env.example`

See [create-mastra](./create-mastra.md) for the generated starter shape and current limits.

## `mastracode`

```bash
cargo run -p mastracode -- run --prompt "hello rust" --continue-latest --format json
```

Current behavior:

- persists history into `~/.mastracode/memory.db`
- can resume the latest thread with `--continue-latest`
- supports `--prompt -` to read stdin
- supports `--format default|json`
- supports `--timeout <seconds>` and exits with code `2` on timeout

See [mastracode](./mastracode.md) for details.

## Pending CLI Alignment

The following upstream CLI commands are still outside the current Rust subset:

- `build`
- `studio`
- `lint`
- `scorers`
- `migrate`

The following command semantics are intentionally documented as pending because the parser already exists but the runtime side is not fully wired yet:

- `mastra-cli dev --dir/--env/--debug`
- `mastra-cli start --dir/--env`
