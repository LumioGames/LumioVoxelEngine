//! L3 operations: query and mutation always; snapshot/streaming behind features.

#![forbid(unsafe_code)]

pub const CRATE_NAME: &str = "lumio-voxel-ops";

pub mod async_support;

#[cfg(feature = "snapshot")]
pub const SNAPSHOT_FEATURE: bool = true;

#[cfg(not(feature = "snapshot"))]
pub const SNAPSHOT_FEATURE: bool = false;

#[cfg(feature = "streaming")]
pub const STREAMING_FEATURE: bool = true;

#[cfg(not(feature = "streaming"))]
pub const STREAMING_FEATURE: bool = false;
