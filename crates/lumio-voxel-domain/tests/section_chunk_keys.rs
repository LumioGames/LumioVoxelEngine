//! 契约 `lumio.voxel-world.v1` 的 `identity` 段:Section 键三坐标、Chunk 键两坐标、
//! 规范写法、y 定义域、int32 边界与两者的互推。
//!
//! 元数即防呆:旧式三坐标 `c:x:y:z` 在新语法下语法非法,必须**显式**拒绝——既不得被解读
//! 成 Chunk 键,也不得被解读成 Section 键,且拒绝原因不能只是「当成别的键解析失败」。

use lumio_voxel_contracts::voxel_world as vw;
use lumio_voxel_domain::key::{ChunkId, KeyKind, KeyRejection, SectionId};

// ------------------------------------------------------------------ 正向:规范键

#[test]
fn section_key_canonical_roundtrip() {
    let id = SectionId::parse("s:-3:7:0").expect("规范 Section 键必须解析成功");
    assert_eq!((id.x(), id.y(), id.z()), (-3, 7, 0));
    assert_eq!(id.key(), "s:-3:7:0");
    assert_eq!(SectionId::new(-3, 7, 0).unwrap(), id);
}

#[test]
fn chunk_key_two_coordinates() {
    let id = ChunkId::parse("c:12:-5").expect("规范 Chunk 键必须解析成功");
    assert_eq!((id.x(), id.z()), (12, -5));
    assert_eq!(id.key(), "c:12:-5");
    assert_eq!(id.key().split(':').count(), 3, "前缀 + 两个坐标");
}

#[test]
fn section_y_spans_zero_through_fifteen() {
    for y in vw::SECTION_Y_MIN..=vw::SECTION_Y_MAX {
        let raw = format!("s:0:{y}:0");
        let id = SectionId::parse(&raw).unwrap_or_else(|e| panic!("{raw} 应合法,却得到 {e:?}"));
        assert_eq!(id.y(), y);
        assert_eq!(id.key(), raw);
    }
}

#[test]
fn int32_extremes_are_first_class() {
    for raw in [
        "s:-2147483648:0:2147483647",
        "s:2147483647:15:-2147483648",
        "c:-2147483648:2147483647",
    ] {
        assert!(
            SectionId::parse(raw).is_ok() || ChunkId::parse(raw).is_ok(),
            "{raw} 落在 int32 定义域内,必须合法"
        );
    }
}

// ------------------------------------------------------------------ 派生:s ↔ c

#[test]
fn section_to_chunk_derivation_drops_y() {
    let section = SectionId::parse("s:12:9:-5").unwrap();
    assert_eq!(section.chunk(), ChunkId::parse("c:12:-5").unwrap());
    assert_eq!(section.chunk().key(), "c:12:-5");
}

#[test]
fn chunk_contains_exactly_sixteen_sections() {
    let chunk = ChunkId::parse("c:12:-5").unwrap();
    let sections: Vec<SectionId> = chunk.sections().collect();
    assert_eq!(sections.len(), vw::SECTIONS_PER_CHUNK as usize);
    assert_eq!(sections.len(), 16);
    let keys: Vec<String> = sections.iter().map(SectionId::key).collect();
    assert_eq!(keys.first().map(String::as_str), Some("s:12:0:-5"));
    assert_eq!(keys.last().map(String::as_str), Some("s:12:15:-5"));
    for (index, section) in sections.iter().enumerate() {
        assert_eq!(section.y() as usize, index);
        assert_eq!(section.chunk(), chunk, "每个 Section 都回推到同一个 Chunk");
    }
}

// --------------------------------------------- 元数即防呆:旧式三坐标 c:x:y:z 被拒

#[test]
fn legacy_three_coordinate_chunk_key_is_rejected_as_a_chunk_key() {
    let err = ChunkId::parse("c:12:9:-5").expect_err("三坐标 c: 键在语法上即非法");
    assert_eq!(err.kind(), KeyKind::Chunk);
    assert_eq!(
        err.rejection(),
        KeyRejection::LegacyThreeCoordinateChunkKey,
        "拒绝原因必须是元数守卫本身,不是笼统的解析失败"
    );
    assert_eq!(err.error_id(), vw::UNKNOWN_CHUNK_KEY);
}

#[test]
fn legacy_three_coordinate_chunk_key_is_rejected_as_a_section_key() {
    let err = SectionId::parse("c:12:9:-5").expect_err("三坐标 c: 键不是 Section 键");
    assert_eq!(err.kind(), KeyKind::Section);
    assert_eq!(err.rejection(), KeyRejection::LegacyThreeCoordinateChunkKey);
    assert_eq!(err.error_id(), vw::UNKNOWN_SECTION_KEY);
}

#[test]
fn legacy_key_is_never_reinterpreted_as_either_layer() {
    let legacy = "c:12:9:-5";
    assert!(ChunkId::parse(legacy).is_err());
    assert!(SectionId::parse(legacy).is_err());
    // 不得被悄悄解读成 c:12:-5 或 s:12:9:-5。
    assert_ne!(
        ChunkId::parse(legacy).ok(),
        Some(ChunkId::parse("c:12:-5").unwrap())
    );
    assert_ne!(
        SectionId::parse(legacy).ok(),
        Some(SectionId::parse("s:12:9:-5").unwrap())
    );
}

#[test]
fn legacy_rejection_is_distinct_from_a_plain_prefix_miss() {
    // 三坐标 c: 键走元数守卫;两坐标 c: 键当 Section 用只是前缀/元数不对。
    let legacy = SectionId::parse("c:1:2:3").expect_err("legacy");
    let plain = SectionId::parse("c:1:3").expect_err("prefix");
    assert_eq!(
        legacy.rejection(),
        KeyRejection::LegacyThreeCoordinateChunkKey
    );
    assert_ne!(
        plain.rejection(),
        KeyRejection::LegacyThreeCoordinateChunkKey
    );
}

// ------------------------------------------------------------------ 反向:非法键

#[test]
fn section_y_sixteen_is_out_of_range() {
    let err = SectionId::parse("s:3:16:9").expect_err("层号只能是 0~15");
    assert_eq!(err.rejection(), KeyRejection::SectionYOutOfRange);
    assert_eq!(err.error_id(), vw::SECTION_Y_OUT_OF_RANGE);
    assert!(SectionId::new(3, 16, 9).is_err());
}

#[test]
fn section_y_negative_is_out_of_range() {
    let err = SectionId::parse("s:0:-1:0").expect_err("层号不能为负");
    assert_eq!(err.error_id(), vw::SECTION_Y_OUT_OF_RANGE);
}

#[test]
fn leading_zero_is_not_canonical() {
    for raw in ["s:012:0:0", "s:0:00:0", "s:0:0:007", "c:012:0"] {
        let rejection = match raw.as_bytes()[0] {
            b's' => SectionId::parse(raw).expect_err(raw).rejection(),
            _ => ChunkId::parse(raw).expect_err(raw).rejection(),
        };
        assert_eq!(rejection, KeyRejection::NonCanonicalCoordinate, "{raw}");
    }
    assert_eq!(
        SectionId::parse("s:012:0:0")
            .expect_err("leading zero")
            .error_id(),
        vw::UNKNOWN_SECTION_KEY
    );
}

#[test]
fn negative_zero_is_not_canonical() {
    let err = SectionId::parse("s:-0:0:0").expect_err("不得出现 -0");
    assert_eq!(err.rejection(), KeyRejection::NonCanonicalCoordinate);
    assert_eq!(err.error_id(), vw::UNKNOWN_SECTION_KEY);
    assert_eq!(
        ChunkId::parse("c:0:-0")
            .expect_err("不得出现 -0")
            .error_id(),
        vw::UNKNOWN_CHUNK_KEY
    );
}

#[test]
fn coordinate_beyond_int32_is_out_of_bounds() {
    for raw in ["s:2147483648:0:0", "s:0:0:-2147483649", "s:99999999999:0:0"] {
        let err = SectionId::parse(raw).expect_err(raw);
        assert_eq!(
            err.rejection(),
            KeyRejection::CoordinateOutOfBounds,
            "{raw}"
        );
        assert_eq!(err.error_id(), vw::COORDINATE_OUT_OF_BOUNDS, "{raw}");
    }
    assert_eq!(
        ChunkId::parse("c:2147483648:0")
            .expect_err("x 越界")
            .error_id(),
        vw::COORDINATE_OUT_OF_BOUNDS
    );
}

#[test]
fn wrong_arity_and_prefix_are_rejected() {
    for raw in ["s:1:2", "s:1:2:3:4", "s:1", "s:", "s"] {
        let err = SectionId::parse(raw).expect_err(raw);
        assert_eq!(err.error_id(), vw::UNKNOWN_SECTION_KEY, "{raw}");
    }
    for raw in ["c:1", "c:", "c", "c:1:2:3:4"] {
        let err = ChunkId::parse(raw).expect_err(raw);
        assert_eq!(err.error_id(), vw::UNKNOWN_CHUNK_KEY, "{raw}");
    }
    for raw in ["x:1:2:3", "1:2:3", "S:1:2:3", ""] {
        assert_eq!(
            SectionId::parse(raw).expect_err(raw).rejection(),
            KeyRejection::WrongPrefix,
            "{raw}"
        );
    }
    assert_eq!(
        ChunkId::parse("s:1:2")
            .expect_err("s 前缀不是 Chunk 键")
            .rejection(),
        KeyRejection::WrongPrefix
    );
}

#[test]
fn non_decimal_components_are_rejected() {
    for raw in ["s:1.0:0:0", "s:+1:0:0", "s: 1:0:0", "s:1e3:0:0", "s:-:0:0"] {
        let err = SectionId::parse(raw).expect_err(raw);
        assert_eq!(err.error_id(), vw::UNKNOWN_SECTION_KEY, "{raw}");
    }
}

// ------------------------------------------------------- Chunk 不携带数据的结构证明

#[test]
fn chunk_id_carries_only_two_coordinates() {
    assert_eq!(
        std::mem::size_of::<ChunkId>(),
        std::mem::size_of::<[i32; 2]>(),
        "Chunk 只有 x/z 两个坐标——没有数据字段,也没有独立 revision"
    );
    const { assert!(!vw::CHUNK_CARRIES_DATA) };
}

// ------------------------------------------------------------- 错误码全部来自契约

#[test]
fn every_key_error_id_is_a_contract_error_code() {
    let samples = [
        SectionId::parse("c:1:2:3").unwrap_err().error_id(),
        SectionId::parse("s:012:0:0").unwrap_err().error_id(),
        SectionId::parse("s:0:16:0").unwrap_err().error_id(),
        SectionId::parse("s:2147483648:0:0").unwrap_err().error_id(),
        ChunkId::parse("c:1:2:3").unwrap_err().error_id(),
        ChunkId::parse("c:1").unwrap_err().error_id(),
    ];
    for id in samples {
        assert!(vw::is_error_code(id), "{id} 不在契约 errorCodes 里");
        assert!(
            std::ptr::eq(vw::intern_error_code(id).expect("interned"), id),
            "{id} 必须是契约表里的那一份 'static 实例"
        );
    }
}
