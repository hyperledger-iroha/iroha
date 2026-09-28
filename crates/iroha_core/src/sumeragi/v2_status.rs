//! Process-local Sumeragi v2 consensus status publication.
//!
//! Consensus state is published exclusively as the exact reducer-owned
//! [`SumeragiV2Status`]. This module also owns the v2 liveness watchdog, the
//! successor-activation guards, the canonical-transition gate and the archival
//! consensus-mode labels. Non-consensus Nexus economics, settlement, lane and
//! queue diagnostics live in [`crate::status`].
use super::{
    FairV2Ingress,
    v2_core::{
        CanonicalIdentityProjection, ProductionAppliedSuccessorTraceProjection,
        ProductionDurablePredecessorIdentityProjection,
        ProductionRecoveredSuccessorTraceProjection,
        ProductionSuccessorPredecessorBindingProjection, ProductionSuccessorSnapshotProjection,
        ProductionSuccessorStartupLifecycleProjection, SUCCESSOR_AUTHORITY_APPLIED,
        SUCCESSOR_AUTHORITY_RECOVERED_COMPLETE_TIP,
        SUCCESSOR_AUTHORITY_RECOVERED_DECIDED_COMPLETE_TIP, SUCCESSOR_AUTHORITY_SNAPSHOT_BOOTSTRAP,
        SUCCESSOR_LIFECYCLE_BEGIN, SUCCESSOR_LIFECYCLE_FAIL, SUCCESSOR_MARKER_ACTIVATED,
        SUCCESSOR_STAGE_COMPLETE, SUCCESSOR_STAGE_QUEUED, SUCCESSOR_STAGE_RUNNING,
        check_production_applied_successor_transition,
        check_production_recovered_successor_transition,
        check_production_successor_startup_lifecycle_transition,
    },
    v2_effects::{EffectExecutorStatus, PendingKuraApplyRecoveryStage},
    v2_first_release_recovery::RetiredRecoveredCompleteTipActivationAuthorityV1,
    v2_recovery::{
        DurableSuccessorActivationAuthority, DurableV2PredecessorIdentity,
        SnapshotSuccessorActivationAuthority, successor_context_refinement_projection,
    },
    v2_runtime::RuntimeQueueLaneSnapshot,
};
use crate::status::lock_operator_status_slot;
#[cfg(test)]
use crate::status::{
    RBC_STATUS_TEST_LOCK, TestLock, TestLockGuard, rbc_status_test_guard, reentrant_test_guard,
    try_reentrant_test_guard,
};
use core::sync::atomic::{AtomicU64, Ordering};
use iroha_data_model::block::consensus_v2::{
    BeaconHorizonStatusV1, BlockSubject, ConsensusRound, GlobalPhase, HeightContextId,
    SumeragiV2BodyState, SumeragiV2LivenessBlocker, SumeragiV2LocalWorkStage,
    SumeragiV2OutboundIntentKind, SumeragiV2OutboundIntentStage, SumeragiV2ProgressTransition,
    SumeragiV2ProgressTransitionStatus, SumeragiV2QueueKind, SumeragiV2QueueStatus,
    SumeragiV2Status, SumeragiV2StatusPhase, SumeragiV2TimeoutQuorumStatus,
    SumeragiV2VoteQuorumStatus,
};
use std::{
    sync::{Arc, Mutex, OnceLock, Weak},
    time::{Duration, Instant},
};
use thiserror::Error;
static SUMERAGI_V2_STATUS: OnceLock<Mutex<Option<SumeragiV2Status>>> = OnceLock::new();
static SUMERAGI_V2_EFFECT_STATUS: OnceLock<Mutex<Option<EffectExecutorStatus>>> = OnceLock::new();
static SUMERAGI_V2_PROGRESS_CLOCK: OnceLock<Mutex<Option<V2ProgressClock>>> = OnceLock::new();
static SUMERAGI_V2_WATCHDOG_REVISION: AtomicU64 = AtomicU64::new(0);
fn bump_v2_watchdog_revision() {
    SUMERAGI_V2_WATCHDOG_REVISION.fetch_add(1, Ordering::Release);
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct V2StatusOwner {
    height_context_id: HeightContextId,
    height: u64,
}
impl V2StatusOwner {
    const fn from_status(status: &SumeragiV2Status) -> Self {
        Self {
            height_context_id: status.height_context_id,
            height: status.height,
        }
    }
}
struct V2NetworkIngressRegistration {
    owner: V2StatusOwner,
    ingress: Weak<FairV2Ingress>,
}
struct V2EffectCompletionRegistration {
    owner: V2StatusOwner,
    observer: Weak<dyn V2IoCompletionQueueObserver>,
}
/// Beacon horizon published by the serialized activation of one exact height.
static SUMERAGI_V2_BEACON_HORIZON: OnceLock<Mutex<Option<(V2StatusOwner, BeaconHorizonStatusV1)>>> =
    OnceLock::new();
static SUMERAGI_V2_NETWORK_INGRESS: OnceLock<Mutex<Option<V2NetworkIngressRegistration>>> =
    OnceLock::new();
static SUMERAGI_V2_EFFECT_COMPLETION_OBSERVER: OnceLock<
    Mutex<Option<V2EffectCompletionRegistration>>,
> = OnceLock::new();
static MODE_TAG: OnceLock<Mutex<String>> = OnceLock::new();
static STAGED_MODE_TAG: OnceLock<Mutex<Option<String>>> = OnceLock::new();
static STAGED_MODE_ACTIVATION_HEIGHT: OnceLock<Mutex<Option<u64>>> = OnceLock::new();
static MODE_ACTIVATION_LAG_BLOCKS: OnceLock<Mutex<Option<u64>>> = OnceLock::new();
#[cfg(test)]
mod archival_status_tests {
    use crate::sumeragi::consensus::PERMISSIONED_TAG;
    #[test]
    fn archival_mode_tags_roundtrip_without_changing_v2_status() {
        let _guard = super::mode_tags_test_guard();
        super::clear_v2_status();
        super::set_mode_tags(PERMISSIONED_TAG, Some("staged"), Some(9));
        assert_eq!(
            super::mode_tags(),
            (
                PERMISSIONED_TAG.to_owned(),
                Some("staged".to_owned()),
                Some(9),
                None,
            )
        );
        assert_eq!(super::v2_status(), None);
        super::set_mode_tags("", None, None);
    }
    #[test]
    fn v2_operational_scalars_saturate_for_wire_diagnostics() {
        assert_eq!(super::bounded_u32(7), 7);
        assert_eq!(super::bounded_u32(usize::MAX), u32::MAX);
        assert_eq!(
            super::age_ms(Some(std::time::Duration::from_millis(19))),
            Some(19)
        );
        assert_eq!(super::age_ms(None), None);
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct V2ProgressMarker {
    generation: u64,
    round: ConsensusRound,
    transition: SumeragiV2ProgressTransition,
}
impl From<SumeragiV2ProgressTransitionStatus> for V2ProgressMarker {
    fn from(status: SumeragiV2ProgressTransitionStatus) -> Self {
        Self {
            generation: status.generation,
            round: status.round,
            transition: status.transition,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct V2ProgressObservation {
    marker: Option<V2ProgressMarker>,
    prepare_quorums: Vec<SumeragiV2VoteQuorumStatus>,
    commit_quorums: Vec<SumeragiV2VoteQuorumStatus>,
    timeout_quorums: Vec<SumeragiV2TimeoutQuorumStatus>,
    height_rank: V2HeightProgressRank,
}
impl V2ProgressObservation {
    fn from_status(status: &SumeragiV2Status) -> Self {
        Self {
            marker: status.liveness.last_progress.map(Into::into),
            prepare_quorums: status.liveness.prepare_quorums.clone(),
            commit_quorums: status.liveness.commit_quorums.clone(),
            timeout_quorums: status.liveness.timeout_quorums.clone(),
            height_rank: V2HeightProgressRank::from_status(status),
        }
    }
    /// Return the newly observed transition, including a repeated vote-admission
    /// transition whose exact partial pool grew.
    fn transition_since(&self, next: &Self) -> Option<SumeragiV2ProgressTransition> {
        let marker = next.marker?;
        if self.marker != next.marker {
            return Some(marker.transition);
        }
        match marker.transition {
            SumeragiV2ProgressTransition::PrepareVoteAdmitted
                if self.prepare_quorums != next.prepare_quorums =>
            {
                Some(marker.transition)
            }
            SumeragiV2ProgressTransition::CommitVoteAdmitted
                if self.commit_quorums != next.commit_quorums =>
            {
                Some(marker.transition)
            }
            SumeragiV2ProgressTransition::TimeoutVoteAdmitted
                if self.timeout_quorums != next.timeout_quorums =>
            {
                Some(marker.transition)
            }
            _ => None,
        }
    }
}
/// Bounded semantic high-water for one height.
///
/// View and reducer generation are deliberately absent. A timeout certificate
/// may replace volatile pools and the durable locked Commit intent may then
/// reconstruct exactly the same partial quorum. Only a strictly greater
/// protocol stage or equal-vote signer component advances this rank, so that
/// cycle cannot refresh the height-wide watchdog indefinitely.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct V2HeightProgressRank {
    stage: u8,
    prepare_signers: u32,
    commit_signers: u32,
}
impl V2HeightProgressRank {
    fn from_status(status: &SumeragiV2Status) -> Self {
        let mut rank = Self::default();
        if let Some(marker) = status.liveness.last_progress {
            rank.stage = progress_transition_rank(marker.transition);
        }
        rank.stage = rank.stage.max(match status.body_state {
            SumeragiV2BodyState::Missing => 0,
            SumeragiV2BodyState::Reconstructing => 1,
            SumeragiV2BodyState::Stored => 3,
            SumeragiV2BodyState::Validated => 4,
            SumeragiV2BodyState::PendingApply => 10,
            SumeragiV2BodyState::Applied => 11,
        });
        rank.stage = rank.stage.max(match status.phase {
            SumeragiV2StatusPhase::AwaitingProposal => 0,
            SumeragiV2StatusPhase::ReconstructingPayload => 1,
            SumeragiV2StatusPhase::ValidatingPayload => 3,
            SumeragiV2StatusPhase::Prepare => 4,
            SumeragiV2StatusPhase::Commit => 7,
            SumeragiV2StatusPhase::PendingApply => 10,
        });
        if status.highest_prepare_qc.is_some() {
            rank.stage = rank.stage.max(6);
        }
        if status.locked_prepare_qc.is_some() {
            rank.stage = rank.stage.max(7);
        }
        for quorum in &status.liveness.prepare_quorums {
            rank.stage = rank.stage.max(5);
            rank.prepare_signers = rank.prepare_signers.max(quorum.signer_count);
        }
        for quorum in &status.liveness.commit_quorums {
            rank.stage = rank.stage.max(8);
            rank.commit_signers = rank.commit_signers.max(quorum.signer_count);
        }
        for intent in &status.liveness.outbound_intents {
            rank.stage = rank.stage.max(match intent.kind {
                SumeragiV2OutboundIntentKind::Proposal => 1,
                SumeragiV2OutboundIntentKind::PrepareVote => 5,
                SumeragiV2OutboundIntentKind::CommitVote => 8,
                SumeragiV2OutboundIntentKind::PrepareQc => 6,
                SumeragiV2OutboundIntentKind::CommitQc => 9,
                SumeragiV2OutboundIntentKind::TimeoutVote
                | SumeragiV2OutboundIntentKind::TimeoutCertificate => 0,
            });
        }
        rank
    }
    /// Merge `next` into this height-wide high-water and report a strict gain.
    fn absorb(&mut self, next: Self) -> bool {
        let advanced = next.stage > self.stage
            || next.prepare_signers > self.prepare_signers
            || next.commit_signers > self.commit_signers;
        self.stage = self.stage.max(next.stage);
        self.prepare_signers = self.prepare_signers.max(next.prepare_signers);
        self.commit_signers = self.commit_signers.max(next.commit_signers);
        advanced
    }
    fn strictly_advances(self, previous: Self) -> bool {
        self.stage > previous.stage
            || self.prepare_signers > previous.prepare_signers
            || self.commit_signers > previous.commit_signers
    }
}
const fn progress_transition_rank(transition: SumeragiV2ProgressTransition) -> u8 {
    match transition {
        SumeragiV2ProgressTransition::TimeoutVoteAdmitted
        | SumeragiV2ProgressTransition::TimeoutCertificateInstalled
        | SumeragiV2ProgressTransition::RecoveryReplayed
        // Activation is the rank-zero boundary of the newly active height.
        // The finalized predecessor retains its terminal Applied rank while
        // the successor must still earn every height-local progress stage.
        | SumeragiV2ProgressTransition::SuccessorHeightActivated => 0,
        SumeragiV2ProgressTransition::ProposalAdmitted => 1,
        SumeragiV2ProgressTransition::BodyAvailable => 2,
        SumeragiV2ProgressTransition::BodyStored => 3,
        SumeragiV2ProgressTransition::BodyValidated => 4,
        SumeragiV2ProgressTransition::PrepareVoteAdmitted => 5,
        SumeragiV2ProgressTransition::PrepareQuorum => 6,
        SumeragiV2ProgressTransition::LockInstalled => 7,
        SumeragiV2ProgressTransition::CommitVoteAdmitted => 8,
        SumeragiV2ProgressTransition::CommitQuorum => 9,
        SumeragiV2ProgressTransition::DecisionPersisted => 10,
        SumeragiV2ProgressTransition::Applied => 11,
    }
}
#[derive(Clone, Debug)]
struct V2ProgressClock {
    owner: V2StatusOwner,
    observation: V2ProgressObservation,
    height_progress_high_water: V2HeightProgressRank,
    status_captured_at: Instant,
    last_transition_at: Option<Instant>,
    height_progress_at: Instant,
    watchdog_threshold: Option<Duration>,
}
impl V2ProgressClock {
    fn new(status: &SumeragiV2Status, now: Instant, watchdog_threshold: Option<Duration>) -> Self {
        let observation = V2ProgressObservation::from_status(status);
        Self {
            owner: V2StatusOwner::from_status(status),
            height_progress_high_water: observation.height_rank,
            observation,
            status_captured_at: now,
            last_transition_at: status.liveness.last_progress.is_some().then_some(now),
            height_progress_at: now,
            watchdog_threshold,
        }
    }
    fn observe(
        &mut self,
        status: &SumeragiV2Status,
        now: Instant,
        watchdog_threshold: Option<Duration>,
    ) {
        if self.owner != V2StatusOwner::from_status(status) {
            *self = Self::new(status, now, watchdog_threshold);
            return;
        }
        if watchdog_threshold.is_some() {
            self.watchdog_threshold = watchdog_threshold;
        }
        let next = V2ProgressObservation::from_status(status);
        if self.observation.transition_since(&next).is_some() {
            self.last_transition_at = Some(now);
        }
        if self.height_progress_high_water.absorb(next.height_rank) {
            self.height_progress_at = now;
        }
        self.observation = next;
        self.status_captured_at = now;
    }
    fn overlay_ages(
        &self,
        status: &mut SumeragiV2Status,
        now: Instant,
    ) -> Option<(Duration, Option<Duration>, Instant)> {
        if self.owner != V2StatusOwner::from_status(status) {
            return None;
        }
        let no_progress_age = now.saturating_duration_since(self.height_progress_at);
        status.liveness.no_progress_age_ms = duration_ms(no_progress_age);
        let marker = status.liveness.last_progress.map(V2ProgressMarker::from);
        if marker == self.observation.marker
            && let (Some(last_progress), Some(started_at)) =
                (&mut status.liveness.last_progress, self.last_transition_at)
        {
            last_progress.age_ms = duration_ms(now.saturating_duration_since(started_at));
        }
        let status_age = now.saturating_duration_since(self.status_captured_at);
        for queue in &mut status.liveness.queues {
            if matches!(
                queue.queue,
                SumeragiV2QueueKind::Ingress
                    | SumeragiV2QueueKind::DeferredNormal
                    | SumeragiV2QueueKind::DeferredProgress
                    | SumeragiV2QueueKind::DeferredCompletion
            ) {
                age_queue_at_read(queue, status_age);
            }
        }
        Some((
            no_progress_age,
            self.watchdog_threshold,
            self.height_progress_at,
        ))
    }
}
fn duration_ms(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}
fn saturating_duration_add(left: Duration, right: Duration) -> Duration {
    left.checked_add(right).unwrap_or(Duration::MAX)
}
fn set_v2_status_at(status: SumeragiV2Status, now: Instant) {
    #[cfg(test)]
    let _guard = rbc_status_test_guard();
    let owner = V2StatusOwner::from_status(&status);
    let watchdog_threshold = latest_v2_effect_status()
        .filter(|effect_status| {
            effect_status.height_context_id == owner.height_context_id
                && effect_status.height == owner.height
        })
        .map(|effect_status| effect_status.watchdog_threshold);
    let mut status_slot = SUMERAGI_V2_STATUS
        .get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let mut clock_slot = SUMERAGI_V2_PROGRESS_CLOCK
        .get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let owner_changed = status_slot.as_ref().map(V2StatusOwner::from_status) != Some(owner);
    match &mut *clock_slot {
        Some(clock) => clock.observe(&status, now, watchdog_threshold),
        None => {
            *clock_slot = Some(V2ProgressClock::new(&status, now, watchdog_threshold));
        }
    }
    *status_slot = Some(status);
    drop(clock_slot);
    drop(status_slot);
    if owner_changed {
        bump_v2_watchdog_revision();
    }
}
/// Publish the exact protocol-v2 reducer snapshot served by Torii.
pub fn set_v2_status(status: SumeragiV2Status) {
    set_v2_status_at(status, Instant::now());
}
/// Publish the latest local effect/runtime ownership snapshot.
pub(crate) fn set_v2_effect_status(status: EffectExecutorStatus) {
    #[cfg(test)]
    let _guard = rbc_status_test_guard();
    let owner = V2StatusOwner {
        height_context_id: status.height_context_id,
        height: status.height,
    };
    let watchdog_threshold = status.watchdog_threshold;
    let watchdog_boundary_changed = {
        let mut slot = SUMERAGI_V2_EFFECT_STATUS
            .get_or_init(|| Mutex::new(None))
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let changed = slot.as_ref().map(|previous| {
            (
                previous.height_context_id,
                previous.height,
                previous.watchdog_threshold,
            )
        }) != Some((owner.height_context_id, owner.height, watchdog_threshold));
        *slot = Some(status);
        changed
    };
    let published_owner = SUMERAGI_V2_STATUS.get().and_then(|slot| {
        slot.lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
            .map(V2StatusOwner::from_status)
    });
    if published_owner == Some(owner)
        && let Some(slot) = SUMERAGI_V2_PROGRESS_CLOCK.get()
    {
        let mut clock = slot
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(clock) = clock.as_mut().filter(|clock| clock.owner == owner) {
            clock.watchdog_threshold = Some(watchdog_threshold);
        }
    }
    if watchdog_boundary_changed {
        bump_v2_watchdog_revision();
    }
}
/// Read-only view of the live bounded I/O completion owner.
///
/// The status registry retains only a weak reference, so diagnostics cannot
/// extend a height worker's lifetime or interfere with finalized teardown.
pub(crate) trait V2IoCompletionQueueObserver: Send + Sync {
    /// Snapshot exact completion ownership at `now`.
    fn completion_queue_snapshot(&self, now: Instant) -> RuntimeQueueLaneSnapshot;
}
/// Register the live height worker completion owner for read-time overlays.
pub(crate) fn set_v2_effect_completion_observer<T>(
    height_context_id: HeightContextId,
    height: u64,
    observer: &Arc<T>,
) where
    T: V2IoCompletionQueueObserver + 'static,
{
    #[cfg(test)]
    let _guard = rbc_status_test_guard();
    let observer = Arc::clone(observer);
    let observer: Arc<dyn V2IoCompletionQueueObserver> = observer;
    *SUMERAGI_V2_EFFECT_COMPLETION_OBSERVER
        .get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) =
        Some(V2EffectCompletionRegistration {
            owner: V2StatusOwner {
                height_context_id,
                height,
            },
            observer: Arc::downgrade(&observer),
        });
}
/// Diagnostic contract failure while transferring one applied height to its
/// exact active successor.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub(crate) enum V2SuccessorActivationError {
    /// No finalized-height reducer status is currently published.
    #[error("no finalized Sumeragi v2 height status is published")]
    MissingFinalizedStatus,
    /// The published status belongs to another height.
    #[error("published Sumeragi v2 height {actual} does not match finalized height {expected}")]
    FinalizedHeightMismatch {
        /// Height whose activation handoff is being completed.
        expected: u64,
        /// Height currently owned by the status registry.
        actual: u64,
    },
    /// The finalized-height handoff is not at the required one-shot stage.
    #[error(
        "Sumeragi v2 successor handoff for height {height} is {actual:?}, expected {expected:?}"
    )]
    WorkStageMismatch {
        /// Finalized height whose handoff was inspected.
        height: u64,
        /// Required predecessor-owned stage.
        expected: SumeragiV2LocalWorkStage,
        /// Currently published predecessor-owned stage.
        actual: SumeragiV2LocalWorkStage,
    },
    /// The predecessor does not carry the exact terminal application witness
    /// from which a successor handoff may be derived.
    #[error("Sumeragi v2 predecessor height {0} is not durably applied")]
    PredecessorNotApplied(u64),
    /// The finalized height has no representable successor.
    #[error("Sumeragi v2 finalized height {0} has no representable successor")]
    SuccessorHeightOverflow(u64),
    /// The prepared reducer status is not the exact next height.
    #[error(
        "prepared Sumeragi v2 successor height {actual} does not match expected height {expected}"
    )]
    SuccessorHeightMismatch {
        /// Exact next height derived from the finalized predecessor.
        expected: u64,
        /// Height carried by the prepared successor status.
        actual: u64,
    },
    /// The prepared successor does not name the finalized height as its parent.
    #[error("prepared Sumeragi v2 successor names committed height {actual}, expected {expected}")]
    SuccessorParentMismatch {
        /// Finalized predecessor height.
        expected: u64,
        /// Committed height reported by the prepared successor.
        actual: u64,
    },
    /// The prepared successor belongs to another authenticated height context.
    #[error(
        "prepared Sumeragi v2 successor context {actual:?} does not match expected context {expected:?}"
    )]
    SuccessorContextMismatch {
        /// Exact successor context authenticated by recovery or construction.
        expected: HeightContextId,
        /// Context carried by the prepared reducer snapshot.
        actual: HeightContextId,
    },
    /// The prepared successor snapshot lacks its adapter-owned activation
    /// witness or binds that witness to another reducer incarnation.
    #[error("prepared Sumeragi v2 successor lacks its exact activation marker")]
    SuccessorMarkerMismatch,
    /// Audited snapshot recovery attempted to replace a status which should
    /// have remained unpublished throughout successor startup.
    #[error("recovered Sumeragi v2 successor found an already published height {0}")]
    RecoveredStatusAlreadyPublished(u64),
    /// Primitive successor fields failed the shared production/Verus decision kernel.
    #[error("Sumeragi v2 successor activation failed the production refinement kernel")]
    RefinementRejected,
    /// The retired CompleteTip token no longer authenticates the canonical H+1
    /// ledger frame and prepared status together.
    #[error("retired CompleteTip authority does not authenticate the prepared successor")]
    RecoveredCompleteTipAuthorityMismatch,
}
const fn successor_stage_projection(stage: SumeragiV2LocalWorkStage) -> u8 {
    match stage {
        SumeragiV2LocalWorkStage::Queued => SUCCESSOR_STAGE_QUEUED,
        SumeragiV2LocalWorkStage::Running => SUCCESSOR_STAGE_RUNNING,
        SumeragiV2LocalWorkStage::Complete => SUCCESSOR_STAGE_COMPLETE,
        _ => super::v2_core::SUCCESSOR_STAGE_NONE,
    }
}
fn successor_snapshot_refinement_projection(
    expected_context_id: HeightContextId,
    successor: &SumeragiV2Status,
) -> ProductionSuccessorSnapshotProjection {
    let expected_context_id = successor_context_refinement_projection(expected_context_id);
    let published_context_id = successor_context_refinement_projection(successor.height_context_id);
    let marker = successor.liveness.last_progress;
    ProductionSuccessorSnapshotProjection {
        expected_context_id,
        published_context_id,
        height: successor.height,
        last_committed_height: successor.last_committed_height,
        view: successor.view,
        generation: successor.liveness.generation,
        marker_context_id: marker.map_or_else(CanonicalIdentityProjection::zero, |marker| {
            successor_context_refinement_projection(marker.round.context_id)
        }),
        marker_height: marker.map_or(0, |marker| marker.round.height),
        marker_view: marker.map_or(0, |marker| marker.round.view),
        marker_generation: marker.map_or(0, |marker| marker.generation),
        marker_kind: marker.map_or(0, |marker| {
            u8::from(marker.transition == SumeragiV2ProgressTransition::SuccessorHeightActivated)
                * SUCCESSOR_MARKER_ACTIVATED
        }),
        marker_age_ms: marker.map_or(u64::MAX, |marker| marker.age_ms),
    }
}
fn update_v2_successor_work_stage_at(
    height: u64,
    expected: SumeragiV2LocalWorkStage,
    stage: SumeragiV2LocalWorkStage,
    now: Instant,
) -> Result<(), V2SuccessorActivationError> {
    #[cfg(test)]
    let _guard = rbc_status_test_guard();
    let Some(mut status) = SUMERAGI_V2_STATUS.get().and_then(|slot| {
        slot.lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }) else {
        return Err(V2SuccessorActivationError::MissingFinalizedStatus);
    };
    validate_v2_predecessor_status(&status, height, expected)?;
    status.liveness.work.successor_height = stage;
    set_v2_status_at(status, now);
    Ok(())
}
fn validate_v2_predecessor_status(
    status: &SumeragiV2Status,
    height: u64,
    expected: SumeragiV2LocalWorkStage,
) -> Result<(), V2SuccessorActivationError> {
    if status.height != height {
        return Err(V2SuccessorActivationError::FinalizedHeightMismatch {
            expected: height,
            actual: status.height,
        });
    }
    let applied = status.phase == SumeragiV2StatusPhase::PendingApply
        && status.body_state == SumeragiV2BodyState::Applied
        && status.liveness.work.application == SumeragiV2LocalWorkStage::Complete
        && matches!(
            status.liveness.last_progress,
            Some(marker)
                if marker.generation == status.liveness.generation
                    && marker.round.context_id == status.height_context_id
                    && marker.round.height == status.height
                    && marker.round.view == status.view
                    && marker.transition == SumeragiV2ProgressTransition::Applied
        );
    if !applied {
        return Err(V2SuccessorActivationError::PredecessorNotApplied(height));
    }
    if status.liveness.work.successor_height != expected {
        return Err(V2SuccessorActivationError::WorkStageMismatch {
            height,
            expected,
            actual: status.liveness.work.successor_height,
        });
    }
    Ok(())
}
/// Publish the start of runner-owned successor construction.
///
/// The reducer can prove only that application completed and therefore reports
/// the handoff as queued. The serialized runner changes that exact owner to
/// running before any fallible successor construction. Dropping the caller's
/// activation token leaves this stage visible and does not claim activation.
pub(crate) fn begin_v2_successor_activation(
    predecessor: DurableV2PredecessorIdentity,
) -> Result<(), V2SuccessorActivationError> {
    let height = predecessor.height();
    let Some(status) = SUMERAGI_V2_STATUS.get().and_then(|slot| {
        slot.lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }) else {
        return Err(V2SuccessorActivationError::MissingFinalizedStatus);
    };
    validate_v2_predecessor_status(&status, height, SumeragiV2LocalWorkStage::Queued)?;
    let lifecycle = ProductionSuccessorStartupLifecycleProjection {
        transition_kind: SUCCESSOR_LIFECYCLE_BEGIN,
        authority_kind: SUCCESSOR_AUTHORITY_APPLIED,
        status_height: height,
        stage_before: successor_stage_projection(status.liveness.work.successor_height),
        stage_after: SUCCESSOR_STAGE_RUNNING,
        published_height_before: status.height,
        published_height_after: status.height,
        restart_required_before: status.restart_required,
        restart_required_after: status.restart_required,
    };
    let Some(checked_lifecycle) =
        check_production_successor_startup_lifecycle_transition(lifecycle)
    else {
        return Err(V2SuccessorActivationError::RefinementRejected);
    };
    let _authorized_lifecycle = checked_lifecycle.into_projection();
    update_v2_successor_work_stage_at(
        height,
        SumeragiV2LocalWorkStage::Queued,
        SumeragiV2LocalWorkStage::Running,
        Instant::now(),
    )
}
fn validate_v2_successor_snapshot(
    finalized_height: u64,
    expected_successor_context_id: HeightContextId,
    successor: &SumeragiV2Status,
) -> Result<(), V2SuccessorActivationError> {
    validate_v2_successor_snapshot_commit_frontier(
        finalized_height,
        finalized_height,
        expected_successor_context_id,
        successor,
    )
}
// Only the sealed recovered-Decision publication may separate these frontiers.
fn validate_v2_successor_snapshot_commit_frontier(
    finalized_height: u64,
    expected_commit_height: u64,
    expected_successor_context_id: HeightContextId,
    successor: &SumeragiV2Status,
) -> Result<(), V2SuccessorActivationError> {
    let expected_successor_height = finalized_height.checked_add(1).ok_or(
        V2SuccessorActivationError::SuccessorHeightOverflow(finalized_height),
    )?;
    if successor.height != expected_successor_height {
        return Err(V2SuccessorActivationError::SuccessorHeightMismatch {
            expected: expected_successor_height,
            actual: successor.height,
        });
    }
    if successor.last_committed_height != expected_commit_height {
        return Err(V2SuccessorActivationError::SuccessorParentMismatch {
            expected: expected_commit_height,
            actual: successor.last_committed_height,
        });
    }
    if successor.height_context_id != expected_successor_context_id {
        return Err(V2SuccessorActivationError::SuccessorContextMismatch {
            expected: expected_successor_context_id,
            actual: successor.height_context_id,
        });
    }
    if !matches!(
        successor.liveness.last_progress,
        Some(marker)
            if marker.generation == successor.liveness.generation
                && marker.round.context_id == successor.height_context_id
                && marker.round.height == successor.height
                && marker.round.view == successor.view
                && marker.transition
                    == SumeragiV2ProgressTransition::SuccessorHeightActivated
                && marker.age_ms == 0
    ) {
        return Err(V2SuccessorActivationError::SuccessorMarkerMismatch);
    }
    Ok(())
}
fn activate_v2_successor_height_at(
    expected_predecessor: DurableV2PredecessorIdentity,
    authority: DurableSuccessorActivationAuthority,
    successor: SumeragiV2Status,
    now: Instant,
) -> Result<(), V2SuccessorActivationError> {
    #[cfg(test)]
    let _guard = rbc_status_test_guard();
    let (authority_predecessor, expected_successor_context_id) = authority.into_parts();
    let finalized_height = expected_predecessor.height();
    validate_v2_successor_snapshot(finalized_height, expected_successor_context_id, &successor)?;
    let predecessor_status = SUMERAGI_V2_STATUS
        .get()
        .and_then(|slot| {
            slot.lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone()
        })
        .ok_or(V2SuccessorActivationError::MissingFinalizedStatus)?;
    validate_v2_predecessor_status(
        &predecessor_status,
        finalized_height,
        SumeragiV2LocalWorkStage::Running,
    )?;
    let trace = ProductionAppliedSuccessorTraceProjection {
        authority_kind: SUCCESSOR_AUTHORITY_APPLIED,
        binding: ProductionSuccessorPredecessorBindingProjection {
            expected_predecessor: expected_predecessor.refinement_projection(),
            authority_predecessor: authority_predecessor.refinement_projection(),
            successor_context_id: successor_context_refinement_projection(
                expected_successor_context_id,
            ),
        },
        predecessor_status_height: predecessor_status.height,
        predecessor_stage_before: successor_stage_projection(
            predecessor_status.liveness.work.successor_height,
        ),
        predecessor_stage_after: SUCCESSOR_STAGE_COMPLETE,
        successor: successor_snapshot_refinement_projection(
            expected_successor_context_id,
            &successor,
        ),
    };
    let Some(checked_trace) = check_production_applied_successor_transition(trace) else {
        return Err(V2SuccessorActivationError::RefinementRejected);
    };
    let _authorized_trace = checked_trace.into_projection();
    // Validate the predecessor and publish Complete before replacing it with
    // the prepared successor snapshot. This is the sole accepted Running ->
    // Complete transition, so a repeated activation cannot refresh either
    // height's progress clock.
    update_v2_successor_work_stage_at(
        finalized_height,
        SumeragiV2LocalWorkStage::Running,
        SumeragiV2LocalWorkStage::Complete,
        now,
    )?;
    set_v2_status_at(successor, now);
    Ok(())
}
// Both recovered startup variants enter this projection-shaped helper only
// through their distinct consuming authorities. CompleteTip additionally
// reauthenticates its Kura-derived successor ledger immediately beforehand.
fn publish_recovered_v2_successor_height_at(
    authority_kind: u8,
    predecessor: ProductionDurablePredecessorIdentityProjection,
    snapshot_record_hash: CanonicalIdentityProjection,
    snapshot_height: u64,
    snapshot_block_hash: CanonicalIdentityProjection,
    expected_successor_context_id: HeightContextId,
    successor: SumeragiV2Status,
    now: Instant,
) -> Result<(), V2SuccessorActivationError> {
    #[cfg(test)]
    let _guard = rbc_status_test_guard();
    let finalized_height = if authority_kind == SUCCESSOR_AUTHORITY_SNAPSHOT_BOOTSTRAP {
        snapshot_height
    } else {
        predecessor.height
    };
    let expected_commit_height =
        if authority_kind == SUCCESSOR_AUTHORITY_RECOVERED_DECIDED_COMPLETE_TIP {
            finalized_height.checked_add(1).ok_or(
                V2SuccessorActivationError::SuccessorHeightOverflow(finalized_height),
            )?
        } else {
            finalized_height
        };
    validate_v2_successor_snapshot_commit_frontier(
        finalized_height,
        expected_commit_height,
        expected_successor_context_id,
        &successor,
    )?;
    let published = SUMERAGI_V2_STATUS.get().and_then(|slot| {
        slot.lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    });
    let trace = ProductionRecoveredSuccessorTraceProjection {
        authority_kind,
        predecessor,
        snapshot_record_hash,
        snapshot_height,
        snapshot_block_hash,
        authority_context_id: successor_context_refinement_projection(
            expected_successor_context_id,
        ),
        published_status_height_before: published.as_ref().map_or(0, |status| status.height),
        successor: successor_snapshot_refinement_projection(
            expected_successor_context_id,
            &successor,
        ),
    };
    let Some(checked_trace) = check_production_recovered_successor_transition(trace) else {
        if let Some(published) = published {
            return Err(V2SuccessorActivationError::RecoveredStatusAlreadyPublished(
                published.height,
            ));
        }
        return Err(V2SuccessorActivationError::RefinementRejected);
    };
    let _authorized_trace = checked_trace.into_projection();
    if let Some(published) = published {
        return Err(V2SuccessorActivationError::RecoveredStatusAlreadyPublished(
            published.height,
        ));
    }
    set_v2_status_at(successor, now);
    Ok(())
}
/// Publish the exact one-shot boundary at which a prepared successor becomes
/// live.
///
/// The runner calls this only after the successor adapter, runtime, services,
/// and startup work have succeeded, its live clocks are armed, and authenticated
/// ingress is open. The predecessor is first marked `Complete`; the visible
/// activation marker is then attached to the successor's own height,
/// generation, context, and view.
pub(crate) fn activate_v2_successor_height(
    expected_predecessor: DurableV2PredecessorIdentity,
    authority: DurableSuccessorActivationAuthority,
    successor: SumeragiV2Status,
) -> Result<(), V2SuccessorActivationError> {
    activate_v2_successor_height_at(expected_predecessor, authority, successor, Instant::now())
}
#[cfg(test)]
fn activate_recovered_complete_tip_v2_height_at(
    authority: RetiredRecoveredCompleteTipActivationAuthorityV1,
    successor: SumeragiV2Status,
    now: Instant,
) -> Result<(), V2SuccessorActivationError> {
    activate_recovered_complete_tip_v2_height_with_decision_at(authority, successor, None, now)
}
fn activate_recovered_complete_tip_v2_height_with_decision_at(
    authority: RetiredRecoveredCompleteTipActivationAuthorityV1,
    successor: SumeragiV2Status,
    decision: Option<super::v2::RecoveredSuccessorDecisionActivationAuthorityV1>,
    now: Instant,
) -> Result<(), V2SuccessorActivationError> {
    if !authority.authorizes_successor_status_with_decision(&successor, decision.as_ref()) {
        return Err(V2SuccessorActivationError::RecoveredCompleteTipAuthorityMismatch);
    }
    let predecessor = authority.predecessor().refinement_projection();
    let expected_successor_context_id = successor.height_context_id;
    let publication = publish_recovered_v2_successor_height_at(
        if decision.is_some() {
            SUCCESSOR_AUTHORITY_RECOVERED_DECIDED_COMPLETE_TIP
        } else {
            SUCCESSOR_AUTHORITY_RECOVERED_COMPLETE_TIP
        },
        predecessor,
        CanonicalIdentityProjection::zero(),
        0,
        CanonicalIdentityProjection::zero(),
        expected_successor_context_id,
        successor,
        now,
    );
    drop(authority);
    publication
}
/// Consume one retired canonical CompleteTip authority to publish its exact H+1 status.
///
/// The runner reaches this bridge only after the successor runtime has armed
/// its clocks and authenticated ingress is open. The token reopens and compares
/// its retained successor ledger before the existing checked recovered-status
/// transition can publish anything.
#[cfg(test)]
pub(in crate::sumeragi) fn activate_recovered_complete_tip_v2_height(
    authority: RetiredRecoveredCompleteTipActivationAuthorityV1,
    successor: SumeragiV2Status,
) -> Result<(), V2SuccessorActivationError> {
    activate_recovered_complete_tip_v2_height_at(authority, successor, Instant::now())
}
/// Publish a complete canonical tip's successor with its exact unapplied WAL Decision.
pub(in crate::sumeragi) fn activate_recovered_complete_tip_v2_height_with_decision(
    authority: RetiredRecoveredCompleteTipActivationAuthorityV1,
    successor: SumeragiV2Status,
    decision: Option<super::v2::RecoveredSuccessorDecisionActivationAuthorityV1>,
) -> Result<(), V2SuccessorActivationError> {
    activate_recovered_complete_tip_v2_height_with_decision_at(
        authority,
        successor,
        decision,
        Instant::now(),
    )
}
fn activate_snapshot_bootstrap_v2_height_at(
    authority: SnapshotSuccessorActivationAuthority,
    successor: SumeragiV2Status,
    now: Instant,
) -> Result<(), V2SuccessorActivationError> {
    let (snapshot_record_hash, snapshot_height, snapshot_block_hash, expected_successor_context_id) =
        authority.into_parts();
    publish_recovered_v2_successor_height_at(
        SUCCESSOR_AUTHORITY_SNAPSHOT_BOOTSTRAP,
        ProductionDurablePredecessorIdentityProjection::default(),
        super::v2_recovery::snapshot_record_refinement_projection(snapshot_record_hash),
        snapshot_height,
        super::v2_recovery::successor_block_refinement_projection(snapshot_block_hash),
        expected_successor_context_id,
        successor,
        now,
    )
}
/// Publish the authenticated first executable height after an audited snapshot.
///
/// This is the empty process-local status publication path for audited snapshot
/// bootstrap; the imported anchor is not a historical CommitQC or Kura finality
/// receipt.
pub(crate) fn activate_snapshot_bootstrap_v2_height(
    authority: SnapshotSuccessorActivationAuthority,
    successor: SumeragiV2Status,
) -> Result<(), V2SuccessorActivationError> {
    activate_snapshot_bootstrap_v2_height_at(authority, successor, Instant::now())
}
/// Register the live bounded transport-to-runner ingress for status overlays.
pub(crate) fn set_v2_network_ingress(
    height_context_id: HeightContextId,
    height: u64,
    ingress: &Arc<FairV2Ingress>,
) {
    #[cfg(test)]
    let _guard = rbc_status_test_guard();
    *SUMERAGI_V2_NETWORK_INGRESS
        .get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(V2NetworkIngressRegistration {
        owner: V2StatusOwner {
            height_context_id,
            height,
        },
        ingress: Arc::downgrade(ingress),
    });
}
/// Register the beacon horizon observed when `height` of `height_context_id` was activated.
///
/// The horizon is overlaid onto status reads for exactly that height owner, so
/// a published snapshot never carries an observation from another height.
pub(crate) fn set_v2_beacon_horizon(
    height_context_id: HeightContextId,
    height: u64,
    horizon: BeaconHorizonStatusV1,
) {
    #[cfg(test)]
    let _guard = rbc_status_test_guard();
    *SUMERAGI_V2_BEACON_HORIZON
        .get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some((
        V2StatusOwner {
            height_context_id,
            height,
        },
        horizon,
    ));
}
fn overlay_v2_beacon_horizon(status: &mut SumeragiV2Status) {
    let owner = V2StatusOwner::from_status(status);
    if let Some(horizon) = SUMERAGI_V2_BEACON_HORIZON.get().and_then(|slot| {
        slot.lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
            .filter(|(registered, _)| *registered == owner)
            .map(|(_, horizon)| *horizon)
    }) {
        status.beacon_horizon = Some(horizon);
    }
}
fn bounded_u32(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}
fn age_ms(age: Option<Duration>) -> Option<u64> {
    age.map(duration_ms)
}
fn age_queue_at_read(queue: &mut SumeragiV2QueueStatus, elapsed: Duration) {
    let captured_age_ms = queue.oldest_age_ms.unwrap_or_default();
    queue.oldest_age_ms =
        (queue.depth != 0).then(|| captured_age_ms.saturating_add(duration_ms(elapsed)));
}
fn latest_v2_effect_status() -> Option<EffectExecutorStatus> {
    SUMERAGI_V2_EFFECT_STATUS.get().and_then(|slot| {
        slot.lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    })
}
fn latest_v2_network_ingress(owner: V2StatusOwner) -> Option<Arc<FairV2Ingress>> {
    SUMERAGI_V2_NETWORK_INGRESS.get().and_then(|slot| {
        slot.lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
            .filter(|registration| registration.owner == owner)
            .and_then(|registration| registration.ingress.upgrade())
    })
}
enum V2EffectCompletionObservation {
    Unregistered,
    Live(RuntimeQueueLaneSnapshot),
    Retired,
}
fn latest_v2_effect_completion_snapshot(
    owner: V2StatusOwner,
    now: Instant,
) -> V2EffectCompletionObservation {
    let Some(slot) = SUMERAGI_V2_EFFECT_COMPLETION_OBSERVER.get() else {
        return V2EffectCompletionObservation::Unregistered;
    };
    let slot = slot
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let Some(registration) = slot.as_ref() else {
        return V2EffectCompletionObservation::Unregistered;
    };
    if registration.owner != owner {
        return V2EffectCompletionObservation::Unregistered;
    }
    let Some(observer) = registration.observer.upgrade() else {
        return V2EffectCompletionObservation::Retired;
    };
    drop(slot);
    V2EffectCompletionObservation::Live(observer.completion_queue_snapshot(now))
}
fn overlay_v2_network_ingress(status: &mut SumeragiV2Status, now: Instant) -> Option<Duration> {
    status
        .liveness
        .queues
        .retain(|queue| queue.queue != SumeragiV2QueueKind::NetworkIngress);
    let Some(ingress) = latest_v2_network_ingress(V2StatusOwner::from_status(status)) else {
        return None;
    };
    let snapshot = ingress.snapshot_at(now);
    status.liveness.queues.push(SumeragiV2QueueStatus {
        queue: SumeragiV2QueueKind::NetworkIngress,
        depth: bounded_u32(snapshot.depth),
        capacity: bounded_u32(snapshot.capacity),
        oldest_age_ms: age_ms(snapshot.oldest_age),
        // Source round-robin service does not retain eligible-skip debt. Its
        // private service-attempt clock lets the watchdog distinguish a
        // stopped runner from old ownership without changing this wire field.
        service_debt: 0,
    });
    snapshot.service_idle_age
}
fn overlay_v2_effect_status(
    status: &mut SumeragiV2Status,
    effect_status: &EffectExecutorStatus,
    now: Instant,
) {
    status.restart_required |= effect_status.fail_closed;
    let queues = effect_status.runtime_queues;
    let snapshot_age = now.saturating_duration_since(effect_status.captured_at);
    status.liveness.queues.retain(|queue| {
        !matches!(
            queue.queue,
            SumeragiV2QueueKind::RuntimeNormal
                | SumeragiV2QueueKind::RuntimeProgress
                | SumeragiV2QueueKind::RuntimeCompletion
                | SumeragiV2QueueKind::EffectCompletion
                | SumeragiV2QueueKind::EffectDispatch
        )
    });
    for (queue, snapshot) in [
        (SumeragiV2QueueKind::RuntimeNormal, queues.normal),
        (SumeragiV2QueueKind::RuntimeProgress, queues.progress),
        (SumeragiV2QueueKind::RuntimeCompletion, queues.completion),
        (
            SumeragiV2QueueKind::EffectCompletion,
            effect_status.effect_completion_queue,
        ),
        (
            SumeragiV2QueueKind::EffectDispatch,
            effect_status.effect_dispatch_queue,
        ),
    ] {
        let oldest_age = (snapshot.depth != 0).then(|| {
            snapshot
                .oldest_age
                .map(|age| saturating_duration_add(age, snapshot_age))
                .unwrap_or(snapshot_age)
        });
        status.liveness.queues.push(SumeragiV2QueueStatus {
            queue,
            depth: bounded_u32(snapshot.depth),
            capacity: bounded_u32(snapshot.capacity),
            oldest_age_ms: age_ms(oldest_age),
            service_debt: snapshot.max_service_debt,
        });
    }
    if effect_status.pending_candidate_loads != 0 {
        status.liveness.work.candidate = SumeragiV2LocalWorkStage::Running;
    }
    if effect_status.pending_fetches != 0 || effect_status.ready_bodies != 0 {
        status.liveness.work.body_recovery = SumeragiV2LocalWorkStage::Running;
    }
    if effect_status.pending_stores != 0 {
        status.liveness.work.body_store = SumeragiV2LocalWorkStage::Running;
    }
    if effect_status.pending_validations != 0 {
        status.liveness.work.validation = SumeragiV2LocalWorkStage::Running;
    }
    if effect_status.pending_applications != 0 || effect_status.deferred_application_merge_work != 0
    {
        status.liveness.work.application = SumeragiV2LocalWorkStage::Running;
    }
    if let Some(stage) = effect_status.pending_tip_recovery_stage {
        overlay_pending_tip_recovery_work(status, stage);
    }
}
/// Replace reducer-queued startup work with the exact closed-ingress recovery stage.
///
/// A durable Decision keeps the public phase/body pair at `PendingApply`.
/// These local-work fields identify whether the node is still replaying body
/// availability, storage, validation, or application without presenting a
/// decided block as an ordinary live-view reconstruction.
fn overlay_pending_tip_recovery_work(
    status: &mut SumeragiV2Status,
    stage: PendingKuraApplyRecoveryStage,
) {
    use SumeragiV2LocalWorkStage::{Complete, Idle, Queued, Running};
    let work = &mut status.liveness.work;
    match stage {
        PendingKuraApplyRecoveryStage::CertifiedFetch => {
            work.body_recovery = Queued;
            work.body_store = Idle;
            work.validation = Idle;
            work.application = Queued;
        }
        PendingKuraApplyRecoveryStage::DurableStore => {
            work.body_recovery = Complete;
            work.body_store = Queued;
            work.validation = Idle;
            work.application = Queued;
        }
        PendingKuraApplyRecoveryStage::DeterministicValidation => {
            work.body_recovery = Complete;
            work.body_store = Complete;
            work.validation = Queued;
            work.application = Queued;
        }
        PendingKuraApplyRecoveryStage::Apply => {
            work.body_recovery = Complete;
            work.body_store = Complete;
            work.validation = Complete;
            work.application = Queued;
        }
        PendingKuraApplyRecoveryStage::ApplicationDispatched => {
            work.body_recovery = Complete;
            work.body_store = Complete;
            work.validation = Complete;
            work.application = Running;
        }
        PendingKuraApplyRecoveryStage::Completed => {
            work.body_recovery = Complete;
            work.body_store = Complete;
            work.validation = Complete;
            work.application = Complete;
        }
    }
}
fn overlay_v2_effect_completion_snapshot(
    status: &mut SumeragiV2Status,
    snapshot: RuntimeQueueLaneSnapshot,
) {
    status
        .liveness
        .queues
        .retain(|queue| queue.queue != SumeragiV2QueueKind::EffectCompletion);
    status.liveness.queues.push(SumeragiV2QueueStatus {
        queue: SumeragiV2QueueKind::EffectCompletion,
        depth: bounded_u32(snapshot.depth),
        capacity: bounded_u32(snapshot.capacity),
        oldest_age_ms: age_ms(snapshot.oldest_age),
        service_debt: snapshot.max_service_debt,
    });
}
const FAIR_SERVICE_CLASS_COUNT: u64 = 3;
fn stage_is_pending(stage: SumeragiV2LocalWorkStage) -> bool {
    matches!(
        stage,
        SumeragiV2LocalWorkStage::Queued | SumeragiV2LocalWorkStage::Running
    )
}
fn quorum_is_complete(quorum: &SumeragiV2VoteQuorumStatus) -> bool {
    quorum.signer_count >= quorum.min_signers
}
fn queue_is_starved(queue: &SumeragiV2QueueStatus) -> bool {
    // `Ingress` is the retained semantic-admission/equivocation table, not a
    // serviceable queue. `EffectDispatch` is a reserved strict FIFO attempted
    // before the runtime advances; pending-work exhaustion makes its head
    // temporarily ineligible rather than scheduler-starved. Queue age remains
    // useful context, but only genuine eligible-skip debt can prove that
    // reserved work repeatedly lost dispatch.
    !matches!(
        queue.queue,
        SumeragiV2QueueKind::Ingress | SumeragiV2QueueKind::EffectDispatch
    ) && queue.depth != 0
        && queue.service_debt >= FAIR_SERVICE_CLASS_COUNT
}
fn has_outbound_intent(status: &SumeragiV2Status, kind: SumeragiV2OutboundIntentKind) -> bool {
    status
        .liveness
        .outbound_intents
        .iter()
        .any(|intent| intent.kind == kind)
}
fn has_current_view_outbound_intent(
    status: &SumeragiV2Status,
    kind: SumeragiV2OutboundIntentKind,
) -> bool {
    status
        .liveness
        .outbound_intents
        .iter()
        .any(|intent| intent.kind == kind && intent.round.view == status.view)
}
fn has_exact_locked_commit_progress(status: &SumeragiV2Status) -> bool {
    let Some(locked) = status.locked_prepare_qc.as_ref() else {
        return false;
    };
    let exact_quorum = status.liveness.commit_quorums.iter().any(|quorum| {
        quorum.round == locked.round
            && quorum.proposal_round == locked.proposal_round
            && quorum.subject == locked.subject
            && quorum.execution_commitment == locked.execution_commitment
            && quorum.signer_count > 0
            && quorum.signed_power > 0
    });
    let exact_outbound = status.liveness.outbound_intents.iter().any(|intent| {
        matches!(
            intent.kind,
            SumeragiV2OutboundIntentKind::CommitVote | SumeragiV2OutboundIntentKind::CommitQc
        ) && intent.round == locked.round
            && intent.proposal_round == Some(locked.proposal_round)
            && intent.subject == Some(locked.subject)
            && intent.execution_commitment == Some(locked.execution_commitment)
    });
    let exact_decision = status.last_committed_height == locked.proposal_round.height
        && status.last_commit_qc.as_ref().is_some_and(|certificate| {
            certificate.certificate.phase == GlobalPhase::Commit
                && certificate.certificate.round.height == locked.round.height
                && certificate.certificate.subject == locked.subject
                && certificate.certificate.execution_commitment == locked.execution_commitment
        });
    exact_quorum || exact_outbound || exact_decision
}
/// Classify one post-threshold snapshot with a stable most-specific precedence.
fn classify_v2_liveness_blocker(
    status: &SumeragiV2Status,
    network_ingress_service_stalled: bool,
) -> SumeragiV2LivenessBlocker {
    let work = status.liveness.work;
    // Body recovery, storage, and validation are prerequisites of application.
    // A durable decision may already put the reducer in `PendingApply` while
    // one of those prerequisite stages still owns progress, so classify the
    // earliest incomplete stage instead of hiding it behind the terminal
    // application phase.
    if matches!(
        status.phase,
        SumeragiV2StatusPhase::ReconstructingPayload | SumeragiV2StatusPhase::ValidatingPayload
    ) || matches!(
        status.body_state,
        SumeragiV2BodyState::Reconstructing | SumeragiV2BodyState::Stored
    ) || stage_is_pending(work.body_recovery)
        || stage_is_pending(work.body_store)
        || stage_is_pending(work.validation)
        || (status.body_state == SumeragiV2BodyState::Missing
            && (status.locked_prepare_qc.is_some() || status.highest_prepare_qc.is_some()))
    {
        return SumeragiV2LivenessBlocker::BodyUnavailable;
    }
    if status.phase == SumeragiV2StatusPhase::PendingApply
        && status.body_state == SumeragiV2BodyState::Applied
        && work.application == SumeragiV2LocalWorkStage::Complete
    {
        return SumeragiV2LivenessBlocker::SuccessorActivationPending;
    }
    if status.phase == SumeragiV2StatusPhase::PendingApply || stage_is_pending(work.application) {
        return SumeragiV2LivenessBlocker::ApplicationPending;
    }
    let local_control_pending = stage_is_pending(work.candidate)
        || status.pending_persistence_id.is_some()
        || status.liveness.outbound_intents.iter().any(|intent| {
            matches!(
                intent.stage,
                SumeragiV2OutboundIntentStage::PendingPersistence
                    | SumeragiV2OutboundIntentStage::PendingSignature
            )
        });
    if local_control_pending {
        return SumeragiV2LivenessBlocker::LocalControlPending;
    }
    let outbound_delivery_stalled = status.liveness.outbound_intents.iter().any(|intent| {
        let relevant_view = !matches!(
            intent.kind,
            SumeragiV2OutboundIntentKind::TimeoutVote
                | SumeragiV2OutboundIntentKind::TimeoutCertificate
        ) || intent.round.view == status.view;
        relevant_view && intent.stage == SumeragiV2OutboundIntentStage::Queued
    });
    if network_ingress_service_stalled
        || outbound_delivery_stalled
        || status.liveness.queues.iter().any(queue_is_starved)
    {
        return SumeragiV2LivenessBlocker::SchedulerStarvation;
    }
    // A durable current-view timeout intent retires local proposal/Prepare
    // ownership and prevents fresh lock acquisition in that view. The reducer
    // may still report its pre-timeout Prepare phase while it collects the
    // timeout quorum, so phase alone is no longer the active progress path.
    // Commit is intentionally different: an exact durable same-round Commit
    // remains retransmittable after later-view TC installation. It retains its
    // old decision path while later progress requires an unchanged reproposal.
    let timeout_pool_started = status
        .liveness
        .timeout_quorums
        .iter()
        .any(|quorum| quorum.round.view == status.view);
    let local_timeout_started =
        has_current_view_outbound_intent(status, SumeragiV2OutboundIntentKind::TimeoutVote)
            || has_current_view_outbound_intent(
                status,
                SumeragiV2OutboundIntentKind::TimeoutCertificate,
            );
    if local_timeout_started && !has_exact_locked_commit_progress(status) {
        let formed = status
            .liveness
            .timeout_quorums
            .iter()
            .any(|quorum| quorum.round.view == status.view && quorum.certificate_formed)
            || has_current_view_outbound_intent(
                status,
                SumeragiV2OutboundIntentKind::TimeoutCertificate,
            );
        return if formed {
            SumeragiV2LivenessBlocker::SchedulerStarvation
        } else {
            SumeragiV2LivenessBlocker::TimeoutCertificateMissing
        };
    }
    if status.phase == SumeragiV2StatusPhase::Commit {
        let formed = status
            .liveness
            .commit_quorums
            .iter()
            .any(quorum_is_complete)
            || has_outbound_intent(status, SumeragiV2OutboundIntentKind::CommitQc);
        return if formed {
            SumeragiV2LivenessBlocker::SchedulerStarvation
        } else {
            SumeragiV2LivenessBlocker::CommitQuorumMissing
        };
    }
    if status.phase == SumeragiV2StatusPhase::Prepare {
        let formed = status
            .liveness
            .prepare_quorums
            .iter()
            .any(quorum_is_complete)
            || has_outbound_intent(status, SumeragiV2OutboundIntentKind::PrepareQc);
        return if formed {
            SumeragiV2LivenessBlocker::SchedulerStarvation
        } else {
            SumeragiV2LivenessBlocker::PrepareQuorumMissing
        };
    }
    if timeout_pool_started {
        let formed = status
            .liveness
            .timeout_quorums
            .iter()
            .any(|quorum| quorum.round.view == status.view && quorum.certificate_formed);
        return if formed {
            SumeragiV2LivenessBlocker::SchedulerStarvation
        } else {
            SumeragiV2LivenessBlocker::TimeoutCertificateMissing
        };
    }
    SumeragiV2LivenessBlocker::MissingProposal
}
fn overlay_v2_liveness_clock(
    status: &mut SumeragiV2Status,
    now: Instant,
) -> Option<(Duration, Option<Duration>, Instant)> {
    SUMERAGI_V2_PROGRESS_CLOCK
        .get()
        .and_then(|slot| {
            slot.lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone()
        })?
        .overlay_ages(status, now)
}
struct V2StatusObservation {
    status: SumeragiV2Status,
    watchdog_threshold: Option<Duration>,
    semantic_progress_at: Option<Instant>,
}
fn v2_status_observation_at(now: Instant) -> Option<V2StatusObservation> {
    let mut status = SUMERAGI_V2_STATUS.get().and_then(|slot| {
        slot.lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    })?;
    let owner = V2StatusOwner::from_status(&status);
    let effect_status = latest_v2_effect_status().filter(|effect_status| {
        effect_status.height_context_id == owner.height_context_id
            && effect_status.height == owner.height
    });
    if let Some(effect_status) = &effect_status {
        overlay_v2_effect_status(&mut status, effect_status, now);
    }
    match latest_v2_effect_completion_snapshot(owner, now) {
        V2EffectCompletionObservation::Live(completion_snapshot) => {
            overlay_v2_effect_completion_snapshot(&mut status, completion_snapshot);
        }
        V2EffectCompletionObservation::Retired => {
            overlay_v2_effect_completion_snapshot(
                &mut status,
                RuntimeQueueLaneSnapshot {
                    depth: 0,
                    capacity: effect_status
                        .as_ref()
                        .map_or(1, |status| status.effect_completion_queue.capacity),
                    oldest_age: None,
                    max_service_debt: 0,
                },
            );
        }
        V2EffectCompletionObservation::Unregistered => {}
    }
    overlay_v2_beacon_horizon(&mut status);
    let network_ingress_service_idle_age = overlay_v2_network_ingress(&mut status, now);
    let (no_progress_age, retained_watchdog_threshold, semantic_progress_at) =
        overlay_v2_liveness_clock(&mut status, now).map_or_else(
            || {
                (
                    Duration::from_millis(status.liveness.no_progress_age_ms),
                    None,
                    None,
                )
            },
            |(no_progress_age, watchdog_threshold, height_progress_at)| {
                (
                    no_progress_age,
                    watchdog_threshold,
                    Some(height_progress_at),
                )
            },
        );
    let watchdog_threshold = effect_status
        .as_ref()
        .map(|effect_status| effect_status.watchdog_threshold)
        .or(retained_watchdog_threshold);
    status.liveness.blocker = watchdog_threshold.and_then(|watchdog_threshold| {
        let network_ingress_service_stalled =
            network_ingress_service_idle_age.is_some_and(|age| age >= watchdog_threshold);
        (no_progress_age >= watchdog_threshold)
            .then(|| classify_v2_liveness_blocker(&status, network_ingress_service_stalled))
    });
    Some(V2StatusObservation {
        status,
        watchdog_threshold,
        semantic_progress_at,
    })
}
fn v2_status_at(now: Instant) -> Option<SumeragiV2Status> {
    v2_status_observation_at(now).map(|observation| observation.status)
}
fn current_v2_status_owner() -> Option<V2StatusOwner> {
    SUMERAGI_V2_STATUS.get().and_then(|slot| {
        slot.lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
            .map(V2StatusOwner::from_status)
    })
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum V2LivenessQuorumKind {
    Prepare,
    Commit,
    Timeout,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct V2LivenessQuorumContext {
    kind: V2LivenessQuorumKind,
    pool_count: usize,
    round: ConsensusRound,
    subject: Option<BlockSubject>,
    signer_count: u32,
    min_signers: u32,
    signed_power: u64,
    total_power: u64,
    certificate_formed: Option<bool>,
}
fn relevant_vote_quorum(
    status: &SumeragiV2Status,
    pools: &[SumeragiV2VoteQuorumStatus],
) -> Option<SumeragiV2VoteQuorumStatus> {
    pools.iter().copied().max_by_key(|quorum| {
        (
            quorum.round.view == status.view,
            quorum.signer_count,
            quorum.signed_power,
            quorum.round.view,
        )
    })
}
fn relevant_timeout_quorum(status: &SumeragiV2Status) -> Option<SumeragiV2TimeoutQuorumStatus> {
    status
        .liveness
        .timeout_quorums
        .iter()
        .copied()
        .max_by_key(|quorum| {
            (
                quorum.round.view == status.view,
                quorum.certificate_formed,
                quorum.signer_count,
                quorum.signed_power,
                quorum.round.view,
            )
        })
}
fn relevant_quorum_context(
    status: &SumeragiV2Status,
    blocker: SumeragiV2LivenessBlocker,
) -> Option<V2LivenessQuorumContext> {
    match blocker {
        SumeragiV2LivenessBlocker::PrepareQuorumMissing => {
            relevant_vote_quorum(status, &status.liveness.prepare_quorums).map(|exact| {
                V2LivenessQuorumContext {
                    kind: V2LivenessQuorumKind::Prepare,
                    pool_count: status.liveness.prepare_quorums.len(),
                    round: exact.round,
                    subject: Some(exact.subject),
                    signer_count: exact.signer_count,
                    min_signers: exact.min_signers,
                    signed_power: exact.signed_power,
                    total_power: exact.total_power,
                    certificate_formed: None,
                }
            })
        }
        SumeragiV2LivenessBlocker::CommitQuorumMissing => {
            relevant_vote_quorum(status, &status.liveness.commit_quorums).map(|exact| {
                V2LivenessQuorumContext {
                    kind: V2LivenessQuorumKind::Commit,
                    pool_count: status.liveness.commit_quorums.len(),
                    round: exact.round,
                    subject: Some(exact.subject),
                    signer_count: exact.signer_count,
                    min_signers: exact.min_signers,
                    signed_power: exact.signed_power,
                    total_power: exact.total_power,
                    certificate_formed: None,
                }
            })
        }
        SumeragiV2LivenessBlocker::TimeoutCertificateMissing => relevant_timeout_quorum(status)
            .map(|exact| V2LivenessQuorumContext {
                kind: V2LivenessQuorumKind::Timeout,
                pool_count: status.liveness.timeout_quorums.len(),
                round: exact.round,
                subject: None,
                signer_count: exact.signer_count,
                min_signers: exact.min_signers,
                signed_power: exact.signed_power,
                total_power: exact.total_power,
                certificate_formed: Some(exact.certificate_formed),
            }),
        SumeragiV2LivenessBlocker::MissingProposal
        | SumeragiV2LivenessBlocker::BodyUnavailable
        | SumeragiV2LivenessBlocker::SchedulerStarvation
        | SumeragiV2LivenessBlocker::ApplicationPending
        | SumeragiV2LivenessBlocker::SuccessorActivationPending
        | SumeragiV2LivenessBlocker::LocalControlPending => None,
    }
}
fn relevant_queue_context(status: &SumeragiV2Status) -> Option<SumeragiV2QueueStatus> {
    status
        .liveness
        .queues
        .iter()
        .copied()
        .filter(|queue| queue.depth != 0)
        .max_by_key(|queue| {
            (
                queue.service_debt,
                queue.oldest_age_ms.unwrap_or_default(),
                queue.depth,
            )
        })
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum V2LivenessWatchdogTransition {
    Blocked {
        blocker: SumeragiV2LivenessBlocker,
        previous_blocker: Option<SumeragiV2LivenessBlocker>,
    },
    Recovered {
        blocker: SumeragiV2LivenessBlocker,
    },
}
struct V2LivenessWatchdogEvent {
    transition: V2LivenessWatchdogTransition,
    status: SumeragiV2Status,
}
#[derive(Clone, Copy, Debug)]
struct V2LivenessActiveAlert {
    blocker: SumeragiV2LivenessBlocker,
    semantic_progress: V2SemanticProgressWitness,
}
#[derive(Clone, Copy, Debug)]
struct V2SemanticProgressWitness {
    height_progress_at: Option<Instant>,
    fallback_rank: V2HeightProgressRank,
}
impl V2SemanticProgressWitness {
    fn strictly_advances(self, previous: Self) -> bool {
        match (self.height_progress_at, previous.height_progress_at) {
            (Some(current), Some(previous)) => current > previous,
            _ => self.fallback_rank.strictly_advances(previous.fallback_rank),
        }
    }
}
/// Edge-triggered operator watchdog for one process-local v2 status stream.
///
/// The serialized runner polls this on every turn. Full status overlays are
/// rebuilt only when their semantic deadline expires, when no deadline exists
/// yet, or when ownership changes.
#[derive(Debug)]
pub(crate) struct V2LivenessWatchdog {
    initialized: bool,
    owner: Option<V2StatusOwner>,
    active: Option<V2LivenessActiveAlert>,
    next_due_at: Option<Instant>,
    seen_revision: u64,
    #[cfg(test)]
    observation_count: u64,
}
impl Default for V2LivenessWatchdog {
    fn default() -> Self {
        Self {
            initialized: false,
            owner: None,
            active: None,
            next_due_at: None,
            seen_revision: 0,
            #[cfg(test)]
            observation_count: 0,
        }
    }
}
impl V2LivenessWatchdog {
    const MIN_POLL_INTERVAL: Duration = Duration::from_millis(10);
    fn reset_to_owner(&mut self, owner: Option<V2StatusOwner>) {
        self.initialized = true;
        self.owner = owner;
        self.active = None;
        self.next_due_at = None;
    }
    fn schedule_next(
        &mut self,
        status: &SumeragiV2Status,
        watchdog_threshold: Option<Duration>,
        now: Instant,
    ) {
        self.next_due_at = watchdog_threshold.and_then(|threshold| {
            let no_progress_age = Duration::from_millis(status.liveness.no_progress_age_ms);
            let delay = if status.liveness.blocker.is_some() {
                threshold
            } else {
                threshold.saturating_sub(no_progress_age)
            }
            .max(Self::MIN_POLL_INTERVAL);
            now.checked_add(delay)
        });
    }
    fn observe(
        &mut self,
        observation: V2StatusObservation,
        now: Instant,
    ) -> Option<V2LivenessWatchdogEvent> {
        let V2StatusObservation {
            status,
            watchdog_threshold,
            semantic_progress_at,
        } = observation;
        let owner = V2StatusOwner::from_status(&status);
        if self.owner != Some(owner) {
            self.reset_to_owner(Some(owner));
        }
        let semantic_progress = V2SemanticProgressWitness {
            height_progress_at: semantic_progress_at,
            fallback_rank: V2HeightProgressRank::from_status(&status),
        };
        let transition = match status.liveness.blocker {
            Some(blocker) => match self.active {
                Some(active)
                    if active.blocker == blocker
                        && !semantic_progress.strictly_advances(active.semantic_progress) =>
                {
                    None
                }
                previous => {
                    self.active = Some(V2LivenessActiveAlert {
                        blocker,
                        semantic_progress,
                    });
                    Some(V2LivenessWatchdogTransition::Blocked {
                        blocker,
                        previous_blocker: previous.map(|active| active.blocker),
                    })
                }
            },
            None => self.active.and_then(|active| {
                semantic_progress
                    .strictly_advances(active.semantic_progress)
                    .then(|| {
                        self.active = None;
                        V2LivenessWatchdogTransition::Recovered {
                            blocker: active.blocker,
                        }
                    })
            }),
        };
        self.schedule_next(&status, watchdog_threshold, now);
        transition.map(|transition| V2LivenessWatchdogEvent { transition, status })
    }
    fn poll_event_at(&mut self, now: Instant) -> Option<V2LivenessWatchdogEvent> {
        let revision = SUMERAGI_V2_WATCHDOG_REVISION.load(Ordering::Acquire);
        let revision_changed = !self.initialized || revision != self.seen_revision;
        let due = self.next_due_at.is_some_and(|deadline| now >= deadline);
        if self.initialized && !due {
            if !revision_changed {
                return None;
            }
            let owner = current_v2_status_owner();
            if owner != self.owner {
                self.reset_to_owner(owner);
                self.seen_revision = revision;
                if owner.is_none() {
                    return None;
                }
            } else if self.next_due_at.is_some() {
                // Publications can defer the semantic deadline or change an
                // active classification, but neither requires rebuilding the
                // overlays before the already scheduled observation boundary.
                self.seen_revision = revision;
                return None;
            }
        }
        self.initialized = true;
        self.seen_revision = revision;
        #[cfg(test)]
        {
            self.observation_count = self.observation_count.saturating_add(1);
        }
        let Some(observation) = v2_status_observation_at(now) else {
            self.reset_to_owner(None);
            self.seen_revision = revision;
            return None;
        };
        self.observe(observation, now)
    }
    /// Poll and emit only edge-triggered liveness diagnostics.
    pub(crate) fn poll(&mut self, now: Instant) {
        if let Some(event) = self.poll_event_at(now) {
            log_v2_liveness_watchdog_event(&event);
        }
    }
    #[cfg(test)]
    const fn observation_count(&self) -> u64 {
        self.observation_count
    }
}
fn log_v2_liveness_watchdog_event(event: &V2LivenessWatchdogEvent) {
    let status = &event.status;
    let queue = relevant_queue_context(status);
    match event.transition {
        V2LivenessWatchdogTransition::Blocked {
            blocker,
            previous_blocker,
        } => {
            let quorum = relevant_quorum_context(status, blocker);
            iroha_logger::warn!(
                height = status.height,
                height_context_id = ?status.height_context_id,
                view = status.view,
                generation = status.liveness.generation,
                leader = status.leader,
                blocker = ?blocker,
                previous_blocker = ?previous_blocker,
                no_progress_age_ms = status.liveness.no_progress_age_ms,
                quorum_kind = ?quorum.map(|context| context.kind),
                quorum_pool_count = ?quorum.map(|context| context.pool_count),
                quorum_round = ?quorum.map(|context| context.round),
                quorum_subject = ?quorum.and_then(|context| context.subject),
                quorum_signer_count = ?quorum.map(|context| context.signer_count),
                quorum_min_signers = ?quorum.map(|context| context.min_signers),
                quorum_signed_power = ?quorum.map(|context| context.signed_power),
                quorum_total_power = ?quorum.map(|context| context.total_power),
                timeout_certificate_formed = ?quorum.and_then(|context| context.certificate_formed),
                queue_kind = ?queue.map(|context| context.queue),
                queue_depth = ?queue.map(|context| context.depth),
                queue_capacity = ?queue.map(|context| context.capacity),
                queue_oldest_age_ms = ?queue.and_then(|context| context.oldest_age_ms),
                queue_service_debt = ?queue.map(|context| context.service_debt),
                work = ?status.liveness.work,
                "Sumeragi v2 height has no classified semantic progress"
            );
        }
        V2LivenessWatchdogTransition::Recovered { blocker } => {
            let quorum = relevant_quorum_context(status, blocker);
            iroha_logger::info!(
                height = status.height,
                height_context_id = ?status.height_context_id,
                view = status.view,
                generation = status.liveness.generation,
                leader = status.leader,
                recovered_blocker = ?blocker,
                no_progress_age_ms = status.liveness.no_progress_age_ms,
                last_progress = ?status.liveness.last_progress,
                quorum_kind = ?quorum.map(|context| context.kind),
                quorum_pool_count = ?quorum.map(|context| context.pool_count),
                quorum_round = ?quorum.map(|context| context.round),
                quorum_subject = ?quorum.and_then(|context| context.subject),
                quorum_signer_count = ?quorum.map(|context| context.signer_count),
                quorum_min_signers = ?quorum.map(|context| context.min_signers),
                quorum_signed_power = ?quorum.map(|context| context.signed_power),
                quorum_total_power = ?quorum.map(|context| context.total_power),
                timeout_certificate_formed = ?quorum.and_then(|context| context.certificate_formed),
                queue_kind = ?queue.map(|context| context.queue),
                queue_depth = ?queue.map(|context| context.depth),
                queue_capacity = ?queue.map(|context| context.capacity),
                queue_oldest_age_ms = ?queue.and_then(|context| context.oldest_age_ms),
                queue_service_debt = ?queue.map(|context| context.service_debt),
                work = ?status.liveness.work,
                "Sumeragi v2 semantic height progress cleared the liveness alert"
            );
        }
    }
}
/// Return the latest protocol-v2 reducer snapshot, if v2 has started.
#[must_use]
pub fn v2_status() -> Option<SumeragiV2Status> {
    v2_status_at(Instant::now())
}
/// Return the latest exact reducer snapshot with process-wide fail-stop state
/// overlaid at read time.
///
/// Kura or snapshot persistence can activate the shared output guard after the
/// reducer's last status publication. Applying the monotonic flag while serving
/// prevents a stale `restart_required = false` observation in that interval.
#[must_use]
pub fn v2_status_with_restart_required(restart_required: bool) -> Option<SumeragiV2Status> {
    v2_status().map(|mut status| {
        status.restart_required |= restart_required;
        status
    })
}
/// Mark a valid current reducer snapshot as restart-required.
///
/// The process-wide output guard is activated by the runner before this local
/// projection is attempted and remains authoritative for admission and public
/// status. A malformed local projection is rejected without mutating the
/// snapshot; readers still observe the already-latched process guard through
/// [`v2_status_with_restart_required`].
pub(crate) fn mark_v2_restart_required() {
    #[cfg(test)]
    let _guard = rbc_status_test_guard();
    let Some(slot) = SUMERAGI_V2_STATUS.get() else {
        return;
    };
    if let Some(status) = slot
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .as_mut()
    {
        if status.liveness.work.successor_height == SumeragiV2LocalWorkStage::Running
            && !status.restart_required
        {
            let lifecycle = ProductionSuccessorStartupLifecycleProjection {
                transition_kind: SUCCESSOR_LIFECYCLE_FAIL,
                authority_kind: SUCCESSOR_AUTHORITY_APPLIED,
                status_height: status.height,
                stage_before: successor_stage_projection(status.liveness.work.successor_height),
                stage_after: successor_stage_projection(status.liveness.work.successor_height),
                published_height_before: status.height,
                published_height_after: status.height,
                restart_required_before: status.restart_required,
                restart_required_after: true,
            };
            let Some(checked_lifecycle) =
                check_production_successor_startup_lifecycle_transition(lifecycle)
            else {
                iroha_logger::error!(
                    height = status.height,
                    "Sumeragi v2 Running successor failure projection was rejected; preserving the unchecked status"
                );
                return;
            };
            let _authorized_lifecycle = checked_lifecycle.into_projection();
        }
        status.restart_required = true;
    }
}
/// Clear protocol-v2 status during shutdown and isolated tests.
pub fn clear_v2_status() {
    #[cfg(test)]
    let _guard = rbc_status_test_guard();
    let mut status = SUMERAGI_V2_STATUS.get().map(|slot| {
        slot.lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    });
    let mut progress_clock = SUMERAGI_V2_PROGRESS_CLOCK.get().map(|slot| {
        slot.lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    });
    if let Some(status) = &mut status {
        **status = None;
    }
    if let Some(progress_clock) = &mut progress_clock {
        **progress_clock = None;
    }
    drop(progress_clock);
    drop(status);
    if let Some(slot) = SUMERAGI_V2_EFFECT_STATUS.get() {
        *slot
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
    }
    if let Some(slot) = SUMERAGI_V2_NETWORK_INGRESS.get() {
        *slot
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
    }
    if let Some(slot) = SUMERAGI_V2_BEACON_HORIZON.get() {
        *slot
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
    }
    if let Some(slot) = SUMERAGI_V2_EFFECT_COMPLETION_OBSERVER.get() {
        *slot
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
    }
    bump_v2_watchdog_revision();
}
#[cfg(test)]
mod v2_liveness_watchdog_tests {
    use super::{
        EffectExecutorStatus, PendingKuraApplyRecoveryStage, SnapshotSuccessorActivationAuthority,
        V2IoCompletionQueueObserver, V2LivenessWatchdog, V2LivenessWatchdogTransition,
        V2SuccessorActivationError, activate_snapshot_bootstrap_v2_height_at,
        activate_v2_successor_height_at, begin_v2_successor_activation,
        classify_v2_liveness_blocker, clear_v2_status, mark_v2_restart_required,
        overlay_v2_effect_status, set_v2_effect_completion_observer, set_v2_effect_status,
        set_v2_network_ingress, set_v2_status_at, update_v2_successor_work_stage_at, v2_status_at,
        v2_status_with_restart_required,
    };
    use crate::sumeragi::{
        BlockMessage, FairV2Ingress, InboundBlockMessage,
        v2_recovery::{DurableSuccessorActivationAuthority, DurableV2PredecessorIdentity},
        v2_runtime::{RuntimeQueueLaneSnapshot, RuntimeQueueSnapshot},
    };
    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
    use iroha_data_model::block::{
        BlockHeader,
        consensus_v2::{
            BlockSubject, ConsensusMessageV2, ConsensusMessageV2Payload, ConsensusMode,
            ConsensusRound, DualQuorum, ExecutionCommitment, GlobalPhase, HeightContext,
            HeightContextId, PROTOCOL_VERSION, QuorumCertificateRef, SnapshotV2BootstrapRecord,
            SumeragiV2BodyState, SumeragiV2HeightContextStatus, SumeragiV2LivenessBlocker,
            SumeragiV2LocalWorkStage, SumeragiV2OutboundIntentKind, SumeragiV2OutboundIntentStage,
            SumeragiV2OutboundIntentStatus, SumeragiV2ProgressTransition,
            SumeragiV2ProgressTransitionStatus, SumeragiV2QueueKind, SumeragiV2QueueStatus,
            SumeragiV2Status, SumeragiV2StatusPhase, SumeragiV2TimeoutQuorumStatus,
            SumeragiV2VoteQuorumStatus, Vote,
        },
    };
    use iroha_model_base::peer::PeerId;
    use std::{
        sync::{Arc, Mutex, mpsc},
        thread,
        time::{Duration, Instant},
    };
    fn test_predecessor(height: u64, label: &[u8]) -> DurableV2PredecessorIdentity {
        DurableV2PredecessorIdentity::for_test(height, label)
    }
    fn test_successor_authority(
        predecessor: DurableV2PredecessorIdentity,
        successor_context_id: HeightContextId,
    ) -> DurableSuccessorActivationAuthority {
        DurableSuccessorActivationAuthority::for_test(predecessor, successor_context_id)
    }
    fn context_id() -> HeightContextId {
        HeightContextId(HashOf::<HeightContext>::from_untyped_unchecked(Hash::new(
            b"v2-liveness-watchdog-context",
        )))
    }
    fn authenticated_ingress_prepare(
        seed: &[u8],
        context_id: HeightContextId,
        height: u64,
    ) -> (PeerId, InboundBlockMessage) {
        let transport = PeerId::new(
            KeyPair::try_from_seed(seed.to_vec(), Algorithm::Ed25519)
                .expect("derive authenticated ingress fixture")
                .public_key()
                .clone(),
        );
        let round = ConsensusRound {
            context_id,
            height,
            view: 0,
        };
        let message = BlockMessage::V2(ConsensusMessageV2::new(ConsensusMessageV2Payload::Vote(
            Vote {
                round,
                proposal_round: round,
                phase: GlobalPhase::Prepare,
                subject: BlockSubject {
                    parent_block_hash: None,
                    block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(seed)),
                    payload_hash: Hash::new(b"watchdog-network-ingress-payload"),
                },
                execution_commitment:
                    ExecutionCommitment::without_kagemusha_top_ups_or_merge_carrier(
                        Hash::new(b"watchdog-network-ingress-parent-state"),
                        Hash::new(b"watchdog-network-ingress-post-state"),
                        Hash::new(b"watchdog-network-ingress-writes"),
                        1,
                        Hash::new(b"watchdog-network-ingress-executed-wire"),
                    ),
                signer: 0,
                signature: vec![0x5A],
            },
        )));
        let inbound = InboundBlockMessage::from_authenticated_peer(message, transport.clone());
        (transport, inbound)
    }
    fn status() -> SumeragiV2Status {
        SumeragiV2Status {
            protocol_version: PROTOCOL_VERSION,
            node_fingerprint: Hash::new(b"node"),
            build_fingerprint: Hash::new(b"build"),
            config_fingerprint: Hash::new(b"config"),
            restart_required: false,
            height_context_id: context_id(),
            height: 7,
            view: 0,
            phase: SumeragiV2StatusPhase::AwaitingProposal,
            leader: 0,
            locked_prepare_qc: None,
            highest_prepare_qc: None,
            last_timeout_certificate: None,
            body_state: SumeragiV2BodyState::Missing,
            pending_persistence_id: None,
            last_committed_height: 6,
            last_committed_subject: None,
            height_context: SumeragiV2HeightContextStatus {
                epoch: 0,
                epoch_end_height: 100,
                mode: ConsensusMode::Permissioned,
                epoch_seed: [0; 32],
                validator_count: 4,
                quorum: DualQuorum {
                    min_signers: 3,
                    total_power: 4,
                },
            },
            last_commit_qc: None,
            liveness: Default::default(),
            beacon_horizon: None,
        }
    }
    fn round(status: &SumeragiV2Status, view: u64) -> ConsensusRound {
        ConsensusRound {
            context_id: status.height_context_id,
            height: status.height,
            view,
        }
    }
    fn subject(seed: u8) -> BlockSubject {
        BlockSubject {
            parent_block_hash: None,
            block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new([seed, 0])),
            payload_hash: Hash::new([seed, 1]),
        }
    }
    fn execution_commitment(seed: u8) -> ExecutionCommitment {
        ExecutionCommitment::without_kagemusha_top_ups_or_merge_carrier(
            Hash::new([seed, 2]),
            Hash::new([seed, 3]),
            Hash::new([seed, 4]),
            1,
            Hash::new([seed, 5]),
        )
    }
    fn prepare_qc(status: &SumeragiV2Status, view: u64, seed: u8) -> QuorumCertificateRef {
        let round = round(status, view);
        QuorumCertificateRef {
            round,
            proposal_round: round,
            phase: GlobalPhase::Prepare,
            subject: subject(seed),
            execution_commitment: execution_commitment(seed),
        }
    }
    fn commit_quorum(
        status: &SumeragiV2Status,
        view: u64,
        signer_count: u32,
    ) -> SumeragiV2VoteQuorumStatus {
        let round = round(status, view);
        SumeragiV2VoteQuorumStatus {
            round,
            proposal_round: round,
            subject: subject(0xA1),
            execution_commitment: execution_commitment(0xA1),
            signer_count,
            signed_power: u64::from(signer_count),
            min_signers: 3,
            total_power: 4,
        }
    }
    fn set_progress(
        status: &mut SumeragiV2Status,
        generation: u64,
        view: u64,
        transition: SumeragiV2ProgressTransition,
    ) {
        status.liveness.generation = generation;
        status.liveness.last_progress = Some(SumeragiV2ProgressTransitionStatus {
            generation,
            round: round(status, view),
            transition,
            age_ms: 0,
        });
    }
    fn lane(capacity: usize) -> RuntimeQueueLaneSnapshot {
        RuntimeQueueLaneSnapshot {
            depth: 0,
            capacity,
            oldest_age: None,
            max_service_debt: 0,
        }
    }
    struct TestCompletionObserver {
        snapshot: Mutex<RuntimeQueueLaneSnapshot>,
    }
    impl V2IoCompletionQueueObserver for TestCompletionObserver {
        fn completion_queue_snapshot(&self, _now: Instant) -> RuntimeQueueLaneSnapshot {
            *self
                .snapshot
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
        }
    }
    fn effect_status(threshold: Duration, captured_at: Instant) -> EffectExecutorStatus {
        EffectExecutorStatus {
            height_context_id: context_id(),
            height: 7,
            captured_at,
            fail_closed: false,
            fatal_reason: None,
            pending_tip_recovery_stage: None,
            pending_tip_recovery_attempts: 0,
            pending_tip_recovery_last_result: None,
            pending_signatures: 0,
            pending_candidate_loads: 0,
            pending_fetches: 0,
            pending_stores: 0,
            pending_validations: 0,
            pending_outputs: 0,
            deferred_application_merge_work: 0,
            pending_applications: 0,
            ready_bodies: 0,
            ready_body_bytes: 0,
            pending_store_bytes: 0,
            queued_runtime_completions: 0,
            effect_completion_queue: lane(112),
            effect_dispatch_queue: lane(8),
            runtime_queues: RuntimeQueueSnapshot {
                normal: lane(64),
                progress: lane(80),
                completion: lane(96),
            },
            watchdog_threshold: threshold,
        }
    }
    #[test]
    fn pending_tip_recovery_overlay_reports_the_exact_local_stage() {
        use SumeragiV2LocalWorkStage::{Complete, Idle, Queued, Running};
        let captured_at = Instant::now();
        let mut baseline = status();
        baseline.phase = SumeragiV2StatusPhase::PendingApply;
        baseline.body_state = SumeragiV2BodyState::PendingApply;
        let cases = [
            (
                PendingKuraApplyRecoveryStage::CertifiedFetch,
                (Queued, Idle, Idle, Queued),
            ),
            (
                PendingKuraApplyRecoveryStage::DurableStore,
                (Complete, Queued, Idle, Queued),
            ),
            (
                PendingKuraApplyRecoveryStage::DeterministicValidation,
                (Complete, Complete, Queued, Queued),
            ),
            (
                PendingKuraApplyRecoveryStage::Apply,
                (Complete, Complete, Complete, Queued),
            ),
            (
                PendingKuraApplyRecoveryStage::ApplicationDispatched,
                (Complete, Complete, Complete, Running),
            ),
            (
                PendingKuraApplyRecoveryStage::Completed,
                (Complete, Complete, Complete, Complete),
            ),
        ];
        for (stage, expected) in cases {
            let mut observed = baseline.clone();
            let phase_before = observed.phase;
            let body_state_before = observed.body_state;
            let mut effects = effect_status(Duration::from_secs(5), captured_at);
            effects.pending_tip_recovery_stage = Some(stage);
            overlay_v2_effect_status(&mut observed, &effects, captured_at);
            assert_eq!(
                (
                    observed.liveness.work.body_recovery,
                    observed.liveness.work.body_store,
                    observed.liveness.work.validation,
                    observed.liveness.work.application,
                ),
                expected,
                "closed-ingress recovery stage {stage:?}"
            );
            assert_eq!(observed.phase, phase_before);
            assert_eq!(observed.body_state, body_state_before);
            let expected_blocker = match stage {
                PendingKuraApplyRecoveryStage::CertifiedFetch
                | PendingKuraApplyRecoveryStage::DurableStore
                | PendingKuraApplyRecoveryStage::DeterministicValidation => {
                    SumeragiV2LivenessBlocker::BodyUnavailable
                }
                PendingKuraApplyRecoveryStage::Apply
                | PendingKuraApplyRecoveryStage::ApplicationDispatched
                | PendingKuraApplyRecoveryStage::Completed => {
                    SumeragiV2LivenessBlocker::ApplicationPending
                }
            };
            assert_eq!(
                classify_v2_liveness_blocker(&observed, false),
                expected_blocker,
                "closed-ingress recovery stage {stage:?} must expose its actual blocker"
            );
        }
    }
    #[test]
    fn beacon_horizon_overlays_only_its_exact_height_owner() {
        let _guard = super::rbc_status_test_guard();
        clear_v2_status();
        let started_at = Instant::now();
        let published = status();
        let horizon = super::BeaconHorizonStatusV1 {
            epoch_length_blocks: 0,
            next_required_pulse_height: Some(published.height + 3),
            active_session_id: Some([0x71; 32]),
            session_covers_next_pulse: true,
            local_provider_ready: true,
        };
        super::set_v2_beacon_horizon(published.height_context_id, published.height + 1, horizon);
        set_v2_status_at(published.clone(), started_at);
        assert_eq!(
            v2_status_at(started_at).expect("published").beacon_horizon,
            None,
            "a horizon for another height is never attached"
        );
        super::set_v2_beacon_horizon(published.height_context_id, published.height, horizon);
        let observed = v2_status_at(started_at).expect("published");
        assert_eq!(observed.beacon_horizon, Some(horizon));
        observed
            .validate()
            .expect("overlaid horizon keeps the status valid");
        clear_v2_status();
        set_v2_status_at(published, started_at);
        assert_eq!(
            v2_status_at(started_at)
                .expect("republished")
                .beacon_horizon,
            None,
            "clearing status retires the registered horizon"
        );
        clear_v2_status();
    }
    #[test]
    fn cross_thread_v2_publication_waits_for_status_test_lease_and_resumes_after_release() {
        let guard = super::rbc_status_test_guard();
        clear_v2_status();
        let started_at = Instant::now();
        let publication = status();
        let (attempted_tx, attempted_rx) = mpsc::sync_channel(0);
        let (completed_tx, completed_rx) = mpsc::sync_channel(0);
        let publisher = thread::spawn(move || {
            assert!(
                super::try_reentrant_test_guard(&super::RBC_STATUS_TEST_LOCK).is_none(),
                "a foreign thread must not enter the stable status window"
            );
            attempted_tx
                .send(())
                .expect("announce the blocked publication attempt");
            set_v2_status_at(publication, started_at);
            completed_tx
                .send(())
                .expect("announce publication after lease release");
        });
        attempted_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("publisher reaches the guarded mutation boundary");
        assert!(
            v2_status_at(started_at).is_none(),
            "the stable test window must exclude a foreign publisher"
        );
        drop(guard);
        completed_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("publisher resumes after the stable window closes");
        publisher.join().expect("publisher thread completes");
        let _cleanup_guard = super::rbc_status_test_guard();
        clear_v2_status();
    }
    #[test]
    fn repeated_timeout_certificates_do_not_mask_height_no_progress() {
        let _guard = super::rbc_status_test_guard();
        clear_v2_status();
        let started_at = Instant::now();
        let mut initial = status();
        set_progress(
            &mut initial,
            0,
            0,
            SumeragiV2ProgressTransition::ProposalAdmitted,
        );
        set_v2_status_at(initial.clone(), started_at);
        set_v2_effect_status(effect_status(Duration::from_secs(5), started_at));
        let mut first_timeout_vote = initial.clone();
        set_progress(
            &mut first_timeout_vote,
            0,
            0,
            SumeragiV2ProgressTransition::TimeoutVoteAdmitted,
        );
        set_v2_status_at(first_timeout_vote, started_at + Duration::from_secs(1));
        let mut first_tc = initial.clone();
        first_tc.view = 1;
        set_progress(
            &mut first_tc,
            1,
            1,
            SumeragiV2ProgressTransition::TimeoutCertificateInstalled,
        );
        set_v2_status_at(first_tc.clone(), started_at + Duration::from_secs(2));
        let mut second_timeout_vote = first_tc;
        set_progress(
            &mut second_timeout_vote,
            1,
            1,
            SumeragiV2ProgressTransition::TimeoutVoteAdmitted,
        );
        set_v2_status_at(second_timeout_vote, started_at + Duration::from_secs(3));
        let mut second_tc = initial;
        second_tc.view = 2;
        set_progress(
            &mut second_tc,
            2,
            2,
            SumeragiV2ProgressTransition::TimeoutCertificateInstalled,
        );
        let prior_view = round(&second_tc, 1);
        second_tc
            .liveness
            .outbound_intents
            .push(SumeragiV2OutboundIntentStatus {
                kind: SumeragiV2OutboundIntentKind::TimeoutCertificate,
                round: prior_view,
                proposal_round: None,
                subject: None,
                execution_commitment: None,
                stage: SumeragiV2OutboundIntentStage::Retained,
            });
        set_v2_status_at(second_tc, started_at + Duration::from_secs(4));
        let observed = v2_status_at(started_at + Duration::from_secs(6)).expect("v2 status");
        assert_eq!(observed.liveness.no_progress_age_ms, 6_000);
        assert_eq!(
            observed
                .liveness
                .last_progress
                .expect("latest TC transition")
                .age_ms,
            2_000
        );
        assert_eq!(
            observed.liveness.blocker,
            Some(SumeragiV2LivenessBlocker::MissingProposal)
        );
        let mut body_available = observed;
        set_progress(
            &mut body_available,
            2,
            2,
            SumeragiV2ProgressTransition::BodyAvailable,
        );
        set_v2_status_at(body_available, started_at + Duration::from_secs(7));
        let resumed = v2_status_at(started_at + Duration::from_secs(8)).expect("v2 status");
        assert_eq!(resumed.liveness.no_progress_age_ms, 1_000);
        assert_eq!(resumed.liveness.blocker, None);
        clear_v2_status();
    }
    #[test]
    fn repeated_tc_reconstruction_of_same_locked_commit_pool_does_not_reset_height_clock() {
        let _guard = super::rbc_status_test_guard();
        clear_v2_status();
        let started_at = Instant::now();
        let mut locked = status();
        locked.phase = SumeragiV2StatusPhase::Commit;
        locked.body_state = SumeragiV2BodyState::Validated;
        locked.liveness.commit_quorums = vec![commit_quorum(&locked, 0, 2)];
        locked
            .liveness
            .outbound_intents
            .push(SumeragiV2OutboundIntentStatus {
                kind: SumeragiV2OutboundIntentKind::CommitVote,
                round: round(&locked, 0),
                proposal_round: Some(round(&locked, 0)),
                subject: Some(subject(0xA1)),
                execution_commitment: Some(execution_commitment(0xA1)),
                stage: SumeragiV2OutboundIntentStage::Retained,
            });
        set_progress(
            &mut locked,
            0,
            0,
            SumeragiV2ProgressTransition::CommitVoteAdmitted,
        );
        set_v2_status_at(locked.clone(), started_at);
        set_v2_effect_status(effect_status(Duration::from_secs(5), started_at));
        let mut first_tc = locked.clone();
        first_tc.view = 1;
        first_tc.liveness.commit_quorums.clear();
        set_progress(
            &mut first_tc,
            1,
            1,
            SumeragiV2ProgressTransition::TimeoutCertificateInstalled,
        );
        set_v2_status_at(first_tc.clone(), started_at + Duration::from_secs(1));
        let mut first_rebuild = first_tc.clone();
        first_rebuild.liveness.commit_quorums = vec![commit_quorum(&first_rebuild, 0, 1)];
        set_progress(
            &mut first_rebuild,
            1,
            0,
            SumeragiV2ProgressTransition::CommitVoteAdmitted,
        );
        set_v2_status_at(first_rebuild.clone(), started_at + Duration::from_secs(2));
        first_rebuild.liveness.commit_quorums = vec![commit_quorum(&first_rebuild, 0, 2)];
        set_v2_status_at(first_rebuild, started_at + Duration::from_secs(3));
        let mut second_tc = first_tc;
        second_tc.view = 2;
        set_progress(
            &mut second_tc,
            2,
            2,
            SumeragiV2ProgressTransition::TimeoutCertificateInstalled,
        );
        set_v2_status_at(second_tc.clone(), started_at + Duration::from_secs(4));
        let mut second_rebuild = second_tc;
        second_rebuild.liveness.commit_quorums = vec![commit_quorum(&second_rebuild, 0, 2)];
        set_progress(
            &mut second_rebuild,
            2,
            0,
            SumeragiV2ProgressTransition::CommitVoteAdmitted,
        );
        set_v2_status_at(second_rebuild.clone(), started_at + Duration::from_secs(5));
        let stalled = v2_status_at(started_at + Duration::from_secs(6)).expect("v2 status");
        assert_eq!(stalled.liveness.no_progress_age_ms, 6_000);
        assert_eq!(
            stalled.liveness.blocker,
            Some(SumeragiV2LivenessBlocker::CommitQuorumMissing)
        );
        second_rebuild.liveness.commit_quorums = vec![commit_quorum(&second_rebuild, 0, 3)];
        set_v2_status_at(second_rebuild, started_at + Duration::from_secs(7));
        let advanced = v2_status_at(started_at + Duration::from_secs(8)).expect("v2 status");
        assert_eq!(advanced.liveness.no_progress_age_ms, 1_000);
        assert_eq!(advanced.liveness.blocker, None);
        clear_v2_status();
    }
    #[test]
    fn blocker_classifier_has_stable_specific_precedence() {
        let baseline = status();
        assert_eq!(
            classify_v2_liveness_blocker(&baseline, false),
            SumeragiV2LivenessBlocker::MissingProposal
        );
        assert_eq!(
            classify_v2_liveness_blocker(&baseline, true),
            SumeragiV2LivenessBlocker::SchedulerStarvation
        );
        let mut body = baseline.clone();
        body.phase = SumeragiV2StatusPhase::ReconstructingPayload;
        body.body_state = SumeragiV2BodyState::Reconstructing;
        assert_eq!(
            classify_v2_liveness_blocker(&body, true),
            SumeragiV2LivenessBlocker::BodyUnavailable,
            "body recovery must take precedence over a stopped ingress scheduler"
        );
        let mut decided_body_recovery = baseline.clone();
        decided_body_recovery.phase = SumeragiV2StatusPhase::PendingApply;
        decided_body_recovery.liveness.work.body_recovery = SumeragiV2LocalWorkStage::Running;
        decided_body_recovery.liveness.work.application = SumeragiV2LocalWorkStage::Queued;
        assert_eq!(
            classify_v2_liveness_blocker(&decided_body_recovery, false),
            SumeragiV2LivenessBlocker::BodyUnavailable,
            "decided-body prerequisites must remain distinguishable from application"
        );
        let mut prepare = baseline.clone();
        prepare.phase = SumeragiV2StatusPhase::Prepare;
        prepare.body_state = SumeragiV2BodyState::Validated;
        assert_eq!(
            classify_v2_liveness_blocker(&prepare, false),
            SumeragiV2LivenessBlocker::PrepareQuorumMissing
        );
        let mut commit = prepare.clone();
        commit.phase = SumeragiV2StatusPhase::Commit;
        assert_eq!(
            classify_v2_liveness_blocker(&commit, false),
            SumeragiV2LivenessBlocker::CommitQuorumMissing
        );
        let mut candidate = commit.clone();
        candidate.liveness.work.candidate = SumeragiV2LocalWorkStage::Running;
        assert_eq!(
            classify_v2_liveness_blocker(&candidate, false),
            SumeragiV2LivenessBlocker::LocalControlPending,
            "locked-candidate acquisition must precede a missing-quorum diagnosis"
        );
        let mut timeout = baseline.clone();
        let timeout_round = round(&timeout, 0);
        timeout
            .liveness
            .outbound_intents
            .push(SumeragiV2OutboundIntentStatus {
                kind: SumeragiV2OutboundIntentKind::TimeoutVote,
                round: timeout_round,
                proposal_round: None,
                subject: None,
                execution_commitment: None,
                stage: SumeragiV2OutboundIntentStage::Retained,
            });
        assert_eq!(
            classify_v2_liveness_blocker(&timeout, false),
            SumeragiV2LivenessBlocker::TimeoutCertificateMissing
        );
        let mut scheduler = baseline.clone();
        scheduler.liveness.queues.push(SumeragiV2QueueStatus {
            queue: SumeragiV2QueueKind::RuntimeProgress,
            depth: 1,
            capacity: 32,
            oldest_age_ms: Some(0),
            service_debt: 3,
        });
        assert_eq!(
            classify_v2_liveness_blocker(&scheduler, false),
            SumeragiV2LivenessBlocker::SchedulerStarvation
        );
        let mut queued = baseline.clone();
        let queued_round = round(&queued, 0);
        queued
            .liveness
            .outbound_intents
            .push(SumeragiV2OutboundIntentStatus {
                kind: SumeragiV2OutboundIntentKind::TimeoutVote,
                round: queued_round,
                proposal_round: None,
                subject: None,
                execution_commitment: None,
                stage: SumeragiV2OutboundIntentStage::Queued,
            });
        assert_eq!(
            classify_v2_liveness_blocker(&queued, false),
            SumeragiV2LivenessBlocker::SchedulerStarvation
        );
        let mut persistence = scheduler.clone();
        persistence.pending_persistence_id = Some(17);
        assert_eq!(
            classify_v2_liveness_blocker(&persistence, true),
            SumeragiV2LivenessBlocker::LocalControlPending,
            "WAL persistence must take precedence over every scheduler witness"
        );
        let mut pending_persistence = scheduler.clone();
        pending_persistence.view = 1;
        let stale_round = round(&pending_persistence, 0);
        pending_persistence
            .liveness
            .outbound_intents
            .push(SumeragiV2OutboundIntentStatus {
                kind: SumeragiV2OutboundIntentKind::TimeoutVote,
                round: stale_round,
                proposal_round: None,
                subject: None,
                execution_commitment: None,
                stage: SumeragiV2OutboundIntentStage::PendingPersistence,
            });
        assert_eq!(
            classify_v2_liveness_blocker(&pending_persistence, false),
            SumeragiV2LivenessBlocker::LocalControlPending,
            "a pending WAL intent blocks the reducer even outside the current view"
        );
        let mut pending_signature = scheduler;
        pending_signature.view = 1;
        let stale_round = round(&pending_signature, 0);
        pending_signature
            .liveness
            .outbound_intents
            .push(SumeragiV2OutboundIntentStatus {
                kind: SumeragiV2OutboundIntentKind::TimeoutVote,
                round: stale_round,
                proposal_round: None,
                subject: None,
                execution_commitment: None,
                stage: SumeragiV2OutboundIntentStage::PendingSignature,
            });
        assert_eq!(
            classify_v2_liveness_blocker(&pending_signature, false),
            SumeragiV2LivenessBlocker::LocalControlPending,
            "a pending signature blocks the reducer even outside the current view"
        );
        let mut application = pending_signature;
        application.liveness.work.application = SumeragiV2LocalWorkStage::Running;
        assert_eq!(
            classify_v2_liveness_blocker(&application, true),
            SumeragiV2LivenessBlocker::ApplicationPending,
            "application must take precedence over a stopped ingress scheduler"
        );
    }
    #[test]
    fn current_view_timeout_path_yields_only_to_an_exact_locked_commit_owner() {
        let mut prepare = status();
        prepare.phase = SumeragiV2StatusPhase::Prepare;
        prepare.body_state = SumeragiV2BodyState::Validated;
        let current_round = round(&prepare, prepare.view);
        prepare
            .liveness
            .timeout_quorums
            .push(SumeragiV2TimeoutQuorumStatus {
                round: current_round,
                signer_count: 1,
                signed_power: 1,
                min_signers: 3,
                total_power: 4,
                certificate_formed: false,
            });
        prepare
            .validate()
            .expect("remote-timeout Prepare fixture is structurally valid");
        assert_eq!(
            classify_v2_liveness_blocker(&prepare, false),
            SumeragiV2LivenessBlocker::PrepareQuorumMissing,
            "a remote partial timeout pool does not close the local Prepare path"
        );
        prepare
            .liveness
            .outbound_intents
            .push(SumeragiV2OutboundIntentStatus {
                kind: SumeragiV2OutboundIntentKind::TimeoutVote,
                round: current_round,
                proposal_round: None,
                subject: None,
                execution_commitment: None,
                stage: SumeragiV2OutboundIntentStage::Retained,
            });
        prepare
            .validate()
            .expect("local-timeout Prepare fixture is structurally valid");
        assert_eq!(
            classify_v2_liveness_blocker(&prepare, false),
            SumeragiV2LivenessBlocker::TimeoutCertificateMissing,
            "a durable timeout closes the current Prepare path"
        );
        let mut current_commit = prepare.clone();
        current_commit.phase = SumeragiV2StatusPhase::Commit;
        let current_lock = prepare_qc(&current_commit, current_commit.view, 0xA2);
        current_commit.locked_prepare_qc = Some(current_lock);
        current_commit.highest_prepare_qc = Some(current_lock);
        current_commit
            .validate()
            .expect("same-view locked Commit fixture is structurally valid");
        assert_eq!(
            classify_v2_liveness_blocker(&current_commit, false),
            SumeragiV2LivenessBlocker::TimeoutCertificateMissing,
            "a lock without Commit ownership follows its durable timeout recovery path"
        );
        current_commit
            .liveness
            .outbound_intents
            .push(SumeragiV2OutboundIntentStatus {
                kind: SumeragiV2OutboundIntentKind::CommitVote,
                round: current_round,
                proposal_round: Some(current_lock.proposal_round),
                subject: Some(current_lock.subject),
                execution_commitment: Some(current_lock.execution_commitment),
                stage: SumeragiV2OutboundIntentStage::Retained,
            });
        assert_eq!(
            classify_v2_liveness_blocker(&current_commit, false),
            SumeragiV2LivenessBlocker::CommitQuorumMissing,
            "an exact durable Commit remains active across its same-view timeout"
        );
        let mut retained_commit = current_commit;
        retained_commit.view = 1;
        let retained_lock = prepare_qc(&retained_commit, 0, 0xA2);
        retained_commit.locked_prepare_qc = Some(retained_lock);
        retained_commit.highest_prepare_qc = Some(retained_lock);
        let later_timeout_round = round(&retained_commit, 1);
        retained_commit.liveness.outbound_intents[0].round = later_timeout_round;
        retained_commit.liveness.timeout_quorums[0].round = later_timeout_round;
        retained_commit
            .validate()
            .expect("retained same-round Commit fixture is structurally valid");
        assert_eq!(
            classify_v2_liveness_blocker(&retained_commit, false),
            SumeragiV2LivenessBlocker::CommitQuorumMissing,
            "a later-view timeout must not hide the retained old-round Commit path"
        );
    }
    #[test]
    fn runtime_work_overlay_precedes_watchdog_classification() {
        let _guard = super::rbc_status_test_guard();
        clear_v2_status();
        let started_at = Instant::now();
        set_v2_status_at(status(), started_at);
        let mut effects = effect_status(Duration::from_secs(1), started_at);
        effects.pending_applications = 1;
        set_v2_effect_status(effects);
        let observed = v2_status_at(started_at + Duration::from_secs(2)).expect("v2 status");
        assert_eq!(
            observed.liveness.work.application,
            SumeragiV2LocalWorkStage::Running
        );
        assert_eq!(
            observed.liveness.blocker,
            Some(SumeragiV2LivenessBlocker::ApplicationPending)
        );
        clear_v2_status();
    }
    #[test]
    fn successor_startup_overlays_never_cross_the_height_context_boundary() {
        let _guard = super::rbc_status_test_guard();
        clear_v2_status();
        let started_at = Instant::now();
        let mut predecessor = status();
        predecessor.phase = SumeragiV2StatusPhase::PendingApply;
        predecessor.body_state = SumeragiV2BodyState::Applied;
        predecessor.liveness.work.application = SumeragiV2LocalWorkStage::Complete;
        predecessor.liveness.work.successor_height = SumeragiV2LocalWorkStage::Running;
        set_progress(
            &mut predecessor,
            3,
            0,
            SumeragiV2ProgressTransition::Applied,
        );
        set_v2_status_at(predecessor.clone(), started_at);
        set_v2_effect_status(effect_status(Duration::from_secs(1), started_at));
        let successor_context_id = HeightContextId(
            HashOf::<HeightContext>::from_untyped_unchecked(Hash::new(b"successor overlay")),
        );
        let successor_height = predecessor.height + 1;
        let mut effects = effect_status(Duration::from_secs(1), started_at);
        effects.height_context_id = successor_context_id;
        effects.height = successor_height;
        effects.pending_fetches = 1;
        set_v2_effect_status(effects);
        let observer = Arc::new(TestCompletionObserver {
            snapshot: Mutex::new(RuntimeQueueLaneSnapshot {
                depth: 1,
                capacity: 4,
                oldest_age: Some(Duration::from_secs(2)),
                max_service_debt: 3,
            }),
        });
        set_v2_effect_completion_observer(successor_context_id, successor_height, &observer);
        let (validator, inbound) = authenticated_ingress_prepare(
            b"successor-overlay-network-ingress",
            successor_context_id,
            successor_height,
        );
        let ingress = Arc::new(FairV2Ingress::new(6, 2 * 1024 * 1024, 1024 * 1024, 0, 0));
        ingress
            .configure_roster([validator])
            .expect("configure test ingress");
        ingress.open().expect("open successor ingress");
        ingress.try_push(inbound).expect("queue successor input");
        set_v2_network_ingress(successor_context_id, successor_height, &ingress);
        let during_startup =
            v2_status_at(started_at + Duration::from_secs(3)).expect("predecessor status");
        assert_eq!(during_startup.height, predecessor.height);
        assert_eq!(
            during_startup.height_context_id,
            predecessor.height_context_id
        );
        assert_eq!(
            during_startup.liveness.work.successor_height,
            SumeragiV2LocalWorkStage::Running
        );
        assert_eq!(
            during_startup.liveness.work.body_recovery, predecessor.liveness.work.body_recovery,
            "successor fetch ownership must not be attributed to the predecessor"
        );
        assert!(
            during_startup.liveness.queues.iter().all(|queue| !matches!(
                queue.queue,
                SumeragiV2QueueKind::RuntimeNormal
                    | SumeragiV2QueueKind::RuntimeProgress
                    | SumeragiV2QueueKind::RuntimeCompletion
                    | SumeragiV2QueueKind::EffectCompletion
                    | SumeragiV2QueueKind::EffectDispatch
                    | SumeragiV2QueueKind::NetworkIngress
            )),
            "no successor-owned queue may overlay the predecessor"
        );
        assert_eq!(
            during_startup.liveness.blocker,
            Some(SumeragiV2LivenessBlocker::SuccessorActivationPending),
            "durably applied predecessor status must expose successor activation, not application, as the remaining blocker"
        );
        let mut successor = status();
        successor.height_context_id = successor_context_id;
        successor.height = successor_height;
        successor.last_committed_height = predecessor.height;
        set_progress(
            &mut successor,
            successor_height,
            0,
            SumeragiV2ProgressTransition::SuccessorHeightActivated,
        );
        set_v2_status_at(successor, started_at + Duration::from_secs(4));
        let active = v2_status_at(started_at + Duration::from_secs(5)).expect("successor status");
        assert_eq!(
            active.liveness.work.body_recovery,
            SumeragiV2LocalWorkStage::Running
        );
        assert!(active.liveness.queues.iter().any(|queue| {
            queue.queue == SumeragiV2QueueKind::EffectCompletion
                && queue.depth == 1
                && queue.service_debt == 3
        }));
        assert!(active.liveness.queues.iter().any(|queue| {
            queue.queue == SumeragiV2QueueKind::NetworkIngress && queue.depth == 1
        }));
        ingress.close();
        clear_v2_status();
    }
    #[test]
    fn rejected_running_successor_failure_projection_preserves_status() {
        let _guard = super::rbc_status_test_guard();
        clear_v2_status();
        let started_at = Instant::now();
        let mut corrupted = status();
        corrupted.height = 0;
        corrupted.restart_required = false;
        corrupted.liveness.work.successor_height = SumeragiV2LocalWorkStage::Running;
        set_v2_status_at(corrupted, started_at);
        mark_v2_restart_required();
        let failed = v2_status_at(started_at).expect("failed status remains visible");
        assert!(
            !failed.restart_required,
            "a rejected lifecycle transition must not mutate the status"
        );
        assert_eq!(
            failed.liveness.work.successor_height,
            SumeragiV2LocalWorkStage::Running,
            "failure cannot fabricate successor completion"
        );
        let public = v2_status_with_restart_required(true)
            .expect("process output guard must overlay the preserved local status");
        assert!(
            public.restart_required,
            "the prelatched process output guard remains authoritative"
        );
        let local = v2_status_at(started_at).expect("local status remains visible");
        assert!(
            !local.restart_required,
            "serving the process overlay must not rewrite rejected local state"
        );
        clear_v2_status();
    }
    #[test]
    fn successor_handoff_is_visible_until_the_exact_successor_becomes_active_once() {
        let _guard = super::rbc_status_test_guard();
        clear_v2_status();
        let started_at = Instant::now();
        let mut applied = status();
        applied.phase = SumeragiV2StatusPhase::PendingApply;
        applied.body_state = SumeragiV2BodyState::Applied;
        applied.liveness.work.application = SumeragiV2LocalWorkStage::Complete;
        applied.liveness.work.successor_height = SumeragiV2LocalWorkStage::Queued;
        set_progress(&mut applied, 3, 0, SumeragiV2ProgressTransition::Applied);
        let height = applied.height;
        set_v2_status_at(applied, started_at);
        update_v2_successor_work_stage_at(
            height,
            SumeragiV2LocalWorkStage::Queued,
            SumeragiV2LocalWorkStage::Running,
            started_at + Duration::from_secs(10),
        )
        .expect("start exact successor handoff");
        let running = v2_status_at(started_at + Duration::from_secs(12)).expect("v2 status");
        assert_eq!(
            running.liveness.work.successor_height,
            SumeragiV2LocalWorkStage::Running
        );
        assert_eq!(
            running
                .liveness
                .last_progress
                .expect("application progress marker")
                .transition,
            SumeragiV2ProgressTransition::Applied
        );
        assert_eq!(
            running.liveness.no_progress_age_ms, 12_000,
            "starting successor construction is ownership, not completed height progress"
        );
        let mut successor = status();
        successor.height = height + 1;
        successor.height_context_id = HeightContextId(
            HashOf::<HeightContext>::from_untyped_unchecked(Hash::new(b"successor context")),
        );
        successor.last_committed_height = height;
        successor.view = 2;
        set_progress(
            &mut successor,
            9,
            2,
            SumeragiV2ProgressTransition::SuccessorHeightActivated,
        );
        let predecessor = test_predecessor(height, b"status exact successor");
        activate_v2_successor_height_at(
            predecessor,
            test_successor_authority(predecessor, successor.height_context_id),
            successor.clone(),
            started_at + Duration::from_secs(13),
        )
        .expect("activate exact successor");
        let active = v2_status_at(started_at + Duration::from_secs(15)).expect("v2 status");
        assert_eq!(active.height, successor.height);
        assert_eq!(
            active.liveness.work.successor_height, successor.liveness.work.successor_height,
            "the marker activates this height; it must not complete this height's own successor"
        );
        let marker = active
            .liveness
            .last_progress
            .expect("successor progress marker");
        assert_eq!(
            marker.transition,
            SumeragiV2ProgressTransition::SuccessorHeightActivated
        );
        assert_eq!(marker.generation, successor.liveness.generation);
        assert_eq!(marker.round.height, successor.height);
        assert_eq!(marker.round.context_id, successor.height_context_id);
        assert_eq!(marker.round.view, successor.view);
        assert_eq!(marker.age_ms, 2_000);
        assert_eq!(active.liveness.no_progress_age_ms, 2_000);
        assert_eq!(
            activate_v2_successor_height_at(
                predecessor,
                test_successor_authority(predecessor, successor.height_context_id),
                successor,
                started_at + Duration::from_secs(16),
            ),
            Err(V2SuccessorActivationError::FinalizedHeightMismatch {
                expected: height,
                actual: height + 1,
            }),
            "the predecessor-owned Running stage makes activation one-shot"
        );
        let not_refreshed = v2_status_at(started_at + Duration::from_secs(17)).expect("v2 status");
        assert_eq!(
            not_refreshed
                .liveness
                .last_progress
                .expect("original activation marker")
                .age_ms,
            4_000,
            "a rejected duplicate must not republish the activation marker"
        );
        assert_eq!(not_refreshed.liveness.no_progress_age_ms, 4_000);
        let mut proposed = not_refreshed;
        set_progress(
            &mut proposed,
            9,
            2,
            SumeragiV2ProgressTransition::ProposalAdmitted,
        );
        set_v2_status_at(proposed, started_at + Duration::from_secs(18));
        let advanced = v2_status_at(started_at + Duration::from_secs(19)).expect("v2 status");
        assert_eq!(
            advanced.liveness.no_progress_age_ms, 1_000,
            "activation is rank zero for the successor, so its first proposal advances progress"
        );
        clear_v2_status();
    }
    #[test]
    fn completed_predecessor_work_alone_never_claims_successor_activation() {
        let _guard = super::rbc_status_test_guard();
        clear_v2_status();
        let started_at = Instant::now();
        let mut applied = status();
        applied.phase = SumeragiV2StatusPhase::PendingApply;
        applied.body_state = SumeragiV2BodyState::Applied;
        applied.liveness.work.application = SumeragiV2LocalWorkStage::Complete;
        applied.liveness.work.successor_height = SumeragiV2LocalWorkStage::Running;
        set_progress(&mut applied, 3, 0, SumeragiV2ProgressTransition::Applied);
        let height = applied.height;
        set_v2_status_at(applied, started_at);
        update_v2_successor_work_stage_at(
            height,
            SumeragiV2LocalWorkStage::Running,
            SumeragiV2LocalWorkStage::Complete,
            started_at + Duration::from_secs(3),
        )
        .expect("complete predecessor-owned construction work");
        let completed = v2_status_at(started_at + Duration::from_secs(5)).expect("v2 status");
        assert_eq!(
            completed.liveness.work.successor_height,
            SumeragiV2LocalWorkStage::Complete
        );
        assert_eq!(
            completed
                .liveness
                .last_progress
                .expect("application marker remains authoritative")
                .transition,
            SumeragiV2ProgressTransition::Applied,
            "Complete is not activation until the successor snapshot is installed"
        );
        assert_eq!(completed.liveness.no_progress_age_ms, 5_000);
        clear_v2_status();
    }
    #[cfg(feature = "bls")]
    #[test]
    fn complete_tip_retirement_and_successor_owner_bind_are_release_bound() {
        crate::sumeragi::v2_first_release_recovery::run_complete_tip_retirement_release_regressions(
        );
    }
    #[test]
    fn snapshot_recovery_publishes_one_exact_live_successor_boundary() {
        let _guard = super::rbc_status_test_guard();
        clear_v2_status();
        let started_at = Instant::now();
        let mut successor = status();
        successor.height = 8;
        successor.last_committed_height = 7;
        successor.view = 2;
        set_progress(
            &mut successor,
            8,
            2,
            SumeragiV2ProgressTransition::SuccessorHeightActivated,
        );
        let snapshot_record_hash = HashOf::<SnapshotV2BootstrapRecord>::from_untyped_unchecked(
            Hash::new(b"status snapshot bootstrap record"),
        );
        let snapshot_block_hash = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
            b"status snapshot bootstrap block",
        ));
        activate_snapshot_bootstrap_v2_height_at(
            SnapshotSuccessorActivationAuthority::for_test(
                snapshot_record_hash,
                7,
                snapshot_block_hash,
                successor.height_context_id,
            ),
            successor.clone(),
            started_at,
        )
        .expect("publish snapshot successor only after startup");
        let active = v2_status_at(started_at + Duration::from_secs(2)).expect("active successor");
        assert_eq!(active.height, 8);
        assert_eq!(active.last_committed_height, 7);
        assert!(matches!(
            active.liveness.last_progress,
            Some(SumeragiV2ProgressTransitionStatus {
                transition: SumeragiV2ProgressTransition::SuccessorHeightActivated,
                age_ms: 2_000,
                ..
            })
        ));
        assert_eq!(
            activate_snapshot_bootstrap_v2_height_at(
                SnapshotSuccessorActivationAuthority::for_test(
                    snapshot_record_hash,
                    7,
                    snapshot_block_hash,
                    successor.height_context_id,
                ),
                successor,
                started_at + Duration::from_secs(3),
            ),
            Err(V2SuccessorActivationError::RecoveredStatusAlreadyPublished(
                8
            ))
        );
        let unchanged =
            v2_status_at(started_at + Duration::from_secs(4)).expect("original successor");
        assert_eq!(
            unchanged
                .liveness
                .last_progress
                .expect("activation marker")
                .age_ms,
            4_000,
            "a repeated recovery publication must not refresh the witness"
        );
        clear_v2_status();
    }
    #[test]
    fn rejected_successor_activation_never_mutates_or_replaces_the_predecessor() {
        let _guard = super::rbc_status_test_guard();
        let started_at = Instant::now();
        struct Case {
            name: &'static str,
            published: Option<(u64, SumeragiV2LocalWorkStage)>,
            finalized_height: u64,
            successor_height: u64,
            successor_parent: u64,
            valid_marker: bool,
            expected: V2SuccessorActivationError,
        }
        let cases = [
            Case {
                name: "missing predecessor",
                published: None,
                finalized_height: 7,
                successor_height: 8,
                successor_parent: 7,
                valid_marker: true,
                expected: V2SuccessorActivationError::MissingFinalizedStatus,
            },
            Case {
                name: "mismatched predecessor height",
                published: Some((7, SumeragiV2LocalWorkStage::Running)),
                finalized_height: 6,
                successor_height: 7,
                successor_parent: 6,
                valid_marker: true,
                expected: V2SuccessorActivationError::FinalizedHeightMismatch {
                    expected: 6,
                    actual: 7,
                },
            },
            Case {
                name: "wrong predecessor stage",
                published: Some((7, SumeragiV2LocalWorkStage::Queued)),
                finalized_height: 7,
                successor_height: 8,
                successor_parent: 7,
                valid_marker: true,
                expected: V2SuccessorActivationError::WorkStageMismatch {
                    height: 7,
                    expected: SumeragiV2LocalWorkStage::Running,
                    actual: SumeragiV2LocalWorkStage::Queued,
                },
            },
            Case {
                name: "successor height overflow",
                published: Some((u64::MAX, SumeragiV2LocalWorkStage::Running)),
                finalized_height: u64::MAX,
                successor_height: 0,
                successor_parent: u64::MAX,
                valid_marker: true,
                expected: V2SuccessorActivationError::SuccessorHeightOverflow(u64::MAX),
            },
            Case {
                name: "wrong successor height",
                published: Some((7, SumeragiV2LocalWorkStage::Running)),
                finalized_height: 7,
                successor_height: 9,
                successor_parent: 7,
                valid_marker: true,
                expected: V2SuccessorActivationError::SuccessorHeightMismatch {
                    expected: 8,
                    actual: 9,
                },
            },
            Case {
                name: "wrong successor parent",
                published: Some((7, SumeragiV2LocalWorkStage::Running)),
                finalized_height: 7,
                successor_height: 8,
                successor_parent: 6,
                valid_marker: true,
                expected: V2SuccessorActivationError::SuccessorParentMismatch {
                    expected: 7,
                    actual: 6,
                },
            },
            Case {
                name: "missing activation marker",
                published: Some((7, SumeragiV2LocalWorkStage::Running)),
                finalized_height: 7,
                successor_height: 8,
                successor_parent: 7,
                valid_marker: false,
                expected: V2SuccessorActivationError::SuccessorMarkerMismatch,
            },
        ];
        for case in cases {
            clear_v2_status();
            if let Some((height, stage)) = case.published {
                let mut predecessor = status();
                predecessor.height = height;
                predecessor.phase = SumeragiV2StatusPhase::PendingApply;
                predecessor.body_state = SumeragiV2BodyState::Applied;
                predecessor.liveness.work.application = SumeragiV2LocalWorkStage::Complete;
                predecessor.liveness.work.successor_height = stage;
                set_progress(
                    &mut predecessor,
                    3,
                    0,
                    SumeragiV2ProgressTransition::Applied,
                );
                set_v2_status_at(predecessor, started_at);
            }
            let mut successor = status();
            successor.height = case.successor_height;
            successor.last_committed_height = case.successor_parent;
            successor.liveness.last_progress = None;
            if case.valid_marker {
                let view = successor.view;
                set_progress(
                    &mut successor,
                    3,
                    view,
                    SumeragiV2ProgressTransition::SuccessorHeightActivated,
                );
            }
            let predecessor_identity =
                test_predecessor(case.finalized_height, case.name.as_bytes());
            assert_eq!(
                activate_v2_successor_height_at(
                    predecessor_identity,
                    test_successor_authority(predecessor_identity, successor.height_context_id,),
                    successor,
                    started_at + Duration::from_secs(1),
                ),
                Err(case.expected),
                "{}",
                case.name
            );
            match case.published {
                None => assert!(
                    v2_status_at(started_at + Duration::from_secs(2)).is_none(),
                    "{} must not publish a successor",
                    case.name
                ),
                Some((height, stage)) => {
                    let unchanged = v2_status_at(started_at + Duration::from_secs(2))
                        .expect("published predecessor remains visible");
                    assert_eq!(unchanged.height, height, "{}", case.name);
                    assert_eq!(
                        unchanged.body_state,
                        SumeragiV2BodyState::Applied,
                        "{}",
                        case.name
                    );
                    assert_eq!(
                        unchanged.liveness.work.successor_height, stage,
                        "{}",
                        case.name
                    );
                    assert_eq!(
                        unchanged
                            .liveness
                            .last_progress
                            .expect("application marker remains authoritative")
                            .transition,
                        SumeragiV2ProgressTransition::Applied,
                        "{} must not publish an activation marker",
                        case.name
                    );
                }
            }
        }
        clear_v2_status();
        let mut predecessor = status();
        predecessor.phase = SumeragiV2StatusPhase::PendingApply;
        predecessor.body_state = SumeragiV2BodyState::Applied;
        predecessor.liveness.work.application = SumeragiV2LocalWorkStage::Complete;
        predecessor.liveness.work.successor_height = SumeragiV2LocalWorkStage::Running;
        set_progress(
            &mut predecessor,
            3,
            0,
            SumeragiV2ProgressTransition::Applied,
        );
        let predecessor_height = predecessor.height;
        set_v2_status_at(predecessor, started_at);
        let mut successor = status();
        successor.height = predecessor_height + 1;
        successor.last_committed_height = predecessor_height;
        set_progress(
            &mut successor,
            3,
            0,
            SumeragiV2ProgressTransition::SuccessorHeightActivated,
        );
        let expected_context_id = HeightContextId(HashOf::<HeightContext>::from_untyped_unchecked(
            Hash::new(b"different authenticated successor context"),
        ));
        let actual_context_id = successor.height_context_id;
        let predecessor_identity = test_predecessor(predecessor_height, b"status context mismatch");
        assert_eq!(
            activate_v2_successor_height_at(
                predecessor_identity,
                test_successor_authority(predecessor_identity, expected_context_id),
                successor,
                started_at + Duration::from_secs(1),
            ),
            Err(V2SuccessorActivationError::SuccessorContextMismatch {
                expected: expected_context_id,
                actual: actual_context_id,
            })
        );
        let unchanged = v2_status_at(started_at + Duration::from_secs(2))
            .expect("context mismatch retains the applied predecessor");
        assert_eq!(unchanged.height, predecessor_height);
        assert_eq!(
            unchanged.liveness.work.successor_height,
            SumeragiV2LocalWorkStage::Running
        );
        assert_eq!(
            unchanged
                .liveness
                .last_progress
                .expect("application marker remains authoritative")
                .transition,
            SumeragiV2ProgressTransition::Applied
        );
        clear_v2_status();
    }
    #[test]
    fn successor_handoff_rejects_every_incomplete_predecessor_witness() {
        let _guard = super::rbc_status_test_guard();
        let started_at = Instant::now();
        for fault in ["phase", "body", "application", "marker"] {
            clear_v2_status();
            let mut predecessor = status();
            predecessor.phase = SumeragiV2StatusPhase::PendingApply;
            predecessor.body_state = SumeragiV2BodyState::Applied;
            predecessor.liveness.work.application = SumeragiV2LocalWorkStage::Complete;
            predecessor.liveness.work.successor_height = SumeragiV2LocalWorkStage::Queued;
            set_progress(
                &mut predecessor,
                3,
                0,
                SumeragiV2ProgressTransition::Applied,
            );
            match fault {
                "phase" => predecessor.phase = SumeragiV2StatusPhase::Commit,
                "body" => predecessor.body_state = SumeragiV2BodyState::PendingApply,
                "application" => {
                    predecessor.liveness.work.application = SumeragiV2LocalWorkStage::Running;
                }
                "marker" => {
                    predecessor
                        .liveness
                        .last_progress
                        .as_mut()
                        .expect("application marker")
                        .generation += 1;
                }
                _ => unreachable!(),
            }
            let original = predecessor.clone();
            let height = predecessor.height;
            set_v2_status_at(predecessor, started_at);
            assert_eq!(
                begin_v2_successor_activation(test_predecessor(height, fault.as_bytes(),)),
                Err(V2SuccessorActivationError::PredecessorNotApplied(height)),
                "{fault}"
            );
            let unchanged = v2_status_at(started_at + Duration::from_secs(1))
                .expect("predecessor remains published");
            assert_eq!(unchanged.height, original.height, "{fault}");
            assert_eq!(unchanged.phase, original.phase, "{fault}");
            assert_eq!(unchanged.body_state, original.body_state, "{fault}");
            assert_eq!(
                unchanged.liveness.work.successor_height,
                SumeragiV2LocalWorkStage::Queued,
                "{fault}"
            );
            assert_ne!(
                unchanged
                    .liveness
                    .last_progress
                    .expect("original marker")
                    .transition,
                SumeragiV2ProgressTransition::SuccessorHeightActivated,
                "{fault}"
            );
        }
        clear_v2_status();
    }
    #[test]
    fn apply_waiting_on_merge_sidecar_is_application_pending_not_body_unavailable() {
        let _guard = super::rbc_status_test_guard();
        clear_v2_status();
        let started_at = Instant::now();
        let mut reducer_status = status();
        reducer_status.phase = SumeragiV2StatusPhase::PendingApply;
        reducer_status.body_state = SumeragiV2BodyState::PendingApply;
        let validation_before_overlay = reducer_status.liveness.work.validation;
        set_v2_status_at(reducer_status, started_at);
        let mut effects = effect_status(Duration::from_secs(1), started_at);
        effects.pending_applications = 1;
        effects.deferred_application_merge_work = 1;
        set_v2_effect_status(effects);
        let observed = v2_status_at(started_at + Duration::from_secs(2)).expect("v2 status");
        assert_eq!(
            observed.liveness.work.validation, validation_before_overlay,
            "an Apply-only sidecar wait must not manufacture validation ownership"
        );
        assert_eq!(
            observed.liveness.work.application,
            SumeragiV2LocalWorkStage::Running
        );
        assert_eq!(
            observed.liveness.blocker,
            Some(SumeragiV2LivenessBlocker::ApplicationPending)
        );
        clear_v2_status();
    }
    #[test]
    fn locked_candidate_load_overlay_precedes_commit_quorum_diagnosis() {
        let _guard = super::rbc_status_test_guard();
        clear_v2_status();
        let started_at = Instant::now();
        let mut reducer_status = status();
        reducer_status.phase = SumeragiV2StatusPhase::Commit;
        reducer_status.body_state = SumeragiV2BodyState::Validated;
        set_v2_status_at(reducer_status, started_at);
        let mut effects = effect_status(Duration::from_secs(1), started_at);
        effects.pending_candidate_loads = 1;
        set_v2_effect_status(effects);
        let observed = v2_status_at(started_at + Duration::from_secs(2)).expect("v2 status");
        assert_eq!(
            observed.liveness.work.candidate,
            SumeragiV2LocalWorkStage::Running
        );
        assert_eq!(
            observed.liveness.blocker,
            Some(SumeragiV2LivenessBlocker::LocalControlPending),
            "current-view locked-body I/O is the active local progress witness"
        );
        clear_v2_status();
    }
    #[test]
    fn paused_status_reads_advance_adapter_and_runtime_queue_ages_once() {
        let _guard = super::rbc_status_test_guard();
        clear_v2_status();
        let captured_at = Instant::now();
        let mut reducer_status = status();
        reducer_status.liveness.queues.push(SumeragiV2QueueStatus {
            queue: SumeragiV2QueueKind::DeferredProgress,
            depth: 1,
            capacity: 8,
            oldest_age_ms: Some(250),
            service_debt: 1,
        });
        set_v2_status_at(reducer_status, captured_at);
        let mut effects = effect_status(Duration::from_secs(60), captured_at);
        effects.runtime_queues.progress.depth = 1;
        effects.runtime_queues.progress.oldest_age = Some(Duration::from_millis(500));
        set_v2_effect_status(effects);
        let queue_age = |status: &SumeragiV2Status, kind| {
            status
                .liveness
                .queues
                .iter()
                .find(|queue| queue.queue == kind)
                .and_then(|queue| queue.oldest_age_ms)
        };
        let first = v2_status_at(captured_at + Duration::from_secs(2)).expect("v2 status");
        assert_eq!(
            queue_age(&first, SumeragiV2QueueKind::DeferredProgress),
            Some(2_250)
        );
        assert_eq!(
            queue_age(&first, SumeragiV2QueueKind::RuntimeProgress),
            Some(2_500)
        );
        assert_eq!(queue_age(&first, SumeragiV2QueueKind::RuntimeNormal), None);
        let later = v2_status_at(captured_at + Duration::from_secs(4)).expect("v2 status");
        assert_eq!(
            queue_age(&later, SumeragiV2QueueKind::DeferredProgress),
            Some(4_250)
        );
        assert_eq!(
            queue_age(&later, SumeragiV2QueueKind::RuntimeProgress),
            Some(4_500)
        );
        assert_eq!(queue_age(&later, SumeragiV2QueueKind::RuntimeNormal), None);
        clear_v2_status();
    }
    #[test]
    fn effect_completion_overlay_preserves_capacity_age_and_service_debt() {
        let _guard = super::rbc_status_test_guard();
        clear_v2_status();
        let captured_at = Instant::now();
        set_v2_status_at(status(), captured_at);
        let mut effects = effect_status(Duration::from_secs(60), captured_at);
        effects.effect_completion_queue = RuntimeQueueLaneSnapshot {
            depth: 1,
            capacity: 4,
            oldest_age: Some(Duration::from_millis(500)),
            max_service_debt: 3,
        };
        set_v2_effect_status(effects);
        let observed = v2_status_at(captured_at + Duration::from_secs(2)).expect("v2 status");
        let completion = observed
            .liveness
            .queues
            .iter()
            .find(|queue| queue.queue == SumeragiV2QueueKind::EffectCompletion)
            .expect("effect completion queue");
        assert_eq!(completion.depth, 1);
        assert_eq!(completion.capacity, 4);
        assert_eq!(completion.oldest_age_ms, Some(2_500));
        assert_eq!(completion.service_debt, 3);
        clear_v2_status();
    }
    #[test]
    fn effect_dispatch_overlay_exposes_age_without_claiming_scheduler_starvation() {
        let _guard = super::rbc_status_test_guard();
        clear_v2_status();
        let captured_at = Instant::now();
        set_v2_status_at(status(), captured_at);
        let mut effects = effect_status(Duration::from_secs(1), captured_at);
        effects.effect_dispatch_queue = RuntimeQueueLaneSnapshot {
            depth: 2,
            capacity: 8,
            oldest_age: Some(Duration::from_millis(500)),
            max_service_debt: 2,
        };
        set_v2_effect_status(effects.clone());
        let below_threshold =
            v2_status_at(captured_at + Duration::from_secs(2)).expect("v2 status");
        let dispatch = below_threshold
            .liveness
            .queues
            .iter()
            .find(|queue| queue.queue == SumeragiV2QueueKind::EffectDispatch)
            .expect("effect dispatch queue");
        assert_eq!(dispatch.depth, 2);
        assert_eq!(dispatch.capacity, 8);
        assert_eq!(dispatch.oldest_age_ms, Some(2_500));
        assert_eq!(dispatch.service_debt, 2);
        assert_ne!(
            below_threshold.liveness.blocker,
            Some(SumeragiV2LivenessBlocker::SchedulerStarvation),
            "EffectDispatch never participates in scheduler skip rotation"
        );
        effects.effect_dispatch_queue.max_service_debt = 3;
        set_v2_effect_status(effects);
        let full_rotation = v2_status_at(captured_at + Duration::from_secs(2)).expect("v2 status");
        assert_ne!(
            full_rotation.liveness.blocker,
            Some(SumeragiV2LivenessBlocker::SchedulerStarvation),
            "EffectDispatch capacity retries cannot constitute scheduler skip debt"
        );
        clear_v2_status();
    }
    #[test]
    fn live_effect_completion_observer_survives_stopped_runner_and_clears_stale_depth() {
        let _guard = super::rbc_status_test_guard();
        clear_v2_status();
        let captured_at = Instant::now();
        set_v2_status_at(status(), captured_at);
        set_v2_effect_status(effect_status(Duration::from_secs(60), captured_at));
        let observer = Arc::new(TestCompletionObserver {
            snapshot: Mutex::new(RuntimeQueueLaneSnapshot {
                depth: 0,
                capacity: 4,
                oldest_age: None,
                max_service_debt: 0,
            }),
        });
        set_v2_effect_completion_observer(context_id(), 7, &observer);
        *observer
            .snapshot
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = RuntimeQueueLaneSnapshot {
            depth: 1,
            capacity: 4,
            oldest_age: Some(Duration::from_millis(750)),
            max_service_debt: 2,
        };
        let retained = v2_status_at(captured_at + Duration::from_secs(2))
            .expect("live completion ownership without another runner publication");
        let completion = retained
            .liveness
            .queues
            .iter()
            .find(|queue| queue.queue == SumeragiV2QueueKind::EffectCompletion)
            .expect("effect completion queue");
        assert_eq!(completion.depth, 1);
        assert_eq!(completion.capacity, 4);
        assert_eq!(completion.oldest_age_ms, Some(750));
        assert_eq!(completion.service_debt, 2);
        *observer
            .snapshot
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = RuntimeQueueLaneSnapshot {
            depth: 0,
            capacity: 4,
            oldest_age: None,
            max_service_debt: 0,
        };
        let acknowledged = v2_status_at(captured_at + Duration::from_secs(3))
            .expect("live acknowledgement clears stale published ownership");
        let completion = acknowledged
            .liveness
            .queues
            .iter()
            .find(|queue| queue.queue == SumeragiV2QueueKind::EffectCompletion)
            .expect("effect completion queue");
        assert_eq!(completion.depth, 0);
        assert_eq!(completion.oldest_age_ms, None);
        assert_eq!(completion.service_debt, 0);
        let mut stale = effect_status(Duration::from_secs(60), captured_at);
        stale.effect_completion_queue = RuntimeQueueLaneSnapshot {
            depth: 3,
            capacity: 8,
            oldest_age: Some(Duration::from_secs(1)),
            max_service_debt: 4,
        };
        set_v2_effect_status(stale);
        drop(observer);
        let retired = v2_status_at(captured_at + Duration::from_secs(4))
            .expect("retired weak observer clears stale published ownership");
        let completion = retired
            .liveness
            .queues
            .iter()
            .find(|queue| queue.queue == SumeragiV2QueueKind::EffectCompletion)
            .expect("retired effect completion queue");
        assert_eq!(completion.depth, 0);
        assert_eq!(completion.capacity, 8);
        clear_v2_status();
        set_v2_status_at(status(), captured_at + Duration::from_secs(4));
        let mut fallback = effect_status(Duration::from_secs(60), captured_at);
        fallback.effect_completion_queue = RuntimeQueueLaneSnapshot {
            depth: 3,
            capacity: 8,
            oldest_age: Some(Duration::from_secs(1)),
            max_service_debt: 4,
        };
        set_v2_effect_status(fallback);
        let after_clear = v2_status_at(captured_at + Duration::from_secs(4))
            .expect("clearing status also unregisters the live observer");
        let completion = after_clear
            .liveness
            .queues
            .iter()
            .find(|queue| queue.queue == SumeragiV2QueueKind::EffectCompletion)
            .expect("fallback effect completion queue");
        assert_eq!(completion.depth, 3);
        assert_eq!(completion.capacity, 8);
        clear_v2_status();
    }
    #[test]
    fn aged_queue_without_service_debt_does_not_claim_scheduler_starvation() {
        let _guard = super::rbc_status_test_guard();
        clear_v2_status();
        let captured_at = Instant::now();
        let mut reducer_status = status();
        reducer_status.liveness.queues.push(SumeragiV2QueueStatus {
            queue: SumeragiV2QueueKind::DeferredProgress,
            depth: 1,
            capacity: 8,
            oldest_age_ms: Some(0),
            service_debt: 0,
        });
        set_v2_status_at(reducer_status, captured_at);
        set_v2_effect_status(effect_status(Duration::from_secs(2), captured_at));
        let before_threshold =
            v2_status_at(captured_at + Duration::from_secs(1)).expect("v2 status");
        assert_eq!(before_threshold.liveness.blocker, None);
        let after_threshold =
            v2_status_at(captured_at + Duration::from_secs(3)).expect("v2 status");
        assert_eq!(
            after_threshold.liveness.blocker,
            Some(SumeragiV2LivenessBlocker::MissingProposal)
        );
        let queue = after_threshold
            .liveness
            .queues
            .iter()
            .find(|queue| queue.queue == SumeragiV2QueueKind::DeferredProgress)
            .expect("deferred progress queue");
        assert_eq!(queue.oldest_age_ms, Some(3_000));
        assert_eq!(queue.service_debt, 0);
        clear_v2_status();
    }
    #[test]
    fn network_ingress_service_clock_distinguishes_stopped_and_active_scans() {
        let _guard = super::rbc_status_test_guard();
        clear_v2_status();
        let captured_at = Instant::now();
        let (validator, inbound) = authenticated_ingress_prepare(
            b"watchdog-network-ingress-service-clock",
            context_id(),
            7,
        );
        let ingress = Arc::new(FairV2Ingress::new(6, 2 * 1024 * 1024, 1024 * 1024, 0, 0));
        ingress
            .configure_roster([validator])
            .expect("validator ingress geometry fits");
        ingress.open().expect("open test network ingress");
        ingress
            .try_push_at(inbound, captured_at - Duration::from_millis(250))
            .expect("enqueue transport message");
        set_v2_network_ingress(context_id(), 7, &ingress);
        let mut reducer_status = status();
        reducer_status.liveness.queues.push(SumeragiV2QueueStatus {
            queue: SumeragiV2QueueKind::Ingress,
            depth: 1,
            capacity: 8,
            oldest_age_ms: Some(60_000),
            service_debt: u64::MAX,
        });
        set_v2_status_at(reducer_status, captured_at);
        set_v2_effect_status(effect_status(Duration::from_secs(2), captured_at));
        let before_threshold =
            v2_status_at(captured_at + Duration::from_secs(1)).expect("v2 status");
        assert_eq!(before_threshold.liveness.blocker, None);
        let network = before_threshold
            .liveness
            .queues
            .iter()
            .find(|queue| queue.queue == SumeragiV2QueueKind::NetworkIngress)
            .expect("live network ingress queue");
        assert_eq!(network.depth, 1);
        assert_eq!(network.capacity, 6);
        assert_eq!(network.oldest_age_ms, Some(1_250));
        assert_eq!(network.service_debt, 0);
        assert!(
            before_threshold
                .liveness
                .queues
                .iter()
                .any(|queue| queue.queue == SumeragiV2QueueKind::Ingress)
        );
        let after_threshold =
            v2_status_at(captured_at + Duration::from_secs(3)).expect("v2 status");
        assert_eq!(
            after_threshold.liveness.blocker,
            Some(SumeragiV2LivenessBlocker::SchedulerStarvation),
            "a non-empty ingress with no scan for a full watchdog interval is scheduler-starved"
        );
        let network = after_threshold
            .liveness
            .queues
            .iter()
            .find(|queue| queue.queue == SumeragiV2QueueKind::NetworkIngress)
            .expect("live network ingress queue");
        assert_eq!(network.oldest_age_ms, Some(3_250));
        assert_eq!(network.service_debt, 0);
        assert!(
            ingress
                .try_recv_if_at(captured_at + Duration::from_secs(3), |_| false)
                .is_none(),
            "a live scan may retain an item blocked on downstream admission"
        );
        let actively_scanned =
            v2_status_at(captured_at + Duration::from_secs(3)).expect("v2 status");
        assert_eq!(
            actively_scanned.liveness.blocker,
            Some(SumeragiV2LivenessBlocker::MissingProposal),
            "fresh service proof clears scheduler starvation without dequeueing the old item"
        );
        let network = actively_scanned
            .liveness
            .queues
            .iter()
            .find(|queue| queue.queue == SumeragiV2QueueKind::NetworkIngress)
            .expect("live network ingress queue");
        assert_eq!(network.depth, 1);
        assert_eq!(network.oldest_age_ms, Some(3_250));
        assert_eq!(network.service_debt, 0);
        clear_v2_status();
    }
    #[test]
    fn retained_semantic_ingress_age_does_not_mask_a_missing_commit_quorum() {
        let mut commit = status();
        commit.phase = SumeragiV2StatusPhase::Commit;
        commit.body_state = SumeragiV2BodyState::Validated;
        commit.liveness.queues.push(SumeragiV2QueueStatus {
            queue: SumeragiV2QueueKind::Ingress,
            depth: 4,
            capacity: 1_028,
            oldest_age_ms: Some(60_000),
            service_debt: u64::MAX,
        });
        assert_eq!(
            classify_v2_liveness_blocker(&commit, false),
            SumeragiV2LivenessBlocker::CommitQuorumMissing
        );
    }
    #[test]
    fn active_watchdog_is_deadline_driven_edge_triggered_and_recovers_on_progress() {
        let _guard = super::rbc_status_test_guard();
        clear_v2_status();
        let started_at = Instant::now();
        set_v2_status_at(status(), started_at);
        set_v2_effect_status(effect_status(Duration::from_secs(1), started_at));
        let mut watchdog = V2LivenessWatchdog::default();
        assert!(
            watchdog
                .poll_event_at(started_at + Duration::from_millis(500))
                .is_none(),
            "the watchdog must remain silent below its retained threshold"
        );
        assert_eq!(watchdog.observation_count(), 1);
        assert!(
            watchdog
                .poll_event_at(started_at + Duration::from_millis(750))
                .is_none(),
            "the runner's fast loop must not rebuild a snapshot before it is due"
        );
        assert_eq!(watchdog.observation_count(), 1);
        let alert = watchdog
            .poll_event_at(started_at + Duration::from_secs(1))
            .expect("threshold crossing emits one alert");
        assert_eq!(
            alert.transition,
            V2LivenessWatchdogTransition::Blocked {
                blocker: SumeragiV2LivenessBlocker::MissingProposal,
                previous_blocker: None,
            }
        );
        assert!(
            watchdog
                .poll_event_at(started_at + Duration::from_millis(1_500))
                .is_none(),
            "an unchanged active blocker must not repeat before the next deadline"
        );
        assert_eq!(watchdog.observation_count(), 2);
        assert!(
            watchdog
                .poll_event_at(started_at + Duration::from_secs(2))
                .is_none(),
            "an unchanged active blocker must remain edge-triggered at reclassification"
        );
        assert_eq!(watchdog.observation_count(), 3);
        let blocker_changed_at = started_at + Duration::from_millis(2_001);
        let mut timeout = status();
        let timeout_round = round(&timeout, 0);
        timeout
            .liveness
            .timeout_quorums
            .push(SumeragiV2TimeoutQuorumStatus {
                round: timeout_round,
                signer_count: 1,
                signed_power: 1,
                min_signers: 3,
                total_power: 4,
                certificate_formed: false,
            });
        set_v2_status_at(timeout, blocker_changed_at);
        assert!(
            watchdog
                .poll_event_at(started_at + Duration::from_millis(2_500))
                .is_none(),
            "a publication does not force an expensive overlay rebuild before the due time"
        );
        assert_eq!(watchdog.observation_count(), 3);
        let changed = watchdog
            .poll_event_at(started_at + Duration::from_secs(3))
            .expect("an active blocker change emits one warning edge");
        assert_eq!(
            changed.transition,
            V2LivenessWatchdogTransition::Blocked {
                blocker: SumeragiV2LivenessBlocker::TimeoutCertificateMissing,
                previous_blocker: Some(SumeragiV2LivenessBlocker::MissingProposal),
            }
        );
        let recovered_at = started_at + Duration::from_millis(3_001);
        let mut progressed = status();
        progressed.phase = SumeragiV2StatusPhase::Prepare;
        progressed.body_state = SumeragiV2BodyState::Validated;
        set_progress(
            &mut progressed,
            0,
            0,
            SumeragiV2ProgressTransition::ProposalAdmitted,
        );
        set_v2_status_at(progressed, recovered_at);
        assert!(
            watchdog.poll_event_at(recovered_at).is_none(),
            "semantic progress is sampled at the next bounded observation deadline"
        );
        let recovered = watchdog
            .poll_event_at(started_at + Duration::from_secs(4))
            .expect("semantic height progress emits one recovery edge");
        assert_eq!(
            recovered.transition,
            V2LivenessWatchdogTransition::Recovered {
                blocker: SumeragiV2LivenessBlocker::TimeoutCertificateMissing,
            }
        );
        assert!(
            watchdog
                .poll_event_at(started_at + Duration::from_secs(4))
                .is_none()
        );
        clear_v2_status();
    }
    #[test]
    fn active_watchdog_resets_on_successor_owner_and_status_clear() {
        let _guard = super::rbc_status_test_guard();
        clear_v2_status();
        let started_at = Instant::now();
        set_v2_status_at(status(), started_at);
        set_v2_effect_status(effect_status(Duration::from_secs(1), started_at));
        let mut watchdog = V2LivenessWatchdog::default();
        let predecessor = watchdog
            .poll_event_at(started_at + Duration::from_secs(1))
            .expect("predecessor crosses its threshold");
        assert_eq!(
            predecessor.transition,
            V2LivenessWatchdogTransition::Blocked {
                blocker: SumeragiV2LivenessBlocker::MissingProposal,
                previous_blocker: None,
            }
        );
        let successor_started_at = started_at + Duration::from_millis(1_100);
        let mut successor = status();
        successor.height = 8;
        successor.last_committed_height = 7;
        set_v2_status_at(successor, successor_started_at);
        let mut successor_effects = effect_status(Duration::from_secs(1), successor_started_at);
        successor_effects.height = 8;
        set_v2_effect_status(successor_effects);
        assert!(
            watchdog.poll_event_at(successor_started_at).is_none(),
            "a successor owner resets the predecessor alert without claiming recovery"
        );
        let successor = watchdog
            .poll_event_at(successor_started_at + Duration::from_secs(1))
            .expect("the successor owns an independent alert interval");
        assert_eq!(
            successor.transition,
            V2LivenessWatchdogTransition::Blocked {
                blocker: SumeragiV2LivenessBlocker::MissingProposal,
                previous_blocker: None,
            }
        );
        clear_v2_status();
        assert!(
            watchdog
                .poll_event_at(successor_started_at + Duration::from_secs(2))
                .is_none()
        );
        assert_eq!(watchdog.owner, None);
        assert!(watchdog.active.is_none());
    }
    #[test]
    fn clear_resets_the_process_local_progress_clock() {
        let _guard = super::rbc_status_test_guard();
        clear_v2_status();
        let started_at = Instant::now();
        let mut initial = status();
        set_progress(
            &mut initial,
            0,
            0,
            SumeragiV2ProgressTransition::ProposalAdmitted,
        );
        set_v2_status_at(initial.clone(), started_at);
        let before_clear = v2_status_at(started_at + Duration::from_secs(3)).expect("v2 status");
        assert_eq!(before_clear.liveness.no_progress_age_ms, 3_000);
        assert_eq!(
            before_clear
                .liveness
                .last_progress
                .expect("proposal transition")
                .age_ms,
            3_000
        );
        clear_v2_status();
        assert_eq!(v2_status_at(started_at + Duration::from_secs(4)), None);
        set_v2_status_at(initial, started_at + Duration::from_secs(10));
        let after_clear =
            v2_status_at(started_at + Duration::from_secs(11)).expect("new v2 status");
        assert_eq!(after_clear.liveness.no_progress_age_ms, 1_000);
        assert_eq!(
            after_clear
                .liveness
                .last_progress
                .expect("new proposal transition")
                .age_ms,
            1_000
        );
        assert_eq!(after_clear.liveness.blocker, None);
        clear_v2_status();
    }
}
/// Record archival consensus-mode labels used by retained evidence validation.
///
/// The labels are process-local diagnostics only. Protocol-v2 consensus mode
/// remains owned by the immutable height context.
pub fn set_mode_tags(
    mode_tag: &str,
    staged_mode_tag: Option<&str>,
    staged_mode_activation_height: Option<u64>,
) {
    *lock_operator_status_slot(
        MODE_TAG.get_or_init(|| Mutex::new(String::new())),
        "mode tag",
    ) = mode_tag.to_owned();
    *lock_operator_status_slot(
        STAGED_MODE_TAG.get_or_init(|| Mutex::new(None)),
        "staged mode tag",
    ) = staged_mode_tag.map(ToOwned::to_owned);
    *lock_operator_status_slot(
        STAGED_MODE_ACTIVATION_HEIGHT.get_or_init(|| Mutex::new(None)),
        "staged mode activation height",
    ) = staged_mode_activation_height;
}
/// Return archival consensus-mode labels used by retained operator routes.
#[must_use]
pub fn mode_tags() -> (String, Option<String>, Option<u64>, Option<u64>) {
    let mode = MODE_TAG
        .get()
        .map(|slot| lock_operator_status_slot(slot, "mode tag").clone())
        .unwrap_or_default();
    let staged = STAGED_MODE_TAG
        .get()
        .map(|slot| lock_operator_status_slot(slot, "staged mode tag").clone())
        .unwrap_or_default();
    let activation = STAGED_MODE_ACTIVATION_HEIGHT
        .get()
        .map(|slot| *lock_operator_status_slot(slot, "staged mode activation height"))
        .unwrap_or_default();
    let lag = MODE_ACTIVATION_LAG_BLOCKS
        .get()
        .map(|slot| *lock_operator_status_slot(slot, "mode activation lag"))
        .unwrap_or_default();
    (mode, staged, activation, lag)
}
#[cfg(test)]
static MODE_TAGS_TEST_LOCK: OnceLock<TestLock> = OnceLock::new();
#[cfg(test)]
/// Serialize tests that mutate archival mode tags.
pub(crate) fn mode_tags_test_guard() -> TestLockGuard {
    reentrant_test_guard(&MODE_TAGS_TEST_LOCK)
}
