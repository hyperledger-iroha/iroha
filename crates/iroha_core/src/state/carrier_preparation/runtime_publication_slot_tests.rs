//! Caller-owned runtime refusal, recovery and unwind keep original cleanup outside siblings.

use super::*;
use std::sync::Mutex;

struct RuntimeSlotProbe {
    state: Arc<State>,
    original: Mutex<Option<RuntimeJournals<()>>>,
    wakes: AtomicUsize,
    acquired: AtomicUsize,
    busy: AtomicUsize,
    other: AtomicUsize,
}
impl Wake for RuntimeSlotProbe {
    fn wake(self: Arc<Self>) {
        self.wakes.fetch_add(1, Ordering::SeqCst);
        let Ok(mut stored) = self.original.try_lock() else {
            self.other.fetch_add(1, Ordering::SeqCst);
            return;
        };
        let Some(original) = stored.take() else {
            self.other.fetch_add(1, Ordering::SeqCst);
            return;
        };
        // Independent real lower probes prevent an early poisoned participant
        // from hiding a later physical sibling. No assertion runs in Wake.
        macro_rules! probe {
            ($field:ident) => {
                match original
                    .$field
                    .try_prepare_publication(&self.state.$field, |_, _| Ok::<_, ()>(()))
                {
                    Ok(prepared) => {
                        let (original, cleanup) = prepared.abort();
                        self.acquired.fetch_add(1, Ordering::SeqCst);
                        drop((original, cleanup));
                    }
                    Err((original, PublicationPreparationError::Busy(_), cleanup)) => {
                        self.busy.fetch_add(1, Ordering::SeqCst);
                        drop((original, cleanup));
                    }
                    Err((original, _, cleanup)) => {
                        self.other.fetch_add(1, Ordering::SeqCst);
                        drop((original, cleanup));
                    }
                }
            };
        }
        probe!(canonical_runtime);
        probe!(commit_topology);
        probe!(prev_commit_topology);
        probe!(lane_consensus_contexts);
    }
}

#[test]
fn runtime_publication_slot_retains_late_refusal_cleanup_and_exact_retry_in_both_modes() {
    for mode in [BlockMode::Ordinary, BlockMode::Replace] {
        let state: Arc<State> = Arc::from(fixture());
        let before = images(&state);
        let watcher = capture(blocks(&state, mode), ());
        let reentry = capture(blocks(&state, mode), ());
        let mut original = blocks(&state, mode);
        mutate(&mut original, 7);
        let original = capture(original, ());
        let pointer = original_topology_allocation(&original);
        let late_busy = state.lane_consensus_contexts.block();
        let mut slot = original.publication_slot(&state);
        assert!(matches!(
            slot.try_prepare(|_, _| Ok::<_, ()>(())),
            Err(RuntimePublicationError::Component {
                field: "lane_consensus_contexts",
                cause: PublicationPreparationError::Busy(_),
            })
        ));
        let (watcher, error, watcher_cleanup) = watcher
            .canonical_runtime
            .try_prepare_publication(&state.canonical_runtime, |_, _| Ok::<_, ()>(()))
            .err()
            .expect("earlier original runtime remains acquired after normal refusal");
        let PublicationPreparationError::Busy(observation) = error else {
            panic!("earlier runtime busy");
        };
        drop((watcher, watcher_cleanup));
        let callback = Arc::new(RuntimeSlotProbe {
            state: Arc::clone(&state),
            original: Mutex::new(Some(reentry)),
            wakes: AtomicUsize::new(0),
            acquired: AtomicUsize::new(0),
            busy: AtomicUsize::new(0),
            other: AtomicUsize::new(0),
        });
        let mut wait = observation.wait_for_release();
        let waker = Waker::from(Arc::clone(&callback));
        assert!(
            Pin::new(&mut wait)
                .poll(&mut Context::from_waker(&waker))
                .is_pending()
        );
        let original = slot.recover_original();
        assert_eq!(original_topology_allocation(&original), pointer);
        assert_eq!(
            callback.wakes.load(Ordering::SeqCst),
            0,
            "actual releases remain caller-owned"
        );
        let cleanup = slot.into_cleanup();
        assert_eq!(
            callback.wakes.load(Ordering::SeqCst),
            0,
            "cleanup transfer cannot signal"
        );
        drop(late_busy);
        assert_eq!(
            callback.wakes.load(Ordering::SeqCst),
            0,
            "unrelated source cannot signal earlier runtime"
        );
        drop(cleanup);
        assert_eq!(callback.wakes.load(Ordering::SeqCst), 1);
        assert_eq!(callback.acquired.load(Ordering::SeqCst), 4);
        assert_eq!(callback.busy.load(Ordering::SeqCst), 0);
        assert_eq!(callback.other.load(Ordering::SeqCst), 0);
        assert!(
            Pin::new(&mut wait)
                .poll(&mut Context::from_waker(Waker::noop()))
                .is_ready()
        );
        assert_eq!(images(&state), before);
        let mut retry = original.publication_slot(&state);
        retry.try_prepare(|_, _| Ok::<_, ()>(())).unwrap();
        let (original, cleanup) = retry.into_prepared().abort();
        assert_eq!(original_topology_allocation(&original), pointer);
        drop(cleanup);
        prepare(original, &state).publish();
        assert_ne!(images(&state), before);
    }
}

#[test]
fn runtime_publication_slot_admission_unwind_is_caller_retained_and_never_recovers_authority() {
    let state = fixture();
    let before = images(&state);
    let dropped = Arc::new(AtomicBool::new(false));
    struct Retained(Arc<AtomicBool>);
    impl Drop for Retained {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
        }
    }
    let original = capture(
        blocks(&state, BlockMode::Ordinary),
        Retained(Arc::clone(&dropped)),
    );
    let mut slot = original.publication_slot::<()>(&state);
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = slot.try_prepare(|_, _| -> Result<(), ()> {
                panic!("actual outer admission unwind");
            });
        }))
        .is_err()
    );
    assert!(
        !dropped.load(Ordering::SeqCst),
        "callee cannot consume original captured admission"
    );
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| slot.recover_original())).is_err()
    );
    slot.release_writers();
    assert!(
        !dropped.load(Ordering::SeqCst),
        "terminal release retains cleanup"
    );
    drop(slot);
    assert!(dropped.load(Ordering::SeqCst));
    assert_eq!(images(&state), before);
    assert_writers_released_except(&state, None);
}
