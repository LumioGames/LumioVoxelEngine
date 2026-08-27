# 0005 · 异步任务携带完整 Origin Token，队列按矩阵声明

- 日期:2026-08-27
- 状态:生效

## 背景

各模块分别写「Generation 校验」「安全点发布」「迟到丢弃」，但 Token 字段和发布 Phase 不统一。审查 VXL-008 指出：World Context、RequestId 与输入 Revision 缺一，迟到 Completion 就能写入新实例或错误 Tick。公共 Token 形状不进跨仓 Schema；本仓必须先冻结内部身份与队列纪律，Foundation 才能开异步路径。

## 决策

每个离开所属 Barrier 的异步任务必须携带不可缩减的 Origin Token：

| 字段 | 来源 | 用途 |
| --- | --- | --- |
| `worldContext` | `voxelContext`（`contextId` + `generation`） | 拒销毁后/复用后的实例 |
| `requestId` | 提交方颁发、任务生命周期内唯一 | 去重、取消、幂等 Completion |
| `inputWorldRevision` | 提交时的可读 WorldRevision | 拒基于已回收视图的结果 |
| `inputChunkRevisionSet` | 任务实际覆盖的 Chunk 子集；无覆盖则空 | 拒 Chunk 被替换后的旧页 |
| `applyPhase` | 架构源 Tick 十三相中的 Runtime Phase 名（提交时绑定） | Completion 只能在该 Phase 进入所属 Barrier；本仓 Chunk/Streaming 状态名不得写入此字段 |

取消、超时、World 销毁之后，任何 Completion 不得入队、不得发布。Token 任一字段不匹配即丢弃并计数，不得降级为「尽量应用」。

队列矩阵（容量数值属 VOX-D-003/006，本决策只冻结构）：

| 队列 | 所有者 | 生产者 | 消费者 | 顺序 | 满载 | 可靠性 | Token | 发布 Phase |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `mutation.request` | mutation | Port / Runtime | Voxel Barrier | FIFO | 拒绝，Host 停接入 | 不可丢 | 提交即绑定 | `VoxelCommit` |
| `mutation.result` | mutation | Barrier | Port | FIFO | 拒绝 | 不可丢 | 同请求 | 调用方读取，不重入 Barrier |
| `query.request` | query | Port | Query Worker | FIFO | `QueueFull` 或按策略取消 | 可拒不可静默丢 | 是 | 只读 Phase（Role 声明） |
| `query.result` | query | Query Worker | Barrier → Port | FIFO | `QueueFull`，停止该请求 | 可截断，须带原因 | 是 | 只读 Phase |
| `streaming.load` | streaming | world / query 缺口 | IO Worker | 优先级 + 截止时间 | 低优先级拒绝/取消 | 可拒 | 是 | 所属 Role 的声明 Phase；Barrier 内才把 Chunk 推到本仓状态 `PublishReady` |
| `streaming.unload` | streaming | world / 预算 | IO Worker | 优先级 | 拒绝未 Ack 的 Dirty | Dirty 不可丢 | 是 | 所属 Role 的声明 Phase；Barrier 内才发布本仓状态 `Unloaded` |
| `streaming.completion` | streaming | IO Worker | Barrier | FIFO | 拒绝并回压 IO | 不可静默丢 | 是 | Barrier |
| `chunk.page-compress` | chunk | mutation / streaming | 压缩 Worker | FIFO | `BudgetExceeded` | 可拒 | 是 | 不改权威状态 |
| `snapshot.encode` | snapshot | world（已持 CaptureRef） | Encode Worker | FIFO | `QueueFull`/`BudgetExceeded` | 不可静默丢已确认 Cut | 是（Cut 投影 + requestId） | 声明 Phase 交 Host |
| `snapshot.restore` | snapshot | Host 字节 | Barrier | FIFO | 拒绝 | 不可丢 | 是 | Barrier 物化 |
| `spatial.request` / `spatial.result` | spatial | Port | Native Job → Barrier | FIFO | `QueueFull`/截断 | 可截断 | 是 | Role 声明 Phase |
| `mesh.build` / `collision.build` | mesh-collision | Port | Native Job → Barrier | 优先级 | 取消低优先级 | 可取消 | 是 | Role 声明 Phase |

`PublishReady` / `Unloaded` 是本仓 Streaming/Chunk 状态机的值，不是架构源 Phase 枚举，不得写入 Origin Token 的 `applyPhase`。

诊断/Metrics 管道可以按策略丢；权威 Mutation、TxnJournal 片段和已确认 Snapshot 不得静默丢失。同步 P0 Profile 可以关闭异步 Query/Streaming/投影队列，但一旦开启就必须遵守本矩阵。

## 后果

Foundation 的 Reference Port 与 Native Port 共用同一 Token 与满载语义。具体容量、优先级权重和背压阈值仍由 VOX-D-003/006 在 Bench 后写入配置快照，不进公共 Schema。
