---
name: repository-architecture
description: 仓库边界与架构契约——VoxelWorld 所有权、跨仓依赖和 Architecture Gate;改 Section、Revision 或公共契约前查
metadata:
  type: doc
  status: 已交付
---

# 仓库边界与架构契约

## 规范来源与优先级

- Agent 的开发流程、测试政策和交付规则以 `.spec/` 为权威。
- 模块边界以根 [`README.md`](../../../README.md) 为本仓入口；共享架构以 `LumioGameEngineArchitecture` 的 `LGE-V1.4-2026-08-27` 为唯一来源，本仓 [`架构镜像`](../../../docs/architecture/LumioGameEngine_Architecture_v1.4.md) 只读。
- 实现蓝图见 [`lve-v1.4-implementation-blueprint.md`](../../../docs/plans/lve-v1.4-implementation-blueprint.md)；crate 地图见 [0006](../../decisions/0006-crate-map.md)，实现基线见 [0007](../../decisions/0007-v1.4-implementation-baseline.md)。
- 冲突时不得在本仓自行改写公共语义；先在架构源完成 ADR、Schema、Fixture 和新 Baseline，再同步镜像与本规范。

## 所有权边界

- 本仓拥有 VoxelWorld、Section、Block、坐标、Revision、Query、Mutation、Voxel Snapshot/Diff payload、Streaming 与 Voxel 空间数据源。
- **命名以 `lumio.voxel-world.v1` 为准**:16×16×16 的数据单元叫 **Section**,Chunk 是 16 个 Section 摞成的列容器、不携带数据。本页与根 `README.md` 里其余仍写 `Chunk` 的段落指的是那个数据单元,是待改的历史措辞;分层与键的现状见 [`features/voxel-section-chunk.md`](../features/voxel-section-chunk.md) 与 [0013](../../decisions/0013-voxel-world-contract-and-section-rename.md)。
- Host 只创建/销毁实例；Runtime 拥有跨域 `SnapshotCut`，只能经版本化 `IVoxelWorldPort` 发起查询、Prepare、Commit、Capture 和取消，不拥有 Voxel 状态机。
- Voxel 只接收不可变 Cut 并持有 `VoxelCaptureRef`；Chunk 数据与 Revision 必须经 `mutation` 的 CommitBatch 原子发布；源码只依赖 `LumioNativeCore`，不编译依赖 `LumioCoreEngine`。
- 本仓不实现 Gameplay、Ability、权限、经济、ECS Entity/Component、Session、网络、Release Pool 或进程治理。
- Server 权威世界、Client Replica 世界与 LocalEmbedded 双实例不得共享对象引用、Chunk Buffer、锁、指针或 Revision 写入。

## Architecture Gate

- Chunk/World 格式、Revision/Snapshot Schema、ID 与错误语义先在架构源更新 Schema、Migration、正向/失败 Fixture 和 Baseline，再生成本仓只读产物。
- 所有 Query 返回读取 Revision；Mutation 携带 Expected Revision，冲突返回稳定 `RevisionConflict`，缺 Chunk 不得伪装成空世界。
- 异步结果必须有界、可取消并携带 Revision；权威修改只能在所属 Role 的 Simulation Barrier 执行。
- CrossWorld Prepare 只验证并预留，Commit 按 `TxnId` 幂等应用；Native 锁内不得回调 C# 或 Hot Gameplay。
- 破坏性 Chunk/Revision 变化必须有旧版本 Fixture、Migration 和失败恢复；Voxel 优化记录密度、AOI、Streaming、CPU、内存和结果版本，不下沉 Gameplay 判断。
