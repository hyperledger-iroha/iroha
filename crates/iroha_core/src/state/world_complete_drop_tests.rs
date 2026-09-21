//! Original World field callbacks must observe all sibling writers released.

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

fn complete_world_drop_releases_peers_before_parameters_callback(
    replacement: bool,
    explicit: bool,
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

    let mut block = if replacement {
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

    if explicit {
        mv::BlockRetirement::release_writers(&mut block);
        assert_eq!(callback.calls.load(Ordering::SeqCst), 0);
        assert!(Pin::new(&mut released).poll(&mut context).is_pending());
    }
    drop(block);

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
    assert!(!observation.is_poisoned());
    assert_eq!(callback.calls.load(Ordering::SeqCst), 1);
    assert_eq!(callback.unavailable_slot.load(Ordering::SeqCst), 0);
    assert_eq!(callback.admitted.load(Ordering::SeqCst), 1);
    assert_eq!(callback.poisoned.load(Ordering::SeqCst), 0);
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
        "the parameters release callback ran while a peers writer was still held",
    );
    assert_eq!(
        callback.acquired.load(Ordering::SeqCst),
        1,
        "the same peers journal must attach both physical writers during Wake",
    );
}

#[test]
fn ordinary_world_drop_unlocks_peers_before_parameters_notification() {
    complete_world_drop_releases_peers_before_parameters_callback(false, false);
}

#[test]
fn replacement_world_drop_unlocks_peers_before_parameters_notification() {
    complete_world_drop_releases_peers_before_parameters_callback(true, false);
}

#[test]
fn ordinary_world_explicit_retirement_defers_original_notifications() {
    complete_world_drop_releases_peers_before_parameters_callback(false, true);
}

#[test]
fn replacement_world_explicit_retirement_defers_original_notifications() {
    complete_world_drop_releases_peers_before_parameters_callback(true, true);
}

#[test]
fn world_block_owner_preserves_canonical_json_and_checked_writer() {
    use norito::json;

    let world = World::default();
    let block = world.block();
    let fields = <crate::state::WorldBlock<'_> as json::FastJsonWrite>::json_object_field_order()
        .expect("one canonical World block schema");
    let encoded = json::to_json(&block).expect("serialize original World journals");
    let json::Value::Object(object) = json::from_str::<json::Value>(&encoded).unwrap() else {
        panic!("World block remains a canonical field object");
    };
    assert_eq!(fields.first(), Some(&"parameters"));
    assert!(fields.contains(&"accounts"));
    assert_eq!(object.len(), fields.len());
    assert!(fields.iter().all(|field| object.contains_key(*field)));
    assert_eq!(
        json::to_json_bounded(&block, 0),
        Err(json::BoundedJsonError::BodyTooLarge),
    );
    assert_eq!(
        json::to_json_bounded(&block, encoded.len()),
        json::to_json_bounded(&*block, encoded.len()),
    );
}
