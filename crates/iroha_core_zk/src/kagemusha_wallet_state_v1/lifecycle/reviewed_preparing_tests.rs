//! Exact approved-preparation dispatch with explicit mock proofs, never phone qualification.

use super::*;

#[test]
fn retained_review_recovers_binding_plan_and_publication_uncertainty_once() {
    for phase in 0..5 {
        let mut wallet = prepared_wallet();
        wallet.proofs.preparation_payload = vec![43; 97];
        let calls = wallet.custody.publication_calls;
        match phase {
            0 => wallet.custody.fail_publication_at = Some((calls + 1, true)),
            1 => wallet.proofs.fail_preparation_proof = true,
            2 => wallet.custody.fail_publication_at = Some((calls + 3, true)),
            3 => wallet.proofs.fail_commit_derivation = true,
            _ => {
                // The immutable recovery batch exists, but the selected Entry has no plan.
                let object = object_digest(&wallet.proofs.preparation_payload);
                *wallet.archive.fail_put.lock().unwrap() =
                    Some((ArchiveKey::Object(object), false));
            }
        }
        let signatures = wallet.custody.signatures;
        assert!(wallet.execute(request()).is_err(), "phase {phase}");
        wallet.proofs.fail_preparation_proof = false;
        wallet.proofs.fail_commit_derivation = false;
        let mut wallet = restart(wallet);
        let (status, binding) = wallet
            .retained_preparation_review_binding(&request())
            .unwrap();
        assert_eq!(status, BoundRequestStatusV1::Preparing, "phase {phase}");
        let plans = wallet.proofs.preparations.load(Ordering::SeqCst);
        let before = (
            derivations(&wallet),
            wallet.custody.signatures,
            wallet.custody.publication_calls,
        );
        assert_eq!(
            wallet
                .execute_reviewed_unbound(request(), None, binding.checkpoint)
                .unwrap(),
            ReviewedExecutionV1::Existing(BoundRequestStatusV1::Preparing)
        );
        assert_eq!(
            (
                derivations(&wallet),
                wallet.custody.signatures,
                wallet.custody.publication_calls
            ),
            before
        );
        let completed = wallet
            .execute_reviewed_preparing(request(), &binding)
            .unwrap();
        let ReviewedExecutionV1::Executed(Completion::Complete(output)) = completed else {
            panic!("exact recovery did not complete phase {phase}: {completed:?}");
        };
        assert_eq!(wallet.custody.signatures, signatures + 1, "phase {phase}");
        assert_eq!(
            wallet.proofs.preparations.load(Ordering::SeqCst),
            plans + usize::from(phase == 0),
            "retained choices must not be planned again, phase {phase}"
        );
        assert_eq!(
            wallet
                .execute_reviewed_preparing(request(), &binding)
                .unwrap(),
            ReviewedExecutionV1::Existing(BoundRequestStatusV1::Outcome(Completion::Complete(
                output
            )))
        );
        assert_eq!(wallet.custody.signatures, signatures + 1);
    }
}

#[test]
fn retained_review_missing_changed_or_conflicting_binding_never_creates_work() {
    let mut wallet = prepared_wallet();
    let empty_checkpoint = wallet.custody.checkpoint.clone();
    assert!(matches!(
        wallet.retained_preparation_review_binding(&request()),
        Err(Error::OperationConflict)
    ));
    wallet.custody.fail_publication = Some(true);
    assert!(wallet.execute(request()).is_err());
    let mut wallet = restart(wallet);
    let (_, binding) = wallet
        .retained_preparation_review_binding(&request())
        .unwrap();
    let before = (
        derivations(&wallet),
        wallet.custody.signatures,
        wallet.custody.publication_calls,
    );
    let mut changed = binding;
    changed.entry[0] ^= 1;
    assert!(matches!(
        wallet.execute_reviewed_preparing(request(), &changed),
        Err(Error::OperationConflict)
    ));
    let mut conflicting = request();
    conflicting.action = OperationActionV1::Unload {
        amount: 1,
        charge: None,
    };
    assert!(matches!(
        wallet.execute_reviewed_preparing(conflicting, &binding),
        Err(Error::OperationConflict)
    ));
    wallet.custody.checkpoint = empty_checkpoint;
    assert!(matches!(
        wallet.execute_reviewed_preparing(request(), &binding),
        Err(Error::OperationConflict)
    ));
    assert_eq!(
        wallet.retry_request(&request().request_id).unwrap(),
        RequestStatusV1::Unknown
    );
    assert_eq!(
        (
            derivations(&wallet),
            wallet.custody.signatures,
            wallet.custody.publication_calls
        ),
        before
    );
}

#[test]
fn retained_review_refuses_a_preparation_that_changed_since_approval() {
    let mut wallet = prepared_wallet();
    wallet.custody.fail_publication = Some(true);
    assert!(wallet.execute(request()).is_err());
    let mut wallet = restart(wallet);
    let (_, binding) = wallet
        .retained_preparation_review_binding(&request())
        .unwrap();
    // A separate attempt retained a plan after this review authenticated the bare binding.
    wallet.proofs.fail_preparation_proof = true;
    assert!(wallet.execute(request()).is_err());
    wallet.proofs.fail_preparation_proof = false;
    let before = (
        derivations(&wallet),
        wallet.custody.signatures,
        wallet.custody.publication_calls,
    );
    assert!(matches!(
        wallet.execute_reviewed_preparing(request(), &binding),
        Err(Error::OperationConflict)
    ));
    assert_eq!(
        (
            derivations(&wallet),
            wallet.custody.signatures,
            wallet.custody.publication_calls
        ),
        before
    );
    // Explicit new approval can authenticate the new locator; it still reuses the plan.
    let (_, current) = wallet
        .retained_preparation_review_binding(&request())
        .unwrap();
    assert_ne!(binding, current);
    let plans = wallet.proofs.preparations.load(Ordering::SeqCst);
    assert!(matches!(
        wallet
            .execute_reviewed_preparing(request(), &current)
            .unwrap(),
        ReviewedExecutionV1::Executed(Completion::Complete(_))
    ));
    assert_eq!(wallet.proofs.preparations.load(Ordering::SeqCst), plans);
}

#[test]
fn retained_review_rechecks_presence_after_maintenance_before_planning() {
    let mut wallet = prepared_wallet();
    let absent = wallet.custody.checkpoint.clone().unwrap();
    wallet.custody.fail_publication = Some(true);
    assert!(wallet.execute(request()).is_err());
    let mut wallet = restart(wallet);
    let (_, binding) = wallet
        .retained_preparation_review_binding(&request())
        .unwrap();
    let before = (
        derivations(&wallet),
        wallet.custody.signatures,
        wallet.custody.publication_calls,
    );
    let mut probe = prepared_wallet();
    let reads = probe.custody.checkpoint_calls;
    probe.require_fresh_work().unwrap();
    let maintenance_reads = probe.custody.checkpoint_calls - reads;
    wallet.custody.checkpoint_replacement = Some((
        wallet.custody.checkpoint_calls + maintenance_reads + 2,
        absent,
    ));
    assert!(matches!(
        wallet.execute_reviewed_preparing(request(), &binding),
        Err(Error::OperationConflict)
    ));
    assert_eq!(
        (
            derivations(&wallet),
            wallet.custody.signatures,
            wallet.custody.publication_calls
        ),
        before
    );
    assert_eq!(
        wallet.retry_request(&request().request_id).unwrap(),
        RequestStatusV1::Unknown
    );
}

#[test]
fn retained_review_observes_selected_pending_without_resuming_or_reproving() {
    let mut wallet = prepared_wallet();
    wallet.proofs.fail_preparation_proof = true;
    assert!(wallet.execute(request()).is_err());
    let (_, binding) = wallet
        .retained_preparation_review_binding(&request())
        .unwrap();
    wallet.proofs.fail_preparation_proof = false;
    wallet.custody.pause = true;
    assert_eq!(wallet.execute(request()).unwrap(), Completion::Pending);
    let before = (
        derivations(&wallet),
        wallet.custody.signatures,
        wallet.custody.publication_calls,
    );
    assert_eq!(
        wallet
            .execute_reviewed_preparing(request(), &binding)
            .unwrap(),
        ReviewedExecutionV1::Existing(BoundRequestStatusV1::Outcome(Completion::Pending))
    );
    assert_eq!(
        (
            derivations(&wallet),
            wallet.custody.signatures,
            wallet.custody.publication_calls
        ),
        before
    );
}
