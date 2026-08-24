//! Allocation primitives and the fail-closed boundary for ordinary iterables.
//!
//! A generic [`ValidQuery`] iterator is not a source proof: producer-local clones, candidate
//! collections, and synthetic-field predicate evaluators can allocate before pagination observes an
//! item. Unadapted producers stop before `ValidQuery::execute`; each admitted producer instead owns
//! rows through its query-specific bounded adapter.
#![allow(unsafe_code)]
use super::{
    OrdinaryQueryExecutionLimits, QueryExecutionStats, ordinary_memory::OrdinaryCursorMode,
};
use crate::{
    smartcontracts::ValidQuery,
    state::{StateReadOnly, WorldReadOnly},
};
use iroha_data_model::{
    peer::PeerId,
    query::{
        QueryOutputBatchBox, QueryOutputBatchBoxTuple, dsl::CompoundPredicate,
        error::QueryExecutionFail as Error, parameters::QueryParams,
    },
};
use norito::{
    core::NoritoSerialize,
    json::{JsonSerialize, Value},
};
use std::{
    alloc::{Layout, alloc},
    any::TypeId,
    io,
    mem::MaybeUninit,
    ptr::NonNull,
};
/// The world-state producer count admitted through a source-specific adapter.
#[cfg(test)]
pub(super) const ADMITTED_WORLD_PRODUCERS: usize = 1;
/// The world-state producer count still awaiting source-specific bounded
/// ownership and exact predicate parity.
#[cfg(test)]
pub(super) const WORLD_PRODUCER_RESIDUALS: usize = 36;
/// The Kura producer count awaiting an authenticated bounded reader/projection.
#[cfg(test)]
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
) -> Result<(impl Iterator<Item = T>, QueryExecutionStats), Error>
where
    T: NoritoSerialize + for<'de> norito::core::NoritoDeserialize<'de> + Send + Sync + 'static,
    Q: ValidQuery<Item = T> + 'static,
{
    if let Some(limits) = limits {
        if TypeId::of::<Q>() == TypeId::of::<iroha_data_model::query::peer::prelude::FindPeers>()
            && TypeId::of::<T>() == TypeId::of::<PeerId>()
            && mode == OrdinaryCursorMode::Ephemeral
            && params.pagination.offset_value() == 0
            && params.sorting.sort_by_metadata_key.is_none()
            && predicate.is_pass()
        {
            drop(predicate);
            let (rows, stats) = collect_peers(params, limits, state)?;
            return Ok((
                OrdinaryIterable::Peers(cast_owned_exact::<
                    ExactOwnedRows<PeerId>,
                    ExactOwnedRows<T>,
                >(rows)?),
                stats,
            ));
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
    Ok((
        OrdinaryIterable::Legacy(ValidQuery::execute(query, predicate, state)?),
        QueryExecutionStats::default(),
    ))
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
    let source: *const From = &*value;
    Ok(unsafe { source.cast::<To>().read() })
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
fn exact_slot_bytes<T>(len: usize) -> Result<u64, Error> {
    let layout = Layout::array::<MaybeUninit<T>>(len).map_err(|_| Error::CapacityLimit)?;
    u64::try_from(layout.size()).map_err(|_| Error::CapacityLimit)
}

/// Aggregate graph budget left after reserving exact inline row slots.
///
/// The per-row ceiling is enforced again immediately before every decode, so
/// neither one row nor the retained set as a whole can exceed the admitted
/// `selected * S` envelope.
struct RetainedDecodeBudget {
    per_item_graph_bytes: u64,
    remaining_graph_bytes: u64,
}
impl RetainedDecodeBudget {
    fn new<T>(rows: usize, per_item_bytes: u64) -> Result<(Self, u64), Error> {
        let rows_u64 = u64::try_from(rows).map_err(|_| Error::CapacityLimit)?;
        let slot_bytes = exact_slot_bytes::<T>(rows)?;
        let aggregate_bytes = rows_u64
            .checked_mul(per_item_bytes)
            .ok_or(Error::CapacityLimit)?;
        let remaining_graph_bytes = aggregate_bytes
            .checked_sub(slot_bytes)
            .ok_or(Error::CapacityLimit)?;
        let inline_bytes =
            u64::try_from(core::mem::size_of::<T>()).map_err(|_| Error::CapacityLimit)?;
        let per_item_graph_bytes = per_item_bytes
            .checked_sub(inline_bytes)
            .ok_or(Error::CapacityLimit)?;
        Ok((
            Self {
                per_item_graph_bytes,
                remaining_graph_bytes,
            },
            slot_bytes,
        ))
    }

    fn next_limit(&self) -> Result<usize, Error> {
        usize::try_from(self.per_item_graph_bytes.min(self.remaining_graph_bytes))
            .map_err(|_| Error::CapacityLimit)
    }

    fn record_decoded(&mut self, allocated_bytes: usize) -> Result<(), Error> {
        let allocated_bytes = u64::try_from(allocated_bytes).map_err(|_| Error::CapacityLimit)?;
        self.remaining_graph_bytes = self
            .remaining_graph_bytes
            .checked_sub(allocated_bytes)
            .ok_or(Error::CapacityLimit)?;
        Ok(())
    }
}
pub(super) struct ExactOwnedRows<T> {
    slots: Box<[MaybeUninit<T>]>,
    len: usize,
    next: usize,
}

/// Build the one-column response tuple without an infallible `vec![..]`
/// allocation at the ordinary-query memory boundary.
pub(super) fn exact_one_column_batch(
    batch: QueryOutputBatchBox,
) -> Result<QueryOutputBatchBoxTuple, Error> {
    let slot_bytes = exact_slot_bytes::<QueryOutputBatchBox>(1)?;
    let mut columns = ExactOwnedRows::new(1, slot_bytes)?;
    columns.push(batch)?;
    QueryOutputBatchBoxTuple::new(columns.finish()?.into_vec()?)
        .map_err(|error| Error::Conversion(error.to_string()))
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
    Ok((
        decoded.map_err(|_| Error::CapacityLimit)?,
        usage.total_allocated_bytes(),
    ))
}
fn collect_peers(
    params: &QueryParams,
    limits: OrdinaryQueryExecutionLimits,
    state: &impl StateReadOnly,
) -> Result<(ExactOwnedRows<PeerId>, QueryExecutionStats), Error> {
    let maximum =
        usize::try_from(limits.max_source_item_bytes()).map_err(|_| Error::CapacityLimit)?;
    let fetch = params
        .fetch_size
        .fetch_size
        .unwrap_or(iroha_data_model::query::parameters::DEFAULT_FETCH_SIZE)
        .get();
    let maximum_rows = peer_prefix_target(
        fetch,
        params.pagination.limit_value().map(|limit| limit.get()),
    )?;
    if maximum_rows
        > limits
            .max_page_items()
            .checked_add(1)
            .ok_or(Error::CapacityLimit)?
    {
        return Err(Error::CapacityLimit);
    }
    let mut selected = 0_u64;
    let mut stats = QueryExecutionStats::default();
    let budget = limits.execution_budget();
    let source_work_per_item = limits
        .max_source_item_bytes()
        .checked_mul(3)
        .ok_or(Error::CapacityLimit)?;
    for peer in state.world().peers() {
        let frame = encode_bounded_frame(peer, maximum)?;
        stats.record_preflighted_item(source_work_per_item, Some(budget))?;
        drop(frame);
        selected = selected.checked_add(1).ok_or(Error::CapacityLimit)?;
        if selected == maximum_rows {
            break;
        }
    }
    let selected = usize::try_from(selected).map_err(|_| Error::CapacityLimit)?;
    let (mut retained_decode, slot_bytes) =
        RetainedDecodeBudget::new::<PeerId>(selected, limits.max_source_item_bytes())?;
    let mut rows = ExactOwnedRows::new(selected, slot_bytes)?;
    if selected == 0 {
        return Ok((rows.finish()?, stats));
    }
    for peer in state.world().peers() {
        let frame = encode_bounded_frame(peer, maximum)?;
        stats.record_preflighted_item(source_work_per_item, Some(budget))?;
        let decode_limit = retained_decode.next_limit()?;
        let (owned, allocated) = decode_bounded_frame::<PeerId>(&frame, decode_limit)?;
        retained_decode.record_decoded(allocated)?;
        rows.push(owned)?;
        if rows.len == selected {
            break;
        }
    }
    Ok((rows.finish()?, stats))
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
    fn retained_decode_budget_reserves_slots_before_graph_decodes() {
        let (mut budget, slot_bytes) =
            RetainedDecodeBudget::new::<u64>(2, 16).expect("two admitted rows");
        assert_eq!(slot_bytes, 2 * 8);
        assert_eq!(budget.remaining_graph_bytes, 2 * 16 - slot_bytes);
        assert_eq!(budget.next_limit().expect("first cap"), 8);
        budget.record_decoded(7).expect("first graph");
        assert_eq!(budget.next_limit().expect("second cap"), 8);
        budget.record_decoded(8).expect("second graph");
        assert_eq!(budget.next_limit().expect("aggregate remainder"), 1);
    }
    #[test]
    fn retained_decode_budget_rejects_inline_slots_above_aggregate_cap() {
        assert!(RetainedDecodeBudget::new::<u64>(1, 7).is_err());
    }
    #[test]
    fn exact_one_column_batch_uses_the_fallible_exact_container() {
        let batch = QueryOutputBatchBox::String(Vec::new());
        let tuple = exact_one_column_batch(batch).expect("one exact column");
        assert_eq!(tuple.column_count(), 1);
    }
    #[test]
    fn ordinary_postprocessing_does_not_recharge_precounted_source_rows() {
        use iroha_data_model::{
            query::{dsl::SelectorTuple, parameters::FetchSize},
            role::RoleId,
        };
        use nonzero_ext::nonzero;
        let budget = super::super::QueryExecutionBudget::from_weighted_limit(128 * 1_024, 1, 1);
        let ordinary = OrdinaryQueryExecutionLimits::try_new(
            1,
            budget,
            16,
            64 * 1_024,
            1_024,
            16 * 1_024,
            16,
            16 * 1_024,
            32 * 1_024,
            16 * 1_024,
            4 * 1_024,
            norito::DecodeLimits::new(64, 4 * 1_024, 256, 16 * 1_024, 16),
        )
        .expect("ordinary limits");
        let params = QueryParams {
            fetch_size: FetchSize::new(Some(nonzero!(2_u64))),
            ..QueryParams::default()
        };
        let source_stats = super::super::QueryExecutionStats {
            processed_items: 6,
            processed_bytes: 18 * 1_024,
        };
        let values = ["one", "two", "probe"].map(|value| {
            value
                .parse::<RoleId>()
                .expect("protocol-bounded role identifier")
        });
        let (output, stats) =
            super::super::apply_query_postprocessing_ephemeral_with_budget_from_stats(
                values.into_iter(),
                SelectorTuple::default(),
                &params,
                super::super::QueryLimits::new(16)
                    .with_count_mode(super::super::QueryCountMode::Bounded)
                    .with_ordinary_execution_limits(ordinary),
                Some(budget),
                source_stats,
            )
            .expect("bounded ordinary postprocessing");
        assert!(output.has_more);
        assert_eq!(stats, source_stats);
    }
    #[test]
    fn peer_adapter_carries_source_work_into_the_final_response_budget() {
        use crate::{
            kura::Kura,
            query::store::LiveQueryStore,
            state::{State, StateReadOnly, World},
        };
        use iroha_crypto::{Algorithm, KeyPair};
        use iroha_data_model::query::{
            ErasedIterQuery, QueryBox, QueryRequest, QueryResponse, QueryWithParams,
            dsl::{CompoundPredicate, SelectorTuple},
            parameters::FetchSize,
            peer::prelude::FindPeers,
        };
        use iroha_test_samples::ALICE_ID;
        use nonzero_ext::nonzero;

        let world = World::default();
        {
            let mut world_block = world.block();
            let mut peers = world_block.peers_mut_for_testing().transaction();
            for seed in [0x31_u8, 0x32, 0x33] {
                let peer = PeerId::new(
                    KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
                        .expect("peer key")
                        .public_key()
                        .clone(),
                );
                let _ = peers.push(peer);
            }
            peers.apply();
            world_block.commit();
        }
        let state = State::new(
            world,
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        let view = state.view();
        let max_page_items = 2_u64;
        let source_bytes = super::super::ORDINARY_NAME_ID_SOURCE_BYTES;
        let response_bytes = 16 * 1_024;
        let archive_bytes = 1_024;
        let decode = norito::DecodeLimits::new(64, 4 * 1_024, 256, 16 * 1_024, 16);
        let execution_headroom = OrdinaryQueryExecutionLimits::required_execution_headroom_bytes(
            max_page_items,
            source_bytes,
            response_bytes,
            4 * 1_024,
            archive_bytes,
            decode,
        )
        .expect("execution geometry");
        let cursor_retained = OrdinaryQueryExecutionLimits::required_cursor_retained_bytes(
            max_page_items,
            source_bytes,
            source_bytes,
            archive_bytes,
        )
        .expect("cursor geometry");
        let ordinary = OrdinaryQueryExecutionLimits::try_new(
            1,
            super::super::QueryExecutionBudget::from_weighted_limit(256 * 1_024, 1, 1),
            max_page_items,
            execution_headroom,
            source_bytes,
            response_bytes,
            max_page_items,
            source_bytes,
            cursor_retained,
            4 * 1_024,
            archive_bytes,
            decode,
        )
        .expect("peer corridor limits");
        let query: QueryBox<QueryOutputBatchBox> = Box::new(ErasedIterQuery::<PeerId>::new(
            CompoundPredicate::PASS,
            SelectorTuple::default(),
            norito::codec::Encode::encode(&FindPeers),
        ));
        let params = QueryParams {
            fetch_size: FetchSize::new(Some(nonzero!(2_u64))),
            ..QueryParams::default()
        };
        let request = super::super::ValidQueryRequest {
            request: QueryRequest::Start(
                QueryWithParams::new(&query, params)
                    .expect("peer query type has a canonical mapping"),
            ),
            limits: super::super::QueryLimits::new(max_page_items)
                .with_count_mode(super::super::QueryCountMode::Bounded)
                .with_ordinary_execution_limits(ordinary),
        };
        let (response, stats) = request
            .execute_ephemeral_with_stats(view.query_handle(), &view, &ALICE_ID, None)
            .expect("bounded peer query");
        let response_work = super::super::bounded_framed_encoded_len(&response, response_bytes)
            .expect("bounded response length");
        assert_eq!(stats.processed_items(), 6, "two passes over F + 1 rows");
        assert_eq!(stats.processed_bytes(), 18 * source_bytes + response_work);
        let QueryResponse::Iterable(output) = response else {
            panic!("expected iterable response")
        };
        assert_eq!(
            output.batch.columns().first().map(QueryOutputBatchBox::len),
            Some(2)
        );
        assert!(output.has_more);
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
        let limited = boundary
            .find("if let Some(limits) = limits")
            .expect("limit gate");
        assert!(limited < boundary.find("ValidQuery::execute").expect("legacy path"));
        assert!(!source.contains(concat!("new_uninit_", "slice")));
        assert!(!source.contains(concat!("BinaryHeap", "::with_capacity")));
        assert!(!source.contains(concat!("to_bytes_", "bounded(")));
    }
}
