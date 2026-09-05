//! Durability-fenced Section residency transitions.
//!
//! Residency is metadata around the published Section directory. It does not perform
//! storage I/O or choose an eviction policy. R-00440 supplies the pin policy through the
//! explicit hook required by `unload_section`.

#![forbid(unsafe_code)]

use super::WorldError;
use super::barrier::{BarrierScope, admit_scope};
use super::instance::VoxelWorld;
use lumio_voxel_contracts::voxel_world as vw;
use lumio_voxel_domain::key::{SectionId, WorldY};
use lumio_voxel_domain::publication::{PublishedReadView, PublishedStateRoot};
use lumio_voxel_domain::revision::WorldRevision;
use lumio_voxel_domain::section::{
    SectionDeltaBuilder, SectionDirectoryBuilder, SectionDirectoryRoot, SectionError,
    SectionPresenceGuard, SectionSlot,
};
use std::collections::{BTreeMap, BTreeSet};

/// R-00440's policy seam. This card does not inspect or maintain region pins.
pub trait PinExemptionHook {
    /// Validate that the requested Section may leave the resident set.
    fn check_pin_exemption(&mut self, section_id: &str) -> Result<(), PinExemptionError>;
}

impl<F> PinExemptionHook for F
where
    F: FnMut(&str) -> Result<(), PinExemptionError>,
{
    fn check_pin_exemption(&mut self, section_id: &str) -> Result<(), PinExemptionError> {
        self(section_id)
    }
}

/// The only pin-policy failure this residency boundary needs to map.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PinExemptionError {
    error_id: &'static str,
}

impl PinExemptionError {
    pub fn invalid_handle() -> Self {
        Self {
            error_id: "InvalidHandle",
        }
    }

    pub fn pinned_section_evicted() -> Self {
        Self {
            error_id: vw::intern_error_code("pinned_section_evicted")
                .expect("pin eviction error must be in the live contract"),
        }
    }

    pub fn error_id(self) -> &'static str {
        self.error_id
    }
}

impl std::fmt::Display for PinExemptionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.error_id)
    }
}

impl std::error::Error for PinExemptionError {}

/// Errors emitted by the region-pin lifecycle and its read/physics gates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegionPinError {
    InvalidHandle { error_id: &'static str },
    BudgetExceeded { error_id: &'static str },
    PinRegionNotReady { error_id: &'static str },
    PinnedReadReturnedPending { error_id: &'static str },
    WorldYOutOfRange { error_id: &'static str },
}

impl RegionPinError {
    pub fn error_id(self) -> &'static str {
        match self {
            Self::InvalidHandle { error_id }
            | Self::BudgetExceeded { error_id }
            | Self::PinRegionNotReady { error_id }
            | Self::PinnedReadReturnedPending { error_id }
            | Self::WorldYOutOfRange { error_id } => error_id,
        }
    }

    fn invalid_handle() -> Self {
        Self::InvalidHandle {
            error_id: "InvalidHandle",
        }
    }

    pub fn residency_pin_exceeds_budget() -> Self {
        Self::BudgetExceeded {
            error_id: contract_error("residency_pin_exceeds_budget"),
        }
    }

    pub fn pin_region_not_ready() -> Self {
        Self::PinRegionNotReady {
            error_id: contract_error("pin_region_not_ready"),
        }
    }

    pub fn pinned_read_returned_pending() -> Self {
        Self::PinnedReadReturnedPending {
            error_id: contract_error("pinned_read_returned_pending"),
        }
    }

    pub fn world_y_out_of_range() -> Self {
        Self::WorldYOutOfRange {
            error_id: contract_error("world_y_out_of_range"),
        }
    }
}

impl std::fmt::Display for RegionPinError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.error_id())
    }
}

impl std::error::Error for RegionPinError {}

/// Caller and host limits are both hard admission fences. No global default exists.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PinBudget {
    pub caller_sections: usize,
    pub host_sections: usize,
}

impl PinBudget {
    pub const fn new(caller_sections: usize, host_sections: usize) -> Self {
        Self {
            caller_sections,
            host_sections,
        }
    }
}

/// Opaque manager-local pin identity. IDs are not shared between worlds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PinId(u64);

impl PinId {
    pub fn value(self) -> u64 {
        self.0
    }
}

/// Pin readiness is distinct from Section presence. A declared pin may still be Pending.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PinReadiness {
    NotReady,
    Ready,
    Released,
}

impl PinReadiness {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotReady => "NotReady",
            Self::Ready => "Ready",
            Self::Released => "Released",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegionPinStatus {
    pin_id: PinId,
    readiness: PinReadiness,
    section_count: usize,
}

impl RegionPinStatus {
    pub fn pin_id(&self) -> PinId {
        self.pin_id
    }

    pub fn readiness(&self) -> PinReadiness {
        self.readiness
    }

    pub fn state(&self) -> PinReadiness {
        self.readiness()
    }

    pub fn section_count(&self) -> usize {
        self.section_count
    }

    pub fn is_ready(&self) -> bool {
        self.readiness == PinReadiness::Ready
    }
}

pub type PinStatus = RegionPinStatus;
pub type PinHandle = PinId;
pub type ResidencyPinError = RegionPinError;

#[derive(Clone, Debug)]
struct RegionPin {
    sections: BTreeSet<String>,
    readiness: PinReadiness,
}

/// Region pin declarations and the R-00440 readiness/budget fences.
///
/// The manager owns only declaration metadata. It does not load Sections, select streaming
/// priorities, or implement storage. Call `mark_ready` only after the caller has observed every
/// Section in the declaration reach `Ready` in one coherent world cut.
#[derive(Clone, Debug)]
pub struct RegionPinManager {
    budget: PinBudget,
    next_pin_id: u64,
    pins: BTreeMap<PinId, RegionPin>,
}

impl RegionPinManager {
    pub fn new(caller_sections: usize, host_sections: usize) -> Self {
        Self::with_budgets(caller_sections, host_sections)
    }

    pub fn from_budget(budget: PinBudget) -> Self {
        Self {
            budget,
            next_pin_id: 1,
            pins: BTreeMap::new(),
        }
    }

    pub fn with_budgets(caller_sections: usize, host_sections: usize) -> Self {
        Self::from_budget(PinBudget::new(caller_sections, host_sections))
    }

    pub fn budget(&self) -> PinBudget {
        self.budget
    }

    /// Admit all Section keys atomically. A rejected declaration changes no manager state.
    pub fn declare_pin<I, S>(&mut self, section_ids: I) -> Result<PinId, RegionPinError>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.declare_pin_with_budget(section_ids, self.budget)
    }

    pub fn declare_pin_with_budget<I, S>(
        &mut self,
        section_ids: I,
        budget: PinBudget,
    ) -> Result<PinId, RegionPinError>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let budget = PinBudget::new(
            budget.caller_sections.min(self.budget.caller_sections),
            budget.host_sections.min(self.budget.host_sections),
        );
        let max_requested = budget.caller_sections.min(budget.host_sections);
        let mut sections = BTreeSet::new();
        for raw in section_ids {
            if sections.len() >= max_requested {
                return Err(RegionPinError::residency_pin_exceeds_budget());
            }
            let raw = raw.into();
            let section = SectionId::parse(&raw).map_err(|_| RegionPinError::invalid_handle())?;
            if !sections.insert(section.key()) {
                return Err(RegionPinError::invalid_handle());
            }
        }
        if sections.is_empty() {
            return Err(RegionPinError::invalid_handle());
        }

        let currently_pinned = self.active_sections();
        let additional = sections
            .iter()
            .filter(|section| !currently_pinned.contains(*section))
            .count();
        let total = currently_pinned
            .len()
            .checked_add(additional)
            .ok_or_else(RegionPinError::residency_pin_exceeds_budget)?;
        if sections.len() > budget.caller_sections
            || sections.len() > budget.host_sections
            || total > budget.caller_sections
            || total > budget.host_sections
        {
            return Err(RegionPinError::residency_pin_exceeds_budget());
        }

        let pin_id = PinId(self.next_pin_id);
        self.next_pin_id = self
            .next_pin_id
            .checked_add(1)
            .ok_or_else(RegionPinError::invalid_handle)?;
        self.pins.insert(
            pin_id,
            RegionPin {
                sections,
                readiness: PinReadiness::NotReady,
            },
        );
        Ok(pin_id)
    }

    pub fn declare<I, S>(&mut self, section_ids: I) -> Result<PinId, RegionPinError>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.declare_pin(section_ids)
    }

    pub fn declare_with_budgets<I, S>(
        &mut self,
        section_ids: I,
        caller_sections: usize,
        host_sections: usize,
    ) -> Result<PinId, RegionPinError>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.declare_pin_with_budget(section_ids, PinBudget::new(caller_sections, host_sections))
    }

    pub fn status(&self, pin_id: PinId) -> Result<RegionPinStatus, RegionPinError> {
        let pin = self
            .pins
            .get(&pin_id)
            .ok_or_else(RegionPinError::invalid_handle)?;
        Ok(RegionPinStatus {
            pin_id,
            readiness: pin.readiness,
            section_count: pin.sections.len(),
        })
    }

    pub fn pin_status(&self, pin_id: PinId) -> Result<RegionPinStatus, RegionPinError> {
        self.status(pin_id)
    }

    pub fn readiness(&self, pin_id: PinId) -> Result<PinReadiness, RegionPinError> {
        Ok(self.status(pin_id)?.readiness())
    }

    pub fn is_ready(&self, pin_id: PinId) -> Result<bool, RegionPinError> {
        Ok(self.readiness(pin_id)? == PinReadiness::Ready)
    }

    pub fn sections(&self, pin_id: PinId) -> Result<Vec<String>, RegionPinError> {
        let pin = self
            .pins
            .get(&pin_id)
            .ok_or_else(RegionPinError::invalid_handle)?;
        Ok(pin.sections.iter().cloned().collect())
    }

    /// Explicit not-ready -> ready transition. Loading/settlement is owned by the caller.
    pub fn mark_ready(&mut self, pin_id: PinId) -> Result<(), RegionPinError> {
        let pin = self
            .pins
            .get_mut(&pin_id)
            .ok_or_else(RegionPinError::invalid_handle)?;
        if pin.readiness != PinReadiness::NotReady {
            return Err(RegionPinError::invalid_handle());
        }
        pin.readiness = PinReadiness::Ready;
        Ok(())
    }

    pub fn signal_ready(&mut self, pin_id: PinId) -> Result<(), RegionPinError> {
        self.mark_ready(pin_id)
    }

    /// Signal readiness only when the current world cut contains Ready slots for every
    /// Section in the declaration.
    pub fn mark_ready_from_world(
        &mut self,
        pin_id: PinId,
        world: &VoxelWorld,
    ) -> Result<(), RegionPinError> {
        let sections = self.sections(pin_id)?;
        let view = world.publication_authority().capture();
        let all_ready = sections.iter().all(|section_id| {
            view.directory()
                .lookup(section_id)
                .ok()
                .flatten()
                .is_some_and(|slot| slot.presence() == "Ready")
        });
        if !all_ready {
            return Err(RegionPinError::pin_region_not_ready());
        }
        self.mark_ready(pin_id)
    }

    /// Gameplay/settlement gate. Reads before ready remain valid Pending results.
    pub fn settle(&self, pin_id: PinId) -> Result<(), RegionPinError> {
        match self.status(pin_id)?.readiness() {
            PinReadiness::Ready => Ok(()),
            PinReadiness::NotReady => Err(RegionPinError::pin_region_not_ready()),
            PinReadiness::Released => Err(RegionPinError::invalid_handle()),
        }
    }

    pub fn ensure_gameplay_ready(&self, pin_id: PinId) -> Result<(), RegionPinError> {
        self.settle(pin_id)
    }

    pub fn release_pin(&mut self, pin_id: PinId) -> Result<(), RegionPinError> {
        let pin = self
            .pins
            .get_mut(&pin_id)
            .ok_or_else(RegionPinError::invalid_handle)?;
        if pin.readiness == PinReadiness::Released {
            return Err(RegionPinError::invalid_handle());
        }
        pin.readiness = PinReadiness::Released;
        Ok(())
    }

    pub fn release(&mut self, pin_id: PinId) -> Result<(), RegionPinError> {
        self.release_pin(pin_id)
    }

    pub fn is_pinned(&self, section_id: &str) -> Result<bool, RegionPinError> {
        let section = SectionId::parse(section_id).map_err(|_| RegionPinError::invalid_handle())?;
        let section = section.key();
        Ok(self
            .pins
            .values()
            .any(|pin| pin.readiness != PinReadiness::Released && pin.sections.contains(&section)))
    }

    pub fn pinned_section_count(&self) -> usize {
        self.active_sections().len()
    }

    pub fn active_pin_count(&self) -> usize {
        self.pins
            .values()
            .filter(|pin| pin.readiness != PinReadiness::Released)
            .count()
    }

    /// Pinned Sections stay out of ordinary streaming priority selection.
    pub fn streaming_priority_eligible(&self, section_id: &str) -> Result<bool, RegionPinError> {
        Ok(!self.is_pinned(section_id)?)
    }

    pub fn excluded_from_streaming_priority(
        &self,
        section_id: &str,
    ) -> Result<bool, RegionPinError> {
        Ok(!self.streaming_priority_eligible(section_id)?)
    }

    /// Before readiness, Pending/Unavailable are valid. Once ready, every pinned result
    /// must be Ready; callers use this gate for both block reads and physical queries.
    pub fn validate_pinned_read(
        &self,
        pin_id: PinId,
        section_id: &str,
        presence: &str,
    ) -> Result<(), RegionPinError> {
        let pin = self
            .pins
            .get(&pin_id)
            .ok_or_else(RegionPinError::invalid_handle)?;
        if pin.readiness == PinReadiness::Released {
            return Err(RegionPinError::invalid_handle());
        }
        let section = SectionId::parse(section_id).map_err(|_| RegionPinError::invalid_handle())?;
        if !pin.sections.contains(&section.key()) {
            return Err(RegionPinError::invalid_handle());
        }
        if !vw::SECTION_PRESENCE.contains(&presence) {
            return Err(RegionPinError::invalid_handle());
        }
        if pin.readiness == PinReadiness::Ready && presence != "Ready" {
            return Err(RegionPinError::pinned_read_returned_pending());
        }
        Ok(())
    }

    pub fn validate_physics_result(
        &self,
        pin_id: PinId,
        section_id: &str,
        presence: &str,
    ) -> Result<(), RegionPinError> {
        self.validate_pinned_read(pin_id, section_id, presence)
    }

    pub fn validate_query_result(
        &self,
        pin_id: PinId,
        section_id: &str,
        presence: &str,
    ) -> Result<(), RegionPinError> {
        self.validate_pinned_read(pin_id, section_id, presence)
    }

    /// Validate an observed Section result against every active pin covering it.
    ///
    /// This is the integration hook used by query and physics paths. A not-ready
    /// declaration may still observe Pending/Unavailable; once any covering pin is
    /// Ready, only a Ready result is admissible.
    pub fn validate_presence(
        &self,
        section_id: &str,
        presence: &str,
    ) -> Result<(), RegionPinError> {
        if !vw::SECTION_PRESENCE.contains(&presence) {
            return Err(RegionPinError::invalid_handle());
        }
        let section = SectionId::parse(section_id)
            .map_err(|_| RegionPinError::invalid_handle())?
            .key();
        if self
            .pins
            .values()
            .any(|pin| pin.readiness == PinReadiness::Ready && pin.sections.contains(&section))
            && presence != "Ready"
        {
            return Err(RegionPinError::pinned_read_returned_pending());
        }
        Ok(())
    }

    fn active_sections(&self) -> BTreeSet<String> {
        self.pins
            .values()
            .filter(|pin| pin.readiness != PinReadiness::Released)
            .flat_map(|pin| pin.sections.iter().cloned())
            .collect()
    }
}

impl PinExemptionHook for RegionPinManager {
    fn check_pin_exemption(&mut self, section_id: &str) -> Result<(), PinExemptionError> {
        if self
            .is_pinned(section_id)
            .map_err(|_| PinExemptionError::invalid_handle())?
        {
            return Err(PinExemptionError::pinned_section_evicted());
        }
        Ok(())
    }
}

impl SectionPresenceGuard for RegionPinManager {
    fn validate_presence(&self, section_id: &str, presence: &str) -> Result<(), &'static str> {
        RegionPinManager::validate_presence(self, section_id, presence)
            .map_err(RegionPinError::error_id)
    }
}

/// Expand an x/y/z block region into canonical Section keys in deterministic order.
pub fn section_keys_for_region(
    min_x: i32,
    max_x: i32,
    min_y: i64,
    max_y: i64,
    min_z: i32,
    max_z: i32,
    budget: PinBudget,
) -> Result<Vec<String>, RegionPinError> {
    if min_x > max_x || min_y > max_y || min_z > max_z {
        return Err(RegionPinError::invalid_handle());
    }
    let min_y = WorldY::new(min_y).map_err(|_| RegionPinError::world_y_out_of_range())?;
    let max_y = WorldY::new(max_y).map_err(|_| RegionPinError::world_y_out_of_range())?;
    let extent = vw::SECTION_EXTENT as i32;
    let min_section_x = min_x.div_euclid(extent);
    let max_section_x = max_x.div_euclid(extent);
    let min_section_y = min_y.section_y();
    let max_section_y = max_y.section_y();
    let min_section_z = min_z.div_euclid(extent);
    let max_section_z = max_z.div_euclid(extent);

    let span_x = inclusive_span(min_section_x, max_section_x);
    let span_y = inclusive_span(i32::from(min_section_y), i32::from(max_section_y));
    let span_z = inclusive_span(min_section_z, max_section_z);
    let count = span_x
        .checked_mul(span_y)
        .and_then(|count| count.checked_mul(span_z))
        .ok_or_else(RegionPinError::residency_pin_exceeds_budget)?;
    if count > budget.caller_sections.min(budget.host_sections) {
        return Err(RegionPinError::residency_pin_exceeds_budget());
    }

    let mut sections = BTreeSet::new();
    for x in min_section_x..=max_section_x {
        for y in min_section_y..=max_section_y {
            for z in min_section_z..=max_section_z {
                sections.insert(SectionId::new(x, i64::from(y), z).unwrap().key());
            }
        }
    }
    Ok(sections.into_iter().collect())
}

/// Conservative hook for callers that have no region-pin manager. Unload must
/// not bypass pin protection; callers with residency state should pass the
/// `RegionPinManager` itself.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoPinExemption;

impl PinExemptionHook for NoPinExemption {
    fn check_pin_exemption(&mut self, _section_id: &str) -> Result<(), PinExemptionError> {
        Err(PinExemptionError::pinned_section_evicted())
    }
}

fn inclusive_span(min: i32, max: i32) -> usize {
    usize::try_from(i64::from(max) - i64::from(min) + 1).unwrap_or(usize::MAX)
}

/// Evidence of one atomic residency publication.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnloadReceipt {
    section_id: String,
    old_root: [u8; 32],
    new_root: [u8; 32],
    pin_exemption_checked: bool,
}

impl UnloadReceipt {
    pub fn section_id(&self) -> &str {
        &self.section_id
    }

    pub fn old_root(&self) -> [u8; 32] {
        self.old_root
    }

    pub fn new_root(&self) -> [u8; 32] {
        self.new_root
    }

    pub fn pin_exemption_checked(&self) -> bool {
        self.pin_exemption_checked
    }
}

/// Unload one clean Ready Section after R-00440's pin-exemption hook approves it.
///
/// Unload changes only the directory presence (`Ready` -> `Unchanged`), so the
/// SectionRevision and WorldRevision remain the same. A dirty Section is rejected
/// before the hook and remains resident. Candidate selection is outside this boundary.
pub fn unload_section<H: PinExemptionHook + ?Sized>(
    world: &mut VoxelWorld,
    section_id: &str,
    pin_exemption: &mut H,
) -> Result<UnloadReceipt, WorldError> {
    let mut barrier = UnloadBarrier::acquire(world)?;
    barrier.enter()?;
    barrier.unload(section_id, pin_exemption)
}

/// Alias matching the stable streaming vocabulary while keeping the same fence.
pub fn request_unload<H: PinExemptionHook + ?Sized>(
    world: &mut VoxelWorld,
    section_id: &str,
    pin_exemption: &mut H,
) -> Result<UnloadReceipt, WorldError> {
    unload_section(world, section_id, pin_exemption)
}

/// Stable short name for the explicit unload operation.
pub fn unload<H: PinExemptionHook + ?Sized>(
    world: &mut VoxelWorld,
    section_id: &str,
    pin_exemption: &mut H,
) -> Result<UnloadReceipt, WorldError> {
    unload_section(world, section_id, pin_exemption)
}

struct UnloadBarrier<'a> {
    world: &'a mut VoxelWorld,
}

impl<'a> UnloadBarrier<'a> {
    fn acquire(world: &'a mut VoxelWorld) -> Result<Self, WorldError> {
        if world.instance.write_occupied {
            return Err(WorldError::mapped("HandleDoubleRelease"));
        }
        world.instance.write_occupied = true;
        Ok(Self { world })
    }

    fn enter(&self) -> Result<(), WorldError> {
        admit_scope(self.world, BarrierScope::StreamingApply)
    }

    fn unload<H: PinExemptionHook + ?Sized>(
        &mut self,
        section_id: &str,
        pin_exemption: &mut H,
    ) -> Result<UnloadReceipt, WorldError> {
        let before = self.world.instance.authority.capture();
        let slot = before
            .directory()
            .lookup(section_id)
            .map_err(map_section)?
            .ok_or_else(|| WorldError::mapped("section_unavailable"))?;
        if before
            .dirty_frontier()
            .latest_revision(section_id)
            .map_err(|err| WorldError::mapped(err.error_id()))?
            .is_some()
        {
            return Err(WorldError::mapped("dirty_section_not_durable"));
        }
        match slot.presence() {
            "Ready" => {}
            "Unchanged" => {
                return Ok(UnloadReceipt {
                    section_id: section_id.to_string(),
                    old_root: before.root().identity(),
                    new_root: before.root().identity(),
                    pin_exemption_checked: false,
                });
            }
            "Pending" | "Unavailable" => {
                return Err(WorldError::mapped("section_unavailable"));
            }
            _ => return Err(WorldError::invalid_handle()),
        }
        // An attached world policy is authoritative even when a caller supplies
        // an additional hook. This closes the unload -> Pending escape hatch for
        // Sections whose pin has already reached Ready.
        if let Some(manager) = self.world.instance.region_pins.as_mut() {
            manager
                .check_pin_exemption(section_id)
                .map_err(|err| WorldError::mapped(err.error_id()))?;
        }
        pin_exemption
            .check_pin_exemption(section_id)
            .map_err(|err| WorldError::mapped(err.error_id()))?;
        self.publish(section_id, before)
    }

    fn publish(
        &mut self,
        section_id: &str,
        before: PublishedReadView,
    ) -> Result<UnloadReceipt, WorldError> {
        // Recheck the immutable cut after acquiring occupancy. This makes the operation
        // safe if another future residency path is added beside this one.
        let current = self.world.instance.authority.capture();
        if current.root().identity() != before.root().identity() {
            return Err(WorldError::mapped("SnapshotBaseMismatch"));
        }
        if current
            .dirty_frontier()
            .latest_revision(section_id)
            .map_err(|err| WorldError::mapped(err.error_id()))?
            .is_some()
        {
            return Err(WorldError::mapped("dirty_section_not_durable"));
        }
        let slot = current
            .directory()
            .lookup(section_id)
            .map_err(map_section)?
            .ok_or_else(|| WorldError::mapped("section_unavailable"))?;
        if slot.presence() != "Ready" {
            return Err(WorldError::mapped("section_unavailable"));
        }

        let mut replacement_builder = SectionDeltaBuilder::new(current.directory());
        replacement_builder
            .stage((section_id, SectionSlot::unchanged()))
            .map_err(map_section)?;
        let replacement = replacement_builder.freeze().map_err(map_section)?;
        let directory = directory_with_unloaded_section(&current, section_id)?;
        let root = PublishedStateRoot::new(
            current.stamp().clone(),
            directory,
            current.dirty_frontier().clone(),
        );
        let mut prepared = self
            .world
            .instance
            .authority
            .prepare(
                world_revision(current.stamp().world_revision)?,
                root,
                replacement,
            )
            .map_err(|err| WorldError::mapped(err.error_id()))?;
        let token = prepared
            .seal()
            .map_err(|err| WorldError::mapped(err.error_id()))?;
        let old_root = current.root().identity();
        let published = self
            .world
            .instance
            .authority
            .publish_once(token)
            .map_err(|err| WorldError::mapped(err.error_id()))?;
        Ok(UnloadReceipt {
            section_id: section_id.to_string(),
            old_root,
            new_root: published.root().identity(),
            pin_exemption_checked: true,
        })
    }
}

impl Drop for UnloadBarrier<'_> {
    fn drop(&mut self) {
        self.world.instance.write_occupied = false;
    }
}

fn directory_with_unloaded_section(
    view: &PublishedReadView,
    section_id: &str,
) -> Result<SectionDirectoryRoot, WorldError> {
    let mut builder = SectionDirectoryBuilder::new();
    let mut saw_target = false;
    for (id, slot) in view.directory().iter() {
        let id = id.key();
        if id == section_id {
            builder
                .insert(&id, SectionSlot::unchanged())
                .map_err(map_section)?;
            saw_target = true;
        } else {
            builder.insert(&id, slot.clone()).map_err(map_section)?;
        }
    }
    if !saw_target {
        builder
            .insert(section_id, SectionSlot::unchanged())
            .map_err(map_section)?;
    }
    Ok(builder.freeze())
}

fn map_section(err: SectionError) -> WorldError {
    WorldError::mapped(err.error_id())
}

fn contract_error(id: &'static str) -> &'static str {
    vw::intern_error_code(id).expect("region pin error must be declared in the live contract")
}

fn world_revision(n: u64) -> Result<WorldRevision, WorldError> {
    Ok(WorldRevision::from_raw(n))
}
