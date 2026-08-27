//! Retention frontier derived only from live pins. No wall-clock TTL.

#![forbid(unsafe_code)]

use super::pin::{PinRegistry, RegistryState, lock_registry};
use super::stamp::GeneratedRevisionStamp;
use std::sync::{Arc, Mutex};

/// Minimum covering stamp still referenced by at least one live pin.
pub struct RetentionFrontier {
    inner: Arc<Mutex<RegistryState>>,
}

impl RetentionFrontier {
    pub fn from_registry(registry: &PinRegistry) -> Self {
        Self {
            inner: registry.share_state(),
        }
    }

    pub fn oldest_live(&self) -> Option<GeneratedRevisionStamp> {
        let inner = lock_registry(&self.inner);
        inner
            .slots
            .values()
            .map(|slot| &slot.stamp)
            .min_by(|a, b| {
                a.world_revision
                    .cmp(&b.world_revision)
                    .then_with(|| a.chunk_revision_set.cmp(&b.chunk_revision_set))
            })
            .cloned()
    }
}
