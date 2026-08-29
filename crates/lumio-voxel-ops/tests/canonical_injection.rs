//! Canonical encoding injection: two semantically different requests must never
//! share a fingerprint, and decode must be a faithful inverse of encode.
//!
//! Reproduces the `canonical_object_pairs` defect adjudicated 2026-08-29.

use lumio_voxel_contracts::{BASELINE_ID, SCHEMA_EPOCH, sha256};
use lumio_voxel_domain::config_snapshot::{
    DecisionEvidence, GateSourceHashes, GeneratedHostCapability, GeneratedVoxelConfig,
    P0_DECISION_GATES, VoxelConfigSnapshot,
};
use lumio_voxel_ops::canonical::CanonicalObject;
use lumio_voxel_ops::mutation::{
    LookupOutcome, MutationRequest, ReceiptLedger, canonical_fingerprint,
};
use lumio_voxel_ops::snapshot::decode_canonical_object;
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
        config_hash: hex32(&sha256(b"canonical-injection")),
        gate_source_hashes: digests,
        host_capability: GeneratedHostCapability::from_names(["Native", "ReferenceVoxel"]),
        start_capabilities: vec!["Native".into(), "ReferenceVoxel".into()],
        key_material: None,
    };
    VoxelConfigSnapshot::from_generated(&cfg, &ev).expect("approved P0 snapshot")
}

fn request(txn_id: &str, fields: &[(&str, &str)]) -> MutationRequest {
    MutationRequest {
        txn_id: txn_id.to_string(),
        world_id: "world-a".to_string(),
        generation: 1,
        fields: fields
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect(),
    }
}

fn digest(request: &MutationRequest) -> String {
    hex32(
        &canonical_fingerprint(request)
            .expect("request has no duplicate member")
            .hash()
            .0,
    )
}

// Digests below were produced by an independent Python implementation of the
// encoding written from its written rules, not from this crate's source, so they
// fail if the implementation drifts rather than drifting with it. The generator
// and the exact canonical bytes for each are in
// `docs/evidence/canonical-encoding-goldens.md`.
const FORGED_ABSORB_ONE: &str = "e85d6d8b9a46381b10225c217678d8f19aac4f6251e881eeccf791097bfcb52a";
const HONEST_TWO_FIELDS: &str = "d1d5607df6b923ab7c53b70ffca2ac7e5d7a0472df7db7f39213c4cc1178dc85";
const FORGED_ABSORB_TWO: &str = "d88dee1c6a4f232f3fab88b4f409191f1685ee0f147f71acd4afa9f05f41ef98";
const HONEST_THREE_FIELDS: &str =
    "eb9a2c4b8e94804c560c07f27a345ebc9f6b09287510e758efe42aa5bafa4f82";
const FORGED_TXN_ID_APPEND: &str =
    "43e486625335b5d174229284fcf35b33eb97a6fd8cb46c03a450036e1e4ef4fc";
const HONEST_EXTRA_FIELD: &str = "285c8ffee661bf7317e05287207b1ddef089f590b6fef8e57cf54254c1167b0c";
const ESCAPE_BOUNDARY: &str = "a36ba67b02000ea7ebe8d23d5babd60ca72770402f331bf58609c833ef524993";
const FORGED_CLOSE_OWN_QUOTES: &str =
    "2d61bd6d59add7a292abf80f9e2df50197479129c3a7acf4204887c1f2689e3c";
const FORGED_KEY_CLOSE_QUOTES: &str =
    "2fd1acdb9bfd53eecae462e386c7ffa6ea6b74b82a3edd542d190a356c408912";

/// Value substitution: one field absorbs a sibling member's name and value.
#[test]
fn a_field_value_cannot_absorb_a_sibling_member() {
    let forged = request("t", &[("a", "1,\"b\":2")]);
    let honest = request("t", &[("a", "1"), ("b", "2")]);
    assert_ne!(digest(&forged), digest(&honest));
    assert_eq!(digest(&forged), FORGED_ABSORB_ONE);
    assert_eq!(digest(&honest), HONEST_TWO_FIELDS);
}

/// Deletion: one field absorbs two sibling members, so three fields read as one.
#[test]
fn a_field_value_cannot_absorb_two_sibling_members() {
    let forged = request("t", &[("a", "1,\"b\":2,\"c\":3")]);
    let honest = request("t", &[("a", "1"), ("b", "2"), ("c", "3")]);
    assert_ne!(digest(&forged), digest(&honest));
    assert_eq!(digest(&forged), FORGED_ABSORB_TWO);
    assert_eq!(digest(&honest), HONEST_THREE_FIELDS);
}

/// Quoting alone is not enough. This value closes its own quotes and reopens them,
/// so it forges the same two members against an encoder that quotes but does not
/// escape — the shape that survives half-fixing this defect.
#[test]
fn a_quoted_field_value_cannot_close_its_own_quotes() {
    let forged = request("t", &[("a", "1\",\"b\":\"2")]);
    let honest = request("t", &[("a", "1"), ("b", "2")]);
    assert_ne!(digest(&forged), digest(&honest));
    assert_eq!(digest(&forged), FORGED_CLOSE_OWN_QUOTES);
    assert_eq!(digest(&honest), HONEST_TWO_FIELDS);
}

/// The same construction from the name side: names are quoted too, so they need
/// the same escape or a name can carry its own value and a sibling.
#[test]
fn a_field_name_cannot_close_its_own_quotes() {
    let forged = request("t", &[("a\":\"1\",\"b", "2")]);
    let honest = request("t", &[("a", "1"), ("b", "2")]);
    assert_ne!(digest(&forged), digest(&honest));
    assert_eq!(digest(&forged), FORGED_KEY_CLOSE_QUOTES);
}

/// Append: a `txn_id` carrying a quote must not be able to append a whole member.
#[test]
fn a_txn_id_cannot_append_a_forged_member() {
    let forged = request("t\",\"u\":\"9", &[]);
    let honest = request("t", &[("u", "\"9\"")]);
    assert_ne!(digest(&forged), digest(&honest));
    assert_eq!(digest(&forged), FORGED_TXN_ID_APPEND);
    assert_eq!(digest(&honest), HONEST_EXTRA_FIELD);
}

/// A caller field named like a contract member is refused, not silently merged.
#[test]
fn a_field_named_like_a_contract_member_is_rejected() {
    for shadowed in ["txn_id", "world_id", "generation", "canonicalForm"] {
        let request = request("t", &[(shadowed, "forged")]);
        let err =
            canonical_fingerprint(&request).expect_err("a field may not shadow a contract member");
        assert_eq!(err.key(), shadowed);
    }
}

/// Escapes are part of the judgment: quotes, backslashes and control characters
/// must all survive into a digest that an independent implementation agrees with.
#[test]
fn escaped_values_match_the_independent_oracle() {
    let request = MutationRequest {
        txn_id: "q\"\\z".to_string(),
        world_id: "world-a".to_string(),
        generation: 1,
        fields: [
            ("nl".to_string(), "a\nb".to_string()),
            ("bs".to_string(), "c\\d".to_string()),
            ("dq".to_string(), "e\"f".to_string()),
        ]
        .into_iter()
        .collect(),
    };
    assert_eq!(digest(&request), ESCAPE_BOUNDARY);
}

/// The consequence: a forged request must not be served the stored receipt.
#[test]
fn a_forged_request_is_not_served_the_stored_receipt() {
    let honest = request("txn-1", &[("a", "1"), ("b", "2")]);
    let forged = request("txn-1", &[("a", "1,\"b\":2")]);

    let mut ledger = ReceiptLedger::from_approved_snapshot(approved_snapshot(), 4).unwrap();
    ledger.reserve(&honest).unwrap();
    ledger
        .finalize(&honest, b"receipt-of-honest".to_vec())
        .unwrap();

    let outcome = ledger.lookup(&forged);
    assert!(
        !matches!(outcome, Ok(LookupOutcome::Duplicate { .. })),
        "a semantically different request was served the stored receipt"
    );
    let err = outcome.expect_err("forged request must be a conflict");
    assert_eq!(err.error_id(), "RevisionConflict");
}

/// Decode must be faithful: bytes encoded from two members must not read as three.
#[test]
fn decode_does_not_regroup_members() {
    let mut encoded = CanonicalObject::new();
    encoded.insert_text("a", "1,\"b\":2").unwrap();
    encoded.insert_text("c", "3").unwrap();
    let bytes = encoded.encode_bytes();

    let decoded = decode_canonical_object(&bytes).expect("bytes came from the encoder");
    assert_eq!(
        decoded.len(),
        encoded.len(),
        "a two-member object decoded as a different member count"
    );
    assert_eq!(decoded, encoded);
}

/// Duplicate members must be rejected, not accepted and silently deduplicated.
#[test]
fn decode_rejects_duplicate_members() {
    let err = decode_canonical_object(b"{\"a\":\"1\",\"a\":\"2\"}")
        .expect_err("duplicate member was accepted");
    assert_eq!(err.error_id(), "InvalidHandle");
}
