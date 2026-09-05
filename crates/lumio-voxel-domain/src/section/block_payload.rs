//! Full and Delta Section payload envelopes.

use super::{SectionEncoding, SectionError, SectionId, SectionStorage};
use crate::block::{BlockId, CellOffset};
use lumio_voxel_contracts::sha256;
use lumio_voxel_contracts::voxel_world as vw;
use std::sync::Arc;

const CELL_COUNT: usize = vw::SECTION_CELLS as usize;
const BLOCK_BYTES: usize = (vw::BLOCK_ID_WIDTH / 8) as usize;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DeltaEntry {
    offset: CellOffset,
    block_id: BlockId,
}

impl DeltaEntry {
    pub const fn new(offset: CellOffset, block_id: BlockId) -> Self {
        Self { offset, block_id }
    }

    pub const fn offset(self) -> CellOffset {
        self.offset
    }

    pub const fn block_id(self) -> BlockId {
        self.block_id
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SectionPayloadEnvelope {
    section_key: String,
    section_revision: u64,
    encoding: SectionEncoding,
    payload_length: u32,
    payload_sha256: [u8; 32],
    base_section_revision: Option<u64>,
    payload: Arc<[u8]>,
}

impl SectionPayloadEnvelope {
    pub fn encode_full(
        section_id: SectionId,
        section_revision: u64,
        storage: &SectionStorage,
    ) -> Self {
        let storage = storage.compacted();
        let encoding = storage.encoding();
        let payload = encode_storage(&storage);
        Self::outgoing(section_id, section_revision, encoding, None, payload)
    }

    pub fn encode_delta(
        section_id: SectionId,
        section_revision: u64,
        base_section_revision: u64,
        entries: &[DeltaEntry],
    ) -> Self {
        let mut payload = Vec::with_capacity(entries.len() * 6);
        for entry in entries {
            payload.extend_from_slice(&entry.offset.raw().to_le_bytes());
            payload.extend_from_slice(&entry.block_id.raw().to_le_bytes());
        }
        Self::outgoing(
            section_id,
            section_revision,
            SectionEncoding::Delta,
            Some(base_section_revision),
            payload,
        )
    }

    pub fn from_wire_parts(
        section_key: impl Into<String>,
        section_revision: u64,
        encoding: SectionEncoding,
        payload_length: u32,
        payload_sha256: [u8; 32],
        base_section_revision: Option<u64>,
        payload: impl Into<Vec<u8>>,
    ) -> Self {
        Self {
            section_key: section_key.into(),
            section_revision,
            encoding,
            payload_length,
            payload_sha256,
            base_section_revision,
            payload: Arc::from(payload.into()),
        }
    }

    pub fn section_key(&self) -> &str {
        &self.section_key
    }

    pub const fn section_revision(&self) -> u64 {
        self.section_revision
    }

    pub const fn encoding(&self) -> SectionEncoding {
        self.encoding
    }

    pub const fn payload_length(&self) -> u32 {
        self.payload_length
    }

    pub const fn payload_sha256(&self) -> [u8; 32] {
        self.payload_sha256
    }

    pub const fn base_section_revision(&self) -> Option<u64> {
        self.base_section_revision
    }

    pub fn payload(&self) -> &[u8] {
        &self.payload
    }

    /// Verify payload bytes before parsing the key, encoding body, or Delta entries.
    pub fn decode(
        &self,
        baseline: Option<(&SectionStorage, u64)>,
    ) -> Result<DecodedSectionPayload, SectionError> {
        if sha256(&self.payload) != self.payload_sha256 {
            return Err(SectionError::section_digest_mismatch());
        }
        if self.payload.len() != self.payload_length as usize {
            return Err(SectionError::contract_violation(
                vw::SECTION_ENCODING_MISMATCH,
            ));
        }

        let section_id = SectionId::parse(&self.section_key)?;
        let storage = match self.encoding {
            SectionEncoding::Uniform | SectionEncoding::Palette | SectionEncoding::Raw => {
                if self.base_section_revision.is_some() {
                    return Err(SectionError::contract_violation(
                        vw::SECTION_ENCODING_MISMATCH,
                    ));
                }
                if let Some((_, current_revision)) = baseline
                    && self.section_revision <= current_revision
                {
                    return Err(SectionError::contract_violation(vw::STALE_SECTION_REVISION));
                }
                decode_full(self.encoding, &self.payload)?
            }
            SectionEncoding::Delta => {
                let Some((baseline, current_revision)) = baseline else {
                    return Err(SectionError::contract_violation(
                        vw::DELTA_USED_FOR_FIRST_DELIVERY,
                    ));
                };
                if self.base_section_revision != Some(current_revision) {
                    return Err(SectionError::contract_violation(
                        vw::DELTA_BASE_REVISION_MISMATCH,
                    ));
                }
                if self.section_revision <= current_revision {
                    return Err(SectionError::contract_violation(
                        vw::DELTA_BASE_REVISION_MISMATCH,
                    ));
                }
                apply_delta(baseline, &self.payload)?
            }
        };
        Ok(DecodedSectionPayload {
            section_id,
            section_revision: self.section_revision,
            storage,
        })
    }

    fn outgoing(
        section_id: SectionId,
        section_revision: u64,
        encoding: SectionEncoding,
        base_section_revision: Option<u64>,
        payload: Vec<u8>,
    ) -> Self {
        let payload_length = payload
            .len()
            .try_into()
            .expect("a Section payload is bounded well below u32::MAX");
        let payload_sha256 = sha256(&payload);
        Self {
            section_key: section_id.key(),
            section_revision,
            encoding,
            payload_length,
            payload_sha256,
            base_section_revision,
            payload: Arc::from(payload),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecodedSectionPayload {
    section_id: SectionId,
    section_revision: u64,
    storage: SectionStorage,
}

impl DecodedSectionPayload {
    pub const fn section_id(&self) -> &SectionId {
        &self.section_id
    }

    pub const fn section_revision(&self) -> u64 {
        self.section_revision
    }

    pub const fn storage(&self) -> &SectionStorage {
        &self.storage
    }
}

fn encode_storage(storage: &SectionStorage) -> Vec<u8> {
    storage.encoded_payload()
}

fn decode_full(encoding: SectionEncoding, payload: &[u8]) -> Result<SectionStorage, SectionError> {
    match encoding {
        SectionEncoding::Uniform => {
            if payload.len() != BLOCK_BYTES {
                return Err(encoding_mismatch());
            }
            Ok(SectionStorage::uniform(BlockId::from_raw(read_u32(
                payload,
            ))))
        }
        SectionEncoding::Palette => decode_palette(payload),
        SectionEncoding::Raw => decode_raw(payload),
        SectionEncoding::Delta => unreachable!("Delta is dispatched before full decoding"),
    }
}

fn decode_palette(payload: &[u8]) -> Result<SectionStorage, SectionError> {
    if payload.len() < 2 {
        return Err(encoding_mismatch());
    }
    let count = usize::from(u16::from_le_bytes([payload[0], payload[1]]));
    if count > vw::PALETTE_MAX_ENTRIES as usize {
        return Err(SectionError::contract_violation(vw::PALETTE_OVERFLOW));
    }
    if count < 2 {
        return Err(encoding_mismatch());
    }
    let table_end = 2 + count * BLOCK_BYTES;
    if payload.len() != table_end + CELL_COUNT {
        return Err(encoding_mismatch());
    }

    let mut entries = Vec::with_capacity(count);
    for bytes in payload[2..table_end].as_chunks::<BLOCK_BYTES>().0 {
        let block_id = BlockId::from_raw(read_u32(bytes));
        if entries.contains(&block_id) {
            return Err(encoding_mismatch());
        }
        entries.push(block_id);
    }

    let indices = &payload[table_end..];
    let mut live = [0_u64; 4];
    let mut cells = Vec::with_capacity(CELL_COUNT);
    for index in indices.iter().copied() {
        let index = usize::from(index);
        if index >= entries.len() {
            return Err(encoding_mismatch());
        }
        live[index / 64] |= 1_u64 << (index % 64);
        cells.push(entries[index]);
    }
    if (0..count).any(|slot| live[slot / 64] & (1_u64 << (slot % 64)) == 0) {
        return Err(SectionError::contract_violation(
            vw::DEAD_PALETTE_ENTRY_IN_PAYLOAD,
        ));
    }
    let storage = SectionStorage::from_cells(&cells)?;
    if storage.encoding() != SectionEncoding::Palette {
        return Err(encoding_mismatch());
    }
    Ok(storage)
}

fn decode_raw(payload: &[u8]) -> Result<SectionStorage, SectionError> {
    if payload.len() != CELL_COUNT * BLOCK_BYTES {
        return Err(encoding_mismatch());
    }
    let cells: Vec<_> = payload
        .as_chunks::<BLOCK_BYTES>()
        .0
        .iter()
        .map(|bytes| BlockId::from_raw(read_u32(bytes)))
        .collect();
    let storage = SectionStorage::from_cells(&cells)?;
    if storage.encoding() != SectionEncoding::Raw {
        return Err(encoding_mismatch());
    }
    Ok(storage)
}

fn apply_delta(baseline: &SectionStorage, payload: &[u8]) -> Result<SectionStorage, SectionError> {
    if !payload.len().is_multiple_of(6) {
        return Err(encoding_mismatch());
    }
    let mut storage = baseline.clone();
    for entry in payload.as_chunks::<6>().0 {
        let offset = u16::from_le_bytes([entry[0], entry[1]]);
        let offset = CellOffset::new(offset)
            .map_err(|_| SectionError::contract_violation(vw::CELL_OFFSET_OUT_OF_RANGE))?;
        let block_id = BlockId::from_raw(read_u32(&entry[2..]));
        storage.write(offset, block_id);
    }
    Ok(storage)
}

fn read_u32(bytes: &[u8]) -> u32 {
    u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}

fn encoding_mismatch() -> SectionError {
    SectionError::contract_violation(vw::SECTION_ENCODING_MISMATCH)
}
