//! Isolated allocator observations for exact physical backing and original charges.

use std::alloc::Layout;
use std::alloc::{GlobalAlloc, System};
use std::cell::Cell;

#[derive(Clone, Copy)]
pub(crate) struct Record {
    pub(crate) pointer: usize,
    pub(crate) layout: Layout,
    pub(crate) freed: bool,
    pub(crate) refunded: bool,
}

thread_local! {
    static RECORDS: Cell<[Option<Record>; 128]> = const { Cell::new([None; 128]) };
    static EXPECTED: Cell<Option<(usize, Layout)>> = const { Cell::new(None) };
    // Control-shell constructors prepay several layouts before allocating them.
    // Keep that explicit witness separate from the strict next-allocation mode.
    static EXPECTED_BATCH: Cell<[Option<(usize, Layout)>; 8]> = const { Cell::new([None; 8]) };
    static ALLOCATIONS: Cell<Option<usize>> = const { Cell::new(None) };
    static REFUSE_NEXT: Cell<Option<Layout>> = const { Cell::new(None) };
}

struct ObservedAllocator;

unsafe impl GlobalAlloc for ObservedAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if REFUSE_NEXT
            .try_with(|next| {
                if next.get() == Some(layout) {
                    next.set(None);
                    true
                } else {
                    false
                }
            })
            .unwrap_or(false)
        {
            return std::ptr::null_mut();
        }
        let pointer = unsafe { System.alloc(layout) };
        let _ = ALLOCATIONS.try_with(|allocations| {
            if let Some(count) = allocations.get() {
                allocations.set(Some(count + 1));
            }
        });
        if !pointer.is_null() {
            let _ = EXPECTED.try_with(|expected| {
                if let Some((id, exact)) = expected.get().filter(|(_, exact)| *exact == layout) {
                    expected.set(None);
                    let _ = RECORDS.try_with(|records| {
                        let mut all = records.get();
                        all[id] = Some(Record {
                            pointer: pointer as usize,
                            layout: exact,
                            freed: false,
                            refunded: false,
                        });
                        records.set(all);
                    });
                }
            });
            let _ = EXPECTED_BATCH.try_with(|pending| {
                let mut batch = pending.get();
                if let Some(slot) = batch
                    .iter_mut()
                    .find(|entry| entry.is_some_and(|(_, exact)| exact == layout))
                {
                    let (id, exact) = slot.take().expect("matched pending layout");
                    pending.set(batch);
                    let _ = RECORDS.try_with(|records| {
                        let mut all = records.get();
                        all[id] = Some(Record {
                            pointer: pointer as usize,
                            layout: exact,
                            freed: false,
                            refunded: false,
                        });
                        records.set(all);
                    });
                }
            });
        }
        pointer
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        // SAFETY: preserve the original zeroing contract after recording the request.
        let pointer = unsafe { self.alloc(layout) };
        if !pointer.is_null() {
            unsafe { pointer.write_bytes(0, layout.size()) };
        }
        pointer
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        let _ = ALLOCATIONS.try_with(|count| {
            if let Some(previous) = count.get() {
                count.set(Some(previous + 1));
            }
        });
        // SAFETY: preserve the live allocation, original layout and requested size.
        unsafe { System.realloc(pointer, layout, size) }
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(pointer, layout) };
        let _ = RECORDS.try_with(|records| {
            let mut all = records.get();
            for record in all.iter_mut().flatten() {
                if record.pointer == pointer as usize && record.layout == layout && !record.freed {
                    record.freed = true;
                }
            }
            records.set(all);
        });
    }
}

#[global_allocator]
static ALLOCATOR: ObservedAllocator = ObservedAllocator;

// Intentionally neither Clone nor Default. Its size also changes real padding.
pub(crate) struct Charge {
    id: usize,
    layout: Layout,
    _storage: [u8; 129],
}

impl Drop for Charge {
    fn drop(&mut self) {
        RECORDS.with(|records| {
            let mut all = records.get();
            let record = all[self.id]
                .as_mut()
                .expect("the charged node was allocated");
            assert_eq!(record.layout, self.layout);
            assert!(record.freed, "node credits returned before System.dealloc");
            assert!(!record.refunded, "original charge returned twice");
            record.refunded = true;
            records.set(all);
        });
    }
}

pub(crate) struct Prepaid {
    pub(crate) next: usize,
    // A finite admitted count suffices for this allocator-observation fixture;
    // production must prepay the complete exact layout sum before mutation.
    pub(crate) remaining: usize,
}

// This fixture observes node storage only. Nested funded payloads use the
// explicit ownership policy exercised by cloning_tests.rs.

impl Prepaid {
    /// Observe one original admitted node or tracking-buffer allocation.
    pub(crate) fn take_allocation_charge(&mut self, layout: Layout) -> Charge {
        EXPECTED_BATCH.with(|pending| assert!(pending.get().iter().all(Option::is_none)));
        self.take_charge(layout, false)
    }

    fn take_charge(&mut self, layout: Layout, batch: bool) -> Charge {
        assert!(self.remaining > 0, "operation exceeded original admission");
        self.remaining -= 1;
        let id = self.next;
        self.next += 1;
        if layout.size() == 0 {
            // Empty/ZST backing storage never reaches the allocator. Preserve
            // its separate original charge without inventing an allocation.
            EXPECTED.with(|expected| assert!(expected.get().is_none()));
            RECORDS.with(|records| {
                let mut all = records.get();
                all[id] = Some(Record {
                    pointer: 0,
                    layout,
                    freed: true,
                    refunded: false,
                });
                records.set(all);
            });
        } else if batch {
            EXPECTED_BATCH.with(|pending| {
                let mut entries = pending.get();
                *entries
                    .iter_mut()
                    .find(|entry| entry.is_none())
                    .expect("bounded pending shell layouts") = Some((id, layout));
                pending.set(entries);
            });
        } else {
            EXPECTED.with(|expected| assert!(expected.replace(Some((id, layout))).is_none()));
        }
        Charge {
            id,
            layout,
            _storage: [0; 129],
        }
    }
}

pub(crate) fn prepaid() -> Prepaid {
    RECORDS.with(|records| records.set([None; 128]));
    EXPECTED.with(|expected| expected.set(None));
    EXPECTED_BATCH.with(|pending| pending.set([None; 8]));
    Prepaid {
        next: 0,
        remaining: 128,
    }
}

pub(crate) fn record(id: usize) -> Record {
    RECORDS.with(|records| records.get()[id].expect("original allocation record"))
}

pub(crate) fn all_refunded(funding: &Prepaid) {
    assert!(EXPECTED.with(Cell::get).is_none());
    EXPECTED_BATCH.with(|pending| assert!(pending.get().iter().all(Option::is_none)));
    for id in 0..funding.next {
        assert!(record(id).refunded);
    }
}

/// Check a real operation without leaving allocator observation armed on panic.
pub(crate) fn without_allocations<T>(action: impl FnOnce() -> T) -> T {
    struct Restore;
    impl Drop for Restore {
        fn drop(&mut self) {
            ALLOCATIONS.with(|allocations| allocations.set(None));
        }
    }
    ALLOCATIONS.with(|allocations| {
        assert!(allocations.get().is_none(), "nested allocation observation");
        allocations.set(Some(0));
    });
    let restore = Restore;
    let result = action();
    let allocations = ALLOCATIONS.with(|allocations| allocations.get().unwrap());
    drop(restore);
    assert_eq!(allocations, 0);
    result
}

/// Refuse exactly one matching allocation using the existing test allocator.
/// Unrelated allocations and other test threads retain their original behavior.
pub(crate) fn refusing_allocation<T>(layout: Layout, action: impl FnOnce() -> T) -> T {
    struct Restore;
    impl Drop for Restore {
        fn drop(&mut self) {
            REFUSE_NEXT.with(|next| next.set(None));
        }
    }
    REFUSE_NEXT.with(|next| {
        assert!(next.get().is_none(), "nested allocation refusal");
        next.set(Some(layout));
    });
    let restore = Restore;
    let result = action();
    let refused = REFUSE_NEXT.with(|next| next.get().is_none());
    drop(restore);
    assert!(refused, "the exact allocation did not reach the allocator");
    result
}
