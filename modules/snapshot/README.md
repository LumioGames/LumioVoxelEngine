# snapshot 模块

> Snapshot Cut、Voxel Snapshot/Diff、Canonical 编码/解码、校验和恢复输入。

## 模块定位与目标

`snapshot` 把某个明确的 `SnapshotCut` 转换为可校验、可恢复、可迁移的 Voxel 数据。它拥有内存中的一致读取切面、Diff 计算和 Canonical Serializer 调用边界，但不拥有文件系统耐久、WAL/Command Log 或最终激活指针。Snapshot 必须能说明自己对应的 `WorldRevision`、`ChunkRevisionSet`、`SessionRevisionVector` 和 Schema Epoch。

## 负责什么

- 在协调 Barrier 接收指定 `SnapshotCut`，通过 [revision](../revision/README.md) Pin 或 COW 固定一致视图。
- 按稳定顺序收集 World 元数据、Chunk 页、Revision 和必要的 Migration 元数据。
- 生成 Snapshot 与局部 Diff 的 Canonical payload，调用生成的 `Encode/Decode` 契约。
- 在 materialize 前校验 Magic、SchemaVersion、Length、Compression、Hash/Checksum、边界和资源上限。
- 提供恢复输入、部分 Chunk 加载和 Snapshot 诊断摘要；不把诊断 JSON 当作权威存储。
- 在编码期间隔离后续写入，完成/取消后正确释放 Pin/COW 和临时 Buffer。

## 明确不负责什么

- 不负责临时文件、fsync、原子替换、Checkpoint 保留或 WAL/TxnJournal 落盘（归 Host/Runtime 持久化编排）。
- 不定义公共 SnapshotHeader 字段和 Schema 版本（归架构源）；不手写第二套 Serializer。
- 不执行 Migration DAG 或覆盖旧 Snapshot（归 [migration](../migration/README.md) 与 Host）。
- 不暂停 Tick、不拥有 World 生命周期或 CrossWorld Coordinator。
- 不把完整 Snapshot 自动复制到 C# Runtime 作为第二权威状态。

## 拥有的状态与资源

- 活跃 `SnapshotCut`、Pin/COW 句柄和 Cut 到 Revision 的映射。
- Snapshot/Diff 编码任务、临时 Canonical Buffer 和校验摘要。
- Decode/Restore 的资源预算、版本判定和失败原因。
- 部分 Chunk Snapshot 的索引与恢复游标。

## 输入、输出与稳定接口

- **输入**：Barrier 产出的 `SnapshotCut`、World/Chunk 只读视图、目标 Schema/Compression、恢复或 Diff 请求。
- **输出**：`SnapshotPayload`（Canonical bytes + Header 元数据）、`DiffPayload`、Decode/Restore 输入流、稳定校验错误。
- **接口草案**（公共 Voxel Payload Schema 待发布）：`capture(cut) -> SnapshotView | StableError`；`diff(base, target) -> DiffPayload`；`encode(view) -> CanonicalBytes`；`decode(header, bytes) -> TypedSnapshot | StableError`；`release(view)`。

## 上游与下游依赖

- **上游**：[world](../world/README.md)（Barrier/Context）、[revision](../revision/README.md)（Pin/COW）、[chunk](../chunk/README.md)（稳定页视图）。
- **下游**：Host/Runtime `persistence-host`（外部编排）和 [migration](../migration/README.md)（不可变输入）；Query/恢复工具消费 Decode 结果。
- **基础依赖**：NativeCore Buffer/Compression Kernel 和架构源生成的 Canonical Serializer。

## 生命周期与状态机

单次 Snapshot：

```text
Requested -> Cutting -> Pinned -> Encoding -> Verified -> Ready
Pinned/Encoding -> Cancelled | Failed
Ready -> Released
```

持久化激活状态遵循架构源 `SnapshotHeader`：`Staged -> Active`，校验失败为 `Invalid`；激活动作不由本模块直接执行。

- `Cutting` 只能在协调 Barrier 固定 Revision；异步编码使用同一 Pin/COW。
- `Verified` 只表示字节和 Header 校验通过，不表示已经 fsync 或成为 Active Checkpoint。
- 取消/失败必须释放 Pin/COW 和临时 Buffer，旧 Active Snapshot 不受影响。

## 线程、队列与并发所有权

- Cut/Pin 建立和释放由 Simulation Owner Thread 或其受控 Barrier 调用；编码可交给有界 Native/IO Worker。
- Worker 只读 Pin/COW，不直接改 Chunk/Revision；Completion 在约定安全点发布。
- Snapshot 请求、编码 Buffer 和恢复输入队列有界；超限返回 `QueueFull`/`BudgetExceeded`，不静默丢弃权威快照。
- Decode 期间执行分配、解压比和最大字段检查；不得跨 FFI 持有内部锁。

## 正常数据流与失败路径

- **生成**：固定 Cut → Pin/COW → 收集稳定 Chunk 顺序 → Encode → Hash/Checksum → 交持久化编排。
- **Diff**：比较指定 Base/Target Revision → 生成稳定变更顺序 → Encode；Base 不匹配时拒绝或要求 Full Snapshot。
- **恢复**：读取不可变字节 → Header/边界/Hash 校验 → Decode typed 状态 → 交 `world`/Runtime materialize。
- **失败路径**：Cut 失效、Pin 超时、编码失败、长度/Hash 不匹配、未知必需字段、解压预算超限都标记失败并保留证据，不覆盖旧版本。

## 错误分类、恢复与降级

- **可重试**：暂时 Pin/Worker 资源不足、外部持久化回执丢失（通过 SnapshotId/Hash 查询）。
- **可拒绝**：不兼容 Schema/Compression、截断/重复字段、Hash/Checksum 失败、Base Revision 不可用、预算超限。
- **可致命**：无法建立一致 Cut、检测到内部视图污染或 Decode 不变量破坏；停止该 World 的快照/写入并上报。
- **降级**：允许显式改为 Full Snapshot 或取消 Diff；不允许用不一致的部分数据宣称成功。

## 配置、Capability 与安全约束

- Snapshot 频率、并发数、保留策略和压缩选择由 Host/Manifest 配置；本模块只执行已声明能力。
- 所有外部字节先校验 Magic、SchemaVersion、Length、Hash/Checksum、最大分配和解压比；不执行输入代码。
- Snapshot 可能包含用户世界数据，访问控制/加密密钥由 Host/部署管理，密钥不进入模块日志。
- Singleplayer 与 Dedicated Server 使用同一 Canonical Voxel payload；差异只能在耐久策略声明。

## 日志、Metrics、Trace 与 Audit

- Diagnostic：Cut/Pin 时长、编码阶段、Hash/Checksum/Decode 错误、取消和预算。
- Metrics：Snapshot 生成 p50/p95/p99、Diff 大小、压缩比、Pin/COW 字节、Decode 分配和恢复吞吐。
- Audit/Failure Bundle：SnapshotId、BaseSnapshotId、World/Chunk Revision、SchemaEpoch、Hash 和失败阶段；耐久事件由 Host 关联。

## 测试面、故障矩阵与性能指标

- **测试面**：Snapshot/Diff round-trip、稳定排序、旧版本读取、未知可选字段、损坏/截断、Pin/COW 并发写、部分 Chunk 恢复。
- **故障矩阵**：长度/Hash 不匹配、解压炸弹、Schema 不兼容、Worker 取消、磁盘回执丢失、崩溃于 Cut/Encode/Verify 各阶段。
- **性能指标**：Cut 停顿、Encode/Decode 吞吐、压缩 CPU/内存、Diff 放大率、恢复时间和对 Tick p99 的影响。

## 对应 ADR、Schema 与 Fixture

- 架构源 `docs/adr/ADR-003-cross-world-txn.md`：SnapshotCut 与 Revision 一致性。
- 架构源 `docs/adr/ADR-010-persistence-config.md`：Canonical Serializer、校验和配置快照。
- 架构源 `schemas/snapshot-header.schema.json`：正例 `fixtures/valid/snapshot-active.json`；反例 `fixtures/invalid/snapshot-length-mismatch.json`。
- Voxel Snapshot/Chunk Payload Schema 尚未发布；本文不替代架构源生成器。

## 尚未批准的决策门

- **VOX-D-005**（Pin/COW、Diff 粒度与 Snapshot 并发预算）：临时保证 Cut 一致性并允许两种实现；需旧版本、损坏、崩溃点和性能基准后确认。
- **D-005**（整体 Snapshot/WAL 耐久级别）由架构源/Host 决定，本模块只提供等价 Canonical bytes。
