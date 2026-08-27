# 0002 · Mutation 以不可失败的 CommitBatch 同时发布数据与 Revision

- 日期:2026-08-27
- 状态:生效

## 背景

`chunk`、`mutation`、`revision` 曾各自叙述「应用 Block / 递增 Revision」，没有唯一提交点。可见写入与公开 Revision 若可撕裂，Query、Snapshot、幂等重放和 `RevisionConflict` 全部失效。`chunk` 与 `revision` 若互相调用，还会在落地 crate 时形成环。

## 决策

- `chunk` 与 `revision` 是同层 sibling，禁止服务互调。
- `mutation` 是唯一写入协调者。逻辑提交顺序固定为：

```text
prepare_revision_delta
-> stage WriteSet
-> validate all invariants
-> infallible publish {
     publish all Chunk pages
     publish Dirty / change summary
     publish ChunkRevisionSet
     publish WorldRevision
     record participant receipt
   }
```

- 第一个可见写入之后不得再执行可能失败的校验。多 Chunk WriteSet 是一个提交单元。
- `chunk` 不调用 `revision.advance()`；`revision` 不回调 `chunk`。两者只接受 `mutation` 持有的受控 publish 能力。
- 若 publish 内部不变量失败，整个 World 进入 `Faulted`，不得返回普通可重试错误。
- Host 对已覆盖该 Chunk 的 Snapshot/WAL 发出耐久回执后，由 `world` 在 Barrier 转交 `chunk.clear_dirty`；`streaming` 与 `snapshot` 都不直接改 Dirty。未获回执的 Dirty Chunk 不得进入 `Unloaded`。

## 后果

单域 Foundation 必须按此协议实现 Mutation。公共错误码、participant receipt 的跨仓字段仍回架构源。receipt 位于 infallible publish 之内，与世界状态同批原子耐久，遵循架构源 `docs/adr/ADR-025-voxel-participant-receipt-durability.md`（`CoDurableWithWorldState`）。物理页 COW 可委托 `chunk` backend，Pin 元数据仍归 `revision`。
