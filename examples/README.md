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
cargo run -p mastra-cli -- lint --root . --dir src/mastra
cargo run -p mastra-cli -- build --root . --dir src/mastra --studio
cargo run
```

To inspect the built starter graph and route snapshot:

```bash
cat .mastra/output/routes.txt
cat .mastra/output/bundle.json
```

## MastraCode Headless

```bash
cargo run -p mastracode -- run --prompt "hello rust" --continue
```
