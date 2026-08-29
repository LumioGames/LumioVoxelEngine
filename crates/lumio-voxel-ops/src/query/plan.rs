//! QueryPlanner binds max_chunks to an approved snapshot and captures one plan.

#![forbid(unsafe_code)]

use super::budget;
use super::validate::{GeneratedVoxelQueryRequest, canonicalize_chunks, validate_request};
use super::{QUERY_SCHEMA, QueryError, query_schema};
use crate::canonical::{CanonicalObject, CanonicalValue};
use lumio_voxel_contracts::{Hash256, sha256};
use lumio_voxel_domain::config_snapshot::VoxelConfigSnapshot;
use lumio_voxel_domain::publication::PublishedReadView;
use lumio_voxel_domain::revision::GeneratedRevisionStamp;
use std::sync::Arc;

pub struct QueryPlanner {
    snapshot: Arc<VoxelConfigSnapshot>,
    max_chunks: usize,
}

/// Captured deterministic plan. Fields are owned so later publish/reload cannot mutate it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QueryPlan {
    canonical_chunks: Vec<String>,
    read_stamp: GeneratedRevisionStamp,
    budget: usize,
    config_hash: String,
    cancel_token: String,
    plan_hash: Hash256,
}

impl QueryPlanner {
    /// Bind a planner to an approved snapshot. `max_chunks` is adapter-internal
    /// StrictAdmission capacity captured with this snapshot; there is no unbounded
    /// constructor and no generated Schema capacity column.
    pub fn from_approved_snapshot(
        snapshot: Arc<VoxelConfigSnapshot>,
        max_chunks: usize,
    ) -> Result<Self, QueryError> {
        let _ = query_schema();
        if max_chunks == 0 {
            return Err(QueryError::budget_exceeded());
        }
        Ok(Self {
            snapshot,
            max_chunks,
        })
    }

    pub fn config_hash(&self) -> &str {
        self.snapshot.config_hash()
    }

    pub fn plan(
        &self,
        request: &GeneratedVoxelQueryRequest,
        view: &PublishedReadView,
        config: &VoxelConfigSnapshot,
    ) -> Result<QueryPlan, QueryError> {
        let stamp = view.stamp();
        validate_request(request, stamp)?;
        if config.capabilities().is_empty() {
            return Err(QueryError::claim_not_granted());
        }
        if budget::exceeds(request.chunk_ids.len(), self.max_chunks) {
            return Err(QueryError::budget_exceeded());
        }
        let canonical_chunks = canonicalize_chunks(&request.chunk_ids)?;
        let read_stamp = stamp.clone();
        let config_hash = config.config_hash().to_string();
        let plan_hash = compute_plan_hash(
            request,
            &read_stamp,
            &canonical_chunks,
            &config_hash,
            self.max_chunks,
        )?;
        Ok(QueryPlan {
            canonical_chunks,
            read_stamp,
            budget: self.max_chunks,
            config_hash,
            cancel_token: request.query_id.clone(),
            plan_hash,
        })
    }
}

impl QueryPlan {
    pub fn canonical_chunks(&self) -> &[String] {
        &self.canonical_chunks
    }

    pub fn read_stamp(&self) -> &GeneratedRevisionStamp {
        &self.read_stamp
    }

    pub fn budget(&self) -> usize {
        self.budget
    }

    pub fn config_hash(&self) -> &str {
        &self.config_hash
    }

    pub fn cancel_token(&self) -> &str {
        &self.cancel_token
    }

    pub fn plan_hash(&self) -> Hash256 {
        self.plan_hash
    }
}

fn compute_plan_hash(
    request: &GeneratedVoxelQueryRequest,
    stamp: &GeneratedRevisionStamp,
    chunks: &[String],
    config_hash: &str,
    budget: usize,
) -> Result<Hash256, QueryError> {
    let mut object = CanonicalObject::new();
    let put = |object: &mut CanonicalObject, key: String, value: CanonicalValue| {
        object
            .insert(key, value)
            .map_err(|_| QueryError::invalid_handle())
    };
    put(
        &mut object,
        "schemaId".into(),
        CanonicalValue::text(QUERY_SCHEMA),
    )?;
    put(
        &mut object,
        "queryId".into(),
        CanonicalValue::text(&request.query_id),
    )?;
    put(
        &mut object,
        "worldId".into(),
        CanonicalValue::text(&request.world_id),
    )?;
    put(
        &mut object,
        "context".into(),
        CanonicalValue::text(&request.context),
    )?;
    put(
        &mut object,
        "canonicalChunks".into(),
        CanonicalValue::TextArray(chunks.to_vec()),
    )?;
    put(
        &mut object,
        "stampWorldId".into(),
        CanonicalValue::text(&stamp.world_id),
    )?;
    put(
        &mut object,
        "stampContextId".into(),
        CanonicalValue::text(&stamp.context_id),
    )?;
    put(
        &mut object,
        "generation".into(),
        CanonicalValue::Uint(stamp.generation),
    )?;
    put(
        &mut object,
        "worldRevision".into(),
        CanonicalValue::Uint(stamp.world_revision),
    )?;
    for (id, rev) in &stamp.chunk_revision_set {
        put(
            &mut object,
            format!("chunkRevision.{id}"),
            CanonicalValue::Uint(*rev),
        )?;
    }
    put(
        &mut object,
        "configHash".into(),
        CanonicalValue::text(config_hash),
    )?;
    put(
        &mut object,
        "maxChunks".into(),
        CanonicalValue::Uint(budget as u64),
    )?;
    Ok(Hash256(sha256(object.encode().as_bytes())))
}
