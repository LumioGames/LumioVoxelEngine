# streaming 模块

> Chunk Load/Unload、优先级、资源预算、取消、背压与 Chunk 可用性发布。

## 模块定位与目标

`streaming` 管理 Chunk 从不可用到可读、再到可驱逐的异步生命周期。它把请求排队、预算、优先级、IO/解压完成和取消结果连接起来，但不直接改变 World 生命周期，也不让后台线程绕过 Barrier 写入 Chunk/Revision。Query 依赖它发布的可用性状态来区分缺失原因。

## 负责什么

- 接收带范围、优先级、截止时间、目标 Revision 和资源预算的 Load/Unload 请求。
- 维护有界优先级队列、并发 Load/Unload 数和每 Chunk 的任务状态。
- 调度 Storage Adapter、解压和校验任务；把完成/失败事件送回 Barrier，由 [chunk](../chunk/README.md) 发布状态。
- 执行背压、取消、超时、重复请求合并和 Chunk 驱逐前置检查。
- 产出 `Ready/NotLoaded/Pending/Unavailable` 可用性视图和 Load Failure 证据。
- 为 Snapshot 部分加载、Migration 工具和测试 Host 提供可控的加载/卸载能力，不决定业务 AOI。P0 全驻留 Profile 可禁用 Unload。

## 明确不负责什么

- 不拥有 Chunk/Block 数据和页布局（归 [chunk](../chunk/README.md)），不递增 Revision（归 [revision](../revision/README.md)）。
- 不决定玩家 Interest、权限、带宽、渲染距离或 Gameplay 优先级；上层只提供已归一化的技术优先级。
- 不在 IO Worker 直接写权威状态，不把失败 Chunk 当空世界。
- 不负责 Snapshot/WAL 文件耐久、Migration 激活或 World 实例销毁。
- 不得把未获 Host DurabilityAck 的 Dirty Chunk 驱逐为 `Unloaded`。
- 不建立无界缓存、无界重试或不可取消的后台任务。

## 拥有的状态与资源

- Chunk Load/Unload 请求表、优先级队列、去重键和取消令牌。
- 每 Chunk 的任务状态、当前 Generation、预算占用和最后失败原因。
- IO/解压并发额度、页 Buffer 租约和背压计数。
- 可用性发布游标与 Streaming Metrics。

## 输入、输出与稳定接口

- **输入**：`LoadRequest`/`UnloadRequest`（ChunkId、优先级、deadline、预算、cancel token）、Storage Adapter 回调、World Quiesce 指令。
- **输出**：`LoadHandle`、Chunk 可用性事件、完成页/失败原因、预算/队列水位。
- **接口草案**（公共 Streaming Schema 待发布）：`request_load(request) -> LoadHandle | StableError`；`request_unload(chunk_id, reason)`；`cancel(handle)`；`poll_status(chunk_id) -> Availability`；`drain(budget) -> CompletionBatch`。

## 依赖（编译 / 控制流 / 事件与数据）

- **编译依赖**：[chunk](../chunk/README.md)（页物化/驱逐）、NativeCore Buffer/Compression/Job、Storage Adapter。不依赖 query、snapshot、revision 或 world。
- **被谁调用**：[world](../world/README.md)（启动/Quiesce/Destroy）、Runtime/Host（预算和技术优先级）。
- **发布/消费**：向 query / spatial / mesh-collision 发布 `AvailabilityChanged`。不消费 mutation 的方法调用。Unload 必须看到 Host DurabilityAck 已清除 Dirty，或 Capability 声明该 Chunk 可丢失。

## 生命周期与状态机

单个 Load 请求：

```text
Queued -> Validating -> Loading -> Verifying -> PublishReady
Queued/Loading/Verifying -> Cancelled | TimedOut | Rejected | Failed
```

单个 Chunk 的公共可用性由 `chunk` 细化；Streaming 只驱动其转换：

```text
NotLoaded -> Pending -> Ready
Ready -> Evicting -> NotLoaded
Dirty -> EvictionRequested -> (DurabilityAck or explicit volatile Capability) -> Evicting -> NotLoaded
Pending/Ready/Evicting -> Unavailable
```

- `PublishReady` 必须在 Barrier 完成 Generation 校验后才对 Query 可见。迟到 Completion 必须同时匹配 World Context/Generation、Chunk Generation 和 RequestId。
- 重复 Load 可合并到同一 Handle；取消一个消费者不能误取消仍被引用的任务。
- World Quiescing 时停止新 Load，完成或取消已有任务。
- Dirty Unload 默认拒绝；Dedicated Server 不得驱逐未获恢复保障的 Dirty Chunk。

## 线程、队列与并发所有权

- 拥有配置数量的 IO/解压 Worker；Worker 只读输入并生成完成事件。
- Load/Unload 队列、Completion 队列和页 Buffer 池均有容量、优先级、满载动作和 Metrics。
- Barrier 线程负责发布 Chunk 状态、Generation、Revision 关联和最终驱逐；后台线程不得持有 WriteView。
- 取消、超时和 World 销毁通过有界信号传播；迟到 Completion 以 Generation 拒绝。

## 正常数据流与失败路径

- **加载**：请求规范化 → 排队/预算检查 → 读取页 → 长度/Hash/解压校验 → Barrier 物化 → 发布 `Ready`。
- **卸载**：停止新写入 → 等待读视图/Pin/构建任务 → 若 Dirty 则等待 DurabilityAck 或显式 volatile Capability → Barrier 标记 `NotLoaded`。
- **背压**：队列/内存/IO 超限按优先级延迟、拒绝或取消；Query 收到明确 `Pending/Unavailable`。
- **失败路径**：IO、校验、解压、超时、Generation 失效和磁盘压力都保留 ChunkId/请求证据，不回退为空 Chunk，不静默丢掉 Dirty。

## 错误分类、恢复与降级

- **可重试**：瞬时 IO、暂时队列/Buffer 不足、远端 Storage 短暂不可用（有限次数、带退避）。
- **可拒绝**：预算不足、队列满、无效优先级/范围、Chunk 已销毁、取消或 deadline 到期。
- **可致命**：重复校验失败、Storage 破坏或无法保证 Generation 隔离；Chunk/World 进入故障路径。
- **降级**：只允许返回 `Pending/Unavailable`、降低并发或丢弃低优先级请求；不静默返回空数据。

## 配置、Capability 与安全约束

- 并发 Load、队列容量、页/解压预算、重试次数和背压阈值来自不可变配置快照。
- 所有页在分配前校验长度、Hash/Checksum、解压比和最大 Chunk 大小；外部数据不可执行。
- `ReferenceVoxelPort` 可提供确定性加载替身；真实布局/性能必须在 `NativeHeadless` 验证。
- LocalEmbedded 与 RemoteDS 复用同一状态和错误语义，不能因本地模式跳过校验或队列。

## 日志、Metrics、Trace 与 Audit

- Diagnostic：请求排队/合并、Load/Unload 阶段、QueueFull、取消、超时和失败原因。
- Metrics：队列深度、命中/合并率、Load/Unload p50/p95/p99、IO/解压耗时、内存/页水位、重试次数。
- Audit/Failure Bundle：ChunkId、World/Chunk Revision、请求优先级、Storage 版本、失败阶段和 TraceId。

## 测试面、故障矩阵与性能指标

- **测试面**：优先级稳定排序、重复请求合并、取消/超时、Ready 发布顺序、Unload 前置条件、缺 Chunk 三态、背压。
- **故障矩阵**：IO 失败、截断/损坏页、解压炸弹、QueueFull、OOM、迟到 Completion、World 销毁竞态、磁盘满。
- **性能指标**：冷/热 Load 延迟、可用性发布尾延迟、吞吐、队列等待、内存峰值、100 Bot AOI 负载下的命中率。

## 对应 ADR、Schema 与 Fixture

- 架构源 `docs/adr/ADR-001-session-lifecycle.md`：Quiesce/销毁顺序和 World 生命周期背景。
- 架构源 `docs/adr/ADR-010-persistence-config.md`：Snapshot/存储校验与配置快照。
- 架构源 `schemas/host-capability.schema.json`：`Native`/`ReferenceVoxel` 能力；正例 `fixtures/valid/host-capability.json`。
- Streaming/Chunk 专属 Schema 和失败 Fixture 尚未发布；VOX-D-003/006 确认前不冻结公共枚举。

## 尚未批准的决策门

- **VOX-D-006**（优先级、并发、队列容量和背压阈值）：临时所有队列有界，低优先级可拒绝/取消；P0 可禁用 Unload。需 NativeHeadless 压测、OOM/QueueFull 故障和 100 人基线。
