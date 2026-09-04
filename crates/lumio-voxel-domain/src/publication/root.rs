//! Immutable published cut: stamp + directory + dirty frontier.

#![forbid(unsafe_code)]

use crate::revision::{GeneratedRevisionStamp, REVISION_STAMP_SCHEMA};
use crate::section::{DirtyFrontier, SectionDirectoryRoot, SectionReplacement};
use lumio_voxel_contracts::{SCHEMA_IDS, sha256};

/// Optional auxiliary indexes. This card ships the empty cut only.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AuxiliaryIndexes;

impl AuxiliaryIndexes {
    pub fn empty() -> Self {
        Self
    }

    pub fn is_empty(&self) -> bool {
        true
    }
}

/// One immutable published cut. Members are never swapped independently.
#[derive(Clone, Debug)]
pub struct PublishedStateRoot {
    stamp: GeneratedRevisionStamp,
    directory: SectionDirectoryRoot,
    dirty_frontier: DirtyFrontier,
    indexes: AuxiliaryIndexes,
    identity: [u8; 32],
}

impl PublishedStateRoot {
    pub fn new(
        stamp: GeneratedRevisionStamp,
        directory: SectionDirectoryRoot,
        dirty_frontier: DirtyFrontier,
    ) -> Self {
        let _ = revision_schema();
        let indexes = AuxiliaryIndexes::empty();
        let identity = fingerprint(&stamp, &directory, &dirty_frontier, &indexes, None);
        Self {
            stamp,
            directory,
            dirty_frontier,
            indexes,
            identity,
        }
    }

    pub fn stamp(&self) -> &GeneratedRevisionStamp {
        &self.stamp
    }

    pub fn directory(&self) -> &SectionDirectoryRoot {
        &self.directory
    }

    pub fn dirty_frontier(&self) -> &DirtyFrontier {
        &self.dirty_frontier
    }

    pub fn indexes(&self) -> &AuxiliaryIndexes {
        &self.indexes
    }

    pub fn identity(&self) -> [u8; 32] {
        self.identity
    }

    pub(crate) fn incorporate_replacement(&mut self, replacement: &SectionReplacement) {
        self.identity = fingerprint(
            &self.stamp,
            &self.directory,
            &self.dirty_frontier,
            &self.indexes,
            Some(replacement.digest()),
        );
    }
}

fn revision_schema() -> &'static str {
    SCHEMA_IDS
        .iter()
        .copied()
        .find(|id| *id == REVISION_STAMP_SCHEMA)
        .expect("voxel-revision-stamp must exist in generated SCHEMA_IDS")
}

fn fingerprint(
    stamp: &GeneratedRevisionStamp,
    directory: &SectionDirectoryRoot,
    frontier: &DirtyFrontier,
    indexes: &AuxiliaryIndexes,
    replacement_digest: Option<[u8; 32]>,
) -> [u8; 32] {
    let mut buf = Vec::new();
    buf.extend_from_slice(stamp.schema_id.as_bytes());
    buf.push(0);
    buf.extend_from_slice(stamp.world_id.as_bytes());
    buf.push(0);
    buf.extend_from_slice(stamp.context_id.as_bytes());
    buf.push(0);
    buf.extend_from_slice(&stamp.generation.to_le_bytes());
    buf.extend_from_slice(&stamp.world_revision.to_le_bytes());
    for (section_id, rev) in &stamp.section_revision_set {
        buf.extend_from_slice(section_id.as_bytes());
        buf.push(0);
        buf.extend_from_slice(&rev.to_le_bytes());
    }
    // Sibling exclusive files keep directory/frontier maps private; Debug is the
    // in-crate fingerprint of the whole cut without adding public iterators.
    buf.extend_from_slice(format!("{directory:?}").as_bytes());
    buf.push(0);
    buf.extend_from_slice(format!("{frontier:?}").as_bytes());
    buf.push(0);
    buf.extend_from_slice(format!("{indexes:?}").as_bytes());
    match replacement_digest {
        Some(digest) => buf.extend_from_slice(&digest),
        None => buf.extend_from_slice(&[0u8; 32]),
    }
    sha256(&buf)
}
