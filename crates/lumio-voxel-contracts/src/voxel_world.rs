//! 体素世界公共语义,来源是 `wire/voxel-world-v1.json`(contractId `lumio.voxel-world.v1`)。
//!
//! 这里的每一个值都必须等于那份 JSON 里的对应字段。等式不是靠人眼抄出来的:
//! `tests/voxel_world_conformance.rs` 解析同一份 JSON 并逐条断言,同时校验 JSON 自身的
//! SHA-256 与 `CONTRACT_SHA256` 相符。改契约的正确顺序是:架构仓改 JSON → 复制到本仓
//! `wire/` → 更新 `CONTRACT_SHA256` 与本文件常量 → 一致性测试变绿。
//!
//! **这不是** `generated/` 下那份 `LGE-V1.4-2026-08-27` 镜像。那份镜像的生成源仓已不存在,
//! 且用 `Chunk` 指代 16×16×16 的数据单元——按本契约,那个单元叫 Section。活代码的体素
//! 分层语义只能从这里取。

/// `contractId`。
pub const CONTRACT_ID: &str = "lumio.voxel-world.v1";
/// `version`。
pub const CONTRACT_VERSION: u32 = 1;
/// `wire/voxel-world-v1.json` 的 SHA-256。副本被改动即在一致性测试里失败。
pub const CONTRACT_SHA256: &str =
    "e474907b397627e168acd6dc1ddbba5375a2c5b53f9a8722130dcf6bb7b02869";

// ------------------------------------------------------------------ identity

/// `identity.sectionKey.syntax`。
pub const SECTION_KEY_SYNTAX: &str = "s:<x>:<y>:<z>";
/// `identity.sectionKey.pattern`。本仓不引入正则依赖,解析器手写实现同一语法。
pub const SECTION_KEY_PATTERN: &str =
    "^s:(0|-?[1-9][0-9]{0,9}):(0|1[0-5]|[1-9]):(0|-?[1-9][0-9]{0,9})$";
/// `identity.chunkKey.syntax`。Chunk 键只有两个坐标。
pub const CHUNK_KEY_SYNTAX: &str = "c:<x>:<z>";
/// `identity.chunkKey.pattern`。
pub const CHUNK_KEY_PATTERN: &str = "^c:(0|-?[1-9][0-9]{0,9}):(0|-?[1-9][0-9]{0,9})$";

/// Section 键前缀。
pub const SECTION_KEY_PREFIX: &str = "s";
/// Chunk 键前缀。
pub const CHUNK_KEY_PREFIX: &str = "c";
/// Section 键的坐标个数(x/y/z)。
pub const SECTION_KEY_ARITY: usize = 3;
/// Chunk 键的坐标个数(x/z)——元数差异本身就是防呆。
pub const CHUNK_KEY_ARITY: usize = 2;

/// `identity.*.coordinates.{x,z}.min`。
pub const SECTION_COORD_MIN: i32 = -2147483648;
/// `identity.*.coordinates.{x,z}.max`。
pub const SECTION_COORD_MAX: i32 = 2147483647;

// -------------------------------------------------------------------- layering

/// `layering.levels.Section.carriesData`。
pub const SECTION_CARRIES_DATA: bool = true;
/// `layering.levels.Chunk.carriesData`——Chunk 是容器,不存字节、不持有独立版本。
pub const CHUNK_CARRIES_DATA: bool = false;

// ---------------------------------------------------------------------- limits

/// `limits.sectionExtent`。
pub const SECTION_EXTENT: u32 = 16;
/// `limits.sectionCells`。
pub const SECTION_CELLS: u32 = 4096;
/// `limits.sectionsPerChunk`。
pub const SECTIONS_PER_CHUNK: u32 = 16;
/// `limits.sectionYMin`。
pub const SECTION_Y_MIN: u8 = 0;
/// `limits.sectionYMax`。
pub const SECTION_Y_MAX: u8 = 15;
/// `limits.worldHeightBlocks`——世界高度恰好一个 Chunk 高,所以 Chunk 只有两个坐标。
pub const WORLD_HEIGHT_BLOCKS: u32 = 256;
/// `limits.paletteMaxEntries`。
pub const PALETTE_MAX_ENTRIES: u32 = 256;
/// `limits.paletteIndexBits`。
pub const PALETTE_INDEX_BITS: u32 = 8;
/// `limits.blockTypeMax`。
pub const BLOCK_TYPE_MAX: u32 = 16777215;
/// `limits.blockStateMax`。
pub const BLOCK_STATE_MAX: u32 = 255;
/// `limits.lightBitsPerCell`。
pub const LIGHT_BITS_PER_CELL: u32 = 16;
/// `limits.lightMaxPropagation`。
pub const LIGHT_MAX_PROPAGATION: u32 = 15;
/// `limits.blockTypeScopeBit`——BlockType 最高位是作用域位。
pub const BLOCK_TYPE_SCOPE_BIT: u32 = 23;
/// `limits.blockTypeScopeMask`。
pub const BLOCK_TYPE_SCOPE_MASK: u32 = 8388608;
/// `limits.systemReservedTypeMax`——0~255 由 `blockId.typeSegments` 固定,不进方块目录。
pub const SYSTEM_RESERVED_TYPE_MAX: u32 = 255;
/// `limits.firstOfficialBlockType`。
pub const FIRST_OFFICIAL_BLOCK_TYPE: u32 = 256;
/// `limits.globalSegmentMax`——全局官方段上界(作用域位 = 0)。
pub const GLOBAL_SEGMENT_MAX: u32 = 8388607;
/// `limits.roomLocalSegmentMin`——房间局部段下界(作用域位 = 1)。
pub const ROOM_LOCAL_SEGMENT_MIN: u32 = 8388608;
/// `limits.worldYMin`——世界 Y 是无符号的,底为 0。
pub const WORLD_Y_MIN: u32 = 0;
/// `limits.worldYMax`。
pub const WORLD_Y_MAX: u32 = 255;
/// `limits.maxCellsPerReadRequest`。
pub const MAX_CELLS_PER_READ_REQUEST: u32 = 262144;
/// `limits.maxEntriesPerWriteBatch`。
pub const MAX_ENTRIES_PER_WRITE_BATCH: u32 = 65536;
/// `limits.firstCatalogBlockType`——方块目录的首个可分配编号。
pub const FIRST_CATALOG_BLOCK_TYPE: u32 = 256;

// ------------------------------------------------------------ identity.cellOffset

/// `identity.cellOffset.strides.y`。
pub const CELL_OFFSET_Y_STRIDE: u16 = 256;
/// `identity.cellOffset.strides.z`。
pub const CELL_OFFSET_Z_STRIDE: u16 = 16;
/// `identity.cellOffset.strides.x`。
pub const CELL_OFFSET_X_STRIDE: u16 = 1;
/// `identity.cellOffset.range` 的下界。
pub const CELL_OFFSET_MIN: u16 = 0;
/// `identity.cellOffset.range` 的上界。
pub const CELL_OFFSET_MAX: u16 = 4095;

// --------------------------------------------------------------------- blockId

/// `blockId.width`。
pub const BLOCK_ID_WIDTH: u32 = 32;
/// `blockId.fields.BlockType.bits`。
pub const BLOCK_TYPE_BITS: u32 = 24;
/// `blockId.fields.BlockType.shift`。
pub const BLOCK_TYPE_SHIFT: u32 = 8;
/// `blockId.fields.BlockState.bits`。
pub const BLOCK_STATE_BITS: u32 = 8;
/// `blockId.fields.BlockState.shift`。
pub const BLOCK_STATE_SHIFT: u32 = 0;

// ---------------------------------------------------------- 枚举(顺序即契约顺序)

/// `diffDispatch.presence` 的四态,文档顺序。Pending / Unavailable 永不等价于空气。
pub static SECTION_PRESENCE: &[&str] = &["Ready", "Unchanged", "Pending", "Unavailable"];

/// `diffDispatch.shortTicket.payloadLength`——Unchanged 必须是零字节短票。
pub const SHORT_TICKET_PAYLOAD_LENGTH: u32 = 0;

/// `sectionPayload.encodings.Delta.bytesPerEntry`.
pub const DELTA_BYTES_PER_ENTRY: u32 = 6;

/// `sectionPayload.encodings` 的四种编码,文档顺序。
///
/// `Delta` 是本次契约扩张新增的一档:它相对一个基线 revision 表达,因此只能用于
/// **非首次**送达(`payload.delta-not-for-first-delivery`),且基线必须对得上
/// (`payload.delta-needs-matching-base`)。本仓尚未实现该档,常量先与契约对齐。
pub static SECTION_PAYLOAD_ENCODINGS: &[&str] = &["Uniform", "Palette", "Raw", "Delta"];

/// `sectionPayload.envelope.required`。
pub static SECTION_PAYLOAD_ENVELOPE_FIELDS: &[&str] = &[
    "sectionKey",
    "sectionRevision",
    "encoding",
    "payloadLength",
    "payloadSha256",
];

/// `materialClasses.classes` 的 v1 材质类,文档顺序。
pub static MATERIAL_CLASSES: &[&str] = &["Solid", "Liquid"];

/// `behaviorTemplates.v1` 的穷尽模板表,文档顺序。
pub static BEHAVIOR_TEMPLATES_V1: &[&str] = &["FullCube", "Liquid"];

// ------------------------------------------------------------------ errorCodes

/// `errorCodes`,文档顺序。这是体素公共语义的稳定错误标识表。
pub static VOXEL_WORLD_ERROR_CODES: &[&str] = &[
    "unknown_section_key",
    "unknown_chunk_key",
    "section_y_out_of_range",
    "coordinate_out_of_bounds",
    "section_unavailable",
    "stale_section_revision",
    "palette_overflow",
    "section_encoding_mismatch",
    "section_digest_mismatch",
    "dirty_section_not_durable",
    "lighting_in_payload",
    "chunk_carries_data",
    "unknown_material_class",
    "material_class_not_a_cell_lane",
    "liquid_auto_propagation_unsupported",
    "cross_material_face_merge",
    // 方块↔实体绑定(契约 `blockEntityBinding`)。本仓尚未实现该面,先按契约登记错误码,
    // 使表与契约逐条对齐;实现落在后续任务卡。
    "entity_binding_missing",
    "entity_binding_orphan",
    "entity_binding_type_mismatch",
    "entity_binding_not_sparse",
    "business_data_in_payload",
    "binding_commit_split",
    // 以下各面由本次契约扩张引入,本仓均**尚未实现**;错误码先按契约登记,使表与契约
    // 逐条对齐(`contract_error_codes_match` 断言全表同名同序),实现落在后续任务卡。
    //
    // BlockType 作用域分段(契约 `blockId.scope` / `blockCatalog` / `assetLibraries`)。
    "block_type_scope_violation",
    "system_reserved_type_misuse",
    "room_local_type_without_mapping",
    "player_type_declares_behavior",
    // 调色板槽位回收与 Delta 载荷(契约 `sectionPayload`)。
    "palette_reclaim_before_escalation",
    "dead_palette_entry_in_payload",
    "delta_base_revision_mismatch",
    "delta_used_for_first_delivery",
    // 物理查询(契约 `physicsQuery`)。
    "unresolved_hit_treated_as_air",
    "unresolved_hit_treated_as_solid",
    "query_buffer_overflow",
    "query_result_divergence",
    "collision_behavior_not_from_material_table",
    "query_mutates_world",
    "world_y_out_of_range",
    // 官方方块目录(契约 `blockCatalog`)。
    "block_catalog_not_dense",
    "block_catalog_name_reused",
    "block_catalog_row_incomplete",
    // 方块读写 API(契约 `blockRead` / `blockWrite`)。
    "read_budget_exceeded",
    "read_result_missing_revision",
    "write_batch_too_large",
    "unstructured_mutation_entry",
    // 固定 cellOffset 算式、区域常驻、行为模板与单格读取闭合。
    "cell_offset_out_of_range",
    "residency_pin_exceeds_budget",
    "pin_region_not_ready",
    "pinned_section_evicted",
    "pinned_read_returned_pending",
    "unknown_behavior_template",
    "cell_read_missing_presence",
];

/// 键不是合法 Section 键(前缀 / 元数 / 规范写法任一不合)。
pub const UNKNOWN_SECTION_KEY: &str = "unknown_section_key";
/// 键不是合法 Chunk 键——三坐标的 `c:` 键在语法上即非法。
pub const UNKNOWN_CHUNK_KEY: &str = "unknown_chunk_key";
/// Section 层号越出 0~15。
pub const SECTION_Y_OUT_OF_RANGE: &str = "section_y_out_of_range";
/// x / z 越出 int32 定义域。
pub const COORDINATE_OUT_OF_BOUNDS: &str = "coordinate_out_of_bounds";
/// Section 当前不可提供;缺块永不等于空气。
pub const SECTION_UNAVAILABLE: &str = "section_unavailable";
/// 回执覆盖上界不足以清除该 Section 的脏标记。
pub const STALE_SECTION_REVISION: &str = "stale_section_revision";
/// 调色板项数超过 256。
pub const PALETTE_OVERFLOW: &str = "palette_overflow";
/// Section 载荷编码与实际内容不一致。
pub const SECTION_ENCODING_MISMATCH: &str = "section_encoding_mismatch";
/// Section 载荷摘要校验失败(必须先于任何解释)。
pub const SECTION_DIGEST_MISMATCH: &str = "section_digest_mismatch";
/// 未被回执覆盖的脏 Section 被要求卸载。
pub const DIRTY_SECTION_NOT_DURABLE: &str = "dirty_section_not_durable";
/// 载荷或回执里出现光照。
pub const LIGHTING_IN_PAYLOAD: &str = "lighting_in_payload";
/// Chunk 记录携带了数据字节或自己的 revision。
pub const CHUNK_CARRIES_DATA_ERROR: &str = "chunk_carries_data";
/// Palette escalation skipped the required live-slot reclamation scan.
pub const PALETTE_RECLAIM_BEFORE_ESCALATION: &str = "palette_reclaim_before_escalation";
/// A serialized Palette carries an entry that no cell references.
pub const DEAD_PALETTE_ENTRY_IN_PAYLOAD: &str = "dead_palette_entry_in_payload";
/// A Delta payload does not match the receiver's current Section revision.
pub const DELTA_BASE_REVISION_MISMATCH: &str = "delta_base_revision_mismatch";
/// A Delta payload was offered without a receiver-side Section baseline.
pub const DELTA_USED_FOR_FIRST_DELIVERY: &str = "delta_used_for_first_delivery";
/// 目录中的材质类不在 `materialClasses.classes` 表中。
pub const UNKNOWN_MATERIAL_CLASS: &str = "unknown_material_class";
/// 材质类被错误地存成逐格数据通道。
pub const MATERIAL_CLASS_NOT_A_CELL_LANE: &str = "material_class_not_a_cell_lane";
/// v1 液体是静态的,体素系统不自动传播。
pub const LIQUID_AUTO_PROPAGATION_UNSUPPORTED: &str = "liquid_auto_propagation_unsupported";
/// 贪心合面跨越了不同材质类。
pub const CROSS_MATERIAL_FACE_MERGE: &str = "cross_material_face_merge";
/// BlockType 越过了作用域位划定的段边界。
pub const BLOCK_TYPE_SCOPE_VIOLATION: &str = "block_type_scope_violation";
/// 普通官方方块占用了 0~255 系统保留段。
pub const SYSTEM_RESERVED_TYPE_MISUSE: &str = "system_reserved_type_misuse";
/// 房间局部 BlockType 缺少随存档保存的映射表。
pub const ROOM_LOCAL_TYPE_WITHOUT_MAPPING: &str = "room_local_type_without_mapping";
/// 房间局部方块声明了自定义行为,而非选择登记模板。
pub const PLAYER_TYPE_DECLARES_BEHAVIOR: &str = "player_type_declares_behavior";
/// 世界 Y 越出 0~255。
pub const WORLD_Y_OUT_OF_RANGE: &str = "world_y_out_of_range";
/// 官方目录的 BlockType 不是从 256 起连续稠密分配。
pub const BLOCK_CATALOG_NOT_DENSE: &str = "block_catalog_not_dense";
/// 官方目录复用了现存或已下线的稳定名称。
pub const BLOCK_CATALOG_NAME_REUSED: &str = "block_catalog_name_reused";
/// 官方目录行的六个必需字段没有填齐。
pub const BLOCK_CATALOG_ROW_INCOMPLETE: &str = "block_catalog_row_incomplete";
/// 格内偏移越出 0~4095 或不符合唯一坐标算式。
pub const CELL_OFFSET_OUT_OF_RANGE: &str = "cell_offset_out_of_range";
/// 目录引用了 v1 登记表之外的行为模板。
pub const UNKNOWN_BEHAVIOR_TEMPLATE: &str = "unknown_behavior_template";

// ------------------------------------------------------------------- blockId 段

/// `blockId.scope` 判定:作用域位为 0 即全局官方段。
pub fn is_global_segment(block_type: u32) -> bool {
    block_type & BLOCK_TYPE_SCOPE_MASK == 0
}

/// `blockId.scope` 判定:作用域位为 1 即房间局部段。
pub fn is_room_local_segment(block_type: u32) -> bool {
    block_type & BLOCK_TYPE_SCOPE_MASK != 0
}

/// `blockId.scope.roomLocal.localIndex`——局部号 = `BlockType & 0x7FFFFF`。
pub fn room_local_index(block_type: u32) -> u32 {
    block_type & !BLOCK_TYPE_SCOPE_MASK
}

/// 是否是契约声明的错误码。
pub fn is_error_code(id: &str) -> bool {
    VOXEL_WORLD_ERROR_CODES.contains(&id)
}

/// 把错误码收敛到表内的那一份 `'static` 实例;不在表里就是 `None`。
///
/// 调用方拿到的引用与表项同址,`std::ptr::eq` 可用来证明 id 确实来自契约表。
pub fn intern_error_code(id: &str) -> Option<&'static str> {
    VOXEL_WORLD_ERROR_CODES
        .iter()
        .copied()
        .find(|candidate| *candidate == id)
}

/// 把 presence 名字收敛到 `SECTION_PRESENCE` 里的那一份 `'static` 实例。
pub fn intern_presence(name: &str) -> Option<&'static str> {
    SECTION_PRESENCE
        .iter()
        .copied()
        .find(|candidate| *candidate == name)
}
