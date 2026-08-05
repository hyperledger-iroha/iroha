//! Durable, bounded tombstones for terminally retired Sumeragi v2 candidates.
//!
//! The safety WAL cannot contain these records: reducer persistence identifiers
//! are deliberately one-to-one with WAL frame sequence numbers.  This adjacent
//! snapshot therefore uses its own context-bound, checksummed frame and the
//! same write/sync/atomic-replace/directory-sync publication order as the
//! consensus sidecars. Ordinary successful service markers remain
//! process-generation local because proposal, quorum-pool, and body-pipeline
//! state can be volatile; restart must permit retransmission to reconstruct
//! that state.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File, OpenOptions},
    io::{ErrorKind, Read, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{block::consensus_v2 as wire, peer::PeerId};
use norito::codec::{Decode, DecodeAll, Encode};

use super::{
    FairV2IngressLeaderWireIdentity, FairV2IngressLeaderWireSlot,
    FairV2IngressLeaderWireSourceClass, FairV2IngressLeaderWireToken,
    v2_body_store::DurableBodyReceipt,
    v2_core::{
        CanonicalIdentityProjection, IDENTITY_DOMAIN_PROCESS_LOCAL,
        IDENTITY_KIND_LEADER_WIRE_LIFECYCLE, LEADER_WIRE_ADMISSION_COALESCE,
        LEADER_WIRE_ADMISSION_INSERT, LEADER_WIRE_ADMISSION_REACTIVATE,
        LEADER_WIRE_ADMISSION_REPLACE_TERMINAL, LEADER_WIRE_LIFECYCLE_ABSENT,
        LEADER_WIRE_LIFECYCLE_DORMANT, LEADER_WIRE_LIFECYCLE_INGRESS,
        LEADER_WIRE_LIFECYCLE_RUNTIME, LEADER_WIRE_LIFECYCLE_TERMINAL,
        LEADER_WIRE_LIFECYCLE_VOLATILE_TERMINAL, ProductionLeaderWireAdmissionTraceProjection,
        check_production_leader_wire_admission_transition,
    },
};

// Version 3 records only restart-safe terminal retirements. Version 4 adds a
// separate, equally bounded producer-continuation lifecycle table. Active
// records retain identity/slot/ordinal metadata but never claim to persist the
// command payload: restart normalizes them to selector-inert Dormant and
// admits exact replay under the same immutable identity. Version 2 snapshots
// contained volatile successful-service markers and must still fail closed
// instead of suppressing reconstruction after restart.
const FORMAT_VERSION_V3: u16 = 3;
const FORMAT_VERSION: u16 = 4;
const FRAME_MAGIC: &[u8; 8] = b"SUMVCAND";
const HASH_BYTES: usize = 32;
const FRAME_HEADER_BYTES: usize = FRAME_MAGIC.len() + 2 + 8 + HASH_BYTES;
const FIXED_FRAME_HEADROOM_BYTES: u64 = 4 * 1024;
const RECORD_FRAME_HEADROOM_BYTES: u64 = 192;
const PRODUCER_CONTINUATION_FRAME_HEADROOM_BYTES: u64 = 1024;
const MAX_PRODUCER_CONTINUATION_HANDOFFS: usize = 3;
const LEADER_WIRE_FORMAT_VERSION: u16 = 2;
const LEADER_WIRE_FRAME_MAGIC: &[u8; 8] = b"SUMVWIRE";
const LEADER_WIRE_RECORD_HEADROOM_BYTES: u64 = 8 * 1024;

/// Closed service-stage carrier shared by adapter policy and durable records.
pub(crate) const SERVICED_CANDIDATE_STAGES_PER_LIFECYCLE: usize = 11;

/// Project the persisted `DeferredEventKind` code onto the closed service
/// stage carrier. Internal callbacks and retry triggers are deliberately
/// untracked, so a decoded producer identity cannot invent a stage for them.
pub(crate) const fn serviced_candidate_stage_for_kind_code(kind: u8) -> Option<u8> {
    match kind {
        0..=6 => Some(kind),
        8 => Some(7),
        9 => Some(8),
        10 => Some(9),
        14 => Some(10),
        _ => None,
    }
}

/// Physical replay class for one drained producer occurrence.
///
/// This is node-local admission metadata, not a wire field. It is repeated in
/// the producer record so a decoded snapshot cannot silently reclassify a
/// transport-conditional or volatile-body occurrence as locally replayable.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode)]
pub(crate) enum ProducerContinuationSourceClass {
    /// A durable local body-pipeline, safety-WAL, or Decision root exists.
    Local,
    /// Replay depends on the explicitly assumed responsive transport corridor.
    ConditionalTransport,
    /// Replay depends on a concrete reconstructed-body completion owner.
    VolatileBody,
}

/// Project the persisted event-kind code onto its exact physical replay class.
pub(crate) const fn producer_continuation_source_class_for_kind_code(
    kind: u8,
) -> Option<ProducerContinuationSourceClass> {
    match kind {
        0 | 6 | 9 | 10 | 14 => Some(ProducerContinuationSourceClass::Local),
        1..=5 => Some(ProducerContinuationSourceClass::ConditionalTransport),
        8 => Some(ProducerContinuationSourceClass::VolatileBody),
        _ => None,
    }
}

/// Route-neutral identity of one reducer occurrence.
///
/// `context_id`, `height`, and `owner` deliberately repeat the snapshot header
/// binding, so a decoded record cannot be transplanted between otherwise
/// valid files or validators. The adapter may use this key transiently for any
/// successful service; this store persists it only after terminal retirement.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode)]
#[norito(deny_unknown_fields)]
pub(crate) struct ServicedCandidateKey {
    context_id: wire::HeightContextId,
    height: wire::Height,
    owner: [u8; 32],
    leader: wire::ValidatorIndex,
    source_view: wire::View,
    target: Option<[u8; 32]>,
    phase: u8,
    class: u8,
    kind: u8,
    evidence: [u8; 32],
}

impl ServicedCandidateKey {
    /// Construct a key from a fully validated, immutable semantic projection.
    #[allow(clippy::too_many_arguments)]
    pub(crate) const fn new(
        context_id: wire::HeightContextId,
        height: wire::Height,
        owner: [u8; 32],
        leader: wire::ValidatorIndex,
        source_view: wire::View,
        target: Option<[u8; 32]>,
        phase: u8,
        class: u8,
        kind: u8,
        evidence: [u8; 32],
    ) -> Self {
        Self {
            context_id,
            height,
            owner,
            leader,
            source_view,
            target,
            phase,
            class,
            kind,
            evidence,
        }
    }

    /// Leader derived from the semantic occurrence's source view.
    #[cfg(test)]
    pub(crate) const fn leader(self) -> wire::ValidatorIndex {
        self.leader
    }

    /// View carried by the semantic occurrence itself.
    pub(crate) const fn source_view(self) -> wire::View {
        self.source_view
    }

    /// Optional exact subject or highest-certificate target carried by the
    /// semantic occurrence.
    pub(crate) const fn target(self) -> Option<[u8; 32]> {
        self.target
    }

    /// Height context which prevents a terminal identity from crossing forks.
    pub(crate) const fn context_id(self) -> wire::HeightContextId {
        self.context_id
    }

    /// Exact height at which the semantic occurrence was consumed.
    pub(crate) const fn height(self) -> wire::Height {
        self.height
    }

    /// Validator-local owner bound into the durable snapshot header.
    pub(crate) const fn owner(self) -> [u8; 32] {
        self.owner
    }

    /// Protocol phase projected by the serviced reducer occurrence.
    pub(crate) const fn phase(self) -> u8 {
        self.phase
    }

    /// Closed reducer-event kind used to derive stage and replay class.
    pub(crate) const fn kind(self) -> u8 {
        self.kind
    }

    /// Semantic adapter lane which owned the serviced occurrence.
    #[cfg(test)]
    pub(crate) const fn class(self) -> u8 {
        self.class
    }

    fn belongs_to(
        self,
        context_id: wire::HeightContextId,
        height: wire::Height,
        owner: [u8; 32],
    ) -> bool {
        self.context_id == context_id && self.height == height && self.owner == owner
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode)]
#[norito(deny_unknown_fields)]
struct PersistedServicedCandidate {
    key: ServicedCandidateKey,
    /// Consumer episode metadata used only for strict-view reclamation.
    service_view: wire::View,
}

#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
#[norito(deny_unknown_fields)]
struct PersistedServicedCandidatesV3 {
    format_version: u16,
    context_id: wire::HeightContextId,
    height: wire::Height,
    owner: [u8; 32],
    capacity: u64,
    decision_reclaimed: bool,
    records: Vec<PersistedServicedCandidate>,
}

/// Node-local index into the immutable lifecycle-stage address space.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode)]
#[norito(deny_unknown_fields)]
pub(crate) struct ProducerContinuationAddress {
    /// Bounded slot assigned by the finite lifecycle-capacity allocator.
    lifecycle_slot: u64,
    /// Closed eleven-class reducer service-stage projection.
    stage: u8,
}

impl ProducerContinuationAddress {
    /// Bounded lifecycle slot reused only through terminal anti-ABA replacement.
    pub(crate) const fn lifecycle_slot(self) -> u64 {
        self.lifecycle_slot
    }

    /// Exact reducer service stage at this bounded slot.
    #[cfg(test)]
    pub(crate) const fn stage(self) -> u8 {
        self.stage
    }
}

/// Full route-neutral identity stored behind a node-local continuation index.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode)]
#[norito(deny_unknown_fields)]
pub(crate) struct ProducerContinuationIdentity {
    /// Exact context-bound reducer occurrence identity.
    candidate: ServicedCandidateKey,
    /// Immutable causal lifecycle identity, excluding process-local routing.
    causal_lifecycle_key: Hash,
    /// Bounded lifecycle allocator slot, repeated for fail-closed validation.
    lifecycle_slot: u64,
    /// Immutable first-admission ordinal, repeated for fail-closed validation.
    admission_ordinal: u128,
    /// Exact reducer service stage, repeated for fail-closed validation.
    stage: u8,
}

impl ProducerContinuationIdentity {
    /// Construct one identity from an allocator-owned bounded lifecycle slot.
    ///
    /// The stage is projected from the same closed event-kind mapping used by
    /// `serviced_candidate_stage`; callers cannot choose a foreign stage.
    pub(crate) fn new(
        candidate: ServicedCandidateKey,
        causal_lifecycle_key: Hash,
        lifecycle_slot: u64,
        admission_ordinal: u128,
    ) -> Result<Self, String> {
        if lifecycle_slot == 0 || admission_ordinal == 0 {
            return Err(
                "producer-continuation lifecycle slot and ordinal must be non-zero".to_owned(),
            );
        }
        let stage = serviced_candidate_stage_for_kind_code(candidate.kind).ok_or_else(|| {
            "producer-continuation candidate kind has no serviced stage".to_owned()
        })?;
        Ok(Self {
            candidate,
            causal_lifecycle_key,
            lifecycle_slot,
            admission_ordinal,
            stage,
        })
    }

    /// Project the bounded node-local address used only as the table index.
    pub(crate) const fn address(self) -> ProducerContinuationAddress {
        ProducerContinuationAddress {
            lifecycle_slot: self.lifecycle_slot,
            stage: self.stage,
        }
    }

    /// Exact serviced candidate frozen by this continuation.
    pub(crate) const fn candidate(self) -> ServicedCandidateKey {
        self.candidate
    }

    /// Immutable causal lifecycle identity shared by exact successors.
    pub(crate) const fn causal_lifecycle_key(self) -> Hash {
        self.causal_lifecycle_key
    }

    /// Immutable first-admission ordinal used for anti-ABA replacement.
    pub(crate) const fn admission_ordinal(self) -> u128 {
        self.admission_ordinal
    }

    /// Exact reducer service-stage projection.
    pub(crate) const fn stage(self) -> u8 {
        self.stage
    }

    fn has_exact_stage(self) -> bool {
        serviced_candidate_stage_for_kind_code(self.candidate.kind) == Some(self.stage)
    }
}

/// Monotone producer-continuation lifecycle state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode)]
pub(crate) enum ProducerContinuationStatus {
    /// Frozen handoff awaits an exact owned successor or semantic retirement.
    Reserved,
    /// One exact nonempty successor owner was observed for one bookkeeping turn.
    Materialized,
    /// Durable high-watermark prevents resurrection at the retired address.
    Terminal,
}

/// Opaque capability naming one exact producer reservation at the runtime cut.
///
/// The token itself is process-local. Its complete identity is also stored in
/// the bounded continuation table, so the adapter can reject an altered or
/// stale acknowledgement and can reconstruct a read-only terminal token after
/// restart. Neither the token nor any of its fields is added to a wire format.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ProducerContinuationHandoffToken {
    identity: ProducerContinuationIdentity,
    source_class: ProducerContinuationSourceClass,
}

impl ProducerContinuationHandoffToken {
    fn from_reserved(record: &ProducerContinuationRecord) -> Option<Self> {
        (record.status == ProducerContinuationStatus::Reserved).then_some(Self {
            identity: record.identity,
            source_class: record.source_class,
        })
    }

    /// Full immutable identity of the drained producer occurrence.
    pub(crate) const fn identity(self) -> ProducerContinuationIdentity {
        self.identity
    }

    /// Bounded node-local address occupied by the reservation.
    pub(crate) const fn address(self) -> ProducerContinuationAddress {
        self.identity.address()
    }

    /// Physical source class frozen before the source can retire.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) const fn source_class(self) -> ProducerContinuationSourceClass {
        self.source_class
    }

    /// Whether this capability still names the exact reserved record.
    pub(crate) fn matches_reserved(self, record: &ProducerContinuationRecord) -> bool {
        record.status == ProducerContinuationStatus::Reserved
            && record.identity == self.identity
            && record.source_class == self.source_class
    }
}

/// Restart-stable read-only proof that one exact producer identity is terminal.
///
/// This token can only be reconstructed from a validated v4 terminal record.
/// It lets outer ingress ownership reconcile a replay against durable
/// consumer memory without consulting process-local ordinals or ambient flags.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode)]
#[norito(deny_unknown_fields)]
pub(crate) struct ProducerContinuationTerminalToken {
    identity: ProducerContinuationIdentity,
    source_class: ProducerContinuationSourceClass,
}

impl ProducerContinuationTerminalToken {
    fn from_terminal(record: &ProducerContinuationRecord) -> Option<Self> {
        (record.status == ProducerContinuationStatus::Terminal).then_some(Self {
            identity: record.identity,
            source_class: record.source_class,
        })
    }

    /// Full immutable identity which cannot be resurrected at its old stage.
    pub(crate) const fn identity(self) -> ProducerContinuationIdentity {
        self.identity
    }

    /// Physical replay class frozen before terminal publication.
    pub(crate) const fn source_class(self) -> ProducerContinuationSourceClass {
        self.source_class
    }
}

/// Outcome of reserving one bounded lifecycle-stage address.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ProducerContinuationReservation {
    /// A previously free bounded address was reserved.
    Inserted,
    /// The exact immutable identity and record were already present.
    Coalesced,
    /// A terminal older-view address was reused by a newer lifecycle.
    ReplacedTerminal,
}

/// Persisted value for one exact producer-continuation lifecycle.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
#[norito(deny_unknown_fields)]
pub(crate) struct ProducerContinuationRecord {
    /// Full route-neutral identity; the map key is never treated as identity.
    identity: ProducerContinuationIdentity,
    /// Exact physical replay class derived from the immutable candidate kind.
    source_class: ProducerContinuationSourceClass,
    /// Monotone handoff status.
    status: ProducerContinuationStatus,
    /// Canonically ordered exact successors emitted by the drained parent.
    handoff_candidates: Vec<ProducerContinuationIdentity>,
}

impl ProducerContinuationRecord {
    /// Construct a record from one exact identity and its frozen successors.
    pub(crate) fn new(
        identity: ProducerContinuationIdentity,
        status: ProducerContinuationStatus,
        handoff_candidates: Vec<ProducerContinuationIdentity>,
    ) -> Result<Self, String> {
        let source_class =
            producer_continuation_source_class_for_kind_code(identity.candidate.kind).ok_or_else(
                || "producer-continuation candidate kind has no physical replay class".to_owned(),
            )?;
        if !identity.has_exact_stage()
            || handoff_candidates.len() > MAX_PRODUCER_CONTINUATION_HANDOFFS
            || handoff_candidates.windows(2).any(|pair| pair[0] >= pair[1])
            || handoff_candidates.iter().any(|successor| {
                !successor.has_exact_stage()
                    || successor.admission_ordinal != identity.admission_ordinal
                    || successor.lifecycle_slot != identity.lifecycle_slot
                    || successor.causal_lifecycle_key != identity.causal_lifecycle_key
                    || successor.candidate.context_id != identity.candidate.context_id
                    || successor.candidate.height != identity.candidate.height
                    || successor.candidate.owner != identity.candidate.owner
                    || *successor == identity
            })
            || status == ProducerContinuationStatus::Materialized && handoff_candidates.is_empty()
        {
            return Err(
                "producer-continuation record is not an exact canonical handoff".to_owned(),
            );
        }
        Ok(Self {
            identity,
            source_class,
            status,
            handoff_candidates,
        })
    }

    /// Full route-neutral identity of the drained candidate.
    pub(crate) const fn identity(&self) -> ProducerContinuationIdentity {
        self.identity
    }

    /// Physical replay class frozen with the drained producer occurrence.
    pub(crate) const fn source_class(&self) -> ProducerContinuationSourceClass {
        self.source_class
    }

    /// Monotone lifecycle status.
    pub(crate) const fn status(&self) -> ProducerContinuationStatus {
        self.status
    }

    /// Mint the exact acknowledgement capability for a live reservation.
    pub(crate) fn handoff_token(&self) -> Option<ProducerContinuationHandoffToken> {
        ProducerContinuationHandoffToken::from_reserved(self)
    }

    /// Reconstruct the read-only durable proof for a terminal record.
    pub(crate) fn terminal_token(&self) -> Option<ProducerContinuationTerminalToken> {
        ProducerContinuationTerminalToken::from_terminal(self)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
#[norito(deny_unknown_fields)]
struct PersistedProducerContinuation {
    address: ProducerContinuationAddress,
    record: ProducerContinuationRecord,
}

#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
#[norito(deny_unknown_fields)]
struct PersistedServicedCandidatesV4 {
    format_version: u16,
    context_id: wire::HeightContextId,
    height: wire::Height,
    owner: [u8; 32],
    serviced_capacity: u64,
    producer_continuation_capacity: u64,
    decision_reclaimed: bool,
    records: Vec<PersistedServicedCandidate>,
    producer_continuations: Vec<PersistedProducerContinuation>,
}

struct DecodedServicedCandidates {
    context_id: wire::HeightContextId,
    height: wire::Height,
    owner: [u8; 32],
    serviced_capacity: u64,
    producer_continuation_capacity: u64,
    decision_reclaimed: bool,
    records: Vec<PersistedServicedCandidate>,
    producer_continuations: Vec<PersistedProducerContinuation>,
}

/// Restored tombstone set and its one-shot durable-Decision reclamation flag.
pub(crate) struct RestoredServicedCandidates {
    /// Canonically ordered, context-bound records.
    pub(crate) records: BTreeMap<ServicedCandidateKey, wire::View>,
    /// Canonically indexed producer continuations restored for this height.
    pub(crate) producer_continuations:
        BTreeMap<ProducerContinuationAddress, ProducerContinuationRecord>,
    /// Whether the pre-Decision epoch has already been reclaimed.
    pub(crate) decision_reclaimed: bool,
}

/// Durable position of one generic productive leader-wire lifecycle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode)]
pub(crate) enum LeaderWireLifecycleStatus {
    /// Restart-restored exact identity with no surviving physical carrier.
    Dormant,
    /// Physical bytes and one fair-ingress slot now own the token.
    Ingress,
    /// The serialized runtime owns the exact downstream lifecycle.
    Runtime,
    /// Same-process consumer departure without restart-stable evidence.
    ///
    /// Exact retries coalesce while this process is alive. Restart always
    /// reopens the same identity and ordinals as selector-dormant Dormant.
    VolatileTerminal,
    /// Independently verified durable evidence suppresses resurrection.
    Terminal,
}

impl LeaderWireLifecycleStatus {
    /// Whether this logical lifecycle still blocks replacement of its slot.
    ///
    /// Dormant is live for anti-ABA ownership but owns no selector turn until
    /// exact atomic readmission publishes `Ingress`.
    const fn is_active(self) -> bool {
        matches!(self, Self::Dormant | Self::Ingress | Self::Runtime)
    }

    const fn refinement_code(self) -> u8 {
        match self {
            Self::Dormant => LEADER_WIRE_LIFECYCLE_DORMANT,
            Self::Ingress => LEADER_WIRE_LIFECYCLE_INGRESS,
            Self::Runtime => LEADER_WIRE_LIFECYCLE_RUNTIME,
            Self::VolatileTerminal => LEADER_WIRE_LIFECYCLE_VOLATILE_TERMINAL,
            Self::Terminal => LEADER_WIRE_LIFECYCLE_TERMINAL,
        }
    }
}

/// Exact serialized-runtime identity bound when ingress drains successfully.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode)]
#[norito(deny_unknown_fields)]
pub(crate) struct LeaderWireRuntimeOwner {
    causal_lifecycle_key: Hash,
    admission_ordinal: u128,
}

impl LeaderWireRuntimeOwner {
    /// Construct a non-zero downstream owner for one exact producer lifecycle.
    pub(crate) fn new(causal_lifecycle_key: Hash, admission_ordinal: u128) -> Result<Self, String> {
        if admission_ordinal == 0 {
            return Err("leader-wire runtime admission ordinal must be non-zero".to_owned());
        }
        Ok(Self {
            causal_lifecycle_key,
            admission_ordinal,
        })
    }

    /// Immutable causal key shared with the adapter producer reservation.
    pub(crate) const fn causal_lifecycle_key(self) -> Hash {
        self.causal_lifecycle_key
    }

    /// Immutable runtime admission ordinal restored after a crash.
    pub(crate) const fn admission_ordinal(self) -> u128 {
        self.admission_ordinal
    }
}

/// Persisted projection of an independently durable body receipt.
///
/// Construction requires the non-forgeable receipt returned by `V2BodyStore`.
/// On restart the projection is compared with the recovery catalog from a
/// separately opened body store; the gate snapshot is never its own authority.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode)]
#[norito(deny_unknown_fields)]
pub(crate) struct LeaderWireDurableBodyTerminalEvidence {
    context_id: wire::HeightContextId,
    height: wire::Height,
    owner: [u8; 32],
    round: wire::ConsensusRound,
    subject: wire::BlockSubject,
    manifest_hash: HashOf<wire::PayloadManifest>,
    frame_hash: Hash,
    runtime_owner: LeaderWireRuntimeOwner,
}

impl LeaderWireDurableBodyTerminalEvidence {
    fn from_receipt(
        receipt: &DurableBodyReceipt,
        owner: [u8; 32],
        runtime_owner: LeaderWireRuntimeOwner,
    ) -> Self {
        Self {
            context_id: receipt.context_id(),
            height: receipt.round().height,
            owner,
            round: receipt.round(),
            subject: receipt.subject(),
            manifest_hash: receipt.manifest_hash(),
            frame_hash: receipt.frame_hash(),
            runtime_owner,
        }
    }

    fn matches_receipt(&self, receipt: &DurableBodyReceipt) -> bool {
        self.context_id == receipt.context_id()
            && self.height == receipt.round().height
            && self.round == receipt.round()
            && self.subject == receipt.subject()
            && self.manifest_hash == receipt.manifest_hash()
            && self.frame_hash == receipt.frame_hash()
    }
}

/// Restart-stable terminal authority for one generic leader-wire lifecycle.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode)]
pub(crate) enum LeaderWireStableTerminalEvidence {
    /// The adapter first published the exact producer continuation terminal.
    Producer(ProducerContinuationTerminalToken),
    /// Exact proposal bytes exist in the independently recovered body store.
    DurableBody(LeaderWireDurableBodyTerminalEvidence),
}

impl From<ProducerContinuationTerminalToken> for LeaderWireStableTerminalEvidence {
    fn from(terminal: ProducerContinuationTerminalToken) -> Self {
        Self::Producer(terminal)
    }
}

/// Opaque durable epoch boundary reconstructed by the already-opened adapter.
///
/// The generic ingress snapshot is opened only after safety-WAL replay. This
/// capability lets it retire records made obsolete by a certified view
/// advance or durable Decision without treating its own terminal projection as
/// authority. It is process-local and never enters either snapshot format.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct LeaderWireRecoveryAuthority {
    context_id: wire::HeightContextId,
    height: wire::Height,
    owner: [u8; 32],
    durable_view: wire::View,
    decision_durable: bool,
}

impl LeaderWireRecoveryAuthority {
    /// Mint the recovery cut only from a replay-complete adapter.
    pub(super) const fn from_replayed_adapter(
        context_id: wire::HeightContextId,
        height: wire::Height,
        owner: [u8; 32],
        durable_view: wire::View,
        decision_durable: bool,
    ) -> Self {
        Self {
            context_id,
            height,
            owner,
            durable_view,
            decision_durable,
        }
    }

    fn matches_geometry(
        self,
        context_id: wire::HeightContextId,
        height: wire::Height,
        owner: [u8; 32],
    ) -> bool {
        self.context_id == context_id && self.height == height && self.owner == owner
    }

    /// Advance this WAL-derived authority to a certified durable view.
    pub(super) fn advance_view(self, durable_view: wire::View) -> Result<Self, String> {
        if durable_view < self.durable_view {
            return Err("leader-wire recovery authority regressed its durable view".to_owned());
        }
        Ok(Self {
            durable_view,
            ..self
        })
    }

    /// Refine this WAL-derived authority after Decision is durable.
    pub(super) const fn with_durable_decision(self) -> Self {
        Self {
            decision_durable: true,
            ..self
        }
    }

    fn monotonically_extends(self, previous: Self) -> bool {
        self.context_id == previous.context_id
            && self.height == previous.height
            && self.owner == previous.owner
            && self.durable_view >= previous.durable_view
            && (!previous.decision_durable || self.decision_durable)
    }

    /// Return whether this durable cut permanently rejects one lifecycle token.
    pub(super) fn obsoletes(self, token: &FairV2IngressLeaderWireToken) -> bool {
        // A certified view or Decision closes reducer-producing control, not
        // transport completion. The selected block can still be missing when
        // Decision becomes durable, so its exact chunk/body response must
        // reach the downstream fetch, manifest, request, and subject checks.
        token.source_class == FairV2IngressLeaderWireSourceClass::Control
            && (self.decision_durable || token.identity.view < self.durable_view)
    }

    fn obsoletes_identity(self, identity: &FairV2IngressLeaderWireIdentity) -> bool {
        identity.phase.source_class() == FairV2IngressLeaderWireSourceClass::Control
            && (self.decision_durable || identity.view < self.durable_view)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
#[norito(deny_unknown_fields)]
struct PersistedLeaderWireLifecycleRecord {
    token: FairV2IngressLeaderWireToken,
    status: LeaderWireLifecycleStatus,
    runtime_owner: Option<LeaderWireRuntimeOwner>,
    terminal_evidence: Option<LeaderWireStableTerminalEvidence>,
}

#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
#[norito(deny_unknown_fields)]
struct PersistedLeaderWireLifecycles {
    format_version: u16,
    context_id: wire::HeightContextId,
    height: wire::Height,
    owner: [u8; 32],
    capacity: u64,
    max_chunk_count: u32,
    last_admission_ordinal: u64,
    scheduler_ordinal_high_watermark: u128,
    records: Vec<PersistedLeaderWireLifecycleRecord>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct LeaderWireLifecycleState {
    /// Current process-local safety-WAL cut. The snapshot never serializes it;
    /// replay mints a fresh authority before this store is opened.
    recovery_authority: LeaderWireRecoveryAuthority,
    last_admission_ordinal: u64,
    scheduler_ordinal_high_watermark: u128,
    records: BTreeMap<FairV2IngressLeaderWireSlot, PersistedLeaderWireLifecycleRecord>,
    /// Active records recovered without a surviving physical ingress carrier.
    ///
    /// This set is deliberately process-local. Its members retain their
    /// immutable durable tokens, but they do not own a scheduler barrier until
    /// the exact packet passes current ingress capacity checks and
    /// `admit_ingress` atomically reactivates the slot. A newer live safety-WAL
    /// cut can retire them first without requiring requester retransmission.
    replay_dormant: BTreeSet<FairV2IngressLeaderWireSlot>,
}

fn leader_wire_lifecycle_identity_projection(
    token: &FairV2IngressLeaderWireToken,
) -> CanonicalIdentityProjection {
    let identity = token.identity_hash();
    CanonicalIdentityProjection::from_bytes(
        IDENTITY_DOMAIN_PROCESS_LOCAL,
        IDENTITY_KIND_LEADER_WIRE_LIFECYCLE,
        *identity.as_ref(),
    )
}

fn leader_wire_admission_trace_projection(
    state: &LeaderWireLifecycleState,
    capacity: usize,
    token: &FairV2IngressLeaderWireToken,
    incumbent: Option<&PersistedLeaderWireLifecycleRecord>,
    operation: u8,
) -> Result<ProductionLeaderWireAdmissionTraceProjection, String> {
    let records_before = u64::try_from(state.records.len())
        .map_err(|_| "leader-wire lifecycle record count is not representable".to_owned())?;
    let capacity = u64::try_from(capacity)
        .map_err(|_| "leader-wire lifecycle capacity is not representable".to_owned())?;
    let records_after = if operation == LEADER_WIRE_ADMISSION_INSERT {
        records_before
            .checked_add(1)
            .ok_or_else(|| "leader-wire lifecycle record count overflowed".to_owned())?
    } else {
        records_before
    };
    let incoming_identity = leader_wire_lifecycle_identity_projection(token);
    let incumbent_identity = incumbent.map_or_else(CanonicalIdentityProjection::zero, |record| {
        leader_wire_lifecycle_identity_projection(&record.token)
    });
    let status_before = incumbent
        .map(|record| record.status.refinement_code())
        .unwrap_or(LEADER_WIRE_LIFECYCLE_ABSENT);
    let status_after = incumbent
        .filter(|_| operation == LEADER_WIRE_ADMISSION_COALESCE)
        .map(|record| record.status.refinement_code())
        .unwrap_or(LEADER_WIRE_LIFECYCLE_INGRESS);
    let last_admission_ordinal_before = u128::from(state.last_admission_ordinal);
    let updates_high_watermarks = matches!(
        operation,
        LEADER_WIRE_ADMISSION_INSERT | LEADER_WIRE_ADMISSION_REPLACE_TERMINAL
    );
    let runtime_owner_before = incumbent.is_some_and(|record| record.runtime_owner.is_some());
    let terminal_evidence_before =
        incumbent.is_some_and(|record| record.terminal_evidence.is_some());
    let retains_incumbent_runtime_owner = matches!(
        operation,
        LEADER_WIRE_ADMISSION_COALESCE | LEADER_WIRE_ADMISSION_REACTIVATE
    );
    let retains_incumbent_ordinals = matches!(
        operation,
        LEADER_WIRE_ADMISSION_COALESCE | LEADER_WIRE_ADMISSION_REACTIVATE
    );
    let stored_admission_ordinal = if retains_incumbent_ordinals {
        incumbent
            .map(|record| u128::from(record.token.admission_ordinal))
            .ok_or_else(|| {
                "leader-wire retry projection lost its incumbent admission ordinal".to_owned()
            })?
    } else {
        u128::from(token.admission_ordinal)
    };
    let stored_scheduler_ordinal = if retains_incumbent_ordinals {
        incumbent
            .map(|record| record.token.scheduler_ordinal)
            .ok_or_else(|| {
                "leader-wire retry projection lost its incumbent scheduler ordinal".to_owned()
            })?
    } else {
        token.scheduler_ordinal
    };
    Ok(ProductionLeaderWireAdmissionTraceProjection {
        operation,
        incoming_identity,
        incumbent_identity,
        stored_identity: incoming_identity,
        incoming_view: token.identity.view,
        incumbent_view: incumbent.map_or(0, |record| record.token.identity.view),
        stored_view: token.identity.view,
        incoming_admission_ordinal: u128::from(token.admission_ordinal),
        incumbent_admission_ordinal: incumbent
            .map_or(0, |record| u128::from(record.token.admission_ordinal)),
        stored_admission_ordinal,
        incoming_scheduler_ordinal: token.scheduler_ordinal,
        incumbent_scheduler_ordinal: incumbent.map_or(0, |record| record.token.scheduler_ordinal),
        stored_scheduler_ordinal,
        last_admission_ordinal_before,
        last_admission_ordinal_after: if updates_high_watermarks {
            u128::from(token.admission_ordinal)
        } else {
            last_admission_ordinal_before
        },
        scheduler_ordinal_high_watermark_before: state.scheduler_ordinal_high_watermark,
        scheduler_ordinal_high_watermark_after: if updates_high_watermarks {
            token.scheduler_ordinal
        } else {
            state.scheduler_ordinal_high_watermark
        },
        records_before,
        records_after,
        capacity,
        status_before,
        status_after,
        replay_dormant_before: state.replay_dormant.contains(&token.slot),
        replay_dormant_after: false,
        runtime_owner_before,
        runtime_owner_after: retains_incumbent_runtime_owner && runtime_owner_before,
        terminal_evidence_before,
        terminal_evidence_after: operation == LEADER_WIRE_ADMISSION_COALESCE
            && terminal_evidence_before,
    })
}

/// Validated restart image for binding the fair-ingress in-memory owner table.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct LeaderWireLifecycleRestore {
    last_admission_ordinal: u64,
    scheduler_ordinal_high_watermark: u128,
    records: Vec<LeaderWireLifecycleRestoredRecord>,
}

impl LeaderWireLifecycleRestore {
    /// Largest immutable fair-ingress ordinal present in the snapshot.
    pub(crate) const fn last_admission_ordinal(&self) -> u64 {
        self.last_admission_ordinal
    }

    /// Largest actor-global scheduler ordinal reserved by this gate.
    pub(crate) const fn scheduler_ordinal_high_watermark(&self) -> u128 {
        self.scheduler_ordinal_high_watermark
    }

    /// Canonically ordered lifecycle records; active statuses are
    /// selector-dormant Dormant until exact physical replay.
    pub(crate) fn records(&self) -> &[LeaderWireLifecycleRestoredRecord] {
        &self.records
    }
}

/// One exact record returned by a validated leader-wire restore.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct LeaderWireLifecycleRestoredRecord {
    token: FairV2IngressLeaderWireToken,
    status: LeaderWireLifecycleStatus,
    runtime_owner: Option<LeaderWireRuntimeOwner>,
    terminal_evidence: Option<LeaderWireStableTerminalEvidence>,
}

impl LeaderWireLifecycleRestoredRecord {
    /// Exact generic token retaining its original fair-ingress ordinal.
    pub(crate) const fn token(&self) -> &FairV2IngressLeaderWireToken {
        &self.token
    }

    /// Restored status; every nonterminal disk status reopens as Dormant.
    pub(crate) const fn status(&self) -> LeaderWireLifecycleStatus {
        self.status
    }

    /// Prior runtime owner used to rebind a replay to its old producer slot.
    pub(crate) const fn runtime_owner(&self) -> Option<LeaderWireRuntimeOwner> {
        self.runtime_owner
    }

    /// Typed stable evidence, present exactly for durable Terminal records.
    pub(crate) const fn terminal_evidence(&self) -> Option<&LeaderWireStableTerminalEvidence> {
        self.terminal_evidence.as_ref()
    }
}

/// Result of atomically admitting a generic leader-wire identity to ingress.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct LeaderWireLifecycleAdmissionReceipt {
    token: FairV2IngressLeaderWireToken,
    status: LeaderWireLifecycleStatus,
    inserted: bool,
}

impl LeaderWireLifecycleAdmissionReceipt {
    /// Persisted token; an exact retry receives the incumbent old ordinal.
    pub(crate) const fn token(&self) -> &FairV2IngressLeaderWireToken {
        &self.token
    }

    /// Status observed at admission, including terminal suppression.
    pub(crate) const fn status(&self) -> LeaderWireLifecycleStatus {
        self.status
    }

    /// Whether this call allocated a new bounded lifecycle record.
    pub(crate) const fn inserted(&self) -> bool {
        self.inserted
    }
}

/// Receipt proving that ingress-to-runtime transfer was durably recorded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct LeaderWireLifecycleRuntimeReceipt {
    token: FairV2IngressLeaderWireToken,
    owner: LeaderWireRuntimeOwner,
}

impl LeaderWireLifecycleRuntimeReceipt {
    /// Generic ingress token transferred to the runtime.
    pub(crate) const fn token(&self) -> &FairV2IngressLeaderWireToken {
        &self.token
    }

    /// Exact downstream owner which must match producer acknowledgement.
    pub(crate) const fn owner(&self) -> LeaderWireRuntimeOwner {
        self.owner
    }
}

/// Context-bound synchronous persistence gate shared with fair ingress.
///
/// `admit_ingress` synchronizes the Ingress record before admission may report
/// Accepted. Later status transitions use the same atomic-replace file. The
/// adapter producer terminal is published first; `mark_terminal` then commits
/// the generic Terminal. Body-backed terminals are checked against receipts
/// reconstructed by a separately opened body store. On restart matching
/// producer/body evidence completes a crash-between-files Runtime record,
/// volatile terminals reopen, and every other active record returns to
/// selector-dormant Dormant with its original ordinals.
#[derive(Debug)]
pub(crate) struct LeaderWireLifecycleStoreGate {
    path: PathBuf,
    context_id: wire::HeightContextId,
    height: wire::Height,
    owner: [u8; 32],
    roster: BTreeSet<PeerId>,
    capacity: usize,
    max_chunk_count: u32,
    max_frame_bytes: u64,
    state: Mutex<LeaderWireLifecycleState>,
}

/// Atomic per-height snapshot stored beside the safety WAL.
#[derive(Debug)]
pub(crate) struct ServicedCandidateStore {
    path: PathBuf,
    context_id: wire::HeightContextId,
    height: wire::Height,
    owner: [u8; 32],
    serviced_capacity: usize,
    producer_continuation_capacity: usize,
    producer_continuation_lifecycle_capacity: u64,
    max_frame_bytes: u64,
}

#[cfg(unix)]
type SnapshotIdentity = (u64, u64);
#[cfg(windows)]
type SnapshotIdentity = (Option<u32>, Option<u64>);
#[cfg(not(any(unix, windows)))]
type SnapshotIdentity = ();

#[cfg(unix)]
type SnapshotRevision = (u64, i64, i64, i64, i64, u64, u32, u32, u32);
#[cfg(windows)]
type SnapshotRevision = (u64, u64, u64, u32, Option<u32>);
#[cfg(not(any(unix, windows)))]
type SnapshotRevision = ();

#[cfg(unix)]
fn snapshot_identity(metadata: &fs::Metadata) -> SnapshotIdentity {
    use std::os::unix::fs::MetadataExt as _;

    (metadata.dev(), metadata.ino())
}

#[cfg(windows)]
fn snapshot_identity(metadata: &fs::Metadata) -> SnapshotIdentity {
    use std::os::windows::fs::MetadataExt as _;

    (metadata.volume_serial_number(), metadata.file_index())
}

#[cfg(not(any(unix, windows)))]
fn snapshot_identity(_metadata: &fs::Metadata) -> SnapshotIdentity {}

#[cfg(unix)]
fn snapshot_revision(metadata: &fs::Metadata) -> SnapshotRevision {
    use std::os::unix::fs::MetadataExt as _;

    (
        metadata.len(),
        metadata.mtime(),
        metadata.mtime_nsec(),
        metadata.ctime(),
        metadata.ctime_nsec(),
        metadata.nlink(),
        metadata.mode(),
        metadata.uid(),
        metadata.gid(),
    )
}

#[cfg(windows)]
fn snapshot_revision(metadata: &fs::Metadata) -> SnapshotRevision {
    use std::os::windows::fs::MetadataExt as _;

    (
        metadata.file_size(),
        metadata.creation_time(),
        metadata.last_write_time(),
        metadata.file_attributes(),
        metadata.number_of_links(),
    )
}

#[cfg(not(any(unix, windows)))]
fn snapshot_revision(_metadata: &fs::Metadata) -> SnapshotRevision {}

#[cfg(unix)]
const fn snapshot_identity_available(_identity: SnapshotIdentity) -> bool {
    true
}

#[cfg(windows)]
const fn snapshot_identity_available(identity: SnapshotIdentity) -> bool {
    identity.0.is_some() && identity.1.is_some()
}

#[cfg(not(any(unix, windows)))]
const fn snapshot_identity_available(_identity: SnapshotIdentity) -> bool {
    false
}

fn snapshot_is_single_link(metadata: &fs::Metadata) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;

        metadata.nlink() == 1
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt as _;

        metadata.number_of_links() == Some(1)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = metadata;
        false
    }
}

#[cfg(windows)]
fn snapshot_is_reparse_point(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt as _;

    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn snapshot_is_reparse_point(_metadata: &fs::Metadata) -> bool {
    false
}

fn snapshot_metadata_is_safe(metadata: &fs::Metadata, max_frame_bytes: u64) -> bool {
    let identity = snapshot_identity(metadata);
    !metadata.file_type().is_symlink()
        && !snapshot_is_reparse_point(metadata)
        && metadata.is_file()
        && snapshot_identity_available(identity)
        && snapshot_is_single_link(metadata)
        && metadata.len() <= max_frame_bytes
}

fn snapshot_metadata_unchanged(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    let identity = snapshot_identity(left);
    snapshot_identity_available(identity)
        && identity == snapshot_identity(right)
        && snapshot_revision(left) == snapshot_revision(right)
}

#[cfg(any(unix, windows))]
fn open_snapshot_nofollow(path: &Path) -> std::io::Result<File> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;

        options.custom_flags(rustix::fs::OFlags::NOFOLLOW.bits() as i32);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt as _;

        const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
        options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    }
    options.open(path)
}

#[cfg(not(any(unix, windows)))]
fn open_snapshot_nofollow(_path: &Path) -> std::io::Result<File> {
    Err(std::io::Error::new(
        ErrorKind::Unsupported,
        "stable serviced-candidate file identities are unsupported on this platform",
    ))
}

fn open_bound_snapshot(
    path: &Path,
    max_frame_bytes: u64,
) -> Result<Option<(File, fs::Metadata)>, String> {
    let path_before = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(format!(
                "failed to inspect serviced-candidate snapshot {}: {error}",
                path.display()
            ));
        }
    };
    if !snapshot_metadata_is_safe(&path_before, max_frame_bytes) {
        return Err(format!(
            "serviced-candidate snapshot {} is not a bounded direct single-link regular file",
            path.display()
        ));
    }
    let file = open_snapshot_nofollow(path).map_err(|error| {
        format!(
            "failed to open serviced-candidate snapshot {} without following links: {error}",
            path.display()
        )
    })?;
    let opened = file.metadata().map_err(|error| {
        format!(
            "failed to inspect opened serviced-candidate snapshot {}: {error}",
            path.display()
        )
    })?;
    if !snapshot_metadata_is_safe(&opened, max_frame_bytes)
        || !snapshot_metadata_unchanged(&path_before, &opened)
    {
        return Err(format!(
            "serviced-candidate snapshot {} changed identity while opening",
            path.display()
        ));
    }
    Ok(Some((file, opened)))
}

fn producer_continuations_are_valid(
    persisted: &[PersistedProducerContinuation],
    context_id: wire::HeightContextId,
    height: wire::Height,
    owner: [u8; 32],
    lifecycle_capacity: u64,
) -> bool {
    if persisted
        .windows(2)
        .any(|pair| pair[0].address >= pair[1].address)
    {
        return false;
    }
    let mut identities = BTreeSet::new();
    let mut candidate_identities = BTreeSet::new();
    let mut ordinal_stages = BTreeSet::new();
    let mut active_ordinals = BTreeSet::new();
    for persisted in persisted {
        let record = &persisted.record;
        let identity = record.identity;
        if identity.address() != persisted.address
            || identity.admission_ordinal == 0
            || identity.lifecycle_slot == 0
            || identity.lifecycle_slot > lifecycle_capacity
            || !identity.has_exact_stage()
            || producer_continuation_source_class_for_kind_code(identity.candidate.kind)
                != Some(record.source_class)
            || !identity.candidate.belongs_to(context_id, height, owner)
            || !identities.insert(identity)
            || !candidate_identities.insert(identity.candidate)
            || !ordinal_stages.insert((identity.admission_ordinal, identity.stage))
            || record.handoff_candidates.len() > MAX_PRODUCER_CONTINUATION_HANDOFFS
            || record
                .handoff_candidates
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || record.handoff_candidates.iter().any(|successor| {
                successor.admission_ordinal != identity.admission_ordinal
                    || successor.lifecycle_slot != identity.lifecycle_slot
                    || !successor.has_exact_stage()
                    || successor.causal_lifecycle_key != identity.causal_lifecycle_key
                    || !successor.candidate.belongs_to(context_id, height, owner)
                    || *successor == identity
            })
            || record.status == ProducerContinuationStatus::Materialized
                && record.handoff_candidates.is_empty()
            || record.status != ProducerContinuationStatus::Terminal
                && !active_ordinals.insert(identity.admission_ordinal)
        {
            return false;
        }
    }
    true
}

fn leader_wire_terminal_matches_runtime(
    terminal: ProducerContinuationTerminalToken,
    token: &FairV2IngressLeaderWireToken,
    runtime_owner: LeaderWireRuntimeOwner,
    context_id: wire::HeightContextId,
    height: wire::Height,
    owner: [u8; 32],
) -> bool {
    let identity = terminal.identity();
    let candidate = identity.candidate();
    candidate.context_id() == context_id
        && candidate.height() == height
        && candidate.owner() == owner
        && candidate.source_view() == token.identity.view
        && terminal.source_class() == ProducerContinuationSourceClass::ConditionalTransport
        && token.source_class == super::FairV2IngressLeaderWireSourceClass::Control
        && leader_wire_control_phase_matches_candidate(token, candidate)
        && identity.causal_lifecycle_key() == runtime_owner.causal_lifecycle_key
        && identity.admission_ordinal() == runtime_owner.admission_ordinal
}

/// Bind the independently retained reducer projection to the exact productive
/// control-wire phase. The causal lifecycle key already commits the complete
/// wire identity (including full subject and authenticated origin); the
/// producer terminal separately retains only the reducer's block target, so
/// view and phase/kind are the strongest fields available on both sides.
fn leader_wire_control_phase_matches_candidate(
    token: &FairV2IngressLeaderWireToken,
    candidate: ServicedCandidateKey,
) -> bool {
    use super::FairV2IngressLeaderWirePhase;

    matches!(
        (token.identity.phase, candidate.kind(), candidate.phase()),
        (FairV2IngressLeaderWirePhase::Proposal, 1, 0)
            | (FairV2IngressLeaderWirePhase::PrepareVote, 2, 1)
            | (FairV2IngressLeaderWirePhase::CommitVote, 2, 2)
            | (FairV2IngressLeaderWirePhase::PrepareQc, 3, 1)
            | (FairV2IngressLeaderWirePhase::CommitQc, 3, 2)
            | (FairV2IngressLeaderWirePhase::TimeoutVote, 4, 3)
            | (FairV2IngressLeaderWirePhase::TimeoutCertificate, 5, 3)
    )
}

fn leader_wire_body_terminal_matches_runtime(
    terminal: &LeaderWireDurableBodyTerminalEvidence,
    token: &FairV2IngressLeaderWireToken,
    runtime_owner: LeaderWireRuntimeOwner,
    context_id: wire::HeightContextId,
    height: wire::Height,
    owner: [u8; 32],
) -> bool {
    let manifest_hash: Hash = terminal.manifest_hash.into();
    terminal.context_id == context_id
        && terminal.height == height
        && terminal.owner == owner
        && terminal.runtime_owner == runtime_owner
        && runtime_owner.causal_lifecycle_key == token.identity_hash()
        && terminal.round.context_id == context_id
        && terminal.round.height == height
        && terminal.round.view == token.identity.view
        && Hash::new(terminal.subject.encode()) == token.identity.subject_hash
        && token.identity.manifest_hash == Some(manifest_hash)
        && matches!(
            token.source_class,
            super::FairV2IngressLeaderWireSourceClass::Chunk
                | super::FairV2IngressLeaderWireSourceClass::CertifiedResponse
        )
}

fn leader_wire_stable_terminal_matches_runtime(
    terminal: &LeaderWireStableTerminalEvidence,
    token: &FairV2IngressLeaderWireToken,
    runtime_owner: LeaderWireRuntimeOwner,
    context_id: wire::HeightContextId,
    height: wire::Height,
    owner: [u8; 32],
) -> bool {
    if runtime_owner.causal_lifecycle_key != token.identity_hash()
        || runtime_owner.admission_ordinal != token.scheduler_ordinal
    {
        return false;
    }
    match terminal {
        LeaderWireStableTerminalEvidence::Producer(terminal) => {
            leader_wire_terminal_matches_runtime(
                *terminal,
                token,
                runtime_owner,
                context_id,
                height,
                owner,
            )
        }
        LeaderWireStableTerminalEvidence::DurableBody(terminal) => {
            leader_wire_body_terminal_matches_runtime(
                terminal,
                token,
                runtime_owner,
                context_id,
                height,
                owner,
            )
        }
    }
}

impl LeaderWireLifecycleStoreGate {
    /// Derive the finite source/phase/chunk owner universe from frozen roster
    /// and existing payload-chunk geometry. Each roster member owns an
    /// independent `max_chunk_count + 8` set of durable slots; identities that
    /// share wire components never collapse capacity across origin, phase, or
    /// chunk position.
    pub(crate) fn derived_capacity(
        roster_len: usize,
        max_chunk_count: u32,
    ) -> Result<usize, String> {
        let per_origin = 8usize
            .checked_add(
                usize::try_from(max_chunk_count)
                    .map_err(|_| "leader-wire chunk count is not addressable".to_owned())?,
            )
            .ok_or_else(|| "leader-wire per-origin capacity overflowed".to_owned())?;
        roster_len
            .checked_mul(per_origin)
            .filter(|capacity| *capacity != 0)
            .ok_or_else(|| "leader-wire lifecycle capacity overflowed".to_owned())
    }

    /// Open a context-bound leader-wire snapshot beside the safety WAL.
    ///
    /// Existing active states are normalized to selector-dormant Dormant. A
    /// producer terminal supplied from the already-opened serviced-candidate
    /// snapshot completes a crash between producer publication and generic
    /// terminal publication. The opaque replay authority removes prior-view
    /// or decided-height owners while retaining both ordinal high-watermarks
    /// against resurrection.
    ///
    /// # Errors
    ///
    /// Returns an error for zero/overflowed geometry, context/capacity drift,
    /// noncanonical or corrupt framing, unmatched Terminal records, or an
    /// atomic publication failure.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn open(
        safety_wal_path: &Path,
        context_id: wire::HeightContextId,
        height: wire::Height,
        owner: [u8; 32],
        roster: BTreeSet<PeerId>,
        capacity: usize,
        max_chunk_count: u32,
        recovery_authority: LeaderWireRecoveryAuthority,
        producer_terminals: &[ProducerContinuationTerminalToken],
        durable_bodies: &[DurableBodyReceipt],
    ) -> Result<(Arc<Self>, LeaderWireLifecycleRestore), String> {
        if roster.is_empty() || capacity == 0 || max_chunk_count == 0 {
            return Err("leader-wire lifecycle geometry must be non-zero".to_owned());
        }
        if !recovery_authority.matches_geometry(context_id, height, owner) {
            return Err("leader-wire recovery authority changed immutable geometry".to_owned());
        }
        let derived_capacity = Self::derived_capacity(roster.len(), max_chunk_count)?;
        if capacity != derived_capacity {
            return Err(format!(
                "leader-wire lifecycle capacity {capacity} does not match derived geometry {derived_capacity}"
            ));
        }
        let mut file_name = safety_wal_path
            .file_name()
            .ok_or_else(|| "safety WAL path has no file name".to_owned())?
            .to_os_string();
        file_name.push(".leader-wire-lifecycles");
        let path = safety_wal_path.with_file_name(file_name);
        let record_bytes = u64::try_from(capacity)
            .map_err(|_| "leader-wire lifecycle capacity is not representable".to_owned())?
            .checked_mul(LEADER_WIRE_RECORD_HEADROOM_BYTES)
            .ok_or_else(|| "leader-wire lifecycle frame bound overflowed".to_owned())?;
        let max_frame_bytes = FIXED_FRAME_HEADROOM_BYTES
            .checked_add(record_bytes)
            .ok_or_else(|| "leader-wire lifecycle frame bound overflowed".to_owned())?;
        let gate = Arc::new(Self {
            path,
            context_id,
            height,
            owner,
            roster,
            capacity,
            max_chunk_count,
            max_frame_bytes,
            state: Mutex::new(LeaderWireLifecycleState {
                recovery_authority,
                last_admission_ordinal: 0,
                scheduler_ordinal_high_watermark: 0,
                records: BTreeMap::new(),
                replay_dormant: BTreeSet::new(),
            }),
        });
        let changed =
            gate.load_and_reconcile(recovery_authority, producer_terminals, durable_bodies)?;
        if changed {
            let state = gate
                .state
                .lock()
                .map_err(|_| "leader-wire lifecycle store lock was poisoned".to_owned())?;
            gate.persist_locked(&state)?;
        }
        let restore = gate.restore()?;
        Ok((gate, restore))
    }

    /// Whether two handles name the same synchronous persistence gate.
    pub(crate) fn ptr_eq(left: &Arc<Self>, right: &Arc<Self>) -> bool {
        Arc::ptr_eq(left, right)
    }

    /// Whether a proposed fair-ingress binding has identical frozen geometry.
    pub(crate) fn matches_geometry(
        &self,
        context_id: wire::HeightContextId,
        height: wire::Height,
        roster: &BTreeSet<PeerId>,
        capacity: usize,
        max_chunk_count: u32,
    ) -> bool {
        self.context_id == context_id
            && self.height == height
            && &self.roster == roster
            && self.capacity == capacity
            && self.max_chunk_count == max_chunk_count
            && Self::derived_capacity(roster.len(), max_chunk_count) == Ok(capacity)
    }

    /// Return the current canonical restart projection.
    pub(crate) fn restore(&self) -> Result<LeaderWireLifecycleRestore, String> {
        let state = self
            .state
            .lock()
            .map_err(|_| "leader-wire lifecycle store lock was poisoned".to_owned())?;
        Ok(Self::restore_from_state(&state))
    }

    /// Logical scheduler ordinals of every durable Ingress owner.
    ///
    /// Runtime and Terminal records have crossed this selector cut. Restored
    /// active records have no surviving physical carrier, so they remain
    /// replay-dormant and are excluded until exact retransmission passes
    /// capacity checks and `admit_ingress` reactivates them. These immutable
    /// logical identities validate the in-memory owner set; they do not order
    /// physical carriers because a replay-dormant owner retains its old
    /// scheduler ordinal while acquiring a fresh ingress occurrence.
    pub(crate) fn ingress_scheduler_ordinals(&self) -> Result<BTreeSet<u128>, String> {
        let state = self
            .state
            .lock()
            .map_err(|_| "leader-wire lifecycle store lock was poisoned".to_owned())?;
        Ok(state
            .records
            .values()
            .filter(|record| record.status == LeaderWireLifecycleStatus::Ingress)
            .map(|record| record.token.scheduler_ordinal)
            .collect())
    }

    /// Minimum logical scheduler ordinal among durable Ingress owners.
    ///
    /// This projection is useful for diagnostics and lifecycle tests only.
    /// Fair ingress must select by the owners' live physical carrier ordinals;
    /// a restored exact retry can have the oldest logical identity and the
    /// newest physical occurrence.
    #[cfg(test)]
    pub(crate) fn earliest_ingress_scheduler_ordinal(&self) -> Result<Option<u128>, String> {
        Ok(self.ingress_scheduler_ordinals()?.into_iter().next())
    }

    /// Look up an exact semantic retry before allocating another scheduler
    /// ordinal. This is the coalescing preflight used by fair ingress; it never
    /// mutates durable state.
    pub(crate) fn lookup_exact(
        &self,
        identity: &FairV2IngressLeaderWireIdentity,
        slot: &FairV2IngressLeaderWireSlot,
    ) -> Result<Option<LeaderWireLifecycleAdmissionReceipt>, String> {
        let state = self
            .state
            .lock()
            .map_err(|_| "leader-wire lifecycle store lock was poisoned".to_owned())?;
        Ok(state.records.get(slot).and_then(|record| {
            (record.token.identity == *identity).then(|| LeaderWireLifecycleAdmissionReceipt {
                token: record.token.clone(),
                status: record.status,
                inserted: false,
            })
        }))
    }

    /// Return whether the latest live safety-WAL cut rejects this identity.
    pub(crate) fn identity_is_obsolete(
        &self,
        identity: &FairV2IngressLeaderWireIdentity,
    ) -> Result<bool, String> {
        let state = self
            .state
            .lock()
            .map_err(|_| "leader-wire lifecycle store lock was poisoned".to_owned())?;
        Ok(state.recovery_authority.obsoletes_identity(identity))
    }

    /// Apply a live, WAL-authorized recovery cut and retire its exact dormant set.
    ///
    /// Fair ingress supplies the complete mirrored dormant set while holding
    /// its own lock. Requiring exact equality makes durable publication and
    /// the volatile mirror one transaction: neither side can silently drop a
    /// carrier-owning Ingress/Runtime record or disagree about which dormant
    /// slots disappeared. Ordinal high-watermarks deliberately survive.
    pub(crate) fn advance_recovery_cut(
        &self,
        next: LeaderWireRecoveryAuthority,
        expected_dormant_slots: &BTreeSet<FairV2IngressLeaderWireSlot>,
    ) -> Result<(), String> {
        if !next.matches_geometry(self.context_id, self.height, self.owner) {
            return Err("leader-wire recovery cut changed immutable geometry".to_owned());
        }
        let mut state = self
            .state
            .lock()
            .map_err(|_| "leader-wire lifecycle store lock was poisoned".to_owned())?;
        if !next.monotonically_extends(state.recovery_authority) {
            return Err("leader-wire recovery cut is not monotone".to_owned());
        }
        let retiring = state
            .records
            .iter()
            .filter_map(|(slot, record)| {
                (record.status == LeaderWireLifecycleStatus::Dormant
                    && next.obsoletes(&record.token))
                .then(|| slot.clone())
            })
            .collect::<BTreeSet<_>>();
        if retiring != *expected_dormant_slots || !retiring.is_subset(&state.replay_dormant) {
            return Err(
                "leader-wire recovery cut disagreed with dormant ingress ownership".to_owned(),
            );
        }

        let previous = state.clone();
        state.recovery_authority = next;
        for slot in &retiring {
            let removed = state
                .records
                .remove(slot)
                .expect("preflighted dormant leader-wire slot remains indexed");
            debug_assert_eq!(removed.status, LeaderWireLifecycleStatus::Dormant);
            let was_dormant = state.replay_dormant.remove(slot);
            debug_assert!(was_dormant);
        }
        if !retiring.is_empty()
            && let Err(error) = self.persist_locked(&state)
        {
            *state = previous;
            return Err(error);
        }
        Ok(())
    }

    /// Atomically persist an Ingress owner before fair ingress returns Accepted.
    ///
    /// An exact Dormant retry receives the incumbent token, preserving both
    /// immutable ordinals, and transitions directly to Ingress. A different
    /// identity cannot replace an active slot; it can replace a Terminal
    /// high-water only with a strictly newer view and ordinal.
    pub(crate) fn admit_ingress(
        &self,
        token: FairV2IngressLeaderWireToken,
    ) -> Result<LeaderWireLifecycleAdmissionReceipt, String> {
        if !token.validate_exact(
            self.context_id,
            self.height,
            &self.roster,
            self.max_chunk_count,
        ) {
            return Err("leader-wire admission crossed immutable geometry".to_owned());
        }
        let mut state = self
            .state
            .lock()
            .map_err(|_| "leader-wire lifecycle store lock was poisoned".to_owned())?;
        if state.recovery_authority.obsoletes(&token) {
            return Err(
                "leader-wire admission is obsolete under the durable recovery cut".to_owned(),
            );
        }
        let incumbent = state.records.get(&token.slot).cloned();
        if let Some(incumbent) = incumbent.as_ref() {
            if incumbent.token.identity == token.identity {
                if incumbent.status == LeaderWireLifecycleStatus::Dormant {
                    let trace = leader_wire_admission_trace_projection(
                        &state,
                        self.capacity,
                        &token,
                        Some(incumbent),
                        LEADER_WIRE_ADMISSION_REACTIVATE,
                    )?;
                    let checked_transition = check_production_leader_wire_admission_transition(
                        trace,
                    )
                    .ok_or_else(|| {
                        "leader-wire Dormant reactivation failed its lifecycle refinement"
                            .to_owned()
                    })?;
                    let _authorized_transition = checked_transition.into_projection();
                    let previous = state.clone();
                    state
                        .records
                        .get_mut(&token.slot)
                        .expect("validated Dormant leader-wire slot remains indexed")
                        .status = LeaderWireLifecycleStatus::Ingress;
                    state.replay_dormant.remove(&token.slot);
                    if let Err(error) = self.persist_locked(&state) {
                        *state = previous;
                        return Err(error);
                    }
                    return Ok(LeaderWireLifecycleAdmissionReceipt {
                        token: incumbent.token.clone(),
                        status: LeaderWireLifecycleStatus::Ingress,
                        inserted: false,
                    });
                }
                let trace = leader_wire_admission_trace_projection(
                    &state,
                    self.capacity,
                    &token,
                    Some(incumbent),
                    LEADER_WIRE_ADMISSION_COALESCE,
                )?;
                let checked_transition = check_production_leader_wire_admission_transition(trace)
                    .ok_or_else(|| {
                    "leader-wire exact retry failed its coalescing lifecycle refinement".to_owned()
                })?;
                let _authorized_transition = checked_transition.into_projection();
                return Ok(LeaderWireLifecycleAdmissionReceipt {
                    token: incumbent.token.clone(),
                    status: incumbent.status,
                    inserted: false,
                });
            }
            if incumbent.status.is_active() {
                return Err("leader-wire slot already has an active predecessor".to_owned());
            }
            if token.identity.view <= incumbent.token.identity.view
                || token.admission_ordinal <= incumbent.token.admission_ordinal
            {
                return Err(
                    "leader-wire terminal replacement did not strictly advance view and ordinal"
                        .to_owned(),
                );
            }
        } else if state.records.len() >= self.capacity {
            return Err("leader-wire lifecycle capacity is exhausted".to_owned());
        }
        if token.admission_ordinal <= state.last_admission_ordinal {
            return Err("new leader-wire admission reused an old admission ordinal".to_owned());
        }
        if token.scheduler_ordinal <= state.scheduler_ordinal_high_watermark {
            return Err("new leader-wire admission reused an old scheduler ordinal".to_owned());
        }
        let operation = if incumbent.is_some() {
            LEADER_WIRE_ADMISSION_REPLACE_TERMINAL
        } else {
            LEADER_WIRE_ADMISSION_INSERT
        };
        let trace = leader_wire_admission_trace_projection(
            &state,
            self.capacity,
            &token,
            incumbent.as_ref(),
            operation,
        )?;
        let checked_transition = check_production_leader_wire_admission_transition(trace)
            .ok_or_else(|| "leader-wire admission failed its lifecycle refinement".to_owned())?;
        let _authorized_transition = checked_transition.into_projection();
        let previous = state.clone();
        state.last_admission_ordinal = token.admission_ordinal;
        state.scheduler_ordinal_high_watermark = token.scheduler_ordinal;
        state.records.insert(
            token.slot.clone(),
            PersistedLeaderWireLifecycleRecord {
                token: token.clone(),
                status: LeaderWireLifecycleStatus::Ingress,
                runtime_owner: None,
                terminal_evidence: None,
            },
        );
        if let Err(error) = self.persist_locked(&state) {
            *state = previous;
            return Err(error);
        }
        Ok(LeaderWireLifecycleAdmissionReceipt {
            token,
            status: LeaderWireLifecycleStatus::Ingress,
            inserted: true,
        })
    }

    #[cfg(test)]
    pub(crate) fn reserve(
        &self,
        token: FairV2IngressLeaderWireToken,
    ) -> Result<LeaderWireLifecycleAdmissionReceipt, String> {
        self.admit_ingress(token)
    }

    #[cfg(test)]
    pub(crate) fn mark_ingress(&self, token: &FairV2IngressLeaderWireToken) -> Result<(), String> {
        let receipt = self
            .lookup_exact(&token.identity, &token.slot)?
            .ok_or_else(|| "leader-wire test admission has no exact slot".to_owned())?;
        if receipt.token() != token || receipt.status() != LeaderWireLifecycleStatus::Ingress {
            return Err("leader-wire test admission is not an exact Ingress owner".to_owned());
        }
        Ok(())
    }

    /// Durably transfer an ingress token to one exact serialized-runtime owner.
    pub(crate) fn mark_runtime(
        &self,
        token: &FairV2IngressLeaderWireToken,
        owner: LeaderWireRuntimeOwner,
    ) -> Result<LeaderWireLifecycleRuntimeReceipt, String> {
        if owner.admission_ordinal != token.scheduler_ordinal
            || owner.causal_lifecycle_key != token.identity_hash()
        {
            return Err(
                "leader-wire transfer changed its shared scheduler/token identity".to_owned(),
            );
        }
        self.transition(token, |record| match record.status {
            LeaderWireLifecycleStatus::Ingress => {
                if record
                    .runtime_owner
                    .is_some_and(|incumbent| incumbent != owner)
                {
                    return Err("leader-wire replay changed its restored runtime owner".to_owned());
                }
                record.runtime_owner = Some(owner);
                record.status = LeaderWireLifecycleStatus::Runtime;
                Ok(true)
            }
            LeaderWireLifecycleStatus::Runtime if record.runtime_owner == Some(owner) => Ok(false),
            LeaderWireLifecycleStatus::Dormant
            | LeaderWireLifecycleStatus::Runtime
            | LeaderWireLifecycleStatus::VolatileTerminal
            | LeaderWireLifecycleStatus::Terminal => {
                Err("leader-wire runtime transition had no exact ingress owner".to_owned())
            }
        })?;
        Ok(LeaderWireLifecycleRuntimeReceipt {
            token: token.clone(),
            owner,
        })
    }

    /// Publish a same-process tombstone without claiming restart-stable proof.
    ///
    /// This state coalesces exact retransmission until process exit. Restore
    /// always rewrites it to Dormant with the same immutable ordinals.
    pub(crate) fn mark_volatile_terminal(
        &self,
        runtime: &LeaderWireLifecycleRuntimeReceipt,
    ) -> Result<(), String> {
        self.transition(&runtime.token, |record| match record.status {
            LeaderWireLifecycleStatus::Runtime
                if record.runtime_owner == Some(runtime.owner)
                    && record.terminal_evidence.is_none() =>
            {
                record.status = LeaderWireLifecycleStatus::VolatileTerminal;
                Ok(true)
            }
            LeaderWireLifecycleStatus::VolatileTerminal
                if record.runtime_owner == Some(runtime.owner)
                    && record.terminal_evidence.is_none() =>
            {
                Ok(false)
            }
            LeaderWireLifecycleStatus::Dormant
            | LeaderWireLifecycleStatus::Ingress
            | LeaderWireLifecycleStatus::Runtime
            | LeaderWireLifecycleStatus::VolatileTerminal
            | LeaderWireLifecycleStatus::Terminal => {
                Err("leader-wire volatile terminal changed exact ownership".to_owned())
            }
        })
    }

    /// Publish the generic Terminal after typed stable evidence exists.
    pub(crate) fn mark_terminal(
        &self,
        runtime: &LeaderWireLifecycleRuntimeReceipt,
        terminal_evidence: impl Into<LeaderWireStableTerminalEvidence>,
    ) -> Result<(), String> {
        let terminal_evidence = terminal_evidence.into();
        if !leader_wire_stable_terminal_matches_runtime(
            &terminal_evidence,
            &runtime.token,
            runtime.owner,
            self.context_id,
            self.height,
            self.owner,
        ) {
            return Err("leader-wire terminal did not match its producer runtime owner".to_owned());
        }
        self.transition(&runtime.token, |record| match record.status {
            LeaderWireLifecycleStatus::Runtime | LeaderWireLifecycleStatus::VolatileTerminal
                if record.runtime_owner == Some(runtime.owner)
                    && record.terminal_evidence.is_none() =>
            {
                record.terminal_evidence = Some(terminal_evidence.clone());
                record.status = LeaderWireLifecycleStatus::Terminal;
                Ok(true)
            }
            LeaderWireLifecycleStatus::Terminal
                if record.runtime_owner == Some(runtime.owner)
                    && record.terminal_evidence.as_ref() == Some(&terminal_evidence) =>
            {
                Ok(false)
            }
            LeaderWireLifecycleStatus::Dormant
            | LeaderWireLifecycleStatus::Ingress
            | LeaderWireLifecycleStatus::Runtime
            | LeaderWireLifecycleStatus::VolatileTerminal
            | LeaderWireLifecycleStatus::Terminal => {
                Err("leader-wire terminal transition changed exact ownership".to_owned())
            }
        })
    }

    /// Publish a producer-backed stable terminal after producer-first ordering.
    pub(crate) fn mark_producer_terminal(
        &self,
        runtime: &LeaderWireLifecycleRuntimeReceipt,
        producer_terminal: ProducerContinuationTerminalToken,
    ) -> Result<(), String> {
        self.mark_terminal(
            runtime,
            LeaderWireStableTerminalEvidence::Producer(producer_terminal),
        )
    }

    /// Publish a body-backed stable terminal from a non-forgeable store receipt.
    pub(crate) fn mark_durable_body_terminal(
        &self,
        runtime: &LeaderWireLifecycleRuntimeReceipt,
        durable_body: &DurableBodyReceipt,
    ) -> Result<(), String> {
        self.mark_terminal(
            runtime,
            LeaderWireStableTerminalEvidence::DurableBody(
                LeaderWireDurableBodyTerminalEvidence::from_receipt(
                    durable_body,
                    self.owner,
                    runtime.owner,
                ),
            ),
        )
    }

    fn transition(
        &self,
        token: &FairV2IngressLeaderWireToken,
        update: impl FnOnce(&mut PersistedLeaderWireLifecycleRecord) -> Result<bool, String>,
    ) -> Result<(), String> {
        if !token.validate_exact(
            self.context_id,
            self.height,
            &self.roster,
            self.max_chunk_count,
        ) {
            return Err("leader-wire transition crossed immutable geometry".to_owned());
        }
        let mut state = self
            .state
            .lock()
            .map_err(|_| "leader-wire lifecycle store lock was poisoned".to_owned())?;
        let previous = state.clone();
        let record = state
            .records
            .get_mut(&token.slot)
            .ok_or_else(|| "leader-wire transition had no reserved slot".to_owned())?;
        if record.token != *token {
            return Err("leader-wire transition changed its immutable token".to_owned());
        }
        if !update(record)? {
            return Ok(());
        }
        if let Err(error) = self.persist_locked(&state) {
            *state = previous;
            return Err(error);
        }
        Ok(())
    }

    fn restore_from_state(state: &LeaderWireLifecycleState) -> LeaderWireLifecycleRestore {
        LeaderWireLifecycleRestore {
            last_admission_ordinal: state.last_admission_ordinal,
            scheduler_ordinal_high_watermark: state.scheduler_ordinal_high_watermark,
            records: state
                .records
                .values()
                .map(|record| LeaderWireLifecycleRestoredRecord {
                    token: record.token.clone(),
                    status: record.status,
                    runtime_owner: record.runtime_owner,
                    terminal_evidence: record.terminal_evidence.clone(),
                })
                .collect(),
        }
    }

    fn load_and_reconcile(
        &self,
        recovery_authority: LeaderWireRecoveryAuthority,
        producer_terminals: &[ProducerContinuationTerminalToken],
        durable_bodies: &[DurableBodyReceipt],
    ) -> Result<bool, String> {
        let Some((mut file, opened_before)) =
            open_bound_snapshot(&self.path, self.max_frame_bytes)?
        else {
            return Ok(false);
        };
        let read_limit = self
            .max_frame_bytes
            .checked_add(1)
            .ok_or_else(|| "leader-wire lifecycle read bound overflowed".to_owned())?;
        let mut bytes = Vec::new();
        Read::by_ref(&mut file)
            .take(read_limit)
            .read_to_end(&mut bytes)
            .map_err(|error| {
                format!(
                    "failed to read leader-wire lifecycle snapshot {}: {error}",
                    self.path.display()
                )
            })?;
        let opened_after = file.metadata().map_err(|error| {
            format!(
                "failed to reinspect leader-wire lifecycle snapshot {}: {error}",
                self.path.display()
            )
        })?;
        let path_after = fs::symlink_metadata(&self.path).map_err(|error| {
            format!(
                "failed to reinspect leader-wire lifecycle path {}: {error}",
                self.path.display()
            )
        })?;
        if !snapshot_metadata_is_safe(&opened_after, self.max_frame_bytes)
            || !snapshot_metadata_is_safe(&path_after, self.max_frame_bytes)
            || !snapshot_metadata_unchanged(&opened_before, &opened_after)
            || !snapshot_metadata_unchanged(&opened_before, &path_after)
            || opened_after.len() != u64::try_from(bytes.len()).unwrap_or(u64::MAX)
        {
            return Err("leader-wire lifecycle snapshot changed while reading".to_owned());
        }
        let decoded = decode_leader_wire_frame(&bytes, self.max_frame_bytes)?;
        if decoded.context_id != self.context_id
            || decoded.height != self.height
            || decoded.owner != self.owner
            || decoded.capacity != u64::try_from(self.capacity).unwrap_or(u64::MAX)
            || decoded.max_chunk_count != self.max_chunk_count
            || decoded.records.len() > self.capacity
        {
            return Err("leader-wire lifecycle snapshot changed immutable geometry".to_owned());
        }
        let terminal_set = producer_terminals.iter().copied().collect::<BTreeSet<_>>();
        let mut records = BTreeMap::new();
        let mut ordinals = BTreeSet::new();
        let mut scheduler_ordinals = BTreeSet::new();
        let mut replay_dormant = BTreeSet::new();
        let mut previous_slot = None;
        let mut changed = false;
        for mut record in decoded.records {
            if previous_slot
                .as_ref()
                .is_some_and(|previous| previous >= &record.token.slot)
            {
                return Err("leader-wire lifecycle records are not strictly ordered".to_owned());
            }
            previous_slot = Some(record.token.slot.clone());
            if !record.token.validate_exact(
                self.context_id,
                self.height,
                &self.roster,
                self.max_chunk_count,
            ) || record.token.admission_ordinal > decoded.last_admission_ordinal
                || record.token.scheduler_ordinal > decoded.scheduler_ordinal_high_watermark
                || !ordinals.insert(record.token.admission_ordinal)
                || !scheduler_ordinals.insert(record.token.scheduler_ordinal)
                || records.contains_key(&record.token.slot)
            {
                return Err("leader-wire lifecycle snapshot has a noncanonical record".to_owned());
            }
            if record.runtime_owner.is_some_and(|runtime_owner| {
                runtime_owner.admission_ordinal != record.token.scheduler_ordinal
                    || runtime_owner.causal_lifecycle_key != record.token.identity_hash()
            }) {
                return Err(
                    "leader-wire snapshot changed its shared scheduler/token identity".to_owned(),
                );
            }
            let status_shape_valid = match (
                record.status,
                record.runtime_owner,
                record.terminal_evidence.as_ref(),
            ) {
                (LeaderWireLifecycleStatus::Dormant, _, None)
                | (LeaderWireLifecycleStatus::Ingress, _, None) => true,
                (LeaderWireLifecycleStatus::Runtime, Some(_), None)
                | (LeaderWireLifecycleStatus::VolatileTerminal, Some(_), None) => true,
                (LeaderWireLifecycleStatus::Terminal, Some(runtime_owner), Some(evidence)) => {
                    leader_wire_stable_terminal_matches_runtime(
                        evidence,
                        &record.token,
                        runtime_owner,
                        self.context_id,
                        self.height,
                        self.owner,
                    )
                }
                _ => false,
            };
            if !status_shape_valid {
                return Err(
                    "leader-wire lifecycle snapshot has an inconsistent status projection"
                        .to_owned(),
                );
            }
            // Safety-WAL replay is an independent monotone authority. Once it
            // has durably advanced beyond this view, or recorded Decision for
            // the height, no generic ingress owner from the obsolete episode
            // can be resurrected. Retire the record while preserving both file
            // high-watermarks so subsequent admission cannot reuse an ordinal.
            if recovery_authority.obsoletes(&record.token) {
                changed = true;
                continue;
            }
            let matching_producer_terminal = record.runtime_owner.and_then(|runtime_owner| {
                terminal_set.iter().copied().find(|terminal| {
                    leader_wire_terminal_matches_runtime(
                        *terminal,
                        &record.token,
                        runtime_owner,
                        self.context_id,
                        self.height,
                        self.owner,
                    )
                })
            });
            let matching_body_terminal = record.runtime_owner.and_then(|runtime_owner| {
                durable_bodies.iter().find_map(|receipt| {
                    let terminal = LeaderWireDurableBodyTerminalEvidence::from_receipt(
                        receipt,
                        self.owner,
                        runtime_owner,
                    );
                    leader_wire_body_terminal_matches_runtime(
                        &terminal,
                        &record.token,
                        runtime_owner,
                        self.context_id,
                        self.height,
                        self.owner,
                    )
                    .then_some(terminal)
                })
            });
            match record.status {
                LeaderWireLifecycleStatus::Terminal => {
                    let Some(runtime_owner) = record.runtime_owner else {
                        return Err("leader-wire Terminal has no runtime owner".to_owned());
                    };
                    let independently_valid = match record.terminal_evidence.as_ref() {
                        Some(LeaderWireStableTerminalEvidence::Producer(terminal)) => {
                            matching_producer_terminal == Some(*terminal)
                                && leader_wire_stable_terminal_matches_runtime(
                                    record
                                        .terminal_evidence
                                        .as_ref()
                                        .expect("matched terminal evidence exists"),
                                    &record.token,
                                    runtime_owner,
                                    self.context_id,
                                    self.height,
                                    self.owner,
                                )
                        }
                        Some(LeaderWireStableTerminalEvidence::DurableBody(terminal)) => {
                            matching_body_terminal.as_ref() == Some(terminal)
                                && durable_bodies
                                    .iter()
                                    .any(|receipt| terminal.matches_receipt(receipt))
                                && leader_wire_body_terminal_matches_runtime(
                                    terminal,
                                    &record.token,
                                    runtime_owner,
                                    self.context_id,
                                    self.height,
                                    self.owner,
                                )
                        }
                        None => false,
                    };
                    if !independently_valid {
                        return Err(
                            "leader-wire Terminal has no independently verified stable evidence"
                                .to_owned(),
                        );
                    }
                }
                LeaderWireLifecycleStatus::VolatileTerminal => {
                    if record.runtime_owner.is_none() || record.terminal_evidence.is_some() {
                        return Err(
                            "leader-wire volatile terminal changed its runtime ownership"
                                .to_owned(),
                        );
                    }
                    record.status = LeaderWireLifecycleStatus::Dormant;
                    changed = true;
                }
                LeaderWireLifecycleStatus::Dormant
                | LeaderWireLifecycleStatus::Ingress
                | LeaderWireLifecycleStatus::Runtime => {
                    if record.status == LeaderWireLifecycleStatus::Runtime
                        && record.runtime_owner.is_none()
                    {
                        return Err("leader-wire Runtime record has no downstream owner".to_owned());
                    }
                    if record.terminal_evidence.is_some() {
                        return Err(
                            "active leader-wire record carried terminal evidence".to_owned()
                        );
                    }
                    if let Some(terminal) = matching_producer_terminal {
                        record.status = LeaderWireLifecycleStatus::Terminal;
                        record.terminal_evidence =
                            Some(LeaderWireStableTerminalEvidence::Producer(terminal));
                        changed = true;
                    } else if let Some(terminal) = matching_body_terminal {
                        record.status = LeaderWireLifecycleStatus::Terminal;
                        record.terminal_evidence =
                            Some(LeaderWireStableTerminalEvidence::DurableBody(terminal));
                        changed = true;
                    } else {
                        changed |= record.status != LeaderWireLifecycleStatus::Dormant;
                        record.status = LeaderWireLifecycleStatus::Dormant;
                    }
                }
            }
            if record.status == LeaderWireLifecycleStatus::Dormant {
                replay_dormant.insert(record.token.slot.clone());
            }
            records.insert(record.token.slot.clone(), record);
        }
        let mut state = self
            .state
            .lock()
            .map_err(|_| "leader-wire lifecycle store lock was poisoned".to_owned())?;
        state.recovery_authority = recovery_authority;
        state.last_admission_ordinal = decoded.last_admission_ordinal;
        state.scheduler_ordinal_high_watermark = decoded.scheduler_ordinal_high_watermark;
        state.records = records;
        state.replay_dormant = replay_dormant;
        Ok(changed)
    }

    fn persist_locked(&self, state: &LeaderWireLifecycleState) -> Result<(), String> {
        let snapshot = PersistedLeaderWireLifecycles {
            format_version: LEADER_WIRE_FORMAT_VERSION,
            context_id: self.context_id,
            height: self.height,
            owner: self.owner,
            capacity: u64::try_from(self.capacity)
                .map_err(|_| "leader-wire lifecycle capacity is not representable".to_owned())?,
            max_chunk_count: self.max_chunk_count,
            last_admission_ordinal: state.last_admission_ordinal,
            scheduler_ordinal_high_watermark: state.scheduler_ordinal_high_watermark,
            records: state.records.values().cloned().collect(),
        };
        let frame = encode_leader_wire_frame(&snapshot, self.max_frame_bytes)?;
        publish_atomic_frame(&self.path, &frame, "leader-wire lifecycle")
    }
}

impl ServicedCandidateStore {
    /// Open the height-bound snapshot adjacent to `safety_wal_path`.
    ///
    /// # Errors
    ///
    /// Returns an error when the derived geometry overflows or an existing
    /// snapshot is missing its canonical framing, checksum, ordering, or exact
    /// height-context binding.
    pub(crate) fn open(
        safety_wal_path: &Path,
        context_id: wire::HeightContextId,
        height: wire::Height,
        owner: [u8; 32],
        lifecycle_capacity: usize,
    ) -> Result<(Self, RestoredServicedCandidates), String> {
        let record_capacity = lifecycle_capacity
            .checked_mul(SERVICED_CANDIDATE_STAGES_PER_LIFECYCLE)
            .ok_or_else(|| "serviced-candidate lifecycle-stage capacity overflowed".to_owned())?;
        Self::open_with_capacities(
            safety_wal_path,
            context_id,
            height,
            owner,
            record_capacity,
            record_capacity,
        )
    }

    fn open_with_capacities(
        safety_wal_path: &Path,
        context_id: wire::HeightContextId,
        height: wire::Height,
        owner: [u8; 32],
        serviced_capacity: usize,
        producer_continuation_capacity: usize,
    ) -> Result<(Self, RestoredServicedCandidates), String> {
        if serviced_capacity == 0 || producer_continuation_capacity == 0 {
            return Err("serviced-candidate capacity must be non-zero".to_owned());
        }
        if producer_continuation_capacity % SERVICED_CANDIDATE_STAGES_PER_LIFECYCLE != 0 {
            return Err(
                "producer-continuation capacity must be an exact lifecycle-stage geometry"
                    .to_owned(),
            );
        }
        let producer_continuation_lifecycle_capacity =
            u64::try_from(producer_continuation_capacity / SERVICED_CANDIDATE_STAGES_PER_LIFECYCLE)
                .map_err(|_| {
                    "producer-continuation lifecycle capacity is not representable".to_owned()
                })?;
        if producer_continuation_lifecycle_capacity == 0 {
            return Err("producer-continuation lifecycle capacity must be non-zero".to_owned());
        }
        let mut file_name = safety_wal_path
            .file_name()
            .ok_or_else(|| "safety WAL path has no file name".to_owned())?
            .to_os_string();
        file_name.push(".serviced-candidates");
        let path = safety_wal_path.with_file_name(file_name);
        let serviced_frame_bytes = u64::try_from(serviced_capacity)
            .map_err(|_| "serviced-candidate capacity is not representable".to_owned())?
            .checked_mul(RECORD_FRAME_HEADROOM_BYTES)
            .ok_or_else(|| "serviced-candidate frame bound overflowed".to_owned())?;
        let producer_frame_bytes = u64::try_from(producer_continuation_capacity)
            .map_err(|_| "producer-continuation capacity is not representable".to_owned())?
            .checked_mul(PRODUCER_CONTINUATION_FRAME_HEADROOM_BYTES)
            .ok_or_else(|| "producer-continuation frame bound overflowed".to_owned())?;
        let max_frame_bytes = FIXED_FRAME_HEADROOM_BYTES
            .checked_add(serviced_frame_bytes)
            .and_then(|bytes| bytes.checked_add(producer_frame_bytes))
            .ok_or_else(|| "serviced-candidate frame bound overflowed".to_owned())?;
        let store = Self {
            path,
            context_id,
            height,
            owner,
            serviced_capacity,
            producer_continuation_capacity,
            producer_continuation_lifecycle_capacity,
            max_frame_bytes,
        };
        let restored = store.load()?;
        Ok((store, restored))
    }

    fn load(&self) -> Result<RestoredServicedCandidates, String> {
        let Some((mut file, opened_before)) =
            open_bound_snapshot(&self.path, self.max_frame_bytes)?
        else {
            return Ok(RestoredServicedCandidates {
                records: BTreeMap::new(),
                producer_continuations: BTreeMap::new(),
                decision_reclaimed: false,
            });
        };
        let read_limit = self
            .max_frame_bytes
            .checked_add(1)
            .ok_or_else(|| "serviced-candidate read bound overflowed".to_owned())?;
        let mut bytes =
            Vec::with_capacity(usize::try_from(opened_before.len()).unwrap_or_default());
        Read::by_ref(&mut file)
            .take(read_limit)
            .read_to_end(&mut bytes)
            .map_err(|error| {
                format!(
                    "failed to read serviced-candidate snapshot {}: {error}",
                    self.path.display()
                )
            })?;
        if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > self.max_frame_bytes {
            return Err("serviced-candidate snapshot grew beyond its read bound".to_owned());
        }
        let opened_after = file.metadata().map_err(|error| {
            format!(
                "failed to reinspect opened serviced-candidate snapshot {}: {error}",
                self.path.display()
            )
        })?;
        let path_after = fs::symlink_metadata(&self.path).map_err(|error| {
            format!(
                "failed to reinspect serviced-candidate snapshot {}: {error}",
                self.path.display()
            )
        })?;
        if !snapshot_metadata_is_safe(&opened_after, self.max_frame_bytes)
            || !snapshot_metadata_is_safe(&path_after, self.max_frame_bytes)
            || !snapshot_metadata_unchanged(&opened_before, &opened_after)
            || !snapshot_metadata_unchanged(&opened_before, &path_after)
            || opened_after.len() != u64::try_from(bytes.len()).unwrap_or(u64::MAX)
        {
            return Err(format!(
                "serviced-candidate snapshot {} changed while reading",
                self.path.display()
            ));
        }
        let state = decode_frame(&bytes, self.max_frame_bytes)?;
        let expected_serviced_capacity = u64::try_from(self.serviced_capacity)
            .map_err(|_| "serviced-candidate capacity is not representable".to_owned())?;
        let expected_producer_capacity = u64::try_from(self.producer_continuation_capacity)
            .map_err(|_| "producer-continuation capacity is not representable".to_owned())?;
        if state.context_id != self.context_id
            || state.height != self.height
            || state.owner != self.owner
            || state.serviced_capacity != expected_serviced_capacity
            || state.producer_continuation_capacity != expected_producer_capacity
            || state.records.len() > self.serviced_capacity
            || state.producer_continuations.len() > self.producer_continuation_capacity
            || state.records.iter().any(|record| {
                !record
                    .key
                    .belongs_to(self.context_id, self.height, self.owner)
            })
            || state
                .records
                .windows(2)
                .any(|pair| pair[0].key >= pair[1].key)
            || !producer_continuations_are_valid(
                &state.producer_continuations,
                self.context_id,
                self.height,
                self.owner,
                self.producer_continuation_lifecycle_capacity,
            )
            || !state.decision_reclaimed
                && state.producer_continuations.iter().any(|persisted| {
                    persisted.record.status == ProducerContinuationStatus::Terminal
                        && !state
                            .records
                            .iter()
                            .any(|serviced| serviced.key == persisted.record.identity.candidate)
                })
            || state.decision_reclaimed
                && (!state.records.is_empty() || !state.producer_continuations.is_empty())
        {
            return Err(
                "serviced-candidate snapshot crossed its immutable context geometry".to_owned(),
            );
        }
        Ok(RestoredServicedCandidates {
            records: state
                .records
                .into_iter()
                .map(|record| (record.key, record.service_view))
                .collect(),
            producer_continuations: state
                .producer_continuations
                .into_iter()
                .map(|persisted| {
                    let record = if persisted.record.status == ProducerContinuationStatus::Terminal
                    {
                        persisted.record
                    } else {
                        ProducerContinuationRecord::new(
                            persisted.record.identity,
                            ProducerContinuationStatus::Reserved,
                            Vec::new(),
                        )
                        .expect("validated active producer record resets to Reserved")
                    };
                    (persisted.address, record)
                })
                .collect(),
            decision_reclaimed: state.decision_reclaimed,
        })
    }

    /// Reserve one bounded lifecycle-stage address without evicting live work.
    ///
    /// An occupied address coalesces only an exact immutable record. Reuse by
    /// another lifecycle requires a terminal incumbent and strict advancement
    /// of both source view and admission ordinal, preventing stale ABA writes.
    /// The caller must publish the resulting table before retiring its source.
    pub(crate) fn reserve_producer_continuation(
        &self,
        producer_continuations: &mut BTreeMap<
            ProducerContinuationAddress,
            ProducerContinuationRecord,
        >,
        record: ProducerContinuationRecord,
    ) -> Result<ProducerContinuationReservation, String> {
        let address = record.identity.address();
        let singleton = [PersistedProducerContinuation {
            address,
            record: record.clone(),
        }];
        if !producer_continuations_are_valid(
            &singleton,
            self.context_id,
            self.height,
            self.owner,
            self.producer_continuation_lifecycle_capacity,
        ) {
            return Err("producer-continuation reservation crossed immutable geometry".to_owned());
        }
        if producer_continuations.values().any(|incumbent| {
            incumbent.identity == record.identity && incumbent.identity.address() != address
                || incumbent.identity.candidate == record.identity.candidate
                    && incumbent.identity != record.identity
                || incumbent.identity.admission_ordinal == record.identity.admission_ordinal
                    && incumbent.identity.stage == record.identity.stage
                    && incumbent.identity != record.identity
                || incumbent.status != ProducerContinuationStatus::Terminal
                    && record.status != ProducerContinuationStatus::Terminal
                    && incumbent.identity.admission_ordinal == record.identity.admission_ordinal
                    && incumbent.identity != record.identity
        }) {
            return Err(
                "producer-continuation reservation conflicts with a live lifecycle".to_owned(),
            );
        }
        let Some(incumbent) = producer_continuations.get(&address) else {
            if producer_continuations.len() >= self.producer_continuation_capacity {
                return Err("producer-continuation capacity is exhausted".to_owned());
            }
            producer_continuations.insert(address, record);
            return Ok(ProducerContinuationReservation::Inserted);
        };
        if incumbent.identity == record.identity {
            if incumbent != &record {
                return Err(
                    "an exact producer-continuation retry changed its frozen record".to_owned(),
                );
            }
            return Ok(ProducerContinuationReservation::Coalesced);
        }
        if incumbent.status != ProducerContinuationStatus::Terminal
            || incumbent.identity.candidate.context_id != record.identity.candidate.context_id
            || incumbent.identity.candidate.height != record.identity.candidate.height
            || incumbent.identity.candidate.source_view >= record.identity.candidate.source_view
            || incumbent.identity.admission_ordinal >= record.identity.admission_ordinal
        {
            return Err(
                "producer-continuation address replacement did not strictly advance a terminal owner"
                    .to_owned(),
            );
        }
        producer_continuations.insert(address, record);
        Ok(ProducerContinuationReservation::ReplacedTerminal)
    }

    /// Publish one complete canonical snapshot before its candidate owner retires.
    ///
    /// # Errors
    ///
    /// Returns an error when a record crosses the immutable store geometry or
    /// the checksummed atomic-replace publication cannot be synchronized.
    #[cfg(test)]
    pub(crate) fn persist(
        &self,
        records: &BTreeMap<ServicedCandidateKey, wire::View>,
        decision_reclaimed: bool,
    ) -> Result<(), String> {
        self.persist_with_producer_continuations(records, &BTreeMap::new(), decision_reclaimed)
    }

    /// Publish tombstones and the separately bounded producer-continuation
    /// lifecycle table.
    ///
    /// `Reserved` and `Materialized` records retain only immutable admission
    /// metadata. They cross this boundary so exact retry can recover the old
    /// slot and ordinal, but open always normalizes them to `Reserved`; their
    /// compact successor projection is never interpreted as a persisted
    /// command or as completion evidence.
    ///
    /// # Errors
    ///
    /// Returns an error when either logical table crosses its exact capacity,
    /// ordering, identity, or height-context geometry, or publication fails.
    pub(crate) fn persist_with_producer_continuations(
        &self,
        records: &BTreeMap<ServicedCandidateKey, wire::View>,
        producer_continuations: &BTreeMap<ProducerContinuationAddress, ProducerContinuationRecord>,
        decision_reclaimed: bool,
    ) -> Result<(), String> {
        if decision_reclaimed && (!records.is_empty() || !producer_continuations.is_empty()) {
            return Err(
                "a decision-reclaimed snapshot cannot retain service or producer owners".to_owned(),
            );
        }
        if !decision_reclaimed
            && producer_continuations.values().any(|record| {
                record.status == ProducerContinuationStatus::Terminal
                    && !records.contains_key(&record.identity.candidate)
            })
        {
            return Err(
                "a producer tombstone requires its exact durable service tombstone".to_owned(),
            );
        }
        if records.len() > self.serviced_capacity
            || records
                .keys()
                .any(|record| !record.belongs_to(self.context_id, self.height, self.owner))
            || producer_continuations.len() > self.producer_continuation_capacity
        {
            return Err(
                "serviced-candidate snapshot crossed its immutable context geometry".to_owned(),
            );
        }
        let persisted_producer_continuations = producer_continuations
            .iter()
            .map(|(address, record)| PersistedProducerContinuation {
                address: *address,
                record: record.clone(),
            })
            .collect::<Vec<_>>();
        if !producer_continuations_are_valid(
            &persisted_producer_continuations,
            self.context_id,
            self.height,
            self.owner,
            self.producer_continuation_lifecycle_capacity,
        ) {
            return Err(
                "producer-continuation snapshot crossed its immutable context geometry".to_owned(),
            );
        }
        let state = PersistedServicedCandidatesV4 {
            format_version: FORMAT_VERSION,
            context_id: self.context_id,
            height: self.height,
            owner: self.owner,
            serviced_capacity: u64::try_from(self.serviced_capacity)
                .map_err(|_| "serviced-candidate capacity is not representable".to_owned())?,
            producer_continuation_capacity: u64::try_from(self.producer_continuation_capacity)
                .map_err(|_| "producer-continuation capacity is not representable".to_owned())?,
            decision_reclaimed,
            records: records
                .iter()
                .map(|(key, service_view)| PersistedServicedCandidate {
                    key: *key,
                    service_view: *service_view,
                })
                .collect(),
            producer_continuations: persisted_producer_continuations,
        };
        let frame = encode_frame_v4(&state, self.max_frame_bytes)?;
        publish_atomic_frame(&self.path, &frame, "serviced-candidate")
    }

    /// Remove and directory-sync the finalized height's obsolete snapshot.
    ///
    /// # Errors
    ///
    /// Returns an error when the snapshot or its containing directory cannot
    /// be synchronized and retired.
    pub(crate) fn retire(self) -> Result<(), String> {
        let Some((file, opened_before)) = open_bound_snapshot(&self.path, self.max_frame_bytes)?
        else {
            return Ok(());
        };
        file.sync_all().map_err(|error| {
            format!(
                "failed to sync serviced-candidate snapshot {} for retirement: {error}",
                self.path.display()
            )
        })?;
        let opened_after = file.metadata().map_err(|error| {
            format!(
                "failed to reinspect serviced-candidate snapshot {} for retirement: {error}",
                self.path.display()
            )
        })?;
        let path_after = fs::symlink_metadata(&self.path).map_err(|error| {
            format!(
                "failed to bind serviced-candidate snapshot {} for retirement: {error}",
                self.path.display()
            )
        })?;
        if !snapshot_metadata_is_safe(&opened_after, self.max_frame_bytes)
            || !snapshot_metadata_is_safe(&path_after, self.max_frame_bytes)
            || !snapshot_metadata_unchanged(&opened_before, &opened_after)
            || !snapshot_metadata_unchanged(&opened_before, &path_after)
        {
            return Err(format!(
                "serviced-candidate snapshot {} changed before retirement",
                self.path.display()
            ));
        }
        drop(file);
        fs::remove_file(&self.path).map_err(|error| {
            format!(
                "failed to retire serviced-candidate snapshot {}: {error}",
                self.path.display()
            )
        })?;
        let parent = self
            .path
            .parent()
            .ok_or_else(|| "serviced-candidate snapshot path has no parent".to_owned())?;
        File::open(parent)
            .and_then(|directory| directory.sync_all())
            .map_err(|error| {
                format!(
                    "failed to sync retired serviced-candidate directory {}: {error}",
                    parent.display()
                )
            })
    }

    /// Return the exact snapshot path for failure-injection tests.
    #[cfg(test)]
    pub(crate) fn path_for_test(&self) -> &Path {
        &self.path
    }
}

fn temporary_path(path: &Path) -> Result<PathBuf, String> {
    let mut name = path
        .file_name()
        .ok_or_else(|| "serviced-candidate snapshot path has no file name".to_owned())?
        .to_os_string();
    name.push(".tmp");
    Ok(path.with_file_name(name))
}

fn publish_atomic_frame(path: &Path, frame: &[u8], label: &str) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("{label} snapshot path has no parent"))?;
    fs::create_dir_all(parent).map_err(|error| {
        format!(
            "failed to create {label} snapshot directory {}: {error}",
            parent.display()
        )
    })?;
    let temporary = temporary_path(path)?;
    if let Ok(metadata) = fs::symlink_metadata(&temporary) {
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(format!(
                "{label} temporary path {} is not a regular file",
                temporary.display()
            ));
        }
        fs::remove_file(&temporary).map_err(|error| {
            format!(
                "failed to remove stale {label} temporary file {}: {error}",
                temporary.display()
            )
        })?;
    }
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temporary)
        .map_err(|error| {
            format!(
                "failed to create {label} temporary snapshot {}: {error}",
                temporary.display()
            )
        })?;
    let publication = file
        .write_all(frame)
        .and_then(|()| file.flush())
        .and_then(|()| file.sync_all())
        .and_then(|()| fs::rename(&temporary, path))
        .and_then(|()| File::open(parent))
        .and_then(|directory| directory.sync_all());
    if let Err(error) = publication {
        drop(file);
        let _ = fs::remove_file(&temporary);
        return Err(format!(
            "failed to publish {label} snapshot {}: {error}",
            path.display()
        ));
    }
    Ok(())
}

fn encode_leader_wire_frame(
    state: &PersistedLeaderWireLifecycles,
    max_frame_bytes: u64,
) -> Result<Vec<u8>, String> {
    let payload = state.encode();
    let payload_len = u64::try_from(payload.len())
        .map_err(|_| "leader-wire lifecycle payload length overflowed".to_owned())?;
    let frame_len = u64::try_from(FRAME_HEADER_BYTES)
        .expect("leader-wire frame header fits u64")
        .checked_add(payload_len)
        .ok_or_else(|| "leader-wire lifecycle frame length overflowed".to_owned())?;
    if frame_len > max_frame_bytes {
        return Err("leader-wire lifecycle frame exceeds its derived byte bound".to_owned());
    }
    let mut frame = Vec::with_capacity(
        usize::try_from(frame_len)
            .map_err(|_| "leader-wire lifecycle frame is not addressable".to_owned())?,
    );
    frame.extend_from_slice(LEADER_WIRE_FRAME_MAGIC);
    frame.extend_from_slice(&LEADER_WIRE_FORMAT_VERSION.to_le_bytes());
    frame.extend_from_slice(&payload_len.to_le_bytes());
    frame.extend_from_slice(Hash::new(&payload).as_ref());
    frame.extend_from_slice(&payload);
    Ok(frame)
}

fn decode_leader_wire_frame(
    bytes: &[u8],
    max_frame_bytes: u64,
) -> Result<PersistedLeaderWireLifecycles, String> {
    if bytes.len() < FRAME_HEADER_BYTES
        || u64::try_from(bytes.len()).unwrap_or(u64::MAX) > max_frame_bytes
        || bytes.get(..LEADER_WIRE_FRAME_MAGIC.len()) != Some(LEADER_WIRE_FRAME_MAGIC.as_slice())
    {
        return Err("leader-wire lifecycle snapshot has an invalid frame header".to_owned());
    }
    let version_offset = LEADER_WIRE_FRAME_MAGIC.len();
    let version = u16::from_le_bytes(
        bytes[version_offset..version_offset + 2]
            .try_into()
            .map_err(|_| "leader-wire lifecycle version is truncated".to_owned())?,
    );
    if version != LEADER_WIRE_FORMAT_VERSION {
        return Err(format!(
            "leader-wire lifecycle snapshot uses unsupported version {version}"
        ));
    }
    let length_offset = version_offset + 2;
    let payload_len = u64::from_le_bytes(
        bytes[length_offset..length_offset + 8]
            .try_into()
            .map_err(|_| "leader-wire lifecycle length is truncated".to_owned())?,
    );
    let payload_len = usize::try_from(payload_len)
        .map_err(|_| "leader-wire lifecycle payload is not addressable".to_owned())?;
    let digest_offset = length_offset + 8;
    let payload_offset = digest_offset + HASH_BYTES;
    if payload_offset.checked_add(payload_len) != Some(bytes.len()) {
        return Err("leader-wire lifecycle frame length is inconsistent".to_owned());
    }
    let payload = &bytes[payload_offset..];
    if Hash::new(payload).as_ref() != &bytes[digest_offset..payload_offset] {
        return Err("leader-wire lifecycle snapshot checksum mismatch".to_owned());
    }
    let mut cursor = payload;
    let state = PersistedLeaderWireLifecycles::decode_all(&mut cursor)
        .map_err(|error| format!("failed to decode leader-wire lifecycle snapshot: {error}"))?;
    if state.format_version != LEADER_WIRE_FORMAT_VERSION || state.encode() != payload {
        return Err("leader-wire lifecycle snapshot is not canonically encoded".to_owned());
    }
    Ok(state)
}

fn encode_payload_frame(
    version: u16,
    payload: Vec<u8>,
    max_frame_bytes: u64,
) -> Result<Vec<u8>, String> {
    let payload_len = u64::try_from(payload.len())
        .map_err(|_| "serviced-candidate payload length overflowed".to_owned())?;
    let frame_len = u64::try_from(FRAME_HEADER_BYTES)
        .expect("serviced-candidate frame header fits u64")
        .checked_add(payload_len)
        .ok_or_else(|| "serviced-candidate frame length overflowed".to_owned())?;
    if frame_len > max_frame_bytes {
        return Err("serviced-candidate frame exceeds its derived byte bound".to_owned());
    }
    let mut frame = Vec::with_capacity(
        usize::try_from(frame_len)
            .map_err(|_| "serviced-candidate frame is not addressable".to_owned())?,
    );
    frame.extend_from_slice(FRAME_MAGIC);
    frame.extend_from_slice(&version.to_le_bytes());
    frame.extend_from_slice(&payload_len.to_le_bytes());
    frame.extend_from_slice(Hash::new(&payload).as_ref());
    frame.extend_from_slice(&payload);
    Ok(frame)
}

fn encode_frame_v4(
    state: &PersistedServicedCandidatesV4,
    max_frame_bytes: u64,
) -> Result<Vec<u8>, String> {
    encode_payload_frame(FORMAT_VERSION, state.encode(), max_frame_bytes)
}

#[cfg(test)]
fn encode_frame_v3(
    state: &PersistedServicedCandidatesV3,
    max_frame_bytes: u64,
) -> Result<Vec<u8>, String> {
    encode_payload_frame(FORMAT_VERSION_V3, state.encode(), max_frame_bytes)
}

fn decode_frame(bytes: &[u8], max_frame_bytes: u64) -> Result<DecodedServicedCandidates, String> {
    if bytes.len() < FRAME_HEADER_BYTES
        || u64::try_from(bytes.len()).unwrap_or(u64::MAX) > max_frame_bytes
        || bytes.get(..FRAME_MAGIC.len()) != Some(FRAME_MAGIC.as_slice())
    {
        return Err("serviced-candidate snapshot has an invalid frame header".to_owned());
    }
    let version_offset = FRAME_MAGIC.len();
    let version = u16::from_le_bytes(
        bytes[version_offset..version_offset + 2]
            .try_into()
            .map_err(|_| "serviced-candidate frame version is truncated".to_owned())?,
    );
    if !matches!(version, FORMAT_VERSION_V3 | FORMAT_VERSION) {
        return Err(format!(
            "serviced-candidate snapshot uses unsupported version {version}"
        ));
    }
    let length_offset = version_offset + 2;
    let payload_len = u64::from_le_bytes(
        bytes[length_offset..length_offset + 8]
            .try_into()
            .map_err(|_| "serviced-candidate frame length is truncated".to_owned())?,
    );
    let payload_len = usize::try_from(payload_len)
        .map_err(|_| "serviced-candidate payload is not addressable".to_owned())?;
    let digest_offset = length_offset + 8;
    let payload_offset = digest_offset + HASH_BYTES;
    if payload_offset.checked_add(payload_len) != Some(bytes.len()) {
        return Err("serviced-candidate frame length is inconsistent".to_owned());
    }
    let payload = &bytes[payload_offset..];
    if Hash::new(payload).as_ref() != &bytes[digest_offset..payload_offset] {
        return Err("serviced-candidate snapshot checksum mismatch".to_owned());
    }
    match version {
        FORMAT_VERSION_V3 => {
            let mut cursor = payload;
            let state =
                PersistedServicedCandidatesV3::decode_all(&mut cursor).map_err(|error| {
                    format!("failed to decode v3 serviced-candidate snapshot: {error}")
                })?;
            if state.format_version != FORMAT_VERSION_V3 || state.encode() != payload {
                return Err("v3 serviced-candidate snapshot is not canonically encoded".to_owned());
            }
            Ok(DecodedServicedCandidates {
                context_id: state.context_id,
                height: state.height,
                owner: state.owner,
                serviced_capacity: state.capacity,
                producer_continuation_capacity: state.capacity,
                decision_reclaimed: state.decision_reclaimed,
                records: state.records,
                producer_continuations: Vec::new(),
            })
        }
        FORMAT_VERSION => {
            let mut cursor = payload;
            let state =
                PersistedServicedCandidatesV4::decode_all(&mut cursor).map_err(|error| {
                    format!("failed to decode v4 serviced-candidate snapshot: {error}")
                })?;
            if state.format_version != FORMAT_VERSION || state.encode() != payload {
                return Err("v4 serviced-candidate snapshot is not canonically encoded".to_owned());
            }
            Ok(DecodedServicedCandidates {
                context_id: state.context_id,
                height: state.height,
                owner: state.owner,
                serviced_capacity: state.serviced_capacity,
                producer_continuation_capacity: state.producer_continuation_capacity,
                decision_reclaimed: state.decision_reclaimed,
                records: state.records,
                producer_continuations: state.producer_continuations,
            })
        }
        _ => unreachable!("unsupported versions return before payload decode"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sumeragi::{
        FairV2IngressLeaderWireIdentity, FairV2IngressLeaderWirePhase,
        FairV2IngressLeaderWireSourceClass,
    };
    use tempfile::TempDir;

    const OWNER_A: [u8; 32] = [0xA1; 32];
    const OWNER_B: [u8; 32] = [0xB2; 32];

    fn context() -> wire::HeightContext {
        context_with_roster_len(4)
    }

    fn context_with_roster_len(roster_len: usize) -> wire::HeightContext {
        use iroha_crypto::{Algorithm, KeyPair};
        use iroha_data_model::peer::PeerId;

        assert!((4..=31).contains(&roster_len) && (roster_len - 1) % 3 == 0);
        let mut roster = (0..roster_len)
            .map(|index| {
                let seed = u8::try_from(index + 7).expect("bounded deterministic seed");
                let key = KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                    .expect("deterministic validator");
                wire::ValidatorPower {
                    validator: PeerId::new(key.public_key().clone()),
                    power: 1,
                }
            })
            .collect::<Vec<_>>();
        roster.sort_by(|left, right| left.validator.cmp(&right.validator));
        let context = wire::HeightContext {
            chain_id: "serviced-candidate-test".into(),
            protocol_version: wire::PROTOCOL_VERSION,
            height: 7,
            epoch: 1,
            epoch_end_height: 100,
            next_epoch_snapshot: None,
            mode: wire::ConsensusMode::Permissioned,
            parent_commit_qc: None,
            snapshot_bootstrap: Some(wire::SnapshotBootstrapAnchor {
                snapshot_height: 6,
                snapshot_block_hash: iroha_crypto::HashOf::from_untyped_unchecked(Hash::new(
                    b"snapshot block",
                )),
                snapshot_block_creation_time_ms: 6_000,
                snapshot_state_hash: Hash::new(b"snapshot state"),
            }),
            quorum: wire::DualQuorum::from_roster(&roster).expect("quorum"),
            roster,
            nexus_amx_context_hash: Hash::new(b"nexus"),
            execution_policy_hash: iroha_crypto::Hash::new(b"test execution policy"),
            da_layout: wire::DataAvailabilityLayout {
                encoding: wire::PayloadEncoding::ReedSolomon16,
                chunk_size_bytes: 1024,
                data_shards: 1,
                parity_shards: 1,
                max_payload_size_bytes: 4096,
                max_chunk_count: 8,
            },
            leader_seed: [9; 32],
        };
        context.validate().expect("valid snapshot-bound context");
        context
    }

    fn successor_context(predecessor: &wire::HeightContext) -> wire::HeightContext {
        let round = wire::ConsensusRound {
            context_id: predecessor.id(),
            height: predecessor.height,
            view: 0,
        };
        let parent = wire::QuorumCertificate {
            round,
            proposal_round: round,
            phase: wire::GlobalPhase::Commit,
            subject: wire::BlockSubject {
                parent_block_hash: predecessor
                    .snapshot_bootstrap
                    .map(|anchor| anchor.snapshot_block_hash),
                block_hash: iroha_crypto::HashOf::from_untyped_unchecked(Hash::new(
                    b"predecessor block",
                )),
                payload_hash: Hash::new(b"predecessor payload"),
            },
            execution_commitment: wire::ExecutionCommitment::without_topups(
                Hash::new(b"predecessor parent state"),
                Hash::new(b"predecessor post state"),
                Hash::new(b"predecessor ordinary writes"),
                Hash::new(b"predecessor wire"),
            ),
            signers: vec![0, 1, 2],
            aggregate_signature: vec![0xA7; 96],
        };
        parent
            .validate(predecessor)
            .expect("structurally quorum-valid predecessor CommitQC");
        let mut successor = predecessor.clone();
        successor.height = predecessor
            .height
            .checked_add(1)
            .expect("fixture height has a successor");
        successor.parent_commit_qc = Some(parent);
        successor.snapshot_bootstrap = None;
        successor.validate().expect("valid successor context");
        assert_ne!(successor.id(), predecessor.id());
        successor
    }

    fn leader_wire_recovery_authority(
        context: &wire::HeightContext,
    ) -> LeaderWireRecoveryAuthority {
        leader_wire_recovery_authority_at(context, OWNER_A, 0, false)
    }

    fn leader_wire_recovery_authority_at(
        context: &wire::HeightContext,
        owner: [u8; 32],
        durable_view: wire::View,
        decision_durable: bool,
    ) -> LeaderWireRecoveryAuthority {
        LeaderWireRecoveryAuthority::from_replayed_adapter(
            context.id(),
            context.height,
            owner,
            durable_view,
            decision_durable,
        )
    }

    fn key_with_kind(
        context: &wire::HeightContext,
        source_view: u64,
        evidence: u8,
        kind: u8,
    ) -> ServicedCandidateKey {
        ServicedCandidateKey::new(
            context.id(),
            context.height,
            OWNER_A,
            context.leader(source_view),
            source_view,
            Some([evidence; 32]),
            1,
            3,
            kind,
            [evidence; 32],
        )
    }

    fn key(context: &wire::HeightContext, source_view: u64, evidence: u8) -> ServicedCandidateKey {
        key_with_kind(context, source_view, evidence, 2)
    }

    fn candidate_kind_for_stage(stage: u8) -> u8 {
        match stage {
            0..=6 => stage,
            7 => 8,
            8 => 9,
            9 => 10,
            10 => 14,
            _ => panic!("test producer stage must be tracked"),
        }
    }

    fn state(
        store: &ServicedCandidateStore,
        records: Vec<PersistedServicedCandidate>,
        decision_reclaimed: bool,
    ) -> PersistedServicedCandidatesV4 {
        PersistedServicedCandidatesV4 {
            format_version: FORMAT_VERSION,
            context_id: store.context_id,
            height: store.height,
            owner: store.owner,
            serviced_capacity: u64::try_from(store.serviced_capacity)
                .expect("test serviced capacity fits u64"),
            producer_continuation_capacity: u64::try_from(store.producer_continuation_capacity)
                .expect("test producer-continuation capacity fits u64"),
            decision_reclaimed,
            records,
            producer_continuations: Vec::new(),
        }
    }

    fn v3_state(
        store: &ServicedCandidateStore,
        records: Vec<PersistedServicedCandidate>,
        decision_reclaimed: bool,
    ) -> PersistedServicedCandidatesV3 {
        PersistedServicedCandidatesV3 {
            format_version: FORMAT_VERSION_V3,
            context_id: store.context_id,
            height: store.height,
            owner: store.owner,
            capacity: u64::try_from(store.serviced_capacity).expect("test capacity fits u64"),
            decision_reclaimed,
            records,
        }
    }

    fn continuation_identity(
        context: &wire::HeightContext,
        lifecycle_slot: u64,
        admission_ordinal: u128,
        stage: u8,
        evidence: u8,
    ) -> ProducerContinuationIdentity {
        ProducerContinuationIdentity::new(
            key_with_kind(context, 2, evidence, candidate_kind_for_stage(stage)),
            Hash::new([0xC1, evidence]),
            lifecycle_slot,
            admission_ordinal,
        )
        .expect("valid producer-continuation identity")
    }

    fn continuation_record(
        context: &wire::HeightContext,
        lifecycle_slot: u64,
        admission_ordinal: u128,
        stage: u8,
        status: ProducerContinuationStatus,
        handoff_stages: &[u8],
    ) -> ProducerContinuationRecord {
        let identity =
            continuation_identity(context, lifecycle_slot, admission_ordinal, stage, stage + 1);
        let mut handoff_candidates = handoff_stages
            .iter()
            .map(|successor_stage| {
                ProducerContinuationIdentity::new(
                    key_with_kind(
                        context,
                        2,
                        successor_stage + 32,
                        candidate_kind_for_stage(*successor_stage),
                    ),
                    identity.causal_lifecycle_key,
                    lifecycle_slot,
                    admission_ordinal,
                )
                .expect("valid exact successor identity")
            })
            .collect::<Vec<_>>();
        handoff_candidates.sort_unstable();
        ProducerContinuationRecord::new(identity, status, handoff_candidates)
            .expect("valid producer-continuation record")
    }

    fn leader_wire_token(
        context: &wire::HeightContext,
        view: wire::View,
        admission_ordinal: u64,
        scheduler_ordinal: u128,
        discriminator: u8,
    ) -> FairV2IngressLeaderWireToken {
        let origin = context.roster[0].validator.clone();
        let phase = FairV2IngressLeaderWirePhase::PrepareVote;
        FairV2IngressLeaderWireToken {
            identity: FairV2IngressLeaderWireIdentity {
                context_id: context.id(),
                height: context.height,
                view,
                subject_hash: Hash::new([0x51, discriminator]),
                manifest_hash: None,
                phase,
                semantic_origin: origin.clone(),
                canonical_wire_hash: Hash::new([0x52, discriminator]),
            },
            slot: FairV2IngressLeaderWireSlot {
                semantic_origin: origin,
                phase,
                chunk_index: None,
            },
            admission_ordinal,
            scheduler_ordinal,
            source_class: FairV2IngressLeaderWireSourceClass::Control,
        }
    }

    fn leader_wire_slot_token(
        context: &wire::HeightContext,
        origin: &PeerId,
        phase: FairV2IngressLeaderWirePhase,
        chunk_index: Option<u32>,
        admission_ordinal: u64,
        scheduler_ordinal: u128,
    ) -> FairV2IngressLeaderWireToken {
        let manifest_hash = matches!(
            phase,
            FairV2IngressLeaderWirePhase::Proposal
                | FairV2IngressLeaderWirePhase::Chunk
                | FairV2IngressLeaderWirePhase::CertifiedResponse
        )
        .then(|| Hash::new(b"shared leader-wire manifest"));
        FairV2IngressLeaderWireToken {
            identity: FairV2IngressLeaderWireIdentity {
                context_id: context.id(),
                height: context.height,
                view: 2,
                subject_hash: Hash::new(b"shared leader-wire subject"),
                manifest_hash,
                phase,
                semantic_origin: origin.clone(),
                canonical_wire_hash: Hash::new(b"shared leader-wire bytes"),
            },
            slot: FairV2IngressLeaderWireSlot {
                semantic_origin: origin.clone(),
                phase,
                chunk_index,
            },
            admission_ordinal,
            scheduler_ordinal,
            source_class: phase.source_class(),
        }
    }

    fn leader_wire_body_token(
        context: &wire::HeightContext,
        receipt: &DurableBodyReceipt,
        admission_ordinal: u64,
        scheduler_ordinal: u128,
    ) -> FairV2IngressLeaderWireToken {
        let origin = context.roster[0].validator.clone();
        let phase = FairV2IngressLeaderWirePhase::CertifiedResponse;
        FairV2IngressLeaderWireToken {
            identity: FairV2IngressLeaderWireIdentity {
                context_id: context.id(),
                height: context.height,
                view: receipt.round().view,
                subject_hash: Hash::new(receipt.subject().encode()),
                manifest_hash: Some(receipt.manifest_hash().into()),
                phase,
                semantic_origin: origin.clone(),
                canonical_wire_hash: Hash::new(b"durable body terminal response"),
            },
            slot: FairV2IngressLeaderWireSlot {
                semantic_origin: origin,
                phase,
                chunk_index: None,
            },
            admission_ordinal,
            scheduler_ordinal,
            source_class: FairV2IngressLeaderWireSourceClass::CertifiedResponse,
        }
    }

    fn matching_terminal(
        context: &wire::HeightContext,
        runtime_owner: LeaderWireRuntimeOwner,
        token: &FairV2IngressLeaderWireToken,
    ) -> ProducerContinuationTerminalToken {
        let (kind, phase) = match token.identity.phase {
            FairV2IngressLeaderWirePhase::Proposal => (1, 0),
            FairV2IngressLeaderWirePhase::PrepareVote => (2, 1),
            FairV2IngressLeaderWirePhase::CommitVote => (2, 2),
            FairV2IngressLeaderWirePhase::PrepareQc => (3, 1),
            FairV2IngressLeaderWirePhase::CommitQc => (3, 2),
            FairV2IngressLeaderWirePhase::TimeoutVote => (4, 3),
            FairV2IngressLeaderWirePhase::TimeoutCertificate => (5, 3),
            FairV2IngressLeaderWirePhase::Chunk
            | FairV2IngressLeaderWirePhase::CertifiedResponse => {
                panic!("body transport cannot mint a producer terminal fixture")
            }
        };
        let candidate = ServicedCandidateKey::new(
            context.id(),
            context.height,
            OWNER_A,
            context.leader(token.identity.view),
            token.identity.view,
            Some([0xD1; 32]),
            phase,
            3,
            kind,
            [0xD1; 32],
        );
        let identity = ProducerContinuationIdentity::new(
            candidate,
            runtime_owner.causal_lifecycle_key(),
            1,
            runtime_owner.admission_ordinal(),
        )
        .expect("matching producer identity");
        ProducerContinuationRecord::new(identity, ProducerContinuationStatus::Terminal, Vec::new())
            .expect("matching producer terminal")
            .terminal_token()
            .expect("terminal token")
    }

    fn terminal_continuation_at_view(
        context: &wire::HeightContext,
        lifecycle_slot: u64,
        admission_ordinal: u128,
        stage: u8,
        source_view: wire::View,
        evidence: u8,
    ) -> ProducerContinuationRecord {
        let identity = ProducerContinuationIdentity::new(
            key_with_kind(
                context,
                source_view,
                evidence,
                candidate_kind_for_stage(stage),
            ),
            Hash::new([0xD2, evidence]),
            lifecycle_slot,
            admission_ordinal,
        )
        .expect("valid terminal continuation identity");
        ProducerContinuationRecord::new(identity, ProducerContinuationStatus::Terminal, Vec::new())
            .expect("valid terminal continuation")
    }

    fn write_frame(store: &ServicedCandidateStore, state: &PersistedServicedCandidatesV4) {
        let frame = encode_frame_v4(state, store.max_frame_bytes).expect("encode fixture frame");
        fs::write(store.path_for_test(), frame).expect("write fixture frame");
    }

    #[test]
    fn snapshot_roundtrips_and_rejects_a_b_a_resurrection() {
        let directory = TempDir::new().expect("temporary directory");
        let context = context();
        let wal = directory.path().join("00000000000000000007.wal");
        let (store, restored) =
            ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 4)
                .expect("open snapshot");
        assert!(restored.records.is_empty());
        let a = key(&context, 2, 1);
        let b = key(&context, 2, 2);
        assert_eq!(a.class(), 3);
        let service_view = 5;
        let mut records = BTreeMap::from([(a, service_view), (b, service_view)]);
        store.persist(&records, false).expect("persist A and B");
        assert_eq!(
            records.insert(a, service_view),
            Some(service_view),
            "A remains serviced after equal-rank B replacement"
        );
        let (_reopened, restored) =
            ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 4)
                .expect("same-height reopen");
        assert_eq!(restored.records, records);
    }

    #[test]
    fn v4_roundtrips_terminal_producer_continuations_and_v3_upgrades_canonically() {
        let directory = TempDir::new().expect("temporary directory");
        let context = context();
        let wal = directory.path().join("continuations.wal");
        let (store, _) =
            ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 4)
                .expect("open v4 snapshot");
        let first_terminal =
            continuation_record(&context, 1, 1, 1, ProducerContinuationStatus::Terminal, &[]);
        let terminal =
            continuation_record(&context, 2, 2, 3, ProducerContinuationStatus::Terminal, &[]);
        let producer_continuations = BTreeMap::from([
            (first_terminal.identity.address(), first_terminal),
            (terminal.identity.address(), terminal),
        ]);
        let serviced = producer_continuations
            .values()
            .map(|record| {
                let candidate = record.identity().candidate();
                (candidate, candidate.source_view())
            })
            .collect::<BTreeMap<_, _>>();
        assert!(
            store
                .persist_with_producer_continuations(
                    &BTreeMap::new(),
                    &producer_continuations,
                    false,
                )
                .is_err(),
            "a terminal producer cannot outlive its durable service tombstone"
        );
        store
            .persist_with_producer_continuations(&serviced, &producer_continuations, false)
            .expect("persist v4 producer continuations");
        let (_, restored) =
            ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 4)
                .expect("restore v4 producer continuations");
        assert_eq!(restored.records, serviced);
        assert_eq!(restored.producer_continuations, producer_continuations);

        let active =
            continuation_record(&context, 3, 3, 4, ProducerContinuationStatus::Reserved, &[]);
        let active_table = BTreeMap::from([(active.identity.address(), active.clone())]);
        store
            .persist_with_producer_continuations(&serviced, &active_table, false)
            .expect("persist exact active admission metadata");
        let (_, active_restored) =
            ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 4)
                .expect("restore active producer admission metadata");
        assert_eq!(active_restored.producer_continuations, active_table);
        assert_eq!(
            active_restored.producer_continuations[&active.identity.address()].status(),
            ProducerContinuationStatus::Reserved
        );

        let materialized = continuation_record(
            &context,
            4,
            4,
            1,
            ProducerContinuationStatus::Materialized,
            &[2],
        );
        store
            .persist_with_producer_continuations(
                &serviced,
                &BTreeMap::from([(materialized.identity.address(), materialized.clone())]),
                false,
            )
            .expect("persist materialized admission metadata");
        let (_, materialized_restored) =
            ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 4)
                .expect("restore materialized producer metadata");
        let reopened =
            &materialized_restored.producer_continuations[&materialized.identity.address()];
        assert_eq!(reopened.status(), ProducerContinuationStatus::Reserved);
        assert!(reopened.handoff_candidates.is_empty());

        let v3_wal = directory.path().join("v3-compatible.wal");
        let (v3_store, _) =
            ServicedCandidateStore::open(&v3_wal, context.id(), context.height, OWNER_A, 4)
                .expect("derive v3-compatible snapshot path");
        let v3 = v3_state(
            &v3_store,
            vec![PersistedServicedCandidate {
                key: key(&context, 2, 7),
                service_view: 2,
            }],
            false,
        );
        let v3_frame =
            encode_frame_v3(&v3, v3_store.max_frame_bytes).expect("encode canonical v3 frame");
        fs::write(v3_store.path_for_test(), v3_frame).expect("write canonical v3 frame");
        let (v3_store, restored) =
            ServicedCandidateStore::open(&v3_wal, context.id(), context.height, OWNER_A, 4)
                .expect("restore exact v3 payload");
        assert_eq!(restored.records, BTreeMap::from([(key(&context, 2, 7), 2)]));
        assert!(restored.producer_continuations.is_empty());
        v3_store
            .persist(&restored.records, restored.decision_reclaimed)
            .expect("canonically upgrade v3 payload to v4");
        let upgraded = fs::read(v3_store.path_for_test()).expect("read upgraded frame");
        assert_eq!(
            &upgraded[FRAME_MAGIC.len()..FRAME_MAGIC.len() + 2],
            &FORMAT_VERSION.to_le_bytes()
        );
    }

    #[test]
    fn leader_wire_gate_restores_both_high_waters_and_normalizes_active_cuts() {
        let directory = TempDir::new().expect("temporary directory");
        let context = context();
        let wal = directory.path().join("leader-wire-active.wal");
        let roster = context
            .roster
            .iter()
            .map(|entry| entry.validator.clone())
            .collect::<BTreeSet<_>>();
        let max_chunks = 4;
        let capacity = LeaderWireLifecycleStoreGate::derived_capacity(roster.len(), max_chunks)
            .expect("derived gate capacity");
        let (gate, restore) = LeaderWireLifecycleStoreGate::open(
            &wal,
            context.id(),
            context.height,
            OWNER_A,
            roster.clone(),
            capacity,
            max_chunks,
            leader_wire_recovery_authority(&context),
            &[],
            &[],
        )
        .expect("open empty leader-wire gate");
        assert_eq!(restore.last_admission_ordinal(), 0);
        assert_eq!(restore.scheduler_ordinal_high_watermark(), 0);
        assert!(gate.matches_geometry(context.id(), context.height, &roster, capacity, max_chunks));

        let token = leader_wire_token(&context, 2, 7, 41, 1);
        let reserved = gate.reserve(token.clone()).expect("persist Reserved");
        assert!(reserved.inserted());
        gate.mark_ingress(&token).expect("persist Ingress");
        let runtime_owner =
            LeaderWireRuntimeOwner::new(token.identity_hash(), 41).expect("runtime owner");
        gate.mark_runtime(&token, runtime_owner)
            .expect("persist Runtime");

        let (reopened, restore) = LeaderWireLifecycleStoreGate::open(
            &wal,
            context.id(),
            context.height,
            OWNER_A,
            roster.clone(),
            capacity,
            max_chunks,
            leader_wire_recovery_authority(&context),
            &[],
            &[],
        )
        .expect("reopen active leader-wire gate");
        assert_eq!(restore.last_admission_ordinal(), 7);
        assert_eq!(restore.scheduler_ordinal_high_watermark(), 41);
        assert_eq!(restore.records().len(), 1);
        assert_eq!(
            restore.records()[0].status(),
            LeaderWireLifecycleStatus::Dormant
        );
        assert_eq!(restore.records()[0].runtime_owner(), Some(runtime_owner));
        assert_eq!(
            reopened
                .earliest_ingress_scheduler_ordinal()
                .expect("selector minimum"),
            None,
            "a restored lifecycle without a carrier is replay-dormant"
        );
        let retry = reopened
            .reserve(leader_wire_token(&context, 2, 99, 100, 1))
            .expect("exact retry coalesces to old durable token");
        assert!(!retry.inserted());
        assert_eq!(retry.token().admission_ordinal(), 7);
        assert_eq!(retry.token().scheduler_ordinal(), 41);
        reopened
            .mark_ingress(retry.token())
            .expect("exact physical retry reactivates the selector owner");
        assert_eq!(
            reopened
                .earliest_ingress_scheduler_ordinal()
                .expect("reactivated selector minimum"),
            Some(41)
        );
    }

    #[test]
    fn leader_wire_gate_retains_independent_cross_origin_phase_and_chunk_slots() {
        let directory = TempDir::new().expect("temporary directory");
        let context = context_with_roster_len(4);
        let wal = directory.path().join("leader-wire-owner-universe.wal");
        let roster = context
            .roster
            .iter()
            .map(|entry| entry.validator.clone())
            .collect::<BTreeSet<_>>();
        let max_chunks = 3;
        let per_origin_capacity = usize::try_from(max_chunks).expect("chunk count fits usize") + 8;
        let capacity = LeaderWireLifecycleStoreGate::derived_capacity(roster.len(), max_chunks)
            .expect("derived gate capacity");
        assert_eq!(capacity, roster.len() * per_origin_capacity);
        let (gate, _) = LeaderWireLifecycleStoreGate::open(
            &wal,
            context.id(),
            context.height,
            OWNER_A,
            roster.clone(),
            capacity,
            max_chunks,
            leader_wire_recovery_authority(&context),
            &[],
            &[],
        )
        .expect("open leader-wire owner-universe gate");

        let mut slots = vec![
            (FairV2IngressLeaderWirePhase::Proposal, None),
            (FairV2IngressLeaderWirePhase::PrepareVote, None),
            (FairV2IngressLeaderWirePhase::CommitVote, None),
            (FairV2IngressLeaderWirePhase::PrepareQc, None),
            (FairV2IngressLeaderWirePhase::CommitQc, None),
            (FairV2IngressLeaderWirePhase::TimeoutVote, None),
            (FairV2IngressLeaderWirePhase::TimeoutCertificate, None),
            (FairV2IngressLeaderWirePhase::CertifiedResponse, None),
        ];
        slots.extend(
            (0..max_chunks)
                .map(|chunk_index| (FairV2IngressLeaderWirePhase::Chunk, Some(chunk_index))),
        );
        assert_eq!(slots.len(), per_origin_capacity);

        let mut admitted = Vec::with_capacity(capacity);
        for origin in &roster {
            for (phase, chunk_index) in &slots {
                let ordinal =
                    u64::try_from(admitted.len() + 1).expect("test owner universe fits u64");
                let scheduler_ordinal = u128::from(ordinal) * 2;
                let token = leader_wire_slot_token(
                    &context,
                    origin,
                    *phase,
                    *chunk_index,
                    ordinal,
                    scheduler_ordinal,
                );
                let reserved = gate
                    .reserve(token.clone())
                    .expect("reserve exact owner slot");
                assert!(reserved.inserted());

                let mut retry = token.clone();
                retry.admission_ordinal = ordinal
                    .checked_add(u64::try_from(capacity).expect("capacity fits u64"))
                    .expect("retry ordinal fits u64");
                retry.scheduler_ordinal = scheduler_ordinal
                    .checked_add(u128::try_from(capacity).expect("capacity fits u128"))
                    .expect("retry scheduler ordinal fits u128");
                let coalesced = gate.reserve(retry).expect("coalesce only the exact slot");
                assert!(!coalesced.inserted());
                assert_eq!(coalesced.token(), &token);
                admitted.push(token);
            }
        }

        let (reopened, restored) = LeaderWireLifecycleStoreGate::open(
            &wal,
            context.id(),
            context.height,
            OWNER_A,
            roster.clone(),
            capacity,
            max_chunks,
            leader_wire_recovery_authority(&context),
            &[],
            &[],
        )
        .expect("reopen complete leader-wire owner universe");
        assert_eq!(restored.records().len(), capacity);
        let expected_slots = admitted
            .iter()
            .map(|token| token.slot.clone())
            .collect::<BTreeSet<_>>();
        let restored_slots = restored
            .records()
            .iter()
            .map(|record| record.token().slot.clone())
            .collect::<BTreeSet<_>>();
        assert_eq!(restored_slots, expected_slots);
        let expected_non_chunk_phases = slots
            .iter()
            .filter_map(|(phase, chunk_index)| chunk_index.is_none().then_some(*phase))
            .collect::<BTreeSet<_>>();
        let expected_chunk_indices = (0..max_chunks).collect::<BTreeSet<_>>();
        for origin in &roster {
            let records = restored
                .records()
                .iter()
                .filter(|record| record.token().slot.semantic_origin == *origin)
                .collect::<Vec<_>>();
            assert_eq!(records.len(), per_origin_capacity);
            let non_chunk_phases = records
                .iter()
                .filter_map(|record| {
                    record
                        .token()
                        .slot
                        .chunk_index
                        .is_none()
                        .then_some(record.token().slot.phase)
                })
                .collect::<BTreeSet<_>>();
            assert_eq!(non_chunk_phases, expected_non_chunk_phases);
            let chunk_indices = records
                .iter()
                .filter_map(|record| record.token().slot.chunk_index)
                .collect::<BTreeSet<_>>();
            assert_eq!(chunk_indices, expected_chunk_indices);
            let chunk_identity_hashes = records
                .iter()
                .filter(|record| record.token().slot.phase == FairV2IngressLeaderWirePhase::Chunk)
                .map(|record| record.token().identity_hash())
                .collect::<Vec<_>>();
            assert_eq!(
                chunk_identity_hashes.len(),
                usize::try_from(max_chunks).expect("chunk count fits usize")
            );
            assert!(
                chunk_identity_hashes
                    .iter()
                    .all(|identity_hash| *identity_hash == chunk_identity_hashes[0]),
                "chunk positions sharing every identity component still own distinct slots"
            );
        }
        assert_eq!(admitted.len(), capacity);

        let terminal_target = admitted
            .iter()
            .find(|token| token.slot.phase == FairV2IngressLeaderWirePhase::PrepareVote)
            .expect("one PrepareVote slot")
            .clone();
        let replay = reopened
            .reserve(terminal_target.clone())
            .expect("reactivate the exact restart-dormant target");
        assert!(!replay.inserted());
        assert_eq!(replay.token(), &terminal_target);
        reopened
            .mark_ingress(&terminal_target)
            .expect("replay target ingress after restart");
        let runtime_owner = LeaderWireRuntimeOwner::new(
            terminal_target.identity_hash(),
            terminal_target.scheduler_ordinal(),
        )
        .expect("exact runtime owner");
        let runtime = reopened
            .mark_runtime(&terminal_target, runtime_owner)
            .expect("rebind exact runtime owner after restart");
        let producer_terminal = matching_terminal(&context, runtime_owner, &terminal_target);
        reopened
            .mark_producer_terminal(&runtime, producer_terminal)
            .expect("publish exact restart-stable terminal");

        let (terminal_gate, terminal_restore) = LeaderWireLifecycleStoreGate::open(
            &wal,
            context.id(),
            context.height,
            OWNER_A,
            roster,
            capacity,
            max_chunks,
            leader_wire_recovery_authority(&context),
            &[producer_terminal],
            &[],
        )
        .expect("reopen complete owner universe with exact terminal evidence");
        let terminal_record = terminal_restore
            .records()
            .iter()
            .find(|record| record.token().slot == terminal_target.slot)
            .expect("terminal slot remains present");
        assert_eq!(
            terminal_record.status(),
            LeaderWireLifecycleStatus::Terminal
        );
        assert_eq!(terminal_record.token(), &terminal_target);

        let mut terminal_retry = terminal_target.clone();
        terminal_retry.admission_ordinal =
            u64::try_from(capacity + 1).expect("capacity successor fits u64");
        terminal_retry.scheduler_ordinal =
            u128::try_from(2 * capacity + 1).expect("scheduler successor fits u128");
        let suppressed = terminal_gate
            .reserve(terminal_retry)
            .expect("exact terminal retry remains coalesced after restart");
        assert!(!suppressed.inserted());
        assert_eq!(suppressed.status(), LeaderWireLifecycleStatus::Terminal);
        assert_eq!(suppressed.token(), &terminal_target);
    }

    #[test]
    fn leader_wire_gate_reconciles_producer_first_terminal_crash() {
        let directory = TempDir::new().expect("temporary directory");
        let context = context();
        let wal = directory.path().join("leader-wire-terminal.wal");
        let roster = context
            .roster
            .iter()
            .map(|entry| entry.validator.clone())
            .collect::<BTreeSet<_>>();
        let max_chunks = 2;
        let capacity = LeaderWireLifecycleStoreGate::derived_capacity(roster.len(), max_chunks)
            .expect("derived gate capacity");
        let (gate, _) = LeaderWireLifecycleStoreGate::open(
            &wal,
            context.id(),
            context.height,
            OWNER_A,
            roster.clone(),
            capacity,
            max_chunks,
            leader_wire_recovery_authority(&context),
            &[],
            &[],
        )
        .expect("open leader-wire gate");
        let token = leader_wire_token(&context, 2, 11, 73, 2);
        gate.reserve(token.clone()).expect("reserve");
        gate.mark_ingress(&token).expect("mark ingress");
        let runtime_owner =
            LeaderWireRuntimeOwner::new(token.identity_hash(), 73).expect("runtime owner");
        gate.mark_runtime(&token, runtime_owner)
            .expect("mark runtime");
        let producer_terminal = matching_terminal(&context, runtime_owner, &token);

        let (reconciled, restore) = LeaderWireLifecycleStoreGate::open(
            &wal,
            context.id(),
            context.height,
            OWNER_A,
            roster.clone(),
            capacity,
            max_chunks,
            leader_wire_recovery_authority(&context),
            &[producer_terminal],
            &[],
        )
        .expect("producer-first crash promotes wire terminal");
        assert_eq!(
            restore.records()[0].status(),
            LeaderWireLifecycleStatus::Terminal
        );
        assert_eq!(
            restore.records()[0].terminal_evidence(),
            Some(&LeaderWireStableTerminalEvidence::Producer(
                producer_terminal
            ))
        );
        let suppressed = reconciled
            .reserve(leader_wire_token(&context, 2, 88, 101, 2))
            .expect("exact terminal retry is suppressed");
        assert_eq!(suppressed.status(), LeaderWireLifecycleStatus::Terminal);
        assert_eq!(suppressed.token().scheduler_ordinal(), 73);

        assert!(
            LeaderWireLifecycleStoreGate::open(
                &wal,
                context.id(),
                context.height,
                OWNER_A,
                roster,
                capacity,
                max_chunks,
                leader_wire_recovery_authority(&context),
                &[],
                &[],
            )
            .is_err(),
            "wire Terminal without its producer terminal fails closed"
        );
    }

    #[test]
    fn leader_wire_gate_rejects_producer_terminal_from_foreign_view_or_phase() {
        let directory = TempDir::new().expect("temporary directory");
        let context = context();
        let wal = directory.path().join("leader-wire-terminal-binding.wal");
        let roster = context
            .roster
            .iter()
            .map(|entry| entry.validator.clone())
            .collect::<BTreeSet<_>>();
        let max_chunks = 2;
        let capacity = LeaderWireLifecycleStoreGate::derived_capacity(roster.len(), max_chunks)
            .expect("derived gate capacity");
        let (gate, _) = LeaderWireLifecycleStoreGate::open(
            &wal,
            context.id(),
            context.height,
            OWNER_A,
            roster.clone(),
            capacity,
            max_chunks,
            leader_wire_recovery_authority(&context),
            &[],
            &[],
        )
        .expect("open leader-wire gate");
        let token = leader_wire_token(&context, 2, 11, 73, 2);
        gate.reserve(token.clone()).expect("reserve");
        gate.mark_ingress(&token).expect("mark ingress");
        let runtime_owner =
            LeaderWireRuntimeOwner::new(token.identity_hash(), 73).expect("runtime owner");
        let runtime = gate
            .mark_runtime(&token, runtime_owner)
            .expect("mark runtime");

        let mut foreign_view = token.clone();
        foreign_view.identity.view = 1;
        let foreign_view_terminal = matching_terminal(&context, runtime_owner, &foreign_view);
        assert!(
            gate.mark_producer_terminal(&runtime, foreign_view_terminal)
                .is_err(),
            "same causal owner and ordinal cannot authenticate a foreign source view"
        );

        let mut foreign_phase = token.clone();
        foreign_phase.identity.phase = FairV2IngressLeaderWirePhase::CommitVote;
        foreign_phase.slot.phase = FairV2IngressLeaderWirePhase::CommitVote;
        let foreign_phase_terminal = matching_terminal(&context, runtime_owner, &foreign_phase);
        assert!(
            gate.mark_producer_terminal(&runtime, foreign_phase_terminal)
                .is_err(),
            "same causal owner and ordinal cannot authenticate a foreign protocol phase"
        );

        let (reopened, restore) = LeaderWireLifecycleStoreGate::open(
            &wal,
            context.id(),
            context.height,
            OWNER_A,
            roster,
            capacity,
            max_chunks,
            leader_wire_recovery_authority(&context),
            &[foreign_view_terminal, foreign_phase_terminal],
            &[],
        )
        .expect("foreign producer terminals cannot suppress exact replay");
        assert_eq!(
            restore.records()[0].status(),
            LeaderWireLifecycleStatus::Dormant
        );
        assert!(restore.records()[0].terminal_evidence().is_none());
        let replay = reopened
            .reserve(token.clone())
            .expect("reactivate the exact restart-dormant owner");
        assert!(!replay.inserted());
        assert_eq!(replay.token(), &token);
        reopened.mark_ingress(&token).expect("replay exact ingress");
        let runtime = reopened
            .mark_runtime(&token, runtime_owner)
            .expect("rebind exact runtime owner");
        reopened
            .mark_producer_terminal(&runtime, matching_terminal(&context, runtime_owner, &token))
            .expect("exact view and phase publish the producer terminal");
    }

    #[test]
    fn leader_wire_recovery_authority_retires_obsolete_records_and_retains_highwaters() {
        for (label, durable_view, decision_durable, publish_terminal) in [
            ("advanced-view", 3, false, false),
            ("decision", 2, true, true),
        ] {
            let directory = TempDir::new().expect("temporary directory");
            let context = context();
            let wal = directory.path().join(format!("leader-wire-{label}.wal"));
            let roster = context
                .roster
                .iter()
                .map(|entry| entry.validator.clone())
                .collect::<BTreeSet<_>>();
            let max_chunks = 2;
            let capacity = LeaderWireLifecycleStoreGate::derived_capacity(roster.len(), max_chunks)
                .expect("derived gate capacity");
            let (gate, _) = LeaderWireLifecycleStoreGate::open(
                &wal,
                context.id(),
                context.height,
                OWNER_A,
                roster.clone(),
                capacity,
                max_chunks,
                leader_wire_recovery_authority(&context),
                &[],
                &[],
            )
            .expect("open leader-wire gate");
            let token = leader_wire_token(&context, 2, 11, 73, 2);
            gate.reserve(token.clone()).expect("reserve");
            gate.mark_ingress(&token).expect("mark ingress");
            let runtime_owner =
                LeaderWireRuntimeOwner::new(token.identity_hash(), 73).expect("runtime owner");
            let runtime = gate
                .mark_runtime(&token, runtime_owner)
                .expect("mark runtime");
            if publish_terminal {
                gate.mark_producer_terminal(
                    &runtime,
                    matching_terminal(&context, runtime_owner, &token),
                )
                .expect("publish independently durable terminal before Decision");
            }

            let (reopened, restore) = LeaderWireLifecycleStoreGate::open(
                &wal,
                context.id(),
                context.height,
                OWNER_A,
                roster,
                capacity,
                max_chunks,
                leader_wire_recovery_authority_at(
                    &context,
                    OWNER_A,
                    durable_view,
                    decision_durable,
                ),
                &[],
                &[],
            )
            .expect("replay authority retires the obsolete lifecycle");
            assert!(restore.records().is_empty(), "{label}");
            assert_eq!(restore.last_admission_ordinal(), 11, "{label}");
            assert_eq!(restore.scheduler_ordinal_high_watermark(), 73, "{label}");
            assert!(
                reopened.reserve(token).is_err(),
                "{label} cannot reuse the retired physical ordinals"
            );
            let newer = leader_wire_token(
                &context,
                durable_view.checked_add(1).expect("fixture view advances"),
                12,
                74,
                3,
            );
            if decision_durable {
                assert!(
                    reopened.reserve(newer).is_err(),
                    "Decision retires every same-height lifecycle"
                );
            } else {
                reopened
                    .reserve(newer)
                    .expect("a strictly newer view remains admissible");
            }
        }
    }

    #[test]
    fn leader_wire_recovery_cut_keeps_body_transport_admissible() {
        for (label, durable_view, decision_durable, control_view) in
            [("advanced-view", 3, false, 2), ("decision", 3, true, 4)]
        {
            let directory = TempDir::new().expect("temporary directory");
            let context = context();
            let wal = directory
                .path()
                .join(format!("leader-wire-body-{label}.wal"));
            let roster = context
                .roster
                .iter()
                .map(|entry| entry.validator.clone())
                .collect::<BTreeSet<_>>();
            let max_chunks = 2;
            let capacity = LeaderWireLifecycleStoreGate::derived_capacity(roster.len(), max_chunks)
                .expect("derived gate capacity");
            let (gate, _) = LeaderWireLifecycleStoreGate::open(
                &wal,
                context.id(),
                context.height,
                OWNER_A,
                roster,
                capacity,
                max_chunks,
                leader_wire_recovery_authority_at(
                    &context,
                    OWNER_A,
                    durable_view,
                    decision_durable,
                ),
                &[],
                &[],
            )
            .expect("open leader-wire gate at the durable cut");

            let control = leader_wire_token(&context, control_view, 1, 1, 0x91);
            let origin = context.roster[0].validator.clone();
            let chunk = leader_wire_slot_token(
                &context,
                &origin,
                FairV2IngressLeaderWirePhase::Chunk,
                Some(0),
                2,
                2,
            );
            let response = leader_wire_slot_token(
                &context,
                &origin,
                FairV2IngressLeaderWirePhase::CertifiedResponse,
                None,
                3,
                3,
            );

            assert!(
                gate.identity_is_obsolete(&control.identity)
                    .expect("inspect control identity"),
                "{label} closes obsolete control"
            );
            assert!(
                !gate
                    .identity_is_obsolete(&chunk.identity)
                    .expect("inspect chunk identity"),
                "{label} keeps an exact body chunk eligible"
            );
            assert!(
                !gate
                    .identity_is_obsolete(&response.identity)
                    .expect("inspect response identity"),
                "{label} keeps an exact certified body response eligible"
            );
            assert!(
                gate.reserve(control).is_err(),
                "{label} rejects obsolete control admission"
            );
            gate.reserve(chunk)
                .expect("the downstream fetch must decide whether the chunk is relevant");
            gate.reserve(response)
                .expect("the downstream request must authenticate the certified response");
        }
    }

    #[test]
    fn leader_wire_live_recovery_cut_retires_only_dormant_records_and_is_monotone() {
        for (label, next_view, decision_durable) in
            [("advanced-view", 3, false), ("decision", 2, true)]
        {
            let directory = TempDir::new().expect("temporary directory");
            let context = context();
            let wal = directory
                .path()
                .join(format!("leader-wire-live-{label}.wal"));
            let roster = context
                .roster
                .iter()
                .map(|entry| entry.validator.clone())
                .collect::<BTreeSet<_>>();
            let max_chunks = 2;
            let capacity = LeaderWireLifecycleStoreGate::derived_capacity(roster.len(), max_chunks)
                .expect("derived gate capacity");
            let initial_authority = leader_wire_recovery_authority(&context);
            let (gate, _) = LeaderWireLifecycleStoreGate::open(
                &wal,
                context.id(),
                context.height,
                OWNER_A,
                roster.clone(),
                capacity,
                max_chunks,
                initial_authority,
                &[],
                &[],
            )
            .expect("open leader-wire gate");
            let token = leader_wire_token(&context, 2, 11, 73, 2);
            gate.reserve(token.clone()).expect("reserve restart owner");
            drop(gate);

            let (gate, restore) = LeaderWireLifecycleStoreGate::open(
                &wal,
                context.id(),
                context.height,
                OWNER_A,
                roster,
                capacity,
                max_chunks,
                initial_authority,
                &[],
                &[],
            )
            .expect("reopen restart owner as dormant");
            assert_eq!(restore.records().len(), 1, "{label}");
            assert_eq!(
                restore.records()[0].status(),
                LeaderWireLifecycleStatus::Dormant,
                "{label}"
            );

            let next =
                leader_wire_recovery_authority_at(&context, OWNER_A, next_view, decision_durable);
            let expected = BTreeSet::from([token.slot.clone()]);
            assert!(
                gate.advance_recovery_cut(
                    leader_wire_recovery_authority_at(
                        &context,
                        OWNER_B,
                        next_view,
                        decision_durable,
                    ),
                    &expected,
                )
                .is_err(),
                "{label} cannot cross immutable owner geometry"
            );
            gate.advance_recovery_cut(next, &expected)
                .expect("advance the live recovery cut");
            gate.advance_recovery_cut(next, &BTreeSet::new())
                .expect("repeating the exact recovery cut is idempotent");

            let restored = gate.restore().expect("inspect retired dormant owner");
            assert!(restored.records().is_empty(), "{label}");
            assert_eq!(restored.last_admission_ordinal(), 11, "{label}");
            assert_eq!(restored.scheduler_ordinal_high_watermark(), 73, "{label}");
            assert!(
                gate.identity_is_obsolete(&token.identity)
                    .expect("inspect live recovery cut"),
                "{label} rejects the retired identity without an exact retry"
            );

            let regressed = leader_wire_recovery_authority_at(
                &context,
                OWNER_A,
                next_view.saturating_sub(1),
                false,
            );
            assert!(
                gate.advance_recovery_cut(regressed, &BTreeSet::new())
                    .is_err(),
                "{label} cannot regress durable view/Decision authority"
            );

            let fresh = leader_wire_token(&context, 3, 12, 74, 3);
            if decision_durable {
                assert!(
                    gate.reserve(fresh).is_err(),
                    "Decision rejects every later admission at the closed height"
                );
            } else {
                gate.reserve(fresh)
                    .expect("the cut admits a current-view replacement");
            }
        }

        for retained_status in [
            LeaderWireLifecycleStatus::Ingress,
            LeaderWireLifecycleStatus::Runtime,
        ] {
            let directory = TempDir::new().expect("temporary active-owner directory");
            let context = context();
            let wal = directory
                .path()
                .join(format!("leader-wire-live-retains-{retained_status:?}.wal"));
            let roster = context
                .roster
                .iter()
                .map(|entry| entry.validator.clone())
                .collect::<BTreeSet<_>>();
            let max_chunks = 1;
            let capacity = LeaderWireLifecycleStoreGate::derived_capacity(roster.len(), max_chunks)
                .expect("derived gate capacity");
            let (gate, _) = LeaderWireLifecycleStoreGate::open(
                &wal,
                context.id(),
                context.height,
                OWNER_A,
                roster,
                capacity,
                max_chunks,
                leader_wire_recovery_authority(&context),
                &[],
                &[],
            )
            .expect("open active-owner gate");
            let token = leader_wire_token(&context, 2, 11, 73, 2);
            gate.admit_ingress(token.clone())
                .expect("publish active ingress owner");
            if retained_status == LeaderWireLifecycleStatus::Runtime {
                let runtime_owner =
                    LeaderWireRuntimeOwner::new(token.identity_hash(), 73).expect("runtime owner");
                gate.mark_runtime(&token, runtime_owner)
                    .expect("publish active runtime owner");
            }

            gate.advance_recovery_cut(
                leader_wire_recovery_authority_at(&context, OWNER_A, 3, false),
                &BTreeSet::new(),
            )
            .expect("advance while an active owner remains live");
            let restore = gate.restore().expect("inspect retained active owner");
            assert_eq!(restore.records().len(), 1, "{retained_status:?}");
            assert_eq!(
                restore.records()[0].status(),
                retained_status,
                "the live cut may reclaim only Dormant records"
            );
            assert_eq!(restore.records()[0].token(), &token);
            assert_eq!(restore.last_admission_ordinal(), 11);
            assert_eq!(restore.scheduler_ordinal_high_watermark(), 73);
            assert!(
                gate.identity_is_obsolete(&token.identity)
                    .expect("inspect advanced recovery authority"),
                "active retention must not roll the recovery authority back"
            );
        }

        {
            let directory = TempDir::new().expect("temporary recovery-cut rollback directory");
            let context = context();
            let wal = directory.path().join("leader-wire-live-cut-rollback.wal");
            let roster = context
                .roster
                .iter()
                .map(|entry| entry.validator.clone())
                .collect::<BTreeSet<_>>();
            let max_chunks = 1;
            let capacity = LeaderWireLifecycleStoreGate::derived_capacity(roster.len(), max_chunks)
                .expect("derived gate capacity");
            let initial_authority = leader_wire_recovery_authority(&context);
            let (gate, _) = LeaderWireLifecycleStoreGate::open(
                &wal,
                context.id(),
                context.height,
                OWNER_A,
                roster.clone(),
                capacity,
                max_chunks,
                initial_authority,
                &[],
                &[],
            )
            .expect("open rollback gate");
            let token = leader_wire_token(&context, 2, 11, 73, 2);
            gate.admit_ingress(token.clone())
                .expect("publish owner before restart");
            drop(gate);
            let (gate, restore) = LeaderWireLifecycleStoreGate::open(
                &wal,
                context.id(),
                context.height,
                OWNER_A,
                roster,
                capacity,
                max_chunks,
                initial_authority,
                &[],
                &[],
            )
            .expect("reopen rollback owner as dormant");
            assert_eq!(
                restore.records()[0].status(),
                LeaderWireLifecycleStatus::Dormant
            );
            std::fs::remove_file(&gate.path).expect("remove published snapshot");
            std::fs::create_dir(&gate.path).expect("block recovery-cut publication");
            assert!(
                gate.advance_recovery_cut(
                    leader_wire_recovery_authority_at(&context, OWNER_A, 3, false),
                    &BTreeSet::from([token.slot.clone()]),
                )
                .is_err(),
                "a failed atomic publication must reject the live cut"
            );
            let restored = gate.restore().expect("inspect recovery-cut rollback");
            assert_eq!(restored.records().len(), 1);
            assert_eq!(restored.records()[0].token(), &token);
            assert_eq!(
                restored.records()[0].status(),
                LeaderWireLifecycleStatus::Dormant
            );
            assert_eq!(restored.last_admission_ordinal(), 11);
            assert_eq!(restored.scheduler_ordinal_high_watermark(), 73);
            assert!(
                !gate
                    .identity_is_obsolete(&token.identity)
                    .expect("inspect rolled-back recovery authority"),
                "failed persistence must restore both the owner and recovery authority"
            );
        }
    }

    #[test]
    fn leader_wire_gate_rejects_foreign_recovery_authority() {
        let directory = TempDir::new().expect("temporary directory");
        let context = context();
        let wal = directory.path().join("leader-wire-foreign-authority.wal");
        let roster = context
            .roster
            .iter()
            .map(|entry| entry.validator.clone())
            .collect::<BTreeSet<_>>();
        let max_chunks = 2;
        let capacity = LeaderWireLifecycleStoreGate::derived_capacity(roster.len(), max_chunks)
            .expect("derived gate capacity");
        assert!(
            LeaderWireLifecycleStoreGate::open(
                &wal,
                context.id(),
                context.height,
                OWNER_A,
                roster,
                capacity,
                max_chunks,
                leader_wire_recovery_authority_at(&context, OWNER_B, 0, false),
                &[],
                &[],
            )
            .is_err(),
            "replay authority cannot cross the owner-bound snapshot geometry"
        );
    }

    include!("serviced_candidate_store/body_terminal_recovery_tests.rs");

    #[test]
    fn leader_wire_gate_rejects_duplicate_scheduler_and_low_high_watermarks() {
        for defect in ["duplicate-scheduler", "low-high-water"] {
            let directory = TempDir::new().expect("temporary directory");
            let context = context();
            let wal = directory.path().join(format!("leader-wire-{defect}.wal"));
            let roster = context
                .roster
                .iter()
                .map(|entry| entry.validator.clone())
                .collect::<BTreeSet<_>>();
            let max_chunks = 1;
            let capacity = LeaderWireLifecycleStoreGate::derived_capacity(roster.len(), max_chunks)
                .expect("derived gate capacity");
            let (gate, _) = LeaderWireLifecycleStoreGate::open(
                &wal,
                context.id(),
                context.height,
                OWNER_A,
                roster.clone(),
                capacity,
                max_chunks,
                leader_wire_recovery_authority(&context),
                &[],
                &[],
            )
            .expect("open empty gate");
            let first = leader_wire_token(&context, 2, 1, 7, 1);
            let mut second = leader_wire_token(&context, 2, 2, 9, 2);
            second.identity.phase = FairV2IngressLeaderWirePhase::CommitVote;
            second.slot.phase = FairV2IngressLeaderWirePhase::CommitVote;
            if defect == "duplicate-scheduler" {
                second.scheduler_ordinal = first.scheduler_ordinal;
            }
            let scheduler_ordinal_high_watermark = if defect == "low-high-water" {
                second.scheduler_ordinal - 1
            } else {
                second.scheduler_ordinal
            };
            let snapshot = PersistedLeaderWireLifecycles {
                format_version: LEADER_WIRE_FORMAT_VERSION,
                context_id: context.id(),
                height: context.height,
                owner: OWNER_A,
                capacity: u64::try_from(capacity).expect("capacity fits u64"),
                max_chunk_count: max_chunks,
                last_admission_ordinal: 2,
                scheduler_ordinal_high_watermark,
                records: vec![first, second]
                    .into_iter()
                    .map(|token| PersistedLeaderWireLifecycleRecord {
                        token,
                        status: LeaderWireLifecycleStatus::Dormant,
                        runtime_owner: None,
                        terminal_evidence: None,
                    })
                    .collect(),
            };
            let frame = encode_leader_wire_frame(&snapshot, gate.max_frame_bytes)
                .expect("encode corrupt-but-canonical fixture");
            fs::write(&gate.path, frame).expect("publish fixture");
            assert!(
                LeaderWireLifecycleStoreGate::open(
                    &wal,
                    context.id(),
                    context.height,
                    OWNER_A,
                    roster,
                    capacity,
                    max_chunks,
                    leader_wire_recovery_authority(&context),
                    &[],
                    &[],
                )
                .is_err(),
                "{defect} must fail closed"
            );
        }
    }

    #[test]
    fn leader_wire_gate_rolls_back_failed_atomic_status_publications() {
        fn replace_snapshot_with_directory(gate: &LeaderWireLifecycleStoreGate) {
            if let Err(error) = std::fs::remove_file(&gate.path)
                && error.kind() != std::io::ErrorKind::NotFound
            {
                panic!("remove prior gate snapshot: {error}");
            }
            std::fs::create_dir(&gate.path).expect("replace snapshot with directory");
        }

        {
            let directory = TempDir::new().expect("temporary directory");
            let context = context();
            let wal = directory.path().join("leader-wire-reserve.wal");
            let roster = context
                .roster
                .iter()
                .map(|entry| entry.validator.clone())
                .collect::<BTreeSet<_>>();
            let max_chunks = 1;
            let capacity = LeaderWireLifecycleStoreGate::derived_capacity(roster.len(), max_chunks)
                .expect("derived gate capacity");
            let (gate, _) = LeaderWireLifecycleStoreGate::open(
                &wal,
                context.id(),
                context.height,
                OWNER_A,
                roster,
                capacity,
                max_chunks,
                leader_wire_recovery_authority(&context),
                &[],
                &[],
            )
            .expect("open gate");
            std::fs::create_dir(&gate.path).expect("block first admission publication");
            assert!(
                gate.admit_ingress(leader_wire_token(&context, 2, 1, 5, 0))
                    .is_err()
            );
            let restored = gate.restore().expect("admission memory rollback");
            assert!(restored.records().is_empty());
            assert_eq!(restored.last_admission_ordinal(), 0);
            assert_eq!(restored.scheduler_ordinal_high_watermark(), 0);
        }

        for failed_cut in ["ingress", "runtime", "volatile-terminal", "terminal"] {
            let directory = TempDir::new().expect("temporary directory");
            let context = context();
            let wal = directory
                .path()
                .join(format!("leader-wire-{failed_cut}.wal"));
            let roster = context
                .roster
                .iter()
                .map(|entry| entry.validator.clone())
                .collect::<BTreeSet<_>>();
            let max_chunks = 1;
            let capacity = LeaderWireLifecycleStoreGate::derived_capacity(roster.len(), max_chunks)
                .expect("derived gate capacity");
            let (gate, _) = LeaderWireLifecycleStoreGate::open(
                &wal,
                context.id(),
                context.height,
                OWNER_A,
                roster,
                capacity,
                max_chunks,
                leader_wire_recovery_authority(&context),
                &[],
                &[],
            )
            .expect("open gate");
            let token = leader_wire_token(&context, 2, 13, 97, 3);
            if failed_cut != "ingress" {
                gate.admit_ingress(token.clone())
                    .expect("admit before later failure");
            }
            let runtime_owner =
                LeaderWireRuntimeOwner::new(token.identity_hash(), 97).expect("runtime owner");
            match failed_cut {
                "ingress" => {
                    replace_snapshot_with_directory(&gate);
                    assert!(gate.admit_ingress(token.clone()).is_err());
                    let restored = gate.restore().expect("memory rollback");
                    assert!(restored.records().is_empty());
                    assert_eq!(restored.last_admission_ordinal(), 0);
                    assert_eq!(restored.scheduler_ordinal_high_watermark(), 0);
                }
                "runtime" => {
                    replace_snapshot_with_directory(&gate);
                    assert!(gate.mark_runtime(&token, runtime_owner).is_err());
                    assert_eq!(
                        gate.restore().expect("memory rollback").records()[0].status(),
                        LeaderWireLifecycleStatus::Ingress
                    );
                }
                "terminal" => {
                    let receipt = gate
                        .mark_runtime(&token, runtime_owner)
                        .expect("mark runtime");
                    replace_snapshot_with_directory(&gate);
                    assert!(
                        gate.mark_terminal(
                            &receipt,
                            matching_terminal(&context, runtime_owner, &token),
                        )
                        .is_err()
                    );
                    assert_eq!(
                        gate.restore().expect("memory rollback").records()[0].status(),
                        LeaderWireLifecycleStatus::Runtime
                    );
                }
                "volatile-terminal" => {
                    let receipt = gate
                        .mark_runtime(&token, runtime_owner)
                        .expect("mark runtime");
                    replace_snapshot_with_directory(&gate);
                    assert!(gate.mark_volatile_terminal(&receipt).is_err());
                    assert_eq!(
                        gate.restore().expect("memory rollback").records()[0].status(),
                        LeaderWireLifecycleStatus::Runtime
                    );
                }
                _ => unreachable!(),
            }
        }

        {
            let directory = TempDir::new().expect("temporary directory");
            let context = context();
            let wal = directory.path().join("leader-wire-live-recovery-cut.wal");
            let roster = context
                .roster
                .iter()
                .map(|entry| entry.validator.clone())
                .collect::<BTreeSet<_>>();
            let max_chunks = 1;
            let capacity = LeaderWireLifecycleStoreGate::derived_capacity(roster.len(), max_chunks)
                .expect("derived gate capacity");
            let initial_authority = leader_wire_recovery_authority(&context);
            let (gate, _) = LeaderWireLifecycleStoreGate::open(
                &wal,
                context.id(),
                context.height,
                OWNER_A,
                roster.clone(),
                capacity,
                max_chunks,
                initial_authority,
                &[],
                &[],
            )
            .expect("open gate");
            let token = leader_wire_token(&context, 2, 13, 97, 3);
            gate.admit_ingress(token.clone())
                .expect("persist owner before recovery-cut rollback");
            drop(gate);
            let (gate, restore) = LeaderWireLifecycleStoreGate::open(
                &wal,
                context.id(),
                context.height,
                OWNER_A,
                roster,
                capacity,
                max_chunks,
                initial_authority,
                &[],
                &[],
            )
            .expect("reopen owner as dormant");
            assert_eq!(
                restore.records()[0].status(),
                LeaderWireLifecycleStatus::Dormant
            );
            replace_snapshot_with_directory(&gate);
            assert!(
                gate.advance_recovery_cut(
                    leader_wire_recovery_authority_at(&context, OWNER_A, 3, false),
                    &BTreeSet::from([token.slot.clone()]),
                )
                .is_err()
            );
            let restored = gate.restore().expect("recovery-cut memory rollback");
            assert_eq!(restored.records().len(), 1);
            assert_eq!(restored.records()[0].token(), &token);
            assert_eq!(
                restored.records()[0].status(),
                LeaderWireLifecycleStatus::Dormant
            );
            assert!(
                !gate
                    .identity_is_obsolete(&token.identity)
                    .expect("recovery authority rollback"),
                "failed persistence must roll the process-local cut back too"
            );
        }
    }

    #[test]
    fn snapshot_rejects_corruption_stale_context_and_capacity_exhaustion() {
        let directory = TempDir::new().expect("temporary directory");
        let context = context();
        let wal = directory.path().join("height.wal");
        let (store, _) = ServicedCandidateStore::open_with_capacities(
            &wal,
            context.id(),
            context.height,
            OWNER_A,
            1,
            SERVICED_CANDIDATE_STAGES_PER_LIFECYCLE,
        )
        .expect("open snapshot");
        let records = BTreeMap::from([(key(&context, 0, 1), 0)]);
        store.persist(&records, false).expect("persist record");
        assert!(
            ServicedCandidateStore::open_with_capacities(
                &wal,
                context.id(),
                context.height + 1,
                OWNER_A,
                1,
                SERVICED_CANDIDATE_STAGES_PER_LIFECYCLE,
            )
            .is_err(),
            "stale height is rejected"
        );
        assert!(
            ServicedCandidateStore::open_with_capacities(
                &wal,
                context.id(),
                context.height,
                OWNER_B,
                1,
                SERVICED_CANDIDATE_STAGES_PER_LIFECYCLE,
            )
            .is_err(),
            "a snapshot cannot be transplanted between local validator owners"
        );
        assert!(
            store
                .persist(
                    &BTreeMap::from([(key(&context, 0, 1), 0), (key(&context, 0, 2), 0)]),
                    false,
                )
                .is_err(),
            "capacity exhaustion fails closed instead of evicting A"
        );
        let mut bytes = fs::read(store.path_for_test()).expect("read snapshot");
        let last = bytes.last_mut().expect("nonempty snapshot");
        *last ^= 1;
        fs::write(store.path_for_test(), bytes).expect("corrupt snapshot");
        assert!(
            ServicedCandidateStore::open_with_capacities(
                &wal,
                context.id(),
                context.height,
                OWNER_A,
                1,
                SERVICED_CANDIDATE_STAGES_PER_LIFECYCLE,
            )
            .is_err(),
            "checksum corruption is rejected"
        );
    }

    #[test]
    fn decision_reclamation_is_canonical_only_for_an_empty_snapshot() {
        let directory = TempDir::new().expect("temporary directory");
        let context = context();
        let wal = directory.path().join("decision.wal");
        let (store, _) =
            ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 2)
                .expect("open snapshot");
        let record = key(&context, 0, 1);
        assert!(
            store.persist(&BTreeMap::from([(record, 0)]), true).is_err(),
            "Decision reclamation cannot coexist with an unreclaimed owner"
        );
        store
            .persist(&BTreeMap::new(), true)
            .expect("publish canonical reclaimed state");
        let (_, restored) =
            ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 2)
                .expect("restore canonical reclaimed state");
        assert!(restored.records.is_empty());
        assert!(restored.decision_reclaimed);

        let forged = state(
            &store,
            vec![PersistedServicedCandidate {
                key: record,
                service_view: 0,
            }],
            true,
        );
        write_frame(&store, &forged);
        assert!(
            ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 2).is_err(),
            "a checksummed nonempty Decision-reclaimed mutation fails closed"
        );

        let orphan =
            continuation_record(&context, 1, 1, 1, ProducerContinuationStatus::Terminal, &[]);
        let mut forged_orphan = state(&store, Vec::new(), true);
        forged_orphan.producer_continuations = vec![PersistedProducerContinuation {
            address: orphan.identity.address(),
            record: orphan,
        }];
        write_frame(&store, &forged_orphan);
        assert!(
            ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 2).is_err(),
            "a Decision-reclaimed snapshot cannot restore an orphan producer high-watermark"
        );
    }

    #[test]
    fn snapshot_rejects_truncation_version_ordering_duplicates_and_oversize() {
        let directory = TempDir::new().expect("temporary directory");
        let context = context();

        let wal = directory.path().join("truncated.wal");
        let (store, _) =
            ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 2)
                .expect("open truncated fixture");
        let valid = state(
            &store,
            vec![PersistedServicedCandidate {
                key: key(&context, 0, 1),
                service_view: 0,
            }],
            false,
        );
        let mut frame = encode_frame_v4(&valid, store.max_frame_bytes).expect("encode valid frame");
        frame.pop();
        fs::write(store.path_for_test(), frame).expect("write truncated frame");
        assert!(
            ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 2).is_err()
        );

        let wal = directory.path().join("version.wal");
        let (store, _) =
            ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 2)
                .expect("open version fixture");
        let valid = state(&store, Vec::new(), false);
        let mut frame = encode_frame_v4(&valid, store.max_frame_bytes).expect("encode valid frame");
        frame[FRAME_MAGIC.len()..FRAME_MAGIC.len() + 2]
            .copy_from_slice(&(FORMAT_VERSION_V3 - 1).to_le_bytes());
        fs::write(store.path_for_test(), frame).expect("write old-version frame");
        assert!(
            ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 2).is_err(),
            "the retired v2 schema fails closed instead of being guessed"
        );

        for (name, records) in [
            (
                "unordered",
                vec![
                    PersistedServicedCandidate {
                        key: key(&context, 0, 2),
                        service_view: 0,
                    },
                    PersistedServicedCandidate {
                        key: key(&context, 0, 1),
                        service_view: 0,
                    },
                ],
            ),
            (
                "duplicate",
                vec![
                    PersistedServicedCandidate {
                        key: key(&context, 0, 1),
                        service_view: 0,
                    },
                    PersistedServicedCandidate {
                        key: key(&context, 0, 1),
                        service_view: 1,
                    },
                ],
            ),
        ] {
            let wal = directory.path().join(format!("{name}.wal"));
            let (store, _) =
                ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 2)
                    .expect("open ordering fixture");
            write_frame(&store, &state(&store, records, false));
            assert!(
                ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 2)
                    .is_err(),
                "{name} records must be rejected"
            );
        }

        let wal = directory.path().join("oversize.wal");
        let (store, _) =
            ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 1)
                .expect("open oversize fixture");
        let oversized_len =
            usize::try_from(store.max_frame_bytes + 1).expect("small fixture bound fits usize");
        fs::write(store.path_for_test(), vec![0; oversized_len]).expect("write oversized frame");
        assert!(
            ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 1).is_err()
        );
    }

    #[test]
    fn v4_rejects_noncanonical_or_over_capacity_producer_tables() {
        let directory = TempDir::new().expect("temporary directory");
        let context = context();

        let first =
            continuation_record(&context, 1, 1, 1, ProducerContinuationStatus::Terminal, &[]);
        let second =
            continuation_record(&context, 2, 2, 2, ProducerContinuationStatus::Terminal, &[]);
        let first_persisted = PersistedProducerContinuation {
            address: first.identity.address(),
            record: first.clone(),
        };
        let second_persisted = PersistedProducerContinuation {
            address: second.identity.address(),
            record: second,
        };

        for (name, continuations) in [
            (
                "producer-unordered",
                vec![second_persisted.clone(), first_persisted.clone()],
            ),
            (
                "producer-duplicate-address",
                vec![first_persisted.clone(), first_persisted.clone()],
            ),
        ] {
            let wal = directory.path().join(format!("{name}.wal"));
            let (store, _) =
                ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 2)
                    .expect("open producer-ordering fixture");
            let mut invalid = state(&store, Vec::new(), false);
            invalid.producer_continuations = continuations;
            write_frame(&store, &invalid);
            assert!(
                ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 2)
                    .is_err(),
                "{name} must fail closed"
            );
        }

        let wal = directory.path().join("active-hash-only.wal");
        let (store, _) =
            ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 2)
                .expect("open active-record fixture");
        let mut malformed =
            continuation_record(&context, 1, 1, 1, ProducerContinuationStatus::Reserved, &[]);
        malformed.status = ProducerContinuationStatus::Materialized;
        let mut invalid = state(&store, Vec::new(), false);
        invalid.producer_continuations = vec![PersistedProducerContinuation {
            address: malformed.identity.address(),
            record: malformed,
        }];
        write_frame(&store, &invalid);
        assert!(
            ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 2).is_err(),
            "startup must reject Materialized metadata without an exact successor"
        );

        let wal = directory.path().join("producer-service-mismatch.wal");
        let (store, _) =
            ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 2)
                .expect("open producer/service binding fixture");
        let terminal =
            continuation_record(&context, 1, 1, 1, ProducerContinuationStatus::Terminal, &[]);
        let mut invalid = state(
            &store,
            vec![PersistedServicedCandidate {
                key: key(&context, 2, 0x71),
                service_view: 2,
            }],
            false,
        );
        invalid.producer_continuations = vec![PersistedProducerContinuation {
            address: terminal.identity.address(),
            record: terminal,
        }];
        write_frame(&store, &invalid);
        assert!(
            ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 2).is_err(),
            "a terminal producer cannot bind a different serviced identity"
        );

        let wal = directory.path().join("producer-capacity.wal");
        let (store, _) =
            ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 1)
                .expect("open producer-capacity fixture");
        let mut over_capacity = (0_u8..u8::try_from(SERVICED_CANDIDATE_STAGES_PER_LIFECYCLE)
            .expect("stage count fits u8"))
            .map(|stage| {
                terminal_continuation_at_view(
                    &context,
                    1,
                    u128::from(stage) + 1,
                    stage,
                    2,
                    stage + 1,
                )
            })
            .collect::<Vec<_>>();
        over_capacity.push(terminal_continuation_at_view(&context, 2, 12, 0, 2, 0x40));
        let mut serviced = over_capacity
            .iter()
            .map(|record| PersistedServicedCandidate {
                key: record.identity().candidate(),
                service_view: 2,
            })
            .collect::<Vec<_>>();
        serviced.sort_unstable_by_key(|record| record.key);
        let mut invalid = state(&store, serviced, false);
        invalid.producer_continuations = over_capacity
            .into_iter()
            .map(|record| PersistedProducerContinuation {
                address: record.identity().address(),
                record,
            })
            .collect();
        write_frame(&store, &invalid);
        assert!(
            ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 1).is_err(),
            "producer-continuation capacity is checked independently"
        );

        let wal = directory.path().join("version-layout-confusion.wal");
        let (store, _) =
            ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 1)
                .expect("open version-layout fixture");
        let valid = state(&store, Vec::new(), false);
        let mut frame = encode_frame_v4(&valid, store.max_frame_bytes).expect("encode v4 frame");
        frame[FRAME_MAGIC.len()..FRAME_MAGIC.len() + 2]
            .copy_from_slice(&FORMAT_VERSION_V3.to_le_bytes());
        fs::write(store.path_for_test(), frame).expect("write mismatched version frame");
        assert!(
            ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 1).is_err(),
            "a v4 payload is never reinterpreted through the v3 decoder"
        );
    }

    #[test]
    fn producer_identity_stage_projection_rejects_foreign_root_and_successor_stages() {
        let context = context();
        for stage in
            0..u8::try_from(SERVICED_CANDIDATE_STAGES_PER_LIFECYCLE).expect("stage count fits u8")
        {
            let identity = continuation_identity(&context, 1, 1, stage, stage + 1);
            assert_eq!(identity.stage(), stage);
            assert_eq!(identity.address().stage(), stage);
            let record = ProducerContinuationRecord::new(
                identity,
                ProducerContinuationStatus::Terminal,
                Vec::new(),
            )
            .expect("tracked stage has one physical replay class");
            let expected_source_class = match stage {
                1..=5 => ProducerContinuationSourceClass::ConditionalTransport,
                7 => ProducerContinuationSourceClass::VolatileBody,
                _ => ProducerContinuationSourceClass::Local,
            };
            assert_eq!(record.source_class(), expected_source_class);
        }
        for untracked_kind in [7, 11, 12, 13, 15, u8::MAX] {
            assert!(
                ProducerContinuationIdentity::new(
                    key_with_kind(&context, 2, untracked_kind, untracked_kind),
                    Hash::new([0xE3, untracked_kind]),
                    1,
                    1,
                )
                .is_err(),
                "untracked event kind {untracked_kind} cannot claim a service stage"
            );
        }

        let directory = TempDir::new().expect("temporary directory");
        let wal = directory.path().join("foreign-root-stage.wal");
        let (store, _) =
            ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 1)
                .expect("open foreign-stage fixture");
        let mut root = continuation_record(
            &context,
            1,
            1,
            1,
            ProducerContinuationStatus::Reserved,
            &[2],
        );
        root.identity.stage = 2;
        let mut invalid = state(&store, Vec::new(), false);
        invalid.producer_continuations = vec![PersistedProducerContinuation {
            address: root.identity.address(),
            record: root,
        }];
        write_frame(&store, &invalid);
        assert!(
            ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 1).is_err(),
            "a decoded root cannot occupy a foreign service stage"
        );

        let wal = directory.path().join("foreign-successor-stage.wal");
        let (store, _) =
            ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 1)
                .expect("open foreign-successor fixture");
        let mut record = continuation_record(
            &context,
            1,
            1,
            1,
            ProducerContinuationStatus::Reserved,
            &[2],
        );
        record.handoff_candidates[0].stage = 3;
        let mut invalid = state(&store, Vec::new(), false);
        invalid.producer_continuations = vec![PersistedProducerContinuation {
            address: record.identity.address(),
            record,
        }];
        write_frame(&store, &invalid);
        assert!(
            ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 1).is_err(),
            "a decoded successor receives the same exact stage validation"
        );

        let wal = directory.path().join("foreign-source-class.wal");
        let (store, _) =
            ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 1)
                .expect("open foreign-source-class fixture");
        let mut record = terminal_continuation_at_view(&context, 1, 1, 1, 2, 0x61);
        assert_eq!(
            record.source_class(),
            ProducerContinuationSourceClass::ConditionalTransport
        );
        record.source_class = ProducerContinuationSourceClass::Local;
        let candidate = record.identity().candidate();
        let mut invalid = state(
            &store,
            vec![PersistedServicedCandidate {
                key: candidate,
                service_view: candidate.source_view(),
            }],
            false,
        );
        invalid.producer_continuations = vec![PersistedProducerContinuation {
            address: record.identity().address(),
            record,
        }];
        write_frame(&store, &invalid);
        assert!(
            ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 1).is_err(),
            "a decoded record cannot strengthen its physical replay source"
        );
    }

    #[test]
    fn bounded_slot_reuse_requires_terminal_strict_view_and_ordinal_advance() {
        let directory = TempDir::new().expect("temporary directory");
        let context = context();
        let wal = directory.path().join("bounded-slot-reuse.wal");
        let (store, _) =
            ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 1)
                .expect("open bounded-slot fixture");
        let first = terminal_continuation_at_view(&context, 1, 1, 2, 1, 1);
        let mut continuations = BTreeMap::new();
        assert_eq!(
            store
                .reserve_producer_continuation(&mut continuations, first.clone())
                .expect("reserve first bounded address"),
            ProducerContinuationReservation::Inserted
        );
        assert_eq!(
            store
                .reserve_producer_continuation(&mut continuations, first.clone())
                .expect("coalesce exact retry"),
            ProducerContinuationReservation::Coalesced
        );

        let same_view = terminal_continuation_at_view(&context, 1, 2, 2, 1, 2);
        assert!(
            store
                .reserve_producer_continuation(&mut continuations, same_view)
                .is_err(),
            "ordinal advance alone cannot reuse a terminal address"
        );
        let same_ordinal = terminal_continuation_at_view(&context, 1, 1, 2, 2, 3);
        assert!(
            store
                .reserve_producer_continuation(&mut continuations, same_ordinal)
                .is_err(),
            "view advance alone cannot reuse a terminal address"
        );

        for episode in 2_u8..=64 {
            let replacement = terminal_continuation_at_view(
                &context,
                1,
                u128::from(episode),
                2,
                u64::from(episode),
                episode,
            );
            assert_eq!(
                store
                    .reserve_producer_continuation(&mut continuations, replacement)
                    .expect("strictly advance terminal address"),
                ProducerContinuationReservation::ReplacedTerminal
            );
            assert_eq!(
                continuations.len(),
                1,
                "sequential lifecycles reuse one bounded address"
            );
        }
        assert!(
            store
                .reserve_producer_continuation(&mut continuations, first)
                .is_err(),
            "a stale ABA writer cannot replace the newer terminal owner"
        );

        let out_of_geometry = terminal_continuation_at_view(&context, 2, 65, 2, 65, 65);
        assert!(
            store
                .reserve_producer_continuation(&mut continuations, out_of_geometry)
                .is_err(),
            "the allocator slot must remain inside the frozen lifecycle capacity"
        );

        let mut active = terminal_continuation_at_view(&context, 1, 64, 2, 64, 64);
        active.status = ProducerContinuationStatus::Reserved;
        continuations.insert(active.identity.address(), active.clone());
        let later = terminal_continuation_at_view(&context, 1, 65, 2, 65, 65);
        assert!(
            store
                .reserve_producer_continuation(&mut continuations, later)
                .is_err(),
            "a live bounded address is never evicted"
        );

        store
            .persist_with_producer_continuations(&BTreeMap::new(), &continuations, false)
            .expect("persist a live bounded address as restart admission metadata");
        let (_, active_restored) =
            ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 1)
                .expect("restore live bounded address");
        assert_eq!(
            active_restored.producer_continuations[&active.identity.address()].status(),
            ProducerContinuationStatus::Reserved
        );
        assert_eq!(
            active_restored.producer_continuations[&active.identity.address()]
                .identity()
                .admission_ordinal(),
            active.identity().admission_ordinal()
        );
        continuations
            .values_mut()
            .for_each(|record| record.status = ProducerContinuationStatus::Terminal);
        let serviced = continuations
            .values()
            .map(|record| {
                let candidate = record.identity().candidate();
                (candidate, candidate.source_view())
            })
            .collect::<BTreeMap<_, _>>();
        store
            .persist_with_producer_continuations(&serviced, &continuations, false)
            .expect("persist bounded terminal table");
        let (_, restored) =
            ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 1)
                .expect("restore bounded terminal table");
        assert_eq!(restored.producer_continuations, continuations);
    }

    #[test]
    fn one_logical_candidate_cannot_resurrect_at_another_bounded_address() {
        let directory = TempDir::new().expect("temporary directory");
        let context = context();
        let wal = directory.path().join("logical-resurrection.wal");
        let (store, _) =
            ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 2)
                .expect("open two-slot fixture");
        let first = terminal_continuation_at_view(&context, 1, 1, 2, 1, 1);
        let mut continuations = BTreeMap::new();
        store
            .reserve_producer_continuation(&mut continuations, first.clone())
            .expect("reserve original logical candidate");

        let mut resurrected = first;
        resurrected.identity.lifecycle_slot = 2;
        resurrected.identity.admission_ordinal = 2;
        resurrected.identity.causal_lifecycle_key = Hash::new(b"forged second lifecycle");
        assert!(
            store
                .reserve_producer_continuation(&mut continuations, resurrected)
                .is_err(),
            "the same drained logical candidate cannot acquire a second address"
        );
        assert_eq!(continuations.len(), 1);
    }

    #[test]
    fn snapshot_rejects_nonregular_artifacts() {
        let directory = TempDir::new().expect("temporary directory");
        let context = context();
        let wal = directory.path().join("directory.wal");
        let (store, _) =
            ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 1)
                .expect("derive snapshot path");
        fs::create_dir(store.path_for_test()).expect("place directory at snapshot path");
        assert!(
            ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 1).is_err()
        );
    }

    #[cfg(unix)]
    #[test]
    fn snapshot_load_and_retire_never_follow_substituted_symlinks() {
        use std::os::unix::fs::symlink;

        let directory = TempDir::new().expect("temporary directory");
        let context = context();
        let wal = directory.path().join("symlink.wal");
        let (store, _) =
            ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 1)
                .expect("open snapshot");
        store
            .persist(&BTreeMap::from([(key(&context, 0, 1), 0)]), false)
            .expect("persist target frame");
        let snapshot = store.path_for_test().to_path_buf();
        let hard_link = directory.path().join("hard-linked.snapshot");
        fs::hard_link(&snapshot, &hard_link).expect("create second link to snapshot");
        assert!(
            ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 1).is_err(),
            "load must reject a multiply linked snapshot"
        );
        fs::remove_file(hard_link).expect("restore single-link fixture");

        let target = directory.path().join("target.snapshot");
        fs::rename(&snapshot, &target).expect("move direct frame to symlink target");
        let target_before = fs::read(&target).expect("read target before substitution");
        symlink(&target, &snapshot).expect("substitute snapshot symlink");

        assert!(
            ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 1).is_err(),
            "load must reject a direct-path symlink"
        );
        assert!(
            store.retire().is_err(),
            "retirement must reject rather than follow a substituted symlink"
        );
        assert_eq!(
            fs::read(&target).expect("read target after rejected retirement"),
            target_before,
            "the symlink target remains untouched"
        );
        assert!(snapshot.is_symlink());
    }

    #[test]
    fn finalized_snapshot_retirement_leaves_successor_rollover_empty() {
        let directory = TempDir::new().expect("temporary directory");
        let context = context();
        let successor = successor_context(&context);
        let wal = directory.path().join("00000000000000000007.wal");
        let (store, _) =
            ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 2)
                .expect("open finalized-height snapshot");
        let terminal =
            continuation_record(&context, 1, 1, 1, ProducerContinuationStatus::Terminal, &[]);
        let producer_continuations = BTreeMap::from([(terminal.identity.address(), terminal)]);
        let producer_candidate = producer_continuations
            .values()
            .next()
            .expect("terminal producer exists")
            .identity()
            .candidate();
        store
            .persist_with_producer_continuations(
                &BTreeMap::from([(producer_candidate, producer_candidate.source_view())]),
                &producer_continuations,
                false,
            )
            .expect("persist finalized-height owner");
        assert!(
            store
                .persist_with_producer_continuations(
                    &BTreeMap::new(),
                    &producer_continuations,
                    true,
                )
                .is_err(),
            "Decision reclamation rejects an orphan producer table"
        );
        store
            .persist_with_producer_continuations(&BTreeMap::new(), &BTreeMap::new(), true)
            .expect("atomically reclaim finalized-height service and producer owners");
        assert!(
            ServicedCandidateStore::open(&wal, successor.id(), successor.height, OWNER_A, 2,)
                .is_err(),
            "a predecessor snapshot cannot be transplanted into the successor context"
        );
        let snapshot_path = store.path_for_test().to_path_buf();
        store.retire().expect("retire finalized-height snapshot");
        assert!(!snapshot_path.exists());

        let successor_wal = directory.path().join("00000000000000000008.wal");
        let (_successor, restored) = ServicedCandidateStore::open(
            &successor_wal,
            successor.id(),
            successor.height,
            OWNER_A,
            2,
        )
        .expect("open independent successor path");
        assert!(restored.records.is_empty());
        assert!(restored.producer_continuations.is_empty());
        assert!(!restored.decision_reclaimed);
    }
}
