//! Bounded, non-destructive proposal assembly for Sumeragi v2.
//!
//! Candidate selection deliberately snapshots pending queue entries instead of
//! acquiring [`TransactionGuard`](crate::queue::TransactionGuard)s.  A guard
//! removes its transaction when released, which creates a lossy remove/reinsert
//! window for an abandoned proposal.  Keeping queue ownership unchanged means
//! transactions selected by a losing candidate, or deferred because lane/AMX
//! work is unavailable, remain pending until the final apply path removes the
//! hashes committed by the decided block.
//!
//! This module constructs only fresh successor bodies.  A reducer lock must be
//! satisfied by loading and re-proposing the exact durable body, never by
//! rebuilding it here.
use super::v2_core::EventTag;
use super::{
    output_guard::ConsensusOutputGuard,
    v2::LocalProposalDirective,
    v2_chunks::{EncodedV2Payload, encode_payload},
    v2_lane_driver::{
        NativeLaneCandidateBatch, NativeLaneCandidatePreparation, NativeLaneDecisionHandoff,
    },
};
use crate::{
    block::{BlockBuilder, Chained},
    queue::{
        GlobalQueueSelectionLease, Queue, RoutingPlan, execution_context_for_routing_plan,
        reconcile_execution_routing_plan,
    },
    state::{State, StateReadOnly, WorldReadOnly, compute_confidential_feature_digest},
    tx::AcceptedTransaction,
};
use iroha_crypto::{Hash, HashOf, KeyPair};
use iroha_data_model::{
    block::{
        AutonomousLanePayloadEnvelopeV1, BlockExecutionContextBundle, BlockHeader,
        CertifiedMergeLedgerReference, SignedBlock,
        consensus::{NativeAmxReceipt, SumeragiLanePayloadOwnership},
        consensus_v2 as wire,
    },
    consensus::NposConsensusEffects,
    da::{commitment::DaCommitmentBundle, pin_intent::DaPinIntentBundle},
    events::pipeline::PipelineEventBox,
    merge::{MAX_MERGE_EXECUTION_BATCH_BYTES, MAX_MERGE_EXECUTION_ENTRYPOINTS, MergeLedgerEntry},
    transaction::{TransactionAdmissionIntent, TransactionEntrypoint},
};
use iroha_primitives::time::TimeSource;
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    num::{NonZeroU64, NonZeroUsize},
    time::Duration,
};
use thiserror::Error;
/// Hard local bounds applied to one candidate-assembly attempt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CandidateLimits {
    max_transactions: NonZeroUsize,
    max_payload_bytes: NonZeroUsize,
    max_queue_scan: NonZeroUsize,
}
impl CandidateLimits {
    /// Construct explicit transaction, exact-body, and queue-scan bounds.
    ///
    /// # Errors
    ///
    /// Returns [`CandidateError::ScanLimitBelowTransactionLimit`] when the
    /// requested scan cannot inspect even one complete maximum-sized batch.
    pub(crate) fn new(
        max_transactions: NonZeroUsize,
        max_payload_bytes: NonZeroUsize,
        max_queue_scan: NonZeroUsize,
    ) -> Result<Self, CandidateError> {
        if max_queue_scan < max_transactions {
            return Err(CandidateError::ScanLimitBelowTransactionLimit {
                max_transactions: max_transactions.get(),
                max_queue_scan: max_queue_scan.get(),
            });
        }
        Ok(Self {
            max_transactions,
            max_payload_bytes,
            max_queue_scan,
        })
    }
    /// Maximum entries selected across one complete carrier candidate.
    pub(crate) const fn max_transactions(self) -> NonZeroUsize {
        self.max_transactions
    }
    /// Maximum canonical carrier payload bytes.
    pub(crate) const fn max_payload_bytes(self) -> NonZeroUsize {
        self.max_payload_bytes
    }
    /// Maximum FIFO entries inspected during one selection attempt.
    pub(crate) const fn max_queue_scan(self) -> NonZeroUsize {
        self.max_queue_scan
    }
}
/// Deterministic block attachments prepared outside the global reducer.
///
/// DA proof policies and the confidential-feature digest are intentionally not
/// caller supplied: the assembler derives them from the same committed state
/// snapshot used to route transactions.  Other attachments represent
/// independently certified or executed subsystems and must be provided as
/// immutable inputs by the height runner.
#[derive(Clone, Debug, Default)]
pub(crate) struct CandidateAttachments {
    /// An enabled time trigger requires the ledger clock to advance.
    ///
    /// This is proposal work rather than serialized block metadata: advancing
    /// signed header time keeps future schedules reachable, and block
    /// execution derives any event which is due at that header.
    pub(crate) time_trigger_clock_progress_required: bool,
    /// DA commitments available for this height.
    pub(crate) da_commitments: Option<DaCommitmentBundle>,
    /// DA pin intents available for this height.
    pub(crate) da_pin_intents: Option<DaPinIntentBundle>,
    /// Mandatory NPoS penalties/pulse and canonically ordered optional evidence.
    /// The assembler selects evidence without consuming its pending custody.
    pub(crate) npos_consensus_effects: Option<NposConsensusEffects>,
    /// The exact mandatory pulse is not reconstructed yet. A complete useful
    /// snapshot returns before signing, retaining its queue and lane owners.
    pub(crate) required_beacon_pulse_pending: bool,
    /// SCCP root derived by deterministic execution, when applicable.
    pub(crate) sccp_commitment_root: Option<[u8; 32]>,
    /// Exact stripped application header certified by an autonomous merge
    /// batch. Ordinary and relay-only candidates leave this absent.
    pub(crate) certified_merge_carrier_header: Option<BlockHeader>,
    /// Complete, locally validated sidecar selected for this exact carrier round.
    /// Only its compact certified reference is embedded in the block.
    pub(crate) certified_merge_entry: Option<MergeLedgerEntry>,
    /// Authenticated complete QueuePlan inputs carried natively by this
    /// Sumeragi proposal. They are globally ordered by the block QC and never
    /// pass through the independent merge committee.
    pub(crate) queue_plan_admissions: Vec<Vec<u8>>,
}
/// Read-only description of one canonically ordered proposal candidate.
#[derive(Clone, Copy, Debug)]
pub(crate) struct CandidateDescriptor<'candidate> {
    transaction: &'candidate AcceptedTransaction<'static>,
    routing_plan: &'candidate RoutingPlan,
    entrypoint_hash: HashOf<TransactionEntrypoint>,
}
impl<'candidate> CandidateDescriptor<'candidate> {
    /// Build a read-only descriptor from one exact accepted entrypoint and
    /// routing plan.
    pub(crate) fn new(
        transaction: &'candidate AcceptedTransaction<'static>,
        routing_plan: &'candidate RoutingPlan,
    ) -> Self {
        Self {
            transaction,
            routing_plan,
            entrypoint_hash: transaction.hash_as_entrypoint(),
        }
    }
    /// Borrow the accepted queue transaction.
    pub(crate) const fn transaction(self) -> &'candidate AcceptedTransaction<'static> {
        self.transaction
    }
    /// Borrow the full coordinator/participant routing plan.
    pub(crate) const fn routing_plan(self) -> &'candidate RoutingPlan {
        self.routing_plan
    }
    /// Canonical entrypoint hash used to bind routing and execution context.
    pub(crate) const fn entrypoint_hash(self) -> HashOf<TransactionEntrypoint> {
        self.entrypoint_hash
    }
}
/// Lane-local, Native AMX, and autonomous control-anchor material for a candidate.
#[derive(Clone, Debug, Default)]
pub(crate) struct PreparedCandidateWork {
    /// Input-only Decisions rejoined to the original State observation. The
    /// reducer's Apply effects remain with the process-lived lane owner.
    pub(crate) native_lane_decisions: Option<NativeLaneCandidateBatch>,
    /// One receipt slot per descriptor. Native AMX plans require `Some` and
    /// single-route plans require `None`.
    pub(crate) native_amx_receipts: Vec<Option<NativeAmxReceipt>>,
    /// Optional lane-local certified ownerships covering the descriptor list.
    pub(crate) lane_payload_ownerships: Vec<SumeragiLanePayloadOwnership>,
    /// Canonically lane-ordered, producer-authenticated autonomous payloads
    /// anchored without ordinary global execution.
    pub(crate) autonomous_lane_payloads: Vec<AutonomousLanePayloadEnvelopeV1>,
}
impl PreparedCandidateWork {
    /// Construct work for a batch containing only available single-route entries.
    #[must_use]
    #[cfg(test)]
    pub(crate) fn single_route_batch(candidate_count: usize) -> Self {
        Self {
            native_lane_decisions: None,
            native_amx_receipts: vec![None; candidate_count],
            lane_payload_ownerships: Vec::new(),
            autonomous_lane_payloads: Vec::new(),
        }
    }
}
/// A bounded subset of candidate indices whose lane-local work is unavailable.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CandidateWorkUnavailable {
    indices: BTreeSet<usize>,
    reason: String,
    defer_native_for_episode: bool,
}
impl CandidateWorkUnavailable {
    /// Construct an unavailable-work result.
    #[must_use]
    pub(crate) fn new(indices: BTreeSet<usize>, reason: impl Into<String>) -> Self {
        Self {
            indices,
            reason: reason.into(),
            defer_native_for_episode: false,
        }
    }
    /// Defer the selected indices and suppress every later Native AMX refill
    /// for this one assembly episode.
    ///
    /// Native coordinator and participant bodies bind a dependency-coupled
    /// transaction slice. Once a provider has staged requests for that slice,
    /// refilling from a different Native cohort can create a conflicting body
    /// for the same participant slot before the first request leaves the
    /// adapter.
    #[must_use]
    pub(crate) fn defer_native_for_episode(
        indices: BTreeSet<usize>,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            indices,
            reason: reason.into(),
            defer_native_for_episode: true,
        }
    }
    /// Candidate indices which must remain queued for a later height/view.
    pub(crate) fn indices(&self) -> &BTreeSet<usize> {
        &self.indices
    }
    /// Stable diagnostic supplied by the lane/AMX adapter.
    pub(crate) fn reason(&self) -> &str {
        &self.reason
    }
    /// Whether every later Native AMX refill must be suppressed for this
    /// assembly episode.
    pub(crate) const fn defers_native_for_episode(&self) -> bool {
        self.defer_native_for_episode
    }
}
/// Why the complete snapshot cannot currently produce a useful carrier, independently of its row count.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CandidateWorkDeferral {
    /// The authenticated native source or its applying State is not available.
    NativeLaneSource,
    /// The exact committed merge frontier or installed reducer view is moving.
    MergeFrontier,
    /// Only optional evidence remains and none fits this carrier. Preserve the
    /// proof pool and recheck for newly serviceable economic work.
    EvidenceEnvelope,
}
/// Explicit provider failure scope; an empty candidate batch cannot encode snapshot deferral.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum CandidateWorkError {
    /// Only these checked positional candidates lack lane-local work.
    Unavailable(CandidateWorkUnavailable),
    /// Keep the complete snapshot queued and retry its temporary dependency.
    Deferred(CandidateWorkDeferral),
    /// Input, authority, or storage failed; never downgrade this to pending work.
    Failed(String),
    /// The provider's fail-stop authority is already closed.
    RestartRequired,
}
impl From<CandidateWorkUnavailable> for CandidateWorkError {
    fn from(unavailable: CandidateWorkUnavailable) -> Self {
        Self::Unavailable(unavailable)
    }
}
/// Snapshot adapter for lane-local and Native AMX readiness.
///
/// Implementations must be deterministic for one committed state and input
/// descriptor list. Returning unavailable indices removes only those entries
/// from this candidate; queue ownership is never changed. The assembler calls
/// [`CandidateWorkProvider::prepare`] even when `candidates` is empty so a
/// provider can surface already-reserved autonomous payloads without adding
/// their entrypoints to ordinary global execution. Providers must return one
/// Native AMX receipt slot per input descriptor and a canonically lane-ordered
/// autonomous envelope vector disjoint from those descriptors. A dependency of
/// the whole snapshot uses [`CandidateWorkError::Deferred`], never an empty
/// unavailable-index set. Input, authority and storage failures remain fatal.
pub(crate) trait CandidateWorkProvider {
    /// Prepare receipts, lane-local ownerships, and autonomous control anchors.
    fn prepare(
        &mut self,
        context: &wire::HeightContext,
        view: wire::View,
        candidates: &[CandidateDescriptor<'_>],
    ) -> Result<PreparedCandidateWork, CandidateWorkError>;
}
/// Exact parent authority available to the first executable candidate.
///
/// Ordinary heights require the complete parent body and CommitQC. Exactly one context imported
/// from an authenticated snapshot may instead use its digest-bound hash-only anchor.
#[derive(Clone, Copy, Debug)]
pub(crate) enum CandidateParent<'parent> {
    /// Complete ordinary parent block.
    Block(&'parent SignedBlock),
    /// Audited parent whose body predates the executable v2 ledger.
    Snapshot(&'parent wire::SnapshotBootstrapAnchor),
}
impl CandidateParent<'_> {
    fn height(self) -> wire::Height {
        match self {
            Self::Block(block) => block.header().height().get(),
            Self::Snapshot(anchor) => anchor.snapshot_height,
        }
    }
    pub(crate) fn hash(self) -> HashOf<iroha_data_model::block::BlockHeader> {
        match self {
            Self::Block(block) => block.hash(),
            Self::Snapshot(anchor) => anchor.snapshot_block_hash,
        }
    }
}
/// Conservative provider used when no certified Native AMX snapshot exists.
///
/// Single-route transactions remain eligible. Native AMX transactions are
/// reported unavailable and therefore remain in the queue without preventing
/// an honest leader from producing a control-work or single-route block.
#[derive(Clone, Copy, Debug, Default)]
#[cfg(test)]
pub(crate) struct SingleRouteWorkProvider;
#[cfg(test)]
impl CandidateWorkProvider for SingleRouteWorkProvider {
    fn prepare(
        &mut self,
        _context: &wire::HeightContext,
        _view: wire::View,
        candidates: &[CandidateDescriptor<'_>],
    ) -> Result<PreparedCandidateWork, CandidateWorkError> {
        let unavailable = unavailable_native_amx_indices(candidates);
        if unavailable.is_empty() {
            Ok(PreparedCandidateWork::single_route_batch(candidates.len()))
        } else {
            Err(CandidateWorkUnavailable::new(
                unavailable,
                "certified Native AMX receipts are not available",
            )
            .into())
        }
    }
}
/// Complete immutable inputs for one fresh successor candidate.
pub(crate) struct CandidateRequest<'request, Work> {
    /// Frozen height context governing this candidate.
    pub(crate) context: &'request wire::HeightContext,
    /// Reducer-owned leader/lock directive for the current incarnation.
    pub(crate) directive: LocalProposalDirective,
    /// Local validator index in the frozen roster.
    pub(crate) local_validator: wire::ValidatorIndex,
    /// Exact ordinary parent body or the one authenticated hash-only snapshot anchor.
    pub(crate) parent: CandidateParent<'request>,
    /// Committed state at the parent height.
    pub(crate) state: &'request State,
    /// Shared pending queue; selection is read-only.
    pub(crate) queue: &'request std::sync::Arc<Queue>,
    /// Consensus key corresponding to `local_validator`.
    pub(crate) key_pair: &'request KeyPair,
    /// Process-lifetime guard covering candidate signing and canonicalization.
    pub(crate) output_guard: &'request ConsensusOutputGuard,
    /// Immutable subsystem attachments for this height.
    pub(crate) attachments: CandidateAttachments,
    /// Frozen readiness adapter for lane-local and Native AMX work.
    pub(crate) work_provider: Work,
}
/// Bounded proposal-selection diagnostics.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct CandidateScanReport {
    /// Pending entries inspected from the queue snapshot.
    pub(crate) inspected: usize,
    /// Inspected entries with a routing plan resolved from committed state.
    pub(crate) routable: usize,
    /// Entries whose route could not be resolved and remain pending.
    pub(crate) unresolved: usize,
    /// Entries skipped by the transaction/body budget and left pending.
    pub(crate) payload_deferred: usize,
    /// Complete admission controls left in durable custody for a later carrier.
    pub(crate) admission_deferred: usize,
    /// Optional evidence proofs left with their original pending owner.
    pub(crate) evidence_deferred: usize,
    /// Decided native groups retained by their original lane owners.
    pub(crate) native_deferred: usize,
    /// Decided native groups included as the sole economic input form.
    pub(crate) native_selected: usize,
    /// Entries skipped because certified lane/AMX work was unavailable.
    pub(crate) work_deferred: usize,
    /// Ordinary FIFO entries excluded by an exact-empty certified execution carrier.
    pub(crate) carrier_excluded: usize,
    /// External transactions included in the final body.
    pub(crate) selected: usize,
}
/// Result of one bounded fresh-candidate assembly attempt.
#[derive(Debug)]
pub(crate) enum CandidateAssemblyOutcome {
    /// A signed body carrying ordinary, autonomous, or internal work.
    Assembled(AssembledV2Candidate),
    /// The queue snapshot and internal providers contained no proposal work.
    NoProposalWork(CandidateScanReport),
    /// Independently useful work exists, but its mandatory pulse is not ready.
    /// No body is signed and the selection lease is released without consumption.
    AwaitingRequiredBeacon(CandidateScanReport),
    /// A complete provider snapshot is temporarily unavailable, even with no selected rows.
    WorkDeferred {
        /// Queue observations made before the provider deferred.
        report: CandidateScanReport,
        /// Exact temporary dependency to recheck without changing work ownership.
        reason: CandidateWorkDeferral,
    },
}
/// Original Native preparation and the result of its one bounded assembly attempt.
///
/// Even an assembly refusal returns every exact source-recovery wait and the
/// complete prepared group set. The caller services those waits independently
/// of candidate delivery; assembly never settles the driver's original Apply.
#[must_use = "retain Native source waits even when candidate assembly is deferred or refused"]
pub(crate) struct NativeCandidateAssembly {
    pub(crate) source: NativeLaneCandidatePreparation,
    pub(crate) outcome: Result<CandidateAssemblyOutcome, CandidateError>,
}

/// Borrowed readiness of one authenticated preparation. It never supplies an
/// ordinary execution fallback when no Native group is ready.
struct NativeCandidateWork<'source>(&'source NativeLaneCandidatePreparation);

impl CandidateWorkProvider for NativeCandidateWork<'_> {
    fn prepare(
        &mut self,
        context: &wire::HeightContext,
        view: wire::View,
        candidates: &[CandidateDescriptor<'_>],
    ) -> Result<PreparedCandidateWork, CandidateWorkError> {
        if self.0.waits.iter().any(|wait| {
            matches!(
                wait,
                crate::state::LaneDecisionGroupPreparationV1::ObservationChanged
            )
        }) {
            return Err(CandidateWorkError::Deferred(
                CandidateWorkDeferral::NativeLaneSource,
            ));
        }
        if let Some(mut ready) = self.0.work.as_ref() {
            return ready.prepare(context, view, candidates);
        }
        if !candidates.is_empty() {
            return Err(CandidateWorkUnavailable::new(
                (0..candidates.len()).collect(),
                "Native candidate input cannot execute ordinary queue entries",
            )
            .into());
        }
        // Admission certificates and other independently useful controls can
        // advance while exact source waits remain with NativeCandidateAssembly.
        // The existing work gate rejects a carrier with no such work.
        Ok(PreparedCandidateWork::default())
    }
}

/// A canonical successor body and its deterministic v2 dispersal plan.
#[derive(Debug)]
pub(crate) struct AssembledV2Candidate {
    tag: EventTag,
    block: SignedBlock,
    canonical_wire: Vec<u8>,
    encoded_payload: EncodedV2Payload,
    events: Vec<PipelineEventBox>,
    scan_report: CandidateScanReport,
    _selection_lease: GlobalQueueSelectionLease,
}
impl AssembledV2Candidate {
    /// Exact reducer incarnation which authorized construction.
    pub(crate) const fn tag(&self) -> EventTag {
        self.tag
    }
    /// Borrow the signed canonical successor block.
    pub(crate) const fn block(&self) -> &SignedBlock {
        &self.block
    }
    /// Bounded queue-selection diagnostics.
    pub(crate) const fn scan_report(&self) -> CandidateScanReport {
        self.scan_report
    }
    /// Consume the candidate into the pieces used by body storage and transport.
    pub(crate) fn into_parts(
        self,
    ) -> (
        SignedBlock,
        Vec<u8>,
        EncodedV2Payload,
        Vec<PipelineEventBox>,
        CandidateScanReport,
        GlobalQueueSelectionLease,
    ) {
        (
            self.block,
            self.canonical_wire,
            self.encoded_payload,
            self.events,
            self.scan_report,
            self._selection_lease,
        )
    }
}
#[derive(Clone, Debug)]
struct CandidateRecord {
    transaction: AcceptedTransaction<'static>,
    routing_plan: RoutingPlan,
    entrypoint_hash: HashOf<TransactionEntrypoint>,
    encoded_len: usize,
    source_ordinal: usize,
}
impl CandidateRecord {
    fn descriptor(&self) -> CandidateDescriptor<'_> {
        CandidateDescriptor {
            transaction: &self.transaction,
            routing_plan: &self.routing_plan,
            entrypoint_hash: self.entrypoint_hash,
        }
    }
}
/// Non-destructive bounded candidate assembler.
#[derive(Clone, Debug)]
pub(crate) struct V2CandidateAssembler {
    limits: CandidateLimits,
    time_source: TimeSource,
}
impl V2CandidateAssembler {
    /// Construct an assembler with explicit bounds and a production/mock clock.
    #[must_use]
    pub(crate) const fn new(limits: CandidateLimits, time_source: TimeSource) -> Self {
        Self {
            limits,
            time_source,
        }
    }
    /// Prepare authenticated Native Decisions and feed the existing carrier fitter.
    ///
    /// Run on the candidate worker, not the control turn: source preparation
    /// authenticates original finalized bodies and route Decisions. The caller
    /// retains the immutable driver handoff across errors; once preparation is
    /// complete, all its waits and evidence return beside the assembly outcome.
    /// No State execution, validation vote, publication or Apply occurs here.
    /// The process-lived candidate worker returns this original preparation on
    /// every outcome; source waits never become ordinary execution or empty work.
    pub(crate) fn assemble_native(
        &self,
        request: CandidateRequest<'_, &NativeLaneDecisionHandoff>,
    ) -> Result<NativeCandidateAssembly, CandidateError> {
        validate_request(&request)?;
        if !request.work_provider.belongs_to(request.state) {
            return Err(CandidateError::NativeLaneDecisionInvalid(
                "Native handoff belongs to another State owner".into(),
            ));
        }
        if request.attachments.certified_merge_entry.is_some()
            || request.attachments.certified_merge_carrier_header.is_some()
        {
            return Err(CandidateError::NativeLaneDecisionInvalid(
                "Native candidate cannot retain a retired certified merge attachment".into(),
            ));
        }
        let source = request
            .work_provider
            .prepare_candidate()
            .map_err(CandidateError::WorkPreparationFailed)?;
        let CandidateRequest {
            context,
            directive,
            local_validator,
            parent,
            state,
            queue,
            key_pair,
            output_guard,
            attachments,
            work_provider: _,
        } = request;
        let outcome = self.assemble(CandidateRequest {
            context,
            directive,
            local_validator,
            parent,
            state,
            queue,
            key_pair,
            output_guard,
            attachments,
            work_provider: NativeCandidateWork(&source),
        });
        Ok(NativeCandidateAssembly { source, outcome })
    }

    /// Assemble, sign, exactly encode, and deterministically chunk one fresh
    /// successor body.
    ///
    /// Candidate selection never consumes the queue. An empty queue or a batch
    /// whose positional work cannot fit returns
    /// [`CandidateAssemblyOutcome::NoProposalWork`] unless genuine internal
    /// work exists. A temporary dependency of the complete provider snapshot
    /// returns [`CandidateAssemblyOutcome::WorkDeferred`] without signing or
    /// removing entries, including when the selected batch is empty.
    ///
    /// # Errors
    ///
    /// Returns [`CandidateError`] for a stale reducer directive, a non-leader
    /// caller, parent/context drift, malformed certified work, signing failure,
    /// or proposal framing which itself exceeds frozen body/chunk limits.
    pub(crate) fn assemble<Work: CandidateWorkProvider>(
        &self,
        mut request: CandidateRequest<'_, Work>,
    ) -> Result<CandidateAssemblyOutcome, CandidateError> {
        validate_request(&request)?;
        if request.queue.transaction_selection_durability_faulted() {
            return Err(CandidateError::RestartRequired);
        }
        let tag = request.directive.tag();
        let view = tag.view();
        let exact_payload_limit = self.limits.max_payload_bytes.get().min(
            usize::try_from(request.context.da_layout.max_payload_size_bytes).unwrap_or(usize::MAX),
        );
        let mut report = CandidateScanReport::default();
        let state_view = request.state.view();
        let selection_max = effective_output_transaction_limit(
            self.limits.max_transactions,
            state_view.world().parameters().block(),
        )?;
        let (pending, mut selection_lease) = request
            .queue
            .bounded_pending_snapshot(&state_view, self.limits.max_queue_scan)
            .ok_or(CandidateError::RestartRequired)?;
        drop(state_view);
        let pool = self.snapshot_routable_candidates(
            request.queue,
            request.state,
            &request.attachments,
            pending,
            exact_payload_limit,
            &mut report,
        )?;
        let mut reserve = VecDeque::from(pool);
        let mut selected = Vec::with_capacity(selection_max);
        fill_selection(
            &mut selected,
            &mut reserve,
            selection_max,
            exact_payload_limit,
            &mut report,
        );
        let original_npos_effects = request.attachments.npos_consensus_effects.clone();
        // Every iteration either returns or permanently removes at least one
        // of the at-most `max_queue_scan` inspected records.
        let max_attempts = self.limits.max_queue_scan.get().saturating_add(1);
        for _ in 0..max_attempts {
            request.attachments.npos_consensus_effects = original_npos_effects.clone();
            let candidate_creation_time =
                self.prospective_candidate_creation_time(view, request.parent, &selected);
            let candidate_ledger_time_ms =
                u64::try_from(candidate_creation_time.as_millis()).unwrap_or(u64::MAX);
            let unavailable_routes = self.unreconciled_execution_route_indices(
                request.context,
                request.state,
                &selected,
                candidate_ledger_time_ms,
            )?;
            if !unavailable_routes.is_empty() {
                let unavailable = CandidateWorkUnavailable::new(
                    unavailable_routes,
                    "committed routing plan is unavailable at the exact candidate time",
                );
                remove_unavailable_candidates(&mut selected, &unavailable, &mut report)?;
                fill_selection(
                    &mut selected,
                    &mut reserve,
                    selection_max,
                    exact_payload_limit,
                    &mut report,
                );
                continue;
            }
            // Restore exact FIFO payload order after any unavailable-work removal
            // and refill. Routing contexts and work receipts are positional and
            // must be prepared against that exact order.
            order_records_by_fifo(&mut selected);
            let descriptors = selected
                .iter()
                .map(CandidateRecord::descriptor)
                .collect::<Vec<_>>();
            let mut prepared_work =
                match request
                    .work_provider
                    .prepare(request.context, view, &descriptors)
                {
                    Ok(work) => work,
                    Err(CandidateWorkError::Unavailable(unavailable)) => {
                        let defer_native_for_episode = unavailable.defers_native_for_episode();
                        remove_unavailable_candidates(&mut selected, &unavailable, &mut report)?;
                        if defer_native_for_episode {
                            defer_native_candidates_for_episode(
                                &mut selected,
                                &mut reserve,
                                &mut report,
                            );
                        }
                        fill_selection(
                            &mut selected,
                            &mut reserve,
                            selection_max,
                            exact_payload_limit,
                            &mut report,
                        );
                        continue;
                    }
                    Err(CandidateWorkError::Deferred(reason)) => {
                        if request.queue.transaction_selection_durability_faulted() {
                            return Err(CandidateError::RestartRequired);
                        }
                        validate_request(&request)?;
                        return Ok(CandidateAssemblyOutcome::WorkDeferred { report, reason });
                    }
                    Err(CandidateWorkError::Failed(reason)) => {
                        return Err(CandidateError::WorkPreparationFailed(reason));
                    }
                    Err(CandidateWorkError::RestartRequired) => {
                        return Err(CandidateError::RestartRequired);
                    }
                };
            validate_prepared_work(request.context, view, &descriptors, &prepared_work)?;
            if let Some(native) = prepared_work.native_lane_decisions.as_mut() {
                if !native.is_current(request.state, request.context) {
                    return Ok(CandidateAssemblyOutcome::WorkDeferred {
                        report,
                        reason: CandidateWorkDeferral::NativeLaneSource,
                    });
                }
                report.native_deferred = native.deferred_groups()
                    + native.batch().groups.len().saturating_sub(selection_max);
                native.retain_prefix(native.batch().groups.len().min(selection_max));
            }
            report.selected = selected.len();
            let candidate_header = BlockHeader::new(
                NonZeroU64::new(request.context.height)
                    .ok_or(CandidateError::BuiltHeaderMismatch)?,
                Some(request.parent.hash()),
                None,
                candidate_ledger_time_ms,
                view,
            );
            if !candidate_has_proposal_work(&selected, &request.attachments, &prepared_work)
                && request
                    .state
                    .deterministic_start_work_pending(&candidate_header)
                    .map_err(CandidateError::LocalStateAdmission)?
                    != Some(true)
            {
                if request.queue.transaction_selection_durability_faulted() {
                    return Err(CandidateError::RestartRequired);
                }
                validate_request(&request)?;
                return Ok(CandidateAssemblyOutcome::NoProposalWork(report));
            }
            if request.attachments.required_beacon_pulse_pending {
                if request.queue.transaction_selection_durability_faulted() {
                    return Err(CandidateError::RestartRequired);
                }
                validate_request(&request)?;
                return Ok(CandidateAssemblyOutcome::AwaitingRequiredBeacon(report));
            }
            let algorithm = request
                .key_pair
                .public_key()
                .try_algorithm()
                .map_err(|error| CandidateError::Signing(error.to_string()))?;
            let evidence_count = original_npos_effects
                .as_ref()
                .map_or(0, |effects| effects.v2_evidence_admissions.len());
            if evidence_count > 0 {
                // The existing candidate owner gives each class first opportunity
                // on alternating heights. Views cannot reset this priority. On
                // the evidence turn, measure against the actual mandatory base;
                // do not let an oversized optional batch strand every class.
                let preferred_count = if candidate_economic_work_first(request.context.height) {
                    0
                } else {
                    let mut mandatory = request.attachments.clone();
                    mandatory.queue_plan_admissions.clear();
                    let anchors = PreparedCandidateWork {
                        autonomous_lane_payloads: prepared_work.autonomous_lane_payloads.clone(),
                        ..PreparedCandidateWork::default()
                    };
                    let base = self.prepare_block_builder(
                        request.context,
                        tag,
                        request.parent,
                        request.state,
                        &mandatory,
                        &[],
                        &anchors,
                        candidate_creation_time,
                    )?;
                    fit_evidence_prefix(
                        base,
                        &original_npos_effects,
                        u64::from(request.local_validator),
                        algorithm,
                        request.context.da_layout,
                        exact_payload_limit,
                    )?
                    .1
                };
                request.attachments.npos_consensus_effects =
                    npos_effects_prefix(&original_npos_effects, preferred_count);
            }
            let mut builder = self.prepare_block_builder(
                request.context,
                tag,
                request.parent,
                request.state,
                &request.attachments,
                &selected,
                &prepared_work,
                candidate_creation_time,
            )?;
            let mut encoded_bytes = builder
                .canonical_proposal_wire_len(u64::from(request.local_validator), algorithm)
                .map_err(CandidateError::CanonicalEncoding)?;
            let mut chunk_count = encoded_chunk_count(request.context.da_layout, encoded_bytes)?;
            let mut first_admission_size = None;
            let mut first_native_size = None;
            if encoded_bytes > exact_payload_limit
                || chunk_count > request.context.da_layout.max_chunk_count as usize
            {
                if selected.pop().is_some() {
                    report.payload_deferred = report.payload_deferred.saturating_add(1);
                    // Keep a canonical FIFO prefix. No private-key operation has
                    // occurred and every removed row retains queue ownership.
                    continue;
                }
                // Native groups are indivisible economic inputs. Fit a strict
                // admission-priority prefix on the actual fully framed builder;
                // never split a group or acknowledge its retained Apply effect.
                if let Some(native) = prepared_work.native_lane_decisions.as_ref() {
                    let native_base = builder
                        .clone()
                        .retain_queue_plan_admission_prefix(0)
                        .map_err(CandidateError::CanonicalEncoding)?;
                    let original_count = native.batch().groups.len();
                    let mut low = 0;
                    let mut high = original_count;
                    while low < high {
                        let mid = low + (high - low).div_ceil(2);
                        let trial = native_base
                            .clone()
                            .retain_native_lane_decision_prefix(mid)
                            .map_err(CandidateError::CanonicalEncoding)?
                            .with_network_input_time_floor(candidate_creation_time)
                            .ok_or(CandidateError::BlockTimeOverflow)?;
                        let bytes = trial
                            .canonical_proposal_wire_len(
                                u64::from(request.local_validator),
                                algorithm,
                            )
                            .map_err(CandidateError::CanonicalEncoding)?;
                        let chunks = encoded_chunk_count(request.context.da_layout, bytes)?;
                        if mid == 1 {
                            first_native_size = Some((bytes, chunks));
                        }
                        if bytes <= exact_payload_limit
                            && chunks <= request.context.da_layout.max_chunk_count as usize
                        {
                            low = mid;
                        } else {
                            high = mid - 1;
                        }
                    }
                    if low < original_count {
                        builder = builder
                            .retain_native_lane_decision_prefix(low)
                            .map_err(CandidateError::CanonicalEncoding)?
                            .with_network_input_time_floor(candidate_creation_time)
                            .ok_or(CandidateError::BlockTimeOverflow)?;
                        report.native_deferred += original_count - low;
                        if low == 0 {
                            prepared_work.native_lane_decisions = None;
                        } else if let Some(native) = prepared_work.native_lane_decisions.as_mut() {
                            native.retain_prefix(low);
                        }
                        encoded_bytes = builder
                            .canonical_proposal_wire_len(
                                u64::from(request.local_validator),
                                algorithm,
                            )
                            .map_err(CandidateError::CanonicalEncoding)?;
                        chunk_count =
                            encoded_chunk_count(request.context.da_layout, encoded_bytes)?;
                    }
                }
                let original_count = request.attachments.queue_plan_admissions.len();
                let mut low = 0usize;
                let mut high = original_count;
                // All mandatory fields are already on this exact builder. Only
                // the admission prefix varies; canonical uncompressed framing is
                // monotone, so a bounded binary search includes real overhead.
                while low < high {
                    let mid = low + (high - low).div_ceil(2);
                    let trial = builder
                        .clone()
                        .retain_queue_plan_admission_prefix(mid)
                        .map_err(CandidateError::CanonicalEncoding)?;
                    let bytes = trial
                        .canonical_proposal_wire_len(u64::from(request.local_validator), algorithm)
                        .map_err(CandidateError::CanonicalEncoding)?;
                    let chunks = encoded_chunk_count(request.context.da_layout, bytes)?;
                    if mid == 1 {
                        first_admission_size = Some((bytes, chunks));
                    }
                    if bytes <= exact_payload_limit
                        && chunks <= request.context.da_layout.max_chunk_count as usize
                    {
                        low = mid;
                    } else {
                        high = mid - 1;
                    }
                }
                if low < original_count {
                    builder = builder
                        .retain_queue_plan_admission_prefix(low)
                        .map_err(CandidateError::CanonicalEncoding)?;
                    request.attachments.queue_plan_admissions.truncate(low);
                    report.admission_deferred = original_count - low;
                    encoded_bytes = builder
                        .canonical_proposal_wire_len(u64::from(request.local_validator), algorithm)
                        .map_err(CandidateError::CanonicalEncoding)?;
                    chunk_count = encoded_chunk_count(request.context.da_layout, encoded_bytes)?;
                }
                if encoded_bytes > exact_payload_limit
                    || chunk_count > request.context.da_layout.max_chunk_count as usize
                {
                    // If no admission fits, report the smallest non-empty
                    // candidate rather than the stripped empty envelope.
                    let (encoded_bytes, chunk_count) = if low == 0 {
                        first_native_size
                            .or(first_admission_size)
                            .unwrap_or((encoded_bytes, chunk_count))
                    } else {
                        (encoded_bytes, chunk_count)
                    };
                    return Err(CandidateError::ProposalFramingExceedsPayloadLimits {
                        encoded_bytes,
                        encoded_chunks: chunk_count,
                        max_bytes: exact_payload_limit,
                        max_chunks: request.context.da_layout.max_chunk_count,
                    });
                }
            }
            if evidence_count > 0 {
                // Fill only the remaining space after the preferred class has
                // obtained its opportunity. Recompute the NPoS header hash while
                // preserving every mandatory penalty and the exact beacon pulse.
                let (fitted, count) = fit_evidence_prefix(
                    builder,
                    &original_npos_effects,
                    u64::from(request.local_validator),
                    algorithm,
                    request.context.da_layout,
                    exact_payload_limit,
                )?;
                builder = fitted;
                request.attachments.npos_consensus_effects =
                    npos_effects_prefix(&original_npos_effects, count);
                report.evidence_deferred = evidence_count - count;
                encoded_bytes = builder
                    .canonical_proposal_wire_len(u64::from(request.local_validator), algorithm)
                    .map_err(CandidateError::CanonicalEncoding)?;
                chunk_count = encoded_chunk_count(request.context.da_layout, encoded_bytes)?;
            }
            if !candidate_has_proposal_work(&selected, &request.attachments, &prepared_work)
                && request
                    .state
                    .deterministic_start_work_pending(&candidate_header)
                    .map_err(CandidateError::LocalStateAdmission)?
                    != Some(true)
            {
                // Optional evidence cannot manufacture an empty/pulse-only
                // carrier or terminate the runner when no proof fits. Retain its
                // original custody and the existing bounded snapshot recheck so
                // later economic arrivals can still obtain their opportunity.
                // TODO: admission must prove that every accepted proof has a
                // reachable envelope under the frozen mandatory-metadata bound.
                if first_admission_size.is_none() && evidence_count > 0 {
                    if request.queue.transaction_selection_durability_faulted() {
                        return Err(CandidateError::RestartRequired);
                    }
                    validate_request(&request)?;
                    return Ok(CandidateAssemblyOutcome::WorkDeferred {
                        report,
                        reason: CandidateWorkDeferral::EvidenceEnvelope,
                    });
                }
                // Required admission work still reports its real non-empty
                // envelope before entering the fail-stop signing region.
                let (encoded_bytes, chunk_count) = first_native_size
                    .or(first_admission_size)
                    .unwrap_or((encoded_bytes, chunk_count));
                return Err(CandidateError::ProposalFramingExceedsPayloadLimits {
                    encoded_bytes,
                    encoded_chunks: chunk_count,
                    max_bytes: exact_payload_limit,
                    max_chunks: request.context.da_layout.max_chunk_count,
                });
            }
            // Candidate signing begins only after the complete actual carrier
            // fits. The sizing projection never signs or publishes placeholder bytes.
            let _native_publication = prepared_work
                .native_lane_decisions
                .as_ref()
                .map(|_| request.state.consensus_publication_lease());
            if prepared_work
                .native_lane_decisions
                .as_ref()
                .is_some_and(|native| !native.is_current(request.state, request.context))
            {
                return Ok(CandidateAssemblyOutcome::WorkDeferred {
                    report,
                    reason: CandidateWorkDeferral::NativeLaneSource,
                });
            }
            let candidate_creation_time = builder.creation_time();
            let signing = request
                .output_guard
                .begin_fail_stop_operation()
                .ok_or(CandidateError::RestartRequired)?;
            let (block, canonical_wire, events) = self.sign_prepared_block(
                builder,
                request.context,
                tag,
                request.local_validator,
                request.parent,
                request.key_pair,
                &selected,
                candidate_creation_time,
            )?;
            if canonical_wire.len() != encoded_bytes {
                return Err(CandidateError::CanonicalEncoding(
                    "signed carrier length differs from its exact unsigned sizing projection"
                        .to_owned(),
                ));
            }
            if !candidate_block_has_proposal_work(
                &block,
                request.state,
                request.attachments.time_trigger_clock_progress_required,
            )
            .map_err(CandidateError::LocalStateAdmission)?
            {
                return Err(CandidateError::BuiltWithoutProposalWork);
            }
            let subject = wire::BlockSubject {
                parent_block_hash: Some(request.parent.hash()),
                block_hash: block.hash(),
                payload_hash: Hash::new(&canonical_wire),
            };
            let round = wire::ConsensusRound {
                context_id: request.context.id(),
                height: request.context.height,
                view,
            };
            let encoded_payload = encode_payload(request.context, round, subject, &canonical_wire)
                .map_err(|error| CandidateError::PayloadEncoding(error.to_string()))?;
            // The height owner is serialized in production, but recheck the
            // committed tip after all bounded external work so an accidental
            // concurrent block-sync commit cannot publish a stale candidate.
            validate_request(&request)?;
            report.selected = selected.len();
            report.native_selected = prepared_work
                .native_lane_decisions
                .as_ref()
                .map_or(0, |native| native.batch().groups.len());
            let selected_hashes = selected
                .iter()
                .map(|record| record.transaction.hash_as_entrypoint())
                .collect::<Vec<_>>();
            if !selection_lease.retain_only(&selected_hashes) {
                return Err(CandidateError::RestartRequired);
            }
            signing.complete();
            return Ok(CandidateAssemblyOutcome::Assembled(AssembledV2Candidate {
                tag,
                block,
                canonical_wire,
                encoded_payload,
                events,
                scan_report: report,
                _selection_lease: selection_lease,
            }));
        }
        Err(CandidateError::AssemblyDidNotConverge)
    }
    fn unreconciled_execution_route_indices(
        &self,
        context: &wire::HeightContext,
        state: &State,
        selected: &[CandidateRecord],
        ledger_time_ms: u64,
    ) -> Result<BTreeSet<usize>, CandidateError> {
        if selected.is_empty() {
            return Ok(BTreeSet::new());
        }
        let mut unavailable = BTreeSet::new();
        let state_view = state.view();
        for (index, candidate) in selected.iter().enumerate() {
            if let Err(error) = reconcile_execution_routing_plan(
                &candidate.transaction,
                &candidate.routing_plan,
                &state_view,
                ledger_time_ms,
                context.height,
            ) {
                if error.is_deferable() {
                    unavailable.insert(index);
                } else {
                    return Err(CandidateError::RoutingAdmissionEvidence(error.to_string()));
                }
            }
        }
        Ok(unavailable)
    }
    fn prospective_candidate_creation_time(
        &self,
        view: wire::View,
        parent: CandidateParent<'_>,
        selected: &[CandidateRecord],
    ) -> Duration {
        let transactions = selected
            .iter()
            .map(|candidate| candidate.transaction.clone())
            .collect::<Vec<_>>();
        let pending = BlockBuilder::new_with_time_source(transactions, self.time_source.clone());
        let builder = match parent {
            CandidateParent::Block(parent) => pending.chain(view, Some(parent)),
            CandidateParent::Snapshot(anchor) => pending.chain_with_parent_hash(
                view,
                anchor.snapshot_height,
                anchor.snapshot_block_hash,
            ),
        };
        builder.carrier_context_header().creation_time()
    }
    fn snapshot_routable_candidates(
        &self,
        queue: &Queue,
        state: &State,
        attachments: &CandidateAttachments,
        pending: Vec<AcceptedTransaction<'static>>,
        payload_limit: usize,
        report: &mut CandidateScanReport,
    ) -> Result<Vec<CandidateRecord>, CandidateError> {
        if queue.transaction_selection_durability_faulted() {
            return Err(CandidateError::RestartRequired);
        }
        let certified_execution_selected = attachments
            .certified_merge_entry
            .as_ref()
            .and_then(|entry| entry.execution_batch.as_ref())
            .is_some();
        let mut carrier_queue_plan_bindings = BTreeMap::new();
        for certificate in &attachments.queue_plan_admissions {
            let admission = crate::torii_proxy::decode_and_validate_lane_admitted_input_v1(
                state.network_id_ref(),
                certificate,
            )
            .map_err(CandidateError::MergeApplicationContext)?;
            let binding = admission.certificate().certificate.binding.clone();
            if carrier_queue_plan_bindings
                .insert(binding.entrypoint_hash.clone(), binding)
                .is_some()
            {
                return Err(CandidateError::MergeApplicationContext(
                    "Sumeragi carrier repeats a QueuePlan entrypoint".to_owned(),
                ));
            }
        }
        let mut records = Vec::with_capacity(pending.len());
        for (source_ordinal, transaction) in pending.into_iter().enumerate() {
            report.inspected = report.inspected.saturating_add(1);
            if record_ordinary_execution_carrier_exclusion(certified_execution_selected, report) {
                continue;
            }
            let entrypoint_hash = transaction.hash_as_entrypoint();
            let queue_plan_synced = transaction.entrypoint().admission_intent()
                == TransactionAdmissionIntent::QueuePlanSynced;
            let queue_plan_binding = if queue_plan_synced {
                match carrier_queue_plan_bindings.get(&entrypoint_hash) {
                    Some(binding) => Some(binding.clone()),
                    None => match state
                        .queue_plan_pending_binding_for_entrypoint(entrypoint_hash.clone())
                    {
                        Ok(Some(binding)) => Some(binding),
                        Ok(None) => {
                            // A QueuePlan transaction is a FIFO barrier until the same carrier or
                            // canonical parent state owns its exact admission certificate. Skipping
                            // it would let later work overtake a signature-bound admission promise.
                            break;
                        }
                        Err(_) => return Err(CandidateError::RestartRequired),
                    },
                }
            } else {
                None
            };
            let routing_plan = match queue.route_plan_with_state(&transaction, state) {
                Ok(plan) => plan,
                Err(_) => {
                    report.unresolved = report.unresolved.saturating_add(1);
                    return Err(CandidateError::RestartRequired);
                }
            };
            if let Some(binding) = queue_plan_binding
                && let Err(reason) = crate::torii_proxy::validate_queue_plan_binding_for_request(
                    &binding,
                    state.network_id_ref(),
                    transaction.entrypoint(),
                    &routing_plan,
                )
            {
                return Err(CandidateError::MergeApplicationContext(format!(
                    "QueuePlan candidate differs from its immutable admission binding: {reason}"
                )));
            }
            if queue.transaction_selection_durability_faulted() {
                return Err(CandidateError::RestartRequired);
            }
            report.routable = report.routable.saturating_add(1);
            if queue_plan_synced {
                // The certificate is proposal-native control work; the
                // transaction itself must cross the autonomous lane and merge
                // corridor. Keep it as a strict FIFO cut even after the exact
                // admission binding becomes canonical, so later ordinary work
                // cannot overtake it while the lane author takes ownership.
                report.work_deferred = report.work_deferred.saturating_add(1);
                break;
            }
            let encoded_len = transaction.encoded_len();
            if encoded_len > payload_limit {
                report.payload_deferred = report.payload_deferred.saturating_add(1);
                continue;
            }
            records.push(CandidateRecord {
                entrypoint_hash,
                transaction,
                routing_plan,
                encoded_len,
                source_ordinal,
            });
        }
        if queue.transaction_selection_durability_faulted() {
            return Err(CandidateError::RestartRequired);
        }
        Ok(records)
    }
    #[allow(clippy::too_many_arguments)]
    fn prepare_block_builder(
        &self,
        context: &wire::HeightContext,
        tag: EventTag,
        parent: CandidateParent<'_>,
        state: &State,
        attachments: &CandidateAttachments,
        selected: &[CandidateRecord],
        prepared_work: &PreparedCandidateWork,
        candidate_creation_time: Duration,
    ) -> Result<BlockBuilder<Chained>, CandidateError> {
        // TODO: compose DA/pin/SCCP with the recorded Native consumer before
        // production activation. Refuse unsupported input before any signing;
        // a shape-valid bundle alone does not prove executable carrier controls.
        if prepared_work.native_lane_decisions.is_some()
            && (attachments.da_commitments.is_some()
                || attachments.da_pin_intents.is_some()
                || attachments.sccp_commitment_root.is_some())
        {
            return Err(CandidateError::NativeLaneDecisionInvalid(
                "native execution does not support additional carrier controls (DA, pin or SCCP)"
                    .into(),
            ));
        }
        let transactions = selected
            .iter()
            .map(|candidate| candidate.transaction.clone())
            .collect::<Vec<_>>();
        let (_, frozen_time_source) = TimeSource::new_mock(candidate_creation_time);
        let pending = BlockBuilder::new_with_time_source(transactions, frozen_time_source);
        let mut builder = match parent {
            CandidateParent::Block(parent) => pending.chain(tag.view(), Some(parent)),
            CandidateParent::Snapshot(anchor) => pending.chain_with_parent_hash(
                tag.view(),
                anchor.snapshot_height,
                anchor.snapshot_block_hash,
            ),
        };
        let certified_batch_header = attachments
            .certified_merge_entry
            .as_ref()
            .and_then(|entry| entry.execution_batch.as_ref())
            .map(|batch| &batch.application_block_header);
        match (
            attachments.certified_merge_carrier_header.as_ref(),
            certified_batch_header,
        ) {
            (Some(certified_header), Some(batch_header)) => {
                let built_context = builder.carrier_context_header();
                if !stripped_carrier_context_matches(&built_context, certified_header)
                    || batch_header != certified_header
                {
                    return Err(CandidateError::MergeApplicationContext(
                        "certified autonomous merge header differs from the shared carrier context"
                            .to_owned(),
                    ));
                }
            }
            (None, None) => {}
            (Some(_), None) | (None, Some(_)) => {
                return Err(CandidateError::MergeApplicationContext(
                    "certified autonomous merge entry has a partial carrier-header binding"
                        .to_owned(),
                ));
            }
        }
        if let Some(batch) = attachments
            .certified_merge_entry
            .as_ref()
            .and_then(|entry| entry.execution_batch.as_ref())
        {
            builder = builder
                .bind_certified_merge_application_context(&batch.application_block_header)
                .map_err(|reason| CandidateError::MergeApplicationContext(reason.to_owned()))?;
        }
        let nexus = state.nexus_snapshot();
        builder = builder
            .with_da_commitments(attachments.da_commitments.clone())
            .with_da_proof_policies(Some(crate::da::active_proof_policy_bundle_at_height(
                &nexus,
                context.height,
            )))
            .with_da_pin_intents(attachments.da_pin_intents.clone())
            .with_npos_consensus_effects(attachments.npos_consensus_effects.clone())
            .with_sccp_commitment_root(attachments.sccp_commitment_root);
        let state_view = state.view();
        let confidential = compute_confidential_feature_digest(
            state_view.world(),
            state_view.zk(),
            state_view.sccp_registry(),
            context.height,
        );
        drop(state_view);
        builder =
            builder.with_confidential_features((!confidential.is_empty()).then_some(confidential));
        let execution_context = selected
            .iter()
            .zip(&prepared_work.native_amx_receipts)
            .map(|(candidate, receipt)| {
                let execution = execution_context_for_routing_plan(
                    candidate.entrypoint_hash,
                    &candidate.routing_plan,
                );
                receipt.clone().map_or(execution.clone(), |receipt| {
                    execution.with_native_amx_receipt(receipt)
                })
            })
            .collect::<Vec<_>>();
        let mut execution_context = BlockExecutionContextBundle::new(execution_context)
            .with_autonomous_lane_payloads(prepared_work.autonomous_lane_payloads.clone())
            .with_lane_payload_ownerships(prepared_work.lane_payload_ownerships.clone())
            .with_queue_plan_admissions(attachments.queue_plan_admissions.clone());
        if let Some(native) = prepared_work.native_lane_decisions.as_ref() {
            execution_context =
                execution_context.with_native_lane_decisions(native.batch().clone());
        }
        if let Some(entry) = attachments.certified_merge_entry.as_ref() {
            execution_context =
                execution_context.with_merge_entry(CertifiedMergeLedgerReference::new(entry));
        }
        execution_context
            .validate_native_lane_decisions_shape()
            .map_err(CandidateError::NativeLaneDecisionInvalid)?;
        builder = builder
            .with_execution_context((!execution_context.is_empty()).then_some(execution_context));
        builder
            .with_network_input_time_floor(candidate_creation_time)
            .ok_or(CandidateError::BlockTimeOverflow)
    }
    #[allow(clippy::too_many_arguments)]
    fn sign_prepared_block(
        &self,
        builder: BlockBuilder<Chained>,
        context: &wire::HeightContext,
        tag: EventTag,
        local_validator: wire::ValidatorIndex,
        parent: CandidateParent<'_>,
        key_pair: &KeyPair,
        selected: &[CandidateRecord],
        candidate_creation_time: Duration,
    ) -> Result<(SignedBlock, Vec<u8>, Vec<PipelineEventBox>), CandidateError> {
        let mut events = Vec::new();
        let new_block = builder
            .try_sign_with_index(key_pair.private_key(), u64::from(local_validator))
            .map_err(|error| CandidateError::Signing(error.to_string()))?
            .unpack(|event| events.push(event));
        let block: SignedBlock = new_block.into();
        if block.header().height().get() != context.height
            || block.header().view_change_index() != tag.view()
            || block.header().prev_block_hash() != Some(parent.hash())
            || block.header().creation_time() != candidate_creation_time
        {
            return Err(CandidateError::BuiltHeaderMismatch);
        }
        if !block.is_resultless_proposal() {
            return Err(CandidateError::BuiltResultBearingProposal);
        }
        let built_entrypoint_hashes = block
            .external_entrypoints_cloned()
            .map(|entrypoint| entrypoint.hash())
            .collect::<Vec<_>>();
        let selected_entrypoint_hashes = selected
            .iter()
            .map(|candidate| candidate.entrypoint_hash)
            .collect::<Vec<_>>();
        if built_entrypoint_hashes != selected_entrypoint_hashes {
            return Err(CandidateError::BuiltEntrypointOrderMismatch);
        }
        let canonical_wire = block
            .encode_wire()
            .map_err(|error| CandidateError::CanonicalEncoding(error.to_string()))?;
        Ok((block, canonical_wire, events))
    }
}
/// Alternate first carrier opportunity without a view-sensitive cursor or a
/// second scheduler. Odd heights favor evidence; even heights favor economic work.
pub(crate) const fn candidate_economic_work_first(height: wire::Height) -> bool {
    height % 2 == 0
}
fn npos_effects_prefix(
    original: &Option<NposConsensusEffects>,
    count: usize,
) -> Option<NposConsensusEffects> {
    original
        .clone()
        .map(|mut effects| {
            effects.v2_evidence_admissions.truncate(count);
            effects
        })
        .filter(|effects| !effects.is_empty())
}
/// Choose the largest canonical evidence prefix using the same complete wire
/// projection as the final carrier. Selection never consumes pending custody.
fn fit_evidence_prefix(
    builder: BlockBuilder<Chained>,
    original: &Option<NposConsensusEffects>,
    signatory: u64,
    algorithm: iroha_crypto::Algorithm,
    layout: wire::DataAvailabilityLayout,
    payload_limit: usize,
) -> Result<(BlockBuilder<Chained>, usize), CandidateError> {
    let mut low = 0;
    let mut high = original
        .as_ref()
        .map_or(0, |effects| effects.v2_evidence_admissions.len());
    while low < high {
        let mid = low + (high - low).div_ceil(2);
        let trial = builder
            .clone()
            .with_npos_consensus_effects(npos_effects_prefix(original, mid));
        let bytes = trial
            .canonical_proposal_wire_len(signatory, algorithm)
            .map_err(CandidateError::CanonicalEncoding)?;
        let chunks = encoded_chunk_count(layout, bytes)?;
        if bytes <= payload_limit && chunks <= layout.max_chunk_count as usize {
            low = mid;
        } else {
            high = mid - 1;
        }
    }
    Ok((
        builder.with_npos_consensus_effects(npos_effects_prefix(original, low)),
        low,
    ))
}
// A mandatory beacon pulse accompanies independently useful ledger work; it
// must never manufacture the carrier whose height requests that same pulse.
fn npos_effects_have_independent_proposal_work(effects: &NposConsensusEffects) -> bool {
    !effects.v2_evidence_admissions.is_empty() || !effects.penalty_actions.is_empty()
}
fn candidate_has_proposal_work(
    selected: &[CandidateRecord],
    attachments: &CandidateAttachments,
    prepared_work: &PreparedCandidateWork,
) -> bool {
    !selected.is_empty()
        || prepared_work.native_lane_decisions.is_some()
        || !prepared_work.autonomous_lane_payloads.is_empty()
        || attachments.time_trigger_clock_progress_required
        || attachments
            .da_commitments
            .as_ref()
            .is_some_and(|bundle| !bundle.is_empty())
        || attachments
            .da_pin_intents
            .as_ref()
            .is_some_and(|bundle| !bundle.is_empty())
        || attachments
            .npos_consensus_effects
            .as_ref()
            .is_some_and(npos_effects_have_independent_proposal_work)
        || attachments.sccp_commitment_root.is_some()
        || attachments.certified_merge_carrier_header.is_some()
        || attachments.certified_merge_entry.is_some()
        || !attachments.queue_plan_admissions.is_empty()
}
/// Return whether a canonical resultless v2 body carries deterministic ledger
/// work. The caller supplies the state-derived clock-progress decision for the
/// exact parent: clock progress is semantic work even when no trigger fires in
/// this particular block and therefore no trigger entrypoint is serialized.
///
/// Scheduled state transitions are independently derived from the exact parent
/// and body header. They need no synthetic transaction or additional wire flag.
/// This is the common fail-closed boundary used after fresh assembly, before
/// validating an inbound body, and before re-proposing a recovered locked body.
pub(crate) fn candidate_block_has_proposal_work(
    block: &SignedBlock,
    state: &State,
    time_trigger_clock_progress_required: bool,
) -> Result<bool, crate::state::StateBlockStartError<iroha_data_model::executor::IvmAdmissionError>>
{
    let independent = block.external_entrypoints_cloned().next().is_some()
        || block.execution_context().is_some_and(|context| {
            !context.autonomous_lane_payloads.is_empty()
                || context
                    .native_lane_decisions
                    .as_ref()
                    .is_some_and(|batch| !batch.groups.is_empty())
                || context.merge_entry.is_some()
                || !context.queue_plan_admissions().is_empty()
        })
        || block
            .da_commitments()
            .is_some_and(|bundle| !bundle.is_empty())
        || block
            .da_pin_intents()
            .is_some_and(|bundle| !bundle.is_empty())
        || block
            .npos_consensus_effects()
            .is_some_and(npos_effects_have_independent_proposal_work)
        || block.header().sccp_commitment_root().is_some()
        || time_trigger_clock_progress_required;
    Ok(independent || state.deterministic_start_work_pending(&block.header())? == Some(true))
}
// Headers carry proposal identity only; complete outputs belong to BlockResult.
// A stripped context therefore removes only the Network input commitment.
fn stripped_carrier_context_matches(
    built_header: &BlockHeader,
    certified_header: &BlockHeader,
) -> bool {
    certified_header.merkle_root().is_none()
        && built_header.height() == certified_header.height()
        && built_header.prev_block_hash() == certified_header.prev_block_hash()
        && built_header.creation_time() == certified_header.creation_time()
        && built_header.view_change_index() == certified_header.view_change_index()
}
fn record_ordinary_execution_carrier_exclusion(
    certified_execution_selected: bool,
    report: &mut CandidateScanReport,
) -> bool {
    // A certified autonomous execution batch commits an exact-empty global
    // carrier. Every ordinary queue candidate therefore conflicts regardless
    // of its timestamp or entrypoint identity.
    if !certified_execution_selected {
        return false;
    }
    report.carrier_excluded = report.carrier_excluded.saturating_add(1);
    true
}
fn validate_request<Work>(request: &CandidateRequest<'_, Work>) -> Result<(), CandidateError> {
    request
        .context
        .validate()
        .map_err(|error| CandidateError::InvalidContext(error.to_string()))?;
    let tag = request.directive.tag();
    if tag.height() != request.context.height {
        return Err(CandidateError::StaleDirective {
            directive_height: tag.height(),
            context_height: request.context.height,
        });
    }
    let expected_leader = request.context.leader(tag.view());
    if request.directive.leader() != expected_leader {
        return Err(CandidateError::DirectiveLeaderMismatch {
            directive: request.directive.leader(),
            expected: expected_leader,
        });
    }
    if request.local_validator != expected_leader {
        return Err(CandidateError::NotExpectedLeader {
            local: request.local_validator,
            expected: expected_leader,
        });
    }
    if request.directive.decided_subject().is_some() {
        return Err(CandidateError::HeightAlreadyDecided);
    }
    if request.directive.locked_subject().is_some() {
        return Err(CandidateError::LockedBodyMustBeReproposed);
    }
    let local = request
        .context
        .roster
        .get(usize::try_from(request.local_validator).unwrap_or(usize::MAX))
        .ok_or(CandidateError::LocalValidatorOutsideRoster)?;
    if local.validator.public_key() != request.key_pair.public_key() {
        return Err(CandidateError::ConsensusKeyMismatch);
    }
    validate_candidate_parent(request.context, request.parent, request.state)?;
    Ok(())
}
fn validate_candidate_parent(
    context: &wire::HeightContext,
    parent: CandidateParent<'_>,
    state: &State,
) -> Result<wire::Height, CandidateError> {
    let parent_height = parent.height();
    match parent {
        CandidateParent::Block(parent) => {
            if context.snapshot_bootstrap.is_some()
                || parent_height.checked_add(1) != Some(context.height)
                || parent.hash()
                    != context
                        .parent_commit_qc
                        .as_ref()
                        .ok_or(CandidateError::MissingParentCertificate)?
                        .subject
                        .block_hash
            {
                return Err(CandidateError::ParentContextMismatch);
            }
        }
        CandidateParent::Snapshot(anchor) => {
            if context.parent_commit_qc.is_some()
                || context.snapshot_bootstrap.as_ref() != Some(anchor)
                || anchor.snapshot_height.checked_add(1) != Some(context.height)
            {
                return Err(CandidateError::ParentContextMismatch);
            }
        }
    }
    let state_view = state.view();
    let state_matches = state_view.height() == usize::try_from(parent_height).unwrap_or(usize::MAX)
        && state_view.latest_block_hash() == Some(parent.hash())
        && state_view.network_id() == &context.network_id;
    drop(state_view);
    if !state_matches {
        return Err(CandidateError::ParentStateMismatch);
    }
    Ok(parent_height)
}
fn order_records_by_fifo(records: &mut [CandidateRecord]) {
    records.sort_by(|left, right| {
        left.source_ordinal
            .cmp(&right.source_ordinal)
            .then_with(|| left.entrypoint_hash.cmp(&right.entrypoint_hash))
    });
}
fn effective_output_transaction_limit(
    configured_max: NonZeroUsize,
    parameters: iroha_data_model::parameter::BlockParameters,
) -> Result<usize, CandidateError> {
    let terminal_max = parameters
        .execution_output()
        .maximum_terminal_network_inputs()
        .map_err(CandidateError::InvalidOutputCapacity)?;
    let terminal_max = usize::try_from(terminal_max).map_err(|_| {
        CandidateError::InvalidOutputCapacity("terminal count exceeds host index width".into())
    })?;
    Ok(
        effective_candidate_transaction_limit(configured_max, parameters.max_transactions())
            .min(terminal_max),
    )
}

fn effective_candidate_transaction_limit(
    configured_max: NonZeroUsize,
    protocol_max: NonZeroU64,
) -> usize {
    configured_max
        .get()
        .min(usize::try_from(protocol_max.get()).unwrap_or(usize::MAX))
}
fn fill_selection(
    selected: &mut Vec<CandidateRecord>,
    reserve: &mut VecDeque<CandidateRecord>,
    max_transactions: usize,
    payload_limit: usize,
    report: &mut CandidateScanReport,
) {
    let mut estimated_bytes = selected.iter().fold(0usize, |total, candidate| {
        total.saturating_add(candidate.encoded_len)
    });
    while selected.len() < max_transactions {
        let Some(candidate) = reserve.pop_front() else {
            break;
        };
        let next = estimated_bytes.saturating_add(candidate.encoded_len);
        if next > payload_limit {
            report.payload_deferred = report.payload_deferred.saturating_add(1);
            continue;
        }
        estimated_bytes = next;
        selected.push(candidate);
    }
}
fn remove_unavailable_candidates(
    selected: &mut Vec<CandidateRecord>,
    unavailable: &CandidateWorkUnavailable,
    report: &mut CandidateScanReport,
) -> Result<(), CandidateError> {
    if unavailable.indices().is_empty() || unavailable.reason().trim().is_empty() {
        return Err(CandidateError::MalformedUnavailableWork);
    }
    if unavailable
        .indices()
        .iter()
        .any(|index| *index >= selected.len())
    {
        return Err(CandidateError::UnavailableIndexOutOfRange);
    }
    for index in unavailable.indices().iter().rev() {
        selected.remove(*index);
        report.work_deferred = report.work_deferred.saturating_add(1);
    }
    Ok(())
}
fn defer_native_candidates_for_episode(
    selected: &mut Vec<CandidateRecord>,
    reserve: &mut VecDeque<CandidateRecord>,
    report: &mut CandidateScanReport,
) {
    let selected_before = selected.len();
    selected.retain(|candidate| !matches!(&candidate.routing_plan, RoutingPlan::NativeAmx(_)));
    let reserve_before = reserve.len();
    reserve.retain(|candidate| !matches!(&candidate.routing_plan, RoutingPlan::NativeAmx(_)));
    report.work_deferred = report
        .work_deferred
        .saturating_add(selected_before.saturating_sub(selected.len()))
        .saturating_add(reserve_before.saturating_sub(reserve.len()));
}
fn validate_prepared_work(
    context: &wire::HeightContext,
    view: wire::View,
    candidates: &[CandidateDescriptor<'_>],
    prepared: &PreparedCandidateWork,
) -> Result<(), CandidateError> {
    if let Some(native) = prepared.native_lane_decisions.as_ref() {
        if !candidates.is_empty()
            || !prepared.native_amx_receipts.is_empty()
            || !prepared.lane_payload_ownerships.is_empty()
            || !prepared.autonomous_lane_payloads.is_empty()
        {
            return Err(CandidateError::NativeLaneDecisionInvalid(
                "native Decisions cannot share another economic input form".into(),
            ));
        }
        native
            .batch()
            .canonical_hash()
            .map_err(CandidateError::NativeLaneDecisionInvalid)?;
        if native.batch().base_state_height.checked_add(1) != Some(context.height) {
            return Err(CandidateError::NativeLaneDecisionInvalid(
                "native Decisions belong to another applying height".into(),
            ));
        }
    }
    if prepared.native_amx_receipts.len() != candidates.len() {
        return Err(CandidateError::NativeAmxReceiptCountMismatch {
            candidates: candidates.len(),
            receipts: prepared.native_amx_receipts.len(),
        });
    }
    for (index, (candidate, receipt)) in candidates
        .iter()
        .zip(&prepared.native_amx_receipts)
        .enumerate()
    {
        match (candidate.routing_plan(), receipt) {
            (RoutingPlan::Single(_), None) | (RoutingPlan::NativeAmx(_), Some(_)) => {}
            (RoutingPlan::Single(_), Some(_)) => {
                return Err(CandidateError::UnexpectedNativeAmxReceipt(index));
            }
            (RoutingPlan::NativeAmx(_), None) => {
                return Err(CandidateError::MissingNativeAmxReceipt(index));
            }
        }
    }
    validate_autonomous_lane_payloads(context, candidates, &prepared.autonomous_lane_payloads)?;
    if prepared.lane_payload_ownerships.is_empty() {
        return Ok(());
    }
    let mut covered = BTreeSet::new();
    for ownership in &prepared.lane_payload_ownerships {
        if ownership.proposal_height != context.height || ownership.proposal_view != view {
            return Err(CandidateError::LaneOwnershipRoundMismatch);
        }
        if ownership.accepted_candidate_indices.len() != ownership.accepted_transaction_hashes.len()
        {
            return Err(CandidateError::LaneOwnershipHashCountMismatch);
        }
        for (raw_index, committed_hash) in ownership
            .accepted_candidate_indices
            .iter()
            .zip(&ownership.accepted_transaction_hashes)
        {
            let index = usize::try_from(*raw_index)
                .map_err(|_| CandidateError::LaneOwnershipIndexOutOfRange)?;
            let candidate = candidates
                .get(index)
                .ok_or(CandidateError::LaneOwnershipIndexOutOfRange)?;
            if !covered.insert(index) {
                return Err(CandidateError::LaneOwnershipDuplicateIndex(index));
            }
            let route = candidate.routing_plan().coordinator_route();
            if ownership.lane_id != route.lane_id
                || ownership.dataspace_id != route.dataspace_id
                || *committed_hash != Hash::from(candidate.entrypoint_hash())
            {
                return Err(CandidateError::LaneOwnershipCandidateMismatch(index));
            }
        }
        ownership
            .validate_replay_material()
            .map_err(|error| CandidateError::LaneOwnershipInvalid(error.to_string()))?;
    }
    if covered.len() != candidates.len() {
        return Err(CandidateError::LaneOwnershipIncompleteCoverage);
    }
    Ok(())
}
fn validate_autonomous_lane_payloads(
    context: &wire::HeightContext,
    candidates: &[CandidateDescriptor<'_>],
    envelopes: &[AutonomousLanePayloadEnvelopeV1],
) -> Result<(), CandidateError> {
    if envelopes.len() > MAX_MERGE_EXECUTION_ENTRYPOINTS {
        return Err(CandidateError::AutonomousLanePayloadCountExceeded {
            count: envelopes.len(),
            max: MAX_MERGE_EXECUTION_ENTRYPOINTS,
        });
    }
    let ordinary_entrypoints = candidates
        .iter()
        .map(|candidate| Hash::from(candidate.entrypoint_hash()))
        .collect::<BTreeSet<_>>();
    let expected_network_id = context.network_id;
    let aggregate_bytes = envelopes.iter().try_fold(0usize, |aggregate, envelope| {
        let envelope_bytes = norito::encode_canonical(envelope)
            .map_err(|error| CandidateError::AutonomousLanePayloadInvalid(error.to_string()))?;
        aggregate.checked_add(envelope_bytes.len()).ok_or(
            CandidateError::AutonomousLanePayloadAggregateBytesExceeded {
                bytes: usize::MAX,
                max: MAX_MERGE_EXECUTION_BATCH_BYTES,
            },
        )
    })?;
    if aggregate_bytes > MAX_MERGE_EXECUTION_BATCH_BYTES {
        return Err(
            CandidateError::AutonomousLanePayloadAggregateBytesExceeded {
                bytes: aggregate_bytes,
                max: MAX_MERGE_EXECUTION_BATCH_BYTES,
            },
        );
    }
    let mut previous_order_key = None;
    let mut route_incarnations = BTreeSet::new();
    let mut lane_blocks = BTreeSet::new();
    let mut proposal_hashes = BTreeSet::new();
    let mut descriptor_hashes = BTreeSet::new();
    let mut payload_hashes = BTreeSet::new();
    let mut autonomous_entrypoints = BTreeSet::new();
    for envelope in envelopes {
        if envelope.proposal_height != context.height {
            return Err(CandidateError::AutonomousLanePayloadHeightMismatch {
                expected: context.height,
                actual: envelope.proposal_height,
            });
        }
        let order_key = (
            envelope.lane_id,
            envelope.dataspace_id,
            envelope.lane_incarnation,
            envelope.lane_block_height,
            envelope.lane_block_view,
            envelope.proposal_hash,
            envelope.payload_hash,
        );
        if !route_incarnations.insert((
            envelope.lane_id,
            envelope.dataspace_id,
            envelope.lane_incarnation,
        )) {
            return Err(CandidateError::AutonomousLanePayloadDuplicateRoute);
        }
        if !lane_blocks.insert((
            envelope.lane_id,
            envelope.dataspace_id,
            envelope.lane_incarnation,
            envelope.lane_block_height,
            envelope.lane_block_view,
        )) {
            return Err(CandidateError::AutonomousLanePayloadDuplicateLaneBlock);
        }
        if !proposal_hashes.insert(envelope.proposal_hash) {
            return Err(CandidateError::AutonomousLanePayloadDuplicateProposal);
        }
        if !descriptor_hashes.insert(envelope.descriptor_hash) {
            return Err(CandidateError::AutonomousLanePayloadDuplicateDescriptor);
        }
        if !payload_hashes.insert(envelope.payload_hash) {
            return Err(CandidateError::AutonomousLanePayloadDuplicatePayload);
        }
        if previous_order_key
            .as_ref()
            .is_some_and(|previous| previous >= &order_key)
        {
            return Err(CandidateError::AutonomousLanePayloadOrder);
        }
        previous_order_key = Some(order_key);
        let payload = crate::lane_consensus::decode_autonomous_lane_payload_envelope(
            envelope,
            expected_network_id,
            context.epoch,
        )
        .map_err(|error| CandidateError::AutonomousLanePayloadInvalid(error.to_string()))?;
        for entrypoint_hash in payload.entrypoint_hashes {
            if ordinary_entrypoints.contains(&entrypoint_hash) {
                return Err(CandidateError::AutonomousLanePayloadOverlapsOrdinary);
            }
            if !autonomous_entrypoints.insert(entrypoint_hash) {
                return Err(CandidateError::AutonomousLanePayloadDuplicateEntrypoint);
            }
        }
    }
    Ok(())
}
#[cfg(test)]
fn unavailable_native_amx_indices(candidates: &[CandidateDescriptor<'_>]) -> BTreeSet<usize> {
    candidates
        .iter()
        .enumerate()
        .filter_map(|(index, candidate)| {
            matches!(candidate.routing_plan(), RoutingPlan::NativeAmx(_)).then_some(index)
        })
        .collect()
}
fn encoded_chunk_count(
    layout: wire::DataAvailabilityLayout,
    payload_len: usize,
) -> Result<usize, CandidateError> {
    let chunk_size = usize::try_from(layout.chunk_size_bytes)
        .map_err(|_| CandidateError::InvalidDataAvailabilityLayout)?;
    if payload_len == 0 || chunk_size == 0 {
        return Err(CandidateError::InvalidDataAvailabilityLayout);
    }
    let data_chunks = payload_len.div_ceil(chunk_size);
    let data_shards = usize::from(layout.data_shards);
    let parity_shards = usize::from(layout.parity_shards);
    if data_shards == 0 || parity_shards == 0 || !chunk_size.is_multiple_of(2) {
        return Err(CandidateError::InvalidDataAvailabilityLayout);
    }
    let stripe_width = data_shards
        .checked_add(parity_shards)
        .ok_or(CandidateError::InvalidDataAvailabilityLayout)?;
    let stripes = data_chunks.div_ceil(data_shards);
    stripes
        .checked_mul(stripe_width)
        .ok_or(CandidateError::InvalidDataAvailabilityLayout)
}
/// Candidate construction failure.
#[derive(Debug, Error)]
pub(crate) enum CandidateError {
    /// Local State acquisition could not proceed; no candidate was signed.
    #[error("local State admission: {0}")]
    LocalStateAdmission(
        crate::state::StateBlockStartError<iroha_data_model::executor::IvmAdmissionError>,
    ),
    /// A prior fatal consensus operation requires process restart.
    #[error("Sumeragi v2 candidate signing requires process restart")]
    RestartRequired,
    /// Queue scan limit is smaller than the maximum block transaction count.
    #[error(
        "Sumeragi v2 queue scan limit {max_queue_scan} is below transaction limit {max_transactions}"
    )]
    ScanLimitBelowTransactionLimit {
        /// Maximum external transactions.
        max_transactions: usize,
        /// Maximum inspected queue entries.
        max_queue_scan: usize,
    },
    /// The agreed output policy cannot reserve a terminal plan before selection.
    #[error("invalid agreed execution output capacity: {0}")]
    InvalidOutputCapacity(String),
    /// Frozen height context failed structural validation.
    #[error("invalid Sumeragi v2 height context: {0}")]
    InvalidContext(String),
    /// Reducer directive belongs to another height.
    #[error(
        "stale Sumeragi v2 proposal directive for height {directive_height}; current height is {context_height}"
    )]
    StaleDirective {
        /// Height carried by the reducer tag.
        directive_height: u64,
        /// Frozen context height.
        context_height: u64,
    },
    /// Adapter directive and frozen context disagree about the view leader.
    #[error("proposal directive leader {directive} differs from frozen leader {expected}")]
    DirectiveLeaderMismatch {
        /// Leader exposed by the reducer adapter.
        directive: wire::ValidatorIndex,
        /// Leader recomputed from the immutable context.
        expected: wire::ValidatorIndex,
    },
    /// The local validator is not the expected rotating leader.
    #[error("local validator {local} is not expected leader {expected}")]
    NotExpectedLeader {
        /// Local frozen-roster index.
        local: wire::ValidatorIndex,
        /// Expected frozen-roster index.
        expected: wire::ValidatorIndex,
    },
    /// A decided height cannot accept another fresh body.
    #[error("Sumeragi v2 height is already decided")]
    HeightAlreadyDecided,
    /// A lock requires exact durable-body reproposal.
    #[error("Sumeragi v2 locked subject must be re-proposed from exact durable bytes")]
    LockedBodyMustBeReproposed,
    /// Local validator index is absent from the roster.
    #[error("local Sumeragi v2 validator is outside the frozen roster")]
    LocalValidatorOutsideRoster,
    /// Local private key does not correspond to the roster entry.
    #[error("local Sumeragi v2 consensus key differs from the frozen roster key")]
    ConsensusKeyMismatch,
    /// A non-genesis context omitted its parent CommitQC.
    #[error("Sumeragi v2 successor context has no parent CommitQC")]
    MissingParentCertificate,
    /// Parent body, height, or parent CommitQC disagree.
    #[error("Sumeragi v2 parent block does not match the frozen height context")]
    ParentContextMismatch,
    /// Committed state does not end at the supplied parent.
    #[error("Sumeragi v2 committed state does not match the supplied parent block")]
    ParentStateMismatch,
    /// Work provider returned no indices or a blank reason.
    #[error("Sumeragi v2 work provider returned a malformed unavailable-work result")]
    MalformedUnavailableWork,
    /// A provider rejected its input, authority, or storage rather than deferring work.
    #[error("Sumeragi v2 candidate work preparation failed: {0}")]
    WorkPreparationFailed(String),
    /// Parent WSV contains malformed or conflicting QueuePlan admission evidence.
    #[error("Sumeragi v2 candidate routing admission evidence is invalid: {0}")]
    RoutingAdmissionEvidence(String),
    /// Work provider returned an index outside its candidate input.
    #[error("Sumeragi v2 unavailable-work index is outside the candidate batch")]
    UnavailableIndexOutOfRange,
    /// Work receipt vector is not aligned with the candidate list.
    #[error(
        "Sumeragi v2 Native AMX receipt count {receipts} differs from candidate count {candidates}"
    )]
    NativeAmxReceiptCountMismatch {
        /// Candidate count.
        candidates: usize,
        /// Receipt-slot count.
        receipts: usize,
    },
    /// Single-route work carried a Native AMX receipt.
    #[error("single-route candidate {0} unexpectedly carries a Native AMX receipt")]
    UnexpectedNativeAmxReceipt(usize),
    /// Native AMX work omitted its certificate.
    #[error("Native AMX candidate {0} is missing its certified receipt")]
    MissingNativeAmxReceipt(usize),
    /// Autonomous anchor count exceeds the protocol-wide bounded source count.
    #[error("Sumeragi v2 autonomous lane payload count {count} exceeds the hard limit {max}")]
    AutonomousLanePayloadCountExceeded {
        /// Supplied autonomous payload count.
        count: usize,
        /// Protocol-wide hard limit.
        max: usize,
    },
    /// Aggregate exact canonical anchor bytes exceed the merge execution budget.
    #[error("Sumeragi v2 autonomous lane payload bytes {bytes} exceed the hard limit {max}")]
    AutonomousLanePayloadAggregateBytesExceeded {
        /// Supplied aggregate exact envelope bytes.
        bytes: usize,
        /// Protocol-wide hard limit.
        max: usize,
    },
    /// An autonomous payload envelope or its exact embedded payload is malformed.
    #[error("invalid Sumeragi v2 autonomous lane payload: {0}")]
    AutonomousLanePayloadInvalid(String),
    /// An autonomous payload was prepared for another global height.
    #[error(
        "Sumeragi v2 autonomous lane payload height {actual} differs from candidate height {expected}"
    )]
    AutonomousLanePayloadHeightMismatch {
        /// Frozen candidate height.
        expected: u64,
        /// Payload proposal height.
        actual: u64,
    },
    /// Autonomous payloads are not in strict canonical lane order.
    #[error("Sumeragi v2 autonomous lane payloads are not in strict canonical lane order")]
    AutonomousLanePayloadOrder,
    /// A route/incarnation supplied more than one autonomous payload.
    #[error("Sumeragi v2 autonomous lane payload route/incarnation is duplicated")]
    AutonomousLanePayloadDuplicateRoute,
    /// A lane-local height/view identity was duplicated.
    #[error("Sumeragi v2 autonomous lane payload height/view identity is duplicated")]
    AutonomousLanePayloadDuplicateLaneBlock,
    /// A proposal hash was duplicated across autonomous payloads.
    #[error("Sumeragi v2 autonomous lane payload proposal hash is duplicated")]
    AutonomousLanePayloadDuplicateProposal,
    /// A descriptor hash was duplicated across autonomous payloads.
    #[error("Sumeragi v2 autonomous lane payload descriptor hash is duplicated")]
    AutonomousLanePayloadDuplicateDescriptor,
    /// A payload hash was duplicated across autonomous payloads.
    #[error("Sumeragi v2 autonomous lane payload hash is duplicated")]
    AutonomousLanePayloadDuplicatePayload,
    /// An autonomous transaction appeared in more than one anchored lane.
    #[error("Sumeragi v2 autonomous lane payload entrypoint is duplicated")]
    AutonomousLanePayloadDuplicateEntrypoint,
    /// An anchored autonomous transaction is also present in the ordinary block body.
    #[error("Sumeragi v2 autonomous lane payload overlaps ordinary global execution")]
    AutonomousLanePayloadOverlapsOrdinary,
    /// Lane ownership belongs to another global round.
    #[error("lane-local ownership belongs to another global proposal round")]
    LaneOwnershipRoundMismatch,
    /// Lane ownership index/hash vectors are not aligned.
    #[error("lane-local ownership index and hash counts differ")]
    LaneOwnershipHashCountMismatch,
    /// Lane ownership index is not representable or outside the candidate list.
    #[error("lane-local ownership index is outside the candidate batch")]
    LaneOwnershipIndexOutOfRange,
    /// Lane ownership covers an entry more than once.
    #[error("lane-local ownership covers candidate {0} more than once")]
    LaneOwnershipDuplicateIndex(usize),
    /// Lane ownership route or entrypoint hash disagrees with the candidate.
    #[error("lane-local ownership does not match candidate {0}")]
    LaneOwnershipCandidateMismatch(usize),
    /// Lane ownership replay hashes are malformed.
    #[error("invalid lane-local ownership replay material: {0}")]
    LaneOwnershipInvalid(String),
    /// Non-empty lane ownerships do not cover every selected entrypoint.
    #[error("lane-local ownerships do not cover the complete candidate batch")]
    LaneOwnershipIncompleteCoverage,
    /// Native input form, exact base or certified group structure is invalid.
    #[error("invalid native lane Decision candidate: {0}")]
    NativeLaneDecisionInvalid(String),
    /// Frozen DA layout cannot deterministically encode chunks.
    #[error("invalid Sumeragi v2 data-availability layout")]
    InvalidDataAvailabilityLayout,
    /// Block signing failed.
    #[error("certified merge application context is invalid: {0}")]
    MergeApplicationContext(String),
    /// Block signing failed.
    #[error("failed to sign Sumeragi v2 candidate: {0}")]
    Signing(String),
    /// Built header drifted from context/tag/parent inputs.
    #[error("built Sumeragi v2 candidate header differs from immutable inputs")]
    BuiltHeaderMismatch,
    /// The exact input clock has no representable successor millisecond.
    #[error("Sumeragi v2 candidate logical time exceeds u64 milliseconds")]
    BlockTimeOverflow,
    /// BlockBuilder output order drifted from execution-context order.
    #[error("built Sumeragi v2 entrypoint order differs from its routing contexts")]
    BuiltEntrypointOrderMismatch,
    /// BlockBuilder unexpectedly attached deterministic execution output.
    #[error("built Sumeragi v2 candidate is not resultless")]
    BuiltResultBearingProposal,
    /// A post-build check found no transaction, internal, autonomous, or
    /// state-derived clock-progress work.
    #[error("built Sumeragi v2 candidate carries no deterministic proposal work")]
    BuiltWithoutProposalWork,
    /// Canonical block framing failed.
    #[error("failed to encode canonical Sumeragi v2 body: {0}")]
    CanonicalEncoding(String),
    /// Mandatory proposal framing exceeds the immutable height limits.
    #[error(
        "Sumeragi v2 proposal framing needs {encoded_bytes} bytes/{encoded_chunks} chunks, exceeding {max_bytes} bytes/{max_chunks} chunks"
    )]
    ProposalFramingExceedsPayloadLimits {
        /// Exact canonical body bytes.
        encoded_bytes: usize,
        /// Deterministic encoded chunks.
        encoded_chunks: usize,
        /// Effective exact-body limit.
        max_bytes: usize,
        /// Frozen chunk-count limit.
        max_chunks: u32,
    },
    /// Deterministic manifest/chunk generation failed.
    #[error("failed to encode Sumeragi v2 payload: {0}")]
    PayloadEncoding(String),
    /// Internal progress bound was exhausted without returning or removing work.
    #[error("bounded Sumeragi v2 candidate assembly did not converge")]
    AssemblyDidNotConverge,
}
#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use crate::{
        block::ValidBlock,
        kura::Kura,
        query::store::LiveQueryStore,
        queue::{LaneQueueReservationKeyV1, RouteLeg, RouteLegRole, RoutingDecision},
        state::{State, World},
        sumeragi::network_topology::Topology,
    };
    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
    use iroha_data_model::{
        account::AccountId,
        block::consensus::{LaneBlockDescriptorV1, LaneBlockProposalV1},
        consensus::VALIDATOR_SET_HASH_VERSION_V1,
        nexus::AxtPolicySnapshot,
        transaction::TransactionBuilder,
    };
    use iroha_model_base::chain::ChainId;
    use iroha_model_base::peer::PeerId;
    use iroha_model_base::{topology::DataSpaceId, topology::LaneId};
    use mv::storage::StorageReadOnly;
    use nonzero_ext::nonzero;
    use std::{
        borrow::Cow,
        num::{NonZeroU64, NonZeroUsize},
        sync::Arc,
        time::Duration,
    };
    fn nonzero(value: usize) -> NonZeroUsize {
        NonZeroUsize::new(value).expect("test value is non-zero")
    }
    fn accepted(seed: u8, _label: &str) -> AcceptedTransaction<'static> {
        let key = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
            .expect("deterministic transaction key");
        let authority = AccountId::new(key.public_key().clone());
        let tx = TransactionBuilder::new(
            crate::sumeragi::synthetic_network_id("v2-candidate-test"),
            authority,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .sign(key.private_key());
        AcceptedTransaction::new_unchecked(Cow::Owned(tx))
    }
    fn accepted_with_intent(
        seed: u8,
        intent: TransactionAdmissionIntent,
    ) -> AcceptedTransaction<'static> {
        let key = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
            .expect("deterministic transaction key");
        let authority = AccountId::new(key.public_key().clone());
        let tx = TransactionBuilder::new(
            crate::sumeragi::synthetic_network_id("v2-candidate-test"),
            authority,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_admission_intent(intent)
        .sign(key.private_key());
        AcceptedTransaction::new_unchecked(Cow::Owned(tx))
    }
    fn autonomous_accepted(seed: u8) -> AcceptedTransaction<'static> {
        accepted_with_intent(seed, TransactionAdmissionIntent::QueuePlanSynced)
    }
    fn record(seed: u8, label: &str, source_ordinal: usize) -> CandidateRecord {
        let transaction = accepted(seed, label);
        CandidateRecord {
            entrypoint_hash: transaction.hash_as_entrypoint(),
            encoded_len: transaction.encoded_len(),
            transaction,
            routing_plan: RoutingPlan::single(RoutingDecision::default()),
            source_ordinal,
        }
    }
    fn autonomous_record(seed: u8, source_ordinal: usize) -> CandidateRecord {
        let transaction = autonomous_accepted(seed);
        CandidateRecord {
            entrypoint_hash: transaction.hash_as_entrypoint(),
            encoded_len: transaction.encoded_len(),
            transaction,
            routing_plan: RoutingPlan::single(RoutingDecision::default()),
            source_ordinal,
        }
    }
    fn autonomous_envelope(
        context: &wire::HeightContext,
        lane_id: LaneId,
        dataspace_id: DataSpaceId,
        lane_incarnation: Hash,
        lane_block_height: u64,
        lane_block_view: u64,
        transaction: &AcceptedTransaction<'static>,
        key_seed: u8,
    ) -> AutonomousLanePayloadEnvelopeV1 {
        let keypairs = (0..3)
            .map(|offset| {
                KeyPair::try_from_seed(
                    vec![key_seed.saturating_add(offset); 32],
                    Algorithm::BlsNormal,
                )
                .expect("deterministic autonomous validator key")
            })
            .collect::<Vec<_>>();
        let mut validator_set = keypairs
            .iter()
            .map(|key| PeerId::new(key.public_key().clone()))
            .collect::<Vec<_>>();
        validator_set.sort();
        let validator_count = u32::try_from(validator_set.len()).expect("validator count fits u32");
        let min_quorum = u32::try_from(crate::sumeragi::network_topology::commit_quorum_from_len(
            validator_set.len(),
        ))
        .expect("validator quorum fits u32");
        let entrypoint_hash = Hash::from(transaction.hash_as_entrypoint());
        let previous_lane_block_height = lane_block_height.saturating_sub(1);
        let mut descriptor = LaneBlockDescriptorV1 {
            lane_id,
            dataspace_id,
            lane_incarnation,
            proposal_height: context.height,
            previous_lane_block_height,
            previous_lane_block_descriptor_hash: (previous_lane_block_height > 0)
                .then(|| Hash::new(b"candidate autonomous predecessor")),
            lane_block_height,
            lane_block_view,
            subject_hash: Hash::new(b"candidate autonomous subject"),
            payload_ownership_hash: Hash::new(b"candidate autonomous ownership"),
            rbc_instance_hash: Hash::new(b"candidate autonomous rbc"),
            accepted_candidate_indices: vec![0],
            accepted_transaction_hashes: vec![entrypoint_hash],
            validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
            validator_set_hash: HashOf::new(&validator_set),
            validator_set: validator_set.clone(),
            validator_count,
            min_quorum,
            qc_mode_tag: format!("permissioned:lane:{lane_id}:dataspace:{dataspace_id}"),
            descriptor_hash: Hash::prehashed([0; Hash::LENGTH]),
        };
        descriptor.descriptor_hash = descriptor.computed_descriptor_hash();
        let mut proposal = LaneBlockProposalV1 {
            descriptor,
            proposal_hash: Hash::prehashed([0; Hash::LENGTH]),
            payload_block_hint: None,
        };
        proposal.proposal_hash = proposal.computed_proposal_hash();
        let producer = crate::lane_consensus::deterministic_lane_author(
            &validator_set,
            proposal.descriptor.lane_block_height,
        )
        .cloned()
        .expect("fixture has a deterministic autonomous producer");
        let producer_key = keypairs
            .iter()
            .find(|key| key.public_key() == producer.public_key())
            .expect("producer belongs to fixture validator set");
        let routing_plan = RoutingPlan::single(RoutingDecision::new(lane_id, dataspace_id));
        let reservation = LaneQueueReservationKeyV1 {
            version: LaneQueueReservationKeyV1::VERSION,
            entrypoint_hash: transaction.hash_as_entrypoint(),
            queue_plan_admission_binding_hash: Hash::new(b"candidate-queue-plan-admission-binding"),
            routing_plan_digest: routing_plan.digest(),
            coordinator_leg: routing_plan.coordinator_leg(),
            lane_id,
            dataspace_id,
            lane_incarnation,
            proposal_height: context.height,
            lane_block_height,
            lane_block_view,
            reservation_owner_hash: Hash::new(b"candidate autonomous reservation owner"),
            proposal_identity_hash: proposal.proposal_hash,
        };
        let network_id = context.network_id;
        let payload = crate::lane_consensus::LaneExecutablePayloadV1::new_signed_with_reservations(
            network_id,
            context.epoch,
            proposal,
            vec![transaction.entrypoint().clone()],
            vec![reservation],
            vec![routing_plan],
            vec![None],
            producer,
            producer_key.private_key(),
        )
        .expect("construct valid autonomous candidate payload");
        crate::lane_consensus::autonomous_lane_payload_envelope(&payload, network_id, context.epoch)
            .expect("construct valid autonomous candidate envelope")
    }
    fn snapshot_parent_fixture() -> (
        State,
        wire::HeightContext,
        wire::SnapshotBootstrapAnchor,
        KeyPair,
    ) {
        snapshot_parent_fixture_with_world(2, World::new())
    }
    fn snapshot_parent_fixture_with_world(
        parent_height: u64,
        world: World,
    ) -> (
        State,
        wire::HeightContext,
        wire::SnapshotBootstrapAnchor,
        KeyPair,
    ) {
        let key = KeyPair::try_from_seed(vec![0xA7; 32], Algorithm::BlsNormal)
            .expect("deterministic validator key");
        let peer = PeerId::new(key.public_key().clone());
        let mut voters = vec![peer];
        voters.extend((0xA8_u8..=0xAA).map(|seed| {
            let voter = KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                .expect("deterministic validator key");
            PeerId::new(voter.public_key().clone())
        }));
        voters.sort();
        let topology = Topology::new(voters.clone());
        let kura = Kura::blank_kura_for_testing();
        let state = State::new_with_chain_and_network_id_for_testing(
            world,
            Arc::clone(&kura),
            LiveQueryStore::start_test(),
            ChainId::from("v2-candidate-snapshot-parent"),
            crate::sumeragi::synthetic_network_id("v2-candidate-test"),
        );
        let mut parent_hash = None;
        for height in 1..=parent_height {
            let valid = ValidBlock::new_dummy_and_modify_header(key.private_key(), |header| {
                header.set_height(NonZeroU64::new(height).expect("non-zero fixture height"));
                header.set_prev_block_hash(parent_hash);
                header.creation_time_ms = height;
                header.merkle_root = None;
            });
            let mut signed: SignedBlock = valid.into();
            {
                let outputs = crate::execution_output_test_support::structural_network_outputs(
                    &signed,
                    &[],
                    Vec::new(),
                );
                let fragments =
                    u64::try_from(outputs.iter().filter(|row| row.result().is_ok()).count())
                        .unwrap();
                signed.set_execution_outputs(
                    outputs,
                    fragments,
                    BTreeMap::new(),
                    Vec::new(),
                    AxtPolicySnapshot::default(),
                    Default::default(),
                    Vec::new(),
                    &crate::execution_output_test_support::structural_output_limits(),
                )
            }
            .expect("fixture parent carries the canonical empty AXT policy snapshot");

            let block = ValidBlock::new_unverified_for_tests(signed)
                .commit_unchecked()
                .unpack(|_| {});
            parent_hash = Some(block.as_ref().hash());
            let mut state_block = state.block(block.as_ref().header());
            let _events = state_block.apply_without_execution(&block, topology.as_ref().to_owned());
            state_block.commit().expect("commit fixture parent state");
        }
        let anchor = wire::SnapshotBootstrapAnchor {
            snapshot_height: parent_height,
            snapshot_block_hash: parent_hash.expect("fixture parent hash"),
            snapshot_block_creation_time_ms: parent_height,
            snapshot_state_hash: Hash::new(b"candidate snapshot state"),
        };
        let roster = voters
            .into_iter()
            .map(|validator| wire::ValidatorPower {
                validator,
                power: 1,
            })
            .collect::<Vec<_>>();
        let network_id = *state.network_id_ref();
        let (kagemusha_mint_finality_authorization, kagemusha_mint_finality_authority) =
            crate::kagemusha_v1_test_fixtures::mint_finality_genesis_authorization(
                network_id,
                u64::MAX,
                &roster,
            );
        let context = wire::HeightContext {
            network_id,
            protocol_version: wire::PROTOCOL_VERSION,
            height: parent_height + 1,
            epoch: 0,
            epoch_end_height: u64::MAX,
            next_epoch_snapshot: None,
            mode: wire::ConsensusMode::Permissioned,
            parent_commit_qc: None,
            snapshot_bootstrap: Some(anchor),
            quorum: wire::DualQuorum::from_roster(&roster).expect("fixture quorum"),
            roster,
            kagemusha_mint_finality_authorization,
            kagemusha_mint_finality_authority,
            nexus_amx_context_hash: Hash::new(b"candidate snapshot Nexus/AMX"),
            execution_policy_hash: iroha_crypto::Hash::new(b"test execution policy"),
            da_layout: wire::DataAvailabilityLayout {
                encoding: wire::PayloadEncoding::ReedSolomon16,
                chunk_size_bytes: 1024,
                data_shards: 1,
                parity_shards: 1,
                max_payload_size_bytes: 4096,
                max_chunk_count: 8,
            },
            leader_seed: [0x43; 32],
        };
        context.validate().expect("fixture snapshot context");
        let leader = &context.roster
            [usize::try_from(context.leader(0)).expect("fixture leader index")]
        .validator;
        let key = (0xA7_u8..=0xAA)
            .map(|seed| {
                KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                    .expect("deterministic validator key")
            })
            .find(|key| key.public_key() == leader.public_key())
            .expect("fixture roster contains its height-selected leader");
        (state, context, anchor, key)
    }
    #[test]
    fn native_source_wait_never_selects_ordinary_fallback() {
        let (_, context, _, _) = snapshot_parent_fixture();
        let pending = NativeLaneCandidatePreparation {
            work: None,
            waits: vec![
                crate::state::LaneDecisionGroupPreparationV1::MissingDecisions(vec![Hash::new(
                    b"original missing route",
                )]),
            ],
        };
        let original = pending.waits.as_ptr();
        let ordinary = record(71, "ordinary fallback forbidden", 0);
        let error = NativeCandidateWork(&pending)
            .prepare(&context, 0, &[ordinary.descriptor()])
            .unwrap_err();
        assert!(matches!(error, CandidateWorkError::Unavailable(unavailable)
            if unavailable.indices() == &BTreeSet::from([0])));
        assert_eq!(pending.waits.as_ptr(), original);
        assert_eq!(pending.waits.len(), 1);
        assert!(
            NativeCandidateWork(&pending)
                .prepare(&context, 0, &[])
                .unwrap()
                .native_lane_decisions
                .is_none()
        );
        let changed = NativeLaneCandidatePreparation {
            work: None,
            waits: vec![crate::state::LaneDecisionGroupPreparationV1::ObservationChanged],
        };
        assert!(matches!(
            NativeCandidateWork(&changed).prepare(&context, 0, &[]),
            Err(CandidateWorkError::Deferred(
                CandidateWorkDeferral::NativeLaneSource
            ))
        ));
    }

    #[test]
    fn candidate_route_preflight_defers_only_unreconciled_topology() {
        let (state, context, anchor, _) = snapshot_parent_fixture();
        let (_, time_source) = TimeSource::new_mock(Duration::from_millis(
            anchor.snapshot_block_creation_time_ms + 1,
        ));
        let assembler = V2CandidateAssembler::new(
            CandidateLimits::new(nonzero(2), nonzero(64 * 1024), nonzero(2))
                .expect("fixture candidate limits"),
            time_source,
        );
        let mut changed_topology = record(0x41, "changed-topology", 0);
        let coordinator = changed_topology.routing_plan.coordinator_route();
        changed_topology.routing_plan = RoutingPlan::native_amx(
            coordinator,
            vec![crate::queue::RouteLeg::new(
                coordinator,
                crate::queue::RouteLegRole::Participant,
            )],
        );
        let live = record(0x42, "live-topology", 1);
        let selected = vec![changed_topology, live];
        let creation_time = assembler.prospective_candidate_creation_time(
            0,
            CandidateParent::Snapshot(&anchor),
            &selected,
        );
        let ledger_time_ms = u64::try_from(creation_time.as_millis()).unwrap_or(u64::MAX);
        assert_eq!(
            assembler
                .unreconciled_execution_route_indices(&context, &state, &selected, ledger_time_ms,)
                .expect("route preflight"),
            BTreeSet::from([0]),
            "a changed Native-AMX topology must defer without hiding later live work"
        );
    }
    #[test]
    fn candidate_build_uses_the_exact_preflight_creation_time() {
        let (state, context, anchor, key) = snapshot_parent_fixture();
        let (time_handle, time_source) = TimeSource::new_mock(Duration::from_millis(
            anchor.snapshot_block_creation_time_ms + 1,
        ));
        let assembler = V2CandidateAssembler::new(
            CandidateLimits::new(nonzero(1), nonzero(64 * 1024), nonzero(1))
                .expect("fixture candidate limits"),
            time_source,
        );
        let selected = vec![record(0x43, "frozen-candidate-time", 0)];
        let parent = CandidateParent::Snapshot(&anchor);
        let view = 0;
        let creation_time = assembler.prospective_candidate_creation_time(view, parent, &selected);
        time_handle.advance(Duration::from_secs(5));
        let tag = EventTag::new(
            context.height,
            view,
            crate::sumeragi::v2_core::Generation::new(0),
        );
        let local_validator = context.leader(view);
        let builder = assembler
            .prepare_block_builder(
                &context,
                tag,
                parent,
                &state,
                &CandidateAttachments::default(),
                &selected,
                &PreparedCandidateWork::single_route_batch(1),
                creation_time,
            )
            .expect("prepare with frozen preflight time");
        let expected = builder
            .canonical_proposal_wire_len(
                u64::from(local_validator),
                key.public_key().try_algorithm().unwrap(),
            )
            .unwrap();
        let (block, bytes, _) = assembler
            .sign_prepared_block(
                builder,
                &context,
                tag,
                local_validator,
                parent,
                &key,
                &selected,
                creation_time,
            )
            .expect("sign with frozen preflight time");
        assert_eq!(bytes.len(), expected);
        assert_eq!(block.header().creation_time(), creation_time);
    }
    fn complete_admission_for_carrier(
        state: &State,
        context: &wire::HeightContext,
        anchor: &wire::SnapshotBootstrapAnchor,
        seed: u8,
        body_bytes: usize,
    ) -> Vec<u8> {
        use crate::governance::manifest::{
            GovernanceRules, LaneManifestRegistry, LaneManifestStatus, ManifestValidatorBinding,
        };
        use crate::queue::{QueuePlanAdmissionContextV1, QueuePlanRouteIncarnationV1};
        use crate::torii_proxy::{
            QueuePlanAdmissionAttestationV1, QueuePlanAdmissionCertificateV1,
            new_queue_plan_admission_binding, queue_plan_admission_attestation_signing_bytes_v1,
        };
        use iroha_data_model::{
            IntoKeyValue, Registrable,
            account::Account,
            consensus::{ConsensusKeyId, ConsensusKeyRecord, ConsensusKeyRole, ConsensusKeyStatus},
        };
        let route = RoutingDecision::default();
        let nexus = state.nexus_snapshot();
        let lane = nexus
            .lane_catalog
            .lanes()
            .iter()
            .find(|lane| lane.id == route.lane_id)
            .expect("carrier admission route exists in the current catalog");
        assert_eq!(lane.dataspace_id, route.dataspace_id);
        // Global commit topology alone does not establish native route authority.
        // Install the exact four accounts/live keys/PoPs and manifest bindings.
        // The manifest must retain the actual catalog identity: a made-up
        // governance policy is discarded by the scoped runtime rebind.
        let keys = (0xA7_u8..=0xAA)
            .map(|tag| KeyPair::try_from_seed(vec![tag; 32], Algorithm::BlsNormal).unwrap())
            .collect::<Vec<_>>();
        let mut world = state.world.block();
        for (index, key) in keys.iter().enumerate() {
            let validator = AccountId::new(key.public_key().clone());
            if world.accounts.get(&validator).is_none() {
                let (account_id, account_value) = Account::new(validator.clone())
                    .build(&validator)
                    .into_key_value();
                world.accounts.insert(account_id, account_value);
            }
            let id = ConsensusKeyId::new(
                ConsensusKeyRole::Validator,
                format!("carrier-budget-{index}"),
            );
            let record = ConsensusKeyRecord {
                id: id.clone(),
                public_key: key.public_key().clone(),
                pop: Some(iroha_crypto::bls_normal_pop_prove(key.private_key()).unwrap()),
                activation_height: 0,
                expiry_height: None,
                replaces: None,
                status: ConsensusKeyStatus::Active,
            };
            world.consensus_keys.insert(id.clone(), record);
            world
                .consensus_keys_by_pk
                .insert(key.public_key().to_string(), vec![id]);
        }
        {
            let mut peers = world.peers_mut_for_testing().transaction();
            for key in &keys {
                let peer = PeerId::new(key.public_key().clone());
                if !peers.iter().any(|existing| existing == &peer) {
                    peers.push(peer);
                }
            }
            peers.apply();
        }
        world.commit();
        let validators = keys
            .iter()
            .map(|key| AccountId::new(key.public_key().clone()))
            .collect::<Vec<_>>();
        let validator_bindings = validators
            .iter()
            .zip(&keys)
            .map(|(validator, key)| ManifestValidatorBinding {
                validator: validator.clone(),
                peer_id: PeerId::new(key.public_key().clone()),
                torii_url: None,
            })
            .collect();
        state.install_lane_manifests(&Arc::new(LaneManifestRegistry::from_statuses(
            BTreeMap::from([(
                lane.id,
                LaneManifestStatus {
                    lane: lane.id,
                    alias: lane.alias.clone(),
                    dataspace: lane.dataspace_id,
                    visibility: lane.visibility,
                    storage: lane.storage,
                    governance: lane.governance.clone(),
                    manifest_path: Some(std::path::PathBuf::from(
                        "/tmp/carrier-budget-manifest.json",
                    )),
                    governance_rules: Some(GovernanceRules {
                        validators,
                        validator_bindings,
                        ..GovernanceRules::default()
                    }),
                    privacy_commitments: Vec::new(),
                },
            )]),
        )));
        let plan = RoutingPlan::single(route);
        let view = state.view();
        let scoped_manifest = view
            .lane_manifests
            .status(lane.id)
            .expect("the current scoped projection retains the carrier manifest");
        assert_eq!(scoped_manifest.alias, lane.alias);
        assert_eq!(scoped_manifest.dataspace, lane.dataspace_id);
        assert_eq!(scoped_manifest.governance, lane.governance);
        assert_eq!(scoped_manifest.visibility, lane.visibility);
        assert_eq!(scoped_manifest.storage, lane.storage);
        let scoped_rules = scoped_manifest
            .governance_rules
            .as_ref()
            .expect("the actual catalog rebind preserves all four validator bindings");
        assert_eq!(scoped_rules.validator_bindings.len(), 4);
        for binding in &scoped_rules.validator_bindings {
            assert!(view.world().accounts().get(&binding.validator).is_some());
            assert!(
                crate::state::live_consensus_key_pop_for_peer_on_lane(
                    view.world(),
                    &binding.peer_id,
                    context.height,
                    lane.id,
                )
                .is_some()
            );
        }
        let validators = crate::queue::queue_plan_authoritative_peers_in_view_at_height(
            &view,
            route,
            context.height,
        )
        .unwrap();
        assert_eq!(validators.len(), 4);
        assert_eq!(
            validators,
            context
                .roster
                .iter()
                .map(|member| member.validator.clone())
                .collect::<Vec<_>>(),
            "the current route authority must be the frozen four-validator committee"
        );
        let lane_incarnation = view
            .lane_incarnation_at_height(route.lane_id, context.height)
            .expect("the same current view owns the admission's lane incarnation");
        drop(view);
        let admission_context = QueuePlanAdmissionContextV1 {
            version: crate::queue::QUEUE_PLAN_ADMISSION_CONTEXT_VERSION_V1,
            authority_height: anchor.snapshot_height,
            proposal_height: context.height,
            predecessor_block_hash: Some(anchor.snapshot_block_hash),
            routing_plan_digest: plan.digest(),
            route_incarnations: vec![QueuePlanRouteIncarnationV1 {
                leg: plan.legs()[0],
                lane_incarnation,
                validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
                validator_set_hash: HashOf::new(&validators),
                validator_count: 4,
                durability_threshold: 2,
                validator_set: validators.clone(),
            }],
        };
        let key = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519).unwrap();
        let mut transaction = TransactionBuilder::new(
            context.network_id,
            AccountId::new(key.public_key().clone()),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        );
        transaction.set_creation_time(Duration::from_millis(u64::from(seed)));
        let entrypoint = TransactionEntrypoint::External(
            transaction
                .with_instructions([iroha_data_model::isi::Log::new(
                    iroha_data_model::Level::INFO,
                    "x".repeat(body_bytes),
                )])
                .with_admission_intent(TransactionAdmissionIntent::QueuePlanSynced)
                .sign(key.private_key()),
        );
        let binding = new_queue_plan_admission_binding(
            &context.network_id,
            &entrypoint,
            &plan,
            admission_context,
            u64::from(seed) + 100,
        )
        .unwrap();
        let keys = (0xA7_u8..=0xAA)
            .map(|tag| KeyPair::try_from_seed(vec![tag; 32], Algorithm::BlsNormal).unwrap())
            .collect::<Vec<_>>();
        let attestations = validators
            .iter()
            .take(2)
            .enumerate()
            .map(|(index, validator)| {
                let key = keys
                    .iter()
                    .find(|key| key.public_key() == validator.public_key())
                    .unwrap();
                let index = u16::try_from(index).unwrap();
                let preimage = queue_plan_admission_attestation_signing_bytes_v1(
                    binding.canonical_hash(),
                    index,
                )
                .unwrap();
                QueuePlanAdmissionAttestationV1 {
                    version: crate::torii_proxy::QUEUE_PLAN_ADMISSION_ATTESTATION_VERSION_V1,
                    validator_index: index,
                    signature: iroha_crypto::Signature::try_new(key.private_key(), &preimage)
                        .unwrap(),
                }
            })
            .collect();
        let input = iroha_data_model::block::lane_admission::LaneAdmittedInputV1 {
            entrypoint,
            certificate: QueuePlanAdmissionCertificateV1 {
                version: crate::torii_proxy::QUEUE_PLAN_ADMISSION_CERTIFICATE_VERSION_V1,
                binding,
                attestations,
            },
        };
        let bytes = norito::encode_canonical(&input).unwrap();
        crate::torii_proxy::decode_and_validate_lane_admitted_input_v1(&context.network_id, &bytes)
            .unwrap();
        bytes
    }

    #[test]
    fn undersized_local_carrier_is_rejected_and_supported_capacity_builds_admission() {
        // This is the existing four-authority snapshot/unit fixture, not a
        // signed-genesis or daemon-startup qualification. Keep canonical RS16
        // geometry unchanged: authenticated startup must reject the smaller local capacity.
        let (state, mut context, anchor, key) = snapshot_parent_fixture();
        context.da_layout = wire::recommended_data_availability_layout();
        context
            .validate()
            .expect("canonical recommended RS16 context");
        let original_context = context.clone();
        let mut configuration = iroha_config::parameters::actual::Sumeragi::default();
        configuration.block.max_payload_bytes = nonzero(512 * 1024);
        configuration.limits.autonomous_carrier_headroom_bytes = nonzero(64 * 1024);
        let config = configuration
            .v2_config(Duration::from_secs(1), context.mode)
            .expect("structural configuration validation precedes authenticated layout validation");
        assert_eq!(config.limits.max_payload_bytes, 512 * 1024);
        assert_eq!(config.limits.autonomous_carrier_headroom_bytes, 64 * 1024);

        let input = complete_admission_for_carrier(&state, &context, &anchor, 0x45, 800 * 1024);
        let checked = crate::torii_proxy::decode_and_validate_lane_admitted_input_v1(
            &context.network_id,
            &input,
        )
        .expect("genuine complete input satisfies protocol size and exact quorum checks");
        // These are the same sizing API and bound used by Torii's pre-dispatch
        // complete-input check. No Torii handler or admission promise is fabricated.
        let maximum_input_bytes = crate::torii_proxy::maximum_lane_admitted_input_encoded_len_v1(
            checked.entrypoint(),
            &checked.input().certificate.binding,
        )
        .unwrap();
        assert!(input.len() <= maximum_input_bytes);
        assert!(maximum_input_bytes <= iroha_data_model::block::MAX_QUEUE_PLAN_ADMISSION_BYTES);
        let configured_payload = usize::try_from(config.limits.max_payload_bytes).unwrap();
        assert!(input.len() > configured_payload);
        assert!(input.len() < usize::try_from(context.da_layout.max_payload_size_bytes).unwrap());
        state
            .kura()
            .persist_pending_queue_plan_admission_certificate(&input)
            .unwrap();
        let durable_before = state
            .kura()
            .pending_queue_plan_admission_certificates()
            .unwrap();
        assert_eq!(durable_before, vec![(Hash::new(&input), input.clone())]);
        let generation_before = state.state_view_generation();
        let parent_before = state.latest_block_hash_fast();

        // The actual recovered-layout check now refuses this local resource
        // configuration before the runner can publish capacity or readiness.
        assert!(
            super::super::admission_capacity::require_local_payload_capacity(
                context.da_layout,
                &config,
            )
            .is_err()
        );
        configuration.block.max_payload_bytes =
            nonzero(usize::try_from(context.da_layout.max_payload_size_bytes).unwrap());
        let config = configuration
            .v2_config(Duration::from_secs(1), context.mode)
            .unwrap();
        super::super::admission_capacity::require_local_payload_capacity(
            context.da_layout,
            &config,
        )
        .unwrap();
        let effective_payload = usize::try_from(context.da_layout.max_payload_size_bytes).unwrap();
        let limits = CandidateLimits::new(
            nonzero(usize::try_from(config.limits.max_transactions).unwrap()),
            nonzero(effective_payload),
            nonzero(usize::try_from(config.limits.max_queue_scan).unwrap()),
        )
        .unwrap();
        let (_, time_source) = TimeSource::new_mock(Duration::from_millis(1000));
        let queue = Arc::new(Queue::test(
            iroha_config::parameters::actual::Queue::default(),
            &time_source,
        ));
        let assembler = V2CandidateAssembler::new(limits, time_source);
        let guard = ConsensusOutputGuard::isolated();
        let tag = EventTag::new(
            context.height,
            0,
            crate::sumeragi::v2_core::Generation::new(0),
        );
        let local = context.leader(0);
        assert_eq!(
            key.public_key(),
            context.roster[local as usize].validator.public_key()
        );
        for _ in 0..2 {
            let result = assembler.assemble(CandidateRequest {
                context: &context,
                directive: LocalProposalDirective::for_test(tag, local, None, None, None),
                local_validator: local,
                parent: CandidateParent::Snapshot(&anchor),
                state: &state,
                queue: &queue,
                key_pair: &key,
                output_guard: &guard,
                attachments: CandidateAttachments {
                    queue_plan_admissions: vec![input.clone()],
                    ..CandidateAttachments::default()
                },
                work_provider: SingleRouteWorkProvider,
            });
            let CandidateAssemblyOutcome::Assembled(candidate) = result.unwrap() else {
                panic!(
                    "the exact durable input must build once local capacity covers the signed envelope"
                );
            };
            assert!(candidate.canonical_wire.len() <= effective_payload);
            assert_eq!(
                candidate
                    .block
                    .execution_context()
                    .unwrap()
                    .queue_plan_admissions(),
                &[input.clone()]
            );
            assert!(!guard.restart_required());
            assert_eq!(context, original_context);
            assert_eq!(state.state_view_generation(), generation_before);
            assert_eq!(state.latest_block_hash_fast(), parent_before);
            assert_eq!(
                state
                    .kura()
                    .pending_queue_plan_admission_certificates()
                    .unwrap(),
                durable_before,
                "assembly and exact retry preserve the original durable complete input until commit"
            );
        }
    }

    fn retained_candidate_evidence(
        state: &State,
    ) -> Vec<iroha_data_model::block::consensus::SumeragiV2EquivocationEvidence> {
        use super::super::evidence::{retain_sumeragi_v2_equivocation, validate_v2_equivocation};
        let (_, context, _, _) = snapshot_parent_fixture_with_world(1, World::new());
        let mut keys = (0xA7_u8..=0xAA)
            .map(|seed| KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal).unwrap())
            .collect::<Vec<_>>();
        keys.sort_by(|a, b| a.public_key().cmp(b.public_key()));
        let proofs = keys
            .iter()
            .map(|key| iroha_crypto::bls_normal_pop_prove(key.private_key()).unwrap())
            .collect::<Vec<_>>();
        let round = wire::ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view: 0,
        };
        let mut evidence = Vec::new();
        for (signer, key) in keys.iter().enumerate() {
            let vote = |seed| {
                let mut vote = wire::Vote {
                    round,
                    proposal_round: round,
                    phase: wire::GlobalPhase::Prepare,
                    subject: wire::BlockSubject {
                        parent_block_hash: Some(
                            context.snapshot_bootstrap.unwrap().snapshot_block_hash,
                        ),
                        block_hash: HashOf::from_untyped_unchecked(Hash::new([seed])),
                        payload_hash: Hash::new([seed, 1]),
                    },
                    execution_commitment:
                        wire::ExecutionCommitment::without_kagemusha_top_ups_or_merge_carrier(
                            Hash::new(b"parent"),
                            Hash::new([seed]),
                            Hash::new(b"writes"),
                            1,
                            Hash::new(b"wire"),
                        ),
                    signer: signer as u32,
                    signature: Vec::new(),
                };
                vote.signature =
                    iroha_crypto::Signature::try_new(key.private_key(), &vote.signature_preimage())
                        .unwrap()
                        .payload()
                        .to_vec();
                vote
            };
            let conflict = wire::SumeragiV2Equivocation::PhaseVote {
                first: vote(41),
                second: vote(42),
            };
            assert!(
                retain_sumeragi_v2_equivocation(state, &context, &proofs, conflict.clone())
                    .unwrap()
            );
            evidence.push(
                super::super::evidence::canonicalize_v2_equivocation_evidence(
                    &iroha_data_model::block::consensus::SumeragiV2EquivocationEvidence {
                        context: context.clone(),
                        proofs_of_possession: proofs.clone(),
                        conflict,
                    },
                ),
            );
        }
        evidence.sort_by_key(super::super::evidence::v2_evidence_admission_key);
        assert_eq!(evidence.len(), 4);
        for proof in &evidence {
            validate_v2_equivocation(proof).unwrap();
        }
        evidence
    }

    #[test]
    fn optional_evidence_and_complete_admissions_alternate_exact_carrier_opportunity() {
        for parent_height in [2, 3] {
            let (state, mut context, anchor, key) =
                snapshot_parent_fixture_with_world(parent_height, World::new());
            let proofs = retained_candidate_evidence(&state);
            let input = complete_admission_for_carrier(&state, &context, &anchor, 0x54, 1024);
            state
                .kura()
                .persist_pending_queue_plan_admission_certificate(&input)
                .unwrap();
            let effects = NposConsensusEffects {
                v2_evidence_admissions: proofs.clone(),
                ..Default::default()
            };
            let (_, time_source) = TimeSource::new_mock(Duration::from_millis(1000));
            let queue = Arc::new(Queue::test(
                iroha_config::parameters::actual::Queue::default(),
                &time_source,
            ));
            let assembler = V2CandidateAssembler::new(
                CandidateLimits::new(nonzero(8), nonzero(1024 * 1024), nonzero(8)).unwrap(),
                time_source,
            );
            let tag = EventTag::new(
                context.height,
                0,
                crate::sumeragi::v2_core::Generation::new(0),
            );
            let size = |attachments: &CandidateAttachments| {
                assembler
                    .prepare_block_builder(
                        &context,
                        tag,
                        CandidateParent::Snapshot(&anchor),
                        &state,
                        attachments,
                        &[],
                        &PreparedCandidateWork::default(),
                        Duration::from_millis(1000),
                    )
                    .unwrap()
                    .canonical_proposal_wire_len(u64::from(context.leader(0)), Algorithm::BlsNormal)
                    .unwrap()
            };
            let admission_only = CandidateAttachments {
                queue_plan_admissions: vec![input.clone()],
                ..Default::default()
            };
            let first_proof = CandidateAttachments {
                npos_consensus_effects: npos_effects_prefix(&Some(effects.clone()), 1),
                ..Default::default()
            };
            let limit = size(&admission_only).max(size(&first_proof));
            let attachments = CandidateAttachments {
                npos_consensus_effects: Some(effects),
                ..admission_only
            };
            assert!(size(&attachments) > limit);
            context.da_layout.max_payload_size_bytes = limit as u64;
            context.da_layout.max_chunk_count = 128;
            context.validate().unwrap();
            let guard = ConsensusOutputGuard::isolated();
            for view in [0, context.roster.len() as u64] {
                let tag = EventTag::new(
                    context.height,
                    view,
                    crate::sumeragi::v2_core::Generation::new(0),
                );
                let local = context.leader(view);
                assert_eq!(
                    key.public_key(),
                    context.roster[local as usize].validator.public_key()
                );
                let outcome = assembler
                    .assemble(CandidateRequest {
                        context: &context,
                        directive: LocalProposalDirective::for_test(tag, local, None, None, None),
                        local_validator: local,
                        parent: CandidateParent::Snapshot(&anchor),
                        state: &state,
                        queue: &queue,
                        key_pair: &key,
                        output_guard: &guard,
                        attachments: attachments.clone(),
                        work_provider: SingleRouteWorkProvider,
                    })
                    .unwrap();
                let CandidateAssemblyOutcome::Assembled(candidate) = outcome else {
                    panic!("a useful class must fit");
                };
                assert!(candidate.canonical_wire.len() <= limit);
                let admitted = candidate
                    .block
                    .execution_context()
                    .map_or(0, |c| c.queue_plan_admissions().len());
                let evidence = candidate
                    .block
                    .npos_consensus_effects()
                    .map_or(&[][..], |e| e.v2_evidence_admissions.as_slice());
                if candidate_economic_work_first(context.height) {
                    assert_eq!(admitted, 1, "economic priority must survive view changes");
                    assert!(evidence.is_empty());
                } else {
                    assert_eq!(admitted, 0);
                    assert!(
                        !evidence.is_empty(),
                        "evidence has its own nonempty opportunity"
                    );
                    assert!(evidence.len() < proofs.len());
                    assert_eq!(evidence, &proofs[..evidence.len()]);
                }
                assert_eq!(
                    candidate.scan_report.evidence_deferred,
                    proofs.len() - evidence.len()
                );
                assert!(!guard.restart_required());
                assert_eq!(
                    state
                        .sumeragi_v2_pending_evidence
                        .lock()
                        .keys()
                        .copied()
                        .collect::<Vec<_>>(),
                    proofs
                        .iter()
                        .map(super::super::evidence::v2_evidence_admission_key)
                        .collect::<Vec<_>>()
                );
                assert_eq!(
                    state
                        .kura()
                        .pending_queue_plan_admission_certificate(Hash::new(&input))
                        .unwrap(),
                    Some(input.clone())
                );
            }
        }
    }

    #[test]
    fn evidence_prefix_fitting_preserves_mandatory_effects_and_exact_signed_size() {
        use iroha_data_model::consensus::{
            NposMarkConsensusEvidenceAppliedAction, NposPenaltyAction,
        };
        let (state, context, anchor, key) = snapshot_parent_fixture();
        let mut effects = pulse_only_effects_fixture();
        effects.v2_evidence_admissions = retained_candidate_evidence(&state);
        effects
            .penalty_actions
            .push(NposPenaltyAction::MarkConsensusEvidenceApplied(
                NposMarkConsensusEvidenceAppliedAction {
                    evidence_key: Hash::new(b"mandatory marker"),
                    height: context.height,
                },
            ));
        let (_, time_source) = TimeSource::new_mock(Duration::from_millis(1000));
        let assembler = V2CandidateAssembler::new(
            CandidateLimits::new(nonzero(8), nonzero(1024 * 1024), nonzero(8)).unwrap(),
            time_source,
        );
        let tag = EventTag::new(
            context.height,
            0,
            crate::sumeragi::v2_core::Generation::new(0),
        );
        let builder = assembler
            .prepare_block_builder(
                &context,
                tag,
                CandidateParent::Snapshot(&anchor),
                &state,
                &CandidateAttachments {
                    npos_consensus_effects: Some(effects.clone()),
                    ..Default::default()
                },
                &[],
                &PreparedCandidateWork::default(),
                Duration::from_millis(1000),
            )
            .unwrap();
        let original = Some(effects.clone());
        let mut layout = context.da_layout;
        layout.max_chunk_count = 1024;
        for count in 0..=effects.v2_evidence_admissions.len() {
            let exact = builder
                .clone()
                .with_npos_consensus_effects(npos_effects_prefix(&original, count))
                .canonical_proposal_wire_len(0, Algorithm::BlsNormal)
                .unwrap();
            for (limit, expected) in [(exact, count), (exact - 1, count.saturating_sub(1))] {
                let (fitted, actual) = fit_evidence_prefix(
                    builder.clone(),
                    &original,
                    0,
                    Algorithm::BlsNormal,
                    layout,
                    limit,
                )
                .unwrap();
                assert_eq!(actual, expected);
                let block: SignedBlock = fitted
                    .try_sign_with_index(key.private_key(), 0)
                    .unwrap()
                    .unpack(|_| {})
                    .into();
                let retained = block.npos_consensus_effects().unwrap();
                assert_eq!(
                    retained.finalized_global_beacon_pulse,
                    effects.finalized_global_beacon_pulse
                );
                assert_eq!(retained.penalty_actions, effects.penalty_actions);
                assert_eq!(
                    retained.v2_evidence_admissions,
                    effects.v2_evidence_admissions[..expected]
                );
                assert_eq!(
                    block.header().npos_effects_hash(),
                    Some(HashOf::new(retained))
                );
                if limit == exact {
                    assert_eq!(block.encode_wire().unwrap().len(), exact);
                }
            }
        }
        let base = builder
            .clone()
            .with_npos_consensus_effects(npos_effects_prefix(&original, 1));
        let one = base
            .canonical_proposal_wire_len(0, Algorithm::BlsNormal)
            .unwrap();
        let chunks = encoded_chunk_count(layout, one).unwrap();
        layout.max_chunk_count = (chunks - 1) as u32;
        assert_eq!(
            fit_evidence_prefix(
                builder,
                &original,
                0,
                Algorithm::BlsNormal,
                layout,
                usize::MAX
            )
            .unwrap()
            .1,
            0
        );
    }

    #[test]
    fn unfit_evidence_does_not_sign_a_pulse_only_carrier() {
        let (state, mut context, anchor, key) = snapshot_parent_fixture();
        let mut effects = pulse_only_effects_fixture();
        effects.v2_evidence_admissions = retained_candidate_evidence(&state);
        let (_, time_source) = TimeSource::new_mock(Duration::from_millis(1000));
        let assembler = V2CandidateAssembler::new(
            CandidateLimits::new(nonzero(8), nonzero(1024 * 1024), nonzero(8)).unwrap(),
            time_source.clone(),
        );
        let queue = Arc::new(Queue::test(
            iroha_config::parameters::actual::Queue::default(),
            &time_source,
        ));
        let tag = EventTag::new(
            context.height,
            0,
            crate::sumeragi::v2_core::Generation::new(0),
        );
        let local = context.leader(0);
        let first_proof = assembler
            .prepare_block_builder(
                &context,
                tag,
                CandidateParent::Snapshot(&anchor),
                &state,
                &CandidateAttachments {
                    npos_consensus_effects: npos_effects_prefix(&Some(effects.clone()), 1),
                    ..Default::default()
                },
                &[],
                &PreparedCandidateWork::default(),
                Duration::from_millis(1000),
            )
            .unwrap()
            .canonical_proposal_wire_len(u64::from(local), Algorithm::BlsNormal)
            .unwrap();
        context.da_layout.max_payload_size_bytes = (first_proof - 1) as u64;
        context.da_layout.max_chunk_count = 128;
        context.validate().unwrap();
        let guard = ConsensusOutputGuard::isolated();
        let outcome = assembler.assemble(CandidateRequest {
            context: &context,
            directive: LocalProposalDirective::for_test(tag, local, None, None, None),
            local_validator: local,
            parent: CandidateParent::Snapshot(&anchor),
            state: &state,
            queue: &queue,
            key_pair: &key,
            output_guard: &guard,
            attachments: CandidateAttachments {
                npos_consensus_effects: Some(effects.clone()),
                ..Default::default()
            },
            work_provider: SingleRouteWorkProvider,
        });
        assert!(
            matches!(outcome.unwrap(), CandidateAssemblyOutcome::WorkDeferred { report, .. } if report.evidence_deferred == 4)
        );
        assert!(!guard.restart_required());
        assert_eq!(state.sumeragi_v2_pending_evidence.lock().len(), 4);
        // A later ordinary arrival must still be serviced by the same height
        // owner after the oversized optional proof caused a snapshot deferral.
        let account_key = KeyPair::try_from_seed(vec![0x71; 32], Algorithm::Ed25519).unwrap();
        let authority = AccountId::new(account_key.public_key().clone());
        let mut world = state.world.block();
        world.accounts.insert(
            authority.clone(),
            iroha_data_model::account::AccountValue::new(
                iroha_data_model::account::AccountDetails::default(),
            ),
        );
        world.commit();
        let transaction = TransactionBuilder::new_with_time_source(
            *state.network_id_ref(),
            authority,
            &time_source,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_admission_intent(TransactionAdmissionIntent::Ordinary)
        .sign(account_key.private_key());
        let transaction = AcceptedTransaction::new_unchecked(Cow::Owned(transaction));
        let hash = transaction.hash_as_entrypoint();
        queue.push(transaction, state.view()).unwrap();
        let outcome = assembler
            .assemble(CandidateRequest {
                context: &context,
                directive: LocalProposalDirective::for_test(tag, local, None, None, None),
                local_validator: local,
                parent: CandidateParent::Snapshot(&anchor),
                state: &state,
                queue: &queue,
                key_pair: &key,
                output_guard: &guard,
                attachments: CandidateAttachments {
                    npos_consensus_effects: Some(effects.clone()),
                    ..Default::default()
                },
                work_provider: SingleRouteWorkProvider,
            })
            .unwrap();
        let CandidateAssemblyOutcome::Assembled(candidate) = outcome else {
            panic!("ordinary arrival must remain serviceable");
        };
        assert_eq!(
            candidate
                .block
                .external_entrypoints_cloned()
                .map(|e| e.hash())
                .collect::<Vec<_>>(),
            vec![hash]
        );
        assert!(
            candidate
                .block
                .npos_consensus_effects()
                .unwrap()
                .v2_evidence_admissions
                .is_empty()
        );
        assert_eq!(
            candidate
                .block
                .npos_consensus_effects()
                .unwrap()
                .finalized_global_beacon_pulse,
            effects.finalized_global_beacon_pulse
        );
        assert_eq!(state.sumeragi_v2_pending_evidence.lock().len(), 4);
        assert!(!guard.restart_required());
    }

    #[test]
    fn complete_admission_batch_fits_two_mib_carrier_and_preserves_deferred_custody() {
        let (state, mut context, anchor, key) = snapshot_parent_fixture();
        context.da_layout.max_payload_size_bytes = 2 * 1024 * 1024;
        context.da_layout.chunk_size_bytes = 8192;
        context.da_layout.max_chunk_count = 512;
        context.validate().unwrap();
        let mut inputs = (0x41..=0x43)
            .map(|seed| complete_admission_for_carrier(&state, &context, &anchor, seed, 800 * 1024))
            .collect::<Vec<_>>();
        inputs.sort_by_cached_key(|input| {
            crate::torii_proxy::decode_and_validate_lane_admitted_input_v1(
                &context.network_id,
                input,
            )
            .unwrap()
            .certificate()
            .registry_key
            .clone()
        });
        assert!(inputs.iter().all(|input| input.len() <= iroha_data_model::block::MAX_QUEUE_PLAN_ADMISSION_BYTES));
        for input in &inputs {
            state
                .kura()
                .persist_pending_queue_plan_admission_certificate(input)
                .unwrap();
        }
        let (_, time_source) = TimeSource::new_mock(Duration::from_millis(1000));
        let queue = Arc::new(Queue::test(
            iroha_config::parameters::actual::Queue::default(),
            &time_source,
        ));
        let assembler = V2CandidateAssembler::new(
            CandidateLimits::new(nonzero(8), nonzero(2 * 1024 * 1024), nonzero(8)).unwrap(),
            time_source,
        );
        let guard = ConsensusOutputGuard::isolated();
        let assemble = |admissions: Vec<Vec<u8>>, view| {
            let tag = EventTag::new(
                context.height,
                view,
                crate::sumeragi::v2_core::Generation::new(0),
            );
            let local = context.leader(view);
            assert_eq!(
                key.public_key(),
                context.roster[local as usize].validator.public_key()
            );
            assembler
                .assemble(CandidateRequest {
                    context: &context,
                    directive: LocalProposalDirective::for_test(tag, local, None, None, None),
                    local_validator: local,
                    parent: CandidateParent::Snapshot(&anchor),
                    state: &state,
                    queue: &queue,
                    key_pair: &key,
                    output_guard: &guard,
                    attachments: CandidateAttachments {
                        queue_plan_admissions: admissions,
                        ..CandidateAttachments::default()
                    },
                    work_provider: SingleRouteWorkProvider,
                })
                .unwrap()
        };
        let CandidateAssemblyOutcome::Assembled(candidate) = assemble(inputs.clone(), 0) else {
            panic!("complete inputs must make progress");
        };
        assert_eq!(
            candidate
                .block()
                .execution_context()
                .unwrap()
                .queue_plan_admissions(),
            &inputs[..2]
        );
        assert_eq!(candidate.scan_report().admission_deferred, 1);
        assert!(candidate.canonical_wire.len() <= 2 * 1024 * 1024);
        assert_eq!(candidate.block().external_entrypoints_cloned().count(), 0);
        assert!(!guard.restart_required());
        for input in &inputs {
            assert_eq!(
                state
                    .kura()
                    .pending_queue_plan_admission_certificate(Hash::new(input))
                    .unwrap()
                    .as_deref(),
                Some(input.as_slice()),
                "selection alone cannot retire durable complete inputs"
            );
        }
        drop(candidate);
        let CandidateAssemblyOutcome::Assembled(next) =
            assemble(vec![inputs[2].clone()], context.roster.len() as u64)
        else {
            panic!("deferred input must remain selectable");
        };
        assert_eq!(
            next.block()
                .execution_context()
                .unwrap()
                .queue_plan_admissions(),
            &inputs[2..]
        );
        assert_eq!(next.scan_report().admission_deferred, 0);
    }

    #[test]
    fn admission_carrier_limit_includes_framing_and_refuses_before_signing() {
        let (state, mut context, anchor, key) = snapshot_parent_fixture();
        let input = complete_admission_for_carrier(&state, &context, &anchor, 0x44, 12 * 1024);
        let (_, time_source) = TimeSource::new_mock(Duration::from_millis(1000));
        let queue = Arc::new(Queue::test(
            iroha_config::parameters::actual::Queue::default(),
            &time_source,
        ));
        let guard = ConsensusOutputGuard::isolated();
        let tag = EventTag::new(
            context.height,
            0,
            crate::sumeragi::v2_core::Generation::new(0),
        );
        let local = context.leader(0);
        let attachments = CandidateAttachments {
            queue_plan_admissions: vec![input.clone()],
            ..CandidateAttachments::default()
        };
        let assembler = V2CandidateAssembler::new(
            CandidateLimits::new(nonzero(8), nonzero(64 * 1024), nonzero(8)).unwrap(),
            time_source,
        );
        let builder = assembler
            .prepare_block_builder(
                &context,
                tag,
                CandidateParent::Snapshot(&anchor),
                &state,
                &attachments,
                &[],
                &PreparedCandidateWork::single_route_batch(0),
                Duration::from_millis(1000),
            )
            .unwrap();
        let size = builder
            .canonical_proposal_wire_len(u64::from(local), Algorithm::BlsNormal)
            .unwrap();
        assert!(
            size > input.len(),
            "the full carrier owns real metadata and framing"
        );
        context.da_layout.max_chunk_count = 128;
        for limit in [size - 1, size] {
            context.da_layout.max_payload_size_bytes = limit as u64;
            context.validate().unwrap();
            let result = assembler.assemble(CandidateRequest {
                context: &context,
                directive: LocalProposalDirective::for_test(tag, local, None, None, None),
                local_validator: local,
                parent: CandidateParent::Snapshot(&anchor),
                state: &state,
                queue: &queue,
                key_pair: &key,
                output_guard: &guard,
                attachments: attachments.clone(),
                work_provider: SingleRouteWorkProvider,
            });
            if limit < size {
                assert!(matches!(
                    result,
                    Err(CandidateError::ProposalFramingExceedsPayloadLimits { encoded_bytes, max_bytes, .. })
                        if encoded_bytes == size && max_bytes == size - 1
                ));
                assert!(
                    !guard.restart_required(),
                    "a sizing refusal must occur before fail-stop signing begins"
                );
            } else {
                let CandidateAssemblyOutcome::Assembled(candidate) = result.unwrap() else {
                    panic!("exact full carrier limit must fit");
                };
                assert_eq!(candidate.canonical_wire.len(), size);
                assert_eq!(
                    candidate
                        .block()
                        .execution_context()
                        .unwrap()
                        .queue_plan_admissions(),
                    &[input.clone()]
                );
            }
        }
    }

    #[test]
    fn proposal_sizing_matches_real_signatures_and_prefix_preserves_metadata() {
        let (state, context, anchor, _) = snapshot_parent_fixture();
        let (_, time_source) = TimeSource::new_mock(Duration::from_millis(1000));
        let assembler = V2CandidateAssembler::new(
            CandidateLimits::new(nonzero(8), nonzero(64 * 1024), nonzero(8)).unwrap(),
            time_source,
        );
        let selected = vec![record(0x48, "actual external", 0)];
        let tag = EventTag::new(
            context.height,
            0,
            crate::sumeragi::v2_core::Generation::new(0),
        );
        let attachments = CandidateAttachments {
            queue_plan_admissions: vec![vec![1; 127], vec![2; 128]],
            sccp_commitment_root: Some([0x99; 32]),
            ..CandidateAttachments::default()
        };
        let builder = assembler
            .prepare_block_builder(
                &context,
                tag,
                CandidateParent::Snapshot(&anchor),
                &state,
                &attachments,
                &selected,
                &PreparedCandidateWork::single_route_batch(1),
                Duration::from_millis(1000),
            )
            .unwrap();
        assert!(
            builder
                .clone()
                .retain_queue_plan_admission_prefix(3)
                .is_err()
        );
        for algorithm in [
            Algorithm::Ed25519,
            Algorithm::Secp256k1,
            Algorithm::BlsNormal,
            Algorithm::BlsSmall,
            Algorithm::MlDsa,
        ] {
            let key = KeyPair::try_from_seed(vec![0x46; 32], algorithm).unwrap();
            for count in [0, 1, 2] {
                let trial = builder
                    .clone()
                    .retain_queue_plan_admission_prefix(count)
                    .unwrap();
                let expected = trial.canonical_proposal_wire_len(257, algorithm).unwrap();
                let block: SignedBlock = trial
                    .try_sign_with_index(key.private_key(), 257)
                    .unwrap()
                    .unpack(|_| {})
                    .into();
                assert_eq!(block.encode_wire().unwrap().len(), expected);
                assert_eq!(
                    block.execution_context().unwrap().queue_plan_admissions(),
                    &attachments.queue_plan_admissions[..count]
                );
                assert_eq!(block.external_entrypoints_cloned().count(), 1);
                assert_eq!(block.header().sccp_commitment_root(), Some([0x99; 32]));
                assert!(block.da_proof_policies().is_some());
            }
        }
        let raw = BlockBuilder::new(Vec::new()).chain_with_parent_hash(
            0,
            anchor.snapshot_height,
            anchor.snapshot_block_hash,
        );
        assert!(
            raw.canonical_proposal_wire_len(0, Algorithm::BlsNormal)
                .is_err()
        );
        assert!(raw.clone().retain_queue_plan_admission_prefix(0).is_ok());
        assert!(raw.retain_queue_plan_admission_prefix(1).is_err());
    }

    fn assemble_empty_snapshot_candidate(
        attachments: CandidateAttachments,
    ) -> CandidateAssemblyOutcome {
        let (state, context, anchor, key) = snapshot_parent_fixture();
        assemble_empty_snapshot_candidate_for_state(attachments, &state, context, anchor, key)
    }
    fn assemble_empty_snapshot_candidate_for_state(
        attachments: CandidateAttachments,
        state: &State,
        mut context: wire::HeightContext,
        anchor: wire::SnapshotBootstrapAnchor,
        key: KeyPair,
    ) -> CandidateAssemblyOutcome {
        context.da_layout.max_payload_size_bytes = 64 * 1024;
        context.da_layout.max_chunk_count = 128;
        context.validate().expect("expanded fixture DA limits");
        let (_, time_source) = TimeSource::new_mock(Duration::from_millis(
            anchor.snapshot_block_creation_time_ms + 1,
        ));
        let queue = Arc::new(Queue::test(
            iroha_config::parameters::actual::Queue::default(),
            &time_source,
        ));
        let output_guard = ConsensusOutputGuard::isolated();
        let tag = EventTag::new(
            context.height,
            0,
            crate::sumeragi::v2_core::Generation::new(0),
        );
        let local_validator = context.leader(tag.view());
        let directive = LocalProposalDirective::for_test(tag, local_validator, None, None, None);
        V2CandidateAssembler::new(
            CandidateLimits::new(nonzero(8), nonzero(64 * 1024), nonzero(8))
                .expect("fixture candidate limits"),
            time_source,
        )
        .assemble(CandidateRequest {
            context: &context,
            directive,
            local_validator,
            parent: CandidateParent::Snapshot(&anchor),
            state,
            queue: &queue,
            key_pair: &key,
            output_guard: &output_guard,
            attachments,
            work_provider: SingleRouteWorkProvider,
        })
        .expect("empty snapshot candidate assembly")
    }
    struct RecordingWorkErrorProvider<'a> {
        error: CandidateWorkError,
        observed: &'a std::cell::RefCell<Vec<Vec<HashOf<TransactionEntrypoint>>>>,
    }
    impl CandidateWorkProvider for RecordingWorkErrorProvider<'_> {
        fn prepare(
            &mut self,
            _context: &wire::HeightContext,
            _view: wire::View,
            candidates: &[CandidateDescriptor<'_>],
        ) -> Result<PreparedCandidateWork, CandidateWorkError> {
            let mut observed = self.observed.borrow_mut();
            assert!(
                observed.is_empty(),
                "a snapshot deferral or malformed/fatal error must end this assembly attempt"
            );
            observed.push(
                candidates
                    .iter()
                    .map(|candidate| candidate.entrypoint_hash())
                    .collect(),
            );
            Err(self.error.clone())
        }
    }

    fn assemble_recorded_work_error(
        error: CandidateWorkError,
        queued_count: u8,
        close_signing_gate: bool,
    ) -> Result<CandidateAssemblyOutcome, CandidateError> {
        let (_, time_source) = TimeSource::new_mock(Duration::from_millis(3));
        let mut world = World::new();
        let mut transactions = Vec::new();
        for offset in 0..queued_count {
            let key = KeyPair::try_from_seed(vec![0x71 + offset; 32], Algorithm::Ed25519)
                .expect("deterministic queued authority");
            let authority = AccountId::new(key.public_key().clone());
            world.accounts.insert(
                authority.clone(),
                iroha_data_model::account::AccountValue::new(
                    iroha_data_model::account::AccountDetails::default(),
                ),
            );
            let transaction = TransactionBuilder::new_with_time_source(
                crate::sumeragi::synthetic_network_id("v2-candidate-test"),
                authority,
                &time_source,
                iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
            )
            .with_admission_intent(TransactionAdmissionIntent::Ordinary)
            .sign(key.private_key());
            transactions.push(AcceptedTransaction::new_unchecked(Cow::Owned(transaction)));
        }
        let expected = transactions
            .iter()
            .map(AcceptedTransaction::hash_as_entrypoint)
            .collect::<Vec<_>>();
        let (state, mut context, anchor, key) = snapshot_parent_fixture_with_world(2, world);
        context.da_layout.max_payload_size_bytes = 64 * 1024;
        context.da_layout.max_chunk_count = 128;
        context.validate().expect("expanded fixture DA limits");
        let queue = Arc::new(Queue::test(
            iroha_config::parameters::actual::Queue::default(),
            &time_source,
        ));
        for transaction in transactions {
            queue
                .push(transaction, state.view())
                .expect("admit ordinary FIFO fixture");
        }
        let output_guard = ConsensusOutputGuard::isolated();
        if close_signing_gate {
            // A deliberate signing barrier: returning WorkDeferred must not require
            // a signing permit, even though attachments below are genuine body work.
            output_guard.activate_restart_required();
            assert!(output_guard.begin_fail_stop_operation().is_none());
        }
        let tag = EventTag::new(
            context.height,
            0,
            crate::sumeragi::v2_core::Generation::new(0),
        );
        let local_validator = context.leader(tag.view());
        let directive = LocalProposalDirective::for_test(tag, local_validator, None, None, None);
        let observed = std::cell::RefCell::new(Vec::new());
        let outcome = V2CandidateAssembler::new(
            CandidateLimits::new(nonzero(8), nonzero(64 * 1024), nonzero(8))
                .expect("fixture candidate limits"),
            time_source,
        )
        .assemble(CandidateRequest {
            context: &context,
            directive,
            local_validator,
            parent: CandidateParent::Snapshot(&anchor),
            state: &state,
            queue: &queue,
            key_pair: &key,
            output_guard: &output_guard,
            attachments: CandidateAttachments {
                time_trigger_clock_progress_required: true,
                ..CandidateAttachments::default()
            },
            work_provider: RecordingWorkErrorProvider {
                error,
                observed: &observed,
            },
        });
        assert_eq!(
            *observed.borrow(),
            vec![expected.clone()],
            "prepare sees one exact FIFO snapshot"
        );
        assert_eq!(
            queue.queued_len(),
            expected.len(),
            "deferral/error must retain every queued entry"
        );
        assert!(queue.live_lane_reservations().is_empty());
        assert!(!queue.transaction_selection_durability_faulted());
        // A fresh lease must recover the same complete prefix, proving that the
        // failed/deferred attempt neither removed entries nor leaked its selection owner.
        let state_view = state.view();
        let (retained, lease) = queue
            .bounded_pending_snapshot(&state_view, nonzero(8))
            .expect("selection lease is released at the attempt boundary");
        assert_eq!(
            retained
                .iter()
                .map(AcceptedTransaction::hash_as_entrypoint)
                .collect::<Vec<_>>(),
            expected
        );
        drop(lease);
        outcome
    }

    /// Pass an exact live-provider error through the real empty-snapshot assembler.
    pub(in crate::sumeragi) fn assert_empty_work_deferral_reaches_assembler(
        error: CandidateWorkError,
    ) {
        assert_eq!(
            error,
            CandidateWorkError::Deferred(CandidateWorkDeferral::MergeFrontier)
        );
        let outcome = assemble_recorded_work_error(error, 0, true)
            .expect("whole-snapshot deferral is retryable before signing");
        let CandidateAssemblyOutcome::WorkDeferred { report, reason } = outcome else {
            panic!("an empty provider deferral must remain a typed whole-snapshot outcome");
        };
        assert_eq!(reason, CandidateWorkDeferral::MergeFrontier);
        assert_eq!(report, CandidateScanReport::default());
    }

    #[test]
    fn empty_snapshot_provider_deferral_returns_once_without_signing() {
        assert_empty_work_deferral_reaches_assembler(CandidateWorkError::Deferred(
            CandidateWorkDeferral::MergeFrontier,
        ));
    }

    #[test]
    fn nonempty_snapshot_provider_deferral_preserves_fifo_and_selection_ownership() {
        let outcome = assemble_recorded_work_error(
            CandidateWorkError::Deferred(CandidateWorkDeferral::MergeFrontier),
            2,
            false,
        )
        .expect("nonempty snapshot deferral remains retryable");
        let CandidateAssemblyOutcome::WorkDeferred { report, reason } = outcome else {
            panic!("a deferred provider must not sign, remove or manufacture candidate work");
        };
        assert_eq!(reason, CandidateWorkDeferral::MergeFrontier);
        assert_eq!(report.inspected, 2);
        assert_eq!(report.routable, 2);
        assert_eq!(
            report.work_deferred, 0,
            "whole-snapshot deferral is not positional removal"
        );
        assert_eq!(report.selected, 0, "no final candidate was assembled");
    }

    #[test]
    fn positional_unavailability_rejects_empty_blank_and_out_of_range_sets() {
        for (queued_count, indices, reason, out_of_range) in [
            (0, BTreeSet::new(), "empty subset", false),
            (2, BTreeSet::new(), "empty subset", false),
            (2, BTreeSet::from([0]), " \t", false),
            (2, BTreeSet::from([2]), "outside batch", true),
        ] {
            let error = assemble_recorded_work_error(
                CandidateWorkError::Unavailable(CandidateWorkUnavailable::new(indices, reason)),
                queued_count,
                false,
            )
            .expect_err("malformed positional unavailability remains fatal");
            if out_of_range {
                assert!(matches!(error, CandidateError::UnavailableIndexOutOfRange));
            } else {
                assert!(matches!(error, CandidateError::MalformedUnavailableWork));
            }
        }
    }

    #[test]
    fn fatal_provider_errors_retain_their_exact_failure_scope() {
        for queued_count in [0, 2] {
            let failed = assemble_recorded_work_error(
                CandidateWorkError::Failed("certified lane storage failed".to_owned()),
                queued_count,
                false,
            )
            .expect_err("storage failure must not become a snapshot retry");
            assert!(
                matches!(failed, CandidateError::WorkPreparationFailed(reason) if reason == "certified lane storage failed")
            );
            let restart = assemble_recorded_work_error(
                CandidateWorkError::RestartRequired,
                queued_count,
                false,
            )
            .expect_err("a closed provider remains restart-required");
            assert!(matches!(restart, CandidateError::RestartRequired));
        }
    }

    #[test]
    fn proposal_work_gate_defers_idle_candidate() {
        let outcome = assemble_empty_snapshot_candidate(CandidateAttachments::default());
        let CandidateAssemblyOutcome::NoProposalWork(report) = outcome else {
            panic!("an idle height must not manufacture an empty candidate");
        };
        assert_eq!(report, CandidateScanReport::default());
    }
    #[test]
    fn pending_privacy_proposal_does_not_create_an_empty_candidate() {
        use iroha_data_model::privacy::{
            PrivacyProposedLifecycleV1, PrivacyProtocolIdV1, PrivacyProtocolLifecycleV1,
        };
        let mut world = World::new();
        let protocol = PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1;
        let proposal = crate::privacy_profiles::compiled_privacy_profile_v1(protocol)
            .expect("compiled private-note profile")
            .activation_record(PrivacyProtocolLifecycleV1::Proposed(
                PrivacyProposedLifecycleV1 {
                    proposed_at_height: 1,
                },
            ));
        let activation_key = crate::privacy_state::PrivacyActivationKeyV1::new(protocol);
        world.privacy_activations.insert(activation_key, proposal);
        let (state, context, anchor, key) = snapshot_parent_fixture_with_world(300, world);
        let header = BlockHeader::new(
            NonZeroU64::new(301).expect("successor height"),
            Some(anchor.snapshot_block_hash),
            None,
            301,
            0,
        );
        assert_eq!(
            state.deterministic_start_work_pending(&header).unwrap(),
            Some(false)
        );
        let outcome = assemble_empty_snapshot_candidate_for_state(
            CandidateAttachments::default(),
            &state,
            context,
            anchor,
            key,
        );
        assert!(
            matches!(outcome, CandidateAssemblyOutcome::NoProposalWork(_)),
            "a pending privacy proposal cannot manufacture block work"
        );
        assert_eq!(
            state.world.privacy_activations.view().get(&activation_key),
            Some(&proposal)
        );
    }
    #[test]
    fn scheduled_privacy_protocol_limits_are_proposal_work_at_exact_height() {
        use iroha_data_model::privacy::{
            PrivacyProposedLifecycleV1, PrivacyProtocolActivationLimitsV1, PrivacyProtocolIdV1,
            PrivacyProtocolLifecycleV1, PrivacyProtocolLimitsTighteningV1,
        };
        let mut world = World::new();
        let protocol = PrivacyProtocolIdV1::VeRangeTransparentRangeV1;
        let mut activation = crate::privacy_profiles::compiled_privacy_profile_v1(protocol)
            .expect("compiled VeRange profile")
            .activation_record(PrivacyProtocolLifecycleV1::Proposed(
                PrivacyProposedLifecycleV1 {
                    proposed_at_height: 1,
                },
            ));
        let mut next_limits = activation.protocol_limits;
        let PrivacyProtocolActivationLimitsV1::VeRangeTransparentRangeV1(ref mut limits) =
            next_limits
        else {
            unreachable!("VeRange fixture");
        };
        limits.max_aggregation_count -= 1;
        activation.pending_protocol_limits_tightening = Some(PrivacyProtocolLimitsTighteningV1 {
            scheduled_at_height: 1,
            effective_at_height: 301,
            next_limits,
        });
        let activation_key = crate::privacy_state::PrivacyActivationKeyV1::new(protocol);
        world.privacy_activations.insert(activation_key, activation);
        let (state, context, anchor, key) = snapshot_parent_fixture_with_world(300, world);
        let parent_hash = anchor.snapshot_block_hash;
        let topology = context
            .roster
            .iter()
            .map(|voter| voter.validator.clone())
            .collect();
        let header = BlockHeader::new(
            NonZeroU64::new(301).expect("protocol-limit effective height"),
            Some(parent_hash),
            None,
            301,
            0,
        );
        assert_eq!(
            state.deterministic_start_work_pending(&header).unwrap(),
            Some(true)
        );
        assert_eq!(
            state.world.privacy_activations.view().get(&activation_key),
            Some(&activation),
            "probing due work must leave the committed activation untouched"
        );
        let mut stale = header.clone();
        stale.set_height(NonZeroU64::new(302).expect("future height"));
        assert_eq!(
            state.deterministic_start_work_pending(&stale).unwrap(),
            None
        );
        stale = header.clone();
        stale.set_prev_block_hash(Some(HashOf::from_untyped_unchecked(Hash::new(
            b"wrong parent",
        ))));
        assert_eq!(
            state.deterministic_start_work_pending(&stale).unwrap(),
            None
        );
        let outcome = assemble_empty_snapshot_candidate_for_state(
            CandidateAttachments::default(),
            &state,
            context,
            anchor,
            key,
        );
        let CandidateAssemblyOutcome::Assembled(candidate) = outcome else {
            panic!("scheduled limits must produce their ordinary carrier without a transaction");
        };
        assert_eq!(candidate.block().header().height().get(), 301);
        assert_eq!(candidate.block().external_entrypoints_cloned().count(), 0);
        assert!(candidate.block().is_resultless_proposal());
        assert!(candidate_block_has_proposal_work(candidate.block(), &state, false).unwrap());
        let mut signed = candidate.block().clone();
        {
            let outputs = crate::execution_output_test_support::structural_network_outputs(
                &signed,
                &[],
                Vec::new(),
            );
            let fragments =
                u64::try_from(outputs.iter().filter(|row| row.result().is_ok()).count()).unwrap();
            signed.set_execution_outputs(
                outputs,
                fragments,
                BTreeMap::new(),
                Vec::new(),
                AxtPolicySnapshot::default(),
                Default::default(),
                Vec::new(),
                &crate::execution_output_test_support::structural_output_limits(),
            )
        }
        .expect("empty protocol-limit execution record");

        let committed = ValidBlock::new_unverified_for_tests(signed)
            .commit_unchecked()
            .unpack(|_| {});
        let mut overlay = state.block(committed.as_ref().header());
        let updated = overlay
            .world
            .privacy_activations
            .get(&activation_key)
            .expect("activation");
        assert_eq!(
            updated.lifecycle, activation.lifecycle,
            "protocol-limit work must not activate a pending proposal"
        );
        assert_eq!(updated.protocol_limits, next_limits);
        assert_eq!(updated.pending_protocol_limits_tightening, None);
        let _events = overlay.apply_without_execution(&committed, topology);
        overlay
            .commit()
            .expect("publish scheduled protocol limits in the ordinary carrier");
        let successor = BlockHeader::new(
            NonZeroU64::new(302).expect("successor height"),
            Some(committed.as_ref().hash()),
            None,
            302,
            0,
        );
        assert_eq!(
            state.deterministic_start_work_pending(&header).unwrap(),
            None
        );
        assert_eq!(
            state.deterministic_start_work_pending(&successor).unwrap(),
            Some(false)
        );
    }
    #[test]
    fn queue_plan_intent_remains_an_autonomous_fifo_barrier_after_exact_binding() {
        let (state, _context, anchor, key) = snapshot_parent_fixture();
        let (_, time_source) = TimeSource::new_mock(Duration::from_millis(3));
        let queue = Queue::test(
            iroha_config::parameters::actual::Queue::default(),
            &time_source,
        );
        let queue_plan = accepted_with_intent(0x81, TransactionAdmissionIntent::QueuePlanSynced);
        let follower = accepted_with_intent(0x82, TransactionAdmissionIntent::Ordinary);
        let assembler = V2CandidateAssembler::new(
            CandidateLimits::new(nonzero(8), nonzero(64 * 1024), nonzero(8))
                .expect("candidate limits"),
            time_source,
        );
        let mut blocked_report = CandidateScanReport::default();
        let blocked = assembler
            .snapshot_routable_candidates(
                &queue,
                &state,
                &CandidateAttachments::default(),
                vec![queue_plan.clone(), follower.clone()],
                64 * 1024,
                &mut blocked_report,
            )
            .expect("an absent marker is a normal bounded wait");
        assert!(blocked.is_empty());
        assert_eq!(blocked_report.inspected, 1);

        let routing_plan = queue
            .route_plan_with_state(&queue_plan, &state)
            .expect("fixture transaction has a routable plan");
        let validators = vec![PeerId::new(key.public_key().clone())];
        let context = crate::queue::QueuePlanAdmissionContextV1 {
            version: crate::queue::QUEUE_PLAN_ADMISSION_CONTEXT_VERSION_V1,
            authority_height: 2,
            proposal_height: 3,
            predecessor_block_hash: Some(anchor.snapshot_block_hash),
            routing_plan_digest: routing_plan.digest(),
            route_incarnations: routing_plan
                .legs()
                .into_iter()
                .map(|leg| crate::queue::QueuePlanRouteIncarnationV1 {
                    leg,
                    lane_incarnation: state
                        .lane_incarnation_at_height(leg.route.lane_id, 3)
                        .expect("fixture route has an active lane incarnation"),
                    validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
                    validator_set_hash: HashOf::new(&validators),
                    validator_set: validators.clone(),
                    validator_count: 1,
                    durability_threshold: 1,
                })
                .collect(),
        };
        let binding = crate::torii_proxy::new_queue_plan_admission_binding(
            state.network_id_ref(),
            queue_plan.entrypoint(),
            &routing_plan,
            context,
            3,
        )
        .expect("construct exact QueuePlan binding");
        state
            .install_queue_plan_pending_binding_for_test(&binding)
            .expect("install exact parent-state binding");

        let mut bound_report = CandidateScanReport::default();
        let bound = assembler
            .snapshot_routable_candidates(
                &queue,
                &state,
                &CandidateAttachments::default(),
                vec![queue_plan.clone(), follower.clone()],
                64 * 1024,
                &mut bound_report,
            )
            .expect("exact parent-state binding preserves the autonomous FIFO cut");
        assert!(bound.is_empty());
        assert_eq!(bound_report.inspected, 1);
        assert_eq!(bound_report.routable, 1);
        assert_eq!(bound_report.work_deferred, 1);
    }
    // Eligibility deliberately does not verify the pulse: the block validator
    // retains that separate cryptographic boundary. The beacon producer tests
    // exercise this gate with a genuinely reconstructed threshold signature.
    fn pulse_only_effects_fixture() -> NposConsensusEffects {
        use iroha_data_model::consensus::{
            FinalizedGlobalThresholdBeaconPulseV1, GlobalThresholdBeaconChainAnchorV1,
        };
        NposConsensusEffects {
            finalized_global_beacon_pulse: Some(FinalizedGlobalThresholdBeaconPulseV1 {
                version: 1,
                network_id: crate::sumeragi::synthetic_network_id("v2-candidate-test"),
                session_id: [1; 32],
                roster_hash: [2; 32],
                transcript_hash: [3; 32],
                height: 3,
                round: 0,
                finalized_chain_anchor: GlobalThresholdBeaconChainAnchorV1 {
                    height: 2,
                    block_hash: HashOf::from_untyped_unchecked(Hash::new(b"pulse parent")),
                },
                signature: [4; 48],
                seed: [5; 32],
                pulse_id: [6; 32],
            }),
            ..NposConsensusEffects::default()
        }
    }
    #[test]
    fn mandatory_beacon_wait_requires_independent_work() {
        let pending = CandidateAttachments {
            required_beacon_pulse_pending: true,
            ..CandidateAttachments::default()
        };
        assert!(matches!(
            assemble_empty_snapshot_candidate(pending.clone()),
            CandidateAssemblyOutcome::NoProposalWork(_)
        ));
        let useful = CandidateAttachments {
            time_trigger_clock_progress_required: true,
            ..pending
        };
        assert!(matches!(
            assemble_empty_snapshot_candidate(useful),
            CandidateAssemblyOutcome::AwaitingRequiredBeacon(_)
        ));
    }
    #[test]
    fn mandatory_beacon_wait_releases_same_queue_prefix_for_retry() {
        let (_, time_source) = TimeSource::new_mock(Duration::from_millis(3));
        let (state, mut context, anchor, key) = snapshot_parent_fixture();
        let mut world = state.world.block();
        let account_key = KeyPair::try_from_seed(vec![0x71; 32], Algorithm::Ed25519)
            .expect("deterministic queued authority");
        let authority = AccountId::new(account_key.public_key().clone());
        world.accounts.insert(
            authority.clone(),
            iroha_data_model::account::AccountValue::new(
                iroha_data_model::account::AccountDetails::default(),
            ),
        );
        let transaction = TransactionBuilder::new_with_time_source(
            crate::sumeragi::synthetic_network_id("v2-candidate-test"),
            authority,
            &time_source,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_admission_intent(TransactionAdmissionIntent::Ordinary)
        .sign(account_key.private_key());
        let transaction = AcceptedTransaction::new_unchecked(Cow::Owned(transaction));
        let expected = transaction.hash_as_entrypoint();
        world.commit();
        context.da_layout.max_payload_size_bytes = 64 * 1024;
        context.da_layout.max_chunk_count = 128;
        context.validate().expect("expanded fixture DA limits");
        let queue = Arc::new(Queue::test(
            iroha_config::parameters::actual::Queue::default(),
            &time_source,
        ));
        queue
            .push(transaction, state.view())
            .expect("admit the exact ordinary entry");
        let tag = EventTag::new(
            context.height,
            0,
            crate::sumeragi::v2_core::Generation::new(0),
        );
        let local_validator = context.leader(tag.view());
        let directive = LocalProposalDirective::for_test(tag, local_validator, None, None, None);
        let assembler = V2CandidateAssembler::new(
            CandidateLimits::new(nonzero(8), nonzero(64 * 1024), nonzero(8)).expect("limits"),
            time_source,
        );
        let blocked_signing = ConsensusOutputGuard::isolated();
        blocked_signing.activate_restart_required();
        // A pending pulse must return before touching the body-signing boundary.
        let pending = assembler
            .assemble(CandidateRequest {
                context: &context,
                directive,
                local_validator,
                parent: CandidateParent::Snapshot(&anchor),
                state: &state,
                queue: &queue,
                key_pair: &key,
                output_guard: &blocked_signing,
                attachments: CandidateAttachments {
                    required_beacon_pulse_pending: true,
                    ..CandidateAttachments::default()
                },
                work_provider: SingleRouteWorkProvider,
            })
            .expect("pending beacon does not sign");
        let CandidateAssemblyOutcome::AwaitingRequiredBeacon(report) = pending else {
            panic!("useful FIFO entry must demand its mandatory pulse");
        };
        assert_eq!(report.selected, 1);
        assert_eq!(queue.queued_len(), 1);
        assert!(queue.live_lane_reservations().is_empty());
        assert!(!queue.transaction_selection_durability_faulted());
        {
            let state_view = state.view();
            let (retained, lease) = queue
                .bounded_pending_snapshot(&state_view, nonzero(8))
                .expect("pending attempt released its lease");
            assert_eq!(
                retained
                    .iter()
                    .map(AcceptedTransaction::hash_as_entrypoint)
                    .collect::<Vec<_>>(),
                vec![expected]
            );
            drop(lease);
        }
        let output_guard = ConsensusOutputGuard::isolated();
        let effects = pulse_only_effects_fixture();
        let resumed = assembler
            .assemble(CandidateRequest {
                context: &context,
                directive,
                local_validator,
                parent: CandidateParent::Snapshot(&anchor),
                state: &state,
                queue: &queue,
                key_pair: &key,
                output_guard: &output_guard,
                attachments: CandidateAttachments {
                    npos_consensus_effects: Some(effects.clone()),
                    ..CandidateAttachments::default()
                },
                work_provider: SingleRouteWorkProvider,
            })
            .expect("same retained work resumes once the pulse is available");
        let CandidateAssemblyOutcome::Assembled(candidate) = resumed else {
            panic!("retained useful work with pulse must assemble");
        };
        assert_eq!(
            candidate
                .block()
                .external_entrypoints_cloned()
                .map(|entry| entry.hash())
                .collect::<Vec<_>>(),
            vec![expected]
        );
        assert_eq!(candidate.block().npos_consensus_effects(), Some(&effects));
        drop(candidate);
        assert_eq!(
            queue.queued_len(),
            1,
            "proposal assembly never consumes the transaction"
        );
    }
    #[test]
    fn proposal_work_gate_rejects_beacon_pulse_only() {
        let effects = pulse_only_effects_fixture();
        assert!(!effects.is_empty(), "the wire effect is retained");
        let attachments = CandidateAttachments {
            npos_consensus_effects: Some(effects),
            ..CandidateAttachments::default()
        };
        assert!(!candidate_has_proposal_work(
            &[],
            &attachments,
            &PreparedCandidateWork::default()
        ));
        assert!(matches!(
            assemble_empty_snapshot_candidate(attachments.clone()),
            CandidateAssemblyOutcome::NoProposalWork(_)
        ));
        let useful = CandidateAttachments {
            time_trigger_clock_progress_required: true,
            ..attachments
        };
        let CandidateAssemblyOutcome::Assembled(candidate) =
            assemble_empty_snapshot_candidate(useful.clone())
        else {
            panic!("independent due clock work must retain the pulse in its carrier");
        };
        assert_eq!(
            candidate.block().npos_consensus_effects(),
            useful.npos_consensus_effects.as_ref()
        );
    }
    #[test]
    fn proposal_work_gate_preserves_non_beacon_effects() {
        use iroha_data_model::consensus::{
            NposMarkConsensusEvidenceAppliedAction, NposPenaltyAction,
        };
        let mut effects = pulse_only_effects_fixture();
        effects
            .penalty_actions
            .push(NposPenaltyAction::MarkConsensusEvidenceApplied(
                NposMarkConsensusEvidenceAppliedAction {
                    evidence_key: Hash::new(b"genuine admitted evidence"),
                    height: 3,
                },
            ));
        let attachments = CandidateAttachments {
            npos_consensus_effects: Some(effects),
            ..CandidateAttachments::default()
        };
        assert!(candidate_has_proposal_work(
            &[],
            &attachments,
            &PreparedCandidateWork::default()
        ));
        let CandidateAssemblyOutcome::Assembled(candidate) =
            assemble_empty_snapshot_candidate(attachments.clone())
        else {
            panic!("deterministic evidence application remains independent work");
        };
        assert_eq!(
            candidate.block().npos_consensus_effects(),
            attachments.npos_consensus_effects.as_ref()
        );
    }
    #[test]
    fn proposal_work_gate_normalizes_empty_control_bundles() {
        let outcome = assemble_empty_snapshot_candidate(CandidateAttachments {
            da_commitments: Some(DaCommitmentBundle::new(Vec::new())),
            da_pin_intents: Some(DaPinIntentBundle::new(Vec::new())),
            npos_consensus_effects: Some(NposConsensusEffects::default()),
            ..CandidateAttachments::default()
        });
        let CandidateAssemblyOutcome::NoProposalWork(report) = outcome else {
            panic!("normalized empty control bundles must not manufacture a candidate");
        };
        assert_eq!(report, CandidateScanReport::default());
    }
    #[test]
    fn proposal_work_gate_preserves_time_trigger_work() {
        let outcome = assemble_empty_snapshot_candidate(CandidateAttachments {
            time_trigger_clock_progress_required: true,
            ..CandidateAttachments::default()
        });
        let CandidateAssemblyOutcome::Assembled(candidate) = outcome else {
            panic!("due time-trigger work must produce a candidate");
        };
        assert_eq!(candidate.scan_report(), CandidateScanReport::default());
        assert_eq!(candidate.block().external_entrypoints_cloned().count(), 0);
    }
    #[test]
    fn canonical_block_work_gate_preserves_transaction_autonomous_and_clock_work() {
        let (state, context, _anchor, key) = snapshot_parent_fixture();
        let mut block: SignedBlock = ValidBlock::new_dummy(key.private_key()).into();
        assert!(!candidate_block_has_proposal_work(&block, &state, false).unwrap());
        assert!(
            candidate_block_has_proposal_work(&block, &state, true).unwrap(),
            "state-derived clock progress is semantic proposal work"
        );
        let transaction = accepted(71, "canonical-block-external");
        block.set_external_entrypoints(vec![transaction.entrypoint().clone()]);
        assert!(candidate_block_has_proposal_work(&block, &state, false).unwrap());
        let mut autonomous: SignedBlock = ValidBlock::new_dummy(key.private_key()).into();
        autonomous.set_execution_context(Some(
            BlockExecutionContextBundle::default().with_autonomous_lane_payloads(vec![
                AutonomousLanePayloadEnvelopeV1 {
                    version: 1,
                    network_id: context.network_id,
                    epoch: context.epoch,
                    lane_id: LaneId::new(1),
                    dataspace_id: DataSpaceId::new(11),
                    lane_incarnation: Hash::new(b"canonical block autonomous incarnation"),
                    proposal_height: context.height,
                    lane_block_height: 1,
                    lane_block_view: 0,
                    proposal_hash: Hash::new(b"canonical block autonomous proposal"),
                    descriptor_hash: Hash::new(b"canonical block autonomous descriptor"),
                    payload_hash: Hash::new(b"canonical block autonomous payload"),
                    producer: PeerId::new(key.public_key().clone()),
                    canonical_payload: vec![0xA5],
                },
            ]),
        ));
        assert!(candidate_block_has_proposal_work(&autonomous, &state, false).unwrap());
        let mut queue_plan: SignedBlock = ValidBlock::new_dummy(key.private_key()).into();
        queue_plan.set_execution_context(Some(
            BlockExecutionContextBundle::default().with_queue_plan_admissions(vec![vec![0xA5]]),
        ));
        assert!(
            candidate_block_has_proposal_work(&queue_plan, &state, false).unwrap(),
            "a proposal-native QueuePlan certificate is deterministic carrier work"
        );
    }
    #[test]
    fn proposal_work_gate_accepts_external_and_control_work() {
        let attachments = CandidateAttachments::default();
        let prepared = PreparedCandidateWork::default();
        assert!(!candidate_has_proposal_work(&[], &attachments, &prepared));
        let external = vec![record(39, "proposal-work", 0)];
        assert!(candidate_has_proposal_work(
            &external,
            &attachments,
            &prepared
        ));
        let control = CandidateAttachments {
            sccp_commitment_root: Some([0x5A; 32]),
            ..CandidateAttachments::default()
        };
        assert!(candidate_has_proposal_work(&[], &control, &prepared));
        let queue_plan = CandidateAttachments {
            queue_plan_admissions: vec![vec![0xA5]],
            ..CandidateAttachments::default()
        };
        assert!(candidate_has_proposal_work(&[], &queue_plan, &prepared));
    }
    #[test]
    fn snapshot_candidate_parent_is_exact_and_one_shot() {
        let (state, context, anchor, key) = snapshot_parent_fixture();
        assert_eq!(
            validate_candidate_parent(&context, CandidateParent::Snapshot(&anchor), &state)
                .expect("exact authenticated snapshot parent"),
            2
        );
        let mut wrong_hash = anchor;
        wrong_hash.snapshot_block_hash = HashOf::from_untyped_unchecked(Hash::new(b"wrong tip"));
        assert!(matches!(
            validate_candidate_parent(&context, CandidateParent::Snapshot(&wrong_hash), &state),
            Err(CandidateError::ParentContextMismatch)
        ));
        let mut wrong_height = anchor;
        wrong_height.snapshot_height = 1;
        assert!(matches!(
            validate_candidate_parent(&context, CandidateParent::Snapshot(&wrong_height), &state),
            Err(CandidateError::ParentContextMismatch)
        ));
        let successor = ValidBlock::new_dummy_and_modify_header(key.private_key(), |header| {
            header.set_height(nonzero!(3_u64));
            header.set_prev_block_hash(Some(anchor.snapshot_block_hash));
            header.creation_time_ms = 3;
            header.merkle_root = None;
        });
        let mut signed: SignedBlock = successor.into();
        {
            let outputs = crate::execution_output_test_support::structural_network_outputs(
                &signed,
                &[],
                Vec::new(),
            );
            let fragments =
                u64::try_from(outputs.iter().filter(|row| row.result().is_ok()).count()).unwrap();
            signed.set_execution_outputs(
                outputs,
                fragments,
                BTreeMap::new(),
                Vec::new(),
                AxtPolicySnapshot::default(),
                Default::default(),
                Vec::new(),
                &crate::execution_output_test_support::structural_output_limits(),
            )
        }
        .expect("snapshot successor carries its exact empty execution outputs");

        let signature = iroha_data_model::block::BlockSignature::new(
            0,
            iroha_crypto::SignatureOf::from_hash(key.private_key(), signed.header().hash()),
        );
        signed
            .replace_signatures(std::collections::BTreeSet::from([signature]))
            .expect("sign the complete snapshot successor header");
        let successor = ValidBlock::new_unverified_for_tests(signed)
            .commit_unchecked()
            .unpack(|_| {});
        let topology = Topology::new(context.roster.iter().map(|entry| entry.validator.clone()));
        let mut state_block = state.block(successor.as_ref().header());
        let _events = state_block.apply_without_execution(&successor, topology.as_ref().to_owned());
        state_block
            .commit()
            .expect("advance beyond snapshot boundary");
        assert!(matches!(
            validate_candidate_parent(&context, CandidateParent::Snapshot(&anchor), &state),
            Err(CandidateError::ParentStateMismatch)
        ));
    }
    #[test]
    fn limits_require_scan_to_cover_maximum_batch() {
        assert!(matches!(
            CandidateLimits::new(nonzero(4), nonzero(1024), nonzero(3)),
            Err(CandidateError::ScanLimitBelowTransactionLimit {
                max_transactions: 4,
                max_queue_scan: 3,
            })
        ));
        assert!(CandidateLimits::new(nonzero(4), nonzero(1024), nonzero(4)).is_ok());
    }
    #[test]
    fn candidate_limit_never_exceeds_the_active_protocol_limit() {
        assert_eq!(
            effective_candidate_transaction_limit(nonzero(8), nonzero!(3_u64)),
            3
        );
        assert_eq!(
            effective_candidate_transaction_limit(nonzero(2), nonzero!(3_u64)),
            2
        );
    }
    #[test]
    fn candidate_selection_reserves_future_terminal_capacity_before_signing() {
        use iroha_data_model::parameter::{BlockParameter, Parameter, Parameters};
        let mut parameters = Parameters::default();
        let mut policy = parameters.block().execution_output();
        policy.max_outputs = 1 + 2 * policy.max_pipeline_triggers + policy.max_time_invocations;
        parameters.set_parameter(Parameter::Block(BlockParameter::ExecutionOutput(policy)));
        assert_eq!(
            effective_output_transaction_limit(nonzero(512), parameters.block()).unwrap(),
            1
        );
        parameters.set_parameter(Parameter::Block(BlockParameter::MaxTimeTriggerInvocations(
            nonzero!(1_u32),
        )));
        assert_eq!(
            effective_output_transaction_limit(nonzero(512), parameters.block()).unwrap(),
            1,
            "selection remains safe through later permitted Time growth"
        );
        policy.max_outputs = 1;
        parameters.set_parameter(Parameter::Block(BlockParameter::ExecutionOutput(policy)));
        assert!(matches!(
            effective_output_transaction_limit(nonzero(512), parameters.block()),
            Err(CandidateError::InvalidOutputCapacity(_))
        ));
    }

    #[test]
    fn canonical_order_preserves_fifo_independent_of_entrypoint_hash() {
        let mut records = vec![
            record(1, "third", 2),
            record(3, "first", 0),
            record(2, "second", 1),
        ];
        records.sort_by(|left, right| right.entrypoint_hash.cmp(&left.entrypoint_hash));
        for (source_ordinal, record) in records.iter_mut().enumerate() {
            record.source_ordinal = source_ordinal;
        }
        let fifo_hashes = records
            .iter()
            .take(2)
            .map(|record| record.entrypoint_hash)
            .collect::<BTreeSet<_>>();
        let mut reserve = VecDeque::from(records);
        let mut selected = Vec::new();
        let mut report = CandidateScanReport::default();
        fill_selection(&mut selected, &mut reserve, 2, usize::MAX, &mut report);
        assert_eq!(
            selected
                .iter()
                .map(|record| record.entrypoint_hash)
                .collect::<BTreeSet<_>>(),
            fifo_hashes,
            "canonical payload order must not change FIFO batch membership"
        );
        assert!(selected[0].entrypoint_hash > selected[1].entrypoint_hash);
        order_records_by_fifo(&mut selected);
        assert!(
            selected
                .windows(2)
                .all(|window| window[0].source_ordinal < window[1].source_ordinal),
            "an attacker-controlled entrypoint hash must not buy earlier execution"
        );
        assert!(selected[0].entrypoint_hash > selected[1].entrypoint_hash);
    }
    #[test]
    fn single_route_provider_defers_native_amx_only() {
        let mut single = record(1, "single", 0);
        single.routing_plan = RoutingPlan::single(RoutingDecision::default());
        let coordinator = RoutingDecision::new(LaneId::new(1), DataSpaceId::new(1));
        let participant = RouteLeg::new(
            RoutingDecision::new(LaneId::new(2), DataSpaceId::new(2)),
            RouteLegRole::Participant,
        );
        let mut native = record(2, "native", 1);
        native.routing_plan = RoutingPlan::native_amx(coordinator, vec![participant]);
        let candidates = [single.descriptor(), native.descriptor()];
        let _provider = SingleRouteWorkProvider;
        assert_eq!(
            unavailable_native_amx_indices(&candidates),
            BTreeSet::from([1])
        );
    }
    #[test]
    fn native_episode_deferral_suppresses_later_native_refill() {
        let coordinator = RoutingDecision::new(LaneId::new(1), DataSpaceId::new(1));
        let participant = RouteLeg::new(
            RoutingDecision::new(LaneId::new(2), DataSpaceId::new(2)),
            RouteLegRole::Participant,
        );
        let mut selected_native = record(1, "selected-native", 0);
        selected_native.routing_plan =
            RoutingPlan::native_amx(coordinator.clone(), vec![participant.clone()]);
        let selected_single = record(2, "selected-single", 1);
        let mut reserve_native = record(3, "reserve-native", 2);
        reserve_native.routing_plan = RoutingPlan::native_amx(coordinator, vec![participant]);
        let reserve_single = record(4, "reserve-single", 3);
        let mut selected = vec![selected_native, selected_single];
        let mut reserve = VecDeque::from([reserve_native, reserve_single]);
        let mut report = CandidateScanReport::default();

        defer_native_candidates_for_episode(&mut selected, &mut reserve, &mut report);
        fill_selection(&mut selected, &mut reserve, 4, usize::MAX, &mut report);

        assert_eq!(selected.len(), 2);
        assert!(
            selected
                .iter()
                .all(|candidate| matches!(&candidate.routing_plan, RoutingPlan::Single(_)))
        );
        assert!(reserve.is_empty());
        assert_eq!(report.work_deferred, 2);
        let unavailable = CandidateWorkUnavailable::defer_native_for_episode(
            BTreeSet::from([0]),
            "Native cohort pending",
        );
        assert!(unavailable.defers_native_for_episode());
    }
    #[test]
    fn autonomous_anchors_validate_without_ordinary_candidates() {
        let (_state, context, _anchor, _key) = snapshot_parent_fixture();
        let first_tx = autonomous_accepted(31);
        let second_tx = autonomous_accepted(32);
        let envelopes = vec![
            autonomous_envelope(
                &context,
                LaneId::new(1),
                DataSpaceId::new(11),
                Hash::new(b"candidate autonomous incarnation one"),
                1,
                0,
                &first_tx,
                41,
            ),
            autonomous_envelope(
                &context,
                LaneId::new(2),
                DataSpaceId::new(12),
                Hash::new(b"candidate autonomous incarnation two"),
                3,
                // The authenticated origin stays at view zero; later lane
                // views require separate NewView evidence.
                0,
                &second_tx,
                51,
            ),
        ];
        let prepared = PreparedCandidateWork {
            native_lane_decisions: None,
            native_amx_receipts: Vec::new(),
            lane_payload_ownerships: Vec::new(),
            autonomous_lane_payloads: envelopes,
        };
        assert!(validate_prepared_work(&context, 0, &[], &prepared).is_ok());
        let empty = PreparedCandidateWork::default();
        assert!(empty.autonomous_lane_payloads.is_empty());
        assert!(validate_prepared_work(&context, 0, &[], &empty).is_ok());
        let mut single_route_provider = SingleRouteWorkProvider;
        let provider_empty = single_route_provider
            .prepare(&context, 0, &[])
            .expect("test provider accepts an empty descriptor batch");
        assert!(provider_empty.autonomous_lane_payloads.is_empty());
    }
    #[test]
    fn autonomous_anchor_order_and_identity_duplicates_fail_closed() {
        let (_state, context, _anchor, _key) = snapshot_parent_fixture();
        let first_tx = autonomous_accepted(33);
        let second_tx = autonomous_accepted(34);
        let first = autonomous_envelope(
            &context,
            LaneId::new(3),
            DataSpaceId::new(13),
            Hash::new(b"candidate autonomous ordered incarnation one"),
            1,
            0,
            &first_tx,
            61,
        );
        let second = autonomous_envelope(
            &context,
            LaneId::new(4),
            DataSpaceId::new(14),
            Hash::new(b"candidate autonomous ordered incarnation two"),
            1,
            0,
            &second_tx,
            71,
        );
        assert!(matches!(
            validate_autonomous_lane_payloads(&context, &[], &[second.clone(), first.clone()]),
            Err(CandidateError::AutonomousLanePayloadOrder)
        ));
        assert!(matches!(
            validate_autonomous_lane_payloads(&context, &[], &[first.clone(), first.clone()]),
            Err(CandidateError::AutonomousLanePayloadDuplicateRoute)
        ));
        let mut duplicate_proposal = second.clone();
        duplicate_proposal.proposal_hash = first.proposal_hash;
        assert!(matches!(
            validate_autonomous_lane_payloads(&context, &[], &[first.clone(), duplicate_proposal]),
            Err(CandidateError::AutonomousLanePayloadDuplicateProposal)
        ));
        let mut duplicate_payload = second;
        duplicate_payload.payload_hash = first.payload_hash;
        assert!(matches!(
            validate_autonomous_lane_payloads(&context, &[], &[first, duplicate_payload]),
            Err(CandidateError::AutonomousLanePayloadDuplicatePayload)
        ));
    }
    #[test]
    fn autonomous_anchor_entrypoints_are_disjoint_from_global_and_each_other() {
        let (_state, context, _anchor, _key) = snapshot_parent_fixture();
        let ordinary = autonomous_record(35, 0);
        let envelope = autonomous_envelope(
            &context,
            LaneId::new(5),
            DataSpaceId::new(15),
            Hash::new(b"candidate autonomous overlap incarnation"),
            1,
            0,
            &ordinary.transaction,
            81,
        );
        let candidates = [ordinary.descriptor()];
        assert!(matches!(
            validate_autonomous_lane_payloads(&context, &candidates, &[envelope]),
            Err(CandidateError::AutonomousLanePayloadOverlapsOrdinary)
        ));
        let shared_tx = autonomous_accepted(36);
        let first = autonomous_envelope(
            &context,
            LaneId::new(6),
            DataSpaceId::new(16),
            Hash::new(b"candidate autonomous duplicate incarnation one"),
            1,
            0,
            &shared_tx,
            91,
        );
        let second = autonomous_envelope(
            &context,
            LaneId::new(7),
            DataSpaceId::new(17),
            Hash::new(b"candidate autonomous duplicate incarnation two"),
            1,
            0,
            &shared_tx,
            101,
        );
        assert!(matches!(
            validate_autonomous_lane_payloads(&context, &[], &[first, second]),
            Err(CandidateError::AutonomousLanePayloadDuplicateEntrypoint)
        ));
    }
    #[test]
    fn autonomous_anchor_height_and_payload_authentication_fail_closed() {
        let (_state, context, _anchor, _key) = snapshot_parent_fixture();
        let transaction = autonomous_accepted(37);
        let envelope = autonomous_envelope(
            &context,
            LaneId::new(8),
            DataSpaceId::new(18),
            Hash::new(b"candidate autonomous authentication incarnation"),
            1,
            0,
            &transaction,
            111,
        );
        let mut wrong_height = envelope.clone();
        wrong_height.proposal_height = context.height.saturating_add(1);
        assert!(matches!(
            validate_autonomous_lane_payloads(&context, &[], &[wrong_height]),
            Err(CandidateError::AutonomousLanePayloadHeightMismatch { .. })
        ));
        let mut corrupt = envelope;
        corrupt.canonical_payload.push(0);
        assert!(matches!(
            validate_autonomous_lane_payloads(&context, &[], &[corrupt]),
            Err(CandidateError::AutonomousLanePayloadInvalid(_))
        ));
    }
    #[test]
    fn autonomous_anchor_count_and_aggregate_bytes_are_bounded() {
        let (_state, context, _anchor, _key) = snapshot_parent_fixture();
        let transaction = autonomous_accepted(38);
        let envelope = autonomous_envelope(
            &context,
            LaneId::new(9),
            DataSpaceId::new(19),
            Hash::new(b"candidate autonomous bounds incarnation"),
            1,
            0,
            &transaction,
            121,
        );
        let too_many = vec![envelope.clone(); MAX_MERGE_EXECUTION_ENTRYPOINTS + 1];
        assert!(matches!(
            validate_autonomous_lane_payloads(&context, &[], &too_many),
            Err(CandidateError::AutonomousLanePayloadCountExceeded { .. })
        ));
        let mut large = envelope;
        large
            .canonical_payload
            .resize(MAX_MERGE_EXECUTION_BATCH_BYTES / 4, 0);
        let aggregate = vec![large; 4];
        let baseline_bytes = match validate_autonomous_lane_payloads(&context, &[], &aggregate) {
            Err(CandidateError::AutonomousLanePayloadAggregateBytesExceeded { bytes, max }) => {
                assert_eq!(max, MAX_MERGE_EXECUTION_BATCH_BYTES);
                bytes
            }
            other => panic!("expected aggregate byte rejection, got {other:?}"),
        };
        let alternate_bytes = {
            let alternate_flags =
                norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
            let _ambient = norito::core::DecodeFlagsGuard::enter(alternate_flags);
            match validate_autonomous_lane_payloads(&context, &[], &aggregate) {
                Err(CandidateError::AutonomousLanePayloadAggregateBytesExceeded { bytes, max }) => {
                    assert_eq!(max, MAX_MERGE_EXECUTION_BATCH_BYTES);
                    bytes
                }
                other => panic!("expected ambient aggregate byte rejection, got {other:?}"),
            }
        };
        assert_eq!(
            alternate_bytes, baseline_bytes,
            "candidate admission must account exact canonical envelope bytes"
        );
    }
    #[test]
    fn chunk_count_rejects_invalid_rs16_geometry_and_matches_stripes() {
        let rs = wire::DataAvailabilityLayout {
            encoding: wire::PayloadEncoding::ReedSolomon16,
            chunk_size_bytes: 8,
            data_shards: 4,
            parity_shards: 2,
            max_payload_size_bytes: 1024,
            max_chunk_count: 1024,
        };
        for invalid in [
            wire::DataAvailabilityLayout {
                data_shards: 0,
                ..rs
            },
            wire::DataAvailabilityLayout {
                parity_shards: 0,
                ..rs
            },
        ] {
            assert!(matches!(
                encoded_chunk_count(invalid, 17),
                Err(CandidateError::InvalidDataAvailabilityLayout)
            ));
        }
        assert_eq!(encoded_chunk_count(rs, 17).expect("one stripe"), 6);
        assert_eq!(encoded_chunk_count(rs, 33).expect("two stripes"), 12);
    }
    #[test]
    fn unavailable_removal_is_bounded_and_keeps_canonical_survivors() {
        let mut selected = vec![
            record(1, "one", 0),
            record(2, "two", 1),
            record(3, "three", 2),
        ];
        order_records_by_fifo(&mut selected);
        let removed_hash = selected[1].entrypoint_hash;
        let surviving = [selected[0].entrypoint_hash, selected[2].entrypoint_hash];
        let unavailable = CandidateWorkUnavailable::new(BTreeSet::from([1]), "lane pending");
        let mut report = CandidateScanReport::default();
        remove_unavailable_candidates(&mut selected, &unavailable, &mut report)
            .expect("valid unavailable set");
        assert_eq!(report.work_deferred, 1);
        assert_eq!(
            selected
                .iter()
                .map(|entry| entry.entrypoint_hash)
                .collect::<Vec<_>>(),
            surviving
        );
        assert!(
            !selected
                .iter()
                .any(|entry| entry.entrypoint_hash == removed_hash)
        );
    }
    #[test]
    fn certified_execution_filter_defers_every_ordinary_entrypoint() {
        let mut report = CandidateScanReport::default();
        for _ in 0..4 {
            assert!(
                record_ordinary_execution_carrier_exclusion(true, &mut report),
                "every ordinary entrypoint conflicts with a selected execution carrier"
            );
        }
        assert_eq!(report.carrier_excluded, 4);
        assert_eq!(
            report.work_deferred, 0,
            "carrier exclusions are not unavailable lane work and must not arm heartbeat fallback"
        );
        assert!(
            !record_ordinary_execution_carrier_exclusion(false, &mut report),
            "ordinary queue selection remains enabled without a selected execution carrier"
        );
        assert_eq!(report.carrier_excluded, 4);
    }
    #[test]
    fn certified_merge_carrier_context_rejects_timestamp_view_and_root_drift() {
        let parent = HashOf::from_untyped_unchecked(Hash::new(b"candidate carrier context parent"));
        let built = BlockHeader::new(nonzero!(7_u64), Some(parent), None, 1_000, 3);
        assert!(stripped_carrier_context_matches(&built, &built));
        let wrong_height = BlockHeader::new(nonzero!(8_u64), Some(parent), None, 1_000, 3);
        assert!(!stripped_carrier_context_matches(&built, &wrong_height));
        let wrong_parent = BlockHeader::new(
            nonzero!(7_u64),
            Some(HashOf::from_untyped_unchecked(Hash::new(
                b"different carrier parent",
            ))),
            None,
            1_000,
            3,
        );
        assert!(!stripped_carrier_context_matches(&built, &wrong_parent));
        let mut wrong_time = built.clone();
        wrong_time.creation_time_ms = wrong_time.creation_time_ms.saturating_add(1);
        assert!(!stripped_carrier_context_matches(&built, &wrong_time));
        let mut wrong_view = built.clone();
        wrong_view.set_view_change_index(4);
        assert!(!stripped_carrier_context_matches(&built, &wrong_view));
        let mut rooted = built.clone();
        rooted.merkle_root = Some(HashOf::from_untyped_unchecked(Hash::new(
            b"unexpected carrier transaction root",
        )));
        assert!(!stripped_carrier_context_matches(&built, &rooted));
    }
}
