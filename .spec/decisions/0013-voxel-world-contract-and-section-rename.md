# 0013 · 体素公共语义改从 `voxel-world-v1.json` 取，16³ 数据单元改名 Section

- 日期:2026-09-04
- 状态:生效

## 背景

本仓一直把 16×16×16 = 4096 格的数据单元叫 `chunk`(`ChunkId {x,y,z}`、`ChunkSlot`、`ChunkPayload`、
`chunk_revision_set`、`CHUNK_PRESENCE`)。这个命名来自 `crates/lumio-voxel-contracts/generated/`——
基线 `LGE-V1.4-2026-08-27` 的只读镜像。

两件事让它站不住:

1. **那份镜像已死。** 生成源仓 `LumioGameEngineArchitecture` 不存在了,镜像永远不可能重新生成。
   [0007](0007-v1.4-implementation-baseline.md) 采用它作实现基线时,它还有上游。
2. **架构仓冻结了体素世界的公共契约**,唯一真值是 `engine/wire/voxel-world-v1.json`
   (`contractId: lumio.voxel-world.v1`)。它把三层分层写死,并且明确点名了这条命名纠正:

   > 任何消费方不得用 Chunk 指代 16×16×16 的数据单元——那个单元的名字是 Section。

   契约里 **Chunk** 是另一个东西:16 个 Section 竖着摞成的列(16×256×16)。世界高 256 格恰好一个
   Chunk 高,所以 Chunk 只有两个坐标,而且**不携带数据、不持有独立 revision**。

## 决策

- **体素分层语义只从活契约取。** `lumio-voxel-contracts` 新增 `wire/voxel-world-v1.json`(架构仓那份的
  逐字节副本)与 `voxel_world` 模块。模块里每一个常量都必须等于 JSON 里的对应字段——这不是靠人眼抄,
  `tests/voxel_world_conformance.rs` 解析同一份 JSON 逐条断言,并校验 JSON 自身的 SHA-256 与
  `CONTRACT_SHA256` 相符;架构仓在场时再逐字节比对上游。改契约的唯一顺序是:架构仓改 → 复制到
  `wire/` → 更新常量与摘要 → 一致性测试变绿。
- **16³ 数据单元一律改名 Section。** `crates/*/src` 下再没有指代该单元的 `chunk` 标识符;
  `src/chunk/` 变成 `src/section/`。规范键从 `c:x:y:z` 变成 `s:<x>:<y>:<z>`,y 是层号,限 0~15。
- **Chunk 作为纯派生的容器概念引入,只有键。** `key::ChunkId` 只有 `x`/`z` 两个字段,没有数据字段、
  没有 revision;它在本仓**没有数据模块**,这正是契约红线 `layering.chunk-carries-no-data` 的结构表达。
  `s:x:y:z → c:x:z`(丢 y)与 `c:x:z → 16 个 Section` 是它的全部内容。
- **元数即防呆:旧式三坐标 `c:x:y:z` 显式失败。** 它不得被解读成 `c:x:z`,也不得被解读成 `s:x:y:z`。
  `KeyError` 除了契约错误码,还带一个 `KeyRejection::LegacyThreeCoordinateChunkKey`——存在的意义是让
  「显式拒绝」可被断言:调用方能证明这个键是被元数守卫挡下的,而不是碰巧解析失败。当它被当作 Section
  键提交报 `unknown_section_key`(契约 `identity.arityIsTheGuard`),被当作 Chunk 键提交报
  `unknown_chunk_key`(契约规则 `key.chunk.arity`)。
- **键的解析只有一份实现。** `lumio-voxel-ops` 里那份重复的 `CanonicalChunkId::parse` 删掉,改调
  `lumio_voxel_domain::key::SectionId`。两份解析器必然漂移,而键语法是契约面——事实上沿用旧仓库状态
  时它们已经开始漂移了。
- **两个错误 id 命名空间并存,由 `is_stable_error_id` 一个谓词收口。** 体素公共语义报活契约的
  snake_case 码(`unknown_section_key`、`section_unavailable`、`dirty_section_not_durable`……);
  契约不定义的引擎通用失败(`InvalidHandle`、`SessionMismatch`、`StaleEpoch`、`BudgetExceeded`……)
  仍报废弃镜像的 `STABLE_ERROR_IDS`。不给后者临时造一套 snake_case,因为那会凭空多出一份没有上游的真值。
- **presence 第二态从 `NotLoaded` 改名 `Unchanged`。** 它是契约 `diffDispatch.presence` 的名字,
  语义是「该 Section 相对原始地图没有改动」,以零字节短票表达。状态迁移拓扑一个不动,只换名字。
- **镜像里躲不掉的两个错名字收进 `legacy_baseline`。** `voxel-chunk-page`(页 schema id)与
  `VoxelChunkResidency`(状态机 id)是冻结产物的 id,不是可以本仓改写的语义。收在一个模块里,
  让「16³ 叫 chunk」在整个工作区只剩这一处,并带着为什么还在的解释。上游重发体素产物时它整体消失。

## 后果

- **两个 golden 断代,改动前写出的字节不再 round-trip。** 这是契约 `limits.notes`(尺寸与坐标语义
  一经冻结即不可变更,改动等于全量转档)在本仓的直接落地,不是测试维护:
  - snapshot manifest 摘要 `b513120c… → 1893afc9…`。manifest 带 `rootIdentity`,而
    `PublishedStateRoot::identity` 的指纹包含目录的 `Debug` 渲染,键类型改名就把它挪了。新值出自
    `tools/canonical/canonical_encoding_oracle.py`(其 `rootIdentity` 输入随之更新),不是从失败输出里抄的。
  - 差分 trace golden `5a77a11d… → 4acbb1e4…`。trace 帧里有 Section 键,`c:` 变 `s:`。参考实现与
    Rust 实现是同一契约的两份独立实现,两边各自落到同一个新值——这才是它可信的理由。
- **`ChunkPayload::schema_id()` 仍然返回 `voxel-chunk-page`。** 值是死基线的产物 id,不是分层语义;
  见上面的 `legacy_baseline`。
- 契约里 `blockEntityBinding`(方块↔实体绑定)的 6 个错误码已按契约登记进表,但本仓尚未实现该面;
  错误码表与契约逐条对齐,实现留给后续任务卡。
