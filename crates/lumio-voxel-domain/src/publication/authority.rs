//! Publication authority: capture clones one Arc; publish_once is the only writer.

#![forbid(unsafe_code)]

use super::prepared::{PreparedPublication, PublicationToken};
use super::root::PublishedStateRoot;
use super::{PublishError, map_pin};
use crate::chunk::{ChunkDirectoryRoot, ChunkReplacement, DirtyFrontier};
use crate::revision::{
    GeneratedRevisionStamp, PinRegistry, REVISION_STAMP_SCHEMA, ReadViewLease, RevisionPin,
    WorldRevision,
};
use std::collections::BTreeSet;
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};

struct PublishedCell {
    root: Arc<PublishedStateRoot>,
    pin: RevisionPin,
    used_ids: BTreeSet<u64>,
    next_token_id: u64,
}

/// Per-world publisher. Two authorities never share this `RwLock` or root `Arc`.
pub struct PublicationAuthority {
    world_id: String,
    context_id: String,
    generation: u64,
    pins: Arc<PinRegistry>,
    /// Lock is only for swapping or cloning the `Arc<PublishedStateRoot>`.
    /// Readers clone the `Arc` then drop the guard before inspecting members.
    inner: RwLock<PublishedCell>,
}

impl PublicationAuthority {
    pub fn new(
        world_id: impl Into<String>,
        context_id: impl Into<String>,
        generation: u64,
        pins: PinRegistry,
        initial: PublishedStateRoot,
    ) -> Result<Self, PublishError> {
        let world_id = world_id.into();
        let context_id = context_id.into();
        if world_id.is_empty() || context_id.is_empty() {
            return Err(PublishError::invalid_handle());
        }
        check_stamp(&world_id, &context_id, generation, initial.stamp())?;
        let pin = pins.try_pin(initial.stamp().clone()).map_err(map_pin)?;
        Ok(Self {
            world_id,
            context_id,
            generation,
            pins: Arc::new(pins),
            inner: RwLock::new(PublishedCell {
                root: Arc::new(initial),
                pin,
                used_ids: BTreeSet::new(),
                next_token_id: 0,
            }),
        })
    }

    pub fn capture(&self) -> PublishedReadView {
        let (root, pin) = {
            let inner = self.read();
            (Arc::clone(&inner.root), inner.pin.clone())
        };
        PublishedReadView::from_parts(root, pin)
    }

    pub fn prepare(
        &self,
        target_revision: WorldRevision,
        mut new_root: PublishedStateRoot,
        replacement: ChunkReplacement,
    ) -> Result<PreparedPublication, PublishError> {
        check_stamp(
            &self.world_id,
            &self.context_id,
            self.generation,
            new_root.stamp(),
        )?;
        if target_revision.value() != new_root.stamp().world_revision {
            return Err(PublishError::invalid_handle());
        }
        new_root.incorporate_replacement(&replacement);
        let (token_id, base_identity) = {
            let mut inner = self.write();
            let token_id = inner.next_token_id;
            inner.next_token_id = token_id
                .checked_add(1)
                .ok_or_else(PublishError::invalid_handle)?;
            (token_id, inner.root.identity())
        };
        Ok(PreparedPublication::bind(
            token_id,
            base_identity,
            self.world_id.clone(),
            self.context_id.clone(),
            self.generation,
            new_root,
            Arc::clone(&self.pins),
        ))
    }

    pub fn publish_once(&self, token: PublicationToken) -> Result<PublishedReadView, PublishError> {
        let mut inner = self.write();
        if inner.used_ids.contains(&token.id) {
            return Err(PublishError::handle_double_release());
        }
        if token.world_id != self.world_id || token.context_id != self.context_id {
            return Err(PublishError::session_mismatch());
        }
        if token.generation != self.generation {
            return Err(PublishError::stale_epoch());
        }
        if token.base_identity != inner.root.identity() {
            return Err(PublishError::snapshot_base_mismatch());
        }

        let view_root = Arc::clone(&token.new_root);
        let view_pin = token.pin.clone();
        inner.used_ids.insert(token.id);

        // Sole visible write: move the prebuilt Arc. No alloc/I/O/callback after this.
        inner.root = token.new_root;
        inner.pin = token.pin;
        drop(inner);

        Ok(PublishedReadView::from_parts(view_root, view_pin))
    }

    fn read(&self) -> RwLockReadGuard<'_, PublishedCell> {
        self.inner
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn write(&self) -> RwLockWriteGuard<'_, PublishedCell> {
        self.inner
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

/// Immutable captured cut. All accessors read the same `Arc`.
#[derive(Debug)]
pub struct PublishedReadView {
    root: Arc<PublishedStateRoot>,
    lease: ReadViewLease,
}

impl PublishedReadView {
    fn from_parts(root: Arc<PublishedStateRoot>, pin: RevisionPin) -> Self {
        Self {
            root,
            lease: ReadViewLease::from_pin(pin),
        }
    }

    pub fn root(&self) -> &PublishedStateRoot {
        &self.root
    }

    pub fn root_arc(&self) -> Arc<PublishedStateRoot> {
        Arc::clone(&self.root)
    }

    pub fn lease(&self) -> &ReadViewLease {
        &self.lease
    }

    pub fn stamp(&self) -> &GeneratedRevisionStamp {
        self.root.stamp()
    }

    pub fn directory(&self) -> &ChunkDirectoryRoot {
        self.root.directory()
    }

    pub fn dirty_frontier(&self) -> &DirtyFrontier {
        self.root.dirty_frontier()
    }
}

fn check_stamp(
    world_id: &str,
    context_id: &str,
    generation: u64,
    stamp: &GeneratedRevisionStamp,
) -> Result<(), PublishError> {
    if stamp.schema_id != REVISION_STAMP_SCHEMA {
        return Err(PublishError::invalid_handle());
    }
    if stamp.world_id != world_id || stamp.context_id != context_id {
        return Err(PublishError::session_mismatch());
    }
    if stamp.generation != generation {
        return Err(PublishError::stale_epoch());
    }
    Ok(())
}
