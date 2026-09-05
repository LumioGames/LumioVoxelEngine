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
    pub const AIR: Self = Self(vw::BLOCK_TYPE_AIR);
    pub const ERROR: Self = Self(vw::BLOCK_TYPE_ERROR);
    pub const ENTITY_OCCUPANCY_PLACEHOLDER: Self = Self(vw::BLOCK_TYPE_ECS_OCCUPANCY);
    pub const ENTITY_OCCUPIED: Self = Self::ENTITY_OCCUPANCY_PLACEHOLDER;
    pub const STRUCTURE_PLACEHOLDER: Self = Self(vw::BLOCK_TYPE_STRUCTURE_PLACEHOLDER);

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

/// The four built-in BlockType values. These are typed sentinels, not catalog rows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuiltinBlockType {
    Air,
    ErrorBlock,
    EntityOccupancyPlaceholder,
    StructurePlaceholder,
}

impl BuiltinBlockType {
    pub const ECS_OCCUPANCY: Self = Self::EntityOccupancyPlaceholder;
    pub const ENTITY_OCCUPANCY_PLACEHOLDER: Self = Self::EntityOccupancyPlaceholder;

    pub const fn from_block_type(block_type: BlockType) -> Option<Self> {
        match block_type.raw() {
            vw::BLOCK_TYPE_AIR => Some(Self::Air),
            vw::BLOCK_TYPE_ERROR => Some(Self::ErrorBlock),
            vw::BLOCK_TYPE_ECS_OCCUPANCY => Some(Self::EntityOccupancyPlaceholder),
            vw::BLOCK_TYPE_STRUCTURE_PLACEHOLDER => Some(Self::StructurePlaceholder),
            _ => None,
        }
    }

    pub const fn block_type(self) -> BlockType {
        match self {
            Self::Air => BlockType::AIR,
            Self::ErrorBlock => BlockType::ERROR,
            Self::EntityOccupancyPlaceholder => BlockType::ENTITY_OCCUPANCY_PLACEHOLDER,
            Self::StructurePlaceholder => BlockType::STRUCTURE_PLACEHOLDER,
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::Air => "air",
            Self::ErrorBlock => "error-block",
            Self::EntityOccupancyPlaceholder => "ecs-occupancy",
            Self::StructurePlaceholder => "structure-placeholder",
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MaterialClass {
    Solid,
    Liquid,
}

impl MaterialClass {
    pub fn parse(raw: &str) -> Result<Self, BlockError> {
        match raw {
            "Solid" => Ok(Self::Solid),
            "Liquid" => Ok(Self::Liquid),
            _ => Err(BlockError::contract(vw::UNKNOWN_MATERIAL_CLASS)),
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::Solid => "Solid",
            Self::Liquid => "Liquid",
        }
    }

    pub const fn profile(self) -> &'static MaterialProfile {
        match self {
            Self::Solid => &MATERIAL_TABLE[0],
            Self::Liquid => &MATERIAL_TABLE[1],
        }
    }

    pub fn validate_greedy_merge(self, other: Self) -> Result<(), BlockError> {
        if self != other {
            return Err(BlockError::contract(vw::CROSS_MATERIAL_FACE_MERGE));
        }
        Ok(())
    }

    pub fn validate_auto_propagation_request(self) -> Result<(), BlockError> {
        if self == Self::Liquid {
            return Err(BlockError::contract(
                vw::LIQUID_AUTO_PROPAGATION_UNSUPPORTED,
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MeshBehavior {
    Solid,
    Liquid,
}

impl MeshBehavior {
    pub const fn face_against(self, neighbor: Option<MaterialClass>) -> FaceVisibility {
        match (self, neighbor) {
            (Self::Liquid, None) => FaceVisibility::Visible,
            (Self::Liquid, Some(_)) => FaceVisibility::Hidden,
            (Self::Solid, Some(MaterialClass::Solid)) => FaceVisibility::Hidden,
            (Self::Solid, _) => FaceVisibility::Visible,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FaceVisibility {
    Hidden,
    Visible,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenderPass {
    Opaque,
    Transparent,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CollisionBehavior {
    Solid,
    Passable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LightAttenuation {
    Opaque,
    Attenuating,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MaterialProfile {
    mesh: MeshBehavior,
    render_pass: RenderPass,
    collision: CollisionBehavior,
    light_attenuation: LightAttenuation,
    queryable: bool,
}

impl MaterialProfile {
    pub const fn mesh(self) -> MeshBehavior {
        self.mesh
    }
    pub const fn render_pass(self) -> RenderPass {
        self.render_pass
    }
    pub const fn collision(self) -> CollisionBehavior {
        self.collision
    }
    pub const fn light_attenuation(self) -> LightAttenuation {
        self.light_attenuation
    }
    pub const fn queryable(self) -> bool {
        self.queryable
    }
}

pub static MATERIAL_TABLE: [MaterialProfile; 2] = [
    MaterialProfile {
        mesh: MeshBehavior::Solid,
        render_pass: RenderPass::Opaque,
        collision: CollisionBehavior::Solid,
        light_attenuation: LightAttenuation::Opaque,
        queryable: true,
    },
    MaterialProfile {
        mesh: MeshBehavior::Liquid,
        render_pass: RenderPass::Transparent,
        collision: CollisionBehavior::Passable,
        light_attenuation: LightAttenuation::Attenuating,
        queryable: true,
    },
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MaterialStorage {
    BlockTypeTable,
    PerCellLane,
}

impl MaterialStorage {
    pub fn validate(self) -> Result<(), BlockError> {
        match self {
            Self::BlockTypeTable => Ok(()),
            Self::PerCellLane => Err(BlockError::contract(vw::MATERIAL_CLASS_NOT_A_CELL_LANE)),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BehaviorTemplate {
    FullCube,
    Liquid,
}

impl BehaviorTemplate {
    pub fn parse(raw: &str) -> Result<Self, BlockError> {
        match raw {
            "FullCube" => Ok(Self::FullCube),
            "Liquid" => Ok(Self::Liquid),
            _ => Err(BlockError::contract(vw::UNKNOWN_BEHAVIOR_TEMPLATE)),
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::FullCube => "FullCube",
            Self::Liquid => "Liquid",
        }
    }

    pub fn state_layout(self) -> StateLayout {
        match self {
            Self::FullCube => StateLayout::empty(),
            Self::Liquid => StateLayout::new(&[StateFieldSpec::new("level", 4)])
                .expect("the frozen Liquid layout uses four of eight bits"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlockDefinition {
    block_type: BlockType,
    name: String,
    material_class: MaterialClass,
    behavior_template: BehaviorTemplate,
    asset_ref: String,
    state_layout: StateLayout,
}

impl BlockDefinition {
    pub const fn block_type(&self) -> BlockType {
        self.block_type
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub const fn material_class(&self) -> MaterialClass {
        self.material_class
    }
    pub const fn behavior_template(&self) -> BehaviorTemplate {
        self.behavior_template
    }
    pub fn asset_ref(&self) -> &str {
        &self.asset_ref
    }
    pub const fn state_layout(&self) -> &StateLayout {
        &self.state_layout
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlockCatalogRowInput {
    pub block_type: Option<u32>,
    pub name: Option<String>,
    pub material_class: Option<String>,
    pub behavior_template: Option<String>,
    pub asset_ref: Option<String>,
    pub state_layout: Option<StateLayout>,
}

impl BlockCatalogRowInput {
    fn into_definition(self) -> Result<BlockDefinition, BlockError> {
        let Some(block_type) = self.block_type else {
            return Err(BlockError::contract(vw::BLOCK_CATALOG_ROW_INCOMPLETE));
        };
        let Some(name) = self.name.filter(|v| !v.trim().is_empty()) else {
            return Err(BlockError::contract(vw::BLOCK_CATALOG_ROW_INCOMPLETE));
        };
        let Some(material_name) = self.material_class.filter(|v| !v.trim().is_empty()) else {
            return Err(BlockError::contract(vw::BLOCK_CATALOG_ROW_INCOMPLETE));
        };
        let Some(template_name) = self.behavior_template.filter(|v| !v.trim().is_empty()) else {
            return Err(BlockError::contract(vw::BLOCK_CATALOG_ROW_INCOMPLETE));
        };
        let Some(asset_ref) = self.asset_ref.filter(|v| !v.trim().is_empty()) else {
            return Err(BlockError::contract(vw::BLOCK_CATALOG_ROW_INCOMPLETE));
        };
        let Some(state_layout) = self.state_layout else {
            return Err(BlockError::contract(vw::BLOCK_CATALOG_ROW_INCOMPLETE));
        };
        let block_type = BlockType::new(block_type)
            .ok_or_else(|| BlockError::contract(vw::BLOCK_CATALOG_NOT_DENSE))?;
        Ok(BlockDefinition {
            block_type,
            name,
            material_class: MaterialClass::parse(&material_name)?,
            behavior_template: BehaviorTemplate::parse(&template_name)?,
            asset_ref,
            state_layout,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OfficialCatalog {
    rows: Vec<Option<BlockDefinition>>,
    retired_names: Vec<String>,
}

impl OfficialCatalog {
    pub fn load(
        rows: impl IntoIterator<Item = BlockCatalogRowInput>,
        retired_names: &[&str],
    ) -> Result<Self, BlockError> {
        let mut catalog = Self {
            rows: vec![None; vw::FIRST_OFFICIAL_BLOCK_TYPE as usize],
            retired_names: retired_names.iter().map(|n| (*n).to_owned()).collect(),
        };
        for row in rows {
            catalog.register(row)?;
        }
        Ok(catalog)
    }

    pub fn register(&mut self, row: BlockCatalogRowInput) -> Result<BlockType, BlockError> {
        let definition = row.into_definition()?;
        let block_type = definition.block_type;
        if block_type.raw() <= vw::SYSTEM_RESERVED_TYPE_MAX {
            return Err(BlockError::contract(vw::SYSTEM_RESERVED_TYPE_MISUSE));
        }
        if block_type.scope() != BlockScope::Global {
            return Err(BlockError::contract(vw::BLOCK_TYPE_SCOPE_VIOLATION));
        }
        if block_type.raw() != self.rows.len() as u32 {
            return Err(BlockError::contract(vw::BLOCK_CATALOG_NOT_DENSE));
        }
        if self.retired_names.iter().any(|n| n == &definition.name)
            || self
                .rows
                .iter()
                .flatten()
                .any(|r| r.name == definition.name)
        {
            return Err(BlockError::contract(vw::BLOCK_CATALOG_NAME_REUSED));
        }
        self.rows.push(Some(definition));
        Ok(block_type)
    }

    pub fn get(&self, block_type: BlockType) -> Result<Option<&BlockDefinition>, BlockError> {
        if block_type.scope() != BlockScope::Global {
            return Err(BlockError::contract(vw::BLOCK_TYPE_SCOPE_VIOLATION));
        }
        Ok(self
            .rows
            .get(block_type.raw() as usize)
            .and_then(Option::as_ref))
    }

    pub fn resolve_palette(
        &self,
        palette: &[BlockId],
    ) -> Result<Vec<&BlockDefinition>, BlockError> {
        palette
            .iter()
            .map(|id| {
                self.get(id.block_type())?
                    .ok_or_else(|| BlockError::contract(vw::UNREGISTERED_BLOCK_TYPE))
            })
            .collect()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RoomBehaviorInput {
    Template(String),
    Custom,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoomLocalRowInput {
    pub name: String,
    pub material_class: String,
    pub behavior: RoomBehaviorInput,
    pub asset_ref: String,
    pub state_layout: StateLayout,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RoomLocalCatalog {
    rows: Vec<BlockDefinition>,
}

impl RoomLocalCatalog {
    pub const fn new() -> Self {
        Self { rows: Vec::new() }
    }

    pub fn register(&mut self, row: RoomLocalRowInput) -> Result<BlockType, BlockError> {
        if row.name.trim().is_empty()
            || row.material_class.trim().is_empty()
            || row.asset_ref.trim().is_empty()
            || matches!(&row.behavior, RoomBehaviorInput::Template(name) if name.trim().is_empty())
        {
            return Err(BlockError::contract(vw::BLOCK_CATALOG_ROW_INCOMPLETE));
        }
        let behavior_template = match row.behavior {
            RoomBehaviorInput::Template(name) => BehaviorTemplate::parse(&name)?,
            RoomBehaviorInput::Custom => {
                return Err(BlockError::contract(vw::PLAYER_TYPE_DECLARES_BEHAVIOR));
            }
        };
        let local_index = self.rows.len() as u32;
        let block_type = BlockType::new(vw::BLOCK_TYPE_SCOPE_MASK | local_index)
            .expect("local index fits uint23");
        let material_class = MaterialClass::parse(&row.material_class)?;
        self.rows.push(BlockDefinition {
            block_type,
            name: row.name,
            material_class,
            behavior_template,
            asset_ref: row.asset_ref,
            state_layout: row.state_layout,
        });
        Ok(block_type)
    }

    pub fn get(&self, block_type: BlockType) -> Result<&BlockDefinition, BlockError> {
        let local_index = block_type
            .room_local_index()
            .ok_or_else(|| BlockError::contract(vw::BLOCK_TYPE_SCOPE_VIOLATION))?;
        self.rows
            .get(local_index as usize)
            .ok_or_else(|| BlockError::contract(vw::UNREGISTERED_BLOCK_TYPE))
    }

    pub fn get_for_save_mapping(
        &self,
        block_type: BlockType,
    ) -> Result<&BlockDefinition, BlockError> {
        let local_index = block_type
            .room_local_index()
            .ok_or_else(|| BlockError::contract(vw::BLOCK_TYPE_SCOPE_VIOLATION))?;
        self.rows
            .get(local_index as usize)
            .ok_or_else(|| BlockError::contract(vw::ROOM_LOCAL_TYPE_WITHOUT_MAPPING))
    }

    pub fn import_from(
        &mut self,
        source: &Self,
    ) -> Result<Vec<(BlockType, BlockType)>, BlockError> {
        let mut staged = self.clone();
        let mut remap = Vec::with_capacity(source.rows.len());
        for definition in &source.rows {
            let new = staged.register(RoomLocalRowInput {
                name: definition.name.clone(),
                material_class: definition.material_class.name().to_owned(),
                behavior: RoomBehaviorInput::Template(
                    definition.behavior_template.name().to_owned(),
                ),
                asset_ref: definition.asset_ref.clone(),
                state_layout: definition.state_layout.clone(),
            })?;
            remap.push((definition.block_type, new));
        }
        *self = staged;
        Ok(remap)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlockTables {
    official: OfficialCatalog,
    room_local: RoomLocalCatalog,
}

/// A resolved block is either a typed sentinel or a registered ordinary definition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlockResolution<'a> {
    Builtin(BuiltinBlockType),
    Ordinary(&'a BlockDefinition),
}

impl<'a> BlockResolution<'a> {
    pub const fn builtin(self) -> Option<BuiltinBlockType> {
        match self {
            Self::Builtin(v) => Some(v),
            Self::Ordinary(_) => None,
        }
    }
    pub const fn ordinary(self) -> Option<&'a BlockDefinition> {
        match self {
            Self::Builtin(_) => None,
            Self::Ordinary(v) => Some(v),
        }
    }
    pub fn name(self) -> &'a str {
        match self {
            Self::Builtin(v) => v.name(),
            Self::Ordinary(v) => v.name(),
        }
    }
    pub const fn material_class(self) -> Option<MaterialClass> {
        match self {
            Self::Builtin(_) => None,
            Self::Ordinary(v) => Some(v.material_class()),
        }
    }
    pub const fn behavior_template(self) -> Option<BehaviorTemplate> {
        match self {
            Self::Builtin(_) => None,
            Self::Ordinary(v) => Some(v.behavior_template()),
        }
    }
}

impl BlockTables {
    pub const fn new(official: OfficialCatalog, room_local: RoomLocalCatalog) -> Self {
        Self {
            official,
            room_local,
        }
    }

    pub fn resolve(&self, block_id: BlockId) -> Result<BlockResolution<'_>, BlockError> {
        let block_type = block_id.block_type();
        if let Some(builtin) = BuiltinBlockType::from_block_type(block_type) {
            return Ok(BlockResolution::Builtin(builtin));
        }
        match block_type.scope() {
            BlockScope::Global => {
                if block_type.raw() <= vw::SYSTEM_RESERVED_TYPE_MAX {
                    return Err(BlockError::contract(vw::UNREGISTERED_BLOCK_TYPE));
                }
                self.official
                    .get(block_type)?
                    .map(BlockResolution::Ordinary)
                    .ok_or_else(|| BlockError::contract(vw::UNREGISTERED_BLOCK_TYPE))
            }
            BlockScope::RoomLocal => self
                .room_local
                .get(block_type)
                .map(BlockResolution::Ordinary),
        }
    }

    pub fn resolve_type(&self, block_type: BlockType) -> Result<BlockResolution<'_>, BlockError> {
        self.resolve(BlockId::from_parts(block_type, BlockState::new(0)))
    }

    pub fn resolve_block_type(
        &self,
        block_type: BlockType,
    ) -> Result<BlockResolution<'_>, BlockError> {
        self.resolve_type(block_type)
    }

    pub fn resolve_for_save_mapping(
        &self,
        block_type: BlockType,
    ) -> Result<&BlockDefinition, BlockError> {
        self.room_local.get_for_save_mapping(block_type)
    }

    pub fn resolve_ordinary(&self, block_id: BlockId) -> Result<&BlockDefinition, BlockError> {
        self.resolve(block_id)?
            .ordinary()
            .ok_or_else(|| BlockError::contract(vw::UNREGISTERED_BLOCK_TYPE))
    }
}
