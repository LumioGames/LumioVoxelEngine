//! Canonical cell/chunk edit batch. Duplicates fail with no partial plan.

#![forbid(unsafe_code)]

use super::fingerprint::MutationRequest;
use super::preconditions::MutationError;
use std::collections::BTreeMap;

/// Wraps generated `GeneratedRevisionStamp.world_revision`. Not a new Schema column.
pub(crate) const WORLD_REVISION_FIELD: &str = "world_revision";

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct ChunkEdits {
    chunk_value: Option<String>,
    cells: BTreeMap<String, String>,
}

impl ChunkEdits {
    pub(crate) fn payload_bytes(&self) -> Vec<u8> {
        if let Some(value) = &self.chunk_value {
            return value.as_bytes().to_vec();
        }
        self.cells
            .values()
            .flat_map(|value| value.as_bytes())
            .copied()
            .collect()
    }

    pub(crate) fn cell_ids(&self) -> impl Iterator<Item = &str> {
        self.cells.keys().map(String::as_str)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MutationPlan {
    expected_world_revision: u64,
    chunks: BTreeMap<String, ChunkEdits>,
}

impl MutationPlan {
    pub fn expected_world_revision(&self) -> u64 {
        self.expected_world_revision
    }

    pub(crate) fn chunk_ids(&self) -> impl Iterator<Item = &str> {
        self.chunks.keys().map(String::as_str)
    }

    pub(crate) fn chunk_edits(&self) -> &BTreeMap<String, ChunkEdits> {
        &self.chunks
    }
}

pub struct MutationPlanner;

impl MutationPlanner {
    pub fn build(request: &MutationRequest) -> Result<MutationPlan, MutationError> {
        if request.txn_id.is_empty() || request.world_id.is_empty() {
            return Err(MutationError::invalid_handle());
        }
        let expected_world_revision = match request.fields.get(WORLD_REVISION_FIELD) {
            Some(raw) => raw
                .parse::<u64>()
                .map_err(|_| MutationError::invalid_handle())?,
            None => return Err(MutationError::invalid_handle()),
        };

        let mut chunks: BTreeMap<String, ChunkEdits> = BTreeMap::new();
        for (key, value) in &request.fields {
            if key == WORLD_REVISION_FIELD {
                continue;
            }
            // Occupancy keys wrap canonical `voxelChunkId` (`c:x:y:z`), not Schema fields.
            if !key.starts_with("c:") {
                continue;
            }
            let (chunk_id, cell_id) = match key.split_once('/') {
                Some((chunk, cell)) => {
                    if cell.is_empty() {
                        return Err(MutationError::invalid_handle());
                    }
                    (chunk, Some(cell))
                }
                None => (key.as_str(), None),
            };
            let entry = chunks.entry(chunk_id.to_string()).or_default();
            match cell_id {
                None => {
                    if entry.chunk_value.is_some() {
                        return Err(MutationError::invalid_handle());
                    }
                    entry.chunk_value = Some(value.clone());
                }
                Some(cell) => {
                    if entry
                        .cells
                        .insert(cell.to_string(), value.clone())
                        .is_some()
                    {
                        return Err(MutationError::invalid_handle());
                    }
                }
            }
        }

        Ok(MutationPlan {
            expected_world_revision,
            chunks,
        })
    }
}
