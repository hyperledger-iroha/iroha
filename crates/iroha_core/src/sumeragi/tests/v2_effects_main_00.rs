use super::*;
use crate::sumeragi::{
    InboundBlockMessage,
    message::BlockMessage,
    v2::{
        AdapterEquivocationEvidence, AdapterError, AdapterFingerprints,
        DecisionLocalProposalDisposition, DeferredAdmissionOrdinalSource, SumeragiV2Adapter,
        VerifiedHeightContext, classify_decided_local_proposal,
    },
    v2_block_sync::{CommitCertificateAdmissionError, V2BlockSyncDiscovery},
    v2_core::Generation,
    v2_lifecycle_coordinator::{
        CertifiedFetchReadyPublicationError, LifecycleDigest, LifecyclePhase, LifecycleState,
        ProductionIngressCapacityRetry, ProductionIngressCapacityStatus,
        ProductionIngressSchedulerInputsError, ProductionIngressTurnPreparation,
        ProductionRecoveredLifecycleSignDispatchErrorV1, WaitSource,
    },
    v2_runtime::{RuntimeLifecycleOrdinalSource, RuntimeQueueConfig},
};
use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, Signature, SignatureOf};
use iroha_data_model::{
    block::{BlockHeader, BlockSignature, SignedBlock, consensus_v2 as wire},
    merge::MergeQuorumCertificate,
    peer::PeerId,
};
use std::{collections::VecDeque, num::NonZeroU64, sync::Arc};
use tempfile::TempDir;
#[test]
fn post_finality_cleanup_accumulates_typed_warnings_in_order() {
    let mut outcome = PostFinalityCleanupOutcome::default();
    outcome.record(PostFinalityCleanupTarget::SafetyWal, "WAL directory sync");
    outcome.record(
        PostFinalityCleanupTarget::DurableBodies,
        "body worker disconnected",
    );
    outcome.record(
        PostFinalityCleanupTarget::PayloadChunks,
        "chunk root retained",
    );
    assert_eq!(outcome.warnings().len(), 3);
    assert_eq!(
        outcome
            .warnings()
            .iter()
            .map(PostFinalityCleanupWarning::target)
            .collect::<Vec<_>>(),
        vec![
            PostFinalityCleanupTarget::SafetyWal,
            PostFinalityCleanupTarget::DurableBodies,
            PostFinalityCleanupTarget::PayloadChunks,
        ]
    );
    assert_eq!(outcome.warnings()[0].reason(), "WAL directory sync");
    assert_eq!(
        PostFinalityCleanupTarget::CleanupWorker.as_str(),
        "cleanup_worker"
    );
}
#[derive(Clone, Debug, PartialEq, Eq)]
enum RuntimeCompletion {
    BodyAvailable(EventTag, wire::PayloadManifest),
    BodyStored(
        EventTag,
        wire::ConsensusRound,
        wire::BlockSubject,
        DurableBodyReceipt,
    ),
    ValidationSucceeded(
        EventTag,
        wire::ConsensusRound,
        wire::BlockSubject,
        ValidatedBodyReceipt,
    ),
    ValidationFailed(EventTag, wire::ConsensusRound, wire::BlockSubject),
    Signature(EventTag, Vec<u8>),
    Application(EventTag, wire::BlockSubject),
    LocalProposal(
        EventTag,
        wire::PayloadManifest,
        DurableBodyReceipt,
        ValidatedBodyReceipt,
    ),
}
#[derive(Default)]
struct FakeRuntime {
    steps: VecDeque<Result<RuntimeStep<AdapterEffect>, String>>,
    completions: Vec<RuntimeCompletion>,
    validation_completion_ownerships: Vec<RuntimeEffectOwnership>,
    bound_validations: Vec<(wire::PayloadManifest, ValidatedBodyReceipt)>,
    reserved_body_available: Option<BodyAvailableReservation>,
    decided_body: Option<DurableDecision>,
    decision_on_next_step: Option<DurableDecision>,
    round_tag: Option<EventTag>,
    locked_body: Option<(wire::ConsensusRound, wire::BlockSubject)>,
    fail_enqueue: bool,
    fail_enqueue_hits: usize,
    certified_fence_escape_credit: bool,
    panic_step: bool,
    scheduler_ownership_ready: bool,
    omit_scheduler_ownership: bool,
    reject_scheduler_ownership: bool,
    next_lifecycle_ordinal: u128,
    effect_ownership_calls: usize,
    effect_owners: BTreeMap<Hash, RuntimeEffectOwnership>,
    local_proposal_intent_owners:
        BTreeMap<(EventTag, HashOf<wire::PayloadManifest>), RuntimeEffectOwnership>,
    terminal_body_candidate_owners: BTreeMap<Hash, RuntimeEffectOwnership>,
    terminal_body_candidate_commits: usize,
    external_lifecycle_owners: Vec<RuntimeLifecycleOwner>,
    external_lifecycle_owner_capacity: Option<usize>,
    active_view_producer_retained: bool,
    completed_proposal_fanouts: Vec<(wire::ConsensusRound, RuntimeEffectOwnership)>,
    leader_wire_terminal_batches: VecDeque<Vec<LeaderWireRuntimeTerminal>>,
    leader_wire_terminal_after_lock: Option<LeaderWireRuntimeTerminal>,
    leader_wire_terminal_after_decision: Option<LeaderWireRuntimeTerminal>,
}
/// Actual executor/runtime ownership projected without retaining a shadow
/// state machine. Fatal fail-stop metadata is deliberately excluded: a
/// rejected admission may latch it, but no body owner or accounting value
/// may change.
#[derive(Clone, Debug, PartialEq, Eq)]
struct BodyOwnershipProjection {
    next_work_id: u64,
    pending_fetches: BTreeMap<EffectWorkId, PendingFetch>,
    pending_stores: BTreeMap<EffectWorkId, PendingStore>,
    pending_validations: BTreeMap<EffectWorkId, PendingValidation>,
    deferred_merge_work: BTreeMap<EffectWorkId, HashOf<MergeLedgerEntry>>,
    body_pipeline_owners: BTreeMap<(wire::ConsensusRound, wire::BlockSubject), BodyPipelineOwner>,
    certified_work: BTreeMap<HashOf<wire::CertifiedBodyRequest>, EffectWorkId>,
    outstanding_request_hashes: BTreeSet<HashOf<wire::CertifiedBodyRequest>>,
    ready_bodies: BTreeMap<(wire::ConsensusRound, wire::BlockSubject), ReadyBody>,
    authenticated_genesis_body: Option<(wire::BlockSubject, Arc<[u8]>)>,
    retained_locked_body: Option<(wire::BlockSubject, Arc<[u8]>)>,
    ready_body_bytes: u64,
    pending_store_bytes: u64,
    recovered_bodies: BTreeMap<
        (wire::ConsensusRound, wire::BlockSubject),
        (wire::PayloadManifest, DurableBodyReceipt),
    >,
    durable_bodies: BTreeMap<(wire::ConsensusRound, wire::BlockSubject), DurableBodyReceipt>,
    validated_bodies: BTreeMap<(wire::ConsensusRound, wire::BlockSubject), ValidatedBodyReceipt>,
    rejected_bodies: BTreeMap<(wire::ConsensusRound, wire::BlockSubject), DurableBodyReceipt>,
    runtime_completions: Vec<RuntimeCompletion>,
    runtime_bound_validations: Vec<(wire::PayloadManifest, ValidatedBodyReceipt)>,
    runtime_body_reservation: Option<BodyAvailableReservation>,
}
impl V2EffectExecutor<FakeRuntime> {
    fn body_ownership_projection(&self) -> BodyOwnershipProjection {
        BodyOwnershipProjection {
            next_work_id: self.next_work_id,
            pending_fetches: self.pending_fetches.clone(),
            pending_stores: self.pending_stores.clone(),
            pending_validations: self.pending_validations.clone(),
            deferred_merge_work: self.deferred_merge_work.clone(),
            body_pipeline_owners: self.body_pipeline_owners.clone(),
            certified_work: self.certified_work.clone(),
            outstanding_request_hashes: self.outstanding_requests.hashes(),
            ready_bodies: self.ready_bodies.clone(),
            authenticated_genesis_body: self.authenticated_genesis_body.clone(),
            retained_locked_body: self.retained_locked_body.clone(),
            ready_body_bytes: self.ready_body_bytes,
            pending_store_bytes: self.pending_store_bytes,
            recovered_bodies: self.recovered_bodies.clone(),
            durable_bodies: self.durable_bodies.clone(),
            validated_bodies: self.validated_bodies.clone(),
            rejected_bodies: self.rejected_bodies.clone(),
            runtime_completions: self.runtime.completions.clone(),
            runtime_bound_validations: self.runtime.bound_validations.clone(),
            runtime_body_reservation: self.runtime.reserved_body_available.clone(),
        }
    }
}
impl FakeRuntime {
    fn push(&mut self, completion: RuntimeCompletion) -> Result<(), EnqueueError> {
        if self.fail_enqueue {
            self.fail_enqueue_hits = self.fail_enqueue_hits.saturating_add(1);
            return Err(EnqueueError::Full);
        }
        self.completions.push(completion);
        Ok(())
    }
    fn test_effect_ownership(&mut self, effect: &AdapterEffect) -> RuntimeEffectOwnership {
        let mut identity = Vec::new();
        match effect {
            AdapterEffect::FetchBody { round, subject, .. }
            | AdapterEffect::StoreBody { round, subject, .. }
            | AdapterEffect::ValidateBody { round, subject, .. } => {
                identity.extend_from_slice(b"body-pipeline");
                identity.extend_from_slice(&round.encode());
                identity.extend_from_slice(&subject.encode());
            }
            AdapterEffect::Sign { request, .. } => {
                identity.extend_from_slice(b"sign");
                identity.extend_from_slice(&request.signature_preimage());
            }
            AdapterEffect::Apply {
                subject,
                certificate,
                ..
            } => {
                identity.extend_from_slice(b"apply");
                identity.extend_from_slice(&subject.encode());
                identity.extend_from_slice(&certificate.as_ref().encode());
            }
            AdapterEffect::Broadcast(message) => {
                identity.extend_from_slice(b"broadcast");
                identity.extend_from_slice(&message.encode());
            }
            AdapterEffect::EnterView { tag, .. } => {
                identity.extend_from_slice(b"enter-view");
                identity.extend_from_slice(&tag.height().to_le_bytes());
                identity.extend_from_slice(&tag.view().to_le_bytes());
            }
            AdapterEffect::ReportEquivocation { .. }
            | AdapterEffect::ReportInvalidCertifiedBody { .. } => {
                identity.extend_from_slice(format!("{effect:?}").as_bytes());
            }
        }
        let identity = Hash::new(identity);
        if let Some(existing) = self.effect_owners.get(&identity) {
            return existing.clone();
        }
        let tag = self
            .round_tag
            .unwrap_or_else(|| EventTag::new(1, 0, Generation::new(0)));
        let ownership = RuntimeEffectOwnership::fresh_for_test(tag, self.next_lifecycle_ordinal);
        self.next_lifecycle_ordinal = self.next_lifecycle_ordinal.saturating_add(1);
        self.effect_owners.insert(identity, ownership.clone());
        ownership
    }
}
impl EffectRuntime for FakeRuntime {
    fn can_admit_network_message_with_ingress_ownership(
        &self,
        message: &wire::ConsensusMessageV2,
        ingress_ownership: &FairV2IngressOwnershipEvidence,
    ) -> bool {
        ingress_ownership.validate_exact()
            && ingress_ownership.matches_message(&BlockMessage::V2(message.clone()))
    }

    fn can_admit_timeout_vote_recovery_episode(
        &self,
        _message: &wire::ConsensusMessageV2,
        _ingress_ownership: &FairV2IngressOwnershipEvidence,
    ) -> bool {
        false
    }

    fn step_effects(&mut self, _now: Instant) -> Result<RuntimeStep<AdapterEffect>, String> {
        assert!(!self.panic_step, "model safety-WAL step panic");
        if self.scheduler_ownership_ready {
            return Err("fake runtime scheduler owner was not consumed".to_owned());
        }
        let step = self.steps.pop_front().unwrap_or(Ok(RuntimeStep::Idle));
        if matches!(&step, Ok(RuntimeStep::Advanced(_)))
            && let Some(decision) = self.decision_on_next_step.take()
        {
            self.decided_body = Some(decision);
        }
        if step.is_ok() && !self.omit_scheduler_ownership {
            self.scheduler_ownership_ready = true;
        }
        step
    }
    fn step_recovery_effects(
        &mut self,
        now: Instant,
    ) -> Result<RuntimeStep<AdapterEffect>, String> {
        self.step_effects(now)
    }
    fn take_effect_ownership(
        &mut self,
        effects: &[AdapterEffect],
    ) -> Result<Vec<RuntimeEffectOwnership>, String> {
        self.effect_ownership_calls = self.effect_ownership_calls.saturating_add(1);
        if effects.is_empty() {
            return Ok(Vec::new());
        }
        let mut ownership = Vec::with_capacity(effects.len());
        for effect in effects {
            let local = match effect {
                AdapterEffect::Sign {
                    tag,
                    request: SignRequest::Proposal(proposal),
                } => self
                    .local_proposal_intent_owners
                    .get(&(*tag, HashOf::new(&proposal.manifest)))
                    .cloned(),
                _ => None,
            };
            ownership.push(local.unwrap_or_else(|| self.test_effect_ownership(effect)));
        }
        bind_adapter_effect_batch_ownership(effects, ownership)
    }
    fn take_leader_wire_runtime_terminals(
        &mut self,
    ) -> Result<Vec<LeaderWireRuntimeTerminal>, String> {
        Ok(self
            .leader_wire_terminal_batches
            .pop_front()
            .unwrap_or_default())
    }
    fn set_external_lifecycle_owners(
        &mut self,
        owners: Vec<RuntimeLifecycleOwner>,
    ) -> Result<(), String> {
        if self
            .external_lifecycle_owner_capacity
            .is_some_and(|capacity| owners.len() > capacity)
        {
            return Err("fake external lifecycle-owner capacity exceeded".to_owned());
        }
        self.external_lifecycle_owners = owners;
        Ok(())
    }
    fn configure_external_lifecycle_owner_capacity(
        &mut self,
        max_pending_work: usize,
    ) -> Result<(), String> {
        let retained_capacity = MAX_EFFECTS_PER_STEP
            .checked_mul(2)
            .ok_or_else(|| "fake external lifecycle-owner capacity overflowed".to_owned())?;
        self.external_lifecycle_owner_capacity = Some(
            max_pending_work
                .checked_add(retained_capacity)
                .ok_or_else(|| "fake external lifecycle-owner capacity overflowed".to_owned())?,
        );
        Ok(())
    }
    fn reconcile_active_view_producer(
        &mut self,
        _tag: EventTag,
        retain: bool,
    ) -> Result<(), String> {
        self.active_view_producer_retained = retain;
        Ok(())
    }
    fn complete_active_view_producer_after_proposal_fanout(
        &mut self,
        proposal_round: wire::ConsensusRound,
        ownership: &RuntimeEffectOwnership,
    ) -> Result<(), String> {
        if self.active_view_producer_retained {
            self.active_view_producer_retained = false;
            self.completed_proposal_fanouts
                .push((proposal_round, ownership.clone()));
        }
        Ok(())
    }
    fn mint_local_proposal_effect_ownership(
        &mut self,
        tag: EventTag,
        manifest: &wire::PayloadManifest,
    ) -> Result<LocalProposalEffectOwnership, String> {
        let mut semantic = Vec::from(b"body-pipeline".as_slice());
        semantic.extend_from_slice(&manifest.round.encode());
        semantic.extend_from_slice(&manifest.subject.encode());
        let identity = Hash::new(semantic);
        let ownership = if let Some(existing) = self.effect_owners.get(&identity) {
            existing.clone()
        } else {
            let ownership =
                RuntimeEffectOwnership::fresh_for_test(tag, self.next_lifecycle_ordinal);
            self.next_lifecycle_ordinal = self.next_lifecycle_ordinal.saturating_add(1);
            self.effect_owners.insert(identity, ownership.clone());
            ownership
        };
        let effect = AdapterEffect::StoreBody {
            tag,
            round: manifest.round,
            subject: manifest.subject,
        };
        let ownership =
            bind_adapter_effect_batch_ownership(std::slice::from_ref(&effect), vec![ownership])?
                .pop()
                .ok_or_else(|| "fake local proposal StoreBody binding was empty".to_owned())?;
        LocalProposalEffectOwnership::for_test(ownership, &effect, manifest).ok_or_else(|| {
            "fake local proposal replay seal did not match its Store owner".to_owned()
        })
    }
    fn take_scheduler_ownership(&mut self) -> Result<(), String> {
        if self.reject_scheduler_ownership {
            return Err("fake runtime scheduler owner was invalid".to_owned());
        }
        if !self.scheduler_ownership_ready {
            return Err("fake runtime scheduler owner was missing".to_owned());
        }
        self.scheduler_ownership_ready = false;
        Ok(())
    }
    fn authoritative_tag(&self) -> Option<EventTag> {
        self.round_tag
    }
    fn reconciliation_frontier(&self) -> Result<RuntimeReconciliationFrontier, String> {
        Ok(RuntimeReconciliationFrontier {
            tag: self.round_tag,
            locked_body: self.locked_body,
            lock_is_authoritative: self.locked_body.is_some(),
            decision: self.decided_body,
        })
    }
    fn decided_body(&self) -> Result<Option<DurableDecision>, String> {
        Ok(self.decided_body)
    }
    fn bind_validated_body(
        &mut self,
        manifest: &wire::PayloadManifest,
        validated_receipt: &ValidatedBodyReceipt,
    ) -> Result<(), String> {
        let durable = validated_receipt.durable();
        if durable.round() != manifest.round
            || durable.subject() != manifest.subject
            || durable.manifest_hash() != HashOf::new(manifest)
        {
            return Err("fake validated binding differs from its manifest".to_owned());
        }
        if let Some((bound_manifest, bound_receipt)) =
            self.bound_validations.iter().find(|(bound_manifest, _)| {
                bound_manifest.round == manifest.round && bound_manifest.subject == manifest.subject
            })
        {
            return (bound_manifest == manifest && bound_receipt == validated_receipt)
                .then_some(())
                .ok_or_else(|| "fake validated binding conflicts with authority".to_owned());
        }
        self.bound_validations
            .push((manifest.clone(), validated_receipt.clone()));
        Ok(())
    }
    fn reserve_body_available(
        &mut self,
        tag: EventTag,
        manifest: wire::PayloadManifest,
    ) -> Result<BodyAvailableReservation, EnqueueError> {
        let mut logical_owners = 0usize;
        let mut exact_owners = 0usize;
        for completion in &self.completions {
            if let RuntimeCompletion::BodyAvailable(queued_tag, queued_manifest) = completion
                && *queued_tag == tag
                && queued_manifest.round == manifest.round
                && queued_manifest.subject == manifest.subject
            {
                logical_owners = logical_owners.saturating_add(1);
                exact_owners =
                    exact_owners.saturating_add(usize::from(queued_manifest == &manifest));
            }
        }
        match (logical_owners, exact_owners) {
            (1, 1) => return Ok(BodyAvailableReservation::coalesced(tag, manifest)),
            (0, 0) => {}
            _ => return Err(EnqueueError::DuplicateCompletionOwnership),
        }
        if let Some(existing) = &self.reserved_body_available {
            // A protecting EnterView retags the physical reservation
            // without replacing its immutable lifecycle root.  Rebuild-
            // and-compare would mint a test-only root at the rebound tag
            // and misclassify that exact retry as duplicate ownership.
            // Match the public token coordinates instead and return the
            // incumbent token byte-for-byte, as production ingress does.
            if existing.tag() == tag && existing.manifest() == &manifest {
                return Ok(existing.clone());
            }
            return Err(EnqueueError::DuplicateCompletionOwnership);
        }
        if self.fail_enqueue {
            self.fail_enqueue_hits = self.fail_enqueue_hits.saturating_add(1);
            return Err(EnqueueError::Full);
        }
        if self.completions.len() >= 16 {
            return Err(EnqueueError::Full);
        }
        let reservation = BodyAvailableReservation::reserved(tag, manifest);
        self.reserved_body_available = Some(reservation.clone());
        Ok(reservation)
    }
    fn commit_body_available(
        &mut self,
        reservation: BodyAvailableReservation,
    ) -> Result<(), EnqueueError> {
        if !reservation.owns_new_slot() {
            return Ok(());
        }
        if self.reserved_body_available.as_ref() != Some(&reservation) {
            return Err(EnqueueError::FailClosed);
        }
        self.reserved_body_available = None;
        self.completions.push(RuntimeCompletion::BodyAvailable(
            reservation.tag(),
            reservation.manifest().clone(),
        ));
        Ok(())
    }
    fn abort_body_available(&mut self, reservation: BodyAvailableReservation) {
        let _retained_exact_owner = !reservation.owns_new_slot()
            || self.reserved_body_available.as_ref() == Some(&reservation);
    }
    fn rebind_body_available(
        &mut self,
        previous: EventTag,
        rebound: EventTag,
        manifest: &wire::PayloadManifest,
    ) -> Result<bool, String> {
        let mut rebound_count = 0usize;
        for completion in &mut self.completions {
            if let RuntimeCompletion::BodyAvailable(tag, queued) = completion
                && *tag == previous
                && queued == manifest
            {
                *tag = rebound;
                rebound_count = rebound_count.saturating_add(1);
            }
        }
        if let Some(reservation) = &mut self.reserved_body_available
            && reservation.rebind_consumer_if_exact(previous, rebound, manifest)
        {
            rebound_count = rebound_count.saturating_add(1);
        }
        if rebound_count > 1 {
            return Err("duplicate queued body-available completions".to_owned());
        }
        Ok(rebound_count == 1)
    }
    fn rebind_unpublished_body_available(
        &mut self,
        previous: EventTag,
        rebound: EventTag,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
    ) -> Result<bool, String> {
        let Some(manifest) = self
            .reserved_body_available
            .as_ref()
            .filter(|reservation| {
                reservation.tag() == previous
                    && reservation.manifest().round == round
                    && reservation.manifest().subject == subject
            })
            .map(|reservation| reservation.manifest().clone())
        else {
            return Ok(false);
        };
        if self.completions.iter().any(|completion| {
            matches!(
                completion,
                RuntimeCompletion::BodyAvailable(tag, queued)
                    if *tag == rebound && queued == &manifest
            )
        }) {
            return Err("unpublished body completion already has a destination owner".to_owned());
        }
        self.rebind_body_available(previous, rebound, &manifest)
    }
    fn retire_unpublished_body_available(
        &mut self,
        tag: EventTag,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
    ) -> Result<bool, String> {
        let Some(manifest) = self
            .reserved_body_available
            .as_ref()
            .filter(|reservation| {
                reservation.tag() == tag
                    && reservation.manifest().round == round
                    && reservation.manifest().subject == subject
            })
            .map(|reservation| reservation.manifest().clone())
        else {
            return Ok(false);
        };
        self.retire_body_available(tag, &manifest)
    }
    fn retire_body_available(
        &mut self,
        tag: EventTag,
        manifest: &wire::PayloadManifest,
    ) -> Result<bool, String> {
        let before = self.completions.len();
        self.completions.retain(|completion| {
            !matches!(
                completion,
                RuntimeCompletion::BodyAvailable(queued_tag, queued_manifest)
                    if *queued_tag == tag && queued_manifest == manifest
            )
        });
        let mut retired = before.saturating_sub(self.completions.len());
        if self
            .reserved_body_available
            .as_ref()
            .is_some_and(|reservation| {
                reservation.tag() == tag && reservation.manifest() == manifest
            })
        {
            self.reserved_body_available = None;
            retired = retired.saturating_add(1);
        }
        if retired > 1 {
            return Err("duplicate queued body-available completions".to_owned());
        }
        Ok(retired == 1)
    }
    fn retire_body_pipeline_completions(
        &mut self,
        tag: EventTag,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
    ) -> Result<RetiredBodyPipelineCompletions, String> {
        let mut retired = RetiredBodyPipelineCompletions::default();
        self.completions.retain(|completion| {
            let remove = match completion {
                RuntimeCompletion::BodyAvailable(queued_tag, manifest)
                    if *queued_tag == tag
                        && manifest.round == round
                        && manifest.subject == subject =>
                {
                    retired.record_body_available();
                    true
                }
                RuntimeCompletion::BodyStored(queued_tag, queued_round, queued_subject, _)
                    if *queued_tag == tag
                        && *queued_round == round
                        && *queued_subject == subject =>
                {
                    retired.record_body_stored();
                    true
                }
                RuntimeCompletion::ValidationSucceeded(
                    queued_tag,
                    queued_round,
                    queued_subject,
                    _,
                )
                | RuntimeCompletion::ValidationFailed(queued_tag, queued_round, queued_subject)
                    if *queued_tag == tag
                        && *queued_round == round
                        && *queued_subject == subject =>
                {
                    retired.record_validation();
                    true
                }
                RuntimeCompletion::LocalProposal(queued_tag, manifest, ..)
                    if *queued_tag == tag
                        && manifest.round == round
                        && manifest.subject == subject =>
                {
                    retired.record_local_proposal();
                    true
                }
                RuntimeCompletion::BodyAvailable(..)
                | RuntimeCompletion::BodyStored(..)
                | RuntimeCompletion::ValidationSucceeded(..)
                | RuntimeCompletion::ValidationFailed(..)
                | RuntimeCompletion::Signature(..)
                | RuntimeCompletion::Application(..)
                | RuntimeCompletion::LocalProposal(..) => false,
            };
            !remove
        });
        if self
            .reserved_body_available
            .as_ref()
            .is_some_and(|reservation| {
                reservation.tag() == tag
                    && reservation.manifest().round == round
                    && reservation.manifest().subject == subject
            })
        {
            if retired.body_available() {
                return Err("duplicate body-available completion owners".to_owned());
            }
            self.reserved_body_available = None;
            retired.record_body_available();
        }
        Ok(retired)
    }
    fn retire_unsafe_proposals_for_lock(
        &mut self,
        _locked_round: wire::ConsensusRound,
        _locked_subject: wire::BlockSubject,
    ) -> Result<usize, String> {
        if let Some(terminal) = self.leader_wire_terminal_after_lock.take() {
            self.leader_wire_terminal_batches.push_back(vec![terminal]);
        }
        Ok(0)
    }
    fn retire_proposal_work_after_decision(
        &mut self,
        decision_round: wire::ConsensusRound,
        decision_subject: wire::BlockSubject,
        decision_commitment: wire::ExecutionCommitment,
    ) -> Result<DecisionProposalRetirement, String> {
        let decision_tag = self.round_tag.unwrap_or_else(|| {
            EventTag::new(
                decision_round.height,
                decision_round.view,
                Generation::new(7),
            )
        });
        let mut retainable = 0usize;
        let mut recovery_only = 0usize;
        let mut conflicting = 0usize;
        for completion in &self.completions {
            let RuntimeCompletion::LocalProposal(
                queued_tag,
                manifest,
                durable_receipt,
                validated_receipt,
            ) = completion
            else {
                continue;
            };
            match classify_decided_local_proposal(
                *queued_tag,
                manifest,
                durable_receipt,
                validated_receipt,
                decision_tag,
                decision_round,
                decision_subject,
                decision_commitment,
            ) {
                Some(DecisionLocalProposalDisposition::Retain) => {
                    retainable = retainable.saturating_add(1);
                }
                Some(DecisionLocalProposalDisposition::RetireForRecovery) => {
                    recovery_only = recovery_only.saturating_add(1);
                }
                Some(DecisionLocalProposalDisposition::Conflict) => {
                    conflicting = conflicting.saturating_add(1);
                }
                None => {}
            }
        }
        if conflicting != 0 {
            return Err(
                "decided local-proposal evidence conflicts with the durable Decision".to_owned(),
            );
        }
        if retainable.saturating_add(recovery_only) > 1 {
            return Err("duplicate exact decided local-proposal completions".to_owned());
        }
        self.completions.retain(|completion| {
            if let RuntimeCompletion::LocalProposal(
                queued_tag,
                manifest,
                durable_receipt,
                validated_receipt,
            ) = completion
                && manifest.round.height == decision_round.height
            {
                return matches!(
                    classify_decided_local_proposal(
                        *queued_tag,
                        manifest,
                        durable_receipt,
                        validated_receipt,
                        decision_tag,
                        decision_round,
                        decision_subject,
                        decision_commitment,
                    ),
                    Some(DecisionLocalProposalDisposition::Retain)
                );
            }
            true
        });
        if let Some(terminal) = self.leader_wire_terminal_after_decision.take() {
            self.leader_wire_terminal_batches.push_back(vec![terminal]);
        }
        Ok(DecisionProposalRetirement::new(
            (retainable == 1).then_some(decision_tag),
            recovery_only,
        ))
    }
    fn enqueue_body_stored(
        &mut self,
        tag: EventTag,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
        receipt: DurableBodyReceipt,
    ) -> Result<(), EnqueueError> {
        self.push(RuntimeCompletion::BodyStored(tag, round, subject, receipt))
    }
    fn enqueue_validation_succeeded(
        &mut self,
        tag: EventTag,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
        receipt: ValidatedBodyReceipt,
    ) -> Result<(), EnqueueError> {
        self.push(RuntimeCompletion::ValidationSucceeded(
            tag, round, subject, receipt,
        ))
    }
    fn enqueue_validation_succeeded_with_owner(
        &mut self,
        tag: EventTag,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
        receipt: ValidatedBodyReceipt,
        ownership: &RuntimeEffectOwnership,
    ) -> Result<(), EnqueueError> {
        self.enqueue_validation_succeeded(tag, round, subject, receipt)?;
        self.validation_completion_ownerships
            .push(ownership.clone());
        Ok(())
    }
    fn enqueue_validation_failed(
        &mut self,
        tag: EventTag,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
    ) -> Result<(), EnqueueError> {
        self.push(RuntimeCompletion::ValidationFailed(tag, round, subject))
    }
    fn enqueue_validation_failed_with_owner(
        &mut self,
        tag: EventTag,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
        ownership: &RuntimeEffectOwnership,
    ) -> Result<(), EnqueueError> {
        self.enqueue_validation_failed(tag, round, subject)?;
        self.validation_completion_ownerships
            .push(ownership.clone());
        Ok(())
    }
    fn enqueue_validation_failures_atomically(
        &mut self,
        failures: &[(EventTag, wire::ConsensusRound, wire::BlockSubject)],
    ) -> Result<(), EnqueueError> {
        if self.fail_enqueue {
            self.fail_enqueue_hits = self.fail_enqueue_hits.saturating_add(1);
            return Err(EnqueueError::Full);
        }
        self.completions.extend(
            failures.iter().copied().map(|(tag, round, subject)| {
                RuntimeCompletion::ValidationFailed(tag, round, subject)
            }),
        );
        Ok(())
    }
    fn enqueue_signature(&mut self, tag: EventTag, signature: Vec<u8>) -> Result<(), EnqueueError> {
        self.push(RuntimeCompletion::Signature(tag, signature))
    }
    fn enqueue_application_completed(
        &mut self,
        tag: EventTag,
        subject: wire::BlockSubject,
    ) -> Result<(), EnqueueError> {
        self.push(RuntimeCompletion::Application(tag, subject))
    }
    fn enqueue_local_proposal(
        &mut self,
        tag: EventTag,
        manifest: wire::PayloadManifest,
        durable_receipt: DurableBodyReceipt,
        validated_receipt: ValidatedBodyReceipt,
    ) -> Result<(), EnqueueError> {
        self.push(RuntimeCompletion::LocalProposal(
            tag,
            manifest,
            durable_receipt,
            validated_receipt,
        ))
    }
    fn enqueue_local_proposal_with_owner(
        &mut self,
        tag: EventTag,
        manifest: wire::PayloadManifest,
        durable_receipt: DurableBodyReceipt,
        validated_receipt: ValidatedBodyReceipt,
        ownership: &RuntimeEffectOwnership,
    ) -> Result<LocalProposalReadyCommandIdentity, EnqueueError> {
        let identity = LocalProposalReadyCommandIdentity::from_exact_handoff(
            tag,
            &manifest,
            &durable_receipt,
            &validated_receipt,
            ownership,
        )
        .ok_or(EnqueueError::FailClosed)?;
        let key = (tag, HashOf::new(&manifest));
        if matches!(
            self.local_proposal_intent_owners.entry(key),
            Entry::Occupied(_)
        ) {
            return Err(EnqueueError::FailClosed);
        }
        self.enqueue_local_proposal(tag, manifest.clone(), durable_receipt, validated_receipt)?;
        let key = (tag, HashOf::new(&manifest));
        let Entry::Vacant(slot) = self.local_proposal_intent_owners.entry(key) else {
            unreachable!("test runtime preflight proved the local owner slot vacant")
        };
        slot.insert(ownership.clone());
        Ok(identity)
    }
    fn verify_certificate(
        &self,
        context: &wire::HeightContext,
        certificate: &wire::QuorumCertificate,
    ) -> Result<(), String> {
        certificate
            .validate(context)
            .map_err(|error| error.to_string())
    }
    fn authenticate_certified_body_request(
        &self,
        context: &wire::HeightContext,
        request: wire::CertifiedBodyRequest,
        authenticated_requester: &PeerId,
    ) -> Result<AuthenticatedCertifiedBodyRequest, V2TransportError> {
        authenticate_certified_body_request(
            context,
            request,
            authenticated_requester,
            |context, certificate| self.verify_certificate(context, certificate),
        )
    }
    fn plan_body_pipeline_candidate_terminal(
        &mut self,
        effect: &AdapterEffect,
        ownership: &RuntimeEffectOwnership,
    ) -> Result<Option<RuntimeEffectOwnership>, String> {
        if !matches!(
            effect,
            AdapterEffect::StoreBody { .. } | AdapterEffect::ValidateBody { .. }
        ) {
            return Ok(None);
        }
        if !ownership.exactly_binds_adapter_effect(effect) {
            return Err("fake runtime terminal query received the wrong effect owner".to_owned());
        }
        let identity = ownership.candidate_semantic_identity().ok_or_else(|| {
            "fake runtime terminal query omitted the candidate identity".to_owned()
        })?;
        let Some(incumbent) = self.terminal_body_candidate_owners.get(&identity) else {
            return Ok(None);
        };
        incumbent
            .adopt_incumbent_body_stage_for_retry_or_authority(ownership, effect)
            .map(Some)
    }
    fn commit_body_pipeline_candidate_terminals(
        &mut self,
        terminals: &[(&AdapterEffect, &RuntimeEffectOwnership)],
    ) -> Result<(), String> {
        let mut identities = BTreeSet::new();
        let mut replacements = Vec::with_capacity(terminals.len());
        for (effect, ownership) in terminals {
            if !ownership.exactly_binds_adapter_effect(effect) {
                return Err(
                    "fake runtime terminal commit changed its exact effect binding".to_owned(),
                );
            }
            let identity = ownership.candidate_semantic_identity().ok_or_else(|| {
                "fake runtime terminal commit omitted the candidate identity".to_owned()
            })?;
            if !identities.insert(identity) {
                return Err(
                    "fake runtime terminal commit duplicated one terminal target".to_owned(),
                );
            }
            let Some(incumbent) = self.terminal_body_candidate_owners.get(&identity) else {
                return Err("fake runtime terminal commit changed its exact owner".to_owned());
            };
            if incumbent.owner() != ownership.owner()
                || incumbent
                    .adopt_incumbent_body_stage_for_retry_or_authority(ownership, effect)
                    .as_ref()
                    != Ok(*ownership)
            {
                return Err("fake runtime terminal commit changed its exact owner".to_owned());
            }
            replacements.push((identity, (**ownership).clone()));
        }
        for (identity, ownership) in replacements {
            let replaced = self
                .terminal_body_candidate_owners
                .insert(identity, ownership);
            debug_assert!(replaced.is_some());
        }
        self.terminal_body_candidate_commits = self
            .terminal_body_candidate_commits
            .saturating_add(terminals.len());
        Ok(())
    }
    fn queued_commands(&self) -> usize {
        self.completions.len()
    }
    fn remaining_completion_capacity(&self) -> usize {
        16usize.saturating_sub(
            self.completions
                .len()
                .saturating_add(usize::from(self.reserved_body_available.is_some())),
        )
    }
    fn has_certified_fence_escape_credit(&self) -> bool {
        self.certified_fence_escape_credit
    }
    fn queue_snapshot(&self, _now: Instant) -> RuntimeQueueSnapshot {
        let empty = RuntimeQueueLaneSnapshot {
            depth: 0,
            capacity: 16,
            oldest_age: None,
            max_service_debt: 0,
        };
        RuntimeQueueSnapshot {
            normal: empty,
            progress: empty,
            completion: RuntimeQueueLaneSnapshot {
                depth: self.completions.len(),
                ..empty
            },
        }
    }
    fn watchdog_threshold(&self) -> Duration {
        Duration::from_secs(12)
    }
}
#[derive(Default)]
struct FakeServices {
    _body_directory: Option<TempDir>,
    body_store: Option<V2BodyStore>,
    requester_key: Option<KeyPair>,
    effect_service_order: Vec<&'static str>,
    sign_tasks: Vec<ConsensusSignTask>,
    cancelled_signatures: Vec<EffectWorkId>,
    retired_outbound_subjects: Vec<wire::BlockSubject>,
    retired_all_outbound: usize,
    retired_candidate_work: usize,
    broadcast_dispositions: VecDeque<ConsensusBroadcastDisposition>,
    broadcast_attempts: Vec<wire::ConsensusMessageV2>,
    broadcasts: Vec<wire::ConsensusMessageV2>,
    fetch_tasks: Vec<BodyFetchTask>,
    cancelled_fetches: Vec<EffectWorkId>,
    completed_reconstruction_fetches: Vec<EffectWorkId>,
    completed_certified_fetches: Vec<EffectWorkId>,
    chunks: Vec<EffectWorkId>,
    reject_authenticated_chunks: bool,
    store_tasks: Vec<BodyStoreTask>,
    cancelled_stores: Vec<EffectWorkId>,
    inflight_stores: BTreeSet<EffectWorkId>,
    validation_tasks: Vec<BodyValidationTask>,
    cancelled_validations: Vec<EffectWorkId>,
    deferred_merge_sidecars: Vec<(
        EffectWorkId,
        wire::ConsensusRound,
        wire::BlockSubject,
        CertifiedMergeLedgerReference,
    )>,
    apply_tasks: Vec<ApplyTask>,
    entered_views: Vec<EventTag>,
    equivocations: Vec<wire::SumeragiV2Equivocation>,
    invalid_bodies: Vec<wire::BlockSubject>,
    rejected_validations: Vec<String>,
    statuses: Vec<EffectExecutorStatus>,
    closed: Vec<String>,
    fail_on: Option<&'static str>,
    fail_on_call: Option<(&'static str, usize)>,
    retry_certified_fetch_once: bool,
    operation_calls: BTreeMap<&'static str, usize>,
    validation_error: Option<String>,
    leader_wire_terminals: Vec<LeaderWireRuntimeTerminal>,
    durable_runtime_decision: Option<wire::BlockSubject>,
}
impl FakeServices {
    fn check(&mut self, operation: &'static str) -> Result<(), String> {
        let call = *self
            .operation_calls
            .entry(operation)
            .and_modify(|count| *count = count.saturating_add(1))
            .or_insert(1);
        if self.fail_on == Some(operation) {
            self.fail_on = None;
            Err(format!("{operation} failed"))
        } else if self.fail_on_call == Some((operation, call)) {
            self.fail_on_call = None;
            Err(format!("{operation} call {call} failed"))
        } else {
            Ok(())
        }
    }
    fn execute_store(&mut self, work_id: EffectWorkId) -> BodyStoreCompletion {
        let task = self
            .store_tasks
            .iter()
            .rev()
            .find(|task| task.id() == work_id)
            .expect("store task")
            .clone();
        self.body_store
            .as_mut()
            .expect("body store service")
            .execute_store_task(&task)
            .expect("execute durable store task")
    }
    fn execute_validation(&mut self, work_id: EffectWorkId) -> BodyValidationCompletion {
        let task = self
            .validation_tasks
            .iter()
            .rev()
            .find(|task| task.id() == work_id)
            .expect("validation task");
        let rejection = self.validation_error.clone();
        let execution_commitment = fixture_execution_commitment();
        self.body_store
            .as_mut()
            .expect("body store service")
            .execute_validation_task(task, move |_| match rejection {
                Some(reason) => Err(reason),
                None => Ok(execution_commitment),
            })
            .expect("execute deterministic validation task")
    }
}
impl V2EffectServices for FakeServices {
    type Error = String;
    fn finish_runtime_step_reconciliation(
        &mut self,
        decided_subject: Option<wire::BlockSubject>,
    ) -> Result<(), Self::Error> {
        self.check("finish-runtime-step-reconciliation")?;
        match (self.durable_runtime_decision, decided_subject) {
            (Some(retained), Some(observed)) if retained != observed => {
                return Err("one height published two durable Decision subjects".to_owned());
            }
            (Some(_), None) => {
                return Err("runtime lost its durable Decision after a WAL step".to_owned());
            }
            (None, Some(observed)) => self.durable_runtime_decision = Some(observed),
            (Some(_), Some(_)) | (None, None) => {}
        }
        Ok(())
    }
    fn complete_leader_wire_runtime_terminal(
        &mut self,
        terminal: LeaderWireRuntimeTerminal,
    ) -> Result<(), Self::Error> {
        self.check("leader-wire-terminal")?;
        self.leader_wire_terminals.push(terminal);
        Ok(())
    }
    fn enqueue_consensus_sign(&mut self, task: ConsensusSignTask) -> Result<(), Self::Error> {
        self.check("sign")?;
        self.effect_service_order.push("sign");
        self.sign_tasks.push(task);
        Ok(())
    }
    fn cancel_consensus_sign(&mut self, work_id: EffectWorkId) -> Result<(), Self::Error> {
        self.check("cancel-sign")?;
        self.cancelled_signatures.push(work_id);
        Ok(())
    }
    fn retire_outbound_payload_for_subject(
        &mut self,
        subject: wire::BlockSubject,
    ) -> Result<(), Self::Error> {
        self.check("retire-outbound-subject")?;
        self.retired_outbound_subjects.push(subject);
        Ok(())
    }
    fn retire_all_outbound_payloads(&mut self) -> Result<(), Self::Error> {
        self.check("retire-all-outbound")?;
        self.retired_all_outbound = self.retired_all_outbound.saturating_add(1);
        Ok(())
    }
    fn retire_candidate_work_after_decision(
        &mut self,
        _decision_round: wire::ConsensusRound,
        _decision_subject: wire::BlockSubject,
    ) -> Result<(), Self::Error> {
        self.check("retire-candidate-work")?;
        self.retired_candidate_work = self.retired_candidate_work.saturating_add(1);
        Ok(())
    }
    fn broadcast_consensus(
        &mut self,
        message: wire::ConsensusMessageV2,
    ) -> Result<ConsensusBroadcastDisposition, Self::Error> {
        self.check("broadcast")?;
        self.effect_service_order.push("broadcast");
        self.broadcast_attempts.push(message.clone());
        let disposition = self
            .broadcast_dispositions
            .pop_front()
            .unwrap_or(ConsensusBroadcastDisposition::ExactServiceAccepted);
        if disposition == ConsensusBroadcastDisposition::ExactServiceAccepted {
            self.broadcasts.push(message);
        }
        Ok(disposition)
    }
    fn sign_body_request(&mut self, preimage: &[u8]) -> Result<Vec<u8>, Self::Error> {
        self.check("body-sign")?;
        let key = self
            .requester_key
            .as_ref()
            .ok_or_else(|| "missing requester key".to_owned())?;
        Ok(Signature::new(key.private_key(), preimage)
            .payload()
            .to_vec())
    }
    fn enqueue_body_fetch(&mut self, task: BodyFetchTask) -> Result<(), Self::Error> {
        self.check("fetch")?;
        self.fetch_tasks.push(task);
        Ok(())
    }
    fn rebind_body_fetch(
        &mut self,
        previous: &BodyFetchTask,
        rebound: BodyFetchTask,
    ) -> Result<(), Self::Error> {
        self.check("rebind-fetch")?;
        if !rebound.rebinds_consumer_of(previous) {
            return Err("invalid body-fetch consumer rebind".to_owned());
        }
        let owned = self
            .fetch_tasks
            .iter_mut()
            .rev()
            .find(|task| task.id() == previous.id())
            .ok_or_else(|| "body-fetch consumer rebind has no service owner".to_owned())?;
        if owned != previous {
            return Err("body-fetch consumer rebind differs from service ownership".to_owned());
        }
        *owned = rebound;
        Ok(())
    }
    fn cancel_body_fetch(&mut self, task: &BodyFetchTask) -> Result<(), Self::Error> {
        self.check("cancel-fetch")?;
        self.cancelled_fetches.push(task.id());
        Ok(())
    }
    fn complete_body_reconstruction_fetch(
        &mut self,
        task: &BodyFetchTask,
    ) -> Result<(), Self::Error> {
        self.check("complete-reconstruction-fetch")?;
        self.completed_reconstruction_fetches.push(task.id());
        Ok(())
    }
    fn complete_certified_body_fetch(
        &mut self,
        task: &BodyFetchTask,
    ) -> Result<CertifiedBodyFetchCompletionDisposition, Self::Error> {
        if self.retry_certified_fetch_once {
            self.retry_certified_fetch_once = false;
            return Ok(CertifiedBodyFetchCompletionDisposition::Retryable);
        }
        self.check("complete-certified-fetch")?;
        self.completed_certified_fetches.push(task.id());
        Ok(CertifiedBodyFetchCompletionDisposition::Completed)
    }
    fn accept_authenticated_chunk(
        &mut self,
        task: &BodyFetchTask,
        _chunk: AuthenticatedPayloadChunk,
    ) -> Result<AuthenticatedChunkDisposition, Self::Error> {
        self.check("chunk")?;
        self.chunks.push(task.id());
        Ok(if self.reject_authenticated_chunks {
            AuthenticatedChunkDisposition::Rejected
        } else {
            AuthenticatedChunkDisposition::Accepted
        })
    }
    fn enqueue_body_store(&mut self, task: BodyStoreTask) -> Result<(), Self::Error> {
        self.check("store")?;
        self.store_tasks.push(task);
        Ok(())
    }
    fn cancel_body_store(&mut self, work_id: EffectWorkId) -> Result<bool, Self::Error> {
        self.check("cancel-store")?;
        self.cancelled_stores.push(work_id);
        Ok(!self.inflight_stores.contains(&work_id))
    }
    fn enqueue_body_validation(&mut self, task: BodyValidationTask) -> Result<(), Self::Error> {
        self.check("validation")?;
        self.validation_tasks.push(task);
        Ok(())
    }
    fn cancel_body_validation(&mut self, work_id: EffectWorkId) -> Result<(), Self::Error> {
        self.check("cancel-validation")?;
        self.cancelled_validations.push(work_id);
        Ok(())
    }
    fn work_deferred_for_merge_sidecar(
        &mut self,
        work_id: EffectWorkId,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
        reference: &CertifiedMergeLedgerReference,
    ) -> Result<(), Self::Error> {
        self.check("merge-sidecar")?;
        self.deferred_merge_sidecars
            .push((work_id, round, subject, reference.clone()));
        Ok(())
    }
    fn enqueue_apply(&mut self, task: ApplyTask) -> Result<(), Self::Error> {
        self.check("apply")?;
        self.apply_tasks.push(task);
        Ok(())
    }
    fn entered_view(
        &mut self,
        tag: EventTag,
        _certificate: wire::TimeoutCertificate,
    ) -> Result<(), Self::Error> {
        self.check("view")?;
        self.entered_views.push(tag);
        Ok(())
    }
    fn report_equivocation(
        &mut self,
        evidence: wire::SumeragiV2Equivocation,
    ) -> Result<(), Self::Error> {
        self.check("equivocation")?;
        self.effect_service_order.push("equivocation");
        self.equivocations.push(evidence);
        Ok(())
    }
    fn report_invalid_certified_body(
        &mut self,
        subject: wire::BlockSubject,
        _certificate: wire::QuorumCertificate,
    ) -> Result<(), Self::Error> {
        self.check("invalid-body")?;
        self.effect_service_order.push("invalid-body");
        self.invalid_bodies.push(subject);
        Ok(())
    }
    fn validation_rejected(
        &mut self,
        _round: wire::ConsensusRound,
        _subject: wire::BlockSubject,
        reason: &str,
    ) {
        self.rejected_validations.push(reason.to_owned());
    }
    fn publish_effect_status(&mut self, status: &EffectExecutorStatus) -> Result<(), Self::Error> {
        self.check("status")?;
        self.statuses.push(status.clone());
        Ok(())
    }
    fn fail_closed(&mut self, reason: &str) {
        self.closed.push(reason.to_owned());
    }
}
struct Fixture {
    context: wire::HeightContext,
    validator_keys: Vec<KeyPair>,
    requester_key: KeyPair,
    block: SignedBlock,
    body: Vec<u8>,
    encoded_chunks: Vec<Vec<u8>>,
    manifest: wire::PayloadManifest,
}
impl Fixture {
    fn new() -> Self {
        let mut validator_keys = (1_u8..=4)
            .map(|seed| {
                KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
                    .expect("deterministic validator key")
            })
            .collect::<Vec<_>>();
        validator_keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
        let roster = validator_keys
            .iter()
            .map(|key| wire::ValidatorPower {
                validator: PeerId::new(key.public_key().clone()),
                power: 1,
            })
            .collect::<Vec<_>>();
        let context = wire::HeightContext {
            network_id: crate::sumeragi::synthetic_network_id("v2-effect-executor-test"),
            protocol_version: wire::PROTOCOL_VERSION,
            height: 1,
            epoch: 0,
            epoch_end_height: 100,
            next_epoch_snapshot: None,
            mode: wire::ConsensusMode::Permissioned,
            parent_commit_qc: None,
            snapshot_bootstrap: None,
            roster: roster.clone(),
            quorum: wire::DualQuorum::from_roster(&roster).expect("quorum"),
            nexus_amx_context_hash: Hash::new(b"nexus amx context"),
            execution_policy_hash: iroha_crypto::Hash::new(b"test execution policy"),
            da_layout: wire::DataAvailabilityLayout {
                encoding: wire::PayloadEncoding::ReedSolomon16,
                chunk_size_bytes: wire::MAX_DA_CHUNK_SIZE_BYTES,
                data_shards: 1,
                parity_shards: 1,
                max_payload_size_bytes: u64::from(wire::MAX_DA_CHUNK_SIZE_BYTES),
                max_chunk_count: 2,
            },
            leader_seed: [0x33; 32],
        };
        let round = round(&context, 0);
        let header = BlockHeader::new(
            NonZeroU64::new(1).expect("height"),
            None,
            None,
            None,
            1_000,
            0,
        );
        let signature = SignatureOf::try_from_hash(validator_keys[0].private_key(), header.hash())
            .expect("block signature");
        let block = SignedBlock::presigned(BlockSignature::new(0, signature), header, Vec::new());
        let body = block.encode_wire().expect("canonical body");
        let subject = wire::BlockSubject {
            parent_block_hash: None,
            block_hash: block.hash(),
            payload_hash: Hash::new(&body),
        };
        let (manifest, encoded_chunks) = encode_payload(&context, round, subject, &body)
            .expect("encode the complete canonical fixture payload")
            .into_parts();
        let requester_key =
            KeyPair::try_from_seed(vec![99; 32], Algorithm::Ed25519).expect("requester key");
        Self {
            context,
            validator_keys,
            requester_key,
            block,
            body,
            encoded_chunks,
            manifest,
        }
    }
    fn services(&self) -> FakeServices {
        let directory = TempDir::new().expect("body-store directory");
        let body_store = V2BodyStore::open_with_policy(
            directory.path(),
            self.context.clone(),
            BlockSignaturePolicy::GenesisAuthority(self.validator_keys[0].public_key().clone()),
        )
        .expect("body-store service");
        FakeServices {
            _body_directory: Some(directory),
            body_store: Some(body_store),
            requester_key: Some(self.requester_key.clone()),
            ..FakeServices::default()
        }
    }
    fn executor(&self, config: EffectQueueConfig) -> V2EffectExecutor<FakeRuntime> {
        V2EffectExecutor::with_runtime(
            FakeRuntime {
                round_tag: Some(tag(0)),
                next_lifecycle_ordinal: 1,
                ..FakeRuntime::default()
            },
            BTreeMap::new(),
            self.context.clone(),
            PeerId::new(self.requester_key.public_key().clone()),
            Some(0),
            config,
        )
        .expect("effect executor")
    }
    fn qc(&self, phase: wire::GlobalPhase) -> wire::QuorumCertificate {
        wire::QuorumCertificate {
            round: self.manifest.round,
            proposal_round: self.manifest.round,
            phase,
            subject: self.manifest.subject,
            execution_commitment: fixture_execution_commitment(),
            signers: vec![0, 1, 2],
            aggregate_signature: vec![1],
        }
    }
}
struct ProductionTransportFixture {
    _directory: TempDir,
    context: wire::HeightContext,
    validator_keys: Vec<KeyPair>,
    requester_key: KeyPair,
    responder_key: KeyPair,
    round: wire::ConsensusRound,
    subject: wire::BlockSubject,
    body: Vec<u8>,
    manifest: wire::PayloadManifest,
    canonical_commitment: wire::ExecutionCommitment,
    conflicting_commitment: wire::ExecutionCommitment,
    lifecycle_ordinals: RuntimeLifecycleOrdinalSource,
    executor: V2EffectExecutor,
}
impl ProductionTransportFixture {
    fn new() -> Self {
        Self::new_with_local_validator(None)
    }
    fn new_validator() -> Self {
        Self::new_with_local_validator(Some(0))
    }
    fn new_with_local_validator(local_validator: Option<wire::ValidatorIndex>) -> Self {
        let mut validator_keys = (1_u8..=4)
            .map(|seed| {
                KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                    .expect("deterministic BLS validator key")
            })
            .collect::<Vec<_>>();
        validator_keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
        let roster = validator_keys
            .iter()
            .map(|key| wire::ValidatorPower {
                validator: PeerId::new(key.public_key().clone()),
                power: 1,
            })
            .collect::<Vec<_>>();
        let context = wire::HeightContext {
            network_id: crate::sumeragi::synthetic_network_id("v2-production-transport-regression"),
            protocol_version: wire::PROTOCOL_VERSION,
            height: 1,
            epoch: 0,
            epoch_end_height: 100,
            next_epoch_snapshot: None,
            mode: wire::ConsensusMode::Permissioned,
            parent_commit_qc: None,
            snapshot_bootstrap: None,
            quorum: wire::DualQuorum::from_roster(&roster).expect("equal-vote quorum"),
            roster,
            nexus_amx_context_hash: Hash::new(b"production transport nexus/amx context"),
            execution_policy_hash: iroha_crypto::Hash::new(b"test execution policy"),
            da_layout: wire::DataAvailabilityLayout {
                encoding: wire::PayloadEncoding::ReedSolomon16,
                chunk_size_bytes: wire::MAX_DA_CHUNK_SIZE_BYTES,
                data_shards: 1,
                parity_shards: 1,
                max_payload_size_bytes: u64::from(wire::MAX_DA_CHUNK_SIZE_BYTES),
                max_chunk_count: 2,
            },
            leader_seed: [0x62; 32],
        };
        let round = round(&context, 0);
        let header = BlockHeader::new(
            NonZeroU64::new(1).expect("height"),
            None,
            None,
            None,
            3_000,
            0,
        );
        let signature = SignatureOf::try_from_hash(validator_keys[0].private_key(), header.hash())
            .expect("production transport block signature");
        let block = SignedBlock::presigned(BlockSignature::new(0, signature), header, Vec::new());
        let body = block
            .encode_wire()
            .expect("production transport canonical block wire");
        let subject = wire::BlockSubject {
            parent_block_hash: None,
            block_hash: block.hash(),
            payload_hash: Hash::new(&body),
        };
        let manifest = canonical_payload_manifest(&context, round, subject, &body);
        let durable =
            DurableBodyReceipt::for_test(context.id(), round, subject, HashOf::new(&manifest));
        let validated = ValidatedBodyReceipt::for_test(durable.clone());
        let canonical_commitment = validated.execution_commitment();
        let conflicting_commitment = wire::ExecutionCommitment::without_topups_or_merge_carrier(
            Hash::new(b"conflicting parent state"),
            Hash::new(b"conflicting post state"),
            Hash::new(b"conflicting ordinary writes"),
            1,
            Hash::new(b"conflicting executed block wire"),
        );
        assert_ne!(canonical_commitment, conflicting_commitment);
        let proofs = validator_keys
            .iter()
            .map(|key| {
                iroha_crypto::bls_normal_pop_prove(key.private_key())
                    .expect("validator proof of possession")
            })
            .collect::<Vec<_>>();
        let verified =
            VerifiedHeightContext::genesis(context.clone(), proofs).expect("verified context");
        let directory = TempDir::new().expect("production runtime directory");
        let (adapter, startup_effects) = SumeragiV2Adapter::open(
            directory.path().join("transport-regression-safety.wal"),
            verified,
            local_validator,
            Generation::new(1),
            [0x63; 32],
            AdapterFingerprints {
                node: Hash::new(b"production transport node"),
                build: Hash::new(b"production transport build"),
                config: Hash::new(b"production transport config"),
            },
            DeferredAdmissionOrdinalSource::new(0),
        )
        .expect("open production adapter");
        assert!(startup_effects.is_empty());
        let started = Instant::now();
        let lifecycle_ordinals = RuntimeLifecycleOrdinalSource::after_high_watermark(0);
        let (mut runtime, startup_effects) = SerializedV2Runtime::new_with_lifecycle_ordinals(
            adapter,
            startup_effects,
            started,
            Duration::from_secs(10),
            RuntimeQueueConfig::default(),
            lifecycle_ordinals.clone(),
        )
        .expect("serialized production runtime");
        assert!(startup_effects.is_empty());
        runtime
            .recover_validated_body(&manifest, &validated)
            .expect("bind locally validated execution commitment");
        let requester_key = KeyPair::try_from_seed(vec![90; 32], Algorithm::BlsNormal)
            .expect("deterministic requester key");
        let responder_key = KeyPair::try_from_seed(vec![91; 32], Algorithm::BlsNormal)
            .expect("deterministic responder key");
        let recovered_bodies = BTreeMap::from([((round, subject), (manifest.clone(), durable))]);
        let executor = V2EffectExecutor::with_runtime(
            runtime,
            recovered_bodies,
            context.clone(),
            PeerId::new(requester_key.public_key().clone()),
            local_validator,
            EffectQueueConfig::default(),
        )
        .expect("production effect executor");
        Self {
            _directory: directory,
            context,
            validator_keys,
            requester_key,
            responder_key,
            round,
            subject,
            body,
            manifest,
            canonical_commitment,
            conflicting_commitment,
            lifecycle_ordinals,
            executor,
        }
    }
    fn quorum_certificate(
        &self,
        phase: wire::GlobalPhase,
        execution_commitment: wire::ExecutionCommitment,
    ) -> wire::QuorumCertificate {
        self.quorum_certificate_for(self.round, self.subject, phase, execution_commitment)
    }
    fn quorum_certificate_for(
        &self,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
        phase: wire::GlobalPhase,
        execution_commitment: wire::ExecutionCommitment,
    ) -> wire::QuorumCertificate {
        let signers = vec![0, 1, 2];
        let preimage = wire::Vote {
            round,
            proposal_round: round,
            phase,
            subject,
            execution_commitment,
            signer: signers[0],
            signature: Vec::new(),
        }
        .signature_preimage();
        let shares = signers
            .iter()
            .map(|signer| {
                Signature::new(
                    self.validator_keys[usize::try_from(*signer).expect("small signer index")]
                        .private_key(),
                    &preimage,
                )
                .payload()
                .to_vec()
            })
            .collect::<Vec<_>>();
        let share_refs = shares.iter().map(Vec::as_slice).collect::<Vec<_>>();
        wire::QuorumCertificate {
            round,
            proposal_round: round,
            phase,
            subject,
            execution_commitment,
            signers,
            aggregate_signature: iroha_crypto::bls_normal_aggregate_signatures(&share_refs)
                .expect("aggregate quorum certificate"),
        }
    }
    fn signed_normal_proposal(&self, ordinal: u64) -> wire::ConsensusMessageV2 {
        let mut body = b"production normal-ingress saturation body".to_vec();
        body.extend_from_slice(&ordinal.to_le_bytes());
        let mut block_preimage = b"production normal-ingress saturation block".to_vec();
        block_preimage.extend_from_slice(&ordinal.to_le_bytes());
        let subject = wire::BlockSubject {
            parent_block_hash: None,
            block_hash: HashOf::from_untyped_unchecked(Hash::new(block_preimage)),
            payload_hash: Hash::new(&body),
        };
        let manifest = canonical_payload_manifest(&self.context, self.round, subject, &body);
        let proposer = self.context.leader(self.round.view);
        let mut proposal = wire::Proposal {
            round: self.round,
            proposer,
            subject,
            manifest,
            justification: wire::ProposalJustification::ParentCommit(
                wire::ParentCommitJustification { certificate: None },
            ),
            signature: Vec::new(),
        };
        proposal.signature = Signature::new(
            self.validator_keys[usize::try_from(proposer).expect("small proposer index")]
                .private_key(),
            &proposal.signature_preimage(),
        )
        .payload()
        .to_vec();
        wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::Proposal(proposal))
    }
    fn signed_timeout_vote(&self, view: u64) -> wire::ConsensusMessageV2 {
        let mut vote = wire::TimeoutVote {
            round: round(&self.context, view),
            highest_prepare_qc: None,
            signer: 0,
            signature: Vec::new(),
        };
        vote.signature = Signature::new(
            self.validator_keys[0].private_key(),
            &vote.signature_preimage(),
        )
        .payload()
        .to_vec();
        wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::TimeoutVote(vote))
    }
    fn certified_sources(&self, _certificate: &wire::QuorumCertificate) -> Vec<PeerId> {
        self.context
            .roster
            .iter()
            .map(|entry| entry.validator.clone())
            .collect()
    }
    fn certified_body_request(
        &self,
        certificate: wire::QuorumCertificate,
    ) -> wire::CertifiedBodyRequest {
        let mut request = wire::CertifiedBodyRequest {
            round: self.round,
            subject: self.subject,
            certificate,
            requester: PeerId::new(self.requester_key.public_key().clone()),
            signature: Vec::new(),
        };
        request.signature = Signature::new(
            self.requester_key.private_key(),
            &request.signature_preimage(),
        )
        .payload()
        .to_vec();
        request
    }
}
#[test]
fn production_certified_body_request_rejects_locally_conflicting_qc_without_fail_close() {
    let fixture = ProductionTransportFixture::new();
    let certificate =
        fixture.quorum_certificate(wire::GlobalPhase::Prepare, fixture.conflicting_commitment);
    let request = fixture.certified_body_request(certificate);
    let requester = PeerId::new(fixture.requester_key.public_key().clone());
    assert!(matches!(
        fixture
            .executor
            .authenticate_certified_body_request(request, &requester),
        Err(V2TransportError::CertificateRejected(reason))
            if reason.contains("conflicting Sumeragi v2 execution commitments")
    ));
    assert!(fixture.executor.runtime.driver().ingress_ready());
    assert!(!fixture.executor.status().fail_closed);
}
#[test]
fn production_commit_certificate_response_conflict_keeps_discovery_outstanding_and_runtime_open() {
    let mut fixture = ProductionTransportFixture::new();
    let requester = PeerId::new(fixture.requester_key.public_key().clone());
    let mut discovery = V2BlockSyncDiscovery::new(fixture.context.clone(), requester, 1)
        .expect("current-height discovery");
    let request_envelope = discovery
        .begin(&fixture.requester_key)
        .expect("begin signed current-height request");
    let wire::ConsensusMessageV2Payload::CommitCertificateRequest(request) =
        request_envelope.payload
    else {
        panic!("discovery emits a CommitCertificateRequest")
    };
    let request_hash = HashOf::new(&request);
    let mut response = wire::CommitCertificateResponse {
        request_hash,
        certificate: fixture
            .quorum_certificate(wire::GlobalPhase::Commit, fixture.conflicting_commitment),
        responder: PeerId::new(fixture.responder_key.public_key().clone()),
        signature: Vec::new(),
    };
    response.signature = Signature::new(
        fixture.responder_key.private_key(),
        &response.signature_preimage(),
    )
    .payload()
    .to_vec();
    let responder = PeerId::new(fixture.responder_key.public_key().clone());
    let retry = response.clone();
    let discovered = discovery
        .authenticate_response(response, &responder)
        .expect("authenticate signed outer response");
    let admission = discovery.enqueue_and_complete(discovered, |message| {
        let reducer_admission = CommitCertificateReducerAdmission::for_test(&message);
        fixture
            .executor
            .enqueue_network(message)
            .map(|_| reducer_admission)
    });
    assert!(matches!(
        admission,
        Err(CommitCertificateAdmissionError::Enqueue(
            NetworkIngressError::Authentication(AdapterError::ConflictingExecutionCommitment)
        ))
    ));
    assert_eq!(discovery.outstanding_len(), 1);
    assert!(discovery.retransmit(request_hash).is_some());
    let _authenticated_retry = discovery
        .authenticate_response(retry, &responder)
        .expect("rejected runtime handoff leaves the response retryable");
    assert!(fixture.executor.runtime.driver().ingress_ready());
    assert!(!fixture.executor.status().fail_closed);
}
#[test]
fn discovered_commit_certificate_mints_exact_reducer_admission_only_after_enqueue() {
    let mut fixture = ProductionTransportFixture::new();
    let message =
        wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::QuorumCertificate(
            fixture.quorum_certificate(wire::GlobalPhase::Commit, fixture.canonical_commitment),
        ));
    let sender = PeerId::new(fixture.validator_keys[0].public_key().clone());
    let ownership = fair_transport_ingress_ownership(message.clone(), sender);
    let admission = fixture
        .executor
        .enqueue_discovered_commit_certificate(message.clone(), ownership)
        .expect("exact authenticated CommitQC enters serialized reducer ingress");
    assert!(admission.matches(&message));
    let prepare =
        wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::QuorumCertificate(
            fixture.quorum_certificate(wire::GlobalPhase::Prepare, fixture.canonical_commitment),
        ));
    let prepare_owner = fair_transport_ingress_ownership(
        prepare.clone(),
        PeerId::new(fixture.validator_keys[0].public_key().clone()),
    );
    assert!(matches!(
        fixture
            .executor
            .enqueue_discovered_commit_certificate(prepare, prepare_owner),
        Err(NetworkIngressError::Authentication(
            AdapterError::DurableCommitMismatch
        ))
    ));
    let transport = wire::ConsensusMessageV2::new(
        wire::ConsensusMessageV2Payload::CommitCertificateRequest(wire::CommitCertificateRequest {
            protocol_version: wire::PROTOCOL_VERSION,
            network_id: fixture.context.network_id.clone(),
            context_id: fixture.context.id(),
            height: fixture.context.height,
            requester: PeerId::new(fixture.requester_key.public_key().clone()),
            signature: Vec::new(),
        }),
    );
    let transport_owner = fair_transport_ingress_ownership(
        transport.clone(),
        PeerId::new(fixture.requester_key.public_key().clone()),
    );
    assert!(matches!(
        fixture
            .executor
            .enqueue_discovered_commit_certificate(transport, transport_owner),
        Err(NetworkIngressError::TransportPayload)
    ));
}
fn round(context: &wire::HeightContext, view: u64) -> wire::ConsensusRound {
    wire::ConsensusRound {
        context_id: context.id(),
        height: context.height,
        view,
    }
}
fn canonical_payload_manifest(
    context: &wire::HeightContext,
    round: wire::ConsensusRound,
    subject: wire::BlockSubject,
    body: &[u8],
) -> wire::PayloadManifest {
    encode_payload(context, round, subject, body)
        .expect("encode the complete canonical fixture payload")
        .manifest()
        .clone()
}
fn deliberately_conflicting_payload_manifest(
    context: &wire::HeightContext,
    round: wire::ConsensusRound,
    original_subject: wire::BlockSubject,
    alternate_body: &[u8],
) -> wire::PayloadManifest {
    let encoded_chunks = wire::encode_payload_chunks(context.da_layout, alternate_body)
        .expect("encode the complete alternate RS16 fixture payload");
    // This negative fixture deliberately binds a canonical alternate RS16
    // codeword to the original subject so the manifest identity conflicts.
    wire::PayloadManifest::derive(
        context,
        round,
        original_subject,
        u64::try_from(alternate_body.len()).expect("alternate body length fits u64"),
        &encoded_chunks,
    )
    .expect("derive the structurally valid conflicting fixture manifest")
}
fn fixture_execution_commitment() -> wire::ExecutionCommitment {
    wire::ExecutionCommitment::without_topups_or_merge_carrier(
        Hash::new(b"effects fixture parent state"),
        Hash::new(b"effects fixture post state"),
        Hash::new(b"effects fixture ordinary writes"),
        1,
        Hash::new(b"effects fixture executed block wire"),
    )
}
fn tag(view: u64) -> EventTag {
    EventTag::new(1, view, Generation::new(7 + view))
}
fn vote(fixture: &Fixture) -> wire::Vote {
    wire::Vote {
        round: fixture.manifest.round,
        proposal_round: fixture.manifest.round,
        phase: wire::GlobalPhase::Prepare,
        subject: fixture.manifest.subject,
        execution_commitment: fixture_execution_commitment(),
        signer: 0,
        signature: Vec::new(),
    }
}
fn vote_equivocation_evidence(
    fixture: &Fixture,
    signer: wire::ValidatorIndex,
) -> AdapterEquivocationEvidence {
    let mut first = vote(fixture);
    first.signer = signer;
    first.signature = vec![0xE1];
    let mut second = first.clone();
    second.subject.block_hash =
        HashOf::from_untyped_unchecked(Hash::new(b"conflicting equivocation block"));
    second.execution_commitment = wire::ExecutionCommitment::without_topups_or_merge_carrier(
        Hash::new(b"conflicting equivocation parent state"),
        Hash::new(b"conflicting equivocation post state"),
        Hash::new(b"conflicting equivocation ordinary writes"),
        1,
        Hash::new(b"conflicting equivocation executed block"),
    );
    second.signature = vec![0xE2];
    AdapterEquivocationEvidence::vote_for_test(first, second)
}
fn proposal(fixture: &Fixture) -> wire::ConsensusMessageV2 {
    wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::Proposal(wire::Proposal {
        round: fixture.manifest.round,
        proposer: fixture.context.leader(fixture.manifest.round.view),
        subject: fixture.manifest.subject,
        manifest: fixture.manifest.clone(),
        justification: wire::ProposalJustification::ParentCommit(wire::ParentCommitJustification {
            certificate: None,
        }),
        signature: vec![0x91],
    }))
}
fn timeout_certificate(fixture: &Fixture) -> wire::TimeoutCertificate {
    wire::TimeoutCertificate {
        round: fixture.manifest.round,
        groups: vec![wire::TimeoutVoteGroup {
            highest_prepare_qc: None,
            signers: vec![0, 1, 2],
            aggregate_signature: vec![1],
        }],
    }
}
fn manifest_at_view(fixture: &Fixture, view: u64) -> wire::PayloadManifest {
    canonical_payload_manifest(
        &fixture.context,
        round(&fixture.context, view),
        fixture.manifest.subject,
        &fixture.body,
    )
}
