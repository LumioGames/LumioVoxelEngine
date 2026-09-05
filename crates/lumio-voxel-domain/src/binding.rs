//! Sparse block/entity composition for one Section.
//!
//! A voxel cell stores its `BlockId`; an entity-backed cell additionally gets one
//! sparse reference to a live ECS entity. Entity business data is deliberately not
//! represented by this module. The policy is injected because the public contract
//! does not select an ECS type-number mapping.

#![forbid(unsafe_code)]

use crate::block::{BlockError, BlockId, BlockType, CellOffset};
use crate::section::{SectionEncoding, SectionError, SectionPayloadEnvelope, SectionStorage};
use lumio_voxel_contracts::voxel_world as vw;
use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};

/// Network identity used by the sparse reference table.
pub type NetEntityId = String;
pub type EntityId = NetEntityId;

pub type SparseBindingTable = SparseReferenceTable;
pub type BindingReferenceTable = SparseReferenceTable;
pub type SparseEntityBindingTable = SparseReferenceTable;

/// ECS metadata needed to validate a binding. Business components are owned by ECS
/// and intentionally have no representation here.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EntityRecord {
    id: NetEntityId,
    entity_type: String,
    alive: bool,
}

impl EntityRecord {
    pub fn new(id: impl ToString, entity_type: impl ToString) -> Self {
        Self {
            id: id.to_string(),
            entity_type: entity_type.to_string(),
            alive: true,
        }
    }

    pub fn dead(id: impl ToString, entity_type: impl ToString) -> Self {
        Self {
            id: id.to_string(),
            entity_type: entity_type.to_string(),
            alive: false,
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn entity_type(&self) -> &str {
        &self.entity_type
    }

    pub const fn is_alive(&self) -> bool {
        self.alive
    }
}

/// Explicit block-to-ECS type policy. No reserved BlockType is selected here.
pub trait BlockEntityTypePolicy {
    /// Returns the ECS type expected for a block type, or `None` for plain blocks.
    fn entity_type_for(&self, block_type: BlockType) -> Option<String>;

    fn requires_entity(&self, block_type: BlockType) -> bool {
        self.entity_type_for(block_type).is_some()
    }

    fn matches(&self, block_type: BlockType, entity_type: &str) -> bool {
        self.entity_type_for(block_type).as_deref() == Some(entity_type)
    }
}

pub use BlockEntityTypePolicy as BlockEntityBindingPolicy;

impl<F> BlockEntityTypePolicy for F
where
    F: Fn(BlockType) -> Option<String>,
{
    fn entity_type_for(&self, block_type: BlockType) -> Option<String> {
        self(block_type)
    }
}

/// Small explicit policy useful for adapters and tests. Callers choose all mappings.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ExplicitBlockEntityTypePolicy {
    mappings: BTreeMap<BlockType, String>,
}

impl ExplicitBlockEntityTypePolicy {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with(mut self, block_type: BlockType, entity_type: impl ToString) -> Self {
        self.mappings.insert(block_type, entity_type.to_string());
        self
    }

    pub fn insert(&mut self, block_type: BlockType, entity_type: impl ToString) {
        self.mappings.insert(block_type, entity_type.to_string());
    }
}

impl BlockEntityTypePolicy for ExplicitBlockEntityTypePolicy {
    fn entity_type_for(&self, block_type: BlockType) -> Option<String> {
        self.mappings.get(&block_type).cloned()
    }
}

/// An input row used by decoders before sparse-shape validation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BindingReference {
    pub cell_offset: u16,
    pub entity_id: NetEntityId,
    /// Set only by malformed input. It is never retained by a valid table.
    pub business_data: Option<Vec<u8>>,
}

impl BindingReference {
    pub fn new(cell_offset: CellOffset, entity_id: impl ToString) -> Self {
        Self {
            cell_offset: cell_offset.raw(),
            entity_id: entity_id.to_string(),
            business_data: None,
        }
    }

    pub fn from_raw(cell_offset: u16, entity_id: impl ToString) -> Result<Self, BindingError> {
        let offset = CellOffset::new(cell_offset).map_err(BindingError::block)?;
        Ok(Self::new(offset, entity_id))
    }

    pub fn cell_offset(&self) -> Result<CellOffset, BindingError> {
        CellOffset::new(self.cell_offset).map_err(BindingError::block)
    }

    pub fn with_business_data(
        cell_offset: CellOffset,
        entity_id: impl ToString,
        business_data: impl Into<Vec<u8>>,
    ) -> Self {
        Self {
            cell_offset: cell_offset.raw(),
            entity_id: entity_id.to_string(),
            business_data: Some(business_data.into()),
        }
    }
}

/// Per-Section sparse `cellOffset -> NetEntityId` table.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SparseReferenceTable {
    entries: BTreeMap<CellOffset, NetEntityId>,
}

impl SparseReferenceTable {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_entries<I, E>(entries: I) -> Result<Self, BindingError>
    where
        I: IntoIterator<Item = (CellOffset, E)>,
        E: ToString,
    {
        let mut table = Self::new();
        for (offset, entity_id) in entries {
            table.insert(offset, entity_id)?;
        }
        Ok(table)
    }

    pub fn from_records<I>(records: I) -> Result<Self, BindingError>
    where
        I: IntoIterator<Item = BindingReference>,
    {
        let mut table = Self::new();
        for record in records {
            if record.business_data.is_some() {
                return Err(BindingError::not_sparse());
            }
            let offset = CellOffset::new(record.cell_offset).map_err(BindingError::block)?;
            table.insert(offset, record.entity_id)?;
        }
        Ok(table)
    }

    /// Dense cell arrays are explicitly outside the binding contract.
    pub fn from_dense<I, E>(entries: I) -> Result<Self, BindingError>
    where
        I: IntoIterator<Item = Option<E>>,
        E: ToString,
    {
        let _ = entries.into_iter().next();
        Err(BindingError::not_sparse())
    }

    pub fn insert(
        &mut self,
        cell_offset: CellOffset,
        entity_id: impl ToString,
    ) -> Result<(), BindingError> {
        let entity_id = entity_id.to_string();
        if entity_id.is_empty() || self.entries.contains_key(&cell_offset) {
            return Err(BindingError::not_sparse());
        }
        self.entries.insert(cell_offset, entity_id);
        Ok(())
    }

    pub fn insert_raw(
        &mut self,
        cell_offset: u16,
        entity_id: impl ToString,
    ) -> Result<(), BindingError> {
        let offset = CellOffset::new(cell_offset).map_err(BindingError::block)?;
        self.insert(offset, entity_id)
    }

    pub fn remove(&mut self, cell_offset: CellOffset) -> Option<NetEntityId> {
        self.entries.remove(&cell_offset)
    }

    pub fn get(&self, cell_offset: CellOffset) -> Option<&NetEntityId> {
        self.entries.get(&cell_offset)
    }

    pub fn contains(&self, cell_offset: CellOffset) -> bool {
        self.entries.contains_key(&cell_offset)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (CellOffset, &NetEntityId)> {
        self.entries
            .iter()
            .map(|(offset, entity_id)| (*offset, entity_id))
    }

    /// Canonical binding payload: count, then offset and entity-id only.
    pub fn to_wire_bytes(&self) -> Result<Vec<u8>, BindingError> {
        let count = u16::try_from(self.entries.len()).map_err(|_| BindingError::not_sparse())?;
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&count.to_le_bytes());
        for (offset, entity_id) in &self.entries {
            let id = entity_id.as_bytes();
            let len = u16::try_from(id.len()).map_err(|_| BindingError::not_sparse())?;
            bytes.extend_from_slice(&offset.raw().to_le_bytes());
            bytes.extend_from_slice(&len.to_le_bytes());
            bytes.extend_from_slice(id);
        }
        Ok(bytes)
    }

    pub fn from_wire_bytes(bytes: &[u8]) -> Result<Self, BindingError> {
        validate_wire_bytes(bytes)?;
        let mut cursor = 0_usize;
        let count = usize::from(read_u16(bytes, &mut cursor)?);
        let mut table = Self::new();
        for _ in 0..count {
            let offset =
                CellOffset::new(read_u16(bytes, &mut cursor)?).map_err(BindingError::block)?;
            let len = usize::from(read_u16(bytes, &mut cursor)?);
            let end = cursor
                .checked_add(len)
                .ok_or_else(BindingError::not_sparse)?;
            let id = std::str::from_utf8(
                bytes
                    .get(cursor..end)
                    .ok_or_else(BindingError::not_sparse)?,
            )
            .map_err(|_| BindingError::not_sparse())?;
            cursor = end;
            table.insert(offset, id.to_owned())?;
        }
        Ok(table)
    }

    pub fn validate_shape(&self) -> Result<(), BindingError> {
        for offset in self.entries.keys() {
            CellOffset::new(offset.raw()).map_err(BindingError::block)?;
        }
        Ok(())
    }
}

/// Immutable, validated composition of a Section and its entity-backed cells.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BindingState {
    section: SectionStorage,
    references: SparseReferenceTable,
    entities: BTreeMap<NetEntityId, EntityRecord>,
}

pub type BlockEntityBinding = BindingState;
pub type BindingSnapshot = BindingState;
pub type BindingComposition = BindingState;

impl BindingState {
    pub fn empty(section: SectionStorage) -> Self {
        Self {
            section,
            references: SparseReferenceTable::new(),
            entities: BTreeMap::new(),
        }
    }

    pub fn new<I>(
        section: SectionStorage,
        references: SparseReferenceTable,
        entities: I,
        policy: &impl BlockEntityTypePolicy,
    ) -> Result<Self, BindingError>
    where
        I: IntoIterator<Item = EntityRecord>,
    {
        let mut registry = BTreeMap::new();
        for entity in entities {
            if entity.id.is_empty() || registry.insert(entity.id.clone(), entity).is_some() {
                return Err(BindingError::orphan());
            }
        }
        let state = Self {
            section,
            references,
            entities: registry,
        };
        state.validate(policy)?;
        Ok(state)
    }

    pub fn validate(&self, policy: &impl BlockEntityTypePolicy) -> Result<(), BindingError> {
        self.references.validate_shape()?;

        for (offset, entity_id) in self.references.iter() {
            let block = self.section.read(offset);
            let expected = policy.entity_type_for(block.block_type());
            let Some(expected) = expected else {
                return Err(BindingError::orphan());
            };
            let Some(entity) = self.entities.get(entity_id) else {
                return Err(BindingError::missing());
            };
            if !entity.alive {
                return Err(BindingError::missing());
            }
            if entity.entity_type != expected {
                return Err(BindingError::type_mismatch());
            }
        }

        for offset_raw in 0..vw::SECTION_CELLS {
            let offset = CellOffset::new(offset_raw as u16).map_err(BindingError::block)?;
            let block = self.section.read(offset);
            let needs_entity = policy.requires_entity(block.block_type());
            let has_reference = self.references.contains(offset);
            if needs_entity && !has_reference {
                return Err(BindingError::missing());
            }
            if !needs_entity && has_reference {
                return Err(BindingError::orphan());
            }
        }

        for entity_id in self.entities.keys() {
            let occurrences = self
                .references
                .iter()
                .filter(|(_, reference)| *reference == entity_id)
                .count();
            if occurrences != 1 {
                return Err(BindingError::orphan());
            }
        }
        Ok(())
    }

    pub fn section(&self) -> &SectionStorage {
        &self.section
    }

    pub fn references(&self) -> &SparseReferenceTable {
        &self.references
    }

    pub fn entities(&self) -> &BTreeMap<NetEntityId, EntityRecord> {
        &self.entities
    }

    pub fn entity(&self, entity_id: &str) -> Option<&EntityRecord> {
        self.entities.get(entity_id)
    }

    pub fn references_payload(&self) -> Result<Vec<u8>, BindingError> {
        self.references.to_wire_bytes()
    }

    pub fn validate_wire_payload(bytes: &[u8]) -> Result<(), BindingError> {
        validate_wire_bytes(bytes)
    }

    /// Section payloads contain BlockIds only; business fields are rejected before
    /// a receiver can interpret or publish the bytes.
    pub fn validate_section_payload(payload: &SectionPayloadEnvelope) -> Result<(), BindingError> {
        match payload.decode(None) {
            Ok(_) => Ok(()),
            Err(error)
                if payload.encoding() == SectionEncoding::Delta
                    && error.error_id() == vw::DELTA_USED_FOR_FIRST_DELIVERY =>
            {
                let Some(base_revision) = payload.base_section_revision() else {
                    return Err(BindingError::business_data());
                };
                if payload.section_revision() <= base_revision {
                    return Err(BindingError::business_data());
                }
                validate_voxel_payload_bytes(payload.payload())
            }
            Err(_) => Err(BindingError::business_data()),
        }
    }
}

/// Unpublished COW transaction. All three halves carry one commit id.
#[derive(Clone, Debug)]
pub struct BindingTransaction {
    base: BindingState,
    block_changes: BTreeMap<CellOffset, BlockId>,
    reference_changes: BTreeMap<CellOffset, Option<NetEntityId>>,
    entity_changes: BTreeMap<NetEntityId, Option<EntityRecord>>,
    commit_id: Option<u64>,
    split: bool,
}

pub type BlockEntityBindingTransaction = BindingTransaction;

impl BindingTransaction {
    pub fn begin(base: &BindingState) -> Self {
        Self {
            base: base.clone(),
            block_changes: BTreeMap::new(),
            reference_changes: BTreeMap::new(),
            entity_changes: BTreeMap::new(),
            commit_id: None,
            split: false,
        }
    }

    pub fn place(
        &mut self,
        cell_offset: CellOffset,
        block_id: BlockId,
        entity: EntityRecord,
    ) -> Result<(), BindingError> {
        let previous_entity = self
            .reference_changes
            .get(&cell_offset)
            .and_then(|value| value.as_ref())
            .cloned()
            .or_else(|| self.base.references.get(cell_offset).cloned());
        let commit_id = self.commit_id.unwrap_or(0);
        self.set_block_commit(commit_id, cell_offset, block_id)?;
        self.set_reference_commit(commit_id, cell_offset, entity.id.clone())?;
        self.set_entity_commit(commit_id, entity)?;
        if let Some(previous_entity) = previous_entity
            && self
                .reference_changes
                .get(&cell_offset)
                .and_then(|value| value.as_ref())
                != Some(&previous_entity)
        {
            self.set_entity_removed_commit(commit_id, previous_entity)?;
        }
        Ok(())
    }

    pub fn remove(
        &mut self,
        cell_offset: CellOffset,
        replacement: BlockId,
    ) -> Result<(), BindingError> {
        let entity_id = self
            .reference_changes
            .get(&cell_offset)
            .and_then(|value| value.as_ref())
            .cloned()
            .or_else(|| self.base.references.get(cell_offset).cloned());
        let commit_id = self.commit_id.unwrap_or(0);
        self.set_block_commit(commit_id, cell_offset, replacement)?;
        self.set_reference_removed_commit(commit_id, cell_offset)?;
        if let Some(entity_id) = entity_id {
            self.set_entity_removed_commit(commit_id, entity_id)?;
        }
        Ok(())
    }

    pub fn set_block_commit(
        &mut self,
        commit_id: u64,
        cell_offset: CellOffset,
        block_id: BlockId,
    ) -> Result<(), BindingError> {
        self.observe_commit(commit_id)?;
        self.block_changes.insert(cell_offset, block_id);
        Ok(())
    }

    pub fn stage_block(
        &mut self,
        commit_id: u64,
        cell_offset: CellOffset,
        block_id: BlockId,
    ) -> Result<(), BindingError> {
        self.set_block_commit(commit_id, cell_offset, block_id)
    }

    pub fn set_reference_commit(
        &mut self,
        commit_id: u64,
        cell_offset: CellOffset,
        entity_id: impl ToString,
    ) -> Result<(), BindingError> {
        self.observe_commit(commit_id)?;
        self.reference_changes
            .insert(cell_offset, Some(entity_id.to_string()));
        Ok(())
    }

    pub fn stage_reference(
        &mut self,
        commit_id: u64,
        cell_offset: CellOffset,
        entity_id: impl ToString,
    ) -> Result<(), BindingError> {
        self.set_reference_commit(commit_id, cell_offset, entity_id)
    }

    pub fn set_reference_removed_commit(
        &mut self,
        commit_id: u64,
        cell_offset: CellOffset,
    ) -> Result<(), BindingError> {
        self.observe_commit(commit_id)?;
        self.reference_changes.insert(cell_offset, None);
        Ok(())
    }

    pub fn set_entity_commit(
        &mut self,
        commit_id: u64,
        entity: EntityRecord,
    ) -> Result<(), BindingError> {
        self.observe_commit(commit_id)?;
        self.entity_changes.insert(entity.id.clone(), Some(entity));
        Ok(())
    }

    pub fn stage_entity(
        &mut self,
        commit_id: u64,
        entity: EntityRecord,
    ) -> Result<(), BindingError> {
        self.set_entity_commit(commit_id, entity)
    }

    pub fn set_entity_removed_commit(
        &mut self,
        commit_id: u64,
        entity_id: impl ToString,
    ) -> Result<(), BindingError> {
        self.observe_commit(commit_id)?;
        self.entity_changes.insert(entity_id.to_string(), None);
        Ok(())
    }

    pub fn commit(self, policy: &impl BlockEntityTypePolicy) -> Result<BindingState, BindingError> {
        if self.split {
            return Err(BindingError::commit_split());
        }
        let mut section = self.base.section.clone();
        for (offset, block_id) in self.block_changes {
            section.write(offset, block_id);
        }
        let mut references = self.base.references.clone();
        for (offset, entity_id) in self.reference_changes {
            match entity_id {
                Some(entity_id) => {
                    references.remove(offset);
                    references.insert(offset, entity_id)?;
                }
                None => {
                    references.remove(offset);
                }
            }
        }
        let mut entities = self.base.entities.values().cloned().collect::<Vec<_>>();
        let mut registry = self.base.entities;
        for (entity_id, entity) in self.entity_changes {
            match entity {
                Some(entity) => {
                    registry.insert(entity_id, entity);
                }
                None => {
                    registry.remove(&entity_id);
                }
            }
        }
        entities.clear();
        entities.extend(registry.into_values());
        BindingState::new(section, references, entities, policy)
    }

    pub fn rollback(self) -> BindingState {
        self.base
    }

    fn observe_commit(&mut self, commit_id: u64) -> Result<(), BindingError> {
        match self.commit_id {
            Some(existing) if existing != commit_id => {
                self.split = true;
                Ok(())
            }
            Some(_) => Ok(()),
            None => {
                self.commit_id = Some(commit_id);
                Ok(())
            }
        }
    }
}

/// Immutable captured view of one published binding cut.
#[derive(Clone, Debug)]
pub struct BindingReadView {
    state: Arc<BindingState>,
}

impl BindingReadView {
    pub fn state(&self) -> &BindingState {
        &self.state
    }

    pub fn state_arc(&self) -> Arc<BindingState> {
        Arc::clone(&self.state)
    }
}

/// Atomic publisher for binding cuts. A transaction is validated before the Arc swap.
#[derive(Debug)]
pub struct BindingPublication {
    current: RwLock<Arc<BindingState>>,
}

pub type BindingPublisher = BindingPublication;

impl BindingPublication {
    pub fn new(initial: BindingState) -> Self {
        Self {
            current: RwLock::new(Arc::new(initial)),
        }
    }

    pub fn capture(&self) -> BindingReadView {
        let state = self
            .current
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone();
        BindingReadView { state }
    }

    pub fn publish(
        &self,
        transaction: BindingTransaction,
        policy: &impl BlockEntityTypePolicy,
    ) -> Result<BindingReadView, BindingError> {
        let mut current = self
            .current
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        // A transaction is bound to the captured base. Recheck that base while
        // holding the write lock so two concurrent transactions cannot publish
        // an older clone over a newer cut.
        if current.as_ref() != &transaction.base {
            return Err(BindingError::commit_split());
        }
        let next = Arc::new(transaction.commit(policy)?);
        *current = Arc::clone(&next);
        Ok(BindingReadView { state: next })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BindingError {
    Block(BlockError),
    Section(SectionError),
    EntityBindingMissing { error_id: &'static str },
    EntityBindingOrphan { error_id: &'static str },
    EntityBindingTypeMismatch { error_id: &'static str },
    EntityBindingNotSparse { error_id: &'static str },
    BusinessDataInPayload { error_id: &'static str },
    BindingCommitSplit { error_id: &'static str },
}

pub type BindingInvariantError = BindingError;

impl BindingError {
    pub fn error_id(&self) -> &'static str {
        match self {
            Self::Block(err) => err.code(),
            Self::Section(err) => err.error_id(),
            Self::EntityBindingMissing { error_id }
            | Self::EntityBindingOrphan { error_id }
            | Self::EntityBindingTypeMismatch { error_id }
            | Self::EntityBindingNotSparse { error_id }
            | Self::BusinessDataInPayload { error_id }
            | Self::BindingCommitSplit { error_id } => error_id,
        }
    }

    pub fn code(&self) -> &'static str {
        self.error_id()
    }

    fn contract(code: &'static str) -> &'static str {
        vw::intern_error_code(code).expect("binding error must be declared in the contract")
    }

    fn block(err: BlockError) -> Self {
        Self::Block(err)
    }

    fn missing() -> Self {
        Self::EntityBindingMissing {
            error_id: Self::contract("entity_binding_missing"),
        }
    }

    fn orphan() -> Self {
        Self::EntityBindingOrphan {
            error_id: Self::contract("entity_binding_orphan"),
        }
    }

    fn type_mismatch() -> Self {
        Self::EntityBindingTypeMismatch {
            error_id: Self::contract("entity_binding_type_mismatch"),
        }
    }

    fn not_sparse() -> Self {
        Self::EntityBindingNotSparse {
            error_id: Self::contract("entity_binding_not_sparse"),
        }
    }

    fn business_data() -> Self {
        Self::BusinessDataInPayload {
            error_id: Self::contract("business_data_in_payload"),
        }
    }

    fn commit_split() -> Self {
        Self::BindingCommitSplit {
            error_id: Self::contract("binding_commit_split"),
        }
    }
}

impl std::fmt::Display for BindingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.error_id())
    }
}

impl std::error::Error for BindingError {}

impl From<SectionError> for BindingError {
    fn from(err: SectionError) -> Self {
        Self::Section(err)
    }
}

fn validate_wire_bytes(bytes: &[u8]) -> Result<(), BindingError> {
    let mut cursor = 0_usize;
    let count = usize::from(read_u16(bytes, &mut cursor)?);
    if count > usize::from(vw::CELL_OFFSET_MAX) + 1 {
        return Err(BindingError::not_sparse());
    }
    let mut offsets = SparseReferenceTable::new();
    for _ in 0..count {
        let offset = CellOffset::new(read_u16(bytes, &mut cursor)?).map_err(BindingError::block)?;
        let len = usize::from(read_u16(bytes, &mut cursor)?);
        let end = cursor
            .checked_add(len)
            .ok_or_else(BindingError::not_sparse)?;
        let id = std::str::from_utf8(
            bytes
                .get(cursor..end)
                .ok_or_else(BindingError::not_sparse)?,
        )
        .map_err(|_| BindingError::not_sparse())?;
        cursor = end;
        offsets.insert(offset, id.to_owned())?;
    }
    if cursor != bytes.len() {
        return Err(BindingError::not_sparse());
    }
    Ok(())
}

fn validate_voxel_payload_bytes(bytes: &[u8]) -> Result<(), BindingError> {
    if bytes.len() == 4 || bytes.len() == vw::SECTION_CELLS as usize * 4 {
        return Ok(());
    }
    if bytes.len() >= 2 {
        let entries = usize::from(u16::from_le_bytes([bytes[0], bytes[1]]));
        if (2..=vw::PALETTE_MAX_ENTRIES as usize).contains(&entries)
            && bytes.len() == 2 + entries * 4 + vw::SECTION_CELLS as usize
        {
            let table_end = 2 + entries * 4;
            let table = bytes[2..table_end]
                .as_chunks::<4>()
                .0
                .iter()
                .map(|raw| u32::from_le_bytes(*raw))
                .collect::<Vec<_>>();
            if table
                .iter()
                .enumerate()
                .any(|(index, value)| table[..index].contains(value))
            {
                return Err(BindingError::business_data());
            }
            let mut live = vec![false; entries];
            for index in &bytes[table_end..] {
                let index = usize::from(*index);
                if index >= entries {
                    return Err(BindingError::business_data());
                }
                live[index] = true;
            }
            return if live.into_iter().all(|used| used) {
                Ok(())
            } else {
                Err(BindingError::business_data())
            };
        }
    }
    if bytes.len().is_multiple_of(6) && !bytes.is_empty() {
        for entry in bytes.as_chunks::<6>().0 {
            CellOffset::new(u16::from_le_bytes([entry[0], entry[1]]))
                .map_err(BindingError::block)?;
        }
        return Ok(());
    }
    Err(BindingError::business_data())
}

fn read_u16(bytes: &[u8], cursor: &mut usize) -> Result<u16, BindingError> {
    let end = cursor.checked_add(2).ok_or_else(BindingError::not_sparse)?;
    let pair = bytes
        .get(*cursor..end)
        .ok_or_else(BindingError::not_sparse)?;
    *cursor = end;
    Ok(u16::from_le_bytes([pair[0], pair[1]]))
}
