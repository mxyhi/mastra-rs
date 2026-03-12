# Task Plan

## Goal
在一次连续 rollout 中尽可能把 `mastra-rs` 推到接近 `1:1 made mastra in rust` 的完成态：消灭剩余模板级 placeholder crate，保持全仓可编译、可测试、可提交，并在过程中自动形成里程碑 commit。

## Constraints
- 用户要求一次性完成，不分批停下汇报。
- 复杂任务必须维护 `task_plan.md`、`findings.md`、`progress.md`。
- 允许 break old formats，不做向后兼容包袱。
- 默认最小改动面，但不为维持占位实现而保守。
- 多 agent 并行，但写集必须互斥，避免互相踩文件。

## Phase Breakdown
- [in_progress] Phase 1: 盘点剩余 placeholder crate，并映射到 `.ref/mastra` 参考实现。
- [in_progress] Phase 2: 收口当前已脏但未提交的 `observability` cluster，并验证 targeted tests。
- [in_progress] Phase 3: 并行实现剩余 cluster：
  - `auth/*`
  - `stores/*`
  - `voice/*` + `workspaces/*`
  - `misc packages` / `integrations` / `explorations` / test-utils
- [pending] Phase 4: 整体集成，反复执行 `cargo test --workspace` 收敛编译与接口漂移。
- [pending] Phase 5: 形成一个或多个自动 commit，并给出真实完成度边界。

## Current Facts
- 已完成并提交的里程碑：
  - `4ec4939 chore(plan): capture mastra-rs execution baseline`
  - `820db0a feat(runtime): add mcp pubsub and inngest primitives`
  - `da50541 feat(runtime): add sdk auth deployer and observability primitives`
- 当前未提交脏改动集中在：
  - `observability/arize`
  - `observability/braintrust`
  - `observability/laminar`
  - `observability/otel-bridge`
  - `observability/otel-exporter`
- `rg -l "fn add\\(|it_works\\(" --glob '!target'` 当前仍命中大量 crate，说明 parity 仍未完成。

## Parallel Ownership
- 主线程：`observability` 剩余 cluster、总集成、全仓回归、commit、planning files。
- Agent Dirac: `auth/**`
- Agent Halley: `stores/**`
- Agent Wegener: `voice/**` + `workspaces/**`
- Agent Hilbert: `misc packages` / `integrations` / `explorations` / test-utils

## Risks
- 并行 agent 可能在 targeted test 通过后再次改坏接口，必须由主线程做最终全仓回归。
- `.ref/mastra` 是 TypeScript monorepo；Rust 实现只能抽取可复用语义，不可能字面复制所有 JS runtime 细节。
- 如果剩余 crate 数量和语义面过大，本轮可能做到“全仓无 placeholder + 测试全绿”，但仍需如实说明与 upstream 真实功能面的差距。
