//! Bounded try_submit port. Full-load action is generated `QueueFull`.

use super::origin::OriginEnvelope;
use lumio_voxel_contracts::{BoundedBuffer, BufferFull, STABLE_ERROR_IDS};
use lumio_voxel_domain::config_snapshot::VoxelConfigSnapshot;
use std::collections::VecDeque;
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SubmitError {
    error_id: &'static str,
}

impl SubmitError {
    pub fn error_id(&self) -> &'static str {
        self.error_id
    }
}

/// Fail-closed full-load action from ADR-0005 / owner confirmation.
pub fn full_load_action() -> &'static str {
    debug_assert!(STABLE_ERROR_IDS.contains(&"QueueFull"));
    "QueueFull"
}

pub struct BoundedJobPort<T> {
    snapshot: Arc<VoxelConfigSnapshot>,
    bound: BoundedBuffer,
    queue: VecDeque<OriginEnvelope<T>>,
}

impl<T> BoundedJobPort<T> {
    /// Bind a port to an approved snapshot. `slots` is adapter-internal and
    /// captured with this snapshot; there is no unbounded constructor.
    pub fn from_approved_snapshot(
        snapshot: Arc<VoxelConfigSnapshot>,
        slots: usize,
    ) -> Result<Self, SubmitError> {
        if slots == 0 {
            return Err(SubmitError {
                error_id: full_load_action(),
            });
        }
        Ok(Self {
            snapshot,
            bound: BoundedBuffer::new(slots),
            queue: VecDeque::new(),
        })
    }

    pub fn config_hash(&self) -> &str {
        self.snapshot.config_hash()
    }

    pub fn try_submit(&mut self, job: OriginEnvelope<T>) -> Result<(), SubmitError> {
        if job.config_hash != self.snapshot.config_hash() {
            return Err(SubmitError {
                error_id: stable("EvidenceDigestMismatch"),
            });
        }
        match self.bound.push(1) {
            Ok(()) => {
                self.queue.push_back(job);
                Ok(())
            }
            Err(BufferFull) => Err(SubmitError {
                error_id: full_load_action(),
            }),
        }
    }

    pub fn pop(&mut self) -> Option<OriginEnvelope<T>> {
        self.queue.pop_front()
    }
}

fn stable(id: &'static str) -> &'static str {
    debug_assert!(STABLE_ERROR_IDS.contains(&id));
    id
}
