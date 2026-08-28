//! Crate-private prepared publication and one-shot, non-Clone token.

#![forbid(unsafe_code)]

use super::root::PublishedStateRoot;
use super::{PublishError, map_pin};
use crate::revision::{PinRegistry, RevisionPin};
use std::sync::Arc;

/// Bound, unpublished cut. Constructed only by `PublicationAuthority::prepare`.
pub struct PreparedPublication {
    token_id: u64,
    base_identity: [u8; 32],
    world_id: String,
    context_id: String,
    generation: u64,
    new_root: PublishedStateRoot,
    pins: Arc<PinRegistry>,
    sealed: bool,
}

impl PreparedPublication {
    pub(crate) fn bind(
        token_id: u64,
        base_identity: [u8; 32],
        world_id: String,
        context_id: String,
        generation: u64,
        new_root: PublishedStateRoot,
        pins: Arc<PinRegistry>,
    ) -> Self {
        Self {
            token_id,
            base_identity,
            world_id,
            context_id,
            generation,
            new_root,
            pins,
            sealed: false,
        }
    }

    /// Identity of the bound cut, after `prepare` folded the replacement digest in.
    /// Callers must read the new root identity here, never from the value they handed
    /// to `prepare` — that one predates `incorporate_replacement` and is never published.
    pub fn new_root_identity(&self) -> [u8; 32] {
        self.new_root.identity()
    }

    /// First call yields the one-shot token. A second call is `HandleDoubleRelease`.
    pub fn seal(&mut self) -> Result<PublicationToken, PublishError> {
        if self.sealed {
            return Err(PublishError::handle_double_release());
        }
        let pin = self
            .pins
            .try_pin(self.new_root.stamp().clone())
            .map_err(map_pin)?;
        self.sealed = true;
        Ok(PublicationToken {
            id: self.token_id,
            base_identity: self.base_identity,
            world_id: self.world_id.clone(),
            context_id: self.context_id.clone(),
            generation: self.generation,
            new_root: Arc::new(self.new_root.clone()),
            pin,
        })
    }
}

/// One-shot publication token. Not `Clone`; `publish_once` consumes it by value.
#[derive(Debug)]
pub struct PublicationToken {
    pub(crate) id: u64,
    pub(crate) base_identity: [u8; 32],
    pub(crate) world_id: String,
    pub(crate) context_id: String,
    pub(crate) generation: u64,
    pub(crate) new_root: Arc<PublishedStateRoot>,
    pub(crate) pin: RevisionPin,
}

impl PublicationToken {
    pub fn id(&self) -> u64 {
        self.id
    }
}

impl std::fmt::Debug for PreparedPublication {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PreparedPublication")
            .field("token_id", &self.token_id)
            .field("world_id", &self.world_id)
            .field("context_id", &self.context_id)
            .field("generation", &self.generation)
            .field("sealed", &self.sealed)
            .finish_non_exhaustive()
    }
}
