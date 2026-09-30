//! Timely committed operations survive delayed finality without weakening current custody checks.

use super::*;

#[derive(Clone, Copy)]
enum DelayPhase {
    CommitResult,
    AfterCommit,
    BeforeRelease,
}

fn delay_observation(state: &mut SourceState, phase: DelayPhase, now: u64) {
    let mutation = Mutation::ObservationTime { now, observed: now };
    match phase {
        DelayPhase::CommitResult => state.mutate_after_commit = Some(mutation),
        DelayPhase::AfterCommit => {
            state.mutate_at_completed =
                Some((SignerCommittedObservationPhaseV1::AfterCommit, mutation));
        }
        DelayPhase::BeforeRelease => state.mutate_at_release = Some(mutation),
    }
}

#[test]
fn timely_completion_observed_at_or_after_expiry_releases_the_original_signatures() {
    for action in [
        SignerOperationActionV1::Sign,
        SignerOperationActionV1::Qualify,
        SignerOperationActionV1::Status,
    ] {
        for phase in [
            DelayPhase::CommitResult,
            DelayPhase::AfterCommit,
            DelayPhase::BeforeRelease,
        ] {
            for now in [1_800, 1_801, 1_999] {
                let fixture = fixture();
                let mut operation = fixture.coordinator.begin(intent(action)).unwrap();
                stage(&mut operation);
                let reservation = operation.reservation;
                let original_custody = SignerOperationCustodyV1::from_verified(&operation.custody);
                let signatures: Vec<_> = operation
                    .signatures
                    .iter()
                    .map(|signature| (signature.purpose, signature.signature.to_vec()))
                    .collect();
                delay_observation(&mut fixture.source.state.lock().unwrap(), phase, now);

                let completed = operation
                    .finish(commitment())
                    .expect("timely completion may finalize after reservation expiry");
                assert_eq!(completed.custody().verified_at_unix_ms(), now);
                assert_eq!(completed.reservation(), reservation);
                assert_eq!(completed.original_custody(), original_custody);
                assert_eq!(completed.commitment(), commitment());
                assert_eq!(completed.intent_digest(), intent(action).digest().unwrap());
                for (purpose, bytes) in &signatures {
                    assert_eq!(completed.signature(*purpose), Some(bytes.as_slice()));
                }
                assert_eq!(
                    fixture.provider.calls.load(Ordering::Relaxed),
                    signatures.len()
                );
                let state = fixture.source.state.lock().unwrap();
                assert_eq!(state.commits, 1);
                assert_eq!(state.used_ids.len(), 1);
                assert!(state.reservation.is_none());
                assert!(state.completed.is_some());
                assert_eq!(state.audit, commitment().audit);
                assert_eq!(
                    state.reserved_phases.last(),
                    Some(&SignerReservedObservationPhaseV1::BeforeCommit)
                );
                assert_eq!(
                    state.completed_phases,
                    [
                        SignerCommittedObservationPhaseV1::AfterCommit,
                        SignerCommittedObservationPhaseV1::BeforeRelease,
                    ]
                );
            }
        }
    }
}

#[test]
fn reservation_expiry_before_completion_still_withholds_all_signatures() {
    for now in [1_800, 1_801] {
        let fixture = fixture();
        let mut operation = fixture
            .coordinator
            .begin(intent(SignerOperationActionV1::Sign))
            .unwrap();
        stage(&mut operation);
        fixture
            .source
            .state
            .lock()
            .unwrap()
            .mutate(Mutation::ObservationTime { now, observed: now });
        assert_eq!(
            operation.finish(commitment()).unwrap_err(),
            SignerOperationErrorV1::ReservationConflict
        );
        assert_eq!(fixture.provider.calls.load(Ordering::Relaxed), 4);
        let state = fixture.source.state.lock().unwrap();
        assert_eq!(state.commits, 0);
        assert!(state.completed.is_none());
        assert!(state.reservation.is_some());
        assert_eq!(state.used_ids.len(), 1);
        assert!(state.completed_phases.is_empty());
    }
}

#[test]
fn delayed_completed_observations_still_reject_custody_drift_revocation_and_stale_time() {
    for phase in [
        SignerCommittedObservationPhaseV1::AfterCommit,
        SignerCommittedObservationPhaseV1::BeforeRelease,
    ] {
        for mutation in [
            Mutation::Control,
            Mutation::Record,
            Mutation::SignerRevoked,
            Mutation::AttesterRevoked,
            Mutation::TimeBackwards,
            Mutation::SameHeightFork,
            Mutation::ObservationTime {
                now: 2_000,
                observed: 2_000,
            },
            Mutation::ObservationTime {
                now: 1_801,
                observed: 1_600,
            },
            Mutation::ObservationTime {
                now: 1_801,
                observed: 1_802,
            },
        ] {
            let fixture = fixture();
            let mut operation = fixture
                .coordinator
                .begin(intent(SignerOperationActionV1::Sign))
                .unwrap();
            stage(&mut operation);
            {
                let mut state = fixture.source.state.lock().unwrap();
                delay_observation(&mut state, DelayPhase::CommitResult, 1_801);
                state.mutate_at_completed = Some((phase, mutation));
            }
            assert!(operation.finish(commitment()).is_err());
            assert_eq!(fixture.provider.calls.load(Ordering::Relaxed), 4);
            let state = fixture.source.state.lock().unwrap();
            assert_eq!(state.commits, 1);
            assert!(state.completed.is_some());
            assert!(state.reservation.is_none());
            assert_eq!(state.completed_phases.last(), Some(&phase));
        }
    }
}

#[test]
fn delayed_finality_never_substitutes_for_the_exact_durable_completed_row() {
    for phase in [
        SignerCommittedObservationPhaseV1::AfterCommit,
        SignerCommittedObservationPhaseV1::BeforeRelease,
    ] {
        let fixture = fixture();
        let mut operation = fixture
            .coordinator
            .begin(intent(SignerOperationActionV1::Sign))
            .unwrap();
        stage(&mut operation);
        {
            let mut state = fixture.source.state.lock().unwrap();
            delay_observation(&mut state, DelayPhase::CommitResult, 1_801);
            state.mutate_at_completed = Some((phase, Mutation::LostCompletion));
        }
        assert_eq!(
            operation.finish(commitment()).unwrap_err(),
            SignerOperationErrorV1::ReservationConflict
        );
        assert!(matches!(
            fixture
                .coordinator
                .begin(intent(SignerOperationActionV1::Sign)),
            Err(SignerOperationErrorV1::ReservationConflict)
        ));
        assert_eq!(fixture.provider.calls.load(Ordering::Relaxed), 4);
        let state = fixture.source.state.lock().unwrap();
        assert_eq!(state.commits, 1);
        assert_eq!(state.used_ids.len(), 1);
        assert!(state.reservation.is_none());
        assert_eq!(state.completed_phases.last(), Some(&phase));
    }
}

#[test]
fn unavailable_completed_observation_after_expiry_never_retries_the_operation() {
    for phase in [
        SignerCommittedObservationPhaseV1::AfterCommit,
        SignerCommittedObservationPhaseV1::BeforeRelease,
    ] {
        let fixture = fixture();
        let mut operation = fixture
            .coordinator
            .begin(intent(SignerOperationActionV1::Sign))
            .unwrap();
        stage(&mut operation);
        {
            let mut state = fixture.source.state.lock().unwrap();
            delay_observation(&mut state, DelayPhase::CommitResult, 1_801);
            state.fail_completed_phase = Some(phase);
        }
        assert_eq!(
            operation.finish(commitment()).unwrap_err(),
            SignerOperationErrorV1::StateUnavailable
        );
        assert!(matches!(
            fixture
                .coordinator
                .begin(intent(SignerOperationActionV1::Sign)),
            Err(SignerOperationErrorV1::ReservationConflict)
        ));
        assert_eq!(fixture.provider.calls.load(Ordering::Relaxed), 4);
        let state = fixture.source.state.lock().unwrap();
        assert_eq!(state.commits, 1);
        assert!(state.completed.is_some());
        assert_eq!(state.used_ids.len(), 1);
    }
}

#[test]
fn delayed_finality_release_preserves_exact_recovery_identity_without_more_provider_io() {
    let fixture = fixture();
    let mut operation = fixture
        .coordinator
        .begin(intent(SignerOperationActionV1::Sign))
        .unwrap();
    stage(&mut operation);
    delay_observation(
        &mut fixture.source.state.lock().unwrap(),
        DelayPhase::CommitResult,
        1_801,
    );
    let original = operation.finish(commitment()).unwrap();
    let recovered = fixture
        .coordinator
        .recover_completed(super::recovery::recovered(&original))
        .unwrap();
    assert_eq!(recovered.original_custody(), original.original_custody());
    assert_eq!(recovered.reservation(), original.reservation());
    assert_eq!(recovered.commitment(), original.commitment());
    assert_eq!(recovered.intent_digest(), original.intent_digest());
    assert_eq!(recovered.signatures_digest(), original.signatures_digest());
    for purpose in [
        SignerKeyOperationPurposeV1::RolePayload,
        SignerKeyOperationPurposeV1::AuditRecord,
        SignerKeyOperationPurposeV1::Provenance,
        SignerKeyOperationPurposeV1::Response,
    ] {
        assert_eq!(recovered.signature(purpose), original.signature(purpose));
    }
    assert_eq!(fixture.provider.calls.load(Ordering::Relaxed), 4);
    let state = fixture.source.state.lock().unwrap();
    assert_eq!(state.commits, 1);
    assert_eq!(state.used_ids.len(), 1);
    assert!(state.reservation.is_none());
}
