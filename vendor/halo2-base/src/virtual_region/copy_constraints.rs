use std::cmp::Reverse;
use std::collections::{BTreeMap, BinaryHeap, HashMap};
use std::ops::DerefMut;
use std::sync::{Arc, Mutex, OnceLock};

use rayon::slice::ParallelSliceMut;

use crate::AssignedValue;
use crate::halo2_proofs::{
    circuit::{Cell, Region},
    plonk::{Assigned, Column, Fixed},
};
use crate::utils::halo2::{Halo2AssignedCell, raw_assign_fixed, raw_constrain_equal};
use crate::{ContextCell, ff::Field};

pub use crate::EXTERNAL_CELL_TYPE_ID;

use super::manager::VirtualRegionManager;

/// Thread-safe shared global manager for all copy constraints.
pub type SharedCopyConstraintManager<F> = Arc<Mutex<CopyConstraintManager<F>>>;

const CONSTANT_CELL_SEGMENT_LEN: usize = 1 << 16;

/// Append-only constant-cell storage partitioned into bounded segments.
///
/// The first segment grows from one cell so singleton constants remain cheap;
/// later segments reserve their complete fixed-size payload up front.
/// Canonicalization sorts each segment. A bounded merge frontier
/// then exposes the same globally sorted sequence as one flat unstable sort;
/// equal cells are indistinguishable, so the segment tie-breakers cannot alter
/// the observable `(constant, ContextCell)` sequence.
#[derive(Clone, Debug, Default)]
struct SegmentedContextCells {
    segments: Vec<Vec<ContextCell>>,
    len: usize,
    canonicalized: bool,
}

impl SegmentedContextCells {
    fn push(&mut self, cell: ContextCell) {
        let next_len = self
            .len
            .checked_add(1)
            .expect("constant-cell bucket length overflowed");
        if self.len % CONSTANT_CELL_SEGMENT_LEN == 0 {
            self.segments.reserve(1);
            let capacity = if self.segments.is_empty() {
                1
            } else {
                CONSTANT_CELL_SEGMENT_LEN
            };
            let mut segment = Vec::with_capacity(capacity);
            segment.push(cell);
            self.segments.push(segment);
        } else {
            let segment = self
                .segments
                .last_mut()
                .expect("constant-cell bucket lost its live tail segment");
            debug_assert!(segment.len() < CONSTANT_CELL_SEGMENT_LEN);
            segment.push(cell);
        }
        self.len = next_len;
        self.canonicalized = false;
    }

    fn len(&self) -> usize {
        self.len
    }

    fn checked_capacity(&self) -> Option<usize> {
        self.segments.iter().try_fold(0_usize, |total, segment| {
            total.checked_add(segment.capacity())
        })
    }

    fn canonicalize(&mut self) {
        if self.canonicalized {
            return;
        }
        for segment in &mut self.segments {
            segment.par_sort_unstable();
        }
        self.canonicalized = true;
    }

    fn iter(&self) -> ConstantCellIter<'_> {
        if self.canonicalized {
            ConstantCellIter::canonical(self)
        } else {
            ConstantCellIter::Insertion {
                bucket: self,
                segment: 0,
                offset: 0,
                remaining: self.len,
            }
        }
    }
}

enum ConstantCellIter<'a> {
    Insertion {
        bucket: &'a SegmentedContextCells,
        segment: usize,
        offset: usize,
        remaining: usize,
    },
    Canonical {
        bucket: &'a SegmentedContextCells,
        frontier: BinaryHeap<Reverse<(ContextCell, usize, usize)>>,
        remaining: usize,
    },
}

impl<'a> ConstantCellIter<'a> {
    fn canonical(bucket: &'a SegmentedContextCells) -> Self {
        let mut frontier = BinaryHeap::with_capacity(bucket.segments.len());
        for (segment, cells) in bucket.segments.iter().enumerate() {
            if let Some(cell) = cells.first() {
                frontier.push(Reverse((*cell, segment, 0)));
            }
        }
        Self::Canonical {
            bucket,
            frontier,
            remaining: bucket.len,
        }
    }
}

impl<'a> Iterator for ConstantCellIter<'a> {
    type Item = &'a ContextCell;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Insertion {
                bucket,
                segment,
                offset,
                remaining,
            } => loop {
                let cells = bucket.segments.get(*segment)?;
                if let Some(cell) = cells.get(*offset) {
                    *offset += 1;
                    *remaining -= 1;
                    return Some(cell);
                }
                *segment += 1;
                *offset = 0;
            },
            Self::Canonical {
                bucket,
                frontier,
                remaining,
            } => {
                let Reverse((_, segment, offset)) = frontier.pop()?;
                let next_offset = offset + 1;
                if let Some(next) = bucket.segments[segment].get(next_offset) {
                    frontier.push(Reverse((*next, segment, next_offset)));
                }
                *remaining -= 1;
                Some(&bucket.segments[segment][offset])
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = match self {
            Self::Insertion { remaining, .. } | Self::Canonical { remaining, .. } => *remaining,
        };
        (remaining, Some(remaining))
    }
}

impl ExactSizeIterator for ConstantCellIter<'_> {}
impl std::iter::FusedIterator for ConstantCellIter<'_> {}

/// Constant-copy constraints grouped by their exact fixed value.
///
/// The former flat `(F, ContextCell)` inventory repeated a full field element
/// for every constrained cell. This representation retains one ordered-map key
/// per distinct constant and stores every constrained cell, including exact
/// duplicates, in a stable bucket. Sorting each bucket by [`ContextCell`] and
/// then iterating the [`BTreeMap`] reproduces the former lexicographic
/// `(constant, cell)` order exactly.
#[derive(Clone, Default, Debug)]
pub struct ConstantEqualities<F: Field + Ord> {
    bucket_by_constant: BTreeMap<F, usize>,
    cell_buckets: Vec<SegmentedContextCells>,
    len: usize,
    last_constant: Option<(F, usize)>,
    last_cache_hits: usize,
    index_lookups: usize,
}

impl<F: Field + Ord> ConstantEqualities<F> {
    /// Adds one constant-copy constraint without collapsing duplicates.
    pub fn push(&mut self, (constant, cell): (F, ContextCell)) {
        let next_len = self
            .len
            .checked_add(1)
            .expect("constant-equality count overflowed");
        let cached = self
            .last_constant
            .filter(|(last, _)| last == &constant)
            .map(|(_, bucket)| bucket);
        let bucket = if let Some(bucket) = cached {
            self.last_cache_hits = self
                .last_cache_hits
                .checked_add(1)
                .expect("constant-equality cache-hit count overflowed");
            bucket
        } else {
            self.index_lookups = self
                .index_lookups
                .checked_add(1)
                .expect("constant-equality index-lookup count overflowed");
            match self.bucket_by_constant.entry(constant) {
                std::collections::btree_map::Entry::Occupied(entry) => *entry.get(),
                std::collections::btree_map::Entry::Vacant(entry) => {
                    let bucket = self.cell_buckets.len();
                    self.cell_buckets.push(SegmentedContextCells::default());
                    entry.insert(bucket);
                    bucket
                }
            }
        };
        self.cell_buckets[bucket].push(cell);
        self.last_constant = Some((constant, bucket));
        self.len = next_len;
    }

    /// Returns the total number of constant-copy constraints, including duplicates.
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Returns `true` when no constant-copy constraints are retained.
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Returns the number of exact distinct constants.
    pub fn distinct_len(&self) -> usize {
        self.bucket_by_constant.len()
    }

    /// Returns the checked sum of all retained cell-bucket lengths.
    pub fn checked_cell_len(&self) -> Option<usize> {
        self.cell_buckets
            .iter()
            .try_fold(0_usize, |total, bucket| total.checked_add(bucket.len()))
    }

    /// Returns the checked sum of all retained cell-bucket capacities.
    pub fn checked_cell_capacity(&self) -> Option<usize> {
        self.cell_buckets.iter().try_fold(0_usize, |total, bucket| {
            total.checked_add(bucket.checked_capacity()?)
        })
    }

    /// Returns the number of pushes served by the last-constant cache.
    pub const fn last_cache_hits(&self) -> usize {
        self.last_cache_hits
    }

    /// Returns the number of pushes that consulted the ordered constant index.
    pub const fn index_lookups(&self) -> usize {
        self.index_lookups
    }

    /// Sorts every cell bucket into the legacy canonical order.
    fn canonicalize_cells(&mut self) {
        for bucket in &mut self.cell_buckets {
            bucket.canonicalize();
        }
    }

    /// Iterates exact distinct constants in field order without constructing a
    /// cell-merge frontier.
    fn constants(&self) -> impl Iterator<Item = &F> {
        self.bucket_by_constant.keys()
    }

    /// Iterates constants in field order and their complete cell buckets.
    fn buckets(&self) -> impl Iterator<Item = (&F, ConstantCellIter<'_>)> {
        self.bucket_by_constant
            .iter()
            .map(|(constant, bucket)| (constant, self.cell_buckets[*bucket].iter()))
    }

    /// Iterates constants in field order and cells in their current bucket order.
    ///
    /// After physical-assignment canonicalization, the flattened sequence is
    /// the legacy lexicographic `(constant, cell)` order. Before then, cell
    /// order reflects constraint insertion within each constant bucket.
    pub fn iter(&self) -> impl Iterator<Item = (&F, &ContextCell)> {
        self.buckets()
            .flat_map(|(constant, bucket)| bucket.map(move |cell| (constant, cell)))
    }

    /// Clears all equality, index, and cache state.
    pub fn clear(&mut self) {
        self.bucket_by_constant.clear();
        self.cell_buckets.clear();
        self.len = 0;
        self.last_constant = None;
        self.last_cache_hits = 0;
        self.index_lookups = 0;
    }
}

/// Global manager for all copy constraints. Thread-safe.
///
/// This will only be accessed during key generation, not proof generation, so it does not need to be optimized.
///
/// Implements [VirtualRegionManager], which should be assigned only after all cells have been assigned
/// by other managers.
#[derive(Clone, Default, Debug)]
pub struct CopyConstraintManager<F: Field + Ord> {
    /// A [Vec] tracking equality constraints between pairs of virtual advice cells, tagged by [ContextCell].
    /// These can be across different virtual regions.
    pub advice_equalities: Vec<(ContextCell, ContextCell)>,

    /// Equality constraints between virtual advice cells and fixed values.
    /// Each exact fixed value is retained only once in the ordered index.
    pub constant_equalities: ConstantEqualities<F>,

    external_cell_count: usize,

    // In circuit assignments
    /// Advice assignments, mapping from virtual [ContextCell] to assigned physical [Cell]
    pub assigned_advices: HashMap<ContextCell, Cell>,
    /// Constant assignments, (key = constant, value = [Cell])
    pub assigned_constants: BTreeMap<F, Cell>,
    /// Flag for whether `assign_raw` has been called, for safety only.
    assigned: OnceLock<()>,
}

impl<F: Field + Ord> CopyConstraintManager<F> {
    /// Adds one virtual advice equality and relieves any released large growth buffer.
    pub(crate) fn push_advice_equality(&mut self, equality: (ContextCell, ContextCell)) {
        let old_capacity = self.advice_equalities.capacity();
        self.advice_equalities.push(equality);
        crate::release_large_vec_reallocation_slack::<(ContextCell, ContextCell)>(
            old_capacity,
            self.advice_equalities.capacity(),
        );
    }

    /// Returns the number of distinct constants used.
    pub fn num_distinct_constants(&self) -> usize {
        self.constant_equalities.distinct_len()
    }

    /// Drops synthesis-local physical cells while retaining the virtual
    /// equality graph.
    ///
    /// Physical [`Cell`] coordinates belong to one layouter invocation. A
    /// deep-cloned or witness-stripped circuit must rebuild this map during
    /// its own synthesis instead of inheriting coordinates from the source
    /// circuit.
    pub(crate) fn reset_physical_assignments(&mut self) {
        self.assigned_advices.clear();
        self.assigned_constants.clear();
        self.assigned.take();
    }

    /// Adds external raw [Halo2AssignedCell] to `self.assigned_advices` and returns a new virtual [AssignedValue]
    /// that can be used in any virtual region. No copy constraint is imposed, as the virtual cell "points" to the
    /// raw assigned cell. The returned [ContextCell] uses [`EXTERNAL_CELL_TYPE_ID`].
    pub fn load_external_assigned(
        &mut self,
        assigned_cell: Halo2AssignedCell<F>,
    ) -> AssignedValue<F> {
        let context_cell = self.load_external_cell(assigned_cell.cell());
        let mut value = Assigned::Trivial(F::ZERO);
        assigned_cell.value().map(|v| {
            #[cfg(feature = "halo2-axiom")]
            {
                value = **v;
            }
            #[cfg(not(feature = "halo2-axiom"))]
            {
                value = *v;
            }
        });
        AssignedValue {
            value,
            cell: Some(context_cell),
        }
    }

    /// Adds external raw Halo2 cell to `self.assigned_advices` and returns a new virtual cell that can be
    /// used as a tag (but will not be re-assigned). The returned [ContextCell]
    /// uses [`EXTERNAL_CELL_TYPE_ID`].
    pub fn load_external_cell(&mut self, cell: Cell) -> ContextCell {
        self.load_external_cell_impl(Some(cell))
    }

    /// Mock to load an external cell for base circuit simulation. If any mock external cell is loaded, calling `assign_raw` will panic.
    pub fn mock_external_assigned(&mut self, v: F) -> AssignedValue<F> {
        let context_cell = self.load_external_cell_impl(None);
        AssignedValue {
            value: Assigned::Trivial(v),
            cell: Some(context_cell),
        }
    }

    fn load_external_cell_impl(&mut self, cell: Option<Cell>) -> ContextCell {
        let context_cell = ContextCell::new(EXTERNAL_CELL_TYPE_ID, 0, self.external_cell_count);
        self.external_cell_count += 1;
        if let Some(cell) = cell {
            if let Some(old_cell) = self.assigned_advices.insert(context_cell, cell) {
                assert!(
                    old_cell.row_offset == cell.row_offset && old_cell.column == cell.column,
                    "External cell already assigned"
                )
            }
        }
        context_cell
    }

    /// Clears state
    pub fn clear(&mut self) {
        self.advice_equalities.clear();
        self.constant_equalities.clear();
        self.assigned_advices.clear();
        self.assigned_constants.clear();
        self.external_cell_count = 0;
        self.assigned.take();
    }
}

impl<F: Field + Ord> Drop for CopyConstraintManager<F> {
    fn drop(&mut self) {
        if self.assigned.get().is_some() {
            return;
        }
        if !self.advice_equalities.is_empty() {
            log::warn!("WARNING: advice_equalities not empty");
        }
        if !self.constant_equalities.is_empty() {
            log::warn!("WARNING: constant_equalities not empty");
        }
    }
}

impl<F: Field + Ord> VirtualRegionManager<F> for SharedCopyConstraintManager<F> {
    // The fixed columns
    type Config = Vec<Column<Fixed>>;
    type Assignment = ();

    /// This should be the last manager to be assigned, after all other managers have assigned cells.
    fn assign_raw(&self, config: &Self::Config, region: &mut Region<F>) -> Self::Assignment {
        let mut guard = self.lock().unwrap();
        let manager = guard.deref_mut();
        // BTreeMap iteration sorts constants deterministically. Sorting every
        // complete cell bucket reproduces the former flat
        // `(constant, ContextCell)` comparator exactly, including duplicates.
        manager.constant_equalities.canonicalize_cells();
        // Assign fixed cells, we go left to right, then top to bottom, to avoid needing to know number of rows here
        let mut fixed_col = 0;
        let mut fixed_offset = 0;
        for constant in manager.constant_equalities.constants() {
            // this will panic if you run out of rows
            let cell = raw_assign_fixed(region, config[fixed_col], fixed_offset, *constant);
            manager.assigned_constants.insert(*constant, cell);
            fixed_col += 1;
            if fixed_col >= config.len() {
                fixed_col = 0;
                fixed_offset += 1;
            }
        }

        // Just in case: we sort by ContextCell because the backend implementation of `raw_constrain_equal` (permutation argument) seems to depend on the order you specify copy constraints...
        manager.advice_equalities.par_sort_unstable();
        // Impose equality constraints between assigned advice cells
        // At this point we assume all cells have been assigned by other VirtualRegionManagers
        for (left, right) in &manager.advice_equalities {
            let left = manager
                .assigned_advices
                .get(left)
                .expect("virtual cell not assigned");
            let right = manager
                .assigned_advices
                .get(right)
                .expect("virtual cell not assigned");
            raw_constrain_equal(region, *left, *right);
        }
        for (constant, cells) in manager.constant_equalities.buckets() {
            let left = manager.assigned_constants[constant];
            for right in cells {
                let right = manager
                    .assigned_advices
                    .get(right)
                    .expect("virtual cell not assigned");
                raw_constrain_equal(region, left, *right);
            }
        }
        // We can't clear advice_equalities and constant_equalities because keygen_vk and keygen_pk will call this function twice
        let _ = manager.assigned.set(());
        // When keygen_vk and keygen_pk are both run, you need to clear assigned constants
        // so the second run still assigns constants in the pk
        manager.assigned_constants.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::FIRST_PHASE_CELL_TYPE_ID;
    use crate::halo2_proofs::halo2curves::bn256::Fr;

    fn equality(value: u64, context: usize, offset: usize) -> (Fr, ContextCell) {
        (
            Fr::from(value),
            ContextCell::new(FIRST_PHASE_CELL_TYPE_ID, context, offset),
        )
    }

    #[test]
    fn bucketed_constants_match_the_complete_legacy_canonical_sequence() {
        let mut manager = CopyConstraintManager::<Fr>::default();
        let fixture = vec![
            equality(7, 2, 4),
            equality(7, 2, 4),
            equality(3, 1, 9),
            equality(7, 0, 8),
            equality(11, 1, 2),
            equality(3, 0, 3),
            equality(11, 0, 1),
            equality(7, 0, 5),
        ];
        for equality in fixture.iter().copied() {
            manager.constant_equalities.push(equality);
        }
        let before = manager
            .constant_equalities
            .iter()
            .map(|(constant, cell)| (*constant, *cell))
            .collect::<Vec<_>>();

        assert_eq!(manager.num_distinct_constants(), 3);
        assert_eq!(manager.constant_equalities.len(), fixture.len());
        assert_eq!(
            manager.constant_equalities.checked_cell_len(),
            Some(fixture.len())
        );
        assert_eq!(manager.constant_equalities.last_cache_hits(), 1);
        assert_eq!(manager.constant_equalities.index_lookups(), 7);
        assert_eq!(
            manager
                .constant_equalities
                .last_cache_hits()
                .checked_add(manager.constant_equalities.index_lookups()),
            Some(fixture.len())
        );
        assert!(
            manager
                .constant_equalities
                .checked_cell_capacity()
                .expect("cell capacity")
                >= fixture.len()
        );
        assert_eq!(
            manager
                .constant_equalities
                .iter()
                .map(|(constant, cell)| (*constant, *cell))
                .collect::<Vec<_>>(),
            before,
            "distinct counting must not canonicalize or otherwise mutate buckets"
        );

        let mut legacy = fixture.clone();
        legacy.par_sort_unstable_by(|(c1, cell1), (c2, cell2)| c1.cmp(c2).then(cell1.cmp(cell2)));
        manager.constant_equalities.canonicalize_cells();
        let canonical = manager
            .constant_equalities
            .iter()
            .map(|(constant, cell)| (*constant, *cell))
            .collect::<Vec<_>>();
        assert_eq!(canonical, legacy);
        manager.constant_equalities.canonicalize_cells();
        assert_eq!(
            manager
                .constant_equalities
                .iter()
                .map(|(constant, cell)| (*constant, *cell))
                .collect::<Vec<_>>(),
            canonical,
            "canonicalizing an already sorted inventory must be idempotent"
        );

        let mut reversed = ConstantEqualities::<Fr>::default();
        for equality in fixture.iter().rev().copied() {
            reversed.push(equality);
        }
        reversed.canonicalize_cells();
        assert_eq!(
            reversed
                .iter()
                .map(|(constant, cell)| (*constant, *cell))
                .collect::<Vec<_>>(),
            canonical,
            "canonical constraints must not depend on insertion schedule"
        );

        let appended = equality(3, 3, 0);
        manager.constant_equalities.push(appended);
        legacy.push(appended);
        legacy.par_sort_unstable_by(|(c1, cell1), (c2, cell2)| c1.cmp(c2).then(cell1.cmp(cell2)));
        manager.constant_equalities.canonicalize_cells();
        assert_eq!(
            manager
                .constant_equalities
                .iter()
                .map(|(constant, cell)| (*constant, *cell))
                .collect::<Vec<_>>(),
            legacy,
            "appending after a keygen-style sort must remain canonical"
        );

        let mut cloned = manager.constant_equalities.clone();
        cloned.push(equality(13, 0, 0));
        assert_eq!(manager.constant_equalities.len(), fixture.len() + 1);
        assert_eq!(cloned.len(), fixture.len() + 2);
        assert_eq!(cloned.distinct_len(), 4);
        cloned.clear();
        assert!(cloned.is_empty());
        assert_eq!(cloned.distinct_len(), 0);
        assert_eq!(cloned.checked_cell_len(), Some(0));
        assert_eq!(cloned.checked_cell_capacity(), Some(0));
        assert_eq!(cloned.last_cache_hits(), 0);
        assert_eq!(cloned.index_lookups(), 0);

        reversed.clear();
        manager.clear();
    }

    #[test]
    fn segmented_constant_bucket_grows_head_then_fixes_later_segments() {
        let cell = ContextCell::new(FIRST_PHASE_CELL_TYPE_ID, 0, 0);
        let mut bucket = SegmentedContextCells::default();

        bucket.push(cell);
        assert_eq!(bucket.segments.len(), 1);
        assert_eq!(bucket.segments[0].capacity(), 1);
        assert_eq!(bucket.checked_capacity(), Some(1));
        assert_eq!(
            bucket.segments[0].capacity() * std::mem::size_of::<ContextCell>(),
            std::mem::size_of::<ContextCell>(),
            "a singleton bucket must not reserve a complete 512 KiB segment"
        );

        bucket.push(cell);
        assert!(bucket.segments[0].capacity() > 1);
        assert!(bucket.segments[0].capacity() < CONSTANT_CELL_SEGMENT_LEN);
        while bucket.len() < CONSTANT_CELL_SEGMENT_LEN {
            bucket.push(cell);
        }
        assert_eq!(bucket.segments.len(), 1);
        assert_eq!(bucket.segments[0].len(), CONSTANT_CELL_SEGMENT_LEN);
        assert_eq!(bucket.segments[0].capacity(), CONSTANT_CELL_SEGMENT_LEN);
        assert_eq!(bucket.checked_capacity(), Some(CONSTANT_CELL_SEGMENT_LEN));

        bucket.push(cell);
        assert_eq!(bucket.segments.len(), 2);
        assert_eq!(bucket.segments[1].len(), 1);
        assert_eq!(bucket.segments[1].capacity(), CONSTANT_CELL_SEGMENT_LEN);
        assert_eq!(
            bucket.checked_capacity(),
            Some(CONSTANT_CELL_SEGMENT_LEN * 2)
        );
    }

    #[test]
    fn segmented_constant_bucket_preserves_boundary_duplicates_and_canonical_order() {
        let cell_count = CONSTANT_CELL_SEGMENT_LEN + 3;
        let inserted = (0..cell_count)
            .map(|index| {
                ContextCell::new(
                    FIRST_PHASE_CELL_TYPE_ID,
                    index % 3,
                    (index.wrapping_mul(37) + 11) % 1_009,
                )
            })
            .collect::<Vec<_>>();
        let mut bucket = SegmentedContextCells::default();
        for cell in inserted.iter().copied() {
            bucket.push(cell);
        }

        assert_eq!(bucket.len(), cell_count);
        assert_eq!(bucket.segments.len(), 2);
        assert_eq!(bucket.segments[0].capacity(), CONSTANT_CELL_SEGMENT_LEN);
        assert_eq!(bucket.segments[1].capacity(), CONSTANT_CELL_SEGMENT_LEN);
        assert_eq!(
            bucket.checked_capacity(),
            Some(CONSTANT_CELL_SEGMENT_LEN * 2)
        );
        assert_eq!(
            bucket.iter().copied().collect::<Vec<_>>(),
            inserted,
            "crossing a segment boundary must preserve every insertion and duplicate"
        );

        let mut expected = inserted;
        expected.par_sort_unstable();
        bucket.canonicalize();
        match bucket.iter() {
            ConstantCellIter::Canonical { frontier, .. } => {
                assert_eq!(frontier.len(), bucket.segments.len());
            }
            ConstantCellIter::Insertion { .. } => panic!("canonical merge iterator required"),
        }
        assert_eq!(
            bucket.iter().copied().collect::<Vec<_>>(),
            expected,
            "segment-local sorts and bounded merge must equal one flat unstable sort"
        );

        let appended = ContextCell::new(FIRST_PHASE_CELL_TYPE_ID, 2, 7);
        bucket.push(appended);
        expected.push(appended);
        expected.par_sort_unstable();
        bucket.canonicalize();
        assert_eq!(
            bucket.iter().copied().collect::<Vec<_>>(),
            expected,
            "append-after-canonicalize must be included by the next canonicalization"
        );
    }

    #[test]
    fn segmented_constant_bucket_iterators_are_exact_and_fused() {
        fn assert_exact_and_fused(mut iter: ConstantCellIter<'_>, expected: &[ContextCell]) {
            assert_eq!(iter.size_hint(), (expected.len(), Some(expected.len())));
            assert_eq!(iter.len(), expected.len());
            for (index, expected_cell) in expected.iter().enumerate() {
                assert_eq!(iter.next(), Some(expected_cell));
                let remaining = expected.len() - index - 1;
                assert_eq!(iter.size_hint(), (remaining, Some(remaining)));
                assert_eq!(iter.len(), remaining);
            }
            assert_eq!(iter.next(), None);
            assert_eq!(iter.next(), None, "an exhausted iterator must remain fused");
            assert_eq!(iter.size_hint(), (0, Some(0)));
            assert_eq!(iter.len(), 0);
        }

        let inserted = [
            ContextCell::new(FIRST_PHASE_CELL_TYPE_ID, 2, 3),
            ContextCell::new(FIRST_PHASE_CELL_TYPE_ID, 0, 1),
            ContextCell::new(FIRST_PHASE_CELL_TYPE_ID, 0, 1),
        ];
        let mut bucket = SegmentedContextCells::default();
        for cell in inserted {
            bucket.push(cell);
        }
        assert_exact_and_fused(bucket.iter(), &inserted);

        bucket.canonicalize();
        let mut canonical = inserted;
        canonical.sort_unstable();
        assert_exact_and_fused(bucket.iter(), &canonical);
    }

    #[cfg(target_pointer_width = "64")]
    #[test]
    fn bucketed_constant_cell_payload_is_eight_bytes() {
        assert_eq!(std::mem::size_of::<ContextCell>(), 8);
        assert_eq!(std::mem::size_of::<(Fr, ContextCell)>(), 40);
    }
}
