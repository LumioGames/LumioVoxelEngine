//! Read-only voxel physics queries over canonical Section storage.
//!
//! The query surface owns traversal and geometry only. A caller supplies the
//! material lookup and mask, so collision eligibility never depends on a
//! block-id branch in this module.

#![forbid(unsafe_code)]

use lumio_voxel_contracts::voxel_world as vw;
use lumio_voxel_domain::block::{BlockId, BlockType, WorldY};
use lumio_voxel_domain::key::SectionId;
use lumio_voxel_domain::publication::PublishedReadView;
use lumio_voxel_domain::section::{SectionPresenceGuard, SectionStorage, SectionStorageResolver};
use std::collections::BTreeMap;
use std::ops::{BitOr, BitOrAssign};

const EPSILON: f32 = 1.0e-6;

/// A three-dimensional point or vector in world coordinates.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec3 {
    x: f32,
    y: f32,
    z: f32,
}

impl Vec3 {
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    pub const fn x(self) -> f32 {
        self.x
    }

    pub const fn y(self) -> f32 {
        self.y
    }

    pub const fn z(self) -> f32 {
        self.z
    }

    fn add(self, other: Self) -> Self {
        Self::new(self.x + other.x, self.y + other.y, self.z + other.z)
    }

    fn scale(self, scalar: f32) -> Self {
        Self::new(self.x * scalar, self.y * scalar, self.z * scalar)
    }

    fn length(self) -> f32 {
        self.x
            .mul_add(self.x, self.y.mul_add(self.y, self.z * self.z))
            .sqrt()
    }

    fn normalized(self) -> Option<Self> {
        let length = self.length();
        if !length.is_finite() || length <= EPSILON {
            None
        } else {
            Some(self.scale(1.0 / length))
        }
    }
}

/// An axis-aligned query shape represented by center and half-extents.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Aabb {
    center: Vec3,
    half_extents: Vec3,
}

impl Aabb {
    pub const fn new(center: Vec3, half_extents: Vec3) -> Self {
        Self {
            center,
            half_extents,
        }
    }

    pub const fn center(self) -> Vec3 {
        self.center
    }

    pub const fn half_extents(self) -> Vec3 {
        self.half_extents
    }

    fn min(self) -> Vec3 {
        Vec3::new(
            self.center.x - self.half_extents.x,
            self.center.y - self.half_extents.y,
            self.center.z - self.half_extents.z,
        )
    }

    fn max(self) -> Vec3 {
        Vec3::new(
            self.center.x + self.half_extents.x,
            self.center.y + self.half_extents.y,
            self.center.z + self.half_extents.z,
        )
    }
}

/// v1 material classes. The lookup is injected so this module does not own a
/// second catalog or a collision policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MaterialClass {
    Solid,
    Liquid,
}

impl MaterialClass {
    const fn bit(self) -> u8 {
        match self {
            Self::Solid => 1,
            Self::Liquid => 2,
        }
    }
}

/// Read-only BlockType to material-class mapping supplied by the caller.
pub trait MaterialClassLookup {
    fn class_for(&self, block_type: BlockType) -> Option<MaterialClass>;
}

/// A small immutable-at-query-time lookup table useful for hosts and tests.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MaterialTable {
    entries: BTreeMap<u32, MaterialClass>,
}

impl MaterialTable {
    pub fn insert(&mut self, block_type: BlockType, class: MaterialClass) {
        self.entries.insert(block_type.raw(), class);
    }

    pub fn with(mut self, block_type: BlockType, class: MaterialClass) -> Self {
        self.insert(block_type, class);
        self
    }
}

impl MaterialClassLookup for MaterialTable {
    fn class_for(&self, block_type: BlockType) -> Option<MaterialClass> {
        self.entries.get(&block_type.raw()).copied()
    }
}

/// Material-class bit mask used by each query.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MaterialMask(u8);

impl MaterialMask {
    pub const fn empty() -> Self {
        Self(0)
    }

    pub const fn all() -> Self {
        Self(MaterialClass::Solid.bit() | MaterialClass::Liquid.bit())
    }

    pub const fn solid() -> Self {
        Self::of(MaterialClass::Solid)
    }

    pub const fn liquid() -> Self {
        Self::of(MaterialClass::Liquid)
    }

    pub const fn of(class: MaterialClass) -> Self {
        Self(class.bit())
    }

    pub const fn contains(self, class: MaterialClass) -> bool {
        self.0 & class.bit() != 0
    }
}

impl BitOr for MaterialMask {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl BitOrAssign for MaterialMask {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

/// Canonical world block coordinate.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CellCoord {
    x: i32,
    y: i32,
    z: i32,
}

impl CellCoord {
    pub const fn new(x: i32, y: i32, z: i32) -> Self {
        Self { x, y, z }
    }

    pub const fn x(self) -> i32 {
        self.x
    }

    pub const fn y(self) -> i32 {
        self.y
    }

    pub const fn z(self) -> i32 {
        self.z
    }
}

/// Identifies whether a result came from voxel storage or a registered body.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum HitTarget {
    Block,
    Body(BodyId),
}

/// Stable identifier for a non-voxel body.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BodyId(u64);

impl BodyId {
    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }

    pub const fn raw(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Debug, PartialEq)]
struct Body {
    shape: Aabb,
}

/// The immutable snapshot consumed by a [`PhysicsQuery`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PhysicsWorld {
    sections: BTreeMap<SectionId, SectionState>,
    bodies: BTreeMap<BodyId, Body>,
}

impl PhysicsWorld {
    pub fn new() -> Self {
        Self::default()
    }

    /// Materialize a physics snapshot from one immutable published cut.
    pub fn from_published_view(view: &PublishedReadView) -> Result<Self, PhysicsQueryError> {
        Self::from_published_view_with_baseline(view, &|_: &SectionId| None)
    }

    /// Materialize a published cut, resolving zero-byte `Unchanged` tickets
    /// against the immutable original-map baseline.
    pub fn from_published_view_with_baseline<R>(
        view: &PublishedReadView,
        baseline: &R,
    ) -> Result<Self, PhysicsQueryError>
    where
        R: SectionStorageResolver + ?Sized,
    {
        let mut world = Self::new();
        for (id, slot) in view.directory().iter() {
            match slot.presence() {
                "Ready" => {
                    let storage = slot
                        .payload()
                        .and_then(|payload| payload.storage().cloned())
                        .ok_or_else(|| {
                            PhysicsQueryError::contract(vw::SECTION_ENCODING_MISMATCH)
                        })?;
                    world.insert_ready(*id, storage);
                }
                "Pending" => world.insert_pending(*id),
                "Unchanged" => {
                    let storage = baseline.resolve(id).ok_or_else(|| {
                        PhysicsQueryError::contract(vw::SECTION_ENCODING_MISMATCH)
                    })?;
                    world.insert_ready(*id, storage);
                }
                "Unavailable" => world.insert_unavailable(*id),
                _ => return Err(PhysicsQueryError::contract(vw::SECTION_ENCODING_MISMATCH)),
            }
        }
        Ok(world)
    }

    pub fn insert_ready(&mut self, section_id: SectionId, storage: SectionStorage) {
        self.sections
            .insert(section_id, SectionState::Ready(storage));
    }

    pub fn insert_pending(&mut self, section_id: SectionId) {
        self.sections.insert(section_id, SectionState::Pending);
    }

    pub fn insert_unavailable(&mut self, section_id: SectionId) {
        self.sections.insert(section_id, SectionState::Unavailable);
    }

    pub fn remove_section(&mut self, section_id: SectionId) -> Option<SectionState> {
        self.sections.remove(&section_id)
    }

    pub fn register_body(&mut self, id: BodyId, shape: Aabb) {
        self.bodies.insert(id, Body { shape });
    }

    pub fn unregister_body(&mut self, id: BodyId) -> Option<Aabb> {
        self.bodies.remove(&id).map(|body| body.shape)
    }

    pub fn section_state(&self, section_id: SectionId) -> Option<&SectionState> {
        self.sections.get(&section_id)
    }
}

/// Residency state visible to the physics query. Missing entries are also
/// unresolved, preserving the distinction between no data and air.
#[derive(Clone, Debug, PartialEq)]
pub enum SectionState {
    Ready(SectionStorage),
    Pending,
    Unavailable,
}

/// Contract-stable rejection returned for misuse or malformed injected data.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PhysicsQueryError {
    error_id: &'static str,
}

impl PhysicsQueryError {
    fn new(error_id: &'static str) -> Self {
        Self { error_id }
    }

    pub const fn error_id(self) -> &'static str {
        self.error_id
    }

    pub fn unresolved_hit_treated_as_air() -> Self {
        Self::contract("unresolved_hit_treated_as_air")
    }

    pub fn unresolved_hit_treated_as_solid() -> Self {
        Self::contract("unresolved_hit_treated_as_solid")
    }

    pub fn query_buffer_overflow() -> Self {
        Self::contract("query_buffer_overflow")
    }

    pub fn query_result_divergence() -> Self {
        Self::contract("query_result_divergence")
    }

    pub fn query_mutates_world() -> Self {
        Self::contract("query_mutates_world")
    }

    pub fn unknown_material_class() -> Self {
        Self::contract(vw::UNKNOWN_MATERIAL_CLASS)
    }

    fn world_y_out_of_range() -> Self {
        Self::contract("world_y_out_of_range")
    }

    fn contract(id: &str) -> Self {
        Self::new(vw::intern_error_code(id).expect("physics error id is declared by the contract"))
    }
}

impl std::fmt::Display for PhysicsQueryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.error_id)
    }
}

impl std::error::Error for PhysicsQueryError {}

/// The three normal query outcomes. Unresolved is a result, not an error.
#[derive(Clone, Debug, PartialEq)]
pub enum QueryResolution<T> {
    Hit(T),
    Miss,
    Unresolved { section: SectionId },
}

impl<T> QueryResolution<T> {
    pub fn hit(&self) -> Option<&T> {
        match self {
            Self::Hit(hit) => Some(hit),
            Self::Miss | Self::Unresolved { .. } => None,
        }
    }

    pub const fn is_hit(&self) -> bool {
        matches!(self, Self::Hit(_))
    }

    pub const fn is_miss(&self) -> bool {
        matches!(self, Self::Miss)
    }

    pub fn unresolved_section(&self) -> Option<SectionId> {
        match self {
            Self::Unresolved { section } => Some(*section),
            Self::Hit(_) | Self::Miss => None,
        }
    }

    pub fn unresolved_section_key(&self) -> Option<String> {
        self.unresolved_section().map(|section| section.key())
    }

    pub fn interpret_as_miss(&self) -> Result<Option<&T>, PhysicsQueryError> {
        match self {
            Self::Unresolved { .. } => Err(PhysicsQueryError::unresolved_hit_treated_as_air()),
            Self::Hit(hit) => Ok(Some(hit)),
            Self::Miss => Ok(None),
        }
    }

    pub fn interpret_as_hit(&self) -> Result<Option<&T>, PhysicsQueryError> {
        match self {
            Self::Unresolved { .. } => Err(PhysicsQueryError::unresolved_hit_treated_as_solid()),
            Self::Hit(hit) => Ok(Some(hit)),
            Self::Miss => Ok(None),
        }
    }
}

/// Common ray/sweep hit data. A block hit always has `block_id`; a body hit
/// is identified by `target` and has no voxel id.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RaycastHit {
    cell: CellCoord,
    block_id: Option<BlockId>,
    point: Vec3,
    normal: Vec3,
    distance: f32,
    target: HitTarget,
}

impl RaycastHit {
    pub const fn cell(self) -> CellCoord {
        self.cell
    }

    pub const fn block_id(self) -> Option<BlockId> {
        self.block_id
    }

    pub const fn point(self) -> Vec3 {
        self.point
    }

    pub const fn normal(self) -> Vec3 {
        self.normal
    }

    pub const fn distance(self) -> f32 {
        self.distance
    }

    pub const fn target(self) -> HitTarget {
        self.target
    }
}

/// Sweep hit data. `fraction` is always clamped to the inclusive 0..1 range.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SweepHit {
    cell: CellCoord,
    block_id: Option<BlockId>,
    point: Vec3,
    normal: Vec3,
    fraction: f32,
    target: HitTarget,
}

impl SweepHit {
    pub const fn cell(self) -> CellCoord {
        self.cell
    }

    pub const fn block_id(self) -> Option<BlockId> {
        self.block_id
    }

    pub const fn point(self) -> Vec3 {
        self.point
    }

    pub const fn normal(self) -> Vec3 {
        self.normal
    }

    pub const fn fraction(self) -> f32 {
        self.fraction
    }

    pub const fn target(self) -> HitTarget {
        self.target
    }
}

/// Caller-owned overlap result entry.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OverlapHit {
    cell: CellCoord,
    block_id: Option<BlockId>,
    target: HitTarget,
}

impl Default for OverlapHit {
    fn default() -> Self {
        Self {
            cell: CellCoord::default(),
            block_id: None,
            target: HitTarget::Block,
        }
    }
}

impl OverlapHit {
    pub const fn cell(self) -> CellCoord {
        self.cell
    }

    pub const fn block_id(self) -> Option<BlockId> {
        self.block_id
    }

    pub const fn target(self) -> HitTarget {
        self.target
    }
}

/// Overlap metadata; entries themselves remain in the caller's buffer.
#[derive(Clone, Debug, PartialEq)]
pub struct OverlapResult {
    resolution: QueryResolution<()>,
    actual_count: usize,
    written_count: usize,
    truncated: bool,
}

impl OverlapResult {
    pub fn resolution(&self) -> &QueryResolution<()> {
        &self.resolution
    }

    pub const fn actual_count(&self) -> usize {
        self.actual_count
    }

    pub const fn written_count(&self) -> usize {
        self.written_count
    }

    pub const fn truncated(&self) -> bool {
        self.truncated
    }

    pub fn require_complete(&self) -> Result<(), PhysicsQueryError> {
        if self.truncated {
            Err(PhysicsQueryError::query_buffer_overflow())
        } else {
            Ok(())
        }
    }

    pub fn unresolved_section_key(&self) -> Option<String> {
        self.resolution.unresolved_section_key()
    }
}

/// Read-only query facade. It borrows the world and material table for the
/// duration of each call; no query method has mutable access to either.
pub struct PhysicsQuery<'world, 'materials> {
    world: &'world PhysicsWorld,
    materials: &'materials dyn MaterialClassLookup,
    guard: Option<&'world dyn SectionPresenceGuard>,
}

impl<'world, 'materials> PhysicsQuery<'world, 'materials> {
    pub fn new(
        world: &'world PhysicsWorld,
        materials: &'materials dyn MaterialClassLookup,
    ) -> Self {
        Self {
            world,
            materials,
            guard: None,
        }
    }
}

impl<'world, 'materials> PhysicsQuery<'world, 'materials> {
    /// Construct a physics query that rejects Pending/Unavailable observations
    /// for Sections protected by a ready residency pin.
    pub fn with_presence_guard(
        world: &'world PhysicsWorld,
        materials: &'materials dyn MaterialClassLookup,
        guard: &'world dyn SectionPresenceGuard,
    ) -> Self {
        Self {
            world,
            materials,
            guard: Some(guard),
        }
    }

    /// DDA traversal of the canonical voxel grid.
    pub fn raycast(
        &self,
        origin: Vec3,
        direction: Vec3,
        max_distance: f32,
        filter: MaterialMask,
    ) -> Result<QueryResolution<RaycastHit>, PhysicsQueryError> {
        if !origin.x.is_finite()
            || !origin.y.is_finite()
            || !origin.z.is_finite()
            || !max_distance.is_finite()
            || max_distance < 0.0
        {
            return Ok(QueryResolution::Miss);
        }
        let Some(direction) = direction.normalized() else {
            return Ok(QueryResolution::Miss);
        };
        let start = CellCoord::new(
            initial_cell(origin.x, direction.x),
            initial_cell(origin.y, direction.y),
            initial_cell(origin.z, direction.z),
        );

        let body_hit = self
            .world
            .bodies
            .iter()
            .filter_map(|(id, body)| {
                ray_aabb(origin, direction, body.shape).map(|(distance, normal)| {
                    (
                        distance,
                        RaycastHit {
                            cell: CellCoord::new(
                                floor_i32(body.shape.center.x),
                                floor_i32(body.shape.center.y),
                                floor_i32(body.shape.center.z),
                            ),
                            block_id: None,
                            point: origin.add(direction.scale(distance)),
                            normal,
                            distance,
                            target: HitTarget::Body(*id),
                        },
                    )
                })
            })
            .filter(|(distance, _)| *distance >= 0.0 && *distance <= max_distance)
            .min_by(|left, right| {
                left.0
                    .total_cmp(&right.0)
                    .then_with(|| left.1.target.cmp(&right.1.target))
            });
        let body_distance = body_hit
            .as_ref()
            .map(|(distance, _)| *distance)
            .unwrap_or(f32::INFINITY);

        let mut cell = start;
        let mut t = 0.0;
        let mut steps = 0_usize;
        let (step_x, mut next_x, delta_x) = dda_axis(origin.x, direction.x, cell.x);
        let (step_y, mut next_y, delta_y) = dda_axis(origin.y, direction.y, cell.y);
        let (step_z, mut next_z, delta_z) = dda_axis(origin.z, direction.z, cell.z);
        let mut normal = boundary_entry_normal(origin, direction);

        while t <= max_distance + EPSILON && t <= body_distance + EPSILON {
            if steps >= MAX_PHYSICS_QUERY_CELLS {
                return Err(PhysicsQueryError::query_buffer_overflow());
            }
            steps += 1;
            match self.block_at(cell, filter)? {
                CellVisit::Unresolved(section) => {
                    return Ok(QueryResolution::Unresolved { section });
                }
                CellVisit::Hit(block_id) => {
                    return Ok(QueryResolution::Hit(RaycastHit {
                        cell,
                        block_id: Some(block_id),
                        point: origin.add(direction.scale(t.max(0.0))),
                        normal,
                        distance: t.max(0.0),
                        target: HitTarget::Block,
                    }));
                }
                CellVisit::Empty => {}
            }

            let (axis, next_t) = min_axis(next_x, next_y, next_z);
            if !next_t.is_finite() || next_t > max_distance + EPSILON {
                break;
            }
            t = next_t;
            match axis {
                Axis::X => {
                    let Some(next) = cell.x.checked_add(step_x) else {
                        break;
                    };
                    cell.x = next;
                    next_x += delta_x;
                    normal = Vec3::new(-step_x as f32, 0.0, 0.0);
                }
                Axis::Y => {
                    let Some(next) = cell.y.checked_add(step_y) else {
                        break;
                    };
                    cell.y = next;
                    next_y += delta_y;
                    normal = Vec3::new(0.0, -step_y as f32, 0.0);
                }
                Axis::Z => {
                    let Some(next) = cell.z.checked_add(step_z) else {
                        break;
                    };
                    cell.z = next;
                    next_z += delta_z;
                    normal = Vec3::new(0.0, 0.0, -step_z as f32);
                }
            }
        }

        if let Some((distance, hit)) = body_hit
            && distance <= max_distance + EPSILON
        {
            return Ok(QueryResolution::Hit(hit));
        }
        Ok(QueryResolution::Miss)
    }

    /// Enumerates the canonical y/z/x cell order into caller-owned storage.
    pub fn overlap(
        &self,
        shape: Aabb,
        filter: MaterialMask,
        buffer: &mut [OverlapHit],
    ) -> Result<OverlapResult, PhysicsQueryError> {
        let Some((min, max)) = cell_range(shape)? else {
            return Ok(OverlapResult {
                resolution: QueryResolution::Miss,
                actual_count: 0,
                written_count: 0,
                truncated: false,
            });
        };
        let mut actual_count = 0_usize;
        let mut written_count = 0_usize;
        for y in min.y..=max.y {
            for z in min.z..=max.z {
                for x in min.x..=max.x {
                    let cell = CellCoord::new(x, y, z);
                    match self.block_at(cell, filter)? {
                        CellVisit::Unresolved(section) => {
                            return Ok(OverlapResult {
                                resolution: QueryResolution::Unresolved { section },
                                actual_count,
                                written_count,
                                truncated: actual_count > buffer.len(),
                            });
                        }
                        CellVisit::Hit(block_id) => {
                            actual_count += 1;
                            write_overlap(
                                buffer,
                                &mut written_count,
                                OverlapHit {
                                    cell,
                                    block_id: Some(block_id),
                                    target: HitTarget::Block,
                                },
                            );
                        }
                        CellVisit::Empty => {}
                    }
                }
            }
        }

        for (id, body) in &self.world.bodies {
            if aabb_intersects(shape, body.shape) {
                actual_count += 1;
                write_overlap(
                    buffer,
                    &mut written_count,
                    OverlapHit {
                        cell: CellCoord::new(
                            floor_i32(body.shape.center.x),
                            floor_i32(body.shape.center.y),
                            floor_i32(body.shape.center.z),
                        ),
                        block_id: None,
                        target: HitTarget::Body(*id),
                    },
                );
            }
        }

        Ok(OverlapResult {
            resolution: if actual_count == 0 {
                QueryResolution::Miss
            } else {
                QueryResolution::Hit(())
            },
            actual_count,
            written_count,
            truncated: actual_count > buffer.len(),
        })
    }

    /// Sweep an AABB through the grid; no response or position mutation is
    /// performed. The same material mask controls every voxel candidate.
    pub fn sweep(
        &self,
        shape: Aabb,
        displacement: Vec3,
        filter: MaterialMask,
    ) -> Result<QueryResolution<SweepHit>, PhysicsQueryError> {
        let end = Aabb::new(shape.center.add(displacement), shape.half_extents);
        let Some((min, max)) = swept_cell_range(shape, end)? else {
            return Ok(QueryResolution::Miss);
        };
        let mut best: Option<(f32, SweepHit)> = None;
        let mut unresolved: Option<(f32, SectionId)> = None;
        for y in min.y..=max.y {
            for z in min.z..=max.z {
                for x in min.x..=max.x {
                    let cell = CellCoord::new(x, y, z);
                    let Some((fraction, normal)) = sweep_aabb(shape, displacement, cell) else {
                        continue;
                    };
                    let block_id = match self.block_at(cell, filter)? {
                        CellVisit::Unresolved(section) => {
                            let replace =
                                unresolved
                                    .as_ref()
                                    .is_none_or(|(old_fraction, old_section)| {
                                        fraction.total_cmp(old_fraction).is_lt()
                                            || (fraction.total_cmp(old_fraction).is_eq()
                                                && section < *old_section)
                                    });
                            if replace {
                                unresolved = Some((fraction, section));
                            }
                            continue;
                        }
                        CellVisit::Hit(block_id) => block_id,
                        CellVisit::Empty => continue,
                    };
                    let hit = SweepHit {
                        cell,
                        block_id: Some(block_id),
                        point: shape.center.add(displacement.scale(fraction)),
                        normal,
                        fraction: fraction.clamp(0.0, 1.0),
                        target: HitTarget::Block,
                    };
                    if best
                        .as_ref()
                        .is_none_or(|(old_fraction, _)| fraction.total_cmp(old_fraction).is_lt())
                    {
                        best = Some((fraction, hit));
                    }
                }
            }
        }

        for (id, body) in &self.world.bodies {
            let body_shape = Aabb::new(
                body.shape.center,
                Vec3::new(
                    body.shape.half_extents.x + shape.half_extents.x,
                    body.shape.half_extents.y + shape.half_extents.y,
                    body.shape.half_extents.z + shape.half_extents.z,
                ),
            );
            let Some((fraction, normal)) = ray_aabb(shape.center, displacement, body_shape) else {
                continue;
            };
            let fraction = fraction.clamp(0.0, 1.0);
            let hit = SweepHit {
                cell: CellCoord::new(
                    floor_i32(body.shape.center.x),
                    floor_i32(body.shape.center.y),
                    floor_i32(body.shape.center.z),
                ),
                block_id: None,
                point: shape.center.add(displacement.scale(fraction)),
                normal,
                fraction,
                target: HitTarget::Body(*id),
            };
            if best
                .as_ref()
                .is_none_or(|(old_fraction, _)| fraction.total_cmp(old_fraction).is_lt())
            {
                best = Some((fraction, hit));
            }
        }

        if let Some((unresolved_fraction, section)) = unresolved
            && best.as_ref().is_none_or(|(best_fraction, _)| {
                !unresolved_fraction.total_cmp(best_fraction).is_gt()
            })
        {
            return Ok(QueryResolution::Unresolved { section });
        }

        Ok(best.map_or(QueryResolution::Miss, |(_, hit)| QueryResolution::Hit(hit)))
    }

    fn block_at(
        &self,
        cell: CellCoord,
        filter: MaterialMask,
    ) -> Result<CellVisit, PhysicsQueryError> {
        if cell.y < 0 || cell.y > i32::from(vw::WORLD_Y_MAX as u8) {
            return Ok(CellVisit::Empty);
        }
        let world_y = WorldY::new(i64::from(cell.y))
            .map_err(|_| PhysicsQueryError::world_y_out_of_range())?;
        let section = SectionId::new(
            cell.x.div_euclid(16),
            i64::from(cell.y.div_euclid(16)),
            cell.z.div_euclid(16),
        )
        .map_err(|_| PhysicsQueryError::world_y_out_of_range())?;
        let Some(state) = self.world.sections.get(&section) else {
            if let Some(guard) = self.guard {
                guard
                    .validate_presence(&section.key(), "Unavailable")
                    .map_err(PhysicsQueryError::contract)?;
            }
            return Ok(CellVisit::Unresolved(section));
        };
        let presence = match state {
            SectionState::Ready(_) => "Ready",
            SectionState::Pending => "Pending",
            SectionState::Unavailable => "Unavailable",
        };
        if let Some(guard) = self.guard {
            guard
                .validate_presence(&section.key(), presence)
                .map_err(PhysicsQueryError::contract)?;
        }
        let SectionState::Ready(storage) = state else {
            return Ok(CellVisit::Unresolved(section));
        };
        let block_id = storage.read_world(cell.x, world_y, cell.z);
        if block_id.block_type().raw() == 0 {
            return Ok(CellVisit::Empty);
        }
        Ok(match self.materials.class_for(block_id.block_type()) {
            Some(class) if filter.contains(class) => CellVisit::Hit(block_id),
            Some(_) => CellVisit::Empty,
            None => return Err(PhysicsQueryError::unknown_material_class()),
        })
    }
}

/// Compares independent query results byte-for-byte at the semantic value
/// level. A mismatch is the contract's deterministic divergence rejection.
pub fn verify_deterministic<T: PartialEq>(left: &T, right: &T) -> Result<(), PhysicsQueryError> {
    if left == right {
        Ok(())
    } else {
        Err(PhysicsQueryError::query_result_divergence())
    }
}

enum CellVisit {
    Hit(BlockId),
    Empty,
    Unresolved(SectionId),
}

#[derive(Clone, Copy)]
enum Axis {
    X,
    Y,
    Z,
}

fn floor_i32(value: f32) -> i32 {
    value.floor() as i32
}

fn initial_cell(origin: f32, direction: f32) -> i32 {
    let floor = floor_i32(origin);
    if direction < -EPSILON && (origin - floor as f32).abs() <= EPSILON {
        floor.saturating_sub(1)
    } else {
        floor
    }
}

fn boundary_entry_normal(origin: Vec3, direction: Vec3) -> Vec3 {
    for (coordinate, component, axis) in [
        (origin.x, direction.x, Axis::X),
        (origin.y, direction.y, Axis::Y),
        (origin.z, direction.z, Axis::Z),
    ] {
        if component.abs() > EPSILON && (coordinate - coordinate.floor()).abs() <= EPSILON {
            return match axis {
                Axis::X => Vec3::new(-component.signum(), 0.0, 0.0),
                Axis::Y => Vec3::new(0.0, -component.signum(), 0.0),
                Axis::Z => Vec3::new(0.0, 0.0, -component.signum()),
            };
        }
    }
    Vec3::default()
}

fn dda_axis(origin: f32, direction: f32, cell: i32) -> (i32, f32, f32) {
    if direction > EPSILON {
        (1, (cell as f32 + 1.0 - origin) / direction, 1.0 / direction)
    } else if direction < -EPSILON {
        (-1, (origin - cell as f32) / -direction, 1.0 / -direction)
    } else {
        (0, f32::INFINITY, f32::INFINITY)
    }
}

fn min_axis(x: f32, y: f32, z: f32) -> (Axis, f32) {
    if x <= y && x <= z {
        (Axis::X, x)
    } else if y <= z {
        (Axis::Y, y)
    } else {
        (Axis::Z, z)
    }
}

const MAX_PHYSICS_QUERY_CELLS: usize = vw::MAX_CELLS_PER_READ_REQUEST as usize;

fn cell_range(shape: Aabb) -> Result<Option<(CellCoord, CellCoord)>, PhysicsQueryError> {
    let min = shape.min();
    let max = shape.max();
    if [min.x, min.y, min.z, max.x, max.y, max.z]
        .into_iter()
        .any(|value| !value.is_finite())
        || min.x > max.x
        || min.y > max.y
        || min.z > max.z
    {
        return Ok(None);
    }
    let x_min = min.x.floor() as i32;
    let y_min = min.y.floor().max(0.0) as i32;
    let z_min = min.z.floor() as i32;
    let x_max = (max.x - EPSILON).floor() as i32;
    let y_max = (max.y - EPSILON)
        .floor()
        .min(f32::from(vw::WORLD_Y_MAX as u8)) as i32;
    let z_max = (max.z - EPSILON).floor() as i32;
    if x_min > x_max || y_min > y_max || z_min > z_max {
        return Ok(None);
    }
    let x_count = (i64::from(x_max) - i64::from(x_min) + 1) as usize;
    let y_count = (i64::from(y_max) - i64::from(y_min) + 1) as usize;
    let z_count = (i64::from(z_max) - i64::from(z_min) + 1) as usize;
    let count = x_count
        .checked_mul(y_count)
        .and_then(|count| count.checked_mul(z_count))
        .ok_or_else(PhysicsQueryError::query_buffer_overflow)?;
    if count > MAX_PHYSICS_QUERY_CELLS {
        return Err(PhysicsQueryError::query_buffer_overflow());
    }
    Ok(Some((
        CellCoord::new(x_min, y_min, z_min),
        CellCoord::new(x_max, y_max, z_max),
    )))
}

fn swept_cell_range(
    start: Aabb,
    end: Aabb,
) -> Result<Option<(CellCoord, CellCoord)>, PhysicsQueryError> {
    let start_min = start.min();
    let start_max = start.max();
    let end_min = end.min();
    let end_max = end.max();
    cell_range(Aabb::new(
        Vec3::new(
            (start_min.x.min(end_min.x) + start_max.x.max(end_max.x)) * 0.5,
            (start_min.y.min(end_min.y) + start_max.y.max(end_max.y)) * 0.5,
            (start_min.z.min(end_min.z) + start_max.z.max(end_max.z)) * 0.5,
        ),
        Vec3::new(
            (start_max.x.max(end_max.x) - start_min.x.min(end_min.x)) * 0.5,
            (start_max.y.max(end_max.y) - start_min.y.min(end_min.y)) * 0.5,
            (start_max.z.max(end_max.z) - start_min.z.min(end_min.z)) * 0.5,
        ),
    ))
}

fn write_overlap(buffer: &mut [OverlapHit], written_count: &mut usize, hit: OverlapHit) {
    if *written_count < buffer.len() {
        buffer[*written_count] = hit;
        *written_count += 1;
    }
}

fn aabb_intersects(left: Aabb, right: Aabb) -> bool {
    let left_min = left.min();
    let left_max = left.max();
    let right_min = right.min();
    let right_max = right.max();
    left_min.x <= right_max.x
        && left_max.x >= right_min.x
        && left_min.y <= right_max.y
        && left_max.y >= right_min.y
        && left_min.z <= right_max.z
        && left_max.z >= right_min.z
}

fn ray_aabb(origin: Vec3, direction: Vec3, shape: Aabb) -> Option<(f32, Vec3)> {
    let min = shape.min();
    let max = shape.max();
    let mut entry = 0.0_f32;
    let mut exit = f32::INFINITY;
    let mut normal = Vec3::default();
    for (origin_axis, direction_axis, min_axis, max_axis, axis) in [
        (origin.x, direction.x, min.x, max.x, Axis::X),
        (origin.y, direction.y, min.y, max.y, Axis::Y),
        (origin.z, direction.z, min.z, max.z, Axis::Z),
    ] {
        if direction_axis.abs() <= EPSILON {
            if origin_axis < min_axis || origin_axis > max_axis {
                return None;
            }
            continue;
        }
        let inverse = 1.0 / direction_axis;
        let mut near = (min_axis - origin_axis) * inverse;
        let mut far = (max_axis - origin_axis) * inverse;
        let mut near_normal = match axis {
            Axis::X => Vec3::new(-direction_axis.signum(), 0.0, 0.0),
            Axis::Y => Vec3::new(0.0, -direction_axis.signum(), 0.0),
            Axis::Z => Vec3::new(0.0, 0.0, -direction_axis.signum()),
        };
        if near > far {
            std::mem::swap(&mut near, &mut far);
            near_normal = match axis {
                Axis::X => Vec3::new(direction_axis.signum(), 0.0, 0.0),
                Axis::Y => Vec3::new(0.0, direction_axis.signum(), 0.0),
                Axis::Z => Vec3::new(0.0, 0.0, direction_axis.signum()),
            };
        }
        if near > entry {
            entry = near;
            normal = near_normal;
        }
        exit = exit.min(far);
        if entry > exit {
            return None;
        }
    }
    if exit < 0.0 {
        None
    } else {
        Some((entry.max(0.0), normal))
    }
}

fn sweep_aabb(shape: Aabb, displacement: Vec3, cell: CellCoord) -> Option<(f32, Vec3)> {
    let expanded = Aabb::new(
        Vec3::new(
            cell.x as f32 + 0.5,
            cell.y as f32 + 0.5,
            cell.z as f32 + 0.5,
        ),
        Vec3::new(
            0.5 + shape.half_extents.x,
            0.5 + shape.half_extents.y,
            0.5 + shape.half_extents.z,
        ),
    );
    ray_aabb(shape.center, displacement, expanded)
        .filter(|(fraction, _)| *fraction <= 1.0 + EPSILON)
}
