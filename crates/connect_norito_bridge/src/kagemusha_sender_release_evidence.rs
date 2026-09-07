//! Structural evidence projection of an existing canonical KAGEMUSHA operation-12 command.
//!
//! The ordinary sender decoder verifies canonical framing, embedded authorization signatures,
//! and public bindings. This development tool never authenticates a release catalog, decides a
//! recursive proof, consumes a hardware nonce, or constructs Core's finalized release capability.

use iroha_data_model::kagemusha::{
    KagemushaCommitEvidenceV1, KagemushaPaymentRequestV1, KagemushaPaymentV1,
    KagemushaRedemptionVoucherV1,
};
use norito::json::{Map, Value};
use sha2::{Digest as _, Sha256};

use crate::kagemusha_device_bridge_v1::sender_payload::{
    SENDER_COMMAND_MAX_BYTES_V1, SenderCommandBodyV1, SenderCommandV1,
    SenderHardwareAuthorizationPurposeV1, SenderHardwareAuthorizationV1, SenderPublicInputsV1,
    SenderTerminalReceiptV1, hardware_authorization_key_reference_v1,
};

/// Maximum local canonical command payload accepted by the structural evidence parser.
pub const KAGEMUSHA_SENDER_RELEASE_COMMAND_MAX_BYTES_V1: usize = SENDER_COMMAND_MAX_BYTES_V1;

fn sha256(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn canonical<T: norito::NoritoSerialize>(value: &T) -> Result<Vec<u8>, String> {
    norito::encode_canonical(value)
        .map_err(|_| "cannot encode canonical evidence subject".to_owned())
}

/// Decode an exact operation-12 payload and return its deterministic Norito JSON projection.
///
/// `operation_id` is the independently retained outer request selector. Digests are lowercase
/// hexadecimal. Receipt SHA-256 covers the underlying canonical acknowledgement or redemption
/// receipt, excluding the sender receipt enum wrapper. `context_sha256` covers the canonical
/// sender wallet context; `certificate_sha256` covers the canonical commit-certificate object.
/// Request fields and the payment commit time are null for redemption. The certificate's
/// `commit_evidence_commitment` is a protocol hiding commitment, not a hash of a raw clock or
/// lease observation; this parser cannot authenticate its private opening.
///
/// `structural_only` is always true. In particular an embedded valid signature under a supplied
/// key is not proof that a release authorized that key. A redemption receipt remains a public
/// selector, and this function cannot grant the in-process finalized redemption capability.
///
/// # Errors
///
/// Rejects empty, oversized, noncanonical, wrong-operation, wrong-selector, invalid-signature,
/// or inconsistent command bytes. No candidate executable or signing material is consumed.
pub fn kagemusha_sender_release_command_projection_v1(
    operation_id: [u8; 32],
    bytes: &[u8],
) -> Result<Vec<u8>, String> {
    if bytes.is_empty() || bytes.len() > KAGEMUSHA_SENDER_RELEASE_COMMAND_MAX_BYTES_V1 {
        return Err(
            "sender release command is empty or exceeds its canonical byte bound".to_owned(),
        );
    }
    let command = SenderCommandV1::decode_canonical_exact(12, operation_id, bytes)
        .map_err(|error| format!("invalid canonical sender release command: {error:?}"))?;
    let SenderCommandBodyV1::Release {
        inputs_digest,
        envelope_digest,
        inputs,
        envelope,
        terminal_receipt,
        hardware_authorization,
    } = &command.body
    else {
        return Err("sender evidence requires operation 12 Release".to_owned());
    };
    let authorization =
        SenderHardwareAuthorizationV1::decode_canonical_exact(hardware_authorization)
            .map_err(|error| format!("invalid sender release authorization: {error:?}"))?;
    if authorization.purpose != SenderHardwareAuthorizationPurposeV1::Release {
        return Err("sender evidence requires Release authorization purpose".to_owned());
    }
    // Reuse the exact model envelope decoders to project the immutable certificate and manifest.
    let (
        operation_kind,
        certificate,
        candidate_digest,
        certificate_digest,
        artifact_manifest_digest,
        payment_request,
    ) = match inputs {
        SenderPublicInputsV1::SendSplit { request } => {
            let request_sha256 = sha256(request);
            let request = KagemushaPaymentRequestV1::decode_canonical_exact(request)
                .map_err(|_| "invalid canonical sender request".to_owned())?;
            let payment =
                KagemushaPaymentV1::decode_canonical_shape_exact_against(envelope, &request)
                    .map_err(|_| "invalid canonical sender payment".to_owned())?;
            (
                "send_split",
                payment.commit_certificate,
                payment.proof.candidate_envelope_digest,
                payment.proof.commit_certificate_digest,
                None,
                Some((
                    request_sha256,
                    request.issued_at_ms,
                    request.expires_at_ms,
                    payment.output.committed_at_ms,
                )),
            )
        }
        SenderPublicInputsV1::RedeemSplit { .. } => {
            let voucher = KagemushaRedemptionVoucherV1::decode_canonical_shape_exact(envelope)
                .map_err(|_| "invalid canonical sender redemption".to_owned())?;
            (
                "redeem_split",
                voucher.commit_certificate,
                voucher.proof.candidate_envelope_digest,
                voucher.proof.commit_certificate_digest,
                Some(voucher.artifact_manifest_digest),
                None,
            )
        }
    };
    if authorization.candidate_digest != candidate_digest {
        return Err("release authorization substitutes the installed candidate".to_owned());
    }
    let (receipt_kind, receipt_bytes) = match terminal_receipt {
        SenderTerminalReceiptV1::PaymentAcknowledgement(bytes) => {
            ("payment_acknowledgement", bytes.clone())
        }
        SenderTerminalReceiptV1::RedemptionSettlement(receipt) => {
            ("finalized_redemption_selector", canonical(receipt)?)
        }
    };
    let receipt_digest = authorization
        .terminal_receipt_digest
        .ok_or_else(|| "release authorization lacks terminal receipt".to_owned())?;
    let context = &command.context;
    let (commit_evidence_source, commit_evidence_commitment) = match certificate.commit_evidence {
        KagemushaCommitEvidenceV1::TrustedTime(evidence) => {
            ("trusted_time", evidence.time_evidence_commitment)
        }
        KagemushaCommitEvidenceV1::MonotonicLease(evidence) => {
            ("monotonic_lease", evidence.lease_evidence_commitment)
        }
    };
    let mut report = Map::new();
    for (key, value) in [
        (
            "schema",
            "iroha.kagemusha_v1.sender_release_command_projection",
        ),
        ("operation_kind", operation_kind),
        ("authorization_purpose", "release"),
        ("receipt_kind", receipt_kind),
        ("commit_evidence_source", commit_evidence_source),
    ] {
        report.insert(key.to_owned(), Value::from(value));
    }
    report.insert("schema_version".to_owned(), Value::from(1_u16));
    report.insert("structural_only".to_owned(), Value::Bool(true));
    report.insert("operation".to_owned(), Value::from(12_u8));
    report.insert(
        "protocol_version".to_owned(),
        Value::from(context.release.protocol_version),
    );
    report.insert(
        "policy_epoch".to_owned(),
        Value::from(context.release.policy_epoch),
    );
    report.insert(
        "hardware_epoch_generation".to_owned(),
        Value::from(context.hardware_epoch.generation),
    );
    report.insert("asset_scale".to_owned(), Value::from(context.lane.scale));
    for (key, value) in [
        ("command_sha256", sha256(bytes)),
        ("context_sha256", sha256(&canonical(context)?)),
        ("envelope_sha256", sha256(envelope)),
        ("certificate_sha256", sha256(&canonical(&certificate)?)),
        ("terminal_receipt_sha256", sha256(&receipt_bytes)),
        (
            "hardware_authorization_sha256",
            sha256(hardware_authorization),
        ),
    ] {
        report.insert(key.to_owned(), Value::from(value));
    }
    for (key, value) in [
        ("operation_id", command.operation_id),
        ("inputs_digest", *inputs_digest),
        ("preparation_id", authorization.preparation_id),
        ("candidate_digest", candidate_digest),
        ("release_id", authorization.release_id),
        ("outcome_id", authorization.outcome_id),
        ("transition_nullifier", authorization.transition_nullifier),
        ("envelope_digest", *envelope_digest),
        ("terminal_receipt_digest", receipt_digest),
        ("authorization_id", authorization.authorization_id),
        (
            "authorization_key_reference",
            hardware_authorization_key_reference_v1(&authorization.authorization_public_key),
        ),
        (
            "core_authorization_key_reference",
            context.core_authorization_key_reference,
        ),
        (
            "prepared_one_use_authorization_digest",
            authorization.prepared_one_use_authorization_digest,
        ),
        (
            "outbox_reservation_commitment",
            authorization.outbox_reservation_commitment,
        ),
        (
            "hardware_one_use_nonce",
            authorization.hardware_one_use_nonce,
        ),
        ("commit_certificate_digest", certificate_digest),
        ("commit_evidence_commitment", commit_evidence_commitment),
        ("network_id", *context.lane.network_id.as_bytes()),
        ("lane_commitment", context.lane.device_lane_id),
        (
            "asset_id",
            context
                .lane
                .normalized_asset_id()
                .map_err(|_| "invalid sender asset identity".to_owned())?,
        ),
        (
            "asset_incarnation",
            *context.release.asset_incarnation.as_bytes(),
        ),
        ("suite_id", context.release.suite_id),
        ("vk_digest", context.release.vk_digest),
        ("hardware_profile_id", context.release.hardware_profile_id),
        ("credential_id", context.credential_id),
        ("hardware_epoch_id", context.hardware_epoch.epoch_id),
        (
            "device_key_reference",
            context.device_policy_binding.device_key_reference,
        ),
        (
            "hardware_policy_id",
            context.device_policy_binding.hardware_policy_id,
        ),
    ] {
        report.insert(key.to_owned(), Value::from(hex::encode(value)));
    }
    report.insert(
        "artifact_manifest_digest".to_owned(),
        artifact_manifest_digest
            .map(|digest| Value::from(hex::encode(digest)))
            .unwrap_or(Value::Null),
    );
    report.insert(
        "request_sha256".to_owned(),
        payment_request
            .as_ref()
            .map(|request| Value::from(request.0.clone()))
            .unwrap_or(Value::Null),
    );
    for (key, index) in [
        ("request_start_ms", 0),
        ("request_end_ms", 1),
        ("payment_committed_at_ms", 2),
    ] {
        report.insert(
            key.to_owned(),
            payment_request
                .as_ref()
                .map(|request| Value::from([request.1, request.2, request.3][index]))
                .unwrap_or(Value::Null),
        );
    }
    norito::json::to_vec(&Value::Object(report))
        .map_err(|_| "cannot encode sender projection JSON".to_owned())
}

#[cfg(test)]
#[path = "kagemusha_sender_release_evidence_tests.rs"]
mod tests;
