# mastra-memory

Thread and message management for the Rust Mastra subset.

## Included Today

- create, get, update, list, clone, and delete threads
- append, list, and delete messages
- pagination and ordering support
- bridge to storage backends that implement the memory store trait

## Notes

Current parity work in this crate is focused on durable thread/message history. Working memory and observational memory remain outside the current subset.
