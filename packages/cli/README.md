# mastra-cli

Rust CLI wrapper for the currently implemented server and scaffolding subset.

## Commands

- `mastra create`
- `mastra init`
- `mastra dev`
- `mastra start`
- `mastra routes`

## Usage

```bash
cargo run -p mastra-cli -- routes
```

```bash
cargo run -p mastra-cli -- create demo-app --dir .
```

```bash
cargo run -p mastra-cli -- init --dir ./demo-app
```

```bash
cargo run -p mastra-cli -- dev --addr 127.0.0.1:4111
```

```bash
cargo run -p mastra-cli -- start --dir .mastra/output
```

## Current Behavior

- `create` creates `<dir>/<project-name>` through the local `create-mastra` crate
- `init` writes the same starter into an existing directory and fails fast if a Rust starter is already present
- `routes` prints `mastra-server` route descriptions
- `dev` and `start` both serve the current `MastraHttpServer` wrapper

## Parsed But Not Fully Wired Yet

The parser already accepts these flags, but the current runtime does not fully consume them yet:

- `dev --dir --env --debug`
- `start --dir --env`

In particular, the current `dev` and `start` subcommands do not yet load a project graph from disk or boot from built `.mastra/output` artifacts.

## Current Boundary

The Rust CLI does not yet implement the broader upstream product commands:

- `build`
- `studio`
- `lint`
- `scorers`
- `migrate`
