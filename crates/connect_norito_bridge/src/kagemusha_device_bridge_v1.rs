//! Fail-closed KAGEMUSHA V1 command framing for the optional secure-device service.
//!
//! This module validates the complete outer command frame before dispatch. The
//! stock bridge deliberately has no successful monetary engine: exact commands
//! return unavailable and malformed commands return a distinct error. Every
//! operation decodes its operation-specific canonical Norito body before the
//! unavailable result is selected.

use iroha_data_model::kagemusha::{
    KAGEMUSHA_DEVICE_SIGNATURE_BYTES_V1, KagemushaDeviceMintStageCommandV1,
    KagemushaDevicePublicKeyV1, KagemushaDeviceSignatureV1, KagemushaHardwareCredentialV1,
    KagemushaHardwareProfileV1, kagemusha_decode_device_success_response_v1,
    kagemusha_device_response_signing_bytes_v1, kagemusha_verify_device_response_v1,
};
use iroha_data_model::{NetworkId, asset::AssetDefinitionId, nexus::AxtAssetIncarnationV1};
use sha2::{Digest as _, Sha256};

use crate::KagemushaDeviceLifecycleOperationV1;

mod control_payload;
mod receiver_payload;
pub(crate) mod sender_payload;

pub(super) const COMMAND_HEADER_BYTES_V1: usize = 80;
pub(super) const MAX_COMMAND_PAYLOAD_BYTES_V1: usize = 64 * 1024;
pub(super) const MAX_COMMAND_BYTES_V1: usize =
    COMMAND_HEADER_BYTES_V1 + MAX_COMMAND_PAYLOAD_BYTES_V1;
pub(super) const RESPONSE_HEADER_BYTES_V1: usize = 116;
pub(super) const MAX_RESPONSE_PAYLOAD_BYTES_V1: usize = 64 * 1024;
pub(super) const RESPONSE_AUTHENTICATOR_BYTES_V1: usize = KAGEMUSHA_DEVICE_SIGNATURE_BYTES_V1;
pub(super) const MAX_RESPONSE_BYTES_V1: usize =
    RESPONSE_HEADER_BYTES_V1 + MAX_RESPONSE_PAYLOAD_BYTES_V1 + RESPONSE_AUTHENTICATOR_BYTES_V1;

const COMMAND_MAGIC_V1: &[u8; 8] = b"IKGMJCM1";
const RESPONSE_MAGIC_V1: &[u8; 8] = b"IKGMJRS1";
const DEVICE_BRIDGE_VERSION_V1: u16 = 1;

/// Public wallet selectors independently pinned by native enrollment/configuration.
/// Constructing these fields supplies no authenticated state or bootstrap authority.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ObservationWalletContextV1 {
    pub(crate) network_id: NetworkId,
    pub(crate) lane_id: [u8; 32],
    pub(crate) asset: AssetDefinitionId,
    pub(crate) asset_incarnation: AxtAssetIncarnationV1,
    pub(crate) scale: u32,
}

/// Shape-checked startup projection; catalog membership and freshness are separate native checks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct QualificationProjectionV1 {
    pub(crate) release_id: [u8; 32],
    pub(crate) hardware_policy_digest: [u8; 32],
    pub(crate) core_authorization_key_reference: [u8; 32],
    pub(crate) profile: KagemushaHardwareProfileV1,
    pub(crate) credential: KagemushaHardwareCredentialV1,
}

pub(crate) fn qualification_projection_v1(bytes: &[u8]) -> Option<QualificationProjectionV1> {
    control_payload::qualification_projection_v1(bytes).ok()
}

/// Verify exact coordinator reply components using the sole device signature transcript.
/// This grants no catalog membership, current credential status, or outstanding-challenge authority.
pub(crate) fn verify_observation_reply_v1(
    operation: u8,
    request_id: [u8; 32],
    command: &[u8],
    reply: &[u8],
    authenticator: &[u8],
    qualification: &QualificationProjectionV1,
    wallet: &ObservationWalletContextV1,
) -> bool {
    if !validate_coordinator_observation_binding_v1(operation, command)
        || request_id == [0; 32]
        || reply.is_empty()
        || reply.len() > MAX_RESPONSE_PAYLOAD_BYTES_V1
        || authenticator.len() != RESPONSE_AUTHENTICATOR_BYTES_V1
    {
        return false;
    }
    let Ok(decoded_command) =
        control_payload::decode_control_command_v1(operation, request_id, command)
    else {
        return false;
    };
    if control_payload::validate_control_reply_v1(&decoded_command, reply).is_err()
        || control_payload::validate_observation_reply_context_v1(
            operation,
            reply,
            qualification,
            wallet,
        )
        .is_err()
    {
        return false;
    }
    let Some(operation) = KagemushaDeviceLifecycleOperationV1::from_code(operation) else {
        return false;
    };
    let frame = DeviceSuccessResponseFrameV1 {
        operation,
        request_id,
        payload: reply,
        authenticator,
    };
    let Some(transcript) = response_authenticator_transcript_v1(
        frame,
        command,
        qualification.hardware_policy_digest,
        qualification.profile.qualification_report_digest,
    ) else {
        return false;
    };
    KagemushaDeviceSignatureV1::from_raw_bytes(authenticator)
        .and_then(|signature| {
            signature.verify(&qualification.credential.device_public_key, &transcript)
        })
        .is_ok()
}

/// Stock-service disposition after exact frame and implemented payload checks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum StockDeviceCommandDispositionV1 {
    /// The outer frame or operation-specific canonical payload is malformed.
    Malformed,
    /// The command is structurally valid, but no qualified service is installed.
    Unavailable,
}

/// JNI projection of a bounded C execution result without signed-length casts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum JniExecutionDispositionV1 {
    /// Exactly this many bounded bytes may be copied into a Java result array.
    Response(usize),
    /// The complete optional hardware service is unavailable.
    Unavailable,
    /// The caller supplied a malformed command.
    Malformed,
    /// A native status or response-length invariant failed.
    Failed,
}

/// Admit a nonnegative JNI array length only inside the complete command bound.
pub(super) fn bounded_jni_command_length_v1(length: i32) -> Option<usize> {
    let length = usize::try_from(length).ok()?;
    (COMMAND_HEADER_BYTES_V1..=MAX_COMMAND_BYTES_V1)
        .contains(&length)
        .then_some(length)
}

/// Map one C status and full-width output length into the JNI result policy.
pub(super) fn classify_jni_execution_v1(
    status: i32,
    written: usize,
    capacity: usize,
) -> JniExecutionDispositionV1 {
    match status {
        0 if capacity <= MAX_RESPONSE_BYTES_V1
            && (RESPONSE_HEADER_BYTES_V1..=capacity).contains(&written) =>
        {
            JniExecutionDispositionV1::Response(written)
        }
        crate::ERR_KAGEMUSHA_DEVICE_UNAVAILABLE_V1 if written == 0 => {
            JniExecutionDispositionV1::Unavailable
        }
        crate::ERR_KAGEMUSHA_V1 if written == 0 => JniExecutionDispositionV1::Malformed,
        _ => JniExecutionDispositionV1::Failed,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct DeviceCommandFrameV1<'a> {
    operation: KagemushaDeviceLifecycleOperationV1,
    request_id: [u8; 32],
    payload: &'a [u8],
}

fn decode_command_frame_v1(bytes: &[u8]) -> Option<DeviceCommandFrameV1<'_>> {
    if !(COMMAND_HEADER_BYTES_V1..=MAX_COMMAND_BYTES_V1).contains(&bytes.len())
        || bytes.get(..8)? != COMMAND_MAGIC_V1
        || u16::from_le_bytes(bytes.get(8..10)?.try_into().ok()?) != DEVICE_BRIDGE_VERSION_V1
        || bytes[11] != 0
    {
        return None;
    }
    let operation = KagemushaDeviceLifecycleOperationV1::from_code(bytes[10])?;
    let request_id: [u8; 32] = bytes.get(12..44)?.try_into().ok()?;
    if request_id == [0; 32] {
        return None;
    }
    let payload_len =
        usize::try_from(u32::from_le_bytes(bytes.get(44..48)?.try_into().ok()?)).ok()?;
    if payload_len == 0
        || payload_len > MAX_COMMAND_PAYLOAD_BYTES_V1
        || COMMAND_HEADER_BYTES_V1.checked_add(payload_len)? != bytes.len()
    {
        return None;
    }
    let payload = bytes.get(COMMAND_HEADER_BYTES_V1..)?;
    let expected_digest: [u8; 32] = bytes.get(48..80)?.try_into().ok()?;
    let actual_digest: [u8; 32] = Sha256::digest(payload).into();
    if expected_digest != actual_digest {
        return None;
    }
    Some(DeviceCommandFrameV1 {
        operation,
        request_id,
        payload,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct DeviceSuccessResponseFrameV1<'a> {
    operation: KagemushaDeviceLifecycleOperationV1,
    request_id: [u8; 32],
    payload: &'a [u8],
    authenticator: &'a [u8],
}

fn decode_success_response_frame_v1(
    bytes: &[u8],
    expected_operation: KagemushaDeviceLifecycleOperationV1,
    expected_request_id: [u8; 32],
) -> Option<DeviceSuccessResponseFrameV1<'_>> {
    let frame = kagemusha_decode_device_success_response_v1(
        bytes,
        expected_operation.code(),
        expected_request_id,
    )
    .ok()?;
    Some(DeviceSuccessResponseFrameV1 {
        operation: expected_operation,
        request_id: frame.request_id,
        payload: frame.payload,
        authenticator: frame.authenticator,
    })
}

/// Build the sole V1 response-authenticator transcript.
///
/// The authenticator digest from the transport header is deliberately absent:
/// it hashes the signature itself and therefore cannot be a signature input.
fn response_authenticator_transcript_v1(
    frame: DeviceSuccessResponseFrameV1<'_>,
    canonical_command: &[u8],
    hardware_policy_id: [u8; 32],
    qualification_report_digest: [u8; 32],
) -> Option<Vec<u8>> {
    kagemusha_device_response_signing_bytes_v1(
        frame.operation.code(),
        frame.request_id,
        canonical_command,
        frame.payload,
        hardware_policy_id,
        qualification_report_digest,
    )
    .ok()
}

/// Verify a successful response under an already accepted device key.
///
/// Operation 1 callers must first validate its profile/credential payload and
/// resolve the returned release through Core's authenticated release catalog.
pub(super) fn verify_success_response_authenticator_v1(
    bytes: &[u8],
    canonical_command: &[u8],
    expected_operation: KagemushaDeviceLifecycleOperationV1,
    expected_request_id: [u8; 32],
    hardware_policy_id: [u8; 32],
    qualification_report_digest: [u8; 32],
    device_public_key: &KagemushaDevicePublicKeyV1,
) -> bool {
    kagemusha_verify_device_response_v1(
        bytes,
        canonical_command,
        expected_operation.code(),
        expected_request_id,
        hardware_policy_id,
        qualification_report_digest,
        device_public_key,
    )
    .is_ok()
}

/// Bootstrap the response key from operation 1 after validating its profile
/// and credential chain against the two accepted capability-frame digests.
///
/// The returned release identifier still needs authenticated catalog
/// membership before a wallet session may perform monetary operations.
pub(super) fn verify_qualification_response_authenticator_v1(
    bytes: &[u8],
    canonical_command: &[u8],
    expected_request_id: [u8; 32],
    hardware_policy_id: [u8; 32],
    qualification_report_digest: [u8; 32],
) -> Option<([u8; 32], KagemushaDevicePublicKeyV1)> {
    let operation = KagemushaDeviceLifecycleOperationV1::ReadActiveHardwareCredential;
    let frame = decode_success_response_frame_v1(bytes, operation, expected_request_id)?;
    let (release_id, device_public_key) = control_payload::qualification_response_key_v1(
        frame.payload,
        hardware_policy_id,
        qualification_report_digest,
    )
    .ok()?;
    verify_success_response_authenticator_v1(
        bytes,
        canonical_command,
        operation,
        expected_request_id,
        hardware_policy_id,
        qualification_report_digest,
        &device_public_key,
    )
    .then_some((release_id, device_public_key))
}

/// Validate one command for the stock service and return only a fail-closed result.
///
/// This function has no success type, which prevents the generic bridge from
/// accidentally treating shape validation as monetary authority.
pub(super) fn classify_stock_device_command_v1(bytes: &[u8]) -> StockDeviceCommandDispositionV1 {
    let Some(frame) = decode_command_frame_v1(bytes) else {
        return StockDeviceCommandDispositionV1::Malformed;
    };
    classify_stock_device_payload_v1(frame)
}

fn classify_stock_device_payload_v1(
    frame: DeviceCommandFrameV1<'_>,
) -> StockDeviceCommandDispositionV1 {
    match frame.operation {
        KagemushaDeviceLifecycleOperationV1::ReadActiveHardwareCredential
        | KagemushaDeviceLifecycleOperationV1::SignReceiveAcknowledgement
        | KagemushaDeviceLifecycleOperationV1::ReadTrustedTimeOrLease
        | KagemushaDeviceLifecycleOperationV1::PrepareMintAuthorization
        | KagemushaDeviceLifecycleOperationV1::RecoverMintAuthorization
        | KagemushaDeviceLifecycleOperationV1::FoldReceiveCredit
        | KagemushaDeviceLifecycleOperationV1::ReadPendingCreditWatermark
        | KagemushaDeviceLifecycleOperationV1::RotateHardwareEpoch
        | KagemushaDeviceLifecycleOperationV1::BootstrapAggregateState
        | KagemushaDeviceLifecycleOperationV1::RecoverWalletSnapshot
        | KagemushaDeviceLifecycleOperationV1::CreateSignedPaymentRequest => {
            match control_payload::dispatch_unavailable_control_v1(
                frame.request_id,
                frame.operation.code(),
                frame.payload,
            ) {
                Err(control_payload::ControlErrorV1::Unavailable(missing)) => {
                    let _ = missing;
                    StockDeviceCommandDispositionV1::Unavailable
                }
                Err(_) | Ok(_) => StockDeviceCommandDispositionV1::Malformed,
            }
        }
        KagemushaDeviceLifecycleOperationV1::StageInboundPayment
        | KagemushaDeviceLifecycleOperationV1::RecoverStagedInboundPayment
        | KagemushaDeviceLifecycleOperationV1::RecoverInboundInboxPage => {
            match receiver_payload::dispatch_unavailable_receiver_v1(
                frame.request_id,
                frame.operation.code(),
                frame.payload,
            ) {
                Err(receiver_payload::ReceiverErrorV1::Unavailable(missing)) => {
                    let _ = missing;
                    StockDeviceCommandDispositionV1::Unavailable
                }
                Err(_) | Ok(_) => StockDeviceCommandDispositionV1::Malformed,
            }
        }
        KagemushaDeviceLifecycleOperationV1::PrepareExactNextTransition
        | KagemushaDeviceLifecycleOperationV1::RecoverPreparedTransition
        | KagemushaDeviceLifecycleOperationV1::CommitVerifiedCandidateAndSignTerminal
        | KagemushaDeviceLifecycleOperationV1::RecoverTerminalOutcome
        | KagemushaDeviceLifecycleOperationV1::InstallTerminalEnvelope
        | KagemushaDeviceLifecycleOperationV1::RecoverInstalledEnvelopeOrStateProof
        | KagemushaDeviceLifecycleOperationV1::ReleaseOutboxEntry => {
            if sender_payload::SenderCommandV1::decode_canonical_exact(
                frame.operation.code(),
                frame.request_id,
                frame.payload,
            )
            .is_ok()
            {
                StockDeviceCommandDispositionV1::Unavailable
            } else {
                StockDeviceCommandDispositionV1::Malformed
            }
        }
        KagemushaDeviceLifecycleOperationV1::VerifyAuthorizationAndStageMintCredit => {
            let Ok(command) =
                KagemushaDeviceMintStageCommandV1::decode_canonical_shape_exact(frame.payload)
            else {
                return StockDeviceCommandDispositionV1::Malformed;
            };
            let Ok((authorization, _)) = command.validated_inputs() else {
                return StockDeviceCommandDispositionV1::Malformed;
            };
            if authorization.statement.context.operation_id != frame.request_id {
                StockDeviceCommandDispositionV1::Malformed
            } else {
                StockDeviceCommandDispositionV1::Unavailable
            }
        }
    }
}

/// Validate the sole reservation binding contract before native coordinator admission.
/// Sender preparation reserves its tagged public inputs before the private preparation exists;
/// every other operation reserves its exact typed device command body. This proves shape and
/// nonce correlation only. No command is dispatched or authenticated by this predicate.
pub(crate) fn validate_coordinator_reservation_binding_v1(
    operation: u8,
    operation_id: [u8; 32],
    binding: &[u8],
) -> bool {
    if crate::kagemusha_core_coordinator_v1::is_observation_operation_v1(u32::from(operation)) {
        return false;
    }
    let Some(operation) = KagemushaDeviceLifecycleOperationV1::from_code(operation) else {
        return false;
    };
    if operation_id == [0; 32] || binding.is_empty() || binding.len() > MAX_COMMAND_PAYLOAD_BYTES_V1
    {
        return false;
    }
    if operation == KagemushaDeviceLifecycleOperationV1::PrepareExactNextTransition {
        use iroha_core::zk::kagemusha_v1_state::KagemushaOutgoingPublicInputsV1;
        let maximum = MAX_COMMAND_PAYLOAD_BYTES_V1;
        let Ok(inputs) = norito::decode_canonical_with_limits::<KagemushaOutgoingPublicInputsV1>(
            binding,
            norito::DecodeLimits::new(maximum, maximum, maximum * 4, maximum * 8, 32),
        ) else {
            return false;
        };
        if !norito::encode_canonical(&inputs).is_ok_and(|canonical| canonical == binding) {
            return false;
        }
        return match &inputs {
            KagemushaOutgoingPublicInputsV1::SendSplit { .. } => inputs.decode_send_parts().is_ok(),
            KagemushaOutgoingPublicInputsV1::RedeemSplit { amount, .. } => *amount != 0,
        };
    }
    classify_stock_device_payload_v1(DeviceCommandFrameV1 {
        operation,
        request_id: operation_id,
        payload: binding,
    }) == StockDeviceCommandDispositionV1::Unavailable
}

/// Check a transient read's exact typed body without allocating a nonce or executing a device.
pub(crate) fn validate_coordinator_observation_binding_v1(operation: u8, binding: &[u8]) -> bool {
    if !crate::kagemusha_core_coordinator_v1::is_observation_operation_v1(u32::from(operation))
        || binding.is_empty()
        || binding.len() > MAX_COMMAND_PAYLOAD_BYTES_V1
    {
        return false;
    }
    control_payload::decode_observation_command_v1(operation, binding).is_ok()
}

/// Construct an implemented canonical body inside an exact test command frame.
#[cfg(test)]
pub(crate) fn canonical_stock_command_for_tests(
    operation: KagemushaDeviceLifecycleOperationV1,
) -> Option<Vec<u8>> {
    let (request_id, payload) = if operation
        == KagemushaDeviceLifecycleOperationV1::VerifyAuthorizationAndStageMintCredit
    {
        let payload = canonical_mint_stage_fixture_bytes_for_tests("command");
        let command = KagemushaDeviceMintStageCommandV1::decode_canonical_shape_exact(&payload)
            .expect("canonical mint-stage fixture");
        let (authorization, _) = command
            .validated_inputs()
            .expect("bound mint-stage fixture");
        (authorization.statement.context.operation_id, payload)
    } else if matches!(
        operation,
        KagemushaDeviceLifecycleOperationV1::StageInboundPayment
            | KagemushaDeviceLifecycleOperationV1::RecoverStagedInboundPayment
            | KagemushaDeviceLifecycleOperationV1::RecoverInboundInboxPage
    ) {
        receiver_payload::canonical_command_body_for_tests(operation.code())?
    } else {
        let request_id =
            control_payload::canonical_request_id_for_tests(operation.code()).unwrap_or([7; 32]);
        let payload =
            control_payload::canonical_command_body_for_tests(operation.code(), request_id)
                .or_else(|| sender_payload::canonical_command_body_for_tests(operation.code()))?;
        (request_id, payload)
    };
    Some(tests::frame(operation.code(), request_id, &payload))
}

/// Load one Rust-owned structural mint-stage fixture section without proof authority.
#[cfg(test)]
pub(crate) fn canonical_mint_stage_fixture_bytes_for_tests(name: &str) -> Vec<u8> {
    let fixture: norito::json::Value = norito::json::from_str(include_str!(
        "../../../fixtures/offline/kagemusha_device_mint_stage_v1.json"
    ))
    .expect("canonical mint-stage fixture JSON");
    let fields = fixture.as_object().expect("fixture object");
    let section = fields
        .get(name)
        .and_then(norito::json::Value::as_object)
        .expect("fixture section");
    let encoded = section
        .get("hex")
        .and_then(norito::json::Value::as_str)
        .expect("fixture hex");
    hex::decode(encoded).expect("canonical fixture bytes")
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(super) fn frame(operation: u8, request_id: [u8; 32], payload: &[u8]) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(COMMAND_HEADER_BYTES_V1 + payload.len());
        bytes.extend_from_slice(COMMAND_MAGIC_V1);
        bytes.extend_from_slice(&DEVICE_BRIDGE_VERSION_V1.to_le_bytes());
        bytes.push(operation);
        bytes.push(0);
        bytes.extend_from_slice(&request_id);
        bytes.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&Sha256::digest(payload));
        bytes.extend_from_slice(payload);
        bytes
    }

    #[test]
    fn outer_frame_validates_every_fixed_binding_before_unavailable() {
        let payload = control_payload::canonical_command_body_for_tests(1, [7; 32]).unwrap();
        let original = frame(1, [7; 32], &payload);
        assert_eq!(
            classify_stock_device_command_v1(&original),
            StockDeviceCommandDispositionV1::Unavailable
        );

        let mut invalid = Vec::new();
        let mutate = |index: usize, value: u8| {
            let mut candidate = original.clone();
            candidate[index] = value;
            candidate
        };
        invalid.push(mutate(0, b'X')); // magic
        invalid.push(mutate(8, 2)); // version
        invalid.push(mutate(10, 0)); // closed operation code
        invalid.push(mutate(11, 1)); // reserved flags
        let mut zero_id = original.clone();
        zero_id[12..44].fill(0);
        invalid.push(zero_id);
        invalid.push(mutate(44, 2)); // declared payload length
        invalid.push(mutate(48, original[48] ^ 1)); // payload digest
        let mut suffix = original.clone();
        suffix.push(0);
        invalid.push(suffix);
        invalid.push(frame(1, [7; 32], &[]));

        for candidate in invalid {
            assert_eq!(
                classify_stock_device_command_v1(&candidate),
                StockDeviceCommandDispositionV1::Malformed
            );
        }
    }

    #[test]
    fn success_response_authenticator_binds_header_payload_and_capabilities() {
        use p256::ecdsa::{Signature as P256Signature, SigningKey, signature::Signer as _};

        let operation = KagemushaDeviceLifecycleOperationV1::ReadPendingCreditWatermark;
        let request_id = [0x61; 32];
        let hardware_policy_id = [0x62; 32];
        let qualification_report_digest = [0x63; 32];
        let payload = b"canonical-success-body";
        let command = b"canonical-command-body";
        let signing_key = SigningKey::from_bytes((&[0x64; 32]).into()).unwrap();
        let device_public_key = KagemushaDevicePublicKeyV1::from_sec1_bytes(
            signing_key
                .verifying_key()
                .to_encoded_point(false)
                .as_bytes(),
        )
        .unwrap();

        let mut response = Vec::with_capacity(
            RESPONSE_HEADER_BYTES_V1 + payload.len() + RESPONSE_AUTHENTICATOR_BYTES_V1,
        );
        response.extend_from_slice(RESPONSE_MAGIC_V1);
        response.extend_from_slice(&DEVICE_BRIDGE_VERSION_V1.to_le_bytes());
        response.push(operation.code());
        response.push(0);
        response.extend_from_slice(&request_id);
        response.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        response.extend_from_slice(&(RESPONSE_AUTHENTICATOR_BYTES_V1 as u32).to_le_bytes());
        response.extend_from_slice(&Sha256::digest(payload));
        let placeholder = [0; RESPONSE_AUTHENTICATOR_BYTES_V1];
        response.extend_from_slice(&Sha256::digest(placeholder));
        response.extend_from_slice(payload);
        response.extend_from_slice(&placeholder);
        let frame = decode_success_response_frame_v1(&response, operation, request_id).unwrap();
        let transcript = response_authenticator_transcript_v1(
            frame,
            command,
            hardware_policy_id,
            qualification_report_digest,
        )
        .unwrap();
        let signature: P256Signature = signing_key.sign(&transcript);
        let signature = signature.normalize_s().unwrap_or(signature);
        let signature_bytes = signature.to_bytes();
        response[84..116].copy_from_slice(&Sha256::digest(signature_bytes));
        let authenticator_offset = RESPONSE_HEADER_BYTES_V1 + payload.len();
        response[authenticator_offset..].copy_from_slice(&signature_bytes);

        assert!(verify_success_response_authenticator_v1(
            &response,
            command,
            operation,
            request_id,
            hardware_policy_id,
            qualification_report_digest,
            &device_public_key,
        ));
        assert_eq!(
            unsafe {
                crate::connect_norito_kagemusha_device_command_response_v1_verify(
                    response.as_ptr(),
                    response.len(),
                    command.as_ptr(),
                    command.len(),
                    operation.code(),
                    request_id.as_ptr(),
                    request_id.len(),
                    hardware_policy_id.as_ptr(),
                    hardware_policy_id.len(),
                    qualification_report_digest.as_ptr(),
                    qualification_report_digest.len(),
                    device_public_key.as_ref().as_ptr(),
                    device_public_key.as_ref().len(),
                )
            },
            0,
        );
        assert!(!verify_success_response_authenticator_v1(
            &response,
            command,
            operation,
            request_id,
            [0x65; 32],
            qualification_report_digest,
            &device_public_key,
        ));
        assert!(!verify_success_response_authenticator_v1(
            &response,
            command,
            operation,
            [0x66; 32],
            hardware_policy_id,
            qualification_report_digest,
            &device_public_key,
        ));

        let mut altered_payload = response.clone();
        assert!(!verify_success_response_authenticator_v1(
            &response,
            b"another-command-with-the-same-native-nonce",
            operation,
            request_id,
            hardware_policy_id,
            qualification_report_digest,
            &device_public_key,
        ));
        altered_payload[RESPONSE_HEADER_BYTES_V1] ^= 1;
        assert!(!verify_success_response_authenticator_v1(
            &altered_payload,
            command,
            operation,
            request_id,
            hardware_policy_id,
            qualification_report_digest,
            &device_public_key,
        ));
        let mut wrong_width = response;
        wrong_width[48..52].copy_from_slice(&63_u32.to_le_bytes());
        wrong_width.pop();
        assert!(!verify_success_response_authenticator_v1(
            &wrong_width,
            command,
            operation,
            request_id,
            hardware_policy_id,
            qualification_report_digest,
            &device_public_key,
        ));
    }

    #[test]
    fn receiver_operation_rejects_noncanonical_body_before_unavailable() {
        assert_eq!(
            classify_stock_device_command_v1(&frame(4, [7; 32], &[9])),
            StockDeviceCommandDispositionV1::Malformed
        );
    }

    #[test]
    fn mint_stage_operation_checks_canonical_body_before_unavailable() {
        let body = canonical_mint_stage_fixture_bytes_for_tests("command");
        let command = KagemushaDeviceMintStageCommandV1::decode_canonical_shape_exact(&body)
            .expect("mint-stage fixture command");
        let (authorization, _) = command
            .validated_inputs()
            .expect("bound mint-stage fixture");
        let operation_id = authorization.statement.context.operation_id;
        assert_eq!(
            classify_stock_device_command_v1(&frame(16, operation_id, &body)),
            StockDeviceCommandDispositionV1::Unavailable
        );
        assert_eq!(
            classify_stock_device_command_v1(&frame(16, [0xF1; 32], &body)),
            StockDeviceCommandDispositionV1::Malformed
        );
        for invalid in [vec![16], [body.as_slice(), &[0]].concat()] {
            assert_eq!(
                classify_stock_device_command_v1(&frame(16, operation_id, &invalid)),
                StockDeviceCommandDispositionV1::Malformed
            );
        }
    }

    #[test]
    fn frame_lengths_and_unknown_operations_reject_without_integer_narrowing() {
        let maximum = frame(1, [7; 32], &vec![9; MAX_COMMAND_PAYLOAD_BYTES_V1]);
        assert!(decode_command_frame_v1(&maximum).is_some());
        assert_eq!(
            classify_stock_device_command_v1(&maximum),
            StockDeviceCommandDispositionV1::Malformed
        );
        let oversized = frame(1, [7; 32], &vec![9; MAX_COMMAND_PAYLOAD_BYTES_V1 + 1]);
        assert_eq!(
            classify_stock_device_command_v1(&oversized),
            StockDeviceCommandDispositionV1::Malformed
        );
        let payload = control_payload::canonical_command_body_for_tests(1, [7; 32]).unwrap();
        let original = frame(1, [7; 32], &payload);
        for length in 0..original.len() {
            assert_eq!(
                classify_stock_device_command_v1(&original[..length]),
                StockDeviceCommandDispositionV1::Malformed
            );
        }
        for code in [0, 28, u8::MAX] {
            assert_eq!(
                classify_stock_device_command_v1(&frame(code, [7; 32], &[9])),
                StockDeviceCommandDispositionV1::Malformed
            );
        }
        let mut declared_overflow = original;
        declared_overflow[44..48].copy_from_slice(&u32::MAX.to_le_bytes());
        assert_eq!(
            classify_stock_device_command_v1(&declared_overflow),
            StockDeviceCommandDispositionV1::Malformed
        );
    }

    #[test]
    fn jni_lengths_and_c_status_mapping_are_exact() {
        for length in [i32::MIN, -1, 0, 79, i32::MAX] {
            assert_eq!(bounded_jni_command_length_v1(length), None);
        }
        for length in [80, 81, MAX_COMMAND_BYTES_V1 as i32] {
            assert_eq!(bounded_jni_command_length_v1(length), Some(length as usize));
        }
        assert_eq!(
            bounded_jni_command_length_v1(MAX_COMMAND_BYTES_V1 as i32 + 1),
            None
        );
        assert_eq!(
            classify_jni_execution_v1(
                crate::ERR_KAGEMUSHA_DEVICE_UNAVAILABLE_V1,
                0,
                MAX_RESPONSE_BYTES_V1
            ),
            JniExecutionDispositionV1::Unavailable
        );
        assert_eq!(
            classify_jni_execution_v1(crate::ERR_KAGEMUSHA_V1, 0, MAX_RESPONSE_BYTES_V1),
            JniExecutionDispositionV1::Malformed
        );
        for written in [RESPONSE_HEADER_BYTES_V1, MAX_RESPONSE_BYTES_V1] {
            assert_eq!(
                classify_jni_execution_v1(0, written, MAX_RESPONSE_BYTES_V1),
                JniExecutionDispositionV1::Response(written)
            );
        }
        for (status, written, capacity) in [
            (0, 0, MAX_RESPONSE_BYTES_V1),
            (0, RESPONSE_HEADER_BYTES_V1 - 1, MAX_RESPONSE_BYTES_V1),
            (0, MAX_RESPONSE_BYTES_V1 + 1, MAX_RESPONSE_BYTES_V1),
            (0, usize::MAX, MAX_RESPONSE_BYTES_V1),
            (0, RESPONSE_HEADER_BYTES_V1, usize::MAX),
            (
                crate::ERR_KAGEMUSHA_DEVICE_UNAVAILABLE_V1,
                1,
                MAX_RESPONSE_BYTES_V1,
            ),
            (crate::ERR_KAGEMUSHA_V1, 1, MAX_RESPONSE_BYTES_V1),
            (crate::ERR_NULL_PTR, 0, MAX_RESPONSE_BYTES_V1),
        ] {
            assert_eq!(
                classify_jni_execution_v1(status, written, capacity),
                JniExecutionDispositionV1::Failed
            );
        }
    }

    #[test]
    fn c_boundary_rejects_malformed_and_impossible_lengths_without_output() {
        let mut command = frame(1, [7; 32], &[9]);
        command[48] ^= 1;
        let mut output = [0xa5; RESPONSE_HEADER_BYTES_V1];
        for length in [0, COMMAND_HEADER_BYTES_V1 - 1, command.len(), usize::MAX] {
            let mut written = usize::MAX;
            assert_eq!(
                unsafe {
                    crate::connect_norito_kagemusha_device_execute_v1(
                        command.as_ptr(),
                        length,
                        output.as_mut_ptr(),
                        output.len(),
                        &mut written,
                    )
                },
                crate::ERR_KAGEMUSHA_V1
            );
            assert_eq!(written, 0);
            assert_eq!(output, [0xa5; RESPONSE_HEADER_BYTES_V1]);
        }
    }
}
