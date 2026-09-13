//! Lossless, in-memory compression of completed virtual numerator segments.
//!
//! Tags, ranks, payload sizes, and timing depend on witness values. Initialized
//! owned payloads use typed volatile erasure before deallocation. This does not
//! erase caller copies, registers, allocator slack, swap or core dumps, and does
//! not provide constant-time access, secure deletion or bounded process RSS.

use super::{COMPACT_ADVICE_NUMERATOR_SEGMENT_LEN, ScalarField};
use std::mem::size_of;

const RANK_BLOCK_LEN: usize = 256;
const TAGS_PER_BYTE: usize = 4;
const OTHER_TAG: u8 = 2;

/// Erase initialized Copy values without assuming a field's byte layout.
fn erase_initialized<T: Copy>(values: &mut [T], zero: T) {
    for value in values {
        // SAFETY: `value` is an exclusive, aligned reference to one initialized
        // T. `zero` is a valid T. Copy excludes ownership requiring a destructor;
        // every write stays within the original mutable slice. Volatile stores
        // prevent dead-store elimination when its owner is about to deallocate.
        unsafe { std::ptr::write_volatile(value, zero) };
    }
    std::sync::atomic::compiler_fence(std::sync::atomic::Ordering::SeqCst);
}

/// Count OTHER high bits without native-endian or alignment assumptions.
fn count_other_tags(bytes: &[u8]) -> usize {
    let mut chunks = bytes.chunks_exact(8);
    let mut count = 0_usize;
    for chunk in &mut chunks {
        let word = u64::from_le_bytes(chunk.try_into().expect("exact eight-byte tag word"));
        count += ((word >> 1) & 0x5555_5555_5555_5555).count_ones() as usize;
    }
    for byte in chunks.remainder() {
        count += ((byte >> 1) & 0x55).count_ones() as usize;
    }
    count
}

#[cfg(test)]
std::thread_local! {
    static DROP_OBSERVATIONS: std::cell::RefCell<Option<Vec<(&'static str, usize, bool)>>> = std::cell::RefCell::new(None);
}

#[cfg(test)]
fn observe_erased_drop(kind: &'static str, len: usize, erased: bool) {
    // Observed after typed erasure and before the owned Vec deallocates. A
    // thread-local observer cannot interfere with other parallel unit tests.
    let _ = DROP_OBSERVATIONS.try_with(|observer| {
        if let Ok(mut observer) = observer.try_borrow_mut() {
            if let Some(events) = observer.as_mut() {
                events.push((kind, len, erased));
            }
        }
    });
}

/// Owned numerator allocations, excluding allocator metadata and stack storage.
///
/// Logical advice length is separate from stored field slots: zeros and ones in
/// tagged segments occupy no field slot. Counts and byte capacities are exact
/// for the live Vec allocations, not process RSS or secure-memory guarantees.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AdviceNumeratorStorage {
    /// Initialized dense and OTHER field slots, respectively.
    pub field_slots_len: [usize; 2],
    /// Allocated dense and OTHER field slots, respectively.
    pub field_slots_capacity: [usize; 2],
    /// Initialized and allocated tag bytes, respectively.
    pub tag_bytes: [usize; 2],
    /// Initialized and allocated rank bytes, respectively.
    pub rank_bytes: [usize; 2],
    /// Dense and tagged segment counts, respectively.
    pub segment_counts: [usize; 2],
    /// Capacity in bytes of the outer segment-header Vec.
    pub segment_header_capacity_bytes: usize,
    /// Sum of field, tag, rank, and outer header capacities in bytes.
    pub owned_capacity_bytes: usize,
    /// Largest owned numerator heap capacity observed while preparing a segment
    /// transition, including the old store and temporary encoded/dense buffers.
    ///
    /// This is one store's historical high-water counter. It excludes other
    /// Context allocations, allocator overhead, stack temporaries, and source
    /// builders that coexist with clones. Summing counters is not a process peak.
    pub max_transition_owned_capacity_bytes: usize,
    /// Conservative owned-capacity bound during outer Vec growth, including
    /// both old and new header allocations when capacity changes. Allocators
    /// may grow in place, so this is not an observed simultaneous allocation.
    pub max_outer_growth_capacity_upper_bound_bytes: usize,
}

pub(super) struct DenseNumerators<F: ScalarField> {
    values: Vec<F>,
}

impl<F: ScalarField> DenseNumerators<F> {
    fn with_capacity(capacity: usize) -> Self {
        Self {
            values: Vec::with_capacity(capacity),
        }
    }
}

impl<F: ScalarField> Clone for DenseNumerators<F> {
    fn clone(&self) -> Self {
        let mut cloned = Self::with_capacity(self.values.capacity());
        cloned.values.extend_from_slice(&self.values);
        cloned
    }
}

impl<F: ScalarField> Drop for DenseNumerators<F> {
    fn drop(&mut self) {
        erase_initialized(&mut self.values, F::ZERO);
        #[cfg(test)]
        observe_erased_drop(
            "dense",
            self.values.len(),
            self.values.iter().all(|value| *value == F::ZERO),
        );
    }
}

pub(super) struct TaggedNumerators<F: ScalarField> {
    tags: Vec<u8>,
    rank: Vec<u16>,
    others: Vec<F>,
}

impl<F: ScalarField> TaggedNumerators<F> {
    fn with_capacities(tags: usize, rank: usize, others: usize) -> Self {
        // Install a cleanup owner before any allocation or secret copy.
        let mut result = Self {
            tags: Vec::new(),
            rank: Vec::new(),
            others: Vec::new(),
        };
        result.tags.reserve_exact(tags);
        result.rank.reserve_exact(rank);
        result.others.reserve_exact(others);
        result
    }

    fn encode_full_segment(&mut self, source: &[F]) {
        assert_eq!(source.len(), COMPACT_ADVICE_NUMERATOR_SEGMENT_LEN);
        assert!(self.tags.is_empty() && self.rank.is_empty() && self.others.is_empty());
        self.tags
            .resize(COMPACT_ADVICE_NUMERATOR_SEGMENT_LEN / TAGS_PER_BYTE, 0);
        for (offset, value) in source.iter().copied().enumerate() {
            if offset % RANK_BLOCK_LEN == 0 {
                // No terminal checkpoint is stored for the 65536 payload count.
                self.rank.push(
                    u16::try_from(self.others.len())
                        .expect("numerator rank checkpoint exceeds u16"),
                );
            }
            let tag = if value == F::ZERO {
                0
            } else if value == F::ONE {
                1
            } else {
                self.others.push(value);
                OTHER_TAG
            };
            self.tags[offset / TAGS_PER_BYTE] |= tag << ((offset % TAGS_PER_BYTE) * 2);
        }
    }

    fn tag(&self, offset: usize) -> u8 {
        (self.tags[offset / TAGS_PER_BYTE] >> ((offset % TAGS_PER_BYTE) * 2)) & 3
    }

    fn get(&self, offset: usize) -> F {
        match self.tag(offset) {
            0 => F::ZERO,
            1 => F::ONE,
            OTHER_TAG => {
                let block = offset / RANK_BLOCK_LEN;
                let first_byte = block * RANK_BLOCK_LEN / TAGS_PER_BYTE;
                let last_byte = offset / TAGS_PER_BYTE;
                let mut index = usize::from(self.rank[block]);
                // At most seven complete words, seven bytes and a partial
                // byte. Each high tag bit denotes OTHER; rank stays in usize.
                index += count_other_tags(&self.tags[first_byte..last_byte]);
                let partial_bits = (offset % TAGS_PER_BYTE) * 2;
                let mask = (1_u8 << partial_bits) - 1;
                index += (((self.tags[last_byte] & mask) >> 1) & 0x55).count_ones() as usize;
                self.others[index]
            }
            _ => panic!("invalid internal numerator tag"),
        }
    }

    fn fits_dense_capacity(&self, dense_bytes: usize) -> bool {
        self.checked_capacity_bytes()
            .expect("tagged numerator capacity overflow")
            < dense_bytes
    }

    fn checked_capacity_bytes(&self) -> Option<usize> {
        self.tags
            .capacity()
            .checked_add(self.rank.capacity().checked_mul(size_of::<u16>())?)?
            .checked_add(self.others.capacity().checked_mul(size_of::<F>())?)
    }

    fn erase_buffers(&mut self) {
        erase_initialized(&mut self.tags, 0_u8);
        erase_initialized(&mut self.rank, 0_u16);
        erase_initialized(&mut self.others, F::ZERO);
    }

    fn wipe(&mut self) {
        self.erase_buffers();
        self.others.clear();
    }
}

impl<F: ScalarField> Clone for TaggedNumerators<F> {
    fn clone(&self) -> Self {
        let mut cloned = Self::with_capacities(
            self.tags.capacity(),
            self.rank.capacity(),
            self.others.capacity(),
        );
        cloned.tags.extend_from_slice(&self.tags);
        cloned.rank.extend_from_slice(&self.rank);
        cloned.others.extend_from_slice(&self.others);
        cloned
    }
}

impl<F: ScalarField> Drop for TaggedNumerators<F> {
    fn drop(&mut self) {
        self.erase_buffers();
        #[cfg(test)]
        {
            observe_erased_drop(
                "tags",
                self.tags.len(),
                self.tags.iter().all(|value| *value == 0),
            );
            observe_erased_drop(
                "rank",
                self.rank.len(),
                self.rank.iter().all(|value| *value == 0),
            );
            observe_erased_drop(
                "others",
                self.others.len(),
                self.others.iter().all(|value| *value == F::ZERO),
            );
        }
    }
}

#[derive(Clone)]
pub(super) enum NumeratorSegment<F: ScalarField> {
    Dense(DenseNumerators<F>),
    Tagged(TaggedNumerators<F>),
}

impl<F: ScalarField> NumeratorSegment<F> {
    fn checked_capacity_bytes(&self) -> Option<usize> {
        match self {
            Self::Dense(dense) => dense.values.capacity().checked_mul(size_of::<F>()),
            Self::Tagged(tagged) => tagged.checked_capacity_bytes(),
        }
    }

    fn get(&self, offset: usize) -> F {
        match self {
            Self::Dense(dense) => dense.values[offset],
            Self::Tagged(tagged) => tagged.get(offset),
        }
    }

    fn wipe(&mut self) {
        match self {
            Self::Dense(dense) => erase_initialized(&mut dense.values, F::ZERO),
            Self::Tagged(tagged) => tagged.wipe(),
        }
    }
}

/// All new allocations for a segment-boundary append. Dropping an uncommitted
/// ticket wipes its initialized payloads/tags/ranks through their owners.
pub(super) struct PreparedNumeratorPush<F: ScalarField> {
    previous: Option<NumeratorSegment<F>>,
    next: DenseNumerators<F>,
}

/// Full completed segments may be tagged; the active tail always stays dense.
/// Every historical get returns a copy of the original field value.
pub(super) struct SegmentedNumerators<F: ScalarField> {
    // Visible to the parent's existing overflow-before-mutation test only.
    pub(super) segments: Vec<NumeratorSegment<F>>,
    pub(super) len: usize,
    max_transition_owned_capacity_bytes: usize,
    max_outer_growth_capacity_upper_bound_bytes: usize,
}

impl<F: ScalarField> Default for SegmentedNumerators<F> {
    fn default() -> Self {
        Self {
            segments: Vec::new(),
            len: 0,
            max_transition_owned_capacity_bytes: 0,
            max_outer_growth_capacity_upper_bound_bytes: 0,
        }
    }
}

impl<F: ScalarField> Clone for SegmentedNumerators<F> {
    fn clone(&self) -> Self {
        let mut cloned = Self {
            segments: Vec::with_capacity(self.segments.capacity()),
            len: self.len,
            max_transition_owned_capacity_bytes: 0,
            max_outer_growth_capacity_upper_bound_bytes: 0,
        };
        // Element owners wipe an incomplete clone if a subsequent allocation
        // unwinds. Source/clone coexistence is outside the transition counter.
        for segment in &self.segments {
            cloned.segments.push(segment.clone());
        }
        cloned
    }
}

impl<F: ScalarField> SegmentedNumerators<F> {
    pub(super) fn len(&self) -> usize {
        self.len
    }

    fn location(index: usize) -> (usize, usize) {
        (
            index / COMPACT_ADVICE_NUMERATOR_SEGMENT_LEN,
            index % COMPACT_ADVICE_NUMERATOR_SEGMENT_LEN,
        )
    }

    fn prepare_tagged(source: &DenseNumerators<F>) -> (Option<NumeratorSegment<F>>, usize) {
        assert_eq!(source.values.len(), COMPACT_ADVICE_NUMERATOR_SEGMENT_LEN);
        let other_count = source
            .values
            .iter()
            .filter(|value| **value != F::ZERO && **value != F::ONE)
            .count();
        let tag_len = COMPACT_ADVICE_NUMERATOR_SEGMENT_LEN / TAGS_PER_BYTE;
        let rank_len = COMPACT_ADVICE_NUMERATOR_SEGMENT_LEN / RANK_BLOCK_LEN;
        let predicted = tag_len
            .checked_add(
                rank_len
                    .checked_mul(size_of::<u16>())
                    .expect("numerator rank size overflow"),
            )
            .and_then(|bytes| bytes.checked_add(other_count.checked_mul(size_of::<F>())?))
            .expect("tagged numerator capacity overflow");
        let dense_bytes = source
            .values
            .capacity()
            .checked_mul(size_of::<F>())
            .expect("dense numerator capacity overflow");
        if predicted >= dense_bytes {
            return (None, 0);
        }
        // Allocate exactly the OTHER request, never the logical segment length.
        let mut tagged = TaggedNumerators::with_capacities(tag_len, rank_len, other_count);
        let allocated = tagged
            .checked_capacity_bytes()
            .expect("tagged numerator capacity overflow");
        // Vec capacity may exceed the requested amount. A real-capacity check,
        // rather than the prediction, determines whether the candidate survives.
        if !tagged.fits_dense_capacity(dense_bytes) {
            return (None, allocated);
        }
        tagged.encode_full_segment(&source.values);
        assert_eq!(tagged.others.len(), other_count);
        assert_eq!(tagged.rank.len(), rank_len);
        (Some(NumeratorSegment::Tagged(tagged)), allocated)
    }

    pub(super) fn prepare_push(&mut self) -> Option<PreparedNumeratorPush<F>> {
        if self.len % COMPACT_ADVICE_NUMERATOR_SEGMENT_LEN != 0 {
            return None;
        }
        let old_header_capacity = self.segments.capacity();
        self.segments.reserve(1);
        let owned = self.storage().owned_capacity_bytes;
        if self.segments.capacity() != old_header_capacity {
            let old_bytes = old_header_capacity
                .checked_mul(size_of::<NumeratorSegment<F>>())
                .expect("numerator header growth overflow");
            self.max_outer_growth_capacity_upper_bound_bytes =
                self.max_outer_growth_capacity_upper_bound_bytes.max(
                    owned
                        .checked_add(old_bytes)
                        .expect("numerator header growth overflow"),
                );
        }
        let (previous, candidate_bytes) = match self.segments.last() {
            Some(NumeratorSegment::Dense(source)) => Self::prepare_tagged(source),
            Some(NumeratorSegment::Tagged(_)) => panic!("numerator active tail must be dense"),
            None => (None, 0),
        };
        self.record_transition(owned, candidate_bytes);
        let next = DenseNumerators::with_capacity(COMPACT_ADVICE_NUMERATOR_SEGMENT_LEN);
        let next_bytes = next
            .values
            .capacity()
            .checked_mul(size_of::<F>())
            .expect("dense numerator capacity overflow");
        // A rejected candidate was already dropped before next was allocated.
        let extra = next_bytes
            .checked_add(if previous.is_some() {
                candidate_bytes
            } else {
                0
            })
            .expect("numerator transition capacity overflow");
        self.record_transition(owned, extra);
        Some(PreparedNumeratorPush { previous, next })
    }

    fn record_transition(&mut self, owned: usize, extra: usize) {
        let total = owned
            .checked_add(extra)
            .expect("numerator transition capacity overflow");
        self.max_transition_owned_capacity_bytes =
            self.max_transition_owned_capacity_bytes.max(total);
    }

    pub(super) fn push_prepared(&mut self, value: F, prepared: Option<PreparedNumeratorPush<F>>) {
        let new_len = self
            .len
            .checked_add(1)
            .expect("SegmentedNumerators length overflow");
        if let Some(mut prepared) = prepared {
            debug_assert_eq!(self.len % COMPACT_ADVICE_NUMERATOR_SEGMENT_LEN, 0);
            if let Some(previous) = prepared.previous.take() {
                *self
                    .segments
                    .last_mut()
                    .expect("prepared numerator predecessor exists") = previous;
            }
            prepared.next.values.push(value);
            self.segments.push(NumeratorSegment::Dense(prepared.next));
        } else {
            debug_assert_ne!(self.len % COMPACT_ADVICE_NUMERATOR_SEGMENT_LEN, 0);
            let Some(NumeratorSegment::Dense(tail)) = self.segments.last_mut() else {
                panic!("numerator active tail must be dense");
            };
            debug_assert!(tail.values.len() < COMPACT_ADVICE_NUMERATOR_SEGMENT_LEN);
            tail.values.push(value);
        }
        self.len = new_len;
    }

    pub(super) fn get(&self, index: usize) -> Option<F> {
        if index >= self.len {
            return None;
        }
        let (segment, offset) = Self::location(index);
        Some(self.segments[segment].get(offset))
    }

    /// Perform the only possibly allocating part of historical replacement
    /// before CompactAdvice removes any rational metadata.
    pub(super) fn prepare_replacement(&mut self, index: usize) {
        assert!(
            index < self.len,
            "numerator replacement offset is out of bounds"
        );
        let (segment, _) = Self::location(index);
        if matches!(self.segments[segment], NumeratorSegment::Dense(_)) {
            return;
        }
        let mut dense = DenseNumerators::with_capacity(COMPACT_ADVICE_NUMERATOR_SEGMENT_LEN);
        for offset in 0..COMPACT_ADVICE_NUMERATOR_SEGMENT_LEN {
            dense.values.push(self.segments[segment].get(offset));
        }
        let owned = self.storage().owned_capacity_bytes;
        self.record_transition(
            owned,
            dense
                .values
                .capacity()
                .checked_mul(size_of::<F>())
                .expect("dense numerator capacity overflow"),
        );
        self.segments[segment] = NumeratorSegment::Dense(dense);
    }

    pub(super) fn replace_prepared(&mut self, index: usize, value: F) {
        let (segment, offset) = Self::location(index);
        let NumeratorSegment::Dense(dense) = &mut self.segments[segment] else {
            panic!("numerator replacement was not prepared");
        };
        dense.values[offset] = value;
    }

    pub(super) fn storage(&self) -> AdviceNumeratorStorage {
        let mut storage = AdviceNumeratorStorage::default();
        fn add(target: &mut usize, amount: usize) {
            *target = target
                .checked_add(amount)
                .expect("numerator storage capacity overflow");
        }
        for segment in &self.segments {
            match segment {
                NumeratorSegment::Dense(dense) => {
                    add(&mut storage.field_slots_len[0], dense.values.len());
                    add(
                        &mut storage.field_slots_capacity[0],
                        dense.values.capacity(),
                    );
                    add(&mut storage.segment_counts[0], 1);
                }
                NumeratorSegment::Tagged(tagged) => {
                    add(&mut storage.field_slots_len[1], tagged.others.len());
                    add(
                        &mut storage.field_slots_capacity[1],
                        tagged.others.capacity(),
                    );
                    add(&mut storage.tag_bytes[0], tagged.tags.len());
                    add(&mut storage.tag_bytes[1], tagged.tags.capacity());
                    add(
                        &mut storage.rank_bytes[0],
                        tagged
                            .rank
                            .len()
                            .checked_mul(size_of::<u16>())
                            .expect("rank length overflow"),
                    );
                    add(
                        &mut storage.rank_bytes[1],
                        tagged
                            .rank
                            .capacity()
                            .checked_mul(size_of::<u16>())
                            .expect("rank capacity overflow"),
                    );
                    add(&mut storage.segment_counts[1], 1);
                }
            }
            add(
                &mut storage.owned_capacity_bytes,
                segment
                    .checked_capacity_bytes()
                    .expect("numerator storage capacity overflow"),
            );
        }
        storage.segment_header_capacity_bytes = self
            .segments
            .capacity()
            .checked_mul(size_of::<NumeratorSegment<F>>())
            .expect("numerator segment header capacity overflow");
        add(
            &mut storage.owned_capacity_bytes,
            storage.segment_header_capacity_bytes,
        );
        storage.max_transition_owned_capacity_bytes = self.max_transition_owned_capacity_bytes;
        storage.max_outer_growth_capacity_upper_bound_bytes =
            self.max_outer_growth_capacity_upper_bound_bytes;
        storage
    }

    pub(super) fn checked_capacity(&self) -> Option<usize> {
        let storage = self.storage();
        storage.field_slots_capacity[0].checked_add(storage.field_slots_capacity[1])
    }

    pub(super) fn segment_count(&self) -> usize {
        self.segments.len()
    }

    pub(super) fn wipe(&mut self) {
        for segment in &mut self.segments {
            segment.wipe();
        }
        // The high-water count is historical witness-dependent metadata too.
        erase_initialized(
            std::slice::from_mut(&mut self.max_transition_owned_capacity_bytes),
            0_usize,
        );
        erase_initialized(
            std::slice::from_mut(&mut self.max_outer_growth_capacity_upper_bound_bytes),
            0_usize,
        );
    }

    pub(super) fn iter(&self) -> SegmentedNumeratorsIter<'_, F> {
        SegmentedNumeratorsIter {
            source: self,
            position: 0,
            other_index: 0,
        }
    }
}

impl<F: ScalarField> Drop for SegmentedNumerators<F> {
    fn drop(&mut self) {
        erase_initialized(std::slice::from_mut(&mut self.len), 0_usize);
        erase_initialized(
            std::slice::from_mut(&mut self.max_transition_owned_capacity_bytes),
            0_usize,
        );
        erase_initialized(
            std::slice::from_mut(&mut self.max_outer_growth_capacity_upper_bound_bytes),
            0_usize,
        );
        // Each segment's field/tag/rank owner wipes once when its fields drop.
    }
}

pub(super) struct SegmentedNumeratorsIter<'a, F: ScalarField> {
    source: &'a SegmentedNumerators<F>,
    position: usize,
    other_index: usize,
}

impl<F: ScalarField> Iterator for SegmentedNumeratorsIter<'_, F> {
    type Item = F;
    fn next(&mut self) -> Option<F> {
        if self.position >= self.source.len {
            return None;
        }
        let (segment, offset) = SegmentedNumerators::<F>::location(self.position);
        let value = match &self.source.segments[segment] {
            NumeratorSegment::Dense(dense) => dense.values[offset],
            NumeratorSegment::Tagged(tagged) => match tagged.tag(offset) {
                0 => F::ZERO,
                1 => F::ONE,
                OTHER_TAG => {
                    let value = tagged.others[self.other_index];
                    self.other_index += 1;
                    value
                }
                _ => panic!("invalid internal numerator tag"),
            },
        };
        self.position += 1;
        if self.position % COMPACT_ADVICE_NUMERATOR_SEGMENT_LEN == 0 {
            if let NumeratorSegment::Tagged(tagged) = &self.source.segments[segment] {
                assert_eq!(
                    self.other_index,
                    tagged.others.len(),
                    "numerator payload contains unused entries"
                );
            }
            self.other_index = 0;
        }
        Some(value)
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.source.len - self.position;
        (remaining, Some(remaining))
    }
}
impl<F: ScalarField> ExactSizeIterator for SegmentedNumeratorsIter<'_, F> {}
impl<F: ScalarField> std::iter::FusedIterator for SegmentedNumeratorsIter<'_, F> {}

#[cfg(test)]
mod tests;

#[cfg(all(test, feature = "halo2-axiom", feature = "test-utils"))]
mod proof_tests;
