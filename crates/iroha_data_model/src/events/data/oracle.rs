//! Oracle data events for feed aggregation outcomes.

use super::*;
use crate::{
    oracle::{
        FeedEvent, FeedId, KeyedHash, OracleChangeClass, OracleChangeId, OracleChangeStage,
        OracleChangeStatus, OracleDispute, OraclePenalty, OracleReward, TwitterBindingRecord,
    },
    prelude::{AccountId, Hash},
};

/// Record emitted after aggregating a feed slot.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct FeedEventRecord {
    /// Feed event describing the aggregation outcome.
    pub event: FeedEvent,
    /// Optional hashes of external evidence (e.g., `SoraFS` bundles).
    #[cfg_attr(feature = "json", norito(default))]
    pub evidence_hashes: Vec<Hash>,
}

/// Event emitted when a twitter binding attestation is recorded.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct TwitterBindingRecorded {
    /// Recorded binding.
    pub record: TwitterBindingRecord,
}

/// Event emitted when a twitter binding is revoked.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct TwitterBindingRevoked {
    /// Binding hash that was revoked.
    pub binding_hash: KeyedHash,
    /// Account that issued the revocation.
    pub revoked_by: AccountId,
    /// Human-readable reason for revocation.
    pub reason: String,
    /// Timestamp (milliseconds) when the revocation was recorded.
    pub recorded_at_ms: u64,
}

/// Event emitted when an oracle change proposal is registered.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct OracleChangeProposed {
    /// Unique identifier of the change.
    pub change_id: OracleChangeId,
    /// Feed identifier targeted by the change.
    pub feed_id: FeedId,
    /// Classification of the change (drives quorum).
    pub class: OracleChangeClass,
    /// Hash of the manifest or evidence bundle.
    pub payload_hash: Hash,
    /// Account that submitted the proposal.
    pub proposer: AccountId,
}

/// Event emitted whenever a pipeline stage records votes or reaches a terminal state.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct OracleChangeStageUpdated {
    /// Identifier of the change.
    pub change_id: OracleChangeId,
    /// Stage receiving the update.
    pub stage: OracleChangeStage,
    /// Current lifecycle status for the change.
    pub status: OracleChangeStatus,
    /// Count of approvals recorded for the stage.
    pub approvals: u32,
    /// Count of rejections recorded for the stage.
    pub rejections: u32,
    /// Evidence hashes attached during this update (if any).
    #[cfg_attr(feature = "json", norito(default))]
    pub evidence_hashes: Vec<Hash>,
}

/// Oracle lifecycle events.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Decode,
    Encode,
    IntoSchema,
    iroha_data_model_derive::EventSet,
)]
#[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
#[cfg_attr(feature = "json", norito(tag = "event", content = "payload"))]
pub enum OracleEvent {
    /// Feed slot aggregated with an outcome.
    FeedProcessed(FeedEventRecord),
    /// Penalty applied to an oracle provider.
    PenaltyApplied(OraclePenalty),
    /// Reward paid to an oracle provider.
    RewardApplied(OracleReward),
    /// Pseudonymous twitter binding recorded.
    TwitterBindingRecorded(TwitterBindingRecorded),
    /// Twitter binding revoked.
    TwitterBindingRevoked(TwitterBindingRevoked),
    /// Dispute opened against an oracle provider.
    DisputeOpened(OracleDispute),
    /// Dispute resolved with the provided status.
    DisputeResolved(OracleDispute),
    /// Oracle change proposal registered.
    ChangeProposed(OracleChangeProposed),
    /// Oracle change stage received a vote or terminal outcome.
    ChangeStageUpdated(OracleChangeStageUpdated),
}

/// Prelude exports.
pub mod prelude {
    pub use super::{
        FeedEventRecord, OracleChangeProposed, OracleChangeStageUpdated, OracleEvent,
        OracleEventSet, TwitterBindingRecorded, TwitterBindingRevoked,
    };
}
