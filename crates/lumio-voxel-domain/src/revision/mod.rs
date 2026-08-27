//! Revision allocator and stamp mapping (R-00070).
//!
//! World and Chunk revision domains stay separate. Numeric lease/capacity
//! Schema columns are not defined here.

#![forbid(unsafe_code)]

pub mod allocator;
pub mod stamp;

pub use allocator::{
    ChunkRevision, RevisionAllocator, RevisionError, RevisionReservation, WorldRevision,
};
pub use stamp::{to_generated_stamp, GeneratedRevisionStamp, REVISION_STAMP_SCHEMA};
