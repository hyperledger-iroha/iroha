//! Concrete finite allocation custody through every current/undo publication path.

use super::*;
use crate::allocation::{AllocationBudget, AllocationCharge, AllocationRefusal};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering::SeqCst},
    },
    time::{Duration, Instant},
};

type ChargedCell<V> = Cell<V, AllocationCharge>;

fn pair_bytes<V: Value>() -> usize {
    ChargedCell::<V>::allocation_layouts()
        .into_iter()
        .map(|layout| layout.size())
        .sum()
}

fn charges<V: Value>(
    budget: &AllocationBudget,
) -> Result<CellAllocationCharges<AllocationCharge>, AllocationRefusal> {
    let [current, undo] = ChargedCell::<V>::allocation_layouts();
    let mut reserved = budget.try_reserve_layouts([current, undo])?;
    Ok(CellAllocationCharges::new(
        reserved.try_split(current).unwrap(),
        reserved.try_split(undo).unwrap(),
    ))
}

fn collect_until(budget: &AllocationBudget, expected: usize) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while budget.reserved_bytes() != expected {
        assert!(
            Instant::now() < deadline,
            "retained EBR credits: {} != {expected}",
            budget.reserved_bytes()
        );
        crossbeam_epoch::pin().flush();
        std::thread::yield_now();
    }
}

struct DropMarker(Arc<AtomicBool>);

impl Drop for DropMarker {
    fn drop(&mut self) {
        self.0.store(true, SeqCst);
    }
}

#[test]
fn startup_ordinary_and_revert_charges_follow_all_actual_generations() {
    let pair = pair_bytes::<u64>();
    let budget = AllocationBudget::new(3 * pair);
    let cell = ChargedCell::new_charged(10_u64, charges::<u64>(&budget).unwrap());
    assert_eq!(budget.reserved_bytes(), pair);
    let unrelated = crossbeam_epoch::pin();
    let startup = cell.view();
    let startup_undo = cell.predecessor_view();
    let mut block = cell.block_charged(charges::<u64>(&budget).unwrap());
    *block.get_mut() = 20;
    block.commit();
    assert_eq!(budget.reserved_bytes(), 2 * pair);
    let published = cell.view();
    let published_undo = cell.predecessor_view();
    assert_eq!(*published, 20);
    assert_eq!(*published_undo, Some(10));
    let replacement = cell.block_and_revert_charged(charges::<u64>(&budget).unwrap());
    assert_eq!(*replacement, 10);
    assert_eq!(replacement.mode(), BlockMode::Replace);
    replacement.commit();
    assert_eq!(*cell.view(), 10);
    assert_eq!(*cell.predecessor_view(), None);
    assert_eq!(*startup, 10);
    assert_eq!(*startup_undo, None);
    assert_eq!(*published, 20);
    assert_eq!(*published_undo, Some(10));
    assert_eq!(budget.reserved_bytes(), 3 * pair);
    drop((startup, startup_undo, published, published_undo));
    drop(cell);
    unrelated.flush();
    assert_eq!(
        budget.reserved_bytes(),
        3 * pair,
        "no views remain, but the unrelated pin still owns the epoch"
    );
    drop(unrelated);
    collect_until(&budget, 0);
}

#[test]
fn same_cut_abort_refunds_writers_and_publish_retains_current_with_original_undo() {
    let pair = pair_bytes::<u64>();
    let current_bytes = ChargedCell::<u64>::allocation_layouts()[0].size();
    let budget = AllocationBudget::new(3 * pair);
    let cell = ChargedCell::new_charged(10_u64, charges::<u64>(&budget).unwrap());
    let pinned = crossbeam_epoch::pin();
    let mut tip = cell.block_charged(charges::<u64>(&budget).unwrap());
    *tip.get_mut() = 20;
    tip.commit();
    let predecessor = cell.publication.capture();
    let tip = cell.view();
    let undo = cell.predecessor_view();
    let abandoned = cell.current_replacement_charged(charges::<u64>(&budget).unwrap());
    assert_eq!(*abandoned.get(), 20);
    assert_eq!(budget.reserved_bytes(), 3 * pair);
    drop(abandoned);
    assert_eq!(budget.reserved_bytes(), 2 * pair);
    assert!(predecessor.matches(&cell.publication));
    cell.current_replacement_charged(charges::<u64>(&budget).unwrap())
        .publish(30);
    assert_eq!(
        budget.reserved_bytes(),
        2 * pair + current_bytes,
        "same-cut undo writer was abandoned, current allocation remains published"
    );
    assert_eq!(*cell.view(), 30);
    assert_eq!(*cell.predecessor_view(), Some(10));
    assert_eq!(*tip, 20);
    assert_eq!(*undo, Some(10));
    assert!(!predecessor.matches(&cell.publication));
    drop((tip, undo));
    drop(cell);
    drop(pinned);
    collect_until(&budget, 0);
}

#[test]
fn detached_abort_keeps_original_journal_and_publish_never_returns_generation_charges() {
    let pair = pair_bytes::<u64>();
    let budget = AllocationBudget::new(2 * pair);
    let cell = ChargedCell::new_charged(10_u64, charges::<u64>(&budget).unwrap());
    let pinned = crossbeam_epoch::pin();
    let captured = Arc::new(AtomicBool::new(false));
    let mut block = cell.block_charged(charges::<u64>(&budget).unwrap());
    *block.get_mut() = 20;
    let journal = block
        .try_detach(|_| Ok::<_, ()>(DropMarker(Arc::clone(&captured))))
        .unwrap();
    assert_eq!(
        budget.reserved_bytes(),
        2 * pair,
        "capture retains both exact execution allocations"
    );
    let original_before = std::ptr::from_ref(journal.touched_value().unwrap().before);
    let original_after = std::ptr::from_ref(journal.touched_value().unwrap().after);
    let installation = Arc::new(AtomicBool::new(false));
    let prepared = journal
        .try_prepare_publication(&cell, |_, _| {
            Ok::<_, ()>(DropMarker(Arc::clone(&installation)))
        })
        .unwrap_or_else(|_| panic!("complete paid installation"));
    assert_eq!(budget.reserved_bytes(), 2 * pair);
    assert_eq!(*cell.view(), 10);
    let journal = prepared.abort().0;
    assert_eq!(budget.reserved_bytes(), 2 * pair);
    assert_eq!(
        std::ptr::from_ref(journal.touched_value().unwrap().before),
        original_before
    );
    assert_eq!(
        std::ptr::from_ref(journal.touched_value().unwrap().after),
        original_after
    );
    assert!(installation.load(SeqCst));
    assert!(!captured.load(SeqCst));
    assert!(journal.matches_current(&cell));
    let touch = journal.touched_value().unwrap();
    assert_eq!((*touch.before, *touch.after), (10, 20));
    let installation = Arc::new(AtomicBool::new(false));
    let prepared = journal
        .try_prepare_publication(&cell, |_, _| {
            Ok::<_, ()>(DropMarker(Arc::clone(&installation)))
        })
        .unwrap_or_else(|_| panic!("retry original journal"));
    let returned = prepared.publish();
    assert_eq!(std::ptr::from_ref(cell.view().get()), original_after);
    assert_eq!(
        std::ptr::from_ref(cell.predecessor_view().as_ref().unwrap()),
        original_before
    );
    assert_eq!(*cell.view(), 20);
    assert_eq!(*cell.predecessor_view(), Some(10));
    drop(returned);
    assert!(captured.load(SeqCst));
    assert!(installation.load(SeqCst));
    assert_eq!(
        budget.reserved_bytes(),
        2 * pair,
        "returned temporary owners cannot refund live or retired generations"
    );
    drop(pinned);
    collect_until(&budget, pair);
    drop(cell);
    collect_until(&budget, 0);
}

#[test]
fn untouched_detached_publication_releases_only_unused_current_charge() {
    let pair = pair_bytes::<u64>();
    let undo_bytes = ChargedCell::<u64>::allocation_layouts()[1].size();
    let budget = AllocationBudget::new(2 * pair);
    let cell = ChargedCell::new_charged(10_u64, charges::<u64>(&budget).unwrap());
    let pinned = crossbeam_epoch::pin();
    let original = cell.publication.capture();
    let view = cell.view();
    let journal = cell
        .block_charged(charges::<u64>(&budget).unwrap())
        .try_detach(|_| Ok::<_, ()>(()))
        .unwrap();
    assert!(!journal.is_dirty());
    let prepared = journal
        .try_prepare_publication(&cell, |_, _| Ok::<_, ()>(()))
        .unwrap_or_else(|_| panic!("untouched installation"));
    prepared.publish();
    assert_eq!(budget.reserved_bytes(), pair + undo_bytes);
    assert_eq!(
        std::ptr::from_ref(view.get()),
        std::ptr::from_ref(cell.view().get())
    );
    assert_eq!(*cell.predecessor_view(), None);
    assert!(
        !original.matches(&cell.publication),
        "clear-undo still advances exact pair identity"
    );
    drop(view);
    drop(pinned);
    collect_until(&budget, pair);
    drop(cell);
    collect_until(&budget, 0);
}

#[derive(Debug)]
struct Counted {
    value: u64,
    clones: Arc<AtomicUsize>,
}

impl Clone for Counted {
    fn clone(&self) -> Self {
        self.clones.fetch_add(1, SeqCst);
        Self {
            value: self.value,
            clones: Arc::clone(&self.clones),
        }
    }
}

#[test]
fn repeated_block_and_transaction_mutation_capture_each_preimage_only_once() {
    let budget = AllocationBudget::new(2 * pair_bytes::<Counted>());
    let copies = Arc::new(AtomicUsize::new(0));
    let cell = ChargedCell::new_charged(
        Counted {
            value: 10,
            clones: Arc::clone(&copies),
        },
        charges::<Counted>(&budget).unwrap(),
    );
    let mut block = cell.block_charged(charges::<Counted>(&budget).unwrap());
    assert_eq!(
        copies.load(SeqCst),
        1,
        "only current COW; empty undo has no V"
    );
    block.get_mut().value = 20;
    block.get_mut().value = 30;
    assert_eq!(copies.load(SeqCst), 2, "one block preimage");
    {
        let mut transaction = block.transaction();
        transaction.get_mut().value = 40;
        transaction.get_mut().value = 50;
        assert_eq!(copies.load(SeqCst), 3, "one transaction preimage");
    }
    assert_eq!(
        block.get().value,
        30,
        "aborted child restores its own preimage"
    );
    assert_eq!(block.get_before_block().value, 10);
    {
        let mut transaction = block.transaction();
        transaction.get_mut().value = 60;
        transaction.get_mut().value = 70;
        transaction.apply();
    }
    block.get_mut().value = 80;
    assert_eq!(
        copies.load(SeqCst),
        4,
        "later child adds one preimage, parent retains its first"
    );
    assert_eq!(block.get_before_block().value, 10);
    block.commit();
    assert_eq!(cell.view().value, 80);
    assert_eq!(cell.predecessor_view().as_ref().unwrap().value, 10);
    drop(cell);
    collect_until(&budget, 0);
}

#[test]
fn refusal_and_writer_contention_return_original_charged_journal_without_extra_clones() {
    let pair = pair_bytes::<Counted>();
    let budget = AllocationBudget::new(4 * pair);
    let copies = Arc::new(AtomicUsize::new(0));
    let cell = ChargedCell::new_charged(
        Counted {
            value: 10,
            clones: Arc::clone(&copies),
        },
        charges::<Counted>(&budget).unwrap(),
    );
    let pinned = crossbeam_epoch::pin();
    let mut tip = cell.block_charged(charges::<Counted>(&budget).unwrap());
    tip.get_mut().value = 20;
    tip.commit();
    let mut candidate = cell.block_charged(charges::<Counted>(&budget).unwrap());
    candidate.get_mut().value = 30;
    let journal = candidate.try_detach(|_| Ok::<_, ()>(())).unwrap();
    let retained = budget.reserved_bytes();
    let original_before = std::ptr::from_ref(journal.touched_value().unwrap().before);
    let original_after = std::ptr::from_ref(journal.touched_value().unwrap().after);
    let exhausted = budget
        .try_reserve_layouts(ChargedCell::<Counted>::allocation_layouts())
        .unwrap();
    copies.store(0, SeqCst);
    let journal = match journal
        .try_prepare_publication(&cell, |_, _| budget.try_reserve(Layout::new::<u64>()))
    {
        Err((
            journal,
            PublicationPreparationError::Admission(AllocationRefusal::Capacity { .. }),
            _,
        )) => journal,
        _ => panic!("finite refusal must return original journal"),
    };
    assert_eq!(copies.load(SeqCst), 0);
    drop(exhausted);
    let [current_layout, undo_layout] = ChargedCell::<Counted>::allocation_layouts();
    let mut reserved = budget.try_reserve(undo_layout).unwrap();
    let undo = cell
        .revert
        .write_charged(|_, _| Ok::<_, ()>(reserved.try_split(undo_layout).unwrap()))
        .unwrap();
    copies.store(0, SeqCst);
    let before = budget.reserved_bytes();
    let journal = match journal.try_prepare_publication(&cell, |_, _| Ok::<_, ()>(())) {
        Err((journal, PublicationPreparationError::Busy(_), _)) => journal,
        _ => panic!("first writer contention must return original journal"),
    };
    assert_eq!(copies.load(SeqCst), 0);
    assert_eq!(budget.reserved_bytes(), before);
    drop(undo);
    let mut reserved = budget.try_reserve(current_layout).unwrap();
    let current = cell
        .blocks
        .write_charged(|_, _| Ok::<_, ()>(reserved.try_split(current_layout).unwrap()))
        .unwrap();
    copies.store(0, SeqCst);
    let before = budget.reserved_bytes();
    let journal = match journal.try_prepare_publication(&cell, |_, _| Ok::<_, ()>(())) {
        Err((journal, PublicationPreparationError::Busy(_), _)) => journal,
        _ => panic!("second writer contention must return original journal"),
    };
    assert_eq!(
        copies.load(SeqCst),
        0,
        "partial reacquisition never clones either original successor"
    );
    assert_eq!(
        budget.reserved_bytes(),
        before,
        "partial reacquisition retains original charges without any refund"
    );
    drop(current);
    assert_eq!(budget.reserved_bytes(), retained);
    assert!(journal.matches_current(&cell));
    let touch = journal.touched_value().unwrap();
    assert_eq!((touch.before.value, touch.after.value), (20, 30));
    assert_eq!(std::ptr::from_ref(touch.before), original_before);
    assert_eq!(std::ptr::from_ref(touch.after), original_after);
    assert_eq!(cell.view().value, 20);
    assert_eq!(cell.predecessor_view().as_ref().unwrap().value, 10);
    drop(journal);
    drop(cell);
    drop(pinned);
    collect_until(&budget, 0);
}

#[test]
fn first_undo_clone_panic_wakes_existing_busy_waiter_and_retains_original_successors() {
    use std::{
        future::Future,
        pin::Pin,
        sync::{Mutex, mpsc},
        task::{Context, Wake, Waker},
    };

    struct CloneGate {
        entered: mpsc::Sender<()>,
        release: mpsc::Receiver<()>,
    }
    struct GatedValue {
        value: u64,
        gate: Arc<Mutex<Option<CloneGate>>>,
    }
    impl Clone for GatedValue {
        fn clone(&self) -> Self {
            let gate = self.gate.lock().unwrap().take();
            if let Some(gate) = gate {
                gate.entered.send(()).unwrap();
                gate.release.recv().unwrap();
                panic!("injected first undo clone panic");
            }
            Self {
                value: self.value,
                gate: Arc::clone(&self.gate),
            }
        }
    }
    struct WakeCount(AtomicUsize);
    impl Wake for WakeCount {
        fn wake(self: Arc<Self>) {
            self.0.fetch_add(1, SeqCst);
        }
    }
    struct ReleaseOnDrop(mpsc::Sender<()>);
    impl Drop for ReleaseOnDrop {
        fn drop(&mut self) {
            let _ = self.0.send(());
        }
    }

    let pair = pair_bytes::<GatedValue>();
    let budget = AllocationBudget::new(4 * pair);
    let gate = Arc::new(Mutex::new(None));
    let cell = ChargedCell::new_charged(
        GatedValue {
            value: 10,
            gate: Arc::clone(&gate),
        },
        charges::<GatedValue>(&budget).unwrap(),
    );
    let pinned = crossbeam_epoch::pin();
    let mut tip = cell.block_charged(charges::<GatedValue>(&budget).unwrap());
    tip.get_mut().value = 20;
    tip.commit();
    let mut candidate = cell.block_charged(charges::<GatedValue>(&budget).unwrap());
    candidate.get_mut().value = 30;
    let journal = candidate.try_detach(|_| Ok::<_, ()>(())).unwrap();
    let original_before = std::ptr::from_ref(journal.touched_value().unwrap().before);
    let original_after = std::ptr::from_ref(journal.touched_value().unwrap().after);
    let (entered, entered_receiver) = mpsc::channel();
    let (release, release_receiver) = mpsc::channel();
    *gate.lock().unwrap() = Some(CloneGate {
        entered,
        release: release_receiver,
    });
    let journal = std::thread::scope(|scope| {
        // Also release the injected clone if an assertion fails in this scope.
        let release = ReleaseOnDrop(release);
        let writer = scope.spawn(|| {
            let _ = cell.block_charged(charges::<GatedValue>(&budget).unwrap());
        });
        entered_receiver
            .recv_timeout(Duration::from_secs(5))
            .unwrap();
        assert_eq!(
            budget.reserved_bytes(),
            4 * pair,
            "both charges were admitted before the first clone"
        );
        let (journal, error, _cleanup) = journal
            .try_prepare_publication(&cell, |_, _| Ok::<_, ()>(()))
            .err()
            .expect("original undo writer is still inside Clone");
        drop(_cleanup);
        let PublicationPreparationError::Busy(observation) = error else {
            panic!("must observe busy before the clone panic");
        };
        let wake = Arc::new(WakeCount(AtomicUsize::new(0)));
        let waker = Waker::from(Arc::clone(&wake));
        let mut context = Context::from_waker(&waker);
        let mut wait = observation.wait_for_release();
        assert!(Pin::new(&mut wait).poll(&mut context).is_pending());
        release.0.send(()).unwrap();
        assert!(writer.join().is_err());
        assert_eq!(
            wake.0.load(SeqCst),
            1,
            "first raw clone unwind must signal its existing waiter"
        );
        assert!(Pin::new(&mut wait).poll(&mut context).is_ready());
        journal
    });
    let (journal, error, _cleanup) = journal
        .try_prepare_publication(&cell, |_, _| Ok::<_, ()>(()))
        .err()
        .expect("poisoned original writer requires recovery");
    drop(_cleanup);
    assert_eq!(error, PublicationPreparationError::Poisoned);
    assert_eq!(
        std::ptr::from_ref(journal.touched_value().unwrap().before),
        original_before
    );
    assert_eq!(
        std::ptr::from_ref(journal.touched_value().unwrap().after),
        original_after
    );
    assert_eq!(cell.view().value, 20);
    assert_eq!(cell.predecessor_view().as_ref().unwrap().value, 10);
    let conservatively_retained = ChargedCell::<GatedValue>::allocation_layouts()[1].size();
    drop(journal);
    drop(cell);
    drop(pinned);
    collect_until(&budget, conservatively_retained);
}
