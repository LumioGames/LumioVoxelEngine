//! Request admission and canonical `voxelChunkId` (`c:x:y:z`) checks.

#![forbid(unsafe_code)]

use super::QueryError;
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
    /// Generated chunk-id list (`voxelChunkId`).
    pub chunk_ids: Vec<String>,
    /// Optional cancel-before-plan flag.
    pub cancel: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct CanonicalChunkId {
    x: i32,
    y: i32,
    z: i32,
}

impl CanonicalChunkId {
    fn parse(raw: &str) -> Result<Self, QueryError> {
        let mut parts = raw.split(':');
        let prefix = parts
            .next()
            .ok_or_else(QueryError::coordinate_out_of_bounds)?;
        if prefix != "c" {
            return Err(QueryError::coordinate_out_of_bounds());
        }
        let x = parse_coord(
            parts
                .next()
                .ok_or_else(QueryError::coordinate_out_of_bounds)?,
        )?;
        let y = parse_coord(
            parts
                .next()
                .ok_or_else(QueryError::coordinate_out_of_bounds)?,
        )?;
        let z = parse_coord(
            parts
                .next()
                .ok_or_else(QueryError::coordinate_out_of_bounds)?,
        )?;
        if parts.next().is_some() {
            return Err(QueryError::coordinate_out_of_bounds());
        }
        Ok(Self { x, y, z })
    }

    fn canonical(self) -> String {
        format!("c:{}:{}:{}", self.x, self.y, self.z)
    }
}

fn parse_coord(raw: &str) -> Result<i32, QueryError> {
    if raw.is_empty() {
        return Err(QueryError::coordinate_out_of_bounds());
    }
    let digits = match raw.strip_prefix('-') {
        Some(rest) => rest,
        None => raw,
    };
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return Err(QueryError::coordinate_out_of_bounds());
    }
    if digits.len() > 1 && digits.as_bytes()[0] == b'0' {
        return Err(QueryError::coordinate_out_of_bounds());
    }
    if raw.starts_with('-') && digits == "0" {
        return Err(QueryError::coordinate_out_of_bounds());
    }
    raw.parse::<i32>()
        .map_err(|_| QueryError::coordinate_out_of_bounds())
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

pub(super) fn canonicalize_chunks(ids: &[String]) -> Result<Vec<String>, QueryError> {
    let mut set = BTreeSet::new();
    for raw in ids {
        set.insert(CanonicalChunkId::parse(raw)?);
    }
    Ok(set.into_iter().map(CanonicalChunkId::canonical).collect())
}
