//! Genuine P-256 original-byte diagnostics using the maintained public synthetic sender fixture.
//! These tests create no authenticated release, recursive verifier, hardware qualification or
//! production Core owner. Pure helper success authenticates bytes/correlation only.

use super::*;
use crate::kagemusha_sender_wire::{
    SenderPublicInputPreimageV1, SenderPublicInputsV1, SenderRecordV1, SenderRecoveryItemV1,
    canonical_command_body_for_tests,
};
use iroha_data_model::kagemusha::{
    KagemushaDevicePublicKeyV1, KagemushaDeviceSignatureV1, KagemushaPaymentRequestV1,
    KagemushaPaymentV1, kagemusha_device_key_reference_v1,
    kagemusha_device_response_signing_bytes_v1,
};
use p256::ecdsa::{Signature, SigningKey, signature::Signer as _};
use sha2::{Digest as _, Sha256};

struct Fixture {
    command: SenderCommandV1,
    reply: SenderReplyV1,
    device_key: SigningKey,
    device_public_key: KagemushaDevicePublicKeyV1,
    policy: DigestV1,
    report: DigestV1,
}

impl Fixture {
    fn new() -> Self {
        let device_key = SigningKey::from_bytes((&[11; 32]).into()).unwrap();
        let device_public_key = KagemushaDevicePublicKeyV1::from_sec1_bytes(
            device_key
                .verifying_key()
                .to_encoded_point(false)
                .as_bytes(),
        )
        .unwrap();
        let mut command = SenderCommandV1::decode_canonical_exact(
            7,
            [7; 32],
            &canonical_command_body_for_tests(7).unwrap(),
        )
        .unwrap();
        let prepare = SenderCommandV1::decode_canonical_exact(
            5,
            [7; 32],
            &canonical_command_body_for_tests(5).unwrap(),
        )
        .unwrap();
        let SenderCommandBodyV1::Prepare { inputs } = prepare.body else {
            panic!("maintained sender fixture must carry public inputs");
        };
        command.context.device_policy_binding.device_key_reference =
            kagemusha_device_key_reference_v1(&device_public_key);
        let inputs_digest = SenderPublicInputPreimageV1 {
            version: 1,
            operation_id: command.operation_id,
            context: command.context.clone(),
            inputs: inputs.clone(),
        }
        .canonical_digest()
        .unwrap();
        let SenderCommandBodyV1::Commit {
            selector,
            candidate_digest,
            hardware_authorization,
        } = &mut command.body
        else {
            panic!("maintained sender fixture must carry op7");
        };
        let mut authorization =
            SenderHardwareAuthorizationV1::decode_canonical_exact(hardware_authorization).unwrap();
        selector.inputs_digest = inputs_digest;
        authorization.inputs_digest = inputs_digest;
        authorization
            .hardware_transition_statement
            .predecessor_device_policy_binding = command.context.device_policy_binding;
        authorization
            .hardware_transition_statement
            .successor_device_policy_binding = command.context.device_policy_binding;
        authorization.authorization_id = authorization
            .unsigned_preimage()
            .authorization_id()
            .unwrap();
        // The known public fixture Core signing seed is separate from the device signing seed.
        let core_key = SigningKey::from_bytes((&[0x61; 32]).into()).unwrap();
        let signature: Signature = core_key.sign(&authorization.authorization_id);
        let signature = signature.normalize_s().unwrap_or(signature);
        authorization.authenticator =
            KagemushaDeviceSignatureV1::from_raw_bytes(signature.to_bytes().as_ref()).unwrap();
        let authorization_bytes = norito::encode_canonical(&authorization).unwrap();
        SenderHardwareAuthorizationV1::decode_canonical_exact(&authorization_bytes).unwrap();
        *hardware_authorization = authorization_bytes;

        let value: norito::json::Value = norito::json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/offline/kagemusha_v1.json"
        )))
        .unwrap();
        let SenderPublicInputsV1::SendSplit { request } = &inputs else {
            panic!("maintained sender fixture must carry a payment request");
        };
        let request = KagemushaPaymentRequestV1::decode_canonical_exact(request).unwrap();
        let payment_bytes = hex::decode(
            value
                .get("payment")
                .unwrap()
                .get("norito_hex")
                .unwrap()
                .as_str()
                .unwrap(),
        )
        .unwrap();
        let payment =
            KagemushaPaymentV1::decode_canonical_shape_exact_against(&payment_bytes, &request)
                .unwrap();
        let observed = SenderRecordV1 {
            operation_id: command.operation_id,
            context: command.context.clone(),
            inputs_digest,
            operation_kind: inputs.operation_kind(),
            preparation_id: selector.preparation_id,
            outbox_reservation_id: [0x39; 32],
            outcome_id: authorization.outcome_id,
            phase: SenderPhaseV1::Committed,
            record_revision: 2,
            inputs: Some(inputs),
            candidate_digest: Some(*candidate_digest),
            commit_certificate_digest: Some(payment.commit_certificate.canonical_digest().unwrap()),
            envelope_digest: None,
            terminal_receipt_digest: None,
        };
        let reply = SenderReplyV1 {
            version: 1,
            operation: 7,
            request_id: command.operation_id,
            context: command.context.clone(),
            index_revision: 2,
            body: SenderReplyBodyV1::Lookup(Some(SenderRecoveryItemV1 {
                record: observed,
                canonical_envelope: Vec::new(),
            })),
        };
        reply.validate_against(&command, &command.context).unwrap();
        Self {
            command,
            reply,
            device_key,
            device_public_key,
            policy: [0x91; 32],
            report: [0x92; 32],
        }
    }

    fn original(&self, reply: &SenderReplyV1) -> KagemushaOriginalOutgoingHardwareCommitV1 {
        let command = self.command.encode_canonical().unwrap();
        let payload = norito::encode_canonical(reply).unwrap();
        let transcript = kagemusha_device_response_signing_bytes_v1(
            7,
            self.command.operation_id,
            &command,
            &payload,
            self.policy,
            self.report,
        )
        .unwrap();
        let signature: Signature = self.device_key.sign(&transcript);
        let signature = signature.normalize_s().unwrap_or(signature).to_bytes();
        let mut response = b"IKGMJRS1".to_vec();
        response.extend_from_slice(&1_u16.to_le_bytes());
        response.extend_from_slice(&[7, 0]);
        response.extend_from_slice(&self.command.operation_id);
        response.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        response.extend_from_slice(&64_u32.to_le_bytes());
        response.extend_from_slice(&Sha256::digest(&payload));
        response.extend_from_slice(&Sha256::digest(&signature));
        response.extend_from_slice(&payload);
        response.extend_from_slice(&signature);
        KagemushaOriginalOutgoingHardwareCommitV1 {
            canonical_command: command,
            original_response: response,
        }
    }

    fn verify(
        &self,
        original: &KagemushaOriginalOutgoingHardwareCommitV1,
    ) -> Result<(SenderCommandV1, SenderReplyV1), KagemushaStateErrorV1> {
        verify_signed_reply(
            original,
            self.command.operation_id,
            &self.command.context,
            self.policy,
            self.report,
            &self.device_public_key,
        )
    }
}

#[test]
fn genuine_signed_op7_original_authenticates_exact_command_and_committed_reply() {
    let fixture = Fixture::new();
    let original = fixture.original(&fixture.reply);
    let (command, reply) = fixture.verify(&original).unwrap();
    assert_eq!(command, fixture.command);
    assert_eq!(reply, fixture.reply);
}

#[test]
fn signed_op7_original_rejects_foreign_key_policy_report_and_context() {
    let fixture = Fixture::new();
    let original = fixture.original(&fixture.reply);
    let foreign_key = SigningKey::from_bytes((&[12; 32]).into()).unwrap();
    let foreign_public = KagemushaDevicePublicKeyV1::from_sec1_bytes(
        foreign_key
            .verifying_key()
            .to_encoded_point(false)
            .as_bytes(),
    )
    .unwrap();
    assert!(
        verify_signed_reply(
            &original,
            fixture.command.operation_id,
            &fixture.command.context,
            fixture.policy,
            fixture.report,
            &foreign_public,
        )
        .is_err()
    );
    for (policy, report) in [([0x93; 32], fixture.report), (fixture.policy, [0x93; 32])] {
        assert!(
            verify_signed_reply(
                &original,
                fixture.command.operation_id,
                &fixture.command.context,
                policy,
                report,
                &fixture.device_public_key,
            )
            .is_err()
        );
    }
    let mut context = fixture.command.context.clone();
    context.credential_id[0] ^= 1;
    assert!(
        verify_signed_reply(
            &original,
            fixture.command.operation_id,
            &context,
            fixture.policy,
            fixture.report,
            &fixture.device_public_key,
        )
        .is_err()
    );
}

#[test]
fn signed_op7_original_rejects_command_status_nonce_and_payload_substitution() {
    let fixture = Fixture::new();
    let original = fixture.original(&fixture.reply);
    let mut command = original.clone();
    let last = command.canonical_command.len() - 1;
    command.canonical_command[last] ^= 1;
    assert!(fixture.verify(&command).is_err());
    for offset in [10, 11, 12, 52, 84, 116] {
        let mut changed = original.clone();
        changed.original_response[offset] ^= 1;
        assert!(
            fixture.verify(&changed).is_err(),
            "substitution at offset {offset}"
        );
    }
    let mut trailing = original;
    trailing.original_response.push(0);
    assert!(fixture.verify(&trailing).is_err());
}

#[test]
fn genuine_signature_does_not_admit_missing_or_substituted_op7_body() {
    let fixture = Fixture::new();
    let mut reply = fixture.reply.clone();
    reply.body = SenderReplyBodyV1::Lookup(None);
    assert!(fixture.verify(&fixture.original(&reply)).is_err());
    let mut reply = fixture.reply.clone();
    let SenderReplyBodyV1::Lookup(Some(item)) = &mut reply.body else {
        unreachable!()
    };
    item.record.preparation_id[0] ^= 1;
    assert!(fixture.verify(&fixture.original(&reply)).is_err());
    let mut reply = fixture.reply.clone();
    let SenderReplyBodyV1::Lookup(Some(item)) = &mut reply.body else {
        unreachable!()
    };
    item.record.candidate_digest.as_mut().unwrap()[0] ^= 1;
    assert!(fixture.verify(&fixture.original(&reply)).is_err());
}

fn expected_record(fixture: &Fixture) -> KagemushaOutgoingOperationRecordV1 {
    let SenderReplyBodyV1::Lookup(Some(item)) = &fixture.reply.body else {
        unreachable!()
    };
    let observed = &item.record;
    let inputs = match observed.inputs.as_ref().unwrap() {
        SenderPublicInputsV1::SendSplit { request } => KagemushaOutgoingPublicInputsV1::SendSplit {
            request: request.clone(),
        },
        SenderPublicInputsV1::RedeemSplit {
            amount,
            beneficiary,
        } => KagemushaOutgoingPublicInputsV1::RedeemSplit {
            amount: *amount,
            beneficiary: beneficiary.clone(),
        },
    };
    KagemushaOutgoingOperationRecordV1 {
        operation_id: observed.operation_id,
        context: observed.context.clone(),
        inputs_digest: observed.inputs_digest,
        operation_kind: observed.operation_kind,
        preparation_id: observed.preparation_id,
        outbox_reservation_id: observed.outbox_reservation_id,
        outcome_id: observed.outcome_id,
        phase: KagemushaOutgoingOperationPhaseV1::CandidatePersisted,
        record_revision: 1,
        inputs: Some(inputs),
        candidate_digest: observed.candidate_digest,
        commit_certificate_digest: None,
        envelope_digest: None,
        terminal_receipt_digest: None,
        reserved_record_bytes: 4096,
    }
}

fn correlate(
    fixture: &Fixture,
    expected: &KagemushaOutgoingOperationRecordV1,
    reply: &SenderReplyV1,
) -> Result<(), KagemushaStateErrorV1> {
    let SenderReplyBodyV1::Lookup(Some(item)) = &fixture.reply.body else {
        unreachable!()
    };
    correlate_observed_fields(
        expected,
        item.record.preparation_id,
        item.record.outbox_reservation_id,
        item.record.candidate_digest.unwrap(),
        item.record.commit_certificate_digest.unwrap(),
        reply,
    )
}

#[test]
fn genuine_signed_committed_original_correlates_with_selected_native_public_fields_only() {
    let fixture = Fixture::new();
    let original = fixture.original(&fixture.reply);
    let (_, reply) = fixture.verify(&original).unwrap();
    correlate(&fixture, &expected_record(&fixture), &reply).unwrap();
}

#[test]
fn signed_precommit_or_later_tombstone_cannot_replace_exact_committed_observation() {
    let fixture = Fixture::new();
    for phase in [
        SenderPhaseV1::CandidatePersisted,
        SenderPhaseV1::Installed,
        SenderPhaseV1::Released,
    ] {
        let mut reply = fixture.reply.clone();
        let SenderReplyBodyV1::Lookup(Some(item)) = &mut reply.body else {
            unreachable!()
        };
        item.record.phase = phase;
        match phase {
            SenderPhaseV1::CandidatePersisted => item.record.commit_certificate_digest = None,
            SenderPhaseV1::Installed => item.record.envelope_digest = Some([0x77; 32]),
            SenderPhaseV1::Released => {
                item.record.inputs = None;
                item.record.envelope_digest = Some([0x77; 32]);
                item.record.terminal_receipt_digest = Some([0x78; 32]);
            }
            _ => unreachable!(),
        }
        let original = fixture.original(&reply);
        let (_, authenticated_data) = fixture.verify(&original).unwrap();
        assert!(correlate(&fixture, &expected_record(&fixture), &authenticated_data).is_err());
    }
}

#[test]
fn genuine_signature_cannot_override_any_selected_native_commit_identity() {
    let fixture = Fixture::new();
    // Every altered payload below receives a real new signature. The signature proves bytes;
    // independently selected native fields still decide which original completion they describe.
    let changes: [fn(&mut SenderRecordV1); 10] = [
        |r| r.operation_id[0] ^= 1,
        |r| r.context.credential_id[0] ^= 1,
        |r| r.context.core_authorization_key_reference[0] ^= 1,
        |r| r.inputs_digest[0] ^= 1,
        |r| r.operation_kind = iroha_data_model::kagemusha::KagemushaOperationKindV1::RedeemSplit,
        |r| r.preparation_id[0] ^= 1,
        |r| r.outbox_reservation_id[0] ^= 1,
        |r| r.outcome_id[0] ^= 1,
        |r| r.candidate_digest.as_mut().unwrap()[0] ^= 1,
        |r| r.commit_certificate_digest.as_mut().unwrap()[0] ^= 1,
    ];
    for (index, change) in changes.into_iter().enumerate() {
        let mut reply = fixture.reply.clone();
        let SenderReplyBodyV1::Lookup(Some(item)) = &mut reply.body else {
            unreachable!()
        };
        change(&mut item.record);
        let original = fixture.original(&reply);
        kagemusha_verify_device_response_v1(
            &original.original_response,
            &original.canonical_command,
            7,
            fixture.command.operation_id,
            fixture.policy,
            fixture.report,
            &fixture.device_public_key,
        )
        .unwrap();
        assert!(
            correlate(&fixture, &expected_record(&fixture), &reply).is_err(),
            "identity {index}"
        );
    }
}

#[test]
fn selected_native_committed_certificate_cannot_be_replaced_by_another_signed_original() {
    let fixture = Fixture::new();
    let mut expected = expected_record(&fixture);
    expected.phase = KagemushaOutgoingOperationPhaseV1::Committed;
    let SenderReplyBodyV1::Lookup(Some(item)) = &fixture.reply.body else {
        unreachable!()
    };
    expected.commit_certificate_digest = item.record.commit_certificate_digest;
    correlate(&fixture, &expected, &fixture.reply).unwrap();
    expected.commit_certificate_digest.as_mut().unwrap()[0] ^= 1;
    assert!(correlate(&fixture, &expected, &fixture.reply).is_err());
}

#[test]
fn original_core_authorization_and_device_authenticator_each_require_their_real_signature() {
    let fixture = Fixture::new();
    let mut original = fixture.original(&fixture.reply);
    let mut command = fixture.command.clone();
    let SenderCommandBodyV1::Commit {
        hardware_authorization,
        ..
    } = &mut command.body
    else {
        unreachable!()
    };
    let mut authorization =
        SenderHardwareAuthorizationV1::decode_canonical_exact(hardware_authorization).unwrap();
    let mut signature = authorization.authenticator.as_raw_bytes().to_vec();
    signature[0] ^= 1;
    authorization.authenticator = KagemushaDeviceSignatureV1::from_raw_bytes(&signature).unwrap();
    *hardware_authorization = norito::encode_canonical(&authorization).unwrap();
    original.canonical_command = norito::encode_canonical(&command).unwrap();
    assert!(
        SenderCommandV1::decode_canonical_exact(
            7,
            fixture.command.operation_id,
            &original.canonical_command
        )
        .is_err()
    );
    assert!(fixture.verify(&original).is_err());
    let mut original = fixture.original(&fixture.reply);
    let last = original.original_response.len() - 1;
    original.original_response[last] ^= 1;
    // Restore the frame's signature digest to isolate ECDSA authentication from framing.
    let payload_len =
        u32::from_le_bytes(original.original_response[44..48].try_into().unwrap()) as usize;
    let digest = Sha256::digest(&original.original_response[116 + payload_len..]);
    original.original_response[84..116].copy_from_slice(&digest);
    assert!(fixture.verify(&original).is_err());
}
