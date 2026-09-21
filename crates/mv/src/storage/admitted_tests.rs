//! Joint publication keeps both original maps and identity ahead of cleanup.

use super::*;
use crate::allocation::{AllocationBudget, AllocationCharge, AllocationReservation};
use concread::bptree::{
    AllocationDemand, ClonePlanning, NodeCloning, NodeFunding, PlanningError, Prepaid,
};
use std::{
    alloc::Layout,
    future::Future,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering::SeqCst},
    },
    task::{Context, Poll, Wake, Waker},
};

struct Records {
    live: Mutex<Vec<(usize, bool, usize)>>,
    panic_on: AtomicUsize,
}
struct Charge {
    credit: AllocationCharge,
    id: usize,
    records: Arc<Records>,
}
impl Drop for Charge {
    fn drop(&mut self) {
        self.records.live.lock().unwrap()[self.id].1 = false;
        assert!(
            self.records
                .panic_on
                .compare_exchange(self.id, usize::MAX, SeqCst, SeqCst)
                .is_err(),
            "injected first current-map retirement panic"
        );
        // Original credit stays owned until the containing charge drops.
        let _ = self.credit.layout();
    }
}
struct Policy {
    reservation: AllocationReservation,
    records: Arc<Records>,
    component: usize,
}
impl NodeFunding for Policy {
    type Charge = Charge;
    fn take_node_charge(&mut self, layout: Layout) -> Charge {
        let mut records = self.records.live.lock().unwrap();
        let id = records.len();
        records.push((layout.size(), true, self.component));
        Charge {
            credit: self.reservation.try_split(layout).unwrap(),
            id,
            records: Arc::clone(&self.records),
        }
    }
}
impl NodeCloning<u64, u64> for Policy {
    fn clone_key(&mut self, key: &u64) -> u64 {
        *key
    }
    fn clone_value(&mut self, value: &u64) -> u64 {
        *value
    }
}
impl ClonePlanning<u64, u64> for Policy {
    fn plan_key(_: &u64, _: &mut AllocationDemand) -> Result<(), PlanningError> {
        Ok(())
    }
    fn plan_value(_: &u64, _: &mut AllocationDemand) -> Result<(), PlanningError> {
        Ok(())
    }
}

struct WakeCount(AtomicUsize);
impl Wake for WakeCount {
    fn wake(self: Arc<Self>) {
        self.0.fetch_add(1, SeqCst);
    }
}

// The retirement panic happened after publication released physical locks.
// Later real contention must retain its own live release observation, not a
// sticky poison hint inherited from destruction of the old retirement owner.
fn assert_healthy_contention<T>(
    storage: &Storage<u64, u64, Prepaid<Policy>>,
    budget: &AllocationBudget,
    records: &Arc<Records>,
    notification: &ReleaseNotification,
    held: T,
) {
    let expected = notification.observe();
    let wait = match storage.try_block_admitted(budget, |_| {
        panic!("a held original writer must refuse before admission")
    }) {
        Err(admitted::StorageAdmissionError::Busy(wait)) => wait,
        Err(error) => panic!("healthy held writer lost its release wait: {error:?}"),
        Ok(_) => panic!("held original writer admitted another block"),
    };
    assert_eq!(wait, expected);
    assert!(!wait.is_poisoned());
    let count = Arc::new(WakeCount(AtomicUsize::new(0)));
    let waker = Waker::from(Arc::clone(&count));
    let mut future = std::pin::pin!(wait.wait_for_release());
    assert_eq!(
        future.as_mut().poll(&mut Context::from_waker(&waker)),
        Poll::Pending
    );
    drop(held);
    assert_eq!(count.0.load(SeqCst), 1);
    assert_eq!(
        future.as_mut().poll(&mut Context::from_waker(&waker)),
        Poll::Ready(())
    );
    let retry = storage
        .try_block_admitted(budget, |reservation| Policy {
            reservation,
            records: Arc::clone(records),
            component: 0,
        })
        .unwrap();
    assert_eq!(retry.get(&7), Some(&71));
    drop(retry);
}

#[test]
fn direct_and_prepared_publication_install_whole_pair_before_charge_cleanup_panics() {
    for prepared in [false, true] {
        let budget = AllocationBudget::new(1 << 20);
        let records = Arc::new(Records {
            live: Mutex::new(Vec::new()),
            panic_on: AtomicUsize::new(usize::MAX),
        });
        let provider = |reservation| Policy {
            reservation,
            records: Arc::clone(&records),
            component: 0,
        };
        let storage =
            Storage::<u64, u64, Prepaid<Policy>>::try_new_with_node_custody(&budget, provider)
                .unwrap();
        budget.with_deferred_refund_notifications(|| {
            let mut block = storage.try_block_admitted(&budget, provider).unwrap();
            let mut tx = block.try_transaction().unwrap();
            tx.try_insert_admitted(7, 70, &budget, provider).unwrap();
            tx.apply();
            block.commit();
        });
        let old_current = storage.blocks.read();
        let old_undo = storage.revert.read();
        let predecessor = storage.publication.capture();
        budget.with_deferred_refund_notifications(|| {
            let mut block = storage.try_block_admitted(&budget, provider).unwrap();
            let first = records.live.lock().unwrap().len();
            let mut tx = block.try_transaction().unwrap();
            let mut component = 0;
            tx.try_insert_admitted(7, 71, &budget, |reservation| {
                component += 1;
                Policy {
                    reservation,
                    records: Arc::clone(&records),
                    component,
                }
            })
            .unwrap();
            tx.apply();
            // The third original reservation is the current-map edit. Its
            // first-seen buffer precedes retirement tracking; both use concrete
            // pointer-array layouts and the first is retired at commit.
            let candidates: Vec<_> = records
                .live
                .lock()
                .unwrap()
                .iter()
                .enumerate()
                .skip(first)
                .filter(|(_, (size, live, component))| {
                    *component == 3 && *size > 0 && *size <= 32 && *live
                })
                .map(|(id, _)| id)
                .collect();
            assert_eq!(candidates.len(), 2);
            let target = candidates[0];
            let result = if prepared {
                let detached = block.try_detach(|_| Ok::<_, ()>(())).unwrap();
                let publisher = detached
                    .try_prepare_publication(&storage, |_, _| Ok::<_, ()>(()))
                    .unwrap_or_else(|_| panic!("same original pair"));
                records.panic_on.store(target, SeqCst);
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    publisher.publish();
                }))
            } else {
                records.panic_on.store(target, SeqCst);
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| block.commit()))
            };
            assert!(result.is_err());
            assert!(!records.live.lock().unwrap()[target].1);
            assert_eq!(storage.blocks.read().get(&7), Some(&71));
            assert_eq!(storage.revert.read().get(&7), Some(&Some(70)));
            assert_eq!(
                predecessor.try_check_current::<()>(&storage.publication),
                Err(PublicationPreparationError::Changed)
            );
            assert_eq!(old_current.get(&7), Some(&70));
            assert_eq!(old_undo.get(&7), Some(&None));
            let current = storage
                .blocks
                .try_write_admitted(|demand| {
                    Ok::<_, ()>(provider(budget.try_reserve_bytes(demand.bytes()).unwrap()))
                })
                .unwrap();
            assert_healthy_contention(
                &storage,
                &budget,
                &records,
                &storage.blocks_released,
                storage.blocks_released.poisoning_guard(current),
            );
            let undo = storage.try_block_admitted(&budget, provider).unwrap();
            assert_healthy_contention(&storage, &budget, &records, &storage.revert_released, undo);
        });
        drop(old_current);
        drop(old_undo);
        drop(storage);
        assert!(
            records
                .live
                .lock()
                .unwrap()
                .iter()
                .all(|(_, live, _)| !live)
        );
        assert_eq!(budget.reserved_bytes(), 0);
    }
}

#[test]
fn refused_admitted_acquisition_signals_only_the_original_released_writer() {
    use admitted::StorageAdmissionError;
    use concread::bptree::MapAdmissionError;

    let budget = AllocationBudget::new(1 << 20);
    let records = Arc::new(Records {
        live: Mutex::new(Vec::new()),
        panic_on: AtomicUsize::new(usize::MAX),
    });
    let provider = |reservation| Policy {
        reservation,
        records: Arc::clone(&records),
        component: 0,
    };
    let storage =
        Storage::<u64, u64, Prepaid<Policy>>::try_new_with_node_custody(&budget, provider).unwrap();
    budget.with_deferred_refund_notifications(|| {
        let mut block = storage.try_block_admitted(&budget, provider).unwrap();
        let mut transaction = block.try_transaction().unwrap();
        transaction
            .try_insert_admitted(7, 70, &budget, provider)
            .unwrap();
        transaction.apply();
        block.commit();

        let notification = &storage.blocks_released;
        let inner_map = BptreeMap::<u64, u64>::new();
        let inner_notification = ReleaseNotification::default();
        let inner_held = inner_notification.poisoning_guard(inner_map.write());
        let inner_wait = inner_notification.observe();
        let exhausted = AllocationBudget::new(0);
        for clear in [false, true] {
            for refuse_with in 0..3 {
                let expected = notification.observe();
                let count = Arc::new(WakeCount(AtomicUsize::new(0)));
                let waker = Waker::from(Arc::clone(&count));
                let mut registered = None;
                let mut refuse = |demand: AllocationDemand| {
                    // Register during actual raw ownership, before the refusal
                    // releases it. A competing writer cannot enter admission.
                    assert!(matches!(
                        storage
                            .blocks
                            .try_write_admitted::<StorageAdmissionError>(|_| panic!(
                                "original raw writer is held"
                            )),
                        Err(MapAdmissionError::Busy)
                    ));
                    let wait = notification.observe();
                    assert_eq!(wait, expected);
                    let mut future = wait.wait_for_release();
                    assert_eq!(
                        std::pin::Pin::new(&mut future).poll(&mut Context::from_waker(&waker)),
                        Poll::Pending
                    );
                    registered = Some(future);
                    Err::<Policy, _>(match refuse_with {
                        0 => StorageAdmissionError::Changed,
                        1 => {
                            assert!(inner_map.try_write().is_none());
                            StorageAdmissionError::Busy(inner_notification.observe())
                        }
                        2 => StorageAdmissionError::Capacity(
                            exhausted
                                .try_reserve_bytes(demand.bytes())
                                .expect_err("actual complete demand exceeds empty budget"),
                        ),
                        _ => unreachable!(),
                    })
                };
                let refused = admitted::acquire_writer(notification, || {
                    if clear {
                        storage.blocks.try_clear_admitted(&mut refuse)
                    } else {
                        storage.blocks.try_write_admitted(&mut refuse)
                    }
                });
                match (refuse_with, refused) {
                    (0, Err(MapAdmissionError::Refused(StorageAdmissionError::Changed)))
                    | (2, Err(MapAdmissionError::Refused(StorageAdmissionError::Capacity(_)))) => {}
                    (1, Err(MapAdmissionError::Refused(StorageAdmissionError::Busy(wait)))) => {
                        assert_eq!(wait, inner_wait);
                        assert_eq!(inner_notification.observe(), inner_wait);
                    }
                    _ => panic!("original callback refusal changed"),
                }
                assert_eq!(count.0.load(SeqCst), 1);
                assert!(!expected.is_poisoned());
                assert_eq!(
                    std::pin::Pin::new(registered.as_mut().expect("registered under raw writer"))
                        .poll(&mut Context::from_waker(&waker)),
                    Poll::Ready(())
                );
                assert_eq!(storage.blocks.read().get(&7), Some(&70));
            }
        }
        drop(inner_held);

        let expected = notification.observe();
        let count = Arc::new(WakeCount(AtomicUsize::new(0)));
        let waker = Waker::from(Arc::clone(&count));
        let mut future = expected.clone().wait_for_release();
        assert_eq!(
            std::pin::Pin::new(&mut future).poll(&mut Context::from_waker(&waker)),
            Poll::Pending
        );
        let held = admitted::acquire_writer(notification, || {
            storage.blocks.try_write_admitted(|demand| {
                Ok::<_, StorageAdmissionError>(provider(
                    budget.try_reserve_bytes(demand.bytes()).unwrap(),
                ))
            })
        })
        .unwrap();
        assert_eq!(count.0.load(SeqCst), 0);
        let busy = admitted::acquire_writer(notification, || {
            storage
                .blocks
                .try_write_admitted::<StorageAdmissionError>(|_| {
                    panic!("Busy must refuse before admission")
                })
        });
        assert!(matches!(busy, Err(MapAdmissionError::Busy)));
        assert_eq!(notification.observe(), expected);
        assert_eq!(count.0.load(SeqCst), 0);
        assert_eq!(
            std::pin::Pin::new(&mut future).poll(&mut Context::from_waker(&waker)),
            Poll::Pending
        );
        drop(held);
        assert_eq!(count.0.load(SeqCst), 1);
        assert!(!expected.is_poisoned());
        assert_eq!(
            std::pin::Pin::new(&mut future).poll(&mut Context::from_waker(&waker)),
            Poll::Ready(())
        );
        assert_eq!(storage.blocks.read().get(&7), Some(&70));
    });
    drop(storage);
    assert!(
        records
            .live
            .lock()
            .unwrap()
            .iter()
            .all(|(_, live, _)| !live)
    );
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn prepaid_removal_matches_transaction_and_block_preimages_across_abort_and_rebalance() {
    use std::collections::{BTreeMap, BTreeSet};

    fn entries(store: &impl StorageReadOnly<u64, u64>) -> Vec<(u64, u64)> {
        store.iter().map(|(key, value)| (*key, *value)).collect()
    }
    fn touches<'a>(
        keys: impl Iterator<Item = &'a u64>,
        before: &BTreeMap<u64, u64>,
        after: &BTreeMap<u64, u64>,
    ) -> Vec<(u64, Option<u64>, Option<u64>)> {
        keys.map(|key| (*key, before.get(key).copied(), after.get(key).copied()))
            .collect()
    }
    fn row(entry: TouchedEntry<'_, u64, u64>) -> (u64, Option<u64>, Option<u64>) {
        (*entry.key, entry.before.copied(), entry.after.copied())
    }

    let budget = AllocationBudget::new(16 << 20);
    let records = Arc::new(Records {
        live: Mutex::new(Vec::new()),
        panic_on: AtomicUsize::new(usize::MAX),
    });
    let provider = |reservation| Policy {
        reservation,
        records: Arc::clone(&records),
        component: 0,
    };
    let admitted =
        Storage::<u64, u64, Prepaid<Policy>>::try_new_with_node_custody(&budget, provider).unwrap();
    let ordinary = Storage::<u64, u64>::default();
    let mut model = BTreeMap::<u64, u64>::new();
    for block_number in 0_u64..3 {
        let block_before = model.clone();
        let old = admitted.view();
        budget.with_deferred_refund_notifications(|| {
            let mut plain = ordinary.block();
            let mut prepaid = admitted.try_block_admitted(&budget, provider).unwrap();
            let mut block_touched = BTreeSet::new();
            let mut block_dirty = false;
            for phase in 0_u64..6 {
                let transaction_before = model.clone();
                let mut next = model.clone();
                let mut touched = BTreeSet::new();
                let mut dirty = false;
                let mut left = plain.try_transaction().unwrap();
                let mut right = prepaid.try_transaction().unwrap();
                let count = if phase == 0 { 4 } else { 256 };
                for step in 0..count {
                    // First keep explicit missing-key touches; then fill, empty,
                    // refill, and mix edits to cross splits and root demotion.
                    let key = match phase {
                        0 => 2048 + step % 2,
                        1..=3 => 255 - step,
                        _ => (step * 73) % 384,
                    };
                    let insert = matches!(phase, 1 | 3) || (phase >= 4 && step % 3 == 0);
                    let expected = if insert {
                        dirty = true;
                        next.insert(key, block_number * 10_000 + phase * 1000 + step)
                    } else {
                        let previous = next.remove(&key);
                        dirty |= previous.is_some();
                        previous
                    };
                    let (actual_left, actual_right) = if insert {
                        let value = block_number * 10_000 + phase * 1000 + step;
                        (
                            left.insert(key, value),
                            right
                                .try_insert_admitted(key, value, &budget, provider)
                                .unwrap(),
                        )
                    } else {
                        (
                            left.remove(key),
                            right.try_remove_admitted(&key, &budget, provider).unwrap(),
                        )
                    };
                    assert_eq!((actual_left, actual_right), (expected, expected));
                    touched.insert(key);
                    assert_eq!(
                        right.get_before_transaction(&key),
                        transaction_before.get(&key)
                    );
                    assert_eq!(right.get_before_block(&key), block_before.get(&key));
                }
                let expected_rows = touches(touched.iter(), &transaction_before, &next);
                assert_eq!(
                    left.touched_entries().map(row).collect::<Vec<_>>(),
                    expected_rows
                );
                assert_eq!(
                    right.touched_entries().map(row).collect::<Vec<_>>(),
                    expected_rows
                );
                assert_eq!(
                    entries(&left),
                    next.iter().map(|(k, v)| (*k, *v)).collect::<Vec<_>>()
                );
                assert_eq!(entries(&right), entries(&left));
                // An aborted sibling must restore both roots, first preimages,
                // dirty state and the parent's earlier successful touches.
                if phase == 4 || (phase == 2 && block_number == 1) {
                    drop(left);
                    drop(right);
                } else {
                    left.apply();
                    right.apply();
                    model = next;
                    block_touched.extend(touched);
                    block_dirty |= dirty;
                }
                let expected_rows = touches(block_touched.iter(), &block_before, &model);
                assert_eq!(
                    plain.touched_entries().map(row).collect::<Vec<_>>(),
                    expected_rows
                );
                assert_eq!(
                    prepaid.touched_entries().map(row).collect::<Vec<_>>(),
                    expected_rows
                );
                assert_eq!(plain.is_dirty(), block_dirty);
                assert_eq!(prepaid.is_dirty(), block_dirty);
                assert_eq!(entries(&prepaid), entries(&plain));
            }
            plain.commit();
            prepaid.commit();
        });
        assert_eq!(entries(&old), block_before.into_iter().collect::<Vec<_>>());
        assert_eq!(
            entries(&admitted.view()),
            model.iter().map(|(k, v)| (*k, *v)).collect::<Vec<_>>()
        );
        assert_eq!(entries(&admitted.view()), entries(&ordinary.view()));
    }
    drop(admitted);
    assert!(
        records
            .live
            .lock()
            .unwrap()
            .iter()
            .all(|(_, live, _)| !live)
    );
    assert_eq!(budget.reserved_bytes(), 0);
}
