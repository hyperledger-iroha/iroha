//! Consensus-related data model DTOs for on-chain persistence.
use std::str::FromStr;

use iroha_crypto::PublicKey;
use iroha_primitives::numeric::Numeric;
use iroha_schema::{Ident, IntoSchema};
#[cfg(feature = "json")]
use mv::json::JsonKeyCodec;
use norito::codec::{Decode, Encode};

pub use crate::block::consensus::{
    CertPhase, Qc, QcAggregate, QcRef, QcVote, SumeragiBlockSyncRosterStatus,
    SumeragiCommitPipelineStatus, SumeragiCommitQuorumStatus, SumeragiConsensusCapsStatus,
    SumeragiConsensusMessageHandlingEntry, SumeragiConsensusMessageHandlingStatus,
    SumeragiMembershipMismatchStatus, SumeragiNposTimeoutsStatus, SumeragiPeerKeyPolicyStatus,
    SumeragiQcEntry, SumeragiQcSnapshot, SumeragiQcStatus, SumeragiRoundGapStatus,
    SumeragiStatusWire, SumeragiViewChangeCauseStatus, SumeragiVoteValidationDropEntry,
    SumeragiVoteValidationDropPeerEntry, SumeragiVoteValidationDropReasonCount,
    SumeragiVoteValidationDropStatus, SumeragiWorkerLoopStatus, SumeragiWorkerQueueDepths,
};
use crate::prelude::*;

/// Hash-version constant for validator set checkpoints.
pub const VALIDATOR_SET_HASH_VERSION_V1: u16 = 1;

// QC types are defined in `block::consensus` and re-exported above.

/// Signed validator set checkpoint used for bootstrap and audit.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
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
        let validator_set_hash = HashOf::new(&validator_set);
        Self {
            height,
            view,
            block_hash,
            parent_state_root,
            post_state_root,
            validator_set_hash,
            validator_set_hash_version,
            validator_set,
            signers_bitmap,
            bls_aggregate_signature,
            expires_at_height,
        }
    }
}

/// Stake snapshot entry for a single validator in a commit roster.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct CommitStakeSnapshotEntry {
    /// Peer identifier for the validator.
    pub peer_id: crate::peer::PeerId,
    /// Total stake attributed to the validator.
    pub stake: Numeric,
}

/// Stake snapshot aligned to the validator set used for commit proof validation.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct CommitStakeSnapshot {
    /// Stable hash of the validator set the snapshot applies to.
    pub validator_set_hash: HashOf<Vec<crate::peer::PeerId>>,
    /// Stake entries aligned to validator roster order.
    pub entries: Vec<CommitStakeSnapshotEntry>,
}

impl CommitStakeSnapshot {
    /// Return `true` when this snapshot hash matches the provided roster.
    #[must_use]
    pub fn matches_roster(&self, roster: &[crate::peer::PeerId]) -> bool {
        self.validator_set_hash == HashOf::new(&roster.to_vec())
    }
}

/// Canonical previous-height roster evidence embedded in block payloads.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct PreviousRosterEvidence {
    /// Height of the block this evidence applies to.
    pub height: u64,
    /// Hash of the block this evidence applies to.
    pub block_hash: HashOf<crate::block::BlockHeader>,
    /// Signed validator checkpoint for the referenced block.
    pub validator_checkpoint: ValidatorSetCheckpoint,
    /// Optional NPoS stake snapshot aligned to the validator set.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub stake_snapshot: Option<CommitStakeSnapshot>,
}

/// Snapshot of the election parameters used when selecting validators for an epoch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ValidatorElectionParameters {
    /// Maximum number of validators allowed in the elected set (0 = unlimited).
    pub max_validators: u32,
    /// Minimum self-bond required for eligibility (stake units).
    pub min_self_bond: u64,
    /// Minimum nomination bond required for delegators (stake units).
    pub min_nomination_bond: u64,
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
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ValidatorTieBreak {
    /// Candidate peer identifier.
    pub peer_id: crate::peer::PeerId,
    /// Blake2b-derived score used to order candidates (lower is preferred).
    pub score: [u8; 32],
}

/// Election outcome for an epoch along with audit metadata.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
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
                min_self_bond: 0,
                min_nomination_bond: 0,
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
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
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
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
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
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
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

/// Participation record for a validator within a VRF epoch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct VrfParticipantRecord {
    /// Validator index in the topology snapshot for the epoch.
    pub signer: u32,
    /// Optional commitment emitted during the commit window.
    #[norito(skip_serializing_if = "Option::is_none")]
    pub commitment: Option<[u8; 32]>,
    /// Optional reveal emitted during the reveal window.
    #[norito(skip_serializing_if = "Option::is_none")]
    pub reveal: Option<[u8; 32]>,
    /// Last block height at which this participant record was updated.
    pub last_updated_height: u64,
}

/// Late reveal emitted after the epoch reveal window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct VrfLateRevealRecord {
    /// Validator index in the topology snapshot for the epoch.
    pub signer: u32,
    /// Reveal accepted after the window closed.
    pub reveal: [u8; 32],
    /// Block height at which the late reveal was recorded.
    pub noted_at_height: u64,
}

/// Snapshot of VRF randomness state for a particular epoch.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct VrfEpochRecord {
    /// Epoch index.
    pub epoch: u64,
    /// Deterministic seed driving PRF-based collector/leader selection for this epoch.
    /// The seed is fixed at epoch start; reveals are mixed to derive the next epoch seed.
    pub seed: [u8; 32],
    /// Length of an epoch in blocks (configuration snapshot).
    pub epoch_length: u64,
    /// Commit window deadline offset (blocks from epoch start, inclusive).
    pub commit_deadline_offset: u64,
    /// Reveal window deadline offset (blocks from epoch start, inclusive).
    pub reveal_deadline_offset: u64,
    /// Total validators in the roster snapshot for the epoch.
    pub roster_len: u32,
    /// Whether the epoch has completed (all penalties computed and seed finalized for the next epoch).
    pub finalized: bool,
    /// Block height at which this record was last updated.
    pub updated_at_height: u64,
    /// Participation entries for validators observed so far.
    pub participants: Vec<VrfParticipantRecord>,
    /// Late reveals accepted after the reveal window (do not affect entropy).
    #[norito(default)]
    #[norito(skip_serializing_if = "Vec::is_empty")]
    pub late_reveals: Vec<VrfLateRevealRecord>,
    /// Validators that committed without revealing within the epoch (finalized epochs only).
    pub committed_no_reveal: Vec<u32>,
    /// Validators that neither committed nor revealed within the epoch (finalized epochs only).
    pub no_participation: Vec<u32>,
    /// Whether penalties associated with this epoch have already been applied.
    #[norito(default)]
    pub penalties_applied: bool,
    /// Block height at which penalties were applied, if any.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub penalties_applied_at_height: Option<u64>,
    /// Election outcome for the next epoch (when available).
    #[norito(skip_serializing_if = "Option::is_none")]
    pub validator_election: Option<ValidatorElectionOutcome>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vrf_epoch_record_roundtrip() {
        let participant = VrfParticipantRecord {
            signer: 5,
            commitment: Some([0xAA; 32]),
            reveal: Some([0xBB; 32]),
            last_updated_height: 42,
        };
        let late = VrfLateRevealRecord {
            signer: 6,
            reveal: [0xCC; 32],
            noted_at_height: 360,
        };
        let record = VrfEpochRecord {
            epoch: 3,
            seed: [0x11; 32],
            epoch_length: 120,
            commit_deadline_offset: 40,
            reveal_deadline_offset: 80,
            roster_len: 7,
            finalized: true,
            updated_at_height: 360,
            participants: vec![participant],
            late_reveals: vec![late],
            committed_no_reveal: vec![2, 4],
            no_participation: vec![6],
            penalties_applied: false,
            penalties_applied_at_height: None,
            validator_election: None,
        };
        let buf = record.encode();
        let decoded = VrfEpochRecord::decode(&mut &buf[..]).expect("decode vrf epoch");
        assert_eq!(decoded.epoch, record.epoch);
        assert_eq!(decoded.seed, record.seed);
        assert_eq!(decoded.participants.len(), 1);
        assert_eq!(decoded.late_reveals.len(), 1);
        assert!(decoded.finalized);
        assert_eq!(decoded.committed_no_reveal, vec![2, 4]);
        assert_eq!(decoded.no_participation, vec![6]);
    }

    #[test]
    fn vrf_epoch_record_accepts_missing_late_reveals() {
        let record = VrfEpochRecord {
            epoch: 7,
            seed: [0x22; 32],
            epoch_length: 120,
            commit_deadline_offset: 40,
            reveal_deadline_offset: 80,
            roster_len: 4,
            finalized: false,
            updated_at_height: 120,
            participants: Vec::new(),
            late_reveals: Vec::new(),
            committed_no_reveal: Vec::new(),
            no_participation: Vec::new(),
            penalties_applied: false,
            penalties_applied_at_height: None,
            validator_election: None,
        };
        let mut value = norito::json::to_value(&record).expect("serialize vrf epoch record");
        if let norito::json::Value::Object(map) = &mut value {
            map.remove("late_reveals");
        }
        let decoded: VrfEpochRecord =
            norito::json::from_value(value).expect("decode without late_reveals");
        assert!(decoded.late_reveals.is_empty());
        assert_eq!(decoded.epoch, record.epoch);
        assert_eq!(decoded.seed, record.seed);
    }

    #[test]
    fn validator_set_checkpoint_roundtrip_and_hash() {
        let kp_a = iroha_crypto::KeyPair::random_with_algorithm(iroha_crypto::Algorithm::BlsNormal);
        let kp_b = iroha_crypto::KeyPair::random_with_algorithm(iroha_crypto::Algorithm::BlsNormal);
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

        let buf = checkpoint.encode();
        let decoded =
            ValidatorSetCheckpoint::decode(&mut &buf[..]).expect("validator checkpoint decodes");
        assert_eq!(decoded.height, 42);
        assert_eq!(decoded.view, 7);
        assert_eq!(decoded.parent_state_root, parent_state_root);
        assert_eq!(decoded.post_state_root, post_state_root);
        assert_eq!(decoded.validator_set_hash, expected_hash);
        assert_eq!(decoded.validator_set, validator_set);
    }

    #[test]
    fn commit_qc_roundtrip() {
        let kp_a = iroha_crypto::KeyPair::random_with_algorithm(iroha_crypto::Algorithm::BlsNormal);
        let kp_b = iroha_crypto::KeyPair::random_with_algorithm(iroha_crypto::Algorithm::BlsNormal);
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
            mode_tag: crate::block::consensus::PERMISSIONED_TAG.to_string(),
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
        let pk = iroha_crypto::KeyPair::random().public_key().clone();
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
        let pk = iroha_crypto::KeyPair::random().public_key().clone();
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
