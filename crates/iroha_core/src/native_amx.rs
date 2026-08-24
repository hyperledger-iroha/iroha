//! Native AMX control-plane messages and deterministic vote-session cache.
use crate::queue::{RouteLeg, RouteLegRole, RoutingDecision, RoutingPlan, RoutingPlan::NativeAmx};
use iroha_crypto::{Algorithm, Hash, HashOf, PublicKey, Signature};
use iroha_data_model::{
    NetworkId,
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
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File},
    num::NonZeroUsize,
    path::{Path, PathBuf},
};
#[cfg(unix)]
use std::{
    fs::{DirBuilder, OpenOptions},
    io::{Read, Write},
    os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt},
};
use thiserror::Error;
const DEFAULT_SESSION_BODY_BUCKET_MAX: usize = 256;
const NATIVE_AMX_SIGNING_GUARD_VERSION: u8 = 5;
#[cfg(unix)]
const NATIVE_AMX_SIGNING_GUARD_DIRECTORY: &str = "native-amx-v2-signing-guard-v5";
#[cfg(unix)]
const NATIVE_AMX_UNSUPPORTED_SIGNING_GUARD_DIRECTORIES: &[&str] = &[
    "native-amx-v2-signing-guard-v1",
    "native-amx-v2-signing-guard-v2",
    "native-amx-v2-signing-guard-v3",
    "native-amx-v2-signing-guard-v4",
];
const NATIVE_AMX_SIGNING_GUARD_RECORD_EXTENSION: &str = "norito";
const NATIVE_AMX_SIGNING_GUARD_TEMP_EXTENSION: &str = "norito.tmp";
#[cfg(unix)]
const NATIVE_AMX_SIGNING_GUARD_LOCK_FILE: &str = "owner.lock";
const NATIVE_AMX_SIGNING_GUARD_ANCHOR_FILE: &str = "chain-anchor.norito";
const NATIVE_AMX_SIGNING_GUARD_ANCHOR_TEMP: &str = "chain-anchor.norito.tmp";
#[cfg(unix)]
const NATIVE_AMX_SIGNER_DIRECTORY_DOMAIN: &[u8] = b"iroha:native-amx:v2:signer-directory:v1\0";
const NATIVE_AMX_SIGNING_BODY_DOMAIN: &[u8] = b"iroha:native-amx:v2:signing-body:v5\0";
const NATIVE_AMX_SIGNING_RECORD_DOMAIN: &[u8] = b"iroha:native-amx:v2:record-chain:v5\0";
const NATIVE_AMX_SIGNING_GENESIS_DOMAIN: &[u8] = b"iroha:native-amx:v2:record-genesis:v5\0";
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
/// Classify whether a control leg needs durable participant application evidence.
/// The exact coordinator route is represented by the global block and creates
/// no duplicate receipt/frontier marker; identity drift is rejected.
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
/// Whether this exact route incarnation needs separate participant application
/// evidence; all legs remain classified so malformed identity drift fails closed.
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
    network_id: NetworkId,
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
        let mut round = body.round;
        round.view = 0;
        Self {
            network_id: body.network_id,
            context_id: body.round.context_id,
            round,
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
    network_id: NetworkId,
    context_id: HeightContextId,
    epoch: u64,
    authority_context_height: u64,
    participant_lane_id: LaneId,
    participant_dataspace_id: DataSpaceId,
    participant_lane_incarnation: Hash,
    participant_lane_block_height: u64,
    participant_lane_block_view: u64,
    signer: PeerId,
}
impl NativeAmxSigningSlotV3 {
    fn from_body(body: &NativeAmxAttestationBodyV2, signer: &PeerId) -> Self {
        Self {
            network_id: body.network_id,
            context_id: body.round.context_id,
            epoch: body.epoch,
            authority_context_height: body.authority_context_height,
            participant_lane_id: body.participant_lane_id,
            participant_dataspace_id: body.participant_dataspace_id,
            participant_lane_incarnation: body.participant_lane_incarnation,
            participant_lane_block_height: body.participant_lane_block_height,
            participant_lane_block_view: body.participant_lane_block_view,
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
/// Immutable source-session claim shared by every phase and participant leg.
/// It binds entrypoint, global context, plan, authority height, and coordinator,
/// while allowing the same request in a later certified global view.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
struct NativeAmxSourceSessionClaimV4 {
    source_id: [u8; Hash::LENGTH],
    tx_entrypoint_hash: HashOf<TransactionEntrypoint>,
    plan_digest: Hash,
    context_id: HeightContextId,
    round_height: u64,
    epoch: u64,
    network_id: NetworkId,
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
            context_id: body.round.context_id,
            round_height: body.round.height,
            epoch: body.epoch,
            network_id: body.network_id,
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
/// Participant route/incarnation attached to one source-session claim.
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
    network_id: NetworkId,
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
    certified_view: Option<u64>,
    record_floor: u32,
    floor_head: Hash,
    record_count: u32,
    head_hash: Hash,
    highest_view: Option<u64>,
    last_prepare_view: Option<u64>,
}
impl NativeAmxSigningAnchorV2 {
    #[cfg(unix)]
    fn empty(binding: NativeAmxHeightBindingV2) -> Result<Self, NativeAmxSigningGuardError> {
        let head_hash = binding.genesis_head()?;
        Ok(Self {
            version: NATIVE_AMX_SIGNING_GUARD_VERSION,
            binding,
            certified_view: None,
            record_floor: 0,
            floor_head: head_hash,
            record_count: 0,
            head_hash,
            highest_view: None,
            last_prepare_view: None,
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
    prepare_view: Option<u64>,
    prepare_records: BTreeMap<NativeAmxSigningKeyV2, NativeAmxAttestationBodyV2>,
    prepare_source_claims: BTreeMap<[u8; Hash::LENGTH], NativeAmxDurableSourceClaimV4>,
    prepare_slot_claims: BTreeMap<NativeAmxSigningSlotV3, NativeAmxSigningSlotClaimV3>,
    prepare_quarantine_view: Option<u64>,
    poisoned: Option<String>,
}
#[derive(Debug)]
#[cfg(unix)]
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
    #[cfg(unix)]
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
    #[cfg(unix)]
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
    /// Restart cannot reconstruct the exact Prepare body signed in this view.
    #[error(
        "native AMX Prepare signing is quarantined in recovered view {view}; wait for a certified higher view"
    )]
    PrepareViewQuarantined {
        /// Recovered view which may already contain a local Prepare signature.
        view: u64,
    },
    /// The same source transaction attempted to change its retained session claim.
    #[error("native AMX source transaction conflicts with its durable session claim")]
    PlanEquivocation,
    /// One lane-local signing slot attempted a different proposal or settlement.
    #[error("native AMX participant slot conflicts with its durable proposal/settlement claim")]
    SlotEquivocation,
    /// The exact signing key already authorizes a different full body.
    #[error("native AMX body conflicts with the durable signing decision")]
    Equivocation,
    /// The guard has reached its configured decision bound.
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
            | Self::ContextMismatch
            | Self::FutureHeight { .. }
            | Self::StaleHeight { .. } => true,
            #[cfg(unix)]
            Self::HeightRegression { .. } | Self::HeightJump { .. } => true,
            #[cfg(not(unix))]
            Self::UnsupportedPlatform => true,
            _ => false,
        }
    }
}
/// Validated runtime ceilings for one Native AMX signing guard.
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
/// Crash-safe local anti-equivocation guard for Native AMX v2 votes.
/// Commits form a checkpointed authenticated chain; prepares fsync and quarantine
/// their volatile view. Call `record` before signing. Whole-directory rollback
/// requires external monotonic storage.
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
    /// Open a signer journal for one frozen context, enforcing the protocol
    /// record bound and exact reopen-or-next-height progression.
    pub(crate) fn open(
        store_root: &Path,
        active_height: u64,
        context_id: HeightContextId,
        epoch: u64,
        network_id: NetworkId,
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
                network_id,
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
                network_id,
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
        network_id: NetworkId,
        signer: PeerId,
        limits: NativeAmxSigningGuardLimits,
    ) -> Result<Self, NativeAmxSigningGuardError> {
        if active_height == 0
            || native_amx_hash_is_zero_sentinel(network_id.as_bytes())
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
        let supplied_binding = NativeAmxHeightBindingV2 {
            active_height,
            context_id,
            epoch,
            network_id,
            signer: signer.clone(),
            max_records: max_records_u32,
        };
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
                if anchor.binding.network_id != network_id
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
        let prepare_quarantine_view = anchor.last_prepare_view;
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
                prepare_view: None,
                prepare_records: BTreeMap::new(),
                prepare_source_claims: BTreeMap::new(),
                prepare_slot_claims: BTreeMap::new(),
                prepare_quarantine_view,
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
        let genesis_head = anchor
            .binding
            .genesis_head()
            .map_err(|error| native_amx_unsafe_journal(&path, error.to_string()))?;
        let active_record_count = anchor.record_count.checked_sub(anchor.record_floor);
        if anchor.version != NATIVE_AMX_SIGNING_GUARD_VERSION
            || anchor.binding.active_height == 0
            || native_amx_hash_is_zero_sentinel(anchor.binding.context_id.0.as_ref())
            || native_amx_hash_is_zero_sentinel(anchor.binding.network_id.as_bytes())
            || anchor.binding.max_records == 0
            || usize::try_from(anchor.binding.max_records)
                .map_or(true, |max| max > MAX_NATIVE_AMX_SIGNING_GUARD_RECORDS_HARD)
            || active_record_count.is_none_or(|count| count > anchor.binding.max_records)
            || (anchor.record_floor == anchor.record_count && anchor.record_floor != 0)
            || (anchor.record_floor == 0 && anchor.floor_head != genesis_head)
            || (anchor.record_count == 0 && anchor.head_hash != genesis_head)
            || anchor
                .certified_view
                .is_some_and(|view| anchor.highest_view.is_none_or(|highest| view > highest))
            || anchor
                .last_prepare_view
                .is_some_and(|view| anchor.highest_view.is_none_or(|highest| view > highest))
            || anchor.certified_view.is_some_and(|certified| {
                anchor
                    .last_prepare_view
                    .is_some_and(|prepare| prepare < certified)
            })
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
            || record.body.phase != NativeAmxPhase::Commit
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
    #[cfg(unix)]
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
            || record.body.network_id != binding.network_id
            || record.key.signer != binding.signer
        {
            return Err(native_amx_unsafe_journal(
                path,
                "record does not match the anchored height context",
            ));
        }
        Ok(())
    }
    #[cfg(unix)]
    fn load_validated_journal(
        directory: &Path,
        directory_handle: &File,
        owner_uid: u32,
        anchor: &NativeAmxSigningAnchorV2,
        max_records: usize,
        max_record_bytes: usize,
    ) -> Result<LoadedNativeAmxJournal, NativeAmxSigningGuardError> {
        let mut current = BTreeMap::<u32, (NativeAmxSigningRecordV2, PathBuf)>::new();
        let mut cleanup_paths = Vec::new();
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
            if final_record_count > max_records.saturating_mul(2) {
                return Err(native_amx_unsafe_journal(
                    directory,
                    "record and crash-leftover count exceeds the configured runtime limit",
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
                cleanup_paths.push(path);
                continue;
            }
            Self::validate_record_binding(&path, &record, anchor)?;
            let retired_by_view = anchor
                .certified_view
                .is_some_and(|view| record.body.round.view < view);
            if record.sequence <= anchor.record_floor
                || (anchor.record_floor == 0 && retired_by_view)
            {
                if !retired_by_view
                    || (record.sequence == anchor.record_floor
                        && anchor.record_floor != 0
                        && record.record_hash != anchor.floor_head)
                {
                    return Err(native_amx_unsafe_journal(
                        &path,
                        "record conflicts with the certified prefix checkpoint",
                    ));
                }
                cleanup_paths.push(path);
                continue;
            }
            if retired_by_view {
                return Err(native_amx_unsafe_journal(
                    &path,
                    "retired-view record appears inside the active suffix",
                ));
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
        let active_record_count = anchor
            .record_count
            .checked_sub(anchor.record_floor)
            .ok_or_else(|| native_amx_unsafe_journal(directory, "record floor exceeds tail"))?;
        let expected_count = usize::try_from(active_record_count)
            .map_err(|_| native_amx_unsafe_journal(directory, "record count overflow"))?;
        let mut head = anchor.floor_head;
        let mut records = BTreeMap::new();
        let mut source_claims = BTreeMap::new();
        let mut slot_claims = BTreeMap::new();
        let mut anchored_paths = Vec::with_capacity(expected_count);
        let mut highest_view = None::<u64>;
        for sequence in anchor.record_floor.saturating_add(1)..=anchor.record_count {
            let Some((record, path)) = current.remove(&sequence) else {
                return Err(native_amx_unsafe_journal(
                    directory,
                    format!("anchored record sequence {sequence} is missing"),
                ));
            };
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
        let anchor_covers_record_views = match (anchor.highest_view, highest_view) {
            (None, None) => true,
            (Some(anchor_view), Some(record_view)) => anchor_view >= record_view,
            (Some(_), None) => true,
            (None, Some(_)) => false,
        };
        if records.len() != expected_count
            || head != anchor.head_hash
            || !anchor_covers_record_views
        {
            return Err(native_amx_unsafe_journal(
                directory,
                "chain anchor count, head, or highest view mismatch",
            ));
        }
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
                || active_record_count >= anchor.binding.max_records
                || anchor
                    .certified_view
                    .is_some_and(|view| tail.body.round.view < view)
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
    #[cfg(unix)]
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
    fn advance_view_high_water(
        &self,
        inner: &mut NativeAmxSigningGuardInner,
        view: u64,
    ) -> Result<(), NativeAmxSigningGuardError> {
        if inner
            .anchor
            .highest_view
            .is_some_and(|highest| view <= highest)
        {
            return Ok(());
        }
        let mut next_anchor = inner.anchor.clone();
        next_anchor.highest_view = Some(view);
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
        Ok(())
    }
    /// Retain Commit claims at or above a certified view, publishing the anchor
    /// before removing retired prefixes so restart sees an authenticated chain.
    pub(crate) fn advance_certified_view(
        &self,
        view: u64,
    ) -> Result<(), NativeAmxSigningGuardError> {
        let mut inner = self.inner.lock();
        if let Some(reason) = inner.poisoned.as_ref() {
            return Err(NativeAmxSigningGuardError::Poisoned(reason.clone()));
        }
        let result = self.advance_certified_view_locked(&mut inner, view);
        if let Err(NativeAmxSigningGuardError::UnsafeJournal(message)) = &result {
            inner.poisoned = Some(message.clone());
        }
        result
    }
    fn advance_certified_view_locked(
        &self,
        inner: &mut NativeAmxSigningGuardInner,
        view: u64,
    ) -> Result<(), NativeAmxSigningGuardError> {
        if inner
            .anchor
            .certified_view
            .is_some_and(|certified| view <= certified)
        {
            return Ok(());
        }
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
        let mut ordered = inner
            .records
            .iter()
            .map(|(key, record)| (record.sequence, key.clone(), record.clone()))
            .collect::<Vec<_>>();
        ordered.sort_by_key(|(sequence, _, _)| *sequence);
        let mut retired = Vec::new();
        let mut saw_retained = false;
        for (_, key, record) in &ordered {
            if record.body.round.view < view {
                if saw_retained {
                    return Err(native_amx_unsafe_journal(
                        &self.directory,
                        "certified-view retirement is not a record-chain prefix",
                    ));
                }
                let (path, _) = inner.record_identities.get(key).ok_or_else(|| {
                    native_amx_unsafe_journal(
                        &self.directory,
                        "retired Commit claim has no retained path identity",
                    )
                })?;
                retired.push((key.clone(), path.clone(), record.clone()));
            } else {
                saw_retained = true;
            }
        }
        let mut next_anchor = inner.anchor.clone();
        next_anchor.certified_view = Some(view);
        next_anchor.highest_view = Some(
            next_anchor
                .highest_view
                .map_or(view, |highest| highest.max(view)),
        );
        if next_anchor
            .last_prepare_view
            .is_some_and(|prepare| prepare < view)
        {
            next_anchor.last_prepare_view = None;
        }
        if retired.len() == ordered.len() {
            let genesis_head = next_anchor.binding.genesis_head()?;
            next_anchor.record_floor = 0;
            next_anchor.floor_head = genesis_head;
            next_anchor.record_count = 0;
            next_anchor.head_hash = genesis_head;
        } else if let Some((_, _, last_retired)) = retired.last() {
            next_anchor.record_floor = last_retired.sequence;
            next_anchor.floor_head = last_retired.record_hash;
        }
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
        for (_, path, _) in &retired {
            fs::remove_file(path)
                .map_err(|error| native_amx_unsafe_journal(path, error.to_string()))?;
        }
        if !retired.is_empty() {
            native_amx_sync_directory_handle(&self.directory, &self.directory_handle)?;
        }
        for (key, _, _) in retired {
            inner.records.remove(&key);
            inner.record_identities.remove(&key);
        }
        inner.anchor = next_anchor;
        inner.anchor_identity = anchor_identity;
        Self::rebuild_durable_claims(inner);
        if inner.prepare_view != Some(view) {
            inner.prepare_view = None;
            inner.prepare_records.clear();
            inner.prepare_source_claims.clear();
            inner.prepare_slot_claims.clear();
        }
        if inner
            .prepare_quarantine_view
            .is_some_and(|quarantined| quarantined < view)
        {
            inner.prepare_quarantine_view = None;
        }
        Ok(())
    }
    fn rebuild_durable_claims(inner: &mut NativeAmxSigningGuardInner) {
        inner.source_claims.clear();
        inner.slot_claims.clear();
        for record in inner.records.values() {
            let body = &record.body;
            inner
                .source_claims
                .entry(body.source_id)
                .and_modify(|claim| claim.insert_participant(body))
                .or_insert_with(|| NativeAmxDurableSourceClaimV4::from_body(body));
            inner.slot_claims.insert(
                NativeAmxSigningSlotV3::from_body(body, &record.key.signer),
                NativeAmxSigningSlotClaimV3::from_body(body),
            );
        }
    }
    fn select_prepare_view(inner: &mut NativeAmxSigningGuardInner, view: u64) {
        if inner.prepare_view != Some(view) {
            inner.prepare_view = Some(view);
            inner.prepare_records.clear();
            inner.prepare_source_claims.clear();
            inner.prepare_slot_claims.clear();
        }
        if inner
            .prepare_quarantine_view
            .is_some_and(|quarantined| view > quarantined)
        {
            inner.prepare_quarantine_view = None;
        }
    }
    fn mark_prepare_view(
        &self,
        inner: &mut NativeAmxSigningGuardInner,
        view: u64,
    ) -> Result<(), NativeAmxSigningGuardError> {
        if inner.anchor.last_prepare_view == Some(view) {
            return Ok(());
        }
        let mut next_anchor = inner.anchor.clone();
        next_anchor.last_prepare_view = Some(view);
        next_anchor.highest_view = Some(
            next_anchor
                .highest_view
                .map_or(view, |highest| highest.max(view)),
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
        Ok(())
    }
    fn retire_prepare_superseded_by_commit(
        inner: &mut NativeAmxSigningGuardInner,
        commit: &NativeAmxAttestationBodyV2,
    ) {
        let mut prepare = *commit;
        prepare.phase = NativeAmxPhase::Prepare;
        let key = NativeAmxSigningKeyV2::from_body(&prepare, &inner.anchor.binding.signer);
        if inner.prepare_records.get(&key) != Some(&prepare) {
            return;
        }
        inner.prepare_records.remove(&key);
        // The matching durable Commit claim was installed before this call.
        // Keep volatile Prepare claims as monotone same-view supersets so
        // replacement stays O(log N) while conflicts remain fail-closed.
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
        let binding = inner.anchor.binding.clone();
        if body.network_id != binding.network_id
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
        Self::select_prepare_view(inner, body.round.view);
        let key = NativeAmxSigningKeyV2::from_body(body, &binding.signer);
        let slot = NativeAmxSigningSlotV3::from_body(body, &binding.signer);
        let slot_claim = NativeAmxSigningSlotClaimV3::from_body(body);
        if body.phase == NativeAmxPhase::Prepare {
            if inner.prepare_quarantine_view == Some(body.round.view) {
                return Err(NativeAmxSigningGuardError::PrepareViewQuarantined {
                    view: body.round.view,
                });
            }
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
            if let Some(existing) = inner.prepare_records.get(&key) {
                return if existing == body {
                    Ok(())
                } else {
                    Err(NativeAmxSigningGuardError::Equivocation)
                };
            }
            if inner
                .prepare_slot_claims
                .get(&slot)
                .is_some_and(|claim| *claim != slot_claim)
            {
                return Err(NativeAmxSigningGuardError::SlotEquivocation);
            }
            if inner
                .prepare_source_claims
                .get(&body.source_id)
                .is_some_and(|claim| !claim.accepts(body))
            {
                return Err(NativeAmxSigningGuardError::PlanEquivocation);
            }
            if inner
                .records
                .len()
                .saturating_add(inner.prepare_records.len())
                >= self.limits.max_records.get()
            {
                return Err(NativeAmxSigningGuardError::Capacity);
            }
            self.mark_prepare_view(inner, body.round.view)?;
            inner.prepare_records.insert(key, *body);
            inner
                .prepare_source_claims
                .entry(body.source_id)
                .and_modify(|claim| claim.insert_participant(body))
                .or_insert_with(|| NativeAmxDurableSourceClaimV4::from_body(body));
            inner.prepare_slot_claims.entry(slot).or_insert(slot_claim);
            return Ok(());
        }
        if let Some(existing) = inner.records.get(&key) {
            if !native_amx_bodies_share_durable_signing_decision(&existing.body, body) {
                return Err(NativeAmxSigningGuardError::Equivocation);
            }
            self.advance_view_high_water(inner, body.round.view)?;
            Self::retire_prepare_superseded_by_commit(inner, body);
            return Ok(());
        }
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
            .prepare_slot_claims
            .get(&slot)
            .is_some_and(|claim| *claim != slot_claim)
        {
            return Err(NativeAmxSigningGuardError::SlotEquivocation);
        }
        if inner
            .prepare_source_claims
            .get(&body.source_id)
            .is_some_and(|claim| !claim.accepts(body))
        {
            return Err(NativeAmxSigningGuardError::PlanEquivocation);
        }
        let mut prepare = *body;
        prepare.phase = NativeAmxPhase::Prepare;
        let prepare_key = NativeAmxSigningKeyV2::from_body(&prepare, &binding.signer);
        let replaces_prepare =
            usize::from(inner.prepare_records.get(&prepare_key) == Some(&prepare));
        if inner
            .records
            .len()
            .saturating_add(inner.prepare_records.len())
            .saturating_sub(replaces_prepare)
            >= self.limits.max_records.get()
        {
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
        Self::retire_prepare_superseded_by_commit(inner, body);
        Ok(())
    }
    /// Authorize a full-plan request before BLS signing. Commits are durable;
    /// prepares are volatile after their view marker is fsynced. Exact replay is
    /// idempotent, while conflicts, stale views, or unsafe I/O poison the guard.
    pub(crate) fn record(
        &self,
        request: &NativeAmxAttestationRequestV2,
    ) -> Result<(), NativeAmxSigningGuardError> {
        request.validate_plan_binding().map_err(|error| {
            NativeAmxSigningGuardError::InvalidInput(format!(
                "unauthenticated Native AMX attestation request: {error}"
            ))
        })?;
        let body = &request.body;
        self.record_validated_body(body)
    }
    fn record_validated_body(
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
    pub(crate) fn record_body_for_test(
        &self,
        body: &NativeAmxAttestationBodyV2,
    ) -> Result<(), NativeAmxSigningGuardError> {
        self.record_validated_body(body)
    }
    #[cfg(test)]
    pub(crate) fn record_count_for_test(&self) -> u32 {
        let inner = self.inner.lock();
        let prepare_count = if inner.prepare_records.is_empty() {
            u32::from(inner.prepare_quarantine_view.is_some())
        } else {
            u32::try_from(inner.prepare_records.len()).unwrap_or(u32::MAX)
        };
        u32::try_from(inner.records.len())
            .unwrap_or(u32::MAX)
            .saturating_add(prepare_count)
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
#[cfg(unix)]
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
        native_amx_reject_unsupported_signer_journals(store_root, signer_digest, owner_uid)?;
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
fn native_amx_reject_unsupported_signer_journals(
    store_root: &Path,
    signer_digest: Hash,
    owner_uid: u32,
) -> Result<(), NativeAmxSigningGuardError> {
    for unsupported_name in NATIVE_AMX_UNSUPPORTED_SIGNING_GUARD_DIRECTORIES {
        let unsupported_root = store_root.join(unsupported_name);
        let unsupported_root_metadata = match fs::symlink_metadata(&unsupported_root) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => {
                return Err(native_amx_unsafe_journal(
                    &unsupported_root,
                    error.to_string(),
                ));
            }
        };
        native_amx_validate_secure_directory_metadata(
            &unsupported_root,
            &unsupported_root_metadata,
            owner_uid,
        )?;
        let unsupported_signer = unsupported_root.join(signer_digest.to_string());
        match fs::symlink_metadata(&unsupported_signer) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(native_amx_unsafe_journal(
                    &unsupported_signer,
                    error.to_string(),
                ));
            }
            Ok(metadata) => {
                native_amx_validate_secure_directory_metadata(
                    &unsupported_signer,
                    &metadata,
                    owner_uid,
                )?;
                return Err(native_amx_unsafe_journal(
                    &unsupported_signer,
                    "unsupported pre-release Native AMX signing evidence must not be reused",
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
#[cfg(unix)]
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
#[cfg(unix)]
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
#[cfg(unix)]
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
/// Full-plan request whose body and canonical legs expose any route-list drift.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct NativeAmxAttestationRequestV2 {
    /// Participant attestation body that will be signed after validation.
    pub body: NativeAmxAttestationBodyV2,
    /// Complete plan in coordinator-first canonical order.
    pub plan_legs: Vec<RouteLeg>,
    /// Coordinator proposal pre-commitment binding lane, committee, predecessor,
    /// and transactions, but not the receipt assembled from its votes.
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
    /// # Errors
    /// Rejects malformed or replay-substituted plan evidence.
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
        && !native_amx_hash_is_zero_sentinel(body.network_id.as_bytes())
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
    /// Validate phase, sender, shape, length, and BLS-normal identity without
    /// parsing or verifying attacker-controlled signature bytes.
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
    /// Validate phase, transport signer, BLS-normal identity, and vote signature;
    /// callers must still check live proof of possession at the planned height.
    /// # Errors
    /// Rejects phase, signer, identity, or canonical-preimage signature mismatch.
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
    /// Validate one certified participant leg's Prepare-to-Commit advance.
    /// # Errors
    /// Returns [`NativeAmxCommitRequestError`] for a wrong phase or any signed
    /// context, transaction, plan, route, or height mismatch.
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
    /// signer bitmap does not carry exactly the canonical committee quorum
    #[error("native AMX QC signer count mismatch: expected exactly {expected}, got {actual}")]
    SignerCountMismatch {
        /// Canonical signer count required by the committee.
        expected: usize,
        /// Signer count carried by the certificate bitmap.
        actual: usize,
    },
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
/// Validate a context-bound certificate against its frozen committee and PoPs.
/// # Errors
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
    if qc.validator_set() != validator_set {
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
    for (validator, embedded_pop) in qc.validators_with_pops() {
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
    if signer_keys.len() != min_signers {
        return Err(NativeAmxQcValidationError::SignerCountMismatch {
            expected: min_signers,
            actual: signer_keys.len(),
        });
    }
    iroha_crypto::bls_normal_verify_preaggregated_same_message(
        &expected_body.signature_preimage(),
        &qc.bls_aggregate_signature,
        &signer_keys,
        &signer_pops,
    )
    .map_err(|_| NativeAmxQcValidationError::InvalidAggregateSignature)
}
/// Validate a receipt's bounded, producer-hashable shape without cryptography or
/// state lookup; admission also checks route, authority, PoPs, and signatures.
#[must_use]
pub(crate) fn receipt_shape_matches_coordinator_payload(
    receipt: Option<&iroha_data_model::block::consensus::NativeAmxReceipt>,
    routing_plan: &RoutingPlan,
    expected_source_id: &[u8],
    expected_entrypoint_hash: Hash,
    expected_network_id: NetworkId,
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
        || receipt.network_id != expected_network_id
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
                let validator_count = qc.validator_set().len();
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
                    && body.network_id == expected_network_id
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
                    && qc.validator_set_hash == qc.computed_validator_set_hash()
                    && qc.validator_set_hash == body.participant_validator_set_hash
                    && qc.validator_set().windows(2).all(|pair| pair[0] < pair[1])
                    && qc.validator_set().iter().all(peer_uses_bls_normal)
                    && qc
                        .validator_set_pops()
                        .iter()
                        .all(|pop| pop.len() == NATIVE_AMX_BLS_PROOF_BYTES)
                    && qc.signers_bitmap.len() == validator_count.div_ceil(8)
                    && trailing_bits_clear
                    && signer_count == expected_quorum
                    && qc.bls_aggregate_signature.len() == NATIVE_AMX_BLS_PROOF_BYTES
            };
            if !common_qc_shape(prepare, NativeAmxPhase::Prepare)
                || !common_qc_shape(commit, NativeAmxPhase::Commit)
                || prepare.validator_set() != commit.validator_set()
                || prepare.validator_set_pops() != commit.validator_set_pops()
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
        && left.network_id == right.network_id
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
fn native_amx_bodies_share_durable_signing_decision(
    left: &NativeAmxAttestationBodyV2,
    right: &NativeAmxAttestationBodyV2,
) -> bool {
    let mut left = *left;
    let mut right = *right;
    left.round.view = 0;
    right.round.view = 0;
    left == right
}
/// Build a deterministic native AMX QC by projecting votes into validator order.
/// # Errors
/// Rejects malformed committee/thresholds, mismatched bodies, duplicate or
/// unknown signers, insufficient quorum, or invalid BLS-normal signatures.
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
        .take(min_signers)
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
    let validator_set_hash = HashOf::new(&validator_set);
    NativeAmxAttestationQcV2::try_new(
        body,
        VALIDATOR_SET_HASH_VERSION_V1,
        validator_set_hash,
        validator_set,
        validator_set_pops,
        signers_bitmap,
        bls_aggregate_signature,
    )
    .map_err(|_| NativeAmxQcBuildError::InvalidProofOfPossession)
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
    fn retain_view(&mut self, view: u64) {
        self.votes
            .retain(|bucket, _| bucket.body.round.view == view);
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
    /// Insert a vote for an exact attestation body.
    /// # Errors
    /// Returns duplicate-signer, plan-equivocation, or capacity errors rather
    /// than replacing safety-relevant evidence.
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
    /// Retain buckets for the certified view and retire orphaned source claims;
    /// the signing guard owns durable commits and recovered-prepare quarantine.
    pub(crate) fn retain_view(&mut self, view: u64) {
        self.sessions.retain(|_, session| {
            session.retain_view(view);
            !session.votes.is_empty()
        });
        let surviving_claims = self
            .sessions
            .keys()
            .map(|key| (key.source_id, key.plan_digest))
            .collect::<BTreeSet<_>>();
        self.source_plan_claims.retain(|source_id, plan_digest| {
            surviving_claims.contains(&(*source_id, *plan_digest))
        });
    }
    /// Retire every volatile vote bucket and source-plan claim at terminal height decision.
    pub(crate) fn clear(&mut self) {
        self.sessions.clear();
        self.source_plan_claims.clear();
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
    include!("native_amx/tests.rs");
}
