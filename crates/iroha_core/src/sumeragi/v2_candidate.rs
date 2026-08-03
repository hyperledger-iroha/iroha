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

use std::{
    collections::{BTreeSet, VecDeque},
    num::NonZeroUsize,
    time::Duration,
};

use super::v2_core::EventTag;
use iroha_crypto::{Hash, HashOf, KeyPair};
use iroha_data_model::{
    block::{
        AutonomousLanePayloadEnvelopeV1, BlockExecutionContextBundle, BlockHeader,
        CertifiedMergeLedgerReference, SignedBlock,
        consensus::{NativeAmxReceipt, SumeragiLanePayloadOwnership},
        consensus_v2 as wire,
    },
    consensus::{NposConsensusEffects, PreviousRosterEvidence},
    da::{commitment::DaCommitmentBundle, pin_intent::DaPinIntentBundle},
    events::pipeline::PipelineEventBox,
    merge::{MAX_MERGE_EXECUTION_BATCH_BYTES, MAX_MERGE_EXECUTION_ENTRYPOINTS, MergeLedgerEntry},
    transaction::TransactionEntrypoint,
};
use iroha_primitives::time::TimeSource;
use thiserror::Error;

use super::{
    output_guard::ConsensusOutputGuard,
    v2::LocalProposalDirective,
    v2_chunks::{EncodedV2Payload, encode_payload},
};
use crate::{
    block::BlockBuilder,
    queue::{GlobalQueueSelectionLease, Queue, RoutingPlan, execution_context_for_routing_plan},
    state::{State, StateReadOnly, compute_confidential_feature_digest},
    tx::AcceptedTransaction,
};

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
    /// Previous-height roster audit evidence, while required by block validity.
    pub(crate) previous_roster_evidence: Option<PreviousRosterEvidence>,
    /// Deterministic NPoS state effects for this height.
    pub(crate) npos_consensus_effects: Option<NposConsensusEffects>,
    /// SCCP root derived by deterministic execution, when applicable.
    pub(crate) sccp_commitment_root: Option<[u8; 32]>,
    /// Exact stripped application header certified by an autonomous merge
    /// batch. Ordinary and relay-only candidates leave this absent.
    pub(crate) certified_merge_carrier_header: Option<BlockHeader>,
    /// Complete, locally validated sidecar selected for this exact carrier round.
    /// Only its compact certified reference is embedded in the block.
    pub(crate) certified_merge_entry: Option<MergeLedgerEntry>,
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

    /// Canonical entrypoint hash which determines block order.
    pub(crate) const fn entrypoint_hash(self) -> HashOf<TransactionEntrypoint> {
        self.entrypoint_hash
    }
}

/// Lane-local, Native AMX, and autonomous control-anchor material for a candidate.
#[derive(Clone, Debug, Default)]
pub(crate) struct PreparedCandidateWork {
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
}

impl CandidateWorkUnavailable {
    /// Construct an unavailable-work result.
    #[must_use]
    pub(crate) fn new(indices: BTreeSet<usize>, reason: impl Into<String>) -> Self {
        Self {
            indices,
            reason: reason.into(),
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
/// autonomous envelope vector disjoint from those descriptors.
pub(crate) trait CandidateWorkProvider {
    /// Prepare receipts, lane-local ownerships, and autonomous control anchors.
    fn prepare(
        &mut self,
        context: &wire::HeightContext,
        view: wire::View,
        candidates: &[CandidateDescriptor<'_>],
    ) -> Result<PreparedCandidateWork, CandidateWorkUnavailable>;
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
/// an honest leader from producing a heartbeat or single-route block.
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
    ) -> Result<PreparedCandidateWork, CandidateWorkUnavailable> {
        let unavailable = unavailable_native_amx_indices(candidates);
        if unavailable.is_empty() {
            Ok(PreparedCandidateWork::single_route_batch(candidates.len()))
        } else {
            Err(CandidateWorkUnavailable::new(
                unavailable,
                "certified Native AMX receipts are not available",
            ))
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
    /// Whether this exact recovery path may deliberately build an empty body.
    pub(crate) allow_empty_recovery_heartbeat: bool,
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
    /// Entries skipped because certified lane/AMX work was unavailable.
    pub(crate) work_deferred: usize,
    /// External transactions included in the final body.
    pub(crate) selected: usize,
}

/// Result of one bounded fresh-candidate assembly attempt.
#[derive(Debug)]
pub(crate) enum CandidateAssemblyOutcome {
    /// A signed body carrying ordinary, autonomous, internal, or explicitly
    /// armed recovery-heartbeat work.
    Assembled(AssembledV2Candidate),
    /// The queue snapshot and internal providers contained no proposal work.
    NoProposalWork(CandidateScanReport),
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

    /// Assemble, sign, exactly encode, and deterministically chunk one fresh
    /// successor body.
    ///
    /// The queue is never mutated. An empty queue, an entirely unavailable
    /// lane/AMX snapshot, or a batch whose transactions do not fit returns
    /// [`CandidateAssemblyOutcome::NoProposalWork`] unless genuine internal
    /// work exists or the caller explicitly armed a recovery heartbeat.
    ///
    /// # Errors
    ///
    /// Returns [`CandidateError`] for a stale reducer directive, a non-leader
    /// caller, parent/context drift, malformed certified work, signing failure,
    /// or a heartbeat which itself exceeds frozen body/chunk limits.
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
        let mut selected = Vec::with_capacity(self.limits.max_transactions.get());
        fill_selection(
            &mut selected,
            &mut reserve,
            self.limits.max_transactions.get(),
            exact_payload_limit,
            &mut report,
        );

        // Every iteration either returns or permanently removes at least one
        // of the at-most `max_queue_scan` inspected records.
        let max_attempts = self.limits.max_queue_scan.get().saturating_add(1);
        for _ in 0..max_attempts {
            // Freeze FIFO membership before adopting the same canonical payload
            // order used by `BlockBuilder`; routing contexts and work receipts
            // are positional and must be prepared against that exact order.
            canonicalize_records(&mut selected);
            let descriptors = selected
                .iter()
                .map(CandidateRecord::descriptor)
                .collect::<Vec<_>>();
            let prepared_work =
                match request
                    .work_provider
                    .prepare(request.context, view, &descriptors)
                {
                    Ok(work) => work,
                    Err(unavailable) => {
                        remove_unavailable_candidates(&mut selected, &unavailable, &mut report)?;
                        fill_selection(
                            &mut selected,
                            &mut reserve,
                            self.limits.max_transactions.get(),
                            exact_payload_limit,
                            &mut report,
                        );
                        continue;
                    }
                };
            validate_prepared_work(request.context, view, &descriptors, &prepared_work)?;

            report.selected = selected.len();
            if !request.allow_empty_recovery_heartbeat
                && !candidate_has_proposal_work(&selected, &request.attachments, &prepared_work)
            {
                if request.queue.transaction_selection_durability_faulted() {
                    return Err(CandidateError::RestartRequired);
                }
                validate_request(&request)?;
                return Ok(CandidateAssemblyOutcome::NoProposalWork(report));
            }

            let signing = request
                .output_guard
                .begin_fail_stop_operation()
                .ok_or(CandidateError::RestartRequired)?;
            let (block, canonical_wire, events) = self.build_block(
                request.context,
                tag,
                request.local_validator,
                request.parent,
                request.state,
                request.key_pair,
                &request.attachments,
                &selected,
                &prepared_work,
            )?;

            let chunk_count = encoded_chunk_count(request.context.da_layout, canonical_wire.len())?;
            let within_size = canonical_wire.len() <= exact_payload_limit;
            let within_chunks = chunk_count
                <= usize::try_from(request.context.da_layout.max_chunk_count).unwrap_or(usize::MAX);
            if !within_size || !within_chunks {
                if selected.pop().is_some() {
                    signing.complete();
                    report.payload_deferred = report.payload_deferred.saturating_add(1);
                    // Do not replace an exact-limit trim with later queue work:
                    // retaining a canonical prefix guarantees progress and a
                    // strict bound on signing/encoding attempts.
                    continue;
                }
                return Err(CandidateError::HeartbeatExceedsPayloadLimits {
                    encoded_bytes: canonical_wire.len(),
                    encoded_chunks: chunk_count,
                    max_bytes: exact_payload_limit,
                    max_chunks: request.context.da_layout.max_chunk_count,
                });
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
            let selected_hashes = selected
                .iter()
                .map(|record| record.transaction.hash())
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
        let certified_merge_filter = attachments
            .certified_merge_entry
            .as_ref()
            .and_then(|entry| entry.execution_batch.as_ref())
            .map(|batch| {
                (
                    batch.application_block_header.creation_time(),
                    batch
                        .lanes
                        .iter()
                        .flat_map(|lane| lane.entrypoint_hashes.iter().copied())
                        .collect::<BTreeSet<_>>(),
                )
            });

        let mut records = Vec::with_capacity(pending.len());
        for (source_ordinal, transaction) in pending.into_iter().enumerate() {
            report.inspected = report.inspected.saturating_add(1);
            if certified_merge_filter
                .as_ref()
                .is_some_and(|(application_time, entrypoints)| {
                    transaction_conflicts_with_certified_merge(
                        transaction.creation_time(),
                        Hash::from(transaction.hash_as_entrypoint()),
                        *application_time,
                        entrypoints,
                    )
                })
            {
                report.work_deferred = report.work_deferred.saturating_add(1);
                continue;
            }
            let routing_plan = match queue.route_plan_with_state(&transaction, state) {
                Ok(plan) => plan,
                Err(_) => {
                    report.unresolved = report.unresolved.saturating_add(1);
                    return Err(CandidateError::RestartRequired);
                }
            };
            if queue.transaction_selection_durability_faulted() {
                return Err(CandidateError::RestartRequired);
            }
            report.routable = report.routable.saturating_add(1);
            let encoded_len = transaction.encoded_len();
            if encoded_len > payload_limit {
                report.payload_deferred = report.payload_deferred.saturating_add(1);
                continue;
            }
            records.push(CandidateRecord {
                entrypoint_hash: transaction.hash_as_entrypoint(),
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
    fn build_block(
        &self,
        context: &wire::HeightContext,
        tag: EventTag,
        local_validator: wire::ValidatorIndex,
        parent: CandidateParent<'_>,
        state: &State,
        key_pair: &KeyPair,
        attachments: &CandidateAttachments,
        selected: &[CandidateRecord],
        prepared_work: &PreparedCandidateWork,
    ) -> Result<(SignedBlock, Vec<u8>, Vec<PipelineEventBox>), CandidateError> {
        let transactions = selected
            .iter()
            .map(|candidate| candidate.transaction.clone())
            .collect::<Vec<_>>();
        let pending = BlockBuilder::new_with_time_source(transactions, self.time_source.clone());
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
            .with_previous_roster_evidence(attachments.previous_roster_evidence.clone())
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
            .with_lane_payload_ownerships(prepared_work.lane_payload_ownerships.clone());
        if let Some(entry) = attachments.certified_merge_entry.as_ref() {
            execution_context =
                execution_context.with_merge_entry(CertifiedMergeLedgerReference::new(entry));
        }
        builder = builder
            .with_execution_context((!execution_context.is_empty()).then_some(execution_context));

        let mut events = Vec::new();
        let new_block = builder
            .try_sign_with_index(key_pair.private_key(), u64::from(local_validator))
            .map_err(|error| CandidateError::Signing(error.to_string()))?
            .unpack(|event| events.push(event));
        let block: SignedBlock = new_block.into();
        if block.header().height().get() != context.height
            || block.header().view_change_index() != tag.view()
            || block.header().prev_block_hash() != Some(parent.hash())
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

fn candidate_has_proposal_work(
    selected: &[CandidateRecord],
    attachments: &CandidateAttachments,
    prepared_work: &PreparedCandidateWork,
) -> bool {
    !selected.is_empty()
        || !prepared_work.autonomous_lane_payloads.is_empty()
        || attachments.time_trigger_clock_progress_required
        || attachments.da_commitments.is_some()
        || attachments.da_pin_intents.is_some()
        || attachments.previous_roster_evidence.is_some()
        || attachments.npos_consensus_effects.is_some()
        || attachments.sccp_commitment_root.is_some()
        || attachments.certified_merge_carrier_header.is_some()
        || attachments.certified_merge_entry.is_some()
}

fn stripped_carrier_context_matches(
    built_header: &BlockHeader,
    certified_header: &BlockHeader,
) -> bool {
    certified_header.merkle_root().is_none()
        && certified_header.result_merkle_root().is_none()
        && built_header.height() == certified_header.height()
        && built_header.prev_block_hash() == certified_header.prev_block_hash()
        && built_header.creation_time() == certified_header.creation_time()
        && built_header.view_change_index() == certified_header.view_change_index()
}

fn transaction_conflicts_with_certified_merge(
    creation_time: Duration,
    entrypoint_hash: Hash,
    application_time: Duration,
    certified_entrypoints: &BTreeSet<Hash>,
) -> bool {
    creation_time >= application_time || certified_entrypoints.contains(&entrypoint_hash)
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

    let parent_height = validate_candidate_parent(request.context, request.parent, request.state)?;

    if let Some(evidence) = &request.attachments.previous_roster_evidence
        && (evidence.height != parent_height || evidence.block_hash != request.parent.hash())
    {
        return Err(CandidateError::PreviousRosterEvidenceMismatch);
    }
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
        && state_view.chain_id() == &context.chain_id;
    drop(state_view);
    if !state_matches {
        return Err(CandidateError::ParentStateMismatch);
    }
    Ok(parent_height)
}

fn canonicalize_records(records: &mut [CandidateRecord]) {
    records.sort_by(|left, right| {
        left.entrypoint_hash
            .cmp(&right.entrypoint_hash)
            .then_with(|| left.source_ordinal.cmp(&right.source_ordinal))
    });
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

fn validate_prepared_work(
    context: &wire::HeightContext,
    view: wire::View,
    candidates: &[CandidateDescriptor<'_>],
    prepared: &PreparedCandidateWork,
) -> Result<(), CandidateError> {
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
    let expected_chain_id_hash = Hash::new(context.chain_id.as_str().as_bytes());
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
            expected_chain_id_hash,
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
    /// Previous-roster audit evidence references another parent.
    #[error("Sumeragi v2 previous-roster evidence does not match the parent block")]
    PreviousRosterEvidenceMismatch,
    /// Work provider returned no indices or a blank reason.
    #[error("Sumeragi v2 work provider returned a malformed unavailable-work result")]
    MalformedUnavailableWork,
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
    /// BlockBuilder output order drifted from execution-context order.
    #[error("built Sumeragi v2 entrypoint order differs from its routing contexts")]
    BuiltEntrypointOrderMismatch,
    /// BlockBuilder unexpectedly attached deterministic execution output.
    #[error("built Sumeragi v2 candidate is not resultless")]
    BuiltResultBearingProposal,
    /// Canonical block framing failed.
    #[error("failed to encode canonical Sumeragi v2 body: {0}")]
    CanonicalEncoding(String),
    /// Even an empty heartbeat exceeds the immutable height limits.
    #[error(
        "empty Sumeragi v2 heartbeat needs {encoded_bytes} bytes/{encoded_chunks} chunks, exceeding {max_bytes} bytes/{max_chunks} chunks"
    )]
    HeartbeatExceedsPayloadLimits {
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
mod tests {
    use std::{
        borrow::Cow,
        num::{NonZeroU64, NonZeroUsize},
        sync::Arc,
    };

    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
    use iroha_data_model::{
        ChainId,
        account::AccountId,
        block::consensus::{LaneBlockDescriptorV1, LaneBlockProposalV1},
        consensus::{VALIDATOR_SET_HASH_VERSION_V1, ValidatorSetCheckpoint},
        nexus::{DataSpaceId, LaneId},
        peer::PeerId,
        transaction::TransactionBuilder,
    };
    use nonzero_ext::nonzero;

    use super::*;
    use crate::{
        block::ValidBlock,
        kura::Kura,
        query::store::LiveQueryStore,
        queue::{LaneQueueReservationKeyV2, RouteLeg, RouteLegRole, RoutingDecision},
        state::{State, World},
        sumeragi::network_topology::Topology,
    };

    fn nonzero(value: usize) -> NonZeroUsize {
        NonZeroUsize::new(value).expect("test value is non-zero")
    }

    fn accepted(seed: u8, _label: &str) -> AcceptedTransaction<'static> {
        let key = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
            .expect("deterministic transaction key");
        let authority = AccountId::new(key.public_key().clone());
        let chain_id: ChainId = "v2-candidate-test".parse().expect("chain id");
        let tx = TransactionBuilder::new(
            chain_id,
            authority,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .sign(key.private_key());
        AcceptedTransaction::new_unchecked(Cow::Owned(tx))
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
            validator_count: u32::try_from(validator_set.len()).expect("validator count fits"),
            min_quorum: 2,
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
        let producer = validator_set[0].clone();
        let producer_key = keypairs
            .iter()
            .find(|key| key.public_key() == producer.public_key())
            .expect("producer belongs to fixture validator set");
        let routing_plan = RoutingPlan::single(RoutingDecision::new(lane_id, dataspace_id));
        let reservation = LaneQueueReservationKeyV2 {
            version: LaneQueueReservationKeyV2::VERSION,
            signed_transaction_hash: transaction.hash(),
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
        let chain_id_hash = Hash::new(context.chain_id.as_str().as_bytes());
        let payload = crate::lane_consensus::LaneExecutablePayloadV1::new_signed_with_reservations(
            chain_id_hash,
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
        crate::lane_consensus::autonomous_lane_payload_envelope(
            &payload,
            chain_id_hash,
            context.epoch,
        )
        .expect("construct valid autonomous candidate envelope")
    }

    fn snapshot_parent_fixture() -> (
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
        let state = State::new_with_chain_for_testing(
            World::new(),
            Arc::clone(&kura),
            LiveQueryStore::start_test(),
            ChainId::from("v2-candidate-snapshot-parent"),
        );
        let mut parent_hash = None;
        for height in 1..=2 {
            let block = ValidBlock::new_dummy_and_modify_header(key.private_key(), |header| {
                header.set_height(NonZeroU64::new(height).expect("non-zero fixture height"));
                header.set_prev_block_hash(parent_hash);
                header.creation_time_ms = height;
                header.merkle_root = None;
            })
            .commit_unchecked()
            .unpack(|_| {});
            parent_hash = Some(block.as_ref().hash());
            let mut state_block = state.block(block.as_ref().header());
            let _events = state_block.apply_without_execution(&block, topology.as_ref().to_owned());
            state_block.commit().expect("commit fixture parent state");
        }
        let anchor = wire::SnapshotBootstrapAnchor {
            snapshot_height: 2,
            snapshot_block_hash: parent_hash.expect("fixture parent hash"),
            snapshot_block_creation_time_ms: 2,
            snapshot_state_hash: Hash::new(b"candidate snapshot state"),
        };
        let roster = voters
            .into_iter()
            .map(|validator| wire::ValidatorPower {
                validator,
                power: 1,
            })
            .collect::<Vec<_>>();
        let context = wire::HeightContext {
            chain_id: state.chain_id_ref().clone(),
            protocol_version: wire::PROTOCOL_VERSION,
            height: 3,
            epoch: 0,
            epoch_end_height: u64::MAX,
            next_epoch_snapshot: None,
            mode: wire::ConsensusMode::Permissioned,
            parent_commit_qc: None,
            snapshot_bootstrap: Some(anchor),
            quorum: wire::DualQuorum::from_roster(&roster).expect("fixture quorum"),
            roster,
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
        (state, context, anchor, key)
    }

    fn assemble_empty_snapshot_candidate(
        allow_empty_recovery_heartbeat: bool,
        attachments: CandidateAttachments,
    ) -> CandidateAssemblyOutcome {
        let (state, mut context, anchor, key) = snapshot_parent_fixture();
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
            state: &state,
            queue: &queue,
            key_pair: &key,
            output_guard: &output_guard,
            attachments,
            allow_empty_recovery_heartbeat,
            work_provider: SingleRouteWorkProvider,
        })
        .expect("empty snapshot candidate assembly")
    }

    #[test]
    fn proposal_work_gate_defers_idle_candidate() {
        let outcome = assemble_empty_snapshot_candidate(false, CandidateAttachments::default());
        let CandidateAssemblyOutcome::NoProposalWork(report) = outcome else {
            panic!("an idle height must not manufacture an empty candidate");
        };
        assert_eq!(report, CandidateScanReport::default());
    }

    #[test]
    fn proposal_work_gate_preserves_armed_recovery_heartbeat() {
        let outcome = assemble_empty_snapshot_candidate(true, CandidateAttachments::default());
        let CandidateAssemblyOutcome::Assembled(candidate) = outcome else {
            panic!("an explicitly armed recovery heartbeat must remain available");
        };
        assert_eq!(candidate.scan_report(), CandidateScanReport::default());
        assert_eq!(candidate.block().external_entrypoints_cloned().count(), 0);
    }

    #[test]
    fn proposal_work_gate_preserves_time_trigger_work() {
        let outcome = assemble_empty_snapshot_candidate(
            false,
            CandidateAttachments {
                time_trigger_clock_progress_required: true,
                ..CandidateAttachments::default()
            },
        );
        let CandidateAssemblyOutcome::Assembled(candidate) = outcome else {
            panic!("due time-trigger work must produce a candidate");
        };
        assert_eq!(candidate.scan_report(), CandidateScanReport::default());
        assert_eq!(candidate.block().external_entrypoints_cloned().count(), 0);
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

        let parent_hash =
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x42; 32]));
        let validator_key = KeyPair::try_from_seed(vec![40; 32], Algorithm::Ed25519)
            .expect("deterministic validator key");
        let roster_evidence = CandidateAttachments {
            previous_roster_evidence: Some(PreviousRosterEvidence {
                height: 1,
                block_hash: parent_hash,
                validator_checkpoint: ValidatorSetCheckpoint::new(
                    1,
                    0,
                    parent_hash,
                    Hash::prehashed([0x12; 32]),
                    Hash::prehashed([0x34; 32]),
                    vec![PeerId::from(validator_key.public_key().clone())],
                    vec![1],
                    Vec::new(),
                    VALIDATOR_SET_HASH_VERSION_V1,
                    None,
                ),
                stake_snapshot: None,
            }),
            ..CandidateAttachments::default()
        };
        assert!(candidate_has_proposal_work(
            &[],
            &roster_evidence,
            &prepared
        ));
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
        })
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
    fn canonical_order_matches_block_builder_hash_order() {
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

        canonicalize_records(&mut selected);
        assert!(selected.windows(2).all(|window| {
            (window[0].entrypoint_hash, window[0].source_ordinal)
                <= (window[1].entrypoint_hash, window[1].source_ordinal)
        }));
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
    fn autonomous_anchors_validate_without_ordinary_candidates() {
        let (_state, context, _anchor, _key) = snapshot_parent_fixture();
        let first_tx = accepted(31, "autonomous-one");
        let second_tx = accepted(32, "autonomous-two");
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
        let first_tx = accepted(33, "autonomous-order-one");
        let second_tx = accepted(34, "autonomous-order-two");
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
        let ordinary = record(35, "global-overlap", 0);
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

        let shared_tx = accepted(36, "cross-lane-duplicate");
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
        let transaction = accepted(37, "autonomous-authentication");
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
        let transaction = accepted(38, "autonomous-bounds");
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
        canonicalize_records(&mut selected);
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
    fn certified_merge_filter_defers_time_boundary_and_duplicate_entrypoints() {
        let application_time = Duration::from_millis(1_000);
        let duplicate = Hash::new(b"certified merge duplicate entrypoint");
        let unrelated = Hash::new(b"ordinary queue entrypoint");
        let certified_entrypoints = BTreeSet::from([duplicate]);

        assert!(!transaction_conflicts_with_certified_merge(
            Duration::from_millis(999),
            unrelated,
            application_time,
            &certified_entrypoints,
        ));
        assert!(transaction_conflicts_with_certified_merge(
            Duration::from_millis(999),
            duplicate,
            application_time,
            &certified_entrypoints,
        ));
        assert!(transaction_conflicts_with_certified_merge(
            application_time,
            unrelated,
            application_time,
            &certified_entrypoints,
        ));
        assert!(transaction_conflicts_with_certified_merge(
            Duration::from_millis(1_001),
            unrelated,
            application_time,
            &certified_entrypoints,
        ));
    }

    #[test]
    fn certified_merge_carrier_context_rejects_timestamp_view_and_root_drift() {
        let parent = HashOf::from_untyped_unchecked(Hash::new(b"candidate carrier context parent"));
        let built = BlockHeader::new(nonzero!(7_u64), Some(parent), None, None, 1_000, 3);
        assert!(stripped_carrier_context_matches(&built, &built));

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
