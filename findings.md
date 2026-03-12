# Findings

## 2026-03-12

### Repo State

- `HEAD` 当前在 `da50541 feat(runtime): add sdk auth deployer and observability primitives`。
- 其后又出现未提交脏改动，集中在 `observability/{arize,braintrust,laminar,otel-bridge,otel-exporter}`。

### Placeholder Inventory

- `rg -l "fn add\\(|it_works\\(" --glob '!target'` 仍命中大批 crate，主要分布在：
  - `auth/*`
  - `stores/*`
  - `voice/*`
  - `workspaces/*`
  - `packages/{agent-builder,codemod,editor,evals,fastembed,playground,playground-ui,schema-compat,mcp-docs-server,mcp-registry-registry,...}`
  - `integrations/opencode`
  - `explorations/longmemeval`
  - 多个 `*_test-utils` / `_vendored` crate

### Observability Cluster

- 当前未提交的 observability crates 已具备真实实现，不再是 placeholder：
  - `arize` / `laminar` 通过 `otel-exporter` 包装 OTLP request builder
  - `braintrust` 直接构造 provider-specific ingest payload
  - `otel-bridge` 暴露 `TraceBatch -> OTLP payload` bridge
  - `otel-exporter` 提供通用 OTLP JSON exporter
- targeted verification 已通过：
  - `cargo test -p mastra-observability-arize -p mastra-observability-braintrust -p mastra-observability-laminar -p mastra-observability-otel-bridge -p mastra-observability-otel-exporter`

### Reference Surface

- `.ref/mastra/observability/*` 显示 upstream 这些 provider 的核心语义确实围绕 tracing/exporter/provider config 展开，而不是复杂业务 runtime。
- `.ref/mastra/auth/*` README 显示 upstream auth provider 包本质是对共享 auth system 的 provider-specific config / JWT verify / login handling 封装。
- 这说明剩余 placeholder crate 大多可以按“共享 primitive + provider config/adapter + tests”的方式分群实现，而不需要每个 crate 从零单独设计。
