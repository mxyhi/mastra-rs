# Findings

## Auth Cluster

- `packages/auth` 已经提供了足够的共享原语：`OidcJwtAuthenticator`、`SessionBackedAuthenticator`、`CallbackSessionAuthenticator`，所以 provider crate 最合理的实现方式是“薄 wrapper + provider-local config/trait”，而不是再发明一层 auth core。
- OIDC 类 provider (`auth0`、`clerk`、`firebase`、`workos` bearer fallback) 统一围绕 `OidcConfiguration` 组装 issuer、audience、JWKS、algorithms 与 leeway。
- Session 类 provider (`better-auth`、`supabase`) 统一通过 request cookie/bearer 解析后委托 provider-local client trait。
- Callback 类 provider (`cloud`、`studio`) 统一复用 `CallbackSessionAuthenticator`，只保留 provider 特有的 callback 交换语义。

## Stores Cluster

- `stores/_test-utils/src/provider_support.rs` 证明 stores rollout 的真实模式已经从占位 crate 收敛到统一的 provider descriptor/bridge 模型：
  - `ProviderDescriptor` 描述 provider kind 与 capability。
  - `ProviderBridge` 绑定 target 与 secrets，并支持 redaction。
  - `ensure_not_blank` 负责通用配置校验。
- 各个 stores crate 当前实现是“provider metadata + config validation + capability bridge”，不再是 `add/it_works` 模板。

## Voice Cluster

- `voice/core/src/lib.rs` 已形成统一的语音 provider 抽象：
  - `VoiceProviderProfile` 定义 env vars、speech/listening models、speaker catalog、capabilities。
  - `VoiceProviderAdapter` 负责把 speak/listen request 解析成 provider-specific resolved request。
  - capability 校验、speaker 校验、transport 解析都已下沉到 core。
- 各 voice provider crate 现在主要提供 provider profile 与 provider-specific defaults，而不是各自重复造校验逻辑。

## Workspaces Cluster

- `workspaces/core/src/lib.rs` 已形成统一的 workspace provider 抽象：
  - `WorkspaceProviderKind` 区分 filesystem/blob store/sandbox。
  - `ConfigField` 与 `ConfigFieldKind` 描述 schema。
  - `WorkspaceProviderAdapter::validate_config` 负责默认值填充、required field 校验、类型校验与 enum 校验。
- 各 workspace provider crate 现在主要声明 kinds、config fields 与 defaults，复用 core 的验证流程。

## Supporting Packages

- `_llm-recorder` 已提供真实的 request hash / recording / contract validation 能力，不再是占位包。
- `explorations/longmemeval` 已提供 memory config 枚举与评估汇总逻辑。
- `_changeset-cli`、`_config`、`_external-types`、`_test-utils`、`_types-builder`、`agent-builder`、`mcp-docs-server`、`mcp-registry-registry`、`schema-compat`、`codemod`、`editor`、`evals`、`fastembed`、`playground`、`playground-ui`、`integrations/opencode`、`server-adapters/_test-utils`、`workflows/_test-utils` 均已替换为最小真实实现，并带单测。

## Verification

- auth 定向验证：
  - `cargo test -p mastra-auth-auth0 -p mastra-auth-better-auth -p mastra-auth-clerk -p mastra-auth-cloud -p mastra-auth-firebase -p mastra-auth-studio -p mastra-auth-supabase -p mastra-auth-workos`
- 全仓验证：
  - `cargo fmt --all`
  - `cargo test --workspace`
- 结果：root workspace 全量测试与 doc-tests 全部通过。
