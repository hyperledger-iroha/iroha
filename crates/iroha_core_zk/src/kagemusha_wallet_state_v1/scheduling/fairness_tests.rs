//! Bounded fold fairness (Λ/Ω §5.6): each sub-proof that payment priority cancels counts once,
//! a durable publication or adoption clears the count, and after
//! `FOLD_FAIRNESS_CANCELLATIONS` cancellations in a row a payment that waits without the wallet
//! owner lets the running sub-proof finish and publish instead of cancelling it. No proof runs;
//! threads stand in for the fold worker and the payers.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread::JoinHandle;
use std::time::Duration;

use super::*;

const SETTLE: Duration = Duration::from_millis(50);
const CHECKPOINT: FoldStatus = FoldStatus::Checkpoint {
    sequence: 0,
    ordinal: 0,
};

fn eligible() -> Scheduler {
    let scheduler = Scheduler::new();
    scheduler.set_activity(true, false);
    scheduler
}

fn wait_until(condition: impl Fn() -> bool) {
    for _ in 0..5_000 {
        if condition() {
            return;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    panic!("condition not reached");
}

/// A fair payment on another thread; the flag is set once it holds priority.
fn fair_payment(scheduler: &Scheduler) -> (JoinHandle<PaymentGuard>, Arc<AtomicBool>) {
    let payer = scheduler.clone();
    let holds = Arc::new(AtomicBool::new(false));
    let observed = Arc::clone(&holds);
    let payment = std::thread::spawn(move || {
        let guard = payer.fair_payment_for(FoldCallClassV1::Payment);
        observed.store(true, Ordering::SeqCst);
        guard
    });
    (payment, holds)
}

/// One sub-proof that a fair payment cancels, ended like a cancelled fold turn.
fn cancelled_by_a_payment(scheduler: &Scheduler) {
    let fold = scheduler.start().expect("a sub-proof may start");
    let (payment, _) = fair_payment(scheduler);
    wait_until(|| fold.token.check().is_err());
    drop(fold);
    drop(payment.join().unwrap());
}

/// A scheduler whose fold payment priority already cancelled the bounded number of times.
fn starved() -> Scheduler {
    let scheduler = eligible();
    for count in 1..=FOLD_FAIRNESS_CANCELLATIONS {
        cancelled_by_a_payment(&scheduler);
        assert_eq!(scheduler.starved_for_test(), count);
    }
    scheduler
}

#[test]
fn after_two_cancellations_a_payment_lets_the_running_sub_proof_publish() {
    let scheduler = starved();
    let fold = scheduler.start().unwrap();
    let (payment, holds) = fair_payment(&scheduler);
    std::thread::sleep(SETTLE);
    fold.token.check().unwrap();
    assert!(!holds.load(Ordering::SeqCst), "it waits for the proof");
    let (_, publication) = fold.release(None);
    std::thread::sleep(SETTLE);
    assert!(
        !holds.load(Ordering::SeqCst),
        "and for the publication, which takes the wallet owner"
    );
    assert!(
        scheduler.start().is_none(),
        "no other sub-proof starts meanwhile"
    );
    publication.settle(Ok(CHECKPOINT));
    let guard = payment.join().unwrap();
    assert_eq!(
        scheduler.starved_for_test(),
        0,
        "the publication clears the count"
    );
    assert!(scheduler.start().is_none(), "the payment holds priority");
    drop(guard);
    // With the count cleared, the next payment cancels again.
    cancelled_by_a_payment(&scheduler);
    assert_eq!(scheduler.starved_for_test(), 1);
}

#[test]
fn each_cancelled_sub_proof_counts_once_and_only_for_payment_priority() {
    let scheduler = eligible();
    // Inactivity and close cancel without counting.
    let fold = scheduler.start().unwrap();
    scheduler.set_activity(false, false);
    drop(fold);
    scheduler.set_activity(true, false);
    assert_eq!(scheduler.starved_for_test(), 0);
    // Two payments that signal the same sub-proof count it once.
    let fold = scheduler.start().unwrap();
    let payers: Vec<_> = [FoldCallClassV1::StatusRead, FoldCallClassV1::Payment]
        .into_iter()
        .map(|class| {
            let payer = scheduler.clone();
            std::thread::spawn(move || drop(payer.payment_for(class)))
        })
        .collect();
    wait_until(|| scheduler.0.lock().payments == 2);
    assert!(matches!(fold.token.check(), Err(Error::Cancelled)));
    drop(fold);
    for payer in payers {
        payer.join().unwrap();
    }
    assert_eq!(scheduler.starved_for_test(), 1);
    // A payment that finds no sub-proof running counts nothing.
    drop(scheduler.fair_payment_for(FoldCallClassV1::Payment));
    assert_eq!(scheduler.starved_for_test(), 1);
}

#[test]
fn a_payment_holding_the_wallet_owner_never_waits_for_a_starved_fold() {
    let scheduler = starved();
    // A released sub-proof's publication needs the owner such a payment holds.
    let fold = scheduler.start().unwrap();
    let (_, publication) = fold.release(None);
    drop(scheduler.payment());
    assert_eq!(
        scheduler.starved_for_test(),
        2,
        "nothing was running to cancel"
    );
    drop(publication);
    // A running sub-proof is cancelled, as before bounded fairness.
    let fold = scheduler.start().unwrap();
    let payer = scheduler.clone();
    let payment = std::thread::spawn(move || drop(payer.payment()));
    wait_until(|| fold.token.check().is_err());
    drop(fold);
    payment.join().unwrap();
    assert_eq!(scheduler.starved_for_test(), 3);
}

#[test]
fn a_waiting_payment_proceeds_when_the_sub_proof_ends_without_durable_progress() {
    #[derive(Clone, Copy, Debug)]
    enum End {
        Inactivity,
        ProofError,
        DiscardedPublication,
    }
    for end in [End::Inactivity, End::ProofError, End::DiscardedPublication] {
        let scheduler = starved();
        let fold = scheduler.start().unwrap();
        let (payment, holds) = fair_payment(&scheduler);
        std::thread::sleep(SETTLE);
        assert!(!holds.load(Ordering::SeqCst), "{end:?}: it waits");
        match end {
            End::Inactivity => {
                scheduler.set_activity(false, false);
                assert!(matches!(fold.token.check(), Err(Error::Cancelled)));
                drop(fold);
            }
            End::ProofError => drop(fold),
            End::DiscardedPublication => {
                let (_, publication) = fold.release(None);
                publication.settle(Err(&Error::Cancelled));
            }
        }
        drop(payment.join().unwrap());
        assert_eq!(
            scheduler.starved_for_test(),
            FOLD_FAIRNESS_CANCELLATIONS,
            "{end:?}: no durable progress keeps the count"
        );
    }
}

#[test]
fn an_adopted_checkpoint_or_omega_clears_the_count() {
    for adopted in [CHECKPOINT, FoldStatus::Folded(0)] {
        let scheduler = starved();
        let fold = scheduler.start().unwrap();
        fold.adopted(FoldStatus::CaughtUp);
        assert_eq!(scheduler.starved_for_test(), FOLD_FAIRNESS_CANCELLATIONS);
        fold.adopted(adopted);
        assert_eq!(scheduler.starved_for_test(), 0, "{adopted:?}");
        drop(fold);
    }
}

#[test]
fn a_speculation_lets_a_starved_fold_publish_before_its_admission() {
    let scheduler = starved();
    let fold = scheduler.start().unwrap();
    let speculation = Speculation::<u32>::new();
    let job = speculation.job();
    let admitting = scheduler.clone();
    let admitted =
        std::thread::spawn(move || admitting.speculate(&job).map(|run| run.finish(1)).is_some());
    std::thread::sleep(SETTLE);
    fold.token.check().unwrap();
    assert!(!scheduler.speculating_for_test());
    let (_, publication) = fold.release(None);
    publication.settle(Ok(CHECKPOINT));
    assert!(admitted.join().unwrap());
    assert_eq!(speculation.take(), Some(1));
    assert_eq!(scheduler.starved_for_test(), 0);
}

/// A fold worker and a stream of fair payments arriving faster than one sub-proof proves:
/// payment priority never cancels more than `FOLD_FAIRNESS_CANCELLATIONS` sub-proofs between
/// two publications, however the threads interleave.
#[test]
fn steady_fair_payments_cancel_at_most_the_bound_between_publications() {
    const PAYMENTS: usize = 40;
    let scheduler = eligible();
    let stop = Arc::new(AtomicBool::new(false));
    let (cancelled, published) = (Arc::new(AtomicU64::new(0)), Arc::new(AtomicU64::new(0)));
    let worker = {
        let (scheduler, stop) = (scheduler.clone(), Arc::clone(&stop));
        let (cancelled, published) = (Arc::clone(&cancelled), Arc::clone(&published));
        std::thread::spawn(move || {
            while !stop.load(Ordering::SeqCst) {
                let Some(fold) = scheduler.start() else {
                    std::thread::sleep(Duration::from_millis(1));
                    continue;
                };
                // A proof of about 15 ms, checked at its task boundaries and once after it.
                for _ in 0..15 {
                    if fold.token.check().is_err() {
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(1));
                }
                if fold.token.check().is_err() {
                    cancelled.fetch_add(1, Ordering::SeqCst);
                    continue;
                }
                let (_, publication) = fold.release(None);
                publication.settle(Ok(CHECKPOINT));
                published.fetch_add(1, Ordering::SeqCst);
            }
        })
    };
    for _ in 0..PAYMENTS {
        let guard = scheduler.fair_payment_for(FoldCallClassV1::Payment);
        std::thread::sleep(Duration::from_millis(1));
        drop(guard);
        // The next payment arrives well within one sub-proof.
        std::thread::sleep(Duration::from_millis(4));
    }
    stop.store(true, Ordering::SeqCst);
    worker.join().unwrap();
    let (cancelled, published) = (
        cancelled.load(Ordering::SeqCst),
        published.load(Ordering::SeqCst),
    );
    assert!(
        cancelled <= u64::from(FOLD_FAIRNESS_CANCELLATIONS) * (published + 1),
        "{cancelled} cancelled sub-proofs for {published} publications"
    );
}
