//! Original charged storage for a transaction's ordered touched-key set.

use std::{alloc::Layout, mem::MaybeUninit, slice};

use concread::bptree::PlanningError;

/// Original fixed key storage; payloads and the buffer keep separate charges.
pub struct AdmittedKeys<K, Charge> {
    // Field order matters: the actual allocation is freed before its charge.
    entries: Box<[MaybeUninit<K>]>,
    initialized: usize,
    _charge: Option<Charge>,
}

/// An allocation-free insertion observation under an exclusive owner borrow.
pub(super) struct KeyInsertion {
    index: usize,
    growth: Option<(usize, Layout)>,
}

impl KeyInsertion {
    pub(super) fn layout(&self) -> Option<Layout> {
        self.growth.map(|(_, layout)| layout)
    }
}

impl<K, Charge> AdmittedKeys<K, Charge> {
    pub(super) fn new() -> Self {
        Self {
            entries: Box::new_uninit_slice(0),
            initialized: 0,
            _charge: None,
        }
    }

    pub(super) fn as_slice(&self) -> &[K] {
        // SAFETY: only the exact initialized prefix owns keys. Spare slots
        // never become observable and the Box remains borrowed by this view.
        unsafe { slice::from_raw_parts(self.entries.as_ptr().cast(), self.initialized) }
    }

    fn grow(&mut self, capacity: usize, charge: Charge) {
        assert!(capacity > self.entries.len());
        let mut replacement = Self {
            entries: Box::new_uninit_slice(capacity),
            initialized: 0,
            _charge: Some(charge),
        };
        // SAFETY: the separately owned replacement has enough spare capacity.
        // Transfer the complete initialized prefix without calling Clone or
        // Drop. No fallible/user operation intervenes before custody transfers.
        unsafe {
            std::ptr::copy_nonoverlapping(
                self.entries.as_ptr(),
                replacement.entries.as_mut_ptr(),
                self.initialized,
            );
        }
        replacement.initialized = self.initialized;
        self.initialized = 0;
        // Install the complete successor before releasing the old charge,
        // whose destructor can unwind. Old entries no longer own any keys.
        let previous = std::mem::replace(self, replacement);
        drop(previous);
    }
}

impl<K: Ord, Charge> AdmittedKeys<K, Charge> {
    pub(super) fn insertion(&self, key: &K) -> Result<Option<KeyInsertion>, PlanningError> {
        let index = match self.as_slice().binary_search(key) {
            Ok(_) => return Ok(None),
            Err(index) => index,
        };
        let required = self
            .initialized
            .checked_add(1)
            .ok_or(PlanningError::Overflow)?;
        let growth = if required <= self.entries.len() {
            None
        } else {
            let doubled = self
                .entries
                .len()
                .checked_mul(2)
                .map(|n| n.max(4).max(required));
            let layout = doubled
                .and_then(|n| Layout::array::<K>(n).ok().map(|layout| (n, layout)))
                .or_else(|| {
                    Layout::array::<K>(required)
                        .ok()
                        .map(|layout| (required, layout))
                })
                .ok_or(PlanningError::Overflow)?;
            Some(layout)
        };
        Ok(Some(KeyInsertion { index, growth }))
    }

    pub(super) fn insert(&mut self, plan: KeyInsertion, key: K, charge: Option<Charge>) {
        assert_eq!(self.as_slice().binary_search(&key), Err(plan.index));
        match (plan.growth, charge) {
            (Some((capacity, _)), Some(charge)) => self.grow(capacity, charge),
            (None, None) => {}
            _ => panic!("key growth must consume its original exact-layout charge"),
        }
        assert!(self.initialized < self.entries.len());
        // SAFETY: move the initialized suffix one slot right within the
        // admitted allocation. The insertion slot then receives its sole key;
        // no payload destructor or fallible call runs during the move.
        unsafe {
            let entries = self.entries.as_mut_ptr();
            std::ptr::copy(
                entries.add(plan.index),
                entries.add(plan.index + 1),
                self.initialized - plan.index,
            );
        }
        self.entries[plan.index].write(key);
        self.initialized += 1;
    }
}

struct KeyDrain<'a, K> {
    entries: &'a mut [MaybeUninit<K>],
    initialized: &'a mut usize,
}

impl<K> KeyDrain<'_, K> {
    fn drain(&mut self) {
        while *self.initialized > 0 {
            *self.initialized -= 1;
            // SAFETY: remove the slot from the live prefix before invoking its
            // arbitrary destructor. Unwind cleanup cannot visit this key twice.
            drop(unsafe { self.entries[*self.initialized].assume_init_read() });
        }
    }
}

impl<K> Drop for KeyDrain<'_, K> {
    fn drop(&mut self) {
        self.drain();
    }
}

impl<K, Charge> Drop for AdmittedKeys<K, Charge> {
    fn drop(&mut self) {
        let mut drain = KeyDrain {
            entries: &mut self.entries,
            initialized: &mut self.initialized,
        };
        drain.drain();
        // If a key panics, the local drain drops the remaining keys, then Rust
        // still destroys this owner's Box before its original charge field.
    }
}
