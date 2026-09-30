//! Exact completed-response recovery rejects forged history and never repeats provider I/O.

use super::*;

fn completed(fixture: &Fixture) -> CompletedSignerOperationV1 {
    let mut operation = fixture
        .coordinator
        .begin(intent(SignerOperationActionV1::Sign))
        .expect("reserve");
    stage(&mut operation);
    operation
        .finish(commitment())
        .expect("durable original completion")
}
pub(super) fn recovered(completed: &CompletedSignerOperationV1) -> RecoveredSignerOperationV1 {
    let signatures = [
        SignerKeyOperationPurposeV1::RolePayload,
        SignerKeyOperationPurposeV1::AuditRecord,
        SignerKeyOperationPurposeV1::Provenance,
        SignerKeyOperationPurposeV1::Response,
    ]
    .into_iter()
    .map(|purpose| {
        let message = match purpose {
            SignerKeyOperationPurposeV1::RolePayload => b"sensitive-operation-payload".to_vec(),
            SignerKeyOperationPurposeV1::AuditRecord => {
                commitment().audit.signing_message().to_vec()
            }
            SignerKeyOperationPurposeV1::Provenance => vec![0x73; 32],
            SignerKeyOperationPurposeV1::Response => {
                commitment().response_signing_message().to_vec()
            }
        };
        RecoveredSignerSignatureV1 {
            purpose,
            message: Zeroizing::new(message),
            signature: Zeroizing::new(
                completed
                    .signature(purpose)
                    .expect("original signature")
                    .to_vec(),
            ),
        }
    })
    .collect();
    RecoveredSignerOperationV1 {
        original_custody: completed.original_custody(),
        intent: intent(SignerOperationActionV1::Sign),
        reservation: completed.reservation(),
        commitment: completed.commitment(),
        signatures,
    }
}

#[test]
fn exact_response_recovery_after_reservation_expiry_does_not_reserve_commit_or_sign_again() {
    let fixture = fixture();
    let original = completed(&fixture);
    {
        let mut state = fixture.source.state.lock().expect("lock");
        state.context.now_unix_ms = 1_801;
        state.context.anchor_observed_at_unix_ms = 1_801;
    }
    let result = fixture
        .coordinator
        .recover_completed(recovered(&original))
        .expect("existing timely completion remains recoverable");
    assert_eq!(result.commitment(), original.commitment());
    assert_eq!(result.reservation(), original.reservation());
    assert_eq!(result.signatures_digest(), original.signatures_digest());
    assert_eq!(result.intent_digest(), original.intent_digest());
    assert_eq!(result.original_custody(), original.original_custody());
    assert_eq!(
        result.signature(SignerKeyOperationPurposeV1::RolePayload),
        original.signature(SignerKeyOperationPurposeV1::RolePayload)
    );
    assert_eq!(result.custody().verified_at_unix_ms(), 1_801);
    assert_eq!(fixture.provider.calls.load(Ordering::Relaxed), 4);
    let state = fixture.source.state.lock().expect("lock");
    assert_eq!(state.commits, 1);
    assert_eq!(state.used_ids.len(), 1);
    assert!(state.reservation.is_none());
}

#[test]
fn valid_signatures_without_the_exact_authoritative_completed_row_cannot_be_recovered() {
    let fixture = fixture();
    let original = completed(&fixture);
    let other = super::fixture();
    assert!(
        other
            .coordinator
            .recover_completed(recovered(&original))
            .is_err()
    );
    assert_eq!(other.provider.calls.load(Ordering::Relaxed), 0);
    fixture.source.state.lock().expect("lock").completed = None;
    assert!(
        fixture
            .coordinator
            .recover_completed(recovered(&original))
            .is_err()
    );
    assert_eq!(fixture.provider.calls.load(Ordering::Relaxed), 4);
}

#[test]
fn recovery_rejects_substituted_request_reservation_response_audit_message_or_signature() {
    let mutations: &[fn(&mut RecoveredSignerOperationV1)] = &[
        |value| value.original_custody.record_digest[0] ^= 1,
        |value| value.original_custody.control_state_digest[0] ^= 1,
        |value| value.intent.operation_id[0] ^= 1,
        |value| value.intent.request_digest[0] ^= 1,
        |value| value.reservation.fence += 1,
        |value| value.reservation.reservation_id[0] ^= 1,
        |value| value.reservation.expires_at_unix_ms -= 1,
        |value| value.commitment.response_digest[0] ^= 1,
        |value| value.commitment.audit.digest[0] ^= 1,
        |value| value.signatures[0].message[0] ^= 1,
        |value| value.signatures[0].signature[0] ^= 1,
        |value| value.signatures.swap(0, 1),
        |value| {
            value.signatures.pop();
        },
        |value| value.intent.action = SignerOperationActionV1::RevokeCustody,
    ];
    for mutate in mutations {
        let fixture = fixture();
        let original = completed(&fixture);
        let mut value = recovered(&original);
        mutate(&mut value);
        assert!(fixture.coordinator.recover_completed(value).is_err());
        assert_eq!(fixture.provider.calls.load(Ordering::Relaxed), 4);
        assert_eq!(fixture.source.state.lock().expect("lock").commits, 1);
    }
}

#[test]
fn recovery_revalidates_current_custody_including_changes_during_history_observation() {
    for mutation in [
        Mutation::Control,
        Mutation::Record,
        Mutation::SignerRevoked,
        Mutation::AttesterRevoked,
        Mutation::TimeBackwards,
        Mutation::SameHeightFork,
    ] {
        let fixture = fixture();
        let original = completed(&fixture);
        fixture.source.state.lock().expect("lock").mutate_at_release = Some(mutation);
        assert!(
            fixture
                .coordinator
                .recover_completed(recovered(&original))
                .is_err()
        );
        assert_eq!(fixture.provider.calls.load(Ordering::Relaxed), 4);
    }
    let fixture = fixture();
    let original = completed(&fixture);
    {
        let mut state = fixture.source.state.lock().expect("lock");
        state.context.now_unix_ms = 2_000;
        state.context.anchor_observed_at_unix_ms = 2_000;
    }
    assert!(
        fixture
            .coordinator
            .recover_completed(recovered(&original))
            .is_err()
    );
    assert_eq!(fixture.provider.calls.load(Ordering::Relaxed), 4);
}
