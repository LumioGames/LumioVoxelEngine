---
name: testing
description: 测试与验收——测试分层政策、TDD 时机、验收 DoD 与验证证据;实现功能/修 bug 时查
metadata:
  type: doc
  status: 已交付
---

# 测试与验收（含 TDD 政策）

> 本文定**政策**（测什么、何时测、怎么算过）；“先写失败测试再实现”的**方法**在技能 [`skills/test-driven-development`](../../skills/test-driven-development/SKILL.md)。

## 测试分层（通用政策）

- **单元测试**：默认层，随项目验证命令（`AGENTS.md`「收口门槛」）每次跑，快、无外部依赖。
- **集成测试**（真库 / 真服务）：显式触发，不进默认验证命令，保持收口快。
- **端到端 / E2E**：显式触发；关键主链路至少一条。

## 何时走 TDD

- 必须走：新功能、修 bug（先写能复现的失败测试，修完留作回归测试）、改无测试保护的关键逻辑。
- 可不走：纯文档改动、一次性脚本。豁免在交回物里声明。
- 写测试、加 mock、想给生产类加 test-only 方法前，先查反模式清单：[`testing-anti-patterns.md`](../../skills/test-driven-development/testing-anti-patterns.md)——测 mock 行为、test-only 方法入生产、不理解依赖就 mock、不完整 mock，一律禁止。

## 验证证据

形式要求以 `AGENTS.md`「交回物格式」为单一权威——「已通过」三个字不是证据。

## 验收标准（Definition of Done）

- [ ] 收口门槛命令全绿（`node .spec/tools/spec-lint.mjs`、`node --test .spec/tools/spec-lint.test.mjs`，以及 Cargo 工作区命令，见下节）。
- [ ] 新增 / 修改行为有测试覆盖；bug 修复留有回归测试。
- [ ] 无 lint / 类型错误、无调试残留。
- [ ] 相关知识文档已更新（见 [`workflow.md`](./workflow.md)）。

## 项目测试栈与命令

默认验证：

```text
node .spec/tools/spec-lint.mjs
node --test .spec/tools/spec-lint.test.mjs
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo check --workspace --no-default-features
cargo test --workspace --all-features
cargo check-crate-dag
cargo check-generated-clean
python tools/architecture/check_crate_dag.py
python tools/architecture/check_generated_clean.py
python tools/architecture/test_guards.py
```

公共 Contract 变更还必须在 `LumioGameEngineArchitecture` 安装 `requirements-dev.txt` 后运行 `python3 tools/lumio_contract.py validate`。

工作区恰好七个 crate（ADR-0006）：`lumio-voxel-contracts`、`lumio-voxel-domain`、`lumio-voxel-ops`、`lumio-voxel-world`、`lumio-voxel-project`、`lumio-voxel-migration`、`lumio-voxel-test-support`。禁止 persistence / runtime / ffi / common crate。DAG 与 generated-clean 的实现入口是 `lumio_voxel_test_support::crate_dag::violations` 与 `lumio_voxel_test_support::generated_clean::violations`。

## 本仓 Headless / 契约测试面

- Section/坐标/边界/Revision/Mutation/Reservation/幂等和冲突 Property/Golden Test；`SectionId`/`ChunkId` 键解析、派生与旧式三坐标 `c:x:y:z` 的显式拒绝。
- Snapshot/Diff、Canonical Serialization、压缩、损坏、恢复和 Migration Fixture。
- Load/Unload/Streaming 背压、取消、超时、缺 Section Query 和资源预算。
- Reference Voxel Port 与真实 Native 实现的 Differential Test。
- Voxel Spatial/AOI/Collision Benchmark，记录 Section 密度、AOI 半径、队列、CPU 与内存。
- Fault：Section Load Failure、Revision Conflict、Lost Result、Snapshot Corruption、Migration Failure、OOM、磁盘满。
- 破坏性 Section/Revision 变化必须同时覆盖旧版本 Fixture、Migration 和失败恢复路径。
