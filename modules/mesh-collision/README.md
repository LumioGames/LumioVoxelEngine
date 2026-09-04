# mesh-collision 模块

> Mesh/Collision Source 构建、缓存、脏 Section 失效和 Native 几何任务；不拥有 Gameplay 规则。
> 物理 crate：`lumio-voxel-project`（[0006](../../.spec/decisions/0006-crate-map.md) / [0007](../../.spec/decisions/0007-v1.4-implementation-baseline.md)）；P2。

## 模块定位与目标

`mesh-collision` 将稳定的 Voxel Section 视图转换为可供 Renderer/Physics Adapter 使用的 Mesh Source 与 Collision Source。它属于 P2 领域投影模块，必须保持与 World/Section/Revision 解耦：输出是带 Revision 的可丢弃缓存，不是 Gameplay 状态，也不是 Renderer/物理引擎的权威对象。

## 负责什么

- 从指定 Section/World Revision 的只读视图构建 Mesh Source、Collision Source 和变更区域摘要。
- 管理脏 Section、邻接依赖、构建任务、缓存键和 Revision/Generation 失效。
- 调用 NativeCore 通用几何/碰撞/压缩 Kernel，通过 Adapter 隔离第三方类型。
- 提供有界构建批次、取消、超时、预算和稳定失败结果。
- 为 Client 表现、Server 碰撞预筛选和 NativeHeadless Benchmark 输出可重放 Source。
- 在 Snapshot/Streaming 变化后安全丢弃旧 Source，避免迟到任务覆盖新结果。

## 明确不负责什么

- 不拥有 Section/Block/Revision 数据（归 [section](../section/README.md)、[revision](../revision/README.md)），不修改 World。
- 不直连 Section Storage；只经 [query](../query/README.md) ReadView 读取。
- 不创建 Renderer/Physics 对象，不决定材质、LOD、碰撞层、触发器、伤害或 Gameplay 规则。
- 不做最终 AOI/权限过滤（归 Runtime/Server；候选范围可来自 [spatial](../spatial/README.md) 事件，但不调用其裁决逻辑）。
- 不保证所有平台位级 Mesh 相同；确定性和容差必须在适配器/测试中声明。
- P2 未激活时，核心 P0/P1 Port 不得依赖本模块才能运行。

## 拥有的状态与资源

- `WorldContext/Generation + SectionId + SectionRevision + BuildProfile` 的 Source 缓存和失效表。
- 脏区域/邻接依赖、构建请求、取消令牌和有界几何 Buffer。
- NativeCore Job/Adapter 句柄、构建版本和失败摘要。
- Source 生命周期：可用、过期、驱逐；不持有下游 Renderer/Physics 所有权。

## 输入、输出与稳定接口

- **输入**：Section 只读视图、目标 Revision/Generation、BuildProfile、邻接 Section 可用性、预算/deadline/cancel。
- **输出**：`MeshSource`/`CollisionSource`（typed Buffer + Revision + bounds + build metadata）或稳定错误。
- **本仓 Port 表面**（仍无跨仓 Mesh/Collision Schema，保持 P2 文档边界）：`build_mesh(request) -> MeshSource | StableError`；`build_collision(request) -> CollisionSource | StableError`；`invalidate(section_id, revision)`；`cancel(build_id)`；`evict(cache_key)`。

## 依赖（编译 / 控制流 / 事件与数据）

- **编译依赖**：[query](../query/README.md)（ReadView）、[revision](../revision/README.md)、NativeCore 几何/碰撞/Buffer/Job。不依赖 section Storage、streaming 或 world。
- **被谁调用**：[world](../world/README.md)（能力/生命周期）。Renderer/Physics Adapter 只消费 Source。
- **发布/消费**：消费 `AvailabilityChanged` 与 `SectionChanged` 以失效缓存；可选消费 spatial 候选范围事件。不调用最终 AOI/权限，不控制 Load。

## 生命周期与状态机

单次构建：

```text
Queued -> Validating -> Reading -> Building -> Verifying -> Ready
Queued/Reading/Building/Verifying -> Cancelled | TimedOut | Rejected | Failed
Ready(Revision) -> Stale -> Evicted
```

- Build 完成后必须再次校验 Section Generation/Revision；不匹配的结果只能丢弃。
- World Quiescing/Destroy 时停止新任务并取消未完成任务；缓存清理不改变 Section/World 状态。

## 线程、队列与并发所有权

- 构建可交给有界 Native Job/Worker；Worker 只读 query ReadView，不持有写锁或调用 C#。
- Completion 在所属 Role 的声明 Phase 核对 Revision/Generation，再原子发布缓存条目。
- 构建队列、邻接等待表和几何 Buffer 池有界；满载按优先级取消/延迟并计数。
- Renderer/Physics Adapter 在模块外拥有消费对象；本模块只负责 Source Buffer 的生命周期和释放契约。

## 正常数据流与失败路径

- **正常**：校验 Profile/预算 → 经 query 获取稳定 Section/邻接 ReadView → NativeCore 生成 → 校验 bounds/Buffer/Revision → 发布 Source。
- **邻接缺失**：返回 `Pending/Unavailable` 或按 Profile 生成明确边界版本，结果必须标注缺失原因；不能默默使用旧邻接数据。
- **失效**：Section 写入、Streaming 卸载或 Revision 变化使缓存 `Stale`，取消/丢弃在途结果。
- **失败路径**：几何不变量、Buffer 超限、Native 错误、超时/取消都不影响 World 权威状态。

## 错误分类、恢复与降级

- **可重试**：邻接 Section `Pending`、暂时 Job/Buffer 不足、缓存 Stale。
- **可拒绝**：Profile 不支持、范围/网格超限、Revision/Generation 失效、P2 Capability 未启用。
- **可致命**：Native Buffer 越界、Source 校验器发现内存破坏；停止相关构建并上报 Native/进程故障域。
- **降级**：可显式降低细节或返回无 Source，由上层等待/隐藏表现；不得改变 Gameplay 碰撞规则或伪造已完成。

## 配置、Capability 与安全约束

- BuildProfile、细节/精度、并发、Buffer/内存预算和缓存保留来自 Capability/不可变配置。
- 外部/Section 数据先校验长度、坐标、bounds、分配和压缩限制；第三方 API 经 Adapter 隔离。
- P2 能力必须由 Host Capability 显式启用；P0/P1 语义不能依赖 Renderer/Physics SDK。
- LocalEmbedded、NativeHeadless 与 Client 表现使用同一 Source Revision/取消语义，不能共享权威 Section Buffer。

## 日志、Metrics、Trace 与 Audit

- Diagnostic：构建 Profile、Revision、邻接缺失、取消、缓存命中/失效、Native 错误。
- Metrics：构建 p50/p95/p99、三角形/碰撞面数量、Buffer/内存峰值、缓存命中率、QueueFull。
- Audit 通常由上层表现/物理系统负责；本模块提供 `sectionId/worldRevision/sectionRevision/buildId/traceId` 关联。

## 测试面、故障矩阵与性能指标

- **测试面**：Section 边界/邻接、Revision 缓存失效、稳定 Source Hash、取消/超时、空/全固体 Section、Reference/Native Differential。
- **故障矩阵**：邻接卸载、旧结果覆盖、Buffer/OOM、Native 错误、队列满、Profile 不支持、World 销毁竞态。
- **性能指标**：构建吞吐和尾延迟、每 Section CPU/内存、Source 大小/压缩比、缓存重建率和 100 Bot 场景影响。

## 对应 ADR、Schema 与 Fixture

- 本仓 [0006](../../.spec/decisions/0006-crate-map.md)、[0007](../../.spec/decisions/0007-v1.4-implementation-baseline.md)。
- 架构源架构正文 §2.1、§14.3、§15.3：Voxel Collision/Spatial 所有权、P2 边界和 Benchmark。
- 架构源 `schemas/host-capability.schema.json`：Native/平台能力声明；正例 `fixtures/valid/host-capability.json`。
- 仍无跨仓 Mesh/Collision Schema；本模块保持 P2 文档边界，不冻结 ABI。

## 尚未批准的决策门

- **VOX-D-007**（几何 Kernel Adapter、缓存键、精度/LOD 与 P2 激活）：临时只缓存带 World Context/Revision 的 Source，需 Native Differential、内存/性能和平台 Capability 评审。
