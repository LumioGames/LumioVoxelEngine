# 0004 · 运行中 Snapshot 只在短 Barrier 固定 Cut，Quiesce 才停写

- 日期:2026-08-27
- 状态:生效

## 背景

`world` 曾把「关闭新写入 → 编码 → Host 持久化 → 恢复运行」写成唯一快照流；`snapshot` 声明不暂停 Tick；`revision` 声明编码期间继续读写。三者直接矛盾。架构源的维护暂停（关 Ingress、固定 Cut、再停 Tick）与运行中异步 Snapshot 也不是同一条路径。

## 决策

运行中 Snapshot：

```text
短 Barrier：
  Runtime 固定 SnapshotCut
  Voxel 校验 Cut
  revision 建立 Pin/COW
  取得不可变 VoxelCaptureRef
  恢复权威写入

后台：
  snapshot 从 CaptureRef 编码
  Verify
  交 Host 持久化
  Release Pin/COW
```

- Pin 失效或预算耗尽不得输出 Ready。
- 编码失败保留旧 Active，不把 World 标为 Faulted，除非内部视图污染。
- Quiesce / 维护快照：先关闭 Ingress 与新写入、排空或中止 Reservation，再走同一套 Cut → CaptureRef；此时停写是生命周期动作，不是编码期间的停 Tick。

Restore materialize：Host 提供不可变字节 → `snapshot.decode` → `world` 的 restore 入口 → `chunk` 物化页 + `revision` 恢复 Stamp。不走 `streaming` 的网络/Storage Load 路径。

## 后果

Cut 停顿只覆盖固定 Cut 与建立 Pin。Host fsync/激活不在 Voxel Barrier 内。Pin/COW 上限与 Diff 粒度仍由 VOX-D-005 与架构源 Snapshot payload 决定。
