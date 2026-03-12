# mastra-rs Task Plan

## Goal

一次性交付一个可运行、可测试、尽量贴近上游 Mastra 的 Rust 版本，并在过程中自动提交里程碑 commit。

## Current Facts

- 当前仓库已按上游 monorepo 目录展开，但大量 crate 仍是占位实现。
- 已有较实质代码的区域集中在：
  - `packages/core`
  - `packages/server`
  - `packages/memory`
  - `stores/libsql`
  - `stores/pg`
  - `mastracode`
- 大量子系统仅有 14 行左右空壳 `lib.rs`，包括 `auth/*`、`client-sdks/*`、`deployers/*`、多数 `stores/*`、多数 `voice/*`、`workflows/*`、`workspaces/*`、`pubsub/*`。

## Constraints

- 目标是高性能、简洁、可维护，不做无意义兼容层。
- 复杂逻辑必须加注释。
- 需要多 agent 并行推进。
- 需要里程碑 commit。
- 需要测试先行，至少核心行为遵循 TDD。

## Execution Phases

### Phase 1: Baseline and Gap Analysis

- [in_progress] 固化仓库现状、上游结构、风险和验证口径。
- [pending] 从 `.ref/mastra` 和官方 Mastra 文档提取核心子系统边界。
- [pending] 形成优先级：核心运行时 > 服务与 CLI > 持久化与适配器 > 长尾 provider。

### Phase 2: Core Runtime Closure

- [pending] 审核并补强 `packages/core` 的 agent/tool/workflow/model/memory 契约。
- [pending] 审核并补强 `packages/server` 的 HTTP 契约、运行时注册表和异步执行链路。
- [pending] 以 `packages/memory` 和 `stores/libsql` 建立真实持久化闭环。
- [pending] 为核心闭环增加失败测试并补实现。

### Phase 3: Parallel Crate Enablement

- [pending] `server-adapters/*`：把核心 server 路由能力映射成各框架适配器。
- [pending] `auth/*`、`observability/*`：建立统一 trait 和最小真实 provider。
- [pending] `stores/*`：优先将空壳改为统一 provider 包装或显式 unsupported 占位错误，而不是假实现。
- [pending] `voice/*`、`workflows/*`、`workspaces/*`、`client-sdks/*`、`deployers/*`、`pubsub/*`：按上游职责补最小真实能力。

### Phase 4: CLI and Developer Experience

- [pending] 补强 `mastracode` 和 `packages/cli` 的运行入口、配置读取和注册机制。
- [pending] 添加示例或 smoke 测试，验证端到端使用路径。

### Phase 5: Verification and Finalization

- [pending] 运行 `cargo test --workspace`。
- [pending] 运行必要的 targeted smoke tests / examples。
- [pending] 汇总未完成边界；若仍有缺口，继续补齐直到核心目标达成或出现硬阻塞。

## Commit Plan

- [pending] `chore(plan): capture mastra-rs execution baseline`
- [pending] `feat(core): close runtime and server loop`
- [pending] `feat(adapters): enable framework and provider crates`
- [pending] `feat(cli): wire mastracode and developer entrypoints`
- [pending] `test(workspace): stabilize workspace verification`

## Risks

- 上游 Mastra 是跨多语言、多 provider、多产品面的巨型 monorepo，严格“1:1 完整复刻”远大于一次常规 feature 开发。
- 部分上游能力依赖 JS/TS 生态和外部 SaaS，Rust 侧需要重新定义边界。
- 若参考仓接口近期变动，必须以当前文档/源码为准，不能凭旧记忆实现。
