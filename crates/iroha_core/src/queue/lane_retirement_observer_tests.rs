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

    fn poll(wait: &mut mv::ReleaseFuture, count: &Arc<WakeCount>) -> Poll<()> {
        let waker = Waker::from(Arc::clone(count));
        Pin::new(wait).poll(&mut Context::from_waker(&waker))
    }

    fn waiting(queue: &Queue) -> mv::ReleaseWait {
        queue
            .try_lock_lane_retirement_observer()
            .err()
            .expect("the original transition mutex is held")
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
