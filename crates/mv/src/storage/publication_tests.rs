//! Exact detached map installation, including refusal, replacement and retained readers.

use super::*;
use std::collections::BTreeMap;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};

#[test]
fn block_identity_binds_original_owner_predecessor_and_mode_without_reading_values() {
    let target: Storage<_, _> = [(1_u64, 10_u64)].into_iter().collect();
    let foreign: Storage<_, _> = [(1_u64, 10_u64)].into_iter().collect();
    let mut block = target.block();
    let identity = block.publication_identity();
    assert!(block.belongs_to(&target));
    assert!(!block.belongs_to(&foreign));
    assert_eq!(identity, block.publication_identity());
    assert_ne!(identity, foreign.block().publication_identity());
    block.insert(1, 99);
    assert!(block.belongs_to(&target));
    assert_eq!(identity, block.publication_identity());
    drop(block);
    assert_eq!(identity, target.block().publication_identity());
    assert_ne!(identity, target.block_and_revert().publication_identity());
    target.block().commit();
    assert_eq!(target.view().get(&1), Some(&10));
    assert_ne!(identity, target.block().publication_identity());
}

fn detach<K: Key, V: Value>(block: Block<'_, K, V>) -> Detached<K, V, ()> {
    block.try_detach(|_| Ok::<_, ()>(())).unwrap()
}

fn prepare<'a, K: Key, V: Value, A>(
    journal: Detached<K, V, A>,
    target: &'a Storage<K, V>,
) -> PreparedPublication<'a, K, V, A, ()> {
    match journal.try_prepare_publication(target, |_, _| Ok::<_, ()>(())) {
        Ok(prepared) => prepared,
        Err((_, error)) => panic!("unexpected preparation refusal: {error:?}"),
    }
}

fn values(storage: &Storage<u64, u64>) -> Vec<(u64, u64)> {
    storage.view().iter().map(|(k, v)| (*k, *v)).collect()
}

fn assert_same(actual: &Storage<u64, u64>, expected: &Storage<u64, u64>) {
    assert_eq!(values(actual), values(expected));
    assert_eq!(
        actual.snapshot().revert_map().iter().collect::<Vec<_>>(),
        expected.snapshot().revert_map().iter().collect::<Vec<_>>()
    );
}

#[test]
fn prepared_delta_matches_direct_commit_and_preserves_existing_readers() {
    let target: Storage<_, _> = [(1, 10), (2, 20)].into_iter().collect();
    let reference: Storage<_, _> = [(1, 10), (2, 20)].into_iter().collect();
    let reader = target.snapshot();
    let observer = detach(target.block());
    let mut block = target.block();
    let mut direct = reference.block();
    for candidate in [&mut block, &mut direct] {
        let mut child = candidate.transaction();
        child.insert(1, 11);
        child.remove(2);
        child.insert(3, 30);
        child.remove(99);
        child.apply();
    }
    direct.commit();
    let prepared = prepare(detach(block), &target);
    assert!(target.revert.try_write().is_none());
    assert!(target.blocks.try_write().is_none());
    assert_eq!(values(&target), [(1, 10), (2, 20)]);
    assert!(observer.matches_current(&target));
    prepared.publish();
    assert_same(&target, &reference);
    assert!(!observer.matches_current(&target));
    assert_eq!(reader.current().get(&1), Some(&10));
    assert_eq!(reader.current().get(&2), Some(&20));
    assert!(reader.revert_map().is_empty());
}

#[test]
fn replacement_restores_discarded_tip_only_keys_and_candidate_undo() {
    for touch in [false, true] {
        let target: Storage<_, _> = [(1, 10), (2, 20)].into_iter().collect();
        let reference: Storage<_, _> = [(1, 10), (2, 20)].into_iter().collect();
        for storage in [&target, &reference] {
            let mut tip = storage.block();
            tip.insert(1, 11);
            tip.remove(2);
            tip.insert(3, 30);
            tip.commit();
        }
        let reader = target.snapshot();
        let mut candidate = target.block_and_revert();
        let mut direct = reference.block_and_revert();
        if touch {
            for block in [&mut candidate, &mut direct] {
                block.insert(1, 12);
                block.remove(88);
            }
        }
        let prepared = prepare(detach(candidate), &target);
        assert_eq!(values(&target), [(1, 11), (3, 30)]);
        direct.commit();
        prepared.publish();
        assert_same(&target, &reference);
        assert_eq!(target.view().get(&2), Some(&20));
        assert_eq!(target.view().get(&3), None);
        assert_eq!(reader.current().get(&1), Some(&11));
        target.block_and_revert().commit();
        reference.block_and_revert().commit();
        assert_same(&target, &reference);
    }
}

#[test]
fn untouched_noop_and_absent_touches_publish_exact_undo_transitions() {
    for case in 0..3 {
        let target: Storage<_, _> = [(1, 10)].into_iter().collect();
        let mut tip = target.block();
        tip.insert(1, 11);
        tip.commit();
        let observer = detach(target.block());
        let mut candidate = target.block();
        match case {
            1 => {
                let _ = candidate.get_mut(&1);
            }
            2 => {
                candidate.remove(99);
            }
            _ => {}
        }
        let expected = match case {
            1 => BTreeMap::from([(1, Some(11))]),
            2 => BTreeMap::from([(99, None)]),
            _ => BTreeMap::new(),
        };
        prepare(detach(candidate), &target).publish();
        assert_eq!(values(&target), [(1, 11)]);
        assert_eq!(
            target
                .snapshot()
                .revert_map()
                .iter()
                .map(|(key, value)| (*key, *value))
                .collect::<BTreeMap<_, _>>(),
            expected
        );
        assert!(!observer.matches_current(&target));
    }
}

#[test]
fn busy_writers_return_same_journal_and_release_partial_acquisition() {
    let target: Storage<_, _> = [(1, 10)].into_iter().collect();
    let mut candidate = target.block();
    candidate.insert(1, 12);
    let mut journal = detach(candidate);
    let original = journal.blocks.get(&1).unwrap() as *const _;
    let original_undo = journal.revert.get(&1).unwrap().as_ref().unwrap() as *const _;
    for which in 0..2 {
        let undo = (which == 0).then(|| target.revert.write());
        let current = (which == 1).then(|| target.blocks.write());
        let (returned, error) = journal
            .try_prepare_publication(&target, |_, _| Ok::<_, ()>(()))
            .err()
            .unwrap();
        assert!(matches!(error, PublicationPreparationError::Busy(_)));
        assert_eq!(returned.blocks.get(&1).unwrap() as *const _, original);
        assert_eq!(
            returned.revert.get(&1).unwrap().as_ref().unwrap() as *const _,
            original_undo
        );
        drop(current);
        drop(undo);
        assert!(target.revert.try_write().is_some());
        assert!(target.blocks.try_write().is_some());
        assert_eq!(values(&target), [(1, 10)]);
        journal = returned;
    }
    prepare(journal, &target).publish();
    assert_eq!(values(&target), [(1, 12)]);
}

#[test]
fn foreign_aba_and_admission_race_cannot_publish_a_stale_journal() {
    let mut target: Storage<_, _> = [(1, 10)].into_iter().collect();
    let foreign: Storage<_, _> = [(1, 10)].into_iter().collect();
    let journal = detach(target.block());
    let (journal, error) = journal
        .try_prepare_publication(&foreign, |_, _| -> Result<(), ()> {
            panic!("foreign owner admitted")
        })
        .err()
        .unwrap();
    assert_eq!(error, PublicationPreparationError::Changed);
    target.insert(1, 11);
    target.insert(1, 10);
    let (_, error) = journal
        .try_prepare_publication(&target, |_, _| -> Result<(), ()> {
            panic!("stale owner admitted")
        })
        .err()
        .unwrap();
    assert_eq!(error, PublicationPreparationError::Changed);
    let journal = detach(target.block());
    let (_, error) = journal
        .try_prepare_publication(&target, |_, owner| {
            owner.block().commit();
            Ok::<_, ()>(())
        })
        .err()
        .unwrap();
    assert_eq!(error, PublicationPreparationError::Changed);
    assert_eq!(values(&target), [(1, 10)]);
    assert!(target.revert.try_write().is_some());
    assert!(target.blocks.try_write().is_some());
}

#[test]
fn abort_keeps_original_owner_available_after_another_component_refuses() {
    let first: Storage<_, _> = [(1, 10)].into_iter().collect();
    let second: Storage<_, _> = [(1, 20)].into_iter().collect();
    let mut block = first.block();
    block.insert(1, 11);
    let journal = detach(block);
    let ptr = journal.blocks.get(&1).unwrap() as *const _;
    let prepared = prepare(journal, &first);
    let mut block = second.block();
    block.insert(1, 21);
    let journal2 = detach(block);
    let (_, error) = journal2
        .try_prepare_publication(&second, |_, _| Err::<(), _>("capacity"))
        .err()
        .unwrap();
    assert_eq!(error, PublicationPreparationError::Admission("capacity"));
    let journal = prepared.abort();
    assert_eq!(journal.blocks.get(&1).unwrap() as *const _, ptr);
    assert_eq!(values(&first), [(1, 10)]);
    assert_eq!(values(&second), [(1, 20)]);
    assert!(journal.matches_current(&first));
    prepare(journal, &first).publish();
    assert_eq!(values(&first), [(1, 11)]);
}

#[test]
fn installation_retains_original_successors_and_both_reservations_survive_publication() {
    #[derive(Debug)]
    struct Counted(Arc<AtomicUsize>);
    impl Clone for Counted {
        fn clone(&self) -> Self {
            self.0.fetch_add(1, Ordering::SeqCst);
            Self(Arc::clone(&self.0))
        }
    }
    struct Reservation(Arc<AtomicBool>);
    impl Drop for Reservation {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
        }
    }
    let copies = Arc::new(AtomicUsize::new(0));
    let target: Storage<u64, _> = [(1, Counted(Arc::clone(&copies)))].into_iter().collect();
    let mut block = target.block();
    let _ = block.get_mut(&1);
    let captured = Arc::new(AtomicBool::new(false));
    let journal = block
        .try_detach(|_| Ok::<_, ()>(Reservation(Arc::clone(&captured))))
        .unwrap();
    copies.store(0, Ordering::SeqCst);
    let (journal, error) = journal
        .try_prepare_publication(&target, |_, _| Err::<(), _>("capacity"))
        .err()
        .unwrap();
    assert_eq!(error, PublicationPreparationError::Admission("capacity"));
    assert_eq!(copies.load(Ordering::SeqCst), 0);
    let installed = Arc::new(AtomicBool::new(false));
    let prepared = match journal.try_prepare_publication(&target, |_, _| {
        assert_eq!(copies.load(Ordering::SeqCst), 0);
        Ok::<_, ()>(Reservation(Arc::clone(&installed)))
    }) {
        Ok(p) => p,
        Err(_) => panic!("admission"),
    };
    assert_eq!(copies.load(Ordering::SeqCst), 0);
    let reservations = prepared.publish();
    assert!(!captured.load(Ordering::SeqCst));
    assert!(!installed.load(Ordering::SeqCst));
    assert!(target.revert.try_write().is_some());
    assert!(target.blocks.try_write().is_some());
    drop(reservations);
    assert!(captured.load(Ordering::SeqCst));
    assert!(installed.load(Ordering::SeqCst));
}

#[test]
fn original_map_and_undo_survive_both_busy_writers_abort_and_publication_without_clones() {
    #[derive(Debug)]
    struct NeverCloneAfterPreparation {
        value: u64,
        frozen: Arc<AtomicBool>,
    }
    impl Clone for NeverCloneAfterPreparation {
        fn clone(&self) -> Self {
            assert!(
                !self.frozen.load(Ordering::SeqCst),
                "successor was reconstructed"
            );
            Self {
                value: self.value,
                frozen: Arc::clone(&self.frozen),
            }
        }
    }
    let frozen = Arc::new(AtomicBool::new(false));
    let target = Storage::from_iter((0_u64..128).map(|key| {
        (
            key,
            NeverCloneAfterPreparation {
                value: key,
                frozen: Arc::clone(&frozen),
            },
        )
    }));
    let old = target.snapshot();
    let mut block = target.block();
    block.get_mut(&1).unwrap().value = 1001;
    block.remove(126);
    let current = block.get(&1).unwrap() as *const _;
    let undo = block.revert_map().get(&1).unwrap().as_ref().unwrap() as *const _;
    frozen.store(true, Ordering::SeqCst);
    let mut journal = detach(block);
    for which in 0..2 {
        let undo_writer = (which == 0).then(|| target.revert.write());
        let map_writer = (which == 1).then(|| target.blocks.write());
        let (returned, error) = journal
            .try_prepare_publication(&target, |_, _| Ok::<_, ()>(()))
            .err()
            .expect("original writer is held");
        assert!(matches!(error, PublicationPreparationError::Busy(_)));
        assert_eq!(returned.blocks.get(&1).unwrap() as *const _, current);
        assert_eq!(
            returned.revert.get(&1).unwrap().as_ref().unwrap() as *const _,
            undo
        );
        drop(map_writer);
        drop(undo_writer);
        journal = returned;
    }
    let journal = prepare(journal, &target).abort();
    assert_eq!(journal.blocks.get(&1).unwrap() as *const _, current);
    assert_eq!(
        journal.revert.get(&1).unwrap().as_ref().unwrap() as *const _,
        undo
    );
    prepare(journal, &target).publish();
    let published = target.snapshot();
    assert_eq!(published.current().get(&1).unwrap() as *const _, current);
    assert_eq!(
        published.revert_map().get(&1).unwrap().as_ref().unwrap() as *const _,
        undo
    );
    assert!(published.current().get(&126).is_none());
    assert_eq!(old.current().get(&1).unwrap().value, 1);
    assert_eq!(old.current().get(&126).unwrap().value, 126);
}

#[test]
fn changed_raw_map_generation_refuses_original_owner_before_any_installation() {
    use std::{
        future::Future,
        task::{Context, Waker},
    };
    let target: Storage<_, _> = [(1, 10), (2, 20)].into_iter().collect();
    let mut candidate = target.block();
    candidate.insert(1, 11);
    let journal = detach(candidate);
    let original = journal.blocks.get(&1).unwrap() as *const _;
    let mut released = std::pin::pin!(target.blocks_released.observe().wait_for_release());
    assert!(
        released
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
            .is_pending()
    );
    let (journal, error) = journal
        .try_prepare_publication(&target, |_, target| {
            // Deliberately bypass only the outer MV identity in this structural test.
            // The tree itself must reject a cursor whose shared-node base changed.
            let mut writer = target.blocks.write();
            writer.insert(2, 22);
            writer.commit();
            assert!(
                released
                    .as_mut()
                    .poll(&mut Context::from_waker(Waker::noop()))
                    .is_pending()
            );
            Ok::<_, ()>(())
        })
        .err()
        .expect("original tree generation changed");
    assert_eq!(error, PublicationPreparationError::Changed);
    assert!(
        released
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
            .is_ready()
    );
    assert_eq!(journal.blocks.get(&1).unwrap() as *const _, original);
    assert_eq!(journal.blocks.get(&2), Some(&20));
    assert_eq!(values(&target), [(1, 10), (2, 22)]);
    assert!(target.snapshot().revert_map().is_empty());
    assert!(target.revert.try_write().is_some());
    assert!(target.blocks.try_write().is_some());
}

#[test]
fn changed_raw_undo_generation_refuses_original_pair_even_after_value_aba() {
    use std::{
        future::Future,
        task::{Context, Waker},
    };
    for restore_same_values in [false, true] {
        let target: Storage<u64, u64> = [(1, 10), (2, 20)].into_iter().collect();
        let mut candidate = target.block();
        candidate.insert(1, 11);
        let journal = detach(candidate);
        let current = journal.blocks.get(&1).unwrap() as *const _;
        let undo = journal.revert.get(&1).unwrap().as_ref().unwrap() as *const _;
        let mut released = std::pin::pin!(target.revert_released.observe().wait_for_release());
        assert!(
            released
                .as_mut()
                .poll(&mut Context::from_waker(Waker::noop()))
                .is_pending()
        );
        let (journal, error) = journal
            .try_prepare_publication(&target, |original, target| {
                // Bypass only the outer pair identity: the original native undo
                // root/base must independently reject this changed generation.
                let mut writer = target.revert.write();
                writer.insert(7, Some(70));
                if restore_same_values {
                    writer.remove(&7);
                }
                writer.commit();
                assert!(
                    released
                        .as_mut()
                        .poll(&mut Context::from_waker(Waker::noop()))
                        .is_pending()
                );
                assert!(original.matches_current(target));
                Ok::<_, ()>(())
            })
            .err()
            .expect("original undo generation changed");
        assert_eq!(error, PublicationPreparationError::Changed);
        assert!(
            released
                .as_mut()
                .poll(&mut Context::from_waker(Waker::noop()))
                .is_ready()
        );
        assert_eq!(journal.blocks.get(&1).unwrap() as *const _, current);
        assert_eq!(
            journal.revert.get(&1).unwrap().as_ref().unwrap() as *const _,
            undo
        );
        assert_eq!(journal.blocks.get(&1), Some(&11));
        assert_eq!(journal.revert.get(&1), Some(&Some(10)));
        assert_eq!(values(&target), [(1, 10), (2, 20)]);
        assert_eq!(
            target.snapshot().revert_map().get(&7),
            (!restore_same_values).then_some(&Some(70))
        );
        assert!(target.revert.try_write().is_some());
        assert!(target.blocks.try_write().is_some());
    }
}

#[test]
fn pair_release_wake_observes_both_roots_and_rotated_identity_without_held_writers() {
    use std::{
        future::Future,
        pin::Pin,
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
        task::{Context, Wake, Waker},
    };
    struct Probe {
        storage: Arc<Storage<u64, u64>>,
        predecessor: CapturedPublication,
        expected: u64,
        expected_undo: Option<Option<u64>>,
        wakes: AtomicUsize,
    }
    impl Wake for Probe {
        fn wake(self: Arc<Self>) {
            assert_eq!(
                self.predecessor
                    .try_check_current::<()>(&self.storage.publication),
                Err(PublicationPreparationError::Changed)
            );
            let current = self
                .storage
                .blocks
                .try_write()
                .expect("current physical writer released before notification");
            let undo = self
                .storage
                .revert
                .try_write()
                .expect("undo physical writer released before notification");
            assert_eq!(current.get(&1), Some(&self.expected));
            assert_eq!(undo.get(&1), self.expected_undo.as_ref());
            self.wakes.fetch_add(1, Ordering::SeqCst);
        }
    }
    for prepared in [false, true] {
        for dirty in [false, true] {
            let storage = Arc::new(Storage::from_iter([(1, 10)]));
            let predecessor = storage.publication.capture();
            let mut block = storage.block();
            if dirty {
                block.insert(1, 20);
            }
            let mut wait = storage.blocks_released.observe().wait_for_release();
            let probe = Arc::new(Probe {
                storage: Arc::clone(&storage),
                predecessor,
                expected: if dirty { 20 } else { 10 },
                expected_undo: dirty.then_some(Some(10)),
                wakes: AtomicUsize::new(0),
            });
            let waker = Waker::from(Arc::clone(&probe));
            assert!(
                Pin::new(&mut wait)
                    .poll(&mut Context::from_waker(&waker))
                    .is_pending()
            );
            if prepared {
                // Subscribe to the acquired original publication writer, after
                // the intentional earlier detach release has already occurred.
                drop(wait);
                let detached = block.try_detach(|_| Ok::<_, ()>(())).unwrap();
                let publish = detached
                    .try_prepare_publication(&storage, |_, _| Ok::<_, ()>(()))
                    .unwrap_or_else(|_| panic!("same original generation"));
                wait = storage.blocks_released.observe().wait_for_release();
                assert!(
                    Pin::new(&mut wait)
                        .poll(&mut Context::from_waker(&waker))
                        .is_pending()
                );
                publish.publish();
            } else {
                block.commit();
            }
            assert_eq!(probe.wakes.load(Ordering::SeqCst), 1);
            assert!(
                Pin::new(&mut wait)
                    .poll(&mut Context::from_waker(&waker))
                    .is_ready()
            );
        }
    }
}

#[test]
fn direct_insert_retirement_panic_preserves_published_identity_and_healthy_contention() {
    use std::{
        future::Future,
        panic::{AssertUnwindSafe, catch_unwind},
        task::{Context, Wake, Waker},
    };

    struct Control {
        next: AtomicUsize,
        panic_on: AtomicUsize,
        panics: AtomicUsize,
    }
    struct Payload {
        number: u64,
        instance: usize,
        control: Arc<Control>,
    }
    impl Payload {
        fn new(number: u64, control: &Arc<Control>) -> Self {
            Self {
                number,
                instance: control.next.fetch_add(1, Ordering::SeqCst),
                control: Arc::clone(control),
            }
        }
    }
    impl Clone for Payload {
        fn clone(&self) -> Self {
            Self::new(self.number, &self.control)
        }
    }
    impl Drop for Payload {
        fn drop(&mut self) {
            if self
                .control
                .panic_on
                .compare_exchange(
                    self.instance,
                    usize::MAX,
                    Ordering::SeqCst,
                    Ordering::SeqCst,
                )
                .is_ok()
            {
                self.control.panics.fetch_add(1, Ordering::SeqCst);
                panic!("original published payload retirement");
            }
        }
    }
    struct Count(AtomicUsize);
    impl Wake for Count {
        fn wake(self: Arc<Self>) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    for panic_on_retirement in [false, true] {
        let control = Arc::new(Control {
            next: AtomicUsize::new(0),
            panic_on: AtomicUsize::new(usize::MAX),
            panics: AtomicUsize::new(0),
        });
        let mut target = Storage::new();
        assert!(target.insert(1_u64, Payload::new(10, &control)).is_none());
        let mut tip = target.block();
        tip.insert(1, Payload::new(20, &control));
        tip.commit();
        let predecessor = target.publication.capture();
        let retired = target.view().get(&1).unwrap().instance;
        let (undo_pointer, undo_instance) = {
            let undo = target.revert.read();
            let value = undo.get(&1).unwrap().as_ref().unwrap();
            (value as *const Payload, value.instance)
        };
        let observation = target.blocks_released.observe();
        let mut released = std::pin::pin!(observation.clone().wait_for_release());
        let count = Arc::new(Count(AtomicUsize::new(0)));
        let waker = Waker::from(Arc::clone(&count));
        assert!(
            released
                .as_mut()
                .poll(&mut Context::from_waker(&waker))
                .is_pending()
        );
        if panic_on_retirement {
            control.panic_on.store(retired, Ordering::SeqCst);
        }
        let result = catch_unwind(AssertUnwindSafe(|| {
            target.insert(1, Payload::new(30, &control))
        }));
        if panic_on_retirement {
            let panic = match result {
                Err(panic) => panic,
                Ok(_) => panic!("original retired value must unwind"),
            };
            assert_eq!(
                panic.downcast_ref::<&str>(),
                Some(&"original published payload retirement")
            );
            assert_eq!(control.panics.load(Ordering::SeqCst), 1);
        } else {
            let previous = result.unwrap_or_else(|_| panic!("normal insertion unwound"));
            assert_eq!(previous.unwrap().number, 20);
            assert_eq!(control.panics.load(Ordering::SeqCst), 0);
        }
        assert_eq!(target.view().get(&1).unwrap().number, 30);
        assert_eq!(
            predecessor.try_check_current::<()>(&target.publication),
            Err(PublicationPreparationError::Changed)
        );
        assert!(!target.blocks.is_poisoned());
        assert!(!observation.is_poisoned());
        assert_eq!(count.0.load(Ordering::SeqCst), 1);
        assert!(
            released
                .as_mut()
                .poll(&mut Context::from_waker(&waker))
                .is_ready()
        );
        {
            let undo = target.revert.read();
            let value = undo.get(&1).unwrap().as_ref().unwrap();
            assert_eq!(value.number, 10);
            assert_eq!(value.instance, undo_instance);
            assert_eq!(value as *const Payload, undo_pointer);
        }

        // A later real waiter must observe contention on the healthy original
        // writer, not permanent poison from the earlier retirement destructor.
        let journal = detach(target.block());
        let held = target
            .blocks_released
            .poisoning_guard(target.blocks.write());
        let expected = target.blocks_released.observe();
        let (journal, error) = journal
            .try_prepare_publication(&target, |_, _| Ok::<_, ()>(()))
            .err()
            .expect("original current writer is held");
        assert_eq!(error, PublicationPreparationError::Busy(expected.clone()));
        let mut retry_wait = std::pin::pin!(expected.wait_for_release());
        assert!(
            retry_wait
                .as_mut()
                .poll(&mut Context::from_waker(&waker))
                .is_pending()
        );
        drop(held);
        assert_eq!(count.0.load(Ordering::SeqCst), 2);
        assert!(
            retry_wait
                .as_mut()
                .poll(&mut Context::from_waker(&waker))
                .is_ready()
        );
        drop(prepare(journal, &target).abort());
        assert_eq!(target.view().get(&1).unwrap().number, 30);
        let undo = target.revert.read();
        let value = undo.get(&1).unwrap().as_ref().unwrap();
        assert_eq!(value.instance, undo_instance);
        assert_eq!(value as *const Payload, undo_pointer);
    }
}
