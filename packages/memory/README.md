# mastra-memory

Thread, message, working-memory, and observation storage for the current Rust Mastra subset.

## Included Today

- create, get, update, list, clone, and delete threads
- append, list, and delete messages
- get and update working memory in thread or resource scope
- append and list observations, including `observed_message_ids`
- new threads inherit resource-scoped working memory for the same resource
- cloned threads copy working memory and remap observation message ids
- pagination and ordering support
- bridge to storage backends that implement the memory store trait

## CLI And Scaffolding Touchpoints

- the `create-mastra` starter uses `Memory::in_memory()` for its zero-config sample app
- `mastracode` uses this crate with a libsql backend to persist local thread history across runs

## Notes

Current memory parity in this crate is a manual data-plane subset:

- it stores and retrieves working memory state
- it stores and paginates observations
- it mirrors those records through the `mastra-core` memory-engine bridge

Still outside the current subset:

- semantic recall or vector search
- upstream automatic working-memory update tooling
- upstream observer/reflector background processors and buffering
