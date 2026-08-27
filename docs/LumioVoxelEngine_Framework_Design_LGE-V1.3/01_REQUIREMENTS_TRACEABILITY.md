# Requirements Traceability

This ledger is an extraction aid. The original source remains authoritative; extraction does not change normative meaning.

| Ref | Source | Line | Extracted requirement | Covered by |
|---|---|---:|---|---|
| `REQ-0001` | `INPUT_BRIEF` | 3 | 你的唯一任务：在【不改公共语义、不重写已冻结边界】的前提下，把已经挂红的框架拆成「每个模块都能直接开工」的详细设计 + 可实现任务。你交付的是设计与任务，不是生产代码。 | Master design + package acceptance criteria + task-card forbidden-change field |
| `REQ-0002` | `INPUT_BRIEF` | 6 | 0. 仓库事实（先读再写，禁止凭记忆发明） | Master design + package acceptance criteria + task-card forbidden-change field |
| `REQ-0003` | `INPUT_BRIEF` | 10 | 定位：可复用的 Rust VoxelWorld 领域实现。Server 权威世界、Client Replica 世界、LocalEmbedded 双实例必须完全隔离。C# Runtime 只能经版本化 IVoxelWorldPort 和生成契约访问，不能读内部 Chunk Storage。 | Master design + package acceptance criteria + task-card forbidden-change field |
| `REQ-0004` | `INPUT_BRIEF` | 14 | - 公共架构与契约的唯一来源：LumioGameEngineArchitecture | Master design + package acceptance criteria + task-card forbidden-change field |
| `REQ-0005` | `INPUT_BRIEF` | 17 | - 内部决策已冻结：.spec/decisions/0001–0006 | Master design + package acceptance criteria + task-card forbidden-change field |
| `REQ-0006` | `INPUT_BRIEF` | 20 | 必须先完整阅读（按这个顺序，读完再动笔）： | Master design + package acceptance criteria + task-card forbidden-change field |
| `REQ-0007` | `INPUT_BRIEF` | 46 | 11. 若本机看得到架构源仓 LumioGameEngineArchitecture，再读对应 Schema / ADR；看不到就只引用本仓已写明的 $id，禁止编造字段布局。 | Master design + package acceptance criteria + task-card forbidden-change field |
| `REQ-0008` | `INPUT_BRIEF` | 48 | 读完后先用 10 行以内复述：已冻结什么、仍开放什么、你这次不碰什么。复述错了就停，先校正，不要继续拆。 | Master design + package acceptance criteria + task-card forbidden-change field |
| `REQ-0009` | `INPUT_BRIEF` | 51 | 1. 角色边界：你设计框架落地，不改宪法 | Master design + package acceptance criteria + task-card forbidden-change field |
| `REQ-0010` | `INPUT_BRIEF` | 62 | - 推翻已生效 ADR 0001–0006。要改必须新增 ADR，不能改写旧文件。 | Master design + package acceptance criteria + task-card forbidden-change field |
| `REQ-0011` | `INPUT_BRIEF` | 70 | - 把未批准的 VOX-D-001–008 写成已冻结数字。 | Master design + package acceptance criteria + task-card forbidden-change field |
| `REQ-0012` | `INPUT_BRIEF` | 75 | 2. 大原则：成熟方案优先，禁止自己再写一套 | Master design + package acceptance criteria + task-card forbidden-change field |
| `REQ-0013` | `INPUT_BRIEF` | 80 | 对每一个需要算法、存储、压缩、哈希、序列化、网格、空间索引、异步任务、测试夹具的点，必须走这个阶梯，并在产出里留证据： | Master design + package acceptance criteria + task-card forbidden-change field |
| `REQ-0014` | `INPUT_BRIEF` | 85 | 4. 任何自研基础设施必须单独说明：评估过哪些候选、为何都不合格、谁维护、如何替换。没有这四项 = 拆解失败。 | Master design + package acceptance criteria + task-card forbidden-change field |
| `REQ-0015` | `INPUT_BRIEF` | 87 | 许可证默认：MIT / Apache-2.0 / BSD / Zlib。GPL/AGPL 等强传染许可证不得进入候选，除非你显式标成「需法务审核，本次不采用」。 | Master design + package acceptance criteria + task-card forbidden-change field |
| `REQ-0016` | `INPUT_BRIEF` | 90 | - 第三方 API 只能出现在 Adapter 后面，不得漏进 IVoxelWorldPort、生成契约、模块稳定 Port。 | Master design + package acceptance criteria + task-card forbidden-change field |
| `REQ-0017` | `INPUT_BRIEF` | 92 | - 禁止引入「完整 Voxel 引擎 / 完整世界运行时」来替代本仓模块。允许复用的是叶子能力：页压缩、确定性哈希、greedy mesh、空间加速、property test、golden snapshot、有界队列原语。 | Master design + package acceptance criteria + task-card forbidden-change field |
| `REQ-0018` | `INPUT_BRIEF` | 93 | - NativeCore 已声明拥有的能力（Handle、Buffer、Job、通用空间、通用碰撞、通用压缩 Kernel）必须复用 NativeCore，不得在本仓再写一套同名 Kernel。 | Master design + package acceptance criteria + task-card forbidden-change field |
| `REQ-0019` | `INPUT_BRIEF` | 94 | - Canonical 编解码必须走架构源生成契约，不得另选一套 wire 格式。 | Master design + package acceptance criteria + task-card forbidden-change field |
| `REQ-0020` | `INPUT_BRIEF` | 96 | 每个模块的「外部方案表」必须包含这些列： | Master design + package acceptance criteria + task-card forbidden-change field |
| `REQ-0021` | `INPUT_BRIEF` | 102 | 3. 已冻结、必须遵守的骨架（拆任务时当公理） | Master design + package acceptance criteria + task-card forbidden-change field |
| `REQ-0022` | `INPUT_BRIEF` | 105 | 物理 crate（ADR 0006，禁止另起炉灶）： | Master design + package acceptance criteria + task-card forbidden-change field |
| `REQ-0023` | `INPUT_BRIEF` | 127 | 关键协议（写任务时必须点名，不能用「适当处理」带过）： | Master design + package acceptance criteria + task-card forbidden-change field |
| `REQ-0024` | `INPUT_BRIEF` | 129 | - ADR 0002：mutation 是唯一写入协调者。CommitBatch 在第一个可见写入后不可失败。publish 中途不变量失败 => World Faulted，不是可重试错误。chunk.clear_dirty 只能由 Host DurabilityAck 经 world Barrier 触发。 | Master design + package acceptance criteria + task-card forbidden-change field |
| `REQ-0025` | `INPUT_BRIEF` | 131 | - ADR 0005：任何离开 Barrier 的异步任务必须带完整 Origin Token（worldContext, requestId, inputWorldRevision, inputChunkRevisionSet, applyPhase）。队列按矩阵声明容量/满载/可靠性；数值本身仍属 VOX-D-003/006，不得假装已冻结。 | Master design + package acceptance criteria + task-card forbidden-change field |
| `REQ-0026` | `INPUT_BRIEF` | 132 | - 缺 Chunk 必须返回 Ready / NotLoaded / Pending / Unavailable，禁止当空气。 | Master design + package acceptance criteria + task-card forbidden-change field |
| `REQ-0027` | `INPUT_BRIEF` | 133 | - Prepare 无可见副作用。Commit 按 TxnId 幂等。Native 锁内不得回调 C# / Hot Gameplay。 | Master design + package acceptance criteria + task-card forbidden-change field |
| `REQ-0028` | `INPUT_BRIEF` | 134 | - LocalEmbedded 两棵世界树：禁止共享对象引用、Chunk Buffer、锁、指针、Revision 写入。 | Master design + package acceptance criteria + task-card forbidden-change field |
| `REQ-0029` | `INPUT_BRIEF` | 136 | 仍开放、只能提案不能冻结的决策门： | Master design + package acceptance criteria + task-card forbidden-change field |
| `REQ-0030` | `INPUT_BRIEF` | 146 | 对这些门：可以给「Foundation 临时默认值 + 必须用配置快照而不是写死到 Port」的建议，必须标 `unapproved`，并写清用什么 Bench 才能转正。 | Master design + package acceptance criteria + task-card forbidden-change field |
| `REQ-0031` | `INPUT_BRIEF` | 149 | 4. 每个模块必须拆到这个深度（不够详细 = 失败） | Master design + package acceptance criteria + task-card forbidden-change field |
| `REQ-0032` | `INPUT_BRIEF` | 152 | 对下列 10 个逻辑模块，各写一份「模块实现设计包」。只复述现有 README 不算完成。你必须把 README 里的 Port 表面，落成 crate 内的类型、文件和任务。 | Master design + package acceptance criteria + task-card forbidden-change field |
| `REQ-0033` | `INPUT_BRIEF` | 159 | 每个模块设计包必须按这个顺序写，禁止缺节，禁止 TBD/TODO/「后续补充」： | Master design + package acceptance criteria + task-card forbidden-change field |
| `REQ-0034` | `INPUT_BRIEF` | 165 | - 建议目录（逻辑模块目录可以保留；crate 的 mod 映射不得反向依赖） | Master design + package acceptance criteria + task-card forbidden-change field |
| `REQ-0035` | `INPUT_BRIEF` | 166 | - 完整文件树：每个 .rs 文件写 1 句「这个文件唯一负责什么」 | Master design + package acceptance criteria + task-card forbidden-change field |
| `REQ-0036` | `INPUT_BRIEF` | 169 | C. 核心类型与不变式（必须写出 Rust 签名草案，不是散文） | Master design + package acceptance criteria + task-card forbidden-change field |
| `REQ-0037` | `INPUT_BRIEF` | 177 | 禁止写「类似 query 那样」。每个模块把签名写全。 | Master design + package acceptance criteria + task-card forbidden-change field |
| `REQ-0038` | `INPUT_BRIEF` | 180 | - 哪些逻辑必须自研（领域语义） | Master design + package acceptance criteria + task-card forbidden-change field |
| `REQ-0039` | `INPUT_BRIEF` | 181 | - 哪些必须复用（填第 2 节的外部方案表） | Master design + package acceptance criteria + task-card forbidden-change field |
| `REQ-0040` | `INPUT_BRIEF` | 194 | mutation 必须把 CommitBatch 逐步写出来。 | Master design + package acceptance criteria + task-card forbidden-change field |
| `REQ-0041` | `INPUT_BRIEF` | 195 | snapshot 必须把短 Barrier 与 Quiesce 两条路径分开写。 | Master design + package acceptance criteria + task-card forbidden-change field |
| `REQ-0042` | `INPUT_BRIEF` | 196 | restore 与 streaming Load 必须写成两条不相交的路径。 | Master design + package acceptance criteria + task-card forbidden-change field |
| `REQ-0043` | `INPUT_BRIEF` | 202 | 禁止「上游/下游」这种词。 | Master design + package acceptance criteria + task-card forbidden-change field |
| `REQ-0044` | `INPUT_BRIEF` | 213 | 同步 P0 Profile 可以关掉哪些异步能力；关掉后哪些函数仍必须存在、哪些 feature 关闭。 | Master design + package acceptance criteria + task-card forbidden-change field |
| `REQ-0045` | `INPUT_BRIEF` | 216 | 把该模块拆成 3–8 张可独立验收的任务（见第 5 节）。模块太大就按「类型骨架 → 状态机 → Port → 失败路径 → 测试夹具」切，不要一张卡覆盖整个模块。 | Master design + package acceptance criteria + task-card forbidden-change field |
| `REQ-0046` | `INPUT_BRIEF` | 222 | 任务真值格式必须遵守 .spec/tasks/README.md： | Master design + package acceptance criteria + task-card forbidden-change field |
| `REQ-0047` | `INPUT_BRIEF` | 233 | ## 验收标准 | Master design + package acceptance criteria + task-card forbidden-change field |
| `REQ-0048` | `INPUT_BRIEF` | 244 | - slug：kebab-case，目录内唯一。建议：p0-domain-revision-stamps、p0-ops-mutation-commit-batch。 | Master design + package acceptance criteria + task-card forbidden-change field |
| `REQ-0049` | `INPUT_BRIEF` | 246 | - 有邻卡依赖的卡必须写接口节，签名与模块设计包一致。 | Master design + package acceptance criteria + task-card forbidden-change field |
| `REQ-0050` | `INPUT_BRIEF` | 247 | - 卡内禁止：TBD、TODO、后续补充、适当处理错误、类似卡 N、引用尚未定义的类型。 | Master design + package acceptance criteria + task-card forbidden-change field |
| `REQ-0051` | `INPUT_BRIEF` | 248 | - 验收标准必须能被实现者或 reviewer 证伪。例如： | Master design + package acceptance criteria + task-card forbidden-change field |
| `REQ-0052` | `INPUT_BRIEF` | 251 | - 文件集互不重叠的卡才能标为同一 wave。重叠必须串行。 | Master design + package acceptance criteria + task-card forbidden-change field |
| `REQ-0053` | `INPUT_BRIEF` | 254 | - 每个破坏性 Chunk/Revision 变化卡必须同时带：旧版本 fixture、migration 节点（若涉及）、失败恢复。P0 若还没 migration，就在卡里写「本卡不引入破坏性格式」。 | Master design + package acceptance criteria + task-card forbidden-change field |
| `REQ-0054` | `INPUT_BRIEF` | 256 | 另外输出一张总表（不要用「上游/下游」）： | Master design + package acceptance criteria + task-card forbidden-change field |
| `REQ-0055` | `INPUT_BRIEF` | 258 | \| slug \| 模块 \| crate \| wave \| 依赖 \| 文件集摘要 \| 验收命令 \| | Master design + package acceptance criteria + task-card forbidden-change field |
| `REQ-0056` | `INPUT_BRIEF` | 260 | Wave 建议（可调整，但必须证明无环、无文件重叠）： | Master design + package acceptance criteria + task-card forbidden-change field |
| `REQ-0057` | `INPUT_BRIEF` | 261 | Wave 0  工程骨架：toolchain、workspace、contracts crate 空绑定、test-support 空 harness、收口命令写入 testing.md 的「将要执行」清单（只提案，先不改规范，除非用户另允） | Master design + package acceptance criteria + task-card forbidden-change field |
| `REQ-0058` | `INPUT_BRIEF` | 276 | 除了模块设计包，必须再交一份「仓库代码地图」，精确到文件： | Master design + package acceptance criteria + task-card forbidden-change field |
| `REQ-0059` | `INPUT_BRIEF` | 280 | 3. 禁止出现的文件：lib_common.rs、globals.rs、event_bus.rs、everything.rs。 | Master design + package acceptance criteria + task-card forbidden-change field |
| `REQ-0060` | `INPUT_BRIEF` | 281 | 4. domain crate 内 chunk 与 revision 必须是并列 mod，只通过 L2 publish 能力碰头。 | Master design + package acceptance criteria + task-card forbidden-change field |
| `REQ-0061` | `INPUT_BRIEF` | 287 | 7. 对 README 已写的函数名，必须原样使用： | Master design + package acceptance criteria + task-card forbidden-change field |
| `REQ-0062` | `INPUT_BRIEF` | 304 | 只写设计与任务，不写生产 Rust。按现有规范落盘： | Master design + package acceptance criteria + task-card forbidden-change field |
| `REQ-0063` | `INPUT_BRIEF` | 333 | 失败必须修好再交付。 | Master design + package acceptance criteria + task-card forbidden-change field |
| `REQ-0064` | `INPUT_BRIEF` | 336 | 8. 自检（交回前必须自己过一遍，不要等 reviewer） | Master design + package acceptance criteria + task-card forbidden-change field |
| `REQ-0065` | `INPUT_BRIEF` | 345 | 5. CommitBatch、Origin Token、短 Barrier Snapshot、Restore≠Load、Dirty 需 DurabilityAck、缺 Chunk 四态、TxnId 幂等、双实例隔离，都至少有一张卡的验收标准直接钉住。 | Master design + package acceptance criteria + task-card forbidden-change field |
| `REQ-0066` | `INPUT_BRIEF` | 352 | 12. 交回物包含：改动清单、验证证据（lint 命令与输出）、known gaps、知识沉淀落点或「无需沉淀」。 | Master design + package acceptance criteria + task-card forbidden-change field |
| `REQ-0067` | `INPUT_BRIEF` | 358 | 先读第 0 节清单，用不超过 10 行复述冻结面。 | Master design + package acceptance criteria + task-card forbidden-change field |
