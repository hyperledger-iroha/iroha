//! Real signed final-promotion receipts over simulated hardware and durable-operation authority.

use super::super::{
    final_promotion::*,
    journal::{SignerReceiptJournalV1, SignerReceiptPurposeV1},
};
use super::*;
use sorafs_manifest::signer::{
    final_promotion::{
        SIGNER_FINAL_PROMOTION_STATEMENT_MAX_BYTES_V1, SignerFinalPromotionExpectedV1,
        SignerFinalPromotionReceiptErrorV1, signer_final_promotion_digest_v1,
        statement::prepare_final_promotion_statement_v1, verify_final_promotion_signer_receipt_v1,
    },
    protocol::signer_operation_signatures_digest_v1,
    receipt::{SignerCompletedOperationV1, SignerOperationFinalizedAnchorV1, SignerReceiptErrorV1},
};
use std::{fs, os::unix::fs::PermissionsExt as _, path::Path};

mod lifecycle;

mod signed_fixture {
    use sorafs_manifest as manifest;
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../sorafs_manifest/src/signer/final_promotion/tests/statement_fixture_support.rs"
    ));
}

fn private_directory() -> tempfile::TempDir {
    let directory = tempfile::tempdir().unwrap();
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700)).unwrap();
    directory
}
fn promotion_fixture() -> Fixture {
    fixture_for(
        SignerRoleV1::FinalPromotionProvenance,
        SignerPurposeBindingV1::FinalPromotionProvenance {
            deployment_id: "production-primary".into(),
        },
    )
}
fn reviewed(message: &[u8]) -> SignerFinalPromotionExpectedV1 {
    SignerFinalPromotionExpectedV1 {
        operation_id: [0x91; 32],
        statement_digest: signer_final_promotion_digest_v1(message),
        statement_size: message.len() as u64,
    }
}
fn forbid_constructor_source_io(fixture: &Fixture) {
    let mut state = fixture.source.state.lock().unwrap();
    state.fail_observe = true;
    // observe() consumes this marker before returning its injected failure. This makes even an
    // ignored premature observation detectable without changing the shared test source.
    state.transition_commits = 1;
    state.mutate_on_transition_observe = Some(Mutation::SignerRevoked);
}
fn assert_no_constructor_io(source: &Source, provider: &Provider, path: &Path) {
    let state = source.state.lock().unwrap();
    assert!(state.mutate_on_transition_observe.is_some());
    assert!(!state.context.signer_revoked);
    assert_eq!(state.signing_reads, 0);
    assert_eq!(state.reserved_reads, 0);
    assert_eq!(state.commits, 0);
    assert!(state.used_ids.is_empty());
    assert_eq!(provider.calls.load(Ordering::SeqCst), 0);
    assert_eq!(fs::read_dir(path).unwrap().count(), 0);
}
fn ceremony(
    directory: &Path,
) -> (
    SignerFinalPromotionServiceV1,
    Arc<Source>,
    Arc<Provider>,
    Vec<u8>,
) {
    let fixture = promotion_fixture();
    let message = signed_fixture::statement_message(&fixture.source.binding);
    fixture.source.state.lock().unwrap().expected_journal = Some(directory.to_owned());
    let service = SignerFinalPromotionServiceV1::new(
        fixture.coordinator,
        reviewed(&message),
        Arc::from(message.clone()),
        SignerReceiptJournalV1::open(directory, SignerReceiptPurposeV1::FinalPromotionProvenance)
            .unwrap(),
    )
    .unwrap();
    (service, fixture.source, fixture.provider, message)
}

#[test]
fn final_promotion_signs_durably_and_public_verification_matches_read_only_recovery() {
    let directory = private_directory();
    let path = directory.path().canonicalize().unwrap();
    let (service, source, provider, message) = ceremony(&path);
    let receipt = service.sign().unwrap();
    assert_eq!(provider.calls.load(Ordering::SeqCst), 4);
    let bytes = norito::encode_canonical(&receipt).unwrap();
    assert_eq!(
        fs::read(path.join(format!(
            "{}.receipt.norito",
            hex::encode(reviewed(&message).operation_id)
        )))
        .unwrap(),
        bytes
    );
    let state = source.state.lock().unwrap();
    assert_eq!(state.journal_checked, 1);
    assert_eq!(state.signing_reads, 1);
    let completion = SignerCompletedOperationV1 {
        operation_id: receipt.intent.operation_id,
        intent_digest: receipt.intent.digest().unwrap(),
        original_custody: receipt.request.original_custody,
        reservation: receipt.reservation,
        commitment: receipt.commitment,
        signatures_digest: signer_operation_signatures_digest_v1(&receipt.signatures).unwrap(),
        completed_at_unix_ms: state.context.now_unix_ms,
        anchor: SignerOperationFinalizedAnchorV1 {
            height: state.context.current_anchor.height,
            block_hash: state.context.current_anchor.block_hash,
            operation_state_digest: [0x92; 32],
        },
    };
    verify_final_promotion_signer_receipt_v1(
        &bytes,
        &message,
        &receipt.signatures[0].signature,
        &reviewed(&message),
        &source.binding,
        &promotion_fixture().coordinator.trust,
        &state.context,
        &completion,
    )
    .expect("actual daemon output satisfies the sole shared receipt verifier");
    drop(state);
    assert_eq!(service.recover().unwrap(), receipt);
    assert_eq!(
        source.state.lock().unwrap().signing_reads,
        1,
        "recovery must not obtain a newer signing predecessor"
    );
    assert_eq!(provider.calls.load(Ordering::SeqCst), 4);
    assert!(service.sign().is_err());
    assert_eq!(
        provider.calls.load(Ordering::SeqCst),
        4,
        "completed IDs cannot sign again"
    );
}

#[test]
fn final_promotion_uses_fresh_audit_head_and_rejects_a_racing_predecessor() {
    for race in [false, true] {
        let directory = private_directory();
        let path = directory.path().canonicalize().unwrap();
        let (service, source, provider, _message) = ceremony(&path);
        let head = SignerOperationAuditHeadV1 {
            sequence: 7,
            digest: [0xa9; 32],
        };
        {
            let mut state = source.state.lock().unwrap();
            state.audit = head;
            state.advance_audit_after_signing_snapshot = race;
        }
        if race {
            assert_eq!(
                service.sign().unwrap_err(),
                SignerFinalPromotionErrorV1::Operation(SignerOperationErrorV1::ReservationConflict)
            );
            assert_eq!(provider.calls.load(Ordering::SeqCst), 0);
            assert_eq!(fs::read_dir(&path).unwrap().count(), 0);
        } else {
            assert_eq!(service.sign().unwrap().intent.previous_audit, head);
            assert_eq!(provider.calls.load(Ordering::SeqCst), 4);
        }
    }
}

#[test]
fn final_promotion_rejects_exactly_reviewed_but_noncanonical_statement_before_io() {
    let directory = private_directory();
    let path = directory.path().canonicalize().unwrap();
    let fixture = promotion_fixture();
    let mut message = signed_fixture::statement_message(&fixture.source.binding);
    message.push(b'\n');
    forbid_constructor_source_io(&fixture);
    let error = SignerFinalPromotionServiceV1::new(
        fixture.coordinator,
        reviewed(&message),
        Arc::from(message.clone()),
        SignerReceiptJournalV1::open(&path, SignerReceiptPurposeV1::FinalPromotionProvenance)
            .unwrap(),
    )
    .unwrap_err();
    assert_eq!(
        error,
        SignerFinalPromotionErrorV1::Receipt(SignerFinalPromotionReceiptErrorV1::InvalidStatement)
    );
    assert_no_constructor_io(&fixture.source, &fixture.provider, &path);
}

#[test]
fn final_promotion_wrong_reviewed_digest_or_size_never_observes_reserves_or_signs() {
    for wrong_digest in [false, true] {
        let directory = private_directory();
        let path = directory.path().canonicalize().unwrap();
        let fixture = promotion_fixture();
        let message = signed_fixture::statement_message(&fixture.source.binding);
        let mut expected = reviewed(&message);
        if wrong_digest {
            expected.statement_digest[0] ^= 1;
        } else {
            expected.statement_size += 1;
        }
        assert_ne!(expected, reviewed(&message));
        forbid_constructor_source_io(&fixture);
        let error = SignerFinalPromotionServiceV1::new(
            fixture.coordinator,
            expected,
            Arc::from(message),
            SignerReceiptJournalV1::open(&path, SignerReceiptPurposeV1::FinalPromotionProvenance)
                .unwrap(),
        )
        .unwrap_err();
        assert_eq!(
            error,
            SignerFinalPromotionErrorV1::Receipt(
                SignerFinalPromotionReceiptErrorV1::StatementMismatch
            )
        );
        assert_no_constructor_io(&fixture.source, &fixture.provider, &path);
    }
}

#[test]
fn final_promotion_constructor_rejects_foreign_journal_role_and_empty_coordinates() {
    for failure in 0..6 {
        let directory = private_directory();
        let path = directory.path().canonicalize().unwrap();
        let fixture = if failure == 1 {
            fixture()
        } else {
            promotion_fixture()
        };
        let mut expected = SignerFinalPromotionExpectedV1 {
            operation_id: [1; 32],
            statement_digest: [2; 32],
            statement_size: 1,
        };
        if failure == 2 {
            expected.statement_size = 0;
        }
        if failure == 3 {
            expected.operation_id = [0; 32];
        }
        if failure == 4 {
            expected.statement_digest = [0; 32];
        }
        if failure == 5 {
            expected.statement_size = SIGNER_FINAL_PROMOTION_STATEMENT_MAX_BYTES_V1 as u64 + 1;
        }
        forbid_constructor_source_io(&fixture);
        let purpose = if failure == 0 {
            SignerReceiptPurposeV1::ReleaseManifest
        } else {
            SignerReceiptPurposeV1::FinalPromotionProvenance
        };
        let result = SignerFinalPromotionServiceV1::new(
            fixture.coordinator,
            expected,
            Arc::from([b'x']),
            SignerReceiptJournalV1::open(&path, purpose).unwrap(),
        )
        .unwrap_err();
        assert_eq!(
            result,
            match failure {
                0 => SignerFinalPromotionErrorV1::Journal,
                1 => SignerFinalPromotionErrorV1::Receipt(
                    SignerFinalPromotionReceiptErrorV1::WrongPurpose
                ),
                _ => SignerFinalPromotionErrorV1::Receipt(
                    SignerFinalPromotionReceiptErrorV1::InvalidReceipt
                ),
            }
        );
        assert_no_constructor_io(&fixture.source, &fixture.provider, &path);
    }
}

#[test]
fn final_promotion_constructor_rejects_canonical_foreign_binding_before_io() {
    for field in 0..3 {
        let directory = private_directory();
        let path = directory.path().canonicalize().unwrap();
        let fixture = promotion_fixture();
        let mut foreign = fixture.source.binding.clone();
        match field {
            0 => {
                foreign.purpose = SignerPurposeBindingV1::FinalPromotionProvenance {
                    deployment_id: "production-secondary".into(),
                };
            }
            1 => foreign.network_id[0] ^= 1,
            _ => foreign.policy_digest[0] ^= 1,
        }
        assert_ne!(foreign, fixture.source.binding);
        let message = signed_fixture::statement_message(&foreign);
        forbid_constructor_source_io(&fixture);
        let error = SignerFinalPromotionServiceV1::new(
            fixture.coordinator,
            reviewed(&message),
            Arc::from(message),
            SignerReceiptJournalV1::open(&path, SignerReceiptPurposeV1::FinalPromotionProvenance)
                .unwrap(),
        )
        .unwrap_err();
        assert_eq!(
            error,
            SignerFinalPromotionErrorV1::Receipt(
                SignerFinalPromotionReceiptErrorV1::InvalidStatement
            )
        );
        assert_no_constructor_io(&fixture.source, &fixture.provider, &path);
    }
}

#[test]
fn final_promotion_constructor_checks_fresh_custody_after_validating_the_pinned_statement() {
    let directory = private_directory();
    let path = directory.path().canonicalize().unwrap();
    let fixture = promotion_fixture();
    let message = signed_fixture::statement_message(&fixture.source.binding);
    forbid_constructor_source_io(&fixture);
    let error = SignerFinalPromotionServiceV1::new(
        fixture.coordinator,
        reviewed(&message),
        Arc::from(message),
        SignerReceiptJournalV1::open(&path, SignerReceiptPurposeV1::FinalPromotionProvenance)
            .unwrap(),
    )
    .unwrap_err();
    assert_eq!(
        error,
        SignerFinalPromotionErrorV1::Operation(SignerOperationErrorV1::StateUnavailable)
    );
    let state = fixture.source.state.lock().unwrap();
    assert!(state.mutate_on_transition_observe.is_none());
    assert!(state.context.signer_revoked);
    assert_eq!(state.signing_reads, 0);
    assert!(state.used_ids.is_empty());
    assert_eq!(fixture.provider.calls.load(Ordering::SeqCst), 0);
    assert_eq!(fs::read_dir(path).unwrap().count(), 0);
}

#[test]
fn final_promotion_sign_and_recovery_use_only_the_constructor_pinned_statement() {
    let directory = private_directory();
    let path = directory.path().canonicalize().unwrap();
    let fixture = promotion_fixture();
    let original = signed_fixture::statement_message(&fixture.source.binding);
    let mut caller_message = original.clone();
    let mut caller_statement: Arc<[u8]> = Arc::from(caller_message.clone());
    let mut caller_expected = reviewed(&original);
    fixture.source.state.lock().unwrap().expected_journal = Some(path.clone());
    let service = SignerFinalPromotionServiceV1::new(
        fixture.coordinator,
        caller_expected,
        Arc::clone(&caller_statement),
        SignerReceiptJournalV1::open(&path, SignerReceiptPurposeV1::FinalPromotionProvenance)
            .unwrap(),
    )
    .unwrap();
    assert!(Arc::get_mut(&mut caller_statement).is_none());
    let marker = b"\"aggregate_checker_sha256\":\"";
    let position = caller_message
        .windows(marker.len())
        .position(|part| part == marker)
        .unwrap()
        + marker.len();
    assert_ne!(caller_message[position], b'1');
    assert_ne!(caller_message[position], b'2');
    Arc::make_mut(&mut caller_statement)[position] = b'1';
    caller_message[position] = b'2';
    caller_expected.operation_id[0] ^= 1;
    caller_expected.statement_digest[0] ^= 1;
    caller_expected.statement_size += 1;
    prepare_final_promotion_statement_v1(&caller_statement, &fixture.source.binding)
        .expect("caller Arc now holds another canonical statement");
    prepare_final_promotion_statement_v1(&caller_message, &fixture.source.binding)
        .expect("caller vector now holds a third canonical statement");
    let receipt = service.sign().unwrap();
    assert_eq!(
        receipt.request.operation_id,
        reviewed(&original).operation_id
    );
    assert_eq!(
        receipt.request.statement_digest,
        reviewed(&original).statement_digest
    );
    assert_eq!(receipt.request.statement_size, original.len() as u64);
    assert_ne!(receipt.request.operation_id, caller_expected.operation_id);
    assert_ne!(
        receipt.request.statement_digest,
        caller_expected.statement_digest
    );
    assert_ne!(
        receipt.request.statement_size,
        caller_expected.statement_size
    );
    let signature = Signature::from_bytes(&receipt.signatures[0].signature);
    signature
        .verify(&fixture.source.binding.public_key, &original)
        .unwrap();
    assert!(
        signature
            .verify(&fixture.source.binding.public_key, &caller_statement)
            .is_err()
    );
    assert!(
        signature
            .verify(&fixture.source.binding.public_key, &caller_message)
            .is_err()
    );
    drop(caller_statement);
    drop(caller_message);
    assert_eq!(service.recover().unwrap(), receipt);
    assert_eq!(fixture.provider.calls.load(Ordering::SeqCst), 4);
    assert_eq!(fixture.source.state.lock().unwrap().commits, 1);
}

#[test]
fn final_promotion_provider_failures_never_release_or_retry() {
    for fault in [
        ProviderFault::WrongKey,
        ProviderFault::WrongMessage,
        ProviderFault::Unavailable,
        ProviderFault::Mutate(Mutation::SignerRevoked),
    ] {
        let directory = private_directory();
        let path = directory.path().canonicalize().unwrap();
        let (service, source, provider, _message) = ceremony(&path);
        *provider.fault.lock().unwrap() = Some(fault);
        assert!(service.sign().is_err());
        assert_eq!(provider.calls.load(Ordering::SeqCst), 1);
        assert!(service.recover().is_err());
        assert!(service.sign().is_err());
        assert_eq!(provider.calls.load(Ordering::SeqCst), 1);
        assert_eq!(source.state.lock().unwrap().commits, 0);
        assert_eq!(fs::read_dir(path).unwrap().count(), 0);
    }
}

#[test]
fn final_promotion_failed_commit_retains_unrecoverable_tombstone() {
    let directory = private_directory();
    let path = directory.path().canonicalize().unwrap();
    let (service, source, provider, _message) = ceremony(&path);
    source.state.lock().unwrap().fail_commit = true;
    assert!(service.sign().is_err());
    assert_eq!(provider.calls.load(Ordering::SeqCst), 4);
    assert_eq!(fs::read_dir(path).unwrap().count(), 1);
    assert!(service.recover().is_err());
    assert!(service.sign().is_err());
    assert_eq!(provider.calls.load(Ordering::SeqCst), 4);
    assert_eq!(source.state.lock().unwrap().commits, 0);
}

#[test]
fn final_promotion_post_commit_journal_or_custody_changes_prevent_release() {
    for failure in 0..4 {
        let directory = private_directory();
        let path = directory.path().canonicalize().unwrap();
        let (service, source, provider, _message) = ceremony(&path);
        {
            let mut state = source.state.lock().unwrap();
            match failure {
                0 => state.mutate_journal_after_commit = true,
                1 => state.mutate_after_commit = Some(Mutation::SignerRevoked),
                2 => state.mutate_at_release = Some(Mutation::AttesterRevoked),
                _ => state.mutate_journal_at_release = true,
            }
        }
        assert!(service.sign().is_err());
        assert_eq!(source.state.lock().unwrap().commits, 1);
        assert!(service.recover().is_err());
        assert_eq!(provider.calls.load(Ordering::SeqCst), 4);
    }
}

#[test]
fn final_promotion_recovery_rejects_tampered_receipt_and_replaced_completion() {
    for tamper_receipt in [false, true] {
        let directory = private_directory();
        let path = directory.path().canonicalize().unwrap();
        let (service, source, provider, _message) = ceremony(&path);
        let mut receipt = service.sign().unwrap();
        if tamper_receipt {
            receipt.signatures[0].signature[0] ^= 1;
            let record = path.join(format!(
                "{}.receipt.norito",
                hex::encode(receipt.intent.operation_id)
            ));
            fs::set_permissions(&record, fs::Permissions::from_mode(0o600)).unwrap();
            fs::write(&record, norito::encode_canonical(&receipt).unwrap()).unwrap();
            fs::set_permissions(&record, fs::Permissions::from_mode(0o400)).unwrap();
        } else {
            source.state.lock().unwrap().completed.as_mut().unwrap().3[0] ^= 1;
        }
        assert!(service.recover().is_err());
        assert_eq!(provider.calls.load(Ordering::SeqCst), 4);
        assert_eq!(source.state.lock().unwrap().signing_reads, 1);
    }
}

#[test]
fn final_promotion_shared_signature_error_is_wrapped_without_losing_its_class() {
    let error = SignerFinalPromotionErrorV1::from(SignerReceiptErrorV1::InvalidSignature);
    assert_eq!(
        error,
        SignerFinalPromotionErrorV1::Receipt(SignerFinalPromotionReceiptErrorV1::Operation(
            SignerReceiptErrorV1::InvalidSignature
        ))
    );
    assert_eq!(error.to_string(), "final promotion receipt rejected");
    assert_eq!(
        SignerFinalPromotionErrorV1::from(SignerOperationErrorV1::ProviderUnavailable).to_string(),
        "final promotion signer operation rejected"
    );
}
