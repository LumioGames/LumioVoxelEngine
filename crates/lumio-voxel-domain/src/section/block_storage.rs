//! Copy-on-write Uniform, Palette, and Raw block storage for one Section.

use super::SectionError;
use crate::block::{BlockId, CellOffset, WorldY};
use crate::key::SectionId;
use lumio_voxel_contracts::sha256;
use lumio_voxel_contracts::voxel_world as vw;
use std::mem::size_of;
use std::sync::Arc;

const CELL_COUNT: usize = vw::SECTION_CELLS as usize;
const PALETTE_CAPACITY: usize = vw::PALETTE_MAX_ENTRIES as usize;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SectionEncoding {
    Uniform,
    Palette,
    Raw,
    Delta,
}

impl SectionEncoding {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Uniform => "Uniform",
            Self::Palette => "Palette",
            Self::Raw => "Raw",
            Self::Delta => "Delta",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaletteCapacityAction {
    NotNeeded,
    ReusedDeadSlot { cell_index_changed: bool },
    EscalatedAfterFullScan,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WriteOutcome {
    encoding: SectionEncoding,
    palette_capacity_action: PaletteCapacityAction,
}

impl WriteOutcome {
    pub const fn encoding(self) -> SectionEncoding {
        self.encoding
    }

    pub const fn palette_capacity_action(self) -> PaletteCapacityAction {
        self.palette_capacity_action
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum StorageState {
    Uniform(BlockId),
    Palette {
        entries: Vec<BlockId>,
        cells: Box<[u8; CELL_COUNT]>,
    },
    Raw(Box<[BlockId; CELL_COUNT]>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SectionStorage {
    state: Arc<StorageState>,
}

/// Resolves canonical storage for a Section whose published slot is an
/// `Unchanged` short ticket and therefore carries no payload bytes.
pub trait SectionStorageResolver {
    fn resolve(&self, section_id: &SectionId) -> Option<SectionStorage>;
}

impl<F> SectionStorageResolver for F
where
    F: Fn(&SectionId) -> Option<SectionStorage>,
{
    fn resolve(&self, section_id: &SectionId) -> Option<SectionStorage> {
        self(section_id)
    }
}

impl SectionStorage {
    pub fn uniform(block_id: BlockId) -> Self {
        Self {
            state: Arc::new(StorageState::Uniform(block_id)),
        }
    }

    pub fn from_cells(cells: &[BlockId]) -> Result<Self, SectionError> {
        if cells.len() != CELL_COUNT {
            return Err(SectionError::contract_violation(
                vw::SECTION_ENCODING_MISMATCH,
            ));
        }
        Ok(Self {
            state: Arc::new(optimal_state(cells)),
        })
    }

    /// Conformance guard for storage adapters that implement their own Palette transition.
    pub fn validate_raw_escalation(reclamation_performed: bool) -> Result<(), SectionError> {
        if reclamation_performed {
            Ok(())
        } else {
            Err(SectionError::contract_violation(
                vw::PALETTE_RECLAIM_BEFORE_ESCALATION,
            ))
        }
    }

    pub fn encoding(&self) -> SectionEncoding {
        match self.state.as_ref() {
            StorageState::Uniform(_) => SectionEncoding::Uniform,
            StorageState::Palette { .. } => SectionEncoding::Palette,
            StorageState::Raw(_) => SectionEncoding::Raw,
        }
    }

    pub fn resident_cell_bytes(&self) -> usize {
        match self.state.as_ref() {
            StorageState::Uniform(_) => 0,
            StorageState::Palette { cells, .. } => cells.len(),
            StorageState::Raw(cells) => cells.len() * size_of::<BlockId>(),
        }
    }

    pub fn palette_entry_count(&self) -> Option<usize> {
        match self.state.as_ref() {
            StorageState::Palette { entries, .. } => Some(entries.len()),
            StorageState::Uniform(_) | StorageState::Raw(_) => None,
        }
    }

    /// Canonical full-payload bytes for this storage state. The representation is
    /// compacted first so dead palette entries cannot affect identity.
    pub(crate) fn encoded_payload(&self) -> Vec<u8> {
        let storage = self.compacted();
        match storage.state() {
            StorageState::Uniform(block_id) => block_id.raw().to_le_bytes().to_vec(),
            StorageState::Palette { entries, cells } => {
                let mut payload = Vec::with_capacity(2 + entries.len() * 4 + CELL_COUNT);
                payload.extend_from_slice(&(entries.len() as u16).to_le_bytes());
                for entry in entries {
                    payload.extend_from_slice(&entry.raw().to_le_bytes());
                }
                payload.extend_from_slice(cells.as_slice());
                payload
            }
            StorageState::Raw(cells) => {
                let mut payload = Vec::with_capacity(CELL_COUNT * size_of::<BlockId>());
                for block_id in cells.iter() {
                    payload.extend_from_slice(&block_id.raw().to_le_bytes());
                }
                payload
            }
        }
    }

    pub(crate) fn identity_digest(&self) -> [u8; 32] {
        sha256(&self.encoded_payload())
    }

    pub fn read(&self, offset: CellOffset) -> BlockId {
        let index = usize::from(offset.raw());
        match self.state.as_ref() {
            StorageState::Uniform(block_id) => *block_id,
            StorageState::Palette { entries, cells } => entries[usize::from(cells[index])],
            StorageState::Raw(cells) => cells[index],
        }
    }

    pub fn read_world(&self, world_x: i32, world_y: WorldY, world_z: i32) -> BlockId {
        self.read(CellOffset::from_world(world_x, world_y, world_z))
    }

    pub fn write(&mut self, offset: CellOffset, block_id: BlockId) -> WriteOutcome {
        if self.read(offset) == block_id {
            return WriteOutcome {
                encoding: self.encoding(),
                palette_capacity_action: PaletteCapacityAction::NotNeeded,
            };
        }
        let index = usize::from(offset.raw());
        let state = Arc::make_mut(&mut self.state);
        let action = match state {
            StorageState::Uniform(current) => {
                let old = *current;
                let mut cells = Box::new([0; CELL_COUNT]);
                cells[index] = 1;
                *state = StorageState::Palette {
                    entries: vec![old, block_id],
                    cells,
                };
                PaletteCapacityAction::NotNeeded
            }
            StorageState::Palette { entries, cells } => {
                match write_palette(entries, cells, index, block_id) {
                    PaletteWrite::Complete(action) => action,
                    PaletteWrite::Escalate(raw) => {
                        *state = StorageState::Raw(raw);
                        PaletteCapacityAction::EscalatedAfterFullScan
                    }
                }
            }
            StorageState::Raw(cells) => {
                cells[index] = block_id;
                if let Some(compacted) = palette_or_uniform(cells.as_slice()) {
                    *state = compacted;
                }
                PaletteCapacityAction::NotNeeded
            }
        };
        WriteOutcome {
            encoding: self.encoding(),
            palette_capacity_action: action,
        }
    }

    pub fn write_world(
        &mut self,
        world_x: i32,
        world_y: WorldY,
        world_z: i32,
        block_id: BlockId,
    ) -> WriteOutcome {
        self.write(CellOffset::from_world(world_x, world_y, world_z), block_id)
    }

    pub(super) fn compacted(&self) -> Self {
        let cells = self.expanded_cells();
        Self {
            state: Arc::new(optimal_state(cells.as_slice())),
        }
    }

    pub(super) fn state(&self) -> &StorageState {
        self.state.as_ref()
    }

    fn expanded_cells(&self) -> Box<[BlockId; CELL_COUNT]> {
        let mut expanded = Box::new([BlockId::from_raw(0); CELL_COUNT]);
        match self.state.as_ref() {
            StorageState::Uniform(block_id) => expanded.fill(*block_id),
            StorageState::Palette { entries, cells } => {
                for (target, palette_index) in expanded.iter_mut().zip(cells.iter()) {
                    *target = entries[usize::from(*palette_index)];
                }
            }
            StorageState::Raw(cells) => expanded.copy_from_slice(cells.as_slice()),
        }
        expanded
    }
}

enum PaletteWrite {
    Complete(PaletteCapacityAction),
    Escalate(Box<[BlockId; CELL_COUNT]>),
}

fn write_palette(
    entries: &mut Vec<BlockId>,
    cells: &mut Box<[u8; CELL_COUNT]>,
    cell_index: usize,
    block_id: BlockId,
) -> PaletteWrite {
    let old_slot = usize::from(cells[cell_index]);
    debug_assert_ne!(entries[old_slot], block_id);
    if let Some(slot) = entries.iter().position(|entry| *entry == block_id) {
        cells[cell_index] = slot as u8;
        return PaletteWrite::Complete(PaletteCapacityAction::NotNeeded);
    }
    if entries.len() < PALETTE_CAPACITY {
        entries.push(block_id);
        cells[cell_index] = (entries.len() - 1) as u8;
        return PaletteWrite::Complete(PaletteCapacityAction::NotNeeded);
    }

    let live = live_slots_excluding(cells, cell_index);
    let dead_slot = if !is_slot_live(&live, old_slot) {
        Some(old_slot)
    } else {
        (0..PALETTE_CAPACITY).find(|slot| !is_slot_live(&live, *slot))
    };
    if let Some(slot) = dead_slot {
        entries[slot] = block_id;
        let cell_index_changed = cells[cell_index] != slot as u8;
        cells[cell_index] = slot as u8;
        return PaletteWrite::Complete(PaletteCapacityAction::ReusedDeadSlot {
            cell_index_changed,
        });
    }

    SectionStorage::validate_raw_escalation(true)
        .expect("the required live-slot scan completed before Raw escalation");
    let mut raw = Box::new([BlockId::from_raw(0); CELL_COUNT]);
    for (target, palette_index) in raw.iter_mut().zip(cells.iter()) {
        *target = entries[usize::from(*palette_index)];
    }
    raw[cell_index] = block_id;
    PaletteWrite::Escalate(raw)
}

fn live_slots_excluding(cells: &[u8; CELL_COUNT], excluded_cell: usize) -> [u64; 4] {
    let mut live = [0_u64; 4];
    for (cell, slot) in cells.iter().copied().enumerate() {
        if cell == excluded_cell {
            continue;
        }
        let slot = usize::from(slot);
        live[slot / 64] |= 1_u64 << (slot % 64);
    }
    live
}

fn is_slot_live(live: &[u64; 4], slot: usize) -> bool {
    live[slot / 64] & (1_u64 << (slot % 64)) != 0
}

fn optimal_state(cells: &[BlockId]) -> StorageState {
    palette_or_uniform(cells).unwrap_or_else(|| {
        let mut raw = Box::new([BlockId::from_raw(0); CELL_COUNT]);
        raw.copy_from_slice(cells);
        StorageState::Raw(raw)
    })
}

fn palette_or_uniform(cells: &[BlockId]) -> Option<StorageState> {
    let mut entries = Vec::with_capacity(PALETTE_CAPACITY);
    let mut indices = Box::new([0; CELL_COUNT]);
    for (cell, block_id) in cells.iter().copied().enumerate() {
        let slot = match entries.iter().position(|entry| *entry == block_id) {
            Some(slot) => slot,
            None if entries.len() < PALETTE_CAPACITY => {
                entries.push(block_id);
                entries.len() - 1
            }
            None => return None,
        };
        indices[cell] = slot as u8;
    }
    if entries.len() == 1 {
        Some(StorageState::Uniform(entries[0]))
    } else {
        Some(StorageState::Palette {
            entries,
            cells: indices,
        })
    }
}
