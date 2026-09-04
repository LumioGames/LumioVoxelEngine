# section 模块

> Section 坐标、Block 数据、页布局、页编码、边界校验与 Section 驻留状态。
> 物理 crate：`lumio-voxel-domain`（[0006](../../.spec/decisions/0006-crate-map.md) / [0007](../../.spec/decisions/0007-v1.4-implementation-baseline.md)）；与 `revision` sibling，不互调。
> 命名以活契约 `lumio.voxel-world.v1` 为准：16×16×16 = 4096 格的数据单元叫 **Section**，`Chunk` 是 16 个 Section 摞成的列容器、不携带数据、不持有独立 revision（[0013](../../.spec/decisions/0013-voxel-world-contract-and-section-rename.md)、[`features/voxel-section-chunk.md`](../../.spec/knowledge/features/voxel-section-chunk.md)）。

## 模块定位与目标

`section` 是 Voxel 数据的最小拥有者。它把世界坐标映射到稳定的 `SectionId`/`BlockCoord`，管理 Section 内部 Block 与页的存储和生命周期，并为上层提供受控的只读/可写视图。具体内存布局、压缩字典和后端实现都必须藏在模块内部，不能泄漏到 `IVoxelWorldPort` 或 C#；尺寸与键语义则由契约冻结，不是本模块可自行决定的自由度。

## 负责什么

- 定义并校验 `SectionId`、局部 Block 坐标、世界边界和 `SectionId → ChunkId` 派生（`s:x:y:z → c:x:z`，丢弃 y）。
- 持有 Section 内的 Block 值、稀疏/致密页、脏页标记和数据 Generation。
- 提供只读页视图、受 Barrier 保护的可写视图和有限范围迭代器；不返回裸 Storage 指针。
- 管理 Section 数据状态：未分配、加载、可用、脏、驱逐和卸载。
- 通过 NativeCore/Adapter 进行页压缩、解压和 Buffer 交换；第三方库不进入稳定契约。
- 暴露数据校验、页摘要和变更范围，供 `revision`、`snapshot`、`streaming` 和投影模块消费。

## 明确不负责什么

- 不拥有 World 生命周期、全局 Revision、Load/Unload 调度或 IO Worker（分别归 [world](../world/README.md)、[revision](../revision/README.md)、[streaming](../streaming/README.md)）。
- 不执行权限、资源、Gameplay 规则或 CrossWorld 协调。
- 不自行递增公共 `WorldRevision`/`SectionRevision`；只提供受控 WriteView，由 `mutation` 的 CommitBatch 在 Barrier 同时发布页与版本。
- 不调用 `revision` 服务。
- 不决定 Mesh、Collision、AOI 或 Renderer 语义。
- 不把压缩库类型、页指针或内部布局写入 ABI/Generated Contract。
- 不给 `Chunk` 建数据模块：Chunk 只有键、不携带字节、不持有独立 revision（契约红线 `layering.chunk-carries-no-data`）。

## 拥有的状态与资源

- Section 坐标索引、Block 页和页级元数据。
- Section 数据状态、脏页集合、加载错误和本地 Generation。
- 受限的压缩/解压 Buffer 池与页校验上下文。
- 只读/可写视图的租约，租约结束后视图自动失效。

## 输入、输出与稳定接口

- **输入**：Section 创建/销毁、Load 完成页、查询坐标范围、Mutation WriteSet、Pin 视图请求、Host DurabilityAck 转发、restore 页。
- **输出**：Block/页只读视图、变更范围、压缩页、Section 可用性、Dirty 状态和稳定数据错误。
- **本仓 Port 表面**（载荷信封字段见契约 `sectionPayload.envelope`；页 schema id 仍是死基线的 `voxel-chunk-page`，见 `lumio_voxel_contracts::legacy_baseline`）：`create(id) -> SectionRef | StableError`；`read(view, coord) -> BlockValue | Missing`；`borrow_read(ref, scope) -> ReadView`；`borrow_write(ref, reservation) -> WriteView`；`publish(write_set)`；`clear_dirty(ack)`；`materialize_pages(decoded)`；`seal_page(ref) -> CompressedPage`；`validate(ref) -> SectionHealth`；`unload(ref) -> Unloaded`。

## 依赖（编译 / 控制流 / 事件与数据）

- **编译依赖**：`LumioNativeCore` 的内存、Buffer、压缩和稳定错误 Kernel；`lumio-voxel-contracts` 的契约常量与错误码。不依赖 `revision`、Runtime 或 Game 源码。
- **被谁调用**：[world](../world/README.md)（Context/生命周期）、[streaming](../streaming/README.md)（加载结果与驱逐指令）、[mutation](../mutation/README.md)（CommitBatch WriteSet）、[query](../query/README.md)（ReadView）。
- **发布/消费**：发布页视图与 Dirty/可用性；消费 Host DurabilityAck（经 world）以 `clear_dirty`。不调用 `revision`。

## 生命周期与状态机

单个 Section 的设计状态机：

```text
Unallocated -> Loading -> Ready <-> Dirty
Ready/Dirty -> Evicting -> Unloaded
Loading/Ready/Dirty/Evicting -> Failed
```

- `Loading` 期间只能接收受限的页填充，不向 Query 宣称 `Ready`。
- `Dirty` 表示存在尚未被 Snapshot/WAL 记录的变更，不等于已提交到持久存储。清除 Dirty 的唯一入口是 Host `DurabilityAck` 经 `world` 到达的 `clear_dirty`；回执只清除 revision ≤ 其声明上界的脏标记，更晚的写入让该 Section 继续为脏（契约 `residency.ackCoverage`）。
- `Evicting` 先拒绝新写入，等待读视图、Pin、构建任务和 Reservation 结束。未获 DurabilityAck 的 Dirty Section 不得进入 `Unloaded`（契约 `residency.evictionFence`，违反即 `dirty_section_not_durable`）；也不得按时间或 LRU 淘汰脏 Section。P0 全驻留 Profile 可禁用 Unload。
- `Failed` 保留错误和必要的原始 Buffer 引用，恢复由 `streaming`/Host 决定；失败页不能被当作空 Section。

## 线程、队列与并发所有权

- Section 数据写入只在 Voxel 所属 Simulation Barrier 的 `WriteView` 中进行。
- Load/Unload 和压缩任务由 `streaming`/Native Job 调度；后台线程不能直接改变公共 Section 状态。
- 读视图可以跨线程短暂使用，但必须带 Generation/Revision 并有明确释放点；不跨 FFI 持有内部引用。
- 页压缩 Buffer 池和任务队列有界；满载返回 `QueueFull`/`BudgetExceeded`，不能无限分配。

## 正常数据流与失败路径

- **加载**：`streaming` 请求 → 读取/校验页 → 解压/物化 → `Ready` → Barrier 发布可用性。
- **读取**：坐标边界校验 → Section 状态检查 → 只读视图读取 → 返回 Block 与 Revision 由 `query` 补齐。
- **写入**：`mutation.prepare` 锁定可写范围 → CommitBatch 在 infallible publish 中同时发布 WriteView 页、Dirty 摘要和 Revision → 之后不再做可失败校验。
- **清除 Dirty**：Host DurabilityAck → `world` Barrier → `clear_dirty`。
- **恢复物化**：`world.restore` → `materialize_pages`；不走 Streaming Load。
- **失败路径**：越界、页摘要校验失败、解压预算超限、Generation 失效、驱逐期间迟到写入均返回稳定错误，不静默填零或复活旧 Section。页载荷摘要必须在任何解释之前校验（契约 `payload.digest-before-interpretation`）。publish 中途失败则 World `Faulted`。

## 错误分类、恢复与降级

- **可重试**：暂时未送达、IO 短暂失败、压缩任务队列暂满（由 `streaming` 有限重试）。
- **可拒绝**：坐标越界（`coordinate_out_of_bounds`）、层号越界（`section_y_out_of_range`）、键元数或前缀不合法（`unknown_section_key` / `unknown_chunk_key`）、Section 未 Ready 的写入、页大小/解压比超限、过期视图或 Generation 不匹配。
- **可致命**：数据校验持续失败、Storage 内部不变量破坏；Section 进入 `Failed` 并上报 World 故障域。
- **降级**：允许显式返回契约四态里的 `Unchanged`/`Pending`/`Unavailable`；`Unchanged` 必须是零字节短票，与「未送达」在语法上可区分，`Pending` 与 `Unavailable` 不得被物化成空气（契约 `presence.missing-is-not-air`）。不得把错误 Section 当空世界，也不得绕过页摘要校验。

## 配置、Capability 与安全约束

- Section 尺寸（16³ = 4096 格）、层号范围（0~15）与键语法由契约 `limits` / `identity` 冻结，改动等于全量转档；页大小、压缩和内存预算来自版本化配置/Schema。
- 所有页输入执行长度、SHA-256 摘要、解压比和分配上限检查；输入不可触发代码执行。
- 第三方压缩/存储 API 经 Adapter 隔离，版本、许可证、SBOM、AOT 和确定性证据由决策门管理。
- LocalEmbedded 的 Server/Client 两个实例各自持有 Section Storage，禁止共享页 Buffer。
- 光照是派生数据，不入页载荷、不落盘（契约 `lighting.never-on-the-wire`）。

## 日志、Metrics、Trace 与 Audit

- Diagnostic：Section 状态迁移、页摘要/解压失败、坐标越界、Generation 失效。
- Metrics：Ready/Dirty Section 数、页密度/压缩比、Load/Unload 延迟、解压分配、队列深度和失败率。
- Audit/Failure Bundle：破坏性数据转换、Section 丢失、恢复保留的原始页和关联 `sectionId/worldRevision/sectionRevision/traceId`。

## 测试面、故障矩阵与性能指标

- **测试面**：坐标边界和负坐标、`SectionId`/`ChunkId` 键解析与派生、旧式三坐标 `c:x:y:z` 的显式拒绝、Block 读写、页 round-trip、压缩确定性、Generation/视图失效、状态机迁移。
- **故障矩阵**：截断页、错误 SHA-256 摘要、解压炸弹、IO 失败、驱逐与读写竞争、OOM、重复 Load/Unload、Local 双实例隔离。
- **性能指标**：Block 查询 p50/p95/p99、页读写吞吐、压缩比/CPU、内存峰值、Load/Unload 尾延迟。

## 对应 ADR、Schema 与 Fixture

- 本仓 [0002](../../.spec/decisions/0002-barrier-commit-batch.md)、[0004](../../.spec/decisions/0004-snapshot-short-barrier-vs-quiesce.md)、[0006](../../.spec/decisions/0006-crate-map.md)、[0007](../../.spec/decisions/0007-v1.4-implementation-baseline.md)、[0013](../../.spec/decisions/0013-voxel-world-contract-and-section-rename.md)。
- 活契约 `lumio.voxel-world.v1`（本仓副本 `crates/lumio-voxel-contracts/wire/voxel-world-v1.json`）：分层与尺寸、`identity` 键语法、`sectionPayload` 四档编码与载荷信封、`diffDispatch` 四态 presence、`residency` 脏页与回执覆盖。一致性由 `cargo test -p lumio-voxel-contracts --test voxel_world_conformance` 逐条断言。
- 死基线 `LGE-V1.4-2026-08-27` 的只读镜像仅余产物 id 在消费：页 schema id `voxel-chunk-page`、驻留状态机 id `VoxelChunkResidency`，收在 `lumio_voxel_contracts::legacy_baseline`；契约不定义的引擎通用失败仍报其 `STABLE_ERROR_IDS`。

## 尚未批准的决策门

- **VOX-D-002**（Block 存储和压缩策略）：临时通过 Adapter/页接口隔离，需密度、CPU、内存、许可证和确定性 Benchmark。

> **VOX-D-001（数值 profile）已由契约收口**：Section 16³ = 4096 格、每 Chunk 16 个 Section、世界高 256 格、调色板上限 256 项均由 `limits` 冻结（`limits.notes`：一经冻结即不可变更）。原门里「尺寸待 Bench」的部分不再成立；剩余的页大小与内存预算调优归 VOX-D-002。历史记录见 [`docs/evidence/decision-gates/VOX-D-001-chunk-profile.md`](../../docs/evidence/decision-gates/VOX-D-001-chunk-profile.md)。
