//! VOX-D-001 seam replay driver (2026-08-29 post-SHA-256-fix retest).
//!
//! `chunk_profile.rs` is the gate-exclusive seam whose SHA-256 is recorded in
//! `docs/evidence/decision-gates/VOX-D-001-chunk-profile.md`; it stays
//! byte-untouched so seam drift stays machine-refutable. This driver only adds
//! the `--test` entry points `run_seam_replay.sh` needs to execute the seam's
//! own `measure()` against the corrected contract-runtime SHA-256.
//!
//! Run: `benchmarks/decision_gates/run_seam_replay.sh chunk_profile_replay`

#![forbid(unsafe_code)]

#[allow(dead_code)]
#[path = "chunk_profile.rs"]
mod chunk_profile;

#[cfg(test)]
mod tests {
    use super::chunk_profile as seam;

    fn hex32(bytes: &[u8; 32]) -> String {
        let mut out = String::with_capacity(64);
        for byte in bytes {
            out.push_str(&format!("{byte:02x}"));
        }
        out
    }

    #[test]
    fn declaration_matches_recorded_approval() {
        assert_eq!(seam::gate_id(), "VOX-D-001");
        assert_eq!(seam::card_id(), "R-00057");
        assert_eq!(seam::approval_status(), "approved");
        assert_eq!(seam::approval_reference(), "LGE-V1.4-VOX-D-P0-2026-08-28");
        assert_eq!(seam::selected_family(), "IsolatedCubicExtentFamily");
        assert_eq!(
            seam::candidate_names(),
            &["IsolatedCubicExtentFamily", "CoupledPageAxisExtentFamily"]
        );
    }

    #[test]
    fn three_repeats_are_byte_identical() {
        let report = seam::measure();
        assert_eq!(report.seed, seam::MEASURE_SEED);
        assert_eq!(report.repeats, 3);
        assert!(report.traces_byte_identical);
        assert_eq!(report.op_count, 10);
        assert_eq!(report.corpus, seam::corpus_names());
        assert_eq!(report.selected_candidate, None);
        eprintln!("VOX-D-001 seed {:#018x}", report.seed);
        eprintln!("VOX-D-001 op_count {}", report.op_count);
        eprintln!(
            "VOX-D-001 traces_byte_identical {}",
            report.traces_byte_identical
        );
        eprintln!("VOX-D-001 snapshot {}", hex32(&report.snapshot));
    }

    #[test]
    fn negative_matrix_matches_shipped_injector() {
        let report = seam::measure();
        assert_eq!(report.negative.len(), 5);
        assert!(report.visible_writes_unrecoverable);
        for row in &report.negative {
            eprintln!(
                "VOX-D-001 negative {} point={} error={} recoverable={} visible_write={} matches_injector={}",
                row.scenario,
                row.fault_point,
                row.error_id,
                row.recoverable,
                row.visible_write,
                row.outcome_matches_injector
            );
            assert!(row.outcome_matches_injector);
            if row.visible_write {
                assert!(!row.recoverable);
            }
        }
    }
}
