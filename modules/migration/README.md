# migration 模块

> Voxel Chunk/World Schema 转换、Migration 节点、校验、Staging 与失败保留。

## 模块定位与目标

`migration` 负责把旧版本 Voxel Snapshot 转换为目标 Schema，同时保持源数据不可变、过程可重跑、结果可校验。它实现 VoxelEngine 所有的领域转换节点，但由 Host/Runtime 负责整体 Migration DAG 编排、文件耐久和最终激活。迁移不运行在 Tick 热路径，也不允许原地改写生产 Snapshot。

## 负责什么

- 声明 Voxel Migration 节点的输入/输出 Schema、版本 Epoch、依赖和幂等性。
- 从不可变 `SnapshotId + SessionRevisionVector` 读取 Chunk/World 数据，在 Staging 中生成目标 payload。
- 执行 Chunk/Block/引用/资源上限/Hash/Checksum 校验，产出转换摘要和失败证据。
- 提供节点级 Checkpoint、重跑和验证接口；同一输入重复执行得到等价结果。
- 向 Host 返回可激活的目标 Snapshot/Manifest 元数据；不直接覆盖旧 Active 指针。
- 支持旧版本读取、明确的降级/拒绝和跨版本不可逆变换保护。

## 明确不负责什么

- 不拥有 Game 语义、Gameplay 迁移顺序或完整 Migration DAG 的最终裁决（由 Game/Runtime/架构源）。
- 不在 World Tick 中执行，不直接写正在运行的 VoxelWorld，不绕过 Snapshot/Revision 校验。
- 不负责文件系统 fsync、原子指针替换、WAL/Checkpoint 保留或进程维护（由 Host/Runtime）。
- 不接受未知 Schema、循环依赖、未经签名/Hash 校验的输入，也不执行输入中的代码。
- 不把临时转换缓存当作权威存档或发布 Artifact。

## 拥有的状态与资源

- Migration 节点注册表、输入/输出 Schema、依赖和幂等标记。
- Staging 目录/版本句柄、节点状态、Checkpoint 和校验摘要。
- 转换资源预算、失败保留清单和重跑参数。
- Voxel 领域转换器 Adapter（Chunk/Block/压缩页），不持有运行中 World 指针。

## 输入、输出与稳定接口

- **输入**：架构源 `MigrationManifest`、不可变 Snapshot、源/目标 Release/Schema、资源预算和取消信号。
- **输出**：节点结果（目标 Canonical payload、Hash、Revision/Schema Epoch）、整体验证结果、Failure Bundle 片段。
- **接口草案**：`validate_manifest(manifest) -> ValidatedGraph | StableError`；`run_node(node, input) -> StagedArtifact | StableError`；`verify(staged) -> VerifiedArtifact | StableError`；`request_activation(artifact) -> ActivationRequest`。

## 上游与下游依赖

- **上游**：Host/维护编排、[snapshot](../snapshot/README.md)（Canonical 输入）、[chunk](../chunk/README.md)/[revision](../revision/README.md)（领域转换视图）。
- **下游**：Host 原子激活与 Runtime/World 恢复；不调用 Gameplay 热路径。
- **基础依赖**：架构源 MigrationManifest/SnapshotHeader Schema、Canonical Serializer 和受审查的转换 Adapter。

## 生命周期与状态机

整体迁移：

```text
Created -> ManifestValidated -> Staging -> NodeRunning -> Verifying
Verifying -> ReadyForActivation
Created/ManifestValidated/Staging/NodeRunning/Verifying -> Rejected | Failed | Cancelled
```

节点：`Pending -> Running -> Checkpointed -> Completed`，失败为 `Failed`；节点必须按 DAG 依赖执行，不能循环或隐式跳过依赖。

- `ReadyForActivation` 只表示 Staging 产物通过校验，不表示已经激活。
- 失败、取消或进程崩溃后源 Snapshot 和旧 Active 指针保持不变；重跑从不可变输入或已验证 Checkpoint 开始。

## 线程、队列与并发所有权

- 迁移运行在独立 Tool/Maintenance Worker，不占用 Simulation Owner Thread 的 Tick 预算。
- 节点执行、解压和校验任务使用有界 Worker/Buffer；结果按 DAG 顺序合并。
- 运行中 World 与 Staging Artifact 之间没有可变共享；取消后迟到节点结果不得激活。
- 资源预算、磁盘水位和并发节点数必须可观测，超限停止图而不是无限排队。

## 正常数据流与失败路径

- **正常**：读取源 Snapshot → 校验 Manifest/DAG → 建立 Staging → 按依赖执行 Voxel 节点 → 校验引用/配额/Hash → 生成目标 Artifact → 请求原子激活。
- **失败路径**：循环/缺依赖、源 Schema 不支持、Chunk/引用错误、配额超限、Hash/签名失败、节点崩溃或磁盘满均终止 Staging，保留旧版本和证据。
- **恢复**：重启后扫描 Staging/Checkpoint，验证输入与工具 Hash；只重跑幂等且未完成的节点。

## 错误分类、恢复与降级

- **可重试**：暂时 IO/Worker 资源不足、节点结果丢失、可验证的 Checkpoint 恢复。
- **可拒绝**：循环/缺依赖、Schema/Release 不匹配、引用/配额/Hash/签名失败、非幂等节点无安全重跑点。
- **可致命**：源存档损坏且无可用 Checkpoint、Staging 无法隔离或转换器破坏不变量；进入维护/进程级恢复路径。
- **降级**：不做隐式降级或原地修复；只能由运维选择兼容的目标 Release 或保留旧版本。

## 配置、Capability 与安全约束

- 源/目标 Release、Schema Epoch、工具版本、资源/磁盘预算和激活策略必须写入 Manifest/配置。
- Snapshot 输入按 Magic、SchemaVersion、Length、Hash/Checksum、引用和解压限制校验；不信任外部节点参数。
- Staging 与 Active 存储隔离，文件权限/加密由 Host 管理；密钥不入库、不进日志。
- Migration Artifact 必须记录输入/输出 Hash、Tool/Compiler 版本和 SBOM 关联，便于重放。

## 日志、Metrics、Trace 与 Audit

- Audit：MigrationId、源/目标 Release、节点开始/完成/失败、激活/回滚（关联 SnapshotId、SchemaEpoch、TraceId）。
- Metrics：节点耗时、吞吐、Staging 磁盘、内存/CPU、失败/重试、恢复重跑时间。
- Failure Bundle：保留源 Snapshot 引用、失败节点输入 Hash、工具版本和可重跑命令，不上传敏感 payload。

## 测试面、故障矩阵与性能指标

- **测试面**：golden 升级、旧版本读取、幂等重跑、引用/配额校验、目标 Schema 校验、原子激活前后可见性。
- **故障矩阵**：循环/缺依赖、Chunk 损坏、Hash/签名错误、节点崩溃、磁盘满、取消、Crash-at-node、旧 Active 保留。
- **性能指标**：每节点吞吐、全图迁移时长、Staging 峰值磁盘/内存、重跑成本和恢复时间。

## 对应 ADR、Schema 与 Fixture

- 架构源 `docs/adr/ADR-013-migration-dag.md`：不可变 Snapshot、DAG、Staging、原子激活和失败恢复。
- 架构源 `schemas/migration-manifest.schema.json`：正例 `fixtures/valid/migration-manifest.json`；反例 `fixtures/invalid/migration-cycle.json`。
- 架构源 `schemas/snapshot-header.schema.json`：源/目标 Snapshot 元数据。
- Voxel 节点的专属输入/输出 Schema 尚未发布；节点字段必须先回架构源登记。

## 尚未批准的决策门

- **VOX-D-008**（Voxel Migration 节点粒度、Checkpoint 和激活策略）：临时采用不可变输入、Staging、幂等节点和原子激活；需 golden、崩溃点和大世界基准。
