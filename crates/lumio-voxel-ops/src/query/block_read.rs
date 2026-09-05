//! Gameplay block reads over an immutable SectionStorage view.
//!
//! Requests are admitted as whole operations. The source is borrowed immutably, so reads
//! cannot publish writes or trigger loading as a side effect.

#![forbid(unsafe_code)]

use super::QueryError;
use lumio_voxel_contracts::voxel_world as vw;
use lumio_voxel_domain::block::{BlockId, CellOffset, WorldY};
use lumio_voxel_domain::key::SectionId;
use lumio_voxel_domain::publication::PublishedReadView;
use lumio_voxel_domain::section::{SectionPresenceGuard, SectionStorage, SectionStorageResolver};
use std::collections::BTreeMap;
use std::ops::RangeInclusive;
use std::sync::Arc;

/// Contract hard limit. A request over this limit is rejected in full.
pub const MAX_CELLS_PER_READ_REQUEST: usize = vw::MAX_CELLS_PER_READ_REQUEST as usize;

/// A validated y range for a column read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ColumnYRange {
    start: i64,
    end: i64,
}

impl ColumnYRange {
    pub fn new(start: i64, end: i64) -> Self {
        Self { start, end }
    }

    pub const fn start(self) -> i64 {
        self.start
    }

    pub const fn end(self) -> i64 {
        self.end
    }

    fn inclusive_len(self) -> Result<usize, QueryError> {
        if self.start > self.end {
            return Err(QueryError::invalid_handle());
        }
        if self.start < i64::from(vw::WORLD_Y_MIN) {
            return Err(QueryError::from_world_y(
                WorldY::new(self.start).expect_err("range start must be rejected"),
            ));
        }
        if self.end > i64::from(vw::WORLD_Y_MAX) {
            return Err(QueryError::from_world_y(
                WorldY::new(self.end).expect_err("range end must be rejected"),
            ));
        }
        let len = (self.end - self.start + 1) as usize;
        admit_budget(len)?;
        Ok(len)
    }
}

impl<T> From<RangeInclusive<T>> for ColumnYRange
where
    T: Copy + Into<i64>,
{
    fn from(range: RangeInclusive<T>) -> Self {
        Self::new((*range.start()).into(), (*range.end()).into())
    }
}

impl From<(i64, i64)> for ColumnYRange {
    fn from((start, end): (i64, i64)) -> Self {
        Self::new(start, end)
    }
}

/// One cell's result. `block_id` is absent for Pending and Unavailable sections.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReadCell {
    offset: CellOffset,
    block_id: Option<BlockId>,
}

impl ReadCell {
    pub fn offset(&self) -> CellOffset {
        self.offset
    }

    pub fn block_id(&self) -> Option<BlockId> {
        self.block_id
    }
}

/// One Section segment in a block-read result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReadSegment {
    section_id: String,
    presence: &'static str,
    section_revision: u64,
    cells: Vec<ReadCell>,
    block_ids: Option<Vec<BlockId>>,
}

impl ReadSegment {
    pub fn section_id(&self) -> &str {
        &self.section_id
    }

    pub fn presence(&self) -> &'static str {
        self.presence
    }

    pub fn section_revision(&self) -> u64 {
        self.section_revision
    }

    pub fn cells(&self) -> &[ReadCell] {
        &self.cells
    }

    /// Resolved segments expose their BlockId array. Missing segments have no array.
    pub fn block_ids(&self) -> Option<&[BlockId]> {
        self.block_ids.as_deref()
    }

    pub fn is_resolved(&self) -> bool {
        self.block_ids.is_some()
    }
}

/// Result shared by box and column reads. Segment order follows first encounter in y/z/x order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlockReadResult {
    segments: Vec<ReadSegment>,
    cell_count: usize,
}

/// Caller-owned metadata for one contiguous Section run in y/z/x result order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BufferedReadSegment {
    section_id: SectionId,
    presence: &'static str,
    section_revision: u64,
    first_cell: usize,
    cell_count: usize,
}

/// One allocation-free cell observation supplied to a caller-owned sink.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BufferedReadCell {
    section_id: SectionId,
    presence: &'static str,
    section_revision: u64,
    cell: ReadCell,
}

impl BufferedReadCell {
    pub const fn section_id(self) -> SectionId {
        self.section_id
    }

    pub const fn presence(self) -> &'static str {
        self.presence
    }

    pub const fn section_revision(self) -> u64 {
        self.section_revision
    }

    pub fn offset(self) -> CellOffset {
        self.cell.offset()
    }

    pub fn block_id(self) -> Option<BlockId> {
        self.cell.block_id()
    }
}

impl BufferedReadSegment {
    pub const fn section_id(self) -> SectionId {
        self.section_id
    }

    pub const fn presence(self) -> &'static str {
        self.presence
    }

    pub const fn section_revision(self) -> u64 {
        self.section_revision
    }

    pub const fn first_cell(self) -> usize {
        self.first_cell
    }

    pub const fn cell_count(self) -> usize {
        self.cell_count
    }
}

/// Allocation-free summary returned by the caller-buffer read path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BufferedBlockReadResult {
    cell_count: usize,
    segment_count: usize,
    fully_resolved: bool,
}

impl BufferedBlockReadResult {
    pub const fn cell_count(self) -> usize {
        self.cell_count
    }

    pub const fn segment_count(self) -> usize {
        self.segment_count
    }

    pub const fn is_fully_resolved(self) -> bool {
        self.fully_resolved
    }
}

impl BlockReadResult {
    pub fn segments(&self) -> &[ReadSegment] {
        &self.segments
    }

    pub fn cell_count(&self) -> usize {
        self.cell_count
    }

    pub fn is_fully_resolved(&self) -> bool {
        self.segments.iter().all(ReadSegment::is_resolved)
    }

    pub fn section_revisions(&self) -> impl Iterator<Item = (&str, u64)> {
        self.segments
            .iter()
            .map(|segment| (segment.section_id(), segment.section_revision()))
    }

    /// Enforce a residency guard over every segment before exposing the result.
    pub fn validate_presence_guard<G: SectionPresenceGuard + ?Sized>(
        &self,
        guard: &G,
    ) -> Result<(), QueryError> {
        for segment in &self.segments {
            guard
                .validate_presence(segment.section_id(), segment.presence())
                .map_err(QueryError::contract)?;
        }
        Ok(())
    }
}

/// One-cell result. It remains a segment, including presence and section revision.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CellReadResult {
    section_id: String,
    presence: &'static str,
    section_revision: u64,
    cell: ReadCell,
}

impl CellReadResult {
    pub fn section_id(&self) -> &str {
        &self.section_id
    }

    pub fn presence(&self) -> &'static str {
        self.presence
    }

    pub fn section_revision(&self) -> u64 {
        self.section_revision
    }

    pub fn cell(&self) -> &ReadCell {
        &self.cell
    }

    pub fn offset(&self) -> CellOffset {
        self.cell.offset()
    }

    pub fn block_id(&self) -> Option<BlockId> {
        self.cell.block_id()
    }

    pub fn validate_presence_guard<G: SectionPresenceGuard + ?Sized>(
        &self,
        guard: &G,
    ) -> Result<(), QueryError> {
        guard
            .validate_presence(self.section_id(), self.presence())
            .map_err(QueryError::contract)
    }
}

/// Validated source segment. Ready and Unchanged carry storage; missing states do not.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlockReadSection {
    presence: &'static str,
    section_revision: u64,
    storage: Option<SectionStorage>,
}

impl BlockReadSection {
    pub fn ready(section_revision: u64, storage: SectionStorage) -> Self {
        Self {
            presence: intern_presence("Ready"),
            section_revision,
            storage: Some(storage),
        }
    }

    pub fn unchanged(section_revision: u64, storage: SectionStorage) -> Self {
        Self {
            presence: intern_presence("Unchanged"),
            section_revision,
            storage: Some(storage),
        }
    }

    pub fn pending(section_revision: u64) -> Self {
        Self {
            presence: intern_presence("Pending"),
            section_revision,
            storage: None,
        }
    }

    pub fn unavailable(section_revision: u64) -> Self {
        Self {
            presence: intern_presence("Unavailable"),
            section_revision,
            storage: None,
        }
    }

    /// Construct from a wire presence and optional revision, rejecting a missing revision.
    pub fn from_parts(
        presence: &str,
        section_revision: Option<u64>,
        storage: Option<SectionStorage>,
    ) -> Result<Self, QueryError> {
        let presence = vw::intern_presence(presence)
            .ok_or_else(|| QueryError::contract("cell_read_missing_presence"))?;
        let section_revision =
            section_revision.ok_or_else(|| QueryError::contract("read_result_missing_revision"))?;
        match presence {
            "Ready" if storage.is_some() => Ok(Self {
                presence,
                section_revision,
                storage,
            }),
            "Unchanged" if storage.is_some() => Ok(Self {
                presence,
                section_revision,
                storage,
            }),
            "Pending" | "Unavailable" if storage.is_none() => Ok(Self {
                presence,
                section_revision,
                storage,
            }),
            _ => Err(QueryError::invalid_handle()),
        }
    }

    pub fn presence(&self) -> &'static str {
        self.presence
    }

    pub fn section_revision(&self) -> u64 {
        self.section_revision
    }
}

/// Immutable SectionStorage map consumed by gameplay reads.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BlockReadWorld {
    sections: Arc<BTreeMap<SectionId, BlockReadSection>>,
}

impl BlockReadWorld {
    pub fn new() -> Self {
        Self::default()
    }

    /// Materialize the read source from one immutable published cut.
    pub fn from_published_view(view: &PublishedReadView) -> Result<Self, QueryError> {
        Self::from_published_view_with_baseline(view, &|_: &SectionId| None)
    }

    /// Materialize a published cut, resolving zero-byte `Unchanged` tickets
    /// against the immutable original-map baseline.
    pub fn from_published_view_with_baseline<R>(
        view: &PublishedReadView,
        baseline: &R,
    ) -> Result<Self, QueryError>
    where
        R: SectionStorageResolver + ?Sized,
    {
        let mut world = Self::new();
        for (id, slot) in view.directory().iter() {
            let key = id.key();
            let revision = view
                .stamp()
                .section_revision_set
                .get(&key)
                .copied()
                .unwrap_or(view.stamp().world_revision);
            let section = match slot.presence() {
                "Ready" => BlockReadSection::from_parts(
                    "Ready",
                    Some(revision),
                    slot.payload()
                        .and_then(|payload| payload.storage().cloned()),
                ),
                "Unchanged" => {
                    BlockReadSection::from_parts("Unchanged", Some(revision), baseline.resolve(id))
                }
                "Pending" => BlockReadSection::from_parts("Pending", Some(revision), None),
                "Unavailable" => BlockReadSection::from_parts("Unavailable", Some(revision), None),
                _ => Err(QueryError::invalid_handle()),
            }
            .map_err(|_| QueryError::contract(vw::SECTION_ENCODING_MISMATCH))?;
            world.insert(&key, section)?;
        }
        Ok(world)
    }

    pub fn from_sections<I, S>(sections: I) -> Result<Self, QueryError>
    where
        I: IntoIterator<Item = (S, BlockReadSection)>,
        S: AsRef<str>,
    {
        let mut world = Self::new();
        for (section_id, section) in sections {
            world.insert(section_id.as_ref(), section)?;
        }
        Ok(world)
    }

    pub fn insert(
        &mut self,
        section_id: &str,
        section: BlockReadSection,
    ) -> Result<(), QueryError> {
        let id = SectionId::parse(section_id).map_err(QueryError::from_key)?;
        let entries = Arc::make_mut(&mut self.sections);
        match entries.entry(id) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(section);
                Ok(())
            }
            std::collections::btree_map::Entry::Occupied(_) => Err(QueryError::invalid_handle()),
        }
    }

    pub fn with_section(
        mut self,
        section_id: &str,
        section: BlockReadSection,
    ) -> Result<Self, QueryError> {
        self.insert(section_id, section)?;
        Ok(self)
    }

    pub fn section(&self, section_id: &str) -> Result<Option<&BlockReadSection>, QueryError> {
        let id = SectionId::parse(section_id).map_err(QueryError::from_key)?;
        Ok(self.sections.get(&id))
    }

    pub fn read_cell<X, Y, Z>(&self, x: X, y: Y, z: Z) -> Result<CellReadResult, QueryError>
    where
        X: TryInto<i32>,
        Y: TryInto<i64>,
        Z: TryInto<i32>,
    {
        read_cell(self, x, y, z)
    }

    pub fn read_cell_with_presence_guard<X, Y, Z, G>(
        &self,
        x: X,
        y: Y,
        z: Z,
        guard: &G,
    ) -> Result<CellReadResult, QueryError>
    where
        X: TryInto<i32>,
        Y: TryInto<i64>,
        Z: TryInto<i32>,
        G: SectionPresenceGuard + ?Sized,
    {
        let result = read_cell(self, x, y, z)?;
        result.validate_presence_guard(guard)?;
        Ok(result)
    }

    pub fn read_box(
        &self,
        min: (i32, i64, i32),
        max: (i32, i64, i32),
    ) -> Result<BlockReadResult, QueryError> {
        read_box(self, min, max)
    }

    pub fn read_box_with_presence_guard<G: SectionPresenceGuard + ?Sized>(
        &self,
        min: (i32, i64, i32),
        max: (i32, i64, i32),
        guard: &G,
    ) -> Result<BlockReadResult, QueryError> {
        let result = read_box(self, min, max)?;
        result.validate_presence_guard(guard)?;
        Ok(result)
    }

    pub fn read_column<R: Into<ColumnYRange>>(
        &self,
        x: i32,
        z: i32,
        range: R,
    ) -> Result<BlockReadResult, QueryError> {
        read_column(self, x, z, range)
    }

    pub fn read_column_with_presence_guard<R, G>(
        &self,
        x: i32,
        z: i32,
        range: R,
        guard: &G,
    ) -> Result<BlockReadResult, QueryError>
    where
        R: Into<ColumnYRange>,
        G: SectionPresenceGuard + ?Sized,
    {
        let result = read_column(self, x, z, range)?;
        result.validate_presence_guard(guard)?;
        Ok(result)
    }

    pub fn read_cell_into<X, Y, Z>(
        &self,
        x: X,
        y: Y,
        z: Z,
        block_id: &mut Option<BlockId>,
    ) -> Result<CellReadResult, QueryError>
    where
        X: TryInto<i32>,
        Y: TryInto<i64>,
        Z: TryInto<i32>,
    {
        read_cell_into(self, x, y, z, block_id)
    }

    pub fn read_cell_into_with_presence_guard<X, Y, Z, G>(
        &self,
        x: X,
        y: Y,
        z: Z,
        block_id: &mut Option<BlockId>,
        guard: &G,
    ) -> Result<CellReadResult, QueryError>
    where
        X: TryInto<i32>,
        Y: TryInto<i64>,
        Z: TryInto<i32>,
        G: SectionPresenceGuard + ?Sized,
    {
        let result = read_cell(self, x, y, z)?;
        result.validate_presence_guard(guard)?;
        *block_id = result.block_id();
        Ok(result)
    }

    pub fn read_box_into(
        &self,
        min: (i32, i64, i32),
        max: (i32, i64, i32),
        block_ids: &mut [Option<BlockId>],
        segments: &mut [Option<BufferedReadSegment>],
    ) -> Result<BufferedBlockReadResult, QueryError> {
        read_box_into(self, min, max, block_ids, segments)
    }

    pub fn read_box_into_with_presence_guard<G: SectionPresenceGuard + ?Sized>(
        &self,
        min: (i32, i64, i32),
        max: (i32, i64, i32),
        block_ids: &mut [Option<BlockId>],
        segments: &mut [Option<BufferedReadSegment>],
        guard: &G,
    ) -> Result<BufferedBlockReadResult, QueryError> {
        // Preflight the immutable result so a rejected pinned read cannot write
        // partial values to caller-owned buffers.
        let result = read_box(self, min, max)?;
        result.validate_presence_guard(guard)?;
        read_box_into(self, min, max, block_ids, segments)
    }

    pub fn read_column_into<R: Into<ColumnYRange>>(
        &self,
        x: i32,
        z: i32,
        range: R,
        block_ids: &mut [Option<BlockId>],
        segments: &mut [Option<BufferedReadSegment>],
    ) -> Result<BufferedBlockReadResult, QueryError> {
        read_column_into(self, x, z, range, block_ids, segments)
    }

    pub fn read_column_into_with_presence_guard<R, G>(
        &self,
        x: i32,
        z: i32,
        range: R,
        block_ids: &mut [Option<BlockId>],
        segments: &mut [Option<BufferedReadSegment>],
        guard: &G,
    ) -> Result<BufferedBlockReadResult, QueryError>
    where
        R: Into<ColumnYRange>,
        G: SectionPresenceGuard + ?Sized,
    {
        let range = range.into();
        let result = read_column(self, x, z, range)?;
        result.validate_presence_guard(guard)?;
        read_column_into(self, x, z, range, block_ids, segments)
    }

    pub fn visit_box<C, S>(
        &self,
        min: (i32, i64, i32),
        max: (i32, i64, i32),
        on_cell: C,
        on_segment: S,
    ) -> Result<BufferedBlockReadResult, QueryError>
    where
        C: FnMut(usize, BufferedReadCell),
        S: FnMut(usize, BufferedReadSegment),
    {
        let count = box_cell_count(min, max)?;
        let (min_x, min_y, min_z) = min;
        let (max_x, max_y, max_z) = max;
        self.visit_cells(
            || {
                (min_y..=max_y).flat_map(move |y| {
                    (min_z..=max_z).flat_map(move |z| (min_x..=max_x).map(move |x| (x, y, z)))
                })
            },
            count,
            on_cell,
            on_segment,
        )
    }

    pub fn visit_column<R, C, S>(
        &self,
        x: i32,
        z: i32,
        range: R,
        on_cell: C,
        on_segment: S,
    ) -> Result<BufferedBlockReadResult, QueryError>
    where
        R: Into<ColumnYRange>,
        C: FnMut(usize, BufferedReadCell),
        S: FnMut(usize, BufferedReadSegment),
    {
        let range = range.into();
        let count = range.inclusive_len()?;
        self.visit_cells(
            || (range.start..=range.end).map(move |y| (x, y, z)),
            count,
            on_cell,
            on_segment,
        )
    }

    pub fn visit_box_with_presence_guard<C, S, G>(
        &self,
        min: (i32, i64, i32),
        max: (i32, i64, i32),
        on_cell: C,
        on_segment: S,
        guard: &G,
    ) -> Result<BufferedBlockReadResult, QueryError>
    where
        C: FnMut(usize, BufferedReadCell),
        S: FnMut(usize, BufferedReadSegment),
        G: SectionPresenceGuard + ?Sized,
    {
        let result = read_box(self, min, max)?;
        result.validate_presence_guard(guard)?;
        self.visit_box(min, max, on_cell, on_segment)
    }

    pub fn visit_column_with_presence_guard<R, C, S, G>(
        &self,
        x: i32,
        z: i32,
        range: R,
        on_cell: C,
        on_segment: S,
        guard: &G,
    ) -> Result<BufferedBlockReadResult, QueryError>
    where
        R: Into<ColumnYRange>,
        C: FnMut(usize, BufferedReadCell),
        S: FnMut(usize, BufferedReadSegment),
        G: SectionPresenceGuard + ?Sized,
    {
        let range = range.into();
        let result = read_column(self, x, z, range)?;
        result.validate_presence_guard(guard)?;
        self.visit_column(x, z, range, on_cell, on_segment)
    }

    fn cell_at(
        &self,
        x: i32,
        y: i64,
        z: i32,
    ) -> Result<(String, &'static str, u64, ReadCell), QueryError> {
        let (section_id, presence, revision, cell) = self.cell_at_section(x, y, z)?;
        Ok((section_id.key(), presence, revision, cell))
    }

    fn cell_at_section(
        &self,
        x: i32,
        y: i64,
        z: i32,
    ) -> Result<(SectionId, &'static str, u64, ReadCell), QueryError> {
        let world_y = WorldY::new(y).map_err(QueryError::from_world_y)?;
        let section_id = SectionId::new(
            x.div_euclid(16),
            i64::from(world_y.section_y()),
            z.div_euclid(16),
        )
        .map_err(QueryError::from_key)?;
        let offset = CellOffset::from_world(x, world_y, z);
        let section = self.sections.get(&section_id);
        let (presence, revision, block_id) = match section {
            Some(section) => {
                let block_id = match section.storage.as_ref() {
                    Some(storage)
                        if section.presence == "Ready" || section.presence == "Unchanged" =>
                    {
                        Some(storage.read(offset))
                    }
                    Some(_) => return Err(QueryError::invalid_handle()),
                    None => None,
                };
                (section.presence, section.section_revision, block_id)
            }
            None => (intern_presence("Unavailable"), 0, None),
        };
        Ok((
            section_id,
            presence,
            revision,
            ReadCell { offset, block_id },
        ))
    }

    fn read_cells<I>(&self, cells: I, count: usize) -> Result<BlockReadResult, QueryError>
    where
        I: IntoIterator<Item = (i32, i64, i32)>,
    {
        admit_budget(count)?;
        let mut segments = Vec::<SegmentBuilder>::new();
        let mut indexes = BTreeMap::<String, usize>::new();
        for (x, y, z) in cells {
            let (section_id, presence, revision, cell) = self.cell_at(x, y, z)?;
            let index = match indexes.get(&section_id) {
                Some(index) => *index,
                None => {
                    let index = segments.len();
                    indexes.insert(section_id.clone(), index);
                    segments.push(SegmentBuilder::new(
                        section_id,
                        presence,
                        revision,
                        cell.block_id.is_some(),
                    ));
                    index
                }
            };
            let segment = &mut segments[index];
            if segment.presence != presence || segment.section_revision != revision {
                return Err(QueryError::invalid_handle());
            }
            segment.push(cell);
        }
        Ok(BlockReadResult {
            segments: segments.into_iter().map(SegmentBuilder::finish).collect(),
            cell_count: count,
        })
    }

    fn read_cells_into_buffers<I, F>(
        &self,
        cells: F,
        count: usize,
        block_ids: &mut [Option<BlockId>],
        segments: &mut [Option<BufferedReadSegment>],
    ) -> Result<BufferedBlockReadResult, QueryError>
    where
        I: IntoIterator<Item = (i32, i64, i32)>,
        F: Fn() -> I,
    {
        let summary = self.preflight_cells(&cells, count)?;
        if block_ids.len() != count {
            return Err(QueryError::invalid_handle());
        }
        if segments.len() < summary.segment_count {
            return Err(QueryError::invalid_handle());
        }
        self.visit_cells_after_preflight(
            cells,
            summary,
            |index, cell| block_ids[index] = cell.block_id(),
            |index, segment| segments[index] = Some(segment),
        );
        Ok(summary)
    }

    fn visit_cells<I, F, C, S>(
        &self,
        cells: F,
        count: usize,
        on_cell: C,
        on_segment: S,
    ) -> Result<BufferedBlockReadResult, QueryError>
    where
        I: IntoIterator<Item = (i32, i64, i32)>,
        F: Fn() -> I,
        C: FnMut(usize, BufferedReadCell),
        S: FnMut(usize, BufferedReadSegment),
    {
        let summary = self.preflight_cells(&cells, count)?;
        self.visit_cells_after_preflight(cells, summary, on_cell, on_segment);
        Ok(summary)
    }

    fn preflight_cells<I, F>(
        &self,
        cells: &F,
        count: usize,
    ) -> Result<BufferedBlockReadResult, QueryError>
    where
        I: IntoIterator<Item = (i32, i64, i32)>,
        F: Fn() -> I,
    {
        admit_budget(count)?;
        let mut segment_count = 0_usize;
        let mut previous = None;
        let mut fully_resolved = true;
        for (x, y, z) in cells() {
            let (section_id, presence, revision, cell) = self.cell_at_section(x, y, z)?;
            let key = (section_id, presence, revision);
            if previous != Some(key) {
                segment_count = segment_count
                    .checked_add(1)
                    .ok_or_else(QueryError::invalid_handle)?;
                previous = Some(key);
            }
            fully_resolved &= cell.block_id.is_some();
        }
        Ok(BufferedBlockReadResult {
            cell_count: count,
            segment_count,
            fully_resolved,
        })
    }

    fn visit_cells_after_preflight<I, F, C, S>(
        &self,
        cells: F,
        summary: BufferedBlockReadResult,
        mut on_cell: C,
        mut on_segment: S,
    ) where
        I: IntoIterator<Item = (i32, i64, i32)>,
        F: Fn() -> I,
        C: FnMut(usize, BufferedReadCell),
        S: FnMut(usize, BufferedReadSegment),
    {
        let mut current = None::<BufferedReadSegment>;
        let mut segment_index = 0_usize;
        for (position, (x, y, z)) in cells().into_iter().enumerate() {
            let (section_id, presence, section_revision, cell) = self
                .cell_at_section(x, y, z)
                .expect("immutable read preflight validated every coordinate");
            on_cell(
                position,
                BufferedReadCell {
                    section_id,
                    presence,
                    section_revision,
                    cell,
                },
            );
            let same_segment = current.is_some_and(|segment| {
                segment.section_id == section_id
                    && segment.presence == presence
                    && segment.section_revision == section_revision
            });
            if same_segment {
                current.as_mut().expect("segment exists").cell_count += 1;
                continue;
            }
            if let Some(segment) = current.replace(BufferedReadSegment {
                section_id,
                presence,
                section_revision,
                first_cell: position,
                cell_count: 1,
            }) {
                on_segment(segment_index, segment);
                segment_index += 1;
            }
        }
        if let Some(segment) = current {
            on_segment(segment_index, segment);
            segment_index += 1;
        }
        debug_assert_eq!(segment_index, summary.segment_count);
    }
}

/// Read one world coordinate.
pub fn read_cell<X, Y, Z>(
    world: &BlockReadWorld,
    x: X,
    y: Y,
    z: Z,
) -> Result<CellReadResult, QueryError>
where
    X: TryInto<i32>,
    Y: TryInto<i64>,
    Z: TryInto<i32>,
{
    let x = x.try_into().map_err(|_| QueryError::invalid_handle())?;
    let y = y
        .try_into()
        .map_err(|_| QueryError::from_world_y(WorldY::new(256).unwrap_err()))?;
    let z = z.try_into().map_err(|_| QueryError::invalid_handle())?;
    admit_budget(1)?;
    let (section_id, presence, revision, cell) = world.cell_at(x, y, z)?;
    Ok(CellReadResult {
        section_id,
        presence,
        section_revision: revision,
        cell,
    })
}

/// Read one world coordinate into the caller-provided optional BlockId slot.
pub fn read_cell_into<X, Y, Z>(
    world: &BlockReadWorld,
    x: X,
    y: Y,
    z: Z,
    block_id: &mut Option<BlockId>,
) -> Result<CellReadResult, QueryError>
where
    X: TryInto<i32>,
    Y: TryInto<i64>,
    Z: TryInto<i32>,
{
    let result = read_cell(world, x, y, z)?;
    *block_id = result.block_id();
    Ok(result)
}

/// Read an inclusive axis-aligned box. Iteration is y outer, z middle, x inner.
pub fn read_box(
    world: &BlockReadWorld,
    min: (i32, i64, i32),
    max: (i32, i64, i32),
) -> Result<BlockReadResult, QueryError> {
    let count = box_cell_count(min, max)?;
    let (min_x, min_y, min_z) = min;
    let (max_x, max_y, max_z) = max;
    world.read_cells(
        (min_y..=max_y).flat_map(move |y| {
            (min_z..=max_z).flat_map(move |z| (min_x..=max_x).map(move |x| (x, y, z)))
        }),
        count,
    )
}

/// Read an inclusive box into caller-provided BlockId and segment buffers.
pub fn read_box_into(
    world: &BlockReadWorld,
    min: (i32, i64, i32),
    max: (i32, i64, i32),
    block_ids: &mut [Option<BlockId>],
    segments: &mut [Option<BufferedReadSegment>],
) -> Result<BufferedBlockReadResult, QueryError> {
    let count = box_cell_count(min, max)?;
    let (min_x, min_y, min_z) = min;
    let (max_x, max_y, max_z) = max;
    world.read_cells_into_buffers(
        || {
            (min_y..=max_y).flat_map(move |y| {
                (min_z..=max_z).flat_map(move |z| (min_x..=max_x).map(move |x| (x, y, z)))
            })
        },
        count,
        block_ids,
        segments,
    )
}

/// Read one x/z column from the lower y bound upward.
pub fn read_column<R: Into<ColumnYRange>>(
    world: &BlockReadWorld,
    x: i32,
    z: i32,
    range: R,
) -> Result<BlockReadResult, QueryError> {
    let range = range.into();
    let count = range.inclusive_len()?;
    world.read_cells((range.start..=range.end).map(move |y| (x, y, z)), count)
}

/// Read a column into caller-provided BlockId and segment buffers.
pub fn read_column_into<R: Into<ColumnYRange>>(
    world: &BlockReadWorld,
    x: i32,
    z: i32,
    range: R,
    block_ids: &mut [Option<BlockId>],
    segments: &mut [Option<BufferedReadSegment>],
) -> Result<BufferedBlockReadResult, QueryError> {
    let range = range.into();
    let count = range.inclusive_len()?;
    world.read_cells_into_buffers(
        || (range.start..=range.end).map(move |y| (x, y, z)),
        count,
        block_ids,
        segments,
    )
}

fn box_cell_count(min: (i32, i64, i32), max: (i32, i64, i32)) -> Result<usize, QueryError> {
    let (min_x, min_y, min_z) = min;
    let (max_x, max_y, max_z) = max;
    if min_x > max_x || min_y > max_y || min_z > max_z {
        return Err(QueryError::invalid_handle());
    }
    WorldY::new(min_y).map_err(QueryError::from_world_y)?;
    WorldY::new(max_y).map_err(QueryError::from_world_y)?;
    let width = inclusive_axis_len(i64::from(min_x), i64::from(max_x))?;
    let height = inclusive_axis_len(min_y, max_y)?;
    let depth = inclusive_axis_len(i64::from(min_z), i64::from(max_z))?;
    width
        .checked_mul(height)
        .and_then(|value| value.checked_mul(depth))
        .ok_or_else(|| QueryError::contract("read_budget_exceeded"))
}

fn inclusive_axis_len(start: i64, end: i64) -> Result<usize, QueryError> {
    end.checked_sub(start)
        .and_then(|delta| delta.checked_add(1))
        .and_then(|length| usize::try_from(length).ok())
        .ok_or_else(|| QueryError::contract("read_budget_exceeded"))
}

fn admit_budget(count: usize) -> Result<(), QueryError> {
    if count > MAX_CELLS_PER_READ_REQUEST {
        Err(QueryError::contract("read_budget_exceeded"))
    } else {
        Ok(())
    }
}

fn intern_presence(name: &str) -> &'static str {
    vw::intern_presence(name).expect("read presence must be a generated SECTION_PRESENCE member")
}

struct SegmentBuilder {
    section_id: String,
    presence: &'static str,
    section_revision: u64,
    cells: Vec<ReadCell>,
    block_ids: Option<Vec<BlockId>>,
}

impl SegmentBuilder {
    fn new(
        section_id: String,
        presence: &'static str,
        section_revision: u64,
        resolved: bool,
    ) -> Self {
        let block_ids = if resolved { Some(Vec::new()) } else { None };
        Self {
            section_id,
            presence,
            section_revision,
            cells: Vec::new(),
            block_ids,
        }
    }

    fn push(&mut self, cell: ReadCell) {
        if let Some(ids) = self.block_ids.as_mut()
            && let Some(block_id) = cell.block_id
        {
            ids.push(block_id);
        }
        self.cells.push(cell);
    }

    fn finish(self) -> ReadSegment {
        ReadSegment {
            section_id: self.section_id,
            presence: self.presence,
            section_revision: self.section_revision,
            cells: self.cells,
            block_ids: self.block_ids,
        }
    }
}
