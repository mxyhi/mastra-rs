# Findings

## 2026-03-12

### Current Verified Surface

- `cargo test -p mastra-packages-auth` 通过，`packages/auth` 现已具备 bearer 解析、session callback、OIDC/JWK 验证能力。
- `cargo test -p mastra-client-sdks-client-js` 通过，`client-js` 已能走真实 `MastraServer` 路由测试 agent/workflow/memory。
- `cargo test -p mastra-client-sdks-ai-sdk` 通过，`ai-sdk` 已能把 message history 转成 agent generate 请求，并把响应适配为流式事件。
- `cargo test -p mastra-client-sdks-react` 通过，`react` crate 现有真实 `ChatController` / `ChatState` 实现，不再只是测试壳。
- `cargo test -p mastra-packages-deployer -p mastra-deployers-cloud -p mastra-deployers-cloudflare -p mastra-deployers-netlify -p mastra-deployers-vercel` 通过，说明共享 deployer 契约与四个 provider plan materialization 已可用。
- `cargo test -p mastra-observability-mastra -p mastra-observability-datadog -p mastra-observability-langfuse -p mastra-observability-langsmith -p mastra-observability-posthog -p mastra-observability-sentry` 通过，说明 observability core 和五个 provider exporter 已具备真实 request builder 行为。

### Parallel Agent Findings

- deployer worker 的结论是：`packages/deployer` 与 `deployers/*` 当前实现已足以支撑 targeted tests，无需再扩接口；保留其已有代码并纳入本轮整体验证即可。
- observability worker/后台线程曾两次导致漂移：
  - 一次把 `observability/datadog/src/lib.rs` 写成了 `})?];` 的非法结构，导致 `cargo test --workspace` 的 doctest 阶段失败。
  - 一次只保留了 trait method，没有给五个 `Exporter` 暴露固有 `build_requests()`，导致 workspace integration tests 编译失败。
- 这证明并行 agent 适合做 bounded 实现，但最终必须由主线程做一次全仓回归与接口一致性收口。

### Evidence of Remaining Non-Parity

- `rg -l "fn add\\(|it_works\\(" --glob '!target' | wc -l` 返回 `78`，当前仍有 78 个 crate 保留生成模板级占位实现。
- 占位 crate 示例：
  - `auth/auth0/src/lib.rs`
  - `stores/couchbase/src/lib.rs`
  - `voice/openai/src/lib.rs`
  - `workspaces/gcs/src/lib.rs`
  - `packages/codemod/src/lib.rs`
- 因此“workspace tests 全绿”只能证明当前代码可编译、现有测试通过，不能证明已经 1:1 复刻完整 Mastra monorepo。

### Verification Notes

- 第一次 `cargo test --workspace` 在 `observability/datadog` doctest 阶段失败，原因是并行改写引入语法错误；已修复。
- 第二次 `cargo test --workspace` 在 observability provider tests 编译阶段失败，原因是 exporter 缺少固有 `build_requests()`；已修复。
- 第三次 `cargo test --workspace` 全量通过，说明当前工作树已稳定到可提交状态。
