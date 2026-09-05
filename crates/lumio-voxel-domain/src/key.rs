//! Section 与 Chunk 的规范键(契约 `lumio.voxel-world.v1` 的 `identity` 段)。
//!
//! 三层里只有两层有键:
//!
//! * [`SectionId`] —— 16×16×16 的**数据单元**,键 `s:<x>:<y>:<z>`,y 是它在所属 Chunk
//!   内的层号 0~15。改动层、版本集、回执覆盖集都以它为键。
//! * [`ChunkId`] —— 16 个 Section 摞成的**列容器**,键 `c:<x>:<z>`。世界高度恰好一个
//!   Chunk 高,所以垂直方向只有一层,Chunk 只需两个坐标。
//!
//! Chunk 在本仓没有自己的数据模块,这正是契约红线 `layering.chunk-carries-no-data` 的
//! 结构表达:它不携带字节、不持有独立 revision,只是键 + 与 Section 的互推。
//!
//! **元数即防呆**:Chunk 键两坐标、Section 键三坐标且前缀不同,所以旧式三坐标 `c:x:y:z`
//! 在新语法下语法非法。它必须显式失败([`KeyRejection::LegacyThreeCoordinateChunkKey`]),
//! 不得被解读成 `c:x:z`,也不得被解读成 `s:x:y:z`——错层引用只能显式失败,不能静默通过。

#![forbid(unsafe_code)]

use lumio_voxel_contracts::voxel_world as vw;

/// 被解析的是哪一种键。决定拒绝时报 `unknown_section_key` 还是 `unknown_chunk_key`。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyKind {
    Section,
    Chunk,
}

/// 拒绝的**具体理由**。存在的意义是让「显式拒绝」可被断言:调用方能证明某个键是被某条
/// 契约规则挡下的,而不是碰巧解析失败。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyRejection {
    /// 旧式三坐标 `c:x:y:z`。契约 `identity.arityIsTheGuard`:语法即非法。
    LegacyThreeCoordinateChunkKey,
    /// 前缀不是本键种类要求的 `s:` / `c:`。
    WrongPrefix,
    /// 坐标个数不对(且不是上面那条旧式键)。
    WrongArity,
    /// 空的坐标段。
    EmptyComponent,
    /// 前导零、`-0`、非十进制写法。契约 `key.canonical`。
    NonCanonicalCoordinate,
    /// 层号越出 0~15。契约 `key.section.y-range`。
    SectionYOutOfRange,
    /// x / z 越出 int32 定义域。契约 `key.coordinate-bounds`。
    CoordinateOutOfBounds,
}

/// 键解析失败。`error_id` 一律取自契约 `errorCodes`,不是本仓自造的字符串。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyError {
    kind: KeyKind,
    rejection: KeyRejection,
}

impl KeyError {
    fn new(kind: KeyKind, rejection: KeyRejection) -> Self {
        Self { kind, rejection }
    }

    pub fn kind(&self) -> KeyKind {
        self.kind
    }

    pub fn rejection(&self) -> KeyRejection {
        self.rejection
    }

    /// 契约错误码。y 越界与坐标越界有各自的码,其余归到「这不是一个合法的 X 键」。
    ///
    /// 返回的是契约表里的那一份 `'static` 实例(经 `intern_error_code`),不是本 crate
    /// 内联的一份字面量副本——`const &str` 会在每个使用点各自物化,`std::ptr::eq` 就认不出
    /// 同一个标识符了。
    pub fn error_id(&self) -> &'static str {
        let code = match (self.rejection, self.kind) {
            (KeyRejection::SectionYOutOfRange, _) => vw::SECTION_Y_OUT_OF_RANGE,
            (KeyRejection::CoordinateOutOfBounds, _) => vw::COORDINATE_OUT_OF_BOUNDS,
            (_, KeyKind::Section) => vw::UNKNOWN_SECTION_KEY,
            (_, KeyKind::Chunk) => vw::UNKNOWN_CHUNK_KEY,
        };
        vw::intern_error_code(code).expect("key error ids are declared in the contract table")
    }
}

impl std::fmt::Display for KeyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.error_id())
    }
}

impl std::error::Error for KeyError {}

/// 世界纵坐标。契约只允许 0~255,构造后即可无失败地拆成 Section 层号与格内 y。
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WorldY(u8);

impl WorldY {
    pub fn new(value: i64) -> Result<Self, WorldYError> {
        if value < i64::from(vw::WORLD_Y_MIN) || value > i64::from(vw::WORLD_Y_MAX) {
            return Err(WorldYError);
        }
        Ok(Self(value as u8))
    }

    pub fn value(self) -> u8 {
        self.0
    }

    /// `sectionY = worldY >> 4`。
    pub fn section_y(self) -> u8 {
        self.0 >> vw::SECTION_EXTENT.trailing_zeros()
    }

    /// `cellY = worldY & 15`。
    pub fn cell_y(self) -> u8 {
        self.0 & vw::SECTION_Y_MAX
    }
}

/// 世界 y 越出契约定义域。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorldYError;

impl WorldYError {
    pub fn error_id(&self) -> &'static str {
        vw::intern_error_code(vw::WORLD_Y_OUT_OF_RANGE)
            .expect("world-y error id is declared in the contract table")
    }
}

impl std::fmt::Display for WorldYError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.error_id())
    }
}

impl std::error::Error for WorldYError {}

/// 16×16×16 数据单元的规范键 `s:<x>:<y>:<z>`。
///
/// 排序是 (x, y, z) 字典序,`BTreeMap<SectionId, _>` 因此有确定的迭代顺序。
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SectionId {
    x: i32,
    y: u8,
    z: i32,
}

impl SectionId {
    /// 按坐标构造。y 必须落在 0~15。
    pub fn new(x: i32, y: i64, z: i32) -> Result<Self, KeyError> {
        let y = check_section_y(y, KeyKind::Section)?;
        Ok(Self { x, y, z })
    }

    /// 解析规范键。任何非规范写法都失败,不做容错修补。
    pub fn parse(raw: &str) -> Result<Self, KeyError> {
        let parts = split_key(raw, vw::SECTION_KEY_PREFIX, KeyKind::Section)?;
        if parts.len() != vw::SECTION_KEY_ARITY {
            return Err(KeyError::new(KeyKind::Section, KeyRejection::WrongArity));
        }
        let x = parse_i32(parts[0], KeyKind::Section)?;
        let y = check_section_y(parse_decimal(parts[1], KeyKind::Section)?, KeyKind::Section)?;
        let z = parse_i32(parts[2], KeyKind::Section)?;
        Ok(Self { x, y, z })
    }

    pub fn x(&self) -> i32 {
        self.x
    }

    /// 在所属 Chunk 内的层号,0~15。
    pub fn y(&self) -> u8 {
        self.y
    }

    pub fn z(&self) -> i32 {
        self.z
    }

    /// 规范键文本。
    pub fn key(&self) -> String {
        format!("s:{}:{}:{}", self.x, self.y, self.z)
    }

    /// 所属 Chunk:丢掉 y。契约 `identity.derivation.sectionToChunk`。
    pub fn chunk(&self) -> ChunkId {
        ChunkId {
            x: self.x,
            z: self.z,
        }
    }
}

impl std::fmt::Display for SectionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "s:{}:{}:{}", self.x, self.y, self.z)
    }
}

/// 16 个 Section 摞成的列容器的规范键 `c:<x>:<z>`。
///
/// **只有两个坐标,没有别的字段**:Chunk 不携带数据字节,也不持有独立 revision
/// (契约 `layering.chunk-carries-no-data`)。它是存档打包与按列计算的容器,
/// 全部内容都能从 x/z 推出来。
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ChunkId {
    x: i32,
    z: i32,
}

impl ChunkId {
    pub fn new(x: i32, z: i32) -> Self {
        Self { x, z }
    }

    /// 解析规范键。三坐标的 `c:` 键在这里以元数守卫显式失败。
    pub fn parse(raw: &str) -> Result<Self, KeyError> {
        let parts = split_key(raw, vw::CHUNK_KEY_PREFIX, KeyKind::Chunk)?;
        if parts.len() != vw::CHUNK_KEY_ARITY {
            return Err(KeyError::new(KeyKind::Chunk, arity_rejection(parts.len())));
        }
        let x = parse_i32(parts[0], KeyKind::Chunk)?;
        let z = parse_i32(parts[1], KeyKind::Chunk)?;
        Ok(Self { x, z })
    }

    pub fn x(&self) -> i32 {
        self.x
    }

    pub fn z(&self) -> i32 {
        self.z
    }

    /// 规范键文本。
    pub fn key(&self) -> String {
        format!("c:{}:{}", self.x, self.z)
    }

    /// 含且仅含 `s:x:0:z … s:x:15:z` 共 16 个 Section,按层号升序。
    /// 契约 `identity.derivation.chunkToSections`。
    pub fn sections(&self) -> impl Iterator<Item = SectionId> + use<> {
        let (x, z) = (self.x, self.z);
        (vw::SECTION_Y_MIN..=vw::SECTION_Y_MAX).map(move |y| SectionId { x, y, z })
    }
}

impl std::fmt::Display for ChunkId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "c:{}:{}", self.x, self.z)
    }
}

/// 元数不对时的理由:恰好三段的 `c:` 键是那条旧式键,别的只是元数错。
fn arity_rejection(found: usize) -> KeyRejection {
    if found == vw::SECTION_KEY_ARITY {
        KeyRejection::LegacyThreeCoordinateChunkKey
    } else {
        KeyRejection::WrongArity
    }
}

/// 切前缀与坐标段。前缀不符即失败——但三坐标的 `c:` 键当 Section 键用时,报的是元数守卫
/// 而不是「前缀不对」,因为契约要的是「这是那条已废弃的旧式键」这个明确结论。
fn split_key<'a>(raw: &'a str, prefix: &str, kind: KeyKind) -> Result<Vec<&'a str>, KeyError> {
    let mut parts = raw.split(':');
    let found_prefix = parts.next().unwrap_or_default();
    let components: Vec<&str> = parts.collect();
    if found_prefix != prefix {
        if kind == KeyKind::Section
            && found_prefix == vw::CHUNK_KEY_PREFIX
            && components.len() == vw::SECTION_KEY_ARITY
        {
            return Err(KeyError::new(
                kind,
                KeyRejection::LegacyThreeCoordinateChunkKey,
            ));
        }
        return Err(KeyError::new(kind, KeyRejection::WrongPrefix));
    }
    Ok(components)
}

/// 规范十进制:非空、无前导零、不得 `-0`、只有 ASCII 数字与可选负号。
/// 值域检查由调用方按坐标轴分别做。
fn parse_decimal(raw: &str, kind: KeyKind) -> Result<i64, KeyError> {
    if raw.is_empty() {
        return Err(KeyError::new(kind, KeyRejection::EmptyComponent));
    }
    let negative = raw.starts_with('-');
    let digits = if negative { &raw[1..] } else { raw };
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return Err(KeyError::new(kind, KeyRejection::NonCanonicalCoordinate));
    }
    if digits.len() > 1 && digits.as_bytes()[0] == b'0' {
        return Err(KeyError::new(kind, KeyRejection::NonCanonicalCoordinate));
    }
    if negative && digits == "0" {
        return Err(KeyError::new(kind, KeyRejection::NonCanonicalCoordinate));
    }
    // 位数超过 i64 也是越界,不是写法问题。
    raw.parse::<i64>()
        .map_err(|_| KeyError::new(kind, KeyRejection::CoordinateOutOfBounds))
}

/// x / z:规范十进制且落在 int32 定义域。
fn parse_i32(raw: &str, kind: KeyKind) -> Result<i32, KeyError> {
    let digits = raw.strip_prefix('-').unwrap_or(raw);
    let max_digits = vw::SECTION_COORD_MIN.unsigned_abs().ilog10() as usize + 1;
    if digits.len() > max_digits {
        return Err(KeyError::new(kind, KeyRejection::NonCanonicalCoordinate));
    }
    let value = parse_decimal(raw, kind)?;
    if value < i64::from(vw::SECTION_COORD_MIN) || value > i64::from(vw::SECTION_COORD_MAX) {
        return Err(KeyError::new(kind, KeyRejection::CoordinateOutOfBounds));
    }
    Ok(value as i32)
}

/// y:层号必须落在 0~15。
fn check_section_y(value: i64, kind: KeyKind) -> Result<u8, KeyError> {
    if value < i64::from(vw::SECTION_Y_MIN) || value > i64::from(vw::SECTION_Y_MAX) {
        return Err(KeyError::new(kind, KeyRejection::SectionYOutOfRange));
    }
    Ok(value as u8)
}
