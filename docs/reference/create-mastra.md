# create-mastra Starter

`create-mastra` currently ships one built-in Rust starter.

## Generate

```bash
cargo run -p create-mastra -- new ./demo-app
```

## Starter Shape

- echo-based agent bootstrapped with `mastra-core`
- in-memory history using `mastra-memory`
- tracing initialization through `mastra-loggers`
- generated `README.md` and `.env.example`

## Boundary

This is not yet the full upstream template ecosystem with selectable templates and template metadata.
