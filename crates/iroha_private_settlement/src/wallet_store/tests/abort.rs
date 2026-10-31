//! Abort cleanup uses genuine native finality over an explicitly synthetic execution result.

use super::super::test_fixture::verified_abort;
use super::*;

#[test]
fn verified_abort_releases_all_original_legs_atomically_and_keeps_a_restart_tombstone() {
    let root = tempfile::tempdir().unwrap();
    let wallet = open_wallet(&root);
    let mut fixture = super::super::test_fixture::complete();
    let abort = verified_abort(&fixture, "private-wallet-all-leg-cleanup");
    stage_complete(&wallet, &mut fixture);
    let round = [91; 32];
    wallet
        .reserve(round, &fixture.intent, 0, &fixture.terms[0])
        .unwrap();
    let before = std::fs::read(wallet.directory.path().join(STATE_NAME)).unwrap();
    assert!(matches!(
        wallet.release_verified_abort(round, &fixture.prepared, &abort),
        Err(Error::Binding)
    ));
    assert_eq!(
        std::fs::read(wallet.directory.path().join(STATE_NAME)).unwrap(),
        before
    );
    claim_complete(&wallet, &fixture, round);
    let authorized = wallet
        .reserve(round, &fixture.intent, 0, &fixture.terms[0])
        .unwrap();
    let (new_intent, new_authorized) = same_inputs_for_another_intent(&fixture.intent, &authorized);
    let opening = wallet
        .policy(&fixture.intent, 0, &fixture.terms[0])
        .unwrap();
    let (original_bytes, _) =
        encode_atomic_private_settlement_reservation_v1(&fixture.intent, 0, &authorized).unwrap();
    drop(authorized);
    PrivateSettlementWallet::reconcile_verified(
        &wallet,
        round,
        &fixture.prepared,
        &VerifiedPrivateSettlementOutcome::Aborted(abort.clone()),
    )
    .unwrap();
    drop(wallet);
    let wallet = open_wallet(&root);
    let terminal = wallet.load().unwrap().0;
    assert!(terminal.reservations.iter().all(|row| row.pending.is_none()
        && row.input_ids.is_empty()
        && matches!(row.completion, Some(Completion::Aborted(_)))));
    let before = std::fs::read(wallet.directory.path().join(STATE_NAME)).unwrap();
    wallet
        .release_verified_abort(round, &fixture.prepared, &abort)
        .unwrap();
    assert_eq!(
        std::fs::read(wallet.directory.path().join(STATE_NAME)).unwrap(),
        before
    );
    assert!(matches!(
        wallet.reserve(round, &fixture.intent, 0, &fixture.terms[0]),
        Err(Error::Aborted)
    ));
    let (original, _) =
        decode_atomic_private_settlement_reservation_v1(original_bytes, &fixture.intent, 0)
            .unwrap();
    let original_opening = PrivateSettlementRestrictedOpening {
        governance: opening.governance.clone(),
        policy: opening.policy.clone(),
    };
    assert!(matches!(
        wallet.stage_authorized(
            &fixture.intent,
            0,
            &fixture.terms[0],
            original_opening,
            original
        ),
        Err(Error::Aborted)
    ));
    assert!(matches!(
        wallet.reconcile_verified(round, &fixture.prepared, &fixture.receipt),
        Err(Error::Binding)
    ));
    wallet
        .stage_authorized(&new_intent, 0, &fixture.terms[0], opening, new_authorized)
        .unwrap();
    assert!(
        wallet
            .reserve([92; 32], &new_intent, 0, &fixture.terms[0])
            .is_ok()
    );
}

#[test]
fn verified_abort_never_releases_consumed_or_foreign_originals_or_a_wrong_round() {
    let root = tempfile::tempdir().unwrap();
    let wallet = open_wallet(&root);
    let mut fixture = super::super::test_fixture::complete();
    let abort = verified_abort(&fixture, "private-wallet-all-leg-cleanup");
    stage_complete(&wallet, &mut fixture);
    let round = [93; 32];
    claim_complete(&wallet, &fixture, round);
    let foreign = super::super::test_fixture::complete_for("wallet-foreign-abort");
    let foreign_abort = verified_abort(&foreign, "wallet-foreign-abort");
    let before = std::fs::read(wallet.directory.path().join(STATE_NAME)).unwrap();
    assert!(matches!(
        wallet.release_verified_abort([94; 32], &fixture.prepared, &abort),
        Err(Error::Binding)
    ));
    assert!(matches!(
        wallet.release_verified_abort(round, &fixture.prepared, &foreign_abort),
        Err(Error::Binding)
    ));
    let constrained =
        FilePrivateSettlementWallet::open(wallet.directory.path(), &[61; 32], 1).unwrap();
    assert!(
        constrained
            .release_verified_abort(round, &fixture.prepared, &abort)
            .is_err()
    );
    assert_eq!(
        std::fs::read(wallet.directory.path().join(STATE_NAME)).unwrap(),
        before
    );
    wallet
        .reconcile_verified(round, &fixture.prepared, &fixture.receipt)
        .unwrap();
    let consumed = std::fs::read(wallet.directory.path().join(STATE_NAME)).unwrap();
    assert!(matches!(
        wallet.release_verified_abort(round, &fixture.prepared, &abort),
        Err(Error::Binding)
    ));
    assert_eq!(
        std::fs::read(wallet.directory.path().join(STATE_NAME)).unwrap(),
        consumed
    );
}

#[test]
fn aborted_outcome_survives_local_reconciliation_failure_without_becoming_settled() {
    let fixture = super::super::test_fixture::complete();
    let abort = verified_abort(&fixture, "private-wallet-all-leg-cleanup");
    let identity = abort.transaction_hash();
    let pending = crate::PrivateSettlementReconciliationPending::new(
        VerifiedPrivateSettlementOutcome::Aborted(abort),
        None,
        OneCallPrivateSettlementError::WalletCustody(Error::Busy),
    );
    assert!(pending.outcome().settled_receipt().is_none());
    assert_eq!(
        pending
            .outcome()
            .abort_receipt()
            .unwrap()
            .transaction_hash(),
        identity
    );
    assert!(pending.progress().is_none());
    assert!(matches!(
        pending.cause(),
        OneCallPrivateSettlementError::WalletCustody(Error::Busy)
    ));
    let (outcome, progress, cause) = pending.into_parts();
    assert_eq!(
        outcome.abort_receipt().unwrap().transaction_hash(),
        identity
    );
    assert!(progress.is_none());
    assert!(matches!(
        cause,
        OneCallPrivateSettlementError::WalletCustody(Error::Busy)
    ));
}
