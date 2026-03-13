# MastraCode Headless Reference

Current Rust MastraCode subset:

```bash
cargo run -p mastracode -- run --prompt "hello rust" --continue --format json
```

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

## Boundary

This is a headless persistence-focused subset, not the full upstream interactive MastraCode application.
