//! Reference Voxel port harness. Inputs/outputs are test-private plus
//! generated schema/error ids (no second Schema).

use crate::fault_injection::{FaultInjector, FaultPoint};
use lumio_voxel_contracts::{SCHEMA_IDS, sha256};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GeneratedVoxelOperation {
    pub schema_id: &'static str,
    pub seq: u64,
    pub payload: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GeneratedVoxelOutcome {
    pub schema_id: &'static str,
    pub seq: u64,
    pub payload: Vec<u8>,
    pub error: Option<&'static str>,
    pub recoverable: bool,
}

pub struct VoxelPortHarness {
    injector: FaultInjector,
    committed: Vec<GeneratedVoxelOperation>,
}

impl Default for VoxelPortHarness {
    fn default() -> Self {
        Self::new()
    }
}

impl VoxelPortHarness {
    pub fn new() -> Self {
        Self {
            injector: FaultInjector::new(),
            committed: Vec::new(),
        }
    }

    pub fn arm(&mut self, point: FaultPoint) {
        self.injector.arm(point);
    }

    pub fn execute(&mut self, op: &GeneratedVoxelOperation) -> GeneratedVoxelOutcome {
        debug_assert!(
            SCHEMA_IDS.contains(&op.schema_id),
            "operation schema_id must be a generated schema id"
        );
        match self.injector.take() {
            Some(FaultPoint::PrePublication) => {
                return fail(op, FaultPoint::PrePublication);
            }
            Some(FaultPoint::StaleCompletion) => {
                return fail(op, FaultPoint::StaleCompletion);
            }
            Some(FaultPoint::LostResult) => {
                self.committed.push(op.clone());
                return fail(op, FaultPoint::LostResult);
            }
            Some(FaultPoint::PostPublication) => {
                self.committed.push(op.clone());
                return fail(op, FaultPoint::PostPublication);
            }
            Some(FaultPoint::CorruptSnapshot) => {
                self.committed.push(op.clone());
                let mut out = ok(op);
                out.error = Some(FaultInjector::error_id(FaultPoint::CorruptSnapshot));
                out.recoverable = false;
                return out;
            }
            None => {}
        }
        self.committed.push(op.clone());
        ok(op)
    }

    pub fn snapshot_hash(&self) -> [u8; 32] {
        let mut buf = Vec::new();
        for op in &self.committed {
            buf.extend_from_slice(&op.seq.to_le_bytes());
            buf.extend_from_slice(op.schema_id.as_bytes());
            buf.extend_from_slice(&op.payload);
        }
        sha256(&buf)
    }
}

fn ok(op: &GeneratedVoxelOperation) -> GeneratedVoxelOutcome {
    GeneratedVoxelOutcome {
        schema_id: op.schema_id,
        seq: op.seq,
        payload: op.payload.clone(),
        error: None,
        recoverable: false,
    }
}

fn fail(op: &GeneratedVoxelOperation, point: FaultPoint) -> GeneratedVoxelOutcome {
    GeneratedVoxelOutcome {
        schema_id: op.schema_id,
        seq: op.seq,
        payload: Vec::new(),
        error: Some(FaultInjector::error_id(point)),
        recoverable: FaultInjector::recoverable(point),
    }
}
