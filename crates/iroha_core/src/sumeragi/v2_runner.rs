//! Serialized production height runner for the authoritative Sumeragi v2 reducer.
//!
//! This module owns exactly one reducer/effect executor at a time. It opens the
//! immutable context and safety WAL before processing network traffic, routes
//! authenticated control and body messages, schedules bounded proposal work,
//! and performs an explicit Kura-authorized rollover after application.

use std::{
    num::{NonZeroU64, NonZeroUsize},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use super::v2_core::{
    CanonicalIdentityProjection, EventTag, Generation, IDENTITY_DOMAIN_DURABLE_ARTIFACT,
    IDENTITY_KIND_FINALITY_ARTIFACT, ProductionSuccessorPredecessorBindingProjection,
    ProductionSuccessorStartupLifecycleProjection,
    ProductionTerminalApplicationWithoutSuccessorActivationProjection,
    SUCCESSOR_AUTHORITY_RECOVERED_COMPLETE_TIP, SUCCESSOR_AUTHORITY_SNAPSHOT_BOOTSTRAP,
    SUCCESSOR_LIFECYCLE_RETRY_COMPLETE_TIP, SUCCESSOR_LIFECYCLE_SNAPSHOT_BOOTSTRAP,
    SUCCESSOR_STAGE_NONE, check_production_successor_startup_lifecycle_transition,
    check_production_terminal_application_transition,
    production_successor_predecessor_binding_kernel,
};
#[cfg(test)]
use super::v2_recovery::RecoveredCompleteTipActivationAuthority;
use super::{
    FairV2Ingress, FairV2IngressBarrierBypass, FairV2IngressCapacityError,
    FairV2IngressDequeueDisposition, FairV2IngressOwnershipEvidence, GenesisWithPubKey,
    InboundBlockMessage, SumeragiWorker,
    message::{BlockMessage, CanonicalExecutedBlockNeedV1},
    output_guard::{ConsensusOutputGuard, ConsensusOutputPermit},
    v2::{
        AdapterFingerprints, DeferredAdmissionOrdinalSource, LocalProposalDirective,
        ServicedCandidateCapacityGeometry, SumeragiV2Adapter,
    },
    v2_apply::{
        LaneReservationReconciliationPlanning, V2ReservationLifecycleError,
        apply_lane_reservation_reconciliation_plan,
        persist_preflighted_historical_autonomous_lane_recoveries, plan_lane_reservation_ownership,
        preflight_historical_autonomous_lane_recovery,
        validate_installed_historical_autonomous_lane_recoveries,
    },
    v2_block_sync::{
        CommitCertificateAdmissionError, V2BlockSyncDiscovery, V2BlockSyncError, V2BlockSyncServer,
    },
    v2_body_store::{BlockSignaturePolicy, V2BodyStore},
    v2_candidate::{
        CandidateAssemblyOutcome, CandidateAttachments, CandidateLimits, CandidateParent,
        CandidateRequest, V2CandidateAssembler, candidate_block_has_proposal_work,
    },
    v2_chunks::{EncodedV2Payload, encode_payload},
    v2_effects::{
        EffectExecutorStep, EffectQueueConfig, EffectTransportError, PendingKuraApplyRecoveryStage,
        PostFinalityCleanupTarget, V2EffectExecutor,
        certified_body_request_is_superseded_after_decision,
        network_ingress_is_certified_fence_escape, v2_ingress_head_can_drain,
    },
    v2_first_release_recovery::{
        CompleteTipPredecessorStorageErrorV1, RetiredRecoveredCompleteTipActivationAuthorityV1,
    },
    v2_lane_work::{
        AuthenticatedGenesisNexusAmxContext, CanonicalExecutedBlockRecovery,
        DurableLaneRolloverAuthority, GlobalBodyLockOutcome, HistoricalRecoveryServiceOutcome,
        LaneApplicationEvidenceRepairPlanning, MergeSidecarDeferralDisposition,
        RetainedMergeSidecars, V2LaneIngressOutcome, V2LaneWorkAdapter, V2LaneWorkEffect,
        V2LaneWorkError, V2LaneWorkLimits, apply_lane_application_evidence_repair,
        persist_canonical_historical_recovery_payload_custody,
        plan_lane_application_evidence_repair, require_validator_storage_platform,
    },
    v2_lifecycle_recovery::{
        AutonomousLifecycleDeferredTerminalRecoveryHandoff, reconcile_autonomous_lifecycle_startup,
        reconcile_pending_autonomous_lifecycle_terminal_outcomes,
    },
    v2_npos::V2NposVrfLifecycle,
    v2_recovery::{
        DurableSuccessorActivationAuthority, DurableV2PredecessorIdentity,
        RecoveredSuccessorActivationAuthority, SnapshotSuccessorActivationAuthority,
        build_verified_successor, recover_active_height_with_plan,
        successor_block_refinement_projection, successor_context_refinement_projection,
    },
    v2_runtime::{NetworkIngressError, RuntimeQueueConfig, SerializedV2Runtime},
    v2_transport::AuthenticatedCertifiedBodyRequest,
    v2_worker::{
        CertifiedServeAdmission, CertifiedServeNegativeOutcome, CertifiedServePrepareError,
        ExactFanoutOwnership, KuraReplicaAdvertRefreshOwner, ProductionV2Services,
        V2CleanupSupervisor, durable_exact_output_handoff_owner_pair,
    },
};
#[cfg(test)]
use super::{
    serviced_candidate_store::LeaderWireLifecycleStoreGate, v2_worker::CertifiedServeIngressGate,
};
use crate::{
    kura::{AutonomousLifecycleProcessGenerationClaim, Kura, KuraV2CommitReceipt},
    merge_sidecar::{
        CertifiedMergeSidecarClosedPrefix, CertifiedMergeSidecarMessage, MergeSidecarLimits,
        MergeSigningGuardLimits,
    },
    native_amx::NativeAmxMessage,
    queue::{GlobalQueueSelectionLease, Queue},
    state::{PendingCertifiedMergeSelection, State},
};
#[cfg(test)]
use iroha_config::parameters::actual::SUMERAGI_V2_CONFIG_FORMAT_VERSION;
use iroha_config::parameters::actual::{NodeRole, SumeragiV2Config, sumeragi_v2_timing_ms};
use iroha_crypto::{Hash, HashOf, KeyPair};
use iroha_data_model::{
    Encode as _,
    account::AccountId,
    block::{BlockHeader, SignedBlock, consensus_v2 as wire},
    events::{EventBox, pipeline::PipelineEventBox},
    peer::PeerId,
};
use thiserror::Error;

#[path = "v2_runner/lifecycle_height_driver.rs"]
mod lifecycle_height_driver;
#[path = "v2_runner/lifecycle_pending_kura.rs"]
mod lifecycle_pending_kura;
#[path = "v2_runner/lifecycle_run_inner.rs"]
pub(in crate::sumeragi) mod lifecycle_run_inner;
#[path = "v2_runner/lifecycle_runner_authority.rs"]
mod lifecycle_runner_authority;
#[path = "v2_runner/ordinary_ingress_consumer.rs"]
pub(in crate::sumeragi) mod ordinary_ingress_consumer;
#[path = "v2_runner/preactivation_ingress.rs"]
mod preactivation_ingress;
pub(in crate::sumeragi) use lifecycle_height_driver::drain_lifecycle_v2_ingress;
#[cfg(test)]
use lifecycle_pending_kura::{PendingTipRecoveryDeadline, pending_tip_recovery_deadline_error};
use lifecycle_run_inner::PendingSuccessorActivation;
pub(in crate::sumeragi) use lifecycle_runner_authority::{
    ProductionLifecycleCompleteTipRunnerActivationV1,
    ProductionLifecyclePendingKuraRunnerActivationV1, ProductionLifecycleRunnerActivationV1,
    RecoveredLifecycleOwnerFactoryDependencyPermitV1,
};
use ordinary_ingress_consumer::{
    PreparedDequeuedV2IngressV1, ProductionPreparedOrdinaryIngressConsumptionV1,
    consume_prepared_dequeued_v2_ingress,
};
pub(in crate::sumeragi) use preactivation_ingress::ProductionLifecycleCanonicalRecoveryIngressV1;

const IDLE_POLL: Duration = Duration::from_millis(10);
const CANDIDATE_WORK_RECHECK: Duration = Duration::from_millis(100);

/// Move-only post-activation ownership of runner readiness and exact ingress.
///
/// The activated lifecycle stack retains this authority until finalization.
/// Dropping it first clears readiness and closes ingress, so the later durable
/// gate teardown cannot leave a carrierless queue advertised as live.
#[must_use = "activated runner authority must remain with the lifecycle height"]
pub(in crate::sumeragi) struct ProductionLifecycleActivatedRunnerAuthorityV1 {
    _seal: ProductionLifecycleActivatedRunnerAuthoritySealV1,
    ingress_ready: Arc<AtomicBool>,
    block_ingress: Arc<FairV2Ingress>,
}

struct ProductionLifecycleActivatedRunnerAuthoritySealV1;

impl Drop for ProductionLifecycleActivatedRunnerAuthoritySealV1 {
    fn drop(&mut self) {}
}

impl ProductionLifecycleActivatedRunnerAuthorityV1 {
    /// Consume the exact readiness owner before lifecycle gate retirement.
    pub(in crate::sumeragi) fn retire(
        self,
        launched_ingress: &Arc<FairV2Ingress>,
    ) -> Result<(), V2RunnerError> {
        retire_lifecycle_runner_ingress(&self.ingress_ready, &self.block_ingress, launched_ingress)
    }
}

fn retire_lifecycle_runner_ingress(
    ingress_ready: &Arc<AtomicBool>,
    block_ingress: &Arc<FairV2Ingress>,
    launched_ingress: &Arc<FairV2Ingress>,
) -> Result<(), V2RunnerError> {
    ingress_ready.store(false, Ordering::Release);
    block_ingress.close();
    if !Arc::ptr_eq(block_ingress, launched_ingress) {
        return Err(V2RunnerError::LifecycleActivationIngressMismatch);
    }
    Ok(())
}

impl Drop for ProductionLifecycleActivatedRunnerAuthorityV1 {
    fn drop(&mut self) {
        self.ingress_ready.store(false, Ordering::Release);
        self.block_ingress.close();
    }
}

/// Process-local borrow key for driving an activated lifecycle stack.
///
/// Only the serialized runner can mint this key. Repeated mutable borrows keep
/// owner, executor, and services inside the activated type state and cannot
/// move any of them into a shadow scheduler.
#[must_use = "the active runner borrow key must remain with the height loop"]
pub(in crate::sumeragi) struct ProductionLifecycleActiveRunnerBorrowV1 {
    _seal: ProductionLifecycleActiveRunnerBorrowSealV1,
}

struct ProductionLifecycleActiveRunnerBorrowSealV1;

impl Drop for ProductionLifecycleActiveRunnerBorrowSealV1 {
    fn drop(&mut self) {}
}

impl ProductionLifecycleActiveRunnerBorrowV1 {
    /// Mint beside the activated owner in the lifecycle-owned height loop.
    fn mint_for_recovered_runner() -> Self {
        Self {
            _seal: ProductionLifecycleActiveRunnerBorrowSealV1,
        }
    }

    /// Mint the same opaque runner borrow for a production-shaped lifecycle test.
    #[cfg(test)]
    pub(in crate::sumeragi) fn for_test() -> Self {
        Self::mint_for_recovered_runner()
    }
}

/// Process-local borrow key for preparing a launched lifecycle before activation.
///
/// Only the serialized runner can mint this key. It permits bounded lane and
/// recovery setup through the opaque launched stack while its exact ingress is
/// still closed; it cannot activate the height or extract any owned component.
/// The key retains the modular runner's live local-Proposal state so
/// recovered Proposal ownership cannot be acknowledged without updating the
/// state used after activation. Its consuming transition is the sole mint
/// for the prepared state required by lifecycle activation.
#[must_use = "the preactivation runner borrow key must remain with setup"]
pub(in crate::sumeragi) struct ProductionLifecyclePreActivationRunnerBorrowV1 {
    _seal: ProductionLifecyclePreActivationRunnerBorrowSealV1,
    local_proposal: Option<ProductionLifecycleLocalProposalStateV1>,
}

struct ProductionLifecyclePreActivationRunnerBorrowSealV1;

impl Drop for ProductionLifecyclePreActivationRunnerBorrowSealV1 {
    fn drop(&mut self) {}
}

impl ProductionLifecyclePreActivationRunnerBorrowV1 {
    /// Mint beside the launched owner at the non-Pending lifecycle boundary.
    fn mint_for_recovered_runner() -> Self {
        Self {
            _seal: ProductionLifecyclePreActivationRunnerBorrowSealV1,
            local_proposal: Some(ProductionLifecycleLocalProposalStateV1::fresh()),
        }
    }

    /// Mint the same opaque setup borrow for a production-shaped lifecycle test.
    #[cfg(test)]
    pub(in crate::sumeragi) fn for_test() -> Self {
        Self::mint_for_recovered_runner()
    }

    /// Bind one exact recovered Proposal owner to the real runner-local state.
    pub(in crate::sumeragi) fn bind_recovered_local_proposal(
        &mut self,
        directive: LocalProposalDirective,
    ) -> bool {
        let Some(local_proposal) = self.local_proposal.as_mut() else {
            return false;
        };
        if !local_proposal.state.is_pristine() {
            return false;
        }
        local_proposal.state =
            LocalProposalState::from_recovered_lifecycle_attempt(true, directive);
        true
    }

    /// Whether setup still owns an untouched local-Proposal scheduler.
    pub(in crate::sumeragi) fn local_proposal_state_is_pristine(&self) -> bool {
        self.local_proposal
            .as_ref()
            .is_some_and(|local_proposal| local_proposal.state.is_pristine())
    }

    /// Revalidate the retained scheduler state against the prepared directive.
    pub(in crate::sumeragi) fn prepared_local_proposal_exactly_matches(
        &self,
        directive: LocalProposalDirective,
    ) -> bool {
        self.local_proposal.as_ref().is_some_and(|local_proposal| {
            local_proposal.state.is_pristine() || local_proposal.already_attempted(directive)
        })
    }

    /// Borrow the prepared opaque scheduler owner for the active runner loop.
    pub(super) fn prepared_local_proposal_mut(
        &mut self,
    ) -> Option<&mut ProductionLifecycleLocalProposalStateV1> {
        self.local_proposal.as_mut()
    }

    /// Check the retained state in focused runner-boundary tests.
    #[cfg(test)]
    pub(in crate::sumeragi) fn already_attempted(&self, directive: LocalProposalDirective) -> bool {
        self.local_proposal
            .as_ref()
            .is_some_and(|local_proposal| local_proposal.already_attempted(directive))
    }
}

/// Exact reducer facts which own one local proposal-side work item.
///
/// A higher PrepareQC can replace the lock without changing [`EventTag`].
/// Tagging local work by the runtime incarnation alone would therefore let a
/// delayed rejection or preparation completion for the old subject mutate the
/// new lock's scheduling state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct LocalProposalOwner {
    tag: EventTag,
    locked_body: Option<(wire::ConsensusRound, wire::BlockSubject)>,
    decided_subject: Option<wire::BlockSubject>,
}
impl From<LocalProposalDirective> for LocalProposalOwner {
    fn from(directive: LocalProposalDirective) -> Self {
        Self {
            tag: directive.tag(),
            locked_body: directive.locked_body(),
            decided_subject: directive.decided_subject(),
        }
    }
}
impl LocalProposalOwner {
    /// Return whether this owner installs the first exact lock for prior
    /// unlocked proposal work from the same reducer incarnation.
    fn installs_first_exact_lock_for(self, prior: Self, subject: wire::BlockSubject) -> bool {
        prior.tag == self.tag
            && prior.decided_subject == self.decided_subject
            && prior.locked_body.is_none()
            && self.locked_body.is_some_and(|(round, locked_subject)| {
                round.height == self.tag.height()
                    && round.view == self.tag.view()
                    && locked_subject == subject
            })
    }
}
#[derive(Debug)]
struct PendingLocalEvents {
    owner: LocalProposalOwner,
    subject: wire::BlockSubject,
    events: Vec<PipelineEventBox>,
}
/// Queue ownership retained until the exact local proposal is decided or abandoned.
///
/// The ordinary transaction remains in the queue while this process-local fence prevents the
/// autonomous lane producer from reserving the same hash. The fence follows only the first exact
/// lock for this proposal subject and is released on every replacement, rejection, or decision.
#[derive(Debug)]
struct PendingGlobalSelection {
    owner: LocalProposalOwner,
    subject: wire::BlockSubject,
    _lease: GlobalQueueSelectionLease,
}
#[derive(Clone, Copy, Debug)]
struct CandidateWorkWait {
    owner: LocalProposalOwner,
    started_at: Instant,
    next_retry: Instant,
}

/// Fallible construction ownership of an applied predecessor's successor.
///
/// Starting construction changes the predecessor's durable diagnostic witness
/// from `Queued` to `Running`. Only a successfully verified successor context
/// can bind this token into [`PendingSuccessorActivation`].
#[derive(Debug)]
struct PendingSuccessorConstruction {
    predecessor: DurableV2PredecessorIdentity,
}
impl PendingSuccessorConstruction {
    fn begin(predecessor: DurableV2PredecessorIdentity) -> Result<Self, V2RunnerError> {
        super::status::begin_v2_successor_activation(predecessor)?;
        Ok(Self { predecessor })
    }
    fn bind(
        self,
        authority: DurableSuccessorActivationAuthority,
    ) -> Result<PendingSuccessorActivation, V2RunnerError> {
        let binding = ProductionSuccessorPredecessorBindingProjection {
            expected_predecessor: self.predecessor.refinement_projection(),
            authority_predecessor: authority.predecessor().refinement_projection(),
            successor_context_id: super::v2_recovery::successor_context_refinement_projection(
                authority.successor_context_id(),
            ),
        };
        if !production_successor_predecessor_binding_kernel(binding) {
            return Err(V2RunnerError::SuccessorPredecessorAuthorityMismatch {
                expected: self.predecessor,
                actual: authority.predecessor(),
            });
        }
        Ok(PendingSuccessorActivation::Applied {
            expected_predecessor: self.predecessor,
            authority,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LocalValidationDisposition {
    Ignored,
    RetryNonEmpty,
    FatalNonEmpty,
}
#[derive(Default, Debug)]
struct LocalProposalState {
    attempted: Option<LocalProposalOwner>,
    submitted: Option<(LocalProposalOwner, wire::BlockSubject)>,
    non_empty_retry: Option<LocalProposalOwner>,
    candidate_work_wait: Option<CandidateWorkWait>,
    pending_events: Option<PendingLocalEvents>,
    global_selection: Option<PendingGlobalSelection>,
}
impl LocalProposalState {
    fn is_pristine(&self) -> bool {
        self.attempted.is_none()
            && self.submitted.is_none()
            && self.non_empty_retry.is_none()
            && self.candidate_work_wait.is_none()
            && self.pending_events.is_none()
            && self.global_selection.is_none()
    }

    /// Initialize from the lifecycle owner's already-authenticated replay join.
    #[cfg_attr(not(test), allow(dead_code))]
    fn from_recovered_lifecycle_attempt(
        already_attempted: bool,
        current: LocalProposalDirective,
    ) -> Self {
        Self {
            attempted: already_attempted.then_some(LocalProposalOwner::from(current)),
            ..Self::default()
        }
    }

    /// Retire every volatile item which is not owned by the exact current
    /// lock/decision snapshot. A Decision owns no further proposal work.
    fn reconcile(&mut self, owner: LocalProposalOwner) -> LocalProposalOwner {
        if owner.decided_subject.is_some() {
            *self = Self::default();
            return owner;
        }
        if let Some((candidate, subject)) = self.submitted
            && candidate != owner
        {
            if owner.installs_first_exact_lock_for(candidate, subject) {
                self.submitted = Some((owner, subject));
            } else {
                self.submitted = None;
            }
        }
        if self
            .pending_events
            .as_ref()
            .is_some_and(|pending| pending.owner != owner)
        {
            let preserve = self.pending_events.as_ref().is_some_and(|pending| {
                owner.installs_first_exact_lock_for(pending.owner, pending.subject)
            });
            if preserve {
                self.pending_events
                    .as_mut()
                    .expect("pending events were observed above")
                    .owner = owner;
            } else {
                self.pending_events = None;
            }
        }
        if self
            .global_selection
            .as_ref()
            .is_some_and(|selection| selection.owner != owner)
        {
            let preserve = self.global_selection.as_ref().is_some_and(|selection| {
                owner.installs_first_exact_lock_for(selection.owner, selection.subject)
            });
            if preserve {
                self.global_selection
                    .as_mut()
                    .expect("global selection was observed above")
                    .owner = owner;
            } else {
                self.global_selection = None;
            }
        }
        let continued_exact_work = self
            .submitted
            .is_some_and(|(candidate, _)| candidate == owner)
            || self
                .pending_events
                .as_ref()
                .is_some_and(|pending| pending.owner == owner)
            || self
                .global_selection
                .as_ref()
                .is_some_and(|selection| selection.owner == owner);
        if self.attempted.is_some_and(|candidate| candidate != owner) {
            self.attempted = continued_exact_work.then_some(owner);
        }
        if self
            .non_empty_retry
            .is_some_and(|candidate| candidate != owner)
        {
            self.non_empty_retry = None;
        }
        if self
            .candidate_work_wait
            .is_some_and(|wait| wait.owner != owner)
        {
            self.candidate_work_wait = None;
        }
        owner
    }
    /// Keep retrying deferred autonomous work for the bounded observation
    /// window, then arm one ordinary non-empty recovery retry for this owner.
    ///
    /// Expiry deliberately changes only proposal shape. It never changes the
    /// lane adapter's routing or queue-ownership policy.
    fn defer_candidate_work(
        &mut self,
        owner: LocalProposalOwner,
        now: Instant,
        wait_bound: Duration,
    ) {
        if self.non_empty_retry == Some(owner) {
            self.candidate_work_wait = None;
            return;
        }
        let started_at = self
            .candidate_work_wait
            .filter(|wait| wait.owner == owner)
            .map_or(now, |wait| wait.started_at);
        if now.saturating_duration_since(started_at) < wait_bound {
            self.candidate_work_wait = Some(CandidateWorkWait {
                owner,
                started_at,
                next_retry: deadline_after(now, CANDIDATE_WORK_RECHECK),
            });
            return;
        }
        self.non_empty_retry = Some(owner);
        self.candidate_work_wait = None;
    }
    /// Retire an armed recovery retry which completed assembly without finding
    /// any publishable work. A later retry must cross a fresh bounded
    /// observation window instead of re-running full assembly every runner
    /// poll.
    fn retire_unsubmitted_non_empty_retry(&mut self, owner: LocalProposalOwner) -> bool {
        let owner = self.reconcile(owner);
        if self.non_empty_retry != Some(owner) {
            return false;
        }
        self.non_empty_retry = None;
        self.candidate_work_wait = None;
        true
    }
    fn handle_validation_rejection(
        &mut self,
        owner: LocalProposalOwner,
        expected_round: wire::ConsensusRound,
        rejected_round: wire::ConsensusRound,
        rejected_subject: wire::BlockSubject,
    ) -> LocalValidationDisposition {
        let owner = self.reconcile(owner);
        if expected_round != rejected_round || self.submitted != Some((owner, rejected_subject)) {
            return LocalValidationDisposition::Ignored;
        }
        if self
            .pending_events
            .as_ref()
            .is_some_and(|pending| pending.owner == owner && pending.subject == rejected_subject)
        {
            self.pending_events = None;
        }
        if self.global_selection.as_ref().is_some_and(|selection| {
            selection.owner == owner && selection.subject == rejected_subject
        }) {
            self.global_selection = None;
        }
        if self.non_empty_retry == Some(owner) {
            return LocalValidationDisposition::FatalNonEmpty;
        }
        self.attempted = None;
        self.non_empty_retry = Some(owner);
        self.submitted = None;
        self.candidate_work_wait = None;
        LocalValidationDisposition::RetryNonEmpty
    }
    /// Abandon a candidate whose lane-local ownership could not be bound
    /// before the body was submitted, then give the exact owner one ordinary
    /// non-empty retry. The caller releases the candidate's selection lease
    /// before crossing this boundary, without publishing a body or events.
    fn handle_candidate_binding_rejection(
        &mut self,
        owner: LocalProposalOwner,
    ) -> LocalValidationDisposition {
        let owner = self.reconcile(owner);
        if self.non_empty_retry == Some(owner) {
            return LocalValidationDisposition::FatalNonEmpty;
        }
        self.attempted = None;
        self.non_empty_retry = Some(owner);
        self.candidate_work_wait = None;
        LocalValidationDisposition::RetryNonEmpty
    }
    fn take_prepared_events(
        &mut self,
        owner: LocalProposalOwner,
        prepared_tag: EventTag,
        prepared_subject: wire::BlockSubject,
    ) -> Option<Vec<PipelineEventBox>> {
        let owner = self.reconcile(owner);
        let matches = self.pending_events.as_ref().is_some_and(|pending| {
            pending.owner == owner
                && pending.owner.tag == prepared_tag
                && pending.subject == prepared_subject
        });
        matches.then(|| {
            self.pending_events
                .take()
                .expect("matching pending events were observed above")
                .events
        })
    }
}
/// Opaque runner ownership of local-Proposal scheduling state.
///
/// The non-PendingKura lifecycle loop constructs one owner for the complete
/// height, lends it to preactivation recovery, then uses the same private state
/// in the live scheduling loop. No lifecycle caller can manufacture a shadow
/// state or extract its recovered owner.
#[must_use = "runner local-Proposal state must remain with the height loop"]
pub(in crate::sumeragi) struct ProductionLifecycleLocalProposalStateV1 {
    state: LocalProposalState,
}

impl ProductionLifecycleLocalProposalStateV1 {
    fn fresh() -> Self {
        Self {
            state: LocalProposalState::default(),
        }
    }

    /// Check whether the retained state owns the exact recovered attempt.
    pub(in crate::sumeragi) fn already_attempted(&self, directive: LocalProposalDirective) -> bool {
        self.state.attempted == Some(LocalProposalOwner::from(directive))
    }
}

/// Run the v2-only worker until shutdown or a fail-closed error.
pub(super) fn run(worker: SumeragiWorker) {
    let mut status_clear = V2StatusClearGuard::new();
    let ingress_ready = Arc::clone(&worker.ingress_ready);
    let block_ingress = Arc::clone(&worker.block_rx);
    let output_guard = Arc::clone(&worker.output_guard);
    let _ingress_clear = V2IngressClearGuard::new(Arc::clone(&ingress_ready), block_ingress);
    // Declared after ingress cleanup so reverse-order unwinding closes the
    // process output gate before readiness state is released.
    let mut failure_guard = V2RunnerFailureGuard::new(Arc::clone(&output_guard));
    match run_inner(worker) {
        Ok(()) => {
            failure_guard.disarm();
            status_clear.clear_on_drop();
        }
        Err(error) => {
            output_guard.activate_restart_required();
            super::status::mark_v2_restart_required();
            iroha_logger::error!(%error, "authoritative Sumeragi v2 runner stopped fail-closed");
        }
    }
    ingress_ready.store(false, Ordering::Release);
}
/// Latch process-lifetime restart recovery when the runner exits abnormally.
///
/// In particular, this guard covers panics before production services exist;
/// those services therefore cannot be relied upon to poison the shared guard
/// during their own abnormal drop.
struct V2RunnerFailureGuard {
    output_guard: Arc<ConsensusOutputGuard>,
    armed: bool,
}
impl V2RunnerFailureGuard {
    fn new(output_guard: Arc<ConsensusOutputGuard>) -> Self {
        Self {
            output_guard,
            armed: true,
        }
    }
    fn disarm(&mut self) {
        self.armed = false;
    }
}
impl Drop for V2RunnerFailureGuard {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        self.output_guard.close_admission_for_restart();
        if !std::thread::panicking() {
            self.output_guard.activate_restart_required();
        }
    }
}
struct V2IngressClearGuard {
    ingress_ready: Arc<AtomicBool>,
    block_ingress: Arc<FairV2Ingress>,
}
impl V2IngressClearGuard {
    fn new(ingress_ready: Arc<AtomicBool>, block_ingress: Arc<FairV2Ingress>) -> Self {
        ingress_ready.store(false, Ordering::Release);
        block_ingress.close();
        Self {
            ingress_ready,
            block_ingress,
        }
    }
}
impl Drop for V2IngressClearGuard {
    fn drop(&mut self) {
        self.ingress_ready.store(false, Ordering::Release);
        self.block_ingress.close();
    }
}
#[cfg(test)]
struct CertifiedServeIngressBinding {
    ingress_ready: Arc<AtomicBool>,
    block_ingress: Arc<FairV2Ingress>,
    gate: Option<CertifiedServeIngressGate>,
}
#[cfg(test)]
impl CertifiedServeIngressBinding {
    fn bind(
        ingress_ready: Arc<AtomicBool>,
        block_ingress: Arc<FairV2Ingress>,
        gate: CertifiedServeIngressGate,
    ) -> Result<Self, V2RunnerError> {
        block_ingress
            .bind_certified_serve_gate(gate.clone())
            .map_err(V2RunnerError::Service)?;
        Ok(Self {
            ingress_ready,
            block_ingress,
            gate: Some(gate),
        })
    }
    fn retire(&mut self) -> Result<(), V2RunnerError> {
        let Some(gate) = self.gate.as_ref() else {
            return Ok(());
        };
        close_ingress_for_rollover(&self.ingress_ready, &self.block_ingress);
        self.block_ingress
            .unbind_certified_serve_gate(gate)
            .map_err(V2RunnerError::Service)?;
        self.gate = None;
        Ok(())
    }
}
#[cfg(test)]
impl Drop for CertifiedServeIngressBinding {
    fn drop(&mut self) {
        if let Err(error) = self.retire() {
            iroha_logger::error!(
                %error,
                "failed to retire the per-height certified Serve ingress gate"
            );
        }
    }
}
/// Per-height binding of durable generic leader-wire ownership to fair ingress.
#[cfg(test)]
struct LeaderWireIngressBinding {
    ingress_ready: Arc<AtomicBool>,
    block_ingress: Arc<FairV2Ingress>,
    gate: Option<Arc<LeaderWireLifecycleStoreGate>>,
}
#[cfg(test)]
impl LeaderWireIngressBinding {
    fn bind(
        ingress_ready: Arc<AtomicBool>,
        block_ingress: Arc<FairV2Ingress>,
        gate: Arc<LeaderWireLifecycleStoreGate>,
        restore: super::serviced_candidate_store::LeaderWireLifecycleRestore,
        lifecycle_ordinals: super::v2_runtime::RuntimeLifecycleOrdinalSource,
        context_id: wire::HeightContextId,
        height: wire::Height,
    ) -> Result<Self, V2RunnerError> {
        block_ingress
            .bind_leader_wire_lifecycle_gate(
                Arc::clone(&gate),
                restore,
                lifecycle_ordinals,
                context_id,
                height,
            )
            .map_err(V2RunnerError::Service)?;
        Ok(Self {
            ingress_ready,
            block_ingress,
            gate: Some(gate),
        })
    }
    fn retire(&mut self) -> Result<(), V2RunnerError> {
        let Some(gate) = self.gate.as_ref() else {
            return Ok(());
        };
        close_ingress_for_rollover(&self.ingress_ready, &self.block_ingress);
        self.block_ingress
            .unbind_leader_wire_lifecycle_gate(gate)
            .map_err(V2RunnerError::Service)?;
        self.gate = None;
        Ok(())
    }
}
#[cfg(test)]
impl Drop for LeaderWireIngressBinding {
    fn drop(&mut self) {
        if let Err(error) = self.retire() {
            iroha_logger::error!(
                %error,
                "failed to retire the per-height durable leader-wire lifecycle gate"
            );
        }
    }
}
include!("v2_runner/height_ingress_bindings.rs");
include!("v2_runner/lifecycle_terminal_recovery.rs");
#[allow(clippy::too_many_lines)]
fn run_inner(worker: SumeragiWorker) -> Result<(), V2RunnerError> {
    let SumeragiWorker {
        config,
        common_config,
        events_sender,
        state,
        queue,
        kura,
        provider_ingest_finalized_archive,
        reputation_finalized_archive,
        startup_replay_plan,
        mut startup_replay_inventory_guard,
        network,
        genesis_network,
        block_rx,
        lane_relay_rx,
        wake_rx,
        shutdown_signal,
        ingress_ready,
        output_guard,
        consensus_frame_byte_capacity,
        block_sync_frame_byte_capacity,
    } = worker;
    // Reject an unsupported voting host before any recovery or durable
    // consensus constructor can touch validator storage. Observers remain
    // available for sync and query service on other platforms.
    require_validator_storage_platform(
        config.role == NodeRole::Validator,
        crate::kura::sumeragi_v2_validator_storage_supported(),
    )?;
    let GenesisWithPubKey {
        genesis,
        public_key: genesis_public_key,
        block_cadence,
        v2_bootstrap,
    } = genesis_network;
    let genesis_body = genesis.map(|block| block.0);
    let recovery = output_guard
        .begin_fail_stop_operation()
        .ok_or(V2RunnerError::RestartRequired)?;
    let recovered = recover_active_height_with_plan(
        kura.as_ref(),
        state.as_ref(),
        v2_bootstrap,
        genesis_public_key.clone(),
        startup_replay_plan,
    )?;
    startup_replay_inventory_guard.finish();
    recovery.complete();
    let pending_kura_apply = recovered.pending_kura_apply();
    let (
        verified_context,
        context_store,
        signature_policy,
        lifecycle_storage_authority,
        first_height_authenticated_genesis,
        recovered_successor_activation,
        staged_genesis_nexus_amx_context,
    ) = recovered.into_parts();
    let local_peer = common_config.peer.id().clone();
    // Height-local roster membership is read-only and must precede the first
    // lifecycle mutation. It controls global duty for this frozen height, not
    // the immutable process capability: a configured validator may be absent
    // now and later rotate into a global roster, or still serve an independently
    // frozen lane descriptor which names its key.
    let _initial_local_validator =
        local_validator_index(verified_context.context(), &local_peer, config.role)?;
    // Claim the process generation before constructing any height-local lane
    // service.  The claim is rooted in the authenticated recovered chain
    // context and the immutable local Kura identity; later startup lifecycle
    // reconciliation consumes this same process-lifetime claim rather than
    // allowing each height adapter to invent a generation.
    let _lifecycle_process_generation = claim_runner_lifecycle_process_generation(
        config.role,
        kura.as_ref(),
        verified_context.context(),
        &local_peer,
    )?;
    // A process which recovered durable v2 height ownership may be behind its
    // peers. Probe that exact active context immediately, then retain eager
    // discovery only while an authenticated discovered CommitQC acquires or
    // coalesces with serialized reducer ownership. Ordinary live finality
    // clears the hint, so this does not add permanent all-to-all traffic.
    let eager_block_sync = recovered_successor_activation.is_some() || pending_kura_apply.is_some();
    // Reconcile exactly once, after an interrupted canonical tip (if any) has
    // completed State application. Running before that boundary could mistake
    // the tip's not-yet-published membership for a losing lane proposal.
    let reservation_reconciliation_pending = true;
    let genesis_account = AccountId::new(genesis_public_key);
    let first_height_genesis = genesis_body;
    let block_sync_server = None;
    // The first-release cadence comes from authenticated signed-genesis or
    // snapshot startup metadata and remains immutable for this process.
    // In particular, fresh startup cannot read the uncommitted base State here:
    // its placeholder cadence predates execution of signed genesis.
    let block_cadence_ms = u64::try_from(block_cadence.as_millis())?;
    let (round_timeout_ms, retransmit_interval_ms) = sumeragi_v2_timing_ms(block_cadence_ms)?;
    let round_timeout = Duration::from_millis(round_timeout_ms);
    let retransmit_interval = Duration::from_millis(retransmit_interval_ms);
    validate_deadline_duration(round_timeout)?;
    validate_deadline_duration(retransmit_interval)?;
    let cleanup_supervisor = V2CleanupSupervisor::default();
    let pending_successor_activation = {
        let recovered_activation_guard = recovered_successor_activation
            .as_ref()
            .map(|_| {
                output_guard
                    .begin_fail_stop_operation()
                    .ok_or(V2RunnerError::RestartRequired)
            })
            .transpose()?;
        let pending_successor_activation = recovered_successor_activation
            .map(|authority| {
                PendingSuccessorActivation::recovered(authority, &common_config.key_pair)
            })
            .transpose()?;
        if let Some(activation) = pending_successor_activation.as_ref() {
            // CompleteTip has already retired H and authenticated H+1 here. Reopen
            // that exact retained successor frame before any H+1 adapter, worker,
            // clock, or output construction; final status binding is repeated at
            // the ingress-publication boundary below.
            activation.preflight_recovered_startup()?;
        }
        if let Some(guard) = recovered_activation_guard {
            guard.complete();
        }
        pending_successor_activation
    };
    let liveness_watchdog = super::status::V2LivenessWatchdog::default();
    let deferred_admission_ordinals = DeferredAdmissionOrdinalSource::new(0);
    let retained_merge_sidecars: Option<RetainedMergeSidecars> = None;
    let kura_replica_advert_refresh = Arc::new(
        KuraReplicaAdvertRefreshOwner::from_kura(kura.as_ref(), Instant::now())
            .map_err(V2RunnerError::Service)?,
    );

    match pending_kura_apply {
        None => lifecycle_run_inner::run_non_pending_lifecycle_loop(
            config,
            common_config,
            events_sender,
            state,
            queue,
            kura,
            provider_ingest_finalized_archive,
            reputation_finalized_archive,
            network,
            block_rx,
            lane_relay_rx,
            wake_rx,
            shutdown_signal,
            ingress_ready,
            output_guard,
            consensus_frame_byte_capacity,
            block_sync_frame_byte_capacity,
            verified_context,
            context_store,
            signature_policy,
            lifecycle_storage_authority,
            first_height_authenticated_genesis,
            pending_successor_activation,
            staged_genesis_nexus_amx_context,
            first_height_genesis,
            genesis_account,
            block_cadence,
            round_timeout,
            retransmit_interval,
            _lifecycle_process_generation,
            reservation_reconciliation_pending,
            eager_block_sync,
            cleanup_supervisor,
            liveness_watchdog,
            deferred_admission_ordinals,
            retained_merge_sidecars,
            kura_replica_advert_refresh,
            block_sync_server,
        ),
        Some(pending) => lifecycle_pending_kura::run_pending_kura_lifecycle_height(
            config,
            common_config,
            events_sender,
            state,
            queue,
            kura,
            provider_ingest_finalized_archive,
            reputation_finalized_archive,
            network,
            block_rx,
            lane_relay_rx,
            wake_rx,
            shutdown_signal,
            ingress_ready,
            output_guard,
            consensus_frame_byte_capacity,
            block_sync_frame_byte_capacity,
            verified_context,
            context_store,
            signature_policy,
            lifecycle_storage_authority,
            first_height_authenticated_genesis,
            pending,
            pending_successor_activation,
            staged_genesis_nexus_amx_context,
            first_height_genesis,
            genesis_account,
            block_cadence,
            round_timeout,
            retransmit_interval,
            _lifecycle_process_generation,
            reservation_reconciliation_pending,
            eager_block_sync,
            cleanup_supervisor,
            liveness_watchdog,
            deferred_admission_ordinals,
            retained_merge_sidecars,
            kura_replica_advert_refresh,
            block_sync_server,
        ),
    }
}
#[derive(Default)]
pub(super) struct CommittedLaneStatusPublisher {
    published_revision: Option<(u64, u64, u64)>,
}
impl CommittedLaneStatusPublisher {
    pub(super) fn publish_if_changed(&mut self, lane_work: &V2LaneWorkAdapter) -> bool {
        self.publish_if_changed_with(
            || lane_work.committed_lane_block_status_revision(),
            || lane_work.committed_lane_block_status_snapshot(),
        )
    }
    fn publish_if_changed_with(
        &mut self,
        mut observe_revision: impl FnMut() -> (u64, u64, u64),
        project: impl FnOnce() -> Vec<super::status::CommittedLaneBlockSnapshot>,
    ) -> bool {
        let revision = observe_revision();
        if self.published_revision == Some(revision) {
            return false;
        }
        let snapshot = project();
        if observe_revision() != revision {
            return false;
        }
        super::status::set_committed_lane_blocks(snapshot);
        self.published_revision = Some(revision);
        true
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct LockedBodyRecoveryPlan {
    request: Option<(EventTag, wire::ConsensusRound, wire::BlockSubject)>,
    may_repropose: bool,
}
fn locked_body_recovery_plan(
    directive: LocalProposalDirective,
    local_validator: wire::ValidatorIndex,
    attempted: Option<LocalProposalOwner>,
    can_admit_local_proposal: bool,
) -> LockedBodyRecoveryPlan {
    let owner = LocalProposalOwner::from(directive);
    let request = directive
        .decided_subject()
        .is_none()
        .then(|| directive.locked_body())
        .flatten()
        .map(|(round, subject)| (directive.tag(), round, subject));
    LockedBodyRecoveryPlan {
        request,
        may_repropose: request.is_some()
            && directive.leader() == local_validator
            && attempted != Some(owner)
            && can_admit_local_proposal,
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct LocalConsensusDuties {
    autonomous_lane_view: Option<wire::View>,
    global_validator: Option<wire::ValidatorIndex>,
}
fn local_consensus_duties(
    directive: LocalProposalDirective,
    global_validator: Option<wire::ValidatorIndex>,
) -> LocalConsensusDuties {
    LocalConsensusDuties {
        autonomous_lane_view: (directive.decided_subject().is_none()
            && directive.locked_body().is_none())
        .then_some(directive.tag().view()),
        global_validator,
    }
}
#[allow(clippy::too_many_arguments)]
fn schedule_local_proposal(
    candidate_limits: CandidateLimits,
    context: &wire::HeightContext,
    local_validator: Option<wire::ValidatorIndex>,
    key_pair: &KeyPair,
    output_guard: &ConsensusOutputGuard,
    state: &State,
    queue: &Arc<Queue>,
    kura: &Kura,
    genesis_body: Option<&SignedBlock>,
    height_started_at: Instant,
    block_cadence: Duration,
    proposal_state: &mut LocalProposalState,
    executor: &mut V2EffectExecutor,
    services: &mut ProductionV2Services,
    lane_work: &mut V2LaneWorkAdapter,
    npos_vrf: &V2NposVrfLifecycle,
    candidate_work_wait_bound: Duration,
) -> Result<(), V2RunnerError> {
    let directive = executor.local_proposal_directive()?;
    let duties = local_consensus_duties(directive, local_validator);
    // Lane authority is frozen independently from the successor global
    // roster. A configured validator removed from that roster must still
    // produce a lane payload when the exact current lane descriptor selects
    // it as author. The adapter rechecks voting role, route, and slot author.
    if let Some(active_view) = duties.autonomous_lane_view {
        lane_work.schedule_autonomous_lane_production(active_view, candidate_limits)?;
    }
    let Some(local_validator) = duties.global_validator else {
        return Ok(());
    };
    let owner = proposal_state.reconcile(LocalProposalOwner::from(directive));
    let recovery_plan = locked_body_recovery_plan(
        directive,
        local_validator,
        proposal_state.attempted,
        executor.can_schedule_local_proposal()?,
    );
    // Bind immutable locked-body acquisition to the current reducer
    // incarnation before observing proposal eligibility. Every validator must
    // authenticate and bind the exact carrier even when it is not the current
    // global leader or has already attempted that proposal; only the later
    // unchanged reproposal remains leader-gated.
    if let Some((tag, locked_round, locked)) = recovery_plan.request {
        services
            .request_locked_candidate(tag, locked_round, locked)
            .map_err(V2RunnerError::Service)?;
    }
    while let Some(loaded) = services.take_loaded_candidate() {
        let current = executor.local_proposal_directive()?;
        let loaded_round = loaded.round();
        let loaded_subject = loaded.subject();
        if loaded.tag() != current.tag()
            || current.locked_body() != Some((loaded_round, loaded_subject))
        {
            iroha_logger::debug!(
                loaded_height = loaded.tag().height(),
                loaded_view = loaded.tag().view(),
                current_height = current.tag().height(),
                current_view = current.tag().view(),
                loaded_subject = ?loaded.subject(),
                current_locked_subject = ?current.locked_subject(),
                "discarded stale locked-body load before Sumeragi v2 reproposal"
            );
            continue;
        }
        let canonical_wire = loaded.into_canonical_wire();
        let block = iroha_data_model::block::decode_framed_signed_block(&canonical_wire)
            .map_err(|error| V2RunnerError::Candidate(error.to_string()))?;
        if !block.is_resultless_proposal() {
            return Err(V2RunnerError::ResultBearingProposal);
        }
        let time_trigger_clock_progress_required = block
            .header()
            .creation_time()
            .checked_sub(block_cadence)
            .is_some_and(|parent_creation_time| {
                state.time_trigger_clock_progress_required_fast(parent_creation_time)
            });
        if !candidate_block_has_proposal_work(&block, time_trigger_clock_progress_required) {
            return Err(V2RunnerError::EmptyProposalWork);
        }
        let lane_binding = if context.height == 1 {
            let authenticated_genesis = genesis_body.ok_or(V2RunnerError::MissingGenesisBody)?;
            lane_work.bind_locked_genesis_body(&block, authenticated_genesis)
        } else {
            lane_work.bind_locked_global_body(&block)
        };
        if lane_binding == V2LaneIngressOutcome::Rejected {
            return Err(V2RunnerError::LaneCandidateBinding);
        }
        let current_owner = proposal_state.reconcile(LocalProposalOwner::from(current));
        let current_recovery_plan = locked_body_recovery_plan(
            current,
            local_validator,
            proposal_state.attempted,
            executor.can_schedule_local_proposal()?,
        );
        if !current_recovery_plan.may_repropose {
            continue;
        }
        // Keep the immutable bytes available under the PrepareQC round before
        // minting the later-round proposal. This preserves both admissible
        // progress branches: an already-started exact-origin recovery can
        // finish, while the current leader re-proposes the same subject
        // unchanged.
        executor.retain_locked_body_for_recovery(
            current.tag(),
            loaded_round,
            loaded_subject,
            canonical_wire.clone(),
            services,
        )?;
        submit_exact_body(
            context,
            current,
            canonical_wire,
            executor,
            services,
            proposal_state,
        )?;
        proposal_state.attempted = Some(current_owner);
        iroha_logger::debug!(
            height = current.tag().height(),
            proposal_view = current.tag().view(),
            locked_view = loaded_round.view,
            subject = ?loaded_subject,
            local_validator,
            "submitted exact locked body for local Sumeragi v2 reproposal"
        );
        return Ok(());
    }
    if directive.decided_subject().is_some() {
        return Ok(());
    }
    if directive.leader() != local_validator
        || proposal_state.attempted == Some(owner)
        || (directive.tag().view() == 0
            && height_started_at.elapsed() < block_cadence
            && context.height > 1)
    {
        return Ok(());
    }
    // Do not consume a fresh or retained candidate until the executor can
    // reserve its local StoreBody owner. Timers, retransmission, and
    // completions continue while this producer waits.
    if !executor.can_schedule_local_proposal()? {
        return Ok(());
    }
    // A locked directive can only consume its exact retained body. It must
    // never fall through to fresh candidate or genesis construction while the
    // asynchronous load is pending.
    if directive.locked_body().is_some() {
        return Ok(());
    }
    if context.height == 1 {
        let body = genesis_body.ok_or(V2RunnerError::MissingGenesisBody)?;
        // Genesis staging retains its deterministic execution image for application, while
        // consensus authenticates the canonical resultless proposal. Project exactly once at
        // that boundary; every downstream proposal path remains strict about result-bearing data.
        submit_exact_body(
            context,
            directive,
            canonical_height_one_proposal_wire(body)?,
            executor,
            services,
            proposal_state,
        )?;
        proposal_state.attempted = Some(owner);
    } else {
        if proposal_state
            .candidate_work_wait
            .is_some_and(|wait| wait.owner == owner && Instant::now() < wait.next_retry)
        {
            return Ok(());
        }
        let parent_body = if let Some(anchor) = &context.snapshot_bootstrap {
            let parent_height = NonZeroUsize::new(usize::try_from(anchor.snapshot_height)?)
                .ok_or(V2RunnerError::InvalidSnapshotBootstrapParent)?;
            if kura.get_block(parent_height).is_some() {
                return Err(V2RunnerError::InvalidSnapshotBootstrapParent);
            }
            None
        } else {
            let parent_height = usize::try_from(context.height.saturating_sub(1))?;
            let parent_height =
                NonZeroUsize::new(parent_height).ok_or(V2RunnerError::MissingParent)?;
            Some(
                kura.get_block(parent_height)
                    .ok_or(V2RunnerError::MissingParent)?,
            )
        };
        let (parent, logical_time) =
            match (context.snapshot_bootstrap.as_ref(), parent_body.as_deref()) {
                (Some(anchor), None) => (
                    CandidateParent::Snapshot(anchor),
                    snapshot_successor_logical_time(anchor, block_cadence)?,
                ),
                (None, Some(parent)) => {
                    let logical_time = parent
                        .header()
                        .creation_time()
                        .checked_add(block_cadence)
                        .ok_or(V2RunnerError::V2BlockTimeOverflow)?;
                    u64::try_from(logical_time.as_millis())
                        .map_err(|_| V2RunnerError::V2BlockTimeOverflow)?;
                    (CandidateParent::Block(parent), logical_time)
                }
                _ => return Err(V2RunnerError::InvalidSnapshotBootstrapParent),
            };
        let carrier_context_header =
            lane_work.merge_carrier_context_header(directive.tag().view())?;
        if carrier_context_header.creation_time() != logical_time {
            return Err(V2RunnerError::Candidate(
                "shared merge carrier timestamp differs from the frozen height cadence".to_owned(),
            ));
        }
        let (_, time_source) =
            iroha_primitives::time::TimeSource::new_mock(carrier_context_header.creation_time());
        let assembler = V2CandidateAssembler::new(candidate_limits, time_source.clone());
        let attachments = candidate_attachments(
            context,
            state,
            parent,
            directive.tag().view(),
            &carrier_context_header,
            npos_vrf,
        )?;
        let assembly = assembler.assemble(CandidateRequest {
            context,
            directive,
            local_validator,
            parent,
            state,
            queue,
            key_pair,
            output_guard,
            attachments,
            work_provider: &mut *lane_work,
        })?;
        let candidate = match assembly {
            CandidateAssemblyOutcome::Assembled(candidate) => candidate,
            CandidateAssemblyOutcome::NoProposalWork(report) => {
                let now = Instant::now();
                proposal_state.retire_unsubmitted_non_empty_retry(owner);
                if report.work_deferred > 0 {
                    proposal_state.defer_candidate_work(owner, now, candidate_work_wait_bound);
                } else {
                    // A clean idle height is not recovery work. Keep a bounded
                    // recheck so asynchronous queue/lane arrivals are observed
                    // promptly without manufacturing a cadence heartbeat.
                    proposal_state.candidate_work_wait = Some(CandidateWorkWait {
                        owner,
                        started_at: now,
                        next_retry: deadline_after(now, CANDIDATE_WORK_RECHECK),
                    });
                }
                iroha_logger::trace!(
                    height = owner.tag.height(),
                    view = owner.tag.view(),
                    ?report,
                    "deferred Sumeragi v2 proposal because no proposal work is ready"
                );
                return Ok(());
            }
        };
        let tag = candidate.tag();
        if tag != owner.tag {
            return Err(V2RunnerError::StaleTag);
        }
        let report = candidate.scan_report();
        if proposal_state.non_empty_retry != Some(owner)
            && report.selected == 0
            && report.work_deferred > 0
        {
            let now = Instant::now();
            proposal_state.defer_candidate_work(owner, now, candidate_work_wait_bound);
            return Ok(());
        }
        proposal_state.candidate_work_wait = None;
        if lane_work.bind_local_candidate(round_for_tag(context, tag)?, candidate.block().hash())
            == V2LaneIngressOutcome::Rejected
        {
            // Binding happens before body storage and reducer submission.
            // Drop the abandoned candidate first so its ordinary queue lease
            // is available to a later-height reproposal.
            drop(candidate);
            match proposal_state.handle_candidate_binding_rejection(owner) {
                LocalValidationDisposition::RetryNonEmpty => {
                    iroha_logger::warn!(
                        height = tag.height(),
                        view = tag.view(),
                        "discarded an unsubmitted candidate after lane-local ownership binding rejected; retrying with non-empty work only"
                    );
                    return Ok(());
                }
                LocalValidationDisposition::FatalNonEmpty | LocalValidationDisposition::Ignored => {
                    return Err(V2RunnerError::LaneCandidateBinding);
                }
            }
        }
        let (_block, canonical_wire, encoded_payload, events, report, selection_lease) =
            candidate.into_parts();
        let subject = encoded_payload.manifest().subject;
        proposal_state.pending_events = Some(PendingLocalEvents {
            owner,
            subject,
            events,
        });
        iroha_logger::debug!(?report, "assembled bounded Sumeragi v2 candidate");
        submit_encoded_body(
            owner,
            canonical_wire,
            encoded_payload,
            executor,
            services,
            proposal_state,
        )?;
        proposal_state.global_selection = Some(PendingGlobalSelection {
            owner,
            subject,
            _lease: selection_lease,
        });
        proposal_state.attempted = Some(owner);
    }
    Ok(())
}
fn canonical_height_one_proposal_wire(body: &SignedBlock) -> Result<Vec<u8>, V2RunnerError> {
    body.canonical_resultless_proposal()
        .encode_wire()
        .map_err(|error| V2RunnerError::Candidate(error.to_string()))
}
fn submit_exact_body(
    context: &wire::HeightContext,
    directive: LocalProposalDirective,
    canonical_wire: Vec<u8>,
    executor: &mut V2EffectExecutor,
    services: &mut ProductionV2Services,
    proposal_state: &mut LocalProposalState,
) -> Result<(), V2RunnerError> {
    let payload = encode_exact_local_body(
        context,
        directive.tag(),
        directive.locked_subject(),
        &canonical_wire,
    )?;
    submit_encoded_body(
        LocalProposalOwner::from(directive),
        canonical_wire,
        payload,
        executor,
        services,
        proposal_state,
    )
}
fn encode_exact_local_body(
    context: &wire::HeightContext,
    tag: EventTag,
    locked_subject: Option<wire::BlockSubject>,
    canonical_wire: &[u8],
) -> Result<EncodedV2Payload, V2RunnerError> {
    let block = iroha_data_model::block::decode_framed_signed_block(canonical_wire)
        .map_err(|error| V2RunnerError::Candidate(error.to_string()))?;
    if !block.is_resultless_proposal() {
        return Err(V2RunnerError::ResultBearingProposal);
    }
    let subject = wire::BlockSubject {
        parent_block_hash: block.header().prev_block_hash(),
        block_hash: block.hash(),
        payload_hash: Hash::new(&canonical_wire),
    };
    if locked_subject.is_some_and(|locked| locked != subject) {
        return Err(V2RunnerError::LockedBodyMismatch);
    }
    let round = round_for_tag(context, tag)?;
    encode_payload(context, round, subject, canonical_wire)
        .map_err(|error| V2RunnerError::Candidate(error.to_string()))
}
fn submit_encoded_body(
    owner: LocalProposalOwner,
    canonical_wire: Vec<u8>,
    payload: EncodedV2Payload,
    executor: &mut V2EffectExecutor,
    services: &mut ProductionV2Services,
    proposal_state: &mut LocalProposalState,
) -> Result<(), V2RunnerError> {
    let manifest = services
        .register_outbound_payload(owner.tag, payload)
        .map_err(V2RunnerError::Service)?;
    proposal_state.submitted = Some((owner, manifest.subject));
    executor.admit_local_proposal(owner.tag, manifest, canonical_wire, services)?;
    Ok(())
}
fn drive_block_sync(
    now: Instant,
    next_attempt: &mut Instant,
    retransmit_interval: Duration,
    request_hash: &mut Option<HashOf<wire::CommitCertificateRequest>>,
    discovery: &mut V2BlockSyncDiscovery,
    key_pair: &KeyPair,
    output_guard: &ConsensusOutputGuard,
    services: &ProductionV2Services,
) -> Result<(), V2RunnerError> {
    if now < *next_attempt {
        return Ok(());
    }
    let next = deadline_after(now, retransmit_interval);
    if let Some(hash) = request_hash.as_ref() {
        let operation = output_guard
            .begin_fail_stop_operation()
            .ok_or(V2RunnerError::RestartRequired)?;
        let message = discovery
            .retransmit(*hash)
            .ok_or(V2RunnerError::BlockSyncRequestDisappeared)?;
        services
            .broadcast_to_voters_while_guarded(message, operation.permit())
            .map_err(V2RunnerError::Service)?;
        operation.complete();
    } else {
        let operation = output_guard
            .begin_fail_stop_operation()
            .ok_or(V2RunnerError::RestartRequired)?;
        let message = discovery.begin(key_pair)?;
        let wire::ConsensusMessageV2Payload::CommitCertificateRequest(request) = &message.payload
        else {
            return Err(V2RunnerError::BlockSyncRequestDisappeared);
        };
        *request_hash = Some(HashOf::new(request));
        if let Err(error) = services.broadcast_to_voters_while_guarded(message, operation.permit())
        {
            drop(operation);
            return Err(V2RunnerError::Service(error));
        }
        operation.complete();
    }
    *next_attempt = next;
    Ok(())
}
fn broadcast_npos_vrf_messages(
    messages: impl IntoIterator<Item = wire::ConsensusMessageV2>,
    output_guard: &ConsensusOutputGuard,
    services: &ProductionV2Services,
) -> Result<(), V2RunnerError> {
    for message in messages {
        let operation = output_guard
            .begin_fail_stop_operation()
            .ok_or(V2RunnerError::RestartRequired)?;
        services
            .broadcast_to_voters_while_guarded(message, operation.permit())
            .map_err(V2RunnerError::Service)?;
        operation.complete();
    }
    Ok(())
}

include!("v2_runner/decided_lane_recovery.rs");
include!("v2_runner/outer_ingress_cursor.rs");

/// Exercise a closure with one genuine borrow-bound current runner turn.
///
/// The cursor advances through earlier turns exactly as production does. The
/// closure's return type cannot borrow the local cursor, so it must consume or
/// drop any pass-through authority before returning. The second tuple element
/// is the target observed immediately afterwards and proves the exact Drop
/// transition behavior without minting a free-standing production snapshot.
#[cfg(test)]
pub(in crate::sumeragi) fn with_lifecycle_current_runner_turn_for_test<R>(
    context: &wire::HeightContext,
    target: LifecycleRunnerRankTarget,
    service: impl for<'cursor> FnOnce(LifecycleCurrentRunnerTurn<'cursor>) -> R,
) -> (R, LifecycleRunnerRankTarget) {
    let mut turns = OuterIngressTurns::new(2, context.id(), context.height);
    loop {
        let turn = turns
            .next_current()
            .expect("two-cycle fixture reaches every outer runner target");
        if turn.target() != target {
            drop(turn);
            continue;
        }
        let result = service(turn);
        let next = turns
            .next_current()
            .expect("one serviced fixture turn leaves a successor target");
        let next_target = next.target();
        drop(next);
        return (result, next_target);
    }
}

fn is_remote_block_sync_rejection(error: &V2BlockSyncError) -> bool {
    matches!(
        error,
        V2BlockSyncError::Wire(_)
            | V2BlockSyncError::Transport(_)
            | V2BlockSyncError::ConflictingServerRequest { .. }
            | V2BlockSyncError::ConflictingHistoricalBodyRequest { .. }
    )
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GuardedBlockSyncServeOutcome {
    Posted,
    NoResponse,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BoundBlockSyncServeOutcome {
    Posted,
    VolatileNoResponse,
    VolatileRemoteRejection,
}
fn serve_block_sync_while_guarded<Response>(
    output_guard: &ConsensusOutputGuard,
    serve: impl FnOnce() -> Result<Option<Response>, V2BlockSyncError>,
    post: impl FnOnce(Response, &ConsensusOutputPermit<'_>) -> Result<(), String>,
) -> Result<GuardedBlockSyncServeOutcome, V2BlockSyncError> {
    let operation = output_guard
        .begin_fail_stop_operation()
        .ok_or(V2BlockSyncError::RestartRequired)?;
    match serve() {
        Ok(Some(response)) => {
            if let Err(error) = post(response, operation.permit()) {
                drop(operation);
                return Err(V2BlockSyncError::ResponsePost(error));
            }
            operation.complete();
            Ok(GuardedBlockSyncServeOutcome::Posted)
        }
        Ok(None) => {
            operation.complete();
            Ok(GuardedBlockSyncServeOutcome::NoResponse)
        }
        Err(error) if is_remote_block_sync_rejection(&error) => {
            operation.complete();
            Err(error)
        }
        Err(error) => {
            drop(operation);
            Err(error)
        }
    }
}
fn finalize_bound_block_sync_serve(
    served: Result<GuardedBlockSyncServeOutcome, V2BlockSyncError>,
    retire_volatile: impl FnOnce() -> Result<(), V2RunnerError>,
    observe_remote_rejection: impl FnOnce(&V2BlockSyncError),
) -> Result<BoundBlockSyncServeOutcome, V2RunnerError> {
    match served {
        Ok(GuardedBlockSyncServeOutcome::Posted) => Ok(BoundBlockSyncServeOutcome::Posted),
        Ok(GuardedBlockSyncServeOutcome::NoResponse) => {
            retire_volatile()?;
            Ok(BoundBlockSyncServeOutcome::VolatileNoResponse)
        }
        Err(error) if is_remote_block_sync_rejection(&error) => {
            retire_volatile()?;
            observe_remote_rejection(&error);
            Ok(BoundBlockSyncServeOutcome::VolatileRemoteRejection)
        }
        Err(error) => Err(error.into()),
    }
}
fn enqueue_control(
    executor: &mut V2EffectExecutor,
    receiver: &FairV2Ingress,
    message: wire::ConsensusMessageV2,
    ingress_ownership: FairV2IngressOwnershipEvidence,
) -> Result<(), V2RunnerError> {
    let terminal_ownership = ingress_ownership.clone();
    complete_control_ingress_admission(
        receiver,
        &terminal_ownership,
        executor.enqueue_network_with_ingress_ownership(message, ingress_ownership),
    )
}
fn complete_control_ingress_admission(
    receiver: &FairV2Ingress,
    terminal_ownership: &FairV2IngressOwnershipEvidence,
    admission: Result<EventTag, NetworkIngressError>,
) -> Result<(), V2RunnerError> {
    match admission {
        Ok(_) => Ok(()),
        Err(NetworkIngressError::FailClosed) => {
            mark_leader_wire_volatile(receiver, terminal_ownership)?;
            Err(V2RunnerError::RuntimeFailClosed)
        }
        Err(NetworkIngressError::Authentication(error)) => {
            iroha_logger::debug!(%error, "rejected Sumeragi v2 control ingress");
            mark_leader_wire_volatile(receiver, terminal_ownership)?;
            Ok(())
        }
        Err(NetworkIngressError::Backpressure(error)) => {
            mark_leader_wire_volatile(receiver, terminal_ownership)?;
            Err(V2RunnerError::RuntimeAdmissionInvariant(error.to_string()))
        }
        Err(NetworkIngressError::TransportPayload) => {
            mark_leader_wire_volatile(receiver, terminal_ownership)?;
            Err(V2RunnerError::RuntimeAdmissionInvariant(
                "transport payload reached reducer-control admission".to_owned(),
            ))
        }
    }
}
fn mark_leader_wire_volatile(
    receiver: &FairV2Ingress,
    ownership: &FairV2IngressOwnershipEvidence,
) -> Result<(), V2RunnerError> {
    if let Some(receipt) = ownership.leader_wire_runtime_receipt() {
        receiver
            .mark_leader_wire_volatile_terminal(receipt)
            .map_err(V2RunnerError::Service)?;
    }
    Ok(())
}
fn commit_certificate_admission_completed(
    admission: Result<(), CommitCertificateAdmissionError<NetworkIngressError>>,
) -> Result<bool, V2RunnerError> {
    match admission {
        Ok(()) => Ok(true),
        Err(CommitCertificateAdmissionError::Enqueue(NetworkIngressError::FailClosed)) => {
            Err(V2RunnerError::RuntimeFailClosed)
        }
        Err(CommitCertificateAdmissionError::Enqueue(NetworkIngressError::Backpressure(error))) => {
            // The dequeue predicate couples the outer occurrence to this exact
            // Progress admission. Treat a defensive mismatch as retryable: the
            // discovery request remains outstanding and retransmission can
            // supply another occurrence after capacity changes.
            iroha_logger::debug!(%error, "deferred authenticated CommitQC response after runtime backpressure");
            Ok(false)
        }
        Err(CommitCertificateAdmissionError::Enqueue(error)) => {
            iroha_logger::debug!(%error, "deferred authenticated CommitQC response");
            Ok(false)
        }
        Err(CommitCertificateAdmissionError::MismatchedReducerAdmission) => {
            Err(V2RunnerError::RuntimeAdmissionInvariant(
                "authenticated CommitQC discovery received foreign reducer admission ownership"
                    .to_owned(),
            ))
        }
        Err(CommitCertificateAdmissionError::RequestDisappeared) => {
            Err(V2RunnerError::BlockSyncRequestDisappeared)
        }
        Err(CommitCertificateAdmissionError::RefinementRejected) => {
            Err(V2RunnerError::RuntimeAdmissionInvariant(
                "authenticated CommitQC discovery failed exact historical refinement".to_owned(),
            ))
        }
    }
}
fn advance_executor(
    receiver: &FairV2Ingress,
    executor: &mut V2EffectExecutor,
    services: &mut ProductionV2Services,
    limit: usize,
) -> Result<(), V2RunnerError> {
    for _ in 0..limit.max(1) {
        executor.set_ingress_physical_cut(receiver.next_physical_admission_ordinal())?;
        match executor.step(Instant::now(), services)? {
            EffectExecutorStep::Idle => break,
            EffectExecutorStep::Advanced { .. } => {
                // A PrepareQC can replace the protected lock without changing
                // the EventTag. Reconcile immediately after every serialized
                // transition so later ingress in the same outer batch cannot
                // reclaim service ownership for the superseded subject.
                let _ = reconcile_executor_locked_body(executor, services)?;
            }
        }
    }
    Ok(())
}
/// Execute at most one serialized transition from an older lifecycle before
/// an exact Serve target turn.
///
/// Lock reconciliation and every other local producer stay behind the target;
/// the ordinary runner path performs them after the barrier drains. This is
/// deliberately not a loop, even when the older causal episode remains live.
fn advance_executor_once_before_exact_serve(
    receiver: &FairV2Ingress,
    executor: &mut V2EffectExecutor,
    services: &mut ProductionV2Services,
) -> Result<(), V2RunnerError> {
    executor.set_ingress_physical_cut(receiver.next_physical_admission_ordinal())?;
    let _ = executor.step(Instant::now(), services)?;
    Ok(())
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CertifiedServeBarrierLivenessAction {
    TimeoutVoteEpisode,
    TimeoutRecoveryPrefix,
    Pacemaker,
}
/// Service the complete timeout-recovery suffix of one selected Serve turn.
///
/// The still-selected Serve carrier admits one eligible direct-roster timeout
/// vote, services an already-owned prefix completion, and runs one typed
/// pacemaker transition in that exact order.
fn service_certified_serve_barrier_liveness_turn<E>(
    recovering_interrupted_tip: bool,
    mut service: impl FnMut(CertifiedServeBarrierLivenessAction) -> Result<(), E>,
) -> Result<(), E> {
    if !recovering_interrupted_tip {
        service(CertifiedServeBarrierLivenessAction::TimeoutVoteEpisode)?;
    }
    service(CertifiedServeBarrierLivenessAction::TimeoutRecoveryPrefix)?;
    service_certified_serve_barrier_pacemaker_turn(recovering_interrupted_tip, || {
        service(CertifiedServeBarrierLivenessAction::Pacemaker)
    })
}
/// Keep the pacemaker live for the full lifetime of a certified Serve barrier.
///
/// The predecessor admission does not gate service: a backpressured target can
/// remain in fair ingress after that transient aperture closes, while the
/// absolute timeout and certified progress roots remain independently live.
fn service_certified_serve_barrier_pacemaker_turn<E>(
    recovering_interrupted_tip: bool,
    service: impl FnOnce() -> Result<(), E>,
) -> Result<(), E> {
    if recovering_interrupted_tip {
        return Ok(());
    }
    service()
}
/// Execute at most one typed timeout/Progress-root transition while an exact
/// transport episode retains ordinary ownership.
fn advance_pacemaker_once(
    receiver: &FairV2Ingress,
    executor: &mut V2EffectExecutor,
    services: &mut ProductionV2Services,
) -> Result<(), V2RunnerError> {
    executor.set_ingress_physical_cut(receiver.next_physical_admission_ordinal())?;
    let _ = executor.step_pacemaker_once(Instant::now(), services)?;
    Ok(())
}
fn reconcile_executor_locked_body(
    executor: &mut V2EffectExecutor,
    services: &mut ProductionV2Services,
) -> Result<LocalProposalDirective, V2RunnerError> {
    let directive = executor.local_proposal_directive()?;
    if directive.decided_subject().is_none()
        && let Some(lock) = directive.locked_body()
    {
        executor.reconcile_locked_body_for_recovery(directive.tag(), lock, services)?;
    }
    Ok(directive)
}
/// Keep lane-work construction behind the completed interrupted-tip
/// application boundary.
///
/// The recovery worker reports completion only after canonical State,
/// post-apply metadata, and strict Native AMX evidence repair are durable.
/// Keeping the constructor in this continuation makes that ordering explicit
/// and independently testable.
fn construct_after_pending_tip_application_recovery<T>(
    recovering_interrupted_tip: bool,
    recovery_complete: bool,
    construct: impl FnOnce() -> Result<T, V2RunnerError>,
) -> Result<T, V2RunnerError> {
    if recovering_interrupted_tip && !recovery_complete {
        return Err(V2RunnerError::PendingTipRecoveryIncomplete);
    }
    construct()
}

fn local_validator_index(
    context: &wire::HeightContext,
    local_peer: &PeerId,
    role: NodeRole,
) -> Result<Option<wire::ValidatorIndex>, V2RunnerError> {
    let index = context
        .roster
        .iter()
        .position(|entry| &entry.validator == local_peer)
        .map(u32::try_from)
        .transpose()?;
    match (role, index) {
        (NodeRole::Observer, _) => Ok(None),
        (NodeRole::Validator, Some(index)) => Ok(Some(index)),
        // Roster changes are authenticated height transitions, not process
        // role changes. A configured validator absent from this height runs
        // the global protocol as an observer while retaining authority to
        // serve any independently frozen lane descriptor that still names
        // its key, including unfinished predecessor work.
        (NodeRole::Validator, None) => Ok(None),
    }
}
/// Claim the immutable process capability independently of this height's roster duty.
///
/// Configured validators must own one generation even while absent from the recovered global
/// roster: a later height may rotate them back in, and a retained frozen lane descriptor may
/// still name their key. Explicit observers never mutate this Kura lifecycle namespace.
fn claim_runner_lifecycle_process_generation(
    role: NodeRole,
    kura: &Kura,
    context: &wire::HeightContext,
    local_peer: &PeerId,
) -> Result<Option<AutonomousLifecycleProcessGenerationClaim>, V2RunnerError> {
    match role {
        NodeRole::Observer => Ok(None),
        NodeRole::Validator => kura
            .claim_autonomous_lifecycle_process_generation(context.network_id, local_peer)
            .map(Some)
            .map_err(|error| {
                V2RunnerError::Service(format!(
                    "failed to claim the durable autonomous lifecycle process generation: {error}"
                ))
            }),
    }
}
fn round_for_tag(
    context: &wire::HeightContext,
    tag: EventTag,
) -> Result<wire::ConsensusRound, V2RunnerError> {
    if tag.height() != context.height {
        return Err(V2RunnerError::StaleTag);
    }
    Ok(wire::ConsensusRound {
        context_id: context.id(),
        height: context.height,
        view: tag.view(),
    })
}
fn runtime_queue_config(config: &SumeragiV2Config) -> Result<RuntimeQueueConfig, V2RunnerError> {
    Ok(RuntimeQueueConfig::new(
        usize::try_from(config.limits.runtime_command_capacity)?,
        usize::try_from(config.limits.runtime_progress_reserve)?,
        usize::try_from(config.limits.runtime_completion_reserve)?,
    ))
}
fn effect_queue_config(config: &SumeragiV2Config) -> Result<EffectQueueConfig, V2RunnerError> {
    let max_pending_work = usize::try_from(config.limits.effect_work_capacity)?;
    let completion_reserve = usize::try_from(config.limits.runtime_completion_reserve)?;
    if max_pending_work > completion_reserve {
        return Err(V2RunnerError::EffectWorkExceedsCompletionReserve {
            pending: max_pending_work,
            reserve: completion_reserve,
        });
    }
    Ok(EffectQueueConfig::new(
        max_pending_work,
        usize::try_from(config.limits.ready_body_capacity)?,
        config.limits.ready_body_bytes,
        usize::try_from(config.limits.certified_request_capacity)?,
    ))
}
fn lane_work_limits(
    config: &SumeragiV2Config,
    reply_source_capacity: usize,
    consensus_frame_byte_capacity: usize,
    block_sync_frame_byte_capacity: usize,
    historical_recovery_retry_floor: Duration,
    historical_recovery_retry_ceiling: Duration,
) -> Result<V2LaneWorkLimits, V2RunnerError> {
    let non_zero = |value: u64| {
        usize::try_from(value)
            .ok()
            .and_then(NonZeroUsize::new)
            .ok_or(V2RunnerError::InvalidLimits)
    };
    let non_zero_u32 = |value: u64| {
        u32::try_from(value)
            .ok()
            .and_then(std::num::NonZeroU32::new)
            .ok_or(V2RunnerError::InvalidLimits)
    };
    let merge_leader_body_frame_headroom_bytes =
        non_zero(config.limits.merge_leader_body_frame_headroom_bytes)?;
    if merge_leader_body_frame_headroom_bytes.get() >= consensus_frame_byte_capacity {
        return Err(V2RunnerError::InvalidLimits);
    }
    let autonomous_producer_recheck_ms =
        NonZeroU64::new(config.limits.autonomous_producer_recheck_ms)
            .ok_or(V2RunnerError::InvalidLimits)?;
    let native_amx_signing_guard_limits = crate::native_amx::NativeAmxSigningGuardLimits::new(
        non_zero(config.limits.native_amx_signing_guard_record_capacity)?,
        non_zero(config.limits.native_amx_signing_guard_record_bytes)?,
        non_zero(config.limits.native_amx_signing_guard_anchor_bytes)?,
    )
    .map_err(|_| V2RunnerError::InvalidLimits)?;
    let max_transactions = non_zero(config.limits.max_transactions)?;
    let control_queue_capacity = non_zero(config.limits.control_queue_capacity)?;
    let native_body_buckets_per_source = crate::native_amx::MAX_NATIVE_AMX_PARTICIPANT_LEGS
        .checked_mul(2)
        .ok_or(V2RunnerError::InvalidLimits)?;
    let required_native_signing_records = max_transactions
        .get()
        .checked_mul(crate::native_amx::MAX_NATIVE_AMX_PARTICIPANT_LEGS)
        .ok_or(V2RunnerError::InvalidLimits)?;
    if native_amx_signing_guard_limits.max_records.get() < required_native_signing_records {
        return Err(V2RunnerError::NativeAmxSigningCapacity {
            configured: native_amx_signing_guard_limits.max_records.get(),
            required: required_native_signing_records,
        });
    }
    let native_session_capacity =
        NonZeroUsize::new(control_queue_capacity.get().max(max_transactions.get()))
            .expect("the maximum of non-zero capacities is non-zero");
    let native_body_buckets_per_session =
        NonZeroUsize::new(max_transactions.get().max(native_body_buckets_per_source))
            .expect("the maximum of non-zero capacities is non-zero");
    let merge_sidecar_request_timeout_ms =
        NonZeroU64::new(config.limits.merge_sidecar_request_timeout_ms)
            .ok_or(V2RunnerError::InvalidLimits)?;
    let merge_sidecar_limits = MergeSidecarLimits::new(
        non_zero(config.limits.merge_sidecar_inbound_session_capacity)?,
        non_zero(config.limits.merge_sidecar_inbound_sessions_per_peer)?,
        non_zero(config.limits.merge_sidecar_inbound_assembly_bytes)?,
        non_zero(config.limits.merge_sidecar_inbound_assembly_bytes_per_peer)?,
        non_zero(config.limits.merge_sidecar_deferred_block_capacity)?,
        NonZeroU64::new(config.limits.merge_sidecar_future_block_distance)
            .ok_or(V2RunnerError::InvalidLimits)?,
        Duration::from_millis(merge_sidecar_request_timeout_ms.get()),
        non_zero(config.limits.merge_sidecar_outbound_sessions_per_source)?,
        non_zero(config.limits.merge_sidecar_outbound_bytes_per_source)?,
        non_zero(config.limits.merge_sidecar_server_request_gates_per_source)?,
    )
    .map_err(|_| V2RunnerError::InvalidLimits)?;
    let merge_signing_guard_limits = MergeSigningGuardLimits::new(
        non_zero(config.limits.merge_signing_guard_record_capacity)?,
        non_zero(config.limits.merge_signing_guard_record_bytes)?,
        non_zero(config.limits.merge_signing_guard_total_bytes)?,
    )
    .map_err(|_| V2RunnerError::InvalidLimits)?;
    Ok(V2LaneWorkLimits::new(
        control_queue_capacity,
        max_transactions,
        non_zero(config.limits.effect_work_capacity)?,
        non_zero(config.limits.chunk_queue_capacity)?,
        non_zero(config.limits.certified_request_capacity)?,
        control_queue_capacity,
        NonZeroUsize::new(reply_source_capacity).ok_or(V2RunnerError::InvalidLimits)?,
        NonZeroUsize::new(consensus_frame_byte_capacity).ok_or(V2RunnerError::InvalidLimits)?,
        NonZeroUsize::new(block_sync_frame_byte_capacity).ok_or(V2RunnerError::InvalidLimits)?,
        non_zero(config.limits.authenticated_merge_qc_capacity)?,
        merge_leader_body_frame_headroom_bytes,
        non_zero(config.limits.autonomous_carrier_headroom_bytes)?,
        Duration::from_millis(autonomous_producer_recheck_ms.get()),
        historical_recovery_retry_floor,
        historical_recovery_retry_ceiling,
        non_zero_u32(config.limits.historical_recovery_stuck_attempts)?,
        non_zero_u32(config.limits.historical_recovery_retry_tier_attempts)?,
        non_zero_u32(config.limits.historical_recovery_max_retry_tier)?,
        non_zero(config.limits.sidecar_service_burst)?,
        merge_sidecar_limits,
        merge_signing_guard_limits,
        native_amx_signing_guard_limits,
    )
    .with_native_cache_limits(
        native_session_capacity,
        native_body_buckets_per_session,
        max_transactions,
    ))
}
fn candidate_limits(
    context: &wire::HeightContext,
    config: &SumeragiV2Config,
) -> Result<CandidateLimits, V2RunnerError> {
    let max_transactions = NonZeroUsize::new(usize::try_from(config.limits.max_transactions)?)
        .ok_or(V2RunnerError::InvalidLimits)?;
    let context_payload = usize::try_from(context.da_layout.max_payload_size_bytes)?;
    let configured_payload = usize::try_from(config.limits.max_payload_bytes)?;
    let max_payload = NonZeroUsize::new(context_payload.min(configured_payload))
        .ok_or(V2RunnerError::InvalidLimits)?;
    CandidateLimits::new(
        max_transactions,
        max_payload,
        NonZeroUsize::new(usize::try_from(config.limits.max_queue_scan)?)
            .ok_or(V2RunnerError::InvalidLimits)?,
    )
    .map_err(Into::into)
}
fn candidate_attachments(
    context: &wire::HeightContext,
    state: &State,
    parent: CandidateParent<'_>,
    view: wire::View,
    round_header: &BlockHeader,
    npos_vrf: &V2NposVrfLifecycle,
) -> Result<CandidateAttachments, V2RunnerError> {
    if round_header.height().get() != context.height
        || round_header.prev_block_hash() != Some(parent.hash())
        || round_header.view_change_index() != view
        || round_header.merkle_root().is_some()
        || round_header.result_merkle_root().is_some()
    {
        return Err(V2RunnerError::Candidate(
            "certified merge carrier probe differs from the frozen round".to_owned(),
        ));
    }
    let effects = if context.mode == wire::ConsensusMode::Npos {
        super::penalties::PenaltyApplier::from_parts(
            state,
            #[cfg(feature = "telemetry")]
            Some(state.metrics()),
            #[cfg(not(feature = "telemetry"))]
            None,
        )
        .derive_npos_consensus_effects(context.height, npos_vrf.pending_records())
        .map_err(|error| V2RunnerError::Candidate(error.to_string()))?
    } else {
        Default::default()
    };
    let npos_consensus_effects = (!effects.is_empty()).then_some(effects);
    super::v2_npos::validate_candidate_records(context, state, npos_consensus_effects.as_ref())
        .map_err(|error| V2RunnerError::Candidate(error.to_string()))?;
    let merge_selection = certified_merge_selection_for_npos(npos_consensus_effects.is_some());
    if merge_selection == PendingCertifiedMergeSelection::ControlOnly {
        iroha_logger::debug!(
            height = context.height,
            view,
            "prioritizing deterministic NPoS effects before a certified execution carrier"
        );
    }
    let expected_merge_epoch = state
        .merge_ledger()
        .latest()
        .map_or(1, |latest| latest.epoch_id.saturating_add(1));
    let selected_merge_entry = state
        .select_pending_certified_merge_entry_for_round(
            round_header,
            expected_merge_epoch,
            merge_selection,
        )
        .map_err(|error| V2RunnerError::Candidate(error.to_string()))?;
    let certified_merge_entry = selected_merge_entry
        .map(|(_, entry, _)| entry)
        .map(|entry| {
            super::v2_lane_work::authenticate_merge_entry_for_height_context(context, &entry)
                .map(|()| entry)
                .map_err(V2RunnerError::Candidate)
        })
        .transpose()?;
    let parent_creation_time = match parent {
        CandidateParent::Block(parent) => parent.header().creation_time(),
        CandidateParent::Snapshot(anchor) => {
            Duration::from_millis(anchor.snapshot_block_creation_time_ms)
        }
    };
    Ok(CandidateAttachments {
        time_trigger_clock_progress_required: state
            .time_trigger_clock_progress_required_fast(parent_creation_time),
        npos_consensus_effects,
        certified_merge_carrier_header: certified_merge_entry
            .as_ref()
            .and_then(|entry| entry.execution_batch.as_ref())
            .map(|batch| batch.application_block_header.clone()),
        certified_merge_entry,
        ..CandidateAttachments::default()
    })
}
const fn certified_merge_selection_for_npos(
    has_npos_effects: bool,
) -> PendingCertifiedMergeSelection {
    if has_npos_effects {
        PendingCertifiedMergeSelection::ControlOnly
    } else {
        PendingCertifiedMergeSelection::Any
    }
}
fn adapter_fingerprints(local_peer: &PeerId, config: &SumeragiV2Config) -> AdapterFingerprints {
    let node = Hash::new(local_peer.encode());
    let mut build_preimage = env!("CARGO_PKG_VERSION").as_bytes().to_vec();
    build_preimage.extend_from_slice(
        option_env!("GIT_COMMIT_HASH")
            .unwrap_or("unknown")
            .as_bytes(),
    );
    AdapterFingerprints {
        node,
        build: Hash::new(build_preimage),
        config: config.fingerprint(),
    }
}
fn apply_bounded_sidecar_admissions<T, Error>(
    limit: usize,
    mut next: impl FnMut() -> Result<Option<T>, Error>,
    mut apply: impl FnMut(T) -> Result<(), Error>,
) -> Result<usize, Error> {
    let mut applied = 0usize;
    for _ in 0..limit.max(1) {
        let Some(admission) = next()? else {
            break;
        };
        apply(admission)?;
        applied = applied.saturating_add(1);
    }
    Ok(applied)
}
fn apply_certified_merge_sidecar_chunk_admissions(
    lane_work: &mut V2LaneWorkAdapter,
    services: &ProductionV2Services,
    limit: usize,
) -> Result<(), V2RunnerError> {
    apply_bounded_sidecar_admissions(
        limit,
        || {
            let mut admissions = services
                .drain_certified_merge_sidecar_chunk_admissions(1)
                .map_err(V2RunnerError::Service)?;
            Ok(admissions.pop())
        },
        |admission| {
            lane_work
                .acknowledge_certified_merge_sidecar_chunk_admission(&admission, Instant::now())
                .map_err(V2RunnerError::LaneWork)
        },
    )?;
    Ok(())
}
fn retry_exact_output_and_apply_sidecar_admissions(
    lane_work: &mut V2LaneWorkAdapter,
    services: &ProductionV2Services,
    limit: usize,
) -> Result<bool, V2RunnerError> {
    let _ = apply_native_amx_output_retention(lane_work, services)?;
    let _ = apply_retired_historical_recovery_requests(lane_work, services)?;
    let _ = apply_retired_merge_sidecar_requests(lane_work, services)?;
    let _ = apply_acknowledged_merge_sidecar_closes(lane_work, services)?;
    apply_certified_merge_sidecar_closed_prefixes(lane_work, services)?;
    let pending = services
        .retry_pending_exact_output()
        .map_err(V2RunnerError::Service)?;
    apply_certified_merge_sidecar_chunk_admissions(lane_work, services, limit)?;
    Ok(pending)
}
fn apply_native_amx_output_retention(
    lane_work: &V2LaneWorkAdapter,
    services: &ProductionV2Services,
) -> Result<usize, V2RunnerError> {
    let Some((round, terminal, expected_requests)) = lane_work.native_amx_output_retention() else {
        return Ok(0);
    };
    let global = services
        .retain_certified_global_view_output(round)
        .map_err(V2RunnerError::Service)?;
    let native = services
        .retain_native_amx_round(round, terminal, &expected_requests)
        .map_err(V2RunnerError::Service)?;
    Ok(global.saturating_add(native))
}
/// Cancel service-owned historical requests whose adapter owner completed
/// before retrying any retained network occurrence.
pub(in crate::sumeragi) fn apply_retired_historical_recovery_requests(
    lane_work: &mut V2LaneWorkAdapter,
    services: &ProductionV2Services,
) -> Result<usize, V2RunnerError> {
    let request_hashes = lane_work.drain_retired_historical_recovery_request_hashes();
    if request_hashes.is_empty() {
        return Ok(0);
    }
    match services.cancel_historical_lane_recovery_requests(&request_hashes) {
        Ok(cancelled) => Ok(cancelled),
        Err(error) => {
            lane_work.requeue_retired_historical_recovery_request_hashes(request_hashes)?;
            Err(V2RunnerError::Service(error))
        }
    }
}
/// Cancel service-owned sidecar requests whose exact transport attempt retired.
pub(in crate::sumeragi) fn apply_retired_merge_sidecar_requests(
    lane_work: &mut V2LaneWorkAdapter,
    services: &ProductionV2Services,
) -> Result<usize, V2RunnerError> {
    let request_hashes = lane_work.drain_retired_merge_sidecar_request_hashes();
    if request_hashes.is_empty() {
        return Ok(0);
    }
    match services.cancel_certified_merge_sidecar_requests(&request_hashes) {
        Ok(cancelled) => Ok(cancelled),
        Err(error) => {
            lane_work.requeue_retired_merge_sidecar_request_hashes(request_hashes)?;
            Err(V2RunnerError::Service(error))
        }
    }
}
/// Cancel requester Close retries covered by an authenticated cumulative ACK.
pub(in crate::sumeragi) fn apply_acknowledged_merge_sidecar_closes(
    lane_work: &mut V2LaneWorkAdapter,
    services: &ProductionV2Services,
) -> Result<usize, V2RunnerError> {
    let acknowledgements = lane_work.drain_acknowledged_merge_sidecar_closes();
    if acknowledgements.is_empty() {
        return Ok(0);
    }
    match services.cancel_acknowledged_certified_merge_sidecar_closes(&acknowledgements) {
        Ok(cancelled) => Ok(cancelled),
        Err(error) => {
            lane_work.requeue_acknowledged_merge_sidecar_closes(acknowledgements)?;
            Err(V2RunnerError::Service(error))
        }
    }
}
include!("v2_runner/finalized_output_rollover.rs");
fn apply_certified_merge_sidecar_closed_prefixes(
    lane_work: &mut V2LaneWorkAdapter,
    services: &ProductionV2Services,
) -> Result<(), V2RunnerError> {
    apply_certified_merge_sidecar_closed_prefixes_with(lane_work, |prefix| {
        services
            .close_certified_merge_sidecar_prefix(prefix)
            .map(|_| ())
    })
}
fn apply_certified_merge_sidecar_closed_prefixes_with(
    lane_work: &mut V2LaneWorkAdapter,
    mut apply: impl FnMut(&CertifiedMergeSidecarClosedPrefix) -> Result<(), String>,
) -> Result<(), V2RunnerError> {
    let mut prefixes = std::collections::VecDeque::from(lane_work.drain_closed_sidecar_prefixes());
    let had_prefixes = !prefixes.is_empty();
    while let Some(prefix) = prefixes.pop_front() {
        if let Err(error) = apply(&prefix) {
            lane_work.requeue_closed_sidecar_prefixes(std::iter::once(prefix).chain(prefixes));
            return Err(V2RunnerError::Service(error));
        }
    }
    if had_prefixes {
        lane_work.confirm_closed_sidecar_prefix_handoff();
    }
    Ok(())
}
/// Require the exact queue owner observed by the preceding guarded peek.
///
/// The runner holds exclusive access to the lane adapter, so an empty second
/// read cannot reflect a competing dequeue. It means the shared output guard
/// closed between the peek and drain permits; leave the original queued owner
/// untouched and surface the already-required process restart.
fn require_peeked_lane_work_effect(
    drained: Option<V2LaneWorkEffect>,
) -> Result<V2LaneWorkEffect, V2RunnerError> {
    drained.ok_or(V2RunnerError::RestartRequired)
}
include!("v2_runner/canonical_recovery_ingress.rs");
pub(in crate::sumeragi) fn dispatch_lane_work_effects(
    lane_work: &mut V2LaneWorkAdapter,
    services: &ProductionV2Services,
    limit: usize,
) -> Result<(), V2RunnerError> {
    dispatch_lane_work_effects_with_progress(lane_work, services, limit)?;
    Ok(())
}
fn dispatch_lane_work_effects_with_progress(
    lane_work: &mut V2LaneWorkAdapter,
    services: &ProductionV2Services,
    limit: usize,
) -> Result<usize, V2RunnerError> {
    let _ = apply_native_amx_output_retention(lane_work, services)?;
    let _ = apply_retired_historical_recovery_requests(lane_work, services)?;
    let _ = apply_retired_merge_sidecar_requests(lane_work, services)?;
    let _ = apply_acknowledged_merge_sidecar_closes(lane_work, services)?;
    apply_certified_merge_sidecar_closed_prefixes(lane_work, services)?;
    apply_certified_merge_sidecar_chunk_admissions(lane_work, services, limit)?;
    let scan_limit = lane_work.effect_count();
    let mut dispatched = 0usize;
    for _ in 0..scan_limit {
        if dispatched >= limit.max(1) {
            break;
        }
        let Some(mut next_effect) = lane_work.next_effect() else {
            break;
        };
        if !retain_active_owned_reply_routes(&mut next_effect) {
            let _ = require_peeked_lane_work_effect(lane_work.drain_effects(1).pop())?;
            continue;
        }
        if !services
            .can_retain_lane_work_effect(&next_effect)
            .map_err(V2RunnerError::Service)?
        {
            let effect = require_peeked_lane_work_effect(lane_work.drain_effects(1).pop())?;
            drop(effect);
            if next_effect.retries_from_native_catalog_after_source_retention() {
                // Catalog ownership survives a known-full worker just as it
                // survives an enqueue race below. Do not let this peer pin the
                // adapter's bounded delivery queue.
                continue;
            }
            if !lane_work.requeue_effect(next_effect) {
                return Err(V2RunnerError::Service(
                    "lane-work scheduler could not restore a reserved effect".to_owned(),
                ));
            }
            continue;
        }
        let effect = require_peeked_lane_work_effect(lane_work.drain_effects(1).pop())?;
        drop(effect);
        match dispatch_lane_work_effect(services, next_effect)? {
            LaneWorkEffectDispatch::Complete => {
                dispatched = dispatched.saturating_add(1);
            }
            LaneWorkEffectDispatch::SourceRetained(effect) => {
                if effect.retries_from_native_catalog_after_source_retention() {
                    // The compact body/peer catalog remains the source owner.
                    // Free this bounded delivery slot so the next cadence can
                    // rotate past a worker-saturated or silent peer.
                    continue;
                }
                if !lane_work.requeue_effect(effect) {
                    return Err(V2RunnerError::Service(
                        "lane-work scheduler could not retain a source-backpressured sidecar effect"
                            .to_owned(),
                    ));
                }
            }
        }
        apply_certified_merge_sidecar_chunk_admissions(lane_work, services, limit)?;
    }
    Ok(dispatched)
}
include!("v2_runner/reply_route_retention.rs");
#[derive(Debug)]
enum LaneWorkEffectDispatch {
    Complete,
    SourceRetained(V2LaneWorkEffect),
}
fn dispatch_lane_work_effect(
    services: &ProductionV2Services,
    effect: V2LaneWorkEffect,
) -> Result<LaneWorkEffectDispatch, V2RunnerError> {
    match effect {
        V2LaneWorkEffect::PostLaneBlock { peer, message } => services
            .post_lane_block(peer, message)
            .map_err(V2RunnerError::Service)?,
        V2LaneWorkEffect::PostDurableLaneCertificate {
            peer,
            reply_routes,
            ingress_ownership,
            certificate,
        } => {
            let reply_routes = reply_routes.ok_or_else(|| {
                V2RunnerError::Service(
                    "durable lane-certificate response lost its authenticated reply routes"
                        .to_owned(),
                )
            })?;
            let ingress_ownership = ingress_ownership.ok_or_else(|| {
                V2RunnerError::Service(
                    "durable lane-certificate response lost its fair-ingress ownership".to_owned(),
                )
            })?;
            if !ingress_ownership.validate_exact()
                || !ingress_ownership.matches_reply_routes(Some(&reply_routes))
            {
                return Err(V2RunnerError::Service(
                    "durable lane-certificate response carried altered ingress ownership"
                        .to_owned(),
                ));
            }
            services
                .post_durable_lane_certificate_on_reply_routes(
                    peer,
                    reply_routes,
                    ingress_ownership,
                    certificate,
                )
                .map_err(V2RunnerError::Service)?;
        }
        V2LaneWorkEffect::PostNativeAmx {
            peer,
            reply_routes,
            message,
        } => {
            services.post_native_amx_with_reply_routes(peer, reply_routes, message);
        }
        V2LaneWorkEffect::PostLaneDrainVote { peer, vote } => {
            services.post_lane_drain_vote(peer, vote);
        }
        V2LaneWorkEffect::BroadcastMerge(signature) => {
            services.broadcast_merge_to_voters(signature);
        }
        V2LaneWorkEffect::PostCertifiedMergeSidecar {
            peer,
            reply_routes,
            message,
        } => {
            let route_shape_is_valid = match message.as_ref() {
                CertifiedMergeSidecarMessage::Request(_)
                | CertifiedMergeSidecarMessage::Close(_) => reply_routes.is_none(),
                CertifiedMergeSidecarMessage::CloseAck(_)
                | CertifiedMergeSidecarMessage::GenerationHint(_)
                | CertifiedMergeSidecarMessage::Chunk(_) => reply_routes.is_some(),
            };
            if !route_shape_is_valid {
                return Err(V2RunnerError::Service(
                    "certified merge-sidecar effect lost its exact reply-route ownership"
                        .to_owned(),
                ));
            }
            let ownership = services
                .post_certified_merge_sidecar_with_reply_routes(
                    peer.clone(),
                    reply_routes.clone(),
                    Arc::clone(&message),
                )
                .map_err(V2RunnerError::Service)?;
            if ownership == ExactFanoutOwnership::SourceRetained {
                return Ok(LaneWorkEffectDispatch::SourceRetained(
                    V2LaneWorkEffect::PostCertifiedMergeSidecar {
                        peer,
                        reply_routes,
                        message,
                    },
                ));
            }
        }
    }
    Ok(LaneWorkEffectDispatch::Complete)
}
include!("v2_runner/merge_sidecar_recovery.rs");
fn drain_lane_relay_ingress(
    lane_relay_rx: &std::sync::mpsc::Receiver<super::LaneRelayMessage>,
    lane_work: &mut V2LaneWorkAdapter,
    active_view: wire::View,
    limit: usize,
) -> std::result::Result<(), V2LaneWorkError> {
    let mut drained_any = false;
    for _ in 0..limit.max(1) {
        let mut drained = false;
        if let Ok(message) = lane_relay_rx.try_recv() {
            let _ = lane_work.accept_relay_message(message, active_view);
            drained = true;
            drained_any = true;
        }
        if !drained {
            break;
        }
    }
    if drained_any {
        let _ = lane_work.service_next_historical_recovery()?;
    }
    Ok(())
}
/// Fail-closed live-runner error.
#[derive(Debug, Error)]
#[allow(variant_size_differences)]
pub(super) enum V2RunnerError {
    /// Active-height recovery failed.
    #[error(transparent)]
    Recovery(#[from] super::v2_recovery::V2RecoveryError),
    /// Runner/status activation ownership was inconsistent.
    #[error(transparent)]
    SuccessorActivation(#[from] super::status::V2SuccessorActivationError),
    /// Successor construction returned authority for another same-height predecessor.
    #[error(
        "Sumeragi v2 successor predecessor authority changed during construction: expected {expected:?}, actual {actual:?}"
    )]
    SuccessorPredecessorAuthorityMismatch {
        /// Exact predecessor identity which began the Running handoff.
        expected: DurableV2PredecessorIdentity,
        /// Exact predecessor identity returned by verified construction.
        actual: DurableV2PredecessorIdentity,
    },
    /// A typed successor lifecycle transition failed the shared pure refinement kernel.
    #[error("Sumeragi v2 successor lifecycle failed the production refinement kernel")]
    SuccessorRefinementRejected,
    /// Canonical CompleteTip lifecycle storage could not be retired exactly.
    #[error(transparent)]
    CompleteTipPredecessorStorage(#[from] CompleteTipPredecessorStorageErrorV1),
    /// CompleteTip retirement succeeded, but its retained canonical successor
    /// ledger or prepared status no longer matches the exact restart authority.
    #[error(
        "Sumeragi v2 CompleteTip predecessor {predecessor:?} no longer authenticates its canonical successor"
    )]
    CompleteTipSuccessorAuthorityInvalid {
        /// Exact retired durable predecessor whose successor was rejected.
        predecessor: DurableV2PredecessorIdentity,
    },
    /// The runner activation permit named another fair-ingress instance.
    #[error("launched lifecycle changed the runner-owned fair-ingress instance")]
    LifecycleActivationIngressMismatch,
    /// Closed-ingress setup of the launched lifecycle failed closed.
    #[error(transparent)]
    LifecyclePreActivation(
        #[from] super::v2_lifecycle_coordinator::ProductionLifecyclePreActivationErrorV1,
    ),
    /// The sealed interrupted-tip replay could not enter local Apply recovery.
    #[error(transparent)]
    PendingKuraLifecycleInstall(
        #[from] super::v2_lifecycle_coordinator::ProductionPendingKuraApplyInstallErrorV1,
    ),
    /// The sealed interrupted-tip local Apply recovery failed closed.
    #[error(transparent)]
    PendingKuraLifecycleRecovery(
        #[from] super::v2_lifecycle_coordinator::ProductionPendingKuraApplyRecoveryErrorV1,
    ),
    /// The recovered adapter/body/storage owners could not form one lifecycle owner.
    #[error(transparent)]
    LifecycleOwnerStartup(#[from] super::v2::ProductionLifecycleOwnerStartupErrorV1),
    /// The lifecycle owner could not transfer into its runtime/executor/service stack.
    #[error(transparent)]
    LifecycleLaunch(#[from] super::v2_lifecycle_coordinator::ProductionLifecycleLaunchErrorV1),
    /// The launched lifecycle could not publish its one-shot live-height boundary.
    #[error(transparent)]
    LifecycleActivation(
        #[from] super::v2_lifecycle_coordinator::ProductionLifecycleActivationErrorV1,
    ),
    /// The launched or active lifecycle could not retire for operator shutdown.
    #[error(transparent)]
    LifecycleShutdown(#[from] super::v2_lifecycle_coordinator::ProductionLifecycleShutdownErrorV1),
    /// The active lifecycle could not complete durable finalization and rollover.
    #[error(transparent)]
    LifecycleFinalization(
        #[from] super::v2_lifecycle_coordinator::ProductionLifecycleFinalizationErrorV1,
    ),
    /// Reducer/WAL adapter failed.
    #[error(transparent)]
    Adapter(#[from] super::v2::AdapterError),
    /// Runtime configuration failed.
    #[error("invalid Sumeragi v2 runtime configuration: {0}")]
    RuntimeConfig(#[from] super::v2_runtime::RuntimeConfigError),
    /// Live pacemaker clocks were activated outside the one-shot startup boundary.
    #[error(transparent)]
    RuntimeClock(#[from] super::v2_runtime::RuntimeClockError),
    /// Canonical shared consensus configuration was invalid.
    #[error(transparent)]
    SharedConfig(#[from] iroha_config::parameters::actual::SumeragiV2ConfigError),
    /// Effect boundary failed closed.
    #[error(transparent)]
    Effect(#[from] super::v2_effects::EffectExecutorError),
    /// Candidate construction failed.
    #[error(transparent)]
    CandidateBuild(#[from] super::v2_candidate::CandidateError),
    /// Bounded lane-local/merge/Native-AMX adapter failed closed.
    #[error(transparent)]
    LaneWork(#[from] super::v2_lane_work::V2LaneWorkError),
    /// Authenticated NPoS VRF lifecycle failed closed.
    #[error(transparent)]
    NposVrf(#[from] super::v2_npos::V2NposError),
    /// Durable lane reservation ownership could not be reconciled exactly.
    #[error(transparent)]
    Reservation(#[from] V2ReservationLifecycleError),
    /// Integer conversion failed.
    #[error(transparent)]
    Integer(#[from] std::num::TryFromIntError),
    /// Sequential CommitQC/body synchronization failed closed.
    #[error(transparent)]
    BlockSync(#[from] V2BlockSyncError),
    /// Production service failed.
    #[error("Sumeragi v2 production service failed: {0}")]
    Service(String),
    /// Fresh genesis leader no longer has the signed genesis body.
    #[error("Sumeragi v2 height one is missing its signed genesis body")]
    MissingGenesisBody,
    /// Interrupted-tip application did not reach its strict durable repair boundary.
    #[error(
        "Sumeragi v2 interrupted-tip recovery did not complete post-apply metadata and Native AMX evidence repair before lane-work construction"
    )]
    PendingTipRecoveryIncomplete,
    /// Closed-ingress interrupted-tip recovery exhausted its cadence-derived deadline.
    #[error(
        "Sumeragi v2 interrupted-tip recovery exceeded {timeout:?} after {attempts} serialized attempts at stage {stage:?}; process restart is required"
    )]
    PendingTipRecoveryDeadlineExceeded {
        /// Cadence-derived maximum local recovery duration.
        timeout: Duration,
        /// Number of serialized recovery scheduler attempts completed.
        attempts: u64,
        /// Exact authenticated recovery stage retained at expiry.
        stage: Option<PendingKuraApplyRecoveryStage>,
    },
    /// Durable parent body is unavailable in Kura.
    #[error("Sumeragi v2 successor is missing its canonical parent block")]
    MissingParent,
    /// Snapshot bootstrap context is not the exact successor of an unavailable Kura parent.
    #[error("Sumeragi v2 snapshot bootstrap parent geometry is invalid or unexpectedly has a body")]
    InvalidSnapshotBootstrapParent,
    /// Snapshot successor cadence is zero or not representable as whole wire milliseconds.
    #[error("Sumeragi v2 snapshot bootstrap cadence must be positive whole milliseconds")]
    InvalidSnapshotBootstrapCadence,
    /// Locked subject differs from loaded durable bytes.
    #[error("loaded Sumeragi v2 locked body differs from the reducer lock")]
    LockedBodyMismatch,
    /// A local or recovered proposal carried execution results.
    #[error("Sumeragi v2 proposal body must be resultless")]
    ResultBearingProposal,
    /// A locally assembled body could not bind its lane-local work to the exact round.
    #[error("local Sumeragi v2 candidate could not bind its lane-local ownership artifacts")]
    LaneCandidateBinding,
    /// Candidate tag belongs to another height.
    #[error("stale Sumeragi v2 proposal tag")]
    StaleTag,
    /// Runtime has already failed closed.
    #[error("Sumeragi v2 runtime is fail-closed")]
    RuntimeFailClosed,
    /// Single-owner runtime capacity changed between fair dequeue and enqueue.
    #[error("Sumeragi v2 atomic runtime admission invariant failed: {0}")]
    RuntimeAdmissionInvariant(String),
    /// A process-lifetime fatal guard was activated by another consensus service.
    #[error("Sumeragi v2 consensus requires process restart")]
    RestartRequired,
    /// A configured limit is zero.
    #[error("Sumeragi v2 configured limits must be positive")]
    InvalidLimits,
    /// The durable Native-AMX journal cannot cover every source/route Commit decision.
    #[error(
        "Sumeragi v2 Native AMX signing capacity {configured} is smaller than the {required} decisions required by max_transactions and the protocol lane bound"
    )]
    NativeAmxSigningCapacity {
        /// Configured durable signing-record capacity.
        configured: usize,
        /// Minimum capacity implied by configured transaction and protocol lane bounds.
        required: usize,
    },
    /// The fixed v2 ingress cannot reserve first-message and progress slots for the roster.
    #[error(
        "Sumeragi v2 body ingress capacity {configured} is smaller than the {required} first-message, progress, and untrusted slots required by the frozen roster"
    )]
    IngressCapacity {
        /// Configured fixed queue capacity.
        configured: usize,
        /// Required validator-lane plus untrusted-lane capacity.
        required: usize,
    },
    /// The fixed v2 ingress cannot isolate one wire-byte quota per active source lane.
    #[error(
        "Sumeragi v2 body ingress byte capacity {configured} is smaller than the {required} bytes required to isolate the frozen roster plus the untrusted lane"
    )]
    IngressByteCapacity {
        /// Configured aggregate canonical-wire byte capacity.
        configured: usize,
        /// Required per-source byte reservations for validators and untrusted traffic.
        required: usize,
    },
    /// Outstanding asynchronous work could overflow trusted completion admission.
    #[error(
        "Sumeragi v2 effect-work capacity {pending} exceeds runtime completion reserve {reserve}"
    )]
    EffectWorkExceedsCompletionReserve {
        /// Maximum outstanding asynchronous tasks.
        pending: usize,
        /// Runtime slots reserved for their trusted completions.
        reserve: usize,
    },
    /// The deterministic parent-plus-cadence timestamp exceeded wire range.
    #[error("Sumeragi v2 logical block timestamp exceeds u64 milliseconds")]
    V2BlockTimeOverflow,
    /// Deterministic local candidate operation failed.
    #[error("Sumeragi v2 candidate failed: {0}")]
    Candidate(String),
    /// A fresh, inbound, or recovered body carried no deterministic ledger work.
    #[error("Sumeragi v2 proposal carries no transaction, internal, or time-trigger work")]
    EmptyProposalWork,
    /// The one bounded non-empty recovery retry failed deterministic validation.
    #[error("Sumeragi v2 non-empty recovery retry failed validation: {0}")]
    LocalNonEmptyRetryRejected(String),
    /// The exact bounded discovery request vanished before reducer admission.
    #[error("Sumeragi v2 CommitQC discovery request disappeared before reducer admission")]
    BlockSyncRequestDisappeared,
}
#[cfg(test)]
#[path = "v2_runner_tests.rs"]
mod tests;
