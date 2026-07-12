//! Norito-encoded consensus message types shared across Sumeragi implementations.
//!
//! These types cover QC voting (prepare/commit/new-view), evidence, VRF
//! commit/reveal envelopes, and optional reliable broadcast
//! helpers. They are split out of `iroha_core` so that other crates (e.g.,
//! Torii, genesis tooling, or test harnesses) can construct and inspect
//! consensus payloads without depending on the core runtime crate.

use core::fmt;
use std::{string::String, vec::Vec};

use iroha_crypto::{Hash, HashOf};
use iroha_schema::{EnumMeta, EnumVariant, Ident, IntoSchema, MetaMap, Metadata, TypeId};
use norito::codec::{Decode, DecodeAll, Encode};

use super::{BlockSignature, Header as BlockHeader};
use crate::{
    account::AccountId,
    fastpq::{FastpqTransitionBatch, TransferTranscriptBundle},
    nexus::{DataSpaceId, LaneId, LaneRelayEnvelope},
    peer::PeerId,
    transaction::TransactionSubmissionReceipt,
};
use iroha_primitives::numeric::{Numeric, NumericSpec};

/// Wire protocol version for the legacy Sumeragi v1 archival message family.
///
/// Live consensus rejects this family. New validators use
/// [`super::consensus_v2`] and its explicit v2 envelope.
pub const PROTO_VERSION: u32 = 1;

/// Legacy permissioned-mode tag retained for decoding and archival verification.
pub const PERMISSIONED_TAG: &str = "iroha2-consensus::permissioned-sumeragi@v1";
/// Legacy `NPoS` mode tag retained for decoding and archival verification.
pub const NPOS_TAG: &str = "iroha2-consensus::npos-sumeragi@v1";

/// Chain-order hash used by fixtures that do not model live validator ordering.
///
/// Live consensus code should populate QC votes and certificates with the
/// selected canonical validator-order hash for the active height/view.
#[must_use]
pub fn default_chain_order_hash() -> Hash {
    Hash::new(b"iroha:sumeragi:v1:chain-order:default")
}

/// Height alias for consensus.
pub type Height = u64;
/// View/round number alias.
pub type View = u64;
/// Validator index within the active set.
pub type ValidatorIndex = u32;

/// Stable identifier for the validator set active in a consensus round.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ValidatorSetId {
    /// Hash of the deterministically ordered validator roster.
    pub hash: HashOf<Vec<PeerId>>,
}

impl ValidatorSetId {
    /// Build a validator-set id from a canonical roster.
    #[must_use]
    pub fn from_roster(roster: &[PeerId]) -> Self {
        Self {
            hash: HashOf::new(&roster.to_vec()),
        }
    }
}

/// Consensus height/view/epoch identity under a specific validator set.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct RoundId {
    /// Block height.
    pub height: Height,
    /// Consensus view.
    pub view: View,
    /// Epoch index.
    pub epoch: u64,
    /// Active validator-set identifier.
    pub validator_set_id: ValidatorSetId,
}

/// Quorum policy for the active validator set.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(tag = "kind", content = "policy", rename_all = "snake_case")]
pub enum QuorumPolicy {
    /// Permissioned count quorum over the active validator count.
    PermissionedCount(
        /// Number of validators in the active set.
        u32,
    ),
    /// `NPoS` stake quorum over the active stake snapshot.
    NposStake(
        /// Total stake in the active set.
        Numeric,
    ),
}

impl QuorumPolicy {
    /// Return the strict supermajority threshold for a permissioned validator count.
    #[must_use]
    pub fn permissioned_threshold(validators: u32) -> Option<u32> {
        if validators == 0 {
            return None;
        }
        u32::try_from(u64::from(validators) * 2 / 3 + 1).ok()
    }

    /// Return true when `signed_count` satisfies this policy's count quorum.
    #[must_use]
    pub fn is_satisfied_by_count(&self, signed_count: u32) -> bool {
        let Self::PermissionedCount(validators) = self else {
            return false;
        };
        Self::permissioned_threshold(*validators)
            .is_some_and(|required| signed_count <= *validators && signed_count >= required)
    }

    /// Return true when `signed_stake` strictly exceeds two thirds of total stake.
    ///
    /// Missing signed stake, zero total stake, arithmetic overflow, and exact
    /// two-thirds stake all fail closed.
    #[must_use]
    pub fn is_satisfied_by_stake(&self, signed_stake: Option<Numeric>) -> bool {
        let Self::NposStake(total_stake) = self else {
            return false;
        };
        if total_stake.is_zero() || total_stake.mantissa().is_negative() {
            return false;
        }
        let Some(signed_stake) = signed_stake else {
            return false;
        };
        if signed_stake.mantissa().is_negative() {
            return false;
        }
        if &signed_stake > total_stake {
            return false;
        }
        let Some(signed_scaled) =
            signed_stake.checked_mul(Numeric::from(3_u64), NumericSpec::unconstrained())
        else {
            return false;
        };
        let Some(total_scaled) = total_stake
            .clone()
            .checked_mul(Numeric::from(2_u64), NumericSpec::unconstrained())
        else {
            return false;
        };
        signed_scaled > total_scaled
    }
}

/// Consensus subject identified by parent, block, and payload commitment.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct BlockSubject {
    /// Parent block hash.
    pub parent_block: HashOf<BlockHeader>,
    /// Subject block hash.
    pub block_hash: HashOf<BlockHeader>,
    /// Deterministic payload hash transported by RBC.
    pub payload_hash: Hash,
}

/// Canonical first-release consensus vote.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct Vote {
    /// Voted phase.
    pub phase: CertPhase,
    /// Voted round.
    pub round: RoundId,
    /// Voted block subject.
    pub subject: BlockSubject,
    /// Highest QC bound into `NewView` votes.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub highest_qc: Option<QcRef>,
    /// Signer index within the active validator set.
    pub signer: ValidatorIndex,
    /// BLS signature over the canonical vote preimage.
    pub bls_sig: Vec<u8>,
}

/// Canonical first-release aggregate certificate.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct Certificate {
    /// Certified phase.
    pub phase: CertPhase,
    /// Certified round.
    pub round: RoundId,
    /// Certified block subject.
    pub subject: BlockSubject,
    /// Quorum policy used to validate this certificate.
    pub quorum_policy: QuorumPolicy,
    /// Highest QC carried by a `NewView` certificate.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub highest_qc: Option<QcRef>,
    /// Compact signer bitmap (LSB-first).
    pub signers_bitmap: Vec<u8>,
    /// BLS12-381 aggregate signature bytes (compressed).
    pub bls_aggregate_signature: Vec<u8>,
}

/// Request a missing consensus payload by round and subject hash.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct PayloadRequest {
    /// Requested round.
    pub round: RoundId,
    /// Requested block hash.
    pub block_hash: HashOf<BlockHeader>,
    /// Expected payload hash.
    pub payload_hash: Hash,
}

/// Response carrying a requested consensus payload.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct PayloadResponse {
    /// Request this response satisfies.
    pub request: PayloadRequest,
    /// Canonical payload bytes.
    pub payload: Vec<u8>,
}

/// Canonical consensus parameters included in the genesis fingerprint.
///
/// These parameters are encoded with Norito (binary) in a fixed order to
/// guarantee determinism across peers and platforms.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, Default)]
pub struct ConsensusGenesisParams {
    /// Maximal amount of time a leader waits before proposing (ms).
    pub block_time_ms: u64,
    /// Maximal amount of time to reach commit (ms).
    pub commit_time_ms: u64,
    /// Minimum finality floor enforced for timing (ms).
    pub min_finality_ms: u64,
    /// Allowed clock drift (ms) for transaction admission.
    pub max_clock_drift_ms: u64,
    /// Number of aggregators (collectors) targeted per block.
    pub collectors_k: u16,
    /// Redundant send fanout per validator (distinct collectors).
    pub redundant_send_r: u8,
    /// Block sizing: max transactions per block.
    pub block_max_transactions: u64,
    /// Data availability enabled (RBC transport; consensus does not gate on DA).
    pub da_enabled: bool,
    /// Epoch length in blocks (`NPoS` mode; 0 in permissioned).
    pub epoch_length_blocks: u64,
    /// BLS domain separation string used for QC-vote signatures.
    pub bls_domain: String,
    /// Optional NPoS-specific configuration captured at genesis.
    #[norito(default)]
    pub npos: Option<NposGenesisParams>,
    /// Explicit global consensus protocol revision.
    #[norito(default)]
    pub protocol_version: u32,
    /// One constant, non-resetting round timeout in milliseconds.
    #[norito(default)]
    pub round_timeout_ms: u64,
    /// Required signed inputs for constructing Sumeragi v2 height contexts.
    ///
    /// `None` exists only so archival v1 payloads remain decodable. A live v2
    /// node must reject it rather than deriving values from local config.
    #[norito(default)]
    pub v2_context: Option<super::consensus_v2::SumeragiV2GenesisContextParameters>,
}

/// `NPoS`-specific consensus parameters hashed into the genesis fingerprint.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, Default)]
pub struct NposGenesisParams {
    /// Target block time for `NPoS` mode (ms).
    pub block_time_ms: u64,
    /// Proposal timeout window (ms).
    pub timeout_propose_ms: u64,
    /// Prevote aggregation timeout (ms).
    pub timeout_prevote_ms: u64,
    /// Precommit aggregation timeout (ms).
    pub timeout_precommit_ms: u64,
    /// Commit finalization timeout (ms).
    pub timeout_commit_ms: u64,
    /// Data-availability timeout (ms).
    pub timeout_da_ms: u64,
    /// Aggregator fallback timeout (ms).
    pub timeout_aggregator_ms: u64,
    /// Number of aggregators (K) per round.
    pub k_aggregators: u16,
    /// Redundant send fanout (distinct aggregators contacted over time).
    pub redundant_send_r: u8,
    /// Deterministic epoch seed for PRF-based leader and collector selection.
    pub epoch_seed: [u8; 32],
    /// VRF commit window length in blocks.
    pub vrf_commit_window_blocks: u64,
    /// VRF reveal window length in blocks.
    pub vrf_reveal_window_blocks: u64,
    /// Maximum validators to elect for the next epoch (0 = unlimited).
    pub max_validators: u32,
    /// Minimum self-bond required for validator eligibility.
    pub min_self_bond: u64,
    /// Minimum nomination bond required for delegators.
    pub min_nomination_bond: u64,
    /// Maximum nominator concentration percentage.
    pub max_nominator_concentration_pct: u8,
    /// Seat allocation variance band percentage.
    pub seat_band_pct: u8,
    /// Maximum correlation percentage across validator entities.
    pub max_entity_correlation_pct: u8,
    /// Finality margin in blocks before activating a newly elected set.
    pub finality_margin_blocks: u64,
    /// Evidence retention horizon in blocks.
    pub evidence_horizon_blocks: u64,
    /// Activation lag in blocks for newly scheduled validator sets.
    pub activation_lag_blocks: u64,
    /// Slashing delay in blocks before evidence penalties apply.
    pub slashing_delay_blocks: u64,
}

/// Consensus certificate phases (BLS-only).
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(tag = "phase", content = "detail", rename_all = "snake_case")]
pub enum CertPhase {
    /// Prepare/lock certificate for a proposal.
    Prepare = 1,
    /// Commit QC for finalization.
    Commit = 2,
    /// New-view certificate for view change.
    NewView = 3,
}

impl TypeId for CertPhase {
    fn id() -> Ident {
        "CertPhase".to_owned()
    }
}

impl IntoSchema for CertPhase {
    fn type_name() -> Ident {
        "CertPhase".to_owned()
    }

    fn update_schema_map(metamap: &mut MetaMap) {
        let variants = vec![
            EnumVariant {
                tag: "Prepare".to_owned(),
                discriminant: CertPhase::Prepare as u8,
                ty: None,
            },
            EnumVariant {
                tag: "Commit".to_owned(),
                discriminant: CertPhase::Commit as u8,
                ty: None,
            },
            EnumVariant {
                tag: "NewView".to_owned(),
                discriminant: CertPhase::NewView as u8,
                ty: None,
            },
        ];
        metamap.insert::<Self>(Metadata::Enum(EnumMeta { variants }));
    }
}

/// Reference to an existing QC header for embedding in proposals.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct QcRef {
    /// Height of the certified block.
    pub height: Height,
    /// View in which the certificate was formed.
    pub view: View,
    /// Epoch index (0 in permissioned mode).
    pub epoch: u64,
    /// Block hash certified by the certificate.
    pub subject_block_hash: HashOf<BlockHeader>,
    /// Phase certified by the certificate.
    pub phase: CertPhase,
}

/// Block header fields essential for consensus (proposal header subset).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
pub struct ConsensusBlockHeader {
    /// Parent block hash.
    pub parent_hash: HashOf<BlockHeader>,
    /// Merkle root of included transactions.
    pub tx_root: Hash,
    /// State commitment after executing the block.
    pub state_root: Hash,
    /// Proposer index within the active validator set.
    pub proposer: ValidatorIndex,
    /// Block height.
    pub height: Height,
    /// Consensus view/round number.
    pub view: View,
    /// Epoch index for `NPoS`. Zero in permissioned builds.
    pub epoch: u64,
    /// Embedded reference to the highest QC known to the proposer.
    pub highest_qc: QcRef,
}

/// Proposal message with payload commitment.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
pub struct Proposal {
    /// Proposal header (consensus-relevant subset).
    pub header: ConsensusBlockHeader,
    /// Hash of the full block payload (DA). Used for availability tracking.
    pub payload_hash: Hash,
}

/// QC vote over a specific block and phase (BLS-only).
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct QcVote {
    /// Target phase (`Prepare`, `Commit`, `NewView`).
    pub phase: CertPhase,
    /// Hash of the block being voted on.
    pub block_hash: HashOf<BlockHeader>,
    /// Parent state root bound into the QC vote.
    pub parent_state_root: Hash,
    /// Post-state root bound into the QC vote.
    pub post_state_root: Hash,
    /// Block height of the subject.
    pub height: Height,
    /// View number of the vote.
    pub view: View,
    /// Epoch index for `NPoS`; 0 in permissioned.
    pub epoch: u64,
    /// Hash of the canonical validator ordering this vote is valid under.
    pub chain_order_hash: Hash,
    /// Validator-order update sequence this vote is valid under.
    pub rechain_seq: u64,
    /// Highest known QC for `NewView` votes, bound into the vote signature.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub highest_qc: Option<QcRef>,
    /// Signer index within the active validator set.
    pub signer: ValidatorIndex,
    /// BLS signature over the canonical QC-vote preimage.
    pub bls_sig: Vec<u8>,
}

/// BLS aggregate signature envelope with signer bitmap for constant-size certificates.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct QcAggregate {
    /// Compact signer bitmap (LSB-first).
    pub signers_bitmap: Vec<u8>,
    /// BLS12-381 aggregate signature bytes (compressed).
    pub bls_aggregate_signature: Vec<u8>,
}

/// QC certifying a phase for a block.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct Qc {
    /// Phase certified by this certificate.
    pub phase: CertPhase,
    /// Block hash certified by the certificate.
    pub subject_block_hash: HashOf<BlockHeader>,
    /// Parent state root bound into the QC.
    pub parent_state_root: Hash,
    /// Post-state root bound into the QC.
    pub post_state_root: Hash,
    /// Height of the subject block.
    pub height: Height,
    /// View in which the certificate was formed.
    pub view: View,
    /// Epoch index.
    pub epoch: u64,
    /// Hash of the canonical validator ordering this certificate is valid under.
    pub chain_order_hash: Hash,
    /// Validator-order update sequence this certificate is valid under.
    pub rechain_seq: u64,
    /// Consensus mode tag used to domain-separate signatures.
    pub mode_tag: String,
    /// Highest known QC that justifies a `NewView` QC, bound into the aggregate signature.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub highest_qc: Option<QcRef>,
    /// Stable hash of the validator set that produced the certificate.
    pub validator_set_hash: HashOf<Vec<PeerId>>,
    /// Version of the validator-set hashing scheme.
    pub validator_set_hash_version: u16,
    /// Ordered validator set used when assembling the certificate.
    pub validator_set: Vec<PeerId>,
    /// Aggregate signature and signer bitmap.
    pub aggregate: QcAggregate,
}

/// Evidence kinds for slashing or governance penalties.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode)]
pub enum EvidenceKind {
    /// Same (height, view) prepare vote on different blocks
    DoublePrepare = 0,
    /// Same (height, view) commit vote on different blocks
    DoubleCommit = 1,
    /// Invalid QC
    InvalidQc = 2,
    /// Invalid proposal
    InvalidProposal = 3,
    /// Transaction censorship proof (submission receipts).
    Censorship = 4,
    /// Exact conflicting Sumeragi v2 artifacts authenticated under one frozen
    /// height context.
    SumeragiV2Equivocation = 5,
}

/// Self-contained frozen context and exact signed artifacts for one Sumeragi
/// v2 equivocation proof.
///
/// Proofs of possession are retained in roster order so an auditor can verify
/// current-context aggregate certificates referenced by the artifacts without
/// consulting mutable validator state. Production persistence additionally
/// compares this context and PoP vector with the locally verified immutable
/// context record.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct SumeragiV2EquivocationEvidence {
    /// Immutable context which governed both conflicting artifacts.
    pub context: super::consensus_v2::HeightContext,
    /// BLS proofs of possession in exact frozen-roster order.
    pub proofs_of_possession: Vec<Vec<u8>>,
    /// Exact pair of conflicting signed artifacts.
    pub conflict: super::consensus_v2::SumeragiV2Equivocation,
}

/// Schema projection of the named fields in [`EvidencePayload::DoubleVote`].
#[derive(IntoSchema)]
pub struct DoubleVoteEvidencePayloadSchema {
    /// First observed vote.
    pub v1: QcVote,
    /// Second observed vote.
    pub v2: QcVote,
}

/// Schema projection of the named fields in [`EvidencePayload::InvalidQc`].
#[derive(IntoSchema)]
pub struct InvalidQcEvidencePayloadSchema {
    /// Certificate flagged as invalid.
    pub certificate: Qc,
    /// Human-readable invalidity reason.
    pub reason: String,
}

/// Schema projection of the named fields in
/// [`EvidencePayload::InvalidProposal`].
#[derive(IntoSchema)]
pub struct InvalidProposalEvidencePayloadSchema {
    /// Proposal flagged as invalid.
    pub proposal: Proposal,
    /// Human-readable invalidity reason.
    pub reason: String,
}

/// Schema projection of the named fields in [`EvidencePayload::Censorship`].
#[derive(IntoSchema)]
pub struct CensorshipEvidencePayloadSchema {
    /// Transaction hash referenced by the receipts.
    pub tx_hash: HashOf<crate::transaction::SignedTransaction>,
    /// Signed submission receipts from validators.
    pub receipts: Vec<TransactionSubmissionReceipt>,
}

/// Evidence payloads.
#[allow(variant_size_differences, clippy::large_enum_variant)]
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub enum EvidencePayload {
    /// Two votes by the same signer for the same (phase, height, view, epoch)
    /// on different block hashes.
    DoubleVote {
        /// First observed vote.
        v1: QcVote,
        /// Second observed vote.
        v2: QcVote,
    },
    /// A QC considered invalid by local checks.
    InvalidQc {
        /// The certificate flagged as invalid.
        certificate: Qc,
        /// Human-readable reason describing the invalidity.
        reason: String,
    },
    /// A proposal considered invalid by local checks.
    InvalidProposal {
        /// The proposal flagged as invalid.
        proposal: Proposal,
        /// Human-readable reason describing the invalidity.
        reason: String,
    },
    /// Evidence that a transaction was submitted but not proposed/committed.
    Censorship {
        /// Transaction hash referenced by the receipts.
        tx_hash: HashOf<crate::transaction::SignedTransaction>,
        /// Signed submission receipts from validators.
        receipts: Vec<TransactionSubmissionReceipt>,
    },
    /// Exact, independently verifiable Sumeragi v2 equivocation material.
    SumeragiV2Equivocation(SumeragiV2EquivocationEvidence),
}

/// Evidence wrapper.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
pub struct Evidence {
    /// High-level classification of the evidence.
    pub kind: EvidenceKind,
    /// Detailed payload carrying the offending material.
    pub payload: EvidencePayload,
}

impl TypeId for EvidenceKind {
    fn id() -> Ident {
        "EvidenceKind".to_owned()
    }
}

impl IntoSchema for EvidenceKind {
    fn type_name() -> Ident {
        "EvidenceKind".to_owned()
    }

    fn update_schema_map(metamap: &mut MetaMap) {
        let variants = vec![
            EnumVariant {
                tag: "DoublePrepare".to_owned(),
                discriminant: EvidenceKind::DoublePrepare as u8,
                ty: None,
            },
            EnumVariant {
                tag: "DoubleCommit".to_owned(),
                discriminant: EvidenceKind::DoubleCommit as u8,
                ty: None,
            },
            EnumVariant {
                tag: "InvalidQc".to_owned(),
                discriminant: EvidenceKind::InvalidQc as u8,
                ty: None,
            },
            EnumVariant {
                tag: "InvalidProposal".to_owned(),
                discriminant: EvidenceKind::InvalidProposal as u8,
                ty: None,
            },
            EnumVariant {
                tag: "Censorship".to_owned(),
                discriminant: EvidenceKind::Censorship as u8,
                ty: None,
            },
            EnumVariant {
                tag: "SumeragiV2Equivocation".to_owned(),
                discriminant: EvidenceKind::SumeragiV2Equivocation as u8,
                ty: None,
            },
        ];
        metamap.insert::<Self>(Metadata::Enum(EnumMeta { variants }));
    }
}

impl TypeId for EvidencePayload {
    fn id() -> Ident {
        "EvidencePayload".to_owned()
    }
}

impl IntoSchema for EvidencePayload {
    fn type_name() -> Ident {
        "EvidencePayload".to_owned()
    }

    fn update_schema_map(metamap: &mut MetaMap) {
        if metamap.contains_key::<Self>() {
            return;
        }
        DoubleVoteEvidencePayloadSchema::update_schema_map(metamap);
        InvalidQcEvidencePayloadSchema::update_schema_map(metamap);
        InvalidProposalEvidencePayloadSchema::update_schema_map(metamap);
        CensorshipEvidencePayloadSchema::update_schema_map(metamap);
        SumeragiV2EquivocationEvidence::update_schema_map(metamap);
        metamap.insert::<Self>(Metadata::Enum(EnumMeta {
            variants: vec![
                EnumVariant {
                    tag: "DoubleVote".to_owned(),
                    discriminant: 0,
                    ty: Some(core::any::TypeId::of::<DoubleVoteEvidencePayloadSchema>()),
                },
                EnumVariant {
                    tag: "InvalidQc".to_owned(),
                    discriminant: 1,
                    ty: Some(core::any::TypeId::of::<InvalidQcEvidencePayloadSchema>()),
                },
                EnumVariant {
                    tag: "InvalidProposal".to_owned(),
                    discriminant: 2,
                    ty: Some(core::any::TypeId::of::<InvalidProposalEvidencePayloadSchema>()),
                },
                EnumVariant {
                    tag: "Censorship".to_owned(),
                    discriminant: 3,
                    ty: Some(core::any::TypeId::of::<CensorshipEvidencePayloadSchema>()),
                },
                EnumVariant {
                    tag: "SumeragiV2Equivocation".to_owned(),
                    discriminant: 4,
                    ty: Some(core::any::TypeId::of::<SumeragiV2EquivocationEvidence>()),
                },
            ],
        }));
    }
}

impl Ord for Evidence {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        let left = self.encode();
        let right = other.encode();
        left.cmp(&right)
    }
}

impl PartialOrd for Evidence {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// Persisted evidence entry annotated with commit metadata.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct EvidenceRecord {
    /// Slashing material captured for governance processing.
    pub evidence: Evidence,
    /// Block height at which this evidence record was appended to WSV.
    pub recorded_at_height: Height,
    /// Consensus view (round) of the block carrying the record.
    pub recorded_at_view: View,
    /// Block creation timestamp in milliseconds since UNIX epoch.
    pub recorded_at_ms: u64,
    /// Whether a penalty was already applied for this evidence record.
    #[norito(default)]
    pub penalty_applied: bool,
    /// Whether governance cancelled penalty application for this evidence record.
    #[norito(default)]
    pub penalty_cancelled: bool,
    /// Block height at which the penalty was cancelled, if any.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub penalty_cancelled_at_height: Option<Height>,
    /// Block height at which the penalty was applied, if any.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub penalty_applied_at_height: Option<Height>,
    /// Block height which first admitted this exact evidence into consensus.
    ///
    /// `None` denotes node-local pending diagnostic material. Pending material
    /// is never eligible for deterministic penalty derivation.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub consensus_admitted_at_height: Option<Height>,
}

/// Membership snapshot exported through `/v1/sumeragi/status`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, Default)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct SumeragiMembershipStatus {
    /// Height associated with the snapshot.
    #[norito(default)]
    pub height: u64,
    /// View associated with the snapshot.
    #[norito(default)]
    pub view: u64,
    /// Epoch associated with the snapshot.
    #[norito(default)]
    pub epoch: u64,
    /// Deterministic roster hash for `(height, view, epoch)`.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub view_hash: Option<[u8; 32]>,
}

/// Membership mismatch snapshot exported through `/v1/sumeragi/status`.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, Default)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct SumeragiMembershipMismatchStatus {
    /// Peers currently flagged for membership mismatches.
    #[norito(default)]
    pub active_peers: Vec<PeerId>,
    /// Last peer observed with a mismatch (best-effort).
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub last_peer: Option<PeerId>,
    /// Height associated with the last mismatch (best-effort).
    #[norito(default)]
    pub last_height: u64,
    /// View associated with the last mismatch (best-effort).
    #[norito(default)]
    pub last_view: u64,
    /// Epoch associated with the last mismatch (best-effort).
    #[norito(default)]
    pub last_epoch: u64,
    /// Local membership hash observed during the last mismatch (best-effort).
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub last_local_hash: Option<[u8; 32]>,
    /// Remote membership hash observed during the last mismatch (best-effort).
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub last_remote_hash: Option<[u8; 32]>,
    /// Milliseconds since UNIX epoch when the last mismatch was recorded.
    #[norito(default)]
    pub last_timestamp_ms: u64,
}

/// Aggregated per-lane commitment summary reported by Sumeragi status.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct SumeragiLaneCommitment {
    /// Block height associated with the commitment.
    pub block_height: u64,
    /// Numeric lane identifier.
    pub lane_id: LaneId,
    /// Number of transactions attributed to the lane.
    pub tx_count: u64,
    /// Total RBC chunks allocated to the lane.
    pub total_chunks: u64,
    /// Total RBC payload bytes allocated to the lane.
    pub rbc_bytes_total: u64,
    /// Total TEU allocated to the lane.
    pub teu_total: u64,
    /// Block hash anchoring the commitment.
    pub block_hash: HashOf<BlockHeader>,
}

/// Aggregated per-dataspace commitment summary reported by Sumeragi status.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct SumeragiDataspaceCommitment {
    /// Block height associated with the commitment.
    pub block_height: u64,
    /// Numeric lane identifier.
    pub lane_id: LaneId,
    /// Numeric dataspace identifier.
    pub dataspace_id: DataSpaceId,
    /// Number of transactions attributed to the dataspace.
    pub tx_count: u64,
    /// Total RBC chunks allocated to the dataspace.
    pub total_chunks: u64,
    /// Total RBC payload bytes allocated to the dataspace.
    pub rbc_bytes_total: u64,
    /// Total TEU allocated to the dataspace.
    pub teu_total: u64,
    /// Block hash anchoring the commitment.
    pub block_hash: HashOf<BlockHeader>,
}

/// Execution status for a certified lane block whose payload is not locally recoverable yet.
pub const COMMITTED_LANE_STATUS_AWAITING_EXECUTABLE_PAYLOAD: &str = "awaiting_executable_payload";
/// Execution status for a certified lane block whose payload can be recovered for execution.
pub const COMMITTED_LANE_STATUS_PAYLOAD_AVAILABLE_AWAITING_EXECUTOR: &str =
    "payload_available_awaiting_executor";
/// Execution status for a certified lane block whose execution input has been recovered.
pub const COMMITTED_LANE_STATUS_PAYLOAD_RECOVERED_AWAITING_STATE_APPLICATION: &str =
    "payload_recovered_awaiting_state_application";
/// Execution status for a certified lane block that preflighted cleanly at the local state tip.
pub const COMMITTED_LANE_STATUS_PAYLOAD_PREFLIGHTED_AWAITING_STATE_APPLICATION: &str =
    "payload_preflighted_awaiting_state_application";
/// Execution status for a certified lane block whose preflight produced at least one rejection.
pub const COMMITTED_LANE_STATUS_PAYLOAD_PREFLIGHT_REJECTED_AWAITING_STATE_APPLICATION: &str =
    "payload_preflight_rejected_awaiting_state_application";
/// Execution status for a certified lane block whose canonical receipt conflicts with preflight.
pub const COMMITTED_LANE_STATUS_APPLICATION_RECEIPT_CONFLICTS_WITH_PREFLIGHT: &str =
    "application_receipt_conflicts_with_preflight";
/// Execution status for a certified lane block waiting for its predecessor to be applied.
pub const COMMITTED_LANE_STATUS_AWAITING_PREDECESSOR_APPLICATION: &str =
    "awaiting_predecessor_application";
/// Execution status for a certified lane block with committed canonical application results.
pub const COMMITTED_LANE_STATUS_STATE_APPLIED_BY_CANONICAL_BLOCK: &str =
    "state_applied_by_canonical_block";
/// Execution status for a certified lane block directly applied to the local WSV.
pub const COMMITTED_LANE_STATUS_STATE_APPLIED_BY_DIRECT_EXECUTION: &str =
    "state_applied_by_direct_execution";

/// Whether a committed lane-block status may count as rollout progress evidence.
///
/// Rejected preflight evidence is an execution blocker, not progress: it proves
/// the payload was recoverable, but it must not satisfy autoscale/localnet
/// expansion evidence until direct application or a canonical receipt resolves it.
#[must_use]
pub fn committed_lane_block_status_counts_as_progress(
    execution_status: &str,
    executable_payload_available: bool,
) -> bool {
    match execution_status {
        COMMITTED_LANE_STATUS_AWAITING_EXECUTABLE_PAYLOAD => !executable_payload_available,
        COMMITTED_LANE_STATUS_PAYLOAD_AVAILABLE_AWAITING_EXECUTOR
        | COMMITTED_LANE_STATUS_PAYLOAD_RECOVERED_AWAITING_STATE_APPLICATION
        | COMMITTED_LANE_STATUS_PAYLOAD_PREFLIGHTED_AWAITING_STATE_APPLICATION
        | COMMITTED_LANE_STATUS_STATE_APPLIED_BY_CANONICAL_BLOCK
        | COMMITTED_LANE_STATUS_STATE_APPLIED_BY_DIRECT_EXECUTION => executable_payload_available,
        COMMITTED_LANE_STATUS_PAYLOAD_PREFLIGHT_REJECTED_AWAITING_STATE_APPLICATION
        | COMMITTED_LANE_STATUS_AWAITING_PREDECESSOR_APPLICATION => false,
        _ => false,
    }
}

/// Certified standalone lane-local block summary reported by Sumeragi status.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct SumeragiCommittedLaneBlock {
    /// Lane whose local block is committed.
    pub lane_id: LaneId,
    /// Dataspace bound to the lane-local block.
    pub dataspace_id: DataSpaceId,
    /// Exact active incarnation of the lane when this block was certified.
    pub lane_incarnation: Hash,
    /// Lane-local block height.
    pub lane_block_height: u64,
    /// Lane-local consensus view.
    pub lane_block_view: u64,
    /// Stable hash of the standalone lane block descriptor.
    pub descriptor_hash: Hash,
    /// Stable hash of the standalone lane block proposal.
    pub proposal_hash: Hash,
    /// Operator-facing execution readiness label.
    pub execution_status: String,
    /// Whether payload material is locally available for standalone execution.
    pub executable_payload_available: bool,
    /// Subject hash certified by the lane block proposal.
    pub subject_hash: Hash,
    /// Payload ownership hash certified by the lane block proposal.
    pub payload_ownership_hash: Hash,
    /// RBC instance hash certified by the lane block proposal.
    pub rbc_instance_hash: Hash,
    /// Consensus/QC mode tag used to derive the lane hashes.
    pub qc_mode_tag: String,
    /// Validator count in the lane descriptor.
    pub validator_count: u32,
    /// Minimum quorum required by the lane descriptor.
    pub min_quorum: u32,
    /// Signers present in the prepare QC.
    pub prepare_qc_signer_count: u32,
    /// Signers present in the commit QC.
    pub commit_qc_signer_count: u32,
}

/// Planned lane-local payload ownership exported by Sumeragi status.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct SumeragiLanePayloadOwnership {
    /// Global proposal height that planned this lane-local payload.
    pub proposal_height: u64,
    /// Global proposal view that planned this lane-local payload.
    pub proposal_view: u64,
    /// Lane whose payload ownership is bound by this identity.
    pub lane_id: LaneId,
    /// Dataspace bound to the lane payload.
    pub dataspace_id: DataSpaceId,
    /// Exact active lane incarnation commitment.
    ///
    /// Recreated lanes receive a new commitment, so delayed artifacts from a
    /// retired incarnation remain invalid regardless of their lane-local height.
    pub lane_incarnation: Hash,
    /// Lane-local block height for the payload.
    pub lane_block_height: u64,
    /// Lane-local view for the payload.
    pub lane_block_view: u64,
    /// Stable digest of the lane-local block subject.
    pub subject_hash: Hash,
    /// Domain-separated QC mode tag used to derive the lane-local subject.
    pub qc_mode_tag: String,
    /// Fetched-batch candidate indices owned by this lane payload.
    pub accepted_candidate_indices: Vec<u64>,
    /// Accepted transaction hashes owned by this lane payload.
    pub accepted_transaction_hashes: Vec<Hash>,
    /// Lane-local predecessor height bound by the descriptor.
    pub previous_lane_block_height: u64,
    /// Descriptor hash of the lane-local predecessor, when the predecessor is known.
    pub previous_lane_block_descriptor_hash: Option<Hash>,
    /// Stable descriptor hash binding standalone lane block replay context.
    pub lane_block_descriptor_hash: Option<Hash>,
    /// Canonical validator set bound by the descriptor.
    pub lane_block_descriptor_validator_set: Vec<PeerId>,
    /// Validator count bound by the descriptor quorum context.
    pub lane_block_descriptor_validator_count: u32,
    /// Minimum quorum bound by the descriptor quorum context.
    pub lane_block_descriptor_min_quorum: u32,
    /// Stable digest naming lane-local payload ownership.
    pub payload_ownership_hash: Hash,
    /// Stable digest naming the lane-local RBC instance for this payload.
    pub rbc_instance_hash: Hash,
}

#[derive(Clone, Debug, Encode)]
struct LaneBlockProposalPreimage {
    purpose: String,
    version: u8,
    proposal_height: u64,
    descriptor_hash: Hash,
    lane_id: LaneId,
    dataspace_id: DataSpaceId,
    lane_incarnation: Hash,
    lane_block_height: u64,
    lane_block_view: u64,
    subject_hash: Hash,
    payload_ownership_hash: Hash,
    rbc_instance_hash: Hash,
    candidate_indices: Vec<u64>,
    candidate_hashes: Vec<Hash>,
    validator_set_hash_version: u16,
    validator_set_hash: HashOf<Vec<PeerId>>,
    validator_set: Vec<PeerId>,
    validator_count: u32,
    min_quorum: u32,
    qc_mode_tag: String,
}

/// Canonical descriptor for a standalone lane-local block proposal.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct LaneBlockDescriptorV1 {
    /// Lane whose local block is described.
    pub lane_id: LaneId,
    /// Dataspace bound to the lane-local block.
    pub dataspace_id: DataSpaceId,
    /// Exact active lane incarnation commitment.
    pub lane_incarnation: Hash,
    /// Global proposal height that planned this lane-local block.
    pub proposal_height: u64,
    /// Latest committed lane-local height used as this block's predecessor tip.
    pub previous_lane_block_height: u64,
    /// Descriptor hash of the predecessor tip, when the predecessor is known.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub previous_lane_block_descriptor_hash: Option<Hash>,
    /// Lane-local block height assigned to the descriptor.
    pub lane_block_height: u64,
    /// Lane-local view assigned to the descriptor.
    pub lane_block_view: u64,
    /// Lane-local subject hash signed by lane validators.
    pub subject_hash: Hash,
    /// DA/RBC payload ownership hash.
    pub payload_ownership_hash: Hash,
    /// DA/RBC instance hash.
    pub rbc_instance_hash: Hash,
    /// Accepted fetched-batch candidate indices in scheduler order.
    pub accepted_candidate_indices: Vec<u64>,
    /// Accepted transaction hashes in scheduler order.
    pub accepted_transaction_hashes: Vec<Hash>,
    /// Version of the validator-set hashing scheme.
    pub validator_set_hash_version: u16,
    /// Stable hash of the validator set eligible to sign this lane block.
    pub validator_set_hash: HashOf<Vec<PeerId>>,
    /// Canonical validator order eligible to sign this lane block.
    pub validator_set: Vec<PeerId>,
    /// Number of validators bound by the descriptor quorum context.
    pub validator_count: u32,
    /// Minimum distinct signer count required for quorum.
    pub min_quorum: u32,
    /// Domain-separated QC mode tag used for lane-local votes.
    pub qc_mode_tag: String,
    /// Stable descriptor digest binding predecessor, work, ownership, committee, and quorum.
    pub descriptor_hash: Hash,
}

impl LaneBlockDescriptorV1 {
    /// Compute the canonical descriptor hash from all descriptor fields except `descriptor_hash`.
    #[must_use]
    pub fn computed_descriptor_hash(&self) -> Hash {
        Hash::new(
            norito::to_bytes(&LaneBlockDescriptorPreimage {
                purpose: "nexus:lane-block-descriptor:v1".to_string(),
                version: 1,
                lane_id: self.lane_id,
                dataspace_id: self.dataspace_id,
                lane_incarnation: self.lane_incarnation,
                proposal_height: self.proposal_height,
                previous_lane_block_height: self.previous_lane_block_height,
                previous_lane_block_descriptor_hash: self.previous_lane_block_descriptor_hash,
                lane_block_height: self.lane_block_height,
                lane_block_view: self.lane_block_view,
                subject_hash: self.subject_hash,
                payload_ownership_hash: self.payload_ownership_hash,
                rbc_instance_hash: self.rbc_instance_hash,
                candidate_indices: self.accepted_candidate_indices.clone(),
                candidate_hashes: self.accepted_transaction_hashes.clone(),
                validator_set_hash_version: self.validator_set_hash_version,
                validator_set_hash: self.validator_set_hash,
                validator_set: self.validator_set.clone(),
                validator_count: self.validator_count,
                min_quorum: self.min_quorum,
                qc_mode_tag: self.qc_mode_tag.clone(),
            })
            .expect("lane block descriptor must encode"),
        )
    }

    /// Compute the canonical validator-set hash for the embedded validator order.
    #[must_use]
    pub fn computed_validator_set_hash(&self) -> HashOf<Vec<PeerId>> {
        HashOf::new(&self.validator_set)
    }
}

/// Advisory pointer to the canonical global block that carried a lane payload.
///
/// This is deliberately not part of [`LaneBlockProposalV1::computed_proposal_hash`].
/// Peers use it only as a recovery hint for fetching a certified block body;
/// the fetched block still has to validate against its commit certificate and
/// the lane descriptor before any payload is replayed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct LaneBlockProposalPayloadHintV1 {
    /// Global proposal height that anchored the lane payload ownership.
    pub proposal_height: u64,
    /// Global proposal view that anchored the lane payload ownership.
    pub proposal_view: u64,
    /// Hash of the global block body that carried the lane payload ownership.
    pub proposal_block_hash: HashOf<BlockHeader>,
}

/// Canonical standalone lane-local block proposal artifact.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct LaneBlockProposalV1 {
    /// Replayable descriptor proposed to the lane committee.
    pub descriptor: LaneBlockDescriptorV1,
    /// Stable proposal digest binding descriptor, work, committee, and quorum.
    pub proposal_hash: Hash,
    /// Optional recovery hint for fetching the global block body with the payload.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub payload_block_hint: Option<LaneBlockProposalPayloadHintV1>,
}

impl LaneBlockProposalV1 {
    /// Compute the canonical proposal hash from the embedded descriptor.
    #[must_use]
    pub fn computed_proposal_hash(&self) -> Hash {
        let descriptor = &self.descriptor;
        Hash::new(
            norito::to_bytes(&LaneBlockProposalPreimage {
                purpose: "nexus:lane-block-proposal:v1".to_string(),
                version: 1,
                proposal_height: descriptor.proposal_height,
                descriptor_hash: descriptor.descriptor_hash,
                lane_id: descriptor.lane_id,
                dataspace_id: descriptor.dataspace_id,
                lane_incarnation: descriptor.lane_incarnation,
                lane_block_height: descriptor.lane_block_height,
                lane_block_view: descriptor.lane_block_view,
                subject_hash: descriptor.subject_hash,
                payload_ownership_hash: descriptor.payload_ownership_hash,
                rbc_instance_hash: descriptor.rbc_instance_hash,
                candidate_indices: descriptor.accepted_candidate_indices.clone(),
                candidate_hashes: descriptor.accepted_transaction_hashes.clone(),
                validator_set_hash_version: descriptor.validator_set_hash_version,
                validator_set_hash: descriptor.validator_set_hash,
                validator_set: descriptor.validator_set.clone(),
                validator_count: descriptor.validator_count,
                min_quorum: descriptor.min_quorum,
                qc_mode_tag: descriptor.qc_mode_tag.clone(),
            })
            .expect("lane block proposal must encode"),
        )
    }

    /// Return `true` when two proposals identify the same certified lane block.
    #[must_use]
    pub fn same_consensus_identity(&self, other: &Self) -> bool {
        self.descriptor == other.descriptor && self.proposal_hash == other.proposal_hash
    }

    /// Attach a payload recovery hint without changing the proposal identity.
    #[must_use]
    pub fn with_payload_block_hint(mut self, hint: LaneBlockProposalPayloadHintV1) -> Self {
        self.payload_block_hint = Some(hint);
        self
    }

    /// Build a canonical lane-block vote body for this proposal and phase.
    #[must_use]
    pub fn vote_body(&self, phase: CertPhase) -> LaneBlockVoteBodyV1 {
        let descriptor = &self.descriptor;
        LaneBlockVoteBodyV1 {
            phase,
            lane_id: descriptor.lane_id,
            dataspace_id: descriptor.dataspace_id,
            lane_incarnation: descriptor.lane_incarnation,
            proposal_height: descriptor.proposal_height,
            lane_block_height: descriptor.lane_block_height,
            lane_block_view: descriptor.lane_block_view,
            proposal_hash: self.proposal_hash,
            descriptor_hash: descriptor.descriptor_hash,
            subject_hash: descriptor.subject_hash,
            payload_ownership_hash: descriptor.payload_ownership_hash,
            rbc_instance_hash: descriptor.rbc_instance_hash,
            accepted_candidate_indices: descriptor.accepted_candidate_indices.clone(),
            accepted_transaction_hashes: descriptor.accepted_transaction_hashes.clone(),
            validator_set_hash_version: descriptor.validator_set_hash_version,
            validator_set_hash: descriptor.validator_set_hash,
            validator_count: descriptor.validator_count,
            min_quorum: descriptor.min_quorum,
            qc_mode_tag: descriptor.qc_mode_tag.clone(),
        }
    }
}

/// Canonical lane-local block vote payload signed by lane committees.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct LaneBlockVoteBodyV1 {
    /// Lane-local QC phase certified by this vote body.
    pub phase: CertPhase,
    /// Lane whose local block is being certified.
    pub lane_id: LaneId,
    /// Dataspace bound to the lane-local block.
    pub dataspace_id: DataSpaceId,
    /// Exact active lane incarnation commitment.
    pub lane_incarnation: Hash,
    /// Global proposal height that planned this lane-local block.
    pub proposal_height: u64,
    /// Lane-local block height being certified.
    pub lane_block_height: u64,
    /// Lane-local view being certified.
    pub lane_block_view: u64,
    /// Standalone lane-block proposal hash.
    pub proposal_hash: Hash,
    /// Standalone lane-block descriptor hash.
    pub descriptor_hash: Hash,
    /// Lane-local subject hash.
    pub subject_hash: Hash,
    /// DA/RBC payload ownership hash.
    pub payload_ownership_hash: Hash,
    /// DA/RBC instance hash.
    pub rbc_instance_hash: Hash,
    /// Accepted fetched-batch candidate indices in scheduler order.
    pub accepted_candidate_indices: Vec<u64>,
    /// Accepted transaction hashes in scheduler order.
    pub accepted_transaction_hashes: Vec<Hash>,
    /// Version of the validator-set hashing scheme.
    pub validator_set_hash_version: u16,
    /// Stable hash of the validator set that may sign this lane block.
    pub validator_set_hash: HashOf<Vec<PeerId>>,
    /// Number of validators bound by the descriptor quorum context.
    pub validator_count: u32,
    /// Minimum distinct signer count required for quorum.
    pub min_quorum: u32,
    /// Domain-separated QC mode tag for this lane block.
    pub qc_mode_tag: String,
}

impl LaneBlockVoteBodyV1 {
    /// Build the domain-separated signature preimage for this lane-block vote body.
    #[must_use]
    pub fn signature_preimage(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(32 + 512);
        out.extend_from_slice(b"iroha:lane-block-vote:v1");
        out.extend_from_slice(&norito::to_bytes(self).expect("lane block vote body must encode"));
        out
    }
}

/// Exact autonomous lane payload retained by one READY signer.
///
/// The body names both the immutable payload's origin proposal and the
/// view-specific proposal being prepared. This prevents a valid payload
/// certificate from being rebound across chains, epochs, lane incarnations,
/// proposals, NewView transitions, or DA/RBC instances.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct LanePayloadAvailabilityBodyV1 {
    /// Artifact schema version. Only version one is accepted.
    pub version: u8,
    /// Hash of the chain identifier that owns the payload.
    pub chain_id_hash: Hash,
    /// Consensus epoch at the proposal compatibility height.
    pub epoch: u64,
    /// Lane whose executable payload is retained.
    pub lane_id: LaneId,
    /// Dataspace bound to the lane.
    pub dataspace_id: DataSpaceId,
    /// Exact lane lifecycle incarnation.
    pub lane_incarnation: Hash,
    /// Global compatibility height that selected the lane work.
    pub proposal_height: u64,
    /// Lane-local block height whose bytes are retained.
    pub lane_block_height: u64,
    /// View of the immutable producer-authenticated origin proposal.
    pub origin_lane_block_view: u64,
    /// Hash of the immutable producer-authenticated origin proposal.
    pub origin_proposal_hash: Hash,
    /// Descriptor hash of the immutable origin proposal.
    pub origin_descriptor_hash: Hash,
    /// View of the exact proposal currently being prepared.
    pub current_lane_block_view: u64,
    /// Hash of the exact proposal currently being prepared.
    pub current_proposal_hash: Hash,
    /// Descriptor hash of the exact proposal currently being prepared.
    pub current_descriptor_hash: Hash,
    /// View-specific lane subject hash.
    pub current_subject_hash: Hash,
    /// View-specific DA/RBC payload ownership hash.
    pub current_payload_ownership_hash: Hash,
    /// View-specific reliable-broadcast instance hash.
    pub current_rbc_instance_hash: Hash,
    /// View-neutral digest of the exact executable payload bytes.
    pub executable_payload_hash: Hash,
    /// Version of the validator-set hashing scheme.
    pub validator_set_hash_version: u16,
    /// Hash of the canonical lane committee.
    pub validator_set_hash: HashOf<Vec<PeerId>>,
    /// Number of validators in the canonical lane committee.
    pub validator_count: u32,
    /// Minimum distinct READY signers required for availability.
    pub min_quorum: u32,
    /// Lane consensus domain tag.
    pub qc_mode_tag: String,
}

impl LanePayloadAvailabilityBodyV1 {
    /// Build the domain-separated READY signature preimage.
    #[must_use]
    pub fn signature_preimage(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(32 + 512);
        out.extend_from_slice(b"iroha:lane-payload-availability-ready:v1");
        out.extend_from_slice(
            &norito::to_bytes(self).expect("lane payload availability body must encode"),
        );
        out
    }
}

/// Quorum proof that the exact autonomous executable payload is durably held.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct LanePayloadAvailabilityQcV1 {
    /// READY body certified by the aggregate signature.
    pub body: LanePayloadAvailabilityBodyV1,
    /// Version of the validator-set hashing scheme.
    pub validator_set_hash_version: u16,
    /// Stable hash of the validator set that produced the certificate.
    pub validator_set_hash: HashOf<Vec<PeerId>>,
    /// Ordered historical validator set indexed by `signers_bitmap`.
    pub validator_set: Vec<PeerId>,
    /// Valid historical PoPs aligned exactly with `validator_set`.
    pub validator_set_pops: Vec<Vec<u8>>,
    /// Compact READY signer bitmap (LSB-first).
    pub signers_bitmap: Vec<u8>,
    /// BLS12-381 aggregate READY signature bytes (compressed).
    pub bls_aggregate_signature: Vec<u8>,
}

/// Validator-set proof for a standalone lane-local block proposal.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct LaneBlockQcV1 {
    /// Vote body certified by the aggregate signature.
    pub body: LaneBlockVoteBodyV1,
    /// Version of the validator-set hashing scheme.
    pub validator_set_hash_version: u16,
    /// Stable hash of the validator set that produced the certificate.
    pub validator_set_hash: HashOf<Vec<PeerId>>,
    /// Ordered validator set used when assembling the certificate.
    pub validator_set: Vec<PeerId>,
    /// Compact signer bitmap (LSB-first).
    pub signers_bitmap: Vec<u8>,
    /// BLS12-381 aggregate signature bytes (compressed).
    pub bls_aggregate_signature: Vec<u8>,
    /// Exact payload-availability proof for autonomous prepare QCs.
    ///
    /// This is `None` for commit QCs and for compatibility lane proposals
    /// whose payload availability is inherited from a canonical global block.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub payload_availability_qc: Option<LanePayloadAvailabilityQcV1>,
}

#[derive(Clone, Debug, Encode)]
struct LanePayloadOwnershipSubjectPreimage {
    version: u8,
    lane_id: LaneId,
    dataspace_id: DataSpaceId,
    lane_incarnation: Hash,
    lane_block_height: u64,
    lane_block_view: u64,
    candidate_indices: Vec<u64>,
    candidate_hashes: Vec<Hash>,
    qc_mode_tag: String,
}

#[derive(Clone, Debug, Encode)]
struct LanePayloadOwnershipPreimage {
    purpose: String,
    version: u8,
    lane_id: LaneId,
    dataspace_id: DataSpaceId,
    lane_incarnation: Hash,
    lane_block_height: u64,
    lane_block_view: u64,
    subject_hash: Hash,
    candidate_indices: Vec<u64>,
    candidate_hashes: Vec<Hash>,
    qc_mode_tag: String,
}

#[derive(Clone, Debug, Encode)]
struct LanePayloadOwnershipRbcPreimage {
    purpose: String,
    version: u8,
    lane_id: LaneId,
    dataspace_id: DataSpaceId,
    lane_incarnation: Hash,
    lane_block_height: u64,
    lane_block_view: u64,
    subject_hash: Hash,
    payload_ownership_hash: Hash,
}

#[derive(Clone, Debug, Encode)]
struct LaneBlockDescriptorPreimage {
    purpose: String,
    version: u8,
    lane_id: LaneId,
    dataspace_id: DataSpaceId,
    lane_incarnation: Hash,
    proposal_height: u64,
    previous_lane_block_height: u64,
    previous_lane_block_descriptor_hash: Option<Hash>,
    lane_block_height: u64,
    lane_block_view: u64,
    subject_hash: Hash,
    payload_ownership_hash: Hash,
    rbc_instance_hash: Hash,
    candidate_indices: Vec<u64>,
    candidate_hashes: Vec<Hash>,
    validator_set_hash_version: u16,
    validator_set_hash: HashOf<Vec<PeerId>>,
    validator_set: Vec<PeerId>,
    validator_count: u32,
    min_quorum: u32,
    qc_mode_tag: String,
}

/// Canonical lane payload ownership replay hashes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SumeragiLanePayloadOwnershipReplayHashes {
    /// Expected lane-local block subject hash.
    pub subject_hash: Hash,
    /// Expected lane-local payload ownership hash.
    pub payload_ownership_hash: Hash,
    /// Expected lane-local RBC instance hash.
    pub rbc_instance_hash: Hash,
    /// Expected standalone lane block descriptor hash.
    pub lane_block_descriptor_hash: Hash,
}

/// Validation error for lane payload ownership replay material.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SumeragiLanePayloadOwnershipReplayError {
    /// Lane incarnation commitment is the reserved all-zero value.
    ZeroLaneIncarnation,
    /// QC mode tag is empty.
    BlankQcModeTag,
    /// No accepted candidate indices are present.
    EmptyCandidateIndices,
    /// Candidate index and accepted transaction hash counts differ.
    CandidateHashCountMismatch,
    /// Lane block height is zero.
    ZeroLaneBlockHeight,
    /// Previous lane block height does not equal `lane_block_height - 1`.
    PreviousLaneBlockHeightMismatch,
    /// Genesis predecessor unexpectedly carries a descriptor hash.
    UnexpectedGenesisPredecessorDescriptorHash,
    /// Descriptor hash is absent.
    MissingDescriptorHash,
    /// Descriptor validator set is empty.
    EmptyValidatorSet,
    /// Descriptor validator set is not in canonical sorted order.
    ValidatorSetNotCanonical,
    /// Descriptor validator set contains duplicate peers.
    DuplicateValidator,
    /// Descriptor validator count does not match the validator set length.
    ValidatorCountMismatch,
    /// Descriptor quorum is zero or exceeds validator count.
    InvalidQuorum,
    /// Norito encoding failed while deriving replay hashes.
    Encode,
    /// Subject hash does not match the replay material.
    SubjectHashMismatch,
    /// Payload ownership hash does not match the replay material.
    PayloadOwnershipHashMismatch,
    /// RBC instance hash does not match the replay material.
    RbcInstanceHashMismatch,
    /// Descriptor hash does not match the replay material.
    DescriptorHashMismatch,
}

impl fmt::Display for SumeragiLanePayloadOwnershipReplayError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::ZeroLaneIncarnation => "zero lane incarnation commitment",
            Self::BlankQcModeTag => "blank QC mode tag",
            Self::EmptyCandidateIndices => "empty candidate indices",
            Self::CandidateHashCountMismatch => "candidate hash count mismatch",
            Self::ZeroLaneBlockHeight => "zero lane block height",
            Self::PreviousLaneBlockHeightMismatch => "previous lane block height mismatch",
            Self::UnexpectedGenesisPredecessorDescriptorHash => {
                "unexpected genesis predecessor descriptor hash"
            }
            Self::MissingDescriptorHash => "missing descriptor hash",
            Self::EmptyValidatorSet => "empty descriptor validator set",
            Self::ValidatorSetNotCanonical => "non-canonical descriptor validator set",
            Self::DuplicateValidator => "duplicate descriptor validator",
            Self::ValidatorCountMismatch => "descriptor validator count mismatch",
            Self::InvalidQuorum => "invalid descriptor quorum",
            Self::Encode => "failed to encode replay preimage",
            Self::SubjectHashMismatch => "subject hash mismatch",
            Self::PayloadOwnershipHashMismatch => "payload ownership hash mismatch",
            Self::RbcInstanceHashMismatch => "RBC instance hash mismatch",
            Self::DescriptorHashMismatch => "descriptor hash mismatch",
        };
        f.write_str(message)
    }
}

impl SumeragiLanePayloadOwnership {
    /// Compute the canonical lane-local subject hash from replay material.
    ///
    /// # Errors
    ///
    /// Returns [`SumeragiLanePayloadOwnershipReplayError::Encode`] if the
    /// canonical preimage cannot be encoded.
    pub fn compute_replay_subject_hash(
        lane_id: LaneId,
        dataspace_id: DataSpaceId,
        lane_incarnation: Hash,
        lane_block_height: u64,
        lane_block_view: u64,
        accepted_candidate_indices: &[u64],
        accepted_transaction_hashes: &[Hash],
        qc_mode_tag: &str,
    ) -> Result<Hash, SumeragiLanePayloadOwnershipReplayError> {
        Ok(Hash::new(
            norito::to_bytes(&LanePayloadOwnershipSubjectPreimage {
                version: 1,
                lane_id,
                dataspace_id,
                lane_incarnation,
                lane_block_height,
                lane_block_view,
                candidate_indices: accepted_candidate_indices.to_vec(),
                candidate_hashes: accepted_transaction_hashes.to_vec(),
                qc_mode_tag: qc_mode_tag.to_string(),
            })
            .map_err(|_| SumeragiLanePayloadOwnershipReplayError::Encode)?,
        ))
    }

    /// Compute the canonical lane-local payload ownership hash.
    ///
    /// # Errors
    ///
    /// Returns [`SumeragiLanePayloadOwnershipReplayError::Encode`] if the
    /// canonical preimage cannot be encoded.
    pub fn compute_replay_payload_ownership_hash(
        lane_id: LaneId,
        dataspace_id: DataSpaceId,
        lane_incarnation: Hash,
        lane_block_height: u64,
        lane_block_view: u64,
        subject_hash: Hash,
        accepted_candidate_indices: &[u64],
        accepted_transaction_hashes: &[Hash],
        qc_mode_tag: &str,
    ) -> Result<Hash, SumeragiLanePayloadOwnershipReplayError> {
        Ok(Hash::new(
            norito::to_bytes(&LanePayloadOwnershipPreimage {
                purpose: "nexus:lane-payload-ownership:v1".to_string(),
                version: 1,
                lane_id,
                dataspace_id,
                lane_incarnation,
                lane_block_height,
                lane_block_view,
                subject_hash,
                candidate_indices: accepted_candidate_indices.to_vec(),
                candidate_hashes: accepted_transaction_hashes.to_vec(),
                qc_mode_tag: qc_mode_tag.to_string(),
            })
            .map_err(|_| SumeragiLanePayloadOwnershipReplayError::Encode)?,
        ))
    }

    /// Compute the canonical lane-local RBC instance hash.
    ///
    /// # Errors
    ///
    /// Returns [`SumeragiLanePayloadOwnershipReplayError::Encode`] if the
    /// canonical preimage cannot be encoded.
    pub fn compute_replay_rbc_instance_hash(
        lane_id: LaneId,
        dataspace_id: DataSpaceId,
        lane_incarnation: Hash,
        lane_block_height: u64,
        lane_block_view: u64,
        subject_hash: Hash,
        payload_ownership_hash: Hash,
    ) -> Result<Hash, SumeragiLanePayloadOwnershipReplayError> {
        Ok(Hash::new(
            norito::to_bytes(&LanePayloadOwnershipRbcPreimage {
                purpose: "nexus:lane-rbc-instance:v1".to_string(),
                version: 1,
                lane_id,
                dataspace_id,
                lane_incarnation,
                lane_block_height,
                lane_block_view,
                subject_hash,
                payload_ownership_hash,
            })
            .map_err(|_| SumeragiLanePayloadOwnershipReplayError::Encode)?,
        ))
    }

    /// Compute canonical replay hashes from the embedded descriptor material.
    ///
    /// # Errors
    ///
    /// Returns [`SumeragiLanePayloadOwnershipReplayError`] when required replay
    /// material is missing, malformed, or cannot be encoded.
    pub fn compute_replay_hashes(
        &self,
    ) -> Result<SumeragiLanePayloadOwnershipReplayHashes, SumeragiLanePayloadOwnershipReplayError>
    {
        self.validate_replay_shape()?;
        let subject_hash = Self::compute_replay_subject_hash(
            self.lane_id,
            self.dataspace_id,
            self.lane_incarnation,
            self.lane_block_height,
            self.lane_block_view,
            &self.accepted_candidate_indices,
            &self.accepted_transaction_hashes,
            &self.qc_mode_tag,
        )?;
        let payload_ownership_hash = Self::compute_replay_payload_ownership_hash(
            self.lane_id,
            self.dataspace_id,
            self.lane_incarnation,
            self.lane_block_height,
            self.lane_block_view,
            subject_hash,
            &self.accepted_candidate_indices,
            &self.accepted_transaction_hashes,
            &self.qc_mode_tag,
        )?;
        let rbc_instance_hash = Self::compute_replay_rbc_instance_hash(
            self.lane_id,
            self.dataspace_id,
            self.lane_incarnation,
            self.lane_block_height,
            self.lane_block_view,
            subject_hash,
            payload_ownership_hash,
        )?;
        let lane_block_descriptor_hash = Hash::new(
            norito::to_bytes(&LaneBlockDescriptorPreimage {
                purpose: "nexus:lane-block-descriptor:v1".to_string(),
                version: 1,
                lane_id: self.lane_id,
                dataspace_id: self.dataspace_id,
                lane_incarnation: self.lane_incarnation,
                proposal_height: self.proposal_height,
                previous_lane_block_height: self.previous_lane_block_height,
                previous_lane_block_descriptor_hash: self.previous_lane_block_descriptor_hash,
                lane_block_height: self.lane_block_height,
                lane_block_view: self.lane_block_view,
                subject_hash,
                payload_ownership_hash,
                rbc_instance_hash,
                candidate_indices: self.accepted_candidate_indices.clone(),
                candidate_hashes: self.accepted_transaction_hashes.clone(),
                validator_set_hash_version: crate::consensus::VALIDATOR_SET_HASH_VERSION_V1,
                validator_set_hash: HashOf::new(&self.lane_block_descriptor_validator_set),
                validator_set: self.lane_block_descriptor_validator_set.clone(),
                validator_count: self.lane_block_descriptor_validator_count,
                min_quorum: self.lane_block_descriptor_min_quorum,
                qc_mode_tag: self.qc_mode_tag.clone(),
            })
            .map_err(|_| SumeragiLanePayloadOwnershipReplayError::Encode)?,
        );
        Ok(SumeragiLanePayloadOwnershipReplayHashes {
            subject_hash,
            payload_ownership_hash,
            rbc_instance_hash,
            lane_block_descriptor_hash,
        })
    }

    /// Validate embedded replay material and all canonical ownership hashes.
    ///
    /// # Errors
    ///
    /// Returns [`SumeragiLanePayloadOwnershipReplayError`] when any replay field
    /// or canonical hash does not match the lane-local payload ownership.
    pub fn validate_replay_material(&self) -> Result<(), SumeragiLanePayloadOwnershipReplayError> {
        let expected = self.compute_replay_hashes()?;
        if self.subject_hash != expected.subject_hash {
            return Err(SumeragiLanePayloadOwnershipReplayError::SubjectHashMismatch);
        }
        if self.payload_ownership_hash != expected.payload_ownership_hash {
            return Err(SumeragiLanePayloadOwnershipReplayError::PayloadOwnershipHashMismatch);
        }
        if self.rbc_instance_hash != expected.rbc_instance_hash {
            return Err(SumeragiLanePayloadOwnershipReplayError::RbcInstanceHashMismatch);
        }
        if self.lane_block_descriptor_hash != Some(expected.lane_block_descriptor_hash) {
            return Err(SumeragiLanePayloadOwnershipReplayError::DescriptorHashMismatch);
        }
        Ok(())
    }

    fn validate_replay_shape(&self) -> Result<(), SumeragiLanePayloadOwnershipReplayError> {
        if self.lane_incarnation.as_ref().iter().all(|byte| *byte == 0) {
            return Err(SumeragiLanePayloadOwnershipReplayError::ZeroLaneIncarnation);
        }
        if self.qc_mode_tag.trim().is_empty() {
            return Err(SumeragiLanePayloadOwnershipReplayError::BlankQcModeTag);
        }
        if self.accepted_candidate_indices.is_empty() {
            return Err(SumeragiLanePayloadOwnershipReplayError::EmptyCandidateIndices);
        }
        if self.accepted_candidate_indices.len() != self.accepted_transaction_hashes.len() {
            return Err(SumeragiLanePayloadOwnershipReplayError::CandidateHashCountMismatch);
        }
        let Some(expected_previous) = self.lane_block_height.checked_sub(1) else {
            return Err(SumeragiLanePayloadOwnershipReplayError::ZeroLaneBlockHeight);
        };
        if self.previous_lane_block_height != expected_previous {
            return Err(SumeragiLanePayloadOwnershipReplayError::PreviousLaneBlockHeightMismatch);
        }
        if self.previous_lane_block_height == 0
            && self.previous_lane_block_descriptor_hash.is_some()
        {
            return Err(
                SumeragiLanePayloadOwnershipReplayError::UnexpectedGenesisPredecessorDescriptorHash,
            );
        }
        if self.previous_lane_block_height > 0 && self.previous_lane_block_descriptor_hash.is_none()
        {
            // Keep the public error surface stable: this variant also covers a
            // required predecessor descriptor hash that is absent.
            return Err(SumeragiLanePayloadOwnershipReplayError::MissingDescriptorHash);
        }
        if self.lane_block_descriptor_hash.is_none() {
            return Err(SumeragiLanePayloadOwnershipReplayError::MissingDescriptorHash);
        }
        if self.lane_block_descriptor_validator_set.is_empty() {
            return Err(SumeragiLanePayloadOwnershipReplayError::EmptyValidatorSet);
        }
        let mut canonical_validator_set = self.lane_block_descriptor_validator_set.clone();
        canonical_validator_set.sort();
        if canonical_validator_set != self.lane_block_descriptor_validator_set {
            return Err(SumeragiLanePayloadOwnershipReplayError::ValidatorSetNotCanonical);
        }
        for pair in canonical_validator_set.windows(2) {
            if pair[0] == pair[1] {
                return Err(SumeragiLanePayloadOwnershipReplayError::DuplicateValidator);
            }
        }
        let Ok(validator_count) = u32::try_from(self.lane_block_descriptor_validator_set.len())
        else {
            return Err(SumeragiLanePayloadOwnershipReplayError::ValidatorCountMismatch);
        };
        if self.lane_block_descriptor_validator_count != validator_count {
            return Err(SumeragiLanePayloadOwnershipReplayError::ValidatorCountMismatch);
        }
        if self.lane_block_descriptor_min_quorum == 0
            || self.lane_block_descriptor_min_quorum > self.lane_block_descriptor_validator_count
        {
            return Err(SumeragiLanePayloadOwnershipReplayError::InvalidQuorum);
        }
        Ok(())
    }
}

/// Deterministic settlement receipt emitted for audit and reconciliation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct LaneSettlementReceipt {
    /// Caller-specified identifier linking the receipt to the originating transaction.
    pub source_id: [u8; 32],
    /// Local gas-token amount debited from the payer (micro units).
    pub local_amount_micro: u128,
    /// XOR amount booked immediately after inclusion (micro units).
    pub xor_due_micro: u128,
    /// XOR amount expected post-haircut (micro units).
    pub xor_after_haircut_micro: u128,
    /// Safety margin consumed by this receipt (`xor_due_micro - xor_after_haircut_micro`).
    pub xor_variance_micro: u128,
    /// UTC timestamp in milliseconds when the receipt was generated.
    pub timestamp_ms: u64,
}

/// Deterministic Nexus fee schedule inputs captured for asynchronous settlement.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct NexusFeeScheduleInputs {
    /// Serialized signed transaction payload length used for fee metering.
    pub tx_bytes_len: u64,
    /// Number of native instructions included in the transaction fee calculation.
    pub instruction_count: u64,
    /// Gas units used by the transaction.
    pub gas_used: u64,
    /// Base fee from `nexus.fees.base_fee`.
    pub base_fee: Numeric,
    /// Per-byte fee from `nexus.fees.per_byte_fee`.
    pub per_byte_fee: Numeric,
    /// Per-instruction fee from `nexus.fees.per_instruction_fee`.
    pub per_instruction_fee: Numeric,
    /// Per-gas-unit fee from `nexus.fees.per_gas_unit_fee`.
    pub per_gas_unit_fee: Numeric,
}

/// Versioned Nexus fee receipt committed by a finalized lane block.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct NexusFeeReceipt {
    /// Receipt format version.
    pub version: u16,
    /// Source transaction hash/id.
    pub source_id: [u8; 32],
    /// DPN dataspace that finalized the source transaction.
    pub dataspace_id: DataSpaceId,
    /// DPN lane that finalized the source transaction.
    pub lane_id: LaneId,
    /// DPN block height that finalized the source transaction.
    pub block_height: u64,
    /// Sponsor or payer Nexus account charged for the public XOR burn.
    pub payer_account_id: AccountId,
    /// Fee asset selector; for DPN settlement this is fixed to `xor#universal`.
    pub fee_asset_id: String,
    /// Computed fee amount to burn on Nexus.
    pub fee_amount: Numeric,
    /// Fee schedule inputs needed to recompute [`Self::fee_amount`].
    pub schedule: NexusFeeScheduleInputs,
}

/// Phase certified by a native AMX participant committee.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(tag = "phase", content = "detail", rename_all = "snake_case")]
pub enum NativeAmxPhase {
    /// Participant prepared its dataspace-local leg.
    Prepare,
    /// Participant committed its dataspace-local leg.
    Commit,
}

/// Canonical native AMX attestation payload signed by participant committees.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct NativeAmxAttestationBodyV1 {
    /// Hash of the chain identifier that owns this attestation.
    pub chain_id_hash: Hash,
    /// Source transaction hash/id.
    pub source_id: [u8; 32],
    /// Hash of the canonical transaction entrypoint.
    pub tx_entrypoint_hash: HashOf<crate::transaction::TransactionEntrypoint>,
    /// Deterministic digest of the full coordinator/participant routing plan.
    pub plan_digest: Hash,
    /// Native AMX phase certified by this body.
    pub phase: NativeAmxPhase,
    /// Coordinator lane selected by the routing plan.
    pub coordinator_lane_id: LaneId,
    /// Coordinator dataspace selected by the routing plan.
    pub coordinator_dataspace_id: DataSpaceId,
    /// Exact active coordinator-lane incarnation at the authority context.
    pub coordinator_lane_incarnation: Hash,
    /// Participant lane certified by the committee.
    pub participant_lane_id: LaneId,
    /// Participant dataspace certified by the committee.
    pub participant_dataspace_id: DataSpaceId,
    /// Exact active participant-lane incarnation at the authority context.
    pub participant_lane_incarnation: Hash,
    /// Hash of the exact canonical participant committee that may attest this leg.
    pub participant_validator_set_hash: HashOf<Vec<PeerId>>,
    /// Number of validators in the exact participant committee.
    pub participant_validator_count: u32,
    /// Minimum number of participant signatures required by the lane quorum policy.
    pub participant_min_quorum: u32,
    /// Global/catalog height used to resolve routes, incarnations, committee,
    /// key activation, and proofs of possession.
    pub authority_context_height: u64,
    /// Coordinator lane-local block height that owns the transaction.
    pub coordinator_lane_block_height: u64,
    /// Coordinator lane-local consensus view for this exact attestation.
    pub coordinator_lane_block_view: u64,
    /// Exact coordinator lane-block proposal authenticated by the request.
    pub coordinator_proposal_hash: Hash,
}

impl NativeAmxAttestationBodyV1 {
    /// Build the domain-separated signature preimage for this attestation body.
    #[must_use]
    pub fn signature_preimage(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(32 + 256);
        out.extend_from_slice(b"iroha:native-amx:v1");
        out.extend_from_slice(
            &norito::to_bytes(self).expect("native AMX attestation body must encode"),
        );
        out
    }
}

/// Canonical Sumeragi v2 native AMX attestation payload.
///
/// The exact frozen round and election epoch are part of the signed payload,
/// preventing a valid lane-local vote from being replayed across chains,
/// parent decisions, epochs, heights, or views.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct NativeAmxAttestationBodyV2 {
    /// Exact frozen global round in which the receipt may be included.
    pub round: super::consensus_v2::ConsensusRound,
    /// Finalized election epoch repeated from the frozen height context.
    pub epoch: u64,
    /// Hash of the chain identifier that owns this attestation.
    pub chain_id_hash: Hash,
    /// Source transaction hash/id.
    pub source_id: [u8; 32],
    /// Hash of the canonical transaction entrypoint.
    pub tx_entrypoint_hash: HashOf<crate::transaction::TransactionEntrypoint>,
    /// Deterministic digest of the full coordinator/participant routing plan.
    pub plan_digest: Hash,
    /// Native AMX phase certified by this body.
    pub phase: NativeAmxPhase,
    /// Coordinator lane selected by the routing plan.
    pub coordinator_lane_id: LaneId,
    /// Coordinator dataspace selected by the routing plan.
    pub coordinator_dataspace_id: DataSpaceId,
    /// Exact active coordinator-lane incarnation at the frozen authority context.
    pub coordinator_lane_incarnation: Hash,
    /// Participant lane certified by the committee.
    pub participant_lane_id: LaneId,
    /// Participant dataspace certified by the committee.
    pub participant_dataspace_id: DataSpaceId,
    /// Exact active participant-lane incarnation at the frozen authority context.
    pub participant_lane_incarnation: Hash,
    /// Hash of the exact canonical participant committee that may attest this leg.
    pub participant_validator_set_hash: HashOf<Vec<PeerId>>,
    /// Number of validators in the exact participant committee.
    pub participant_validator_count: u32,
    /// Minimum number of participant signatures required by the lane quorum policy.
    pub participant_min_quorum: u32,
    /// Global/catalog height used to resolve routes, lane incarnations, keys, and PoPs.
    pub authority_context_height: u64,
    /// Coordinator block height planned for final inclusion.
    pub planned_coordinator_block_height: u64,
    /// Coordinator lane-local consensus view for this exact attestation.
    pub coordinator_lane_block_view: u64,
    /// Exact coordinator lane-block proposal authenticated by the full-plan request.
    pub coordinator_proposal_hash: Hash,
}

impl NativeAmxAttestationBodyV2 {
    /// Build the domain-separated signature preimage for this v2 attestation.
    #[must_use]
    pub fn signature_preimage(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(32 + 320);
        out.extend_from_slice(b"iroha:native-amx:v2");
        out.extend_from_slice(
            &norito::to_bytes(self).expect("native AMX v2 attestation body must encode"),
        );
        out
    }
}

/// Validator-set proof for a native AMX attestation body.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct NativeAmxAttestationQcV1 {
    /// Body certified by the aggregate signature.
    pub body: NativeAmxAttestationBodyV1,
    /// Version of the validator-set hashing scheme.
    pub validator_set_hash_version: u16,
    /// Stable hash of the validator set that produced the certificate.
    pub validator_set_hash: HashOf<Vec<PeerId>>,
    /// Ordered validator set used when assembling the certificate.
    pub validator_set: Vec<PeerId>,
    /// Historical BLS proofs-of-possession aligned exactly with `validator_set`.
    ///
    /// Keeping the full aligned vector makes the certificate independently
    /// verifiable after consensus-key rotation or lane retirement. The signed
    /// attestation body binds the validator-set hash, count, and quorum.
    pub validator_set_pops: Vec<Vec<u8>>,
    /// Compact signer bitmap (LSB-first).
    pub signers_bitmap: Vec<u8>,
    /// BLS12-381 aggregate signature bytes (compressed).
    pub bls_aggregate_signature: Vec<u8>,
}

/// Validator-set proof for a context-bound native AMX v2 attestation.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct NativeAmxAttestationQcV2 {
    /// Context-bound body certified by the aggregate signature.
    pub body: NativeAmxAttestationBodyV2,
    /// Version of the validator-set hashing scheme.
    pub validator_set_hash_version: u16,
    /// Stable hash of the validator set that produced the certificate.
    pub validator_set_hash: HashOf<Vec<PeerId>>,
    /// Ordered validator set used when assembling the certificate.
    pub validator_set: Vec<PeerId>,
    /// Historical BLS proofs-of-possession aligned exactly with `validator_set`.
    ///
    /// Embedding the full aligned vector keeps a certificate independently
    /// verifiable after consensus-key rotation or lane retirement.
    pub validator_set_pops: Vec<Vec<u8>>,
    /// Compact signer bitmap (LSB-first).
    pub signers_bitmap: Vec<u8>,
    /// BLS12-381 aggregate signature bytes (compressed).
    pub bls_aggregate_signature: Vec<u8>,
}

/// Per-dataspace native AMX leg committed by the routing-plan coordinator.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct NativeAmxLegRecord {
    /// Participant lane certified by both phase QCs.
    pub lane_id: LaneId,
    /// Dataspace participating in the native AMX group.
    pub dataspace_id: DataSpaceId,
    /// Exact participant-lane incarnation certified by both phase QCs.
    pub lane_incarnation: Hash,
    /// Participant prepare QC.
    pub prepare_qc: NativeAmxAttestationQcV1,
    /// Participant commit QC.
    pub commit_qc: NativeAmxAttestationQcV1,
}

/// Per-dataspace native AMX v2 leg committed by the routing-plan coordinator.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct NativeAmxLegRecordV2 {
    /// Participant lane certified by both phase QCs.
    pub lane_id: LaneId,
    /// Dataspace participating in the native AMX group.
    pub dataspace_id: DataSpaceId,
    /// Context-bound participant prepare QC.
    pub prepare_qc: NativeAmxAttestationQcV2,
    /// Context-bound participant commit QC.
    pub commit_qc: NativeAmxAttestationQcV2,
}

/// Versioned native AMX receipt committed by a finalized coordinator block.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct NativeAmxReceipt {
    /// Receipt format version.
    pub version: u16,
    /// Source transaction hash/id.
    pub source_id: [u8; 32],
    /// Hash of the chain identifier that owns this receipt.
    pub chain_id_hash: Hash,
    /// Deterministic digest of the coordinator/participant routing plan.
    pub plan_digest: Hash,
    /// Coordinator lane that finalized the transaction.
    pub lane_id: LaneId,
    /// Coordinator dataspace that finalized the transaction.
    pub dataspace_id: DataSpaceId,
    /// Exact coordinator-lane incarnation at the authority context.
    pub lane_incarnation: Hash,
    /// Global/catalog height used to resolve all lane and key authority.
    pub authority_context_height: u64,
    /// Coordinator lane-local height that owns the transaction.
    pub lane_block_height: u64,
    /// Coordinator lane-local view that owns the transaction.
    pub lane_block_view: u64,
    /// Exact coordinator lane-block proposal authenticated by participant QCs.
    pub coordinator_proposal_hash: Hash,
    /// Prepared and committed dataspace legs.
    pub legs: Vec<NativeAmxLegRecordV2>,
}

/// Liquidity profile applied when computing XOR conversions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(tag = "profile", content = "state")]
pub enum LaneLiquidityProfile {
    /// Deep pools with negligible slippage.
    Tier1,
    /// Medium depth pools with moderate slippage.
    Tier2,
    /// Thin pools or credit-constrained venues.
    Tier3,
}

/// Volatility bucket applied when computing the safety margin.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema, Default)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(tag = "bucket", content = "state")]
pub enum LaneVolatilityClass {
    /// Normal operating conditions.
    #[default]
    Stable,
    /// Elevated but healthy volatility.
    Elevated,
    /// Dislocated markets requiring maximal margin.
    Dislocated,
}

/// Swap metadata describing the deterministic conversion parameters.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct LaneSwapMetadata {
    /// Basis-point safety margin applied on top of the TWAP.
    pub epsilon_bps: u16,
    /// TWAP window length in seconds.
    pub twap_window_seconds: u32,
    /// Liquidity profile guiding haircut selection.
    pub liquidity_profile: LaneLiquidityProfile,
    /// Human-readable TWAP value (`local_token / XOR`) captured as a decimal string.
    pub twap_local_per_xor: String,
    /// Volatility bucket recorded when applying the epsilon.
    #[norito(default)]
    pub volatility_class: LaneVolatilityClass,
}

/// Aggregated per-lane settlement commitment captured within a block.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct LaneBlockCommitment {
    /// Lane-local block height associated with the commitment.
    pub block_height: u64,
    /// Numeric lane identifier.
    pub lane_id: LaneId,
    /// Active incarnation commitment for the lane-local height namespace.
    pub lane_incarnation: Hash,
    /// Numeric dataspace identifier.
    pub dataspace_id: DataSpaceId,
    /// Number of transactions contributing settlement receipts.
    pub tx_count: u64,
    /// Total local gas-token amount recorded in the block (micro units).
    pub total_local_micro: u128,
    /// Total XOR due immediately after inclusion (micro units).
    pub total_xor_due_micro: u128,
    /// Total XOR expected after applying liquidity haircuts (micro units).
    pub total_xor_after_haircut_micro: u128,
    /// Aggregate difference between the XOR debited and the post-haircut expectation (micro units).
    pub total_xor_variance_micro: u128,
    /// Deterministic metadata describing the conversion parameters.
    #[norito(default)]
    pub swap_metadata: Option<LaneSwapMetadata>,
    /// Deterministic receipts contributing to the commitment.
    #[norito(default)]
    pub receipts: Vec<LaneSettlementReceipt>,
    /// Versioned Nexus fee receipts committed for asynchronous public XOR settlement.
    #[norito(default)]
    pub nexus_fee_receipts: Vec<NexusFeeReceipt>,
    /// Versioned native AMX receipts committed by coordinator execution.
    #[norito(default)]
    pub native_amx_receipts: Vec<NativeAmxReceipt>,
}

impl<'a> norito::core::DecodeFromSlice<'a> for LaneSwapMetadata {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        decode_from_slice_canonical(bytes)
    }
}

/// Runtime-upgrade governance hook snapshot.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, Default)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct SumeragiRuntimeUpgradeHook {
    /// Whether runtime-upgrade instructions are allowed.
    pub allow: bool,
    /// Whether runtime-upgrade instructions must include metadata.
    pub require_metadata: bool,
    /// Metadata key enforced by the manifest, if specified.
    #[norito(default)]
    pub metadata_key: Option<String>,
    /// Allowed metadata values when an allowlist is configured.
    #[norito(default)]
    pub allowed_ids: Vec<String>,
}

/// Governance manifest readiness snapshot for a lane.
#[allow(missing_copy_implementations)]
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct SumeragiLaneGovernance {
    /// Numeric lane identifier.
    pub lane_id: LaneId,
    /// Human-readable lane alias.
    pub alias: String,
    /// Governance module configured for the lane, if any.
    #[norito(default)]
    pub governance: Option<String>,
    /// Whether the lane requires a governance manifest.
    pub manifest_required: bool,
    /// Whether a manifest has been loaded and validated.
    pub manifest_ready: bool,
    /// Path of the loaded manifest (best-effort; operator visibility).
    #[norito(default)]
    pub manifest_path: Option<String>,
    /// Validator identifiers derived from the manifest.
    #[norito(default)]
    pub validator_ids: Vec<String>,
    /// Quorum threshold configured by the manifest.
    #[norito(default)]
    pub quorum: Option<u32>,
    /// Protected namespaces enforced by the manifest.
    #[norito(default)]
    pub protected_namespaces: Vec<String>,
    /// Runtime-upgrade governance hook configuration.
    #[norito(default)]
    pub runtime_upgrade: Option<SumeragiRuntimeUpgradeHook>,
}

/// DA availability reason reported by `/v1/sumeragi/status`.
#[allow(missing_copy_implementations)]
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, Default)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(tag = "kind", content = "detail", rename_all = "snake_case")]
pub enum SumeragiDaGateReason {
    /// No gate currently blocking commit/finalize.
    #[default]
    None,
    /// Missing local data required to validate the pending block.
    MissingLocalData,
    /// Manifest is missing for the pending commitment.
    ManifestMissing,
    /// Manifest hash mismatched the commitment.
    ManifestHashMismatch,
    /// Manifest could not be read from disk.
    ManifestReadFailed,
    /// Manifest spool could not be scanned.
    ManifestSpoolScan,
}

/// Which DA availability condition was satisfied most recently.
#[allow(missing_copy_implementations)]
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, Default)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(tag = "kind", content = "detail", rename_all = "snake_case")]
pub enum SumeragiDaGateSatisfaction {
    /// No condition has been satisfied yet.
    #[default]
    None,
    /// Missing local data was recovered.
    MissingDataRecovered,
    /// Manifest guard was satisfied after previously reporting missing or invalid manifests.
    ManifestGuardRecovered,
}

/// Snapshot of DA availability tracking counters for `/v1/sumeragi/status`.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, Default)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct SumeragiDaGateStatus {
    /// Most recent reason that reported missing availability evidence.
    pub reason: SumeragiDaGateReason,
    /// Most recent condition that satisfied availability tracking.
    pub last_satisfied: SumeragiDaGateSatisfaction,
    /// Count of times local data was missing.
    pub missing_local_data_total: u64,
    /// Count of times the manifest guard reported missing/invalid manifests.
    #[norito(default)]
    pub manifest_guard_total: u64,
}

/// Snapshot of missing-block fetch attempts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, Default)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct SumeragiMissingBlockFetchStatus {
    /// Total fetch evaluations after QC-first arrival (including backoff/no-target cases).
    pub total: u64,
    /// Target count on the most recent fetch attempt.
    pub last_targets: u64,
    /// Dwell time in milliseconds observed before the most recent fetch attempt.
    pub last_dwell_ms: u64,
}

/// Snapshot of kura persistence failures and retries.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, Default)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct SumeragiKuraStoreStatus {
    /// Total times a block failed to enqueue for persistence.
    pub failures_total: u64,
    /// Total times kura persistence retries were exhausted for a block.
    pub abort_total: u64,
    /// Total times a block reached the staging phase before persistence.
    #[norito(default)]
    pub stage_total: u64,
    /// Total times a staged commit was rolled back before WSV application.
    #[norito(default)]
    pub rollback_total: u64,
    /// Height of the last staged block (best-effort).
    #[norito(default)]
    pub stage_last_height: u64,
    /// View of the last staged block (best-effort).
    #[norito(default)]
    pub stage_last_view: u64,
    /// Hash of the last staged block (best-effort).
    #[norito(skip_serializing_if = "Option::is_none")]
    pub stage_last_hash: Option<HashOf<BlockHeader>>,
    /// Height of the last staged commit rolled back (best-effort).
    #[norito(default)]
    pub rollback_last_height: u64,
    /// View of the last staged commit rolled back (best-effort).
    #[norito(default)]
    pub rollback_last_view: u64,
    /// Hash of the last staged commit rolled back (best-effort).
    #[norito(skip_serializing_if = "Option::is_none")]
    pub rollback_last_hash: Option<HashOf<BlockHeader>>,
    /// Reason label for the last rollback (best-effort).
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub rollback_last_reason: Option<String>,
    /// Total times Highest/Locked QC were reset after a kura abort.
    #[norito(default)]
    pub lock_reset_total: u64,
    /// Height associated with the last lock reset (best-effort).
    #[norito(default)]
    pub lock_reset_last_height: u64,
    /// View associated with the last lock reset (best-effort).
    #[norito(default)]
    pub lock_reset_last_view: u64,
    /// Hash associated with the last lock reset (best-effort).
    #[norito(skip_serializing_if = "Option::is_none")]
    pub lock_reset_last_hash: Option<HashOf<BlockHeader>>,
    /// Reason label for the last lock reset (best-effort).
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub lock_reset_last_reason: Option<String>,
    /// Last observed retry attempt count.
    pub last_retry_attempt: u64,
    /// Last observed retry backoff in milliseconds.
    pub last_retry_backoff_ms: u64,
    /// Height of the last block that failed to persist (best-effort).
    pub last_height: u64,
    /// View of the last block that failed to persist (best-effort).
    pub last_view: u64,
    /// Hash of the last block that failed to persist (best-effort).
    #[norito(skip_serializing_if = "Option::is_none")]
    pub last_hash: Option<HashOf<BlockHeader>>,
}

/// Session evicted from the RBC store due to TTL or capacity enforcement.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct SumeragiRbcEvictedSession {
    /// Block hash associated with the evicted session.
    pub block_hash: HashOf<BlockHeader>,
    /// Block height for the evicted session.
    pub height: u64,
    /// View index for the evicted session.
    pub view: u64,
}

impl Default for SumeragiRbcEvictedSession {
    fn default() -> Self {
        Self {
            block_hash: HashOf::from_untyped_unchecked(Hash::prehashed([0; Hash::LENGTH])),
            height: 0,
            view: 0,
        }
    }
}

/// Snapshot of the RBC on-disk store state.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, Default)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct SumeragiRbcStoreStatus {
    /// Current number of persisted RBC sessions on disk.
    pub sessions: u64,
    /// Current persisted RBC payload bytes on disk.
    pub bytes: u64,
    /// Current RBC store pressure level (0 = normal, 1 = soft limit, 2 = hard limit).
    pub pressure_level: u8,
    /// Total number of times proposal assembly was deferred due to RBC store pressure.
    pub backpressure_deferrals_total: u64,
    /// Total number of RBC sessions evicted due to TTL or capacity enforcement.
    pub evictions_total: u64,
    /// Most recent RBC sessions evicted due to TTL or capacity enforcement (bounded list).
    #[norito(default)]
    pub recent_evictions: Vec<SumeragiRbcEvictedSession>,
    /// Total number of RBC persist requests dropped due to full async queues.
    #[norito(default)]
    pub persist_drops_total: u64,
}

/// Per-peer RBC payload mismatch counters.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct SumeragiRbcMismatchEntry {
    /// Peer associated with the mismatch counts.
    pub peer_id: PeerId,
    /// Count of RBC chunk digest mismatches attributed to the peer.
    pub chunk_digest_mismatch_total: u64,
    /// Count of payload-hash mismatches attributed to the peer.
    pub payload_hash_mismatch_total: u64,
    /// Count of chunk-root mismatches attributed to the peer.
    pub chunk_root_mismatch_total: u64,
    /// Timestamp (ms since UNIX epoch) when the last mismatch was recorded.
    pub last_timestamp_ms: u64,
}

/// Snapshot of RBC mismatch counters.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, Default)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct SumeragiRbcMismatchStatus {
    /// Per-peer mismatch counters.
    #[norito(default)]
    pub entries: Vec<SumeragiRbcMismatchEntry>,
}

/// Snapshot of pending (pre-INIT) RBC stashes.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct SumeragiPendingRbcEntry {
    /// Block hash associated with the pending session.
    pub block_hash: HashOf<BlockHeader>,
    /// Block height for the pending session.
    pub height: u64,
    /// View index for the pending session.
    pub view: u64,
    /// Number of chunk frames currently buffered.
    pub chunks: u64,
    /// Total chunk payload bytes buffered.
    pub bytes: u64,
    /// Number of READY frames buffered.
    #[norito(default)]
    pub ready: u64,
    /// Number of DELIVER frames buffered.
    #[norito(default)]
    pub deliver: u64,
    /// Chunk frames dropped for this session due to caps.
    #[norito(default)]
    pub dropped_chunks: u64,
    /// Chunk payload bytes dropped for this session due to caps.
    #[norito(default)]
    pub dropped_bytes: u64,
    /// READY frames dropped for this session due to caps.
    #[norito(default)]
    pub dropped_ready: u64,
    /// DELIVER frames dropped for this session due to caps.
    #[norito(default)]
    pub dropped_deliver: u64,
    /// Approximate age (ms) since the first pending message was recorded.
    #[norito(default)]
    pub age_ms: u64,
}

impl Default for SumeragiPendingRbcEntry {
    fn default() -> Self {
        Self {
            block_hash: HashOf::from_untyped_unchecked(Hash::prehashed([0; Hash::LENGTH])),
            height: 0,
            view: 0,
            chunks: 0,
            bytes: 0,
            ready: 0,
            deliver: 0,
            dropped_chunks: 0,
            dropped_bytes: 0,
            dropped_ready: 0,
            dropped_deliver: 0,
            age_ms: 0,
        }
    }
}

/// Aggregated pending RBC stash telemetry.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, Default)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct SumeragiPendingRbcStatus {
    /// Current pending sessions awaiting INIT.
    pub sessions: u64,
    /// Maximum pending sessions retained (hard cap).
    pub session_cap: u64,
    /// Aggregate pending chunk frames across sessions.
    pub chunks: u64,
    /// Aggregate pending chunk payload bytes across sessions.
    pub bytes: u64,
    /// Configured per-session chunk cap.
    pub max_chunks_per_session: u64,
    /// Configured per-session byte cap.
    pub max_bytes_per_session: u64,
    /// Configured TTL (milliseconds) before pending entries expire.
    pub ttl_ms: u64,
    /// Total pending frames dropped across all reasons.
    #[norito(default)]
    pub drops_total: u64,
    /// Total pending frames dropped due to cap/session-cap enforcement.
    #[norito(default)]
    pub drops_cap_total: u64,
    /// Aggregate payload/signature bytes dropped due to caps.
    #[norito(default)]
    pub drops_cap_bytes_total: u64,
    /// Total pending frames dropped due to TTL expiry.
    #[norito(default)]
    pub drops_ttl_total: u64,
    /// Aggregate payload/signature bytes dropped due to TTL expiry.
    #[norito(default)]
    pub drops_ttl_bytes_total: u64,
    /// Total pending bytes dropped across all reasons.
    #[norito(default)]
    pub drops_bytes_total: u64,
    /// Total pending sessions evicted (TTL expiry or stash-cap eviction).
    #[norito(default)]
    pub evicted_total: u64,
    /// Total READY frames stashed before processing.
    #[norito(default)]
    pub stash_ready_total: u64,
    /// READY frames stashed because INIT has not arrived yet.
    #[norito(default)]
    pub stash_ready_init_missing_total: u64,
    /// READY frames stashed because the commit roster is missing.
    #[norito(default)]
    pub stash_ready_roster_missing_total: u64,
    /// READY frames stashed because the commit roster hash mismatched.
    #[norito(default)]
    pub stash_ready_roster_hash_mismatch_total: u64,
    /// READY frames stashed while the commit roster is unverified.
    #[norito(default)]
    pub stash_ready_roster_unverified_total: u64,
    /// Total DELIVER frames stashed before processing.
    #[norito(default)]
    pub stash_deliver_total: u64,
    /// DELIVER frames stashed because INIT has not arrived yet.
    #[norito(default)]
    pub stash_deliver_init_missing_total: u64,
    /// DELIVER frames stashed because the commit roster is missing.
    #[norito(default)]
    pub stash_deliver_roster_missing_total: u64,
    /// DELIVER frames stashed because the commit roster hash mismatched.
    #[norito(default)]
    pub stash_deliver_roster_hash_mismatch_total: u64,
    /// DELIVER frames stashed while the commit roster is unverified.
    #[norito(default)]
    pub stash_deliver_roster_unverified_total: u64,
    /// Chunk frames stashed before INIT arrives.
    #[norito(default)]
    pub stash_chunk_total: u64,
    /// Pending sessions with per-session drop counters.
    #[norito(default)]
    pub entries: Vec<SumeragiPendingRbcEntry>,
}

/// Block-sync roster selection counters exposed via Sumeragi status.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, Default)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct SumeragiBlockSyncRosterStatus {
    /// Total times a commit certificate hint was used.
    #[norito(default)]
    pub commit_qc_hint_total: u64,
    /// Total times a validator-checkpoint hint was used.
    #[norito(default)]
    pub checkpoint_hint_total: u64,
    /// Total times commit-certificate history was used.
    #[norito(default)]
    pub commit_qc_history_total: u64,
    /// Total times validator-checkpoint history was used.
    #[norito(default)]
    pub checkpoint_history_total: u64,
    /// Total times a roster sidecar was used.
    #[norito(default)]
    pub roster_sidecar_total: u64,
    /// Total times a commit-roster journal snapshot was used.
    #[norito(default)]
    pub commit_roster_journal_total: u64,
    /// Block-sync drops due to missing/invalid roster proofs.
    #[norito(default)]
    pub drop_missing_total: u64,
    /// Block-sync `ShareBlocks` drops without a matching request.
    #[norito(default)]
    pub drop_unsolicited_share_blocks_total: u64,
}

/// View-change cause counters surfaced via `/v1/sumeragi/status`.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, Default)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct SumeragiViewChangeCauseStatus {
    /// Total view changes triggered after commit failures (with QC quorum).
    #[norito(default)]
    pub commit_failure_total: u64,
    /// Total view changes triggered after quorum timeouts/missing commits.
    #[norito(default)]
    pub quorum_timeout_total: u64,
    /// Total view changes triggered after stake-quorum timeouts (`NPoS` only).
    #[norito(default)]
    pub stake_quorum_timeout_total: u64,
    /// Total view changes triggered after roster-unavailability recovery.
    #[norito(default)]
    pub roster_unavailable_total: u64,
    /// Total view changes triggered after the DA availability gate remains unresolved.
    #[norito(default)]
    pub da_gate_total: u64,
    /// Total view changes triggered after censorship evidence reaches quorum.
    #[norito(default)]
    pub censorship_evidence_total: u64,
    /// Total view changes triggered after missing payloads exceeded dwell.
    #[norito(default)]
    pub missing_payload_total: u64,
    /// Total view changes triggered after missing or stale QCs.
    #[norito(default)]
    pub missing_qc_total: u64,
    /// Total view changes triggered after validation rejects before voting.
    #[norito(default)]
    pub validation_reject_total: u64,
    /// Last recorded view-change cause label (if any).
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub last_cause: Option<String>,
    /// Milliseconds since UNIX epoch when the last cause was recorded.
    #[norito(default)]
    pub last_cause_timestamp_ms: u64,
    /// Milliseconds since UNIX epoch when a commit-failure cause was last recorded.
    #[norito(default)]
    pub last_commit_failure_timestamp_ms: u64,
    /// Milliseconds since UNIX epoch when a quorum-timeout cause was last recorded.
    #[norito(default)]
    pub last_quorum_timeout_timestamp_ms: u64,
    /// Milliseconds since UNIX epoch when a stake-quorum-timeout cause was last recorded.
    #[norito(default)]
    pub last_stake_quorum_timeout_timestamp_ms: u64,
    /// Milliseconds since UNIX epoch when a roster-unavailable cause was last recorded.
    #[norito(default)]
    pub last_roster_unavailable_timestamp_ms: u64,
    /// Milliseconds since UNIX epoch when a DA-gate cause was last recorded.
    #[norito(default)]
    pub last_da_gate_timestamp_ms: u64,
    /// Milliseconds since UNIX epoch when a censorship-evidence cause was last recorded.
    #[norito(default)]
    pub last_censorship_evidence_timestamp_ms: u64,
    /// Milliseconds since UNIX epoch when a missing-payload cause was last recorded.
    #[norito(default)]
    pub last_missing_payload_timestamp_ms: u64,
    /// Milliseconds since UNIX epoch when a missing-QC cause was last recorded.
    #[norito(default)]
    pub last_missing_qc_timestamp_ms: u64,
    /// Milliseconds since UNIX epoch when a validation-reject cause was last recorded.
    #[norito(default)]
    pub last_validation_reject_timestamp_ms: u64,
}

/// Validation-gate reject counters and last-occurrence snapshot.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, Default)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct SumeragiValidationRejectStatus {
    /// Total rejects recorded before voting.
    #[norito(default)]
    pub total: u64,
    /// Stateless validation rejects (header, timestamps, genesis checks).
    #[norito(default)]
    pub stateless_total: u64,
    /// Execution/stateful validation rejects (transaction execution, DA availability checks).
    #[norito(default)]
    pub execution_total: u64,
    /// Prev-block hash mismatch rejects.
    #[norito(default)]
    pub prev_hash_total: u64,
    /// Prev-block height mismatch rejects.
    #[norito(default)]
    pub prev_height_total: u64,
    /// Topology/roster mismatch rejects.
    #[norito(default)]
    pub topology_total: u64,
    /// Last recorded reason label (best-effort).
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub last_reason: Option<String>,
    /// Last rejected block height (best-effort).
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub last_height: Option<u64>,
    /// Last rejected block view (best-effort).
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub last_view: Option<u64>,
    /// Last rejected block hash (best-effort).
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub last_block: Option<HashOf<BlockHeader>>,
    /// Milliseconds since UNIX epoch when the last reject was recorded.
    #[norito(default)]
    pub last_timestamp_ms: u64,
}

/// Peer consensus-key policy reject counters and last-occurrence snapshot.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, Default)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct SumeragiPeerKeyPolicyStatus {
    /// Total peer-key policy rejects recorded.
    #[norito(default)]
    pub total: u64,
    /// Rejects due to missing HSM binding when required.
    #[norito(default)]
    pub missing_hsm_total: u64,
    /// Rejects due to disallowed public-key algorithm.
    #[norito(default)]
    pub disallowed_algorithm_total: u64,
    /// Rejects due to disallowed HSM provider.
    #[norito(default)]
    pub disallowed_provider_total: u64,
    /// Rejects due to activation height violating lead-time policy.
    #[norito(default)]
    pub lead_time_violation_total: u64,
    /// Rejects due to activation height being in the past.
    #[norito(default)]
    pub activation_in_past_total: u64,
    /// Rejects due to expiry occurring before activation.
    #[norito(default)]
    pub expiry_before_activation_total: u64,
    /// Rejects due to identifier collisions for the same public key.
    #[norito(default)]
    pub identifier_collision_total: u64,
    /// Last recorded reject reason (best-effort).
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub last_reason: Option<String>,
    /// Milliseconds since UNIX epoch when the last reject was recorded.
    #[norito(default)]
    pub last_timestamp_ms: u64,
}

/// Consensus message drop/deferral counter entry.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct SumeragiConsensusMessageHandlingEntry {
    /// Message kind label (e.g., `block_created`).
    pub kind: String,
    /// Handling outcome label (e.g., `dropped` or `deferred`).
    pub outcome: String,
    /// Drop/deferral reason label.
    pub reason: String,
    /// Total observed for the `(kind,outcome,reason)` tuple.
    pub total: u64,
}

/// Consensus message drop/deferral counters surfaced via Sumeragi status.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, Default)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct SumeragiConsensusMessageHandlingStatus {
    /// Per-kind drop/deferral counters (best-effort).
    #[norito(default)]
    pub entries: Vec<SumeragiConsensusMessageHandlingEntry>,
}

/// Vote validation drop entry with roster context.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct SumeragiVoteValidationDropEntry {
    /// Drop reason label.
    pub reason: String,
    /// Vote height.
    pub height: u64,
    /// Vote view.
    pub view: u64,
    /// Vote epoch.
    pub epoch: u64,
    /// Signer index from the vote payload.
    pub signer_index: u32,
    /// Peer ID resolved from the validation roster (if any).
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub peer_id: Option<PeerId>,
    /// Validator roster hash used for validation (if any).
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub roster_hash: Option<HashOf<Vec<PeerId>>>,
    /// Validator roster length used for validation (if known).
    pub roster_len: u32,
    /// Block hash referenced by the vote.
    pub block_hash: HashOf<BlockHeader>,
    /// Milliseconds since UNIX epoch when the drop was recorded.
    pub timestamp_ms: u64,
}

/// Aggregated count for a vote-validation drop reason.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct SumeragiVoteValidationDropReasonCount {
    /// Drop reason label.
    pub reason: String,
    /// Total drops recorded for the reason.
    pub total: u64,
}

/// Aggregated vote validation drops for a peer/roster hash pairing.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct SumeragiVoteValidationDropPeerEntry {
    /// Peer associated with the drop counts.
    pub peer_id: PeerId,
    /// Validator roster hash used for validation (if any).
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub roster_hash: Option<HashOf<Vec<PeerId>>>,
    /// Validator roster length used for validation (if known).
    pub roster_len: u32,
    /// Total drops recorded for this peer/roster pairing.
    pub total: u64,
    /// Per-reason drop counters.
    #[norito(default)]
    pub reasons: Vec<SumeragiVoteValidationDropReasonCount>,
    /// Height associated with the last drop.
    pub last_height: u64,
    /// View associated with the last drop.
    pub last_view: u64,
    /// Epoch associated with the last drop.
    pub last_epoch: u64,
    /// Milliseconds since UNIX epoch when the last drop was recorded.
    pub last_timestamp_ms: u64,
}

/// Vote validation drop snapshot surfaced via Sumeragi status.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, Default)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct SumeragiVoteValidationDropStatus {
    /// Total vote validation drops recorded.
    #[norito(default)]
    pub total: u64,
    /// Recent drop entries (newest-first, bounded).
    #[norito(default)]
    pub entries: Vec<SumeragiVoteValidationDropEntry>,
    /// Aggregated drop counters per peer/roster pairing.
    #[norito(default)]
    pub peer_entries: Vec<SumeragiVoteValidationDropPeerEntry>,
}

/// Deterministic consensus configuration caps captured alongside status snapshots.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, Default)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[allow(clippy::struct_excessive_bools)] // Independent capability toggles are kept explicit for the status surface.
pub struct SumeragiConsensusCapsStatus {
    /// Canonical digest of deterministic, locally configured Nexus policy.
    #[norito(default)]
    pub nexus_policy_digest: [u8; 32],
    /// Number of collectors (K).
    pub collectors_k: u16,
    /// Redundant send fanout (r).
    pub redundant_send_r: u8,
    /// Data availability enabled (RBC + availability QC gating).
    pub da_enabled: bool,
    /// Maximum RBC chunk size in bytes.
    pub rbc_chunk_max_bytes: u64,
    /// RBC payload encoding.
    pub rbc_encoding: RbcEncoding,
    /// RS16 data shards per stripe (`0` when plain chunking is active).
    pub rbc_rs16_data_shards: u16,
    /// RS16 parity shards per stripe (`0` when plain chunking is active).
    pub rbc_rs16_parity_shards: u16,
    /// RBC session TTL in milliseconds.
    pub rbc_session_ttl_ms: u64,
    /// Hard cap on persisted RBC sessions.
    pub rbc_store_max_sessions: u32,
    /// Soft cap on persisted RBC sessions.
    pub rbc_store_soft_sessions: u32,
    /// Hard cap on persisted RBC payload bytes.
    pub rbc_store_max_bytes: u64,
    /// Soft cap on persisted RBC payload bytes.
    pub rbc_store_soft_bytes: u64,
}

/// Queue depth snapshot for Sumeragi worker-loop channels.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, Default)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct SumeragiWorkerQueueDepths {
    /// Vote channel depth.
    #[norito(default)]
    pub vote_rx: u64,
    /// Block payload channel depth.
    #[norito(default)]
    pub block_payload_rx: u64,
    /// RBC chunk channel depth.
    #[norito(default)]
    pub rbc_chunk_rx: u64,
    /// Block channel depth.
    #[norito(default)]
    pub block_rx: u64,
    /// Consensus control channel depth.
    #[norito(default)]
    pub consensus_rx: u64,
    /// Lane relay channel depth.
    #[norito(default)]
    pub lane_relay_rx: u64,
    /// Background post channel depth.
    #[norito(default)]
    pub background_rx: u64,
}

/// Per-queue totals for worker-loop diagnostics.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, Default)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct SumeragiWorkerQueueTotals {
    /// Vote channel total.
    #[norito(default)]
    pub vote_rx: u64,
    /// Block payload channel total.
    #[norito(default)]
    pub block_payload_rx: u64,
    /// RBC chunk channel total.
    #[norito(default)]
    pub rbc_chunk_rx: u64,
    /// Block channel total.
    #[norito(default)]
    pub block_rx: u64,
    /// Consensus control channel total.
    #[norito(default)]
    pub consensus_rx: u64,
    /// Lane relay channel total.
    #[norito(default)]
    pub lane_relay_rx: u64,
    /// Background post channel total.
    #[norito(default)]
    pub background_rx: u64,
}

/// Worker-loop queue diagnostics (drops/blocking).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, Default)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct SumeragiWorkerQueueDiagnostics {
    /// Total count of blocking enqueues per queue.
    #[norito(default)]
    pub blocked_total: SumeragiWorkerQueueTotals,
    /// Total time spent blocked (ms) per queue.
    #[norito(default)]
    pub blocked_ms_total: SumeragiWorkerQueueTotals,
    /// Maximum block duration (ms) per queue.
    #[norito(default)]
    pub blocked_max_ms: SumeragiWorkerQueueTotals,
    /// Total count of dropped enqueues per queue.
    #[norito(default)]
    pub dropped_total: SumeragiWorkerQueueTotals,
}

/// Worker-loop diagnostics exposed by `/v1/sumeragi/status`.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, Default)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct SumeragiWorkerLoopStatus {
    /// Last observed worker-loop stage label.
    #[norito(default)]
    pub stage: String,
    /// Timestamp (ms since UNIX epoch) when the stage was last updated.
    #[norito(default)]
    pub stage_started_ms: u64,
    /// Duration of the most recent worker iteration in milliseconds.
    #[norito(default)]
    pub last_iteration_ms: u64,
    /// Queue depth snapshot for worker-loop channels.
    #[norito(default)]
    pub queue_depths: SumeragiWorkerQueueDepths,
    /// Queue enqueue diagnostics (drops/blocking).
    #[norito(default)]
    pub queue_diagnostics: SumeragiWorkerQueueDiagnostics,
}

/// Commit inflight diagnostics exposed by `/v1/sumeragi/status`.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, Default)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct SumeragiCommitInflightStatus {
    /// Whether a commit job is currently in flight.
    #[norito(default)]
    pub active: bool,
    /// Inflight commit id (best-effort).
    #[norito(default)]
    pub id: u64,
    /// Block height associated with the inflight commit.
    #[norito(default)]
    pub height: u64,
    /// View associated with the inflight commit.
    #[norito(default)]
    pub view: u64,
    /// Block hash associated with the inflight commit.
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub block_hash: Option<HashOf<BlockHeader>>,
    /// Timestamp (ms since UNIX epoch) when the inflight commit was enqueued.
    #[norito(default)]
    pub started_ms: u64,
    /// Milliseconds elapsed since the inflight commit started (best-effort).
    #[norito(default)]
    pub elapsed_ms: u64,
    /// Configured inflight timeout in milliseconds.
    #[norito(default)]
    pub timeout_ms: u64,
    /// Total inflight timeouts observed.
    #[norito(default)]
    pub timeout_total: u64,
    /// Timestamp (ms since UNIX epoch) of the last inflight timeout.
    #[norito(default)]
    pub last_timeout_timestamp_ms: u64,
    /// Duration (ms) of the last inflight timeout.
    #[norito(default)]
    pub last_timeout_elapsed_ms: u64,
    /// Height associated with the last inflight timeout.
    #[norito(default)]
    pub last_timeout_height: u64,
    /// View associated with the last inflight timeout.
    #[norito(default)]
    pub last_timeout_view: u64,
    /// Block hash associated with the last inflight timeout.
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub last_timeout_block_hash: Option<HashOf<BlockHeader>>,
    /// Total number of pacemaker pauses caused by inflight commits.
    #[norito(default)]
    pub pause_total: u64,
    /// Total number of pacemaker resumes following inflight completion.
    #[norito(default)]
    pub resume_total: u64,
    /// Timestamp (ms since UNIX epoch) when the current pause began.
    #[norito(default)]
    pub paused_since_ms: u64,
    /// Queue depth snapshot recorded when the inflight pause started.
    #[norito(default)]
    pub pause_queue_depths: SumeragiWorkerQueueDepths,
    /// Queue depth snapshot recorded when the inflight pause ended.
    #[norito(default)]
    pub resume_queue_depths: SumeragiWorkerQueueDepths,
}

/// Commit-pipeline timing snapshot exposed by `/v1/sumeragi/status`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, Default)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct SumeragiCommitPipelineStatus {
    /// End-to-end time spent in the most recent commit-pipeline run.
    #[norito(default)]
    pub last_total_ms: u64,
    /// Time spent validating/finalizing candidate blocks before gating.
    #[norito(default)]
    pub last_validation_ms: u64,
    /// Time spent rebuilding cached QCs from votes.
    #[norito(default)]
    pub last_qc_rebuild_ms: u64,
    /// Time spent in validation/availability gate checks.
    #[norito(default)]
    pub last_gate_ms: u64,
    /// Time spent finalizing pending blocks into the commit worker.
    #[norito(default)]
    pub last_finalize_ms: u64,
    /// Time spent draining finished commit results.
    #[norito(default)]
    pub last_drain_results_ms: u64,
    /// Sum of QC verification subtotals across drained commit results.
    #[norito(default)]
    pub last_drain_qc_verify_ms: u64,
    /// Sum of persistence subtotals across drained commit results.
    #[norito(default)]
    pub last_drain_persist_ms: u64,
    /// Sum of Kura store subtotals across drained commit results.
    #[norito(default)]
    pub last_drain_kura_store_ms: u64,
    /// Sum of state-apply subtotals across drained commit results.
    #[norito(default)]
    pub last_drain_state_apply_ms: u64,
    /// Sum of state-commit subtotals across drained commit results.
    #[norito(default)]
    pub last_drain_state_commit_ms: u64,
    /// EMA of end-to-end commit-pipeline time.
    #[norito(default)]
    pub ema_total_ms: u64,
    /// EMA of validation time.
    #[norito(default)]
    pub ema_validation_ms: u64,
    /// EMA of gate time.
    #[norito(default)]
    pub ema_gate_ms: u64,
    /// EMA of finalize time.
    #[norito(default)]
    pub ema_finalize_ms: u64,
}

/// DELIVER-to-next-proposal gap snapshot exposed by `/v1/sumeragi/status`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, Default)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct SumeragiRoundGapStatus {
    /// Most recent elapsed time from first accepted DELIVER to local state commit.
    #[norito(default)]
    pub last_deliver_to_state_commit_ms: u64,
    /// Most recent elapsed time from local state commit to pacemaker unblock.
    #[norito(default)]
    pub last_state_commit_to_next_propose_ms: u64,
    /// Most recent elapsed time from first accepted DELIVER to pacemaker unblock.
    #[norito(default)]
    pub last_deliver_to_next_propose_ms: u64,
    /// EMA of DELIVER-to-state-commit.
    #[norito(default)]
    pub ema_deliver_to_state_commit_ms: u64,
    /// EMA of state-commit-to-next-propose.
    #[norito(default)]
    pub ema_state_commit_to_next_propose_ms: u64,
    /// EMA of DELIVER-to-next-propose.
    #[norito(default)]
    pub ema_deliver_to_next_propose_ms: u64,
}

/// Latest commit-quorum signature tally exposed by `/v1/sumeragi/status`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, Default)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct SumeragiCommitQuorumStatus {
    /// Block height associated with the tally.
    #[norito(default)]
    pub height: u64,
    /// View associated with the tally.
    #[norito(default)]
    pub view: u64,
    /// Block hash associated with the tally.
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub block_hash: Option<HashOf<BlockHeader>>,
    /// Total signatures present on the block.
    #[norito(default)]
    pub signatures_present: u64,
    /// Signatures counted toward the commit quorum.
    #[norito(default)]
    pub signatures_counted: u64,
    /// Signatures contributed by set-B validators.
    #[norito(default)]
    pub signatures_set_b: u64,
    /// Required commit quorum size.
    #[norito(default)]
    pub signatures_required: u64,
    /// Timestamp (ms since UNIX epoch) when the tally was recorded.
    #[norito(default)]
    pub last_updated_ms: u64,
}

/// Latest commit QC summary exposed by `/v1/sumeragi/status`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, Default)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct SumeragiQcStatus {
    /// Block height certified by the commit QC.
    #[norito(default)]
    pub height: u64,
    /// View associated with the commit QC.
    #[norito(default)]
    pub view: u64,
    /// Epoch associated with the commit QC.
    #[norito(default)]
    pub epoch: u64,
    /// Block hash certified by the commit QC.
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub block_hash: Option<HashOf<BlockHeader>>,
    /// Stable hash of the validator set that produced the QC.
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub validator_set_hash: Option<HashOf<Vec<PeerId>>>,
    /// Number of validators in the recorded set.
    #[norito(default)]
    pub validator_set_len: u64,
    /// Total signatures attached to the QC.
    #[norito(default)]
    pub signatures_total: u64,
}

/// Effective `NPoS` timeout values (ms) exposed via `/v1/sumeragi/status`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, Default)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct SumeragiNposTimeoutsStatus {
    /// Proposal timeout (ms).
    #[norito(default)]
    pub propose_ms: u64,
    /// Prevote aggregation timeout (ms).
    #[norito(default)]
    pub prevote_ms: u64,
    /// Precommit aggregation timeout (ms).
    #[norito(default)]
    pub precommit_ms: u64,
    /// Commit finalization timeout (ms).
    #[norito(default)]
    pub commit_ms: u64,
    /// Data-availability timeout (ms).
    #[norito(default)]
    pub da_ms: u64,
    /// Aggregator fallback timeout (ms).
    #[norito(default)]
    pub aggregator_ms: u64,
    /// Execution timeout (ms).
    #[norito(default)]
    pub exec_ms: u64,
    /// Witness collection timeout (ms).
    #[norito(default)]
    pub witness_ms: u64,
}

/// Observational `NPoS` repair fanout stake-coverage snapshot.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, Default)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct SumeragiNposRepairCoverageStatus {
    /// Last height for which a repair fanout selection was recorded.
    #[norito(default)]
    pub last_repair_height: u64,
    /// Last view for which a repair fanout selection was recorded.
    #[norito(default)]
    pub last_repair_view: u64,
    /// Operator-facing reason label for the latest repair selection.
    #[norito(default)]
    pub reason: String,
    /// Number of peers selected for the latest repair fanout.
    #[norito(default)]
    pub selected_repair_peer_count: u64,
    /// Required stake quorum threshold in basis points.
    #[norito(default)]
    pub required_stake_quorum_bps: u16,
    /// Selected repair fanout stake coverage in basis points.
    #[norito(default)]
    pub selected_stake_coverage_bps: u16,
    /// Whether the latest selected fanout reached the stake quorum threshold.
    #[norito(default)]
    pub reached_stake_quorum_coverage: bool,
}

/// Canonical Sumeragi V1 status surface exposed by `/v1/sumeragi/status`.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, Default)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct SumeragiV1StatusWire {
    /// Current consensus height.
    #[norito(default)]
    pub height: u64,
    /// Current consensus view.
    #[norito(default)]
    pub view: u64,
    /// Current protocol phase (`proposal`, `prepare`, `commit`, or `pending_finality`).
    #[norito(default)]
    pub phase: String,
    /// Current leader index in the canonical validator ordering.
    #[norito(default)]
    pub leader_index: u64,
    /// Highest QC summary.
    #[norito(default)]
    pub highest_qc: SumeragiQcEntry,
    /// Locked QC summary.
    #[norito(default)]
    pub locked_qc: SumeragiQcEntry,
    /// Pending finality block hash when a commit certificate is waiting for local payload.
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub pending_finality: Option<HashOf<BlockHeader>>,
    /// Active validator-set id when known.
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub validator_set_id: Option<ValidatorSetId>,
    /// Active quorum policy when it can be derived from local status.
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub quorum_policy: Option<QuorumPolicy>,
    /// Local payload status label.
    #[norito(default)]
    pub payload_status: String,
    /// RBC/payload transport status label.
    #[norito(default)]
    pub rbc_status: String,
}

/// Cached standalone lane-block consensus session status.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct SumeragiLaneBlockSessionStatus {
    /// Lane whose lane-local block is being certified.
    #[norito(default)]
    pub lane_id: LaneId,
    /// Dataspace bound to the lane-local block.
    #[norito(default)]
    pub dataspace_id: DataSpaceId,
    /// Exact lane incarnation bound to every proposal, vote, and certificate.
    pub lane_incarnation: Hash,
    /// Lane-local block height.
    #[norito(default)]
    pub lane_block_height: u64,
    /// Lane-local block view.
    #[norito(default)]
    pub lane_block_view: u64,
    /// Proposal hash identifying the cached session.
    pub proposal_hash: Hash,
    /// Whether the proposal artifact is cached locally.
    #[norito(default)]
    pub has_proposal: bool,
    /// Number of cached prepare votes.
    #[norito(default)]
    pub prepare_vote_count: u32,
    /// Number of cached commit votes.
    #[norito(default)]
    pub commit_vote_count: u32,
    /// Whether a prepare QC is cached.
    #[norito(default)]
    pub has_prepare_qc: bool,
    /// Whether a commit QC is cached.
    #[norito(default)]
    pub has_commit_qc: bool,
    /// Whether this peer has a pending local commit-vote opportunity.
    #[norito(default)]
    pub pending_commit_vote_request: bool,
    /// Whether this session is ready to drain as a committed lane block.
    #[norito(default)]
    pub pending_committed_session_drain: bool,
    /// Whether this session already drained to the committed-lane queue.
    #[norito(default)]
    pub committed_session_drained: bool,
    /// Validator count advertised by the session body.
    #[norito(default)]
    pub validator_count: u32,
    /// Minimum quorum advertised by the session body.
    #[norito(default)]
    pub min_quorum: u32,
}

/// Proposal-gate inputs from the most recent pacemaker evaluation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, Default)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "operator diagnostics expose independent proposal-gate booleans"
)]
pub struct SumeragiProposalGateStatus {
    /// Height currently considered by the proposal path.
    #[norito(default)]
    pub height: u64,
    /// View currently considered by the proposal path.
    #[norito(default)]
    pub view: u64,
    /// Number of locally queued transactions when the gate was evaluated.
    #[norito(default)]
    pub queue_len: u64,
    /// Total locally tracked pending blocks.
    #[norito(default)]
    pub pending_blocks_total: u64,
    /// Pending blocks considered blocking by proposal backpressure.
    #[norito(default)]
    pub pending_blocks_blocking: u64,
    /// Active pending blocks that still extend the local tip.
    #[norito(default)]
    pub active_pending_for_tip: u64,
    /// Whether transaction-queue capacity pressure is gating proposals.
    #[norito(default)]
    pub queue_saturated: bool,
    /// Whether active pending block state is gating proposals.
    #[norito(default)]
    pub active_pending: bool,
    /// Whether RBC backlog is gating proposals.
    #[norito(default)]
    pub rbc_backlog: bool,
    /// Whether lane relay backpressure is gating proposals.
    #[norito(default)]
    pub relay_backpressure: bool,
    /// Whether consensus worker queues are gating proposals.
    #[norito(default)]
    pub consensus_queue_backpressure: bool,
    /// Whether aggregate proposal backpressure defers proposal assembly.
    #[norito(default)]
    pub should_defer: bool,
    /// Whether deferral is only queue/consensus pacing.
    #[norito(default)]
    pub only_pacing_backpressure: bool,
    /// Whether a commit job is currently in flight.
    #[norito(default)]
    pub commit_inflight_active: bool,
    /// Whether the current height/view has a cached proposal.
    #[norito(default)]
    pub cached_proposal_present: bool,
    /// Whether the current height/view has a cached proposal hint.
    #[norito(default)]
    pub cached_proposal_hint_present: bool,
    /// Whether local round-liveness evidence exists for the current height/view.
    #[norito(default)]
    pub round_liveness_present: bool,
    /// Whether a local frontier owner still exists for this height/view.
    #[norito(default)]
    pub frontier_owner_present: bool,
    /// Whether missing-QC liveness recovery is active for this height/view.
    #[norito(default)]
    pub missing_qc_liveness_active: bool,
    /// Milliseconds since the last pacemaker proposal attempt.
    #[norito(default)]
    pub last_pacemaker_attempt_age_ms: u64,
    /// Milliseconds since the last successful proposal assembly.
    #[norito(default)]
    pub last_successful_proposal_age_ms: u64,
}

/// Compact Norito payload returned by Torii for `/v1/sumeragi/status`.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, Default)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "first-release consensus status wire exposes independent telemetry flags without compatibility aliases"
)]
pub struct SumeragiStatusWire {
    /// Canonical first-release consensus state.
    #[norito(default)]
    pub canonical: SumeragiV1StatusWire,
    /// Current runtime mode tag.
    #[norito(default)]
    pub mode_tag: String,
    /// Staged mode tag if activation is pending.
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub staged_mode_tag: Option<String>,
    /// Activation height for staged mode (if any).
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub staged_mode_activation_height: Option<u64>,
    /// Blocks elapsed since activation height passed without applying the staged mode (if any).
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub mode_activation_lag_blocks: Option<u64>,
    /// Whether runtime mode flips are currently allowed by configuration.
    #[norito(default)]
    pub mode_flip_kill_switch: bool,
    /// Whether the last flip attempt was blocked (e.g., by kill switch).
    #[norito(default)]
    pub mode_flip_blocked: bool,
    /// Total successful runtime mode flips.
    #[norito(default)]
    pub mode_flip_success_total: u64,
    /// Total failed mode flip attempts.
    #[norito(default)]
    pub mode_flip_fail_total: u64,
    /// Total mode flip attempts blocked by configuration.
    #[norito(default)]
    pub mode_flip_blocked_total: u64,
    /// Timestamp (ms since UNIX epoch) of the last attempted flip, if any.
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub last_mode_flip_timestamp_ms: Option<u64>,
    /// Last recorded flip error (if any).
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub last_mode_flip_error: Option<String>,
    /// Consensus handshake caps derived from runtime configuration (if computed).
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub consensus_caps: Option<SumeragiConsensusCapsStatus>,
    /// Effective minimum finality floor (ms).
    #[norito(default)]
    pub effective_min_finality_ms: u64,
    /// Effective block time (ms).
    #[norito(default)]
    pub effective_block_time_ms: u64,
    /// Effective commit time (ms).
    #[norito(default)]
    pub effective_commit_time_ms: u64,
    /// Effective pacing factor (basis points, `10_000` = 1.0x).
    #[norito(default)]
    pub effective_pacing_factor_bps: u64,
    /// Effective commit quorum timeout (ms).
    #[norito(default)]
    pub effective_commit_quorum_timeout_ms: u64,
    /// Effective availability timeout (ms).
    #[norito(default)]
    pub effective_availability_timeout_ms: u64,
    /// Effective pacemaker interval (ms).
    #[norito(default)]
    pub effective_pacemaker_interval_ms: u64,
    /// Effective `NPoS` timeouts (ms) when in `NPoS` mode.
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub effective_npos_timeouts: Option<SumeragiNposTimeoutsStatus>,
    /// Effective collector count (K) for the active mode.
    #[norito(default)]
    pub effective_collectors_k: u64,
    /// Effective redundant send fanout (r) for the active mode.
    #[norito(default)]
    pub effective_redundant_send_r: u64,
    /// Current leader index (topology position).
    pub leader_index: u64,
    /// `HighestQC` height.
    pub highest_qc_height: u64,
    /// `HighestQC` view.
    pub highest_qc_view: u64,
    /// `HighestQC` subject block hash when available.
    #[norito(skip_serializing_if = "Option::is_none")]
    pub highest_qc_subject: Option<HashOf<BlockHeader>>,
    /// `LockedQC` height.
    pub locked_qc_height: u64,
    /// `LockedQC` view.
    pub locked_qc_view: u64,
    /// `LockedQC` subject block hash when available.
    #[norito(skip_serializing_if = "Option::is_none")]
    pub locked_qc_subject: Option<HashOf<BlockHeader>>,
    /// Latest commit QC summary (best-effort).
    #[norito(default)]
    pub commit_qc: SumeragiQcStatus,
    /// Latest commit quorum signature tally (best-effort).
    #[norito(default)]
    pub commit_quorum: SumeragiCommitQuorumStatus,
    /// Total view-change proofs accepted (advanced the proof chain).
    #[norito(default)]
    pub view_change_proof_accepted_total: u64,
    /// Total view-change proofs ignored as stale/outdated.
    #[norito(default)]
    pub view_change_proof_stale_total: u64,
    /// Total view-change proofs rejected due to validation failures.
    #[norito(default)]
    pub view_change_proof_rejected_total: u64,
    /// Total local view-change suggestions emitted.
    #[norito(default)]
    pub view_change_suggest_total: u64,
    /// Total view changes installed locally (proof advanced).
    #[norito(default)]
    pub view_change_install_total: u64,
    /// View-change cause counters and last occurrence (best-effort).
    #[norito(default)]
    pub view_change_causes: SumeragiViewChangeCauseStatus,
    /// Total gossip fallback invocations.
    pub gossip_fallback_total: u64,
    /// Total proposals dropped due to locked QC gate.
    pub block_created_dropped_by_lock_total: u64,
    /// Total proposals rejected due to hint mismatches.
    pub block_created_hint_mismatch_total: u64,
    /// Total proposals rejected due to payload/header mismatches.
    pub block_created_proposal_mismatch_total: u64,
    /// Consensus message drop/deferral counters (best-effort).
    #[norito(default)]
    pub consensus_message_handling: SumeragiConsensusMessageHandlingStatus,
    /// Vote validation drop snapshot (best-effort).
    #[norito(default)]
    pub vote_validation_drops: SumeragiVoteValidationDropStatus,
    /// Total blocks rejected by the validation gate before voting.
    #[norito(default)]
    pub validation_reject_total: u64,
    /// Last validation-reject reason label (best-effort).
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub validation_reject_reason: Option<String>,
    /// Validation gate reject breakdown and last-occurrence snapshot.
    #[norito(default)]
    pub validation_rejects: SumeragiValidationRejectStatus,
    /// Peer consensus-key policy rejects and last-occurrence snapshot.
    #[norito(default)]
    pub peer_key_policy: SumeragiPeerKeyPolicyStatus,
    /// Block-sync roster selection counters.
    #[norito(default)]
    pub block_sync_roster: SumeragiBlockSyncRosterStatus,
    /// Total pacemaker proposal attempts deferred due to transaction queue backpressure.
    pub pacemaker_backpressure_deferrals_total: u64,
    /// Most recent proposal-gate inputs observed by the pacemaker tick loop.
    #[norito(default)]
    pub proposal_gate: SumeragiProposalGateStatus,
    /// Total commit-pipeline executions triggered by the pacemaker tick loop.
    #[norito(default)]
    pub commit_pipeline_tick_total: u64,
    /// Total DA deadline reschedules that moved transactions into later slots.
    #[norito(default)]
    pub da_reschedule_total: u64,
    /// Missing-block fetch counters after QC-first arrivals.
    #[norito(default)]
    pub missing_block_fetch: SumeragiMissingBlockFetchStatus,
    /// Total QCs deferred because payload was missing locally.
    #[norito(default)]
    pub qc_deferred_missing_payload_total: u64,
    /// Total deferred QCs resolved after payload arrival.
    #[norito(default)]
    pub qc_deferred_resolved_total: u64,
    /// Total deferred QCs expired after bounded retries.
    #[norito(default)]
    pub qc_deferred_expired_total: u64,
    /// Bounded missing-QC reacquire attempts before rotating views.
    #[norito(default)]
    pub consensus_missing_qc_reacquire_attempt_total: u64,
    /// Missing-QC reacquire attempts that triggered recovery before rotation.
    #[norito(default)]
    pub consensus_missing_qc_reacquire_success_total: u64,
    /// Missing-QC reacquire windows exhausted before controlled rotation.
    #[norito(default)]
    pub consensus_missing_qc_reacquire_exhausted_total: u64,
    /// Forced leader self-proposal attempts under missing-QC liveness watchdog.
    #[norito(default)]
    pub consensus_forced_proposal_attempt_total: u64,
    /// Forced leader self-proposal attempts that assembled a proposal.
    #[norito(default)]
    pub consensus_forced_proposal_success_total: u64,
    /// Range-pull escalations requested by dependency recovery.
    #[norito(default)]
    pub blocksync_range_pull_escalation_total: u64,
    /// Range-pull recoveries that succeeded.
    #[norito(default)]
    pub blocksync_range_pull_success_total: u64,
    /// Range-pull recoveries that expired without progress.
    #[norito(default)]
    pub blocksync_range_pull_failure_total: u64,
    /// Range-pull recovery tiers exhausted before broader recovery.
    #[norito(default)]
    pub blocksync_range_pull_candidate_exhausted_total: u64,
    /// Committed-edge conflicts reclassified as obsolete/non-actionable dependencies.
    #[norito(default)]
    pub committed_edge_conflict_obsolete_total: u64,
    /// Repeated roster-sidecar mismatch tuples reclassified as obsolete/non-actionable.
    #[norito(default)]
    pub roster_sidecar_mismatch_obsolete_total: u64,
    /// DA availability telemetry and last-satisfied snapshot.
    #[norito(default)]
    pub da_gate: SumeragiDaGateStatus,
    /// Kura persistence snapshot.
    #[norito(default)]
    pub kura_store: SumeragiKuraStoreStatus,
    /// RBC store snapshot.
    #[norito(default)]
    pub rbc_store: SumeragiRbcStoreStatus,
    /// Per-peer RBC payload mismatch counters.
    #[norito(default)]
    pub rbc_mismatch: SumeragiRbcMismatchStatus,
    /// Pending RBC stash snapshot.
    #[norito(default)]
    pub pending_rbc: SumeragiPendingRbcStatus,
    /// Current transaction queue depth.
    pub tx_queue_depth: u64,
    /// Configured transaction queue capacity.
    pub tx_queue_capacity: u64,
    /// Estimated retained transaction queue bytes.
    #[norito(default)]
    pub tx_queue_retained_bytes: u64,
    /// Configured retained transaction queue byte budget.
    #[norito(default)]
    pub tx_queue_max_retained_bytes: u64,
    /// Whether the transaction queue is saturated.
    pub tx_queue_saturated: bool,
    /// Whether the transaction queue is saturated by transaction count.
    #[norito(default)]
    pub tx_queue_saturated_by_count: bool,
    /// Whether the transaction queue is saturated by retained bytes.
    #[norito(default)]
    pub tx_queue_saturated_by_bytes: bool,
    /// Whether the oldest queued transaction exceeded the queue age budget.
    #[norito(default)]
    pub tx_queue_saturated_by_age: bool,
    /// Oldest queued transaction age in milliseconds.
    #[norito(default)]
    pub tx_queue_oldest_queued_age_ms: u64,
    /// Epoch length in blocks (`NPoS` mode; zero when not applicable).
    #[norito(default)]
    pub epoch_length_blocks: u64,
    /// Commit window deadline offset from epoch start (blocks; zero when not applicable).
    #[norito(default)]
    pub epoch_commit_deadline_offset: u64,
    /// Reveal window deadline offset from epoch start (blocks; zero when not applicable).
    #[norito(default)]
    pub epoch_reveal_deadline_offset: u64,
    /// PRF epoch seed used for deterministic leader/collector selection.
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub prf_epoch_seed: Option<[u8; 32]>,
    /// Height associated with the recorded PRF context.
    pub prf_height: u64,
    /// View associated with the recorded PRF context.
    pub prf_view: u64,
    /// Latest epoch index for which VRF penalties were recorded.
    pub vrf_penalty_epoch: u64,
    /// Number of validators that committed without revealing in the latest epoch snapshot.
    pub vrf_committed_no_reveal_total: u64,
    /// Number of validators that neither committed nor revealed in the latest epoch snapshot.
    pub vrf_no_participation_total: u64,
    /// Number of validators that revealed after the reveal window in the latest epoch snapshot.
    pub vrf_late_reveals_total: u64,
    /// Total consensus penalties applied (evidence-driven).
    #[norito(default)]
    pub consensus_penalties_applied_total: u64,
    /// Consensus evidence records pending activation before penalties apply.
    #[norito(default)]
    pub consensus_penalties_pending: u64,
    /// Total VRF penalties applied.
    #[norito(default)]
    pub vrf_penalties_applied_total: u64,
    /// VRF penalty snapshots pending activation.
    #[norito(default)]
    pub vrf_penalties_pending: u64,
    /// Deterministic membership snapshot.
    #[norito(default)]
    pub membership: SumeragiMembershipStatus,
    /// Membership mismatch snapshot.
    #[norito(default)]
    pub membership_mismatch: SumeragiMembershipMismatchStatus,
    /// Aggregated lane-level commitment snapshots.
    #[norito(default)]
    pub lane_commitments: Vec<SumeragiLaneCommitment>,
    /// Aggregated dataspace-level commitment snapshots.
    #[norito(default)]
    pub dataspace_commitments: Vec<SumeragiDataspaceCommitment>,
    /// Aggregated lane-level settlement commitments.
    #[norito(default)]
    pub lane_settlement_commitments: Vec<LaneBlockCommitment>,
    /// Relay envelopes capturing lane block headers, QCs, DA digests, and settlement proofs.
    #[norito(default)]
    pub lane_relay_envelopes: Vec<LaneRelayEnvelope>,
    /// Planned lane-local payload ownership and RBC instance identities.
    #[norito(default)]
    pub lane_payload_ownerships: Vec<SumeragiLanePayloadOwnership>,
    /// Certified standalone lane-local block summaries.
    #[norito(default)]
    pub committed_lane_blocks: Vec<SumeragiCommittedLaneBlock>,
    /// Cached standalone lane-local block consensus sessions.
    #[norito(default)]
    pub lane_block_sessions: Vec<SumeragiLaneBlockSessionStatus>,
    /// Count of lanes that still require a governance manifest.
    #[norito(default)]
    pub lane_governance_sealed_total: u32,
    /// Aliases of lanes that remain sealed (manifest missing).
    #[norito(default)]
    pub lane_governance_sealed_aliases: Vec<String>,
    /// Governance manifest readiness per lane.
    #[norito(default)]
    pub lane_governance: Vec<SumeragiLaneGovernance>,
    /// Worker-loop stage and queue depth snapshot.
    #[norito(default)]
    pub worker_loop: SumeragiWorkerLoopStatus,
    /// Commit inflight diagnostics snapshot.
    #[norito(default)]
    pub commit_inflight: SumeragiCommitInflightStatus,
    /// Commit-pipeline budget snapshot.
    #[norito(default)]
    pub commit_pipeline: SumeragiCommitPipelineStatus,
    /// DELIVER-to-next-proposal gap snapshot.
    #[norito(default)]
    pub round_gap: SumeragiRoundGapStatus,
    /// Observational `NPoS` repair fanout coverage, present only when locally recorded.
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub npos_repair_coverage: Option<SumeragiNposRepairCoverageStatus>,
}

/// Entry describing a QC snapshot used by `/v1/sumeragi/qc`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, Default)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct SumeragiQcEntry {
    /// Certified block height.
    pub height: Height,
    /// View in which the QC was formed.
    pub view: View,
    /// Subject block hash if known.
    #[norito(skip_serializing_if = "Option::is_none")]
    pub subject_block_hash: Option<HashOf<BlockHeader>>,
}

/// Norito payload returned by Torii for `/v1/sumeragi/qc`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct SumeragiQcSnapshot {
    /// `HighestQC` snapshot.
    pub highest_qc: SumeragiQcEntry,
    /// `LockedQC` snapshot.
    pub locked_qc: SumeragiQcEntry,
}

/// Minimal execution witness KV pair for SBV-AM prototypes.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ExecKv {
    /// Raw key bytes.
    pub key: Vec<u8>,
    /// Raw value bytes.
    pub value: Vec<u8>,
}

/// Execution witness containing reads and writes for SMT recomputation.
#[derive(Clone, Debug, PartialEq, Eq, Default, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ExecWitness {
    /// Witnessed reads during execution (key,value).
    pub reads: Vec<ExecKv>,
    /// Writes performed during execution (key,value). Overrides reads on conflict.
    pub writes: Vec<ExecKv>,
    /// FASTPQ transfer transcripts grouped per entry hash.
    pub fastpq_transcripts: Vec<TransferTranscriptBundle>,
    /// FASTPQ transition batches prepared for prover ingestion.
    pub fastpq_batches: Vec<FastpqTransitionBatch>,
}

/// Execution witness message bound to a specific block and round. Used on-wire.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
pub struct ExecWitnessMsg {
    /// Hash of the block the witness applies to.
    pub block_hash: HashOf<BlockHeader>,
    /// Height of the block.
    pub height: Height,
    /// View/round for which the witness applies.
    pub view: View,
    /// Epoch index (0 in permissioned mode).
    pub epoch: u64,
    /// The execution witness payload.
    pub witness: ExecWitness,
}

/// VRF commit used by the Sumeragi epoch-randomness path.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct VrfCommit {
    /// Epoch index to which the commit applies.
    pub epoch: u64,
    /// Hiding commitment to the reveal.
    pub commitment: [u8; 32],
    /// Signer index within the validator set.
    pub signer: ValidatorIndex,
    /// BLS signature over the canonical VRF-commit preimage.
    pub bls_sig: Vec<u8>,
}

/// VRF reveal used by the Sumeragi epoch-randomness path.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct VrfReveal {
    /// Epoch index to which the reveal applies.
    pub epoch: u64,
    /// Revealed preimage value.
    pub reveal: [u8; 32],
    /// Signer index within the validator set.
    pub signer: ValidatorIndex,
    /// BLS signature over the canonical VRF-reveal preimage.
    pub bls_sig: Vec<u8>,
}

/// Reconfiguration payload (permissioned governance path).
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct Reconfig {
    /// New validator roster (ordered deterministically).
    pub new_roster: Vec<PeerId>,
    /// First height at which the new set becomes active.
    pub activation_height: Height,
}

/// RBC payload encoding used for chunk distribution.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode, Default)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(tag = "encoding", content = "state", rename_all = "snake_case")]
pub enum RbcEncoding {
    /// Raw payload chunking without parity shards.
    #[default]
    Plain,
    /// RS16 stripe encoding with parity shards.
    Rs16,
}

impl RbcEncoding {
    /// Stable operator-facing label for the encoding.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Plain => "plain",
            Self::Rs16 => "rs16",
        }
    }
}

/// RBC init message for payload distribution scaffolding.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct RbcInit {
    /// Subject block hash.
    pub block_hash: HashOf<BlockHeader>,
    /// Height.
    pub height: Height,
    /// View.
    pub view: View,
    /// Epoch.
    pub epoch: u64,
    /// Commit roster snapshot for this RBC session.
    pub roster: Vec<PeerId>,
    /// Hash of the Norito-encoded roster snapshot.
    pub roster_hash: Hash,
    /// Total chunk count for the payload.
    pub total_chunks: u32,
    /// Payload chunk encoding.
    pub encoding: RbcEncoding,
    /// Configured shard/chunk size in bytes.
    pub chunk_size_bytes: u32,
    /// Canonical payload size before any RS16 padding.
    pub payload_size_bytes: u64,
    /// Data shards per RS16 stripe (`0` for plain chunking).
    pub data_shards: u16,
    /// Parity shards per RS16 stripe (`0` for plain chunking).
    pub parity_shards: u16,
    /// SHA-256 digests for each chunk (indexed by chunk position).
    pub chunk_digests: Vec<[u8; 32]>,
    /// Payload hash commitment (optional, when leader is also proposer).
    pub payload_hash: Hash,
    /// Merkle root of chunk digests for integrity proofs.
    pub chunk_root: Hash,
    /// Full block header used to recover signed payloads without `BlockCreated`.
    pub block_header: BlockHeader,
    /// Leader signature over the block header.
    pub leader_signature: BlockSignature,
}

/// RBC payload chunk.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct RbcChunk {
    /// Subject block hash.
    pub block_hash: HashOf<BlockHeader>,
    /// Height.
    pub height: Height,
    /// View.
    pub view: View,
    /// Epoch.
    pub epoch: u64,
    /// Chunk index (0-based).
    pub idx: u32,
    /// Chunk bytes.
    pub bytes: Vec<u8>,
}

/// Request the RBC INIT scaffold for a specific `(block_hash, height, view)` session.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode)]
pub struct RbcInitRequest {
    /// Subject block hash.
    pub block_hash: HashOf<BlockHeader>,
    /// Height.
    pub height: Height,
    /// View.
    pub view: View,
}

/// Request missing RBC payload chunks for a specific `(block_hash, height, view)` session.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct RbcChunkRequest {
    /// Subject block hash.
    pub block_hash: HashOf<BlockHeader>,
    /// Height.
    pub height: Height,
    /// View.
    pub view: View,
    /// Missing encoded chunk indices requested from the peer.
    pub missing_indices: Vec<u32>,
}

/// RBC READY signal.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct RbcReady {
    /// Subject block hash.
    pub block_hash: HashOf<BlockHeader>,
    /// Height.
    pub height: Height,
    /// View.
    pub view: View,
    /// Epoch.
    pub epoch: u64,
    /// Hash of the roster snapshot used to validate READY signatures.
    pub roster_hash: Hash,
    /// Merkle root of chunk digests for integrity proofs.
    pub chunk_root: Hash,
    /// Sender index within the active set.
    pub sender: ValidatorIndex,
    /// Signature authenticating the sender for this READY.
    pub signature: Vec<u8>,
}

/// READY signature included with RBC DELIVER to seed quorum recovery.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct RbcReadySignature {
    /// Sender index within the active set.
    pub sender: ValidatorIndex,
    /// Signature authenticating the sender for this READY.
    pub signature: Vec<u8>,
}

/// RBC DELIVER notification.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct RbcDeliver {
    /// Subject block hash.
    pub block_hash: HashOf<BlockHeader>,
    /// Height.
    pub height: Height,
    /// View.
    pub view: View,
    /// Epoch.
    pub epoch: u64,
    /// Hash of the roster snapshot used to validate DELIVER signatures.
    pub roster_hash: Hash,
    /// Merkle root of chunk digests for integrity proofs.
    pub chunk_root: Hash,
    /// Sender index within the active set.
    pub sender: ValidatorIndex,
    /// Signature authenticating the sender for this DELIVER.
    pub signature: Vec<u8>,
    /// READY signatures observed by the sender for this session.
    pub ready_signatures: Vec<RbcReadySignature>,
}

#[cfg(feature = "sumeragi-multiproof")]
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct BlockMultiproof {
    pub reads: Vec<ReadNode>,
    pub read_keys: Vec<Vec<u8>>, // canonical bytes for keys
    pub writes: Vec<WriteEntry>,
    pub per_tx_read_index: Option<Vec<TxReadSpan>>,
    pub proof_aux: Vec<u8>,
}

#[cfg(feature = "sumeragi-multiproof")]
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct ReadNode {
    /// Opaque encoding of a Verkle/SMT node needed for verification
    pub node_bytes: Vec<u8>,
}

#[cfg(feature = "sumeragi-multiproof")]
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct WriteEntry {
    pub key: Vec<u8>,
    pub value_bytes: Vec<u8>,
    pub pre_version: u64,
    pub new_version: u64,
}

#[cfg(feature = "sumeragi-multiproof")]
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct TxReadSpan {
    /// start index (inclusive) into `BlockMultiproof::read_keys`
    pub start: u32,
    /// end index (exclusive)
    pub end: u32,
}

// --- Helpers for Norito slice decoding bridges ---
fn decode_from_slice_canonical<T>(bytes: &[u8]) -> Result<(T, usize), norito::core::Error>
where
    T: DecodeAll + Encode,
{
    let mut slice: &[u8] = bytes;
    let value = T::decode_all(&mut slice)
        .map_err(|e| norito::core::Error::Message(format!("codec decode error: {e}")))?;
    let canonical = value.encode();
    if bytes.len() < canonical.len() {
        return Err(norito::core::Error::LengthMismatch);
    }
    if bytes[..canonical.len()] != canonical {
        return Err(norito::core::Error::Message("payload mismatch".into()));
    }
    Ok((value, canonical.len()))
}

macro_rules! impl_decode_from_slice_via_codec {
    ($t:ty) => {
        impl<'a> norito::core::DecodeFromSlice<'a> for $t {
            fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
                decode_from_slice_canonical(bytes)
            }
        }
    };
}

impl_decode_from_slice_via_codec!(QcRef);
impl_decode_from_slice_via_codec!(ValidatorSetId);
impl_decode_from_slice_via_codec!(RoundId);
impl_decode_from_slice_via_codec!(QuorumPolicy);
impl_decode_from_slice_via_codec!(BlockSubject);
impl_decode_from_slice_via_codec!(Vote);
impl_decode_from_slice_via_codec!(Certificate);
impl_decode_from_slice_via_codec!(PayloadRequest);
impl_decode_from_slice_via_codec!(PayloadResponse);
impl_decode_from_slice_via_codec!(ConsensusBlockHeader);
impl_decode_from_slice_via_codec!(Proposal);
impl_decode_from_slice_via_codec!(QcVote);
impl_decode_from_slice_via_codec!(QcAggregate);
impl_decode_from_slice_via_codec!(Qc);
impl_decode_from_slice_via_codec!(ExecKv);
impl_decode_from_slice_via_codec!(ExecWitness);
impl_decode_from_slice_via_codec!(Evidence);
impl_decode_from_slice_via_codec!(EvidencePayload);
impl_decode_from_slice_via_codec!(EvidenceKind);
impl_decode_from_slice_via_codec!(ExecWitnessMsg);
impl_decode_from_slice_via_codec!(VrfCommit);
impl_decode_from_slice_via_codec!(VrfReveal);
impl_decode_from_slice_via_codec!(Reconfig);
impl_decode_from_slice_via_codec!(RbcInit);
impl_decode_from_slice_via_codec!(RbcChunk);
impl_decode_from_slice_via_codec!(RbcReady);
impl_decode_from_slice_via_codec!(RbcReadySignature);
impl_decode_from_slice_via_codec!(RbcDeliver);
#[cfg(feature = "sumeragi-multiproof")]
impl_decode_from_slice_via_codec!(BlockMultiproof);
#[cfg(feature = "sumeragi-multiproof")]
impl_decode_from_slice_via_codec!(ReadNode);
#[cfg(feature = "sumeragi-multiproof")]
impl_decode_from_slice_via_codec!(WriteEntry);
#[cfg(feature = "sumeragi-multiproof")]
impl_decode_from_slice_via_codec!(TxReadSpan);
impl_decode_from_slice_via_codec!(ConsensusGenesisParams);
impl_decode_from_slice_via_codec!(NposGenesisParams);
impl_decode_from_slice_via_codec!(SumeragiMembershipStatus);
impl_decode_from_slice_via_codec!(SumeragiLaneCommitment);
impl_decode_from_slice_via_codec!(SumeragiDataspaceCommitment);
impl_decode_from_slice_via_codec!(SumeragiCommittedLaneBlock);
impl_decode_from_slice_via_codec!(SumeragiLanePayloadOwnership);
impl_decode_from_slice_via_codec!(LaneBlockDescriptorV1);
impl_decode_from_slice_via_codec!(LaneBlockProposalV1);
impl_decode_from_slice_via_codec!(LaneBlockVoteBodyV1);
impl_decode_from_slice_via_codec!(LaneBlockQcV1);
impl_decode_from_slice_via_codec!(SumeragiRuntimeUpgradeHook);
impl_decode_from_slice_via_codec!(SumeragiLaneGovernance);
impl_decode_from_slice_via_codec!(SumeragiV1StatusWire);
impl_decode_from_slice_via_codec!(SumeragiStatusWire);
impl_decode_from_slice_via_codec!(NativeAmxPhase);
impl_decode_from_slice_via_codec!(NativeAmxAttestationBodyV1);
impl_decode_from_slice_via_codec!(NativeAmxAttestationQcV1);
impl_decode_from_slice_via_codec!(NativeAmxLegRecord);
impl_decode_from_slice_via_codec!(NativeAmxAttestationBodyV2);
impl_decode_from_slice_via_codec!(NativeAmxAttestationQcV2);
impl_decode_from_slice_via_codec!(NativeAmxLegRecordV2);
impl_decode_from_slice_via_codec!(NativeAmxReceipt);

// Provide nicer `Debug` rendering for validator indices in test snapshots.
impl fmt::Display for CertPhase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            CertPhase::Prepare => "Prepare",
            CertPhase::Commit => "Commit",
            CertPhase::NewView => "NewView",
        };
        f.write_str(s)
    }
}

impl<'a> norito::core::DecodeFromSlice<'a> for LaneSettlementReceipt {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let mut input = bytes;
        let value = <Self as DecodeAll>::decode_all(&mut input)?;
        let consumed = bytes.len() - input.len();
        Ok((value, consumed))
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU64;

    use iroha_crypto::{Algorithm, KeyPair, MerkleTree, SignatureOf};
    use iroha_primitives::{bigint::BigInt, numeric::Numeric};
    use norito::core::DecodeFromSlice;

    use crate::consensus::VALIDATOR_SET_HASH_VERSION_V1;

    use super::*;

    fn dummy_hash() -> HashOf<BlockHeader> {
        HashOf::from_untyped_unchecked(Hash::prehashed([0u8; 32]))
    }

    fn checked_random_keypair() -> KeyPair {
        KeyPair::try_random().expect("generate checked consensus fixture keypair")
    }

    fn checked_random_keypair_with_algorithm(algorithm: Algorithm) -> KeyPair {
        KeyPair::try_random_with_algorithm(algorithm)
            .expect("generate checked consensus fixture keypair")
    }

    fn sample_roster() -> Vec<PeerId> {
        (0..3)
            .map(|_| {
                PeerId::new(
                    checked_random_keypair_with_algorithm(Algorithm::BlsNormal)
                        .public_key()
                        .clone(),
                )
            })
            .collect()
    }

    fn roster_hash(roster: &[PeerId]) -> Hash {
        Hash::new(roster.to_vec().encode())
    }

    fn sample_qc_ref() -> QcRef {
        QcRef {
            height: 4,
            view: 1,
            epoch: 1,
            subject_block_hash: dummy_hash(),
            phase: CertPhase::Prepare,
        }
    }

    fn sample_consensus_header() -> ConsensusBlockHeader {
        ConsensusBlockHeader {
            parent_hash: dummy_hash(),
            tx_root: Hash::new(b"tx_root"),
            state_root: Hash::new(b"state_root"),
            proposer: 1,
            height: 6,
            view: 3,
            epoch: 1,
            highest_qc: sample_qc_ref(),
        }
    }

    #[test]
    fn committed_lane_block_status_progress_policy_is_fail_closed() {
        for (status, executable) in [
            (COMMITTED_LANE_STATUS_AWAITING_EXECUTABLE_PAYLOAD, false),
            (
                COMMITTED_LANE_STATUS_PAYLOAD_AVAILABLE_AWAITING_EXECUTOR,
                true,
            ),
            (
                COMMITTED_LANE_STATUS_PAYLOAD_RECOVERED_AWAITING_STATE_APPLICATION,
                true,
            ),
            (
                COMMITTED_LANE_STATUS_PAYLOAD_PREFLIGHTED_AWAITING_STATE_APPLICATION,
                true,
            ),
            (COMMITTED_LANE_STATUS_STATE_APPLIED_BY_CANONICAL_BLOCK, true),
            (
                COMMITTED_LANE_STATUS_STATE_APPLIED_BY_DIRECT_EXECUTION,
                true,
            ),
        ] {
            assert!(
                committed_lane_block_status_counts_as_progress(status, executable),
                "{status} with matching availability should count as audited progress"
            );
        }

        assert!(!committed_lane_block_status_counts_as_progress(
            COMMITTED_LANE_STATUS_APPLICATION_RECEIPT_CONFLICTS_WITH_PREFLIGHT,
            false
        ));
        assert!(!committed_lane_block_status_counts_as_progress(
            COMMITTED_LANE_STATUS_AWAITING_PREDECESSOR_APPLICATION,
            false
        ));
        assert!(!committed_lane_block_status_counts_as_progress(
            COMMITTED_LANE_STATUS_PAYLOAD_PREFLIGHT_REJECTED_AWAITING_STATE_APPLICATION,
            true
        ));
        assert!(!committed_lane_block_status_counts_as_progress(
            COMMITTED_LANE_STATUS_PAYLOAD_PREFLIGHT_REJECTED_AWAITING_STATE_APPLICATION,
            false
        ));
        assert!(!committed_lane_block_status_counts_as_progress(
            COMMITTED_LANE_STATUS_AWAITING_PREDECESSOR_APPLICATION,
            true
        ));
        assert!(!committed_lane_block_status_counts_as_progress(
            "future_status",
            true
        ));
        assert!(!committed_lane_block_status_counts_as_progress(
            COMMITTED_LANE_STATUS_AWAITING_EXECUTABLE_PAYLOAD,
            true
        ));
        assert!(!committed_lane_block_status_counts_as_progress(
            COMMITTED_LANE_STATUS_PAYLOAD_RECOVERED_AWAITING_STATE_APPLICATION,
            false
        ));
        assert!(!committed_lane_block_status_counts_as_progress(
            COMMITTED_LANE_STATUS_STATE_APPLIED_BY_DIRECT_EXECUTION,
            false
        ));
    }

    fn sample_round_id() -> RoundId {
        let roster = sample_roster();
        RoundId {
            height: 6,
            view: 3,
            epoch: 1,
            validator_set_id: ValidatorSetId::from_roster(&roster),
        }
    }

    fn sample_block_subject() -> BlockSubject {
        BlockSubject {
            parent_block: dummy_hash(),
            block_hash: HashOf::from_untyped_unchecked(Hash::new(b"canonical-block")),
            payload_hash: Hash::new(b"canonical-payload"),
        }
    }

    fn max_positive_numeric() -> Numeric {
        let mut bytes = [0xff; 64];
        bytes[63] = 0x7f;
        Numeric::new(
            BigInt::from_twos_bytes(&bytes).expect("512-bit positive mantissa fits"),
            0,
        )
    }

    #[derive(Encode)]
    struct LegacyLaneBlockCommitment {
        block_height: u64,
        lane_id: LaneId,
        dataspace_id: DataSpaceId,
        tx_count: u64,
        total_local_micro: u128,
        total_xor_due_micro: u128,
        total_xor_after_haircut_micro: u128,
        total_xor_variance_micro: u128,
        swap_metadata: Option<LaneSwapMetadata>,
        receipts: Vec<LaneSettlementReceipt>,
    }

    fn sample_nexus_fee_receipt(source_id: [u8; 32]) -> NexusFeeReceipt {
        NexusFeeReceipt {
            version: 1,
            source_id,
            dataspace_id: DataSpaceId::new(7),
            lane_id: LaneId::new(1),
            block_height: 42,
            payer_account_id: crate::account::AccountId::new(
                checked_random_keypair_with_algorithm(Algorithm::Ed25519)
                    .public_key()
                    .clone(),
            ),
            fee_asset_id: "xor#universal".to_owned(),
            fee_amount: Numeric::new(1, 3),
            schedule: NexusFeeScheduleInputs {
                tx_bytes_len: 100,
                instruction_count: 1,
                gas_used: 0,
                base_fee: Numeric::zero(),
                per_byte_fee: Numeric::zero(),
                per_instruction_fee: Numeric::new(1, 3),
                per_gas_unit_fee: Numeric::zero(),
            },
        }
    }

    fn sample_entrypoint_hash(seed: u8) -> HashOf<crate::transaction::TransactionEntrypoint> {
        HashOf::from_untyped_unchecked(Hash::prehashed([seed; Hash::LENGTH]))
    }

    fn sample_native_amx_qc(
        phase: NativeAmxPhase,
        source_id: [u8; 32],
        plan_digest: Hash,
        coordinator: (LaneId, DataSpaceId),
        participant: (LaneId, DataSpaceId),
        validator_set: Vec<PeerId>,
    ) -> NativeAmxAttestationQcV2 {
        let (coordinator_lane_id, coordinator_dataspace_id) = coordinator;
        let (participant_lane_id, participant_dataspace_id) = participant;
        let participant_validator_count =
            u32::try_from(validator_set.len()).expect("fixture validator count fits u32");
        let participant_min_quorum = u32::try_from(
            validator_set
                .len()
                .saturating_sub(validator_set.len().saturating_sub(1) / 3)
                .max(1),
        )
        .expect("fixture validator quorum fits u32");
        let validator_set_hash = HashOf::new(&validator_set);
        let validator_set_pops = vec![vec![0x5A; 96]; validator_set.len()];
        let chain_id_hash = Hash::new(b"native-amx-model-chain");
        let coordinator_lane_incarnation = Hash::new(b"native-amx-model-coordinator");
        let participant_lane_incarnation = Hash::new(
            [
                b"native-amx-model-participant:".as_slice(),
                &participant_lane_id.as_u32().to_be_bytes(),
            ]
            .concat(),
        );
        let coordinator_proposal_hash = Hash::new(b"native-amx-model-proposal");
        NativeAmxAttestationQcV2 {
            body: NativeAmxAttestationBodyV2 {
                round: crate::block::consensus_v2::ConsensusRound {
                    context_id: crate::block::consensus_v2::HeightContextId(
                        HashOf::from_untyped_unchecked(Hash::new(b"native-amx-receipt-context")),
                    ),
                    height: 42,
                    view: 3,
                },
                epoch: 7,
                chain_id_hash,
                source_id,
                tx_entrypoint_hash: sample_entrypoint_hash(0x42),
                plan_digest,
                phase,
                coordinator_lane_id,
                coordinator_dataspace_id,
                coordinator_lane_incarnation,
                participant_lane_id,
                participant_dataspace_id,
                participant_lane_incarnation,
                participant_validator_set_hash: validator_set_hash,
                participant_validator_count,
                participant_min_quorum,
                authority_context_height: 42,
                planned_coordinator_block_height: 42,
                coordinator_lane_block_view: 3,
                coordinator_proposal_hash,
            },
            validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
            validator_set_hash,
            validator_set,
            validator_set_pops,
            signers_bitmap: vec![0b0000_0111],
            bls_aggregate_signature: vec![0xA5; 96],
        }
    }

    #[test]
    fn legacy_lane_block_commitment_decodes_with_empty_nexus_fee_receipts() {
        let legacy = LegacyLaneBlockCommitment {
            block_height: 42,
            lane_id: LaneId::new(1),
            dataspace_id: DataSpaceId::new(7),
            tx_count: 0,
            total_local_micro: 0,
            total_xor_due_micro: 0,
            total_xor_after_haircut_micro: 0,
            total_xor_variance_micro: 0,
            swap_metadata: None,
            receipts: Vec::new(),
        };

        let mut bytes = norito::to_bytes(&legacy).expect("legacy commitment encodes");
        // Older payloads used the same `LaneBlockCommitment` type-name schema
        // hash. The test-only legacy struct has a different Rust type name, so
        // patch the header to exercise payload compatibility rather than the
        // unrelated schema-name guard.
        bytes[6..22]
            .copy_from_slice(&<LaneBlockCommitment as norito::NoritoSerialize>::schema_hash());
        let decoded: LaneBlockCommitment =
            norito::decode_from_bytes(&bytes).expect("new commitment decodes legacy bytes");

        assert!(decoded.nexus_fee_receipts.is_empty());
        assert!(decoded.native_amx_receipts.is_empty());
    }

    #[test]
    fn nexus_fee_receipts_change_lane_block_commitment_hash_inputs() {
        let base = LaneBlockCommitment {
            block_height: 42,
            lane_id: LaneId::new(1),
            lane_incarnation: Hash::new(b"commitment-hash-test-incarnation"),
            dataspace_id: DataSpaceId::new(7),
            tx_count: 1,
            total_local_micro: 0,
            total_xor_due_micro: 0,
            total_xor_after_haircut_micro: 0,
            total_xor_variance_micro: 0,
            swap_metadata: None,
            receipts: Vec::new(),
            nexus_fee_receipts: vec![sample_nexus_fee_receipt([0x11; 32])],
            native_amx_receipts: Vec::new(),
        };
        let mut changed = base.clone();
        changed.nexus_fee_receipts[0].fee_amount = Numeric::new(2, 3);

        assert_ne!(Hash::new(base.encode()), Hash::new(changed.encode()));
    }

    #[test]
    fn native_amx_receipts_change_lane_block_commitment_hash_inputs() {
        let plan_digest = Hash::new(b"test-native-amx-plan");
        let source_id = [0xAB; 32];
        let coordinator_lane_id = LaneId::new(0);
        let coordinator_dataspace_id = DataSpaceId::UNIVERSAL;
        let validators = sample_roster();
        let base = LaneBlockCommitment {
            block_height: 42,
            lane_id: coordinator_lane_id,
            lane_incarnation: Hash::new(b"amx-commitment-test-incarnation"),
            dataspace_id: coordinator_dataspace_id,
            tx_count: 1,
            total_local_micro: 0,
            total_xor_due_micro: 0,
            total_xor_after_haircut_micro: 0,
            total_xor_variance_micro: 0,
            swap_metadata: None,
            receipts: Vec::new(),
            nexus_fee_receipts: Vec::new(),
            native_amx_receipts: vec![NativeAmxReceipt {
                version: 2,
                source_id,
                chain_id_hash: Hash::new(b"native-amx-model-chain"),
                plan_digest,
                lane_id: coordinator_lane_id,
                dataspace_id: coordinator_dataspace_id,
                lane_incarnation: Hash::new(b"native-amx-model-coordinator"),
                authority_context_height: 42,
                lane_block_height: 7,
                lane_block_view: 2,
                coordinator_proposal_hash: Hash::new(b"native-amx-model-proposal"),
                legs: vec![
                    NativeAmxLegRecordV2 {
                        lane_id: LaneId::new(7),
                        dataspace_id: DataSpaceId::new(7),
                        prepare_qc: sample_native_amx_qc(
                            NativeAmxPhase::Prepare,
                            source_id,
                            plan_digest,
                            (coordinator_lane_id, coordinator_dataspace_id),
                            (LaneId::new(7), DataSpaceId::new(7)),
                            validators.clone(),
                        ),
                        commit_qc: sample_native_amx_qc(
                            NativeAmxPhase::Commit,
                            source_id,
                            plan_digest,
                            (coordinator_lane_id, coordinator_dataspace_id),
                            (LaneId::new(7), DataSpaceId::new(7)),
                            validators.clone(),
                        ),
                    },
                    NativeAmxLegRecordV2 {
                        lane_id: LaneId::new(8),
                        dataspace_id: DataSpaceId::new(8),
                        prepare_qc: sample_native_amx_qc(
                            NativeAmxPhase::Prepare,
                            source_id,
                            plan_digest,
                            (coordinator_lane_id, coordinator_dataspace_id),
                            (LaneId::new(8), DataSpaceId::new(8)),
                            validators.clone(),
                        ),
                        commit_qc: sample_native_amx_qc(
                            NativeAmxPhase::Commit,
                            source_id,
                            plan_digest,
                            (coordinator_lane_id, coordinator_dataspace_id),
                            (LaneId::new(8), DataSpaceId::new(8)),
                            validators,
                        ),
                    },
                ],
            }],
        };
        let mut changed = base.clone();
        changed.native_amx_receipts[0].legs[1].commit_qc.body.phase = NativeAmxPhase::Prepare;

        assert_ne!(Hash::new(base.encode()), Hash::new(changed.encode()));
    }

    #[test]
    fn native_amx_attestation_preimage_is_domain_separated() {
        let body = NativeAmxAttestationBodyV1 {
            chain_id_hash: Hash::new(b"native-amx-model-chain"),
            source_id: [0x11; 32],
            tx_entrypoint_hash: sample_entrypoint_hash(0x12),
            plan_digest: Hash::new(b"plan"),
            phase: NativeAmxPhase::Prepare,
            coordinator_lane_id: LaneId::new(1),
            coordinator_dataspace_id: DataSpaceId::UNIVERSAL,
            coordinator_lane_incarnation: Hash::new(b"native-amx-model-coordinator"),
            participant_lane_id: LaneId::new(2),
            participant_dataspace_id: DataSpaceId::new(2),
            participant_lane_incarnation: Hash::new(b"native-amx-model-participant"),
            participant_validator_set_hash: HashOf::new(&Vec::<PeerId>::new()),
            participant_validator_count: 1,
            participant_min_quorum: 1,
            authority_context_height: 7,
            coordinator_lane_block_height: 3,
            coordinator_lane_block_view: 1,
            coordinator_proposal_hash: Hash::new(b"native-amx-model-proposal"),
        };
        let preimage = body.signature_preimage();
        assert!(preimage.starts_with(b"iroha:native-amx:v1"));
        assert!(preimage.len() > b"iroha:native-amx:v1".len());
    }

    #[test]
    fn native_amx_v2_attestation_preimage_binds_round_and_epoch() {
        let body = sample_native_amx_qc(
            NativeAmxPhase::Prepare,
            [0x31; 32],
            Hash::new(b"v2-context-bound-plan"),
            (LaneId::new(1), DataSpaceId::new(7)),
            (LaneId::new(2), DataSpaceId::new(8)),
            sample_roster(),
        )
        .body;
        let preimage = body.signature_preimage();
        let mut another_view = body;
        another_view.round.view = another_view.round.view.saturating_add(1);
        let mut another_epoch = body;
        another_epoch.epoch = another_epoch.epoch.saturating_add(1);

        assert!(preimage.starts_with(b"iroha:native-amx:v2"));
        assert_ne!(preimage, another_view.signature_preimage());
        assert_ne!(preimage, another_epoch.signature_preimage());
    }

    fn sample_lane_block_vote_body(phase: CertPhase) -> LaneBlockVoteBodyV1 {
        LaneBlockVoteBodyV1 {
            phase,
            lane_id: LaneId::new(7),
            dataspace_id: DataSpaceId::new(11),
            lane_incarnation: Hash::new(b"lane-consensus-model-fixture"),
            proposal_height: 12,
            lane_block_height: 13,
            lane_block_view: 2,
            proposal_hash: Hash::prehashed([0x21; Hash::LENGTH]),
            descriptor_hash: Hash::prehashed([0x22; Hash::LENGTH]),
            subject_hash: Hash::prehashed([0x23; Hash::LENGTH]),
            payload_ownership_hash: Hash::prehashed([0x24; Hash::LENGTH]),
            rbc_instance_hash: Hash::prehashed([0x25; Hash::LENGTH]),
            accepted_candidate_indices: vec![3, 1],
            accepted_transaction_hashes: vec![
                Hash::prehashed([0x26; Hash::LENGTH]),
                Hash::prehashed([0x27; Hash::LENGTH]),
            ],
            validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
            validator_set_hash: HashOf::new(&sample_roster()),
            validator_count: 3,
            min_quorum: 3,
            qc_mode_tag: "permissioned:lane:7:dataspace:11".to_string(),
        }
    }

    fn sample_lane_block_proposal() -> LaneBlockProposalV1 {
        let roster = sample_roster();
        let mut descriptor = LaneBlockDescriptorV1 {
            lane_id: LaneId::new(7),
            dataspace_id: DataSpaceId::new(11),
            lane_incarnation: Hash::new(b"lane-consensus-model-fixture"),
            proposal_height: 12,
            previous_lane_block_height: 12,
            previous_lane_block_descriptor_hash: Some(Hash::prehashed([0x20; Hash::LENGTH])),
            lane_block_height: 13,
            lane_block_view: 2,
            subject_hash: Hash::prehashed([0x23; Hash::LENGTH]),
            payload_ownership_hash: Hash::prehashed([0x24; Hash::LENGTH]),
            rbc_instance_hash: Hash::prehashed([0x25; Hash::LENGTH]),
            accepted_candidate_indices: vec![3, 1],
            accepted_transaction_hashes: vec![
                Hash::prehashed([0x26; Hash::LENGTH]),
                Hash::prehashed([0x27; Hash::LENGTH]),
            ],
            validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
            validator_set_hash: HashOf::new(&roster),
            validator_set: roster,
            validator_count: 3,
            min_quorum: 3,
            qc_mode_tag: "permissioned:lane:7:dataspace:11".to_string(),
            descriptor_hash: Hash::prehashed([0x00; Hash::LENGTH]),
        };
        descriptor.descriptor_hash = descriptor.computed_descriptor_hash();
        let mut proposal = LaneBlockProposalV1 {
            descriptor,
            proposal_hash: Hash::prehashed([0x00; Hash::LENGTH]),
            payload_block_hint: None,
        };
        proposal.proposal_hash = proposal.computed_proposal_hash();
        proposal
    }

    fn refresh_lane_block_descriptor_hash(proposal: &mut LaneBlockProposalV1) {
        proposal.descriptor.descriptor_hash = proposal.descriptor.computed_descriptor_hash();
    }

    #[test]
    fn lane_block_vote_body_signature_preimage_binds_phase_and_descriptor() {
        let body = sample_lane_block_vote_body(CertPhase::Prepare);
        let preimage = body.signature_preimage();

        assert!(preimage.starts_with(b"iroha:lane-block-vote:v1"));
        assert!(preimage.len() > b"iroha:lane-block-vote:v1".len());

        let mut commit_body = body.clone();
        commit_body.phase = CertPhase::Commit;
        assert_ne!(
            preimage,
            commit_body.signature_preimage(),
            "prepare and commit lane votes must be domain-separated"
        );

        let mut descriptor_drift = body;
        descriptor_drift.descriptor_hash = Hash::prehashed([0x29; Hash::LENGTH]);
        assert_ne!(
            preimage,
            descriptor_drift.signature_preimage(),
            "descriptor drift must change the lane vote preimage"
        );
    }

    #[test]
    fn lane_block_vote_body_signature_preimage_binds_replay_and_quorum_fields() {
        let body = sample_lane_block_vote_body(CertPhase::Prepare);
        let preimage = body.signature_preimage();

        let mut cases = Vec::<(&str, LaneBlockVoteBodyV1)>::new();

        let mut lane_drift = body.clone();
        lane_drift.lane_id = LaneId::new(8);
        cases.push(("lane id", lane_drift));

        let mut dataspace_drift = body.clone();
        dataspace_drift.dataspace_id = DataSpaceId::new(12);
        cases.push(("dataspace id", dataspace_drift));

        let mut proposal_height_drift = body.clone();
        proposal_height_drift.proposal_height =
            proposal_height_drift.proposal_height.saturating_add(1);
        cases.push(("proposal height", proposal_height_drift));

        let mut height_drift = body.clone();
        height_drift.lane_block_height = height_drift.lane_block_height.saturating_add(1);
        cases.push(("lane block height", height_drift));

        let mut view_drift = body.clone();
        view_drift.lane_block_view = view_drift.lane_block_view.saturating_add(1);
        cases.push(("lane block view", view_drift));

        let mut proposal_drift = body.clone();
        proposal_drift.proposal_hash = Hash::prehashed([0x31; Hash::LENGTH]);
        cases.push(("proposal hash", proposal_drift));

        let mut subject_drift = body.clone();
        subject_drift.subject_hash = Hash::prehashed([0x32; Hash::LENGTH]);
        cases.push(("subject hash", subject_drift));

        let mut ownership_drift = body.clone();
        ownership_drift.payload_ownership_hash = Hash::prehashed([0x33; Hash::LENGTH]);
        cases.push(("payload ownership hash", ownership_drift));

        let mut rbc_drift = body.clone();
        rbc_drift.rbc_instance_hash = Hash::prehashed([0x34; Hash::LENGTH]);
        cases.push(("rbc instance hash", rbc_drift));

        let mut candidate_indices_drift = body.clone();
        candidate_indices_drift.accepted_candidate_indices.reverse();
        cases.push(("accepted candidate indices", candidate_indices_drift));

        let mut transaction_hashes_drift = body.clone();
        transaction_hashes_drift
            .accepted_transaction_hashes
            .reverse();
        cases.push(("accepted transaction hashes", transaction_hashes_drift));

        let mut validator_hash_version_drift = body.clone();
        validator_hash_version_drift.validator_set_hash_version = validator_hash_version_drift
            .validator_set_hash_version
            .saturating_add(1);
        cases.push(("validator set hash version", validator_hash_version_drift));

        let mut validator_hash_drift = body.clone();
        validator_hash_drift.validator_set_hash =
            HashOf::from_untyped_unchecked(Hash::prehashed([0x35; Hash::LENGTH]));
        cases.push(("validator set hash", validator_hash_drift));

        let mut validator_count_drift = body.clone();
        validator_count_drift.validator_count =
            validator_count_drift.validator_count.saturating_add(1);
        cases.push(("validator count", validator_count_drift));

        let mut quorum_drift = body.clone();
        quorum_drift.min_quorum = quorum_drift.min_quorum.saturating_sub(1);
        cases.push(("minimum quorum", quorum_drift));

        let mut qc_mode_drift = body.clone();
        qc_mode_drift.qc_mode_tag.push_str(":drift");
        cases.push(("qc mode tag", qc_mode_drift));

        for (label, drifted) in cases {
            assert_ne!(
                preimage,
                drifted.signature_preimage(),
                "{label} drift must change the lane vote preimage"
            );
        }
    }

    #[test]
    fn lane_block_proposal_hashes_bind_predecessor_and_committee() {
        let proposal = sample_lane_block_proposal();

        assert_eq!(
            proposal.descriptor.computed_descriptor_hash(),
            proposal.descriptor.descriptor_hash
        );
        assert_eq!(proposal.computed_proposal_hash(), proposal.proposal_hash);

        let mut predecessor_drift = proposal.clone();
        predecessor_drift
            .descriptor
            .previous_lane_block_descriptor_hash = Some(Hash::prehashed([0x31; Hash::LENGTH]));
        assert_ne!(
            predecessor_drift.descriptor.computed_descriptor_hash(),
            proposal.descriptor.descriptor_hash,
            "predecessor descriptor drift must change descriptor identity"
        );

        let mut committee_drift = proposal.clone();
        committee_drift.descriptor.validator_set.reverse();
        assert_ne!(
            committee_drift.descriptor.computed_descriptor_hash(),
            proposal.descriptor.descriptor_hash,
            "committee order drift must change descriptor identity"
        );
    }

    #[test]
    fn lane_block_descriptor_hash_binds_replay_and_quorum_fields() {
        let descriptor = sample_lane_block_proposal().descriptor;
        let mut cases = Vec::<(&str, LaneBlockDescriptorV1)>::new();

        let mut lane_drift = descriptor.clone();
        lane_drift.lane_id = LaneId::new(8);
        cases.push(("lane id", lane_drift));

        let mut dataspace_drift = descriptor.clone();
        dataspace_drift.dataspace_id = DataSpaceId::new(12);
        cases.push(("dataspace id", dataspace_drift));

        let mut proposal_height_drift = descriptor.clone();
        proposal_height_drift.proposal_height =
            proposal_height_drift.proposal_height.saturating_add(1);
        cases.push(("proposal height", proposal_height_drift));

        let mut previous_height_drift = descriptor.clone();
        previous_height_drift.previous_lane_block_height = previous_height_drift
            .previous_lane_block_height
            .saturating_sub(1);
        cases.push(("previous lane block height", previous_height_drift));

        let mut predecessor_drift = descriptor.clone();
        predecessor_drift.previous_lane_block_descriptor_hash = None;
        cases.push(("previous descriptor hash", predecessor_drift));

        let mut height_drift = descriptor.clone();
        height_drift.lane_block_height = height_drift.lane_block_height.saturating_add(1);
        cases.push(("lane block height", height_drift));

        let mut view_drift = descriptor.clone();
        view_drift.lane_block_view = view_drift.lane_block_view.saturating_add(1);
        cases.push(("lane block view", view_drift));

        let mut subject_drift = descriptor.clone();
        subject_drift.subject_hash = Hash::prehashed([0x31; Hash::LENGTH]);
        cases.push(("subject hash", subject_drift));

        let mut ownership_drift = descriptor.clone();
        ownership_drift.payload_ownership_hash = Hash::prehashed([0x32; Hash::LENGTH]);
        cases.push(("payload ownership hash", ownership_drift));

        let mut rbc_drift = descriptor.clone();
        rbc_drift.rbc_instance_hash = Hash::prehashed([0x33; Hash::LENGTH]);
        cases.push(("rbc instance hash", rbc_drift));

        let mut candidate_indices_drift = descriptor.clone();
        candidate_indices_drift.accepted_candidate_indices.reverse();
        cases.push(("accepted candidate indices", candidate_indices_drift));

        let mut transaction_hashes_drift = descriptor.clone();
        transaction_hashes_drift
            .accepted_transaction_hashes
            .reverse();
        cases.push(("accepted transaction hashes", transaction_hashes_drift));

        let mut validator_hash_version_drift = descriptor.clone();
        validator_hash_version_drift.validator_set_hash_version = validator_hash_version_drift
            .validator_set_hash_version
            .saturating_add(1);
        cases.push(("validator set hash version", validator_hash_version_drift));

        let mut validator_hash_drift = descriptor.clone();
        validator_hash_drift.validator_set_hash =
            HashOf::from_untyped_unchecked(Hash::prehashed([0x34; Hash::LENGTH]));
        cases.push(("validator set hash", validator_hash_drift));

        let mut validator_set_drift = descriptor.clone();
        validator_set_drift.validator_set.reverse();
        cases.push(("validator set order", validator_set_drift));

        let mut validator_count_drift = descriptor.clone();
        validator_count_drift.validator_count =
            validator_count_drift.validator_count.saturating_add(1);
        cases.push(("validator count", validator_count_drift));

        let mut quorum_drift = descriptor.clone();
        quorum_drift.min_quorum = quorum_drift.min_quorum.saturating_sub(1);
        cases.push(("minimum quorum", quorum_drift));

        let mut qc_mode_drift = descriptor.clone();
        qc_mode_drift.qc_mode_tag.push_str(":drift");
        cases.push(("qc mode tag", qc_mode_drift));

        for (label, drifted) in cases {
            assert_ne!(
                drifted.computed_descriptor_hash(),
                descriptor.descriptor_hash,
                "{label} drift must change descriptor identity"
            );
        }
    }

    #[test]
    fn lane_block_proposal_hash_binds_descriptor_replay_and_quorum_fields() {
        let proposal = sample_lane_block_proposal();
        let mut cases = Vec::<(&str, LaneBlockProposalV1)>::new();

        let mut descriptor_hash_drift = proposal.clone();
        descriptor_hash_drift.descriptor.descriptor_hash = Hash::prehashed([0x31; Hash::LENGTH]);
        cases.push(("descriptor hash", descriptor_hash_drift));

        let mut lane_drift = proposal.clone();
        lane_drift.descriptor.lane_id = LaneId::new(8);
        refresh_lane_block_descriptor_hash(&mut lane_drift);
        cases.push(("lane id", lane_drift));

        let mut dataspace_drift = proposal.clone();
        dataspace_drift.descriptor.dataspace_id = DataSpaceId::new(12);
        refresh_lane_block_descriptor_hash(&mut dataspace_drift);
        cases.push(("dataspace id", dataspace_drift));

        let mut proposal_height_drift = proposal.clone();
        proposal_height_drift.descriptor.proposal_height = proposal_height_drift
            .descriptor
            .proposal_height
            .saturating_add(1);
        refresh_lane_block_descriptor_hash(&mut proposal_height_drift);
        cases.push(("proposal height", proposal_height_drift));

        let mut previous_height_drift = proposal.clone();
        previous_height_drift.descriptor.previous_lane_block_height = previous_height_drift
            .descriptor
            .previous_lane_block_height
            .saturating_sub(1);
        refresh_lane_block_descriptor_hash(&mut previous_height_drift);
        cases.push(("previous lane block height", previous_height_drift));

        let mut predecessor_drift = proposal.clone();
        predecessor_drift
            .descriptor
            .previous_lane_block_descriptor_hash = None;
        refresh_lane_block_descriptor_hash(&mut predecessor_drift);
        cases.push(("previous descriptor hash", predecessor_drift));

        let mut height_drift = proposal.clone();
        height_drift.descriptor.lane_block_height =
            height_drift.descriptor.lane_block_height.saturating_add(1);
        refresh_lane_block_descriptor_hash(&mut height_drift);
        cases.push(("lane block height", height_drift));

        let mut view_drift = proposal.clone();
        view_drift.descriptor.lane_block_view =
            view_drift.descriptor.lane_block_view.saturating_add(1);
        refresh_lane_block_descriptor_hash(&mut view_drift);
        cases.push(("lane block view", view_drift));

        let mut subject_drift = proposal.clone();
        subject_drift.descriptor.subject_hash = Hash::prehashed([0x32; Hash::LENGTH]);
        refresh_lane_block_descriptor_hash(&mut subject_drift);
        cases.push(("subject hash", subject_drift));

        let mut ownership_drift = proposal.clone();
        ownership_drift.descriptor.payload_ownership_hash = Hash::prehashed([0x33; Hash::LENGTH]);
        refresh_lane_block_descriptor_hash(&mut ownership_drift);
        cases.push(("payload ownership hash", ownership_drift));

        let mut rbc_drift = proposal.clone();
        rbc_drift.descriptor.rbc_instance_hash = Hash::prehashed([0x34; Hash::LENGTH]);
        refresh_lane_block_descriptor_hash(&mut rbc_drift);
        cases.push(("rbc instance hash", rbc_drift));

        let mut candidate_indices_drift = proposal.clone();
        candidate_indices_drift
            .descriptor
            .accepted_candidate_indices
            .reverse();
        refresh_lane_block_descriptor_hash(&mut candidate_indices_drift);
        cases.push(("accepted candidate indices", candidate_indices_drift));

        let mut transaction_hashes_drift = proposal.clone();
        transaction_hashes_drift
            .descriptor
            .accepted_transaction_hashes
            .reverse();
        refresh_lane_block_descriptor_hash(&mut transaction_hashes_drift);
        cases.push(("accepted transaction hashes", transaction_hashes_drift));

        let mut validator_hash_version_drift = proposal.clone();
        validator_hash_version_drift
            .descriptor
            .validator_set_hash_version = validator_hash_version_drift
            .descriptor
            .validator_set_hash_version
            .saturating_add(1);
        refresh_lane_block_descriptor_hash(&mut validator_hash_version_drift);
        cases.push(("validator set hash version", validator_hash_version_drift));

        let mut validator_hash_drift = proposal.clone();
        validator_hash_drift.descriptor.validator_set_hash =
            HashOf::from_untyped_unchecked(Hash::prehashed([0x35; Hash::LENGTH]));
        refresh_lane_block_descriptor_hash(&mut validator_hash_drift);
        cases.push(("validator set hash", validator_hash_drift));

        let mut validator_set_drift = proposal.clone();
        validator_set_drift.descriptor.validator_set.reverse();
        refresh_lane_block_descriptor_hash(&mut validator_set_drift);
        cases.push(("validator set order", validator_set_drift));

        let mut validator_count_drift = proposal.clone();
        validator_count_drift.descriptor.validator_count = validator_count_drift
            .descriptor
            .validator_count
            .saturating_add(1);
        refresh_lane_block_descriptor_hash(&mut validator_count_drift);
        cases.push(("validator count", validator_count_drift));

        let mut quorum_drift = proposal.clone();
        quorum_drift.descriptor.min_quorum = quorum_drift.descriptor.min_quorum.saturating_sub(1);
        refresh_lane_block_descriptor_hash(&mut quorum_drift);
        cases.push(("minimum quorum", quorum_drift));

        let mut qc_mode_drift = proposal.clone();
        qc_mode_drift.descriptor.qc_mode_tag.push_str(":drift");
        refresh_lane_block_descriptor_hash(&mut qc_mode_drift);
        cases.push(("qc mode tag", qc_mode_drift));

        for (label, drifted) in cases {
            assert_ne!(
                drifted.computed_proposal_hash(),
                proposal.proposal_hash,
                "{label} drift must change proposal identity"
            );
        }
    }

    #[test]
    fn lane_block_proposal_roundtrips_and_derives_vote_body() {
        let proposal = sample_lane_block_proposal();
        let encoded = norito::to_bytes(&proposal).expect("lane proposal encodes");
        let decoded: LaneBlockProposalV1 =
            norito::decode_from_bytes(&encoded).expect("lane proposal decodes");
        assert_eq!(decoded, proposal);

        let body = decoded.vote_body(CertPhase::Prepare);
        assert_eq!(body.proposal_hash, decoded.proposal_hash);
        assert_eq!(body.descriptor_hash, decoded.descriptor.descriptor_hash);
        assert_eq!(body.proposal_height, decoded.descriptor.proposal_height);
        assert_eq!(
            body.validator_set_hash,
            decoded.descriptor.computed_validator_set_hash()
        );
        assert_eq!(
            body.accepted_transaction_hashes,
            decoded.descriptor.accepted_transaction_hashes
        );
    }

    fn sample_proposal() -> Proposal {
        Proposal {
            header: sample_consensus_header(),
            payload_hash: Hash::new(b"payload"),
        }
    }

    fn sample_reconfig() -> Reconfig {
        let peers = (0..2)
            .map(|_| PeerId::new(checked_random_keypair().public_key().clone()))
            .collect();
        Reconfig {
            new_roster: peers,
            activation_height: 42,
        }
    }

    fn sample_rbc_init() -> RbcInit {
        let roster = sample_roster();
        let roster_hash = roster_hash(&roster);
        let chunk_digests = vec![[0x11; 32], [0x22; 32], [0x33; 32]];
        let chunk_root = MerkleTree::<[u8; 32]>::from_hashed_leaves_sha256(chunk_digests.clone())
            .root()
            .map(Hash::from)
            .expect("chunk root");
        let block_header = BlockHeader::new(
            NonZeroU64::new(6).expect("block height must be non-zero"),
            None,
            None,
            None,
            0,
            3,
        );
        let leader_key = checked_random_keypair_with_algorithm(Algorithm::BlsNormal);
        let (_, leader_private) = leader_key.into_parts();
        let leader_signature = BlockSignature::new(
            0,
            SignatureOf::try_from_hash(&leader_private, block_header.hash())
                .expect("checked RBC init leader fixture signature"),
        );
        RbcInit {
            block_hash: block_header.hash(),
            height: 6,
            view: 3,
            epoch: 1,
            roster,
            roster_hash,
            total_chunks: 3,
            encoding: RbcEncoding::Plain,
            chunk_size_bytes: 128,
            payload_size_bytes: 257,
            data_shards: 0,
            parity_shards: 0,
            chunk_digests,
            payload_hash: Hash::new(b"payload_hash"),
            chunk_root,
            block_header,
            leader_signature,
        }
    }

    fn sample_rbc_chunk() -> RbcChunk {
        RbcChunk {
            block_hash: dummy_hash(),
            height: 6,
            view: 3,
            epoch: 1,
            idx: 1,
            bytes: vec![1, 2, 3, 4],
        }
    }

    fn sample_rbc_init_request() -> RbcInitRequest {
        RbcInitRequest {
            block_hash: dummy_hash(),
            height: 6,
            view: 3,
        }
    }

    fn sample_rbc_chunk_request() -> RbcChunkRequest {
        RbcChunkRequest {
            block_hash: dummy_hash(),
            height: 6,
            view: 3,
            missing_indices: vec![0, 2, 5],
        }
    }

    fn sample_rbc_ready() -> RbcReady {
        let roster = sample_roster();
        RbcReady {
            block_hash: dummy_hash(),
            height: 6,
            view: 3,
            epoch: 1,
            roster_hash: roster_hash(&roster),
            chunk_root: Hash::prehashed([0xAA; Hash::LENGTH]),
            sender: 2,
            signature: vec![0x10, 0x11],
        }
    }

    fn sample_rbc_deliver() -> RbcDeliver {
        let roster = sample_roster();
        RbcDeliver {
            block_hash: dummy_hash(),
            height: 6,
            view: 3,
            epoch: 1,
            roster_hash: roster_hash(&roster),
            chunk_root: Hash::prehashed([0xAA; Hash::LENGTH]),
            sender: 2,
            signature: vec![0x21, 0x22],
            ready_signatures: vec![RbcReadySignature {
                sender: 1,
                signature: vec![0x31, 0x32],
            }],
        }
    }

    fn sample_vrf_commit() -> VrfCommit {
        VrfCommit {
            epoch: 7,
            commitment: [0xAB; 32],
            signer: 5,
            bls_sig: Vec::new(),
        }
    }

    fn sample_vrf_reveal() -> VrfReveal {
        VrfReveal {
            epoch: 7,
            reveal: [0xCD; 32],
            signer: 5,
            bls_sig: Vec::new(),
        }
    }

    #[test]
    fn qc_roundtrip_encode_decode() {
        let roster = sample_roster();
        let highest = sample_qc_ref();
        let cert = Qc {
            phase: CertPhase::NewView,
            subject_block_hash: highest.subject_block_hash,
            parent_state_root: Hash::prehashed([0u8; Hash::LENGTH]),
            post_state_root: Hash::prehashed([0u8; Hash::LENGTH]),
            height: highest.height,
            view: 7,
            epoch: 0,
            chain_order_hash: default_chain_order_hash(),
            rechain_seq: 0,
            mode_tag: PERMISSIONED_TAG.to_string(),
            highest_qc: Some(highest),
            validator_set_hash: HashOf::new(&roster),
            validator_set_hash_version: 1,
            validator_set: roster,
            aggregate: QcAggregate {
                signers_bitmap: vec![0xAA, 0x01],
                bls_aggregate_signature: vec![1, 2, 3],
            },
        };
        let bytes = cert.encode();
        let dec = Qc::decode(&mut &bytes[..]).expect("decode certificate");
        assert_eq!(cert, dec);
    }

    #[test]
    fn exec_witness_roundtrip_codec() {
        let w = ExecWitness {
            reads: vec![ExecKv {
                key: b"key:read".to_vec(),
                value: b"value-pre".to_vec(),
            }],
            writes: vec![ExecKv {
                key: b"key:write".to_vec(),
                value: b"value-post".to_vec(),
            }],
            fastpq_transcripts: Vec::new(),
            fastpq_batches: Vec::new(),
        };
        let bytes = w.encode();
        let dec = ExecWitness::decode(&mut &bytes[..]).expect("decode witness");
        assert_eq!(w, dec);
    }

    #[test]
    fn rbc_repair_requests_roundtrip_codec() {
        let init_request = sample_rbc_init_request();
        let init_bytes = init_request.encode();
        let init_decoded =
            RbcInitRequest::decode(&mut &init_bytes[..]).expect("decode RBC init request");
        assert_eq!(init_request, init_decoded);

        let chunk_request = sample_rbc_chunk_request();
        let chunk_bytes = chunk_request.encode();
        let chunk_decoded =
            RbcChunkRequest::decode(&mut &chunk_bytes[..]).expect("decode RBC chunk request");
        assert_eq!(chunk_request, chunk_decoded);
    }

    #[test]
    fn evidence_roundtrip_codec() {
        let roster = sample_roster();
        let ev = Evidence {
            kind: EvidenceKind::InvalidQc,
            payload: EvidencePayload::InvalidQc {
                certificate: Qc {
                    phase: CertPhase::Commit,
                    subject_block_hash: dummy_hash(),
                    parent_state_root: Hash::new(b"parent_root"),
                    post_state_root: Hash::new(b"post_root"),
                    height: 12,
                    view: 3,
                    epoch: 0,
                    chain_order_hash: default_chain_order_hash(),
                    rechain_seq: 0,
                    mode_tag: PERMISSIONED_TAG.to_string(),
                    highest_qc: None,
                    validator_set_hash: HashOf::new(&roster),
                    validator_set_hash_version: 1,
                    validator_set: roster,
                    aggregate: QcAggregate {
                        signers_bitmap: vec![0xFF],
                        bls_aggregate_signature: vec![4, 5, 6],
                    },
                },
                reason: "test".to_string(),
            },
        };
        let bytes = ev.encode();
        let dec = Evidence::decode(&mut &bytes[..]).expect("decode evidence");
        assert_eq!(ev, dec);
    }

    #[test]
    fn sumeragi_v2_equivocation_evidence_roundtrip_and_kind_id() {
        use super::super::consensus_v2 as v2;

        let mut peers = sample_roster();
        peers.sort();
        let roster = peers
            .into_iter()
            .map(|validator| v2::ValidatorPower {
                validator,
                power: 1,
            })
            .collect::<Vec<_>>();
        let context = v2::HeightContext {
            chain_id: crate::ChainId::from("v2-evidence-codec"),
            protocol_version: v2::PROTOCOL_VERSION,
            height: 1,
            epoch: 0,
            epoch_end_height: u64::MAX,
            mode: v2::ConsensusMode::Permissioned,
            parent_commit_qc: None,
            quorum: v2::DualQuorum::from_roster(&roster).expect("dual quorum"),
            roster,
            nexus_amx_context_hash: Hash::new(b"v2-evidence-codec-context"),
            da_layout: v2::DataAvailabilityLayout {
                encoding: v2::PayloadEncoding::Plain,
                chunk_size_bytes: 16,
                data_shards: 0,
                parity_shards: 0,
                max_payload_size_bytes: 64,
                max_chunk_count: 4,
            },
            leader_seed: [0x42; 32],
        };
        let round = v2::ConsensusRound {
            context_id: context.id(),
            height: 1,
            view: 0,
        };
        let subject = |seed| v2::BlockSubject {
            parent_block_hash: None,
            block_hash: HashOf::from_untyped_unchecked(Hash::prehashed([seed; 32])),
            payload_hash: Hash::prehashed([seed.wrapping_add(1); 32]),
        };
        let ev = Evidence {
            kind: EvidenceKind::SumeragiV2Equivocation,
            payload: EvidencePayload::SumeragiV2Equivocation(SumeragiV2EquivocationEvidence {
                context,
                proofs_of_possession: vec![vec![0xA5; 48]; 3],
                conflict: v2::SumeragiV2Equivocation::PhaseVote {
                    first: v2::Vote {
                        round,
                        phase: v2::GlobalPhase::Prepare,
                        subject: subject(0x31),
                        signer: 1,
                        signature: vec![0xB1; 96],
                    },
                    second: v2::Vote {
                        round,
                        phase: v2::GlobalPhase::Prepare,
                        subject: subject(0x32),
                        signer: 1,
                        signature: vec![0xB2; 96],
                    },
                },
            }),
        };
        let bytes = ev.encode();
        let decoded = Evidence::decode(&mut &bytes[..]).expect("decode v2 evidence");
        assert_eq!(decoded, ev);
        assert_eq!(EvidenceKind::SumeragiV2Equivocation as u8, 5);

        let schema = EvidencePayload::schema();
        let Some(Metadata::Enum(metadata)) = schema.get::<EvidencePayload>() else {
            panic!("evidence payload schema must remain an enum");
        };
        let v2 = metadata
            .variants
            .iter()
            .find(|variant| variant.tag == "SumeragiV2Equivocation")
            .expect("v2 evidence schema variant");
        assert_eq!(v2.discriminant, 4);
        assert_eq!(
            v2.ty,
            Some(core::any::TypeId::of::<SumeragiV2EquivocationEvidence>())
        );
        assert!(schema.contains_key::<SumeragiV2EquivocationEvidence>());
        let Some(Metadata::Enum(conflict)) =
            schema.get::<super::super::consensus_v2::SumeragiV2Equivocation>()
        else {
            panic!("v2 equivocation conflict schema must remain an enum");
        };
        assert_eq!(
            conflict
                .variants
                .iter()
                .map(|variant| (variant.tag.as_str(), variant.discriminant))
                .collect::<Vec<_>>(),
            vec![("proposal", 0), ("phase_vote", 1), ("timeout_vote", 2)]
        );
    }

    #[test]
    fn censorship_evidence_roundtrip_codec() {
        let key_pair = checked_random_keypair();
        let payload = crate::transaction::TransactionSubmissionReceiptPayload {
            tx_hash: HashOf::from_untyped_unchecked(Hash::prehashed([0xAA; 32])),
            entrypoint_hash: HashOf::from_untyped_unchecked(Hash::prehashed([0xAA; 32])),
            signed_transaction_hash: None,
            submitted_at_ms: 10,
            submitted_at_height: 2,
            signer: key_pair.public_key().clone(),
        };
        let receipt =
            crate::transaction::TransactionSubmissionReceipt::try_sign(payload, &key_pair)
                .expect("checked censorship evidence receipt fixture signature");
        let tx_hash = receipt.payload.tx_hash;
        let ev = Evidence {
            kind: EvidenceKind::Censorship,
            payload: EvidencePayload::Censorship {
                tx_hash,
                receipts: vec![receipt],
            },
        };
        let bytes = ev.encode();
        let dec = Evidence::decode(&mut &bytes[..]).expect("decode censorship evidence");
        assert_eq!(ev, dec);
    }

    #[test]
    fn evidence_record_roundtrip() {
        let ev = Evidence {
            kind: EvidenceKind::DoublePrepare,
            payload: EvidencePayload::DoubleVote {
                v1: QcVote {
                    phase: CertPhase::Prepare,
                    block_hash: dummy_hash(),
                    parent_state_root: Hash::prehashed([0u8; Hash::LENGTH]),
                    post_state_root: Hash::prehashed([0u8; Hash::LENGTH]),
                    height: 10,
                    view: 1,
                    epoch: 0,
                    chain_order_hash: default_chain_order_hash(),
                    rechain_seq: 0,
                    highest_qc: None,
                    signer: 2,
                    bls_sig: vec![],
                },
                v2: QcVote {
                    phase: CertPhase::Prepare,
                    block_hash: dummy_hash(),
                    parent_state_root: Hash::prehashed([0u8; Hash::LENGTH]),
                    post_state_root: Hash::prehashed([0u8; Hash::LENGTH]),
                    height: 10,
                    view: 1,
                    epoch: 0,
                    chain_order_hash: default_chain_order_hash(),
                    rechain_seq: 0,
                    highest_qc: None,
                    signer: 2,
                    bls_sig: vec![],
                },
            },
        };
        let rec = EvidenceRecord {
            evidence: ev,
            recorded_at_height: 11,
            recorded_at_view: 2,
            recorded_at_ms: 1_689_000,
            penalty_applied: false,
            penalty_cancelled: false,
            penalty_cancelled_at_height: None,
            penalty_applied_at_height: None,
            consensus_admitted_at_height: Some(11),
        };
        let bytes = rec.encode();
        let dec = EvidenceRecord::decode(&mut &bytes[..]).expect("decode evidence record");
        assert_eq!(rec, dec);
    }

    #[test]
    fn rbc_ready_decode_from_slice_matches_encode() {
        let ready = RbcReady {
            block_hash: dummy_hash(),
            height: 5,
            view: 1,
            epoch: 0,
            roster_hash: Hash::prehashed([0xAA; Hash::LENGTH]),
            chunk_root: Hash::prehashed([0u8; Hash::LENGTH]),
            sender: 2,
            signature: vec![9, 9, 9],
        };
        let canonical = ready.encode();
        let (decoded, used) =
            RbcReady::decode_from_slice(&canonical).expect("decode_from_slice ready");
        assert_eq!(ready, decoded);
        assert_eq!(used, canonical.len());
    }

    #[test]
    fn proposal_roundtrip_codec() {
        let prop = sample_proposal();
        let bytes = prop.encode();
        let dec = Proposal::decode(&mut &bytes[..]).expect("decode proposal");
        assert_eq!(prop, dec);
    }

    #[test]
    fn canonical_v1_consensus_types_roundtrip_codec() {
        let round = sample_round_id();
        let subject = sample_block_subject();
        let vote = Vote {
            phase: CertPhase::Prepare,
            round,
            subject,
            highest_qc: Some(sample_qc_ref()),
            signer: 1,
            bls_sig: vec![1, 2, 3],
        };
        let certificate = Certificate {
            phase: CertPhase::Commit,
            round,
            subject,
            quorum_policy: QuorumPolicy::PermissionedCount(4),
            highest_qc: Some(sample_qc_ref()),
            signers_bitmap: vec![0b0000_0111],
            bls_aggregate_signature: vec![4, 5, 6],
        };
        let request = PayloadRequest {
            round,
            block_hash: subject.block_hash,
            payload_hash: subject.payload_hash,
        };
        let response = PayloadResponse {
            request,
            payload: vec![7, 8, 9],
        };

        for encoded in [
            round.encode(),
            subject.encode(),
            vote.encode(),
            certificate.encode(),
            request.encode(),
            response.encode(),
        ] {
            assert!(!encoded.is_empty(), "canonical type should encode");
        }
        assert_eq!(Vote::decode(&mut &vote.encode()[..]).expect("vote"), vote);
        assert_eq!(
            Certificate::decode(&mut &certificate.encode()[..]).expect("certificate"),
            certificate
        );
        assert_eq!(
            PayloadResponse::decode(&mut &response.encode()[..]).expect("payload response"),
            response
        );
    }

    #[test]
    fn canonical_v1_status_wire_roundtrip_codec() {
        let status = SumeragiV1StatusWire {
            height: 12,
            view: 3,
            phase: "pending_finality".to_owned(),
            leader_index: 2,
            highest_qc: SumeragiQcEntry {
                height: 11,
                view: 1,
                subject_block_hash: Some(dummy_hash()),
            },
            locked_qc: SumeragiQcEntry {
                height: 10,
                view: 0,
                subject_block_hash: Some(dummy_hash()),
            },
            pending_finality: Some(dummy_hash()),
            validator_set_id: Some(sample_round_id().validator_set_id),
            quorum_policy: Some(QuorumPolicy::PermissionedCount(4)),
            payload_status: "waiting_for_local_payload".to_owned(),
            rbc_status: "pending".to_owned(),
        };
        let encoded = status.encode();
        let decoded = SumeragiV1StatusWire::decode(&mut &encoded[..]).expect("status decodes");
        assert_eq!(decoded, status);

        let (decoded_from_slice, used) =
            SumeragiV1StatusWire::decode_from_slice(&encoded).expect("status decodes from slice");
        assert_eq!(decoded_from_slice, status);
        assert_eq!(used, encoded.len());
    }

    fn checked_seeded_peer_id(seed: u8) -> PeerId {
        PeerId::new(
            KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
                .expect("fixture seed must produce a keypair")
                .public_key()
                .clone(),
        )
    }

    fn sample_lane_payload_ownership_with_replay_material() -> SumeragiLanePayloadOwnership {
        let mut validator_set = vec![checked_seeded_peer_id(1), checked_seeded_peer_id(2)];
        validator_set.sort();
        let validator_count = u32::try_from(validator_set.len()).expect("validator count fits u32");
        let mut ownership = SumeragiLanePayloadOwnership {
            proposal_height: 12,
            proposal_view: 3,
            lane_id: LaneId::new(7),
            dataspace_id: DataSpaceId::new(42),
            lane_incarnation: Hash::new(b"lane-ownership-model-fixture"),
            lane_block_height: 2,
            lane_block_view: 1,
            subject_hash: Hash::new(b"lane subject placeholder"),
            qc_mode_tag: "test-lane-qc-mode".to_string(),
            accepted_candidate_indices: vec![0, 2],
            accepted_transaction_hashes: vec![
                Hash::new(b"lane accepted tx 0"),
                Hash::new(b"lane accepted tx 2"),
            ],
            previous_lane_block_height: 1,
            previous_lane_block_descriptor_hash: Some(Hash::new(b"lane predecessor descriptor")),
            lane_block_descriptor_hash: Some(Hash::new(b"lane block descriptor placeholder")),
            lane_block_descriptor_validator_set: validator_set,
            lane_block_descriptor_validator_count: validator_count,
            lane_block_descriptor_min_quorum: validator_count,
            payload_ownership_hash: Hash::new(b"lane payload ownership placeholder"),
            rbc_instance_hash: Hash::new(b"lane rbc instance placeholder"),
        };
        let replay_hashes = ownership
            .compute_replay_hashes()
            .expect("replay hashes compute for canonical lane ownership");
        ownership.subject_hash = replay_hashes.subject_hash;
        ownership.payload_ownership_hash = replay_hashes.payload_ownership_hash;
        ownership.rbc_instance_hash = replay_hashes.rbc_instance_hash;
        ownership.lane_block_descriptor_hash = Some(replay_hashes.lane_block_descriptor_hash);
        ownership
    }

    #[test]
    fn lane_payload_ownership_replay_material_validates_canonical_hashes() {
        let ownership = sample_lane_payload_ownership_with_replay_material();
        let replay_hashes = ownership
            .compute_replay_hashes()
            .expect("canonical replay material should hash");

        assert_eq!(ownership.subject_hash, replay_hashes.subject_hash);
        assert_eq!(
            ownership.payload_ownership_hash,
            replay_hashes.payload_ownership_hash
        );
        assert_eq!(ownership.rbc_instance_hash, replay_hashes.rbc_instance_hash);
        assert_eq!(
            ownership.lane_block_descriptor_hash,
            Some(replay_hashes.lane_block_descriptor_hash)
        );
        ownership
            .validate_replay_material()
            .expect("canonical replay material should validate");
    }

    #[test]
    fn lane_payload_ownership_replay_material_rejects_accepted_hash_drift() {
        let mut ownership = sample_lane_payload_ownership_with_replay_material();
        ownership.accepted_transaction_hashes[0] = Hash::new(b"forged accepted tx 0");

        assert_eq!(
            ownership.validate_replay_material(),
            Err(SumeragiLanePayloadOwnershipReplayError::SubjectHashMismatch)
        );
    }

    #[test]
    fn lane_payload_ownership_replay_material_rejects_proposal_height_drift() {
        let mut ownership = sample_lane_payload_ownership_with_replay_material();
        ownership.proposal_height = ownership.proposal_height.saturating_add(1);

        assert_eq!(
            ownership.validate_replay_material(),
            Err(SumeragiLanePayloadOwnershipReplayError::DescriptorHashMismatch)
        );
    }

    #[test]
    fn lane_payload_ownership_replay_material_rejects_defaulted_candidate_hashes() {
        let mut ownership = sample_lane_payload_ownership_with_replay_material();
        ownership.accepted_transaction_hashes.clear();

        assert_eq!(
            ownership.validate_replay_material(),
            Err(SumeragiLanePayloadOwnershipReplayError::CandidateHashCountMismatch)
        );
    }

    #[test]
    fn lane_payload_ownership_replay_material_rejects_defaulted_predecessor_height() {
        let mut ownership = sample_lane_payload_ownership_with_replay_material();
        ownership.previous_lane_block_height = 0;

        assert_eq!(
            ownership.validate_replay_material(),
            Err(SumeragiLanePayloadOwnershipReplayError::PreviousLaneBlockHeightMismatch)
        );
    }

    #[test]
    fn lane_payload_ownership_replay_material_rejects_missing_non_genesis_predecessor_hash() {
        let mut ownership = sample_lane_payload_ownership_with_replay_material();
        assert!(ownership.previous_lane_block_height > 0);
        ownership.previous_lane_block_descriptor_hash = None;

        assert_eq!(
            ownership.validate_replay_material(),
            Err(SumeragiLanePayloadOwnershipReplayError::MissingDescriptorHash)
        );
    }

    #[test]
    fn lane_payload_ownership_replay_material_rejects_missing_descriptor_hash() {
        let mut ownership = sample_lane_payload_ownership_with_replay_material();
        ownership.lane_block_descriptor_hash = None;

        assert_eq!(
            ownership.validate_replay_material(),
            Err(SumeragiLanePayloadOwnershipReplayError::MissingDescriptorHash)
        );
    }

    #[test]
    fn lane_payload_ownership_replay_material_rejects_empty_validator_set() {
        let mut ownership = sample_lane_payload_ownership_with_replay_material();
        ownership.lane_block_descriptor_validator_set.clear();
        ownership.lane_block_descriptor_validator_count = 0;
        ownership.lane_block_descriptor_min_quorum = 0;

        assert_eq!(
            ownership.validate_replay_material(),
            Err(SumeragiLanePayloadOwnershipReplayError::EmptyValidatorSet)
        );
    }

    #[test]
    fn lane_payload_ownership_replay_material_rejects_validator_count_drift() {
        let mut ownership = sample_lane_payload_ownership_with_replay_material();
        ownership.lane_block_descriptor_validator_count = ownership
            .lane_block_descriptor_validator_count
            .saturating_add(1);

        assert_eq!(
            ownership.validate_replay_material(),
            Err(SumeragiLanePayloadOwnershipReplayError::ValidatorCountMismatch)
        );
    }

    #[test]
    fn lane_payload_ownership_replay_material_rejects_noncanonical_validator_set() {
        let mut ownership = sample_lane_payload_ownership_with_replay_material();
        ownership.lane_block_descriptor_validator_set.reverse();

        assert_eq!(
            ownership.validate_replay_material(),
            Err(SumeragiLanePayloadOwnershipReplayError::ValidatorSetNotCanonical)
        );
    }

    #[test]
    fn lane_payload_ownership_replay_material_rejects_genesis_predecessor_descriptor() {
        let mut ownership = sample_lane_payload_ownership_with_replay_material();
        ownership.lane_block_height = 1;
        ownership.previous_lane_block_height = 0;
        ownership.previous_lane_block_descriptor_hash =
            Some(Hash::new(b"unexpected genesis predecessor descriptor"));

        assert_eq!(
            ownership.validate_replay_material(),
            Err(
                SumeragiLanePayloadOwnershipReplayError::UnexpectedGenesisPredecessorDescriptorHash
            )
        );
    }

    #[test]
    fn lane_payload_ownership_status_roundtrip_codec() {
        let ownership = SumeragiLanePayloadOwnership {
            proposal_height: 12,
            proposal_view: 3,
            lane_id: LaneId::new(7),
            dataspace_id: DataSpaceId::new(42),
            lane_incarnation: Hash::new(b"lane-ownership-model-fixture"),
            lane_block_height: 2,
            lane_block_view: 1,
            subject_hash: Hash::new(b"lane subject"),
            qc_mode_tag: "test-lane-qc-mode".to_string(),
            accepted_candidate_indices: vec![0, 2],
            accepted_transaction_hashes: vec![
                Hash::new(b"lane accepted tx 0"),
                Hash::new(b"lane accepted tx 2"),
            ],
            previous_lane_block_height: 1,
            previous_lane_block_descriptor_hash: Some(Hash::new(b"lane predecessor descriptor")),
            lane_block_descriptor_hash: Some(Hash::new(b"lane block descriptor")),
            lane_block_descriptor_validator_set: Vec::new(),
            lane_block_descriptor_validator_count: 0,
            lane_block_descriptor_min_quorum: 0,
            payload_ownership_hash: Hash::new(b"lane payload ownership"),
            rbc_instance_hash: Hash::new(b"lane rbc instance"),
        };
        let encoded = ownership.encode();
        let decoded = SumeragiLanePayloadOwnership::decode(&mut &encoded[..])
            .expect("lane payload ownership decodes");
        assert_eq!(decoded, ownership);

        let (decoded_from_slice, used) = SumeragiLanePayloadOwnership::decode_from_slice(&encoded)
            .expect("lane payload ownership decodes from slice");
        assert_eq!(decoded_from_slice, ownership);
        assert_eq!(used, encoded.len());
    }

    #[test]
    fn quorum_policy_enforces_strict_supermajority_boundaries() {
        assert_eq!(QuorumPolicy::permissioned_threshold(1), Some(1));
        assert_eq!(QuorumPolicy::permissioned_threshold(2), Some(2));
        assert_eq!(QuorumPolicy::permissioned_threshold(3), Some(3));
        assert_eq!(QuorumPolicy::permissioned_threshold(4), Some(3));
        assert_eq!(QuorumPolicy::permissioned_threshold(5), Some(4));
        assert_eq!(QuorumPolicy::permissioned_threshold(6), Some(5));
        assert_eq!(QuorumPolicy::permissioned_threshold(7), Some(5));
        assert_eq!(QuorumPolicy::permissioned_threshold(8), Some(6));
        assert_eq!(QuorumPolicy::permissioned_threshold(9), Some(7));
        assert_eq!(
            QuorumPolicy::permissioned_threshold(u32::MAX),
            Some(2_863_311_531)
        );
        assert_eq!(QuorumPolicy::permissioned_threshold(0), None);
        assert!(!QuorumPolicy::PermissionedCount(0).is_satisfied_by_count(u32::MAX));

        let count = QuorumPolicy::PermissionedCount(5);
        assert!(!count.is_satisfied_by_count(3));
        assert!(count.is_satisfied_by_count(4));
        assert!(!count.is_satisfied_by_count(6));
        assert!(!count.is_satisfied_by_stake(Some(Numeric::from(4_u64))));
        for validators in 1..=3 {
            let policy = QuorumPolicy::PermissionedCount(validators);
            assert!(!policy.is_satisfied_by_count(validators - 1));
            assert!(policy.is_satisfied_by_count(validators));
            assert!(!policy.is_satisfied_by_count(validators + 1));
        }
        let max_count = QuorumPolicy::PermissionedCount(u32::MAX);
        assert!(!max_count.is_satisfied_by_count(2_863_311_530));
        assert!(max_count.is_satisfied_by_count(2_863_311_531));

        let stake = QuorumPolicy::NposStake(Numeric::from(3_u64));
        assert!(!stake.is_satisfied_by_count(3));
        assert!(!stake.is_satisfied_by_stake(None));
        assert!(!stake.is_satisfied_by_stake(Some(Numeric::new(-1_i128, 0))));
        assert!(!stake.is_satisfied_by_stake(Some(Numeric::from(2_u64))));
        assert!(!stake.is_satisfied_by_stake(Some(Numeric::from(4_u64))));
        assert!(stake.is_satisfied_by_stake(Some(Numeric::new(201_u128, 2))));

        let fractional_stake = QuorumPolicy::NposStake(Numeric::new(15_u128, 1));
        assert!(!fractional_stake.is_satisfied_by_stake(Some(Numeric::new(10_u128, 1))));
        assert!(fractional_stake.is_satisfied_by_stake(Some(Numeric::new(101_u128, 2))));

        let tiny_fractional_stake = QuorumPolicy::NposStake(Numeric::new(3_u128, 2));
        assert!(!tiny_fractional_stake.is_satisfied_by_stake(Some(Numeric::new(2_u128, 2))));
        assert!(
            tiny_fractional_stake.is_satisfied_by_stake(Some(Numeric::new(
                200_000_000_000_000_000_000_000_001_u128,
                28,
            )))
        );

        let zero_total = QuorumPolicy::NposStake(Numeric::zero());
        assert!(!zero_total.is_satisfied_by_stake(Some(Numeric::from(1_u64))));

        let negative_total = QuorumPolicy::NposStake(Numeric::new(-3_i128, 0));
        assert!(!negative_total.is_satisfied_by_stake(Some(Numeric::from(1_u64))));

        let max_total = max_positive_numeric();
        let overflowing_stake = QuorumPolicy::NposStake(max_total.clone());
        assert!(!overflowing_stake.is_satisfied_by_stake(Some(max_total)));
    }

    #[test]
    fn qc_vote_roundtrip_codec_and_decode_from_slice() {
        let vote = QcVote {
            phase: CertPhase::Commit,
            block_hash: dummy_hash(),
            parent_state_root: Hash::new(b"parent_root"),
            post_state_root: Hash::new(b"post_root"),
            height: 7,
            view: 2,
            epoch: 0,
            chain_order_hash: default_chain_order_hash(),
            rechain_seq: 0,
            highest_qc: None,
            signer: 3,
            bls_sig: vec![0x01, 0x02],
        };
        let bytes = vote.encode();
        let dec = QcVote::decode(&mut &bytes[..]).expect("decode qc vote");
        assert_eq!(vote, dec);
        let (slice_dec, used) =
            QcVote::decode_from_slice(&bytes).expect("decode_from_slice qc vote");
        assert_eq!(vote, slice_dec);
        assert_eq!(used, bytes.len());
    }

    #[test]
    fn vrf_commit_roundtrip_codec() {
        let commit = sample_vrf_commit();
        let bytes = commit.encode();
        let dec = VrfCommit::decode(&mut &bytes[..]).expect("decode vrf commit");
        assert_eq!(commit, dec);
    }

    #[test]
    fn vrf_reveal_roundtrip_codec() {
        let reveal = sample_vrf_reveal();
        let bytes = reveal.encode();
        let dec = VrfReveal::decode(&mut &bytes[..]).expect("decode vrf reveal");
        assert_eq!(reveal, dec);
    }

    #[test]
    fn reconfig_roundtrip_codec() {
        let reconfig = sample_reconfig();
        let bytes = reconfig.encode();
        let dec = Reconfig::decode(&mut &bytes[..]).expect("decode reconfig");
        assert_eq!(reconfig, dec);
    }

    #[test]
    fn rbc_init_roundtrip_codec() {
        let init = sample_rbc_init();
        let bytes = init.encode();
        let dec = RbcInit::decode(&mut &bytes[..]).expect("decode rbc init");
        assert_eq!(init, dec);
    }

    #[test]
    fn rbc_chunk_roundtrip_codec() {
        let chunk = sample_rbc_chunk();
        let bytes = chunk.encode();
        let dec = RbcChunk::decode(&mut &bytes[..]).expect("decode rbc chunk");
        assert_eq!(chunk, dec);
    }

    #[test]
    fn rbc_ready_roundtrip_codec() {
        let ready = sample_rbc_ready();
        let bytes = ready.encode();
        let dec = RbcReady::decode(&mut &bytes[..]).expect("decode rbc ready");
        assert_eq!(ready, dec);
    }

    #[test]
    fn rbc_deliver_roundtrip_codec() {
        let deliver = sample_rbc_deliver();
        let bytes = deliver.encode();
        let dec = RbcDeliver::decode(&mut &bytes[..]).expect("decode rbc deliver");
        assert_eq!(deliver, dec);
    }
}
