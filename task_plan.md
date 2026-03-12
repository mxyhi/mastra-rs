# Task Plan

## Goal
在单次交付内尽可能推进 `mastra-rs` toward upstream Mastra parity，并把当前已经脱离占位实现的子系统全部做实、验证、提交。

## Constraints
- 用户要求一次性完成，不分批停止；但结论必须尊重仓库现状，不能把占位 crate 误报成 1:1 完成。
- 默认最小改动面，优先收口已有半成品与现成测试覆盖的模块。
- 复杂任务持续维护 `task_plan.md` / `findings.md` / `progress.md`，记录命令、失败和修复。
- 允许自动协调多个 agent，但要防止并行改写同一文件造成回归。

## Phases
- [complete] Phase 1: 盘点当前工作树、已有提交、SDK/auth/runtime 模块测试状态。
- [complete] Phase 2: 收口 `packages/mcp`、`workflows/inngest`、`pubsub/google-cloud-pubsub`、`packages/auth`、`client-sdks/*`、`packages/deployer`、`deployers/*`、`observability/{mastra,datadog,langfuse,langsmith,posthog,sentry}`。
- [in_progress] Phase 3: 稳定并行开发结果，修复全仓回归中暴露的接口/语法漂移。
- [pending] Phase 4: 提交本轮实现并给出真实完成度边界与剩余占位面。

## Verified Implementations
- `packages/mcp`: 本地 transport、tool/resource/prompt catalog、agent/workflow MCP tool 暴露。
- `workflows/inngest`: 事件绑定与 workflow dispatch runtime。
- `pubsub/google-cloud-pubsub`: 内存版 topic/subscription/publish/pull/ack 行为。
- `packages/auth`: bearer token 提取、session resolver、OIDC/JWK 验证。
- `client-sdks/client-js`: agent/workflow/memory 路由客户端。
- `client-sdks/ai-sdk`: AI SDK 风格 request/response 适配。
- `client-sdks/react`: `ChatAction` / `ChatState` / `ChatController`。
- `packages/deployer` + `deployers/{cloud,cloudflare,netlify,vercel}`: bundle artifact/materialization + provider plan。
- `observability/mastra` + provider exporters: HTTP exporter core + Datadog/Langfuse/LangSmith/PostHog/Sentry request builders。

## Known Gaps
- `rg -l "fn add\\(|it_works\\(" --glob '!target' | wc -l` 当前仍命中 `78` 个 crate，说明仓库距离“全仓 1:1 parity”仍有明显差距。
- 这些占位面主要分布在 `auth/*` provider wrappers、`stores/*`、`voice/*`、`workspaces/*`、若干 `packages/*` / `observability/*` / `integrations/*`。
- `cargo test --workspace` 的绿色状态不能直接等价为功能对齐，因为大量 crate 仍只保留 `it_works` 级测试。

## Risks
- 并行 agent 改写同一文件会造成“定向测试已过，但全仓回归再次失败”的漂移。
- 当前 workspace 中大量 crate 仍是占位实现，继续宣称“1:1 完成”会违反事实约束。

## Rollback
- 已有基线提交：`4ec4939 chore(plan): capture mastra-rs execution baseline`
- 已有运行时里程碑提交：`820db0a feat(runtime): add mcp pubsub and inngest primitives`
- 本轮收口完成后追加新提交，不重写历史。
