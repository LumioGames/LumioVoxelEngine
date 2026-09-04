# revision 模块

> World/Section Revision、读取一致性、比较、Snapshot Pin/COW 与 Revision 冲突语义。
> 物理 crate：`lumio-voxel-domain`（[0006](../../.spec/decisions/0006-crate-map.md) / [0007](../../.spec/decisions/0007-v1.4-implementation-baseline.md)）；L2 ReadView 在本 crate，不进入 `lumio-voxel-ops`。

## 模块定位与目标

`revision` 是 VoxelWorld 的版本基础。它让每次读取、修改、Snapshot 和空间投影都能说明自己观察到哪个版本，避免用一个没有域语义的整数或时间戳代替一致性协议。`WorldRevision` 用于世界级排序和 Snapshot，`SectionRevision` 用于局部乐观并发；两者都必须单调且带 Context 生命周期。

## 负责什么

- 持有每个 World 的单调 `WorldRevision` 与每个 Section 的 `SectionRevision`。
- 生成只读 `RevisionStamp`/读取令牌，供 Query、Snapshot、Spatial 和 Mesh/Collision 结果携带。
- 比较 `Expected Revision` 与当前版本，输出稳定 `RevisionConflict` 诊断，不静默覆盖。
- 在协调 Snapshot Cut 中提供 Pin 或 Copy-on-Write（COW）视图，保证异步编码期间旧 Cut 不被后续写入污染。
- 管理 Revision 与 World/Section Generation/Context 的关联；销毁或上下文切换后拒绝旧令牌。
- 提供 Revision 变化摘要，供 Mutation、Replay、Audit 和 Failure Bundle 关联。

## 明确不负责什么

- 不存储 Section/Block 数据，不决定 Section 布局或压缩方式（归 [section](../section/README.md)）。
- 不执行 Mutation、Reservation、权限或 Gameplay 资源检查（归 [mutation](../mutation/README.md) 或上层 Runtime）。
- 不决定何时 Tick、何时生成 SnapshotCut 或何时加载 Section；只提供版本操作。
- 不回调 `section`，不自行应用 Block 写入；递增只发生在 `mutation` 的 CommitBatch publish 中。
- 不把 `TickId`、`ConfigRevision`、`ReplicationRevision` 等其他域版本混成 Voxel Revision。
- 不向 C# 暴露可变引用、内部计数器或跨 World 的共享令牌。

## 拥有的状态与资源

- `WorldRevision` 当前值及单调递增策略。
- `SectionId -> SectionRevision` 的有界表和 Generation 校验信息。
- 活跃读取令牌、Snapshot Pin/COW 记录及其引用计数/租约。
- Revision 变化摘要与冲突诊断上下文。

## 输入、输出与稳定接口

- **输入**：World/Section 创建与销毁通知、读取范围、Expected Revision、Mutation 提交摘要、Snapshot Cut 请求。
- **输出**：读取 `RevisionStamp`、Revision 比较结果、Pin/COW 句柄、提交后的新版本和稳定冲突原因。
- **本仓 Port 表面**（Stamp 形状见架构源 `voxel-revision-stamp`）：`current_world() -> WorldRevision`；`current_section(section_id) -> SectionRevision`；`observe(scope) -> RevisionStamp`；`check(expected, observed) -> Ok | RevisionConflict`；`pin(cut) -> SnapshotPin | StableError`；`release(pin)`；`advance(changes) -> RevisionDelta`。

## 依赖（编译 / 控制流 / 事件与数据）

- **编译依赖**：`LumioNativeCore` 的固定宽度 ID、Handle 和稳定错误模型；架构源 Revision 基础类型。不依赖 `section` 或其他上层 Voxel 模块。
- **被谁调用**：`world` 提供 Context 生命周期；`mutation` 在 CommitBatch 中请求 publish；`query`/`snapshot`/`spatial`/`mesh-collision` 读取 Stamp 或请求 Pin。
- **发布/消费**：发布 RevisionDelta / Stamp；消费 Mutation 已验证的变化摘要。不调用 `section`。

## 生命周期与状态机

模块生命周期：

```text
Uninitialized -> Active -> Quiescing -> Closed
Active -> Faulted
```

Snapshot Pin 生命周期：

```text
Requested -> Pinned -> Released
Requested/Pinned -> Expired | Invalidated
```

- `Active` 期间 Revision 只能在所属 Simulation Barrier 递增。
- `Quiescing` 拒绝新的写入版本分配，但允许已批准的只读 Pin 按租约完成。
- `Closed` 后所有令牌和 Pin 均以稳定错误失效，不能作用于新建 World。

## 线程、队列与并发所有权

- 版本递增、SectionRevision 表更新和 Pin 建立在 Voxel 所属 Barrier 串行执行；本模块不创建 Host Wall Clock。
- 只读 `RevisionStamp` 可以复制到异步任务，但不能携带可变引用；异步结果必须校验 Context/Generation。
- Pin 的引用计数与释放可由受控线程调用，但不跨 FFI 持有 Rust 锁，也不阻塞 Simulation Owner Thread。
- 不拥有无界队列；冲突诊断和 Metrics 通过上层有界观测管道发送。

## 正常数据流与失败路径

- **正常读**：请求范围 → 读取当前版本 → 生成 Stamp → 调用方读取 → 结果回带 Stamp。
- **正常写**：Expected Revision 校验通过 → `mutation` 的 CommitBatch 同时发布 Section 页与 `advance` → 公开 RevisionDelta。本模块不单独应用 Block。
- **Snapshot**：Barrier 接收 Runtime Cut → Pin/COW → 编码期间继续读写 → 完成后释放 Pin。
- **失败路径**：
  - Expected 版本落后：返回 `RevisionConflict`，不写入、不递增。
  - Section/World Context 已销毁：返回 `InvalidHandle`/稳定上下文错误，不触碰新实例。
  - Pin 超时或预算不足：拒绝 Pin，保留当前 Active 版本。
  - Revision 溢出或内部表不一致：进入 `Faulted`，输出证据并由 `world` 决定实例处置。 publish 一旦开始可见则不得退回普通可重试错误。

## 错误分类、恢复与降级

- **可重试**：读取 Stamp 过期（调用方重新观察后重试）；Pin 暂时达到并发上限。
- **可拒绝**：Expected Revision 不匹配、未知 Section、Context/Generation 不匹配、Pin 超预算。
- **可致命**：无法保持单调性或检测到版本表损坏；实例必须停止接受写入并进入恢复/重建路径。
- **降级**：不把冲突降级为强制覆盖；只能由上层显式重新读取并生成新命令。

## 配置、Capability 与安全约束

- Revision 宽度、持久化表示和 Schema Epoch 由架构源契约决定，模块不得自行缩窄或复用其他域字段。
- Pin/COW 内存预算来自不可变配置快照；超限必须返回稳定原因并计数。
- 令牌只在所属 World/Context 有效；不得把它当作权限凭据或跨 World 授权。
- LocalEmbedded 的两份 World 各自维护 Revision；任何共享计数器都视为边界违规。

## 日志、Metrics、Trace 与 Audit

- Diagnostic：Revision 冲突、Pin 等待/超时、表水位、Generation 失效。
- Metrics：World/Section Revision 递增速率、冲突率、活跃 Pin 数、Pin 持续时间、COW 字节。
- Audit/Failure Bundle：破坏性版本迁移、Revision 表故障和 Snapshot Cut 失败带 `worldId/sectionId/worldRevision/sectionRevision/snapshotId/traceId`。

## 测试面、故障矩阵与性能指标

- **测试面**：单调性、Section 独立递增、Expected 冲突不写入、Stamp 传播、Pin/COW 隔离、销毁后令牌拒绝、Local 双实例隔离。
- **故障矩阵**：并发读写、Pin 超时/预算耗尽、Context Generation 复用、计数器边界、Snapshot 期间写入、异常退出恢复。
- **性能指标**：Revision 比较 p50/p95/p99、Pin 建立/释放延迟、每 Tick 版本表更新开销、COW 峰值内存。

## 对应 ADR、Schema 与 Fixture

- 本仓 [0001](../../.spec/decisions/0001-snapshotcut-vs-capture-ref.md)、[0002](../../.spec/decisions/0002-barrier-commit-batch.md)、[0004](../../.spec/decisions/0004-snapshot-short-barrier-vs-quiesce.md)、[0006](../../.spec/decisions/0006-crate-map.md)、[0007](../../.spec/decisions/0007-v1.4-implementation-baseline.md)。
- 架构源 `docs/adr/ADR-003-cross-world-txn.md`：Expected Revision、SnapshotCut、幂等和恢复语义。
- 架构源 `schemas/common.schema.json` / `schemas/session-revision-vector.schema.json`：`revision` 与 `chunkRevisionSet`（Runtime 拥有的跨域向量，字段名是死基线冻结拼写，不随本仓 Section 改名；本仓自己的 stamp 用 `section_revision_set`）；正例 `fixtures/valid/session-revision-vector.json`，反例 `fixtures/invalid/session-revision-negative.json`。
- 架构源 `schemas/snapshot-header.schema.json`：Snapshot 版本关联；正例 `fixtures/valid/snapshot-active.json`。
- 架构源 `schemas/voxel-revision-stamp.schema.json`：WorldRevision 与 SectionRevisionSet；ADR-024。Pin/COW 策略仍属 VOX-D-005。

## 尚未批准的决策门

- **VOX-D-005**（Snapshot Pin/COW 与子 Section Diff 粒度）：载荷线格式已交付；物化策略待 Bench（架构源 D-014）。
- Revision 数值宽度、溢出处理和分布式/跨 World 扩展属于公共契约问题，必须先在架构源新增 ADR/Schema，不能在本模块单独决定。
