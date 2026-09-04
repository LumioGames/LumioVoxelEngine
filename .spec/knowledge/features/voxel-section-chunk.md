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

- 契约 `blockEntityBinding`(方块↔实体的 Section 级稀疏引用表)只登记了错误码,未实现。
- 契约 `sectionPayload` 的四档编码(Uniform / Palette / Raw / Delta)只登记了枚举与上限;当前载荷仍是
  死基线的 `Dense` / `None` 适配器。`Delta` 相对基线 revision 表达,只能用于非首次送达。
- 契约扩张引入、本仓只登记了错误码与常量而**未实现**的面:`blockCatalog`(官方方块目录与铸号规程)、
  `assetLibraries`(官方 / 玩家素材库)、`blockId.scope`(BlockType 第 23 位划分全局段与房间局部段)、
  `blockRead` / `blockWrite`(读写预算与批量语义)、`physicsQuery`(未决命中既不算空气也不算实心)。
  段判定已有 `voxel_world::{is_global_segment, is_room_local_segment, room_local_index}` 三个纯函数
  并受一致性测试覆盖,其余只是常量。
- 材质类、光照、网格生成、物理 DDA、流式加载都还没落地。

## 相关

- 决策:[0013](../../decisions/0013-voxel-world-contract-and-section-rename.md)、
  [0007](../../decisions/0007-v1.4-implementation-baseline.md)(采用已废弃基线的那条)。
- 代码:`crates/lumio-voxel-domain/src/key.rs`、`crates/lumio-voxel-domain/src/section/`、
  `crates/lumio-voxel-contracts/src/voxel_world.rs`、`crates/lumio-voxel-contracts/src/legacy_baseline.rs`。
- 测试:`crates/lumio-voxel-domain/tests/section_chunk_keys.rs`、
  `crates/lumio-voxel-contracts/tests/voxel_world_conformance.rs`。
