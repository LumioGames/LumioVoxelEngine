//! VOX-D-004 seam replay driver (2026-08-29 post-SHA-256-fix retest).
//!
//! `reservation_receipt.rs` is the gate-exclusive seam whose SHA-256 is
//! recorded in `docs/evidence/decision-gates/VOX-D-004-reservation-receipt.md`;
//! it stays byte-untouched so seam drift stays machine-refutable. This driver
//! only adds the `--test` entry points `run_seam_replay.sh` needs to execute
//! the seam's own three-repeat replay against the corrected contract-runtime
//! SHA-256. The 2026-08-28 evidence recorded no trace hashes (`traceHashes:
//! []`, host could not link); the values printed here are the first executed
//! ones.
//!
//! Run: `benchmarks/decision_gates/run_seam_replay.sh reservation_receipt_replay`

#![forbid(unsafe_code)]

#[allow(dead_code)]
#[path = "reservation_receipt.rs"]
mod reservation_receipt;

#[cfg(test)]
mod tests {
    use super::reservation_receipt as seam;

    #[test]
    fn declaration_matches_recorded_approval() {
        assert_eq!(seam::gate_id(), "VOX-D-004");
        assert_eq!(seam::card_id(), "R-00060");
        assert_eq!(seam::approval_status(), "approved");
        assert_eq!(seam::approval_reference(), "LGE-V1.4-VOX-D-P0-2026-08-28");
        assert_eq!(seam::selected_family(), "GenerationBoundLeaseFamily");
        assert_eq!(seam::selected_lease(), None);
        assert_eq!(seam::selected_capacity(), None);
        assert_eq!(seam::selected_prune_rule(), None);
    }

    #[test]
    fn three_repeats_are_byte_identical() {
        let report = seam::run_three_repeats();
        assert_eq!(report.seed, seam::MEASUREMENT_SEED);
        assert!(report.byte_identical);
        eprintln!("VOX-D-004 seed {:#010x}", report.seed);
        eprintln!("VOX-D-004 op_count {}", seam::receipt_corpus().len());
        eprintln!(
            "VOX-D-004 encoded_trace_bytes {}",
            seam::encode_trace(&report.traces[0]).len()
        );
        for i in 0..seam::REPEAT_COUNT {
            eprintln!(
                "VOX-D-004 run{} trace_hash {}",
                i + 1,
                seam::hex32(&report.trace_hashes[i])
            );
            eprintln!(
                "VOX-D-004 run{} snapshot {}",
                i + 1,
                seam::hex32(&report.snapshot_hashes[i])
            );
        }
    }

    #[test]
    fn fault_matrix_matches_shipped_injector() {
        let rows = seam::run_fault_matrix();
        assert_eq!(rows.len(), 3);
        for row in rows {
            eprintln!(
                "VOX-D-004 fault {} point={:?} error={} recoverable={}",
                row.name, row.point, row.error_id, row.recoverable
            );
        }
        let recoverable: Vec<bool> = rows.iter().map(|r| r.recoverable).collect();
        assert_eq!(recoverable, [false, false, true]);
    }
}
