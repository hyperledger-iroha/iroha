//! Concrete charged nodes and fixed bookkeeping in the original cursor engine.

use super::super::allocation::NodeFunding;
use super::super::node::allocation_tests::{
    all_refunded, prepaid, record, without_allocations, Charge, Prepaid,
};
use super::super::tracking::FixedTrackingBuffer;
use super::*;
use crate::internals::lincowcell::LinCowCell;
use std::alloc::Layout;
use std::cell::Cell;
use std::panic::{catch_unwind, AssertUnwindSafe};

struct Funded(Prepaid);

impl NodeFunding for Funded {
    type Charge = Charge;

    fn take_node_charge(&mut self, layout: Layout) -> Charge {
        self.0.take_allocation_charge(layout)
    }
}

// These controls isolate node and bookkeeping custody; nested payload custody
// has a separate concrete policy in node::cloning_tests.
impl<K: Clone, V: Clone> NodeCloning<K, V> for Funded {
    fn clone_key(&mut self, key: &K) -> K {
        key.clone()
    }
    fn clone_value(&mut self, value: &V) -> V {
        value.clone()
    }
}

impl<K: Clone + Ord + Debug, V: Clone> CursorMode<K, V> for Funded {
    type Buffer = FixedTrackingBuffer<*mut Node<K, V, Charge>, Charge>;
    type Input = (Self, Self::Buffer, Self::Buffer);

    fn into_parts(input: Self::Input) -> Self::Input {
        input
    }
}

type FundedCell<V = usize> = LinCowCell<
    SuperBlock<usize, V, Funded>,
    CursorRead<usize, V, Funded>,
    CursorWrite<usize, V, Funded>,
>;

fn new_cell<V: Clone>() -> (FundedCell<V>, Funded) {
    let mut funding = Funded(prepaid());
    // SAFETY: this exact fresh root is immediately adopted by its linear cell.
    let root = unsafe { SuperBlock::new_with_funding(&mut funding) };
    (LinCowCell::new(root), funding)
}

fn input<V: Clone>(funding: Funded) -> <Funded as CursorMode<usize, V>>::Input {
    input_with_capacity(funding, 64, 64)
}

fn input_with_capacity<V: Clone>(
    mut funding: Funded,
    new_slots: usize,
    retired_slots: usize,
) -> <Funded as CursorMode<usize, V>>::Input {
    type Buffer<V> = FixedTrackingBuffer<*mut Node<usize, V, Charge>, Charge>;
    let layout = Buffer::<V>::allocation_layout(new_slots).unwrap();
    let first = Buffer::try_new(new_slots, funding.0.take_allocation_charge(layout))
        .unwrap_or_else(|_| panic!("valid original first-seen layout"));
    let layout = Buffer::<V>::allocation_layout(retired_slots).unwrap();
    let last = Buffer::try_new(retired_slots, funding.0.take_allocation_charge(layout))
        .unwrap_or_else(|_| panic!("valid original last-seen layout"));
    (funding, first, last)
}

fn assert_refunded_range(start: usize, end: usize) {
    for id in start..end {
        assert!(record(id).freed, "original allocation {id} still live");
        assert!(record(id).refunded, "original allocation {id} not refunded");
    }
}

#[test]
fn funded_cursor_keeps_exact_buffers_and_nodes_across_detach_retry_and_abort() {
    let (cell, funding) = new_cell();
    let old = cell.read();
    let mut writer = cell.write_with(|_| input(funding));
    let first = writer.first_seen.as_ptr();
    let last = writer.last_seen.as_ref().unwrap().as_ptr();
    for key in 0..32 {
        assert_eq!(writer.try_insert(key, key * 3).unwrap(), None);
    }
    assert!(writer.verify());
    let count = writer.funding.0.next;
    let detached = without_allocations(|| writer.detach());
    assert_eq!(detached.as_ref().first_seen.as_ptr(), first);
    assert_eq!(detached.as_ref().last_seen.as_ref().unwrap().as_ptr(), last);
    assert_eq!(detached.as_ref().search(&17), Some(&51));
    assert_eq!(old.search(&17), None);
    assert_eq!(
        detached
            .as_ref()
            .range(7..=11)
            .map(|(k, v)| (*k, *v))
            .collect::<Vec<_>>(),
        vec![(7, 21), (8, 24), (9, 27), (10, 30), (11, 33)]
    );
    let writer = without_allocations(|| match cell.try_write_owned(detached) {
        Ok(writer) => writer,
        Err(_) => panic!("original unchanged reader accepts its retained cursor"),
    });
    assert_eq!(writer.first_seen.as_ptr(), first);
    assert_eq!(writer.last_seen.as_ref().unwrap().as_ptr(), last);
    without_allocations(|| drop(writer));
    assert_refunded_range(1, count);
    assert!(
        !record(0).freed,
        "abort must retain the original current root"
    );
    assert!(cell.read().search(&17).is_none());
    drop(old);
    drop(cell);
    all_refunded(&Prepaid {
        next: count,
        remaining: 0,
    });
}

#[test]
fn committed_retirement_moves_original_fixed_buffer_until_old_reader_release() {
    let (cell, funding) = new_cell();
    let old = cell.read();
    let mut writer = cell.write_with(|_| input(funding));
    for key in 0..32 {
        writer.try_insert(key, key * 3).unwrap();
    }
    let original_retirement = writer.last_seen.as_ref().unwrap().as_ptr();
    let count = writer.funding.0.next;
    let detached = without_allocations(|| writer.detach());
    let writer = without_allocations(|| match cell.try_write_owned(detached) {
        Ok(writer) => writer,
        Err(_) => panic!("original cursor must reattach"),
    });
    without_allocations(|| writer.commit());
    assert_eq!(old.last_seen.get().unwrap().as_ptr(), original_retirement);
    assert!(
        record(1).refunded,
        "private first-seen buffer can be reclaimed"
    );
    assert!(
        !record(2).freed,
        "retirement backing remains with old reader"
    );
    assert!(
        !record(0).freed,
        "retired original node remains with old reader"
    );
    assert_eq!(old.search(&17), None);
    let current = cell.read();
    assert_eq!(current.search(&17), Some(&51));
    assert_eq!(current.first_key_value(), Some((&0, &0)));
    assert_eq!(current.last_key_value(), Some((&31, &93)));
    assert_eq!(current.kv_iter().count(), 32);
    assert_eq!(current.kv_iter().next_back(), Some((&31, &93)));
    without_allocations(|| drop(old));
    assert!(record(0).refunded);
    assert!(record(2).refunded);
    for id in 3..count {
        assert!(!record(id).freed, "published nodes outlive the old reader");
    }
    drop(current);
    without_allocations(|| drop(cell));
    all_refunded(&Prepaid {
        next: count,
        remaining: 0,
    });
}

#[test]
fn detached_funded_cursor_retains_original_root_after_cell_is_dropped() {
    let (cell, funding) = new_cell();
    let mut writer = cell.write_with(|_| input(funding));
    writer.try_insert(4, 8).unwrap();
    let count = writer.funding.0.next;
    let owned = writer.detach();
    without_allocations(|| drop(cell));
    assert!(!record(0).freed);
    assert_eq!(owned.as_ref().search(&4), Some(&8));
    without_allocations(|| drop(owned));
    all_refunded(&Prepaid {
        next: count,
        remaining: 0,
    });
}

thread_local! {
    static FAIL_CLONE: Cell<bool> = const { Cell::new(false) };
}

#[derive(Debug)]
struct PanicValue(usize);

impl Clone for PanicValue {
    fn clone(&self) -> Self {
        assert!(
            !FAIL_CLONE.with(Cell::get),
            "injected original value clone failure"
        );
        Self(self.0)
    }
}

#[test]
fn funded_cursor_clone_unwind_reclaims_original_new_nodes_and_buffers() {
    let (cell, funding) = new_cell::<PanicValue>();
    let mut writer = cell.write_with(|_| input(funding));
    writer.try_insert(4, PanicValue(8)).unwrap();
    let next = writer.funding.0.next;
    writer.commit();
    let before = cell.read();
    FAIL_CLONE.with(|fail| fail.set(true));
    let failed = catch_unwind(AssertUnwindSafe(|| {
        let mut writer = cell.write_with(|_| {
            input(Funded(Prepaid {
                next,
                remaining: 128 - next,
            }))
        });
        writer.try_insert(9, PanicValue(18)).unwrap();
    }));
    FAIL_CLONE.with(|fail| fail.set(false));
    assert!(failed.is_err());
    // Two original tracking buffers and the partially cloned leaf all reclaim.
    assert_refunded_range(next, next + 3);
    assert_eq!(before.search(&4).map(|v| v.0), Some(8));
    assert!(before.search(&9).is_none());
    assert!(cell.is_poisoned());
    drop(before);
    drop(cell);
    all_refunded(&Prepaid {
        next: next + 3,
        remaining: 0,
    });
}

#[test]
fn successive_commits_retain_each_original_retirement_until_its_reader_drops() {
    let (cell, funding) = new_cell();
    let first_reader = cell.read();
    let mut writer = cell.write_with(|_| input(funding));
    for key in 0..32 {
        writer.try_insert(key, key).unwrap();
    }
    let first_end = writer.funding.0.next;
    without_allocations(|| writer.commit());
    let second_reader = cell.read();
    let mut writer = cell.write_with(|_| {
        input(Funded(Prepaid {
            next: first_end,
            remaining: 128 - first_end,
        }))
    });
    assert_eq!(writer.try_insert(17, 999).unwrap(), Some(17));
    let last_pointer = writer.last_seen.as_ref().unwrap().as_ptr();
    let end = writer.funding.0.next;
    without_allocations(|| writer.commit());
    assert_eq!(
        second_reader.last_seen.get().unwrap().as_ptr(),
        last_pointer
    );
    assert!(!record(2).freed);
    assert!(!record(first_end + 1).freed);
    assert_eq!(second_reader.search(&17), Some(&17));
    assert_eq!(cell.read().search(&17), Some(&999));
    without_allocations(|| drop(first_reader));
    assert!(record(0).refunded);
    assert!(record(2).refunded);
    assert!(
        !record(first_end + 1).freed,
        "second reader retains its own retirement"
    );
    without_allocations(|| drop(second_reader));
    assert!(record(first_end + 1).refunded);
    without_allocations(|| drop(cell));
    all_refunded(&Prepaid {
        next: end,
        remaining: 0,
    });
}

#[test]
fn shared_cursor_thread_traits_require_charge_payload_and_provider_safety() {
    use std::marker::PhantomData;
    use std::rc::Rc;
    struct Typed<C, Provider>(PhantomData<(C, Provider)>);
    impl<C, P> NodeFunding for Typed<C, P> {
        type Charge = C;
        fn take_node_charge(&mut self, _: Layout) -> C {
            unreachable!("type-only assertion")
        }
    }
    impl<K, V, C, P> NodeCloning<K, V> for Typed<C, P> {
        fn clone_key(&mut self, _: &K) -> K {
            unreachable!("type-only assertion")
        }
        fn clone_value(&mut self, _: &V) -> V {
            unreachable!("type-only assertion")
        }
    }
    impl<K: Clone + Ord + Debug, V: Clone, C, P> CursorMode<K, V> for Typed<C, P> {
        type Buffer = FixedTrackingBuffer<*mut Node<K, V, C>, C>;
        type Input = (Self, Self::Buffer, Self::Buffer);
        fn into_parts(input: Self::Input) -> Self::Input {
            input
        }
    }
    fn send_sync<T: Send + Sync>() {}
    send_sync::<SuperBlock<usize, usize, Funded>>();
    send_sync::<CursorRead<usize, usize, Funded>>();
    send_sync::<CursorWrite<usize, usize, Funded>>();
    // A reader stores node/buffer charges, not the writer's provider.
    send_sync::<CursorRead<usize, usize, Typed<Untracked, Rc<()>>>>();
    trait AmbiguousIfSync<A> {
        fn probe() {}
    }
    impl<T: ?Sized> AmbiguousIfSync<()> for T {}
    impl<T: ?Sized + Sync> AmbiguousIfSync<u8> for T {}
    trait AmbiguousIfSend<A> {
        fn probe() {}
    }
    impl<T: ?Sized> AmbiguousIfSend<()> for T {}
    impl<T: ?Sized + Send> AmbiguousIfSend<u8> for T {}
    let _ = <SuperBlock<usize, usize, Typed<Rc<()>, ()>> as AmbiguousIfSend<_>>::probe;
    let _ = <CursorRead<usize, usize, Typed<Rc<()>, ()>> as AmbiguousIfSend<_>>::probe;
    let _ = <CursorWrite<usize, usize, Typed<Rc<()>, ()>> as AmbiguousIfSend<_>>::probe;
    let _ = <SuperBlock<usize, usize, Typed<Cell<usize>, ()>> as AmbiguousIfSync<_>>::probe;
    let _ = <CursorRead<usize, usize, Typed<Cell<usize>, ()>> as AmbiguousIfSync<_>>::probe;
    let _ = <CursorWrite<usize, usize, Typed<Cell<usize>, ()>> as AmbiguousIfSync<_>>::probe;
    let _ = <CursorWrite<usize, usize, Typed<Untracked, Rc<()>>> as AmbiguousIfSend<_>>::probe;
    let _ = <CursorWrite<usize, usize, Typed<Untracked, Cell<usize>>> as AmbiguousIfSync<_>>::probe;
    let _ = <CursorRead<Cell<usize>, usize> as AmbiguousIfSend<_>>::probe;
    let _ = <CursorRead<usize, Cell<usize>> as AmbiguousIfSend<_>>::probe;
}

#[test]
fn insufficient_tracking_returns_original_entry_before_any_node_allocation() {
    for (new_slots, retired_slots) in [(0, 1), (2, 1), (3, 0)] {
        let (cell, funding) = new_cell::<Box<usize>>();
        let mut writer =
            cell.write_with(|_| input_with_capacity(funding, new_slots, retired_slots));
        let root = writer.root;
        let count = writer.funding.0.next;
        let value = Box::new(42);
        let original = std::ptr::from_ref(value.as_ref());
        let (key, value) = without_allocations(|| writer.try_insert(9, value)).unwrap_err();
        assert_eq!(key, 9);
        assert_eq!(std::ptr::from_ref(value.as_ref()), original);
        assert_eq!(writer.root, root);
        assert_eq!(writer.funding.0.next, count);
        assert!(writer.first_seen.as_slice().is_empty());
        assert!(writer.last_seen.as_ref().unwrap().as_slice().is_empty());
        without_allocations(|| drop(writer));
        // A new fully admitted attempt receives that very same original entry.
        let mut writer = cell.write_with(|_| {
            input_with_capacity(
                Funded(Prepaid {
                    next: count,
                    remaining: 128 - count,
                }),
                3,
                1,
            )
        });
        assert!(writer.try_insert(key, value).unwrap().is_none());
        assert_eq!(
            std::ptr::from_ref(writer.search(&9).unwrap().as_ref()),
            original
        );
        let count = writer.funding.0.next;
        without_allocations(|| writer.commit());
        drop(cell);
        all_refunded(&Prepaid {
            next: count,
            remaining: 0,
        });
    }
}
