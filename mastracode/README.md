# mastracode

Persistent headless MastraCode subset for the Rust workspace.

## Usage

```bash
cargo run -p mastracode -- run --prompt "hello rust" --continue --format json
```

Read prompt text from stdin:

```bash
printf 'summarize this repo' | cargo run -p mastracode -- run --prompt - --timeout 5
```

## Included Today

- persistent local memory at `~/.mastracode/memory.db`
- `--continue` to resume the latest thread
- `--format default|json`
- `--timeout`

## Current Boundary

This crate is a headless runner. It is not yet the upstream interactive TUI product with OAuth, slash commands, or multi-provider UI flows.
