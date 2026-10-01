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

#[test]
fn historical_sender_context_is_checked_separately_from_the_current_signing_epoch() {
    let (old, mut qualification, command, _, _) = fixture();
    let mut current = old.clone();
    current.hardware_epoch.generation += 1;
    current.hardware_epoch.epoch_id = [0x92; 32];
    qualification.credential.hardware_epoch_generation += 1;
    qualification.credential.hardware_epoch_id = current.hardware_epoch.epoch_id;
    let mut command: SenderCommandV1 = norito::decode_canonical(&command).unwrap();
    command.operation = 10;
    command.body=crate::kagemusha_device_bridge_v1::sender_payload::SenderCommandBodyV1::RecoverInstalled {
        selector:crate::kagemusha_device_bridge_v1::sender_payload::SenderRecoverySelectorV1::Lookup {inputs_digest:[5;32]},
    };
    let reply = SenderReplyV1 {
        version: 1,
        operation: 10,
        request_id: command.operation_id,
        context: current.clone(),
        index_revision: 2,
        body: SenderReplyBodyV1::Lookup(None),
    }
    .encode_canonical(&command, &current)
    .unwrap();
    let command = command.encode_canonical().unwrap();
    let transcript = kagemusha_device_response_signing_bytes_v1(
        10,
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
    let signature = signature.normalize_s().unwrap_or(signature).to_bytes();
    let accepted = AuthenticatedSenderReplyV1::authenticate(
        10,
        [7; 32],
        &command,
        &reply,
        &signature,
        &current,
        &qualification,
        current.device_policy_binding.hardware_policy_id,
    )
    .unwrap();
    assert_eq!(accepted.command().context, old);
    assert_eq!(accepted.reply().context, current);
    assert!(
        AuthenticatedSenderReplyV1::authenticate(
            10,
            [7; 32],
            &command,
            &reply,
            &signature,
            &old,
            &qualification,
            current.device_policy_binding.hardware_policy_id
        )
        .is_err()
    );
}

#[test]
fn signed_read_observation_cannot_be_relabelled_as_original_release_response() {
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
    assert!(token.original_response().is_err());
    // The existing token independently verifies this public signature; the test framing
    // specimen cannot change its admitted operation or create a release capability.
    use sha2::{Digest as _, Sha256};
    let mut original = b"IKGMJRS1".to_vec();
    original.extend_from_slice(&1_u16.to_le_bytes());
    original.extend_from_slice(&[6, 0]);
    original.extend_from_slice(&[7; 32]);
    original.extend_from_slice(&(reply.len() as u32).to_le_bytes());
    original.extend_from_slice(&64_u32.to_le_bytes());
    original.extend_from_slice(&Sha256::digest(&reply));
    original.extend_from_slice(&Sha256::digest(&signature));
    original.extend_from_slice(&reply);
    original.extend_from_slice(&signature);
    assert!(require_original_response(6, [7; 32], &reply, &signature, &original).is_ok());
    assert!(token.retain_original_response(&original).is_err());
}

#[test]
fn signed_release_token_retains_the_exact_original_frame_across_credential_rotation() {
    use crate::kagemusha_device_bridge_v1::sender_payload::{
        SenderCommandBodyV1, SenderHardwareAuthorizationV1, SenderPhaseV1, SenderPublicInputsV1,
        SenderRecordV1, SenderRecoveryItemV1,
    };
    use iroha_data_model::kagemusha::{
        KagemushaOperationKindV1, KagemushaPaymentRequestV1, KagemushaPaymentV1,
    };
    use sha2::{Digest as _, Sha256};
    // Actual canonical Release and independently verified diagnostic signatures. These
    // module-private public crypto specimens construct no native owner or monetary grant.
    let command_bytes = canonical_command_body_for_tests(12).unwrap();
    let command = SenderCommandV1::decode_canonical_exact(12, [7; 32], &command_bytes).unwrap();
    let SenderCommandBodyV1::Release {
        inputs_digest,
        envelope_digest,
        inputs,
        envelope,
        hardware_authorization,
        ..
    } = &command.body
    else {
        panic!("Release fixture")
    };
    let authorization =
        SenderHardwareAuthorizationV1::decode_canonical_exact(hardware_authorization).unwrap();
    let SenderPublicInputsV1::SendSplit { request } = inputs else {
        panic!("payment fixture")
    };
    let request = KagemushaPaymentRequestV1::decode_canonical_exact(request).unwrap();
    let payment =
        KagemushaPaymentV1::decode_canonical_shape_exact_against(envelope, &request).unwrap();
    let mut qualification = super::super::startup_qualification::tests::qualification(2);
    qualification.credential.network_id = command.context.lane.network_id;
    qualification.credential.lane_commitment = command.context.lane.device_lane_id;
    let qualification = super::super::startup_qualification::tests::reseal(qualification);
    qualification
        .credential
        .validate_against_profile(&qualification.profile)
        .unwrap();
    let mut current = command.context.clone();
    current.credential_id = qualification.credential.credential_id;
    current.release.release_id = qualification.release_id;
    current.release.hardware_profile_id = qualification.profile.hardware_profile_id;
    current.release.suite_id = qualification.credential.suite_id;
    current.release.policy_epoch = qualification.credential.policy_epoch;
    current.hardware_epoch.generation =
        u128::from(qualification.credential.hardware_epoch_generation);
    current.hardware_epoch.epoch_id = qualification.credential.hardware_epoch_id;
    current.device_policy_binding.device_key_reference =
        qualification.credential.device_key_reference;
    current.device_policy_binding.hardware_policy_id = [0x91; 32];
    current.core_authorization_key_reference = qualification.core_authorization_key_reference;
    let record = SenderRecordV1 {
        operation_id: command.operation_id,
        context: command.context.clone(),
        inputs_digest: *inputs_digest,
        operation_kind: KagemushaOperationKindV1::SendSplit,
        preparation_id: authorization.preparation_id,
        outbox_reservation_id: [0x77; 32],
        outcome_id: authorization.outcome_id,
        phase: SenderPhaseV1::Released,
        record_revision: 4,
        inputs: None,
        candidate_digest: Some(authorization.candidate_digest),
        commit_certificate_digest: Some(payment.proof.commit_certificate_digest),
        envelope_digest: Some(*envelope_digest),
        terminal_receipt_digest: authorization.terminal_receipt_digest,
    };
    let reply = SenderReplyV1 {
        version: 1,
        operation: 12,
        request_id: command.operation_id,
        context: current.clone(),
        index_revision: 4,
        body: SenderReplyBodyV1::Lookup(Some(SenderRecoveryItemV1 {
            record,
            canonical_envelope: Vec::new(),
        })),
    }
    .encode_canonical(&command, &current)
    .unwrap();
    let transcript = kagemusha_device_response_signing_bytes_v1(
        12,
        command.operation_id,
        &command_bytes,
        &reply,
        qualification.hardware_policy_digest,
        qualification.profile.qualification_report_digest,
    )
    .unwrap();
    let signature: Signature = SigningKey::from_bytes((&[12; 32]).into())
        .unwrap()
        .sign(&transcript);
    let signature = signature
        .normalize_s()
        .unwrap_or(signature)
        .to_bytes()
        .to_vec();
    let token = AuthenticatedSenderReplyV1::authenticate(
        12,
        command.operation_id,
        &command_bytes,
        &reply,
        &signature,
        &current,
        &qualification,
        current.device_policy_binding.hardware_policy_id,
    )
    .unwrap();
    assert_eq!(token.command().context, command.context);
    assert_ne!(
        token.reply().context.credential_id,
        command.context.credential_id
    );
    assert!(token.original_response().is_err());
    let mut original = b"IKGMJRS1".to_vec();
    original.extend_from_slice(&1_u16.to_le_bytes());
    original.extend_from_slice(&[12, 0]);
    original.extend_from_slice(&command.operation_id);
    original.extend_from_slice(&(reply.len() as u32).to_le_bytes());
    original.extend_from_slice(&64_u32.to_le_bytes());
    original.extend_from_slice(&Sha256::digest(&reply));
    original.extend_from_slice(&Sha256::digest(&signature));
    original.extend_from_slice(&reply);
    original.extend_from_slice(&signature);
    let retained = token.clone().retain_original_response(&original).unwrap();
    assert_eq!(retained.original_response().unwrap(), original.as_slice());
    assert_eq!(retained.original_command(), command_bytes);
    assert_eq!(retained.original_reply(), reply);
    assert_eq!(retained.original_authenticator(), signature);
    assert!(retained == token.clone().retain_original_response(&original).unwrap());
    let truncated = &original[..original.len() - 1];
    assert!(token.clone().retain_original_response(truncated).is_err());
    for index in [10, 12, 52, 116, original.len() - 1] {
        let mut changed = original.clone();
        changed[index] ^= 1;
        assert!(token.clone().retain_original_response(&changed).is_err());
    }
    let mut foreign_key = qualification.clone();
    foreign_key.credential.device_public_key =
        super::super::startup_qualification::tests::qualification(3)
            .credential
            .device_public_key;
    assert!(
        AuthenticatedSenderReplyV1::authenticate(
            12,
            command.operation_id,
            &command_bytes,
            &reply,
            &signature,
            &current,
            &foreign_key,
            current.device_policy_binding.hardware_policy_id
        )
        .is_err()
    );
}
