//! Immutable revision pins bound to one WorldContext (R-00071).
//!
//! clone/drop change only the private refcount; they do not advance Revision.
//! `max_pins` is adapter-internal because the snapshot has no pin-capacity field.

#![forbid(unsafe_code)]

use super::stamp::{GeneratedRevisionStamp, REVISION_STAMP_SCHEMA};
use crate::config_snapshot::VoxelConfigSnapshot;
use lumio_voxel_contracts::{SCHEMA_IDS, STABLE_ERROR_IDS};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PinError {
    InvalidHandle { error_id: &'static str },
    BudgetExceeded { error_id: &'static str },
}

impl PinError {
    pub fn error_id(&self) -> &'static str {
        match self {
            Self::InvalidHandle { error_id } | Self::BudgetExceeded { error_id } => error_id,
        }
    }

    fn invalid() -> Self {
        Self::InvalidHandle {
            error_id: stable("InvalidHandle"),
        }
    }

    fn budget() -> Self {
        Self::BudgetExceeded {
            error_id: stable("BudgetExceeded"),
        }
    }
}

impl std::fmt::Display for PinError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.error_id())
    }
}

impl std::error::Error for PinError {}

fn stable(id: &'static str) -> &'static str {
    debug_assert!(STABLE_ERROR_IDS.contains(&id));
    id
}

pub(crate) struct LivePin {
    pub(crate) stamp: GeneratedRevisionStamp,
    refs: usize,
}

pub(crate) struct RegistryState {
    context_id: String,
    generation: u64,
    max_pins: usize,
    destroyed: bool,
    next_id: u64,
    pub(crate) slots: BTreeMap<u64, LivePin>,
}

pub(crate) fn lock_registry(state: &Mutex<RegistryState>) -> MutexGuard<'_, RegistryState> {
    state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// One WorldContext's pin table. Two registries never share this state.
pub struct PinRegistry {
    snapshot: Arc<VoxelConfigSnapshot>,
    inner: Arc<Mutex<RegistryState>>,
}

impl PinRegistry {
    /// Bind a registry to an approved snapshot. `max_pins` is captured with
    /// this instance; there is no unbounded constructor and no Schema column.
    pub fn from_approved_snapshot(
        snapshot: Arc<VoxelConfigSnapshot>,
        max_pins: usize,
        context_id: impl Into<String>,
        generation: u64,
    ) -> Self {
        debug_assert!(SCHEMA_IDS.contains(&REVISION_STAMP_SCHEMA));
        Self {
            snapshot,
            inner: Arc::new(Mutex::new(RegistryState {
                context_id: context_id.into(),
                generation,
                max_pins,
                destroyed: false,
                next_id: 0,
                slots: BTreeMap::new(),
            })),
        }
    }

    pub fn config_hash(&self) -> &str {
        self.snapshot.config_hash()
    }

    pub fn destroy(&self) {
        lock_registry(&self.inner).destroyed = true;
    }

    pub fn try_pin(&self, stamp: GeneratedRevisionStamp) -> Result<RevisionPin, PinError> {
        let mut inner = lock_registry(&self.inner);
        if inner.destroyed {
            return Err(PinError::invalid());
        }
        if stamp.schema_id != REVISION_STAMP_SCHEMA
            || stamp.context_id != inner.context_id
            || stamp.generation != inner.generation
        {
            return Err(PinError::invalid());
        }
        if inner.slots.len() >= inner.max_pins {
            return Err(PinError::budget());
        }
        let id = inner.next_id;
        inner.next_id = inner.next_id.checked_add(1).ok_or_else(PinError::invalid)?;
        inner.slots.insert(
            id,
            LivePin {
                stamp: stamp.clone(),
                refs: 1,
            },
        );
        drop(inner);
        Ok(RevisionPin {
            stamp,
            id,
            inner: Arc::clone(&self.inner),
        })
    }

    pub(crate) fn share_state(&self) -> Arc<Mutex<RegistryState>> {
        Arc::clone(&self.inner)
    }
}

/// Holds the exact stamp captured at `try_pin`. Clone/drop do not publish.
pub struct RevisionPin {
    stamp: GeneratedRevisionStamp,
    id: u64,
    inner: Arc<Mutex<RegistryState>>,
}

impl RevisionPin {
    pub fn stamp(&self) -> &GeneratedRevisionStamp {
        &self.stamp
    }
}

impl Clone for RevisionPin {
    fn clone(&self) -> Self {
        {
            let mut inner = lock_registry(&self.inner);
            if let Some(slot) = inner.slots.get_mut(&self.id) {
                slot.refs = slot.refs.saturating_add(1);
            }
        }
        Self {
            stamp: self.stamp.clone(),
            id: self.id,
            inner: Arc::clone(&self.inner),
        }
    }
}

impl Drop for RevisionPin {
    fn drop(&mut self) {
        let mut inner = lock_registry(&self.inner);
        let Some(slot) = inner.slots.get_mut(&self.id) else {
            return;
        };
        if slot.refs <= 1 {
            inner.slots.remove(&self.id);
        } else {
            slot.refs -= 1;
        }
    }
}

impl std::fmt::Debug for RevisionPin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RevisionPin")
            .field("stamp", &self.stamp)
            .finish_non_exhaustive()
    }
}
