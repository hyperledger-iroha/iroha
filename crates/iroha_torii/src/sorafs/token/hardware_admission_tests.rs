//! Fresh current-custody admission checks with real deterministic role and observer signatures.
use super::hardware_test_support::*;
use super::*;
use sorafs_manifest::signer::stream_token_evidence::{
    SignerStreamTokenObservationPhaseV1 as Phase,
    SignerStreamTokenObservationRequestSubjectV1 as Subject,
};

fn issued_for_admission(ttl_secs: u64) -> (StreamTokenIssuer, Arc<SignedFixture>, StreamTokenV1) {
    let fixture = SignedFixture::new(8, TestSignerMode::Sign);
    let issuer = fixture.issuer().unwrap();
    let key =
        iroha_crypto::KeyPair::try_from_seed(vec![0x77; 32], iroha_crypto::Algorithm::Ed25519)
            .unwrap();
    let issue = issuer
        .issue_token(
            StreamTokenQuotaSubject::from_authenticated_operator(key.public_key()),
            vec![0xAA],
            PROVIDER,
            "sorafs.sf1@1.0.0".into(),
            TokenOverrides {
                ttl_secs: Some(ttl_secs),
                ..TokenOverrides::default()
            },
        )
        .unwrap();
    issue.token.verify(issuer.verifying_key()).unwrap();
    (issuer, fixture, issue.token)
}
#[test]
fn serving_admission_uses_unique_current_challenges_and_never_signs_again() {
    let (issuer, fixture, token) = issued_for_admission(60);
    let before = fixture.observer_calls.load(Ordering::SeqCst);
    for _ in 0..2 {
        assert_eq!(issuer.before_admission(&token.body).unwrap(), NOW_MS);
    }
    let requests = fixture.requests.lock().unwrap();
    let admissions = &requests[before..];
    assert_eq!(admissions.len(), 2);
    for request in admissions {
        assert_eq!(request.phase, Phase::BeforeAdmission);
        assert!(matches!(request.subject, Subject::CurrentCustody { .. }));
    }
    assert_ne!(admissions[0].challenge, admissions[1].challenge);
    assert_eq!(fixture.calls.load(Ordering::SeqCst), 1);
    assert_eq!(fixture.recover_calls.load(Ordering::SeqCst), 0);
}
#[test]
fn serving_admission_rejects_signed_revocation_outage_staleness_and_substitution() {
    for fault in [
        ObserverFault::SignerRevoked,
        ObserverFault::AttesterRevoked,
        ObserverFault::Unavailable,
        ObserverFault::Stale,
        ObserverFault::WrongPhase,
        ObserverFault::WrongKey,
        ObserverFault::WrongRequest,
        ObserverFault::WrongFinality,
    ] {
        let (issuer, fixture, token) = issued_for_admission(60);
        let before = fixture.observer_calls.load(Ordering::SeqCst);
        fixture.faults.lock().unwrap().insert(before + 1, fault);
        assert!(issuer.before_admission(&token.body).is_err(), "{fault:?}");
        assert_eq!(fixture.observer_calls.load(Ordering::SeqCst), before + 1);
        assert_eq!(fixture.calls.load(Ordering::SeqCst), 1);
        assert_eq!(fixture.recover_calls.load(Ordering::SeqCst), 0);
    }
}
#[test]
fn serving_admission_rechecks_time_after_observer_and_finality_work() {
    for boundary in 0..4 {
        let (issuer, fixture, token) = issued_for_admission(1);
        let next = fixture.observer_calls.load(Ordering::SeqCst) + 1;
        let expiry = token.body.ttl_epoch * 1_000;
        match boundary {
            0 => {
                fixture
                    .observer_return_clock
                    .lock()
                    .unwrap()
                    .insert(next, expiry);
            }
            1 => {
                *fixture.finality.advance_clock.lock().unwrap() = Some((
                    fixture.finality.validations.load(Ordering::SeqCst) + 2,
                    fixture.clock.clone(),
                    expiry,
                ));
            }
            2 => {
                fixture
                    .observer_return_clock
                    .lock()
                    .unwrap()
                    .insert(next, NOW_MS - 1);
            }
            _ => {
                fixture
                    .observer_return_tip
                    .lock()
                    .unwrap()
                    .insert(next, 101);
            }
        }
        assert!(
            issuer.before_admission(&token.body).is_err(),
            "boundary {boundary}"
        );
        assert_eq!(fixture.calls.load(Ordering::SeqCst), 1);
        assert_eq!(fixture.recover_calls.load(Ordering::SeqCst), 0);
    }
}
#[test]
fn serving_admission_rejects_wrong_body_before_observer_io_and_returns_final_time() {
    let (issuer, fixture, token) = issued_for_admission(60);
    let before = fixture.observer_calls.load(Ordering::SeqCst);
    for field in 0..4 {
        let mut body = token.body.clone();
        match field {
            0 => body.provider_id[0] ^= 1,
            1 => body.token_pk_version += 1,
            2 => body.max_streams = 0,
            _ => {
                body.issued_at = u64::MAX - 1;
                body.ttl_epoch = u64::MAX;
            }
        }
        assert!(issuer.before_admission(&body).is_err());
    }
    assert_eq!(fixture.observer_calls.load(Ordering::SeqCst), before);
    *fixture.finality.advance_clock.lock().unwrap() = Some((
        fixture.finality.validations.load(Ordering::SeqCst) + 2,
        fixture.clock.clone(),
        NOW_MS + 100,
    ));
    assert_eq!(issuer.before_admission(&token.body).unwrap(), NOW_MS + 100);
}

#[test]
fn serving_admission_accepts_fresh_same_key_custody_renewal_without_resigning_token() {
    let (issuer, fixture, token) = issued_for_admission(60);
    let next = fixture.observer_calls.load(Ordering::SeqCst) + 1;
    fixture
        .faults
        .lock()
        .unwrap()
        .insert(next, ObserverFault::RenewedCustody);
    assert_eq!(issuer.before_admission(&token.body).unwrap(), NOW_MS);
    token.verify(issuer.verifying_key()).unwrap();
    assert_eq!(fixture.calls.load(Ordering::SeqCst), 1);
    assert_eq!(fixture.recover_calls.load(Ordering::SeqCst), 0);
}
