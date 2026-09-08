//! Provider-bound prepared stream-token operations for independently qualified hardware custody.
//!
//! A prepared subject is derived from the issuer's own body and pinned binding. It is not custody
//! or completion evidence. Receipt and authenticated state verification remain required before
//! any token is released. These values contain no hardware key material or credentials.

use super::{
    custody::{
        SignerCustodyBindingV1, SignerCustodyErrorV1, SignerCustodyTrustV1,
        SignerCustodyUseContextV1, VerifiedSignerCustodyV1, validate_binding,
        verify_signer_custody_use_v1,
    },
    protocol::{
        SignerKeyAlgorithmV1, SignerKeyOperationPurposeV1, SignerOperationActionV1,
        SignerOperationAuditHeadV1, SignerOperationCommitmentV1, SignerOperationCustodyV1,
        SignerOperationIntentV1, SignerOperationReservationV1, SignerOperationSignatureV1,
        SignerPurposeBindingV1, SignerRoleV1, digest_canonical, digest_parts,
    },
    receipt::{
        SignerCompletedOperationV1, SignerOperationProvenanceV1, SignerReceiptErrorV1, shared,
    },
};
use crate::token::{
    STREAM_TOKEN_MAX_WIRE_BYTES_V1, STREAM_TOKEN_SIGNATURE_DOMAIN_V1, StreamTokenBodyV1,
    StreamTokenV1, validate_token_body,
};
use norito::codec::{Decode, Encode};
use std::fmt;

/// Maximum canonical stream-token signing payload, including its signature domain.
pub const SIGNER_STREAM_TOKEN_MAX_PAYLOAD_BYTES_V1: usize = 2_048;
const BINDING_DOMAIN: &[u8] = b"iroha.sorafs.signer.custody-binding.v1";
const PAYLOAD_DOMAIN: &[u8] = b"iroha.sorafs.signer.stream-token.payload.v1";
const OPERATION_DOMAIN: &[u8] = b"iroha.sorafs.signer.stream-token.operation.v1";
const REQUEST_DOMAIN: &[u8] = b"iroha.sorafs.signer.stream-token.request.v1";

/// Prepare the exact canonical stream-token payload using independently configured authority.
///
/// This derives the body and operation identity only. It authenticates neither custody nor
/// completion and cannot authorize capability release. Daemon and broker callers share this one
/// bounded decoder rather than reconstructing the signing domain or accepting alternate frames.
///
/// # Errors
/// Rejects the complete payload ceiling before decoding, wrong or absent domain, noncanonical
/// body framing, malformed leaves, another provider/key generation or a different pinned purpose.
pub fn prepare_stream_token_signing_payload_v1(
    payload: &[u8],
    pinned_binding: &SignerCustodyBindingV1,
) -> Result<(StreamTokenBodyV1, SignerStreamTokenExpectedV1), SignerStreamTokenReceiptErrorV1> {
    if payload.is_empty() || payload.len() > SIGNER_STREAM_TOKEN_MAX_PAYLOAD_BYTES_V1 {
        return Err(SignerStreamTokenReceiptErrorV1::InvalidReceipt);
    }
    let bytes = payload
        .strip_prefix(STREAM_TOKEN_SIGNATURE_DOMAIN_V1)
        .filter(|bytes| !bytes.is_empty())
        .ok_or(SignerStreamTokenReceiptErrorV1::InvalidReceipt)?;
    let allocation = bytes
        .len()
        .checked_mul(8)
        .and_then(|value| value.checked_add(64 * 1024))
        .ok_or(SignerStreamTokenReceiptErrorV1::InvalidReceipt)?;
    let body: StreamTokenBodyV1 = norito::decode_canonical_with_limits(
        bytes,
        norito::DecodeLimits::new(128, bytes.len(), 2048, allocation, 16),
    )
    .map_err(|_| SignerStreamTokenReceiptErrorV1::InvalidReceipt)?;
    let expected = SignerStreamTokenExpectedV1::new(&body, pinned_binding)?;
    if body
        .signing_payload_bytes()
        .map_err(|_| SignerStreamTokenReceiptErrorV1::InvalidReceipt)?
        != payload
    {
        return Err(SignerStreamTokenReceiptErrorV1::InvalidReceipt);
    }
    Ok((body, expected))
}

/// Independently prepared exact operation; neither decoded nor constructed from a receipt.
///
/// The original custody generation is excluded from the operation identity. Renewing custody
/// cannot reserve the same prepared body and binding again under a different operation identity.
#[derive(PartialEq, Eq)]
pub struct SignerStreamTokenExpectedV1 {
    operation_id: [u8; 32],
    binding_digest: [u8; 32],
    signing_payload_digest: [u8; 32],
    signing_payload_size: u64,
    issued_at_unix_ms: u64,
    expires_at_unix_ms: u64,
}

impl SignerStreamTokenExpectedV1 {
    /// Derive one operation from a locally prepared body and independently pinned signer binding.
    ///
    /// # Errors
    /// Rejects invalid body/binding leaves before encoding, another provider or signing purpose,
    /// unrepresentable times, key-generation mismatch, or either canonical payload/frame ceiling.
    pub fn new(
        body: &StreamTokenBodyV1,
        pinned_binding: &SignerCustodyBindingV1,
    ) -> Result<Self, SignerStreamTokenReceiptErrorV1> {
        validate_token_body(body).map_err(|_| SignerStreamTokenReceiptErrorV1::TokenMismatch)?;
        validate_binding(pinned_binding)
            .map_err(|_| SignerStreamTokenReceiptErrorV1::InvalidReceipt)?;
        let SignerPurposeBindingV1::StreamToken { provider_id } = &pinned_binding.purpose else {
            return Err(SignerStreamTokenReceiptErrorV1::WrongPurpose);
        };
        if pinned_binding.role != SignerRoleV1::StreamToken
            || pinned_binding.algorithm != SignerKeyAlgorithmV1::Ed25519
            || *provider_id != body.provider_id
        {
            return Err(SignerStreamTokenReceiptErrorV1::WrongPurpose);
        }
        if u64::from(body.token_pk_version) != pinned_binding.key_revision {
            return Err(SignerStreamTokenReceiptErrorV1::TokenMismatch);
        }
        let issued_at_unix_ms = body
            .issued_at
            .checked_mul(1_000)
            .ok_or(SignerStreamTokenReceiptErrorV1::InvalidTime)?;
        let expires_at_unix_ms = body
            .ttl_epoch
            .checked_mul(1_000)
            .ok_or(SignerStreamTokenReceiptErrorV1::InvalidTime)?;
        let body_len = norito::canonical_frame_len(body)
            .map_err(|_| SignerStreamTokenReceiptErrorV1::InvalidReceipt)?;
        let payload_len = STREAM_TOKEN_SIGNATURE_DOMAIN_V1
            .len()
            .checked_add(body_len)
            .filter(|length| *length <= SIGNER_STREAM_TOKEN_MAX_PAYLOAD_BYTES_V1)
            .ok_or(SignerStreamTokenReceiptErrorV1::InvalidReceipt)?;
        // Every variable leaf has been validated before this bounded clone. Count the actual
        // signed schema with its fixed signature width, rather than guessing framing overhead.
        let token = StreamTokenV1 {
            body: body.clone(),
            signature: vec![0; 64],
        };
        if norito::canonical_frame_len(&token)
            .map_err(|_| SignerStreamTokenReceiptErrorV1::InvalidReceipt)?
            > STREAM_TOKEN_MAX_WIRE_BYTES_V1
        {
            return Err(SignerStreamTokenReceiptErrorV1::InvalidReceipt);
        }
        let payload = body
            .signing_payload_bytes()
            .map_err(|_| SignerStreamTokenReceiptErrorV1::InvalidReceipt)?;
        if payload.len() != payload_len {
            return Err(SignerStreamTokenReceiptErrorV1::InvalidReceipt);
        }
        let binding_digest = stream_token_binding_digest_v1(pinned_binding)?;
        Ok(Self {
            operation_id: digest_parts(OPERATION_DOMAIN, &[&binding_digest, &payload]),
            binding_digest,
            signing_payload_digest: digest_parts(PAYLOAD_DOMAIN, &[&payload]),
            signing_payload_size: u64::try_from(payload_len)
                .map_err(|_| SignerStreamTokenReceiptErrorV1::InvalidReceipt)?,
            issued_at_unix_ms,
            expires_at_unix_ms,
        })
    }

    /// Exact operation identity independently derived before provider I/O.
    #[must_use]
    pub const fn operation_id(&self) -> [u8; 32] {
        self.operation_id
    }

    /// Commitment to the exact independently pinned public signer binding.
    #[must_use]
    pub const fn binding_digest(&self) -> [u8; 32] {
        self.binding_digest
    }

    /// Commitment to the exact domain-prefixed canonical token body.
    #[must_use]
    pub const fn signing_payload_digest(&self) -> [u8; 32] {
        self.signing_payload_digest
    }

    /// Exact signing payload length, including its domain separator.
    #[must_use]
    pub const fn signing_payload_size(&self) -> u64 {
        self.signing_payload_size
    }

    /// Checked issue time derived from the independently prepared canonical body.
    #[must_use]
    pub const fn issued_at_unix_ms(&self) -> u64 {
        self.issued_at_unix_ms
    }

    /// Checked exclusive expiry derived from the independently prepared canonical body.
    #[must_use]
    pub const fn expires_at_unix_ms(&self) -> u64 {
        self.expires_at_unix_ms
    }

    /// Check this body's chronology against an independently supplied current clock.
    ///
    /// This validates only the derived time window, not custody, reservation, completion,
    /// trusted-clock provenance or token release. A caller-provided timestamp grants no authority.
    ///
    /// # Errors
    /// Rejects an issue time after `now_unix_ms`, then an expiry at or before that time.
    pub fn validate_time_at(
        &self,
        now_unix_ms: u64,
    ) -> Result<(), SignerStreamTokenReceiptErrorV1> {
        if self.issued_at_unix_ms > now_unix_ms {
            return Err(SignerStreamTokenReceiptErrorV1::InvalidTime);
        }
        if now_unix_ms >= self.expires_at_unix_ms {
            return Err(SignerStreamTokenReceiptErrorV1::TokenExpired);
        }
        Ok(())
    }
}

impl fmt::Debug for SignerStreamTokenExpectedV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SignerStreamTokenExpectedV1")
            .finish_non_exhaustive()
    }
}

/// Exact prepared request, tied to its independently verified original custody generation.
#[derive(Clone, Copy, PartialEq, Eq, Decode, Encode)]
pub struct SignerStreamTokenRequestV1 {
    /// Exact independently prepared operation identity.
    pub operation_id: [u8; 32],
    /// Exact independently pinned public signer binding commitment.
    pub binding_digest: [u8; 32],
    /// Original approved custody and control generation; never replaced on recovery.
    pub original_custody: SignerOperationCustodyV1,
    /// Commitment to the domain-prefixed canonical body signed by the role key.
    pub signing_payload_digest: [u8; 32],
    /// Exact byte count of the signed payload, including its domain separator.
    pub signing_payload_size: u64,
    /// Claimed issue time; verification must reconstruct it from the exact prepared body.
    pub issued_at_unix_ms: u64,
    /// Claimed exclusive expiry; verification must reconstruct it from the exact prepared body.
    pub expires_at_unix_ms: u64,
}

impl SignerStreamTokenRequestV1 {
    /// Bind the independently prepared subject to already verified active original custody.
    ///
    /// # Errors
    /// Rejects any changed prepared body, provider, network, key, purpose or policy binding.
    pub fn new(
        custody: &VerifiedSignerCustodyV1,
        expected: &SignerStreamTokenExpectedV1,
        body: &StreamTokenBodyV1,
    ) -> Result<Self, SignerStreamTokenReceiptErrorV1> {
        let actual = SignerStreamTokenExpectedV1::new(body, &custody.statement().binding)?;
        if &actual != expected {
            return Err(SignerStreamTokenReceiptErrorV1::TokenMismatch);
        }
        Ok(Self {
            operation_id: expected.operation_id,
            binding_digest: expected.binding_digest,
            original_custody: SignerOperationCustodyV1::from_verified(custody),
            signing_payload_digest: expected.signing_payload_digest,
            signing_payload_size: expected.signing_payload_size,
            issued_at_unix_ms: expected.issued_at_unix_ms,
            expires_at_unix_ms: expected.expires_at_unix_ms,
        })
    }

    /// Exact canonical request commitment used by the reserved operation intent.
    ///
    /// # Errors
    /// Rejects canonical serialization failure.
    pub fn digest(&self) -> Result<[u8; 32], SignerStreamTokenReceiptErrorV1> {
        digest_canonical(REQUEST_DOMAIN, self)
            .map_err(|_| SignerStreamTokenReceiptErrorV1::InvalidReceipt)
    }
}

impl fmt::Debug for SignerStreamTokenRequestV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SignerStreamTokenRequestV1")
            .finish_non_exhaustive()
    }
}

/// Payload-free failures from stream-token operation admission and receipt verification.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SignerStreamTokenReceiptErrorV1 {
    /// Canonical encoding, public binding or resource bounds are invalid.
    InvalidReceipt,
    /// The pinned role, provider or signing purpose does not authorize this body.
    WrongPurpose,
    /// The independently retained body or governed key generation does not match.
    TokenMismatch,
    /// Token or completion chronology cannot establish timely issuance and observation.
    InvalidTime,
    /// Independent original active custody verification failed.
    Custody(SignerCustodyErrorV1),
    /// A token or ordered operation signature/message is invalid.
    InvalidSignature,
    /// The exact independently authenticated immutable completion does not match.
    CompletionMismatch,
    /// The token has reached its exclusive expiry at the independently trusted current time.
    TokenExpired,
}

impl fmt::Display for SignerStreamTokenReceiptErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidReceipt => "invalid stream-token operation receipt",
            Self::WrongPurpose => "stream-token signing purpose mismatch",
            Self::TokenMismatch => "stream-token operation subject mismatch",
            Self::InvalidTime => "invalid stream-token operation time",
            Self::Custody(_) => "stream-token receipt lacks independently verified active custody",
            Self::InvalidSignature => "stream-token receipt signature binding is invalid",
            Self::CompletionMismatch => {
                "stream-token receipt lacks the exact authenticated completion"
            }
            Self::TokenExpired => "stream token expired before receipt qualification",
        })
    }
}

impl std::error::Error for SignerStreamTokenReceiptErrorV1 {}

/// Maximum complete canonical stream-token signing receipt, independent of the token wire bound.
pub const SIGNER_STREAM_TOKEN_RECEIPT_MAX_BYTES_V1: usize = 64 * 1024;
/// Sole first-release stream-token operation receipt marker.
pub const SIGNER_STREAM_TOKEN_RECEIPT_MAGIC_V1: [u8; 8] = *b"IRSTKR01";
const AUDIT_DOMAIN: &[u8] = b"iroha.sorafs.signer.stream-token.audit.v1";
const RESPONSE_DOMAIN: &[u8] = b"iroha.sorafs.signer.stream-token.response.v1";

/// Canonical public claim for exactly one prepared stream-token signing operation.
///
/// The receipt carries no observer trust, verification clock or authoritative completion. It
/// cannot qualify its own custody or authorize release without independent current-state evidence.
#[derive(Clone, PartialEq, Eq, Decode, Encode)]
pub struct SignerStreamTokenReceiptV1 {
    /// Exact [`SIGNER_STREAM_TOKEN_RECEIPT_MAGIC_V1`] marker.
    pub magic: [u8; 8],
    /// Sole first-release schema version, one.
    pub version: u16,
    /// Exact canonical independently attested original custody record.
    pub custody_record: Vec<u8>,
    /// Exact independently prepared body and provider-scoped signer binding.
    pub request: SignerStreamTokenRequestV1,
    /// Exact ordinary signing action and original audit predecessor.
    pub intent: SignerOperationIntentV1,
    /// Immutable original exclusive reservation.
    pub reservation: SignerOperationReservationV1,
    /// Original qualified custody, signing anchor and audit provenance.
    pub provenance: SignerOperationProvenanceV1,
    /// Exact resulting audit and response commitment.
    pub commitment: SignerOperationCommitmentV1,
    /// Exactly four signatures: token payload, audit, provenance and final response.
    pub signatures: Vec<SignerOperationSignatureV1>,
}

impl SignerStreamTokenReceiptV1 {
    /// Decode a bounded canonical receipt claim without conferring custody or token authority.
    ///
    /// This shares the full evidence verifier's finite decoder. Signature, request, original
    /// custody, completion and fresh challenged observer verification remain mandatory.
    ///
    /// # Errors
    /// Rejects noncanonical frames, resource ceilings, invalid markers or custody-record bounds.
    pub fn decode_canonical(bytes: &[u8]) -> Result<Self, SignerStreamTokenReceiptErrorV1> {
        let receipt = decode_receipt(bytes)?;
        receipt.validate_record_bounds()?;
        Ok(receipt)
    }

    /// Encode one bounded canonical public receipt after validating every variable-width leaf.
    ///
    /// Encoding a claim is not custody or completion verification. The producer must obtain its
    /// signatures and committed row through the authoritative operation coordinator.
    ///
    /// # Errors
    /// Rejects markers, custody-record bounds, signature shape or a complete frame over 64 KiB.
    pub fn encode_canonical(&self) -> Result<Vec<u8>, SignerStreamTokenReceiptErrorV1> {
        self.validate_record_bounds()?;
        self.validate_signature_shape()?;
        // All remaining members are fixed width. Counting avoids allocating the full output
        // frame; Norito may still allocate bounded field staging while calculating its size.
        let length = norito::canonical_frame_len(self)
            .map_err(|_| SignerStreamTokenReceiptErrorV1::InvalidReceipt)?;
        if length > SIGNER_STREAM_TOKEN_RECEIPT_MAX_BYTES_V1 {
            return Err(SignerStreamTokenReceiptErrorV1::InvalidReceipt);
        }
        let bytes = norito::encode_canonical(self)
            .map_err(|_| SignerStreamTokenReceiptErrorV1::InvalidReceipt)?;
        if bytes.len() != length {
            return Err(SignerStreamTokenReceiptErrorV1::InvalidReceipt);
        }
        Ok(bytes)
    }

    /// Extract the untrusted role signature claim after exact ordered four-signature shape checks.
    ///
    /// This performs no signature, custody, completion, observer or token-liveness authentication.
    /// The caller must retain its own prepared body and run the complete fresh challenged evidence
    /// verifier before releasing a token; the receipt can never choose those expected inputs.
    ///
    /// # Errors
    /// Rejects malformed markers/record bounds, signature count, purpose order or signature width.
    pub fn role_signature_claim(&self) -> Result<[u8; 64], SignerStreamTokenReceiptErrorV1> {
        self.validate_record_bounds()?;
        self.validate_signature_shape()?;
        self.signatures[0]
            .signature
            .as_slice()
            .try_into()
            .map_err(|_| SignerStreamTokenReceiptErrorV1::InvalidSignature)
    }

    fn validate_signature_shape(&self) -> Result<(), SignerStreamTokenReceiptErrorV1> {
        if self.signatures.len() != 4
            || self
                .signatures
                .iter()
                .zip([
                    SignerKeyOperationPurposeV1::RolePayload,
                    SignerKeyOperationPurposeV1::AuditRecord,
                    SignerKeyOperationPurposeV1::Provenance,
                    SignerKeyOperationPurposeV1::Response,
                ])
                .any(|(signature, purpose)| {
                    signature.purpose != purpose || signature.signature.len() != 64
                })
        {
            return Err(SignerStreamTokenReceiptErrorV1::InvalidSignature);
        }
        Ok(())
    }

    fn validate_record_bounds(&self) -> Result<(), SignerStreamTokenReceiptErrorV1> {
        if self.magic != SIGNER_STREAM_TOKEN_RECEIPT_MAGIC_V1
            || self.version != 1
            || self.custody_record.is_empty()
            || self.custody_record.len() > super::custody::SIGNER_CUSTODY_MAX_BYTES_V1
        {
            return Err(SignerStreamTokenReceiptErrorV1::InvalidReceipt);
        }
        Ok(())
    }

    fn operation_view(&self) -> shared::SignerOperationReceiptViewV1<'_> {
        shared::SignerOperationReceiptViewV1 {
            operation_id: self.request.operation_id,
            original_custody: self.request.original_custody,
            reservation: self.reservation,
            provenance: &self.provenance,
            commitment: &self.commitment,
        }
    }
}

impl fmt::Debug for SignerStreamTokenReceiptV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SignerStreamTokenReceiptV1")
            .field("version", &self.version)
            .finish_non_exhaustive()
    }
}

/// Privately constructed complete receipt result, unavailable from public raw-state claims.
///
/// The public runtime release path must authenticate fresh challenged observer evidence before
/// calling the internal receipt verifier. Neither decoding nor a raw context can mint this result.
pub struct VerifiedStreamTokenSignerReceiptV1 {
    custody: VerifiedSignerCustodyV1,
    completion: SignerCompletedOperationV1,
    signing_payload_digest: [u8; 32],
    observed_at_unix_ms: u64,
}

impl VerifiedStreamTokenSignerReceiptV1 {
    /// Authenticated observation time retained for the caller's monotonic release floor.
    #[must_use]
    pub const fn observed_at_unix_ms(&self) -> u64 {
        self.observed_at_unix_ms
    }

    /// Independently verified active custody for the exact original operation.
    #[must_use]
    pub fn custody(&self) -> &VerifiedSignerCustodyV1 {
        &self.custody
    }

    /// Exact independently authenticated immutable completed operation.
    #[must_use]
    pub const fn completion(&self) -> &SignerCompletedOperationV1 {
        &self.completion
    }

    /// Commitment to the exact domain-prefixed canonical token body.
    #[must_use]
    pub const fn signing_payload_digest(&self) -> [u8; 32] {
        self.signing_payload_digest
    }
}

impl fmt::Debug for VerifiedStreamTokenSignerReceiptV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifiedStreamTokenSignerReceiptV1")
            .finish_non_exhaustive()
    }
}

/// Derive the exact next audit for one reserved token payload and its role signature.
///
/// # Errors
/// Rejects a different action/request/id, invalid predecessor or non-Ed25519 signature width.
pub fn signer_stream_token_audit_v1(
    request: &SignerStreamTokenRequestV1,
    intent: &SignerOperationIntentV1,
    reservation: SignerOperationReservationV1,
    token_signature: &[u8],
) -> Result<SignerOperationAuditHeadV1, SignerStreamTokenReceiptErrorV1> {
    if intent.action != SignerOperationActionV1::Sign
        || intent.operation_id != request.operation_id
        || intent.request_digest != request.digest()?
        || token_signature.len() != 64
    {
        return Err(SignerStreamTokenReceiptErrorV1::InvalidReceipt);
    }
    let intent_digest = intent
        .digest()
        .map_err(|_| SignerStreamTokenReceiptErrorV1::InvalidReceipt)?;
    let sequence = intent
        .previous_audit
        .sequence
        .checked_add(1)
        .ok_or(SignerStreamTokenReceiptErrorV1::InvalidReceipt)?;
    let digest = digest_canonical(
        AUDIT_DOMAIN,
        &(
            *request,
            intent_digest,
            reservation,
            digest_parts(
                b"iroha.sorafs.signer.operation.signature.v1",
                &[token_signature],
            ),
        ),
    )
    .map_err(|_| SignerStreamTokenReceiptErrorV1::InvalidReceipt)?;
    Ok(SignerOperationAuditHeadV1 { sequence, digest })
}

/// Derive the purpose-bound final response after the first three ordered signatures exist.
///
/// # Errors
/// Rejects signature count/order/size or canonical serialization failure.
pub fn signer_stream_token_response_digest_v1(
    request: &SignerStreamTokenRequestV1,
    provenance: &SignerOperationProvenanceV1,
    first_signatures: &[SignerOperationSignatureV1],
) -> Result<[u8; 32], SignerStreamTokenReceiptErrorV1> {
    let signatures_digest = shared::first_three_signatures_digest(first_signatures)
        .map_err(map_common_receipt_error)?;
    digest_canonical(RESPONSE_DOMAIN, &(*request, *provenance, signatures_digest))
        .map_err(|_| SignerStreamTokenReceiptErrorV1::InvalidReceipt)
}

/// Check exact token bytes, original custody/provenance and every ordered signature.
///
/// This signature-only helper does not establish durable completion, current observer authority
/// or token liveness, and never authorizes release. Runtime recovery must authenticate the exact
/// immutable completed row and fresh challenged current-state evidence before releasing output.
///
/// # Errors
/// Rejects substituted custody, request/provider/body, malformed signature messages or signatures.
pub fn validate_stream_token_signatures_v1(
    receipt: &SignerStreamTokenReceiptV1,
    token: &StreamTokenV1,
    expected: &SignerStreamTokenExpectedV1,
    custody: &VerifiedSignerCustodyV1,
) -> Result<[u8; 32], SignerStreamTokenReceiptErrorV1> {
    receipt.validate_record_bounds()?;
    if token.signature.len() != 64
        || digest_parts(
            super::custody::CUSTODY_RECORD_DIGEST_DOMAIN_V1,
            &[&receipt.custody_record],
        ) != custody.record_digest()
    {
        return Err(SignerStreamTokenReceiptErrorV1::InvalidReceipt);
    }
    let request = SignerStreamTokenRequestV1::new(custody, expected, &token.body)?;
    if receipt.request != request {
        return Err(SignerStreamTokenReceiptErrorV1::TokenMismatch);
    }
    let intent_digest = receipt
        .intent
        .digest()
        .map_err(|_| SignerStreamTokenReceiptErrorV1::InvalidReceipt)?;
    let audit = signer_stream_token_audit_v1(
        &request,
        &receipt.intent,
        receipt.reservation,
        &token.signature,
    )?;
    let operation = receipt.operation_view();
    operation
        .validate_provenance(intent_digest, audit, custody)
        .map_err(map_common_receipt_error)?;
    let signatures = shared::exact_four_signatures(&receipt.signatures, &token.signature)
        .map_err(map_common_receipt_error)?;
    if signer_stream_token_response_digest_v1(&request, &receipt.provenance, &signatures[..3])?
        != receipt.commitment.response_digest
    {
        return Err(SignerStreamTokenReceiptErrorV1::InvalidSignature);
    }
    // Enforce the actual token's strict Ed25519 key/R/S checks in addition to the shared operation
    // signature binding. The body was fully bounded before any signing-message allocation.
    verify_stream_token_role_signature_v1(token, &custody.statement().binding)?;
    let payload = token
        .body
        .signing_payload_bytes()
        .map_err(|_| SignerStreamTokenReceiptErrorV1::InvalidReceipt)?;
    operation
        .validate_signatures(signatures, &payload, audit, custody)
        .map_err(map_common_receipt_error)
}

/// Internal complete receipt verifier for the authenticated stream-token evidence owner.
///
/// The sibling evidence owner must authenticate fresh challenged current/completed observations
/// against independent configured trust before calling. This is intentionally not a public raw-
/// context-to-qualification entry point. The exact prepared subject and binding are independently
/// retained by the caller; no candidate receipt field becomes an expected value.
pub(super) fn verify_stream_token_signer_receipt_v1(
    receipt_bytes: &[u8],
    token: &StreamTokenV1,
    expected: &SignerStreamTokenExpectedV1,
    expected_binding: &SignerCustodyBindingV1,
    trust: &SignerCustodyTrustV1,
    current: &SignerCustodyUseContextV1,
    completion: &SignerCompletedOperationV1,
) -> Result<VerifiedStreamTokenSignerReceiptV1, SignerStreamTokenReceiptErrorV1> {
    if receipt_bytes.is_empty()
        || receipt_bytes.len() > SIGNER_STREAM_TOKEN_RECEIPT_MAX_BYTES_V1
        || token.signature.len() != 64
    {
        return Err(SignerStreamTokenReceiptErrorV1::InvalidReceipt);
    }
    // Validate leaves and both payload/frame ceilings before any clone or encoding of the body.
    let actual = SignerStreamTokenExpectedV1::new(&token.body, expected_binding)?;
    if &actual != expected {
        return Err(SignerStreamTokenReceiptErrorV1::TokenMismatch);
    }
    let receipt = decode_receipt(receipt_bytes)?;
    receipt.validate_record_bounds()?;
    let custody =
        verify_signer_custody_use_v1(&receipt.custody_record, expected_binding, trust, current)
            .map_err(SignerStreamTokenReceiptErrorV1::Custody)?;
    let signatures_digest =
        validate_stream_token_signatures_v1(&receipt, token, expected, &custody)?;
    let intent_digest = receipt
        .intent
        .digest()
        .map_err(|_| SignerStreamTokenReceiptErrorV1::InvalidReceipt)?;
    receipt
        .operation_view()
        .validate_completion(
            completion,
            intent_digest,
            signatures_digest,
            &custody,
            current,
        )
        .map_err(map_common_receipt_error)?;
    validate_token_completion_time(expected, &custody, current, completion)?;
    Ok(VerifiedStreamTokenSignerReceiptV1 {
        custody,
        completion: *completion,
        signing_payload_digest: expected.signing_payload_digest,
        observed_at_unix_ms: current.anchor_observed_at_unix_ms,
    })
}

fn decode_receipt(
    bytes: &[u8],
) -> Result<SignerStreamTokenReceiptV1, SignerStreamTokenReceiptErrorV1> {
    if bytes.is_empty() || bytes.len() > SIGNER_STREAM_TOKEN_RECEIPT_MAX_BYTES_V1 {
        return Err(SignerStreamTokenReceiptErrorV1::InvalidReceipt);
    }
    let allocation = bytes
        .len()
        .checked_mul(8)
        .and_then(|value| value.checked_add(64 * 1024))
        .map(|value| value.min(512 * 1024))
        .ok_or(SignerStreamTokenReceiptErrorV1::InvalidReceipt)?;
    norito::decode_canonical_with_limits(
        bytes,
        norito::DecodeLimits::new(16 * 1024, bytes.len(), 8192, allocation, 24),
    )
    .map_err(|_| SignerStreamTokenReceiptErrorV1::InvalidReceipt)
}

fn validate_token_completion_time(
    expected: &SignerStreamTokenExpectedV1,
    custody: &VerifiedSignerCustodyV1,
    current: &SignerCustodyUseContextV1,
    completion: &SignerCompletedOperationV1,
) -> Result<(), SignerStreamTokenReceiptErrorV1> {
    if expected.issued_at_unix_ms() > completion.completed_at_unix_ms
        || completion.completed_at_unix_ms > current.anchor_observed_at_unix_ms
        || custody.statement().issued_at_unix_ms > current.anchor_observed_at_unix_ms
        || current.anchor_observed_at_unix_ms > current.now_unix_ms
    {
        return Err(SignerStreamTokenReceiptErrorV1::InvalidTime);
    }
    expected.validate_time_at(current.now_unix_ms)
}

fn map_common_receipt_error(error: SignerReceiptErrorV1) -> SignerStreamTokenReceiptErrorV1 {
    match error {
        SignerReceiptErrorV1::InvalidReceipt => SignerStreamTokenReceiptErrorV1::InvalidReceipt,
        SignerReceiptErrorV1::WrongPurpose => SignerStreamTokenReceiptErrorV1::WrongPurpose,
        SignerReceiptErrorV1::ManifestMismatch => SignerStreamTokenReceiptErrorV1::TokenMismatch,
        SignerReceiptErrorV1::Custody(error) => SignerStreamTokenReceiptErrorV1::Custody(error),
        SignerReceiptErrorV1::InvalidSignature => SignerStreamTokenReceiptErrorV1::InvalidSignature,
        SignerReceiptErrorV1::CompletionMismatch => {
            SignerStreamTokenReceiptErrorV1::CompletionMismatch
        }
    }
}

/// Validate and digest the exact provider-scoped public binding without qualifying custody.
///
/// This checks only independently configured bounded public authority and returns a digest, never
/// a verified marker. Attestation, current ACTIVE state, completion and fresh challenged observer
/// evidence remain mandatory before capability release.
///
/// # Errors
/// Rejects malformed binding leaves, a different role/purpose, zero provider, non-Ed25519 key,
/// unrepresentable governed token key revision, or canonical serialization failure.
pub fn stream_token_binding_digest_v1(
    binding: &SignerCustodyBindingV1,
) -> Result<[u8; 32], SignerStreamTokenReceiptErrorV1> {
    validate_binding(binding).map_err(|_| SignerStreamTokenReceiptErrorV1::InvalidReceipt)?;
    if binding.role != SignerRoleV1::StreamToken
        || binding.algorithm != SignerKeyAlgorithmV1::Ed25519
        || !matches!(binding.purpose, SignerPurposeBindingV1::StreamToken { provider_id } if provider_id != [0; 32])
    {
        return Err(SignerStreamTokenReceiptErrorV1::WrongPurpose);
    }
    if binding.key_revision > u64::from(u32::MAX) {
        return Err(SignerStreamTokenReceiptErrorV1::TokenMismatch);
    }
    digest_canonical(BINDING_DOMAIN, binding)
        .map_err(|_| SignerStreamTokenReceiptErrorV1::InvalidReceipt)
}

/// Admit only the bounded exact prepared receipt before requesting a fresh observer challenge.
/// This creates no custody/current/completion marker and cannot authorize release.
pub(super) fn prevalidate_stream_token_receipt_v1(
    bytes: &[u8],
    token: &StreamTokenV1,
    expected: &SignerStreamTokenExpectedV1,
    binding: &SignerCustodyBindingV1,
) -> Result<(SignerStreamTokenReceiptV1, [u8; 32]), SignerStreamTokenReceiptErrorV1> {
    let actual = SignerStreamTokenExpectedV1::new(&token.body, binding)?;
    if &actual != expected {
        return Err(SignerStreamTokenReceiptErrorV1::TokenMismatch);
    }
    let receipt = decode_receipt(bytes)?;
    // This bounded producer check admits all four exact purposes/widths and the original record.
    // The canonical decoder already guarantees byte identity; no alternate layout is normalized.
    receipt.encode_canonical()?;
    let request = &receipt.request;
    if request.operation_id != expected.operation_id()
        || request.binding_digest != expected.binding_digest()
        || request.signing_payload_digest != expected.signing_payload_digest()
        || request.signing_payload_size != expected.signing_payload_size()
        || request.issued_at_unix_ms != expected.issued_at_unix_ms()
        || request.expires_at_unix_ms != expected.expires_at_unix_ms()
        || request.original_custody.record_digest == [0; 32]
        || request.original_custody.control_state_digest == [0; 32]
        || receipt.intent.action != SignerOperationActionV1::Sign
        || receipt.intent.operation_id != request.operation_id
        || receipt.intent.request_digest != request.digest()?
    {
        return Err(SignerStreamTokenReceiptErrorV1::TokenMismatch);
    }
    shared::exact_four_signatures(&receipt.signatures, &token.signature)
        .map_err(map_common_receipt_error)?;
    verify_stream_token_role_signature_v1(token, binding)?;
    let signatures_digest =
        super::protocol::signer_operation_signatures_digest_v1(&receipt.signatures)
            .map_err(|_| SignerStreamTokenReceiptErrorV1::InvalidSignature)?;
    Ok((receipt, signatures_digest))
}

fn verify_stream_token_role_signature_v1(
    token: &StreamTokenV1,
    binding: &SignerCustodyBindingV1,
) -> Result<(), SignerStreamTokenReceiptErrorV1> {
    let (algorithm, public_key) = binding
        .public_key
        .try_to_bytes()
        .map_err(|_| SignerStreamTokenReceiptErrorV1::InvalidSignature)?;
    if algorithm != iroha_crypto::Algorithm::Ed25519 {
        return Err(SignerStreamTokenReceiptErrorV1::WrongPurpose);
    }
    let public_key: [u8; 32] = public_key
        .try_into()
        .map_err(|_| SignerStreamTokenReceiptErrorV1::InvalidSignature)?;
    let verifier = ed25519_dalek::VerifyingKey::from_bytes(&public_key)
        .map_err(|_| SignerStreamTokenReceiptErrorV1::InvalidSignature)?;
    token
        .verify(&verifier)
        .map_err(|_| SignerStreamTokenReceiptErrorV1::InvalidSignature)?;
    Ok(())
}

#[cfg(test)]
mod receipt_tests;
#[cfg(test)]
mod subject_tests;

#[cfg(test)]
#[path = "stream_token/payload_preparation_tests.rs"]
mod payload_preparation_tests;
