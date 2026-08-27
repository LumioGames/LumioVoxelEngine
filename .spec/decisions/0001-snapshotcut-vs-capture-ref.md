# 0001 · Runtime 拥有 SnapshotCut，Voxel 只拥有 VoxelCaptureRef

- 日期:2026-08-27
- 状态:生效

## 背景

架构源把 `SnapshotCut` 定义为 Tick Barrier 上固定的跨 World 一致读取切面，由 `LumioGameRuntime` Coordinator 拥有；VoxelEngine 拥有的是 Voxel Snapshot/Diff payload。本仓根 README 与模块所有权表曾把 `SnapshotCut` 写成 Voxel 状态，snapshot 模块还声明拥有「活跃 SnapshotCut」，会让实现者再造一份 Session 切面。

## 决策

- 跨域 `SnapshotCut` 与对应 `SessionRevisionVector` 的唯一所有者是 Runtime Coordinator。
- VoxelEngine 只接收不可变 Cut 描述，不得创建、推进或改写公共 Cut。
- `snapshot` 拥有一次捕获的 `VoxelCaptureRef`、编码任务和 Canonical payload；`revision` 拥有 Pin/COW 记录；`world` 只路由 `capture(cut)` 请求，不缓存 Cut。
- 仓内文档与候选接口使用 `VoxelCaptureRef` / `SnapshotCaptureTask`，不再把公共 `SnapshotCut` 列为 Voxel 状态。

## 后果

Port 的 Snapshot 入口以 Cut 为输入。本仓不修改架构源 Cut 语义；Voxel payload Schema 仍由架构源另行冻结。
