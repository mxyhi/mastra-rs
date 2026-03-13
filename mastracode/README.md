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
- `--continue` / `-c` to resume the latest thread
- `--format default|json`
- `--timeout` with exit code `2` on timeout
- a single local storage file, not the upstream project-scoped thread/config layout

## Runtime Notes

- the current runner uses `StaticModel::echo()`
- `--thread-id` pins an explicit thread instead of looking up the latest one
- `--resource-id` is forwarded into request context and persisted output
- `--continue-latest` is still accepted as a compatibility alias for older Rust-port docs

## Current Boundary

This crate is a headless runner. It is not yet the upstream interactive TUI product with OAuth, slash commands, or multi-provider UI flows.

Compared with upstream Mastra Code today:

- upstream headless mode uses `--continue`, not `--continue-latest`
- upstream TUI/headless stack supports model packs, custom OpenAI-compatible providers, OAuth, and API-key-based model routing
- this Rust port does not yet consume `.mastracode` config files or provider API keys for model resolution
- this Rust port does not yet implement the upstream project-scoped thread registry or app-data auth/settings/plans/config files
- `run` still executes through a fixed `StaticModel::echo()` backend
