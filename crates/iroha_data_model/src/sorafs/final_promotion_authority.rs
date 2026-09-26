//! Deployment-scoped native authority for final promotion custody and signing operations.
//!
//! Custody and operation histories have separate commitments. Native execution supplies all
//! provenance; completion transactions carry commitments, never unreleased role signatures.
use crate::{DeriveJsonDeserialize, DeriveJsonSerialize, account::AccountId};
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
use sorafs_manifest::signer::{
    final_promotion::SignerFinalPromotionRequestV1,
    protocol::{
        SignerOperationAuditHeadV1, SignerOperationCommitmentV1, SignerOperationCustodyV1,
        SignerOperationIntentV1, SignerOperationReservationV1,
    },
};

/// Total retained custody revisions, including two emergency revocations.
pub const FINAL_PROMOTION_CUSTODY_MAX_REVISIONS_V1: u64 = 8_194;
/// Normal custody revision ceiling, leaving room to revoke both key generations.
pub const FINAL_PROMOTION_CUSTODY_NORMAL_REVISIONS_V1: u64 = 8_192;
/// Permanent operation-ID capacity; exhaustion fails closed instead of deleting tombstones.
pub const FINAL_PROMOTION_MAX_OPERATIONS_V1: u64 = 65_536;
/// Maximum exclusive reservation lifetime, capped further by current custody eligibility.
pub const FINAL_PROMOTION_RESERVATION_MS_V1: u64 = 60_000;
/// Maximum complete canonical native record, including Norito framing.
pub const FINAL_PROMOTION_MAX_RECORD_BYTES_V1: usize = 32 * 1024;
/// Native custody commitment domain, independent of operation/audit progress.
pub const FINAL_PROMOTION_CUSTODY_RECORD_DOMAIN_V1: &[u8] =
    b"iroha.sorafs.final-promotion.custody-control.v1\0";
/// Native immutable operation commitment domain.
pub const FINAL_PROMOTION_OPERATION_RECORD_DOMAIN_V1: &[u8] =
    b"iroha.sorafs.final-promotion.operation.v1\0";

/// Monotonic current-generation revocation, independent of the old signing device.
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
    name = "iroha_data_model::sorafs::final_promotion_authority::FinalPromotionRevocationV1"
)]
#[norito(deny_unknown_fields)]
pub struct FinalPromotionRevocationV1 {
    /// Revoke the current signer key generation.
    pub signer: bool,
    /// Revoke the current independent attester generation.
    pub attester: bool,
}

/// Request an exclusive slot under the exact currently enrolled custody and audit head.
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
    name = "iroha_data_model::sorafs::final_promotion_authority::FinalPromotionReserveV1"
)]
#[norito(deny_unknown_fields)]
pub struct FinalPromotionReserveV1 {
    /// Exact validated service request, action, operation ID and audit predecessor.
    pub intent: SignerOperationIntentV1,
    /// Independently qualified custody/control identity used by the original request.
    pub custody: SignerOperationCustodyV1,
}

/// Complete one original reservation without publishing its unreleased signatures.
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
    name = "iroha_data_model::sorafs::final_promotion_authority::FinalPromotionCompleteV1"
)]
#[norito(deny_unknown_fields)]
pub struct FinalPromotionCompleteV1 {
    /// Exact original intent, including its independently expected predecessor.
    pub intent: SignerOperationIntentV1,
    /// Original custody/control identity; renewal cannot substitute a new identity.
    pub custody: SignerOperationCustodyV1,
    /// Exact native reservation, fence and exclusive expiry.
    pub reservation: SignerOperationReservationV1,
    /// Durable staged response and exactly one next audit record.
    pub commitment: SignerOperationCommitmentV1,
    /// Commitment to every ordered staged signature; signatures stay private until finality.
    #[norito(json = "crate::json_helpers::fixed_bytes")]
    pub signatures_digest: [u8; 32],
}

/// Terminalize the exact expired slot while preserving its operation-ID tombstone.
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
    name = "iroha_data_model::sorafs::final_promotion_authority::FinalPromotionExpireV1"
)]
#[norito(deny_unknown_fields)]
pub struct FinalPromotionExpireV1 {
    /// Original operation identity; it can never be reserved again.
    #[norito(json = "crate::json_helpers::fixed_bytes")]
    pub operation_id: [u8; 32],
    /// Exact slot to expire; elapsed time alone cannot select a different reservation.
    pub reservation: SignerOperationReservationV1,
}

/// Sole native mutation surface; custody management and operation rights are separate.
#[expect(
    clippy::large_enum_variant,
    reason = "role-15 Check and Complete stay inline in the signed canonical V1 action; boxing changes Norito and schema"
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
    name = "iroha_data_model::sorafs::final_promotion_authority::FinalPromotionAuthorityActionV1"
)]
#[norito(
    tag = "action",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum FinalPromotionAuthorityActionV1 {
    /// Canonical Manifest `SignerCustodyPolicyV1`, bound to role 14 and this deployment.
    #[codec(index = 0)]
    Configure(Vec<u8>),
    /// Independently signed canonical `SignerCustodyRecordV1` for the committed predecessor.
    #[codec(index = 1)]
    Enroll(Vec<u8>),
    /// Governed emergency revocation needs no old-key signature and invalidates a pending slot.
    #[codec(index = 2)]
    Revoke(FinalPromotionRevocationV1),
    /// Allocate a durable exclusive reservation before provider I/O.
    #[codec(index = 3)]
    Reserve(FinalPromotionReserveV1),
    /// Commit before the original exclusive expiry and advance the audit exactly once.
    #[codec(index = 4)]
    Complete(FinalPromotionCompleteV1),
    /// Release an expired slot without erasing its identity or advancing the audit.
    #[codec(index = 5)]
    Expire(FinalPromotionExpireV1),
    /// Evaluate fresh scoped eligibility without changing authority history or consuming an ID.
    #[codec(index = 6)]
    Check(FinalPromotionCheckV1),
}

/// One challenged eligibility predicate ordered through the native transaction path.
///
/// These are public claims, not a verified observation. A consumer independently retains the
/// challenge, complete signed transaction, network and finalized floor before submission.
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
    name = "iroha_data_model::sorafs::final_promotion_authority::FinalPromotionCheckV1"
)]
#[norito(deny_unknown_fields)]
pub struct FinalPromotionCheckV1 {
    /// Fresh unpredictable 256-bit consumer challenge, retired when its local attempt ends.
    #[norito(json = "crate::json_helpers::fixed_bytes")]
    pub challenge: [u8; 32],
    /// Independently expected genesis-derived native network identity.
    #[norito(json = "crate::json_helpers::fixed_bytes")]
    pub network_id: [u8; 32],
    /// Independently expected registered operator, distinct from the Check observer.
    /// The operator must retain the exact deployment operation permission at execution and use.
    pub expected_operator: AccountId,
    /// Independently retained positive finalized floor, preceding this check's execution.
    pub minimum_height: u64,
    /// Exact native block hash at the independently retained floor.
    #[norito(json = "crate::json_helpers::fixed_bytes")]
    pub minimum_block_hash: [u8; 32],
    /// Existing canonical request binding reviewed bytes, operation and original custody.
    pub request: SignerFinalPromotionRequestV1,
    /// Exact observation phase and its native audit or original operation coordinates.
    pub subject: FinalPromotionCheckSubjectV1,
}

/// Closed eligibility phases; operation phases retain the exact existing native record.
///
/// A fresh challenge is required for every invocation, including repeated provider phases.
/// Completed phases permit later audit progress while retaining the original completion.
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
    name = "iroha_data_model::sorafs::final_promotion_authority::FinalPromotionCheckSubjectV1"
)]
#[norito(
    tag = "phase",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum FinalPromotionCheckSubjectV1 {
    /// Current eligible custody and exact audit head; grants no reservation ownership.
    #[codec(index = 0)]
    Current(SignerOperationAuditHeadV1),
    /// Exact exclusive unexpired Reserved row immediately before provider work.
    #[codec(index = 1)]
    BeforeProvider(FinalPromotionOperationRecordV1),
    /// Exact exclusive unexpired Reserved row immediately after provider work.
    #[codec(index = 2)]
    AfterProvider(FinalPromotionOperationRecordV1),
    /// Exact exclusive unexpired Reserved row immediately before completion CAS.
    #[codec(index = 3)]
    BeforeCommit(FinalPromotionOperationRecordV1),
    /// Exact timely Completed row after completion CAS or at recovery entry.
    #[codec(index = 4)]
    AfterCommit(FinalPromotionOperationRecordV1),
    /// Exact timely Completed row immediately before releasing the private receipt.
    #[codec(index = 5)]
    BeforeRelease(FinalPromotionOperationRecordV1),
}

/// Deterministic native execution provenance, with no self-referential current block hash.
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
    name = "iroha_data_model::sorafs::final_promotion_authority::FinalPromotionExecutionV1"
)]
#[norito(deny_unknown_fields)]
pub struct FinalPromotionExecutionV1 {
    /// Actual executing block height.
    pub height: u64,
    /// Zero-based transition ordinal within this deployment's corresponding history and block.
    pub ordinal: u32,
    /// Actual execution block timestamp in Unix milliseconds.
    pub recorded_at_unix_ms: u64,
    /// Registered universal transaction authority holding the exact deployment permission.
    pub authority: AccountId,
}

/// Immutable custody transition; operation changes never alter this record's digest.
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
    name = "iroha_data_model::sorafs::final_promotion_authority::FinalPromotionCustodyRecordV1"
)]
#[norito(deny_unknown_fields)]
pub struct FinalPromotionCustodyRecordV1 {
    /// Stable deployment identity, retained across all key/policy changes.
    pub deployment_id: String,
    /// Strictly one-based custody revision.
    pub revision: u64,
    /// Previous custody record digest; zero only before first configuration.
    #[norito(json = "crate::json_helpers::fixed_bytes")]
    pub predecessor_digest: [u8; 32],
    /// Exact canonical mutation and original authority commitment.
    #[norito(json = "crate::json_helpers::fixed_bytes")]
    pub request_digest: [u8; 32],
    /// Native execution metadata.
    pub execution: FinalPromotionExecutionV1,
    /// Canonical Manifest `SignerCustodyControlStateV1` frame.
    #[norito(json = "crate::json_helpers::base64_vec")]
    pub control_state: Vec<u8>,
    /// Exact admitted signed custody frame, cleared by configuration and retained by revocation.
    pub enrollment: Option<Vec<u8>>,
}

/// Immutable completion commitments, carrying no unreleased signatures.
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
    name = "iroha_data_model::sorafs::final_promotion_authority::FinalPromotionCompletedV1"
)]
#[norito(deny_unknown_fields)]
pub struct FinalPromotionCompletedV1 {
    /// Exact next audit record and response digest.
    pub commitment: SignerOperationCommitmentV1,
    /// Commitment to ordered signatures retained privately before release.
    #[norito(json = "crate::json_helpers::fixed_bytes")]
    pub signatures_digest: [u8; 32],
}

/// Permanent state of one signing operation; terminal entries never become reservations again.
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
    name = "iroha_data_model::sorafs::final_promotion_authority::FinalPromotionOperationOutcomeV1"
)]
#[norito(
    tag = "state",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum FinalPromotionOperationOutcomeV1 {
    /// Exclusively reserved, not yet completed or terminalized.
    #[codec(index = 0)]
    Reserved,
    /// Immutable timely completion; finalized readers attach its block hash afterward.
    #[codec(index = 1)]
    Completed(FinalPromotionCompletedV1),
    /// Expired without completion; retains replay protection.
    #[codec(index = 2)]
    Expired,
    /// Custody changed or was revoked; retains replay protection without a completion claim.
    #[codec(index = 3)]
    Invalidated,
}

/// Direct signed Network entry that caused one native role-14 operation revision.
///
/// The hash names the entrypoint payload. The index distinguishes the exact entry inside its
/// finalized block; consumers must also authenticate the complete signed External bytes and
/// aligned successful result. This is never derived from a caller-supplied operation claim.
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
    name = "iroha_data_model::sorafs::final_promotion_authority::FinalPromotionOperationOriginV1"
)]
#[norito(deny_unknown_fields)]
pub struct FinalPromotionOperationOriginV1 {
    /// Hash of the exact outer signed Network entrypoint payload.
    #[norito(json = "crate::json_helpers::fixed_bytes")]
    pub entry_hash: [u8; 32],
    /// Zero-based position of that entrypoint in its canonical block.
    pub entry_index: u32,
}

/// Immutable operation transition under a deployment-wide history and never-reset fence.
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
    name = "iroha_data_model::sorafs::final_promotion_authority::FinalPromotionOperationRecordV1"
)]
#[norito(deny_unknown_fields)]
pub struct FinalPromotionOperationRecordV1 {
    /// Stable role-14 deployment scope.
    pub deployment_id: String,
    /// One-based global operation-history revision, independent of custody revisions.
    pub revision: u64,
    /// Previous global operation-history digest; zero only at revision one.
    #[norito(json = "crate::json_helpers::fixed_bytes")]
    pub predecessor_digest: [u8; 32],
    /// Canonical transition and actual submitting authority commitment.
    #[norito(json = "crate::json_helpers::fixed_bytes")]
    pub request_digest: [u8; 32],
    /// Native execution of this immutable transition.
    pub execution: FinalPromotionExecutionV1,
    /// Direct signed source of this transition for Reserve/Complete; terminal control actions
    /// without a protected account signature carry no source claim.
    #[norito(required)]
    pub execution_origin: Option<FinalPromotionOperationOriginV1>,
    /// Exact original request/action/audit predecessor.
    pub intent: SignerOperationIntentV1,
    /// Exact original custody/control identity.
    pub custody: SignerOperationCustodyV1,
    /// Original exclusive reservation retained through every terminal outcome.
    pub reservation: SignerOperationReservationV1,
    /// Original reservation execution and authority; custody managers cannot replace its owner.
    pub reserved: FinalPromotionExecutionV1,
    /// Direct signed source that allocated the original reservation, retained across all endings.
    pub reserved_origin: FinalPromotionOperationOriginV1,
    /// Current outcome committed by this transition.
    pub outcome: FinalPromotionOperationOutcomeV1,
}

#[cfg(test)]
mod tests;
