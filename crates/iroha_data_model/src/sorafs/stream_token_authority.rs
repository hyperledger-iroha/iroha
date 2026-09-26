//! Role-11 stream-token operation claims under the existing provider-scoped custody contract.
//!
//! These are bounded canonical claims, not native State records or signing authority. The private
//! signer prepares the exact token body and binding, while Core must own immutable reservation,
//! completion, expiry, audit and replay-tombstone rows and a finalized Check consumer before any
//! production operation can be admitted. A caller must compare these claims to independently
//! retained body, binding, original custody and audit inputs; detached observations cannot do so.
//! TODO: Add purpose-owned State operation storage, signed Check execution/finality and the
//! production state source before connecting the private four-signature stream-token service.

use crate::{DeriveJsonDeserialize, DeriveJsonSerialize};
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
use sorafs_manifest::signer::{
    protocol::{
        SignerOperationActionV1, SignerOperationAuditHeadV1, SignerOperationCommitmentV1,
        SignerOperationIntentV1, SignerOperationReservationV1,
    },
    stream_token::{SIGNER_STREAM_TOKEN_MAX_PAYLOAD_BYTES_V1, SignerStreamTokenRequestV1},
};
use std::fmt;

/// Maximum complete canonical role-11 operation claim, including the Norito header.
pub const STREAM_TOKEN_OPERATION_CLAIM_MAX_BYTES_V1: usize = 8 * 1024;

/// Exact private-service request and ordinary Sign intent, before any provider I/O.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_data_model::sorafs::stream_token_authority::StreamTokenReviewedV1")]
#[norito(deny_unknown_fields)]
pub struct StreamTokenReviewedV1 {
    /// Exact provider, original custody, canonical payload and derived issue/expiry commitments.
    pub request: SignerStreamTokenRequestV1,
    /// Exact Sign action, request digest and independently selected audit predecessor.
    pub intent: SignerOperationIntentV1,
}

/// Candidate completion of the original exclusive reservation and four staged signatures.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_data_model::sorafs::stream_token_authority::StreamTokenCompleteV1")]
#[norito(deny_unknown_fields)]
pub struct StreamTokenCompleteV1 {
    /// Original reviewed request and intent; renewal cannot replace either.
    pub reviewed: StreamTokenReviewedV1,
    /// Original exclusive reservation and fence, never a replacement.
    pub reservation: SignerOperationReservationV1,
    /// Exact next audit and staged response commitment.
    pub commitment: SignerOperationCommitmentV1,
    /// Nonzero digest of four exact ordered privately staged signatures.
    pub signatures_digest: [u8; 32],
    /// Claimed completion time; Core must replace it with actual execution time.
    pub completed_at_unix_ms: u64,
}

/// Candidate terminalization of one original reservation without ID reuse.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_data_model::sorafs::stream_token_authority::StreamTokenExpireV1")]
#[norito(deny_unknown_fields)]
pub struct StreamTokenExpireV1 {
    /// Original operation ID; it remains permanently spent after terminalization.
    pub operation_id: [u8; 32],
    /// Exact original reservation to expire.
    pub reservation: SignerOperationReservationV1,
}

/// Claimed outcome of one original operation; only native retained state can authenticate it.
#[expect(
    clippy::large_enum_variant,
    reason = "role-11 completion stays inline and Copy in canonical V1 Norito; boxing adds a wire length and heap allocation"
)]
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_data_model::sorafs::stream_token_authority::StreamTokenOutcomeV1")]
#[norito(
    tag = "state",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum StreamTokenOutcomeV1 {
    /// Original exclusive slot remains open; no signature can be released.
    #[codec(index = 0)]
    Reserved,
    /// Original operation was claimed completed under the exact slot.
    #[codec(index = 1)]
    Completed(StreamTokenCompleteV1),
    /// Original operation was terminalized; the ID remains spent.
    #[codec(index = 2)]
    Expired,
}

/// Claimed original slot and outcome, never proof of native execution or finality.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_data_model::sorafs::stream_token_authority::StreamTokenOperationV1")]
#[norito(deny_unknown_fields)]
pub struct StreamTokenOperationV1 {
    /// Original reviewed request, purpose and audit predecessor.
    pub reviewed: StreamTokenReviewedV1,
    /// Original exclusive reservation.
    pub reservation: SignerOperationReservationV1,
    /// Claimed outcome, authenticated only by a future native reader.
    pub outcome: StreamTokenOutcomeV1,
}

/// Pure claim-shape failure; no variant represents admitted native authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StreamTokenClaimErrorV1 {
    /// Missing, oversized, truncated or noncanonical Norito frame.
    Encoding,
    /// Reviewed body, provider, custody, intent or audit predecessor differs.
    Review,
    /// Reservation, completion, expiry or audit successor differs.
    Operation,
    /// A Check phase or claimed native row contradicts its original outcome.
    Phase,
    /// Network, provider, control, observer challenge, operator or finality floor differs.
    Round,
}
impl fmt::Display for StreamTokenClaimErrorV1 {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(match self {
            Self::Encoding => "invalid stream-token operation claim encoding",
            Self::Review => "invalid stream-token reviewed request",
            Self::Operation => "invalid stream-token operation claim",
            Self::Phase => "invalid stream-token check phase",
            Self::Round => "stream-token authority round mismatch",
        })
    }
}
impl std::error::Error for StreamTokenClaimErrorV1 {}

/// Decode one bounded canonical operation claim without admitting or authenticating it.
///
/// # Errors
/// Rejects missing, oversized, truncated or noncanonical Norito frames.
pub fn decode_stream_token_operation_claim_v1(
    frame: &[u8],
) -> Result<StreamTokenOperationV1, StreamTokenClaimErrorV1> {
    if frame.is_empty() || frame.len() > STREAM_TOKEN_OPERATION_CLAIM_MAX_BYTES_V1 {
        return Err(StreamTokenClaimErrorV1::Encoding);
    }
    norito::decode_canonical(frame).map_err(|_| StreamTokenClaimErrorV1::Encoding)
}

/// Compare a claim to an independently prepared exact body/binding/custody and audit predecessor.
///
/// The caller must derive `expected_request` using the existing Manifest stream-token body and
/// binding verifier plus independently authenticated original custody. Equality here alone
/// cannot establish those inputs, permission, finality, or a right to sign.
///
/// # Errors
/// Rejects wrong or inert request/intent coordinates, altered custody, payload or predecessor.
pub fn validate_stream_token_reviewed_claim_v1(
    reviewed: &StreamTokenReviewedV1,
    expected_request: &SignerStreamTokenRequestV1,
    expected_audit: SignerOperationAuditHeadV1,
) -> Result<(), StreamTokenClaimErrorV1> {
    let request = &reviewed.request;
    if request != expected_request
        || request.operation_id == [0; 32]
        || request.binding_digest == [0; 32]
        || request.original_custody.record_digest == [0; 32]
        || request.original_custody.control_state_digest == [0; 32]
        || request.signing_payload_digest == [0; 32]
        || request.signing_payload_size == 0
        || request.signing_payload_size > SIGNER_STREAM_TOKEN_MAX_PAYLOAD_BYTES_V1 as u64
        || request.issued_at_unix_ms == 0
        || request.issued_at_unix_ms >= request.expires_at_unix_ms
        || reviewed.intent.action != SignerOperationActionV1::Sign
        || reviewed.intent.operation_id != request.operation_id
        || reviewed.intent.request_digest
            != request
                .digest()
                .map_err(|_| StreamTokenClaimErrorV1::Review)?
        || reviewed.intent.previous_audit != expected_audit
        || reviewed.intent.digest().is_err()
    {
        return Err(StreamTokenClaimErrorV1::Review);
    }
    Ok(())
}

/// Check a claimed completion against one independently retained original Reserved row.
///
/// Core must derive completion time from execution and compare the real original row, current
/// custody, staged receipt, four signatures, audit successor and finality before release.
///
/// # Errors
/// Rejects a changed request, slot, audit successor, signature digest or impossible chronology.
pub fn validate_stream_token_complete_claim_v1(
    completion: &StreamTokenCompleteV1,
    original: &StreamTokenOperationV1,
) -> Result<(), StreamTokenClaimErrorV1> {
    let previous = completion.reviewed.intent.previous_audit;
    let reservation = completion.reservation;
    if original.outcome != StreamTokenOutcomeV1::Reserved
        || completion.reviewed != original.reviewed
        || reservation != original.reservation
        || reservation.reservation_id == [0; 32]
        || reservation.fence == 0
        || reservation.expires_at_unix_ms == 0
        || completion.completed_at_unix_ms < completion.reviewed.request.issued_at_unix_ms
        || completion.completed_at_unix_ms >= reservation.expires_at_unix_ms
        || completion.completed_at_unix_ms >= completion.reviewed.request.expires_at_unix_ms
        || completion.commitment.audit.sequence
            != previous
                .sequence
                .checked_add(1)
                .ok_or(StreamTokenClaimErrorV1::Operation)?
        || completion.commitment.audit.digest == [0; 32]
        || completion.commitment.response_digest == [0; 32]
        || completion.signatures_digest == [0; 32]
    {
        return Err(StreamTokenClaimErrorV1::Operation);
    }
    Ok(())
}

/// Compare an expiry claim with the independently retained exact original Reserved row.
///
/// Core must derive the actual expiry decision from its trusted execution time. This pure check
/// cannot terminalize the ID or prove that the original reservation was ever committed.
///
/// # Errors
/// Rejects a foreign ID/slot or an already terminal row.
pub fn validate_stream_token_expire_claim_v1(
    expired: &StreamTokenExpireV1,
    original: &StreamTokenOperationV1,
) -> Result<(), StreamTokenClaimErrorV1> {
    if original.outcome != StreamTokenOutcomeV1::Reserved
        || expired.operation_id != original.reviewed.request.operation_id
        || expired.reservation != original.reservation
        || expired.operation_id == [0; 32]
        || expired.reservation.reservation_id == [0; 32]
        || expired.reservation.fence == 0
        || expired.reservation.expires_at_unix_ms == 0
    {
        return Err(StreamTokenClaimErrorV1::Operation);
    }
    Ok(())
}

mod native_contract;
pub use native_contract::{
    STREAM_TOKEN_AUTHORITY_REQUEST_MAX_BYTES_V1, StreamTokenAuthorityActionV1,
    StreamTokenAuthorityRequestV1, StreamTokenCheckPhaseV1, StreamTokenCheckV1,
    StreamTokenCompleteRequestV1, StreamTokenExecutionV1, StreamTokenFinalityFloorV1,
    StreamTokenNativeOperationV1, decode_stream_token_authority_request_claim_v1,
    validate_stream_token_check_claim_v1, validate_stream_token_complete_request_claim_v1,
    validate_stream_token_native_operation_claim_v1,
};

#[cfg(test)]
mod tests;
