//! Source-level lifecycle and canonical-frame controls, without installed Native qualification.

use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

#[test]
fn constructor_failure_consumes_registration_before_a_second_original_load() {
    let mut registration = StartupRegistration::new();
    let loads = AtomicUsize::new(0);
    let publications = AtomicUsize::new(0);
    assert_eq!(
        registration.install::<u8>(
            || {
                loads.fetch_add(1, Ordering::SeqCst);
                Err(Error::Rejected)
            },
            |_| {
                publications.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }
        ),
        Err(Error::Rejected)
    );
    assert_eq!(
        registration.install(
            || {
                loads.fetch_add(1, Ordering::SeqCst);
                Ok(Arc::new(1))
            },
            |_| {
                publications.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }
        ),
        Err(Error::Rejected)
    );
    assert_eq!(loads.load(Ordering::SeqCst), 1);
    assert_eq!(publications.load(Ordering::SeqCst), 0);
}

#[test]
fn publication_uncertainty_cannot_replace_the_same_original_owner() {
    let mut registration = StartupRegistration::new();
    let original = Arc::new(7_u8);
    assert_eq!(
        registration.install(
            || Ok(original.clone()),
            |published| {
                assert!(Arc::ptr_eq(&published, &original));
                Err(Error::Rejected)
            }
        ),
        Err(Error::Rejected)
    );
    assert_eq!(
        registration.install(
            || panic!("Must not construct another Native owner"),
            |_: Arc<u8>| panic!("Must not replace an unknown publication")
        ),
        Err(Error::Rejected)
    );
}

#[test]
fn construction_and_publication_retain_the_exact_same_owner_once() {
    let mut registration = StartupRegistration::new();
    let original = Arc::new(9_u8);
    let returned = registration
        .install(
            || Ok(original.clone()),
            |published| {
                assert!(Arc::ptr_eq(&published, &original));
                Ok(())
            },
        )
        .unwrap();
    assert!(Arc::ptr_eq(&returned, &original));
    assert_eq!(
        registration.install(
            || panic!("Already constructed"),
            |_: Arc<u8>| panic!("Already installed")
        ),
        Err(Error::Rejected)
    );
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
