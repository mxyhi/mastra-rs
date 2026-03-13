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
cargo run -p mastra-cli -- init --dir ./demo-app
```

## Current Boundary

The Rust CLI does not yet implement the broader upstream product commands:

- `build`
- `studio`
- `lint`
- `scorers`
- `migrate`
