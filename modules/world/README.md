# world 模块

> VoxelWorld 实例组装、Authority/Replica Role、Context/Handle 生命周期、Barrier 入口与模块协调。

## 模块定位与目标

`world` 是 LumioVoxelEngine 的运行期组合根和 VoxelWorld 生命周期所有者。它创建一个独立的 Authority 或 Replica 世界，组装各模块并提供版本化 Port 的入口；它保存句柄和 Context，不把 Chunk Storage、Revision 表或投影缓存上移到 Runtime/Host。Host 负责进程级创建/销毁编排，Runtime 负责 Logical Tick/Coordinator，`world` 负责实例内部状态转换和一致性闸门。

## 负责什么

- 创建、初始化、Ready、运行、Quiesce、Capture 路由、restore 入口、迁移配合和销毁一个 VoxelWorld 实例。
- 校验 Role、WorldId、Context、Capability、Schema/ABI 和资源预算，然后组装 `chunk/revision/query/mutation`。
- 按能力挂接 `snapshot/streaming/spatial/mesh-collision`，记录模块句柄和初始化顺序。
- 提供唯一的 `IVoxelWorldPort`/Reference Port 入口，把 Query、Prepare、Commit、Abort、Capture 和取消转交给正确模块。
- 维护 Voxel 侧 Simulation Barrier/Generation 闸门，拒绝销毁后或 Context 不匹配的迟到结果。
- 转发 Host `DurabilityAck` 到 `chunk.clear_dirty`；转发 restore 字节到 `snapshot.decode` 再物化进 `chunk`/`revision`。
- 在 LocalEmbedded 中创建两棵完全独立的 World 树，验证不共享 Storage、Lock、Buffer 或 Revision 写入。

## 明确不负责什么

- 不拥有 Chunk/Block 数据、Revision 计数、Query 结果、Reservation、Pin 记录、投影缓存或 `SnapshotCut`。
- 不决定 Host Wall Clock、Logical Tick Phase、跨域 Cut、CrossWorld 的 Game/ECS 提交顺序或最终 Gameplay 权限。
- 不直接操作 C# ECS/Session/Connection，不调用 Hot Gameplay；跨边界只使用版本化 Port/Generated Contract。
- 不执行文件 fsync、WAL、Release 路由或进程级恢复；只向 Host/Runtime 提供 Capture/恢复接口。
- 不把 `migration` 的业务 DAG 当作 Tick 内部状态机；迁移由维护/工具编排驱动。
- 不编译依赖 `LumioCoreEngine`。

## 拥有的状态与资源

- `VoxelWorldHandle`、`WorldId`、Role、Context/Generation 和实例生命周期状态。
- 子模块注册表、初始化/析构顺序、Barrier 闸门和异步任务取消源。
- World 级资源预算视图、健康状态和 Capability view。
- Port 请求路由和销毁后的句柄失效表。

## 输入、输出与稳定接口

- **输入**：Host 创建参数（Role/WorldId/Capability/预算）、Runtime Port 调用、Barrier Tick 入口、已固定的 `SnapshotCut`、Host `DurabilityAck`、Quiesce/Destroy 指令、restore 字节。
- **输出**：不透明 `VoxelWorldHandle`、Ready/Running 状态、Port 结果、`VoxelCaptureRef`、稳定故障和诊断事件。
- **接口草案**（公共 ABI/字段待架构源冻结）：`create_world(config) -> VoxelWorldHandle | StableError`；`query(handle, request)`；`prepare_mutation(handle, batch)`；`commit(handle, txn_id, token)`；`abort(handle, txn_id, token, reason)`；`capture(handle, cut) -> VoxelCaptureRef`；`apply_durability_ack(handle, ack)`；`restore(handle, decoded)`；`quiesce(handle, reason)`；`destroy(handle)`。

## 依赖（编译 / 控制流 / 事件与数据）

- **编译依赖**：已启用的领域模块；`LumioNativeCore` 已发布 Handle/Buffer API；架构源生成契约。不编译依赖 `LumioCoreEngine`、Runtime、Server、Client 或 Game 源码。CoreEngine 只出现在运行时加载与发布组合中。
- **被谁调用**：Host/Runtime 发起创建、Barrier、Capture、DurabilityAck、Quiesce、Destroy。
- **发布/消费**：向 Host 交出 `VoxelCaptureRef`/Canonical bytes 元数据；消费 Runtime 固定的 `SnapshotCut` 与 Host 耐久回执。不缓存子模块领域状态。

## 生命周期与状态机

公共 WorldSlot/Session 生命周期由架构源定义；本模块细化 VoxelWorld 实例状态：

```text
Created -> Initializing -> Ready -> Running <-> Quiescing
Running/Quiescing -> Snapshotting | Reloading | Migrating
Snapshotting/Reloading/Migrating -> Running | Stopping
Stopping -> Destroyed
Created/Initializing/Ready/Running/Quiescing/... -> Faulted
```

- `Ready` 仅表示所有必需 P0 模块初始化完成；可选 P1/P2 能力缺失必须由 Capability 明确声明。
- `Quiescing` 先关闭新 Ingress/写入，再排空或取消请求；Cut 仍由 Runtime 固定，本模块只停止新写入并准备 Capture。
- `Destroyed` 后所有 Handle、Token、View 和异步结果都以稳定错误失效，不能写入新实例。
- 任一步初始化失败都按逆序释放已成功初始化的模块，不留下半初始化 World。

## 线程、队列与并发所有权

- Host/Runtime 提供 Simulation Owner Thread；`world` 维护 Barrier 入口和 Context Generation，不另行读取 Wall Clock。
- 只有 Barrier 能调用子模块的权威写入、CommitBatch publish 和状态迁移；查询/构建 Completion 必须在所属 Role 的声明 Phase 发布，不得写入新 Context。
- `world` 负责汇总子模块取消源和有界任务句柄；不拥有 Native Job/IO Worker 的内部线程。
- 初始化、Quiesce、Destroy 操作串行化；重复调用幂等，销毁后的迟到调用拒绝。

## 正常数据流与失败路径

- **创建**：校验 Manifest/ABI/Capability/预算 → 建立 Context → 初始化基础模块 → 挂接可选模块 → `Ready`。
- **运行**：Runtime Port 调用 → Context/Role/预算检查 → Query 或 Mutation → Barrier 提交 → 返回 Revision/错误。
- **运行中快照**：接收 Runtime 已固定的 Cut → `revision` Pin/COW → 取得 `VoxelCaptureRef` → 立即恢复权威写入 → `snapshot` 后台编码 → 交 Host 持久化。
- **Quiesce/维护快照**：关闭新写入并排空请求后，再走同一套 Cut → CaptureRef 路径。
- **恢复**：Host 字节 → `snapshot.decode` → `chunk` 物化页 + `revision` 恢复 Stamp；不走 Streaming Load。
- **耐久回执**：Host `DurabilityAck` → Barrier → `chunk.clear_dirty`。
- **关闭**：取消新请求 → 完成/中止 Reservation → 停止 Streaming/投影任务 → 释放 Pin/Views → 逆序释放模块 → `Destroyed`。
- **失败路径**：模块初始化/状态迁移/Context 校验/资源预算失败进入明确 `Faulted`；已创建资源按逆序清理并保留 Failure Bundle 素材。

## 错误分类、恢复与降级

- **可重试**：P1/P2 可选模块暂时不可用（仅在 Capability 允许的情况下延迟挂接）；核心 P0 初始化不隐式降级。
- **可拒绝**：Role/Schema/ABI/Capability 不匹配、预算不足、非法状态调用、过期 Handle/Token。
- **可致命**：Context/Storage 不变量破坏、无法保证 World 隔离或 Barrier 一致性；实例停止并由 Host/Runtime 恢复。
- **降级**：只允许 Capability 声明的 Reference/Native、无 Spatial/Mesh 等明确能力差异；不得把缺 Chunk、Snapshot 失败或 Mutation 冲突当成功。

## 配置、Capability 与安全约束

- Role、WorldId、Schema/ABI、资源预算和启用模块来自不可变配置/Manifest；运行期只在 Tick 边界切换快照。
- Handle 使用 Index+Generation+Context 语义；不暴露指针、对象引用或内部地址。
- Server、Client、LocalEmbedded 各自创建 World；Local 不得通过同进程捷径跳过序列化、权限、大小限制和有界队列。
- 公共契约变化必须先在架构源新增 ADR/Schema/Fixture/Baseline，再更新本模块。

## 日志、Metrics、Trace 与 Audit

- Audit：World 创建、Ready、Quiesce、Snapshot、迁移配合、Faulted、Destroy（关联 `worldId/role/sessionId/tickId/snapshotId/traceId`）。
- Metrics：实例数、状态驻留时长、初始化/销毁耗时、请求/任务数、资源水位、异步取消和迟到拒绝。
- `Faulted` 产出 Failure Bundle 片段：模块状态、Generation、最后 Revision、活跃 Pin/Reservation 和队列水位。

## 测试面、故障矩阵与性能指标

- **测试面**：完整状态机、逆序清理、重复 Destroy、迟到 Handle 拒绝、Port 路由、Role 隔离、Local 双实例无共享引用、Reference/Native 一致性。
- **故障矩阵**：任一 P0 初始化失败、P1 挂接失败、预算不足、Barrier 取消、Snapshot 失败、Streaming/构建任务泄漏、Context Generation 复用。
- **性能指标**：World 冷启动到 Ready、Quiesce/Snapshot 停顿、销毁耗时、Port 路由开销和 1/10/25/50/100/150/200 Bot 场景的 Tick 贡献。

## 对应 ADR、Schema 与 Fixture

- 本仓 [0001](../../.spec/decisions/0001-snapshotcut-vs-capture-ref.md)、[0002](../../.spec/decisions/0002-barrier-commit-batch.md)、[0004](../../.spec/decisions/0004-snapshot-short-barrier-vs-quiesce.md)。
- 架构源 `docs/adr/ADR-001-session-lifecycle.md`：World/Role/Host 所有权和销毁顺序。
- 架构源 `docs/adr/ADR-002-tick-determinism.md`：Simulation Owner Thread 与 Barrier。
- 架构源 `schemas/native-managed-abi.schema.json`：Root API/Handle/错误边界；正例 `fixtures/valid/native-managed-abi.json`。
- 架构源 `schemas/host-capability.schema.json`：Role/Capability；正例 `fixtures/valid/host-capability.json`。
- VoxelWorld 专属 Port/状态 Schema 尚未发布；本文接口是本仓内部草案。

## 尚未批准的决策门

- 无独立数值决策门。World 的公共状态机、Handle 布局、ABI 和 Capability 变化必须回架构源；模块组合顺序可通过本仓 ADR 细化并同步模块地图。
