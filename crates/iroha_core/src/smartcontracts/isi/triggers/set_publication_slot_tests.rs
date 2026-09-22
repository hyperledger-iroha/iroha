//! Caller-owned ten-map trigger preparation retains real late-refusal cleanup.

use super::*;
use std::{
    future::Future,
    pin::Pin,
    sync::Mutex,
    task::{Context, Wake, Waker},
};

struct SetSlotProbe {
    target: Arc<Set>,
    original: Mutex<Option<DetachedSet<()>>>,
    wakes: AtomicUsize,
    acquired: AtomicUsize,
    busy: AtomicUsize,
    other: AtomicUsize,
}
impl Wake for SetSlotProbe {
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
        macro_rules! probe { ($($field:ident),+ $(,)?) => { $(
            match original.$field.try_prepare_publication(&self.target.$field, |_, _| Ok::<_, ()>(())) {
                Ok(prepared) => {
                    let (original, cleanup) = prepared.abort();
                    self.acquired.fetch_add(1, Ordering::SeqCst); drop((original, cleanup));
                }
                Err((original, PublicationPreparationError::Busy(_), cleanup)) => {
                    self.busy.fetch_add(1, Ordering::SeqCst); drop((original, cleanup));
                }
                Err((original, _, cleanup)) => {
                    self.other.fetch_add(1, Ordering::SeqCst); drop((original, cleanup));
                }
            }
        )+ }; }
        probe!(
            data_triggers,
            pipeline_triggers,
            time_triggers,
            by_call_triggers,
            ids,
            active_data_trigger_ids,
            active_pipeline_trigger_ids,
            active_time_trigger_ids,
            active_by_call_trigger_ids,
            contracts
        );
    }
}

#[test]
fn trigger_publication_slot_retains_late_refusal_cleanup_and_exact_retry_in_both_modes() {
    for replacement in [false, true] {
        let target = seeded_set();
        let before = images(&target);
        let block = || {
            if replacement {
                target.block_and_revert()
            } else {
                target.block()
            }
        };
        let watcher = capture(block());
        let reentry = capture(block());
        let mut candidate = block();
        // Replacement restores the pre-seeding state; register a real trigger
        // in that image rather than asking mutate_all to find discarded entries.
        if replacement {
            let mut tx = candidate.transaction();
            register_call(&mut tx, "replacement_slot");
            tx.apply();
        } else {
            mutate_all(&mut candidate);
        }
        let original = capture(candidate);
        let pointer = contract_touch_pointer(&original);
        let late_busy = target.contracts.block();
        let mut slot = original.publication_slot(&target);
        assert!(matches!(
            slot.try_prepare(|_, _| Ok::<_, ()>(())),
            Err(SetPublicationError::Component {
                field: "contracts",
                cause: PublicationPreparationError::Busy(_),
            })
        ));
        let (watcher, error, cleanup) = watcher
            .data_triggers
            .try_prepare_publication(&target.data_triggers, |_, _| Ok::<_, ()>(()))
            .err()
            .expect("first original trigger map remains acquired");
        let PublicationPreparationError::Busy(observation) = error else {
            panic!("first original trigger busy");
        };
        drop((watcher, cleanup));
        let callback = Arc::new(SetSlotProbe {
            target: Arc::clone(&target),
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
        assert_eq!(contract_touch_pointer(&original), pointer);
        assert_eq!(callback.wakes.load(Ordering::SeqCst), 0);
        let cleanup = slot.into_cleanup();
        drop(late_busy);
        assert_eq!(callback.wakes.load(Ordering::SeqCst), 0);
        drop(cleanup);
        assert_eq!(callback.wakes.load(Ordering::SeqCst), 1);
        assert_eq!(callback.acquired.load(Ordering::SeqCst), 10);
        assert_eq!(callback.busy.load(Ordering::SeqCst), 0);
        assert_eq!(callback.other.load(Ordering::SeqCst), 0);
        assert!(
            Pin::new(&mut wait)
                .poll(&mut Context::from_waker(Waker::noop()))
                .is_ready()
        );
        assert_eq!(images(&target), before);
        let mut retry = original.publication_slot(&target);
        retry.try_prepare(|_, _| Ok::<_, ()>(())).unwrap();
        let (original, cleanup) = retry.into_prepared().abort();
        assert_eq!(contract_touch_pointer(&original), pointer);
        drop(cleanup);
        prepare_trigger_publication(original, &target).publish();
        assert_ne!(images(&target), before);
    }
}

#[test]
fn trigger_publication_slot_admission_unwind_keeps_original_guard_until_caller_cleanup() {
    let target = seeded_set();
    let before = images(&target);
    let dropped = Arc::new(AtomicBool::new(false));
    let original = target
        .block()
        .try_detach(|_| Ok::<_, ()>(Reservation(Arc::clone(&dropped))))
        .unwrap();
    let mut slot = original.publication_slot::<()>(&target);
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = slot.try_prepare(|_, _| -> Result<(), ()> {
                panic!("actual trigger admission unwind");
            });
        }))
        .is_err()
    );
    assert!(!dropped.load(Ordering::SeqCst));
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| slot.recover_original())).is_err()
    );
    slot.release_writers();
    assert!(!dropped.load(Ordering::SeqCst));
    drop(slot);
    assert!(dropped.load(Ordering::SeqCst));
    assert_eq!(images(&target), before);
    assert_all_writers_released(&target);
}
