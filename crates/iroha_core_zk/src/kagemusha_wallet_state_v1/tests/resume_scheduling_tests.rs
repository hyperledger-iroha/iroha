//! Selected recovery keeps payment priority; empty polls preserve useful fold work.
//! Proof fixtures are mocks and these tests do not qualify Native proving or a phone.

use super::*;
use std::{ops::ControlFlow, sync::mpsc, thread, time::Instant};

fn released_wallet() -> Wallet {
    let mut wallet = wallet();
    wallet.proofs.checkpoint_stages = 3;
    assert!(matches!(
        wallet.commit(bootstrap()).unwrap(),
        Completion::Complete(_)
    ));
    wallet.scheduler().set_activity(true, false);
    wallet
}

#[test]
fn pending_inspection_distinguishes_enrollment_released_and_selected_operation() {
    let mut wallet = wallet();
    assert!(!wallet.has_pending_operation().unwrap());
    wallet.custody.pause = true;
    assert_eq!(wallet.commit(bootstrap()).unwrap(), Completion::Pending);
    assert!(wallet.has_pending_operation().unwrap());
    assert_eq!(wallet.custody.signatures, 0);
    wallet.custody.pause = false;
    assert!(matches!(
        wallet.resume().unwrap(),
        Some(Completion::Complete(_))
    ));
    assert!(!wallet.has_pending_operation().unwrap());
    assert_eq!(wallet.custody.signatures, 1);
}

#[test]
fn idle_resume_does_not_cancel_or_join_an_admitted_fold_job() {
    let mut wallet = released_wallet();
    let job = match wallet.fold_prepare().unwrap() {
        ControlFlow::Continue(job) => job,
        ControlFlow::Break(ended) => panic!("expected admitted job: {ended:?}"),
    };
    let (finished_tx, finished) = mpsc::channel();
    let poll = thread::spawn(move || {
        let result = wallet.resume();
        finished_tx.send(()).unwrap();
        (wallet, result)
    });
    // Always release the job before asserting, including against the old implementation
    // that waits for its own cancellation to release this scheduler slot.
    let returned_before_proof = finished.recv_timeout(Duration::from_secs(5)).is_ok();
    let proved = job.prove();
    let (mut wallet, result) = poll.join().unwrap();
    assert!(
        returned_before_proof,
        "an empty recovery poll joined the fold"
    );
    assert_eq!(result.unwrap(), None);
    let ended = wallet
        .fold_publish(proved.expect("poll must not cancel the proof"))
        .unwrap();
    assert_eq!(
        ended.status,
        FoldStatus::Checkpoint {
            sequence: 0,
            ordinal: 0
        }
    );
    assert!(wallet.scheduler().holds_retained_for_test());
    assert_eq!(wallet.custody.signatures, 1);
}

#[test]
fn idle_resume_preserves_the_published_fold_retention_and_durable_state() {
    let mut wallet = released_wallet();
    assert_eq!(
        wallet.fold_once().unwrap(),
        FoldStatus::Checkpoint {
            sequence: 0,
            ordinal: 0
        }
    );
    let retained = wallet.proofs.last_left();
    let manifest = wallet.custody.checkpoint.clone();
    let records = wallet.archive.records.lock().unwrap().clone();
    assert!(wallet.scheduler().holds_retained_for_test());
    assert_eq!(wallet.resume().unwrap(), None);
    assert!(wallet.scheduler().holds_retained_for_test());
    assert_eq!(wallet.custody.checkpoint, manifest);
    assert_eq!(*wallet.archive.records.lock().unwrap(), records);
    assert_eq!(
        wallet.fold_once().unwrap(),
        FoldStatus::Checkpoint {
            sequence: 0,
            ordinal: 1
        }
    );
    assert_eq!(wallet.proofs.last_handed(), Some(retained));
    assert_eq!(wallet.custody.signatures, 1);
}

#[test]
fn failed_pending_inspection_and_resume_preserve_fold_retention() {
    let mut wallet = released_wallet();
    wallet.fold_once().unwrap();
    let retained = wallet.proofs.last_left();
    let failure = ProviderError::Unavailable(
        crate::kagemusha_wallet_advance_v1::KagemushaWalletUnavailableV1::Locked,
    );
    wallet.custody.status_error = Some(failure);
    assert!(
        matches!(wallet.has_pending_operation(), Err(Error::Provider(error)) if error == failure)
    );
    assert!(matches!(wallet.resume(), Err(Error::Provider(error)) if error == failure));
    assert!(wallet.scheduler().holds_retained_for_test());
    wallet.custody.status_error = None;
    wallet.fold_once().unwrap();
    assert_eq!(wallet.proofs.last_handed(), Some(retained));
    assert_eq!(wallet.custody.signatures, 1);
}

#[test]
fn selected_resume_cancels_and_joins_fold_work_before_exact_completion() {
    let mut wallet = wallet();
    let boot = bootstrap();
    let operation = boot.capsule.body().operation_id;
    wallet.custody.pause = true;
    assert_eq!(wallet.commit(boot).unwrap(), Completion::Pending);
    wallet.custody.pause = false;
    let scheduler = wallet.scheduler();
    scheduler.set_activity(true, false);
    let (entered_tx, entered) = mpsc::channel();
    let released = Arc::new(AtomicBool::new(false));
    let observed = released.clone();
    let proof = thread::spawn(move || {
        scheduler.sub_proof_for_test(0, |token| {
            entered_tx.send(()).unwrap();
            let deadline = Instant::now() + Duration::from_secs(5);
            loop {
                if let Err(error) = token.check() {
                    observed.store(true, Ordering::SeqCst);
                    return Err(error);
                }
                assert!(
                    Instant::now() < deadline,
                    "selected recovery did not preempt"
                );
                thread::yield_now();
            }
        })
    });
    entered.recv_timeout(Duration::from_secs(5)).unwrap();
    let complete = wallet.resume().unwrap().expect("selected recovery");
    assert!(
        released.load(Ordering::SeqCst),
        "fold work must end before recovery returns"
    );
    assert!(matches!(proof.join().unwrap(), Err(Error::Cancelled)));
    assert!(matches!(complete, Completion::Complete(_)));
    assert_eq!(wallet.retry(&operation).unwrap(), Some(complete));
    assert_eq!(wallet.custody.signatures, 1);
}

#[test]
fn selected_resume_rechecks_custody_after_acquiring_payment_priority() {
    for unavailable in [false, true] {
        let mut wallet = wallet();
        let enrollment = wallet.custody.status.clone();
        wallet.custody.pause = true;
        assert_eq!(wallet.commit(bootstrap()).unwrap(), Completion::Pending);
        wallet.custody.pause = false;
        let after_priority = wallet.custody.status_calls + 2;
        if unavailable {
            wallet.custody.status_unavailable_at = Some(after_priority);
            assert!(matches!(
                wallet.resume(),
                Err(Error::Provider(ProviderError::Unavailable(_)))
            ));
        } else {
            wallet.custody.status_replacement = Some((after_priority, enrollment));
            assert_eq!(
                wallet.resume().unwrap(),
                None,
                "a vanished selection cannot use cached inputs"
            );
        }
        assert_eq!(
            wallet.custody.signatures, 0,
            "no Advance after a refused recheck"
        );
        assert!(
            wallet.custody.pending.is_some(),
            "the fixture still retains the original attempt"
        );
    }
}
