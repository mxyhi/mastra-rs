# CLI Reference

Current Rust CLI surface in this repository.

## `mastra`

```bash
cargo run -p mastra-cli -- routes
```

Supported commands:

- `create`
- `init`
- `dev`
- `start`
- `routes`

## `create-mastra`

```bash
cargo run -p create-mastra -- new ./demo-app
```

Generates:

- `Cargo.toml`
- `src/main.rs`
- `README.md`
- `.env.example`

## Not Yet Implemented

The following upstream CLI commands are still outside the current Rust subset:

- `build`
- `studio`
- `lint`
- `scorers`
- `migrate`
