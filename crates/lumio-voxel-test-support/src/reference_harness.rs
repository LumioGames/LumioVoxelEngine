//! Reference Voxel port harness. Inputs/outputs are test-private plus
//! generated schema/error ids (no second Schema).

use crate::fault_injection::{FaultInjector, FaultPoint};
use lumio_voxel_contracts::voxel_world as vw;
use lumio_voxel_contracts::{BASELINE_ID, SCHEMA_EPOCH, SCHEMA_IDS, sha256};
use lumio_voxel_domain::config_snapshot::{
    DecisionEvidence, GateSourceHashes, GeneratedHostCapability, GeneratedVoxelConfig,
    P0_DECISION_GATES, VoxelConfigSnapshot,
};
use lumio_voxel_domain::publication::PublishedStateRoot;
use lumio_voxel_domain::revision::{
    GeneratedRevisionStamp, REVISION_STAMP_SCHEMA, RevisionAllocator, WorldRevision,
};
use lumio_voxel_domain::section::{
    CoveredSectionAck, DirtyFrontier, DurabilityAckContext, SectionDeltaBuilder,
    SectionDirectoryBuilder, SectionPage, SectionPayload, SectionSlot,
};
use lumio_voxel_ops::async_support::{OriginEnvelope, OriginToken};
use lumio_voxel_ops::mutation::{MutationRequest, PreparedMutation, canonical_fingerprint};
use lumio_voxel_ops::query::GeneratedVoxelQueryRequest;
use lumio_voxel_ops::snapshot::{
    MemoryCaptureWriter, RestorePreflight, RestoreShadowBuilder, VoxelCaptureRef,
    decode_canonical_object, encode_capture,
};
use lumio_voxel_world::port::GeneratedVoxelWorldPortAdapter;
use lumio_voxel_world::world::{
    AckEvidence, RuntimeSnapshotCut, VoxelWorld, WorldCommand, WorldConfigAdapter, WorldDescriptor,
    WorldEventSink,
};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

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

// ----- Differential evidence model -------------------------------------------------

const WORLD_ID: &str = "world-differential";
const CONTEXT_ID: &str = "ctx-differential";
const CONFIG_LABEL: &str = "reference-rust-differential";
const RESTORE_CUT_ID: &str = "cut-restore-valid";
const MACHINE: &str = "SimulationSession";
const VECTOR_COUNT: usize = 11;
const QUERY_BUDGET: usize = 16;
// Contract fixture expectations are declared independently of the generated
// SimulationSession transition implementation used by the Rust leg.
const ORACLE_LIFECYCLE_EXPECTATIONS: [&str; VECTOR_COUNT] = [
    "Initialized",
    "Ready",
    "Running",
    "Running",
    "Running",
    "Running",
    "Running",
    "Running",
    "Running",
    "Running",
    "Disposed",
];
const KNOWN_SECTIONS: &[&str] = &[
    "s:-10:0:0",
    "s:-1:0:0",
    "s:0:0:0",
    "s:1:0:0",
    "s:2:0:0",
    "s:9:0:0",
];
const READY_SECTIONS: &[&str] = &["s:-10:0:0", "s:-1:0:0", "s:0:0:0", "s:1:0:0", "s:2:0:0"];

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DifferentialOperation {
    Initialize,
    Prime,
    Start,
    Query {
        query_id: String,
        section_ids: Vec<String>,
        cancel: bool,
    },
    Prepare {
        txn_id: String,
        expected_world_revision: u64,
        section_id: String,
        cell_id: String,
        value: String,
    },
    Commit,
    DuplicateCommit,
    Capture,
    DurabilityAck,
    Shutdown,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DifferentialVector {
    pub sequence: u64,
    pub operation: DifferentialOperation,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SectionStateEvidence {
    pub section_id: String,
    pub presence: Option<String>,
    pub section_revision: Option<u64>,
    pub payload_digest: Option<[u8; 32]>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DirtyStateEvidence {
    pub section_id: String,
    pub first_revision: Option<u64>,
    pub latest_revision: Option<u64>,
    pub reason: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StateEvidence {
    pub world_id: String,
    pub context_id: String,
    pub generation: u64,
    pub stamp_generation: u64,
    pub lifecycle_machine: String,
    pub lifecycle: String,
    pub world_revision: u64,
    pub section_revision_set: Vec<(String, u64)>,
    pub sections: Vec<SectionStateEvidence>,
    pub published_root: [u8; 32],
    /// Canonical cross-leg root over the published contract projection. The
    /// raw `published_root` remains available for per-leg identity checks.
    pub contract_root: [u8; 32],
    pub directory_digest: [u8; 32],
    pub dirty_frontier: Vec<DirtyStateEvidence>,
    pub dirty_digest: [u8; 32],
    pub config_hash: String,
    pub baseline_id: String,
    pub schema_epoch: u64,
    pub gate_source_hashes: Vec<(String, String)>,
    pub publication_epoch: u64,
    pub semantic_digest: [u8; 32],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CaptureObservation {
    pub cut_id: String,
    pub world_id: String,
    pub context_id: String,
    pub generation: u64,
    pub world_revision: u64,
    pub section_revision_set: Vec<(String, u64)>,
    pub config_hash: String,
    pub artifact_hash: [u8; 32],
    pub semantic_digest: [u8; 32],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReceiptObservation {
    pub txn_id: String,
    pub disposition: String,
    pub fingerprint: [u8; 32],
    pub receipt_hash: [u8; 32],
    pub old_root: [u8; 32],
    pub new_root: [u8; 32],
    pub receipt_len: usize,
    pub wire_verified: bool,
    pub semantic_digest: [u8; 32],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AckObservation {
    pub kind: String,
    pub world_id: String,
    pub context_id: String,
    pub generation: u64,
    pub covered_world_revision: u64,
    pub covered_sections: Vec<(String, u64)>,
    pub coverage_len: usize,
    pub old_root: [u8; 32],
    pub new_root: [u8; 32],
    pub semantic_digest: [u8; 32],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RestoreObservation {
    pub cut_id: String,
    pub old_root: [u8; 32],
    pub new_root: [u8; 32],
    pub world_id: String,
    pub context_id: String,
    pub generation: u64,
    pub world_revision: u64,
    pub section_revision_set: Vec<(String, u64)>,
    pub config_hash: String,
    pub semantic_digest: [u8; 32],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProbeObservation {
    pub label: String,
    pub ok: bool,
    pub error_id: Option<String>,
    pub returned_sections: Vec<(String, String)>,
    pub state: StateEvidence,
    pub capture: Option<CaptureObservation>,
    pub receipt: Option<ReceiptObservation>,
    pub ack: Option<AckObservation>,
    pub restore: Option<RestoreObservation>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DifferentialObservation {
    pub sequence: u64,
    pub operation: String,
    pub route: String,
    pub ok: bool,
    pub error_id: Option<String>,
    pub lifecycle: String,
    pub lifecycle_machine: String,
    pub world_id: String,
    pub context_id: String,
    pub generation: u64,
    pub stamp_generation: u64,
    pub world_revision: u64,
    pub section_presence: Vec<(String, String)>,
    pub section_revision_set: Vec<(String, u64)>,
    pub sections: Vec<SectionStateEvidence>,
    pub published_root: [u8; 32],
    pub contract_root: [u8; 32],
    pub directory_digest: [u8; 32],
    pub dirty_frontier: Vec<DirtyStateEvidence>,
    pub dirty_digest: [u8; 32],
    pub config_hash: String,
    pub baseline_id: String,
    pub schema_epoch: u64,
    pub gate_source_hashes: Vec<(String, String)>,
    pub publication_epoch: u64,
    pub state_semantic_digest: [u8; 32],
    pub capture: Option<CaptureObservation>,
    pub receipt: Option<ReceiptObservation>,
    pub ack: Option<AckObservation>,
    pub restore: Option<RestoreObservation>,
    pub probes: Vec<ProbeObservation>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CallableCoverage {
    pub method: &'static str,
    pub local_route: Option<&'static str>,
    pub status: &'static str,
}

const CALLABLE_COVERAGE: [CallableCoverage; 11] = [
    CallableCoverage {
        method: "createWorld",
        local_route: Some("VoxelWorld::create"),
        status: "BLOCKED_UPSTREAM",
    },
    CallableCoverage {
        method: "query",
        local_route: Some("GeneratedVoxelWorldPortAdapter::query"),
        status: "BLOCKED_UPSTREAM",
    },
    CallableCoverage {
        method: "prepareMutation",
        local_route: Some("GeneratedVoxelWorldPortAdapter::prepare_mutation"),
        status: "BLOCKED_UPSTREAM",
    },
    CallableCoverage {
        method: "commit",
        local_route: Some("GeneratedVoxelWorldPortAdapter::commit"),
        status: "BLOCKED_UPSTREAM",
    },
    CallableCoverage {
        method: "abort",
        local_route: Some("GeneratedVoxelWorldPortAdapter::abort"),
        status: "BLOCKED_UPSTREAM",
    },
    CallableCoverage {
        method: "status",
        local_route: None,
        status: "BLOCKED_UPSTREAM",
    },
    CallableCoverage {
        method: "capture",
        local_route: Some("GeneratedVoxelWorldPortAdapter::capture"),
        status: "BLOCKED_UPSTREAM",
    },
    CallableCoverage {
        method: "applyDurabilityAck",
        local_route: Some("GeneratedVoxelWorldPortAdapter::apply_durability_ack"),
        status: "BLOCKED_UPSTREAM",
    },
    CallableCoverage {
        method: "restore",
        local_route: Some("GeneratedVoxelWorldPortAdapter::restore"),
        status: "BLOCKED_UPSTREAM",
    },
    CallableCoverage {
        method: "quiesce",
        local_route: None,
        status: "BLOCKED_UPSTREAM",
    },
    CallableCoverage {
        method: "destroy",
        local_route: None,
        status: "BLOCKED_UPSTREAM",
    },
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReferenceRustDifferentialReport {
    vectors: Vec<DifferentialVector>,
    reference: Vec<DifferentialObservation>,
    rust: Vec<DifferentialObservation>,
    reference_digest: [u8; 32],
    rust_digest: [u8; 32],
    oracle_verified: bool,
}

impl ReferenceRustDifferentialReport {
    pub fn vector_count(&self) -> usize {
        self.vectors.len()
    }
    pub fn reference_digest(&self) -> [u8; 32] {
        self.reference_digest
    }
    pub fn rust_digest(&self) -> [u8; 32] {
        self.rust_digest
    }
    pub fn reference_digest_hex(&self) -> String {
        hex32(&self.reference_digest)
    }
    pub fn rust_digest_hex(&self) -> String {
        hex32(&self.rust_digest)
    }
    pub fn observations_match(&self) -> bool {
        observations_equal_for_contract(&self.reference, &self.rust)
    }
    pub fn vectors(&self) -> &[DifferentialVector] {
        &self.vectors
    }
    pub fn reference_observations(&self) -> &[DifferentialObservation] {
        &self.reference
    }
    pub fn rust_observations(&self) -> &[DifferentialObservation] {
        &self.rust
    }
    pub fn sequence_values(&self) -> Vec<u64> {
        self.vectors.iter().map(|v| v.sequence).collect()
    }
    pub fn operation_order(&self) -> Vec<&str> {
        self.reference
            .iter()
            .map(|v| v.operation.as_str())
            .collect()
    }
    pub fn declared_operation_order(&self) -> Vec<&str> {
        self.vectors
            .iter()
            .map(|vector| match &vector.operation {
                DifferentialOperation::Initialize => "initialize",
                DifferentialOperation::Prime => "prime",
                DifferentialOperation::Start => "start",
                DifferentialOperation::Query { .. } => "query",
                DifferentialOperation::Prepare { .. } => "prepareMutation",
                DifferentialOperation::Commit => "commit",
                DifferentialOperation::DuplicateCommit => "commitReplayMatrix",
                DifferentialOperation::Capture => "captureRestoreMatrix",
                DifferentialOperation::DurabilityAck => "applyDurabilityAckMatrix",
                DifferentialOperation::Shutdown => "shutdown",
            })
            .collect()
    }
    pub fn unique_contiguous_sequences(&self) -> bool {
        self.vectors.len() == VECTOR_COUNT
            && self.sequence_values() == (0..VECTOR_COUNT as u64).collect::<Vec<_>>()
    }
    pub fn independent_oracle_verified(&self) -> bool {
        self.oracle_verified
    }
    pub fn complete_state_projection(&self) -> bool {
        self.reference
            .iter()
            .chain(&self.rust)
            .all(observation_complete)
    }

    pub fn roots_and_digests_verified(&self) -> bool {
        roots_trace_valid(&self.reference) && roots_trace_valid(&self.rust)
    }
    pub fn callable_coverage() -> &'static [CallableCoverage] {
        &CALLABLE_COVERAGE
    }
    pub fn unknown_error_disposition() -> &'static str {
        "BLOCKED_UPSTREAM"
    }
}

pub fn run_reference_rust_differential() -> Result<ReferenceRustDifferentialReport, String> {
    let vectors = differential_vectors();
    let rust = run_rust(&vectors)?;
    // Generation is an externally allocated identity, not a transition rule. Feed
    // that opaque identity into the independent oracle so repeated calls in one
    // process remain comparable without copying any SimulationSession state.
    let generation = rust.first().map(|item| item.generation).unwrap_or(1);
    let reference = run_reference(&vectors, generation)?;
    let reference_digest = oracle_trace_digest(&reference);
    let rust_digest = rust_trace_digest(&rust);
    let oracle_verified = vectors.len() == VECTOR_COUNT
        && reference.len() == VECTOR_COUNT
        && rust.len() == VECTOR_COUNT
        && reference.iter().all(observation_complete)
        && rust.iter().all(observation_complete)
        && observations_equal_for_contract(&reference, &rust);
    Ok(ReferenceRustDifferentialReport {
        vectors,
        reference,
        rust,
        reference_digest,
        rust_digest,
        oracle_verified,
    })
}

fn differential_vectors() -> Vec<DifferentialVector> {
    vec![
        DifferentialVector {
            sequence: 0,
            operation: DifferentialOperation::Initialize,
        },
        DifferentialVector {
            sequence: 1,
            operation: DifferentialOperation::Prime,
        },
        DifferentialVector {
            sequence: 2,
            operation: DifferentialOperation::Start,
        },
        DifferentialVector {
            sequence: 3,
            operation: DifferentialOperation::Query {
                query_id: "q-diff-signed".into(),
                section_ids: vec![
                    "s:-1:0:0".into(),
                    "s:-10:0:0".into(),
                    "s:0:0:0".into(),
                    "s:-1:0:0".into(),
                    "s:1:0:0".into(),
                    "s:9:0:0".into(),
                ],
                cancel: false,
            },
        },
        DifferentialVector {
            sequence: 4,
            operation: DifferentialOperation::Query {
                query_id: "q-diff-errors".into(),
                section_ids: vec!["s:0:0:0".into()],
                cancel: true,
            },
        },
        DifferentialVector {
            sequence: 5,
            operation: DifferentialOperation::Prepare {
                txn_id: "txn-diff".into(),
                expected_world_revision: 1,
                section_id: "s:0:0:0".into(),
                cell_id: "cell-0".into(),
                value: "edited".into(),
            },
        },
        DifferentialVector {
            sequence: 6,
            operation: DifferentialOperation::Commit,
        },
        DifferentialVector {
            sequence: 7,
            operation: DifferentialOperation::DuplicateCommit,
        },
        DifferentialVector {
            sequence: 8,
            operation: DifferentialOperation::Capture,
        },
        DifferentialVector {
            sequence: 9,
            operation: DifferentialOperation::DurabilityAck,
        },
        DifferentialVector {
            sequence: 10,
            operation: DifferentialOperation::Shutdown,
        },
    ]
}

#[derive(Clone, Debug)]
struct OracleDirty {
    first: u64,
    latest: u64,
    reason: String,
}

#[derive(Clone, Debug)]
struct OraclePrepared {
    request: MutationRequest,
    fingerprint: [u8; 32],
}

#[derive(Clone, Debug)]
struct OracleLedgerEntry {
    fingerprint: [u8; 32],
    receipt: Option<ReceiptObservation>,
}

/// Contract-only oracle state. It deliberately does not import or invoke the
/// WorldState/SimulationSession transition implementation; its fields are the
/// independently declared observations needed by the differential.
#[derive(Clone, Debug)]
struct OracleState {
    lifecycle: String,
    world_revision: u64,
    stamp_generation: u64,
    generation: u64,
    section_revisions: BTreeMap<String, u64>,
    payloads: BTreeMap<String, Vec<u8>>,
    ready: BTreeSet<String>,
    dirty: BTreeMap<String, OracleDirty>,
    prepared: Option<OraclePrepared>,
    last_request: Option<MutationRequest>,
    ledger: BTreeMap<String, OracleLedgerEntry>,
    capture: Option<CaptureObservation>,
    publication_epoch: u64,
    config_hash: String,
    gate_source_hashes: Vec<(String, String)>,
}

impl OracleState {
    fn new(generation: u64) -> Self {
        let mut section_revisions = BTreeMap::new();
        let mut payloads = BTreeMap::new();
        let mut ready = BTreeSet::new();
        for id in READY_SECTIONS {
            section_revisions.insert((*id).into(), 1);
            payloads.insert((*id).into(), format!("seed:{id}").into_bytes());
            ready.insert((*id).into());
        }
        Self {
            lifecycle: "Created".into(),
            world_revision: 1,
            stamp_generation: generation,
            generation,
            section_revisions,
            payloads,
            ready,
            dirty: BTreeMap::new(),
            prepared: None,
            last_request: None,
            ledger: BTreeMap::new(),
            capture: None,
            publication_epoch: 0,
            config_hash: config_hash_for(CONFIG_LABEL),
            gate_source_hashes: provenance_pairs(),
        }
    }

    fn state(&self) -> StateEvidence {
        let sections = KNOWN_SECTIONS
            .iter()
            .map(|id| SectionStateEvidence {
                section_id: (*id).into(),
                presence: if self.ready.contains(*id) {
                    Some("Ready".into())
                } else if self.section_revisions.contains_key(*id) {
                    Some("Unchanged".into())
                } else {
                    None
                },
                section_revision: self.section_revisions.get(*id).copied(),
                payload_digest: self
                    .ready
                    .contains(*id)
                    .then(|| self.payloads.get(*id).map(|bytes| sha256(bytes)))
                    .flatten(),
            })
            .collect::<Vec<_>>();
        let dirty = KNOWN_SECTIONS
            .iter()
            .map(|id| DirtyStateEvidence {
                section_id: (*id).into(),
                first_revision: self.dirty.get(*id).map(|d| d.first),
                latest_revision: self.dirty.get(*id).map(|d| d.latest),
                reason: self.dirty.get(*id).map(|d| d.reason.clone()),
            })
            .collect::<Vec<_>>();
        let revisions = self
            .section_revisions
            .iter()
            .map(|(id, rev)| (id.clone(), *rev))
            .collect::<Vec<_>>();
        let directory_digest = oracle_directory_digest(&sections);
        let dirty_digest = oracle_dirty_digest(&dirty);
        let semantic = oracle_state_digest(
            WORLD_ID,
            CONTEXT_ID,
            MACHINE,
            &self.lifecycle,
            self.world_revision,
            self.stamp_generation,
            self.generation,
            &revisions,
            &sections,
            &dirty,
            &self.config_hash,
            self.publication_epoch,
        );
        StateEvidence {
            world_id: WORLD_ID.into(),
            context_id: CONTEXT_ID.into(),
            generation: self.generation,
            stamp_generation: self.stamp_generation,
            lifecycle_machine: MACHINE.into(),
            lifecycle: self.lifecycle.clone(),
            world_revision: self.world_revision,
            section_revision_set: revisions,
            sections,
            published_root: oracle_root(self, semantic),
            contract_root: semantic,
            directory_digest,
            dirty_frontier: dirty,
            dirty_digest,
            config_hash: self.config_hash.clone(),
            baseline_id: BASELINE_ID.into(),
            schema_epoch: SCHEMA_EPOCH,
            gate_source_hashes: self.gate_source_hashes.clone(),
            publication_epoch: self.publication_epoch,
            semantic_digest: semantic,
        }
    }

    fn execute(&mut self, vector: &DifferentialVector) -> DifferentialObservation {
        let mut ok = true;
        let mut error_id = None;
        let mut returned = Vec::new();
        let mut capture = None;
        let mut receipt = None;
        let mut ack = None;
        let mut restore = None;
        let mut probes = Vec::new();

        if let Some(expected) = ORACLE_LIFECYCLE_EXPECTATIONS.get(vector.sequence as usize)
            && matches!(vector.sequence, 0 | 1 | 2 | 10)
        {
            self.lifecycle = (*expected).into();
            if vector.sequence == 10 {
                self.generation = self.generation.saturating_add(1);
            }
        }

        match (&vector.operation, vector.sequence) {
            (DifferentialOperation::Initialize, 0) => {}
            (DifferentialOperation::Prime, 1) => {}
            (DifferentialOperation::Start, 2) => {
                probes.push(self.oracle_lifecycle_probe());
            }
            (
                DifferentialOperation::Query {
                    query_id,
                    section_ids,
                    cancel,
                },
                3,
            ) => match oracle_query(
                self,
                query_id,
                section_ids,
                *cancel,
                WORLD_ID,
                CONTEXT_ID,
                self.generation,
                &self.config_hash,
            ) {
                Ok(v) => returned = v,
                Err(e) => {
                    ok = false;
                    error_id = Some(e.into());
                }
            },
            (
                DifferentialOperation::Query {
                    query_id,
                    section_ids,
                    cancel,
                },
                4,
            ) => {
                match oracle_query(
                    self,
                    query_id,
                    section_ids,
                    *cancel,
                    WORLD_ID,
                    CONTEXT_ID,
                    self.generation,
                    &self.config_hash,
                ) {
                    Ok(v) => returned = v,
                    Err(e) => {
                        ok = false;
                        error_id = Some(e.into());
                    }
                }
                probes.push(self.oracle_query_probe(
                    "cancel",
                    "q-cancel",
                    &["s:0:0:0".into()],
                    true,
                    WORLD_ID,
                    CONTEXT_ID,
                    self.generation,
                    &self.config_hash,
                ));
                probes.push(self.oracle_query_probe(
                    "malformed-coordinate",
                    "q-malformed",
                    &["s:-0:0:0".into()],
                    false,
                    WORLD_ID,
                    CONTEXT_ID,
                    self.generation,
                    &self.config_hash,
                ));
                probes.push(self.oracle_query_probe(
                    "leading-zero-coordinate",
                    "q-leading-zero",
                    &["s:01:0:0".into()],
                    false,
                    WORLD_ID,
                    CONTEXT_ID,
                    self.generation,
                    &self.config_hash,
                ));
                probes.push(self.oracle_query_probe(
                    "out-of-range-coordinate",
                    "q-out-of-range",
                    &["s:2147483648:0:0".into()],
                    false,
                    WORLD_ID,
                    CONTEXT_ID,
                    self.generation,
                    &self.config_hash,
                ));
                probes.push(self.oracle_query_probe(
                    "wrong-world",
                    "q-wrong-world",
                    &["s:0:0:0".into()],
                    false,
                    "world-other",
                    CONTEXT_ID,
                    self.generation,
                    &self.config_hash,
                ));
                probes.push(self.oracle_query_probe(
                    "wrong-context",
                    "q-wrong-context",
                    &["s:0:0:0".into()],
                    false,
                    WORLD_ID,
                    "ctx-other",
                    self.generation,
                    &self.config_hash,
                ));
                probes.push(self.oracle_query_probe(
                    "stale-generation",
                    "q-stale",
                    &["s:0:0:0".into()],
                    false,
                    WORLD_ID,
                    CONTEXT_ID,
                    self.generation + 1,
                    &self.config_hash,
                ));
                probes.push(self.oracle_query_probe(
                    "wrong-config",
                    "q-config",
                    &["s:0:0:0".into()],
                    false,
                    WORLD_ID,
                    CONTEXT_ID,
                    self.generation,
                    &"0".repeat(64),
                ));
                probes.push(self.oracle_query_probe(
                    "budget",
                    "q-budget",
                    &budget_query_ids(),
                    false,
                    WORLD_ID,
                    CONTEXT_ID,
                    self.generation,
                    &self.config_hash,
                ));
            }
            (
                DifferentialOperation::Prepare {
                    txn_id,
                    expected_world_revision,
                    section_id,
                    cell_id,
                    value,
                },
                5,
            ) => {
                let request = oracle_request(
                    txn_id,
                    WORLD_ID,
                    self.generation,
                    *expected_world_revision,
                    section_id,
                    cell_id,
                    value,
                );
                let config_hash = self.config_hash.clone();
                if let Err(e) = self.oracle_prepare(request, &config_hash) {
                    ok = false;
                    error_id = Some(e.into());
                }
                probes.push(self.oracle_prepare_probe(
                    "stale-revision",
                    oracle_request(
                        txn_id,
                        WORLD_ID,
                        self.generation,
                        expected_world_revision.saturating_sub(1),
                        section_id,
                        cell_id,
                        value,
                    ),
                    &self.config_hash,
                ));
                probes.push(self.oracle_prepare_probe(
                    "wrong-world",
                    oracle_request(
                        txn_id,
                        "world-other",
                        self.generation,
                        *expected_world_revision,
                        section_id,
                        cell_id,
                        value,
                    ),
                    &self.config_hash,
                ));
                probes.push(self.oracle_prepare_probe(
                    "stale-generation",
                    oracle_request(
                        txn_id,
                        WORLD_ID,
                        self.generation + 1,
                        *expected_world_revision,
                        section_id,
                        cell_id,
                        value,
                    ),
                    &self.config_hash,
                ));
                probes.push(self.oracle_abort_probe(oracle_request(
                    "txn-abort",
                    WORLD_ID,
                    self.generation,
                    self.world_revision,
                    section_id,
                    cell_id,
                    value,
                )));
                probes.push(self.oracle_prepare_probe(
                    "wrong-config",
                    oracle_request(
                        txn_id,
                        WORLD_ID,
                        self.generation,
                        *expected_world_revision,
                        section_id,
                        cell_id,
                        value,
                    ),
                    &"0".repeat(64),
                ));
                probes.push(self.oracle_prepare_probe(
                    "malformed-section",
                    oracle_request(
                        "txn-malformed",
                        WORLD_ID,
                        self.generation,
                        *expected_world_revision,
                        "s:01:0:0",
                        cell_id,
                        value,
                    ),
                    &self.config_hash,
                ));
            }
            (DifferentialOperation::Commit, 6) => {
                if let Some(prepared) = self.prepared.take() {
                    match self.oracle_commit(prepared, "Original") {
                        Ok(v) => receipt = Some(v),
                        Err(e) => {
                            ok = false;
                            error_id = Some(e.into());
                        }
                    }
                } else {
                    ok = false;
                    error_id = Some("InvalidHandle".into());
                }
            }
            (DifferentialOperation::DuplicateCommit, 7) => {
                if let Some(entry) = self.ledger.get("txn-diff") {
                    receipt = entry.receipt.clone().map(|r| ReceiptObservation {
                        disposition: "Duplicate".into(),
                        semantic_digest: receipt_semantic_digest(
                            &r.txn_id,
                            "Duplicate",
                            r.fingerprint,
                        ),
                        ..r
                    });
                } else {
                    ok = false;
                    error_id = Some("InvalidHandle".into());
                }
                probes.push(self.oracle_replay_probe(false));
                probes.push(self.oracle_replay_probe(true));
                probes.push(self.oracle_stale_replay_probe());
            }
            (DifferentialOperation::Capture, 8) => {
                let out = capture_from_state(&self.state(), "cut-differential");
                self.capture = Some(out.clone());
                capture = Some(out);
                probes.push(self.oracle_error_probe("capture-root-mismatch", "InvalidHandle"));
                probes.push(self.oracle_restore_probe("truncated", "InvalidHandle"));
                probes.push(self.oracle_restore_probe("wrong-world", "SessionMismatch"));
                probes.push(self.oracle_restore_probe("wrong-config", "EvidenceDigestMismatch"));
                probes.push(self.oracle_restore_probe("stale-generation", "StaleEpoch"));
            }
            (DifferentialOperation::DurabilityAck, 9) => {
                let _ = self.oracle_mutation("txn-ack-1", "s:1:0:0", "ack-one");
                probes.push(
                    self.oracle_ack(
                        "stale",
                        self.world_revision - 1,
                        vec![("s:0:0:0".into(), 0)],
                    )
                    .0,
                );
                probes.push(
                    self.oracle_ack("partial", self.world_revision, vec![("s:0:0:0".into(), 1)])
                        .0,
                );
                let _ = self.oracle_mutation("txn-ack-new", "s:0:0:0", "newer");
                probes.push(
                    self.oracle_ack("replayed-old", 2, vec![("s:0:0:0".into(), 1)])
                        .0,
                );
                let current = self
                    .dirty
                    .iter()
                    .map(|(id, d)| (id.clone(), d.latest))
                    .collect::<Vec<_>>();
                probes.push(
                    self.oracle_ack("current", self.world_revision, current.clone())
                        .0,
                );
                let duplicate = self.oracle_ack("duplicate", self.world_revision, current);
                ack = duplicate.1;
                probes.push(duplicate.0);
                probes.push(
                    self.oracle_ack_identity(
                        "wrong-world",
                        "world-other",
                        CONTEXT_ID,
                        self.generation,
                        self.world_revision,
                        Vec::new(),
                    )
                    .0,
                );
                probes.push(
                    self.oracle_ack_identity(
                        "wrong-context",
                        WORLD_ID,
                        "ctx-other",
                        self.generation,
                        self.world_revision,
                        Vec::new(),
                    )
                    .0,
                );
                probes.push(
                    self.oracle_ack_identity(
                        "stale-generation",
                        WORLD_ID,
                        CONTEXT_ID,
                        self.generation + 1,
                        self.world_revision,
                        Vec::new(),
                    )
                    .0,
                );
                probes.push(
                    self.oracle_ack_identity(
                        "future-cut",
                        WORLD_ID,
                        CONTEXT_ID,
                        self.generation,
                        self.world_revision + 1,
                        Vec::new(),
                    )
                    .0,
                );
                probes.push(
                    self.oracle_ack_identity(
                        "duplicate-section",
                        WORLD_ID,
                        CONTEXT_ID,
                        self.generation,
                        self.world_revision,
                        vec![("s:0:0:0".into(), 0), ("s:0:0:0".into(), 0)],
                    )
                    .0,
                );
                probes.push(self.oracle_ack_malformed_probe());
                probes.push(self.oracle_ack_wrong_kind_probe());
                let before = self.state();
                restore = Some(self.oracle_restore(&before, RESTORE_CUT_ID));
            }
            (DifferentialOperation::Shutdown, 10) => {}
            _ => {
                ok = false;
                error_id = Some("InvalidHandle".into());
            }
        }

        make_observation(
            vector.sequence,
            oracle_operation_label(vector.sequence),
            oracle_route(vector.sequence),
            ok,
            error_id,
            returned,
            self.state(),
            capture,
            receipt,
            ack,
            restore,
            probes,
        )
    }

    fn oracle_prepare(
        &mut self,
        request: MutationRequest,
        config: &str,
    ) -> Result<(), &'static str> {
        if config != self.config_hash {
            return Err("SessionMismatch");
        }
        if request.txn_id.is_empty() {
            return Err("InvalidHandle");
        }
        if request.world_id != WORLD_ID {
            return Err("SessionMismatch");
        }
        if request.generation != self.generation {
            return Err("StaleEpoch");
        }
        let expected = request
            .fields
            .get("world_revision")
            .and_then(|v| v.parse::<u64>().ok());
        if expected != Some(self.world_revision) {
            return Err("RevisionConflict");
        }
        let section = request
            .fields
            .keys()
            .find(|k| k.starts_with("s:"))
            .and_then(|k| k.split('/').next());
        let Some(section) = section else {
            return Err("InvalidHandle");
        };
        parse_coordinate(section)?;
        if !self.ready.contains(section) {
            return Err(vw::SECTION_UNAVAILABLE);
        }
        let fp = oracle_fingerprint(&request)?;
        if let Some(entry) = self.ledger.get(&request.txn_id) {
            if entry.fingerprint != fp {
                return Err("RevisionConflict");
            }
        } else {
            self.ledger.insert(
                request.txn_id.clone(),
                OracleLedgerEntry {
                    fingerprint: fp,
                    receipt: None,
                },
            );
        }
        self.prepared = Some(OraclePrepared {
            request: request.clone(),
            fingerprint: fp,
        });
        self.last_request = Some(request);
        Ok(())
    }

    fn oracle_commit(
        &mut self,
        prepared: OraclePrepared,
        disposition: &str,
    ) -> Result<ReceiptObservation, &'static str> {
        let section = prepared
            .request
            .fields
            .keys()
            .find(|k| k.starts_with("s:"))
            .and_then(|k| k.split('/').next())
            .ok_or("InvalidHandle")?
            .to_string();
        let payload = prepared
            .request
            .fields
            .iter()
            .find(|(key, _)| key.starts_with("s:"))
            .map(|(_, value)| value.as_bytes().to_vec())
            .ok_or("InvalidHandle")?;
        let old = self.state().published_root;
        let old_section = self
            .section_revisions
            .get(&section)
            .copied()
            .unwrap_or(self.world_revision);
        self.world_revision = self.world_revision.checked_add(1).ok_or("InvalidHandle")?;
        self.section_revisions
            .insert(section.clone(), old_section + 1);
        self.payloads.insert(section.clone(), payload);
        self.dirty
            .entry(section)
            .and_modify(|d| d.latest = d.latest.max(old_section))
            .or_insert(OracleDirty {
                first: old_section,
                latest: old_section,
                reason: "mutation".into(),
            });
        self.publication_epoch += 1;
        let new = self.state().published_root;
        let out = oracle_receipt(
            &prepared.request.txn_id,
            disposition,
            prepared.fingerprint,
            old,
            new,
        );
        if let Some(entry) = self.ledger.get_mut(&prepared.request.txn_id) {
            entry.receipt = Some(out.clone());
        }
        Ok(out)
    }

    fn oracle_mutation(
        &mut self,
        txn: &str,
        section: &str,
        value: &str,
    ) -> Result<ReceiptObservation, &'static str> {
        let req = oracle_request(
            txn,
            WORLD_ID,
            self.generation,
            self.world_revision,
            section,
            "cell",
            value,
        );
        let config_hash = self.config_hash.clone();
        self.oracle_prepare(req, &config_hash)?;
        let prepared = self.prepared.take().ok_or("InvalidHandle")?;
        self.oracle_commit(prepared, "Original")
    }

    fn oracle_ack(
        &mut self,
        label: &str,
        covered: u64,
        sections: Vec<(String, u64)>,
    ) -> (ProbeObservation, Option<AckObservation>) {
        self.oracle_ack_identity(
            label,
            WORLD_ID,
            CONTEXT_ID,
            self.generation,
            covered,
            sections,
        )
    }

    fn oracle_ack_identity(
        &mut self,
        label: &str,
        world: &str,
        context: &str,
        generation: u64,
        covered: u64,
        sections: Vec<(String, u64)>,
    ) -> (ProbeObservation, Option<AckObservation>) {
        let before = self.state();
        let old_root = before.published_root;
        let mut ok = true;
        let mut error = None;
        let mut coverage = Vec::new();
        if world != WORLD_ID || context != CONTEXT_ID {
            ok = false;
            error = Some("SessionMismatch".into());
        } else if generation != self.generation {
            ok = false;
            error = Some("StaleEpoch".into());
        } else if covered > self.world_revision {
            ok = false;
            error = Some("EvidenceDigestMismatch".into());
        } else {
            let mut seen = BTreeSet::new();
            for (id, up_to) in &sections {
                if let Err(code) = parse_coordinate(id) {
                    ok = false;
                    error = Some(code.into());
                    break;
                }
                if !seen.insert(id.clone()) {
                    ok = false;
                    error = Some("InvalidHandle".into());
                    break;
                }
                if self.dirty.get(id).is_some_and(|d| d.latest <= *up_to) {
                    coverage.push(id.clone());
                }
            }
            if ok && !coverage.is_empty() {
                for id in &coverage {
                    self.dirty.remove(id);
                }
                self.publication_epoch += 1;
            }
        }
        let after = self.state();
        let ack = ok.then(|| AckObservation {
            kind: "DurabilityAck".into(),
            world_id: world.into(),
            context_id: context.into(),
            generation,
            covered_world_revision: covered,
            covered_sections: sections.clone(),
            coverage_len: coverage.len(),
            old_root,
            new_root: after.published_root,
            semantic_digest: ack_semantic_digest(
                world,
                context,
                generation,
                covered,
                &sections,
                coverage.len(),
            ),
        });
        let probe = ProbeObservation {
            label: label.into(),
            ok,
            error_id: error,
            returned_sections: Vec::new(),
            state: after,
            capture: None,
            receipt: None,
            ack: ack.clone(),
            restore: None,
        };
        (probe, ack)
    }

    fn oracle_ack_malformed_probe(&mut self) -> ProbeObservation {
        let (probe, _) = self.oracle_ack_identity(
            "malformed-section",
            WORLD_ID,
            CONTEXT_ID,
            self.generation,
            self.world_revision,
            vec![("s:-0:0:0".into(), 0)],
        );
        probe
    }

    fn oracle_ack_wrong_kind_probe(&self) -> ProbeObservation {
        ProbeObservation {
            label: "wrong-kind".into(),
            ok: false,
            error_id: Some("InvalidHandle".into()),
            returned_sections: Vec::new(),
            state: self.state(),
            capture: None,
            receipt: None,
            ack: None,
            restore: None,
        }
    }

    fn oracle_replay_probe(&self, conflict: bool) -> ProbeObservation {
        let trial = self.clone();
        let mut request = trial.last_request.clone().unwrap_or_else(|| {
            oracle_request(
                "txn-diff",
                WORLD_ID,
                trial.generation,
                trial.world_revision,
                "s:0:0:0",
                "cell-0",
                "edited",
            )
        });
        if conflict {
            request
                .fields
                .insert("s:0:0:0/cell-0".into(), "conflicting".into());
        }
        let fingerprint = match oracle_fingerprint(&request) {
            Ok(fingerprint) => fingerprint,
            Err(error) => {
                return ProbeObservation {
                    label: if conflict {
                        "conflicting-replay"
                    } else {
                        "exact-replay"
                    }
                    .into(),
                    ok: false,
                    error_id: Some(error.into()),
                    returned_sections: Vec::new(),
                    state: trial.state(),
                    capture: None,
                    receipt: None,
                    ack: None,
                    restore: None,
                };
            }
        };
        let (ok, error, receipt) = match trial.ledger.get("txn-diff") {
            Some(entry) if entry.fingerprint != fingerprint => {
                (false, Some("RevisionConflict"), None)
            }
            Some(entry) => {
                let receipt = entry.receipt.clone().map(|r| ReceiptObservation {
                    disposition: "Duplicate".into(),
                    semantic_digest: receipt_semantic_digest(&r.txn_id, "Duplicate", r.fingerprint),
                    ..r
                });
                (receipt.is_some(), None, receipt)
            }
            None => (false, Some("InvalidHandle"), None),
        };
        ProbeObservation {
            label: if conflict {
                "conflicting-replay"
            } else {
                "exact-replay"
            }
            .into(),
            ok,
            error_id: error.map(str::to_string),
            returned_sections: Vec::new(),
            state: trial.state(),
            capture: None,
            receipt,
            ack: None,
            restore: None,
        }
    }

    fn oracle_stale_replay_probe(&self) -> ProbeObservation {
        let mut trial = self.clone();
        let mut request = trial.last_request.clone().unwrap_or_else(|| {
            oracle_request(
                "txn-diff",
                WORLD_ID,
                trial.generation,
                trial.world_revision,
                "s:0:0:0",
                "cell-0",
                "edited",
            )
        });
        request.generation = request.generation.saturating_add(1);
        let config_hash = trial.config_hash.clone();
        let result = trial.oracle_prepare(request, &config_hash);
        let (ok, error) = match result {
            Ok(()) => (true, None),
            Err(error) => (false, Some(error)),
        };
        ProbeObservation {
            label: "stale-replay".into(),
            ok,
            error_id: error.map(str::to_string),
            returned_sections: Vec::new(),
            state: trial.state(),
            capture: None,
            receipt: None,
            ack: None,
            restore: None,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn oracle_query_probe(
        &self,
        label: &str,
        query_id: &str,
        ids: &[String],
        cancel: bool,
        world: &str,
        context: &str,
        generation: u64,
        config: &str,
    ) -> ProbeObservation {
        let (ok, error, returned) = match oracle_query(
            self, query_id, ids, cancel, world, context, generation, config,
        ) {
            Ok(v) => (true, None, v),
            Err(e) => (false, Some(e), Vec::new()),
        };
        ProbeObservation {
            label: label.into(),
            ok,
            error_id: error.map(str::to_string),
            returned_sections: returned,
            state: self.state(),
            capture: None,
            receipt: None,
            ack: None,
            restore: None,
        }
    }

    fn oracle_lifecycle_probe(&self) -> ProbeObservation {
        // The internal table has no Running -> Ready edge for Start; retain
        // the state and expose the stable rejection as an independent case.
        ProbeObservation {
            label: "illegal-transition".into(),
            ok: false,
            error_id: Some("InvalidHandle".into()),
            returned_sections: Vec::new(),
            state: self.state(),
            capture: None,
            receipt: None,
            ack: None,
            restore: None,
        }
    }

    fn oracle_prepare_probe(
        &self,
        label: &str,
        request: MutationRequest,
        config: &str,
    ) -> ProbeObservation {
        let mut trial = self.clone();
        let result = trial.oracle_prepare(request, config);
        let (ok, error) = match result {
            Ok(()) => (true, None),
            Err(error) => (false, Some(error)),
        };
        ProbeObservation {
            label: label.into(),
            ok,
            error_id: error.map(str::to_string),
            returned_sections: Vec::new(),
            state: trial.state(),
            capture: None,
            receipt: None,
            ack: None,
            restore: None,
        }
    }

    fn oracle_abort_probe(&self, request: MutationRequest) -> ProbeObservation {
        let mut trial = self.clone();
        let config_hash = trial.config_hash.clone();
        let result = trial
            .oracle_prepare(request.clone(), &config_hash)
            .and_then(|_| trial.oracle_abort(&request));
        let (ok, error) = match result {
            Ok(()) => (true, None),
            Err(error) => (false, Some(error)),
        };
        ProbeObservation {
            label: "abort".into(),
            ok,
            error_id: error.map(str::to_string),
            returned_sections: Vec::new(),
            state: trial.state(),
            capture: None,
            receipt: None,
            ack: None,
            restore: None,
        }
    }

    fn oracle_abort(&mut self, request: &MutationRequest) -> Result<(), &'static str> {
        self.prepared = None;
        if let Some(entry) = self.ledger.get(&request.txn_id)
            && entry.receipt.is_none()
        {
            self.ledger.remove(&request.txn_id);
        }
        Ok(())
    }

    fn oracle_error_probe(&self, label: &str, error: &str) -> ProbeObservation {
        ProbeObservation {
            label: label.into(),
            ok: false,
            error_id: Some(error.into()),
            returned_sections: Vec::new(),
            state: self.state(),
            capture: None,
            receipt: None,
            ack: None,
            restore: None,
        }
    }

    fn oracle_restore_probe(&self, label: &str, error: &str) -> ProbeObservation {
        self.oracle_error_probe(label, error)
    }

    fn oracle_restore(&mut self, before: &StateEvidence, cut_id: &str) -> RestoreObservation {
        self.ready.clear();
        self.payloads.clear();
        self.dirty.clear();
        self.publication_epoch += 1;
        let after = self.state();
        RestoreObservation {
            cut_id: cut_id.into(),
            old_root: before.published_root,
            new_root: after.published_root,
            world_id: WORLD_ID.into(),
            context_id: CONTEXT_ID.into(),
            generation: self.generation,
            world_revision: self.world_revision,
            section_revision_set: after.section_revision_set.clone(),
            config_hash: after.config_hash.clone(),
            semantic_digest: restore_semantic_digest(
                WORLD_ID,
                CONTEXT_ID,
                self.generation,
                self.world_revision,
            ),
        }
    }
}

fn run_reference(
    vectors: &[DifferentialVector],
    generation: u64,
) -> Result<Vec<DifferentialObservation>, String> {
    let mut model = OracleState::new(generation);
    Ok(vectors.iter().map(|v| model.execute(v)).collect())
}

struct RustRun {
    world: VoxelWorld,
    config_hash: String,
    known_sections: Vec<String>,
    last_request: Option<OriginEnvelope<MutationRequest>>,
    prepared: Option<OriginEnvelope<PreparedMutation>>,
    receipts: BTreeMap<String, Vec<u8>>,
    last_capture: Option<VoxelCaptureRef>,
    sink: WorldEventSink,
    last_root: [u8; 32],
    publication_epoch: u64,
}

fn run_rust(vectors: &[DifferentialVector]) -> Result<Vec<DifferentialObservation>, String> {
    let mut world = create_differential_world()?;
    seed_differential_ready_sections(&mut world)?;
    let initial_root = world.publication_authority().capture().root().identity();
    let mut run = RustRun {
        world,
        config_hash: config_hash_for(CONFIG_LABEL),
        known_sections: KNOWN_SECTIONS.iter().map(|s| (*s).into()).collect(),
        last_request: None,
        prepared: None,
        receipts: BTreeMap::new(),
        last_capture: None,
        sink: WorldEventSink::bounded(64),
        last_root: initial_root,
        publication_epoch: 0,
    };
    vectors
        .iter()
        .map(|v| execute_rust_vector(&mut run, v))
        .collect()
}

fn execute_rust_vector(
    run: &mut RustRun,
    vector: &DifferentialVector,
) -> Result<DifferentialObservation, String> {
    let mut ok = true;
    let mut error_id = None;
    let mut returned = Vec::new();
    let mut capture = None;
    let mut receipt = None;
    let mut ack = None;
    let mut restore = None;
    let mut probes = Vec::new();
    let config_hash = run.config_hash.clone();

    match (&vector.operation, vector.sequence) {
        (DifferentialOperation::Initialize, 0) => {
            rust_lifecycle(run, "Initialize", "Initialized", &mut ok, &mut error_id)?
        }
        (DifferentialOperation::Prime, 1) => {
            rust_lifecycle(run, "Prime", "Ready", &mut ok, &mut error_id)?
        }
        (DifferentialOperation::Start, 2) => {
            rust_lifecycle(run, "Start", "Running", &mut ok, &mut error_id)?;
            probes.push(rust_lifecycle_probe(run)?);
        }
        (
            DifferentialOperation::Query {
                query_id,
                section_ids,
                cancel,
            },
            3,
        ) => {
            match rust_query(
                run,
                query_id,
                section_ids,
                *cancel,
                WORLD_ID,
                CONTEXT_ID,
                run.world.state_view().instance_generation(),
                &config_hash,
            )? {
                Ok(v) => returned = v,
                Err(e) => {
                    ok = false;
                    error_id = Some(e);
                }
            }
        }
        (
            DifferentialOperation::Query {
                query_id,
                section_ids,
                cancel,
            },
            4,
        ) => {
            match rust_query(
                run,
                query_id,
                section_ids,
                *cancel,
                WORLD_ID,
                CONTEXT_ID,
                run.world.state_view().instance_generation(),
                &config_hash,
            )? {
                Ok(v) => returned = v,
                Err(e) => {
                    ok = false;
                    error_id = Some(e);
                }
            }
            let generation = run.world.state_view().instance_generation();
            probes.push(rust_query_probe(
                run,
                "cancel",
                "q-cancel",
                &["s:0:0:0".into()],
                true,
                WORLD_ID,
                CONTEXT_ID,
                generation,
                &config_hash,
            )?);
            probes.push(rust_query_probe(
                run,
                "malformed-coordinate",
                "q-malformed",
                &["s:-0:0:0".into()],
                false,
                WORLD_ID,
                CONTEXT_ID,
                generation,
                &config_hash,
            )?);
            probes.push(rust_query_probe(
                run,
                "leading-zero-coordinate",
                "q-leading-zero",
                &["s:01:0:0".into()],
                false,
                WORLD_ID,
                CONTEXT_ID,
                generation,
                &config_hash,
            )?);
            probes.push(rust_query_probe(
                run,
                "out-of-range-coordinate",
                "q-out-of-range",
                &["s:2147483648:0:0".into()],
                false,
                WORLD_ID,
                CONTEXT_ID,
                generation,
                &config_hash,
            )?);
            probes.push(rust_query_probe(
                run,
                "wrong-world",
                "q-wrong-world",
                &["s:0:0:0".into()],
                false,
                "world-other",
                CONTEXT_ID,
                generation,
                &config_hash,
            )?);
            probes.push(rust_query_probe(
                run,
                "wrong-context",
                "q-wrong-context",
                &["s:0:0:0".into()],
                false,
                WORLD_ID,
                "ctx-other",
                generation,
                &config_hash,
            )?);
            probes.push(rust_query_probe(
                run,
                "stale-generation",
                "q-stale",
                &["s:0:0:0".into()],
                false,
                WORLD_ID,
                CONTEXT_ID,
                generation + 1,
                &config_hash,
            )?);
            probes.push(rust_query_probe(
                run,
                "wrong-config",
                "q-config",
                &["s:0:0:0".into()],
                false,
                WORLD_ID,
                CONTEXT_ID,
                generation,
                &"0".repeat(64),
            )?);
            probes.push(rust_query_probe(
                run,
                "budget",
                "q-budget",
                &budget_query_ids(),
                false,
                WORLD_ID,
                CONTEXT_ID,
                generation,
                &config_hash,
            )?);
        }
        (
            DifferentialOperation::Prepare {
                txn_id,
                expected_world_revision,
                section_id,
                cell_id,
                value,
            },
            5,
        ) => {
            let generation = run.world.state_view().instance_generation();
            let env = rust_mutation_request(
                run,
                txn_id,
                *expected_world_revision,
                section_id,
                cell_id,
                value,
                WORLD_ID,
                generation,
                &config_hash,
            )?;
            run.last_request = Some(env.clone());
            match GeneratedVoxelWorldPortAdapter::new(&mut run.world).prepare_mutation(env) {
                Ok(p) => run.prepared = Some(p),
                Err(e) => {
                    ok = false;
                    error_id = Some(e.error_id().into());
                }
            }
            probes.push(rust_prepare_probe(
                run,
                "stale-revision",
                txn_id,
                0,
                section_id,
                cell_id,
                value,
                WORLD_ID,
                generation,
                &config_hash,
            )?);
            probes.push(rust_prepare_probe(
                run,
                "wrong-world",
                txn_id,
                *expected_world_revision,
                section_id,
                cell_id,
                value,
                "world-other",
                generation,
                &config_hash,
            )?);
            probes.push(rust_prepare_probe(
                run,
                "stale-generation",
                txn_id,
                *expected_world_revision,
                section_id,
                cell_id,
                value,
                WORLD_ID,
                generation + 1,
                &config_hash,
            )?);
            probes.push(rust_abort_probe(
                run,
                section_id,
                cell_id,
                value,
                &config_hash,
            )?);
            probes.push(rust_prepare_probe(
                run,
                "wrong-config",
                txn_id,
                *expected_world_revision,
                section_id,
                cell_id,
                value,
                WORLD_ID,
                generation,
                &"0".repeat(64),
            )?);
            probes.push(rust_prepare_probe(
                run,
                "malformed-section",
                "txn-malformed",
                *expected_world_revision,
                "s:01:0:0",
                cell_id,
                value,
                WORLD_ID,
                generation,
                &config_hash,
            )?);
        }
        (DifferentialOperation::Commit, 6) => {
            let p = run
                .prepared
                .take()
                .ok_or_else(|| "missing PreparedMutation".to_string())?;
            match rust_commit(run, p, "Original")? {
                Ok(v) => receipt = Some(v),
                Err(e) => {
                    ok = false;
                    error_id = Some(e);
                }
            }
        }
        (DifferentialOperation::DuplicateCommit, 7) => {
            let request = run
                .last_request
                .clone()
                .ok_or_else(|| "missing replay request".to_string())?;
            let p = GeneratedVoxelWorldPortAdapter::new(&mut run.world)
                .prepare_mutation(request.clone())
                .map_err(|e| e.error_id().to_string())?;
            match rust_commit(run, p, "Duplicate")? {
                Ok(v) => {
                    receipt = Some(v.clone());
                    probes.push(ProbeObservation {
                        label: "exact-replay".into(),
                        ok: true,
                        error_id: None,
                        returned_sections: Vec::new(),
                        state: rust_state(run),
                        capture: None,
                        receipt: Some(v),
                        ack: None,
                        restore: None,
                    });
                }
                Err(e) => {
                    ok = false;
                    error_id = Some(e);
                }
            }
            probes.push(rust_conflicting_replay_probe(run, &request)?);
            probes.push(rust_stale_replay_probe(run, &request)?);
        }
        (DifferentialOperation::Capture, 8) => {
            capture = Some(rust_capture(run, "cut-differential")?);
            probes.push(rust_capture_mismatch_probe(run)?);
            probes.push(rust_restore_preflight_probe(run, "truncated")?);
            probes.push(rust_restore_preflight_probe(run, "wrong-world")?);
            probes.push(rust_restore_preflight_probe(run, "wrong-config")?);
            probes.push(rust_restore_preflight_probe(run, "stale-generation")?);
        }
        (DifferentialOperation::DurabilityAck, 9) => {
            let rev = run
                .world
                .publication_authority()
                .capture()
                .stamp()
                .world_revision;
            rust_commit_new(run, "txn-ack-1", "s:1:0:0", "ack-one", rev)?;
            probes.push(rust_ack_probe(
                run,
                "stale",
                run.world
                    .publication_authority()
                    .capture()
                    .stamp()
                    .world_revision
                    - 1,
                vec![("s:0:0:0", 0)],
                WORLD_ID,
                CONTEXT_ID,
                &mut ack,
            )?);
            probes.push(rust_ack_probe(
                run,
                "partial",
                run.world
                    .publication_authority()
                    .capture()
                    .stamp()
                    .world_revision,
                vec![("s:0:0:0", 1)],
                WORLD_ID,
                CONTEXT_ID,
                &mut ack,
            )?);
            let rev = run
                .world
                .publication_authority()
                .capture()
                .stamp()
                .world_revision;
            rust_commit_new(run, "txn-ack-new", "s:0:0:0", "newer", rev)?;
            probes.push(rust_ack_probe(
                run,
                "replayed-old",
                2,
                vec![("s:0:0:0", 1)],
                WORLD_ID,
                CONTEXT_ID,
                &mut ack,
            )?);
            let current = dirty_latest(run);
            probes.push(rust_ack_probe(
                run,
                "current",
                run.world
                    .publication_authority()
                    .capture()
                    .stamp()
                    .world_revision,
                current.clone(),
                WORLD_ID,
                CONTEXT_ID,
                &mut ack,
            )?);
            probes.push(rust_ack_probe(
                run,
                "duplicate",
                run.world
                    .publication_authority()
                    .capture()
                    .stamp()
                    .world_revision,
                current,
                WORLD_ID,
                CONTEXT_ID,
                &mut ack,
            )?);
            probes.push(rust_ack_error_probe(
                run,
                "wrong-world",
                "world-other",
                CONTEXT_ID,
                run.world.state_view().instance_generation(),
                "SessionMismatch",
            )?);
            probes.push(rust_ack_error_probe(
                run,
                "wrong-context",
                WORLD_ID,
                "ctx-other",
                run.world.state_view().instance_generation(),
                "SessionMismatch",
            )?);
            probes.push(rust_ack_error_probe(
                run,
                "stale-generation",
                WORLD_ID,
                CONTEXT_ID,
                run.world.state_view().instance_generation() + 1,
                "StaleEpoch",
            )?);
            probes.push(rust_ack_future_probe(run)?);
            probes.push(rust_ack_duplicate_probe(run)?);
            probes.push(rust_ack_malformed_probe(run)?);
            probes.push(rust_ack_wrong_kind_probe(run)?);
            restore = Some(rust_valid_restore(run)?);
        }
        (DifferentialOperation::Shutdown, 10) => {
            GeneratedVoxelWorldPortAdapter::new(&mut run.world)
                .shutdown(&mut run.sink)
                .map_err(|e| e.error_id().to_string())?;
        }
        _ => {
            ok = false;
            error_id = Some("InvalidHandle".into());
        }
    }

    Ok(rust_observation(
        run, vector, ok, error_id, returned, capture, receipt, ack, restore, probes,
    ))
}

#[allow(clippy::too_many_arguments)]
fn rust_observation(
    run: &mut RustRun,
    vector: &DifferentialVector,
    ok: bool,
    error_id: Option<String>,
    returned: Vec<(String, String)>,
    capture: Option<CaptureObservation>,
    receipt: Option<ReceiptObservation>,
    ack: Option<AckObservation>,
    restore: Option<RestoreObservation>,
    probes: Vec<ProbeObservation>,
) -> DifferentialObservation {
    let state = rust_state(run);
    make_observation(
        vector.sequence,
        rust_operation_label(vector.sequence),
        rust_route(vector.sequence),
        ok,
        error_id,
        returned,
        state,
        capture,
        receipt,
        ack,
        restore,
        probes,
    )
}

#[allow(clippy::too_many_arguments)]
fn make_observation(
    sequence: u64,
    operation: &str,
    route: &str,
    ok: bool,
    error_id: Option<String>,
    returned: Vec<(String, String)>,
    state: StateEvidence,
    capture: Option<CaptureObservation>,
    receipt: Option<ReceiptObservation>,
    ack: Option<AckObservation>,
    restore: Option<RestoreObservation>,
    probes: Vec<ProbeObservation>,
) -> DifferentialObservation {
    DifferentialObservation {
        sequence,
        operation: operation.into(),
        route: route.into(),
        ok,
        error_id,
        lifecycle: state.lifecycle.clone(),
        lifecycle_machine: state.lifecycle_machine.clone(),
        world_id: state.world_id.clone(),
        context_id: state.context_id.clone(),
        generation: state.generation,
        stamp_generation: state.stamp_generation,
        world_revision: state.world_revision,
        section_presence: returned,
        section_revision_set: state.section_revision_set.clone(),
        sections: state.sections.clone(),
        published_root: state.published_root,
        contract_root: state.contract_root,
        directory_digest: state.directory_digest,
        dirty_frontier: state.dirty_frontier.clone(),
        dirty_digest: state.dirty_digest,
        config_hash: state.config_hash.clone(),
        baseline_id: state.baseline_id.clone(),
        schema_epoch: state.schema_epoch,
        gate_source_hashes: state.gate_source_hashes.clone(),
        publication_epoch: state.publication_epoch,
        state_semantic_digest: state.semantic_digest,
        capture,
        receipt,
        ack,
        restore,
        probes,
    }
}

fn rust_state(run: &mut RustRun) -> StateEvidence {
    let view = run.world.publication_authority().capture();
    let stamp = view.stamp();
    let root = view.root().identity();
    if root != run.last_root {
        run.publication_epoch += 1;
        run.last_root = root;
    }
    let state_view = run.world.state_view();
    let sections = run
        .known_sections
        .iter()
        .map(|id| {
            let presence = match view.directory().lookup(id) {
                Ok(Some(slot)) => Some(slot.presence().into()),
                Ok(None) | Err(_) => None,
            };
            let payload_digest = presence
                .as_deref()
                .filter(|value| *value == "Ready")
                .and_then(|_value| {
                    view.directory()
                        .lookup(id)
                        .ok()
                        .flatten()
                        .and_then(|slot| slot.payload())
                        .and_then(payload_declared_digest)
                });
            SectionStateEvidence {
                section_id: id.clone(),
                presence,
                section_revision: stamp.section_revision_set.get(id).copied(),
                payload_digest,
            }
        })
        .collect::<Vec<_>>();
    let dirty = run
        .known_sections
        .iter()
        .map(|id| DirtyStateEvidence {
            section_id: id.clone(),
            first_revision: view.dirty_frontier().first_revision(id).ok().flatten(),
            latest_revision: view.dirty_frontier().latest_revision(id).ok().flatten(),
            reason: view
                .dirty_frontier()
                .reason(id)
                .ok()
                .flatten()
                .map(str::to_string),
        })
        .collect::<Vec<_>>();
    let revisions = stamp
        .section_revision_set
        .iter()
        .map(|(id, rev)| (id.clone(), *rev))
        .collect::<Vec<_>>();
    let directory_digest = rust_directory_digest(&sections);
    let dirty_digest = rust_dirty_digest(&dirty);
    let semantic = rust_state_digest(
        stamp,
        state_view.lifecycle(),
        state_view.lifecycle_machine(),
        state_view.instance_generation(),
        &revisions,
        &sections,
        &dirty,
        &run.config_hash,
        run.publication_epoch,
    );
    StateEvidence {
        world_id: stamp.world_id.clone(),
        context_id: stamp.context_id.clone(),
        generation: state_view.instance_generation(),
        stamp_generation: stamp.generation,
        lifecycle_machine: state_view.lifecycle_machine().into(),
        lifecycle: state_view.lifecycle().into(),
        world_revision: stamp.world_revision,
        section_revision_set: revisions,
        sections,
        published_root: root,
        contract_root: semantic,
        directory_digest,
        dirty_frontier: dirty,
        dirty_digest,
        config_hash: run.config_hash.clone(),
        baseline_id: BASELINE_ID.into(),
        schema_epoch: SCHEMA_EPOCH,
        gate_source_hashes: provenance_pairs(),
        publication_epoch: run.publication_epoch,
        semantic_digest: semantic,
    }
}

fn rust_lifecycle(
    run: &mut RustRun,
    event: &'static str,
    to: &'static str,
    ok: &mut bool,
    error: &mut Option<String>,
) -> Result<(), String> {
    let command = WorldCommand::Lifecycle {
        event,
        to,
        origin: rust_origin(&run.world, event, None, None)?,
    };
    if let Err(e) = GeneratedVoxelWorldPortAdapter::new(&mut run.world).admit(command) {
        *ok = false;
        *error = Some(e.error_id().into());
    }
    Ok(())
}

fn rust_lifecycle_probe(run: &mut RustRun) -> Result<ProbeObservation, String> {
    let command = WorldCommand::Lifecycle {
        event: "Start",
        to: "Ready",
        origin: rust_origin(&run.world, "illegal-transition", None, None)?,
    };
    let result = GeneratedVoxelWorldPortAdapter::new(&mut run.world).admit(command);
    let (ok, error) = match result {
        Ok(_) => (true, None),
        Err(error) => (false, Some(error.error_id().into())),
    };
    Ok(ProbeObservation {
        label: "illegal-transition".into(),
        ok,
        error_id: error,
        returned_sections: Vec::new(),
        state: rust_state(run),
        capture: None,
        receipt: None,
        ack: None,
        restore: None,
    })
}

#[allow(clippy::too_many_arguments)]
fn rust_query(
    run: &mut RustRun,
    query_id: &str,
    ids: &[String],
    cancel: bool,
    world: &str,
    context: &str,
    generation: u64,
    config: &str,
) -> Result<Result<Vec<(String, String)>, String>, String> {
    let request = GeneratedVoxelQueryRequest {
        query_id: query_id.into(),
        world_id: world.into(),
        context: context.into(),
        section_ids: ids.to_vec(),
        cancel,
    };
    let env = OriginEnvelope {
        origin: rust_origin(&run.world, query_id, Some(context), Some(generation))?,
        config_hash: config.into(),
        payload: request,
    };
    match GeneratedVoxelWorldPortAdapter::new(&mut run.world).query(env) {
        Ok(out) => Ok(Ok(out
            .payload
            .items()
            .iter()
            .map(|i| (i.section_id().into(), i.presence().into()))
            .collect())),
        Err(e) => Ok(Err(e.error_id().into())),
    }
}

#[allow(clippy::too_many_arguments)]
fn rust_query_probe(
    run: &mut RustRun,
    label: &str,
    q: &str,
    ids: &[String],
    cancel: bool,
    world: &str,
    context: &str,
    generation: u64,
    config: &str,
) -> Result<ProbeObservation, String> {
    let result = rust_query(run, q, ids, cancel, world, context, generation, config)?;
    let (ok, error, returned) = match result {
        Ok(v) => (true, None, v),
        Err(e) => (false, Some(e), Vec::new()),
    };
    Ok(ProbeObservation {
        label: label.into(),
        ok,
        error_id: error,
        returned_sections: returned,
        state: rust_state(run),
        capture: None,
        receipt: None,
        ack: None,
        restore: None,
    })
}

#[allow(clippy::too_many_arguments)]
fn rust_mutation_request(
    run: &RustRun,
    txn: &str,
    expected: u64,
    section: &str,
    cell: &str,
    value: &str,
    world: &str,
    generation: u64,
    config: &str,
) -> Result<OriginEnvelope<MutationRequest>, String> {
    let context = run.world.state_view().world_context_id().to_string();
    let mut fields = BTreeMap::new();
    fields.insert("world_revision".into(), expected.to_string());
    fields.insert(format!("{section}/{cell}"), value.into());
    Ok(OriginEnvelope {
        origin: rust_origin(&run.world, txn, Some(&context), Some(generation))?,
        config_hash: config.into(),
        payload: MutationRequest {
            txn_id: txn.into(),
            world_id: world.into(),
            generation,
            fields,
        },
    })
}

#[allow(clippy::too_many_arguments)]
fn rust_prepare_probe(
    run: &mut RustRun,
    label: &str,
    txn: &str,
    expected: u64,
    section: &str,
    cell: &str,
    value: &str,
    world: &str,
    generation: u64,
    config: &str,
) -> Result<ProbeObservation, String> {
    let env = rust_mutation_request(
        run, txn, expected, section, cell, value, world, generation, config,
    )?;
    let result = GeneratedVoxelWorldPortAdapter::new(&mut run.world).prepare_mutation(env);
    let (ok, error) = match result {
        Ok(_) => (true, None),
        Err(e) => (false, Some(e.error_id().into())),
    };
    Ok(ProbeObservation {
        label: label.into(),
        ok,
        error_id: error,
        returned_sections: Vec::new(),
        state: rust_state(run),
        capture: None,
        receipt: None,
        ack: None,
        restore: None,
    })
}

fn rust_abort_probe(
    run: &mut RustRun,
    section: &str,
    cell: &str,
    value: &str,
    config: &str,
) -> Result<ProbeObservation, String> {
    let expected = run
        .world
        .publication_authority()
        .capture()
        .stamp()
        .world_revision;
    let generation = run.world.state_view().instance_generation();
    let env = rust_mutation_request(
        run,
        "txn-abort",
        expected,
        section,
        cell,
        value,
        WORLD_ID,
        generation,
        config,
    )?;
    let result =
        match GeneratedVoxelWorldPortAdapter::new(&mut run.world).prepare_mutation(env.clone()) {
            Ok(_) => GeneratedVoxelWorldPortAdapter::new(&mut run.world).abort(env),
            Err(e) => Err(e),
        };
    let (ok, error) = match result {
        Ok(_) => (true, None),
        Err(e) => (false, Some(e.error_id().into())),
    };
    Ok(ProbeObservation {
        label: "abort".into(),
        ok,
        error_id: error,
        returned_sections: Vec::new(),
        state: rust_state(run),
        capture: None,
        receipt: None,
        ack: None,
        restore: None,
    })
}

fn rust_commit(
    run: &mut RustRun,
    prepared: OriginEnvelope<PreparedMutation>,
    hint: &str,
) -> Result<Result<ReceiptObservation, String>, String> {
    let txn = prepared.payload.txn_id().to_string();
    let request = run
        .last_request
        .as_ref()
        .map(|e| e.payload.clone())
        .ok_or_else(|| "request missing".to_string())?;
    let result = GeneratedVoxelWorldPortAdapter::new(&mut run.world).commit(prepared);
    match result {
        Ok(out) => {
            let fields = decode_canonical_object(&out.payload.receipt)
                .map_err(|e| e.error_id().to_string())?;
            if fields.len() != 4
                || ["fingerprint", "new_root", "old_root", "txn_id"]
                    .iter()
                    .any(|key| !fields.contains_key(key))
            {
                return Err("receipt member set mismatch".into());
            }
            let fp_text = fields
                .get("fingerprint")
                .and_then(|v| v.as_text())
                .ok_or_else(|| "receipt fingerprint missing".to_string())?;
            let fp =
                parse_hex32(fp_text).ok_or_else(|| "receipt fingerprint malformed".to_string())?;
            let receipt_txn = fields
                .get("txn_id")
                .and_then(|v| v.as_text())
                .ok_or_else(|| "receipt txn_id missing".to_string())?;
            let old_root = fields
                .get("old_root")
                .and_then(|v| v.as_text())
                .and_then(parse_hex32)
                .ok_or_else(|| "receipt old_root malformed".to_string())?;
            let new_root = fields
                .get("new_root")
                .and_then(|v| v.as_text())
                .and_then(parse_hex32)
                .ok_or_else(|| "receipt new_root malformed".to_string())?;
            let expected = canonical_fingerprint(&request)
                .map_err(|_| "fingerprint failed".to_string())?
                .hash()
                .0;
            let already_recorded = run.receipts.contains_key(&txn);
            let evidence_matches = if already_recorded {
                // A duplicate returns the retained wire receipt while its
                // evidence describes the current prepared base. Verify both
                // independently so a fresh receipt cannot masquerade as a replay.
                out.payload.evidence.old_root
                    == run
                        .world
                        .publication_authority()
                        .capture()
                        .root()
                        .identity()
                    && out.payload.evidence.new_root
                        == run
                            .world
                            .publication_authority()
                            .capture()
                            .root()
                            .identity()
                    && run
                        .receipts
                        .get(&txn)
                        .is_some_and(|stored| stored == &out.payload.receipt)
            } else {
                old_root == out.payload.evidence.old_root
                    && new_root == out.payload.evidence.new_root
            };
            if receipt_txn != txn
                || !evidence_matches
                || fp != expected
                || sha256(&out.payload.receipt) != out.payload.evidence.receipt_hash
            {
                return Err("receipt evidence mismatch".into());
            }
            let disposition = if run
                .receipts
                .get(&txn)
                .is_some_and(|b| b == &out.payload.receipt)
            {
                "Duplicate"
            } else {
                hint
            };
            if disposition == "Original" {
                run.receipts
                    .insert(txn.clone(), out.payload.receipt.clone());
            }
            Ok(Ok(ReceiptObservation {
                txn_id: txn.clone(),
                disposition: disposition.into(),
                fingerprint: fp,
                receipt_hash: out.payload.evidence.receipt_hash,
                old_root,
                new_root,
                receipt_len: out.payload.receipt.len(),
                wire_verified: true,
                semantic_digest: receipt_semantic_digest(&txn, disposition, fp),
            }))
        }
        Err(e) => Ok(Err(e.error_id().into())),
    }
}

fn rust_commit_new(
    run: &mut RustRun,
    txn: &str,
    section: &str,
    value: &str,
    expected: u64,
) -> Result<ReceiptObservation, String> {
    let generation = run.world.state_view().instance_generation();
    let env = rust_mutation_request(
        run,
        txn,
        expected,
        section,
        "cell",
        value,
        WORLD_ID,
        generation,
        &run.config_hash,
    )?;
    run.last_request = Some(env.clone());
    let prepared = GeneratedVoxelWorldPortAdapter::new(&mut run.world)
        .prepare_mutation(env)
        .map_err(|e| e.error_id().to_string())?;
    rust_commit(run, prepared, "Original")?
}

fn rust_conflicting_replay_probe(
    run: &mut RustRun,
    request: &OriginEnvelope<MutationRequest>,
) -> Result<ProbeObservation, String> {
    let mut conflict = request.clone();
    conflict
        .payload
        .fields
        .insert("s:0:0:0/cell-0".into(), "conflicting".into());
    let result = GeneratedVoxelWorldPortAdapter::new(&mut run.world).prepare_mutation(conflict);
    let (ok, error) = match result {
        Ok(_) => (true, None),
        Err(e) => (false, Some(e.error_id().into())),
    };
    Ok(ProbeObservation {
        label: "conflicting-replay".into(),
        ok,
        error_id: error,
        returned_sections: Vec::new(),
        state: rust_state(run),
        capture: None,
        receipt: None,
        ack: None,
        restore: None,
    })
}

fn rust_capture(run: &mut RustRun, cut_id: &str) -> Result<CaptureObservation, String> {
    let cut = RuntimeSnapshotCut::from_live(&run.world, cut_id);
    let (capture, evidence) = GeneratedVoxelWorldPortAdapter::new(&mut run.world)
        .capture(&cut)
        .map_err(|e| e.error_id().to_string())?;
    if evidence.root_hash != capture.root_identity()
        || evidence.voxel_stamp != *capture.stamp()
        || evidence.voxel_stamp != *capture.pin_stamp()
        || !evidence.barrier_released
    {
        return Err("capture evidence mismatch".into());
    }
    let stamp = evidence.voxel_stamp.clone();
    let out = CaptureObservation {
        cut_id: evidence.cut_id,
        world_id: stamp.world_id.clone(),
        context_id: stamp.context_id.clone(),
        generation: stamp.generation,
        world_revision: stamp.world_revision,
        section_revision_set: stamp
            .section_revision_set
            .iter()
            .map(|(id, rev)| (id.clone(), *rev))
            .collect(),
        config_hash: run.config_hash.clone(),
        artifact_hash: evidence.root_hash,
        semantic_digest: capture_semantic_digest(
            &stamp.world_id,
            &stamp.context_id,
            stamp.generation,
            stamp.world_revision,
            &run.config_hash,
            &stamp
                .section_revision_set
                .iter()
                .map(|(id, rev)| (id.clone(), *rev))
                .collect::<Vec<_>>(),
        ),
    };
    run.last_capture = Some(capture);
    Ok(out)
}

fn rust_capture_mismatch_probe(run: &mut RustRun) -> Result<ProbeObservation, String> {
    let mut cut = RuntimeSnapshotCut::from_live(&run.world, "cut-root-mismatch");
    cut.artifact_hash[0] ^= 1;
    let result = GeneratedVoxelWorldPortAdapter::new(&mut run.world).capture(&cut);
    let (ok, error) = match result {
        Ok(_) => (true, None),
        Err(error) => (false, Some(error.error_id().into())),
    };
    if ok || error.as_deref() != Some("InvalidHandle") {
        return Err(format!(
            "capture-root-mismatch expected InvalidHandle, got {error:?}"
        ));
    }
    Ok(ProbeObservation {
        label: "capture-root-mismatch".into(),
        ok,
        error_id: error,
        returned_sections: Vec::new(),
        state: rust_state(run),
        capture: None,
        receipt: None,
        ack: None,
        restore: None,
    })
}

fn rust_restore_preflight_probe(
    run: &mut RustRun,
    label: &str,
) -> Result<ProbeObservation, String> {
    let capture = run
        .last_capture
        .clone()
        .ok_or_else(|| "capture missing".to_string())?;
    let mut writer = MemoryCaptureWriter::new(8192);
    encode_capture(&capture, &mut writer).map_err(|e| e.error_id().to_string())?;
    let bytes = writer.as_slice().to_vec();
    let generation = run.world.state_view().instance_generation();
    let snapshot = differential_snapshot(CONFIG_LABEL)?;
    let result = match label {
        "truncated" => RestorePreflight::validate(
            &bytes[..bytes.len() / 2],
            WORLD_ID,
            generation,
            snapshot.as_ref(),
        )
        .map(|_| ())
        .map_err(|e| e.error_id().to_string()),
        "wrong-world" => {
            RestorePreflight::validate(&bytes, "world-other", generation, snapshot.as_ref())
                .map(|_| ())
                .map_err(|e| e.error_id().to_string())
        }
        "wrong-config" => RestorePreflight::validate(
            &bytes,
            WORLD_ID,
            generation,
            differential_snapshot("wrong-config")?.as_ref(),
        )
        .map(|_| ())
        .map_err(|e| e.error_id().to_string()),
        "stale-generation" => {
            RestorePreflight::validate(&bytes, WORLD_ID, generation + 1, snapshot.as_ref())
                .map(|_| ())
                .map_err(|e| e.error_id().to_string())
        }
        _ => Err("InvalidHandle".into()),
    };
    let (ok, error) = match result {
        Ok(()) => (true, None),
        Err(e) => (false, Some(e)),
    };
    Ok(ProbeObservation {
        label: label.into(),
        ok,
        error_id: error,
        returned_sections: Vec::new(),
        state: rust_state(run),
        capture: None,
        receipt: None,
        ack: None,
        restore: None,
    })
}

fn rust_valid_restore(run: &mut RustRun) -> Result<RestoreObservation, String> {
    let capture = rust_capture_ref(run, "cut-restore-valid")?;
    let mut writer = MemoryCaptureWriter::new(8192);
    encode_capture(&capture, &mut writer).map_err(|e| e.error_id().to_string())?;
    let snapshot = differential_snapshot(CONFIG_LABEL)?;
    let generation = run.world.state_view().instance_generation();
    let decoded =
        RestorePreflight::validate(writer.as_slice(), WORLD_ID, generation, snapshot.as_ref())
            .map_err(|e| e.error_id().to_string())?;
    let candidate = RestoreShadowBuilder::build(&decoded).map_err(|e| e.error_id().to_string())?;
    if !candidate.hash_matches() {
        return Err("restore candidate hash mismatch".into());
    }
    let old_root = run
        .world
        .publication_authority()
        .capture()
        .root()
        .identity();
    let receipt = GeneratedVoxelWorldPortAdapter::new(&mut run.world)
        .restore(candidate)
        .map_err(|e| e.error_id().to_string())?;
    if receipt.old_root() != old_root || receipt.new_root() == old_root {
        return Err("restore receipt mismatch".into());
    }
    let view = run.world.publication_authority().capture();
    Ok(RestoreObservation {
        cut_id: RESTORE_CUT_ID.into(),
        old_root: receipt.old_root(),
        new_root: receipt.new_root(),
        world_id: view.stamp().world_id.clone(),
        context_id: view.stamp().context_id.clone(),
        generation: run.world.state_view().instance_generation(),
        world_revision: view.stamp().world_revision,
        section_revision_set: view
            .stamp()
            .section_revision_set
            .iter()
            .map(|(id, rev)| (id.clone(), *rev))
            .collect(),
        config_hash: run.config_hash.clone(),
        semantic_digest: restore_semantic_digest(
            &view.stamp().world_id,
            &view.stamp().context_id,
            view.stamp().generation,
            view.stamp().world_revision,
        ),
    })
}

fn rust_capture_ref(run: &mut RustRun, cut_id: &str) -> Result<VoxelCaptureRef, String> {
    let cut = RuntimeSnapshotCut::from_live(&run.world, cut_id);
    let (capture, _) = GeneratedVoxelWorldPortAdapter::new(&mut run.world)
        .capture(&cut)
        .map_err(|e| e.error_id().to_string())?;
    run.last_capture = Some(capture.clone());
    Ok(capture)
}

fn dirty_latest(run: &RustRun) -> Vec<(&'static str, u64)> {
    KNOWN_SECTIONS
        .iter()
        .filter_map(|id| {
            run.world
                .publication_authority()
                .capture()
                .dirty_frontier()
                .latest_revision(id)
                .ok()
                .flatten()
                .map(|rev| (*id, rev))
        })
        .collect()
}

fn rust_ack_probe(
    run: &mut RustRun,
    label: &str,
    covered: u64,
    sections: Vec<(&'static str, u64)>,
    world: &str,
    context: &str,
    last: &mut Option<AckObservation>,
) -> Result<ProbeObservation, String> {
    let generation = run.world.state_view().instance_generation();
    let pairs = sections
        .iter()
        .map(|(id, rev)| ((*id).into(), *rev))
        .collect::<Vec<_>>();
    let evidence = AckEvidence {
        kind: "DurabilityAck".into(),
        world_id: world.into(),
        context: DurabilityAckContext {
            context_id: context.into(),
            generation,
        },
        covered_world_revision: covered,
        covered_sections: sections
            .iter()
            .map(|(id, rev)| CoveredSectionAck {
                section_id: (*id).into(),
                up_to_section_revision: *rev,
            })
            .collect(),
    };
    let old_root = run
        .world
        .publication_authority()
        .capture()
        .root()
        .identity();
    let result = GeneratedVoxelWorldPortAdapter::new(&mut run.world).apply_durability_ack(evidence);
    match result {
        Ok(receipt) => {
            let out = AckObservation {
                kind: "DurabilityAck".into(),
                world_id: world.into(),
                context_id: context.into(),
                generation,
                covered_world_revision: covered,
                covered_sections: pairs.clone(),
                coverage_len: receipt.coverage_len(),
                old_root,
                new_root: receipt.new_root(),
                semantic_digest: ack_semantic_digest(
                    world,
                    context,
                    generation,
                    covered,
                    &pairs,
                    receipt.coverage_len(),
                ),
            };
            *last = Some(out.clone());
            Ok(ProbeObservation {
                label: label.into(),
                ok: true,
                error_id: None,
                returned_sections: Vec::new(),
                state: rust_state(run),
                capture: None,
                receipt: None,
                ack: Some(out),
                restore: None,
            })
        }
        Err(e) => Ok(ProbeObservation {
            label: label.into(),
            ok: false,
            error_id: Some(e.error_id().into()),
            returned_sections: Vec::new(),
            state: rust_state(run),
            capture: None,
            receipt: None,
            ack: None,
            restore: None,
        }),
    }
}

fn rust_ack_error_probe(
    run: &mut RustRun,
    label: &str,
    world: &str,
    context: &str,
    generation: u64,
    expected: &str,
) -> Result<ProbeObservation, String> {
    let evidence = AckEvidence {
        kind: "DurabilityAck".into(),
        world_id: world.into(),
        context: DurabilityAckContext {
            context_id: context.into(),
            generation,
        },
        covered_world_revision: 0,
        covered_sections: Vec::new(),
    };
    let result = GeneratedVoxelWorldPortAdapter::new(&mut run.world).apply_durability_ack(evidence);
    let (ok, error) = match result {
        Ok(_) => (true, None),
        Err(e) => (false, Some(e.error_id().into())),
    };
    if ok || error.as_deref() != Some(expected) {
        return Err(format!("{label} expected {expected}, got {error:?}"));
    }
    Ok(ProbeObservation {
        label: label.into(),
        ok,
        error_id: error,
        returned_sections: Vec::new(),
        state: rust_state(run),
        capture: None,
        receipt: None,
        ack: None,
        restore: None,
    })
}

fn rust_ack_future_probe(run: &mut RustRun) -> Result<ProbeObservation, String> {
    let generation = run.world.state_view().instance_generation();
    let covered = run
        .world
        .publication_authority()
        .capture()
        .stamp()
        .world_revision
        + 1;
    let evidence = AckEvidence {
        kind: "DurabilityAck".into(),
        world_id: WORLD_ID.into(),
        context: DurabilityAckContext {
            context_id: CONTEXT_ID.into(),
            generation,
        },
        covered_world_revision: covered,
        covered_sections: Vec::new(),
    };
    let result = GeneratedVoxelWorldPortAdapter::new(&mut run.world).apply_durability_ack(evidence);
    let (ok, error) = match result {
        Ok(_) => (true, None),
        Err(e) => (false, Some(e.error_id().into())),
    };
    if ok || error.as_deref() != Some("EvidenceDigestMismatch") {
        return Err(format!(
            "future-cut expected EvidenceDigestMismatch, got {error:?}"
        ));
    }
    Ok(ProbeObservation {
        label: "future-cut".into(),
        ok,
        error_id: error,
        returned_sections: Vec::new(),
        state: rust_state(run),
        capture: None,
        receipt: None,
        ack: None,
        restore: None,
    })
}

fn rust_ack_duplicate_probe(run: &mut RustRun) -> Result<ProbeObservation, String> {
    let generation = run.world.state_view().instance_generation();
    let evidence = AckEvidence {
        kind: "DurabilityAck".into(),
        world_id: WORLD_ID.into(),
        context: DurabilityAckContext {
            context_id: CONTEXT_ID.into(),
            generation,
        },
        covered_world_revision: run
            .world
            .publication_authority()
            .capture()
            .stamp()
            .world_revision,
        covered_sections: vec![
            CoveredSectionAck {
                section_id: "s:0:0:0".into(),
                up_to_section_revision: 0,
            },
            CoveredSectionAck {
                section_id: "s:0:0:0".into(),
                up_to_section_revision: 0,
            },
        ],
    };
    let result = GeneratedVoxelWorldPortAdapter::new(&mut run.world).apply_durability_ack(evidence);
    let (ok, error) = match result {
        Ok(_) => (true, None),
        Err(e) => (false, Some(e.error_id().into())),
    };
    if ok || error.as_deref() != Some("InvalidHandle") {
        return Err(format!(
            "duplicate-section expected InvalidHandle, got {error:?}"
        ));
    }
    Ok(ProbeObservation {
        label: "duplicate-section".into(),
        ok,
        error_id: error,
        returned_sections: Vec::new(),
        state: rust_state(run),
        capture: None,
        receipt: None,
        ack: None,
        restore: None,
    })
}

fn rust_ack_malformed_probe(run: &mut RustRun) -> Result<ProbeObservation, String> {
    let generation = run.world.state_view().instance_generation();
    let evidence = AckEvidence {
        kind: "DurabilityAck".into(),
        world_id: WORLD_ID.into(),
        context: DurabilityAckContext {
            context_id: CONTEXT_ID.into(),
            generation,
        },
        covered_world_revision: run
            .world
            .publication_authority()
            .capture()
            .stamp()
            .world_revision,
        covered_sections: vec![CoveredSectionAck {
            section_id: "s:-0:0:0".into(),
            up_to_section_revision: 0,
        }],
    };
    let result = GeneratedVoxelWorldPortAdapter::new(&mut run.world).apply_durability_ack(evidence);
    let (ok, error) = match result {
        Ok(_) => (true, None),
        Err(error) => (false, Some(error.error_id().into())),
    };
    if ok || error.as_deref() != Some(vw::UNKNOWN_SECTION_KEY) {
        return Err(format!(
            "malformed-section expected {}, got {error:?}",
            vw::UNKNOWN_SECTION_KEY
        ));
    }
    Ok(ProbeObservation {
        label: "malformed-section".into(),
        ok,
        error_id: error,
        returned_sections: Vec::new(),
        state: rust_state(run),
        capture: None,
        receipt: None,
        ack: None,
        restore: None,
    })
}

fn rust_ack_wrong_kind_probe(run: &mut RustRun) -> Result<ProbeObservation, String> {
    let generation = run.world.state_view().instance_generation();
    let evidence = AckEvidence {
        kind: "NotDurableAck".into(),
        world_id: WORLD_ID.into(),
        context: DurabilityAckContext {
            context_id: CONTEXT_ID.into(),
            generation,
        },
        covered_world_revision: run
            .world
            .publication_authority()
            .capture()
            .stamp()
            .world_revision,
        covered_sections: Vec::new(),
    };
    let result = GeneratedVoxelWorldPortAdapter::new(&mut run.world).apply_durability_ack(evidence);
    let (ok, error) = match result {
        Ok(_) => (true, None),
        Err(error) => (false, Some(error.error_id().into())),
    };
    if ok || error.as_deref() != Some("InvalidHandle") {
        return Err(format!("wrong-kind expected InvalidHandle, got {error:?}"));
    }
    Ok(ProbeObservation {
        label: "wrong-kind".into(),
        ok,
        error_id: error,
        returned_sections: Vec::new(),
        state: rust_state(run),
        capture: None,
        receipt: None,
        ack: None,
        restore: None,
    })
}

fn oracle_request(
    txn: &str,
    world: &str,
    generation: u64,
    expected: u64,
    section: &str,
    cell: &str,
    value: &str,
) -> MutationRequest {
    let mut fields = BTreeMap::new();
    fields.insert("world_revision".into(), expected.to_string());
    fields.insert(format!("{section}/{cell}"), value.into());
    MutationRequest {
        txn_id: txn.into(),
        world_id: world.into(),
        generation,
        fields,
    }
}

#[allow(clippy::too_many_arguments)]
fn oracle_query(
    state: &OracleState,
    query_id: &str,
    ids: &[String],
    cancel: bool,
    world: &str,
    context: &str,
    generation: u64,
    config: &str,
) -> Result<Vec<(String, String)>, &'static str> {
    if config != state.config_hash {
        return Err("SessionMismatch");
    }
    if world != WORLD_ID || context != CONTEXT_ID {
        return Err("SessionMismatch");
    }
    if generation != state.generation {
        return Err("StaleEpoch");
    }
    if cancel || query_id.is_empty() {
        return Err("InvalidHandle");
    }
    if ids.len() > QUERY_BUDGET {
        return Err("BudgetExceeded");
    }
    let mut parsed = Vec::new();
    for id in ids {
        parsed.push((parse_coordinate(id)?, id.as_str()));
    }
    parsed.sort_by_key(|(coord, _)| *coord);
    parsed.dedup_by_key(|(coord, _)| *coord);
    Ok(parsed
        .into_iter()
        .map(|(coord, _)| {
            let id = canonical_coordinate(coord);
            let presence = if state.ready.contains(&id) {
                "Ready"
            } else {
                "Unchanged"
            };
            (id, presence.into())
        })
        .collect())
}

fn budget_query_ids() -> Vec<String> {
    (0..=QUERY_BUDGET).map(|_| "s:0:0:0".to_string()).collect()
}

/// 参考实现自己解析 Section 键(契约 `identity`),不复用被测实现——差分测试的意义就在
/// 于两侧独立。返回的 Err 是契约错误码。
fn parse_coordinate(raw: &str) -> Result<(i32, i32, i32), &'static str> {
    let mut parts = raw.split(':');
    let prefix = parts.next().unwrap_or_default();
    let rest: Vec<&str> = parts.collect();
    if prefix != "s" || rest.len() != 3 {
        return Err(vw::UNKNOWN_SECTION_KEY);
    }
    let x = parse_coord(rest[0]).ok_or(vw::UNKNOWN_SECTION_KEY)?;
    let y = parse_coord(rest[1]).ok_or(vw::UNKNOWN_SECTION_KEY)?;
    let z = parse_coord(rest[2]).ok_or(vw::UNKNOWN_SECTION_KEY)?;
    if y < i64::from(vw::SECTION_Y_MIN) || y > i64::from(vw::SECTION_Y_MAX) {
        return Err(vw::SECTION_Y_OUT_OF_RANGE);
    }
    let in_i32 =
        |v: i64| v >= i64::from(vw::SECTION_COORD_MIN) && v <= i64::from(vw::SECTION_COORD_MAX);
    if !in_i32(x) || !in_i32(z) {
        return Err(vw::COORDINATE_OUT_OF_BOUNDS);
    }
    Ok((x as i32, y as i32, z as i32))
}

/// 规范十进制:非空、无前导零、不得 `-0`。值域由调用方按坐标轴判定。
fn parse_coord(raw: &str) -> Option<i64> {
    if raw.is_empty() {
        return None;
    }
    let digits = raw.strip_prefix('-').unwrap_or(raw);
    if digits.is_empty()
        || !digits.bytes().all(|b| b.is_ascii_digit())
        || (digits.len() > 1 && digits.starts_with('0'))
        || (raw.starts_with('-') && digits == "0")
    {
        return None;
    }
    raw.parse().ok()
}

fn canonical_coordinate((x, y, z): (i32, i32, i32)) -> String {
    format!("s:{x}:{y}:{z}")
}

fn oracle_fingerprint(request: &MutationRequest) -> Result<[u8; 32], &'static str> {
    if request.fields.keys().any(|key| {
        matches!(
            key.as_str(),
            "canonicalForm" | "txn_id" | "world_id" | "generation"
        )
    }) {
        return Err("InvalidHandle");
    }
    let mut entries = BTreeMap::new();
    for (key, value) in &request.fields {
        entries.insert(key.clone(), oracle_quote(value));
    }
    entries.insert(
        "canonicalForm".into(),
        oracle_quote("VoxelCanonicalObjectV1"),
    );
    entries.insert("txn_id".into(), oracle_quote(&request.txn_id));
    entries.insert("world_id".into(), oracle_quote(&request.world_id));
    entries.insert("generation".into(), request.generation.to_string());
    let mut encoded = String::from("{");
    for (index, (key, value)) in entries.iter().enumerate() {
        if index > 0 {
            encoded.push(',');
        }
        encoded.push_str(&oracle_quote(key));
        encoded.push(':');
        encoded.push_str(value);
    }
    encoded.push('}');
    Ok(sha256(encoded.as_bytes()))
}

fn oracle_quote(value: &str) -> String {
    let mut out = String::from("\"");
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn oracle_root(state: &OracleState, _semantic: [u8; 32]) -> [u8; 32] {
    let mut bytes = Vec::new();
    frame_oracle(&mut bytes, "PublishedStateRootProjectionV1");
    frame_oracle(&mut bytes, WORLD_ID);
    frame_oracle(&mut bytes, CONTEXT_ID);
    frame_oracle(&mut bytes, &state.publication_epoch.to_string());
    for (id, revision) in &state.section_revisions {
        frame_oracle(&mut bytes, id);
        frame_oracle(&mut bytes, &revision.to_string());
    }
    let sections = state
        .section_revisions
        .keys()
        .map(|id| SectionStateEvidence {
            section_id: id.clone(),
            presence: state.ready.contains(id).then(|| "Ready".into()),
            section_revision: state.section_revisions.get(id).copied(),
            payload_digest: state.payloads.get(id).map(|bytes| sha256(bytes)),
        })
        .collect::<Vec<_>>();
    bytes.extend_from_slice(&oracle_directory_digest(&sections));
    let dirty = KNOWN_SECTIONS
        .iter()
        .map(|id| DirtyStateEvidence {
            section_id: (*id).into(),
            first_revision: state.dirty.get(*id).map(|item| item.first),
            latest_revision: state.dirty.get(*id).map(|item| item.latest),
            reason: state.dirty.get(*id).map(|item| item.reason.clone()),
        })
        .collect::<Vec<_>>();
    bytes.extend_from_slice(&oracle_dirty_digest(&dirty));
    sha256(&bytes)
}

/// Extract the declared page digest from the immutable payload debug view.
/// `SectionPayload` intentionally exposes only its schema id; the page digest is
/// still authoritative evidence and is parsed without depending on private
/// page fields or manufacturing a digest from an id/revision tuple.
fn payload_declared_digest(payload: &SectionPayload) -> Option<[u8; 32]> {
    let debug = format!("{payload:?}");
    let start = debug.find("digest: [")? + "digest: [".len();
    let end = debug[start..].find(']')? + start;
    let values = debug[start..end]
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::parse::<u8>)
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    let values: [u8; 32] = values.try_into().ok()?;
    Some(values)
}

fn oracle_directory_digest(sections: &[SectionStateEvidence]) -> [u8; 32] {
    let mut bytes = Vec::new();
    for section in sections {
        frame_oracle(&mut bytes, &section.section_id);
        frame_oracle(
            &mut bytes,
            section.presence.as_deref().unwrap_or("<missing>"),
        );
        frame_oracle(
            &mut bytes,
            &section.section_revision.unwrap_or(u64::MAX).to_string(),
        );
        if let Some(payload) = section.payload_digest {
            bytes.extend_from_slice(&payload);
        } else {
            bytes.extend_from_slice(&[0u8; 32]);
        }
    }
    sha256(&bytes)
}

fn oracle_dirty_digest(dirty: &[DirtyStateEvidence]) -> [u8; 32] {
    let mut bytes = Vec::new();
    for item in dirty {
        frame_oracle(&mut bytes, &item.section_id);
        frame_oracle(
            &mut bytes,
            &item
                .first_revision
                .map_or("<none>".into(), |value| value.to_string()),
        );
        frame_oracle(
            &mut bytes,
            &item
                .latest_revision
                .map_or("<none>".into(), |value| value.to_string()),
        );
        frame_oracle(&mut bytes, item.reason.as_deref().unwrap_or("<none>"));
    }
    sha256(&bytes)
}

#[allow(clippy::too_many_arguments)]
fn oracle_state_digest(
    world_id: &str,
    context_id: &str,
    lifecycle_machine: &str,
    lifecycle: &str,
    world_revision: u64,
    stamp_generation: u64,
    generation: u64,
    revisions: &[(String, u64)],
    sections: &[SectionStateEvidence],
    dirty: &[DirtyStateEvidence],
    config: &str,
    epoch: u64,
) -> [u8; 32] {
    let mut bytes = Vec::new();
    let generation_delta = generation.saturating_sub(stamp_generation);
    for value in [
        world_id.to_string(),
        context_id.to_string(),
        lifecycle_machine.to_string(),
        lifecycle.to_string(),
        world_revision.to_string(),
        "generation-bound".to_string(),
        generation_delta.to_string(),
        config.to_string(),
        epoch.to_string(),
    ] {
        frame_oracle(&mut bytes, &value);
    }
    for (id, revision) in revisions {
        frame_oracle(&mut bytes, id);
        frame_oracle(&mut bytes, &revision.to_string());
    }
    bytes.extend_from_slice(&oracle_directory_digest(sections));
    bytes.extend_from_slice(&oracle_dirty_digest(dirty));
    sha256(&bytes)
}

fn frame_oracle(out: &mut Vec<u8>, value: &str) {
    out.extend_from_slice(&(value.len() as u64).to_le_bytes());
    out.extend_from_slice(value.as_bytes());
}

fn rust_directory_digest(sections: &[SectionStateEvidence]) -> [u8; 32] {
    let mut bytes = Vec::new();
    for section in sections {
        append_rust(&mut bytes, section.section_id.as_bytes());
        append_rust(
            &mut bytes,
            section
                .presence
                .as_deref()
                .unwrap_or("<missing>")
                .as_bytes(),
        );
        append_rust(
            &mut bytes,
            section
                .section_revision
                .unwrap_or(u64::MAX)
                .to_string()
                .as_bytes(),
        );
        if let Some(payload) = section.payload_digest {
            bytes.extend_from_slice(&payload);
        } else {
            bytes.extend_from_slice(&[0u8; 32]);
        }
    }
    sha256(&bytes)
}

fn rust_dirty_digest(dirty: &[DirtyStateEvidence]) -> [u8; 32] {
    let mut bytes = Vec::new();
    for item in dirty {
        append_rust(&mut bytes, item.section_id.as_bytes());
        append_rust(
            &mut bytes,
            item.first_revision
                .map_or("<none>".into(), |value| value.to_string())
                .as_bytes(),
        );
        append_rust(
            &mut bytes,
            item.latest_revision
                .map_or("<none>".into(), |value| value.to_string())
                .as_bytes(),
        );
        append_rust(
            &mut bytes,
            item.reason.as_deref().unwrap_or("<none>").as_bytes(),
        );
    }
    sha256(&bytes)
}

#[allow(clippy::too_many_arguments)]
fn rust_state_digest(
    stamp: &GeneratedRevisionStamp,
    lifecycle: &str,
    lifecycle_machine: &str,
    generation: u64,
    revisions: &[(String, u64)],
    sections: &[SectionStateEvidence],
    dirty: &[DirtyStateEvidence],
    config: &str,
    epoch: u64,
) -> [u8; 32] {
    let mut bytes = Vec::new();
    let generation_delta = generation.saturating_sub(stamp.generation);
    for value in [
        stamp.world_id.as_bytes(),
        stamp.context_id.as_bytes(),
        lifecycle_machine.as_bytes(),
        lifecycle.as_bytes(),
        stamp.world_revision.to_string().as_bytes(),
        b"generation-bound",
        generation_delta.to_string().as_bytes(),
        config.as_bytes(),
        epoch.to_string().as_bytes(),
    ] {
        append_rust(&mut bytes, value);
    }
    for (id, revision) in revisions {
        append_rust(&mut bytes, id.as_bytes());
        append_rust(&mut bytes, revision.to_string().as_bytes());
    }
    bytes.extend_from_slice(&rust_directory_digest(sections));
    bytes.extend_from_slice(&rust_dirty_digest(dirty));
    sha256(&bytes)
}

fn append_rust(out: &mut Vec<u8>, value: &[u8]) {
    let len = value.len() as u64;
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(value);
}

fn oracle_receipt(
    txn: &str,
    disposition: &str,
    fingerprint: [u8; 32],
    old: [u8; 32],
    new: [u8; 32],
) -> ReceiptObservation {
    let receipt_bytes = oracle_receipt_bytes(txn, old, new, fingerprint);
    let receipt_hash = sha256(&receipt_bytes);
    ReceiptObservation {
        txn_id: txn.into(),
        disposition: disposition.into(),
        fingerprint,
        // The reference independently encodes the four published receipt
        // members; no production receipt bytes or disposition flag are copied.
        receipt_hash,
        old_root: old,
        new_root: new,
        receipt_len: receipt_bytes.len(),
        wire_verified: true,
        semantic_digest: receipt_semantic_digest(txn, disposition, fingerprint),
    }
}

fn oracle_receipt_bytes(txn: &str, old: [u8; 32], new: [u8; 32], fingerprint: [u8; 32]) -> Vec<u8> {
    let members = [
        ("fingerprint", oracle_quote(&hex32(&fingerprint))),
        ("new_root", oracle_quote(&hex32(&new))),
        ("old_root", oracle_quote(&hex32(&old))),
        ("txn_id", oracle_quote(txn)),
    ];
    let mut encoded = String::from("{");
    for (index, (key, value)) in members.iter().enumerate() {
        if index > 0 {
            encoded.push(',');
        }
        encoded.push_str(&oracle_quote(key));
        encoded.push(':');
        encoded.push_str(value);
    }
    encoded.push('}');
    encoded.into_bytes()
}

fn receipt_semantic_digest(txn: &str, disposition: &str, fingerprint: [u8; 32]) -> [u8; 32] {
    let mut bytes = Vec::new();
    frame_oracle(&mut bytes, txn);
    frame_oracle(&mut bytes, disposition);
    bytes.extend_from_slice(&fingerprint);
    sha256(&bytes)
}

fn ack_semantic_digest(
    world: &str,
    context: &str,
    generation: u64,
    covered: u64,
    sections: &[(String, u64)],
    count: usize,
) -> [u8; 32] {
    let mut bytes = Vec::new();
    frame_oracle(&mut bytes, world);
    frame_oracle(&mut bytes, context);
    frame_oracle(&mut bytes, &generation.to_string());
    frame_oracle(&mut bytes, &covered.to_string());
    for (id, revision) in sections {
        frame_oracle(&mut bytes, id);
        frame_oracle(&mut bytes, &revision.to_string());
    }
    frame_oracle(&mut bytes, &count.to_string());
    sha256(&bytes)
}

fn capture_from_state(state: &StateEvidence, cut: &str) -> CaptureObservation {
    CaptureObservation {
        cut_id: cut.into(),
        world_id: state.world_id.clone(),
        context_id: state.context_id.clone(),
        generation: state.stamp_generation,
        world_revision: state.world_revision,
        section_revision_set: state.section_revision_set.clone(),
        config_hash: state.config_hash.clone(),
        artifact_hash: state.published_root,
        semantic_digest: capture_semantic_digest(
            &state.world_id,
            &state.context_id,
            state.stamp_generation,
            state.world_revision,
            &state.config_hash,
            &state.section_revision_set,
        ),
    }
}

fn capture_semantic_digest(
    world: &str,
    context: &str,
    generation: u64,
    revision: u64,
    config: &str,
    sections: &[(String, u64)],
) -> [u8; 32] {
    let mut bytes = Vec::new();
    frame_oracle(&mut bytes, world);
    frame_oracle(&mut bytes, context);
    frame_oracle(&mut bytes, &generation.to_string());
    frame_oracle(&mut bytes, &revision.to_string());
    frame_oracle(&mut bytes, config);
    for (id, value) in sections {
        frame_oracle(&mut bytes, id);
        frame_oracle(&mut bytes, &value.to_string());
    }
    sha256(&bytes)
}

fn restore_semantic_digest(world: &str, context: &str, generation: u64, revision: u64) -> [u8; 32] {
    sha256(format!("{world}:{context}:{generation}:{revision}").as_bytes())
}

fn hex32(bytes: &[u8; 32]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(64);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 15) as usize] as char);
    }
    out
}

fn parse_hex32(value: &str) -> Option<[u8; 32]> {
    if value.len() != 64 || !value.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let mut out = [0u8; 32];
    for (index, slot) in out.iter_mut().enumerate() {
        *slot = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16).ok()?;
    }
    Some(out)
}

fn oracle_trace_digest(items: &[DifferentialObservation]) -> [u8; 32] {
    let mut bytes = Vec::new();
    for item in items {
        frame_oracle(&mut bytes, &item.sequence.to_string());
        frame_oracle(&mut bytes, &item.operation);
        frame_oracle(&mut bytes, if item.ok { "ok" } else { "error" });
        frame_oracle(&mut bytes, item.error_id.as_deref().unwrap_or("<none>"));
        frame_oracle(&mut bytes, &hex32(&item.state_semantic_digest));
        frame_oracle(&mut bytes, &item.publication_epoch.to_string());
        frame_oracle(
            &mut bytes,
            if item.capture.is_some() {
                "capture"
            } else {
                "<none>"
            },
        );
        frame_oracle(
            &mut bytes,
            if item.receipt.is_some() {
                "receipt"
            } else {
                "<none>"
            },
        );
        frame_oracle(
            &mut bytes,
            if item.ack.is_some() { "ack" } else { "<none>" },
        );
        frame_oracle(
            &mut bytes,
            if item.restore.is_some() {
                "restore"
            } else {
                "<none>"
            },
        );
        if let Some(receipt) = &item.receipt {
            frame_oracle(&mut bytes, &receipt.disposition);
            frame_oracle(&mut bytes, &receipt.receipt_len.to_string());
            frame_oracle(
                &mut bytes,
                if receipt.fingerprint != [0; 32] {
                    "fingerprint-present"
                } else {
                    "fingerprint-missing"
                },
            );
            frame_oracle(
                &mut bytes,
                if receipt.semantic_digest != [0; 32] {
                    "semantic-present"
                } else {
                    "semantic-missing"
                },
            );
            frame_oracle(
                &mut bytes,
                if receipt.old_root == receipt.new_root {
                    "same-root"
                } else {
                    "advanced-root"
                },
            );
            frame_oracle(
                &mut bytes,
                if receipt.new_root == item.published_root {
                    "current-root"
                } else {
                    "non-current-root"
                },
            );
            frame_oracle(
                &mut bytes,
                if receipt.wire_verified {
                    "wire-ok"
                } else {
                    "wire-bad"
                },
            );
        }
        if let Some(ack) = &item.ack {
            frame_oracle(&mut bytes, &ack.covered_world_revision.to_string());
            frame_oracle(&mut bytes, &ack.coverage_len.to_string());
            for (id, revision) in &ack.covered_sections {
                frame_oracle(&mut bytes, id);
                frame_oracle(&mut bytes, &revision.to_string());
            }
            frame_oracle(&mut bytes, &ack.kind);
            frame_oracle(&mut bytes, &ack.world_id);
            frame_oracle(&mut bytes, &ack.context_id);
            frame_oracle(
                &mut bytes,
                if ack.generation == item.generation {
                    "current-generation"
                } else {
                    "other-generation"
                },
            );
            frame_oracle(
                &mut bytes,
                if ack.semantic_digest != [0; 32] {
                    "semantic-present"
                } else {
                    "semantic-missing"
                },
            );
            frame_oracle(
                &mut bytes,
                if ack.old_root == ack.new_root {
                    "same-root"
                } else {
                    "advanced-root"
                },
            );
            frame_oracle(
                &mut bytes,
                if ack.new_root == item.published_root {
                    "current-root"
                } else {
                    "non-current-root"
                },
            );
        }
        if let Some(capture) = &item.capture {
            frame_oracle(&mut bytes, &capture.cut_id);
            frame_oracle(&mut bytes, &capture.world_id);
            frame_oracle(&mut bytes, &capture.context_id);
            frame_oracle(
                &mut bytes,
                if capture.generation == item.stamp_generation {
                    "stamp-generation"
                } else {
                    "other-generation"
                },
            );
            frame_oracle(&mut bytes, &capture.world_revision.to_string());
            frame_oracle(&mut bytes, &capture.section_revision_set.len().to_string());
            for (id, revision) in &capture.section_revision_set {
                frame_oracle(&mut bytes, id);
                frame_oracle(&mut bytes, &revision.to_string());
            }
            frame_oracle(&mut bytes, &capture.config_hash);
            frame_oracle(
                &mut bytes,
                if capture.semantic_digest != [0; 32] {
                    "semantic-present"
                } else {
                    "semantic-missing"
                },
            );
            frame_oracle(
                &mut bytes,
                if capture.artifact_hash == item.published_root {
                    "current-root"
                } else {
                    "non-current-root"
                },
            );
        }
        if let Some(restore) = &item.restore {
            frame_oracle(&mut bytes, &restore.cut_id);
            frame_oracle(&mut bytes, &restore.world_id);
            frame_oracle(&mut bytes, &restore.context_id);
            frame_oracle(
                &mut bytes,
                if restore.generation == item.generation {
                    "current-generation"
                } else {
                    "other-generation"
                },
            );
            frame_oracle(&mut bytes, &restore.world_revision.to_string());
            frame_oracle(&mut bytes, &restore.section_revision_set.len().to_string());
            for (id, revision) in &restore.section_revision_set {
                frame_oracle(&mut bytes, id);
                frame_oracle(&mut bytes, &revision.to_string());
            }
            frame_oracle(&mut bytes, &restore.config_hash);
            frame_oracle(
                &mut bytes,
                if restore.semantic_digest != [0; 32] {
                    "semantic-present"
                } else {
                    "semantic-missing"
                },
            );
            frame_oracle(
                &mut bytes,
                if restore.old_root == restore.new_root {
                    "same-root"
                } else {
                    "advanced-root"
                },
            );
            frame_oracle(
                &mut bytes,
                if restore.new_root == item.published_root {
                    "current-root"
                } else {
                    "non-current-root"
                },
            );
        }
        for probe in &item.probes {
            frame_oracle(&mut bytes, &probe.label);
            frame_oracle(&mut bytes, if probe.ok { "ok" } else { "error" });
            frame_oracle(&mut bytes, probe.error_id.as_deref().unwrap_or("<none>"));
            frame_oracle(&mut bytes, &hex32(&probe.state.semantic_digest));
            frame_oracle(
                &mut bytes,
                if probe.capture.is_some() {
                    "capture"
                } else {
                    "<none>"
                },
            );
            frame_oracle(
                &mut bytes,
                if probe.receipt.is_some() {
                    "receipt"
                } else {
                    "<none>"
                },
            );
            frame_oracle(
                &mut bytes,
                if probe.ack.is_some() { "ack" } else { "<none>" },
            );
            frame_oracle(
                &mut bytes,
                if probe.restore.is_some() {
                    "restore"
                } else {
                    "<none>"
                },
            );
            if let Some(receipt) = &probe.receipt {
                frame_oracle(&mut bytes, &receipt.disposition);
                frame_oracle(&mut bytes, &receipt.receipt_len.to_string());
                frame_oracle(
                    &mut bytes,
                    if receipt.fingerprint != [0; 32] {
                        "fingerprint-present"
                    } else {
                        "fingerprint-missing"
                    },
                );
                frame_oracle(
                    &mut bytes,
                    if receipt.semantic_digest != [0; 32] {
                        "semantic-present"
                    } else {
                        "semantic-missing"
                    },
                );
                frame_oracle(
                    &mut bytes,
                    if receipt.old_root == receipt.new_root {
                        "same-root"
                    } else {
                        "advanced-root"
                    },
                );
                frame_oracle(
                    &mut bytes,
                    if receipt.new_root == probe.state.published_root {
                        "current-root"
                    } else {
                        "non-current-root"
                    },
                );
                frame_oracle(
                    &mut bytes,
                    if receipt.wire_verified {
                        "wire-ok"
                    } else {
                        "wire-bad"
                    },
                );
            }
            if let Some(ack) = &probe.ack {
                frame_oracle(&mut bytes, &ack.covered_world_revision.to_string());
                frame_oracle(&mut bytes, &ack.coverage_len.to_string());
                for (id, revision) in &ack.covered_sections {
                    frame_oracle(&mut bytes, id);
                    frame_oracle(&mut bytes, &revision.to_string());
                }
                frame_oracle(&mut bytes, &ack.kind);
                frame_oracle(&mut bytes, &ack.world_id);
                frame_oracle(&mut bytes, &ack.context_id);
                frame_oracle(
                    &mut bytes,
                    if ack.generation == probe.state.generation {
                        "current-generation"
                    } else {
                        "other-generation"
                    },
                );
                frame_oracle(
                    &mut bytes,
                    if ack.semantic_digest != [0; 32] {
                        "semantic-present"
                    } else {
                        "semantic-missing"
                    },
                );
                frame_oracle(
                    &mut bytes,
                    if ack.old_root == ack.new_root {
                        "same-root"
                    } else {
                        "advanced-root"
                    },
                );
                frame_oracle(
                    &mut bytes,
                    if ack.new_root == probe.state.published_root {
                        "current-root"
                    } else {
                        "non-current-root"
                    },
                );
            }
            if let Some(capture) = &probe.capture {
                frame_oracle(&mut bytes, &capture.cut_id);
                frame_oracle(&mut bytes, &capture.world_id);
                frame_oracle(&mut bytes, &capture.context_id);
                frame_oracle(
                    &mut bytes,
                    if capture.generation == probe.state.stamp_generation {
                        "stamp-generation"
                    } else {
                        "other-generation"
                    },
                );
                frame_oracle(&mut bytes, &capture.world_revision.to_string());
                frame_oracle(&mut bytes, &capture.section_revision_set.len().to_string());
                for (id, revision) in &capture.section_revision_set {
                    frame_oracle(&mut bytes, id);
                    frame_oracle(&mut bytes, &revision.to_string());
                }
                frame_oracle(&mut bytes, &capture.config_hash);
                frame_oracle(
                    &mut bytes,
                    if capture.semantic_digest != [0; 32] {
                        "semantic-present"
                    } else {
                        "semantic-missing"
                    },
                );
                frame_oracle(
                    &mut bytes,
                    if capture.artifact_hash == probe.state.published_root {
                        "current-root"
                    } else {
                        "non-current-root"
                    },
                );
            }
            if let Some(restore) = &probe.restore {
                frame_oracle(&mut bytes, &restore.cut_id);
                frame_oracle(&mut bytes, &restore.world_id);
                frame_oracle(&mut bytes, &restore.context_id);
                frame_oracle(
                    &mut bytes,
                    if restore.generation == probe.state.generation {
                        "current-generation"
                    } else {
                        "other-generation"
                    },
                );
                frame_oracle(&mut bytes, &restore.world_revision.to_string());
                frame_oracle(&mut bytes, &restore.section_revision_set.len().to_string());
                for (id, revision) in &restore.section_revision_set {
                    frame_oracle(&mut bytes, id);
                    frame_oracle(&mut bytes, &revision.to_string());
                }
                frame_oracle(&mut bytes, &restore.config_hash);
                frame_oracle(
                    &mut bytes,
                    if restore.semantic_digest != [0; 32] {
                        "semantic-present"
                    } else {
                        "semantic-missing"
                    },
                );
                frame_oracle(
                    &mut bytes,
                    if restore.old_root == restore.new_root {
                        "same-root"
                    } else {
                        "advanced-root"
                    },
                );
                frame_oracle(
                    &mut bytes,
                    if restore.new_root == probe.state.published_root {
                        "current-root"
                    } else {
                        "non-current-root"
                    },
                );
            }
        }
    }
    sha256(&bytes)
}

fn rust_trace_digest(items: &[DifferentialObservation]) -> [u8; 32] {
    let mut bytes = Vec::new();
    for item in items {
        append_rust(&mut bytes, item.sequence.to_string().as_bytes());
        append_rust(&mut bytes, item.operation.as_bytes());
        append_rust(&mut bytes, if item.ok { b"ok" } else { b"error" });
        append_rust(
            &mut bytes,
            item.error_id.as_deref().unwrap_or("<none>").as_bytes(),
        );
        append_rust(&mut bytes, hex32(&item.state_semantic_digest).as_bytes());
        append_rust(&mut bytes, item.publication_epoch.to_string().as_bytes());
        append_rust(
            &mut bytes,
            if item.capture.is_some() {
                b"capture"
            } else {
                b"<none>"
            },
        );
        append_rust(
            &mut bytes,
            if item.receipt.is_some() {
                b"receipt"
            } else {
                b"<none>"
            },
        );
        append_rust(
            &mut bytes,
            if item.ack.is_some() {
                b"ack"
            } else {
                b"<none>"
            },
        );
        append_rust(
            &mut bytes,
            if item.restore.is_some() {
                b"restore"
            } else {
                b"<none>"
            },
        );
        if let Some(receipt) = &item.receipt {
            append_rust(&mut bytes, receipt.disposition.as_bytes());
            append_rust(&mut bytes, receipt.receipt_len.to_string().as_bytes());
            append_rust(
                &mut bytes,
                if receipt.fingerprint != [0; 32] {
                    b"fingerprint-present"
                } else {
                    b"fingerprint-missing"
                },
            );
            append_rust(
                &mut bytes,
                if receipt.semantic_digest != [0; 32] {
                    b"semantic-present"
                } else {
                    b"semantic-missing"
                },
            );
            append_rust(
                &mut bytes,
                if receipt.old_root == receipt.new_root {
                    b"same-root"
                } else {
                    b"advanced-root"
                },
            );
            append_rust(
                &mut bytes,
                if receipt.new_root == item.published_root {
                    b"current-root"
                } else {
                    b"non-current-root"
                },
            );
            append_rust(
                &mut bytes,
                if receipt.wire_verified {
                    b"wire-ok"
                } else {
                    b"wire-bad"
                },
            );
        }
        if let Some(ack) = &item.ack {
            append_rust(
                &mut bytes,
                ack.covered_world_revision.to_string().as_bytes(),
            );
            append_rust(&mut bytes, ack.coverage_len.to_string().as_bytes());
            for (id, revision) in &ack.covered_sections {
                append_rust(&mut bytes, id.as_bytes());
                append_rust(&mut bytes, revision.to_string().as_bytes());
            }
            append_rust(&mut bytes, ack.kind.as_bytes());
            append_rust(&mut bytes, ack.world_id.as_bytes());
            append_rust(&mut bytes, ack.context_id.as_bytes());
            append_rust(
                &mut bytes,
                if ack.generation == item.generation {
                    b"current-generation"
                } else {
                    b"other-generation"
                },
            );
            append_rust(
                &mut bytes,
                if ack.semantic_digest != [0; 32] {
                    b"semantic-present"
                } else {
                    b"semantic-missing"
                },
            );
            append_rust(
                &mut bytes,
                if ack.old_root == ack.new_root {
                    b"same-root"
                } else {
                    b"advanced-root"
                },
            );
            append_rust(
                &mut bytes,
                if ack.new_root == item.published_root {
                    b"current-root"
                } else {
                    b"non-current-root"
                },
            );
        }
        if let Some(capture) = &item.capture {
            append_rust(&mut bytes, capture.cut_id.as_bytes());
            append_rust(&mut bytes, capture.world_id.as_bytes());
            append_rust(&mut bytes, capture.context_id.as_bytes());
            append_rust(
                &mut bytes,
                if capture.generation == item.stamp_generation {
                    b"stamp-generation"
                } else {
                    b"other-generation"
                },
            );
            append_rust(&mut bytes, capture.world_revision.to_string().as_bytes());
            append_rust(
                &mut bytes,
                capture.section_revision_set.len().to_string().as_bytes(),
            );
            for (id, revision) in &capture.section_revision_set {
                append_rust(&mut bytes, id.as_bytes());
                append_rust(&mut bytes, revision.to_string().as_bytes());
            }
            append_rust(&mut bytes, capture.config_hash.as_bytes());
            append_rust(
                &mut bytes,
                if capture.semantic_digest != [0; 32] {
                    b"semantic-present"
                } else {
                    b"semantic-missing"
                },
            );
            append_rust(
                &mut bytes,
                if capture.artifact_hash == item.published_root {
                    b"current-root"
                } else {
                    b"non-current-root"
                },
            );
        }
        if let Some(restore) = &item.restore {
            append_rust(&mut bytes, restore.cut_id.as_bytes());
            append_rust(&mut bytes, restore.world_id.as_bytes());
            append_rust(&mut bytes, restore.context_id.as_bytes());
            append_rust(
                &mut bytes,
                if restore.generation == item.generation {
                    b"current-generation"
                } else {
                    b"other-generation"
                },
            );
            append_rust(&mut bytes, restore.world_revision.to_string().as_bytes());
            append_rust(
                &mut bytes,
                restore.section_revision_set.len().to_string().as_bytes(),
            );
            for (id, revision) in &restore.section_revision_set {
                append_rust(&mut bytes, id.as_bytes());
                append_rust(&mut bytes, revision.to_string().as_bytes());
            }
            append_rust(&mut bytes, restore.config_hash.as_bytes());
            append_rust(
                &mut bytes,
                if restore.semantic_digest != [0; 32] {
                    b"semantic-present"
                } else {
                    b"semantic-missing"
                },
            );
            append_rust(
                &mut bytes,
                if restore.old_root == restore.new_root {
                    b"same-root"
                } else {
                    b"advanced-root"
                },
            );
            append_rust(
                &mut bytes,
                if restore.new_root == item.published_root {
                    b"current-root"
                } else {
                    b"non-current-root"
                },
            );
        }
        for probe in &item.probes {
            append_rust(&mut bytes, probe.label.as_bytes());
            append_rust(&mut bytes, if probe.ok { b"ok" } else { b"error" });
            append_rust(
                &mut bytes,
                probe.error_id.as_deref().unwrap_or("<none>").as_bytes(),
            );
            append_rust(&mut bytes, hex32(&probe.state.semantic_digest).as_bytes());
            append_rust(
                &mut bytes,
                if probe.capture.is_some() {
                    b"capture"
                } else {
                    b"<none>"
                },
            );
            append_rust(
                &mut bytes,
                if probe.receipt.is_some() {
                    b"receipt"
                } else {
                    b"<none>"
                },
            );
            append_rust(
                &mut bytes,
                if probe.ack.is_some() {
                    b"ack"
                } else {
                    b"<none>"
                },
            );
            append_rust(
                &mut bytes,
                if probe.restore.is_some() {
                    b"restore"
                } else {
                    b"<none>"
                },
            );
            if let Some(receipt) = &probe.receipt {
                append_rust(&mut bytes, receipt.disposition.as_bytes());
                append_rust(&mut bytes, receipt.receipt_len.to_string().as_bytes());
                append_rust(
                    &mut bytes,
                    if receipt.fingerprint != [0; 32] {
                        b"fingerprint-present"
                    } else {
                        b"fingerprint-missing"
                    },
                );
                append_rust(
                    &mut bytes,
                    if receipt.semantic_digest != [0; 32] {
                        b"semantic-present"
                    } else {
                        b"semantic-missing"
                    },
                );
                append_rust(
                    &mut bytes,
                    if receipt.old_root == receipt.new_root {
                        b"same-root"
                    } else {
                        b"advanced-root"
                    },
                );
                append_rust(
                    &mut bytes,
                    if receipt.new_root == probe.state.published_root {
                        b"current-root"
                    } else {
                        b"non-current-root"
                    },
                );
                append_rust(
                    &mut bytes,
                    if receipt.wire_verified {
                        b"wire-ok"
                    } else {
                        b"wire-bad"
                    },
                );
            }
            if let Some(ack) = &probe.ack {
                append_rust(
                    &mut bytes,
                    ack.covered_world_revision.to_string().as_bytes(),
                );
                append_rust(&mut bytes, ack.coverage_len.to_string().as_bytes());
                for (id, revision) in &ack.covered_sections {
                    append_rust(&mut bytes, id.as_bytes());
                    append_rust(&mut bytes, revision.to_string().as_bytes());
                }
                append_rust(&mut bytes, ack.kind.as_bytes());
                append_rust(&mut bytes, ack.world_id.as_bytes());
                append_rust(&mut bytes, ack.context_id.as_bytes());
                append_rust(
                    &mut bytes,
                    if ack.generation == probe.state.generation {
                        b"current-generation"
                    } else {
                        b"other-generation"
                    },
                );
                append_rust(
                    &mut bytes,
                    if ack.semantic_digest != [0; 32] {
                        b"semantic-present"
                    } else {
                        b"semantic-missing"
                    },
                );
                append_rust(
                    &mut bytes,
                    if ack.old_root == ack.new_root {
                        b"same-root"
                    } else {
                        b"advanced-root"
                    },
                );
                append_rust(
                    &mut bytes,
                    if ack.new_root == probe.state.published_root {
                        b"current-root"
                    } else {
                        b"non-current-root"
                    },
                );
            }
            if let Some(capture) = &probe.capture {
                append_rust(&mut bytes, capture.cut_id.as_bytes());
                append_rust(&mut bytes, capture.world_id.as_bytes());
                append_rust(&mut bytes, capture.context_id.as_bytes());
                append_rust(
                    &mut bytes,
                    if capture.generation == probe.state.stamp_generation {
                        b"stamp-generation"
                    } else {
                        b"other-generation"
                    },
                );
                append_rust(&mut bytes, capture.world_revision.to_string().as_bytes());
                append_rust(
                    &mut bytes,
                    capture.section_revision_set.len().to_string().as_bytes(),
                );
                for (id, revision) in &capture.section_revision_set {
                    append_rust(&mut bytes, id.as_bytes());
                    append_rust(&mut bytes, revision.to_string().as_bytes());
                }
                append_rust(&mut bytes, capture.config_hash.as_bytes());
                append_rust(
                    &mut bytes,
                    if capture.semantic_digest != [0; 32] {
                        b"semantic-present"
                    } else {
                        b"semantic-missing"
                    },
                );
                append_rust(
                    &mut bytes,
                    if capture.artifact_hash == probe.state.published_root {
                        b"current-root"
                    } else {
                        b"non-current-root"
                    },
                );
            }
            if let Some(restore) = &probe.restore {
                append_rust(&mut bytes, restore.cut_id.as_bytes());
                append_rust(&mut bytes, restore.world_id.as_bytes());
                append_rust(&mut bytes, restore.context_id.as_bytes());
                append_rust(
                    &mut bytes,
                    if restore.generation == probe.state.generation {
                        b"current-generation"
                    } else {
                        b"other-generation"
                    },
                );
                append_rust(&mut bytes, restore.world_revision.to_string().as_bytes());
                append_rust(
                    &mut bytes,
                    restore.section_revision_set.len().to_string().as_bytes(),
                );
                for (id, revision) in &restore.section_revision_set {
                    append_rust(&mut bytes, id.as_bytes());
                    append_rust(&mut bytes, revision.to_string().as_bytes());
                }
                append_rust(&mut bytes, restore.config_hash.as_bytes());
                append_rust(
                    &mut bytes,
                    if restore.semantic_digest != [0; 32] {
                        b"semantic-present"
                    } else {
                        b"semantic-missing"
                    },
                );
                append_rust(
                    &mut bytes,
                    if restore.old_root == restore.new_root {
                        b"same-root"
                    } else {
                        b"advanced-root"
                    },
                );
                append_rust(
                    &mut bytes,
                    if restore.new_root == probe.state.published_root {
                        b"current-root"
                    } else {
                        b"non-current-root"
                    },
                );
            }
        }
    }
    sha256(&bytes)
}

fn observations_equal_for_contract(
    reference: &[DifferentialObservation],
    rust: &[DifferentialObservation],
) -> bool {
    reference.len() == rust.len()
        && root_transition_shapes_match(reference, rust)
        && reference
            .iter()
            .zip(rust)
            .all(|(left, right)| observation_matches(left, right))
}

fn observation_matches(a: &DifferentialObservation, b: &DifferentialObservation) -> bool {
    a.sequence == b.sequence
        && a.operation == b.operation
        && a.ok == b.ok
        && a.error_id == b.error_id
        && a.lifecycle == b.lifecycle
        && a.lifecycle_machine == b.lifecycle_machine
        && a.world_id == b.world_id
        && a.context_id == b.context_id
        && a.generation == b.generation
        && a.stamp_generation == b.stamp_generation
        && a.world_revision == b.world_revision
        && a.section_presence == b.section_presence
        && a.section_revision_set == b.section_revision_set
        && sections_match(&a.sections, &b.sections)
        && a.published_root != [0; 32]
        && b.published_root != [0; 32]
        && a.contract_root == b.contract_root
        && a.directory_digest == b.directory_digest
        && a.dirty_frontier == b.dirty_frontier
        && a.dirty_digest == b.dirty_digest
        && a.config_hash == b.config_hash
        && a.baseline_id == b.baseline_id
        && a.schema_epoch == b.schema_epoch
        && a.gate_source_hashes == b.gate_source_hashes
        && a.publication_epoch == b.publication_epoch
        && a.state_semantic_digest == b.state_semantic_digest
        && optional_capture_match(a.capture.as_ref(), b.capture.as_ref())
        && optional_receipt_match(a.receipt.as_ref(), b.receipt.as_ref())
        && optional_ack_match(a.ack.as_ref(), b.ack.as_ref())
        && optional_restore_match(a.restore.as_ref(), b.restore.as_ref())
        && a.probes.len() == b.probes.len()
        && a.probes
            .iter()
            .zip(&b.probes)
            .all(|(left, right)| probe_matches(left, right))
}

fn sections_match(a: &[SectionStateEvidence], b: &[SectionStateEvidence]) -> bool {
    a.len() == b.len()
        && a.iter().zip(b).all(|(left, right)| {
            left.section_id == right.section_id
                && left.presence == right.presence
                && left.section_revision == right.section_revision
                && left.payload_digest == right.payload_digest
        })
}

fn optional_capture_match(a: Option<&CaptureObservation>, b: Option<&CaptureObservation>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(left), Some(right)) => {
            left.cut_id == right.cut_id
                && left.world_id == right.world_id
                && left.context_id == right.context_id
                && left.generation == right.generation
                && left.world_revision == right.world_revision
                && left.section_revision_set == right.section_revision_set
                && left.config_hash == right.config_hash
                && left.artifact_hash != [0; 32]
                && right.artifact_hash != [0; 32]
                && left.semantic_digest == right.semantic_digest
        }
        _ => false,
    }
}

fn optional_receipt_match(a: Option<&ReceiptObservation>, b: Option<&ReceiptObservation>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(left), Some(right)) => {
            left.txn_id == right.txn_id
                && left.disposition == right.disposition
                && left.fingerprint == right.fingerprint
                && left.receipt_len == right.receipt_len
                && left.receipt_hash != [0; 32]
                && right.receipt_hash != [0; 32]
                && (left.old_root == left.new_root) == (right.old_root == right.new_root)
                && left.old_root != [0; 32]
                && left.new_root != [0; 32]
                && right.old_root != [0; 32]
                && right.new_root != [0; 32]
                && left.wire_verified
                && right.wire_verified
                && left.semantic_digest == right.semantic_digest
        }
        _ => false,
    }
}

fn optional_ack_match(a: Option<&AckObservation>, b: Option<&AckObservation>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(left), Some(right)) => {
            left.kind == right.kind
                && left.world_id == right.world_id
                && left.context_id == right.context_id
                && left.generation == right.generation
                && left.covered_world_revision == right.covered_world_revision
                && left.covered_sections == right.covered_sections
                && left.coverage_len == right.coverage_len
                && left.old_root != [0; 32]
                && left.new_root != [0; 32]
                && right.old_root != [0; 32]
                && right.new_root != [0; 32]
                && (left.old_root == left.new_root) == (right.old_root == right.new_root)
                && left.semantic_digest == right.semantic_digest
        }
        _ => false,
    }
}

fn optional_restore_match(a: Option<&RestoreObservation>, b: Option<&RestoreObservation>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(left), Some(right)) => {
            left.cut_id == right.cut_id
                && left.world_id == right.world_id
                && left.context_id == right.context_id
                && left.generation == right.generation
                && left.world_revision == right.world_revision
                && left.section_revision_set == right.section_revision_set
                && left.config_hash == right.config_hash
                && left.old_root != [0; 32]
                && left.new_root != [0; 32]
                && right.old_root != [0; 32]
                && right.new_root != [0; 32]
                && left.semantic_digest == right.semantic_digest
        }
        _ => false,
    }
}

fn probe_matches(a: &ProbeObservation, b: &ProbeObservation) -> bool {
    a.label == b.label
        && a.ok == b.ok
        && a.error_id == b.error_id
        && a.returned_sections == b.returned_sections
        && state_matches_for_contract(&a.state, &b.state)
        && optional_capture_match(a.capture.as_ref(), b.capture.as_ref())
        && optional_receipt_match(a.receipt.as_ref(), b.receipt.as_ref())
        && optional_ack_match(a.ack.as_ref(), b.ack.as_ref())
        && optional_restore_match(a.restore.as_ref(), b.restore.as_ref())
}

fn state_matches_for_contract(a: &StateEvidence, b: &StateEvidence) -> bool {
    a.world_id == b.world_id
        && a.context_id == b.context_id
        && a.generation == b.generation
        && a.stamp_generation == b.stamp_generation
        && a.lifecycle_machine == b.lifecycle_machine
        && a.lifecycle == b.lifecycle
        && a.world_revision == b.world_revision
        && a.section_revision_set == b.section_revision_set
        && sections_match(&a.sections, &b.sections)
        && a.published_root != [0; 32]
        && b.published_root != [0; 32]
        && a.contract_root == b.contract_root
        && a.directory_digest == b.directory_digest
        && a.dirty_frontier == b.dirty_frontier
        && a.dirty_digest == b.dirty_digest
        && a.config_hash == b.config_hash
        && a.baseline_id == b.baseline_id
        && a.schema_epoch == b.schema_epoch
        && a.gate_source_hashes == b.gate_source_hashes
        && a.publication_epoch == b.publication_epoch
        && a.semantic_digest == b.semantic_digest
}

fn root_transition_shapes_match(
    reference: &[DifferentialObservation],
    rust: &[DifferentialObservation],
) -> bool {
    if reference.len() != rust.len() {
        return false;
    }
    reference
        .iter()
        .zip(rust)
        .enumerate()
        .all(|(index, (left, right))| {
            let previous_reference = index
                .checked_sub(1)
                .and_then(|previous| reference.get(previous));
            let previous_rust = index.checked_sub(1).and_then(|previous| rust.get(previous));
            left.published_root != [0; 32]
                && right.published_root != [0; 32]
                && previous_reference
                    .zip(previous_rust)
                    .is_none_or(|(old_reference, old_rust)| {
                        (left.published_root == old_reference.published_root)
                            == (right.published_root == old_rust.published_root)
                            && (left.contract_root == old_reference.contract_root)
                                == (right.contract_root == old_rust.contract_root)
                    })
        })
}

fn roots_trace_valid(trace: &[DifferentialObservation]) -> bool {
    let mut previous: Option<([u8; 32], u64)> = None;
    for observation in trace {
        if observation.published_root == [0; 32] {
            return false;
        }
        if observation.contract_root == [0; 32]
            || observation.contract_root != observation.state_semantic_digest
        {
            return false;
        }
        if let Some((old_root, old_epoch)) = previous {
            if observation.publication_epoch < old_epoch {
                return false;
            }
            if observation.publication_epoch == old_epoch && observation.published_root != old_root
            {
                return false;
            }
            if observation.publication_epoch > old_epoch && observation.published_root == old_root {
                return false;
            }
        }
        for receipt in observation.receipt.iter().chain(
            observation
                .probes
                .iter()
                .filter_map(|probe| probe.receipt.as_ref()),
        ) {
            let expected_receipt = oracle_receipt_bytes(
                &receipt.txn_id,
                receipt.old_root,
                receipt.new_root,
                receipt.fingerprint,
            );
            if receipt.old_root == [0; 32]
                || receipt.new_root == [0; 32]
                || receipt.fingerprint == [0; 32]
                || receipt.receipt_hash == [0; 32]
                || receipt.receipt_len != expected_receipt.len()
                || receipt.receipt_hash != sha256(&expected_receipt)
                || receipt.semantic_digest == [0; 32]
                || receipt.new_root != observation.published_root
                || (receipt.disposition == "Original"
                    && previous.is_some_and(|(root, _)| receipt.old_root != root))
            {
                return false;
            }
        }
        for ack in observation.ack.iter().chain(
            observation
                .probes
                .iter()
                .filter_map(|probe| probe.ack.as_ref()),
        ) {
            if ack.old_root == [0; 32]
                || ack.new_root == [0; 32]
                || ack.semantic_digest == [0; 32]
                || (ack.coverage_len > 0 && ack.old_root == ack.new_root)
                || (ack.coverage_len == 0 && ack.old_root != ack.new_root)
            {
                return false;
            }
        }
        for capture in observation.capture.iter().chain(
            observation
                .probes
                .iter()
                .filter_map(|probe| probe.capture.as_ref()),
        ) {
            if capture.artifact_hash == [0; 32]
                || capture.artifact_hash != observation.published_root
                || capture.semantic_digest == [0; 32]
            {
                return false;
            }
        }
        for restore in observation.restore.iter().chain(
            observation
                .probes
                .iter()
                .filter_map(|probe| probe.restore.as_ref()),
        ) {
            if restore.old_root == [0; 32]
                || restore.new_root == [0; 32]
                || restore.old_root == restore.new_root
                || restore.new_root != observation.published_root
                || restore.semantic_digest == [0; 32]
            {
                return false;
            }
        }
        for probe in &observation.probes {
            if let Some(receipt) = &probe.receipt
                && (receipt.new_root != probe.state.published_root
                    || receipt.old_root == [0; 32]
                    || receipt.new_root == [0; 32])
            {
                return false;
            }
            if let Some(ack) = &probe.ack
                && (ack.new_root != probe.state.published_root
                    || ack.old_root == [0; 32]
                    || ack.new_root == [0; 32])
            {
                return false;
            }
            if let Some(capture) = &probe.capture
                && (capture.artifact_hash != probe.state.published_root
                    || capture.artifact_hash == [0; 32])
            {
                return false;
            }
            if let Some(restore) = &probe.restore
                && (restore.new_root != probe.state.published_root
                    || restore.old_root == [0; 32]
                    || restore.new_root == [0; 32])
            {
                return false;
            }
        }
        previous = Some((observation.published_root, observation.publication_epoch));
    }
    true
}

fn observation_complete(observation: &DifferentialObservation) -> bool {
    !observation.world_id.is_empty()
        && !observation.context_id.is_empty()
        && observation.generation > 0
        && observation.stamp_generation > 0
        && !observation.lifecycle_machine.is_empty()
        && !observation.config_hash.is_empty()
        && observation.baseline_id == BASELINE_ID
        && observation.schema_epoch == SCHEMA_EPOCH
        && observation.gate_source_hashes.len() == 5
        && observation
            .gate_source_hashes
            .iter()
            .all(|(_, value)| value != "differential" && !value.is_empty())
        && observation.section_revision_set.len() >= READY_SECTIONS.len()
        && observation.sections.len() == KNOWN_SECTIONS.len()
        && observation.dirty_frontier.len() == KNOWN_SECTIONS.len()
        && section_projection_complete(
            &observation.section_revision_set,
            &observation.sections,
            &observation.dirty_frontier,
        )
        && observation.published_root != [0; 32]
        && observation.contract_root != [0; 32]
        && observation.contract_root == observation.state_semantic_digest
        && observation.directory_digest != [0; 32]
        && observation.dirty_digest != [0; 32]
        && observation.state_semantic_digest != [0; 32]
        && observation.sections.iter().all(|section| {
            if section.presence.as_deref() == Some("Ready") {
                section.payload_digest.is_some()
            } else {
                section.payload_digest.is_none()
            }
        })
        && observation
            .probes
            .iter()
            .all(|probe| state_complete(&probe.state))
}

fn state_complete(state: &StateEvidence) -> bool {
    !state.world_id.is_empty()
        && !state.context_id.is_empty()
        && state.generation > 0
        && state.stamp_generation > 0
        && !state.lifecycle_machine.is_empty()
        && !state.config_hash.is_empty()
        && state.baseline_id == BASELINE_ID
        && state.schema_epoch == SCHEMA_EPOCH
        && state.gate_source_hashes.len() == 5
        && state
            .gate_source_hashes
            .iter()
            .all(|(_, value)| value != "differential" && !value.is_empty())
        && state.section_revision_set.len() >= READY_SECTIONS.len()
        && state.sections.len() == KNOWN_SECTIONS.len()
        && state.dirty_frontier.len() == KNOWN_SECTIONS.len()
        && section_projection_complete(
            &state.section_revision_set,
            &state.sections,
            &state.dirty_frontier,
        )
        && state.published_root != [0; 32]
        && state.contract_root != [0; 32]
        && state.contract_root == state.semantic_digest
        && state.directory_digest != [0; 32]
        && state.dirty_digest != [0; 32]
        && state.semantic_digest != [0; 32]
        && state.sections.iter().all(|section| {
            if section.presence.as_deref() == Some("Ready") {
                section.payload_digest.is_some()
            } else {
                section.payload_digest.is_none()
            }
        })
}

fn section_projection_complete(
    revisions: &[(String, u64)],
    sections: &[SectionStateEvidence],
    dirty: &[DirtyStateEvidence],
) -> bool {
    let revision_ids = revisions
        .iter()
        .map(|(id, _)| id.as_str())
        .collect::<BTreeSet<_>>();
    let expected_revisions = READY_SECTIONS.iter().copied().collect::<BTreeSet<_>>();
    let section_ids = sections
        .iter()
        .map(|section| section.section_id.as_str())
        .collect::<Vec<_>>();
    let dirty_ids = dirty
        .iter()
        .map(|entry| entry.section_id.as_str())
        .collect::<Vec<_>>();
    revision_ids == expected_revisions
        && section_ids == KNOWN_SECTIONS
        && dirty_ids == KNOWN_SECTIONS
        && sections.iter().all(|section| {
            section.presence.as_deref() == Some("Ready")
                || section.presence.as_deref() == Some("Unchanged")
                || section.presence.is_none()
        })
}

fn rust_origin(
    world: &VoxelWorld,
    request_id: &str,
    context: Option<&str>,
    generation: Option<u64>,
) -> Result<OriginToken, String> {
    let guard = world.generation_guard();
    OriginToken::try_new(
        context.unwrap_or(guard.world_context_id()),
        generation.unwrap_or(guard.generation()),
        request_id,
        0,
        BTreeMap::new(),
        "VoxelCommit",
    )
    .map_err(|e| e.error_id().to_string())
}

fn create_differential_world() -> Result<VoxelWorld, String> {
    VoxelWorld::create(
        WorldDescriptor {
            role: "Authority".into(),
            world_context_id: CONTEXT_ID.into(),
            capabilities: vec!["Native".into(), "ReferenceVoxel".into()],
            config: WorldConfigAdapter {
                world_id: WORLD_ID.into(),
            },
        },
        differential_snapshot(CONFIG_LABEL)?,
    )
    .map_err(|e| e.error_id().to_string())
}

fn seed_differential_ready_sections(world: &mut VoxelWorld) -> Result<(), String> {
    let view = world.publication_authority().capture();
    let next = view
        .stamp()
        .world_revision
        .checked_add(1)
        .ok_or_else(|| "seed overflow".to_string())?;
    let mut directory = SectionDirectoryBuilder::new();
    for id in READY_SECTIONS {
        let bytes = format!("seed:{id}").into_bytes();
        let payload = SectionPayload::from_pages([SectionPage::new(
            "Dense",
            "None",
            bytes.clone(),
            sha256(&bytes),
        )])
        .map_err(|e| e.error_id().to_string())?;
        directory
            .insert(id, SectionSlot::ready(payload))
            .map_err(|e| e.error_id().to_string())?;
    }
    let mut revisions = BTreeMap::new();
    for id in READY_SECTIONS {
        revisions.insert((*id).into(), next);
    }
    let stamp = GeneratedRevisionStamp {
        schema_id: REVISION_STAMP_SCHEMA,
        world_id: view.stamp().world_id.clone(),
        context_id: view.stamp().context_id.clone(),
        generation: view.stamp().generation,
        world_revision: next,
        section_revision_set: revisions,
    };
    let dirty = DirtyFrontier::new(WORLD_ID, view.stamp().generation)
        .map_err(|e| e.error_id().to_string())?;
    let root = PublishedStateRoot::new(stamp, directory.freeze(), dirty);
    let replacement = SectionDeltaBuilder::new(view.directory())
        .freeze()
        .map_err(|e| e.error_id().to_string())?;
    let mut prepared = world
        .publication_authority()
        .prepare(world_revision(next)?, root, replacement)
        .map_err(|e| e.error_id().to_string())?;
    let token = prepared.seal().map_err(|e| e.error_id().to_string())?;
    world
        .publication_authority()
        .publish_once(token)
        .map_err(|e| e.error_id().to_string())?;
    Ok(())
}

fn differential_snapshot(label: &str) -> Result<Arc<VoxelConfigSnapshot>, String> {
    let source = GateSourceHashes {
        architecture_baseline_id: BASELINE_ID.into(),
        voxel_head: "61cb864978dedfe9bdf7b687fea08660b31469f1".into(),
        architecture_mirror_sha256:
            "f1d36acf33a1f5e8326a9e58d609fcf7d9fa85177f9b5b60bb3f4742c1afebd0".into(),
        v13_decision_gates_sha256:
            "4850057dd8926c11c8c3beebe109d18dffdb7e84cd451426d7d635860be5ede2".into(),
        blueprint_sha256: "32e76066eb298aad20f4149760abbeddacb6d6c43e096945f1cf0ea75b2471aa".into(),
    };
    let digests = P0_DECISION_GATES
        .iter()
        .map(|gate| {
            (
                (*gate).into(),
                hex32(&sha256(format!("approved-{gate}").as_bytes())),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let evidence = P0_DECISION_GATES
        .iter()
        .map(|gate| DecisionEvidence {
            gate_id: (*gate).into(),
            approval_status: "approved".into(),
            source_hashes: source.clone(),
            evidence_digest: digests[*gate].clone(),
        })
        .collect::<Vec<_>>();
    let generated = GeneratedVoxelConfig {
        schema_id: "config-table",
        host_capability_schema_id: "host-capability",
        schema_epoch: SCHEMA_EPOCH,
        config_hash: config_hash_for(label),
        gate_source_hashes: digests,
        host_capability: GeneratedHostCapability::from_names(["Native", "ReferenceVoxel"]),
        start_capabilities: vec!["Native".into(), "ReferenceVoxel".into()],
        key_material: None,
    };
    VoxelConfigSnapshot::from_generated(&generated, &evidence).map_err(|e| e.to_string())
}

fn config_hash_for(label: &str) -> String {
    hex32(&sha256(label.as_bytes()))
}

fn provenance_pairs() -> Vec<(String, String)> {
    vec![
        ("architectureBaselineId".into(), BASELINE_ID.into()),
        (
            "voxelHead".into(),
            "61cb864978dedfe9bdf7b687fea08660b31469f1".into(),
        ),
        (
            "architectureMirrorSha256".into(),
            "f1d36acf33a1f5e8326a9e58d609fcf7d9fa85177f9b5b60bb3f4742c1afebd0".into(),
        ),
        (
            "v13DecisionGatesSha256".into(),
            "4850057dd8926c11c8c3beebe109d18dffdb7e84cd451426d7d635860be5ede2".into(),
        ),
        (
            "blueprintSha256".into(),
            "32e76066eb298aad20f4149760abbeddacb6d6c43e096945f1cf0ea75b2471aa".into(),
        ),
    ]
}

fn world_revision(value: u64) -> Result<WorldRevision, String> {
    let mut allocator = RevisionAllocator::new();
    for _ in 0..value {
        allocator
            .reserve_world()
            .map_err(|e| e.error_id().to_string())?
            .abandon();
    }
    allocator
        .reserve_world()
        .map_err(|e| e.error_id().to_string())?
        .finalize()
        .map_err(|e| e.error_id().to_string())
}

fn oracle_operation_label(sequence: u64) -> &'static str {
    match sequence {
        0 => "initialize",
        1 => "prime",
        2 => "start",
        3 | 4 => "query",
        5 => "prepareMutation",
        6 => "commit",
        7 => "commitReplayMatrix",
        8 => "captureRestoreMatrix",
        9 => "applyDurabilityAckMatrix",
        10 => "shutdown",
        _ => "invalid",
    }
}

fn rust_operation_label(sequence: u64) -> &'static str {
    match sequence {
        0 => "initialize",
        1 => "prime",
        2 => "start",
        3 | 4 => "query",
        5 => "prepareMutation",
        6 => "commit",
        7 => "commitReplayMatrix",
        8 => "captureRestoreMatrix",
        9 => "applyDurabilityAckMatrix",
        10 => "shutdown",
        _ => "invalid",
    }
}

fn oracle_route(sequence: u64) -> &'static str {
    match sequence {
        0..=2 => "ReferenceVoxel.lifecycle",
        3 | 4 => "ReferenceVoxel.query",
        5 => "ReferenceVoxel.prepareMutation",
        6 | 7 => "ReferenceVoxel.commit",
        8 => "ReferenceVoxel.capture+restore-preflight",
        9 => "ReferenceVoxel.applyDurabilityAck+restore",
        10 => "SimulationSession.shutdown",
        _ => "invalid",
    }
}

fn rust_route(sequence: u64) -> &'static str {
    match sequence {
        0..=2 => "GeneratedVoxelWorldPortAdapter.admit(SimulationSession)",
        3 | 4 => "GeneratedVoxelWorldPortAdapter.query",
        5 => "GeneratedVoxelWorldPortAdapter.prepare_mutation",
        6 | 7 => "GeneratedVoxelWorldPortAdapter.commit+prepare_mutation",
        8 => "GeneratedVoxelWorldPortAdapter.capture+RestorePreflight",
        9 => "GeneratedVoxelWorldPortAdapter.apply_durability_ack+restore",
        10 => "GeneratedVoxelWorldPortAdapter.shutdown",
        _ => "invalid",
    }
}

fn rust_stale_replay_probe(
    run: &mut RustRun,
    request: &OriginEnvelope<MutationRequest>,
) -> Result<ProbeObservation, String> {
    let mut stale = request.clone();
    let generation = run.world.state_view().instance_generation();
    stale.origin = rust_origin(&run.world, "stale-replay", None, Some(generation + 1))?;
    let result = GeneratedVoxelWorldPortAdapter::new(&mut run.world).prepare_mutation(stale);
    let (ok, error) = match result {
        Ok(_) => (true, None),
        Err(e) => (false, Some(e.error_id().into())),
    };
    Ok(ProbeObservation {
        label: "stale-replay".into(),
        ok,
        error_id: error,
        returned_sections: Vec::new(),
        state: rust_state(run),
        capture: None,
        receipt: None,
        ack: None,
        restore: None,
    })
}
