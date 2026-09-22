//! Capture keeps original first-runtime notification behind the final runtime writer.

use super::*;
use mv::PublicationCleanup;
use std::sync::Mutex;

struct ContextProbe {
    journal: Option<mv::cell::Detached<LaneConsensusContextsV1, ()>>,
    cleanup: Option<PublicationCleanup<()>>,
}

struct ProbeContextsOnRuntimeRelease {
    state: Arc<State>,
    original: Mutex<ContextProbe>,
    calls: AtomicUsize,
    admitted: AtomicUsize,
    acquired: AtomicUsize,
    busy: AtomicUsize,
    other: AtomicUsize,
    unavailable: AtomicUsize,
}

impl Wake for ProbeContextsOnRuntimeRelease {
    fn wake(self: Arc<Self>) {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let Ok(mut original) = self.original.try_lock() else {
            self.unavailable.fetch_add(1, Ordering::SeqCst);
            return;
        };
        if original.cleanup.is_some() {
            self.unavailable.fetch_add(1, Ordering::SeqCst);
            return;
        }
        let Some(journal) = original.journal.take() else {
            self.unavailable.fetch_add(1, Ordering::SeqCst);
            return;
        };
        match journal.try_prepare_publication(&self.state.lane_consensus_contexts, |_, _| {
            self.admitted.fetch_add(1, Ordering::SeqCst);
            Ok::<_, ()>(())
        }) {
            Ok(prepared) => {
                self.acquired.fetch_add(1, Ordering::SeqCst);
                let (journal, cleanup) = prepared.abort();
                original.journal = Some(journal);
                original.cleanup = Some(cleanup);
            }
            Err((journal, error, cleanup)) => {
                if matches!(error, PublicationPreparationError::Busy(_)) {
                    self.busy.fetch_add(1, Ordering::SeqCst);
                } else {
                    self.other.fetch_add(1, Ordering::SeqCst);
                }
                original.journal = Some(journal);
                original.cleanup = Some(cleanup);
            }
        }
    }
}

fn runtime_capture_releases_last_before_first(mode: BlockMode) {
    // The existing fixture commits a real populated four-cell tip, so Replace
    // actually consumes retained preimages rather than an all-empty shortcut.
    let state: Arc<State> = Arc::from(fixture());
    let before = images(&state);
    let runtime_probe = state
        .canonical_runtime
        .block()
        .try_detach(|_| Ok::<_, ()>(()))
        .unwrap();
    let contexts_probe = state
        .lane_consensus_contexts
        .block()
        .try_detach(|_| Ok::<_, ()>(()))
        .unwrap();
    let mut original = blocks(&state, mode);
    mutate(&mut original, 2);
    let runtime_ptr = std::ptr::from_ref(original.0.get());
    let contexts_ptr = std::ptr::from_ref(original.3.get());
    let admitted = AtomicUsize::new(0);
    let (runtime_probe, error, refused) = runtime_probe
        .try_prepare_publication(&state.canonical_runtime, |_, _| {
            admitted.fetch_add(1, Ordering::SeqCst);
            Ok::<_, ()>(())
        })
        .err()
        .expect("the real runtime block owns the original first writer");
    assert_eq!(admitted.load(Ordering::SeqCst), 1);
    let PublicationPreparationError::Busy(observation) = error else {
        panic!("expected original first-runtime physical contention");
    };
    assert!(!observation.is_poisoned());
    let callback = Arc::new(ProbeContextsOnRuntimeRelease {
        state: Arc::clone(&state),
        original: Mutex::new(ContextProbe {
            journal: Some(contexts_probe),
            cleanup: None,
        }),
        calls: AtomicUsize::new(0),
        admitted: AtomicUsize::new(0),
        acquired: AtomicUsize::new(0),
        busy: AtomicUsize::new(0),
        other: AtomicUsize::new(0),
        unavailable: AtomicUsize::new(0),
    });
    let waker = Waker::from(Arc::clone(&callback));
    let mut poll = Context::from_waker(&waker);
    let mut released = observation.clone().wait_for_release();
    assert!(Pin::new(&mut released).poll(&mut poll).is_pending());
    assert_eq!(callback.calls.load(Ordering::SeqCst), 0);

    let captured = capture(original, ());
    drop(refused);
    let (contexts_probe, cleanup) = {
        let mut original = callback.original.lock().unwrap();
        (original.journal.take().unwrap(), original.cleanup.take())
    };
    drop(cleanup);
    assert!(Pin::new(&mut released).poll(&mut poll).is_ready());
    assert!(!observation.is_poisoned());
    assert_eq!(callback.calls.load(Ordering::SeqCst), 1);
    assert_eq!(callback.admitted.load(Ordering::SeqCst), 1);
    assert_eq!(callback.busy.load(Ordering::SeqCst), 0);
    assert_eq!(callback.other.load(Ordering::SeqCst), 0);
    assert_eq!(callback.unavailable.load(Ordering::SeqCst), 0);
    assert_eq!(callback.acquired.load(Ordering::SeqCst), 1);
    assert!(runtime_probe.matches_current(&state.canonical_runtime));
    assert!(contexts_probe.matches_current(&state.lane_consensus_contexts));
    assert!(captured.matches_current(&state));
    assert_eq!(captured.canonical_runtime.mode(), mode);
    assert_eq!(captured.commit_topology.mode(), mode);
    assert_eq!(captured.prev_commit_topology.mode(), mode);
    assert_eq!(captured.lane_consensus_contexts.mode(), mode);
    assert_eq!(
        std::ptr::from_ref(captured.canonical_runtime.touched_value().unwrap().after),
        runtime_ptr
    );
    assert_eq!(
        std::ptr::from_ref(
            captured
                .lane_consensus_contexts
                .touched_value()
                .unwrap()
                .after
        ),
        contexts_ptr
    );
    assert_eq!(images(&state), before);
    drop((captured, runtime_probe, contexts_probe));
}

#[test]
fn ordinary_runtime_capture_unlocks_contexts_before_runtime_notification() {
    runtime_capture_releases_last_before_first(BlockMode::Ordinary);
}

#[test]
fn replacement_runtime_capture_unlocks_contexts_before_runtime_notification() {
    runtime_capture_releases_last_before_first(BlockMode::Replace);
}
