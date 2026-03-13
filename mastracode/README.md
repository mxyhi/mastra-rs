# mastracode

Persistent headless MastraCode subset for the Rust workspace.

## Usage

```bash
cargo run -p mastracode -- --prompt "hello rust" --continue --format json
```

Read prompt text from stdin:

```bash
printf 'summarize this repo' | cargo run -p mastracode -- --prompt - --timeout 5
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
- `run --prompt ...` is still accepted as a compatibility entry shape

## Current Boundary

This crate is a headless runner. It is not yet the upstream interactive TUI product with OAuth, slash commands, or multi-provider UI flows.

Compared with upstream Mastra Code today:

- top-level headless mode now matches upstream `--prompt` entry
- `--continue-latest` remains an extra compatibility alias for older Rust-port docs
- upstream TUI/headless stack supports model packs, custom OpenAI-compatible providers, OAuth, and API-key-based model routing
- this Rust port does not yet consume `.mastracode` config files or provider API keys for model resolution
- this Rust port does not yet implement the upstream project-scoped thread registry or app-data auth/settings/plans/config files
- the implemented headless path still executes through a fixed `StaticModel::echo()` backend
