//! Single-cut execute against one captured `PublishedReadView`.
//!
//! No implicit Load, no mixed cut, no writes, no recapture.

#![forbid(unsafe_code)]

use super::QueryError;
use super::budget;
use super::plan::QueryPlan;
use super::result_assembly::{GeneratedVoxelQueryOutcome, assemble};
use super::section_access;
use lumio_voxel_domain::publication::PublishedReadView;
use lumio_voxel_domain::section::SectionPresenceGuard;

pub struct QueryExecutor;

impl QueryExecutor {
    pub fn execute(
        plan: &QueryPlan,
        view: &PublishedReadView,
    ) -> Result<GeneratedVoxelQueryOutcome, QueryError> {
        Self::walk(plan, view, 0)
    }

    /// Execute a query while enforcing a caller-owned residency pin invariant.
    ///
    /// The guard is checked against every Section result in the same captured cut,
    /// before the outcome is returned to the caller.
    pub fn execute_with_presence_guard<G: SectionPresenceGuard>(
        plan: &QueryPlan,
        view: &PublishedReadView,
        guard: &G,
    ) -> Result<GeneratedVoxelQueryOutcome, QueryError> {
        Self::walk_with_presence_guard(plan, view, 0, guard)
    }

    /// Continue a cut-local walk charged against remaining budget.
    pub fn walk(
        plan: &QueryPlan,
        view: &PublishedReadView,
        already_used: usize,
    ) -> Result<GeneratedVoxelQueryOutcome, QueryError> {
        bind_cut(plan, view)?;
        walk_bound(plan, view, already_used, None)
    }

    pub fn walk_with_presence_guard<G: SectionPresenceGuard>(
        plan: &QueryPlan,
        view: &PublishedReadView,
        already_used: usize,
        guard: &G,
    ) -> Result<GeneratedVoxelQueryOutcome, QueryError> {
        bind_cut(plan, view)?;
        walk_bound(plan, view, already_used, Some(guard))
    }

    /// Generated cancel path. No directory walk or writes.
    pub fn execute_cancelled(
        plan: &QueryPlan,
        view: &PublishedReadView,
    ) -> Result<GeneratedVoxelQueryOutcome, QueryError> {
        bind_cut(plan, view)?;
        Err(QueryError::loader_cancelled())
    }
}

fn bind_cut(plan: &QueryPlan, view: &PublishedReadView) -> Result<(), QueryError> {
    let planned = plan.read_stamp();
    let observed = view.stamp();
    // Cut identity is world/context/generation/world_revision; do not recapture.
    if planned.world_id != observed.world_id
        || planned.context_id != observed.context_id
        || planned.generation != observed.generation
        || planned.world_revision != observed.world_revision
    {
        return Err(QueryError::invalid_handle());
    }
    if plan.cancel_token().is_empty() {
        return Err(QueryError::invalid_handle());
    }
    Ok(())
}

fn walk_bound(
    plan: &QueryPlan,
    view: &PublishedReadView,
    already_used: usize,
    guard: Option<&dyn SectionPresenceGuard>,
) -> Result<GeneratedVoxelQueryOutcome, QueryError> {
    if budget::exceeds(already_used, plan.budget()) {
        return Err(QueryError::budget_exceeded());
    }
    let mut used = already_used;
    let mut items = Vec::with_capacity(plan.canonical_sections().len());
    for section_id in plan.canonical_sections() {
        used = used
            .checked_add(1)
            .ok_or_else(QueryError::budget_exceeded)?;
        if budget::exceeds(used, plan.budget()) {
            return Err(QueryError::budget_exceeded());
        }
        let item = section_access::access(view, section_id)?;
        if let Some(guard) = guard {
            guard
                .validate_presence(item.section_id(), item.presence())
                .map_err(QueryError::contract)?;
        }
        items.push(item);
    }
    Ok(assemble(
        items,
        plan.read_stamp().clone(),
        used,
        plan.plan_hash(),
    ))
}
