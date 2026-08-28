//! MVP vertical slice: create→query→prepare→commit→duplicate replay→capture→encode→restore→durability ack→close.
//!
//! Chain traffic goes through `GeneratedVoxelWorldPortAdapter`. Four-state / Ready
//! directory fixtures still use the copied B2 `PublicationAuthority` helpers because
//! adapter mutation can only overlay `Ready` slots.

#![forbid(unsafe_code)]

use crate::b0_harness::{B0VerificationReport, run_b0_matrix};
use crate::b2_harness::{B2VerificationReport, run_b2_matrix};
use crate::deterministic_executor::{DeterministicExecutor, Schedule};
use crate::reference_harness::GeneratedVoxelOperation;
use crate::workspace_root_from_manifest;
use lumio_voxel_contracts::{
    BASELINE_ID, CHUNK_PRESENCE, SCHEMA_EPOCH, SCHEMA_IDS, STABLE_ERROR_IDS, sha256,
    verify_artifact_hashes,
};
use lumio_voxel_domain::chunk::{
    ChunkDeltaBuilder, ChunkDirectoryBuilder, ChunkDirectoryRoot, ChunkPage, ChunkPayload,
    ChunkReplacement, ChunkSlot, CoveredChunkAck, DirtyFrontier, DurabilityAckContext,
};
use lumio_voxel_domain::config_snapshot::{
    DecisionEvidence, GateSourceHashes, GeneratedHostCapability, GeneratedVoxelConfig,
    P0_DECISION_GATES, VoxelConfigSnapshot,
};
use lumio_voxel_domain::publication::PublishedStateRoot;
use lumio_voxel_domain::revision::{
    GeneratedRevisionStamp, REVISION_STAMP_SCHEMA, RevisionAllocator, WorldRevision,
};
use lumio_voxel_ops::SNAPSHOT_FEATURE;
use lumio_voxel_ops::async_support::{APPLY_PHASES, OriginEnvelope, OriginToken};
use lumio_voxel_ops::mutation::{GeneratedMutationReceipt, MutationRequest, PreparedMutation};
use lumio_voxel_ops::query::{GeneratedVoxelQueryRequest, QUERY_SCHEMA};
use lumio_voxel_ops::snapshot::{
    MemoryCaptureWriter, RestorePreflight, RestoreShadowBuilder, VoxelCaptureRef, encode_capture,
};
use lumio_voxel_world::port::GeneratedVoxelWorldPortAdapter;
use lumio_voxel_world::world::{
    AckEvidence, RuntimeSnapshotCut, VoxelWorld, WorldCommand, WorldConfigAdapter, WorldDescriptor,
    WorldEventSink, intern_local_embedded_pair,
};
use std::collections::BTreeMap;
use std::sync::Arc;

pub const STEP_COUNT: usize = 10;

pub const STEP_NAMES: [&str; STEP_COUNT] = [
    "create",
    "query",
    "prepare",
    "commit",
    "duplicate_replay",
    "capture",
    "encode",
    "restore",
    "durability_ack",
    "close",
];

const STEPS: [(&str, &str); STEP_COUNT] = [
    ("1", "create"),
    ("2", "query"),
    ("3", "prepare"),
    ("4", "commit"),
    ("5", "duplicate_replay"),
    ("6", "capture"),
    ("7", "encode"),
    ("8", "restore"),
    ("9", "durability_ack"),
    ("10", "close"),
];

const SEED_A: u64 = 0x00A1_1CE0;
const SEED_B: u64 = 0x00B0_5EED;

const FOUR_STATE_IDS: [&str; 5] = ["c:4:0:0", "c:3:0:0", "c:2:0:0", "c:1:0:0", "c:0:0:0"];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MvpStepResult {
    pub id: &'static str,
    pub name: &'static str,
    pub ok: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MvpReceipt {
    pub txn_id: String,
    pub old_root: [u8; 32],
    pub new_root: [u8; 32],
    pub receipt_hash: [u8; 32],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MvpIntegrationReport {
    pub baseline: &'static str,
    pub commit: String,
    pub artifact_hashes: String,
    pub config_hash: String,
    pub steps: Vec<MvpStepResult>,
    pub receipts: Vec<MvpReceipt>,
    pub snapshot_hash: [u8; 32],
    pub restored_hash: [u8; 32],
    pub trace_hash: [u8; 32],
    pub commands: Vec<String>,
    pub authority_identity: [u8; 32],
    pub replica_identity: [u8; 32],
}

impl MvpIntegrationReport {
    pub fn all_ok(&self) -> bool {
        self.steps.len() == STEP_COUNT
            && self.steps.iter().all(|step| step.ok)
            && self.authority_identity != self.replica_identity
            && !self.artifact_hashes.starts_with("verify_artifact_hashes:")
    }
}

pub type B0MatrixFn = fn() -> B0VerificationReport;
pub type B2MatrixFn = fn() -> B2VerificationReport;

/// Type-check presence of B0/B2 matrices. Not invoked at runtime (this host needs `link.exe`).
pub fn matrix_entry_points() -> (B0MatrixFn, B2MatrixFn) {
    (run_b0_matrix, run_b2_matrix)
}

pub fn run_mvp_vertical_slice() -> MvpIntegrationReport {
    let mut report = MvpIntegrationReport {
        baseline: BASELINE_ID,
        commit: git_head(),
        artifact_hashes: String::new(),
        config_hash: String::new(),
        steps: Vec::with_capacity(STEP_COUNT),
        receipts: Vec::new(),
        snapshot_hash: [0; 32],
        restored_hash: [0; 32],
        trace_hash: [0; 32],
        commands: Vec::new(),
        authority_identity: [0; 32],
        replica_identity: [0; 32],
    };

    let (_b0, _b2) = matrix_entry_points();
    report.commands.push(
        "run_b0_matrix/run_b2_matrix present (not invoked; runtime also needs link.exe)".into(),
    );

    match reference_trace() {
        Ok(hash) => report.trace_hash = hash,
        Err(err) => report.commands.push(format!("reference_trace: {err}")),
    }

    match verify_artifact_hashes() {
        Ok(()) => {
            let mut buf = Vec::new();
            for id in SCHEMA_IDS {
                buf.extend_from_slice(id.as_bytes());
                buf.push(0);
            }
            report.artifact_hashes = hex32(&sha256(&buf));
        }
        Err(err) => {
            report.artifact_hashes = format!("verify_artifact_hashes: {err}");
        }
    }

    if let Err(err) = run_chain(&mut report) {
        pad_remaining(&mut report.steps, &err);
    }
    report
}

fn run_chain(report: &mut MvpIntegrationReport) -> Result<(), String> {
    let created = step_create(report);
    let mut live = bind(report, 0, created)?;
    let queried = step_query(&mut live, report);
    bind(report, 1, queried)?;
    let prepared_res = step_prepare(&mut live, report);
    let prepared = bind(report, 2, prepared_res)?;
    let committed = step_commit(&mut live, report, prepared);
    bind(report, 3, committed)?;
    let duplicated = step_duplicate_replay(&mut live, report);
    bind(report, 4, duplicated)?;
    let captured_res = step_capture(&mut live, report);
    let captured = bind(report, 5, captured_res)?;
    let encoded_res = step_encode(captured, report);
    let encoded = bind(report, 6, encoded_res)?;
    let restored = step_restore(&mut live, report, &encoded);
    bind(report, 7, restored)?;
    let acked = step_durability_ack(&mut live, report);
    bind(report, 8, acked)?;
    let closed = step_close(&mut live, report);
    bind(report, 9, closed)?;
    Ok(())
}

struct LiveSlice {
    authority: VoxelWorld,
    replica: VoxelWorld,
    snap: Arc<VoxelConfigSnapshot>,
    replica_identity: [u8; 32],
    mutation: Option<OriginEnvelope<MutationRequest>>,
    original_receipt: Option<GeneratedMutationReceipt>,
}

fn step_create(report: &mut MvpIntegrationReport) -> Result<LiveSlice, String> {
    if !SNAPSHOT_FEATURE {
        return Err("lumio-voxel-ops snapshot feature is off".into());
    }
    let interned_schema = intern_schema("voxel-world-port")?;
    let (authority_role, replica_role) = intern_local_embedded_pair("Authority", "Replica")
        .map_err(|err| format!("intern_local_embedded_pair: {}", err.error_id()))?;
    let snap = approved_snapshot("mvp-authority");
    report.config_hash = snap.config_hash().to_string();
    let mut authority = create_world(
        authority_role,
        "ctx-mvp-auth",
        "world-mvp-auth",
        Arc::clone(&snap),
    )?;
    let mut replica = create_world(
        replica_role,
        "ctx-mvp-repl",
        "world-mvp-repl",
        approved_snapshot("mvp-replica"),
    )?;
    report.commands.push("VoxelWorld::create Authority".into());
    report.commands.push("VoxelWorld::create Replica".into());
    drive_to_running(&mut authority, report)?;
    drive_to_running(&mut replica, report)?;
    if authority.generation_guard().generation() == replica.generation_guard().generation() {
        return Err("worlds share instance generation".into());
    }
    let adapter_schema = {
        let adapter = GeneratedVoxelWorldPortAdapter::new(&mut authority);
        adapter.schema_id()
    };
    if !std::ptr::eq(adapter_schema, interned_schema) {
        return Err("adapter.schema_id is not interned SCHEMA_IDS".into());
    }
    let replica_query = query_envelope(&replica, "q-replica", &["c:0:0:0"])?;
    let replica_items = {
        let mut adapter = GeneratedVoxelWorldPortAdapter::new(&mut replica);
        adapter
            .query(replica_query)
            .map_err(|err| format!("replica query: {}", err.error_id()))?
    };
    report
        .commands
        .push("GeneratedVoxelWorldPortAdapter::query Replica".into());
    if replica_items.payload.items().len() != 1
        || replica_items.payload.items()[0].presence() != "NotLoaded"
    {
        return Err("replica query implicit-loaded an empty directory".into());
    }
    intern_presence(replica_items.payload.items()[0].presence())?;
    let authority_identity = identity_of(&authority);
    let replica_identity = identity_of(&replica);
    if authority_identity == replica_identity {
        return Err("independent worlds published the same identity".into());
    }
    report.authority_identity = authority_identity;
    report.replica_identity = replica_identity;
    Ok(LiveSlice {
        authority,
        replica,
        snap,
        replica_identity,
        mutation: None,
        original_receipt: None,
    })
}

fn step_query(live: &mut LiveSlice, report: &mut MvpIntegrationReport) -> Result<String, String> {
    require_schema(QUERY_SCHEMA)?;
    if CHUNK_PRESENCE != ["Ready", "NotLoaded", "Pending", "Unavailable"] {
        return Err(format!("CHUNK_PRESENCE {CHUNK_PRESENCE:?}"));
    }
    // Adapter mutation cannot insert Pending/Unavailable; this fixture is the B2 helper.
    seed_four_state(&live.authority)?;
    report
        .commands
        .push("seed_four_state PublicationAuthority fixture".into());
    let before = identity_of(&live.authority);
    let envelope = query_envelope(&live.authority, "q-mvp-four", &FOUR_STATE_IDS)?;
    let outcome = {
        let mut adapter = GeneratedVoxelWorldPortAdapter::new(&mut live.authority);
        adapter
            .query(envelope)
            .map_err(|err| format!("adapter query: {}", err.error_id()))?
    };
    report
        .commands
        .push("GeneratedVoxelWorldPortAdapter::query".into());
    if identity_of(&live.authority) != before {
        return Err("query mutated the published identity".into());
    }
    let expected = [
        ("c:0:0:0", "Ready", true),
        ("c:1:0:0", "NotLoaded", false),
        ("c:2:0:0", "Pending", false),
        ("c:3:0:0", "Unavailable", false),
        ("c:4:0:0", "NotLoaded", false),
    ];
    if outcome.payload.items().len() != expected.len() {
        return Err(format!("item count {}", outcome.payload.items().len()));
    }
    for (item, (id, presence, ready)) in outcome.payload.items().iter().zip(expected) {
        intern_presence(item.presence())?;
        if item.chunk_id() != id || item.presence() != presence {
            return Err(format!(
                "{} mapped to {} not {presence}",
                item.chunk_id(),
                item.presence()
            ));
        }
        if (item.presence() == "Ready") != ready {
            return Err(format!("{} ready flag", item.chunk_id()));
        }
        if ready {
            let schema = item
                .schema_id()
                .ok_or_else(|| "Ready missing schema_id".to_string())?;
            require_schema(schema)?;
        } else if item.schema_id().is_some() {
            return Err(format!("{} leaked schema_id", item.chunk_id()));
        }
    }
    live.authority_refresh(report);
    Ok("four-state query via adapter; absent id NotLoaded; no implicit load".into())
}

fn step_prepare(
    live: &mut LiveSlice,
    report: &mut MvpIntegrationReport,
) -> Result<OriginEnvelope<PreparedMutation>, String> {
    let before = identity_of(&live.authority);
    let envelope = mutation_envelope(
        &live.authority,
        "txn-mvp",
        &[("c:0:0:0/cell-0", "mvp-edit")],
    )?;
    live.mutation = Some(envelope.clone());
    let prepared = {
        let mut adapter = GeneratedVoxelWorldPortAdapter::new(&mut live.authority);
        adapter
            .prepare_mutation(envelope)
            .map_err(|err| format!("adapter prepare: {}", err.error_id()))?
    };
    report
        .commands
        .push("GeneratedVoxelWorldPortAdapter::prepare_mutation".into());
    if identity_of(&live.authority) != before {
        return Err("prepare published".into());
    }
    if prepared.payload.txn_id() != "txn-mvp" {
        return Err(format!("prepared txn {}", prepared.payload.txn_id()));
    }
    Ok(prepared)
}

fn step_commit(
    live: &mut LiveSlice,
    report: &mut MvpIntegrationReport,
    prepared: OriginEnvelope<PreparedMutation>,
) -> Result<String, String> {
    let before = identity_of(&live.authority);
    let receipt = {
        let mut adapter = GeneratedVoxelWorldPortAdapter::new(&mut live.authority);
        adapter
            .commit(prepared)
            .map_err(|err| format!("adapter commit: {}", err.error_id()))?
    };
    report
        .commands
        .push("GeneratedVoxelWorldPortAdapter::commit".into());
    push_receipt(report, &receipt.payload);
    live.original_receipt = Some(receipt.payload.clone());
    let after = identity_of(&live.authority);
    if receipt.payload.txn_id != "txn-mvp" || receipt.payload.evidence.txn_id != "txn-mvp" {
        return Err("commit txn mismatch".into());
    }
    if receipt.payload.evidence.old_root != before
        || receipt.payload.evidence.new_root == before
        || after != receipt.payload.evidence.new_root
    {
        return Err("commit did not publish a new identity".into());
    }
    live.authority_refresh(report);
    Ok("commit swapped a complete new identity".into())
}

fn step_duplicate_replay(
    live: &mut LiveSlice,
    report: &mut MvpIntegrationReport,
) -> Result<String, String> {
    let Some(dup_env) = live.mutation.clone() else {
        return Err("duplicate replay missing original MutationRequest".into());
    };
    let original = live
        .original_receipt
        .clone()
        .ok_or_else(|| "duplicate replay missing original receipt".to_string())?;
    let before = identity_of(&live.authority);
    let prepared = {
        let mut adapter = GeneratedVoxelWorldPortAdapter::new(&mut live.authority);
        adapter
            .prepare_mutation(dup_env)
            .map_err(|err| format!("duplicate prepare: {}", err.error_id()))?
    };
    report
        .commands
        .push("GeneratedVoxelWorldPortAdapter::prepare_mutation duplicate".into());
    let replayed = {
        let mut adapter = GeneratedVoxelWorldPortAdapter::new(&mut live.authority);
        adapter
            .commit(prepared)
            .map_err(|err| format!("duplicate commit: {}", err.error_id()))?
    };
    report
        .commands
        .push("GeneratedVoxelWorldPortAdapter::commit duplicate TxnId".into());
    if replayed.payload.txn_id != original.txn_id
        || replayed.payload.evidence.txn_id != original.evidence.txn_id
    {
        return Err("duplicate commit txn mismatch".into());
    }
    if replayed.payload.receipt != original.receipt
        || replayed.payload.evidence.receipt_hash != original.evidence.receipt_hash
    {
        return Err("duplicate commit returned a different receipt".into());
    }
    if identity_of(&live.authority) != before {
        return Err("duplicate commit published a second root".into());
    }
    Ok("adapter.commit of the same TxnId returned the original receipt; identity unchanged".into())
}

fn step_capture(
    live: &mut LiveSlice,
    report: &mut MvpIntegrationReport,
) -> Result<(VoxelCaptureRef, String), String> {
    let id_before = identity_of(&live.authority);
    let cut = RuntimeSnapshotCut::from_live(&live.authority, "cut-mvp");
    let (captured, evidence) = {
        let mut adapter = GeneratedVoxelWorldPortAdapter::new(&mut live.authority);
        adapter
            .capture(&cut)
            .map_err(|err| format!("adapter capture: {}", err.error_id()))?
    };
    report
        .commands
        .push("GeneratedVoxelWorldPortAdapter::capture".into());
    if !evidence.barrier_released {
        return Err("adapter capture held CaptureCut".into());
    }
    if captured.root_identity() != id_before || captured.root_identity() != evidence.root_hash {
        return Err("adapter capture evidence hash mismatch".into());
    }
    report.snapshot_hash = captured.root_identity();
    if identity_of(&live.authority) != id_before {
        return Err("capture mutated the live identity".into());
    }
    Ok((
        captured,
        "capture released CaptureCut and pinned the live identity".into(),
    ))
}

fn step_encode(
    captured: VoxelCaptureRef,
    report: &mut MvpIntegrationReport,
) -> Result<(Vec<u8>, String), String> {
    let mut writer = MemoryCaptureWriter::new(8192);
    let meta = encode_capture(&captured, &mut writer)
        .map_err(|err| format!("encode_capture: {}", err.error_id()))?;
    report
        .commands
        .push("encode_capture outside CaptureCut".into());
    if meta.root_identity() != captured.root_identity() {
        return Err("encode root_identity diverged".into());
    }
    let bytes = writer.as_slice().to_vec();
    drop(captured);
    if bytes.is_empty() {
        return Err("encode_capture wrote zero bytes".into());
    }
    Ok((
        bytes,
        format!(
            "encode_capture wrote {} bytes outside the barrier",
            meta.byte_len()
        ),
    ))
}

fn step_restore(
    live: &mut LiveSlice,
    report: &mut MvpIntegrationReport,
    bytes: &[u8],
) -> Result<String, String> {
    let before = identity_of(&live.authority);
    let decoded = RestorePreflight::validate(
        bytes,
        live.authority.state_view().world_id(),
        live.authority.state_view().instance_generation(),
        live.snap.as_ref(),
    )
    .map_err(|err| format!("preflight: {}", err.error_id()))?;
    report.commands.push("RestorePreflight::validate".into());
    let candidate = RestoreShadowBuilder::build(&decoded)
        .map_err(|err| format!("shadow: {}", err.error_id()))?;
    report.commands.push("RestoreShadowBuilder::build".into());
    if !candidate.hash_matches() {
        return Err("shadow hash mismatch".into());
    }
    let receipt = {
        let mut adapter = GeneratedVoxelWorldPortAdapter::new(&mut live.authority);
        adapter
            .restore(candidate)
            .map_err(|err| format!("adapter restore: {}", err.error_id()))?
    };
    report
        .commands
        .push("GeneratedVoxelWorldPortAdapter::restore".into());
    if receipt.old_root() != before || receipt.new_root() == before {
        return Err("restore did not swap identity".into());
    }
    if identity_of(&live.authority) != receipt.new_root() {
        return Err("live identity is not the restored root".into());
    }
    report.restored_hash = receipt.new_root();
    live.authority_refresh(report);
    Ok("restore swapped a new identity (NotLoaded rematerialize)".into())
}

fn step_durability_ack(
    live: &mut LiveSlice,
    report: &mut MvpIntegrationReport,
) -> Result<String, String> {
    require_schema("voxel-durability-ack")?;
    // RestoreShadowBuilder publishes an empty dirty frontier; re-seed Ready then mutate.
    seed_ready(&live.authority, &["c:0:0:0"])?;
    report
        .commands
        .push("seed_ready after restore for DurabilityAck dirty".into());
    let dirty_env = mutation_envelope(
        &live.authority,
        "txn-mvp-ack",
        &[("c:0:0:0/cell-1", "mvp-ack-edit")],
    )?;
    let dirty_receipt = {
        let mut adapter = GeneratedVoxelWorldPortAdapter::new(&mut live.authority);
        let prepared = adapter
            .prepare_mutation(dirty_env)
            .map_err(|err| format!("ack prepare: {}", err.error_id()))?;
        adapter
            .commit(prepared)
            .map_err(|err| format!("ack commit: {}", err.error_id()))?
    };
    report
        .commands
        .push("GeneratedVoxelWorldPortAdapter::prepare_mutation/commit (ack dirty)".into());
    push_receipt(report, &dirty_receipt.payload);
    let latest =
        latest_dirty(&live.authority, "c:0:0:0")?.ok_or_else(|| "chunk not dirty".to_string())?;
    if latest == 0 {
        return Err("no older revision available for a stale ack".into());
    }
    let before = identity_of(&live.authority);
    let old: AckEvidence = ack_for(&live.authority, &[("c:0:0:0", latest - 1)]);
    let old_receipt = {
        let mut adapter = GeneratedVoxelWorldPortAdapter::new(&mut live.authority);
        adapter
            .apply_durability_ack(old)
            .map_err(|err| format!("old ack: {}", err.error_id()))?
    };
    report
        .commands
        .push("GeneratedVoxelWorldPortAdapter::apply_durability_ack old".into());
    if old_receipt.coverage_len() != 0
        || old_receipt.old_root() != before
        || old_receipt.new_root() != before
        || identity_of(&live.authority) != before
        || latest_dirty(&live.authority, "c:0:0:0")? != Some(latest)
    {
        return Err("old ack cleared newer dirty".into());
    }

    let covering: AckEvidence = ack_for(&live.authority, &[("c:0:0:0", latest)]);
    let receipt = {
        let mut adapter = GeneratedVoxelWorldPortAdapter::new(&mut live.authority);
        adapter
            .apply_durability_ack(covering)
            .map_err(|err| format!("covering ack: {}", err.error_id()))?
    };
    report
        .commands
        .push("GeneratedVoxelWorldPortAdapter::apply_durability_ack covering".into());
    if receipt.coverage_len() != 1 || receipt.old_root() != before || receipt.new_root() == before {
        return Err("covering ack did not publish a new identity".into());
    }
    if identity_of(&live.authority) != receipt.new_root()
        || latest_dirty(&live.authority, "c:0:0:0")?.is_some()
    {
        return Err("covering ack left dirty in place".into());
    }
    live.authority_refresh(report);
    Ok("old ack no-op; covering DurabilityAck clears latest".into())
}

fn step_close(live: &mut LiveSlice, report: &mut MvpIntegrationReport) -> Result<String, String> {
    let stale_query = query_envelope(&live.authority, "q-stale-close", &["c:0:0:0"])?;
    let before_gen = live.authority.generation_guard().generation();
    let before_id = identity_of(&live.authority);
    {
        let mut adapter = GeneratedVoxelWorldPortAdapter::new(&mut live.authority);
        let mut sink = WorldEventSink::bounded(8);
        adapter
            .shutdown(&mut sink)
            .map_err(|err| format!("adapter shutdown: {}", err.error_id()))?;
    }
    report
        .commands
        .push("GeneratedVoxelWorldPortAdapter::shutdown".into());
    if live.authority.state_view().lifecycle() != "Disposed" {
        return Err(format!(
            "shutdown lifecycle {}",
            live.authority.state_view().lifecycle()
        ));
    }
    if live.authority.generation_guard().generation() == before_gen {
        return Err("shutdown did not advance instance generation".into());
    }
    if identity_of(&live.authority) != before_id {
        return Err("shutdown mutated capture identity".into());
    }
    let err = {
        let mut adapter = GeneratedVoxelWorldPortAdapter::new(&mut live.authority);
        adapter
            .query(stale_query)
            .err()
            .ok_or_else(|| "stale origin query succeeded after shutdown".to_string())?
    };
    report
        .commands
        .push("GeneratedVoxelWorldPortAdapter::query stale after shutdown".into());
    require_stable(err.error_id())?;
    if err.error_id() != "StaleEpoch" {
        return Err(format!("stale origin {}", err.error_id()));
    }
    if identity_of(&live.replica) != live.replica_identity {
        return Err("replica capture changed during authority chain".into());
    }
    live.authority_refresh(report);
    report.replica_identity = identity_of(&live.replica);
    if report.authority_identity == report.replica_identity {
        return Err("dual-instance identities collided at close".into());
    }
    Ok("shutdown then stale origin StaleEpoch; replica identity unchanged".into())
}

impl LiveSlice {
    fn authority_refresh(&self, report: &mut MvpIntegrationReport) {
        report.authority_identity = identity_of(&self.authority);
    }
}

trait BindStep {
    type Value;
    fn into_bind(self) -> (Self::Value, String);
}

impl BindStep for LiveSlice {
    type Value = Self;
    fn into_bind(self) -> (Self::Value, String) {
        (
            self,
            "Authority+Replica create; drive_to_running via adapter.admit; identities differ"
                .into(),
        )
    }
}

impl BindStep for String {
    type Value = ();
    fn into_bind(self) -> (Self::Value, String) {
        ((), self)
    }
}

impl BindStep for OriginEnvelope<PreparedMutation> {
    type Value = Self;
    fn into_bind(self) -> (Self::Value, String) {
        (self, "prepare reserved InFlight and did not publish".into())
    }
}

impl<T> BindStep for (T, String) {
    type Value = T;
    fn into_bind(self) -> (Self::Value, String) {
        self
    }
}

fn bind<B: BindStep>(
    report: &mut MvpIntegrationReport,
    index: usize,
    result: Result<B, String>,
) -> Result<B::Value, String> {
    let (id, name) = STEPS[index];
    match result {
        Ok(bound) => {
            let (value, detail) = bound.into_bind();
            report.steps.push(MvpStepResult {
                id,
                name,
                ok: true,
                detail,
            });
            Ok(value)
        }
        Err(detail) => {
            report.steps.push(MvpStepResult {
                id,
                name,
                ok: false,
                detail: detail.clone(),
            });
            Err(detail)
        }
    }
}

fn pad_remaining(steps: &mut Vec<MvpStepResult>, err: &str) {
    for (id, name) in STEPS.iter().skip(steps.len()) {
        steps.push(MvpStepResult {
            id,
            name,
            ok: false,
            detail: format!("not reached: {err}"),
        });
    }
}

fn push_receipt(report: &mut MvpIntegrationReport, receipt: &GeneratedMutationReceipt) {
    report.receipts.push(MvpReceipt {
        txn_id: receipt.txn_id.clone(),
        old_root: receipt.evidence.old_root,
        new_root: receipt.evidence.new_root,
        receipt_hash: receipt.evidence.receipt_hash,
    });
}

fn reference_trace() -> Result<[u8; 32], String> {
    let ops = mvp_corpus()?;
    let vec_fold = DeterministicExecutor::vec_fold_payloads(&ops);
    let map_fold = DeterministicExecutor::hashmap_fold_payloads(&ops);
    if vec_fold == map_fold {
        return Err("hashmap fold matched vec fold".into());
    }
    let mut hashes = Vec::new();
    for seed in [SEED_A, SEED_B] {
        let schedule = Schedule {
            seed,
            ops: ops.clone(),
        };
        let a = DeterministicExecutor::run(&schedule);
        let b = DeterministicExecutor::run(&schedule);
        if a != b || a.snapshot != b.snapshot {
            return Err(format!("seed {seed:#x} replay diverged"));
        }
        hashes.push(a.snapshot);
    }
    if hashes[0] != hashes[1] {
        return Err("same corpus produced different snapshot hashes across seeds".into());
    }
    Ok(hashes[0])
}

fn mvp_corpus() -> Result<Vec<GeneratedVoxelOperation>, String> {
    let ids = [
        ("voxel-query", 0u64, b"query-four-state".as_slice()),
        ("voxel-mutation-receipt", 1, b"prepare-commit"),
        ("voxel-mutation-receipt", 2, b"duplicate-replay"),
        ("voxel-snapshot-payload", 3, b"capture-encode-restore"),
        ("voxel-durability-ack", 4, b"durability-ack"),
        ("voxel-world-port", 5, b"close"),
    ];
    let mut ops = Vec::with_capacity(ids.len());
    for (schema, seq, payload) in ids {
        ops.push(GeneratedVoxelOperation {
            schema_id: intern_schema(schema)?,
            seq,
            payload: payload.to_vec(),
        });
    }
    Ok(ops)
}

fn approved_snapshot(label: &str) -> Arc<VoxelConfigSnapshot> {
    let source = GateSourceHashes {
        architecture_baseline_id: BASELINE_ID.to_string(),
        voxel_head: "b2f0d8a3763a02f805e29cbd101560ba7fdca77b".to_string(),
        architecture_mirror_sha256:
            "f1d36acf33a1f5e8326a9e58d609fcf7d9fa85177f9b5b60bb3f4742c1afebd0".to_string(),
        v13_decision_gates_sha256:
            "4850057dd8926c11c8c3beebe109d18dffdb7e84cd451426d7d635860be5ede2".to_string(),
        blueprint_sha256: "32e76066eb298aad20f4149760abbeddacb6d6c43e096945f1cf0ea75b2471aa"
            .to_string(),
    };
    let digests: BTreeMap<String, String> = P0_DECISION_GATES
        .iter()
        .map(|gate| {
            (
                (*gate).to_string(),
                hex32(&sha256(format!("approved-{gate}").as_bytes())),
            )
        })
        .collect();
    let evidence: Vec<DecisionEvidence> = P0_DECISION_GATES
        .iter()
        .map(|gate| DecisionEvidence {
            gate_id: (*gate).to_string(),
            approval_status: "approved".to_string(),
            source_hashes: source.clone(),
            evidence_digest: digests[*gate].clone(),
        })
        .collect();
    let cfg = GeneratedVoxelConfig {
        schema_id: "config-table",
        host_capability_schema_id: "host-capability",
        schema_epoch: SCHEMA_EPOCH,
        config_hash: hex32(&sha256(label.as_bytes())),
        gate_source_hashes: digests,
        host_capability: GeneratedHostCapability::from_names(["Native", "ReferenceVoxel"]),
        start_capabilities: vec!["Native".into(), "ReferenceVoxel".into()],
        key_material: None,
    };
    VoxelConfigSnapshot::from_generated(&cfg, &evidence).expect("approved P0 snapshot")
}

fn create_world(
    role: &str,
    context: &str,
    world_id: &str,
    snap: Arc<VoxelConfigSnapshot>,
) -> Result<VoxelWorld, String> {
    VoxelWorld::create(
        WorldDescriptor {
            role: role.to_string(),
            world_context_id: context.to_string(),
            capabilities: vec!["Native".into(), "ReferenceVoxel".into()],
            config: WorldConfigAdapter {
                world_id: world_id.to_string(),
            },
        },
        snap,
    )
    .map_err(|err| format!("VoxelWorld::create {role}: {}", err.error_id()))
}

fn apply_phase() -> Result<&'static str, String> {
    APPLY_PHASES
        .iter()
        .copied()
        .find(|phase| *phase == "VoxelCommit")
        .ok_or_else(|| "VoxelCommit missing from APPLY_PHASES".to_string())
}

fn origin_of(world: &VoxelWorld, request_id: &str) -> Result<OriginToken, String> {
    let guard = world.generation_guard();
    OriginToken::try_new(
        guard.world_context_id(),
        guard.generation(),
        request_id,
        0,
        BTreeMap::new(),
        apply_phase()?,
    )
    .map_err(|err| format!("OriginToken: {}", err.error_id()))
}

fn lifecycle_cmd(
    world: &VoxelWorld,
    event: &'static str,
    to: &'static str,
) -> Result<WorldCommand, String> {
    Ok(WorldCommand::Lifecycle {
        event,
        to,
        origin: origin_of(world, event)?,
    })
}

fn drive_to_running(
    world: &mut VoxelWorld,
    report: &mut MvpIntegrationReport,
) -> Result<(), String> {
    for (event, to) in [
        ("Initialize", "Initialized"),
        ("Prime", "Ready"),
        ("Start", "Running"),
    ] {
        let cmd = lifecycle_cmd(world, event, to)?;
        GeneratedVoxelWorldPortAdapter::new(world)
            .admit(cmd)
            .map_err(|err| format!("{event}->{to}: {}", err.error_id()))?;
        report.commands.push(format!(
            "GeneratedVoxelWorldPortAdapter::admit {event}->{to}"
        ));
        if world.state_view().lifecycle() != to {
            return Err(format!(
                "{event} left lifecycle {}",
                world.state_view().lifecycle()
            ));
        }
    }
    Ok(())
}

fn query_envelope(
    world: &VoxelWorld,
    query_id: &str,
    chunks: &[&str],
) -> Result<OriginEnvelope<GeneratedVoxelQueryRequest>, String> {
    let view = world.state_view();
    Ok(OriginEnvelope {
        origin: origin_of(world, query_id)?,
        config_hash: String::new(),
        payload: GeneratedVoxelQueryRequest {
            query_id: query_id.to_string(),
            world_id: view.world_id().to_string(),
            context: view.world_context_id().to_string(),
            chunk_ids: chunks.iter().map(|chunk| (*chunk).to_string()).collect(),
            cancel: false,
        },
    })
}

fn mutation_envelope(
    world: &VoxelWorld,
    txn_id: &str,
    extra: &[(&str, &str)],
) -> Result<OriginEnvelope<MutationRequest>, String> {
    let view = world.state_view();
    let world_revision = world
        .publication_authority()
        .capture()
        .stamp()
        .world_revision;
    let mut fields = BTreeMap::new();
    fields.insert("world_revision".to_string(), world_revision.to_string());
    for (k, v) in extra {
        fields.insert((*k).to_string(), (*v).to_string());
    }
    Ok(OriginEnvelope {
        origin: origin_of(world, txn_id)?,
        config_hash: String::new(),
        payload: MutationRequest {
            txn_id: txn_id.to_string(),
            world_id: view.world_id().to_string(),
            generation: view.instance_generation(),
            fields,
        },
    })
}

fn world_rev(n: u64) -> WorldRevision {
    let mut alloc = RevisionAllocator::new();
    for _ in 0..n {
        alloc.reserve_world().unwrap().abandon();
    }
    let mut reserved = alloc.reserve_world().unwrap();
    reserved.finalize().unwrap()
}

fn payload(bytes: &[u8]) -> ChunkPayload {
    ChunkPayload::from_pages([ChunkPage::new(
        "Dense",
        "None",
        bytes.to_vec(),
        sha256(bytes),
    )])
    .expect("valid dense uncompressed page")
}

fn empty_replacement(base: &ChunkDirectoryRoot) -> ChunkReplacement {
    ChunkDeltaBuilder::new(base)
        .freeze()
        .expect("empty replacement")
}

fn seed_four_state(world: &VoxelWorld) -> Result<(), String> {
    let view = world.state_view();
    let before = world.publication_authority().capture();
    let next = before.stamp().world_revision + 1;
    let mut builder = ChunkDirectoryBuilder::new();
    builder
        .insert("c:0:0:0", ChunkSlot::ready(payload(b"mvp-ready")))
        .map_err(|err| format!("insert Ready: {}", err.error_id()))?;
    builder
        .insert("c:1:0:0", ChunkSlot::not_loaded())
        .map_err(|err| format!("insert NotLoaded: {}", err.error_id()))?;
    builder
        .insert("c:2:0:0", ChunkSlot::pending())
        .map_err(|err| format!("insert Pending: {}", err.error_id()))?;
    builder
        .insert("c:3:0:0", ChunkSlot::unavailable())
        .map_err(|err| format!("insert Unavailable: {}", err.error_id()))?;
    let mut chunk_revision_set = BTreeMap::new();
    for id in ["c:0:0:0", "c:1:0:0", "c:2:0:0", "c:3:0:0"] {
        chunk_revision_set.insert(id.to_string(), next);
    }
    let stamp = GeneratedRevisionStamp {
        schema_id: REVISION_STAMP_SCHEMA,
        world_id: view.world_id().to_string(),
        context_id: view.world_context_id().to_string(),
        generation: view.instance_generation(),
        world_revision: next,
        chunk_revision_set,
    };
    let dirty = DirtyFrontier::new(view.world_id(), view.instance_generation())
        .map_err(|err| err.error_id().to_string())?;
    let later = PublishedStateRoot::new(stamp, builder.freeze(), dirty);
    let mut prepared = world
        .publication_authority()
        .prepare(
            world_rev(next),
            later,
            empty_replacement(before.directory()),
        )
        .map_err(|err| format!("four-state prepare: {}", err.error_id()))?;
    world
        .publication_authority()
        .publish_once(
            prepared
                .seal()
                .map_err(|err| format!("four-state seal: {}", err.error_id()))?,
        )
        .map_err(|err| format!("four-state publish: {}", err.error_id()))?;
    Ok(())
}

fn seed_ready(world: &VoxelWorld, chunks: &[&str]) -> Result<(), String> {
    let view = world.state_view();
    let before = world.publication_authority().capture();
    let next = before.stamp().world_revision + 1;
    let mut builder = ChunkDirectoryBuilder::new();
    let mut chunk_revision_set = BTreeMap::new();
    for id in chunks {
        builder
            .insert(id, ChunkSlot::ready(payload(id.as_bytes())))
            .map_err(|err| format!("seed {id}: {}", err.error_id()))?;
        chunk_revision_set.insert((*id).to_string(), next);
    }
    let stamp = GeneratedRevisionStamp {
        schema_id: REVISION_STAMP_SCHEMA,
        world_id: view.world_id().to_string(),
        context_id: view.world_context_id().to_string(),
        generation: view.instance_generation(),
        world_revision: next,
        chunk_revision_set,
    };
    let dirty = DirtyFrontier::new(view.world_id(), view.instance_generation())
        .map_err(|err| err.error_id().to_string())?;
    let later = PublishedStateRoot::new(stamp, builder.freeze(), dirty);
    let mut prepared = world
        .publication_authority()
        .prepare(
            world_rev(next),
            later,
            empty_replacement(before.directory()),
        )
        .map_err(|err| format!("seed prepare: {}", err.error_id()))?;
    world
        .publication_authority()
        .publish_once(
            prepared
                .seal()
                .map_err(|err| format!("seed seal: {}", err.error_id()))?,
        )
        .map_err(|err| format!("seed publish: {}", err.error_id()))?;
    Ok(())
}

fn ack_for(world: &VoxelWorld, chunks: &[(&str, u64)]) -> AckEvidence {
    let view = world.state_view();
    AckEvidence {
        kind: "DurabilityAck".to_string(),
        world_id: view.world_id().to_string(),
        context: DurabilityAckContext {
            context_id: view.world_context_id().to_string(),
            generation: view.instance_generation(),
        },
        covered_world_revision: world
            .publication_authority()
            .capture()
            .stamp()
            .world_revision,
        covered_chunks: chunks
            .iter()
            .map(|(id, rev)| CoveredChunkAck {
                chunk_id: (*id).to_string(),
                up_to_chunk_revision: *rev,
            })
            .collect(),
    }
}

fn latest_dirty(world: &VoxelWorld, chunk_id: &str) -> Result<Option<u64>, String> {
    world
        .publication_authority()
        .capture()
        .dirty_frontier()
        .latest_revision(chunk_id)
        .map_err(|err| err.error_id().to_string())
}

fn identity_of(world: &VoxelWorld) -> [u8; 32] {
    world.publication_authority().capture().root().identity()
}

fn intern_schema(id: &str) -> Result<&'static str, String> {
    SCHEMA_IDS
        .iter()
        .copied()
        .find(|item| *item == id)
        .ok_or_else(|| format!("{id} missing from SCHEMA_IDS"))
}

fn intern_presence(name: &str) -> Result<&'static str, String> {
    CHUNK_PRESENCE
        .iter()
        .copied()
        .find(|item| *item == name)
        .ok_or_else(|| format!("{name} missing from CHUNK_PRESENCE"))
}

fn require_schema(id: &str) -> Result<(), String> {
    intern_schema(id).map(|_| ())
}

fn require_stable(id: &str) -> Result<(), String> {
    if STABLE_ERROR_IDS.contains(&id) {
        Ok(())
    } else {
        Err(format!("{id} is not in STABLE_ERROR_IDS"))
    }
}

fn hex32(bytes: &[u8; 32]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(64);
    for b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0xf) as usize] as char);
    }
    out
}

fn git_head() -> String {
    let root = workspace_root_from_manifest(env!("CARGO_MANIFEST_DIR"));
    std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(root)
        .output()
        .ok()
        .and_then(|out| String::from_utf8(out.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".to_string())
}
