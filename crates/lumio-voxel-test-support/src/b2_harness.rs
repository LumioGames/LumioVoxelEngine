//! B2 matrix: drive shipped Query / Mutation / World / Restore / Port / Fault APIs.

#![forbid(unsafe_code)]

use crate::fault_injection::{FaultInjector, FaultPoint};
use crate::workspace_root_from_manifest;
use lumio_voxel_contracts::voxel_world::SECTION_PRESENCE;
use lumio_voxel_contracts::{BASELINE_ID, SCHEMA_EPOCH, SCHEMA_IDS, is_stable_error_id, sha256};
use lumio_voxel_domain::config_snapshot::{
    DecisionEvidence, GateSourceHashes, GeneratedHostCapability, GeneratedVoxelConfig,
    P0_DECISION_GATES, VoxelConfigSnapshot,
};
use lumio_voxel_domain::publication::{
    PublicationAuthority, PublishedReadView, PublishedStateRoot,
};
use lumio_voxel_domain::revision::{
    GeneratedRevisionStamp, PinRegistry, REVISION_STAMP_SCHEMA, RevisionAllocator, WorldRevision,
    to_generated_stamp,
};
use lumio_voxel_domain::section::{
    CoveredSectionAck, DirtyFrontier, DurabilityAckContext, DurabilityAckEvidence,
    SectionDeltaBuilder, SectionDirectoryBuilder, SectionDirectoryRoot, SectionPage,
    SectionPayload, SectionReplacement, SectionSlot,
};
use lumio_voxel_ops::SNAPSHOT_FEATURE;
use lumio_voxel_ops::async_support::{OriginEnvelope, OriginToken};
use lumio_voxel_ops::mutation::{
    LookupOutcome, MutationRequest, ReceiptLedger, ReplayDisposition, canonical_fingerprint,
    commit, prepare,
};
use lumio_voxel_ops::query::{
    GeneratedVoxelQueryRequest, QUERY_SCHEMA, QueryExecutor, QueryPlanner,
};
use lumio_voxel_ops::snapshot::{
    MemoryCaptureWriter, RestorePreflight, RestoreShadowBuilder, encode_capture,
};
use lumio_voxel_world::port::GeneratedVoxelWorldPortAdapter;
use lumio_voxel_world::world::{
    AdmittedCommand, BarrierScope, FaultEvidence, ForbiddenWork, RuntimeSnapshotCut, VoxelWorld,
    WorldCommand, WorldConfigAdapter, WorldDescriptor, WorldError, WorldEventSink, WorldFaultPort,
    WorldShutdown, WorldWriteLane, apply_durability_ack, capture, intern_local_embedded_pair,
    reject_forbidden, restore,
};
use std::collections::BTreeMap;
use std::sync::Arc;

pub const MATRIX_ROWS: usize = 12;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct B2CaseResult {
    pub id: &'static str,
    pub name: &'static str,
    pub ok: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct B2VerificationReport {
    pub baseline: &'static str,
    pub commit: String,
    pub cases: Vec<B2CaseResult>,
}

impl B2VerificationReport {
    pub fn all_ok(&self) -> bool {
        self.cases.len() == MATRIX_ROWS && self.cases.iter().all(|case| case.ok)
    }
}

pub fn run_b2_matrix() -> B2VerificationReport {
    let cases = vec![
        case_query_single_cut_plan_hash(),
        case_query_four_state(),
        case_query_cancel_budget(),
        case_prepare_wrong_world(),
        case_prepare_does_not_publish(),
        case_commit_atomic_duplicate(),
        case_dual_world_fault_isolation(),
        case_capture_encode_outside_barrier(),
        case_restore_preflight_and_swap(),
        case_durability_ack_covers_latest(),
        case_port_adapter_routes(),
        case_fault_injector_recoverable(),
    ];
    B2VerificationReport {
        baseline: BASELINE_ID,
        commit: git_head(),
        cases,
    }
}

pub fn case_query_single_cut_plan_hash() -> B2CaseResult {
    wrap(
        "1",
        "query single-cut + permutation-independent plan_hash",
        query_single_cut_plan_hash,
    )
}

pub fn case_query_four_state() -> B2CaseResult {
    wrap(
        "2",
        "query four-state / Unchanged mapping",
        query_four_state,
    )
}

pub fn case_query_cancel_budget() -> B2CaseResult {
    wrap("3", "query cancel / budget", query_cancel_budget)
}

pub fn case_prepare_wrong_world() -> B2CaseResult {
    wrap(
        "4",
        "prepare failure (wrong world) leaves ledger vacant",
        prepare_wrong_world,
    )
}

pub fn case_prepare_does_not_publish() -> B2CaseResult {
    wrap(
        "5",
        "prepare success does not publish",
        prepare_does_not_publish,
    )
}

pub fn case_commit_atomic_duplicate() -> B2CaseResult {
    wrap(
        "6",
        "commit atomic old-or-new + duplicate receipt",
        commit_atomic_duplicate,
    )
}

pub fn case_dual_world_fault_isolation() -> B2CaseResult {
    wrap(
        "7",
        "dual VoxelWorld isolation + target fault on A",
        dual_world_fault_isolation,
    )
}

pub fn case_capture_encode_outside_barrier() -> B2CaseResult {
    wrap(
        "8",
        "CaptureCut then encode outside barrier",
        capture_encode_outside_barrier,
    )
}

pub fn case_restore_preflight_and_swap() -> B2CaseResult {
    wrap(
        "9",
        "restore preflight reject truncated; success swap",
        restore_preflight_and_swap,
    )
}

pub fn case_durability_ack_covers_latest() -> B2CaseResult {
    wrap(
        "10",
        "DurabilityAck covers latest; old ack no-op",
        durability_ack_covers_latest,
    )
}

pub fn case_port_adapter_routes() -> B2CaseResult {
    wrap(
        "11",
        "port adapter query/mutate/capture routes",
        port_adapter_routes,
    )
}

pub fn case_fault_injector_recoverable() -> B2CaseResult {
    wrap(
        "12",
        "FaultInjector PrePublication recoverable vs PostPublication not",
        fault_injector_recoverable,
    )
}

fn wrap(id: &'static str, name: &'static str, f: fn() -> Result<String, String>) -> B2CaseResult {
    match f() {
        Ok(detail) => B2CaseResult {
            id,
            name,
            ok: true,
            detail,
        },
        Err(detail) => B2CaseResult {
            id,
            name,
            ok: false,
            detail,
        },
    }
}

fn query_single_cut_plan_hash() -> Result<String, String> {
    require_schema(QUERY_SCHEMA)?;
    let snap = approved_snapshot("b2-query-cut");
    let planner = QueryPlanner::from_approved_snapshot(Arc::clone(&snap), 8)
        .map_err(|err| format!("from_approved_snapshot: {}", err.error_id()))?;
    let auth = authority(
        "b2-query-cut-view",
        "world-a",
        "ctx-1",
        1,
        dummy_root("world-a", "ctx-1", 1, 0, b"cut-a"),
    )?;
    let view_a = auth.capture();
    let hash0 = view_a.root().identity();
    let plan_a = planner
        .plan(
            &query_request(&["s:1:0:0", "s:0:0:0", "s:-1:0:0"], false),
            &view_a,
            snap.as_ref(),
        )
        .map_err(|err| format!("plan a: {}", err.error_id()))?;
    let plan_b = planner
        .plan(
            &query_request(&["s:-1:0:0", "s:1:0:0", "s:0:0:0"], false),
            &view_a,
            snap.as_ref(),
        )
        .map_err(|err| format!("plan b: {}", err.error_id()))?;
    if plan_a.plan_hash() != plan_b.plan_hash() {
        return Err("permutation changed plan_hash".into());
    }
    if plan_a.read_stamp() != view_a.stamp() || plan_a.read_stamp() != plan_b.read_stamp() {
        return Err("plan did not bind the captured stamp".into());
    }
    if plan_a.canonical_sections()
        != [
            "s:-1:0:0".to_string(),
            "s:0:0:0".to_string(),
            "s:1:0:0".to_string(),
        ]
    {
        return Err(format!("canonical order {:?}", plan_a.canonical_sections()));
    }

    let outcome = QueryExecutor::execute(&plan_a, &view_a)
        .map_err(|err| format!("execute cut A: {}", err.error_id()))?;
    if outcome.evidence().plan_hash() != plan_a.plan_hash() {
        return Err("execute plan_hash diverged".into());
    }
    if outcome.evidence().read_stamp() != view_a.stamp() {
        return Err("execute mixed a later stamp".into());
    }

    let later = dummy_root("world-a", "ctx-1", 1, 1, b"cut-b");
    let mut prepared = auth
        .prepare(world_rev(1), later, empty_replacement(view_a.directory()))
        .map_err(|err| format!("prepare later: {}", err.error_id()))?;
    let view_b = auth
        .publish_once(
            prepared
                .seal()
                .map_err(|err| format!("seal: {}", err.error_id()))?,
        )
        .map_err(|err| format!("publish later: {}", err.error_id()))?;
    if view_b.stamp() == plan_a.read_stamp() {
        return Err("later cut kept the planned stamp".into());
    }
    let mismatch = QueryExecutor::execute(&plan_a, &view_b)
        .err()
        .ok_or_else(|| "stamp mismatch execute succeeded".to_string())?;
    require_stable(mismatch.error_id())?;
    if mismatch.error_id() != "InvalidHandle" {
        return Err(format!("stamp mismatch {}", mismatch.error_id()));
    }
    if view_a.root().identity() != hash0 {
        return Err("old cut identity mixed after publish".into());
    }
    Ok("permutation-independent plan_hash; execute binds one cut; mismatch InvalidHandle".into())
}

fn query_four_state() -> Result<String, String> {
    if SECTION_PRESENCE != ["Ready", "Unchanged", "Pending", "Unavailable"] {
        return Err(format!("SECTION_PRESENCE {SECTION_PRESENCE:?}"));
    }
    let snap = approved_snapshot("b2-query-four");
    let planner = QueryPlanner::from_approved_snapshot(Arc::clone(&snap), 8)
        .map_err(|err| format!("from_approved_snapshot: {}", err.error_id()))?;
    let auth = authority(
        "b2-query-four-view",
        "world-a",
        "ctx-1",
        1,
        four_state_root("world-a", "ctx-1", 1),
    )?;
    let view = auth.capture();
    let hash0 = view.root().identity();
    let plan = planner
        .plan(
            &query_request(
                &["s:4:0:0", "s:3:0:0", "s:2:0:0", "s:1:0:0", "s:0:0:0"],
                false,
            ),
            &view,
            snap.as_ref(),
        )
        .map_err(|err| format!("plan four-state: {}", err.error_id()))?;
    let outcome = QueryExecutor::execute(&plan, &view)
        .map_err(|err| format!("execute four-state: {}", err.error_id()))?;
    let expected = [
        ("s:0:0:0", "Ready", true),
        ("s:1:0:0", "Unchanged", false),
        ("s:2:0:0", "Pending", false),
        ("s:3:0:0", "Unavailable", false),
        ("s:4:0:0", "Unchanged", false),
    ];
    if outcome.items().len() != expected.len() {
        return Err(format!("item count {}", outcome.items().len()));
    }
    for (item, (id, presence, ready)) in outcome.items().iter().zip(expected) {
        intern_presence(item.presence())?;
        if item.section_id() != id || item.presence() != presence {
            return Err(format!(
                "{} mapped to {} not {presence}",
                item.section_id(),
                item.presence()
            ));
        }
        if (item.presence() == "Ready") != ready {
            return Err(format!("{} ready flag", item.section_id()));
        }
        if ready {
            let schema = item
                .schema_id()
                .ok_or_else(|| "Ready missing schema_id".to_string())?;
            require_schema(schema)?;
        } else if item.schema_id().is_some() {
            return Err(format!("{} leaked schema_id", item.section_id()));
        }
    }
    let missing = outcome.evidence().missing_states();
    if missing.len() != 4
        || missing[3].section_id() != "s:4:0:0"
        || missing[3].presence() != "Unchanged"
    {
        return Err("absent directory id is not Unchanged".into());
    }
    if view.root().identity() != hash0 {
        return Err("four-state execute mutated the root identity".into());
    }
    Ok("Ready/Unchanged/Pending/Unavailable + absent Unchanged; identity unchanged".into())
}

fn query_cancel_budget() -> Result<String, String> {
    let snap = approved_snapshot("b2-query-budget");
    let n = 2;
    let planner = QueryPlanner::from_approved_snapshot(Arc::clone(&snap), n)
        .map_err(|err| format!("from_approved_snapshot: {}", err.error_id()))?;
    let auth = authority(
        "b2-query-budget-view",
        "world-a",
        "ctx-1",
        1,
        dummy_root("world-a", "ctx-1", 1, 0, b"must-not-read"),
    )?;
    let view = auth.capture();
    let hash0 = view.root().identity();
    let plan = planner
        .plan(
            &query_request(&["s:0:0:0", "s:1:0:0"], false),
            &view,
            snap.as_ref(),
        )
        .map_err(|err| format!("plan: {}", err.error_id()))?;
    let cancelled = QueryExecutor::execute_cancelled(&plan, &view)
        .err()
        .ok_or_else(|| "execute_cancelled succeeded".to_string())?;
    require_stable(cancelled.error_id())?;
    if cancelled.error_id() != "LoaderCancelled" {
        return Err(format!("cancel {}", cancelled.error_id()));
    }
    let first = QueryExecutor::execute(&plan, &view)
        .map_err(|err| format!("first walk: {}", err.error_id()))?;
    if first.evidence().budget_used() != n {
        return Err(format!("budget_used {}", first.evidence().budget_used()));
    }
    let over = QueryExecutor::walk(&plan, &view, first.evidence().budget_used())
        .err()
        .ok_or_else(|| "second walk succeeded".to_string())?;
    require_stable(over.error_id())?;
    if over.error_id() != "BudgetExceeded" {
        return Err(format!("second walk {}", over.error_id()));
    }
    if view.root().identity() != hash0 {
        return Err("cancel/budget mutated the root identity".into());
    }
    Ok("execute_cancelled LoaderCancelled; second walk BudgetExceeded".into())
}

fn prepare_wrong_world() -> Result<String, String> {
    let (auth, snap) = published_mutation_world("world-a", 1, "b2-prepare-wrong")?;
    let mut ledger = ReceiptLedger::from_approved_snapshot(snap, 4)
        .map_err(|err| format!("ledger: {}", err.error_id()))?;
    let view = auth.capture();
    let hash0 = view.root().identity();
    let dirty0 = view.dirty_frontier().clone();
    let req = mutation_request("txn-1", "world-b", 1, 0, &[("s:0:0:0", "edit")]);
    let err = prepare(&req, &view, &mut ledger)
        .err()
        .ok_or_else(|| "wrong-world prepare succeeded".to_string())?;
    require_stable(err.error_id())?;
    if err.error_id() != "SessionMismatch" {
        return Err(format!("wrong world {}", err.error_id()));
    }
    match ledger
        .lookup(&req)
        .map_err(|err| format!("lookup: {}", err.error_id()))?
    {
        LookupOutcome::Vacant => {}
        other => return Err(format!("failed prepare reserved {other:?}")),
    }
    let after = auth.capture();
    if after.root().identity() != hash0 || after.dirty_frontier() != &dirty0 {
        return Err("failed prepare published or dirtied the root".into());
    }
    Ok("SessionMismatch; ledger Vacant; root identity unchanged".into())
}

fn prepare_does_not_publish() -> Result<String, String> {
    let (auth, snap) = published_mutation_world("world-a", 1, "b2-prepare-ok")?;
    let mut ledger = ReceiptLedger::from_approved_snapshot(snap, 4)
        .map_err(|err| format!("ledger: {}", err.error_id()))?;
    let view = auth.capture();
    let hash0 = view.root().identity();
    let dirty0 = view.dirty_frontier().clone();
    let req = mutation_request("txn-1", "world-a", 1, 0, &[("s:0:0:0", "edit")]);
    let expected_fp = canonical_fingerprint(&req).map_err(|err| err.to_string())?;
    let token =
        prepare(&req, &view, &mut ledger).map_err(|err| format!("prepare: {}", err.error_id()))?;
    if token.fingerprint() != expected_fp {
        return Err("prepared fingerprint mismatch".into());
    }
    if token.base_identity() != hash0 {
        return Err("prepared base identity mismatch".into());
    }
    match ledger
        .lookup(&req)
        .map_err(|err| format!("lookup: {}", err.error_id()))?
    {
        LookupOutcome::InFlight => {}
        other => return Err(format!("successful prepare lookup {other:?}")),
    }
    let after = auth.capture();
    if after.root().identity() != hash0 || after.dirty_frontier() != &dirty0 {
        return Err("successful prepare published".into());
    }
    drop(token);
    Ok("prepare reserved InFlight and did not publish".into())
}

fn commit_atomic_duplicate() -> Result<String, String> {
    let (auth, snap) = published_mutation_world("world-a", 1, "b2-commit")?;
    let mut ledger = ReceiptLedger::from_approved_snapshot(snap, 4)
        .map_err(|err| format!("ledger: {}", err.error_id()))?;
    let before = auth.capture();
    let hash0 = before.root().identity();
    let req = mutation_request("txn-1", "world-a", 1, 0, &[("s:0:0:0", "edit")]);
    let prepared = prepare(&req, &before, &mut ledger)
        .map_err(|err| format!("prepare: {}", err.error_id()))?;
    let receipt = commit(prepared, &auth, &mut ledger)
        .map_err(|err| format!("commit: {}", err.error_id()))?;
    if receipt.evidence.old_root != hash0 {
        return Err("commit old_root mismatch".into());
    }
    assert_consistent_cut(&before)?;
    if before.root().identity() != hash0 {
        return Err("pre-commit capture mixed with the new cut".into());
    }
    let after = auth.capture();
    assert_consistent_cut(&after)?;
    let hash1 = after.root().identity();
    if hash1 == hash0 || receipt.evidence.new_root != hash1 {
        return Err("commit did not swap to a complete new identity".into());
    }
    if after.stamp().world_revision != 1 {
        return Err(format!(
            "committed world_revision {}",
            after.stamp().world_revision
        ));
    }
    if after
        .dirty_frontier()
        .reason("s:0:0:0")
        .map_err(|err| err.error_id().to_string())?
        != Some("mutation")
    {
        return Err("commit dirty reason is not mutation".into());
    }
    match ledger
        .lookup(&req)
        .map_err(|err| format!("lookup: {}", err.error_id()))?
    {
        LookupOutcome::Duplicate { receipt: stored } if stored == receipt.receipt => {}
        other => return Err(format!("commit lookup {other:?}")),
    }

    let conflict = mutation_request("txn-1", "world-a", 1, 0, &[("s:0:0:0", "edit-b")]);
    let conflict_err = prepare(&conflict, &auth.capture(), &mut ledger)
        .err()
        .ok_or_else(|| "conflict fingerprint prepare succeeded".to_string())?;
    require_stable(conflict_err.error_id())?;
    if conflict_err.error_id() != "RevisionConflict" {
        return Err(format!("conflict {}", conflict_err.error_id()));
    }
    if conflict_err.disposition() != Some(ReplayDisposition::Conflict) {
        return Err("conflict missing ReplayDisposition::Conflict".into());
    }
    if auth.capture().root().identity() != hash1 {
        return Err("conflict prepare mutated the published identity".into());
    }

    let (auth2, snap2) = published_mutation_world("world-a", 1, "b2-commit")?;
    let mut ledger2 = ReceiptLedger::from_approved_snapshot(snap2, 4)
        .map_err(|err| format!("ledger2: {}", err.error_id()))?;
    let view2 = auth2.capture();
    let hash2 = view2.root().identity();
    let req2 = mutation_request("txn-dup", "world-a", 1, 0, &[("s:0:0:0", "edit-2")]);
    let prepared2 = prepare(&req2, &view2, &mut ledger2)
        .map_err(|err| format!("prepare dup: {}", err.error_id()))?;
    ledger2
        .finalize(&req2, receipt.receipt.clone())
        .map_err(|err| format!("finalize dup: {}", err.error_id()))?;
    let replayed = commit(prepared2, &auth2, &mut ledger2)
        .map_err(|err| format!("duplicate commit: {}", err.error_id()))?;
    if replayed.receipt != receipt.receipt {
        return Err("duplicate commit returned a different receipt".into());
    }
    if auth2.capture().root().identity() != hash2 {
        return Err("duplicate commit published a second root".into());
    }
    Ok("commit swapped complete new identity; duplicate receipt; RevisionConflict".into())
}

fn dual_world_fault_isolation() -> Result<String, String> {
    if !SNAPSHOT_FEATURE {
        return Err("lumio-voxel-ops snapshot feature is off".into());
    }
    let (authority_role, replica_role) = intern_local_embedded_pair("Authority", "Replica")
        .map_err(|err| format!("intern_local_embedded_pair: {}", err.error_id()))?;
    let mut world_a = create_world(authority_role, "ctx-b2-a", "world-b2-a", "b2-fault-a")?;
    let mut world_b = create_world(replica_role, "ctx-b2-b", "world-b2-b", "b2-fault-b")?;
    drive_to_running(&mut world_a)?;
    drive_to_running(&mut world_b)?;
    if world_a.generation_guard().generation() == world_b.generation_guard().generation() {
        return Err("worlds share instance generation".into());
    }
    publish_cut(&world_a, b"b2-auth-cut")?;
    let id_a = identity_of(&world_a);
    let id_b = identity_of(&world_b);
    if id_a == id_b {
        return Err("independent worlds published the same identity".into());
    }

    let forbidden = reject_forbidden(ForbiddenWork::Io);
    require_stable(forbidden.error_id())?;
    if forbidden.error_id() != "LoaderTimeout" {
        return Err(format!("reject_forbidden Io {}", forbidden.error_id()));
    }
    if identity_of(&world_a) != id_a || identity_of(&world_b) != id_b {
        return Err("reject_forbidden mutated a published identity".into());
    }

    let mut sink = WorldEventSink::bounded(8);
    let evidence = FaultEvidence::new("invariant-breach");
    WorldFaultPort::trip(&mut world_a, &mut sink, "MaintenanceKick", &evidence)
        .map_err(|err| format!("trip A: {}", err.error_id()))?;
    if world_a.state_view().lifecycle() != "Disposed" {
        return Err(format!(
            "tripped A lifecycle {}",
            world_a.state_view().lifecycle()
        ));
    }
    if identity_of(&world_a) != id_a {
        return Err("trip mutated A's published identity".into());
    }
    if identity_of(&world_b) != id_b {
        return Err("trip on A changed B's capture identity".into());
    }
    let q_after = query_cmd(&world_b, "q-after-a-trip")?;
    admit(&mut world_b, q_after)
        .map_err(|err| format!("B query after A trip: {}", err.error_id()))?;

    let mut shutdown_sink = WorldEventSink::bounded(8);
    WorldShutdown::begin(&mut world_b, &mut shutdown_sink)
        .map_err(|err| format!("shutdown begin: {}", err.error_id()))?;
    WorldShutdown::drain(&mut world_b, &mut shutdown_sink)
        .map_err(|err| format!("shutdown drain: {}", err.error_id()))?;
    WorldShutdown::finalize(&mut world_b, &mut shutdown_sink)
        .map_err(|err| format!("shutdown finalize: {}", err.error_id()))?;
    if world_b.state_view().lifecycle() != "Disposed" {
        return Err(format!(
            "B shutdown lifecycle {}",
            world_b.state_view().lifecycle()
        ));
    }
    if identity_of(&world_b) != id_b {
        return Err("WorldShutdown mutated B's capture identity".into());
    }
    Ok("dual instances; Io rejected; trip A leaves B identity; shutdown ordered".into())
}

fn capture_encode_outside_barrier() -> Result<String, String> {
    let mut world = create_world(
        "Authority",
        "ctx-b2-capture",
        "world-b2-capture",
        "b2-capture",
    )?;
    drive_to_running(&mut world)?;
    let view = world.publication_authority().capture();
    let hash0 = view.root().identity();
    let cut = RuntimeSnapshotCut::from_live(&world, "cut-b2");
    let (captured, evidence) =
        capture(&mut world, &cut).map_err(|err| format!("capture: {}", err.error_id()))?;
    if !evidence.barrier_released {
        return Err("CaptureCut occupancy was not released".into());
    }
    if captured.root_identity() != hash0 || captured.stamp() != view.stamp() {
        return Err("capture did not pin the live cut".into());
    }
    let mut writer = MemoryCaptureWriter::new(8192);
    let meta = encode_capture(&captured, &mut writer)
        .map_err(|err| format!("encode_capture: {}", err.error_id()))?;
    if meta.root_identity() != captured.root_identity() {
        return Err("encode root_identity diverged".into());
    }
    if meta.world_revision() != captured.stamp().world_revision {
        return Err("encode world_revision diverged".into());
    }
    drop(captured);
    let lease = WorldWriteLane::try_acquire(&mut world)
        .map_err(|err| format!("lane after capture: {}", err.error_id()))?;
    drop(lease);
    if identity_of(&world) != hash0 {
        return Err("encode mutated the live identity".into());
    }
    Ok("capture released CaptureCut; encode_capture ran outside the barrier".into())
}

fn restore_preflight_and_swap() -> Result<String, String> {
    let snap = approved_snapshot("b2-restore");
    let mut world = create_world(
        "Authority",
        "ctx-b2-restore",
        "world-b2-restore",
        "b2-restore",
    )?;
    drive_to_running(&mut world)?;
    publish_cut(&world, b"restore-src")?;
    let before = identity_of(&world);
    let cut = RuntimeSnapshotCut::from_live(&world, "cut-restore");
    let (captured, evidence) =
        capture(&mut world, &cut).map_err(|err| format!("capture: {}", err.error_id()))?;
    if !evidence.barrier_released {
        return Err("restore capture held the barrier".into());
    }
    let mut writer = MemoryCaptureWriter::new(8192);
    encode_capture(&captured, &mut writer).map_err(|err| format!("encode: {}", err.error_id()))?;
    let bytes = writer.as_slice().to_vec();
    drop(captured);
    if bytes.len() < 2 {
        return Err("encoded capture too small to truncate".into());
    }

    let truncated = RestorePreflight::validate(
        &bytes[..bytes.len() / 2],
        world.state_view().world_id(),
        world.state_view().instance_generation(),
        snap.as_ref(),
    )
    .err()
    .ok_or_else(|| "truncated preflight succeeded".to_string())?;
    require_stable(truncated.error_id())?;
    if truncated.error_id() != "InvalidHandle" {
        return Err(format!("truncated {}", truncated.error_id()));
    }
    if identity_of(&world) != before {
        return Err("truncated preflight mutated identity".into());
    }

    let decoded = RestorePreflight::validate(
        &bytes,
        world.state_view().world_id(),
        world.state_view().instance_generation(),
        snap.as_ref(),
    )
    .map_err(|err| format!("preflight: {}", err.error_id()))?;
    let candidate = RestoreShadowBuilder::build(&decoded)
        .map_err(|err| format!("shadow: {}", err.error_id()))?;
    if !candidate.hash_matches() {
        return Err("shadow hash mismatch".into());
    }
    let receipt =
        restore(&mut world, candidate).map_err(|err| format!("restore: {}", err.error_id()))?;
    if receipt.old_root() != before || receipt.new_root() == before {
        return Err("restore did not swap identity".into());
    }
    if identity_of(&world) != receipt.new_root() {
        return Err("live identity is not the restored root".into());
    }
    let lease = WorldWriteLane::try_acquire(&mut world)
        .map_err(|err| format!("lane after restore: {}", err.error_id()))?;
    drop(lease);
    Ok("truncated bytes InvalidHandle; restore swapped a new identity".into())
}

fn durability_ack_covers_latest() -> Result<String, String> {
    require_schema("voxel-durability-ack")?;
    let mut world = create_world("Authority", "ctx-b2-ack", "world-b2-ack", "b2-ack")?;
    drive_to_running(&mut world)?;
    seed_ready(&world, &["s:0:0:0"])?;
    mutate(&mut world, "txn-dirty", &[("s:0:0:0", "edit")])?;
    let latest = latest_dirty(&world, "s:0:0:0")?.ok_or_else(|| "section not dirty".to_string())?;
    if latest == 0 {
        return Err("no older revision available for a stale ack".into());
    }
    let before = identity_of(&world);
    let frontier = world
        .publication_authority()
        .capture()
        .dirty_frontier()
        .clone();

    let old: DurabilityAckEvidence = ack_for(&world, &[("s:0:0:0", latest - 1)]);
    let old_covered = frontier
        .covered_by(&old)
        .map_err(|err| format!("covered_by old: {}", err.error_id()))?;
    if old_covered
        .contains("s:0:0:0")
        .map_err(|err| err.error_id().to_string())?
    {
        return Err("old ack covered the newer dirty entry".into());
    }
    if frontier
        .except_covered(&old_covered)
        .latest_revision("s:0:0:0")
        .map_err(|err| err.error_id().to_string())?
        != Some(latest)
    {
        return Err("except_covered dropped uncovered dirty".into());
    }
    let old_receipt = apply_durability_ack(&mut world, old)
        .map_err(|err| format!("old ack: {}", err.error_id()))?;
    if old_receipt.coverage_len() != 0
        || old_receipt.old_root() != before
        || old_receipt.new_root() != before
        || identity_of(&world) != before
        || latest_dirty(&world, "s:0:0:0")? != Some(latest)
    {
        return Err("old ack cleared newer dirty".into());
    }

    let covering: DurabilityAckEvidence = ack_for(&world, &[("s:0:0:0", latest)]);
    let receipt = apply_durability_ack(&mut world, covering)
        .map_err(|err| format!("covering ack: {}", err.error_id()))?;
    if receipt.coverage_len() != 1 || receipt.old_root() != before || receipt.new_root() == before {
        return Err("covering ack did not publish a new identity".into());
    }
    if identity_of(&world) != receipt.new_root() || latest_dirty(&world, "s:0:0:0")?.is_some() {
        return Err("covering ack left dirty in place".into());
    }
    let lease = WorldWriteLane::try_acquire(&mut world)
        .map_err(|err| format!("lane after ack: {}", err.error_id()))?;
    drop(lease);
    Ok("old ack no-op; covering DurabilityAck clears latest and swaps identity".into())
}

fn port_adapter_routes() -> Result<String, String> {
    let interned_schema = intern_schema("voxel-world-port")?;
    let mut world = create_world("Authority", "ctx-b2-port", "world-b2-port", "b2-port")?;
    drive_to_running(&mut world)?;
    let stamp_before = world.publication_authority().capture().stamp().clone();
    let id_before = identity_of(&world);
    let query_env = query_envelope(&world, "q-port")?;
    let cut = RuntimeSnapshotCut::from_live(&world, "cut-port");
    let mut_env = mutation_envelope(&world, "txn-port")?;
    let (via_query, receipt) = {
        let mut adapter = GeneratedVoxelWorldPortAdapter::new(&mut world);
        if !std::ptr::eq(adapter.schema_id(), interned_schema) {
            return Err("adapter.schema_id is not interned SCHEMA_IDS".into());
        }
        let via_query = adapter
            .query(query_env)
            .map_err(|err| format!("adapter query: {}", err.error_id()))?;
        let (captured, evidence) = adapter
            .capture(&cut)
            .map_err(|err| format!("adapter capture: {}", err.error_id()))?;
        if !evidence.barrier_released {
            return Err("adapter capture held CaptureCut".into());
        }
        if captured.root_identity() != id_before || captured.root_identity() != evidence.root_hash {
            return Err("adapter capture evidence hash mismatch".into());
        }
        drop(captured);
        let prepared = adapter
            .prepare_mutation(mut_env)
            .map_err(|err| format!("adapter prepare: {}", err.error_id()))?;
        let receipt = adapter
            .commit(prepared)
            .map_err(|err| format!("adapter commit: {}", err.error_id()))?;
        (via_query, receipt)
    };
    if via_query.payload.evidence().read_stamp() != &stamp_before {
        return Err("adapter query mixed a later stamp".into());
    }
    if receipt.payload.txn_id != "txn-port" || receipt.payload.evidence.txn_id != "txn-port" {
        return Err("adapter commit txn mismatch".into());
    }
    if receipt.payload.evidence.old_root != id_before
        || receipt.payload.evidence.new_root == id_before
        || identity_of(&world) != receipt.payload.evidence.new_root
    {
        return Err("adapter commit did not publish a new identity".into());
    }
    Ok("GeneratedVoxelWorldPortAdapter query/prepare/commit/capture routed".into())
}

fn fault_injector_recoverable() -> Result<String, String> {
    if !FaultInjector::recoverable(FaultPoint::PrePublication) {
        return Err("PrePublication must be recoverable".into());
    }
    if FaultInjector::recoverable(FaultPoint::PostPublication) {
        return Err("PostPublication must not be recoverable".into());
    }
    require_stable(FaultInjector::error_id(FaultPoint::PrePublication))?;
    require_stable(FaultInjector::error_id(FaultPoint::PostPublication))?;
    if FaultInjector::error_id(FaultPoint::PrePublication) != "InvalidHandle" {
        return Err("PrePublication error_id".into());
    }
    if FaultInjector::error_id(FaultPoint::PostPublication) != "PartialLoadRolledBack" {
        return Err("PostPublication error_id".into());
    }
    let mut injector = FaultInjector::new();
    injector.arm(FaultPoint::PrePublication);
    if injector.take() != Some(FaultPoint::PrePublication) {
        return Err("armed PrePublication was not taken".into());
    }
    injector.arm(FaultPoint::PostPublication);
    if injector.take() != Some(FaultPoint::PostPublication) {
        return Err("armed PostPublication was not taken".into());
    }
    Ok("PrePublication recoverable; PostPublication not recoverable".into())
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
    label: &str,
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
        approved_snapshot(label),
    )
    .map_err(|err| format!("VoxelWorld::create {role}: {}", err.error_id()))
}

fn authority(
    label: &str,
    world_id: &str,
    context: &str,
    generation: u64,
    initial: PublishedStateRoot,
) -> Result<PublicationAuthority, String> {
    let pins =
        PinRegistry::from_approved_snapshot(approved_snapshot(label), 16, context, generation);
    PublicationAuthority::new(world_id, context, generation, pins, initial)
        .map_err(|err| format!("PublicationAuthority::new: {}", err.error_id()))
}

fn published_mutation_world(
    world_id: &str,
    generation: u64,
    label: &str,
) -> Result<(PublicationAuthority, Arc<VoxelConfigSnapshot>), String> {
    let snap = approved_snapshot(label);
    let stamp = stamp_at(
        world_id,
        "ctx-1",
        generation,
        0,
        &[
            ("s:0:0:0", 0),
            ("s:1:0:0", 0),
            ("s:2:0:0", 0),
            ("s:3:0:0", 0),
        ],
    );
    let mut builder = SectionDirectoryBuilder::new();
    builder
        .insert("s:0:0:0", SectionSlot::ready(payload(b"base-ready")))
        .map_err(|err| format!("insert ready: {}", err.error_id()))?;
    builder
        .insert("s:1:0:0", SectionSlot::unchanged())
        .map_err(|err| format!("insert unchanged: {}", err.error_id()))?;
    builder
        .insert("s:2:0:0", SectionSlot::pending())
        .map_err(|err| format!("insert pending: {}", err.error_id()))?;
    builder
        .insert("s:3:0:0", SectionSlot::unavailable())
        .map_err(|err| format!("insert unavailable: {}", err.error_id()))?;
    let root = PublishedStateRoot::new(
        stamp,
        builder.freeze(),
        DirtyFrontier::new(world_id, generation).map_err(|err| err.error_id().to_string())?,
    );
    let pins = PinRegistry::from_approved_snapshot(Arc::clone(&snap), 16, "ctx-1", generation);
    let auth = PublicationAuthority::new(world_id, "ctx-1", generation, pins, root)
        .map_err(|err| format!("PublicationAuthority::new: {}", err.error_id()))?;
    Ok((auth, snap))
}

fn origin_of(world: &VoxelWorld, request_id: &str) -> Result<OriginToken, String> {
    let guard = world.generation_guard();
    OriginToken::try_new(
        guard.world_context_id(),
        guard.generation(),
        request_id,
        0,
        BTreeMap::new(),
        "VoxelCommit",
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

fn admit(world: &mut VoxelWorld, command: WorldCommand) -> Result<AdmittedCommand, WorldError> {
    world.endpoint().admit(command)
}

fn drive(world: &mut VoxelWorld, steps: &[(&'static str, &'static str)]) -> Result<(), String> {
    for (event, to) in steps {
        let cmd = lifecycle_cmd(world, event, to)?;
        admit(world, cmd).map_err(|err| format!("{event}->{to}: {}", err.error_id()))?;
        if world.state_view().lifecycle() != *to {
            return Err(format!(
                "{event} left lifecycle {}",
                world.state_view().lifecycle()
            ));
        }
    }
    Ok(())
}

fn drive_to_running(world: &mut VoxelWorld) -> Result<(), String> {
    drive(
        world,
        &[
            ("Initialize", "Initialized"),
            ("Prime", "Ready"),
            ("Start", "Running"),
        ],
    )
}

fn query_cmd(world: &VoxelWorld, query_id: &str) -> Result<WorldCommand, String> {
    let view = world.state_view();
    Ok(WorldCommand::Query {
        origin: origin_of(world, query_id)?,
        request: GeneratedVoxelQueryRequest {
            query_id: query_id.to_string(),
            world_id: view.world_id().to_string(),
            context: view.world_context_id().to_string(),
            section_ids: vec!["s:0:0:0".to_string()],
            cancel: false,
        },
    })
}

fn query_envelope(
    world: &VoxelWorld,
    query_id: &str,
) -> Result<OriginEnvelope<GeneratedVoxelQueryRequest>, String> {
    let view = world.state_view();
    Ok(OriginEnvelope {
        origin: origin_of(world, query_id)?,
        config_hash: String::new(),
        payload: GeneratedVoxelQueryRequest {
            query_id: query_id.to_string(),
            world_id: view.world_id().to_string(),
            context: view.world_context_id().to_string(),
            section_ids: vec!["s:0:0:0".to_string()],
            cancel: false,
        },
    })
}

fn mutation_envelope(
    world: &VoxelWorld,
    txn_id: &str,
) -> Result<OriginEnvelope<MutationRequest>, String> {
    let view = world.state_view();
    let world_revision = world
        .publication_authority()
        .capture()
        .stamp()
        .world_revision;
    let mut fields = BTreeMap::new();
    fields.insert("world_revision".to_string(), world_revision.to_string());
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

fn query_request(sections: &[&str], cancel: bool) -> GeneratedVoxelQueryRequest {
    GeneratedVoxelQueryRequest {
        query_id: "q-1".to_string(),
        world_id: "world-a".to_string(),
        context: "ctx-1".to_string(),
        section_ids: sections
            .iter()
            .map(|section| (*section).to_string())
            .collect(),
        cancel,
    }
}

fn mutation_request(
    txn_id: &str,
    world_id: &str,
    generation: u64,
    world_revision: u64,
    extra: &[(&str, &str)],
) -> MutationRequest {
    let mut fields = BTreeMap::new();
    fields.insert("world_revision".to_string(), world_revision.to_string());
    for (k, v) in extra {
        fields.insert((*k).to_string(), (*v).to_string());
    }
    MutationRequest {
        txn_id: txn_id.to_string(),
        world_id: world_id.to_string(),
        generation,
        fields,
    }
}

fn world_rev(n: u64) -> WorldRevision {
    let mut alloc = RevisionAllocator::new();
    for _ in 0..n {
        alloc.reserve_world().unwrap().abandon();
    }
    let mut reserved = alloc.reserve_world().unwrap();
    reserved.finalize().unwrap()
}

fn stamp_at(
    world_id: &str,
    context_id: &str,
    generation: u64,
    world_rev_n: u64,
    sections: &[(&str, u64)],
) -> GeneratedRevisionStamp {
    let world = world_rev(world_rev_n);
    let mut pairs = Vec::new();
    for (id, rev) in sections {
        let mut section_alloc = RevisionAllocator::new();
        for _ in 0..*rev {
            section_alloc.reserve_section().unwrap().abandon();
        }
        let mut reserved = section_alloc.reserve_section().unwrap();
        pairs.push((id.to_string(), reserved.finalize().unwrap()));
    }
    to_generated_stamp(world_id, context_id, generation, world, &pairs)
}

fn payload(bytes: &[u8]) -> SectionPayload {
    SectionPayload::from_pages([SectionPage::new(
        "Dense",
        "None",
        bytes.to_vec(),
        sha256(bytes),
    )])
    .expect("valid dense uncompressed page")
}

fn empty_replacement(base: &SectionDirectoryRoot) -> SectionReplacement {
    SectionDeltaBuilder::new(base)
        .freeze()
        .expect("empty replacement")
}

fn dummy_root(
    world_id: &str,
    context: &str,
    generation: u64,
    world_revision: u64,
    payload_bytes: &[u8],
) -> PublishedStateRoot {
    let mut builder = SectionDirectoryBuilder::new();
    builder
        .insert("s:0:0:0", SectionSlot::ready(payload(payload_bytes)))
        .expect("canonical dummy id");
    PublishedStateRoot::new(
        GeneratedRevisionStamp {
            schema_id: REVISION_STAMP_SCHEMA,
            world_id: world_id.to_string(),
            context_id: context.to_string(),
            generation,
            world_revision,
            section_revision_set: BTreeMap::new(),
        },
        builder.freeze(),
        DirtyFrontier::new(world_id, generation).expect("world id"),
    )
}

fn four_state_root(world_id: &str, context: &str, generation: u64) -> PublishedStateRoot {
    let mut builder = SectionDirectoryBuilder::new();
    builder
        .insert("s:0:0:0", SectionSlot::ready(payload(b"ready-bytes")))
        .expect("Ready");
    builder
        .insert("s:1:0:0", SectionSlot::unchanged())
        .expect("Unchanged");
    builder
        .insert("s:2:0:0", SectionSlot::pending())
        .expect("Pending");
    builder
        .insert("s:3:0:0", SectionSlot::unavailable())
        .expect("Unavailable");
    PublishedStateRoot::new(
        GeneratedRevisionStamp {
            schema_id: REVISION_STAMP_SCHEMA,
            world_id: world_id.to_string(),
            context_id: context.to_string(),
            generation,
            world_revision: 0,
            section_revision_set: BTreeMap::new(),
        },
        builder.freeze(),
        DirtyFrontier::new(world_id, generation).expect("world id"),
    )
}

fn publish_cut(world: &VoxelWorld, label: &[u8]) -> Result<(), String> {
    let view = world.state_view();
    let before = world.publication_authority().capture();
    let next = before.stamp().world_revision + 1;
    let mut prepared = world
        .publication_authority()
        .prepare(
            world_rev(next),
            root_at(
                view.world_id(),
                view.world_context_id(),
                view.instance_generation(),
                next,
                SectionSlot::ready(payload(label)),
                Some("mutation"),
            ),
            empty_replacement(before.directory()),
        )
        .map_err(|err| format!("prepare cut: {}", err.error_id()))?;
    world
        .publication_authority()
        .publish_once(
            prepared
                .seal()
                .map_err(|err| format!("seal cut: {}", err.error_id()))?,
        )
        .map_err(|err| format!("publish cut: {}", err.error_id()))?;
    Ok(())
}

fn root_at(
    world_id: &str,
    context_id: &str,
    generation: u64,
    world_rev_n: u64,
    slot: SectionSlot,
    dirty_reason: Option<&str>,
) -> PublishedStateRoot {
    let mut builder = SectionDirectoryBuilder::new();
    builder
        .insert("s:0:0:0", slot)
        .expect("canonical section id");
    let directory = builder.freeze();
    let stamp = GeneratedRevisionStamp {
        schema_id: REVISION_STAMP_SCHEMA,
        world_id: world_id.to_string(),
        context_id: context_id.to_string(),
        generation,
        world_revision: world_rev_n,
        section_revision_set: BTreeMap::from([("s:0:0:0".to_string(), world_rev_n)]),
    };
    let dirty = match dirty_reason {
        Some(reason) => DirtyFrontier::new(world_id, generation)
            .expect("world id")
            .record("s:0:0:0", world_rev_n, reason)
            .expect("record dirty"),
        None => DirtyFrontier::new(world_id, generation).expect("world id"),
    };
    PublishedStateRoot::new(stamp, directory, dirty)
}

fn seed_ready(world: &VoxelWorld, sections: &[&str]) -> Result<(), String> {
    let view = world.state_view();
    let before = world.publication_authority().capture();
    let next = before.stamp().world_revision + 1;
    let mut builder = SectionDirectoryBuilder::new();
    let mut section_revision_set = BTreeMap::new();
    for id in sections {
        builder
            .insert(id, SectionSlot::ready(payload(id.as_bytes())))
            .map_err(|err| format!("seed {id}: {}", err.error_id()))?;
        section_revision_set.insert((*id).to_string(), next);
    }
    let stamp = GeneratedRevisionStamp {
        schema_id: REVISION_STAMP_SCHEMA,
        world_id: view.world_id().to_string(),
        context_id: view.world_context_id().to_string(),
        generation: view.instance_generation(),
        world_revision: next,
        section_revision_set,
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

fn mutate(world: &mut VoxelWorld, txn_id: &str, sections: &[(&str, &str)]) -> Result<(), String> {
    let view = world.state_view();
    let world_revision = world
        .publication_authority()
        .capture()
        .stamp()
        .world_revision;
    let mut fields = BTreeMap::new();
    fields.insert("world_revision".to_string(), world_revision.to_string());
    for (id, value) in sections {
        fields.insert((*id).to_string(), (*value).to_string());
    }
    let request = MutationRequest {
        txn_id: txn_id.to_string(),
        world_id: view.world_id().to_string(),
        generation: view.instance_generation(),
        fields,
    };
    let mut lease = WorldWriteLane::try_acquire(world)
        .map_err(|err| format!("lane for mutation: {}", err.error_id()))?;
    lease
        .enter(BarrierScope::Mutation)
        .map_err(|err| format!("enter Mutation: {}", err.error_id()))?;
    let prepared = lease
        .prepare(&request)
        .map_err(|err| format!("lane prepare {txn_id}: {}", err.error_id()))?;
    lease
        .commit(prepared)
        .map_err(|err| format!("lane commit {txn_id}: {}", err.error_id()))?;
    Ok(())
}

fn ack_for(world: &VoxelWorld, sections: &[(&str, u64)]) -> DurabilityAckEvidence {
    let view = world.state_view();
    DurabilityAckEvidence {
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
        covered_sections: sections
            .iter()
            .map(|(id, rev)| CoveredSectionAck {
                section_id: (*id).to_string(),
                up_to_section_revision: *rev,
            })
            .collect(),
    }
}

fn latest_dirty(world: &VoxelWorld, section_id: &str) -> Result<Option<u64>, String> {
    world
        .publication_authority()
        .capture()
        .dirty_frontier()
        .latest_revision(section_id)
        .map_err(|err| err.error_id().to_string())
}

fn identity_of(world: &VoxelWorld) -> [u8; 32] {
    world.publication_authority().capture().root().identity()
}

fn assert_consistent_cut(view: &PublishedReadView) -> Result<(), String> {
    if view.stamp() != view.root().stamp() || view.stamp() != view.lease().stamp() {
        return Err("stamp/lease/root mixed".into());
    }
    if view.directory() != view.root().directory() {
        return Err("directory mixed with another cut".into());
    }
    if view.dirty_frontier() != view.root().dirty_frontier() {
        return Err("dirty frontier mixed with another cut".into());
    }
    Ok(())
}

fn intern_schema(id: &str) -> Result<&'static str, String> {
    SCHEMA_IDS
        .iter()
        .copied()
        .find(|item| *item == id)
        .ok_or_else(|| format!("{id} missing from SCHEMA_IDS"))
}

fn intern_presence(name: &str) -> Result<&'static str, String> {
    SECTION_PRESENCE
        .iter()
        .copied()
        .find(|item| *item == name)
        .ok_or_else(|| format!("{name} missing from SECTION_PRESENCE"))
}

fn require_schema(id: &str) -> Result<(), String> {
    intern_schema(id).map(|_| ())
}

fn require_stable(id: &str) -> Result<(), String> {
    if is_stable_error_id(id) {
        Ok(())
    } else {
        Err(format!(
            "{id} is neither a contract error code nor a frozen-mirror STABLE_ERROR_IDS member"
        ))
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
