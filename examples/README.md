# Examples

Current smoke paths for the Rust Mastra subset.

These commands are meant to prove that the implemented Rust entry points build,
boot, and round-trip. They are not a claim of full upstream feature parity or a
catalog of production-ready examples.

## Core Agent Example

```bash
cargo run -p mastra-core --example minimal_agent
```

## Server Example

```bash
cargo run -p mastra-server --example minimal_server
```

## Generated Starter Smoke

```bash
cargo run -p create-mastra -- new ./demo-app
cd demo-app
cargo run -p mastra-cli -- lint --root . --dir src/mastra
cargo run -p mastra-cli -- build --root . --dir src/mastra --studio
cargo run
```

The generated graph only exercises the currently supported starter subset:
path-referenced memories/tools/agents/workflows, echo-style demo models, and
`identity|static_json|tool|agent` workflow steps.

To inspect the built starter graph and route snapshot:

```bash
cat .mastra/output/routes.txt
cat .mastra/output/bundle.json
```

## MastraCode Headless Smoke

```bash
cargo run -p mastracode -- run --prompt "hello rust" --continue
```

This validates the Rust headless entry only. It does not cover the upstream
MastraCode TUI, top-level `--prompt` flow, OAuth onboarding, or provider/model
management UX.
