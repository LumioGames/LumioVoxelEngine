//! Request admission and canonical Section id (`s:<x>:<y>:<z>`) checks.
//!
//! 键的语法与定义域只有一份实现:`lumio_voxel_domain::key::SectionId`。这里不再复写
//! 一份解析器——两份解析器必然漂移,而键语法是契约面。

#![forbid(unsafe_code)]

use super::QueryError;
use lumio_voxel_domain::key::SectionId;
use lumio_voxel_domain::revision::GeneratedRevisionStamp;
use std::collections::BTreeSet;

/// Generated `voxel-query` request. Field names wrap `queryId`, `worldId`, `context`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GeneratedVoxelQueryRequest {
    /// Generated field `queryId`.
    pub query_id: String,
    /// Generated field `worldId`.
    pub world_id: String,
    /// Generated field `context`.
    pub context: String,
    /// Generated section-id list (`voxelSectionId`).
    pub section_ids: Vec<String>,
    /// Optional cancel-before-plan flag.
    pub cancel: bool,
}

pub(super) fn validate_request(
    request: &GeneratedVoxelQueryRequest,
    stamp: &GeneratedRevisionStamp,
) -> Result<(), QueryError> {
    if request.cancel {
        return Err(QueryError::invalid_handle());
    }
    if request.query_id.is_empty() || request.world_id.is_empty() || request.context.is_empty() {
        return Err(QueryError::invalid_handle());
    }
    if request.world_id != stamp.world_id || request.context != stamp.context_id {
        return Err(QueryError::invalid_handle());
    }
    Ok(())
}

pub(super) fn canonicalize_sections(ids: &[String]) -> Result<Vec<String>, QueryError> {
    let mut set = BTreeSet::new();
    for raw in ids {
        set.insert(SectionId::parse(raw).map_err(QueryError::from_key)?);
    }
    Ok(set.into_iter().map(|id| id.key()).collect())
}
