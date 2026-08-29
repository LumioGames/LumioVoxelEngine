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
    slots: usize,
    bound: BoundedBuffer,
    queue: VecDeque<OriginEnvelope<T>>,
}

/// Rebuild the bounded budget for a live occupancy.
///
/// The generated `BoundedBuffer` has `push` but no release API, and generated
/// code must not be hand-edited, so a freed slot is expressed by rebuilding the
/// budget from the queue's current length. Occupancy is therefore always
/// re-derived from the queue and can never drift below zero or wrap.
fn occupancy_bound(slots: usize, occupancy: usize) -> BoundedBuffer {
    debug_assert!(occupancy <= slots);
    let mut bound = BoundedBuffer::new(slots);
    for _ in 0..occupancy {
        bound.push(1).expect("occupancy never exceeds slots");
    }
    bound
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
            slots,
            bound: occupancy_bound(slots, 0),
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

    /// Pop a job and return its slot to the bounded budget.
    ///
    /// A pop on an empty queue is a no-op: the `?` returns before the budget is
    /// touched, so surplus pops cannot inflate capacity past `slots`.
    pub fn pop(&mut self) -> Option<OriginEnvelope<T>> {
        let job = self.queue.pop_front()?;
        self.bound = occupancy_bound(self.slots, self.queue.len());
        Some(job)
    }
}

fn stable(id: &'static str) -> &'static str {
    debug_assert!(STABLE_ERROR_IDS.contains(&id));
    id
}
