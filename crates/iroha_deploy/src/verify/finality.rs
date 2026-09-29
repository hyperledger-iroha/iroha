//! Bounded, contiguous native finality verification for deployment observations.
//!
//! Trust starts at independently authenticated signed genesis or an operator-selected complete
//! native checkpoint. Every successor is verified; a supplied committee, context digest or proof
//! is never promoted to its own trust root. Fresh BLS attestations require `2f + 1` distinct
//! members of the authenticated tip committee. The native checkpoint retains exact predecessor
//! decisions, allowing one-block-lagging members to count across an epoch boundary.
//!
//! Height-one outputs have no QC. They require a certified successor or independent committee
//! attestations; merely constructing a verifier does not authenticate genesis execution.
// TODO(P2): provide the native HTTP transport and concurrent bounded peer reads.

use iroha_crypto::HashOf;
use iroha_data_model::{
    NetworkId,
    block::{BlockHeader, SignedBlock},
    sumeragi_finality::{
        FinalityError as NativeFinalityError, FinalityValidator, ScheduledSlot,
        SumeragiFinalityAttestation, SumeragiFinalityCheckpoint, SumeragiFinalityProof,
        SumeragiFinalityVerifier,
    },
};
use iroha_model_base::peer::PeerId;
use std::{cmp::Reverse, collections::BTreeMap, num::NonZeroU64};

/// Smallest committee: `f = 1`.
pub const MIN_COMMITTEE_MEMBERS: usize = 4;

/// Largest committee permitted by the Sumeragi protocol (`f = 10`).
pub const MAX_COMMITTEE_MEMBERS: usize = iroha_data_model::sumeragi::epoch::MAX_VALIDATORS;

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
        self.members() - self.faults()
    }
}

/// Maximum successor proofs processed by one advance or complete observation.
pub const MAX_ADVANCE_PROOFS: usize = 4096;
/// Maximum canonical block bytes processed by one advance or complete observation.
pub const MAX_ADVANCE_BYTES: usize = 64 * 1024 * 1024;
/// Maximum distinct peers queried by one observation across committee changes.
pub const MAX_OBSERVATION_PEERS: usize = 4 * MAX_COMMITTEE_MEMBERS;

/// Genesis and chain label the caller authenticated independently of the transport.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenesisAnchor {
    /// Independently selected genesis-derived network.
    pub network_id: NetworkId,
    /// Chain label used for native consensus instance derivation.
    pub chain_id: String,
    /// Original signed genesis whose signature the caller authenticated.
    pub genesis: SignedBlock,
    /// Exact ordered native BLS committee and possession proofs selected with genesis.
    pub validators: Vec<FinalityValidator>,
}

/// Untrusted transport for native proof frames and challenged node statements.
pub trait FinalitySource {
    /// Transport failure.
    type Error: std::error::Error + Send + Sync + 'static;
    /// Fetch the exact native proof at `height`.
    ///
    /// # Errors
    /// Any transport failure.
    fn finality_proof(&self, height: NonZeroU64) -> Result<SumeragiFinalityProof, Self::Error>;
    /// Fetch `peer`'s fresh native durable-tip attestation over `challenge`.
    ///
    /// # Errors
    /// Any transport failure.
    fn latest_attestation(
        &self,
        peer: &PeerId,
        challenge: &[u8; 32],
    ) -> Result<SumeragiFinalityAttestation, Self::Error>;
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

/// Why an independently anchored native finality observation failed.
#[derive(Debug, thiserror::Error)]
pub enum FinalityError {
    /// The roster is not exact `3f + 1` within deployment bounds.
    #[error("invalid deployment committee size {members}")]
    CommitteeSize {
        /// Roster size found.
        members: usize,
    },
    /// The response belongs to another network.
    #[error("expected network {expected}, got {actual}")]
    WrongNetwork {
        /// Selected network.
        expected: NetworkId,
        /// Response network.
        actual: NetworkId,
    },
    /// Genesis differs from the independently selected root.
    #[error("genesis differs from the selected root")]
    WrongGenesis,
    /// The source returned another height.
    #[error("expected height {expected}, got {actual}")]
    UnexpectedHeight {
        /// Requested height.
        expected: u64,
        /// Returned height.
        actual: u64,
    },
    /// Checkpoints never move backward.
    #[error("tip {height} precedes checkpoint {checkpoint}")]
    StaleTip {
        /// Current checkpoint height.
        checkpoint: u64,
        /// Offered height.
        height: u64,
    },
    /// An attestation exceeds the verified prefix.
    #[error("tip {height} exceeds checkpoint {checkpoint}")]
    AheadOfCheckpoint {
        /// Current checkpoint height.
        checkpoint: u64,
        /// Offered height.
        height: u64,
    },
    /// A lagging tip lacks a retained authenticated parent decision.
    #[error("tip {height} is outside the retained prefix at checkpoint {checkpoint}")]
    OutsideRetainedPrefix {
        /// Current checkpoint height.
        checkpoint: u64,
        /// Offered height.
        height: u64,
    },
    /// Native structural, cryptographic, schedule or exact-decision verification failed.
    #[error("native finality: {0}")]
    Native(#[from] NativeFinalityError),
    /// A zero challenge permits replay.
    #[error("the attestation challenge must be nonzero")]
    ZeroChallenge,
    /// The statement answers another request.
    #[error("the attestation answers another challenge")]
    StaleChallenge,
    /// The signer is not in the authenticated current committee.
    #[error("attesting node {peer} is not in the authenticated committee")]
    NotInCommittee {
        /// Rejected signer.
        peer: Box<PeerId>,
    },
    /// A queried node was substituted.
    #[error("asked {expected}, received a statement from {actual}")]
    UnexpectedPeer {
        /// Queried node.
        expected: Box<PeerId>,
        /// Returned signer.
        actual: Box<PeerId>,
    },
    /// Too few distinct current members supplied valid fresh statements.
    #[error("{} of {} required committee members attested", .0.verified(), .0.required)]
    InsufficientAttestations(Box<AttestationQuorum>),
    /// The requested work exceeds a finite observation budget.
    #[error("finality observation exceeds its {0} budget")]
    ResourceLimit(&'static str),
    /// An untrusted transport failed.
    #[error("finality source: {0}")]
    Source(#[source] Box<dyn std::error::Error + Send + Sync>),
}

impl FinalityError {
    fn transport(error: impl std::error::Error + Send + Sync + 'static) -> Self {
        Self::Source(Box::new(error))
    }
}

struct Budget {
    proofs: usize,
    bytes: usize,
}
impl Budget {
    fn new() -> Self {
        Self {
            proofs: MAX_ADVANCE_PROOFS,
            bytes: MAX_ADVANCE_BYTES,
        }
    }
    fn charge(&mut self, proof: &SumeragiFinalityProof) -> Result<(), FinalityError> {
        self.proofs = self
            .proofs
            .checked_sub(1)
            .ok_or(FinalityError::ResourceLimit("proof count"))?;
        self.bytes = self
            .bytes
            .checked_sub(proof.block_wire.len())
            .ok_or(FinalityError::ResourceLimit("proof bytes"))?;
        Ok(())
    }
}

/// A complete independently anchored checkpoint; failure never replaces it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinalityVerifier {
    checkpoint: SumeragiFinalityCheckpoint,
}

impl FinalityVerifier {
    /// Initialize from authenticated signed genesis and its result-only native frame.
    /// Height-one execution still requires a successor or fresh quorum attestations.
    ///
    /// # Errors
    /// Wrong network, genesis, committee, chain label or malformed native material.
    pub fn from_genesis(
        anchor: &GenesisAnchor,
        genesis: &SumeragiFinalityProof,
    ) -> Result<Self, FinalityError> {
        let actual = NetworkId::from_genesis_hash(anchor.genesis.hash());
        if actual != anchor.network_id {
            return Err(FinalityError::WrongNetwork {
                expected: anchor.network_id,
                actual,
            });
        }
        CommitteeSize::new(anchor.validators.len())?;
        if genesis.height() != 1 {
            return Err(FinalityError::UnexpectedHeight {
                expected: 1,
                actual: genesis.height(),
            });
        }
        if genesis.block_header.hash() != anchor.genesis.hash() {
            return Err(FinalityError::WrongGenesis);
        }
        let mut verifier = SumeragiFinalityVerifier::new(
            &anchor.genesis,
            &anchor.chain_id,
            anchor.validators.clone(),
        )?;
        verifier.verify(genesis)?;
        Ok(Self {
            checkpoint: verifier.export_checkpoint(genesis)?,
        })
    }

    /// Resume from independently selected local checkpoint bytes, never a responding peer's claim.
    ///
    /// # Errors
    /// Wrong selected network or chain label, or inconsistent native checkpoint.
    pub fn from_checkpoint(
        checkpoint: SumeragiFinalityCheckpoint,
        expected_network: NetworkId,
        expected_chain: &str,
    ) -> Result<Self, FinalityError> {
        SumeragiFinalityVerifier::from_trusted_checkpoint(
            &checkpoint,
            &expected_network,
            expected_chain,
        )?;
        CommitteeSize::new(checkpoint.tip().committee.len())?;
        Ok(Self { checkpoint })
    }

    /// Complete native checkpoint to persist for independently authenticated restart.
    pub fn checkpoint(&self) -> &SumeragiFinalityCheckpoint {
        &self.checkpoint
    }
    /// Size of the exact committee that certified the tip.
    pub fn committee_size(&self) -> CommitteeSize {
        CommitteeSize(self.checkpoint.tip().committee.len())
    }
    fn native(&self) -> Result<SumeragiFinalityVerifier, FinalityError> {
        Ok(SumeragiFinalityVerifier::from_trusted_checkpoint(
            &self.checkpoint,
            &self.checkpoint.network_id(),
            self.checkpoint.chain_id(),
        )?)
    }
    fn members(&self) -> Vec<PeerId> {
        self.checkpoint
            .tip()
            .committee
            .iter()
            .map(|v| PeerId::new(v.public_key.clone()))
            .collect()
    }

    /// Verify every successor through `tip`, atomically replacing the checkpoint.
    /// Returns the number of intermediate proofs fetched; the supplied tip is not fetched again.
    ///
    /// # Errors
    /// Any missing, reordered, altered or invalid successor; bounded-work exhaustion; stale tip.
    pub fn advance<S: FinalitySource + ?Sized>(
        &mut self,
        source: &S,
        tip: &SumeragiFinalityProof,
    ) -> Result<usize, FinalityError> {
        self.advance_with_budget(source, tip, &mut Budget::new())
    }
    fn advance_with_budget<S: FinalitySource + ?Sized>(
        &mut self,
        source: &S,
        tip: &SumeragiFinalityProof,
        budget: &mut Budget,
    ) -> Result<usize, FinalityError> {
        self.advance_recording(source, tip, budget, &mut |_, _| {})
    }
    fn advance_recording<S: FinalitySource + ?Sized>(
        &mut self,
        source: &S,
        tip: &SumeragiFinalityProof,
        budget: &mut Budget,
        on_verified: &mut impl FnMut(&SumeragiFinalityVerifier, u64),
    ) -> Result<usize, FinalityError> {
        let current = self.checkpoint.height();
        let height = tip.height();
        if height < current {
            return Err(FinalityError::StaleTip {
                checkpoint: current,
                height,
            });
        }
        if height == current {
            budget.charge(tip)?;
            let native = self.native()?;
            native.verify_same_decision(self.checkpoint.tip(), tip)?;
            on_verified(&native, height);
            return Ok(0);
        }
        if height - current > budget.proofs as u64 {
            return Err(FinalityError::ResourceLimit("proof count"));
        }
        let mut native = self.native()?;
        let mut fetched = 0;
        for expected in current + 1..height {
            let proof = source
                .finality_proof(NonZeroU64::new(expected).expect("successor is positive"))
                .map_err(FinalityError::transport)?;
            if proof.height() != expected {
                return Err(FinalityError::UnexpectedHeight {
                    expected,
                    actual: proof.height(),
                });
            }
            budget.charge(&proof)?;
            CommitteeSize::new(proof.committee.len())?;
            native.verify(&proof)?;
            on_verified(&native, expected);
            fetched += 1;
        }
        budget.charge(tip)?;
        CommitteeSize::new(tip.committee.len())?;
        native.verify(tip)?;
        on_verified(&native, height);
        let checkpoint = native.export_checkpoint(tip)?;
        self.checkpoint = checkpoint;
        Ok(fetched)
    }

    /// Verify a current member's fresh attestation against the exact retained native prefix.
    /// At most one block of lag is accepted; no arbitrary same-epoch decision is trusted.
    ///
    /// # Errors
    /// Invalid identity/signature/challenge, wrong network, nonmember, future or unretained tip.
    pub fn verify_attestation(
        &self,
        challenge: &[u8; 32],
        attestation: &SumeragiFinalityAttestation,
    ) -> Result<AttestedTip, FinalityError> {
        self.verify_attestation_identity(challenge, attestation)?;
        if !self.members().contains(&attestation.body.node_id) {
            return Err(FinalityError::NotInCommittee {
                peer: Box::new(attestation.body.node_id.clone()),
            });
        }
        let tip = &attestation.body.finality_proof;
        let height = tip.height();
        let checkpoint = self.checkpoint.height();
        if height > checkpoint {
            return Err(FinalityError::AheadOfCheckpoint { checkpoint, height });
        }
        if height < checkpoint.saturating_sub(1) {
            return Err(FinalityError::OutsideRetainedPrefix { checkpoint, height });
        }
        self.native()?.verify_retained_decision(tip)?;
        Ok(AttestedTip {
            height: tip.block_header.height(),
            block_hash: tip.block_header.hash(),
        })
    }

    /// Count distinct authenticated committee members; duplicate statements count once.
    ///
    /// # Errors
    /// Zero challenge or insufficient valid committee statements, with per-peer outcomes.
    pub fn attestation_quorum(
        &self,
        challenge: &[u8; 32],
        attestations: &[SumeragiFinalityAttestation],
    ) -> Result<AttestationQuorum, FinalityError> {
        require_challenge(challenge)?;
        let mut peers = self
            .members()
            .into_iter()
            .map(|p| (p, AttestationOutcome::Missing))
            .collect::<Vec<_>>();
        for attestation in attestations {
            let Some((_, outcome)) = peers
                .iter_mut()
                .find(|(p, _)| *p == attestation.body.node_id)
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

    /// Query current and authenticated next-committee members, follow contiguous proof chains,
    /// and publish the new checkpoint only when a fresh quorum of its committee attests.
    /// Responses observed before an advance remain eligible when their exact original native
    /// decisions were authenticated during that advance, including across committee boundaries.
    /// This bounded observation-local custody is separate from the compact restart window.
    ///
    /// # Errors
    /// Zero challenge, peer/proof budget exhaustion, or insufficient valid attestations.
    pub fn observe<S: FinalitySource + ?Sized>(
        &mut self,
        source: &S,
        challenge: &[u8; 32],
    ) -> Result<AttestationQuorum, FinalityError> {
        require_challenge(challenge)?;
        let mut trial = self.clone();
        let mut reads = BTreeMap::new();
        // Each entry certifies one immutable challenged response in `reads`, while its exact
        // original decision and parent are retained by contiguous native verification. At most
        // MAX_OBSERVATION_PEERS entries live here; no unbounded history is retained on restart.
        let mut observed_prefix = BTreeMap::new();
        let mut budget = Budget::new();
        loop {
            let count = reads.len();
            trial.read_members(source, challenge, &mut reads)?;
            if reads.len() == count {
                break;
            }
            let candidates = reads
                .iter()
                .filter_map(|(peer, read)| {
                    let attestation = read.as_ref().ok()?;
                    (attestation.body.node_id == *peer
                        && trial
                            .verify_attestation_identity(challenge, attestation)
                            .is_ok())
                    .then_some((peer, attestation))
                })
                .collect::<Vec<_>>();
            let native = trial.native()?;
            record_observed_prefix(
                &native,
                trial.checkpoint.height(),
                &candidates,
                &mut observed_prefix,
            );
            if trial.checkpoint.height() > 1 {
                record_observed_prefix(
                    &native,
                    trial.checkpoint.height() - 1,
                    &candidates,
                    &mut observed_prefix,
                );
            }
            let mut tips = candidates
                .iter()
                .map(|(_, a)| &a.body.finality_proof)
                .filter(|tip| tip.height() > trial.checkpoint.height())
                .collect::<Vec<_>>();
            tips.sort_by_key(|tip| Reverse(tip.height()));
            tips.dedup();
            for tip in tips {
                // An implausibly distant peer claim spends no budget and must not prevent
                // another current member's bounded tip from forming a live quorum.
                if tip.height() - trial.checkpoint.height() > budget.proofs as u64 {
                    continue;
                }
                let mut candidate_prefix = observed_prefix.clone();
                let mut capture = |native: &SumeragiFinalityVerifier, height| {
                    record_observed_prefix(native, height, &candidates, &mut candidate_prefix);
                };
                match trial.advance_recording(source, tip, &mut budget, &mut capture) {
                    Ok(_) => {
                        observed_prefix = candidate_prefix;
                        break;
                    }
                    Err(error @ FinalityError::ResourceLimit(_)) => return Err(error),
                    Err(_) => {}
                }
            }
        }
        let current_epoch = trial
            .native()?
            .verify_retained_decision(trial.checkpoint.tip())?
            .commitment()
            .schedule
            .current
            .authorization
            .epoch;
        let peers = trial
            .members()
            .into_iter()
            .map(|peer| {
                let outcome = match reads.get(&peer) {
                    None => AttestationOutcome::Missing,
                    Some(Err(error)) => AttestationOutcome::Unreachable(error.clone()),
                    Some(Ok(a)) if a.body.node_id != peer => {
                        AttestationOutcome::Rejected(FinalityError::UnexpectedPeer {
                            expected: Box::new(peer.clone()),
                            actual: Box::new(a.body.node_id.clone()),
                        })
                    }
                    Some(Ok(_))
                        if observed_prefix.get(&peer).is_some_and(|observed| {
                            observed.epoch >= current_epoch.saturating_sub(1)
                        }) =>
                    {
                        AttestationOutcome::Verified(observed_prefix[&peer].tip)
                    }
                    Some(Ok(a)) => trial
                        .verify_attestation(challenge, a)
                        .map_or_else(AttestationOutcome::Rejected, AttestationOutcome::Verified),
                };
                (peer, outcome)
            })
            .collect();
        let quorum = trial.quorum(peers)?;
        *self = trial;
        Ok(quorum)
    }
    fn read_members<S: FinalitySource + ?Sized>(
        &self,
        source: &S,
        challenge: &[u8; 32],
        reads: &mut BTreeMap<PeerId, Result<SumeragiFinalityAttestation, String>>,
    ) -> Result<(), FinalityError> {
        let mut members = self.members();
        let verified = self
            .native()?
            .verify_retained_decision(self.checkpoint.tip())?;
        if let ScheduledSlot::Ready(next) = &verified.commitment().schedule.next {
            CommitteeSize::new(next.epoch.committee.len())?;
            members.extend(next.epoch.committee.iter().map(|v| v.validator.clone()));
        }
        for peer in members {
            if !reads.contains_key(&peer) {
                if reads.len() == MAX_OBSERVATION_PEERS {
                    return Err(FinalityError::ResourceLimit("peer count"));
                }
                let outcome = source
                    .latest_attestation(&peer, challenge)
                    .map_err(|e| e.to_string());
                reads.insert(peer, outcome);
            }
        }
        Ok(())
    }
    fn quorum(
        &self,
        peers: Vec<(PeerId, AttestationOutcome)>,
    ) -> Result<AttestationQuorum, FinalityError> {
        let quorum = AttestationQuorum {
            height: self.checkpoint.tip().block_header.height(),
            block_hash: self.checkpoint.block_hash(),
            required: self.committee_size().quorum(),
            peers,
        };
        if quorum.verified() >= quorum.required {
            Ok(quorum)
        } else {
            Err(FinalityError::InsufficientAttestations(Box::new(quorum)))
        }
    }
    fn verify_attestation_identity(
        &self,
        challenge: &[u8; 32],
        attestation: &SumeragiFinalityAttestation,
    ) -> Result<(), FinalityError> {
        require_challenge(challenge)?;
        if attestation.body.challenge != *challenge {
            return Err(FinalityError::StaleChallenge);
        }
        if attestation.body.network_id != self.checkpoint.network_id() {
            return Err(FinalityError::WrongNetwork {
                expected: self.checkpoint.network_id(),
                actual: attestation.body.network_id,
            });
        }
        attestation.verify()?;
        // validate_consistency binds the complete decoded genesis frame to this independently
        // selected network hash. Its result-only execution is not authority for the current tip.
        // The tip is checked separately against exact retained decisions or contiguous successors.
        Ok(())
    }
}

/// Record only an unchanged challenged response whose complete decision has just been
/// independently authenticated. Source reads are immutable throughout the observation, and
/// committee membership is checked by constructing the final report from the final roster.
#[derive(Clone, Copy)]
struct ObservedDecision {
    tip: AttestedTip,
    epoch: u64,
}

fn record_observed_prefix(
    native: &SumeragiFinalityVerifier,
    height: u64,
    candidates: &[(&PeerId, &SumeragiFinalityAttestation)],
    observed: &mut BTreeMap<PeerId, ObservedDecision>,
) {
    for (peer, attestation) in candidates {
        let proof = &attestation.body.finality_proof;
        if proof.height() != height {
            continue;
        }
        if let Ok(verified) = native.verify_retained_decision(proof) {
            observed.insert(
                (*peer).clone(),
                ObservedDecision {
                    tip: AttestedTip {
                        height: proof.block_header.height(),
                        block_hash: proof.block_header.hash(),
                    },
                    epoch: verified.commitment().schedule.current.authorization.epoch,
                },
            );
        }
    }
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
