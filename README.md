# LumioVoxelEngine

> 可复用的 Rust VoxelWorld 领域实现、Chunk 数据域和空间数据源。

## 架构基线

- Baseline：`LGE-V1.0-2026-08-27`
- 唯一架构源：`LumioGameEngineArchitecture`
- 本地镜像：[`docs/architecture/LumioGameEngine_Architecture_v1.0.md`](docs/architecture/LumioGameEngine_Architecture_v1.0.md)

本仓库拥有 VoxelWorld 的权威数据和领域生命周期。Server 保存权威世界，Client 保存独立 VoxelReplicaWorld；LocalEmbedded 也必须创建两份实例。C# Runtime 只能通过版本化 `IVoxelWorldPort` 和生成契约访问，不能读取内部 Chunk Storage。

## Architecture Gate

Voxel Contract、Revision/Snapshot Schema、ID Registry、正向/失败 Fixture 和契约校验器只维护在 `LumioGameEngineArchitecture`。Chunk/World 格式变更必须先更新源 Schema、Migration/Fixture 和 Baseline，再生成本仓库使用的只读产物；校验命令为 `python3 tools/lumio_contract.py validate`（在架构源执行）。

## 拥有的状态与生命周期

- World、Chunk、Block、坐标、加载/卸载和 Streaming 状态。
- 单调 `WorldRevision`、每 Chunk `ChunkRevision`、Mutation Batch 和 Snapshot Cut。
- Mesh Source、Collision Source、Spatial Source 的缓存和构建任务。
- `VoxelWorldHandle`、`VoxelChunkHandle`、ChunkId 及其 Generation/Context 生命周期。

Host 负责创建和销毁实例，VoxelEngine 负责实例内部状态转换和数据一致性。Runtime Coordinator 只能通过 Port 发起查询、Prepare、Commit 和取消，不拥有 Voxel 状态机。

## 子模块

| 子模块 | 责任 | 首批状态 |
| --- | --- | --- |
| `world` | VoxelWorld 实例、权限域和实例生命周期 | P0 |
| `chunk` | Chunk 布局、Block、坐标、压缩页和加载状态 | P0 |
| `revision` | World/Chunk Revision、比较和 Snapshot Pin | P0 |
| `query` | 只读批量查询、缺 Chunk 结果和版本返回 | P0 |
| `mutation` | 单域 Mutation、Prepare Token、幂等 Commit/Abort | P0 |
| `snapshot` | Snapshot/Diff、Canonical 编码、校验和恢复 | P1 |
| `streaming` | Load/Unload、优先级、预算、取消和背压 | P1 |
| `spatial` | Voxel 候选、遮挡和带 Revision 的空间 Source | P1 |
| `migration` | Chunk/World Schema 转换、校验和失败保留 | P1 |
| `mesh-collision` | Mesh/Collision Source 构建，不拥有 Gameplay 规则 | P2 |

## 职责

- 定义 VoxelWorld、Chunk、Block、Revision、Mutation、Snapshot/Diff 和 Streaming 的领域 Schema。
- 提供有界、可取消、版本化的只读 Query 和批量 Mutation API。
- 实现 Voxel 侧 Prepare/Reservation/Commit/Abort 参与者；Coordinator 语义由 Runtime 统一编排。
- 提供带 `WorldRevision/ChunkRevision` 的 Voxel Spatial/Collision/Chunk 候选结果。
- 生成 Native ABI 源元数据、Voxel Contract 输入、序列化 Fixture 和恢复测试。
- 为 Server 权威、Client Replica、NativeHeadless 和 Local 双实例提供适配器。

## 明确不负责什么

- 不实现 Ability、Effect、Attribute、Tag、背包、权限、扣费、战斗或其他 Gameplay 判断。
- 不创建或直接修改 C# ECS Entity/Component、Session 或 Host 状态。
- 不拥有 Connection、RPC 路由、Release Pool、CoreCLR、Socket 或进程治理。
- 不把玩家权限、阵营、隐身、订阅优先级等产品语义放入 Voxel Kernel。
- 不把完整 Chunk 复制到 C# 作为第二权威真相。

## Revision 与读取一致性

`WorldRevision` 用于世界级排序和 Snapshot；`ChunkRevision` 用于局部乐观并发。所有 Query 返回读取 Revision，Mutation 携带 Expected Revision；冲突必须返回稳定 `RevisionConflict`，不能静默覆盖。

在协调 Snapshot Cut 中，VoxelEngine 对指定 Revision 做 Pin 或 Copy-on-Write。异步序列化期间 Chunk 可以继续服务读取，但不得把变化后的数据标记为旧 Snapshot。缺 Chunk 的 Query 必须明确返回 `NotLoaded`、`Pending` 或 `Unavailable`，不能当作空世界。

## CrossWorldTxnV1 参与者

Voxel Prepare 只验证 Chunk 可用性、Cell 可写性、Expected ChunkRevision、容量和权限所需的结构条件，并创建有租约的不可见 Mutation Reservation。V1 在 `CommitIntent` 持久化后先执行 Voxel Apply，再由 Runtime 提交 Game/ECS；Commit 使用 `TxnId` 幂等应用并返回新的 World/Chunk Revision，重复 Commit 返回原结果。Native 锁内不能调用 C#，Worker 不能回调 Hot Gameplay。

## AOI、Streaming 与空间边界

NativeCore 提供通用空间 Kernel；本仓库根据 Chunk/Block/遮挡/可用性生成 `VoxelInterestCandidateBatch` 或 `VoxelSpatialProjection`。Runtime/Server 再结合 Role、Owner、Permission、带宽和 Interest 做最终过滤。结果必须包含 Revision、批次上限、查询预算、超时和取消原因，不暴露内部指针或 Storage。

## 序列化、存档与恢复

- Chunk、World Snapshot、Diff 和 Migration 输入使用版本化 Canonical Serializer。
- Envelope 至少包含 Magic、SchemaVersion、Length、World/Chunk Revision、Hash/Checksum、Compression 和可选加密信息。
- Snapshot 采用临时文件、校验、fsync/原子替换；WAL/Command Log 由 Host 追加并关联 Txn/Session。
- 支持部分 Chunk 加载、流式加载、旧版本 Migrator、损坏检测和从最近有效 Checkpoint 恢复。
- Singleplayer 与 Dedicated Server 在同一 Release 使用同一 Voxel 存档格式；跨版本必须通过迁移工具。

## 日志与观测

输出 Voxel Diagnostic、Streaming Metrics、Revision/Mutation Audit 和 Trace Event，不拥有最终日志 Sink。事件至少带 `GameReleaseId、SessionId、WorldId、TickId、TxnId、ChunkId、WorldRevision、ChunkRevision、TraceId`。Load Failure、Revision Conflict、Snapshot Corruption、Migration Failure 和 QueueFull 必须有稳定错误码和 Failure Bundle 片段。

## Source / Compile-Time Dependencies

- `LumioNativeCore`：通用 Handle、空间、碰撞、压缩、Buffer 和 Typed Job Kernel。
- Rust toolchain、平台 SDK 和经过供应链审查的通用 crates。
- 不依赖 `LumioGameRuntime`、Server、Client 或 Game 源码；Port 依赖只通过版本化契约。

## Generated Contract Dependencies

本仓库拥有 Voxel 源 Schema、ABI 元数据、Revision/Error/Capability 定义和行为 Fixture。`LumioCoreEngine` 负责聚合 Header、统一版本前缀和最终托管绑定；不得同时维护第二套 P/Invoke 签名。Runtime 只消费 `IVoxelWorldPort`/生成物。

## Runtime Loading Relationships

```text
LumioCoreEngine package
  -> Host Loader
  -> VoxelWorld instance (authority or replica)
  -> Runtime IVoxelWorldPort / generated Voxel Contract
```

Server/Client/Local 分别创建实例；Local 的两份世界不能共享对象引用、Chunk Buffer 或 Revision 写入。

## Release Composition Relationships

发布 Voxel Schema、Chunk 格式、压缩字典、Migration 版本、Artifact Hash、NativeCore 依赖和平台能力。`GameManifest` 锁定 Voxel ABI/Schema/Migration；不负责 Product Release 路由或 Gameplay 语义。

## Room Modes / Host Profiles

向 `PublicDedicatedServer`、`PlayerHostedDedicatedServer`、`LocalhostDedicatedServer`、`LocalEmbedded`、`PureHeadless`、`NativeHeadless`、`LocalSplitProcess`、`RemoteDS` 和 `MobileLocal` 提供同一 Port 语义。Scenario 是否需要真实 Native/Streaming 由 Capability 声明决定，Reference Port 只用于语义测试。

## Headless Test Surface

- Chunk/坐标/边界/Revision/Mutation/Reservation/幂等和冲突 Property/Golden Test。
- Snapshot/Diff、Canonical Serialization、压缩、损坏、恢复和 Migration Fixture。
- Load/Unload/Streaming 背压、取消、超时、缺 Chunk Query 和资源预算。
- Reference Voxel Port 与真实 Native 实现的 Differential Test。
- Voxel Spatial/AOI/Collision Benchmark，记录 Chunk 密度、AOI 半径、队列和内存。
- Fault：Chunk Load Failure、Revision Conflict、Lost Result、Snapshot Corruption、Migration Failure、OOM、磁盘满。

## Version / Manifest

Manifest 至少包含 Voxel API/ABI、World/Chunk Schema、压缩字典、Migration、平台 Artifact Hash、NativeCore 版本和 Capability。启动、迁移和重连前校验；不匹配返回稳定错误并拒绝使用不兼容数据。

## 开源优先与供应链

优先复用成熟 Voxel 数据结构、压缩、异步 IO 和序列化方案，但必须通过许可证、维护、漏洞、确定性、AOT 和性能验证。第三方 API 经 Adapter 隔离，锁定 Commit、记录 SBOM 和许可证；领域状态和跨 World 语义仍由本仓库负责。

## 开发规范

- 权威修改只能在 VoxelWorld 所属 Role 的 Simulation Barrier 执行。
- 不持有对方 World 的指针、锁或 Storage 引用；异步结果必须可取消且带 Revision。
- 每个破坏性 Chunk/Revision 变化都要有旧版本 Fixture、Migration 和失败恢复路径。
- Voxel-aware 优化记录数据密度、AOI、Streaming、CPU、内存和结果版本，不下沉 Gameplay 判断。

## 当前阶段与开发节奏

1. **Architecture Gate**：冻结 World/Chunk/Revision/Mutation/Snapshot Schema 和 Port 错误语义。
2. **Foundation**：实现 `world/chunk/revision/query/mutation` 单域闭环和 NativeHeadless 测试。
3. **Vertical Slice**：接入 CrossWorldTxn、Local 双实例、Snapshot/WAL 和 Reference Differential。
4. **Production Hardening**：Streaming/AOI/恢复/Migration/损坏注入和性能曲线。
5. **P2**：复杂空间优化、可替换后端和跨服迁移；不改变 V1 权威边界。
