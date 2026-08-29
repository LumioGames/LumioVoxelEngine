//! VOX-D-003 seam replay driver (2026-08-29 post-SHA-256-fix retest).
//!
//! `query_budget.rs` is the gate-exclusive seam whose SHA-256 is recorded in
//! `docs/evidence/decision-gates/VOX-D-003-query-budget.md`; it stays
//! byte-untouched so seam drift stays machine-refutable. This driver only adds
//! the `--test` entry points `run_seam_replay.sh` needs to execute the seam's
//! own corpus replay against the corrected contract-runtime SHA-256.
//!
//! The 2026-08-28 evidence recorded one snapshot digest computed *outside*
//! the harness (Node `crypto.createHash('sha256')` over the committed-op
//! encoding) because that host could not link a binary. This driver replays
//! the same schedule through the actually linked harness so the recorded
//! value can be confirmed or refuted against the fixed `sha256`.
//!
//! Run: `benchmarks/decision_gates/run_seam_replay.sh query_budget_replay`

#![forbid(unsafe_code)]

#[allow(dead_code)]
#[path = "query_budget.rs"]
mod query_budget;

#[cfg(test)]
mod tests {
    use super::query_budget as seam;

    /// Snapshot digest recorded in the 2026-08-28 evidence (§4.1), computed
    /// with Node crypto over the committed-op encoding, not by the (then
    /// defective) generated contract runtime.
    const RECORDED_SNAPSHOT_2026_08_28: &str =
        "bf6aadfa5375cdaead9ec5dc65a7f2d3ad7b43063775c9b74ab77b38d9150029";

    #[test]
    fn declaration_matches_recorded_approval() {
        assert_eq!(seam::gate_id(), "VOX-D-003");
        assert_eq!(seam::card_id(), "R-00059");
        assert_eq!(seam::approval_status(), "approved");
        assert_eq!(seam::approval_reference(), "LGE-V1.4-VOX-D-P0-2026-08-28");
        assert_eq!(seam::selected_family(), "StrictAdmissionBudgetFamily");
        assert!(seam::query_schema_registered());
        assert!(seam::mapped_fault_error_ids_are_stable());
    }

    #[test]
    fn three_runs_reproduce_the_recorded_snapshot() {
        let schedule = seam::corpus_schedule();
        assert_eq!(schedule.seed, seam::CORPUS_SEED);
        let committed_len: usize = schedule
            .ops
            .iter()
            .map(|op| 8 + op.schema_id.len() + op.payload.len())
            .sum();
        let m = seam::measure_corpus();
        assert_eq!(m.seed, seam::CORPUS_SEED);
        assert!(m.identical);
        assert_eq!(m.outcome_count, 7);
        assert_eq!(m.errors, 0);
        eprintln!("VOX-D-003 seed {:#010x}", m.seed);
        eprintln!("VOX-D-003 op_count {}", m.outcome_count);
        eprintln!("VOX-D-003 committed_encoding_bytes {committed_len}");
        for (i, snapshot) in m.snapshots.iter().enumerate() {
            eprintln!("VOX-D-003 run{} snapshot {}", i + 1, seam::hex32(snapshot));
        }
        eprintln!("VOX-D-003 recorded-2026-08-28 snapshot {RECORDED_SNAPSHOT_2026_08_28}");
        let matches_recorded = seam::hex32(&m.snapshots[0]) == RECORDED_SNAPSHOT_2026_08_28;
        eprintln!("VOX-D-003 matches_recorded_value {matches_recorded}");
        // Confirmed on 2026-08-29: the linked harness reproduces the recorded
        // value bit-for-bit, so it is asserted from here on.
        assert!(matches_recorded);
    }

    #[test]
    fn fault_matrix_stays_on_stable_error_ids() {
        for row in seam::measure_faults() {
            eprintln!(
                "VOX-D-003 fault {} error={:?} recoverable={} stable={}",
                row.scenario, row.error, row.recoverable, row.error_is_stable
            );
            assert!(row.error_is_stable);
        }
    }
}
