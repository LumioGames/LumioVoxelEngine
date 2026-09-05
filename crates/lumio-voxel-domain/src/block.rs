//! Contract-safe block identity and dynamic state layout primitives.
//!
//! Catalog, material, and behavior resolution belong to later decisions. This module only
//! provides the byte-level identity and generic state-field mechanics needed by Section storage.

#![forbid(unsafe_code)]

use lumio_voxel_contracts::voxel_world as vw;

pub use crate::key::{WorldY, WorldYError};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlockError {
    code: &'static str,
}

impl BlockError {
    fn contract(code: &'static str) -> Self {
        Self {
            code: vw::intern_error_code(code)
                .expect("block error must be declared by the contract"),
        }
    }

    pub const fn code(self) -> &'static str {
        self.code
    }
}

impl std::fmt::Display for BlockError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.code)
    }
}

impl std::error::Error for BlockError {}

/// The 24-bit type segment carried by a [`BlockId`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BlockType(u32);

impl BlockType {
    pub const fn new(raw: u32) -> Option<Self> {
        if raw <= vw::BLOCK_TYPE_MAX {
            Some(Self(raw))
        } else {
            None
        }
    }

    pub const fn raw(self) -> u32 {
        self.0
    }

    pub const fn scope(self) -> BlockScope {
        if self.0 & vw::BLOCK_TYPE_SCOPE_MASK == 0 {
            BlockScope::Global
        } else {
            BlockScope::RoomLocal
        }
    }

    pub const fn room_local_index(self) -> Option<u32> {
        if self.0 & vw::BLOCK_TYPE_SCOPE_MASK != 0 {
            Some(self.0 & !vw::BLOCK_TYPE_SCOPE_MASK)
        } else {
            None
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlockScope {
    Global,
    RoomLocal,
}

/// The 8-bit state segment carried by a [`BlockId`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BlockState(u8);

impl BlockState {
    pub const fn new(raw: u8) -> Self {
        Self(raw)
    }

    pub const fn raw(self) -> u8 {
        self.0
    }
}

/// Complete unsigned 32-bit block identity: `BlockType << 8 | BlockState`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BlockId(u32);

impl BlockId {
    pub const fn from_raw(raw: u32) -> Self {
        Self(raw)
    }

    pub const fn from_parts(block_type: BlockType, block_state: BlockState) -> Self {
        Self((block_type.raw() << vw::BLOCK_TYPE_SHIFT) | block_state.raw() as u32)
    }

    pub const fn raw(self) -> u32 {
        self.0
    }

    pub const fn block_type(self) -> BlockType {
        BlockType(self.0 >> vw::BLOCK_TYPE_SHIFT)
    }

    pub const fn block_state(self) -> BlockState {
        BlockState((self.0 & vw::BLOCK_STATE_MAX) as u8)
    }
}

/// The canonical y/z/x cell order inside one 16^3 Section.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CellOffset(u16);

impl CellOffset {
    pub fn new(raw: u16) -> Result<Self, BlockError> {
        if raw > vw::CELL_OFFSET_MAX {
            return Err(BlockError::contract(vw::CELL_OFFSET_OUT_OF_RANGE));
        }
        Ok(Self(raw))
    }

    pub const fn from_world(world_x: i32, world_y: WorldY, world_z: i32) -> Self {
        let x = (world_x & (vw::SECTION_EXTENT as i32 - 1)) as u16;
        let y = world_y.cell_y() as u16;
        let z = (world_z & (vw::SECTION_EXTENT as i32 - 1)) as u16;
        Self(
            y * vw::CELL_OFFSET_Y_STRIDE
                + z * vw::CELL_OFFSET_Z_STRIDE
                + x * vw::CELL_OFFSET_X_STRIDE,
        )
    }

    pub fn validate_for_world(
        raw: u16,
        world_x: i32,
        world_y: WorldY,
        world_z: i32,
    ) -> Result<Self, BlockError> {
        let offset = Self::new(raw)?;
        if offset != Self::from_world(world_x, world_y, world_z) {
            return Err(BlockError::contract(vw::CELL_OFFSET_OUT_OF_RANGE));
        }
        Ok(offset)
    }

    pub const fn raw(self) -> u16 {
        self.0
    }

    pub const fn y(self) -> u8 {
        ((self.0 >> 8) & 15) as u8
    }

    pub const fn z(self) -> u8 {
        ((self.0 >> 4) & 15) as u8
    }

    pub const fn x(self) -> u8 {
        (self.0 & 15) as u8
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StateFieldSpec<'a> {
    name: &'a str,
    width: u8,
}

impl<'a> StateFieldSpec<'a> {
    pub const fn new(name: &'a str, width: u8) -> Self {
        Self { name, width }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StateField {
    name: String,
    offset: u8,
    width: u8,
}

impl StateField {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub const fn offset(&self) -> u8 {
        self.offset
    }

    pub const fn width(&self) -> u8 {
        self.width
    }

    const fn value_mask(&self) -> u16 {
        (1_u16 << self.width) - 1
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StateLayoutError {
    EmptyName,
    DuplicateName,
    ZeroWidth,
    WidthOverflow,
}

impl std::fmt::Display for StateLayoutError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for StateLayoutError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StateAccessError {
    UnknownField,
    ValueOutOfRange,
}

impl std::fmt::Display for StateAccessError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for StateAccessError {}

/// Generic dynamic layout for the eight bits in a [`BlockState`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StateLayout {
    fields: Vec<StateField>,
}

impl StateLayout {
    pub const fn empty() -> Self {
        Self { fields: Vec::new() }
    }

    pub fn new(specs: &[StateFieldSpec<'_>]) -> Result<Self, StateLayoutError> {
        let mut fields = Vec::with_capacity(specs.len());
        let mut offset = 0_u8;
        for spec in specs {
            if spec.name.is_empty() {
                return Err(StateLayoutError::EmptyName);
            }
            if spec.width == 0 {
                return Err(StateLayoutError::ZeroWidth);
            }
            if fields
                .iter()
                .any(|field: &StateField| field.name == spec.name)
            {
                return Err(StateLayoutError::DuplicateName);
            }
            let end = offset
                .checked_add(spec.width)
                .ok_or(StateLayoutError::WidthOverflow)?;
            if end > vw::BLOCK_STATE_BITS as u8 {
                return Err(StateLayoutError::WidthOverflow);
            }
            fields.push(StateField {
                name: spec.name.to_owned(),
                offset,
                width: spec.width,
            });
            offset = end;
        }
        Ok(Self { fields })
    }

    pub fn fields(&self) -> &[StateField] {
        &self.fields
    }

    pub fn field(&self, name: &str) -> Option<&StateField> {
        self.fields.iter().find(|field| field.name == name)
    }

    pub fn read(&self, state: BlockState, name: &str) -> Result<u8, StateAccessError> {
        let field = self.field(name).ok_or(StateAccessError::UnknownField)?;
        Ok(((u16::from(state.raw()) >> field.offset) & field.value_mask()) as u8)
    }

    pub fn write(
        &self,
        state: BlockState,
        name: &str,
        value: u8,
    ) -> Result<BlockState, StateAccessError> {
        let field = self.field(name).ok_or(StateAccessError::UnknownField)?;
        let value_mask = field.value_mask();
        if u16::from(value) > value_mask {
            return Err(StateAccessError::ValueOutOfRange);
        }
        let raw = (u16::from(state.raw()) & !(value_mask << field.offset))
            | (u16::from(value) << field.offset);
        Ok(BlockState::new(raw as u8))
    }
}
