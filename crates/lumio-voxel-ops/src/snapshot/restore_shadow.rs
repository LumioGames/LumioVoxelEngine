//! Build an unpublished complete restore root without live World refs.

#![forbid(unsafe_code)]

use super::restore_preflight::{DecodedRestore, RestoreError};
use super::{hex32, quote};
use lumio_voxel_contracts::{canonical_object_pairs, sha256};
use lumio_voxel_domain::chunk::{
    ChunkDeltaBuilder, ChunkDirectoryBuilder, ChunkReplacement, ChunkSlot, DirtyFrontier,
};
use lumio_voxel_domain::publication::PublishedStateRoot;
use lumio_voxel_domain::revision::{
    ChunkRevision, GeneratedRevisionStamp, RevisionAllocator, WorldRevision, to_generated_stamp,
};

/// Move-only sealed unpublished restore root. Not `Clone`.
pub struct SealedRestoreCandidate {
    root: PublishedStateRoot,
    replacement: ChunkReplacement,
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
        self.candidate_hash == fingerprint(&self.root, &self.replacement, &self.config_hash)
    }

    pub fn into_publication(self) -> (PublishedStateRoot, ChunkReplacement, WorldRevision) {
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

/// Materialize NotLoaded directory slots and an empty dirty frontier.
pub struct RestoreShadowBuilder;

impl RestoreShadowBuilder {
    pub fn build(decoded: &DecodedRestore) -> Result<SealedRestoreCandidate, RestoreError> {
        let world = world_revision(decoded.world_revision())?;
        let mut chunk_pairs = Vec::new();
        let mut directory = ChunkDirectoryBuilder::new();
        for (chunk_id, revision) in decoded.chunk_revision_set() {
            chunk_pairs.push((chunk_id.clone(), chunk_revision(*revision)?));
            directory
                .insert(chunk_id, ChunkSlot::not_loaded())
                .map_err(|err| RestoreError::mapped(err.error_id()))?;
        }
        let stamp = to_generated_stamp(
            decoded.world_id(),
            decoded.context_id(),
            decoded.generation(),
            world,
            &chunk_pairs,
        );
        let frozen = directory.freeze();
        let dirty = DirtyFrontier::new(decoded.world_id(), decoded.generation())
            .map_err(|err| RestoreError::mapped(err.error_id()))?;
        let replacement = ChunkDeltaBuilder::new(&frozen)
            .freeze()
            .map_err(|err| RestoreError::mapped(err.error_id()))?;
        let root = PublishedStateRoot::new(stamp, frozen, dirty);
        let candidate_hash = fingerprint(&root, &replacement, decoded.config_hash());
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
    replacement: &ChunkReplacement,
    config_hash: &str,
) -> [u8; 32] {
    let stamp = root.stamp();
    let mut pairs = vec![
        ("configHash".to_string(), quote(config_hash)),
        ("contextId".to_string(), quote(&stamp.context_id)),
        ("generation".to_string(), stamp.generation.to_string()),
        (
            "replacement".to_string(),
            quote(&hex32(&replacement.digest())),
        ),
        ("rootIdentity".to_string(), quote(&hex32(&root.identity()))),
        ("worldId".to_string(), quote(&stamp.world_id)),
        (
            "worldRevision".to_string(),
            stamp.world_revision.to_string(),
        ),
    ];
    sha256(canonical_object_pairs(&mut pairs).as_bytes())
}

fn world_revision(n: u64) -> Result<WorldRevision, RestoreError> {
    let mut alloc = RevisionAllocator::new();
    for _ in 0..n {
        alloc
            .reserve_world()
            .map_err(|err| RestoreError::mapped(err.error_id()))?
            .abandon();
    }
    alloc
        .reserve_world()
        .map_err(|err| RestoreError::mapped(err.error_id()))?
        .finalize()
        .map_err(|err| RestoreError::mapped(err.error_id()))
}

fn chunk_revision(n: u64) -> Result<ChunkRevision, RestoreError> {
    let mut alloc = RevisionAllocator::new();
    for _ in 0..n {
        alloc
            .reserve_chunk()
            .map_err(|err| RestoreError::mapped(err.error_id()))?
            .abandon();
    }
    alloc
        .reserve_chunk()
        .map_err(|err| RestoreError::mapped(err.error_id()))?
        .finalize()
        .map_err(|err| RestoreError::mapped(err.error_id()))
}
