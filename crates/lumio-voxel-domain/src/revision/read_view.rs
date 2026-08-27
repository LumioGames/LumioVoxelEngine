//! Long-lived ReadView lease. The stamp is the one captured at pin/create time.

#![forbid(unsafe_code)]

use super::pin::RevisionPin;
use super::stamp::GeneratedRevisionStamp;

/// Immutable read lease. Concurrent commit or config reload cannot mix cuts.
#[derive(Clone, Debug)]
pub struct ReadViewLease {
    pin: RevisionPin,
}

impl ReadViewLease {
    pub fn from_pin(pin: RevisionPin) -> Self {
        Self { pin }
    }

    pub fn stamp(&self) -> &GeneratedRevisionStamp {
        self.pin.stamp()
    }
}
