# migration 模块

> Voxel Chunk/World Schema 转换节点、节点级校验与幂等执行；不拥有全图编排。

## 模块定位与目标

`migration` 提供 VoxelEngine 的领域转换节点：把旧版本 Voxel Snapshot 转为目标 Schema，保持源数据不可变、节点可重跑、结果可校验。Host/Server 拥有完整 Migration DAG 编排、Staging 目录生命周期、Checkpoint 索引、进程恢复和原子激活。迁移不运行在 Tick 热路径，也不允许原地改写生产 Snapshot。

## 负责什么

- 声明 Voxel Migration 节点的输入/输出 Schema、版本 Epoch、依赖和幂等性。
- 从 Host 提供的不可变 Snapshot 字节读取 Chunk/World 数据，产出节点局部目标 payload。
- 执行 Chunk/Block/引用/资源上限/Hash/Checksum 校验，产出节点转换摘要和失败证据。
- 提供节点级重跑和验证接口；同一输入重复执行得到等价结果。
- 向 Host 返回可激活的节点 Artifact 元数据；不直接覆盖旧 Active 指针。
- 支持旧版本读取、明确的降级/拒绝和跨版本不可逆变换保护。

## 明确不负责什么

- 不拥有 Game 语义、Gameplay 迁移顺序或完整 Migration DAG 的编排与最终裁决（由 Game/Runtime/Host）。
- 不拥有 Staging 目录、磁盘水位、Checkpoint 索引、重启扫描、原子激活或 Active 指针。
- 不在 World Tick 中执行，不直接写正在运行的 VoxelWorld，不绕过 Snapshot/Revision 校验。
- 不负责文件系统 fsync、原子指针替换、WAL 保留或进程维护（由 Host/Runtime）。
- 不接受未知 Schema、循环依赖、未经签名/Hash 校验的输入，也不执行输入中的代码。
- 不把临时转换缓存当作权威存档或发布 Artifact。

## 拥有的状态与资源

- Migration 节点注册表、输入/输出 Schema、依赖和幂等标记。
- 节点局部执行状态、校验摘要和失败原因。
- 转换资源预算和重跑参数。
- Voxel 领域转换器 Adapter（Chunk/Block/压缩页），不持有运行中 World 指针、Staging 路径或 Active 指针。

## 输入、输出与稳定接口

- **输入**：Host 传入的节点描述、不可变 Snapshot 字节、源/目标 Schema、资源预算和取消信号。
- **输出**：节点结果（目标 Canonical payload、Hash、Revision/Schema Epoch）、节点级验证结果、Failure Bundle 片段。
- **本仓 Port 表面**：`describe_nodes() -> NodeDescriptor[]`；`run_node(node, input) -> NodeArtifact | StableError`；`verify_node(artifact) -> VerifiedArtifact | StableError`。不提供 `validate_manifest` 全图接口或 `request_activation`。

## 依赖（编译 / 控制流 / 事件与数据）

- **编译依赖**：[snapshot](../snapshot/README.md)（Canonical decode 类型）、[chunk](../chunk/README.md)/[revision](../revision/README.md)（领域转换视图）、架构源 MigrationManifest/SnapshotHeader 与 Canonical Serializer。不依赖 world。
- **被谁调用**：Host/维护编排。不从 Tick 热路径回调。
- **发布/消费**：消费不可变 Snapshot Artifact；向 Host 交还节点 payload。不扫描 Staging，不合并全图结果。

## 生命周期与状态机

节点：

```text
Pending -> Running -> Completed
Pending/Running -> Failed | Cancelled
```

- 节点必须按 Host 提供的依赖顺序执行；本模块不裁决全图，不检测或修复 Manifest 循环（Host/架构源校验器负责）。
- `Completed` 只表示该节点 payload 通过校验，不表示已经激活。
- 失败、取消或进程崩溃后源 Snapshot 和旧 Active 指针由 Host 保持不变；重跑由 Host 从不可变输入或已验证节点 Checkpoint 发起。

## 线程、队列与并发所有权

- 迁移运行在独立 Tool/Maintenance Worker，不占用 Simulation Owner Thread 的 Tick 预算。
- 节点执行、解压和校验任务使用有界 Worker/Buffer。
- 运行中 World 与节点 Artifact 之间没有可变共享；取消后迟到节点结果不得被 Host 激活。
- 资源预算必须可观测，超限停止该节点而不是无限排队。磁盘水位与并发图调度归 Host。

## 正常数据流与失败路径

- **正常**：Host 读取源 Snapshot 并校验 Manifest/DAG → 对本模块调用节点 → 节点校验引用/配额/Hash → 交还目标 payload → Host 负责 Staging 与原子激活。
- **失败路径**：源 Schema 不支持、Chunk/引用错误、配额超限、Hash 失败、节点崩溃均终止该节点，Host 保留旧版本和证据。
- **恢复**：本模块不扫描 Staging。Host 验证输入与工具 Hash 后，只重跑幂等且未完成的节点。

## 错误分类、恢复与降级

- **可重试**：暂时 IO/Worker 资源不足、节点结果丢失、可验证的 Checkpoint 恢复。
- **可拒绝**：Schema/Release 不匹配、引用/配额/Hash/签名失败、非幂等节点无安全重跑点。循环/缺依赖由 Host 在调度前拒绝。
- **可致命**：源存档损坏且无可用 Checkpoint、Staging 无法隔离或转换器破坏不变量；进入维护/进程级恢复路径。
- **降级**：不做隐式降级或原地修复；只能由运维选择兼容的目标 Release 或保留旧版本。

## 配置、Capability 与安全约束

- 源/目标 Schema Epoch、工具版本、资源预算必须由 Host 写入 Manifest/配置后再传入节点。
- Snapshot 输入按 Magic、SchemaVersion、Length、Hash/Checksum、引用和解压限制校验；不信任外部节点参数。
- Staging 与 Active 存储隔离由 Host 管理；密钥不入库、不进日志。
- 节点 Artifact 必须记录输入/输出 Hash 和 Tool/Compiler 版本，便于 Host 重放。公共 Manifest 字段以架构源 Schema 为准（当前 Schema 与 ADR-013 的 hash/tool version 要求尚不一致，不得在本模块自行补字段）。

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
- Manifest 节点现要求 `inputHash`/`outputHash`/`toolVersion`（ADR-013）。节点载荷 Schema 仍须先回架构源登记。

## 尚未批准的决策门

- **VOX-D-008**（Voxel Migration 节点粒度）：Manifest 公共字段已齐；节点切分与大世界基准仍开放。
