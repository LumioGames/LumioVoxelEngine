//! CLI: canonical decode cost against manifest size.
//!
//! `RestorePreflight::validate` takes bytes the Host read back off the filesystem
//! (VOX-D-008 puts DAG orchestration, fsync and the Active-pointer swap on the
//! Host), so decode cost is a property of untrusted input length, not of a
//! trusted in-memory object. This harness reports that cost so a super-linear
//! decoder cannot come back unnoticed.
//!
//! Run: `cargo run --release -p lumio-voxel-test-support --example bench-canonical-decode`
//! Optional args: section counts, e.g. `... -- 1000 4000 16000`.

use lumio_voxel_ops::canonical::{CanonicalObject, CanonicalValue};
use lumio_voxel_ops::snapshot::decode_canonical_object;
use std::env;
use std::time::Instant;

/// The snapshot manifest shape `ManifestAdapter::object` builds: the fixed header
/// members plus one `sectionRevision.<id>` per section.
fn manifest(sections: usize) -> CanonicalObject {
    let mut object = CanonicalObject::new();
    let members: Vec<(String, CanonicalValue)> = vec![
        (
            "schemaId".into(),
            CanonicalValue::text("voxel-snapshot-payload"),
        ),
        (
            "headerSchemaId".into(),
            CanonicalValue::text("snapshot-header"),
        ),
        ("magic".into(), CanonicalValue::text("LUMIOSNP1")),
        ("schemaEpoch".into(), CanonicalValue::Uint(1)),
        ("worldId".into(), CanonicalValue::text("world-a")),
        ("contextId".into(), CanonicalValue::text("ctx-1")),
        ("generation".into(), CanonicalValue::Uint(1)),
        ("worldRevision".into(), CanonicalValue::Uint(0)),
        ("configHash".into(), CanonicalValue::text("a".repeat(64))),
        ("rootIdentity".into(), CanonicalValue::text("b".repeat(64))),
    ];
    for (key, value) in members {
        object.insert(key, value).expect("distinct header members");
    }
    for i in 0..sections {
        object
            .insert(
                format!("sectionRevision.c:{i}:0:0"),
                CanonicalValue::Uint(1),
            )
            .expect("distinct section members");
    }
    object
}

/// Wall clock for the fastest of `runs` decodes, in microseconds. The minimum is
/// the least noisy summary of a deterministic, allocation-bound routine.
fn best_micros(bytes: &[u8], runs: u32) -> u128 {
    let mut best = u128::MAX;
    for _ in 0..runs {
        let start = Instant::now();
        let decoded = decode_canonical_object(bytes).expect("manifest decodes");
        let elapsed = start.elapsed().as_micros();
        // Keep the result observable so the decode cannot be optimised away.
        assert!(!decoded.is_empty());
        best = best.min(elapsed);
    }
    best
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let counts: Vec<usize> = if args.is_empty() {
        vec![1_000, 4_000, 16_000]
    } else {
        args.iter()
            .map(|a| a.parse().expect("section count must be a number"))
            .collect()
    };

    println!(
        "{:>8}  {:>12}  {:>14}  {:>8}",
        "sections", "bytes", "decode µs", "µs/KB"
    );
    let mut first: Option<(f64, f64)> = None;
    for sections in counts {
        let bytes = manifest(sections).encode_bytes();
        let micros = best_micros(&bytes, 3);
        let kb = bytes.len() as f64 / 1024.0;
        println!(
            "{sections:>8}  {:>12}  {micros:>14}  {:>8.1}",
            bytes.len(),
            micros as f64 / kb
        );
        // Linear decode holds µs/KB flat as the input grows; quadratic decode
        // multiplies it by the same factor the input grew by.
        let point = (kb, micros as f64);
        match first {
            None => first = Some(point),
            Some((kb0, us0)) => {
                let size_ratio = kb / kb0;
                let time_ratio = point.1 / us0;
                println!(
                    "          size ×{size_ratio:.1} vs first, time ×{time_ratio:.1} \
                     (linear ≈ ×{size_ratio:.1}, quadratic ≈ ×{:.1})",
                    size_ratio * size_ratio
                );
            }
        }
    }
}
