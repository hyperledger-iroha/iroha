//! Source-level lifecycle and canonical-frame controls, without installed Native qualification.

use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

#[test]
fn constructor_failure_consumes_registration_before_a_second_original_load() {
    let mut registration = StartupRegistration::new();
    let loads = AtomicUsize::new(0);
    let publications = AtomicUsize::new(0);
    let original = registration.claim().unwrap();
    let retirement = Arc::clone(&original.retirement);
    assert_eq!(
        original.install::<u8>(
            |_| {
                loads.fetch_add(1, Ordering::SeqCst);
                Err(Error::Rejected)
            },
            |_| {
                publications.fetch_add(1, Ordering::SeqCst);
                Ok(())
            },
        ),
        Err(Error::Rejected),
    );
    assert!(matches!(registration.claim(), Err(Error::Rejected)));
    assert_eq!(retirement.capture_original(), Err(Error::Rejected));
    assert_eq!(loads.load(Ordering::SeqCst), 1);
    assert_eq!(publications.load(Ordering::SeqCst), 0);
}

#[test]
fn publication_uncertainty_cannot_replace_the_same_original_owner() {
    let mut registration = StartupRegistration::new();
    let value = Arc::new(7_u8);
    let original = registration.claim().unwrap();
    let retirement = Arc::clone(&original.retirement);
    assert_eq!(
        original.install(
            |_| Ok(value.clone()),
            |published| {
                assert!(Arc::ptr_eq(&published, &value));
                Err(Error::Rejected)
            },
        ),
        Err(Error::Rejected),
    );
    assert!(matches!(registration.claim(), Err(Error::Rejected)));
    assert_eq!(retirement.capture_original(), Err(Error::Rejected));
}

#[test]
fn construction_and_publication_retain_the_exact_same_owner_once() {
    let mut registration = StartupRegistration::new();
    let value = Arc::new(9_u8);
    let original = registration.claim().unwrap();
    let returned = original
        .install(
            |_| Ok(value.clone()),
            |published| {
                assert!(Arc::ptr_eq(&published, &value));
                Ok(())
            },
        )
        .unwrap();
    assert!(Arc::ptr_eq(&returned, &value));
    assert!(matches!(registration.claim(), Err(Error::Rejected)));
}

#[test]
fn cleanup_without_composition_does_not_consume_the_first_account_claim() {
    let registration = Mutex::new(StartupRegistration::new());
    assert_eq!(retire_registered_composition(&registration), Ok(()));
    let original = registration.lock().unwrap().claim().unwrap();
    assert_eq!(original.require_current(), Ok(()));
    assert!(registration.lock().unwrap().attempted);
}

#[test]
fn retirement_before_first_constructor_read_refuses_io_and_publication() {
    let registration = Mutex::new(StartupRegistration::new());
    let original = registration.lock().unwrap().claim().unwrap();
    retire_registered_composition(&registration).unwrap();
    assert_eq!(
        original.install::<u8>(
            |_| panic!("Retired original must not start source or key I/O"),
            |_| panic!("Retired original must not publish"),
        ),
        Err(Error::Rejected)
    );
    assert!(matches!(
        registration.lock().unwrap().claim(),
        Err(Error::Rejected)
    ));
}

#[test]
fn retirement_during_constructor_io_does_not_wait_for_io_or_publish() {
    use std::sync::mpsc;
    let registration = Arc::new(Mutex::new(StartupRegistration::new()));
    let original = registration.lock().unwrap().claim().unwrap();
    let retirement = Arc::clone(&original.retirement);
    let (entered_tx, entered_rx) = mpsc::channel();
    let (complete_tx, complete_rx) = mpsc::channel();
    let worker = std::thread::spawn(move || {
        original.install::<u8>(
            |_| {
                entered_tx.send(()).unwrap();
                complete_rx.recv().unwrap();
                Ok(Arc::new(1))
            },
            |_| panic!("Retirement during I/O must refuse publication"),
        )
    });
    entered_rx.recv().unwrap();
    assert!(registration.try_lock().is_ok());
    assert!(retirement.publication.try_lock().is_ok());
    retire_registered_composition(&registration).unwrap();
    assert_eq!(retirement.capture_original(), Err(Error::Rejected));
    complete_tx.send(()).unwrap();
    assert_eq!(worker.join().unwrap(), Err(Error::Rejected));
    assert!(matches!(
        registration.lock().unwrap().claim(),
        Err(Error::Rejected)
    ));
}

#[test]
fn abandoned_intake_permanently_consumes_and_retires_the_same_original() {
    let mut registration = StartupRegistration::new();
    let original = registration.claim().unwrap();
    let retirement = Arc::clone(&original.retirement);
    drop(original);
    assert_eq!(retirement.capture_original(), Err(Error::Rejected));
    assert!(matches!(registration.claim(), Err(Error::Rejected)));
}

#[test]
fn lost_publication_result_denies_the_exact_already_published_original() {
    let mut registration = StartupRegistration::new();
    let original = registration.claim().unwrap();
    let retirement = Arc::clone(&original.retirement);
    let value = Arc::new(3_u8);
    let published = OnceLock::new();
    assert_eq!(
        original.install(
            |_| Ok(Arc::clone(&value)),
            |actual| {
                published.set(actual).unwrap();
                Err(Error::Rejected)
            },
        ),
        Err(Error::Rejected)
    );
    assert!(Arc::ptr_eq(published.get().unwrap(), &value));
    assert_eq!(retirement.capture_original(), Err(Error::Rejected));
    assert!(matches!(registration.claim(), Err(Error::Rejected)));
}

#[test]
fn published_original_retirement_cannot_be_revived_by_a_new_registry_selection() {
    let registration = Mutex::new(StartupRegistration::new());
    let original = registration.lock().unwrap().claim().unwrap();
    let retirement = Arc::clone(&original.retirement);
    let value = original
        .install(|actual| Ok(Arc::new(actual)), |_| Ok(()))
        .unwrap();
    assert!(Arc::ptr_eq(value.as_ref(), &retirement));
    retire_registered_composition(&registration).unwrap();
    assert_eq!(value.capture_original(), Err(Error::Rejected));
    let registry: SessionRegistry<AccountId, u8, ()> = SessionRegistry::new();
    registry.revoke_selection().unwrap();
    assert_eq!(retirement.require_original(0), Err(Error::Rejected));
    assert!(matches!(
        registration.lock().unwrap().claim(),
        Err(Error::Rejected)
    ));
}

#[test]
fn publication_and_retirement_share_one_atomic_completion_boundary() {
    use std::sync::mpsc;
    let mut registration = StartupRegistration::new();
    let original = registration.claim().unwrap();
    let retirement = Arc::clone(&original.retirement);
    let published = Arc::new(OnceLock::new());
    let held = Arc::clone(&published);
    let (entered_tx, entered_rx) = mpsc::channel();
    let (complete_tx, complete_rx) = mpsc::channel();
    let worker = std::thread::spawn(move || {
        original.install::<u8>(
            |_| Ok(Arc::new(5)),
            |value| {
                // A deterministic control pauses at the otherwise I/O-free STARTUP.set boundary.
                entered_tx.send(()).unwrap();
                complete_rx.recv().unwrap();
                held.set(value).unwrap();
                Ok(())
            },
        )
    });
    entered_rx.recv().unwrap();
    assert!(retirement.publication.try_lock().is_err());
    assert_eq!(retirement.capture_original(), Ok(0));
    let retiring = Arc::clone(&retirement);
    let cleanup = std::thread::spawn(move || retiring.retire());
    complete_tx.send(()).unwrap();
    let result = worker.join().unwrap();
    assert!(result.is_ok() || result == Err(Error::Rejected));
    cleanup.join().unwrap().unwrap();
    assert_eq!(published.get().unwrap().as_ref(), &5);
    assert_eq!(retirement.capture_original(), Err(Error::Rejected));
    assert!(matches!(registration.claim(), Err(Error::Rejected)));
}

#[test]
fn poisoned_publication_still_denies_original_and_reports_cleanup_failure() {
    let mut registration = StartupRegistration::new();
    let original = registration.claim().unwrap();
    let retirement = Arc::clone(&original.retirement);
    let unknown = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        original.install::<u8>(
            |_| Ok(Arc::new(1)),
            |_| panic!("Unknown publication outcome"),
        )
    }));
    assert!(unknown.is_err());
    assert_eq!(retirement.capture_original(), Err(Error::Rejected));
    assert_eq!(retirement.retire(), Err(Error::Rejected));
    assert!(matches!(registration.claim(), Err(Error::Rejected)));
}

#[test]
fn initial_acquisition_unknown_result_freezes_all_lifecycle_reentry() {
    for failure in [Error::Unavailable, Error::Rejected] {
        let mut acquisition = InitialAcquisition::new();
        assert_eq!(acquisition.require_callable(), Ok(()));
        assert_eq!(
            acquisition.acquire(|| panic!("No current owner exists yet"), || Err(failure)),
            Err(failure)
        );
        assert_eq!(acquisition.require_callable(), Err(Error::Rejected));
        assert_eq!(
            acquisition.acquire(
                || panic!("Unknown original cannot become current"),
                || panic!("Unknown original cannot reserve another read")
            ),
            Err(Error::Rejected)
        );
    }
}

#[test]
fn initial_completed_retry_rechecks_real_current_custody_without_new_read() {
    let mut acquisition = InitialAcquisition::new();
    let reads = AtomicUsize::new(0);
    acquisition
        .acquire(
            || panic!("Initial path must acquire"),
            || {
                reads.fetch_add(1, Ordering::SeqCst);
                Ok(())
            },
        )
        .unwrap();
    assert_eq!(acquisition.require_callable(), Ok(()));
    acquisition
        .acquire(|| Ok(()), || panic!("Same original must not be read twice"))
        .unwrap();
    assert_eq!(
        acquisition.acquire(
            || Err(Error::Rejected),
            || panic!("Expired current custody cannot refresh from installation retry")
        ),
        Err(Error::Rejected)
    );
    assert_eq!(reads.load(Ordering::SeqCst), 1);
}

#[test]
fn uncertain_initial_acquisition_allows_retirement_without_reopening_authority() {
    let mut acquisition = InitialAcquisition::new();
    let reads = AtomicUsize::new(0);
    assert_eq!(
        acquisition.acquire(
            || panic!("No completed owner exists"),
            || {
                reads.fetch_add(1, Ordering::SeqCst);
                Err(Error::Rejected)
            },
        ),
        Err(Error::Rejected),
    );
    // A lost original must not leave its pending/current Native selection alive on logout.
    // These phases can only cancel, close or revoke existing registry entries.
    for phase in [3, 4, 5] {
        assert_eq!(acquisition.require_phase(phase), Ok(()));
        assert_eq!(acquisition.require_callable(), Err(Error::Rejected));
    }
    for phase in [1, 2, 6] {
        assert_eq!(acquisition.require_phase(phase), Err(Error::Rejected));
    }
    assert_eq!(
        acquisition.acquire(
            || panic!("Cleanup cannot convert uncertainty to completion"),
            || panic!("Cleanup cannot select another original read"),
        ),
        Err(Error::Rejected),
    );
    assert_eq!(reads.load(Ordering::SeqCst), 1);
}

#[test]
fn initial_acquisition_phase_gate_refuses_unknown_selectors_in_every_state() {
    let idle = InitialAcquisition::new();
    let complete = InitialAcquisition {
        attempted: true,
        complete: true,
    };
    let uncertain = InitialAcquisition {
        attempted: true,
        complete: false,
    };
    for phase in 0..=u8::MAX {
        if !(1..=6).contains(&phase) {
            for state in [&idle, &complete, &uncertain] {
                assert_eq!(state.require_phase(phase), Err(Error::Rejected));
            }
        }
    }
    for phase in 1..=6 {
        assert_eq!(idle.require_phase(phase), Ok(()));
        assert_eq!(complete.require_phase(phase), Ok(()));
    }
}

#[test]
fn active_initial_acquisition_cannot_hold_retirement_behind_its_mutex() {
    let acquisition = Mutex::new(InitialAcquisition::new());
    let mut in_progress = acquisition.lock().unwrap();
    in_progress.attempted = true;
    for phase in [3, 4, 5] {
        assert_eq!(require_startup_phase(&acquisition, phase), Ok(()));
    }
    for phase in [0, 1, 2, 6, 7, u8::MAX] {
        assert_eq!(
            require_startup_phase(&acquisition, phase),
            Err(Error::Rejected)
        );
    }
    drop(in_progress);
    // Ending the attempt does not clear an unknown result, even after cleanup was permitted.
    for phase in [1, 2, 6] {
        assert_eq!(
            require_startup_phase(&acquisition, phase),
            Err(Error::Rejected)
        );
    }
    for phase in [3, 4, 5] {
        assert_eq!(require_startup_phase(&acquisition, phase), Ok(()));
    }
}

#[test]
fn logout_during_initial_read_revokes_the_original_native_preparation() {
    let acquisition = Mutex::new(InitialAcquisition {
        attempted: true,
        complete: false,
    });
    let in_progress = acquisition.lock().unwrap();
    let registry = SessionRegistry::<u8, u8, u8>::new();
    let owner = Arc::new(Mutex::new(9_u8));
    let deadline = NativeDeadlineV1::start(Duration::from_secs(10)).unwrap();
    let result = registry.begin(deadline, |permit| {
        permit.require_current()?;
        assert_eq!(require_startup_phase(&acquisition, 5), Ok(()));
        registry.revoke_selection()?;
        assert_eq!(permit.require_current(), Err(RegistryError::Rejected));
        // Even an otherwise identical completed read cannot republish this retired selection.
        Ok((7, owner.clone(), 3))
    });
    assert_eq!(result, Err(RegistryError::Rejected));
    drop(in_progress);
    assert_eq!(require_startup_phase(&acquisition, 1), Err(Error::Rejected));
}

#[test]
fn logout_before_registry_permit_refuses_original_initial_publication() {
    let retirement = StartupRetirement::new();
    // Production captures this before its first package/path check and before begin exists.
    let generation = retirement.capture_original().unwrap();
    let acquisition = Mutex::new(InitialAcquisition::new());
    let mut in_progress = acquisition.lock().unwrap();
    let registry = SessionRegistry::<u8, u8, u8>::new();
    let owner = Arc::new(Mutex::new(9_u8));
    let publications = AtomicUsize::new(0);
    assert_eq!(
        in_progress.acquire_original(
            &retirement,
            generation,
            || panic!("No completed original exists"),
            || {
                // Logout completes while acquisition owns its mutex, before registry.begin.
                assert_eq!(require_startup_phase(&acquisition, 5), Ok(()));
                retirement.retire().unwrap();
                registry.revoke_selection().unwrap();
                let deadline = NativeDeadlineV1::start(Duration::from_secs(10)).unwrap();
                let result = registry.begin(deadline, |permit| {
                    // A fresh registry permit alone would succeed after that logout.
                    permit.require_current()?;
                    retirement
                        .require_original(generation)
                        .map_err(|_| RegistryError::Rejected)?;
                    publications.fetch_add(1, Ordering::SeqCst);
                    Ok((7, owner.clone(), 3))
                });
                assert_eq!(result, Err(RegistryError::Rejected));
                result.map(|_| ()).map_err(|_| Error::Rejected)
            },
        ),
        Err(Error::Rejected),
    );
    assert_eq!(publications.load(Ordering::SeqCst), 0);
    assert!(in_progress.attempted);
    assert!(!in_progress.complete);
    assert_eq!(in_progress.require_phase(1), Err(Error::Rejected));
    assert_eq!(retirement.capture_original(), Err(Error::Rejected));
}

#[test]
fn completed_logout_refuses_first_initial_acquisition_on_retained_original() {
    let retirement = StartupRetirement::new();
    retirement.retire().unwrap();
    let mut acquisition = InitialAcquisition::new();
    assert_eq!(retirement.capture_original(), Err(Error::Rejected));
    assert_eq!(
        acquisition.acquire_original(
            &retirement,
            0,
            || panic!("Retired original cannot reuse a current owner"),
            || panic!("Retired original cannot perform its first source read"),
        ),
        Err(Error::Rejected),
    );
    assert!(!acquisition.attempted);
    assert!(!acquisition.complete);
    for phase in [3, 4, 5] {
        assert_eq!(acquisition.require_phase(phase), Ok(()));
    }
}

#[test]
fn retirement_at_acquisition_completion_preserves_unknown_result_fence() {
    let retirement = StartupRetirement::new();
    let generation = retirement.capture_original().unwrap();
    let mut acquisition = InitialAcquisition::new();
    assert_eq!(
        acquisition.acquire_original(
            &retirement,
            generation,
            || panic!("Initial acquisition must run"),
            || {
                retirement.retire()?;
                Ok(())
            },
        ),
        Err(Error::Rejected),
    );
    assert!(acquisition.attempted);
    assert!(!acquisition.complete);
    for phase in [1, 2, 6] {
        assert_eq!(acquisition.require_phase(phase), Err(Error::Rejected));
    }
    assert_eq!(
        acquisition.acquire_original(
            &retirement,
            generation,
            || panic!("Retirement cannot clear uncertainty"),
            || panic!("Retirement cannot reserve another original"),
        ),
        Err(Error::Rejected),
    );
}

#[test]
fn retired_original_is_not_revived_by_new_same_owner_registry_selection() {
    let retirement = StartupRetirement::new();
    let generation = retirement.capture_original().unwrap();
    let registry = SessionRegistry::<u8, u8, u8>::new();
    let owner = Arc::new(Mutex::new(9_u8));
    let deadline = NativeDeadlineV1::start(Duration::from_secs(10)).unwrap();
    let old_id = registry
        .begin(deadline, |_| Ok((7, owner.clone(), 3)))
        .unwrap();
    let old_completion = registry.take_completion(old_id).unwrap();
    let old_handle = registry
        .finish(
            old_completion,
            Ok,
            |_, verified| Ok(verified),
            |_, _| Ok(()),
        )
        .unwrap();
    retirement.retire().unwrap();
    registry.revoke_selection().unwrap();
    assert_eq!(
        registry.require_current_handle(old_handle),
        Err(RegistryError::Rejected),
    );
    let deadline = NativeDeadlineV1::start(Duration::from_secs(10)).unwrap();
    let id = registry
        .begin(deadline, |_| Ok((7, owner.clone(), 3)))
        .unwrap();
    let completion = registry.take_completion(id).unwrap();
    let handle = registry
        .finish(completion, Ok, |_, verified| Ok(verified), |_, _| Ok(()))
        .unwrap();
    assert_eq!(registry.require_current_handle(handle), Ok(()));
    // The actual installed BoundNativeAccountSession checks its retained generation as well.
    assert_eq!(
        retirement.require_original(generation),
        Err(Error::Rejected)
    );
    assert_eq!(retirement.capture_original(), Err(Error::Rejected));
}

#[test]
fn startup_public_frames_roundtrip_exact_phases_ids_and_complete_original() {
    for (phase, id, original) in [
        (1, 0, vec![]),
        (2, 81, vec![1, 2, 3]),
        (3, 81, vec![]),
        (4, 92, vec![]),
        (5, 0, vec![]),
        (6, 81, vec![]),
    ] {
        let request = KagemushaOrdinaryNativeStartupRequestV1 {
            version: 1,
            phase,
            id,
            original,
        };
        let frame = norito::encode_canonical(&request).unwrap();
        let decoded: KagemushaOrdinaryNativeStartupRequestV1 =
            norito::decode_canonical(&frame).unwrap();
        assert_eq!(decoded.version, request.version);
        assert_eq!(decoded.phase, request.phase);
        assert_eq!(decoded.id, request.id);
        assert_eq!(decoded.original, request.original);
        assert_eq!(norito::encode_canonical(&decoded).unwrap(), frame);
        assert!(
            norito::decode_canonical::<KagemushaOrdinaryNativeStartupRequestV1>(
                &frame[..frame.len() - 1]
            )
            .is_err()
        );
        let response = KagemushaOrdinaryNativeStartupResponseV1 {
            version: 1,
            phase,
            id,
            fields: if phase == 1 {
                vec![vec![7; 32], vec![8; 64], vec![9; 64]]
            } else {
                vec![]
            },
        };
        let frame = norito::encode_canonical(&response).unwrap();
        let decoded: KagemushaOrdinaryNativeStartupResponseV1 =
            norito::decode_canonical(&frame).unwrap();
        assert_eq!(decoded.version, response.version);
        assert_eq!(decoded.phase, response.phase);
        assert_eq!(decoded.id, response.id);
        assert_eq!(decoded.fields, response.fields);
        assert_eq!(norito::encode_canonical(&decoded).unwrap(), frame);
    }
}
