# mutation 模块

> 单域 Mutation、Prepare/Reservation、幂等 Commit/Abort、Expected Revision 冲突与恢复摘要。
> 物理 crate：`lumio-voxel-ops`（[0006](../../.spec/decisions/0006-crate-map.md) / [0007](../../.spec/decisions/0007-v1.4-implementation-baseline.md)）；L2 WriteSet/CommitBatch 在 `lumio-voxel-domain`。

## 模块定位与目标

`mutation` 是 VoxelWorld 唯一的写入入口。它将上层命令转换为受边界保护的变更批次，在 Prepare 阶段完成所有可失败检查并创建不可见 Reservation，在固定 Barrier 由 Coordinator 决定后幂等 Commit 或 Abort。它只判断 Voxel 结构条件，不判断 Gameplay 权限、扣费或资源语义。

## 负责什么

- 校验坐标、Chunk 可用性、Cell 可写性、容量、Expected `ChunkRevision` 和 Context/Generation。
- 创建带租约的不可见 `MutationReservation`/`PreparedVoxelToken`，防止 Prepare 产生可见副作用。
- 在 Simulation Barrier 经 CommitBatch 原子发布已批准的 Block 变化与 `WorldRevision/ChunkRevision`，并返回变更摘要。本模块协调 publish，但不代替 `revision` 拥有版本计数器。
- 按 `TxnId` 保存 participant receipt；重复 Commit 返回原结果，不重复写入。收费语义由 Game 侧负责。
- 支持 Abort、超时、取消、租约到期和 Chunk Unloaded 的稳定原因。
- 为 Runtime Coordinator 提供 Voxel 参与者状态、结果 Revision 和可查询 receipt。

## 明确不负责什么

- 不拥有 CrossWorld 协调、全局 `CommitIntent`、Game/ECS CommandBuffer 或 TxnJournal 的最终持久化（归 Runtime/Host）。
- 不做玩家权限、阵营、隐身、库存、扣费、Ability 或其他 Gameplay 判断；只接受上层已完成的结构前置条件上下文。
- 不直接加载 Chunk、不绕过 [query](../query/README.md) 的只读边界、不持有跨模块 Storage 锁。
- 不调用 [snapshot](../snapshot/README.md) 或 [streaming](../streaming/README.md)；只读 Availability Port，并发布 `ChunkChanged`。
- 不在 Native 锁内回调 C#，不由 Worker 线程直接触发 Hot Gameplay。
- 不允许无 Expected Revision 的静默覆盖写入。

## 拥有的状态与资源

- 活跃 Reservation、租约截止和已锁定 Chunk/Cell 范围。
- `TxnId -> ParticipantReceipt` 的有界幂等缓存（与 CommitBatch 共同耐久，见架构源 ADR-025；表容量属 VOX-D-004）。
- Prepare 失败原因、Abort 原因、变更摘要和待提交 RevisionDelta。
- Barrier 内的临时 WriteSet / CommitBatch；不长期持有 Chunk 可变引用。

## 输入、输出与稳定接口

- **输入**：Mutation Batch（坐标/新值/Expected Revision/TxnId/Deadline）、World Context、上层已验证的结构上下文。
- **输出**：Prepare Token、Reservation 状态、Commit/Abort 结果、变更范围和新 Revision。
- **本仓 Port 表面**（receipt/`status` 形态见架构源 `voxel-mutation-receipt`）：`prepare(batch) -> PreparedVoxelToken | MutationError`；`commit(txn_id, token) -> CommitResult | StableError`；`abort(txn_id, token, reason)`；`status(txn_id) -> MutationStatus`。

## 依赖（编译 / 控制流 / 事件与数据）

- **编译依赖**：[chunk](../chunk/README.md)（WriteView）、[revision](../revision/README.md)（Stamp/advance publish）、Availability Port 类型、NativeCore 稳定错误。不依赖 snapshot、streaming 或 world。
- **被谁调用**：[world](../world/README.md)（Context/Barrier）；Runtime Coordinator 只经 Port 决定 Commit/Abort。
- **发布/消费**：发布 `ChunkChanged` 供 snapshot Diff / 投影缓存失效消费；只读 Availability，不控制 Load。Host/Runtime 持久化 `TxnJournal`；本模块只提供可查询 participant receipt。

## 生命周期与状态机

单域 / Voxel 参与者状态（本模块拥有）：

```text
Created -> Validating -> Prepared -> Applied
                         |          |
                         v          v
                       Aborted   Duplicate
Prepared -> Expired | Cancelled | ChunkUnavailable
Created/Validating -> Rejected
Applied -> receipt retained until pruning handshake
```

Runtime 拥有的全局 CrossWorldTxn 状态（本模块不存储）：

```text
Created -> Prepared -> CommitIntent -> Committed
       \-> Aborted
Prepared -> Indeterminate
```

- Prepare 阶段不得改变可见 Block 或 Revision。
- Commit 必须在 `VoxelCommit` Barrier 经 CommitBatch 执行；第一个可见写入后不得再失败为普通错误。
- 重复 `TxnId` 只能返回原 receipt。
- `Indeterminate` 不由本模块猜测成功/失败；Runtime 通过 Journal 标记和 `status(txnId)` 解决。
- 本模块不进入 `CommitIntent` 状态。 Coordinator 必须先持久化 Intent，再调用 Voxel Apply。

## 线程、队列与并发所有权

- Prepare 可在受控调用路径执行只读校验；Reservation 的建立、Commit、Abort 和 Revision 更新在 Voxel Barrier 串行化。
- Reservation 表是有界状态；租约清理可由 Worker 触发请求，但实际状态变更回到 Barrier。
- Commit 期间只持有必要的 Chunk 写视图，不跨 FFI 或异步边界持锁。
- Mutation 请求/结果队列必须声明容量和满载动作；可靠写入不能静默丢弃，满载由 Runtime/Host 停止接入或拒绝。

## 正常数据流与失败路径

- **Prepare**：批次规范化 → Chunk/Cell/Revision/容量检查 → 建立 Reservation → 返回 Token。
- **Commit**：确认 Coordinator 已持久化 `CommitIntent`（CrossWorld）或单域调用方已批准 → CommitBatch 在同一原子批内发布页、Revision 并记录 participant receipt（架构源 ADR-025：`CoDurableWithWorldState`）→ 发布 `ChunkChanged` → 返回结果。
- **Abort**：释放 Reservation，不产生可见变更；重复 Abort 幂等。
- **失败路径**：Revision 冲突、Chunk 未加载、Cell 不可写、容量超限、租约过期、取消、Context 失效均在可见写入前拒绝；Commit 后结果丢失通过 `status(txnId)` 查询。崩溃后 receipt 与写入一起从共同耐久批次恢复，遵循架构源 ADR-025（含 pruning handshake 与 `ResultPruned` 终态）。

## 错误分类、恢复与降级

- **可重试**：Chunk 暂不可用、短暂 Reservation 资源不足、结果丢失（状态查询后按幂等规则重试）。
- **可拒绝**：RevisionConflict、ChunkUnloaded、ValidationFailed、DeadlineExceeded、Cancelled、Context/Generation 不匹配。
- **可致命**：Commit 后无法维护幂等记录、检测到写入不变量破坏；上报 World/进程故障域并停止新写入。
- **降级**：只允许显式 Abort 或重新读版本后生成新 Mutation；禁止强制覆盖、部分静默提交或把 `Indeterminate` 当成功。

## 配置、Capability 与安全约束

- Reservation 租约、批次大小、单 Tick 写预算和幂等记录上限来自不可变配置快照。
- 所有输入先做长度、坐标、资源上限和 Revision 校验；Token 是不透明值，不携带权限。
- LocalEmbedded 与 RemoteDS 复用同一 Prepare/Commit/Abort 语义；本地路径不得旁路验证。
- 破坏性 Chunk/Revision 变化必须保留旧 Fixture、Migration 和失败恢复路径。

## 日志、Metrics、Trace 与 Audit

- Diagnostic：Prepare/Commit/Abort 状态、冲突、租约、队列和结果丢失。
- Metrics：Prepare/Commit 延迟、冲突率、Reservation 数/租约超时、幂等命中率、写入字节和批次大小。
- Audit/TxnJournal 由 Runtime/Host 持久化；模块提供 `txnId/sessionId/tickId/chunkId/worldRevision/chunkRevision/traceId` 关联片段。

## 测试面、故障矩阵与性能指标

- **测试面**：Prepare 无副作用、Revision 冲突、重复 Commit/Abort、租约到期、批次边界、稳定排序、World/Chunk Revision 单调性。
- **故障矩阵**：Chunk Unloaded、Lost Result、Deadline、取消、Commit 前崩溃、Voxel/Game 两参与者之间崩溃、幂等恢复、QueueFull。
- **性能指标**：Prepare/Commit p50/p95/p99、每 Tick 批量写入吞吐、Reservation 内存、冲突重试成本、Reference/Native Differential。

## 对应 ADR、Schema 与 Fixture

- 本仓 [0002](../../.spec/decisions/0002-barrier-commit-batch.md)、[0006](../../.spec/decisions/0006-crate-map.md)、[0007](../../.spec/decisions/0007-v1.4-implementation-baseline.md)。
- 架构源 `docs/adr/ADR-003-cross-world-txn.md`：Prepare/Reservation/CommitIntent、固定 Commit 顺序和 Indeterminate 恢复。
- 架构源 `schemas/cross-world-txn.schema.json`：正例 `fixtures/valid/cross-world-txn-committed.json`、`fixtures/valid/cross-world-txn-aborted.json`；反例 `fixtures/invalid/cross-world-txn-partial-commit.json`。
- 架构源 `schemas/voxel-mutation-receipt.schema.json`：participant receipt 与 `status(txnId)`；ADR-025（`CoDurableWithWorldState`）。

## 尚未批准的决策门

- **VOX-D-004**（Reservation 租约与 receipt 表容量）：崩溃恢复协议已冻结；租约和表上限待容量测试。
