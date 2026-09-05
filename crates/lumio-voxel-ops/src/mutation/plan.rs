//! Canonical cell/section edit batch. Duplicate cells retain submission order so
//! the final entry is the authoritative last write.

#![forbid(unsafe_code)]

use super::fingerprint::{MutationEntry, MutationRequest};
use super::preconditions::MutationError;
use lumio_voxel_contracts::voxel_world as vw;
use lumio_voxel_domain::key::SectionId;
use std::collections::BTreeMap;

pub const MAX_WRITE_BATCH_ENTRIES: usize = vw::MAX_ENTRIES_PER_WRITE_BATCH as usize;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct SectionEdits {
    entries: Vec<MutationEntry>,
}

impl SectionEdits {
    pub(crate) fn entries(&self) -> &[MutationEntry] {
        &self.entries
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MutationPlan {
    sections: BTreeMap<String, SectionEdits>,
}

impl MutationPlan {
    pub(crate) fn section_ids(&self) -> impl Iterator<Item = &str> {
        self.sections.keys().map(String::as_str)
    }

    pub(crate) fn section_edits(&self) -> &BTreeMap<String, SectionEdits> {
        &self.sections
    }
}

pub struct MutationPlanner;

impl MutationPlanner {
    pub fn build(request: &MutationRequest) -> Result<MutationPlan, MutationError> {
        if request.txn_id.is_empty() || request.world_id.is_empty() {
            return Err(MutationError::invalid_handle());
        }
        let mut sections: BTreeMap<String, SectionEdits> = BTreeMap::new();
        if request.entries.len() > MAX_WRITE_BATCH_ENTRIES {
            return Err(MutationError::write_batch_too_large());
        }
        for entry in &request.entries {
            if entry.section_key.is_empty() {
                return Err(MutationError::unstructured_mutation_entry());
            }
            SectionId::parse(&entry.section_key)
                .map_err(|err| MutationError::from_section(err.into()))?;
            sections
                .entry(entry.section_key.clone())
                .or_default()
                .entries
                .push(entry.clone());
        }

        Ok(MutationPlan { sections })
    }
}
