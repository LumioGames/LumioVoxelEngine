//! Private staged replacements. freeze() does not mutate the bound root.

#![forbid(unsafe_code)]

use super::directory::ChunkDirectoryRoot;
use super::replacement::{ChunkReplacement, ReplacementSet};
use super::slot::ChunkSlot;
use super::{ChunkError, ChunkId};
use std::collections::{BTreeMap, BTreeSet};

/// One unpublished chunk replacement. Cell ids are occupancy keys only.
#[derive(Clone, Debug)]
pub struct StagedEdit {
    chunk_id: String,
    cell_ids: Vec<String>,
    slot: ChunkSlot,
}

impl StagedEdit {
    pub fn new(chunk_id: impl Into<String>, slot: ChunkSlot) -> Self {
        Self {
            chunk_id: chunk_id.into(),
            cell_ids: Vec::new(),
            slot,
        }
    }

    pub fn cells<I, S>(mut self, cell_ids: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.cell_ids = cell_ids.into_iter().map(Into::into).collect();
        self
    }
}

impl From<(&str, ChunkSlot)> for StagedEdit {
    fn from((chunk_id, slot): (&str, ChunkSlot)) -> Self {
        Self::new(chunk_id, slot)
    }
}

impl From<(String, ChunkSlot)> for StagedEdit {
    fn from((chunk_id, slot): (String, ChunkSlot)) -> Self {
        Self::new(chunk_id, slot)
    }
}

/// Mutates only this builder. The bound `ChunkDirectoryRoot` stays immutable.
pub struct ChunkDeltaBuilder {
    base: ChunkDirectoryRoot,
    staged: BTreeMap<ChunkId, ChunkSlot>,
}

impl ChunkDeltaBuilder {
    pub fn new(root: &ChunkDirectoryRoot) -> Self {
        Self {
            base: root.clone(),
            staged: BTreeMap::new(),
        }
    }

    pub fn stage(&mut self, edit: impl Into<StagedEdit>) -> Result<(), ChunkError> {
        let edit = edit.into();
        let id = ChunkId::parse(&edit.chunk_id)?;
        let mut cells = BTreeSet::new();
        for cell in &edit.cell_ids {
            if cell.is_empty() || !cells.insert(cell.clone()) {
                return Err(ChunkError::invalid_handle());
            }
        }
        if self.staged.contains_key(&id) {
            return Err(ChunkError::invalid_handle());
        }
        self.staged.insert(id, edit.slot);
        Ok(())
    }

    pub fn freeze(self) -> Result<ChunkReplacement, ChunkError> {
        for (id, slot) in &self.staged {
            let canonical = canonical_chunk_id(*id);
            if let Some(current) = self.base.lookup(&canonical)? {
                let payload = slot.payload().cloned();
                current.try_convert(slot.presence(), payload)?;
            }
        }
        Ok(ChunkReplacement::from_slots(ReplacementSet::from_entries(
            self.staged,
        )))
    }
}

fn canonical_chunk_id(id: ChunkId) -> String {
    format!("c:{}:{}:{}", id.x, id.y, id.z)
}
