//! Light finality verifier for exact `3f + 1` committees (spec §11.2 D-7,
//! gate G5).
//!
//! The verifier trusts one anchor and nothing that its transport returns:
//!
//! - a genesis the caller has authenticated ([`GenesisAnchor`]); the height-one
//!   proof must be signed by exactly the genesis validators; or
//! - a [`FinalityCheckpointV1`] that an earlier run verified and stored as
//!   `checkpoint.norito`.
//!
//! From the anchor it verifies forward in O(new epochs). A proof inside the
//! pinned epoch is final when exactly `2f + 1` distinct members of the pinned
//! committee signed its `CommitQC`. The proof at an epoch's last height carries
//! the next committee (`next_epoch_snapshot`), which that same certificate
//! authenticates, so only epoch-terminal proofs and the tip are fetched.
//!
//! Liveness and identity come from challenge-bound attestations: a node signs
//! its durable tip together with the caller's fresh nonce, and at least
//! `2f + 1` distinct committee members must produce one that verifies against
//! the checkpoint. The checkpoint also keeps the epoch before its own, so a
//! member whose tip is still just behind an epoch boundary that committed
//! between two reads is verified against that epoch's committee and terminal
//! decision.
//!
//! Signature, quorum and proof-of-possession checks are the data model's own
//! (`verify_bridge_finality_proof`, `BridgeFinalityAttestationV1::verify`).
//! This module only decides which committee a proof must match. Transport is
//! behind [`FinalitySource`].
// TODO(P1): anchor from the pinned network card once `NetworkCardV1` exists.
// TODO(P6): verify lane certificates against an expected dataspace committee
// (spec §10.7) once the lane certificate route exists.

use std::{cmp::Reverse, collections::BTreeMap, num::NonZeroU64};

use iroha_crypto::{HashOf, PublicKey};
use iroha_data_model::{
    NetworkId,
    block::{
        BlockHeader,
        consensus_v2::{
            ConsensusMode, GlobalPhase, QuorumCertificateRef, ValidatorPower,
            finality::{FinalizedNextEpochSnapshot, V2FinalityArtifact},
        },
    },
    bridge::{
        BridgeFinalityAttestationV1, BridgeFinalityAttestationValidationError, BridgeFinalityProof,
        BridgeFinalityVerifyError, verify_bridge_finality_proof,
    },
};
use iroha_model_base::peer::PeerId;
use norito::codec::{Decode, Encode};

/// Smallest committee: `f = 1`.
pub const MIN_COMMITTEE_MEMBERS: usize = 4;

/// Largest committee the deployment tooling accepts (spec §10.7).
///
/// The global validator roster is further capped by the protocol
/// (`consensus_v2::MAX_VALIDATORS_PER_HEIGHT`); data-model validation of every
/// proof enforces that bound.
pub const MAX_COMMITTEE_MEMBERS: usize = 128;

/// Size of an exact `3f + 1` committee.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CommitteeSize(usize);

impl CommitteeSize {
    /// Accept `members = 3f + 1` with `f >= 1` and at most
    /// [`MAX_COMMITTEE_MEMBERS`].
    ///
    /// # Errors
    ///
    /// [`FinalityError::CommitteeSize`] for any other size.
    pub fn new(members: usize) -> Result<Self, FinalityError> {
        if (MIN_COMMITTEE_MEMBERS..=MAX_COMMITTEE_MEMBERS).contains(&members)
            && (members - 1).is_multiple_of(3)
        {
            Ok(Self(members))
        } else {
            Err(FinalityError::CommitteeSize { members })
        }
    }

    /// Number of members, `3f + 1`.
    pub const fn members(self) -> usize {
        self.0
    }

    /// Byzantine members tolerated, `f`.
    pub const fn faults(self) -> usize {
        (self.0 - 1) / 3
    }

    /// Exact certificate and minimum attestation quorum, `2f + 1`.
    pub const fn quorum(self) -> usize {
        2 * self.faults() + 1
    }
}

/// Genesis facts the caller has already authenticated.
///
/// Build it from a genesis whose signature and expected hash were checked, for
/// example from `iroha_genesis::ValidatedGenesisBundle::{expected_hash,
/// validator_pops}` and its consensus metadata. The verifier derives the
/// genesis roster from these keys, never from a proof.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenesisAnchor {
    /// Network identity; it must be derived from `genesis_block_hash`.
    pub network_id: NetworkId,
    /// Hash of the signed genesis block (height one).
    pub genesis_block_hash: HashOf<BlockHeader>,
    /// Consensus mode fixed by genesis.
    pub mode: ConsensusMode,
    /// Consensus keys of the genesis validators with their BLS proofs of
    /// possession.
    pub validators: BTreeMap<PublicKey, Vec<u8>>,
}

impl GenesisAnchor {
    /// The genesis roster in canonical peer-id order, one vote each, with
    /// aligned proofs of possession.
    fn roster(&self) -> Result<(Vec<ValidatorPower>, Vec<Vec<u8>>), FinalityError> {
        CommitteeSize::new(self.validators.len())?;
        let ordered: BTreeMap<_, _> = self
            .validators
            .iter()
            .map(|(key, pop)| (PeerId::new(key.clone()), pop))
            .collect();
        Ok(ordered
            .into_iter()
            .map(|(validator, pop)| {
                (
                    ValidatorPower {
                        validator,
                        power: 1,
                    },
                    pop.clone(),
                )
            })
            .unzip())
    }
}

/// Verified finality state, stored as `checkpoint.norito` (spec §4).
///
/// It is trusted local state: the verifier wrote it after verifying the block
/// at `height`, and resumes from it without reading history again. The
/// committees reuse the data-model type that hands one epoch's frozen election
/// inputs to the next.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_deploy::verify::finality::FinalityCheckpointV1")]
pub struct FinalityCheckpointV1 {
    /// Network identity, derived from the genesis hash.
    pub network_id: NetworkId,
    /// Hash of the genesis block.
    pub genesis_block_hash: HashOf<BlockHeader>,
    /// Genesis `CommitQC` decision. An attestation's genesis proof must
    /// certify the same decision.
    pub genesis_decision: QuorumCertificateRef,
    /// Height of the last verified block.
    pub height: NonZeroU64,
    /// Hash of the last verified block.
    pub block_hash: HashOf<BlockHeader>,
    /// `CommitQC` decision that finalized `block_hash`.
    pub decision: QuorumCertificateRef,
    /// Frozen election inputs of the epoch that finalized `height`.
    pub committee: FinalizedNextEpochSnapshot,
    /// Inputs of the following epoch, present exactly when `height` is the
    /// last height of `committee`'s epoch.
    pub next_committee: Option<FinalizedNextEpochSnapshot>,
    /// The epoch just before `committee`'s, once a verified advance crossed
    /// into `committee`'s epoch.
    pub previous_epoch: Option<PreviousEpochV1>,
}

/// A verified epoch that ended before the checkpoint's epoch.
///
/// Attestations are read one member after another, so an epoch boundary can
/// commit between two reads. Keeping the epoch that was just crossed lets the
/// members still behind that boundary count.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_deploy::verify::finality::PreviousEpochV1")]
pub struct PreviousEpochV1 {
    /// Frozen election inputs of the previous epoch.
    pub committee: FinalizedNextEpochSnapshot,
    /// `CommitQC` decision that finalized its last height.
    pub terminal_decision: QuorumCertificateRef,
}

impl FinalityCheckpointV1 {
    /// Encode as one canonical Norito frame (header included).
    ///
    /// # Errors
    ///
    /// [`FinalityError::Codec`] if encoding fails.
    pub fn to_bytes(&self) -> Result<Vec<u8>, FinalityError> {
        norito::encode_canonical(self).map_err(FinalityError::Codec)
    }

    /// Decode one canonical Norito frame and check its consistency.
    ///
    /// # Errors
    ///
    /// [`FinalityError::Codec`] for bytes that are not a canonical frame, or
    /// any error of [`Self::validate`].
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, FinalityError> {
        let checkpoint: Self = norito::decode_canonical(bytes).map_err(FinalityError::Codec)?;
        checkpoint.validate()?;
        Ok(checkpoint)
    }

    /// The committee that must finalize the next height.
    pub fn governing_committee(&self) -> &FinalizedNextEpochSnapshot {
        self.next_committee.as_ref().unwrap_or(&self.committee)
    }

    /// Check that the fields agree with each other and that every committee
    /// is an exact `3f + 1` roster.
    ///
    /// # Errors
    ///
    /// [`FinalityError::InvalidCheckpoint`] naming the broken rule, or
    /// [`FinalityError::CommitteeSize`].
    pub fn validate(&self) -> Result<(), FinalityError> {
        let invalid = |reason| Err(FinalityError::InvalidCheckpoint(reason));
        if NetworkId::from_genesis_hash(self.genesis_block_hash) != self.network_id {
            return invalid("network id is not derived from the genesis hash");
        }
        if !certifies(&self.genesis_decision, 1, self.genesis_block_hash)
            || self.genesis_decision.subject.parent_block_hash.is_some()
        {
            return invalid("genesis decision does not certify the genesis block");
        }
        if !certifies(&self.decision, self.height.get(), self.block_hash) {
            return invalid("decision does not certify the checkpoint block");
        }
        if self.height.get() == 1 && !self.decision.same_commit_decision(self.genesis_decision) {
            return invalid("height-one checkpoint differs from the genesis decision");
        }
        validate_committee(&self.committee)?;
        let end = self.committee.epoch_end_height;
        if self.height.get() > end {
            return invalid("checkpoint height is past the end of its epoch");
        }
        if let Some(previous) = &self.previous_epoch {
            let previous_end = previous.committee.epoch_end_height;
            if previous.committee.epoch.checked_add(1) != Some(self.committee.epoch)
                || previous_end >= self.height.get()
                || previous.committee.mode != self.committee.mode
                || !certifies(
                    &previous.terminal_decision,
                    previous_end,
                    previous.terminal_decision.subject.block_hash,
                )
            {
                return invalid("previous epoch does not precede the checkpoint epoch");
            }
            validate_committee(&previous.committee)?;
        }
        match &self.next_committee {
            // The terminal height `u64::MAX` has no representable successor.
            None if self.height.get() == end && end != u64::MAX => {
                invalid("epoch-terminal checkpoint lacks the next committee")
            }
            None => Ok(()),
            Some(_) if self.height.get() != end => {
                invalid("next committee is present before the epoch ends")
            }
            Some(next) => {
                if self.committee.epoch.checked_add(1) != Some(next.epoch)
                    || next.epoch_end_height <= end
                    || next.mode != self.committee.mode
                {
                    return invalid("next committee does not follow the checkpoint epoch");
                }
                validate_committee(next)
            }
        }
    }

    /// Verify `proof` as the finalized block at `expected`, a height above the
    /// checkpoint within the governing epoch, and return the checkpoint at it.
    fn extend(
        &self,
        proof: &BridgeFinalityProof,
        expected: NonZeroU64,
    ) -> Result<Self, FinalityError> {
        let height = proof.block_header.height();
        if height != expected {
            return Err(FinalityError::UnexpectedHeight {
                expected: expected.get(),
                actual: height.get(),
            });
        }
        let committee = self.governing_committee();
        verify_in_committee(proof, committee, self.network_id)?;
        let artifact = &proof.finality_artifact;
        // Leaving an epoch-terminal checkpoint keeps the epoch it ends.
        let previous_epoch = if self.next_committee.is_some() {
            Some(PreviousEpochV1 {
                committee: self.committee.clone(),
                terminal_decision: self.decision,
            })
        } else {
            self.previous_epoch.clone()
        };
        let next = Self {
            network_id: self.network_id,
            genesis_block_hash: self.genesis_block_hash,
            genesis_decision: self.genesis_decision,
            height,
            block_hash: artifact.block_hash,
            decision: artifact.commit_qc.as_ref(),
            committee: committee.clone(),
            next_committee: artifact
                .height_context
                .next_epoch_snapshot
                .as_ref()
                .map(epoch_inputs),
            previous_epoch,
        };
        next.validate()?;
        Ok(next)
    }
}

/// Transport for the verifier. Everything it returns is verified, so a faulty
/// or hostile source can only make verification fail.
pub trait FinalitySource {
    /// Transport failure.
    type Error: std::error::Error + Send + Sync + 'static;

    /// The finality proof of the block at `height`
    /// (`GET /v1/bridge/finality/{height}`).
    ///
    /// # Errors
    ///
    /// Any transport failure.
    fn finality_proof(&self, height: NonZeroU64) -> Result<BridgeFinalityProof, Self::Error>;

    /// A fresh attestation of `peer`'s durable tip bound to `challenge`
    /// (`GET /v1/bridge/finality/attestation/latest`).
    ///
    /// # Errors
    ///
    /// Any transport failure.
    fn latest_attestation(
        &self,
        peer: &PeerId,
        challenge: &[u8; 32],
    ) -> Result<BridgeFinalityAttestationV1, Self::Error>;
}

/// A durable tip that one committee member attested to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AttestedTip {
    /// Height of the attested tip.
    pub height: NonZeroU64,
    /// Hash of the attested tip.
    pub block_hash: HashOf<BlockHeader>,
}

/// What became of one committee member's attestation.
#[derive(Debug)]
pub enum AttestationOutcome {
    /// It verified, and the member's tip is final on the verified chain.
    Verified(AttestedTip),
    /// It failed verification.
    Rejected(FinalityError),
    /// The source could not fetch it.
    Unreachable(String),
    /// None was supplied.
    Missing,
}

/// Attestations tallied against a checkpoint.
#[derive(Debug)]
pub struct AttestationQuorum {
    /// Checkpoint height the attestations were verified against.
    pub height: NonZeroU64,
    /// Checkpoint block hash.
    pub block_hash: HashOf<BlockHeader>,
    /// Verified attestations required, `2f + 1`.
    pub required: usize,
    /// One outcome per committee member, in roster order.
    pub peers: Vec<(PeerId, AttestationOutcome)>,
}

impl AttestationQuorum {
    /// Number of distinct members whose attestation verified.
    pub fn verified(&self) -> usize {
        self.peers
            .iter()
            .filter(|(_, outcome)| matches!(outcome, AttestationOutcome::Verified(_)))
            .count()
    }
}

/// Why finality could not be verified.
#[derive(Debug, thiserror::Error)]
pub enum FinalityError {
    /// The roster is not an exact `3f + 1` committee within bounds.
    #[error(
        "a committee of {members} is not an exact 3f+1 roster of \
         {MIN_COMMITTEE_MEMBERS} to {MAX_COMMITTEE_MEMBERS} members"
    )]
    CommitteeSize {
        /// Roster size found.
        members: usize,
    },
    /// The value belongs to another network.
    #[error("expected network {expected}, got {actual}")]
    WrongNetwork {
        /// Pinned network.
        expected: NetworkId,
        /// Network found.
        actual: NetworkId,
    },
    /// The genesis block hash differs from the pinned genesis.
    #[error("the genesis block hash differs from the pinned genesis")]
    WrongGenesis,
    /// The source returned a proof for another height.
    #[error("expected a finality proof for height {expected}, got height {actual}")]
    UnexpectedHeight {
        /// Requested height.
        expected: u64,
        /// Height returned.
        actual: u64,
    },
    /// The tip is below the checkpoint; the checkpoint never moves back.
    #[error("the tip at height {height} is below the checkpoint at height {checkpoint}")]
    StaleTip {
        /// Checkpoint height.
        checkpoint: u64,
        /// Tip height.
        height: u64,
    },
    /// The tip is above the checkpoint; advance the verifier first.
    #[error("the tip at height {height} is above the checkpoint at height {checkpoint}")]
    AheadOfCheckpoint {
        /// Checkpoint height.
        checkpoint: u64,
        /// Tip height.
        height: u64,
    },
    /// The tip belongs to an epoch before the checkpoint's that the checkpoint
    /// no longer keeps (older than the previous epoch).
    #[error("the tip at height {height} is from epoch {epoch}, before the checkpoint epoch")]
    EarlierEpoch {
        /// Tip height.
        height: u64,
        /// Tip epoch.
        epoch: u64,
    },
    /// The proof's roster, proofs of possession or epoch inputs differ from
    /// the pinned committee.
    #[error("the proof at height {height} is not from the pinned committee and epoch")]
    CommitteeMismatch {
        /// Proof height.
        height: u64,
    },
    /// The proof starts from a snapshot bootstrap, which the first release
    /// does not produce.
    #[error("the proof at height {height} is anchored in a snapshot bootstrap")]
    SnapshotBootstrap {
        /// Proof height.
        height: u64,
    },
    /// The `CommitQC` names a signer twice or out of order.
    #[error("the CommitQC signers at height {height} repeat a validator or are out of order")]
    NonCanonicalSigners {
        /// Proof height.
        height: u64,
    },
    /// The `CommitQC` names a signer outside the roster.
    #[error("CommitQC signer {signer} at height {height} is outside the {members}-member roster")]
    SignerOutsideRoster {
        /// Proof height.
        height: u64,
        /// Signer index.
        signer: u32,
        /// Roster size.
        members: usize,
    },
    /// The `CommitQC` does not have exactly `2f + 1` signers.
    #[error(
        "the CommitQC at height {height} has {actual} signers; exactly {expected} are required"
    )]
    SignerCount {
        /// Proof height.
        height: u64,
        /// `2f + 1`.
        expected: usize,
        /// Signers found.
        actual: usize,
    },
    /// Data-model verification of the proof failed (structure, network,
    /// quorum, proofs of possession or aggregate signature).
    #[error("the finality proof at height {height} does not verify: {source}")]
    Proof {
        /// Proof height.
        height: u64,
        /// Data-model error.
        source: BridgeFinalityVerifyError,
    },
    /// A valid certificate for a different decision than the verified chain.
    #[error("the proof at height {height} certifies a different decision than the verified chain")]
    ConflictingDecision {
        /// Proof height.
        height: u64,
    },
    /// An all-zero challenge could be replayed.
    #[error("the attestation challenge must be non-zero")]
    ZeroChallenge,
    /// The attestation answers another challenge.
    #[error("the attestation is not bound to this challenge")]
    StaleChallenge,
    /// The attestation is inconsistent or its node signature is invalid.
    #[error("invalid attestation: {0}")]
    Attestation(#[source] BridgeFinalityAttestationValidationError),
    /// The attesting node is not a member of the checkpoint committee.
    #[error("attesting node {peer} is not a member of the checkpoint committee")]
    NotInCommittee {
        /// Attesting node.
        peer: Box<PeerId>,
    },
    /// The source returned another node's attestation.
    #[error("asked {expected} for an attestation, got one signed by {actual}")]
    UnexpectedPeer {
        /// Node asked.
        expected: Box<PeerId>,
        /// Node that signed.
        actual: Box<PeerId>,
    },
    /// Fewer than `2f + 1` distinct committee members attested.
    #[error("{} of the {} required committee members attested", .0.verified(), .0.required)]
    InsufficientAttestations(Box<AttestationQuorum>),
    /// The stored checkpoint is inconsistent.
    #[error("invalid finality checkpoint: {0}")]
    InvalidCheckpoint(&'static str),
    /// The checkpoint bytes are not a canonical Norito frame.
    #[error("finality checkpoint codec: {0}")]
    Codec(#[source] norito::Error),
    /// The source failed.
    #[error("finality source: {0}")]
    Source(#[source] Box<dyn std::error::Error + Send + Sync>),
}

impl FinalityError {
    fn transport(error: impl std::error::Error + Send + Sync + 'static) -> Self {
        Self::Source(Box::new(error))
    }
}

/// Light finality verifier holding one verified checkpoint.
///
/// Every method leaves the checkpoint unchanged when it fails.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinalityVerifier {
    checkpoint: FinalityCheckpointV1,
}

impl FinalityVerifier {
    /// Anchor at an authenticated genesis: `genesis` must be the height-one
    /// proof of `anchor`'s block, signed by exactly `2f + 1` of its validators.
    ///
    /// # Errors
    ///
    /// [`FinalityError::WrongNetwork`] or [`FinalityError::WrongGenesis`] for
    /// another chain, [`FinalityError::CommitteeSize`] for a roster that is
    /// not `3f + 1`, [`FinalityError::CommitteeMismatch`] when the proof's
    /// roster, proofs of possession or mode differ from the anchor, and any
    /// certificate error.
    pub fn from_genesis(
        anchor: &GenesisAnchor,
        genesis: &BridgeFinalityProof,
    ) -> Result<Self, FinalityError> {
        let derived = NetworkId::from_genesis_hash(anchor.genesis_block_hash);
        if derived != anchor.network_id {
            return Err(FinalityError::WrongNetwork {
                expected: anchor.network_id,
                actual: derived,
            });
        }
        let height = genesis.block_header.height();
        if height.get() != 1 {
            return Err(FinalityError::UnexpectedHeight {
                expected: 1,
                actual: height.get(),
            });
        }
        if genesis.block_header.hash() != anchor.genesis_block_hash {
            return Err(FinalityError::WrongGenesis);
        }
        let (roster, pops) = anchor.roster()?;
        let artifact = &genesis.finality_artifact;
        let committee = committee_of(artifact);
        if committee.mode != anchor.mode
            || committee.roster != roster
            || committee.validator_set_pops != pops
        {
            return Err(FinalityError::CommitteeMismatch { height: 1 });
        }
        verify_in_committee(genesis, &committee, anchor.network_id)?;
        let decision = artifact.commit_qc.as_ref();
        let checkpoint = FinalityCheckpointV1 {
            network_id: anchor.network_id,
            genesis_block_hash: anchor.genesis_block_hash,
            genesis_decision: decision,
            height,
            block_hash: artifact.block_hash,
            decision,
            committee,
            next_committee: artifact
                .height_context
                .next_epoch_snapshot
                .as_ref()
                .map(epoch_inputs),
            previous_epoch: None,
        };
        checkpoint.validate()?;
        Ok(Self { checkpoint })
    }

    /// Resume from a stored checkpoint of the pinned network.
    ///
    /// # Errors
    ///
    /// [`FinalityError::WrongNetwork`] for another network's checkpoint, or
    /// any error of [`FinalityCheckpointV1::validate`].
    pub fn from_checkpoint(
        checkpoint: FinalityCheckpointV1,
        expected_network: NetworkId,
    ) -> Result<Self, FinalityError> {
        checkpoint.validate()?;
        if checkpoint.network_id != expected_network {
            return Err(FinalityError::WrongNetwork {
                expected: expected_network,
                actual: checkpoint.network_id,
            });
        }
        Ok(Self { checkpoint })
    }

    /// The verified checkpoint; store it to resume later.
    pub fn checkpoint(&self) -> &FinalityCheckpointV1 {
        &self.checkpoint
    }

    /// Size of the committee that finalized the checkpoint.
    pub fn committee_size(&self) -> CommitteeSize {
        CommitteeSize(self.checkpoint.committee.roster.len())
    }

    /// Verify `tip` and move the checkpoint to it.
    ///
    /// For every epoch that ends between the checkpoint and the tip, only the
    /// epoch-terminal proof is fetched. A tip at the checkpoint height must
    /// certify the checkpoint decision. Returns the number of epoch-terminal
    /// proofs fetched.
    ///
    /// # Errors
    ///
    /// [`FinalityError::StaleTip`] for a tip below the checkpoint, any
    /// transport error, and any certificate or committee error of the tip or
    /// an epoch-terminal proof.
    pub fn advance<S: FinalitySource + ?Sized>(
        &mut self,
        source: &S,
        tip: &BridgeFinalityProof,
    ) -> Result<usize, FinalityError> {
        let current = &self.checkpoint;
        let height = tip.block_header.height();
        if height < current.height {
            return Err(FinalityError::StaleTip {
                checkpoint: current.height.get(),
                height: height.get(),
            });
        }
        if height == current.height {
            self.verify_settled(tip)?;
            return Ok(0);
        }
        if height.get() > current.governing_committee().epoch_end_height {
            // Before fetching anything for a later epoch, require the tip to
            // carry a valid certificate from the roster it names.
            verify_in_committee(
                tip,
                &committee_of(&tip.finality_artifact),
                current.network_id,
            )?;
        }
        let mut state = current.clone();
        let mut crossed = 0;
        while height.get() > state.governing_committee().epoch_end_height {
            let end = NonZeroU64::new(state.governing_committee().epoch_end_height).ok_or(
                FinalityError::InvalidCheckpoint("an epoch ends at height zero"),
            )?;
            let terminal = source
                .finality_proof(end)
                .map_err(FinalityError::transport)?;
            state = state.extend(&terminal, end)?;
            crossed += 1;
        }
        self.checkpoint = state.extend(tip, height)?;
        Ok(crossed)
    }

    /// Verify one attestation against the checkpoint: signed by a member of
    /// the checkpoint committee over `challenge`, on the pinned network and
    /// genesis, reporting a tip that is final on the verified chain at or
    /// below the checkpoint. A tip in the kept previous epoch is verified
    /// against that epoch's committee and terminal decision.
    ///
    /// # Errors
    ///
    /// [`FinalityError::AheadOfCheckpoint`] when the tip is above the
    /// checkpoint (advance first), [`FinalityError::EarlierEpoch`] when it is
    /// in an epoch before the checkpoint's other than the kept previous one,
    /// and any identity, binding or certificate error.
    pub fn verify_attestation(
        &self,
        challenge: &[u8; 32],
        attestation: &BridgeFinalityAttestationV1,
    ) -> Result<AttestedTip, FinalityError> {
        self.verify_attestation_identity(challenge, attestation)?;
        let body = &attestation.body;
        if !is_member(&self.checkpoint.committee, &body.node_id) {
            return Err(FinalityError::NotInCommittee {
                peer: Box::new(body.node_id.clone()),
            });
        }
        let tip = &body.finality_proof;
        let height = tip.block_header.height();
        if height > self.checkpoint.height {
            return Err(FinalityError::AheadOfCheckpoint {
                checkpoint: self.checkpoint.height.get(),
                height: height.get(),
            });
        }
        self.verify_settled(tip)?;
        Ok(AttestedTip {
            height,
            block_hash: tip.block_header.hash(),
        })
    }

    /// Tally attestations: at least `2f + 1` distinct members of the
    /// checkpoint committee must have one that verifies. Repeated attestations
    /// from one member count once; attestations from non-members are ignored.
    ///
    /// # Errors
    ///
    /// [`FinalityError::ZeroChallenge`], or
    /// [`FinalityError::InsufficientAttestations`] with every member's outcome.
    pub fn attestation_quorum(
        &self,
        challenge: &[u8; 32],
        attestations: &[BridgeFinalityAttestationV1],
    ) -> Result<AttestationQuorum, FinalityError> {
        require_challenge(challenge)?;
        let mut peers = self
            .checkpoint
            .committee
            .roster
            .iter()
            .map(|member| (member.validator.clone(), AttestationOutcome::Missing))
            .collect::<Vec<_>>();
        for attestation in attestations {
            let Some((_, outcome)) = peers
                .iter_mut()
                .find(|(peer, _)| *peer == attestation.body.node_id)
            else {
                continue;
            };
            if !matches!(outcome, AttestationOutcome::Verified(_)) {
                *outcome = self
                    .verify_attestation(challenge, attestation)
                    .map_or_else(AttestationOutcome::Rejected, AttestationOutcome::Verified);
            }
        }
        self.quorum(peers)
    }

    /// One full finality observation (gate G5): ask every committee member
    /// for a fresh attestation over `challenge`, advance to the highest tip
    /// that verifies, repeat while the advance reaches committee members not
    /// yet asked, and require `2f + 1` verified attestations against the new
    /// checkpoint.
    ///
    /// Tips that fail to verify do not block the others; their attestations
    /// are reported as rejected. The checkpoint moves only if the quorum
    /// holds.
    ///
    /// # Errors
    ///
    /// [`FinalityError::ZeroChallenge`], or
    /// [`FinalityError::InsufficientAttestations`] with every member's outcome.
    pub fn observe<S: FinalitySource + ?Sized>(
        &mut self,
        source: &S,
        challenge: &[u8; 32],
    ) -> Result<AttestationQuorum, FinalityError> {
        require_challenge(challenge)?;
        let mut trial = self.clone();
        let mut reads = BTreeMap::new();
        // Each round asks at least one new member, so this ends.
        loop {
            let asked = reads.len();
            trial.read_members(source, challenge, &mut reads);
            if reads.len() == asked {
                break;
            }
            trial.advance_to_highest(source, challenge, &reads);
        }
        let peers = trial
            .checkpoint
            .committee
            .roster
            .iter()
            .map(|member| {
                let peer = &member.validator;
                let outcome = match reads.get(peer) {
                    None => AttestationOutcome::Missing,
                    Some(Err(error)) => AttestationOutcome::Unreachable(error.clone()),
                    Some(Ok(attestation)) if attestation.body.node_id != *peer => {
                        AttestationOutcome::Rejected(FinalityError::UnexpectedPeer {
                            expected: Box::new(peer.clone()),
                            actual: Box::new(attestation.body.node_id.clone()),
                        })
                    }
                    Some(Ok(attestation)) => trial
                        .verify_attestation(challenge, attestation)
                        .map_or_else(AttestationOutcome::Rejected, AttestationOutcome::Verified),
                };
                (peer.clone(), outcome)
            })
            .collect();
        let quorum = trial.quorum(peers)?;
        *self = trial;
        Ok(quorum)
    }

    /// Advance to the highest attested tip above the checkpoint that
    /// verifies, trying lower tips when a higher one fails.
    fn advance_to_highest<S: FinalitySource + ?Sized>(
        &mut self,
        source: &S,
        challenge: &[u8; 32],
        reads: &BTreeMap<PeerId, Result<BridgeFinalityAttestationV1, String>>,
    ) {
        let mut tips = reads
            .values()
            .filter_map(|read| read.as_ref().ok())
            .filter(|attestation| {
                attestation.body.finality_proof.block_header.height() > self.checkpoint.height
                    && self
                        .verify_attestation_identity(challenge, attestation)
                        .is_ok()
            })
            .map(|attestation| &attestation.body.finality_proof)
            .collect::<Vec<_>>();
        tips.sort_by_key(|tip| Reverse(tip.block_header.height()));
        tips.dedup();
        for tip in tips {
            if self.advance(source, tip).is_ok() {
                return;
            }
        }
    }

    /// Fetch an attestation from every member of the checkpoint and governing
    /// committees not yet in `reads`.
    // TODO(P2): fetch concurrently once the HTTP source lands; the frozen
    // `iroha_cli` verifier read its four peers in parallel.
    fn read_members<S: FinalitySource + ?Sized>(
        &self,
        source: &S,
        challenge: &[u8; 32],
        reads: &mut BTreeMap<PeerId, Result<BridgeFinalityAttestationV1, String>>,
    ) {
        let checkpoint = &self.checkpoint;
        let members = checkpoint.committee.roster.iter().chain(
            checkpoint
                .next_committee
                .iter()
                .flat_map(|next| &next.roster),
        );
        for member in members {
            reads.entry(member.validator.clone()).or_insert_with(|| {
                source
                    .latest_attestation(&member.validator, challenge)
                    .map_err(|error| error.to_string())
            });
        }
    }

    fn quorum(
        &self,
        peers: Vec<(PeerId, AttestationOutcome)>,
    ) -> Result<AttestationQuorum, FinalityError> {
        let quorum = AttestationQuorum {
            height: self.checkpoint.height,
            block_hash: self.checkpoint.block_hash,
            required: self.committee_size().quorum(),
            peers,
        };
        if quorum.verified() >= quorum.required {
            Ok(quorum)
        } else {
            Err(FinalityError::InsufficientAttestations(Box::new(quorum)))
        }
    }

    /// Everything about an attestation except its tip: the node signature,
    /// the challenge, the network and genesis bindings and the genesis
    /// decision.
    fn verify_attestation_identity(
        &self,
        challenge: &[u8; 32],
        attestation: &BridgeFinalityAttestationV1,
    ) -> Result<(), FinalityError> {
        require_challenge(challenge)?;
        attestation.verify().map_err(FinalityError::Attestation)?;
        let checkpoint = &self.checkpoint;
        let body = &attestation.body;
        if body.challenge != *challenge {
            return Err(FinalityError::StaleChallenge);
        }
        if body.network_id != checkpoint.network_id {
            return Err(FinalityError::WrongNetwork {
                expected: checkpoint.network_id,
                actual: body.network_id,
            });
        }
        if body.genesis_block_hash != checkpoint.genesis_block_hash {
            return Err(FinalityError::WrongGenesis);
        }
        let genesis = &body.genesis_finality_proof;
        if !genesis
            .finality_artifact
            .commit_qc
            .as_ref()
            .same_commit_decision(checkpoint.genesis_decision)
        {
            return Err(FinalityError::ConflictingDecision { height: 1 });
        }
        // The decision pins the genesis context, and so its roster; this
        // checks that the witness certificate is genuinely signed by it.
        verify_bridge_finality_proof(genesis, &checkpoint.network_id)
            .map_err(|source| FinalityError::Proof { height: 1, source })
    }

    /// Verify a proof at or below the checkpoint height against the committee
    /// of its epoch: the checkpoint committee, or the kept previous epoch's
    /// for a proof at or below that epoch's end. At the checkpoint height, or
    /// at the previous epoch's end, it must also certify the decision
    /// verified there.
    fn verify_settled(&self, proof: &BridgeFinalityProof) -> Result<(), FinalityError> {
        let checkpoint = &self.checkpoint;
        let height = proof.block_header.height();
        let epoch = proof.finality_artifact.height_context.epoch;
        let (committee, settled_height, settled_decision) =
            if height == checkpoint.height || epoch == checkpoint.committee.epoch {
                (
                    &checkpoint.committee,
                    checkpoint.height.get(),
                    checkpoint.decision,
                )
            } else {
                match &checkpoint.previous_epoch {
                    Some(previous)
                        if epoch == previous.committee.epoch
                            && height.get() <= previous.committee.epoch_end_height =>
                    {
                        (
                            &previous.committee,
                            previous.committee.epoch_end_height,
                            previous.terminal_decision,
                        )
                    }
                    _ => {
                        return Err(FinalityError::EarlierEpoch {
                            height: height.get(),
                            epoch,
                        });
                    }
                }
            };
        verify_in_committee(proof, committee, checkpoint.network_id)?;
        if height.get() == settled_height
            && !proof
                .finality_artifact
                .commit_qc
                .as_ref()
                .same_commit_decision(settled_decision)
        {
            return Err(FinalityError::ConflictingDecision {
                height: height.get(),
            });
        }
        Ok(())
    }
}

/// Verify that `proof` was finalized by `committee`: equal epoch inputs,
/// exactly `2f + 1` distinct in-roster signers, and a data-model-verified
/// certificate on `network`.
fn verify_in_committee(
    proof: &BridgeFinalityProof,
    committee: &FinalizedNextEpochSnapshot,
    network: NetworkId,
) -> Result<(), FinalityError> {
    let artifact = &proof.finality_artifact;
    let height = proof.block_header.height().get();
    if artifact.height_context.snapshot_bootstrap.is_some() {
        return Err(FinalityError::SnapshotBootstrap { height });
    }
    if committee_of(artifact) != *committee {
        return Err(FinalityError::CommitteeMismatch { height });
    }
    let size = CommitteeSize::new(committee.roster.len())?;
    check_signers(&artifact.commit_qc.signers, size, height)?;
    verify_bridge_finality_proof(proof, &network)
        .map_err(|source| FinalityError::Proof { height, source })
}

/// Exactly `2f + 1` strictly increasing signer indices inside the roster.
fn check_signers(signers: &[u32], size: CommitteeSize, height: u64) -> Result<(), FinalityError> {
    if signers.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(FinalityError::NonCanonicalSigners { height });
    }
    if let Some(&signer) = signers.iter().find(|&&signer| {
        usize::try_from(signer)
            .ok()
            .is_none_or(|index| index >= size.members())
    }) {
        return Err(FinalityError::SignerOutsideRoster {
            height,
            signer,
            members: size.members(),
        });
    }
    if signers.len() != size.quorum() {
        return Err(FinalityError::SignerCount {
            height,
            expected: size.quorum(),
            actual: signers.len(),
        });
    }
    Ok(())
}

/// The frozen epoch inputs that finalized `artifact`.
fn committee_of(artifact: &V2FinalityArtifact) -> FinalizedNextEpochSnapshot {
    let context = &artifact.height_context;
    FinalizedNextEpochSnapshot {
        committee_preparation: None,
        epoch: context.epoch,
        kagemusha_mint_finality_authorization: context.kagemusha_mint_finality_authorization,
        kagemusha_mint_finality_authority: context.kagemusha_mint_finality_authority.clone(),
        epoch_end_height: context.epoch_end_height,
        mode: context.mode,
        roster: context.roster.clone(),
        validator_set_pops: artifact.validator_set_pops.clone(),
        quorum: context.quorum,
        leader_seed: context.leader_seed,
    }
}

/// The election inputs of the epoch a signed next-epoch snapshot governs.
///
/// A snapshot's `committee_preparation` is the committee frozen for the epoch
/// after it, prepared at the snapshot's selection height. No height context of
/// the governed epoch repeats it, so it is not part of that epoch's committee.
fn epoch_inputs(snapshot: &FinalizedNextEpochSnapshot) -> FinalizedNextEpochSnapshot {
    FinalizedNextEpochSnapshot {
        committee_preparation: None,
        ..snapshot.clone()
    }
}

/// An exact `3f + 1` roster, sorted, one vote each, with aligned proofs of
/// possession and the canonical quorum, and only the epoch's own inputs.
fn validate_committee(committee: &FinalizedNextEpochSnapshot) -> Result<(), FinalityError> {
    if committee.committee_preparation.is_some() {
        return Err(FinalityError::InvalidCheckpoint(
            "a committee carries a later epoch's preparation",
        ));
    }
    let size = CommitteeSize::new(committee.roster.len())?;
    if committee.validator_set_pops.len() != size.members()
        || committee.roster.iter().any(|member| member.power != 1)
        || committee
            .roster
            .windows(2)
            .any(|pair| pair[0].validator >= pair[1].validator)
        || usize::try_from(committee.quorum.min_signers).ok() != Some(size.quorum())
        || usize::try_from(committee.quorum.total_power).ok() != Some(size.members())
    {
        return Err(FinalityError::InvalidCheckpoint(
            "a committee is not a canonical 3f+1 roster",
        ));
    }
    Ok(())
}

/// Whether `decision` is a `CommitQC` decision for `block_hash` at `height`.
fn certifies(
    decision: &QuorumCertificateRef,
    height: u64,
    block_hash: HashOf<BlockHeader>,
) -> bool {
    decision.phase == GlobalPhase::Commit
        && decision.round.height == height
        && decision.proposal_round == decision.round
        && decision.subject.block_hash == block_hash
}

fn is_member(committee: &FinalizedNextEpochSnapshot, peer: &PeerId) -> bool {
    committee
        .roster
        .iter()
        .any(|member| member.validator == *peer)
}

fn require_challenge(challenge: &[u8; 32]) -> Result<(), FinalityError> {
    if *challenge == [0; 32] {
        Err(FinalityError::ZeroChallenge)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests;
