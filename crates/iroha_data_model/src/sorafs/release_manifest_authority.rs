//! Role-13 release-manifest authority claims for one canonical native operation.
//!
//! These bounded Norito values include claims and a distinct immutable custody record schema.
//! Decoding either does not establish custody, permission, execution, finality, or a right to sign.
//! Core must derive execution coordinates, authenticate the independent reviewed manifest and
//! current custody, and atomically retain reservation, completion, audit and ID tombstones.
//! The shared Manifest request and these native claims retain one strict canonical JSON/schema
//! representation alongside their Norito wire frames. The registered ISI is closed in Core:
//! neither decoding nor successful claim preflight grants signing or mutation authority.
//! TODO: Add persisted indices, same-State Check consumer and finalized daemon state source
//! before admitting this instruction or exposing the release signing command.

use crate::{DeriveJsonDeserialize, DeriveJsonSerialize, account::AccountId};
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
use sorafs_manifest::signer::{
    protocol::{
        SIGNER_RELEASE_MANIFEST_MAX_BYTES_V1, SignerOperationActionV1, SignerOperationAuditHeadV1,
        SignerOperationCommitmentV1, SignerOperationIntentV1, SignerOperationReservationV1,
    },
    receipt::SignerReleaseManifestRequestV1,
};
use std::fmt;

/// Independent native namespace; other signer roles cannot share its history or permissions.
pub const RELEASE_MANIFEST_AUTHORITY_NAMESPACE_V1: &str = "sorafs_release_manifest_authority_v1";
/// Maximum canonical native action frame, including its Norito layout header.
pub const RELEASE_MANIFEST_ACTION_MAX_BYTES_V1: usize = 48 * 1024;
/// Permanent operation-ID capacity; terminalization must retain every admitted ID.
pub const RELEASE_MANIFEST_OPERATION_LIMIT_V1: u64 = 65_536;
/// Maximum exclusive reservation lifetime, further capped by governed custody.
pub const RELEASE_MANIFEST_RESERVATION_MS_V1: u64 = 60_000;
/// Total retained custody revisions, including two emergency revocations.
pub const RELEASE_MANIFEST_CUSTODY_MAX_REVISIONS_V1: u64 = 8_194;
/// Configure/enroll ceiling, reserving two revisions for emergency revocation.
pub const RELEASE_MANIFEST_CUSTODY_NORMAL_REVISIONS_V1: u64 = 8_192;
/// Maximum complete canonical native custody record, including its Norito header.
pub const RELEASE_MANIFEST_CUSTODY_MAX_RECORD_BYTES_V1: usize = 32 * 1024;
/// Purpose-owned immutable custody commitment domain, distinct from all other signer roles.
pub const RELEASE_MANIFEST_CUSTODY_RECORD_DOMAIN_V1: &[u8] =
    b"iroha.sorafs.release-manifest.custody-control.v1\0";

/// Governed revocation of the current release signer or independent attester generation.
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
    name = "iroha_data_model::sorafs::release_manifest_authority::ReleaseManifestRevocationV1"
)]
#[norito(deny_unknown_fields)]
pub struct ReleaseManifestRevocationV1 {
    /// Revoke the current signer key generation.
    pub signer: bool,
    /// Revoke the current independent attester generation.
    pub attester: bool,
}

/// Exact reviewed manifest request and audit predecessor to reserve before provider I/O.
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
    name = "iroha_data_model::sorafs::release_manifest_authority::ReleaseManifestReserveV1"
)]
#[norito(deny_unknown_fields)]
pub struct ReleaseManifestReserveV1 {
    /// Existing purpose-specific request; its manifest digest is of the original raw bytes.
    pub request: SignerReleaseManifestRequestV1,
    /// Exact Sign action, request digest and independently selected audit predecessor.
    pub intent: SignerOperationIntentV1,
}

/// Completion claims the original reservation and commits privately staged signatures.
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
    name = "iroha_data_model::sorafs::release_manifest_authority::ReleaseManifestCompleteV1"
)]
#[norito(deny_unknown_fields)]
pub struct ReleaseManifestCompleteV1 {
    /// Original reviewed request and intent; renewal cannot replace them.
    pub reviewed: ReleaseManifestReserveV1,
    /// Exact exclusive reservation and fence, never a newly issued replacement.
    pub reservation: SignerOperationReservationV1,
    /// One next immutable audit and the exact staged response commitment.
    pub commitment: SignerOperationCommitmentV1,
    /// Nonzero digest of the four ordered privately staged signatures.
    #[norito(json = "crate::json_helpers::fixed_bytes")]
    pub signatures_digest: [u8; 32],
    /// Core-derived execution time, strictly before the original exclusive expiry.
    pub completed_at_unix_ms: u64,
}

/// Explicit terminalization without deleting the operation-ID tombstone.
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
    name = "iroha_data_model::sorafs::release_manifest_authority::ReleaseManifestExpireV1"
)]
#[norito(deny_unknown_fields)]
pub struct ReleaseManifestExpireV1 {
    /// Original admitted ID; no future reservation may reuse it.
    #[norito(json = "crate::json_helpers::fixed_bytes")]
    pub operation_id: [u8; 32],
    /// Exact original reservation to expire.
    pub reservation: SignerOperationReservationV1,
}

/// Claimed immutable operation outcome; the native reader must authenticate the retained row.
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
    name = "iroha_data_model::sorafs::release_manifest_authority::ReleaseManifestOutcomeV1"
)]
#[norito(
    tag = "state",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum ReleaseManifestOutcomeV1 {
    /// Exclusive slot remains open, with no signature released.
    #[codec(index = 0)]
    Reserved,
    /// Original operation completed timely and exactly once.
    #[codec(index = 1)]
    Completed(ReleaseManifestCompleteV1),
    /// Original slot expired or was invalidated; its ID stays spent.
    #[codec(index = 2)]
    Expired,
}

/// Claimed native indexed row, never proof of its own execution or finality.
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
    name = "iroha_data_model::sorafs::release_manifest_authority::ReleaseManifestOperationV1"
)]
#[norito(deny_unknown_fields)]
pub struct ReleaseManifestOperationV1 {
    /// Original request, purpose and audit predecessor.
    pub reviewed: ReleaseManifestReserveV1,
    /// Original exclusive reservation.
    pub reservation: SignerOperationReservationV1,
    /// Current immutable outcome.
    pub outcome: ReleaseManifestOutcomeV1,
}

/// Actual deterministic role-13 custody execution, never a submitted finality assertion.
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
    name = "iroha_data_model::sorafs::release_manifest_authority::ReleaseManifestExecutionV1"
)]
#[norito(deny_unknown_fields)]
pub struct ReleaseManifestExecutionV1 {
    /// Actual executing block height.
    pub height: u64,
    /// Zero-based transition ordinal within this deployment's custody history and block.
    pub ordinal: u32,
    /// Executing block's logical timestamp in Unix milliseconds.
    pub recorded_at_unix_ms: u64,
    /// Registered universal account that submitted this custody transition.
    pub authority: AccountId,
}

/// Immutable role-13 custody transition, independent of operation and audit progress.
///
/// Native storage also retains permanent first-use indexes for signer and attester keys.
/// A decoded record alone proves neither successful execution nor consensus finality.
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
    name = "iroha_data_model::sorafs::release_manifest_authority::ReleaseManifestCustodyRecordV1"
)]
#[norito(deny_unknown_fields)]
pub struct ReleaseManifestCustodyRecordV1 {
    /// Stable role-13 deployment identity across key and policy rotation.
    pub deployment_id: String,
    /// Strictly one-based custody revision.
    pub revision: u64,
    /// Previous role-13 custody digest; zero only at initial configuration.
    #[norito(json = "crate::json_helpers::fixed_bytes")]
    pub predecessor_digest: [u8; 32],
    /// Canonical mutation and submitting authority commitment.
    #[norito(json = "crate::json_helpers::fixed_bytes")]
    pub request_digest: [u8; 32],
    /// Native execution coordinates, derived by Core from the actual transaction.
    pub execution: ReleaseManifestExecutionV1,
    /// Canonical Manifest `SignerCustodyControlStateV1` frame.
    #[norito(json = "crate::json_helpers::base64_vec")]
    pub control_state: Vec<u8>,
    /// Exact admitted signed custody frame, cleared by reconfiguration.
    pub enrollment: Option<Vec<u8>>,
}

/// Independently retained finalized floor requested for a fresh Check.
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
    name = "iroha_data_model::sorafs::release_manifest_authority::ReleaseManifestFloorV1"
)]
#[norito(deny_unknown_fields)]
pub struct ReleaseManifestFloorV1 {
    /// Positive finalized block height strictly before the Check execution.
    pub height: u64,
    /// Exact native block hash at that independently retained height.
    #[norito(json = "crate::json_helpers::fixed_bytes")]
    pub block_hash: [u8; 32],
}

/// Closed phase set; every invocation requires a separately retained fresh challenge.
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
    name = "iroha_data_model::sorafs::release_manifest_authority::ReleaseManifestCheckPhaseV1"
)]
#[norito(
    tag = "phase",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum ReleaseManifestCheckPhaseV1 {
    /// Current eligible custody and exact audit predecessor, before reservation.
    #[codec(index = 0)]
    Current(SignerOperationAuditHeadV1),
    /// Exact unexpired original slot before provider I/O.
    #[codec(index = 1)]
    BeforeProvider(ReleaseManifestOperationV1),
    /// Exact unexpired original slot after provider I/O.
    #[codec(index = 2)]
    AfterProvider(ReleaseManifestOperationV1),
    /// Exact unexpired original slot before the completion CAS.
    #[codec(index = 3)]
    BeforeCommit(ReleaseManifestOperationV1),
    /// Exact timely completed row after CAS or at recovery entry.
    #[codec(index = 4)]
    AfterCommit(ReleaseManifestOperationV1),
    /// Exact timely completed row before private signature release.
    #[codec(index = 5)]
    BeforeRelease(ReleaseManifestOperationV1),
}

/// No-write Check claim; only a native exact transaction/result reader can authenticate it.
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
    name = "iroha_data_model::sorafs::release_manifest_authority::ReleaseManifestCheckV1"
)]
#[norito(deny_unknown_fields)]
pub struct ReleaseManifestCheckV1 {
    /// Fresh consumer challenge, retained privately before transaction submission.
    #[norito(json = "crate::json_helpers::fixed_bytes")]
    pub challenge: [u8; 32],
    /// Independently pinned genesis-derived native network identity.
    #[norito(json = "crate::json_helpers::fixed_bytes")]
    pub network_id: [u8; 32],
    /// Registered operator whose exact role-13 permission Core must check at execution and use.
    pub expected_operator: AccountId,
    /// Independently retained finalized floor.
    pub floor: ReleaseManifestFloorV1,
    /// Exact reviewed raw-manifest request and original audit predecessor in every phase.
    pub reviewed: ReleaseManifestReserveV1,
    /// Current audit or original immutable indexed row for the selected phase.
    pub phase: ReleaseManifestCheckPhaseV1,
}

/// Closed native action surface; its registered instruction has no production execution path.
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
    name = "iroha_data_model::sorafs::release_manifest_authority::ReleaseManifestActionV1"
)]
#[norito(
    tag = "action",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum ReleaseManifestActionV1 {
    /// Canonical role-13 custody policy, admitted only by future native permission checks.
    #[codec(index = 0)]
    Configure(Vec<u8>),
    /// Canonical independently attested enrollment under the committed predecessor.
    #[codec(index = 1)]
    Enroll(Vec<u8>),
    /// Governed emergency revocation invalidates the active operation.
    #[codec(index = 2)]
    Revoke(ReleaseManifestRevocationV1),
    /// Reserve one exact original operation before key I/O.
    #[codec(index = 3)]
    Reserve(ReleaseManifestReserveV1),
    /// Commit one original reservation and staged signature digest.
    #[codec(index = 4)]
    Complete(ReleaseManifestCompleteV1),
    /// Terminalize without advancing audit or erasing the operation ID.
    #[codec(index = 5)]
    Expire(ReleaseManifestExpireV1),
    /// Evaluate a fresh native ordered predicate without mutating authority history.
    #[codec(index = 6)]
    Check(ReleaseManifestCheckV1),
}

/// Failure of claim-shape checks only; no variant represents native authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReleaseManifestClaimErrorV1 {
    /// Malformed or over-limit canonical frame.
    Encoding,
    /// Inert, mismatched or wrongly purposed reviewed request and intent.
    Review,
    /// Reservation, immutable completion, or audit successor differs.
    Operation,
    /// Phase does not match the claimed original operation outcome.
    Phase,
    /// Fresh consumer challenge, network, operator or finalized floor differs.
    Round,
}
impl fmt::Display for ReleaseManifestClaimErrorV1 {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(match self {
            Self::Encoding => "invalid release-manifest action encoding",
            Self::Review => "invalid release-manifest reviewed request",
            Self::Operation => "invalid release-manifest operation claim",
            Self::Phase => "invalid release-manifest check phase",
            Self::Round => "release-manifest check round mismatch",
        })
    }
}
impl std::error::Error for ReleaseManifestClaimErrorV1 {}

/// Decode one bounded canonical action frame, without granting native admission.
///
/// # Errors
/// Rejects empty, oversized, truncated or noncanonical Norito frames before claim inspection.
pub fn decode_release_manifest_action_claim_v1(
    frame: &[u8],
) -> Result<ReleaseManifestActionV1, ReleaseManifestClaimErrorV1> {
    if frame.is_empty() || frame.len() > RELEASE_MANIFEST_ACTION_MAX_BYTES_V1 {
        return Err(ReleaseManifestClaimErrorV1::Encoding);
    }
    norito::decode_canonical(frame).map_err(|_| ReleaseManifestClaimErrorV1::Encoding)
}

/// Check an untrusted claim against the independently constructed exact reviewed request.
///
/// The caller must construct `expected_request` from verified active role-13 custody and the
/// independently reviewed original manifest bytes; `expected_audit` must come from the same
/// authenticated state snapshot. This function checks neither provenance nor execution.
///
/// # Errors
/// Rejects purpose, request, operation identity, digest, predecessor or bounds mismatch.
pub fn validate_release_manifest_reserve_claim_v1(
    reviewed: &ReleaseManifestReserveV1,
    expected_request: &SignerReleaseManifestRequestV1,
    expected_audit: SignerOperationAuditHeadV1,
) -> Result<(), ReleaseManifestClaimErrorV1> {
    let request = &reviewed.request;
    if request != expected_request
        || request.operation_id == [0; 32]
        || request.binding_digest == [0; 32]
        || request.original_custody.record_digest == [0; 32]
        || request.original_custody.control_state_digest == [0; 32]
        || request.manifest_digest == [0; 32]
        || request.manifest_size == 0
        || request.manifest_size > SIGNER_RELEASE_MANIFEST_MAX_BYTES_V1 as u64
        || reviewed.intent.action != SignerOperationActionV1::Sign
        || reviewed.intent.operation_id != request.operation_id
        || reviewed.intent.request_digest
            != request
                .digest()
                .map_err(|_| ReleaseManifestClaimErrorV1::Review)?
        || reviewed.intent.previous_audit != expected_audit
        || reviewed.intent.digest().is_err()
    {
        return Err(ReleaseManifestClaimErrorV1::Review);
    }
    Ok(())
}

/// Check a claimed completion against an independently retained original Reserved row.
///
/// Core must derive `completed_at_unix_ms`, compare the indexed original, and atomically publish
/// the next audit and permanent tombstone. This function alone cannot establish any of those.
///
/// # Errors
/// Rejects altered original request/reservation, malformed fence or a non-successor audit. The
/// caller must derive completion time from Core execution; this checks only the supplied claim.
pub fn validate_release_manifest_complete_claim_v1(
    completion: &ReleaseManifestCompleteV1,
    original: &ReleaseManifestOperationV1,
) -> Result<(), ReleaseManifestClaimErrorV1> {
    let previous = completion.reviewed.intent.previous_audit;
    let reservation = completion.reservation;
    if original.outcome != ReleaseManifestOutcomeV1::Reserved
        || completion.reviewed != original.reviewed
        || reservation != original.reservation
        || reservation.reservation_id == [0; 32]
        || reservation.fence == 0
        || reservation.expires_at_unix_ms == 0
        || completion.completed_at_unix_ms >= reservation.expires_at_unix_ms
        || completion.commitment.audit.sequence
            != previous
                .sequence
                .checked_add(1)
                .ok_or(ReleaseManifestClaimErrorV1::Operation)?
        || completion.commitment.audit.digest == [0; 32]
        || completion.commitment.response_digest == [0; 32]
        || completion.signatures_digest == [0; 32]
    {
        return Err(ReleaseManifestClaimErrorV1::Operation);
    }
    Ok(())
}

/// Check the untrusted Check claim against privately retained round inputs and phase shape.
///
/// A successful return is only preflight: Core must execute the exact signed transaction, read
/// its ordered result from the same finalized State, authenticate retained rows and verify the
/// independent floor before the daemon treats the observation as authority.
///
/// # Errors
/// Rejects a challenge that differs from the caller's privately retained round input, changed
/// review/floor/operator, or a forged outcome phase. The caller must itself create, retain and
/// retire each fresh challenge; equality here cannot establish freshness or detect its reuse.
pub fn validate_release_manifest_check_claim_v1(
    check: &ReleaseManifestCheckV1,
    expected_challenge: [u8; 32],
    expected_network: [u8; 32],
    expected_operator: &AccountId,
    expected_floor: ReleaseManifestFloorV1,
    expected_request: &SignerReleaseManifestRequestV1,
    expected_audit: SignerOperationAuditHeadV1,
) -> Result<(), ReleaseManifestClaimErrorV1> {
    if expected_challenge == [0; 32]
        || expected_network == [0; 32]
        || expected_floor.height == 0
        || expected_floor.block_hash == [0; 32]
        || check.challenge != expected_challenge
        || check.network_id != expected_network
        || check.expected_operator != *expected_operator
        || check.floor != expected_floor
    {
        return Err(ReleaseManifestClaimErrorV1::Round);
    }
    validate_release_manifest_reserve_claim_v1(&check.reviewed, expected_request, expected_audit)?;
    match check.phase {
        ReleaseManifestCheckPhaseV1::Current(audit) => {
            if audit != expected_audit {
                return Err(ReleaseManifestClaimErrorV1::Phase);
            }
        }
        ReleaseManifestCheckPhaseV1::BeforeProvider(row)
        | ReleaseManifestCheckPhaseV1::AfterProvider(row)
        | ReleaseManifestCheckPhaseV1::BeforeCommit(row) => {
            if row.reviewed != check.reviewed
                || row.outcome != ReleaseManifestOutcomeV1::Reserved
                || row.reservation.reservation_id == [0; 32]
                || row.reservation.fence == 0
                || row.reservation.expires_at_unix_ms == 0
            {
                return Err(ReleaseManifestClaimErrorV1::Phase);
            }
        }
        ReleaseManifestCheckPhaseV1::AfterCommit(row)
        | ReleaseManifestCheckPhaseV1::BeforeRelease(row) => {
            if row.reviewed != check.reviewed {
                return Err(ReleaseManifestClaimErrorV1::Phase);
            }
            let ReleaseManifestOutcomeV1::Completed(completion) = row.outcome else {
                return Err(ReleaseManifestClaimErrorV1::Phase);
            };
            let original = ReleaseManifestOperationV1 {
                outcome: ReleaseManifestOutcomeV1::Reserved,
                ..row
            };
            validate_release_manifest_complete_claim_v1(&completion, &original)
                .map_err(|_| ReleaseManifestClaimErrorV1::Phase)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
