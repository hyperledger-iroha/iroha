//! Exact sender ACK release using the original simulated provider keys and native transcripts.
//!
//! Genuine State/Guard/Terminal/Wrapper proofs and the actual persisted receiver ACK remain
//! prerequisites. The keys below belong to the explicitly simulated diagnostic provider;
//! a signed component response does not qualify physical hardware or a native Core owner.

use super::device_owner::DiagnosticDeviceV1;
use super::handoff_send::DiagnosticSentPaymentV1;
use super::*;
use crate::kagemusha_sender_wire::{
    SenderCommandBodyV1, SenderCommandV1, SenderHardwareAuthorizationPreimageV1,
    SenderHardwareAuthorizationPurposeV1, SenderPhaseV1, SenderPublicInputsV1, SenderRecordV1,
    SenderRecoveryItemV1, SenderReplyBodyV1, SenderReplyV1, SenderTerminalReceiptV1,
    SenderWalletContextV1, acknowledgement_digest_v1, hardware_authorization_key_reference_v1,
};
use crate::kagemusha_v1_state::{
    DurableAcknowledgementV1, KagemushaOutgoingOperationPhaseV1, KagemushaOutgoingPublicInputsV1,
};
use iroha_data_model::kagemusha::{
    KagemushaAcknowledgementV1, kagemusha_device_response_signing_bytes_v1,
    kagemusha_verify_device_response_v1,
};
use sha2::{Digest as _, Sha256};

fn sign_response(
    key: &SigningKey,
    operation_id: DigestV1,
    command: &[u8],
    reply: &[u8],
    policy: DigestV1,
    report: DigestV1,
) -> Result<Vec<u8>, String> {
    let transcript = kagemusha_device_response_signing_bytes_v1(
        12,
        operation_id,
        command,
        reply,
        policy,
        report,
    )
    .map_err(|error| error.to_string())?;
    let signature = device_signature(key, &transcript);
    let signature = signature.as_raw_bytes();
    let mut response = b"IKGMJRS1".to_vec();
    response.extend_from_slice(&1_u16.to_le_bytes());
    response.extend_from_slice(&[12, 0]);
    response.extend_from_slice(&operation_id);
    response.extend_from_slice(
        &u32::try_from(reply.len())
            .map_err(|error| error.to_string())?
            .to_le_bytes(),
    );
    response.extend_from_slice(&64_u32.to_le_bytes());
    response.extend_from_slice(&Sha256::digest(reply));
    response.extend_from_slice(&Sha256::digest(signature));
    response.extend_from_slice(reply);
    response.extend_from_slice(signature);
    kagemusha_verify_device_response_v1(
        &response,
        command,
        12,
        operation_id,
        policy,
        report,
        &device_public_key(key),
    )
    .map_err(|error| error.to_string())?;
    Ok(response)
}

/// Retire only the installed original after actual receiver staging and exact signed op12.
pub(super) fn release(
    sender: &mut DiagnosticDeviceV1<'_>,
    operation_id: DigestV1,
    sent: &DiagnosticSentPaymentV1,
    acknowledgement: &DurableAcknowledgementV1,
    check_refusals: bool,
) -> Result<(), String> {
    let record = sender
        .machine
        .outgoing_operation_index()
        .lookup(operation_id)
        .cloned()
        .ok_or("original sender operation is missing")?;
    ensure(
        record.phase == KagemushaOutgoingOperationPhaseV1::Installed,
        "ACK release requires the actual installed original",
    )?;
    let actual = sender
        .machine
        .outgoing_candidate_journal()
        .finalized_envelope(record.outbox_reservation_id)
        .ok_or("original installed sender envelope is missing")?;
    let prepared = &actual.committed.candidate.prepared;
    let PreparedOutgoingRecoveryViewV1::Send { request, .. } = prepared.recovery_view() else {
        return Err("original sender preparation is not Send".into());
    };
    let original = norito::encode_canonical(&sent.payment).map_err(|error| error.to_string())?;
    ensure(
        request == &sent.request
            && original == actual.canonical_envelope_bytes
            && record.envelope_digest == Some(actual.envelope_digest),
        "sent proof fixture differs from the installed original",
    )?;
    let parsed = KagemushaAcknowledgementV1::decode_canonical_shape_exact_against(
        &acknowledgement.canonical_bytes,
        &sent.request,
        &sent.payment,
    )
    .map_err(|error| error.to_string())?;
    ensure(
        parsed == acknowledgement.acknowledgement,
        "actual receiver ACK projection differs from its canonical original",
    )?;
    let receipt_digest = acknowledgement_digest_v1(&acknowledgement.canonical_bytes)
        .map_err(|error| error.to_string())?;
    let KagemushaOutgoingPublicInputsV1::SendSplit {
        request: request_bytes,
    } = record
        .inputs
        .clone()
        .ok_or("original sender inputs are missing")?
    else {
        return Err("original sender inputs are not Send".into());
    };
    ensure(
        request_bytes
            == norito::encode_canonical(&sent.request).map_err(|error| error.to_string())?,
        "retained sender request bytes differ",
    )?;
    let core_public_key = device_public_key(&sender.journal_key);
    ensure(
        hardware_authorization_key_reference_v1(&core_public_key)
            == sender
                .machine
                .enrollment_binding()
                .core_authorization_key_reference
            && device_public_key(&sender.device_key)
                == sender.material.hardware_credential.device_public_key,
        "release signer keys must be retained from actual device bootstrap",
    )?;
    let terminal = actual
        .committed
        .candidate
        .hardware_terminal_body()
        .map_err(|error| error.to_string())?;
    let mut nonce = Sha256::new();
    nonce.update(b"iroha:kagemusha:diagnostic:original-release-challenge\0");
    nonce.update(operation_id);
    nonce.update(record.record_revision.to_le_bytes());
    nonce.update(receipt_digest);
    let authorization = SenderHardwareAuthorizationPreimageV1 {
        version: 1,
        purpose: SenderHardwareAuthorizationPurposeV1::Release,
        operation_id,
        inputs_digest: record.inputs_digest,
        preparation_id: record.preparation_id,
        candidate_digest: record
            .candidate_digest
            .ok_or("original candidate is missing")?,
        release_id: record.context.release.release_id,
        hardware_transition_statement: prepared.hardware_statement(),
        prepared_one_use_authorization_digest: prepared.prepared_one_use_authorization_digest,
        outbox_reservation_commitment: terminal.outbox_reservation_commitment,
        outcome_id: record.outcome_id,
        transition_nullifier: terminal.transition_nullifier,
        envelope_digest: record.envelope_digest,
        terminal_receipt_digest: Some(receipt_digest),
        hardware_one_use_nonce: nonce.finalize().into(),
        authorization_public_key: core_public_key,
    };
    let signature = device_signature(
        &sender.journal_key,
        &authorization
            .authorization_id()
            .map_err(|error| error.to_string())?,
    );
    let authorization = authorization
        .with_signature(signature)
        .map_err(|error| error.to_string())?;
    let command = SenderCommandV1 {
        version: 1,
        operation: 12,
        operation_id,
        context: record.context.clone(),
        body: SenderCommandBodyV1::Release {
            inputs_digest: record.inputs_digest,
            envelope_digest: actual.envelope_digest,
            inputs: SenderPublicInputsV1::SendSplit {
                request: request_bytes,
            },
            envelope: original,
            terminal_receipt: SenderTerminalReceiptV1::PaymentAcknowledgement(
                acknowledgement.canonical_bytes.clone(),
            ),
            hardware_authorization: norito::encode_canonical(&authorization)
                .map_err(|error| error.to_string())?,
        },
    };
    let canonical_command = command
        .encode_canonical()
        .map_err(|error| error.to_string())?;
    let next_revision = sender
        .machine
        .outgoing_operation_index()
        .revision()
        .checked_add(1)
        .ok_or("release index revision overflow")?;
    let state = sender.machine.state();
    let credential = sender
        .machine
        .accepted_credential_floor()
        .oem_original()
        .map_err(|error| error.to_string())?;
    let current_context = SenderWalletContextV1 {
        lane: state.lane.clone(),
        release: state.context(),
        credential_id: credential.credential_id,
        hardware_epoch: state.hardware_epoch,
        device_policy_binding: state.device_policy_binding,
        core_authorization_key_reference: sender
            .machine
            .enrollment_binding()
            .core_authorization_key_reference,
    };
    let reply = SenderReplyV1 {
        version: 1,
        operation: 12,
        request_id: operation_id,
        context: current_context.clone(),
        index_revision: next_revision,
        body: SenderReplyBodyV1::Lookup(Some(SenderRecoveryItemV1 {
            record: SenderRecordV1 {
                operation_id,
                context: record.context.clone(),
                inputs_digest: record.inputs_digest,
                operation_kind: record.operation_kind,
                preparation_id: record.preparation_id,
                outbox_reservation_id: record.outbox_reservation_id,
                outcome_id: record.outcome_id,
                phase: SenderPhaseV1::Released,
                record_revision: next_revision,
                inputs: None,
                candidate_digest: record.candidate_digest,
                commit_certificate_digest: record.commit_certificate_digest,
                envelope_digest: record.envelope_digest,
                terminal_receipt_digest: Some(receipt_digest),
            },
            canonical_envelope: Vec::new(),
        })),
    };
    let canonical_reply = reply
        .encode_canonical(&command, &current_context)
        .map_err(|error| error.to_string())?;
    let (policy, report, original_device_key) = sender
        .machine
        .diagnostic_release_response_context()
        .map_err(|error| error.to_string())?;
    ensure(
        original_device_key == device_public_key(&sender.device_key),
        "response signer differs from the original machine credential",
    )?;
    let response = sign_response(
        &sender.device_key,
        operation_id,
        &canonical_command,
        &canonical_reply,
        policy,
        report,
    )?;
    let before = sender
        .machine
        .snapshot()
        .map_err(|error| error.to_string())?;
    if check_refusals {
        let mut changed_id = operation_id;
        changed_id[0] ^= 1;
        assert!(
            sender
                .machine
                .diagnostic_release_outgoing_payment(changed_id, &canonical_command, &response,)
                .is_err()
        );
        let mut changed_response = response.clone();
        *changed_response
            .last_mut()
            .ok_or("original response is empty")? ^= 1;
        assert!(
            sender
                .machine
                .diagnostic_release_outgoing_payment(
                    operation_id,
                    &canonical_command,
                    &changed_response,
                )
                .is_err()
        );
        assert_eq!(
            sender
                .machine
                .snapshot()
                .map_err(|error| error.to_string())?,
            before
        );
    }
    sender
        .machine
        .diagnostic_release_outgoing_payment(operation_id, &canonical_command, &response)
        .map_err(|error| error.to_string())?;
    let after = sender
        .machine
        .snapshot()
        .map_err(|error| error.to_string())?;
    assert_eq!(after.state, before.state);
    assert_eq!(after.journal_revision, before.journal_revision);
    let released = sender
        .machine
        .outgoing_operation_index()
        .lookup(operation_id)
        .ok_or("released operation tombstone is missing")?;
    assert_eq!(released.phase, KagemushaOutgoingOperationPhaseV1::Released);
    assert_eq!(released.inputs, None);
    assert_eq!(released.record_revision, next_revision);
    assert_eq!(released.envelope_digest, record.envelope_digest);
    assert_eq!(released.terminal_receipt_digest, Some(receipt_digest));
    assert!(
        sender
            .machine
            .outgoing_candidate_journal()
            .finalized_envelope(record.outbox_reservation_id)
            .is_none()
    );
    Ok(())
}

#[test]
fn simulated_release_response_preserves_the_native_command_bound_signature() {
    let key = deterministic_signing_key(0);
    let response = sign_response(&key, [7; 32], b"command", b"reply", [8; 32], [9; 32]).unwrap();
    let verified = kagemusha_verify_device_response_v1(
        &response,
        b"command",
        12,
        [7; 32],
        [8; 32],
        [9; 32],
        &device_public_key(&key),
    )
    .unwrap();
    assert_eq!(verified.payload, b"reply");
    for (command, operation, id, policy, report, public_key) in [
        (
            b"other".as_slice(),
            12,
            [7; 32],
            [8; 32],
            [9; 32],
            device_public_key(&key),
        ),
        (
            b"command".as_slice(),
            7,
            [7; 32],
            [8; 32],
            [9; 32],
            device_public_key(&key),
        ),
        (
            b"command".as_slice(),
            12,
            [6; 32],
            [8; 32],
            [9; 32],
            device_public_key(&key),
        ),
        (
            b"command".as_slice(),
            12,
            [7; 32],
            [6; 32],
            [9; 32],
            device_public_key(&key),
        ),
        (
            b"command".as_slice(),
            12,
            [7; 32],
            [8; 32],
            [6; 32],
            device_public_key(&key),
        ),
        (
            b"command".as_slice(),
            12,
            [7; 32],
            [8; 32],
            [9; 32],
            device_public_key(&deterministic_signing_key(1)),
        ),
    ] {
        assert!(
            kagemusha_verify_device_response_v1(
                &response,
                command,
                operation,
                id,
                policy,
                report,
                &public_key,
            )
            .is_err()
        );
    }
    let mut suffix = response.clone();
    suffix.push(0);
    assert!(
        kagemusha_verify_device_response_v1(
            &suffix,
            b"command",
            12,
            [7; 32],
            [8; 32],
            [9; 32],
            &device_public_key(&key),
        )
        .is_err()
    );
}
