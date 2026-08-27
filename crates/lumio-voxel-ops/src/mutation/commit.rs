//! Recheck a prepared batch, overlay one unpublished root, then seal.

#![forbid(unsafe_code)]

use super::commit_finalize::publish_once_and_finalize;
use super::fingerprint::{MUTATION_RECEIPT_SCHEMA, MutationRequest};
use super::plan::MutationPlanner;
use super::preconditions::MutationError;
use super::prepared_token::PreparedMutation;
use super::receipt_ledger::{LookupOutcome, ReceiptLedger};
use lumio_voxel_contracts::{CHUNK_PRESENCE, SCHEMA_IDS, canonical_object_pairs, sha256};
use lumio_voxel_domain::chunk::{ChunkDirectoryBuilder, ChunkDirectoryRoot, ChunkReplacement};
use lumio_voxel_domain::publication::{
    PublicationAuthority, PublishedReadView, PublishedStateRoot,
};
use lumio_voxel_domain::revision::{
    ChunkRevision, GeneratedRevisionStamp, RevisionAllocator, WorldRevision, to_generated_stamp,
};
use std::collections::BTreeSet;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommitEvidence {
    pub old_root: [u8; 32],
    pub new_root: [u8; 32],
    pub txn_id: String,
    pub receipt_hash: [u8; 32],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GeneratedMutationReceipt {
    pub txn_id: String,
    pub receipt: Vec<u8>,
    pub evidence: CommitEvidence,
}

/// Recheck, overlay, seal; `publish_once` is the only visible swap.
pub fn commit(
    prepared: PreparedMutation,
    authority: &PublicationAuthority,
    ledger: &mut ReceiptLedger,
) -> Result<GeneratedMutationReceipt, MutationError> {
    debug_assert!(SCHEMA_IDS.contains(&MUTATION_RECEIPT_SCHEMA));
    let view = authority.capture();
    recheck_prepared(&prepared, &view, ledger)?;
    // Prepare-time check rejects InFlight/Duplicate; commit re-runs presence then lookup.
    recheck_presence(prepared.request(), &view)?;

    match ledger
        .lookup(prepared.request())
        .map_err(MutationError::from_ledger)?
    {
        LookupOutcome::Duplicate { receipt } => {
            return Ok(duplicate_receipt(&prepared, &view, receipt));
        }
        LookupOutcome::Vacant => return Err(MutationError::invalid_handle()),
        LookupOutcome::InFlight => {}
    }

    let plan = MutationPlanner::build(prepared.request())?;
    let replacement = prepared.replacement().clone();
    let overlay_ids = overlay_ids(view.stamp().chunk_revision_set.keys(), plan.chunk_ids());
    let new_dir = overlay_directory(&view, &replacement, &overlay_ids)?;
    let new_stamp = build_stamp(&prepared, &view, &replacement, &overlay_ids)?;
    let new_root = PublishedStateRoot::new(new_stamp, new_dir, prepared.dirty().clone());
    let new_identity = new_root.identity();
    let world = world_revision(prepared.target_world_revision())?;
    let mut publication = authority
        .prepare(world, new_root, replacement)
        .map_err(MutationError::from_publish)?;
    let token = publication.seal().map_err(MutationError::from_publish)?;

    let request = prepared.request().clone();
    let txn_id = prepared.txn_id().to_string();
    let old_root = prepared.base_identity();
    let receipt_bytes = receipt_bytes(
        &txn_id,
        old_root,
        new_identity,
        prepared.fingerprint().hash().0,
    );
    let evidence = CommitEvidence {
        old_root,
        new_root: new_identity,
        txn_id: txn_id.clone(),
        receipt_hash: sha256(&receipt_bytes),
    };

    let receipt = publish_once_and_finalize(authority, ledger, token, request, receipt_bytes)?;
    Ok(GeneratedMutationReceipt {
        txn_id,
        receipt,
        evidence,
    })
}

fn recheck_prepared(
    prepared: &PreparedMutation,
    view: &PublishedReadView,
    ledger: &ReceiptLedger,
) -> Result<(), MutationError> {
    let stamp = view.stamp();
    if prepared.generation() != stamp.generation
        || prepared.request().generation != stamp.generation
        || prepared.reservation().generation() != stamp.generation
    {
        return Err(MutationError::stale_epoch());
    }
    if prepared.request().world_id != stamp.world_id
        || prepared.reservation().world_id() != stamp.world_id
    {
        return Err(MutationError::session_mismatch());
    }
    if prepared.config_hash() != ledger.config_hash() {
        return Err(MutationError::session_mismatch());
    }
    if prepared.base_identity() != view.root().identity() {
        return Err(MutationError::snapshot_base_mismatch());
    }
    Ok(())
}

fn duplicate_receipt(
    prepared: &PreparedMutation,
    view: &PublishedReadView,
    receipt: Vec<u8>,
) -> GeneratedMutationReceipt {
    let txn_id = prepared.txn_id().to_string();
    let receipt_hash = sha256(&receipt);
    GeneratedMutationReceipt {
        txn_id: txn_id.clone(),
        receipt,
        evidence: CommitEvidence {
            old_root: prepared.base_identity(),
            new_root: view.root().identity(),
            txn_id,
            receipt_hash,
        },
    }
}

fn overlay_ids<'a>(
    stamp_ids: impl Iterator<Item = &'a String>,
    request_ids: impl Iterator<Item = &'a str>,
) -> BTreeSet<String> {
    let mut ids = BTreeSet::new();
    for id in stamp_ids {
        ids.insert(id.clone());
    }
    for id in request_ids {
        ids.insert(id.to_string());
    }
    ids
}

fn overlay_directory(
    view: &PublishedReadView,
    replacement: &ChunkReplacement,
    overlay_ids: &BTreeSet<String>,
) -> Result<ChunkDirectoryRoot, MutationError> {
    let mut builder = ChunkDirectoryBuilder::new();
    for id in overlay_ids {
        let slot = match replacement
            .set()
            .get(id)
            .map_err(MutationError::from_chunk)?
            .cloned()
        {
            Some(slot) => slot,
            None => match view
                .directory()
                .lookup(id)
                .map_err(MutationError::from_chunk)?
                .cloned()
            {
                Some(slot) => slot,
                None => continue,
            },
        };
        builder
            .insert(id, slot)
            .map_err(MutationError::from_chunk)?;
    }
    Ok(builder.freeze())
}

fn build_stamp(
    prepared: &PreparedMutation,
    view: &PublishedReadView,
    replacement: &ChunkReplacement,
    overlay_ids: &BTreeSet<String>,
) -> Result<GeneratedRevisionStamp, MutationError> {
    let stamp = view.stamp();
    let mut pairs = Vec::new();
    for id in overlay_ids {
        let old = stamp
            .chunk_revision_set
            .get(id)
            .copied()
            .unwrap_or(stamp.world_revision);
        let replaced = replacement
            .set()
            .get(id)
            .map_err(MutationError::from_chunk)?
            .is_some();
        let rev_n = if replaced {
            old.checked_add(1)
                .ok_or_else(MutationError::invalid_handle)?
        } else {
            old
        };
        pairs.push((id.clone(), chunk_revision(rev_n)?));
    }
    Ok(to_generated_stamp(
        stamp.world_id.clone(),
        stamp.context_id.clone(),
        stamp.generation,
        world_revision(prepared.target_world_revision())?,
        &pairs,
    ))
}

fn world_revision(n: u64) -> Result<WorldRevision, MutationError> {
    let mut alloc = RevisionAllocator::new();
    for _ in 0..n {
        alloc
            .reserve_world()
            .map_err(|_| MutationError::invalid_handle())?
            .abandon();
    }
    alloc
        .reserve_world()
        .map_err(|_| MutationError::invalid_handle())?
        .finalize()
        .map_err(|_| MutationError::invalid_handle())
}

fn chunk_revision(n: u64) -> Result<ChunkRevision, MutationError> {
    let mut alloc = RevisionAllocator::new();
    for _ in 0..n {
        alloc
            .reserve_chunk()
            .map_err(|_| MutationError::invalid_handle())?
            .abandon();
    }
    alloc
        .reserve_chunk()
        .map_err(|_| MutationError::invalid_handle())?
        .finalize()
        .map_err(|_| MutationError::invalid_handle())
}

fn receipt_bytes(
    txn_id: &str,
    old_root: [u8; 32],
    new_root: [u8; 32],
    fingerprint: [u8; 32],
) -> Vec<u8> {
    let mut pairs = vec![
        ("txn_id".to_string(), quote(txn_id)),
        ("old_root".to_string(), quote(&hex32(&old_root))),
        ("new_root".to_string(), quote(&hex32(&new_root))),
        ("fingerprint".to_string(), quote(&hex32(&fingerprint))),
    ];
    canonical_object_pairs(&mut pairs).into_bytes()
}

fn quote(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len() + 2);
    out.push('"');
    out.push_str(raw);
    out.push('"');
    out
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

fn recheck_presence(
    request: &MutationRequest,
    view: &PublishedReadView,
) -> Result<(), MutationError> {
    let stamp = view.stamp();
    if request.world_id != stamp.world_id {
        return Err(MutationError::session_mismatch());
    }
    if request.generation != stamp.generation {
        return Err(MutationError::stale_epoch());
    }
    let plan = MutationPlanner::build(request)?;
    if plan.expected_world_revision() != stamp.world_revision {
        return Err(MutationError::revision_conflict());
    }
    for chunk_id in plan.chunk_ids() {
        match view.directory().lookup(chunk_id) {
            Ok(Some(slot)) => match slot.presence() {
                "Ready" => {}
                "NotLoaded" | "Pending" | "Unavailable" => {
                    debug_assert!(CHUNK_PRESENCE.contains(&slot.presence()));
                    return Err(MutationError::chunk_unavailable());
                }
                other => {
                    let _ = CHUNK_PRESENCE.contains(&other);
                    return Err(MutationError::invalid_handle());
                }
            },
            Ok(None) => return Err(MutationError::chunk_unavailable()),
            Err(err) if err.error_id() == "CoordinateOutOfBounds" => continue,
            Err(err) => return Err(MutationError::from_chunk(err)),
        }
    }
    Ok(())
}
