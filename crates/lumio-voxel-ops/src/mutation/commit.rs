//! Recheck a prepared batch, overlay one unpublished root, then seal.

#![forbid(unsafe_code)]

use super::commit_finalize::publish_once_and_finalize;
use super::fingerprint::{MUTATION_RECEIPT_SCHEMA, MutationRequest};
use super::plan::MutationPlanner;
use super::preconditions::MutationError;
use super::prepared_token::PreparedMutation;
use super::receipt_ledger::{LookupOutcome, ReceiptEvidence, ReceiptLedger};
use crate::canonical::CanonicalObject;
use lumio_voxel_contracts::voxel_world::SECTION_PRESENCE;
use lumio_voxel_contracts::{SCHEMA_IDS, sha256};
use lumio_voxel_domain::publication::{
    PublicationAuthority, PublishedReadView, PublishedStateRoot,
};
use lumio_voxel_domain::revision::{
    GeneratedRevisionStamp, SectionRevision, WorldRevision, to_generated_stamp,
};
use lumio_voxel_domain::section::{
    SectionDirectoryBuilder, SectionDirectoryRoot, SectionError, SectionReplacement,
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
    // Idempotent replay must not require the unpublished base. Lookup first.
    match ledger
        .lookup(prepared.request())
        .map_err(MutationError::from_ledger)?
    {
        LookupOutcome::Duplicate { receipt } => {
            let receipt = prepared
                .replay_receipt()
                .unwrap_or(receipt.as_slice())
                .to_vec();
            return duplicate_receipt(&prepared, receipt);
        }
        LookupOutcome::Vacant => return Err(MutationError::invalid_handle()),
        LookupOutcome::InFlight => {}
    }
    let view = authority.capture();
    recheck_prepared(&prepared, &view, ledger)?;
    recheck_presence(prepared.request(), &view)?;

    let plan = MutationPlanner::build(prepared.request())?;
    let replacement = prepared.replacement().clone();
    let overlay_ids = overlay_ids(view.stamp().section_revision_set.keys(), plan.section_ids());
    let new_dir = overlay_directory(&view, &replacement, &overlay_ids)?;
    let new_stamp = build_stamp(&prepared, &view, &replacement, &overlay_ids)?;
    let new_root = PublishedStateRoot::new(new_stamp, new_dir, prepared.dirty().clone());
    let world = world_revision(prepared.target_world_revision())?;
    let mut publication = authority
        .prepare(world, new_root, replacement)
        .map_err(MutationError::from_publish)?;
    // `prepare` folds the replacement digest into the cut identity; the value computed
    // before `prepare` is not what `publish_once` will make visible.
    let new_identity = publication.new_root_identity();

    let txn_id = prepared.txn_id().to_string();
    let old_root = prepared.base_identity();
    // Encode the receipt before sealing. The seal is the point of no return, and an
    // encoding refusal after it would leave a sealed cut with no receipt.
    let receipt_bytes = receipt_bytes(
        &txn_id,
        old_root,
        new_identity,
        prepared.fingerprint().hash().0,
    )?;
    let token = publication.seal().map_err(MutationError::from_publish)?;

    let request = prepared.request().clone();
    let evidence = CommitEvidence {
        old_root,
        new_root: new_identity,
        txn_id: txn_id.clone(),
        receipt_hash: sha256(&receipt_bytes),
    };

    let receipt =
        publish_once_and_finalize(authority, ledger, token, request.clone(), receipt_bytes)?;
    ledger.record_evidence_after_publish(
        &request,
        ReceiptEvidence {
            old_root: evidence.old_root,
            new_root: evidence.new_root,
        },
    );
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
    receipt: Vec<u8>,
) -> Result<GeneratedMutationReceipt, MutationError> {
    let txn_id = prepared.txn_id().to_string();
    let (old_root, new_root) = prepared
        .replay_evidence()
        .map(|evidence| (evidence.old_root, evidence.new_root))
        .unwrap_or_else(|| (prepared.base_identity(), prepared.base_identity()));
    let receipt_hash = sha256(&receipt);
    Ok(GeneratedMutationReceipt {
        txn_id: txn_id.clone(),
        receipt,
        evidence: CommitEvidence {
            old_root,
            new_root,
            txn_id,
            receipt_hash,
        },
    })
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
    replacement: &SectionReplacement,
    overlay_ids: &BTreeSet<String>,
) -> Result<SectionDirectoryRoot, MutationError> {
    let mut builder = SectionDirectoryBuilder::new();
    for id in overlay_ids {
        let slot = match replacement
            .set()
            .get(id)
            .map_err(MutationError::from_section)?
            .cloned()
        {
            Some(slot) => slot,
            None => match view
                .directory()
                .lookup(id)
                .map_err(MutationError::from_section)?
                .cloned()
            {
                Some(slot) => slot,
                None => continue,
            },
        };
        builder
            .insert(id, slot)
            .map_err(MutationError::from_section)?;
    }
    Ok(builder.freeze())
}

fn build_stamp(
    prepared: &PreparedMutation,
    view: &PublishedReadView,
    replacement: &SectionReplacement,
    overlay_ids: &BTreeSet<String>,
) -> Result<GeneratedRevisionStamp, MutationError> {
    let stamp = view.stamp();
    let mut pairs = Vec::new();
    for id in overlay_ids {
        let old = stamp
            .section_revision_set
            .get(id)
            .copied()
            .unwrap_or(stamp.world_revision);
        let replaced = replacement
            .set()
            .get(id)
            .map_err(MutationError::from_section)?
            .is_some();
        let rev_n = if replaced {
            old.checked_add(1)
                .ok_or_else(MutationError::invalid_handle)?
        } else {
            old
        };
        pairs.push((id.clone(), section_revision(rev_n)?));
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
    Ok(WorldRevision::from_raw(n))
}

fn section_revision(n: u64) -> Result<SectionRevision, MutationError> {
    Ok(SectionRevision::from_raw(n))
}

fn receipt_bytes(
    txn_id: &str,
    old_root: [u8; 32],
    new_root: [u8; 32],
    fingerprint: [u8; 32],
) -> Result<Vec<u8>, MutationError> {
    let mut object = CanonicalObject::new();
    for (key, value) in [
        ("txn_id", txn_id.to_string()),
        ("old_root", hex32(&old_root)),
        ("new_root", hex32(&new_root)),
        ("fingerprint", hex32(&fingerprint)),
    ] {
        object
            .insert_text(key, value)
            .map_err(|_| MutationError::invalid_handle())?;
    }
    Ok(object.encode_bytes())
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
    for section_id in plan.section_ids() {
        let edits = plan
            .section_edits()
            .get(section_id)
            .ok_or_else(MutationError::unstructured_mutation_entry)?;
        let current = stamp
            .section_revision_set
            .get(section_id)
            .copied()
            .unwrap_or(stamp.world_revision);
        if edits
            .entries()
            .iter()
            .any(|entry| entry.expected_section_revision != current)
        {
            return Err(MutationError::stale_section_revision());
        }
    }
    for section_id in plan.section_ids() {
        match view.directory().lookup(section_id) {
            Ok(Some(slot)) => match slot.presence() {
                "Ready" => {}
                "Unchanged" | "Pending" | "Unavailable" => {
                    debug_assert!(SECTION_PRESENCE.contains(&slot.presence()));
                    return Err(MutationError::section_unavailable());
                }
                other => {
                    let _ = SECTION_PRESENCE.contains(&other);
                    return Err(MutationError::invalid_handle());
                }
            },
            Ok(None) => return Err(MutationError::section_unavailable()),
            // 占位/单元格 key 不是 Section 键,跳过;它们在私有 stage 路径上失败。
            Err(SectionError::Key(_)) => continue,
            Err(err) => return Err(MutationError::from_section(err)),
        }
    }
    Ok(())
}
