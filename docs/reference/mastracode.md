# MastraCode Headless Reference

Current Rust MastraCode subset:

```bash
cargo run -p mastracode -- run --prompt "hello rust" --continue --format json
```

## Entry Point Difference

This repository only implements the headless `run --prompt ...` path. That is
deliberately narrower than upstream MastraCode, which defaults to an
interactive TUI and also supports top-level headless entry via
`mastracode --prompt ...`.

## Flags

- `--prompt <text>`
- `--prompt -` to read from stdin
- `--continue`
- `--thread-id <id>`
- `--resource-id <id>`
- `--format default|json`
- `--timeout <seconds>`

## Persistence

- default storage path: `~/.mastracode/memory.db`
- latest-thread reuse via `--continue`
- `--continue-latest` remains accepted as a compatibility alias
- timeout exits with code `2`

## Boundary

This is a headless persistence-focused subset, not the full upstream interactive MastraCode application.

Today it still runs a fixed echo model through `StaticModel::echo()` and does not yet cover:

- provider or gateway selection
- `.mastracode` project/global settings
- interactive TUI flows
- slash commands
- OAuth or editor integrations

Examples and reference snippets in this Rust workspace intentionally keep using
`cargo run -p mastracode -- run --prompt ...` so they match the implemented
entry point instead of the upstream product UX.

## Pending Alignment Notes

Document these once the mainline implementation lands:

- model/provider configuration surface
- project-local versus global MastraCode config files
- real model routing instead of the current `StaticModel::echo()` backend
