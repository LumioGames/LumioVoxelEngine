//! R-00076: staged delta, dirty frontier coverage, replacement freeze.

use lumio_voxel_contracts::{SCHEMA_IDS, STABLE_ERROR_IDS, sha256};
use lumio_voxel_domain::chunk::{
    ChunkDeltaBuilder, ChunkDirectoryBuilder, ChunkDirectoryRoot, ChunkPage, ChunkPayload,
    ChunkSlot, CoveredChunkAck, DirtyFrontier, DurabilityAckContext, DurabilityAckEvidence,
    StagedEdit,
};
use std::collections::HashMap;

fn dense_page(bytes: &[u8]) -> ChunkPage {
    ChunkPage::new("Dense", "None", bytes.to_vec(), sha256(bytes))
}

fn payload(bytes: &[u8]) -> ChunkPayload {
    ChunkPayload::from_pages([dense_page(bytes)]).expect("valid dense uncompressed page")
}

fn assert_stable_error(id: &str) {
    assert!(
        STABLE_ERROR_IDS.contains(&id),
        "error id {id} is not a generated STABLE_ERROR_IDS member"
    );
}

/// Canonical input-root fingerprint: sorted chunk ids + presence + payload digest.
fn hash_root(root: &ChunkDirectoryRoot, known: &[(&str, Option<[u8; 32]>)]) -> [u8; 32] {
    let mut items = known.to_vec();
    items.sort_by(|a, b| a.0.cmp(b.0));
    let mut buf = Vec::new();
    for (id, digest) in items {
        buf.extend_from_slice(id.as_bytes());
        buf.push(0);
        let slot = root
            .lookup(id)
            .expect("canonical chunk id")
            .expect("slot present in input root");
        buf.extend_from_slice(slot.presence().as_bytes());
        buf.push(0);
        match (slot.payload().is_some(), digest) {
            (true, Some(d)) => buf.extend_from_slice(&d),
            (false, None) => buf.extend_from_slice(&[0u8; 32]),
            _ => panic!("payload presence and digest witness disagree for {id}"),
        }
    }
    sha256(&buf)
}

fn sample_root() -> (ChunkDirectoryRoot, [u8; 32], [u8; 32]) {
    let ready_bytes = b"ready-payload";
    let ready_digest = sha256(ready_bytes);
    let mut builder = ChunkDirectoryBuilder::new();
    builder
        .insert("c:0:0:0", ChunkSlot::ready(payload(ready_bytes)))
        .expect("canonical chunk id");
    builder
        .insert("c:1:0:0", ChunkSlot::unavailable())
        .expect("canonical chunk id");
    builder
        .insert("c:2:2:2", ChunkSlot::pending())
        .expect("canonical chunk id");
    let root = builder.freeze();
    let hash = hash_root(
        &root,
        &[
            ("c:0:0:0", Some(ready_digest)),
            ("c:1:0:0", None),
            ("c:2:2:2", None),
        ],
    );
    (root, hash, ready_digest)
}

fn known_entries(ready_digest: [u8; 32]) -> [(&'static str, Option<[u8; 32]>); 3] {
    [
        ("c:0:0:0", Some(ready_digest)),
        ("c:1:0:0", None),
        ("c:2:2:2", None),
    ]
}

fn ack(
    world_id: &str,
    generation: u64,
    covered_world_revision: u64,
    chunks: &[(&str, u64)],
) -> DurabilityAckEvidence {
    DurabilityAckEvidence {
        kind: "DurabilityAck".to_string(),
        world_id: world_id.to_string(),
        context: DurabilityAckContext {
            context_id: "ctx-1".to_string(),
            generation,
        },
        covered_world_revision,
        covered_chunks: chunks
            .iter()
            .map(|(id, rev)| CoveredChunkAck {
                chunk_id: (*id).to_string(),
                up_to_chunk_revision: *rev,
            })
            .collect(),
    }
}

#[test]
fn failed_stage_or_freeze_leaves_input_root_hash_unchanged() {
    let (root, before_hash, ready_digest) = sample_root();
    let before = root.clone();
    let known = known_entries(ready_digest);

    let mut builder = ChunkDeltaBuilder::new(&root);
    let err = builder
        .stage(("c:01:0:0", ChunkSlot::not_loaded()))
        .unwrap_err();
    assert_eq!(err.error_id(), "CoordinateOutOfBounds");
    assert_stable_error(err.error_id());
    assert_eq!(root, before);
    assert_eq!(hash_root(&root, &known), before_hash);

    builder
        .stage(("c:0:0:0", ChunkSlot::ready(payload(b"first"))))
        .expect("first stage");
    let err = builder
        .stage(("c:0:0:0", ChunkSlot::ready(payload(b"conflict"))))
        .unwrap_err();
    assert_eq!(err.error_id(), "InvalidHandle");
    assert_stable_error(err.error_id());

    let err = builder
        .stage(StagedEdit::new("c:3:0:0", ChunkSlot::pending()).cells(["0:0:0", "1:0:0", "0:0:0"]))
        .unwrap_err();
    assert_eq!(err.error_id(), "InvalidHandle");
    assert_stable_error(err.error_id());
    drop(builder);

    let mut illegal = ChunkDeltaBuilder::new(&root);
    illegal
        .stage(("c:1:0:0", ChunkSlot::ready(payload(b"skip-pending"))))
        .expect("stage does not publish");
    let err = illegal.freeze().unwrap_err();
    assert_eq!(err.error_id(), "ChunkUnavailable");
    assert_stable_error(err.error_id());

    assert_eq!(root, before);
    assert_eq!(hash_root(&root, &known), before_hash);
    assert_eq!(
        root.lookup("c:1:0:0")
            .expect("canonical id")
            .expect("present")
            .presence(),
        "Unavailable"
    );
    assert_eq!(
        root.lookup("c:0:0:0")
            .expect("canonical id")
            .expect("present")
            .presence(),
        "Ready"
    );
}

#[test]
fn successful_replacement_replay_equal_hashes() {
    let (root, before_hash, ready_digest) = sample_root();
    let known = known_entries(ready_digest);
    let next_a = payload(b"page-a");
    let next_b = payload(b"page-b");

    let mut unordered = HashMap::new();
    unordered.insert("c:2:2:2", ChunkSlot::not_loaded());
    unordered.insert("c:0:0:0", ChunkSlot::ready(next_a.clone()));

    let mut left = ChunkDeltaBuilder::new(&root);
    for (id, slot) in unordered {
        left.stage((id, slot)).expect("hashmap order stage");
    }
    let left_repl = left.freeze().expect("left freeze");

    let mut right = ChunkDeltaBuilder::new(&root);
    right
        .stage(StagedEdit::new("c:0:0:0", ChunkSlot::ready(next_a)).cells(["1:0:0", "0:0:0"]))
        .expect("canonical cell sort");
    right
        .stage(("c:2:2:2", ChunkSlot::not_loaded()))
        .expect("second chunk");
    let right_repl = right.freeze().expect("right freeze");

    assert_eq!(left_repl.digest(), right_repl.digest());
    assert_eq!(left_repl.set(), right_repl.set());
    assert_eq!(
        left_repl
            .set()
            .get("c:0:0:0")
            .expect("canonical id")
            .expect("replacement")
            .presence(),
        "Ready"
    );
    assert_eq!(
        right_repl
            .set()
            .get("c:2:2:2")
            .expect("canonical id")
            .expect("replacement")
            .presence(),
        "NotLoaded"
    );

    let mut other = ChunkDeltaBuilder::new(&root);
    other
        .stage(("c:0:0:0", ChunkSlot::ready(next_b)))
        .expect("different payload");
    let other_repl = other.freeze().expect("other freeze");
    assert_ne!(left_repl.digest(), other_repl.digest());

    assert_eq!(hash_root(&root, &known), before_hash);
    assert_eq!(
        root.lookup("c:2:2:2")
            .expect("canonical id")
            .expect("present")
            .presence(),
        "Pending"
    );
}

#[test]
fn dirty_frontier_newer_dirty_not_covered_by_older_ack() {
    assert!(SCHEMA_IDS.contains(&"voxel-durability-ack"));

    let frontier = DirtyFrontier::new("world-a", 7).expect("bound frontier");
    let first = frontier
        .record("c:0:0:0", 5, "AuthoritativeWrite")
        .expect("first dirty");
    assert!(
        frontier
            .latest_revision("c:0:0:0")
            .expect("canonical id")
            .is_none(),
        "record returns a new frontier"
    );
    assert_eq!(
        first.first_revision("c:0:0:0").expect("canonical id"),
        Some(5)
    );
    assert_eq!(
        first.latest_revision("c:0:0:0").expect("canonical id"),
        Some(5)
    );
    assert_eq!(
        first.reason("c:0:0:0").expect("canonical id"),
        Some("AuthoritativeWrite")
    );

    let newer = first
        .record("c:0:0:0", 9, "AuthoritativeWrite")
        .expect("later dirty");
    assert_eq!(
        newer.first_revision("c:0:0:0").expect("canonical id"),
        Some(5)
    );
    assert_eq!(
        newer.latest_revision("c:0:0:0").expect("canonical id"),
        Some(9)
    );
    assert_eq!(
        first.latest_revision("c:0:0:0").expect("canonical id"),
        Some(5),
        "old frontier object is unchanged"
    );

    let old_ack = ack("world-a", 7, 4, &[("c:0:0:0", 5)]);
    let old_cover = first.covered_by(&old_ack).expect("cut matches first dirty");
    assert!(old_cover.contains("c:0:0:0").expect("canonical id"));
    let newer_cover = newer
        .covered_by(&old_ack)
        .expect("older ack is still a valid cut");
    assert!(
        !newer_cover.contains("c:0:0:0").expect("canonical id"),
        "newer dirty must not be covered by an older ack revision"
    );

    let matching = ack("world-a", 7, 8, &[("c:0:0:0", 9)]);
    let covered = newer.covered_by(&matching).expect("ack covers latest");
    assert!(covered.contains("c:0:0:0").expect("canonical id"));

    let wrong_world = ack("world-b", 7, 8, &[("c:0:0:0", 9)]);
    let err = newer.covered_by(&wrong_world).unwrap_err();
    assert_eq!(err.error_id(), "SessionMismatch");
    assert_stable_error(err.error_id());

    let wrong_generation = ack("world-a", 8, 8, &[("c:0:0:0", 9)]);
    let err = newer.covered_by(&wrong_generation).unwrap_err();
    assert_eq!(err.error_id(), "StaleEpoch");
    assert_stable_error(err.error_id());

    let mut bad_kind = matching.clone();
    bad_kind.kind = "NotAnAck".to_string();
    let err = newer.covered_by(&bad_kind).unwrap_err();
    assert_eq!(err.error_id(), "InvalidHandle");
    assert_stable_error(err.error_id());
}

/// Strips Rust comments so the guard scans executable source, not prose.
///
/// The forbidden tokens below describe forbidden *code*. A doc comment that names a
/// token in order to state the rule is obeyed (dirty.rs: "Not named clear_dirty") is
/// documentation of compliance, not a violation; scanning raw file text made this
/// guard fire on its own success. String literals are preserved so a token that really
/// does appear in code is still caught.
///
/// Char literals are skipped explicitly: a bare `'"'` would otherwise flip the string
/// state and swallow everything up to the next quote, making the guard scan *less* than
/// it claims to. That fails open, which is the same class of silent-miss as the bug this
/// helper was written for. Raw strings (`r#"…"#`) are still not modelled — none of the
/// scanned files uses one; add handling here before that changes.
fn code_only(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let mut chars = src.chars().peekable();
    let mut in_line_comment = false;
    let mut block_depth = 0usize;
    let mut in_str = false;
    let mut escaped = false;
    while let Some(c) = chars.next() {
        if in_line_comment {
            if c == '\n' {
                in_line_comment = false;
                out.push(c);
            }
            continue;
        }
        if block_depth > 0 {
            if c == '/' && chars.peek() == Some(&'*') {
                chars.next();
                block_depth += 1;
            } else if c == '*' && chars.peek() == Some(&'/') {
                chars.next();
                block_depth -= 1;
            } else if c == '\n' {
                out.push(c);
            }
            continue;
        }
        if in_str {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_str = false;
            }
            out.push(c);
            continue;
        }
        match c {
            '\'' => {
                // Char literal or lifetime. Consume a `'x'` / `'\x'` literal wholesale so a
                // quote inside it cannot flip `in_str`; a lifetime just falls through.
                out.push(c);
                let mut probe = chars.clone();
                let first = probe.next();
                if first == Some('\\') {
                    probe.next();
                }
                if probe.next() == Some('\'') {
                    for _ in 0..(if first == Some('\\') { 3 } else { 2 }) {
                        if let Some(lit) = chars.next() {
                            out.push(lit);
                        }
                    }
                }
            }
            '"' => {
                in_str = true;
                out.push(c);
            }
            '/' if chars.peek() == Some(&'/') => {
                chars.next();
                in_line_comment = true;
            }
            '/' if chars.peek() == Some(&'*') => {
                chars.next();
                block_depth = 1;
            }
            _ => out.push(c),
        }
    }
    out
}

#[test]
fn chunk_delta_dirty_source_has_no_publish_clear_or_fs() {
    let sources = [
        include_str!("../src/chunk/delta.rs"),
        include_str!("../src/chunk/dirty.rs"),
        include_str!("../src/chunk/replacement.rs"),
    ];
    for src in sources {
        let src = code_only(src);
        assert!(!src.contains("std::fs"), "must not use std::fs");
        assert!(!src.contains("::File"), "must not use File");
        assert!(!src.contains("clear_dirty"), "must not clear dirty");
        assert!(!src.contains("fn publish"), "must not publish");
        assert!(
            !src.contains("lumio_voxel_world"),
            "must not reference world"
        );
        assert!(
            !src.contains("crate::revision"),
            "must not call revision as a service"
        );
        assert!(
            !src.contains("lumio_voxel_domain::revision"),
            "must not use the revision module"
        );
        assert!(!src.contains("chunk_size"), "no public chunk_size");
    }
}
