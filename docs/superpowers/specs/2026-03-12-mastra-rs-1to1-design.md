# mastra-rs 1:1 复刻设计

## 背景

当前仓库目标是把 `mastra` monorepo 的核心能力以 Rust workspace 形式复刻出来。用户要求一次性推进，不采用分批确认，因此本设计默认已获执行批准。

## 目标

1. 以 `.ref/mastra` 为参考，建立 Rust workspace 与上游能力的映射。
2. 保证 `packages/core`、`packages/server`、`packages/cli`、`packages/memory` 等核心包闭环可运行。
3. 对外围适配层做必要补齐，使当前仓库已纳入 workspace 的 crate 行为尽量与上游同构。
4. 通过测试与定向验证形成可复验证据。

## 非目标

- 在单次会话内重写上游所有 docs、examples、前端站点和发布流水线。
- 为了兼容旧格式而保留不合理抽象；本仓允许破坏式前进。

## 架构策略

### 核心层

- `packages/core` 负责 Agent、Model、Tool、Workflow、Memory 等统一抽象。
- `packages/server` 负责将核心抽象暴露为 HTTP 路由和流式契约。
- `packages/cli` / `mastracode` 负责本地运行入口与开发者体验。

### 兼容层

- `auth/*`、`stores/*`、`voice/*`、`observability/*`、`workspaces/*`、`deployers/*` 提供与上游包族相同的概念边界。
- 每个 crate 只保留单一职责，避免把上游 monorepo 的复杂耦合直接照搬进 Rust。

### 验证策略

1. 先建立差距清单。
2. 按 TDD 为关键缺口补失败测试。
3. 补最小实现直到通过。
4. 以稳定里程碑生成 commit。

## 风险

- 上游 monorepo 规模很大，当前仓库可能只覆盖其子集；需要谨慎区分“未纳入范围”和“实现缺口”。
- 某些外围 crate 可能是概念对齐而非逐 API 对齐，需要优先保证核心运行面。

## 验收

- 关键 crate 测试通过。
- 核心运行路径可验证。
- 差距清单被收敛并体现在提交记录里。
