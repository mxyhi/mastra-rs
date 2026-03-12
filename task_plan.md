# Task Plan

## Goal

基于当前 `mastra-rs` 工作树与参考仓 `.ref/mastra`，验证“Mastra 已经被 1:1 复刻到 Rust”是否成立；若不成立，则把真实缺口与当前完成边界记录清楚，避免后续继续建立在错误前提上。

## Constraints

- 用户要求一次性完成，不分批停下。
- 过程中自动生成 commit，并行协调多个 agent。
- `task_plan.md`、`findings.md`、`progress.md` 仅由主线程维护。
- 结论必须以本地源码、`.ref/mastra`、官方 Mastra 文档与 DeepWiki 证据为准。
- 不能把“workspace 测试通过”误报为“1:1 复刻完成”。

## Parallel Ownership

- 主线程：真实性审计、planning files、集成结论、commit。
- James：目录与 coverage audit。
- Maxwell：核心 crate 完成度审计。
- Linnaeus：参考仓规模与 facade/skeleton 判断。

## Phases

1. 审计当前 Rust workspace、git 历史与 planning files，确认现状是否与“已完成 parity”一致。
2. 对比 `.ref/mastra` 与官方文档，生成按严重度排序的缺口清单。
3. 判断缺口是否属于可在当前回合一次性完成的实现规模。
4. 若存在可闭环范围，则按 cluster 并行实现并生成阶段性 commit。
5. 执行 `cargo fmt --all --check` 与 `cargo test --workspace`，更新 planning files。
6. 生成审计 commit，固定真实执行基线。

## Status

- [x] Phase 1: 已确认旧 planning 叙述失真，当前仓库不能直接认定为“1:1 parity 已完成”。
- [x] Phase 2: 已完成目录、文件数、LOC、核心导出面与关键源码结构的对比。
- [x] Phase 3: 结论已明确，差距规模远超“本轮一次性补齐”的现实范围。
- [x] Phase 4: 已停止基于错误完成假设的伪闭环编码。
- [x] Phase 5: `cargo fmt --all --check` 与 `cargo test --workspace` 已通过。
- [x] Phase 6: 已生成审计 commit `78c7253 chore(plan): record parity reality check`。

## Evidence Anchors

- 当前 `packages/core/src` 只有 `9` 个 Rust 文件，而参考 `.ref/mastra/packages/core/src` 有 `783` 个 TS 源文件。
- 当前 `packages/memory/src` `5` 个文件，对应参考 `32` 个。
- 当前 `packages/server/src` `6` 个文件，对应参考 `140` 个。
- 当前 `packages/rag/src` `1` 个文件，对应参考 `64` 个。
- 当前 `packages/cli/src` `2` 个文件，对应参考 `72` 个。
- 当前 `mastracode/src/lib.rs` 仍是 headless echo runner，不是上游 `mastracode` 的 TUI/auth/mcp/lsp/hooks/subagents 体系。

## Current Judgment

- 当前仓更准确的描述是 `broad monorepo parity scaffold with partial runtime primitives`。
- 当前仓不能被诚实描述为 “Mastra 1:1 完成”。

## Commit Checkpoints

- `820db0a feat(runtime): add mcp pubsub and inngest primitives`
- `da50541 feat(runtime): add sdk auth deployer and observability primitives`
- `43546e8 feat(observability): add otel and provider exporters`
- `67dfacd feat(auth): add provider wrappers for auth crates`
- `1c94530 feat(parity): add provider metadata and tooling clusters`
- `4a2201f chore(fmt): normalize auth and observability formatting`
- `78c7253 chore(plan): record parity reality check`
