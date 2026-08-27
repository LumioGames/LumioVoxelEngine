use lumio_voxel_contracts::{
    ACTIVE_PERMISSION_FIELDS, BINDINGS, BoundedBuffer, CHUNK_PRESENCE, Hash256, MAPPING_ROLES,
    SCHEMA_IDS, SNAPSHOT_MAGIC, STABLE_ERROR_IDS, VOXEL_WORLD_ROLES, checksum_domain_doc,
    hash_chain_append, hash_chain_verify, is_active_field, machine_ids, sha256,
    state_transition_table,
};

#[test]
fn voxel_schema_ids_come_from_generated_artifact() {
    for id in [
        "voxel-world-port",
        "voxel-chunk-page",
        "voxel-revision-stamp",
        "voxel-query",
        "voxel-mutation-receipt",
        "voxel-snapshot-payload",
        "voxel-durability-ack",
    ] {
        assert!(
            SCHEMA_IDS.contains(&id),
            "missing voxel schema id {id} in generated ContractTypes"
        );
    }
}

#[test]
fn twelve_state_machines_include_voxel() {
    let machines: Vec<_> = machine_ids().collect();
    assert_eq!(machines.len(), 12, "{machines:?}");
    assert!(machines.contains(&"VoxelSnapshotCapture"));
    assert!(machines.contains(&"VoxelChunkResidency"));
}

#[test]
fn gas_ability_positive_fixture_transition_accepted() {
    let table = state_transition_table();
    assert!(
        table.iter().any(|t| t.machine == "GasAbility"
            && t.from == "Executing"
            && t.to == "Completed"
            && t.event == "Finish"),
        "gas-ability-complete Executing->Completed must come from generated table"
    );
}

#[test]
fn gas_ability_illegal_fixture_transition_rejected() {
    let table = state_transition_table();
    assert!(
        !table
            .iter()
            .any(|t| t.machine == "GasAbility" && t.from == "Completed" && t.to == "Requested"),
        "terminal Ability must not return to Requested"
    );
}

#[test]
fn canonical_and_runtime_come_from_generated_artifacts() {
    assert_eq!(SNAPSHOT_MAGIC, "LUMIOSNP1");
    assert!(checksum_domain_doc().contains("canonical JSON"));
    assert_eq!(VOXEL_WORLD_ROLES, &["Authority", "Replica"]);
    assert_eq!(
        CHUNK_PRESENCE,
        &["Ready", "NotLoaded", "Pending", "Unavailable"]
    );
    assert!(MAPPING_ROLES.contains(&"ServerToClient"));
    assert!(ACTIVE_PERMISSION_FIELDS.contains(&"verdict"));
    assert!(is_active_field("sessionId"));
    assert!(!is_active_field("inventedField"));
    assert!(BINDINGS.iter().any(|b| b.schema_id == "voxel-world-port"));
    assert!(STABLE_ERROR_IDS.contains(&"RevisionConflict"));

    let genesis = Hash256(sha256(b""));
    let next = hash_chain_append(&genesis, b"rec-1");
    assert!(hash_chain_verify(&genesis, b"rec-1", &next).is_ok());
    assert!(hash_chain_verify(&genesis, b"rec-2", &next).is_err());
    let mut buf = BoundedBuffer::new(1);
    assert!(buf.push(1).is_ok());
    assert!(buf.push(2).is_err());
}

#[test]
fn no_handwritten_schema_dto_in_contracts_lib() {
    let src = include_str!("../src/lib.rs");
    for needle in [
        "struct Chunk ",
        "struct World ",
        "enum ErrorCode",
        "struct SnapshotHeader",
        "struct MutationReceipt",
        "struct VoxelQuery",
    ] {
        assert!(
            !src.contains(needle),
            "handwritten DTO {needle} is forbidden; re-export generated artifacts only"
        );
    }
}
