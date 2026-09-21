//! Checked cursor bookkeeping refusal and exact root-leaf allocation custody.

use super::*;
use crate::bptree::Prepaid;
use crate::internals::bptree::{
    allocation::NodeFunding,
    node::allocation_tests::{
        all_refunded, prepaid, record, without_allocations, Charge, Prepaid as Provider,
    },
    tracking::FixedTrackingBuffer,
};

type Mode = Prepaid<Provider>;
type Pointer = *mut Node<usize, usize, Charge>;

fn root_fixture(
    populated: bool,
    capacities: [usize; 2],
) -> (
    SuperBlock<usize, usize, Mode>,
    CursorWrite<usize, usize, Mode>,
) {
    let mut funding = Prepaid(Some(prepaid()));
    // The fixture owns this exact original charged root until after cursor abort.
    let mut base = unsafe { SuperBlock::new_with_funding(&mut funding) };
    if populated {
        let leaf = unsafe { &mut *base.root.cast::<Leaf<usize, usize, Charge>>() };
        assert!(matches!(
            leaf.insert_or_update(7, 70, &mut funding),
            LeafInsertState::Ok(None)
        ));
        base.size = 1;
    }
    Node::make_ro_raw(base.root);
    let buffers = capacities.map(|capacity| {
        let layout = FixedTrackingBuffer::<Pointer, Charge>::allocation_layout(capacity).unwrap();
        FixedTrackingBuffer::try_new(capacity, funding.take_node_charge(layout))
            .unwrap_or_else(|_| panic!("exact admitted fixture tracking layout"))
    });
    let [first, last] = buffers;
    let cursor = CursorWrite::with_input(&base, (funding, first, last));
    (base, cursor)
}

#[test]
fn remove_tracking_bound_is_checked_for_root_branch_and_overflow() {
    without_allocations(|| {
        assert_eq!(remove_tracking_slots(0), Some([1, 1]));
        assert_eq!(remove_tracking_slots(1), Some([3, 5]));
        assert_eq!(remove_tracking_slots(2), Some([5, 8]));
        let height = usize::BITS as usize - 1;
        assert_eq!(
            remove_tracking_slots(height),
            Some([2 * height + 1, 3 * height + 2])
        );
        assert_eq!(
            remove_tracking_slots(usize::MAX / 3),
            None,
            "retirement addition overflow"
        );
        assert_eq!(
            remove_tracking_slots(usize::MAX / 2 + 1),
            None,
            "clone multiplication overflow"
        );
        assert_eq!(remove_tracking_slots(usize::MAX), None);
    });
}

#[test]
fn remove_fixed_slot_refusal_precedes_any_node_copy_and_preserves_original_root() {
    for populated in [false, true] {
        for query in [7, 99] {
            for capacities in [[0, 1], [1, 0]] {
                let (base, mut cursor) = root_fixture(populated, capacities);
                let old = base.create_reader();
                let root = cursor.root;
                let length = cursor.length;
                let first = cursor.first_seen.as_ptr();
                let last = cursor.last_seen.as_ref().unwrap().as_ptr();
                let taken = cursor.funding.0.as_ref().unwrap().next;
                // No node allocation remains authorized: bookkeeping refusal
                // must occur before asking the finite provider for any clone.
                cursor.funding.0.as_mut().unwrap().remaining = 0;
                cursor.begin_admitted_edit();
                assert_eq!(without_allocations(|| cursor.try_remove(&query)), Err(()));
                assert!(cursor.edit_failed, "the original caller still owns cleanup");
                assert_eq!((cursor.root, cursor.length), (root, length));
                assert_eq!(cursor.first_seen.as_ptr(), first);
                assert_eq!(cursor.last_seen.as_ref().unwrap().as_ptr(), last);
                assert!(cursor.first_seen.as_slice().is_empty());
                assert!(cursor.last_seen.as_ref().unwrap().as_slice().is_empty());
                assert_eq!(old.search(&7), populated.then_some(&70));
                let provider = cursor.take_completed_admitted_funding();
                assert_eq!(provider.next, taken);
                assert_eq!(provider.remaining, 0);
                without_allocations(|| drop(cursor));
                without_allocations(|| drop(old));
                without_allocations(|| drop(base));
                all_refunded(&provider);
            }
        }
    }
}

#[test]
fn remove_exact_root_slots_preserve_absent_clone_and_original_reader_custody() {
    for populated in [false, true] {
        for query in [7, 99] {
            let (base, mut cursor) = root_fixture(populated, [1, 1]);
            let old = base.create_reader();
            let old_root = cursor.root;
            let taken = cursor.funding.0.as_ref().unwrap().next;
            cursor.funding.0.as_mut().unwrap().remaining = 1;
            cursor.begin_admitted_edit();
            let previous = cursor.try_remove(&query).unwrap();
            assert_eq!(previous, (populated && query == 7).then_some(70));
            assert!(
                cursor.edit_failed,
                "success never seals arbitrary caller cleanup"
            );
            assert_ne!(
                cursor.root, old_root,
                "canonical missing deletion still clones the old generation"
            );
            assert!(self_meta_shared!(cursor.root).is_leaf());
            assert_eq!(cursor.length, usize::from(populated && query != 7));
            assert_eq!(cursor.first_seen.as_slice(), &[cursor.root]);
            assert_eq!(cursor.last_seen.as_ref().unwrap().as_slice(), &[old_root]);
            assert_eq!(old.search(&7), populated.then_some(&70));
            assert_eq!(old.len(), usize::from(populated));
            let provider = cursor.take_completed_admitted_funding();
            assert_eq!(provider.next, taken + 1);
            assert_eq!(provider.remaining, 0);
            let clone = record(taken);
            assert_eq!(clone.pointer, cursor.root as usize);
            assert!(!clone.freed && !clone.refunded);
            without_allocations(|| drop(cursor));
            assert!(record(taken).freed && record(taken).refunded);
            assert_eq!(old.search(&7), populated.then_some(&70));
            assert!(!record(0).freed && !record(0).refunded);
            without_allocations(|| drop(old));
            without_allocations(|| drop(base));
            all_refunded(&provider);
        }
    }
}
