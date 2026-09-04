//! Deterministic query planner and adapter-internal budget admission (R-00080).
//!
//! Production code must not depend on `lumio-voxel-test-support`.
//! The planner does not read section payload, stream, or load.

#![forbid(unsafe_code)]

mod budget;
mod execute;
mod plan;
mod result_assembly;
mod section_access;
mod validate;

pub use budget::BUDGET_FAMILY;
pub use execute::QueryExecutor;
pub use plan::{QueryPlan, QueryPlanner};
pub use result_assembly::{GeneratedVoxelQueryOutcome, QueryEvidence};
pub use section_access::SectionAccessResult;
pub use validate::GeneratedVoxelQueryRequest;

use lumio_voxel_contracts::{SCHEMA_IDS, STABLE_ERROR_IDS};
use lumio_voxel_domain::key::KeyError;

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
