# Task Plan

## Goal

在当前 `mastra-rs` 工作树上，一次性收口本轮 parity rollout：把新增的 auth、stores、voice、workspaces、supporting packages 与 observability cluster 从 placeholder/半成品状态推进到“有真实 API、有测试、可通过 root workspace 回归”的状态。

## Constraints

- 用户要求一次性完成，不分批停下。
- 过程中自动生成 commit，并行协调多个 agent。
- `task_plan.md`、`findings.md`、`progress.md` 仅由主线程维护，避免再次被子任务污染。
- 不追求对旧占位格式做兼容，优先最小真实实现与可测试性。

## Parallel Ownership

- 主线程：`auth/**`、总集成、commit、planning files。
- Halley：`stores/**`。
- Wegener：`voice/**`、`workspaces/**`。
- Hilbert：`integrations/**`、`packages/**`、`server-adapters/_test-utils`、`workflows/_test-utils`。

## Phases

1. 审计现有工作树、识别被污染的 planning files、确认各 cluster 的未完成面。
2. 收口 auth 八个 provider crate，并落单独 commit。
3. 回收并验证 stores、voice/workspaces、misc packages cluster。
4. 在真实 root workspace 上执行完整 `cargo test --workspace`。
5. 重写 planning files，记录事实证据与验证结果。
6. 生成最终 integration commit。

## Status

- [x] Phase 1: 审计工作树与 planning 污染面。
- [x] Phase 2: auth cluster 完成并提交 `67dfacd feat(auth): add provider wrappers for auth crates`。
- [x] Phase 3: stores、voice/workspaces、misc packages 已集成到当前树并通过 root workspace 回归。
- [x] Phase 4: `cargo test --workspace` 通过。
- [x] Phase 5: planning files 已重写为本次 rollout 记录。
- [ ] Phase 6: 生成最终 integration commit。

## Commit Checkpoints

- `820db0a feat(runtime): add mcp pubsub and inngest primitives`
- `da50541 feat(runtime): add sdk auth deployer and observability primitives`
- `43546e8 feat(observability): add otel and provider exporters`
- `67dfacd feat(auth): add provider wrappers for auth crates`
