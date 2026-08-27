//! Fault points for the Voxel port harness (R-00047).
//!
//! Visible writes already published must not be followed by a recoverable
//! failure. Those points are unrecoverable and carry a stable error id.

use lumio_voxel_contracts::STABLE_ERROR_IDS;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FaultPoint {
    PrePublication,
    PostPublication,
    LostResult,
    CorruptSnapshot,
    StaleCompletion,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FaultInjector {
    armed: Option<FaultPoint>,
}

impl FaultInjector {
    pub fn new() -> Self {
        Self { armed: None }
    }

    pub fn arm(&mut self, point: FaultPoint) {
        self.armed = Some(point);
    }

    pub fn take(&mut self) -> Option<FaultPoint> {
        self.armed.take()
    }

    pub fn error_id(point: FaultPoint) -> &'static str {
        let id = match point {
            FaultPoint::PrePublication => "InvalidHandle",
            FaultPoint::PostPublication => "PartialLoadRolledBack",
            FaultPoint::LostResult => "EvidenceMissing",
            FaultPoint::CorruptSnapshot => "EvidenceDigestMismatch",
            FaultPoint::StaleCompletion => "StaleEpoch",
        };
        debug_assert!(STABLE_ERROR_IDS.contains(&id));
        id
    }

    pub fn recoverable(point: FaultPoint) -> bool {
        match point {
            FaultPoint::PrePublication | FaultPoint::StaleCompletion => true,
            FaultPoint::PostPublication | FaultPoint::LostResult | FaultPoint::CorruptSnapshot => {
                false
            }
        }
    }
}
