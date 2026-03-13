# MastraCode Headless Reference

Current Rust MastraCode subset:

```bash
cargo run -p mastracode -- --prompt "hello rust" --continue --format json
```

## Entry Point

This repository now supports both implemented headless entry shapes:

- `mastracode --prompt ...`
- `mastracode run --prompt ...`

It is still deliberately narrower than upstream MastraCode because the default
interactive TUI path is absent.

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
- storage is a single local libsql file for this Rust port, not the upstream
  project-scoped thread + auth/settings/plans/config directory model

## Boundary

This is a headless persistence-focused subset, not the full upstream interactive MastraCode application.

Today it still runs a fixed echo model through `StaticModel::echo()` and does not yet cover:

- provider or gateway selection
- `.mastracode` project/global settings
- project-scoped thread registries and app-data config files
- interactive TUI flows
- slash commands
- OAuth or editor integrations

Examples and reference snippets in this Rust workspace intentionally keep using
the top-level `--prompt` form because it now matches the implemented headless
entry point most closely.

## Pending Alignment Notes

Document these once the mainline implementation lands:

- model/provider configuration surface
- project-local versus global MastraCode config files
- real model routing instead of the current `StaticModel::echo()` backend
