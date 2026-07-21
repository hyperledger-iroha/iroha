//! Authenticated, bounded NPoS VRF lifecycle for the authoritative v2 runner.
//!
//! VRF observations are height-local proposal inputs. They never mutate the
//! world state directly: the only persistence path is a
//! [`VrfEpochRecord`] carried by a finalized block's
//! [`NposConsensusEffects`](iroha_data_model::consensus::NposConsensusEffects).

use std::collections::{BTreeMap, BTreeSet};

use iroha_crypto::{Hash, KeyPair, PrivateKey, Signature};
use iroha_data_model::{
    block::consensus_v2 as wire,
    consensus::{
        NposConsensusEffects, VrfCommitProof, VrfEpochRecord, VrfLateRevealRecord,
        VrfParticipantRecord, VrfRevealProof,
    },
    peer::PeerId,
};
use mv::storage::StorageReadOnly;
use thiserror::Error;
use zeroize::Zeroizing;

use super::consensus::{NPOS_TAG, VrfCommit, VrfReveal, vrf_commit_preimage, vrf_reveal_preimage};
use crate::state::{State, WorldReadOnly};

/// Domain separator for deterministic NPoS VRF input derivation.
const VRF_INPUT_DOMAIN: &[u8] = b"iroha:npos:vrf:input:v1";

fn derive_vrf_material_from_key(
    chain_hash: &Hash,
    private_key: &PrivateKey,
    epoch: u64,
    signer: wire::ValidatorIndex,
) -> Result<([u8; 32], [u8; 32]), String> {
    let mut message = Vec::with_capacity(
        VRF_INPUT_DOMAIN.len() + chain_hash.as_ref().len() + core::mem::size_of::<u64>() * 2,
    );
    message.extend_from_slice(VRF_INPUT_DOMAIN);
    message.extend_from_slice(chain_hash.as_ref());
    message.extend_from_slice(&epoch.to_be_bytes());
    message.extend_from_slice(&u64::from(signer).to_be_bytes());
    let reveal: [u8; 32] = if private_key.algorithm() == iroha_crypto::Algorithm::BlsNormal {
        let payload =
            Zeroizing::new(private_key.try_payload().map_err(|error| {
                format!("failed to expose BLS key for VRF derivation: {error}")
            })?);
        let secret = iroha_crypto::BlsNormal::parse_private_key(payload.as_slice())
            .map_err(|error| format!("failed to parse BLS key for VRF derivation: {error}"))?;
        let (output, _proof) =
            iroha_crypto::vrf::prove_normal_with_chain(&secret, chain_hash.as_ref(), &message)
                .map_err(|error| {
                    format!("failed to derive deterministic BLS VRF material: {error}")
                })?;
        output.0
    } else {
        let signature = Signature::try_new(private_key, &message)
            .map_err(|error| format!("failed to sign local VRF input material: {error}"))?;
        Hash::new(signature.payload()).into()
    };
    let commitment: [u8; 32] = Hash::new(reveal).into();
    Ok((reveal, commitment))
}

/// Result of admitting one authenticated VRF observation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum V2VrfIngressOutcome {
    /// A new in-window commitment or reveal was retained.
    Accepted,
    /// A valid reveal after the reveal deadline was retained for penalty relief.
    AcceptedLate,
    /// The exact already-retained observation was received again.
    Duplicate,
    /// The message failed a frozen-context, cryptographic, or window check.
    Rejected(V2VrfRejection),
}

/// Stable reason for rejecting an inbound VRF message.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum V2VrfRejection {
    /// Network ingress did not identify the outer sender.
    MissingSender,
    /// The outer sender is not the peer at the claimed frozen roster index.
    SenderMismatch,
    /// The claimed signer index is outside the frozen roster.
    SignerOutOfRange,
    /// The message belongs to another epoch.
    EpochMismatch,
    /// The message is not valid in the active commit/reveal window.
    OutOfWindow,
    /// Signature bytes did not pass the checked wire constructor.
    MalformedSignature,
    /// The signature does not verify under the frozen roster key.
    InvalidSignature,
    /// A reveal has no retained commitment from the same signer.
    MissingCommitment,
    /// A reveal does not hash to the retained commitment.
    CommitmentMismatch,
    /// A signer supplied a second, different commitment.
    ConflictingCommitment,
    /// A signer supplied a second, different reveal.
    ConflictingReveal,
    /// The explicit per-height observation bound was exhausted.
    Capacity,
}

/// Reconciliation result after the finalized block has been applied to WSV.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum V2VrfReconcileOutcome {
    /// There was no locally pending record.
    NoPending,
    /// The finalized world-state record covers every locally pending observation.
    Committed,
    /// The winning block did not include all compatible local observations.
    Deferred,
    /// The winning block contains a conflicting signed observation; local state was discarded.
    ConflictDiscarded,
}

/// Fatal construction or local-emission failure.
#[derive(Debug, Error)]
pub(crate) enum V2NposError {
    /// Frozen context itself is malformed.
    #[error("invalid frozen NPoS height context: {0}")]
    Context(#[from] wire::ValidationError),
    /// Frozen NPoS epoch parameters do not form a usable window schedule.
    #[error("invalid frozen NPoS VRF epoch schedule")]
    InvalidSchedule,
    /// Authoritative v2 requires the signed genesis/on-chain NPoS parameter snapshot.
    #[error("authoritative v2 NPoS requires committed sumeragi_npos_parameters")]
    MissingCommittedParameters,
    /// The frozen roster cannot be represented by the record wire type.
    #[error("frozen NPoS roster exceeds the VRF record index range")]
    RosterTooLarge,
    /// A persisted record conflicts with the active frozen epoch.
    #[error("persisted NPoS VRF epoch record conflicts with the frozen height context")]
    PersistedRecordConflict,
    /// Local validator index or key is inconsistent with the frozen roster.
    #[error("local NPoS validator identity conflicts with the frozen roster")]
    LocalIdentityMismatch,
    /// Deterministic local VRF material could not be derived.
    #[error("failed to derive local NPoS VRF material: {0}")]
    LocalMaterial(String),
    /// Local VRF metadata could not be signed.
    #[error("failed to sign local NPoS VRF metadata: {0}")]
    LocalSignature(String),
    /// Persisted local commitment differs from deterministic crash-recovery material.
    #[error("persisted local NPoS commitment differs from deterministic key material")]
    LocalCommitmentMismatch,
    /// A proposed record bundle violates the authoritative v2 epoch contract.
    #[error("invalid authoritative v2 NPoS VRF epoch record: {0}")]
    InvalidRecord(&'static str),
    /// The epoch-boundary candidate omitted its mandatory finalized seal.
    #[error("authoritative v2 NPoS epoch boundary requires exactly one current-epoch seal")]
    MissingBoundarySeal,
    /// The first candidate of an epoch omitted the schedule snapshot that
    /// freezes commit and reveal windows for every later height in that epoch.
    #[error("authoritative v2 NPoS epoch start requires exactly one current-epoch record")]
    MissingEpochStartRecord,
    /// A freshly signed local message failed the same boundary used for remote ingress.
    #[error("fresh local NPoS VRF message failed authenticated ingress: {0:?}")]
    LocalAdmission(V2VrfIngressOutcome),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct EpochSchedule {
    length: u64,
    commit_end: u64,
    reveal_end: u64,
    position: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct NposEpochParams {
    epoch_length_blocks: u64,
    commit_deadline_offset: u64,
    reveal_deadline_offset: u64,
}

fn committed_epoch_params(world: &impl WorldReadOnly) -> Result<NposEpochParams, V2NposError> {
    let params = world
        .sumeragi_npos_parameters()
        .ok_or(V2NposError::MissingCommittedParameters)?;
    let epoch_length_blocks = params.epoch_length_blocks().get();
    let commit_deadline_offset = params.vrf_commit_window_blocks();
    let reveal_window_blocks = params.vrf_reveal_window_blocks();
    if epoch_length_blocks == 0
        || commit_deadline_offset == 0
        || reveal_window_blocks == 0
        || commit_deadline_offset > epoch_length_blocks
    {
        return Err(V2NposError::InvalidSchedule);
    }
    let reveal_deadline_offset = commit_deadline_offset
        .checked_add(reveal_window_blocks)
        .filter(|deadline| *deadline <= epoch_length_blocks)
        .ok_or(V2NposError::InvalidSchedule)?;
    Ok(NposEpochParams {
        epoch_length_blocks,
        commit_deadline_offset,
        reveal_deadline_offset,
    })
}

#[derive(Debug)]
struct ActiveVrfLifecycle {
    context: wire::HeightContext,
    schedule: EpochSchedule,
    roster_len: u32,
    observation_capacity: usize,
    committed_record: Option<VrfEpochRecord>,
    participants: BTreeMap<u32, VrfParticipantRecord>,
    late_reveals: BTreeMap<u32, VrfLateRevealRecord>,
    updated_at_height: u64,
    penalties_applied: bool,
    penalties_applied_at_height: Option<u64>,
    validator_election: Option<iroha_data_model::consensus::ValidatorElectionOutcome>,
    outbound: Vec<super::message::BlockMessage>,
    retransmit: Option<super::message::BlockMessage>,
}

/// Per-height NPoS VRF state owned exclusively by the serialized v2 runner.
#[derive(Debug, Default)]
pub(crate) struct V2NposVrfLifecycle {
    active: Option<ActiveVrfLifecycle>,
}

impl V2NposVrfLifecycle {
    /// Restore the active epoch from finalized WSV and stage deterministic
    /// local emission for the current frozen window.
    pub(crate) fn open(
        context: &wire::HeightContext,
        state: &State,
        local_validator: Option<wire::ValidatorIndex>,
        key_pair: &KeyPair,
    ) -> Result<Self, V2NposError> {
        context.validate()?;
        if context.mode != wire::ConsensusMode::Npos {
            return Ok(Self::default());
        }
        let (params, committed_record) = {
            let world = state.world_view();
            (
                committed_epoch_params(&world)?,
                world.vrf_epochs().get(&context.epoch).cloned(),
            )
        };
        // The first finalized block of every epoch is required to persist this
        // schedule snapshot. Later on-chain epoch/window changes therefore apply
        // only to a future epoch and cannot move the active commit/reveal windows.
        let (length, commit_end, reveal_end) = committed_record.as_ref().map_or(
            (
                params.epoch_length_blocks,
                params.commit_deadline_offset,
                params.reveal_deadline_offset,
            ),
            |record| {
                (
                    record.epoch_length,
                    record.commit_deadline_offset,
                    record.reveal_deadline_offset,
                )
            },
        );
        let schedule = EpochSchedule::for_context(context, length, commit_end, reveal_end)?;
        Self::from_parts(
            context.clone(),
            schedule,
            committed_record,
            local_validator,
            key_pair,
            context.roster.len(),
        )
    }

    fn from_parts(
        context: wire::HeightContext,
        schedule: EpochSchedule,
        committed_record: Option<VrfEpochRecord>,
        local_validator: Option<wire::ValidatorIndex>,
        key_pair: &KeyPair,
        observation_capacity: usize,
    ) -> Result<Self, V2NposError> {
        let roster_len =
            u32::try_from(context.roster.len()).map_err(|_| V2NposError::RosterTooLarge)?;
        let mut participants = BTreeMap::new();
        let mut late_reveals = BTreeMap::new();
        let mut updated_at_height = 0;
        let mut penalties_applied = false;
        let mut penalties_applied_at_height = None;
        let mut validator_election = None;
        if let Some(record) = committed_record.as_ref() {
            validate_persisted_record(&context, schedule, record, roster_len)?;
            updated_at_height = record.updated_at_height;
            penalties_applied = record.penalties_applied;
            penalties_applied_at_height = record.penalties_applied_at_height;
            validator_election.clone_from(&record.validator_election);
            participants.extend(
                record
                    .participants
                    .iter()
                    .cloned()
                    .map(|participant| (participant.signer, participant)),
            );
            late_reveals.extend(
                record
                    .late_reveals
                    .iter()
                    .cloned()
                    .map(|reveal| (reveal.signer, reveal)),
            );
        }
        let mut active = ActiveVrfLifecycle {
            context,
            schedule,
            roster_len,
            observation_capacity,
            committed_record,
            participants,
            late_reveals,
            updated_at_height,
            penalties_applied,
            penalties_applied_at_height,
            validator_election,
            outbound: Vec::with_capacity(1),
            retransmit: None,
        };
        active.stage_local_message(local_validator, key_pair)?;
        Ok(Self {
            active: Some(active),
        })
    }

    /// Authenticate and retain an inbound commitment without mutating WSV.
    pub(crate) fn accept_commit(
        &mut self,
        commit: VrfCommit,
        sender: Option<&PeerId>,
    ) -> V2VrfIngressOutcome {
        self.active.as_mut().map_or(
            V2VrfIngressOutcome::Rejected(V2VrfRejection::OutOfWindow),
            |active| active.accept_commit(commit, sender),
        )
    }

    /// Authenticate and retain an inbound reveal without mutating WSV.
    pub(crate) fn accept_reveal(
        &mut self,
        reveal: VrfReveal,
        sender: Option<&PeerId>,
    ) -> V2VrfIngressOutcome {
        self.active.as_mut().map_or(
            V2VrfIngressOutcome::Rejected(V2VrfRejection::OutOfWindow),
            |active| active.accept_reveal(reveal, sender),
        )
    }

    /// Drain locally generated messages for one bounded broadcast to the
    /// frozen voter set.
    pub(crate) fn take_outbound(&mut self) -> Vec<super::message::BlockMessage> {
        self.active
            .as_mut()
            .map_or_else(Vec::new, |active| std::mem::take(&mut active.outbound))
    }

    /// Clone the single locally authenticated message for bounded periodic
    /// retransmission while this height remains active.
    pub(crate) fn retransmission(&self) -> Vec<super::message::BlockMessage> {
        self.active
            .as_ref()
            .and_then(|active| active.retransmit.clone())
            .into_iter()
            .collect()
    }

    /// Return the current proposal record when it extends finalized WSV.
    pub(crate) fn pending_records(&self) -> Vec<VrfEpochRecord> {
        self.active
            .as_ref()
            .and_then(ActiveVrfLifecycle::pending_record)
            .into_iter()
            .collect()
    }

    /// Reconcile the winning block's committed effects after application.
    pub(crate) fn reconcile_committed(&mut self, state: &State) -> V2VrfReconcileOutcome {
        let Some(active) = self.active.as_mut() else {
            return V2VrfReconcileOutcome::NoPending;
        };
        let Some(pending) = active.pending_record() else {
            return V2VrfReconcileOutcome::NoPending;
        };
        let committed = {
            let world = state.world_view();
            world.vrf_epochs().get(&pending.epoch).cloned()
        };
        let Some(committed) = committed else {
            return V2VrfReconcileOutcome::Deferred;
        };
        if record_extends(&pending, &committed) {
            active.committed_record = Some(committed);
            return V2VrfReconcileOutcome::Committed;
        }
        if record_extends(&committed, &pending) {
            return V2VrfReconcileOutcome::Deferred;
        }
        active.committed_record = Some(committed);
        V2VrfReconcileOutcome::ConflictDiscarded
    }
}

/// Validate every VRF observation carried by an authoritative v2 candidate.
///
/// This is intentionally called both during Prepare validation and immediately
/// before application.  It does not trust unauthenticated participant summary
/// fields: each entropy-bearing entry must contain the exact signed message
/// and must verify against the immutable roster and chain domain in `context`.
pub(crate) fn validate_candidate_records(
    context: &wire::HeightContext,
    state: &State,
    effects: Option<&NposConsensusEffects>,
) -> Result<(), V2NposError> {
    context.validate()?;
    let records = effects.map_or(&[][..], |effects| effects.vrf_epoch_seals.as_slice());
    if context.mode != wire::ConsensusMode::Npos {
        return if records.is_empty() {
            Ok(())
        } else {
            Err(V2NposError::InvalidRecord(
                "permissioned candidate carries an NPoS VRF record",
            ))
        };
    }

    if records.len() > 1 {
        return Err(V2NposError::InvalidRecord(
            "candidate carries more than one active-epoch record",
        ));
    }
    let (params, existing) = {
        let world = state.world_view();
        (
            committed_epoch_params(&world)?,
            world.vrf_epochs().get(&context.epoch).cloned(),
        )
    };
    let (length, commit_end, reveal_end) = existing.as_ref().map_or(
        (
            params.epoch_length_blocks,
            params.commit_deadline_offset,
            params.reveal_deadline_offset,
        ),
        |record| {
            (
                record.epoch_length,
                record.commit_deadline_offset,
                record.reveal_deadline_offset,
            )
        },
    );
    let schedule = EpochSchedule::for_context(context, length, commit_end, reveal_end)?;
    let boundary = context.height == context.epoch_end_height;
    if boundary && records.len() != 1 {
        return Err(V2NposError::MissingBoundarySeal);
    }
    if schedule.position == 1 && existing.is_none() && records.len() != 1 {
        return Err(V2NposError::MissingEpochStartRecord);
    }
    let Some(record) = records.first() else {
        return Ok(());
    };
    let roster_len =
        u32::try_from(context.roster.len()).map_err(|_| V2NposError::RosterTooLarge)?;
    validate_authenticated_record(context, schedule, record, roster_len, boundary)?;
    if let Some(existing) = existing.as_ref()
        && !record_extends(existing, record)
    {
        return Err(V2NposError::InvalidRecord(
            "record is not a monotonic extension of finalized state",
        ));
    }
    validate_extension_at_candidate_height(context, schedule, existing.as_ref(), record)?;
    Ok(())
}

impl EpochSchedule {
    fn for_context(
        context: &wire::HeightContext,
        length: u64,
        commit_end: u64,
        reveal_end: u64,
    ) -> Result<Self, V2NposError> {
        if length == 0 || commit_end == 0 || commit_end > reveal_end || reveal_end > length {
            return Err(V2NposError::InvalidSchedule);
        }
        let start = context
            .epoch_end_height
            .checked_add(1)
            .and_then(|end_exclusive| end_exclusive.checked_sub(length))
            .ok_or(V2NposError::InvalidSchedule)?;
        let position = context
            .height
            .checked_sub(start)
            .and_then(|offset| offset.checked_add(1))
            .filter(|position| *position <= length)
            .ok_or(V2NposError::InvalidSchedule)?;
        Ok(Self {
            length,
            commit_end,
            reveal_end,
            position,
        })
    }
}

impl ActiveVrfLifecycle {
    fn accept_commit(&mut self, commit: VrfCommit, sender: Option<&PeerId>) -> V2VrfIngressOutcome {
        let signer = match self.authenticate_commit(&commit, sender) {
            Ok(signer) => signer,
            Err(reason) => return V2VrfIngressOutcome::Rejected(reason),
        };
        if let Some(existing) = self.participants.get(&commit.signer) {
            return match existing.commitment {
                Some(value)
                    if value == commit.commitment
                        && existing.commit_proof.as_ref().is_some_and(|proof| {
                            proof.epoch == commit.epoch
                                && proof.commitment == commit.commitment
                                && proof.signer == commit.signer
                                && proof.signature == commit.bls_sig
                        }) =>
                {
                    V2VrfIngressOutcome::Duplicate
                }
                Some(_) => V2VrfIngressOutcome::Rejected(V2VrfRejection::ConflictingCommitment),
                None => self.retain_commit(signer, commit),
            };
        }
        if self.participants.len() >= self.observation_capacity {
            return V2VrfIngressOutcome::Rejected(V2VrfRejection::Capacity);
        }
        self.retain_commit(signer, commit)
    }

    fn accept_reveal(&mut self, reveal: VrfReveal, sender: Option<&PeerId>) -> V2VrfIngressOutcome {
        let signer = match self.authenticate_reveal(&reveal, sender) {
            Ok(signer) => signer,
            Err(reason) => return V2VrfIngressOutcome::Rejected(reason),
        };
        let Some(participant) = self.participants.get(&reveal.signer) else {
            return V2VrfIngressOutcome::Rejected(V2VrfRejection::MissingCommitment);
        };
        let Some(commitment) = participant.commitment else {
            return V2VrfIngressOutcome::Rejected(V2VrfRejection::MissingCommitment);
        };
        let actual: [u8; 32] = Hash::new(reveal.reveal).into();
        if actual != commitment {
            return V2VrfIngressOutcome::Rejected(V2VrfRejection::CommitmentMismatch);
        }
        if let Some(existing) = participant.reveal {
            return if existing == reveal.reveal
                && participant.reveal_proof.as_ref().is_some_and(|proof| {
                    proof.epoch == reveal.epoch
                        && proof.reveal == reveal.reveal
                        && proof.signer == reveal.signer
                        && proof.signature == reveal.bls_sig
                }) {
                V2VrfIngressOutcome::Duplicate
            } else {
                V2VrfIngressOutcome::Rejected(V2VrfRejection::ConflictingReveal)
            };
        }
        if let Some(existing) = self.late_reveals.get(&reveal.signer) {
            return if existing.reveal == reveal.reveal
                && existing.reveal_proof.as_ref().is_some_and(|proof| {
                    proof.epoch == reveal.epoch
                        && proof.reveal == reveal.reveal
                        && proof.signer == reveal.signer
                        && proof.signature == reveal.bls_sig
                }) {
                V2VrfIngressOutcome::Duplicate
            } else {
                V2VrfIngressOutcome::Rejected(V2VrfRejection::ConflictingReveal)
            };
        }

        if self.schedule.position <= self.schedule.reveal_end {
            let participant = self
                .participants
                .get_mut(&signer)
                .expect("authenticated reveal has a retained participant");
            participant.reveal = Some(reveal.reveal);
            participant.reveal_proof = Some(VrfRevealProof {
                epoch: reveal.epoch,
                reveal: reveal.reveal,
                signer: reveal.signer,
                signature: reveal.bls_sig,
                observed_at_height: self.context.height,
            });
            participant.last_updated_height = self.context.height;
            self.updated_at_height = self.updated_at_height.max(self.context.height);
            V2VrfIngressOutcome::Accepted
        } else {
            self.late_reveals.insert(
                signer,
                VrfLateRevealRecord {
                    signer,
                    reveal: reveal.reveal,
                    reveal_proof: Some(VrfRevealProof {
                        epoch: reveal.epoch,
                        reveal: reveal.reveal,
                        signer: reveal.signer,
                        signature: reveal.bls_sig,
                        observed_at_height: self.context.height,
                    }),
                    noted_at_height: self.context.height,
                },
            );
            self.updated_at_height = self.updated_at_height.max(self.context.height);
            V2VrfIngressOutcome::AcceptedLate
        }
    }

    fn retain_commit(&mut self, signer: u32, commit: VrfCommit) -> V2VrfIngressOutcome {
        let participant = self
            .participants
            .entry(signer)
            .or_insert(VrfParticipantRecord {
                signer,
                commitment: None,
                reveal: None,
                commit_proof: None,
                reveal_proof: None,
                last_updated_height: self.context.height,
            });
        participant.commitment = Some(commit.commitment);
        participant.commit_proof = Some(VrfCommitProof {
            epoch: commit.epoch,
            commitment: commit.commitment,
            signer: commit.signer,
            signature: commit.bls_sig,
            observed_at_height: self.context.height,
        });
        participant.last_updated_height = self.context.height;
        self.updated_at_height = self.updated_at_height.max(self.context.height);
        V2VrfIngressOutcome::Accepted
    }

    fn authenticate_commit(
        &self,
        commit: &VrfCommit,
        sender: Option<&PeerId>,
    ) -> Result<u32, V2VrfRejection> {
        if commit.epoch != self.context.epoch {
            return Err(V2VrfRejection::EpochMismatch);
        }
        if self.schedule.position > self.schedule.commit_end {
            return Err(V2VrfRejection::OutOfWindow);
        }
        let peer = self.bound_sender(commit.signer, sender)?;
        let signature = Signature::try_from_bytes(&commit.bls_sig)
            .map_err(|_| V2VrfRejection::MalformedSignature)?;
        signature
            .verify(
                peer.public_key(),
                &vrf_commit_preimage(&self.context.chain_id, NPOS_TAG, commit),
            )
            .map_err(|_| V2VrfRejection::InvalidSignature)?;
        Ok(commit.signer)
    }

    fn authenticate_reveal(
        &self,
        reveal: &VrfReveal,
        sender: Option<&PeerId>,
    ) -> Result<u32, V2VrfRejection> {
        if reveal.epoch != self.context.epoch {
            return Err(V2VrfRejection::EpochMismatch);
        }
        if self.schedule.position <= self.schedule.commit_end {
            return Err(V2VrfRejection::OutOfWindow);
        }
        let peer = self.bound_sender(reveal.signer, sender)?;
        let signature = Signature::try_from_bytes(&reveal.bls_sig)
            .map_err(|_| V2VrfRejection::MalformedSignature)?;
        signature
            .verify(
                peer.public_key(),
                &vrf_reveal_preimage(&self.context.chain_id, NPOS_TAG, reveal),
            )
            .map_err(|_| V2VrfRejection::InvalidSignature)?;
        Ok(reveal.signer)
    }

    fn bound_sender(
        &self,
        signer: u32,
        sender: Option<&PeerId>,
    ) -> Result<&PeerId, V2VrfRejection> {
        let index = usize::try_from(signer).map_err(|_| V2VrfRejection::SignerOutOfRange)?;
        let peer = self
            .context
            .roster
            .get(index)
            .map(|entry| &entry.validator)
            .ok_or(V2VrfRejection::SignerOutOfRange)?;
        let sender = sender.ok_or(V2VrfRejection::MissingSender)?;
        if sender != peer {
            return Err(V2VrfRejection::SenderMismatch);
        }
        Ok(peer)
    }

    fn stage_local_message(
        &mut self,
        local_validator: Option<wire::ValidatorIndex>,
        key_pair: &KeyPair,
    ) -> Result<(), V2NposError> {
        let Some(local_validator) = local_validator else {
            return Ok(());
        };
        let index =
            usize::try_from(local_validator).map_err(|_| V2NposError::LocalIdentityMismatch)?;
        let local_peer = self
            .context
            .roster
            .get(index)
            .map(|entry| entry.validator.clone())
            .ok_or(V2NposError::LocalIdentityMismatch)?;
        if local_peer.public_key() != key_pair.public_key() {
            return Err(V2NposError::LocalIdentityMismatch);
        }
        let chain_hash = Hash::new(self.context.chain_id.clone().into_inner().as_bytes());
        let (reveal, commitment) = derive_vrf_material_from_key(
            &chain_hash,
            key_pair.private_key(),
            self.context.epoch,
            local_validator,
        )
        .map_err(|error| V2NposError::LocalMaterial(error.to_string()))?;

        let existing = self.participants.get(&local_validator);
        if existing
            .and_then(|participant| participant.commitment)
            .is_some_and(|persisted| persisted != commitment)
        {
            return Err(V2NposError::LocalCommitmentMismatch);
        }

        if self.schedule.position <= self.schedule.commit_end {
            if existing
                .and_then(|participant| participant.commitment)
                .is_none()
            {
                let mut commit = VrfCommit {
                    epoch: self.context.epoch,
                    commitment,
                    signer: local_validator,
                    bls_sig: Vec::new(),
                };
                commit.bls_sig = Signature::try_new(
                    key_pair.private_key(),
                    &vrf_commit_preimage(&self.context.chain_id, NPOS_TAG, &commit),
                )
                .map_err(|error| V2NposError::LocalSignature(error.to_string()))?
                .payload()
                .to_vec();
                let outcome = self.accept_commit(commit.clone(), Some(&local_peer));
                if outcome != V2VrfIngressOutcome::Accepted {
                    return Err(V2NposError::LocalAdmission(outcome));
                }
                let message = super::message::BlockMessage::VrfCommit(commit);
                self.outbound.push(message.clone());
                self.retransmit = Some(message);
            }
            return Ok(());
        }

        if self.schedule.position <= self.schedule.reveal_end
            && existing
                .and_then(|participant| participant.commitment)
                .is_some()
            && existing
                .and_then(|participant| participant.reveal)
                .is_none()
            && !self.late_reveals.contains_key(&local_validator)
        {
            let mut reveal_message = VrfReveal {
                epoch: self.context.epoch,
                reveal,
                signer: local_validator,
                bls_sig: Vec::new(),
            };
            reveal_message.bls_sig = Signature::try_new(
                key_pair.private_key(),
                &vrf_reveal_preimage(&self.context.chain_id, NPOS_TAG, &reveal_message),
            )
            .map_err(|error| V2NposError::LocalSignature(error.to_string()))?
            .payload()
            .to_vec();
            let outcome = self.accept_reveal(reveal_message.clone(), Some(&local_peer));
            if outcome != V2VrfIngressOutcome::Accepted {
                return Err(V2NposError::LocalAdmission(outcome));
            }
            let message = super::message::BlockMessage::VrfReveal(reveal_message);
            self.outbound.push(message.clone());
            self.retransmit = Some(message);
        }
        Ok(())
    }

    fn record(&self) -> VrfEpochRecord {
        let finalized = self.context.height == self.context.epoch_end_height;
        let late_signers = self.late_reveals.keys().copied().collect::<BTreeSet<_>>();
        let (committed_no_reveal, no_participation) = if finalized {
            let committed_no_reveal = self
                .participants
                .values()
                .filter(|participant| {
                    participant.commitment.is_some()
                        && participant.reveal.is_none()
                        && !late_signers.contains(&participant.signer)
                })
                .map(|participant| participant.signer)
                .collect();
            let participated = self
                .participants
                .keys()
                .copied()
                .chain(late_signers.iter().copied())
                .collect::<BTreeSet<_>>();
            let no_participation = (0..self.roster_len)
                .filter(|signer| !participated.contains(signer))
                .collect();
            (committed_no_reveal, no_participation)
        } else {
            (Vec::new(), Vec::new())
        };
        VrfEpochRecord {
            epoch: self.context.epoch,
            seed: self.context.leader_seed,
            epoch_length: self.schedule.length,
            commit_deadline_offset: self.schedule.commit_end,
            reveal_deadline_offset: self.schedule.reveal_end,
            roster_len: self.roster_len,
            finalized,
            updated_at_height: if finalized {
                self.context.height
            } else {
                self.updated_at_height
            },
            participants: self.participants.values().cloned().collect(),
            late_reveals: self.late_reveals.values().cloned().collect(),
            committed_no_reveal,
            no_participation,
            penalties_applied: self.penalties_applied,
            penalties_applied_at_height: self.penalties_applied_at_height,
            validator_election: self.validator_election.clone(),
        }
    }

    fn pending_record(&self) -> Option<VrfEpochRecord> {
        let record = self.record();
        (self.committed_record.as_ref() != Some(&record)).then_some(record)
    }
}

const MAX_VRF_SIGNATURE_BYTES: usize = 512;

fn validate_extension_at_candidate_height(
    context: &wire::HeightContext,
    schedule: EpochSchedule,
    existing: Option<&VrfEpochRecord>,
    proposed: &VrfEpochRecord,
) -> Result<(), V2NposError> {
    let expected_penalties_applied = existing.is_some_and(|record| record.penalties_applied);
    let expected_penalties_height = existing.and_then(|record| record.penalties_applied_at_height);
    let expected_election = existing.and_then(|record| record.validator_election.as_ref());
    if proposed.penalties_applied != expected_penalties_applied
        || proposed.penalties_applied_at_height != expected_penalties_height
        || proposed.validator_election.as_ref() != expected_election
    {
        return Err(V2NposError::InvalidRecord(
            "candidate rewrites non-VRF epoch metadata",
        ));
    }
    let existing_participants = existing.map_or_else(BTreeMap::new, |record| {
        record
            .participants
            .iter()
            .map(|participant| (participant.signer, participant))
            .collect::<BTreeMap<_, _>>()
    });
    let existing_late = existing.map_or_else(BTreeMap::new, |record| {
        record
            .late_reveals
            .iter()
            .map(|reveal| (reveal.signer, reveal))
            .collect::<BTreeMap<_, _>>()
    });

    for participant in &proposed.participants {
        let old = existing_participants.get(&participant.signer).copied();
        if old.and_then(|record| record.commitment).is_none() {
            let proof = participant
                .commit_proof
                .as_ref()
                .ok_or(V2NposError::InvalidRecord(
                    "new commitment is missing its proof",
                ))?;
            if schedule.position > schedule.commit_end || proof.observed_at_height != context.height
            {
                return Err(V2NposError::InvalidRecord(
                    "new commitment is backdated or introduced outside the commit window",
                ));
            }
        }
        if old.and_then(|record| record.reveal).is_none() && participant.reveal.is_some() {
            let proof = participant
                .reveal_proof
                .as_ref()
                .ok_or(V2NposError::InvalidRecord(
                    "new reveal is missing its proof",
                ))?;
            if schedule.position <= schedule.commit_end
                || schedule.position > schedule.reveal_end
                || proof.observed_at_height != context.height
                || old.and_then(|record| record.commitment).is_none()
            {
                return Err(V2NposError::InvalidRecord(
                    "new reveal is backdated, outside the reveal window, or lacks a pre-state commitment",
                ));
            }
        }
    }

    for late in &proposed.late_reveals {
        if existing_late.contains_key(&late.signer) {
            continue;
        }
        let proof = late
            .reveal_proof
            .as_ref()
            .ok_or(V2NposError::InvalidRecord(
                "new late reveal is missing its proof",
            ))?;
        if schedule.position <= schedule.reveal_end
            || proof.observed_at_height != context.height
            || old_commitment_without_reveal(&existing_participants, late.signer).is_none()
        {
            return Err(V2NposError::InvalidRecord(
                "new late reveal is backdated, premature, or paired with a same-block commitment",
            ));
        }
    }
    Ok(())
}

fn old_commitment_without_reveal(
    participants: &BTreeMap<u32, &VrfParticipantRecord>,
    signer: u32,
) -> Option<[u8; 32]> {
    participants.get(&signer).and_then(|participant| {
        participant
            .commitment
            .filter(|_| participant.reveal.is_none())
    })
}

#[allow(clippy::too_many_lines)]
fn validate_authenticated_record(
    context: &wire::HeightContext,
    schedule: EpochSchedule,
    record: &VrfEpochRecord,
    roster_len: u32,
    boundary: bool,
) -> Result<(), V2NposError> {
    if record.epoch != context.epoch {
        return Err(V2NposError::InvalidRecord("epoch does not match context"));
    }
    if record.seed != context.leader_seed {
        return Err(V2NposError::InvalidRecord(
            "seed does not match frozen context",
        ));
    }
    if record.epoch_length != schedule.length
        || record.commit_deadline_offset != schedule.commit_end
        || record.reveal_deadline_offset != schedule.reveal_end
    {
        return Err(V2NposError::InvalidRecord(
            "window schedule does not match frozen epoch parameters",
        ));
    }
    if record.roster_len != roster_len {
        return Err(V2NposError::InvalidRecord(
            "roster length does not match frozen context",
        ));
    }
    if record.finalized != boundary {
        return Err(V2NposError::InvalidRecord(
            "finalized marker does not match epoch boundary",
        ));
    }
    if boundary {
        if record.updated_at_height != context.epoch_end_height {
            return Err(V2NposError::InvalidRecord(
                "finalized record is not sealed at the epoch boundary",
            ));
        }
    } else if record.updated_at_height > context.height {
        return Err(V2NposError::InvalidRecord(
            "record update height is ahead of candidate height",
        ));
    }
    if record.penalties_applied_at_height.is_some() && !record.penalties_applied {
        return Err(V2NposError::InvalidRecord(
            "penalty height is present without the applied marker",
        ));
    }

    let mut participant_signers = BTreeSet::new();
    let mut latest_observation_height = 0_u64;
    for participant in &record.participants {
        if participant.signer >= roster_len || !participant_signers.insert(participant.signer) {
            return Err(V2NposError::InvalidRecord(
                "participant signer is duplicated or outside the frozen roster",
            ));
        }
        let Some(commitment) = participant.commitment else {
            return Err(V2NposError::InvalidRecord(
                "participant has no authenticated commitment",
            ));
        };
        let Some(commit_proof) = participant.commit_proof.as_ref() else {
            return Err(V2NposError::InvalidRecord(
                "participant commitment proof is missing",
            ));
        };
        if commit_proof.epoch != record.epoch
            || commit_proof.signer != participant.signer
            || commit_proof.commitment != commitment
            || commit_proof.observed_at_height > record.updated_at_height
            || window_position(context, schedule, commit_proof.observed_at_height)
                .is_none_or(|position| position > schedule.commit_end)
        {
            return Err(V2NposError::InvalidRecord(
                "commit proof fields or observation window are inconsistent",
            ));
        }
        verify_commit_proof(context, commit_proof)?;

        let last_observed = if let Some(reveal) = participant.reveal {
            let actual: [u8; 32] = Hash::new(reveal).into();
            if actual != commitment {
                return Err(V2NposError::InvalidRecord(
                    "participant reveal does not open its commitment",
                ));
            }
            let Some(reveal_proof) = participant.reveal_proof.as_ref() else {
                return Err(V2NposError::InvalidRecord(
                    "participant reveal proof is missing",
                ));
            };
            if reveal_proof.epoch != record.epoch
                || reveal_proof.signer != participant.signer
                || reveal_proof.reveal != reveal
                || reveal_proof.observed_at_height > record.updated_at_height
                || window_position(context, schedule, reveal_proof.observed_at_height).is_none_or(
                    |position| position <= schedule.commit_end || position > schedule.reveal_end,
                )
            {
                return Err(V2NposError::InvalidRecord(
                    "reveal proof fields or observation window are inconsistent",
                ));
            }
            verify_reveal_proof(context, reveal_proof)?;
            commit_proof
                .observed_at_height
                .max(reveal_proof.observed_at_height)
        } else {
            if participant.reveal_proof.is_some() {
                return Err(V2NposError::InvalidRecord(
                    "reveal proof is present without a reveal",
                ));
            }
            commit_proof.observed_at_height
        };
        if participant.last_updated_height != last_observed {
            return Err(V2NposError::InvalidRecord(
                "participant update height does not match authenticated observations",
            ));
        }
        latest_observation_height = latest_observation_height.max(last_observed);
    }
    if !record
        .participants
        .windows(2)
        .all(|pair| pair[0].signer < pair[1].signer)
    {
        return Err(V2NposError::InvalidRecord(
            "participants are not in canonical signer order",
        ));
    }

    let participants = record
        .participants
        .iter()
        .map(|participant| (participant.signer, participant))
        .collect::<BTreeMap<_, _>>();
    let mut late_signers = BTreeSet::new();
    for late in &record.late_reveals {
        if late.signer >= roster_len || !late_signers.insert(late.signer) {
            return Err(V2NposError::InvalidRecord(
                "late-reveal signer is duplicated or outside the frozen roster",
            ));
        }
        let Some(proof) = late.reveal_proof.as_ref() else {
            return Err(V2NposError::InvalidRecord(
                "late reveal authentication proof is missing",
            ));
        };
        if proof.epoch != record.epoch
            || proof.signer != late.signer
            || proof.reveal != late.reveal
            || proof.observed_at_height != late.noted_at_height
            || proof.observed_at_height > record.updated_at_height
            || window_position(context, schedule, proof.observed_at_height)
                .is_none_or(|position| position <= schedule.reveal_end)
        {
            return Err(V2NposError::InvalidRecord(
                "late reveal proof fields or observation window are inconsistent",
            ));
        }
        let actual: [u8; 32] = Hash::new(late.reveal).into();
        if participants.get(&late.signer).is_none_or(|participant| {
            participant.commitment != Some(actual) || participant.reveal.is_some()
        }) {
            return Err(V2NposError::InvalidRecord(
                "late reveal does not open an authenticated unrevealed commitment",
            ));
        }
        verify_reveal_proof(context, proof)?;
        latest_observation_height = latest_observation_height.max(proof.observed_at_height);
    }
    if !record
        .late_reveals
        .windows(2)
        .all(|pair| pair[0].signer < pair[1].signer)
    {
        return Err(V2NposError::InvalidRecord(
            "late reveals are not in canonical signer order",
        ));
    }

    if !boundary && record.updated_at_height != latest_observation_height {
        return Err(V2NposError::InvalidRecord(
            "record update height is not derived from admitted proof observations",
        ));
    }

    if boundary {
        let expected_non_reveal = record
            .participants
            .iter()
            .filter(|participant| {
                participant.reveal.is_none() && !late_signers.contains(&participant.signer)
            })
            .map(|participant| participant.signer)
            .collect::<Vec<_>>();
        let expected_no_participation = (0..roster_len)
            .filter(|signer| !participant_signers.contains(signer))
            .collect::<Vec<_>>();
        if record.committed_no_reveal != expected_non_reveal
            || record.no_participation != expected_no_participation
        {
            return Err(V2NposError::InvalidRecord(
                "finalized offender partition is not exact",
            ));
        }
    } else if !record.committed_no_reveal.is_empty() || !record.no_participation.is_empty() {
        return Err(V2NposError::InvalidRecord(
            "non-boundary record contains finalized offender sets",
        ));
    }
    Ok(())
}

fn window_position(
    context: &wire::HeightContext,
    schedule: EpochSchedule,
    height: u64,
) -> Option<u64> {
    let start = context
        .epoch_end_height
        .checked_add(1)?
        .checked_sub(schedule.length)?;
    height
        .checked_sub(start)?
        .checked_add(1)
        .filter(|position| *position <= schedule.length)
}

fn verify_commit_proof(
    context: &wire::HeightContext,
    proof: &VrfCommitProof,
) -> Result<(), V2NposError> {
    if proof.signature.is_empty() || proof.signature.len() > MAX_VRF_SIGNATURE_BYTES {
        return Err(V2NposError::InvalidRecord(
            "commit signature length is outside the canonical bound",
        ));
    }
    let peer = context
        .roster
        .get(usize::try_from(proof.signer).map_err(|_| {
            V2NposError::InvalidRecord("commit signer index cannot address the frozen roster")
        })?)
        .ok_or(V2NposError::InvalidRecord(
            "commit signer is outside the frozen roster",
        ))?;
    let message = VrfCommit {
        epoch: proof.epoch,
        commitment: proof.commitment,
        signer: proof.signer,
        bls_sig: proof.signature.clone(),
    };
    let signature = Signature::try_from_bytes(&proof.signature)
        .map_err(|_| V2NposError::InvalidRecord("commit signature encoding is malformed"))?;
    signature
        .verify(
            peer.validator.public_key(),
            &vrf_commit_preimage(&context.chain_id, NPOS_TAG, &message),
        )
        .map_err(|_| V2NposError::InvalidRecord("commit signature verification failed"))
}

fn verify_reveal_proof(
    context: &wire::HeightContext,
    proof: &VrfRevealProof,
) -> Result<(), V2NposError> {
    if proof.signature.is_empty() || proof.signature.len() > MAX_VRF_SIGNATURE_BYTES {
        return Err(V2NposError::InvalidRecord(
            "reveal signature length is outside the canonical bound",
        ));
    }
    let peer = context
        .roster
        .get(usize::try_from(proof.signer).map_err(|_| {
            V2NposError::InvalidRecord("reveal signer index cannot address the frozen roster")
        })?)
        .ok_or(V2NposError::InvalidRecord(
            "reveal signer is outside the frozen roster",
        ))?;
    let message = VrfReveal {
        epoch: proof.epoch,
        reveal: proof.reveal,
        signer: proof.signer,
        bls_sig: proof.signature.clone(),
    };
    let signature = Signature::try_from_bytes(&proof.signature)
        .map_err(|_| V2NposError::InvalidRecord("reveal signature encoding is malformed"))?;
    signature
        .verify(
            peer.validator.public_key(),
            &vrf_reveal_preimage(&context.chain_id, NPOS_TAG, &message),
        )
        .map_err(|_| V2NposError::InvalidRecord("reveal signature verification failed"))
}

fn validate_persisted_record(
    context: &wire::HeightContext,
    schedule: EpochSchedule,
    record: &VrfEpochRecord,
    roster_len: u32,
) -> Result<(), V2NposError> {
    if record.epoch != context.epoch
        || record.seed != context.leader_seed
        || record.epoch_length != schedule.length
        || record.commit_deadline_offset != schedule.commit_end
        || record.reveal_deadline_offset != schedule.reveal_end
        || record.roster_len != roster_len
        || record.finalized
        || record.updated_at_height >= context.height
        || (record.penalties_applied_at_height.is_some() && !record.penalties_applied)
        || !record.committed_no_reveal.is_empty()
        || !record.no_participation.is_empty()
    {
        return Err(V2NposError::PersistedRecordConflict);
    }
    validate_authenticated_record(context, schedule, record, roster_len, false)
        .map_err(|_| V2NposError::PersistedRecordConflict)
}

fn record_extends(base: &VrfEpochRecord, candidate: &VrfEpochRecord) -> bool {
    if base.epoch != candidate.epoch
        || base.seed != candidate.seed
        || base.epoch_length != candidate.epoch_length
        || base.commit_deadline_offset != candidate.commit_deadline_offset
        || base.reveal_deadline_offset != candidate.reveal_deadline_offset
        || base.roster_len != candidate.roster_len
        || base.finalized && !candidate.finalized
        || base.updated_at_height > candidate.updated_at_height
        || base.penalties_applied && !candidate.penalties_applied
        || base
            .penalties_applied_at_height
            .is_some_and(|height| candidate.penalties_applied_at_height != Some(height))
        || base
            .validator_election
            .as_ref()
            .is_some_and(|election| candidate.validator_election.as_ref() != Some(election))
    {
        return false;
    }
    let participants = candidate
        .participants
        .iter()
        .map(|participant| (participant.signer, participant))
        .collect::<BTreeMap<_, _>>();
    let late = candidate
        .late_reveals
        .iter()
        .map(|reveal| (reveal.signer, reveal))
        .collect::<BTreeMap<_, _>>();
    base.participants.iter().all(|old| {
        participants.get(&old.signer).is_some_and(|new| {
            old.commitment
                .is_none_or(|value| new.commitment == Some(value))
                && old.reveal.is_none_or(|value| new.reveal == Some(value))
                && old
                    .commit_proof
                    .as_ref()
                    .is_none_or(|proof| new.commit_proof.as_ref() == Some(proof))
                && old
                    .reveal_proof
                    .as_ref()
                    .is_none_or(|proof| new.reveal_proof.as_ref() == Some(proof))
                && new.last_updated_height >= old.last_updated_height
        })
    }) && base
        .late_reveals
        .iter()
        .all(|old| late.get(&old.signer) == Some(&old))
        && base
            .committed_no_reveal
            .iter()
            .all(|signer| candidate.committed_no_reveal.contains(signer))
        && base
            .no_participation
            .iter()
            .all(|signer| candidate.no_participation.contains(signer))
}

#[cfg(test)]
mod tests {
    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_data_model::{
        ChainId,
        consensus::{NposConsensusEffects, VrfEpochRecord},
        parameter::system::SumeragiNposParameters,
    };

    use super::*;
    use crate::{kura::Kura, query::store::LiveQueryStore, state::World};

    fn keys() -> Vec<KeyPair> {
        let mut keys = (1_u8..=4)
            .map(|seed| {
                KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                    .expect("deterministic BLS key")
            })
            .collect::<Vec<_>>();
        keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
        keys
    }

    fn context(height: u64, keys: &[KeyPair]) -> wire::HeightContext {
        let roster = keys
            .iter()
            .map(|key| wire::ValidatorPower {
                validator: PeerId::new(key.public_key().clone()),
                power: 1,
            })
            .collect::<Vec<_>>();
        wire::HeightContext {
            chain_id: ChainId::from("v2-npos-vrf-test"),
            protocol_version: wire::PROTOCOL_VERSION,
            height,
            epoch: 0,
            epoch_end_height: 10,
            next_epoch_snapshot: None,
            snapshot_bootstrap: None,
            mode: wire::ConsensusMode::Npos,
            parent_commit_qc: (height > 1).then(|| unreachable_parent_qc(height - 1)),
            quorum: wire::DualQuorum::from_roster(&roster).expect("quorum"),
            roster,
            nexus_amx_context_hash: Hash::new(b"v2-npos-test-nexus"),
            da_layout: wire::DataAvailabilityLayout {
                encoding: wire::PayloadEncoding::Plain,
                chunk_size_bytes: 1024,
                data_shards: 0,
                parity_shards: 0,
                max_payload_size_bytes: 4096,
                max_chunk_count: 4,
            },
            leader_seed: [0x44; 32],
        }
    }

    fn unreachable_parent_qc(parent_height: u64) -> wire::QuorumCertificate {
        // Unit construction below does not call HeightContext::validate; only
        // the fields consumed by the lifecycle are relevant after height one.
        wire::QuorumCertificate {
            round: wire::ConsensusRound {
                context_id: wire::HeightContextId(iroha_crypto::HashOf::from_untyped_unchecked(
                    Hash::new(b"parent-context"),
                )),
                height: parent_height,
                view: 0,
            },
            proposal_round: wire::ConsensusRound {
                context_id: wire::HeightContextId(iroha_crypto::HashOf::from_untyped_unchecked(
                    Hash::new(b"parent-context"),
                )),
                height: parent_height,
                view: 0,
            },
            phase: wire::GlobalPhase::Commit,
            subject: wire::BlockSubject {
                parent_block_hash: None,
                block_hash: iroha_crypto::HashOf::from_untyped_unchecked(Hash::new(
                    b"parent-block",
                )),
                payload_hash: Hash::new(b"parent-payload"),
            },
            execution_commitment: wire::ExecutionCommitment::without_topups(
                Hash::new(b"parent-state"),
                Hash::new(b"parent-post-state"),
                Hash::new(b"parent-ordinary-writes"),
                Hash::new(b"parent-executed-block-wire"),
            ),
            signers: vec![0, 1, 2],
            aggregate_signature: vec![1],
        }
    }

    fn schedule(position: u64) -> EpochSchedule {
        EpochSchedule {
            length: 10,
            commit_end: 3,
            reveal_end: 6,
            position,
        }
    }

    fn state_with_record(record: Option<VrfEpochRecord>) -> State {
        let world = World::new();
        {
            let mut block = world.block();
            let mut params = SumeragiNposParameters::default();
            params.epoch_length_blocks = core::num::NonZeroU64::new(10).expect("non-zero epoch");
            params.vrf_commit_window_blocks = 3;
            params.vrf_reveal_window_blocks = 3;
            block.parameters.get_mut().custom.insert(
                SumeragiNposParameters::parameter_id(),
                params.into_custom_parameter(),
            );
            if let Some(record) = record {
                block.vrf_epochs.insert(record.epoch, record);
            }
            block.commit();
        }
        State::new_with_chain_for_testing(
            world,
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
            ChainId::from("v2-npos-vrf-test"),
        )
    }

    fn effects(record: VrfEpochRecord) -> NposConsensusEffects {
        NposConsensusEffects {
            vrf_epoch_seals: vec![record],
            v2_evidence_admissions: Vec::new(),
            penalty_actions: Vec::new(),
        }
    }

    fn lifecycle_at(
        height: u64,
        position: u64,
        committed: Option<VrfEpochRecord>,
        local: Option<u32>,
        keys: &[KeyPair],
    ) -> V2NposVrfLifecycle {
        V2NposVrfLifecycle::from_parts(
            context(height, keys),
            schedule(position),
            committed,
            local,
            &keys[local.unwrap_or(0) as usize],
            keys.len(),
        )
        .expect("lifecycle")
    }

    fn sign_commit(
        key: &KeyPair,
        context: &wire::HeightContext,
        mut commit: VrfCommit,
    ) -> VrfCommit {
        commit.bls_sig = Signature::try_new(
            key.private_key(),
            &vrf_commit_preimage(&context.chain_id, NPOS_TAG, &commit),
        )
        .expect("commit signature")
        .payload()
        .to_vec();
        commit
    }

    fn sign_reveal(
        key: &KeyPair,
        context: &wire::HeightContext,
        mut reveal: VrfReveal,
    ) -> VrfReveal {
        reveal.bls_sig = Signature::try_new(
            key.private_key(),
            &vrf_reveal_preimage(&context.chain_id, NPOS_TAG, &reveal),
        )
        .expect("reveal signature")
        .payload()
        .to_vec();
        reveal
    }

    #[test]
    fn authoritative_schedule_requires_committed_parameters() {
        let keys = keys();
        let context = context(1, &keys);
        let world = World::new();
        let missing = State::new_with_chain_for_testing(
            world,
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
            ChainId::from("v2-npos-vrf-test"),
        );
        assert!(matches!(
            V2NposVrfLifecycle::open(&context, &missing, None, &keys[0]),
            Err(V2NposError::MissingCommittedParameters)
        ));

        let state = state_with_record(None);
        let lifecycle =
            V2NposVrfLifecycle::open(&context, &state, None, &keys[0]).expect("committed schedule");
        assert_eq!(
            lifecycle.active.as_ref().map(|active| active.schedule),
            Some(schedule(1))
        );
    }

    #[test]
    fn authoritative_schedule_rejects_invalid_committed_windows() {
        let keys = keys();
        let context = context(1, &keys);
        let state = state_with_record(None);
        {
            let mut world = state.world.block();
            let invalid = SumeragiNposParameters {
                epoch_length_blocks: core::num::NonZeroU64::new(10).expect("non-zero epoch"),
                vrf_commit_window_blocks: u64::MAX,
                vrf_reveal_window_blocks: 1,
                ..Default::default()
            };
            world.parameters.get_mut().custom.insert(
                SumeragiNposParameters::parameter_id(),
                invalid.into_custom_parameter(),
            );
            world.commit();
        }

        assert!(matches!(
            V2NposVrfLifecycle::open(&context, &state, None, &keys[0]),
            Err(V2NposError::MissingCommittedParameters)
        ));
        assert!(matches!(
            validate_candidate_records(&context, &state, None),
            Err(V2NposError::MissingCommittedParameters)
        ));
    }

    #[test]
    fn epoch_start_candidate_must_commit_the_schedule_snapshot() {
        let keys = keys();
        let context = context(1, &keys);
        let state = state_with_record(None);

        assert!(matches!(
            validate_candidate_records(&context, &state, None),
            Err(V2NposError::MissingEpochStartRecord)
        ));

        let lifecycle = V2NposVrfLifecycle::open(&context, &state, Some(0), &keys[0])
            .expect("open epoch-start lifecycle");
        let record = lifecycle
            .pending_records()
            .pop()
            .expect("epoch-start schedule record");
        validate_candidate_records(&context, &state, Some(&effects(record)))
            .expect("epoch-start record freezes the committed schedule");
    }

    #[test]
    fn mid_epoch_parameter_update_reuses_epoch_start_schedule() {
        let keys = keys();
        let first_context = context(1, &keys);
        let state = state_with_record(None);
        let first = V2NposVrfLifecycle::open(&first_context, &state, Some(0), &keys[0])
            .expect("open first epoch height");
        let first_record = first
            .pending_records()
            .pop()
            .expect("mandatory epoch-start record");
        validate_candidate_records(&first_context, &state, Some(&effects(first_record.clone())))
            .expect("valid epoch-start record");

        // Model one finalized block that both persists the mandatory snapshot
        // and changes the on-chain schedule. The update is valid for a future
        // epoch but is deliberately incompatible with the active context's end
        // height, so consulting it at height two would fail construction.
        {
            let mut world = state.world.block();
            world.vrf_epochs.insert(first_record.epoch, first_record);
            let params = SumeragiNposParameters {
                epoch_length_blocks: core::num::NonZeroU64::new(12).expect("non-zero epoch"),
                vrf_commit_window_blocks: 4,
                vrf_reveal_window_blocks: 4,
                ..Default::default()
            };
            world.parameters.get_mut().custom.insert(
                SumeragiNposParameters::parameter_id(),
                params.into_custom_parameter(),
            );
            world.commit();
        }

        let second_context = context(2, &keys);
        let reopened = V2NposVrfLifecycle::open(&second_context, &state, None, &keys[0])
            .expect("active epoch must reopen from its height-one snapshot");
        assert_eq!(
            reopened.active.as_ref().map(|active| active.schedule),
            Some(EpochSchedule {
                length: 10,
                commit_end: 3,
                reveal_end: 6,
                position: 2,
            })
        );
        validate_candidate_records(&second_context, &state, None)
            .expect("unchanged active-epoch record remains valid after parameter update");
    }

    #[test]
    fn wrong_sender_key_index_epoch_window_and_signature_are_rejected() {
        let keys = keys();
        let mut lifecycle = lifecycle_at(2, 2, None, None, &keys);
        let context = context(2, &keys);
        let commitment = [0x31; 32];
        let base = VrfCommit {
            epoch: 0,
            commitment,
            signer: 0,
            bls_sig: Vec::new(),
        };
        let valid = sign_commit(&keys[0], &context, base.clone());
        assert_eq!(
            lifecycle.accept_commit(valid.clone(), None),
            V2VrfIngressOutcome::Rejected(V2VrfRejection::MissingSender)
        );
        assert_eq!(
            lifecycle.accept_commit(valid.clone(), Some(&context.roster[1].validator)),
            V2VrfIngressOutcome::Rejected(V2VrfRejection::SenderMismatch)
        );
        let wrong_key = sign_commit(&keys[1], &context, base.clone());
        assert_eq!(
            lifecycle.accept_commit(wrong_key, Some(&context.roster[0].validator)),
            V2VrfIngressOutcome::Rejected(V2VrfRejection::InvalidSignature)
        );
        let mut wrong_epoch = base.clone();
        wrong_epoch.epoch = 1;
        let wrong_epoch = sign_commit(&keys[0], &context, wrong_epoch);
        assert_eq!(
            lifecycle.accept_commit(wrong_epoch, Some(&context.roster[0].validator)),
            V2VrfIngressOutcome::Rejected(V2VrfRejection::EpochMismatch)
        );
        let mut wrong_index = base;
        wrong_index.signer = 99;
        assert_eq!(
            lifecycle.accept_commit(wrong_index, Some(&context.roster[0].validator)),
            V2VrfIngressOutcome::Rejected(V2VrfRejection::SignerOutOfRange)
        );

        let mut reveal_window = lifecycle_at(4, 4, None, None, &keys);
        assert_eq!(
            reveal_window.accept_commit(valid, Some(&context.roster[0].validator)),
            V2VrfIngressOutcome::Rejected(V2VrfRejection::OutOfWindow)
        );
        let reveal_too_early = sign_reveal(
            &keys[0],
            &context,
            VrfReveal {
                epoch: 0,
                reveal: [0x22; 32],
                signer: 0,
                bls_sig: Vec::new(),
            },
        );
        assert_eq!(
            lifecycle.accept_reveal(reveal_too_early, Some(&context.roster[0].validator)),
            V2VrfIngressOutcome::Rejected(V2VrfRejection::OutOfWindow)
        );
        let malformed = VrfReveal {
            epoch: 0,
            reveal: [0x22; 32],
            signer: 0,
            bls_sig: vec![0; 96],
        };
        assert_eq!(
            reveal_window.accept_reveal(malformed, Some(&context.roster[0].validator)),
            V2VrfIngressOutcome::Rejected(V2VrfRejection::MalformedSignature)
        );
    }

    #[test]
    fn reveal_requires_matching_commitment_and_conflicts_are_deduplicated() {
        let keys = keys();
        let commit_context = context(2, &keys);
        let reveal = [0x52; 32];
        let commitment: [u8; 32] = Hash::new(reveal).into();
        let commit = sign_commit(
            &keys[0],
            &commit_context,
            VrfCommit {
                epoch: 0,
                commitment,
                signer: 0,
                bls_sig: Vec::new(),
            },
        );
        let mut commit_lifecycle = lifecycle_at(2, 2, None, None, &keys);
        assert_eq!(
            commit_lifecycle
                .accept_commit(commit.clone(), Some(&commit_context.roster[0].validator)),
            V2VrfIngressOutcome::Accepted
        );
        assert_eq!(
            commit_lifecycle.accept_commit(commit, Some(&commit_context.roster[0].validator)),
            V2VrfIngressOutcome::Duplicate
        );
        let conflict = sign_commit(
            &keys[0],
            &commit_context,
            VrfCommit {
                epoch: 0,
                commitment: [0x99; 32],
                signer: 0,
                bls_sig: Vec::new(),
            },
        );
        assert_eq!(
            commit_lifecycle.accept_commit(conflict, Some(&commit_context.roster[0].validator)),
            V2VrfIngressOutcome::Rejected(V2VrfRejection::ConflictingCommitment)
        );

        let committed = commit_lifecycle
            .pending_records()
            .pop()
            .expect("commit record");
        let reveal_context = context(4, &keys);
        let mut reveal_lifecycle = lifecycle_at(4, 4, Some(committed), None, &keys);
        let missing = sign_reveal(
            &keys[1],
            &reveal_context,
            VrfReveal {
                epoch: 0,
                reveal,
                signer: 1,
                bls_sig: Vec::new(),
            },
        );
        assert_eq!(
            reveal_lifecycle.accept_reveal(missing, Some(&reveal_context.roster[1].validator)),
            V2VrfIngressOutcome::Rejected(V2VrfRejection::MissingCommitment)
        );
        let mismatch = sign_reveal(
            &keys[0],
            &reveal_context,
            VrfReveal {
                epoch: 0,
                reveal: [0x53; 32],
                signer: 0,
                bls_sig: Vec::new(),
            },
        );
        assert_eq!(
            reveal_lifecycle.accept_reveal(mismatch, Some(&reveal_context.roster[0].validator)),
            V2VrfIngressOutcome::Rejected(V2VrfRejection::CommitmentMismatch)
        );
        let valid = sign_reveal(
            &keys[0],
            &reveal_context,
            VrfReveal {
                epoch: 0,
                reveal,
                signer: 0,
                bls_sig: Vec::new(),
            },
        );
        assert_eq!(
            reveal_lifecycle
                .accept_reveal(valid.clone(), Some(&reveal_context.roster[0].validator)),
            V2VrfIngressOutcome::Accepted
        );
        assert_eq!(
            reveal_lifecycle.accept_reveal(valid, Some(&reveal_context.roster[0].validator)),
            V2VrfIngressOutcome::Duplicate
        );
    }

    #[test]
    fn capacity_restart_and_boundary_seal_are_bounded_and_deterministic() {
        let keys = keys();
        let mut first = V2NposVrfLifecycle::from_parts(
            context(2, &keys),
            schedule(2),
            None,
            Some(0),
            &keys[0],
            1,
        )
        .expect("first lifecycle");
        let first_outbound = first.take_outbound();
        let first_retransmission = first.retransmission();
        let first_record = first.pending_records().pop().expect("pending local commit");

        let mut restarted = V2NposVrfLifecycle::from_parts(
            context(2, &keys),
            schedule(2),
            None,
            Some(0),
            &keys[0],
            1,
        )
        .expect("restarted lifecycle");
        let restarted_record = restarted
            .pending_records()
            .pop()
            .expect("re-emitted commit");
        assert_eq!(first_record, restarted_record);
        assert_eq!(first_outbound.len(), 1);
        assert!(matches!(
            (first_outbound.as_slice(), first_retransmission.as_slice()),
            ([super::super::message::BlockMessage::VrfCommit(first)],
             [super::super::message::BlockMessage::VrfCommit(repeated)])
                if first.epoch == repeated.epoch
                    && first.signer == repeated.signer
                    && first.commitment == repeated.commitment
                    && first.bls_sig == repeated.bls_sig
        ));
        assert_eq!(restarted.take_outbound().len(), 1);

        let ctx = context(2, &keys);
        let second = sign_commit(
            &keys[1],
            &ctx,
            VrfCommit {
                epoch: 0,
                commitment: [0xA1; 32],
                signer: 1,
                bls_sig: Vec::new(),
            },
        );
        assert_eq!(
            first.accept_commit(second, Some(&ctx.roster[1].validator)),
            V2VrfIngressOutcome::Rejected(V2VrfRejection::Capacity)
        );

        let boundary = lifecycle_at(10, 10, Some(first_record), None, &keys);
        let seal = boundary.pending_records().pop().expect("boundary seal");
        assert!(seal.finalized);
        assert_eq!(seal.updated_at_height, 10);
        assert_eq!(seal.committed_no_reveal, vec![0]);
        assert_eq!(seal.no_participation, vec![1, 2, 3]);
    }

    #[test]
    fn committed_local_commit_recovers_and_emits_reveal_after_restart() {
        let keys = keys();
        let commit_height = lifecycle_at(2, 2, None, Some(0), &keys);
        let committed = commit_height
            .pending_records()
            .pop()
            .expect("commit record");
        let mut reveal_height = lifecycle_at(4, 4, Some(committed), Some(0), &keys);
        assert!(matches!(
            reveal_height.take_outbound().as_slice(),
            [super::super::message::BlockMessage::VrfReveal(_)]
        ));
        let record = reveal_height
            .pending_records()
            .pop()
            .expect("reveal record");
        assert!(record.participants[0].reveal.is_some());
    }

    #[test]
    fn authenticated_late_reveal_relaxes_boundary_penalty_without_changing_entropy_reveal() {
        let keys = keys();
        let commit_context = context(2, &keys);
        let reveal = [0x72; 32];
        let commitment: [u8; 32] = Hash::new(reveal).into();
        let mut commit_height = lifecycle_at(2, 2, None, None, &keys);
        let commit = sign_commit(
            &keys[0],
            &commit_context,
            VrfCommit {
                epoch: 0,
                commitment,
                signer: 0,
                bls_sig: Vec::new(),
            },
        );
        assert_eq!(
            commit_height.accept_commit(commit, Some(&commit_context.roster[0].validator)),
            V2VrfIngressOutcome::Accepted
        );
        let committed = commit_height
            .pending_records()
            .pop()
            .expect("commit record");

        let late_context = context(8, &keys);
        let mut late_height = lifecycle_at(8, 8, Some(committed.clone()), None, &keys);
        let late = sign_reveal(
            &keys[0],
            &late_context,
            VrfReveal {
                epoch: 0,
                reveal,
                signer: 0,
                bls_sig: Vec::new(),
            },
        );
        assert_eq!(
            late_height.accept_reveal(late, Some(&late_context.roster[0].validator)),
            V2VrfIngressOutcome::AcceptedLate
        );
        let late_record = late_height.pending_records().pop().expect("late record");
        assert!(late_record.participants[0].reveal.is_none());
        assert_eq!(late_record.late_reveals.len(), 1);
        validate_candidate_records(
            &late_context,
            &state_with_record(Some(committed.clone())),
            Some(&effects(late_record.clone())),
        )
        .expect("late reveal extends a pre-state commitment");
        assert!(
            validate_candidate_records(
                &late_context,
                &state_with_record(None),
                Some(&effects(late_record.clone())),
            )
            .is_err(),
            "a candidate must not backdate a same-block commitment and late reveal"
        );

        let boundary = lifecycle_at(10, 10, Some(late_record), None, &keys);
        let seal = boundary.pending_records().pop().expect("boundary seal");
        assert!(!seal.committed_no_reveal.contains(&0));
        assert!(!seal.no_participation.contains(&0));
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn candidate_record_verifies_exact_commit_proof_and_rejects_fabrication() {
        let keys = keys();
        let commit_context = context(2, &keys);
        let mut lifecycle = lifecycle_at(2, 2, None, None, &keys);
        let signed = sign_commit(
            &keys[0],
            &commit_context,
            VrfCommit {
                epoch: 0,
                commitment: [0x91; 32],
                signer: 0,
                bls_sig: Vec::new(),
            },
        );
        assert_eq!(
            lifecycle.accept_commit(signed.clone(), Some(&commit_context.roster[0].validator)),
            V2VrfIngressOutcome::Accepted
        );
        let record = lifecycle.pending_records().pop().expect("commit record");
        let proof = record.participants[0]
            .commit_proof
            .as_ref()
            .expect("retained commit proof");
        assert_eq!(proof.signature, signed.bls_sig);
        assert_eq!(proof.observed_at_height, commit_context.height);
        validate_candidate_records(
            &commit_context,
            &state_with_record(None),
            Some(&effects(record.clone())),
        )
        .expect("authentic first commit is valid");

        let extension_context = context(3, &keys);
        let mut extension_lifecycle = lifecycle_at(3, 3, Some(record.clone()), None, &keys);
        let extension = sign_commit(
            &keys[1],
            &extension_context,
            VrfCommit {
                epoch: 0,
                commitment: [0x92; 32],
                signer: 1,
                bls_sig: Vec::new(),
            },
        );
        assert_eq!(
            extension_lifecycle
                .accept_commit(extension, Some(&extension_context.roster[1].validator),),
            V2VrfIngressOutcome::Accepted
        );
        let extension_record = extension_lifecycle
            .pending_records()
            .pop()
            .expect("monotonic commit extension");
        validate_candidate_records(
            &extension_context,
            &state_with_record(Some(record.clone())),
            Some(&effects(extension_record)),
        )
        .expect("non-boundary record remains a valid monotonic extension");

        let mut forged = record.clone();
        forged.participants[0]
            .commit_proof
            .as_mut()
            .expect("commit proof")
            .signature[0] ^= 0x80;
        assert!(
            validate_candidate_records(
                &commit_context,
                &state_with_record(None),
                Some(&effects(forged)),
            )
            .is_err()
        );

        let mut backdated = record.clone();
        backdated.participants[0]
            .commit_proof
            .as_mut()
            .expect("commit proof")
            .observed_at_height = 1;
        backdated.participants[0].last_updated_height = 1;
        assert!(matches!(
            validate_candidate_records(
                &commit_context,
                &state_with_record(None),
                Some(&effects(backdated)),
            ),
            Err(V2NposError::InvalidRecord(_))
        ));

        let mut wrong_signer = record.clone();
        wrong_signer.participants[0].signer = 1;
        wrong_signer.participants[0]
            .commit_proof
            .as_mut()
            .expect("commit proof")
            .signer = 1;
        assert!(
            validate_candidate_records(
                &commit_context,
                &state_with_record(None),
                Some(&effects(wrong_signer)),
            )
            .is_err()
        );

        let mut replay_context = commit_context.clone();
        replay_context.chain_id = ChainId::from("v2-npos-vrf-other-chain");
        assert!(
            validate_candidate_records(
                &replay_context,
                &state_with_record(None),
                Some(&effects(record.clone())),
            )
            .is_err()
        );

        let mut duplicate = record.clone();
        duplicate
            .participants
            .push(duplicate.participants[0].clone());
        assert!(
            validate_candidate_records(
                &commit_context,
                &state_with_record(None),
                Some(&effects(duplicate)),
            )
            .is_err()
        );

        let mut wrong_schedule = record.clone();
        wrong_schedule.epoch_length = 9;
        assert!(
            validate_candidate_records(
                &commit_context,
                &state_with_record(None),
                Some(&effects(wrong_schedule)),
            )
            .is_err()
        );

        let mut rewritten = record.clone();
        rewritten.updated_at_height = 3;
        rewritten.participants[0]
            .commit_proof
            .as_mut()
            .expect("commit proof")
            .observed_at_height = 3;
        rewritten.participants[0].last_updated_height = 3;
        assert!(
            validate_candidate_records(
                &context(3, &keys),
                &state_with_record(Some(record)),
                Some(&effects(rewritten)),
            )
            .is_err(),
            "unsigned observation height is immutable once committed"
        );

        let mut forged_metadata = lifecycle.pending_records().pop().expect("commit record");
        forged_metadata.penalties_applied = true;
        forged_metadata.penalties_applied_at_height = Some(2);
        assert!(
            validate_candidate_records(
                &commit_context,
                &state_with_record(None),
                Some(&effects(forged_metadata)),
            )
            .is_err()
        );
    }

    #[test]
    fn reveal_extension_requires_committed_commit_and_current_height_admission() {
        let keys = keys();
        let commit_context = context(2, &keys);
        let reveal = [0xA4; 32];
        let commitment: [u8; 32] = Hash::new(reveal).into();
        let mut commit_lifecycle = lifecycle_at(2, 2, None, None, &keys);
        let commit = sign_commit(
            &keys[0],
            &commit_context,
            VrfCommit {
                epoch: 0,
                commitment,
                signer: 0,
                bls_sig: Vec::new(),
            },
        );
        assert_eq!(
            commit_lifecycle.accept_commit(commit, Some(&commit_context.roster[0].validator)),
            V2VrfIngressOutcome::Accepted
        );
        let committed = commit_lifecycle
            .pending_records()
            .pop()
            .expect("commit record");

        let reveal_context = context(5, &keys);
        let mut reveal_lifecycle = lifecycle_at(5, 5, Some(committed.clone()), None, &keys);
        let signed_reveal = sign_reveal(
            &keys[0],
            &reveal_context,
            VrfReveal {
                epoch: 0,
                reveal,
                signer: 0,
                bls_sig: Vec::new(),
            },
        );
        assert_eq!(
            reveal_lifecycle.accept_reveal(
                signed_reveal.clone(),
                Some(&reveal_context.roster[0].validator)
            ),
            V2VrfIngressOutcome::Accepted
        );
        let revealed = reveal_lifecycle
            .pending_records()
            .pop()
            .expect("reveal extension");
        assert_eq!(
            revealed.participants[0]
                .reveal_proof
                .as_ref()
                .expect("reveal proof")
                .signature,
            signed_reveal.bls_sig
        );
        validate_candidate_records(
            &reveal_context,
            &state_with_record(Some(committed.clone())),
            Some(&effects(revealed.clone())),
        )
        .expect("current-height reveal extends committed commitment");

        let mut backdated = revealed.clone();
        backdated.participants[0]
            .reveal_proof
            .as_mut()
            .expect("reveal proof")
            .observed_at_height = 4;
        backdated.participants[0].last_updated_height = 4;
        assert!(
            validate_candidate_records(
                &reveal_context,
                &state_with_record(Some(committed.clone())),
                Some(&effects(backdated)),
            )
            .is_err()
        );

        assert!(
            validate_candidate_records(
                &reveal_context,
                &state_with_record(None),
                Some(&effects(revealed.clone())),
            )
            .is_err()
        );

        let mut mismatched = revealed;
        mismatched.participants[0].reveal = Some([0xA5; 32]);
        assert!(
            validate_candidate_records(
                &reveal_context,
                &state_with_record(Some(committed)),
                Some(&effects(mismatched)),
            )
            .is_err()
        );
    }

    #[test]
    fn boundary_requires_one_exact_authenticated_seal() {
        let keys = keys();
        let commit_context = context(2, &keys);
        let mut lifecycle = lifecycle_at(2, 2, None, None, &keys);
        let signed = sign_commit(
            &keys[0],
            &commit_context,
            VrfCommit {
                epoch: 0,
                commitment: [0xB1; 32],
                signer: 0,
                bls_sig: Vec::new(),
            },
        );
        assert_eq!(
            lifecycle.accept_commit(signed, Some(&commit_context.roster[0].validator)),
            V2VrfIngressOutcome::Accepted
        );
        let committed = lifecycle.pending_records().pop().expect("commit record");
        let mut boundary_context = context(10, &keys);
        boundary_context.next_epoch_snapshot = Some(wire::finality::FinalizedNextEpochSnapshot {
            epoch: boundary_context.epoch + 1,
            epoch_end_height: 20,
            mode: boundary_context.mode,
            roster: boundary_context.roster.clone(),
            validator_set_pops: keys
                .iter()
                .map(|key| {
                    iroha_crypto::bls_normal_pop_prove(key.private_key())
                        .expect("next-epoch validator proof of possession")
                })
                .collect(),
            quorum: boundary_context.quorum,
            leader_seed: [0x45; 32],
        });
        boundary_context
            .validate()
            .expect("boundary fixture carries the mandatory next-epoch snapshot");
        let boundary = lifecycle_at(10, 10, Some(committed.clone()), None, &keys);
        let seal = boundary.pending_records().pop().expect("boundary seal");
        validate_candidate_records(
            &boundary_context,
            &state_with_record(Some(committed.clone())),
            Some(&effects(seal.clone())),
        )
        .expect("exact boundary seal");

        assert!(matches!(
            validate_candidate_records(
                &boundary_context,
                &state_with_record(Some(committed.clone())),
                None,
            ),
            Err(V2NposError::MissingBoundarySeal)
        ));

        let duplicate = NposConsensusEffects {
            vrf_epoch_seals: vec![seal.clone(), seal.clone()],
            v2_evidence_admissions: Vec::new(),
            penalty_actions: Vec::new(),
        };
        assert!(
            validate_candidate_records(
                &boundary_context,
                &state_with_record(Some(committed.clone())),
                Some(&duplicate),
            )
            .is_err()
        );

        let mut unfinalized = seal.clone();
        unfinalized.finalized = false;
        assert!(
            validate_candidate_records(
                &boundary_context,
                &state_with_record(Some(committed.clone())),
                Some(&effects(unfinalized)),
            )
            .is_err()
        );

        let mut wrong_boundary = seal.clone();
        wrong_boundary.updated_at_height = 9;
        assert!(
            validate_candidate_records(
                &boundary_context,
                &state_with_record(Some(committed.clone())),
                Some(&effects(wrong_boundary)),
            )
            .is_err()
        );

        let mut forged_offenders = seal;
        forged_offenders.no_participation.clear();
        assert!(
            validate_candidate_records(
                &boundary_context,
                &state_with_record(Some(committed)),
                Some(&effects(forged_offenders)),
            )
            .is_err()
        );
    }
}
