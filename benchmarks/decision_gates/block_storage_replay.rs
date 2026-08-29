//! VOX-D-002 seam replay driver (2026-08-29 post-SHA-256-fix retest).
//!
//! `block_storage.rs` is the gate-exclusive seam whose SHA-256 is recorded in
//! `docs/evidence/decision-gates/VOX-D-002-block-storage.md`; it stays
//! byte-untouched so seam drift stays machine-refutable. This driver only adds
//! the `--test` entry points `run_seam_replay.sh` needs to execute the seam's
//! own three-run replay against the corrected contract-runtime SHA-256.
//!
//! Run: `benchmarks/decision_gates/run_seam_replay.sh block_storage_replay`

#![forbid(unsafe_code)]

#[allow(dead_code)]
#[path = "block_storage.rs"]
mod block_storage;

#[cfg(test)]
mod tests {
    use super::block_storage as seam;

    fn hex32(bytes: &[u8; 32]) -> String {
        let mut out = String::with_capacity(64);
        for byte in bytes {
            out.push_str(&format!("{byte:02x}"));
        }
        out
    }

    #[test]
    fn declaration_matches_recorded_approval() {
        assert_eq!(seam::gate_id(), "VOX-D-002");
        assert_eq!(seam::card_id(), "R-00058");
        assert_eq!(seam::approval_status(), "approved");
        assert_eq!(seam::approval_reference(), "LGE-V1.4-VOX-D-P0-2026-08-28");
        assert_eq!(seam::selected_family(), "DenseUncompressedAdapter");
        assert_eq!(seam::selected_backend(), None);
    }

    #[test]
    fn three_runs_are_byte_identical() {
        let schedule = seam::corpus_schedule();
        assert_eq!(schedule.seed, seam::SCHEDULE_SEED);
        assert_eq!(schedule.ops.len(), 9);
        let run = seam::replay_three();
        assert!(run.traces_eq);
        assert!(run.snapshots_eq);
        eprintln!("VOX-D-002 seed {:#010x}", seam::SCHEDULE_SEED);
        eprintln!("VOX-D-002 op_count {}", schedule.ops.len());
        for (i, snapshot) in run.snapshots.iter().enumerate() {
            eprintln!("VOX-D-002 run{} snapshot {}", i + 1, hex32(snapshot));
        }
    }

    #[test]
    fn fault_matrix_stays_unrecoverable() {
        for row in seam::drive_fault_matrix() {
            eprintln!(
                "VOX-D-002 fault {} point={:?} error={:?} recoverable={}",
                row.label, row.point, row.error, row.recoverable
            );
            assert!(row.error.is_some());
            assert!(!row.recoverable);
        }
        assert!(seam::visible_write_faults_unrecoverable());
    }
}
