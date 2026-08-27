# spatial 模块

> Voxel 候选、遮挡投影、空间 Source 与带 Revision 的空间查询结果。

## 模块定位与目标

`spatial` 把 VoxelEngine 拥有的 Chunk/Block/遮挡信息投影为可供 Runtime/Server 继续过滤的空间候选。它是 Voxel-aware 的数据源，不是通用空间 Kernel，也不是最终 AOI/权限裁决器。每个结果都必须带读取 Revision、批次上限、预算、超时和取消原因，防止上层误用过期空间数据。

## 负责什么

- 在指定 World/Chunk Revision 上执行候选点、体素体积、遮挡和邻域投影。
- 组合 Chunk 可用性和 Block 几何信息，生成 `VoxelInterestCandidateBatch`/`VoxelSpatialProjection` 草案。
- 调用 `LumioNativeCore` 通用空间 Kernel，并隔离其 Buffer/Job 类型。
- 管理带 Revision 的投影缓存、脏化和失效；缓存键不得只依赖坐标。
- 提供有界批次、预算、deadline、取消和稳定的缺数据结果。
- 为 AOI、碰撞预筛选和 Headless Benchmark 提供可重放输入与诊断。

## 明确不负责什么

- 不决定最终 Interest、Role、Owner、Permission、带宽或隐身规则；这些由 Runtime/Server 过滤。
- 不拥有通用 Spatial/Collision 算法、Chunk 数据或 Revision（归 NativeCore、[chunk](../chunk/README.md)、[revision](../revision/README.md)）。
- 不执行 Gameplay 写入、不修改 ECS、不创建 Entity，也不触发网络发送。
- 不把不可用 Chunk 当作无遮挡/空空间，不返回内部指针或缓存句柄给 C#。
- 不承诺跨平台浮点位级一致；确定性规则由契约与 Benchmark 明确。

## 拥有的状态与资源

- 投影请求表、批次 Buffer、取消令牌和预算账本。
- `ChunkId + ChunkRevision + QueryShape + Capability` 组成的缓存键与失效记录。
- NativeCore Job/Buffer Adapter 句柄和结果 Revision。
- 遮挡/候选诊断摘要与差异测试输入。

## 输入、输出与稳定接口

- **输入**：空间范围/射线/体积、目标 Revision 或最新视图、Chunk 可用性、预算、deadline、cancel token。
- **输出**：候选/投影批次（带 Revision、Chunk 状态、截断原因）或稳定错误。
- **接口草案**（Voxel Spatial Schema 尚未发布）：`project(request) -> VoxelSpatialProjection | StableError`；`candidates(request) -> CandidateBatch | Pending`；`invalidate(chunk_id, revision)`；`cancel(handle)`。

## 上游与下游依赖

- **上游**：[world](../world/README.md)（Context/Capability）、[query](../query/README.md)（只读输入）、[chunk](../chunk/README.md) 和 [revision](../revision/README.md)。
- **下游**：Runtime/Server AOI、Collision 预筛选和诊断工具；不直接依赖 Gameplay。
- **基础依赖**：`LumioNativeCore` Spatial Kernel、Buffer 和 Typed Job。

## 生命周期与状态机

单次投影请求：

```text
Created -> Validating -> Reading -> Computing -> Completed
Reading -> Pending (Chunk 未就绪)
Created/Reading/Computing -> Cancelled | TimedOut | Rejected | Failed
```

缓存条目：`Absent -> Building -> Valid(Revision) -> Stale -> Evicted`。

- 目标 Revision 变化或 Chunk Generation 变化时，旧条目只能标记 `Stale`，不能继续作为新结果。
- World Quiescing 时取消新投影；已完成的批次仍需带原 Revision 并由上层决定是否丢弃。

## 线程、队列与并发所有权

- 查询读取在只读视图上执行；计算可交给有界 Native Job，Completion 在安全点发布。
- 投影 Worker 不持有 Chunk 写锁、不修改 Revision；缓存失效在 Barrier 或串行缓存上下文完成。
- 请求和结果队列有界，超限返回 `QueueFull`/截断原因；取消是幂等且必须阻止迟到结果入队。

## 正常数据流与失败路径

- **正常**：校验形状/预算 → 获取 Revision 一致读视图 → 检查 Chunk 可用性 → NativeCore 计算 → 稳定排序/截断 → 发布带 Revision 的批次。
- **缺数据**：返回 `Pending` 或 `Unavailable` 和受影响 ChunkId，不假设空空间。
- **缓存失效**：Revision/Generation 不匹配时丢弃旧结果并按策略重算。
- **失败路径**：Native Job 错误、预算超限、超时、取消、浮点/几何不变量失败都返回分类原因并保留输入摘要。

## 错误分类、恢复与降级

- **可重试**：Chunk `Pending`、暂时 Job/Buffer 不足、缓存 Stale。
- **可拒绝**：范围/形状超限、Revision 不可读、预算/队列超限、Context 失效。
- **可致命**：检测到跨 World 视图或 Native Buffer 越界；取消该 World 投影并上报故障。
- **降级**：可返回截断批次、降低精度或等待 Chunk，但必须显式标注；不得静默扩大 AOI 或改变权限。

## 配置、Capability 与安全约束

- 最大候选数、空间范围、精度、Job 并发和缓存预算来自 Capability/不可变配置快照。
- NativeCore Adapter 只接收受限 typed Buffer；输入长度、坐标范围、分配和取消边界先校验。
- `ReferenceVoxelPort` 与真实 Native 实现必须共用结果语义；性能优化不能改变 Revision/缺失状态。
- 任何最终权限/AOI 过滤都必须在 Runtime/Server 完成，不能下沉为本模块配置开关。

## 日志、Metrics、Trace 与 Audit

- Diagnostic：请求形状、命中/未命中 Chunk、Revision、截断/取消、Native Job 失败。
- Metrics：候选数量、截断率、投影 p50/p95/p99、缓存命中率、Native CPU/内存、队列水位。
- Audit 仅由上层对 Gameplay/权限决定负责；本模块提供可关联的 `worldId/chunkId/worldRevision/chunkRevision/traceId`。

## 测试面、故障矩阵与性能指标

- **测试面**：稳定排序、Revision/Generation 缓存隔离、遮挡边界、缺 Chunk 三态、预算/取消/超时、Reference/Native Differential。
- **故障矩阵**：Chunk 在计算中卸载、旧 Revision 结果、QueueFull、Native Buffer 错误、浮点边界、OOM、取消竞态。
- **性能指标**：候选吞吐、投影尾延迟、缓存命中率、每候选字节、不同 Chunk 密度/AOI 半径下的 CPU/内存。

## 对应 ADR、Schema 与 Fixture

- 架构源 `docs/adr/ADR-014-platform-capability.md`：Capability/Preset 与实现能力声明。
- 架构源 `schemas/host-capability.schema.json`：`ReferenceVoxel`/`Native` 能力；正例 `fixtures/valid/host-capability.json`。
- 架构源架构正文 §10、§15：Voxel-aware AOI/Spatial 边界和 Benchmark 要求。
- Voxel Spatial Projection Schema 尚未发布；本文输出类型不构成公共 Wire 契约。

## 尚未批准的决策门

- **VOX-D-007**（Spatial Kernel Adapter、缓存键和精度）：临时缓存必须包含 Revision/Generation，通用算法留在 NativeCore；需 Differential、故障和性能基线。
