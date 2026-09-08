//! Shared command-bound device response verification, with no freshness or wallet authority.
//!
//! The signature transcript and operation-1 codecs are the same first-release contract used
//! by the native bridge. Callers separately authenticate catalog membership, expected wallet
//! context, trusted time and their own outstanding challenge. Enrollment challenges never
//! enter the native transient-observation owner.

use super::{
    KagemushaDevicePublicKeyV1, KagemushaDeviceSignatureV1, KagemushaHardwareCredentialV1,
    KagemushaHardwareProfileV1,
};

use crate::{DeriveJsonDeserialize, DeriveJsonSerialize};
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
use sha2::{Digest as _, Sha256};

/// Exact fixed success-response header length.
pub const KAGEMUSHA_DEVICE_RESPONSE_HEADER_BYTES_V1: usize = 116;
/// Maximum body in either direction for this first release.
pub const KAGEMUSHA_DEVICE_PAYLOAD_MAX_BYTES_V1: usize = 64 * 1024;
/// Exact raw low-S P-256 signature length.
pub const KAGEMUSHA_DEVICE_RESPONSE_SIGNATURE_BYTES_V1: usize = 64;
/// Maximum complete success-response frame.
pub const KAGEMUSHA_DEVICE_RESPONSE_MAX_BYTES_V1: usize = KAGEMUSHA_DEVICE_RESPONSE_HEADER_BYTES_V1
    + KAGEMUSHA_DEVICE_PAYLOAD_MAX_BYTES_V1
    + KAGEMUSHA_DEVICE_RESPONSE_SIGNATURE_BYTES_V1;
const RESPONSE_MAGIC: &[u8; 8] = b"IKGMJRS1";
const RESPONSE_DOMAIN: &[u8] = b"iroha:kagemusha:device:v1:response-authenticator";

/// Exact shape/signature failures; none implies wallet or current-challenge admission.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KagemushaDeviceResponseErrorV1 {
    /// Invalid frame bounds, digest, version, operation, status or correlation.
    Frame,
    /// Empty/oversized command or invalid expected policy/report.
    Binding,
    /// Invalid low-S device signature over the complete command-bound transcript.
    Signature,
    /// Invalid bounded canonical operation-1 body or governed credential shape.
    Qualification,
}
impl core::fmt::Display for KagemushaDeviceResponseErrorV1 {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "invalid device response: {self:?}")
    }
}
impl std::error::Error for KagemushaDeviceResponseErrorV1 {}
type Result<T> = core::result::Result<T, KagemushaDeviceResponseErrorV1>;

/// Borrowed shape-checked response. Decoding grants no signature/freshness authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KagemushaDeviceSuccessResponseV1<'a> {
    /// Exact correlated operation.
    pub operation: u8,
    /// Exact correlated request identity.
    pub request_id: [u8; 32],
    /// Canonical operation reply bytes whose digest matched the frame.
    pub payload: &'a [u8],
    /// Raw response authenticator whose digest matched the frame.
    pub authenticator: &'a [u8],
}

/// Parse the sole success frame, enforcing both body/authenticator digests and exact length.
///
/// # Errors
/// Rejects any malformed frame, non-success status, operation or nonce substitution.
pub fn kagemusha_decode_device_success_response_v1(
    bytes: &[u8],
    expected_operation: u8,
    expected_request_id: [u8; 32],
) -> Result<KagemushaDeviceSuccessResponseV1<'_>> {
    use KagemushaDeviceResponseErrorV1::Frame;
    if !(1..=22).contains(&expected_operation)
        || expected_request_id == [0; 32]
        || !(KAGEMUSHA_DEVICE_RESPONSE_HEADER_BYTES_V1..=KAGEMUSHA_DEVICE_RESPONSE_MAX_BYTES_V1)
            .contains(&bytes.len())
        || &bytes[..8] != RESPONSE_MAGIC
        || bytes[8..10] != 1_u16.to_le_bytes()
        || bytes[10] != expected_operation
        || bytes[11] != 0
        || bytes[12..44] != expected_request_id
    {
        return Err(Frame);
    }
    let payload_len = u32::from_le_bytes(bytes[44..48].try_into().map_err(|_| Frame)?) as usize;
    let signature_len = u32::from_le_bytes(bytes[48..52].try_into().map_err(|_| Frame)?) as usize;
    if payload_len == 0
        || payload_len > KAGEMUSHA_DEVICE_PAYLOAD_MAX_BYTES_V1
        || signature_len != KAGEMUSHA_DEVICE_RESPONSE_SIGNATURE_BYTES_V1
        || KAGEMUSHA_DEVICE_RESPONSE_HEADER_BYTES_V1 + payload_len + signature_len != bytes.len()
    {
        return Err(Frame);
    }
    let payload = &bytes[KAGEMUSHA_DEVICE_RESPONSE_HEADER_BYTES_V1
        ..KAGEMUSHA_DEVICE_RESPONSE_HEADER_BYTES_V1 + payload_len];
    let authenticator = &bytes[KAGEMUSHA_DEVICE_RESPONSE_HEADER_BYTES_V1 + payload_len..];
    if bytes[52..84] != Sha256::digest(payload)[..]
        || bytes[84..116] != Sha256::digest(authenticator)[..]
    {
        return Err(Frame);
    }
    Ok(KagemushaDeviceSuccessResponseV1 {
        operation: expected_operation,
        request_id: expected_request_id,
        payload,
        authenticator,
    })
}

/// Build the sole first-release signature transcript from exact command/reply bytes.
///
/// The signature digest is excluded because it hashes the signature itself. This function
/// validates framing inputs but does not validate a command's operation-specific schema.
///
/// # Errors
/// Rejects unknown operations, reserved identities/policies and empty/oversized bodies.
pub fn kagemusha_device_response_signing_bytes_v1(
    operation: u8,
    request_id: [u8; 32],
    canonical_command: &[u8],
    canonical_reply: &[u8],
    hardware_policy_id: [u8; 32],
    qualification_report_digest: [u8; 32],
) -> Result<Vec<u8>> {
    if !(1..=22).contains(&operation)
        || request_id == [0; 32]
        || canonical_command.is_empty()
        || canonical_command.len() > KAGEMUSHA_DEVICE_PAYLOAD_MAX_BYTES_V1
        || canonical_reply.is_empty()
        || canonical_reply.len() > KAGEMUSHA_DEVICE_PAYLOAD_MAX_BYTES_V1
        || hardware_policy_id == [0; 32]
        || qualification_report_digest == [0; 32]
        || hardware_policy_id == qualification_report_digest
    {
        return Err(KagemushaDeviceResponseErrorV1::Binding);
    }
    let mut transcript = Vec::with_capacity(RESPONSE_DOMAIN.len() + 1 + 84 + 32 + 64);
    transcript.extend_from_slice(RESPONSE_DOMAIN);
    transcript.push(0);
    transcript.extend_from_slice(RESPONSE_MAGIC);
    transcript.extend_from_slice(&1_u16.to_le_bytes());
    transcript.push(operation);
    transcript.push(0);
    transcript.extend_from_slice(&request_id);
    transcript.extend_from_slice(&(canonical_reply.len() as u32).to_le_bytes());
    transcript
        .extend_from_slice(&(KAGEMUSHA_DEVICE_RESPONSE_SIGNATURE_BYTES_V1 as u32).to_le_bytes());
    transcript.extend_from_slice(&Sha256::digest(canonical_reply));
    transcript.extend_from_slice(&Sha256::digest(canonical_command));
    transcript.extend_from_slice(&hardware_policy_id);
    transcript.extend_from_slice(&qualification_report_digest);
    Ok(transcript)
}

/// Verify one complete frame under an independently selected device public key.
///
/// # Errors
/// Rejects malformed/correlation-substituted frames or invalid command-bound signatures.
pub fn kagemusha_verify_device_response_v1<'a>(
    bytes: &'a [u8],
    canonical_command: &[u8],
    expected_operation: u8,
    expected_request_id: [u8; 32],
    hardware_policy_id: [u8; 32],
    qualification_report_digest: [u8; 32],
    device_public_key: &KagemushaDevicePublicKeyV1,
) -> Result<KagemushaDeviceSuccessResponseV1<'a>> {
    let response = kagemusha_decode_device_success_response_v1(
        bytes,
        expected_operation,
        expected_request_id,
    )?;
    let transcript = kagemusha_device_response_signing_bytes_v1(
        expected_operation,
        expected_request_id,
        canonical_command,
        response.payload,
        hardware_policy_id,
        qualification_report_digest,
    )?;
    KagemushaDeviceSignatureV1::from_raw_bytes(response.authenticator)
        .and_then(|signature| signature.verify(device_public_key, &transcript))
        .map_err(|_| KagemushaDeviceResponseErrorV1::Signature)?;
    Ok(response)
}

/// Existing exact operation-1 command; it has no account or enrollment fields.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(schema_name = "iroha.kagemusha.device.v1.read-active-hardware-credential-command")]
#[norito(deny_unknown_fields)]
pub struct KagemushaDeviceReadCredentialCommandV1 {
    /// Sole command body version, 1.
    pub version: u16,
    /// Exact operation, 1.
    pub operation: u8,
}
impl KagemushaDeviceReadCredentialCommandV1 {
    /// Encode the existing first-release operation-1 command.
    ///
    /// # Errors
    /// Returns an error only when canonical encoding fails.
    pub fn canonical_bytes() -> Result<Vec<u8>> {
        norito::encode_canonical(&Self {
            version: 1,
            operation: 1,
        })
        .map_err(|_| KagemushaDeviceResponseErrorV1::Qualification)
    }
}

/// Existing operation-1 body; signature/catalog/freshness authority are separate checks.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(schema_name = "iroha.kagemusha.device.v1.active-hardware-credential-reply")]
#[norito(deny_unknown_fields)]
pub struct KagemushaDeviceQualificationReplyV1 {
    /// Sole reply version, 1.
    pub version: u16,
    /// Exact operation, 1.
    pub operation: u8,
    /// Selected release, requiring authenticated catalog lookup.
    pub release_id: [u8; 32],
    /// Exact hardware catalog policy.
    pub hardware_policy_digest: [u8; 32],
    /// Selected native Core authorization key reference.
    pub core_authorization_key_reference: [u8; 32],
    /// Governed profile; embedded bytes alone do not authorize it.
    pub profile: KagemushaHardwareProfileV1,
    /// Issuer-signed hardware credential.
    pub credential: KagemushaHardwareCredentialV1,
}
impl KagemushaDeviceQualificationReplyV1 {
    /// Decode a bounded canonical body and validate its self-contained credential chain.
    ///
    /// # Errors
    /// Rejects malformed/noncanonical/oversized bytes, reserved fields and invalid chains.
    pub fn decode_canonical_exact(bytes: &[u8]) -> Result<Self> {
        if bytes.is_empty() || bytes.len() > 2 * 1024 {
            return Err(KagemushaDeviceResponseErrorV1::Qualification);
        }
        let reply: Self = norito::decode_canonical_with_limits(
            bytes,
            norito::canonical_decode_limits(bytes.len()),
        )
        .map_err(|_| KagemushaDeviceResponseErrorV1::Qualification)?;
        if reply.version != 1
            || reply.operation != 1
            || reply.release_id == [0; 32]
            || reply.hardware_policy_digest == [0; 32]
            || reply.core_authorization_key_reference == [0; 32]
            || reply
                .credential
                .validate_against_profile(&reply.profile)
                .is_err()
        {
            return Err(KagemushaDeviceResponseErrorV1::Qualification);
        }
        Ok(reply)
    }
}
