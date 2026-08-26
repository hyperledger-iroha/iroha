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
use super::{
    FairV2IngressLeaderWireIdentity, FairV2IngressLeaderWirePhase, FairV2IngressLeaderWireSlot,
    FairV2IngressLeaderWireSourceClass, FairV2IngressLeaderWireToken,
    safety_wal::{SafetyWalLeaderWireStoreAuthority, SafetyWalServicedCandidateStoreAuthority},
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
use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{block::consensus_v2 as wire, peer::PeerId};
use norito::codec::{Decode, DecodeAll, Encode};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, Mutex},
};
#[cfg(test)]
use std::{
    fs,
    path::{Path, PathBuf},
};
// Version 4 records restart-safe terminal retirements and a separate, equally
// bounded producer-continuation lifecycle table. Active records retain
// identity/slot/ordinal metadata but never claim to persist the command
// payload: restart normalizes them to selector-inert Dormant and admits exact
// replay under the same immutable identity. Every other snapshot version fails
// closed; the first release has no persistence migration path.
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
    #[cfg(test)]
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
/// capability lets it retire view-scoped control records made obsolete by a
/// certified view advance or durable Decision without treating its own
/// terminal projection as authority. The exact durable-lock Commit statement
/// and any CommitQC remain reducer progress across view changes. Manifest
/// chunks and historical certified-body responses are transport completions;
/// the reducer can make Decision durable before obtaining the exact body, so
/// neither cut can obsolete them. The capability is process-local and never
/// enters either snapshot format.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct LeaderWireRecoveryAuthority {
    context_id: wire::HeightContextId,
    height: wire::Height,
    owner: [u8; 32],
    durable_view: wire::View,
    decision_durable: bool,
    /// Exact durable lock whose historical Commit votes remain reducer inputs.
    ///
    /// This projection is process-local safety-WAL authority. It is deliberately
    /// absent from the leader-wire snapshot: replay reconstructs it from the
    /// authenticated PrepareQC before opening this adjacent store.
    protected_lock: Option<(wire::ConsensusRound, wire::BlockSubject)>,
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
            protected_lock: None,
        }
    }
    fn protected_lock_is_well_formed(
        self,
        protected_lock: (wire::ConsensusRound, wire::BlockSubject),
    ) -> bool {
        let (round, _) = protected_lock;
        round.context_id == self.context_id
            && round.height == self.height
            && round.view <= self.durable_view
    }
    fn protected_lock_monotonically_extends(self, previous: Self) -> bool {
        match (previous.protected_lock, self.protected_lock) {
            (None, _) => true,
            (Some(_), None) => false,
            (Some(previous), Some(next)) => next == previous || next.0.view > previous.0.view,
        }
    }
    /// Attach the exact replayed durable lock before opening the adjacent store.
    pub(super) fn with_protected_lock(
        self,
        protected_lock: Option<(wire::ConsensusRound, wire::BlockSubject)>,
    ) -> Result<Self, String> {
        let next = Self {
            protected_lock,
            ..self
        };
        if protected_lock.is_some_and(|lock| !next.protected_lock_is_well_formed(lock)) {
            return Err(
                "leader-wire recovery authority carried a future protected lock".to_owned(),
            );
        }
        if !next.protected_lock_monotonically_extends(self) {
            return Err("leader-wire recovery authority regressed its protected lock".to_owned());
        }
        Ok(next)
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
    pub(super) fn advance_view(
        self,
        durable_view: wire::View,
        protected_lock: Option<(wire::ConsensusRound, wire::BlockSubject)>,
    ) -> Result<Self, String> {
        if durable_view < self.durable_view {
            return Err("leader-wire recovery authority regressed its durable view".to_owned());
        }
        let next = Self {
            durable_view,
            protected_lock,
            ..self
        };
        if protected_lock.is_some_and(|lock| !next.protected_lock_is_well_formed(lock)) {
            return Err(
                "leader-wire recovery authority carried a future protected lock".to_owned(),
            );
        }
        if !next.protected_lock_monotonically_extends(self) {
            return Err("leader-wire recovery authority regressed its protected lock".to_owned());
        }
        Ok(next)
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
            && self
                .protected_lock
                .is_none_or(|lock| self.protected_lock_is_well_formed(lock))
            && self.protected_lock_monotonically_extends(previous)
    }
    fn protects_commit_vote(self, identity: &FairV2IngressLeaderWireIdentity) -> bool {
        identity.phase == FairV2IngressLeaderWirePhase::CommitVote
            && self.protected_lock.is_some_and(|(round, subject)| {
                identity.context_id == round.context_id
                    && identity.height == round.height
                    && identity.view == round.view
                    && identity.subject_hash == Hash::new(subject.encode())
            })
    }
    /// Return whether this durable cut retires one stored lifecycle owner.
    pub(super) fn retires(self, token: &FairV2IngressLeaderWireToken) -> bool {
        self.retires_stored_identity(&token.identity)
    }
    fn retires_stored_identity(self, identity: &FairV2IngressLeaderWireIdentity) -> bool {
        if identity.phase.source_class() != FairV2IngressLeaderWireSourceClass::Control {
            return false;
        }
        self.decision_durable
            || (identity.view < self.durable_view
                && identity.phase != FairV2IngressLeaderWirePhase::CommitQc
                && !self.protects_commit_vote(identity))
    }
    /// Return whether this durable cut still admits new ingress of this identity.
    fn admits_ingress_identity(self, identity: &FairV2IngressLeaderWireIdentity) -> bool {
        // A certified view closes ordinary old-view control, but the reducer
        // still accepts the exact locked Commit statement and terminal
        // CommitQC. Decision closes every control class, not transport
        // completion. The selected block can still be missing when Decision
        // becomes durable, so its exact chunk/body response must reach the
        // downstream fetch, manifest, request, and subject checks.
        if identity.phase.source_class() != FairV2IngressLeaderWireSourceClass::Control {
            return true;
        }
        if self.decision_durable {
            return false;
        }
        identity.phase == FairV2IngressLeaderWirePhase::CommitQc
            || self.protects_commit_vote(identity)
            || identity.view >= self.durable_view
            || (identity.phase == FairV2IngressLeaderWirePhase::TimeoutCertificate
                && identity.view.saturating_add(1) == self.durable_view)
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
        incoming_phase_is_timeout_certificate: token.identity.phase
            == FairV2IngressLeaderWirePhase::TimeoutCertificate,
        incumbent_phase_is_timeout_certificate: incumbent.is_some_and(|record| {
            record.token.identity.phase == FairV2IngressLeaderWirePhase::TimeoutCertificate
        }),
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
    storage: SafetyWalLeaderWireStoreAuthority,
    #[cfg(test)]
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
/// Move-only proof that every physical ingress carrier supplied by one sealed
/// fair queue was durably returned to selector-dormant ownership.
///
/// Exact gate and carrier equality is release-checked before this opaque proof
/// is minted. It authorizes volatile release only after synchronous sidecar
/// publication succeeds and is deliberately neither `Clone` nor `Copy`.
#[must_use = "closed ingress retirement must authorize the volatile queue release"]
pub(super) struct SealedLeaderWireIngressRetirementV1 {
    _private: (),
}
impl SealedLeaderWireIngressRetirementV1 {
    /// Consume the proof after the volatile queue and binding are gone.
    pub(super) fn complete(self) {}
    /// Drop the process-local proof at an injected process boundary.
    #[cfg(test)]
    pub(super) fn abandon_at_crash_cut(self) {}
}
/// Atomic per-height snapshot stored beside the safety WAL.
#[derive(Debug)]
pub(crate) struct ServicedCandidateStore {
    storage: SafetyWalServicedCandidateStoreAuthority,
    #[cfg(test)]
    path: PathBuf,
    context_id: wire::HeightContextId,
    height: wire::Height,
    owner: [u8; 32],
    serviced_capacity: usize,
    producer_continuation_capacity: usize,
    producer_continuation_lifecycle_capacity: u64,
    max_frame_bytes: u64,
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
    /// Open a context-bound leader-wire snapshot through the safety WAL's
    /// one-shot adjacent-store authority.
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
    pub(crate) fn open_with_safety_wal_authority(
        storage: SafetyWalLeaderWireStoreAuthority,
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
        Self::open_with_storage(
            storage,
            context_id,
            height,
            owner,
            roster,
            capacity,
            max_chunk_count,
            recovery_authority,
            producer_terminals,
            durable_bodies,
        )
    }
    /// Test-only raw-path adapter for the sealed production constructor.
    #[cfg(test)]
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
        let storage = SafetyWalLeaderWireStoreAuthority::for_test_path(safety_wal_path)?;
        Self::open_with_storage(
            storage,
            context_id,
            height,
            owner,
            roster,
            capacity,
            max_chunk_count,
            recovery_authority,
            producer_terminals,
            durable_bodies,
        )
    }
    #[allow(clippy::too_many_arguments)]
    fn open_with_storage(
        storage: SafetyWalLeaderWireStoreAuthority,
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
        let record_bytes = u64::try_from(capacity)
            .map_err(|_| "leader-wire lifecycle capacity is not representable".to_owned())?
            .checked_mul(LEADER_WIRE_RECORD_HEADROOM_BYTES)
            .ok_or_else(|| "leader-wire lifecycle frame bound overflowed".to_owned())?;
        let max_frame_bytes = FIXED_FRAME_HEADROOM_BYTES
            .checked_add(record_bytes)
            .ok_or_else(|| "leader-wire lifecycle frame bound overflowed".to_owned())?;
        #[cfg(test)]
        let path = storage.path_for_test().to_path_buf();
        let gate = Arc::new(Self {
            storage,
            #[cfg(test)]
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
    /// Test-only closed projection of an exact durable Ingress owner.
    #[cfg(test)]
    pub(crate) fn exact_record_is_ingress_for_test(
        &self,
        token: &FairV2IngressLeaderWireToken,
    ) -> bool {
        let Ok(state) = self.state.lock() else {
            return false;
        };
        state.records.get(&token.slot).is_some_and(|record| {
            record.token == *token
                && record.status == LeaderWireLifecycleStatus::Ingress
                && record.runtime_owner.is_none()
                && record.terminal_evidence.is_none()
        })
    }
    /// Test-only closed projection of the exact body-backed terminal handoff.
    #[cfg(test)]
    pub(crate) fn exact_record_is_durable_body_terminal_for_test(
        &self,
        token: &FairV2IngressLeaderWireToken,
    ) -> bool {
        let Ok(state) = self.state.lock() else {
            return false;
        };
        state.records.get(&token.slot).is_some_and(|record| {
            let (
                Some(runtime_owner),
                Some(terminal @ LeaderWireStableTerminalEvidence::DurableBody(_)),
            ) = (record.runtime_owner, record.terminal_evidence.as_ref())
            else {
                return false;
            };
            record.token == *token
                && record.status == LeaderWireLifecycleStatus::Terminal
                && leader_wire_stable_terminal_matches_runtime(
                    terminal,
                    token,
                    runtime_owner,
                    self.context_id,
                    self.height,
                    self.owner,
                )
        })
    }
    /// Test-only closed projection of one same-process ordinary retirement.
    #[cfg(test)]
    pub(crate) fn exact_record_is_volatile_terminal_for_test(
        &self,
        token: &FairV2IngressLeaderWireToken,
    ) -> bool {
        let Ok(state) = self.state.lock() else {
            return false;
        };
        state.records.get(&token.slot).is_some_and(|record| {
            record.token == *token
                && record.status == LeaderWireLifecycleStatus::VolatileTerminal
                && record.runtime_owner.is_some_and(|owner| {
                    owner.causal_lifecycle_key() == token.identity_hash()
                        && owner.admission_ordinal() == token.scheduler_ordinal()
                })
                && record.terminal_evidence.is_none()
        })
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
        Ok(!state.recovery_authority.admits_ingress_identity(identity))
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
                (record.status == LeaderWireLifecycleStatus::Dormant && next.retires(&record.token))
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
    /// Return every carrier still owned by a sealed fair queue to Dormant.
    ///
    /// The caller holds the fair-ingress service and state locks and supplies
    /// its complete productive-carrier projection. Exact equality with the
    /// durable Ingress set prevents a partial drain from orphaning either a
    /// physical carrier or a sidecar owner. The returned move-only receipt is
    /// minted only after the atomic replacement and directory sync complete.
    pub(super) fn park_sealed_ingress(
        self: &Arc<Self>,
        carriers: BTreeMap<FairV2IngressLeaderWireSlot, FairV2IngressLeaderWireToken>,
    ) -> Result<SealedLeaderWireIngressRetirementV1, String> {
        if carriers.iter().any(|(slot, token)| {
            slot != &token.slot
                || !token.validate_exact(
                    self.context_id,
                    self.height,
                    &self.roster,
                    self.max_chunk_count,
                )
        }) {
            return Err("sealed leader-wire ingress changed immutable geometry".to_owned());
        }
        let mut state = self
            .state
            .lock()
            .map_err(|_| "leader-wire lifecycle store lock was poisoned".to_owned())?;
        let durable_ingress = state
            .records
            .iter()
            .filter_map(|(slot, record)| {
                (record.status == LeaderWireLifecycleStatus::Ingress)
                    .then(|| (slot.clone(), record.token.clone()))
            })
            .collect::<BTreeMap<_, _>>();
        if durable_ingress != carriers
            || carriers
                .keys()
                .any(|slot| state.replay_dormant.contains(slot))
            || carriers.iter().any(|(slot, token)| {
                state.records.get(slot).is_none_or(|record| {
                    record.token != *token
                        || record.status != LeaderWireLifecycleStatus::Ingress
                        || record.runtime_owner.is_some()
                        || record.terminal_evidence.is_some()
                })
            })
        {
            return Err(
                "sealed leader-wire ingress disagreed with durable carrier ownership".to_owned(),
            );
        }
        let previous = state.clone();
        for (slot, token) in &carriers {
            let record = state
                .records
                .get_mut(slot)
                .expect("exact durable Ingress projection retains every supplied slot");
            // The exact checks above deliberately remain hard release-mode
            // checks. An Ingress row carrying downstream or terminal evidence
            // is not a parkable physical owner.
            debug_assert_eq!(record.token, *token);
            debug_assert_eq!(record.status, LeaderWireLifecycleStatus::Ingress);
            debug_assert!(record.runtime_owner.is_none());
            debug_assert!(record.terminal_evidence.is_none());
            record.status = LeaderWireLifecycleStatus::Dormant;
            let inserted = state.replay_dormant.insert(slot.clone());
            debug_assert!(inserted);
        }
        if !carriers.is_empty()
            && let Err(error) = self.persist_locked(&state)
        {
            *state = previous;
            return Err(error);
        }
        Ok(SealedLeaderWireIngressRetirementV1 { _private: () })
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
        if !state
            .recovery_authority
            .admits_ingress_identity(&token.identity)
        {
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
            let same_round_timeout_upgrade = token.identity.phase
                == FairV2IngressLeaderWirePhase::TimeoutCertificate
                && incumbent.token.identity.phase
                    == FairV2IngressLeaderWirePhase::TimeoutCertificate
                && token.identity.view == incumbent.token.identity.view;
            if (token.identity.view <= incumbent.token.identity.view && !same_round_timeout_upgrade)
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
        let Some(bytes) = self.storage.read_bounded(self.max_frame_bytes)? else {
            return Ok(false);
        };
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
            // has durably advanced beyond this view, no view-scoped control
            // owner from the obsolete episode can be resurrected. Manifest
            // chunks and request-bound certified-body responses remain
            // necessary historical recovery data even after Decision: the
            // reducer can decide before obtaining its exact body. Retire
            // rejected records while preserving both file high-watermarks so
            // subsequent admission cannot reuse an ordinal.
            if recovery_authority.retires(&record.token) {
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
        self.storage.publish_atomic(&frame, self.max_frame_bytes)
    }
}
impl ServicedCandidateStore {
    /// Open the height-bound snapshot through the safety WAL's one-shot
    /// serviced-candidate authority.
    ///
    /// # Errors
    ///
    /// Returns an error when the derived geometry overflows or an existing
    /// snapshot is missing its canonical framing, checksum, ordering, or exact
    /// height-context binding.
    pub(crate) fn open_with_safety_wal_authority(
        storage: SafetyWalServicedCandidateStoreAuthority,
        context_id: wire::HeightContextId,
        height: wire::Height,
        owner: [u8; 32],
        lifecycle_capacity: usize,
    ) -> Result<(Self, RestoredServicedCandidates), String> {
        let record_capacity = lifecycle_capacity
            .checked_mul(SERVICED_CANDIDATE_STAGES_PER_LIFECYCLE)
            .ok_or_else(|| "serviced-candidate lifecycle-stage capacity overflowed".to_owned())?;
        Self::open_with_storage_and_capacities(
            storage,
            context_id,
            height,
            owner,
            record_capacity,
            record_capacity,
        )
    }
    /// Test-only raw-path adapter for the sealed production constructor.
    #[cfg(test)]
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
        let storage = SafetyWalServicedCandidateStoreAuthority::for_test_path(safety_wal_path)?;
        Self::open_with_storage_and_capacities(
            storage,
            context_id,
            height,
            owner,
            record_capacity,
            record_capacity,
        )
    }
    #[cfg(test)]
    fn open_with_capacities(
        safety_wal_path: &Path,
        context_id: wire::HeightContextId,
        height: wire::Height,
        owner: [u8; 32],
        serviced_capacity: usize,
        producer_continuation_capacity: usize,
    ) -> Result<(Self, RestoredServicedCandidates), String> {
        let storage = SafetyWalServicedCandidateStoreAuthority::for_test_path(safety_wal_path)?;
        Self::open_with_storage_and_capacities(
            storage,
            context_id,
            height,
            owner,
            serviced_capacity,
            producer_continuation_capacity,
        )
    }
    fn open_with_storage_and_capacities(
        storage: SafetyWalServicedCandidateStoreAuthority,
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
        #[cfg(test)]
        let path = storage.path_for_test().to_path_buf();
        let store = Self {
            storage,
            #[cfg(test)]
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
        let Some(bytes) = self.storage.read_bounded(self.max_frame_bytes)? else {
            return Ok(RestoredServicedCandidates {
                records: BTreeMap::new(),
                producer_continuations: BTreeMap::new(),
                decision_reclaimed: false,
            });
        };
        if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > self.max_frame_bytes {
            return Err("serviced-candidate snapshot grew beyond its read bound".to_owned());
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
        self.storage.publish_atomic(&frame, self.max_frame_bytes)
    }
    /// Remove and directory-sync the finalized height's obsolete snapshot.
    ///
    /// # Errors
    ///
    /// Returns an error when the snapshot or its containing directory cannot
    /// be synchronized and retired.
    pub(crate) fn retire(self) -> Result<(), String> {
        self.storage.retire(self.max_frame_bytes)
    }
    /// Return the exact snapshot path for failure-injection tests.
    #[cfg(test)]
    pub(crate) fn path_for_test(&self) -> &Path {
        &self.path
    }
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
    if version != FORMAT_VERSION {
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
    let mut cursor = payload;
    let state = PersistedServicedCandidatesV4::decode_all(&mut cursor)
        .map_err(|error| format!("failed to decode v4 serviced-candidate snapshot: {error}"))?;
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
#[cfg(test)]
include!("serviced_candidate_store_cases.rs");
