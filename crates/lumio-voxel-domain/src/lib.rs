//! L1+L2 domain: `section` / `revision` siblings and L2 publication primitives.
//!
//! Sibling modules must not call each other as services. Concrete files are
//! owned by later cards; this crate only exists as the physical home.

#![forbid(unsafe_code)]

pub const CRATE_NAME: &str = "lumio-voxel-domain";

pub mod config_snapshot;
pub mod key;
pub mod publication;
pub mod revision;
pub mod section;
