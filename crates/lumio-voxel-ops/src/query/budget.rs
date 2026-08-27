//! StrictAdmissionBudgetFamily: hard batch admission, no Schema capacity column.

#![forbid(unsafe_code)]

/// Adapter-internal family selected with the approved snapshot. Not a Schema field.
pub const BUDGET_FAMILY: &str = "StrictAdmissionBudgetFamily";

pub(super) fn exceeds(requested: usize, max_chunks: usize) -> bool {
    max_chunks == 0 || requested > max_chunks
}
