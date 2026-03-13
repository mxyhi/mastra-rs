# mastracode

Persistent headless MastraCode subset for the Rust workspace.

## Usage

```bash
cargo run -p mastracode -- run --prompt "hello rust" --continue-latest --format json
```

Read prompt text from stdin:

```bash
printf 'summarize this repo' | cargo run -p mastracode -- run --prompt - --timeout 5
```

## Included Today

- persistent local memory at `~/.mastracode/memory.db`
- `--continue-latest` to resume the latest thread
- `--format default|json`
- `--timeout` with exit code `2` on timeout

## Runtime Notes

- the current runner uses `StaticModel::echo()`
- `--thread-id` pins an explicit thread instead of looking up the latest one
- `--resource-id` is forwarded into request context and persisted output

## Current Boundary

This crate is a headless runner. It is not yet the upstream interactive TUI product with OAuth, slash commands, or multi-provider UI flows.
