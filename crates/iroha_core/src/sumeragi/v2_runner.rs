//! Serialized production height runner for the authoritative Sumeragi v2 reducer.
//!
//! This module owns exactly one reducer/effect executor at a time. It opens the
//! immutable context and safety WAL before processing network traffic, routes
//! authenticated control and body messages, schedules bounded proposal work,
//! and performs an explicit Kura-authorized rollover after application.

use std::{
    collections::BTreeSet,
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

use super::{
    FairV2Ingress, FairV2IngressBarrierBypass, FairV2IngressCapacityError,
    FairV2IngressDequeueDisposition, FairV2IngressOwnershipEvidence, GenesisWithPubKey,
    InboundBlockMessage, SumeragiWorker,
    message::BlockMessage,
    output_guard::{ConsensusOutputGuard, ConsensusOutputPermit},
    serviced_candidate_store::LeaderWireLifecycleStoreGate,
    v2::{
        AdapterEffect, AdapterFingerprints, DeferredAdmissionOrdinalSource, LocalProposalDirective,
        ServicedCandidateCapacityGeometry, SignRequest, SumeragiV2Adapter,
    },
    v2_apply::{V2ApplyService, V2ReservationLifecycleError, reconcile_lane_reservation_ownership},
    v2_block_sync::{
        CommitCertificateAdmissionError, V2BlockSyncDiscovery, V2BlockSyncError, V2BlockSyncServer,
    },
    v2_body_store::{BlockSignaturePolicy, V2BodyStore},
    v2_candidate::{
        CandidateAssemblyOutcome, CandidateAttachments, CandidateDescriptor, CandidateLimits,
        CandidateParent, CandidateRequest, CandidateWorkProvider, CandidateWorkUnavailable,
        PreparedCandidateWork, V2CandidateAssembler,
    },
    v2_chunks::{EncodedV2Payload, encode_payload},
    v2_effects::{
        EffectExecutorStep, EffectQueueConfig, EffectTransportError, PendingKuraApplyRecoveryStage,
        PostFinalityCleanupTarget, V2EffectExecutor, network_ingress_is_certified_fence_escape,
    },
    v2_lane_work::{
        AuthenticatedGenesisNexusAmxContext, GlobalBodyLockOutcome,
        HistoricalRecoveryServiceOutcome, MergeSidecarDeferralDisposition, RetainedMergeSidecars,
        V2LaneIngressOutcome, V2LaneWorkAdapter, V2LaneWorkEffect, V2LaneWorkError,
        V2LaneWorkLimits, require_validator_storage_platform,
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
        CertifiedServeAdmission, CertifiedServeIngressGate, CertifiedServeNegativeOutcome,
        CertifiedServePrepareError, ExactFanoutOwnership, ProductionV2Services,
        V2CleanupSupervisor, durable_exact_output_handoff_owner_pair,
    },
};
use crate::{
    kura::{Kura, KuraV2CommitReceipt},
    merge_sidecar::{
        CertifiedMergeSidecarClosedPrefix, CertifiedMergeSidecarMessage, MergeSidecarLimits,
        MergeSigningGuardLimits,
    },
    native_amx::NativeAmxMessage,
    queue::{GlobalQueueSelectionLease, Queue},
    state::State,
};

const IDLE_POLL: Duration = Duration::from_millis(10);
const CANDIDATE_WORK_RECHECK: Duration = Duration::from_millis(100);
const PENDING_TIP_RECOVERY_DEADLINE_ROUNDS: u32 = 3;

/// Cadence-derived process-local deadline for closed-ingress interrupted-tip recovery.
#[derive(Clone, Copy, Debug)]
struct PendingTipRecoveryDeadline {
    started_at: Instant,
    deadline: Instant,
    timeout: Duration,
}

impl PendingTipRecoveryDeadline {
    fn new(started_at: Instant, round_timeout: Duration) -> Result<Self, V2RunnerError> {
        let timeout = round_timeout
            .checked_mul(PENDING_TIP_RECOVERY_DEADLINE_ROUNDS)
            .ok_or(V2RunnerError::InvalidLimits)?;
        let deadline = started_at
            .checked_add(timeout)
            .ok_or(V2RunnerError::InvalidLimits)?;
        Ok(Self {
            started_at,
            deadline,
            timeout,
        })
    }

    fn expired(self, now: Instant) -> bool {
        now >= self.deadline
    }

    fn remaining(self, now: Instant) -> Duration {
        self.deadline.saturating_duration_since(now)
    }

    fn elapsed(self, now: Instant) -> Duration {
        now.saturating_duration_since(self.started_at)
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ReplayedProposalSign {
    tag: EventTag,
    round: wire::ConsensusRound,
    subject: wire::BlockSubject,
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

/// One-shot ownership of an authenticated successor's activation handoff.
///
/// Construction failure simply drops this token, leaving the predecessor's
/// `Running` work stage visible. The outer runner failure guard then closes
/// output and requires restart; only [`Self::publish`] can claim activation.
#[derive(Debug)]
enum PendingSuccessorActivation {
    /// Uninterrupted rollover whose published Applied predecessor owns the
    /// Running handoff.
    Applied {
        expected_predecessor: DurableV2PredecessorIdentity,
        authority: DurableSuccessorActivationAuthority,
    },
    /// Process restart after recovery authenticated an exact complete durable
    /// tip; the process-local predecessor registry was intentionally cleared.
    RecoveredCompleteTip {
        authority: DurableSuccessorActivationAuthority,
    },
    /// First executable height derived from an authenticated audited snapshot.
    /// This carries no historical CommitQC or Kura finality receipt.
    SnapshotBootstrap {
        authority: SnapshotSuccessorActivationAuthority,
    },
}

impl PendingSuccessorActivation {
    fn recovered(authority: RecoveredSuccessorActivationAuthority) -> Result<Self, V2RunnerError> {
        let (transition, authority_kind, status_height) = match &authority {
            RecoveredSuccessorActivationAuthority::CompleteTip(authority) => (
                SUCCESSOR_LIFECYCLE_RETRY_COMPLETE_TIP,
                SUCCESSOR_AUTHORITY_RECOVERED_COMPLETE_TIP,
                authority.predecessor().height(),
            ),
            RecoveredSuccessorActivationAuthority::SnapshotBootstrap(authority) => (
                SUCCESSOR_LIFECYCLE_SNAPSHOT_BOOTSTRAP,
                SUCCESSOR_AUTHORITY_SNAPSHOT_BOOTSTRAP,
                authority.snapshot_anchor_height(),
            ),
        };
        let published_height = super::status::v2_status().map_or(0, |status| status.height);
        let lifecycle = ProductionSuccessorStartupLifecycleProjection {
            transition_kind: transition,
            authority_kind,
            status_height,
            stage_before: SUCCESSOR_STAGE_NONE,
            stage_after: SUCCESSOR_STAGE_NONE,
            published_height_before: published_height,
            published_height_after: published_height,
            restart_required_before: false,
            restart_required_after: false,
        };
        let Some(checked_lifecycle) =
            check_production_successor_startup_lifecycle_transition(lifecycle)
        else {
            return Err(V2RunnerError::SuccessorRefinementRejected);
        };
        let _authorized_lifecycle = checked_lifecycle.into_projection();
        Ok(match authority {
            RecoveredSuccessorActivationAuthority::CompleteTip(authority) => {
                Self::RecoveredCompleteTip { authority }
            }
            RecoveredSuccessorActivationAuthority::SnapshotBootstrap(authority) => {
                Self::SnapshotBootstrap { authority }
            }
        })
    }

    fn publish(self, successor: wire::SumeragiV2Status) -> Result<(), V2RunnerError> {
        match self {
            Self::Applied {
                expected_predecessor,
                authority,
            } => {
                super::status::activate_v2_successor_height(
                    expected_predecessor,
                    authority,
                    successor,
                )?;
            }
            Self::RecoveredCompleteTip { authority } => {
                super::status::activate_recovered_v2_successor_height(authority, successor)?;
            }
            Self::SnapshotBootstrap { authority } => {
                super::status::activate_snapshot_bootstrap_v2_height(authority, successor)?;
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LocalValidationDisposition {
    Ignored,
    RetryHeartbeat,
    FatalHeartbeat,
}

#[derive(Default, Debug)]
struct LocalProposalState {
    attempted: Option<LocalProposalOwner>,
    submitted: Option<(LocalProposalOwner, wire::BlockSubject)>,
    heartbeat_only: Option<LocalProposalOwner>,
    candidate_work_wait: Option<CandidateWorkWait>,
    pending_events: Option<PendingLocalEvents>,
    global_selection: Option<PendingGlobalSelection>,
}

impl LocalProposalState {
    fn from_replayed_proposal(
        replayed: Option<ReplayedProposalSign>,
        current: LocalProposalDirective,
    ) -> Self {
        let owner = LocalProposalOwner::from(current);
        let replayed_owns_current = replayed.is_some_and(|replayed| {
            replayed.tag == owner.tag
                && replayed.round.height == replayed.tag.height()
                && replayed.round.view == replayed.tag.view()
                && owner.decided_subject.is_none()
                && owner
                    .locked_body
                    .is_none_or(|(locked_round, locked_subject)| {
                        replayed.round.context_id == locked_round.context_id
                            && replayed.round.height == locked_round.height
                            && replayed.subject == locked_subject
                    })
        });
        Self {
            attempted: replayed_owns_current.then_some(owner),
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
            .heartbeat_only
            .is_some_and(|candidate| candidate != owner)
        {
            self.heartbeat_only = None;
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
    /// window, then arm an explicit empty heartbeat for this exact owner.
    ///
    /// Expiry deliberately changes only proposal shape. It never changes the
    /// lane adapter's routing or queue-ownership policy.
    fn defer_candidate_work(
        &mut self,
        owner: LocalProposalOwner,
        now: Instant,
        wait_bound: Duration,
    ) {
        if self.heartbeat_only == Some(owner) {
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
        self.heartbeat_only = Some(owner);
        self.candidate_work_wait = None;
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
        if self.heartbeat_only == Some(owner) {
            return LocalValidationDisposition::FatalHeartbeat;
        }
        self.attempted = None;
        self.heartbeat_only = Some(owner);
        self.submitted = None;
        self.candidate_work_wait = None;
        LocalValidationDisposition::RetryHeartbeat
    }

    /// Abandon a candidate whose lane-local ownership could not be bound
    /// before the body was submitted, then give the exact owner one empty
    /// heartbeat attempt. The caller releases the candidate's selection lease
    /// before crossing this boundary, without publishing a body or events.
    fn handle_candidate_binding_rejection(
        &mut self,
        owner: LocalProposalOwner,
    ) -> LocalValidationDisposition {
        let owner = self.reconcile(owner);
        if self.heartbeat_only == Some(owner) {
            return LocalValidationDisposition::FatalHeartbeat;
        }
        self.attempted = None;
        self.heartbeat_only = Some(owner);
        self.candidate_work_wait = None;
        LocalValidationDisposition::RetryHeartbeat
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

struct CertifiedServeIngressBinding {
    ingress_ready: Arc<AtomicBool>,
    block_ingress: Arc<FairV2Ingress>,
    gate: Option<CertifiedServeIngressGate>,
}

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
struct LeaderWireIngressBinding {
    ingress_ready: Arc<AtomicBool>,
    block_ingress: Arc<FairV2Ingress>,
    gate: Option<Arc<LeaderWireLifecycleStoreGate>>,
}

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

/// Joint per-height ownership of the fair-ingress durable gates.
///
/// Once both gates are live they must retire in one queue transaction: the
/// Serve binding and leader-wire binding describe carriers in the same lanes.
struct HeightIngressBindings {
    certified_serve: CertifiedServeIngressBinding,
    leader_wire: LeaderWireIngressBinding,
}

impl HeightIngressBindings {
    fn new(
        certified_serve: CertifiedServeIngressBinding,
        leader_wire: LeaderWireIngressBinding,
    ) -> Self {
        Self {
            certified_serve,
            leader_wire,
        }
    }

    fn retire(&mut self) -> Result<(), V2RunnerError> {
        match (
            self.certified_serve.gate.as_ref(),
            self.leader_wire.gate.as_ref(),
        ) {
            (None, None) => return Ok(()),
            (Some(_), None) | (None, Some(_)) => {
                return Err(V2RunnerError::Service(
                    "per-height ingress gates changed joint ownership".to_owned(),
                ));
            }
            (Some(_), Some(_)) => {}
        }
        if !Arc::ptr_eq(
            &self.certified_serve.ingress_ready,
            &self.leader_wire.ingress_ready,
        ) || !Arc::ptr_eq(
            &self.certified_serve.block_ingress,
            &self.leader_wire.block_ingress,
        ) {
            return Err(V2RunnerError::Service(
                "per-height ingress gates changed their shared queue".to_owned(),
            ));
        }

        close_ingress_for_rollover(
            &self.certified_serve.ingress_ready,
            &self.certified_serve.block_ingress,
        );
        self.certified_serve
            .block_ingress
            .unbind_height_ingress_gates(
                self.certified_serve
                    .gate
                    .as_ref()
                    .expect("joint binding retains the certified Serve gate"),
                self.leader_wire
                    .gate
                    .as_ref()
                    .expect("joint binding retains the leader-wire gate"),
            )
            .map_err(V2RunnerError::Service)?;
        self.certified_serve.gate = None;
        self.leader_wire.gate = None;
        Ok(())
    }
}

impl Drop for HeightIngressBindings {
    fn drop(&mut self) {
        if let Err(error) = self.retire() {
            // Joint validation failed before mutation. Keep the shared queue
            // fail-closed and disarm the child guards: retrying their former
            // split teardown would recreate the carrierless-Ingress cut this
            // owner exists to prevent.
            close_ingress_for_rollover(
                &self.certified_serve.ingress_ready,
                &self.certified_serve.block_ingress,
            );
            self.certified_serve.gate = None;
            self.leader_wire.gate = None;
            iroha_logger::error!(
                %error,
                "failed to atomically retire the per-height ingress gates"
            );
        }
    }
}

struct V2StatusClearGuard {
    clear_on_drop: bool,
}

impl V2StatusClearGuard {
    fn new() -> Self {
        super::status::clear_v2_status();
        Self {
            clear_on_drop: false,
        }
    }

    fn clear_on_drop(&mut self) {
        self.clear_on_drop = true;
    }
}

impl Drop for V2StatusClearGuard {
    fn drop(&mut self) {
        if self.clear_on_drop {
            super::status::clear_v2_status();
        }
    }
}

fn close_ingress_for_rollover(ingress_ready: &AtomicBool, block_ingress: &FairV2Ingress) {
    ingress_ready.store(false, Ordering::Release);
    block_ingress.close();
}

fn open_ingress_for_active_height(
    output_guard: &ConsensusOutputGuard,
    ingress_ready: &AtomicBool,
    block_ingress: &FairV2Ingress,
    activation: Option<(PendingSuccessorActivation, wire::SumeragiV2Status)>,
) -> Result<(), V2RunnerError> {
    let Some(ingress_permit) = output_guard.acquire() else {
        return Err(V2RunnerError::RestartRequired);
    };
    block_ingress.open().map_err(ingress_capacity_error)?;
    ingress_ready.store(true, Ordering::Release);
    if let Some((activation, successor)) = activation
        && let Err(error) = activation.publish(successor)
    {
        close_ingress_for_rollover(ingress_ready, block_ingress);
        return Err(error);
    }
    drop(ingress_permit);
    Ok(())
}

fn ingress_capacity_error(error: FairV2IngressCapacityError) -> V2RunnerError {
    if error.is_bytes() {
        V2RunnerError::IngressByteCapacity {
            configured: error.configured(),
            required: error.required(),
        }
    } else {
        V2RunnerError::IngressCapacity {
            configured: error.configured(),
            required: error.required(),
        }
    }
}

fn validate_deadline_duration(duration: Duration) -> Result<(), V2RunnerError> {
    Instant::now()
        .checked_add(duration)
        .ok_or(V2RunnerError::InvalidLimits)?;
    Ok(())
}

fn deadline_after(now: Instant, duration: Duration) -> Instant {
    now.checked_add(duration)
        .expect("consensus deadline duration was prevalidated before height startup")
}

fn initial_block_sync_deadline(
    height_started_at: Instant,
    round_timeout: Duration,
    eager_recovery: bool,
) -> Instant {
    if eager_recovery {
        height_started_at
    } else {
        deadline_after(height_started_at, round_timeout)
    }
}

const fn retain_eager_block_sync(
    recovering_interrupted_tip: bool,
    admitted_discovered_commit_qc: bool,
) -> bool {
    recovering_interrupted_tip || admitted_discovered_commit_qc
}

fn snapshot_successor_logical_time(
    anchor: &wire::SnapshotBootstrapAnchor,
    block_cadence: Duration,
) -> Result<Duration, V2RunnerError> {
    let cadence_ms =
        u64::try_from(block_cadence.as_millis()).map_err(|_| V2RunnerError::V2BlockTimeOverflow)?;
    if cadence_ms == 0 || Duration::from_millis(cadence_ms) != block_cadence {
        return Err(V2RunnerError::InvalidSnapshotBootstrapCadence);
    }
    let successor_ms = anchor
        .snapshot_block_creation_time_ms
        .checked_add(cadence_ms)
        .ok_or(V2RunnerError::V2BlockTimeOverflow)?;
    Ok(Duration::from_millis(successor_ms))
}

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
    let mut pending_kura_apply = recovered.pending_kura_apply();
    let (
        mut verified_context,
        context_store,
        mut signature_policy,
        recovered_successor_activation,
        mut staged_genesis_nexus_amx_context,
    ) = recovered.into_parts();
    // A process which recovered durable v2 height ownership may be behind its
    // peers. Probe that exact active context immediately, then retain eager
    // discovery only while an authenticated discovered CommitQC acquires or
    // coalesces with serialized reducer ownership. Ordinary live finality
    // clears the hint, so this does not add permanent all-to-all traffic.
    let mut eager_block_sync =
        recovered_successor_activation.is_some() || pending_kura_apply.is_some();
    let reservation_recovery = output_guard
        .begin_fail_stop_operation()
        .ok_or(V2RunnerError::RestartRequired)?;
    let recovered_reservations = queue.live_lane_reservations().len();
    let (finalized_committed_reservations, released_orphans) =
        reconcile_lane_reservation_ownership(
            state.as_ref(),
            queue.as_ref(),
            kura.as_ref(),
            &verified_context.context().chain_id,
        )?;
    reservation_recovery.complete();
    if recovered_reservations > 0 || finalized_committed_reservations > 0 || released_orphans > 0 {
        iroha_logger::info!(
            recovered = recovered_reservations,
            finalized_committed = finalized_committed_reservations,
            released_orphans,
            "reconciled durable lane reservations against committed v2 merge history"
        );
    }
    let local_peer = common_config.peer.id().clone();
    let genesis_account = AccountId::new(genesis_public_key);
    let mut first_height_genesis = genesis_body;
    let mut block_sync_server = None;
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
    let mut cleanup_supervisor = V2CleanupSupervisor::default();
    let mut pending_successor_activation = recovered_successor_activation
        .map(PendingSuccessorActivation::recovered)
        .transpose()?;
    let mut liveness_watchdog = super::status::V2LivenessWatchdog::default();
    let deferred_admission_ordinals = DeferredAdmissionOrdinalSource::new(0);
    let mut retained_merge_sidecars: Option<RetainedMergeSidecars> = None;

    loop {
        cleanup_supervisor.reap_finished();
        if output_guard.restart_required() {
            return Err(V2RunnerError::RestartRequired);
        }
        if shutdown_signal.is_sent() {
            return Ok(());
        }
        let context = verified_context.context().clone();
        close_ingress_for_rollover(&ingress_ready, &block_rx);
        block_rx
            .configure_roster_for_context(
                context
                    .roster
                    .iter()
                    .map(|validator| validator.validator.clone()),
                &context.chain_id,
                context.da_layout,
            )
            .map_err(ingress_capacity_error)?;
        super::status::set_v2_network_ingress(context.id(), context.height, &block_rx);
        let validator_set_pops = verified_context.proofs_of_possession().to_vec();
        let shared_config = config.v2_config(block_cadence, context.mode)?;
        let fingerprints = adapter_fingerprints(&local_peer, &shared_config);
        let control_queue_capacity = usize::try_from(shared_config.limits.control_queue_capacity)?;
        let body_queue_capacity = usize::try_from(shared_config.limits.body_queue_capacity)?;
        let chunk_queue_capacity = usize::try_from(shared_config.limits.chunk_queue_capacity)?;
        let certified_request_capacity =
            usize::try_from(shared_config.limits.certified_request_capacity)?;
        let effect_work_capacity = usize::try_from(shared_config.limits.effect_work_capacity)?;
        validate_deadline_duration(CANDIDATE_WORK_RECHECK)?;
        let runtime_queue = runtime_queue_config(&shared_config)?;
        let effect_queue = effect_queue_config(&shared_config)?;
        let serviced_candidate_capacity_geometry = ServicedCandidateCapacityGeometry::new(
            usize::try_from(shared_config.limits.runtime_command_capacity)?,
            effect_work_capacity,
        );
        let lane_work_limits = lane_work_limits(
            &shared_config,
            network.reply_route_source_capacity(),
            consensus_frame_byte_capacity,
            block_sync_frame_byte_capacity,
        )?;
        let candidate_limits = candidate_limits(&context, &shared_config)?;
        let local_validator = local_validator_index(&context, &local_peer, config.role)?;
        let mut npos_vrf = V2NposVrfLifecycle::open(
            &context,
            state.as_ref(),
            local_validator,
            &common_config.key_pair,
        )?;
        let new_block_sync_server = block_sync_server
            .is_none()
            .then(|| V2BlockSyncServer::new(context.chain_id.clone(), certified_request_capacity))
            .transpose()?;
        let mut block_sync = V2BlockSyncDiscovery::new(
            context.clone(),
            local_peer.clone(),
            certified_request_capacity,
        )?;
        let consensus_key_hash: [u8; 32] =
            Hash::new(common_config.key_pair.public_key().encode()).into();
        let storage_root = kura.sumeragi_v2_storage_root();
        let wal_path = storage_root
            .join("wal")
            .join(format!("{:020}.wal", context.height));
        // Complete every pure or validation-only height preflight before any
        // WAL, body-store, chunk-store, or lane-work constructor can mutate
        // durable state. Publish the newly validated in-memory server only
        // after the full preflight succeeds.
        if let Some(server) = new_block_sync_server {
            block_sync_server = Some(server);
        }
        let lifecycle_ordinals = ProductionV2Services::restore_lifecycle_ordinal_source(
            &context,
            storage_root.join("chunks"),
            network.reply_route_source_capacity().max(1),
            certified_request_capacity,
        )
        .map_err(V2RunnerError::Service)?;
        // Open and validate the body store before the runtime can mint a new
        // scheduler ordinal. Its independently reconstructed receipt catalog
        // is the authority for body-backed leader-wire terminals; the gate's
        // adjacent snapshot cannot validate itself after a crash.
        let mut body_store = V2BodyStore::open_with_policy(
            storage_root.join("bodies"),
            context.clone(),
            signature_policy,
        )
        .map_err(|error| {
            V2RunnerError::Effect(super::v2_effects::EffectExecutorError::BodyStore(
                error.to_string(),
            ))
        })?;
        let recovery_validator = V2ApplyService::new(
            Arc::clone(&state),
            Arc::clone(&queue),
            Arc::clone(&kura),
            provider_ingest_finalized_archive.clone(),
            reputation_finalized_archive.clone(),
            context.chain_id.clone(),
            block_cadence,
            genesis_account.clone(),
            events_sender.clone(),
            validator_set_pops.clone(),
        );
        if let Some(decided_subject) = recovery_validator
            .recovered_finality_subject(&context)
            .map_err(|error| V2RunnerError::Service(error.to_string()))?
        {
            body_store
                .retain_recovered_markers_for_subject(decided_subject)
                .map_err(|error| {
                    V2RunnerError::Effect(super::v2_effects::EffectExecutorError::BodyStore(
                        error.to_string(),
                    ))
                })?;
        }
        let recovered_body_catalog = body_store.recovery_catalog().map_err(|error| {
            V2RunnerError::Effect(super::v2_effects::EffectExecutorError::BodyStore(
                error.to_string(),
            ))
        })?;
        let recovered_body_receipts = recovered_body_catalog
            .values()
            .map(|(_, receipt)| receipt.clone())
            .collect::<Vec<_>>();
        let adapter_construction = output_guard
            .begin_fail_stop_operation()
            .ok_or(V2RunnerError::RestartRequired)?;
        let adapter = if pending_successor_activation.is_some() {
            // Preserve the finalized predecessor's Running handoff until the
            // complete successor stack is live. No reducer status from this
            // adapter may escape the construction boundary early.
            SumeragiV2Adapter::open_deferred_status_with_capacity_geometry(
                wal_path.clone(),
                verified_context.clone(),
                local_validator,
                Generation::INITIAL,
                consensus_key_hash,
                fingerprints,
                serviced_candidate_capacity_geometry,
                deferred_admission_ordinals.clone(),
            )
        } else {
            SumeragiV2Adapter::open_with_capacity_geometry(
                wal_path.clone(),
                verified_context.clone(),
                local_validator,
                Generation::INITIAL,
                consensus_key_hash,
                fingerprints,
                serviced_candidate_capacity_geometry,
                deferred_admission_ordinals.clone(),
            )
        };
        let (adapter, startup_effects) = adapter?;
        // Adapter replay returns plain effects; their fresh lifecycle owners
        // are minted only by `SerializedV2Runtime` below. Fold the validated
        // producer snapshot into the shared source before that constructor so
        // neither startup/runtime work nor a later Serve reservation can reuse
        // a reclaimed producer ordinal.
        if let Some(high_watermark) =
            adapter.restored_producer_continuation_ordinal_high_watermark()
        {
            lifecycle_ordinals
                .advance_past(high_watermark)
                .map_err(V2RunnerError::Service)?;
        }
        let leader_wire_roster = context
            .roster
            .iter()
            .map(|entry| entry.validator.clone())
            .collect::<BTreeSet<_>>();
        let leader_wire_max_chunk_count = context.da_layout.max_chunk_count;
        let leader_wire_capacity = LeaderWireLifecycleStoreGate::derived_capacity(
            leader_wire_roster.len(),
            leader_wire_max_chunk_count,
        )
        .map_err(V2RunnerError::Service)?;
        let leader_wire_owner: [u8; 32] = fingerprints.node.into();
        let leader_wire_recovery_authority = adapter.leader_wire_recovery_authority()?;
        let producer_terminals = adapter.durable_producer_terminal_tokens();
        let (leader_wire_gate, leader_wire_restore) = LeaderWireLifecycleStoreGate::open(
            &wal_path,
            context.id(),
            context.height,
            leader_wire_owner,
            leader_wire_roster,
            leader_wire_capacity,
            leader_wire_max_chunk_count,
            leader_wire_recovery_authority,
            &producer_terminals,
            &recovered_body_receipts,
        )
        .map_err(V2RunnerError::Service)?;
        lifecycle_ordinals
            .advance_past(leader_wire_restore.scheduler_ordinal_high_watermark())
            .map_err(V2RunnerError::Service)?;
        let leader_wire_ingress_binding = LeaderWireIngressBinding::bind(
            Arc::clone(&ingress_ready),
            Arc::clone(&block_rx),
            Arc::clone(&leader_wire_gate),
            leader_wire_restore,
            lifecycle_ordinals.clone(),
            context.id(),
            context.height,
        )?;
        // The body directory may retain markers from arbitrarily many past
        // views. WAL replay is the sole authority for deciding which bounded
        // frontier can recover vote authority; re-execute only those exact
        // identities before constructing the live serialized runtime.
        let recovered_validation_authority =
            adapter.recovered_validation_authority(&startup_effects)?;
        body_store
            .retain_recovered_markers_for_authority(recovered_validation_authority)
            .map_err(|error| {
                V2RunnerError::Effect(super::v2_effects::EffectExecutorError::BodyStore(
                    error.to_string(),
                ))
            })?;
        body_store
            .revalidate_recovered_markers(|body| {
                recovery_validator.revalidate_recovered_candidate(&context, body)
            })
            .map_err(|error| {
                V2RunnerError::Effect(super::v2_effects::EffectExecutorError::BodyStore(
                    error.to_string(),
                ))
            })?;
        adapter_construction.complete();
        let runtime_construction = output_guard
            .begin_fail_stop_operation()
            .ok_or(V2RunnerError::RestartRequired)?;
        let (runtime, mut startup_effects) = SerializedV2Runtime::new_with_lifecycle_ordinals(
            adapter,
            startup_effects,
            Instant::now(),
            round_timeout,
            runtime_queue,
            lifecycle_ordinals.clone(),
        )?;
        runtime_construction.complete();
        let (mut executor, body_store) = V2EffectExecutor::open_with_body_store(
            runtime,
            body_store,
            context.clone(),
            local_peer.clone(),
            local_validator,
            Arc::clone(&output_guard),
            effect_queue,
        )?;
        if let Some(authenticated_genesis) = first_height_genesis.as_ref() {
            executor.install_authenticated_genesis_body(authenticated_genesis)?;
        }
        // A replayed ProposalIntent owns candidate work only when its exact
        // tag, round, and subject still match the current lock snapshot. Its
        // asynchronous signature completion must restore and broadcast the
        // exact durable payload before any fresh candidate work is admitted.
        let replayed_proposal = replayed_proposal_sign(&startup_effects);
        let recovering_interrupted_tip = pending_kura_apply.is_some();
        let pending_recovery_identity = pending_kura_apply;
        let initially_recovered_applied_height = pending_kura_apply.filter(|pending| {
            usize::try_from(pending.height()).is_ok_and(|height| state.committed_height() == height)
        });
        let mut authenticated_genesis_nexus_amx_context =
            staged_genesis_nexus_amx_context.map(AuthenticatedGenesisNexusAmxContext::Staged);
        if let Some(pending) = pending_kura_apply.take() {
            let pending_replay_verification = output_guard
                .begin_fail_stop_operation()
                .ok_or(V2RunnerError::RestartRequired)?;
            let replayed_genesis_nexus_amx_context =
                executor.verify_pending_kura_apply_replay(pending, &startup_effects)?;
            pending_replay_verification.complete();
            if initially_recovered_applied_height.is_none()
                && let Some(replayed) = replayed_genesis_nexus_amx_context
                && authenticated_genesis_nexus_amx_context
                    .replace(AuthenticatedGenesisNexusAmxContext::ReplayedPending(
                        replayed,
                    ))
                    .is_some()
            {
                return Err(V2RunnerError::ConflictingGenesisNexusContext);
            }
        }
        let (exact_output_service_owner, exact_output_transport_owner) =
            durable_exact_output_handoff_owner_pair();
        let durable_decided_subject = executor.local_proposal_directive()?.decided_subject();
        let mut services = ProductionV2Services::start(
            context.clone(),
            executor.current_tag(),
            durable_decided_subject,
            validator_set_pops,
            local_peer.clone(),
            local_validator,
            common_config.key_pair.clone(),
            network.clone(),
            storage_root.join("chunks"),
            body_store,
            Arc::clone(&state),
            Arc::clone(&queue),
            Arc::clone(&kura),
            provider_ingest_finalized_archive.clone(),
            reputation_finalized_archive.clone(),
            block_cadence,
            genesis_account.clone(),
            events_sender.clone(),
            effect_work_capacity,
            certified_request_capacity,
            chunk_queue_capacity,
            lifecycle_ordinals,
            Arc::clone(&output_guard),
            Arc::clone(&block_rx),
            leader_wire_recovery_authority,
            exact_output_service_owner,
        )
        .map_err(V2RunnerError::Service)?;
        let certified_serve_ingress_binding = CertifiedServeIngressBinding::bind(
            Arc::clone(&ingress_ready),
            Arc::clone(&block_rx),
            services
                .certified_serve_ingress_gate()
                .map_err(V2RunnerError::Service)?,
        )?;
        let mut height_ingress_bindings = HeightIngressBindings::new(
            certified_serve_ingress_binding,
            leader_wire_ingress_binding,
        );

        // A Native receipt at the durable tip may have crossed its
        // finality/manifest/receipt boundary before WSV checkpoint and commit
        // metadata were published. Finish the exact local replay first.
        // `DurableApplyCompletion` is emitted only after post-apply metadata
        // and strict Native evidence repair both succeed, so no lane-work
        // constructor can observe or reject the recoverable intermediate
        // shape.
        if recovering_interrupted_tip {
            let recovery_deadline = PendingTipRecoveryDeadline::new(Instant::now(), round_timeout)?;
            iroha_logger::info!(
                height = context.height,
                timeout = ?recovery_deadline.timeout,
                stage = ?executor
                    .pending_kura_apply_recovery_evidence()
                    .map(|evidence| evidence.stage()),
                "started bounded Sumeragi v2 interrupted-tip recovery"
            );
            let _ = reconcile_executor_locked_body(&mut executor, &mut services)?;
            executor.consume_pending_tip_recovery_effects(
                std::mem::take(&mut startup_effects),
                &mut services,
            )?;
            while executor.durable_finality().is_none() {
                if output_guard.restart_required() {
                    return Err(V2RunnerError::RestartRequired);
                }
                if shutdown_signal.is_sent() {
                    height_ingress_bindings.retire()?;
                    services.allow_clean_shutdown();
                    return Ok(());
                }
                let now = Instant::now();
                if recovery_deadline.expired(now) {
                    executor.record_pending_tip_recovery_deadline_exceeded(&mut services)?;
                    let error = pending_tip_recovery_deadline_error(
                        output_guard.as_ref(),
                        recovery_deadline.timeout,
                        executor.pending_tip_recovery_attempts(),
                        executor
                            .pending_kura_apply_recovery_evidence()
                            .map(|evidence| evidence.stage()),
                    );
                    return Err(error);
                }
                let completions = services.drain_completions(&mut executor)?;
                let advanced = advance_pending_tip_recovery_executor(
                    &mut executor,
                    &mut services,
                    control_queue_capacity,
                )?;
                if executor.durable_finality().is_none() && completions == 0 && advanced == 0 {
                    let remaining = recovery_deadline.remaining(Instant::now());
                    if !remaining.is_zero() {
                        let _ = wake_rx.recv_timeout(remaining.min(IDLE_POLL));
                    }
                }
            }
            iroha_logger::info!(
                height = context.height,
                elapsed = ?recovery_deadline.elapsed(Instant::now()),
                attempts = executor.pending_tip_recovery_attempts(),
                stage = ?executor
                    .pending_kura_apply_recovery_evidence()
                    .map(|evidence| evidence.stage()),
                "finished bounded Sumeragi v2 interrupted-tip recovery"
            );
        }
        let recovered_applied_height = pending_recovery_identity.filter(|pending| {
            usize::try_from(pending.height()).is_ok_and(|height| {
                state.committed_height() == height
                    && state.latest_block_hash_fast() == Some(pending.block_hash())
            })
        });
        if recovering_interrupted_tip {
            // A replayed genesis projection is a pre-apply capability. The
            // completed State tip is now authoritative and the lane adapter
            // must enter through its exact post-apply recovery path.
            authenticated_genesis_nexus_amx_context = None;
        }
        let mut lane_work = construct_after_pending_tip_application_recovery(
            recovering_interrupted_tip,
            executor.durable_finality().is_some() && recovered_applied_height.is_some(),
            || {
                V2LaneWorkAdapter::new_with_output_guard_and_transport(
                    &verified_context,
                    local_peer.clone(),
                    common_config.key_pair.clone(),
                    config.role == NodeRole::Validator,
                    Arc::clone(&state),
                    Arc::clone(&kura),
                    lane_work_limits,
                    authenticated_genesis_nexus_amx_context,
                    recovered_applied_height,
                    Arc::clone(&output_guard),
                    exact_output_transport_owner,
                    retained_merge_sidecars.take(),
                )
                .map_err(V2RunnerError::from)
            },
        )?;
        lane_work.install_lane_drain_queue(Arc::clone(&queue))?;
        let mut committed_lane_status_publisher = CommittedLaneStatusPublisher::default();
        committed_lane_status_publisher.publish_if_changed(&lane_work);
        if let Some(scheduler_ordinal) = services
            .dormant_certified_serve_ingress_scheduler_ordinal()
            .map_err(V2RunnerError::Service)?
        {
            let _ = services.fail_closed_dormant_certified_serve(scheduler_ordinal);
            return Err(V2RunnerError::RestartRequired);
        }
        // Seed executor lock ownership from replay before consuming startup
        // effects. Otherwise a recovered lock would look like a live first-lock
        // transition and could retire safe work reconstructed from the same WAL.
        let _ = reconcile_executor_locked_body(&mut executor, &mut services)?;
        if !recovering_interrupted_tip {
            executor.consume_effects(std::mem::take(&mut startup_effects), &mut services)?;
        }
        let startup_directive = executor.local_proposal_directive()?;
        // Adapter construction is deliberately carrier-silent. Only the exact
        // reducer/WAL recovery directive may unlock candidate signing or the
        // decided carrier's bounded lane-completion traffic.
        lane_work.retain_merge_sidecars_for_global_view(
            startup_directive.tag().view(),
            startup_directive.locked_subject(),
            startup_directive.decided_subject(),
        )?;
        if startup_directive.decided_subject().is_none()
            && let Some((locked_round, locked)) = startup_directive.locked_body()
        {
            let _ = lane_work.mark_global_body_locked(locked_round, locked)?;
        }
        dispatch_lane_work_effects(&mut lane_work, &services, control_queue_capacity)?;
        // Startup recovery and durable constructor work must not consume the
        // live height cadence. Constructor repair already emitted the first
        // bounded Native recovery request for every marker it could own; any
        // pressure-deferred marker, or retransmission to the next authenticated
        // signer, waits for `next_lane_retransmit`. Interrupted-tip replay
        // remains permanently unarmed because the already-decided runtime is
        // consumed as soon as its local Apply finishes; the fresh successor is
        // armed normally.
        // These additions are infallible after the early representability
        // probes above.
        let height_started_at = Instant::now();
        if !recovering_interrupted_tip {
            executor.arm_live_clocks(height_started_at)?;
        }
        let mut next_block_sync_attempt =
            initial_block_sync_deadline(height_started_at, round_timeout, eager_block_sync);
        let mut next_lane_retransmit = deadline_after(height_started_at, retransmit_interval);
        let mut next_npos_vrf_retransmit = deadline_after(height_started_at, retransmit_interval);
        let initial_directive = reconcile_executor_locked_body(&mut executor, &mut services)?;
        let mut local_proposal_state =
            LocalProposalState::from_replayed_proposal(replayed_proposal, initial_directive);
        debug_assert!(!recovering_interrupted_tip || pending_successor_activation.is_none());
        let activation = pending_successor_activation
            .take()
            .map(|pending| {
                executor
                    .successor_activation_status_snapshot()
                    .map(|status| (pending, status))
            })
            .transpose()?;
        // Interrupted-tip recovery admits transport so validators can finish
        // only the exact replayed Decision's lane session. Its dedicated drain
        // below discards terminal global traffic instead of re-entering it into
        // the already-decided reducer.
        open_ingress_for_active_height(
            output_guard.as_ref(),
            &ingress_ready,
            &block_rx,
            activation,
        )?;
        if !recovering_interrupted_tip {
            broadcast_npos_vrf_messages(
                npos_vrf.take_outbound(),
                output_guard.as_ref(),
                &services,
            )?;
        }

        let mut block_sync_request = None;
        let mut admitted_discovered_commit_qc = false;

        let finality = loop {
            committed_lane_status_publisher.publish_if_changed(&lane_work);
            cleanup_supervisor.reap_finished();
            if output_guard.restart_required() {
                return Err(V2RunnerError::RestartRequired);
            }
            if shutdown_signal.is_sent() {
                height_ingress_bindings.retire()?;
                services.allow_clean_shutdown();
                return Ok(());
            }
            // Every retry/continue path returns through this edge-triggered
            // poll. It rebuilds the live overlays only at its next semantic
            // deadline or after the published height owner changes.
            liveness_watchdog.poll(Instant::now());
            if let Some(scheduler_ordinal) = services
                .dormant_certified_serve_ingress_scheduler_ordinal()
                .map_err(V2RunnerError::Service)?
            {
                let _ = services.fail_closed_dormant_certified_serve(scheduler_ordinal);
                return Err(V2RunnerError::RestartRequired);
            }
            let certified_serve_barrier = services
                .certified_serve_barrier()
                .map_err(V2RunnerError::Service)?;
            let now = Instant::now();
            if !recovering_interrupted_tip && now >= next_npos_vrf_retransmit {
                broadcast_npos_vrf_messages(
                    npos_vrf.retransmission(),
                    output_guard.as_ref(),
                    &services,
                )?;
                next_npos_vrf_retransmit = deadline_after(now, retransmit_interval);
            }

            if executor.has_retained_certified_body_response() {
                let target_ordinal = executor
                    .retained_certified_body_response_scheduler_ordinal()?
                    .ok_or_else(|| {
                        V2RunnerError::Service(
                            "retained certified body response lost its exact scheduler position"
                                .to_owned(),
                        )
                    })?;
                // Give one strict frozen predecessor first claim on newly
                // available completion capacity. Otherwise the response could
                // occupy the sole returned slot and strand that older owner.
                services.drain_exact_serve_runtime_predecessor(&mut executor, target_ordinal)?;
                if executor
                    .older_runtime_lifecycle_predates_exact_serve(Instant::now(), target_ordinal)?
                {
                    // The matching PendingFetch is itself an older passive
                    // owner, so this is one bounded opportunity rather than a
                    // prerequisite for retry.
                    advance_executor_once_before_exact_serve(
                        &block_rx,
                        &mut executor,
                        &mut services,
                    )?;
                }
                if let Some(timeout_recovery_cut) = executor.timeout_recovery_lifecycle_cut()? {
                    // The retained response may predate the timeout itself,
                    // so its strict `< target` completion turn cannot admit
                    // the timeout signer's callback. Give exactly one owner
                    // from the separately frozen inclusive `<= timeout` prefix
                    // a turn before retrying the response.
                    services.drain_timeout_recovery_prefix_completion(
                        &mut executor,
                        timeout_recovery_cut,
                    )?;
                }
                let response_backpressured = match executor
                    .retry_retained_certified_body_response(&mut services)
                {
                    Ok(_) => false,
                    Err(EffectTransportError::Backpressure) => true,
                    Err(EffectTransportError::FailClosed(reason)) => {
                        return Err(V2RunnerError::Service(reason));
                    }
                    Err(error) => {
                        iroha_logger::debug!(%error, "rejected retained certified body response");
                        false
                    }
                };
                if response_backpressured {
                    // Transport capacity is not pacemaker authority. This
                    // retained response owns one bounded opportunity to admit
                    // a new authenticated certificate into the typed Progress
                    // lane. Once charged, later retries service the finite
                    // retained certificate prefix but cannot replenish it
                    // from fresh ingress and recreate the same capacity cycle.
                    if executor.retained_response_may_admit_certified_fence_escape() {
                        drain_v2_ingress(
                            &block_rx,
                            &mut executor,
                            &mut services,
                            &mut lane_work,
                            output_guard.as_ref(),
                            kura.as_ref(),
                            &common_config.key_pair,
                            block_sync_server
                                .as_mut()
                                .expect("block-sync server initialized before ingress"),
                            &mut block_sync,
                            &mut block_sync_request,
                            &mut npos_vrf,
                            V2IngressDrainMode::CertifiedFenceEscape,
                            1,
                        )?;
                    }
                    // Certificate escape is a one-shot retained-response
                    // credit. TimeoutVote production is a distinct frozen
                    // roster episode: service one eligible source on every
                    // backpressured outer turn so reaching quorum never
                    // depends on that certificate credit remaining unused.
                    drain_v2_ingress(
                        &block_rx,
                        &mut executor,
                        &mut services,
                        &mut lane_work,
                        output_guard.as_ref(),
                        kura.as_ref(),
                        &common_config.key_pair,
                        block_sync_server
                            .as_mut()
                            .expect("block-sync server initialized before ingress"),
                        &mut block_sync,
                        &mut block_sync_request,
                        &mut npos_vrf,
                        V2IngressDrainMode::TimeoutVoteEpisode,
                        1,
                    )?;
                    executor.reconcile_retained_response_certified_fence_escape_phase();
                    // Give one already-owned timeout/Progress root a turn
                    // before retrying the exact response. Ordinary work stays
                    // parked behind the retained physical carrier.
                    advance_pacemaker_once(&block_rx, &mut executor, &mut services)?;
                    executor.reconcile_retained_response_certified_fence_escape_phase();
                }
                committed_lane_status_publisher.publish_if_changed(&lane_work);
                let _ = wake_rx.recv_timeout(IDLE_POLL);
                continue;
            }

            // The network thread installs an exact certified-body ticket before
            // its carrier becomes visible in fair ingress. Give that target a
            // dedicated runner turn before completions, runtime work, lock
            // reconciliation, or any other local producer can acquire a later
            // I/O position.
            if let Some(serve_barrier) = certified_serve_barrier {
                // One exact ticket closes its finite older-owner prefix through
                // bounded turns. Each turn atomically selects at most one
                // completed causal lifecycle whose immutable ordinal is
                // strictly older than the ticket, publishes/freezes resulting
                // runtime ownership, and executes at most one serialized
                // transition. The ticket reaches target-only ingress only once
                // re-evaluation finds no older owner. Later producers cannot
                // enter the frozen prefix, and exact carrier retry retains the
                // same episode state and ticket identity.
                let mut older_predecessor_remains = false;
                let claimed_older_runtime_episode = services
                    .claim_certified_serve_runtime_episode(serve_barrier)
                    .map_err(V2RunnerError::Service)?;
                if claimed_older_runtime_episode {
                    services.drain_exact_serve_runtime_predecessor(
                        &mut executor,
                        serve_barrier.scheduler_ordinal(),
                    )?;
                    // A frozen Control/Serve prefix may still own every
                    // physical I/O unit. Yield this turn until one unit drains
                    // instead of dispatching a retained causal effect into
                    // backpressure. Once capacity exists, the queue admits
                    // only this turn's strict older lifecycle and keeps the
                    // exact target barrier installed.
                    if executor.older_runtime_lifecycle_predates_exact_serve(
                        Instant::now(),
                        serve_barrier.scheduler_ordinal(),
                    )? && services
                        .certified_serve_runtime_predecessor_capacity_available(serve_barrier)
                        .map_err(V2RunnerError::Service)?
                    {
                        if recovering_interrupted_tip {
                            let _ = advance_pending_tip_recovery_executor(
                                &mut executor,
                                &mut services,
                                1,
                            )?;
                        } else {
                            advance_executor_once_before_exact_serve(
                                &block_rx,
                                &mut executor,
                                &mut services,
                            )?;
                        }
                    }
                    if !recovering_interrupted_tip {
                        // A backpressured Serve ticket may retain the sole I/O
                        // unit indefinitely. It cannot suppress authenticated
                        // certified progress admission or an already-admitted
                        // certificate root.
                        drain_v2_ingress(
                            &block_rx,
                            &mut executor,
                            &mut services,
                            &mut lane_work,
                            output_guard.as_ref(),
                            kura.as_ref(),
                            &common_config.key_pair,
                            block_sync_server
                                .as_mut()
                                .expect("block-sync server initialized before ingress"),
                            &mut block_sync,
                            &mut block_sync_request,
                            &mut npos_vrf,
                            V2IngressDrainMode::CertifiedFenceEscape,
                            1,
                        )?;
                    }
                    older_predecessor_remains = executor
                        .older_runtime_lifecycle_predates_exact_serve(
                            Instant::now(),
                            serve_barrier.scheduler_ordinal(),
                        )?;
                    services
                        .finish_certified_serve_runtime_episode_turn(
                            serve_barrier,
                            older_predecessor_remains,
                        )
                        .map_err(V2RunnerError::Service)?;
                }
                if !recovering_interrupted_tip {
                    // The one-shot older-runtime claim above cannot own the
                    // whole timeout-recovery episode. Admit one authenticated
                    // direct-roster TimeoutVote on every selected-Serve outer
                    // turn, including after that claim reaches Complete.
                    drain_v2_ingress(
                        &block_rx,
                        &mut executor,
                        &mut services,
                        &mut lane_work,
                        output_guard.as_ref(),
                        kura.as_ref(),
                        &common_config.key_pair,
                        block_sync_server
                            .as_mut()
                            .expect("block-sync server initialized before ingress"),
                        &mut block_sync,
                        &mut block_sync_request,
                        &mut npos_vrf,
                        V2IngressDrainMode::TimeoutVoteEpisode,
                        1,
                    )?;
                }
                if let Some(timeout_recovery_cut) = executor.timeout_recovery_lifecycle_cut()? {
                    services.drain_timeout_recovery_prefix_completion(
                        &mut executor,
                        timeout_recovery_cut,
                    )?;
                }
                service_certified_serve_barrier_pacemaker_turn(
                    recovering_interrupted_tip,
                    claimed_older_runtime_episode,
                    || advance_pacemaker_once(&block_rx, &mut executor, &mut services),
                )?;
                if !older_predecessor_remains {
                    // A full prefix may include an earlier Serve lifecycle whose
                    // auxiliary unit is released only after its sealed response is
                    // posted and acknowledged. Service at most one I/O completion
                    // from the prefix frozen by this target; the barrier prevents
                    // later I/O replenishment and the source-only turn excludes
                    // unrelated local completions.
                    services.drain_certified_serve_predecessor_completion(&mut executor)?;
                    if recovering_interrupted_tip {
                        drain_decided_lane_recovery_ingress(
                            &block_rx,
                            &executor,
                            &mut services,
                            &mut lane_work,
                            executor.current_tag().view(),
                            output_guard.as_ref(),
                            kura.as_ref(),
                            &common_config.key_pair,
                            block_sync_server
                                .as_mut()
                                .expect("block-sync server initialized before ingress"),
                        )?;
                    } else {
                        drain_v2_ingress(
                            &block_rx,
                            &mut executor,
                            &mut services,
                            &mut lane_work,
                            output_guard.as_ref(),
                            kura.as_ref(),
                            &common_config.key_pair,
                            block_sync_server
                                .as_mut()
                                .expect("block-sync server initialized before ingress"),
                            &mut block_sync,
                            &mut block_sync_request,
                            &mut npos_vrf,
                            V2IngressDrainMode::Ordinary,
                            1,
                        )?;
                    }
                }
                // Popping an ordinary frozen predecessor materializes the
                // barrier inside the worker queue lock. A Serve predecessor
                // retains its unit through completion posting, so the dedicated
                // source-only turn above acknowledges exactly that finite
                // prefix before another target turn.
                committed_lane_status_publisher.publish_if_changed(&lane_work);
                let _ = wake_rx.recv_timeout(IDLE_POLL);
                continue;
            }
            let Some(_certified_serve_producer_episode) = services
                .try_begin_certified_serve_producer_episode()
                .map_err(V2RunnerError::Service)?
            else {
                // Exact admission won the queue-locked race after the
                // observation above. Restart at the dedicated target turn.
                let _ = wake_rx.recv_timeout(IDLE_POLL);
                continue;
            };

            debug_assert!(startup_effects.is_empty());

            // Retry actor-owned output first, but keep servicing bounded
            // reducer and completion sources while one target is unavailable.
            // Each producer either transfers its complete fanout into the
            // corridor or retains the durable/reconstructible semantic source.
            let _ = retry_exact_output_and_apply_sidecar_admissions(
                &mut lane_work,
                &services,
                control_queue_capacity,
            )?;
            services.drain_completions(&mut executor)?;
            let _ = retry_exact_output_and_apply_sidecar_admissions(
                &mut lane_work,
                &services,
                control_queue_capacity,
            )?;
            if !recovering_interrupted_tip {
                let directive = reconcile_executor_locked_body(&mut executor, &mut services)?;
                local_proposal_state.reconcile(LocalProposalOwner::from(directive));
                lane_work.retain_merge_sidecars_for_global_view(
                    directive.tag().view(),
                    directive.locked_subject(),
                    directive.decided_subject(),
                )?;
                drive_merge_sidecar_recovery(&mut executor, &mut services, &mut lane_work)?;
                services
                    .replay_buffered_chunks(&mut executor)
                    .map_err(V2RunnerError::Service)?;
                while let Some(rejection) = services.take_validation_rejection() {
                    let current = executor.local_proposal_directive()?;
                    let expected_round = round_for_tag(&context, current.tag())?;
                    match local_proposal_state.handle_validation_rejection(
                        LocalProposalOwner::from(current),
                        expected_round,
                        rejection.round(),
                        rejection.subject(),
                    ) {
                        LocalValidationDisposition::Ignored => {}
                        LocalValidationDisposition::FatalHeartbeat => {
                            return Err(V2RunnerError::LocalHeartbeatRejected(
                                rejection.reason().to_owned(),
                            ));
                        }
                        LocalValidationDisposition::RetryHeartbeat => {
                            iroha_logger::warn!(
                                reason = rejection.reason(),
                                "local Sumeragi v2 candidate rejected; retrying an empty heartbeat"
                            );
                        }
                    }
                }

                let terminal_decision = directive.decided_subject().is_some();
                if !terminal_decision {
                    drive_block_sync(
                        Instant::now(),
                        &mut next_block_sync_attempt,
                        retransmit_interval,
                        &mut block_sync_request,
                        &mut block_sync,
                        &common_config.key_pair,
                        output_guard.as_ref(),
                        &services,
                    )?;
                }
                let discovery_was_outstanding = block_sync_request.is_some();
                drain_v2_ingress(
                    &block_rx,
                    &mut executor,
                    &mut services,
                    &mut lane_work,
                    output_guard.as_ref(),
                    kura.as_ref(),
                    &common_config.key_pair,
                    block_sync_server
                        .as_mut()
                        .expect("block-sync server initialized before ingress"),
                    &mut block_sync,
                    &mut block_sync_request,
                    &mut npos_vrf,
                    V2IngressDrainMode::Ordinary,
                    body_queue_capacity,
                )?;
                if discovery_was_outstanding && block_sync_request.is_none() {
                    // `drain_v2_ingress` retires this sole request only after
                    // the authenticated response's CommitQC is admitted to or
                    // coalesces with serialized reducer ownership. Preserve
                    // exactly that catch-up witness for the successor deadline.
                    admitted_discovered_commit_qc = true;
                }
                let _ = retry_exact_output_and_apply_sidecar_admissions(
                    &mut lane_work,
                    &services,
                    control_queue_capacity,
                )?;
                let directive = reconcile_executor_locked_body(&mut executor, &mut services)?;
                local_proposal_state.reconcile(LocalProposalOwner::from(directive));
                lane_work.retain_merge_sidecars_for_global_view(
                    directive.tag().view(),
                    directive.locked_subject(),
                    directive.decided_subject(),
                )?;
                drain_lane_relay_ingress(
                    &lane_relay_rx,
                    &mut lane_work,
                    executor.current_tag().view(),
                    control_queue_capacity,
                )?;
                drive_merge_sidecar_recovery(&mut executor, &mut services, &mut lane_work)?;
                let now = Instant::now();
                if now >= next_lane_retransmit {
                    let _ = service_historical_recovery_tick(&mut lane_work)?;
                    lane_work.service_next_native_participant_recovery_request()?;
                    lane_work.schedule_autonomous_new_view_timeouts(
                        now,
                        executor.current_tag().view(),
                        round_timeout,
                    )?;
                    lane_work.schedule_retransmission()?;
                    next_lane_retransmit = deadline_after(now, retransmit_interval);
                }
                dispatch_lane_work_effects(&mut lane_work, &services, control_queue_capacity)?;
                let _ = retry_exact_output_and_apply_sidecar_admissions(
                    &mut lane_work,
                    &services,
                    control_queue_capacity,
                )?;
            } else {
                let directive = reconcile_executor_locked_body(&mut executor, &mut services)?;
                lane_work.retain_merge_sidecars_for_global_view(
                    directive.tag().view(),
                    directive.locked_subject(),
                    directive.decided_subject(),
                )?;
                drain_decided_lane_recovery_ingress(
                    &block_rx,
                    &executor,
                    &mut services,
                    &mut lane_work,
                    executor.current_tag().view(),
                    output_guard.as_ref(),
                    kura.as_ref(),
                    &common_config.key_pair,
                    block_sync_server
                        .as_mut()
                        .expect("block-sync server initialized before ingress"),
                )?;
                drain_lane_relay_ingress(
                    &lane_relay_rx,
                    &mut lane_work,
                    executor.current_tag().view(),
                    control_queue_capacity,
                )?;
                drive_merge_sidecar_recovery(&mut executor, &mut services, &mut lane_work)?;
                let now = Instant::now();
                if now >= next_lane_retransmit {
                    let _ = service_historical_recovery_tick(&mut lane_work)?;
                    lane_work.service_next_native_participant_recovery_request()?;
                    lane_work.schedule_autonomous_new_view_timeouts(
                        now,
                        executor.current_tag().view(),
                        round_timeout,
                    )?;
                    lane_work.schedule_retransmission()?;
                    next_lane_retransmit = deadline_after(now, retransmit_interval);
                }
                dispatch_lane_work_effects(&mut lane_work, &services, control_queue_capacity)?;
            }

            if recovering_interrupted_tip {
                advance_pending_tip_recovery_executor(
                    &mut executor,
                    &mut services,
                    control_queue_capacity,
                )?;
            } else {
                advance_executor(
                    &block_rx,
                    &mut executor,
                    &mut services,
                    control_queue_capacity,
                )?;
                let _ = retry_exact_output_and_apply_sidecar_admissions(
                    &mut lane_work,
                    &services,
                    control_queue_capacity,
                )?;
                let directive = reconcile_executor_locked_body(&mut executor, &mut services)?;
                local_proposal_state.reconcile(LocalProposalOwner::from(directive));
                lane_work.retain_merge_sidecars_for_global_view(
                    directive.tag().view(),
                    directive.locked_subject(),
                    directive.decided_subject(),
                )?;
                if directive.decided_subject().is_none()
                    && let Some((locked_round, locked)) = directive.locked_body()
                {
                    let lock_outcome = lane_work.mark_global_body_locked(locked_round, locked)?;
                    if lock_outcome == GlobalBodyLockOutcome::Inserted && local_validator.is_some()
                    {
                        services
                            .request_locked_candidate(executor.current_tag(), locked_round, locked)
                            .map_err(V2RunnerError::Service)?;
                    }
                }
                while let Some(prepared) = services.take_prepared_candidate() {
                    let current = executor.local_proposal_directive()?;
                    if let Some(events) = local_proposal_state.take_prepared_events(
                        LocalProposalOwner::from(current),
                        prepared.tag(),
                        prepared.subject(),
                    ) {
                        let Some(_permit) = output_guard.acquire() else {
                            return Err(V2RunnerError::RestartRequired);
                        };
                        for event in events {
                            let _ = events_sender.send(EventBox::Pipeline(event));
                        }
                    }
                }
                services
                    .replay_buffered_chunks(&mut executor)
                    .map_err(V2RunnerError::Service)?;
            }

            committed_lane_status_publisher.publish_if_changed(&lane_work);
            if executor.ready_to_finish() {
                let (durable_receipt, durable_artifact) = executor
                    .durable_finality()
                    .map(|(receipt, artifact)| (receipt.clone(), artifact.clone()))
                    .ok_or_else(|| {
                        V2RunnerError::Service(
                            "ready Sumeragi v2 executor has no durable finality authority"
                                .to_owned(),
                        )
                    })?;
                height_ingress_bindings.retire()?;
                let (runtime, receipt, artifact) = executor.into_finalized_parts()?;
                let wal_retirement = output_guard
                    .begin_fail_stop_operation()
                    .ok_or(V2RunnerError::RestartRequired)?;
                let finalized = runtime.into_driver().finish_height(&receipt, &artifact)?;
                wal_retirement.complete();
                if let Some(warning) = finalized.wal_retirement_warning() {
                    iroha_logger::warn!(
                        height = receipt.height(),
                        context_id = ?receipt.context_id(),
                        block_hash = %receipt.block_hash(),
                        cleanup_target = PostFinalityCleanupTarget::SafetyWal.as_str(),
                        reason = warning,
                        "Sumeragi v2 finalized with retained local cleanup state"
                    );
                }
                committed_lane_status_publisher.publish_if_changed(&lane_work);
                debug_assert_eq!(durable_receipt.height(), receipt.height());
                debug_assert_eq!(durable_receipt.context_id(), receipt.context_id());
                debug_assert_eq!(durable_receipt.block_hash(), receipt.block_hash());
                debug_assert_eq!(durable_receipt.subject(), receipt.subject());
                debug_assert_eq!(durable_receipt.certificate(), receipt.certificate());
                debug_assert_eq!(durable_receipt.artifact_hash(), receipt.artifact_hash());
                debug_assert_eq!(durable_artifact, artifact);
                break (receipt, artifact, lane_work, services);
            }

            if recovering_interrupted_tip {
                // Global body/application recovery is local to the durable
                // Decision. Exact decided-lane votes or QCs may still wake
                // progress until their certificate and receipt are durable;
                // the recovery-specific executor rejects every
                // network-producing global reducer effect.
                committed_lane_status_publisher.publish_if_changed(&lane_work);
                let _ = wake_rx.recv_timeout(IDLE_POLL);
                continue;
            }

            schedule_local_proposal(
                candidate_limits,
                &context,
                local_validator,
                &common_config.key_pair,
                output_guard.as_ref(),
                state.as_ref(),
                &queue,
                kura.as_ref(),
                first_height_genesis.as_ref(),
                height_started_at,
                block_cadence,
                &mut local_proposal_state,
                &mut executor,
                &mut services,
                &mut lane_work,
                &npos_vrf,
                retransmit_interval,
            )?;
            dispatch_lane_work_effects(&mut lane_work, &services, control_queue_capacity)?;

            committed_lane_status_publisher.publish_if_changed(&lane_work);
            let _ = wake_rx.recv_timeout(IDLE_POLL);
        };

        let (receipt, artifact, lane_work, mut finalized_services) = finality;
        eager_block_sync =
            retain_eager_block_sync(recovering_interrupted_tip, admitted_discovered_commit_qc);
        let predecessor = DurableV2PredecessorIdentity::authenticate(&artifact, &receipt)?;
        let artifact_hash = HashOf::new(&artifact);
        let terminal_application =
            ProductionTerminalApplicationWithoutSuccessorActivationProjection {
                context_id: successor_context_refinement_projection(context.id()),
                context_height: context.height,
                receipt_context_id: successor_context_refinement_projection(receipt.context_id()),
                receipt_height: receipt.height(),
                receipt_block_hash: successor_block_refinement_projection(receipt.block_hash()),
                receipt_artifact_hash: CanonicalIdentityProjection::from_bytes(
                    IDENTITY_DOMAIN_DURABLE_ARTIFACT,
                    IDENTITY_KIND_FINALITY_ARTIFACT,
                    *receipt.artifact_hash().as_ref(),
                ),
                artifact_context_id: successor_context_refinement_projection(artifact.context_id()),
                artifact_height: artifact.height,
                artifact_block_hash: successor_block_refinement_projection(artifact.block_hash),
                artifact_hash: CanonicalIdentityProjection::from_bytes(
                    IDENTITY_DOMAIN_DURABLE_ARTIFACT,
                    IDENTITY_KIND_FINALITY_ARTIFACT,
                    *artifact_hash.as_ref(),
                ),
                predecessor: predecessor.refinement_projection(),
                pending_successor_activation_present: pending_successor_activation.is_some(),
            };
        let Some(checked_application) =
            check_production_terminal_application_transition(terminal_application)
        else {
            return Err(V2RunnerError::SuccessorRefinementRejected);
        };
        let _authorized_application = checked_application.into_projection();
        let activation = PendingSuccessorConstruction::begin(predecessor)?;
        let successor_construction = output_guard
            .begin_fail_stop_operation()
            .ok_or(V2RunnerError::RestartRequired)?;
        let successor =
            build_verified_successor(state.as_ref(), &context_store, &artifact, &receipt)?;
        successor_construction.complete();
        let (next_verified_context, successor_authority) = successor.into_parts();
        let next_context = next_verified_context.context().clone();
        retained_merge_sidecars = Some(rollover_finalized_height_outputs(
            lane_work,
            &finalized_services,
            &receipt,
            &artifact,
            &next_context,
            control_queue_capacity,
        )?);
        finalized_services.allow_clean_shutdown();
        let cleanup = finalized_services.finish_height(
            receipt.clone(),
            Duration::ZERO,
            &mut cleanup_supervisor,
        );
        for warning in cleanup.warnings() {
            iroha_logger::warn!(
                height = receipt.height(),
                context_id = ?receipt.context_id(),
                block_hash = %receipt.block_hash(),
                cleanup_target = warning.target().as_str(),
                reason = warning.reason(),
                "Sumeragi v2 finalized with retained local cleanup state"
            );
        }
        pending_successor_activation = Some(activation.bind(successor_authority)?);
        verified_context = next_verified_context;
        signature_policy = BlockSignaturePolicy::RotatingLeader;
        first_height_genesis = None;
        staged_genesis_nexus_amx_context = None;
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

fn replayed_proposal_sign(effects: &[AdapterEffect]) -> Option<ReplayedProposalSign> {
    effects.iter().find_map(|effect| match effect {
        AdapterEffect::Sign {
            tag,
            request: SignRequest::Proposal(proposal),
        } => Some(ReplayedProposalSign {
            tag: *tag,
            round: proposal.round,
            subject: proposal.subject,
        }),
        AdapterEffect::Sign { .. }
        | AdapterEffect::Broadcast(_)
        | AdapterEffect::FetchBody { .. }
        | AdapterEffect::StoreBody { .. }
        | AdapterEffect::ValidateBody { .. }
        | AdapterEffect::Apply { .. }
        | AdapterEffect::EnterView { .. }
        | AdapterEffect::ReportEquivocation { .. }
        | AdapterEffect::ReportInvalidCertifiedBody { .. } => None,
    })
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
        let assembly = if proposal_state.heartbeat_only == Some(owner) {
            assembler.assemble(CandidateRequest {
                context,
                directive,
                local_validator,
                parent,
                state,
                queue,
                key_pair,
                output_guard,
                attachments,
                allow_empty_recovery_heartbeat: true,
                work_provider: HeartbeatOnlyWorkProvider,
            })?
        } else {
            assembler.assemble(CandidateRequest {
                context,
                directive,
                local_validator,
                parent,
                state,
                queue,
                key_pair,
                output_guard,
                attachments,
                allow_empty_recovery_heartbeat: false,
                work_provider: &mut *lane_work,
            })?
        };
        let candidate = match assembly {
            CandidateAssemblyOutcome::Assembled(candidate) => candidate,
            CandidateAssemblyOutcome::NoProposalWork(report) => {
                let now = Instant::now();
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
        if proposal_state.heartbeat_only != Some(owner)
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
                LocalValidationDisposition::RetryHeartbeat => {
                    iroha_logger::warn!(
                        height = tag.height(),
                        view = tag.view(),
                        "discarded an unsubmitted candidate after lane-local ownership binding rejected; retrying as an empty heartbeat"
                    );
                    return Ok(());
                }
                LocalValidationDisposition::FatalHeartbeat
                | LocalValidationDisposition::Ignored => {
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

enum PreparedCertifiedServe {
    Admitted(CertifiedServeAdmission),
    Rejected(String),
    Service(String),
}

enum DecidedLaneRecoveryCurrentServe {
    Authenticated {
        authenticated_via: PeerId,
        request: AuthenticatedCertifiedBodyRequest,
    },
    Negative {
        request_hash: HashOf<wire::CertifiedBodyRequest>,
        outcome: CertifiedServeNegativeOutcome,
        reason: String,
    },
    Service(String),
}

enum DecidedLaneRecoveryIngressPreparation {
    LaneLocal,
    CurrentServe(DecidedLaneRecoveryCurrentServe),
    HistoricalServe,
    LeaderWireRetire,
}

enum DecidedLaneRecoveryCurrentDrain<Admission> {
    Admitted(Admission),
    Rejected(String),
}

enum DecidedLaneRecoveryDrainAuthorization<Admission> {
    LaneLocal,
    CurrentServe(DecidedLaneRecoveryCurrentDrain<Admission>),
    HistoricalServe,
    LeaderWireRetire,
}

enum DecidedLaneRecoveryDrainDecision<Admission> {
    Retain,
    Authorized(DecidedLaneRecoveryDrainAuthorization<Admission>),
    FailClosed(String),
}

trait DecidedLaneRecoveryDrainAuthorizer {
    type Admission;

    fn stage_negative(
        &mut self,
        request_hash: HashOf<wire::CertifiedBodyRequest>,
        outcome: CertifiedServeNegativeOutcome,
    ) -> Result<(), String>;

    fn prepare_exact(
        &mut self,
        authenticated_via: &PeerId,
        request: AuthenticatedCertifiedBodyRequest,
    ) -> Result<Self::Admission, CertifiedServePrepareError>;
}

impl DecidedLaneRecoveryDrainAuthorizer for ProductionV2Services {
    type Admission = CertifiedServeAdmission;

    fn stage_negative(
        &mut self,
        request_hash: HashOf<wire::CertifiedBodyRequest>,
        outcome: CertifiedServeNegativeOutcome,
    ) -> Result<(), String> {
        self.stage_certified_serve_rejection(request_hash, outcome)
    }

    fn prepare_exact(
        &mut self,
        authenticated_via: &PeerId,
        request: AuthenticatedCertifiedBodyRequest,
    ) -> Result<Self::Admission, CertifiedServePrepareError> {
        self.prepare_certified_request(authenticated_via, request)
    }
}

fn authorize_decided_lane_recovery_drain<A: DecidedLaneRecoveryDrainAuthorizer>(
    preparation: DecidedLaneRecoveryIngressPreparation,
    authorizer: &mut A,
) -> DecidedLaneRecoveryDrainDecision<A::Admission> {
    match preparation {
        DecidedLaneRecoveryIngressPreparation::LaneLocal => {
            DecidedLaneRecoveryDrainDecision::Authorized(
                DecidedLaneRecoveryDrainAuthorization::LaneLocal,
            )
        }
        DecidedLaneRecoveryIngressPreparation::HistoricalServe => {
            DecidedLaneRecoveryDrainDecision::Authorized(
                DecidedLaneRecoveryDrainAuthorization::HistoricalServe,
            )
        }
        DecidedLaneRecoveryIngressPreparation::LeaderWireRetire => {
            DecidedLaneRecoveryDrainDecision::Authorized(
                DecidedLaneRecoveryDrainAuthorization::LeaderWireRetire,
            )
        }
        DecidedLaneRecoveryIngressPreparation::CurrentServe(
            DecidedLaneRecoveryCurrentServe::Negative {
                request_hash,
                outcome,
                reason,
            },
        ) => match authorizer.stage_negative(request_hash, outcome) {
            Ok(()) => DecidedLaneRecoveryDrainDecision::Authorized(
                DecidedLaneRecoveryDrainAuthorization::CurrentServe(
                    DecidedLaneRecoveryCurrentDrain::Rejected(reason),
                ),
            ),
            Err(error) => DecidedLaneRecoveryDrainDecision::FailClosed(error),
        },
        DecidedLaneRecoveryIngressPreparation::CurrentServe(
            DecidedLaneRecoveryCurrentServe::Service(reason),
        ) => DecidedLaneRecoveryDrainDecision::FailClosed(reason),
        DecidedLaneRecoveryIngressPreparation::CurrentServe(
            DecidedLaneRecoveryCurrentServe::Authenticated {
                authenticated_via,
                request,
            },
        ) => match authorizer.prepare_exact(&authenticated_via, request) {
            Ok(admission) => DecidedLaneRecoveryDrainDecision::Authorized(
                DecidedLaneRecoveryDrainAuthorization::CurrentServe(
                    DecidedLaneRecoveryCurrentDrain::Admitted(admission),
                ),
            ),
            Err(CertifiedServePrepareError::Backpressure) => {
                DecidedLaneRecoveryDrainDecision::Retain
            }
            Err(CertifiedServePrepareError::Rejected(reason)) => {
                DecidedLaneRecoveryDrainDecision::Authorized(
                    DecidedLaneRecoveryDrainAuthorization::CurrentServe(
                        DecidedLaneRecoveryCurrentDrain::Rejected(reason),
                    ),
                )
            }
            Err(CertifiedServePrepareError::Service(reason)) => {
                DecidedLaneRecoveryDrainDecision::FailClosed(reason)
            }
        },
    }
}

fn prepare_decided_lane_recovery_ingress(
    inbound: &InboundBlockMessage,
    active_height: wire::Height,
    decided_subject: wire::BlockSubject,
    authenticate: impl FnOnce(
        wire::CertifiedBodyRequest,
        &PeerId,
    ) -> Result<AuthenticatedCertifiedBodyRequest, String>,
) -> DecidedLaneRecoveryIngressPreparation {
    if inbound.message().is_lane_local() {
        return DecidedLaneRecoveryIngressPreparation::LaneLocal;
    }
    let BlockMessage::V2(message) = inbound.message() else {
        return DecidedLaneRecoveryIngressPreparation::LeaderWireRetire;
    };
    let wire::ConsensusMessageV2Payload::CertifiedBodyRequest(request) = &message.payload else {
        return DecidedLaneRecoveryIngressPreparation::LeaderWireRetire;
    };
    if message.validate_version().is_err() {
        return DecidedLaneRecoveryIngressPreparation::CurrentServe(
            DecidedLaneRecoveryCurrentServe::Service(
                "terminal recovery Serve ingress crossed version validation".to_owned(),
            ),
        );
    }
    if request.round.height < active_height {
        return DecidedLaneRecoveryIngressPreparation::HistoricalServe;
    }
    if request.round.height > active_height {
        return DecidedLaneRecoveryIngressPreparation::LeaderWireRetire;
    }
    let Some(sender) = inbound.sender() else {
        return DecidedLaneRecoveryIngressPreparation::CurrentServe(
            DecidedLaneRecoveryCurrentServe::Service(
                "terminal recovery Serve ingress lost its authenticated sender".to_owned(),
            ),
        );
    };
    let Some(authenticated_via) = inbound.via() else {
        return DecidedLaneRecoveryIngressPreparation::CurrentServe(
            DecidedLaneRecoveryCurrentServe::Service(
                "terminal recovery Serve ingress lost its authenticated source".to_owned(),
            ),
        );
    };
    let Some(reply_routes) = inbound.reply_routes() else {
        return DecidedLaneRecoveryIngressPreparation::CurrentServe(
            DecidedLaneRecoveryCurrentServe::Service(
                "terminal recovery Serve ingress lost its reply capability".to_owned(),
            ),
        );
    };
    let Some(ingress_ownership) = inbound.ingress_ownership() else {
        return DecidedLaneRecoveryIngressPreparation::CurrentServe(
            DecidedLaneRecoveryCurrentServe::Service(
                "terminal recovery Serve ingress lost its ownership evidence".to_owned(),
            ),
        );
    };
    if reply_routes.semantic_target() != sender
        || !ingress_ownership.validate_exact()
        || !ingress_ownership.matches_message(inbound.message())
        || !ingress_ownership.matches_semantic_origin(Some(sender))
        || !ingress_ownership.matches_reply_routes(Some(reply_routes))
    {
        return DecidedLaneRecoveryIngressPreparation::CurrentServe(
            DecidedLaneRecoveryCurrentServe::Service(
                "terminal recovery Serve ingress changed its transport ownership".to_owned(),
            ),
        );
    }
    let authenticated = match authenticate(request.clone(), sender) {
        Ok(authenticated) => authenticated,
        Err(reason) => {
            return DecidedLaneRecoveryIngressPreparation::CurrentServe(
                DecidedLaneRecoveryCurrentServe::Negative {
                    request_hash: HashOf::new(request),
                    outcome: CertifiedServeNegativeOutcome::InvalidCertificate,
                    reason,
                },
            );
        }
    };
    if request.subject != decided_subject {
        return DecidedLaneRecoveryIngressPreparation::CurrentServe(
            DecidedLaneRecoveryCurrentServe::Negative {
                request_hash: authenticated.request_hash(),
                outcome: CertifiedServeNegativeOutcome::SupersededByDurableDecision(
                    decided_subject,
                ),
                reason: "terminal recovery Serve request was superseded by durable Decision"
                    .to_owned(),
            },
        );
    }
    DecidedLaneRecoveryIngressPreparation::CurrentServe(
        DecidedLaneRecoveryCurrentServe::Authenticated {
            authenticated_via: authenticated_via.clone(),
            request: authenticated,
        },
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum V2IngressDrainMode {
    /// Normal completion/runtime/ingress round-robin.
    Ordinary,
    /// Only a TC/CommitQC which can supersede a hung signing fence.
    CertifiedFenceEscape,
    /// Only one member of the finite current-view TimeoutVote producer episode.
    /// This remains available after a retained response spends its separate
    /// certificate credit.
    TimeoutVoteEpisode,
}

fn drain_v2_ingress(
    receiver: &FairV2Ingress,
    executor: &mut V2EffectExecutor,
    services: &mut ProductionV2Services,
    lane_work: &mut V2LaneWorkAdapter,
    output_guard: &ConsensusOutputGuard,
    kura: &Kura,
    local_key: &KeyPair,
    block_sync_server: &mut V2BlockSyncServer,
    block_sync: &mut V2BlockSyncDiscovery,
    block_sync_request: &mut Option<HashOf<wire::CommitCertificateRequest>>,
    npos_vrf: &mut V2NposVrfLifecycle,
    mode: V2IngressDrainMode,
    limit: usize,
) -> Result<(), V2RunnerError> {
    if mode == V2IngressDrainMode::Ordinary && executor.has_retained_certified_body_response() {
        // The dedicated outer episode owns all progress until this exact
        // transport completion either crosses capacity or reaches a permanent
        // terminal. Do not give even the Runtime half-turn of a new batch to a
        // later owner.
        return Ok(());
    }
    for turn in outer_ingress_turns(limit) {
        if mode != V2IngressDrainMode::Ordinary && turn != OuterIngressTurn::Ingress {
            continue;
        }
        if turn == OuterIngressTurn::Completion {
            if services
                .certified_serve_barrier_request_hash()
                .map_err(V2RunnerError::Service)?
                .is_some()
            {
                // A provisional or prepared exact target owns this turn. The
                // outer runner services it before any queued completion.
                continue;
            }
            // I/O completion is a separate producer from the serialized
            // reducer. Service it before every ingress occurrence so a
            // completed durable store cannot remain hidden for the duration
            // of a large authenticated ingress batch.
            services.drain_completions(executor)?;
            continue;
        }
        if turn == OuterIngressTurn::Runtime {
            if services
                .certified_serve_barrier()
                .map_err(V2RunnerError::Service)?
                .is_some()
            {
                // A provisional or prepared exact target owns this turn. The
                // outer runner services it before any queued runtime producer.
                continue;
            }
            // A whole authenticated ingress batch can be expensive. Give the
            // serialized runtime one service turn after completions and before
            // every outer occurrence so trusted timers and reducer work cannot
            // remain hidden behind that batch.
            let was_terminal = executor
                .local_proposal_directive()?
                .decided_subject()
                .is_some();
            advance_executor(receiver, executor, services, 1)?;
            let is_terminal = executor
                .local_proposal_directive()?
                .decided_subject()
                .is_some();
            if !was_terminal && is_terminal {
                // Publish the new terminal carrier to lane work before any
                // further ingress occurrence can be admitted. In particular,
                // do not use a pre-batch snapshot to enqueue another global
                // reducer event after this runtime turn installed Decision.
                return Ok(());
            }
            continue;
        }
        let terminal_subject = executor.local_proposal_directive()?.decided_subject();
        let terminal_decision = terminal_subject.is_some();
        let mut prepared_serve = None;
        let barrier_bypass = match mode {
            V2IngressDrainMode::TimeoutVoteEpisode => {
                FairV2IngressBarrierBypass::TimeoutVoteEpisode
            }
            V2IngressDrainMode::Ordinary | V2IngressDrainMode::CertifiedFenceEscape => {
                FairV2IngressBarrierBypass::None
            }
        };
        let Some((mut inbound, dequeue_disposition)) = receiver
            .try_recv_if_checked_retiring_obsolete_with_barrier_bypass(barrier_bypass, |inbound| {
                if mode != V2IngressDrainMode::Ordinary {
                    let BlockMessage::V2(message) = inbound.message() else {
                        return false;
                    };
                    if message.validate_version().is_err() {
                        return false;
                    }
                    let selected_mode_matches = match mode {
                        V2IngressDrainMode::Ordinary => true,
                        V2IngressDrainMode::CertifiedFenceEscape => {
                            network_ingress_is_certified_fence_escape(&message.payload)
                        }
                        V2IngressDrainMode::TimeoutVoteEpisode => {
                            inbound.ingress_ownership().is_some_and(|ownership| {
                                executor.can_admit_timeout_vote_recovery_episode(message, ownership)
                            })
                        }
                    };
                    if !selected_mode_matches {
                        return false;
                    }
                }
                if !v2_ingress_head_can_drain(inbound, executor, terminal_subject) {
                    return false;
                }
                let BlockMessage::V2(message) = inbound.message() else {
                    return true;
                };
                if message.validate_version().is_err() {
                    prepared_serve = Some(PreparedCertifiedServe::Service(
                        "reserved certified-body ingress crossed version validation".to_owned(),
                    ));
                    return true;
                }
                let wire::ConsensusMessageV2Payload::CertifiedBodyRequest(request) =
                    &message.payload
                else {
                    return true;
                };
                if request.round.height != executor.context().height {
                    return true;
                }
                let superseded_by_decision = certified_body_request_is_superseded_after_decision(
                    request,
                    terminal_subject,
                    executor.context().height,
                );
                let Some(sender) = inbound.sender() else {
                    prepared_serve = Some(PreparedCertifiedServe::Service(
                        "reserved certified-body ingress lost its authenticated sender".to_owned(),
                    ));
                    return true;
                };
                let Some(authenticated_via) = inbound.via() else {
                    prepared_serve = Some(PreparedCertifiedServe::Service(
                        "reserved certified-body ingress lost its authenticated source".to_owned(),
                    ));
                    return true;
                };
                let Some(reply_routes) = inbound.reply_routes() else {
                    prepared_serve = Some(PreparedCertifiedServe::Service(
                        "reserved certified-body ingress lost its reply capability".to_owned(),
                    ));
                    return true;
                };
                let Some(ingress_ownership) = inbound.ingress_ownership() else {
                    prepared_serve = Some(PreparedCertifiedServe::Service(
                        "reserved certified-body ingress lost its ownership evidence".to_owned(),
                    ));
                    return true;
                };
                if reply_routes.semantic_target() != sender
                    || !ingress_ownership.validate_exact()
                    || !ingress_ownership.matches_message(inbound.message())
                    || !ingress_ownership.matches_semantic_origin(Some(sender))
                    || !ingress_ownership.matches_reply_routes(Some(reply_routes))
                {
                    prepared_serve = Some(PreparedCertifiedServe::Service(
                        "reserved certified-body ingress changed its transport ownership"
                            .to_owned(),
                    ));
                    return true;
                }
                let authenticated =
                    match executor.authenticate_certified_body_request(request.clone(), sender) {
                        Ok(authenticated) => authenticated,
                        Err(error) => {
                            prepared_serve = Some(
                                match services.stage_certified_serve_rejection(
                                    HashOf::new(request),
                                    CertifiedServeNegativeOutcome::InvalidCertificate,
                                ) {
                                    Ok(()) => PreparedCertifiedServe::Rejected(error.to_string()),
                                    Err(reason) => PreparedCertifiedServe::Service(reason),
                                },
                            );
                            return true;
                        }
                    };
                if superseded_by_decision {
                    let decided = terminal_subject.expect(
                        "Decision supersession requires the durable exact terminal subject",
                    );
                    prepared_serve = Some(
                        match services.stage_certified_serve_rejection(
                            authenticated.request_hash(),
                            CertifiedServeNegativeOutcome::SupersededByDurableDecision(decided),
                        ) {
                            Ok(()) => PreparedCertifiedServe::Rejected(
                                "certified body request was superseded by durable Decision"
                                    .to_owned(),
                            ),
                            Err(reason) => PreparedCertifiedServe::Service(reason),
                        },
                    );
                    return true;
                }
                match services.prepare_certified_request(authenticated_via, authenticated) {
                    Ok(admission) => {
                        prepared_serve = Some(PreparedCertifiedServe::Admitted(admission));
                        true
                    }
                    Err(CertifiedServePrepareError::Backpressure) => {
                        // `prepare_certified_request` installs the off-queue debt
                        // before returning capacity backpressure. The fair
                        // selector's immutable physical cutoff keeps every later
                        // ingress occurrence behind this retained target.
                        false
                    }
                    Err(CertifiedServePrepareError::Rejected(reason)) => {
                        prepared_serve = Some(PreparedCertifiedServe::Rejected(reason));
                        true
                    }
                    Err(CertifiedServePrepareError::Service(reason)) => {
                        prepared_serve = Some(PreparedCertifiedServe::Service(reason));
                        true
                    }
                }
            })
            .map_err(V2RunnerError::Service)?
        else {
            break;
        };
        if inbound.message().is_lane_local() {
            let _ = lane_work
                .accept_lane_message_with_ingress_ownership(inbound, executor.current_tag().view());
            let _ = lane_work.service_next_historical_recovery()?;
            continue;
        }
        let mut ingress_ownership = inbound.take_ingress_ownership().ok_or_else(|| {
            V2RunnerError::Service(
                "global Sumeragi v2 ingress lost its fair ownership carrier".to_owned(),
            )
        })?;
        if !ingress_ownership.validate_exact()
            || !ingress_ownership.matches_message(inbound.message())
            || !ingress_ownership.matches_semantic_origin(inbound.sender())
        {
            return Err(V2RunnerError::Service(
                "global Sumeragi v2 ingress carried altered fair ownership".to_owned(),
            ));
        }
        receiver
            .bind_leader_wire_runtime_ownership(&mut ingress_ownership)
            .map_err(V2RunnerError::Service)?;
        if dequeue_disposition == FairV2IngressDequeueDisposition::RetireObsolete {
            let receipt = ingress_ownership
                .leader_wire_runtime_receipt()
                .ok_or_else(|| {
                    V2RunnerError::Service(
                        "obsolete leader-wire dequeue lost its runtime receipt".to_owned(),
                    )
                })?;
            let token = receipt.token();
            iroha_logger::debug!(
                message_kind = ?super::FairV2IngressMessageKind::classify(inbound.message()),
                semantic_origin = ?inbound.sender(),
                authenticated_via = ?inbound.via(),
                obsolete_view = token.view(),
                active_view = executor.current_tag().view(),
                "retired WAL-obsolete Sumeragi v2 leader-wire carrier"
            );
            receiver
                .mark_obsolete_leader_wire_volatile_terminal(receipt)
                .map_err(V2RunnerError::Service)?;
            continue;
        }
        let (message, sender, reply_routes) = inbound.into_message_sender_and_reply_routes();
        if !ingress_ownership.matches_reply_routes(reply_routes.as_ref()) {
            return Err(V2RunnerError::Service(
                "global Sumeragi v2 ingress changed its authenticated reply routes".to_owned(),
            ));
        }
        let BlockMessage::V2(message) = message else {
            iroha_logger::debug!("rejected legacy global message on v2-only consensus ingress");
            mark_leader_wire_volatile(receiver, &ingress_ownership)?;
            continue;
        };
        if let Err(error) = message.validate_version() {
            iroha_logger::debug!(%error, "rejected wrong-version Sumeragi v2 envelope");
            mark_leader_wire_volatile(receiver, &ingress_ownership)?;
            continue;
        }
        match message.payload {
            wire::ConsensusMessageV2Payload::VrfCommit(commit) => {
                drop(ingress_ownership);
                let outcome = npos_vrf.accept_commit(commit, sender.as_ref());
                if matches!(outcome, super::v2_npos::V2VrfIngressOutcome::Rejected(_)) {
                    iroha_logger::debug!(?outcome, "rejected NPoS VRF commitment");
                }
            }
            wire::ConsensusMessageV2Payload::VrfReveal(reveal) => {
                drop(ingress_ownership);
                let outcome = npos_vrf.accept_reveal(reveal, sender.as_ref());
                if matches!(outcome, super::v2_npos::V2VrfIngressOutcome::Rejected(_)) {
                    iroha_logger::debug!(?outcome, "rejected NPoS VRF reveal");
                }
            }
            wire::ConsensusMessageV2Payload::Proposal(proposal) => {
                if !terminal_decision {
                    enqueue_control(
                        executor,
                        receiver,
                        wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::Proposal(
                            proposal,
                        )),
                        ingress_ownership,
                    )?;
                } else {
                    mark_leader_wire_volatile(receiver, &ingress_ownership)?;
                }
            }
            wire::ConsensusMessageV2Payload::Vote(vote) => {
                if !terminal_decision {
                    enqueue_control(
                        executor,
                        receiver,
                        wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::Vote(vote)),
                        ingress_ownership,
                    )?;
                } else {
                    mark_leader_wire_volatile(receiver, &ingress_ownership)?;
                }
            }
            wire::ConsensusMessageV2Payload::QuorumCertificate(certificate) => {
                if !terminal_decision {
                    enqueue_control(
                        executor,
                        receiver,
                        wire::ConsensusMessageV2::new(
                            wire::ConsensusMessageV2Payload::QuorumCertificate(certificate),
                        ),
                        ingress_ownership,
                    )?;
                } else {
                    mark_leader_wire_volatile(receiver, &ingress_ownership)?;
                }
            }
            wire::ConsensusMessageV2Payload::TimeoutVote(vote) => {
                if !terminal_decision {
                    enqueue_control(
                        executor,
                        receiver,
                        wire::ConsensusMessageV2::new(
                            wire::ConsensusMessageV2Payload::TimeoutVote(vote),
                        ),
                        ingress_ownership,
                    )?;
                } else {
                    mark_leader_wire_volatile(receiver, &ingress_ownership)?;
                }
            }
            wire::ConsensusMessageV2Payload::TimeoutCertificate(certificate) => {
                if !terminal_decision {
                    enqueue_control(
                        executor,
                        receiver,
                        wire::ConsensusMessageV2::new(
                            wire::ConsensusMessageV2Payload::TimeoutCertificate(certificate),
                        ),
                        ingress_ownership,
                    )?;
                } else {
                    mark_leader_wire_volatile(receiver, &ingress_ownership)?;
                }
            }
            wire::ConsensusMessageV2Payload::PayloadManifest(manifest) => {
                if let Err(error) = manifest.validate(executor.context()) {
                    iroha_logger::debug!(%error, "rejected standalone Sumeragi v2 manifest");
                }
                mark_leader_wire_volatile(receiver, &ingress_ownership)?;
            }
            wire::ConsensusMessageV2Payload::PayloadChunk(chunk) => {
                let Some(sender) = sender else {
                    mark_leader_wire_volatile(receiver, &ingress_ownership)?;
                    continue;
                };
                if terminal_decision
                    && services
                        .fetch_work_for_manifest(chunk.manifest_hash)
                        .is_none()
                {
                    // Proposal reordering justifies buffering an orphan chunk
                    // only while another Proposal can still open its fetch.
                    // After Decision, unmatched chunks can never become
                    // relevant and must not crowd the decided body's bounded
                    // transport completion out of the orphan buffer.
                    mark_leader_wire_volatile(receiver, &ingress_ownership)?;
                    continue;
                }
                services
                    .route_payload_chunk(executor, sender, chunk, ingress_ownership)
                    .map_err(V2RunnerError::Service)?;
            }
            wire::ConsensusMessageV2Payload::CertifiedBodyRequest(request) => {
                let Some(sender) = sender else {
                    mark_leader_wire_volatile(receiver, &ingress_ownership)?;
                    continue;
                };
                let Some(reply_routes) = reply_routes else {
                    iroha_logger::debug!(
                        %sender,
                        "rejected certified body request without authenticated reply route"
                    );
                    mark_leader_wire_volatile(receiver, &ingress_ownership)?;
                    continue;
                };
                if reply_routes.semantic_target() != &sender {
                    iroha_logger::debug!(
                        %sender,
                        "rejected certified body request with mismatched reply target"
                    );
                    mark_leader_wire_volatile(receiver, &ingress_ownership)?;
                    continue;
                }
                if request.round.height < executor.context().height {
                    let response_peer = sender.clone();
                    let terminal_ownership = ingress_ownership.clone();
                    let served = serve_block_sync_while_guarded(
                        output_guard,
                        || {
                            block_sync_server
                                .serve_historical_body(kura, request, &sender, local_key)
                        },
                        |response, permit| {
                            services.post_durable_history_response_on_reply_routes_with_permit(
                                response_peer,
                                reply_routes,
                                ingress_ownership,
                                response,
                                permit,
                            )
                        },
                    );
                    match finalize_bound_block_sync_serve(
                        served,
                        || mark_leader_wire_volatile(receiver, &terminal_ownership),
                        |error| {
                            iroha_logger::debug!(%error, "rejected historical certified body request");
                        },
                    )? {
                        BoundBlockSyncServeOutcome::Posted
                        | BoundBlockSyncServeOutcome::VolatileRemoteRejection => {}
                        BoundBlockSyncServeOutcome::VolatileNoResponse => {
                            iroha_logger::debug!(
                                "retired historical certified body request without a local response"
                            );
                        }
                    }
                } else if request.round.height == executor.context().height {
                    if certified_body_request_is_superseded_after_decision(
                        &request,
                        terminal_subject,
                        executor.context().height,
                    ) {
                        // Current-height serving authority narrows to the
                        // exact Decision. A certified losing body remains
                        // useful only before that terminal choice.
                        match prepared_serve.take() {
                            Some(PreparedCertifiedServe::Rejected(reason)) => {
                                iroha_logger::debug!(
                                    %reason,
                                    "retired certified body request superseded by Decision"
                                );
                                mark_leader_wire_volatile(receiver, &ingress_ownership)?;
                                continue;
                            }
                            Some(PreparedCertifiedServe::Service(reason)) => {
                                return Err(V2RunnerError::Service(reason));
                            }
                            Some(PreparedCertifiedServe::Admitted(_)) | None => {
                                return Err(V2RunnerError::Service(
                                    "Decision-superseded certified-body ingress crossed physical drain without its durable negative outcome"
                                        .to_owned(),
                                ));
                            }
                        }
                    }
                    match prepared_serve.take() {
                        Some(PreparedCertifiedServe::Admitted(admission)) => {
                            services
                                .serve_certified_request_on_routes(
                                    admission,
                                    reply_routes,
                                    ingress_ownership,
                                )
                                .map_err(V2RunnerError::Service)?;
                        }
                        Some(PreparedCertifiedServe::Rejected(reason)) => {
                            iroha_logger::debug!(%reason, "rejected certified body request");
                            mark_leader_wire_volatile(receiver, &ingress_ownership)?;
                        }
                        Some(PreparedCertifiedServe::Service(reason)) => {
                            return Err(V2RunnerError::Service(reason));
                        }
                        None => {
                            return Err(V2RunnerError::Service(
                                "current-height certified-body ingress crossed fair removal without an atomic Serve admission"
                                    .to_owned(),
                            ));
                        }
                    }
                } else {
                    iroha_logger::debug!(
                        requested_height = request.round.height,
                        active_height = executor.context().height,
                        "rejected future-height certified body request"
                    );
                    mark_leader_wire_volatile(receiver, &ingress_ownership)?;
                }
            }
            wire::ConsensusMessageV2Payload::CertifiedBodyResponse(response) => {
                let Some(sender) = sender else {
                    mark_leader_wire_volatile(receiver, &ingress_ownership)?;
                    continue;
                };
                let admission = executor.accept_certified_body_response_with_ingress_ownership(
                    response,
                    &sender,
                    &ingress_ownership,
                    services,
                );
                match admission {
                    Ok(_) => {}
                    Err(EffectTransportError::Backpressure) => {
                        // End the complete batch immediately. A second
                        // Runtime/Ingress pair could otherwise let later work
                        // overtake the newly retained exact carrier before the
                        // dedicated outer episode observes it.
                        return Ok(());
                    }
                    Err(EffectTransportError::FailClosed(reason)) => {
                        return Err(V2RunnerError::Service(reason));
                    }
                    Err(error) => {
                        iroha_logger::debug!(%error, "rejected certified body response");
                    }
                }
            }
            wire::ConsensusMessageV2Payload::CommitCertificateRequest(request) => {
                let Some(sender) = sender else {
                    mark_leader_wire_volatile(receiver, &ingress_ownership)?;
                    continue;
                };
                let Some(reply_routes) = reply_routes else {
                    iroha_logger::debug!(
                        %sender,
                        "rejected CommitQC request without authenticated reply route"
                    );
                    mark_leader_wire_volatile(receiver, &ingress_ownership)?;
                    continue;
                };
                if reply_routes.semantic_target() != &sender {
                    iroha_logger::debug!(
                        %sender,
                        "rejected CommitQC request with mismatched reply target"
                    );
                    mark_leader_wire_volatile(receiver, &ingress_ownership)?;
                    continue;
                }
                let response_peer = sender.clone();
                let terminal_ownership = ingress_ownership.clone();
                let served = serve_block_sync_while_guarded(
                    output_guard,
                    || block_sync_server.serve(kura, request, &sender, local_key),
                    |response, permit| {
                        services.post_durable_history_response_on_reply_routes_with_permit(
                            response_peer,
                            reply_routes,
                            ingress_ownership,
                            response,
                            permit,
                        )
                    },
                );
                match finalize_bound_block_sync_serve(
                    served,
                    || mark_leader_wire_volatile(receiver, &terminal_ownership),
                    |error| {
                        iroha_logger::debug!(%error, "rejected CommitQC discovery request");
                    },
                )? {
                    BoundBlockSyncServeOutcome::Posted
                    | BoundBlockSyncServeOutcome::VolatileRemoteRejection => {}
                    BoundBlockSyncServeOutcome::VolatileNoResponse => {
                        iroha_logger::debug!(
                            "retired CommitQC discovery request without a local response"
                        );
                    }
                }
            }
            wire::ConsensusMessageV2Payload::CommitCertificateResponse(response) => {
                if terminal_decision {
                    // A discovery response unwraps into a CommitQC and is
                    // therefore reducer-producing, unlike body/chunk
                    // transport completions. Decision is terminal for global
                    // consensus input at this height.
                    mark_leader_wire_volatile(receiver, &ingress_ownership)?;
                    continue;
                }
                let Some(sender) = sender else {
                    mark_leader_wire_volatile(receiver, &ingress_ownership)?;
                    continue;
                };
                let discovered = match block_sync.authenticate_response(response, &sender) {
                    Ok(discovered) => discovered,
                    Err(error) => {
                        iroha_logger::debug!(%error, "rejected CommitQC discovery response");
                        mark_leader_wire_volatile(receiver, &ingress_ownership)?;
                        continue;
                    }
                };
                let admission = block_sync.enqueue_and_complete(discovered, |message| {
                    executor.enqueue_discovered_commit_certificate(message, ingress_ownership)
                });
                if commit_certificate_admission_completed(admission)? {
                    *block_sync_request = None;
                }
            }
        }
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DecidedLaneRecoveryDrainCommitOutcome {
    LaneLocal,
    CurrentServe,
    HistoricalServe,
    LeaderWireVolatile,
}

trait DecidedLaneRecoveryDrainCommitter {
    type Admission;

    fn commit_lane_local(&mut self) -> Result<(), V2RunnerError>;

    fn commit_current_serve(
        &mut self,
        current: DecidedLaneRecoveryCurrentDrain<Self::Admission>,
    ) -> Result<(), V2RunnerError>;

    fn bind_leader_wire(&mut self) -> Result<(), V2RunnerError>;

    fn commit_historical_serve(&mut self) -> Result<(), V2RunnerError>;

    fn commit_leader_wire_volatile(&mut self) -> Result<(), V2RunnerError>;
}

fn commit_decided_lane_recovery_drain<C: DecidedLaneRecoveryDrainCommitter>(
    authorization: DecidedLaneRecoveryDrainAuthorization<C::Admission>,
    committer: &mut C,
) -> Result<DecidedLaneRecoveryDrainCommitOutcome, V2RunnerError> {
    match authorization {
        DecidedLaneRecoveryDrainAuthorization::LaneLocal => {
            committer.commit_lane_local()?;
            Ok(DecidedLaneRecoveryDrainCommitOutcome::LaneLocal)
        }
        DecidedLaneRecoveryDrainAuthorization::CurrentServe(current) => {
            committer.commit_current_serve(current)?;
            Ok(DecidedLaneRecoveryDrainCommitOutcome::CurrentServe)
        }
        DecidedLaneRecoveryDrainAuthorization::HistoricalServe => {
            committer.bind_leader_wire()?;
            committer.commit_historical_serve()?;
            Ok(DecidedLaneRecoveryDrainCommitOutcome::HistoricalServe)
        }
        DecidedLaneRecoveryDrainAuthorization::LeaderWireRetire => {
            committer.bind_leader_wire()?;
            committer.commit_leader_wire_volatile()?;
            Ok(DecidedLaneRecoveryDrainCommitOutcome::LeaderWireVolatile)
        }
    }
}

struct ProductionDecidedLaneRecoveryDrainCommitter<'a> {
    receiver: &'a FairV2Ingress,
    inbound: Option<InboundBlockMessage>,
    bound_leader_wire: Option<FairV2IngressOwnershipEvidence>,
    executor: &'a V2EffectExecutor,
    services: &'a mut ProductionV2Services,
    lane_work: &'a mut V2LaneWorkAdapter,
    active_view: wire::View,
    output_guard: &'a ConsensusOutputGuard,
    kura: &'a Kura,
    local_key: &'a KeyPair,
    block_sync_server: &'a mut V2BlockSyncServer,
}

impl ProductionDecidedLaneRecoveryDrainCommitter<'_> {
    fn take_inbound(&mut self) -> Result<InboundBlockMessage, V2RunnerError> {
        self.inbound.take().ok_or_else(|| {
            V2RunnerError::Service(
                "terminal recovery drain attempted to consume one ingress occurrence twice"
                    .to_owned(),
            )
        })
    }

    fn take_bound_leader_wire(&mut self) -> Result<FairV2IngressOwnershipEvidence, V2RunnerError> {
        self.bound_leader_wire.take().ok_or_else(|| {
            V2RunnerError::Service(
                "terminal recovery drain used leader-wire ownership before binding it".to_owned(),
            )
        })
    }
}

impl DecidedLaneRecoveryDrainCommitter for ProductionDecidedLaneRecoveryDrainCommitter<'_> {
    type Admission = CertifiedServeAdmission;

    fn commit_lane_local(&mut self) -> Result<(), V2RunnerError> {
        let inbound = self.take_inbound()?;
        let _ = self
            .lane_work
            .accept_lane_message_with_ingress_ownership(inbound, self.active_view);
        let _ = self.lane_work.service_next_historical_recovery()?;
        Ok(())
    }

    fn commit_current_serve(
        &mut self,
        current: DecidedLaneRecoveryCurrentDrain<Self::Admission>,
    ) -> Result<(), V2RunnerError> {
        let mut inbound = self.take_inbound()?;
        match current {
            DecidedLaneRecoveryCurrentDrain::Admitted(admission) => {
                let ingress_ownership = inbound.take_ingress_ownership().ok_or_else(|| {
                    V2RunnerError::Service(
                        "terminal recovery Serve admission lost its fair ownership".to_owned(),
                    )
                })?;
                let (_, _, reply_routes) = inbound.into_message_sender_and_reply_routes();
                self.services
                    .serve_certified_request_on_routes(
                        admission,
                        reply_routes.ok_or_else(|| {
                            V2RunnerError::Service(
                                "terminal recovery Serve admission lost its reply routes"
                                    .to_owned(),
                            )
                        })?,
                        ingress_ownership,
                    )
                    .map_err(V2RunnerError::Service)
            }
            DecidedLaneRecoveryCurrentDrain::Rejected(reason) => {
                iroha_logger::debug!(
                    %reason,
                    "retired terminal-recovery certified body request"
                );
                Ok(())
            }
        }
    }

    fn bind_leader_wire(&mut self) -> Result<(), V2RunnerError> {
        let inbound = self.inbound.as_mut().ok_or_else(|| {
            V2RunnerError::Service(
                "discarded terminal-recovery ingress was already consumed".to_owned(),
            )
        })?;
        let mut ingress_ownership = inbound.take_ingress_ownership().ok_or_else(|| {
            V2RunnerError::Service(
                "discarded terminal-recovery ingress lost its fair ownership carrier".to_owned(),
            )
        })?;
        if !ingress_ownership.validate_exact()
            || !ingress_ownership.matches_message(inbound.message())
            || !ingress_ownership.matches_semantic_origin(inbound.sender())
            || !ingress_ownership.matches_reply_routes(inbound.reply_routes())
        {
            return Err(V2RunnerError::Service(
                "discarded terminal-recovery ingress carried altered fair ownership".to_owned(),
            ));
        }
        self.receiver
            .bind_leader_wire_runtime_ownership(&mut ingress_ownership)
            .map_err(V2RunnerError::Service)?;
        self.bound_leader_wire = Some(ingress_ownership);
        Ok(())
    }

    fn commit_historical_serve(&mut self) -> Result<(), V2RunnerError> {
        let inbound = self.take_inbound()?;
        let ingress_ownership = self.take_bound_leader_wire()?;
        let (message, sender, reply_routes) = inbound.into_message_sender_and_reply_routes();
        let BlockMessage::V2(message) = message else {
            return Err(V2RunnerError::Service(
                "historical terminal-recovery route changed message class after authorization"
                    .to_owned(),
            ));
        };
        message
            .validate_version()
            .map_err(|error| V2RunnerError::Service(error.to_string()))?;
        let wire::ConsensusMessageV2Payload::CertifiedBodyRequest(request) = message.payload else {
            return Err(V2RunnerError::Service(
                "historical terminal-recovery route changed payload after authorization".to_owned(),
            ));
        };
        if request.round.height >= self.executor.context().height {
            return Err(V2RunnerError::Service(
                "historical terminal-recovery route crossed the active height".to_owned(),
            ));
        }
        let Some(sender) = sender else {
            mark_leader_wire_volatile(self.receiver, &ingress_ownership)?;
            return Ok(());
        };
        let Some(reply_routes) = reply_routes else {
            mark_leader_wire_volatile(self.receiver, &ingress_ownership)?;
            return Ok(());
        };
        if reply_routes.semantic_target() != &sender {
            mark_leader_wire_volatile(self.receiver, &ingress_ownership)?;
            return Ok(());
        }
        let response_peer = sender.clone();
        let terminal_ownership = ingress_ownership.clone();
        let output_guard = self.output_guard;
        let block_sync_server = &mut *self.block_sync_server;
        let kura = self.kura;
        let local_key = self.local_key;
        let services = &mut *self.services;
        let served = serve_block_sync_while_guarded(
            output_guard,
            || block_sync_server.serve_historical_body(kura, request, &sender, local_key),
            |response, permit| {
                services.post_durable_history_response_on_reply_routes_with_permit(
                    response_peer,
                    reply_routes,
                    ingress_ownership,
                    response,
                    permit,
                )
            },
        );
        match finalize_bound_block_sync_serve(
            served,
            || mark_leader_wire_volatile(self.receiver, &terminal_ownership),
            |error| {
                iroha_logger::debug!(
                    %error,
                    "rejected historical certified body request during terminal recovery"
                );
            },
        )? {
            BoundBlockSyncServeOutcome::Posted
            | BoundBlockSyncServeOutcome::VolatileRemoteRejection => {}
            BoundBlockSyncServeOutcome::VolatileNoResponse => {
                iroha_logger::debug!(
                    "retired terminal-recovery historical body request without a local response"
                );
            }
        }
        Ok(())
    }

    fn commit_leader_wire_volatile(&mut self) -> Result<(), V2RunnerError> {
        let _ = self.take_inbound()?;
        let ingress_ownership = self.take_bound_leader_wire()?;
        mark_leader_wire_volatile(self.receiver, &ingress_ownership)
    }
}

#[allow(clippy::too_many_arguments)]
fn drain_decided_lane_recovery_ingress(
    receiver: &FairV2Ingress,
    executor: &V2EffectExecutor,
    services: &mut ProductionV2Services,
    lane_work: &mut V2LaneWorkAdapter,
    active_view: wire::View,
    output_guard: &ConsensusOutputGuard,
    kura: &Kura,
    local_key: &KeyPair,
    block_sync_server: &mut V2BlockSyncServer,
) -> Result<(), V2RunnerError> {
    let decided_subject = executor
        .local_proposal_directive()?
        .decided_subject()
        .ok_or_else(|| {
            V2RunnerError::Service(
                "terminal lane recovery ingress lost its durable Decision subject".to_owned(),
            )
        })?;
    let mut authorization = None;
    let mut authorization_error = None;
    let inbound = receiver
        .try_recv_if_checked(|inbound| {
            if authorization_error.is_some() {
                return false;
            }
            let preparation = prepare_decided_lane_recovery_ingress(
                inbound,
                executor.context().height,
                decided_subject,
                |request, sender| {
                    executor
                        .authenticate_certified_body_request(request, sender)
                        .map_err(|error| error.to_string())
                },
            );
            match authorize_decided_lane_recovery_drain(preparation, services) {
                DecidedLaneRecoveryDrainDecision::Retain => false,
                DecidedLaneRecoveryDrainDecision::Authorized(candidate) => {
                    if authorization.replace(candidate).is_some() {
                        authorization_error = Some(
                            "terminal recovery selected more than one checked ingress occurrence"
                                .to_owned(),
                        );
                        false
                    } else {
                        true
                    }
                }
                DecidedLaneRecoveryDrainDecision::FailClosed(reason) => {
                    authorization_error = Some(reason);
                    false
                }
            }
        })
        .map_err(V2RunnerError::Service)?;
    if let Some(reason) = authorization_error {
        return Err(V2RunnerError::Service(reason));
    }
    let Some(inbound) = inbound else {
        return Ok(());
    };
    let authorization = authorization.ok_or_else(|| {
        V2RunnerError::Service(
            "terminal recovery checked dequeue lost its pre-drain authorization".to_owned(),
        )
    })?;
    let mut committer = ProductionDecidedLaneRecoveryDrainCommitter {
        receiver,
        inbound: Some(inbound),
        bound_leader_wire: None,
        executor,
        services,
        lane_work,
        active_view,
        output_guard,
        kura,
        local_key,
        block_sync_server,
    };
    let _ = commit_decided_lane_recovery_drain(authorization, &mut committer)?;
    // Non-Serve global traffic for this replayed terminal height is
    // intentionally dropped. The durable Decision and finality tuple are the
    // only global reducer authority. Current-height Serve traffic is instead
    // fully authenticated above and atomically terminalized before the carrier
    // can leave fair ingress. One occurrence per outer loop keeps pending
    // Apply/completion work dominant.
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum OuterIngressTurn {
    Completion,
    Runtime,
    Ingress,
}

fn outer_ingress_turns(limit: usize) -> impl Iterator<Item = OuterIngressTurn> {
    (0..limit.max(1)).flat_map(|_| {
        [
            OuterIngressTurn::Completion,
            OuterIngressTurn::Runtime,
            OuterIngressTurn::Ingress,
        ]
    })
}

fn v2_ingress_head_can_drain(
    inbound: &InboundBlockMessage,
    executor: &V2EffectExecutor,
    terminal_subject: Option<wire::BlockSubject>,
) -> bool {
    let BlockMessage::V2(message) = inbound.message() else {
        return true;
    };
    if message.validate_version().is_err() {
        return true;
    }
    if terminal_subject.is_some() && v2_payload_is_terminal_reducer_control(&message.payload) {
        // These messages are consumed and discarded once Decision is
        // installed. They must not remain behind a full terminal reducer
        // prefix and starve exact lane-completion traffic in fair ingress.
        return true;
    }
    if let wire::ConsensusMessageV2Payload::CertifiedBodyRequest(request) = &message.payload
        && certified_body_request_is_superseded_after_decision(
            request,
            terminal_subject,
            executor.context().height,
        )
    {
        // Losing current-height requests are discarded without consuming a
        // certified-body response slot, so they cannot pin fair ingress.
        return true;
    }
    let Some(ingress_ownership) = inbound.ingress_ownership() else {
        // Drain the malformed local carrier so the mutating seam can reject it
        // instead of blocking the fair queue forever.
        return true;
    };
    if !executor.can_admit_network_message_with_ingress_ownership(message, ingress_ownership) {
        return false;
    }
    true
}

fn certified_body_request_is_superseded_after_decision(
    request: &wire::CertifiedBodyRequest,
    terminal_subject: Option<wire::BlockSubject>,
    active_height: wire::Height,
) -> bool {
    terminal_subject
        .is_some_and(|decided| request.round.height == active_height && request.subject != decided)
}

const fn v2_payload_is_terminal_reducer_control(payload: &wire::ConsensusMessageV2Payload) -> bool {
    matches!(
        payload,
        wire::ConsensusMessageV2Payload::Proposal(_)
            | wire::ConsensusMessageV2Payload::Vote(_)
            | wire::ConsensusMessageV2Payload::QuorumCertificate(_)
            | wire::ConsensusMessageV2Payload::TimeoutVote(_)
            | wire::ConsensusMessageV2Payload::TimeoutCertificate(_)
            | wire::ConsensusMessageV2Payload::CommitCertificateResponse(_)
    )
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

/// Keep the pacemaker live for the full lifetime of a certified Serve barrier.
///
/// `older_runtime_episode_claimed` deliberately does not gate service. The
/// finite predecessor episode becomes Complete before a backpressured target
/// necessarily leaves fair ingress, while the absolute timeout and certified
/// progress roots remain independently live.
fn service_certified_serve_barrier_pacemaker_turn<E>(
    recovering_interrupted_tip: bool,
    _older_runtime_episode_claimed: bool,
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

fn pending_tip_recovery_deadline_error(
    output_guard: &ConsensusOutputGuard,
    timeout: Duration,
    attempts: u64,
    stage: Option<PendingKuraApplyRecoveryStage>,
) -> V2RunnerError {
    output_guard.activate_restart_required();
    super::status::mark_v2_restart_required();
    V2RunnerError::PendingTipRecoveryDeadlineExceeded {
        timeout,
        attempts,
        stage,
    }
}

fn advance_pending_tip_recovery_executor(
    executor: &mut V2EffectExecutor,
    services: &mut ProductionV2Services,
    limit: usize,
) -> Result<usize, V2RunnerError> {
    let mut advanced = 0_usize;
    for _ in 0..limit.max(1) {
        match executor.step_pending_tip_recovery(Instant::now(), services)? {
            EffectExecutorStep::Idle => break,
            EffectExecutorStep::Advanced { effects } => {
                advanced = advanced.saturating_add(effects);
            }
        }
    }
    Ok(advanced)
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
        non_zero(config.limits.control_queue_capacity)?,
        non_zero(config.limits.max_transactions)?,
        non_zero(config.limits.effect_work_capacity)?,
        non_zero(config.limits.chunk_queue_capacity)?,
        non_zero(config.limits.certified_request_capacity)?,
        non_zero(config.limits.control_queue_capacity)?,
        NonZeroUsize::new(reply_source_capacity).ok_or(V2RunnerError::InvalidLimits)?,
        NonZeroUsize::new(consensus_frame_byte_capacity).ok_or(V2RunnerError::InvalidLimits)?,
        NonZeroUsize::new(block_sync_frame_byte_capacity).ok_or(V2RunnerError::InvalidLimits)?,
        non_zero(config.limits.authenticated_merge_qc_capacity)?,
        merge_leader_body_frame_headroom_bytes,
        non_zero(config.limits.autonomous_carrier_headroom_bytes)?,
        Duration::from_millis(autonomous_producer_recheck_ms.get()),
        non_zero_u32(config.limits.historical_recovery_stuck_attempts)?,
        non_zero_u32(config.limits.historical_recovery_retry_tier_attempts)?,
        non_zero_u32(config.limits.historical_recovery_max_retry_tier)?,
        non_zero(config.limits.sidecar_service_burst)?,
        merge_sidecar_limits,
        merge_signing_guard_limits,
        native_amx_signing_guard_limits,
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
    let expected_merge_epoch = state
        .merge_ledger()
        .latest()
        .map_or(1, |latest| latest.epoch_id.saturating_add(1));
    let certified_merge_entry = state
        .select_pending_certified_merge_entry_for_round(round_header, expected_merge_epoch)
        .map_err(|error| V2RunnerError::Candidate(error.to_string()))?
        .map(|(_, entry, _)| entry)
        .map(|entry| {
            super::v2_lane_work::authenticate_merge_entry_for_height_context(context, &entry)
                .map(|()| entry)
                .map_err(V2RunnerError::Candidate)
        })
        .transpose()?;

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
    let parent_creation_time = match parent {
        CandidateParent::Block(parent) => parent.header().creation_time(),
        CandidateParent::Snapshot(anchor) => {
            Duration::from_millis(anchor.snapshot_block_creation_time_ms)
        }
    };
    Ok(CandidateAttachments {
        time_trigger_clock_progress_required: state
            .time_trigger_clock_progress_required_fast(parent_creation_time),
        npos_consensus_effects: (!effects.is_empty()).then_some(effects),
        certified_merge_carrier_header: certified_merge_entry
            .as_ref()
            .and_then(|entry| entry.execution_batch.as_ref())
            .map(|batch| batch.application_block_header.clone()),
        certified_merge_entry,
        ..CandidateAttachments::default()
    })
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

#[derive(Clone, Copy, Debug)]
struct HeartbeatOnlyWorkProvider;

impl CandidateWorkProvider for HeartbeatOnlyWorkProvider {
    fn prepare(
        &mut self,
        _context: &wire::HeightContext,
        _view: wire::View,
        candidates: &[CandidateDescriptor<'_>],
    ) -> Result<PreparedCandidateWork, CandidateWorkUnavailable> {
        if candidates.is_empty() {
            return Ok(PreparedCandidateWork::default());
        }
        Err(CandidateWorkUnavailable::new(
            (0..candidates.len()).collect::<BTreeSet<_>>(),
            "local fallback requested an empty heartbeat",
        ))
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
    apply_certified_merge_sidecar_closed_prefixes(lane_work, services)?;
    let pending = services
        .retry_pending_exact_output()
        .map_err(V2RunnerError::Service)?;
    apply_certified_merge_sidecar_chunk_admissions(lane_work, services, limit)?;
    Ok(pending)
}

fn rollover_finalized_height_outputs(
    mut lane_work: V2LaneWorkAdapter,
    services: &ProductionV2Services,
    receipt: &KuraV2CommitReceipt,
    artifact: &wire::finality::V2FinalityArtifact,
    successor: &wire::HeightContext,
    control_queue_capacity: usize,
) -> Result<RetainedMergeSidecars, V2RunnerError> {
    // Finality makes every current-height global/lane output either
    // Kura-reconstructible or explicitly superseded. Move those owners across
    // one local durable boundary before the successor opens the merge journal;
    // no predecessor thread survives this function.
    let _ = retry_exact_output_and_apply_sidecar_admissions(
        &mut lane_work,
        services,
        control_queue_capacity,
    )?;
    let _ = lane_work.recover_decided_canonical_lane_body(receipt, artifact)?;
    lane_work.persist_anchored_sessions()?;
    lane_work.prepare_canonical_lane_rollover(artifact)?;
    let durable_lane_authority = lane_work
        .durable_lane_rollover_authority(artifact)?
        .ok_or_else(|| {
            V2RunnerError::Service(
                "finalized lane output has not crossed its local durable reconstruction boundary"
                    .to_owned(),
            )
        })?;
    lane_work.prune_finalized_merge_sidecars()?;
    lane_work.retain_successor_owned_rollover_effects(artifact, &durable_lane_authority)?;

    loop {
        apply_certified_merge_sidecar_closed_prefixes(&mut lane_work, services)?;
        apply_certified_merge_sidecar_chunk_admissions(
            &mut lane_work,
            services,
            control_queue_capacity,
        )?;
        let retired = services
            .handoff_applied_height_output_to_durable_reconstruction(
                receipt,
                artifact,
                &durable_lane_authority,
            )
            .map_err(V2RunnerError::Service)?;
        apply_certified_merge_sidecar_chunk_admissions(
            &mut lane_work,
            services,
            control_queue_capacity,
        )?;
        if !services
            .has_pending_exact_output()
            .map_err(V2RunnerError::Service)?
        {
            break;
        }
        if retired == 0 {
            return Err(V2RunnerError::Service(
                "finalized exact output has no durable or move-only successor source".to_owned(),
            ));
        }
    }

    let _ = services
        .handoff_applied_height_output_to_durable_reconstruction(
            receipt,
            artifact,
            &durable_lane_authority,
        )
        .map_err(V2RunnerError::Service)?;
    if lane_work.has_pending_committed_output_handoff()
        || services
            .has_pending_exact_output()
            .map_err(V2RunnerError::Service)?
    {
        return Err(V2RunnerError::Service(
            "finalized output remained owned after durable handoff".to_owned(),
        ));
    }
    let exact_output_handoff = services
        .seal_applied_height_output_handoff(receipt, artifact, &durable_lane_authority)
        .map_err(V2RunnerError::Service)?;
    lane_work
        .into_retained_merge_sidecars(exact_output_handoff, artifact, successor)
        .map_err(V2RunnerError::from)
}

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

fn dispatch_lane_work_effects(
    lane_work: &mut V2LaneWorkAdapter,
    services: &ProductionV2Services,
    limit: usize,
) -> Result<(), V2RunnerError> {
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
    Ok(())
}

/// Remove retired source occurrences from semantic work already owned by the
/// lane adapter. A malformed effect which never carried its required route is
/// left intact so normal strict validation rejects it.
fn retain_active_owned_reply_routes(effect: &mut V2LaneWorkEffect) -> bool {
    retain_active_owned_reply_routes_with_snapshot_hook(effect, || {})
}

#[cfg(test)]
fn retain_active_owned_reply_routes_after_snapshot<AfterSnapshot>(
    effect: &mut V2LaneWorkEffect,
    after_snapshot: AfterSnapshot,
) -> bool
where
    AfterSnapshot: FnOnce(),
{
    retain_active_owned_reply_routes_with_snapshot_hook(effect, after_snapshot)
}

fn retain_active_owned_reply_routes_with_snapshot_hook<AfterSnapshot>(
    effect: &mut V2LaneWorkEffect,
    after_snapshot: AfterSnapshot,
) -> bool
where
    AfterSnapshot: FnOnce(),
{
    if let V2LaneWorkEffect::PostDurableLaneCertificate {
        reply_routes,
        ingress_ownership,
        ..
    } = effect
    {
        let Some(routes) = reply_routes.as_mut() else {
            return true;
        };
        let Some(ownership) = ingress_ownership.as_mut() else {
            return true;
        };
        if !ownership.validate_exact() || !ownership.matches_reply_routes(Some(routes)) {
            return true;
        }
        let (retained_routes, receipt) = routes.retain_active_with_receipt();
        after_snapshot();
        let Some(projected_routes) = ownership.project_retained_reply_routes(receipt) else {
            // Preserve malformed pre-existing ownership for strict dispatch;
            // ordinary retirement cannot reach this branch because the exact
            // route snapshot is projected without another liveness read.
            return true;
        };
        *routes = projected_routes;
        return retained_routes != 0;
    }
    let reply_routes = match effect {
        V2LaneWorkEffect::PostNativeAmx {
            reply_routes,
            message: NativeAmxMessage::PrepareVote(_) | NativeAmxMessage::CommitVote(_),
            ..
        } => reply_routes,
        V2LaneWorkEffect::PostCertifiedMergeSidecar {
            reply_routes,
            message,
            ..
        } if matches!(
            message.as_ref(),
            CertifiedMergeSidecarMessage::CloseAck(_)
                | CertifiedMergeSidecarMessage::GenerationHint(_)
                | CertifiedMergeSidecarMessage::Chunk(_)
        ) =>
        {
            reply_routes
        }
        V2LaneWorkEffect::PostLaneBlock { .. }
        | V2LaneWorkEffect::PostDurableLaneCertificate { .. }
        | V2LaneWorkEffect::PostNativeAmx { .. }
        | V2LaneWorkEffect::PostLaneDrainVote { .. }
        | V2LaneWorkEffect::BroadcastMerge(_)
        | V2LaneWorkEffect::PostCertifiedMergeSidecar { .. } => return true,
    };
    let Some(routes) = reply_routes.as_mut() else {
        return true;
    };
    let before = routes.clone();
    let (retained, receipt) = routes.retain_active_with_receipt();
    let Some(projected) = receipt.into_output(&before) else {
        // Preserve the operation's mutated value for normal strict dispatch;
        // this branch is unreachable for a module-minted receipt and exists
        // only to fail closed if its exact-history contract is broken.
        return true;
    };
    *routes = projected;
    retained != 0
}

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

fn drive_merge_sidecar_recovery(
    executor: &mut V2EffectExecutor,
    services: &mut ProductionV2Services,
    lane_work: &mut V2LaneWorkAdapter,
) -> Result<(), V2RunnerError> {
    lane_work.retain_deferred_merge_sidecars(&executor.deferred_merge_sidecar_blocks())?;
    while let Some(deferred) = services.take_merge_sidecar_deferral() {
        let entry_hash = deferred.reference().entry_hash;
        if !executor.retains_deferred_merge_sidecar(
            deferred.work_id(),
            deferred.round(),
            deferred.subject(),
            entry_hash,
        ) {
            continue;
        }
        let disposition = if executor.deferred_merge_sidecar_is_decided(deferred.work_id()) {
            lane_work.defer_missing_decided_merge_sidecar(
                deferred.round(),
                deferred.subject(),
                deferred.reference().clone(),
            )?
        } else {
            lane_work.defer_missing_merge_sidecar(
                deferred.round(),
                deferred.subject(),
                deferred.reference().clone(),
            )?
        };
        match disposition {
            MergeSidecarDeferralDisposition::Fetching
            | MergeSidecarDeferralDisposition::Available => {}
            MergeSidecarDeferralDisposition::RetryLater => {
                services
                    .requeue_merge_sidecar_deferral(deferred)
                    .map_err(V2RunnerError::Service)?;
                break;
            }
            MergeSidecarDeferralDisposition::Rejected(reason) => {
                let _ = executor.reject_deferred_merge_sidecar_work(
                    deferred.work_id(),
                    reason,
                    services,
                )?;
            }
        }
    }
    while let Some(entry_hash) = lane_work.take_completed_merge_sidecar() {
        let _ = executor.retry_deferred_merge_sidecar(entry_hash, services)?;
    }
    while let Some(rejected) = lane_work.take_rejected_merge_sidecar() {
        let _ = executor.reject_deferred_merge_sidecar(
            rejected.entry_hash(),
            rejected.reason(),
            services,
        )?;
    }
    lane_work.retain_deferred_merge_sidecars(&executor.deferred_merge_sidecar_blocks())?;
    Ok(())
}

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

/// Advance one retained historical lane owner on the ordinary retransmission
/// cadence, even when no lane or relay ingress arrives to trigger recovery.
fn service_historical_recovery_tick(
    lane_work: &mut V2LaneWorkAdapter,
) -> Result<HistoricalRecoveryServiceOutcome, V2RunnerError> {
    lane_work
        .service_next_historical_recovery()
        .map_err(V2RunnerError::from)
}

/// Fail-closed live-runner error.
#[derive(Debug, Error)]
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
    /// Staged and pending-replay height-one capabilities were both present.
    #[error("Sumeragi v2 startup produced conflicting authenticated genesis Nexus/AMX contexts")]
    ConflictingGenesisNexusContext,
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
    /// Even an empty fallback failed deterministic validation.
    #[error("Sumeragi v2 empty heartbeat failed validation: {0}")]
    LocalHeartbeatRejected(String),
    /// The exact bounded discovery request vanished before reducer admission.
    #[error("Sumeragi v2 CommitQC discovery request disappeared before reducer admission")]
    BlockSyncRequestDisappeared,
}

#[cfg(test)]
mod tests {
    use std::{
        cell::{Cell, RefCell},
        collections::VecDeque,
        sync::{Mutex, atomic::AtomicUsize},
    };

    use iroha_config::parameters::actual::{NodeRole, SumeragiV2KeyPolicy, SumeragiV2Limits};
    use iroha_crypto::{Algorithm, KeyPair, Signature};
    use iroha_data_model::{
        ChainId,
        account::AccountId,
        block::decode_framed_signed_block,
        isi::Log,
        peer::PeerId,
        transaction::{TransactionBuilder, signed::TransactionResultInner},
        trigger::DataTriggerSequence,
    };
    use iroha_logger::Level;
    use iroha_p2p::network::{
        NetworkActorAdmissionError, NetworkReplyFlushAckTestFixture, NetworkReplyRouteTestFixture,
        NetworkReplyRoutes,
    };
    use tempfile::TempDir;

    use super::super::FairV2IngressPushError;
    use super::*;
    use crate::{
        NetworkMessage,
        merge_sidecar::{
            CERTIFIED_MERGE_SIDECAR_VERSION_V1, CertifiedMergeSidecarChunkV1,
            CertifiedMergeSidecarCloseV1, CertifiedMergeSidecarMessage,
            CertifiedMergeSidecarSemanticSequenceV1, CertifiedMergeSidecarServiceGenerationV1,
            CertifiedMergeSidecarStreamEpochV1,
        },
        sumeragi::LaneRelayMessage,
    };

    #[test]
    fn complete_certified_serve_episode_cannot_veto_pacemaker() {
        let calls = Cell::new(0_u8);
        for older_runtime_episode_claimed in [true, false] {
            service_certified_serve_barrier_pacemaker_turn(
                false,
                older_runtime_episode_claimed,
                || {
                    calls.set(calls.get().saturating_add(1));
                    Ok::<(), ()>(())
                },
            )
            .expect("live certified Serve barrier services one pacemaker turn");
        }
        assert_eq!(
            calls.get(),
            2,
            "a Complete predecessor episode must service the pacemaker exactly like a newly claimed episode"
        );

        service_certified_serve_barrier_pacemaker_turn(false, false, || {
            calls.set(calls.get().saturating_add(1));
            Err::<(), _>("typed pacemaker failure")
        })
        .expect_err("live runner propagates a typed pacemaker failure");
        assert_eq!(calls.get(), 3);

        service_certified_serve_barrier_pacemaker_turn(true, false, || {
            calls.set(calls.get().saturating_add(1));
            Ok::<(), ()>(())
        })
        .expect("interrupted-tip recovery does not arm a fresh pacemaker");
        assert_eq!(calls.get(), 3);
    }

    #[test]
    fn dormant_live_serve_debt_latches_restart_instead_of_waiting_for_requester() {
        let (mut services, _) = super::super::v2_worker::tests::fixture();
        assert!(!services.exact_output_restart_required_for_test());
        let reason = services.fail_closed_dormant_certified_serve(41);
        assert!(reason.contains("41"));
        assert!(reason.contains("restart is required for local discharge"));
        assert!(
            services.exact_output_restart_required_for_test(),
            "a carrierless live Serve lifecycle must restart into local startup discharge"
        );
    }

    #[test]
    fn committed_lane_status_publisher_retries_revision_drift_without_publication() {
        let _guard = super::super::status::rbc_status_test_guard();
        super::super::status::clear_v2_status();
        let revision = Cell::new((1_u64, 1_u64, 1_u64));
        let mut publisher = CommittedLaneStatusPublisher::default();

        assert!(publisher.publish_if_changed_with(|| revision.get(), Vec::new));
        assert_eq!(publisher.published_revision, Some((1, 1, 1)));

        revision.set((2, 1, 1));
        let projection_ran = Cell::new(false);
        assert!(
            !publisher.publish_if_changed_with(
                || revision.get(),
                || {
                    projection_ran.set(true);
                    revision.set((3, 1, 1));
                    Vec::new()
                },
            ),
            "a projection spanning two revisions must not replace the global status root"
        );
        assert!(projection_ran.get());
        assert_eq!(
            publisher.published_revision,
            Some((1, 1, 1)),
            "revision drift must retain the prior acknowledgement for retry"
        );
        assert!(
            super::super::status::committed_lane_blocks_snapshot().is_empty(),
            "revision drift must retain the prior global status root"
        );

        assert!(
            publisher.publish_if_changed_with(|| revision.get(), Vec::new),
            "the next stable runner edge must retry the newer revision"
        );
        assert_eq!(publisher.published_revision, Some((3, 1, 1)));
        super::super::status::clear_v2_status();
    }

    #[test]
    fn pending_tip_recovery_gate_precedes_lane_work_construction() {
        let constructed = Cell::new(false);
        let error = construct_after_pending_tip_application_recovery(true, false, || {
            constructed.set(true);
            Ok(())
        })
        .expect_err("incomplete pending-tip recovery must block construction");
        assert!(matches!(error, V2RunnerError::PendingTipRecoveryIncomplete));
        assert!(
            !constructed.get(),
            "the lane-work constructor must not run before strict tip repair completes"
        );

        let value = construct_after_pending_tip_application_recovery(true, true, || {
            constructed.set(true);
            Ok(7_u8)
        })
        .expect("completed pending-tip recovery may construct lane work");
        assert_eq!(value, 7);
        assert!(constructed.get());
    }

    #[test]
    fn pending_tip_recovery_deadline_is_bounded_and_fail_closed() {
        let _status_guard = super::super::status::rbc_status_test_guard();
        super::super::status::clear_v2_status();
        let started_at = Instant::now();
        let round_timeout = Duration::from_secs(10);
        let deadline = PendingTipRecoveryDeadline::new(started_at, round_timeout)
            .expect("derive recovery deadline");
        assert_eq!(deadline.timeout, Duration::from_secs(30));
        assert!(!deadline.expired(started_at + Duration::from_secs(30) - Duration::from_nanos(1)));
        assert!(deadline.expired(started_at + Duration::from_secs(30)));
        assert_eq!(
            deadline.remaining(started_at + Duration::from_secs(29)),
            Duration::from_secs(1)
        );

        let output_guard = ConsensusOutputGuard::isolated();
        let error = pending_tip_recovery_deadline_error(
            output_guard.as_ref(),
            deadline.timeout,
            17,
            Some(PendingKuraApplyRecoveryStage::ApplicationDispatched),
        );
        assert!(output_guard.restart_required());
        assert!(matches!(
            error,
            V2RunnerError::PendingTipRecoveryDeadlineExceeded {
                timeout,
                attempts: 17,
                stage: Some(PendingKuraApplyRecoveryStage::ApplicationDispatched),
            } if timeout == Duration::from_secs(30)
        ));
        super::super::status::clear_v2_status();
    }

    #[test]
    fn bounded_sidecar_admission_turn_applies_only_its_budget() {
        let mut queued = VecDeque::from([1_u8, 2, 3]);
        let mut applied = Vec::new();
        let count = apply_bounded_sidecar_admissions(
            1,
            || Ok::<_, ()>(queued.pop_front()),
            |item| {
                applied.push(item);
                Ok::<_, ()>(())
            },
        )
        .expect("bounded admission turn");
        assert_eq!(count, 1);
        assert_eq!(applied, vec![1]);
        assert_eq!(queued, VecDeque::from([2, 3]));

        let result = apply_bounded_sidecar_admissions(
            2,
            || Ok::<_, &'static str>(queued.pop_front()),
            |_item| Err("fail-stop acknowledgement"),
        );
        assert_eq!(result, Err("fail-stop acknowledgement"));
        assert_eq!(queued, VecDeque::from([3]));
    }

    #[test]
    fn quiet_retransmission_tick_services_one_retained_historical_session() {
        let mut lane_work = super::super::v2_lane_work::tests::quiet_historical_recovery_fixture();
        assert!(lane_work.has_pending_historical_recovery());

        let outcome = service_historical_recovery_tick(&mut lane_work)
            .expect("quiet retransmission tick advances retained history");
        let HistoricalRecoveryServiceOutcome::Waiting(wait) = outcome else {
            panic!("missing canonical body must remain a typed quiet-network retry: {outcome:?}");
        };
        assert_eq!(
            wait.reason(),
            super::super::v2_lane_work::HistoricalRecoveryWaitReason::CanonicalBlockPending
        );
        assert!(wait.first_observation());
        assert!(
            lane_work.has_pending_historical_recovery(),
            "one bounded wait turn must retain the exact historical owner"
        );
    }

    #[test]
    fn empty_drain_after_peek_is_restart_required_without_panicking() {
        assert!(matches!(
            require_peeked_lane_work_effect(None),
            Err(V2RunnerError::RestartRequired)
        ));
    }

    fn context() -> (wire::HeightContext, Vec<KeyPair>) {
        let mut keys = (1_u8..=4)
            .map(|seed| {
                KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                    .expect("deterministic key")
            })
            .collect::<Vec<_>>();
        keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
        let roster = keys
            .iter()
            .map(|key| wire::ValidatorPower {
                validator: PeerId::new(key.public_key().clone()),
                power: 1,
            })
            .collect::<Vec<_>>();
        (
            wire::HeightContext {
                chain_id: ChainId::from("v2-runner-test"),
                protocol_version: wire::PROTOCOL_VERSION,
                height: 1,
                epoch: 0,
                epoch_end_height: u64::MAX,
                next_epoch_snapshot: None,
                mode: wire::ConsensusMode::Permissioned,
                parent_commit_qc: None,
                snapshot_bootstrap: None,
                quorum: wire::DualQuorum::from_roster(&roster).expect("quorum"),
                roster,
                nexus_amx_context_hash: Hash::new(b"runner-test-nexus-amx"),
                execution_policy_hash: iroha_crypto::Hash::new(b"test execution policy"),
                da_layout: wire::DataAvailabilityLayout {
                    encoding: wire::PayloadEncoding::ReedSolomon16,
                    chunk_size_bytes: 1024,
                    data_shards: 1,
                    parity_shards: 1,
                    max_payload_size_bytes: 4096,
                    max_chunk_count: 8,
                },
                leader_seed: [0x42; 32],
            },
            keys,
        )
    }

    fn decided_recovery_certified_request(
        context: &wire::HeightContext,
        requester_key: &KeyPair,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
    ) -> wire::CertifiedBodyRequest {
        let mut request = wire::CertifiedBodyRequest {
            round,
            subject,
            certificate: wire::QuorumCertificate {
                round,
                proposal_round: round,
                phase: wire::GlobalPhase::Prepare,
                subject,
                execution_commitment: wire::ExecutionCommitment::without_topups(
                    Hash::new(b"runner recovery parent state"),
                    Hash::new(b"runner recovery post state"),
                    Hash::new(b"runner recovery ordinary writes"),
                    Hash::new(b"runner recovery executed block"),
                ),
                signers: (0..context.roster.len())
                    .map(|index| u32::try_from(index).expect("small runner roster index"))
                    .collect(),
                aggregate_signature: vec![0xA5; 48],
            },
            requester: PeerId::new(requester_key.public_key().clone()),
            signature: Vec::new(),
        };
        request.signature =
            Signature::new(requester_key.private_key(), &request.signature_preimage())
                .payload()
                .to_vec();
        request
    }

    fn admitted_decided_recovery_request(
        context: &wire::HeightContext,
        request: &wire::CertifiedBodyRequest,
    ) -> InboundBlockMessage {
        let requester = request.requester.clone();
        super::super::fair_v2_ingress_admit_with_roster_for_test(
            InboundBlockMessage::from_transport(
                BlockMessage::V2(wire::ConsensusMessageV2::new(
                    wire::ConsensusMessageV2Payload::CertifiedBodyRequest(request.clone()),
                )),
                requester.clone(),
                requester,
            ),
            context
                .roster
                .iter()
                .map(|entry| entry.validator.clone())
                .collect(),
        )
    }

    enum RecordedPrepareOutcome {
        Admitted(u8),
        Backpressure,
        Rejected(&'static str),
        Service(&'static str),
    }

    struct RecordingDecidedLaneAuthorizer {
        calls: Vec<&'static str>,
        stage_error: Option<&'static str>,
        prepare_outcome: Option<RecordedPrepareOutcome>,
    }

    impl RecordingDecidedLaneAuthorizer {
        fn new(prepare_outcome: RecordedPrepareOutcome) -> Self {
            Self {
                calls: Vec::new(),
                stage_error: None,
                prepare_outcome: Some(prepare_outcome),
            }
        }
    }

    impl DecidedLaneRecoveryDrainAuthorizer for RecordingDecidedLaneAuthorizer {
        type Admission = u8;

        fn stage_negative(
            &mut self,
            _request_hash: HashOf<wire::CertifiedBodyRequest>,
            _outcome: CertifiedServeNegativeOutcome,
        ) -> Result<(), String> {
            self.calls.push("stage-negative");
            self.stage_error
                .map_or(Ok(()), |reason| Err(reason.to_owned()))
        }

        fn prepare_exact(
            &mut self,
            _authenticated_via: &PeerId,
            _request: AuthenticatedCertifiedBodyRequest,
        ) -> Result<Self::Admission, CertifiedServePrepareError> {
            self.calls.push("prepare-exact");
            match self
                .prepare_outcome
                .take()
                .expect("recording authorizer is called at most once")
            {
                RecordedPrepareOutcome::Admitted(admission) => Ok(admission),
                RecordedPrepareOutcome::Backpressure => {
                    Err(CertifiedServePrepareError::Backpressure)
                }
                RecordedPrepareOutcome::Rejected(reason) => {
                    Err(CertifiedServePrepareError::Rejected(reason.to_owned()))
                }
                RecordedPrepareOutcome::Service(reason) => {
                    Err(CertifiedServePrepareError::Service(reason.to_owned()))
                }
            }
        }
    }

    struct RecordingDecidedLaneCommitter {
        calls: Vec<&'static str>,
        fail_on: Option<&'static str>,
    }

    impl RecordingDecidedLaneCommitter {
        fn new(fail_on: Option<&'static str>) -> Self {
            Self {
                calls: Vec::new(),
                fail_on,
            }
        }

        fn record(&mut self, operation: &'static str) -> Result<(), V2RunnerError> {
            self.calls.push(operation);
            if self.fail_on == Some(operation) {
                Err(V2RunnerError::Service(format!("{operation} failed")))
            } else {
                Ok(())
            }
        }
    }

    impl DecidedLaneRecoveryDrainCommitter for RecordingDecidedLaneCommitter {
        type Admission = u8;

        fn commit_lane_local(&mut self) -> Result<(), V2RunnerError> {
            self.record("lane-local")
        }

        fn commit_current_serve(
            &mut self,
            current: DecidedLaneRecoveryCurrentDrain<Self::Admission>,
        ) -> Result<(), V2RunnerError> {
            match current {
                DecidedLaneRecoveryCurrentDrain::Admitted(7) => self.record("serve-exact-decided"),
                DecidedLaneRecoveryCurrentDrain::Rejected(_) => {
                    self.record("retire-staged-negative")
                }
                DecidedLaneRecoveryCurrentDrain::Admitted(other) => Err(V2RunnerError::Service(
                    format!("unexpected test admission {other}"),
                )),
            }
        }

        fn bind_leader_wire(&mut self) -> Result<(), V2RunnerError> {
            self.record("bind-leader-wire")
        }

        fn commit_historical_serve(&mut self) -> Result<(), V2RunnerError> {
            self.record("serve-history")
        }

        fn commit_leader_wire_volatile(&mut self) -> Result<(), V2RunnerError> {
            self.record("retire-volatile")
        }
    }

    #[test]
    fn decided_lane_checked_drain_requires_staged_or_prepared_outcome() {
        let (context, keys) = context();
        let round = wire::ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view: 3,
        };
        let subject = proposal_subject(b"decided drain authorization subject");
        let decided_subject = proposal_subject(b"decided drain authorization winner");
        let request = decided_recovery_certified_request(&context, &keys[1], round, subject);
        let inbound = admitted_decided_recovery_request(&context, &request);
        let prepare = |winner| {
            prepare_decided_lane_recovery_ingress(
                &inbound,
                context.height,
                winner,
                |request, sender| {
                    super::super::v2_transport::authenticate_certified_body_request(
                        &context,
                        request,
                        sender,
                        |_, _| Ok::<(), String>(()),
                    )
                    .map_err(|error| error.to_string())
                },
            )
        };

        let mut staged = RecordingDecidedLaneAuthorizer::new(RecordedPrepareOutcome::Backpressure);
        let staged_decision =
            authorize_decided_lane_recovery_drain(prepare(decided_subject), &mut staged);
        assert!(matches!(
            staged_decision,
            DecidedLaneRecoveryDrainDecision::Authorized(
                DecidedLaneRecoveryDrainAuthorization::CurrentServe(
                    DecidedLaneRecoveryCurrentDrain::Rejected(_)
                )
            )
        ));
        staged.calls.push("checked-drain");
        assert_eq!(staged.calls, ["stage-negative", "checked-drain"]);

        let mut failed_stage =
            RecordingDecidedLaneAuthorizer::new(RecordedPrepareOutcome::Backpressure);
        failed_stage.stage_error = Some("durable negative write failed");
        assert!(matches!(
            authorize_decided_lane_recovery_drain(
                prepare(decided_subject),
                &mut failed_stage,
            ),
            DecidedLaneRecoveryDrainDecision::FailClosed(reason)
                if reason == "durable negative write failed"
        ));
        assert_eq!(failed_stage.calls, ["stage-negative"]);

        let mut admitted = RecordingDecidedLaneAuthorizer::new(RecordedPrepareOutcome::Admitted(7));
        assert!(matches!(
            authorize_decided_lane_recovery_drain(prepare(subject), &mut admitted),
            DecidedLaneRecoveryDrainDecision::Authorized(
                DecidedLaneRecoveryDrainAuthorization::CurrentServe(
                    DecidedLaneRecoveryCurrentDrain::Admitted(7)
                )
            )
        ));
        admitted.calls.push("checked-drain");
        assert_eq!(admitted.calls, ["prepare-exact", "checked-drain"]);

        let mut typed_rejection = RecordingDecidedLaneAuthorizer::new(
            RecordedPrepareOutcome::Rejected("typed negative was staged"),
        );
        assert!(matches!(
            authorize_decided_lane_recovery_drain(prepare(subject), &mut typed_rejection),
            DecidedLaneRecoveryDrainDecision::Authorized(
                DecidedLaneRecoveryDrainAuthorization::CurrentServe(
                    DecidedLaneRecoveryCurrentDrain::Rejected(reason)
                )
            ) if reason == "typed negative was staged"
        ));
        typed_rejection.calls.push("checked-drain");
        assert_eq!(typed_rejection.calls, ["prepare-exact", "checked-drain"]);

        let mut backpressured =
            RecordingDecidedLaneAuthorizer::new(RecordedPrepareOutcome::Backpressure);
        assert!(matches!(
            authorize_decided_lane_recovery_drain(prepare(subject), &mut backpressured),
            DecidedLaneRecoveryDrainDecision::Retain
        ));
        assert_eq!(backpressured.calls, ["prepare-exact"]);

        let mut failed_prepare = RecordingDecidedLaneAuthorizer::new(
            RecordedPrepareOutcome::Service("Serve preparation failed"),
        );
        assert!(matches!(
            authorize_decided_lane_recovery_drain(prepare(subject), &mut failed_prepare),
            DecidedLaneRecoveryDrainDecision::FailClosed(reason)
                if reason == "Serve preparation failed"
        ));
        assert_eq!(
            failed_prepare.calls,
            ["prepare-exact"],
            "service failure cannot authorize checked dequeue"
        );
    }

    #[test]
    fn decided_lane_commit_orders_bind_before_history_or_volatile_retirement() {
        let mut exact = RecordingDecidedLaneCommitter::new(None);
        assert_eq!(
            commit_decided_lane_recovery_drain(
                DecidedLaneRecoveryDrainAuthorization::CurrentServe(
                    DecidedLaneRecoveryCurrentDrain::Admitted(7),
                ),
                &mut exact,
            )
            .expect("commit exact decided Serve"),
            DecidedLaneRecoveryDrainCommitOutcome::CurrentServe
        );
        assert_eq!(exact.calls, ["serve-exact-decided"]);

        let mut negative = RecordingDecidedLaneCommitter::new(None);
        assert_eq!(
            commit_decided_lane_recovery_drain(
                DecidedLaneRecoveryDrainAuthorization::CurrentServe(
                    DecidedLaneRecoveryCurrentDrain::Rejected(
                        "durable negative staged".to_owned(),
                    ),
                ),
                &mut negative,
            )
            .expect("retire staged negative"),
            DecidedLaneRecoveryDrainCommitOutcome::CurrentServe
        );
        assert_eq!(negative.calls, ["retire-staged-negative"]);

        let mut historical = RecordingDecidedLaneCommitter::new(None);
        assert_eq!(
            commit_decided_lane_recovery_drain(
                DecidedLaneRecoveryDrainAuthorization::HistoricalServe,
                &mut historical,
            )
            .expect("route historical Serve"),
            DecidedLaneRecoveryDrainCommitOutcome::HistoricalServe
        );
        assert_eq!(historical.calls, ["bind-leader-wire", "serve-history"]);

        let mut volatile = RecordingDecidedLaneCommitter::new(None);
        assert_eq!(
            commit_decided_lane_recovery_drain(
                DecidedLaneRecoveryDrainAuthorization::LeaderWireRetire,
                &mut volatile,
            )
            .expect("retire non-Serve terminal traffic"),
            DecidedLaneRecoveryDrainCommitOutcome::LeaderWireVolatile
        );
        assert_eq!(volatile.calls, ["bind-leader-wire", "retire-volatile"]);

        let mut failed_bind = RecordingDecidedLaneCommitter::new(Some("bind-leader-wire"));
        assert!(
            commit_decided_lane_recovery_drain(
                DecidedLaneRecoveryDrainAuthorization::LeaderWireRetire,
                &mut failed_bind,
            )
            .is_err()
        );
        assert_eq!(
            failed_bind.calls,
            ["bind-leader-wire"],
            "a failed bind cannot publish VolatileTerminal"
        );
    }

    #[test]
    fn drain_decided_lane_recovery_ingress_prepares_exact_and_typed_negative_branches() {
        let (context, keys) = context();
        let round = wire::ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view: 3,
        };
        let subject = proposal_subject(b"decided recovery exact subject");
        let request = decided_recovery_certified_request(&context, &keys[1], round, subject);
        let inbound = admitted_decided_recovery_request(&context, &request);
        let authenticate = |request, sender: &PeerId| {
            super::super::v2_transport::authenticate_certified_body_request(
                &context,
                request,
                sender,
                |_, _| Ok::<(), String>(()),
            )
            .map_err(|error| error.to_string())
        };
        assert!(matches!(
            prepare_decided_lane_recovery_ingress(
                &inbound,
                context.height,
                subject,
                authenticate,
            ),
            DecidedLaneRecoveryIngressPreparation::CurrentServe(
                DecidedLaneRecoveryCurrentServe::Authenticated { request: authenticated, .. }
            ) if authenticated.request_hash() == HashOf::new(&request)
        ));

        assert!(matches!(
            prepare_decided_lane_recovery_ingress(
                &inbound,
                context.height,
                subject,
                |_, _| Err("invalid aggregate signature".to_owned()),
            ),
            DecidedLaneRecoveryIngressPreparation::CurrentServe(
                DecidedLaneRecoveryCurrentServe::Negative {
                    request_hash,
                    outcome: CertifiedServeNegativeOutcome::InvalidCertificate,
                    ..
                }
            ) if request_hash == HashOf::new(&request)
        ));

        let decided_subject = proposal_subject(b"decided recovery winning subject");
        assert_ne!(decided_subject, subject);
        assert!(matches!(
            prepare_decided_lane_recovery_ingress(
                &inbound,
                context.height,
                decided_subject,
                |request, sender| {
                    super::super::v2_transport::authenticate_certified_body_request(
                        &context,
                        request,
                        sender,
                        |_, _| Ok::<(), String>(()),
                    )
                    .map_err(|error| error.to_string())
                },
            ),
            DecidedLaneRecoveryIngressPreparation::CurrentServe(
                DecidedLaneRecoveryCurrentServe::Negative {
                    request_hash,
                    outcome:
                        CertifiedServeNegativeOutcome::SupersededByDurableDecision(decided),
                    ..
                }
            ) if request_hash == HashOf::new(&request) && decided == decided_subject
        ));
    }

    #[test]
    fn drain_decided_lane_recovery_ingress_routes_history_and_volatile_terminal_traffic() {
        let (context, keys) = context();
        let round = wire::ConsensusRound {
            context_id: context.id(),
            height: context.height.saturating_sub(1),
            view: 0,
        };
        let subject = proposal_subject(b"decided recovery historical subject");
        let historical = decided_recovery_certified_request(&context, &keys[1], round, subject);
        let historical_inbound = admitted_decided_recovery_request(&context, &historical);
        assert!(matches!(
            prepare_decided_lane_recovery_ingress(
                &historical_inbound,
                context.height,
                subject,
                |_, _| panic!("historical classification precedes active Serve authentication"),
            ),
            DecidedLaneRecoveryIngressPreparation::HistoricalServe
        ));

        let peer = context.roster[0].validator.clone();
        let non_serve = wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::PayloadChunk(wire::PayloadChunk {
                manifest_hash: HashOf::from_untyped_unchecked(Hash::new(
                    b"wrong-version recovery chunk",
                )),
                index: 0,
                bytes: vec![0xA5],
                sender: 0,
                signature: vec![0x5A],
            }),
        );
        let ordinary_non_serve =
            InboundBlockMessage::new(BlockMessage::V2(non_serve.clone()), Some(peer.clone()));
        assert!(matches!(
            prepare_decided_lane_recovery_ingress(
                &ordinary_non_serve,
                context.height,
                subject,
                |_, _| panic!("non-Serve traffic never reaches Serve authentication"),
            ),
            DecidedLaneRecoveryIngressPreparation::LeaderWireRetire
        ));

        let mut wrong_version = non_serve;
        wrong_version.protocol_version = wrong_version.protocol_version.saturating_sub(1);
        let wrong_version =
            InboundBlockMessage::new(BlockMessage::V2(wrong_version), Some(peer.clone()));
        assert!(matches!(
            prepare_decided_lane_recovery_ingress(
                &wrong_version,
                context.height,
                subject,
                |_, _| panic!("non-Serve traffic never reaches Serve authentication"),
            ),
            DecidedLaneRecoveryIngressPreparation::LeaderWireRetire
        ));
    }

    fn leader_wire_runtime_ingress_fixture() -> (
        TempDir,
        FairV2Ingress,
        Arc<LeaderWireLifecycleStoreGate>,
        FairV2IngressOwnershipEvidence,
        wire::ConsensusMessageV2,
        PeerId,
    ) {
        let (context, keys) = context();
        let round = wire::ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view: 0,
        };
        let body = b"runner authenticated-Coalesce defense".to_vec();
        let subject = wire::BlockSubject {
            parent_block_hash: None,
            block_hash: HashOf::from_untyped_unchecked(Hash::new(
                b"runner authenticated-Coalesce block",
            )),
            payload_hash: Hash::new(&body),
        };
        let manifest = encode_payload(&context, round, subject, &body)
            .expect("encode runner leader-wire fixture payload")
            .manifest()
            .clone();
        let proposer = context.leader(round.view);
        let mut proposal = wire::Proposal {
            round,
            proposer,
            subject,
            manifest,
            justification: wire::ProposalJustification::ParentCommit(
                wire::ParentCommitJustification { certificate: None },
            ),
            signature: Vec::new(),
        };
        proposal.signature = Signature::new(
            keys[usize::try_from(proposer).expect("small runner proposer index")].private_key(),
            &proposal.signature_preimage(),
        )
        .payload()
        .to_vec();
        let message =
            wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::Proposal(proposal));
        let semantic_origin = context.roster
            [usize::try_from(proposer).expect("small runner proposer index")]
        .validator
        .clone();
        let roster = context
            .roster
            .iter()
            .map(|entry| entry.validator.clone())
            .collect::<Vec<_>>();
        let directory = TempDir::new().expect("temporary runner leader-wire directory");
        let ingress = FairV2Ingress::new_with_source_geometry_and_transport_frame_caps(
            64,
            512 * 1024 * 1024,
            64 * 1024 * 1024,
            super::super::CERTIFIED_FENCE_ESCAPE_RESERVE_BYTES,
            8 * 1024 * 1024,
            8 * 1024 * 1024,
            usize::MAX,
            usize::MAX,
            usize::MAX,
            usize::MAX,
            None,
        );
        ingress
            .configure_roster_for_context(roster.clone(), &context.chain_id, context.da_layout)
            .expect("configure runner leader-wire ingress");
        ingress.require_leader_wire_lifecycle_gate();
        let capacity = LeaderWireLifecycleStoreGate::derived_capacity(
            roster.len(),
            context.da_layout.max_chunk_count,
        )
        .expect("derive runner leader-wire capacity");
        let owner = [0xD4; 32];
        let recovery_authority =
            super::super::serviced_candidate_store::LeaderWireRecoveryAuthority::from_replayed_adapter(
                context.id(),
                context.height,
                owner,
                round.view,
                false,
            );
        let (gate, restore) = LeaderWireLifecycleStoreGate::open(
            &directory.path().join("runner-leader-wire.wal"),
            context.id(),
            context.height,
            owner,
            roster.iter().cloned().collect(),
            capacity,
            context.da_layout.max_chunk_count,
            recovery_authority,
            &[],
            &[],
        )
        .expect("open runner leader-wire gate");
        ingress
            .bind_leader_wire_lifecycle_gate(
                Arc::clone(&gate),
                restore,
                super::super::v2_runtime::RuntimeLifecycleOrdinalSource::after_high_watermark(64),
                context.id(),
                context.height,
            )
            .expect("bind runner leader-wire gate");
        ingress.open().expect("open runner leader-wire ingress");
        assert!(matches!(
            ingress.try_push(InboundBlockMessage::new(
                BlockMessage::V2(message.clone()),
                Some(semantic_origin.clone()),
            )),
            Ok(super::super::FairV2IngressPushDisposition::Enqueued)
        ));
        let mut admitted = ingress
            .try_recv()
            .expect("drain runner leader-wire fixture");
        let mut ownership = admitted
            .take_ingress_ownership()
            .expect("runner leader-wire fixture retains ingress ownership");
        ingress
            .bind_leader_wire_runtime_ownership(&mut ownership)
            .expect("bind runner leader-wire runtime receipt");
        (
            directory,
            ingress,
            gate,
            ownership,
            message,
            semantic_origin,
        )
    }

    #[test]
    fn fail_closed_authenticated_coalesce_releases_gate_and_suppresses_retry() {
        let (_directory, ingress, gate, ownership, message, semantic_origin) =
            leader_wire_runtime_ingress_fixture();
        assert!(
            ownership.leader_wire_runtime_receipt().is_some(),
            "the synthetic stale Coalesce result carries a fresh runtime receipt"
        );
        assert_eq!(
            gate.earliest_ingress_scheduler_ordinal()
                .expect("read runner leader-wire minimum"),
            None,
            "a runtime-bound owner has already left the durable Ingress selector"
        );

        assert!(matches!(
            complete_control_ingress_admission(
                &ingress,
                &ownership,
                Err(NetworkIngressError::FailClosed),
            ),
            Err(V2RunnerError::RuntimeFailClosed)
        ));
        assert_eq!(
            gate.earliest_ingress_scheduler_ordinal()
                .expect("read retired runner leader-wire minimum"),
            None
        );
        assert!(matches!(
            ingress.try_push(InboundBlockMessage::new(
                BlockMessage::V2(message),
                Some(semantic_origin),
            )),
            Ok(super::super::FairV2IngressPushDisposition::Coalesced)
        ));
        assert_eq!(
            gate.earliest_ingress_scheduler_ordinal()
                .expect("retry retains the terminal tombstone"),
            None
        );
    }

    #[test]
    fn heartbeat_provider_accepts_only_an_empty_descriptor_batch() {
        let (context, _) = context();
        let mut provider = HeartbeatOnlyWorkProvider;
        assert_eq!(
            provider
                .prepare(&context, 0, &[])
                .expect("an explicit heartbeat has no lane work")
                .native_amx_receipts,
            Vec::new()
        );
    }

    fn test_predecessor(
        context: &wire::HeightContext,
        label: &[u8],
    ) -> DurableV2PredecessorIdentity {
        DurableV2PredecessorIdentity::for_test(context.height, label)
    }

    fn test_successor_authority(
        predecessor: DurableV2PredecessorIdentity,
        successor_context_id: wire::HeightContextId,
    ) -> DurableSuccessorActivationAuthority {
        DurableSuccessorActivationAuthority::for_test(predecessor, successor_context_id)
    }

    fn valid_ingress_probe() -> BlockMessage {
        let validator = PeerId::new(
            KeyPair::try_from_seed(vec![0xD7; 32], Algorithm::BlsNormal)
                .expect("deterministic ingress probe key")
                .public_key()
                .clone(),
        );
        super::super::v2_worker::tests::lane_commit_qc_block_message(validator)
    }

    fn runner_status(context: &wire::HeightContext) -> wire::SumeragiV2Status {
        wire::SumeragiV2Status {
            protocol_version: wire::PROTOCOL_VERSION,
            node_fingerprint: Hash::new(b"runner status node"),
            build_fingerprint: Hash::new(b"runner status build"),
            config_fingerprint: Hash::new(b"runner status config"),
            restart_required: false,
            height_context_id: context.id(),
            height: context.height,
            view: 0,
            phase: wire::SumeragiV2StatusPhase::AwaitingProposal,
            leader: context.leader(0),
            locked_prepare_qc: None,
            highest_prepare_qc: None,
            last_timeout_certificate: None,
            body_state: wire::SumeragiV2BodyState::Missing,
            pending_persistence_id: None,
            last_committed_height: context.height.saturating_sub(1),
            last_committed_subject: None,
            height_context: wire::SumeragiV2HeightContextStatus {
                epoch: context.epoch,
                epoch_end_height: context.epoch_end_height,
                mode: context.mode,
                epoch_seed: context.leader_seed,
                validator_count: u32::try_from(context.roster.len()).expect("validator count"),
                quorum: context.quorum,
            },
            last_commit_qc: None,
            liveness: Default::default(),
        }
    }

    fn publish_applied_runner_status(context: &wire::HeightContext) {
        let mut status = runner_status(context);
        status.phase = wire::SumeragiV2StatusPhase::PendingApply;
        status.body_state = wire::SumeragiV2BodyState::Applied;
        status.liveness.generation = context.height;
        status.liveness.work.application = wire::SumeragiV2LocalWorkStage::Complete;
        status.liveness.work.successor_height = wire::SumeragiV2LocalWorkStage::Queued;
        status.liveness.last_progress = Some(wire::SumeragiV2ProgressTransitionStatus {
            generation: context.height,
            round: wire::ConsensusRound {
                context_id: context.id(),
                height: context.height,
                view: 0,
            },
            transition: wire::SumeragiV2ProgressTransition::Applied,
            age_ms: 0,
        });
        super::super::status::set_v2_status(status);
    }

    fn labelled_lane_qc_message(peer: PeerId, label: &[u8]) -> BlockMessage {
        let mut message = super::super::v2_worker::tests::lane_commit_qc_block_message(peer);
        let BlockMessage::LaneBlockQc(qc) = &mut message else {
            unreachable!("lane-QC fixture must return a lane CommitQC")
        };
        qc.body.proposal_hash = Hash::new(label);
        message
    }

    fn lane_qc_label(message: &NetworkMessage) -> Hash {
        let NetworkMessage::SumeragiBlock(wire) = message else {
            panic!("runner scheduler fixture emitted a non-block network message")
        };
        let BlockMessage::LaneBlockQc(qc) = wire.as_message() else {
            panic!("runner scheduler fixture emitted a non-lane-QC block message")
        };
        qc.body.proposal_hash.clone()
    }

    fn runner_sidecar_chunk(
        local: PeerId,
        requester: PeerId,
        label: &[u8],
    ) -> CertifiedMergeSidecarChunkV1 {
        CertifiedMergeSidecarChunkV1 {
            version: CERTIFIED_MERGE_SIDECAR_VERSION_V1,
            service_generation: CertifiedMergeSidecarServiceGenerationV1::INITIAL,
            stream_epoch: CertifiedMergeSidecarStreamEpochV1(
                NonZeroU64::new(1).expect("runner sidecar stream epoch is non-zero"),
            ),
            semantic_sequence: CertifiedMergeSidecarSemanticSequenceV1(
                NonZeroU64::new(1).expect("runner semantic sequence is non-zero"),
            ),
            request_id: Hash::new(label),
            entry_hash: HashOf::from_untyped_unchecked(Hash::new(b"runner sidecar entry")),
            encoded_len: 4,
            epoch_id: 7,
            reference_digest: Hash::new(b"runner sidecar reference"),
            requester,
            responder: local,
            chunk_index: 0,
            chunk_count: 1,
            bytes: vec![1, 2, 3, 4],
        }
    }

    #[test]
    fn reserved_lane_output_bypasses_unserviceable_head_without_losing_owner() {
        let (mut services, keys) = super::super::v2_worker::tests::fixture();
        services
            .set_exact_output_shared_unit_capacity_for_test(1)
            .expect("install one shared slot plus frozen-validator reservations");
        let blocked = PeerId::new(keys[1].public_key().clone());
        let responsive = PeerId::new(keys[2].public_key().clone());
        let keep_blocked = Arc::new(AtomicBool::new(true));
        let keep_blocked_for_hook = Arc::clone(&keep_blocked);
        let blocked_for_hook = blocked.clone();
        let admitted = Arc::new(Mutex::new(Vec::new()));
        let admitted_for_hook = Arc::clone(&admitted);
        services.set_exact_output_admission_hook(move |post, ticket| {
            if post.peer_id == blocked_for_hook && keep_blocked_for_hook.load(Ordering::Acquire) {
                return Err(NetworkActorAdmissionError::Backpressured {
                    message: post,
                    ticket,
                    rank: 13,
                });
            }
            admitted_for_hook
                .lock()
                .expect("record admitted lane output")
                .push((post.peer_id.clone(), lane_qc_label(&post.data)));
            Ok(())
        });

        let reserved_filler = Hash::new(b"runner reserved filler");
        let shared_filler = Hash::new(b"runner shared filler");
        for (label, expected) in [
            (
                b"runner reserved filler".as_slice(),
                reserved_filler.clone(),
            ),
            (b"runner shared filler".as_slice(), shared_filler.clone()),
        ] {
            services
                .post_lane_block(
                    blocked.clone(),
                    labelled_lane_qc_message(blocked.clone(), label),
                )
                .expect("blocked validator output remains exactly owned");
            assert!(
                services
                    .has_pending_exact_output()
                    .expect("inspect blocked exact output")
            );
            assert!(
                admitted
                    .lock()
                    .expect("inspect admitted output")
                    .iter()
                    .all(|(_, actual)| actual != &expected)
            );
        }

        let blocked_label = Hash::new(b"runner blocked effect A");
        let responsive_label = Hash::new(b"runner reserved effect B");
        let blocked_effect = V2LaneWorkEffect::PostLaneBlock {
            peer: blocked.clone(),
            message: labelled_lane_qc_message(blocked.clone(), b"runner blocked effect A"),
        };
        let responsive_effect = V2LaneWorkEffect::PostLaneBlock {
            peer: responsive.clone(),
            message: labelled_lane_qc_message(responsive.clone(), b"runner reserved effect B"),
        };
        let (mut lane_work, _) =
            super::super::v2_lane_work::tests::fixture(wire::ConsensusMode::Permissioned);
        assert!(lane_work.requeue_effect(blocked_effect));
        assert!(lane_work.requeue_effect(responsive_effect));

        dispatch_lane_work_effects(&mut lane_work, &services, 1)
            .expect("reserved work bypasses the unserviceable head");
        assert_eq!(lane_work.effect_count(), 1);
        match lane_work.next_effect() {
            Some(V2LaneWorkEffect::PostLaneBlock {
                peer,
                message: BlockMessage::LaneBlockQc(qc),
            }) => {
                assert_eq!(peer, blocked);
                assert_eq!(qc.body.proposal_hash, blocked_label);
            }
            other => panic!("blocked effect A must remain the exact queued owner: {other:?}"),
        }
        {
            let admitted = admitted.lock().expect("inspect admitted output");
            assert_eq!(
                admitted
                    .iter()
                    .filter(|(peer, label)| peer == &responsive && label == &responsive_label)
                    .count(),
                1
            );
            assert!(admitted.iter().all(|(_, label)| label != &blocked_label));
        }

        keep_blocked.store(false, Ordering::Release);
        assert!(
            !services
                .retry_pending_exact_output()
                .expect("responsive retry drains both retained fillers")
        );
        dispatch_lane_work_effects(&mut lane_work, &services, 1)
            .expect("the retained head dispatches after capacity reopens");
        assert_eq!(lane_work.effect_count(), 0);
        assert!(
            !services
                .has_pending_exact_output()
                .expect("all exact lane output is admitted")
        );
        let admitted = admitted.lock().expect("inspect final admitted output");
        for (peer, label) in [
            (&blocked, &reserved_filler),
            (&blocked, &shared_filler),
            (&blocked, &blocked_label),
            (&responsive, &responsive_label),
        ] {
            assert_eq!(
                admitted
                    .iter()
                    .filter(|(actual_peer, actual_label)| {
                        actual_peer == peer && actual_label == label
                    })
                    .count(),
                1,
                "each semantic output must be admitted exactly once"
            );
        }
        assert_eq!(admitted.len(), 4);
    }

    #[test]
    fn runner_dispatch_preserves_durable_lane_certificate_reply_routes() {
        let history = super::super::v2_lane_work::tests::durable_lane_history_fixture();
        let requester = history
            .certificate
            .commit_qc
            .validator_set
            .iter()
            .find(|peer| peer.public_key() != history.validators[0].public_key())
            .cloned()
            .expect("durable lane fixture has a remote requester");
        let mut services = super::super::v2_worker::tests::service_for_history_context(
            Arc::clone(&history.kura),
            history.context,
            &history.validators,
        );
        let dispatch_attempts = Arc::new(AtomicUsize::new(0));
        let dispatch_attempts_for_hook = Arc::clone(&dispatch_attempts);
        services.set_exact_output_admission_hook(move |post, ticket| {
            dispatch_attempts_for_hook.fetch_add(1, Ordering::Relaxed);
            Err(NetworkActorAdmissionError::Backpressured {
                message: post,
                ticket,
                rank: 1,
            })
        });
        let hub_a = PeerId::new(KeyPair::random().public_key().clone());
        let hub_b = PeerId::new(KeyPair::random().public_key().clone());
        let mut route_fixture =
            NetworkReplyRouteTestFixture::with_source_capacity(hub_a.clone(), 2);
        let route_a = route_fixture.mint_via(requester.clone(), hub_a.clone());
        let route_b = route_fixture.mint_via(requester.clone(), hub_b.clone());
        assert!(route_a.source_key() != route_b.source_key());

        let mut reply_routes = NetworkReplyRoutes::try_from_route(route_a.clone())
            .expect("first authenticated durable-response source");
        reply_routes
            .merge(
                &NetworkReplyRoutes::try_from_route(route_b.clone())
                    .expect("second authenticated durable-response source"),
            )
            .expect("attach the independent durable-response source");

        let mut admitted_a = super::super::fair_v2_ingress_admit_for_test(
            InboundBlockMessage::try_from_transport_with_reply_route(
                BlockMessage::LaneBlockProposal(history.certificate.proposal.clone()),
                requester.clone(),
                hub_a,
                route_a.clone(),
            )
            .expect("first durable request route binds its fair-ingress occurrence"),
        );
        let mut ingress_ownership = admitted_a
            .take_ingress_ownership()
            .expect("fair ingress supplies first exact durable-request ownership");
        let mut admitted_b = super::super::fair_v2_ingress_admit_for_test(
            InboundBlockMessage::try_from_transport_with_reply_route(
                BlockMessage::LaneBlockProposal(history.certificate.proposal.clone()),
                requester.clone(),
                hub_b,
                route_b.clone(),
            )
            .expect("second durable request route binds its fair-ingress occurrence"),
        );
        assert!(
            ingress_ownership.merge_downstream(
                admitted_b
                    .take_ingress_ownership()
                    .expect("fair ingress supplies second exact durable-request ownership")
            ),
            "independent authenticated sources merge under one semantic request identity"
        );
        assert!(ingress_ownership.validate_exact());
        assert!(ingress_ownership.matches_reply_routes(Some(&reply_routes)));

        let mut effect = V2LaneWorkEffect::PostDurableLaneCertificate {
            peer: requester,
            reply_routes: Some(reply_routes),
            ingress_ownership: Some(ingress_ownership),
            certificate: history.certificate,
        };
        assert!(retain_active_owned_reply_routes_after_snapshot(
            &mut effect,
            || assert!(route_fixture.retire(&route_a))
        ));
        assert!(!route_a.is_active());
        assert!(route_b.is_active());
        match &effect {
            V2LaneWorkEffect::PostDurableLaneCertificate {
                reply_routes: Some(routes),
                ingress_ownership: Some(ownership),
                ..
            } => {
                assert_eq!(routes.len(), 2);
                assert!(routes.iter().any(|route| route.same_delivery(&route_a)));
                assert!(routes.iter().any(|route| route.same_delivery(&route_b)));
                assert!(ownership.validate_exact());
                assert!(ownership.matches_reply_routes(Some(routes)));
            }
            other => panic!("durable response lost exact route ownership: {other:?}"),
        }

        dispatch_lane_work_effect(&services, effect)
            .expect("runner hands the Kura-backed certificate to exact output");
        assert!(
            !services
                .retains_reply_route_for_test(&route_a)
                .expect("inspect retired durable certificate route")
        );
        assert!(
            services
                .retains_reply_route_for_test(&route_b)
                .expect("inspect retained sibling durable certificate route")
        );
        assert_eq!(
            dispatch_attempts.load(Ordering::Relaxed),
            1,
            "only the responsive authenticated source may reach exact-output dispatch"
        );
    }

    #[test]
    fn runner_dispatch_preserves_certified_sidecar_chunk_reply_routes() {
        let (mut services, keys) = super::super::v2_worker::tests::fixture();
        services.set_exact_output_admission_hook(|post, ticket| {
            Err(NetworkActorAdmissionError::Backpressured {
                message: post,
                ticket,
                rank: 1,
            })
        });
        let local = PeerId::new(keys[0].public_key().clone());
        let requester = PeerId::new(keys[1].public_key().clone());
        let hub = PeerId::new(KeyPair::random().public_key().clone());
        let mut route_fixture = NetworkReplyRouteTestFixture::new(hub);
        let route = route_fixture.mint(requester.clone());
        let reply_routes =
            NetworkReplyRoutes::try_from_route(route.clone()).expect("live reply route set");
        let chunk = runner_sidecar_chunk(local, requester.clone(), b"runner sidecar request");

        dispatch_lane_work_effect(
            &services,
            V2LaneWorkEffect::PostCertifiedMergeSidecar {
                peer: requester,
                reply_routes: Some(reply_routes),
                message: Arc::new(CertifiedMergeSidecarMessage::Chunk(chunk)),
            },
        )
        .expect("runner hands the certified chunk to exact output");
        assert!(
            services
                .retains_reply_route_for_test(&route)
                .expect("inspect retained sidecar route")
        );
    }

    #[test]
    fn direct_close_ack_retains_reply_route_from_lane_through_worker() {
        let super::super::v2_lane_work::tests::CertifiedSidecarServerFixture {
            mut adapter,
            validators,
            kura,
            context,
            local_validator,
            requester,
            request,
        } = super::super::v2_lane_work::tests::certified_sidecar_server_fixture();
        let mut services =
            super::super::v2_worker::tests::service_for_history_context_with_local_validator(
                kura,
                context,
                &validators,
                local_validator,
            );
        services.set_exact_output_admission_hook(|post, ticket| {
            Err(NetworkActorAdmissionError::Backpressured {
                message: post,
                ticket,
                rank: 1,
            })
        });
        let mut routes = NetworkReplyRouteTestFixture::new(requester.clone());
        let reply_route = routes.mint(requester.clone());
        let mut close = CertifiedMergeSidecarCloseV1 {
            version: CERTIFIED_MERGE_SIDECAR_VERSION_V1,
            service_generation: request.service_generation,
            stream_epoch: request.stream_epoch,
            closed_through: request.semantic_sequence.get(),
            close_id: Hash::prehashed([0; Hash::LENGTH]),
            requester: requester.clone(),
            responder: request.responder,
        };
        close.close_id = close.canonical_close_id();

        assert_eq!(
            adapter.accept_relay_message(
                LaneRelayMessage::CertifiedMergeSidecar {
                    sender: requester,
                    reply_route: Some(reply_route.clone()),
                    message: CertifiedMergeSidecarMessage::Close(close),
                },
                0,
            ),
            V2LaneIngressOutcome::Inserted
        );
        assert!(matches!(
            adapter.next_effect(),
            Some(V2LaneWorkEffect::PostCertifiedMergeSidecar {
                reply_routes: Some(reply_routes),
                message,
                ..
            }) if reply_routes
                .iter()
                .any(|route| route.same_delivery(&reply_route))
                && matches!(
                    message.as_ref(),
                    CertifiedMergeSidecarMessage::CloseAck(_)
                )
        ));
        let mut malformed = adapter
            .next_effect()
            .expect("the direct CloseAck remains lane-owned before dispatch");
        let V2LaneWorkEffect::PostCertifiedMergeSidecar { reply_routes, .. } = &mut malformed
        else {
            unreachable!("the direct CloseAck keeps its sidecar effect kind")
        };
        *reply_routes = None;
        assert!(
            dispatch_lane_work_effect(&services, malformed)
                .expect_err("a responder control without its return route is malformed")
                .to_string()
                .contains("reply-route ownership")
        );

        dispatch_lane_work_effects(&mut adapter, &services, 1)
            .expect("dispatch the direct CloseAck through exact output");
        assert_eq!(adapter.effect_count(), 0);
        assert!(
            services
                .retains_reply_route_for_test(&reply_route)
                .expect("inspect direct CloseAck route in worker ownership")
        );
    }

    #[test]
    fn closed_sidecar_prefix_handoff_requeues_only_failed_suffix() {
        let fixture = super::super::v2_lane_work::tests::certified_sidecar_server_fixture();
        let mut adapter = fixture.adapter;
        let responder = fixture.request.responder.clone();
        let second_requester = fixture
            .context
            .roster
            .iter()
            .map(|entry| entry.validator.clone())
            .find(|peer| peer != &responder && peer != &fixture.requester)
            .expect("runner prefix retry fixture has a second remote requester");
        let mut second_request = fixture.request.clone();
        second_request.requester = second_requester;
        second_request.request_id = second_request.canonical_request_id();
        let requests = [fixture.request, second_request];
        let hub = PeerId::new(KeyPair::random().public_key().clone());
        // The shared lane fixture reserves the production test corridor for
        // eight authenticated sources. Each capability must advertise that
        // exact geometry even though this case exercises one source.
        let mut routes = NetworkReplyRouteTestFixture::new(hub.clone());

        for request in &requests {
            let reply_route = routes.mint_via(request.requester.clone(), hub.clone());
            assert_eq!(
                adapter
                    .accept_certified_merge_sidecar_for_test(
                        request.requester.clone(),
                        reply_route.clone(),
                        request.clone(),
                    )
                    .expect("admit the exact Kura-backed request before closing it"),
                V2LaneIngressOutcome::Inserted
            );
            let mut close = CertifiedMergeSidecarCloseV1 {
                version: CERTIFIED_MERGE_SIDECAR_VERSION_V1,
                service_generation: request.service_generation,
                stream_epoch: request.stream_epoch,
                closed_through: request.semantic_sequence.get(),
                close_id: Hash::prehashed([0; Hash::LENGTH]),
                requester: request.requester.clone(),
                responder: responder.clone(),
            };
            close.close_id = close.canonical_close_id();
            assert_eq!(
                adapter.accept_relay_message(
                    LaneRelayMessage::CertifiedMergeSidecar {
                        sender: request.requester.clone(),
                        reply_route: Some(reply_route),
                        message: CertifiedMergeSidecarMessage::Close(close),
                    },
                    0,
                ),
                V2LaneIngressOutcome::Inserted
            );
        }

        let mut first_applied = Vec::new();
        let mut calls = 0usize;
        let error = apply_certified_merge_sidecar_closed_prefixes_with(&mut adapter, |prefix| {
            calls = calls.saturating_add(1);
            if calls == 2 {
                Err("injected exact-output close failure".to_owned())
            } else {
                first_applied.push(prefix.clone());
                Ok(())
            }
        })
        .expect_err("the second exact-output close fails");
        assert!(matches!(
            error,
            V2RunnerError::Service(ref reason)
                if reason == "injected exact-output close failure"
        ));
        assert_eq!(first_applied.len(), 1);

        let mut retry_applied = Vec::new();
        apply_certified_merge_sidecar_closed_prefixes_with(&mut adapter, |prefix| {
            retry_applied.push(prefix.clone());
            Ok(())
        })
        .expect("retry applies the retained failed suffix");
        assert_eq!(retry_applied.len(), 1);
        assert_ne!(
            retry_applied[0], first_applied[0],
            "the already-applied prefix must not be repeated"
        );
        let applied_requesters = first_applied
            .iter()
            .chain(&retry_applied)
            .map(|prefix| prefix.requester.clone())
            .collect::<BTreeSet<_>>();
        let requesters = requests
            .into_iter()
            .map(|request| request.requester)
            .collect::<BTreeSet<_>>();
        assert_eq!(
            applied_requesters, requesters,
            "the successful prefix plus the retried suffix cover the exact drained batch"
        );

        apply_certified_merge_sidecar_closed_prefixes_with(&mut adapter, |_| {
            panic!("a confirmed handoff must leave no prefix for another retry")
        })
        .expect("the confirmed handoff is empty");
    }

    #[test]
    fn relayed_generation_hint_preserves_reply_route_from_lane_through_worker() {
        let super::super::v2_lane_work::tests::CertifiedSidecarServerFixture {
            mut adapter,
            validators,
            kura,
            context,
            local_validator,
            requester,
            request,
        } = super::super::v2_lane_work::tests::certified_sidecar_server_fixture();
        let changed_roster =
            super::super::v2_lane_work::tests::changed_merge_sidecar_server_roster();
        adapter
            .transition_merge_sidecar_responder_roster_for_test(&changed_roster)
            .expect("advance the quiescent responder generation");
        let mut services =
            super::super::v2_worker::tests::service_for_history_context_with_local_validator(
                kura,
                context,
                &validators,
                local_validator,
            );
        services.set_exact_output_admission_hook(|post, ticket| {
            Err(NetworkActorAdmissionError::Backpressured {
                message: post,
                ticket,
                rank: 1,
            })
        });
        let hub_a = PeerId::new(KeyPair::random().public_key().clone());
        let hub_b = PeerId::new(KeyPair::random().public_key().clone());
        let mut routes = NetworkReplyRouteTestFixture::new(hub_a.clone());
        let route_a = routes.mint_via(requester.clone(), hub_a);
        let route_b = routes.mint_via(requester.clone(), hub_b);

        assert_eq!(
            adapter.accept_relay_message(
                LaneRelayMessage::CertifiedMergeSidecar {
                    sender: requester.clone(),
                    reply_route: Some(route_a.clone()),
                    message: CertifiedMergeSidecarMessage::Request(request.clone()),
                },
                0,
            ),
            V2LaneIngressOutcome::Inserted
        );
        assert_eq!(
            adapter.accept_relay_message(
                LaneRelayMessage::CertifiedMergeSidecar {
                    sender: requester.clone(),
                    reply_route: Some(route_b.clone()),
                    message: CertifiedMergeSidecarMessage::Request(request),
                },
                0,
            ),
            V2LaneIngressOutcome::Inserted,
            "an alternate authenticated delivery joins the same stateless Hint"
        );
        assert!(matches!(
            adapter.next_effect(),
            Some(V2LaneWorkEffect::PostCertifiedMergeSidecar {
                reply_routes: Some(reply_routes),
                message,
                ..
            }) if reply_routes.len() == 2
                && reply_routes
                    .iter()
                    .any(|route| route.same_delivery(&route_a))
                && reply_routes
                    .iter()
                    .any(|route| route.same_delivery(&route_b))
                && matches!(
                    message.as_ref(),
                    CertifiedMergeSidecarMessage::GenerationHint(_)
                )
        ));

        let mut malformed = adapter
            .next_effect()
            .expect("the routed GenerationHint remains lane-owned before dispatch");
        let V2LaneWorkEffect::PostCertifiedMergeSidecar { reply_routes, .. } = &mut malformed
        else {
            unreachable!("the GenerationHint keeps its sidecar effect kind")
        };
        *reply_routes = None;
        assert!(
            dispatch_lane_work_effect(&services, malformed)
                .expect_err("a GenerationHint without reply-route ownership is malformed")
                .to_string()
                .contains("reply-route ownership")
        );

        assert!(routes.retire(&route_a));
        dispatch_lane_work_effects(&mut adapter, &services, 1)
            .expect("dispatch the routed GenerationHint through exact output");
        assert_eq!(adapter.effect_count(), 0);
        assert!(
            !services
                .retains_reply_route_for_test(&route_a)
                .expect("the retired Hint source must not reach worker ownership")
        );
        assert!(
            services
                .retains_reply_route_for_test(&route_b)
                .expect("the live Hint sibling must remain in worker ownership")
        );
    }

    #[test]
    fn runner_dispatch_prunes_retired_sidecar_source_without_losing_live_sibling() {
        let (mut services, keys) = super::super::v2_worker::tests::fixture();
        services.set_exact_output_admission_hook(|post, ticket| {
            Err(NetworkActorAdmissionError::Backpressured {
                message: post,
                ticket,
                rank: 7,
            })
        });
        let local = PeerId::new(keys[0].public_key().clone());
        let requester = PeerId::new(keys[1].public_key().clone());
        let hub_a = PeerId::new(KeyPair::random().public_key().clone());
        let hub_b = PeerId::new(KeyPair::random().public_key().clone());
        let mut route_fixture =
            NetworkReplyRouteTestFixture::with_source_capacity(hub_a.clone(), 2);
        let route_a = route_fixture.mint_via(requester.clone(), hub_a);
        let route_b = route_fixture.mint_via(requester.clone(), hub_b);
        let mut reply_routes = NetworkReplyRoutes::try_from_route(route_a.clone())
            .expect("first sidecar response source");
        reply_routes
            .merge(
                &NetworkReplyRoutes::try_from_route(route_b.clone())
                    .expect("second sidecar response source"),
            )
            .expect("attach independent sidecar response source");
        let effect = V2LaneWorkEffect::PostCertifiedMergeSidecar {
            peer: requester.clone(),
            reply_routes: Some(reply_routes),
            message: Arc::new(CertifiedMergeSidecarMessage::Chunk(runner_sidecar_chunk(
                local,
                requester,
                b"runner prune retired source",
            ))),
        };
        let (mut lane_work, _) =
            super::super::v2_lane_work::tests::fixture(wire::ConsensusMode::Permissioned);
        assert!(lane_work.requeue_effect(effect));
        assert!(route_fixture.retire(&route_a));

        dispatch_lane_work_effects(&mut lane_work, &services, 1)
            .expect("a retired owned route cannot poison its live sibling");
        assert_eq!(lane_work.effect_count(), 0);
        assert!(
            !services
                .retains_reply_route_for_test(&route_a)
                .expect("inspect retired route ownership")
        );
        assert!(
            services
                .retains_reply_route_for_test(&route_b)
                .expect("inspect live sibling ownership")
        );
    }

    #[test]
    fn runner_preflight_enqueue_race_retains_sidecar_source_until_capacity_reopens() {
        let fixture = super::super::v2_lane_work::tests::certified_sidecar_server_fixture();
        let local_peer = fixture.request.responder.clone();
        let mut lane_work = fixture.adapter;
        let mut services =
            super::super::v2_worker::tests::service_for_history_context_with_local_validator(
                fixture.kura,
                fixture.context,
                &fixture.validators,
                fixture.local_validator,
            );
        services
            .set_exact_output_shared_unit_capacity_for_test(1)
            .expect("install one shared race slot plus frozen target reservations");

        let hub_a = PeerId::new(KeyPair::random().public_key().clone());
        let hub_b = PeerId::new(KeyPair::random().public_key().clone());
        let hub_c = PeerId::new(KeyPair::random().public_key().clone());
        // The lane fixture's production seam is configured for eight
        // authenticated reply sources. Route capabilities advertise that
        // exact geometry, even though this race uses only three sources.
        let mut routes = NetworkReplyRouteTestFixture::new(hub_a.clone());
        let actual_route = routes.mint_via(fixture.requester.clone(), hub_a);
        assert_eq!(
            lane_work
                .accept_certified_merge_sidecar_for_test(
                    fixture.requester.clone(),
                    actual_route.clone(),
                    fixture.request,
                )
                .expect("materialize the source-owned sidecar effect"),
            V2LaneIngressOutcome::Inserted
        );
        let actual_effect = lane_work
            .next_effect()
            .expect("materialized chunk enters the lane queue");
        assert!(
            services
                .can_retain_lane_work_effect(&actual_effect)
                .expect("race preflight validates the empty corridor")
        );
        let _ = lane_work
            .drain_effects(1)
            .pop()
            .expect("take the exact effect after successful preflight");

        services.set_exact_output_admission_hook(|post, ticket| {
            Err(NetworkActorAdmissionError::Backpressured {
                message: post,
                ticket,
                rank: 5,
            })
        });
        let filler_a = routes.mint_via(fixture.requester.clone(), hub_b);
        let filler_b = routes.mint_via(fixture.requester.clone(), hub_c);
        for (route, label) in [
            (filler_a.clone(), b"runner race filler A".as_slice()),
            (filler_b.clone(), b"runner race filler B".as_slice()),
        ] {
            let reply_routes =
                NetworkReplyRoutes::try_from_route(route).expect("live filler source route");
            assert!(matches!(
                dispatch_lane_work_effect(
                    &services,
                    V2LaneWorkEffect::PostCertifiedMergeSidecar {
                        peer: fixture.requester.clone(),
                        reply_routes: Some(reply_routes),
                        message: Arc::new(CertifiedMergeSidecarMessage::Chunk(
                            runner_sidecar_chunk(
                                local_peer.clone(),
                                fixture.requester.clone(),
                                label,
                            )
                        )),
                    },
                )
                .expect("fill exact target/class ownership after preflight"),
                LaneWorkEffectDispatch::Complete
            ));
        }

        let retained_effect = match dispatch_lane_work_effect(&services, actual_effect)
            .expect("the enqueue race is bounded source backpressure")
        {
            LaneWorkEffectDispatch::SourceRetained(effect) => effect,
            LaneWorkEffectDispatch::Complete => {
                panic!("post-preflight capacity race must return the exact source owner")
            }
        };
        assert!(!services.exact_output_restart_required_for_test());

        let filler_controls = Arc::new(Mutex::new(Vec::new()));
        let filler_controls_for_hook = Arc::clone(&filler_controls);
        let mut filler_routes = vec![
            (Hash::new(b"runner race filler A"), filler_a),
            (Hash::new(b"runner race filler B"), filler_b),
        ];
        services.set_exact_output_flush_admission_hook(move |post, ticket| {
            assert!(ticket.is_none());
            let request_id = match &post.data {
                NetworkMessage::CertifiedMergeSidecar(message) => match message.as_ref() {
                    CertifiedMergeSidecarMessage::Chunk(chunk) => &chunk.request_id,
                    CertifiedMergeSidecarMessage::Request(_)
                    | CertifiedMergeSidecarMessage::Close(_)
                    | CertifiedMergeSidecarMessage::CloseAck(_)
                    | CertifiedMergeSidecarMessage::GenerationHint(_) => {
                        panic!("retained sidecar filler changed into a request")
                    }
                },
                _ => panic!("retained sidecar filler changed its network message kind"),
            };
            let index = filler_routes
                .iter()
                .position(|(expected, _)| expected == request_id)
                .expect("retained filler preserves its immutable request identity");
            let (_, route) = filler_routes.remove(index);
            let (control, ack) = NetworkReplyFlushAckTestFixture::for_reply(&post, &route);
            filler_controls_for_hook
                .lock()
                .expect("retain filler writer controls")
                .push(control);
            Ok(super::super::v2_worker::ExactOutputTestAdmission::SidecarFlush(ack))
        });
        services
            .retry_pending_exact_output()
            .expect("responsive filler writers release exact ownership capacity");
        {
            let mut controls = filler_controls.lock().expect("flush filler controls");
            assert_eq!(controls.len(), 2);
            for control in controls.iter_mut() {
                assert!(control.flush());
            }
        }
        assert!(
            services
                .retry_pending_exact_output()
                .expect("flushed filler receipts remain owned until lane delivery")
        );
        assert_eq!(
            services
                .drain_certified_merge_sidecar_chunk_admissions(2)
                .expect("drain both successfully flushed filler receipts")
                .len(),
            2
        );
        assert!(
            !services
                .has_pending_exact_output()
                .expect("drained filler receipts release their exact ownership")
        );

        let actual_admissions = Arc::new(AtomicUsize::new(0));
        let actual_admissions_for_hook = Arc::clone(&actual_admissions);
        let actual_route_for_hook = actual_route.clone();
        let actual_control = Arc::new(Mutex::new(None));
        let actual_control_for_hook = Arc::clone(&actual_control);
        services.set_exact_output_flush_admission_hook(move |post, ticket| {
            assert!(ticket.is_none());
            actual_admissions_for_hook.fetch_add(1, Ordering::Relaxed);
            let (control, ack) =
                NetworkReplyFlushAckTestFixture::for_reply(&post, &actual_route_for_hook);
            assert!(
                actual_control_for_hook
                    .lock()
                    .expect("retain actual writer control")
                    .replace(control)
                    .is_none()
            );
            Ok(super::super::v2_worker::ExactOutputTestAdmission::SidecarFlush(ack))
        });
        assert!(lane_work.requeue_effect(retained_effect));
        dispatch_lane_work_effects(&mut lane_work, &services, 1)
            .expect("retained source dispatches after capacity reopens");
        assert_eq!(lane_work.effect_count(), 0);
        assert_eq!(actual_admissions.load(Ordering::Relaxed), 1);
        assert!(
            actual_control
                .lock()
                .expect("lock actual writer control")
                .as_mut()
                .expect("actual source reached writer admission")
                .flush()
        );
        dispatch_lane_work_effects(&mut lane_work, &services, 1)
            .expect("actual writer flush advances the source exactly once");
        assert_eq!(actual_admissions.load(Ordering::Relaxed), 1);
        assert!(
            !services
                .has_pending_exact_output()
                .expect("actual source receipt is fully applied")
        );
        assert!(!services.exact_output_restart_required_for_test());
    }

    #[test]
    fn runner_dispatch_advances_certified_sidecar_only_after_writer_flush() {
        let (mut services, keys) = super::super::v2_worker::tests::fixture();
        let local = PeerId::new(keys[0].public_key().clone());
        let requester = PeerId::new(keys[1].public_key().clone());
        let hub = PeerId::new(KeyPair::random().public_key().clone());
        let mut route_fixture = NetworkReplyRouteTestFixture::new(hub);
        let route = route_fixture.mint(requester.clone());
        let route_for_hook = route.clone();
        let flush_control = Arc::new(Mutex::new(None));
        let flush_control_for_hook = Arc::clone(&flush_control);
        services.set_exact_output_flush_admission_hook(move |post, _| {
            let (control, flush_ack) =
                NetworkReplyFlushAckTestFixture::for_reply(&post, &route_for_hook);
            assert!(
                flush_control_for_hook
                    .lock()
                    .expect("lock exact test writer-flush control")
                    .replace(control)
                    .is_none(),
                "one sidecar occurrence owns one exact writer-flush control"
            );
            Ok(super::super::v2_worker::ExactOutputTestAdmission::SidecarFlush(flush_ack))
        });
        let reply_routes = NetworkReplyRoutes::try_from_route(route).expect("live reply route set");
        let chunk =
            runner_sidecar_chunk(local, requester.clone(), b"runner admitted sidecar request");

        dispatch_lane_work_effect(
            &services,
            V2LaneWorkEffect::PostCertifiedMergeSidecar {
                peer: requester,
                reply_routes: Some(reply_routes),
                message: Arc::new(CertifiedMergeSidecarMessage::Chunk(chunk)),
            },
        )
        .expect("runner hands the certified chunk to exact output");
        assert!(
            services
                .has_pending_exact_output()
                .expect("receipt remains process-locally owned")
        );
        assert!(
            services
                .retry_pending_exact_output()
                .expect("pending writer completion remains visible to retry callers")
        );
        assert!(
            services
                .drain_certified_merge_sidecar_chunk_admissions(2)
                .expect("poll pending sidecar writer completion")
                .is_empty(),
            "actor admission alone must not advance the source cursor"
        );
        assert!(
            flush_control
                .lock()
                .expect("lock exact test writer-flush control")
                .as_mut()
                .expect("runner admission minted the exact writer-flush control")
                .flush()
        );
        assert!(
            services
                .retry_pending_exact_output()
                .expect("writer-flushed receipt remains owned until lane application")
        );
        assert_eq!(
            services
                .drain_certified_merge_sidecar_chunk_admissions(2)
                .expect("drain writer-flushed sidecar receipt")
                .len(),
            1
        );
        assert!(
            !services
                .has_pending_exact_output()
                .expect("receipt ownership is released after drain")
        );

        let (mut closed_services, keys) = super::super::v2_worker::tests::fixture();
        let local = PeerId::new(keys[0].public_key().clone());
        let requester = PeerId::new(keys[1].public_key().clone());
        let hub = PeerId::new(KeyPair::random().public_key().clone());
        let mut route_fixture = NetworkReplyRouteTestFixture::new(hub);
        let route = route_fixture.mint(requester.clone());
        let route_for_hook = route.clone();
        let close_control = Arc::new(Mutex::new(None));
        let close_control_for_hook = Arc::clone(&close_control);
        closed_services.set_exact_output_flush_admission_hook(move |post, _| {
            let (control, close_ack) =
                NetworkReplyFlushAckTestFixture::for_reply(&post, &route_for_hook);
            assert!(
                close_control_for_hook
                    .lock()
                    .expect("lock exact test closed control")
                    .replace(control)
                    .is_none(),
                "one closed occurrence owns one exact writer-flush control"
            );
            Ok(super::super::v2_worker::ExactOutputTestAdmission::SidecarFlush(close_ack))
        });
        let reply_routes =
            NetworkReplyRoutes::try_from_route(route).expect("live closed-path reply route set");
        dispatch_lane_work_effect(
            &closed_services,
            V2LaneWorkEffect::PostCertifiedMergeSidecar {
                peer: requester.clone(),
                reply_routes: Some(reply_routes),
                message: Arc::new(CertifiedMergeSidecarMessage::Chunk(runner_sidecar_chunk(
                    local,
                    requester,
                    b"runner closed sidecar request",
                ))),
            },
        )
        .expect("runner retains a second sidecar completion");
        assert!(
            close_control
                .lock()
                .expect("lock exact test closed control")
                .as_mut()
                .expect("runner admission minted the exact closed control")
                .close()
        );
        assert!(
            closed_services
                .drain_certified_merge_sidecar_chunk_admissions(2)
                .expect("closed writer completion is harmless")
                .is_empty(),
            "closed writer ownership must never produce a cursor receipt"
        );
        assert!(
            closed_services
                .has_pending_exact_output()
                .expect("closed completion retains the current item for exact retry"),
            "a closed writer acknowledgement cannot erase an active source's current item"
        );
    }

    #[test]
    fn runner_dispatch_retired_admission_race_emits_no_sidecar_receipt() {
        let (mut services, keys) = super::super::v2_worker::tests::fixture();
        let local = PeerId::new(keys[0].public_key().clone());
        let requester = PeerId::new(keys[1].public_key().clone());
        let hub = PeerId::new(KeyPair::random().public_key().clone());
        let mut route_fixture = NetworkReplyRouteTestFixture::new(hub);
        let route = route_fixture.mint(requester.clone());
        let route_for_hook = route.clone();
        services.set_exact_output_flush_admission_hook(move |_, _| {
            assert!(route_fixture.retire(&route_for_hook));
            Ok(super::super::v2_worker::ExactOutputTestAdmission::Retired)
        });
        let reply_routes =
            NetworkReplyRoutes::try_from_route(route).expect("initially live reply route set");

        dispatch_lane_work_effect(
            &services,
            V2LaneWorkEffect::PostCertifiedMergeSidecar {
                peer: requester.clone(),
                reply_routes: Some(reply_routes),
                message: Arc::new(CertifiedMergeSidecarMessage::Chunk(runner_sidecar_chunk(
                    local,
                    requester,
                    b"runner retired admission race",
                ))),
            },
        )
        .expect("tenure cancellation retires only the exact occurrence");
        assert!(
            services
                .drain_certified_merge_sidecar_chunk_admissions(1)
                .expect("retired occurrence owns no receipt")
                .is_empty()
        );
        assert!(
            !services
                .has_pending_exact_output()
                .expect("retired occurrence releases worker ownership")
        );
    }

    #[test]
    fn runner_closed_sidecar_flush_reconnect_retries_same_chunk_then_advances_once() {
        let fixture = super::super::v2_lane_work::tests::certified_sidecar_server_fixture();
        let mut lane_work = fixture.adapter;
        let mut services =
            super::super::v2_worker::tests::service_for_history_context_with_local_validator(
                fixture.kura,
                fixture.context,
                &fixture.validators,
                fixture.local_validator,
            );
        let hub = PeerId::new(KeyPair::random().public_key().clone());
        let mut routes = NetworkReplyRouteTestFixture::new(hub);
        let first_route = routes.mint(fixture.requester.clone());
        assert_eq!(
            lane_work
                .accept_certified_merge_sidecar_for_test(
                    fixture.requester.clone(),
                    first_route.clone(),
                    fixture.request.clone(),
                )
                .expect("materialize the first Kura-backed chunk"),
            V2LaneIngressOutcome::Inserted
        );
        let first_message = match lane_work.next_effect() {
            Some(V2LaneWorkEffect::PostCertifiedMergeSidecar { message, .. })
                if matches!(message.as_ref(), CertifiedMergeSidecarMessage::Chunk(_)) =>
            {
                message
            }
            other => panic!("expected first Kura-backed sidecar chunk, got {other:?}"),
        };

        let first_route_for_hook = first_route.clone();
        let close_control = Arc::new(Mutex::new(None));
        let close_control_for_hook = Arc::clone(&close_control);
        services.set_exact_output_flush_admission_hook(move |post, _| {
            let (control, close_ack) =
                NetworkReplyFlushAckTestFixture::for_reply(&post, &first_route_for_hook);
            assert!(
                close_control_for_hook
                    .lock()
                    .expect("lock first exact sidecar control")
                    .replace(control)
                    .is_none(),
                "first chunk owns one exact writer-flush control"
            );
            Ok(super::super::v2_worker::ExactOutputTestAdmission::SidecarFlush(close_ack))
        });
        dispatch_lane_work_effects(&mut lane_work, &services, 1)
            .expect("dispatch first chunk without advancing its cursor");
        assert!(routes.retire(&first_route));
        let reconnected_route = routes.mint(fixture.requester.clone());
        assert_eq!(
            lane_work
                .accept_certified_merge_sidecar_for_test(
                    fixture.requester.clone(),
                    reconnected_route.clone(),
                    fixture.request,
                )
                .expect("reconnect rematerializes the retained current chunk"),
            V2LaneIngressOutcome::Inserted
        );
        match lane_work.next_effect() {
            Some(V2LaneWorkEffect::PostCertifiedMergeSidecar {
                reply_routes: Some(reply_routes),
                message,
                ..
            }) => {
                assert!(matches!(
                    message.as_ref(),
                    CertifiedMergeSidecarMessage::Chunk(_)
                ));
                assert_eq!(
                    message, first_message,
                    "reconnect must preserve the exact current chunk even when its bounded transport cache was rematerialized"
                );
                assert!(
                    reply_routes
                        .iter()
                        .any(|route| route.same_delivery(&reconnected_route))
                );
            }
            other => panic!("expected reconnected current sidecar chunk, got {other:?}"),
        }

        dispatch_lane_work_effects(&mut lane_work, &services, 1)
            .expect("the worker absorbs the reconnect into its retained source attempt");
        assert_eq!(
            lane_work.effect_count(),
            0,
            "the route capability transfers exactly once into worker ownership"
        );
        assert!(
            services
                .has_pending_exact_output()
                .expect("old writer flush remains process-locally owned")
        );

        let reconnected_route_for_hook = reconnected_route.clone();
        let flush_control = Arc::new(Mutex::new(None));
        let flush_control_for_hook = Arc::clone(&flush_control);
        services.set_exact_output_flush_admission_hook(move |post, _| {
            let (control, flush_ack) =
                NetworkReplyFlushAckTestFixture::for_reply(&post, &reconnected_route_for_hook);
            assert!(
                flush_control_for_hook
                    .lock()
                    .expect("lock reconnected exact sidecar control")
                    .replace(control)
                    .is_none(),
                "reconnected chunk owns one exact writer-flush control"
            );
            Ok(super::super::v2_worker::ExactOutputTestAdmission::SidecarFlush(flush_ack))
        });
        assert!(
            close_control
                .lock()
                .expect("lock first exact sidecar control")
                .as_mut()
                .expect("first chunk admission minted its exact closed control")
                .close()
        );
        retry_exact_output_and_apply_sidecar_admissions(&mut lane_work, &services, 1)
            .expect("closed old writer retries the retained current chunk on the reconnect");
        assert_eq!(lane_work.effect_count(), 0);
        assert!(
            flush_control
                .lock()
                .expect("lock reconnected exact sidecar control")
                .as_mut()
                .expect("reconnected admission minted its exact flush control")
                .flush()
        );
        retry_exact_output_and_apply_sidecar_admissions(&mut lane_work, &services, 1)
            .expect("writer flush advances exactly the reconnected source cursor");
        assert_eq!(lane_work.effect_count(), 0);
        assert!(
            !services
                .has_pending_exact_output()
                .expect("writer-flushed receipt is fully applied")
        );
    }

    #[test]
    fn runner_old_flushed_sidecar_receipt_cancels_queued_reconnect_retry() {
        let fixture = super::super::v2_lane_work::tests::certified_sidecar_server_fixture();
        let mut lane_work = fixture.adapter;
        let mut services =
            super::super::v2_worker::tests::service_for_history_context_with_local_validator(
                fixture.kura,
                fixture.context,
                &fixture.validators,
                fixture.local_validator,
            );
        let hub = PeerId::new(KeyPair::random().public_key().clone());
        let mut routes = NetworkReplyRouteTestFixture::new(hub);
        let first_route = routes.mint(fixture.requester.clone());
        assert_eq!(
            lane_work
                .accept_certified_merge_sidecar_for_test(
                    fixture.requester.clone(),
                    first_route.clone(),
                    fixture.request.clone(),
                )
                .expect("materialize the first exact sidecar chunk"),
            V2LaneIngressOutcome::Inserted
        );

        let first_route_for_hook = first_route.clone();
        let flush_control = Arc::new(Mutex::new(None));
        let flush_control_for_hook = Arc::clone(&flush_control);
        services.set_exact_output_flush_admission_hook(move |post, _| {
            let (control, ack) =
                NetworkReplyFlushAckTestFixture::for_reply(&post, &first_route_for_hook);
            assert!(
                flush_control_for_hook
                    .lock()
                    .expect("lock old exact writer control")
                    .replace(control)
                    .is_none()
            );
            Ok(super::super::v2_worker::ExactOutputTestAdmission::SidecarFlush(ack))
        });
        dispatch_lane_work_effects(&mut lane_work, &services, 1)
            .expect("dispatch the first route chunk");

        assert!(routes.retire(&first_route));
        let reconnected_route = routes.mint(fixture.requester.clone());
        assert_eq!(
            lane_work
                .accept_certified_merge_sidecar_for_test(
                    fixture.requester.clone(),
                    reconnected_route.clone(),
                    fixture.request,
                )
                .expect("reconnect retains the old current chunk"),
            V2LaneIngressOutcome::Inserted
        );
        dispatch_lane_work_effects(&mut lane_work, &services, 1)
            .expect("the worker absorbs the reconnect into its retained source attempt");
        assert_eq!(lane_work.effect_count(), 0);
        assert!(
            services
                .has_pending_exact_output()
                .expect("the worker owns the reconnect while the old flush is pending")
        );

        assert!(
            flush_control
                .lock()
                .expect("lock old exact writer control")
                .as_mut()
                .expect("first admission installed its writer control")
                .flush()
        );
        retry_exact_output_and_apply_sidecar_admissions(&mut lane_work, &services, 1)
            .expect("late old flush advances the rebound source without identity mismatch");

        assert_eq!(
            lane_work.effect_count(),
            0,
            "the terminal old flush must remove the queued reconnect current chunk"
        );
        assert!(
            !services
                .retains_reply_route_for_test(&reconnected_route)
                .expect("reconnect was never transferred into worker ownership")
        );
        assert!(
            !services
                .has_pending_exact_output()
                .expect("the terminal receipt is fully applied exactly once")
        );
        assert!(!services.exact_output_restart_required_for_test());
    }

    #[test]
    fn runner_dispatch_rejects_certified_sidecar_chunk_without_reply_route() {
        let (services, keys) = super::super::v2_worker::tests::fixture();
        let local = PeerId::new(keys[0].public_key().clone());
        let requester = PeerId::new(keys[1].public_key().clone());
        let chunk = runner_sidecar_chunk(local, requester.clone(), b"runner missing sidecar route");

        let error = dispatch_lane_work_effect(
            &services,
            V2LaneWorkEffect::PostCertifiedMergeSidecar {
                peer: requester,
                reply_routes: None,
                message: Arc::new(CertifiedMergeSidecarMessage::Chunk(chunk)),
            },
        )
        .expect_err("runner must reject a sidecar response without local reply authority");
        assert!(error.to_string().contains("reply-route ownership"));
    }

    #[test]
    fn runner_dispatch_rejects_durable_response_without_reply_routes() {
        let history = super::super::v2_lane_work::tests::durable_lane_history_fixture();
        let requester = history.certificate.commit_qc.validator_set[1].clone();
        let services = super::super::v2_worker::tests::service_for_history_context(
            history.kura,
            history.context,
            &history.validators,
        );
        let mut effect = V2LaneWorkEffect::PostDurableLaneCertificate {
            peer: requester,
            reply_routes: None,
            ingress_ownership: None,
            certificate: history.certificate,
        };
        assert!(
            retain_active_owned_reply_routes(&mut effect),
            "the scheduler prefilter must leave malformed ownership for strict dispatch"
        );
        let error = dispatch_lane_work_effect(&services, effect)
            .expect_err("runner must reject a durable response without local reply authority");
        assert!(
            error
                .to_string()
                .contains("lost its authenticated reply routes")
        );
    }

    #[test]
    fn snapshot_successor_time_is_exact_bounded_and_restart_deterministic() {
        let height_started_at = Instant::now();
        let round_timeout = Duration::from_secs(20);
        assert_eq!(
            initial_block_sync_deadline(height_started_at, round_timeout, false),
            deadline_after(height_started_at, round_timeout),
            "an ordinary live height keeps the full quiet round before discovery"
        );
        assert_eq!(
            initial_block_sync_deadline(height_started_at, round_timeout, true),
            height_started_at,
            "a recovered or historically synchronized height probes immediately"
        );
        assert!(!retain_eager_block_sync(false, false));
        assert!(retain_eager_block_sync(true, false));
        assert!(retain_eager_block_sync(false, true));

        let anchor = wire::SnapshotBootstrapAnchor {
            snapshot_height: 99,
            snapshot_block_hash: HashOf::from_untyped_unchecked(Hash::new(b"snapshot tip")),
            snapshot_block_creation_time_ms: 50_000,
            snapshot_state_hash: Hash::new(b"snapshot state"),
        };
        let cadence = Duration::from_millis(750);
        let first = snapshot_successor_logical_time(&anchor, cadence)
            .expect("representable snapshot successor time");
        let restarted = snapshot_successor_logical_time(&anchor, cadence)
            .expect("restart derives the same successor time");
        assert_eq!(first, Duration::from_millis(50_750));
        assert_eq!(restarted, first);

        assert!(matches!(
            snapshot_successor_logical_time(&anchor, Duration::ZERO),
            Err(V2RunnerError::InvalidSnapshotBootstrapCadence)
        ));
        assert!(matches!(
            snapshot_successor_logical_time(&anchor, Duration::from_nanos(999_999)),
            Err(V2RunnerError::InvalidSnapshotBootstrapCadence)
        ));
        let overflowing = wire::SnapshotBootstrapAnchor {
            snapshot_block_creation_time_ms: u64::MAX,
            ..anchor
        };
        assert!(matches!(
            snapshot_successor_logical_time(&overflowing, Duration::from_millis(1)),
            Err(V2RunnerError::V2BlockTimeOverflow)
        ));
    }

    #[test]
    fn unsupported_storage_platform_rejects_runner_voter_and_admits_observer() {
        let admit_role =
            |role| require_validator_storage_platform(role == NodeRole::Validator, false);
        assert!(matches!(
            admit_role(NodeRole::Validator),
            Err(super::super::v2_lane_work::V2LaneWorkError::UnsupportedValidatorStoragePlatform)
        ));
        assert_eq!(
            admit_role(NodeRole::Observer),
            Ok(()),
            "an unsupported host may enter only the explicitly non-voting runner path"
        );
    }

    #[test]
    fn global_voting_role_tracks_each_frozen_roster_without_losing_validator_processes() {
        let (context, keys) = context();
        let peer = PeerId::new(keys[0].public_key().clone());
        assert_eq!(
            local_validator_index(&context, &peer, NodeRole::Observer).expect("observer"),
            None
        );
        assert_eq!(
            local_validator_index(&context, &peer, NodeRole::Validator)
                .expect("roster member remains a global validator"),
            context
                .roster
                .iter()
                .position(|entry| entry.validator == peer)
                .map(|index| u32::try_from(index).expect("fixture roster index fits u32"))
        );
        assert_eq!(
            local_validator_index(
                &context,
                &PeerId::new(
                    KeyPair::try_from_seed(vec![0x55; 32], Algorithm::BlsNormal)
                        .expect("deterministic non-member key")
                        .public_key()
                        .clone()
                ),
                NodeRole::Validator
            )
            .expect("a removed validator continues as a global observer"),
            None
        );
    }

    #[test]
    fn runtime_queue_reserves_progress_and_completions() {
        let config = SumeragiV2Config {
            format_version: SUMERAGI_V2_CONFIG_FORMAT_VERSION,
            protocol_version: wire::PROTOCOL_VERSION,
            mode: wire::ConsensusMode::Permissioned,
            block_cadence_ms: 1_000,
            limits: SumeragiV2Limits {
                max_transactions: 512,
                max_payload_bytes: 16 * 1024 * 1024,
                max_queue_scan: 2_048,
                control_queue_capacity: 128,
                runtime_command_capacity: 8,
                runtime_progress_reserve: 2,
                runtime_completion_reserve: 2,
                body_queue_capacity: 16,
                authenticated_non_validator_source_capacity: 2,
                body_bytes: 160 * 1024 * 1024,
                body_source_bytes: 32 * 1024 * 1024,
                chunk_queue_capacity: 64,
                effect_work_capacity: 2,
                ready_body_capacity: 8,
                ready_body_bytes: 32 * 1024 * 1024,
                certified_request_capacity: 8,
                authenticated_merge_qc_capacity: 64,
                merge_leader_body_frame_headroom_bytes: 1024 * 1024,
                autonomous_carrier_headroom_bytes: 1024 * 1024,
                autonomous_producer_recheck_ms: 100,
                historical_recovery_stuck_attempts: 32,
                historical_recovery_retry_tier_attempts: 4,
                historical_recovery_max_retry_tier: 6,
                sidecar_service_burst: 8,
                merge_sidecar_inbound_session_capacity: 32,
                merge_sidecar_inbound_sessions_per_peer: 4,
                merge_sidecar_inbound_assembly_bytes: 64 * 1024 * 1024,
                merge_sidecar_inbound_assembly_bytes_per_peer: 32 * 1024 * 1024,
                merge_sidecar_deferred_block_capacity: 128,
                merge_sidecar_future_block_distance: 64,
                merge_sidecar_request_timeout_ms: 10_000,
                merge_sidecar_outbound_sessions_per_source: 2,
                merge_sidecar_outbound_bytes_per_source: 16 * 1024 * 1024,
                merge_sidecar_server_request_gates_per_source: 4,
                pending_certified_merge_entry_capacity: 1_024,
                pending_queue_plan_admission_capacity: 1_024,
                pending_control_sidecar_bytes: 256 * 1024 * 1024,
                merge_signing_guard_record_capacity: 1_024,
                merge_signing_guard_record_bytes: 16 * 1024 * 1024 + 64 * 1024,
                merge_signing_guard_total_bytes: 256 * 1024 * 1024,
                native_amx_signing_guard_record_capacity: 524_288,
                native_amx_signing_guard_record_bytes: 16 * 1024,
                native_amx_signing_guard_anchor_bytes: 4 * 1024,
            },
            key_policy: SumeragiV2KeyPolicy {
                activation_lead_blocks: 1,
                overlap_grace_blocks: 1,
                expiry_grace_blocks: 1,
                require_hsm: false,
                allowed_algorithms: vec![Algorithm::BlsNormal],
                allowed_hsm_providers: Vec::new(),
            },
        };
        assert!(runtime_queue_config(&config).is_ok());
        assert!(effect_queue_config(&config).is_ok());

        let mut invalid = config;
        invalid.limits.effect_work_capacity = 3;
        assert!(matches!(
            effect_queue_config(&invalid),
            Err(V2RunnerError::EffectWorkExceedsCompletionReserve {
                pending: 3,
                reserve: 2,
            })
        ));
    }

    #[test]
    fn commit_certificate_runtime_backpressure_remains_retryable() {
        let admission = Err(CommitCertificateAdmissionError::Enqueue(
            NetworkIngressError::Backpressure(
                crate::sumeragi::v2_runtime::EnqueueError::ReservedCapacity,
            ),
        ));
        assert!(matches!(
            commit_certificate_admission_completed(admission),
            Ok(false)
        ));
        assert!(matches!(
            commit_certificate_admission_completed(Ok(())),
            Ok(true)
        ));
    }

    #[test]
    fn tag_roundtrip_rejects_another_height() {
        let (context, _) = context();
        let tag = EventTag::new(1, 3, Generation::new(7));
        assert_eq!(round_for_tag(&context, tag).expect("round").view, 3);
        assert!(matches!(
            round_for_tag(&context, EventTag::new(2, 0, Generation::new(7))),
            Err(V2RunnerError::StaleTag)
        ));
    }

    fn proposal_owner(
        context: &wire::HeightContext,
        tag: EventTag,
        lock: Option<(u64, wire::BlockSubject)>,
        decided_subject: Option<wire::BlockSubject>,
    ) -> LocalProposalOwner {
        LocalProposalOwner {
            tag,
            locked_body: lock.map(|(view, subject)| {
                (
                    wire::ConsensusRound {
                        context_id: context.id(),
                        height: context.height,
                        view,
                    },
                    subject,
                )
            }),
            decided_subject,
        }
    }

    fn proposal_subject(label: &[u8]) -> wire::BlockSubject {
        wire::BlockSubject {
            parent_block_hash: None,
            block_hash: HashOf::from_untyped_unchecked(Hash::new(label)),
            payload_hash: Hash::new(&[label, b" payload"].concat()),
        }
    }

    #[test]
    fn locked_body_recovery_is_independent_of_reproposal_gates() {
        let (context, _) = context();
        let tag = EventTag::new(context.height, 5, Generation::new(18));
        let subject = proposal_subject(b"nonleader locked-body recovery");
        let locked_round = wire::ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view: 3,
        };
        let leader = context.leader(tag.view());
        let nonleader = if leader == 0 { 1 } else { 0 };
        let directive =
            LocalProposalDirective::for_test(tag, leader, Some(locked_round), Some(subject), None);
        let owner = LocalProposalOwner::from(directive);
        let expected_request = Some((tag, locked_round, subject));

        for (local_validator, attempted, can_admit) in [
            (nonleader, None, true),
            (leader, Some(owner), true),
            (leader, None, false),
        ] {
            let plan = locked_body_recovery_plan(directive, local_validator, attempted, can_admit);
            assert_eq!(plan.request, expected_request);
            assert!(
                !plan.may_repropose,
                "body recovery must survive every local reproposal gate"
            );
        }

        let eligible = locked_body_recovery_plan(directive, leader, None, true);
        assert_eq!(eligible.request, expected_request);
        assert!(eligible.may_repropose);

        let decided = LocalProposalDirective::for_test(
            tag,
            leader,
            Some(locked_round),
            Some(subject),
            Some(subject),
        );
        assert_eq!(
            locked_body_recovery_plan(decided, leader, None, true),
            LockedBodyRecoveryPlan {
                request: None,
                may_repropose: false,
            }
        );
    }

    #[test]
    fn lane_production_duty_survives_successor_global_roster_removal() {
        let (context, _) = context();
        let tag = EventTag::new(context.height, 4, Generation::new(23));
        let directive =
            LocalProposalDirective::for_test(tag, context.leader(tag.view()), None, None, None);

        assert_eq!(
            local_consensus_duties(directive, None),
            LocalConsensusDuties {
                autonomous_lane_view: Some(tag.view()),
                global_validator: None,
            },
            "successor-global observer status must not suppress independently frozen lane-author work"
        );

        let subject = proposal_subject(b"lane production duty lock");
        let locked_round = wire::ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view: 2,
        };
        let locked = LocalProposalDirective::for_test(
            tag,
            context.leader(tag.view()),
            Some(locked_round),
            Some(subject),
            None,
        );
        assert_eq!(
            local_consensus_duties(locked, None).autonomous_lane_view,
            None,
            "a global lock still suppresses fresh lane payload production"
        );

        let decided = LocalProposalDirective::for_test(
            tag,
            context.leader(tag.view()),
            None,
            None,
            Some(subject),
        );
        assert_eq!(
            local_consensus_duties(decided, None).autonomous_lane_view,
            None,
            "a terminal global decision retires fresh lane payload production"
        );
    }

    #[test]
    fn same_tag_higher_lock_retires_all_local_proposal_owners() {
        let (context, _) = context();
        let tag = EventTag::new(context.height, 5, Generation::new(11));
        let subject_a = proposal_subject(b"local owner A");
        let subject_b = proposal_subject(b"local owner B");
        let owner_a = proposal_owner(&context, tag, Some((2, subject_a)), None);
        let owner_b = proposal_owner(&context, tag, Some((4, subject_b)), None);
        let now = Instant::now();
        let mut state = LocalProposalState {
            attempted: Some(owner_a),
            submitted: Some((owner_a, subject_a)),
            heartbeat_only: Some(owner_a),
            candidate_work_wait: Some(CandidateWorkWait {
                owner: owner_a,
                started_at: now,
                next_retry: now,
            }),
            pending_events: Some(PendingLocalEvents {
                owner: owner_a,
                subject: subject_a,
                events: Vec::new(),
            }),
            global_selection: None,
        };

        state.reconcile(owner_b);

        assert!(state.attempted.is_none());
        assert!(state.submitted.is_none());
        assert!(state.heartbeat_only.is_none());
        assert!(state.candidate_work_wait.is_none());
        assert!(state.pending_events.is_none());
    }

    #[test]
    fn deferred_autonomous_work_timeout_arms_only_an_empty_heartbeat() {
        let (context, _) = context();
        let owner = proposal_owner(
            &context,
            EventTag::new(context.height, 3, Generation::new(17)),
            None,
            None,
        );
        let started_at = Instant::now();
        let wait_bound = Duration::from_secs(2);
        let mut state = LocalProposalState::default();

        state.defer_candidate_work(owner, started_at, wait_bound);
        assert_eq!(state.heartbeat_only, None);
        assert!(
            state
                .candidate_work_wait
                .is_some_and(|wait| wait.owner == owner && wait.started_at == started_at)
        );

        let expired_at = started_at
            .checked_add(wait_bound)
            .expect("fixture wait deadline is representable");
        state.defer_candidate_work(owner, expired_at, wait_bound);
        assert_eq!(state.heartbeat_only, Some(owner));
        assert!(state.candidate_work_wait.is_none());

        state.defer_candidate_work(owner, expired_at, wait_bound);
        assert_eq!(
            state.heartbeat_only,
            Some(owner),
            "repeated timeout handling must never re-arm ordinary candidate work"
        );
        assert!(state.candidate_work_wait.is_none());
    }

    #[test]
    fn pre_submit_lane_binding_rejection_arms_one_empty_heartbeat_retry() {
        let (context, _) = context();
        let owner = proposal_owner(
            &context,
            EventTag::new(context.height, 3, Generation::new(19)),
            None,
            None,
        );
        let now = Instant::now();
        let mut state = LocalProposalState {
            attempted: Some(owner),
            candidate_work_wait: Some(CandidateWorkWait {
                owner,
                started_at: now,
                next_retry: now,
            }),
            ..LocalProposalState::default()
        };

        assert_eq!(
            state.handle_candidate_binding_rejection(owner),
            LocalValidationDisposition::RetryHeartbeat,
            "an unsubmitted lane-binding rejection must not stop the process"
        );
        assert_eq!(state.attempted, None);
        assert_eq!(state.heartbeat_only, Some(owner));
        assert!(state.candidate_work_wait.is_none());
        assert!(state.submitted.is_none());
        assert!(state.pending_events.is_none());
        assert!(state.global_selection.is_none());

        assert_eq!(
            state.handle_candidate_binding_rejection(owner),
            LocalValidationDisposition::FatalHeartbeat,
            "a rejected empty heartbeat still fails closed"
        );
    }

    #[test]
    fn first_same_subject_lock_preserves_pending_local_proposal_events() {
        let (context, _) = context();
        let tag = EventTag::new(context.height, 5, Generation::new(14));
        let subject = proposal_subject(b"first lock keeps local subject");
        let unlocked = proposal_owner(&context, tag, None, None);
        let locked = proposal_owner(&context, tag, Some((5, subject)), None);
        let mut state = LocalProposalState {
            attempted: Some(unlocked),
            submitted: Some((unlocked, subject)),
            pending_events: Some(PendingLocalEvents {
                owner: unlocked,
                subject,
                events: Vec::new(),
            }),
            ..LocalProposalState::default()
        };

        state.reconcile(locked);

        assert_eq!(state.attempted, Some(locked));
        assert_eq!(state.submitted, Some((locked, subject)));
        assert!(
            state
                .pending_events
                .as_ref()
                .is_some_and(|pending| { pending.owner == locked && pending.subject == subject })
        );
    }

    #[test]
    fn higher_same_subject_lock_retires_prior_origin_work() {
        let (context, _) = context();
        let tag = EventTag::new(context.height, 5, Generation::new(15));
        let subject = proposal_subject(b"higher lock retires old origin");
        let lower = proposal_owner(&context, tag, Some((2, subject)), None);
        let higher = proposal_owner(&context, tag, Some((4, subject)), None);
        let mut state = LocalProposalState {
            attempted: Some(lower),
            submitted: Some((lower, subject)),
            pending_events: Some(PendingLocalEvents {
                owner: lower,
                subject,
                events: Vec::new(),
            }),
            ..LocalProposalState::default()
        };

        assert_ne!(lower, higher);
        state.reconcile(higher);

        assert!(state.attempted.is_none());
        assert!(state.submitted.is_none());
        assert!(state.pending_events.is_none());
    }

    #[test]
    fn first_same_subject_lock_from_prior_view_retires_unlocked_work() {
        let (context, _) = context();
        let tag = EventTag::new(context.height, 5, Generation::new(16));
        let subject = proposal_subject(b"old-origin first lock");
        let unlocked = proposal_owner(&context, tag, None, None);
        let locked = proposal_owner(&context, tag, Some((4, subject)), None);
        let mut state = LocalProposalState {
            attempted: Some(unlocked),
            submitted: Some((unlocked, subject)),
            pending_events: Some(PendingLocalEvents {
                owner: unlocked,
                subject,
                events: Vec::new(),
            }),
            ..LocalProposalState::default()
        };

        state.reconcile(locked);

        assert!(state.attempted.is_none());
        assert!(state.submitted.is_none());
        assert!(state.pending_events.is_none());
    }

    #[test]
    fn late_old_rejection_cannot_arm_heartbeat_for_replacement_lock() {
        let (context, _) = context();
        let tag = EventTag::new(context.height, 5, Generation::new(12));
        let subject_a = proposal_subject(b"rejected old A");
        let subject_b = proposal_subject(b"current B");
        let owner_a = proposal_owner(&context, tag, Some((2, subject_a)), None);
        let owner_b = proposal_owner(&context, tag, Some((4, subject_b)), None);
        let proposal_round = wire::ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view: tag.view(),
        };
        let mut state = LocalProposalState {
            submitted: Some((owner_a, subject_a)),
            ..LocalProposalState::default()
        };

        assert_eq!(
            state.handle_validation_rejection(owner_b, proposal_round, proposal_round, subject_a,),
            LocalValidationDisposition::Ignored
        );
        assert_eq!(state.heartbeat_only, None);

        state.submitted = Some((owner_b, subject_b));
        assert_eq!(
            state.handle_validation_rejection(owner_b, proposal_round, proposal_round, subject_b,),
            LocalValidationDisposition::RetryHeartbeat
        );
        assert_eq!(state.heartbeat_only, Some(owner_b));

        state.submitted = Some((owner_b, subject_b));
        assert_eq!(
            state.handle_validation_rejection(owner_b, proposal_round, proposal_round, subject_b,),
            LocalValidationDisposition::FatalHeartbeat
        );
    }

    #[test]
    fn decision_retires_local_work_before_prepared_delivery() {
        let (context, _) = context();
        let tag = EventTag::new(context.height, 6, Generation::new(13));
        let subject = proposal_subject(b"decided proposal");
        let active = proposal_owner(&context, tag, Some((4, subject)), None);
        let decided = proposal_owner(&context, tag, Some((4, subject)), Some(subject));
        let mut state = LocalProposalState {
            attempted: Some(active),
            submitted: Some((active, subject)),
            heartbeat_only: None,
            candidate_work_wait: None,
            pending_events: Some(PendingLocalEvents {
                owner: active,
                subject,
                events: Vec::new(),
            }),
            global_selection: None,
        };

        assert!(state.take_prepared_events(decided, tag, subject).is_none());
        assert!(state.attempted.is_none());
        assert!(state.submitted.is_none());
        assert!(state.pending_events.is_none());
    }

    #[test]
    fn height_one_proposal_projects_staged_genesis_to_resultless_wire() {
        let key_pair = KeyPair::try_from_seed(vec![0x71; 32], Algorithm::Ed25519)
            .expect("deterministic genesis key");
        let transaction = TransactionBuilder::new(
            ChainId::from("height-one-resultless-projection"),
            AccountId::new(key_pair.public_key().clone()),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(Level::INFO, "staged genesis execution".to_owned())])
        .sign(key_pair.private_key());
        let entrypoint = transaction.hash_as_entrypoint();
        let mut staged =
            SignedBlock::genesis(vec![transaction], key_pair.private_key(), None, None);
        staged
            .set_transaction_results(
                Vec::new(),
                &[entrypoint],
                vec![TransactionResultInner::Ok(DataTriggerSequence::default())],
            )
            .expect("attach deterministic staged genesis results");
        assert!(staged.has_results());
        assert!(!staged.is_resultless_proposal());
        assert!(staged.header().result_merkle_root().is_some());

        let staged_header_hash = staged.header().hash();
        let staged_hash = staged.hash();
        let staged_signatures = staged.signatures().cloned().collect::<Vec<_>>();
        let staged_result_root = staged.header().result_merkle_root();
        let staged_execution_wire = staged.encode_wire().expect("encode staged execution image");
        let wire = canonical_height_one_proposal_wire(&staged)
            .expect("encode canonical height-one proposal");
        let proposal = decode_framed_signed_block(&wire).expect("decode height-one proposal");

        assert!(proposal.is_resultless_proposal());
        assert!(!proposal.has_results());
        assert!(proposal.header().result_merkle_root().is_none());
        assert_eq!(proposal.header().hash(), staged_header_hash);
        assert_eq!(proposal.hash(), staged_hash);
        assert_eq!(
            proposal.signatures().cloned().collect::<Vec<_>>(),
            staged_signatures
        );
        assert_eq!(
            staged.header().result_merkle_root(),
            staged_result_root,
            "proposal projection must not mutate the staged result root"
        );
        assert_eq!(
            staged
                .encode_wire()
                .expect("re-encode staged execution image"),
            staged_execution_wire,
            "proposal projection must not mutate the staged execution image"
        );
        assert_eq!(
            Hash::new(&wire),
            staged
                .canonical_proposal_wire_hash()
                .expect("hash canonical staged-genesis proposal"),
        );
    }

    #[test]
    fn exact_locked_body_is_reencoded_at_the_reproposal_round_without_byte_drift() {
        let (context, _) = context();
        let key_pair = KeyPair::try_from_seed(vec![0x72; 32], Algorithm::Ed25519)
            .expect("deterministic proposal key");
        let transaction = TransactionBuilder::new(
            ChainId::from("locked-reproposal-exact-body"),
            AccountId::new(key_pair.public_key().clone()),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(Level::INFO, "immutable locked body".to_owned())])
        .sign(key_pair.private_key());
        let block = SignedBlock::genesis(vec![transaction], key_pair.private_key(), None, None)
            .canonical_resultless_proposal();
        assert!(block.is_resultless_proposal());
        let canonical_wire = block.encode_wire().expect("encode exact proposal body");
        let locked_subject = wire::BlockSubject {
            parent_block_hash: block.header().prev_block_hash(),
            block_hash: block.hash(),
            payload_hash: Hash::new(&canonical_wire),
        };
        let tag = EventTag::new(context.height, 3, Generation::new(17));

        let encoded = encode_exact_local_body(&context, tag, Some(locked_subject), &canonical_wire)
            .expect("encode unchanged locked body at the reproposal round");
        assert_eq!(
            encoded.manifest().round,
            round_for_tag(&context, tag).unwrap()
        );
        assert_eq!(encoded.manifest().subject, locked_subject);
        let (_, chunks) = encoded.into_parts();
        assert_eq!(chunks.concat(), canonical_wire);

        let foreign_subject = proposal_subject(b"foreign locked subject");
        assert!(matches!(
            encode_exact_local_body(&context, tag, Some(foreign_subject), &canonical_wire,),
            Err(V2RunnerError::LockedBodyMismatch)
        ));
    }

    #[test]
    fn replayed_proposal_sign_reserves_only_the_exact_current_lock_owner() {
        let (context, _) = context();
        let tag = EventTag::new(context.height, 3, Generation::new(9));
        let round = wire::ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view: tag.view(),
        };
        let body = b"replayed proposal payload";
        let subject = wire::BlockSubject {
            parent_block_hash: None,
            block_hash: HashOf::from_untyped_unchecked(Hash::new(b"replayed proposal block")),
            payload_hash: Hash::new(body),
        };
        let manifest = encode_payload(&context, round, subject, body)
            .expect("encode replayed proposal fixture payload")
            .manifest()
            .clone();
        let proposal = wire::Proposal {
            round,
            proposer: context.leader(round.view),
            subject,
            manifest,
            justification: wire::ProposalJustification::ParentCommit(
                wire::ParentCommitJustification { certificate: None },
            ),
            signature: Vec::new(),
        };
        let effects = [
            AdapterEffect::Broadcast(wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::Proposal(proposal.clone()),
            )),
            AdapterEffect::Sign {
                tag,
                request: SignRequest::Proposal(proposal),
            },
        ];

        let replayed = replayed_proposal_sign(&effects).expect("extract exact replay owner");
        assert_eq!(
            replayed,
            ReplayedProposalSign {
                tag,
                round,
                subject,
            }
        );
        assert_eq!(replayed_proposal_sign(&effects[..1]), None);
        assert_eq!(replayed_proposal_sign(&[]), None);

        let directive = |locked_subject: Option<wire::BlockSubject>,
                         decided_subject: Option<wire::BlockSubject>| {
            LocalProposalDirective::for_test(
                tag,
                context.leader(tag.view()),
                locked_subject.map(|_| wire::ConsensusRound {
                    context_id: context.id(),
                    height: context.height,
                    view: 1,
                }),
                locked_subject,
                decided_subject,
            )
        };
        let unlocked = directive(None, None);
        assert_eq!(
            LocalProposalState::from_replayed_proposal(Some(replayed), unlocked).attempted,
            Some(LocalProposalOwner::from(unlocked))
        );

        let exact_lock = directive(Some(subject), None);
        assert_eq!(
            LocalProposalState::from_replayed_proposal(Some(replayed), exact_lock).attempted,
            Some(LocalProposalOwner::from(exact_lock)),
            "the exact replayed subject owns current locked-body work"
        );

        let foreign_lock = directive(Some(proposal_subject(b"foreign replay lock")), None);
        assert!(
            LocalProposalState::from_replayed_proposal(Some(replayed), foreign_lock)
                .attempted
                .is_none(),
            "an equal-tag proposal for another subject cannot reserve the current lock owner"
        );

        let mismatched_round = ReplayedProposalSign {
            round: wire::ConsensusRound { view: 2, ..round },
            ..replayed
        };
        assert!(
            LocalProposalState::from_replayed_proposal(Some(mismatched_round), unlocked)
                .attempted
                .is_none(),
            "the replayed proposal round must match its reducer tag"
        );

        let decided = directive(Some(subject), Some(subject));
        assert!(
            LocalProposalState::from_replayed_proposal(Some(replayed), decided)
                .attempted
                .is_none(),
            "a decision retires every replayed proposal reservation"
        );
    }

    #[test]
    fn outer_ingress_batch_services_completions_and_runtime_before_every_ingress() {
        assert_eq!(
            outer_ingress_turns(3).collect::<Vec<_>>(),
            vec![
                OuterIngressTurn::Completion,
                OuterIngressTurn::Runtime,
                OuterIngressTurn::Ingress,
                OuterIngressTurn::Completion,
                OuterIngressTurn::Runtime,
                OuterIngressTurn::Ingress,
                OuterIngressTurn::Completion,
                OuterIngressTurn::Runtime,
                OuterIngressTurn::Ingress,
            ]
        );
        assert_eq!(
            outer_ingress_turns(0).collect::<Vec<_>>(),
            vec![
                OuterIngressTurn::Completion,
                OuterIngressTurn::Runtime,
                OuterIngressTurn::Ingress,
            ],
            "a zero-sized batch still owes completion and runtime service opportunities"
        );
    }

    #[test]
    fn terminal_ingress_discards_commit_discovery_and_losing_current_body_requests() {
        let (context, keys) = context();
        let round = wire::ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view: 0,
        };
        let body = b"terminal ingress exact body".to_vec();
        let subject = wire::BlockSubject {
            parent_block_hash: None,
            block_hash: HashOf::from_untyped_unchecked(Hash::new(b"terminal ingress block")),
            payload_hash: Hash::new(&body),
        };
        let certificate = wire::QuorumCertificate {
            round,
            proposal_round: round,
            phase: wire::GlobalPhase::Commit,
            subject,
            execution_commitment: wire::ExecutionCommitment::without_topups(
                Hash::new(b"terminal ingress parent state"),
                Hash::new(b"terminal ingress post state"),
                Hash::new(b"terminal ingress writes"),
                Hash::new(b"terminal ingress executed block"),
            ),
            signers: vec![0, 1, 2],
            aggregate_signature: vec![1],
        };
        let response = wire::ConsensusMessageV2Payload::CommitCertificateResponse(
            wire::CommitCertificateResponse {
                request_hash: HashOf::from_untyped_unchecked(Hash::new(
                    b"terminal ingress commit request",
                )),
                certificate: certificate.clone(),
                responder: PeerId::new(keys[0].public_key().clone()),
                signature: vec![1],
            },
        );
        assert!(v2_payload_is_terminal_reducer_control(&response));

        let manifest = encode_payload(&context, round, subject, &body)
            .expect("encode terminal body fixture payload")
            .manifest()
            .clone();
        assert!(!v2_payload_is_terminal_reducer_control(
            &wire::ConsensusMessageV2Payload::PayloadManifest(manifest)
        ));

        let exact_request = wire::CertifiedBodyRequest {
            round,
            subject,
            certificate: certificate.clone(),
            requester: PeerId::new(keys[1].public_key().clone()),
            signature: vec![1],
        };
        assert!(!certified_body_request_is_superseded_after_decision(
            &exact_request,
            Some(subject),
            context.height,
        ));

        let losing_subject = wire::BlockSubject {
            block_hash: HashOf::from_untyped_unchecked(Hash::new(b"losing terminal block")),
            ..subject
        };
        let mut losing_request = exact_request.clone();
        losing_request.subject = losing_subject;
        losing_request.certificate.subject = losing_subject;
        assert!(certified_body_request_is_superseded_after_decision(
            &losing_request,
            Some(subject),
            context.height,
        ));

        losing_request.round.height = context.height.saturating_sub(1);
        losing_request.certificate.round.height = losing_request.round.height;
        losing_request.certificate.proposal_round.height = losing_request.round.height;
        assert!(!certified_body_request_is_superseded_after_decision(
            &losing_request,
            Some(subject),
            context.height,
        ));
    }

    #[test]
    fn finalized_rollover_closes_ingress_before_successor_replay() {
        let ready = AtomicBool::new(true);
        let ingress = FairV2Ingress::new(1, 1024 * 1024, 1024 * 1024, 0, 0);
        ingress
            .configure_roster(std::iter::empty())
            .expect("configure untrusted test lane");
        ingress.open().expect("open test ingress");
        close_ingress_for_rollover(&ready, &ingress);
        assert!(!ready.load(Ordering::Acquire));
        assert!(matches!(
            ingress.try_push(InboundBlockMessage::new(valid_ingress_probe(), None)),
            Err(FairV2IngressPushError::Closed(_))
        ));
    }

    #[test]
    fn synthesized_durable_rollover_contract_allows_successor_after_dead_target_handoff() {
        // This narrow rollover contract starts from a synthesized, internally
        // consistent Kura receipt/finality artifact. It does not exercise the
        // QC -> body recovery -> store -> validation -> application pipeline or
        // claim end-to-end catch-up coverage.
        let _guard = super::super::status::rbc_status_test_guard();
        super::super::status::clear_v2_status();
        let context = super::super::v2_worker::tests::production_output_handoff_with_dead_target();
        publish_applied_runner_status(&context);

        let predecessor = test_predecessor(&context, b"dead target rollover");
        let construction =
            PendingSuccessorConstruction::begin(predecessor).expect("begin successor handoff");
        let ready = AtomicBool::new(false);
        let ingress = FairV2Ingress::new(1, 1024 * 1024, 1024 * 1024, 0, 0);
        ingress
            .configure_roster(std::iter::empty())
            .expect("configure successor ingress");
        let mut successor_context = context.clone();
        successor_context.height = successor_context.height.saturating_add(1);
        let mut successor = runner_status(&successor_context);
        successor.last_committed_height = context.height;
        successor.liveness.generation = successor_context.height;
        successor.liveness.last_progress = Some(wire::SumeragiV2ProgressTransitionStatus {
            generation: successor.liveness.generation,
            round: wire::ConsensusRound {
                context_id: successor.height_context_id,
                height: successor.height,
                view: successor.view,
            },
            transition: wire::SumeragiV2ProgressTransition::SuccessorHeightActivated,
            age_ms: 0,
        });
        let activation = construction
            .bind(test_successor_authority(
                predecessor,
                successor.height_context_id,
            ))
            .expect("bind exact predecessor authority");
        let output_guard = ConsensusOutputGuard::isolated();

        open_ingress_for_active_height(
            output_guard.as_ref(),
            &ready,
            &ingress,
            Some((activation, successor.clone())),
        )
        .expect("dead-target durable handoff permits successor activation");

        assert!(ready.load(Ordering::Acquire));
        let active = super::super::status::v2_status().expect("active successor status");
        assert_eq!(active.height, successor.height);
        assert_eq!(active.last_committed_height, context.height);
        assert!(matches!(
            active.liveness.last_progress,
            Some(wire::SumeragiV2ProgressTransitionStatus {
                transition: wire::SumeragiV2ProgressTransition::SuccessorHeightActivated,
                ..
            })
        ));
        close_ingress_for_rollover(&ready, &ingress);
        super::super::status::clear_v2_status();
    }

    #[test]
    fn successor_activation_is_published_only_after_ingress_is_open() {
        let _guard = super::super::status::rbc_status_test_guard();
        super::super::status::clear_v2_status();
        let (context, _) = context();
        publish_applied_runner_status(&context);
        let predecessor = test_predecessor(&context, b"live ingress rollover");
        let construction =
            PendingSuccessorConstruction::begin(predecessor).expect("begin successor handoff");
        let ready = AtomicBool::new(false);
        let ingress = FairV2Ingress::new(1, 1024 * 1024, 1024 * 1024, 0, 0);
        ingress
            .configure_roster(std::iter::empty())
            .expect("configure untrusted test lane");
        let before = super::super::status::v2_status().expect("predecessor status");
        assert_eq!(before.height, context.height);
        assert_eq!(
            before.liveness.work.successor_height,
            wire::SumeragiV2LocalWorkStage::Running
        );
        assert_eq!(
            before
                .liveness
                .last_progress
                .expect("application marker")
                .transition,
            wire::SumeragiV2ProgressTransition::Applied
        );
        assert!(!ready.load(Ordering::Acquire));
        assert!(
            matches!(
                ingress.try_push(InboundBlockMessage::new(valid_ingress_probe(), None)),
                Err(FairV2IngressPushError::Closed(_))
            ),
            "closed ingress must precede activation publication"
        );

        let mut successor_context = context.clone();
        successor_context.height += 1;
        let mut successor = runner_status(&successor_context);
        successor.last_committed_height = context.height;
        successor.liveness.generation = successor_context.height;
        successor.liveness.last_progress = Some(wire::SumeragiV2ProgressTransitionStatus {
            generation: successor.liveness.generation,
            round: wire::ConsensusRound {
                context_id: successor.height_context_id,
                height: successor.height,
                view: successor.view,
            },
            transition: wire::SumeragiV2ProgressTransition::SuccessorHeightActivated,
            age_ms: 0,
        });
        let activation = construction
            .bind(test_successor_authority(
                predecessor,
                successor.height_context_id,
            ))
            .expect("bind exact predecessor authority");
        let output_guard = ConsensusOutputGuard::isolated();
        open_ingress_for_active_height(
            output_guard.as_ref(),
            &ready,
            &ingress,
            Some((activation, successor.clone())),
        )
        .expect("open ingress and publish one activation");

        assert!(ready.load(Ordering::Acquire));
        ingress
            .try_push(InboundBlockMessage::new(valid_ingress_probe(), None))
            .expect("activation publication follows open ingress");
        let active = super::super::status::v2_status().expect("active successor status");
        assert_eq!(active.height, successor.height);
        let marker = active
            .liveness
            .last_progress
            .expect("successor activation marker");
        assert_eq!(
            marker.transition,
            wire::SumeragiV2ProgressTransition::SuccessorHeightActivated
        );
        assert_eq!(marker.generation, successor.liveness.generation);
        assert_eq!(marker.round.context_id, successor.height_context_id);
        assert_eq!(marker.round.height, successor.height);
        close_ingress_for_rollover(&ready, &ingress);
        super::super::status::clear_v2_status();

        publish_applied_runner_status(&context);
        let predecessor = test_predecessor(&context, b"foreign successor context");
        let construction = PendingSuccessorConstruction::begin(predecessor)
            .expect("begin mismatched-context handoff");
        let foreign_context_id =
            wire::HeightContextId(HashOf::<wire::HeightContext>::from_untyped_unchecked(
                Hash::new(b"foreign successor context"),
            ));
        let activation = construction
            .bind(test_successor_authority(predecessor, foreign_context_id))
            .expect("bind the exact predecessor but foreign successor context");
        let rejected_ready = AtomicBool::new(false);
        let rejected_ingress = FairV2Ingress::new(1, 1024 * 1024, 1024 * 1024, 0, 0);
        rejected_ingress
            .configure_roster(std::iter::empty())
            .expect("configure rejected test lane");
        assert!(
            open_ingress_for_active_height(
                output_guard.as_ref(),
                &rejected_ready,
                &rejected_ingress,
                Some((activation, successor)),
            )
            .is_err(),
            "an activation token cannot authorize another successor context"
        );
        assert!(!rejected_ready.load(Ordering::Acquire));
        assert!(
            matches!(
                rejected_ingress.try_push(InboundBlockMessage::new(valid_ingress_probe(), None)),
                Err(FairV2IngressPushError::Closed(_))
            ),
            "foreign-context rejection must close ingress again"
        );
        let predecessor = super::super::status::v2_status()
            .expect("foreign-context rejection retains the predecessor");
        assert_eq!(predecessor.height, context.height);
        assert_eq!(
            predecessor.liveness.work.successor_height,
            wire::SumeragiV2LocalWorkStage::Running
        );
        assert_eq!(
            predecessor
                .liveness
                .last_progress
                .expect("application remains authoritative")
                .transition,
            wire::SumeragiV2ProgressTransition::Applied
        );
        super::super::status::clear_v2_status();
    }

    #[test]
    fn complete_tip_recovery_uses_the_same_live_successor_boundary() {
        let _guard = super::super::status::rbc_status_test_guard();
        super::super::status::clear_v2_status();
        let (parent_context, _) = context();
        let ready = AtomicBool::new(false);
        let ingress = FairV2Ingress::new(1, 1024 * 1024, 1024 * 1024, 0, 0);
        ingress
            .configure_roster(std::iter::empty())
            .expect("configure untrusted test lane");

        let mut successor_context = parent_context.clone();
        successor_context.height += 1;
        let mut successor = runner_status(&successor_context);
        successor.last_committed_height = parent_context.height;
        successor.liveness.generation = successor_context.height;
        successor.liveness.last_progress = Some(wire::SumeragiV2ProgressTransitionStatus {
            generation: successor.liveness.generation,
            round: wire::ConsensusRound {
                context_id: successor.height_context_id,
                height: successor.height,
                view: successor.view,
            },
            transition: wire::SumeragiV2ProgressTransition::SuccessorHeightActivated,
            age_ms: 0,
        });
        let output_guard = ConsensusOutputGuard::isolated();
        let foreign_context_id =
            wire::HeightContextId(HashOf::<wire::HeightContext>::from_untyped_unchecked(
                Hash::new(b"foreign recovered successor context"),
            ));
        let predecessor = test_predecessor(&parent_context, b"complete tip recovery");
        let foreign_activation = PendingSuccessorActivation::recovered(
            RecoveredSuccessorActivationAuthority::CompleteTip(test_successor_authority(
                predecessor,
                foreign_context_id,
            )),
        )
        .expect("authenticate complete-tip retry lifecycle");
        assert!(
            open_ingress_for_active_height(
                output_guard.as_ref(),
                &ready,
                &ingress,
                Some((foreign_activation, successor.clone())),
            )
            .is_err(),
            "recovery cannot authorize a same-height snapshot from another context"
        );
        assert!(!ready.load(Ordering::Acquire));
        assert!(
            super::super::status::v2_status().is_none(),
            "rejected recovery must not publish a successor"
        );

        let activation = PendingSuccessorActivation::recovered(
            RecoveredSuccessorActivationAuthority::CompleteTip(test_successor_authority(
                predecessor,
                successor.height_context_id,
            )),
        )
        .expect("authenticate complete-tip retry lifecycle");
        open_ingress_for_active_height(
            output_guard.as_ref(),
            &ready,
            &ingress,
            Some((activation, successor.clone())),
        )
        .expect("open recovered successor");

        assert!(ready.load(Ordering::Acquire));
        let active = super::super::status::v2_status().expect("recovered successor status");
        assert_eq!(active.height, successor.height);
        assert_eq!(active.last_committed_height, parent_context.height);
        assert!(matches!(
            active.liveness.last_progress,
            Some(wire::SumeragiV2ProgressTransitionStatus {
                transition: wire::SumeragiV2ProgressTransition::SuccessorHeightActivated,
                ..
            })
        ));
        close_ingress_for_rollover(&ready, &ingress);
        super::super::status::clear_v2_status();
    }

    #[test]
    fn successor_construction_rejects_foreign_same_height_predecessor_authority() {
        let _guard = super::super::status::rbc_status_test_guard();
        super::super::status::clear_v2_status();
        let (context, _) = context();
        publish_applied_runner_status(&context);
        let expected = test_predecessor(&context, b"expected predecessor");
        let foreign = test_predecessor(&context, b"foreign same-height predecessor");
        assert_eq!(expected.height(), foreign.height());
        assert_ne!(expected, foreign);

        let construction =
            PendingSuccessorConstruction::begin(expected).expect("begin exact predecessor handoff");
        let mut successor_context = context.clone();
        successor_context.height += 1;
        let error = construction
            .bind(test_successor_authority(foreign, successor_context.id()))
            .expect_err("same-height foreign predecessor must not bind activation");
        assert!(matches!(
            error,
            V2RunnerError::SuccessorPredecessorAuthorityMismatch {
                expected: actual_expected,
                actual,
            } if actual_expected == expected && actual == foreign
        ));
        let predecessor = super::super::status::v2_status().expect("predecessor remains visible");
        assert_eq!(
            predecessor.liveness.work.successor_height,
            wire::SumeragiV2LocalWorkStage::Running
        );
        super::super::status::clear_v2_status();
    }

    #[test]
    fn successor_startup_failure_stays_running_and_fails_closed_without_activation() {
        let _guard = super::super::status::rbc_status_test_guard();
        super::super::status::clear_v2_status();
        let (context, keys) = context();
        publish_applied_runner_status(&context);
        let activation = PendingSuccessorConstruction::begin(test_predecessor(
            &context,
            b"failed successor startup",
        ))
        .expect("begin successor handoff");
        let ready = Arc::new(AtomicBool::new(false));
        let ingress = Arc::new(FairV2Ingress::new(1, 1024 * 1024, 1024 * 1024, 0, 0));
        ingress
            .configure_roster(std::iter::empty())
            .expect("configure untrusted test lane");
        let output_guard = ConsensusOutputGuard::isolated();

        // Force the real adapter constructor to fail on an existing directory
        // where it requires a WAL file. Runtime, service, and later startup
        // failures return through the same armed token/runner-guard boundary.
        let failure_guard = V2RunnerFailureGuard::new(Arc::clone(&output_guard));
        let proofs = keys
            .iter()
            .map(|key| {
                iroha_crypto::bls_normal_pop_prove(key.private_key())
                    .expect("validator proof of possession")
            })
            .collect::<Vec<_>>();
        let verified = super::super::v2::VerifiedHeightContext::genesis(context.clone(), proofs)
            .expect("verified constructor context");
        let directory = TempDir::new().expect("temporary directory");
        let constructor = SumeragiV2Adapter::open_deferred_status(
            directory.path(),
            verified,
            None,
            Generation::new(context.height),
            [0xA7; 32],
            AdapterFingerprints {
                node: Hash::new(b"failed constructor node"),
                build: Hash::new(b"failed constructor build"),
                config: Hash::new(b"failed constructor config"),
            },
            DeferredAdmissionOrdinalSource::new(0),
        );
        assert!(
            constructor.is_err(),
            "a directory cannot be opened as a WAL"
        );
        drop(activation);
        drop(failure_guard);

        assert!(output_guard.restart_required());
        assert!(!ready.load(Ordering::Acquire));
        assert!(matches!(
            ingress.try_push(InboundBlockMessage::new(valid_ingress_probe(), None)),
            Err(FairV2IngressPushError::Closed(_))
        ));
        let stalled = super::super::status::v2_status().expect("stalled predecessor status");
        assert_eq!(stalled.height, context.height);
        assert_eq!(
            stalled.liveness.work.successor_height,
            wire::SumeragiV2LocalWorkStage::Running
        );
        assert_eq!(
            stalled
                .liveness
                .last_progress
                .expect("application remains the final progress marker")
                .transition,
            wire::SumeragiV2ProgressTransition::Applied,
            "dropping an incomplete activation token must not claim successor activation"
        );
        super::super::status::clear_v2_status();
    }

    #[test]
    fn status_guard_retains_failure_snapshot_and_clears_clean_shutdown() {
        let _guard = super::super::status::rbc_status_test_guard();
        super::super::status::clear_v2_status();
        let (context, _) = context();

        let failure_status_guard = V2StatusClearGuard::new();
        publish_applied_runner_status(&context);
        super::super::status::mark_v2_restart_required();
        drop(failure_status_guard);
        let retained = super::super::status::v2_status().expect("retained failure snapshot");
        assert_eq!(retained.height, context.height);
        assert!(retained.restart_required);

        let mut clean_status_guard = V2StatusClearGuard::new();
        publish_applied_runner_status(&context);
        clean_status_guard.clear_on_drop();
        drop(clean_status_guard);
        assert!(super::super::status::v2_status().is_none());
    }

    #[test]
    fn ingress_capacity_error_preserves_message_and_byte_units() {
        let (context, _) = context();
        let validators = context
            .roster
            .iter()
            .take(2)
            .map(|validator| validator.validator.clone())
            .collect::<Vec<_>>();

        let count_error = FairV2Ingress::new(8, 3 * 1024, 1024, 0, 0)
            .configure_roster(validators.clone())
            .expect_err("two validators require ten protected message slots");
        assert!(matches!(
            ingress_capacity_error(count_error),
            V2RunnerError::IngressCapacity {
                configured: 8,
                required: 10,
            }
        ));

        let byte_error = FairV2Ingress::new(10, 2 * 1024, 1024, 0, 0)
            .configure_roster(validators)
            .expect_err("two validators and untrusted traffic require three byte partitions");
        assert!(matches!(
            ingress_capacity_error(byte_error),
            V2RunnerError::IngressByteCapacity {
                configured: 2048,
                required: 3072,
            }
        ));
    }

    #[test]
    fn ingress_guard_fails_closed_during_unwind() {
        let ready = Arc::new(AtomicBool::new(true));
        let ingress = Arc::new(FairV2Ingress::new(1, 1024 * 1024, 1024 * 1024, 0, 0));
        ingress
            .configure_roster(std::iter::empty())
            .expect("configure untrusted test lane");
        ingress.open().expect("open test ingress");
        let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe({
            let ready = Arc::clone(&ready);
            let ingress = Arc::clone(&ingress);
            move || {
                let _guard = V2IngressClearGuard::new(Arc::clone(&ready), Arc::clone(&ingress));
                ingress.open().expect("reopen inside guarded runner");
                ready.store(true, Ordering::Release);
                panic!("model runner panic");
            }
        }));
        assert!(unwind.is_err());
        assert!(!ready.load(Ordering::Acquire));
        assert!(matches!(
            ingress.try_push(InboundBlockMessage::new(valid_ingress_probe(), None)),
            Err(FairV2IngressPushError::Closed(_))
        ));
    }

    #[test]
    fn runner_failure_guard_latches_restart_required_during_unwind() {
        let output_guard = ConsensusOutputGuard::isolated();
        let admitted_output = output_guard.acquire().expect("admit earlier output");
        let unwind = std::panic::catch_unwind({
            let output_guard = Arc::clone(&output_guard);
            move || {
                let _failure_guard = V2RunnerFailureGuard::new(output_guard);
                panic!("model runner panic before production services start");
            }
        });

        assert!(unwind.is_err(), "runner panic must continue unwinding");
        assert!(output_guard.restart_required());
        assert!(output_guard.acquire().is_none());
        drop(admitted_output);
        assert!(output_guard.acquire().is_none());
    }

    #[test]
    fn clean_runner_completion_leaves_output_guard_open() {
        let output_guard = ConsensusOutputGuard::isolated();
        let mut failure_guard = V2RunnerFailureGuard::new(Arc::clone(&output_guard));
        failure_guard.disarm();
        drop(failure_guard);

        assert!(!output_guard.restart_required());
        assert!(output_guard.acquire().is_some());
    }

    #[test]
    fn bound_block_sync_finalization_retires_only_nonfatal_no_output_paths() {
        let output_guard = ConsensusOutputGuard::isolated();
        let calls = RefCell::new(Vec::new());
        let served = serve_block_sync_while_guarded(
            output_guard.as_ref(),
            || Ok(Some(())),
            |(), _permit| {
                calls.borrow_mut().push("post");
                Ok(())
            },
        );
        let posted = finalize_bound_block_sync_serve(
            served,
            || {
                calls.borrow_mut().push("volatile");
                Ok(())
            },
            |_| calls.borrow_mut().push("remote-rejection"),
        )
        .expect("posted response owns its bound runtime receipt");
        assert_eq!(posted, BoundBlockSyncServeOutcome::Posted);
        assert_eq!(
            calls.borrow().as_slice(),
            ["post"],
            "posted exact output, not VolatileTerminal, owns the runtime receipt"
        );
        calls.borrow_mut().clear();

        let served = serve_block_sync_while_guarded(
            output_guard.as_ref(),
            || Ok::<Option<()>, V2BlockSyncError>(None),
            |(), _permit| {
                calls.borrow_mut().push("unexpected-post");
                Ok(())
            },
        );
        let no_response = finalize_bound_block_sync_serve(
            served,
            || {
                calls.borrow_mut().push("volatile");
                Ok(())
            },
            |_| calls.borrow_mut().push("remote-rejection"),
        )
        .expect("no-response history retires through VolatileTerminal");
        assert_eq!(no_response, BoundBlockSyncServeOutcome::VolatileNoResponse);
        assert_eq!(calls.borrow().as_slice(), ["volatile"]);
        calls.borrow_mut().clear();

        let served = serve_block_sync_while_guarded(
            output_guard.as_ref(),
            || {
                Err::<Option<()>, _>(V2BlockSyncError::Wire(
                    wire::ValidationError::WrongHeightContext,
                ))
            },
            |(), _permit| {
                calls.borrow_mut().push("unexpected-post");
                Ok(())
            },
        );
        let remote_rejection = finalize_bound_block_sync_serve(
            served,
            || {
                calls.borrow_mut().push("volatile");
                Ok(())
            },
            |_| calls.borrow_mut().push("remote-rejection"),
        )
        .expect("remote rejection retires through VolatileTerminal");
        assert_eq!(
            remote_rejection,
            BoundBlockSyncServeOutcome::VolatileRemoteRejection
        );
        assert_eq!(
            calls.borrow().as_slice(),
            ["volatile", "remote-rejection"],
            "runtime retirement precedes nonfatal rejection observation"
        );
        calls.borrow_mut().clear();

        let fatal = finalize_bound_block_sync_serve(
            Err(V2BlockSyncError::RestartRequired),
            || {
                calls.borrow_mut().push("volatile");
                Ok(())
            },
            |_| calls.borrow_mut().push("remote-rejection"),
        );
        assert!(matches!(
            fatal,
            Err(V2RunnerError::BlockSync(V2BlockSyncError::RestartRequired))
        ));
        assert!(
            calls.borrow().is_empty(),
            "fatal service failure leaves the durable Runtime owner for restart recovery"
        );
    }

    #[test]
    fn prelatched_historical_serve_invokes_no_signer_cache_or_network() {
        let output_guard = ConsensusOutputGuard::isolated();
        output_guard.activate_restart_required();
        let signer_calls = Cell::new(0_u8);
        let cache_writes = Cell::new(0_u8);
        let network_posts = Cell::new(0_u8);

        let result = serve_block_sync_while_guarded(
            output_guard.as_ref(),
            || {
                signer_calls.set(signer_calls.get().saturating_add(1));
                cache_writes.set(cache_writes.get().saturating_add(1));
                Ok(Some(()))
            },
            |(), _permit| {
                network_posts.set(network_posts.get().saturating_add(1));
                Ok(())
            },
        );

        assert!(matches!(result, Err(V2BlockSyncError::RestartRequired)));
        assert_eq!(signer_calls.get(), 0);
        assert_eq!(cache_writes.get(), 0);
        assert_eq!(network_posts.get(), 0);
    }
}
