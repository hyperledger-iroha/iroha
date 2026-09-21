//! Original World capture must release every writer before original notifications.

use crate::{Peers, state::World};
use mv::{PublicationCleanup, PublicationPreparationError, cell::Detached};
use std::{
    future::Future,
    pin::Pin,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    task::{Context, Wake, Waker},
};

struct PeerProbeSlot {
    journal: Option<Detached<Peers, ()>>,
    // A callback must not retire its own probe under the still-dropping World.
    cleanup: Option<PublicationCleanup<()>>,
}

struct ProbePeersOnParametersRelease {
    world: Arc<World>,
    slot: Mutex<PeerProbeSlot>,
    calls: AtomicUsize,
    admitted: AtomicUsize,
    acquired: AtomicUsize,
    busy: AtomicUsize,
    poisoned: AtomicUsize,
    changed: AtomicUsize,
    admission_refused: AtomicUsize,
    unavailable_slot: AtomicUsize,
}

impl Wake for ProbePeersOnParametersRelease {
    fn wake(self: Arc<Self>) {
        // All observations are nonblocking and assertions run after World drop.
        // In particular, do not call Cell::block(), clone a payload, synthesize
        // a notification, or assert from a callback during owner cleanup.
        self.calls.fetch_add(1, Ordering::SeqCst);
        let Ok(mut slot) = self.slot.try_lock() else {
            self.unavailable_slot.fetch_add(1, Ordering::SeqCst);
            return;
        };
        if slot.cleanup.is_some() {
            self.unavailable_slot.fetch_add(1, Ordering::SeqCst);
            return;
        }
        let Some(journal) = slot.journal.take() else {
            self.unavailable_slot.fetch_add(1, Ordering::SeqCst);
            return;
        };
        match journal.try_prepare_publication(&self.world.peers, |_, _| {
            // This runs only AFTER the exact original publication identity has
            // been checked and its mutex released. A subsequent Busy therefore
            // concerns an actual writer, not that preliminary identity check.
            self.admitted.fetch_add(1, Ordering::SeqCst);
            Ok::<_, ()>(())
        }) {
            Ok(prepared) => {
                self.acquired.fetch_add(1, Ordering::SeqCst);
                // Actual attachment of both original native writers is the
                // healthy-unlocked witness. Return their same journal, retaining
                // deferred signals until the enclosing World has finished drop.
                let (journal, cleanup) = prepared.abort();
                slot.journal = Some(journal);
                slot.cleanup = Some(cleanup);
            }
            Err((journal, error, cleanup)) => {
                match error {
                    PublicationPreparationError::Busy(_) => {
                        self.busy.fetch_add(1, Ordering::SeqCst);
                    }
                    PublicationPreparationError::Poisoned => {
                        self.poisoned.fetch_add(1, Ordering::SeqCst);
                    }
                    PublicationPreparationError::Changed => {
                        self.changed.fetch_add(1, Ordering::SeqCst);
                    }
                    PublicationPreparationError::Admission(()) => {
                        self.admission_refused.fetch_add(1, Ordering::SeqCst);
                    }
                }
                slot.journal = Some(journal);
                slot.cleanup = Some(cleanup);
            }
        }
    }
}

#[derive(Clone, Copy)]
enum CaptureAttempt {
    Success,
    Refusal,
    AdmissionPanic,
}

fn complete_world_capture_releases_peers_before_parameters_callback(
    replacement: bool,
    attempt: CaptureAttempt,
) {
    let world = Arc::new(World::default());
    // Seed genuine retained preimages, even though these two values are equal.
    // The replacement constructor must execute its real Some(preimage) path.
    {
        let mut tip = world.parameters.block();
        let _ = tip.get_mut();
        tip.commit();
    }
    {
        let mut tip = world.peers.block();
        let _ = tip.get_mut();
        tip.commit();
    }
    assert!(world.parameters.predecessor_view().is_some());
    assert!(world.peers.predecessor_view().is_some());
    let parameters_before = world.parameters.view();
    let peers_before = world.peers.view();
    let undo_parameters_before = world.parameters.predecessor_view();
    let undo_peers_before = world.peers.predecessor_view();

    // These are the original detached generations, created before any World
    // writer is held. The callback never constructs a replacement generation.
    let parameters_probe = world
        .parameters
        .block()
        .try_detach(|_| Ok::<_, ()>(()))
        .expect("detach the original parameters probe");
    let peers_probe = world
        .peers
        .block()
        .try_detach(|_| Ok::<_, ()>(()))
        .expect("detach the original peers probe");

    let block = if replacement {
        world.block_and_revert()
    } else {
        world.block()
    };
    let parameters_admitted = AtomicUsize::new(0);
    let (parameters_probe, error, parameters_cleanup) = parameters_probe
        .try_prepare_publication(&world.parameters, |_, _| {
            parameters_admitted.fetch_add(1, Ordering::SeqCst);
            Ok::<_, ()>(())
        })
        .err()
        .expect("the actual World owns parameters writers");
    assert_eq!(parameters_admitted.load(Ordering::SeqCst), 1);
    let PublicationPreparationError::Busy(observation) = error else {
        panic!("healthy original parameters writer must be physically Busy");
    };
    assert!(!observation.is_poisoned());
    let callback = Arc::new(ProbePeersOnParametersRelease {
        world: Arc::clone(&world),
        slot: Mutex::new(PeerProbeSlot {
            journal: Some(peers_probe),
            cleanup: None,
        }),
        calls: AtomicUsize::new(0),
        admitted: AtomicUsize::new(0),
        acquired: AtomicUsize::new(0),
        busy: AtomicUsize::new(0),
        poisoned: AtomicUsize::new(0),
        changed: AtomicUsize::new(0),
        admission_refused: AtomicUsize::new(0),
        unavailable_slot: AtomicUsize::new(0),
    });
    let waker = Waker::from(Arc::clone(&callback));
    let mut context = Context::from_waker(&waker);
    let mut released = observation.clone().wait_for_release();
    assert!(Pin::new(&mut released).poll(&mut context).is_pending());
    assert_eq!(callback.calls.load(Ordering::SeqCst), 0);

    let detached = match attempt {
        CaptureAttempt::Success => Some(
            block
                .try_detach_journals(|_| Ok::<_, ()>(()))
                .expect("capture every original World journal"),
        ),
        CaptureAttempt::Refusal => {
            let result = block.try_detach_journals(|_| Err::<(), _>("admission"));
            assert!(matches!(
                result,
                Err(crate::state::world_journals::CaptureError::Admission(
                    "admission"
                ))
            ));
            None
        }
        CaptureAttempt::AdmissionPanic => {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                block.try_detach_journals(|_| -> Result<(), ()> {
                    panic!("original World admission");
                })
            }));
            assert!(result.is_err());
            None
        }
    };
    if let Some(detached) = &detached {
        assert!(detached.matches_current(&world));
    }
    let panicked = matches!(attempt, CaptureAttempt::AdmissionPanic);

    // The original refusal cleanup and callback-created cleanup are not retired
    // until every World field has finished dropping. No assertion in Wake can
    // turn an ordering observation into a second panic during cleanup.
    drop(parameters_cleanup);
    let (peers_probe, peers_cleanup) = {
        let mut slot = callback.slot.lock().expect("the recorder never panics");
        (slot.journal.take(), slot.cleanup.take())
    };
    drop(peers_cleanup);
    assert!(Pin::new(&mut released).poll(&mut context).is_ready());
    assert_eq!(observation.is_poisoned(), panicked);
    assert_eq!(callback.calls.load(Ordering::SeqCst), 1);
    assert_eq!(callback.unavailable_slot.load(Ordering::SeqCst), 0);
    assert_eq!(callback.admitted.load(Ordering::SeqCst), 1);
    assert_eq!(
        callback.poisoned.load(Ordering::SeqCst),
        usize::from(panicked)
    );
    assert_eq!(callback.changed.load(Ordering::SeqCst), 0);
    assert_eq!(callback.admission_refused.load(Ordering::SeqCst), 0);
    assert_eq!(world.parameters.view().get(), parameters_before.get());
    assert_eq!(world.peers.view().get(), peers_before.get());
    assert_eq!(
        world.parameters.predecessor_view().get(),
        undo_parameters_before.get(),
    );
    assert_eq!(
        world.peers.predecessor_view().get(),
        undo_peers_before.get(),
    );
    assert!(parameters_probe.matches_current(&world.parameters));
    assert!(
        peers_probe
            .as_ref()
            .expect("the same probe is retained on either outcome")
            .matches_current(&world.peers),
    );
    assert_eq!(
        callback.busy.load(Ordering::SeqCst),
        0,
        "World capture notified parameters while a peers writer was still held",
    );
    assert_eq!(
        callback.acquired.load(Ordering::SeqCst),
        usize::from(!panicked),
        "the same peers journal must attach both physical writers during Wake",
    );
}

#[test]
fn ordinary_world_capture_unlocks_peers_before_parameters_notification() {
    complete_world_capture_releases_peers_before_parameters_callback(
        false,
        CaptureAttempt::Success,
    );
}

#[test]
fn replacement_world_capture_unlocks_peers_before_parameters_notification() {
    complete_world_capture_releases_peers_before_parameters_callback(true, CaptureAttempt::Success);
}

#[test]
fn refused_world_capture_releases_all_writers_before_original_notifications() {
    for replacement in [false, true] {
        complete_world_capture_releases_peers_before_parameters_callback(
            replacement,
            CaptureAttempt::Refusal,
        );
    }
}

#[test]
fn panicked_world_capture_releases_all_writers_and_preserves_native_poison() {
    for replacement in [false, true] {
        complete_world_capture_releases_peers_before_parameters_callback(
            replacement,
            CaptureAttempt::AdmissionPanic,
        );
    }
}
