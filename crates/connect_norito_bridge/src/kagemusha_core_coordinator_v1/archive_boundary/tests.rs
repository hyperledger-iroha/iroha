//! Native canonical archives, input bindings and release authorization regression tests.

use super::*;
use crate::kagemusha_device_bridge_v1::sender_payload::canonical_command_body_for_tests;

fn vector(name: &str) -> Vec<u8> {
    let fixture: norito::json::Value = norito::json::from_str(include_str!(
        "../../../../../fixtures/offline/kagemusha_core_coordinator_archives_v1.json"
    ))
    .unwrap();
    hex::decode(fixture[name]["norito_hex"].as_str().unwrap()).unwrap()
}

fn request_fields(name: &str) -> Vec<Vec<u8>> {
    let line =
        include_str!("../../../../../fixtures/offline/kagemusha_core_coordinator_frame_v1.tsv")
            .lines()
            .find(|line| line.split('\t').next() == Some(name))
            .unwrap();
    let bytes = hex::decode(line.split('\t').nth(2).unwrap()).unwrap();
    kagemusha_core_coordinator_decode_request_v1(&bytes).unwrap()
}

fn frame(fields: &[Vec<u8>]) -> Vec<u8> {
    kagemusha_core_coordinator_encode_request_v1(fields).unwrap()
}

fn response(fields: &[Vec<u8>]) -> Vec<u8> {
    kagemusha_core_coordinator_encode_response_v1(fields).unwrap()
}

/// Use the actual signed hardware release fixture for the native dispatch regression.
pub(crate) fn release_fields() -> (Vec<Vec<u8>>, Vec<Vec<u8>>) {
    let bytes = canonical_command_body_for_tests(12).unwrap();
    let command = SenderCommandV1::decode_canonical_exact(12, [7; 32], &bytes).unwrap();
    let SenderCommandBodyV1::Release {
        inputs_digest,
        envelope_digest,
        inputs,
        envelope,
        terminal_receipt,
        hardware_authorization,
    } = command.body
    else {
        panic!("release fixture")
    };
    let preparation = KagemushaCoreSenderPreparationArchiveV1 {
        version: 1,
        operation_id: command.operation_id,
        context: command.context,
        inputs_digest,
    };
    let authorization =
        SenderHardwareAuthorizationV1::decode_canonical_exact(&hardware_authorization).unwrap();
    let mut fields = vec![authorization.outcome_id.to_vec()];
    match inputs {
        SenderPublicInputsV1::SendSplit { request } => {
            fields.extend([0_u32.to_le_bytes().to_vec(), request]);
        }
        SenderPublicInputsV1::RedeemSplit {
            amount,
            beneficiary,
        } => {
            fields.extend([
                1_u32.to_le_bytes().to_vec(),
                amount.to_le_bytes().to_vec(),
                beneficiary.encode(),
            ]);
        }
    }
    fields.push(envelope.clone());
    let mut receipt = Vec::new();
    match terminal_receipt {
        SenderTerminalReceiptV1::PaymentAcknowledgement(bytes) => {
            receipt.extend_from_slice(&0_u32.to_le_bytes());
            receipt.extend(bytes);
        }
        SenderTerminalReceiptV1::RedemptionSettlement(value) => {
            receipt.extend_from_slice(&1_u32.to_le_bytes());
            receipt.extend(norito::encode_canonical(&value).unwrap());
        }
    }
    fields.push(receipt);
    fields.extend(request_fields("qualification").into_iter().take(5));
    let output = vec![
        preparation.operation_id.to_vec(),
        preparation.encode_canonical().unwrap(),
        envelope_digest.to_vec(),
        envelope,
        hardware_authorization,
    ];
    (fields, output)
}

fn installed_recovery_fields() -> (Vec<Vec<u8>>, Vec<u8>) {
    use crate::kagemusha_device_bridge_v1::sender_payload::{SenderRecordV1, SenderRecoveryItemV1};
    use iroha_data_model::kagemusha::{KagemushaOperationKindV1, KagemushaPaymentV1};
    let (release, output) = release_fields();
    let preparation =
        KagemushaCoreSenderPreparationArchiveV1::decode_canonical_exact(&output[1]).unwrap();
    let authorization = SenderHardwareAuthorizationV1::decode_canonical_exact(&output[4]).unwrap();
    let request = KagemushaPaymentRequestV1::decode_canonical_exact(&release[2]).unwrap();
    let payment =
        KagemushaPaymentV1::decode_canonical_shape_exact_against(&output[3], &request).unwrap();
    let recovery = KagemushaCoreSenderRecoveryArchiveV1 {
        version: 1,
        operation_id: preparation.operation_id,
        terminal_id: authorization.outcome_id,
        context: preparation.context.clone(),
        inputs_digest: preparation.inputs_digest,
    };
    let command = SenderCommandV1 {
        version: 1,
        operation: 10,
        operation_id: preparation.operation_id,
        context: preparation.context.clone(),
        body: SenderCommandBodyV1::RecoverInstalled {
            selector: SenderRecoverySelectorV1::Lookup {
                inputs_digest: preparation.inputs_digest,
            },
        },
    };
    let reply = SenderReplyV1 {
        version: 1,
        operation: 10,
        request_id: preparation.operation_id,
        context: preparation.context.clone(),
        index_revision: 1,
        body: SenderReplyBodyV1::Lookup(Some(SenderRecoveryItemV1 {
            record: SenderRecordV1 {
                operation_id: preparation.operation_id,
                context: preparation.context.clone(),
                inputs_digest: preparation.inputs_digest,
                operation_kind: KagemushaOperationKindV1::SendSplit,
                preparation_id: authorization.preparation_id,
                outbox_reservation_id: authorization.outbox_reservation_commitment,
                outcome_id: authorization.outcome_id,
                phase: SenderPhaseV1::Installed,
                record_revision: 1,
                inputs: Some(SenderPublicInputsV1::SendSplit {
                    request: release[2].clone(),
                }),
                candidate_digest: Some(authorization.candidate_digest),
                commit_certificate_digest: Some(payment.proof.commit_certificate_digest),
                envelope_digest: Some(terminal_envelope_digest_v1(&output[3]).unwrap()),
                terminal_receipt_digest: None,
            },
            canonical_envelope: output[3].clone(),
        })),
    };
    (
        vec![
            recovery.encode_canonical().unwrap(),
            reply
                .encode_canonical(&command, &preparation.context)
                .unwrap(),
        ],
        output[3].clone(),
    )
}

#[test]
fn native_archive_requests_reject_opaque_wrong_schema_and_trailing_bytes() {
    for (method, name, archive) in [
        (
            KagemushaCoreCoordinatorMethodV1::ProvePreparedSenderTransition,
            "prove",
            "preparation",
        ),
        (
            KagemushaCoreCoordinatorMethodV1::BuildTerminalEnvelope,
            "terminal-envelope",
            "candidate",
        ),
        (
            KagemushaCoreCoordinatorMethodV1::AcceptInstalledTerminal,
            "installed-terminal",
            "candidate",
        ),
        (
            KagemushaCoreCoordinatorMethodV1::RecoverTerminalEnvelope,
            "recover-envelope",
            "recovery",
        ),
    ] {
        let mut fields = request_fields(name);
        assert!(validate_request(method, &frame(&fields)).is_err());
        if method == KagemushaCoreCoordinatorMethodV1::RecoverTerminalEnvelope {
            fields = installed_recovery_fields().0;
        } else {
            fields[0] = vector(archive);
        }
        assert_eq!(validate_request(method, &frame(&fields)), Ok(()));
        fields[0].push(0);
        assert!(validate_request(method, &frame(&fields)).is_err());
        fields[0] = vector(if archive == "preparation" {
            "recovery"
        } else {
            "preparation"
        });
        assert!(validate_request(method, &frame(&fields)).is_err());
    }
}

#[test]
fn native_begin_preparation_binds_operation_and_complete_input_preimage() {
    let prep =
        KagemushaCoreSenderPreparationArchiveV1::decode_canonical_exact(&vector("preparation"))
            .unwrap();
    let fixture: norito::json::Value = norito::json::from_str(include_str!(
        "../../../../../fixtures/offline/kagemusha_v1.json"
    ))
    .unwrap();
    let mut fields = request_fields("begin-send");
    fields[0] = prep.operation_id.to_vec();
    fields[2] = hex::decode(fixture["payment_request"]["norito_hex"].as_str().unwrap()).unwrap();
    let method = KagemushaCoreCoordinatorMethodV1::BeginSenderTransition;
    let request = frame(&fields);
    assert_eq!(
        validate_response(
            method,
            &request,
            &response(&[fields[0].clone(), vector("preparation")])
        ),
        Ok(())
    );
    for mutate in [
        (|p: &mut KagemushaCoreSenderPreparationArchiveV1| p.operation_id[0] ^= 1)
            as fn(&mut KagemushaCoreSenderPreparationArchiveV1),
        |p| p.inputs_digest[0] ^= 1,
        |p| p.context.core_authorization_key_reference[0] ^= 1,
        |p| p.context.credential_id[0] ^= 1,
    ] {
        let mut changed = prep.clone();
        mutate(&mut changed);
        assert!(
            validate_response(
                method,
                &request,
                &response(&[fields[0].clone(), changed.encode_canonical().unwrap()])
            )
            .is_err()
        );
    }
    fields[2].push(0);
    assert!(validate_request(method, &frame(&fields)).is_err());
}

#[test]
fn native_redeem_inputs_are_exact_bounded_canonical_accounts() {
    let mut fields = request_fields("begin-redeem");
    let account = iroha_data_model::account::AccountId::new(
        iroha_crypto::KeyPair::random().public_key().clone(),
    );
    fields[3] = account.encode();
    let method = KagemushaCoreCoordinatorMethodV1::BeginSenderTransition;
    let mut preparation =
        KagemushaCoreSenderPreparationArchiveV1::decode_canonical_exact(&vector("preparation"))
            .unwrap();
    preparation.operation_id.copy_from_slice(&fields[0]);
    let inputs = sender_inputs(&fields).unwrap();
    preparation.inputs_digest = SenderPublicInputPreimageV1 {
        version: 1,
        operation_id: preparation.operation_id,
        context: preparation.context.clone(),
        inputs,
    }
    .canonical_digest()
    .unwrap();
    assert_eq!(
        validate_response(
            method,
            &frame(&fields),
            &response(&[fields[0].clone(), preparation.encode_canonical().unwrap()])
        ),
        Ok(())
    );
    for invalid in [Vec::new(), vec![0; 513], {
        let mut bytes = account.encode();
        bytes.push(0);
        bytes
    }] {
        fields[3] = invalid;
        assert!(validate_request(method, &frame(&fields)).is_err());
    }
}

#[test]
fn native_candidate_and_recovery_reject_nested_identity_substitution() {
    let method = KagemushaCoreCoordinatorMethodV1::ProvePreparedSenderTransition;
    let mut fields = request_fields("prove");
    fields[0] = vector("preparation");
    assert_eq!(
        validate_response(method, &frame(&fields), &response(&[vector("candidate")])),
        Ok(())
    );
    let mut preparation =
        KagemushaCoreSenderPreparationArchiveV1::decode_canonical_exact(&fields[0]).unwrap();
    preparation.context.credential_id[0] ^= 1;
    fields[0] = preparation.encode_canonical().unwrap();
    assert!(validate_response(method, &frame(&fields), &response(&[vector("candidate")])).is_err());
    let method = KagemushaCoreCoordinatorMethodV1::RecoverSender;
    let recovery =
        KagemushaCoreSenderRecoveryArchiveV1::decode_canonical_exact(&vector("recovery")).unwrap();
    let mut fields = request_fields("recover-sender");
    fields[1] = recovery.operation_id.to_vec();
    let mut output = vec![
        recovery.operation_id.to_vec(),
        recovery.terminal_id.to_vec(),
        vector("recovery"),
    ];
    assert_eq!(
        validate_response(method, &frame(&fields), &response(&output)),
        Ok(())
    );
    output[1][0] ^= 1;
    assert!(validate_response(method, &frame(&fields), &response(&output)).is_err());
}

#[test]
fn native_release_validates_signed_authorization_and_exact_terminal_binding() {
    let method = KagemushaCoreCoordinatorMethodV1::ReleaseOutbox;
    let (fields, output) = release_fields();
    assert_eq!(
        validate_response(method, &frame(&fields), &response(&output)),
        Ok(())
    );
    for index in 0..output.len() {
        let mut changed = output.clone();
        changed[index][0] ^= 1;
        assert!(
            validate_response(method, &frame(&fields), &response(&changed)).is_err(),
            "field {index}"
        );
    }
    let mut wrong_terminal = fields.clone();
    wrong_terminal[0][0] ^= 1;
    assert!(validate_response(method, &frame(&wrong_terminal), &response(&output)).is_err());
    let mut receipt_trailing = fields.clone();
    receipt_trailing[4].push(0);
    assert!(validate_request(method, &frame(&receipt_trailing)).is_err());
    let mut opaque_receipt = fields;
    opaque_receipt[4] = [0_u32.to_le_bytes().as_slice(), b"opaque"].concat();
    assert!(validate_request(method, &frame(&opaque_receipt)).is_err());
}

#[test]
fn native_redemption_receipt_rejects_nested_enum_and_trailing_encoding() {
    let bytes = vector("redemption_terminal_receipt");
    let mut field = [1_u32.to_le_bytes().as_slice(), bytes.as_slice()].concat();
    assert!(matches!(
        terminal_receipt(&field),
        Ok(SenderTerminalReceiptV1::RedemptionSettlement(_))
    ));
    field.push(0);
    assert!(terminal_receipt(&field).is_err());
    let receipt =
        terminal_receipt(&[1_u32.to_le_bytes().as_slice(), bytes.as_slice()].concat()).unwrap();
    let nested = norito::encode_canonical(&receipt).unwrap();
    assert!(
        terminal_receipt(&[1_u32.to_le_bytes().as_slice(), nested.as_slice()].concat()).is_err()
    );
}

#[test]
fn native_terminal_envelope_responses_enforce_shared_wire_bound() {
    for (method, name, archive) in [
        (
            KagemushaCoreCoordinatorMethodV1::BuildTerminalEnvelope,
            "terminal-envelope",
            "candidate",
        ),
        (
            KagemushaCoreCoordinatorMethodV1::RecoverTerminalEnvelope,
            "recover-envelope",
            "recovery",
        ),
    ] {
        let mut fields = request_fields(name);
        fields[0] = vector(archive);
        if method == KagemushaCoreCoordinatorMethodV1::RecoverTerminalEnvelope {
            let (recovery_fields, envelope) = installed_recovery_fields();
            fields = recovery_fields;
            assert_eq!(
                validate_response(method, &frame(&fields), &response(&[envelope])),
                Ok(())
            );
        }
        for (size, valid) in [(0, false), (7936, true), (7937, false)] {
            assert_eq!(
                validate_response(method, &frame(&fields), &response(&[vec![1; size]])).is_ok(),
                valid && method == KagemushaCoreCoordinatorMethodV1::BuildTerminalEnvelope
            );
        }
    }
}

#[test]
fn native_recovery_envelope_matches_exact_installed_reply_and_terminal() {
    let method = KagemushaCoreCoordinatorMethodV1::RecoverTerminalEnvelope;
    let (fields, envelope) = installed_recovery_fields();
    assert_eq!(
        validate_response(method, &frame(&fields), &response(&[envelope.clone()])),
        Ok(())
    );
    let mut substituted = envelope.clone();
    substituted[0] ^= 1;
    assert!(validate_response(method, &frame(&fields), &response(&[substituted])).is_err());
    let mut changed = fields.clone();
    let mut recovery =
        KagemushaCoreSenderRecoveryArchiveV1::decode_canonical_exact(&changed[0]).unwrap();
    recovery.terminal_id[0] ^= 1;
    changed[0] = recovery.encode_canonical().unwrap();
    assert!(validate_request(method, &frame(&changed)).is_err());
    changed = fields;
    changed[1].push(0);
    assert!(validate_request(method, &frame(&changed)).is_err());
}

#[test]
fn native_recovery_never_turns_missing_or_released_into_installed_bytes() {
    let method = KagemushaCoreCoordinatorMethodV1::RecoverTerminalEnvelope;
    let (fields, _) = installed_recovery_fields();
    let reply: SenderReplyV1 = norito::decode_canonical(&fields[1]).unwrap();
    let mut missing = reply.clone();
    missing.body = SenderReplyBodyV1::Lookup(None);
    let mut released = reply;
    let SenderReplyBodyV1::Lookup(Some(item)) = &mut released.body else {
        panic!("installed fixture");
    };
    item.record.phase = SenderPhaseV1::Released;
    item.record.inputs = None;
    item.record.terminal_receipt_digest = Some([0x91; 32]);
    item.canonical_envelope.clear();
    for reply in [missing, released] {
        let mut changed = fields.clone();
        changed[1] = norito::encode_canonical(&reply).unwrap();
        assert!(validate_request(method, &frame(&changed)).is_err());
    }
}
