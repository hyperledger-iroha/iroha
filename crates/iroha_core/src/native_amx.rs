//! Native AMX control-plane messages and deterministic vote-session cache.

use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    num::NonZeroUsize,
    path::{Path, PathBuf},
};

use iroha_crypto::{Algorithm, Hash, HashOf, PublicKey, Signature};
use iroha_data_model::{
    block::consensus::{
        LaneBlockCommitment, LaneBlockProposalV1, NativeAmxAttestationBodyV2,
        NativeAmxAttestationQcV2, NativeAmxLegRecordV2, NativeAmxPhase, NativeAmxReceipt,
    },
    block::consensus_v2::{ConsensusRound, HeightContextId},
    consensus::VALIDATOR_SET_HASH_VERSION_V1,
    nexus::{DataSpaceId, LaneId},
    peer::PeerId,
    transaction::TransactionEntrypoint,
};
use norito::codec::{Decode, Encode};
use parking_lot::Mutex;
use thiserror::Error;

#[cfg(unix)]
use std::{
    fs::DirBuilder,
    os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt},
};

use crate::queue::{RouteLeg, RouteLegRole, RoutingDecision, RoutingPlan, RoutingPlan::NativeAmx};

const DEFAULT_SESSION_BODY_BUCKET_MAX: usize = 256;
const NATIVE_AMX_SIGNING_GUARD_VERSION: u8 = 4;
#[cfg(unix)]
const NATIVE_AMX_SIGNING_GUARD_DIRECTORY: &str = "native-amx-v2-signing-guard-v4";
#[cfg(unix)]
const NATIVE_AMX_LEGACY_SIGNING_GUARD_DIRECTORIES: &[&str] = &[
    "native-amx-v2-signing-guard-v1",
    "native-amx-v2-signing-guard-v2",
    "native-amx-v2-signing-guard-v3",
];
const NATIVE_AMX_SIGNING_GUARD_RECORD_EXTENSION: &str = "norito";
const NATIVE_AMX_SIGNING_GUARD_TEMP_EXTENSION: &str = "norito.tmp";
const NATIVE_AMX_SIGNING_GUARD_LOCK_FILE: &str = "owner.lock";
const NATIVE_AMX_SIGNING_GUARD_ANCHOR_FILE: &str = "chain-anchor.norito";
const NATIVE_AMX_SIGNING_GUARD_ANCHOR_TEMP: &str = "chain-anchor.norito.tmp";
#[cfg(unix)]
const NATIVE_AMX_SIGNER_DIRECTORY_DOMAIN: &[u8] = b"iroha:native-amx:v2:signer-directory:v1\0";
const NATIVE_AMX_SIGNING_BODY_DOMAIN: &[u8] = b"iroha:native-amx:v2:signing-body:v4\0";
const NATIVE_AMX_SIGNING_RECORD_DOMAIN: &[u8] = b"iroha:native-amx:v2:record-chain:v4\0";
const NATIVE_AMX_SIGNING_GENESIS_DOMAIN: &[u8] = b"iroha:native-amx:v2:record-genesis:v4\0";
/// Absolute bound for durable Native AMX signing decisions retained at one height.
pub(crate) const MAX_NATIVE_AMX_SIGNING_GUARD_RECORDS_HARD: usize =
    iroha_config::parameters::defaults::sumeragi::V2_NATIVE_AMX_SIGNING_GUARD_RECORD_CAPACITY_MAX;
/// Absolute byte bound for one canonical durable Native AMX signing decision.
pub(crate) const MAX_NATIVE_AMX_SIGNING_GUARD_RECORD_BYTES_HARD: usize =
    iroha_config::parameters::defaults::sumeragi::V2_NATIVE_AMX_SIGNING_GUARD_RECORD_BYTES_MAX;
/// Absolute byte bound for the durable Native AMX signing chain anchor.
pub(crate) const MAX_NATIVE_AMX_SIGNING_GUARD_ANCHOR_BYTES_HARD: usize =
    iroha_config::parameters::defaults::sumeragi::V2_NATIVE_AMX_SIGNING_GUARD_ANCHOR_BYTES_MAX;
#[cfg(unix)]
const NATIVE_AMX_SIGNING_DIRECTORY_MODE: u32 = 0o700;
#[cfg(unix)]
const NATIVE_AMX_SIGNING_FILE_MODE: u32 = 0o600;
/// Hard protocol cap for a coordinator plus all native AMX participant legs.
pub(crate) const MAX_NATIVE_AMX_PLAN_LEGS: usize = 256;
/// Maximum participant legs after reserving one plan slot for the coordinator.
pub(crate) const MAX_NATIVE_AMX_PARTICIPANT_LEGS: usize = MAX_NATIVE_AMX_PLAN_LEGS - 1;
/// Hard protocol cap for one native AMX participant committee.
pub(crate) const MAX_NATIVE_AMX_VALIDATORS: usize = 128;
/// Hard protocol cap for sources sharing one participant-control commitment.
pub(crate) const MAX_NATIVE_AMX_PARTICIPANT_CONTROL_SOURCES: usize =
    crate::lane_consensus::MAX_LANE_EXECUTABLE_ENTRYPOINTS;
/// Canonical compressed BLS-normal signature/proof size.
pub(crate) const NATIVE_AMX_BLS_PROOF_BYTES: usize = 96;

/// Canonical application role of one Native AMX control leg.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NativeAmxParticipantApplicationRole {
    /// The leg is the exact coordinator proposal represented in participant
    /// form and must not create separate durable application evidence.
    Coordinator,
    /// The leg belongs to a distinct participant route and needs its own
    /// durable application evidence.
    SeparateParticipant,
}

/// Classify whether a Native AMX control leg needs its own durable participant
/// application evidence.
///
/// A coordinator-route leg certifies routing and settlement, but its economic
/// effects are already carried by the canonical global block. It must never
/// create a second participant receipt or WSV frontier marker. A same-route
/// leg is classified as the coordinator only when its incarnation, proposal
/// height, lane-local height/view, and proposal hash exactly match the
/// coordinator identity. Any identity drift is an error rather than a second
/// participant route.
pub(crate) fn native_amx_participant_application_role(
    receipt: &NativeAmxReceipt,
    leg: &NativeAmxLegRecordV2,
) -> Result<NativeAmxParticipantApplicationRole, &'static str> {
    let descriptor = &leg.participant_proposal.descriptor;
    let prepare = &leg.prepare_qc.body;
    let commit = &leg.commit_qc.body;
    let settlement_hash =
        iroha_data_model::nexus::compute_settlement_hash(&leg.participant_settlement)
            .map_err(|_| "Native AMX participant settlement cannot be hashed")?;
    if descriptor.lane_id != leg.lane_id
        || descriptor.dataspace_id != leg.dataspace_id
        || prepare.participant_lane_id != leg.lane_id
        || commit.participant_lane_id != leg.lane_id
        || prepare.participant_dataspace_id != leg.dataspace_id
        || commit.participant_dataspace_id != leg.dataspace_id
        || descriptor.lane_incarnation != prepare.participant_lane_incarnation
        || descriptor.lane_incarnation != commit.participant_lane_incarnation
        || descriptor.proposal_height != prepare.authority_context_height
        || descriptor.proposal_height != commit.authority_context_height
        || descriptor.previous_lane_block_height != prepare.participant_previous_block_height
        || descriptor.previous_lane_block_height != commit.participant_previous_block_height
        || descriptor.previous_lane_block_descriptor_hash
            != prepare.participant_previous_block_descriptor_hash
        || descriptor.previous_lane_block_descriptor_hash
            != commit.participant_previous_block_descriptor_hash
        || descriptor.lane_block_height != prepare.participant_lane_block_height
        || descriptor.lane_block_height != commit.participant_lane_block_height
        || descriptor.lane_block_view != prepare.participant_lane_block_view
        || descriptor.lane_block_view != commit.participant_lane_block_view
        || leg.participant_proposal.proposal_hash != prepare.participant_proposal_hash
        || leg.participant_proposal.proposal_hash != commit.participant_proposal_hash
        || settlement_hash != leg.participant_settlement_hash
        || leg.participant_settlement.lane_id != descriptor.lane_id
        || leg.participant_settlement.dataspace_id != descriptor.dataspace_id
        || leg.participant_settlement.lane_incarnation != descriptor.lane_incarnation
        || leg.participant_settlement.block_height != descriptor.lane_block_height
        || Hash::from(settlement_hash) != prepare.participant_settlement_commitment
        || Hash::from(settlement_hash) != commit.participant_settlement_commitment
        || prepare.coordinator_lane_id != receipt.lane_id
        || commit.coordinator_lane_id != receipt.lane_id
        || prepare.coordinator_dataspace_id != receipt.dataspace_id
        || commit.coordinator_dataspace_id != receipt.dataspace_id
        || prepare.coordinator_lane_incarnation != receipt.lane_incarnation
        || commit.coordinator_lane_incarnation != receipt.lane_incarnation
        || prepare.authority_context_height != receipt.authority_context_height
        || commit.authority_context_height != receipt.authority_context_height
        || prepare.planned_coordinator_block_height != receipt.lane_block_height
        || commit.planned_coordinator_block_height != receipt.lane_block_height
        || prepare.coordinator_lane_block_view != receipt.lane_block_view
        || commit.coordinator_lane_block_view != receipt.lane_block_view
        || prepare.coordinator_proposal_hash != receipt.coordinator_proposal_hash
        || commit.coordinator_proposal_hash != receipt.coordinator_proposal_hash
    {
        return Err("Native AMX participant leg identity is internally inconsistent");
    }

    let same_route =
        descriptor.lane_id == receipt.lane_id && descriptor.dataspace_id == receipt.dataspace_id;
    if !same_route {
        return Ok(NativeAmxParticipantApplicationRole::SeparateParticipant);
    }
    if descriptor.lane_incarnation != receipt.lane_incarnation
        || descriptor.proposal_height != receipt.authority_context_height
        || descriptor.lane_block_height != receipt.lane_block_height
        || descriptor.lane_block_view != receipt.lane_block_view
        || leg.participant_proposal.proposal_hash != receipt.coordinator_proposal_hash
    {
        return Err("Native AMX same-route leg differs from the coordinator identity");
    }
    Ok(NativeAmxParticipantApplicationRole::Coordinator)
}

/// Return whether one exact route incarnation needs separate participant
/// application evidence in this receipt.
///
/// Every leg is classified, even after a match, so malformed or same-route
/// identity-drift evidence always fails closed.
pub(crate) fn native_amx_receipt_requires_separate_participant_application_for(
    receipt: &NativeAmxReceipt,
    lane_id: LaneId,
    dataspace_id: DataSpaceId,
    lane_incarnation: Hash,
) -> Result<bool, &'static str> {
    let mut matches = false;
    for leg in &receipt.legs {
        match native_amx_participant_application_role(receipt, leg)? {
            NativeAmxParticipantApplicationRole::Coordinator => {}
            NativeAmxParticipantApplicationRole::SeparateParticipant => {
                let descriptor = &leg.participant_proposal.descriptor;
                matches |= descriptor.lane_id == lane_id
                    && descriptor.dataspace_id == dataspace_id
                    && descriptor.lane_incarnation == lane_incarnation;
            }
        }
    }
    Ok(matches)
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode)]
struct NativeAmxSigningKeyV2 {
    chain_id_hash: Hash,
    context_id: HeightContextId,
    round: ConsensusRound,
    epoch: u64,
    source_id: [u8; Hash::LENGTH],
    plan_digest: Hash,
    participant_lane_id: LaneId,
    participant_dataspace_id: DataSpaceId,
    phase: NativeAmxPhase,
    signer: PeerId,
}

impl NativeAmxSigningKeyV2 {
    fn from_body(body: &NativeAmxAttestationBodyV2, signer: &PeerId) -> Self {
        Self {
            chain_id_hash: body.chain_id_hash,
            context_id: body.round.context_id,
            round: body.round,
            epoch: body.epoch,
            source_id: body.source_id,
            plan_digest: body.plan_digest,
            participant_lane_id: body.participant_lane_id,
            participant_dataspace_id: body.participant_dataspace_id,
            phase: body.phase,
            signer: signer.clone(),
        }
    }
}

/// One signer-local participant lane slot, deliberately excluding the global
/// round view so a view-change replay cannot authorize a different proposal at
/// the same lane-local height/view (ABA equivocation).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode)]
struct NativeAmxSigningSlotV3 {
    chain_id_hash: Hash,
    context_id: HeightContextId,
    epoch: u64,
    authority_context_height: u64,
    participant_lane_id: LaneId,
    participant_dataspace_id: DataSpaceId,
    participant_lane_incarnation: Hash,
    participant_lane_block_height: u64,
    participant_lane_block_view: u64,
    phase: NativeAmxPhase,
    signer: PeerId,
}

impl NativeAmxSigningSlotV3 {
    fn from_body(body: &NativeAmxAttestationBodyV2, signer: &PeerId) -> Self {
        Self {
            chain_id_hash: body.chain_id_hash,
            context_id: body.round.context_id,
            epoch: body.epoch,
            authority_context_height: body.authority_context_height,
            participant_lane_id: body.participant_lane_id,
            participant_dataspace_id: body.participant_dataspace_id,
            participant_lane_incarnation: body.participant_lane_incarnation,
            participant_lane_block_height: body.participant_lane_block_height,
            participant_lane_block_view: body.participant_lane_block_view,
            phase: body.phase,
            signer: signer.clone(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
struct NativeAmxSigningSlotClaimV3 {
    participant_proposal_hash: Hash,
    participant_settlement_commitment: Hash,
}

impl NativeAmxSigningSlotClaimV3 {
    fn from_body(body: &NativeAmxAttestationBodyV2) -> Self {
        Self {
            participant_proposal_hash: body.participant_proposal_hash,
            participant_settlement_commitment: body.participant_settlement_commitment,
        }
    }
}

/// Durable source-session claim shared by every phase and participant leg.
///
/// Participant proposal and settlement identities deliberately remain in the
/// separate slot claim. This claim prevents a source from changing its typed
/// entrypoint, global round, routing plan, authority height, or coordinator
/// identity while still allowing the same grouped source to certify each
/// participant route exactly once.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
struct NativeAmxSourceSessionClaimV4 {
    source_id: [u8; Hash::LENGTH],
    tx_entrypoint_hash: HashOf<TransactionEntrypoint>,
    plan_digest: Hash,
    round: ConsensusRound,
    epoch: u64,
    chain_id_hash: Hash,
    authority_context_height: u64,
    coordinator_lane_id: LaneId,
    coordinator_dataspace_id: DataSpaceId,
    coordinator_lane_incarnation: Hash,
    planned_coordinator_block_height: u64,
    coordinator_lane_block_view: u64,
    coordinator_proposal_hash: Hash,
}

impl NativeAmxSourceSessionClaimV4 {
    fn from_body(body: &NativeAmxAttestationBodyV2) -> Self {
        Self {
            source_id: body.source_id,
            tx_entrypoint_hash: body.tx_entrypoint_hash,
            plan_digest: body.plan_digest,
            round: body.round,
            epoch: body.epoch,
            chain_id_hash: body.chain_id_hash,
            authority_context_height: body.authority_context_height,
            coordinator_lane_id: body.coordinator_lane_id,
            coordinator_dataspace_id: body.coordinator_dataspace_id,
            coordinator_lane_incarnation: body.coordinator_lane_incarnation,
            planned_coordinator_block_height: body.planned_coordinator_block_height,
            coordinator_lane_block_view: body.coordinator_lane_block_view,
            coordinator_proposal_hash: body.coordinator_proposal_hash,
        }
    }
}

/// Participant route/incarnation attached to one durable source-session claim.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode)]
struct NativeAmxSourceParticipantClaimV4 {
    lane_id: LaneId,
    dataspace_id: DataSpaceId,
    lane_incarnation: Hash,
}

impl NativeAmxSourceParticipantClaimV4 {
    fn from_body(body: &NativeAmxAttestationBodyV2) -> Self {
        Self {
            lane_id: body.participant_lane_id,
            dataspace_id: body.participant_dataspace_id,
            lane_incarnation: body.participant_lane_incarnation,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct NativeAmxDurableSourceClaimV4 {
    session: NativeAmxSourceSessionClaimV4,
    participants: BTreeMap<(LaneId, DataSpaceId), NativeAmxSourceParticipantClaimV4>,
}

impl NativeAmxDurableSourceClaimV4 {
    fn from_body(body: &NativeAmxAttestationBodyV2) -> Self {
        let participant = NativeAmxSourceParticipantClaimV4::from_body(body);
        Self {
            session: NativeAmxSourceSessionClaimV4::from_body(body),
            participants: std::iter::once((
                (participant.lane_id, participant.dataspace_id),
                participant,
            ))
            .collect(),
        }
    }

    fn accepts(&self, body: &NativeAmxAttestationBodyV2) -> bool {
        let participant = NativeAmxSourceParticipantClaimV4::from_body(body);
        self.session == NativeAmxSourceSessionClaimV4::from_body(body)
            && self
                .participants
                .get(&(participant.lane_id, participant.dataspace_id))
                .is_none_or(|claim| *claim == participant)
    }

    fn insert_participant(&mut self, body: &NativeAmxAttestationBodyV2) {
        let participant = NativeAmxSourceParticipantClaimV4::from_body(body);
        self.participants
            .entry((participant.lane_id, participant.dataspace_id))
            .or_insert(participant);
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
struct NativeAmxHeightBindingV2 {
    active_height: u64,
    context_id: HeightContextId,
    epoch: u64,
    chain_id_hash: Hash,
    signer: PeerId,
    max_records: u32,
}

impl NativeAmxHeightBindingV2 {
    fn genesis_head(&self) -> Result<Hash, NativeAmxSigningGuardError> {
        let encoded = norito::encode_canonical(self)
            .map_err(|error| NativeAmxSigningGuardError::UnsafeJournal(error.to_string()))?;
        Ok(Hash::new_from_chunks(&[
            NATIVE_AMX_SIGNING_GENESIS_DOMAIN,
            encoded.as_slice(),
        ]))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
struct NativeAmxSigningRecordV2 {
    version: u8,
    sequence: u32,
    previous_head: Hash,
    key: NativeAmxSigningKeyV2,
    body: NativeAmxAttestationBodyV2,
    body_digest: Hash,
    record_hash: Hash,
}

impl NativeAmxSigningRecordV2 {
    fn from_body(
        sequence: u32,
        previous_head: Hash,
        body: &NativeAmxAttestationBodyV2,
        signer: &PeerId,
    ) -> Result<Self, NativeAmxSigningGuardError> {
        let encoded_body = norito::encode_canonical(body)
            .map_err(|error| NativeAmxSigningGuardError::UnsafeJournal(error.to_string()))?;
        let mut record = Self {
            version: NATIVE_AMX_SIGNING_GUARD_VERSION,
            sequence,
            previous_head,
            key: NativeAmxSigningKeyV2::from_body(body, signer),
            body: *body,
            body_digest: Hash::new_from_chunks(&[
                NATIVE_AMX_SIGNING_BODY_DOMAIN,
                encoded_body.as_slice(),
            ]),
            record_hash: Hash::prehashed([0; Hash::LENGTH]),
        };
        record.record_hash = record.computed_record_hash()?;
        Ok(record)
    }

    fn computed_body_digest(&self) -> Result<Hash, NativeAmxSigningGuardError> {
        let encoded = norito::encode_canonical(&self.body)
            .map_err(|error| NativeAmxSigningGuardError::UnsafeJournal(error.to_string()))?;
        Ok(Hash::new_from_chunks(&[
            NATIVE_AMX_SIGNING_BODY_DOMAIN,
            encoded.as_slice(),
        ]))
    }

    fn computed_record_hash(&self) -> Result<Hash, NativeAmxSigningGuardError> {
        let mut hashable = self.clone();
        hashable.record_hash = Hash::prehashed([0; Hash::LENGTH]);
        let encoded = norito::encode_canonical(&hashable)
            .map_err(|error| NativeAmxSigningGuardError::UnsafeJournal(error.to_string()))?;
        Ok(Hash::new_from_chunks(&[
            NATIVE_AMX_SIGNING_RECORD_DOMAIN,
            encoded.as_slice(),
        ]))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
struct NativeAmxSigningAnchorV2 {
    version: u8,
    binding: NativeAmxHeightBindingV2,
    record_count: u32,
    head_hash: Hash,
    highest_view: Option<u64>,
}

impl NativeAmxSigningAnchorV2 {
    fn empty(binding: NativeAmxHeightBindingV2) -> Result<Self, NativeAmxSigningGuardError> {
        let head_hash = binding.genesis_head()?;
        Ok(Self {
            version: NATIVE_AMX_SIGNING_GUARD_VERSION,
            binding,
            record_count: 0,
            head_hash,
            highest_view: None,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct NativeAmxFileIdentity {
    device: u64,
    inode: u64,
}

#[derive(Debug)]
struct NativeAmxSigningGuardInner {
    anchor: NativeAmxSigningAnchorV2,
    anchor_identity: NativeAmxFileIdentity,
    records: BTreeMap<NativeAmxSigningKeyV2, NativeAmxSigningRecordV2>,
    record_identities: BTreeMap<NativeAmxSigningKeyV2, (PathBuf, NativeAmxFileIdentity)>,
    source_claims: BTreeMap<[u8; Hash::LENGTH], NativeAmxDurableSourceClaimV4>,
    slot_claims: BTreeMap<NativeAmxSigningSlotV3, NativeAmxSigningSlotClaimV3>,
    poisoned: Option<String>,
}

#[derive(Debug)]
struct LoadedNativeAmxJournal {
    records: BTreeMap<NativeAmxSigningKeyV2, NativeAmxSigningRecordV2>,
    source_claims: BTreeMap<[u8; Hash::LENGTH], NativeAmxDurableSourceClaimV4>,
    slot_claims: BTreeMap<NativeAmxSigningSlotV3, NativeAmxSigningSlotClaimV3>,
    anchored_paths: Vec<PathBuf>,
}

/// Failure to establish a durable Native AMX anti-equivocation decision.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub(crate) enum NativeAmxSigningGuardError {
    /// A malformed, non-canonical, oversized, or unsafe filesystem entry was found.
    #[error("native AMX signing guard is unsafe: {0}")]
    UnsafeJournal(String),
    /// A previous unsafe journal or I/O failure permanently poisoned this guard instance.
    #[error("native AMX signing guard is poisoned: {0}")]
    Poisoned(String),
    /// This platform cannot provide the required filesystem identity and permission checks.
    #[cfg(not(unix))]
    #[error("native AMX signing guard requires a Unix filesystem")]
    UnsupportedPlatform,
    /// The caller supplied a height below the durable active-height high-water.
    #[error(
        "native AMX signing height regressed from durable height {durable_height} to {supplied_height}"
    )]
    HeightRegression {
        /// Height supplied by canonical state on this open.
        supplied_height: u64,
        /// Highest height already persisted by the guard.
        durable_height: u64,
    },
    /// Heights may only stay fixed or advance by exactly one finalized height.
    #[error(
        "native AMX signing height jumped from durable height {durable_height} to {supplied_height}"
    )]
    HeightJump {
        /// Height supplied by canonical state on this open.
        supplied_height: u64,
        /// Height retained by the durable anchor.
        durable_height: u64,
    },
    /// The configured chain, signer, height context, epoch, or capacity changed unexpectedly.
    #[error("native AMX signing journal binding changed")]
    ContextMismatch,
    /// A record was found or requested above the supplied canonical active height.
    #[error(
        "native AMX signing record height {record_height} is ahead of active height {active_height}"
    )]
    FutureHeight {
        /// Height carried by the record or attempted body.
        record_height: u64,
        /// Height supplied by canonical state.
        active_height: u64,
    },
    /// A body below the active height must never be signed again after pruning.
    #[error(
        "native AMX signing record height {record_height} is below active height {active_height}"
    )]
    StaleHeight {
        /// Height carried by the attempted body.
        record_height: u64,
        /// Current durable active height.
        active_height: u64,
    },
    /// A vote below the durable view high-water must not be signed.
    #[error("native AMX signing view {attempted_view} is below durable view {highest_view}")]
    StaleView {
        /// View carried by the attempted body.
        attempted_view: u64,
        /// Highest view durably authorized at this height.
        highest_view: u64,
    },
    /// The same source transaction attempted to change its durable session claim.
    #[error("native AMX source transaction conflicts with its durable session claim")]
    PlanEquivocation,
    /// One lane-local signing slot attempted a different proposal or settlement.
    #[error("native AMX participant slot conflicts with its durable proposal/settlement claim")]
    SlotEquivocation,
    /// The exact signing key already authorizes a different full body.
    #[error("native AMX body conflicts with the durable signing decision")]
    Equivocation,
    /// The journal has reached its configured record bound.
    #[error("native AMX signing journal reached its record capacity")]
    Capacity,
    /// The attempted body or open parameters are structurally invalid.
    #[error("invalid native AMX signing guard input: {0}")]
    InvalidInput(String),
}

impl NativeAmxSigningGuardError {
    /// Return whether the signer must stop all consensus output until a verified reopen.
    #[must_use]
    pub(crate) const fn requires_restart_recovery(&self) -> bool {
        match self {
            Self::UnsafeJournal(_)
            | Self::Poisoned(_)
            | Self::HeightRegression { .. }
            | Self::HeightJump { .. }
            | Self::ContextMismatch
            | Self::FutureHeight { .. }
            | Self::StaleHeight { .. } => true,
            #[cfg(not(unix))]
            Self::UnsupportedPlatform => true,
            _ => false,
        }
    }
}

/// Validated runtime ceilings for one Native AMX signing journal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct NativeAmxSigningGuardLimits {
    pub(crate) max_records: NonZeroUsize,
    pub(crate) max_record_bytes: NonZeroUsize,
    pub(crate) max_anchor_bytes: NonZeroUsize,
}

impl NativeAmxSigningGuardLimits {
    /// Validate configured journal ceilings against fixed implementation maxima.
    pub(crate) fn new(
        max_records: NonZeroUsize,
        max_record_bytes: NonZeroUsize,
        max_anchor_bytes: NonZeroUsize,
    ) -> Result<Self, NativeAmxSigningGuardError> {
        if max_records.get() > MAX_NATIVE_AMX_SIGNING_GUARD_RECORDS_HARD
            || max_record_bytes.get() > MAX_NATIVE_AMX_SIGNING_GUARD_RECORD_BYTES_HARD
            || max_anchor_bytes.get() > MAX_NATIVE_AMX_SIGNING_GUARD_ANCHOR_BYTES_HARD
        {
            return Err(NativeAmxSigningGuardError::InvalidInput(
                "signing journal runtime ceiling exceeds its implementation maximum".to_owned(),
            ));
        }
        Ok(Self {
            max_records,
            max_record_bytes,
            max_anchor_bytes,
        })
    }
}

/// Crash-safe local anti-equivocation journal for Native AMX v2 votes.
///
/// Every record is appended to a hash chain. The updated chain anchor is also
/// atomically written and directory-fsynced before record returns. Callers must
/// invoke record immediately before creating the BLS signature.
///
/// The chain detects partial deletion, replacement, and rollback relative to
/// its retained anchor. Coordinated rollback of the complete signer directory,
/// including both every record and its anchor, is indistinguishable from an
/// older valid filesystem snapshot and requires external monotonic storage or
/// a TPM; that threat is outside a filesystem-only guard's guarantees.
#[derive(Debug)]
pub(crate) struct NativeAmxSigningGuard {
    directory: PathBuf,
    directory_handle: File,
    directory_identity: NativeAmxFileIdentity,
    owner_uid: u32,
    lock_path: PathBuf,
    owner_lock: File,
    lock_identity: NativeAmxFileIdentity,
    limits: NativeAmxSigningGuardLimits,
    inner: Mutex<NativeAmxSigningGuardInner>,
}

impl NativeAmxSigningGuard {
    /// Open one signer-specific journal at an exact frozen height context.
    ///
    /// The configured record bound must be non-zero and no greater than the
    /// protocol hard ceiling. An existing signer may reopen the same exact
    /// height context or advance by exactly one height.
    pub(crate) fn open(
        store_root: &Path,
        active_height: u64,
        context_id: HeightContextId,
        epoch: u64,
        chain_id_hash: Hash,
        signer: PeerId,
        limits: NativeAmxSigningGuardLimits,
    ) -> Result<Self, NativeAmxSigningGuardError> {
        #[cfg(not(unix))]
        {
            let _ = (
                store_root,
                active_height,
                context_id,
                epoch,
                chain_id_hash,
                signer,
                limits,
            );
            return Err(NativeAmxSigningGuardError::UnsupportedPlatform);
        }

        #[cfg(unix)]
        {
            Self::open_unix(
                store_root,
                active_height,
                context_id,
                epoch,
                chain_id_hash,
                signer,
                limits,
            )
        }
    }

    #[cfg(unix)]
    fn open_unix(
        store_root: &Path,
        active_height: u64,
        context_id: HeightContextId,
        epoch: u64,
        chain_id_hash: Hash,
        signer: PeerId,
        limits: NativeAmxSigningGuardLimits,
    ) -> Result<Self, NativeAmxSigningGuardError> {
        if active_height == 0
            || native_amx_hash_is_zero_sentinel(chain_id_hash.as_ref())
            || native_amx_hash_is_zero_sentinel(context_id.0.as_ref())
        {
            return Err(NativeAmxSigningGuardError::InvalidInput(
                "height, context, chain, or record capacity is invalid".to_owned(),
            ));
        }
        let max_records_u32 = u32::try_from(limits.max_records.get()).map_err(|_| {
            NativeAmxSigningGuardError::InvalidInput(
                "record capacity does not fit the durable format".to_owned(),
            )
        })?;
        let (directory, owner_uid) = native_amx_ensure_signer_directory(store_root, &signer)?;
        let (directory_handle, directory_identity) =
            native_amx_open_secure_directory(&directory, owner_uid)?;
        let (owner_lock, lock_path, lock_identity) =
            native_amx_acquire_owner_lock(&directory, &directory_handle, owner_uid)?;
        native_amx_verify_owned_directory(
            &directory,
            &directory_handle,
            directory_identity,
            owner_uid,
            &lock_path,
            &owner_lock,
            lock_identity,
        )?;
        native_amx_reconcile_guard_temps(
            &directory,
            &directory_handle,
            owner_uid,
            limits.max_record_bytes.get(),
            limits.max_anchor_bytes.get(),
        )?;

        let supplied_binding = NativeAmxHeightBindingV2 {
            active_height,
            context_id,
            epoch,
            chain_id_hash,
            signer: signer.clone(),
            max_records: max_records_u32,
        };
        let durable_anchor =
            Self::read_anchor(&directory, owner_uid, limits.max_anchor_bytes.get())?;
        let (anchor, records, source_claims, slot_claims) = match durable_anchor {
            None => {
                Self::ensure_empty_uninitialized_directory(&directory)?;
                let anchor = NativeAmxSigningAnchorV2::empty(supplied_binding)?;
                Self::persist_anchor(
                    &directory,
                    &directory_handle,
                    owner_uid,
                    &anchor,
                    limits.max_anchor_bytes.get(),
                )?;
                (anchor, BTreeMap::new(), BTreeMap::new(), BTreeMap::new())
            }
            Some(anchor) => {
                if anchor.binding.chain_id_hash != chain_id_hash
                    || anchor.binding.signer != signer
                    || anchor.binding.max_records != max_records_u32
                {
                    return Err(NativeAmxSigningGuardError::ContextMismatch);
                }
                let loaded = Self::load_validated_journal(
                    &directory,
                    &directory_handle,
                    owner_uid,
                    &anchor,
                    limits.max_records.get(),
                    limits.max_record_bytes.get(),
                )?;
                if active_height < anchor.binding.active_height {
                    return Err(NativeAmxSigningGuardError::HeightRegression {
                        supplied_height: active_height,
                        durable_height: anchor.binding.active_height,
                    });
                }
                if active_height == anchor.binding.active_height {
                    if context_id != anchor.binding.context_id
                        || epoch != anchor.binding.epoch
                        || max_records_u32 != anchor.binding.max_records
                    {
                        return Err(NativeAmxSigningGuardError::ContextMismatch);
                    }
                    (
                        anchor,
                        loaded.records,
                        loaded.source_claims,
                        loaded.slot_claims,
                    )
                } else {
                    let Some(next_height) = anchor.binding.active_height.checked_add(1) else {
                        return Err(NativeAmxSigningGuardError::HeightJump {
                            supplied_height: active_height,
                            durable_height: anchor.binding.active_height,
                        });
                    };
                    if active_height != next_height {
                        return Err(NativeAmxSigningGuardError::HeightJump {
                            supplied_height: active_height,
                            durable_height: anchor.binding.active_height,
                        });
                    }
                    if context_id == anchor.binding.context_id || epoch < anchor.binding.epoch {
                        return Err(NativeAmxSigningGuardError::ContextMismatch);
                    }
                    let next_anchor = NativeAmxSigningAnchorV2::empty(supplied_binding)?;
                    Self::persist_anchor(
                        &directory,
                        &directory_handle,
                        owner_uid,
                        &next_anchor,
                        limits.max_anchor_bytes.get(),
                    )?;
                    for path in loaded.anchored_paths {
                        fs::remove_file(&path)
                            .map_err(|error| native_amx_unsafe_journal(&path, error.to_string()))?;
                    }
                    native_amx_sync_directory_handle(&directory, &directory_handle)?;
                    (
                        next_anchor,
                        BTreeMap::new(),
                        BTreeMap::new(),
                        BTreeMap::new(),
                    )
                }
            }
        };
        native_amx_verify_owned_directory(
            &directory,
            &directory_handle,
            directory_identity,
            owner_uid,
            &lock_path,
            &owner_lock,
            lock_identity,
        )?;
        let anchor_identity = native_amx_secure_file_identity(
            &Self::anchor_path(&directory),
            limits.max_anchor_bytes.get(),
            owner_uid,
        )?;
        let record_identities = records
            .iter()
            .map(|(key, record)| {
                let path = Self::record_path(&directory, record);
                native_amx_secure_file_identity(&path, limits.max_record_bytes.get(), owner_uid)
                    .map(|identity| (key.clone(), (path, identity)))
            })
            .collect::<Result<BTreeMap<_, _>, _>>()?;
        Ok(Self {
            directory,
            directory_handle,
            directory_identity,
            owner_uid,
            lock_path,
            owner_lock,
            lock_identity,
            limits,
            inner: Mutex::new(NativeAmxSigningGuardInner {
                anchor,
                anchor_identity,
                records,
                record_identities,
                source_claims,
                slot_claims,
                poisoned: None,
            }),
        })
    }

    fn anchor_path(directory: &Path) -> PathBuf {
        directory.join(NATIVE_AMX_SIGNING_GUARD_ANCHOR_FILE)
    }

    fn anchor_temp_path(directory: &Path) -> PathBuf {
        directory.join(NATIVE_AMX_SIGNING_GUARD_ANCHOR_TEMP)
    }

    fn record_filename(record: &NativeAmxSigningRecordV2) -> String {
        format!(
            "{:020}.{:010}.{}.{}",
            record.body.authority_context_height,
            record.sequence,
            record.record_hash,
            NATIVE_AMX_SIGNING_GUARD_RECORD_EXTENSION
        )
    }

    fn record_path(directory: &Path, record: &NativeAmxSigningRecordV2) -> PathBuf {
        directory.join(Self::record_filename(record))
    }

    fn read_anchor(
        directory: &Path,
        owner_uid: u32,
        max_anchor_bytes: usize,
    ) -> Result<Option<NativeAmxSigningAnchorV2>, NativeAmxSigningGuardError> {
        let path = Self::anchor_path(directory);
        match fs::symlink_metadata(&path) {
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(native_amx_unsafe_journal(&path, error.to_string())),
        }
        let bytes = native_amx_read_secure_file(&path, max_anchor_bytes, owner_uid)?;
        let anchor = norito::decode_canonical::<NativeAmxSigningAnchorV2>(&bytes)
            .map_err(|error| native_amx_unsafe_journal(&path, error.to_string()))?;
        if anchor.version != NATIVE_AMX_SIGNING_GUARD_VERSION
            || anchor.binding.active_height == 0
            || native_amx_hash_is_zero_sentinel(anchor.binding.context_id.0.as_ref())
            || native_amx_hash_is_zero_sentinel(anchor.binding.chain_id_hash.as_ref())
            || anchor.binding.max_records == 0
            || usize::try_from(anchor.binding.max_records)
                .map_or(true, |max| max > MAX_NATIVE_AMX_SIGNING_GUARD_RECORDS_HARD)
            || anchor.record_count > anchor.binding.max_records
        {
            return Err(native_amx_unsafe_journal(
                &path,
                "unsupported, non-canonical, or invalid chain anchor",
            ));
        }
        Ok(Some(anchor))
    }

    fn read_record(
        directory: &Path,
        path: &Path,
        owner_uid: u32,
        max_record_bytes: usize,
    ) -> Result<NativeAmxSigningRecordV2, NativeAmxSigningGuardError> {
        let bytes = native_amx_read_secure_file(path, max_record_bytes, owner_uid)?;
        let record = norito::decode_canonical::<NativeAmxSigningRecordV2>(&bytes)
            .map_err(|error| native_amx_unsafe_journal(path, error.to_string()))?;
        let expected_key = NativeAmxSigningKeyV2::from_body(&record.body, &record.key.signer);
        if record.version != NATIVE_AMX_SIGNING_GUARD_VERSION
            || record.sequence == 0
            || !native_amx_body_shape_valid(&record.body)
            || record.key != expected_key
            || record.body_digest != record.computed_body_digest()?
            || record.record_hash != record.computed_record_hash()?
            || Self::record_path(directory, &record) != path
        {
            return Err(native_amx_unsafe_journal(
                path,
                "unsupported, non-canonical, misnamed, or corrupt signing record",
            ));
        }
        Ok(record)
    }

    fn validate_record_binding(
        path: &Path,
        record: &NativeAmxSigningRecordV2,
        anchor: &NativeAmxSigningAnchorV2,
    ) -> Result<(), NativeAmxSigningGuardError> {
        let binding = &anchor.binding;
        if record.body.authority_context_height != binding.active_height
            || record.body.round.height != binding.active_height
            || record.body.round.context_id != binding.context_id
            || record.body.epoch != binding.epoch
            || record.body.chain_id_hash != binding.chain_id_hash
            || record.key.signer != binding.signer
        {
            return Err(native_amx_unsafe_journal(
                path,
                "record does not match the anchored height context",
            ));
        }
        Ok(())
    }

    fn load_validated_journal(
        directory: &Path,
        directory_handle: &File,
        owner_uid: u32,
        anchor: &NativeAmxSigningAnchorV2,
        max_records: usize,
        max_record_bytes: usize,
    ) -> Result<LoadedNativeAmxJournal, NativeAmxSigningGuardError> {
        let mut current = BTreeMap::<u32, (NativeAmxSigningRecordV2, PathBuf)>::new();
        let mut stale_paths = Vec::new();
        let mut final_record_count = 0_usize;
        for item in fs::read_dir(directory)
            .map_err(|error| native_amx_unsafe_journal(directory, error.to_string()))?
        {
            let item =
                item.map_err(|error| native_amx_unsafe_journal(directory, error.to_string()))?;
            let path = item.path();
            let name = item.file_name();
            let name = name.to_string_lossy();
            if name == NATIVE_AMX_SIGNING_GUARD_LOCK_FILE
                || name == NATIVE_AMX_SIGNING_GUARD_ANCHOR_FILE
            {
                continue;
            }
            if !native_amx_valid_record_filename(&name) {
                return Err(native_amx_unsafe_journal(&path, "unknown journal file"));
            }
            final_record_count = final_record_count.saturating_add(1);
            if final_record_count > max_records {
                return Err(native_amx_unsafe_journal(
                    directory,
                    "record count exceeds the configured runtime limit",
                ));
            }
            let record = Self::read_record(directory, &path, owner_uid, max_record_bytes)?;
            let record_height = record.body.authority_context_height;
            if record_height > anchor.binding.active_height {
                return Err(NativeAmxSigningGuardError::FutureHeight {
                    record_height,
                    active_height: anchor.binding.active_height,
                });
            }
            if record_height < anchor.binding.active_height {
                if record_height.checked_add(1) != Some(anchor.binding.active_height) {
                    return Err(native_amx_unsafe_journal(
                        &path,
                        "record is more than one height behind the anchor",
                    ));
                }
                stale_paths.push(path);
                continue;
            }
            if current
                .insert(record.sequence, (record, path.clone()))
                .is_some()
            {
                return Err(native_amx_unsafe_journal(
                    &path,
                    "duplicate record sequence",
                ));
            }
        }

        let expected_count = usize::try_from(anchor.record_count)
            .map_err(|_| native_amx_unsafe_journal(directory, "record count overflow"))?;
        let mut head = anchor.binding.genesis_head()?;
        let mut records = BTreeMap::new();
        let mut source_claims = BTreeMap::new();
        let mut slot_claims = BTreeMap::new();
        let mut anchored_paths = Vec::with_capacity(expected_count);
        let mut highest_view = None::<u64>;
        for sequence in 1..=anchor.record_count {
            let Some((record, path)) = current.remove(&sequence) else {
                return Err(native_amx_unsafe_journal(
                    directory,
                    format!("anchored record sequence {sequence} is missing"),
                ));
            };
            Self::validate_record_binding(&path, &record, anchor)?;
            if record.previous_head != head {
                return Err(native_amx_unsafe_journal(
                    &path,
                    "record chain predecessor mismatch",
                ));
            }
            match source_claims.entry(record.body.source_id) {
                std::collections::btree_map::Entry::Vacant(entry) => {
                    entry.insert(NativeAmxDurableSourceClaimV4::from_body(&record.body));
                }
                std::collections::btree_map::Entry::Occupied(mut entry) => {
                    if !entry.get().accepts(&record.body) {
                        return Err(native_amx_unsafe_journal(
                            &path,
                            "one source has conflicting anchored session claims",
                        ));
                    }
                    entry.get_mut().insert_participant(&record.body);
                }
            }
            let slot = NativeAmxSigningSlotV3::from_body(&record.body, &record.key.signer);
            let slot_claim = NativeAmxSigningSlotClaimV3::from_body(&record.body);
            if slot_claims
                .insert(slot, slot_claim)
                .is_some_and(|claimed| claimed != slot_claim)
            {
                return Err(native_amx_unsafe_journal(
                    &path,
                    "one participant slot has conflicting anchored proposal/settlement claims",
                ));
            }
            if highest_view.is_some_and(|view| record.body.round.view < view) {
                return Err(native_amx_unsafe_journal(
                    &path,
                    "record chain regresses its durable view high-water",
                ));
            }
            highest_view = Some(highest_view.map_or(record.body.round.view, |view| {
                view.max(record.body.round.view)
            }));
            head = record.record_hash;
            if records.insert(record.key.clone(), record).is_some() {
                return Err(native_amx_unsafe_journal(
                    &path,
                    "duplicate anchored signing key",
                ));
            }
            anchored_paths.push(path);
        }
        if records.len() != expected_count
            || head != anchor.head_hash
            || highest_view != anchor.highest_view
        {
            return Err(native_amx_unsafe_journal(
                directory,
                "chain anchor count, head, or highest view mismatch",
            ));
        }

        let mut cleanup_paths = stale_paths;
        if !current.is_empty() {
            let tail_sequence = anchor
                .record_count
                .checked_add(1)
                .ok_or_else(|| native_amx_unsafe_journal(directory, "tail sequence overflow"))?;
            if current.len() != 1 || !current.contains_key(&tail_sequence) {
                return Err(native_amx_unsafe_journal(
                    directory,
                    "more than one unpublished tail record",
                ));
            }
            let (tail, tail_path) = current
                .remove(&tail_sequence)
                .expect("tail membership checked");
            Self::validate_record_binding(&tail_path, &tail, anchor)?;
            let tail_slot = NativeAmxSigningSlotV3::from_body(&tail.body, &tail.key.signer);
            let tail_slot_claim = NativeAmxSigningSlotClaimV3::from_body(&tail.body);
            if tail.previous_head != anchor.head_hash
                || anchor.record_count >= anchor.binding.max_records
                || anchor
                    .highest_view
                    .is_some_and(|view| tail.body.round.view < view)
                || records.contains_key(&tail.key)
                || source_claims
                    .get(&tail.body.source_id)
                    .is_some_and(|claim| !claim.accepts(&tail.body))
                || slot_claims
                    .get(&tail_slot)
                    .is_some_and(|claim| *claim != tail_slot_claim)
            {
                return Err(native_amx_unsafe_journal(
                    &tail_path,
                    "invalid unpublished tail record",
                ));
            }
            cleanup_paths.push(tail_path);
        }
        if !cleanup_paths.is_empty() {
            for path in cleanup_paths {
                fs::remove_file(&path)
                    .map_err(|error| native_amx_unsafe_journal(&path, error.to_string()))?;
            }
            native_amx_sync_directory_handle(directory, directory_handle)?;
        }
        Ok(LoadedNativeAmxJournal {
            records,
            source_claims,
            slot_claims,
            anchored_paths,
        })
    }

    fn ensure_empty_uninitialized_directory(
        directory: &Path,
    ) -> Result<(), NativeAmxSigningGuardError> {
        for item in fs::read_dir(directory)
            .map_err(|error| native_amx_unsafe_journal(directory, error.to_string()))?
        {
            let item =
                item.map_err(|error| native_amx_unsafe_journal(directory, error.to_string()))?;
            if item.file_name() != NATIVE_AMX_SIGNING_GUARD_LOCK_FILE {
                return Err(native_amx_unsafe_journal(
                    &item.path(),
                    "journal content exists without a chain anchor",
                ));
            }
        }
        Ok(())
    }

    fn persist_anchor(
        directory: &Path,
        directory_handle: &File,
        owner_uid: u32,
        anchor: &NativeAmxSigningAnchorV2,
        max_anchor_bytes: usize,
    ) -> Result<(), NativeAmxSigningGuardError> {
        let bytes = norito::encode_canonical(anchor)
            .map_err(|error| native_amx_unsafe_journal(directory, error.to_string()))?;
        if bytes.len() > max_anchor_bytes {
            return Err(native_amx_unsafe_journal(
                directory,
                "chain anchor exceeds its configured runtime byte limit",
            ));
        }
        let temp = Self::anchor_temp_path(directory);
        native_amx_write_new_secure_temp(&temp, &bytes, max_anchor_bytes, owner_uid)?;
        let path = Self::anchor_path(directory);
        fs::rename(&temp, &path)
            .map_err(|error| native_amx_unsafe_journal(&path, error.to_string()))?;
        native_amx_sync_directory_handle(directory, directory_handle)?;
        let persisted =
            Self::read_anchor(directory, owner_uid, max_anchor_bytes)?.ok_or_else(|| {
                native_amx_unsafe_journal(&path, "anchor disappeared after publication")
            })?;
        if &persisted != anchor {
            return Err(native_amx_unsafe_journal(
                &path,
                "anchor changed during publication",
            ));
        }
        Ok(())
    }

    fn verify_retained_journal_paths(
        &self,
        inner: &NativeAmxSigningGuardInner,
    ) -> Result<(), NativeAmxSigningGuardError> {
        let anchor_path = Self::anchor_path(&self.directory);
        let anchor_identity = native_amx_secure_file_identity(
            &anchor_path,
            self.limits.max_anchor_bytes.get(),
            self.owner_uid,
        )?;
        if anchor_identity != inner.anchor_identity {
            return Err(native_amx_unsafe_journal(
                &anchor_path,
                "retained anchor path was replaced",
            ));
        }
        if inner.record_identities.len() != inner.records.len() {
            return Err(native_amx_unsafe_journal(
                &self.directory,
                "retained record identity set is incomplete",
            ));
        }
        for (key, (path, retained_identity)) in &inner.record_identities {
            let Some(record) = inner.records.get(key) else {
                return Err(native_amx_unsafe_journal(
                    path,
                    "retained record identity has no signing decision",
                ));
            };
            if Self::record_path(&self.directory, record) != *path {
                return Err(native_amx_unsafe_journal(
                    path,
                    "retained record path is non-canonical",
                ));
            }
            let linked_identity = native_amx_secure_file_identity(
                path,
                self.limits.max_record_bytes.get(),
                self.owner_uid,
            )?;
            if linked_identity != *retained_identity {
                return Err(native_amx_unsafe_journal(
                    path,
                    "retained record path was replaced",
                ));
            }
        }
        Ok(())
    }

    fn record_locked(
        &self,
        inner: &mut NativeAmxSigningGuardInner,
        body: &NativeAmxAttestationBodyV2,
    ) -> Result<(), NativeAmxSigningGuardError> {
        native_amx_verify_owned_directory(
            &self.directory,
            &self.directory_handle,
            self.directory_identity,
            self.owner_uid,
            &self.lock_path,
            &self.owner_lock,
            self.lock_identity,
        )?;
        self.verify_retained_journal_paths(inner)?;
        if !native_amx_body_shape_valid(body) {
            return Err(NativeAmxSigningGuardError::InvalidInput(
                "malformed attestation body".to_owned(),
            ));
        }
        let binding = &inner.anchor.binding;
        if body.chain_id_hash != binding.chain_id_hash
            || body.round.context_id != binding.context_id
            || body.epoch != binding.epoch
        {
            return Err(NativeAmxSigningGuardError::ContextMismatch);
        }
        if body.authority_context_height > binding.active_height {
            return Err(NativeAmxSigningGuardError::FutureHeight {
                record_height: body.authority_context_height,
                active_height: binding.active_height,
            });
        }
        if body.authority_context_height < binding.active_height {
            return Err(NativeAmxSigningGuardError::StaleHeight {
                record_height: body.authority_context_height,
                active_height: binding.active_height,
            });
        }
        let key = NativeAmxSigningKeyV2::from_body(body, &binding.signer);
        if let Some(existing) = inner.records.get(&key) {
            return if existing.body == *body {
                Ok(())
            } else {
                Err(NativeAmxSigningGuardError::Equivocation)
            };
        }
        let slot = NativeAmxSigningSlotV3::from_body(body, &binding.signer);
        let slot_claim = NativeAmxSigningSlotClaimV3::from_body(body);
        if inner
            .slot_claims
            .get(&slot)
            .is_some_and(|claim| *claim != slot_claim)
        {
            return Err(NativeAmxSigningGuardError::SlotEquivocation);
        }
        if inner
            .source_claims
            .get(&body.source_id)
            .is_some_and(|claim| !claim.accepts(body))
        {
            return Err(NativeAmxSigningGuardError::PlanEquivocation);
        }
        if inner
            .anchor
            .highest_view
            .is_some_and(|highest| body.round.view < highest)
        {
            return Err(NativeAmxSigningGuardError::StaleView {
                attempted_view: body.round.view,
                highest_view: inner.anchor.highest_view.expect("checked"),
            });
        }
        if inner.records.len() >= self.limits.max_records.get() {
            return Err(NativeAmxSigningGuardError::Capacity);
        }
        let sequence = inner.anchor.record_count.checked_add(1).ok_or_else(|| {
            native_amx_unsafe_journal(&self.directory, "record sequence overflow")
        })?;
        let record = NativeAmxSigningRecordV2::from_body(
            sequence,
            inner.anchor.head_hash,
            body,
            &binding.signer,
        )?;
        let bytes = norito::encode_canonical(&record)
            .map_err(|error| native_amx_unsafe_journal(&self.directory, error.to_string()))?;
        if bytes.len() > self.limits.max_record_bytes.get() {
            return Err(native_amx_unsafe_journal(
                &self.directory,
                "record exceeds its configured runtime byte limit",
            ));
        }
        let path = Self::record_path(&self.directory, &record);
        let temp = path.with_extension(NATIVE_AMX_SIGNING_GUARD_TEMP_EXTENSION);
        native_amx_write_new_secure_temp(
            &temp,
            &bytes,
            self.limits.max_record_bytes.get(),
            self.owner_uid,
        )?;
        match fs::symlink_metadata(&path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Ok(_) => {
                return Err(native_amx_unsafe_journal(
                    &path,
                    "next record path already exists",
                ));
            }
            Err(error) => return Err(native_amx_unsafe_journal(&path, error.to_string())),
        }
        fs::rename(&temp, &path)
            .map_err(|error| native_amx_unsafe_journal(&path, error.to_string()))?;
        native_amx_sync_directory_handle(&self.directory, &self.directory_handle)?;
        native_amx_verify_owned_directory(
            &self.directory,
            &self.directory_handle,
            self.directory_identity,
            self.owner_uid,
            &self.lock_path,
            &self.owner_lock,
            self.lock_identity,
        )?;
        let persisted = Self::read_record(
            &self.directory,
            &path,
            self.owner_uid,
            self.limits.max_record_bytes.get(),
        )?;
        if persisted != record {
            return Err(native_amx_unsafe_journal(
                &path,
                "record changed during publication",
            ));
        }
        let record_identity = native_amx_secure_file_identity(
            &path,
            self.limits.max_record_bytes.get(),
            self.owner_uid,
        )?;

        let mut next_anchor = inner.anchor.clone();
        next_anchor.record_count = sequence;
        next_anchor.head_hash = record.record_hash;
        next_anchor.highest_view = Some(
            next_anchor
                .highest_view
                .map_or(body.round.view, |view| view.max(body.round.view)),
        );
        Self::persist_anchor(
            &self.directory,
            &self.directory_handle,
            self.owner_uid,
            &next_anchor,
            self.limits.max_anchor_bytes.get(),
        )?;
        let anchor_identity = native_amx_secure_file_identity(
            &Self::anchor_path(&self.directory),
            self.limits.max_anchor_bytes.get(),
            self.owner_uid,
        )?;
        native_amx_verify_owned_directory(
            &self.directory,
            &self.directory_handle,
            self.directory_identity,
            self.owner_uid,
            &self.lock_path,
            &self.owner_lock,
            self.lock_identity,
        )?;

        inner.anchor = next_anchor;
        inner.anchor_identity = anchor_identity;
        inner
            .record_identities
            .insert(key.clone(), (path, record_identity));
        inner.records.insert(key, record);
        inner
            .source_claims
            .entry(body.source_id)
            .and_modify(|claim| claim.insert_participant(body))
            .or_insert_with(|| NativeAmxDurableSourceClaimV4::from_body(body));
        inner.slot_claims.entry(slot).or_insert(slot_claim);
        Ok(())
    }

    /// Durably authorize the exact full body before BLS signature creation.
    ///
    /// Exact replay at the current view is idempotent. A changed source session,
    /// a conflicting body for one key, or a stale view is refused.
    /// Unsafe journal and I/O failures permanently poison this guard instance.
    pub(crate) fn record(
        &self,
        body: &NativeAmxAttestationBodyV2,
    ) -> Result<(), NativeAmxSigningGuardError> {
        let mut inner = self.inner.lock();
        if let Some(reason) = inner.poisoned.as_ref() {
            return Err(NativeAmxSigningGuardError::Poisoned(reason.clone()));
        }
        let result = self.record_locked(&mut inner, body);
        if let Err(NativeAmxSigningGuardError::UnsafeJournal(message)) = &result {
            inner.poisoned = Some(message.clone());
        }
        result
    }

    #[cfg(test)]
    pub(crate) fn record_count_for_test(&self) -> u32 {
        self.inner.lock().anchor.record_count
    }

    #[cfg(test)]
    pub(crate) const fn max_records_for_test(&self) -> usize {
        self.limits.max_records.get()
    }

    #[cfg(test)]
    pub(crate) fn remove_one_record_for_test(&self) {
        let path = self
            .inner
            .lock()
            .record_identities
            .values()
            .next()
            .map(|(path, _)| path.clone())
            .expect("test signing guard has a retained record");
        std::fs::remove_file(path).expect("remove one retained signing record for test");
    }
}

fn native_amx_ensure_signer_directory(
    store_root: &Path,
    signer: &PeerId,
) -> Result<(PathBuf, u32), NativeAmxSigningGuardError> {
    #[cfg(not(unix))]
    {
        let _ = (store_root, signer);
        return Err(NativeAmxSigningGuardError::UnsupportedPlatform);
    }

    #[cfg(unix)]
    {
        let root_metadata = fs::symlink_metadata(store_root)
            .map_err(|error| native_amx_unsafe_journal(store_root, error.to_string()))?;
        if root_metadata.file_type().is_symlink() || !root_metadata.file_type().is_dir() {
            return Err(native_amx_unsafe_journal(
                store_root,
                "store root must be a regular directory",
            ));
        }
        let owner_uid = native_amx_effective_user_id(store_root)?;
        native_amx_validate_uid(store_root, &root_metadata, owner_uid)?;
        let signer_digest = native_amx_signer_directory_digest(store_root, signer)?;
        native_amx_reject_legacy_signer_journals(store_root, signer_digest, owner_uid)?;
        let guard_root = store_root.join(NATIVE_AMX_SIGNING_GUARD_DIRECTORY);
        let guard_root_created = native_amx_ensure_secure_directory(&guard_root, owner_uid)?;
        if guard_root_created {
            native_amx_sync_directory_path(store_root)?;
        }
        let directory = guard_root.join(signer_digest.to_string());
        let signer_created = native_amx_ensure_secure_directory(&directory, owner_uid)?;
        if signer_created {
            native_amx_sync_directory_path(&guard_root)?;
        }
        Ok((directory, owner_uid))
    }
}

#[cfg(unix)]
fn native_amx_signer_directory_digest(
    store_root: &Path,
    signer: &PeerId,
) -> Result<Hash, NativeAmxSigningGuardError> {
    let signer_bytes = norito::encode_canonical(signer)
        .map_err(|error| native_amx_unsafe_journal(store_root, error.to_string()))?;
    Ok(Hash::new_from_chunks(&[
        NATIVE_AMX_SIGNER_DIRECTORY_DOMAIN,
        signer_bytes.as_slice(),
    ]))
}

#[cfg(unix)]
fn native_amx_reject_legacy_signer_journals(
    store_root: &Path,
    signer_digest: Hash,
    owner_uid: u32,
) -> Result<(), NativeAmxSigningGuardError> {
    for legacy_name in NATIVE_AMX_LEGACY_SIGNING_GUARD_DIRECTORIES {
        let legacy_root = store_root.join(legacy_name);
        let legacy_root_metadata = match fs::symlink_metadata(&legacy_root) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => {
                return Err(native_amx_unsafe_journal(&legacy_root, error.to_string()));
            }
        };
        native_amx_validate_secure_directory_metadata(
            &legacy_root,
            &legacy_root_metadata,
            owner_uid,
        )?;
        let legacy_signer = legacy_root.join(signer_digest.to_string());
        match fs::symlink_metadata(&legacy_signer) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(native_amx_unsafe_journal(&legacy_signer, error.to_string()));
            }
            Ok(metadata) => {
                native_amx_validate_secure_directory_metadata(
                    &legacy_signer,
                    &metadata,
                    owner_uid,
                )?;
                return Err(native_amx_unsafe_journal(
                    &legacy_signer,
                    "legacy Native AMX signing evidence requires authenticated recovery; it must not be silently ignored",
                ));
            }
        }
    }
    Ok(())
}

#[cfg(unix)]
fn native_amx_ensure_secure_directory(
    path: &Path,
    owner_uid: u32,
) -> Result<bool, NativeAmxSigningGuardError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            native_amx_validate_secure_directory_metadata(path, &metadata, owner_uid)?;
            return Ok(false);
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(native_amx_unsafe_journal(path, error.to_string())),
    }
    let mut builder = DirBuilder::new();
    builder.mode(NATIVE_AMX_SIGNING_DIRECTORY_MODE);
    builder
        .create(path)
        .map_err(|error| native_amx_unsafe_journal(path, error.to_string()))?;
    fs::set_permissions(
        path,
        fs::Permissions::from_mode(NATIVE_AMX_SIGNING_DIRECTORY_MODE),
    )
    .map_err(|error| native_amx_unsafe_journal(path, error.to_string()))?;
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| native_amx_unsafe_journal(path, error.to_string()))?;
    native_amx_validate_secure_directory_metadata(path, &metadata, owner_uid)?;
    let (handle, _) = native_amx_open_secure_directory(path, owner_uid)?;
    native_amx_sync_directory_handle(path, &handle)?;
    Ok(true)
}

#[cfg(unix)]
fn native_amx_validate_secure_directory_metadata(
    path: &Path,
    metadata: &fs::Metadata,
    owner_uid: u32,
) -> Result<(), NativeAmxSigningGuardError> {
    native_amx_validate_uid(path, metadata, owner_uid)?;
    if metadata.file_type().is_symlink()
        || !metadata.file_type().is_dir()
        || metadata.mode() & 0o777 != NATIVE_AMX_SIGNING_DIRECTORY_MODE
    {
        return Err(native_amx_unsafe_journal(
            path,
            "directory must be regular and mode 0700",
        ));
    }
    Ok(())
}

#[cfg(unix)]
fn native_amx_open_secure_directory(
    path: &Path,
    owner_uid: u32,
) -> Result<(File, NativeAmxFileIdentity), NativeAmxSigningGuardError> {
    let before = fs::symlink_metadata(path)
        .map_err(|error| native_amx_unsafe_journal(path, error.to_string()))?;
    native_amx_validate_secure_directory_metadata(path, &before, owner_uid)?;
    let mut options = OpenOptions::new();
    options.read(true);
    native_amx_set_no_follow_flag(&mut options);
    let handle = options
        .open(path)
        .map_err(|error| native_amx_unsafe_journal(path, error.to_string()))?;
    let opened = handle
        .metadata()
        .map_err(|error| native_amx_unsafe_journal(path, error.to_string()))?;
    native_amx_validate_secure_directory_metadata(path, &opened, owner_uid)?;
    let before_identity = native_amx_file_identity(&before);
    let opened_identity = native_amx_file_identity(&opened);
    if before_identity != opened_identity {
        return Err(native_amx_unsafe_journal(
            path,
            "directory changed between inspection and open",
        ));
    }
    let after = fs::symlink_metadata(path)
        .map_err(|error| native_amx_unsafe_journal(path, error.to_string()))?;
    native_amx_validate_secure_directory_metadata(path, &after, owner_uid)?;
    if native_amx_file_identity(&after) != opened_identity {
        return Err(native_amx_unsafe_journal(
            path,
            "directory path changed while opening",
        ));
    }
    Ok((handle, opened_identity))
}

#[cfg(unix)]
fn native_amx_acquire_owner_lock(
    directory: &Path,
    directory_handle: &File,
    owner_uid: u32,
) -> Result<(File, PathBuf, NativeAmxFileIdentity), NativeAmxSigningGuardError> {
    let path = directory.join(NATIVE_AMX_SIGNING_GUARD_LOCK_FILE);
    let before = match fs::symlink_metadata(&path) {
        Ok(metadata) => {
            native_amx_validate_secure_file_metadata(&path, &metadata, 0, owner_uid)?;
            Some(metadata)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(native_amx_unsafe_journal(&path, error.to_string())),
    };
    let mut options = OpenOptions::new();
    options
        .read(true)
        .write(true)
        .create(true)
        .mode(NATIVE_AMX_SIGNING_FILE_MODE);
    native_amx_set_no_follow_flag(&mut options);
    let file = options
        .open(&path)
        .map_err(|error| native_amx_unsafe_journal(&path, error.to_string()))?;
    if before.is_none() {
        file.set_permissions(fs::Permissions::from_mode(NATIVE_AMX_SIGNING_FILE_MODE))
            .map_err(|error| native_amx_unsafe_journal(&path, error.to_string()))?;
    }
    let opened = file
        .metadata()
        .map_err(|error| native_amx_unsafe_journal(&path, error.to_string()))?;
    native_amx_validate_secure_file_metadata(&path, &opened, 0, owner_uid)?;
    let opened_identity = native_amx_file_identity(&opened);
    if before
        .as_ref()
        .is_some_and(|metadata| native_amx_file_identity(metadata) != opened_identity)
    {
        return Err(native_amx_unsafe_journal(
            &path,
            "owner lock changed between inspection and open",
        ));
    }
    let after = fs::symlink_metadata(&path)
        .map_err(|error| native_amx_unsafe_journal(&path, error.to_string()))?;
    native_amx_validate_secure_file_metadata(&path, &after, 0, owner_uid)?;
    if native_amx_file_identity(&after) != opened_identity {
        return Err(native_amx_unsafe_journal(
            &path,
            "owner lock path changed while opening",
        ));
    }
    match file.try_lock() {
        Ok(()) => {}
        Err(fs::TryLockError::WouldBlock) => {
            return Err(native_amx_unsafe_journal(
                &path,
                "signer journal is already owned by another process",
            ));
        }
        Err(fs::TryLockError::Error(error)) => {
            return Err(native_amx_unsafe_journal(&path, error.to_string()));
        }
    }
    let locked = fs::symlink_metadata(&path)
        .map_err(|error| native_amx_unsafe_journal(&path, error.to_string()))?;
    native_amx_validate_secure_file_metadata(&path, &locked, 0, owner_uid)?;
    if native_amx_file_identity(&locked) != opened_identity {
        return Err(native_amx_unsafe_journal(
            &path,
            "owner lock path changed while locking",
        ));
    }
    if before.is_none() {
        file.sync_all()
            .map_err(|error| native_amx_unsafe_journal(&path, error.to_string()))?;
        native_amx_sync_directory_handle(directory, directory_handle)?;
    }
    Ok((file, path, opened_identity))
}

#[cfg(unix)]
fn native_amx_verify_owned_directory(
    directory: &Path,
    directory_handle: &File,
    directory_identity: NativeAmxFileIdentity,
    owner_uid: u32,
    lock_path: &Path,
    owner_lock: &File,
    lock_identity: NativeAmxFileIdentity,
) -> Result<(), NativeAmxSigningGuardError> {
    let opened_directory = directory_handle
        .metadata()
        .map_err(|error| native_amx_unsafe_journal(directory, error.to_string()))?;
    native_amx_validate_secure_directory_metadata(directory, &opened_directory, owner_uid)?;
    let linked_directory = fs::symlink_metadata(directory)
        .map_err(|error| native_amx_unsafe_journal(directory, error.to_string()))?;
    native_amx_validate_secure_directory_metadata(directory, &linked_directory, owner_uid)?;
    if native_amx_file_identity(&opened_directory) != directory_identity
        || native_amx_file_identity(&linked_directory) != directory_identity
    {
        return Err(native_amx_unsafe_journal(
            directory,
            "owned signer directory was replaced",
        ));
    }
    let opened_lock = owner_lock
        .metadata()
        .map_err(|error| native_amx_unsafe_journal(lock_path, error.to_string()))?;
    native_amx_validate_secure_file_metadata(lock_path, &opened_lock, 0, owner_uid)?;
    let linked_lock = fs::symlink_metadata(lock_path)
        .map_err(|error| native_amx_unsafe_journal(lock_path, error.to_string()))?;
    native_amx_validate_secure_file_metadata(lock_path, &linked_lock, 0, owner_uid)?;
    if native_amx_file_identity(&opened_lock) != lock_identity
        || native_amx_file_identity(&linked_lock) != lock_identity
    {
        return Err(native_amx_unsafe_journal(
            lock_path,
            "owner lock path was replaced",
        ));
    }
    Ok(())
}

#[cfg(not(unix))]
fn native_amx_verify_owned_directory(
    _directory: &Path,
    _directory_handle: &File,
    _directory_identity: NativeAmxFileIdentity,
    _owner_uid: u32,
    _lock_path: &Path,
    _owner_lock: &File,
    _lock_identity: NativeAmxFileIdentity,
) -> Result<(), NativeAmxSigningGuardError> {
    Err(NativeAmxSigningGuardError::UnsupportedPlatform)
}

fn native_amx_reconcile_guard_temps(
    directory: &Path,
    directory_handle: &File,
    owner_uid: u32,
    max_record_bytes: usize,
    max_anchor_bytes: usize,
) -> Result<(), NativeAmxSigningGuardError> {
    let mut temp_path = None;
    for item in fs::read_dir(directory)
        .map_err(|error| native_amx_unsafe_journal(directory, error.to_string()))?
    {
        let item = item.map_err(|error| native_amx_unsafe_journal(directory, error.to_string()))?;
        let path = item.path();
        let name = item.file_name();
        let name = name.to_string_lossy();
        if name == NATIVE_AMX_SIGNING_GUARD_LOCK_FILE
            || name == NATIVE_AMX_SIGNING_GUARD_ANCHOR_FILE
            || native_amx_valid_record_filename(&name)
        {
            continue;
        }
        let known_temp = name == NATIVE_AMX_SIGNING_GUARD_ANCHOR_TEMP
            || native_amx_valid_record_temp_filename(&name);
        if !known_temp || temp_path.is_some() {
            return Err(native_amx_unsafe_journal(
                &path,
                "unknown or multiple unpublished journal temps",
            ));
        }
        let metadata = fs::symlink_metadata(&path)
            .map_err(|error| native_amx_unsafe_journal(&path, error.to_string()))?;
        let max_bytes = if name == NATIVE_AMX_SIGNING_GUARD_ANCHOR_TEMP {
            max_anchor_bytes
        } else {
            max_record_bytes
        };
        native_amx_validate_secure_file_metadata(&path, &metadata, max_bytes, owner_uid)?;
        temp_path = Some(path);
    }
    if let Some(path) = temp_path {
        fs::remove_file(&path)
            .map_err(|error| native_amx_unsafe_journal(&path, error.to_string()))?;
        native_amx_sync_directory_handle(directory, directory_handle)?;
    }
    Ok(())
}

fn native_amx_secure_file_identity(
    path: &Path,
    max_bytes: usize,
    owner_uid: u32,
) -> Result<NativeAmxFileIdentity, NativeAmxSigningGuardError> {
    #[cfg(not(unix))]
    {
        let _ = (path, max_bytes, owner_uid);
        return Err(NativeAmxSigningGuardError::UnsupportedPlatform);
    }

    #[cfg(unix)]
    {
        let metadata = fs::symlink_metadata(path)
            .map_err(|error| native_amx_unsafe_journal(path, error.to_string()))?;
        native_amx_validate_secure_file_metadata(path, &metadata, max_bytes, owner_uid)?;
        Ok(native_amx_file_identity(&metadata))
    }
}

fn native_amx_read_secure_file(
    path: &Path,
    max_bytes: usize,
    owner_uid: u32,
) -> Result<Vec<u8>, NativeAmxSigningGuardError> {
    #[cfg(not(unix))]
    {
        let _ = (path, max_bytes, owner_uid);
        return Err(NativeAmxSigningGuardError::UnsupportedPlatform);
    }

    #[cfg(unix)]
    {
        let path_metadata = fs::symlink_metadata(path)
            .map_err(|error| native_amx_unsafe_journal(path, error.to_string()))?;
        native_amx_validate_secure_file_metadata(path, &path_metadata, max_bytes, owner_uid)?;
        let mut options = OpenOptions::new();
        options.read(true);
        native_amx_set_no_follow_flag(&mut options);
        let mut file = options
            .open(path)
            .map_err(|error| native_amx_unsafe_journal(path, error.to_string()))?;
        let opened = file
            .metadata()
            .map_err(|error| native_amx_unsafe_journal(path, error.to_string()))?;
        native_amx_validate_secure_file_metadata(path, &opened, max_bytes, owner_uid)?;
        let identity = native_amx_file_identity(&opened);
        if native_amx_file_identity(&path_metadata) != identity {
            return Err(native_amx_unsafe_journal(
                path,
                "file changed between inspection and open",
            ));
        }
        let initial_len = opened.len();
        let mut bytes = Vec::with_capacity(initial_len as usize);
        (&mut file)
            .take((max_bytes as u64).saturating_add(1))
            .read_to_end(&mut bytes)
            .map_err(|error| native_amx_unsafe_journal(path, error.to_string()))?;
        if bytes.len() > max_bytes || bytes.len() as u64 != initial_len {
            return Err(native_amx_unsafe_journal(
                path,
                "file changed size while being read",
            ));
        }
        let after = fs::symlink_metadata(path)
            .map_err(|error| native_amx_unsafe_journal(path, error.to_string()))?;
        native_amx_validate_secure_file_metadata(path, &after, max_bytes, owner_uid)?;
        if after.len() != initial_len || native_amx_file_identity(&after) != identity {
            return Err(native_amx_unsafe_journal(
                path,
                "file path changed while being read",
            ));
        }
        Ok(bytes)
    }
}

fn native_amx_write_new_secure_temp(
    path: &Path,
    bytes: &[u8],
    max_bytes: usize,
    owner_uid: u32,
) -> Result<(), NativeAmxSigningGuardError> {
    #[cfg(not(unix))]
    {
        let _ = (path, bytes, max_bytes, owner_uid);
        return Err(NativeAmxSigningGuardError::UnsupportedPlatform);
    }

    #[cfg(unix)]
    {
        if bytes.len() > max_bytes {
            return Err(native_amx_unsafe_journal(
                path,
                "temporary record exceeds its hard byte limit",
            ));
        }
        match fs::symlink_metadata(path) {
            Ok(_) => {
                return Err(native_amx_unsafe_journal(
                    path,
                    "unexpected pre-existing temporary record",
                ));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(native_amx_unsafe_journal(path, error.to_string())),
        }
        let mut options = OpenOptions::new();
        options
            .create_new(true)
            .write(true)
            .mode(NATIVE_AMX_SIGNING_FILE_MODE);
        native_amx_set_no_follow_flag(&mut options);
        let mut file = options
            .open(path)
            .map_err(|error| native_amx_unsafe_journal(path, error.to_string()))?;
        file.set_permissions(fs::Permissions::from_mode(NATIVE_AMX_SIGNING_FILE_MODE))
            .map_err(|error| native_amx_unsafe_journal(path, error.to_string()))?;
        file.write_all(bytes)
            .map_err(|error| native_amx_unsafe_journal(path, error.to_string()))?;
        file.sync_all()
            .map_err(|error| native_amx_unsafe_journal(path, error.to_string()))?;
        let opened = file
            .metadata()
            .map_err(|error| native_amx_unsafe_journal(path, error.to_string()))?;
        native_amx_validate_secure_file_metadata(path, &opened, max_bytes, owner_uid)?;
        if opened.len() != bytes.len() as u64 {
            return Err(native_amx_unsafe_journal(
                path,
                "temporary record length changed during write",
            ));
        }
        let linked = fs::symlink_metadata(path)
            .map_err(|error| native_amx_unsafe_journal(path, error.to_string()))?;
        native_amx_validate_secure_file_metadata(path, &linked, max_bytes, owner_uid)?;
        if native_amx_file_identity(&opened) != native_amx_file_identity(&linked) {
            return Err(native_amx_unsafe_journal(
                path,
                "temporary path changed during write",
            ));
        }
        Ok(())
    }
}

#[cfg(unix)]
fn native_amx_validate_secure_file_metadata(
    path: &Path,
    metadata: &fs::Metadata,
    max_bytes: usize,
    owner_uid: u32,
) -> Result<(), NativeAmxSigningGuardError> {
    native_amx_validate_uid(path, metadata, owner_uid)?;
    if metadata.file_type().is_symlink()
        || !metadata.file_type().is_file()
        || metadata.len() > max_bytes as u64
        || metadata.nlink() != 1
        || metadata.mode() & 0o777 != NATIVE_AMX_SIGNING_FILE_MODE
    {
        return Err(native_amx_unsafe_journal(
            path,
            "entry must be a single-link bounded regular file with mode 0600",
        ));
    }
    Ok(())
}

#[cfg(unix)]
fn native_amx_validate_uid(
    path: &Path,
    metadata: &fs::Metadata,
    expected_uid: u32,
) -> Result<(), NativeAmxSigningGuardError> {
    if metadata.uid() != expected_uid {
        return Err(native_amx_unsafe_journal(
            path,
            format!(
                "entry is owned by uid {}, expected effective uid {expected_uid}",
                metadata.uid()
            ),
        ));
    }
    Ok(())
}

#[cfg(unix)]
fn native_amx_effective_user_id(path: &Path) -> Result<u32, NativeAmxSigningGuardError> {
    // A newly created file is owned by the process's effective user. Probe in
    // the store itself so UID namespace/filesystem mappings match the guarded
    // entries, and read the kernel-assigned owner through safe Rust without
    // weakening the workspace-wide `unsafe_code = "deny"` policy.
    let probe = tempfile::tempfile_in(path).map_err(|error| {
        native_amx_unsafe_journal(path, format!("failed to establish effective UID: {error}"))
    })?;
    let metadata = probe.metadata().map_err(|error| {
        native_amx_unsafe_journal(path, format!("failed to inspect effective UID: {error}"))
    })?;
    Ok(metadata.uid())
}

fn native_amx_valid_record_filename(name: &str) -> bool {
    let suffix = format!(".{NATIVE_AMX_SIGNING_GUARD_RECORD_EXTENSION}");
    let Some(stem) = name.strip_suffix(&suffix) else {
        return false;
    };
    let mut fields = stem.split('.');
    let (Some(height), Some(sequence), Some(hash), None) =
        (fields.next(), fields.next(), fields.next(), fields.next())
    else {
        return false;
    };
    height.len() == 20
        && height.bytes().all(|byte| byte.is_ascii_digit())
        && sequence.len() == 10
        && sequence.bytes().all(|byte| byte.is_ascii_digit())
        && hash.len() == Hash::LENGTH * 2
        && hash.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn native_amx_valid_record_temp_filename(name: &str) -> bool {
    let suffix = format!(".{NATIVE_AMX_SIGNING_GUARD_TEMP_EXTENSION}");
    let Some(final_name) = name.strip_suffix(&suffix) else {
        return false;
    };
    native_amx_valid_record_filename(&format!(
        "{final_name}.{NATIVE_AMX_SIGNING_GUARD_RECORD_EXTENSION}"
    ))
}

#[cfg(unix)]
fn native_amx_file_identity(metadata: &fs::Metadata) -> NativeAmxFileIdentity {
    NativeAmxFileIdentity {
        device: metadata.dev(),
        inode: metadata.ino(),
    }
}

#[cfg(unix)]
fn native_amx_set_no_follow_flag(options: &mut OpenOptions) {
    options.custom_flags(native_amx_platform_no_follow_flag());
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn native_amx_platform_no_follow_flag() -> i32 {
    0o400000
}

#[cfg(all(
    unix,
    not(any(target_os = "linux", target_os = "android")),
    any(
        target_os = "macos",
        target_os = "ios",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd",
        target_os = "dragonfly"
    )
))]
fn native_amx_platform_no_follow_flag() -> i32 {
    0x100
}

#[cfg(all(
    unix,
    not(any(
        target_os = "linux",
        target_os = "android",
        target_os = "macos",
        target_os = "ios",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd",
        target_os = "dragonfly"
    ))
))]
fn native_amx_platform_no_follow_flag() -> i32 {
    0
}

fn native_amx_sync_directory_handle(
    path: &Path,
    directory: &File,
) -> Result<(), NativeAmxSigningGuardError> {
    directory
        .sync_all()
        .map_err(|error| native_amx_unsafe_journal(path, error.to_string()))
}

#[cfg(unix)]
fn native_amx_sync_directory_path(path: &Path) -> Result<(), NativeAmxSigningGuardError> {
    let mut options = OpenOptions::new();
    options.read(true);
    native_amx_set_no_follow_flag(&mut options);
    let directory = options
        .open(path)
        .map_err(|error| native_amx_unsafe_journal(path, error.to_string()))?;
    let metadata = directory
        .metadata()
        .map_err(|error| native_amx_unsafe_journal(path, error.to_string()))?;
    if !metadata.file_type().is_dir() {
        return Err(native_amx_unsafe_journal(path, "path is not a directory"));
    }
    native_amx_sync_directory_handle(path, &directory)
}

fn native_amx_unsafe_journal(
    path: &Path,
    message: impl Into<String>,
) -> NativeAmxSigningGuardError {
    NativeAmxSigningGuardError::UnsafeJournal(format!("{}: {}", path.display(), message.into()))
}

/// Native AMX session key scoped to one source transaction and routing plan.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode)]
pub struct NativeAmxSessionKey {
    /// Source transaction hash/id.
    pub source_id: [u8; iroha_crypto::Hash::LENGTH],
    /// Full routing-plan digest.
    pub plan_digest: Hash,
}

impl NativeAmxSessionKey {
    /// Construct a session key from an attestation body.
    #[must_use]
    pub fn from_body(body: &NativeAmxAttestationBodyV2) -> Self {
        Self {
            source_id: body.source_id,
            plan_digest: body.plan_digest,
        }
    }
}

/// Individual native AMX vote before participant committee aggregation.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct NativeAmxVoteV2 {
    /// Body signed by the participant validator.
    pub body: NativeAmxAttestationBodyV2,
    /// Validator that produced the vote.
    pub signer: PeerId,
    /// BLS signature over [`NativeAmxAttestationBodyV2::signature_preimage`].
    pub bls_signature: Vec<u8>,
}

/// Full-plan request presented to a native AMX participant committee.
///
/// The signed attestation body carries the stable plan digest and the exact
/// participant leg. The complete canonical leg list is included so a signer
/// can independently recompute that digest and reject omitted, extra,
/// duplicated, or role-swapped routes before producing a vote.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct NativeAmxAttestationRequestV2 {
    /// Participant attestation body that will be signed after validation.
    pub body: NativeAmxAttestationBodyV2,
    /// Complete plan in coordinator-first canonical order.
    pub plan_legs: Vec<RouteLeg>,
    /// Exact coordinator lane proposal whose transaction membership is being attested.
    ///
    /// This proposal is a non-circular pre-commitment: its hash binds lane
    /// coordinates, committee, predecessor, and transaction hashes, but does
    /// not include the native AMX receipt assembled from the resulting votes.
    pub coordinator_proposal: LaneBlockProposalV1,
    /// Exact control-only participant proposal whose result is supplied by the coordinator.
    pub participant_proposal: LaneBlockProposalV1,
    /// Deterministic participant-local settlement committed by that proposal.
    pub participant_settlement: LaneBlockCommitment,
}

/// Failure while validating a full-plan native AMX attestation request.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum NativeAmxRequestError {
    /// Request omitted a coordinator or every participant.
    #[error("native AMX request has an incomplete route plan")]
    IncompletePlan,
    /// Coordinator/participant roles or canonical ordering are invalid.
    #[error("native AMX request route roles or ordering are invalid")]
    InvalidRolesOrOrder,
    /// The same lane/dataspace participant route occurs more than once.
    #[error("native AMX request contains a duplicate participant route")]
    DuplicateRoute,
    /// The body names a coordinator or participant different from the plan.
    #[error("native AMX request body route does not match the full plan")]
    BodyRouteMismatch,
    /// The advertised digest does not commit to the supplied full plan.
    #[error("native AMX request plan digest mismatch")]
    PlanDigestMismatch,
    /// A request exceeds a protocol resource cap.
    #[error("native AMX request exceeds a protocol resource cap")]
    ResourceLimitExceeded,
    /// The supplied coordinator proposal is malformed.
    #[error("native AMX request coordinator proposal is malformed")]
    InvalidCoordinatorProposal,
    /// The supplied participant proposal is malformed.
    #[error("native AMX request participant proposal is malformed")]
    InvalidParticipantProposal,
    /// The attestation body does not bind the supplied coordinator proposal.
    #[error("native AMX request coordinator proposal binding mismatch")]
    CoordinatorProposalMismatch,
    /// The participant proposal or settlement differs from the signed body.
    #[error("native AMX request participant finality binding mismatch")]
    ParticipantProposalMismatch,
}

impl NativeAmxAttestationRequestV2 {
    /// Validate complete plan membership, canonical roles/order, and digest.
    ///
    /// # Errors
    /// Returns an error for malformed or replay-substituted plan evidence.
    pub fn validate_plan_binding(&self) -> Result<(), NativeAmxRequestError> {
        if !native_amx_body_shape_valid(&self.body)
            || self.plan_legs.len() > MAX_NATIVE_AMX_PLAN_LEGS
            || self.coordinator_proposal.descriptor.validator_set.len() > MAX_NATIVE_AMX_VALIDATORS
            || self
                .coordinator_proposal
                .descriptor
                .accepted_transaction_hashes
                .len()
                > crate::lane_consensus::MAX_LANE_EXECUTABLE_ENTRYPOINTS
            || self
                .coordinator_proposal
                .descriptor
                .accepted_candidate_indices
                .len()
                > crate::lane_consensus::MAX_LANE_EXECUTABLE_ENTRYPOINTS
            || self.participant_settlement.receipts.len()
                > MAX_NATIVE_AMX_PARTICIPANT_CONTROL_SOURCES
        {
            return Err(NativeAmxRequestError::ResourceLimitExceeded);
        }
        let Some(coordinator) = self.plan_legs.first().copied() else {
            return Err(NativeAmxRequestError::IncompletePlan);
        };
        if coordinator.role != RouteLegRole::Coordinator || self.plan_legs.len() < 2 {
            return Err(NativeAmxRequestError::IncompletePlan);
        }
        let participants = &self.plan_legs[1..];
        let mut previous = None;
        let mut seen = std::collections::BTreeSet::new();
        for participant in participants {
            if participant.role != RouteLegRole::Participant {
                return Err(NativeAmxRequestError::InvalidRolesOrOrder);
            }
            let key = (participant.route.dataspace_id, participant.route.lane_id);
            if previous.is_some_and(|previous| previous >= key) {
                return Err(if previous == Some(key) {
                    NativeAmxRequestError::DuplicateRoute
                } else {
                    NativeAmxRequestError::InvalidRolesOrOrder
                });
            }
            if !seen.insert(key) {
                return Err(NativeAmxRequestError::DuplicateRoute);
            }
            previous = Some(key);
        }
        let body = &self.body;
        if coordinator.route
            != RoutingDecision::new(body.coordinator_lane_id, body.coordinator_dataspace_id)
            || !participants.iter().any(|participant| {
                participant.route
                    == RoutingDecision::new(body.participant_lane_id, body.participant_dataspace_id)
            })
        {
            return Err(NativeAmxRequestError::BodyRouteMismatch);
        }
        let expected = RoutingPlan::native_amx(coordinator.route, participants.to_vec());
        if expected.digest() != body.plan_digest {
            return Err(NativeAmxRequestError::PlanDigestMismatch);
        }
        crate::lane_consensus::validate_lane_block_proposal(&self.coordinator_proposal)
            .map_err(|_| NativeAmxRequestError::InvalidCoordinatorProposal)?;
        let descriptor = &self.coordinator_proposal.descriptor;
        let entrypoint_hash = Hash::from(body.tx_entrypoint_hash);
        if self.coordinator_proposal.proposal_hash != body.coordinator_proposal_hash
            || descriptor.lane_id != body.coordinator_lane_id
            || descriptor.dataspace_id != body.coordinator_dataspace_id
            || descriptor.lane_incarnation != body.coordinator_lane_incarnation
            || descriptor.proposal_height != body.authority_context_height
            || descriptor.lane_block_height != body.planned_coordinator_block_height
            || descriptor.lane_block_view != body.coordinator_lane_block_view
            || body.round.height != body.authority_context_height
            || descriptor
                .accepted_transaction_hashes
                .iter()
                .filter(|hash| **hash == entrypoint_hash)
                .count()
                != 1
        {
            return Err(NativeAmxRequestError::CoordinatorProposalMismatch);
        }
        crate::lane_consensus::validate_lane_block_proposal(&self.participant_proposal)
            .map_err(|_| NativeAmxRequestError::InvalidParticipantProposal)?;
        let participant_descriptor = &self.participant_proposal.descriptor;
        let settlement_hash =
            iroha_data_model::nexus::compute_settlement_hash(&self.participant_settlement)
                .map_err(|_| NativeAmxRequestError::ParticipantProposalMismatch)?;
        let participant_is_coordinator_route = body.participant_lane_id == body.coordinator_lane_id
            && body.participant_dataspace_id == body.coordinator_dataspace_id;
        let participant_work_matches = if participant_is_coordinator_route {
            body.participant_lane_incarnation == body.coordinator_lane_incarnation
                && self
                    .participant_proposal
                    .same_consensus_identity(&self.coordinator_proposal)
        } else {
            true
        };
        let settlement_receipts = &self.participant_settlement.receipts;
        let participant_entrypoint_position = participant_descriptor
            .accepted_transaction_hashes
            .iter()
            .position(|hash| *hash == entrypoint_hash);
        let participant_entrypoint_count = participant_descriptor
            .accepted_transaction_hashes
            .iter()
            .filter(|hash| **hash == entrypoint_hash)
            .count();
        let settlement_sources_are_canonical = !settlement_receipts.is_empty()
            && settlement_receipts
                .iter()
                .map(|receipt| receipt.source_id)
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                == settlement_receipts.len()
            && settlement_receipts.iter().all(|receipt| {
                receipt.local_amount.is_zero()
                    && receipt.xor_due.is_zero()
                    && receipt.xor_after_haircut.is_zero()
                    && receipt.xor_variance.is_zero()
                    && receipt.timestamp_ms == body.authority_context_height
            })
            && settlement_receipts
                .iter()
                .filter(|receipt| receipt.source_id == body.source_id)
                .count()
                == 1
            && (participant_entrypoint_count == 0
                || (participant_entrypoint_count == 1
                    && participant_descriptor.accepted_candidate_indices.len()
                        == settlement_receipts.len()
                    && participant_descriptor.accepted_transaction_hashes.len()
                        == settlement_receipts.len()
                    && participant_entrypoint_position.is_some_and(|position| {
                        settlement_receipts
                            .get(position)
                            .is_some_and(|receipt| receipt.source_id == body.source_id)
                    })));
        if self.participant_proposal.payload_block_hint.is_some()
            || participant_descriptor.lane_id != body.participant_lane_id
            || participant_descriptor.dataspace_id != body.participant_dataspace_id
            || participant_descriptor.lane_incarnation != body.participant_lane_incarnation
            || participant_descriptor.proposal_height != body.authority_context_height
            || participant_descriptor.previous_lane_block_height
                != body.participant_previous_block_height
            || participant_descriptor.previous_lane_block_descriptor_hash
                != body.participant_previous_block_descriptor_hash
            || participant_descriptor.lane_block_height != body.participant_lane_block_height
            || participant_descriptor.lane_block_view != body.participant_lane_block_view
            || self.participant_proposal.proposal_hash != body.participant_proposal_hash
            || !participant_work_matches
            || participant_entrypoint_count > 1
            || (participant_is_coordinator_route && participant_entrypoint_count != 1)
            || participant_descriptor.validator_set_hash_version != VALIDATOR_SET_HASH_VERSION_V1
            || participant_descriptor.validator_set_hash != body.participant_validator_set_hash
            || participant_descriptor.validator_set_hash
                != HashOf::new(&participant_descriptor.validator_set)
            || participant_descriptor.validator_count != body.participant_validator_count
            || participant_descriptor.min_quorum != body.participant_min_quorum
            || self.participant_settlement.block_height != body.participant_lane_block_height
            || self.participant_settlement.lane_id != body.participant_lane_id
            || self.participant_settlement.dataspace_id != body.participant_dataspace_id
            || self.participant_settlement.lane_incarnation != body.participant_lane_incarnation
            || self.participant_settlement.tx_count
                != u64::try_from(settlement_receipts.len()).unwrap_or(u64::MAX)
            || !self.participant_settlement.total_local_amount.is_zero()
            || !self.participant_settlement.total_xor_due.is_zero()
            || !self
                .participant_settlement
                .total_xor_after_haircut
                .is_zero()
            || !self.participant_settlement.total_xor_variance.is_zero()
            || self.participant_settlement.swap_metadata.is_some()
            || !self.participant_settlement.nexus_fee_receipts.is_empty()
            || !self.participant_settlement.native_amx_receipts.is_empty()
            || !settlement_sources_are_canonical
            || Hash::from(settlement_hash) != body.participant_settlement_commitment
        {
            return Err(NativeAmxRequestError::ParticipantProposalMismatch);
        }
        Ok(())
    }
}

fn peer_uses_bls_normal(peer: &PeerId) -> bool {
    peer.public_key()
        .try_algorithm()
        .is_ok_and(|algorithm| algorithm == Algorithm::BlsNormal)
}

fn native_amx_hash_is_zero_sentinel(bytes: &[u8]) -> bool {
    bytes.len() == Hash::LENGTH
        && bytes[..Hash::LENGTH - 1].iter().all(|byte| *byte == 0)
        && bytes[Hash::LENGTH - 1] <= 1
}

fn native_amx_body_shape_valid(body: &NativeAmxAttestationBodyV2) -> bool {
    let Ok(validator_count) = usize::try_from(body.participant_validator_count) else {
        return false;
    };
    let Ok(min_quorum) = usize::try_from(body.participant_min_quorum) else {
        return false;
    };
    let expected_quorum =
        crate::sumeragi::network_topology::commit_quorum_from_len(validator_count).max(1);
    body.round.height != 0
        && !native_amx_hash_is_zero_sentinel(body.round.context_id.0.as_ref())
        && body.authority_context_height == body.round.height
        && body.planned_coordinator_block_height != 0
        && !native_amx_hash_is_zero_sentinel(body.chain_id_hash.as_ref())
        && body.source_id.iter().any(|byte| *byte != 0)
        && !native_amx_hash_is_zero_sentinel(body.tx_entrypoint_hash.as_ref())
        && !native_amx_hash_is_zero_sentinel(body.plan_digest.as_ref())
        && !native_amx_hash_is_zero_sentinel(body.coordinator_lane_incarnation.as_ref())
        && !native_amx_hash_is_zero_sentinel(body.participant_lane_incarnation.as_ref())
        && body.participant_lane_block_height != 0
        && body.participant_previous_block_height.checked_add(1)
            == Some(body.participant_lane_block_height)
        && (body.participant_previous_block_height == 0)
            == body.participant_previous_block_descriptor_hash.is_none()
        && body
            .participant_previous_block_descriptor_hash
            .is_none_or(|hash| !native_amx_hash_is_zero_sentinel(hash.as_ref()))
        && !native_amx_hash_is_zero_sentinel(body.participant_proposal_hash.as_ref())
        && !native_amx_hash_is_zero_sentinel(body.participant_settlement_commitment.as_ref())
        && !native_amx_hash_is_zero_sentinel(body.participant_validator_set_hash.as_ref())
        && !native_amx_hash_is_zero_sentinel(body.coordinator_proposal_hash.as_ref())
        && validator_count != 0
        && validator_count <= MAX_NATIVE_AMX_VALIDATORS
        && min_quorum == expected_quorum
}

impl NativeAmxVoteV2 {
    /// Validate the cheap, stateless vote envelope before doing BLS verification.
    ///
    /// This checks the phase, authenticated transport signer, body shape, signature
    /// length, and BLS-normal signer algorithm without parsing or verifying the
    /// attacker-controlled signature bytes.
    pub(crate) fn validate_ingress_shape(
        &self,
        expected_phase: NativeAmxPhase,
        sender: Option<&PeerId>,
    ) -> Result<(), NativeAmxVoteIngressError> {
        if self.body.phase != expected_phase {
            return Err(NativeAmxVoteIngressError::PhaseMismatch {
                expected: expected_phase,
                actual: self.body.phase,
            });
        }
        if let Some(sender) = sender
            && sender != &self.signer
        {
            return Err(NativeAmxVoteIngressError::SenderMismatch);
        }
        if !native_amx_body_shape_valid(&self.body) {
            return Err(NativeAmxVoteIngressError::InvalidBody);
        }
        if self.bls_signature.len() != NATIVE_AMX_BLS_PROOF_BYTES {
            return Err(NativeAmxVoteIngressError::InvalidSignature);
        }
        if !peer_uses_bls_normal(&self.signer) {
            return Err(NativeAmxVoteIngressError::SignerNotBlsNormal);
        }
        Ok(())
    }

    /// Parse and verify the BLS signature after cheap authorization gates pass.
    pub(crate) fn verify_signature(&self) -> Result<(), NativeAmxVoteIngressError> {
        Signature::try_from_bytes(&self.bls_signature)
            .map_err(|_| NativeAmxVoteIngressError::InvalidSignature)?
            .verify(self.signer.public_key(), &self.body.signature_preimage())
            .map_err(|_| NativeAmxVoteIngressError::InvalidSignature)
    }

    /// Validate phase, transport signer binding, BLS-normal identity, and vote signature.
    ///
    /// This is the stateless ingress prefilter. Callers that know the current world state must
    /// still verify that the signer has a live proof of possession at the planned block height.
    ///
    /// # Errors
    /// Returns an error when the vote is carried by the wrong phase message, the authenticated
    /// sender does not match the signer, the signer is not BLS-normal, or the BLS signature does
    /// not verify against the canonical attestation preimage.
    pub fn validate_ingress(
        &self,
        expected_phase: NativeAmxPhase,
        sender: Option<&PeerId>,
    ) -> Result<(), NativeAmxVoteIngressError> {
        self.validate_ingress_shape(expected_phase, sender)?;
        self.verify_signature()
    }
}

/// Native AMX control-plane request or vote.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub enum NativeAmxMessage {
    /// Coordinator asks a participant dataspace committee to prepare a leg.
    PrepareRequest(NativeAmxAttestationRequestV2),
    /// Participant validator prepare vote.
    PrepareVote(NativeAmxVoteV2),
    /// Coordinator asks a participant committee to commit after proving Prepare.
    CommitRequest(NativeAmxCommitRequestV2),
    /// Participant validator commit vote.
    CommitVote(NativeAmxVoteV2),
}

/// Context-bound native AMX Commit request carrying the prerequisite PrepareQC.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct NativeAmxCommitRequestV2 {
    /// Commit-phase full-plan request presented to the participant committee.
    pub request: NativeAmxAttestationRequestV2,
    /// Prepare certificate for the same context, transaction, plan, and leg.
    pub prepare_qc: NativeAmxAttestationQcV2,
}

impl NativeAmxCommitRequestV2 {
    /// Validate that the request advances exactly one certified participant leg
    /// from Prepare to Commit.
    ///
    /// # Errors
    ///
    /// Returns [`NativeAmxCommitRequestError`] if either phase is wrong or any
    /// signed context, transaction, plan, route, or height field differs.
    pub fn validate_shape(&self) -> Result<(), NativeAmxCommitRequestError> {
        if self.request.validate_plan_binding().is_err() {
            return Err(NativeAmxCommitRequestError::InvalidPlanBinding);
        }
        if self.request.body.phase != NativeAmxPhase::Commit {
            return Err(NativeAmxCommitRequestError::CommitPhaseMismatch);
        }
        if self.prepare_qc.body.phase != NativeAmxPhase::Prepare {
            return Err(NativeAmxCommitRequestError::PreparePhaseMismatch);
        }
        if !native_amx_bodies_match_leg(&self.request.body, &self.prepare_qc.body) {
            return Err(NativeAmxCommitRequestError::LegMismatch);
        }
        Ok(())
    }
}

/// Structural failure in a native AMX v2 Commit request.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum NativeAmxCommitRequestError {
    /// full-plan or coordinator-proposal evidence is malformed
    #[error("native AMX commit request full-plan binding is invalid")]
    InvalidPlanBinding,
    /// requested body is not a Commit body
    #[error("native AMX commit request body is not in Commit phase")]
    CommitPhaseMismatch,
    /// prerequisite certificate is not a PrepareQC
    #[error("native AMX commit request prerequisite is not a PrepareQC")]
    PreparePhaseMismatch,
    /// PrepareQC and Commit body describe different context or participant work
    #[error("native AMX commit request changes its prepared participant leg")]
    LegMismatch,
}

/// Failure while validating a native AMX vote before session-cache insertion.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum NativeAmxVoteIngressError {
    /// native AMX vote message phase does not match the embedded body phase
    #[error("native AMX vote phase mismatch: expected {expected:?}, got {actual:?}")]
    PhaseMismatch {
        /// Phase implied by the received message variant.
        expected: NativeAmxPhase,
        /// Phase embedded in the signed attestation body.
        actual: NativeAmxPhase,
    },
    /// native AMX vote was transported by a peer other than the signer
    #[error("native AMX vote sender does not match signer")]
    SenderMismatch,
    /// native AMX vote body has malformed or oversized authority coordinates
    #[error("native AMX vote body is malformed")]
    InvalidBody,
    /// native AMX vote signer is not a BLS-normal consensus identity
    #[error("native AMX vote signer is not BLS-normal")]
    SignerNotBlsNormal,
    /// native AMX vote signature is missing, malformed, or invalid
    #[error("native AMX vote signature is invalid")]
    InvalidSignature,
}

/// Failure while adding a native AMX vote to the session cache.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum NativeAmxSessionError {
    /// native AMX vote phase does not match the target cache bucket
    #[error("native AMX vote phase does not match the target cache bucket")]
    PhaseMismatch,
    /// native AMX vote signer already exists in this session
    #[error("native AMX vote signer already exists in this session")]
    DuplicateSigner,
    /// one source transaction attempted to occupy two live routing plans
    #[error("native AMX source transaction attempted routing-plan equivocation")]
    PlanEquivocation,
    /// Retaining another session or exact-body bucket would exceed the configured bound.
    #[error("native AMX session cache reached its configured capacity")]
    Capacity,
}

/// Failure while building a native AMX attestation QC from participant votes.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum NativeAmxQcBuildError {
    /// no votes were supplied for the requested native AMX phase
    #[error("no votes were supplied for the requested native AMX phase")]
    EmptyVotes,
    /// participant committee is empty, oversized, duplicated, non-canonical, or has a non-canonical quorum
    #[error("native AMX participant validator set is malformed")]
    InvalidValidatorSet,
    /// signed participant committee hash/count/quorum does not match assembly inputs
    #[error("native AMX signed participant committee context mismatch")]
    CommitteeContextMismatch,
    /// aligned historical proof-of-possession material is malformed or invalid
    #[error("native AMX participant validator proof-of-possession is invalid")]
    InvalidProofOfPossession,
    /// a vote signed a different native AMX attestation body
    #[error("a vote signed a different native AMX attestation body")]
    BodyMismatch,
    /// a vote signer is not in the participant validator set
    #[error("a vote signer is not in the participant validator set")]
    SignerNotInValidatorSet,
    /// a vote signer appears more than once
    #[error("a vote signer appears more than once")]
    DuplicateSigner,
    /// a vote signer is not a BLS-normal consensus identity
    #[error("a vote signer is not BLS-normal")]
    SignerNotBlsNormal,
    /// an individual vote signature is missing, malformed, or invalid
    #[error("an individual native AMX vote signature is invalid")]
    InvalidSignature,
    /// the vote set does not satisfy the participant quorum
    #[error("native AMX vote quorum is not met")]
    QuorumNotMet,
    /// BLS signature aggregation failed
    #[error("failed to aggregate native AMX BLS signatures")]
    SignatureAggregate,
}

/// Failure while validating an aggregated native AMX v2 certificate.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum NativeAmxQcValidationError {
    /// certificate body differs from the exact expected body
    #[error("native AMX QC body mismatch")]
    BodyMismatch,
    /// authoritative participant committee is empty, oversized, duplicated, non-canonical, or has a non-canonical quorum
    #[error("native AMX QC validator set is malformed")]
    InvalidValidatorSet,
    /// certificate validator set differs from the authoritative committee
    #[error("native AMX QC validator set mismatch")]
    ValidatorSetMismatch,
    /// validator-set hash metadata is malformed
    #[error("native AMX QC validator-set hash mismatch")]
    ValidatorSetHashMismatch,
    /// signer bitmap has the wrong length or an out-of-range bit
    #[error("native AMX QC signer bitmap is malformed")]
    InvalidSignerBitmap,
    /// signer bitmap is below the required committee quorum
    #[error("native AMX QC quorum is not met")]
    QuorumNotMet,
    /// selected signer is not a BLS-normal identity
    #[error("native AMX QC signer is not BLS-normal")]
    SignerNotBlsNormal,
    /// selected signer has no valid proof of possession
    #[error("native AMX QC signer proof of possession is missing or invalid")]
    InvalidProofOfPossession,
    /// aggregate signature is empty or does not verify
    #[error("native AMX QC aggregate signature is invalid")]
    InvalidAggregateSignature,
}

/// Validate one exact context-bound native AMX certificate against the frozen
/// participant committee and its proof-of-possession map.
///
/// # Errors
///
/// Returns [`NativeAmxQcValidationError`] for body, committee, quorum, PoP, or
/// aggregate-signature drift.
pub fn validate_native_amx_qc(
    qc: &NativeAmxAttestationQcV2,
    expected_body: &NativeAmxAttestationBodyV2,
    validator_set: &[PeerId],
    min_signers: usize,
    pops: &BTreeMap<PublicKey, Vec<u8>>,
) -> Result<(), NativeAmxQcValidationError> {
    if &qc.body != expected_body {
        return Err(NativeAmxQcValidationError::BodyMismatch);
    }
    let expected_quorum =
        crate::sumeragi::network_topology::commit_quorum_from_len(validator_set.len()).max(1);
    if validator_set.is_empty()
        || validator_set.len() > MAX_NATIVE_AMX_VALIDATORS
        || validator_set.windows(2).any(|pair| pair[0] >= pair[1])
        || min_signers != expected_quorum
    {
        return Err(NativeAmxQcValidationError::InvalidValidatorSet);
    }
    if qc.validator_set != validator_set {
        return Err(NativeAmxQcValidationError::ValidatorSetMismatch);
    }
    if qc.validator_set_hash_version != VALIDATOR_SET_HASH_VERSION_V1
        || qc.validator_set_hash != HashOf::new(&validator_set.to_vec())
        || qc.body.participant_validator_set_hash != qc.validator_set_hash
        || usize::try_from(qc.body.participant_validator_count) != Ok(validator_set.len())
        || usize::try_from(qc.body.participant_min_quorum) != Ok(min_signers)
    {
        return Err(NativeAmxQcValidationError::ValidatorSetHashMismatch);
    }
    let expected_bitmap_len = validator_set.len().div_ceil(8);
    if qc.signers_bitmap.len() != expected_bitmap_len
        || qc.bls_aggregate_signature.len() != NATIVE_AMX_BLS_PROOF_BYTES
    {
        return Err(
            if qc.bls_aggregate_signature.len() != NATIVE_AMX_BLS_PROOF_BYTES {
                NativeAmxQcValidationError::InvalidAggregateSignature
            } else {
                NativeAmxQcValidationError::InvalidSignerBitmap
            },
        );
    }

    if qc.validator_set_pops.len() != validator_set.len() {
        return Err(NativeAmxQcValidationError::InvalidProofOfPossession);
    }
    for (validator, embedded_pop) in validator_set.iter().zip(&qc.validator_set_pops) {
        if !peer_uses_bls_normal(validator) {
            return Err(NativeAmxQcValidationError::SignerNotBlsNormal);
        }
        let pop = pops
            .get(validator.public_key())
            .filter(|pop| pop.len() == NATIVE_AMX_BLS_PROOF_BYTES)
            .ok_or(NativeAmxQcValidationError::InvalidProofOfPossession)?;
        if embedded_pop != pop {
            return Err(NativeAmxQcValidationError::InvalidProofOfPossession);
        }
        iroha_crypto::bls_normal_pop_verify(validator.public_key(), embedded_pop)
            .map_err(|_| NativeAmxQcValidationError::InvalidProofOfPossession)?;
    }

    let mut signer_keys = Vec::new();
    let mut signer_pops = Vec::new();
    for (byte_index, byte) in qc.signers_bitmap.iter().copied().enumerate() {
        for bit in 0..8 {
            if byte & (1_u8 << bit) == 0 {
                continue;
            }
            let index = byte_index * 8 + bit;
            let Some(signer) = validator_set.get(index) else {
                return Err(NativeAmxQcValidationError::InvalidSignerBitmap);
            };
            let pop = pops
                .get(signer.public_key())
                .ok_or(NativeAmxQcValidationError::InvalidProofOfPossession)?;
            signer_keys.push(signer.public_key());
            signer_pops.push(pop.as_slice());
        }
    }
    if signer_keys.len() < min_signers {
        return Err(NativeAmxQcValidationError::QuorumNotMet);
    }
    iroha_crypto::bls_normal_verify_preaggregated_same_message(
        &expected_body.signature_preimage(),
        &qc.bls_aggregate_signature,
        &signer_keys,
        &signer_pops,
    )
    .map_err(|_| NativeAmxQcValidationError::InvalidAggregateSignature)
}

/// Validate the bounded, producer-hashable shape of an aligned native AMX v2 receipt.
///
/// This deliberately performs no aggregate cryptography or state lookup. Block
/// admission and merge pre-execution additionally validate the historical route,
/// committee authority, proofs of possession, and aggregate signatures.
#[must_use]
pub(crate) fn receipt_shape_matches_coordinator_payload(
    receipt: Option<&iroha_data_model::block::consensus::NativeAmxReceipt>,
    routing_plan: &RoutingPlan,
    expected_source_id: &[u8],
    expected_entrypoint_hash: Hash,
    expected_chain_id_hash: Hash,
    coordinator_proposal: &LaneBlockProposalV1,
) -> bool {
    let NativeAmx(native_plan) = routing_plan else {
        return receipt.is_none();
    };
    if native_plan.participants.is_empty()
        || !native_amx_participant_leg_count_within_limit(native_plan.participants.len())
    {
        return false;
    }
    let Some(receipt) = receipt else {
        return false;
    };
    let descriptor = &coordinator_proposal.descriptor;
    if receipt.version != 2
        || receipt.source_id.as_slice() != expected_source_id
        || receipt.chain_id_hash != expected_chain_id_hash
        || receipt.plan_digest != routing_plan.digest()
        || receipt.lane_id != descriptor.lane_id
        || receipt.dataspace_id != descriptor.dataspace_id
        || receipt.lane_incarnation != descriptor.lane_incarnation
        || receipt.authority_context_height != descriptor.proposal_height
        || receipt.lane_block_height != descriptor.lane_block_height
        || receipt.lane_block_view != descriptor.lane_block_view
        || receipt.coordinator_proposal_hash != coordinator_proposal.proposal_hash
        || receipt.legs.len() != native_plan.participants.len()
        || !native_amx_participant_leg_count_within_limit(receipt.legs.len())
    {
        return false;
    }
    let Some(first_leg) = receipt.legs.first() else {
        return false;
    };
    let expected_round = first_leg.prepare_qc.body.round;
    let expected_epoch = first_leg.prepare_qc.body.epoch;

    receipt
        .legs
        .iter()
        .zip(&native_plan.participants)
        .all(|(leg, planned)| {
            if leg.lane_id != planned.route.lane_id
                || leg.dataspace_id != planned.route.dataspace_id
            {
                return false;
            }
            let prepare = &leg.prepare_qc;
            let commit = &leg.commit_qc;
            let participant_request = NativeAmxAttestationRequestV2 {
                body: prepare.body,
                plan_legs: routing_plan.legs(),
                coordinator_proposal: coordinator_proposal.clone(),
                participant_proposal: leg.participant_proposal.clone(),
                participant_settlement: leg.participant_settlement.clone(),
            };
            if participant_request.validate_plan_binding().is_err()
                || iroha_data_model::nexus::compute_settlement_hash(&leg.participant_settlement)
                    .ok()
                    != Some(leg.participant_settlement_hash)
                || Hash::from(leg.participant_settlement_hash)
                    != prepare.body.participant_settlement_commitment
                || native_amx_participant_application_role(receipt, leg).is_err()
            {
                return false;
            }
            let common_qc_shape = |qc: &NativeAmxAttestationQcV2, phase: NativeAmxPhase| {
                let body = &qc.body;
                let validator_count = qc.validator_set.len();
                let expected_quorum =
                    crate::sumeragi::network_topology::commit_quorum_from_len(validator_count)
                        .max(1);
                let signer_count = qc
                    .signers_bitmap
                    .iter()
                    .map(|byte| byte.count_ones() as usize)
                    .sum::<usize>();
                let trailing_bits_clear = qc.signers_bitmap.last().is_none_or(|last| {
                    let used = validator_count % 8;
                    used == 0 || *last & !((1_u8 << used) - 1) == 0
                });
                let Ok(advertised_validator_count) =
                    usize::try_from(body.participant_validator_count)
                else {
                    return false;
                };
                let Ok(advertised_min_quorum) = usize::try_from(body.participant_min_quorum) else {
                    return false;
                };
                body.round == expected_round
                    && body.round.height == receipt.authority_context_height
                    && body.epoch == expected_epoch
                    && body.chain_id_hash == expected_chain_id_hash
                    && body.source_id == receipt.source_id
                    && Hash::from(body.tx_entrypoint_hash) == expected_entrypoint_hash
                    && body.plan_digest == receipt.plan_digest
                    && body.phase == phase
                    && body.coordinator_lane_id == descriptor.lane_id
                    && body.coordinator_dataspace_id == descriptor.dataspace_id
                    && body.coordinator_lane_incarnation == descriptor.lane_incarnation
                    && body.participant_lane_id == leg.lane_id
                    && body.participant_dataspace_id == leg.dataspace_id
                    && body
                        .participant_lane_incarnation
                        .as_ref()
                        .iter()
                        .any(|byte| *byte != 0)
                    && body.participant_previous_block_height
                        == leg
                            .participant_proposal
                            .descriptor
                            .previous_lane_block_height
                    && body.participant_previous_block_descriptor_hash
                        == leg
                            .participant_proposal
                            .descriptor
                            .previous_lane_block_descriptor_hash
                    && body.participant_lane_block_height
                        == leg.participant_proposal.descriptor.lane_block_height
                    && body.participant_lane_block_view
                        == leg.participant_proposal.descriptor.lane_block_view
                    && body.participant_proposal_hash == leg.participant_proposal.proposal_hash
                    && body.participant_settlement_commitment
                        == Hash::from(leg.participant_settlement_hash)
                    && body.authority_context_height == descriptor.proposal_height
                    && body.planned_coordinator_block_height == descriptor.lane_block_height
                    && body.coordinator_lane_block_view == descriptor.lane_block_view
                    && body.coordinator_proposal_hash == coordinator_proposal.proposal_hash
                    && validator_count > 0
                    && validator_count <= MAX_NATIVE_AMX_VALIDATORS
                    && advertised_validator_count == validator_count
                    && advertised_min_quorum == expected_quorum
                    && qc.validator_set_hash_version == VALIDATOR_SET_HASH_VERSION_V1
                    && qc.validator_set_hash == HashOf::new(&qc.validator_set)
                    && qc.validator_set_hash == body.participant_validator_set_hash
                    && qc.validator_set.windows(2).all(|pair| pair[0] < pair[1])
                    && qc.validator_set.iter().all(peer_uses_bls_normal)
                    && qc.validator_set_pops.len() == validator_count
                    && qc
                        .validator_set_pops
                        .iter()
                        .all(|pop| pop.len() == NATIVE_AMX_BLS_PROOF_BYTES)
                    && qc.signers_bitmap.len() == validator_count.div_ceil(8)
                    && trailing_bits_clear
                    && signer_count >= expected_quorum
                    && qc.bls_aggregate_signature.len() == NATIVE_AMX_BLS_PROOF_BYTES
            };
            if !common_qc_shape(prepare, NativeAmxPhase::Prepare)
                || !common_qc_shape(commit, NativeAmxPhase::Commit)
                || prepare.validator_set != commit.validator_set
                || prepare.validator_set_pops != commit.validator_set_pops
                || prepare.validator_set_hash != commit.validator_set_hash
            {
                return false;
            }
            let mut expected_commit_body = prepare.body;
            expected_commit_body.phase = NativeAmxPhase::Commit;
            commit.body == expected_commit_body
        })
}

/// Return whether a participant-only leg count fits the coordinator-inclusive plan cap.
#[must_use]
pub(crate) const fn native_amx_participant_leg_count_within_limit(count: usize) -> bool {
    count <= MAX_NATIVE_AMX_PARTICIPANT_LEGS
}

fn native_amx_bodies_match_leg(
    left: &NativeAmxAttestationBodyV2,
    right: &NativeAmxAttestationBodyV2,
) -> bool {
    left.round == right.round
        && left.epoch == right.epoch
        && left.chain_id_hash == right.chain_id_hash
        && left.source_id == right.source_id
        && left.tx_entrypoint_hash == right.tx_entrypoint_hash
        && left.plan_digest == right.plan_digest
        && left.coordinator_lane_id == right.coordinator_lane_id
        && left.coordinator_dataspace_id == right.coordinator_dataspace_id
        && left.coordinator_lane_incarnation == right.coordinator_lane_incarnation
        && left.participant_lane_id == right.participant_lane_id
        && left.participant_dataspace_id == right.participant_dataspace_id
        && left.participant_lane_incarnation == right.participant_lane_incarnation
        && left.participant_previous_block_height == right.participant_previous_block_height
        && left.participant_previous_block_descriptor_hash
            == right.participant_previous_block_descriptor_hash
        && left.participant_lane_block_height == right.participant_lane_block_height
        && left.participant_lane_block_view == right.participant_lane_block_view
        && left.participant_proposal_hash == right.participant_proposal_hash
        && left.participant_settlement_commitment == right.participant_settlement_commitment
        && left.participant_validator_set_hash == right.participant_validator_set_hash
        && left.participant_validator_count == right.participant_validator_count
        && left.participant_min_quorum == right.participant_min_quorum
        && left.authority_context_height == right.authority_context_height
        && left.planned_coordinator_block_height == right.planned_coordinator_block_height
        && left.coordinator_lane_block_view == right.coordinator_lane_block_view
        && left.coordinator_proposal_hash == right.coordinator_proposal_hash
}

/// Build a native AMX attestation QC from sorted or unsorted participant votes.
///
/// The resulting bitmap and aggregate signature are deterministic because votes are projected into
/// the supplied validator-set order before aggregation.
///
/// # Errors
/// Returns an error when the committee or its canonical commit threshold is
/// malformed, votes do not match `body`, include duplicate or unknown signers,
/// fail to meet `min_signers`, or cannot be aggregated as BLS-normal
/// signatures.
pub fn aggregate_votes_to_qc(
    body: NativeAmxAttestationBodyV2,
    validator_set: Vec<PeerId>,
    validator_set_pops: Vec<Vec<u8>>,
    votes: &[NativeAmxVoteV2],
    min_signers: usize,
) -> Result<NativeAmxAttestationQcV2, NativeAmxQcBuildError> {
    if votes.is_empty() {
        return Err(NativeAmxQcBuildError::EmptyVotes);
    }
    let expected_quorum =
        crate::sumeragi::network_topology::commit_quorum_from_len(validator_set.len()).max(1);
    if validator_set.is_empty()
        || validator_set.len() > MAX_NATIVE_AMX_VALIDATORS
        || validator_set.windows(2).any(|pair| pair[0] >= pair[1])
        || min_signers != expected_quorum
    {
        return Err(NativeAmxQcBuildError::InvalidValidatorSet);
    }
    let Ok(validator_count) = u32::try_from(validator_set.len()) else {
        return Err(NativeAmxQcBuildError::InvalidValidatorSet);
    };
    let Ok(min_quorum) = u32::try_from(min_signers) else {
        return Err(NativeAmxQcBuildError::CommitteeContextMismatch);
    };
    if body.participant_validator_set_hash != HashOf::new(&validator_set)
        || body.participant_validator_count != validator_count
        || body.participant_min_quorum != min_quorum
    {
        return Err(NativeAmxQcBuildError::CommitteeContextMismatch);
    }
    if validator_set_pops.len() != validator_set.len()
        || validator_set_pops
            .iter()
            .any(|pop| pop.len() != NATIVE_AMX_BLS_PROOF_BYTES)
    {
        return Err(NativeAmxQcBuildError::InvalidProofOfPossession);
    }
    if validator_set.iter().any(|peer| !peer_uses_bls_normal(peer)) {
        return Err(NativeAmxQcBuildError::SignerNotBlsNormal);
    }
    for (validator, pop) in validator_set.iter().zip(&validator_set_pops) {
        if iroha_crypto::bls_normal_pop_verify(validator.public_key(), pop).is_err() {
            return Err(NativeAmxQcBuildError::InvalidProofOfPossession);
        }
    }

    let mut indexed_signatures: BTreeMap<usize, Vec<u8>> = BTreeMap::new();
    for vote in votes {
        if vote.body != body {
            return Err(NativeAmxQcBuildError::BodyMismatch);
        }
        let Some(index) = validator_set
            .iter()
            .position(|validator| validator == &vote.signer)
        else {
            return Err(NativeAmxQcBuildError::SignerNotInValidatorSet);
        };
        if indexed_signatures
            .insert(index, vote.bls_signature.clone())
            .is_some()
        {
            return Err(NativeAmxQcBuildError::DuplicateSigner);
        }
        if !peer_uses_bls_normal(&vote.signer) {
            return Err(NativeAmxQcBuildError::SignerNotBlsNormal);
        }
        if vote.bls_signature.len() != NATIVE_AMX_BLS_PROOF_BYTES {
            return Err(NativeAmxQcBuildError::InvalidSignature);
        }
        let signature = Signature::try_from_bytes(&vote.bls_signature)
            .map_err(|_| NativeAmxQcBuildError::InvalidSignature)?;
        if signature
            .verify(vote.signer.public_key(), &body.signature_preimage())
            .is_err()
        {
            return Err(NativeAmxQcBuildError::InvalidSignature);
        }
    }

    if indexed_signatures.len() < min_signers {
        return Err(NativeAmxQcBuildError::QuorumNotMet);
    }

    let mut signers_bitmap = vec![0_u8; validator_set.len().div_ceil(8)];
    let ordered_signatures = indexed_signatures
        .into_iter()
        .map(|(index, signature)| {
            signers_bitmap[index / 8] |= 1_u8 << (index % 8);
            signature
        })
        .collect::<Vec<_>>();
    let signature_refs = ordered_signatures
        .iter()
        .map(Vec::as_slice)
        .collect::<Vec<_>>();
    let bls_aggregate_signature = iroha_crypto::bls_normal_aggregate_signatures(&signature_refs)
        .map_err(|_| NativeAmxQcBuildError::SignatureAggregate)?;

    Ok(NativeAmxAttestationQcV2 {
        body,
        validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
        validator_set_hash: HashOf::new(&validator_set),
        validator_set,
        validator_set_pops,
        signers_bitmap,
        bls_aggregate_signature,
    })
}

#[derive(Default)]
struct NativeAmxSession {
    votes: BTreeMap<NativeAmxVoteBucket, BTreeMap<PeerId, NativeAmxVoteV2>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct NativeAmxVoteBucket {
    body: NativeAmxAttestationBodyV2,
}

impl NativeAmxVoteBucket {
    const fn from_body(body: &NativeAmxAttestationBodyV2) -> Self {
        Self { body: *body }
    }
}

impl NativeAmxSession {
    fn insert_vote(
        &mut self,
        vote: NativeAmxVoteV2,
        max_body_buckets: NonZeroUsize,
    ) -> Result<(), NativeAmxSessionError> {
        let bucket = NativeAmxVoteBucket::from_body(&vote.body);
        if !self.votes.contains_key(&bucket) && self.votes.len() >= max_body_buckets.get() {
            return Err(NativeAmxSessionError::Capacity);
        }
        let target = self.votes.entry(bucket).or_default();
        if target.contains_key(&vote.signer) {
            return Err(NativeAmxSessionError::DuplicateSigner);
        }
        target.insert(vote.signer.clone(), vote);
        Ok(())
    }

    fn votes_for_body(&self, body: &NativeAmxAttestationBodyV2) -> Vec<NativeAmxVoteV2> {
        self.votes
            .get(&NativeAmxVoteBucket::from_body(body))
            .map(|source| source.values().cloned().collect())
            .unwrap_or_default()
    }
}

/// Bounded cache of native AMX vote sessions keyed by source transaction and plan digest.
pub struct NativeAmxSessionCache {
    max_sessions: NonZeroUsize,
    max_body_buckets_per_session: NonZeroUsize,
    sessions: BTreeMap<NativeAmxSessionKey, NativeAmxSession>,
    source_plan_claims: BTreeMap<[u8; iroha_crypto::Hash::LENGTH], Hash>,
}

impl NativeAmxSessionCache {
    /// Create a bounded native AMX session cache.
    #[must_use]
    pub fn new(max_sessions: NonZeroUsize) -> Self {
        Self::with_limits(
            max_sessions,
            NonZeroUsize::new(DEFAULT_SESSION_BODY_BUCKET_MAX).expect("default is non-zero"),
        )
    }

    /// Create a bounded native AMX session cache with an exact-body cap per session.
    #[must_use]
    pub fn with_limits(
        max_sessions: NonZeroUsize,
        max_body_buckets_per_session: NonZeroUsize,
    ) -> Self {
        Self {
            max_sessions,
            max_body_buckets_per_session,
            sessions: BTreeMap::new(),
            source_plan_claims: BTreeMap::new(),
        }
    }

    /// Insert a vote, rejecting duplicate signers for the same exact attestation body.
    ///
    /// # Errors
    /// Returns [`NativeAmxSessionError::DuplicateSigner`] when a signer votes twice for one body,
    /// [`NativeAmxSessionError::PlanEquivocation`] for a conflicting source-plan claim, or
    /// [`NativeAmxSessionError::Capacity`] instead of evicting a safety-relevant claim.
    pub fn insert_vote(&mut self, vote: NativeAmxVoteV2) -> Result<(), NativeAmxSessionError> {
        let key = NativeAmxSessionKey::from_body(&vote.body);
        if self
            .source_plan_claims
            .get(&key.source_id)
            .is_some_and(|claimed| *claimed != key.plan_digest)
        {
            return Err(NativeAmxSessionError::PlanEquivocation);
        }
        if !self.sessions.contains_key(&key) {
            if self.sessions.len() >= self.max_sessions.get() {
                return Err(NativeAmxSessionError::Capacity);
            }
            self.source_plan_claims
                .insert(key.source_id, key.plan_digest);
        }
        self.sessions
            .entry(key)
            .or_default()
            .insert_vote(vote, self.max_body_buckets_per_session)
    }

    /// Return votes sorted deterministically by signer id for a session phase.
    #[must_use]
    pub fn sorted_votes(
        &self,
        key: NativeAmxSessionKey,
        phase: NativeAmxPhase,
    ) -> Vec<NativeAmxVoteV2> {
        self.sessions
            .get(&key)
            .map(|session| {
                session
                    .votes
                    .iter()
                    .filter(|(bucket, _)| bucket.body.phase == phase)
                    .flat_map(|(_, votes)| votes.values().cloned())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Return votes sorted deterministically by signer id for an exact participant body.
    #[must_use]
    pub fn sorted_votes_for_body(
        &self,
        key: NativeAmxSessionKey,
        body: &NativeAmxAttestationBodyV2,
    ) -> Vec<NativeAmxVoteV2> {
        self.sessions
            .get(&key)
            .map(|session| session.votes_for_body(body))
            .unwrap_or_default()
    }

    /// Return exact-body votes restricted to the validator set used for QC assembly.
    #[must_use]
    pub fn sorted_votes_for_body_from(
        &self,
        key: NativeAmxSessionKey,
        body: &NativeAmxAttestationBodyV2,
        validator_set: &[PeerId],
    ) -> Vec<NativeAmxVoteV2> {
        self.sorted_votes_for_body(key, body)
            .into_iter()
            .filter(|vote| {
                validator_set
                    .iter()
                    .any(|validator| validator == &vote.signer)
            })
            .collect()
    }

    /// Return whether retained Native AMX vote evidence names an exact lane
    /// incarnation as coordinator or participant.
    #[must_use]
    pub(crate) fn has_pending_votes_for_lane(
        &self,
        lane_id: iroha_data_model::nexus::LaneId,
        dataspace_id: iroha_data_model::nexus::DataSpaceId,
        lane_incarnation: Hash,
    ) -> bool {
        self.sessions.values().any(|session| {
            session.votes.keys().any(|bucket| {
                let body = &bucket.body;
                (body.coordinator_lane_id == lane_id
                    && body.coordinator_dataspace_id == dataspace_id
                    && body.coordinator_lane_incarnation == lane_incarnation)
                    || (body.participant_lane_id == lane_id
                        && body.participant_dataspace_id == dataspace_id
                        && body.participant_lane_incarnation == lane_incarnation)
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroUsize;

    use iroha_crypto::{Algorithm, KeyPair, Signature};
    use iroha_data_model::{
        block::{
            consensus::NativeAmxAttestationBodyV2,
            consensus_v2::{ConsensusRound, HeightContext, HeightContextId},
        },
        nexus::{DataSpaceId, LaneId},
        peer::PeerId,
        transaction::TransactionEntrypoint,
    };

    use super::*;

    fn checked_random_ed25519_keypair() -> KeyPair {
        KeyPair::try_random_with_algorithm(Algorithm::Ed25519)
            .expect("generate checked native AMX fixture keypair")
    }

    fn checked_bls_keypair(seed: u8) -> KeyPair {
        KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
            .expect("generate checked native AMX BLS fixture keypair")
    }

    fn checked_bls_signature_payload(keypair: &KeyPair, message: &[u8]) -> Vec<u8> {
        let signature = Signature::try_new(keypair.private_key(), message)
            .expect("checked native AMX vote fixture signature");
        signature
            .verify(keypair.public_key(), message)
            .expect("checked native AMX vote fixture signature verifies");
        signature.payload().to_vec()
    }

    fn body(phase: NativeAmxPhase) -> NativeAmxAttestationBodyV2 {
        let mut body = NativeAmxAttestationBodyV2 {
            round: ConsensusRound {
                context_id: HeightContextId(HashOf::<HeightContext>::from_untyped_unchecked(
                    Hash::new(b"native-amx-v2-test-context"),
                )),
                height: 42,
                view: 3,
            },
            epoch: 7,
            chain_id_hash: Hash::new(b"native-amx-v2-test-chain"),
            source_id: [0xCD; iroha_crypto::Hash::LENGTH],
            tx_entrypoint_hash:
                iroha_crypto::HashOf::<TransactionEntrypoint>::from_untyped_unchecked(
                    Hash::prehashed([0xCD; iroha_crypto::Hash::LENGTH]),
                ),
            plan_digest: Hash::new(b"native-amx-plan"),
            phase,
            coordinator_lane_id: LaneId::new(1),
            coordinator_dataspace_id: DataSpaceId::new(7),
            coordinator_lane_incarnation: Hash::new(b"native-amx-v2-coordinator-incarnation"),
            participant_lane_id: LaneId::new(2),
            participant_dataspace_id: DataSpaceId::new(8),
            participant_lane_incarnation: Hash::new(b"native-amx-v2-participant-incarnation"),
            participant_previous_block_height: 0,
            participant_previous_block_descriptor_hash: None,
            participant_lane_block_height: 1,
            participant_lane_block_view: 0,
            participant_proposal_hash: Hash::new(b"native-amx-v2-participant-proposal"),
            participant_settlement_commitment: Hash::prehashed([0; Hash::LENGTH]),
            participant_validator_set_hash: HashOf::new(&Vec::<PeerId>::new()),
            participant_validator_count: 1,
            participant_min_quorum: 1,
            authority_context_height: 42,
            planned_coordinator_block_height: 42,
            coordinator_lane_block_view: 3,
            coordinator_proposal_hash: Hash::new(b"native-amx-v2-coordinator-proposal"),
        };
        body.participant_settlement_commitment = body
            .computed_grouped_participant_settlement_commitment(&[body.source_id])
            .expect("single-source test fixture settlement is valid");
        body
    }

    fn signing_guard_signer(seed: u8) -> (KeyPair, PeerId) {
        let keypair = checked_bls_keypair(seed);
        let signer = PeerId::new(keypair.public_key().clone());
        (keypair, signer)
    }

    fn signing_guard_capacity(value: usize) -> NonZeroUsize {
        NonZeroUsize::new(value).expect("test signing capacity is non-zero")
    }

    fn signing_guard_limits(max_records: usize) -> NativeAmxSigningGuardLimits {
        NativeAmxSigningGuardLimits::new(
            signing_guard_capacity(max_records),
            iroha_config::parameters::defaults::sumeragi::V2_NATIVE_AMX_SIGNING_GUARD_RECORD_BYTES,
            iroha_config::parameters::defaults::sumeragi::V2_NATIVE_AMX_SIGNING_GUARD_ANCHOR_BYTES,
        )
        .expect("test signing guard limits are valid")
    }

    #[test]
    fn signing_journal_identity_ignores_ambient_norito_layout() {
        let (_, signer) = signing_guard_signer(0x6D);
        let body = body(NativeAmxPhase::Prepare);
        let binding = NativeAmxHeightBindingV2 {
            active_height: body.authority_context_height,
            context_id: body.round.context_id,
            epoch: body.epoch,
            chain_id_hash: body.chain_id_hash,
            signer: signer.clone(),
            max_records: 8,
        };
        let genesis_head = binding
            .genesis_head()
            .expect("derive canonical genesis head");
        let record = NativeAmxSigningRecordV2::from_body(1, genesis_head, &body, &signer)
            .expect("derive canonical signing record");
        let body_digest = record
            .computed_body_digest()
            .expect("derive canonical body digest");
        let record_hash = record
            .computed_record_hash()
            .expect("derive canonical record hash");
        let canonical_record =
            norito::encode_canonical(&record).expect("encode canonical signing record");

        let alternate_flags =
            norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
        let _alternate = norito::core::DecodeFlagsGuard::enter(alternate_flags);
        assert_ne!(
            norito::to_bytes(&record).expect("encode alternate-layout signing record"),
            canonical_record,
            "fixture must exercise a distinct ambient Norito layout"
        );
        assert_eq!(
            binding
                .genesis_head()
                .expect("derive genesis head under alternate layout"),
            genesis_head
        );
        assert_eq!(
            record
                .computed_body_digest()
                .expect("derive body digest under alternate layout"),
            body_digest
        );
        assert_eq!(
            record
                .computed_record_hash()
                .expect("derive record hash under alternate layout"),
            record_hash
        );
    }

    #[cfg(unix)]
    fn open_signing_guard(
        root: &Path,
        body: &NativeAmxAttestationBodyV2,
        signer: PeerId,
        max_records: usize,
    ) -> Result<NativeAmxSigningGuard, NativeAmxSigningGuardError> {
        NativeAmxSigningGuard::open(
            root,
            body.authority_context_height,
            body.round.context_id,
            body.epoch,
            body.chain_id_hash,
            signer,
            signing_guard_limits(max_records),
        )
    }

    #[cfg(unix)]
    fn signing_record_paths(guard: &NativeAmxSigningGuard) -> Vec<PathBuf> {
        let mut paths = fs::read_dir(&guard.directory)
            .expect("read signer journal")
            .map(|entry| entry.expect("journal entry"))
            .filter(|entry| native_amx_valid_record_filename(&entry.file_name().to_string_lossy()))
            .map(|entry| entry.path())
            .collect::<Vec<_>>();
        paths.sort();
        paths
    }

    #[cfg(unix)]
    fn write_secure_new(path: &Path, bytes: &[u8]) {
        let mut options = OpenOptions::new();
        options
            .create_new(true)
            .write(true)
            .mode(NATIVE_AMX_SIGNING_FILE_MODE);
        let mut file = options.open(path).expect("create secure test record");
        file.write_all(bytes).expect("write secure test record");
        file.sync_all().expect("sync secure test record");
        fs::set_permissions(
            path,
            fs::Permissions::from_mode(NATIVE_AMX_SIGNING_FILE_MODE),
        )
        .expect("set secure test record mode");
    }

    fn another_context(label: &[u8]) -> HeightContextId {
        HeightContextId(HashOf::<HeightContext>::from_untyped_unchecked(Hash::new(
            label,
        )))
    }

    #[test]
    fn participant_leg_cap_reserves_one_slot_for_the_coordinator() {
        assert_eq!(
            MAX_NATIVE_AMX_PARTICIPANT_LEGS + 1,
            MAX_NATIVE_AMX_PLAN_LEGS
        );
        assert!(native_amx_participant_leg_count_within_limit(0));
        assert!(native_amx_participant_leg_count_within_limit(
            MAX_NATIVE_AMX_PARTICIPANT_LEGS
        ));
        assert!(!native_amx_participant_leg_count_within_limit(
            MAX_NATIVE_AMX_PLAN_LEGS
        ));
    }

    #[cfg(not(unix))]
    #[test]
    fn signing_guard_fails_closed_on_unsupported_filesystems() {
        let body = body(NativeAmxPhase::Prepare);
        let (_keypair, signer) = signing_guard_signer(0x70);
        assert!(matches!(
            NativeAmxSigningGuard::open(
                Path::new("."),
                body.authority_context_height,
                body.round.context_id,
                body.epoch,
                body.chain_id_hash,
                signer,
                signing_guard_limits(8),
            ),
            Err(NativeAmxSigningGuardError::UnsupportedPlatform)
        ));
    }

    #[cfg(unix)]
    #[test]
    fn signing_guard_rejects_legacy_signer_journal_instead_of_ignoring_it() {
        let root = tempfile::tempdir().expect("temp dir");
        let body = body(NativeAmxPhase::Prepare);
        let (_keypair, signer) = signing_guard_signer(0x6F);
        let owner_uid = native_amx_effective_user_id(root.path()).expect("effective uid");
        let signer_digest =
            native_amx_signer_directory_digest(root.path(), &signer).expect("signer digest");
        let legacy_root = root.path().join("native-amx-v2-signing-guard-v3");
        native_amx_ensure_secure_directory(&legacy_root, owner_uid)
            .expect("create secure legacy root");
        native_amx_ensure_secure_directory(&legacy_root.join(signer_digest.to_string()), owner_uid)
            .expect("create secure legacy signer journal");

        assert!(matches!(
            open_signing_guard(root.path(), &body, signer, 8),
            Err(NativeAmxSigningGuardError::UnsafeJournal(message))
                if message.contains("authenticated recovery")
        ));
        assert!(
            !root
                .path()
                .join(NATIVE_AMX_SIGNING_GUARD_DIRECTORY)
                .exists()
        );
    }

    #[cfg(unix)]
    #[test]
    fn signing_guard_is_restart_safe_idempotent_and_rejects_body_equivocation() {
        let root = tempfile::tempdir().expect("temp dir");
        let (_keypair, signer) = signing_guard_signer(0x71);
        let body = body(NativeAmxPhase::Prepare);
        let guard =
            open_signing_guard(root.path(), &body, signer.clone(), 8).expect("open signing guard");
        guard.record(&body).expect("record first body");
        guard.record(&body).expect("exact replay is idempotent");

        let mut conflict = body;
        conflict.coordinator_proposal_hash = Hash::new(b"conflicting coordinator proposal");
        assert_eq!(
            guard.record(&conflict),
            Err(NativeAmxSigningGuardError::Equivocation)
        );
        drop(guard);

        let restarted =
            open_signing_guard(root.path(), &body, signer, 8).expect("restart signing guard");
        restarted.record(&body).expect("restart exact replay");
        assert_eq!(
            restarted.record(&conflict),
            Err(NativeAmxSigningGuardError::Equivocation)
        );
    }

    #[cfg(unix)]
    #[test]
    fn signing_guard_durably_binds_full_source_session_and_participant_incarnation() {
        let root = tempfile::tempdir().expect("temp dir");
        let (_keypair, signer) = signing_guard_signer(0x6E);
        let base = body(NativeAmxPhase::Prepare);
        let guard =
            open_signing_guard(root.path(), &base, signer.clone(), 32).expect("open signing guard");
        guard.record(&base).expect("record source-session claim");
        drop(guard);

        let mut drifts = Vec::new();

        let mut entrypoint = base;
        entrypoint.phase = NativeAmxPhase::Commit;
        entrypoint.tx_entrypoint_hash = HashOf::<TransactionEntrypoint>::from_untyped_unchecked(
            Hash::new(b"source-entrypoint-drift"),
        );
        drifts.push(entrypoint);

        let mut global_view = base;
        global_view.phase = NativeAmxPhase::Commit;
        global_view.round.view = global_view.round.view.saturating_add(1);
        drifts.push(global_view);

        let mut coordinator_route = base;
        coordinator_route.phase = NativeAmxPhase::Commit;
        coordinator_route.coordinator_lane_id = LaneId::new(9);
        drifts.push(coordinator_route);

        let mut coordinator_incarnation = base;
        coordinator_incarnation.phase = NativeAmxPhase::Commit;
        coordinator_incarnation.coordinator_lane_incarnation =
            Hash::new(b"coordinator-incarnation-drift");
        drifts.push(coordinator_incarnation);

        let mut planned_height = base;
        planned_height.phase = NativeAmxPhase::Commit;
        planned_height.planned_coordinator_block_height = planned_height
            .planned_coordinator_block_height
            .saturating_add(1);
        drifts.push(planned_height);

        let mut coordinator_view = base;
        coordinator_view.phase = NativeAmxPhase::Commit;
        coordinator_view.coordinator_lane_block_view = coordinator_view
            .coordinator_lane_block_view
            .saturating_add(1);
        drifts.push(coordinator_view);

        let mut coordinator_proposal = base;
        coordinator_proposal.phase = NativeAmxPhase::Commit;
        coordinator_proposal.coordinator_proposal_hash = Hash::new(b"coordinator-proposal-drift");
        drifts.push(coordinator_proposal);

        let mut participant_incarnation = base;
        participant_incarnation.phase = NativeAmxPhase::Commit;
        participant_incarnation.participant_lane_incarnation =
            Hash::new(b"participant-incarnation-drift");
        drifts.push(participant_incarnation);

        for drift in drifts {
            let restarted = open_signing_guard(root.path(), &base, signer.clone(), 32)
                .expect("restart signing guard");
            assert_eq!(
                restarted.record(&drift),
                Err(NativeAmxSigningGuardError::PlanEquivocation)
            );
        }

        let mut second_participant = base;
        second_participant.participant_lane_id = LaneId::new(3);
        second_participant.participant_dataspace_id = DataSpaceId::new(9);
        second_participant.participant_lane_incarnation =
            Hash::new(b"second-planned-participant-incarnation");
        second_participant.participant_proposal_hash =
            Hash::new(b"second-planned-participant-proposal");
        second_participant.participant_settlement_commitment = second_participant
            .computed_grouped_participant_settlement_commitment(&[second_participant.source_id])
            .expect("single-source test fixture settlement is valid");
        let restarted =
            open_signing_guard(root.path(), &base, signer, 32).expect("restart signing guard");
        restarted
            .record(&second_participant)
            .expect("same source may bind another planned participant route");
    }

    #[cfg(unix)]
    #[test]
    fn signing_guard_durably_rejects_same_source_plan_equivocation_across_views() {
        let root = tempfile::tempdir().expect("temp dir");
        let (_keypair, signer) = signing_guard_signer(0x72);
        let body = body(NativeAmxPhase::Prepare);
        let guard =
            open_signing_guard(root.path(), &body, signer.clone(), 8).expect("open signing guard");
        guard.record(&body).expect("record source-plan claim");

        let mut conflicting_plan = body;
        conflicting_plan.round.view += 1;
        conflicting_plan.coordinator_lane_block_view += 1;
        conflicting_plan.plan_digest = Hash::new(b"conflicting durable native AMX plan");
        assert_eq!(
            guard.record(&conflicting_plan),
            Err(NativeAmxSigningGuardError::PlanEquivocation)
        );
        drop(guard);

        conflicting_plan.round.view += 1;
        conflicting_plan.coordinator_lane_block_view += 1;
        let restarted =
            open_signing_guard(root.path(), &body, signer, 8).expect("restart signing guard");
        assert_eq!(
            restarted.record(&conflicting_plan),
            Err(NativeAmxSigningGuardError::PlanEquivocation)
        );
    }

    #[cfg(unix)]
    #[test]
    fn signing_guard_durably_rejects_participant_slot_aba_across_sources_and_views() {
        let root = tempfile::tempdir().expect("temp dir");
        let (_keypair, signer) = signing_guard_signer(0x8D);
        let first = body(NativeAmxPhase::Prepare);
        let guard =
            open_signing_guard(root.path(), &first, signer.clone(), 8).expect("open signing guard");
        guard.record(&first).expect("record first slot claim");

        let mut conflicting_proposal = first;
        conflicting_proposal.round.view += 1;
        conflicting_proposal.participant_proposal_hash = Hash::new(b"slot-conflicting proposal");
        assert_eq!(
            guard.record(&conflicting_proposal),
            Err(NativeAmxSigningGuardError::SlotEquivocation)
        );

        let mut conflicting = first;
        conflicting.round.view += 1;
        conflicting.source_id = [0xEF; Hash::LENGTH];
        conflicting.tx_entrypoint_hash = HashOf::<TransactionEntrypoint>::from_untyped_unchecked(
            Hash::prehashed(conflicting.source_id),
        );
        conflicting.participant_settlement_commitment = conflicting
            .computed_grouped_participant_settlement_commitment(&[conflicting.source_id])
            .expect("single-source test fixture settlement is valid");
        assert_eq!(
            guard.record(&conflicting),
            Err(NativeAmxSigningGuardError::SlotEquivocation)
        );
        drop(guard);

        conflicting.round.view += 1;
        let restarted =
            open_signing_guard(root.path(), &first, signer, 8).expect("restart signing guard");
        assert_eq!(
            restarted.record(&conflicting),
            Err(NativeAmxSigningGuardError::SlotEquivocation)
        );
    }

    #[cfg(unix)]
    #[test]
    fn signing_guard_binds_context_epoch_and_monotonic_view_then_resets_next_height() {
        let root = tempfile::tempdir().expect("temp dir");
        let (_keypair, signer) = signing_guard_signer(0x73);
        let base = body(NativeAmxPhase::Prepare);
        let guard =
            open_signing_guard(root.path(), &base, signer.clone(), 8).expect("open signing guard");
        guard.record(&base).expect("record base view");
        let mut high = base;
        high.source_id = [0xA1; Hash::LENGTH];
        high.tx_entrypoint_hash = HashOf::<TransactionEntrypoint>::from_untyped_unchecked(
            Hash::prehashed([0xA1; Hash::LENGTH]),
        );
        high.round.view += 2;
        high.coordinator_lane_block_view += 2;
        guard.record(&high).expect("advance durable view");
        drop(guard);

        assert!(matches!(
            NativeAmxSigningGuard::open(
                root.path(),
                base.authority_context_height,
                another_context(b"same-height-context-drift"),
                base.epoch,
                base.chain_id_hash,
                signer.clone(),
                signing_guard_limits(8),
            ),
            Err(NativeAmxSigningGuardError::ContextMismatch)
        ));
        assert!(matches!(
            NativeAmxSigningGuard::open(
                root.path(),
                base.authority_context_height,
                base.round.context_id,
                base.epoch + 1,
                base.chain_id_hash,
                signer.clone(),
                signing_guard_limits(8),
            ),
            Err(NativeAmxSigningGuardError::ContextMismatch)
        ));

        let restarted = open_signing_guard(root.path(), &base, signer.clone(), 8)
            .expect("restart exact context");
        let mut stale = base;
        stale.source_id = [0xA2; Hash::LENGTH];
        stale.tx_entrypoint_hash = HashOf::<TransactionEntrypoint>::from_untyped_unchecked(
            Hash::prehashed([0xA2; Hash::LENGTH]),
        );
        stale.round.view += 1;
        stale.coordinator_lane_block_view += 1;
        assert_eq!(
            restarted.record(&stale),
            Err(NativeAmxSigningGuardError::StaleView {
                attempted_view: base.round.view + 1,
                highest_view: base.round.view + 2,
            })
        );
        drop(restarted);

        let next_context = another_context(b"next-height-context");
        let next_guard = NativeAmxSigningGuard::open(
            root.path(),
            base.authority_context_height + 1,
            next_context,
            base.epoch,
            base.chain_id_hash,
            signer.clone(),
            signing_guard_limits(8),
        )
        .expect("advance exact next height");
        let mut next = base;
        next.round.height += 1;
        next.round.context_id = next_context;
        next.round.view = 0;
        next.coordinator_lane_block_view = 0;
        next.authority_context_height += 1;
        next.planned_coordinator_block_height += 1;
        next_guard
            .record(&next)
            .expect("view high-water resets at next height");
        drop(next_guard);

        assert!(matches!(
            open_signing_guard(root.path(), &next, signer.clone(), 16),
            Err(NativeAmxSigningGuardError::ContextMismatch)
        ));

        assert!(matches!(
            open_signing_guard(root.path(), &base, signer, 8),
            Err(NativeAmxSigningGuardError::HeightRegression { .. })
        ));
    }

    #[cfg(unix)]
    #[test]
    fn signing_guard_detects_plain_deletion_of_anchored_record() {
        let root = tempfile::tempdir().expect("temp dir");
        let (_keypair, signer) = signing_guard_signer(0x74);
        let body = body(NativeAmxPhase::Prepare);
        let guard =
            open_signing_guard(root.path(), &body, signer.clone(), 8).expect("open signing guard");
        guard.record(&body).expect("record anchored body");
        let path = signing_record_paths(&guard)
            .into_iter()
            .next()
            .expect("anchored record");
        drop(guard);
        fs::remove_file(&path).expect("delete anchored record");

        assert!(matches!(
            open_signing_guard(root.path(), &body, signer, 8),
            Err(NativeAmxSigningGuardError::UnsafeJournal(_))
        ));
    }

    #[cfg(unix)]
    #[test]
    fn signing_guard_detects_live_anchor_and_record_deletion_before_another_append() {
        for delete_anchor in [false, true] {
            let root = tempfile::tempdir().expect("temp dir");
            let (_keypair, signer) = signing_guard_signer(if delete_anchor { 0x87 } else { 0x86 });
            let first = body(NativeAmxPhase::Prepare);
            let guard =
                open_signing_guard(root.path(), &first, signer, 8).expect("open signing guard");
            guard.record(&first).expect("record anchored body");
            let deleted_path = if delete_anchor {
                NativeAmxSigningGuard::anchor_path(&guard.directory)
            } else {
                signing_record_paths(&guard)
                    .into_iter()
                    .next()
                    .expect("anchored record")
            };
            fs::remove_file(&deleted_path).expect("delete live retained journal path");

            let mut second = first;
            second.source_id = [0xD1; Hash::LENGTH];
            second.tx_entrypoint_hash = HashOf::<TransactionEntrypoint>::from_untyped_unchecked(
                Hash::prehashed([0xD1; Hash::LENGTH]),
            );
            assert!(matches!(
                guard.record(&second),
                Err(NativeAmxSigningGuardError::UnsafeJournal(_))
            ));
            assert!(matches!(
                guard.record(&second),
                Err(NativeAmxSigningGuardError::Poisoned(_))
            ));
        }
    }

    #[cfg(unix)]
    #[test]
    fn signing_guard_detects_live_anchor_and_record_replacement_before_another_append() {
        for replace_anchor in [false, true] {
            let root = tempfile::tempdir().expect("temp dir");
            let (_keypair, signer) = signing_guard_signer(if replace_anchor { 0x89 } else { 0x88 });
            let first = body(NativeAmxPhase::Prepare);
            let guard =
                open_signing_guard(root.path(), &first, signer, 8).expect("open signing guard");
            guard.record(&first).expect("record anchored body");
            let replaced_path = if replace_anchor {
                NativeAmxSigningGuard::anchor_path(&guard.directory)
            } else {
                signing_record_paths(&guard)
                    .into_iter()
                    .next()
                    .expect("anchored record")
            };
            let bytes = fs::read(&replaced_path).expect("read retained journal path");
            let replacement = guard.directory.join(if replace_anchor {
                "replacement-anchor"
            } else {
                "replacement-record"
            });
            write_secure_new(&replacement, &bytes);
            fs::rename(&replacement, &replaced_path).expect("replace retained journal path");

            let mut second = first;
            second.source_id = [0xD2; Hash::LENGTH];
            second.tx_entrypoint_hash = HashOf::<TransactionEntrypoint>::from_untyped_unchecked(
                Hash::prehashed([0xD2; Hash::LENGTH]),
            );
            assert!(matches!(
                guard.record(&second),
                Err(NativeAmxSigningGuardError::UnsafeJournal(_))
            ));
        }
    }

    #[cfg(unix)]
    #[test]
    fn signing_guard_rejects_anchor_deletion_when_records_remain() {
        let root = tempfile::tempdir().expect("temp dir");
        let (_keypair, signer) = signing_guard_signer(0x83);
        let body = body(NativeAmxPhase::Prepare);
        let guard =
            open_signing_guard(root.path(), &body, signer.clone(), 8).expect("open signing guard");
        guard.record(&body).expect("record anchored body");
        let anchor_path = NativeAmxSigningGuard::anchor_path(&guard.directory);
        drop(guard);
        fs::remove_file(anchor_path).expect("delete chain anchor");

        assert!(matches!(
            open_signing_guard(root.path(), &body, signer, 8),
            Err(NativeAmxSigningGuardError::UnsafeJournal(_))
        ));
    }

    #[cfg(unix)]
    #[test]
    fn signing_guard_detects_hardlink_move_of_anchored_record() {
        let root = tempfile::tempdir().expect("temp dir");
        let (_keypair, signer) = signing_guard_signer(0x75);
        let body = body(NativeAmxPhase::Prepare);
        let guard =
            open_signing_guard(root.path(), &body, signer.clone(), 8).expect("open signing guard");
        guard.record(&body).expect("record anchored body");
        let path = signing_record_paths(&guard)
            .into_iter()
            .next()
            .expect("anchored record");
        drop(guard);
        let escaped = root.path().join("escaped-record.norito");
        fs::hard_link(&path, &escaped).expect("hardlink record outside signer journal");
        fs::remove_file(&path).expect("unlink anchored journal path");

        assert!(matches!(
            open_signing_guard(root.path(), &body, signer, 8),
            Err(NativeAmxSigningGuardError::UnsafeJournal(_))
        ));
    }

    #[cfg(unix)]
    #[test]
    fn signing_guard_rejects_changed_noncanonical_and_hardlinked_records() {
        let root = tempfile::tempdir().expect("temp dir");
        let (_keypair, signer) = signing_guard_signer(0x76);
        let body = body(NativeAmxPhase::Prepare);
        let guard =
            open_signing_guard(root.path(), &body, signer.clone(), 8).expect("open signing guard");
        guard.record(&body).expect("record anchored body");
        let path = signing_record_paths(&guard)
            .into_iter()
            .next()
            .expect("anchored record");
        drop(guard);
        let bytes = fs::read(&path).expect("read record");
        let record =
            norito::decode_from_bytes::<NativeAmxSigningRecordV2>(&bytes).expect("decode record");
        fs::write(&path, record.encode()).expect("replace with bare Norito");
        assert!(matches!(
            open_signing_guard(root.path(), &body, signer.clone(), 8),
            Err(NativeAmxSigningGuardError::UnsafeJournal(_))
        ));

        fs::write(&path, bytes).expect("restore framed record");
        let escaped = root.path().join("record-hardlink");
        fs::hard_link(&path, &escaped).expect("create hardlink");
        assert!(matches!(
            open_signing_guard(root.path(), &body, signer, 8),
            Err(NativeAmxSigningGuardError::UnsafeJournal(_))
        ));
    }

    #[cfg(unix)]
    #[test]
    fn signing_guard_reconciles_only_one_unpublished_tail() {
        let root = tempfile::tempdir().expect("temp dir");
        let (_keypair, signer) = signing_guard_signer(0x77);
        let base = body(NativeAmxPhase::Prepare);
        let guard =
            open_signing_guard(root.path(), &base, signer.clone(), 8).expect("open signing guard");
        guard.record(&base).expect("record anchored base");
        let anchor = guard.inner.lock().anchor.clone();
        let mut tail_body = base;
        tail_body.source_id = [0xB1; Hash::LENGTH];
        tail_body.tx_entrypoint_hash = HashOf::<TransactionEntrypoint>::from_untyped_unchecked(
            Hash::prehashed([0xB1; Hash::LENGTH]),
        );
        let tail = NativeAmxSigningRecordV2::from_body(
            anchor.record_count + 1,
            anchor.head_hash,
            &tail_body,
            &signer,
        )
        .expect("build unpublished tail");
        let tail_path = NativeAmxSigningGuard::record_path(&guard.directory, &tail);
        write_secure_new(
            &tail_path,
            &norito::to_bytes(&tail).expect("encode unpublished tail"),
        );
        drop(guard);

        let restarted = open_signing_guard(root.path(), &base, signer, 8)
            .expect("reconcile one unpublished tail");
        assert!(!tail_path.exists());
        assert_eq!(restarted.inner.lock().anchor.record_count, 1);
    }

    #[cfg(unix)]
    #[test]
    fn signing_guard_discards_crash_left_anchor_temp_without_losing_committed_head() {
        let root = tempfile::tempdir().expect("temp dir");
        let (_keypair, signer) = signing_guard_signer(0x8B);
        let base = body(NativeAmxPhase::Prepare);
        let guard =
            open_signing_guard(root.path(), &base, signer.clone(), 8).expect("open signing guard");
        guard.record(&base).expect("record anchored body");
        let committed_anchor = guard.inner.lock().anchor.clone();
        let temp_path = NativeAmxSigningGuard::anchor_temp_path(&guard.directory);
        write_secure_new(
            &temp_path,
            &norito::to_bytes(&committed_anchor).expect("encode crash-left anchor temp"),
        );
        drop(guard);

        let restarted = open_signing_guard(root.path(), &base, signer, 8)
            .expect("reconcile crash-left anchor temp");
        assert!(!temp_path.exists());
        assert_eq!(restarted.inner.lock().anchor, committed_anchor);
        restarted
            .record(&base)
            .expect("committed head remains replayable");
    }

    #[cfg(unix)]
    #[test]
    fn signing_guard_finishes_height_transition_after_anchor_publish_crash() {
        let root = tempfile::tempdir().expect("temp dir");
        let (_keypair, signer) = signing_guard_signer(0x8C);
        let base = body(NativeAmxPhase::Prepare);
        let guard =
            open_signing_guard(root.path(), &base, signer.clone(), 8).expect("open signing guard");
        guard.record(&base).expect("record old-height body");
        let old_record = signing_record_paths(&guard)
            .into_iter()
            .next()
            .expect("old-height record");

        let next_context = another_context(b"crash-boundary-next-height-context");
        let next_binding = NativeAmxHeightBindingV2 {
            active_height: base.authority_context_height + 1,
            context_id: next_context,
            epoch: base.epoch,
            chain_id_hash: base.chain_id_hash,
            signer: signer.clone(),
            max_records: 8,
        };
        let next_anchor =
            NativeAmxSigningAnchorV2::empty(next_binding).expect("build next-height empty anchor");
        NativeAmxSigningGuard::persist_anchor(
            &guard.directory,
            &guard.directory_handle,
            guard.owner_uid,
            &next_anchor,
            guard.limits.max_anchor_bytes.get(),
        )
        .expect("publish next-height anchor before simulated crash");
        drop(guard);

        let mut next = base;
        next.round.height += 1;
        next.round.context_id = next_context;
        next.round.view = 0;
        next.coordinator_lane_block_view = 0;
        next.authority_context_height += 1;
        next.planned_coordinator_block_height += 1;
        let restarted = open_signing_guard(root.path(), &next, signer, 8)
            .expect("finish stale-record cleanup after anchor publication crash");
        assert!(!old_record.exists());
        restarted.record(&next).expect("sign at recovered height");
    }

    #[cfg(unix)]
    #[test]
    fn signing_guard_rejects_multiple_unpublished_tails() {
        let root = tempfile::tempdir().expect("temp dir");
        let (_keypair, signer) = signing_guard_signer(0x78);
        let base = body(NativeAmxPhase::Prepare);
        let guard =
            open_signing_guard(root.path(), &base, signer.clone(), 8).expect("open signing guard");
        let anchor = guard.inner.lock().anchor.clone();

        let mut first_body = base;
        first_body.source_id = [0xB2; Hash::LENGTH];
        first_body.tx_entrypoint_hash = HashOf::<TransactionEntrypoint>::from_untyped_unchecked(
            Hash::prehashed([0xB2; Hash::LENGTH]),
        );
        let first = NativeAmxSigningRecordV2::from_body(1, anchor.head_hash, &first_body, &signer)
            .expect("first tail");
        let mut second_body = base;
        second_body.source_id = [0xB3; Hash::LENGTH];
        second_body.tx_entrypoint_hash = HashOf::<TransactionEntrypoint>::from_untyped_unchecked(
            Hash::prehashed([0xB3; Hash::LENGTH]),
        );
        let second =
            NativeAmxSigningRecordV2::from_body(2, first.record_hash, &second_body, &signer)
                .expect("second tail");
        for record in [&first, &second] {
            let path = NativeAmxSigningGuard::record_path(&guard.directory, record);
            write_secure_new(&path, &norito::to_bytes(record).expect("encode tail"));
        }
        drop(guard);

        assert!(matches!(
            open_signing_guard(root.path(), &base, signer, 8),
            Err(NativeAmxSigningGuardError::UnsafeJournal(_))
        ));
    }

    #[cfg(unix)]
    #[test]
    fn signing_guard_latches_poison_after_lock_path_deletion() {
        let root = tempfile::tempdir().expect("temp dir");
        let (_keypair, signer) = signing_guard_signer(0x79);
        let body = body(NativeAmxPhase::Prepare);
        let guard = open_signing_guard(root.path(), &body, signer, 8).expect("open signing guard");
        fs::remove_file(&guard.lock_path).expect("delete retained lock path");

        assert!(matches!(
            guard.record(&body),
            Err(NativeAmxSigningGuardError::UnsafeJournal(_))
        ));
        assert!(matches!(
            guard.record(&body),
            Err(NativeAmxSigningGuardError::Poisoned(_))
        ));
    }

    #[cfg(unix)]
    #[test]
    fn signing_guard_latches_poison_after_directory_or_lock_replacement() {
        for replace_directory in [false, true] {
            let root = tempfile::tempdir().expect("temp dir");
            let (_keypair, signer) =
                signing_guard_signer(if replace_directory { 0x8C } else { 0x8B });
            let body = body(NativeAmxPhase::Prepare);
            let guard =
                open_signing_guard(root.path(), &body, signer, 8).expect("open signing guard");

            if replace_directory {
                let moved = root.path().join("moved-signer-directory");
                fs::rename(&guard.directory, moved).expect("move retained signer directory");
                let mut builder = DirBuilder::new();
                builder.mode(NATIVE_AMX_SIGNING_DIRECTORY_MODE);
                builder
                    .create(&guard.directory)
                    .expect("create replacement signer directory");
            } else {
                let replacement = guard.directory.join("replacement-owner-lock");
                write_secure_new(&replacement, b"");
                fs::rename(replacement, &guard.lock_path).expect("replace owner lock path");
            }

            assert!(matches!(
                guard.record(&body),
                Err(NativeAmxSigningGuardError::UnsafeJournal(_))
            ));
            assert!(matches!(
                guard.record(&body),
                Err(NativeAmxSigningGuardError::Poisoned(_))
            ));
        }
    }

    #[cfg(unix)]
    #[test]
    fn signing_guard_rejects_malformed_context_and_view_bodies() {
        let root = tempfile::tempdir().expect("temp dir");
        let (_keypair, signer) = signing_guard_signer(0x80);
        let body = body(NativeAmxPhase::Prepare);
        let guard = open_signing_guard(root.path(), &body, signer, 8).expect("open signing guard");

        let mut foreign_context = body;
        foreign_context.round.context_id =
            HeightContextId(HashOf::<HeightContext>::from_untyped_unchecked(Hash::new(
                b"foreign-signing-guard-context",
            )));
        assert_eq!(
            guard.record(&foreign_context),
            Err(NativeAmxSigningGuardError::ContextMismatch)
        );

        let mut zero_source = body;
        zero_source.source_id = [0; Hash::LENGTH];
        assert!(matches!(
            guard.record(&zero_source),
            Err(NativeAmxSigningGuardError::InvalidInput(_))
        ));

        let mut zero_planned_height = body;
        zero_planned_height.planned_coordinator_block_height = 0;
        assert!(matches!(
            guard.record(&zero_planned_height),
            Err(NativeAmxSigningGuardError::InvalidInput(_))
        ));

        guard.record(&body).expect("record baseline body");

        let mut entrypoint_drift = body;
        entrypoint_drift.phase = NativeAmxPhase::Commit;
        entrypoint_drift.tx_entrypoint_hash =
            HashOf::<TransactionEntrypoint>::from_untyped_unchecked(Hash::new(
                b"conflicting-signing-guard-entrypoint",
            ));
        assert_eq!(
            guard.record(&entrypoint_drift),
            Err(NativeAmxSigningGuardError::PlanEquivocation)
        );

        let mut mismatched_view = body;
        mismatched_view.coordinator_lane_block_view += 1;
        assert_eq!(
            guard.record(&mismatched_view),
            Err(NativeAmxSigningGuardError::Equivocation)
        );
    }

    #[cfg(unix)]
    #[test]
    fn signing_guard_rejects_unknown_and_symlink_temps() {
        use std::os::unix::fs::symlink;

        let root = tempfile::tempdir().expect("temp dir");
        let (_keypair, signer) = signing_guard_signer(0x81);
        let body = body(NativeAmxPhase::Prepare);
        let guard =
            open_signing_guard(root.path(), &body, signer.clone(), 8).expect("open signing guard");
        guard.record(&body).expect("record body");
        let record_path = signing_record_paths(&guard)
            .into_iter()
            .next()
            .expect("record path");
        let directory = guard.directory.clone();
        drop(guard);

        let unknown = directory.join("unknown.tmp");
        write_secure_new(&unknown, b"unknown");
        assert!(matches!(
            open_signing_guard(root.path(), &body, signer.clone(), 8),
            Err(NativeAmxSigningGuardError::UnsafeJournal(_))
        ));
        fs::remove_file(&unknown).expect("remove unknown temp");

        let temp_link = record_path.with_extension(NATIVE_AMX_SIGNING_GUARD_TEMP_EXTENSION);
        symlink(&record_path, &temp_link).expect("create known-name temp symlink");
        assert!(matches!(
            open_signing_guard(root.path(), &body, signer, 8),
            Err(NativeAmxSigningGuardError::UnsafeJournal(_))
        ));
    }

    #[cfg(unix)]
    #[test]
    fn signing_guard_fails_closed_on_injected_future_record() {
        let root = tempfile::tempdir().expect("temp dir");
        let (_keypair, signer) = signing_guard_signer(0x82);
        let current = body(NativeAmxPhase::Prepare);
        let guard = open_signing_guard(root.path(), &current, signer.clone(), 8)
            .expect("open signing guard");
        let anchor = guard.inner.lock().anchor.clone();
        let mut future = current;
        future.round.height += 1;
        future.authority_context_height += 1;
        future.planned_coordinator_block_height += 1;
        let record = NativeAmxSigningRecordV2::from_body(1, anchor.head_hash, &future, &signer)
            .expect("future record");
        let path = NativeAmxSigningGuard::record_path(&guard.directory, &record);
        write_secure_new(
            &path,
            &norito::to_bytes(&record).expect("encode future record"),
        );
        drop(guard);

        assert_eq!(
            open_signing_guard(root.path(), &current, signer, 8)
                .expect_err("future record must fail closed"),
            NativeAmxSigningGuardError::FutureHeight {
                record_height: future.authority_context_height,
                active_height: current.authority_context_height,
            }
        );
    }

    #[cfg(unix)]
    #[test]
    fn signing_guard_enforces_configured_and_protocol_capacity() {
        let root = tempfile::tempdir().expect("temp dir");
        let (_keypair, signer) = signing_guard_signer(0x7A);
        let first = body(NativeAmxPhase::Prepare);
        let guard = open_signing_guard(root.path(), &first, signer.clone(), 1)
            .expect("open one-record guard");
        guard.record(&first).expect("record within capacity");
        let mut second = first;
        second.source_id = [0xCE; Hash::LENGTH];
        second.tx_entrypoint_hash = HashOf::<TransactionEntrypoint>::from_untyped_unchecked(
            Hash::prehashed([0xCE; Hash::LENGTH]),
        );
        assert_eq!(
            guard.record(&second),
            Err(NativeAmxSigningGuardError::Capacity)
        );
        drop(guard);

        let _ = signer;
        assert!(matches!(
            NativeAmxSigningGuardLimits::new(
                signing_guard_capacity(MAX_NATIVE_AMX_SIGNING_GUARD_RECORDS_HARD + 1),
                iroha_config::parameters::defaults::sumeragi::V2_NATIVE_AMX_SIGNING_GUARD_RECORD_BYTES,
                iroha_config::parameters::defaults::sumeragi::V2_NATIVE_AMX_SIGNING_GUARD_ANCHOR_BYTES,
            ),
            Err(NativeAmxSigningGuardError::InvalidInput(_))
        ));
    }

    #[cfg(unix)]
    #[test]
    fn signing_guard_uses_signer_specific_journals_for_key_rotation() {
        let root = tempfile::tempdir().expect("temp dir");
        let (_first_keypair, first_signer) = signing_guard_signer(0x7B);
        let (_second_keypair, second_signer) = signing_guard_signer(0x7C);
        let body = body(NativeAmxPhase::Prepare);
        let first = open_signing_guard(root.path(), &body, first_signer.clone(), 8)
            .expect("open first signer");
        first.record(&body).expect("record first signer body");
        let first_directory = first.directory.clone();
        drop(first);

        let second =
            open_signing_guard(root.path(), &body, second_signer, 8).expect("open rotated signer");
        second.record(&body).expect("record rotated signer body");
        assert_ne!(first_directory, second.directory);
        drop(second);

        let first_restarted = open_signing_guard(root.path(), &body, first_signer, 8)
            .expect("reopen retained first signer journal");
        let mut conflict = body;
        conflict.coordinator_proposal_hash = Hash::new(b"first signer conflict");
        assert_eq!(
            first_restarted.record(&conflict),
            Err(NativeAmxSigningGuardError::Equivocation)
        );
    }

    #[cfg(unix)]
    #[test]
    fn corrupted_retired_signer_journal_does_not_brick_rotated_signer() {
        let root = tempfile::tempdir().expect("temp dir");
        let (_first_keypair, first_signer) = signing_guard_signer(0x84);
        let (_second_keypair, second_signer) = signing_guard_signer(0x85);
        let body = body(NativeAmxPhase::Prepare);
        let first = open_signing_guard(root.path(), &body, first_signer.clone(), 8)
            .expect("open first signer");
        first.record(&body).expect("record first signer body");
        let first_record = signing_record_paths(&first)
            .into_iter()
            .next()
            .expect("first signer record");
        drop(first);
        fs::remove_file(first_record).expect("corrupt retired signer journal");

        let second = open_signing_guard(root.path(), &body, second_signer, 8)
            .expect("rotated signer remains isolated");
        second.record(&body).expect("record rotated signer body");
        drop(second);
        assert!(matches!(
            open_signing_guard(root.path(), &body, first_signer, 8),
            Err(NativeAmxSigningGuardError::UnsafeJournal(_))
        ));
    }

    #[cfg(unix)]
    #[test]
    fn signing_guard_sets_strict_directory_and_file_modes() {
        let root = tempfile::tempdir().expect("temp dir");
        let (_keypair, signer) = signing_guard_signer(0x7D);
        let body = body(NativeAmxPhase::Prepare);
        let guard = open_signing_guard(root.path(), &body, signer, 8).expect("open signing guard");
        guard.record(&body).expect("record body");
        assert_eq!(
            fs::symlink_metadata(&guard.directory)
                .expect("directory metadata")
                .mode()
                & 0o777,
            NATIVE_AMX_SIGNING_DIRECTORY_MODE
        );
        for path in signing_record_paths(&guard).into_iter().chain([
            guard.lock_path.clone(),
            NativeAmxSigningGuard::anchor_path(&guard.directory),
        ]) {
            assert_eq!(
                fs::symlink_metadata(path).expect("file metadata").mode() & 0o777,
                NATIVE_AMX_SIGNING_FILE_MODE
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn signing_guard_rejects_foreign_uid_for_every_trusted_path_class() {
        let root = tempfile::tempdir().expect("temp dir");
        let (_keypair, signer) = signing_guard_signer(0x8A);
        let body = body(NativeAmxPhase::Prepare);
        let guard = open_signing_guard(root.path(), &body, signer, 8).expect("open signing guard");
        guard.record(&body).expect("record body");
        assert_eq!(
            native_amx_effective_user_id(root.path()).expect("probe effective UID"),
            guard.owner_uid
        );
        let wrong_uid = guard.owner_uid ^ 1;

        let root_metadata = fs::symlink_metadata(root.path()).expect("store root metadata");
        assert_eq!(root_metadata.uid(), guard.owner_uid);
        assert!(matches!(
            native_amx_validate_uid(root.path(), &root_metadata, wrong_uid),
            Err(NativeAmxSigningGuardError::UnsafeJournal(_))
        ));
        let directory_metadata =
            fs::symlink_metadata(&guard.directory).expect("signer directory metadata");
        assert!(matches!(
            native_amx_validate_uid(&guard.directory, &directory_metadata, wrong_uid),
            Err(NativeAmxSigningGuardError::UnsafeJournal(_))
        ));
        for path in signing_record_paths(&guard).into_iter().chain([
            guard.lock_path.clone(),
            NativeAmxSigningGuard::anchor_path(&guard.directory),
        ]) {
            let metadata = fs::symlink_metadata(&path).expect("trusted file metadata");
            assert!(matches!(
                native_amx_validate_uid(&path, &metadata, wrong_uid),
                Err(NativeAmxSigningGuardError::UnsafeJournal(_))
            ));
        }
    }

    fn body_for_validator_set(
        phase: NativeAmxPhase,
        validator_set: &[PeerId],
    ) -> NativeAmxAttestationBodyV2 {
        let mut body = body(phase);
        body.participant_validator_set_hash = HashOf::new(&validator_set.to_vec());
        body.participant_validator_count =
            u32::try_from(validator_set.len()).expect("fixture validator count fits u32");
        body.participant_min_quorum = u32::try_from(
            crate::sumeragi::network_topology::commit_quorum_from_len(validator_set.len()).max(1),
        )
        .expect("fixture validator quorum fits u32");
        body
    }

    fn aligned_pops(validator_set: &[PeerId], keypairs: &[KeyPair]) -> Vec<Vec<u8>> {
        validator_set
            .iter()
            .map(|validator| {
                let keypair = keypairs
                    .iter()
                    .find(|keypair| keypair.public_key() == validator.public_key())
                    .expect("fixture validator has key material");
                iroha_crypto::bls_normal_pop_prove(keypair.private_key())
                    .expect("prove fixture PoP")
            })
            .collect()
    }

    fn full_plan_request(
        mut body: NativeAmxAttestationBodyV2,
        coordinator_validator_set: Vec<PeerId>,
    ) -> NativeAmxAttestationRequestV2 {
        let coordinator =
            RoutingDecision::new(body.coordinator_lane_id, body.coordinator_dataspace_id);
        let participant =
            RoutingDecision::new(body.participant_lane_id, body.participant_dataspace_id);
        let routing_plan = RoutingPlan::native_amx(
            coordinator,
            vec![RouteLeg::new(participant, RouteLegRole::Participant)],
        );
        body.plan_digest = routing_plan.digest();
        let validator_count = u32::try_from(coordinator_validator_set.len())
            .expect("fixture coordinator validator count fits u32");
        let min_quorum = u32::try_from(
            crate::sumeragi::network_topology::commit_quorum_from_len(
                coordinator_validator_set.len(),
            )
            .max(1),
        )
        .expect("fixture coordinator quorum fits u32");
        let participant_validator_set = coordinator_validator_set.clone();
        let mut descriptor = iroha_data_model::block::consensus::LaneBlockDescriptorV1 {
            lane_id: body.coordinator_lane_id,
            dataspace_id: body.coordinator_dataspace_id,
            lane_incarnation: body.coordinator_lane_incarnation,
            proposal_height: body.authority_context_height,
            previous_lane_block_height: body.planned_coordinator_block_height.saturating_sub(1),
            previous_lane_block_descriptor_hash: (body.planned_coordinator_block_height > 1)
                .then(|| Hash::new(b"native-amx-v2-test-previous-descriptor")),
            lane_block_height: body.planned_coordinator_block_height,
            lane_block_view: body.coordinator_lane_block_view,
            subject_hash: Hash::new(b"native-amx-v2-test-subject"),
            payload_ownership_hash: Hash::new(b"native-amx-v2-test-ownership"),
            rbc_instance_hash: Hash::new(b"native-amx-v2-test-rbc"),
            accepted_candidate_indices: vec![0],
            accepted_transaction_hashes: vec![Hash::from(body.tx_entrypoint_hash)],
            validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
            validator_set_hash: HashOf::new(&coordinator_validator_set),
            validator_set: coordinator_validator_set,
            validator_count,
            min_quorum,
            qc_mode_tag: "permissioned:native-amx-v2-test".to_owned(),
            descriptor_hash: Hash::prehashed([0; Hash::LENGTH]),
        };
        descriptor.descriptor_hash = descriptor.computed_descriptor_hash();
        let mut coordinator_proposal = LaneBlockProposalV1 {
            descriptor,
            proposal_hash: Hash::prehashed([0; Hash::LENGTH]),
            payload_block_hint: None,
        };
        coordinator_proposal.proposal_hash = coordinator_proposal.computed_proposal_hash();
        body.coordinator_proposal_hash = coordinator_proposal.proposal_hash;
        let mut participant_descriptor =
            iroha_data_model::block::consensus::LaneBlockDescriptorV1 {
                lane_id: body.participant_lane_id,
                dataspace_id: body.participant_dataspace_id,
                lane_incarnation: body.participant_lane_incarnation,
                proposal_height: body.authority_context_height,
                previous_lane_block_height: body.participant_previous_block_height,
                previous_lane_block_descriptor_hash: body
                    .participant_previous_block_descriptor_hash,
                lane_block_height: body.participant_lane_block_height,
                lane_block_view: body.participant_lane_block_view,
                subject_hash: Hash::new(b"native-amx-v2-test-participant-subject"),
                payload_ownership_hash: Hash::new(b"native-amx-v2-test-participant-ownership"),
                rbc_instance_hash: Hash::new(b"native-amx-v2-test-participant-rbc"),
                accepted_candidate_indices: vec![0],
                accepted_transaction_hashes: vec![Hash::from(body.tx_entrypoint_hash)],
                validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
                validator_set_hash: HashOf::new(&participant_validator_set),
                validator_set: participant_validator_set,
                validator_count: body.participant_validator_count,
                min_quorum: body.participant_min_quorum,
                qc_mode_tag: "permissioned:native-amx-v2-test".to_owned(),
                descriptor_hash: Hash::prehashed([0; Hash::LENGTH]),
            };
        participant_descriptor.descriptor_hash = participant_descriptor.computed_descriptor_hash();
        let mut participant_proposal = LaneBlockProposalV1 {
            descriptor: participant_descriptor,
            proposal_hash: Hash::prehashed([0; Hash::LENGTH]),
            payload_block_hint: None,
        };
        participant_proposal.proposal_hash = participant_proposal.computed_proposal_hash();
        body.participant_proposal_hash = participant_proposal.proposal_hash;
        let participant_settlement = body
            .computed_grouped_participant_settlement(&[body.source_id])
            .expect("single-source test fixture settlement is valid");
        body.participant_settlement_commitment = Hash::from(
            iroha_data_model::nexus::compute_settlement_hash(&participant_settlement)
                .expect("fixture participant settlement hashes"),
        );
        NativeAmxAttestationRequestV2 {
            body,
            plan_legs: routing_plan.legs(),
            coordinator_proposal,
            participant_proposal,
            participant_settlement,
        }
    }

    fn vote(phase: NativeAmxPhase) -> NativeAmxVoteV2 {
        let keypair = checked_random_ed25519_keypair();
        NativeAmxVoteV2 {
            body: body(phase),
            signer: PeerId::new(keypair.public_key().clone()),
            bls_signature: vec![0xA5; 96],
        }
    }

    #[test]
    fn session_cache_rejects_duplicate_signer() {
        let mut cache = NativeAmxSessionCache::new(NonZeroUsize::new(4).expect("nonzero"));
        let vote = vote(NativeAmxPhase::Prepare);
        cache
            .insert_vote(vote.clone())
            .expect("first vote should insert");
        assert!(matches!(
            cache.insert_vote(vote),
            Err(NativeAmxSessionError::DuplicateSigner)
        ));
    }

    #[test]
    fn session_cache_rejects_live_source_plan_equivocation() {
        let mut cache = NativeAmxSessionCache::new(NonZeroUsize::new(4).expect("nonzero"));
        let first = vote(NativeAmxPhase::Prepare);
        cache.insert_vote(first.clone()).expect("first plan claim");
        let mut equivocation = first;
        equivocation.body.plan_digest = Hash::new(b"equivocating-native-amx-plan");
        assert_eq!(
            cache.insert_vote(equivocation),
            Err(NativeAmxSessionError::PlanEquivocation)
        );
    }

    #[test]
    fn full_plan_request_binds_canonical_routes_and_coordinator_proposal() {
        let keypair = checked_bls_keypair(0x77);
        let validators = vec![PeerId::new(keypair.public_key().clone())];
        let request = full_plan_request(
            body_for_validator_set(NativeAmxPhase::Prepare, &validators),
            validators,
        );
        assert_eq!(request.validate_plan_binding(), Ok(()));

        let mut coordinator_participates = request.clone();
        coordinator_participates.body.participant_lane_id =
            coordinator_participates.body.coordinator_lane_id;
        coordinator_participates.body.participant_dataspace_id =
            coordinator_participates.body.coordinator_dataspace_id;
        coordinator_participates.body.participant_lane_incarnation =
            coordinator_participates.body.coordinator_lane_incarnation;
        coordinator_participates
            .body
            .participant_previous_block_height = coordinator_participates
            .coordinator_proposal
            .descriptor
            .previous_lane_block_height;
        coordinator_participates
            .body
            .participant_previous_block_descriptor_hash = coordinator_participates
            .coordinator_proposal
            .descriptor
            .previous_lane_block_descriptor_hash;
        coordinator_participates.body.participant_lane_block_height = coordinator_participates
            .coordinator_proposal
            .descriptor
            .lane_block_height;
        coordinator_participates.body.participant_lane_block_view = coordinator_participates
            .coordinator_proposal
            .descriptor
            .lane_block_view;
        coordinator_participates.participant_proposal =
            coordinator_participates.coordinator_proposal.clone();
        coordinator_participates.body.participant_proposal_hash =
            coordinator_participates.participant_proposal.proposal_hash;
        let coordinator_route = RoutingDecision::new(
            coordinator_participates.body.coordinator_lane_id,
            coordinator_participates.body.coordinator_dataspace_id,
        );
        let overlapping_plan = RoutingPlan::native_amx(
            coordinator_route,
            vec![RouteLeg::new(coordinator_route, RouteLegRole::Participant)],
        );
        coordinator_participates.body.plan_digest = overlapping_plan.digest();
        coordinator_participates.plan_legs = overlapping_plan.legs();
        coordinator_participates.participant_settlement = coordinator_participates
            .body
            .computed_grouped_participant_settlement(&[coordinator_participates.body.source_id])
            .expect("single-source test fixture settlement is valid");
        coordinator_participates
            .body
            .participant_settlement_commitment = Hash::from(
            iroha_data_model::nexus::compute_settlement_hash(
                &coordinator_participates.participant_settlement,
            )
            .expect("overlapping participant settlement hashes"),
        );
        assert_eq!(
            coordinator_participates.validate_plan_binding(),
            Ok(()),
            "the coordinator route may also own one participant leg"
        );

        let mut stale_same_route = coordinator_participates.clone();
        let stale_incarnation = Hash::new(b"stale same-route participant incarnation");
        stale_same_route.body.participant_lane_incarnation = stale_incarnation;
        stale_same_route
            .participant_proposal
            .descriptor
            .lane_incarnation = stale_incarnation;
        stale_same_route
            .participant_proposal
            .descriptor
            .descriptor_hash = stale_same_route
            .participant_proposal
            .descriptor
            .computed_descriptor_hash();
        stale_same_route.participant_proposal.proposal_hash = stale_same_route
            .participant_proposal
            .computed_proposal_hash();
        stale_same_route.body.participant_proposal_hash =
            stale_same_route.participant_proposal.proposal_hash;
        stale_same_route.participant_settlement = stale_same_route
            .body
            .computed_grouped_participant_settlement(&[stale_same_route.body.source_id])
            .expect("stale same-route settlement fixture remains structurally valid");
        stale_same_route.body.participant_settlement_commitment = Hash::from(
            iroha_data_model::nexus::compute_settlement_hash(
                &stale_same_route.participant_settlement,
            )
            .expect("stale same-route settlement hashes"),
        );
        assert_eq!(
            stale_same_route.validate_plan_binding(),
            Err(NativeAmxRequestError::ParticipantProposalMismatch),
            "a same-route participant cannot drift to another lane incarnation"
        );

        let mut omitted_participant = request.clone();
        omitted_participant.plan_legs.truncate(1);
        assert_eq!(
            omitted_participant.validate_plan_binding(),
            Err(NativeAmxRequestError::IncompletePlan)
        );

        let mut substituted_proposal = request;
        substituted_proposal.body.coordinator_proposal_hash =
            Hash::new(b"substituted-native-amx-coordinator-proposal");
        assert_eq!(
            substituted_proposal.validate_plan_binding(),
            Err(NativeAmxRequestError::CoordinatorProposalMismatch)
        );
    }

    #[test]
    fn session_cache_allows_same_signer_for_retried_body() {
        let mut cache = NativeAmxSessionCache::new(NonZeroUsize::new(4).expect("nonzero"));
        let vote = vote(NativeAmxPhase::Prepare);
        let key = NativeAmxSessionKey::from_body(&vote.body);
        let mut retried_vote = vote.clone();
        retried_vote.body.planned_coordinator_block_height = retried_vote
            .body
            .planned_coordinator_block_height
            .saturating_add(1);

        cache.insert_vote(vote.clone()).expect("first body vote");
        cache
            .insert_vote(retried_vote.clone())
            .expect("same signer may vote on a retried body");

        assert_eq!(cache.sorted_votes_for_body(key, &vote.body), vec![vote]);
        assert_eq!(
            cache.sorted_votes_for_body(key, &retried_vote.body),
            vec![retried_vote]
        );
        assert_eq!(cache.sorted_votes(key, NativeAmxPhase::Prepare).len(), 2);
    }

    #[test]
    fn session_cache_allows_same_signer_for_different_participant_legs() {
        let mut cache = NativeAmxSessionCache::new(NonZeroUsize::new(4).expect("nonzero"));
        let vote = vote(NativeAmxPhase::Prepare);
        let key = NativeAmxSessionKey::from_body(&vote.body);
        let mut other_leg = vote.clone();
        other_leg.body.participant_lane_id = LaneId::new(9);
        other_leg.body.participant_dataspace_id = DataSpaceId::new(10);

        cache.insert_vote(vote.clone()).expect("first leg vote");
        cache
            .insert_vote(other_leg.clone())
            .expect("same signer may vote on another participant leg");

        assert_eq!(cache.sorted_votes_for_body(key, &vote.body), vec![vote]);
        assert_eq!(
            cache.sorted_votes_for_body(key, &other_leg.body),
            vec![other_leg]
        );
    }

    #[test]
    fn session_cache_filters_exact_body_votes_to_validator_set() {
        let mut cache = NativeAmxSessionCache::new(NonZeroUsize::new(4).expect("nonzero"));
        let allowed_keypair = KeyPair::try_random_with_algorithm(Algorithm::BlsNormal)
            .expect("generate checked allowed native AMX BLS fixture keypair");
        let unknown_keypair = KeyPair::try_random_with_algorithm(Algorithm::BlsNormal)
            .expect("generate checked unknown native AMX BLS fixture keypair");
        let allowed = PeerId::new(allowed_keypair.public_key().clone());
        let unknown = PeerId::new(unknown_keypair.public_key().clone());
        let body = body(NativeAmxPhase::Prepare);
        let allowed_vote = NativeAmxVoteV2 {
            body,
            signer: allowed.clone(),
            bls_signature: vec![1],
        };
        let unknown_vote = NativeAmxVoteV2 {
            body,
            signer: unknown,
            bls_signature: vec![2],
        };
        let key = NativeAmxSessionKey::from_body(&body);

        cache
            .insert_vote(allowed_vote.clone())
            .expect("allowed signer vote");
        cache
            .insert_vote(unknown_vote)
            .expect("unknown signer vote");

        assert_eq!(
            cache.sorted_votes_for_body_from(key, &body, &[allowed]),
            vec![allowed_vote]
        );
    }

    #[test]
    fn session_cache_capacity_does_not_evict_source_plan_claims() {
        let mut cache = NativeAmxSessionCache::new(NonZeroUsize::new(1).expect("nonzero"));
        let first = vote(NativeAmxPhase::Prepare);
        let first_key = NativeAmxSessionKey::from_body(&first.body);
        cache.insert_vote(first.clone()).expect("first vote");

        let mut second = vote(NativeAmxPhase::Prepare);
        second.body.source_id = [0xAC; iroha_crypto::Hash::LENGTH];
        let second_key = NativeAmxSessionKey::from_body(&second.body);
        assert_eq!(
            cache.insert_vote(second),
            Err(NativeAmxSessionError::Capacity)
        );

        assert_eq!(
            cache.sorted_votes(first_key, NativeAmxPhase::Prepare).len(),
            1
        );
        assert!(
            cache
                .sorted_votes(second_key, NativeAmxPhase::Prepare)
                .is_empty()
        );

        let mut conflicting_plan = first;
        conflicting_plan.body.plan_digest = Hash::new(b"claim must survive capacity failure");
        assert_eq!(
            cache.insert_vote(conflicting_plan),
            Err(NativeAmxSessionError::PlanEquivocation)
        );
    }

    #[test]
    fn session_cache_body_capacity_fails_without_fifo_eviction() {
        let mut cache = NativeAmxSessionCache::with_limits(
            NonZeroUsize::new(4).expect("nonzero sessions"),
            NonZeroUsize::new(2).expect("nonzero body buckets"),
        );
        let first = vote(NativeAmxPhase::Prepare);
        let key = NativeAmxSessionKey::from_body(&first.body);
        let mut second = first.clone();
        second.body.planned_coordinator_block_height = 43;
        let mut third = first.clone();
        third.body.planned_coordinator_block_height = 44;

        cache.insert_vote(first.clone()).expect("first vote");
        cache.insert_vote(second.clone()).expect("second vote");
        assert_eq!(
            cache.insert_vote(third.clone()),
            Err(NativeAmxSessionError::Capacity)
        );

        assert_eq!(cache.sorted_votes_for_body(key, &first.body), vec![first]);
        assert_eq!(cache.sorted_votes_for_body(key, &second.body), vec![second]);
        assert!(cache.sorted_votes_for_body(key, &third.body).is_empty());
        assert_eq!(cache.sorted_votes(key, NativeAmxPhase::Prepare).len(), 2);
    }

    fn signed_vote(body: &NativeAmxAttestationBodyV2, keypair: &KeyPair) -> NativeAmxVoteV2 {
        NativeAmxVoteV2 {
            body: *body,
            signer: PeerId::new(keypair.public_key().clone()),
            bls_signature: checked_bls_signature_payload(keypair, &body.signature_preimage()),
        }
    }

    #[test]
    fn vote_ingress_validation_accepts_matching_signed_bls_vote() {
        let keypair = checked_bls_keypair(0xE1);
        let body = body(NativeAmxPhase::Prepare);
        let vote = signed_vote(&body, &keypair);
        let sender = vote.signer.clone();

        assert_eq!(
            vote.validate_ingress(NativeAmxPhase::Prepare, Some(&sender)),
            Ok(())
        );
    }

    #[test]
    fn vote_ingress_validation_rejects_phase_and_sender_mismatches() {
        let keypair = checked_bls_keypair(0xE2);
        let other_keypair = checked_bls_keypair(0xE3);
        let body = body(NativeAmxPhase::Prepare);
        let vote = signed_vote(&body, &keypair);
        let sender = vote.signer.clone();
        let other_sender = PeerId::new(other_keypair.public_key().clone());

        assert_eq!(
            vote.validate_ingress(NativeAmxPhase::Commit, Some(&sender)),
            Err(NativeAmxVoteIngressError::PhaseMismatch {
                expected: NativeAmxPhase::Commit,
                actual: NativeAmxPhase::Prepare
            })
        );
        assert_eq!(
            vote.validate_ingress(NativeAmxPhase::Prepare, Some(&other_sender)),
            Err(NativeAmxVoteIngressError::SenderMismatch)
        );
    }

    #[test]
    fn vote_ingress_validation_rejects_non_bls_and_bad_signatures() {
        let ed25519_keypair = checked_random_ed25519_keypair();
        let body = body(NativeAmxPhase::Commit);
        let ed25519_signature =
            Signature::try_new(ed25519_keypair.private_key(), &body.signature_preimage())
                .expect("checked Ed25519 fixture signature")
                .payload()
                .to_vec();
        let ed25519_vote = NativeAmxVoteV2 {
            body,
            signer: PeerId::new(ed25519_keypair.public_key().clone()),
            bls_signature: ed25519_signature,
        };

        assert_eq!(
            ed25519_vote.validate_ingress(NativeAmxPhase::Commit, None),
            Err(NativeAmxVoteIngressError::InvalidSignature),
            "the fixed-width signature gate runs before signer-algorithm inspection"
        );

        let mut non_bls_vote = ed25519_vote;
        non_bls_vote.bls_signature = vec![0_u8; NATIVE_AMX_BLS_PROOF_BYTES];
        assert_eq!(
            non_bls_vote.validate_ingress(NativeAmxPhase::Commit, None),
            Err(NativeAmxVoteIngressError::SignerNotBlsNormal)
        );

        let bls_keypair = checked_bls_keypair(0xE4);
        let mut bad_signature_vote = signed_vote(&body, &bls_keypair);
        bad_signature_vote.bls_signature = vec![0_u8; 96];

        assert_eq!(
            bad_signature_vote.validate_ingress_shape(NativeAmxPhase::Commit, None),
            Ok(()),
            "the cheap envelope gate must not parse attacker-controlled BLS bytes"
        );
        assert_eq!(
            bad_signature_vote.verify_signature(),
            Err(NativeAmxVoteIngressError::InvalidSignature)
        );
        assert_eq!(
            bad_signature_vote.validate_ingress(NativeAmxPhase::Commit, None),
            Err(NativeAmxVoteIngressError::InvalidSignature)
        );
    }

    #[test]
    fn aggregate_votes_to_qc_orders_votes_by_validator_set() {
        let keypairs = [
            checked_bls_keypair(0xA1),
            checked_bls_keypair(0xB2),
            checked_bls_keypair(0xC3),
        ];
        let mut validator_set = keypairs
            .iter()
            .map(|keypair| PeerId::new(keypair.public_key().clone()))
            .collect::<Vec<_>>();
        validator_set.sort();
        let body = body_for_validator_set(NativeAmxPhase::Commit, &validator_set);
        let validator_set_pops = aligned_pops(&validator_set, &keypairs);
        let votes = vec![
            signed_vote(&body, &keypairs[2]),
            signed_vote(&body, &keypairs[0]),
            signed_vote(&body, &keypairs[1]),
        ];

        let qc = aggregate_votes_to_qc(
            body,
            validator_set.clone(),
            validator_set_pops.clone(),
            &votes,
            3,
        )
        .expect("valid quorum should aggregate");

        assert_eq!(qc.body, body);
        assert_eq!(qc.validator_set, validator_set);
        assert_eq!(qc.validator_set_pops, validator_set_pops);
        let mut expected_bitmap = vec![0_u8; validator_set.len().div_ceil(8)];
        for keypair in [&keypairs[2], &keypairs[0], &keypairs[1]] {
            let signer = PeerId::new(keypair.public_key().clone());
            let index = validator_set
                .iter()
                .position(|validator| validator == &signer)
                .expect("vote signer belongs to fixture committee");
            expected_bitmap[index / 8] |= 1_u8 << (index % 8);
        }
        assert_eq!(qc.signers_bitmap, expected_bitmap);
        let individual_signatures = [
            signed_vote(&body, &keypairs[0]).bls_signature,
            signed_vote(&body, &keypairs[1]).bls_signature,
            signed_vote(&body, &keypairs[2]).bls_signature,
        ];
        let signature_refs = individual_signatures
            .iter()
            .map(Vec::as_slice)
            .collect::<Vec<_>>();
        let expected_aggregate = iroha_crypto::bls_normal_aggregate_signatures(&signature_refs)
            .expect("aggregate reference signatures");
        assert_eq!(qc.bls_aggregate_signature, expected_aggregate);
    }

    #[test]
    fn aggregate_votes_to_qc_preserves_sparse_high_index_signer_order() {
        let mut keypairs = (1_u8..=10).map(checked_bls_keypair).collect::<Vec<_>>();
        keypairs.sort_by_key(|keypair| PeerId::new(keypair.public_key().clone()));
        let validator_set = keypairs
            .iter()
            .map(|keypair| PeerId::new(keypair.public_key().clone()))
            .collect::<Vec<_>>();
        let body = body_for_validator_set(NativeAmxPhase::Commit, &validator_set);
        let validator_set_pops = aligned_pops(&validator_set, &keypairs);
        let signer_indices = [0_usize, 1, 2, 3, 4, 8, 9];
        let votes = signer_indices
            .into_iter()
            .map(|index| signed_vote(&body, &keypairs[index]))
            .collect::<Vec<_>>();

        let qc = aggregate_votes_to_qc(body, validator_set.clone(), validator_set_pops, &votes, 7)
            .expect("exact-threshold sparse native AMX QC");
        assert_eq!(qc.signers_bitmap, vec![0b0001_1111, 0b0000_0011]);
        let pops = keypairs
            .iter()
            .map(|keypair| {
                (
                    keypair.public_key().clone(),
                    iroha_crypto::bls_normal_pop_prove(keypair.private_key())
                        .expect("prove fixture PoP"),
                )
            })
            .collect::<BTreeMap<_, _>>();
        assert_eq!(
            validate_native_amx_qc(&qc, &body, &validator_set, 7, &pops),
            Ok(())
        );

        let mut high_padding_bit = qc;
        high_padding_bit.signers_bitmap[1] |= 0b1000_0000;
        assert_eq!(
            validate_native_amx_qc(&high_padding_bit, &body, &validator_set, 7, &pops),
            Err(NativeAmxQcValidationError::InvalidSignerBitmap)
        );
    }

    #[test]
    fn aggregate_votes_to_qc_rejects_bad_vote_sets() {
        let keypairs = [checked_bls_keypair(0xD1), checked_bls_keypair(0xD2)];
        let mut validator_set = keypairs
            .iter()
            .map(|keypair| PeerId::new(keypair.public_key().clone()))
            .collect::<Vec<_>>();
        validator_set.sort();
        let body = body_for_validator_set(NativeAmxPhase::Prepare, &validator_set);
        let validator_set_pops = aligned_pops(&validator_set, &keypairs);
        let vote = signed_vote(&body, &keypairs[0]);

        assert_eq!(
            aggregate_votes_to_qc(
                body,
                validator_set.clone(),
                validator_set_pops.clone(),
                &[],
                2,
            ),
            Err(NativeAmxQcBuildError::EmptyVotes)
        );
        assert_eq!(
            aggregate_votes_to_qc(
                body,
                validator_set.clone(),
                validator_set_pops.clone(),
                &[vote.clone()],
                2,
            ),
            Err(NativeAmxQcBuildError::QuorumNotMet)
        );
        assert_eq!(
            aggregate_votes_to_qc(
                body,
                validator_set.clone(),
                validator_set_pops.clone(),
                &[vote.clone(), vote.clone()],
                2
            ),
            Err(NativeAmxQcBuildError::DuplicateSigner)
        );

        let outsider = checked_bls_keypair(0xD3);
        assert_eq!(
            aggregate_votes_to_qc(
                body,
                validator_set.clone(),
                validator_set_pops.clone(),
                &[signed_vote(&body, &outsider)],
                2
            ),
            Err(NativeAmxQcBuildError::SignerNotInValidatorSet)
        );

        let ed25519_keypair = checked_random_ed25519_keypair();
        let ed25519_signer = PeerId::new(ed25519_keypair.public_key().clone());
        let ed25519_body = body_for_validator_set(
            NativeAmxPhase::Prepare,
            std::slice::from_ref(&ed25519_signer),
        );
        let ed25519_vote = NativeAmxVoteV2 {
            body: ed25519_body,
            signer: ed25519_signer.clone(),
            bls_signature: Signature::try_new(
                ed25519_keypair.private_key(),
                &ed25519_body.signature_preimage(),
            )
            .expect("checked Ed25519 fixture signature")
            .payload()
            .to_vec(),
        };
        assert_eq!(
            aggregate_votes_to_qc(
                ed25519_body,
                vec![ed25519_signer],
                vec![vec![0; NATIVE_AMX_BLS_PROOF_BYTES]],
                &[ed25519_vote],
                1,
            ),
            Err(NativeAmxQcBuildError::SignerNotBlsNormal)
        );

        let mut bad_signature_vote = vote.clone();
        bad_signature_vote.bls_signature = vec![0_u8; 96];
        assert_eq!(
            aggregate_votes_to_qc(
                body,
                validator_set.clone(),
                validator_set_pops.clone(),
                &[bad_signature_vote],
                2
            ),
            Err(NativeAmxQcBuildError::InvalidSignature)
        );

        let mut wrong_body_vote = vote;
        wrong_body_vote.body.phase = NativeAmxPhase::Commit;
        assert_eq!(
            aggregate_votes_to_qc(
                body,
                validator_set,
                validator_set_pops,
                &[wrong_body_vote],
                2,
            ),
            Err(NativeAmxQcBuildError::BodyMismatch)
        );

        let keypairs = [
            checked_bls_keypair(0xD4),
            checked_bls_keypair(0xD5),
            checked_bls_keypair(0xD6),
            checked_bls_keypair(0xD7),
        ];
        let mut validator_set = keypairs
            .iter()
            .map(|keypair| PeerId::new(keypair.public_key().clone()))
            .collect::<Vec<_>>();
        validator_set.sort();
        let mut lowered_body = body_for_validator_set(NativeAmxPhase::Prepare, &validator_set);
        lowered_body.participant_min_quorum = 2;
        let lowered_votes = keypairs
            .iter()
            .map(|keypair| signed_vote(&lowered_body, keypair))
            .collect::<Vec<_>>();
        assert_eq!(
            aggregate_votes_to_qc(
                lowered_body,
                validator_set.clone(),
                aligned_pops(&validator_set, &keypairs),
                &lowered_votes,
                2,
            ),
            Err(NativeAmxQcBuildError::InvalidValidatorSet),
            "a signed committee context must not lower the canonical threshold"
        );

        let canonical_body = body_for_validator_set(NativeAmxPhase::Prepare, &validator_set);
        let mut reversed = validator_set.clone();
        reversed.reverse();
        assert_eq!(
            aggregate_votes_to_qc(
                canonical_body,
                reversed,
                aligned_pops(&validator_set, &keypairs),
                &lowered_votes,
                3,
            ),
            Err(NativeAmxQcBuildError::InvalidValidatorSet)
        );
    }

    // Commit-request and QC replay-validation tests retain their stable libtest paths.
    include!("native_amx/commit_validation_tail_tests.rs");
}
