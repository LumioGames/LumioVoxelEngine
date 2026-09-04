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
    "4fd903f424702f8ddab6a9781492ac0b25a8c61ff585749ceb39369d3afb5d43";

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

/// `sectionPage.encodings` 的三种编码,文档顺序。
pub static SECTION_PAGE_ENCODINGS: &[&str] = &["Uniform", "Palette", "Raw"];

/// `sectionPage.envelope.required`。
pub static SECTION_PAGE_ENVELOPE_FIELDS: &[&str] = &[
    "sectionKey",
    "sectionRevision",
    "encoding",
    "payloadLength",
    "payloadSha256",
];

/// `materialClasses.classes` 的 v1 材质类,文档顺序。
pub static MATERIAL_CLASSES: &[&str] = &["Solid", "Liquid"];

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
    "page_encoding_mismatch",
    "page_digest_mismatch",
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
/// 页编码与实际内容不一致。
pub const PAGE_ENCODING_MISMATCH: &str = "page_encoding_mismatch";
/// 页载荷摘要校验失败(必须先于任何解释)。
pub const PAGE_DIGEST_MISMATCH: &str = "page_digest_mismatch";
/// 未被回执覆盖的脏 Section 被要求卸载。
pub const DIRTY_SECTION_NOT_DURABLE: &str = "dirty_section_not_durable";
/// 载荷或回执里出现光照。
pub const LIGHTING_IN_PAYLOAD: &str = "lighting_in_payload";
/// Chunk 记录携带了数据字节或自己的 revision。
pub const CHUNK_CARRIES_DATA_ERROR: &str = "chunk_carries_data";

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
