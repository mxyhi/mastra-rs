# Examples

Current runnable examples for the Rust Mastra subset.

## Core Agent Example

```bash
cargo run -p mastra-core --example minimal_agent
```

## Server Example

```bash
cargo run -p mastra-server --example minimal_server
```

## Generated Starter

```bash
cargo run -p create-mastra -- new ./demo-app
cd demo-app
cargo run
```

## MastraCode Headless

```bash
cargo run -p mastracode -- run --prompt "hello rust" --continue
```
