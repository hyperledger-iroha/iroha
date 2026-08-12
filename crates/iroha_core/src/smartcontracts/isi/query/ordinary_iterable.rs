//! Allocation primitives and the fail-closed boundary for ordinary iterables.
//!
//! A generic [`ValidQuery`] iterator is not a source proof: producer-local
//! clones, candidate collections, and synthetic-field predicate evaluators can
//! allocate before pagination observes an item. Unadapted producers stop
//! before `ValidQuery::execute`; each admitted producer instead owns rows
//! through its query-specific bounded adapter.

use std::{
    alloc::{Layout, alloc},
    any::TypeId,
    io,
    mem::MaybeUninit,
    ptr::NonNull,
};

use iroha_data_model::{
    peer::PeerId,
    query::{
        dsl::CompoundPredicate,
        error::QueryExecutionFail as Error,
        parameters::QueryParams,
    },
};
use norito::{
    core::NoritoSerialize,
    json::{JsonSerialize, Value},
};

use super::{OrdinaryQueryExecutionLimits, ordinary_memory::OrdinaryCursorMode};
use crate::{
    smartcontracts::ValidQuery,
    state::{StateReadOnly, WorldReadOnly},
};

/// The world-state producer count admitted through a source-specific adapter.
pub(super) const ADMITTED_WORLD_PRODUCERS: usize = 1;
/// The world-state producer count still awaiting source-specific bounded
/// ownership and exact predicate parity.
pub(super) const WORLD_PRODUCER_RESIDUALS: usize = 36;
/// The Kura producer count awaiting an authenticated bounded reader/projection.
pub(super) const KURA_PRODUCER_RESIDUALS: usize = 3;

/// Execute a legacy iterable only for callers which did not attach the
/// server-owned ordinary memory corridor.
///
/// The limit-bearing branch deliberately returns before source execution. A
/// preflight followed by the legacy producer would not couple its infallible
/// clones and eager collections to the admitted allocation envelope.
pub(super) fn execute<T, Q>(
    query: Q,
    predicate: CompoundPredicate<T>,
    params: &QueryParams,
    mode: OrdinaryCursorMode,
    limits: Option<OrdinaryQueryExecutionLimits>,
    state: &impl StateReadOnly,
) -> Result<impl Iterator<Item = T>, Error>
where
    T: NoritoSerialize + for<'de> norito::core::NoritoDeserialize<'de> + Send + Sync + 'static,
    Q: ValidQuery<Item = T> + 'static,
{
    if let Some(limits) = limits {
        if TypeId::of::<Q>()
            == TypeId::of::<iroha_data_model::query::peer::prelude::FindPeers>()
            && TypeId::of::<T>() == TypeId::of::<PeerId>()
            && mode == OrdinaryCursorMode::Ephemeral
            && params.pagination.offset_value() == 0
            && params.sorting.sort_by_metadata_key.is_none()
            && predicate.is_pass()
        {
            drop(predicate);
            let rows = collect_peers(params, limits, state)?;
            return Ok(OrdinaryIterable::Peers(cast_owned_exact::<
                ExactOwnedRows<PeerId>,
                ExactOwnedRows<T>,
            >(rows)?));
        }
        // TODO: Route each of the remaining 36 world producers through a query-specific
        // borrowed scan which preserves its synthetic-field predicate rules,
        // owns only the requested prefix/top-K through fallible exact storage,
        // and performs bounded selector projection. The three Kura producers
        // additionally need an authenticated fixed projection in the reader.
        return Err(Error::Conversion(
            "ordinary iterable source adapters are not yet complete".to_owned(),
        ));
    }
    Ok(OrdinaryIterable::Legacy(ValidQuery::execute(
        query, predicate, state,
    )?))
}

enum OrdinaryIterable<I, T> {
    Legacy(I),
    Peers(ExactOwnedRows<T>),
}

impl<I, T> Iterator for OrdinaryIterable<I, T>
where
    I: Iterator<Item = T>,
{
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Legacy(iter) => iter.next(),
            Self::Peers(iter) => iter.next(),
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        match self {
            Self::Legacy(iter) => iter.size_hint(),
            Self::Peers(iter) => iter.size_hint(),
        }
    }
}

fn cast_owned_exact<From: 'static, To: 'static>(value: From) -> Result<To, Error> {
    if TypeId::of::<From>() != TypeId::of::<To>() {
        return Err(Error::Conversion(
            "ordinary iterable source/item type mismatch".to_owned(),
        ));
    }
    let value = core::mem::ManuallyDrop::new(value);
    // SAFETY: equality of `'static` TypeIds proves the concrete types match;
    // `ManuallyDrop` transfers the one owned value through this read.
    Ok(unsafe { (&*value as *const From).cast::<To>().read() })
}

/// Serialize a predicate candidate through the checked ordinary JSON helper.
pub(crate) fn predicate_json_value_for_execution<T: JsonSerialize + ?Sized>(
    value: &T,
) -> Option<Value> {
    iroha_data_model::query::json::predicate_json_value_for_execution(value)
}

fn try_exact_uninit_box<T>(len: usize) -> Result<(Box<[MaybeUninit<T>]>, u64), Error> {
    let layout = Layout::array::<MaybeUninit<T>>(len).map_err(|_| Error::CapacityLimit)?;
    let bytes = u64::try_from(layout.size()).map_err(|_| Error::CapacityLimit)?;
    let allocation = if layout.size() == 0 {
        NonNull::<MaybeUninit<T>>::dangling()
    } else {
        // SAFETY: `layout` describes exactly `len` consecutive slots. Null is
        // handled before ownership is constructed; `Box` later deallocates the
        // identical slice layout.
        NonNull::new(unsafe { alloc(layout).cast::<MaybeUninit<T>>() })
            .ok_or(Error::CapacityLimit)?
    };
    let slice = core::ptr::slice_from_raw_parts_mut(allocation.as_ptr(), len);
    // SAFETY: `slice` is the exact allocation above (or the aligned dangling
    // representation permitted for a zero-sized allocation) and is unique.
    Ok((unsafe { Box::from_raw(slice) }, bytes))
}

pub(super) struct ExactOwnedRows<T> {
    slots: Box<[MaybeUninit<T>]>,
    len: usize,
    next: usize,
}

impl<T> ExactOwnedRows<T> {
    pub(super) fn new(len: usize, maximum_bytes: u64) -> Result<Self, Error> {
        let layout = Layout::array::<MaybeUninit<T>>(len).map_err(|_| Error::CapacityLimit)?;
        let bytes = u64::try_from(layout.size()).map_err(|_| Error::CapacityLimit)?;
        if bytes > maximum_bytes {
            return Err(Error::CapacityLimit);
        }
        let (slots, allocated_bytes) = try_exact_uninit_box(len)?;
        debug_assert_eq!(bytes, allocated_bytes);
        Ok(Self {
            slots,
            len: 0,
            next: 0,
        })
    }

    pub(super) fn push(&mut self, value: T) -> Result<(), Error> {
        let Some(slot) = self.slots.get_mut(self.len) else {
            return Err(Error::CapacityLimit);
        };
        slot.write(value);
        self.len += 1;
        Ok(())
    }

    pub(super) fn finish(self) -> Result<Self, Error> {
        if self.len != self.slots.len() {
            return Err(Error::CapacityLimit);
        }
        Ok(self)
    }

    pub(super) fn into_vec(self) -> Result<Vec<T>, Error> {
        if self.next != 0 || self.len != self.slots.len() {
            return Err(Error::CapacityLimit);
        }
        let this = core::mem::ManuallyDrop::new(self);
        let slots = unsafe { core::ptr::read(&this.slots) };
        let len = slots.len();
        let raw = Box::into_raw(slots) as *mut MaybeUninit<T>;
        let raw = core::ptr::slice_from_raw_parts_mut(raw.cast::<T>(), len);
        // SAFETY: `finish` established that every slot is initialized, and
        // `MaybeUninit<T>` has the same layout as `T`.
        Ok(unsafe { Box::from_raw(raw) }.into_vec())
    }
}

impl<T> Iterator for ExactOwnedRows<T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.next == self.len {
            return None;
        }
        let index = self.next;
        self.next += 1;
        // SAFETY: `index < len` is initialized and transferred once.
        Some(unsafe { self.slots[index].assume_init_read() })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.len.saturating_sub(self.next);
        (remaining, Some(remaining))
    }
}

impl<T> ExactSizeIterator for ExactOwnedRows<T> {}

impl<T> Drop for ExactOwnedRows<T> {
    fn drop(&mut self) {
        for slot in &mut self.slots[self.next..self.len] {
            // SAFETY: precisely the unconsumed initialized suffix remains.
            unsafe { slot.assume_init_drop() };
        }
    }
}

struct ExactFrameWriter {
    bytes: Box<[MaybeUninit<u8>]>,
    written: usize,
}

impl ExactFrameWriter {
    fn new(len: usize) -> Result<Self, Error> {
        let (bytes, _) = try_exact_uninit_box(len)?;
        Ok(Self { bytes, written: 0 })
    }

    fn finish(self) -> Result<Box<[u8]>, Error> {
        if self.written != self.bytes.len() {
            return Err(Error::CapacityLimit);
        }
        // SAFETY: the exact writer initialized every byte in the box.
        Ok(unsafe { self.bytes.assume_init() })
    }
}

impl io::Write for ExactFrameWriter {
    fn write(&mut self, source: &[u8]) -> io::Result<usize> {
        let remaining = self.bytes.len().saturating_sub(self.written);
        if remaining == 0 && !source.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::WriteZero,
                "canonical frame exceeded its counted length",
            ));
        }
        let take = remaining.min(source.len());
        for (slot, byte) in self.bytes[self.written..self.written + take]
            .iter_mut()
            .zip(&source[..take])
        {
            slot.write(*byte);
        }
        self.written += take;
        Ok(take)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Encode one canonical frame through an exact fallible allocation. Converting
/// the completed box into a vector is zero-copy and preserves `capacity == len`.
pub(super) fn encode_bounded_frame<T: NoritoSerialize>(
    value: &T,
    maximum: usize,
) -> Result<Vec<u8>, Error> {
    let encoded_len = norito::core::encoded_frame_len(value).map_err(|_| Error::CapacityLimit)?;
    if encoded_len > maximum {
        return Err(Error::CapacityLimit);
    }
    let mut writer = ExactFrameWriter::new(encoded_len)?;
    norito::core::write_canonical_to_writer(value, &mut writer)
        .map_err(|_| Error::CapacityLimit)?;
    Ok(writer.finish()?.into_vec())
}

fn decode_bounded_frame<T>(
    frame: &[u8],
    maximum_allocated_bytes: usize,
) -> Result<(T, usize), Error>
where
    T: for<'de> norito::core::NoritoDeserialize<'de>,
{
    let elements = frame.len().checked_mul(8).ok_or(Error::CapacityLimit)?;
    let limits = norito::DecodeLimits::new(
        elements,
        frame.len(),
        elements,
        maximum_allocated_bytes,
        norito::core::MAX_OWNED_VALUE_DECODE_DEPTH,
    );
    let (decoded, usage) = norito::core::with_decode_limits_measured(limits, || {
        norito::decode_from_bytes_with_limits::<T>(frame, limits)
    });
    Ok((decoded.map_err(|_| Error::CapacityLimit)?, usage.total_allocated_bytes()))
}

fn collect_peers(
    params: &QueryParams,
    limits: OrdinaryQueryExecutionLimits,
    state: &impl StateReadOnly,
) -> Result<ExactOwnedRows<PeerId>, Error> {
    let maximum = usize::try_from(limits.max_source_item_bytes())
        .map_err(|_| Error::CapacityLimit)?;
    let fetch = params
        .fetch_size
        .fetch_size
        .unwrap_or(iroha_data_model::query::parameters::DEFAULT_FETCH_SIZE)
        .get();
    let maximum_rows = peer_prefix_target(
        fetch,
        params.pagination.limit_value().map(|limit| limit.get()),
    )?;
    if maximum_rows > limits.max_page_items().checked_add(1).ok_or(Error::CapacityLimit)? {
        return Err(Error::CapacityLimit);
    }
    let source_maximum = maximum_rows
        .checked_mul(limits.max_source_item_bytes())
        .ok_or(Error::CapacityLimit)?;
    let mut selected = 0_u64;
    let mut visited = 0_u64;
    let mut traversed_bytes = 0_u64;

    for peer in state.world().peers() {
        let frame = encode_bounded_frame(peer, maximum)?;
        visited = visited.checked_add(1).ok_or(Error::CapacityLimit)?;
        traversed_bytes = traversed_bytes
            .checked_add(
                u64::try_from(frame.len())
                    .map_err(|_| Error::CapacityLimit)?
                    .checked_mul(3)
                    .ok_or(Error::CapacityLimit)?,
            )
            .ok_or(Error::CapacityLimit)?;
        limits
            .execution_budget()
            .ensure(visited, traversed_bytes)
            .map_err(|_| Error::CapacityLimit)?;
        drop(frame);
        selected = selected.checked_add(1).ok_or(Error::CapacityLimit)?;
        if selected == maximum_rows {
            break;
        }
    }

    let selected = usize::try_from(selected).map_err(|_| Error::CapacityLimit)?;
    let mut rows = ExactOwnedRows::new(selected, source_maximum)?;
    let mut retained_bytes = 0_u64;
    for peer in state.world().peers() {
        visited = visited.checked_add(1).ok_or(Error::CapacityLimit)?;
        limits
            .execution_budget()
            .ensure(visited, traversed_bytes)
            .map_err(|_| Error::CapacityLimit)?;
        let frame = encode_bounded_frame(peer, maximum)?;
        traversed_bytes = traversed_bytes
            .checked_add(
                u64::try_from(frame.len())
                    .map_err(|_| Error::CapacityLimit)?
                    .checked_mul(3)
                    .ok_or(Error::CapacityLimit)?,
            )
            .ok_or(Error::CapacityLimit)?;
        limits
            .execution_budget()
            .ensure(visited, traversed_bytes)
            .map_err(|_| Error::CapacityLimit)?;
        let (owned, allocated) = decode_bounded_frame::<PeerId>(&frame, maximum)?;
        let resident = core::mem::size_of::<PeerId>()
            .checked_add(allocated)
            .and_then(|bytes| u64::try_from(bytes).ok())
            .ok_or(Error::CapacityLimit)?;
        retained_bytes = retained_bytes
            .checked_add(resident)
            .ok_or(Error::CapacityLimit)?;
        if resident > limits.max_source_item_bytes() || retained_bytes > source_maximum {
            return Err(Error::CapacityLimit);
        }
        rows.push(owned)?;
        if rows.len == selected {
            break;
        }
    }
    rows.finish()
}

fn peer_prefix_target(fetch: u64, limit: Option<u64>) -> Result<u64, Error> {
    let probe = fetch.checked_add(1).ok_or(Error::CapacityLimit)?;
    Ok(limit.map_or(probe, |limit| limit.min(probe)))
}

/// Fixed-layout fallible max-heap used to retain the smallest `capacity`
/// values without `Vec`/`BinaryHeap` capacity excess.
pub(super) struct ExactTopK<T> {
    slots: Box<[MaybeUninit<T>]>,
    len: usize,
    next: usize,
}

impl<T: Ord> ExactTopK<T> {
    /// Allocate exactly `keep` slots, rejecting layout overflow, the caller's
    /// byte ceiling, or allocator failure before any item is inserted.
    pub(super) fn new(keep: usize, maximum_bytes: u64) -> Result<(Self, u64), Error> {
        let layout = Layout::array::<MaybeUninit<T>>(keep).map_err(|_| Error::CapacityLimit)?;
        let bytes = u64::try_from(layout.size()).map_err(|_| Error::CapacityLimit)?;
        if bytes > maximum_bytes {
            return Err(Error::CapacityLimit);
        }
        let (slots, allocated_bytes) = try_exact_uninit_box(keep)?;
        debug_assert_eq!(bytes, allocated_bytes);
        Ok((
            Self {
                slots,
                len: 0,
                next: 0,
            },
            bytes,
        ))
    }

    fn initialized(&self) -> &[T] {
        // SAFETY: every slot below `len` is initialized and `MaybeUninit<T>`
        // has the same layout as `T`.
        unsafe { core::slice::from_raw_parts(self.slots.as_ptr().cast::<T>(), self.len) }
    }

    fn initialized_mut(&mut self) -> &mut [T] {
        // SAFETY: every slot below `len` is initialized and uniquely borrowed.
        unsafe { core::slice::from_raw_parts_mut(self.slots.as_mut_ptr().cast::<T>(), self.len) }
    }

    fn sift_up(&mut self, mut index: usize) {
        let values = self.initialized_mut();
        while index > 0 {
            let parent = (index - 1) / 2;
            if values[parent] >= values[index] {
                break;
            }
            values.swap(parent, index);
            index = parent;
        }
    }

    fn sift_down(&mut self, mut index: usize) {
        let len = self.len;
        let values = self.initialized_mut();
        loop {
            let left = index.saturating_mul(2).saturating_add(1);
            if left >= len {
                break;
            }
            let right = left + 1;
            let largest = if right < len && values[right] > values[left] {
                right
            } else {
                left
            };
            if values[index] >= values[largest] {
                break;
            }
            values.swap(index, largest);
            index = largest;
        }
    }

    /// Retain the smallest `capacity` values and return the displaced value.
    pub(super) fn retain_smallest(&mut self, value: T) -> Option<T> {
        if self.slots.is_empty() {
            return Some(value);
        }
        if self.len < self.slots.len() {
            self.slots[self.len].write(value);
            self.len += 1;
            self.sift_up(self.len - 1);
            return None;
        }
        if self.initialized()[0] <= value {
            return Some(value);
        }
        let displaced = core::mem::replace(&mut self.initialized_mut()[0], value);
        self.sift_down(0);
        Some(displaced)
    }

    /// Convert the retained heap into ascending iterator order in-place.
    pub(super) fn into_sorted(mut self) -> Self {
        self.initialized_mut().sort_unstable();
        self.next = 0;
        self
    }
}

impl<T> Iterator for ExactTopK<T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.next == self.len {
            return None;
        }
        let index = self.next;
        self.next += 1;
        // SAFETY: `index < len` is initialized and advancing `next` transfers
        // that slot's ownership exactly once.
        Some(unsafe { self.slots[index].assume_init_read() })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.len.saturating_sub(self.next);
        (remaining, Some(remaining))
    }
}

impl<T> ExactSizeIterator for ExactTopK<T> {}

impl<T> Drop for ExactTopK<T> {
    fn drop(&mut self) {
        for slot in &mut self.slots[self.next..self.len] {
            // SAFETY: precisely the unconsumed initialized suffix remains
            // owned by this container.
            unsafe { slot.assume_init_drop() };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn residual_inventory_is_explicit_and_exhaustive() {
        assert_eq!(ADMITTED_WORLD_PRODUCERS, 1);
        assert_eq!(WORLD_PRODUCER_RESIDUALS, 36);
        assert_eq!(KURA_PRODUCER_RESIDUALS, 3);
        assert_eq!(
            ADMITTED_WORLD_PRODUCERS + WORLD_PRODUCER_RESIDUALS + KURA_PRODUCER_RESIDUALS,
            40
        );
    }

    #[test]
    fn bounded_peer_prefix_matches_ephemeral_pagination_probe() {
        assert_eq!(peer_prefix_target(16, None).expect("probe"), 17);
        assert_eq!(peer_prefix_target(16, Some(7)).expect("short limit"), 7);
        assert_eq!(peer_prefix_target(16, Some(16)).expect("page limit"), 16);
        assert_eq!(peer_prefix_target(16, Some(32)).expect("long limit"), 17);
    }

    #[test]
    fn exact_topk_retains_smallest_values_in_order() {
        let (mut heap, bytes) = ExactTopK::new(3, 3 * 8).expect("three exact u64 slots");
        assert_eq!(bytes, 3 * 8);
        for value in [7_u64, 2, 11, 5, 3] {
            let _ = heap.retain_smallest(value);
        }
        assert_eq!(heap.into_sorted().collect::<Vec<_>>(), [2, 3, 5]);
    }

    #[test]
    fn exact_topk_rejects_max_minus_one_before_allocation() {
        assert!(ExactTopK::<u64>::new(3, 3 * 8 - 1).is_err());
    }

    #[test]
    fn exact_canonical_frame_has_no_capacity_excess() {
        let expected = norito::core::encoded_frame_len(&17_u64).expect("count frame");
        let bytes = encode_bounded_frame(&17_u64, expected).expect("exact maximum");
        assert_eq!(bytes.len(), expected);
        assert_eq!(bytes.capacity(), expected);
        assert!(encode_bounded_frame(&17_u64, expected - 1).is_err());
    }

    #[test]
    fn ordinary_source_never_reaches_legacy_execute() {
        let source = include_str!("ordinary_iterable.rs");
        let boundary = source
            .split_once("pub(super) fn execute")
            .expect("ordinary execution boundary")
            .1
            .split_once("pub(crate) fn predicate_json_value_for_execution")
            .expect("end of execution boundary")
            .0;
        let limited = boundary.find("if let Some(limits) = limits").expect("limit gate");
        assert!(limited < boundary.find("ValidQuery::execute").expect("legacy path"));
        assert!(!source.contains(concat!("new_uninit_", "slice")));
        assert!(!source.contains(concat!("BinaryHeap", "::with_capacity")));
        assert!(!source.contains(concat!("to_bytes_", "bounded(")));
    }
}
