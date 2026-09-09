//! Fresh challenged observer evidence for provider-bound hardware stream-token operations.
//!
//! Independently configured trust and one caller-owned pending attempt are mandatory. Signed
//! observer claims authenticate accountable observations; they are not consensus proofs or
//! hardware attestations. The runtime owns unpredictable challenges, one-use attempt state,
//! monotonic clock/finality history and actual authoritative reads after each requested phase.

use super::{
    custody::{
        SIGNER_CUSTODY_MAX_BYTES_V1, SignerCustodyActiveHeadV1, SignerCustodyAnchorV1,
        SignerCustodyAuthorityV1, SignerCustodyBindingV1, SignerCustodyTrustV1,
        SignerCustodyUseContextV1, VerifiedSignerCustodyV1, verify_signer_custody_use_v1,
    },
    protocol::{digest_parts, valid_identity},
    receipt::SignerCompletedOperationV1,
    state_observation::{
        SIGNER_STATE_OBSERVATION_MAX_AGE_MS_V1, SignerStateObservationViewV1,
        SignerStateObserverTrustV1,
    },
    stream_token::{
        SIGNER_STREAM_TOKEN_MAX_PAYLOAD_BYTES_V1, SignerStreamTokenExpectedV1,
        SignerStreamTokenReceiptErrorV1, VerifiedStreamTokenSignerReceiptV1,
        prevalidate_stream_token_receipt_v1, stream_token_binding_digest_v1,
        verify_stream_token_signer_receipt_v1,
    },
};
use crate::token::StreamTokenV1;
use norito::codec::{Decode, Encode};
use std::fmt;

/// Maximum canonical signed observation, separate from token and receipt ceilings.
pub const SIGNER_STREAM_TOKEN_EVIDENCE_MAX_BYTES_V1: usize = 64 * 1024;
/// Maximum complete canonical observation request.
pub const SIGNER_STREAM_TOKEN_OBSERVATION_REQUEST_MAX_BYTES_V1: usize = 8 * 1024;
const STATE_MAGIC: [u8; 8] = *b"IRSTKS01";
const REQUEST_MAGIC: [u8; 8] = *b"IRSTKQ01";
const STATE_DOMAIN: &[u8] = b"iroha.sorafs.stream-token.finalized-state.v1\0";
const REQUEST_DOMAIN: &[u8] = b"iroha.sorafs.stream-token.observation-request.v1";
const RECEIPT_DOMAIN: &[u8] = b"iroha.sorafs.signer.stream-token.receipt.v1";

include!("stream_token_evidence/wire.rs");
include!("stream_token_evidence/request.rs");

/// Payload-free evidence admission failures.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SignerStreamTokenEvidenceErrorV1 {
    /// A bounded canonical document is malformed or exceeds its resource budget.
    InvalidDocument,
    /// A candidate differs from the independently retained phase, query, body or binding.
    SourceMismatch,
    /// Independently pinned observer trust is invalid or shares an authority/key slot.
    InvalidTrust,
    /// A state signature, identity, current status, time or finalized floor is invalid.
    InvalidState,
    /// Authenticated state does not qualify the exact original custody and receipt.
    Receipt(SignerStreamTokenReceiptErrorV1),
}
impl fmt::Display for SignerStreamTokenEvidenceErrorV1 {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(match self {
            Self::InvalidDocument => "invalid canonical stream-token evidence document",
            Self::SourceMismatch => {
                "stream-token evidence differs from the retained observation attempt"
            }
            Self::InvalidTrust => "stream-token evidence lacks independent observer trust",
            Self::InvalidState => {
                "stream-token evidence lacks authenticated current finalized state"
            }
            Self::Receipt(_) => "stream-token evidence does not qualify the exact signing receipt",
        })
    }
}
impl std::error::Error for SignerStreamTokenEvidenceErrorV1 {}

/// Authenticated current-only phase; this result cannot authorize token release.
pub struct VerifiedStreamTokenSignerQualificationV1 {
    custody: VerifiedSignerCustodyV1,
    phase: SignerStreamTokenObservationPhaseV1,
    observed_at_unix_ms: u64,
}
impl VerifiedStreamTokenSignerQualificationV1 {
    /// Independently authenticated custody for this exact current-only phase.
    #[must_use]
    pub const fn custody(&self) -> &VerifiedSignerCustodyV1 {
        &self.custody
    }
    /// Exact phase chosen by the retained caller attempt.
    #[must_use]
    pub const fn phase(&self) -> SignerStreamTokenObservationPhaseV1 {
        self.phase
    }
    /// Authenticated time to retain in the caller's monotonic state.
    #[must_use]
    pub const fn observed_at_unix_ms(&self) -> u64 {
        self.observed_at_unix_ms
    }
}
impl fmt::Debug for VerifiedStreamTokenSignerQualificationV1 {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.debug_struct("VerifiedStreamTokenSignerQualificationV1")
            .finish_non_exhaustive()
    }
}

/// Authenticated AfterCommit fence, with no conversion into final token-release authority.
pub struct VerifiedStreamTokenSignerCompletedObservationV1 {
    receipt: VerifiedStreamTokenSignerReceiptV1,
}
impl VerifiedStreamTokenSignerCompletedObservationV1 {
    /// Original independently authenticated custody at the AfterCommit fence.
    #[must_use]
    pub fn custody(&self) -> &VerifiedSignerCustodyV1 {
        self.receipt.custody()
    }
    /// Exact independently authenticated immutable completed row.
    #[must_use]
    pub const fn completion(&self) -> &SignerCompletedOperationV1 {
        self.receipt.completion()
    }
    /// Authenticated observation time for the caller's next release floor.
    #[must_use]
    pub const fn observed_at_unix_ms(&self) -> u64 {
        self.receipt.observed_at_unix_ms()
    }
}
impl fmt::Debug for VerifiedStreamTokenSignerCompletedObservationV1 {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.debug_struct("VerifiedStreamTokenSignerCompletedObservationV1")
            .finish_non_exhaustive()
    }
}

/// Consume one current-only attempt and authenticate its independently signed observation.
///
/// # Errors
/// Rejects any candidate-selected phase/query/trust, stale/revoked/forked state or invalid custody.
/// The caller retires its pending attempt on every result; rebuilding an old challenge is forbidden.
pub fn verify_stream_token_signer_current_evidence_v1(
    custody_record: &[u8],
    observation_bytes: &[u8],
    binding: &SignerCustodyBindingV1,
    custody_trust: &SignerCustodyTrustV1,
    observer_trust: &SignerStateObserverTrustV1,
    attempt: SignerStreamTokenObservationExpectedV1,
    now_unix_ms: u64,
) -> Result<VerifiedStreamTokenSignerQualificationV1, SignerStreamTokenEvidenceErrorV1> {
    if custody_record.is_empty() || custody_record.len() > SIGNER_CUSTODY_MAX_BYTES_V1 {
        return Err(SignerStreamTokenEvidenceErrorV1::InvalidDocument);
    }
    let binding_digest = stream_token_binding_digest_v1(binding)
        .map_err(SignerStreamTokenEvidenceErrorV1::Receipt)?;
    if !attempt.request.phase.is_current()
        || attempt.request.subject
            != (SignerStreamTokenObservationRequestSubjectV1::CurrentCustody { binding_digest })
    {
        return Err(SignerStreamTokenEvidenceErrorV1::SourceMismatch);
    }
    let observation = authenticate_observation(
        observation_bytes,
        &attempt,
        binding,
        custody_trust,
        observer_trust,
        now_unix_ms,
    )?;
    if observation.body.subject
        != (SignerStreamTokenStateSubjectV1::CurrentCustody { binding_digest })
    {
        return Err(SignerStreamTokenEvidenceErrorV1::SourceMismatch);
    }
    let current = observation.body.current(now_unix_ms);
    let custody = verify_signer_custody_use_v1(custody_record, binding, custody_trust, &current)
        .map_err(|error| {
            SignerStreamTokenEvidenceErrorV1::Receipt(SignerStreamTokenReceiptErrorV1::Custody(
                error,
            ))
        })?;
    if custody.statement().issued_at_unix_ms > observation.body.observed_at_unix_ms {
        return Err(SignerStreamTokenEvidenceErrorV1::InvalidState);
    }
    Ok(VerifiedStreamTokenSignerQualificationV1 {
        custody,
        phase: attempt.request.phase,
        observed_at_unix_ms: observation.body.observed_at_unix_ms,
    })
}

/// Authenticate one AfterCommit fence without authorizing final token publication.
///
/// # Errors
/// Rejects other phases and every query/state/custody/receipt mismatch. A later fresh BeforeRelease
/// challenge is mandatory; this marker has no token, signature or final-result conversion.
#[expect(
    clippy::too_many_arguments,
    reason = "independent inputs are deliberately distinct from candidate documents"
)]
pub fn verify_stream_token_signer_completed_observation_v1(
    receipt_bytes: &[u8],
    observation_bytes: &[u8],
    token: &StreamTokenV1,
    prepared: &SignerStreamTokenExpectedV1,
    binding: &SignerCustodyBindingV1,
    custody_trust: &SignerCustodyTrustV1,
    observer_trust: &SignerStateObserverTrustV1,
    attempt: SignerStreamTokenObservationExpectedV1,
    now_unix_ms: u64,
) -> Result<VerifiedStreamTokenSignerCompletedObservationV1, SignerStreamTokenEvidenceErrorV1> {
    if attempt.request.phase != SignerStreamTokenObservationPhaseV1::AfterCommit {
        return Err(SignerStreamTokenEvidenceErrorV1::SourceMismatch);
    }
    verify_completed(
        receipt_bytes,
        observation_bytes,
        token,
        prepared,
        binding,
        custody_trust,
        observer_trust,
        attempt,
        now_unix_ms,
    )
    .map(|receipt| VerifiedStreamTokenSignerCompletedObservationV1 { receipt })
}

/// Sole public final receipt release path, requiring a fresh exact BeforeRelease observation.
///
/// # Errors
/// Rejects other phases, substituted pending attempts, unauthorized observer/current state and
/// any exact original receipt failure. The consumed attempt cannot be used again after failure.
#[expect(
    clippy::too_many_arguments,
    reason = "independent inputs are deliberately distinct from candidate documents"
)]
pub fn verify_stream_token_signer_evidence_v1(
    receipt_bytes: &[u8],
    observation_bytes: &[u8],
    token: &StreamTokenV1,
    prepared: &SignerStreamTokenExpectedV1,
    binding: &SignerCustodyBindingV1,
    custody_trust: &SignerCustodyTrustV1,
    observer_trust: &SignerStateObserverTrustV1,
    attempt: SignerStreamTokenObservationExpectedV1,
    now_unix_ms: u64,
) -> Result<VerifiedStreamTokenSignerReceiptV1, SignerStreamTokenEvidenceErrorV1> {
    if attempt.request.phase != SignerStreamTokenObservationPhaseV1::BeforeRelease {
        return Err(SignerStreamTokenEvidenceErrorV1::SourceMismatch);
    }
    verify_completed(
        receipt_bytes,
        observation_bytes,
        token,
        prepared,
        binding,
        custody_trust,
        observer_trust,
        attempt,
        now_unix_ms,
    )
}

#[expect(
    clippy::too_many_arguments,
    reason = "one private owner shares the complete independent evidence checks"
)]
fn verify_completed(
    receipt_bytes: &[u8],
    observation_bytes: &[u8],
    token: &StreamTokenV1,
    prepared: &SignerStreamTokenExpectedV1,
    binding: &SignerCustodyBindingV1,
    custody_trust: &SignerCustodyTrustV1,
    observer_trust: &SignerStateObserverTrustV1,
    attempt: SignerStreamTokenObservationExpectedV1,
    now_unix_ms: u64,
) -> Result<VerifiedStreamTokenSignerReceiptV1, SignerStreamTokenEvidenceErrorV1> {
    let exact_subject = completed_request_subject(receipt_bytes, token, prepared, binding)?;
    if attempt.request.subject != exact_subject || attempt.request.phase.is_current() {
        return Err(SignerStreamTokenEvidenceErrorV1::SourceMismatch);
    }
    let observation = authenticate_observation(
        observation_bytes,
        &attempt,
        binding,
        custody_trust,
        observer_trust,
        now_unix_ms,
    )?;
    let SignerStreamTokenStateSubjectV1::CompletedOperation {
        completed_operation,
        ..
    } = &observation.body.subject
    else {
        return Err(SignerStreamTokenEvidenceErrorV1::SourceMismatch);
    };
    let current = observation.body.current(now_unix_ms);
    verify_stream_token_signer_receipt_v1(
        receipt_bytes,
        token,
        prepared,
        binding,
        custody_trust,
        &current,
        completed_operation,
    )
    .map_err(SignerStreamTokenEvidenceErrorV1::Receipt)
}

fn authenticate_observation(
    bytes: &[u8],
    attempt: &SignerStreamTokenObservationExpectedV1,
    binding: &SignerCustodyBindingV1,
    custody: &SignerCustodyTrustV1,
    trust: &SignerStateObserverTrustV1,
    now: u64,
) -> Result<SignerStreamTokenStateObservationV1, SignerStreamTokenEvidenceErrorV1> {
    let observation = SignerStreamTokenStateObservationV1::decode_canonical(bytes)?;
    let body = &observation.body;
    if body.request_digest != attempt.request.digest()?
        || body.phase != attempt.request.phase
        || !body.subject.matches_request(&attempt.request.subject)
    {
        return Err(SignerStreamTokenEvidenceErrorV1::SourceMismatch);
    }
    trust
        .validate(binding, custody, now)
        .map_err(|_| SignerStreamTokenEvidenceErrorV1::InvalidTrust)?;
    let state = SignerStateObservationViewV1 {
        authority: &body.authority,
        chain_id: &body.chain_id,
        network_id: &body.network_id,
        observed_at_unix_ms: body.observed_at_unix_ms,
        expires_at_unix_ms: body.expires_at_unix_ms,
        current_anchor: body.current_anchor,
        signer_revoked: body.signer_revoked,
        attester_revoked: body.attester_revoked,
    };
    if !state.matches_identity(trust, binding)
        || now < attempt.request.not_before_unix_ms
        || body.observed_at_unix_ms < attempt.request.not_before_unix_ms
    {
        return Err(SignerStreamTokenEvidenceErrorV1::InvalidState);
    }
    state
        .validate_freshness(trust, now)
        .map_err(|_| SignerStreamTokenEvidenceErrorV1::InvalidState)?;
    if let SignerStreamTokenStateSubjectV1::CompletedOperation {
        completed_operation,
        ..
    } = &body.subject
    {
        if completed_operation.completed_at_unix_ms > body.observed_at_unix_ms {
            return Err(SignerStreamTokenEvidenceErrorV1::InvalidState);
        }
    }
    state
        .validate_finality(trust, &attempt.request.minimum_anchor)
        .map_err(|_| SignerStreamTokenEvidenceErrorV1::InvalidState)?;
    trust
        .verify_signature(&body.signing_payload()?, &observation.signature)
        .map_err(|_| SignerStreamTokenEvidenceErrorV1::InvalidState)?;
    Ok(observation)
}

fn valid_anchor(anchor: SignerCustodyAnchorV1) -> bool {
    anchor.height != 0 && anchor.block_hash != [0; 32] && anchor.state_digest != [0; 32]
}

fn encode_document<T: norito::NoritoSerialize>(
    value: &T,
    limit: usize,
) -> Result<Vec<u8>, SignerStreamTokenEvidenceErrorV1> {
    let length = norito::canonical_frame_len(value)
        .map_err(|_| SignerStreamTokenEvidenceErrorV1::InvalidDocument)?;
    if length > limit {
        return Err(SignerStreamTokenEvidenceErrorV1::InvalidDocument);
    }
    let bytes = norito::encode_canonical(value)
        .map_err(|_| SignerStreamTokenEvidenceErrorV1::InvalidDocument)?;
    if bytes.len() != length {
        return Err(SignerStreamTokenEvidenceErrorV1::InvalidDocument);
    }
    Ok(bytes)
}

fn decode_document<T: for<'de> norito::NoritoDeserialize<'de> + norito::NoritoSerialize>(
    bytes: &[u8],
    limit: usize,
) -> Result<T, SignerStreamTokenEvidenceErrorV1> {
    if bytes.is_empty() || bytes.len() > limit {
        return Err(SignerStreamTokenEvidenceErrorV1::InvalidDocument);
    }
    let allocation = bytes
        .len()
        .checked_mul(8)
        .and_then(|n| n.checked_add(64 * 1024))
        .map(|n| n.min(512 * 1024))
        .ok_or(SignerStreamTokenEvidenceErrorV1::InvalidDocument)?;
    norito::decode_canonical_with_limits(
        bytes,
        norito::DecodeLimits::new(4096, bytes.len(), 8192, allocation, 24),
    )
    .map_err(|_| SignerStreamTokenEvidenceErrorV1::InvalidDocument)
}

#[cfg(test)]
mod tests;
