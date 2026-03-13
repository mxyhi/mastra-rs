# mastra-cli

Rust CLI wrapper for the currently implemented server and scaffolding subset.

## Commands

- `mastra create`
- `mastra init`
- `mastra lint`
- `mastra dev`
- `mastra build`
- `mastra start`
- `mastra studio`
- `mastra migrate`
- `mastra scorers`
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
cargo run -p mastra-cli -- lint --root ./demo-app --dir src/mastra
```

```bash
cargo run -p mastra-cli -- dev --root ./demo-app --dir src/mastra --addr 127.0.0.1:4111
```

```bash
cargo run -p mastra-cli -- build --root ./demo-app --dir src/mastra --studio
```

```bash
cargo run -p mastra-cli -- start --dir .mastra/output
```

```bash
cargo run -p mastra-cli -- studio --port 3000 --server-port 4111
```

```bash
cargo run -p mastra-cli -- scorers list
```

## Current Behavior

- `create` creates `<dir>/<project-name>` through the local `create-mastra` crate
- `init` writes the same starter into an existing directory and fails fast if a Rust starter is already present
- `lint` validates ids and references in either the local single-file manifest or the `create-mastra` graph manifest
- `dev` loads a project graph from disk, registers it into a real `MastraHttpServer`, and serves it
- `build` writes `.mastra/output/bundle.json` plus `routes.txt`, with an optional static Studio shell
- `start` boots the same runtime from the built bundle under `.mastra/output`
- `studio` serves a lightweight HTML shell pointed at a configurable server URL
- `migrate` initializes each `libsql` memory declared in the manifest
- `scorers` lists built-in templates or writes a scorer stub into `src/mastra/scorers`
- `routes` prints `mastra-server` route descriptions

## Current Boundary

- `build` is a normalized bundle writer, not the upstream bundler pipeline
- `studio` is a static shell, not the upstream Studio application
- `migrate` only touches `libsql` memories
- several upstream-only flags are parsed and warned about but intentionally ignored
