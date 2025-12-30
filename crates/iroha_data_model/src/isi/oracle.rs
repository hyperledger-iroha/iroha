use iroha_primitives::numeric::Numeric;

use super::*;
use crate::{
    oracle::{
        FeedConfig, FeedId, FeedSlot, KeyedHash, Observation, OracleChangeClass, OracleChangeId,
        OracleChangeStage, OracleDisputeId, OracleDisputeOutcome, OracleId,
        TwitterBindingAttestation,
    },
    prelude::Hash,
};

isi! {
    /// Register an oracle feed configuration on-chain.
    pub struct RegisterOracleFeed {
        /// Feed configuration to make available for oracle submissions.
        pub feed: FeedConfig,
    }
}

isi! {
    /// Submit a signed oracle observation for admission.
    pub struct SubmitOracleObservation {
        /// Observation payload and signature produced by the oracle provider.
        pub observation: Observation,
    }
}

isi! {
    /// Aggregate admitted observations for a feed slot into a feed event.
    pub struct AggregateOracleFeed {
        /// Target feed identifier.
        pub feed_id: FeedId,
        /// Slot index being aggregated.
        pub slot: FeedSlot,
        /// Canonical request hash for the aggregation window.
        pub request_hash: Hash,
        /// Optional hashes of external evidence (e.g., `SoraFS` bundles).
        #[cfg_attr(feature = "json", norito(default))]
        pub evidence_hashes: Vec<Hash>,
    }
}

isi! {
    /// Open a dispute against an oracle provider for a specific feed slot.
    pub struct OpenOracleDispute {
        /// Feed identifier for the disputed observation window.
        pub feed_id: FeedId,
        /// Slot index being challenged.
        pub slot: FeedSlot,
        /// Canonical request hash for the disputed window.
        pub request_hash: Hash,
        /// Provider being challenged.
        pub target: OracleId,
        /// Optional bond override (falls back to config when `None`).
        #[cfg_attr(feature = "json", norito(default))]
        pub bond: Option<Numeric>,
        /// Evidence hashes supplied by the challenger.
        #[cfg_attr(feature = "json", norito(default))]
        pub evidence_hashes: Vec<Hash>,
        /// Human-readable reason for the dispute.
        pub reason: String,
    }
}

isi! {
    /// Resolve an open oracle dispute.
    pub struct ResolveOracleDispute {
        /// Identifier of the dispute being resolved.
        pub dispute_id: OracleDisputeId,
        /// Outcome applied to the dispute.
        pub outcome: OracleDisputeOutcome,
        /// Optional operator notes.
        #[cfg_attr(feature = "json", norito(default))]
        pub notes: String,
    }
}

isi! {
    /// Propose a governance change for an oracle feed.
    pub struct ProposeOracleChange {
        /// Unique identifier for the change proposal.
        pub change_id: OracleChangeId,
        /// Feed configuration that will be enacted after approval.
        pub feed: FeedConfig,
        /// Change classification driving quorum thresholds.
        pub class: OracleChangeClass,
        /// Hash of the change manifest or external artefact.
        pub payload_hash: Hash,
        /// Optional evidence bundle hashes attached at intake.
        #[cfg_attr(feature = "json", norito(default))]
        pub evidence_hashes: Vec<Hash>,
    }
}

isi! {
    /// Record a stage vote for an oracle change proposal.
    pub struct VoteOracleChangeStage {
        /// Identifier of the change proposal being reviewed.
        pub change_id: OracleChangeId,
        /// Stage receiving the vote.
        pub stage: OracleChangeStage,
        /// Whether this vote approves (`true`) or rejects (`false`) the stage.
        pub approve: bool,
        /// Optional evidence hashes linked to this vote.
        #[cfg_attr(feature = "json", norito(default))]
        pub evidence_hashes: Vec<Hash>,
    }
}

isi! {
    /// Explicitly roll back an oracle change proposal.
    pub struct RollbackOracleChange {
        /// Identifier of the change being rolled back.
        pub change_id: OracleChangeId,
        /// Optional stage to tag the rollback against (defaults to the active stage).
        #[cfg_attr(feature = "json", norito(default))]
        pub stage: Option<OracleChangeStage>,
        /// Human-readable reason for the rollback.
        pub reason: String,
    }
}

isi! {
    /// Record a twitter follow binding attestation.
    pub struct RecordTwitterBinding {
        /// Attestation payload produced by the oracle committee.
        pub attestation: TwitterBindingAttestation,
        /// Feed identifier expected for the attestation (e.g., `twitter_follow_binding`).
        pub feed_id: FeedId,
    }
}

isi! {
    /// Revoke a twitter follow binding record.
    pub struct RevokeTwitterBinding {
        /// Binding keyed hash to revoke.
        pub binding_hash: KeyedHash,
        /// Human-readable reason for the revocation.
        pub reason: String,
    }
}

impl crate::seal::Instruction for RegisterOracleFeed {}
impl crate::seal::Instruction for SubmitOracleObservation {}
impl crate::seal::Instruction for AggregateOracleFeed {}
impl crate::seal::Instruction for OpenOracleDispute {}
impl crate::seal::Instruction for ResolveOracleDispute {}
impl crate::seal::Instruction for ProposeOracleChange {}
impl crate::seal::Instruction for VoteOracleChangeStage {}
impl crate::seal::Instruction for RollbackOracleChange {}
impl crate::seal::Instruction for RecordTwitterBinding {}
impl crate::seal::Instruction for RevokeTwitterBinding {}
