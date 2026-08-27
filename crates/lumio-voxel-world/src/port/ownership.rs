//! Owned result buffer wrapping generated `BoundedBuffer`. No raw pointers.

#![forbid(unsafe_code)]

use super::error_mapping::{PortError, map_internal_error};
use lumio_voxel_contracts::BoundedBuffer;

/// Process-local owned result bytes. Transfer moves the buffer; release invalidates it.
pub struct OwnedResultBuffer {
    inner: Option<BoundedBuffer>,
}

impl OwnedResultBuffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            inner: Some(BoundedBuffer::new(capacity)),
        }
    }

    pub fn as_slice(&self) -> Result<&[u8], PortError> {
        match &self.inner {
            Some(buffer) => Ok(buffer.as_slice()),
            None => Err(map_internal_error("InvalidHandle")),
        }
    }

    pub fn transfer(&mut self) -> Result<BoundedBuffer, PortError> {
        self.inner
            .take()
            .ok_or_else(|| map_internal_error("InvalidHandle"))
    }

    pub fn release(&mut self) -> Result<(), PortError> {
        match self.inner.take() {
            Some(_) => Ok(()),
            None => Err(map_internal_error("HandleDoubleRelease")),
        }
    }
}
