//! Monotonic World/Chunk revision allocator (R-00070).

#![allow(dead_code)]

use lumio_voxel_contracts::STABLE_ERROR_IDS;

/// World and Chunk revision domains are separate generated integers (min 0).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WorldRevision(u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ChunkRevision(u64);

impl WorldRevision {
    pub fn value(self) -> u64 {
        self.0
    }
}

impl ChunkRevision {
    pub fn value(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RevisionError {
    Overflow { error_id: &'static str },
    DoubleFinalize { error_id: &'static str },
    Abandoned { error_id: &'static str },
}

impl RevisionError {
    pub fn error_id(&self) -> &'static str {
        match self {
            Self::Overflow { error_id }
            | Self::DoubleFinalize { error_id }
            | Self::Abandoned { error_id } => error_id,
        }
    }
}

impl std::fmt::Display for RevisionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.error_id())
    }
}

impl std::error::Error for RevisionError {}

fn stable(id: &'static str) -> &'static str {
    debug_assert!(STABLE_ERROR_IDS.contains(&id));
    id
}

#[derive(Debug)]
pub struct RevisionReservation<T> {
    value: T,
    finalized: bool,
    abandoned: bool,
}

impl<T: Copy> RevisionReservation<T> {
    pub fn value(&self) -> T {
        self.value
    }

    pub fn finalize(&mut self) -> Result<T, RevisionError> {
        if self.abandoned {
            return Err(RevisionError::Abandoned {
                error_id: stable("InvalidHandle"),
            });
        }
        if self.finalized {
            return Err(RevisionError::DoubleFinalize {
                error_id: stable("HandleDoubleRelease"),
            });
        }
        self.finalized = true;
        Ok(self.value)
    }

    pub fn abandon(&mut self) {
        self.abandoned = true;
    }
}

#[derive(Debug)]
pub struct RevisionAllocator {
    next_world: u64,
    next_chunk: u64,
}

impl Default for RevisionAllocator {
    fn default() -> Self {
        Self::new()
    }
}

impl RevisionAllocator {
    pub fn new() -> Self {
        Self {
            next_world: 0,
            next_chunk: 0,
        }
    }

    pub fn reserve_world(&mut self) -> Result<RevisionReservation<WorldRevision>, RevisionError> {
        let v = self.next_world;
        let next = v.checked_add(1).ok_or(RevisionError::Overflow {
            error_id: stable("InvalidHandle"),
        })?;
        self.next_world = next;
        Ok(RevisionReservation {
            value: WorldRevision(v),
            finalized: false,
            abandoned: false,
        })
    }

    pub fn reserve_chunk(&mut self) -> Result<RevisionReservation<ChunkRevision>, RevisionError> {
        let v = self.next_chunk;
        let next = v.checked_add(1).ok_or(RevisionError::Overflow {
            error_id: stable("InvalidHandle"),
        })?;
        self.next_chunk = next;
        Ok(RevisionReservation {
            value: ChunkRevision(v),
            finalized: false,
            abandoned: false,
        })
    }
}

#[cfg(test)]
mod overflow_tests {
    use super::*;

    #[test]
    fn overflow_fails_before_any_visible_write() {
        let mut a = RevisionAllocator {
            next_world: u64::MAX,
            next_chunk: u64::MAX,
        };
        let err = a.reserve_world().unwrap_err();
        assert_eq!(err.error_id(), "InvalidHandle");
        assert_eq!(a.next_world, u64::MAX);
        let err = a.reserve_chunk().unwrap_err();
        assert_eq!(err.error_id(), "InvalidHandle");
        assert_eq!(a.next_chunk, u64::MAX);
    }
}
