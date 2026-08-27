//! Consensus-related data model DTOs for on-chain persistence.
pub use crate::block::consensus::{
    CertPhase, Qc, QcAggregate, QcRef, QcVote, SumeragiCommitPipelineStatus,
    SumeragiCommitQuorumStatus, SumeragiConsensusCapsStatus, SumeragiConsensusMessageHandlingEntry,
    SumeragiConsensusMessageHandlingStatus, SumeragiMembershipMismatchStatus,
    SumeragiPeerKeyPolicyStatus, SumeragiQcStatus, SumeragiRoundGapStatus,
    SumeragiViewChangeCauseStatus, SumeragiVoteValidationDropEntry,
    SumeragiVoteValidationDropPeerEntry, SumeragiVoteValidationDropReasonCount,
    SumeragiVoteValidationDropStatus, SumeragiWorkerLoopStatus, SumeragiWorkerQueueDepths,
    default_chain_order_hash,
};
/// Canonical Sumeragi v2 wire types.
pub use crate::block::consensus_v2 as v2;
use crate::prelude::*;
#[cfg(feature = "json")]
use crate::{DeriveJsonDeserialize, DeriveJsonSerialize};
use iroha_crypto::{Hash, PublicKey};
use iroha_primitives::numeric::Quantity;
use iroha_schema::{Ident, IntoSchema};
#[cfg(feature = "json")]
use mv::json::JsonKeyCodec;
use norito::codec::{Decode, Encode};
use std::str::FromStr;
/// Hash-version constant for validator set checkpoints.
pub const VALIDATOR_SET_HASH_VERSION_V1: u16 = 1;
/// Protocol version for the first-release global threshold beacon.
pub const GLOBAL_THRESHOLD_BEACON_VERSION_V1: u16 = 1;
/// Hard upper bound for one autonomous lane consensus committee.
///
/// This bounds proposal, vote, quorum-certificate, drain, and persisted proof
/// envelopes across configuration, runtime admission, and restart recovery.
pub const MAX_LANE_CONSENSUS_VALIDATORS: usize = 128;
// QC types are defined in `block::consensus` and re-exported above.
/// Signed validator set checkpoint used for bootstrap and audit.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct ValidatorSetCheckpoint {
    /// Block height covered by the checkpoint.
    pub height: u64,
    /// Block view (view-change index) covered by the checkpoint.
    pub view: u64,
    /// Block hash bound into the checkpoint.
    pub block_hash: HashOf<crate::block::BlockHeader>,
    /// Parent state root bound into the checkpoint.
    pub parent_state_root: Hash,
    /// Post-state root bound into the checkpoint.
    pub post_state_root: Hash,
    /// Hash of the vNext chain order bound into the checkpoint's aggregate signature.
    pub chain_order_hash: Hash,
    /// Re-chain sequence bound into the checkpoint's aggregate signature.
    pub rechain_seq: u64,
    /// Stable hash of the validator set encoded with [`VALIDATOR_SET_HASH_VERSION_V1`].
    pub validator_set_hash: HashOf<Vec<crate::peer::PeerId>>,
    /// Version of the validator-set hashing scheme.
    pub validator_set_hash_version: u16,
    /// Ordered validator set used to assemble the commit certificate.
    pub validator_set: Vec<crate::peer::PeerId>,
    /// Compact signer bitmap (LSB-first).
    pub signers_bitmap: Vec<u8>,
    /// BLS12-381 aggregate signature bytes (compressed).
    pub bls_aggregate_signature: Vec<u8>,
    /// Optional expiry height for the checkpoint (exclusive).
    #[norito(skip_serializing_if = "Option::is_none")]
    pub expires_at_height: Option<u64>,
}
impl ValidatorSetCheckpoint {
    /// Construct a checkpoint using the supplied block hash, validator set, and signatures.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        height: u64,
        view: u64,
        block_hash: HashOf<crate::block::BlockHeader>,
        parent_state_root: Hash,
        post_state_root: Hash,
        validator_set: Vec<crate::peer::PeerId>,
        signers_bitmap: Vec<u8>,
        bls_aggregate_signature: Vec<u8>,
        validator_set_hash_version: u16,
        expires_at_height: Option<u64>,
    ) -> Self {
        Self::new_with_chain_order(
            height,
            view,
            block_hash,
            default_chain_order_hash(),
            0,
            parent_state_root,
            post_state_root,
            validator_set,
            signers_bitmap,
            bls_aggregate_signature,
            validator_set_hash_version,
            expires_at_height,
        )
    }
    /// Construct a checkpoint with an explicit vNext chain-order binding.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn new_with_chain_order(
        height: u64,
        view: u64,
        block_hash: HashOf<crate::block::BlockHeader>,
        chain_order_hash: Hash,
        rechain_seq: u64,
        parent_state_root: Hash,
        post_state_root: Hash,
        validator_set: Vec<crate::peer::PeerId>,
        signers_bitmap: Vec<u8>,
        bls_aggregate_signature: Vec<u8>,
        validator_set_hash_version: u16,
        expires_at_height: Option<u64>,
    ) -> Self {
        let validator_set_hash = HashOf::new(&validator_set);
        Self {
            height,
            view,
            block_hash,
            parent_state_root,
            post_state_root,
            chain_order_hash,
            rechain_seq,
            validator_set_hash,
            validator_set_hash_version,
            validator_set,
            signers_bitmap,
            bls_aggregate_signature,
            expires_at_height,
        }
    }
}
/// Deterministic `NPoS` state effects embedded in a signed block.
///
/// These effects are applied as part of the committed block transition so every
/// peer replays the same threshold-beacon pulse, evidence, and penalty state.
#[derive(Debug, Clone, Default, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct NposConsensusEffects {
    /// Unique finalized global threshold-beacon pulse carried by this block.
    ///
    /// Partial signatures and reconstruction subsets never enter the signed
    /// block; validators independently verify this final signature against the
    /// active public DKG session before applying it.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub finalized_global_beacon_pulse: Option<FinalizedGlobalThresholdBeaconPulseV1>,
    /// Fully validated Sumeragi v2 equivocation evidence admitted by this
    /// signed block in canonical evidence-key order.
    #[norito(default)]
    #[norito(skip_serializing_if = "Vec::is_empty")]
    pub v2_evidence_admissions: Vec<crate::block::consensus::SumeragiV2EquivocationEvidence>,
    /// Penalty and marker actions applied by this block.
    #[norito(default)]
    #[norito(skip_serializing_if = "Vec::is_empty")]
    pub penalty_actions: Vec<NposPenaltyAction>,
}
impl NposConsensusEffects {
    /// Returns true when the bundle carries no committed state changes.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.finalized_global_beacon_pulse.is_none()
            && self.v2_evidence_admissions.is_empty()
            && self.penalty_actions.is_empty()
    }
}
impl Ord for NposConsensusEffects {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.encode().cmp(&other.encode())
    }
}
impl PartialOrd for NposConsensusEffects {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
/// A deterministic consensus-evidence slash action.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct NposConsensusSlashAction {
    /// Consensus evidence key.
    pub evidence_key: Vec<u8>,
    /// Signer index in the evidence roster.
    pub signer: u32,
    /// Peer identity resolved from the evidence roster.
    pub peer_id: crate::peer::PeerId,
    /// Public lane containing the validator registration.
    pub lane_id: crate::nexus::LaneId,
    /// Validator account to slash.
    pub validator: crate::account::AccountId,
    /// Slash identifier recorded in validator status.
    pub slash_id: Hash,
    /// Amount to slash.
    pub amount: Quantity,
}
/// Marker that a consensus evidence record's penalty was applied.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct NposMarkConsensusEvidenceAppliedAction {
    /// Consensus evidence key.
    pub evidence_key: Vec<u8>,
    /// Block height that applied the marker.
    pub height: u64,
}
/// Penalty or marker action applied by a committed `NPoS` effects bundle.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[norito(tag = "kind", content = "value", rename_all = "snake_case")]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub enum NposPenaltyAction {
    /// Slash a validator from consensus evidence.
    ConsensusSlash(NposConsensusSlashAction),
    /// Mark a consensus evidence record's penalty as applied.
    MarkConsensusEvidenceApplied(NposMarkConsensusEvidenceAppliedAction),
}
/// Snapshot of the election parameters used when selecting validators for an epoch.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct ValidatorElectionParameters {
    /// Maximum number of validators allowed in the elected set (0 = unlimited).
    pub max_validators: u32,
    /// Minimum self-bond required for eligibility (stake units).
    pub min_self_bond: Quantity,
    /// Minimum nomination bond required for delegators (stake units).
    pub min_nomination_bond: Quantity,
    /// Maximum percentage of total stake a single nominator may contribute to one validator.
    pub max_nominator_concentration_pct: u8,
    /// Seat band (percentage) for tie-breaking near the cut line.
    pub seat_band_pct: u8,
    /// Maximum percentage of validators that may share a common entity.
    pub max_entity_correlation_pct: u8,
    /// Finality margin (blocks) required when activating a newly elected set.
    pub finality_margin_blocks: u64,
}
/// Deterministic tie-break record used when ordering candidates.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct ValidatorTieBreak {
    /// Candidate peer identifier.
    pub peer_id: crate::peer::PeerId,
    /// Blake2b-derived score used to order candidates (lower is preferred).
    pub score: [u8; 32],
}
/// Election outcome for an epoch along with audit metadata.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct ValidatorElectionOutcome {
    /// Epoch index the elected set will service.
    pub epoch: u64,
    /// Height at which the election ran.
    pub snapshot_height: u64,
    /// Seed used to derive deterministic ordering/tie-breaks.
    pub seed: [u8; 32],
    /// Total candidates considered.
    pub candidates_total: u32,
    /// Ordered elected validator set.
    pub validator_set: Vec<crate::peer::PeerId>,
    /// Stable hash of the elected validator set.
    pub validator_set_hash: HashOf<Vec<crate::peer::PeerId>>,
    /// Parameters in effect for the election.
    pub params: ValidatorElectionParameters,
    /// Optional rejection or misconfiguration reason.
    #[norito(skip_serializing_if = "Option::is_none")]
    pub rejection_reason: Option<String>,
    /// Tie-break scores for auditability.
    #[norito(default)]
    #[norito(skip_serializing_if = "Vec::is_empty")]
    pub tie_break: Vec<ValidatorTieBreak>,
}
impl ValidatorElectionOutcome {
    /// Construct an empty election outcome for failed or skipped elections.
    #[must_use]
    pub fn empty(epoch: u64, snapshot_height: u64, seed: [u8; 32]) -> Self {
        let validator_set = Vec::new();
        let validator_set_hash = HashOf::new(&validator_set);
        Self {
            epoch,
            snapshot_height,
            seed,
            candidates_total: 0,
            validator_set,
            validator_set_hash,
            params: ValidatorElectionParameters {
                max_validators: 0,
                min_self_bond: Quantity::zero(),
                min_nomination_bond: Quantity::zero(),
                max_nominator_concentration_pct: 0,
                seat_band_pct: 0,
                max_entity_correlation_pct: 0,
                finality_margin_blocks: 0,
            },
            rejection_reason: Some("validator election failed".to_owned()),
            tie_break: Vec::new(),
        }
    }
}
/// Logical role for a consensus or committee key.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    IntoSchema,
    derive_more::Display,
)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize),
    norito(tag = "role", content = "value")
)]
pub enum ConsensusKeyRole {
    /// Validator signing key used for blocks/commit certificates.
    Validator,
    /// JDG/committee attestation key.
    Committee,
    /// Domain/endorsement committee key.
    Endorsement,
}
/// Identifier for a consensus/committee key (role + stable name).
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema, derive_more::Display,
)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[display("{role}:{name}")]
pub struct ConsensusKeyId {
    /// Logical role served by this key.
    pub role: ConsensusKeyRole,
    /// Human-friendly name (stable across rotations).
    pub name: Ident,
}
impl ConsensusKeyId {
    /// Construct a new key identifier.
    #[must_use]
    pub fn new(role: ConsensusKeyRole, name: impl Into<Ident>) -> Self {
        Self {
            role,
            name: name.into(),
        }
    }
}
/// HSM/keystore binding for a consensus key.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct HsmBinding {
    /// Provider identifier (e.g., `pkcs11`, `yubihsm`, `softkey` for tests).
    pub provider: String,
    /// Provider-specific key label or path.
    pub key_label: String,
    /// Optional slot/index inside the provider.
    #[norito(skip_serializing_if = "Option::is_none")]
    pub slot: Option<u16>,
}
/// Lifecycle state of a consensus key.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    IntoSchema,
    derive_more::Display,
)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize),
    norito(tag = "status", content = "value")
)]
pub enum ConsensusKeyStatus {
    /// Scheduled but not yet active for signing.
    Pending,
    /// Active and allowed for signing/verification.
    Active,
    /// Overlap/retirement window; still accepted until grace elapses.
    Retiring,
    /// Disabled or superseded; signatures should be rejected.
    Disabled,
}
/// Recorded consensus/committee key with lifecycle metadata.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct ConsensusKeyRecord {
    /// Identifier of the key (role + name).
    pub id: ConsensusKeyId,
    /// Public key material used for signatures.
    pub public_key: PublicKey,
    /// Optional Proof-of-Possession for BLS keys (required for BLS algorithms).
    #[norito(skip_serializing_if = "Option::is_none")]
    pub pop: Option<Vec<u8>>,
    /// First block height (inclusive) at which this key becomes valid.
    pub activation_height: u64,
    /// Optional block height (exclusive) at which this key expires.
    #[norito(skip_serializing_if = "Option::is_none")]
    pub expiry_height: Option<u64>,
    /// Optional HSM binding backing this key.
    #[norito(skip_serializing_if = "Option::is_none")]
    pub hsm: Option<HsmBinding>,
    /// Optional link to the key this record supersedes.
    #[norito(skip_serializing_if = "Option::is_none")]
    pub replaces: Option<ConsensusKeyId>,
    /// Declared lifecycle status.
    pub status: ConsensusKeyStatus,
}
#[cfg(feature = "json")]
impl JsonKeyCodec for ConsensusKeyId {
    fn encode_json_key(&self, out: &mut String) {
        norito::json::write_json_string(&self.to_string(), out);
    }
    fn decode_json_key(encoded: &str) -> Result<Self, norito::json::Error> {
        let (role_str, name_str) = encoded.split_once(':').ok_or_else(|| {
            norito::json::Error::Message("invalid consensus key id; expected role:name".into())
        })?;
        let role = match role_str {
            "Validator" => ConsensusKeyRole::Validator,
            "Committee" => ConsensusKeyRole::Committee,
            "Endorsement" => ConsensusKeyRole::Endorsement,
            other => return Err(norito::json::Error::unknown_field(other)),
        };
        let name = Ident::from_str(name_str).map_err(|err| {
            norito::json::Error::Message(format!("invalid consensus key name: {err}"))
        })?;
        Ok(ConsensusKeyId { role, name })
    }
}
impl ConsensusKeyRecord {
    /// Determine whether the key should be accepted at `height`, honoring overlap/expiry grace.
    #[must_use]
    pub fn is_live_at(
        &self,
        height: u64,
        overlap_grace_blocks: u64,
        expiry_grace_blocks: u64,
    ) -> bool {
        if matches!(self.status, ConsensusKeyStatus::Disabled) {
            return false;
        }
        if height < self.activation_height {
            return false;
        }
        if let Some(expiry) = self.expiry_height {
            let last_allowed = expiry.saturating_add(expiry_grace_blocks.max(overlap_grace_blocks));
            if height >= last_allowed {
                return false;
            }
        }
        true
    }
}

/// One canonically indexed public verification share in a global beacon DKG transcript.
///
/// Participant indices are one-based and the enclosing session must contain the
/// exact sequence `1..=committee_size`. `participant_seat_binding` commits to
/// the typed session, frozen ordered-roster commitment, and one-based seat.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct GlobalThresholdBeaconPublicShareV1 {
    /// Canonical one-based participant index.
    pub index: u16,
    /// Deterministic binding of this one-based seat to the frozen ordered roster.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub participant_seat_binding: [u8; 32],
    /// Canonical compressed BLS12-381 G2 public verification share.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub public_key_share: [u8; 96],
}

/// Immutable bindings and height windows for one adaptive beacon DKG run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct GlobalThresholdBeaconDkgSessionV1 {
    /// Fixed first-release beacon protocol version.
    pub version: u16,
    /// Exact deployment identity derived from genesis.
    pub network_id: NetworkId,
    /// Unique threshold-beacon session identifier.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub session_id: [u8; 32],
    /// Hash of the frozen ordered DKG participant roster.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub roster_hash: [u8; 32],
    /// Exact `3f + 1` participant count.
    pub committee_size: u16,
    /// Exact `f + 1` reconstruction threshold (and coefficient count).
    pub threshold: u16,
    /// First height at which dealer commitments are accepted.
    pub start_height: u64,
    /// Exclusive end of the dealer-commitment window.
    pub sharing_end_height: u64,
    /// Exclusive end of the complaint window.
    pub complaints_end_height: u64,
    /// Exclusive end of the complaint-response window.
    pub responses_end_height: u64,
}

/// Schnorr proof that a dealer knows the constant-term exponent.
///
/// Since the constant coefficient commitment is exactly `g^s`, this proof is
/// what enforces `r(0) = u(0) = 0` in the augmented JF/Pedersen DKG.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct GlobalThresholdBeaconDkgConstantProofV1 {
    /// Canonical compressed G2 Schnorr nonce commitment.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub commitment: [u8; 96],
    /// Canonical big-endian scalar response.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub response: [u8; 32],
}

/// One dealer's Figure-5 augmented JF/Pedersen coefficient commitments.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct GlobalThresholdBeaconDkgDealerCommitmentV1 {
    /// Dealer's canonical one-based participant index.
    pub dealer_index: u16,
    /// `C_k = g^s_k h^r_k v^u_k` in ascending coefficient order.
    ///
    /// The list length must equal the reconstruction threshold. `C_0` is
    /// exactly `g^s_0` because both zero-polynomials have zero constant term.
    #[cfg_attr(
        feature = "json",
        norito(json = "crate::json_helpers::fixed_bytes::vec")
    )]
    pub coefficient_commitments: Vec<[u8; 96]>,
    /// Fiat-Shamir Schnorr proof of knowledge for the constant coefficient.
    pub constant_term_proof: GlobalThresholdBeaconDkgConstantProofV1,
}

/// Stable reason for a recipient complaint during adaptive beacon DKG.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(tag = "reason", content = "detail", rename_all = "snake_case")]
pub enum GlobalThresholdBeaconDkgComplaintReasonV1 {
    /// The dealer did not deliver the recipient's private `(s, r, u)` share.
    MissingPrivateShare,
    /// The delivered share failed the composite coefficient equation.
    InvalidPrivateShare,
}

/// Public complaint against one dealer/recipient share edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct GlobalThresholdBeaconDkgComplaintV1 {
    /// Dealer accused by the complaint.
    pub dealer_index: u16,
    /// Recipient which raised the complaint.
    pub complainant_index: u16,
    /// Hash of the exact canonical dealer commitment under dispute.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub dealer_commitment_hash: [u8; 32],
    /// Stable complaint classification.
    pub reason: GlobalThresholdBeaconDkgComplaintReasonV1,
    /// Canonical complaint ID recomputed from this record and session bindings.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub complaint_id: [u8; 32],
}

/// Dealer's public response to one DKG complaint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct GlobalThresholdBeaconDkgComplaintResponseV1 {
    /// Exact complaint being answered.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub complaint_id: [u8; 32],
    /// Accused dealer index copied from the complaint.
    pub dealer_index: u16,
    /// Recipient index copied from the complaint.
    pub recipient_index: u16,
    /// Canonical scalar `s_i(recipient)`.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub s_share: [u8; 32],
    /// Canonical scalar `r_i(recipient)`.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub r_share: [u8; 32],
    /// Canonical scalar `u_i(recipient)`.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub u_share: [u8; 32],
}

/// Canonical public audit transcript for a completed adaptive beacon DKG.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct GlobalThresholdBeaconDkgTranscriptV1 {
    /// Immutable session and height-window bindings.
    pub session: GlobalThresholdBeaconDkgSessionV1,
    /// Independently domain-derived compressed G2 generator `h`.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub generator_h: [u8; 96],
    /// Independently domain-derived compressed G2 generator `v`.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub generator_v: [u8; 96],
    /// Valid dealer broadcasts in strictly ascending dealer order.
    pub dealer_commitments: Vec<GlobalThresholdBeaconDkgDealerCommitmentV1>,
    /// Complaints in canonical `(dealer, complainant)` order.
    pub complaints: Vec<GlobalThresholdBeaconDkgComplaintV1>,
    /// Valid public responses in canonical `(dealer, recipient)` order.
    pub complaint_responses: Vec<GlobalThresholdBeaconDkgComplaintResponseV1>,
    /// Locally derived qualified dealer indices in strictly ascending order.
    pub qualified_dealers: Vec<u16>,
    /// Hash of the complete canonical DKG event transcript above.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub event_hash: [u8; 32],
    /// Height at which qualification and the public key were finalized.
    pub finalized_at_height: u64,
}

/// Figure-3 proof attached to one adaptive threshold-BLS partial signature.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct GlobalThresholdBeaconPartialSignatureProofV1 {
    /// Composite-key Schnorr commitment `x` in compressed G2.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub x: [u8; 96],
    /// Partial-signature Schnorr commitment `y` in compressed G1.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub y: [u8; 48],
    /// Response for the `s` witness.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub z_s: [u8; 32],
    /// Response for the `r` witness.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub z_r: [u8; 32],
    /// Response for the `u` witness.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub z_u: [u8; 32],
}

/// One internal adaptive threshold-BLS signature share and its Figure-3 proof.
///
/// This object is used only while reconstructing the unique final signature;
/// it is never embedded in a finalized public pulse.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct GlobalThresholdBeaconPartialSignatureV1 {
    /// Typed beacon session identifier.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub session_id: [u8; 32],
    /// Canonical one-based signer index.
    pub signer_index: u16,
    /// `H0(m)^s(i) H1(m)^r(i)` in compressed G1.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub signature_share: [u8; 48],
    /// Three-witness non-interactive proof of correct share construction.
    pub proof: GlobalThresholdBeaconPartialSignatureProofV1,
}

/// Complete public commitment for one global threshold-beacon key session.
///
/// This DTO deliberately contains the complete ordered public-share transcript.
/// A consumer must recompute `transcript_hash`; accepting the supplied hash by
/// itself would permit a session to claim a roster or DKG transcript it did not
/// actually use. Beacon sessions are a separate typed cryptographic domain from
/// Parliament timelock-release sessions.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct GlobalThresholdBeaconKeySessionV1 {
    /// Fixed protocol version; must equal [`GLOBAL_THRESHOLD_BEACON_VERSION_V1`].
    pub version: u16,
    /// Exact deployment identity derived from the genesis header.
    pub network_id: NetworkId,
    /// Unique non-zero identifier for this beacon key session.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub session_id: [u8; 32],
    /// Hash of the frozen ordered validator roster.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub roster_hash: [u8; 32],
    /// Exact `3f + 1` committee size.
    pub committee_size: u16,
    /// Exact `f + 1` reconstruction threshold.
    pub threshold: u16,
    /// Canonical compressed BLS12-381 G2 group public key.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub group_public_key: [u8; 96],
    /// Complete verification-share list in exact one-based index order.
    pub public_shares: Vec<GlobalThresholdBeaconPublicShareV1>,
    /// Complete augmented JF/Pedersen DKG audit transcript.
    pub adaptive_dkg: GlobalThresholdBeaconDkgTranscriptV1,
    /// Non-zero commitment to all qualified DKG contributions.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub dkg_contribution_hash: [u8; 32],
    /// Recomputed commitment to the complete typed public transcript.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub transcript_hash: [u8; 32],
}

/// Finalized-chain point bound into a global threshold-beacon pulse.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct GlobalThresholdBeaconChainAnchorV1 {
    /// Finalized block height authenticated by the pulse.
    pub height: u64,
    /// Exact finalized block-header hash at `height`.
    pub block_hash: HashOf<crate::block::BlockHeader>,
}

/// One finalized pulse from the canonical global threshold beacon.
///
/// There is intentionally no signer bitmap, share list, or reconstruction
/// subset. A threshold-BLS final signature is unique for its message and group
/// key, so the public result cannot vary with the subset used internally to
/// reconstruct it. `seed` and `pulse_id` are redundant audit fields which must
/// be recomputed from the verified final signature.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct FinalizedGlobalThresholdBeaconPulseV1 {
    /// Fixed protocol version; must equal [`GLOBAL_THRESHOLD_BEACON_VERSION_V1`].
    pub version: u16,
    /// Exact deployment identity derived from the genesis header.
    pub network_id: NetworkId,
    /// Beacon key session which produced this pulse.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub session_id: [u8; 32],
    /// Frozen roster hash inherited from the key session.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub roster_hash: [u8; 32],
    /// Recomputed public-transcript commitment inherited from the key session.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub transcript_hash: [u8; 32],
    /// Consensus height at which this pulse is finalized.
    pub height: u64,
    /// Canonical fixed protocol round (zero in V1), independent of consensus view.
    pub round: u64,
    /// Exact finalized-chain point authenticated by this pulse.
    pub finalized_chain_anchor: GlobalThresholdBeaconChainAnchorV1,
    /// Canonical compressed BLS12-381 G1 final group signature.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub signature: [u8; 48],
    /// Unique 32-byte seed derived from the verified final signature.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub seed: [u8; 32],
    /// Canonical identifier recomputed from the signed pulse and final signature.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub pulse_id: [u8; 32],
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_primitives::numeric::Numeric;
    #[derive(Encode)]
    struct ForgedNposConsensusSlashAction {
        evidence_key: Vec<u8>,
        signer: u32,
        peer_id: crate::peer::PeerId,
        lane_id: crate::nexus::LaneId,
        validator: crate::account::AccountId,
        slash_id: Hash,
        amount: Numeric,
    }
    fn checked_random_keypair() -> KeyPair {
        KeyPair::try_random().expect("generate checked consensus DTO fixture keypair")
    }
    fn checked_random_keypair_with_algorithm(algorithm: Algorithm) -> KeyPair {
        KeyPair::try_random_with_algorithm(algorithm)
            .expect("generate checked consensus DTO fixture keypair")
    }
    fn threshold_beacon_network(marker: u8) -> NetworkId {
        NetworkId::from_genesis_hash(HashOf::<crate::block::BlockHeader>::from_untyped_unchecked(
            Hash::prehashed([marker; Hash::LENGTH]),
        ))
    }
    fn threshold_beacon_session_fixture() -> GlobalThresholdBeaconKeySessionV1 {
        let network_id = threshold_beacon_network(0x81);
        let session_id = [0x22; 32];
        let roster_hash = [0x33; 32];
        let dkg_session = GlobalThresholdBeaconDkgSessionV1 {
            version: GLOBAL_THRESHOLD_BEACON_VERSION_V1,
            network_id,
            session_id,
            roster_hash,
            committee_size: 4,
            threshold: 2,
            start_height: 1,
            sharing_end_height: 10,
            complaints_end_height: 20,
            responses_end_height: 30,
        };
        let dealer_commitments = (1_u16..=4)
            .map(|dealer_index| GlobalThresholdBeaconDkgDealerCommitmentV1 {
                dealer_index,
                coefficient_commitments: vec![[dealer_index as u8 + 0x20; 96]; 2],
                constant_term_proof: GlobalThresholdBeaconDkgConstantProofV1 {
                    commitment: [dealer_index as u8 + 0x30; 96],
                    response: [dealer_index as u8 + 0x40; 32],
                },
            })
            .collect();
        GlobalThresholdBeaconKeySessionV1 {
            version: GLOBAL_THRESHOLD_BEACON_VERSION_V1,
            network_id,
            session_id,
            roster_hash,
            committee_size: 4,
            threshold: 2,
            group_public_key: [0x44; 96],
            public_shares: (1_u16..=4)
                .map(|index| GlobalThresholdBeaconPublicShareV1 {
                    index,
                    participant_seat_binding: [index as u8; 32],
                    public_key_share: [index as u8 + 0x50; 96],
                })
                .collect(),
            adaptive_dkg: GlobalThresholdBeaconDkgTranscriptV1 {
                session: dkg_session,
                generator_h: [0x61; 96],
                generator_v: [0x62; 96],
                dealer_commitments,
                complaints: Vec::new(),
                complaint_responses: Vec::new(),
                qualified_dealers: vec![1, 2, 3, 4],
                event_hash: [0x66; 32],
                finalized_at_height: 30,
            },
            dkg_contribution_hash: [0x66; 32],
            transcript_hash: [0x77; 32],
        }
    }
    fn threshold_beacon_pulse_fixture() -> FinalizedGlobalThresholdBeaconPulseV1 {
        let session = threshold_beacon_session_fixture();
        FinalizedGlobalThresholdBeaconPulseV1 {
            version: GLOBAL_THRESHOLD_BEACON_VERSION_V1,
            network_id: session.network_id,
            session_id: session.session_id,
            roster_hash: session.roster_hash,
            transcript_hash: session.transcript_hash,
            height: 42,
            round: 0,
            finalized_chain_anchor: GlobalThresholdBeaconChainAnchorV1 {
                height: 41,
                block_hash: HashOf::<crate::block::BlockHeader>::from_untyped_unchecked(
                    Hash::prehashed([0xAA; Hash::LENGTH]),
                ),
            },
            signature: [0xBB; 48],
            seed: [0xCC; 32],
            pulse_id: [0xDD; 32],
        }
    }
    #[test]
    fn threshold_beacon_session_and_pulse_norito_roundtrip() {
        let session = threshold_beacon_session_fixture();
        let encoded_session = session.encode();
        let decoded_session =
            GlobalThresholdBeaconKeySessionV1::decode(&mut encoded_session.as_slice())
                .expect("decode threshold beacon session");
        assert_eq!(decoded_session, session);

        let pulse = threshold_beacon_pulse_fixture();
        let encoded_pulse = pulse.encode();
        let decoded_pulse =
            FinalizedGlobalThresholdBeaconPulseV1::decode(&mut encoded_pulse.as_slice())
                .expect("decode finalized threshold beacon pulse");
        assert_eq!(decoded_pulse, pulse);

        let partial = GlobalThresholdBeaconPartialSignatureV1 {
            session_id: session.session_id,
            signer_index: 2,
            signature_share: [0xE1; 48],
            proof: GlobalThresholdBeaconPartialSignatureProofV1 {
                x: [0xE2; 96],
                y: [0xE3; 48],
                z_s: [0xE4; 32],
                z_r: [0xE5; 32],
                z_u: [0xE6; 32],
            },
        };
        let encoded_partial = partial.encode();
        let decoded_partial =
            GlobalThresholdBeaconPartialSignatureV1::decode(&mut encoded_partial.as_slice())
                .expect("decode adaptive partial signature");
        assert_eq!(decoded_partial, partial);
    }
    #[cfg(feature = "json")]
    #[test]
    fn threshold_beacon_session_and_pulse_json_roundtrip_without_subset_metadata() {
        let session = threshold_beacon_session_fixture();
        let session_json = norito::json::to_json(&session).expect("encode beacon session JSON");
        let decoded_session: GlobalThresholdBeaconKeySessionV1 =
            norito::json::from_str(&session_json).expect("decode beacon session JSON");
        assert_eq!(decoded_session, session);

        let pulse = threshold_beacon_pulse_fixture();
        let pulse_json = norito::json::to_json(&pulse).expect("encode beacon pulse JSON");
        let decoded_pulse: FinalizedGlobalThresholdBeaconPulseV1 =
            norito::json::from_str(&pulse_json).expect("decode beacon pulse JSON");
        assert_eq!(decoded_pulse, pulse);
        assert!(!pulse_json.contains("signer"));
        assert!(!pulse_json.contains("subset"));
        assert!(!pulse_json.contains("share"));
    }
    #[test]
    fn negative_numeric_payload_cannot_decode_as_consensus_slash_amount() {
        let key_pair = checked_random_keypair();
        let peer_id = crate::peer::PeerId::new(key_pair.public_key().clone());
        let slash = ForgedNposConsensusSlashAction {
            evidence_key: vec![0xA5],
            signer: 0,
            peer_id,
            lane_id: crate::nexus::LaneId::SINGLE,
            validator: crate::account::AccountId::new(key_pair.public_key().clone()),
            slash_id: Hash::new(b"negative-consensus-slash"),
            amount: Numeric::new(-1_i32, 0),
        };
        let encoded = slash.encode();
        assert!(
            NposConsensusSlashAction::decode(&mut encoded.as_slice()).is_err(),
            "a negative signed payload must not decode as a consensus slash amount"
        );
    }
    #[test]
    fn validator_set_checkpoint_roundtrip_and_hash() {
        let kp_a = checked_random_keypair_with_algorithm(Algorithm::BlsNormal);
        let kp_b = checked_random_keypair_with_algorithm(Algorithm::BlsNormal);
        let validator_set = vec![
            crate::peer::PeerId::new(kp_a.public_key().clone()),
            crate::peer::PeerId::new(kp_b.public_key().clone()),
        ];
        let block_hash = HashOf::<crate::block::BlockHeader>::from_untyped_unchecked(
            iroha_crypto::Hash::prehashed([0xAA; 32]),
        );
        let parent_state_root = iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]);
        let post_state_root = iroha_crypto::Hash::prehashed([1u8; iroha_crypto::Hash::LENGTH]);
        let checkpoint = ValidatorSetCheckpoint::new(
            42,
            7,
            block_hash,
            parent_state_root,
            post_state_root,
            validator_set.clone(),
            vec![0x01],
            vec![0xAA, 0xBB],
            VALIDATOR_SET_HASH_VERSION_V1,
            None,
        );
        let expected_hash = HashOf::new(&validator_set);
        assert_eq!(checkpoint.validator_set_hash, expected_hash);
        assert_eq!(checkpoint.chain_order_hash, default_chain_order_hash());
        assert_eq!(checkpoint.rechain_seq, 0);
        let buf = checkpoint.encode();
        let decoded =
            ValidatorSetCheckpoint::decode(&mut &buf[..]).expect("validator checkpoint decodes");
        assert_eq!(decoded.height, 42);
        assert_eq!(decoded.view, 7);
        assert_eq!(decoded.parent_state_root, parent_state_root);
        assert_eq!(decoded.post_state_root, post_state_root);
        assert_eq!(decoded.chain_order_hash, default_chain_order_hash());
        assert_eq!(decoded.rechain_seq, 0);
        assert_eq!(decoded.validator_set_hash, expected_hash);
        assert_eq!(decoded.validator_set, validator_set);
        let chain_order_hash = iroha_crypto::Hash::new(b"checkpoint-chain-order");
        let explicit = ValidatorSetCheckpoint::new_with_chain_order(
            42,
            7,
            block_hash,
            chain_order_hash,
            5,
            parent_state_root,
            post_state_root,
            decoded.validator_set,
            vec![0x01],
            vec![0xAA, 0xBB],
            VALIDATOR_SET_HASH_VERSION_V1,
            None,
        );
        assert_eq!(explicit.chain_order_hash, chain_order_hash);
        assert_eq!(explicit.rechain_seq, 5);
    }
    #[test]
    fn commit_qc_roundtrip() {
        let kp_a = checked_random_keypair_with_algorithm(Algorithm::BlsNormal);
        let kp_b = checked_random_keypair_with_algorithm(Algorithm::BlsNormal);
        let validator_set = vec![
            crate::peer::PeerId::new(kp_a.public_key().clone()),
            crate::peer::PeerId::new(kp_b.public_key().clone()),
        ];
        let validator_set_hash = HashOf::new(&validator_set);
        let block_hash = HashOf::<crate::block::BlockHeader>::from_untyped_unchecked(
            iroha_crypto::Hash::prehashed([0xCC; 32]),
        );
        let cert = Qc {
            phase: CertPhase::Commit,
            subject_block_hash: block_hash,
            parent_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
            post_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
            height: 7,
            view: 3,
            epoch: 0,
            chain_order_hash: crate::consensus::default_chain_order_hash(),
            rechain_seq: 0,
            mode_tag: crate::block::consensus_v2::PERMISSIONED_TAG.to_string(),
            highest_qc: None,
            validator_set_hash,
            validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
            validator_set: validator_set.clone(),
            aggregate: QcAggregate {
                signers_bitmap: vec![0x03],
                bls_aggregate_signature: vec![0x01, 0x02],
            },
        };
        let buf = cert.encode();
        let decoded = Qc::decode(&mut &buf[..]).expect("decode commit cert");
        assert_eq!(decoded.height, cert.height);
        assert_eq!(decoded.view, cert.view);
        assert_eq!(decoded.validator_set_hash, validator_set_hash);
        assert_eq!(decoded.validator_set, validator_set);
        assert_eq!(
            decoded.aggregate.bls_aggregate_signature,
            cert.aggregate.bls_aggregate_signature
        );
    }
    #[test]
    fn consensus_key_record_liveness_respects_activation_and_expiry() {
        let id = ConsensusKeyId::new(ConsensusKeyRole::Validator, "v1");
        let pk = checked_random_keypair().public_key().clone();
        let record = ConsensusKeyRecord {
            id,
            public_key: pk,
            pop: None,
            activation_height: 10,
            expiry_height: Some(20),
            hsm: None,
            replaces: None,
            status: ConsensusKeyStatus::Active,
        };
        assert!(!record.is_live_at(9, 0, 0));
        assert!(record.is_live_at(10, 0, 0));
        assert!(record.is_live_at(19, 0, 0));
        assert!(!record.is_live_at(20, 0, 0));
        // overlap/expiry grace extends acceptance
        assert!(record.is_live_at(20, 2, 0));
        assert!(!record.is_live_at(23, 2, 0));
    }
    #[test]
    fn consensus_key_record_disabled_is_never_live() {
        let id = ConsensusKeyId::new(ConsensusKeyRole::Validator, "v1");
        let pk = checked_random_keypair().public_key().clone();
        let record = ConsensusKeyRecord {
            id,
            public_key: pk,
            pop: None,
            activation_height: 0,
            expiry_height: None,
            hsm: None,
            replaces: None,
            status: ConsensusKeyStatus::Disabled,
        };
        assert!(!record.is_live_at(0, 5, 5));
        assert!(!record.is_live_at(100, 5, 5));
    }
    #[test]
    fn validator_election_outcome_empty_has_expected_defaults() {
        let seed = [0x44; 32];
        let outcome = ValidatorElectionOutcome::empty(7, 42, seed);
        assert_eq!(outcome.epoch, 7);
        assert_eq!(outcome.snapshot_height, 42);
        assert_eq!(outcome.seed, seed);
        assert_eq!(outcome.candidates_total, 0);
        assert!(outcome.validator_set.is_empty());
        assert_eq!(
            outcome.validator_set_hash,
            HashOf::new(&outcome.validator_set)
        );
        assert_eq!(outcome.params.max_validators, 0);
        assert_eq!(outcome.params.finality_margin_blocks, 0);
        assert_eq!(
            outcome.rejection_reason.as_deref(),
            Some("validator election failed")
        );
        assert!(outcome.tie_break.is_empty());
    }
}
