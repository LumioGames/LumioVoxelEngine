//! Build an unpublished complete restore root without live World refs.

#![forbid(unsafe_code)]

use super::hex32;
use super::restore_preflight::{DecodedRestore, RestoreError};
use crate::canonical::{CanonicalObject, CanonicalValue};
use lumio_voxel_contracts::sha256;
use lumio_voxel_domain::publication::PublishedStateRoot;
use lumio_voxel_domain::revision::{
    GeneratedRevisionStamp, SectionRevision, WorldRevision, to_generated_stamp,
};
use lumio_voxel_domain::section::{
    DirtyFrontier, SectionDeltaBuilder, SectionDirectoryBuilder, SectionReplacement, SectionSlot,
};

/// Move-only sealed unpublished restore root. Not `Clone`.
pub struct SealedRestoreCandidate {
    root: PublishedStateRoot,
    replacement: SectionReplacement,
    world_revision: WorldRevision,
    config_hash: String,
    candidate_hash: [u8; 32],
}

impl SealedRestoreCandidate {
    pub fn world_id(&self) -> &str {
        &self.root.stamp().world_id
    }

    pub fn context_id(&self) -> &str {
        &self.root.stamp().context_id
    }

    pub fn generation(&self) -> u64 {
        self.root.stamp().generation
    }

    pub fn world_revision(&self) -> u64 {
        self.root.stamp().world_revision
    }

    pub fn config_hash(&self) -> &str {
        &self.config_hash
    }

    pub fn candidate_hash(&self) -> [u8; 32] {
        self.candidate_hash
    }

    pub fn stamp(&self) -> &GeneratedRevisionStamp {
        self.root.stamp()
    }

    pub fn hash_matches(&self) -> bool {
        fingerprint(&self.root, &self.replacement, &self.config_hash)
            .is_ok_and(|hash| hash == self.candidate_hash)
    }

    pub fn into_publication(self) -> (PublishedStateRoot, SectionReplacement, WorldRevision) {
        (self.root, self.replacement, self.world_revision)
    }
}

impl std::fmt::Debug for SealedRestoreCandidate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SealedRestoreCandidate")
            .field("world_id", &self.world_id())
            .field("generation", &self.generation())
            .field("world_revision", &self.world_revision())
            .finish_non_exhaustive()
    }
}

/// Materialize Unchanged directory slots and an empty dirty frontier.
pub struct RestoreShadowBuilder;

impl RestoreShadowBuilder {
    pub fn build(decoded: &DecodedRestore) -> Result<SealedRestoreCandidate, RestoreError> {
        let world = world_revision(decoded.world_revision())?;
        let mut section_pairs = Vec::new();
        let mut directory = SectionDirectoryBuilder::new();
        for (section_id, revision) in decoded.section_revision_set() {
            section_pairs.push((section_id.clone(), section_revision(*revision)?));
            directory
                .insert(section_id, SectionSlot::unchanged())
                .map_err(|err| RestoreError::mapped(err.error_id()))?;
        }
        let stamp = to_generated_stamp(
            decoded.world_id(),
            decoded.context_id(),
            decoded.generation(),
            world,
            &section_pairs,
        );
        let frozen = directory.freeze();
        let dirty = DirtyFrontier::new(decoded.world_id(), decoded.generation())
            .map_err(|err| RestoreError::mapped(err.error_id()))?;
        let replacement = SectionDeltaBuilder::new(&frozen)
            .freeze()
            .map_err(|err| RestoreError::mapped(err.error_id()))?;
        let root = PublishedStateRoot::new(stamp, frozen, dirty);
        let candidate_hash = fingerprint(&root, &replacement, decoded.config_hash())?;
        Ok(SealedRestoreCandidate {
            root,
            replacement,
            world_revision: world,
            config_hash: decoded.config_hash().to_string(),
            candidate_hash,
        })
    }
}

fn fingerprint(
    root: &PublishedStateRoot,
    replacement: &SectionReplacement,
    config_hash: &str,
) -> Result<[u8; 32], RestoreError> {
    let stamp = root.stamp();
    let mut object = CanonicalObject::new();
    for (key, value) in [
        ("configHash", CanonicalValue::text(config_hash)),
        ("contextId", CanonicalValue::text(&stamp.context_id)),
        ("generation", CanonicalValue::Uint(stamp.generation)),
        (
            "replacement",
            CanonicalValue::text(hex32(&replacement.digest())),
        ),
        (
            "rootIdentity",
            CanonicalValue::text(hex32(&root.identity())),
        ),
        ("worldId", CanonicalValue::text(&stamp.world_id)),
        ("worldRevision", CanonicalValue::Uint(stamp.world_revision)),
    ] {
        object
            .insert(key, value)
            .map_err(|_| RestoreError::invalid_handle())?;
    }
    Ok(sha256(object.encode().as_bytes()))
}

fn world_revision(n: u64) -> Result<WorldRevision, RestoreError> {
    Ok(WorldRevision::from_raw(n))
}

fn section_revision(n: u64) -> Result<SectionRevision, RestoreError> {
    Ok(SectionRevision::from_raw(n))
}
