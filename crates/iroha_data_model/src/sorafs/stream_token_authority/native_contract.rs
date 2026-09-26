//! Purpose-owned role-11 native operation and challenged Check DTOs.
//!
//! These are untrusted typed claims, not registered instructions or proof of execution. Core must
//! derive operation reservation IDs, fences, completion times and execution provenance from
//! authoritative State, compare one-time operation IDs, and consume signed Checks against Kura/QC
//! finality before the private signer can use them.

use super::{
    StreamTokenClaimErrorV1, StreamTokenExpireV1, StreamTokenOperationV1, StreamTokenOutcomeV1,
    StreamTokenReviewedV1, validate_stream_token_complete_claim_v1,
    validate_stream_token_reviewed_claim_v1,
};
use crate::{
    DeriveJsonDeserialize, DeriveJsonSerialize, account::AccountId,
    block::consensus_v2::HeightContextId, sorafs::capacity::ProviderId,
};
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
use sorafs_manifest::signer::protocol::{
    SignerOperationAuditHeadV1, SignerOperationCommitmentV1, SignerOperationReservationV1,
};

/// Maximum complete canonical role-11 native request claim, including its Norito header.
pub const STREAM_TOKEN_AUTHORITY_REQUEST_MAX_BYTES_V1: usize = 16 * 1024;

/// Complete the exact original slot without letting a transaction choose its execution time.
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
#[norito_schema(
    name = "iroha_data_model::sorafs::stream_token_authority::StreamTokenCompleteRequestV1"
)]
#[norito(deny_unknown_fields)]
pub struct StreamTokenCompleteRequestV1 {
    /// Exact original provider-scoped body, binding, custody and audit predecessor.
    pub reviewed: StreamTokenReviewedV1,
    /// Original exclusive reservation and fence from native Reserve.
    pub reservation: SignerOperationReservationV1,
    /// Next audit head and response commitment staged before the completion transaction.
    pub commitment: SignerOperationCommitmentV1,
    /// Digest of all four ordered signatures in the immutable private receipt.
    #[norito(json = "crate::json_helpers::fixed_bytes")]
    pub signatures_digest: [u8; 32],
}

/// Actual native transition coordinates, never copied from a submitted request.
#[derive(
    Clone,
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
#[norito_schema(name = "iroha_data_model::sorafs::stream_token_authority::StreamTokenExecutionV1")]
#[norito(deny_unknown_fields)]
pub struct StreamTokenExecutionV1 {
    /// Actual one-based executing block height.
    pub height: u64,
    /// Hash of the exact canonical signed transaction entry in the executing block.
    #[norito(json = "crate::json_helpers::fixed_bytes")]
    pub transaction_hash: [u8; 32],
    /// Zero-based transaction entry index in that block's network execution proof.
    pub entry_index: u32,
    /// Zero-based native instruction index in the successful transaction entry.
    pub instruction_index: u32,
    /// Actual deterministic execution block timestamp in Unix milliseconds.
    pub recorded_at_unix_ms: u64,
    /// Registered provider owner with the exact role-11 operation permission at execution.
    pub authority: AccountId,
}

/// Claimed immutable native operation row with its original and terminal execution coordinates.
///
/// This can be serialized by tests or transport but is authoritative only after a native reader
/// reconstructs it from the same committed State as a successful signed Check and Kura/QC proof.
#[derive(
    Clone,
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
#[norito_schema(
    name = "iroha_data_model::sorafs::stream_token_authority::StreamTokenNativeOperationV1"
)]
#[norito(deny_unknown_fields)]
pub struct StreamTokenNativeOperationV1 {
    /// Stable registered provider whose operation-ID tombstone cannot be reused.
    pub provider_id: ProviderId,
    /// Exact native custody control revision when the slot was reserved.
    pub custody_control_revision: u64,
    /// Exact canonical native custody control digest at reservation.
    #[norito(json = "crate::json_helpers::fixed_bytes")]
    pub custody_control_digest: [u8; 32],
    /// Original immutable reviewed request, reservation and current outcome.
    pub operation: StreamTokenOperationV1,
    /// Execution-derived Reserve provenance, retained after completion or expiry.
    pub reserved_execution: StreamTokenExecutionV1,
    /// Execution-derived Complete or Expire provenance, absent while reserved.
    pub terminal_execution: Option<StreamTokenExecutionV1>,
}

/// Independently retained finalized floor preceding each new Check transaction.
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
#[norito_schema(
    name = "iroha_data_model::sorafs::stream_token_authority::StreamTokenFinalityFloorV1"
)]
#[norito(deny_unknown_fields)]
pub struct StreamTokenFinalityFloorV1 {
    /// Positive finalized native height, independently retained before this Check.
    pub height: u64,
    /// Exact signed-RS16 block hash at that height.
    #[norito(json = "crate::json_helpers::fixed_bytes")]
    pub block_hash: [u8; 32],
    /// Exact signed-RS16 height context required for successor-chain verification.
    pub context_id: HeightContextId,
}

/// Closed observation phases for one challenged role-11 native operation.
#[derive(
    Clone,
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
#[norito_schema(name = "iroha_data_model::sorafs::stream_token_authority::StreamTokenCheckPhaseV1")]
#[norito(
    tag = "phase",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum StreamTokenCheckPhaseV1 {
    /// Current eligible custody and audit predecessor before reserving this operation ID.
    #[codec(index = 0)]
    Current(SignerOperationAuditHeadV1),
    /// Exact original unexpired Reserved row immediately before provider use.
    #[codec(index = 1)]
    BeforeProvider(StreamTokenNativeOperationV1),
    /// Same original Reserved row after provider use.
    #[codec(index = 2)]
    AfterProvider(StreamTokenNativeOperationV1),
    /// Same original Reserved row before completion CAS.
    #[codec(index = 3)]
    BeforeCommit(StreamTokenNativeOperationV1),
    /// Exact timely Completed row after CAS or at recovery entry.
    #[codec(index = 4)]
    AfterCommit(StreamTokenNativeOperationV1),
    /// Exact Completed row immediately before private receipt release.
    #[codec(index = 5)]
    BeforeRelease(StreamTokenNativeOperationV1),
}

/// Fresh no-write native Check, authorized by an observer distinct from the provider operator.
#[derive(
    Clone,
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
#[norito_schema(name = "iroha_data_model::sorafs::stream_token_authority::StreamTokenCheckV1")]
#[norito(deny_unknown_fields)]
pub struct StreamTokenCheckV1 {
    /// One fresh unpredictable 256-bit challenge, retired after the local attempt.
    #[norito(json = "crate::json_helpers::fixed_bytes")]
    pub challenge: [u8; 32],
    /// Independently pinned registered provider owner expected to have operation permission.
    pub expected_operator: AccountId,
    /// Independently pinned Check observer, distinct from the protected signer and operator.
    pub expected_observer: AccountId,
    /// Independently retained finalized floor, never chosen by the candidate row.
    pub floor: StreamTokenFinalityFloorV1,
    /// Exact original body, binding, custody, operation ID and audit predecessor.
    pub reviewed: StreamTokenReviewedV1,
    /// Phase-specific audit or immutable original native operation.
    pub phase: StreamTokenCheckPhaseV1,
}

/// Sole proposed role-11 native operation surface; not registered as an instruction.
#[expect(
    clippy::large_enum_variant,
    reason = "role-11 Check and Complete stay inline in canonical V1 Norito; boxing adds a wire length and decoder allocation"
)]
#[derive(
    Clone,
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
#[norito_schema(
    name = "iroha_data_model::sorafs::stream_token_authority::StreamTokenAuthorityActionV1"
)]
#[norito(
    tag = "action",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum StreamTokenAuthorityActionV1 {
    /// Request a first-use operation ID and native-derived exclusive reservation.
    #[codec(index = 0)]
    Reserve(StreamTokenReviewedV1),
    /// Commit the original reservation and exact staged four-signature digest.
    #[codec(index = 1)]
    Complete(StreamTokenCompleteRequestV1),
    /// Terminalize an expired or revoked slot without removing its ID tombstone.
    #[codec(index = 2)]
    Expire(StreamTokenExpireV1),
    /// Evaluate a fresh phase-specific predicate without changing native state.
    #[codec(index = 3)]
    Check(StreamTokenCheckV1),
}

/// Exact first-release native request DTO, currently outside the instruction registry.
///
/// Core must bind these coordinates to the actual signed transaction network, registered
/// provider owner, custody revision/digest and separately authorized observer. Decoding this
/// type gives no permission, committed operation, finalized Check or token-release authority.
#[derive(
    Clone,
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
#[norito_schema(
    name = "iroha_data_model::sorafs::stream_token_authority::StreamTokenAuthorityRequestV1"
)]
#[norito(deny_unknown_fields)]
pub struct StreamTokenAuthorityRequestV1 {
    /// Genesis-derived native network identity.
    #[norito(json = "crate::json_helpers::fixed_bytes")]
    pub network_id: [u8; 32],
    /// Exact registered provider and per-provider operation-ID replay namespace.
    pub provider_id: ProviderId,
    /// Current native custody control revision required by CAS.
    pub expected_control_revision: u64,
    /// Current native custody control digest required by CAS.
    #[norito(json = "crate::json_helpers::fixed_bytes")]
    pub expected_control_digest: [u8; 32],
    /// One phase-specific action; no compatibility action or alternate layout exists.
    pub action: StreamTokenAuthorityActionV1,
}

/// Decode one bounded canonical role-11 request claim without registering it for execution.
///
/// # Errors
/// Rejects empty, oversized, truncated or noncanonical Norito frames.
pub fn decode_stream_token_authority_request_claim_v1(
    frame: &[u8],
) -> Result<StreamTokenAuthorityRequestV1, StreamTokenClaimErrorV1> {
    if frame.is_empty() || frame.len() > STREAM_TOKEN_AUTHORITY_REQUEST_MAX_BYTES_V1 {
        return Err(StreamTokenClaimErrorV1::Encoding);
    }
    norito::decode_canonical(frame).map_err(|_| StreamTokenClaimErrorV1::Encoding)
}

/// Check a candidate Complete action against one independently retained Reserved row.
///
/// The caller must derive the original row from authoritative native State and compare its
/// actual execution time separately; this shape check cannot commit or release a token.
///
/// # Errors
/// Rejects another request, slot, audit successor or inert signature digest.
pub fn validate_stream_token_complete_request_claim_v1(
    completion: &StreamTokenCompleteRequestV1,
    original: &StreamTokenOperationV1,
) -> Result<(), StreamTokenClaimErrorV1> {
    let previous = completion.reviewed.intent.previous_audit;
    if original.outcome != StreamTokenOutcomeV1::Reserved
        || completion.reviewed != original.reviewed
        || completion.reservation != original.reservation
        || completion.reservation.reservation_id == [0; 32]
        || completion.reservation.fence == 0
        || completion.reservation.expires_at_unix_ms == 0
        || completion.commitment.audit.sequence
            != previous
                .sequence
                .checked_add(1)
                .ok_or(StreamTokenClaimErrorV1::Operation)?
        || completion.commitment.audit.digest == [0; 32]
        || completion.commitment.audit.digest == previous.digest
        || completion.commitment.response_digest == [0; 32]
        || completion.signatures_digest == [0; 32]
    {
        return Err(StreamTokenClaimErrorV1::Operation);
    }
    Ok(())
}

/// Validate one untrusted operation row's structural chronology and purpose binding.
///
/// The caller must compare this row to an indexed native State record at the exact finalized
/// height. A coherent serialized row remains only a claim.
///
/// # Errors
/// Rejects a foreign provider, custody generation, operator, impossible phase or time sequence.
pub fn validate_stream_token_native_operation_claim_v1(
    row: &StreamTokenNativeOperationV1,
    expected_provider: ProviderId,
    expected_control_revision: u64,
    expected_control_digest: [u8; 32],
    expected_operator: &AccountId,
) -> Result<(), StreamTokenClaimErrorV1> {
    let reservation = row.operation.reservation;
    if expected_provider.as_bytes() == &[0; 32]
        || row.provider_id != expected_provider
        || expected_control_revision == 0
        || row.custody_control_revision != expected_control_revision
        || expected_control_digest == [0; 32]
        || row.custody_control_digest != expected_control_digest
        || row.reserved_execution.height == 0
        || row.reserved_execution.transaction_hash == [0; 32]
        || row.reserved_execution.authority != *expected_operator
        || row.reserved_execution.recorded_at_unix_ms
            < row.operation.reviewed.request.issued_at_unix_ms
        || row.reserved_execution.recorded_at_unix_ms >= reservation.expires_at_unix_ms
        || row.reserved_execution.recorded_at_unix_ms
            >= row.operation.reviewed.request.expires_at_unix_ms
        || reservation.reservation_id == [0; 32]
        || reservation.fence == 0
    {
        return Err(StreamTokenClaimErrorV1::Operation);
    }
    validate_stream_token_reviewed_claim_v1(
        &row.operation.reviewed,
        &row.operation.reviewed.request,
        row.operation.reviewed.intent.previous_audit,
    )?;
    match (&row.operation.outcome, &row.terminal_execution) {
        (StreamTokenOutcomeV1::Reserved, None) => {}
        (StreamTokenOutcomeV1::Completed(completion), Some(terminal)) => {
            let original = StreamTokenOperationV1 {
                outcome: StreamTokenOutcomeV1::Reserved,
                ..row.operation
            };
            validate_stream_token_complete_claim_v1(completion, &original)?;
            if (
                terminal.height,
                terminal.entry_index,
                terminal.instruction_index,
            ) <= (
                row.reserved_execution.height,
                row.reserved_execution.entry_index,
                row.reserved_execution.instruction_index,
            ) || terminal.transaction_hash == [0; 32]
                || terminal.transaction_hash == row.reserved_execution.transaction_hash
                || terminal.authority != *expected_operator
                || terminal.recorded_at_unix_ms < row.reserved_execution.recorded_at_unix_ms
                || terminal.recorded_at_unix_ms != completion.completed_at_unix_ms
            {
                return Err(StreamTokenClaimErrorV1::Operation);
            }
        }
        (StreamTokenOutcomeV1::Expired, Some(terminal)) => {
            // A custody-manager revocation may terminalize the original operator's slot.
            // Core must authenticate the exact terminal action and its scoped permission.
            if (
                terminal.height,
                terminal.entry_index,
                terminal.instruction_index,
            ) <= (
                row.reserved_execution.height,
                row.reserved_execution.entry_index,
                row.reserved_execution.instruction_index,
            ) || terminal.transaction_hash == [0; 32]
                || terminal.transaction_hash == row.reserved_execution.transaction_hash
                || terminal.recorded_at_unix_ms < row.reserved_execution.recorded_at_unix_ms
            {
                return Err(StreamTokenClaimErrorV1::Operation);
            }
        }
        _ => return Err(StreamTokenClaimErrorV1::Phase),
    }
    Ok(())
}

/// Compare a Check to independently pinned network/provider/control, observer nonce and floor.
///
/// The expected phase must come from the same authenticated native State view; equality and
/// structural checks here cannot prove the signed Check actually executed or finalized.
///
/// # Errors
/// Rejects substituted network, provider, control, operator, nonce, floor, request or phase.
#[expect(
    clippy::too_many_arguments,
    reason = "all independent Check pins stay explicit"
)]
pub fn validate_stream_token_check_claim_v1(
    request: &StreamTokenAuthorityRequestV1,
    expected_network_id: [u8; 32],
    expected_provider: ProviderId,
    expected_control_revision: u64,
    expected_control_digest: [u8; 32],
    expected_operator: &AccountId,
    expected_observer: &AccountId,
    expected_challenge: [u8; 32],
    expected_floor: StreamTokenFinalityFloorV1,
    expected_reviewed: &StreamTokenReviewedV1,
    expected_phase: &StreamTokenCheckPhaseV1,
) -> Result<(), StreamTokenClaimErrorV1> {
    let StreamTokenAuthorityActionV1::Check(check) = &request.action else {
        return Err(StreamTokenClaimErrorV1::Phase);
    };
    if expected_network_id == [0; 32]
        || request.network_id != expected_network_id
        || request.provider_id != expected_provider
        || expected_provider.as_bytes() == &[0; 32]
        || expected_control_revision == 0
        || request.expected_control_revision != expected_control_revision
        || expected_control_digest == [0; 32]
        || request.expected_control_digest != expected_control_digest
        || expected_challenge == [0; 32]
        || check.challenge != expected_challenge
        || &check.expected_operator != expected_operator
        || expected_observer == expected_operator
        || &check.expected_observer != expected_observer
        || expected_floor.height == 0
        || expected_floor.block_hash == [0; 32]
        || *expected_floor.context_id.0.as_ref() == [0; 32]
        || check.floor != expected_floor
        || &check.reviewed != expected_reviewed
        || &check.phase != expected_phase
    {
        return Err(StreamTokenClaimErrorV1::Round);
    }
    validate_stream_token_reviewed_claim_v1(
        &check.reviewed,
        &expected_reviewed.request,
        expected_reviewed.intent.previous_audit,
    )?;
    match &check.phase {
        StreamTokenCheckPhaseV1::Current(audit) => {
            if *audit != check.reviewed.intent.previous_audit {
                return Err(StreamTokenClaimErrorV1::Phase);
            }
        }
        StreamTokenCheckPhaseV1::BeforeProvider(row)
        | StreamTokenCheckPhaseV1::AfterProvider(row)
        | StreamTokenCheckPhaseV1::BeforeCommit(row) => {
            validate_stream_token_native_operation_claim_v1(
                row,
                expected_provider,
                expected_control_revision,
                expected_control_digest,
                expected_operator,
            )?;
            if row.reserved_execution.height > expected_floor.height
                || row.operation.reviewed != check.reviewed
                || row.operation.outcome != StreamTokenOutcomeV1::Reserved
            {
                return Err(StreamTokenClaimErrorV1::Phase);
            }
        }
        StreamTokenCheckPhaseV1::AfterCommit(row) | StreamTokenCheckPhaseV1::BeforeRelease(row) => {
            validate_stream_token_native_operation_claim_v1(
                row,
                expected_provider,
                expected_control_revision,
                expected_control_digest,
                expected_operator,
            )?;
            if row.reserved_execution.height > expected_floor.height
                || row
                    .terminal_execution
                    .as_ref()
                    .is_some_and(|execution| execution.height > expected_floor.height)
                || row.operation.reviewed != check.reviewed
                || !matches!(row.operation.outcome, StreamTokenOutcomeV1::Completed(_))
            {
                return Err(StreamTokenClaimErrorV1::Phase);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
