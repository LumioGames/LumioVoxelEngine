//! Canonical cell/section edit batch. Duplicates fail with no partial plan.

#![forbid(unsafe_code)]

use super::fingerprint::MutationRequest;
use super::preconditions::MutationError;
use std::collections::BTreeMap;

/// Wraps generated `GeneratedRevisionStamp.world_revision`. Not a new Schema column.
pub(crate) const WORLD_REVISION_FIELD: &str = "world_revision";

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct SectionEdits {
    section_value: Option<String>,
    cells: BTreeMap<String, String>,
}

impl SectionEdits {
    pub(crate) fn payload_bytes(&self) -> Vec<u8> {
        if let Some(value) = &self.section_value {
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
    sections: BTreeMap<String, SectionEdits>,
}

impl MutationPlan {
    pub fn expected_world_revision(&self) -> u64 {
        self.expected_world_revision
    }

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
        let expected_world_revision = match request.fields.get(WORLD_REVISION_FIELD) {
            Some(raw) => raw
                .parse::<u64>()
                .map_err(|_| MutationError::invalid_handle())?,
            None => return Err(MutationError::invalid_handle()),
        };

        let mut sections: BTreeMap<String, SectionEdits> = BTreeMap::new();
        for (key, value) in &request.fields {
            if key == WORLD_REVISION_FIELD {
                continue;
            }
            // Occupancy keys wrap canonical `voxelSectionId` (`c:x:y:z`), not Schema fields.
            if !key.starts_with("s:") {
                continue;
            }
            let (section_id, cell_id) = match key.split_once('/') {
                Some((section, cell)) => {
                    if cell.is_empty() {
                        return Err(MutationError::invalid_handle());
                    }
                    (section, Some(cell))
                }
                None => (key.as_str(), None),
            };
            let entry = sections.entry(section_id.to_string()).or_default();
            match cell_id {
                None => {
                    if entry.section_value.is_some() {
                        return Err(MutationError::invalid_handle());
                    }
                    entry.section_value = Some(value.clone());
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
            sections,
        })
    }
}
