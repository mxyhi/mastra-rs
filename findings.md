# Findings

## Reality Check

- 根工作区 `cargo fmt --all --check` 与 `cargo test --workspace` 全部通过，只能证明当前 Rust facade 自洽，不能证明已经达到 Mastra 1:1 复刻。
- 当前仓的顶层 crate 类别与参考仓 `.ref/mastra` 高度相似，但这只是 monorepo 拓扑层面的 parity，不是功能层面的 parity。
- 参考仓根目录还包含 `docs`、`examples`、`templates`、`e2e-tests`、`ee`、`communications`、`scripts`、`patches` 等完整产品资产；当前 Rust 仓没有这些等价面。

## Quantified Gap

- `packages/core`: Rust `9` files / `1687` LOC，参考 `783` files / `297704` LOC。
- `packages/memory`: Rust `5` files / `1075` LOC，参考 `32` files / `29565` LOC。
- `packages/server`: Rust `6` files / `1703` LOC，参考 `140` files / `43113` LOC。
- `packages/rag`: Rust `1` file / `187` LOC，参考 `64` files / `13501` LOC。
- `packages/mcp`: Rust `5` files，参考 `31` files。
- `packages/cli`: Rust `2` files，参考 `72` files。
- `packages/playground`: Rust 只有 manifest 数据结构，参考是完整 Studio UI。

## Core Gap

- `packages/core/src/lib.rs` 当前只导出 `Agent`、`Mastra`、`Memory`、`Model`、`Tool`、`Workflow` 等基础对象。
- 参考 `.ref/mastra/packages/core/src` 拥有 `a2a`、`action`、`auth`、`bundler`、`cache`、`datasets`、`deployer`、`di`、`editor`、`evals`、`events`、`features`、`harness`、`hooks`、`integration`、`loop`、`mcp`、`observability`、`processors`、`relevance`、`server`、`storage`、`stream`、`tts`、`vector`、`voice`、`workspace` 等完整子系统。
- 当前 Rust `Mastra` 运行时只注册 `agents/tools/workflows/memory` 四类对象；参考 TS `Mastra` 还包含 `storage/vectors/logger/tts/observability/deployer/server/mcpServers/pubsub/scorers/processors/workspace/gateways/events/editor` 等大块能力。
- 结论：当前 Rust core 是最小 runtime primitive，不是 `@mastra/core` 的同级实现。

## Memory Gap

- `packages/memory/src/lib.rs` 当前主要是 thread/message store facade 与 `MemoryEngine` bridge。
- 参考 `.ref/mastra/packages/memory/src/index.ts` 包含 working memory、observational memory、semantic recall、vector recall、processors、memory tools 等能力。
- 结论：当前 Rust memory 只是最小 memory store，不是完整 Mastra memory product layer。

## Server Gap

- `packages/server/src` 当前只覆盖 `health/routes/agents/memory/workflows` 这些最小 HTTP 路由。
- 参考 `.ref/mastra/packages/server/src/server` 拥有 `handlers`、`server-adapter`、`a2a`、`auth`、`schemas` 等分层，并覆盖 observability、vector、voice、workspace、tools、scores、mcp 等域。
- 结论：当前 Rust server 只是最小 HTTP facade，不等价于 `@mastra/server`。

## CLI and MastraCode Gap

- `packages/cli/src/main.rs` 当前只有 `serve` 与 `routes` 两个子命令。
- 参考 `.ref/mastra/packages/cli/src/index.ts` 具备 `create`、`init`、`lint`、`dev`、`build`、`start`、`studio`、`migrate`、`scorers` 以及 analytics/template/skills/MCP 等完整 tooling 面。
- `mastracode/src/lib.rs` 当前仍是 headless echo runner；参考 `.ref/mastra/mastracode/src` 拥有 TUI、auth、hooks、IPC、LSP、MCP manager、workspace、modes、subagents、permissions、settings/onboarding 等完整体系。
- 结论：CLI 与 MastraCode 都远未达到产品级 parity。

## RAG / MCP / Playground Gap

- `packages/rag/src/lib.rs` 当前只提供 `MDocument` 与 chunking；参考版还包括 `document`、`rerank`、`GraphRAG`、`tools`、`utils/default-settings`。
- `packages/mcp/src` 当前只有极小本地 client/server facade；参考版拆成 `client`、`server`、`shared`、fixtures 与多 transport/protocol 能力。
- `packages/playground/src/lib.rs` 当前只是 manifest/route 数据结构；参考版 `packages/playground/src/App.tsx` 是完整 Studio UI。

## Current Status

- 当前仓可以称为 `broad monorepo parity scaffold with partial runtime primitives`。
- 当前仓不能被诚实地描述为 “Mastra 1:1 完成”。
