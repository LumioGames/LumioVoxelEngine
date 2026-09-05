//! Canonical request fingerprint over the Voxel-local canonical object encoding.

use crate::canonical::{CanonicalObject, DuplicateMember};
use lumio_voxel_contracts::{Hash256, SCHEMA_IDS, sha256};
use lumio_voxel_domain::block::{BlockId, CellOffset};

/// Generated schema this mapping wraps. Must stay in `SCHEMA_IDS`.
pub const MUTATION_RECEIPT_SCHEMA: &str = "voxel-mutation-receipt";

/// Member naming the encoding the fingerprint was taken over.
///
/// Without it the bytes carry no statement of which form produced them, so a later
/// format change would show up only as digests that quietly stopped matching. This
/// is deliberately not the Architecture form id: that form's member-name grammar
/// excludes `txn_id` and `c:0:0:0`, so claiming it here would be a false mark.
pub const CANONICAL_FORM_FIELD: &str = "canonicalForm";
pub const CANONICAL_FORM_ID: &str = "VoxelCanonicalObjectV1";

/// One authoritative block write mirroring the four members of `blockWrite.entry`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MutationEntry {
    pub section_key: String,
    pub cell_offset: CellOffset,
    pub block_id: BlockId,
    pub expected_section_revision: u64,
}

impl MutationEntry {
    pub fn new(
        section_key: impl Into<String>,
        cell_offset: CellOffset,
        block_id: BlockId,
        expected_section_revision: u64,
    ) -> Self {
        Self {
            section_key: section_key.into(),
            cell_offset,
            block_id,
            expected_section_revision,
        }
    }

    pub fn section_key(&self) -> &str {
        &self.section_key
    }

    pub const fn cell_offset(&self) -> CellOffset {
        self.cell_offset
    }

    pub const fn block_id(&self) -> BlockId {
        self.block_id
    }

    pub const fn expected_section_revision(&self) -> u64 {
        self.expected_section_revision
    }
}

pub type BlockWrite = MutationEntry;
pub type MutationWrite = MutationEntry;
pub type BlockWriteEntry = MutationEntry;
pub type MutationWriteEntry = MutationEntry;

/// Generated request identity plus structured block-write entries.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MutationRequest {
    pub txn_id: String,
    pub world_id: String,
    pub generation: u64,
    pub entries: Vec<MutationEntry>,
}

impl MutationRequest {
    pub fn new(
        txn_id: impl Into<String>,
        world_id: impl Into<String>,
        generation: u64,
        entries: impl Into<Vec<MutationEntry>>,
    ) -> Self {
        Self {
            txn_id: txn_id.into(),
            world_id: world_id.into(),
            generation,
            entries: entries.into(),
        }
    }

    pub fn from_entries(
        txn_id: impl Into<String>,
        world_id: impl Into<String>,
        generation: u64,
        entries: impl Into<Vec<MutationEntry>>,
    ) -> Self {
        Self::new(txn_id, world_id, generation, entries)
    }

    pub fn entries(&self) -> &[MutationEntry] {
        &self.entries
    }

    pub fn writes(&self) -> &[MutationEntry] {
        &self.entries
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RequestFingerprint {
    hash: Hash256,
}

impl RequestFingerprint {
    pub fn hash(self) -> Hash256 {
        self.hash
    }
}

/// Fingerprint covers the ordered, typed entries as well as request identity.
/// Entry order is significant because block writes use last-write-wins semantics.
pub fn canonical_fingerprint(
    request: &MutationRequest,
) -> Result<RequestFingerprint, DuplicateMember> {
    debug_assert!(SCHEMA_IDS.contains(&MUTATION_RECEIPT_SCHEMA));
    let mut object = CanonicalObject::new();
    object.insert_text("entries", encode_entries(&request.entries))?;
    object.insert_text(CANONICAL_FORM_FIELD, CANONICAL_FORM_ID)?;
    object.insert_text("txn_id", request.txn_id.clone())?;
    object.insert_text("world_id", request.world_id.clone())?;
    object.insert_uint("generation", request.generation)?;
    Ok(RequestFingerprint {
        hash: Hash256(sha256(object.encode().as_bytes())),
    })
}

fn encode_entries(entries: &[MutationEntry]) -> String {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&(entries.len() as u64).to_le_bytes());
    for entry in entries {
        let section = entry.section_key.as_bytes();
        bytes.extend_from_slice(&(section.len() as u64).to_le_bytes());
        bytes.extend_from_slice(section);
        bytes.extend_from_slice(&entry.cell_offset.raw().to_le_bytes());
        bytes.extend_from_slice(&entry.block_id.raw().to_le_bytes());
        bytes.extend_from_slice(&entry.expected_section_revision.to_le_bytes());
    }
    hex(&bytes)
}

fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[usize::from(byte >> 4)] as char);
        out.push(HEX[usize::from(byte & 0x0f)] as char);
    }
    out
}
