//! Original executing Cell pairs release jointly before native callbacks.

use super::*;
use std::{
    future::Future,
    panic::{AssertUnwindSafe, catch_unwind},
    pin::Pin,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering::SeqCst},
    },
    task::{Context, Wake, Waker},
};

struct Probe<V: Value> {
    cell: Arc<Cell<V>>,
    calls: AtomicUsize,
    saw_held_writer: AtomicBool,
    panic_once: AtomicBool,
}

impl<V: Value> Wake for Probe<V> {
    fn wake(self: Arc<Self>) {
        let undo_released =
            self.cell.revert.is_poisoned() || self.cell.revert.try_write().is_some();
        let current_released =
            self.cell.blocks.is_poisoned() || self.cell.blocks.try_write().is_some();
        self.saw_held_writer
            .fetch_or(!undo_released || !current_released, SeqCst);
        self.calls.fetch_add(1, SeqCst);
        assert!(!self.panic_once.swap(false, SeqCst), "original wake panic");
    }
}

fn arm<V: Value>(
    cell: &Arc<Cell<V>>,
    panic_once: bool,
) -> (Arc<Probe<V>>, [concread::release::ReleaseFuture; 2]) {
    let probe = Arc::new(Probe {
        cell: Arc::clone(cell),
        calls: AtomicUsize::new(0),
        saw_held_writer: AtomicBool::new(false),
        panic_once: AtomicBool::new(panic_once),
    });
    let mut waits = [
        cell.revert_released.observe().wait_for_release(),
        cell.blocks_released.observe().wait_for_release(),
    ];
    let waker = Waker::from(Arc::clone(&probe));
    for wait in &mut waits {
        assert!(
            Pin::new(wait)
                .poll(&mut Context::from_waker(&waker))
                .is_pending()
        );
    }
    (probe, waits)
}

fn assert_released<V: Value>(
    probe: &Arc<Probe<V>>,
    mut waits: [concread::release::ReleaseFuture; 2],
) {
    assert_eq!(probe.calls.load(SeqCst), 2);
    assert!(!probe.saw_held_writer.load(SeqCst));
    let waker = Waker::from(Arc::clone(probe));
    for wait in &mut waits {
        assert!(
            Pin::new(wait)
                .poll(&mut Context::from_waker(&waker))
                .is_ready()
        );
    }
    assert_eq!(
        (
            probe.cell.revert_released.observe().is_poisoned(),
            probe.cell.blocks_released.observe().is_poisoned(),
        ),
        (
            probe.cell.revert.is_poisoned(),
            probe.cell.blocks.is_poisoned()
        ),
        "native observations retain the actual physical poison verdicts",
    );
}

fn seeded() -> Arc<Cell<u64>> {
    let cell = Arc::new(Cell::new(10));
    let mut tip = cell.block();
    *tip.get_mut() = 20;
    tip.commit();
    cell
}

#[test]
fn ordinary_replacement_and_same_cut_abandonment_release_the_original_pair() {
    for mode in 0..3 {
        for fault in 0..3 {
            let cell = seeded();
            let predecessor = cell.publication.capture();
            let original = match mode {
                0 => (Some(cell.block()), None),
                1 => (Some(cell.block_and_revert()), None),
                _ => (None, Some(cell.current_replacement())),
            };
            let (probe, waits) = arm(&cell, fault == 2);
            let result = catch_unwind(AssertUnwindSafe(|| {
                let original = original;
                assert!(fault != 1, "original caller panic");
                drop(original);
            }));
            assert_eq!(result.is_err(), fault != 0);
            if let Err(error) = result {
                assert_eq!(
                    error.downcast_ref::<&str>(),
                    Some(&if fault == 1 {
                        "original caller panic"
                    } else {
                        "original wake panic"
                    }),
                );
            }
            assert_released(&probe, waits);
            let expected_poison = fault == 1;
            assert_eq!(cell.revert.is_poisoned(), expected_poison);
            assert_eq!(cell.blocks.is_poisoned(), expected_poison);
            assert_eq!(*cell.view(), 20);
            assert_eq!(*cell.predecessor_view(), Some(10));
            assert_eq!(
                predecessor.try_check_current::<()>(&cell.publication).0,
                Ok(())
            );
            if !expected_poison {
                drop(cell.block());
            }
        }
    }
}

#[test]
fn capture_refusal_and_admission_unwind_release_both_original_writers() {
    for replacement in [false, true] {
        for panic in [false, true] {
            let cell = seeded();
            let block = if replacement {
                cell.block_and_revert()
            } else {
                cell.block()
            };
            let original_identity = block.publication_identity();
            let (probe, waits) = arm(&cell, false);
            let result = catch_unwind(AssertUnwindSafe(|| {
                block.try_detach(|original| {
                    assert_eq!(original.publication_identity(), original_identity);
                    assert!(!panic, "original admission panic");
                    Err::<(), _>(73_u8)
                })
            }));
            if panic {
                assert_eq!(
                    result.err().unwrap().downcast_ref::<&str>(),
                    Some(&"original admission panic")
                );
            } else {
                assert!(matches!(result, Ok(Err(73))));
            }
            assert_released(&probe, waits);
            assert_eq!(cell.revert.is_poisoned(), panic);
            assert_eq!(cell.blocks.is_poisoned(), panic);
            assert_eq!(*cell.view(), 20);
            assert_eq!(*cell.predecessor_view(), Some(10));
        }
    }
}

#[test]
fn capture_moves_exact_current_and_undo_allocations_before_notifying_either_writer() {
    for replacement in [false, true] {
        let cell = Arc::new(Cell::new(vec![10_u64, 11]));
        let mut tip = cell.block();
        *tip.get_mut() = vec![20, 21];
        tip.commit();
        let mut block = if replacement {
            cell.block_and_revert()
        } else {
            cell.block()
        };
        *block.get_mut() = vec![30, 31];
        let before = block.get_before_block().as_ptr();
        let after = block.get().as_ptr();
        let (probe, waits) = arm(&cell, false);
        let journal = block.try_detach(|_| Ok::<_, ()>(())).unwrap();
        assert_released(&probe, waits);
        let touched = journal.touched_value().unwrap();
        assert_eq!(touched.before.as_ptr(), before);
        assert_eq!(touched.after.as_ptr(), after);
        assert_eq!(
            journal.mode(),
            if replacement {
                BlockMode::Replace
            } else {
                BlockMode::Ordinary
            }
        );
        assert!(journal.matches_current(&cell));
        assert_eq!(cell.view().get(), &vec![20, 21]);
        assert_eq!(cell.predecessor_view().get(), &Some(vec![10, 11]));
        let prepared = journal.try_prepare_publication(&cell, |_, _| Ok::<_, ()>(()));
        let prepared = prepared.unwrap_or_else(|_| panic!("same original cell must remain usable"));
        let (journal, cleanup) = prepared.abort();
        drop(cleanup);
        let touched = journal.touched_value().unwrap();
        assert_eq!(touched.before.as_ptr(), before);
        assert_eq!(touched.after.as_ptr(), after);
    }
}

#[derive(Clone)]
struct Payload {
    id: usize,
    fault: Arc<AtomicUsize>,
}

impl Drop for Payload {
    fn drop(&mut self) {
        assert!(
            self.fault
                .compare_exchange(self.id, 0, SeqCst, SeqCst)
                .is_err(),
            "original payload panic",
        );
    }
}

#[test]
fn private_payload_unwind_after_unlock_keeps_both_writers_healthy_before_native_wakes() {
    for fault_id in [1, 2] {
        let fault = Arc::new(AtomicUsize::new(0));
        let cell = Arc::new(Cell::new(Payload {
            id: 1,
            fault: Arc::clone(&fault),
        }));
        let predecessor = cell.publication.capture();
        let mut block = cell.block();
        block.get_mut().id = 2;
        let (probe, waits) = arm(&cell, false);
        fault.store(fault_id, SeqCst);
        let result = catch_unwind(AssertUnwindSafe(|| drop(block)));
        assert_eq!(
            result.err().unwrap().downcast_ref::<&str>(),
            Some(&"original payload panic")
        );
        assert_eq!(fault.load(SeqCst), 0);
        assert_released(&probe, waits);
        // Private payload destruction follows physical release, so its panic
        // cannot poison either already-unlocked writer.
        assert_eq!(
            (cell.revert.is_poisoned(), cell.blocks.is_poisoned()),
            (false, false)
        );
        assert_eq!(cell.view().id, 1);
        assert!(cell.predecessor_view().is_none());
        assert_eq!(
            predecessor.try_check_current::<()>(&cell.publication).0,
            Ok(())
        );
    }
}

#[test]
fn reset_revert_and_same_cut_payload_failure_keep_joint_custody_before_return() {
    for operation in 0..3 {
        let fault = Arc::new(AtomicUsize::new(0));
        let cell = Arc::new(Cell::new(Payload {
            id: 1,
            fault: Arc::clone(&fault),
        }));
        let mut tip = cell.block();
        tip.get_mut().id = 2;
        tip.commit();
        let predecessor = cell.publication.capture();
        // For the same-cut operation acquire first. Other modes exercise the
        // original reset/revert tail before its Block wrapper is returned.
        let replacement = (operation == 2).then(|| cell.current_replacement());
        let (probe, waits) = arm(&cell, false);
        fault.store(if operation == 0 { 1 } else { 2 }, SeqCst);
        let result = catch_unwind(AssertUnwindSafe(|| match operation {
            0 => drop(cell.block()),
            1 => drop(cell.block_and_revert()),
            _ => replacement.unwrap().publish(Payload {
                id: 3,
                fault: Arc::clone(&fault),
            }),
        }));
        assert_eq!(
            result.err().unwrap().downcast_ref::<&str>(),
            Some(&"original payload panic")
        );
        assert_eq!(fault.load(SeqCst), 0);
        assert_released(&probe, waits);
        assert!(cell.revert.is_poisoned());
        assert!(cell.blocks.is_poisoned());
        assert_eq!(cell.view().id, 2);
        assert_eq!(cell.predecessor_view().as_ref().unwrap().id, 1);
        assert_eq!(
            predecessor.try_check_current::<()>(&cell.publication).0,
            Ok(())
        );
    }
}

#[test]
fn paired_writer_codec_borrows_the_same_original_undo_and_current_values() {
    let cell = Cell::new(10_u64);
    let mut block = cell.block();
    assert_eq!((block.original_undo(), block.get()), (&None, &10));
    assert_eq!(
        norito::json::to_json(&block).unwrap(),
        r#"{"revert":null,"blocks":10}"#
    );
    *block.get_mut() = 20;
    assert_eq!((block.original_undo(), block.get()), (&Some(10), &20));
    assert_eq!(
        norito::json::to_json(&block).unwrap(),
        r#"{"revert":10,"blocks":20}"#
    );
    block.commit();
    let replacement = cell.block_and_revert();
    assert_eq!(
        (replacement.original_undo(), replacement.get()),
        (&None, &10)
    );
    assert_eq!(
        norito::json::to_json(&replacement).unwrap(),
        r#"{"revert":null,"blocks":10}"#
    );
    drop(replacement);
    assert_eq!(*cell.view(), 20);
    assert_eq!(*cell.predecessor_view(), Some(10));
}
