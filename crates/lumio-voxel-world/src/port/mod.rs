//! Generated `voxel-world-port` total adapter. No second ABI, no FFI.

#![forbid(unsafe_code)]

mod adapter;
mod error_mapping;
mod ownership;

pub use adapter::{GeneratedVoxelWorldPortAdapter, PortEvidence};
pub use error_mapping::{
    PortError, map_internal_error, map_mutation_error, map_query_error, map_world_error,
};
pub use ownership::OwnedResultBuffer;
