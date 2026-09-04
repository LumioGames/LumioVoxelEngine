# query 模块

> 有界只读批量查询、缺 Section 结果、读取 Revision、预算、超时与取消。
> 物理 crate：`lumio-voxel-ops`（[0006](../../.spec/decisions/0006-crate-map.md) / [0007](../../.spec/decisions/0007-v1.4-implementation-baseline.md)）；经 domain `ReadView` 读取。

## 模块定位与目标

`query` 是所有上层读取 Voxel 状态的唯一领域 API。它把坐标范围、Section 可用性、读取一致性、批次上限和取消语义组合成可审计的结果，供 Runtime、Spatial、Mesh/Collision 和 Headless 测试使用。它永远不产生可见写入，也不把内部 Storage 复制成 C# 第二真相。

## 负责什么

- 校验 Query 范围、点数、批次大小、Context/Generation 和调用能力。
- 在指定 Revision 或请求开始时固定的 Latest 可读视图上执行点查询、范围查询和批量候选读取。目标 Revision 在 `begin` 时绑定，后续批次与 continuation 不得改观察版本。
- 对每个结果返回读取 `WorldRevision/SectionRevision` 或明确的一致性令牌；多 Section 批次属于同一个已绑定 Revision。
- 区分 `Ready`、`Unchanged`、`Pending`、`Unavailable`、`OutOfBudget`、`Cancelled` 和 `TimedOut` 等结果类别；缺 Section 四态的名字以活契约 `lumio.voxel-world.v1` 的 `diffDispatch.presence` 为准（[0013](../../.spec/decisions/0013-voxel-world-contract-and-section-rename.md)），其余结果类别仍以架构源 `voxel-query` / ADR-024（`LGE-V1.4-2026-08-27`）为准。
- 维护每请求的预算、截止时间、取消令牌、批次计数和诊断上下文。
- 为 Spatial/Geometry 投影提供只读稳定 ReadView，不替上层做权限、AOI 或 Gameplay 过滤。

## 明确不负责什么

- 不加载或驱逐 Section（归 [streaming](../streaming/README.md)），不修改 Block（归 [mutation](../mutation/README.md)）。
- 不拥有 World/Section 生命周期或 Revision 递增（归 [world](../world/README.md)、[revision](../revision/README.md)）。
- 不把缺 Section 当作空值，不隐式等待无限时间，也不分配无界结果集合。
- 不做玩家权限、阵营、隐身、带宽或最终 AOI 判断；上层根据候选结果做最终过滤。
- 不泄漏 Storage 指针、锁、页地址或第三方容器。
- 不依赖 [spatial](../spatial/README.md) 或 [mesh-collision](../mesh-collision/README.md)。

## 拥有的状态与资源

- 活跃 Query 请求表（请求 ID、Context、预算、截止时间、取消状态）。
- 有界结果批次 Buffer 和按请求的已消费游标。
- Query 诊断计数、缺 Section 分类和读取 Revision 记录。

## 输入、输出与稳定接口

- **输入**：`QueryRequest`（范围/点集、目标 Revision、最大结果、预算、deadline、cancel token）、来自 `world` 的只读 Context。
- **输出**：`QueryBatch`（typed voxel result + `RevisionStamp` + Section 状态）、稳定错误/取消原因和 Metrics。
- **本仓 Port 表面**（一致性与缺 Section 四态见架构源 `voxel-query`）：`begin(request) -> QueryHandle | StableError`；`poll(handle, budget) -> QueryBatch | Pending | Done`；`cancel(handle, reason)`；`read_at(context, coord) -> VoxelRead | QueryStatus`。

## 依赖（编译 / 控制流 / 事件与数据）

- **编译依赖**：[section](../section/README.md)（ReadView）、[revision](../revision/README.md)（Stamp/Pin）、NativeCore 稳定错误。不依赖 spatial、mesh-collision、streaming 或 world。
- **被谁调用**：[world](../world/README.md) Port；[spatial](../spatial/README.md) 与 [mesh-collision](../mesh-collision/README.md) 经 ReadView 读取。
- **发布/消费**：消费 [streaming](../streaming/README.md) 的 `AvailabilityChanged`；不直接发 Load。Pending 恢复必须继续绑定 `begin` 时的目标 Revision；目标已被回收则返回稳定 stale/unavailable，不得改读最新。

## 生命周期与状态机

单个请求：

```text
Created -> Validating -> Running -> Completed
Running -> Pending -> Running
Created/Running/Pending -> Cancelled | TimedOut | Rejected
Running -> Failed
```

- `Pending` 只表示请求依赖的 Section 尚未可用，必须带截止时间和当前状态。
- `Completed` 的批次不可再追加；`Cancelled/TimedOut/Rejected` 后的迟到结果必须丢弃。
- World 进入 Quiescing/Closed 后，新 Query 拒绝，已运行请求收到明确取消原因。

## 线程、队列与并发所有权

- 轻量点查询可在 Simulation Owner Thread 直接读取只读视图；大批量查询可以交给有界 Native Job。
- Query Worker 不修改 Section/Revision；Completion 只在所属 Role 的声明 Phase 发布。
- 每请求和全局结果队列均有容量/预算/截止时间；队列满返回 `QueueFull` 或按调用方策略取消。
- 取消是协作式且幂等；销毁 Context 后不得继续访问内部视图。

## 正常数据流与失败路径

- **正常**：校验请求 → 在 `begin` 绑定目标 Revision（显式或 Latest-at-Acquire）→ 必要时 Pin/ReadView → 按批读取 Section → 生成结果 → 发布完成标记。
- **缺 Section**：查询返回 `Unchanged/Pending/Unavailable` 和相关 SectionId/Revision，不生成空 Block。
- **超预算/超时**：返回已完成批次加明确终止原因；不得静默截断为成功。continuation 仍绑定原 Revision。
- **失败路径**：Context 失效、视图 Generation 变化、目标 Revision 已回收、解压/读取错误或结果队列满时停止该请求并保留诊断。

## 错误分类、恢复与降级

- **可重试**：`Pending`、暂时 QueueFull、短暂 Native Job 资源不足（由调用方以新请求/游标重试）。
- **可拒绝**：范围越界、请求超过最大点数/分配预算、版本不兼容、Context 无效。
- **可致命**：检测到读视图破坏或跨 World 数据泄漏；上报 World 故障域并取消相关请求。
- **降级**：按明确的最大批次提前完成、返回缺失状态或取消；不能退化为读取过期/空世界。

## 配置、Capability 与安全约束

- 最大点数、批次字节、并发请求、deadline 和 Native Job 预算来自不可变配置快照。
- 反序列化/解压输入先校验长度、Hash/Checksum、边界和分配上限；不执行输入中的代码。
- ReferenceVoxelPort 可在 `PureHeadless` 使用相同 Query 语义；NativeHeadless 验证布局、预算和性能。
- Query 结果只包含必要 typed 数据和 Revision，不包含内部地址或权限结论。

## 日志、Metrics、Trace 与 Audit

- Diagnostic：请求拒绝、缺 Section 分类、超时/取消、QueueFull、读取 Revision。
- Metrics：请求数、批次大小、Pending 时长、p50/p95/p99、结果字节、预算命中率和取消率。
- 需要审计的跨域读取由 Runtime 关联 `txnId/sessionId`；Query 不把普通读取写成 Gameplay Audit。

## 测试面、故障矩阵与性能指标

- **测试面**：点/范围/批量查询、边界坐标、缺 Section 三态、Revision 传播、批次上限、取消/超时幂等、结果稳定排序。
- **故障矩阵**：Section 在读取中卸载、Generation 复用、QueueFull、解压失败、OOM、旧 Revision、Local 双实例串读。
- **性能指标**：单点/批量吞吐、结果排序成本、Pending 等待尾延迟、每请求分配量、Native/Reference Differential 差异。

## 对应 ADR、Schema 与 Fixture

- 本仓 [0006](../../.spec/decisions/0006-crate-map.md)、[0007](../../.spec/decisions/0007-v1.4-implementation-baseline.md)。
- 架构源 `docs/adr/ADR-003-cross-world-txn.md`：读取 Revision 与 Section 可用性前置条件。
- 架构源 `schemas/common.schema.json` / `schemas/session-revision-vector.schema.json`：Revision 结构。
- 架构源 `schemas/voxel-query.schema.json`：一致性模式、continuation 绑定、缺 Section 多态；ADR-024。批次/预算默认值仍属 VOX-D-003。

## 尚未批准的决策门

- **VOX-D-003**（Query 批次、预算默认值）：一致性与缺 Section 四态已冻结；容量与超时默认值待 Bench。
