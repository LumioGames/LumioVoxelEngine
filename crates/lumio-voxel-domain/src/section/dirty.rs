//! Revision-aware dirty frontier. `covered_by` is a pure function.

#![forbid(unsafe_code)]

use super::SectionId;
use crate::key::KeyRejection;
use lumio_voxel_contracts::voxel_world as vw;
use lumio_voxel_contracts::{SCHEMA_IDS, STABLE_ERROR_IDS};
use std::collections::BTreeMap;

const ACK_SCHEMA: &str = "voxel-durability-ack";
const ACK_KIND: &str = "DurabilityAck";

/// Schema `common.schema.json#/$defs/revision` integer (min 0). Not an allocator type.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct SchemaRevision(u64);

#[derive(Clone, Debug, PartialEq, Eq)]
struct DirtyEntry {
    first: SchemaRevision,
    latest: SchemaRevision,
    reason: String,
}

/// Generated `voxel-durability-ack` field `context`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DurabilityAckContext {
    /// Schema field `contextId`.
    pub context_id: String,
    /// Schema field `generation`.
    pub generation: u64,
}

/// Generated `coveredSections[]` element.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CoveredSectionAck {
    /// Schema field `sectionId`.
    pub section_id: String,
    /// Schema field `upToSectionRevision`.
    pub up_to_section_revision: u64,
}

/// Evidence shape for schema `voxel-durability-ack`. Kind is `DurabilityAck`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DurabilityAckEvidence {
    /// Schema field `kind`.
    pub kind: String,
    /// Schema field `worldId`.
    pub world_id: String,
    /// Schema field `context`.
    pub context: DurabilityAckContext,
    /// Schema field `coveredWorldRevision`.
    pub covered_world_revision: u64,
    /// Schema field `coveredSections`.
    pub covered_sections: Vec<CoveredSectionAck>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DirtyCoverage {
    covered: BTreeMap<SectionId, SchemaRevision>,
}

impl DirtyCoverage {
    pub fn contains(&self, section_id: &str) -> Result<bool, DirtyError> {
        let id = parse_section(section_id)?;
        Ok(self.covered.contains_key(&id))
    }

    pub fn len(&self) -> usize {
        self.covered.len()
    }

    pub fn is_empty(&self) -> bool {
        self.covered.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DirtyError {
    /// 引擎通用:活契约不定义,沿用废弃镜像的稳定 id。
    InvalidHandle {
        error_id: &'static str,
    },
    /// 引擎通用。
    SessionMismatch {
        error_id: &'static str,
    },
    /// 引擎通用。
    StaleEpoch {
        error_id: &'static str,
    },
    UnknownSectionKey {
        error_id: &'static str,
    },
    SectionYOutOfRange {
        error_id: &'static str,
    },
    CoordinateOutOfBounds {
        error_id: &'static str,
    },
    /// 回执覆盖上界不足以清除该 Section 的脏标记。
    StaleSectionRevision {
        error_id: &'static str,
    },
    /// 未被回执覆盖的脏 Section 被要求卸载。
    DirtySectionNotDurable {
        error_id: &'static str,
    },
}

impl DirtyError {
    pub fn error_id(&self) -> &'static str {
        match self {
            Self::InvalidHandle { error_id }
            | Self::SessionMismatch { error_id }
            | Self::StaleEpoch { error_id }
            | Self::UnknownSectionKey { error_id }
            | Self::SectionYOutOfRange { error_id }
            | Self::CoordinateOutOfBounds { error_id }
            | Self::StaleSectionRevision { error_id }
            | Self::DirtySectionNotDurable { error_id } => error_id,
        }
    }

    fn invalid_handle() -> Self {
        Self::InvalidHandle {
            error_id: stable("InvalidHandle"),
        }
    }

    fn session_mismatch() -> Self {
        Self::SessionMismatch {
            error_id: stable("SessionMismatch"),
        }
    }

    fn stale_epoch() -> Self {
        Self::StaleEpoch {
            error_id: stable("StaleEpoch"),
        }
    }

    fn unknown_section_key() -> Self {
        Self::UnknownSectionKey {
            error_id: contract(vw::UNKNOWN_SECTION_KEY),
        }
    }

    fn section_y_out_of_range() -> Self {
        Self::SectionYOutOfRange {
            error_id: contract(vw::SECTION_Y_OUT_OF_RANGE),
        }
    }

    fn coordinate_out_of_bounds() -> Self {
        Self::CoordinateOutOfBounds {
            error_id: contract(vw::COORDINATE_OUT_OF_BOUNDS),
        }
    }

    /// 回执只清除 revision 不超过其声明上界的脏标记(契约 `residency.ack-covers-declared-bound`)。
    pub fn stale_section_revision() -> Self {
        Self::StaleSectionRevision {
            error_id: contract(vw::STALE_SECTION_REVISION),
        }
    }

    /// 未被回执覆盖的脏 Section 不得卸载(契约 `residency.dirty-needs-ack`)。
    pub fn dirty_section_not_durable() -> Self {
        Self::DirtySectionNotDurable {
            error_id: contract(vw::DIRTY_SECTION_NOT_DURABLE),
        }
    }
}

impl std::fmt::Display for DirtyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.error_id())
    }
}

impl std::error::Error for DirtyError {}

fn stable(id: &'static str) -> &'static str {
    STABLE_ERROR_IDS
        .iter()
        .copied()
        .find(|candidate| *candidate == id)
        .expect("mapped error id must exist in the frozen mirror's STABLE_ERROR_IDS")
}

fn contract(id: &'static str) -> &'static str {
    vw::intern_error_code(id).expect("mapped error id must exist in the contract errorCodes")
}

fn parse_section(raw: &str) -> Result<SectionId, DirtyError> {
    SectionId::parse(raw).map_err(|err| match err.rejection() {
        KeyRejection::CoordinateOutOfBounds => DirtyError::coordinate_out_of_bounds(),
        KeyRejection::SectionYOutOfRange => DirtyError::section_y_out_of_range(),
        _ => DirtyError::unknown_section_key(),
    })
}

fn ack_schema_id() -> &'static str {
    SCHEMA_IDS
        .iter()
        .copied()
        .find(|id| *id == ACK_SCHEMA)
        .expect("voxel-durability-ack must exist in generated SCHEMA_IDS")
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DirtyFrontier {
    world_id: String,
    generation: u64,
    entries: BTreeMap<SectionId, DirtyEntry>,
}

impl DirtyFrontier {
    pub fn new(world_id: impl Into<String>, generation: u64) -> Result<Self, DirtyError> {
        let world_id = world_id.into();
        if world_id.is_empty() {
            return Err(DirtyError::invalid_handle());
        }
        let _ = ack_schema_id();
        Ok(Self {
            world_id,
            generation,
            entries: BTreeMap::new(),
        })
    }

    /// Returns a new frontier. `self` is unchanged.
    pub fn record(
        &self,
        section_id: &str,
        revision: u64,
        reason: &str,
    ) -> Result<Self, DirtyError> {
        if reason.is_empty() {
            return Err(DirtyError::invalid_handle());
        }
        let id = parse_section(section_id)?;
        let rev = SchemaRevision(revision);
        let mut next = self.clone();
        match next.entries.get(&id) {
            None => {
                next.entries.insert(
                    id,
                    DirtyEntry {
                        first: rev,
                        latest: rev,
                        reason: reason.to_string(),
                    },
                );
            }
            Some(prev) => {
                let first = if rev < prev.first { rev } else { prev.first };
                let latest = if rev > prev.latest { rev } else { prev.latest };
                next.entries.insert(
                    id,
                    DirtyEntry {
                        first,
                        latest,
                        reason: reason.to_string(),
                    },
                );
            }
        }
        Ok(next)
    }

    pub fn first_revision(&self, section_id: &str) -> Result<Option<u64>, DirtyError> {
        let id = parse_section(section_id)?;
        Ok(self.entries.get(&id).map(|e| e.first.0))
    }

    pub fn latest_revision(&self, section_id: &str) -> Result<Option<u64>, DirtyError> {
        let id = parse_section(section_id)?;
        Ok(self.entries.get(&id).map(|e| e.latest.0))
    }

    pub fn reason(&self, section_id: &str) -> Result<Option<&str>, DirtyError> {
        let id = parse_section(section_id)?;
        Ok(self.entries.get(&id).map(|e| e.reason.as_str()))
    }

    /// Pure coverage: does not clear entries. Wrong world/generation is a generated error.
    pub fn covered_by(&self, ack: &DurabilityAckEvidence) -> Result<DirtyCoverage, DirtyError> {
        let _ = ack_schema_id();
        if ack.kind != ACK_KIND {
            return Err(DirtyError::invalid_handle());
        }
        if ack.world_id != self.world_id {
            return Err(DirtyError::session_mismatch());
        }
        if ack.context.generation != self.generation {
            return Err(DirtyError::stale_epoch());
        }
        let _cut = SchemaRevision(ack.covered_world_revision);

        let mut up_to = BTreeMap::new();
        for section in &ack.covered_sections {
            let id = parse_section(&section.section_id)?;
            if up_to
                .insert(id, SchemaRevision(section.up_to_section_revision))
                .is_some()
            {
                return Err(DirtyError::invalid_handle());
            }
        }

        let mut covered = BTreeMap::new();
        for (id, entry) in &self.entries {
            if let Some(cut_rev) = up_to.get(id)
                && entry.latest <= *cut_rev
            {
                covered.insert(*id, entry.latest);
            }
        }
        Ok(DirtyCoverage { covered })
    }

    /// Returns a new frontier with covered entries removed. Does not mutate `self`.
    /// Not named clear_dirty. The World DurabilityAck path is the only caller.
    pub fn except_covered(&self, coverage: &DirtyCoverage) -> Self {
        let mut next = self.clone();
        for id in coverage.covered.keys() {
            next.entries.remove(id);
        }
        next
    }
}
