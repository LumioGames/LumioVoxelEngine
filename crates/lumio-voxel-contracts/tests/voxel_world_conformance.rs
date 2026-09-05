//! 契约一致性:本仓 `voxel_world` 常量必须逐条等于 `wire/voxel-world-v1.json` 里的值。
//!
//! 这份测试是「本仓的值与契约一致」的机器证明——不是人眼抄。任何一条常量与 JSON 漂移,
//! 或 JSON 本身被改动(摘要不符),都在这里失败。JSON 与架构仓 `engine/wire/` 的字节
//! 一致性由 `vendored_copy_matches_upstream_when_available` 覆盖。

use lumio_voxel_contracts::sha256;
use lumio_voxel_contracts::voxel_world as vw;
use std::collections::BTreeMap;
use std::path::PathBuf;

// ---------------------------------------------------------------- 极简 JSON 读取
// 本仓零第三方依赖(工作区没有任何 external crate),所以校验侧自带一个只读解析器。
// 只支持契约文件用到的形状:对象 / 数组 / 字符串 / 无符号与负整数 / bool。

#[derive(Clone, Debug, PartialEq)]
enum Json {
    Str(String),
    Int(i64),
    Bool(bool),
    Arr(Vec<Json>),
    /// 保留文档顺序的对象:契约里 presence / encodings 的枚举顺序是有意义的。
    Obj(Vec<(String, Json)>),
}

impl Json {
    fn get(&self, key: &str) -> &Json {
        match self {
            Json::Obj(members) => members
                .iter()
                .find(|(k, _)| k == key)
                .map(|(_, v)| v)
                .unwrap_or_else(|| panic!("契约缺少成员 {key}")),
            other => panic!("{other:?} 不是对象,取不到 {key}"),
        }
    }

    fn keys(&self) -> Vec<&str> {
        match self {
            Json::Obj(members) => members.iter().map(|(k, _)| k.as_str()).collect(),
            other => panic!("{other:?} 不是对象"),
        }
    }

    fn as_str(&self) -> &str {
        match self {
            Json::Str(s) => s,
            other => panic!("{other:?} 不是字符串"),
        }
    }

    fn as_int(&self) -> i64 {
        match self {
            Json::Int(v) => *v,
            other => panic!("{other:?} 不是整数"),
        }
    }

    fn as_arr(&self) -> &[Json] {
        match self {
            Json::Arr(items) => items,
            other => panic!("{other:?} 不是数组"),
        }
    }

    fn str_list(&self) -> Vec<&str> {
        self.as_arr().iter().map(Json::as_str).collect()
    }
}

struct Parser<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Parser<'a> {
    fn new(text: &'a str) -> Self {
        Self {
            bytes: text.as_bytes(),
            pos: 0,
        }
    }

    fn skip_ws(&mut self) {
        while self.pos < self.bytes.len() && self.bytes[self.pos].is_ascii_whitespace() {
            self.pos += 1;
        }
    }

    fn eat(&mut self, ch: u8) {
        self.skip_ws();
        assert_eq!(self.bytes[self.pos], ch, "在偏移 {} 期待 {ch:?}", self.pos);
        self.pos += 1;
    }

    fn peek(&mut self) -> u8 {
        self.skip_ws();
        self.bytes[self.pos]
    }

    fn value(&mut self) -> Json {
        match self.peek() {
            b'{' => self.object(),
            b'[' => self.array(),
            b'"' => Json::Str(self.string()),
            b't' => {
                self.pos += 4;
                Json::Bool(true)
            }
            b'f' => {
                self.pos += 5;
                Json::Bool(false)
            }
            _ => self.number(),
        }
    }

    fn object(&mut self) -> Json {
        self.eat(b'{');
        let mut members = Vec::new();
        if self.peek() == b'}' {
            self.pos += 1;
            return Json::Obj(members);
        }
        loop {
            self.skip_ws();
            let key = self.string();
            self.eat(b':');
            let value = self.value();
            members.push((key, value));
            match self.peek() {
                b',' => self.pos += 1,
                b'}' => {
                    self.pos += 1;
                    break;
                }
                other => panic!("对象里出现意外字节 {other:?}"),
            }
        }
        Json::Obj(members)
    }

    fn array(&mut self) -> Json {
        self.eat(b'[');
        let mut items = Vec::new();
        if self.peek() == b']' {
            self.pos += 1;
            return Json::Arr(items);
        }
        loop {
            items.push(self.value());
            match self.peek() {
                b',' => self.pos += 1,
                b']' => {
                    self.pos += 1;
                    break;
                }
                other => panic!("数组里出现意外字节 {other:?}"),
            }
        }
        Json::Arr(items)
    }

    fn string(&mut self) -> String {
        self.eat(b'"');
        let mut out = String::new();
        loop {
            let byte = self.bytes[self.pos];
            match byte {
                b'"' => {
                    self.pos += 1;
                    return out;
                }
                b'\\' => {
                    self.pos += 1;
                    let esc = self.bytes[self.pos];
                    self.pos += 1;
                    match esc {
                        b'n' => out.push('\n'),
                        b't' => out.push('\t'),
                        b'r' => out.push('\r'),
                        b'b' => out.push('\u{8}'),
                        b'f' => out.push('\u{c}'),
                        b'u' => {
                            let hex = std::str::from_utf8(&self.bytes[self.pos..self.pos + 4])
                                .expect("\\u 后必须是 4 位十六进制");
                            self.pos += 4;
                            let code = u32::from_str_radix(hex, 16).expect("非法 \\u 转义");
                            out.push(char::from_u32(code).expect("非法码位"));
                        }
                        other => out.push(other as char),
                    }
                }
                _ => {
                    // 多字节 UTF-8 逐字节搬运:字符串内容原样保留。
                    let start = self.pos;
                    while self.pos < self.bytes.len()
                        && self.bytes[self.pos] != b'"'
                        && self.bytes[self.pos] != b'\\'
                    {
                        self.pos += 1;
                    }
                    out.push_str(
                        std::str::from_utf8(&self.bytes[start..self.pos]).expect("非法 UTF-8"),
                    );
                }
            }
        }
    }

    fn number(&mut self) -> Json {
        self.skip_ws();
        let start = self.pos;
        if self.bytes[self.pos] == b'-' {
            self.pos += 1;
        }
        while self.pos < self.bytes.len() && self.bytes[self.pos].is_ascii_digit() {
            self.pos += 1;
        }
        let text = std::str::from_utf8(&self.bytes[start..self.pos]).expect("非法数字");
        Json::Int(text.parse().expect("契约里只出现整数"))
    }
}

fn wire_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("wire/voxel-world-v1.json")
}

fn contract() -> Json {
    let text = std::fs::read_to_string(wire_path()).expect("契约文件必须随仓提交");
    let mut parser = Parser::new(&text);
    parser.value()
}

fn hex32(bytes: [u8; 32]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

// ---------------------------------------------------------------- 一致性断言

#[test]
fn vendored_contract_bytes_match_recorded_digest() {
    let bytes = std::fs::read(wire_path()).expect("契约文件必须随仓提交");
    assert_eq!(
        hex32(sha256(&bytes)),
        vw::CONTRACT_SHA256,
        "wire/voxel-world-v1.json 与 voxel_world::CONTRACT_SHA256 不符——契约副本被改过"
    );
}

#[test]
fn contract_identity_matches() {
    let c = contract();
    assert_eq!(c.get("contractId").as_str(), vw::CONTRACT_ID);
    assert_eq!(c.get("version").as_int(), i64::from(vw::CONTRACT_VERSION));

    let section = c.get("identity").get("sectionKey");
    assert_eq!(section.get("syntax").as_str(), vw::SECTION_KEY_SYNTAX);
    assert_eq!(section.get("pattern").as_str(), vw::SECTION_KEY_PATTERN);
    let sy = section.get("coordinates").get("y");
    assert_eq!(sy.get("min").as_int(), i64::from(vw::SECTION_Y_MIN));
    assert_eq!(sy.get("max").as_int(), i64::from(vw::SECTION_Y_MAX));
    for axis in ["x", "z"] {
        let coord = section.get("coordinates").get(axis);
        assert_eq!(coord.get("min").as_int(), i64::from(vw::SECTION_COORD_MIN));
        assert_eq!(coord.get("max").as_int(), i64::from(vw::SECTION_COORD_MAX));
    }

    let chunk = c.get("identity").get("chunkKey");
    assert_eq!(chunk.get("syntax").as_str(), vw::CHUNK_KEY_SYNTAX);
    assert_eq!(chunk.get("pattern").as_str(), vw::CHUNK_KEY_PATTERN);
    assert_eq!(
        chunk.get("coordinates").keys(),
        vec!["x", "z"],
        "Chunk 键只有两个坐标"
    );
    for axis in ["x", "z"] {
        let coord = chunk.get("coordinates").get(axis);
        assert_eq!(coord.get("min").as_int(), i64::from(vw::SECTION_COORD_MIN));
        assert_eq!(coord.get("max").as_int(), i64::from(vw::SECTION_COORD_MAX));
    }
}

#[test]
fn contract_cell_offset_layout_matches() {
    let c = contract();
    let offset = c.get("identity").get("cellOffset");
    let strides = offset.get("strides");
    assert_eq!(
        strides.get("y").as_int(),
        i64::from(vw::CELL_OFFSET_Y_STRIDE)
    );
    assert_eq!(
        strides.get("z").as_int(),
        i64::from(vw::CELL_OFFSET_Z_STRIDE)
    );
    assert_eq!(
        strides.get("x").as_int(),
        i64::from(vw::CELL_OFFSET_X_STRIDE)
    );
    assert_eq!(vw::CELL_OFFSET_MIN, 0);
    assert_eq!(vw::CELL_OFFSET_MAX, vw::SECTION_CELLS as u16 - 1);
}

#[test]
fn contract_layering_matches() {
    let c = contract();
    let levels = c.get("layering").get("levels");

    let section = levels.get("Section");
    assert_eq!(
        section
            .get("extent")
            .as_arr()
            .iter()
            .map(Json::as_int)
            .collect::<Vec<_>>(),
        vec![i64::from(vw::SECTION_EXTENT); 3]
    );
    assert_eq!(section.get("cells").as_int(), i64::from(vw::SECTION_CELLS));
    assert_eq!(
        section.get("carriesData"),
        &Json::Bool(vw::SECTION_CARRIES_DATA)
    );

    let chunk = levels.get("Chunk");
    assert_eq!(
        chunk.get("sectionsPerChunk").as_int(),
        i64::from(vw::SECTIONS_PER_CHUNK)
    );
    assert_eq!(
        chunk.get("carriesData"),
        &Json::Bool(vw::CHUNK_CARRIES_DATA),
        "Chunk 不携带数据是契约红线"
    );
    const { assert!(!vw::CHUNK_CARRIES_DATA) };
    assert_eq!(
        chunk
            .get("extent")
            .as_arr()
            .iter()
            .map(Json::as_int)
            .collect::<Vec<_>>(),
        vec![
            i64::from(vw::SECTION_EXTENT),
            i64::from(vw::WORLD_HEIGHT_BLOCKS),
            i64::from(vw::SECTION_EXTENT)
        ]
    );
    assert_eq!(
        c.get("layering").get("worldHeightBlocks").as_int(),
        i64::from(vw::WORLD_HEIGHT_BLOCKS)
    );
}

#[test]
fn contract_limits_match() {
    let c = contract();
    let limits = c.get("limits");
    let expected: BTreeMap<&str, i64> = BTreeMap::from([
        ("sectionExtent", i64::from(vw::SECTION_EXTENT)),
        ("sectionCells", i64::from(vw::SECTION_CELLS)),
        ("sectionsPerChunk", i64::from(vw::SECTIONS_PER_CHUNK)),
        ("sectionYMin", i64::from(vw::SECTION_Y_MIN)),
        ("sectionYMax", i64::from(vw::SECTION_Y_MAX)),
        ("worldHeightBlocks", i64::from(vw::WORLD_HEIGHT_BLOCKS)),
        ("paletteMaxEntries", i64::from(vw::PALETTE_MAX_ENTRIES)),
        ("paletteIndexBits", i64::from(vw::PALETTE_INDEX_BITS)),
        ("blockTypeMax", i64::from(vw::BLOCK_TYPE_MAX)),
        ("blockStateMax", i64::from(vw::BLOCK_STATE_MAX)),
        ("lightBitsPerCell", i64::from(vw::LIGHT_BITS_PER_CELL)),
        ("lightMaxPropagation", i64::from(vw::LIGHT_MAX_PROPAGATION)),
        ("blockTypeScopeBit", i64::from(vw::BLOCK_TYPE_SCOPE_BIT)),
        ("blockTypeScopeMask", i64::from(vw::BLOCK_TYPE_SCOPE_MASK)),
        (
            "systemReservedTypeMax",
            i64::from(vw::SYSTEM_RESERVED_TYPE_MAX),
        ),
        (
            "firstOfficialBlockType",
            i64::from(vw::FIRST_OFFICIAL_BLOCK_TYPE),
        ),
        ("globalSegmentMax", i64::from(vw::GLOBAL_SEGMENT_MAX)),
        ("roomLocalSegmentMin", i64::from(vw::ROOM_LOCAL_SEGMENT_MIN)),
        ("worldYMin", i64::from(vw::WORLD_Y_MIN)),
        ("worldYMax", i64::from(vw::WORLD_Y_MAX)),
        (
            "maxCellsPerReadRequest",
            i64::from(vw::MAX_CELLS_PER_READ_REQUEST),
        ),
        (
            "maxEntriesPerWriteBatch",
            i64::from(vw::MAX_ENTRIES_PER_WRITE_BATCH),
        ),
        (
            "firstCatalogBlockType",
            i64::from(vw::FIRST_CATALOG_BLOCK_TYPE),
        ),
    ]);
    for (key, value) in &expected {
        assert_eq!(limits.get(key).as_int(), *value, "limits.{key} 漂移");
    }
    // 契约新增数值必须被本仓认领,不许静默漏掉。
    let declared: Vec<&str> = limits
        .keys()
        .into_iter()
        .filter(|k| *k != "notes")
        .collect();
    assert_eq!(
        declared.len(),
        expected.len(),
        "契约 limits 有本仓未映射的条目: {declared:?}"
    );
}

#[test]
fn contract_block_id_bitfields_match() {
    let c = contract();
    let block = c.get("blockId");
    assert_eq!(block.get("width").as_int(), i64::from(vw::BLOCK_ID_WIDTH));
    let ty = block.get("fields").get("BlockType");
    assert_eq!(ty.get("bits").as_int(), i64::from(vw::BLOCK_TYPE_BITS));
    assert_eq!(ty.get("shift").as_int(), i64::from(vw::BLOCK_TYPE_SHIFT));
    assert_eq!(ty.get("max").as_int(), i64::from(vw::BLOCK_TYPE_MAX));
    let st = block.get("fields").get("BlockState");
    assert_eq!(st.get("bits").as_int(), i64::from(vw::BLOCK_STATE_BITS));
    assert_eq!(st.get("shift").as_int(), i64::from(vw::BLOCK_STATE_SHIFT));
    assert_eq!(st.get("max").as_int(), i64::from(vw::BLOCK_STATE_MAX));
}

#[test]
fn contract_presence_and_encodings_match() {
    let c = contract();
    assert_eq!(
        c.get("diffDispatch").get("presence").keys(),
        vw::SECTION_PRESENCE.to_vec(),
        "四态 presence 必须与契约同名同序"
    );
    assert_eq!(
        c.get("diffDispatch")
            .get("shortTicket")
            .get("payloadLength")
            .as_int(),
        i64::from(vw::SHORT_TICKET_PAYLOAD_LENGTH)
    );
    assert_eq!(
        c.get("sectionPayload").get("encodings").keys(),
        vw::SECTION_PAYLOAD_ENCODINGS.to_vec()
    );
    assert_eq!(
        c.get("sectionPayload")
            .get("encodings")
            .get("Delta")
            .get("bytesPerEntry")
            .as_int(),
        i64::from(vw::DELTA_BYTES_PER_ENTRY)
    );
    assert_eq!(
        c.get("sectionPayload")
            .get("envelope")
            .get("required")
            .str_list(),
        vw::SECTION_PAYLOAD_ENVELOPE_FIELDS.to_vec()
    );
    assert_eq!(
        c.get("materialClasses").get("classes").keys(),
        vw::MATERIAL_CLASSES.to_vec()
    );
}

#[test]
fn contract_error_codes_match() {
    let c = contract();
    assert_eq!(
        c.get("errorCodes").str_list(),
        vw::VOXEL_WORLD_ERROR_CODES.to_vec(),
        "错误码表必须与契约同名同序"
    );
    // 任务卡点名要求覆盖的 7 条,必须都在表里且都有具名常量。
    for id in [
        vw::UNKNOWN_SECTION_KEY,
        vw::UNKNOWN_CHUNK_KEY,
        vw::SECTION_Y_OUT_OF_RANGE,
        vw::COORDINATE_OUT_OF_BOUNDS,
        vw::SECTION_UNAVAILABLE,
        vw::STALE_SECTION_REVISION,
        vw::DIRTY_SECTION_NOT_DURABLE,
    ] {
        assert!(vw::is_error_code(id), "{id} 必须出现在契约 errorCodes 里");
    }
    for extra in [
        vw::SECTION_ENCODING_MISMATCH,
        vw::SECTION_DIGEST_MISMATCH,
        vw::PALETTE_OVERFLOW,
        vw::CHUNK_CARRIES_DATA_ERROR,
        vw::LIGHTING_IN_PAYLOAD,
        vw::BLOCK_TYPE_SCOPE_VIOLATION,
        vw::ROOM_LOCAL_TYPE_WITHOUT_MAPPING,
        vw::WORLD_Y_OUT_OF_RANGE,
    ] {
        assert!(vw::is_error_code(extra));
    }
    // 上一版契约的页命名已被 sectionPayload 取代,旧 id 不得再被当作契约错误码。
    for retired in ["page_encoding_mismatch", "page_digest_mismatch"] {
        assert!(
            !vw::is_error_code(retired),
            "{retired} 已被 section_* 取代,不应还在契约表里"
        );
    }
    assert!(
        !vw::is_error_code("ChunkUnavailable"),
        "废弃镜像的驼峰 id 不是契约错误码"
    );
}

#[test]
fn contract_rules_that_this_repo_enforces_are_present() {
    let c = contract();
    let rules: BTreeMap<&str, &str> = c
        .get("rules")
        .as_arr()
        .iter()
        .map(|r| (r.get("id").as_str(), r.get("onViolation").as_str()))
        .collect();
    assert_eq!(rules["key.section.arity"], vw::UNKNOWN_SECTION_KEY);
    assert_eq!(rules["key.chunk.arity"], vw::UNKNOWN_CHUNK_KEY);
    assert_eq!(rules["key.canonical"], vw::UNKNOWN_SECTION_KEY);
    assert_eq!(rules["key.section.y-range"], vw::SECTION_Y_OUT_OF_RANGE);
    assert_eq!(rules["key.coordinate-bounds"], vw::COORDINATE_OUT_OF_BOUNDS);
    assert_eq!(
        rules["layering.chunk-carries-no-data"],
        vw::CHUNK_CARRIES_DATA_ERROR
    );
    assert_eq!(
        rules["residency.dirty-needs-ack"],
        vw::DIRTY_SECTION_NOT_DURABLE
    );
    assert_eq!(
        rules["residency.ack-covers-declared-bound"],
        vw::STALE_SECTION_REVISION
    );
    assert_eq!(
        rules["presence.missing-is-not-air"],
        vw::SECTION_UNAVAILABLE
    );
    assert_eq!(
        rules["presence.short-ticket-is-zero-bytes"],
        vw::SECTION_ENCODING_MISMATCH
    );
    // 页语义改名 payload 后,本仓消费的两条规则改从新 id 取。
    assert_eq!(
        rules["payload.digest-before-interpretation"],
        vw::SECTION_DIGEST_MISMATCH
    );
    assert_eq!(
        rules["payload.encoding-matches-content"],
        vw::SECTION_ENCODING_MISMATCH
    );
    assert_eq!(rules["payload.palette-cap"], vw::PALETTE_OVERFLOW);
}

/// `blockId.scope`:作用域位是段归属的唯一权威,本仓的判定函数必须与之一致。
#[test]
fn contract_block_type_scope_matches() {
    let c = contract();
    let scope = c.get("blockId").get("scope");
    assert_eq!(
        scope.get("bit").as_int(),
        i64::from(vw::BLOCK_TYPE_SCOPE_BIT)
    );
    assert_eq!(
        scope.get("mask").as_int(),
        i64::from(vw::BLOCK_TYPE_SCOPE_MASK)
    );
    assert_eq!(
        scope.get("global").get("firstOfficialBlock").as_int(),
        i64::from(vw::FIRST_OFFICIAL_BLOCK_TYPE)
    );
    const { assert!(vw::BLOCK_TYPE_SCOPE_MASK == 1 << vw::BLOCK_TYPE_SCOPE_BIT) };

    // 段边界两侧各取一点,证明判定函数落在契约声明的区间里。
    assert!(vw::is_global_segment(vw::GLOBAL_SEGMENT_MAX));
    assert!(!vw::is_room_local_segment(vw::GLOBAL_SEGMENT_MAX));
    assert!(vw::is_room_local_segment(vw::ROOM_LOCAL_SEGMENT_MIN));
    assert!(!vw::is_global_segment(vw::ROOM_LOCAL_SEGMENT_MIN));
    assert_eq!(vw::room_local_index(vw::ROOM_LOCAL_SEGMENT_MIN), 0);
    assert_eq!(
        vw::room_local_index(vw::BLOCK_TYPE_MAX),
        vw::GLOBAL_SEGMENT_MAX
    );
}

/// 架构仓在场时,证明本仓副本与上游逐字节一致;不在场时打印跳过原因而不是假装通过。
#[test]
fn vendored_copy_matches_upstream_when_available() {
    let upstream = match std::env::var("LUMIO_ENGINE_WIRE_DIR") {
        Ok(dir) => PathBuf::from(dir).join("voxel-world-v1.json"),
        Err(_) => PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../LumioGameEngine/engine/wire/voxel-world-v1.json"),
    };
    if !upstream.is_file() {
        eprintln!(
            "跳过上游比对:{} 不存在(设 LUMIO_ENGINE_WIRE_DIR 指向架构仓 engine/wire)",
            upstream.display()
        );
        return;
    }
    let ours = std::fs::read(wire_path()).expect("契约文件必须随仓提交");
    let theirs = std::fs::read(&upstream).expect("上游契约可读");
    assert_eq!(
        hex32(sha256(&ours)),
        hex32(sha256(&theirs)),
        "本仓 wire/voxel-world-v1.json 与架构仓 {} 不是同一份字节",
        upstream.display()
    );
}

#[test]
fn contract_dispatch_cases_are_the_cases_exercised_by_the_domain() {
    let c = contract();
    let positive: Vec<_> = c
        .get("testCases")
        .as_arr()
        .iter()
        .filter(|case| case.get("class").as_str() == "dispatch")
        .map(|case| case.get("name").as_str())
        .collect();
    assert_eq!(positive, ["unchanged_section_is_zero_bytes"]);

    let invalid: BTreeMap<_, _> = c
        .get("invalidCases")
        .as_arr()
        .iter()
        .filter(|case| case.get("class").as_str() == "dispatch")
        .map(|case| {
            (
                case.get("name").as_str(),
                case.get("expectedRejection").as_str(),
            )
        })
        .collect();
    assert_eq!(invalid.len(), 3);
    assert_eq!(
        invalid["pending_section_rendered_as_air"],
        vw::SECTION_UNAVAILABLE
    );
    assert_eq!(
        invalid["unavailable_section_treated_as_deleted"],
        vw::SECTION_UNAVAILABLE
    );
    assert_eq!(
        invalid["unchanged_answered_with_full_payload"],
        vw::SECTION_ENCODING_MISMATCH
    );
}
