//! Public deterministic-signature C component tests; they grant no installed issuer authority.
use super::*;
use iroha_crypto::{Algorithm, KeyPair, Signature};
use iroha_data_model::{
    kagemusha::KagemushaDevicePublicKeyV1,
    testing::ordinary_app_enrollment::KagemushaOrdinaryRetailEnrollmentFixtureV1 as Fixture,
};
use p256::ecdsa::SigningKey;
use std::os::unix::fs::PermissionsExt as _;

struct PrivateTestRoot {
    _directory: tempfile::TempDir,
    canonical: std::path::PathBuf,
}
impl PrivateTestRoot {
    fn path(&self) -> &Path {
        &self.canonical
    }
}
fn private_root() -> PrivateTestRoot {
    let dir = tempfile::tempdir().unwrap();
    std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    // Darwin TMPDIR may contain /var -> /private/var. The production journal's
    // component-by-component NOFOLLOW walk correctly rejects that offered spelling.
    let canonical = dir.path().canonicalize().unwrap();
    PrivateTestRoot {
        _directory: dir,
        canonical,
    }
}
fn selected(f: &Fixture, now: u64) -> Arc<KagemushaOrdinaryPreparationSelectedOriginalsV1> {
    let key = SigningKey::from_bytes((&[9; 32]).into()).unwrap();
    let core = KagemushaDevicePublicKeyV1::from_sec1_bytes(
        key.verifying_key().to_encoded_point(false).as_bytes(),
    )
    .unwrap();
    Arc::new(
        KagemushaOrdinaryPreparationSelectedOriginalsV1::from_selected_originals(
            f.selection.owner.clone(),
            f.release.clone(),
            f.issuer_policy.clone(),
            Arc::clone(&f.ordinary_policy),
            f.trust.clone(),
            f.app_authority.clone(),
            f.selection.preparation.challenge.hardware_profile_id,
            &core,
            now,
        )
        .unwrap(),
    )
}
fn carrier(f: &Fixture) -> KagemushaOrdinaryPreparationCarrierV1 {
    KagemushaOrdinaryPreparationCarrierV1 {
        account_i105: f.selection.owner.account_id.canonical_i105().unwrap(),
        client_nonce: [91; 32],
        release_id: f.release.release_id(),
        hardware_profile_id: f.selection.preparation.challenge.hardware_profile_id,
        lane_id: f.selection.owner.lane_id,
        financial_authority_commitment: [92; 32],
    }
}
fn signed(owner: &KagemushaOrdinaryIssuerPreparationAttemptV1, seed: u8) -> Vec<u8> {
    let (challenge, pin) = KagemushaOrdinaryAppEnrollmentChallengeV1::from_signing_request(
        owner.signing_request().unwrap(),
    )
    .unwrap();
    let issuer = KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519);
    if seed == 63 {
        assert_eq!(pin.as_slice(), issuer.public_key().to_bytes().1);
    }
    let signature = Signature::new(
        issuer.private_key(),
        &challenge.canonical_signing_bytes().unwrap(),
    );
    KagemushaSignedOrdinaryAppEnrollmentChallengeV1 {
        challenge,
        signature,
    }
    .to_transport_bytes()
    .unwrap()
}
#[test]
fn issuer_c_reserves_before_signing_and_recovers_identical_lost_result_for_both_platforms() {
    for apple in [false, true] {
        let f = Fixture::new(apple);
        let root = private_root();
        let scope = selected(&f, 300);
        let carrier = carrier(&f);
        let mut owner = KagemushaOrdinaryIssuerPreparationAttemptV1::reserve(
            root.path(),
            scope.clone(),
            carrier.clone(),
            [5; 32],
        )
        .unwrap();
        assert_eq!(owner.journal.recovery_prefix().unwrap().sequence, 1);
        let request = owner.signing_request().unwrap().to_vec();
        assert!(
            KagemushaOrdinaryIssuerPreparationAttemptV1::reserve(
                root.path(),
                scope.clone(),
                carrier.clone(),
                [5; 32]
            )
            .is_err()
        );
        assert!(
            KagemushaOrdinaryIssuerPreparationAttemptV1::open_existing(
                root.path(),
                scope.clone(),
                carrier.clone(),
                [5; 32]
            )
            .is_err()
        );
        let original = signed(&owner, 63);
        assert_eq!(
            owner.publish_original(&original, [5; 32]).unwrap(),
            original
        );
        assert!(owner.signing_request().is_err());
        assert_eq!(
            owner.publish_original(&original, [5; 32]).unwrap(),
            original
        );
        assert_eq!(owner.journal.recovery_prefix().unwrap().sequence, 2);
        drop(owner);
        let recovered = KagemushaOrdinaryIssuerPreparationAttemptV1::open_existing(
            root.path(),
            scope,
            carrier,
            [5; 32],
        )
        .unwrap();
        assert_eq!(recovered.signing_request, request);
        assert_eq!(recovered.published_original([5; 32]).unwrap(), original);
        assert!(recovered.signing_request().is_err());
    }
}
#[test]
fn issuer_c_refuses_changed_body_scope_key_and_signed_subject_before_publication() {
    let f = Fixture::android_with_integrity();
    let root = private_root();
    let scope = selected(&f, 300);
    let carrier = carrier(&f);
    let mut owner = KagemushaOrdinaryIssuerPreparationAttemptV1::reserve(
        root.path(),
        scope.clone(),
        carrier.clone(),
        [5; 32],
    )
    .unwrap();
    let original = signed(&owner, 63);
    assert!(owner.publish_original(&original, [6; 32]).is_err());
    for other_seed in [61, 62] {
        let wrong_key_original = signed(&owner, other_seed);
        assert!(
            owner
                .publish_original(&wrong_key_original, [5; 32])
                .is_err()
        );
    }
    for change in 0..8 {
        let mut foreign =
            KagemushaSignedOrdinaryAppEnrollmentChallengeV1::from_transport_bytes(&original)
                .unwrap();
        match change {
            0 => foreign.challenge.account_binding[0] ^= 1,
            1 => foreign.challenge.network_id[0] ^= 1,
            2 => foreign.challenge.hardware_profile_id[0] ^= 1,
            3 => foreign.challenge.client_nonce[0] ^= 1,
            4 => foreign.challenge.server_nonce[0] ^= 1,
            5 => foreign.challenge.financial_authority_commitment[0] ^= 1,
            6 => foreign.challenge.hardware_epoch += 1,
            _ => foreign.challenge.expires_at_ms -= 1,
        }
        let key = KeyPair::from_seed(vec![63; 32], Algorithm::Ed25519);
        foreign.signature = Signature::new(
            key.private_key(),
            &foreign.challenge.canonical_signing_bytes().unwrap(),
        );
        assert!(
            owner
                .publish_original(&foreign.to_transport_bytes().unwrap(), [5; 32])
                .is_err()
        );
        assert_eq!(owner.journal.recovery_prefix().unwrap().sequence, 1);
    }
    owner.publish_original(&original, [5; 32]).unwrap();
    drop(owner);
    assert!(
        KagemushaOrdinaryIssuerPreparationAttemptV1::open_existing(
            root.path(),
            scope.clone(),
            carrier.clone(),
            [6; 32]
        )
        .is_err()
    );
    let mut other = carrier;
    other.financial_authority_commitment[0] ^= 1;
    assert!(
        KagemushaOrdinaryIssuerPreparationAttemptV1::open_existing(
            root.path(),
            scope,
            other,
            [5; 32]
        )
        .is_err()
    );
}
// Public fixture for a shorter C interval while its authentic selected policies remain current.
// The actual private journal and canonical owner recheck are still used; no production switch.
fn narrow_reservation(
    dir: &Path,
    selected: Arc<KagemushaOrdinaryPreparationSelectedOriginalsV1>,
    carrier: KagemushaOrdinaryPreparationCarrierV1,
) -> KagemushaOrdinaryIssuerPreparationAttemptV1 {
    let challenge = selected
        .issuer_challenge_for_carrier(&carrier, [93; 32], 300, 5000)
        .unwrap();
    let signing_request = challenge
        .to_signing_request(selected.preparation_issuer_key().unwrap())
        .unwrap();
    let reserved_record = encode(&Record::Reserved {
        original_body_sha256: [5; 32],
        challenge: challenge.clone(),
        signing_request: signing_request.clone(),
    })
    .unwrap();
    let mut journal = PrivateJournal::create_new(
        &KagemushaOrdinaryIssuerPreparationAttemptV1::path(dir, &selected, &carrier).unwrap(),
        FORMAT,
    )
    .unwrap();
    journal.append(&reserved_record).unwrap();
    let attempt = KagemushaOrdinaryIssuerPreparationAttemptV1 {
        selected,
        carrier,
        original_body_sha256: [5; 32],
        challenge,
        signing_request,
        reserved_record,
        published: None,
        journal,
    };
    attempt.recheck().unwrap();
    attempt
}
#[test]
fn issuer_c_pending_restart_keeps_original_body_and_expiry_never_renews() {
    let f = Fixture::new(false);
    let root = private_root();
    let carrier = carrier(&f);
    let owner = narrow_reservation(root.path(), selected(&f, 300), carrier.clone());
    let request = owner.signing_request().unwrap().to_vec();
    drop(owner);
    let pending = KagemushaOrdinaryIssuerPreparationAttemptV1::open_existing(
        root.path(),
        selected(&f, 1000),
        carrier.clone(),
        [5; 32],
    )
    .unwrap();
    assert_eq!(pending.signing_request().unwrap(), request);
    drop(pending);
    let expired = KagemushaOrdinaryIssuerPreparationAttemptV1::open_existing(
        root.path(),
        selected(&f, 8000),
        carrier.clone(),
        [5; 32],
    )
    .unwrap();
    assert_eq!(expired.signing_request, request);
    assert_eq!(expired.challenge.expires_at_ms, 5000);
    assert!(expired.signing_request().is_err());
    assert!(expired.published_original([5; 32]).is_err());
    drop(expired);
    assert!(
        KagemushaOrdinaryIssuerPreparationAttemptV1::reserve(
            root.path(),
            selected(&f, 8000),
            carrier.clone(),
            [5; 32]
        )
        .is_err()
    );
    let published_root = private_root();
    let mut owner = narrow_reservation(published_root.path(), selected(&f, 300), carrier.clone());
    let original = signed(&owner, 63);
    owner.publish_original(&original, [5; 32]).unwrap();
    drop(owner);
    let recovered = KagemushaOrdinaryIssuerPreparationAttemptV1::open_existing(
        published_root.path(),
        selected(&f, 8000),
        carrier,
        [5; 32],
    )
    .unwrap();
    assert_eq!(recovered.published_original([5; 32]).unwrap(), original);
    assert_eq!(recovered.challenge.expires_at_ms, 5000);
    assert!(recovered.signing_request().is_err());
}
#[test]
fn issuer_c_uncertain_publication_freezes_owner_then_recovers_complete_original() {
    use super::super::super::private_journal::TestPersistenceFailure;
    for failure in [
        TestPersistenceFailure::BeforeSync,
        TestPersistenceFailure::AfterSync,
    ] {
        let f = Fixture::new(false);
        let root = private_root();
        let scope = selected(&f, 300);
        let carrier = carrier(&f);
        let mut owner = KagemushaOrdinaryIssuerPreparationAttemptV1::reserve(
            root.path(),
            scope.clone(),
            carrier.clone(),
            [5; 32],
        )
        .unwrap();
        let original = signed(&owner, 63);
        owner.journal.failure.set(Some(failure));
        assert!(matches!(
            owner.publish_original(&original, [5; 32]),
            Err(KagemushaOrdinaryIdentityErrorV1::UnknownOutcome)
        ));
        assert!(owner.publish_original(&original, [5; 32]).is_err());
        drop(owner);
        let recovered = KagemushaOrdinaryIssuerPreparationAttemptV1::open_existing(
            root.path(),
            scope,
            carrier,
            [5; 32],
        )
        .unwrap();
        assert_eq!(recovered.published_original([5; 32]).unwrap(), original);
        assert_eq!(recovered.journal.recovery_prefix().unwrap().sequence, 2);
    }
}
#[test]
fn issuer_c_recovery_rejects_torn_extra_records_without_replacement() {
    use super::super::super::private_journal::TestPersistenceFailure;
    let f = Fixture::new(false);
    let root = private_root();
    let scope = selected(&f, 300);
    let carrier = carrier(&f);
    let mut owner = KagemushaOrdinaryIssuerPreparationAttemptV1::reserve(
        root.path(),
        scope.clone(),
        carrier.clone(),
        [5; 32],
    )
    .unwrap();
    let original = signed(&owner, 63);
    owner
        .journal
        .failure
        .set(Some(TestPersistenceFailure::PartialWrite));
    assert!(owner.publish_original(&original, [5; 32]).is_err());
    drop(owner);
    assert!(
        KagemushaOrdinaryIssuerPreparationAttemptV1::open_existing(
            root.path(),
            scope.clone(),
            carrier.clone(),
            [5; 32]
        )
        .is_err()
    );
    assert!(
        KagemushaOrdinaryIssuerPreparationAttemptV1::reserve(
            root.path(),
            scope.clone(),
            carrier.clone(),
            [5; 32]
        )
        .is_err()
    );
    let fresh = private_root();
    let mut owner = KagemushaOrdinaryIssuerPreparationAttemptV1::reserve(
        fresh.path(),
        scope.clone(),
        carrier.clone(),
        [5; 32],
    )
    .unwrap();
    let original = signed(&owner, 63);
    owner.publish_original(&original, [5; 32]).unwrap();
    let extra = encode(&Record::Published {
        signed_original: original,
        authenticated_at_ms: 300,
    })
    .unwrap();
    owner.journal.append(&extra).unwrap();
    assert!(owner.published_original([5; 32]).is_err());
    drop(owner);
    assert!(
        KagemushaOrdinaryIssuerPreparationAttemptV1::open_existing(
            fresh.path(),
            scope,
            carrier,
            [5; 32]
        )
        .is_err()
    );
}

#[test]
fn issuer_c_live_refuses_replaced_descriptor_and_restart_preserves_original() {
    let f = Fixture::new(false);
    let root = private_root();
    let scope = selected(&f, 300);
    let carrier = carrier(&f);
    let owner = KagemushaOrdinaryIssuerPreparationAttemptV1::reserve(
        root.path(),
        scope.clone(),
        carrier.clone(),
        [5; 32],
    )
    .unwrap();
    let path = owner
        .journal
        .original_directory()
        .unwrap()
        .join(FORMAT.filename);
    let replacement = path.with_extension("replacement");
    std::fs::copy(&path, &replacement).unwrap();
    std::fs::set_permissions(&replacement, std::fs::Permissions::from_mode(0o600)).unwrap();
    std::fs::rename(&replacement, &path).unwrap();
    assert!(owner.signing_request().is_err());
    assert!(owner.published_original([5; 32]).is_err());
    drop(owner);
    // Reopening an exact durably complete original can adopt a surviving copied inode; it
    // supplies no hardware anti-clone claim. It still cannot renew or replace that C body.
    let recovered = KagemushaOrdinaryIssuerPreparationAttemptV1::open_existing(
        root.path(),
        scope.clone(),
        carrier.clone(),
        [5; 32],
    )
    .unwrap();
    assert_eq!(recovered.journal.recovery_prefix().unwrap().sequence, 1);
    drop(recovered);
    assert!(
        KagemushaOrdinaryIssuerPreparationAttemptV1::reserve(root.path(), scope, carrier, [5; 32])
            .is_err()
    );
}
