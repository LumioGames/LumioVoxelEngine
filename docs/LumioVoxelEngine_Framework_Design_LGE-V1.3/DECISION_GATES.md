# Decision Gates

Baseline: `LGE-V1.3-2026-08-27`. Status of all entries is fixed as `unapproved` for this design package.

| Decision | Recovered source statement | Status | Implementation rule |
|---|---|---|---|
| `VOX-D-001` | 把未批准的 VOX-D-001–008 写成已冻结数字。  _(source: `brief`)_ | `unapproved` | Expose configuration/measurement seam only; do not select a default or algorithm. |
| `VOX-D-002` | VOX-D-002 Block 存储与压缩后端  _(source: `brief`)_ | `unapproved` | Expose configuration/measurement seam only; do not select a default or algorithm. |
| `VOX-D-003` | ADR 0005：任何离开 Barrier 的异步任务必须带完整 Origin Token（worldContext, requestId, inputWorldRevision, inputChunkRevisionSet, applyPhase）。队列按矩阵声明容量/满载/可靠性；数值本身仍属 VOX-D-003/006，不得假装已冻结。  _(source: `brief`)_ | `unapproved` | Expose configuration/measurement seam only; do not select a default or algorithm. |
| `VOX-D-004` | VOX-D-004 Reservation 租约与 receipt 表容量  _(source: `brief`)_ | `unapproved` | Expose configuration/measurement seam only; do not select a default or algorithm. |
| `VOX-D-005` | VOX-D-005 Pin/COW 与子 chunk Diff 粒度  _(source: `brief`)_ | `unapproved` | Expose configuration/measurement seam only; do not select a default or algorithm. |
| `VOX-D-006` | VOX-D-006 Streaming 优先级/并发/背压阈值  _(source: `brief`)_ | `unapproved` | Expose configuration/measurement seam only; do not select a default or algorithm. |
| `VOX-D-007` | VOX-D-007 Spatial/Collision Kernel 适配与缓存键  _(source: `brief`)_ | `unapproved` | Expose configuration/measurement seam only; do not select a default or algorithm. |
| `VOX-D-008` | VOX-D-008 Migration 节点粒度  _(source: `brief`)_ | `unapproved` | Expose configuration/measurement seam only; do not select a default or algorithm. |

## Gate protocol

1. The architecture owner approves the decision in its authoritative source.
2. Generated/config contract changes, if any, are produced by the architecture repository—not handwritten here.
3. The affected task records the approved source revision and adds benchmark/fixture evidence.
4. Review verifies no unrelated module boundary or public schema changed.
5. Until all four steps complete, blocked behavior cannot be presented as production-ready.
