//! Actual trigger cleanup defers notifications through every sibling writer.

use super::*;
use mv::{PublicationCleanup, PublicationPreparationError};
use std::{
    future::Future,
    pin::Pin,
    sync::Mutex,
    task::{Context, Wake, Waker},
};

struct ActiveProbe {
    original: Option<DetachedStorage<TriggerId, (), ()>>,
    cleanup: Option<PublicationCleanup<()>>,
}

struct ProbeActiveOnIdsRelease {
    set: Arc<Set>,
    probe: Mutex<ActiveProbe>,
    calls: AtomicUsize,
    acquired: AtomicUsize,
    failed: AtomicUsize,
}

impl Wake for ProbeActiveOnIdsRelease {
    fn wake(self: Arc<Self>) {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let Ok(mut probe) = self.probe.try_lock() else {
            self.failed.fetch_add(1, Ordering::SeqCst);
            return;
        };
        if probe.cleanup.is_some() {
            self.failed.fetch_add(1, Ordering::SeqCst);
            return;
        }
        let Some(original) = probe.original.take() else {
            self.failed.fetch_add(1, Ordering::SeqCst);
            return;
        };
        match original.try_prepare_publication(&self.set.active_data_trigger_ids, |_, _| {
            Ok::<_, ()>(())
        }) {
            Ok(prepared) => {
                self.acquired.fetch_add(1, Ordering::SeqCst);
                let (original, cleanup) = prepared.abort();
                probe.original = Some(original);
                probe.cleanup = Some(cleanup);
            }
            Err((original, _, cleanup)) => {
                self.failed.fetch_add(1, Ordering::SeqCst);
                probe.original = Some(original);
                probe.cleanup = Some(cleanup);
            }
        }
    }
}

fn complete_trigger_drop_releases_siblings(replacement: bool) {
    let set = seeded_set();
    let before = images(&set);
    let ids = set.ids.block().try_detach(|_| Ok::<_, ()>(())).unwrap();
    let active = set
        .active_data_trigger_ids
        .block()
        .try_detach(|_| Ok::<_, ()>(()))
        .unwrap();
    let block = if replacement {
        set.block_and_revert()
    } else {
        set.block()
    };
    let admitted = AtomicUsize::new(0);
    let (ids, error, refused) = ids
        .try_prepare_publication(&set.ids, |_, _| {
            admitted.fetch_add(1, Ordering::SeqCst);
            Ok::<_, ()>(())
        })
        .err()
        .expect("the complete trigger set holds its ids writers");
    assert_eq!(admitted.load(Ordering::SeqCst), 1);
    let PublicationPreparationError::Busy(observation) = error else {
        panic!("an actual healthy writer must be Busy");
    };
    let callback = Arc::new(ProbeActiveOnIdsRelease {
        set: Arc::clone(&set),
        probe: Mutex::new(ActiveProbe {
            original: Some(active),
            cleanup: None,
        }),
        calls: AtomicUsize::new(0),
        acquired: AtomicUsize::new(0),
        failed: AtomicUsize::new(0),
    });
    let waker = Waker::from(Arc::clone(&callback));
    let mut context = Context::from_waker(&waker);
    let mut released = observation.wait_for_release();
    assert!(Pin::new(&mut released).poll(&mut context).is_pending());
    drop(block);
    drop(refused);
    let (active, cleanup) = {
        let mut probe = callback.probe.lock().unwrap();
        (probe.original.take(), probe.cleanup.take())
    };
    drop(cleanup);
    assert!(Pin::new(&mut released).poll(&mut context).is_ready());
    assert_eq!(callback.calls.load(Ordering::SeqCst), 1);
    assert_eq!(callback.failed.load(Ordering::SeqCst), 0);
    assert_eq!(callback.acquired.load(Ordering::SeqCst), 1);
    assert!(ids.matches_current(&set.ids));
    assert!(active.unwrap().matches_current(&set.active_data_trigger_ids));
    assert_eq!(images(&set), before);
}

#[test]
fn ordinary_trigger_drop_unlocks_active_index_before_ids_notification() {
    complete_trigger_drop_releases_siblings(false);
}

#[test]
fn replacement_trigger_drop_unlocks_active_index_before_ids_notification() {
    complete_trigger_drop_releases_siblings(true);
}
