//! Canonical boxed platform preparation and exact descriptor-owned replay using public fixtures.
//!
//! These byte and journal checks invoke no platform approval or production custody constructor.

use super::*;
use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{
    NetworkId,
    kagemusha::{
        KagemushaAppOperationApprovalPurposeV1, KagemushaHardwareTransitionSelectionV1,
        KagemushaOperationKindV1, kagemusha_ordinary_financial_epoch_id_v1,
    },
    testing::ordinary_app_enrollment::KagemushaOrdinaryRetailEnrollmentFixtureV1,
};
use sha2::{Digest as _, Sha256};

fn public_challenge() -> (
    KagemushaAppOperationApprovalChallengeV1,
    KagemushaHardwarePlatformClassV1,
    DigestV1,
) {
    // Only public model originals and explicit synthetic operation selectors are used here.
    // Constructing this public challenge grants no enrollment, platform or financial authority.
    let fixture = KagemushaOrdinaryRetailEnrollmentFixtureV1::new(false);
    let credential = &fixture.selection.issuance.credential.subject;
    let subject = KagemushaHardwareTransitionSelectionV1 {
        version: 1,
        release_id: credential.release_id,
        provider_policy_root: fixture.release.provider_policy_root(),
        app_policy_digest: credential.app_authority_policy_digest,
        credential_id: fixture.certificate.subject.ordinary_app_credential_digest,
        network_id: NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(
            Hash::from_marked_bytes(credential.network_id).unwrap(),
        )),
        lane_commitment: credential.lane_id,
        hardware_profile_id: credential.hardware_profile_id,
        policy_epoch: credential.policy_epoch,
        hardware_epoch_id: kagemusha_ordinary_financial_epoch_id_v1(credential).unwrap(),
        hardware_epoch_generation: credential.hardware_epoch,
        operation_kind: KagemushaOperationKindV1::Bootstrap,
        transition_statement_digest: [61; 32],
        candidate_envelope_digest: [0; 32],
        terminal_body_commitment: [0; 32],
        secure_index_before: 0,
        secure_index_after: 0,
    };
    let challenge = KagemushaAppOperationApprovalChallengeV1 {
        version: 1,
        purpose: KagemushaAppOperationApprovalPurposeV1::MonetaryTransition,
        operation_id: [64; 32],
        nonce: [65; 32],
        account_binding: credential.account_binding,
        authority_policy_digest: credential.app_authority_policy_digest,
        attested_key_id: credential.attested_key_id,
        enrollment_digest: Sha256::digest(fixture.certificate.canonical_bytes().unwrap()).into(),
        subject_signing_digest: Sha256::digest(subject.canonical_signing_bytes().unwrap()).into(),
        normalized_guard_digest: [66; 32],
        issued_at_ms: 300,
        expires_at_ms: 9000,
        subject,
    };
    challenge.canonical_signing_bytes().unwrap();
    (
        challenge,
        credential.platform_class,
        credential.app_release_digest,
    )
}

#[test]
fn boxed_prepared_record_roundtrips_and_rejects_trailing_and_oversized_payloads() {
    let (challenge, _, _) = public_challenge();
    let ticket = 17;
    let scope = [99; 32];
    let bytes = norito::encode_canonical(&Record::Prepared {
        ticket,
        challenge: Box::new(challenge),
        scope,
    })
    .unwrap();
    assert!(bytes.len() < FORMAT.maximum_payload_bytes as usize);
    let decoded = decode(&bytes).unwrap();
    match &decoded {
        Record::Prepared {
            ticket: held_ticket,
            challenge: held_challenge,
            scope: held_scope,
        } => {
            assert_eq!(*held_ticket, ticket);
            assert_eq!(held_challenge.as_ref(), &challenge);
            assert_eq!(*held_scope, scope);
        }
        _ => panic!("prepared record must retain its exact variant"),
    }
    assert_eq!(norito::encode_canonical(&decoded).unwrap(), bytes);

    let mut trailing = bytes.clone();
    trailing.push(0);
    assert!(decode(&trailing).is_err());
    let mut oversized = bytes;
    oversized.resize(FORMAT.maximum_payload_bytes as usize + 1, 0);
    assert!(matches!(
        decode(&oversized),
        Err(KagemushaStateErrorV1::SnapshotIntegrity)
    ));
}

#[test]
fn prepared_attempt_reopens_same_challenge_and_scope_and_refuses_substitution() {
    let (challenge, class, app_release) = public_challenge();
    let scope = [99; 32];
    let temp = tempfile::tempdir().unwrap();
    let path = temp
        .path()
        .canonicalize()
        .unwrap()
        .join("boxed-prepared-attempt");
    let attempt =
        BootstrapPlatformAttempt::create(&path, challenge, scope, class, app_release).unwrap();
    let ticket = attempt.ticket();
    assert_ne!(ticket, 0);
    assert_eq!(attempt.challenge(), &challenge);
    assert_eq!(attempt.scope(), scope);
    let prefix = attempt.wal.recovery_prefix().unwrap();
    assert_eq!(prefix.sequence, 1);
    let wal_path = attempt
        .wal
        .original_directory()
        .unwrap()
        .join(FORMAT.filename);
    let wal_bytes = std::fs::read(&wal_path).unwrap();
    assert_eq!(prefix.byte_len, wal_bytes.len() as u64);
    drop(attempt);

    let reopened =
        BootstrapPlatformAttempt::open_existing(&path, challenge, scope, class, app_release)
            .unwrap();
    assert_eq!(reopened.ticket(), ticket);
    assert_eq!(reopened.challenge(), &challenge);
    assert_eq!(reopened.scope(), scope);
    assert_eq!(reopened.recover().unwrap(), vec![vec![0], vec![], vec![]]);
    assert_eq!(reopened.wal.recovery_prefix().unwrap(), prefix);
    assert_eq!(std::fs::read(&wal_path).unwrap(), wal_bytes);
    drop(reopened);

    let mut substituted = challenge;
    substituted.normalized_guard_digest[0] ^= 1;
    substituted.canonical_signing_bytes().unwrap();
    assert!(matches!(
        BootstrapPlatformAttempt::open_existing(&path, substituted, scope, class, app_release),
        Err(KagemushaStateErrorV1::SnapshotIntegrity)
    ));
    assert_eq!(std::fs::read(&wal_path).unwrap(), wal_bytes);
    let mut substituted_scope = scope;
    substituted_scope[0] ^= 1;
    assert!(matches!(
        BootstrapPlatformAttempt::open_existing(
            &path,
            challenge,
            substituted_scope,
            class,
            app_release,
        ),
        Err(KagemushaStateErrorV1::SnapshotIntegrity)
    ));
    assert_eq!(std::fs::read(&wal_path).unwrap(), wal_bytes);

    let reopened =
        BootstrapPlatformAttempt::open_existing(&path, challenge, scope, class, app_release)
            .unwrap();
    assert_eq!(reopened.ticket(), ticket);
    assert_eq!(reopened.challenge(), &challenge);
    assert_eq!(reopened.scope(), scope);
    assert_eq!(reopened.wal.recovery_prefix().unwrap(), prefix);
    assert_eq!(std::fs::read(&wal_path).unwrap(), wal_bytes);
}
