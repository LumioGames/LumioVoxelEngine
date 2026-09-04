//! Private staged replacements. freeze() does not mutate the bound root.

#![forbid(unsafe_code)]

use super::directory::SectionDirectoryRoot;
use super::replacement::{ReplacementSet, SectionReplacement};
use super::slot::SectionSlot;
use super::{SectionError, SectionId};
use std::collections::{BTreeMap, BTreeSet};

/// One unpublished section replacement. Cell ids are occupancy keys only.
#[derive(Clone, Debug)]
pub struct StagedEdit {
    section_id: String,
    cell_ids: Vec<String>,
    slot: SectionSlot,
}

impl StagedEdit {
    pub fn new(section_id: impl Into<String>, slot: SectionSlot) -> Self {
        Self {
            section_id: section_id.into(),
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

impl From<(&str, SectionSlot)> for StagedEdit {
    fn from((section_id, slot): (&str, SectionSlot)) -> Self {
        Self::new(section_id, slot)
    }
}

impl From<(String, SectionSlot)> for StagedEdit {
    fn from((section_id, slot): (String, SectionSlot)) -> Self {
        Self::new(section_id, slot)
    }
}

/// Mutates only this builder. The bound `SectionDirectoryRoot` stays immutable.
pub struct SectionDeltaBuilder {
    base: SectionDirectoryRoot,
    staged: BTreeMap<SectionId, SectionSlot>,
}

impl SectionDeltaBuilder {
    pub fn new(root: &SectionDirectoryRoot) -> Self {
        Self {
            base: root.clone(),
            staged: BTreeMap::new(),
        }
    }

    pub fn stage(&mut self, edit: impl Into<StagedEdit>) -> Result<(), SectionError> {
        let edit = edit.into();
        let id = SectionId::parse(&edit.section_id)?;
        let mut cells = BTreeSet::new();
        for cell in &edit.cell_ids {
            if cell.is_empty() || !cells.insert(cell.clone()) {
                return Err(SectionError::invalid_handle());
            }
        }
        if self.staged.contains_key(&id) {
            return Err(SectionError::invalid_handle());
        }
        self.staged.insert(id, edit.slot);
        Ok(())
    }

    pub fn freeze(self) -> Result<SectionReplacement, SectionError> {
        for (id, slot) in &self.staged {
            let canonical = id.key();
            if let Some(current) = self.base.lookup(&canonical)? {
                let payload = slot.payload().cloned();
                current.try_convert(slot.presence(), payload)?;
            }
        }
        Ok(SectionReplacement::from_slots(
            ReplacementSet::from_entries(self.staged),
        ))
    }
}
