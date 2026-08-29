//! Judgments on the Voxel-local canonical encoder and its decoder.
//!
//! The property under test is injectivity: distinct member sets must produce
//! distinct bytes, and decode must return exactly what encode was given.

use lumio_voxel_ops::canonical::{CanonicalObject, CanonicalValue, DecodeError, decode};

fn object(members: &[(&str, CanonicalValue)]) -> CanonicalObject {
    let mut object = CanonicalObject::new();
    for (key, value) in members {
        object.insert(*key, value.clone()).expect("distinct names");
    }
    object
}

fn text(value: &str) -> CanonicalValue {
    CanonicalValue::text(value)
}

/// The whole point: near-miss member sets, none sharing bytes with another.
#[test]
fn distinct_member_sets_encode_to_distinct_bytes() {
    let cases = vec![
        // value substitution across a member boundary
        object(&[("a", text("1,\"b\":2"))]),
        object(&[("a", text("1")), ("b", text("2"))]),
        // key that tries to carry its own delimiter
        object(&[("a\":1,\"b", text("2"))]),
        // append and delete
        object(&[("a", text("1"))]),
        object(&[("a", text("1")), ("b", text("2")), ("c", text("3"))]),
        object(&[("a", text("1,\"b\":2,\"c\":3"))]),
        // a string that looks like an integer is not that integer
        object(&[("a", text("7"))]),
        object(&[("a", CanonicalValue::Uint(7))]),
        // an array element that tries to become two elements
        object(&[("a", CanonicalValue::TextArray(vec!["x\",\"y".into()]))]),
        object(&[("a", CanonicalValue::TextArray(vec!["x".into(), "y".into()]))]),
        // an array that tries to become a string, and back
        object(&[("a", CanonicalValue::TextArray(vec![]))]),
        object(&[("a", text("[]"))]),
        // escape carriers
        object(&[("a", text("\\"))]),
        object(&[("a", text("\\\\"))]),
        object(&[("a", text("\""))]),
        object(&[("a", text("\n"))]),
        object(&[("a", text("\\u000a"))]),
    ];
    let mut seen: Vec<(String, &CanonicalObject)> = Vec::new();
    for candidate in &cases {
        let bytes = candidate.encode();
        if let Some((_, other)) = seen.iter().find(|(existing, _)| *existing == bytes) {
            panic!("distinct objects share bytes {bytes}:\n  {other:?}\n  {candidate:?}");
        }
        seen.push((bytes, candidate));
    }
}

#[test]
fn insert_rejects_a_repeated_name() {
    let mut object = CanonicalObject::new();
    object.insert_text("a", "first").expect("first insert");
    let err = object
        .insert_text("a", "second")
        .expect_err("a repeated name must be refused");
    assert_eq!(err.key(), "a");
    // The refusal must not have overwritten the member already present.
    assert_eq!(
        object.get("a").and_then(CanonicalValue::as_text),
        Some("first")
    );
}

#[test]
fn decode_round_trips_every_value_kind() {
    let original = object(&[
        ("plain", text("value")),
        ("quote", text("a\"b")),
        ("backslash", text("a\\b")),
        ("control", text("a\nb")),
        ("astral", text("\u{1f600}")),
        ("empty", text("")),
        ("zero", CanonicalValue::Uint(0)),
        ("max", CanonicalValue::Uint(u64::MAX)),
        (
            "list",
            CanonicalValue::TextArray(vec!["x".into(), "y,\"z".into()]),
        ),
        ("emptylist", CanonicalValue::TextArray(vec![])),
        ("c:0:0:0", text("chunk")),
        ("chunkRevision.c:0:0:0", CanonicalValue::Uint(7)),
    ]);
    let bytes = original.encode_bytes();
    assert_eq!(decode(&bytes).expect("round trip"), original);
}

#[test]
fn empty_object_round_trips() {
    let empty = CanonicalObject::new();
    assert_eq!(empty.encode(), "{}");
    assert_eq!(decode(b"{}").expect("round trip"), empty);
}

/// The recanonicalize guard is load-bearing, not true by construction: the parser
/// accepts these, and only the re-encode comparison rejects them.
#[test]
fn decode_rejects_input_that_parses_but_is_not_canonical() {
    // members out of order
    assert_eq!(
        decode(b"{\"b\":\"2\",\"a\":\"1\"}"),
        Err(DecodeError::NotCanonical)
    );
    // a non-minimal escape: `\u0041` parses to `A`, which encodes back as `A`
    assert_eq!(
        decode(b"{\"a\":\"\\u0041\"}"),
        Err(DecodeError::NotCanonical)
    );
    // a non-minimal escape of a character that does need escaping
    assert_eq!(
        decode(b"{\"a\":\"\\u0022\"}"),
        Err(DecodeError::NotCanonical)
    );
    // both spellings of the same member name
    assert_eq!(
        decode(b"{\"\\u0061\":\"1\"}"),
        Err(DecodeError::NotCanonical)
    );
}

#[test]
fn decode_rejects_duplicate_member_names() {
    assert_eq!(
        decode(b"{\"a\":\"1\",\"a\":\"2\"}"),
        Err(DecodeError::DuplicateMember)
    );
}

/// A UTF-16 surrogate half is not a scalar value. C# strings can hold one and
/// .NET's default UTF-8 encoder folds it onto U+FFFD, which would give two
/// different strings one digest; refusing the escape keeps that off this side.
#[test]
fn decode_rejects_a_lone_surrogate_escape() {
    assert_eq!(decode(b"{\"a\":\"\\ud800\"}"), Err(DecodeError::Malformed));
    assert_eq!(decode(b"{\"a\":\"\\udfff\"}"), Err(DecodeError::Malformed));
}

#[test]
fn decode_rejects_malformed_input() {
    for bad in [
        &b"{\"a\":\"\\n\"}"[..],              // an escape the encoder never emits
        &b"{\"a\":\"\\x41\"}"[..],            // not an escape at all
        &b"{\"a\":\"b\"}}"[..],               // trailing byte
        &b"{\"a\":\"b\""[..],                 // truncated object
        &b"{\"a\":\"b}"[..],                  // truncated string
        &b"{\"a\":01}"[..],                   // leading zero integer
        &b"{\"a\":-1}"[..],                   // signed integer
        &b"{\"a\":18446744073709551616}"[..], // beyond u64
        &b"{\"a\":\"b\",}"[..],               // trailing comma
        &b"{\"a\"\"b\"}"[..],                 // missing separator
        &b"{a:\"b\"}"[..],                    // unquoted name
        &b"{\"a\":\"b\n\"}"[..],              // raw control byte in a string
        &b""[..],                             // nothing at all
    ] {
        assert_eq!(
            decode(bad),
            Err(DecodeError::Malformed),
            "accepted {:?}",
            String::from_utf8_lossy(bad)
        );
    }
}
