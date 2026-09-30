//! Known public crypto specimens, without an authenticated Core or monetary capability.
use super::*;
use crate::kagemusha_device_bridge_v1::sender_payload::{
    SenderReplyBodyV1, canonical_command_body_for_tests,
};
use p256::ecdsa::{Signature, SigningKey, signature::Signer as _};

fn fixture() -> (
    SenderWalletContextV1,
    QualificationProjectionV1,
    Vec<u8>,
    Vec<u8>,
    Vec<u8>,
) {
    let qualification = super::super::startup_qualification::tests::qualification(1);
    let command = canonical_command_body_for_tests(6).unwrap();
    let mut command: SenderCommandV1 = norito::decode_from_bytes(&command).unwrap();
    command.context.lane.network_id = qualification.credential.network_id;
    command.context.lane.device_lane_id = qualification.credential.lane_commitment;
    command.context.release.release_id = qualification.release_id;
    command.context.release.hardware_profile_id = qualification.profile.hardware_profile_id;
    command.context.release.suite_id = qualification.credential.suite_id;
    command.context.credential_id = qualification.credential.credential_id;
    command.context.hardware_epoch.generation =
        u128::from(qualification.credential.hardware_epoch_generation);
    command.context.hardware_epoch.epoch_id = qualification.credential.hardware_epoch_id;
    command.context.device_policy_binding.device_key_reference =
        qualification.credential.device_key_reference;
    command.context.device_policy_binding.hardware_policy_id = [0x91; 32];
    command.context.core_authorization_key_reference =
        qualification.core_authorization_key_reference;
    let context = command.context.clone();
    let reply = SenderReplyV1 {
        version: 1,
        operation: 6,
        request_id: command.operation_id,
        context: context.clone(),
        index_revision: 1,
        body: SenderReplyBodyV1::Lookup(None),
    };
    let reply = reply.encode_canonical(&command, &context).unwrap();
    let command = command.encode_canonical().unwrap();
    let transcript = kagemusha_device_response_signing_bytes_v1(
        6,
        [7; 32],
        &command,
        &reply,
        qualification.hardware_policy_digest,
        qualification.profile.qualification_report_digest,
    )
    .unwrap();
    let signature: Signature = SigningKey::from_bytes((&[11; 32]).into())
        .unwrap()
        .sign(&transcript);
    let signature = signature
        .normalize_s()
        .unwrap_or(signature)
        .to_bytes()
        .to_vec();
    (context, qualification, command, reply, signature)
}

#[test]
fn original_sender_signature_authenticates_exact_missing_observation_only() {
    let (context, qualification, command, reply, signature) = fixture();
    let token = AuthenticatedSenderReplyV1::authenticate(
        6,
        [7; 32],
        &command,
        &reply,
        &signature,
        &context,
        &qualification,
        context.device_policy_binding.hardware_policy_id,
    )
    .unwrap();
    assert!(matches!(
        token.reply().body,
        SenderReplyBodyV1::Lookup(None)
    ));
    assert_eq!(token.command().operation, 6);
    token.require_original_reply(&reply).unwrap();
    let mut changed = reply;
    changed.push(0);
    assert!(token.require_original_reply(&changed).is_err());
}

#[test]
fn foreign_owner_credential_and_changed_request_refuse_signed_reply() {
    let (context, qualification, command, reply, signature) = fixture();
    let mut foreign = context.clone();
    foreign.credential_id[0] ^= 1;
    assert!(
        AuthenticatedSenderReplyV1::authenticate(
            6,
            [7; 32],
            &command,
            &reply,
            &signature,
            &foreign,
            &qualification,
            context.device_policy_binding.hardware_policy_id
        )
        .is_err()
    );
    assert!(
        AuthenticatedSenderReplyV1::authenticate(
            6,
            [8; 32],
            &command,
            &reply,
            &signature,
            &context,
            &qualification,
            context.device_policy_binding.hardware_policy_id
        )
        .is_err()
    );
    let mut foreign = qualification;
    foreign.profile.qualification_report_digest[0] ^= 1;
    assert!(
        AuthenticatedSenderReplyV1::authenticate(
            6,
            [7; 32],
            &command,
            &reply,
            &signature,
            &context,
            &foreign,
            context.device_policy_binding.hardware_policy_id
        )
        .is_err()
    );
}

#[test]
fn malformed_or_substituted_originals_and_signature_never_create_observation() {
    let (context, qualification, command, mut reply, mut signature) = fixture();
    reply.push(0);
    assert!(
        AuthenticatedSenderReplyV1::authenticate(
            6,
            [7; 32],
            &command,
            &reply,
            &signature,
            &context,
            &qualification,
            context.device_policy_binding.hardware_policy_id
        )
        .is_err()
    );
    reply.pop();
    signature[0] ^= 1;
    assert!(
        AuthenticatedSenderReplyV1::authenticate(
            6,
            [7; 32],
            &command,
            &reply,
            &signature,
            &context,
            &qualification,
            context.device_policy_binding.hardware_policy_id
        )
        .is_err()
    );
    assert!(
        AuthenticatedSenderReplyV1::authenticate(
            6,
            [7; 32],
            &command,
            &reply,
            &[],
            &context,
            &qualification,
            context.device_policy_binding.hardware_policy_id
        )
        .is_err()
    );
}

#[test]
fn provider_root_and_manifest_hardware_digest_have_distinct_native_bindings() {
    let (context, qualification, command, reply, signature) = fixture();
    assert_ne!(
        context.device_policy_binding.hardware_policy_id,
        qualification.hardware_policy_digest
    );
    assert!(
        AuthenticatedSenderReplyV1::authenticate(
            6,
            [7; 32],
            &command,
            &reply,
            &signature,
            &context,
            &qualification,
            context.device_policy_binding.hardware_policy_id
        )
        .is_ok()
    );
    assert!(
        AuthenticatedSenderReplyV1::authenticate(
            6,
            [7; 32],
            &command,
            &reply,
            &signature,
            &context,
            &qualification,
            qualification.hardware_policy_digest
        )
        .is_err()
    );
}
