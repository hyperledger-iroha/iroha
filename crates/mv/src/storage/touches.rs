//! Original finite storage for sorted transaction touch keys.
//!
//! This is metadata of the existing Storage transaction, not a second map.
//! A unique touch plans its real fixed array growth and policy-owned key copy;
//! the plan extends the canonical pair demand before the one reservation.
//! No-op and absent-to-absent keys remain present. Duplicate touches neither
//! clone nor rewrite the retained first key. There is no growing iterator.

use crate::{Key, Value, allocation::AllocationCharge};
use concread::bptree::{AllocationDemand, ClonePlanning, NodeCloning, NodeFunding, PlanningError};
use std::{alloc::Layout, mem::MaybeUninit, ptr, slice};

// Field order is custody: payloads are drained by Drop, then the actual backing
// Box deallocates before its original charge refunds, including during unwind.
struct Buffer<K: Key> {
    entries: Box<[MaybeUninit<K>]>,
    initialized: usize,
    _charge: AllocationCharge,
}

impl<K: Key> Buffer<K> {
    fn as_slice(&self) -> &[K] {
        // SAFETY: only install initializes entries, exactly in this prefix.
        unsafe { slice::from_raw_parts(self.entries.as_ptr().cast(), self.initialized) }
    }
}

struct Drain<'a, K: Key> {
    entries: *mut K,
    remaining: &'a mut usize,
}
impl<K: Key> Drain<'_, K> {
    fn drain(&mut self) {
        while *self.remaining != 0 {
            *self.remaining -= 1;
            // SAFETY: remove ownership from the initialized prefix before any
            // arbitrary key destructor; each prior initialized entry moves once.
            let key = unsafe { self.entries.add(*self.remaining).read() };
            drop(key);
        }
    }
}
impl<K: Key> Drop for Drain<'_, K> {
    fn drop(&mut self) {
        self.drain();
    }
}
impl<K: Key> Drop for Buffer<K> {
    fn drop(&mut self) {
        let mut drain = Drain::<K> {
            entries: self.entries.as_mut_ptr().cast(),
            remaining: &mut self.initialized,
        };
        // If one key destructor unwinds, Drain finishes the remaining prefix.
        // Rust then drops this Buffer's fields on either exit: real Box first,
        // charge last. A second destructor panic aborts as in ordinary Rust Drop.
        drain.drain();
    }
}

/// Ordered unique touched keys retaining their actual payload and array owners.
/// Keep all operations/destruction inside the original pool's writer-wide
/// refund-notification scope; aggregate failure must stay armed across cleanup.
pub(super) struct SortedTouches<K: Key> {
    buffer: Option<Buffer<K>>,
}

impl<K: Key> SortedTouches<K> {
    /// No allocation or fabricated zero-capacity charge.
    pub(super) const fn new() -> Self {
        Self { buffer: None }
    }

    #[cfg(test)]
    pub(super) fn len(&self) -> usize {
        self.buffer.as_ref().map_or(0, |buffer| buffer.initialized)
    }

    /// Borrow only the original sorted initialized prefix, without allocation.
    pub(super) fn iter(&self) -> slice::Iter<'_, K> {
        self.buffer
            .as_ref()
            .map_or(&[][..], Buffer::as_slice)
            .iter()
    }

    /// Search/plan while the caller's aggregate failure guard is armed. The
    /// exclusive borrow prevents mutation or a foreign target during preparation.
    /// Extend the original pair demand directly, retaining its checked allocation
    /// count and byte sum. Duplicate touches return that demand unchanged.
    pub(super) fn plan<'set, 'key, V, P>(
        &'set mut self,
        key: &'key K,
        mut demand: AllocationDemand,
    ) -> Result<TouchPlan<'set, 'key, K>, PlanningError>
    where
        V: Value,
        P: ClonePlanning<K, V> + NodeFunding<Charge = AllocationCharge>,
    {
        let entries = self.buffer.as_ref().map_or(&[][..], Buffer::as_slice);
        let index = match entries.binary_search(key) {
            Ok(_) => None,
            Err(index) => Some(index),
        };
        let growth = if index.is_some() {
            let capacity = self
                .buffer
                .as_ref()
                .map_or(0, |buffer| buffer.entries.len());
            let growth = growth::<K>(entries.len(), capacity)?;
            if let Some(growth) = &growth {
                demand.add_layout(growth.layout)?;
            }
            P::plan_key(key, &mut demand)?;
            growth
        } else {
            None
        };
        Ok(TouchPlan {
            target: self,
            key,
            index,
            growth,
            demand,
        })
    }
}

struct Growth {
    capacity: usize,
    layout: Layout,
}
fn growth<K>(initialized: usize, capacity: usize) -> Result<Option<Growth>, PlanningError> {
    let needed = initialized.checked_add(1).ok_or(PlanningError::Overflow)?;
    if needed <= capacity {
        return Ok(None);
    }
    let enlarged = capacity.checked_mul(2).map(|doubled| doubled.max(needed));
    let (capacity, layout) = enlarged
        .and_then(|capacity| {
            Layout::array::<MaybeUninit<K>>(capacity)
                .ok()
                .map(|layout| (capacity, layout))
        })
        .or_else(|| {
            Layout::array::<MaybeUninit<K>>(needed)
                .ok()
                .map(|layout| (needed, layout))
        })
        .ok_or(PlanningError::Overflow)?;
    Ok(Some(Growth { capacity, layout }))
}

/// One complete checked pair-plus-touch demand for this exact target and key.
pub(super) struct TouchPlan<'set, 'key, K: Key> {
    target: &'set mut SortedTouches<K>,
    key: &'key K,
    index: Option<usize>,
    growth: Option<Growth>,
    demand: AllocationDemand,
}
impl<'set, K: Key> TouchPlan<'set, '_, K> {
    pub(super) fn demand(&self) -> AllocationDemand {
        self.demand
    }

    /// Consume the touch part of the same complete original provider before
    /// returning its remainder to the two-map executor. No target entry moves.
    /// The prepared owner no longer borrows the incoming key, which can then
    /// move into the actual map edit. Clone policy preserves canonical ordering.
    pub(super) fn prepare<V, P>(self, provider: &mut P) -> PreparedTouch<'set, K>
    where
        V: Value,
        P: ClonePlanning<K, V> + NodeFunding<Charge = AllocationCharge>,
    {
        let replacement = self.growth.map(|growth| {
            let charge = provider.take_node_charge(growth.layout);
            assert_eq!(
                charge.layout(),
                growth.layout,
                "exact original touch array charge"
            );
            Buffer {
                entries: Box::<[K]>::new_uninit_slice(growth.capacity),
                initialized: 0,
                _charge: charge,
            }
        });
        let key = self
            .index
            .map(|_| <P as NodeCloning<K, V>>::clone_key(provider, self.key));
        PreparedTouch {
            target: self.target,
            key,
            index: self.index,
            replacement,
        }
    }
}

/// Prepared key and optional exact array, still separate from the original set.
/// Abandonment drops only these newly admitted owners; the target is untouched.
pub(super) struct PreparedTouch<'set, K: Key> {
    target: &'set mut SortedTouches<K>,
    key: Option<K>,
    index: Option<usize>,
    replacement: Option<Buffer<K>>,
}

/// An emptied superseded array. Drop explicitly while aggregate failure remains
/// armed: deallocation precedes credit refund and no key destructor remains.
#[must_use = "drop the original retired touch array under the aggregate failure guard"]
pub(super) struct TouchRetirement<K: Key> {
    _buffer: Option<Buffer<K>>,
}

impl<K: Key> PreparedTouch<'_, K> {
    /// Move keys and install at the already planned index without comparison,
    /// allocation or user destruction. The exclusive target borrow proves the
    /// prefix/capacity still match the plan. Cleanup is returned to the caller.
    pub(super) fn install(self) -> TouchRetirement<K> {
        let Self {
            target,
            key,
            index,
            replacement,
        } = self;
        let Some(key) = key else {
            return TouchRetirement { _buffer: None };
        };
        let retired = replacement.and_then(|mut replacement| {
            let mut old = target.buffer.take();
            if let Some(old) = old.as_mut() {
                // SAFETY: distinct live backing allocations, no payload callback,
                // new capacity covers all old entries plus the new unique key.
                unsafe {
                    ptr::copy_nonoverlapping(
                        old.entries.as_ptr().cast::<K>(),
                        replacement.entries.as_mut_ptr().cast::<K>(),
                        old.initialized,
                    );
                }
                replacement.initialized = old.initialized;
                old.initialized = 0;
            }
            target.buffer = Some(replacement);
            old
        });
        let buffer = target
            .buffer
            .as_mut()
            .expect("planned nonempty touch array");
        let index = index.expect("prepared unique key index");
        // SAFETY: exclusive plan fixes index <= initialized and one spare slot.
        // Overlapping move shifts the suffix without cloning or dropping keys.
        unsafe {
            let slot = buffer.entries.as_mut_ptr().cast::<K>().add(index);
            ptr::copy(slot, slot.add(1), buffer.initialized - index);
            slot.write(key);
        }
        buffer.initialized += 1;
        TouchRetirement { _buffer: retired }
    }
}

#[cfg(test)]
#[path = "touches_tests.rs"]
mod tests;
