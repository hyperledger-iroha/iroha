// Included by queue::tests to reuse the actual durable admission fixtures.
mod lane_retirement_observer {
    //! Release-driven retirement observations use the original Queue mutex.

    use super::*;
    use std::{
        future::Future,
        pin::Pin,
        task::{Context, Poll, Wake, Waker},
    };

    #[derive(Default)]
    struct WakeCount(AtomicUsize);

    impl Wake for WakeCount {
        fn wake(self: Arc<Self>) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    fn poll(wait: &mut concread::release::ReleaseFuture, count: &Arc<WakeCount>) -> Poll<()> {
        let waker = Waker::from(Arc::clone(count));
        Pin::new(wait).poll(&mut Context::from_waker(&waker))
    }

    fn waiting(queue: &Queue) -> concread::release::ReleaseWait {
        queue
            .try_lock_lane_retirement_observer()
            .err()
            .expect("the original transition mutex is held")
    }

    #[test]
    fn deferred_cut_unlocks_all_original_queue_fences_before_reentrant_callbacks() {
        use crate::publication_lock::PublicationMutex;
        struct Reenter {
            queue: Arc<Queue>,
            outer: Arc<PublicationMutex>,
            wakes: AtomicUsize,
        }
        impl Wake for Reenter {
            fn wake(self: Arc<Self>) {
                assert!(self.outer.try_lock_or_wait().is_ok(), "outer fence released");
                assert!(self.queue.lane_reservation_transition_lock.try_lock_or_wait().is_ok());
                assert!(self.queue.push_remove_lock.try_lock_or_wait().is_ok());
                assert!(self.queue.lane_reservations.try_lock_or_wait().is_ok());
                self.wakes.fetch_add(1, Ordering::SeqCst);
            }
        }
        for unwind in [false, true] {
            let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
            let queue = Arc::new(Queue::test(config_factory(), &time_source));
            let outer = Arc::new(PublicationMutex::default());
            let guard = outer.lock();
            let cut = queue.try_lock_lane_retirement_observer().unwrap().try_into_cut().unwrap();
            let mut waits = [
                queue.lane_reservation_transition_lock.try_lock_or_wait().err().unwrap().wait_for_release(),
                queue.push_remove_lock.try_lock_or_wait().err().unwrap().wait_for_release(),
                queue.lane_reservations.try_lock_or_wait().err().unwrap().wait_for_release(),
            ];
            let probe = Arc::new(Reenter { queue: Arc::clone(&queue), outer: Arc::clone(&outer), wakes: AtomicUsize::new(0) });
            let waker = Waker::from(Arc::clone(&probe));
            for wait in &mut waits {
                assert!(Pin::new(wait).poll(&mut Context::from_waker(&waker)).is_pending());
            }
            let released = cut.release_deferred();
            assert_eq!(probe.wakes.load(Ordering::SeqCst), 0);
            if unwind {
                assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
                    let _released = released;
                    let _outer = guard;
                    panic!("Queue completion unwind");
                })).is_err());
            } else {
                drop(guard);
                drop(released);
            }
            assert_eq!(probe.wakes.load(Ordering::SeqCst), 3);
            for wait in &mut waits {
                assert!(Pin::new(wait).poll(&mut Context::from_waker(&waker)).is_ready());
            }
            assert!(!queue.transaction_selection_durability_faulted());
        }
    }

    #[test]
    fn refused_cut_retains_original_notifications_through_outer_fence() {
        use crate::publication_lock::PublicationMutex;
        struct Reenter { queue: Arc<Queue>, outer: Arc<PublicationMutex>, wakes: AtomicUsize }
        impl Wake for Reenter {
            fn wake(self: Arc<Self>) {
                assert!(self.outer.try_lock_or_wait().is_ok());
                assert!(self.queue.lane_reservation_transition_lock.try_lock_or_wait().is_ok());
                assert!(self.queue.push_remove_lock.try_lock_or_wait().is_ok());
                assert!(self.queue.lane_reservations.try_lock_or_wait().is_ok());
                self.wakes.fetch_add(1, Ordering::SeqCst);
            }
        }
        for reservations in [false, true] {
            let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
            let queue = Arc::new(Queue::test(config_factory(), &time_source));
            let outer = Arc::new(PublicationMutex::default());
            let observer = queue.try_lock_lane_retirement_observer().unwrap();
            let guard = outer.lock();
            let mut wait = waiting(&queue).wait_for_release();
            let callback = Arc::new(Reenter { queue: Arc::clone(&queue), outer: Arc::clone(&outer), wakes: AtomicUsize::new(0) });
            let waker = Waker::from(Arc::clone(&callback));
            assert!(Pin::new(&mut wait).poll(&mut Context::from_waker(&waker)).is_pending());
            let mutation = (!reservations).then(|| queue.push_remove_lock.lock());
            let reservation = reservations.then(|| queue.lane_reservations.lock());
            let (error, cleanup) = observer.try_into_cut().err().expect("held inner owner");
            assert_eq!(error.field, if reservations { "lane_reservations" } else { "push_remove_lock" });
            assert_eq!(callback.wakes.load(Ordering::SeqCst), 0);
            drop((mutation, reservation));
            assert_eq!(callback.wakes.load(Ordering::SeqCst), 0);
            drop(guard);
            drop(cleanup);
            assert_eq!(callback.wakes.load(Ordering::SeqCst), 1);
            assert!(Pin::new(&mut wait).poll(&mut Context::from_waker(Waker::noop())).is_ready());
        }
    }

    #[test]
    fn cut_waits_on_exact_inner_owner_and_releases_every_attempted_guard() {
        let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
        let queue = Queue::test(config_factory(), &time_source);
        let other = Queue::test(config_factory(), &time_source);
        for field in ["push_remove_lock", "lane_reservations"] {
            let mutation = (field == "push_remove_lock").then(|| queue.push_remove_lock.lock());
            let reservations =
                (field == "lane_reservations").then(|| queue.lane_reservations.lock());
            let (busy, cleanup) = queue
                .try_lock_lane_retirement_observer()
                .expect("outer available")
                .try_into_cut()
                .err()
                .expect("the actual inner owner is held");
            drop(cleanup);
            assert_eq!(busy.field, field);
            // Refusal cannot retain T, or P when R was the contended mutex.
            drop(
                queue
                    .lane_reservation_transition_lock
                    .try_lock()
                    .expect("outer released"),
            );
            if field == "lane_reservations" {
                drop(
                    queue
                        .push_remove_lock
                        .try_lock()
                        .expect("mutation attempt released"),
                );
            }
            let count = Arc::new(WakeCount::default());
            let mut wait = busy.wait.wait_for_release();
            assert!(
                poll(&mut wait, &count).is_pending(),
                "an earlier guard release is not this wait"
            );
            drop(other.push_remove_lock.lock());
            drop(other.lane_reservations.lock());
            assert_eq!(count.0.load(Ordering::SeqCst), 0);
            drop(mutation);
            drop(reservations);
            assert_eq!(count.0.load(Ordering::SeqCst), 1);
            assert!(poll(&mut wait, &count).is_ready());

            let cut = queue
                .try_lock_lane_retirement_observer()
                .expect("retry outer")
                .try_into_cut()
                .expect("retry inner owners");
            assert!(queue.push_remove_lock.try_lock().is_none());
            assert!(queue.lane_reservations.try_lock().is_none());
            assert!(queue.lane_reservation_transition_lock.try_lock().is_none());
            drop(cut);
        }
    }

    #[test]
    fn cut_release_before_registration_and_unwind_wake_original_inner_waiters() {
        let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
        let queue = Queue::test(config_factory(), &time_source);
        for unwind in [false, true] {
            let cut = queue
                .try_lock_lane_retirement_observer()
                .expect("outer")
                .try_into_cut()
                .expect("inner owners");
            let mutation_wait = queue
                .push_remove_lock
                .try_lock_or_wait()
                .err()
                .expect("P held");
            let reservation_wait = queue
                .lane_reservations
                .try_lock_or_wait()
                .err()
                .expect("R held");
            let transition_wait = waiting(&queue);
            assert_ne!(mutation_wait, reservation_wait);
            assert_ne!(reservation_wait, transition_wait);
            if unwind {
                assert!(
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
                        let _cut = cut;
                        panic!("abort retained Queue cut");
                    }))
                    .is_err()
                );
            } else {
                drop(cut);
            }
            // A successor cannot steal the release observed by the failed attempt.
            let successor = queue
                .try_lock_lane_retirement_observer()
                .expect("successor outer")
                .try_into_cut()
                .expect("successor owners");
            let count = Arc::new(WakeCount::default());
            for wait in [mutation_wait, reservation_wait, transition_wait] {
                assert!(poll(&mut wait.wait_for_release(), &count).is_ready());
            }
            let mut successor_wait = queue
                .lane_reservations
                .try_lock_or_wait()
                .err()
                .expect("successor holds R")
                .wait_for_release();
            assert!(poll(&mut successor_wait, &count).is_pending());
            drop(successor);
            assert!(poll(&mut successor_wait, &count).is_ready());
        }
        assert!(!queue.transaction_selection_durability_faulted());
    }

    #[test]
    fn retained_cut_excludes_enqueue_from_an_already_captured_state_view() {
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let mut state = State::new(world_with_test_domains(), kura, query_handle);
        let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
        let queue = Queue::test(config_factory(), &time_source);
        let tx = accepted_tx_by_someone(&time_source);
        register_accepted_tx_authority_for_queue_test(&mut state, &tx);
        let hash = tx.hash_as_entrypoint();
        let incarnation = Hash::new(b"retained-empty-queue-cut");
        let (captured_tx, captured_rx) = std::sync::mpsc::sync_channel(1);
        let (proceed_tx, proceed_rx) = std::sync::mpsc::sync_channel(1);
        let (done_tx, done_rx) = std::sync::mpsc::sync_channel(1);
        let (captured, observed_pending, mutation_held, while_held, result) =
            std::thread::scope(|scope| {
                let worker_queue = &queue;
                let worker_state = &state;
                let worker = scope.spawn(move || {
                    // State views retain thread-local EBR readers. Capture and
                    // consume this original view on the same worker thread.
                    let view = worker_state.view();
                    let _ = captured_tx.send(());
                    if proceed_rx.recv_timeout(Duration::from_secs(5)).is_err() {
                        return None;
                    }
                    let result = worker_queue.push(tx, view);
                    let _ = done_tx.send(());
                    Some(result)
                });
                let captured = captured_rx.recv_timeout(Duration::from_secs(5));
                let cut = queue
                    .try_lock_lane_retirement_observer()
                    .ok()
                    .and_then(|observer| observer.try_into_cut().ok());
                let observed_pending = cut.as_ref().map(|cut| {
                    cut.lane_has_pending_work(LaneId::SINGLE, DataSpaceId::UNIVERSAL, incarnation)
                });
                let mutation_held = queue.push_remove_lock.try_lock().is_none();
                let _ = proceed_tx.send(());
                let while_held = done_rx.try_recv();
                // Release physical owners and channels before assertions/joins,
                // including if acquisition or the capture acknowledgement failed.
                drop(cut);
                drop(proceed_tx);
                drop(captured_rx);
                drop(done_rx);
                (
                    captured,
                    observed_pending,
                    mutation_held,
                    while_held,
                    worker.join(),
                )
            });
        assert_eq!(captured, Ok(()));
        assert_eq!(
            observed_pending,
            Some(false),
            "original cut acquired after the view capture"
        );
        assert!(mutation_held);
        assert!(matches!(
            while_held,
            Err(std::sync::mpsc::TryRecvError::Empty)
        ));
        result
            .expect("enqueue worker")
            .expect("worker released to enqueue")
            .expect("captured-view enqueue after release");
        let retry = queue
            .try_lock_lane_retirement_observer()
            .expect("retry outer")
            .try_into_cut()
            .expect("retry owners");
        assert!(retry.lane_has_pending_work(LaneId::SINGLE, DataSpaceId::UNIVERSAL, incarnation));
        assert!(queue.contains_entrypoint_hash(hash));
        assert_eq!(queue.active_len(), 1);
    }

    #[test]
    fn cut_preserves_exact_reservation_barriers_and_fail_stop() {
        let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
        let state = lane_reservation_test_state();
        let queue = Arc::new(Queue::test(config_factory(), &time_source));
        let dir = tempdir().expect("durable Queue fixture");
        install_test_reservation_journal(&queue, &dir);
        push_globally_bound_lane_reservation_candidate(
            &queue,
            &state,
            &dir,
            accepted_queue_plan_tx_by_someone(&time_source),
        );
        let scope = lane_reservation_scope(&state, b"cut-owner", b"cut-proposal");
        let other = Hash::new(b"cut-recreated-incarnation");
        let key = *queue
            .reserve_transactions_for_lane(&state, scope, nonzero!(1_usize))
            .expect("durable reservation")[0]
            .key();
        for barrier in [false, true] {
            if barrier {
                let _transition = queue.lane_reservation_transition_lock.lock();
                queue
                    .lane_reservation_journal
                    .lock()
                    .as_mut()
                    .expect("journal")
                    .commit(key)
                    .expect("durable commit barrier");
                let mut reservations = queue.lane_reservations.lock();
                reservations.live_by_entrypoint.remove(&key.entrypoint_hash);
                reservations.commit_barriers.push(key);
            }
            let cut = queue
                .try_lock_lane_retirement_observer()
                .expect("outer")
                .try_into_cut()
                .expect("inner owners");
            assert!(cut.lane_has_pending_work(
                scope.lane_id,
                scope.dataspace_id,
                scope.lane_incarnation
            ));
            assert!(!cut.lane_has_pending_work(scope.lane_id, scope.dataspace_id, other));
            assert!(cut.lane_has_pending_work(
                scope.lane_id,
                scope.dataspace_id,
                Hash::prehashed([0; Hash::LENGTH])
            ));
            if barrier {
                queue
                    .lane_reservation_durability_fault
                    .store(true, Ordering::Release);
                assert!(cut.lane_has_pending_work(LaneId::new(77), DataSpaceId::new(77), other));
            }
        }
    }

    #[test]
    fn blocking_and_try_observers_wake_on_normal_and_aborted_release() {
        let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
        let queue = Queue::test(config_factory(), &time_source);
        for aborted in [false, true] {
            let original = queue.lock_lane_retirement_observer();
            let count = Arc::new(WakeCount::default());
            let mut wait = waiting(&queue).wait_for_release();
            assert!(poll(&mut wait, &count).is_pending());
            if aborted {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
                    let _original = original;
                    panic!("abort Queue observation without any publication");
                }));
                assert!(result.is_err());
            } else {
                drop(original);
            }
            assert_eq!(count.0.load(Ordering::SeqCst), 1);
            assert!(poll(&mut wait, &count).is_ready());
            let retry = queue
                .try_lock_lane_retirement_observer()
                .expect("non-poisoning physical mutex is available after release");
            assert!(queue.lane_reservation_transition_lock.try_lock().is_none());
            let mut retry_wait = waiting(&queue).wait_for_release();
            assert!(poll(&mut retry_wait, &count).is_pending());
            drop(retry);
            assert!(poll(&mut retry_wait, &count).is_ready());
            assert!(queue.lane_reservation_transition_lock.try_lock().is_some());
        }
        assert_eq!(queue.active_len(), 0);
        assert_eq!(queue.queued_len(), 0);
        assert!(!queue.transaction_selection_durability_faulted());
    }

    #[test]
    fn release_before_wait_registration_does_not_alias_a_successor_observer() {
        let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
        let queue = Queue::test(config_factory(), &time_source);
        let original = queue.lock_lane_retirement_observer();
        let original_wait = waiting(&queue);
        drop(original);
        let successor = queue
            .try_lock_lane_retirement_observer()
            .expect("successor owns the same physical mutex");
        let successor_wait = waiting(&queue);
        assert_ne!(original_wait, successor_wait);
        let count = Arc::new(WakeCount::default());
        assert!(poll(&mut original_wait.wait_for_release(), &count).is_ready());
        let mut successor_wait = successor_wait.wait_for_release();
        assert!(poll(&mut successor_wait, &count).is_pending());
        drop(successor);
        assert_eq!(count.0.load(Ordering::SeqCst), 1);
        assert!(poll(&mut successor_wait, &count).is_ready());
    }

    #[test]
    fn other_queues_and_other_locks_do_not_wake_retirement_waiters() {
        let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
        let queue = Queue::test(config_factory(), &time_source);
        let other = Queue::test(config_factory(), &time_source);
        let original = queue.lock_lane_retirement_observer();
        let other_original = other.lock_lane_retirement_observer();
        assert_ne!(waiting(&queue), waiting(&other));
        let count = Arc::new(WakeCount::default());
        let mut wait = waiting(&queue).wait_for_release();
        assert!(poll(&mut wait, &count).is_pending());
        drop(other_original);
        drop(
            other
                .try_lock_lane_retirement_observer()
                .expect("other Queue"),
        );
        drop(queue.push_remove_lock.lock());
        drop(queue.lane_reservations.lock());
        assert_eq!(count.0.load(Ordering::SeqCst), 0);
        assert!(poll(&mut wait, &count).is_pending());
        drop(original);
        assert_eq!(count.0.load(Ordering::SeqCst), 1);
        assert!(poll(&mut wait, &count).is_ready());
    }

    #[test]
    fn canceled_wait_keeps_other_waiter_and_owned_thread_handoff_live() {
        let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
        let queue = Queue::test(config_factory(), &time_source);
        let original = queue.lock_lane_retirement_observer();
        let count = Arc::new(WakeCount::default());
        let mut canceled = waiting(&queue).wait_for_release();
        assert!(poll(&mut canceled, &count).is_pending());
        let handoff = waiting(&queue);
        // The worker owns only the observation, with no Queue borrow or guard.
        let retained = std::thread::spawn(move || handoff)
            .join()
            .expect("owned release observation crosses a static worker boundary");
        let live_count = Arc::new(WakeCount::default());
        let mut retained = retained.wait_for_release();
        assert!(poll(&mut retained, &live_count).is_pending());
        drop(canceled);
        drop(original);
        assert_eq!(count.0.load(Ordering::SeqCst), 0);
        assert_eq!(live_count.0.load(Ordering::SeqCst), 1);
        assert!(poll(&mut retained, &live_count).is_ready());
        drop(queue.try_lock_lane_retirement_observer().expect("retry"));
    }

    #[test]
    fn try_observer_preserves_exact_reservation_barriers_and_fault_predicate() {
        let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
        let state = lane_reservation_test_state();
        let queue = Arc::new(Queue::test(config_factory(), &time_source));
        let dir = tempdir().expect("durable Queue fixture");
        install_test_reservation_journal(&queue, &dir);
        push_globally_bound_lane_reservation_candidate(
            &queue,
            &state,
            &dir,
            accepted_queue_plan_tx_by_someone(&time_source),
        );
        let scope = lane_reservation_scope(&state, b"try-observer-owner", b"try-observer-proposal");
        let other_incarnation = Hash::new(b"try-observer-recreated-incarnation");
        {
            let observer = queue
                .try_lock_lane_retirement_observer()
                .expect("observe FIFO");
            assert!(!observer.durability_faulted());
            assert!(observer.lane_has_pending_work(
                scope.lane_id,
                scope.dataspace_id,
                scope.lane_incarnation
            ));
            assert!(observer.lane_has_pending_work(
                scope.lane_id,
                scope.dataspace_id,
                other_incarnation
            ));
            assert!(!observer.lane_has_pending_work(
                LaneId::new(77),
                scope.dataspace_id,
                scope.lane_incarnation
            ));
            assert!(!observer.lane_has_pending_work(
                scope.lane_id,
                DataSpaceId::new(77),
                scope.lane_incarnation
            ));
            assert!(observer.lane_has_pending_work(
                scope.lane_id,
                scope.dataspace_id,
                Hash::prehashed([0; Hash::LENGTH])
            ));
        }
        let key = *queue
            .reserve_transactions_for_lane(&state, scope, nonzero!(1_usize))
            .expect("actual durable reservation")[0]
            .key();
        {
            let observer = queue
                .try_lock_lane_retirement_observer()
                .expect("observe reservation");
            assert!(!observer.durability_faulted());
            assert!(observer.lane_has_pending_work(
                scope.lane_id,
                scope.dataspace_id,
                scope.lane_incarnation
            ));
            assert!(!observer.lane_has_pending_work(
                scope.lane_id,
                scope.dataspace_id,
                other_incarnation
            ));
        }
        // Preserve the same durable-commit crash cut as the existing blocking
        // observer regression: the exact tombstone remains retirement work.
        {
            let _transition = queue.lane_reservation_transition_lock.lock();
            queue
                .lane_reservation_journal
                .lock()
                .as_mut()
                .expect("reservation journal")
                .commit(key)
                .expect("persist commit barrier");
            let mut reservations = queue.lane_reservations.lock();
            reservations.live_by_entrypoint.remove(&key.entrypoint_hash);
            reservations.commit_barriers.push(key);
        }
        {
            let observer = queue
                .try_lock_lane_retirement_observer()
                .expect("observe barrier");
            assert!(!observer.durability_faulted());
            assert!(observer.lane_has_pending_work(
                scope.lane_id,
                scope.dataspace_id,
                scope.lane_incarnation
            ));
            assert!(!observer.lane_has_pending_work(
                scope.lane_id,
                scope.dataspace_id,
                other_incarnation
            ));
        }
        queue
            .lane_reservation_durability_fault
            .store(true, Ordering::Release);
        let observer = queue
            .try_lock_lane_retirement_observer()
            .expect("observe fault");
        assert!(observer.durability_faulted());
        assert!(observer.lane_has_pending_work(
            LaneId::new(88),
            DataSpaceId::new(88),
            other_incarnation
        ));
    }

    #[test]
    fn try_observer_retains_every_admitted_cross_route_leg() {
        let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
        let fixture = native_amx_participant_drift_fixture(&time_source);
        let queue = Queue::test(config_factory(), &time_source);
        let hash = fixture.tx.hash_as_entrypoint();
        queue
            .push_with_gossip_payload_with_state_and_routing_plan(
                fixture.tx,
                &fixture.state,
                fixture.current_plan.clone(),
                None,
            )
            .expect("admit the plan derived from current account/domain ownership");
        let observer = queue
            .try_lock_lane_retirement_observer()
            .expect("observe all route legs");
        let legs = fixture.current_plan.legs();
        assert!(legs.iter().any(|leg| leg.role == RouteLegRole::Participant));
        for leg in legs {
            for incarnation in [Hash::new(b"cross-route-old"), Hash::new(b"cross-route-new")] {
                assert!(observer.lane_has_pending_work(
                    leg.route.lane_id,
                    leg.route.dataspace_id,
                    incarnation
                ));
            }
        }
        assert!(!observer.lane_has_pending_work(
            LaneId::new(77),
            DataSpaceId::new(77),
            Hash::new(b"unrelated-route")
        ));
        assert!(queue.txs.contains_key(&hash));
        assert_eq!(queue.routing_plan_hint(&hash), Some(fixture.current_plan));
        assert_eq!(queue.active_len(), 1);
        assert_eq!(queue.queued_len(), 1);
        assert!(!queue.transaction_selection_durability_faulted());
    }
}

// Included by queue::tests to exercise the actual journal and State fixtures.
mod replay_terminal_release {
    //! Canonical cleanup must finish when local selection releases, without another block.

    use super::*;

    fn select(fixture: &GloballyBoundGuardFixture) -> GlobalQueueSelectionLease {
        install_queue_plan_registry_value_for_test(&fixture.state, &fixture.binding);
        let (selected, lease) = fixture
            .queue
            .bounded_pending_snapshot(&fixture.state.view(), nonzero!(1_usize))
            .expect("select the original QueuePlan owner");
        assert_eq!(selected.len(), 1);
        assert_eq!(
            selected[0].hash_as_entrypoint(),
            fixture.binding.entrypoint_hash
        );
        lease
    }

    fn defer_committed_cleanup(fixture: &GloballyBoundGuardFixture) -> concread::release::ReleaseFuture {
        commit_globally_bound_fixture_directly(fixture);
        assert_eq!(
            fixture
                .queue
                .remove_state_committed_replay_owners_preserving_globally_bound(
                    &fixture.state.view(),
                    None,
                )
                .expect("authenticate canonical cleanup while selection is retained"),
            0,
        );
        fixture.assert_live_journal_claim();
        let mut wait = fixture
            .queue
            .lock_lane_retirement_observer()
            .lane_pending_work_release(
                LaneId::SINGLE,
                DataSpaceId::UNIVERSAL,
                fixture.binding.admission_context.route_incarnations[0].lane_incarnation,
            )
            .expect("healthy original Queue")
            .expect("the retained Queue owner still blocks retirement")
            .wait_for_release();
        assert!(poll_lane_retirement_release(&mut wait).is_pending());
        wait
    }

    #[test]
    fn original_selection_drop_finishes_canonical_cleanup_without_another_apply() {
        let fixture = globally_bound_guard_fixture();
        let lease = select(&fixture);
        let mut wait = defer_committed_cleanup(&fixture);
        let height = fixture.state.committed_height();
        let (wake_tx, wake_rx) = mpsc::sync_channel(8);
        fixture.queue.set_sumeragi_wake(wake_tx);

        drop(lease);

        fixture.assert_terminally_removed();
        assert!(poll_lane_retirement_release(&mut wait).is_ready());
        assert!(
            wake_rx.try_recv().is_ok(),
            "terminal cleanup wakes retirement"
        );
        assert_eq!(fixture.state.committed_height(), height);
        assert!(fixture.queue.global_selection_owners.lock().is_empty());
    }

    #[test]
    fn selection_narrowing_finishes_only_released_canonical_owners() {
        let fixture = globally_bound_guard_fixture();
        let mut lease = select(&fixture);
        let mut wait = defer_committed_cleanup(&fixture);
        assert!(lease.retain_only(&[fixture.binding.entrypoint_hash]));
        fixture.assert_live_journal_claim();
        assert!(poll_lane_retirement_release(&mut wait).is_pending());

        assert!(lease.retain_only(&[]));

        fixture.assert_terminally_removed();
        assert!(poll_lane_retirement_release(&mut wait).is_ready());
        drop(lease);
        fixture.assert_terminally_removed();
    }

    #[test]
    fn last_selection_attempt_finishes_retained_canonical_cleanup() {
        let fixture = globally_bound_guard_fixture();
        let first = fixture.queue.begin_selection_attempt();
        let last = fixture.queue.begin_selection_attempt();
        let mut wait = defer_committed_cleanup(&fixture);
        drop(first);
        fixture.assert_live_journal_claim();
        assert!(poll_lane_retirement_release(&mut wait).is_pending());

        drop(last);

        fixture.assert_terminally_removed();
        assert!(poll_lane_retirement_release(&mut wait).is_ready());
    }

    #[test]
    fn popped_guard_release_finishes_canonical_cleanup_after_fifo_restoration() {
        let fixture = globally_bound_guard_fixture();
        let guard = fixture.pop_guard();
        let mut wait = defer_committed_cleanup(&fixture);

        drop(guard);

        fixture.assert_terminally_removed();
        assert!(poll_lane_retirement_release(&mut wait).is_ready());
        assert_eq!(fixture.queue.inflight_guards.load(Ordering::Acquire), 0);
    }

    #[test]
    fn final_local_owner_release_retries_after_either_release_order() {
        for guard_first in [false, true] {
            let fixture = globally_bound_guard_fixture();
            let guard = fixture.pop_guard();
            let attempt = fixture.queue.begin_selection_attempt();
            let mut wait = defer_committed_cleanup(&fixture);
            if guard_first {
                drop(guard);
                fixture.assert_live_journal_claim();
                assert!(poll_lane_retirement_release(&mut wait).is_pending());
                drop(attempt);
            } else {
                drop(attempt);
                fixture.assert_live_journal_claim();
                assert!(poll_lane_retirement_release(&mut wait).is_pending());
                drop(guard);
            }
            fixture.assert_terminally_removed();
            assert!(poll_lane_retirement_release(&mut wait).is_ready());
        }
    }

    #[test]
    fn local_release_without_canonical_evidence_retains_original_claim() {
        let fixture = globally_bound_guard_fixture();
        let lease = select(&fixture);
        assert_eq!(
            fixture
                .queue
                .remove_state_committed_replay_owners_preserving_globally_bound(
                    &fixture.state.view(),
                    None,
                )
                .expect("uncommitted owner has no terminal authority"),
            0,
        );
        drop(lease);
        fixture.assert_restored_fifo_owner();
        let attempt = fixture.queue.begin_selection_attempt();
        drop(attempt);
        fixture.assert_restored_fifo_owner();
        let guard = fixture.pop_guard();
        drop(guard);
        fixture.assert_restored_fifo_owner();
    }

    #[test]
    fn terminal_cleanup_durability_failure_retains_owner_and_wakes_recovery() {
        let fixture = globally_bound_guard_fixture();
        let lease = select(&fixture);
        let mut wait = defer_committed_cleanup(&fixture);
        fixture
            .queue
            .inject_plan_journal_fault(QueuePlanJournalTestFault::GeneralParentSync);

        drop(lease);

        let hash = fixture.binding.entrypoint_hash;
        assert!(fixture.queue.transaction_selection_durability_faulted());
        assert!(fixture.queue.txs.contains_key(&hash));
        assert!(fixture.queue.durable_plan_claims.contains_key(&hash));
        assert!(fixture.queue.routing_plans.contains_key(&hash));
        assert_eq!(fixture.queue.active_len(), 1);
        assert!(poll_lane_retirement_release(&mut wait).is_ready());
        assert_eq!(
            fixture
                .queue
                .lock_lane_retirement_observer()
                .lane_pending_work_release(
                    LaneId::SINGLE,
                    DataSpaceId::UNIVERSAL,
                    fixture.binding.admission_context.route_incarnations[0].lane_incarnation,
                ),
            Err(QueueLaneRetirementUnavailable::DurabilityFault),
        );
    }

    #[test]
    fn local_release_preserves_actual_autonomous_reservation_custody() {
        let fixture = globally_bound_guard_fixture_with_journals(0, true);
        install_queue_plan_registry_value_for_test(&fixture.state, &fixture.binding);
        let scope = lane_reservation_scope(
            &fixture.state,
            b"replay-terminal-reserved-owner",
            b"replay-terminal-reserved-proposal",
        );
        let reservation = fixture
            .queue
            .reserve_transactions_for_lane(&fixture.state, scope, nonzero!(1_usize))
            .expect("reserve original autonomous owner");
        assert_eq!(reservation.len(), 1);
        let original = fixture.queue.live_lane_reservations();
        let attempt = fixture.queue.begin_selection_attempt();
        let mut wait = defer_committed_cleanup(&fixture);

        drop(attempt);

        fixture.assert_live_journal_claim();
        assert_eq!(fixture.queue.active_len(), 1);
        assert_eq!(fixture.queue.live_lane_reservations(), original);
        assert!(poll_lane_retirement_release(&mut wait).is_pending());
        assert!(!fixture.queue.transaction_selection_durability_faulted());
    }
}

// Included by queue::tests; use the original QueuePlan and reservation journals.
mod replay_terminal_custody {
    //! Ordinary release must not replace an autonomous Kura terminal join.

    use super::*;

    #[test]
    fn forgotten_queue_release_does_not_authorize_replay_terminal_cleanup() {
        let fixture = globally_bound_guard_fixture_with_journals(0, true);
        install_queue_plan_registry_value_for_test(&fixture.state, &fixture.binding);
        let reserved = fixture
            .queue
            .reserve_transactions_for_lane(
                &fixture.state,
                lane_reservation_scope(
                    &fixture.state,
                    b"replay-terminal-release-owner",
                    b"replay-terminal-release-proposal",
                ),
                nonzero!(1_usize),
            )
            .expect("reserve the original admission");
        let keys = reserved
            .iter()
            .map(|entry| *entry.key())
            .collect::<Vec<_>>();
        assert_eq!(keys.len(), 1);
        let barrier = lane_reservation_release_barrier(keys, b"replay-terminal-release");
        fixture
            .queue
            .prepare_lane_reservation_release_barrier(&barrier)
            .expect("prepare the exact Queue release");
        // Exercise the real Queue journal suffix only. No Kura Complete
        // authorization is manufactured by this fixture or consumed below.
        assert_eq!(
            fixture
                .queue
                .finalize_lane_reservation_release_barrier(&barrier)
                .expect("restore FIFO and durably forget the Queue release"),
            1,
        );
        assert!(fixture.queue.live_lane_reservations().is_empty());
        assert!(fixture.queue.lane_reservation_release_barriers().is_empty());
        fixture.assert_restored_fifo_owner();

        let attempt = fixture.queue.begin_selection_attempt();
        commit_globally_bound_fixture_directly(&fixture);
        assert_eq!(
            fixture
                .queue
                .remove_state_committed_replay_owners_preserving_globally_bound(
                    &fixture.state.view(),
                    None,
                )
                .expect("State application cannot substitute for Kura Complete"),
            0,
        );
        let mut wait = fixture
            .queue
            .lock_lane_retirement_observer()
            .lane_pending_work_release(
                LaneId::SINGLE,
                DataSpaceId::UNIVERSAL,
                fixture.binding.admission_context.route_incarnations[0].lane_incarnation,
            )
            .expect("healthy original Queue")
            .expect("released autonomous FIFO still owns retirement work")
            .wait_for_release();
        drop(attempt);

        fixture.assert_restored_fifo_owner();
        assert!(poll_lane_retirement_release(&mut wait).is_pending());
        assert!(!fixture.queue.transaction_selection_durability_faulted());
    }
}

/// Run a regression while one actual inner retirement mutex is held.
pub(crate) fn with_retirement_inner_fence_for_test<R>(queue: &Queue, reservations: Option<bool>, run: impl FnOnce() -> R) -> R {
    match reservations {
        Some(true) => { let _held = queue.lane_reservations.lock(); run() },
        Some(false) => { let _held = queue.push_remove_lock.lock(); run() },
        None => run(),
    }
}
