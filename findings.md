# Findings

## 2026-03-12 Baseline Scan

### Workspace Shape

- Rust workspace 成员覆盖 `mastracode`、`packages/*`、`auth/*`、`client-sdks/*`、`deployers/*`、`observability/*`、`server-adapters/*`、`stores/*`、`voice/*`、`workflows/*`、`workspaces/*`、`integrations/*`、`pubsub/*`、`explorations/*`。
- 证据：顶层 `Cargo.toml` workspace members。

### Crates With Material Code

- `packages/server/src/lib.rs` 576 行
- `packages/server/src/router.rs` 531 行
- `packages/core/src/mastra.rs` 503 行
- `packages/core/src/agent.rs` 417 行
- `packages/core/src/workflow.rs` 280 行
- `packages/memory/src/in_memory.rs` 245 行
- `stores/libsql/src/lib.rs` 608 行
- `stores/pg/src/lib.rs` 129 行
- `mastracode/src/lib.rs` 217 行

### Mostly Placeholder Areas

- `auth/*` 总计 112 行，8 个 crate，单 crate 多为 14 行。
- `client-sdks/*` 总计 42 行，3 个 crate，单 crate 14 行。
- `deployers/*` 总计 56 行，4 个 crate，单 crate 14 行。
- `observability/*` 总计 182 行，13 个 crate，多数 14 行。
- `voice/*` 总计 196 行，14 个 crate，多数 14 行。
- `workflows/*` 总计 28 行，2 个 crate，多数 14 行。
- `workspaces/*` 总计 84 行，6 个 crate，多数 14 行。
- `pubsub/google-cloud-pubsub` 14 行。

### Core Capabilities Already Present

- `packages/core` 已有：
  - `Mastra` 注册与快照
  - `Agent` 生成与流式响应
  - `Workflow` 顺序 step 执行
  - `Tool` / `Memory` / `RequestContext` / `Model` 基础契约
- `packages/server` 已有：
  - `axum` HTTP server
  - agents / workflows / memories 路由
  - runtime registry
  - workflow run records

### Initial Conclusion

- 这个仓库不是从零开始，但距离“1:1 复刻 Mastra”仍差一个数量级。
- 当前最合理路径不是横向把所有空壳同时填满，而是先闭合核心运行时，再把长尾 crate 接到统一契约上。

## 2026-03-12 Test Baseline

### Workspace Verification

- `cargo test --workspace` 当前通过。
- 这说明现有代码在“当前断言集”下可编译可运行。

### Important Caveat

- 通过的测试里，大量占位 crate 仍然只是：
  - `pub fn add(left, right) -> left + right`
  - `it_works()`
- 因此当前全绿只能证明“骨架可编译”，不能证明“接近上游功能完整”。
