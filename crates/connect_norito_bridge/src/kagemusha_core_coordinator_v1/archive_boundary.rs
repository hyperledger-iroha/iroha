//! Typed archive checks at the C/JNI boundary, separate from transport-only frame fixtures.
//!
//! These checks authenticate no platform or durable state. Every selector must still resolve
//! against the qualified backend's retained operation and nonserializable Core capabilities.

use super::*;
use crate::kagemusha_device_bridge_v1::sender_payload::{
    SenderCommandBodyV1, SenderCommandV1, SenderHardwareAuthorizationV1, SenderPhaseV1,
    SenderPublicInputPreimageV1, SenderPublicInputsV1, SenderRecoverySelectorV1, SenderReplyBodyV1,
    SenderReplyV1, SenderTerminalReceiptV1, terminal_envelope_digest_v1,
};
use iroha_core::zk::kagemusha_v1_state::KagemushaRedemptionTerminalReceiptV1;
use iroha_data_model::{
    account::AccountId,
    kagemusha::{KagemushaAcknowledgementV1, KagemushaPaymentRequestV1},
};
use norito::{DecodeLimits, codec::Encode};

type Result<T> = std::result::Result<T, KagemushaCoreCoordinatorFrameErrorV1>;

fn field_error<T>(_: T) -> KagemushaCoreCoordinatorFrameErrorV1 {
    KagemushaCoreCoordinatorFrameErrorV1::Field
}

fn require_binding(condition: bool) -> Result<()> {
    if condition {
        Ok(())
    } else {
        Err(field_error(()))
    }
}

fn sender_inputs(fields: &[Vec<u8>]) -> Result<SenderPublicInputsV1> {
    match require_u32_field(fields.get(1))? {
        KAGEMUSHA_CORE_COORDINATOR_SEND_SPLIT_V1 => {
            KagemushaPaymentRequestV1::decode_canonical_exact(&fields[2]).map_err(field_error)?;
            Ok(SenderPublicInputsV1::SendSplit {
                request: fields[2].clone(),
            })
        }
        KAGEMUSHA_CORE_COORDINATOR_REDEEM_SPLIT_V1 => {
            // Coordinator schema 2 fixes bare AccountId to Norito's V1 COMPACT_LEN layout.
            // It is bounded and re-encoded exactly; no layout bits are inferred from input.
            let bytes = &fields[3];
            require_binding(!bytes.is_empty() && bytes.len() <= 512)?;
            let beneficiary: AccountId =
                norito::codec::decode_adaptive(bytes).map_err(field_error)?;
            require_binding(beneficiary.encode() == *bytes)?;
            Ok(SenderPublicInputsV1::RedeemSplit {
                amount: u128::from_le_bytes(fields[2].as_slice().try_into().map_err(field_error)?),
                beneficiary,
            })
        }
        _ => Err(field_error(())),
    }
}

fn terminal_receipt(field: &[u8]) -> Result<SenderTerminalReceiptV1> {
    let (tag, bytes) = field
        .split_first_chunk::<4>()
        .ok_or_else(|| field_error(()))?;
    match u32::from_le_bytes(*tag) {
        KAGEMUSHA_CORE_COORDINATOR_SEND_SPLIT_V1 => {
            // Request/payment correlation is checked by the complete release command below.
            let maximum = iroha_data_model::kagemusha::KAGEMUSHA_ACKNOWLEDGEMENT_MAX_BYTES_V1;
            require_binding(!bytes.is_empty() && bytes.len() <= maximum)?;
            let _: KagemushaAcknowledgementV1 = norito::decode_canonical_with_limits(
                bytes,
                DecodeLimits::new(maximum, maximum, maximum * 4, maximum * 8, 32),
            )
            .map_err(field_error)?;
            Ok(SenderTerminalReceiptV1::PaymentAcknowledgement(
                bytes.to_vec(),
            ))
        }
        KAGEMUSHA_CORE_COORDINATOR_REDEEM_SPLIT_V1 => {
            let maximum = KAGEMUSHA_CORE_COORDINATOR_ARCHIVE_MAX_BYTES_V1;
            require_binding(!bytes.is_empty() && bytes.len() <= maximum)?;
            let receipt: KagemushaRedemptionTerminalReceiptV1 =
                norito::decode_canonical_with_limits(
                    bytes,
                    DecodeLimits::new(maximum, maximum, maximum * 4, maximum * 8, 32),
                )
                .map_err(field_error)?;
            receipt.validate_shape().map_err(field_error)?;
            Ok(SenderTerminalReceiptV1::RedemptionSettlement(receipt))
        }
        _ => Err(field_error(())),
    }
}

fn bind_preparation_inputs(
    preparation: &KagemushaCoreSenderPreparationArchiveV1,
    fields: &[Vec<u8>],
) -> Result<SenderPublicInputsV1> {
    let inputs = sender_inputs(fields)?;
    let digest = SenderPublicInputPreimageV1 {
        version: 1,
        operation_id: preparation.operation_id,
        context: preparation.context.clone(),
        inputs: inputs.clone(),
    }
    .canonical_digest()
    .map_err(field_error)?;
    require_binding(digest == preparation.inputs_digest)?;
    Ok(inputs)
}

fn installed_recovery_projection(fields: &[Vec<u8>]) -> Result<Vec<u8>> {
    let recovery = KagemushaCoreSenderRecoveryArchiveV1::decode_canonical_exact(&fields[0])
        .map_err(field_error)?;
    let maximum = crate::kagemusha_device_bridge_v1::sender_payload::SENDER_REPLY_MAX_BYTES_V1;
    require_binding(!fields[1].is_empty() && fields[1].len() <= maximum)?;
    let reply: SenderReplyV1 = norito::decode_canonical_with_limits(
        &fields[1],
        DecodeLimits::new(maximum, maximum, maximum * 4, maximum * 8, 32),
    )
    .map_err(field_error)?;
    let command = SenderCommandV1 {
        version: 1,
        operation: 10,
        operation_id: recovery.operation_id,
        context: recovery.context,
        body: SenderCommandBodyV1::RecoverInstalled {
            selector: SenderRecoverySelectorV1::Lookup {
                inputs_digest: recovery.inputs_digest,
            },
        },
    };
    // The reply's current context is only a public projection here. The qualified backend must
    // resolve this exact reply through its authenticated method-3 journal before using it.
    reply
        .validate_against(&command, &reply.context)
        .map_err(field_error)?;
    let SenderReplyBodyV1::Lookup(Some(item)) = reply.body else {
        return Err(field_error(()));
    };
    require_binding(
        item.record.phase == SenderPhaseV1::Installed
            && item.record.outcome_id == recovery.terminal_id,
    )?;
    Ok(item.canonical_envelope)
}

pub(crate) fn validate_request(
    method: KagemushaCoreCoordinatorMethodV1,
    frame: &[u8],
) -> Result<()> {
    kagemusha_core_coordinator_validate_method_request_v1(method, frame)?;
    let fields = kagemusha_core_coordinator_decode_request_v1(frame)?;
    match method {
        KagemushaCoreCoordinatorMethodV1::BeginObservation => {
            let operation =
                u8::try_from(require_u32_field(fields.first())?).map_err(field_error)?;
            require_binding(
                crate::kagemusha_device_bridge_v1::validate_coordinator_observation_binding_v1(
                    operation, &fields[1],
                ),
            )?;
        }
        KagemushaCoreCoordinatorMethodV1::ReserveOperationId => {
            let operation =
                u8::try_from(require_u32_field(fields.first())?).map_err(field_error)?;
            let operation_id = fields[1].as_slice().try_into().map_err(field_error)?;
            require_binding(
                crate::kagemusha_device_bridge_v1::validate_coordinator_reservation_binding_v1(
                    operation,
                    operation_id,
                    &fields[2],
                ),
            )?;
        }
        KagemushaCoreCoordinatorMethodV1::BeginSenderTransition => {
            sender_inputs(&fields)?;
        }
        KagemushaCoreCoordinatorMethodV1::ProvePreparedSenderTransition => {
            KagemushaCoreSenderPreparationArchiveV1::decode_canonical_exact(&fields[0])
                .map_err(field_error)?;
        }
        KagemushaCoreCoordinatorMethodV1::BuildTerminalEnvelope
        | KagemushaCoreCoordinatorMethodV1::AcceptInstalledTerminal => {
            KagemushaCoreSenderCandidateArchiveV1::decode_canonical_exact(&fields[0])
                .map_err(field_error)?;
            if method == KagemushaCoreCoordinatorMethodV1::AcceptInstalledTerminal {
                terminal_envelope_digest_v1(&fields[1]).map_err(field_error)?;
            }
        }
        KagemushaCoreCoordinatorMethodV1::RecoverTerminalEnvelope => {
            installed_recovery_projection(&fields)?;
        }
        KagemushaCoreCoordinatorMethodV1::ReleaseOutbox => {
            sender_inputs(&fields)?;
            let (_, terminal_start) = require_sender_input_fields(&fields, 1)?;
            terminal_receipt(&fields[terminal_start + 1])?;
        }
        _ => {}
    }
    Ok(())
}

pub(crate) fn validate_response(
    method: KagemushaCoreCoordinatorMethodV1,
    request_frame: &[u8],
    response_frame: &[u8],
) -> Result<()> {
    validate_request(method, request_frame)?;
    kagemusha_core_coordinator_validate_method_response_v1(method, request_frame, response_frame)?;
    let request = kagemusha_core_coordinator_decode_request_v1(request_frame)?;
    let response = kagemusha_core_coordinator_decode_response_v1(response_frame)?;
    match method {
        KagemushaCoreCoordinatorMethodV1::BeginSenderTransition => {
            let preparation =
                KagemushaCoreSenderPreparationArchiveV1::decode_canonical_exact(&response[1])
                    .map_err(field_error)?;
            require_binding(preparation.operation_id.as_slice() == response[0])?;
            bind_preparation_inputs(&preparation, &request)?;
        }
        KagemushaCoreCoordinatorMethodV1::ProvePreparedSenderTransition => {
            let candidate =
                KagemushaCoreSenderCandidateArchiveV1::decode_canonical_exact(&response[0])
                    .map_err(field_error)?;
            require_binding(
                candidate
                    .preparation
                    .encode_canonical()
                    .map_err(field_error)?
                    == request[0],
            )?;
        }
        KagemushaCoreCoordinatorMethodV1::RecoverSender if !response.is_empty() => {
            let recovery =
                KagemushaCoreSenderRecoveryArchiveV1::decode_canonical_exact(&response[2])
                    .map_err(field_error)?;
            require_binding(
                recovery.operation_id.as_slice() == response[0]
                    && recovery.terminal_id.as_slice() == response[1],
            )?;
        }
        KagemushaCoreCoordinatorMethodV1::BuildTerminalEnvelope => {
            terminal_envelope_digest_v1(&response[0]).map_err(field_error)?;
        }
        KagemushaCoreCoordinatorMethodV1::RecoverTerminalEnvelope => {
            require_binding(installed_recovery_projection(&request)? == response[0])?;
        }
        KagemushaCoreCoordinatorMethodV1::ReleaseOutbox => {
            let preparation =
                KagemushaCoreSenderPreparationArchiveV1::decode_canonical_exact(&response[1])
                    .map_err(field_error)?;
            require_binding(preparation.operation_id.as_slice() == response[0])?;
            let inputs = bind_preparation_inputs(&preparation, &request)?;
            let (_, terminal_start) = require_sender_input_fields(&request, 1)?;
            let envelope_digest = terminal_envelope_digest_v1(&response[3]).map_err(field_error)?;
            require_binding(envelope_digest.as_slice() == response[2])?;
            let receipt = terminal_receipt(&request[terminal_start + 1])?;
            // Reuse the hardware op-12 contract, including the exact receipt, canonical low-S
            // authorization, full input preimage and envelope. This remains a public shape
            // check: the backend also needs the admitted key and verified settlement capability.
            SenderCommandV1 {
                version: 1,
                operation: 12,
                operation_id: preparation.operation_id,
                context: preparation.context,
                body: SenderCommandBodyV1::Release {
                    inputs_digest: preparation.inputs_digest,
                    envelope_digest,
                    inputs,
                    envelope: response[3].clone(),
                    terminal_receipt: receipt,
                    hardware_authorization: response[4].clone(),
                },
            }
            .validate_shape()
            .map_err(field_error)?;
            let authorization = SenderHardwareAuthorizationV1::decode_canonical_exact(&response[4])
                .map_err(field_error)?;
            require_binding(authorization.outcome_id.as_slice() == request[0])?;
        }
        _ => {}
    }
    Ok(())
}

#[cfg(test)]
pub(crate) mod tests;
