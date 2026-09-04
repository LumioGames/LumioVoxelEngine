//! Judgments on the Voxel-local canonical encoder and its decoder.
//!
//! The property under test is injectivity: distinct member sets must produce
//! distinct bytes, and decode must return exactly what encode was given.

use lumio_voxel_ops::canonical::{CanonicalObject, CanonicalValue, DecodeError, decode};
use std::collections::HashMap;
use std::time::Instant;

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
        ("s:0:0:0", text("section")),
        ("sectionRevision.c:0:0:0", CanonicalValue::Uint(7)),
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
///
/// The refusal covers a *paired* surrogate escape too, which is a deliberate choice
/// and not an oversight: the encoder emits `\u` only for C0 controls, so
/// `\ud83d\ude00` would be a second spelling of a character that already has one,
/// and a second spelling is a second path to one meaning. Nor is it an inability to
/// carry astral characters — the same character round-trips as raw UTF-8, asserted
/// at the end. Anyone later teaching the parser to combine surrogate pairs, on the
/// reasonable-sounding grounds that JSON allows it, should have this fail.
#[test]
fn decode_rejects_surrogate_escapes_in_every_form() {
    // lone high half, lone low half
    assert_eq!(decode(b"{\"a\":\"\\ud800\"}"), Err(DecodeError::Malformed));
    assert_eq!(decode(b"{\"a\":\"\\udfff\"}"), Err(DecodeError::Malformed));
    // a well-formed pair spelling U+1F600
    assert_eq!(
        decode(b"{\"a\":\"\\ud83d\\ude00\"}"),
        Err(DecodeError::Malformed)
    );
    // a high half followed by an ordinary character
    assert_eq!(decode(b"{\"a\":\"\\ud83db\"}"), Err(DecodeError::Malformed));

    // ...while the character itself is carried, in the one spelling that exists.
    let mut object = CanonicalObject::new();
    object.insert_text("a", "\u{1f600}").expect("first insert");
    let bytes = object.encode_bytes();
    assert_eq!(bytes, "{\"a\":\"\u{1f600}\"}".as_bytes());
    assert_eq!(decode(&bytes).expect("astral round trip"), object);
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

/// Every atom the corpus below builds names and values from. The set is chosen so
/// that anything the encoder treats specially appears both as itself and inside a
/// string that is trying to impersonate it: the structural bytes `" \ , : { } [ ]`,
/// a value that spells out a whole extra member, a value that closes its own quotes,
/// a trailing backslash, the two ends of the C0 range, DEL, the *literal text* of an
/// escape (`A`, `\uD800`) as opposed to the escape itself, an astral pair, a
/// non-ASCII scalar, and the empty string.
const ATOMS: &[&str] = &[
    "",
    "a",
    "A",
    "\"",
    "\\",
    ",",
    ":",
    "{",
    "}",
    "[",
    "]",
    "1,\"b\":2",
    "1\",\"b\":\"2",
    "x\\",
    "\\\\",
    "\0",
    "\u{1f}",
    "\n",
    "\u{7f}",
    "\\u0041",
    "\\uD800",
    "\u{1f600}\u{1f600}",
    "é",
];

/// One value of each variant, so a corpus member can differ by shape and not only
/// by content: a string that looks like an integer must not collide with that
/// integer, and a one-element array must not collide with its element.
fn value_alphabet() -> Vec<CanonicalValue> {
    let mut values: Vec<CanonicalValue> = ATOMS.iter().map(|a| text(a)).collect();
    values.extend([
        CanonicalValue::Uint(0),
        CanonicalValue::Uint(1),
        CanonicalValue::Uint(7),
        CanonicalValue::Uint(u64::MAX),
    ]);
    values.push(CanonicalValue::TextArray(vec![]));
    values.push(CanonicalValue::TextArray(vec!["a".into()]));
    values.push(CanonicalValue::TextArray(vec![
        "x\",\"y".into(),
        "\\".into(),
    ]));
    values.push(CanonicalValue::TextArray(vec![
        "a".into(),
        "".into(),
        "\u{1f600}".into(),
    ]));
    values
}

/// Objects of one, two and three members over the alphabets above, plus the cases
/// that are too long to belong in a combinatorial sweep.
fn corpus() -> Vec<CanonicalObject> {
    let values = value_alphabet();
    let mut cases = Vec::new();

    // one member: every name against every value
    for name in ATOMS {
        for value in &values {
            cases.push(object(&[(name, value.clone())]));
        }
    }

    // two members: names must differ, so the pair is drawn without repetition
    let names2 = &ATOMS[..12];
    let values2 = &values[..12];
    for (i, a) in names2.iter().enumerate() {
        for b in &names2[i + 1..] {
            for va in values2 {
                for vb in values2 {
                    cases.push(object(&[(a, va.clone()), (b, vb.clone())]));
                }
            }
        }
    }

    // three members: the same, narrowed so the sweep stays a second rather than a minute
    let names3 = &ATOMS[..8];
    let values3 = &values[..7];
    for (i, a) in names3.iter().enumerate() {
        for (j, b) in names3.iter().enumerate().skip(i + 1) {
            for c in &names3[j + 1..] {
                for va in values3 {
                    for vb in values3 {
                        for vc in values3 {
                            cases.push(object(&[
                                (a, va.clone()),
                                (b, vb.clone()),
                                (c, vc.clone()),
                            ]));
                        }
                    }
                }
            }
        }
    }

    // Long names and values: a scan that resets on a delimiter behaves differently
    // once the run between delimiters is longer than any buffer it might reuse.
    let long_plain = "z".repeat(600);
    let long_escaping = "\"\\\n".repeat(200);
    cases.push(object(&[(long_plain.as_str(), text("v"))]));
    cases.push(object(&[("k", text(&long_plain))]));
    cases.push(object(&[(long_escaping.as_str(), text(&long_escaping))]));
    cases.push(object(&[
        (long_plain.as_str(), text(&long_escaping)),
        ("k", CanonicalValue::TextArray(vec![long_plain.clone()])),
    ]));

    cases
}

/// Injectivity as a property rather than as a list of near misses.
///
/// `decode` being a left inverse of `encode` *is* injectivity — if two objects
/// shared bytes, decode could not return both of them — so the property is worth
/// far more than the hand-picked pairs above, and costs one loop. The map is the
/// same statement said forwards, and catches the case a left inverse alone would
/// not: an encoder and a decoder wrong in compensating ways.
#[test]
fn encode_is_injective_over_the_corpus_and_decode_is_its_left_inverse() {
    let cases = corpus();
    assert!(
        cases.len() > 20_000,
        "corpus shrank to {} cases; it is meant to be a sweep",
        cases.len()
    );
    let mut seen: HashMap<Vec<u8>, CanonicalObject> = HashMap::with_capacity(cases.len());
    for candidate in cases {
        let bytes = candidate.encode_bytes();

        // Left inverse: what comes back is the object, not a regrouping of it.
        let decoded = decode(&bytes).unwrap_or_else(|err| {
            panic!(
                "own bytes rejected as {err:?}: {:?} encoded {:?}",
                candidate,
                String::from_utf8_lossy(&bytes)
            )
        });
        assert_eq!(
            decoded,
            candidate,
            "round trip changed the object: {:?}",
            String::from_utf8_lossy(&bytes)
        );

        // Injectivity, stated forwards.
        if let Some(other) = seen.insert(bytes.clone(), candidate.clone())
            && other != candidate
        {
            panic!(
                "distinct objects share bytes {}:\n  {other:?}\n  {candidate:?}",
                String::from_utf8_lossy(&bytes)
            );
        }
    }
}

/// An empty member name holding an empty value is canonical and is not the empty
/// object. Nothing rejects it, which is a decision rather than an oversight: the
/// name is quoted and escaped like any other, so it cannot merge with a neighbour,
/// and refusing it would add a rule the injectivity argument does not need.
#[test]
fn empty_member_name_and_value_is_canonical_and_distinct() {
    let empty_member = object(&[("", text(""))]);
    assert_eq!(empty_member.encode(), "{\"\":\"\"}");
    assert_eq!(decode(b"{\"\":\"\"}").expect("round trip"), empty_member);

    // ...and is a different object from the three it sits closest to.
    assert_ne!(empty_member.encode(), CanonicalObject::new().encode());
    assert_ne!(empty_member.encode(), object(&[("", text("a"))]).encode());
    assert_ne!(empty_member.encode(), object(&[("a", text(""))]).encode());
}

/// The snapshot manifest shape `ManifestAdapter::object` builds, at a chosen size:
/// fixed header members plus one `sectionRevision.<id>` per section.
fn manifest_bytes(sections: usize) -> Vec<u8> {
    let mut manifest = CanonicalObject::new();
    for (key, value) in [
        ("schemaId", text("voxel-snapshot-payload")),
        ("headerSchemaId", text("snapshot-header")),
        ("magic", text("LUMIOSNP1")),
        ("schemaEpoch", CanonicalValue::Uint(1)),
        ("worldId", text("world-a")),
        ("contextId", text("ctx-1")),
        ("generation", CanonicalValue::Uint(1)),
        ("worldRevision", CanonicalValue::Uint(0)),
        ("configHash", text(&"a".repeat(64))),
        ("rootIdentity", text(&"b".repeat(64))),
    ] {
        manifest
            .insert(key, value)
            .expect("distinct header members");
    }
    for i in 0..sections {
        manifest
            .insert(
                format!("sectionRevision.c:{i}:0:0"),
                CanonicalValue::Uint(1),
            )
            .expect("distinct section members");
    }
    manifest.encode_bytes()
}

fn decode_nanos(bytes: &[u8]) -> f64 {
    let start = Instant::now();
    let decoded = decode(bytes).expect("manifest decodes");
    let elapsed = start.elapsed().as_nanos();
    // Keep the result live so the decode cannot be optimised out from under the clock.
    assert!(!decoded.is_empty());
    elapsed as f64
}

/// Decode cost must track input length, not its square.
///
/// `RestorePreflight::validate` takes bytes the Host read back off the filesystem —
/// VOX-D-008 puts DAG orchestration, fsync and the Active-pointer swap on the Host —
/// so the length is chosen by whatever wrote the file, and a corrupt or hostile
/// archive is on this path. A parser that re-validates the *remaining* buffer once
/// per character is quadratic in that length: a 469 KB manifest cost 1.56 s against
/// 7.4 ms once the check was hoisted to a single pass.
///
/// Nothing else in this file can tell those two parsers apart, because they accept
/// and reject exactly the same bytes — the difference is only cost, so only a cost
/// judgment catches a regression.
///
/// The verdict is a ratio, so it does not encode the speed of the machine it runs
/// on: growing the input ~4× must not grow the time more than ~8×, where linear
/// lands near 4 and quadratic near 16. Five independent paired measurements are
/// taken and the *smallest* ratio decides, so a single contended sample cannot fail
/// a build.
#[test]
fn decode_cost_grows_with_input_length_not_its_square() {
    let small = manifest_bytes(1_000);
    let large = manifest_bytes(8_000);
    let size_ratio = large.len() as f64 / small.len() as f64;
    let ceiling = 2.0 * size_ratio;

    let mut best = f64::MAX;
    for _ in 0..5 {
        let t_small = decode_nanos(&small);
        let t_large = decode_nanos(&large);
        assert!(t_small > 0.0, "clock too coarse to judge decode cost");
        best = best.min(t_large / t_small);
    }

    assert!(
        best < ceiling,
        "decode time grew ×{best:.1} for an input ×{size_ratio:.1} longer \
         ({} B to {} B); linear is ≈×{size_ratio:.1}, quadratic ≈×{:.1}, \
         and the ceiling is ×{ceiling:.1}",
        small.len(),
        large.len(),
        size_ratio * size_ratio
    );
}
