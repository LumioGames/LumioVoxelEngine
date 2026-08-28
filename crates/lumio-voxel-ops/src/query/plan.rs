//! QueryPlanner binds max_chunks to an approved snapshot and captures one plan.

#![forbid(unsafe_code)]

use super::budget;
use super::validate::{GeneratedVoxelQueryRequest, canonicalize_chunks, validate_request};
use super::{QUERY_SCHEMA, QueryError, query_schema};
use lumio_voxel_contracts::{Hash256, canonical_object_pairs, sha256};
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
        );
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
) -> Hash256 {
    let mut pairs: Vec<(String, String)> = Vec::new();
    pairs.push(("schemaId".to_string(), quote(QUERY_SCHEMA)));
    pairs.push(("queryId".to_string(), quote(&request.query_id)));
    pairs.push(("worldId".to_string(), quote(&request.world_id)));
    pairs.push(("context".to_string(), quote(&request.context)));
    pairs.push((
        "canonicalChunks".to_string(),
        format!(
            "[{}]",
            chunks
                .iter()
                .map(|c| quote(c))
                .collect::<Vec<_>>()
                .join(",")
        ),
    ));
    pairs.push(("stampWorldId".to_string(), quote(&stamp.world_id)));
    pairs.push(("stampContextId".to_string(), quote(&stamp.context_id)));
    pairs.push(("generation".to_string(), stamp.generation.to_string()));
    pairs.push((
        "worldRevision".to_string(),
        stamp.world_revision.to_string(),
    ));
    for (id, rev) in &stamp.chunk_revision_set {
        pairs.push((format!("chunkRevision.{id}"), rev.to_string()));
    }
    pairs.push(("configHash".to_string(), quote(config_hash)));
    pairs.push(("maxChunks".to_string(), budget.to_string()));
    let canonical = canonical_object_pairs(&mut pairs);
    Hash256(sha256(canonical.as_bytes()))
}

fn quote(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len() + 2);
    out.push('"');
    out.push_str(raw);
    out.push('"');
    out
}
