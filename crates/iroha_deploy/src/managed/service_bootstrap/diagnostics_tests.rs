//! Closed startup observations preserve refusal classes without retaining private inputs.

use super::*;

#[test]
fn capacity_refusals_report_only_exact_closed_reasons() {
    for (message, expected) in [
        (
            "fresh capacity predecessor unavailable",
            ErrorClass::CapacityPredecessor,
        ),
        (
            "fresh capacity declaration predecessor unavailable; retain original intent",
            ErrorClass::CapacityPredecessor,
        ),
        (
            "provider capacity declaration preparation failed; retain original intent and journal",
            ErrorClass::CapacityPreparation,
        ),
        (
            "provider capacity declaration transaction unresolved; recover its original journal",
            ErrorClass::CapacityRecovery,
        ),
    ] {
        assert_eq!(ErrorClass::of(&Error::Invalid(message.into())), expected);
        // A prefix, suffix, or embedded remote/private detail is never retained.
        for altered in [
            format!("{message}: PRIVATE_KEY_AND_REMOTE_BODY"),
            format!("remote: {message}"),
        ] {
            assert_eq!(
                ErrorClass::of(&Error::Invalid(altered)),
                ErrorClass::Invalid
            );
        }
    }
}

#[test]
fn closed_error_classes_discard_paths_bodies_and_signing_material() {
    let secret = "PRIVATE_KEY_AND_REMOTE_BODY_51c7";
    let errors = [
        Error::Invalid(secret.into()),
        Error::RuntimeMismatch(secret.into()),
        Error::Busy(secret.into()),
        Error::Io(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            secret,
        )),
        Error::ContractCall {
            journal: secret.into(),
            source: color_eyre::eyre::eyre!(secret),
        },
        Error::WorkerFailure {
            failure: secret.into(),
            cleanup: Some(Box::new(Error::Invalid(secret.into()))),
            publication: None,
        },
        Error::Bootstrap(ManagedBootstrapFailure::SignedUnresolved),
        Error::NativeDeadline,
    ];
    let expected = [
        ErrorClass::Invalid,
        ErrorClass::RuntimeMismatch,
        ErrorClass::Busy,
        ErrorClass::Io(std::io::ErrorKind::PermissionDenied),
        ErrorClass::ContractCall,
        ErrorClass::WorkerFailure,
        ErrorClass::Bootstrap(ManagedBootstrapFailure::SignedUnresolved),
        ErrorClass::NativeDeadline,
    ];
    let progress = Progress::default();
    for (error, expected) in errors.iter().zip(expected) {
        assert_eq!(ErrorClass::of(error), expected);
        progress.parent_begin(true);
        progress.parent_finish::<()>(&Err(Error::Invalid(secret.into())));
        let message = progress.snapshot().unwrap().message();
        assert!(!message.contains(secret));
        assert!(!format!("{:?}", ErrorClass::of(error)).contains(secret));
    }
    assert_eq!(progress.lock()[0].unwrap().failures, 8);
}

#[test]
fn independent_worker_progress_retains_last_error_during_next_attempt() {
    let progress = Progress::default();
    assert!(progress.snapshot().is_none());
    std::thread::scope(|scope| {
        for slot in 0..3 {
            let progress = &progress;
            scope.spawn(move || {
                progress.begin(Stage::Custody, Mode::Advance, Some(slot));
                progress.finish(
                    Stage::Custody,
                    Some(slot),
                    Outcome::Error(ErrorClass::CheckpointHttp(
                        iroha::http::PublicHttpError::Status(503),
                    )),
                );
                progress.begin(Stage::Custody, Mode::Advance, Some(slot));
                progress.finish(Stage::Custody, Some(slot), Outcome::Complete);
                progress.begin(Stage::Account, Mode::Advance, Some(slot));
            });
        }
    });
    let snapshot = progress.snapshot().unwrap();
    for slot in 2..5 {
        let value = snapshot.0[slot].unwrap();
        assert_eq!(value.calls, 3);
        assert_eq!(value.attempts, 2);
        assert_eq!(progress.lock()[slot].unwrap().failures, 1);
        assert_eq!(value.stage, Stage::Account);
        assert_eq!(value.outcome, State::Running);
        assert_eq!(
            value.last_error,
            Some(ErrorClass::CheckpointHttp(
                iroha::http::PublicHttpError::Status(503)
            ))
        );
    }
    assert!(snapshot.0[..2].iter().all(Option::is_none));
    let message = snapshot.message();
    assert!(message.contains("provider0=Advance/Account,calls=3,attempts=2"));
    assert!(message.contains("Status(503)"));
}

#[test]
fn diagnostics_are_bounded_and_preserve_pending_rejection() {
    let progress = Progress::default();
    progress.begin(Stage::Funding, Mode::Advance, Some(0));
    {
        let value = &mut progress.lock()[2];
        let value = value.as_mut().unwrap();
        value.calls = u16::MAX;
        value.failures = u16::MAX;
        value.attempts = u16::MAX;
    }
    progress.begin(Stage::Custody, Mode::Advance, Some(0));
    progress.finish(Stage::Custody, Some(0), Outcome::Error(ErrorClass::Invalid));
    progress.begin(Stage::Funding, Mode::Advance, Some(0));
    progress.finish(
        Stage::Funding,
        Some(0),
        Outcome::Funding(FundingStep::Credit, OperationStatus::Rejected),
    );
    let before = progress.snapshot().unwrap();
    assert_eq!(before.0[2].unwrap().calls, u16::MAX);
    assert_eq!(before.0[2].unwrap().attempts, u16::MAX);
    assert_eq!(progress.lock()[2].unwrap().failures, u16::MAX);
    assert!(before.message().contains("Funding(Credit, Rejected)"));
    progress.begin(Stage::Account, Mode::Advance, Some(usize::MAX));
    progress.finish(Stage::Account, Some(usize::MAX), Outcome::Complete);
    assert_eq!(before, progress.snapshot().unwrap());
    // Even every fixed slot at maximum scalar values stays inside the public summary bound.
    let maximal = Summary {
        elapsed_ms: u32::MAX,
        ..before.0[2].unwrap()
    };
    assert!(Snapshot([Some(maximal); 5]).message().len() < 2048);
    assert!(std::mem::size_of::<Snapshot>() <= 112);
}
