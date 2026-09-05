---
name: voxel-section-chunk
description: Section / Chunk 分层与规范键——数据单元、列容器、键语法与契约来源;动体素身份或键前查
metadata:
  type: doc
  status: 实施中
---

# Section / Chunk 分层与规范键

体素世界的身份层:哪个是数据单元、哪个是容器、它们的键长什么样、这些值从哪来。
决策与取舍记在 [0013](../../decisions/0013-voxel-world-contract-and-section-rename.md),本篇只描述现状。

## 背景 / 目标

公共语义的唯一真值是架构仓 `engine/wire/voxel-world-v1.json`(`contractId: lumio.voxel-world.v1`)。
本仓消费它,不另写一份;`generated/` 下那份 `LGE-V1.4-2026-08-27` 镜像的生成源仓已不存在,
它用 `Chunk` 指代 16³ 数据单元,活代码不再从它取分层语义。

## 设计

### 三层

| 层 | 尺寸 | 携带数据 | 是什么的单位 |
|----|------|----------|--------------|
| Block | 1×1×1 | 是(一个 32 位 BlockId) | 世界最小单位 |
| **Section** | 16×16×16 = 4096 格 | **是** | 最小同步单位 / 驻留单位 / 版本锚点(SectionRevision) |
| **Chunk** | 16×256×16 = 16 个 Section | **否** | 存档打包 / 按列计算 / 流式批量请求 |

世界高 256 格 = 恰好一个 Chunk 高,所以垂直方向只有一层 Chunk,Chunk 只需两个坐标,
Section 的层号取值 0~15 恰好 4 bit。

### 键(`lumio_voxel_domain::key`)

- **Section 键** `s:<x>:<y>:<z>`——`SectionId`,x/z 是 int32 全域,y 限 0~15。
- **Chunk 键** `c:<x>:<z>`——`ChunkId`,**只有 x/z 两个字段**,没有数据字段、没有独立 revision。
  Chunk 在本仓没有数据模块,这就是契约红线 `layering.chunk-carries-no-data` 的结构表达。
- 派生:`s:x:y:z` 所属 Chunk 是 `c:x:z`(丢 y);`c:x:z` 含且仅含 `s:x:0:z … s:x:15:z` 共 16 个。
- 规范写法:十进制、无前导零、不得 `-0`;负坐标是一等公民。

**元数即防呆。** Chunk 键两坐标、Section 键三坐标且前缀不同,所以旧式三坐标 `c:x:y:z` 在新语法下
语法即非法,必须显式失败——不得被解读成 `c:x:z`,也不得被解读成 `s:x:y:z`。`KeyError` 除了契约
错误码还带 `KeyRejection`,`LegacyThreeCoordinateChunkKey` 让「显式拒绝」这件事本身可被断言,
而不是只看到一个笼统的解析失败。

### 方块存储与载荷

- `lumio_voxel_domain::section::SectionStorage` 以不可变 `Arc` 状态提供 `Uniform` / `Palette` / `Raw`
  三态存储；写入经 `Arc::make_mut` 形成完整新状态后替换，既有快照继续读取旧状态。格访问只接收
  `CellOffset`，世界坐标便利入口复用 `block::CellOffset::from_world` 的 y/z/x 固定顺序。
- Palette 固定使用 8 位索引。死槽平时保留；撞到 256 项时才用栈上 256 位图扫描活槽，有死槽就复用，
  全活才升级 Raw，不维护常驻引用计数或空闲链表。全量序列化按实际活项重编，Raw 写入后种类降到
  256 以内时重新降级。
- `SectionPayloadEnvelope` 携带 Section 键、revision、编码、载荷长度与 SHA-256；解码先验摘要，再解释
  Uniform / Palette / Raw 或每条 6 字节的 Delta。Delta 必须匹配接收方基线 revision，首次送达只接受
  全量编码，且 Delta 必须显式携带 base revision、target revision 必须严格大于 base。`ChunkRecord` 的
  有效形状只有 `ChunkId`，数据字节或独立 revision 都被拒绝。
- `SectionPayload::from_storage` 从同一份 canonical `SectionStorage` 同时生成密封页与 COW sidecar；构造器
  会拒绝 page bytes 与 sidecar 不一致。通用 `from_pages` 仍可承载 opaque page，但没有 sidecar 的 Ready
  Section 不可进入结构化 mutation，不能被回退解释成全空气。sidecar 摘要参与发布 Root 身份；无
  sidecar 的旧 opaque directory / replacement identity 保持兼容。
- Mutation 对 published sidecar 做局部 COW，Dirty frontier 记录新发布的 SectionRevision。`BlockReadWorld` 与
  `PhysicsWorld` 均从同一个 `PublishedReadView` 物化只读视图，所以 commit 后的方块值在玩法读与物理查询
  中来自同一份 Section storage，而不是各自维护权威副本。`Unchanged` 是零字节短票，两种只读视图都必须
  通过 `SectionStorageResolver` 取得原图 baseline；缺 resolver 时明确拒绝，不能伪造空气或降成 Unavailable。
- 批量读的 `*_into` 路径由调用方同时提供 BlockId 与连续 Section 段元数据缓冲，只返回固定大小的计数摘要；
  公共 visitor 先对整条请求做无写入预检，再直接写调用方缓冲，不在体素侧构造与查询体积成正比的结果数组。
- 物理材质表的键是 `BlockType`，同类型的全部 `BlockState` 共用一项材质；非空气 BlockType 缺映射时报
  `unknown_material_class`，不能静默当成 Empty / Miss。Sweep 以几何进入 fraction 比较已知命中与未决区，
  远处 Pending / Unavailable 不得覆盖更近且已经证明的命中。
- Mutation ledger 对同一 `TxnId` + fingerprint 回放原始 receipt 字节与首次提交证据；重复 prepare 不再读取
  当前 Section storage，因此首次提交后即使 Section 已卸载或变成 `Unchanged`，回放仍保持字段等价。
- Region pin 的 caller / host budget 由调用方注入，声明在超过有效预算后立即停止消费输入；超大 region
  在展开 Section 键前先做 checked 规模计算。卸载同时受 Dirty durability frontier 与 pin hook 约束。
- `WorldRouter` 的 query / prepare / commit / abort envelope 必须携带当前冻结配置的精确 `configHash`；空值与
  不匹配值都报 `SessionMismatch`，Native provider 只通过 `VoxelWorld::config_hash` 取同一身份。

### 错误码

体素公共语义报活契约 `errorCodes` 里的 snake_case 码:`unknown_section_key` / `unknown_chunk_key` /
`section_y_out_of_range` / `coordinate_out_of_bounds` / `section_unavailable` / `stale_section_revision` /
`dirty_section_not_durable` / `section_digest_mismatch` 等。契约不定义的引擎通用失败(`InvalidHandle`、
`SessionMismatch`、`StaleEpoch`、`BudgetExceeded`……)仍报废弃镜像的 `STABLE_ERROR_IDS`。
两个命名空间由 `lumio_voxel_contracts::is_stable_error_id` 一个谓词收口,调用方不必知道来自哪边。

### 契约来源怎么保证不漂

- `crates/lumio-voxel-contracts/wire/voxel-world-v1.json` 是架构仓那份的逐字节副本。
- `crates/lumio-voxel-contracts/src/voxel_world.rs` 的每个常量都必须等于 JSON 里的对应字段。
- `cargo test -p lumio-voxel-contracts --test voxel_world_conformance` 解析同一份 JSON 逐条断言,
  另外校验 JSON 的 SHA-256 与 `CONTRACT_SHA256` 相符;架构仓在场时(默认找同级
  `../LumioGameEngine/engine/wire/`,或设 `LUMIO_ENGINE_WIRE_DIR`)再逐字节比对上游,不在场时打印
  跳过原因而不是假装通过。
- 改契约的顺序:架构仓改 JSON → 复制到 `wire/` → 更新 `CONTRACT_SHA256` 与常量 → 测试变绿。
  本仓不得自行改写契约语义;发现缺口回架构仓提。

## 待解决

- `blockCatalog` 的占位 BlockType、保留号段与缺失 `materialClass` 错误优先级仍有三处公共契约冲突，
  本仓不能替架构 Owner 选边；目录的完整内建映射要等契约裁决。
- `assetLibraries`(官方 / 玩家素材库)、光照、网格生成尚未落地。
- Residency 已有 pin、readiness、durability-fenced unload，但完整 streaming loader、候选选择、缓存预算与
  eviction policy 仍是上层/后续工作；不得把本地技术上限伪装成冻结的公共 pin 预算。

## 相关

- 决策:[0013](../../decisions/0013-voxel-world-contract-and-section-rename.md)、
  [0007](../../decisions/0007-v1.4-implementation-baseline.md)(采用已废弃基线的那条)。
- 代码:`crates/lumio-voxel-domain/src/key.rs`、`crates/lumio-voxel-domain/src/section/`、
  `crates/lumio-voxel-contracts/src/voxel_world.rs`、`crates/lumio-voxel-contracts/src/legacy_baseline.rs`。
- 测试:`crates/lumio-voxel-domain/tests/section_chunk_keys.rs`、
  `crates/lumio-voxel-domain/tests/section_block_storage.rs`、
  `crates/lumio-voxel-domain/tests/section_payload_contract.rs`、
  `crates/lumio-voxel-contracts/tests/voxel_world_conformance.rs`。
