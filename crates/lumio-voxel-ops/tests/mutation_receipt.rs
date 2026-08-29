//! R-00093: canonical fingerprint and txn receipt ledger.

use lumio_voxel_contracts::{BASELINE_ID, SCHEMA_EPOCH, SCHEMA_IDS, STABLE_ERROR_IDS, sha256};
use lumio_voxel_domain::config_snapshot::{
    DecisionEvidence, GateSourceHashes, GeneratedHostCapability, GeneratedVoxelConfig,
    P0_DECISION_GATES, VoxelConfigSnapshot,
};
use lumio_voxel_ops::mutation::{
    LedgerError, LookupOutcome, MUTATION_RECEIPT_SCHEMA, MutationRequest, ReceiptLedger,
    ReplayDisposition, canonical_fingerprint,
};
use std::collections::BTreeMap;
use std::sync::Arc;

fn hex32(bytes: &[u8; 32]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(64);
    for b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0xf) as usize] as char);
    }
    out
}

fn approved_snapshot() -> Arc<VoxelConfigSnapshot> {
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
        .map(|g| {
            (
                (*g).to_string(),
                hex32(&sha256(format!("approved-{g}").as_bytes())),
            )
        })
        .collect();
    let ev: Vec<DecisionEvidence> = P0_DECISION_GATES
        .iter()
        .map(|g| DecisionEvidence {
            gate_id: (*g).to_string(),
            approval_status: "approved".to_string(),
            source_hashes: source.clone(),
            evidence_digest: digests[*g].clone(),
        })
        .collect();
    let cfg = GeneratedVoxelConfig {
        schema_id: "config-table",
        host_capability_schema_id: "host-capability",
        schema_epoch: SCHEMA_EPOCH,
        config_hash: hex32(&sha256(b"r00093-approved")),
        gate_source_hashes: digests,
        host_capability: GeneratedHostCapability::from_names(["Native", "ReferenceVoxel"]),
        start_capabilities: vec!["Native".into(), "ReferenceVoxel".into()],
        key_material: None,
    };
    VoxelConfigSnapshot::from_generated(&cfg, &ev).expect("approved P0 snapshot")
}

fn request(txn_id: &str, fields: BTreeMap<String, String>) -> MutationRequest {
    MutationRequest {
        txn_id: txn_id.to_string(),
        world_id: "world-a".to_string(),
        generation: 1,
        fields,
    }
}

// Expected digests come from an independent Python implementation of the encoding,
// written from its written rules rather than from this crate's source. The previous
// version of this helper re-implemented the encoder inline, which made the assertion
// below hold whether or not values were escaped — it could not fail. Generator and
// canonical bytes: `docs/evidence/canonical-encoding-goldens.md`.
//
// `request("txn-1", {k1: "1", k2: "2"})`
const FINGERPRINT_K1_1_K2_2: &str =
    "5fb39c21495127d88c5e884668b9e9585a3dca72a7ee2ead9a2df4592333f075";
// `request("txn-1", {k1: "1", k2: "3"})`
const FINGERPRINT_K1_1_K2_3: &str =
    "bb20b986f36de85f4109b7fe854d876da6d113665211beeeba2bdb81bedc5e6b";
// `request("txn-1", {})`
const FINGERPRINT_NO_FIELDS: &str =
    "5698db6879ec4f6c542f6e76ca0e22153903f074f0c33292bc6462175aa74743";

fn digest(req: &MutationRequest) -> String {
    hex32(
        &canonical_fingerprint(req)
            .expect("request has no duplicate member")
            .hash()
            .0,
    )
}

#[test]
fn fingerprint_is_order_independent_and_field_sensitive() {
    assert!(SCHEMA_IDS.contains(&MUTATION_RECEIPT_SCHEMA));
    assert_eq!(MUTATION_RECEIPT_SCHEMA, "voxel-mutation-receipt");

    let mut fields_a = BTreeMap::new();
    fields_a.insert("k1".to_string(), "1".to_string());
    fields_a.insert("k2".to_string(), "2".to_string());
    let mut fields_b = BTreeMap::new();
    fields_b.insert("k2".to_string(), "2".to_string());
    fields_b.insert("k1".to_string(), "1".to_string());
    let r1 = request("txn-1", fields_a);
    let r2 = request("txn-1", fields_b);
    assert_eq!(digest(&r1), digest(&r2));
    assert_eq!(digest(&r1), FINGERPRINT_K1_1_K2_2);
    assert_eq!(
        digest(&request("txn-1", BTreeMap::new())),
        FINGERPRINT_NO_FIELDS
    );

    let mut fields_c = BTreeMap::new();
    fields_c.insert("k1".to_string(), "1".to_string());
    fields_c.insert("k2".to_string(), "3".to_string());
    let r3 = request("txn-1", fields_c);
    assert_ne!(digest(&r1), digest(&r3));
    assert_eq!(digest(&r3), FINGERPRINT_K1_1_K2_3);

    let snap = approved_snapshot();
    let mut ledger = ReceiptLedger::from_approved_snapshot(snap, 4).unwrap();
    ledger.reserve(&r1).unwrap();
    ledger.finalize(&r1, b"receipt-a".to_vec()).unwrap();
    let err = ledger.reserve(&r3).unwrap_err();
    assert_eq!(err.error_id(), "RevisionConflict");
    assert!(STABLE_ERROR_IDS.contains(&err.error_id()));
    assert_eq!(err.disposition(), Some(ReplayDisposition::Conflict));
}

#[test]
fn same_txn_same_fingerprint_finalize_twice_is_duplicate() {
    let mut fields = BTreeMap::new();
    fields.insert("k1".to_string(), "1".to_string());
    let req = request("txn-1", fields);
    let receipt = b"receipt-original".to_vec();
    let snap = approved_snapshot();
    let mut ledger = ReceiptLedger::from_approved_snapshot(snap, 4).unwrap();
    ledger.reserve(&req).unwrap();
    assert_eq!(ledger.reserve_count(), 1);
    let first = ledger.finalize(&req, receipt.clone()).unwrap();
    assert_eq!(first.disposition, ReplayDisposition::Original);
    assert_eq!(first.receipt, receipt);
    let second = ledger.finalize(&req, b"ignored-rewrite".to_vec()).unwrap();
    assert_eq!(second.disposition, ReplayDisposition::Duplicate);
    assert_eq!(second.receipt, first.receipt);
    assert_eq!(second.receipt, receipt);
    assert_eq!(ledger.reserve_count(), 1);
    match ledger.lookup(&req).unwrap() {
        LookupOutcome::Duplicate { receipt: stored } => assert_eq!(stored, receipt),
        other => panic!("expected duplicate lookup, got {other:?}"),
    }
}

#[test]
fn same_txn_different_fingerprint_is_conflict() {
    let mut fields_a = BTreeMap::new();
    fields_a.insert("k1".to_string(), "1".to_string());
    let mut fields_b = BTreeMap::new();
    fields_b.insert("k1".to_string(), "2".to_string());
    let req_a = request("txn-1", fields_a);
    let req_b = request("txn-1", fields_b);
    let receipt = b"receipt-first".to_vec();
    let snap = approved_snapshot();
    let mut ledger = ReceiptLedger::from_approved_snapshot(snap, 4).unwrap();
    ledger.reserve(&req_a).unwrap();
    let first = ledger.finalize(&req_a, receipt.clone()).unwrap();
    assert_eq!(first.disposition, ReplayDisposition::Original);

    let err: LedgerError = ledger
        .finalize(&req_b, b"receipt-other".to_vec())
        .unwrap_err();
    assert_eq!(err.error_id(), "RevisionConflict");
    assert!(STABLE_ERROR_IDS.contains(&err.error_id()));
    assert_eq!(err.disposition(), Some(ReplayDisposition::Conflict));

    match ledger.lookup(&req_a).unwrap() {
        LookupOutcome::Duplicate { receipt: stored } => assert_eq!(stored, receipt),
        other => panic!("first receipt must be unchanged, got {other:?}"),
    }
    let lookup_conflict = ledger.lookup(&req_b).unwrap_err();
    assert_eq!(lookup_conflict.error_id(), "RevisionConflict");
    assert_eq!(ledger.reserve_count(), 1);
}

#[test]
fn capacity_exhausted_leaves_ledger_unchanged() {
    let mut fields_a = BTreeMap::new();
    fields_a.insert("k1".to_string(), "1".to_string());
    let mut fields_b = BTreeMap::new();
    fields_b.insert("k1".to_string(), "1".to_string());
    let first = request("txn-1", fields_a);
    let second = request("txn-2", fields_b);
    let receipt = b"receipt-first".to_vec();
    let snap = approved_snapshot();
    let mut ledger = ReceiptLedger::from_approved_snapshot(snap.clone(), 1).unwrap();
    assert_eq!(ledger.config_hash(), snap.config_hash());
    ledger.reserve(&first).unwrap();
    ledger.finalize(&first, receipt.clone()).unwrap();
    assert_eq!(ledger.reserve_count(), 1);

    let err = ledger.reserve(&second).unwrap_err();
    assert!(err.error_id() == "BudgetExceeded" || err.error_id() == "QueueFull");
    assert!(STABLE_ERROR_IDS.contains(&err.error_id()));

    match ledger.lookup(&first).unwrap() {
        LookupOutcome::Duplicate { receipt: stored } => assert_eq!(stored, receipt),
        other => panic!("first entry must remain, got {other:?}"),
    }
    assert_eq!(ledger.reserve_count(), 1);
    assert_eq!(ledger.lookup(&second).unwrap(), LookupOutcome::Vacant);
}
