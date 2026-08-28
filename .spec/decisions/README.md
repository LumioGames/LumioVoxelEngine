# Decisions(决策记录 · ADR)

用 ADR(Architecture Decision Record)记录决策:为什么这样调度、为什么定这种结构、为什么划这条边界。**本目录是全仓决策记录的唯一落点**——功能内决策与框架级决策都记这里,feature 文档只描述设计现状,不留决策记录。

> 跨仓公共语义的决策只在 `LumioGameEngineArchitecture` 维护；本目录仅记录 VoxelEngine 内部实现决策，并从 `0001` 开始编号。

## 怎么写一条 ADR

- 一个决策 = 一个文件 `NNNN-<slug>.md`,编号从 `0001` 递增;写完在下方索引加一行。
- **一旦记录不改写**:被推翻就新增一条,把旧的状态标成「被 NNNN 取代」,历史留痕。
- 无 frontmatter。格式照抄:

      # NNNN · <一句话决策>

      - 日期:YYYY-MM-DD
      - 状态:生效 | 被 NNNN 取代

      ## 背景
      面对什么问题。

      ## 决策
      定了什么。

      ## 后果
      接受了什么代价。

## 索引

| 编号 | 决策 | 状态 |
|------|------|------|
| [0001](0001-snapshotcut-vs-capture-ref.md) | Runtime 拥有 SnapshotCut，Voxel 只拥有 VoxelCaptureRef | 生效 |
| [0002](0002-barrier-commit-batch.md) | Mutation 以不可失败的 CommitBatch 同时发布数据与 Revision | 生效 |
| [0003](0003-dependency-graphs-and-layering.md) | 用三张图描述依赖，逻辑模块不必等于 crate | 生效 |
| [0004](0004-snapshot-short-barrier-vs-quiesce.md) | 运行中 Snapshot 只在短 Barrier 固定 Cut，Quiesce 才停写 | 生效 |
| [0005](0005-origin-token-and-queue-matrix.md) | 异步任务携带完整 Origin Token，队列按矩阵声明 | 生效 |
| [0006](0006-crate-map.md) | 按分层合并 crate，不按逻辑模块开仓 | 生效 |
| [0007](0007-v1.4-implementation-baseline.md) | 采用 LGE-V1.4 作为实现基线 | 生效 |
| [0008](0008-interned-contract-tables-as-static.md) | 三张 interned 契约表以 `static` 而非 `const` 再导出 | 生效 |
| [0009](0009-consume-adr-040-041-artifacts.md) | 随镜像同步消费上游已发布的 ADR-040 / ADR-041 产物 | 生效 |
