//! Real account/device signatures over private structural open fixtures. These fixtures do
//! not construct or claim authenticated release, issuer-certificate or Core-machine authority.

use super::*;
use crate::kagemusha_core_coordinator_v1::startup_qualification::tests as fixture;
use crate::kagemusha_device_bridge_v1::QualificationProjectionV1;
use iroha_core::zk::kagemusha_v1_state::{
    DevicePolicyBindingV1, HardwareEpochV1, KagemushaLaneIdV1,
};
use iroha_crypto::{Hash, HashOf, KeyPair};

fn account_key() -> KeyPair {
    KeyPair::from_seed(vec![211; 32], Algorithm::Ed25519)
}

fn initial_pending(qualification: &QualificationProjectionV1) -> PendingEnrolledOpenV1 {
    PendingEnrolledOpenV1::begin(
        fixture::owner(qualification),
        fixture::enrollment_binding(qualification),
        EnrolledOpenAuthoritySourceV1::InitialCertificate {
            certificate_digest: [77; 32],
        },
        RequiredCredentialV1::Initial(qualification.credential),
        qualification.release_id,
        qualification.hardware_policy_digest,
        qualification.core_authorization_key_reference,
        NativeDeadlineV1::start(LIFETIME).unwrap(),
    )
    .unwrap()
}

pub(in crate::kagemusha_core_coordinator_v1) fn recovery_source(
    qualification: &QualificationProjectionV1,
) -> EnrolledOpenAuthoritySourceV1 {
    let enrollment = fixture::enrollment_binding(qualification);
    EnrolledOpenAuthoritySourceV1::RecoveryCheckpoint {
        statement: DurabilityAnchorStatementV1 {
            metadata_revision: 7,
            version: 1,
            lane: KagemushaLaneIdV1 {
                network_id: enrollment.owner.runtime.network_id,
                device_lane_id: enrollment.owner.lane_id,
                asset: enrollment.owner.runtime.asset,
                scale: enrollment.owner.runtime.scale,
            },
            state_commitment: [81; 32],
            hardware_epoch: HardwareEpochV1 {
                generation: u128::from(qualification.credential.hardware_epoch_generation),
                epoch_id: qualification.credential.hardware_epoch_id,
            },
            device_policy_binding: DevicePolicyBindingV1 {
                device_key_reference: qualification.credential.device_key_reference,
                hardware_policy_id: qualification.hardware_policy_digest,
            },
            state_nonce_commitment: [82; 32],
            logical_sequence: 19,
            journal_revision: 20,
            inbox_revision: 21,
            snapshot_commitment: [83; 32],
        },
        terminal_certificate_digest: [84; 32],
    }
}

fn recovery_pending(
    current: &QualificationProjectionV1,
    historical: KagemushaHardwareCredentialV1,
) -> PendingEnrolledOpenV1 {
    recovery_pending_for_source(
        current,
        historical,
        recovery_source(current),
        NativeDeadlineV1::start(LIFETIME).unwrap(),
    )
}

pub(in crate::kagemusha_core_coordinator_v1) fn recovery_pending_for_source(
    current: &QualificationProjectionV1,
    historical: KagemushaHardwareCredentialV1,
    source: EnrolledOpenAuthoritySourceV1,
    deadline: NativeDeadlineV1,
) -> PendingEnrolledOpenV1 {
    PendingEnrolledOpenV1::begin(
        fixture::restored_owner(current, historical),
        fixture::enrollment_binding(current),
        source,
        RequiredCredentialV1::Recovered {
            generation: u128::from(current.credential.hardware_epoch_generation),
            epoch_id: current.credential.hardware_epoch_id,
            key_reference: current.credential.device_key_reference,
        },
        current.release_id,
        current.hardware_policy_digest,
        current.core_authorization_key_reference,
        deadline,
    )
    .unwrap()
}

fn sign_account(pending: &PendingEnrolledOpenV1) -> Vec<u8> {
    Signature::try_new(
        account_key().private_key(),
        &pending.account_signing_message(),
    )
    .unwrap()
    .payload()
    .to_vec()
}

fn frame(pending: &PendingEnrolledOpenV1, qualification: &QualificationProjectionV1) -> Vec<u8> {
    frame_for_command(pending, qualification, pending.device_command())
}

fn frame_for_command(
    pending: &PendingEnrolledOpenV1,
    qualification: &QualificationProjectionV1,
    command: &[u8],
) -> Vec<u8> {
    frame_for_projection(pending.nonce(), command, qualification)
}

pub(in crate::kagemusha_core_coordinator_v1) fn frame_for_projection(
    nonce: [u8; 32],
    command: &[u8],
    qualification: &QualificationProjectionV1,
) -> Vec<u8> {
    let reply = fixture::reply(qualification);
    let signature = fixture::sign_reply_for_command(1, nonce, command, &reply, qualification);
    // Independently assemble the success frame, including both payload/signature digests.
    let mut bytes = b"IKGMJRS1".to_vec();
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&[1, 0]);
    bytes.extend_from_slice(&nonce);
    bytes.extend_from_slice(&(reply.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&64_u32.to_le_bytes());
    bytes.extend_from_slice(&Sha256::digest(&reply));
    bytes.extend_from_slice(&Sha256::digest(&signature));
    bytes.extend_from_slice(&reply);
    bytes.extend_from_slice(&signature);
    bytes
}

/// Signed structural fixture for the native registry's concurrency/revocation tests only.
/// It supplies no fake authenticated release, initial certificate or Core machine.
pub(in crate::kagemusha_core_coordinator_v1) fn signed_fixture(
    generation: u64,
) -> (PendingEnrolledOpenV1, Vec<u8>, Vec<u8>) {
    let qualification = fixture::qualification(generation);
    let pending = initial_pending(&qualification);
    let account_signature = sign_account(&pending);
    let response = frame(&pending, &qualification);
    (pending, account_signature, response)
}

#[test]
fn exact_dual_possession_returns_the_original_observation_and_single_observer() {
    let (pending, signature, response) = signed_fixture(1);
    let expected_challenge = pending.challenge_bytes().to_vec();
    let expected_owner = pending.enrollment_binding();
    let expected_source = pending.authority_source().clone();
    let expected_nonce = pending.nonce();
    pending.require_unexpired().unwrap();
    let complete = pending.complete(&signature, &response).unwrap();
    assert_eq!(
        complete.evidence().challenge_bytes().unwrap(),
        expected_challenge
    );
    assert_eq!(complete.evidence().enrollment_binding(), expected_owner);
    assert_eq!(complete.evidence().authority_source(), &expected_source);
    assert_eq!(
        complete.evidence().account_signature().as_slice(),
        signature
    );
    assert_eq!(complete.evidence().observation().nonce, expected_nonce);
    assert!(
        NativeContinuousInstantV1::now()
            .unwrap()
            .checked_duration_since(complete.evidence().completed_at())
            .is_some()
    );
    let (mut observer, evidence) = complete.into_parts().unwrap();
    evidence.require_unexpired().unwrap();
    // The transferred observer already holds the signed qualification, permitting fresh reads.
    let command = crate::kagemusha_device_bridge_v1::canonical_stock_command_for_tests(
        crate::KagemushaDeviceLifecycleOperationV1::from_code(21).unwrap(),
    )
    .unwrap()[80..]
        .to_vec();
    observer.begin(21, &command).unwrap();
}

#[test]
fn external_account_signer_uses_exact_typed_hash_and_not_the_canonical_payload() {
    let qualification = fixture::qualification(1);
    let pending = initial_pending(&qualification);
    let canonical = pending.challenge_bytes();
    let decoded: EnrolledOpenAccountChallengeV1 = norito::decode_canonical_with_limits(
        canonical,
        norito::canonical_decode_limits(canonical.len()),
    )
    .unwrap();
    assert_eq!(
        pending.account_signing_message(),
        *HashOf::new(&decoded).as_ref()
    );
    let external = sign_account(&pending);
    assert_eq!(
        external,
        SignatureOf::try_new(account_key().private_key(), &decoded)
            .unwrap()
            .payload()
    );
    let wrong = Signature::try_new(account_key().private_key(), canonical).unwrap();
    let response = frame(&pending, &qualification);
    assert_eq!(
        pending.complete(wrong.payload(), &response).err(),
        Some(EnrolledOpenErrorV1::AccountSignature)
    );
}

#[test]
fn wrong_account_and_non_exact_signature_lengths_fail_before_device_acceptance() {
    for mutation in 0..4 {
        let (pending, mut signature, response) = signed_fixture(1);
        match mutation {
            0 => {
                signature = Signature::try_new(
                    KeyPair::from_seed(vec![212; 32], Algorithm::Ed25519).private_key(),
                    &pending.account_signing_message(),
                )
                .unwrap()
                .payload()
                .to_vec()
            }
            1 => {
                signature.pop();
            }
            2 => signature.push(0),
            _ => signature[0] ^= 1,
        }
        assert_eq!(
            pending.complete(&signature, &response).err(),
            Some(EnrolledOpenErrorV1::AccountSignature),
        );
    }
}

#[test]
fn non_ed25519_account_controller_cannot_begin_possession() {
    let qualification = fixture::qualification(1);
    let mut enrollment = fixture::enrollment_binding(&qualification);
    enrollment.owner.account_id = iroha_data_model::account::AccountId::new(
        KeyPair::from_seed(vec![213; 32], Algorithm::Secp256k1)
            .public_key()
            .clone(),
    );
    enrollment.enrollment_id = enrollment.owner.enrollment_id().unwrap();
    assert_eq!(
        PendingEnrolledOpenV1::begin(
            fixture::owner(&qualification),
            enrollment,
            EnrolledOpenAuthoritySourceV1::InitialCertificate {
                certificate_digest: [77; 32]
            },
            RequiredCredentialV1::Initial(qualification.credential),
            qualification.release_id,
            qualification.hardware_policy_digest,
            qualification.core_authorization_key_reference,
            NativeDeadlineV1::start(LIFETIME).unwrap(),
        )
        .err(),
        Some(EnrolledOpenErrorV1::AccountController)
    );
}

#[test]
fn another_pending_attempt_cannot_reuse_either_possession_signature() {
    let qualification = fixture::qualification(1);
    for reuse_account in [false, true] {
        let old = initial_pending(&qualification);
        let next = initial_pending(&qualification);
        assert_ne!(old.nonce(), next.nonce());
        assert_ne!(
            old.account_signing_message(),
            next.account_signing_message()
        );
        let signature = sign_account(if reuse_account { &old } else { &next });
        let response = frame(if reuse_account { &next } else { &old }, &qualification);
        assert_eq!(
            next.complete(&signature, &response).err(),
            Some(if reuse_account {
                EnrolledOpenErrorV1::AccountSignature
            } else {
                EnrolledOpenErrorV1::DeviceBinding
            })
        );
    }
}

#[test]
fn full_response_requires_exact_frame_digests_and_original_command_signature() {
    let qualification = fixture::qualification(1);
    for mutation in 0..8 {
        let pending = initial_pending(&qualification);
        let signature = sign_account(&pending);
        let mut response = frame(&pending, &qualification);
        match mutation {
            0 => response[11] = 1,
            1 => response[52] ^= 1,
            2 => response[84] ^= 1,
            3 => response.push(0),
            4 => {
                response.pop();
            }
            5 => response[10] = 13,
            6 => response = frame_for_command(&pending, &qualification, b"another command"),
            _ => {
                let last = response.len() - 1;
                response[last] ^= 1;
                let digest = Sha256::digest(&response[response.len() - 64..]);
                response[84..116].copy_from_slice(&digest);
            }
        }
        assert!(
            pending.complete(&signature, &response).is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn initial_open_requires_the_exact_certificate_credential_even_for_a_valid_renewal() {
    let selected = fixture::qualification(1);
    let mut renewed = selected.clone();
    renewed.credential.issued_at_ms += 1;
    let renewed = fixture::reseal(renewed);
    let pending = initial_pending(&selected);
    let signature = sign_account(&pending);
    let response = frame(&pending, &renewed);
    assert_eq!(
        pending.complete(&signature, &response).err(),
        Some(EnrolledOpenErrorV1::DeviceBinding)
    );
}

#[test]
fn recovery_accepts_current_epoch_with_an_older_original_credential_and_catalog() {
    let historical = fixture::qualification(1);
    let mut current = fixture::qualification(2);
    current.release_id = [31; 32];
    current.profile.expires_at_ms += 1;
    current.profile = current.profile.seal_hardware_profile_id().unwrap();
    current.credential.hardware_profile_id = current.profile.hardware_profile_id;
    let current = fixture::reseal(current);
    let pending = recovery_pending(&current, historical.credential);
    let signature = sign_account(&pending);
    let response = frame(&pending, &current);
    let complete = pending.complete(&signature, &response).unwrap();
    assert!(complete.evidence().initial_enrollment().is_none());
    assert_eq!(
        complete.evidence().observation().qualification.credential,
        current.credential
    );
    assert!(matches!(
        complete.evidence().authority_source(),
        EnrolledOpenAuthoritySourceV1::RecoveryCheckpoint { .. }
    ));
}

#[test]
fn recovery_cannot_replace_current_core_epoch_with_an_older_or_future_device_epoch() {
    let current = fixture::qualification(2);
    for changed in [fixture::qualification(1), fixture::qualification(3)] {
        let pending = recovery_pending(&current, fixture::qualification(1).credential);
        let signature = sign_account(&pending);
        let response = frame(&pending, &changed);
        assert_eq!(
            pending.complete(&signature, &response).err(),
            Some(EnrolledOpenErrorV1::DeviceBinding)
        );
    }
}

#[test]
fn recovery_never_narrows_the_native_u128_epoch_generation() {
    let current = fixture::qualification(1);
    let mut pending = recovery_pending(&current, current.credential);
    pending.required_credential = RequiredCredentialV1::Recovered {
        generation: u128::from(u64::MAX) + 2,
        epoch_id: current.credential.hardware_epoch_id,
        key_reference: current.credential.device_key_reference,
    };
    let signature = sign_account(&pending);
    let response = frame(&pending, &current);
    assert_eq!(
        pending.complete(&signature, &response).err(),
        Some(EnrolledOpenErrorV1::DeviceBinding)
    );
}

#[test]
fn recovery_preserves_same_epoch_issuance_floor_while_admitting_a_valid_renewal() {
    let old = fixture::qualification(2);
    let mut current = old.clone();
    current.credential.issued_at_ms += 1;
    let current = fixture::reseal(current);
    let mut renewed = current.clone();
    renewed.credential.issued_at_ms += 1;
    let renewed = fixture::reseal(renewed);
    for (credential, accepted) in [(old, false), (current.clone(), true), (renewed, true)] {
        let pending = recovery_pending(&current, current.credential);
        let signature = sign_account(&pending);
        let response = frame(&pending, &credential);
        assert_eq!(pending.complete(&signature, &response).is_ok(), accepted);
    }
}

#[test]
fn continuous_deadline_expires_pending_proofs_and_delayed_registry_publication() {
    let (mut pending, signature, response) = signed_fixture(1);
    pending.deadline = NativeDeadlineV1::expired_for_test();
    assert_eq!(
        pending.require_unexpired(),
        Err(EnrolledOpenErrorV1::Expired)
    );
    assert_eq!(
        pending.complete(&signature, &response).err(),
        Some(EnrolledOpenErrorV1::Expired)
    );

    let (pending, signature, response) = signed_fixture(1);
    let mut complete = pending.complete(&signature, &response).unwrap();
    complete.evidence.deadline = NativeDeadlineV1::expired_for_test();
    assert_eq!(
        complete.into_parts().err(),
        Some(EnrolledOpenErrorV1::Expired)
    );
}

#[test]
fn account_challenge_commits_full_owner_source_catalog_and_purpose() {
    let (pending, _, _) = signed_fixture(1);
    let original = pending.account_signing_message();
    for mutation in 0..17 {
        let mut challenge = pending.challenge.clone();
        match mutation {
            0 => challenge.domain.push_str(":another-purpose"),
            1 => challenge.enrollment_id[0] ^= 1,
            2 => {
                challenge.owner.account_id = iroha_data_model::account::AccountId::new(
                    KeyPair::from_seed(vec![212; 32], Algorithm::Ed25519)
                        .public_key()
                        .clone(),
                )
            }
            3 => challenge.owner.runtime.fi_id = "other-fi".parse().unwrap(),
            4 => {
                challenge.owner.runtime.ledger_dataspace_id =
                    iroha_data_model::nexus::DataSpaceId::new(11)
            }
            5 => challenge.owner.runtime.authentication_namespace = "other-auth".parse().unwrap(),
            6 => {
                challenge.owner.runtime.network_id = iroha_data_model::NetworkId::from_genesis_hash(
                    HashOf::from_untyped_unchecked(Hash::new(b"other-network")),
                )
            }
            7 => challenge.owner.lane_id[0] ^= 1,
            8 => challenge.nonce[0] ^= 1,
            9 => {
                challenge.authority_source = EnrolledOpenAuthoritySourceV1::InitialCertificate {
                    certificate_digest: [78; 32],
                }
            }
            10 => challenge.release_id[0] ^= 1,
            11 => challenge.hardware_policy_digest[0] ^= 1,
            12 => challenge.core_authorization_key_reference[0] ^= 1,
            13 => challenge.lifetime_ms += 1,
            14 => {
                challenge.owner.runtime.asset =
                    iroha_data_model::asset::AssetDefinitionId::derive_from_components(
                        iroha_data_model::domain::DomainId::try_new("hardware", "universal")
                            .unwrap(),
                        "other-cash".parse().unwrap(),
                    )
            }
            15 => {
                challenge.owner.runtime.asset_incarnation =
                    iroha_data_model::nexus::AxtAssetIncarnationV1::try_from_bytes(
                        *Hash::new(b"other-incarnation").as_ref(),
                    )
                    .unwrap()
            }
            _ => challenge.owner.runtime.scale += 1,
        }
        assert_ne!(
            original,
            *HashOf::new(&challenge).as_ref(),
            "field {mutation}"
        );
    }
}

#[test]
fn recovery_account_signature_binds_full_checkpoint_and_original_terminal_certificate() {
    let qualification = fixture::qualification(2);
    let pending = recovery_pending(&qualification, fixture::qualification(1).credential);
    let original = pending.account_signing_message();
    for mutation in 0..12 {
        let mut challenge = pending.challenge.clone();
        let EnrolledOpenAuthoritySourceV1::RecoveryCheckpoint {
            statement,
            terminal_certificate_digest,
        } = &mut challenge.authority_source
        else {
            panic!("recovery fixture")
        };
        match mutation {
            0 => statement.metadata_revision += 1,
            1 => statement.lane.device_lane_id[0] ^= 1,
            2 => statement.state_commitment[0] ^= 1,
            3 => statement.hardware_epoch.generation += 1,
            4 => statement.hardware_epoch.epoch_id[0] ^= 1,
            5 => statement.device_policy_binding.device_key_reference[0] ^= 1,
            6 => statement.state_nonce_commitment[0] ^= 1,
            7 => statement.logical_sequence += 1,
            8 => statement.journal_revision += 1,
            9 => statement.inbox_revision += 1,
            10 => statement.snapshot_commitment[0] ^= 1,
            _ => terminal_certificate_digest[0] ^= 1,
        }
        assert_ne!(
            original,
            *HashOf::new(&challenge).as_ref(),
            "checkpoint field {mutation}"
        );
    }
}

// The shared fixtures are structural wire values, not authenticated native owners.
fn sdk_challenge_fixture() -> norito::json::Value {
    norito::json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/offline/kagemusha_enrolled_open_challenge_v1.json"
    )))
    .expect("shared SDK challenge fixture")
}

fn sdk_fixture_bytes(fixture: &norito::json::Value, field: &str) -> Vec<u8> {
    hex::decode(
        fixture
            .get(field)
            .and_then(norito::json::Value::as_str)
            .expect(field),
    )
    .expect("canonical fixture hex")
}

#[test]
fn sdk_challenge_fixtures_match_actual_native_types_and_account_signatures() {
    let fixture = sdk_challenge_fixture();
    let selector =
        iroha_data_model::kagemusha::KagemushaEnrolledOpenSelectorV1::decode_canonical_exact(
            &sdk_fixture_bytes(&fixture, "selector_canonical_hex"),
        )
        .unwrap();
    for name in ["initial", "recovery"] {
        let canonical = sdk_fixture_bytes(&fixture, &format!("{name}_challenge_canonical_hex"));
        let challenge: EnrolledOpenAccountChallengeV1 = norito::decode_canonical_with_limits(
            &canonical,
            norito::canonical_decode_limits(CHALLENGE_MAX_BYTES),
        )
        .unwrap();
        assert_eq!(norito::encode_canonical(&challenge).unwrap(), canonical);
        assert_eq!(challenge.version, 1);
        assert_eq!(challenge.domain, ACCOUNT_DOMAIN);
        assert_eq!(challenge.owner, selector.owner);
        assert_eq!(challenge.enrollment_id, selector.enrollment_id);
        assert_eq!(challenge.nonce, [88; 32]);
        assert_eq!(challenge.release_id, [89; 32]);
        assert_eq!(challenge.hardware_policy_digest, [84; 32]);
        assert_eq!(challenge.core_authorization_key_reference, [90; 32]);
        assert_eq!(challenge.lifetime_ms, LIFETIME_MS);

        let source_bytes =
            sdk_fixture_bytes(&fixture, &format!("{name}_authority_source_canonical_hex"));
        let source: EnrolledOpenAuthoritySourceV1 = norito::decode_canonical_with_limits(
            &source_bytes,
            norito::canonical_decode_limits(CHALLENGE_MAX_BYTES),
        )
        .unwrap();
        assert_eq!(source, challenge.authority_source);
        assert_eq!(norito::encode_canonical(&source).unwrap(), source_bytes);

        let message = HashOf::new(&challenge);
        assert_eq!(
            message.as_ref().as_slice(),
            sdk_fixture_bytes(&fixture, &format!("{name}_account_signing_message_hex"),)
        );
        let signature = Signature::try_from_bytes(&sdk_fixture_bytes(
            &fixture,
            &format!("{name}_account_signature_hex"),
        ))
        .unwrap();
        let public_key = challenge
            .owner
            .account_id
            .controller()
            .single_signatory()
            .unwrap();
        signature.verify(public_key, message.as_ref()).unwrap();
        assert!(signature.verify(public_key, &canonical).is_err());
        assert!(
            signature
                .verify(public_key, Hash::new(&canonical).as_ref())
                .is_err()
        );
        assert!(
            signature
                .verify(public_key, Hash::new(message.as_ref()).as_ref())
                .is_err()
        );
    }
}

#[test]
fn sdk_recovery_challenge_fixture_preserves_every_checkpoint_field() {
    let fixture = sdk_challenge_fixture();
    let canonical = sdk_fixture_bytes(&fixture, "recovery_challenge_canonical_hex");
    let challenge: EnrolledOpenAccountChallengeV1 = norito::decode_canonical_with_limits(
        &canonical,
        norito::canonical_decode_limits(CHALLENGE_MAX_BYTES),
    )
    .unwrap();
    let EnrolledOpenAuthoritySourceV1::RecoveryCheckpoint {
        statement,
        terminal_certificate_digest,
    } = challenge.authority_source
    else {
        panic!("complete recovery statement")
    };
    assert_eq!(statement.version, 1);
    assert_eq!(statement.metadata_revision, (1_u128 << 80) + 7);
    assert_eq!(
        statement.lane.network_id,
        challenge.owner.runtime.network_id
    );
    assert_eq!(statement.lane.device_lane_id, challenge.owner.lane_id);
    assert_eq!(statement.lane.asset, challenge.owner.runtime.asset);
    assert_eq!(statement.lane.scale, challenge.owner.runtime.scale);
    assert_eq!(statement.state_commitment, [81; 32]);
    assert_eq!(statement.hardware_epoch.generation, (1_u128 << 72) + 1);
    assert_eq!(statement.hardware_epoch.epoch_id, [82; 32]);
    assert_eq!(
        statement.device_policy_binding.device_key_reference,
        [83; 32]
    );
    assert_eq!(statement.device_policy_binding.hardware_policy_id, [84; 32]);
    assert_eq!(statement.state_nonce_commitment, [85; 32]);
    assert_eq!(statement.logical_sequence, (1_u128 << 90) + 19);
    assert_eq!(statement.journal_revision, (1_u128 << 91) + 20);
    assert_eq!(statement.inbox_revision, (1_u128 << 92) + 21);
    assert_eq!(statement.snapshot_commitment, [86; 32]);
    assert_eq!(terminal_certificate_digest, [87; 32]);
}
