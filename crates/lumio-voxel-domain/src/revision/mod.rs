//! Revision allocator and stamp mapping (R-00070).
//!
//! World and Section revision domains stay separate. Numeric lease/capacity
//! Schema columns are not defined here.

#![forbid(unsafe_code)]

pub mod allocator;
pub mod pin;
pub mod read_view;
pub mod retention;
pub mod stamp;

pub use allocator::{
    RevisionAllocator, RevisionError, RevisionReservation, SectionRevision, WorldRevision,
};
pub use pin::{PinError, PinRegistry, RevisionPin};
pub use read_view::ReadViewLease;
pub use retention::RetentionFrontier;
pub use stamp::{GeneratedRevisionStamp, REVISION_STAMP_SCHEMA, to_generated_stamp};
