# LumioVoxelEngine

> LumioGameEngine v0.2 架构中的独立 Rust 体素引擎与体素世界权威域。

## 定位

`LumioVoxelEngine` 是整个项目的体素核心。服务器和客户端使用同一套 Rust 实现，但采用不同链接与宿主方式。完整体素世界只由该引擎拥有，不在 C# ECS 中复制另一份权威数据。

## 职责

- Voxel World、Chunk 生命周期、坐标、数据布局与存储。
- 体素读取、批量修改、事务、Revision 和变更区域。
- Chunk Load/Unload、Streaming、Snapshot、Diff、序列化和压缩接入。
- Mesh 数据生成、Voxel Collision Source 和 Voxel Spatial Source。
- `VoxelWorldHandle`、`VoxelChunkHandle`、`ChunkId`、Mutation Batch 与 Result Batch。
- 发布服务器可直接链接的 Rust Crate，以及客户端 DLL、SO、dylib、静态库或 WASM 产物。

## 依赖关系

### 上游依赖

- [`LumioNativeCore`](https://github.com/LumioGames/LumioNativeCore)：通用 Handle、空间索引、碰撞、导航、压缩和 Typed Job。

### 下游使用者

- [`LumioServer`](https://github.com/LumioGames/LumioServer)：直接链接并维护服务器权威 Voxel World。
- [`LumioClient`](https://github.com/LumioGames/LumioClient)：加载平台原生库，维护客户端 Chunk 与预测视图。
- [`LumioGameRuntime`](https://github.com/LumioGames/LumioGameRuntime)：通过稳定抽象和版本化 Batch 契约访问体素能力。
- [`LumioGame`](https://github.com/LumioGames/LumioGame)：组合锁定版本的引擎产物，不直接拥有底层体素实现。

```text
LumioNativeCore
└─> LumioVoxelEngine
    ├─> LumioServer
    ├─> LumioClient
    └─> LumioGameRuntime adapter
```

## 契约所有权

本仓库是 Voxel/Chunk Handle、Voxel Query、Mutation Batch、Revision、事务结果和底层错误码的唯一事实源。

## 禁止事项

- 禁止实现背包、权限、任务、战斗、建造规则或其他 Gameplay 判断。
- 禁止创建或直接修改 C# ECS Component。
- 禁止在 C# 中复制完整 Chunk/Block 权威数据作为第二真相来源。
- 禁止承担 Connection、Session、RPC 路由和服务器进程生命周期。
- 禁止保存 C# Delegate、托管对象引用、热更方法地址或跨边界裸指针。
- 禁止依赖 `LumioServer`、`LumioClient`、`LumioGameRuntime` 或 `LumioGame`。

## 当前状态

`v0.1.0` 仅冻结仓库职责与依赖边界；尚未发布代码或软件包。

