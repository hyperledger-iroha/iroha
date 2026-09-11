//! Cryptographic and lifecycle tests for the native observation owner. Test catalog projections
//! are module-private fixtures; they are not a production authenticated-release constructor.

use super::*;
use iroha_crypto::{Hash, HashOf};
use iroha_data_model::kagemusha::{
    KagemushaDeviceSignatureV1, KagemushaEvidenceFileV1, KagemushaHardwarePlatformClassV1,
    KagemushaRetailEnrollmentOwnerV1, KagemushaRetailEnrollmentRuntimeV1,
    kagemusha_device_key_reference_v1, kagemusha_suite_commitment_v1,
};
use iroha_data_model::{NetworkId, asset::AssetDefinitionId, nexus::AxtAssetIncarnationV1};
use norito::codec::Encode;
use p256::ecdsa::{Signature, SigningKey, signature::Signer as _};
use sha2::{Digest as _, Sha256};

fn key(seed: u8) -> SigningKey {
    SigningKey::from_bytes((&[seed; 32]).into()).unwrap()
}

fn public(key: &SigningKey) -> KagemushaDevicePublicKeyV1 {
    KagemushaDevicePublicKeyV1::from_sec1_bytes(
        key.verifying_key().to_encoded_point(false).as_bytes(),
    )
    .unwrap()
}

fn signature(key: &SigningKey, bytes: &[u8]) -> Vec<u8> {
    let signature: Signature = key.sign(bytes);
    signature
        .normalize_s()
        .unwrap_or(signature)
        .to_bytes()
        .to_vec()
}

pub(in crate::kagemusha_core_coordinator_v1) fn qualification(
    generation: u64,
) -> QualificationProjectionV1 {
    let profile = KagemushaHardwareProfileV1 {
        version: 1,
        protocol_version: 1,
        hardware_profile_id: [0; 32],
        provider_id: [1; 32],
        platform_class: KagemushaHardwarePlatformClassV1::DedicatedSecureElement,
        product_class_digest: [2; 32],
        firmware_policy_digest: [3; 32],
        enrollment_attestation_verifier_digest: [4; 32],
        attestation_trust_roots_digest: [5; 32],
        allowed_suite_commitment: kagemusha_suite_commitment_v1([6; 32]),
        policy_epoch: 1,
        governance_credential_public_key: public(&key(7)),
        capability_mask: 0xffff,
        qualification_report_digest: [8; 32],
        valid_from_ms: 1,
        expires_at_ms: 100_000,
    }
    .seal_hardware_profile_id()
    .unwrap();
    let device_public_key = public(&key(10 + generation as u8));
    let mut credential = KagemushaHardwareCredentialV1 {
        version: 1,
        credential_id: [0; 32],
        network_id: NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
            b"observation-test-genesis",
        ))),
        hardware_profile_id: profile.hardware_profile_id,
        suite_id: [6; 32],
        firmware_policy_digest: profile.firmware_policy_digest,
        policy_epoch: profile.policy_epoch,
        lane_commitment: [9; 32],
        hardware_epoch_id: [generation as u8; 32],
        hardware_epoch_generation: generation,
        device_public_key,
        device_key_reference: kagemusha_device_key_reference_v1(&device_public_key),
        issued_at_ms: 10 + generation,
        expires_at_ms: 90_000,
        governance_signature: KagemushaDeviceSignatureV1::from_raw_bytes(&[1; 64]).unwrap(),
    }
    .seal_credential_id()
    .unwrap();
    credential.governance_signature = KagemushaDeviceSignatureV1::from_raw_bytes(&signature(
        &key(7),
        &credential.canonical_signing_bytes().unwrap(),
    ))
    .unwrap();
    credential.validate_against_profile(&profile).unwrap();
    QualificationProjectionV1 {
        release_id: [21; 32],
        hardware_policy_digest: [22; 32],
        core_authorization_key_reference: hardware_authorization_key_reference_v1(&public(&key(
            23,
        ))),
        profile,
        credential,
    }
}

pub(in crate::kagemusha_core_coordinator_v1) fn reseal(
    mut qualification: QualificationProjectionV1,
) -> QualificationProjectionV1 {
    qualification.credential = qualification.credential.seal_credential_id().unwrap();
    qualification.credential.governance_signature =
        KagemushaDeviceSignatureV1::from_raw_bytes(&signature(
            &key(7),
            &qualification.credential.canonical_signing_bytes().unwrap(),
        ))
        .unwrap();
    qualification
        .credential
        .validate_against_profile(&qualification.profile)
        .unwrap();
    qualification
}

fn epoch_floor(qualification: &QualificationProjectionV1) -> CoreEpochFloorV1 {
    CoreEpochFloorV1::for_credential(&qualification.credential)
}

fn wallet_context(qualification: &QualificationProjectionV1) -> ObservationWalletContextV1 {
    ObservationWalletContextV1 {
        network_id: qualification.credential.network_id,
        lane_id: qualification.credential.lane_commitment,
        asset: AssetDefinitionId::derive_from_components(
            iroha_model_base::domain::DomainId::try_new("hardware", "universal").unwrap(),
            "cash".parse().unwrap(),
        ),
        asset_incarnation: AxtAssetIncarnationV1::try_from_bytes(
            *Hash::new(b"observation-asset").as_ref(),
        )
        .unwrap(),
        scale: 2,
    }
}

pub(in crate::kagemusha_core_coordinator_v1) fn owner(
    qualification: &QualificationProjectionV1,
) -> NativeStartupQualificationOwnerV1 {
    NativeStartupQualificationOwnerV1 {
        catalog: CatalogBindingsV1 {
            release_id: qualification.release_id,
            hardware_policy_digest: qualification.hardware_policy_digest,
            provider_policy_root: [24; 32],
            enabled_profiles: vec![KagemushaEnabledProfileV1 {
                hardware_profile: qualification.profile,
                hardware_profile_id: qualification.profile.hardware_profile_id,
                suite_id: qualification.credential.suite_id,
                vk_digest: [25; 32],
                qualification_digest: [26; 32],
                policy_epoch: qualification.profile.policy_epoch,
                qualification_report: KagemushaEvidenceFileV1 {
                    sha256: qualification.profile.qualification_report_digest,
                    byte_len: 512,
                },
            }],
            wallet: wallet_context(qualification),
            core_key_reference: qualification.core_authorization_key_reference,
        },
        enrollment: enrollment_binding(qualification),
        current: None,
        core_epoch_floor: None,
        last_credential: None,
        pending: BTreeMap::new(),
    }
}

// Structural ownership only: this does not manufacture a verified issuer certificate.
pub(in crate::kagemusha_core_coordinator_v1) fn enrollment_binding(
    qualification: &QualificationProjectionV1,
) -> KagemushaRecoveryEnrollmentBindingV1 {
    let wallet = wallet_context(qualification);
    let owner = KagemushaRetailEnrollmentOwnerV1 {
        account_id: iroha_data_model::account::AccountId::new(
            iroha_crypto::KeyPair::from_seed(vec![211; 32], iroha_crypto::Algorithm::Ed25519)
                .public_key()
                .clone(),
        ),
        runtime: KagemushaRetailEnrollmentRuntimeV1 {
            fi_id: "observation-fi".parse().unwrap(),
            ledger_dataspace_id: iroha_model_base::topology::DataSpaceId::new(10),
            authentication_namespace: "observation-auth".parse().unwrap(),
            network_id: wallet.network_id,
            asset: wallet.asset,
            asset_incarnation: wallet.asset_incarnation,
            scale: wallet.scale,
        },
        lane_id: wallet.lane_id,
    };
    KagemushaRecoveryEnrollmentBindingV1 {
        enrollment_id: owner.enrollment_id().unwrap(),
        owner,
    }
}

fn issuance(qualification: &QualificationProjectionV1) -> KagemushaRetailEnrollmentIssuanceV1 {
    KagemushaRetailEnrollmentIssuanceV1 {
        release_id: qualification.release_id,
        hardware_policy_digest: qualification.hardware_policy_digest,
        core_authorization_key_reference: qualification.core_authorization_key_reference,
        credential: qualification.credential,
    }
}

// Shared signed-observer fixture for enrolled-open tests. This function is compiled only in
// the private test module; it does not construct an authenticated release or Core machine.
pub(in crate::kagemusha_core_coordinator_v1) fn restored_owner(
    current: &QualificationProjectionV1,
    historical: KagemushaHardwareCredentialV1,
) -> NativeStartupQualificationOwnerV1 {
    let mut owner = owner(current);
    owner
        .advance_validated_core_floor(&owner.enrollment.clone(), epoch_floor(current), historical)
        .unwrap();
    owner
}

fn command(operation: u8) -> Vec<u8> {
    crate::kagemusha_device_bridge_v1::canonical_stock_command_for_tests(
        crate::KagemushaDeviceLifecycleOperationV1::from_code(operation).unwrap(),
    )
    .unwrap()[80..]
        .to_vec()
}

fn fields(qualification: &QualificationProjectionV1) -> Vec<Vec<u8>> {
    vec![
        1_u32.to_le_bytes().to_vec(),
        qualification.release_id.to_vec(),
        norito::encode_canonical(&qualification.profile).unwrap(),
        norito::encode_canonical(&qualification.credential).unwrap(),
        0xffff_u32.to_le_bytes().to_vec(),
    ]
}

fn stage(
    owner: &mut NativeStartupQualificationOwnerV1,
    qualification: &QualificationProjectionV1,
) -> Result<()> {
    let mut fields = fields(qualification);
    fields.push(qualification.hardware_policy_digest.to_vec());
    owner.stage_qualification(&fields)
}

#[derive(Encode, norito::NoritoSchema)]
#[norito_schema(
    name = "connect_norito_bridge::kagemusha_core_coordinator_v1::startup_qualification::tests::QualificationReply",
    frame = "iroha.kagemusha.device.v1.active-hardware-credential-reply"
)]
struct QualificationReply {
    version: u16,
    operation: u8,
    release_id: [u8; 32],
    hardware_policy_digest: [u8; 32],
    core_authorization_key_reference: [u8; 32],
    profile: KagemushaHardwareProfileV1,
    credential: KagemushaHardwareCredentialV1,
}

pub(in crate::kagemusha_core_coordinator_v1) fn reply(
    qualification: &QualificationProjectionV1,
) -> Vec<u8> {
    norito::encode_canonical(&QualificationReply {
        version: 1,
        operation: 1,
        release_id: qualification.release_id,
        hardware_policy_digest: qualification.hardware_policy_digest,
        core_authorization_key_reference: qualification.core_authorization_key_reference,
        profile: qualification.profile,
        credential: qualification.credential,
    })
    .unwrap()
}

fn sign_reply(
    operation: u8,
    nonce: [u8; 32],
    reply: &[u8],
    qualification: &QualificationProjectionV1,
) -> Vec<u8> {
    sign_reply_for_command(operation, nonce, &command(operation), reply, qualification)
}

pub(in crate::kagemusha_core_coordinator_v1) fn sign_reply_for_command(
    operation: u8,
    nonce: [u8; 32],
    command: &[u8],
    reply: &[u8],
    qualification: &QualificationProjectionV1,
) -> Vec<u8> {
    // Independently assemble the cross-platform device response transcript.
    let mut transcript = b"iroha:kagemusha:device:v1:response-authenticator\0IKGMJRS1".to_vec();
    transcript.extend_from_slice(&1_u16.to_le_bytes());
    transcript.extend_from_slice(&[operation, 0]);
    transcript.extend_from_slice(&nonce);
    transcript.extend_from_slice(&(reply.len() as u32).to_le_bytes());
    transcript.extend_from_slice(&64_u32.to_le_bytes());
    transcript.extend_from_slice(&Sha256::digest(reply));
    transcript.extend_from_slice(&Sha256::digest(command));
    transcript.extend_from_slice(&qualification.hardware_policy_digest);
    transcript.extend_from_slice(&qualification.profile.qualification_report_digest);
    signature(
        &key(10 + qualification.credential.hardware_epoch_generation as u8),
        &transcript,
    )
}

pub(in crate::kagemusha_core_coordinator_v1) fn qualify(
    owner: &mut NativeStartupQualificationOwnerV1,
    qualification: &QualificationProjectionV1,
) -> ([u8; 32], Vec<u8>, Vec<u8>) {
    let command = command(1);
    let nonce = owner.begin(1, &command).unwrap();
    stage(owner, qualification).unwrap();
    let reply = reply(qualification);
    let signature = sign_reply(1, nonce, &reply, qualification);
    assert_eq!(
        owner
            .accept(
                1,
                nonce,
                &command,
                &reply,
                &signature,
                &fields(qualification)
            )
            .unwrap()
            .0,
        ObservationDispositionV1::Fresh
    );
    (nonce, reply, signature)
}

#[test]
fn expired_unaccepted_qualification_cannot_stage_or_install_and_requires_a_new_nonce() {
    for expire_before_stage in [false, true] {
        let qualification = qualification(1);
        let mut owner = owner(&qualification);
        let command = command(1);
        let nonce = owner.begin(1, &command).unwrap();
        if !expire_before_stage {
            stage(&mut owner, &qualification).unwrap();
        }
        owner.pending.get_mut(&1).unwrap().deadline = NativeDeadlineV1::expired_for_test();
        assert_eq!(
            stage(&mut owner, &qualification),
            Err(ObservationErrorV1::Expired)
        );
        let reply = reply(&qualification);
        let signature = sign_reply(1, nonce, &reply, &qualification);
        assert_eq!(
            owner.accept(
                1,
                nonce,
                &command,
                &reply,
                &signature,
                &fields(&qualification)
            ),
            Err(ObservationErrorV1::Expired)
        );
        assert!(owner.current.is_none());
        assert!(owner.last_credential.is_none());
        let (fresh, _, _) = qualify(&mut owner, &qualification);
        assert_ne!(fresh, nonce);
        assert_eq!(owner.current, Some(qualification));
    }
}

#[test]
fn expiry_preserves_exact_historical_retry_without_renewing_freshness() {
    let qualification = qualification(1);
    let mut owner = owner(&qualification);
    let (nonce, reply, signature) = qualify(&mut owner, &qualification);
    owner.pending.get_mut(&1).unwrap().deadline = NativeDeadlineV1::expired_for_test();
    stage(&mut owner, &qualification).unwrap();
    assert_eq!(
        owner
            .accept(
                1,
                nonce,
                &command(1),
                &reply,
                &signature,
                &fields(&qualification)
            )
            .unwrap()
            .0,
        ObservationDispositionV1::AlreadyAccepted
    );
    assert!(owner.pending[&1].deadline.check().is_err());
    let read_nonce = owner.begin(21, &command(21)).unwrap();
    owner.pending.get_mut(&21).unwrap().deadline = NativeDeadlineV1::expired_for_test();
    assert_eq!(
        owner.accept(
            21,
            read_nonce,
            &command(21),
            &[],
            &[],
            &fields(&qualification)
        ),
        Err(ObservationErrorV1::Expired)
    );
    assert_eq!(owner.last_credential, Some(qualification.credential));
    assert_eq!(owner.current, Some(qualification));
}

#[test]
fn candidate_cannot_install_a_key_and_exact_reply_is_applied_only_once() {
    let qualification = qualification(1);
    let mut owner = owner(&qualification);
    assert_eq!(
        stage(&mut owner, &qualification),
        Err(ObservationErrorV1::MissingChallenge)
    );
    let command = command(1);
    let nonce = owner.begin(1, &command).unwrap();
    stage(&mut owner, &qualification).unwrap();
    assert!(owner.current.is_none());
    assert_eq!(
        owner.begin(21, &self::command(21)),
        Err(ObservationErrorV1::InvalidQualification)
    );
    let reply = reply(&qualification);
    let signature = sign_reply(1, nonce, &reply, &qualification);
    assert_eq!(
        owner
            .accept(
                1,
                nonce,
                &command,
                &reply,
                &signature,
                &fields(&qualification)
            )
            .unwrap()
            .0,
        ObservationDispositionV1::Fresh
    );
    assert_eq!(
        owner
            .accept(
                1,
                nonce,
                &command,
                &reply,
                &signature,
                &fields(&qualification)
            )
            .unwrap()
            .0,
        ObservationDispositionV1::AlreadyAccepted
    );
}

#[test]
fn superseded_and_recreated_owners_reject_old_signed_responses() {
    let qualification = qualification(1);
    let mut first = owner(&qualification);
    let (old_nonce, reply, signature) = qualify(&mut first, &qualification);
    let fresh_nonce = first.begin(1, &command(1)).unwrap();
    assert_ne!(old_nonce, fresh_nonce);
    stage(&mut first, &qualification).unwrap();
    assert_eq!(
        first.accept(
            1,
            old_nonce,
            &command(1),
            &reply,
            &signature,
            &fields(&qualification)
        ),
        Err(ObservationErrorV1::Conflict)
    );
    let mut recreated = owner(&qualification);
    let restarted_nonce = recreated.begin(1, &command(1)).unwrap();
    assert_ne!(old_nonce, restarted_nonce);
    stage(&mut recreated, &qualification).unwrap();
    assert_eq!(
        recreated.accept(
            1,
            restarted_nonce,
            &command(1),
            &reply,
            &signature,
            &fields(&qualification)
        ),
        Err(ObservationErrorV1::Authentication)
    );
}

#[test]
fn native_catalog_and_key_reference_cannot_be_selected_by_device_projection() {
    let qualification = qualification(1);
    let mut owner = owner(&qualification);
    let nonce = owner.begin(1, &command(1)).unwrap();
    let mut wrong_release = qualification.clone();
    wrong_release.release_id = [99; 32];
    assert_eq!(
        stage(&mut owner, &wrong_release),
        Err(ObservationErrorV1::InvalidQualification)
    );
    stage(&mut owner, &qualification).unwrap();
    let mut wrong_key = qualification.clone();
    wrong_key.core_authorization_key_reference = [99; 32];
    let reply = reply(&wrong_key);
    let signature = sign_reply(1, nonce, &reply, &wrong_key);
    assert_eq!(
        owner.accept(
            1,
            nonce,
            &command(1),
            &reply,
            &signature,
            &fields(&qualification)
        ),
        Err(ObservationErrorV1::InvalidQualification)
    );
    // Even a self-consistent valid profile is not membership in the native release.
    let mut changed_catalog = self::owner(&qualification);
    changed_catalog.catalog.enabled_profiles.clear();
    changed_catalog.begin(1, &command(1)).unwrap();
    assert_eq!(
        stage(&mut changed_catalog, &qualification),
        Err(ObservationErrorV1::InvalidQualification)
    );
}

#[test]
fn malformed_or_substituted_command_reply_and_signature_never_complete_a_read() {
    let qualification = qualification(1);
    let mut owner = owner(&qualification);
    let nonce = owner.begin(1, &command(1)).unwrap();
    stage(&mut owner, &qualification).unwrap();
    let reply = reply(&qualification);
    let signature = sign_reply(1, nonce, &reply, &qualification);
    assert_eq!(
        owner.accept(
            1,
            nonce,
            &command(13),
            &reply,
            &signature,
            &fields(&qualification)
        ),
        Err(ObservationErrorV1::Conflict)
    );
    assert_eq!(
        owner.accept(
            1,
            nonce,
            &command(1),
            &reply,
            &[0; 64],
            &fields(&qualification)
        ),
        Err(ObservationErrorV1::Authentication)
    );
    let mut malformed = reply.clone();
    malformed.push(0);
    assert_eq!(
        owner.accept(
            1,
            nonce,
            &command(1),
            &malformed,
            &signature,
            &fields(&qualification)
        ),
        Err(ObservationErrorV1::InvalidQualification)
    );
    assert!(owner.current.is_none());
}

#[test]
fn credential_rotation_invalidates_all_other_pending_reads() {
    let first = qualification(1);
    let second = qualification(2);
    let mut owner = owner(&first);
    qualify(&mut owner, &first);
    for operation in [13, 18, 21] {
        owner.begin(operation, &command(operation)).unwrap();
    }
    assert_eq!(owner.pending.len(), 4);
    qualify(&mut owner, &second);
    assert_eq!(owner.pending.len(), 1);
    assert_eq!(owner.current.as_ref(), Some(&second));
    owner.invalidate();
    assert!(owner.pending.is_empty());
    assert!(owner.current.is_none());
}

#[test]
fn outstanding_candidate_is_immutable_and_observed_epoch_cannot_roll_back() {
    let first = qualification(1);
    let second = qualification(2);
    let mut owner = owner(&first);
    let nonce = owner.begin(1, &command(1)).unwrap();
    stage(&mut owner, &first).unwrap();
    assert_eq!(
        stage(&mut owner, &second),
        Err(ObservationErrorV1::Conflict)
    );
    assert!(owner.pending.get(&1).unwrap().nonce == nonce);
    qualify(&mut owner, &second);
    let nonce = owner.begin(1, &command(1)).unwrap();
    stage(&mut owner, &first).unwrap();
    let reply = reply(&first);
    let signature = sign_reply(1, nonce, &reply, &first);
    assert_eq!(
        owner.accept(1, nonce, &command(1), &reply, &signature, &fields(&first)),
        Err(ObservationErrorV1::InvalidQualification)
    );
    assert_eq!(owner.current.as_ref(), Some(&second));
    owner.invalidate();
    let nonce = owner.begin(1, &command(1)).unwrap();
    stage(&mut owner, &first).unwrap();
    let signature = sign_reply(1, nonce, &reply, &first);
    assert_eq!(
        owner.accept(1, nonce, &command(1), &reply, &signature, &fields(&first)),
        Err(ObservationErrorV1::InvalidQualification)
    );
    assert_eq!(owner.last_credential.as_ref(), Some(&second.credential));
}

#[test]
fn equal_issuance_time_cannot_order_two_different_governed_credentials() {
    let first = qualification(1);
    let mut changed = first.clone();
    changed.credential.expires_at_ms -= 1;
    changed.credential = changed.credential.seal_credential_id().unwrap();
    changed.credential.governance_signature =
        KagemushaDeviceSignatureV1::from_raw_bytes(&signature(
            &key(7),
            &changed.credential.canonical_signing_bytes().unwrap(),
        ))
        .unwrap();
    changed
        .credential
        .validate_against_profile(&changed.profile)
        .unwrap();
    let mut owner = owner(&first);
    qualify(&mut owner, &first);
    owner.invalidate();
    let nonce = owner.begin(1, &command(1)).unwrap();
    stage(&mut owner, &changed).unwrap();
    let reply = reply(&changed);
    let signature = sign_reply(1, nonce, &reply, &changed);
    assert_eq!(
        owner.accept(1, nonce, &command(1), &reply, &signature, &fields(&changed)),
        Err(ObservationErrorV1::InvalidQualification)
    );
}

#[test]
fn abandoned_read_attempts_have_fixed_live_capacity_and_never_reserve_money() {
    let qualification = qualification(1);
    let mut owner = owner(&qualification);
    qualify(&mut owner, &qualification);
    for _ in 0..100 {
        for operation in [13, 18, 21] {
            owner.begin(operation, &command(operation)).unwrap();
        }
    }
    assert_eq!(owner.pending.len(), 4);
    assert_eq!(
        owner.begin(20, &command(20)),
        Err(ObservationErrorV1::InvalidCommand)
    );
    assert_eq!(
        owner.begin(1, b"malformed"),
        Err(ObservationErrorV1::InvalidCommand)
    );
}

#[derive(Encode, norito::NoritoSchema)]
#[norito_schema(
    name = "connect_norito_bridge::kagemusha_core_coordinator_v1::startup_qualification::tests::SnapshotReply",
    frame = "iroha.kagemusha.device.v1.wallet-recovery-snapshot-reply"
)]
struct SnapshotReply {
    version: u16,
    operation: u8,
    canonical_aggregate_state: Option<Vec<u8>>,
    journal_revision: u128,
    pending_credit_count: u128,
    retry_outbox_count: u128,
}

#[test]
fn accepted_snapshot_is_exact_evidence_and_cannot_publish_again_after_invalidation() {
    let qualification = qualification(1);
    let mut owner = owner(&qualification);
    qualify(&mut owner, &qualification);
    let command = command(21);
    let nonce = owner.begin(21, &command).unwrap();
    let reply = norito::encode_canonical(&SnapshotReply {
        version: 1,
        operation: 21,
        canonical_aggregate_state: None,
        journal_revision: 3,
        pending_credit_count: 2,
        retry_outbox_count: 1,
    })
    .unwrap();
    let signature = sign_reply(21, nonce, &reply, &qualification);
    let (disposition, evidence) = owner
        .accept(
            21,
            nonce,
            &command,
            &reply,
            &signature,
            &fields(&qualification),
        )
        .unwrap();
    assert_eq!(disposition, ObservationDispositionV1::Fresh);
    assert_eq!(evidence.canonical_reply, reply);
    assert_eq!(evidence.authenticator, signature);
    assert_eq!(
        owner
            .accept(
                21,
                nonce,
                &command,
                &reply,
                &signature,
                &fields(&qualification)
            )
            .unwrap()
            .0,
        ObservationDispositionV1::AlreadyAccepted
    );
    let changed = norito::encode_canonical(&SnapshotReply {
        version: 1,
        operation: 21,
        canonical_aggregate_state: None,
        journal_revision: 4,
        pending_credit_count: 2,
        retry_outbox_count: 1,
    })
    .unwrap();
    let changed_signature = sign_reply(21, nonce, &changed, &qualification);
    assert_eq!(
        owner.accept(
            21,
            nonce,
            &command,
            &changed,
            &changed_signature,
            &fields(&qualification)
        ),
        Err(ObservationErrorV1::Conflict)
    );
    owner.invalidate();
    assert_eq!(
        owner.accept(
            21,
            nonce,
            &command,
            &reply,
            &signature,
            &fields(&qualification)
        ),
        Err(ObservationErrorV1::MissingChallenge)
    );
}

#[test]
fn independently_selected_native_network_and_lane_remain_required() {
    let qualification = qualification(1);
    let mut wrong_lane = owner(&qualification);
    wrong_lane.catalog.wallet.lane_id = [99; 32];
    wrong_lane.begin(1, &command(1)).unwrap();
    assert_eq!(
        stage(&mut wrong_lane, &qualification),
        Err(ObservationErrorV1::InvalidQualification)
    );
    let mut wrong_network = owner(&qualification);
    wrong_network.catalog.wallet.network_id =
        NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(b"other-genesis")));
    wrong_network.begin(1, &command(1)).unwrap();
    assert_eq!(
        stage(&mut wrong_network, &qualification),
        Err(ObservationErrorV1::InvalidQualification)
    );
}

#[derive(Clone, Copy, Encode, norito::NoritoSchema)]
#[norito_schema(
    name = "connect_norito_bridge::kagemusha_core_coordinator_v1::startup_qualification::tests::Watermark",
    frame = "iroha.kagemusha.device.v1.pending-credit-watermark"
)]
struct Watermark {
    hardware_epoch_generation: u128,
    hardware_epoch_id: [u8; 32],
    inbox_revision: u128,
}

#[derive(Encode, norito::NoritoSchema)]
#[norito_schema(
    name = "connect_norito_bridge::kagemusha_core_coordinator_v1::startup_qualification::tests::WatermarkReply",
    frame = "iroha.kagemusha.device.v1.pending-credit-watermark-reply"
)]
struct WatermarkReply {
    version: u16,
    operation: u8,
    watermark: Watermark,
    next_pending: Option<()>,
}

fn watermark_reply(
    qualification: &QualificationProjectionV1,
    generation: u128,
    epoch: [u8; 32],
) -> Vec<u8> {
    assert_ne!(qualification.credential.hardware_epoch_generation, 0);
    norito::encode_canonical(&WatermarkReply {
        version: 1,
        operation: 18,
        watermark: Watermark {
            hardware_epoch_generation: generation,
            hardware_epoch_id: epoch,
            inbox_revision: 4,
        },
        next_pending: None,
    })
    .unwrap()
}

#[test]
fn signed_watermark_must_use_the_qualified_epoch_and_generation() {
    let qualification = qualification(1);
    let mut owner = owner(&qualification);
    qualify(&mut owner, &qualification);
    for (generation, epoch, accepted) in [
        (2, qualification.credential.hardware_epoch_id, false),
        (1, [42; 32], false),
        (1, qualification.credential.hardware_epoch_id, true),
    ] {
        let command = command(18);
        let nonce = owner.begin(18, &command).unwrap();
        let reply = watermark_reply(&qualification, generation, epoch);
        let signature = sign_reply(18, nonce, &reply, &qualification);
        assert_eq!(
            owner
                .accept(
                    18,
                    nonce,
                    &command,
                    &reply,
                    &signature,
                    &fields(&qualification)
                )
                .is_ok(),
            accepted
        );
    }
}

fn aggregate(
    qualification: &QualificationProjectionV1,
) -> iroha_data_model::kagemusha::KagemushaAggregateStateCommitmentV1 {
    let wallet = wallet_context(qualification);
    iroha_data_model::kagemusha::KagemushaAggregateStateCommitmentV1 {
        version: 1,
        release_id: qualification.release_id,
        network_id: wallet.network_id,
        asset: wallet.asset.clone(),
        asset_incarnation: wallet.asset_incarnation,
        scale: wallet.scale,
        liability_pool_id: iroha_data_model::kagemusha::kagemusha_liability_pool_id_v1(
            &wallet.network_id,
            &wallet.asset,
            wallet.asset_incarnation,
        )
        .unwrap(),
        lane_id: wallet.lane_id,
        hardware_epoch_id: qualification.credential.hardware_epoch_id,
        key_reference: qualification.credential.device_key_reference,
        hardware_policy_id: qualification.hardware_policy_digest,
        sequence: 2,
        state_commitment: [43; 32],
    }
}

#[test]
fn signed_snapshot_cannot_choose_another_wallet_or_qualification_context() {
    let qualification = qualification(1);
    let mut owner = owner(&qualification);
    qualify(&mut owner, &qualification);
    for altered in 0..10 {
        let mut state = aggregate(&qualification);
        match altered {
            0 => state.release_id = [50; 32],
            1 => state.hardware_policy_id = [51; 32],
            2 => state.lane_id = [52; 32],
            3 => state.hardware_epoch_id = [53; 32],
            4 => state.key_reference = [54; 32],
            5 => state.scale += 1,
            6 => {
                state.network_id = NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(
                    Hash::new(b"other-network"),
                ))
            }
            7 => {
                state.asset = AssetDefinitionId::derive_from_components(
                    iroha_model_base::domain::DomainId::try_new("hardware", "universal").unwrap(),
                    "other".parse().unwrap(),
                )
            }
            8 => {
                state.asset_incarnation =
                    AxtAssetIncarnationV1::try_from_bytes(*Hash::new(b"other-incarnation").as_ref())
                        .unwrap()
            }
            _ => (),
        }
        state.liability_pool_id = iroha_data_model::kagemusha::kagemusha_liability_pool_id_v1(
            &state.network_id,
            &state.asset,
            state.asset_incarnation,
        )
        .unwrap();
        state.validate().unwrap(); // Each substitution is canonical and publicly valid.
        let reply = norito::encode_canonical(&SnapshotReply {
            version: 1,
            operation: 21,
            canonical_aggregate_state: Some(norito::encode_canonical(&state).unwrap()),
            journal_revision: 5,
            pending_credit_count: 0,
            retry_outbox_count: 0,
        })
        .unwrap();
        let command = command(21);
        let nonce = owner.begin(21, &command).unwrap();
        let signature = sign_reply(21, nonce, &reply, &qualification);
        assert_eq!(
            owner
                .accept(
                    21,
                    nonce,
                    &command,
                    &reply,
                    &signature,
                    &fields(&qualification)
                )
                .is_ok(),
            altered == 9,
            "altered selector {altered}"
        );
    }
}

#[derive(Encode, norito::NoritoSchema)]
#[norito_schema(
    name = "connect_norito_bridge::kagemusha_core_coordinator_v1::startup_qualification::tests::PendingTarget",
    frame = "iroha.kagemusha.device.v1.pending-credit-target"
)]
enum PendingTarget {
    DrainAll,
    RequiredBalance(u128),
}

#[derive(Encode, norito::NoritoSchema)]
#[norito_schema(
    name = "connect_norito_bridge::kagemusha_core_coordinator_v1::startup_qualification::tests::WatermarkCommand",
    frame = "iroha.kagemusha.device.v1.read-pending-credit-watermark-command"
)]
struct WatermarkCommand {
    version: u16,
    operation: u8,
    watermark: Option<Watermark>,
    target: PendingTarget,
}

#[test]
fn same_nonce_cannot_authenticate_a_different_credit_selection_target() {
    let qualification = qualification(1);
    let mut owner = owner(&qualification);
    qualify(&mut owner, &qualification);
    let body = |target| {
        norito::encode_canonical(&WatermarkCommand {
            version: 1,
            operation: 18,
            watermark: None,
            target,
        })
        .unwrap()
    };
    let requested = body(PendingTarget::RequiredBalance(100));
    let substituted = body(PendingTarget::DrainAll);
    let nonce = owner.begin(18, &requested).unwrap();
    let reply = watermark_reply(
        &qualification,
        1,
        qualification.credential.hardware_epoch_id,
    );
    let signature = sign_reply_for_command(18, nonce, &substituted, &reply, &qualification);
    assert_eq!(
        owner.accept(
            18,
            nonce,
            &requested,
            &reply,
            &signature,
            &fields(&qualification)
        ),
        Err(ObservationErrorV1::Authentication)
    );
    let signature = sign_reply_for_command(18, nonce, &requested, &reply, &qualification);
    assert!(
        owner
            .accept(
                18,
                nonce,
                &requested,
                &reply,
                &signature,
                &fields(&qualification)
            )
            .is_ok()
    );
}

#[test]
fn restored_core_epoch_floor_rejects_older_credentials_before_first_observation() {
    let old = qualification(1);
    let current = qualification(2);
    let mut owner = owner(&old);
    owner
        .advance_core_floor(CoreEpochFloorV1 {
            generation: 2,
            epoch_id: current.credential.hardware_epoch_id,
            key_reference: current.credential.device_key_reference,
        })
        .unwrap();
    // No earlier observation exists in this process; the Core floor still rejects generation1.
    let nonce = owner.begin(1, &command(1)).unwrap();
    stage(&mut owner, &old).unwrap();
    let reply = reply(&old);
    let signature = sign_reply(1, nonce, &reply, &old);
    assert_eq!(
        owner.accept(1, nonce, &command(1), &reply, &signature, &fields(&old)),
        Err(ObservationErrorV1::InvalidQualification)
    );
    qualify(&mut owner, &current);
    owner.invalidate();
    assert_eq!(owner.core_epoch_floor.unwrap().generation, 2);
    assert_eq!(owner.last_credential, Some(current.credential));
}

#[test]
fn core_epoch_advance_is_monotone_and_never_narrows_u128_generation() {
    let qualification = qualification(1);
    let mut owner = owner(&qualification);
    let floor = CoreEpochFloorV1 {
        generation: 1,
        epoch_id: qualification.credential.hardware_epoch_id,
        key_reference: qualification.credential.device_key_reference,
    };
    owner.advance_core_floor(floor).unwrap();
    qualify(&mut owner, &qualification);
    for rejected in [
        CoreEpochFloorV1 {
            generation: 0,
            ..floor
        },
        CoreEpochFloorV1 {
            epoch_id: [99; 32],
            ..floor
        },
        CoreEpochFloorV1 {
            key_reference: [99; 32],
            ..floor
        },
    ] {
        assert_eq!(
            owner.advance_core_floor(rejected),
            Err(ObservationErrorV1::InvalidQualification)
        );
        assert_eq!(owner.current.as_ref(), Some(&qualification));
    }
    let high = CoreEpochFloorV1 {
        generation: u128::from(u64::MAX) + 1,
        ..floor
    };
    owner.advance_core_floor(high).unwrap();
    assert!(owner.current.is_none());
    assert!(owner.pending.is_empty());
    assert_eq!(owner.last_credential, Some(qualification.credential));
    assert!(!high.admits_credential(&qualification.credential));
    assert_eq!(
        owner.advance_core_floor(floor),
        Err(ObservationErrorV1::InvalidQualification)
    );
    assert_eq!(owner.core_epoch_floor, Some(high));
}

// These tests use private catalog/floor fixtures to exercise replacement and freshness only.
// They do not assert that a fixture passed the production authenticated-release/Core constructor.
#[test]
fn repin_preserves_same_epoch_issuance_floor_across_catalog_replacement() {
    let original = qualification(1);
    let mut newer = original.clone();
    newer.credential.issued_at_ms += 1;
    let newer = reseal(newer);
    let floor = CoreEpochFloorV1 {
        generation: 1,
        epoch_id: newer.credential.hardware_epoch_id,
        key_reference: newer.credential.device_key_reference,
    };
    let mut active = owner(&newer);
    active.advance_core_floor(floor).unwrap();
    qualify(&mut active, &newer);
    for operation in [13, 18, 21] {
        active.begin(operation, &command(operation)).unwrap();
    }

    let mut replacement = newer.clone();
    replacement.release_id = [31; 32];
    let mut next = owner(&replacement);
    next.advance_validated_core_floor(&next.enrollment.clone(), floor, original.credential)
        .unwrap();
    active.replace_validated_owner(next).unwrap();
    assert_eq!(active.catalog.release_id, replacement.release_id);
    assert_eq!(active.core_epoch_floor, Some(floor));
    assert_eq!(active.last_credential, Some(newer.credential));
    assert!(active.current.is_none());
    assert!(active.pending.is_empty());

    let mut older = original;
    older.release_id = replacement.release_id;
    let mut conflicting = replacement.clone();
    conflicting.credential.expires_at_ms -= 1;
    let conflicting = reseal(conflicting);
    for rejected in [older, conflicting] {
        let nonce = active.begin(1, &command(1)).unwrap();
        stage(&mut active, &rejected).unwrap();
        let bytes = reply(&rejected);
        let signature = sign_reply(1, nonce, &bytes, &rejected);
        assert_eq!(
            active.accept(
                1,
                nonce,
                &command(1),
                &bytes,
                &signature,
                &fields(&rejected)
            ),
            Err(ObservationErrorV1::InvalidQualification)
        );
        assert_eq!(active.last_credential, Some(newer.credential));
        assert!(active.current.is_none());
    }
    qualify(&mut active, &replacement);
    assert_eq!(active.current.as_ref(), Some(&replacement));
}

#[test]
fn rejected_repin_preserves_the_existing_wallet_and_outstanding_challenge() {
    let qualification = qualification(2);
    let floor = CoreEpochFloorV1 {
        generation: 2,
        epoch_id: qualification.credential.hardware_epoch_id,
        key_reference: qualification.credential.device_key_reference,
    };
    let mut active = owner(&qualification);
    active.advance_core_floor(floor).unwrap();
    qualify(&mut active, &qualification);
    let nonce = active.begin(21, &command(21)).unwrap();
    let wallet = active.catalog.wallet.clone();

    for rejected in 0..3 {
        let mut next = owner(&qualification);
        match rejected {
            0 => {
                next.advance_validated_core_floor(
                    &next.enrollment.clone(),
                    floor,
                    qualification.credential,
                )
                .unwrap();
                next.catalog.wallet.scale += 1;
            }
            1 => next
                .advance_core_floor(CoreEpochFloorV1 {
                    generation: 1,
                    ..floor
                })
                .unwrap(),
            _ => (), // A replacement lacking an authenticated Core floor is never accepted.
        }
        next.catalog.release_id = [31; 32];
        assert_eq!(
            active.replace_validated_owner(next),
            Err(ObservationErrorV1::InvalidQualification)
        );
        assert_eq!(active.catalog.wallet, wallet);
        assert_eq!(active.catalog.release_id, qualification.release_id);
        assert_eq!(active.core_epoch_floor, Some(floor));
        assert_eq!(active.last_credential, Some(qualification.credential));
        assert_eq!(active.current.as_ref(), Some(&qualification));
        assert_eq!(active.pending.get(&21).unwrap().nonce, nonce);
    }
}

#[test]
fn identical_core_epoch_floor_still_invalidates_a_signed_pending_snapshot() {
    let qualification = qualification(1);
    let floor = CoreEpochFloorV1 {
        generation: 1,
        epoch_id: qualification.credential.hardware_epoch_id,
        key_reference: qualification.credential.device_key_reference,
    };
    let mut owner = owner(&qualification);
    owner.advance_core_floor(floor).unwrap();
    qualify(&mut owner, &qualification);
    let command = command(21);
    let nonce = owner.begin(21, &command).unwrap();
    let reply = norito::encode_canonical(&SnapshotReply {
        version: 1,
        operation: 21,
        canonical_aggregate_state: None,
        journal_revision: 5,
        pending_credit_count: 1,
        retry_outbox_count: 0,
    })
    .unwrap();
    let signature = sign_reply(21, nonce, &reply, &qualification);

    // Inbox/journal changes must invalidate observations even when this epoch/key tuple is equal.
    owner.advance_core_floor(floor).unwrap();
    assert_eq!(owner.core_epoch_floor, Some(floor));
    assert_eq!(owner.last_credential, Some(qualification.credential));
    assert!(owner.current.is_none());
    assert!(owner.pending.is_empty());
    assert_eq!(
        owner.accept(
            21,
            nonce,
            &command,
            &reply,
            &signature,
            &fields(&qualification)
        ),
        Err(ObservationErrorV1::MissingChallenge)
    );
    assert_eq!(
        owner.begin(21, &command),
        Err(ObservationErrorV1::InvalidQualification)
    );

    qualify(&mut owner, &qualification);
    let fresh = owner.begin(21, &command).unwrap();
    assert_ne!(fresh, nonce);
    assert_eq!(
        owner.accept(
            21,
            fresh,
            &command,
            &reply,
            &signature,
            &fields(&qualification)
        ),
        Err(ObservationErrorV1::Authentication)
    );
}

// These restoration fixtures exercise the private merge used by the opaque-machine entrypoint.
// They do not construct a production machine or manufacture authenticated release authority.
#[test]
fn restored_exact_credential_floor_rejects_same_epoch_issuance_rollback_and_conflict() {
    let old = qualification(1);
    let mut renewed = old.clone();
    renewed.credential.issued_at_ms += 1;
    let renewed = reseal(renewed);
    let mut conflicting = renewed.clone();
    conflicting.credential.expires_at_ms -= 1;
    let conflicting = reseal(conflicting);
    let mut restarted = owner(&renewed);
    restarted
        .advance_validated_core_floor(
            &restarted.enrollment.clone(),
            epoch_floor(&renewed),
            renewed.credential,
        )
        .unwrap();
    assert!(restarted.current.is_none());
    assert!(restarted.pending.is_empty());
    assert_eq!(restarted.last_credential, Some(renewed.credential));

    for rejected in [old, conflicting] {
        let nonce = restarted.begin(1, &command(1)).unwrap();
        stage(&mut restarted, &rejected).unwrap();
        let bytes = reply(&rejected);
        let signature = sign_reply(1, nonce, &bytes, &rejected);
        assert_eq!(
            restarted.accept(
                1,
                nonce,
                &command(1),
                &bytes,
                &signature,
                &fields(&rejected)
            ),
            Err(ObservationErrorV1::InvalidQualification)
        );
        assert_eq!(restarted.last_credential, Some(renewed.credential));
        assert!(restarted.current.is_none());
    }
    qualify(&mut restarted, &renewed);
}

#[test]
fn restored_original_credential_may_precede_current_epoch_and_catalog_profile() {
    let historical = qualification(1);
    let mut current = qualification(2);
    current.release_id = [31; 32];
    current.profile.expires_at_ms += 1;
    current.profile = current.profile.seal_hardware_profile_id().unwrap();
    current.credential.hardware_profile_id = current.profile.hardware_profile_id;
    let current = reseal(current);
    let mut restarted = owner(&current);
    assert_eq!(
        restarted.catalog.validate(&historical),
        Err(ObservationErrorV1::InvalidQualification)
    );

    // Core already authenticated the retained credential against its original catalog. The
    // observer must neither reject that provenance nor let it lower the current hardware floor.
    restarted
        .advance_validated_core_floor(
            &restarted.enrollment.clone(),
            epoch_floor(&current),
            historical.credential,
        )
        .unwrap();
    assert_eq!(restarted.last_credential, Some(historical.credential));
    assert_eq!(restarted.core_epoch_floor, Some(epoch_floor(&current)));
    assert!(
        !restarted
            .core_epoch_floor
            .unwrap()
            .admits_credential(&historical.credential)
    );
    qualify(&mut restarted, &current);
    assert_eq!(restarted.last_credential, Some(current.credential));
}

#[test]
fn same_epoch_checkpoint_advance_merges_issuance_and_invalidates_freshness() {
    let original = qualification(1);
    let mut renewed = original.clone();
    renewed.credential.issued_at_ms += 1;
    let renewed = reseal(renewed);
    let mut newest = renewed.clone();
    newest.credential.issued_at_ms += 1;
    let newest = reseal(newest);
    let floor = epoch_floor(&original);
    let mut active = owner(&original);
    active
        .advance_validated_core_floor(&active.enrollment.clone(), floor, original.credential)
        .unwrap();
    qualify(&mut active, &renewed);
    active.begin(21, &command(21)).unwrap();

    active
        .advance_validated_core_floor(&active.enrollment.clone(), floor, original.credential)
        .unwrap();
    assert_eq!(active.last_credential, Some(renewed.credential));
    assert!(active.pending.is_empty());
    assert!(active.current.is_none());
    qualify(&mut active, &renewed);
    active.begin(21, &command(21)).unwrap();

    active
        .advance_validated_core_floor(&active.enrollment.clone(), floor, newest.credential)
        .unwrap();
    assert_eq!(active.last_credential, Some(newest.credential));
    assert_eq!(active.core_epoch_floor, Some(floor));
    assert!(active.pending.is_empty());
    assert!(active.current.is_none());
    qualify(&mut active, &newest);
}

#[test]
fn repin_adopts_stronger_checkpointed_issuance_without_overwriting_it_with_process_floor() {
    let original = qualification(1);
    let mut renewed = original.clone();
    renewed.release_id = [31; 32];
    renewed.credential.issued_at_ms += 1;
    let renewed = reseal(renewed);
    let floor = epoch_floor(&original);
    let mut active = owner(&original);
    active
        .advance_validated_core_floor(&active.enrollment.clone(), floor, original.credential)
        .unwrap();
    qualify(&mut active, &original);
    let mut next = owner(&renewed);
    next.advance_validated_core_floor(&next.enrollment.clone(), floor, renewed.credential)
        .unwrap();

    active.replace_validated_owner(next).unwrap();
    assert_eq!(active.last_credential, Some(renewed.credential));
    assert_eq!(active.catalog.release_id, renewed.release_id);
    assert!(active.current.is_none());
    assert!(active.pending.is_empty());
    let mut stale = original;
    stale.release_id = renewed.release_id;
    let nonce = active.begin(1, &command(1)).unwrap();
    stage(&mut active, &stale).unwrap();
    let bytes = reply(&stale);
    let signature = sign_reply(1, nonce, &bytes, &stale);
    assert_eq!(
        active.accept(1, nonce, &command(1), &bytes, &signature, &fields(&stale)),
        Err(ObservationErrorV1::InvalidQualification)
    );
    qualify(&mut active, &renewed);
}

#[test]
fn conflicting_checkpointed_credential_cannot_replace_process_floor_or_pending_read() {
    let original = qualification(1);
    let floor = epoch_floor(&original);
    let mut active = owner(&original);
    active
        .advance_validated_core_floor(&active.enrollment.clone(), floor, original.credential)
        .unwrap();
    qualify(&mut active, &original);
    let nonce = active.begin(21, &command(21)).unwrap();
    let mut conflicting = original.clone();
    conflicting.release_id = [31; 32];
    conflicting.credential.expires_at_ms -= 1;
    let conflicting = reseal(conflicting);
    let mut next = owner(&conflicting);
    next.advance_validated_core_floor(&next.enrollment.clone(), floor, conflicting.credential)
        .unwrap();

    assert_eq!(
        active.replace_validated_owner(next),
        Err(ObservationErrorV1::InvalidQualification)
    );
    assert_eq!(
        active.advance_validated_core_floor(
            &active.enrollment.clone(),
            floor,
            conflicting.credential
        ),
        Err(ObservationErrorV1::InvalidQualification)
    );
    assert_eq!(active.last_credential, Some(original.credential));
    assert_eq!(active.current.as_ref(), Some(&original));
    assert_eq!(active.core_epoch_floor, Some(floor));
    assert_eq!(active.catalog.release_id, original.release_id);
    assert_eq!(active.pending.get(&21).unwrap().nonce, nonce);
}

#[test]
fn enrolled_wallet_projection_requires_the_complete_canonical_owner_identity() {
    let qualification = qualification(1);
    let enrollment = enrollment_binding(&qualification);
    assert_eq!(
        NativeStartupQualificationOwnerV1::enrolled_wallet_context(&enrollment).unwrap(),
        wallet_context(&qualification)
    );
    for mutation in 0..3 {
        let mut changed = enrollment.clone();
        match mutation {
            0 => changed.enrollment_id = [99; 32],
            1 => changed.owner.lane_id = [0; 32],
            _ => changed.owner.runtime.scale = u32::MAX,
        }
        assert_eq!(
            NativeStartupQualificationOwnerV1::enrolled_wallet_context(&changed),
            Err(ObservationErrorV1::InvalidQualification)
        );
    }
}

// The next two tests exercise the private issuance-selection kernel. Their catalog and
// ownership fixtures are deliberately not presented as opaque verified enrollment evidence.
#[test]
fn enrollment_issuance_retains_the_exact_floor_without_asserting_freshness() {
    let old = qualification(1);
    let mut renewed = old.clone();
    renewed.credential.issued_at_ms += 1;
    let renewed = reseal(renewed);
    let mut active = owner(&renewed);
    active
        .pin_verified_enrollment_issuance(&issuance(&renewed))
        .unwrap();
    assert_eq!(active.last_credential, Some(renewed.credential));
    assert!(active.current.is_none());
    assert!(active.core_epoch_floor.is_none());
    assert!(active.pending.is_empty());
    assert_eq!(
        active.begin(21, &command(21)),
        Err(ObservationErrorV1::InvalidQualification)
    );

    let nonce = active.begin(1, &command(1)).unwrap();
    stage(&mut active, &old).unwrap();
    let reply = reply(&old);
    let signature = sign_reply(1, nonce, &reply, &old);
    assert_eq!(
        active.accept(1, nonce, &command(1), &reply, &signature, &fields(&old)),
        Err(ObservationErrorV1::InvalidQualification)
    );
    assert_eq!(active.last_credential, Some(renewed.credential));
    qualify(&mut active, &renewed);
}

#[test]
fn enrollment_issuance_cannot_select_another_catalog_key_or_credential_scope() {
    let qualification = qualification(1);
    let selected = issuance(&qualification);
    let mut active = owner(&qualification);
    for mutation in 0..7 {
        let mut changed = selected.clone();
        match mutation {
            0 => changed.release_id = [99; 32],
            1 => changed.hardware_policy_digest = [99; 32],
            2 => changed.core_authorization_key_reference = [99; 32],
            3 => {
                changed.credential.network_id = NetworkId::from_genesis_hash(
                    HashOf::from_untyped_unchecked(Hash::new(b"other-enrollment-genesis")),
                )
            }
            4 => changed.credential.lane_commitment = [99; 32],
            5 => changed.credential.hardware_profile_id = [99; 32],
            _ => {
                changed.credential.governance_signature =
                    KagemushaDeviceSignatureV1::from_raw_bytes(&[1; 64]).unwrap()
            }
        }
        if matches!(mutation, 3 | 4) {
            let mut changed_qualification = qualification.clone();
            changed_qualification.credential = changed.credential;
            changed.credential = reseal(changed_qualification).credential;
        }
        assert_eq!(
            active.pin_verified_enrollment_issuance(&changed),
            Err(ObservationErrorV1::InvalidQualification),
            "substitution {mutation}"
        );
        assert!(active.last_credential.is_none());
        assert!(active.current.is_none());
        assert!(active.pending.is_empty());
    }
    active.pin_verified_enrollment_issuance(&selected).unwrap();
    assert_eq!(active.last_credential, Some(selected.credential));
}

fn substituted_enrollment(
    selected: &KagemushaRecoveryEnrollmentBindingV1,
    mutation: usize,
) -> KagemushaRecoveryEnrollmentBindingV1 {
    let mut changed = selected.clone();
    match mutation {
        0 => {
            changed.owner.account_id = iroha_data_model::account::AccountId::new(
                iroha_crypto::KeyPair::from_seed(vec![212; 32], iroha_crypto::Algorithm::Ed25519)
                    .public_key()
                    .clone(),
            )
        }
        1 => changed.owner.runtime.fi_id = "other-fi".parse().unwrap(),
        2 => {
            changed.owner.runtime.ledger_dataspace_id =
                iroha_model_base::topology::DataSpaceId::new(11)
        }
        3 => changed.owner.runtime.authentication_namespace = "other-auth".parse().unwrap(),
        4 => {
            changed.owner.runtime.network_id = NetworkId::from_genesis_hash(
                HashOf::from_untyped_unchecked(Hash::new(b"other-owner-genesis")),
            )
        }
        5 => {
            changed.owner.runtime.asset = AssetDefinitionId::derive_from_components(
                iroha_model_base::domain::DomainId::try_new("hardware", "universal").unwrap(),
                "other-cash".parse().unwrap(),
            )
        }
        6 => {
            changed.owner.runtime.asset_incarnation = AxtAssetIncarnationV1::try_from_bytes(
                *Hash::new(b"other-owner-incarnation").as_ref(),
            )
            .unwrap()
        }
        7 => changed.owner.runtime.scale += 1,
        8 => changed.owner.lane_id = [99; 32],
        _ => {
            changed.enrollment_id = [99; 32];
            return changed;
        }
    }
    changed.enrollment_id = changed.owner.enrollment_id().unwrap();
    changed
}

#[test]
fn checkpoint_advance_rejects_every_immutable_owner_substitution_before_invalidation() {
    let qualification = qualification(1);
    let floor = epoch_floor(&qualification);
    let mut active = owner(&qualification);
    let enrollment = active.enrollment.clone();
    active
        .advance_validated_core_floor(&enrollment, floor, qualification.credential)
        .unwrap();
    qualify(&mut active, &qualification);
    let nonce = active.begin(21, &command(21)).unwrap();
    for mutation in 0..10 {
        let changed = substituted_enrollment(&enrollment, mutation);
        assert_eq!(
            active.advance_validated_core_floor(&changed, floor, qualification.credential),
            Err(ObservationErrorV1::InvalidQualification),
            "owner field {mutation}"
        );
        assert_eq!(active.enrollment, enrollment);
        assert_eq!(active.core_epoch_floor, Some(floor));
        assert_eq!(active.last_credential, Some(qualification.credential));
        assert_eq!(active.current.as_ref(), Some(&qualification));
        assert_eq!(active.pending.get(&21).unwrap().nonce, nonce);
    }
}

#[test]
fn catalog_repin_cannot_transfer_ownership_even_when_the_wallet_projection_matches() {
    let qualification = qualification(1);
    let floor = epoch_floor(&qualification);
    let mut active = owner(&qualification);
    let enrollment = active.enrollment.clone();
    active
        .advance_validated_core_floor(&enrollment, floor, qualification.credential)
        .unwrap();
    qualify(&mut active, &qualification);
    let nonce = active.begin(21, &command(21)).unwrap();
    for mutation in 0..10 {
        let mut next = owner(&qualification);
        next.advance_validated_core_floor(&enrollment, floor, qualification.credential)
            .unwrap();
        next.enrollment = substituted_enrollment(&enrollment, mutation);
        // Deliberately keep the old projection to cover identities omitted from that projection.
        assert_eq!(active.catalog.wallet, next.catalog.wallet);
        assert_eq!(
            active.replace_validated_owner(next),
            Err(ObservationErrorV1::InvalidQualification),
            "owner field {mutation}"
        );
        assert_eq!(active.enrollment, enrollment);
        assert_eq!(active.core_epoch_floor, Some(floor));
        assert_eq!(active.last_credential, Some(qualification.credential));
        assert_eq!(active.current.as_ref(), Some(&qualification));
        assert_eq!(active.pending.get(&21).unwrap().nonce, nonce);
    }
}

#[test]
fn reopen_preserves_the_stronger_process_floor_and_the_new_signed_observation() {
    let old = qualification(1);
    let mut renewed = old.clone();
    renewed.credential.issued_at_ms += 1;
    let renewed = reseal(renewed);
    let mut active = restored_owner(&old, old.credential);
    qualify(&mut active, &renewed);
    let outstanding = active.begin(21, &command(21)).unwrap();
    let mut stale = restored_owner(&old, old.credential);
    qualify(&mut stale, &old);
    assert!(matches!(
        active.prepare_reopen(stale),
        Err(ObservationErrorV1::InvalidQualification)
    ));
    assert_eq!(active.last_credential, Some(renewed.credential));
    assert_eq!(active.pending.get(&21).unwrap().nonce, outstanding);

    let mut current = restored_owner(&old, old.credential);
    let (nonce, _, _) = qualify(&mut current, &renewed);
    let prepared = active.prepare_reopen(current).unwrap();
    assert_eq!(prepared.last_credential, Some(renewed.credential));
    assert_eq!(prepared.current.as_ref(), Some(&renewed));
    assert_eq!(prepared.pending.get(&1).unwrap().nonce, nonce);
    assert!(!prepared.pending.contains_key(&21));
}

#[test]
fn reopen_requires_fresh_qualification_and_preserves_the_original_owner_on_conflict() {
    let selected = qualification(1);
    let mut active = restored_owner(&selected, selected.credential);
    qualify(&mut active, &selected);
    assert!(
        active
            .prepare_reopen(restored_owner(&selected, selected.credential))
            .is_err()
    );
    let mut conflicting = selected.clone();
    conflicting.credential.expires_at_ms -= 1;
    let conflicting = reseal(conflicting);
    let mut candidate = restored_owner(&conflicting, conflicting.credential);
    qualify(&mut candidate, &conflicting);
    assert!(active.prepare_reopen(candidate).is_err());
    assert_eq!(active.current.as_ref(), Some(&selected));
    assert_eq!(active.last_credential, Some(selected.credential));
}

#[cfg(test)]
mod explicit_schema_identity_tests {
    use super::*;

    macro_rules! identity {
        ($root:ty, $nominal:literal, $frame:literal) => {
            assert_eq!(<$root as norito::NoritoSchema>::nominal_name(), $nominal);
            assert_eq!(<$root as norito::NoritoSchema>::frame_name(), $frame);
            assert_eq!(
                norito::schema::identity::frame_hash::<$root>(),
                norito::core::schema_hash_for_name($frame)
            );
            assert_eq!(
                <Vec<$root> as norito::NoritoSchema>::nominal_name(),
                format!("alloc::vec::Vec<{}>", $nominal)
            );
        };
    }

    #[test]
    fn framed_roots_keep_nominal_and_protocol_identities() {
        identity!(
            QualificationReply,
            "connect_norito_bridge::kagemusha_core_coordinator_v1::startup_qualification::tests::QualificationReply",
            "iroha.kagemusha.device.v1.active-hardware-credential-reply"
        );
        identity!(
            SnapshotReply,
            "connect_norito_bridge::kagemusha_core_coordinator_v1::startup_qualification::tests::SnapshotReply",
            "iroha.kagemusha.device.v1.wallet-recovery-snapshot-reply"
        );
        identity!(
            WatermarkReply,
            "connect_norito_bridge::kagemusha_core_coordinator_v1::startup_qualification::tests::WatermarkReply",
            "iroha.kagemusha.device.v1.pending-credit-watermark-reply"
        );
        identity!(
            WatermarkCommand,
            "connect_norito_bridge::kagemusha_core_coordinator_v1::startup_qualification::tests::WatermarkCommand",
            "iroha.kagemusha.device.v1.read-pending-credit-watermark-command"
        );

        let qualification = qualification(1);
        let bytes = reply(&qualification);
        let header = norito::core::Header::read(bytes.as_slice()).unwrap();
        assert_eq!(
            header.schema,
            norito::schema::identity::frame_hash::<QualificationReply>()
        );
        let canonical: iroha_data_model::kagemusha::KagemushaDeviceQualificationReplyV1 =
            norito::decode_canonical(&bytes).expect("fixture decodes as the actual device reply");
        assert_eq!(canonical.credential, qualification.credential);
        assert_eq!(norito::encode_canonical(&canonical).unwrap(), bytes);
        assert_ne!(
            <QualificationReply as norito::NoritoSchema>::nominal_name(),
            <iroha_data_model::kagemusha::KagemushaDeviceQualificationReplyV1 as norito::NoritoSchema>::nominal_name(),
        );
        assert!(matches!(
            norito::decode_canonical::<u32>(&bytes),
            Err(norito::Error::SchemaMismatch)
        ));
    }
}
