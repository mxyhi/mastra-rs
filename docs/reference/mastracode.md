# MastraCode Headless Reference

Current Rust MastraCode subset:

```bash
cargo run -p mastracode -- run --prompt "hello rust" --continue-latest --format json
```

## Flags

- `--prompt <text>`
- `--prompt -` to read from stdin
- `--continue-latest`
- `--thread-id <id>`
- `--resource-id <id>`
- `--format default|json`
- `--timeout <seconds>`

## Persistence

- default storage path: `~/.mastracode/memory.db`
- latest-thread reuse via `--continue-latest`
- timeout exits with code `2`

## Boundary

This is a headless persistence-focused subset, not the full upstream interactive MastraCode application.

Today it still runs a fixed echo model through `StaticModel::echo()` and does not yet cover:

- provider or gateway selection
- interactive TUI flows
- slash commands
- OAuth or editor integrations

## Pending Alignment Notes

Document these once the mainline implementation lands:

- model/provider configuration surface
- project-local versus global MastraCode config files
- any future command aliases if `--continue` is added as a stable compatibility flag
