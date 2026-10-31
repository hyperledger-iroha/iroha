//! Real private-file tests for wallet custody; opaque index fixtures grant no authorization.

use super::*;
use iroha_core_privacy::privacy_engines::atomic_private_settlement::{
    AtomicPrivateSettlementInputSecretV1, atomic_private_settlement_reservation_fixture_v1,
    prepare_atomic_private_settlement_authorization_inputs_v1,
    test_fixtures::{authorize_output_view_keys, authorize_payer_inputs},
};
use iroha_crypto::{Algorithm, KeyPair};

mod abort;

type Original = (
    PrivateSettlementProofIntentV1,
    PrivateSettlementBusinessTerms,
    PrivateSettlementRestrictedOpening,
    AtomicPrivateSettlementAuthorizedWalletLegV1,
);

fn original() -> Original {
    let (intent, governance, policy, authorized) =
        atomic_private_settlement_reservation_fixture_v1(None);
    let plaintext = authorized.plaintext();
    let terms = PrivateSettlementBusinessTerms {
        asset: plaintext.asset_definition_id.clone(),
        payer: plaintext.payer.clone(),
        recipient: plaintext.recipient.clone(),
        amount: plaintext.amount,
        reimbursement: plaintext.sponsor_reimbursement_amount,
        memo: plaintext.memo.clone(),
    };
    (
        intent,
        terms,
        PrivateSettlementRestrictedOpening { governance, policy },
        authorized,
    )
}

fn same_inputs_for_another_intent(
    intent: &PrivateSettlementProofIntentV1,
    original: &AtomicPrivateSettlementAuthorizedWalletLegV1,
) -> (
    PrivateSettlementProofIntentV1,
    AtomicPrivateSettlementAuthorizedWalletLegV1,
) {
    let changed = PrivateSettlementProofIntentV1::new(
        intent.network_id,
        intent.authority_context_height + 1,
        intent.expiry_height,
        intent.sponsor.clone(),
        intent.public_fee_intent.clone(),
        intent.reimbursement_terms_commitment,
        intent.reimbursement_leg_ordinal,
        intent.legs.clone(),
    )
    .unwrap();
    let mut plaintext = original.plaintext().clone();
    plaintext.bundle_id = changed.bundle_id;
    let secrets = Zeroizing::new([[0x81; 32], [0x82; 32]]);
    let nullifiers = prepare_atomic_private_settlement_authorization_inputs_v1(
        &changed,
        0,
        &mut plaintext.inputs,
        &secrets,
    )
    .unwrap();
    let payer = KeyPair::from_seed(vec![0x38; 32], Algorithm::Ed25519);
    let recipient = KeyPair::from_seed(vec![0x39; 32], Algorithm::Ed25519);
    let sponsor = KeyPair::from_seed(vec![0x23; 32], Algorithm::Ed25519);
    authorize_payer_inputs(&mut plaintext, &nullifiers, &payer);
    authorize_output_view_keys(&mut plaintext, [&recipient, &payer, &sponsor]);
    let inputs = core::array::from_fn(|index| {
        AtomicPrivateSettlementInputSecretV1::new(
            secrets[index],
            index as u32,
            &core::array::from_fn(|level| [0x40 + level as u8; 32]),
        )
        .unwrap()
    });
    (
        changed,
        AtomicPrivateSettlementAuthorizedWalletLegV1::new(plaintext, inputs),
    )
}

fn open_wallet(root: &tempfile::TempDir) -> FilePrivateSettlementWallet {
    FilePrivateSettlementWallet::open(root.path().join("wallet"), &[61; 32], 4 * 1024 * 1024)
        .unwrap()
}

// These inner ciphertexts exercise index/AEAD ownership only, never native witness admission.
fn opaque_row(wallet: &FilePrivateSettlementWallet, seed: u8) -> Reservation {
    let mut row = Reservation {
        id: [seed; 32],
        terms: [seed.wrapping_add(1); 32],
        original: [seed.wrapping_add(2); 32],
        input_ids: vec![Hash::new([seed.wrapping_add(3)])],
        round: None,
        completion: None,
        pending: None,
    };
    row.pending = Some(Pending {
        opening: wallet.encrypt(&row, "opening", b"opaque-opening").unwrap(),
        authorization: wallet
            .encrypt(&row, "authorization", b"opaque-native-codec")
            .unwrap(),
    });
    row
}

#[test]
fn wallet_open_creates_no_record_and_refuses_zero_key_or_unbounded_allowance() {
    let root = tempfile::tempdir().unwrap();
    let wallet = open_wallet(&root);
    assert_eq!(
        std::fs::read_dir(wallet.directory.path()).unwrap().count(),
        0
    );
    let (state, exists) = wallet.load().unwrap();
    assert!(!exists);
    assert!(state.reservations.is_empty());
    assert_eq!(
        std::fs::read_dir(wallet.directory.path()).unwrap().count(),
        0
    );
    assert!(matches!(
        FilePrivateSettlementWallet::open(root.path().join("zero"), &[0; 32], 1),
        Err(Error::Binding)
    ));
    for bound in [0, MAX_PRIVATE_SETTLEMENT_WALLET_BYTES + 1] {
        assert!(matches!(
            FilePrivateSettlementWallet::open(root.path().join("capacity"), &[61; 32], bound),
            Err(Error::Capacity)
        ));
    }
}

#[test]
fn independent_wallet_handles_share_one_native_publication_lock() {
    let root = tempfile::tempdir().unwrap();
    let first = open_wallet(&root);
    let second = open_wallet(&root);
    let lock = first.writer().unwrap();
    assert!(matches!(second.writer(), Err(Error::Busy)));
    first.publish(&lock, false, &State::default()).unwrap();
    drop(lock);
    let lock = second.writer().unwrap();
    let (state, exists) = second.load().unwrap();
    assert!(exists);
    assert!(state.reservations.is_empty());
    second.publish(&lock, true, &state).unwrap();
}

#[test]
fn pending_inner_ciphertexts_bind_exact_original_and_purpose() {
    let root = tempfile::tempdir().unwrap();
    let wallet = open_wallet(&root);
    let mut row = opaque_row(&wallet, 21);
    let pending = row.pending.as_ref().unwrap();
    assert_eq!(
        wallet
            .decrypt(&row, "authorization", &pending.authorization)
            .unwrap()
            .as_slice(),
        b"opaque-native-codec"
    );
    assert!(matches!(
        wallet.decrypt(&row, "opening", &pending.authorization),
        Err(Error::Binding)
    ));
    let ciphertext = pending.authorization.clone();
    row.terms = [91; 32];
    assert!(matches!(
        wallet.decrypt(&row, "authorization", &ciphertext),
        Err(Error::Binding)
    ));
    row.terms = [22; 32];
    row.id = [92; 32];
    assert!(matches!(
        wallet.decrypt(&row, "authorization", &ciphertext),
        Err(Error::Binding)
    ));
    row.id = [21; 32];
    row.original = [93; 32];
    assert!(matches!(
        wallet.decrypt(&row, "authorization", &ciphertext),
        Err(Error::Binding)
    ));
}

#[test]
fn wallet_index_reopen_refuses_foreign_key_and_corruption_without_replacement() {
    let root = tempfile::tempdir().unwrap();
    let wallet = open_wallet(&root);
    let row = opaque_row(&wallet, 24);
    let expected_id = row.id;
    let state = State {
        reservations: vec![row],
    };
    let lock = wallet.writer().unwrap();
    wallet.publish(&lock, false, &state).unwrap();
    drop(lock);
    let path = wallet.directory.path().join(STATE_NAME);
    let original = std::fs::read(&path).unwrap();
    assert!(
        !original
            .windows(b"opaque-native-codec".len())
            .any(|bytes| bytes == b"opaque-native-codec")
    );
    let foreign =
        FilePrivateSettlementWallet::open(wallet.directory.path(), &[62; 32], wallet.maximum)
            .unwrap();
    assert!(matches!(foreign.load(), Err(Error::Binding)));
    assert_eq!(std::fs::read(&path).unwrap(), original);
    drop(wallet);
    let reopened = open_wallet(&root);
    assert_eq!(reopened.load().unwrap().0.reservations[0].id, expected_id);
    let mut corrupt = original;
    corrupt[0] ^= 1;
    reopened
        .directory
        .write_atomic(STATE_NAME, &corrupt, PublishMode::Replace)
        .unwrap();
    assert!(matches!(reopened.load(), Err(Error::Binding)));
    assert_eq!(std::fs::read(&path).unwrap(), corrupt);
}

#[test]
fn refused_wallet_publication_preserves_the_original_index() {
    let root = tempfile::tempdir().unwrap();
    let wallet = open_wallet(&root);
    let lock = wallet.writer().unwrap();
    wallet.publish(&lock, false, &State::default()).unwrap();
    let original = std::fs::read(wallet.directory.path().join(STATE_NAME)).unwrap();
    drop(lock);
    let limited = FilePrivateSettlementWallet::open(wallet.directory.path(), &[61; 32], 1).unwrap();
    let lock = wallet.writer().unwrap();
    let candidate = State {
        reservations: vec![opaque_row(&wallet, 31)],
    };
    assert!(matches!(
        limited.publish(&lock, true, &candidate),
        Err(Error::Capacity)
    ));
    assert_eq!(
        std::fs::read(wallet.directory.path().join(STATE_NAME)).unwrap(),
        original
    );
    assert!(wallet.load().unwrap().0.reservations.is_empty());
}

#[test]
fn consumed_markers_keep_input_exclusions_and_require_complete_terminal_shape() {
    let root = tempfile::tempdir().unwrap();
    let wallet = open_wallet(&root);
    let mut consumed = opaque_row(&wallet, 35);
    consumed.pending = None;
    consumed.round = Some([41; 32]);
    consumed.completion = Some(Completion::Consumed([42; 32]));
    let input = consumed.input_ids[0];
    let lock = wallet.writer().unwrap();
    let mut state = State {
        reservations: vec![consumed],
    };
    wallet.publish(&lock, false, &state).unwrap();
    let reopened = wallet.load().unwrap().0;
    assert_eq!(reopened.reservations[0].input_ids, [input]);
    assert!(reopened.reservations[0].pending.is_none());
    state.reservations[0].completion = None;
    wallet.publish(&lock, true, &state).unwrap();
    assert!(matches!(wallet.load(), Err(Error::Binding)));
    state.reservations[0].completion = Some(Completion::Consumed([42; 32]));
    let mut other = opaque_row(&wallet, 36);
    other.input_ids = vec![input];
    state.reservations.push(other);
    wallet.publish(&lock, true, &state).unwrap();
    assert!(
        matches!(wallet.load(), Err(Error::Binding)),
        "a consumed input must not appear in another reservation"
    );
}

#[test]
fn independent_business_terms_bind_every_field_and_refuse_unbounded_memo() {
    let original = PrivateSettlementBusinessTerms {
        asset: "6TEAJqbb8oEPmLncoNiMRbLEK6tw".parse().unwrap(),
        payer: iroha_test_samples::ALICE_ID.clone(),
        recipient: iroha_test_samples::BOB_ID.clone(),
        amount: 17,
        reimbursement: 2,
        memo: b"restricted-original-reference".to_vec(),
    };
    let digest = terms_digest(&original).unwrap();
    for mutate in [
        |terms: &mut PrivateSettlementBusinessTerms| {
            let mut bytes = [44; 16];
            bytes[6] = 0x40;
            bytes[8] = 0x80;
            terms.asset =
                iroha_data_model::asset::AssetDefinitionId::from_uuid_bytes(bytes).unwrap();
        },
        |terms: &mut PrivateSettlementBusinessTerms| terms.payer = terms.recipient.clone(),
        |terms: &mut PrivateSettlementBusinessTerms| terms.recipient = terms.payer.clone(),
        |terms: &mut PrivateSettlementBusinessTerms| terms.amount += 1,
        |terms: &mut PrivateSettlementBusinessTerms| terms.reimbursement += 1,
        |terms: &mut PrivateSettlementBusinessTerms| terms.memo.push(b'!'),
    ] {
        let mut changed = original.clone();
        mutate(&mut changed);
        assert_ne!(terms_digest(&changed).unwrap(), digest);
    }
    let mut changed = original.clone();
    changed.amount = 0;
    assert!(matches!(terms_digest(&changed), Err(Error::Binding)));
    changed.amount = original.amount;
    changed.memo = vec![0; PRIVATE_SETTLEMENT_MAX_AUDIT_MEMO_BYTES_V1 + 1];
    assert!(matches!(terms_digest(&changed), Err(Error::Binding)));
}

#[test]
fn native_authorization_reopens_exactly_and_first_round_cannot_be_replaced() {
    let root = tempfile::tempdir().unwrap();
    let wallet = open_wallet(&root);
    let (intent, terms, opening, authorized) = original();
    let original_opening = PrivateSettlementRestrictedOpening {
        governance: opening.governance.clone(),
        policy: opening.policy.clone(),
    };
    let (expected, inspection) =
        encode_atomic_private_settlement_reservation_v1(&intent, 0, &authorized).unwrap();
    wallet
        .stage_authorized(&intent, 0, &terms, opening, authorized)
        .unwrap();
    let bytes = std::fs::read(wallet.directory.path().join(STATE_NAME)).unwrap();
    assert!(
        !bytes
            .windows(terms.memo.len())
            .any(|window| window == terms.memo)
    );
    let first = wallet.reserve([71; 32], &intent, 0, &terms).unwrap();
    let (actual, inspected) =
        encode_atomic_private_settlement_reservation_v1(&intent, 0, &first).unwrap();
    assert!(
        *actual == *expected,
        "native secret custody must reopen exact original bytes"
    );
    assert_eq!(inspected, inspection);
    drop(first);
    drop(wallet);
    let reopened = open_wallet(&root);
    let original_bytes = std::fs::read(reopened.directory.path().join(STATE_NAME)).unwrap();
    assert!(reopened.reserve([71; 32], &intent, 0, &terms).is_ok());
    assert!(matches!(
        reopened.reserve([72; 32], &intent, 0, &terms),
        Err(Error::Binding)
    ));
    let mut changed = terms.clone();
    changed.memo.push(b'!');
    assert!(matches!(
        reopened.policy(&intent, 0, &changed),
        Err(Error::Binding)
    ));
    assert!(matches!(
        reopened.reserve([71; 32], &intent, 0, &changed),
        Err(Error::Binding)
    ));
    assert_eq!(
        std::fs::read(reopened.directory.path().join(STATE_NAME)).unwrap(),
        original_bytes
    );
    // Reuse the exact authenticated original. A newly generated fixture can choose
    // different randomized output-encryption keys and therefore another intent.
    let (authorized, _) =
        decode_atomic_private_settlement_reservation_v1(expected, &intent, 0).unwrap();
    reopened
        .stage_authorized(&intent, 0, &terms, original_opening, authorized)
        .unwrap();
    assert_eq!(
        std::fs::read(reopened.directory.path().join(STATE_NAME)).unwrap(),
        original_bytes
    );
}

#[test]
fn another_valid_signed_intent_cannot_take_an_original_active_input_after_restart() {
    let root = tempfile::tempdir().unwrap();
    let wallet = open_wallet(&root);
    let (intent, terms, opening, authorized) = original();
    let (changed, competing) = same_inputs_for_another_intent(&intent, &authorized);
    let competing_opening = PrivateSettlementRestrictedOpening {
        governance: opening.governance.clone(),
        policy: opening.policy.clone(),
    };
    let (_, original_inspection) =
        encode_atomic_private_settlement_reservation_v1(&intent, 0, &authorized).unwrap();
    let (_, competing_inspection) =
        encode_atomic_private_settlement_reservation_v1(&changed, 0, &competing).unwrap();
    assert_ne!(
        reservation_id(&intent, 0).unwrap(),
        reservation_id(&changed, 0).unwrap()
    );
    assert_eq!(
        original_inspection.input_ids,
        competing_inspection.input_ids
    );
    wallet
        .stage_authorized(&intent, 0, &terms, opening, authorized)
        .unwrap();
    wallet.reserve([73; 32], &intent, 0, &terms).unwrap();
    drop(wallet);
    let reopened = open_wallet(&root);
    let original = std::fs::read(reopened.directory.path().join(STATE_NAME)).unwrap();
    assert!(matches!(
        reopened.stage_authorized(&changed, 0, &terms, competing_opening, competing),
        Err(Error::InputReserved)
    ));
    assert_eq!(
        std::fs::read(reopened.directory.path().join(STATE_NAME)).unwrap(),
        original
    );
    assert!(reopened.reserve([73; 32], &intent, 0, &terms).is_ok());
}

#[test]
fn native_stage_capacity_refusal_publishes_no_partial_input_exclusion() {
    let root = tempfile::tempdir().unwrap();
    let wallet =
        FilePrivateSettlementWallet::open(root.path().join("wallet"), &[61; 32], 1).unwrap();
    let (intent, terms, opening, authorized) = original();
    assert!(matches!(
        wallet.stage_authorized(&intent, 0, &terms, opening, authorized),
        Err(Error::Capacity)
    ));
    assert!(!wallet.directory.path().join(STATE_NAME).exists());
    drop(wallet);
    let reopened = open_wallet(&root);
    let (intent, terms, opening, authorized) = original();
    reopened
        .stage_authorized(&intent, 0, &terms, opening, authorized)
        .unwrap();
    assert!(reopened.reserve([74; 32], &intent, 0, &terms).is_ok());
}

fn stage_complete(
    wallet: &FilePrivateSettlementWallet,
    fixture: &mut super::test_fixture::Fixture,
) {
    for (index, (opening, authorized)) in fixture
        .openings
        .drain(..)
        .zip(fixture.authorized.drain(..))
        .enumerate()
    {
        wallet
            .stage_authorized(
                &fixture.intent,
                index as u8,
                &fixture.terms[index],
                opening,
                authorized,
            )
            .unwrap();
    }
}
fn claim_complete(
    wallet: &FilePrivateSettlementWallet,
    fixture: &super::test_fixture::Fixture,
    round: Digest32,
) {
    for (index, terms) in fixture.terms.iter().enumerate() {
        drop(
            wallet
                .reserve_or_reopen(round, &fixture.intent, index as u8, terms)
                .unwrap(),
        );
    }
}

#[test]
fn verified_success_consumes_all_original_legs_atomically_and_reopens_only_as_consumed() {
    let root = tempfile::tempdir().unwrap();
    let wallet = open_wallet(&root);
    let mut fixture = super::test_fixture::complete();
    stage_complete(&wallet, &mut fixture);
    let round = [83; 32];
    drop(
        wallet
            .reserve_or_reopen(round, &fixture.intent, 0, &fixture.terms[0])
            .unwrap(),
    );
    let original = std::fs::read(wallet.directory.path().join(STATE_NAME)).unwrap();
    assert!(matches!(
        wallet.reconcile_verified(round, &fixture.prepared, &fixture.receipt),
        Err(Error::Binding)
    ));
    assert_eq!(
        std::fs::read(wallet.directory.path().join(STATE_NAME)).unwrap(),
        original,
        "a missing second-leg round cannot wipe any original authorization"
    );
    assert!(
        wallet
            .load()
            .unwrap()
            .0
            .reservations
            .iter()
            .all(|row| row.pending.is_some() && row.completion.is_none())
    );
    claim_complete(&wallet, &fixture, round);
    drop(wallet);
    let reopened = open_wallet(&root);
    let pending = reopened.load().unwrap().0;
    let original_inputs = pending
        .reservations
        .iter()
        .map(|row| row.input_ids.clone())
        .collect::<Vec<_>>();
    reopened
        .reconcile_verified(round, &fixture.prepared, &fixture.receipt)
        .unwrap();
    let terminal = reopened.load().unwrap().0;
    assert_eq!(terminal.reservations.len(), 2);
    assert!(
        terminal.reservations.iter().all(|row| row.pending.is_none()
            && row.completion.is_some()
            && row.round == Some(round))
    );
    assert_eq!(
        terminal
            .reservations
            .iter()
            .map(|row| row.input_ids.clone())
            .collect::<Vec<_>>(),
        original_inputs
    );
    let completed = std::fs::read(reopened.directory.path().join(STATE_NAME)).unwrap();
    reopened
        .reconcile_verified(round, &fixture.prepared, &fixture.receipt)
        .unwrap();
    assert_eq!(
        std::fs::read(reopened.directory.path().join(STATE_NAME)).unwrap(),
        completed,
        "same verified success is a read-only idempotent cleanup retry"
    );
    for (index, terms) in fixture.terms.iter().enumerate() {
        assert!(matches!(
            reopened.reserve_or_reopen(round, &fixture.intent, index as u8, terms),
            Err(OneCallPrivateSettlementError::WalletCustody(
                Error::Consumed
            ))
        ));
    }
    assert!(matches!(
        reopened.reconcile_verified([84; 32], &fixture.prepared, &fixture.receipt),
        Err(Error::Binding)
    ));
    assert_eq!(
        std::fs::read(reopened.directory.path().join(STATE_NAME)).unwrap(),
        completed
    );
}

#[test]
fn verified_success_cleanup_refuses_foreign_receipt_and_local_custody_failure_without_releasing_inputs()
 {
    let root = tempfile::tempdir().unwrap();
    let wallet = open_wallet(&root);
    let mut fixture = super::test_fixture::complete();
    stage_complete(&wallet, &mut fixture);
    claim_complete(&wallet, &fixture, [85; 32]);
    let original = std::fs::read(wallet.directory.path().join(STATE_NAME)).unwrap();
    let other = super::test_fixture::complete_for("private-wallet-unrelated-cleanup");
    assert_ne!(fixture.intent.network_id, other.intent.network_id);
    assert!(matches!(
        wallet.reconcile_verified([85; 32], &fixture.prepared, &other.receipt),
        Err(Error::Binding)
    ));
    assert_eq!(
        std::fs::read(wallet.directory.path().join(STATE_NAME)).unwrap(),
        original
    );
    let constrained =
        FilePrivateSettlementWallet::open(wallet.directory.path(), &[61; 32], 1).unwrap();
    assert!(
        constrained
            .reconcile_verified([85; 32], &fixture.prepared, &fixture.receipt)
            .is_err()
    );
    assert_eq!(
        std::fs::read(wallet.directory.path().join(STATE_NAME)).unwrap(),
        original
    );
    assert!(
        wallet
            .load()
            .unwrap()
            .0
            .reservations
            .iter()
            .all(|row| row.pending.is_some())
    );
    wallet
        .reconcile_verified([85; 32], &fixture.prepared, &fixture.receipt)
        .unwrap();
}

#[test]
fn local_reconciliation_failure_retains_verified_success_optional_progress_and_original_cause() {
    use crate::PrivateSettlementReconciliationPending;
    use iroha::workflow::WorkflowErrorCode;
    use std::error::Error as _;
    let fixture = super::test_fixture::complete();
    let original_hash = fixture.receipt.transaction_hash();
    let pending = PrivateSettlementReconciliationPending::new(
        VerifiedPrivateSettlementOutcome::Settled(fixture.receipt),
        None,
        OneCallPrivateSettlementError::WalletCustody(Error::Io(std::io::Error::other(
            "restricted original cause",
        ))),
    );
    assert_eq!(
        pending
            .outcome()
            .settled_receipt()
            .unwrap()
            .transaction_hash(),
        original_hash
    );
    assert!(pending.progress().is_none());
    assert!(pending.source().is_some());
    assert!(!format!("{pending:?} {pending}").contains("restricted original cause"));
    let error = OneCallPrivateSettlementError::ReconciliationPending(Box::new(pending));
    assert_eq!(
        error.workflow_code(),
        WorkflowErrorCode::LocalResourceUnavailable
    );
    let OneCallPrivateSettlementError::ReconciliationPending(pending) = error else {
        panic!("typed success owner lost")
    };
    let (receipt, progress, cause) = pending.into_parts();
    assert_eq!(
        receipt.settled_receipt().unwrap().transaction_hash(),
        original_hash
    );
    assert!(progress.is_none());
    let OneCallPrivateSettlementError::WalletCustody(Error::Io(original)) = cause else {
        panic!("original cause lost")
    };
    assert_eq!(original.to_string(), "restricted original cause");
}

#[test]
fn original_writer_reconciles_one_bounded_native_staging_file_and_refuses_unknown_or_excess_custody()
 {
    let root = tempfile::tempdir().unwrap();
    let wallet = open_wallet(&root);
    let lock = wallet.writer().unwrap();
    wallet.publish(&lock, false, &State::default()).unwrap();
    drop(lock);
    let original = std::fs::read(wallet.directory.path().join(STATE_NAME)).unwrap();
    let staged = ".iroha-fs-713-0.tmp";
    for _ in 0..3 {
        wallet
            .directory
            .write_atomic(
                staged,
                b"bounded interrupted ciphertext",
                PublishMode::CreateNew,
            )
            .unwrap();
        drop(wallet.writer().unwrap());
        assert!(!wallet.directory.path().join(staged).exists());
        assert_eq!(
            std::fs::read(wallet.directory.path().join(STATE_NAME)).unwrap(),
            original
        );
    }
    wallet
        .directory
        .write_atomic(
            staged,
            b"original staged ciphertext",
            PublishMode::CreateNew,
        )
        .unwrap();
    wallet
        .directory
        .write_atomic(".iroha-fs-713-1.tmp", b"second", PublishMode::CreateNew)
        .unwrap();
    assert!(wallet.writer().is_err());
    assert!(
        wallet.directory.path().join(staged).exists(),
        "excess inventory must fail before any cleanup"
    );
    wallet
        .directory
        .remove_private(".iroha-fs-713-1.tmp")
        .unwrap();
    wallet
        .directory
        .write_atomic("unexpected", b"unknown", PublishMode::CreateNew)
        .unwrap();
    assert!(wallet.writer().is_err());
    assert!(wallet.directory.path().join(staged).exists());
    wallet.directory.remove_private("unexpected").unwrap();
    std::fs::OpenOptions::new()
        .write(true)
        .open(wallet.directory.path().join(staged))
        .unwrap()
        .set_len((wallet.maximum + AEAD_OVERHEAD + 1) as u64)
        .unwrap();
    assert!(wallet.writer().is_err());
    assert!(wallet.directory.path().join(staged).exists());
    wallet.directory.remove_private(staged).unwrap();
    wallet
        .directory
        .write_atomic(LOCK_NAME, b"not an empty native lock", PublishMode::Replace)
        .unwrap();
    assert!(matches!(wallet.writer(), Err(Error::Binding)));
    assert_eq!(
        std::fs::read(wallet.directory.path().join(STATE_NAME)).unwrap(),
        original
    );
}

#[test]
fn successful_cleanup_worker_keeps_original_round_custody_after_awaiter_cancellation() {
    use iroha::{
        client::run_native_worker,
        workflow::{FileWorkflowStorage, WorkflowId, WorkflowStorage},
    };
    use std::sync::Arc;
    let root = tempfile::tempdir().unwrap();
    let wallet = Arc::new(open_wallet(&root));
    let mut fixture = super::test_fixture::complete();
    stage_complete(&wallet, &mut fixture);
    let round = [87; 32];
    claim_complete(&wallet, &fixture, round);
    let path = root.path().join("journals");
    let storage = FileWorkflowStorage::open_or_create(&path).unwrap();
    let id = WorkflowId::from_bytes(domain_digest(
        b"iroha:private-settlement:proof-worker:v1\0",
        &[&round],
    ));
    let lease = storage.acquire_runner(&id).unwrap();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .max_blocking_threads(1)
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let permits = Arc::new(tokio::sync::Semaphore::new(1));
        let permit = permits.clone().acquire_owned().await.unwrap();
        let (entered, started) = tokio::sync::oneshot::channel();
        let (release, gate) = std::sync::mpsc::channel();
        let worker_wallet = wallet.clone();
        let work = tokio::spawn(async move {
            run_native_worker(permit, lease, move || {
                entered.send(()).unwrap();
                gate.recv().unwrap();
                worker_wallet
                    .reconcile_verified(round, &fixture.prepared, &fixture.receipt)
                    .unwrap();
            })
            .await
        });
        started.await.unwrap();
        work.abort();
        assert!(matches!(work.await, Err(error) if error.is_cancelled()));
        let reopened = FileWorkflowStorage::open_or_create(&path).unwrap();
        assert!(
            reopened.acquire_runner(&id).is_err(),
            "cold owner cannot outlive original cleanup worker"
        );
        assert!(
            wallet
                .load()
                .unwrap()
                .0
                .reservations
                .iter()
                .all(|row| row.pending.is_some())
        );
        release.send(()).unwrap();
        let permit = tokio::time::timeout(std::time::Duration::from_secs(5), permits.acquire())
            .await
            .unwrap()
            .unwrap();
        drop(permit);
        let _released_original = reopened.acquire_runner(&id).unwrap();
        assert!(
            wallet
                .load()
                .unwrap()
                .0
                .reservations
                .iter()
                .all(|row| row.pending.is_none() && row.completion.is_some())
        );
    });
}

#[test]
fn local_reconciliation_pending_preserves_present_carrier_progress_exactly() {
    use crate::PrivateSettlementReconciliationPending;
    use iroha::workflow::{
        PartialCompletionV1, WorkflowId, WorkflowKindV1, WorkflowProgressV1, WorkflowStatusV1,
    };
    let fixture = super::test_fixture::complete();
    let progress = WorkflowProgressV1 {
        workflow_id: WorkflowId::from_bytes([88; 32]),
        kind: WorkflowKindV1::PrivateSettlement,
        status: WorkflowStatusV1::Completed,
        sequence: 19,
        cancel_requested: false,
        atomicity: "per_transaction_only",
        current_step: None,
        failure_code: None,
        steps: vec![],
        partial_completion: PartialCompletionV1::default(),
    };
    let pending = PrivateSettlementReconciliationPending::new(
        VerifiedPrivateSettlementOutcome::Settled(fixture.receipt),
        Some(progress.clone()),
        OneCallPrivateSettlementError::WalletCustody(Error::Busy),
    );
    assert_eq!(pending.progress(), Some(&progress));
    assert!(matches!(
        pending.cause(),
        OneCallPrivateSettlementError::WalletCustody(Error::Busy)
    ));
    assert_eq!(pending.into_parts().1, Some(progress));
}
