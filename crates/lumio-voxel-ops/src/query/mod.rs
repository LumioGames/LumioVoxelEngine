//! Deterministic query planner and adapter-internal budget admission (R-00080).
//!
//! Production code must not depend on `lumio-voxel-test-support`.
//! The planner does not read section payload, stream, or load.

#![forbid(unsafe_code)]

mod block_read;
mod budget;
mod execute;
mod plan;
mod result_assembly;
mod section_access;
mod validate;

pub use block_read::{
    BlockReadResult, BlockReadSection, BlockReadWorld, BufferedBlockReadResult, BufferedReadCell,
    BufferedReadSegment, CellReadResult, ColumnYRange, MAX_CELLS_PER_READ_REQUEST, ReadCell,
    ReadSegment, read_box, read_box_into, read_cell, read_cell_into, read_column, read_column_into,
};
pub use budget::BUDGET_FAMILY;
pub use execute::QueryExecutor;
pub use plan::{QueryPlan, QueryPlanner};
pub use result_assembly::{GeneratedVoxelQueryOutcome, QueryEvidence};
pub use section_access::SectionAccessResult;
pub use validate::GeneratedVoxelQueryRequest;

use lumio_voxel_contracts::voxel_world as vw;
use lumio_voxel_contracts::{SCHEMA_IDS, STABLE_ERROR_IDS};
use lumio_voxel_domain::key::{KeyError, WorldYError};
pub use lumio_voxel_domain::section::SectionPresenceGuard;

/// Generated schema this mapping wraps. Must stay in `SCHEMA_IDS`.
pub const QUERY_SCHEMA: &str = "voxel-query";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QueryError {
    error_id: &'static str,
}

impl QueryError {
    pub fn error_id(&self) -> &'static str {
        self.error_id
    }

    pub(super) fn budget_exceeded() -> Self {
        Self {
            error_id: stable("BudgetExceeded"),
        }
    }

    /// 直接透传键解析给出的契约错误码。
    pub(super) fn from_key(err: KeyError) -> Self {
        Self {
            error_id: err.error_id(),
        }
    }

    pub(super) fn invalid_handle() -> Self {
        Self {
            error_id: stable("InvalidHandle"),
        }
    }

    pub(super) fn claim_not_granted() -> Self {
        Self {
            error_id: stable("ClaimNotGranted"),
        }
    }

    pub(super) fn loader_cancelled() -> Self {
        Self {
            error_id: stable("LoaderCancelled"),
        }
    }

    pub fn contract(id: &'static str) -> Self {
        Self {
            error_id: vw::intern_error_code(id)
                .expect("mapped query error must exist in the voxel-world contract"),
        }
    }

    pub(super) fn from_world_y(_err: WorldYError) -> Self {
        Self::contract(vw::WORLD_Y_OUT_OF_RANGE)
    }

    pub fn pinned_read_returned_pending() -> Self {
        Self::contract("pinned_read_returned_pending")
    }
}

impl std::fmt::Display for QueryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.error_id)
    }
}

impl std::error::Error for QueryError {}

fn stable(id: &'static str) -> &'static str {
    STABLE_ERROR_IDS
        .iter()
        .copied()
        .find(|candidate| *candidate == id)
        .expect("mapped error id must exist in generated STABLE_ERROR_IDS")
}

fn query_schema() -> &'static str {
    SCHEMA_IDS
        .iter()
        .copied()
        .find(|id| *id == QUERY_SCHEMA)
        .expect("voxel-query must exist in generated SCHEMA_IDS")
}
