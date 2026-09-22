//! Retained hash callbacks follow actual State, Queue and Kura fence release.
use super::*;

struct Probe {
    state: Arc<State>,
    queue: Arc<Queue>,
    wakes: AtomicUsize,
    busy: AtomicUsize,
}
impl Wake for Probe {
    fn wake(self: Arc<Self>) {
        self.wakes.fetch_add(1, Ordering::SeqCst);
        let state_busy = [
            &self.state.state_commit_lock,
            &self.state.lane_lifecycle_lock,
            &self.state.state_write_lock,
        ]
        .into_iter()
        .filter(|lock| lock.try_lock().is_none())
        .count();
        let kura_busy = self.state.kura.try_publication_lease().is_err();
        let queue_busy = match self.queue.try_lock_lane_retirement_observer() {
            Ok(observer) => observer.try_into_cut().is_err(),
            Err(_) => true,
        };
        let hash_busy = self
            .state
            .block_hashes
            .map()
            .unwrap()
            .try_acquire_writer()
            .is_none();
        self.busy.fetch_add(
            state_busy + usize::from(kura_busy) + usize::from(queue_busy) + usize::from(hash_busy),
            Ordering::SeqCst,
        );
    }
}

#[test]
fn retained_hash_admission_and_preflight_cleanup_follow_all_original_carrier_fences() {
    for failure in 0..3 {
        let (state, decision) = fixture_decision();
        let state: Arc<State> = state.into();
        let queue = phase_queue();
        let source = OriginalCarrierQueue::for_test(&state, &queue);
        let cut = source.try_observe().unwrap().try_into_cut().unwrap();
        let queue_owner = CarrierQueueRetirement::try_new(
            &state,
            &decision.journals.geometry,
            decision.block().header(),
            &source,
            cut,
        )
        .unwrap_or_else(|_| panic!("original Queue cut"));
        let original_pointer = decision
            .journals
            .components
            .block_hashes
            .get(0)
            .map(std::ptr::from_ref);
        let probe = Arc::new(Probe {
            state: Arc::clone(&state),
            queue: Arc::clone(&queue),
            wakes: AtomicUsize::new(0),
            busy: AtomicUsize::new(0),
        });
        let waker = Waker::from(Arc::clone(&probe));
        let mut wait = state
            .block_hashes
            .map()
            .unwrap()
            .observe_reader_release()
            .wait_for_release();
        assert!(
            Pin::new(&mut wait)
                .poll(&mut Context::from_waker(&waker))
                .is_pending()
        );
        let fences = CarrierFences {
            _state: StateFences::try_acquire::<Infallible>(&state)
                .unwrap_or_else(|_| panic!("original State fences")),
            _queue: Some(queue_owner),
            _kura: state.kura.try_publication_lease().unwrap(),
        };
        let mut owner = CarrierPreparation::new(decision.journals.components, &state, fences);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            owner.try_prepare_hash(|_, target| {
                assert!(target.writer_available());
                match failure {
                    1 => Err("installation capacity"),
                    2 => panic!("actual admission unwind"),
                    _ => Ok(()),
                }
            })
        }));
        assert_eq!(probe.wakes.load(Ordering::SeqCst), 0);
        assert!(state.state_commit_lock.try_lock().is_none());
        assert!(queue.try_lock_lane_retirement_observer().is_err());
        assert!(state.kura.try_publication_lease().is_err());
        let original = match failure {
            0 => {
                assert!(matches!(result, Ok(Ok(()))));
                Some(owner.recover_original())
            }
            1 => {
                assert!(matches!(
                    result,
                    Ok(Err(mv::PublicationPreparationError::Admission(
                        "installation capacity"
                    )))
                ));
                Some(owner.recover_original())
            }
            _ => {
                assert!(result.is_err());
                None
            }
        };
        assert_eq!(probe.wakes.load(Ordering::SeqCst), 0);
        drop(owner);
        assert_eq!(probe.wakes.load(Ordering::SeqCst), 1);
        assert_eq!(probe.busy.load(Ordering::SeqCst), 0);
        assert!(
            Pin::new(&mut wait)
                .poll(&mut Context::from_waker(Waker::noop()))
                .is_ready()
        );
        assert_eq!(state.committed_height(), 0);
        if let Some(original) = original {
            assert_eq!(
                original.block_hashes.get(0).map(std::ptr::from_ref),
                original_pointer
            );
            assert!(original.block_hashes.matches_current(&state.block_hashes));
        }
        // The unmoved finality/source/capacity fields outlive component retirement.
    }
}

struct AggregateProbe {
    fences: Probe,
    original: Mutex<Option<DetachedCarrierComponents>>,
    counts: Mutex<Option<[usize; 3]>>,
}

impl Wake for AggregateProbe {
    fn wake(self: Arc<Self>) {
        // The original fence/hash probe uses only nonblocking physical locks.
        let state = &self.fences.state;
        let state_busy = [
            &state.state_commit_lock,
            &state.lane_lifecycle_lock,
            &state.state_write_lock,
        ]
        .into_iter()
        .filter(|lock| lock.try_lock().is_none())
        .count();
        let kura_busy = state.kura.try_publication_lease().is_err();
        let queue_busy = match self.fences.queue.try_lock_lane_retirement_observer() {
            Ok(observer) => observer.try_into_cut().is_err(),
            Err(_) => true,
        };
        let hash_busy = state
            .block_hashes
            .map()
            .unwrap()
            .try_acquire_writer()
            .is_none();
        self.fences.busy.fetch_add(
            state_busy + usize::from(kura_busy) + usize::from(queue_busy) + usize::from(hash_busy),
            Ordering::SeqCst,
        );
        self.fences.wakes.fetch_add(1, Ordering::SeqCst);
        let Ok(mut stored) = self.original.try_lock() else {
            return;
        };
        let Some(original) = stored.take() else {
            return;
        };
        let mut counts = crate::state::world_journals::preparation_tests::probe_fields(
            original.world,
            &state.world,
        );
        macro_rules! probe {
            ($original:expr, $target:expr) => {
                match $original.try_prepare_publication($target, |_, _| Ok::<_, ()>(())) {
                    Ok(prepared) => {
                        counts[0] += 1;
                        drop(prepared.abort());
                    }
                    Err((original, mv::PublicationPreparationError::Busy(_), cleanup)) => {
                        counts[1] += 1;
                        drop((original, cleanup));
                    }
                    Err((original, _, cleanup)) => {
                        counts[2] += 1;
                        drop((original, cleanup));
                    }
                }
            };
        }
        probe!(original.runtime.canonical_runtime, &state.canonical_runtime);
        probe!(original.runtime.commit_topology, &state.commit_topology);
        probe!(
            original.runtime.prev_commit_topology,
            &state.prev_commit_topology
        );
        probe!(
            original.runtime.lane_consensus_contexts,
            &state.lane_consensus_contexts
        );
        probe!(original.transactions, &state.transactions);
        if let Ok(mut result) = self.counts.try_lock() {
            *result = Some(counts);
        }
    }
}

#[test]
fn complete_carrier_late_world_panic_releases_every_participant_before_any_callback() {
    use crate::state::world_journals::preparation_tests::{
        after_last_preparation, arm_first_release,
    };
    for unwind_owner in [false, true] {
        let (state, mut decision) = fixture_decision();
        let state: Arc<State> = state.into();
        let queue = phase_queue();
        let capture_world = || {
            state
                .world
                .block()
                .try_detach_journals(|_| Ok::<_, ()>(()))
                .unwrap()
        };
        let mut observation = Some(capture_world());
        let mut membership = state.transactions.block();
        membership.insert_block(Default::default(), std::num::NonZeroUsize::MIN);
        let probes = DetachedCarrierComponents {
            world: capture_world(),
            runtime: super::super::super::super::runtime_journals::RuntimeJournals::capture(
                state.canonical_runtime.block(),
                state.commit_topology.block(),
                state.prev_commit_topology.block(),
                state.lane_consensus_contexts.block(),
                |_| Ok::<_, ()>(()),
            )
            .unwrap(),
            transactions: membership.prepare_commit().unwrap().detach(),
            block_hashes: state.block_hashes.block().detach(),
        };
        let callback = Arc::new(AggregateProbe {
            fences: Probe {
                state: Arc::clone(&state),
                queue: Arc::clone(&queue),
                wakes: AtomicUsize::new(0),
                busy: AtomicUsize::new(0),
            },
            original: Mutex::new(Some(probes)),
            counts: Mutex::new(None),
        });
        let future = Arc::new(Mutex::new(None));
        let stored = Arc::clone(&future);
        let waker = Waker::from(Arc::clone(&callback));
        after_last_preparation(&mut decision.journals.components.world, move |world| {
            *stored.lock().unwrap() = Some(arm_first_release(
                observation.take().unwrap(),
                world,
                &waker,
            ));
            panic!("complete carrier after final actual World preparation");
        });
        let source = OriginalCarrierQueue::for_test(&state, &queue);
        let cut = source.try_observe().unwrap().try_into_cut().unwrap();
        let queue_owner = CarrierQueueRetirement::try_new(
            &state,
            &decision.journals.geometry,
            decision.block().header(),
            &source,
            cut,
        )
        .unwrap_or_else(|_| panic!("original Queue cut"));
        let fences = CarrierFences {
            _state: StateFences::try_acquire::<Infallible>(&state)
                .unwrap_or_else(|_| panic!("original State fences")),
            _queue: Some(queue_owner),
            _kura: state.kura.try_publication_lease().unwrap(),
        };
        let mut owner = CarrierPreparation::new(decision.journals.components, &state, fences);
        if unwind_owner {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
                owner.try_prepare::<Infallible>()
            }));
            assert!(result.is_err());
        } else {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                owner.try_prepare::<Infallible>()
            }));
            assert!(result.is_err());
            assert_eq!(callback.fences.wakes.load(Ordering::SeqCst), 0);
            assert!(state.state_commit_lock.try_lock().is_none());
            assert!(queue.try_lock_lane_retirement_observer().is_err());
            assert!(state.kura.try_publication_lease().is_err());
            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| owner.recover_original()))
                    .is_err()
            );
            drop(owner);
        }
        assert_eq!(callback.fences.wakes.load(Ordering::SeqCst), 1);
        assert_eq!(callback.fences.busy.load(Ordering::SeqCst), 0);
        let counts = callback
            .counts
            .lock()
            .unwrap()
            .expect("each physical participant probed");
        assert_eq!(
            counts[1], 0,
            "no original writer survives its first callback"
        );
        assert_eq!(counts.iter().sum::<usize>(), 283);
        if !unwind_owner {
            assert_eq!(counts, [283, 0, 0]);
        }
        let mut wait = future.lock().unwrap().take().unwrap();
        assert!(
            Pin::new(&mut wait)
                .poll(&mut Context::from_waker(Waker::noop()))
                .is_ready()
        );
        assert_eq!(state.committed_height(), 0);
    }
}
