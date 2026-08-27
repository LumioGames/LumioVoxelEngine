# LumioVoxelEngine 模块化架构审查报告

## 审查范围与证据边界

本报告按照附件定义的待审提交、架构基线、审查维度、严重级别和 A—I 场景执行。历史 `v0.3` 文件只作为兼容入口，规范基线以 `LumioGameEngine_Architecture_v1.0.md` 为准。

行号均按待审 commit 的 Raw 文件、从 1 开始计算。待审提交确为 `f415ae20c287107d4244e882a07061e6abb5a2e0`，包含 12 个文档文件变更、1,394 行新增，没有 Cargo/Rust 实现；仓库文档本身也明确说明当前阶段没有 Cargo 工程、源码、Schema 或测试实现。

本环境不能访问用户机器上的 `/Users/cui/LumioGames/...` 工作树，因此没有执行：

- `git status --short --branch`
- `spec-lint`
- Node 测试
- `.baseline.sha256` 校验
- 架构源 `lumio_contract.py validate`

以下结论基于 GitHub 上精确 commit 的不可变文件内容和架构源 commit `0fe0173`。因此不对本地未提交变更、Lint 结果、Hash 一致性、Rust 编译、运行时正确性或性能作任何通过声明。

---

# Findings

## [VXL-001] [P0] `SnapshotCut` 权威所有者在 Runtime 与 Voxel 模块之间发生冲突

**文件与行号：**

- `README.md:15-22`
- `modules/README.md:126-139`
- `modules/snapshot/README.md:24-29`
- 架构源 `LumioGameEngine_Architecture_v1.0.md:95-114, 216-231`
- 架构源 `ADR-003-cross-world-txn.md:1-14`

**违反的架构原则或来源：**

单一权威状态所有者；`SnapshotCut` 是跨 GameWorld/VoxelWorld 的 Session 一致切面，由 Runtime Coordinator 固定，而不是某个领域参与者自行拥有。

**证据：**

根 README 把 `Snapshot Cut` 列为 VoxelEngine 拥有状态；模块所有权表进一步把 `SnapshotCut` 指定给 `snapshot`，而 snapshot README 又声明拥有“活跃 SnapshotCut”。但架构源把 `Coordinator + SnapshotCut` 放在 `ServerSimulationSession` 中，ADR-003 的协调所有者也是 `LumioGameRuntime`。与此同时，revision README 又说自己不决定何时生成 SnapshotCut。当前至少存在三种不同所有权表述。

**影响：**

可能出现 Runtime 固定一个 `SessionRevisionVector`，Voxel snapshot 模块又创建或更新另一个 Cut 的实现。最终会产生：

- Game Snapshot 与 Voxel Snapshot 不在同一 Tick/Revision；
- 恢复时无法证明跨域一致性；
- Snapshot、Replication 和 CrossWorldTxn 使用不同 Revision 切面；
- LocalEmbedded 双实例可能错误复用 Cut 状态。

这是权威所有权冲突，属于 P0。

**建议：**

统一成以下模型：

- Runtime Coordinator：唯一拥有跨域 `SnapshotCut` 和对应 `SessionRevisionVector`。
- Voxel `snapshot`：只接收不可变 Cut 描述，拥有一次 `VoxelSnapshotCaptureTask`、编码 Buffer 和 Voxel payload。
- `revision`：拥有 Pin/COW 记录、租约和旧 Revision 视图。
- `world`：只路由请求和管理生命周期，不复制 Cut 状态。

仓内不再把 `SnapshotCut` 列为 Voxel 状态；可使用 `VoxelCaptureRef`、`SnapshotTask` 等内部名称，但不能重新定义公共 Cut。

**是否需要修改架构源：**

否。现有架构源已经足以确定所有权。Voxel payload Schema 仍需另行在架构源补充。

**是否阻塞 Foundation：**

是。

---

## [VXL-002] [P0] `TxnId` 幂等结果的耐久性、保留期和崩溃恢复协议尚未成立

**文件与行号：**

- `modules/mutation/README.md:24-34, 40-62, 69-80, 87-95`
- `modules/README.md:204-214`
- 架构源 `LumioGameEngine_Architecture_v1.0.md:232-263`
- 架构源 `ADR-003-cross-world-txn.md:9-27`

**违反的架构原则或来源：**

CrossWorldTxn 必须在结果丢失和参与者间崩溃后保持幂等；Duplicate `TxnId` 必须返回原结果，不能重复应用；`Indeterminate` 必须通过 Journal 和状态查询解决，不能猜测。

**证据：**

mutation 仅声明一个有界的 `TxnId -> CommitResult` 缓存或“日志引用”，但没有定义：

- 重启后该结果从哪里恢复；
- 缓存何时允许删除；
- 删除是否必须等待协调 Journal/Checkpoint 确认；
- Voxel 已提交、但 Runtime 尚未写 participant marker 时如何识别重复 Apply；
- `status(txnId)` 在缓存淘汰、Snapshot 恢复和日志截断后的返回语义。

`VOX-D-004` 也明确把 Reservation 租约和幂等记录保留列为待决问题。mutation 的参与者状态机还直接包含 `CommitIntent`，容易把 Runtime 的全局协调状态误实现为 Voxel 本地状态。架构源则要求 durable `TxnJournal`、每个参与者结果标记、状态查询和仅重放缺失的幂等步骤。

典型危险窗口是：

```text
CommitIntent 已持久化
-> Voxel 数据已经 Apply
-> 进程在 Voxel participant marker 持久化前崩溃
-> 恢复逻辑认为 Voxel 步骤缺失并再次调用 Commit
```

当前文档不足以证明第二次调用不会重复修改 Block 或重复递增 Revision。

**影响：**

可能造成：

- Voxel 修改重复应用；
- Revision 重复递增；
- Lost Result 后返回不同结果；
- Journal 认为未提交，但 World 已发生写入；
- CrossWorldTxn 无法安全恢复。

**建议：**

在架构源明确选择并冻结一种恢复模型：

1. Voxel participant receipt 与 Voxel 状态一起耐久化并可按 `SessionId + TxnId` 查询；或
2. 通过协调 Snapshot、Command Log 和确定性重放，保证重建出的 participant 状态能够辨认已应用事务。

无论选择哪一种，都必须定义：

- participant 状态与全局 CrossWorldTxn 状态的映射；
- 原始 `CommitResult` 的恢复方式；
- retention/pruning horizon；
- Runtime 在截断 TxnJournal 或推进 Checkpoint 前向参与者发出的安全回收确认；
- `Unknown`、`Prepared`、`Applied`、`Aborted`、结果已回收等状态的稳定语义；
- 每个 Journal 崩溃边界的 Failure Fixture。

Voxel participant 不应拥有全局 `CommitIntent` 状态，只应消费 Coordinator 已持久化 Intent 的证明并报告自己的 participant receipt。

**是否需要修改架构源：**

是。需要补充 ADR、Schema、错误/状态定义及 Duplicate、Lost Result、Crash-between-markers Fixture。

**是否阻塞 Foundation：**

是。

---

## [VXL-003] [P0] Chunk 数据发布与 Revision 递增没有被定义成同一个不可分割提交点

**文件与行号：**

- `modules/chunk/README.md:54-65`
- `modules/mutation/README.md:8-14, 69-79`
- `modules/revision/README.md:59-74`
- 架构源 `LumioGameEngine_Architecture_v1.0.md:255-261`

**违反的架构原则或来源：**

权威状态只能在固定 Barrier 提交；Prepare 后 Apply 必须幂等，且不能再次发生业务校验失败；数据状态与公开 Revision 必须原子可见。

**证据：**

当前三个模块分别描述：

```text
chunk:
应用 Block -> 标记 Dirty -> revision 提交版本

mutation:
应用写入 -> 递增 WorldRevision/ChunkRevision

revision:
应用变化 -> 递增 ChunkRevision/WorldRevision
```

但没有定义一个唯一的原子 Commit 协议，也没有规定：

- Revision 是否在写入前预留；
- 写入过程中发生内部错误时如何回滚；
- 多 Chunk Mutation 是否全有或全无；
- `revision.advance()` 是否保证在可见写入开始后绝不失败；
- Revision 溢出、表损坏或 Context 失效若在数据写入后发现，World 应处于什么状态。

文档同时承认 Revision 表不一致可能进入 `Faulted`。按当前顺序直接实现，会允许 Block 已变化、但 Revision 仍是旧值的撕裂状态。

**影响：**

会破坏 Query 一致性、Snapshot、缓存失效、幂等重放和 RevisionConflict。该问题可能直接导致权威数据损坏。

**建议：**

定义单一的 Barrier Commit Primitive，例如逻辑上的：

```text
prepare_revision_delta
-> stage WriteSet
-> validate all invariants
-> infallible publish {
     publish all Chunk pages
     publish Dirty/change summary
     publish ChunkRevisionSet
     publish WorldRevision
   }
-> record participant receipt
```

关键约束：

- 第一个可见写入之后不得再执行可能失败的校验；
- 多 Chunk WriteSet 必须作为一个提交单元；
- `chunk` 不调用 revision service，`revision` 也不回调 chunk；
- mutation/commit coordinator 同时持有两者的受控提交能力；
- 若内部不变量在 publish 中失败，必须将整个 World 标为不可继续服务，而不是返回普通可重试错误。

**是否需要修改架构源：**

部分需要。原子性原则已有；内部 staging/publish 机制可在本仓 ADR 解决。若新增公共 Commit 状态、Receipt 或错误码，则必须回架构源。

**是否阻塞 Foundation：**

是。

---

## [VXL-004] [P1] Architecture Gate 尚未提供实现 P0 所需的 Voxel 公共契约

**文件与行号：**

- `README.md:79-81, 120-124`
- `modules/README.md:16-25, 167-180, 204-229`
- 架构源 `schemas/index.json:1-19`
- `.spec/knowledge/standards/repository-architecture.md:22-28`

**违反的架构原则或来源：**

公共 Chunk/World 格式、Revision/Snapshot、ID、错误、ABI 和跨仓字段必须先在公共架构源发布 Schema、Migration、正向/失败 Fixture，再进入实现。

**证据：**

模块文档正确地把接口标记为“候选接口”，并明确承认当前没有完整的 Voxel Chunk、Block、Query、Streaming、Spatial 专属 Schema。架构源 Schema Index 目前包含通用 Session Revision、CrossWorldTxn、SnapshotHeader、ABI、Capability 等契约，但没有 Voxel 专属契约。仓库自身定义的实现顺序也是先 Architecture Gate，再 Foundation。

这不是“接口被误写成已冻结公共契约”的问题；相反，文档在这一点上标注是正确的。问题在于：既然 Gate 尚未完成，就还不能宣称文档已经足以指导正式 Rust Foundation。

**影响：**

以下内容仍无法稳定实现或生成跨语言绑定：

- World 创建参数和 `IVoxelWorldPort`；
- Chunk/Block/坐标与页格式；
- Query 一致性、分页、缺 Chunk 状态；
- Mutation Token、participant receipt 和稳定错误；
- Voxel Snapshot/Diff payload；
- Streaming 请求、可用性和 Generation；
- Voxel Error/Capability/ID Registry。

**建议：**

先在公共架构源完成 P0 Contract Set，并生成只读产物。本仓只能消费生成结果，不应自行冻结字段、枚举、数值 ID 或 ABI 布局。

**是否需要修改架构源：**

是。

**是否阻塞 Foundation：**

是。

---

## [VXL-005] [P1] `world` 文档允许了对 `LumioCoreEngine` 的直接基础依赖

**文件与行号：**

- `modules/world/README.md:34-38`
- `README.md:74-81`
- 架构源 `LumioGameEngine_Architecture_v1.0.md:48-60`

**违反的架构原则或来源：**

源码依赖必须是：

```text
LumioVoxelEngine -> LumioNativeCore
LumioCoreEngine  -> LumioNativeCore + LumioVoxelEngine 的发布 Artifact
```

不能反向形成 `VoxelEngine -> CoreEngine -> VoxelEngine`。

**证据：**

world README 把 `LumioNativeCore` 和 `LumioCoreEngine` 同时列为“外部基础”的 ABI/Handle/Buffer 来源。根 README 和公共架构源则明确规定 VoxelEngine 只源码依赖 NativeCore，CoreEngine 消费 Voxel 发布的 Schema/Artifact 并负责最终聚合和加载。

即使作者原意只是描述运行时由 CoreEngine Loader 加载，当前章节标题和措辞仍允许实现者建立编译依赖。

**影响：**

- 形成跨仓构建循环；
- Voxel 内部代码开始依赖 Root ABI 或 Loader 类型；
- 无法独立运行 NativeHeadless/Reference 测试；
- CoreEngine 聚合层反向渗入领域模块。

**建议：**

world 的编译基础只能是：

- `LumioNativeCore` 发布 API；
- 架构源生成的 Voxel/ABI Contract；
- 仓内定义的领域 Port。

CoreEngine 只出现在“运行时加载关系”和“发布组合关系”中，不得出现在模块源码依赖中。

**是否需要修改架构源：**

否，架构源已明确。

**是否阻塞 Foundation：**

是。

---

## [VXL-006] [P1] 总依赖图混合了编译依赖、调用方向和数据消费关系，并与模块正文不一致

**文件与行号：**

- `modules/README.md:58-115`
- `modules/chunk/README.md:35-39`
- `modules/revision/README.md:35-39`
- `modules/mutation/README.md:35-39`
- `modules/streaming/README.md:35-39`
- `modules/mesh-collision/README.md:34-38`

**违反的架构原则或来源：**

依赖方向必须可用于建立无环 Rust crate DAG；“上游/下游”、控制调用和事件订阅不能被混成一张没有语义的图。

**证据：**

总图声明：

- `mutation -> chunk + revision`
- `streaming -> chunk + revision`
- `mesh-collision -> query + chunk + revision`

但模块正文又声明：

- mutation 的“下游”还包括 snapshot、streaming；
- streaming 的“下游”包括 query、snapshot；
- mesh-collision 正文依赖 streaming，并可使用 spatial；
- chunk 把 revision/query/snapshot 等称为“下游”，revision 又把 chunk/query/snapshot 等称为“下游”。

因此无法判断箭头到底表示：

1. Rust `use`/crate compile dependency；
2. 运行期方法调用；
3. 数据被谁消费；
4. 事件由谁订阅；
5. 生命周期由谁驱动。

这也解释了图中漏掉的边和正文多出的边。

**影响：**

未来很容易形成：

- `chunk <-> revision`
- `query <-> streaming`
- `mutation <-> snapshot/streaming`
- `snapshot <-> migration`
- `CoreEngine <-> VoxelEngine`

等编译或运行时循环。

**建议：**

拆成至少三张图：

1. **Compile-Time DAG**：crate/API 依赖。
2. **Runtime Control Flow**：谁发起调用，谁完成调用。
3. **Event/Data Flow**：`ChunkChanged`、`AvailabilityChanged`、Completion 等事件由谁生产和消费。

同时统一术语：

- `depends on`：只能表示编译/API 依赖；
- `called by`：控制流上游；
- `publishes/consumes`：事件和数据流；
- 禁止再用方向不明确的“上游/下游”。

**是否需要修改架构源：**

仓内模块图可在本仓解决；跨仓 compile DAG 不得改变。

**是否阻塞 Foundation：**

是。

---

## [VXL-007] [P1] Query 的多 Chunk 一致性和续传语义不足以实现

**文件与行号：**

- `modules/query/README.md:8-15, 39-65`
- `modules/revision/README.md:65-73`
- 架构源 `LumioGameEngine_Architecture_v1.0.md:216-231`

**违反的架构原则或来源：**

所有读取必须返回有明确域语义的 Revision；不能把“取得一个 Stamp”误当成已经获得稳定多 Chunk 视图。

**证据：**

Query 支持“指定 Revision 或最新可读视图”，流程是“取得 Revision Stamp，再按批读取 Chunk”，并允许 Pending、部分批次和异步完成。但没有定义：

- “最新”是请求开始时固定，还是每个 Chunk 各自读取最新；
- 多 Chunk 结果是否属于同一个 WorldRevision；
- 目标 Revision 已被回收时返回什么；
- Query 是否必须持有 Revision Pin/ReadView；
- Pending 恢复后继续使用原 Revision，还是重新观察最新 Revision；
- continuation/cursor 是否绑定原始 Revision；
- 部分结果中每个元素、每个 Chunk 和批次级 Stamp 的关系；
- Revision 在 Worker 执行期间变化时，是返回旧视图、Stale 还是整体失败。

当前“先读 Stamp、再读 Chunk”本身不能保证一致性。

**影响：**

可能产生一个批次内部混合 Revision，导致：

- Spatial/Mesh 输入不可重放；
- Expected Revision 重试基于错误观察值；
- Snapshot/Diff 和 Query 结果无法比较；
- Pending 后返回的数据覆盖较新的调用结果。

**建议：**

在架构源定义 Query consistency contract，至少覆盖：

- 请求目标是固定 Revision 还是明确的 Latest-at-Acquire；
- 多 Chunk ReadSet；
- ReadView/Pin 生命周期；
- target revision unavailable/stale；
- continuation 与原 Revision 的绑定；
- partial result 的 Revision 表达；
- cancellation、deadline 和迟到 Completion。

具体枚举名可以后定，但语义必须先冻结。

**是否需要修改架构源：**

是，需要 Query Schema、ADR 和正反 Fixture。

**是否阻塞 Foundation：**

是，因为 query 是 P0。

---

## [VXL-008] [P1] 异步 Completion 的身份令牌、精确发布阶段和队列契约不完整

**文件与行号：**

- `modules/README.md:142-166`
- `modules/world/README.md:54-59`
- `modules/query/README.md:53-58`
- `modules/streaming/README.md:60-65`
- `modules/spatial/README.md:23-32, 49-57`
- `modules/mesh-collision/README.md:23-33, 49-55`
- 架构源 `LumioGameEngine_Architecture_v1.0.md:144-165, 289-298`

**违反的架构原则或来源：**

异步任务只能在规定 Phase/Barrier 发布；World 销毁、Chunk 替换、任务取消后的 Completion 不得写入新实例；所有跨线程队列必须有版本化身份和有界策略。

**证据：**

文档普遍使用“约定的 Barrier”“只读安全点”“安全点发布”等表述，但没有把它们映射到架构源的具体 Phase。Generation 也分散在：

- World Context Generation；
- Chunk 本地 Generation；
- Streaming 当前 Generation；
- Query request；
- Mesh build；
- Spatial request。

Spatial 缓存键仅列出 `ChunkId + ChunkRevision + QueryShape + Capability`；Mesh 缓存键仅列出 `ChunkId + ChunkRevision + BuildProfile`。除非强制保证每个缓存实例只属于一个永不复用的 World，否则这些键不足以排除旧 World、旧 Chunk 或旧任务结果。队列文档多数只写“有界”，没有形成包含 producer、consumer、ordering、capacity source、full action、cancellation、visibility phase 的统一清单。架构源则要求每个阶段的可写状态和可见性在 API Contract 中声明，并规定 Handle 为 Index+Generation+Context。

**影响：**

迟到 Completion 可能：

- 写入重新创建的 World；
- 覆盖新 Chunk Generation 的缓存；
- 在错误 Tick Phase 对 Query 可见；
- 在取消或超时后重新完成请求；
- 让 Local 双实例因 ID 相同而误命中缓存。

**建议：**

所有异步任务统一携带不可缩减的 Origin Token：

```text
WorldContext/Generation
ChunkId/ChunkGeneration（适用时）
RequestId/TaskGeneration
Input WorldRevision/ChunkRevisionSet
Capability/Profile identity
```

并建立队列矩阵，逐队列明确：

- producer / consumer；
- 容量来源；
- 优先级和稳定排序；
- QueueFull 行为；
- 取消传播；
- Completion 校验顺序；
- 精确发布 Phase；
- stale result 的销毁责任。

**是否需要修改架构源：**

部分需要。公共 Token、错误和 Phase 可见性需要回架构源；仓内队列实现及默认容量可由本仓 ADR 决定。

**是否阻塞 Foundation：**

是。同步 P0 探索可以临时禁用异步能力，但不能据此宣告 Foundation 完成。

---

## [VXL-009] [P1] Snapshot 期间是否停止写入存在直接矛盾

**文件与行号：**

- `modules/world/README.md:60-64`
- `modules/snapshot/README.md:16-29, 40-57`
- `modules/revision/README.md:49-69`

**违反的架构原则或来源：**

Snapshot Cut 应在短 Barrier 内固定；后续编码应使用 Pin/COW 的稳定视图。是否停止 Tick、停止写入以及停止到何时，必须只有一个明确语义。

**证据：**

world 的数据流写成：

```text
关闭新写入
-> 固定 Cut
-> Pin/COW
-> snapshot 编码
-> Host 持久化
-> 恢复运行
```

这意味着整个编码甚至持久化期间都停止写入。snapshot README 则明确说“不暂停 Tick”；revision README 又明确说“编码期间继续读写”。同时 Pin 状态允许 `Expired | Invalidated`，但 snapshot 没有定义 Pin 在 Encoding 中失效时的完整结果、清理和重试协议。

**影响：**

不同实现者可能分别实现：

- 长时间停服式 Snapshot；
- 写入不停但没有正确 COW；
- Pin 超限后静默丢弃旧页；
- Snapshot 编码完成但 Cut 已失效。

前者会造成不可接受的 Tick 停顿，后几种会破坏 Snapshot 一致性。

**建议：**

冻结唯一流程：

```text
短 Barrier：
  Runtime 固定 SnapshotCut
  Voxel 校验 Cut
  revision 建立 Pin/COW
  取得不可变 CaptureRef
  恢复权威写入

后台：
  snapshot 从 CaptureRef 编码
  Verify
  交 Host 持久化
  Release Pin/COW
```

同时定义：

- Pin/COW 最大内存和最大持续时间；
- 预算耗尽时 Snapshot 失败还是阻止新写；
- Pin 失效后不得输出 Ready；
- Snapshot 失败与 World Faulted 的分界；
- Full Snapshot 与 Diff 的降级条件。

**是否需要修改架构源：**

是，需要 Voxel Snapshot payload/Capture ADR、Schema 和并发写 Fixture。

**是否阻塞 Foundation：**

是，因为 SnapshotCut/Revision 语义属于 Architecture Gate，即使编码模块实现优先级是 P1。

---

## [VXL-010] [P1] Dirty Chunk 可以进入 Unloaded，但没有定义任何耐久性栅栏

**文件与行号：**

- `modules/chunk/README.md:40-53`
- `modules/streaming/README.md:40-70`
- `modules/snapshot/README.md:16-21`

**违反的架构原则或来源：**

已经提交的权威修改不得因驱逐而丢失；Streaming 不拥有 WAL/Snapshot 文件耐久，因此不能自行把未耐久 Dirty Chunk 变成 Unloaded。

**证据：**

chunk 明确规定 `Dirty` 表示尚未被 Snapshot/WAL 记录。streaming 状态机却允许：

```text
Ready/Dirty -> Evicting -> NotLoaded
```

卸载流程只写了“Flush/保留脏状态”，但：

- streaming 明确不负责 WAL/Snapshot 耐久；
- snapshot 也不负责文件、fsync 或激活；
- 没有定义由谁发起 Flush；
- 没有定义 durable acknowledgement；
- 没有定义 Host 失败、磁盘满或回执丢失时 Chunk 停在哪个状态；
- “保留脏状态”在 Chunk 已成为 NotLoaded 后具体保留在哪里并不明确。

**影响：**

可能在内存回收后永久丢失已确认的体素修改，或者重新加载旧页并回退权威世界。

**建议：**

引入明确的 Eviction Durability Fence：

```text
Dirty
-> EvictionRequested
-> DurableOrExplicitlyVolatileAcknowledged
-> NoReadView/Pin/Reservation/BuildTask
-> Evicting
-> Unloaded
```

Dedicated Server 默认不得驱逐未获得恢复保障的 Dirty Chunk。Singleplayer 允许轻量耐久策略时，也必须由 Capability/Manifest 明确声明最大丢失边界，不能由 streaming 静默决定。

**是否需要修改架构源：**

部分需要。仓内状态协调可本仓解决；跨 Host 的 durability acknowledgement、Capability 和错误语义需要回架构源。

**是否阻塞 Foundation：**

有条件阻塞。若 Foundation 明确使用全驻留、禁用 Unload 的 Capability，可暂不实现；任何启用 Streaming/Unload 的路径都被阻塞。

---

## [VXL-011] [P1] Migration 模块跨越了 Host 编排与文件系统所有权，且公共 ADR 与 Schema 自身不一致

**文件与行号：**

- `modules/migration/README.md:16-29, 40-65, 72-82`
- 架构源 `ADR-013-migration-dag.md:10-19`
- 架构源 `schemas/migration-manifest.schema.json:1-33`

**违反的架构原则或来源：**

VoxelEngine 应拥有 Voxel 迁移语义和节点实现；Server/Host 拥有完整 DAG 编排、Staging 目录生命周期、进程恢复和原子激活。

**证据：**

migration README 一方面说自己不负责文件系统、Checkpoint 保留和完整 DAG 最终裁决，另一方面又声明拥有：

- Migration 节点注册表和依赖；
- Staging 目录/版本句柄；
- Checkpoint；
- “整体迁移”状态机；
- 重启后扫描 Staging/Checkpoint；
- 完整 DAG 校验与顺序合并。

这些已经超出“Voxel 节点提供者”的职责，进入 Host/Server orchestration。

公共架构源还存在一处独立缺口：ADR-013 要求 Migration Manifest 包含 input/output hash、tool version 和 idempotency；当前 Schema 只要求 nodeId、owner、dependsOn、inputSchema、outputSchema、idempotent，没有输入/输出 Hash 或工具版本字段。

**影响：**

- Voxel migration 与 Server migration orchestrator 会各自维护一套 DAG 状态；
- Staging 扫描和恢复责任重复；
- 进程崩溃后不能唯一确定谁恢复；
- Manifest 无法满足 ADR 规定的可重放和供应链证据；
- snapshot 与 migration 容易形成反向控制循环。

**建议：**

将本模块收窄为：

```text
VoxelMigrationNodeProvider
VoxelMigrationNodeExecutor
VoxelMigrationArtifactValidator
VoxelMigrationFixtureProvider
```

它只拥有节点局部输入、输出、转换器和节点级结果。以下内容归 Host/Server：

- 完整图调度；
- Staging 目录和 Checkpoint 索引；
- 重启扫描；
- 磁盘水位；
- 原子激活；
- Active 指针；
- 全图 Failure Bundle。

同时修正公共 Migration Manifest Schema，使其与 ADR-013 一致。

**是否需要修改架构源：**

是。

**是否阻塞 Foundation：**

按 Architecture Gate 的完整性要求，是；不阻塞一次性 P0 内部实验，但阻塞任何正式 migration P1 实现和基线完成声明。

---

## [VXL-012] [P2] 根 README 的持久化措辞模糊了 Host 与 snapshot 模块的边界

**文件与行号：**

- `README.md:64-69`
- `modules/README.md:107-111, 126-139`
- `modules/snapshot/README.md:16-21`

**违反的架构原则或来源：**

领域模块只产出 Canonical bytes；文件、目录、fsync、原子替换和 WAL 由 Host/Runtime 持久化编排拥有。

**证据：**

根 README 写成“Snapshot 采用临时文件、校验、fsync/原子替换”，容易被解释成 snapshot 模块负责这些操作；模块总表和 snapshot README 又明确将这些职责排除并交给 Host/Runtime。

**影响：**

实现者可能把文件系统、Checkpoint 激活和 WAL 逻辑下沉到 Voxel snapshot crate，造成 Host 与 Voxel 双重持久化实现。

**建议：**

根 README 改成系统级流程表述，并明确主语：

```text
Voxel snapshot 生成并校验 Canonical payload；
Host persistence 负责 staging、fsync、原子激活和 checkpoint retention。
```

**是否需要修改架构源：**

否。

**是否阻塞 Foundation：**

否；但应与 P0/P1 文档整改一起修正。

---

# 已正确建立的关键边界

以下设计不建议推翻：

1. **Query 应继续保持独立。** 它确实拥有自己的预算、取消、Pending、部分结果和读取 Revision 语义，不应并入 chunk 或 world。
2. **Revision 应继续保持独立。** Chunk 不自行递增公共 Revision，Revision 也明确不拥有 Block Storage，这个方向正确。
3. **Spatial 与 Mesh/Collision 应继续分开。** 前者是空间候选/遮挡投影，后者是 Renderer/Physics 可消费的几何 Source；二者均正确排除了最终 AOI、权限、Renderer、Physics 和 Gameplay 所有权。
4. **CrossWorldTxn 的固定提交顺序写对了。** 文档采用 `VoxelCommit -> EcsCommandBufferCommit`，架构源校验器源码也对该顺序及 CommitIntent、participant marker、Indeterminate 进行了语义校验。这里的问题是恢复和幂等记录，不是提交顺序。
5. **后台 Worker 不直接写权威状态的原则写得正确。** Streaming、Query、Spatial、Mesh 均要求 Worker 只生产结果并在 Barrier/安全点发布。问题是“安全点”尚未精确化。
6. **LocalEmbedded 双 World 隔离边界正确。** 文档禁止共享 Storage、Buffer、锁和 Revision，架构源也要求本地模式复用同一协议、队列和错误语义。

---

# 模块逐项评分

除“耦合风险”外，其余项目 5 分最好；“耦合风险”0 表示低风险、5 表示高风险。

| 模块 | 边界清晰度 0-5 | 内聚性 0-5 | 耦合风险 0-5 | 接口准备度 0-5 | 失败/恢复 0-5 | 测试准备度 0-5 | 结论 |
|---|---:|---:|---:|---:|---:|---:|---|
| world | 3 | 4 | 4 | 2 | 4 | 4 | 保留；先修 Cut 所有权和 CoreEngine 依赖 |
| chunk | 4 | 5 | 3 | 2 | 3 | 4 | 保留；需要原子提交和耐久驱逐协议 |
| revision | 3 | 4 | 4 | 2 | 3 | 4 | 保留独立；澄清与 chunk/Pin 的接口 |
| query | 4 | 4 | 3 | 2 | 3 | 4 | 保留独立；一致性 Schema 未完成 |
| mutation | 3 | 4 | 5 | 1 | 2 | 4 | 保留独立；当前被 P0 幂等和原子性阻塞 |
| snapshot | 3 | 4 | 4 | 2 | 3 | 4 | 保留独立；不得拥有跨域 Cut 和文件耐久 |
| streaming | 3 | 4 | 4 | 2 | 2 | 4 | 保留独立；必须增加 durability fence |
| spatial | 4 | 4 | 3 | 2 | 3 | 4 | 与 mesh 分开合理；补全异步身份和 Schema |
| migration | 2 | 3 | 5 | 1 | 3 | 4 | 收窄为 Voxel 节点提供者，不拥有全图编排 |
| mesh-collision | 4 | 4 | 3 | 2 | 4 | 4 | P2 合理；保持可丢弃 Source 边界 |

模块关键判断：

1. **world**：不需要拆分，但必须保持薄组合根；不得保存 SnapshotCut、子模块状态副本或 CoreEngine 领域对象。
2. **chunk**：当前职责内聚，不建议再拆成“block/page/compression”三个一等领域模块；这些可作为内部子模块或 Adapter。
3. **revision**：应继续独立；可把 Pin/COW 的物理页实现委托给 chunk backend，但 Pin 元数据与版本权威仍归 revision。
4. **query**：必须独立，不能合并进 chunk；进入实现前必须补公共一致性契约。
5. **mutation**：必须独立，不能并入 world；在幂等恢复和原子 publish 解决前不能进入正式实现。
6. **snapshot**：应与 migration、Host persistence 分开；只拥有 capture、codec 和临时 bytes。
7. **streaming**：应保持独立；Storage Adapter 是基础设施 Port，不应成为另一个权威领域状态模块。
8. **spatial**：与 mesh-collision 分开合理；应依赖稳定 ReadView，不应直接依赖 Chunk Storage。
9. **migration**：不应与 snapshot 合并；应收窄到 Voxel 节点语义，完整 DAG 和 Staging 激活留在 Host。
10. **mesh-collision**：P2 定位正确，不应成为 P0/P1 Port 的必需依赖。

从文档结构看，各 README 基本覆盖了规定的 15 个章节，错误分类、观测和故障测试面也明显强于普通骨架文档；问题不是“缺标题”，而是关键语义在不同章节间仍不一致。

---

# 依赖和所有权审查

## 1. 根据正文合并得到的实际依赖关系

当前文档无法导出唯一的 compile DAG。把总图和模块正文合并后，实际描述大致为：

```text
world
  -> chunk
  -> revision
  -> query
  -> mutation
  -> snapshot
  -> streaming
  -> spatial
  -> mesh-collision
  -> LumioNativeCore
  -> LumioCoreEngine       [正文额外边，违反跨仓 compile DAG]

chunk
  -> revision
  -> LumioNativeCore
  <- mutation WritePlan
  <- streaming load/unload completion

query
  -> chunk
  -> revision
  <- streaming availability event

mutation
  -> chunk
  -> revision
  -> snapshot              [正文额外边]
  -> streaming             [正文额外边]

snapshot
  -> chunk
  -> revision
  -> generated codec / NativeCore
  -> Host persistence      [外部调用边]
  -> migration artifact    [数据消费边]

streaming
  -> chunk
  -> revision              [总图声明]
  -> query                 [正文关系]
  -> snapshot              [正文关系]
  -> Storage Adapter
  -> NativeCore

spatial
  -> query
  -> chunk
  -> revision
  -> NativeCore

mesh-collision
  -> chunk
  -> revision
  -> streaming             [正文关系]
  -> spatial               [可选候选关系]
  -> NativeCore

migration
  -> snapshot
  -> chunk
  -> revision
  -> generated codec
  -> Host activation
```

总图与正文的不一致来自对“依赖”的不同解释。

## 2. 已发现的循环或潜在循环

| 潜在循环 | 形成原因 | 处理方式 |
|---|---|---|
| `chunk <-> revision` | chunk 图上依赖 revision，revision 又消费 Chunk 创建/销毁与变化摘要 | 两者作为同层 sibling；通过 `ChangeSet`/`CommitBatch` 由 mutation 协调，禁止服务互调 |
| `query <-> streaming` | Query Pending 等待 Streaming；Streaming 又把 Query 写成下游 | 用单向 `AvailabilityEvent` 或 `ChunkAvailabilityView`，Query 不直接发 Load |
| `mutation <-> streaming` | Mutation 检查可用性，正文又直接依赖 streaming | Mutation 只读 Availability Port，不控制 Load |
| `mutation <-> snapshot` | Snapshot 消费变更，Mutation 又把 snapshot 写为下游 | Mutation 发布 `ChunkChanged`；Snapshot/Diff 索引消费事件，不进行反向方法调用 |
| `snapshot <-> migration` | Migration 读取 Snapshot，Restore 又可能调用 world/migration | 仅传不可变 Artifact；完整流程由 Host 编排 |
| `VoxelEngine <-> CoreEngine` | world 把 CoreEngine 当基础；CoreEngine 又聚合 Voxel Artifact | 删除 Voxel 到 CoreEngine 的源码边 |

目前没有代码，因此不能声称已经存在编译循环；上述是当前文档允许出现、且在 crate 落地时很可能形成的循环。

## 3. 建议的稳定分层

```text
L0  generated-contracts / contract-types
    NativeCore published traits and POD types

L1  domain-types
    chunk-store
    revision-ledger
    （chunk-store 与 revision-ledger 不直接调用对方）

L2  read-view
    write-set / commit-batch
    chunk-availability-port
    storage-port

L3  query
    mutation
    snapshot
    streaming

L4  spatial
    mesh-collision

Tool Path
    voxel-migration-node-provider
    voxel-migration-node-executor

L5  world composition root
```

`world` 依赖所有已启用模块；任何 L0—L4 模块都不得反向依赖 world。

## 4. 状态所有权裁定

| 状态或资源 | 正确唯一所有者 | 当前状态 | 裁定 |
|---|---|---|---|
| VoxelWorld 实例内部生命周期 | `world`；Host 负责创建/销毁编排 | 基本清楚 | 通过 |
| Chunk/Block 数据 | `chunk` | 清楚 | 通过 |
| Chunk 权威可用性/加载状态 | `chunk`；streaming 只拥有请求与任务 | 基本清楚 | 通过 |
| WorldRevision | `revision` | mutation 也写成“递增” | 需改为 mutation 请求原子提交 |
| ChunkRevision | `revision` | 同上 | 需统一 |
| Snapshot Pin/COW 记录 | `revision` | snapshot 又声明拥有 Pin/COW handle | 需拆分“记录所有权”和“借用引用” |
| Query 请求、预算、游标 | `query` | 清楚 | 通过 |
| Mutation Reservation | `mutation` | 清楚 | 通过 |
| TxnId participant result | `mutation` participant；耐久恢复锚定 Runtime/Host Journal | 缓存和恢复边界不完整 | P0 冲突 |
| 跨域 SnapshotCut | Runtime Coordinator | root/snapshot 声称拥有 | P0 冲突 |
| Canonical bytes | snapshot 临时拥有；Host 获得后拥有耐久 Artifact | 基本清楚 | 通过 |
| Streaming 队列和任务 | `streaming` | 清楚 | 通过 |
| Spatial Cache | `spatial` | 缺完整 Context/Generation scope | 有条件通过 |
| Mesh/Collision Cache | `mesh-collision` | 缺完整 Context/Generation scope | 有条件通过 |
| Migration Staging 目录和 Active 指针 | Host/Server orchestration | migration 声明部分拥有 | P1 冲突 |
| Voxel migration 节点局部状态 | `migration` | 清楚 | 通过 |
| World Handle/Context | `world` + NativeCore Handle Registry | 基本清楚 | 通过 |
| Chunk/Request/Build Generation | 各模块拥有自身 generation；统一进入 Origin Token | 当前分散 | 需补契约 |

## 5. 组合根是否过度膨胀

`world` 当前还没有成为实际巨型模块，但文档已经给它安排了：

- Port 路由；
- Barrier；
- Capability；
- 生命周期；
- 所有子模块注册；
- Snapshot；
- Migration 配合；
- 异步取消；
- 诊断汇总；
- 最后 Revision/Snapshot 元数据。

其中 Port 路由、生命周期和组合是合理的；以下内容必须避免进入 world 内部状态：

- SnapshotCut；
- Chunk/Revision 副本；
- Query cursor；
- participant receipt；
- Streaming queue；
- Spatial/Mesh cache；
- Migration graph；
- 持久化文件状态。

建议为 world 设置明确约束：它只保存模块句柄、生命周期、Context/Generation、Capability view 和 Barrier gate，不缓存任何子模块领域状态。

## 6. 缺少的基础层，而非新的领域模块

建议补充以下物理 crate/逻辑层，但不必把十个模块扩成十四个一等领域模块：

| 建议层 | 形态 | 作用 |
|---|---|---|
| Generated Contract / Contract Types | 独立叶子 crate | 只消费架构源生成物，不成为第二契约源 |
| Voxel Port Facade | 公共 Port crate 或 world 的独立 facade | 定义 `IVoxelWorldPort` 对应 Rust 入口和版本适配 |
| ReadView / WriteSet / CommitBatch | 仓内基础接口 | 打断 chunk、revision、query、mutation 的互相引用 |
| Storage Port | 基础设施 trait/adapter | 为 streaming、snapshot decode、migration 提供存储抽象，不拥有领域状态 |
| Test Support / Reference Port | dev-only crate | ReferenceVoxelPort、fixture builder、fault decorator、differential harness |

不建议新增为独立领域模块的内容：

- **Serializer**：应是生成 Codec Adapter，不是手写领域模块。
- **Error/Capability**：基础类型来自 NativeCore/生成契约；Voxel 专属错误在架构源登记。
- **Handle/Context**：由 world 和 NativeCore Registry 协同，不应再建立第三个所有者。
- **通用 Common/Utils**：容易成为隐性循环和全局依赖容器。

当前没有 Cargo/source，物理结构调整成本很低；现在确定这些叶子层，比代码开始后拆环成本低得多。

---

# P0 / P1 / P2 优先级判断

| 判断项 | 结论 |
|---|---|
| `world/chunk/revision/query/mutation` 是否是最小 P0 闭环 | 概念上是，但必须加 Contract Types/ReadView/CommitBatch 基础层 |
| snapshot 是否应提升到 P0 | 完整编码实现可保持 P1；但 SnapshotCut、payload、Pin/COW 契约必须在 Foundation 前完成 |
| streaming 是否应提升到 P0 | 可保持 P1，前提是 P0 Profile 预加载必要 Chunk、禁用 Dirty Unload，并明确 Capability |
| spatial P1 是否合理 | 合理；PlaceVoxel P0 不应依赖它 |
| mesh-collision P2 是否合理 | 合理；不能阻塞 P0/P1 Port |
| migration P1 是否合理 | 工具实现可为 P1；公共 Manifest 和节点边界应先修 |
| PlaceVoxelAbility Vertical Slice 是否可开始 | 否；P0 幂等、原子提交、Query 契约、Local fixture、Snapshot/WAL 恢复尚未闭环 |
| 是否有 P0 模块缺实现条件 | 有：query、mutation、revision 及 world Port 均缺公共 Schema/错误和关键并发语义 |

---

# 场景推演

| 场景 | 结果 | 证据 | 暴露的问题 | 修复优先级 |
|---|---|---|---|---|
| A：创建 World | 有条件通过 | world 规定 P0 初始化、可选模块挂接、失败逆序释放、旧 Handle 失效和 Failure Bundle。 | CoreEngine 依赖错误；创建 Port/Config/Capability 尚未冻结；“可选模块延迟挂接”与 Ready 能力协商需 Schema | P1 |
| B：读取 Ready/Pending/Unavailable/Evicting Chunk | 有条件通过 | Query 明确不把缺 Chunk 当空世界，支持取消、超预算、迟到丢弃和部分结果。 | 多 Chunk 一致性、Pending 续传 Revision、Evicting ReadView 和 target revision retention 未定义 | P1 |
| C：Mutation Revision 冲突 | 有条件通过 | 文档要求冲突在可见写入前拒绝，不写入也不递增，调用方重新读取后重试。 | 稳定 Wire 错误尚未冻结；数据与 Revision 的原子发布未定义 | P0 |
| D：CrossWorldTxn PlaceVoxel | 失败 | 固定顺序和 CommitIntent 顺序正确，源校验器也验证该顺序。 | Duplicate、Lost Result、VoxelCommit 后崩溃的 participant receipt/保留期不成立；participant 状态和全局状态混淆 | P0 |
| E：Snapshot Cut 与并发写 | 失败 | snapshot/revision 允许异步编码期间继续写；world 却关闭写入直到持久化完成。 | Cut 所有权冲突、停写范围矛盾、Pin expiry 和 COW 上限不完整 | P0/P1 |
| F：Streaming 迟到 Completion | 有条件通过 | Streaming 要求 Barrier 校验 Generation 并丢弃迟到 Completion。 | Generation 只局部定义，缺 World Context、Task Generation 和 Input Revision 的完整组合令牌 | P1 |
| G：Spatial/Geometry 过期结果 | 有条件通过 | Spatial 和 Mesh 都要求 Revision/Generation 不匹配时标记 Stale 或丢弃，不拥有 AOI/Renderer/Physics。 | 缓存 scope/key 和精确 publish phase 不完整 | P1 |
| H：Migration 中途崩溃 | 有条件通过 | 不可变输入、旧 Active 保留、Checkpoint 重跑和 Failure Bundle 均有描述。 | Host 与 Voxel 对 Staging/DAG/恢复所有权重叠；Manifest 缺 ADR 要求字段 | P1 |
| I：LocalEmbedded 双实例 | 通过（文档设计） | 明确创建两份 World，禁止共享 Storage/Buffer/Revision，并要求本地复用同一协议、队列和错误语义。 | 尚无 Rust 实现、Fixture 或引用扫描证据，不能视为运行时已验证 | 后续验证门 |

---

# 契约和知识缺口

## 必须回架构源解决

1. **Voxel World/Port 契约**
   - World 创建参数；
   - Role、WorldId、Context/Generation；
   - Capability 和资源预算；
   - Handle、生命周期结果；
   - Port 方法、批量 Buffer、错误与兼容规则。

2. **Chunk/Block/Page 契约**
   - 坐标范围和负坐标；
   - ChunkId 映射；
   - Block value/type；
   - 页版本、长度、Hash、Compression；
   - 边界、损坏、旧版本 Fixture；
   - Migration 规则。

3. **Revision 契约**
   - WorldRevision 与 ChunkRevision 的编码和域；
   - `ChunkRevisionSet`；
   - Read Stamp；
   - target revision availability；
   - Snapshot Cut 到 Voxel Revision 的投影；
   - overflow 和 epoch 迁移。

4. **Query 契约**
   - 一致性模式；
   - 多 Chunk ReadSet；
   - 分页/continuation；
   - partial result；
   - Ready/NotLoaded/Pending/Unavailable；
   - timeout/cancel/stale；
   - 分配和批次限制。

5. **Mutation 参与者契约**
   - Reservation/Prepared token；
   - participant status；
   - idempotent receipt；
   - original result retention；
   - pruning/checkpoint handshake；
   - Duplicate/Lost Result/Crash fixture；
   - RevisionConflict 与 ChunkUnavailable 稳定错误。

6. **Voxel Snapshot/Diff payload**
   - 通用 `SnapshotHeader` 已存在，但只是 envelope，不等于 Voxel payload。
   - 需要 Chunk 顺序、页索引、局部 Snapshot、Diff base/target、COW/capture 语义和恢复 Fixture。

7. **Streaming 契约**
   - Load/Unload 请求；
   - Availability；
   - Generation；
   - durability acknowledgement；
   - late completion；
   - QueueFull/Storage failure；
   - Capability 禁用/降级语义。

8. **Spatial/Mesh Source 契约**
   - 仅当结果需要跨仓、跨语言或进入发布 Artifact 时进入架构源；
   - 内部缓存实现和算法无需成为公共 Schema。

9. **Migration**
   - Voxel migration node 输入/输出；
   - 修正 `migration-manifest.schema.json`，补 ADR-013 已要求的 input/output hash、tool/compiler version 等字段；
   - crash-at-node、cycle、missing dependency、old-active-retained fixtures。

10. **错误、ID、Capability 和 ABI**
    - 所有跨仓稳定错误和数值 ID；
    - Voxel Capability bits；
    - ABI 布局和 Buffer ownership；
    - 不得在模块 README 自行分配。

11. **不需要改变的公共语义**
    - `VoxelCommit -> EcsCommandBufferCommit` 顺序；
    - Prepare 无可见副作用；
    - CommitIntent 在首个参与者写入前持久化；
    - `Indeterminate` 通过状态查询解决。

这些规则已经在架构源及语义校验器中明确，不应由本仓改写。

## 可以在本仓解决

- 三张依赖图和唯一所有权表；
- 内部 crate/文件布局；
- ReadView、WriteSet、CommitBatch、Availability Port；
- Storage Adapter trait；
- world 初始化与逆序清理；
- 各队列的内部容量默认值；
- 队列 producer/consumer 和稳定排序；
- Cache scope/key；
- 同步 P0 与异步 P1 的 Capability 分层；
- ReferenceVoxelPort 和 test-support 组织；
- Property/Fuzz/Fault/Differential 测试目录；
- README 中“安全点”“按策略”“Flush/保留”等模糊措辞；
- 根 README 持久化主语；
- 模块目录与未来 crate 不必 1:1 的映射规则。

---

# 最终整改顺序

## Step 1：先解决 P0 权威所有权和 CrossWorld participant 恢复语义

**修改哪些文件：**

- 架构源 `ADR-003-cross-world-txn.md`
- 新增或扩展 Voxel participant ADR/Schema/Fixture
- 本仓 `README.md`
- `modules/README.md`
- `modules/mutation/README.md`
- `modules/snapshot/README.md`
- `modules/revision/README.md`
- `modules/chunk/README.md`

**为什么必须先做：**

SnapshotCut、幂等 receipt 和数据/Revision 原子性决定权威数据是否可恢复，不能留到编码中再决定。

**是否需要架构源变更：**

是，其中 SnapshotCut 所有权只需本仓对齐；participant receipt 和公共结果需要架构源变更。

**验收标准：**

- 每项状态只有一个所有者；
- Voxel participant 不拥有全局 CommitIntent；
- Duplicate 在重启和缓存淘汰后仍返回原结果；
- 每个 Journal 边界崩溃都有 fixture；
- Chunk 数据与 Revision 不存在可观察撕裂。

**对后续模块的影响：**

解除 mutation、revision、snapshot 和 PlaceVoxel Vertical Slice 的首要阻塞。

---

## Step 2：完成 P0 Voxel Contract Set

**修改哪些文件：**

- 架构源 `schemas/index.json`
- 新增 World/Chunk/Revision/Query/Mutation/Voxel Snapshot payload Schema
- `ids/index.json`
- Capability/Error Registry
- `fixtures/valid`
- `fixtures/invalid`
- Migration fixtures
- 生成器输入与生成物索引

**为什么必须先做：**

当前候选 API 无法稳定进入 Rust、C ABI 和 C# Binding。

**是否需要架构源变更：**

是。

**验收标准：**

- 所有 P0 public type 都来自生成物；
- 本仓 README 不复制公共字段布局；
- 正向、边界、损坏、未知字段、Duplicate、Lost Result、RevisionConflict Fixture 全部通过；
- 架构源 validator 全绿。

**对后续模块的影响：**

为 Contract/Port crate、Reference Port 和 Native ABI 提供稳定输入。

---

## Step 3：重写依赖图并确定基础 Port 层

**修改哪些文件：**

- `modules/README.md`
- 全部模块 README 的依赖章节
- 本仓内部 ADR 索引
- 未来 crate map 文档

**为什么必须先做：**

当前图不能导出无环 crate DAG。

**是否需要架构源变更：**

跨仓 DAG 不变；仓内分层不需要。

**验收标准：**

- 分别存在 Compile、Runtime Control、Event/Data 三张图；
- 每条 compile edge 都能解释；
- lower layer 不依赖 world；
- chunk 与 revision 不互相调用；
- query 不控制 streaming；
- mutation 不直接调用 snapshot；
- VoxelEngine 不编译依赖 CoreEngine。

**对后续模块的影响：**

降低 crate 落地后再拆循环的成本。

---

## Step 4：冻结 Barrier Commit、异步 Origin Token 和队列矩阵

**修改哪些文件：**

- `modules/world/README.md`
- `modules/chunk/README.md`
- `modules/revision/README.md`
- `modules/query/README.md`
- `modules/mutation/README.md`
- `modules/streaming/README.md`
- `modules/spatial/README.md`
- `modules/mesh-collision/README.md`
- 仓内并发/调度 ADR

**为什么必须先做：**

当前“安全点”和碎片化 Generation 不能防止迟到结果污染新状态。

**是否需要架构源变更：**

公共 Token、Phase 可见性和错误需要；内部队列实现不需要。

**验收标准：**

- 所有异步任务携带完整 Origin Token；
- 每个 Completion 有唯一应用 Phase；
- 每个队列有 producer、consumer、capacity、order、full action、metrics；
- 取消和超时后迟到结果无法发布；
- 第一个可见写入后无可失败校验。

**对后续模块的影响：**

为 Query、Streaming、Spatial 和 Mesh 并发实现建立统一安全模型。

---

## Step 5：统一 Snapshot 并发模型和 Dirty Eviction 耐久协议

**修改哪些文件：**

- `README.md`
- `modules/world/README.md`
- `modules/revision/README.md`
- `modules/snapshot/README.md`
- `modules/chunk/README.md`
- `modules/streaming/README.md`
- 架构源 Snapshot/Capability/持久化契约

**为什么必须先做：**

否则可能长时间暂停 Tick，或者驱逐未耐久的权威修改。

**是否需要架构源变更：**

是，涉及 Cut projection、durability acknowledgement 和 Profile loss boundary。

**验收标准：**

- Cut Barrier 只覆盖固定 Cut 和建立 CaptureRef；
- 编码期间写入可继续且不会污染旧 Cut；
- Pin/COW 有上限和确定失败结果；
- Dirty Chunk 未获 durability ack 时不能 Unload；
- 磁盘满、回执丢失、Snapshot 失败均有明确状态和 fixture。

**对后续模块的影响：**

解除 Snapshot、Streaming 和恢复链路阻塞。

---

## Step 6：收窄 migration 并修正公共 Manifest

**修改哪些文件：**

- `modules/migration/README.md`
- `modules/README.md`
- 架构源 `ADR-013-migration-dag.md`
- `schemas/migration-manifest.schema.json`
- Migration valid/invalid/crash fixtures

**为什么必须先做：**

当前 Host 和 Voxel 同时拥有 Staging/DAG 恢复职责，且 Schema 不满足 ADR。

**是否需要架构源变更：**

是。

**验收标准：**

- Voxel 只拥有节点语义和节点局部结果；
- Host 只拥有完整图、目录、Checkpoint 索引和激活；
- Manifest 含 ADR 要求的 Hash/Tool version；
- Crash-at-node 后旧 Active 保持；
- 可从不可变输入或已验证 Checkpoint 重跑。

**对后续模块的影响：**

消除 snapshot/migration/Host 的恢复责任重叠。

---

## Step 7：确定物理 crate map 和测试支撑层

**修改哪些文件：**

- 新增仓内 crate-map/implementation-readiness ADR
- `modules/README.md`
- 测试规范文档

**为什么必须先做：**

逻辑模块不应被机械映射为十个互相引用的 crate。

**是否需要架构源变更：**

不需要，除非暴露新的公共 Port 字段。

**验收标准：**

- 明确 generated-contracts、port、domain-types、storage-port、test-support；
- 明确哪些逻辑模块可在同一 crate 内；
- Reference Port 与 Native Port 使用同一公共契约；
- 不存在 generic common/global singleton/event bus。

**对后续模块的影响：**

允许按边界实现，而不是按目录数量实现。

---

## Step 8：执行并保存机器验证证据，再进行 Foundation 复审

**应执行：**

```text
git status --short --branch
git show --stat --oneline f415ae2
rg -n "TODO|TBD|FIXME|后续补充|适当处理|安全点|按策略" README.md modules

node .spec/tools/spec-lint.mjs
node --test .spec/tools/spec-lint.test.mjs
sha256sum -c docs/architecture/.baseline.sha256

cd LumioGameEngineArchitecture
python3 tools/lumio_contract.py validate
```

引入 Cargo 后再执行：

```text
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
```

**为什么必须最后执行：**

文档结构校验不能证明架构语义正确；但语义修正后仍必须用机器验证防止链接、基线、Schema 和 Fixture 漂移。

**是否需要架构源变更：**

前序步骤完成后执行。

**验收标准：**

- 所有命令有完整输出证据；
- Baseline Hash 一致；
- Contract validator 全绿；
- P0/P1 Findings 关闭；
- A—I 场景至少在 Reference Port/Fixture 层可执行。

**对后续模块的影响：**

满足进入正式 Foundation 实现的最低门槛。

---

# 最终结论

## **REWORK REQUIRED**

- **总体评分：61 / 100**
- **模块边界判断置信度：中高，约 0.78**
- **进入 Foundation 的建议：暂不进入正式 Rust Foundation 实现**
- **是否建议先补架构源 Schema/ADR/Fixture：是，且为强制前置**

十个逻辑模块的**总体拆分方向是合理的**：

- Query 和 Revision 应独立；
- Snapshot、Streaming、Migration 不应合并；
- Spatial 与 Mesh/Collision 应分开；
- world 应作为组合根；
- Gameplay、ECS、Host、AOI、Renderer、Physics 和双实例边界总体正确。

但当前文档仍有三个必须先修的 P0：

1. `SnapshotCut` 权威所有权冲突；
2. CrossWorld participant 幂等与崩溃恢复协议不完整；
3. Chunk 数据与 Revision 没有定义成不可分割提交。

再加上公共 Voxel Contract 未完成、依赖图不唯一、Query 一致性、异步 Completion、Snapshot 并发、Dirty Eviction 和 Migration 所有权等 P1 问题，这套文档**尚不足以安全指导正式 Rust 实现**。

它不是 `BLOCKED`：公共基线和整改方向均已足够明确；但必须先完成上述架构重构和 Architecture Gate，之后才适合进入 Foundation。
