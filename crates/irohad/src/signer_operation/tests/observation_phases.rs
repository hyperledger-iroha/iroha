//! Simulated observation failures fence signatures at each exact operation boundary.

use super::*;

#[test]
fn every_key_call_and_durable_release_requires_its_own_observation_phase() {
    use SignerReservedObservationPhaseV1::{AfterProvider, BeforeCommit, BeforeProvider};
    let fixture = fixture();
    let mut operation = fixture
        .coordinator
        .begin(intent(SignerOperationActionV1::Sign))
        .unwrap();
    stage(&mut operation);
    let completed = operation.finish(commitment()).unwrap();
    assert!(
        completed
            .signature(SignerKeyOperationPurposeV1::RolePayload)
            .is_some()
    );
    assert_eq!(fixture.provider.calls.load(Ordering::Relaxed), 4);
    let state = fixture.source.state.lock().unwrap();
    assert_eq!(
        state.reserved_phases,
        [
            BeforeProvider,
            BeforeProvider,
            AfterProvider,
            BeforeProvider,
            AfterProvider,
            BeforeProvider,
            AfterProvider,
            BeforeProvider,
            AfterProvider,
            BeforeCommit
        ]
    );
    assert_eq!(
        state.completed_phases,
        [
            SignerCommittedObservationPhaseV1::AfterCommit,
            SignerCommittedObservationPhaseV1::BeforeRelease
        ]
    );
    assert_eq!(state.commits, 1);
}

#[test]
fn initial_reserved_observation_failure_keeps_tombstone_without_key_use() {
    let fixture = fixture();
    fixture.source.state.lock().unwrap().fail_reserved_phase =
        Some(SignerReservedObservationPhaseV1::BeforeProvider);
    assert!(matches!(
        fixture
            .coordinator
            .begin(intent(SignerOperationActionV1::Sign)),
        Err(SignerOperationErrorV1::StateUnavailable)
    ));
    {
        let mut state = fixture.source.state.lock().unwrap();
        assert_eq!(state.used_ids.len(), 1);
        assert!(state.reservation.is_some());
        assert_eq!(state.commits, 0);
        state.fail_reserved_phase = None;
    }
    assert!(matches!(
        fixture
            .coordinator
            .begin(intent(SignerOperationActionV1::Sign)),
        Err(SignerOperationErrorV1::ReservationConflict)
    ));
    assert_eq!(fixture.provider.calls.load(Ordering::Relaxed), 0);
}

#[test]
fn before_and_after_provider_failures_poison_without_releasing_or_retrying_signatures() {
    for (phase, calls) in [
        (SignerReservedObservationPhaseV1::BeforeProvider, 0),
        (SignerReservedObservationPhaseV1::AfterProvider, 1),
    ] {
        let fixture = fixture();
        let mut operation = fixture
            .coordinator
            .begin(intent(SignerOperationActionV1::Sign))
            .unwrap();
        fixture.source.state.lock().unwrap().fail_reserved_phase = Some(phase);
        assert_eq!(
            operation
                .sign(SignerKeyOperationPurposeV1::RolePayload, b"payload")
                .unwrap_err(),
            SignerOperationErrorV1::StateUnavailable
        );
        assert!(operation.signatures.is_empty());
        fixture.source.state.lock().unwrap().fail_reserved_phase = None;
        assert_eq!(
            operation
                .sign(SignerKeyOperationPurposeV1::RolePayload, b"payload")
                .unwrap_err(),
            SignerOperationErrorV1::Poisoned
        );
        assert_eq!(
            operation.finish(commitment()).unwrap_err(),
            SignerOperationErrorV1::Poisoned
        );
        assert_eq!(fixture.provider.calls.load(Ordering::Relaxed), calls);
        let state = fixture.source.state.lock().unwrap();
        assert_eq!(state.commits, 0);
        assert_eq!(state.used_ids.len(), 1);
        assert!(state.reservation.is_some());
        assert!(state.completed.is_none());
    }
}

#[test]
fn before_commit_failure_preserves_pending_operation_without_completion() {
    let fixture = fixture();
    let mut operation = fixture
        .coordinator
        .begin(intent(SignerOperationActionV1::Sign))
        .unwrap();
    stage(&mut operation);
    fixture.source.state.lock().unwrap().fail_reserved_phase =
        Some(SignerReservedObservationPhaseV1::BeforeCommit);
    assert_eq!(
        operation.finish(commitment()).unwrap_err(),
        SignerOperationErrorV1::StateUnavailable
    );
    assert_eq!(fixture.provider.calls.load(Ordering::Relaxed), 4);
    let state = fixture.source.state.lock().unwrap();
    assert_eq!(state.commits, 0);
    assert!(state.completed.is_none());
    assert!(state.reservation.is_some());
    assert!(state.completed_phases.is_empty());
}

#[test]
fn either_completed_phase_failure_withholds_output_after_durable_commit() {
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
        fixture.source.state.lock().unwrap().fail_completed_phase = Some(phase);
        assert_eq!(
            operation.finish(commitment()).unwrap_err(),
            SignerOperationErrorV1::StateUnavailable
        );
        {
            let mut state = fixture.source.state.lock().unwrap();
            assert_eq!(state.commits, 1);
            assert!(state.completed.is_some());
            assert!(state.reservation.is_none());
            assert_eq!(state.completed_phases.last(), Some(&phase));
            state.fail_completed_phase = None;
        }
        assert!(matches!(
            fixture
                .coordinator
                .begin(intent(SignerOperationActionV1::Sign)),
            Err(SignerOperationErrorV1::ReservationConflict)
        ));
        assert_eq!(fixture.provider.calls.load(Ordering::Relaxed), 4);
    }
}

#[test]
fn recovery_requires_both_fresh_completed_phases_without_sign_or_commit() {
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
        let original = operation.finish(commitment()).unwrap();
        {
            let mut state = fixture.source.state.lock().unwrap();
            state.completed_phases.clear();
            state.fail_completed_phase = Some(phase);
        }
        assert_eq!(
            fixture
                .coordinator
                .recover_completed(super::recovery::recovered(&original))
                .unwrap_err(),
            SignerOperationErrorV1::StateUnavailable
        );
        {
            let mut state = fixture.source.state.lock().unwrap();
            assert_eq!(state.completed_phases.last(), Some(&phase));
            state.completed_phases.clear();
            state.fail_completed_phase = None;
        }
        let recovered = fixture
            .coordinator
            .recover_completed(super::recovery::recovered(&original))
            .unwrap();
        assert_eq!(recovered.commitment(), original.commitment());
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
        assert_eq!(
            state.completed_phases,
            [
                SignerCommittedObservationPhaseV1::AfterCommit,
                SignerCommittedObservationPhaseV1::BeforeRelease
            ]
        );
    }
}
