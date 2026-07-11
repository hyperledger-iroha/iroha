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
};

use super::v2_core::EventTag;
use iroha_crypto::{Hash, HashOf, KeyPair};
use iroha_data_model::{
    block::{
        BlockExecutionContextBundle, SignedBlock,
        consensus::{NativeAmxReceipt, SumeragiLanePayloadOwnership},
        consensus_v2 as wire,
    },
    consensus::{NposConsensusEffects, PreviousRosterEvidence},
    da::{commitment::DaCommitmentBundle, pin_intent::DaPinIntentBundle},
    events::pipeline::PipelineEventBox,
    transaction::{SignedTransaction, TransactionEntrypoint},
};
use iroha_primitives::time::TimeSource;
use thiserror::Error;

use super::{
    v2::LocalProposalDirective,
    v2_chunks::{EncodedV2Payload, encode_payload},
};
use crate::{
    block::BlockBuilder,
    queue::{Queue, RoutingPlan, execution_context_for_routing_plan},
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

    /// Maximum number of external transactions in one candidate.
    pub(crate) const fn max_transactions(self) -> NonZeroUsize {
        self.max_transactions
    }

    /// Maximum exact canonical body size in bytes.
    pub(crate) const fn max_payload_bytes(self) -> NonZeroUsize {
        self.max_payload_bytes
    }

    /// Maximum number of pending queue entries inspected per attempt.
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
}

/// Read-only description of one canonically ordered proposal candidate.
#[derive(Clone, Copy, Debug)]
pub(crate) struct CandidateDescriptor<'candidate> {
    transaction: &'candidate AcceptedTransaction<'static>,
    routing_plan: &'candidate RoutingPlan,
    entrypoint_hash: HashOf<TransactionEntrypoint>,
}

impl<'candidate> CandidateDescriptor<'candidate> {
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

/// Lane-local and Native AMX material aligned with a candidate descriptor list.
#[derive(Clone, Debug, Default)]
pub(crate) struct PreparedCandidateWork {
    /// One receipt slot per descriptor. Native AMX plans require `Some` and
    /// single-route plans require `None`.
    pub(crate) native_amx_receipts: Vec<Option<NativeAmxReceipt>>,
    /// Optional lane-local certified ownerships covering the descriptor list.
    pub(crate) lane_payload_ownerships: Vec<SumeragiLanePayloadOwnership>,
}

impl PreparedCandidateWork {
    /// Construct work for a batch containing only available single-route entries.
    #[must_use]
    pub(crate) fn single_route_batch(candidate_count: usize) -> Self {
        Self {
            native_amx_receipts: vec![None; candidate_count],
            lane_payload_ownerships: Vec::new(),
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
/// from this candidate; queue ownership is never changed.
pub(crate) trait CandidateWorkProvider {
    /// Prepare receipts and lane-local ownership commitments for `candidates`.
    fn prepare(
        &mut self,
        context: &wire::HeightContext,
        view: wire::View,
        candidates: &[CandidateDescriptor<'_>],
    ) -> Result<PreparedCandidateWork, CandidateWorkUnavailable>;
}

/// Conservative provider used when no certified Native AMX snapshot exists.
///
/// Single-route transactions remain eligible. Native AMX transactions are
/// reported unavailable and therefore remain in the queue without preventing
/// an honest leader from producing a heartbeat or single-route block.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct SingleRouteWorkProvider;

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
    /// Exact committed parent body.
    pub(crate) parent: &'request SignedBlock,
    /// Committed state at the parent height.
    pub(crate) state: &'request State,
    /// Shared pending queue; selection is read-only.
    pub(crate) queue: &'request Queue,
    /// Consensus key corresponding to `local_validator`.
    pub(crate) key_pair: &'request KeyPair,
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
    /// Entries skipped because certified lane/AMX work was unavailable.
    pub(crate) work_deferred: usize,
    /// External transactions included in the final body.
    pub(crate) selected: usize,
}

/// A canonical successor body and its deterministic v2 dispersal plan.
#[derive(Debug)]
pub(crate) struct AssembledV2Candidate {
    tag: EventTag,
    block: SignedBlock,
    canonical_wire: Vec<u8>,
    encoded_payload: EncodedV2Payload,
    selected_transaction_hashes: Vec<HashOf<SignedTransaction>>,
    selected_entrypoint_hashes: Vec<HashOf<TransactionEntrypoint>>,
    events: Vec<PipelineEventBox>,
    scan_report: CandidateScanReport,
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

    /// Exact canonical `SignedBlockWire` bytes committed by the manifest.
    pub(crate) fn canonical_wire(&self) -> &[u8] {
        &self.canonical_wire
    }

    /// Borrow the deterministic manifest and chunk sequence.
    pub(crate) const fn encoded_payload(&self) -> &EncodedV2Payload {
        &self.encoded_payload
    }

    /// Queue transaction hashes included in the candidate.
    pub(crate) fn selected_transaction_hashes(&self) -> &[HashOf<SignedTransaction>] {
        &self.selected_transaction_hashes
    }

    /// Canonical external-entrypoint order committed by the candidate.
    pub(crate) fn selected_entrypoint_hashes(&self) -> &[HashOf<TransactionEntrypoint>] {
        &self.selected_entrypoint_hashes
    }

    /// Pipeline events emitted when the block was constructed.
    pub(crate) fn events(&self) -> &[PipelineEventBox] {
        &self.events
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
    ) {
        (
            self.block,
            self.canonical_wire,
            self.encoded_payload,
            self.events,
            self.scan_report,
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
    /// lane/AMX snapshot, or a batch whose transactions do not fit produces a
    /// valid empty heartbeat body as long as the body-level size limit permits
    /// it.
    ///
    /// # Errors
    ///
    /// Returns [`CandidateError`] for a stale reducer directive, a non-leader
    /// caller, parent/context drift, malformed certified work, signing failure,
    /// or a heartbeat which itself exceeds frozen body/chunk limits.
    pub(crate) fn assemble<Work: CandidateWorkProvider>(
        &self,
        mut request: CandidateRequest<'_, Work>,
    ) -> Result<AssembledV2Candidate, CandidateError> {
        validate_request(&request)?;

        let tag = request.directive.tag();
        let view = tag.view();
        let exact_payload_limit = self.limits.max_payload_bytes.get().min(
            usize::try_from(request.context.da_layout.max_payload_size_bytes).unwrap_or(usize::MAX),
        );
        let mut report = CandidateScanReport::default();
        let mut pool = self.snapshot_routable_candidates(
            request.queue,
            request.state,
            exact_payload_limit,
            &mut report,
        );
        canonicalize_records(&mut pool);

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
            let descriptors = selected
                .iter()
                .map(CandidateRecord::descriptor)
                .collect::<Vec<_>>();
            let prepared_work = if descriptors.is_empty() {
                PreparedCandidateWork::default()
            } else {
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
                }
            };
            validate_prepared_work(request.context, view, &descriptors, &prepared_work)?;

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
            return Ok(AssembledV2Candidate {
                tag,
                block,
                canonical_wire,
                encoded_payload,
                selected_transaction_hashes: selected
                    .iter()
                    .map(|candidate| candidate.transaction.hash())
                    .collect(),
                selected_entrypoint_hashes: selected
                    .iter()
                    .map(|candidate| candidate.entrypoint_hash)
                    .collect(),
                events,
                scan_report: report,
            });
        }

        Err(CandidateError::AssemblyDidNotConverge)
    }

    fn snapshot_routable_candidates(
        &self,
        queue: &Queue,
        state: &State,
        payload_limit: usize,
        report: &mut CandidateScanReport,
    ) -> Vec<CandidateRecord> {
        let state_view = state.view();
        let pending = queue.bounded_pending_snapshot(&state_view, self.limits.max_queue_scan);
        drop(state_view);

        let mut records = Vec::with_capacity(pending.len());
        for (source_ordinal, transaction) in pending.into_iter().enumerate() {
            report.inspected = report.inspected.saturating_add(1);
            let routing_plan = match queue.route_plan_with_state(&transaction, state) {
                Ok(plan) => plan,
                Err(_) => {
                    report.unresolved = report.unresolved.saturating_add(1);
                    continue;
                }
            };
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
        records
    }

    #[allow(clippy::too_many_arguments)]
    fn build_block(
        &self,
        context: &wire::HeightContext,
        tag: EventTag,
        local_validator: wire::ValidatorIndex,
        parent: &SignedBlock,
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
        let mut builder =
            BlockBuilder::new_with_time_source(transactions, self.time_source.clone())
                .chain(tag.view(), Some(parent));

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
        let execution_context = BlockExecutionContextBundle::new(execution_context)
            .with_lane_payload_ownerships(prepared_work.lane_payload_ownerships.clone());
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

    let parent_height = request.parent.header().height().get();
    if parent_height.checked_add(1) != Some(request.context.height)
        || request.parent.hash()
            != request
                .context
                .parent_commit_qc
                .as_ref()
                .ok_or(CandidateError::MissingParentCertificate)?
                .subject
                .block_hash
    {
        return Err(CandidateError::ParentContextMismatch);
    }
    let state_view = request.state.view();
    let state_matches = state_view.height() == usize::try_from(parent_height).unwrap_or(usize::MAX)
        && state_view.latest_block_hash() == Some(request.parent.hash())
        && state_view.chain_id() == &request.context.chain_id;
    drop(state_view);
    if !state_matches {
        return Err(CandidateError::ParentStateMismatch);
    }

    if let Some(evidence) = &request.attachments.previous_roster_evidence
        && (evidence.height != parent_height || evidence.block_hash != request.parent.hash())
    {
        return Err(CandidateError::PreviousRosterEvidenceMismatch);
    }
    Ok(())
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
    match layout.encoding {
        wire::PayloadEncoding::Plain => Ok(data_chunks),
        wire::PayloadEncoding::ReedSolomon16 => {
            let data_shards = usize::from(layout.data_shards);
            let parity_shards = usize::from(layout.parity_shards);
            if data_shards == 0 || parity_shards == 0 || !chunk_size.is_multiple_of(2) {
                return Err(CandidateError::InvalidDataAvailabilityLayout);
            }
            let stripes = data_chunks.div_ceil(data_shards);
            stripes
                .checked_mul(data_shards.saturating_add(parity_shards))
                .ok_or(CandidateError::InvalidDataAvailabilityLayout)
        }
    }
}

/// Candidate construction failure.
#[derive(Debug, Error)]
pub(crate) enum CandidateError {
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
    #[error("failed to sign Sumeragi v2 candidate: {0}")]
    Signing(String),
    /// Built header drifted from context/tag/parent inputs.
    #[error("built Sumeragi v2 candidate header differs from immutable inputs")]
    BuiltHeaderMismatch,
    /// BlockBuilder output order drifted from execution-context order.
    #[error("built Sumeragi v2 entrypoint order differs from its routing contexts")]
    BuiltEntrypointOrderMismatch,
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
    use std::{borrow::Cow, num::NonZeroUsize};

    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_data_model::{
        ChainId,
        account::AccountId,
        nexus::{DataSpaceId, LaneId},
        transaction::TransactionBuilder,
    };

    use super::*;
    use crate::queue::{RouteLeg, RouteLegRole, RoutingDecision};

    fn nonzero(value: usize) -> NonZeroUsize {
        NonZeroUsize::new(value).expect("test value is non-zero")
    }

    fn accepted(seed: u8, _label: &str) -> AcceptedTransaction<'static> {
        let key = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
            .expect("deterministic transaction key");
        let authority = AccountId::new(key.public_key().clone());
        let chain_id: ChainId = "v2-candidate-test".parse().expect("chain id");
        let tx = TransactionBuilder::new(chain_id, authority).sign(key.private_key());
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
    fn canonical_order_is_entrypoint_hash_then_source_ordinal() {
        let mut records = vec![
            record(3, "third", 2),
            record(1, "first", 0),
            record(2, "second", 1),
        ];
        canonicalize_records(&mut records);
        assert!(records.windows(2).all(|window| {
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
        assert_eq!(
            unavailable_native_amx_indices(&candidates),
            BTreeSet::from([1])
        );
    }

    #[test]
    fn chunk_count_matches_plain_and_rs16_stripes() {
        let plain = wire::DataAvailabilityLayout {
            encoding: wire::PayloadEncoding::Plain,
            chunk_size_bytes: 8,
            data_shards: 0,
            parity_shards: 0,
            max_payload_size_bytes: 1024,
            max_chunk_count: 1024,
        };
        assert_eq!(encoded_chunk_count(plain, 17).expect("plain count"), 3);

        let rs = wire::DataAvailabilityLayout {
            encoding: wire::PayloadEncoding::ReedSolomon16,
            chunk_size_bytes: 8,
            data_shards: 4,
            parity_shards: 2,
            max_payload_size_bytes: 1024,
            max_chunk_count: 1024,
        };
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
}
