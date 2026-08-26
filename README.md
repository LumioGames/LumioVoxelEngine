# LumioVoxelEngine

> 可复用的 Rust VoxelWorld 领域实现与 Chunk 权威域。

## 定位

`LumioVoxelEngine` 拥有完整 `VoxelWorld` 的数据和生命周期。Server 维护权威 VoxelWorld；Client 维护独立的 VoxelReplicaWorld/预测视图；Local 模式在同一进程内也不共享这两份状态。它通过稳定 Port 和 Generated Contract 被 Runtime、Server、Client 使用，不把 Voxel 领域实现泄漏到 C# ECS。

总架构基线见 [`docs/architecture/LumioGameEngine_Architecture_v0.3.md`](docs/architecture/LumioGameEngine_Architecture_v0.3.md)。

体素引擎及其 Chunk、Streaming、Revision、Mesh 和体素查询实现统一使用 Rust；C# 侧只能通过生成的 Voxel Contract/`IVoxelWorldPort` 访问。

## 拥有的状态与生命周期

- VoxelWorld、Chunk、坐标、Block 数据布局、存储和加载状态。
- Chunk Revision、Mutation/Transaction、变更区域、Snapshot/Diff 和 Streaming 状态。
- Mesh、Voxel Collision Source、Voxel Spatial Source 的构建缓存。
- `VoxelWorldHandle`、`VoxelChunkHandle`、`ChunkId` 及其 Generation 生命周期。

Server、Client 和 Local 的世界实例由各自 Host 创建、Tick、Snapshot、迁移和销毁；Client 实例永远不是 Server 权威数据的第二真相来源。

## 职责

- 实现 VoxelWorld/Chunk 的创建、读取、批量修改、事务提交、Revision 和回放。
- 实现 Chunk Load/Unload、Streaming、Snapshot、Diff、序列化、压缩和恢复。
- 生成 Mesh 数据、碰撞源和空间源，并用 `LumioNativeCore` 的通用 Kernel 做批量优化。
- 提供 Voxel Query、Mutation Batch、Result Batch、Revision 和错误契约。
- 提供服务器 Rust crate、统一 Native 产物所需的客户端平台库和 Headless 测试适配。
- 为跨 World 操作提供 Tick 内 Prepare/Commit 所需的变更摘要和可重试结果。

AOI/Streaming/Collision 的体素感知策略可以在本仓库或 Runtime Coordinator 中组合，但通用 Kernel 仍归 `LumioNativeCore`。

## 明确不负责什么

- 不实现 Ability、Effect、Attribute、Tag、背包、权限、任务、战斗或其他 Gameplay 判断。
- 不创建或直接修改 C# ECS Entity/Component；只返回版本化 Voxel 结果。
- 不在 C# 保存完整 Chunk/Block 权威副本。
- 不承担 Connection、Session、RPC 路由、端口监听、CoreCLR 或 Server 进程生命周期。
- 不依赖 `LumioGameRuntime`、`LumioServer`、`LumioClient` 或 `LumioGame` 源码。

## 对外产物与契约

- Rust crate 与服务器链接库。
- `LumioVoxel` C ABI/托管绑定契约：Handle、ChunkId、Query、Mutation、Revision、Snapshot/Diff、Result Batch。
- Chunk 序列化格式、压缩字典、错误码、能力表和迁移版本。
- Mesh/Collision/Spatial Source 数据契约及 Headless fixture。

所有结构都带版本和长度；破坏性变化提升契约主版本，并由 `LumioCoreEngine` 统一打包。

## Source / Compile-Time Dependencies

- `LumioNativeCore`：通用 Handle、空间、碰撞、压缩和 Typed Job Kernel。
- Rust toolchain、平台 SDK 和经审核的 Rust crates。
- 不得依赖任何上层 Runtime、Server、Client 或 Game 源码。

## Generated Contract Dependencies

生成 Voxel ABI Header、C# P/Invoke/源生成绑定、序列化器、Revision Schema 和 Migration Metadata。`LumioGameRuntime` 仅引用 `IVoxelWorldPort` 与这些生成契约；禁止引用本仓库内部模块。

## Runtime Loading Relationships

```text
LumioCoreEngine platform package
  -> LumioServer / LumioClient native loader
  -> VoxelWorld instance (authority or replica)
LumioGameRuntime
  -> IVoxelWorldPort + generated Voxel contract
```

Server 与 Client 分别创建 VoxelWorld 和 VoxelReplicaWorld。LocalEmbedded 通过 InMemoryTransport 传输 Voxel Snapshot/Mutation 结果，仍保持两个实例。

## Release Composition Relationships

本仓库发布 Voxel 领域版本；`LumioCoreEngine` 将其与 NativeCore 组合为一个平台包。`LumioGame` 只锁定该包的版本/Hash。Server/Client 的 Game Release 必须声明兼容的 Voxel ABI 和 Chunk Migration 版本。

## Room Modes / Host Profiles

提供相同的 Voxel API 给：

- `PublicDedicatedServer`、`PlayerHostedDedicatedServer`、`LocalhostDedicatedServer`：Server 侧权威世界 + Client 侧副本。
- `LocalEmbedded`：同进程 Server/Client 双角色与两个世界实例。
- `PureHeadless`、`NativeHeadless`、`LocalSplitProcess`、`RemoteDS`、`MobileLocal`：由 Host 选择实例角色和传输方式。

## Headless Test Surface

- Chunk 读写、边界、坐标、Revision、Mutation Transaction 和冲突测试。
- Streaming/Load/Unload、Snapshot/Diff、序列化/压缩和崩溃恢复测试。
- Voxel-aware AOI/Collision/Spatial 查询 Benchmark，覆盖约 100 名真实玩家规模的世界数据基线。
- Server Authority、Client Replica、Local 双实例和 Replay 重放一致性测试。

## Version / Manifest

Manifest 必须包含 Voxel API/ABI、Chunk Schema、压缩字典、Migration 版本、平台产物 Hash 和依赖的 NativeCore 版本。启动与迁移前校验 World Schema；不兼容时由 Server 升级编排拒绝启动。

## 开发规范

- 权威修改只能在 VoxelWorld 所属 Role 执行；Client 预测必须可被 Revision/Correction 覆盖。
- 跨 World 读写通过 Runtime Coordinator 的 Prepare/Commit Port；禁止持有对方 World 的指针或 Storage 引用。
- Chunk/Mutation API 先定义容量、顺序、失败语义和回放格式，再实现优化。
- Voxel-aware 优化记录体数、Chunk 密度、AOI 半径、加载队列和内存指标；不得把玩法判断下沉。
- 每个破坏性 Chunk/Revision 变化都必须提供 Migration、旧版本 fixture 和失败回滚测试。

## 当前阶段任务

- 冻结 VoxelWorld/Chunk/Revision/Mutation 的 v0.3 生成契约。
- 建立 Server 权威、Client Replica、Local 双实例和 Native Headless 最小测试。
- 完成 Chunk Streaming、Snapshot/Diff、Voxel-aware AOI 的可重复 Benchmark。
