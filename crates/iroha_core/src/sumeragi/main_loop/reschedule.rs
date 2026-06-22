//! Pending-block rescheduling and quorum timeout handling.

use iroha_logger::prelude::*;

use super::*;

const RETRANSMIT_RBC_BYTES_SOFT: u64 = 128 * 1024 * 1024;
const RETRANSMIT_RBC_BYTES_HARD: u64 = 512 * 1024 * 1024;
const NEAR_QUORUM_PREEMPTIVE_RECOVERY_PER_TICK: usize = 1;
const ISOLATED_VOTE_BACKED_HANDOFF_REASON: &str = "frontier_stall_reset_fallback";

pub(super) fn quorum_retransmit_near_commit_quorum(
    min_votes_for_commit: usize,
    vote_count: usize,
) -> bool {
    min_votes_for_commit > 0
        && vote_count < min_votes_for_commit
        && vote_count.saturating_add(1) >= min_votes_for_commit
}

fn quorum_rebroadcast_observed_vote_count(
    recorded_vote_count: usize,
    local_commit_vote_emitted: bool,
    vote_count: usize,
) -> usize {
    let local_floor = usize::from(recorded_vote_count == 0 && local_commit_vote_emitted);
    recorded_vote_count.max(local_floor).max(vote_count)
}

fn quorum_rebroadcast_force_full_repair_fanout(
    widen_repair_fanout: bool,
    drop_pending: bool,
    observed_vote_backing: bool,
    vote_count: usize,
    min_votes_for_commit: usize,
) -> bool {
    widen_repair_fanout
        && !drop_pending
        && observed_vote_backing
        && vote_count < min_votes_for_commit
}

fn quorum_rebroadcast_should_request_missing_commit_qc(
    drop_pending: bool,
    target_count: usize,
    has_cached_commit_qc: bool,
    observed_vote_backing: bool,
) -> bool {
    !drop_pending && target_count > 0 && !has_cached_commit_qc && observed_vote_backing
}

fn quorum_rebroadcast_should_broadcast_block_created(
    drop_pending: bool,
    target_count: usize,
    observed_vote_backing: bool,
) -> bool {
    !drop_pending && target_count > 0 && observed_vote_backing
}

#[allow(clippy::too_many_arguments)]
fn quorum_rebroadcast_should_broadcast_vote_backed_block_sync(
    drop_pending: bool,
    target_count: usize,
    non_local_target_count: usize,
    observed_vote_backing: bool,
    contiguous_frontier: bool,
    min_votes_for_commit: usize,
    vote_count: usize,
    block_sync_update_available: bool,
    block_sync_update_fits_frame: bool,
) -> bool {
    !drop_pending
        && target_count > 0
        && non_local_target_count > 0
        && observed_vote_backing
        && contiguous_frontier
        && quorum_retransmit_near_commit_quorum(min_votes_for_commit, vote_count)
        && block_sync_update_available
        && block_sync_update_fits_frame
}

fn quorum_rebroadcast_should_mark_precommit(
    local_vote: bool,
    votes: usize,
    block_sync: bool,
    block: bool,
    missing_block_fetch: bool,
) -> bool {
    local_vote || votes > 0 || block_sync || block || missing_block_fetch
}

fn isolated_vote_backed_handoff_admission(
    resilience_enabled: bool,
    vote_count: usize,
    min_votes_for_commit: usize,
    height: u64,
    committed_height: u64,
    cached_commit_qc: bool,
) -> bool {
    resilience_enabled
        && vote_count == 1
        && vote_count < min_votes_for_commit
        && height == committed_height.saturating_add(1)
        && !cached_commit_qc
}

fn isolated_vote_backed_handoff_slot_valid(
    slot_present: bool,
    height_matches: bool,
    view_matches: bool,
    block_hash_matches: bool,
    body_present: bool,
    commit_qc_observed: bool,
    vote_backed_owner_state: bool,
) -> bool {
    slot_present
        && height_matches
        && view_matches
        && block_hash_matches
        && body_present
        && !commit_qc_observed
        && vote_backed_owner_state
}

fn isolated_vote_backed_handoff_requests_anchor(admission: bool, slot_valid: bool) -> bool {
    admission && slot_valid
}

fn isolated_vote_backed_handoff_reason_ok(reason: &str) -> bool {
    reason == ISOLATED_VOTE_BACKED_HANDOFF_REASON
}

fn isolated_vote_backed_handoff_action(
    requests_anchor: bool,
    range_pull_succeeds: bool,
    reason_ok: bool,
) -> bool {
    requests_anchor && range_pull_succeeds && reason_ok
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PreemptiveVoteBackedRetransmitTargetSource {
    NoSource,
    VoteRoster,
    CommitTopology,
}

#[allow(clippy::too_many_arguments)]
fn preemptive_vote_backed_retransmit_candidate(
    resend_window_available: bool,
    has_votes: bool,
    has_qc: bool,
    validation_inflight: bool,
    missing_local_data: bool,
    allowed_under_recovery: bool,
    progress_stall_age: Duration,
    resend_window: Duration,
    quorum_timeout: Duration,
    due: bool,
) -> bool {
    resend_window_available
        && has_votes
        && !has_qc
        && !validation_inflight
        && !missing_local_data
        && allowed_under_recovery
        && progress_stall_age >= resend_window
        && progress_stall_age < quorum_timeout
        && due
}

fn preemptive_vote_backed_retransmit_target_source(
    candidate: bool,
    pending_present: bool,
    vote_roster_targets_available: bool,
    commit_topology_targets_available: bool,
) -> PreemptiveVoteBackedRetransmitTargetSource {
    if !candidate || !pending_present {
        return PreemptiveVoteBackedRetransmitTargetSource::NoSource;
    }
    if vote_roster_targets_available {
        return PreemptiveVoteBackedRetransmitTargetSource::VoteRoster;
    }
    if commit_topology_targets_available {
        return PreemptiveVoteBackedRetransmitTargetSource::CommitTopology;
    }
    PreemptiveVoteBackedRetransmitTargetSource::NoSource
}

fn preemptive_vote_backed_retransmit_widen_fanout(
    vote_count: usize,
    min_votes_for_commit: usize,
) -> bool {
    vote_count < min_votes_for_commit
}

fn preemptive_vote_backed_retransmit_action(
    rebroadcasted_votes: usize,
    rebroadcasted_block_sync: bool,
    rebroadcasted_block: bool,
) -> bool {
    rebroadcasted_votes > 0 || rebroadcasted_block_sync || rebroadcasted_block
}

fn near_quorum_fresh_missing_block_request_suppresses(
    height_matches: bool,
    view_matches: bool,
    actionable: bool,
    request_age: Duration,
    retry_window: Duration,
    fetch_freshness_cap: Duration,
) -> bool {
    height_matches
        && view_matches
        && actionable
        && request_age
            < retry_window
                .max(Duration::from_millis(1))
                .min(fetch_freshness_cap)
}

pub(super) fn near_quorum_inflight_recovery_suppresses(
    hash_matches: bool,
    view_matches: bool,
    range_pull_inflight: bool,
    inflight_age: Duration,
    ttl: Duration,
) -> bool {
    hash_matches
        && view_matches
        && range_pull_inflight
        && inflight_age < ttl.max(Duration::from_millis(1))
}

pub(super) fn quorum_retransmit_targets_from_observed_peers(
    topology_peers: &[PeerId],
    local_peer_id: &PeerId,
    observed_signer_peers: Option<&std::collections::BTreeSet<PeerId>>,
    min_votes_for_commit: usize,
    vote_count: usize,
) -> Vec<PeerId> {
    if topology_peers.is_empty() {
        return Vec::new();
    }

    let all_non_local_targets: Vec<_> = topology_peers
        .iter()
        .filter(|peer| *peer != local_peer_id)
        .cloned()
        .collect();
    if observed_signer_peers.is_none() {
        return all_non_local_targets;
    }
    if quorum_retransmit_near_commit_quorum(min_votes_for_commit, vote_count)
        && !all_non_local_targets.is_empty()
    {
        return all_non_local_targets;
    }

    let Some(observed_signer_peers) = observed_signer_peers else {
        return all_non_local_targets;
    };
    topology_peers
        .iter()
        .filter(|peer| *peer != local_peer_id && !observed_signer_peers.contains(*peer))
        .cloned()
        .collect()
}

fn adaptive_quorum_reschedule_backoff(
    base_backoff: Duration,
    quorum_stall_age: Duration,
    quorum_timeout: Duration,
    vote_count: usize,
    min_votes_for_commit: usize,
) -> (Duration, bool) {
    if base_backoff == Duration::ZERO {
        return (Duration::ZERO, false);
    }

    let vote_deficit = min_votes_for_commit.saturating_sub(vote_count);
    let mut multiplier = if vote_deficit >= min_votes_for_commit.saturating_sub(1) {
        3
    } else if vote_deficit > 0 {
        2
    } else {
        1
    };
    let mut escalated = false;
    if quorum_timeout != Duration::ZERO {
        let severe_stall = super::saturating_mul_duration(quorum_timeout, 4);
        let moderate_stall = super::saturating_mul_duration(quorum_timeout, 2);
        if quorum_stall_age >= severe_stall {
            multiplier = multiplier.max(5);
            escalated = true;
        } else if quorum_stall_age >= moderate_stall {
            multiplier = multiplier.max(4);
            escalated = true;
        }
    }

    (
        super::saturating_mul_duration(base_backoff, multiplier),
        escalated,
    )
}

fn retransmit_pressure_score(
    tx_depth: u64,
    tx_capacity: u64,
    tx_saturated: bool,
    rbc_bytes: u64,
    rbc_pressure_level: u8,
) -> u8 {
    let tx_utilization_pct = if tx_capacity == 0 {
        0
    } else {
        tx_depth.saturating_mul(100).saturating_div(tx_capacity)
    };
    let mut score = 0u8;
    if tx_saturated || tx_utilization_pct >= 95 {
        score = score.saturating_add(3);
    } else if tx_utilization_pct >= 80 {
        score = score.saturating_add(2);
    } else if tx_utilization_pct >= 60 {
        score = score.saturating_add(1);
    }

    if rbc_pressure_level >= 2 {
        score = score.saturating_add(3);
    } else if rbc_pressure_level == 1 {
        score = score.saturating_add(2);
    }
    if rbc_bytes >= RETRANSMIT_RBC_BYTES_HARD {
        score = score.saturating_add(2);
    } else if rbc_bytes >= RETRANSMIT_RBC_BYTES_SOFT {
        score = score.saturating_add(1);
    }
    score
}

fn retransmit_target_limit(target_count: usize, pressure_score: u8) -> usize {
    if target_count == 0 {
        return 0;
    }
    if pressure_score >= 6 {
        // Keep a deterministic liveness floor under heavy pressure: never fully disable
        // retransmit fanout when there are known missing targets.
        return 1;
    }
    if pressure_score >= 4 {
        return target_count.div_ceil(4).max(1);
    }
    if pressure_score >= 2 {
        return target_count.div_ceil(2).max(1);
    }
    target_count
}

fn retransmit_cooldown_multiplier(pressure_score: u8) -> u32 {
    if pressure_score >= 6 {
        4
    } else if pressure_score >= 4 {
        3
    } else if pressure_score >= 2 {
        2
    } else {
        1
    }
}

fn consensus_ingress_reschedule_backoff(
    base_backoff: Duration,
    consensus_queue_backlog: bool,
    near_quorum_queue_backlog: bool,
) -> Duration {
    if base_backoff == Duration::ZERO {
        return Duration::ZERO;
    }
    let multiplier = if near_quorum_queue_backlog {
        8
    } else if consensus_queue_backlog {
        4
    } else {
        1
    };
    super::saturating_mul_duration(base_backoff, multiplier)
}

pub(super) fn near_quorum_payload_timeout(rebroadcast_cooldown: Duration) -> Duration {
    super::saturating_mul_duration(rebroadcast_cooldown, 2)
        .clamp(Duration::from_millis(200), Duration::from_millis(2_000))
}

pub(super) fn paced_retransmit_targets(
    mut targets: Vec<PeerId>,
    height: u64,
    view: u64,
    limit: usize,
) -> Vec<PeerId> {
    if limit == 0 || targets.is_empty() {
        return Vec::new();
    }
    if targets.len() <= limit {
        return targets;
    }
    targets.sort();
    targets.dedup();
    if targets.len() <= limit {
        return targets;
    }
    let Some(offset) = paced_retransmit_rotation_offset(targets.len(), height, view) else {
        targets.truncate(limit);
        return targets;
    };
    targets.rotate_left(offset);
    targets.truncate(limit);
    targets
}

fn paced_retransmit_rotation_offset(target_count: usize, height: u64, view: u64) -> Option<usize> {
    let target_count = u64::try_from(target_count).ok()?;
    paced_retransmit_rotation_offset_with_limit(target_count, height, view, usize::MAX as u64)
}

fn paced_retransmit_rotation_offset_with_limit(
    target_count: u64,
    height: u64,
    view: u64,
    max_offset: u64,
) -> Option<usize> {
    if target_count == 0 {
        return None;
    }
    let offset_seed = height.rotate_left(17) ^ view.rotate_left(5);
    let offset = offset_seed % target_count;
    if offset > max_offset {
        return None;
    }
    usize::try_from(offset).ok()
}

pub(super) fn contiguous_frontier_vote_backed_resend_window(
    rebroadcast_cooldown: Duration,
    vote_count: usize,
    min_votes_for_commit: usize,
) -> Duration {
    let _ = (vote_count, min_votes_for_commit);
    rebroadcast_cooldown.max(Duration::from_millis(1))
}

pub(super) fn contiguous_frontier_vote_backed_fast_resend_window(
    rebroadcast_cooldown: Duration,
    contiguous_frontier: bool,
    vote_count: usize,
    min_votes_for_commit: usize,
    relay_backpressure: bool,
    vote_queue_backlog: bool,
    rbc_availability_unresolved: bool,
) -> Option<Duration> {
    if !contiguous_frontier
        || vote_count == 0
        || vote_count >= min_votes_for_commit
        || relay_backpressure
        || vote_queue_backlog
        || rbc_availability_unresolved
    {
        return None;
    }

    Some(contiguous_frontier_vote_backed_resend_window(
        rebroadcast_cooldown,
        vote_count,
        min_votes_for_commit,
    ))
}

#[derive(Clone, Copy, Debug)]
struct RbcAvailabilityRescheduleSession {
    invalid: bool,
    malformed_chunk_shape: bool,
    delivered: bool,
    complete_delivery: bool,
    total_chunks: u32,
    received_chunks: u32,
    ready_signatures: usize,
    required_ready: usize,
}

fn rbc_availability_unresolved_for_reschedule_decision(
    da_enabled: bool,
    stall_age: Duration,
    availability_timeout: Duration,
    local_payload_available: bool,
    pending_entry: bool,
    session: Option<RbcAvailabilityRescheduleSession>,
) -> bool {
    if !da_enabled {
        return false;
    }
    // After the availability timeout, allow reschedules even if RBC is still incomplete.
    if availability_timeout != Duration::ZERO && stall_age >= availability_timeout {
        return false;
    }
    if local_payload_available {
        return false;
    }
    if pending_entry {
        return true;
    }
    let Some(session) = session else {
        return false;
    };
    if session.invalid {
        return false;
    }
    let ready_quorum = session.ready_signatures >= session.required_ready;
    if session.malformed_chunk_shape {
        return true;
    }
    if session.complete_delivery {
        return !ready_quorum;
    }
    let missing_chunks =
        session.total_chunks != 0 && session.received_chunks < session.total_chunks;
    let unverified_complete_chunks = session.delivered
        && session.total_chunks != 0
        && session.received_chunks == session.total_chunks;
    missing_chunks || !ready_quorum || unverified_complete_chunks
}

fn vote_backed_frontier_reassembly_hard_cap_from_windows(
    frontier_recovery_window: Duration,
    quorum_timeout: Duration,
    rebroadcast_cooldown: Duration,
    vote_count: usize,
    min_votes_for_commit: usize,
) -> Duration {
    let resend_window = contiguous_frontier_vote_backed_resend_window(
        rebroadcast_cooldown,
        vote_count,
        min_votes_for_commit,
    );
    super::saturating_mul_duration(
        frontier_recovery_window
            .max(quorum_timeout)
            .max(resend_window)
            .max(Duration::from_millis(1)),
        2,
    )
}

#[derive(Clone, Copy, Debug)]
struct VoteBackedReassemblySlotOwnerState {
    height: u64,
    view: u64,
    active_mode: bool,
    last_reason_quorum_timeout: bool,
    lag_started_at: Instant,
    last_progress_at: Instant,
    last_fetch_at: Option<Instant>,
    last_view_advance_at: Option<Instant>,
    deep_catchup_entered_at: Option<Instant>,
    last_vote_at: Option<Instant>,
    last_commit_qc_at: Option<Instant>,
}

#[derive(Clone, Copy, Debug)]
struct VoteBackedReassemblyRecoveryOwnerState {
    frontier_height: u64,
    last_view: u64,
    last_cause_quorum_timeout: bool,
    entered_at: Instant,
    last_progress_at: Instant,
    last_dependency_progress_at: Option<Instant>,
    last_action_at: Option<Instant>,
}

fn vote_backed_frontier_reassembly_owner_stall_age_from_sources(
    frontier_height: u64,
    frontier_view: u64,
    now: Instant,
    slot_exact_height: bool,
    slot: Option<VoteBackedReassemblySlotOwnerState>,
    recovery: Option<VoteBackedReassemblyRecoveryOwnerState>,
) -> Option<Duration> {
    if slot_exact_height
        && let Some(slot) = slot
        && slot.height == frontier_height
        && slot.view == frontier_view
        && slot.active_mode
        && slot.last_reason_quorum_timeout
    {
        let last_owner_progress_at = [
            Some(slot.lag_started_at),
            Some(slot.last_progress_at),
            slot.last_fetch_at,
            slot.last_view_advance_at,
            slot.deep_catchup_entered_at,
            slot.last_vote_at,
            slot.last_commit_qc_at,
        ]
        .into_iter()
        .flatten()
        .max()
        .unwrap_or(slot.lag_started_at);
        return Some(now.saturating_duration_since(last_owner_progress_at));
    }

    recovery
        .filter(|state| {
            state.frontier_height == frontier_height
                && state.last_view == frontier_view
                && state.last_cause_quorum_timeout
        })
        .map(|state| {
            let last_owner_progress_at = [
                Some(state.entered_at),
                Some(state.last_progress_at),
                state.last_dependency_progress_at,
                state.last_action_at,
            ]
            .into_iter()
            .flatten()
            .max()
            .unwrap_or(state.entered_at);
            now.saturating_duration_since(last_owner_progress_at)
        })
}

fn vote_backed_frontier_reassembly_stall_expiry(
    owner_stall_age: Option<Duration>,
    quorum_stall_age: Duration,
    hard_cap: Duration,
) -> Option<(Duration, Duration)> {
    let owner_stall_age = owner_stall_age?;
    (owner_stall_age >= hard_cap && quorum_stall_age >= hard_cap)
        .then_some((owner_stall_age, hard_cap))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CompletedQuorumViewAdvanceRoute {
    ExactSlot,
    ExactFallback,
    Generic,
}

fn completed_quorum_view_advance_route(
    height: u64,
    committed_height: u64,
    slot_height: Option<u64>,
) -> CompletedQuorumViewAdvanceRoute {
    let frontier_height = committed_height.saturating_add(1);
    if height != frontier_height {
        return CompletedQuorumViewAdvanceRoute::Generic;
    }
    if slot_height == Some(frontier_height) {
        CompletedQuorumViewAdvanceRoute::ExactSlot
    } else {
        CompletedQuorumViewAdvanceRoute::ExactFallback
    }
}

impl Actor {
    fn vote_backed_frontier_reassembly_hard_cap(
        &self,
        quorum_timeout: Duration,
        vote_count: usize,
        min_votes_for_commit: usize,
    ) -> Duration {
        vote_backed_frontier_reassembly_hard_cap_from_windows(
            self.frontier_recovery_window(),
            quorum_timeout,
            self.rebroadcast_cooldown(),
            vote_count,
            min_votes_for_commit,
        )
    }

    fn vote_backed_frontier_reassembly_owner_stall_age(
        &self,
        frontier_height: u64,
        frontier_view: u64,
        now: Instant,
    ) -> Option<Duration> {
        let slot = self
            .frontier_slot
            .as_ref()
            .map(|slot| VoteBackedReassemblySlotOwnerState {
                height: slot.height,
                view: slot.view,
                active_mode: !matches!(
                    slot.mode,
                    FrontierSlotMode::Finalized | FrontierSlotMode::PassiveCatchup
                ),
                last_reason_quorum_timeout: slot.repair_state.last_reason == Some("quorum_timeout"),
                lag_started_at: slot.lag_started_at(),
                last_progress_at: slot.timers.last_progress_at,
                last_fetch_at: slot.timers.last_fetch_at,
                last_view_advance_at: slot.timers.last_view_advance_at,
                deep_catchup_entered_at: slot.timers.deep_catchup_entered_at,
                last_vote_at: slot.quorum_progress.last_vote_at,
                last_commit_qc_at: slot.quorum_progress.last_commit_qc_at,
            });
        let recovery = self
            .frontier_recovery
            .map(|state| VoteBackedReassemblyRecoveryOwnerState {
                frontier_height: state.frontier_height,
                last_view: state.last_view,
                last_cause_quorum_timeout: state.last_cause == "quorum_timeout",
                entered_at: state.entered_at,
                last_progress_at: state.last_progress_at,
                last_dependency_progress_at: state.last_dependency_progress_at,
                last_action_at: state.last_action_at,
            });
        vote_backed_frontier_reassembly_owner_stall_age_from_sources(
            frontier_height,
            frontier_view,
            now,
            self.frontier_slot_is_exact_height(frontier_height),
            slot,
            recovery,
        )
    }

    fn vote_backed_frontier_reassembly_stall_expired(
        &self,
        frontier_height: u64,
        frontier_view: u64,
        quorum_stall_age: Duration,
        quorum_timeout: Duration,
        vote_count: usize,
        min_votes_for_commit: usize,
        now: Instant,
    ) -> Option<(Duration, Duration)> {
        let hard_cap = self.vote_backed_frontier_reassembly_hard_cap(
            quorum_timeout,
            vote_count,
            min_votes_for_commit,
        );
        let owner_stall_age = self.vote_backed_frontier_reassembly_owner_stall_age(
            frontier_height,
            frontier_view,
            now,
        );
        vote_backed_frontier_reassembly_stall_expiry(owner_stall_age, quorum_stall_age, hard_cap)
    }

    fn advance_view_after_completed_quorum_reschedule(
        &mut self,
        height: u64,
        view: u64,
        cause: ViewChangeCause,
        now: Instant,
    ) {
        match completed_quorum_view_advance_route(
            height,
            self.committed_height_snapshot(),
            self.frontier_slot.as_ref().map(|slot| slot.height),
        ) {
            CompletedQuorumViewAdvanceRoute::ExactSlot => {
                let _ = self.handle_frontier_slot_event(
                    now,
                    super::FrontierSlotEvent::OnViewAdvanceRequested {
                        cause,
                        requested_view: view,
                    },
                );
            }
            CompletedQuorumViewAdvanceRoute::ExactFallback => {
                self.frontier_slot = None;
                let _ = self.handle_frontier_slot_event(
                    now,
                    super::FrontierSlotEvent::OnViewAdvanceRequested {
                        cause,
                        requested_view: view,
                    },
                );
            }
            CompletedQuorumViewAdvanceRoute::Generic => {
                self.trigger_view_change_with_cause(height, view, cause);
            }
        }
    }

    pub(super) fn reschedule_stale_pending_blocks(
        &mut self,
        tick_deadline: Option<Instant>,
    ) -> bool {
        self.reschedule_stale_pending_blocks_with_now(Instant::now(), tick_deadline)
    }

    pub(super) fn rbc_availability_unresolved_for_reschedule(
        &self,
        key: super::rbc_store::SessionKey,
        commit_topology: &super::network_topology::Topology,
        stall_age: Duration,
        availability_timeout: Duration,
    ) -> bool {
        let session = self
            .subsystems
            .da_rbc
            .rbc
            .sessions
            .get(&key)
            .map(|session| RbcAvailabilityRescheduleSession {
                invalid: session.is_invalid(),
                malformed_chunk_shape: rbc_session_has_invalid_chunk_shape(session),
                delivered: session.delivered,
                complete_delivery: rbc_session_has_complete_delivery(session),
                total_chunks: session.total_chunks(),
                received_chunks: session.received_chunks(),
                ready_signatures: session.ready_signatures.len(),
                required_ready: Self::rbc_protocol_deliver_quorum(commit_topology),
            });
        rbc_availability_unresolved_for_reschedule_decision(
            self.runtime_da_enabled(),
            stall_age,
            availability_timeout,
            self.block_payload_available_locally(key.0),
            self.subsystems.da_rbc.rbc.pending.contains_key(&key),
            session,
        )
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn reschedule_stale_pending_blocks_with_now(
        &mut self,
        now: Instant,
        tick_deadline: Option<Instant>,
    ) -> bool {
        if self.pending.pending_blocks.is_empty() {
            return false;
        }
        let committed_height = self.committed_height_snapshot();
        // Allow pruning aborted or obsolete payloads even when no active pending blocks remain.
        let has_aborted = self
            .pending
            .pending_blocks
            .values()
            .any(|pending| pending.aborted);
        let has_obsolete_non_aborted = self
            .pending
            .pending_blocks
            .values()
            .any(|pending| !pending.aborted && pending.height <= committed_height);
        if !has_aborted && !has_obsolete_non_aborted && self.active_pending_blocks_len() == 0 {
            return false;
        }

        let reschedule_start = Instant::now();
        let mut budget_exhausted = false;
        let mut active_roster: Option<Vec<PeerId>> = None;
        let local_peer_id = self.common_config.peer.id().clone();
        let da_enabled = self.runtime_da_enabled();
        let quorum_timeout = self.quorum_timeout(da_enabled);
        let quorum_reschedule_cooldown =
            super::quorum_reschedule_backoff_from_timeout(quorum_timeout);
        let quorum_reschedule_retention = quorum_timeout.max(QUORUM_RESCHEDULE_COOLDOWN);
        let availability_timeout = self.availability_timeout(quorum_timeout, da_enabled);
        // Keep aborted payloads long enough for missing-block fetches after reschedule drops.
        let retention_factor = self
            .config
            .recovery
            .missing_block_signer_fallback_attempts
            .saturating_add(2)
            .max(4);
        let aborted_retention = quorum_reschedule_retention.saturating_mul(retention_factor);
        let queue_depths = super::status::worker_queue_depth_snapshot();
        let relay_backpressure = self.relay_backpressure_active(now, self.rebroadcast_cooldown());
        let tip_height = self.state.committed_height();
        let tip_hash = self.state.latest_block_hash_fast();
        let fast_timeout_permissioned = self.pending_fast_path_timeout_current();
        let fast_timeout_npos = self.pending_fast_path_timeout_current();

        let mut stale_pending = Vec::new();
        let mut aborted_expired = Vec::new();
        let mut to_reschedule = Vec::new();
        let mut prevote_timeouts = Vec::new();
        let mut near_quorum_recovery_candidates: Vec<(
            super::rbc_store::SessionKey,
            Duration,
            Duration,
            usize,
            usize,
        )> = Vec::new();
        let mut preemptive_vote_retransmit_candidates: Vec<(
            super::rbc_store::SessionKey,
            usize,
            usize,
            Duration,
            Duration,
        )> = Vec::new();
        let mut reschedule_backoff_skipped = 0usize;
        let mut missing_data_backoff_skipped = 0usize;
        let mut quorum_stall_escalations = 0usize;
        let mut near_quorum_preemptive_escalations = 0usize;
        let mut preemptive_vote_retransmits = 0usize;
        let mut stale_removed = 0usize;
        let mut aborted_removed = 0usize;
        for (hash, pending) in &self.pending.pending_blocks {
            if Self::tick_budget_exhausted(tick_deadline, Instant::now()) {
                budget_exhausted = true;
                break;
            }
            if pending.aborted {
                if self.kura.get_block_height_by_hash(*hash).is_some() {
                    aborted_expired.push((*hash, pending.height, pending.view));
                    continue;
                }
                let has_votes = self.stored_votes().any(|vote| {
                    vote.block_hash == *hash
                        && vote.height == pending.height
                        && vote.view == pending.view
                });
                let missing_commit_qc_request = self
                    .pending
                    .missing_commit_qc_requests
                    .get(hash)
                    .is_some_and(|request| {
                        request.height == pending.height
                            && request.view == pending.view
                            && self.missing_commit_qc_request_has_actionable_dependency(
                                *hash,
                                request,
                                committed_height,
                                now,
                            )
                    });
                let expected_epoch = self.epoch_for_height(pending.height);
                let commit_qc_cached = cached_qc_for(
                    &self.qc_cache,
                    crate::sumeragi::consensus::Phase::Commit,
                    *hash,
                    pending.height,
                    pending.view,
                    expected_epoch,
                )
                .is_some();
                if has_votes || missing_commit_qc_request || commit_qc_cached {
                    continue;
                }
                let pending_age = now.saturating_duration_since(pending.inserted_at);
                if pending_age >= aborted_retention {
                    aborted_expired.push((*hash, pending.height, pending.view));
                }
                continue;
            }
            if pending.height <= committed_height {
                info!(
                    height = pending.height,
                    view = pending.view,
                    block = %hash,
                    committed_height,
                    "dropping obsolete pending block at or below committed height"
                );
                stale_pending.push((*hash, pending.height));
                continue;
            }
            if self.kura.get_block_height_by_hash(*hash).is_some() {
                if pending.kura_persisted {
                    debug!(
                        height = pending.height,
                        view = pending.view,
                        block = %hash,
                        "retaining kura-persisted pending block until state commit catches up"
                    );
                } else {
                    info!(
                        height = pending.height,
                        view = pending.view,
                        block = %hash,
                        "dropping pending block already committed in kura"
                    );
                    stale_pending.push((*hash, pending.height));
                    continue;
                }
            }
            if !pending_extends_tip(
                pending.height,
                pending.block.header().prev_block_hash(),
                tip_height,
                tip_hash,
            ) {
                continue;
            }
            if !self.pending_block_has_consensus_evidence(*hash, pending) {
                debug!(
                    height = pending.height,
                    view = pending.view,
                    block = %hash,
                    "skipping quorum reschedule for payload-only pending block"
                );
                continue;
            }
            let (consensus_mode, _, _) = self.consensus_context_for_height(pending.height);
            let pending_age = now.saturating_duration_since(pending.inserted_at);
            let fast_timeout = match consensus_mode {
                ConsensusMode::Permissioned => fast_timeout_permissioned,
                ConsensusMode::Npos => fast_timeout_npos,
            };
            let mut commit_roster =
                self.roster_for_vote_with_mode(*hash, pending.height, pending.view, consensus_mode);
            if commit_roster.is_empty() {
                let fallback =
                    active_roster.get_or_insert_with(|| self.effective_commit_topology());
                commit_roster.clone_from(fallback);
            }
            if commit_roster.is_empty() {
                debug!(
                    height = pending.height,
                    view = pending.view,
                    block = %hash,
                    "skipping reschedule: empty commit roster"
                );
                continue;
            }
            let commit_topology = super::network_topology::Topology::new(commit_roster.clone());
            let min_votes_for_commit = commit_topology.min_votes_for_commit();

            let key = (*hash, pending.height, pending.view);
            let expected_epoch = self.epoch_for_height(pending.height);
            let qc_precommit = cached_qc_for(
                &self.qc_cache,
                crate::sumeragi::consensus::Phase::Commit,
                *hash,
                pending.height,
                pending.view,
                expected_epoch,
            );
            let commit_qc_cached = qc_precommit.is_some();
            let qc_any = qc_precommit.clone().or_else(|| {
                cached_qc_for(
                    &self.qc_cache,
                    crate::sumeragi::consensus::Phase::Prepare,
                    *hash,
                    pending.height,
                    pending.view,
                    expected_epoch,
                )
            });
            let qc_phase = qc_any.as_ref().map(|qc| qc.phase);
            if prevote_quorum_stale(qc_phase, pending_age, quorum_timeout) {
                prevote_timeouts.push((key, pending_age, qc_any, commit_roster));
                continue;
            }
            let (vote_count, quorum_reached, stake_quorum_missing) =
                if pending.commit_qc_observed() || commit_qc_cached {
                    (0, true, false)
                } else {
                    let status = self.commit_vote_quorum_status_for_block_detail(
                        *hash,
                        pending.height,
                        pending.view,
                    );
                    (
                        status.vote_count,
                        status.quorum_reached,
                        status.stake_quorum_missing,
                    )
                };
            let has_qc = pending.commit_qc_observed() || commit_qc_cached || qc_any.is_some();
            let validation_inflight = pending.validation_status == ValidationStatus::Pending
                && self.subsystems.validation.inflight.contains_key(hash);
            let payload_available = da_enabled && self.payload_available_for_da(pending);
            let allow_da_fast_reschedule =
                da_enabled && self.config.pacemaker.da_fast_reschedule && payload_available;
            let has_votes = vote_count > 0;
            let near_commit_quorum = has_votes
                && min_votes_for_commit > 0
                && vote_count < min_votes_for_commit
                && vote_count.saturating_add(1) >= min_votes_for_commit;
            let pending_parent = pending.block.header().prev_block_hash();
            let contiguous_frontier = pending.height == committed_height.saturating_add(1)
                && pending_extends_tip(pending.height, pending_parent, tip_height, tip_hash);
            let rbc_key = (*hash, pending.height, pending.view);
            let rbc_pending_entry = self.subsystems.da_rbc.rbc.pending.contains_key(&rbc_key);
            let required_ready = Self::rbc_protocol_deliver_quorum(&commit_topology);
            let rbc_session_incomplete = da_enabled
                && self
                    .subsystems
                    .da_rbc
                    .rbc
                    .sessions
                    .get(&rbc_key)
                    .is_some_and(|session| {
                        rbc_session_availability_incomplete(
                            session,
                            rbc_pending_entry,
                            required_ready,
                        )
                    });
            let consensus_queue_backlog = queue_depths.rbc_chunk_rx > 0
                || queue_depths.block_payload_rx > 0
                || queue_depths.block_rx > 0
                || queue_depths.consensus_rx > 0;
            let block_payload_threshold =
                Self::near_quorum_queue_depth_threshold(self.config.queues.block_payload);
            let rbc_chunk_threshold =
                Self::near_quorum_queue_depth_threshold(self.config.queues.rbc_chunks);
            let block_threshold =
                Self::near_quorum_queue_depth_threshold(self.config.queues.blocks);
            let consensus_threshold =
                Self::near_quorum_queue_depth_threshold(self.config.queues.control);
            let near_quorum_queue_backlog = queue_depths.rbc_chunk_rx >= rbc_chunk_threshold
                || queue_depths.block_payload_rx >= block_payload_threshold
                || queue_depths.block_rx >= block_threshold
                || queue_depths.consensus_rx >= consensus_threshold;
            let missing_local_data = da_enabled && !payload_available;
            let near_quorum_timeout = near_quorum_payload_timeout(self.rebroadcast_cooldown());
            let near_quorum_fast_timeout_allowed = near_commit_quorum
                && missing_local_data
                && !near_quorum_queue_backlog
                && !rbc_session_incomplete;
            let vote_queue_backlog = queue_depths.vote_rx > 0;
            let vote_backed_validation_inflight = validation_inflight
                && contiguous_frontier
                && has_votes
                && !has_qc
                && !relay_backpressure
                && !rbc_session_incomplete;
            let fast_path_allowed = (!da_enabled || allow_da_fast_reschedule)
                && !has_votes
                && !has_qc
                && !validation_inflight;
            let effective_quorum_timeout = if fast_path_allowed {
                fast_timeout.min(quorum_timeout)
            } else if near_quorum_fast_timeout_allowed {
                near_quorum_timeout.min(quorum_timeout)
            } else {
                quorum_timeout
            };
            if pending_age < fast_timeout && !near_quorum_fast_timeout_allowed {
                continue;
            }
            if validation_inflight && !has_votes && !has_qc {
                debug!(
                    height = pending.height,
                    view = pending.view,
                    block = %hash,
                    pending_age_ms = pending_age.as_millis(),
                    quorum_timeout_ms = effective_quorum_timeout.as_millis(),
                    "deferring quorum reschedule while pre-vote validation is inflight"
                );
                continue;
            }
            if vote_backed_validation_inflight {
                debug!(
                    height = pending.height,
                    view = pending.view,
                    block = %hash,
                    votes = vote_count,
                    min_votes = min_votes_for_commit,
                    pending_age_ms = pending_age.as_millis(),
                    quorum_timeout_ms = effective_quorum_timeout.as_millis(),
                    "deferring vote-backed quorum reschedule while pre-vote validation is inflight"
                );
                continue;
            }
            // Once any vote/QC progress exists, quorum staleness must be measured from the
            // latest observed progress, not from the original block insertion time.
            let quorum_stall_age = if has_votes || has_qc {
                pending.progress_age(now)
            } else {
                pending_age
            };
            let progress_stall_age = if has_votes || has_qc {
                pending.progress_age(now)
            } else {
                pending_age
            };
            let near_quorum_recovery_window = near_quorum_timeout
                .checked_div(2)
                .unwrap_or(near_quorum_timeout)
                .max(Duration::from_millis(200));
            let same_height_dependency_backlog_active = contiguous_frontier
                && self.frontier_recovery_same_height_dependency_backlog_active(
                    pending.height,
                    now,
                    queue_depths,
                );
            let same_height_vote_backed_work_active = contiguous_frontier
                && self.frontier_recovery_same_slot_vote_backed_work_active(
                    pending.height,
                    pending.view,
                    now,
                    false,
                );
            let same_height_rbc_sender_activity_active = contiguous_frontier
                && self
                    .frontier_recovery_same_height_rbc_sender_activity_active(pending.height, now);
            let same_height_fresh_missing_block_request = contiguous_frontier
                && self
                    .pending
                    .missing_block_requests
                    .get(hash)
                    .is_some_and(|request| {
                        request.height == pending.height
                            && request.view == pending.view
                            && matches!(
                                request.phase,
                                crate::sumeragi::consensus::Phase::Prepare
                                    | crate::sumeragi::consensus::Phase::Commit
                            )
                            && (now.saturating_duration_since(request.last_requested)
                                < self
                                    .frontier_recovery_window()
                                    .max(Duration::from_millis(1))
                                || now.saturating_duration_since(request.last_dependency_progress)
                                    < self
                                        .frontier_recovery_window()
                                        .max(Duration::from_millis(1)))
                            && self.missing_block_request_has_actionable_dependency(
                                *hash,
                                request,
                                committed_height,
                                now,
                            )
                    });
            let same_height_missing_block_recovery_backlog_active = contiguous_frontier
                && (queue_depths.block_payload_rx > 0
                    || queue_depths.rbc_chunk_rx > 0
                    || queue_depths.block_rx > 0)
                && self
                    .pending
                    .missing_block_requests
                    .iter()
                    .any(|(missing_hash, request)| {
                        request.height == pending.height
                            && matches!(
                                request.phase,
                                crate::sumeragi::consensus::Phase::Prepare
                                    | crate::sumeragi::consensus::Phase::Commit
                            )
                            && self.missing_block_request_has_actionable_dependency(
                                *missing_hash,
                                request,
                                committed_height,
                                now,
                            )
                    });
            let same_height_quorum_timeout_owner_present = contiguous_frontier
                && self.frontier_recovery.as_ref().is_some_and(|state| {
                    state.frontier_height == pending.height
                        && state.last_cause == "quorum_timeout"
                        && (state.last_action_at.is_some()
                            || state.last_dependency_progress_at.is_some())
                });
            let same_height_quorum_timeout_owner_active = same_height_quorum_timeout_owner_present
                && self.frontier_recovery.as_ref().is_some_and(|state| {
                    now.saturating_duration_since(state.entered_at)
                        < self
                            .frontier_recovery_window()
                            .max(Duration::from_millis(1))
                });
            let same_height_non_owner_vote_retransmit_blocker = same_height_vote_backed_work_active
                || same_height_rbc_sender_activity_active
                || same_height_fresh_missing_block_request
                || same_height_missing_block_recovery_backlog_active;
            let same_height_actionable_progress_active = same_height_dependency_backlog_active
                || same_height_vote_backed_work_active
                || same_height_rbc_sender_activity_active
                || same_height_fresh_missing_block_request
                || same_height_missing_block_recovery_backlog_active
                || same_height_quorum_timeout_owner_active;
            let same_height_vote_recovery_active = same_height_vote_backed_work_active
                || same_height_fresh_missing_block_request
                || same_height_quorum_timeout_owner_active;
            if near_commit_quorum
                && missing_local_data
                && !quorum_reached
                && !near_quorum_queue_backlog
                && progress_stall_age >= near_quorum_recovery_window
            {
                near_quorum_recovery_candidates.push((
                    rbc_key,
                    progress_stall_age,
                    near_quorum_recovery_window,
                    vote_count,
                    min_votes_for_commit,
                ));
            }
            let rbc_availability_unresolved = self.rbc_availability_unresolved_for_reschedule(
                rbc_key,
                &commit_topology,
                pending_age,
                availability_timeout,
            );
            let contiguous_frontier_fast_resend_window =
                contiguous_frontier_vote_backed_fast_resend_window(
                    self.rebroadcast_cooldown(),
                    pending.height == committed_height.saturating_add(1),
                    vote_count,
                    min_votes_for_commit,
                    relay_backpressure,
                    vote_queue_backlog || consensus_queue_backlog,
                    rbc_availability_unresolved,
                );
            let same_height_quorum_timeout_owner_rearm_window = if contiguous_frontier
                && has_votes
                && !has_qc
                && same_height_quorum_timeout_owner_present
                && !same_height_non_owner_vote_retransmit_blocker
                && !validation_inflight
                && !missing_local_data
                && !rbc_availability_unresolved
            {
                let resend_window = contiguous_frontier_vote_backed_resend_window(
                    self.rebroadcast_cooldown(),
                    vote_count,
                    min_votes_for_commit,
                );
                self.frontier_recovery.as_ref().and_then(|state| {
                    let last_owner_action = state
                        .last_action_at
                        .or(state.last_dependency_progress_at)
                        .unwrap_or(state.entered_at);
                    (now.saturating_duration_since(last_owner_action) >= resend_window)
                        .then_some(resend_window)
                })
            } else {
                None
            };
            let vote_backed_frontier_resend_window = contiguous_frontier_fast_resend_window
                .or(same_height_quorum_timeout_owner_rearm_window);
            let vote_backed_retransmit_allowed_under_same_height_recovery =
                same_height_quorum_timeout_owner_rearm_window.is_some()
                    || (!same_height_actionable_progress_active && !consensus_queue_backlog);
            if let Some(fast_resend_window) = vote_backed_frontier_resend_window
                && preemptive_vote_backed_retransmit_candidate(
                    true,
                    has_votes,
                    has_qc,
                    validation_inflight,
                    missing_local_data,
                    vote_backed_retransmit_allowed_under_same_height_recovery,
                    progress_stall_age,
                    fast_resend_window,
                    effective_quorum_timeout,
                    pending.precommit_rebroadcast_due(now, fast_resend_window),
                )
            {
                preemptive_vote_retransmit_candidates.push((
                    key,
                    vote_count,
                    min_votes_for_commit,
                    progress_stall_age,
                    fast_resend_window,
                ));
            }
            if missing_quorum_stale(quorum_stall_age, effective_quorum_timeout, quorum_reached) {
                if rbc_session_incomplete && pending_age < availability_timeout {
                    debug!(
                        height = pending.height,
                        view = pending.view,
                        block = %hash,
                        pending_age_ms = pending_age.as_millis(),
                        progress_stall_age_ms = progress_stall_age.as_millis(),
                        quorum_timeout_ms = effective_quorum_timeout.as_millis(),
                        "deferring quorum reschedule while RBC session is incomplete"
                    );
                    continue;
                }
                let backlog_extension_active = if contiguous_frontier {
                    same_height_actionable_progress_active || rbc_session_incomplete
                } else {
                    consensus_queue_backlog || rbc_session_incomplete
                };
                let near_quorum_recent_progress_grace =
                    super::saturating_mul_duration(self.rebroadcast_cooldown(), 4)
                        .max(Duration::from_millis(200));
                let first_vote_backed_frontier_quiet_window =
                    super::saturating_mul_duration(self.rebroadcast_cooldown(), 8)
                        .max(Duration::from_millis(400));
                let first_single_vote_frontier_quiet_window =
                    if vote_count == 1 && contiguous_frontier && !near_commit_quorum {
                        first_vote_backed_frontier_quiet_window.max(effective_quorum_timeout)
                    } else {
                        first_vote_backed_frontier_quiet_window
                    };
                let zero_vote_backlog_grace =
                    super::saturating_mul_duration(self.rebroadcast_cooldown(), 8)
                        .max(Duration::from_millis(400));
                let zero_vote_backlog_deadline_base = effective_quorum_timeout
                    .saturating_add(zero_vote_backlog_grace)
                    .max(availability_timeout);
                let zero_vote_backlog_deadline = self.backlog_extended_view_change_timeout(
                    zero_vote_backlog_deadline_base,
                    backlog_extension_active,
                );
                let vote_backlog_grace =
                    super::saturating_mul_duration(self.rebroadcast_cooldown(), 8)
                        .max(Duration::from_millis(400));
                let vote_backlog_deadline_base =
                    availability_timeout.saturating_add(vote_backlog_grace);
                let vote_backlog_deadline = self.backlog_extended_view_change_timeout(
                    vote_backlog_deadline_base,
                    backlog_extension_active,
                );
                if has_votes
                    && contiguous_frontier
                    && vote_queue_backlog
                    && progress_stall_age < vote_backlog_deadline
                {
                    debug!(
                        height = pending.height,
                        view = pending.view,
                        block = %hash,
                        votes = vote_count,
                        min_votes = min_votes_for_commit,
                        progress_stall_age_ms = progress_stall_age.as_millis(),
                        availability_timeout_ms = availability_timeout.as_millis(),
                        vote_backlog_grace_ms = vote_backlog_grace.as_millis(),
                        vote_backlog_deadline_base_ms = vote_backlog_deadline_base.as_millis(),
                        vote_backlog_deadline_ms = vote_backlog_deadline.as_millis(),
                        vote_rx_depth = queue_depths.vote_rx,
                        "deferring quorum reschedule: vote-backed block still has queued votes to drain"
                    );
                    continue;
                }
                if !has_votes
                    && same_height_actionable_progress_active
                    && progress_stall_age < zero_vote_backlog_deadline
                {
                    let same_slot_ingress_active = self.frontier_recovery_same_slot_ingress_active(
                        pending.height,
                        pending.view,
                        now,
                        queue_depths,
                    );
                    if contiguous_frontier
                        && self.frontier_recovery.is_none()
                        && (same_height_missing_block_recovery_backlog_active
                            || same_height_dependency_backlog_active
                            || same_slot_ingress_active)
                    {
                        self.frontier_recovery = Some(super::FrontierRecoveryState {
                            frontier_height: pending.height,
                            phase: super::FrontierRecoveryPhase::CatchUp,
                            entered_at: now,
                            last_progress_at: now,
                            last_dependency_progress_at:
                                (same_height_missing_block_recovery_backlog_active
                                    || same_height_dependency_backlog_active)
                                    .then_some(now),
                            last_action_at: None,
                            no_progress_windows: 0,
                            cleanup_done: false,
                            last_view: pending.view,
                            last_rotation_view: None,
                            last_cause: "quorum_timeout",
                        });
                    }
                    debug!(
                        height = pending.height,
                        view = pending.view,
                        block = %hash,
                        progress_stall_age_ms = progress_stall_age.as_millis(),
                        quorum_timeout_ms = effective_quorum_timeout.as_millis(),
                        availability_timeout_ms = availability_timeout.as_millis(),
                        zero_vote_backlog_grace_ms = zero_vote_backlog_grace.as_millis(),
                        zero_vote_backlog_deadline_base_ms = zero_vote_backlog_deadline_base.as_millis(),
                        zero_vote_backlog_deadline_ms = zero_vote_backlog_deadline.as_millis(),
                        block_payload_rx_depth = queue_depths.block_payload_rx,
                        rbc_chunk_rx_depth = queue_depths.rbc_chunk_rx,
                        block_rx_depth = queue_depths.block_rx,
                        consensus_rx_depth = queue_depths.consensus_rx,
                        "deferring quorum reschedule: zero-vote block still has same-height recovery progress in flight"
                    );
                    continue;
                }
                let same_slot_ingress_active = contiguous_frontier
                    && !has_votes
                    && self.frontier_recovery_same_slot_ingress_active(
                        pending.height,
                        pending.view,
                        now,
                        queue_depths,
                    );
                let availability_ingress_active =
                    queue_depths.block_payload_rx > 0 || queue_depths.rbc_chunk_rx > 0;
                if same_slot_ingress_active
                    && (availability_ingress_active
                        || progress_stall_age < zero_vote_backlog_deadline)
                {
                    if self.frontier_recovery.is_none() {
                        self.frontier_recovery = Some(super::FrontierRecoveryState {
                            frontier_height: pending.height,
                            phase: super::FrontierRecoveryPhase::CatchUp,
                            entered_at: now,
                            last_progress_at: now,
                            last_dependency_progress_at: None,
                            last_action_at: None,
                            no_progress_windows: 0,
                            cleanup_done: false,
                            last_view: pending.view,
                            last_rotation_view: None,
                            last_cause: "quorum_timeout",
                        });
                    }
                    continue;
                }
                if has_votes
                    && contiguous_frontier
                    && pending.last_quorum_reschedule.is_none()
                    && !consensus_queue_backlog
                    && !rbc_session_incomplete
                    && !relay_backpressure
                    && !near_quorum_fast_timeout_allowed
                    && progress_stall_age < first_single_vote_frontier_quiet_window
                {
                    debug!(
                        height = pending.height,
                        view = pending.view,
                        block = %hash,
                        votes = vote_count,
                        min_votes = min_votes_for_commit,
                        progress_stall_age_ms = progress_stall_age.as_millis(),
                        quiet_window_ms = first_single_vote_frontier_quiet_window.as_millis(),
                        "deferring quorum reschedule: first contiguous-frontier vote progress is still settling"
                    );
                    continue;
                }
                if near_commit_quorum
                    && !near_quorum_queue_backlog
                    && progress_stall_age < near_quorum_recent_progress_grace
                {
                    debug!(
                        height = pending.height,
                        view = pending.view,
                        block = %hash,
                        votes = vote_count,
                        min_votes = min_votes_for_commit,
                        progress_stall_age_ms = progress_stall_age.as_millis(),
                        grace_ms = near_quorum_recent_progress_grace.as_millis(),
                        "deferring quorum reschedule: near quorum with recent vote progress"
                    );
                    continue;
                }
                if has_votes
                    && !near_commit_quorum
                    && same_height_vote_recovery_active
                    && progress_stall_age < vote_backlog_deadline
                {
                    debug!(
                        height = pending.height,
                        view = pending.view,
                        block = %hash,
                        votes = vote_count,
                        min_votes = min_votes_for_commit,
                        progress_stall_age_ms = progress_stall_age.as_millis(),
                        availability_timeout_ms = availability_timeout.as_millis(),
                        vote_backlog_grace_ms = vote_backlog_grace.as_millis(),
                        vote_backlog_deadline_base_ms = vote_backlog_deadline_base.as_millis(),
                        vote_backlog_deadline_ms = vote_backlog_deadline.as_millis(),
                        block_payload_rx_depth = queue_depths.block_payload_rx,
                        rbc_chunk_rx_depth = queue_depths.rbc_chunk_rx,
                        block_rx_depth = queue_depths.block_rx,
                        consensus_rx_depth = queue_depths.consensus_rx,
                        "deferring quorum reschedule: vote-backed block still has same-height recovery progress in flight"
                    );
                    continue;
                }
                if near_commit_quorum
                    && same_height_actionable_progress_active
                    && progress_stall_age < vote_backlog_deadline
                {
                    debug!(
                        height = pending.height,
                        view = pending.view,
                        block = %hash,
                        votes = vote_count,
                        min_votes = min_votes_for_commit,
                        progress_stall_age_ms = progress_stall_age.as_millis(),
                        availability_timeout_ms = availability_timeout.as_millis(),
                        vote_backlog_grace_ms = vote_backlog_grace.as_millis(),
                        vote_backlog_deadline_base_ms = vote_backlog_deadline_base.as_millis(),
                        vote_backlog_deadline_ms = vote_backlog_deadline.as_millis(),
                        block_payload_rx_depth = queue_depths.block_payload_rx,
                        rbc_chunk_rx_depth = queue_depths.rbc_chunk_rx,
                        block_rx_depth = queue_depths.block_rx,
                        consensus_rx_depth = queue_depths.consensus_rx,
                        "deferring quorum reschedule: near quorum while same-height recovery is still progressing"
                    );
                    continue;
                }
                if rbc_availability_unresolved {
                    debug!(
                        height = pending.height,
                        view = pending.view,
                        block = %hash,
                        pending_age_ms = pending_age.as_millis(),
                        progress_stall_age_ms = progress_stall_age.as_millis(),
                        quorum_timeout_ms = effective_quorum_timeout.as_millis(),
                        "deferring quorum reschedule while RBC availability is unresolved"
                    );
                    continue;
                }
                if (missing_local_data
                    || matches!(pending.last_gate, Some(GateReason::MissingLocalData)))
                    && pending_age < availability_timeout
                    && !near_quorum_fast_timeout_allowed
                {
                    missing_data_backoff_skipped = missing_data_backoff_skipped.saturating_add(1);
                    continue;
                }
                let (effective_reschedule_backoff, stall_escalated) =
                    adaptive_quorum_reschedule_backoff(
                        quorum_reschedule_cooldown,
                        quorum_stall_age,
                        effective_quorum_timeout,
                        vote_count,
                        min_votes_for_commit,
                    );
                let effective_reschedule_backoff = if near_quorum_fast_timeout_allowed {
                    effective_reschedule_backoff
                        .min(near_quorum_timeout.max(Duration::from_millis(1)))
                } else if let Some(fast_resend_window) = vote_backed_frontier_resend_window {
                    effective_reschedule_backoff.min(fast_resend_window)
                } else {
                    effective_reschedule_backoff
                };
                let zero_vote_reschedule = !has_votes && !has_qc;
                let effective_reschedule_backoff = consensus_ingress_reschedule_backoff(
                    effective_reschedule_backoff,
                    if zero_vote_reschedule {
                        consensus_queue_backlog
                    } else {
                        queue_depths.consensus_rx > 0
                    },
                    zero_vote_reschedule && near_quorum_queue_backlog,
                );
                if stall_escalated {
                    quorum_stall_escalations = quorum_stall_escalations.saturating_add(1);
                    super::status::inc_quorum_stall_age_escalation();
                }
                let reschedule_due = if has_votes || has_qc {
                    pending.vote_backed_reschedule_due(
                        now,
                        effective_reschedule_backoff,
                        vote_count,
                    ) || (contiguous_frontier
                        && pending.last_quorum_reschedule.is_some_and(|last| {
                            now.saturating_duration_since(last) >= effective_reschedule_backoff
                        }))
                } else {
                    pending.reschedule_due(now, effective_reschedule_backoff)
                };
                if !reschedule_due {
                    reschedule_backoff_skipped = reschedule_backoff_skipped.saturating_add(1);
                    continue;
                }
                let bundle_window_override = if near_quorum_fast_timeout_allowed
                    || vote_backed_frontier_resend_window.is_some()
                {
                    Some(effective_reschedule_backoff)
                } else {
                    None
                };
                to_reschedule.push((
                    key,
                    pending_age,
                    quorum_stall_age,
                    vote_count,
                    min_votes_for_commit,
                    stake_quorum_missing,
                    effective_reschedule_backoff,
                    bundle_window_override,
                ));
            }
        }

        let mut near_quorum_preemptive_progress = false;
        if !near_quorum_recovery_candidates.is_empty() {
            let fetch_freshness_cap =
                super::saturating_mul_duration(self.rebroadcast_cooldown(), 2)
                    .max(Duration::from_millis(1));
            for (key, progress_stall_age, recovery_window, vote_count, min_votes) in
                near_quorum_recovery_candidates
                    .into_iter()
                    .take(NEAR_QUORUM_PREEMPTIVE_RECOVERY_PER_TICK)
            {
                if Self::tick_budget_exhausted(tick_deadline, Instant::now()) {
                    budget_exhausted = true;
                    break;
                }
                if self.pending.pending_blocks.get(&key.0).is_none() {
                    continue;
                }
                if self
                    .pending
                    .missing_block_requests
                    .get(&key.0)
                    .is_some_and(|request| {
                        let actionable = self.missing_block_request_has_actionable_dependency(
                            key.0,
                            request,
                            committed_height,
                            now,
                        );
                        let request_age = now.saturating_duration_since(request.last_requested);
                        near_quorum_fresh_missing_block_request_suppresses(
                            request.height == key.1,
                            request.view == key.2,
                            actionable,
                            request_age,
                            request.retry_window,
                            fetch_freshness_cap,
                        )
                    })
                {
                    debug!(
                        height = key.1,
                        view = key.2,
                        block = %key.0,
                        "suppressing duplicate pre-timeout near-quorum escalation while missing-block fetch is still fresh"
                    );
                    continue;
                }
                if self.should_skip_missing_block_recovery_escalation(key.0, key.1, key.2, now) {
                    debug!(
                        height = key.1,
                        view = key.2,
                        block = %key.0,
                        "suppressing duplicate pre-timeout near-quorum escalation while prior recovery is in-flight"
                    );
                    continue;
                }
                if self.maybe_escalate_missing_block_height_recovery(key.0, key.1, key.2, now) {
                    near_quorum_preemptive_escalations =
                        near_quorum_preemptive_escalations.saturating_add(1);
                    near_quorum_preemptive_progress = true;
                    debug!(
                        height = key.1,
                        view = key.2,
                        block = %key.0,
                        votes = vote_count,
                        min_votes,
                        progress_stall_age_ms = progress_stall_age.as_millis(),
                        escalation_window_ms = recovery_window.as_millis(),
                        "triggered pre-timeout near-quorum missing-payload recovery escalation"
                    );
                }
            }
        }
        let scan_done = Instant::now();

        let to_reschedule_len = to_reschedule.len();
        let prevote_timeout_len = prevote_timeouts.len();

        for (hash, height, view) in aborted_expired {
            if Self::tick_budget_exhausted(tick_deadline, Instant::now()) {
                budget_exhausted = true;
                break;
            }
            let expected_epoch = self.epoch_for_height(height);
            let keep_commit_qc = cached_qc_for(
                &self.qc_cache,
                crate::sumeragi::consensus::Phase::Commit,
                hash,
                height,
                view,
                expected_epoch,
            )
            .is_some();
            if !keep_commit_qc {
                self.clean_rbc_sessions_for_block(hash, height);
            }
            self.qc_cache.retain(|(phase, qc_hash, _, _, _, _, _), _| {
                *qc_hash != hash
                    || (keep_commit_qc
                        && matches!(phase, crate::sumeragi::consensus::Phase::Commit))
            });
            self.qc_signer_tally
                .retain(|(phase, qc_hash, _, _, _, _, _), _| {
                    *qc_hash != hash
                        || (keep_commit_qc
                            && matches!(phase, crate::sumeragi::consensus::Phase::Commit))
                });
            self.block_signer_cache.remove_block(&hash);
            self.pending.pending_blocks.remove(&hash);
            self.clear_validation_ownership_for_block(hash);
            aborted_removed = aborted_removed.saturating_add(1);
        }

        for (hash, height) in stale_pending {
            if Self::tick_budget_exhausted(tick_deadline, Instant::now()) {
                budget_exhausted = true;
                break;
            }
            self.pending.pending_blocks.remove(&hash);
            self.clear_validation_ownership_for_block(hash);
            self.clean_rbc_sessions_for_block(hash, height);
            self.qc_cache
                .retain(|(_, qc_hash, _, _, _, _, _), _| qc_hash != &hash);
            self.qc_signer_tally
                .retain(|(_, qc_hash, _, _, _, _, _), _| qc_hash != &hash);
            self.block_signer_cache.remove_block(&hash);
            stale_removed = stale_removed.saturating_add(1);
        }

        let mut progress =
            near_quorum_preemptive_progress || aborted_removed > 0 || stale_removed > 0;
        for (key, vote_count, min_votes, progress_stall_age, fast_resend_window) in
            preemptive_vote_retransmit_candidates
        {
            if Self::tick_budget_exhausted(tick_deadline, Instant::now()) {
                budget_exhausted = true;
                break;
            }
            if self.pending.pending_blocks.get(&key.0).is_none() {
                continue;
            }
            let action_taken = self.preemptive_rebroadcast_vote_backed_frontier_block(
                key,
                min_votes,
                vote_count,
                progress_stall_age,
                fast_resend_window,
                now,
            );
            if action_taken {
                preemptive_vote_retransmits = preemptive_vote_retransmits.saturating_add(1);
                progress = true;
            }
        }
        for (
            key,
            age,
            quorum_stall_age,
            vote_count,
            min_votes,
            _stake_quorum_missing,
            effective_reschedule_backoff,
            bundle_window_override,
        ) in to_reschedule
        {
            if Self::tick_budget_exhausted(tick_deadline, Instant::now()) {
                budget_exhausted = true;
                break;
            }
            if let Some(pending) = self.pending.pending_blocks.remove(&key.0) {
                self.subsystems.validation.inflight.remove(&key.0);
                self.subsystems.validation.superseded_results.remove(&key.0);
                let action_taken = self.reschedule_pending_quorum_block(
                    pending,
                    age,
                    quorum_stall_age,
                    min_votes,
                    vote_count,
                    quorum_timeout,
                    effective_reschedule_backoff,
                    bundle_window_override,
                    now,
                );
                progress |= action_taken;
            }
        }

        for (key, pending_age, qc, commit_roster) in prevote_timeouts {
            if Self::tick_budget_exhausted(tick_deadline, Instant::now()) {
                budget_exhausted = true;
                break;
            }
            if let Some(pending) = self.pending.pending_blocks.remove(&key.0) {
                self.subsystems.validation.inflight.remove(&key.0);
                self.subsystems.validation.superseded_results.remove(&key.0);
                let roster_len = commit_roster.len();
                let vote_count = qc
                    .as_ref()
                    .map_or(0, |qc| qc_voting_signer_count(qc, roster_len));
                let txs: Vec<_> = pending.block.external_entrypoints_cloned().collect();
                let (requeued, failures, _duplicate_failures, _gossip_hashes) =
                    requeue_block_transactions(self.queue.as_ref(), self.state.as_ref(), txs);
                if relay_backpressure {
                    debug!(
                        height = key.1,
                        view = key.2,
                        block = %key.0,
                        "skipping prevote-timeout rebroadcast due to relay backpressure"
                    );
                } else {
                    let msg = Arc::new(BlockMessage::BlockCreated(super::message::BlockCreated {
                        block: pending.block.clone(),
                        frontier: None,
                    }));
                    let encoded = Arc::new(BlockMessageWire::encode_message(msg.as_ref()));
                    for peer in &commit_roster {
                        if peer == &local_peer_id {
                            continue;
                        }
                        self.schedule_background(BackgroundRequest::Post {
                            peer: peer.clone(),
                            msg: BlockMessageWire::with_encoded(
                                Arc::clone(&msg),
                                Arc::clone(&encoded),
                            ),
                        });
                    }
                    if let Some(qc) = qc {
                        let msg = Arc::new(BlockMessage::Qc(qc.clone()));
                        let encoded = Arc::new(BlockMessageWire::encode_message(msg.as_ref()));
                        for peer in &commit_roster {
                            if peer == &local_peer_id {
                                continue;
                            }
                            self.schedule_background(BackgroundRequest::Post {
                                peer: peer.clone(),
                                msg: BlockMessageWire::with_encoded(
                                    Arc::clone(&msg),
                                    Arc::clone(&encoded),
                                ),
                            });
                        }
                    }
                }
                #[cfg(feature = "telemetry")]
                self.telemetry.inc_prevote_timeout(self.mode_tag());
                super::status::inc_prevote_timeout();
                self.clean_rbc_sessions_for_block(key.0, key.1);
                self.qc_cache
                    .retain(|(_, qc_hash, _, _, _, _, _), _| qc_hash != &key.0);
                self.qc_signer_tally
                    .retain(|(_, qc_hash, _, _, _, _, _), _| qc_hash != &key.0);
                self.block_signer_cache.remove_block(&key.0);
                if let Some(highest) = self.highest_qc {
                    if highest.subject_block_hash == key.0
                        && highest.height == key.1
                        && highest.view == key.2
                    {
                        if let Some(committed) = self.latest_committed_qc() {
                            self.highest_qc = Some(committed);
                            super::status::set_highest_qc(committed.height, committed.view);
                            super::status::set_highest_qc_hash(committed.subject_block_hash);
                        } else {
                            self.highest_qc = None;
                            super::status::set_highest_qc(0, 0);
                            super::status::set_highest_qc_hash(
                                HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed(
                                    [0; Hash::LENGTH],
                                )),
                            );
                        }
                    }
                }
                let queue_depths = super::status::worker_queue_depth_snapshot();
                warn!(
                    block = %key.0,
                    height = key.1,
                    view = key.2,
                    pending_age_ms = pending_age.as_millis(),
                    quorum_timeout_ms = quorum_timeout.as_millis(),
                    vote_count,
                    requeued,
                    failures,
                    vote_rx_depth = queue_depths.vote_rx,
                    block_payload_rx_depth = queue_depths.block_payload_rx,
                    rbc_chunk_rx_depth = queue_depths.rbc_chunk_rx,
                    block_rx_depth = queue_depths.block_rx,
                    consensus_rx_depth = queue_depths.consensus_rx,
                    lane_relay_rx_depth = queue_depths.lane_relay_rx,
                    background_rx_depth = queue_depths.background_rx,
                    "prevote quorum stalled; rebroadcasting and rotating view"
                );
                self.trigger_view_change_with_cause(
                    key.1,
                    key.2,
                    view_change_cause_for_quorum(vote_count, false),
                );
                progress = true;
            }
        }

        let scan_cost = scan_done.saturating_duration_since(reschedule_start);
        let total_cost = reschedule_start.elapsed();
        if total_cost >= RESCHEDULE_TIMING_LOG_THRESHOLD
            || progress
            || reschedule_backoff_skipped > 0
            || missing_data_backoff_skipped > 0
            || stale_removed > 0
            || aborted_removed > 0
        {
            iroha_logger::info!(
                pending = self.pending.pending_blocks.len(),
                rescheduled = to_reschedule_len,
                prevote_timeouts = prevote_timeout_len,
                stale_removed,
                aborted_removed,
                backoff_skipped = reschedule_backoff_skipped,
                missing_data_skipped = missing_data_backoff_skipped,
                stall_escalations = quorum_stall_escalations,
                near_quorum_preemptive_escalations,
                preemptive_vote_retransmits,
                budget_exhausted,
                scan_ms = scan_cost.as_millis(),
                total_ms = total_cost.as_millis(),
                "reschedule sweep timing"
            );
        }

        progress
    }

    fn preemptive_rebroadcast_vote_backed_frontier_block(
        &mut self,
        key: super::rbc_store::SessionKey,
        min_votes_for_commit: usize,
        vote_count: usize,
        progress_stall_age: Duration,
        fast_resend_window: Duration,
        now: Instant,
    ) -> bool {
        let Some(mut pending) = self.pending.pending_blocks.remove(&key.0) else {
            return false;
        };
        let (consensus_mode, _, _) = self.consensus_context_for_height(key.1);
        let vote_roster_peers = self.roster_for_vote_with_mode(key.0, key.1, key.2, consensus_mode);
        let commit_topology_peers = if vote_roster_peers.is_empty() {
            self.effective_commit_topology()
        } else {
            Vec::new()
        };
        let target_source = preemptive_vote_backed_retransmit_target_source(
            true,
            true,
            !vote_roster_peers.is_empty(),
            !commit_topology_peers.is_empty(),
        );
        let topology_peers = match target_source {
            PreemptiveVoteBackedRetransmitTargetSource::NoSource => {
                self.pending.pending_blocks.insert(key.0, pending);
                return false;
            }
            PreemptiveVoteBackedRetransmitTargetSource::VoteRoster => vote_roster_peers,
            PreemptiveVoteBackedRetransmitTargetSource::CommitTopology => commit_topology_peers,
        };
        if topology_peers.is_empty() {
            self.pending.pending_blocks.insert(key.0, pending);
            return false;
        }
        let rebroadcast = self.rebroadcast_pending_block_updates(
            &mut pending,
            key.0,
            key.1,
            key.2,
            false,
            &topology_peers,
            min_votes_for_commit,
            vote_count,
            preemptive_vote_backed_retransmit_widen_fanout(vote_count, min_votes_for_commit),
            now,
        );
        let action_taken = preemptive_vote_backed_retransmit_action(
            rebroadcast.votes,
            rebroadcast.block_sync,
            rebroadcast.block,
        );
        self.pending.pending_blocks.insert(key.0, pending);
        if action_taken {
            debug!(
                block = %key.0,
                height = key.1,
                view = key.2,
                votes = vote_count,
                min_votes = min_votes_for_commit,
                progress_stall_age_ms = progress_stall_age.as_millis(),
                resend_window_ms = fast_resend_window.as_millis(),
                rebroadcasted_votes = rebroadcast.votes,
                rebroadcasted_block_sync = rebroadcast.block_sync,
                rebroadcasted_block = rebroadcast.block,
                "triggered pre-timeout vote-backed frontier retransmit"
            );
        }
        action_taken
    }

    fn maybe_handoff_isolated_vote_backed_frontier_to_anchor(
        &mut self,
        block_hash: HashOf<BlockHeader>,
        height: u64,
        view: u64,
        vote_count: usize,
        min_votes_for_commit: usize,
        now: Instant,
    ) -> bool {
        let cached_commit_qc = self
            .cached_commit_qc_for_block(block_hash, height, view)
            .is_some();
        let admission = isolated_vote_backed_handoff_admission(
            self.config.resilience.enabled,
            vote_count,
            min_votes_for_commit,
            height,
            self.committed_height_snapshot(),
            cached_commit_qc,
        );
        if !admission {
            return false;
        }

        let _ = self.seed_frontier_recovery_for_quorum_timeout(height, view, now);
        let _ = self.handle_frontier_slot_event(
            now,
            super::FrontierSlotEvent::OnBodyAvailable {
                block_hash,
                view,
                sender: None,
            },
        );
        let Some(slot) = self.frontier_slot.as_ref() else {
            return false;
        };
        let slot_valid = isolated_vote_backed_handoff_slot_valid(
            true,
            slot.height == height,
            slot.view == view,
            slot.block_hash == block_hash,
            slot.body_present(),
            slot.quorum_progress.commit_qc_observed,
            self.frontier_slot_has_vote_backed_owner_state_in_slot(slot),
        );
        let requests_anchor = isolated_vote_backed_handoff_requests_anchor(admission, slot_valid);
        if !requests_anchor {
            return false;
        }

        let requested =
            self.request_range_pull_from_anchor(height, ISOLATED_VOTE_BACKED_HANDOFF_REASON, now);
        let action = isolated_vote_backed_handoff_action(
            requests_anchor,
            requested,
            isolated_vote_backed_handoff_reason_ok(ISOLATED_VOTE_BACKED_HANDOFF_REASON),
        );
        if action {
            info!(
                block = %block_hash,
                height,
                view,
                votes = vote_count,
                min_votes = min_votes_for_commit,
                "handed isolated vote-backed frontier owner to committed-anchor catch-up"
            );
        }
        action
    }

    #[allow(clippy::too_many_arguments, clippy::too_many_lines)]
    pub(super) fn reschedule_pending_quorum_block(
        &mut self,
        mut pending: PendingBlock,
        pending_age: Duration,
        quorum_stall_age: Duration,
        min_votes_for_commit: usize,
        vote_count: usize,
        quorum_timeout: Duration,
        reschedule_backoff: Duration,
        bundle_window_override: Option<Duration>,
        now: Instant,
    ) -> bool {
        let block_hash = pending.block.hash();
        let height = pending.height;
        let view = pending.view;
        let expected_epoch = self.epoch_for_height(height);
        // Preserve commit QCs so late payloads can still finalize after a drop.
        let keep_commit_qc = cached_qc_for(
            &self.qc_cache,
            crate::sumeragi::consensus::Phase::Commit,
            block_hash,
            height,
            view,
            expected_epoch,
        )
        .is_some();
        let queue_depths = super::status::worker_queue_depth_snapshot();
        let state_height = self.state.committed_height();
        let tip_hash = self.state.latest_block_hash_fast();
        let pending_parent = pending.block.header().prev_block_hash();
        if !pending_extends_tip(height, pending_parent, state_height, tip_hash) {
            debug!(
                ?block_hash,
                height,
                view,
                expected_height = u64::try_from(state_height.saturating_add(1))
                    .unwrap_or(u64::MAX),
                tip_hash = ?tip_hash,
                prev_hash = ?pending_parent,
                "skipping quorum reschedule: pending block not on local tip"
            );
            self.pending.pending_blocks.insert(block_hash, pending);
            return false;
        }

        let (consensus_mode, _, _) = self.consensus_context_for_height(height);
        let mut topology_peers =
            self.roster_for_vote_with_mode(block_hash, height, view, consensus_mode);
        if topology_peers.is_empty() {
            topology_peers = self.effective_commit_topology();
        }
        let emitted_local_vote = if topology_peers.is_empty() {
            false
        } else {
            self.with_registered_pending_block(block_hash, &mut pending, |actor| {
                actor.maybe_emit_local_commit_vote_for_pending_event(
                    block_hash,
                    height,
                    view,
                    &topology_peers,
                    "quorum_reschedule",
                )
            })
        };
        let mut precommit_vote_count =
            self.pending_block_commit_votes_count(block_hash, height, view);
        // Local commit votes are emitted before async vote verification drains into vote_log.
        if precommit_vote_count == 0 && pending.local_commit_vote_emitted() {
            precommit_vote_count = 1;
        }
        let commit_vote_count = vote_count;
        let reschedule_vote_count = precommit_vote_count.max(commit_vote_count);
        let has_reschedule_votes = reschedule_vote_count > 0;
        let frontier_height = u64::try_from(state_height)
            .unwrap_or(u64::MAX)
            .saturating_add(1);
        let contiguous_frontier = height == frontier_height;
        let progress_age = pending.progress_age(now);
        let last_reschedule_ms = pending
            .last_quorum_reschedule
            .map(|ts| now.saturating_duration_since(ts).as_millis());
        let stake_quorum_missing = vote_count > 0
            && self
                .commit_vote_quorum_status_for_block_detail(block_hash, height, view)
                .stake_quorum_missing;
        let direct_view_change_cause =
            view_change_cause_for_quorum(reschedule_vote_count, stake_quorum_missing);
        let local_only_commit_topology =
            topology_peers.len() == 1 && topology_peers[0] == *self.common_config.peer.id();
        let no_commit_evidence = reschedule_vote_count == 0;
        if no_commit_evidence
            && local_only_commit_topology
            && !pending.local_commit_vote_emitted()
            && matches!(pending.validation_status, ValidationStatus::Pending)
        {
            debug!(
                block = %block_hash,
                height,
                view,
                pending_age_ms = pending_age.as_millis(),
                progress_age_ms = progress_age.as_millis(),
                validation_status = ?pending.validation_status,
                "deferring zero-vote quorum reschedule: local-only commit topology is still awaiting its first local vote"
            );
            self.pending.pending_blocks.insert(block_hash, pending);
            return false;
        }
        if local_only_commit_topology && pending.local_commit_vote_emitted() {
            debug!(
                block = %block_hash,
                height,
                view,
                pending_age_ms = pending_age.as_millis(),
                progress_age_ms = progress_age.as_millis(),
                "deferring quorum reschedule: local-only commit topology already emitted its local vote"
            );
            self.pending.pending_blocks.insert(block_hash, pending);
            return false;
        }
        let zero_vote_progress_window = reschedule_backoff.max(Duration::from_millis(1));
        let zero_vote_fast_reschedule_allowed = self.config.pacemaker.da_fast_reschedule
            && self.runtime_da_enabled()
            && self.payload_available_for_da(&pending);
        let da_enabled = self.runtime_da_enabled();
        let availability_timeout = self.availability_timeout(quorum_timeout, da_enabled);
        let missing_local_data_wait_expired = da_enabled
            && matches!(pending.last_gate, Some(GateReason::MissingLocalData))
            && (availability_timeout == Duration::ZERO || pending_age >= availability_timeout);
        if no_commit_evidence
            && pending.last_quorum_reschedule.is_none()
            && progress_age < zero_vote_progress_window
            && !zero_vote_fast_reschedule_allowed
            && !missing_local_data_wait_expired
        {
            debug!(
                block = %block_hash,
                height,
                view,
                votes = vote_count,
                min_votes = min_votes_for_commit,
                pending_age_ms = pending_age.as_millis(),
                progress_age_ms = progress_age.as_millis(),
                reschedule_backoff_ms = reschedule_backoff.as_millis(),
                "deferring zero-vote quorum reschedule: recent pending progress is still within backoff window"
            );
            self.pending.pending_blocks.insert(block_hash, pending);
            return false;
        }
        let passive_frontier_catchup_owner =
            contiguous_frontier && self.frontier_slot_passive_catchup_owns_height(height);
        if passive_frontier_catchup_owner {
            debug!(
                block = %block_hash,
                height,
                view,
                pending_age_ms = pending_age.as_millis(),
                quorum_stall_age_ms = quorum_stall_age.as_millis(),
                "suppressing quorum reschedule while committed-anchor catch-up passively owns the contiguous frontier"
            );
            self.pending.pending_blocks.insert(block_hash, pending);
            return false;
        }
        if contiguous_frontier
            && vote_count == 0
            && !pending.local_commit_vote_emitted()
            && !keep_commit_qc
            && self.seed_frontier_slot_from_same_height_evidence(
                height,
                view,
                now,
                "quorum_timeout",
                false,
            )
        {
            self.pending.pending_blocks.insert(block_hash, pending);
            return false;
        }
        let frontier_slot_owner_was_active =
            contiguous_frontier && self.frontier_slot_has_active_owner_state_for_view(height, view);
        let frontier_slot_present_for_view = contiguous_frontier
            && self
                .frontier_slot
                .as_ref()
                .is_some_and(|slot| slot.height == height && slot.view == view);
        let stale_frontier_slot_owner_for_other_view = contiguous_frontier
            && self.frontier_slot.as_ref().is_some_and(|slot| {
                slot.height == height
                    && slot.view != view
                    && Self::frontier_slot_has_active_owner_state_in_slot(slot)
            });
        let frontier_slot_vote_backed_owner_was_active = contiguous_frontier
            && self.frontier_slot.as_ref().is_some_and(|slot| {
                slot.height == height
                    && slot.view == view
                    && (slot.quorum_progress.votes_observed
                        || slot.quorum_progress.commit_qc_observed
                        || self.slot_has_vote_backed_consensus_evidence(slot.height, slot.view))
            });
        let same_slot_vote_backed_evidence = contiguous_frontier
            && !keep_commit_qc
            && self.slot_has_vote_backed_consensus_evidence(height, view);
        let manifest_gate_pending =
            matches!(pending.last_gate, Some(GateReason::ManifestGuard { .. }));
        if contiguous_frontier
            && !has_reschedule_votes
            && !frontier_slot_owner_was_active
            && !keep_commit_qc
            && self.seed_frontier_slot_from_same_height_evidence(
                height,
                view,
                now,
                "quorum_timeout",
                false,
            )
        {
            self.pending.pending_blocks.insert(block_hash, pending);
            return false;
        }
        let effective_has_reschedule_votes = has_reschedule_votes
            || same_slot_vote_backed_evidence
            || frontier_slot_vote_backed_owner_was_active
            || manifest_gate_pending;
        // Once quorum timeout expires with no same-height evidence, this block is just zombie
        // state: keeping and rebroadcasting it only multiplies conflicting frontier candidates.
        let drop_pending = !effective_has_reschedule_votes;
        let authoritative_payload_present =
            !drop_pending && self.payload_available_for_da(&pending);
        let resilience_ingress_backlog_active = self.config.resilience.enabled
            && (Self::frontier_consensus_ingress_queued(queue_depths)
                || queue_depths.consensus_rx > 0
                || queue_depths.lane_relay_rx > 0);
        let authoritative_frontier_rotation_candidate = contiguous_frontier
            && effective_has_reschedule_votes
            && !manifest_gate_pending
            && authoritative_payload_present
            && frontier_slot_present_for_view
            && !frontier_slot_owner_was_active
            && !stale_frontier_slot_owner_for_other_view;
        let rotate_authoritative_frontier_immediately =
            authoritative_frontier_rotation_candidate && !resilience_ingress_backlog_active;
        if authoritative_frontier_rotation_candidate && resilience_ingress_backlog_active {
            debug!(
                block = %block_hash,
                height,
                view,
                votes = vote_count,
                min_votes = min_votes_for_commit,
                vote_rx_depth = queue_depths.vote_rx,
                block_payload_rx_depth = queue_depths.block_payload_rx,
                rbc_chunk_rx_depth = queue_depths.rbc_chunk_rx,
                block_rx_depth = queue_depths.block_rx,
                consensus_rx_depth = queue_depths.consensus_rx,
                lane_relay_rx_depth = queue_depths.lane_relay_rx,
                "suppressing immediate payload-backed frontier rotation while consensus ingress drains"
            );
        }
        let frontier_window = self
            .frontier_recovery_window()
            .max(Duration::from_millis(1));
        let authoritative_payload_can_bypass_reassembly = authoritative_payload_present
            && queue_depths.block_payload_rx == 0
            && queue_depths.block_rx == 0
            && !self.frontier_recovery_same_height_rbc_sender_activity_active(height, now);
        let authoritative_payload_can_bypass_recovery_window =
            authoritative_payload_can_bypass_reassembly
                && !self.frontier_recovery.as_ref().is_some_and(|state| {
                    state.frontier_height == height && state.last_cause == "missing_payload"
                });
        let vote_backed_frontier_same_height_recovery_active = contiguous_frontier
            && effective_has_reschedule_votes
            && !drop_pending
            && !rotate_authoritative_frontier_immediately
            && !authoritative_payload_can_bypass_reassembly
            && self.frontier_recovery_same_slot_reassembly_active(height, view, now, queue_depths);
        let vote_backed_frontier_same_height_recovery_expired =
            vote_backed_frontier_same_height_recovery_active
                .then(|| {
                    self.vote_backed_frontier_reassembly_stall_expired(
                        height,
                        view,
                        quorum_stall_age,
                        quorum_timeout,
                        vote_count,
                        min_votes_for_commit,
                        now,
                    )
                })
                .flatten();
        let reduced_missing_payload_window = bundle_window_override.is_some()
            && !authoritative_payload_present
            && vote_count > 0
            && vote_count.saturating_add(1) >= min_votes_for_commit;
        let vote_backed_frontier_window_owned = contiguous_frontier
            && effective_has_reschedule_votes
            && !drop_pending
            && !rotate_authoritative_frontier_immediately
            && !authoritative_payload_can_bypass_recovery_window
            && !reduced_missing_payload_window
            && self.frontier_recovery_owns_height_window_with_window(
                height,
                now,
                bundle_window_override.unwrap_or(frontier_window),
            );
        if vote_backed_frontier_same_height_recovery_active
            && vote_backed_frontier_same_height_recovery_expired.is_none()
        {
            let recovery_cause = self
                .frontier_dependency_recovery_cause(height, view, now)
                .unwrap_or("quorum_timeout");
            let created_frontier_owner =
                self.seed_frontier_recovery_for_quorum_timeout(height, view, now);
            debug!(
                block = %block_hash,
                height,
                view,
                votes = vote_count,
                min_votes = min_votes_for_commit,
                pending_age_ms = pending_age.as_millis(),
                quorum_stall_age_ms = quorum_stall_age.as_millis(),
                block_payload_rx_depth = queue_depths.block_payload_rx,
                rbc_chunk_rx_depth = queue_depths.rbc_chunk_rx,
                block_rx_depth = queue_depths.block_rx,
                consensus_rx_depth = queue_depths.consensus_rx,
                lane_relay_rx_depth = queue_depths.lane_relay_rx,
                background_rx_depth = queue_depths.background_rx,
                frontier_recovery_cause = recovery_cause,
                created_frontier_owner,
                "suppressing vote-backed quorum reschedule; same-slot frontier recovery is still converging"
            );
            self.pending.pending_blocks.insert(block_hash, pending);
            return false;
        }
        if let Some((owner_stall_age, hard_cap)) = vote_backed_frontier_same_height_recovery_expired
        {
            debug!(
                block = %block_hash,
                height,
                view,
                votes = vote_count,
                min_votes = min_votes_for_commit,
                pending_age_ms = pending_age.as_millis(),
                quorum_stall_age_ms = quorum_stall_age.as_millis(),
                owner_stall_age_ms = owner_stall_age.as_millis(),
                hard_cap_ms = hard_cap.as_millis(),
                "allowing vote-backed quorum reschedule after same-slot frontier recovery stalled past hard cap"
            );
        }
        if vote_backed_frontier_window_owned {
            debug!(
                block = %block_hash,
                height,
                view,
                votes = vote_count,
                min_votes = min_votes_for_commit,
                pending_age_ms = pending_age.as_millis(),
                quorum_stall_age_ms = quorum_stall_age.as_millis(),
                "suppressing vote-backed quorum reschedule; frontier recovery already acted this window"
            );
            self.pending.pending_blocks.insert(block_hash, pending);
            return false;
        }
        let synthetic_body_progress_snapshot = if contiguous_frontier {
            let has_vote_backed_evidence =
                self.slot_has_vote_backed_consensus_evidence(height, view);
            self.frontier_slot.as_ref().and_then(|slot| {
                (slot.height == height
                    && slot.view == view
                    && slot.block_hash == block_hash
                    && slot.body_missing()
                    && slot.repair_state.last_reason == Some("quorum_timeout")
                    && (slot.quorum_progress.votes_observed
                        || slot.quorum_progress.commit_qc_observed
                        || matches!(slot.phase, FrontierSlotPhase::AwaitCommitQc)
                        || has_vote_backed_evidence))
                    .then_some((
                        slot.timers.last_progress_at,
                        slot.timers.lag_window_started_at,
                        slot.repair_state.quorum_timeout_rebroadcasted,
                    ))
            })
        } else {
            None
        };
        if contiguous_frontier {
            let _ = self.handle_frontier_slot_event(
                now,
                super::FrontierSlotEvent::OnBodyAvailable {
                    block_hash,
                    view,
                    sender: None,
                },
            );
            if let Some((last_progress_at, lag_window_started_at, quorum_timeout_rebroadcasted)) =
                synthetic_body_progress_snapshot
                && let Some(slot) = self.frontier_slot.as_mut()
                && slot.height == height
                && slot.view == view
                && slot.block_hash == block_hash
            {
                slot.timers.last_progress_at = last_progress_at;
                slot.timers.lag_window_started_at = lag_window_started_at;
                slot.repair_state.quorum_timeout_rebroadcasted = quorum_timeout_rebroadcasted;
            }
        }
        let rotate_zero_vote_frontier_immediately = contiguous_frontier && drop_pending;
        let handoff_frontier_quorum_timeout_owner = contiguous_frontier
            && effective_has_reschedule_votes
            && !drop_pending
            && !rotate_authoritative_frontier_immediately;
        let handoff_zero_vote_frontier_owner = contiguous_frontier
            && drop_pending
            && !keep_commit_qc
            && !frontier_slot_owner_was_active
            && self.seed_frontier_slot_from_same_height_evidence(
                height,
                view,
                now,
                "quorum_timeout",
                false,
            );
        if handoff_zero_vote_frontier_owner {
            debug!(
                block = %block_hash,
                height,
                view,
                pending_age_ms = pending_age.as_millis(),
                quorum_stall_age_ms = quorum_stall_age.as_millis(),
                "preserving zero-vote contiguous frontier pending block because same-height QC/vote evidence handed ownership to the slot tracker"
            );
            self.pending.pending_blocks.insert(block_hash, pending);
            return false;
        }
        let (requeued, failures, _duplicate_failures, _gossip_hashes) =
            if !effective_has_reschedule_votes || drop_pending {
                // Avoid conflicting proposals once votes exist (precommit or commit), unless we've
                // already retried with availability evidence and need to unblock proposal assembly.
                let txs: Vec<_> = pending.block.external_entrypoints_cloned().collect();
                requeue_block_transactions(self.queue.as_ref(), self.state.as_ref(), txs)
            } else {
                (0, 0, 0, Vec::new())
            };
        if !drop_pending
            && requeued == 0
            && self
                .quorum_retransmit_targets_for_missing_votes(
                    block_hash,
                    height,
                    view,
                    &topology_peers,
                    min_votes_for_commit,
                    reschedule_vote_count,
                )
                .is_empty()
        {
            let rotate_stake_quorum_noop_immediately =
                contiguous_frontier && stake_quorum_missing && effective_has_reschedule_votes;
            self.pending.pending_blocks.insert(block_hash, pending);
            if handoff_frontier_quorum_timeout_owner {
                let created_frontier_owner =
                    self.seed_frontier_recovery_for_quorum_timeout(height, view, now);
                debug!(
                    block = %block_hash,
                    height,
                    view,
                    votes = vote_count,
                    min_votes = min_votes_for_commit,
                    pending_age_ms = pending_age.as_millis(),
                    quorum_stall_age_ms = quorum_stall_age.as_millis(),
                    created_frontier_owner,
                    "skipping no-op commit-quorum reschedule: preserved contiguous-frontier quorum-timeout recovery ownership"
                );
            }
            debug!(
                block = %block_hash,
                height,
                view,
                votes = vote_count,
                min_votes = min_votes_for_commit,
                stake_quorum_missing,
                pending_age_ms = pending_age.as_millis(),
                quorum_stall_age_ms = quorum_stall_age.as_millis(),
                "skipping no-op commit-quorum reschedule: no actionable retransmit targets remain"
            );
            if rotate_authoritative_frontier_immediately
                || rotate_zero_vote_frontier_immediately
                || rotate_stake_quorum_noop_immediately
            {
                info!(
                    block = %block_hash,
                    height,
                    view,
                    votes = vote_count,
                    min_votes = min_votes_for_commit,
                    pending_age_ms = pending_age.as_millis(),
                    quorum_stall_age_ms = quorum_stall_age.as_millis(),
                    drop_pending,
                    stake_quorum_missing,
                    "no actionable quorum retransmit targets remain for contiguous frontier block; rotating view deterministically"
                );
                if rotate_authoritative_frontier_immediately {
                    self.advance_view_after_completed_quorum_reschedule(
                        height,
                        view,
                        direct_view_change_cause,
                        now,
                    );
                } else if rotate_stake_quorum_noop_immediately {
                    self.apply_view_change_with_cause_for_height(
                        height,
                        view,
                        direct_view_change_cause,
                    );
                } else {
                    self.trigger_view_change_with_cause(height, view, direct_view_change_cause);
                }
                return true;
            }
            return false;
        }
        let commit_quorum_bundle_window =
            bundle_window_override.unwrap_or_else(|| self.round_recovery_bundle_window());
        if !self.try_reserve_round_recovery_bundle_window_with_window(
            height,
            super::RoundRecoveryBundleSource::CommitQuorumReschedule,
            commit_quorum_bundle_window,
            now,
        ) {
            debug!(
                block = %block_hash,
                height,
                view,
                pending_age_ms = pending_age.as_millis(),
                quorum_stall_age_ms = quorum_stall_age.as_millis(),
                resend_window_ms = commit_quorum_bundle_window.as_millis(),
                "suppressing repeated commit-quorum reschedule in current deterministic recovery bundle window"
            );
            self.pending.pending_blocks.insert(block_hash, pending);
            if rotate_zero_vote_frontier_immediately {
                info!(
                    block = %block_hash,
                    height,
                    view,
                    votes = vote_count,
                    min_votes = min_votes_for_commit,
                    pending_age_ms = pending_age.as_millis(),
                    quorum_stall_age_ms = quorum_stall_age.as_millis(),
                    drop_pending,
                    "contiguous frontier quorum retransmit window was pacing-limited; rotating view deterministically"
                );
                self.trigger_view_change_with_cause(height, view, direct_view_change_cause);
                return true;
            }
            return false;
        }
        let rebroadcast = self.rebroadcast_pending_block_updates(
            &mut pending,
            block_hash,
            height,
            view,
            drop_pending,
            &topology_peers,
            min_votes_for_commit,
            reschedule_vote_count,
            contiguous_frontier
                && effective_has_reschedule_votes
                && reschedule_vote_count < min_votes_for_commit,
            now,
        );
        let action_taken = drop_pending
            || requeued > 0
            || manifest_gate_pending
            || emitted_local_vote
            || rebroadcast.local_vote
            || rebroadcast.votes > 0
            || rebroadcast.block_sync
            || rebroadcast.block
            || rebroadcast.missing_block_fetch;
        if !action_taken {
            self.pending.pending_blocks.insert(block_hash, pending);
            if handoff_frontier_quorum_timeout_owner {
                let created_frontier_owner =
                    self.seed_frontier_recovery_for_quorum_timeout(height, view, now);
                debug!(
                    block = %block_hash,
                    height,
                    view,
                    votes = vote_count,
                    min_votes = min_votes_for_commit,
                    pending_age_ms = pending_age.as_millis(),
                    quorum_stall_age_ms = quorum_stall_age.as_millis(),
                    created_frontier_owner,
                    "skipping no-op commit-quorum reschedule: preserved contiguous-frontier quorum-timeout recovery ownership after pacing"
                );
            }
            debug!(
                block = %block_hash,
                height,
                view,
                votes = vote_count,
                min_votes = min_votes_for_commit,
                pending_age_ms = pending_age.as_millis(),
                quorum_stall_age_ms = quorum_stall_age.as_millis(),
                "skipping no-op commit-quorum reschedule after pacing/cooldown suppressed all retransmit work"
            );
            if rotate_zero_vote_frontier_immediately {
                info!(
                    block = %block_hash,
                    height,
                    view,
                    votes = vote_count,
                    min_votes = min_votes_for_commit,
                    pending_age_ms = pending_age.as_millis(),
                    quorum_stall_age_ms = quorum_stall_age.as_millis(),
                    "zero-vote contiguous frontier quorum timeout had no retransmit work; rotating view deterministically"
                );
                self.trigger_view_change_with_cause(height, view, direct_view_change_cause);
                return true;
            }
            return false;
        }
        let mut recorded_vote_count = self
            .pending_block_commit_votes_count(block_hash, height, view)
            .max(reschedule_vote_count);
        if recorded_vote_count == 0 && pending.local_commit_vote_emitted() {
            recorded_vote_count = 1;
        }
        if recorded_vote_count > 0 {
            pending.mark_vote_backed_quorum_reschedule(now, recorded_vote_count);
        } else {
            pending.mark_quorum_reschedule(now);
        }

        if drop_pending {
            if !keep_commit_qc {
                self.clean_rbc_sessions_for_block(block_hash, height);
            }
            self.qc_cache.retain(|(phase, qc_hash, _, _, _, _, _), _| {
                *qc_hash != block_hash
                    || (keep_commit_qc
                        && matches!(phase, crate::sumeragi::consensus::Phase::Commit))
            });
            self.qc_signer_tally
                .retain(|(phase, qc_hash, _, _, _, _, _), _| {
                    *qc_hash != block_hash
                        || (keep_commit_qc
                            && matches!(phase, crate::sumeragi::consensus::Phase::Commit))
                });
            self.pending.pending_fetch_requests.remove(&block_hash);
            self.pending.pending_block_body_requests.remove(&block_hash);
            self.subsystems.validation.inflight.remove(&block_hash);
            self.subsystems
                .validation
                .superseded_results
                .remove(&block_hash);
        } else {
            // Keep the pending block and cached certificates so late commit certificates
            // can still finalize it. Do not refresh frontier progress here: votes/RBC already
            // own progress, and quorum reschedule must stay a bounded retransmit side effect.
            self.pending.pending_blocks.insert(block_hash, pending);
            if manifest_gate_pending {
                if let Some(stored) = self.pending.pending_blocks.get_mut(&block_hash) {
                    stored.mark_quorum_reschedule(now);
                }
            }
        }
        let isolated_frontier_anchor_handoff = if handoff_frontier_quorum_timeout_owner {
            self.maybe_handoff_isolated_vote_backed_frontier_to_anchor(
                block_hash,
                height,
                view,
                reschedule_vote_count,
                min_votes_for_commit,
                now,
            )
        } else {
            false
        };
        let frontier_recovery_advance = if handoff_frontier_quorum_timeout_owner {
            let _ = self.seed_frontier_recovery_for_quorum_timeout(height, view, now);
            Some(self.advance_frontier_recovery(
                "quorum_timeout",
                height,
                view,
                false,
                false,
                true,
                now,
            ))
        } else {
            None
        };

        let queue_depths = super::status::worker_queue_depth_snapshot();
        warn!(
            ?block_hash,
            height,
            view,
            pending_age_ms = pending_age.as_millis(),
            quorum_stall_age_ms = quorum_stall_age.as_millis(),
            progress_age_ms = progress_age.as_millis(),
            quorum_timeout_ms = quorum_timeout.as_millis(),
            votes = vote_count,
            min_votes = min_votes_for_commit,
            requeued,
            failures,
            vote_rx_depth = queue_depths.vote_rx,
            block_payload_rx_depth = queue_depths.block_payload_rx,
            rbc_chunk_rx_depth = queue_depths.rbc_chunk_rx,
            block_rx_depth = queue_depths.block_rx,
            consensus_rx_depth = queue_depths.consensus_rx,
            lane_relay_rx_depth = queue_depths.lane_relay_rx,
            background_rx_depth = queue_depths.background_rx,
            rebroadcasted_votes = rebroadcast.votes,
            rebroadcasted_block = rebroadcast.block,
            rebroadcasted_block_sync = rebroadcast.block_sync,
            requested_missing_block_fetch = rebroadcast.missing_block_fetch,
            drop_pending,
            same_slot_vote_backed_evidence,
            frontier_slot_owner_active = frontier_slot_owner_was_active,
            effective_has_reschedule_votes,
            handoff_frontier_quorum_timeout_owner,
            isolated_frontier_anchor_handoff,
            frontier_recovery_advance = ?frontier_recovery_advance,
            precommit_votes = precommit_vote_count,
            commit_votes = commit_vote_count,
            reschedule_backoff_ms = reschedule_backoff.as_millis(),
            last_reschedule_ms = last_reschedule_ms,
            rotate_zero_vote_immediately = rotate_zero_vote_frontier_immediately,
            rotate_immediately = rotate_authoritative_frontier_immediately,
            "commit quorum missing past timeout; rescheduling block for reassembly"
        );
        if rotate_authoritative_frontier_immediately {
            info!(
                block = %block_hash,
                height,
                view,
                votes = vote_count,
                min_votes = min_votes_for_commit,
                pending_age_ms = pending_age.as_millis(),
                quorum_stall_age_ms = quorum_stall_age.as_millis(),
                "payload-backed contiguous frontier quorum timeout completed recovery work; rotating view deterministically"
            );
            self.advance_view_after_completed_quorum_reschedule(
                height,
                view,
                direct_view_change_cause,
                now,
            );
        } else if rotate_zero_vote_frontier_immediately {
            self.trigger_view_change_with_cause(height, view, direct_view_change_cause);
        }
        true
    }

    fn paced_retransmit_targets(
        &self,
        targets: Vec<PeerId>,
        height: u64,
        view: u64,
        limit: usize,
    ) -> Vec<PeerId> {
        paced_retransmit_targets(targets, height, view, limit)
    }

    fn retransmit_backlog_pacing(&self, target_count: usize) -> (usize, Duration, u8) {
        let tx_queue = super::status::tx_queue_backpressure();
        let (_, rbc_store_bytes, rbc_pressure_level) = super::status::rbc_store_pressure();
        let pressure_score = retransmit_pressure_score(
            tx_queue.depth,
            tx_queue.capacity,
            tx_queue.saturated || tx_queue.saturated_by_age,
            rbc_store_bytes,
            rbc_pressure_level,
        );
        let limit = retransmit_target_limit(target_count, pressure_score);
        let cooldown = super::saturating_mul_duration(
            self.rebroadcast_cooldown(),
            retransmit_cooldown_multiplier(pressure_score),
        );
        (limit, cooldown, pressure_score)
    }

    fn broadcast_vote_backed_block_sync_update(
        &mut self,
        block: &SignedBlock,
        targets: &[PeerId],
        trigger: &'static str,
    ) -> bool {
        if targets.is_empty() {
            return false;
        }

        let block_hash = block.hash();
        let height = block.header().height().get();
        let view = block.header().view_change_index();
        let msg = self.build_fetch_pending_block_payload(block);
        let BlockMessage::BlockSyncUpdate(_) = msg else {
            return false;
        };

        let encoded_len = super::consensus_block_wire_len(self.common_config.peer.id(), &msg);
        if encoded_len > self.consensus_payload_frame_cap {
            warn!(
                height,
                view,
                block = %block_hash,
                encoded_len,
                cap = self.consensus_payload_frame_cap,
                trigger,
                "skipping vote-backed BlockSyncUpdate rebroadcast: frame cap exceeded"
            );
            return false;
        }

        let msg = Arc::new(msg);
        let encoded = Arc::new(BlockMessageWire::encode_message(msg.as_ref()));
        let local_peer = self.common_config.peer.id().clone();
        let mut sent = false;
        for peer in targets {
            if peer == &local_peer {
                continue;
            }
            self.schedule_background(BackgroundRequest::Post {
                peer: peer.clone(),
                msg: BlockMessageWire::with_encoded(Arc::clone(&msg), Arc::clone(&encoded)),
            });
            sent = true;
        }
        if sent {
            debug!(
                height,
                view,
                block = %block_hash,
                targets = targets.len(),
                trigger,
                "rebroadcasting vote-backed BlockSyncUpdate for quorum recovery"
            );
        }
        sent
    }

    fn vote_backed_block_sync_update_shape(&self, block: &SignedBlock) -> (bool, bool) {
        let msg = self.build_fetch_pending_block_payload(block);
        let BlockMessage::BlockSyncUpdate(_) = msg else {
            return (false, false);
        };
        let encoded_len = super::consensus_block_wire_len(self.common_config.peer.id(), &msg);
        (true, encoded_len <= self.consensus_payload_frame_cap)
    }

    #[allow(clippy::too_many_arguments)]
    fn rebroadcast_pending_block_updates(
        &mut self,
        pending: &mut PendingBlock,
        block_hash: HashOf<BlockHeader>,
        height: u64,
        view: u64,
        drop_pending: bool,
        topology_peers: &[PeerId],
        min_votes_for_commit: usize,
        vote_count: usize,
        widen_repair_fanout: bool,
        now: Instant,
    ) -> RescheduleRebroadcast {
        let local_vote = if drop_pending || topology_peers.is_empty() {
            false
        } else {
            self.with_registered_pending_block(block_hash, pending, |actor| {
                actor.maybe_emit_local_commit_vote_for_pending_event(
                    block_hash,
                    height,
                    view,
                    topology_peers,
                    "quorum_reschedule",
                )
            })
        };
        let current_vote_count = quorum_rebroadcast_observed_vote_count(
            self.pending_block_commit_votes_count(block_hash, height, view),
            pending.local_commit_vote_emitted(),
            vote_count,
        );
        let observed_vote_backing = current_vote_count > 0;
        if self.relay_backpressure_active(now, self.rebroadcast_cooldown()) {
            super::status::inc_retransmit_skip_relay_backpressure();
            debug!(
                height,
                view,
                block = %block_hash,
                "skipping reschedule rebroadcast due to relay backpressure"
            );
            return RescheduleRebroadcast {
                local_vote,
                votes: 0,
                block_sync: false,
                block: false,
                missing_block_fetch: false,
            };
        }
        let mut retransmit_targets = self.quorum_retransmit_targets_for_missing_votes(
            block_hash,
            height,
            view,
            topology_peers,
            min_votes_for_commit,
            current_vote_count,
        );
        let force_full_repair_fanout = quorum_rebroadcast_force_full_repair_fanout(
            widen_repair_fanout,
            drop_pending,
            observed_vote_backing,
            current_vote_count,
            min_votes_for_commit,
        );
        if force_full_repair_fanout {
            let local_peer_id = self.common_config.peer.id();
            let all_non_local_targets: Vec<_> = topology_peers
                .iter()
                .filter(|peer| *peer != local_peer_id)
                .cloned()
                .collect();
            if all_non_local_targets.len() > retransmit_targets.len() {
                // Exact near-quorum stalls are the targeted-load danger case: a single
                // saturated or mis-inferred target set can pin finality one vote short.
                // This remains volatile repair traffic only; it does not alter validator
                // ordering, vote validity, or deterministic state.
                retransmit_targets = all_non_local_targets;
            }
        }
        if retransmit_targets.is_empty() {
            super::status::inc_retransmit_skip_no_targets();
            debug!(
                height,
                view,
                block = %block_hash,
                "skipping reschedule rebroadcast because no peers are missing votes"
            );
            return RescheduleRebroadcast {
                local_vote,
                votes: 0,
                block_sync: false,
                block: false,
                missing_block_fetch: false,
            };
        }

        let (target_limit, adaptive_cooldown, pressure_score) =
            self.retransmit_backlog_pacing(retransmit_targets.len());
        if !force_full_repair_fanout && !pending.precommit_rebroadcast_due(now, adaptive_cooldown) {
            super::status::inc_retransmit_skip_cooldown();
            debug!(
                height,
                view,
                block = %block_hash,
                pressure_score,
                cooldown_ms = adaptive_cooldown.as_millis(),
                "skipping reschedule rebroadcast due to adaptive cooldown"
            );
            return RescheduleRebroadcast {
                local_vote,
                votes: 0,
                block_sync: false,
                block: false,
                missing_block_fetch: false,
            };
        }

        if !force_full_repair_fanout && target_limit == 0 {
            super::status::inc_retransmit_skip_backlog_pacing();
            debug!(
                height,
                view,
                block = %block_hash,
                pressure_score,
                "skipping reschedule rebroadcast due to backlog pacing"
            );
            return RescheduleRebroadcast {
                local_vote,
                votes: 0,
                block_sync: false,
                block: false,
                missing_block_fetch: false,
            };
        }

        let retransmit_targets = if force_full_repair_fanout {
            let mut targets = retransmit_targets;
            targets.sort();
            targets.dedup();
            debug!(
                height,
                view,
                block = %block_hash,
                targets = targets.len(),
                pressure_score,
                votes = current_vote_count,
                min_votes = min_votes_for_commit,
                "widening vote-backed quorum repair fanout"
            );
            targets
        } else {
            self.paced_retransmit_targets(retransmit_targets, height, view, target_limit)
        };
        if retransmit_targets.is_empty() {
            super::status::inc_retransmit_skip_backlog_pacing();
            return RescheduleRebroadcast {
                local_vote,
                votes: 0,
                block_sync: false,
                block: false,
                missing_block_fetch: false,
            };
        }
        super::status::record_retransmit_target_set_size(retransmit_targets.len());

        let votes = self.rebroadcast_block_votes_to_targets(
            crate::sumeragi::consensus::Phase::Commit,
            block_hash,
            height,
            view,
            &retransmit_targets,
        );
        let mut block_sync = false;
        let mut missing_block_fetch = false;
        if !drop_pending && !retransmit_targets.is_empty() {
            let has_cached_commit_qc = self
                .cached_commit_qc_for_block(block_hash, height, view)
                .is_some();
            block_sync = self.with_registered_pending_block(block_hash, pending, |actor| {
                actor.maybe_replay_known_block_commit_evidence(
                    block_hash,
                    height,
                    view,
                    &retransmit_targets,
                    "quorum_reschedule",
                )
            });
            if quorum_rebroadcast_should_request_missing_commit_qc(
                drop_pending,
                retransmit_targets.len(),
                has_cached_commit_qc,
                observed_vote_backing,
            ) {
                missing_block_fetch = self.maybe_request_known_block_commit_qc_recovery(
                    block_hash,
                    height,
                    view,
                    &retransmit_targets,
                    Some(pending),
                    "quorum_reschedule",
                );
            }
        }
        let mut block = false;
        if quorum_rebroadcast_should_broadcast_block_created(
            drop_pending,
            retransmit_targets.len(),
            observed_vote_backing,
        ) {
            let contiguous_frontier = height == self.committed_height_snapshot().saturating_add(1)
                && pending_extends_tip(
                    height,
                    pending.block.header().prev_block_hash(),
                    self.state.committed_height(),
                    self.state.latest_block_hash_fast(),
                );
            let non_local_target_count = retransmit_targets
                .iter()
                .filter(|peer| *peer != self.common_config.peer.id())
                .count();
            let (block_sync_update_available, block_sync_update_fits_frame) =
                self.vote_backed_block_sync_update_shape(&pending.block);
            if quorum_rebroadcast_should_broadcast_vote_backed_block_sync(
                drop_pending,
                retransmit_targets.len(),
                non_local_target_count,
                observed_vote_backing,
                contiguous_frontier,
                min_votes_for_commit,
                current_vote_count,
                block_sync_update_available,
                block_sync_update_fits_frame,
            ) {
                block_sync |= self.broadcast_vote_backed_block_sync_update(
                    &pending.block,
                    &retransmit_targets,
                    "quorum_reschedule",
                );
            }
            let created = self.frontier_block_created_for_wire(&pending.block);
            self.broadcast_block_created(created, &retransmit_targets);
            block = true;
        }
        if quorum_rebroadcast_should_mark_precommit(
            local_vote,
            votes,
            block_sync,
            block,
            missing_block_fetch,
        ) {
            pending.mark_precommit_rebroadcast(now);
        }
        RescheduleRebroadcast {
            local_vote,
            votes,
            block_sync,
            block,
            missing_block_fetch,
        }
    }

    fn with_registered_pending_block<T>(
        &mut self,
        block_hash: HashOf<BlockHeader>,
        pending: &mut PendingBlock,
        f: impl FnOnce(&mut Self) -> T,
    ) -> T {
        let placeholder = PendingBlock::new(
            pending.block.clone(),
            pending.payload_hash,
            pending.height,
            pending.view,
        );
        let detached = std::mem::replace(pending, placeholder);
        let replaced = self.pending.pending_blocks.insert(block_hash, detached);
        let result = f(self);
        let restored = self.pending.pending_blocks.remove(&block_hash);
        if let Some(previous) = replaced {
            self.pending.pending_blocks.insert(block_hash, previous);
        }
        if let Some(restored) = restored {
            *pending = restored;
        }
        result
    }

    pub(super) fn quorum_retransmit_targets_for_missing_votes(
        &self,
        block_hash: HashOf<BlockHeader>,
        height: u64,
        view: u64,
        topology_peers: &[PeerId],
        min_votes_for_commit: usize,
        vote_count: usize,
    ) -> Vec<PeerId> {
        if topology_peers.is_empty() {
            return Vec::new();
        }
        let local_peer_id = self.common_config.peer.id();
        let observed_signers: std::collections::BTreeSet<
            crate::sumeragi::consensus::ValidatorIndex,
        > = self
            .vote_log
            .values()
            .filter(|vote| {
                vote.phase == crate::sumeragi::consensus::Phase::Commit
                    && vote.block_hash == block_hash
                    && vote.height == height
                    && vote.view == view
            })
            .filter_map(|vote| {
                crate::sumeragi::consensus::ValidatorIndex::try_from(vote.signer).ok()
            })
            .collect();
        let canonical_topology = super::network_topology::Topology::new(topology_peers.to_vec());
        let (consensus_mode, mode_tag, prf_seed) = self.consensus_context_for_height(height);
        let signature_topology =
            super::topology_for_view(&canonical_topology, height, view, mode_tag, prf_seed);
        let observed_signer_peers = match super::signer_peers_for_topology(
            &observed_signers,
            &signature_topology,
        ) {
            Ok(peers) => Some(peers),
            Err(err) => {
                debug!(
                    height,
                    view,
                    block = %block_hash,
                    ?err,
                    "failed to map observed vote signers for retransmit target selection; falling back to full fanout"
                );
                None
            }
        };

        let targets = quorum_retransmit_targets_from_observed_peers(
            topology_peers,
            local_peer_id,
            observed_signer_peers.as_ref(),
            min_votes_for_commit,
            vote_count,
        );
        let near_commit_quorum =
            quorum_retransmit_near_commit_quorum(min_votes_for_commit, vote_count);
        let all_non_local_targets: Vec<PeerId> = topology_peers
            .iter()
            .filter(|peer| *peer != local_peer_id)
            .cloned()
            .collect();
        let selected_target_set: std::collections::BTreeSet<_> = targets.iter().cloned().collect();
        let full_fanout_target_set: std::collections::BTreeSet<_> =
            all_non_local_targets.iter().cloned().collect();
        let selected_reaches_stake_quorum =
            crate::sumeragi::stake_snapshot::stake_quorum_reached_for_world(
                self.state.view().world(),
                topology_peers,
                &selected_target_set,
            )
            .unwrap_or(false);
        let full_fanout_reaches_stake_quorum =
            crate::sumeragi::stake_snapshot::stake_quorum_reached_for_world(
                self.state.view().world(),
                topology_peers,
                &full_fanout_target_set,
            )
            .unwrap_or(false);
        if near_commit_quorum && !all_non_local_targets.is_empty() {
            // Near quorum, peers can hold overlapping vote subsets. Fan out to every remote peer
            // so observed voters can merge partial sets instead of only targeting inferred gaps.
            if matches!(consensus_mode, ConsensusMode::Npos) {
                self.record_npos_repair_coverage(
                    height,
                    view,
                    "near_commit_quorum_full_fanout",
                    topology_peers,
                    &all_non_local_targets,
                );
            }
            return targets;
        }
        if matches!(consensus_mode, ConsensusMode::Npos)
            && vote_count < min_votes_for_commit
            && !targets.is_empty()
            && !selected_reaches_stake_quorum
            && full_fanout_reaches_stake_quorum
        {
            self.record_npos_repair_coverage(
                height,
                view,
                "insufficient_target_stake_full_fanout",
                topology_peers,
                &all_non_local_targets,
            );
            return all_non_local_targets;
        }
        if matches!(consensus_mode, ConsensusMode::Npos) {
            self.record_npos_repair_coverage(
                height,
                view,
                "missing_commit_votes",
                topology_peers,
                &targets,
            );
        }
        targets
    }

    fn record_npos_repair_coverage(
        &self,
        height: u64,
        view: u64,
        reason: &str,
        topology_peers: &[PeerId],
        selected_targets: &[PeerId],
    ) {
        let selected: std::collections::BTreeSet<_> = selected_targets.iter().cloned().collect();
        let selected_bps = crate::sumeragi::stake_snapshot::stake_coverage_bps_for_world(
            self.state.view().world(),
            topology_peers,
            &selected,
        )
        .unwrap_or(0);
        let reached = crate::sumeragi::stake_snapshot::stake_quorum_reached_for_world(
            self.state.view().world(),
            topology_peers,
            &selected,
        )
        .unwrap_or(false);
        super::status::record_npos_repair_coverage(
            height,
            view,
            reason,
            selected_targets.len(),
            6_667,
            selected_bps,
            reached,
        );
    }
}

#[derive(Clone, Copy, Debug)]
struct RescheduleRebroadcast {
    local_vote: bool,
    votes: usize,
    block_sync: bool,
    block: bool,
    missing_block_fetch: bool,
}

#[cfg(test)]
mod tests {
    use super::{
        CompletedQuorumViewAdvanceRoute, NEAR_QUORUM_PREEMPTIVE_RECOVERY_PER_TICK,
        PreemptiveVoteBackedRetransmitTargetSource, RETRANSMIT_RBC_BYTES_HARD,
        RETRANSMIT_RBC_BYTES_SOFT, RbcAvailabilityRescheduleSession,
        VoteBackedReassemblyRecoveryOwnerState, VoteBackedReassemblySlotOwnerState,
        adaptive_quorum_reschedule_backoff, completed_quorum_view_advance_route,
        consensus_ingress_reschedule_backoff, contiguous_frontier_vote_backed_fast_resend_window,
        contiguous_frontier_vote_backed_resend_window, isolated_vote_backed_handoff_action,
        isolated_vote_backed_handoff_admission, isolated_vote_backed_handoff_reason_ok,
        isolated_vote_backed_handoff_requests_anchor, isolated_vote_backed_handoff_slot_valid,
        near_quorum_fresh_missing_block_request_suppresses,
        near_quorum_inflight_recovery_suppresses, near_quorum_payload_timeout,
        paced_retransmit_rotation_offset_with_limit, paced_retransmit_targets,
        preemptive_vote_backed_retransmit_action, preemptive_vote_backed_retransmit_candidate,
        preemptive_vote_backed_retransmit_target_source,
        preemptive_vote_backed_retransmit_widen_fanout,
        quorum_rebroadcast_force_full_repair_fanout, quorum_rebroadcast_observed_vote_count,
        quorum_rebroadcast_should_broadcast_block_created,
        quorum_rebroadcast_should_broadcast_vote_backed_block_sync,
        quorum_rebroadcast_should_mark_precommit,
        quorum_rebroadcast_should_request_missing_commit_qc,
        rbc_availability_unresolved_for_reschedule_decision, retransmit_cooldown_multiplier,
        retransmit_pressure_score, retransmit_target_limit,
        vote_backed_frontier_reassembly_hard_cap_from_windows,
        vote_backed_frontier_reassembly_owner_stall_age_from_sources,
        vote_backed_frontier_reassembly_stall_expiry,
    };
    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_data_model::peer::PeerId;
    use std::time::{Duration, Instant};

    fn paced_retransmit_formal_peer_ids() -> Vec<PeerId> {
        let mut peers = (1..=4)
            .map(|idx| {
                PeerId::new(
                    KeyPair::try_from_seed(
                        format!("paced-retransmit-targets-{idx}").into_bytes(),
                        Algorithm::BlsNormal,
                    )
                    .expect("fixture seed must derive a valid BLS keypair")
                    .public_key()
                    .clone(),
                )
            })
            .collect::<Vec<_>>();
        peers.sort();
        peers
    }

    #[test]
    fn paced_retransmit_formal_peer_ids_use_checked_seed_derivation() {
        assert!(
            KeyPair::try_from_seed(vec![0; 32], Algorithm::BlsNormal).is_err(),
            "checked BLS seed derivation must reject weak all-zero fixture seeds"
        );
        let peers = paced_retransmit_formal_peer_ids();
        assert_eq!(peers.len(), 4);
        assert!(peers.windows(2).all(|pair| pair[0] < pair[1]));
    }

    fn paced_retransmit_formal_targets(peers: &[PeerId], labels: &[usize]) -> Vec<PeerId> {
        labels
            .iter()
            .map(|label| peers[label - 1].clone())
            .collect()
    }

    #[test]
    fn paced_retransmit_targets_formal_gate_matrix() {
        struct Case {
            targets: &'static [usize],
            height: u64,
            view: u64,
            limit: usize,
            expected: &'static [usize],
        }

        let peers = paced_retransmit_formal_peer_ids();
        for case in [
            Case {
                targets: &[1],
                height: 0,
                view: 0,
                limit: 0,
                expected: &[],
            },
            Case {
                targets: &[],
                height: 0,
                view: 0,
                limit: 2,
                expected: &[],
            },
            Case {
                targets: &[3, 1],
                height: 0,
                view: 0,
                limit: 3,
                expected: &[3, 1],
            },
            Case {
                targets: &[2, 2, 1],
                height: 0,
                view: 0,
                limit: 3,
                expected: &[2, 2, 1],
            },
            Case {
                targets: &[2, 1, 1],
                height: 0,
                view: 0,
                limit: 2,
                expected: &[1, 2],
            },
            Case {
                targets: &[3, 1, 2],
                height: 0,
                view: 0,
                limit: 2,
                expected: &[1, 2],
            },
            Case {
                targets: &[1, 2, 3],
                height: 0,
                view: 2,
                limit: 2,
                expected: &[2, 3],
            },
            Case {
                targets: &[1, 2, 3, 4],
                height: 0,
                view: 3_u64 << 59,
                limit: 2,
                expected: &[4, 1],
            },
            Case {
                targets: &[1, 2, 3, 4],
                height: 0,
                view: (2_u64 << 59) | 4,
                limit: 2,
                expected: &[3, 4],
            },
            Case {
                targets: &[1, 2, 3, 4],
                height: 0,
                view: 2_u64 << 59,
                limit: 1,
                expected: &[3],
            },
            Case {
                targets: &[1, 2, 3, 4],
                height: 2_u64 << 47,
                view: 0,
                limit: 2,
                expected: &[3, 4],
            },
            Case {
                targets: &[1, 2, 3],
                height: 0,
                view: 2,
                limit: 2,
                expected: &[2, 3],
            },
            Case {
                targets: &[2, 1, 2],
                height: 0,
                view: 0,
                limit: 2,
                expected: &[1, 2],
            },
        ] {
            let targets = paced_retransmit_formal_targets(&peers, case.targets);
            let expected = paced_retransmit_formal_targets(&peers, case.expected);

            assert_eq!(
                paced_retransmit_targets(targets, case.height, case.view, case.limit),
                expected
            );
        }
    }

    #[test]
    fn paced_retransmit_rotation_offset_fails_closed_when_offset_is_unrepresentable() {
        assert_eq!(
            paced_retransmit_rotation_offset_with_limit(0, 0, 0, u64::MAX),
            None,
            "empty target sets have no rotation offset"
        );
        assert_eq!(
            paced_retransmit_rotation_offset_with_limit(10, 0, 2, 3),
            None,
            "rotation offsets above the platform limit must not panic"
        );
        assert_eq!(
            paced_retransmit_rotation_offset_with_limit(10, 0, 2, 4),
            Some(4),
            "representable offsets remain deterministic"
        );
    }

    #[test]
    fn quorum_reschedule_backoff_formal_gate_matrix() {
        struct Case {
            name: &'static str,
            base_backoff_ms: u64,
            quorum_timeout_ms: u64,
            stall_age_ms: u64,
            vote_count: usize,
            min_votes_for_commit: usize,
            rebroadcast_cooldown_ms: u64,
            contiguous_frontier: bool,
            relay_backpressure: bool,
            vote_queue_backlog: bool,
            rbc_unresolved: bool,
            expected_backoff_ms: u64,
            expected_escalated: bool,
            expected_fast_window_ms: Option<u64>,
        }

        for case in [
            Case {
                name: "base_zero",
                base_backoff_ms: 0,
                quorum_timeout_ms: 50,
                stall_age_ms: 0,
                vote_count: 0,
                min_votes_for_commit: 3,
                rebroadcast_cooldown_ms: 25,
                contiguous_frontier: true,
                relay_backpressure: false,
                vote_queue_backlog: false,
                rbc_unresolved: false,
                expected_backoff_ms: 0,
                expected_escalated: false,
                expected_fast_window_ms: None,
            },
            Case {
                name: "no_votes_no_stall",
                base_backoff_ms: 100,
                quorum_timeout_ms: 50,
                stall_age_ms: 0,
                vote_count: 0,
                min_votes_for_commit: 3,
                rebroadcast_cooldown_ms: 25,
                contiguous_frontier: true,
                relay_backpressure: false,
                vote_queue_backlog: false,
                rbc_unresolved: false,
                expected_backoff_ms: 300,
                expected_escalated: false,
                expected_fast_window_ms: None,
            },
            Case {
                name: "one_missing_no_stall",
                base_backoff_ms: 100,
                quorum_timeout_ms: 50,
                stall_age_ms: 0,
                vote_count: 2,
                min_votes_for_commit: 3,
                rebroadcast_cooldown_ms: 25,
                contiguous_frontier: true,
                relay_backpressure: false,
                vote_queue_backlog: false,
                rbc_unresolved: false,
                expected_backoff_ms: 200,
                expected_escalated: false,
                expected_fast_window_ms: Some(25),
            },
            Case {
                name: "at_quorum_no_stall",
                base_backoff_ms: 100,
                quorum_timeout_ms: 50,
                stall_age_ms: 0,
                vote_count: 3,
                min_votes_for_commit: 3,
                rebroadcast_cooldown_ms: 25,
                contiguous_frontier: true,
                relay_backpressure: false,
                vote_queue_backlog: false,
                rbc_unresolved: false,
                expected_backoff_ms: 100,
                expected_escalated: false,
                expected_fast_window_ms: None,
            },
            Case {
                name: "over_quorum_no_stall",
                base_backoff_ms: 100,
                quorum_timeout_ms: 50,
                stall_age_ms: 0,
                vote_count: 5,
                min_votes_for_commit: 3,
                rebroadcast_cooldown_ms: 25,
                contiguous_frontier: true,
                relay_backpressure: false,
                vote_queue_backlog: false,
                rbc_unresolved: false,
                expected_backoff_ms: 100,
                expected_escalated: false,
                expected_fast_window_ms: None,
            },
            Case {
                name: "timeout_zero_huge_stall",
                base_backoff_ms: 100,
                quorum_timeout_ms: 0,
                stall_age_ms: 1_000,
                vote_count: 0,
                min_votes_for_commit: 3,
                rebroadcast_cooldown_ms: 25,
                contiguous_frontier: true,
                relay_backpressure: false,
                vote_queue_backlog: false,
                rbc_unresolved: false,
                expected_backoff_ms: 300,
                expected_escalated: false,
                expected_fast_window_ms: None,
            },
            Case {
                name: "below_moderate_stall",
                base_backoff_ms: 100,
                quorum_timeout_ms: 50,
                stall_age_ms: 99,
                vote_count: 0,
                min_votes_for_commit: 3,
                rebroadcast_cooldown_ms: 25,
                contiguous_frontier: true,
                relay_backpressure: false,
                vote_queue_backlog: false,
                rbc_unresolved: false,
                expected_backoff_ms: 300,
                expected_escalated: false,
                expected_fast_window_ms: None,
            },
            Case {
                name: "moderate_boundary_no_votes",
                base_backoff_ms: 100,
                quorum_timeout_ms: 50,
                stall_age_ms: 100,
                vote_count: 0,
                min_votes_for_commit: 3,
                rebroadcast_cooldown_ms: 25,
                contiguous_frontier: true,
                relay_backpressure: false,
                vote_queue_backlog: false,
                rbc_unresolved: false,
                expected_backoff_ms: 400,
                expected_escalated: true,
                expected_fast_window_ms: None,
            },
            Case {
                name: "moderate_boundary_at_quorum",
                base_backoff_ms: 100,
                quorum_timeout_ms: 50,
                stall_age_ms: 100,
                vote_count: 3,
                min_votes_for_commit: 3,
                rebroadcast_cooldown_ms: 25,
                contiguous_frontier: true,
                relay_backpressure: false,
                vote_queue_backlog: false,
                rbc_unresolved: false,
                expected_backoff_ms: 400,
                expected_escalated: true,
                expected_fast_window_ms: None,
            },
            Case {
                name: "severe_boundary_one_missing",
                base_backoff_ms: 100,
                quorum_timeout_ms: 50,
                stall_age_ms: 200,
                vote_count: 2,
                min_votes_for_commit: 3,
                rebroadcast_cooldown_ms: 25,
                contiguous_frontier: true,
                relay_backpressure: false,
                vote_queue_backlog: false,
                rbc_unresolved: false,
                expected_backoff_ms: 500,
                expected_escalated: true,
                expected_fast_window_ms: Some(25),
            },
            Case {
                name: "min_zero_boundary",
                base_backoff_ms: 100,
                quorum_timeout_ms: 50,
                stall_age_ms: 0,
                vote_count: 0,
                min_votes_for_commit: 0,
                rebroadcast_cooldown_ms: 25,
                contiguous_frontier: true,
                relay_backpressure: false,
                vote_queue_backlog: false,
                rbc_unresolved: false,
                expected_backoff_ms: 300,
                expected_escalated: false,
                expected_fast_window_ms: None,
            },
            Case {
                name: "resend_zero_cooldown",
                base_backoff_ms: 100,
                quorum_timeout_ms: 50,
                stall_age_ms: 0,
                vote_count: 0,
                min_votes_for_commit: 3,
                rebroadcast_cooldown_ms: 0,
                contiguous_frontier: true,
                relay_backpressure: false,
                vote_queue_backlog: false,
                rbc_unresolved: false,
                expected_backoff_ms: 300,
                expected_escalated: false,
                expected_fast_window_ms: None,
            },
            Case {
                name: "resend_nonzero_cooldown",
                base_backoff_ms: 100,
                quorum_timeout_ms: 50,
                stall_age_ms: 0,
                vote_count: 0,
                min_votes_for_commit: 3,
                rebroadcast_cooldown_ms: 25,
                contiguous_frontier: true,
                relay_backpressure: false,
                vote_queue_backlog: false,
                rbc_unresolved: false,
                expected_backoff_ms: 300,
                expected_escalated: false,
                expected_fast_window_ms: None,
            },
            Case {
                name: "fast_enabled",
                base_backoff_ms: 100,
                quorum_timeout_ms: 50,
                stall_age_ms: 0,
                vote_count: 2,
                min_votes_for_commit: 3,
                rebroadcast_cooldown_ms: 25,
                contiguous_frontier: true,
                relay_backpressure: false,
                vote_queue_backlog: false,
                rbc_unresolved: false,
                expected_backoff_ms: 200,
                expected_escalated: false,
                expected_fast_window_ms: Some(25),
            },
            Case {
                name: "fast_enabled_zero_cooldown",
                base_backoff_ms: 100,
                quorum_timeout_ms: 50,
                stall_age_ms: 0,
                vote_count: 2,
                min_votes_for_commit: 3,
                rebroadcast_cooldown_ms: 0,
                contiguous_frontier: true,
                relay_backpressure: false,
                vote_queue_backlog: false,
                rbc_unresolved: false,
                expected_backoff_ms: 200,
                expected_escalated: false,
                expected_fast_window_ms: Some(1),
            },
            Case {
                name: "fast_not_contiguous",
                base_backoff_ms: 100,
                quorum_timeout_ms: 50,
                stall_age_ms: 0,
                vote_count: 2,
                min_votes_for_commit: 3,
                rebroadcast_cooldown_ms: 25,
                contiguous_frontier: false,
                relay_backpressure: false,
                vote_queue_backlog: false,
                rbc_unresolved: false,
                expected_backoff_ms: 200,
                expected_escalated: false,
                expected_fast_window_ms: None,
            },
            Case {
                name: "fast_zero_votes",
                base_backoff_ms: 100,
                quorum_timeout_ms: 50,
                stall_age_ms: 0,
                vote_count: 0,
                min_votes_for_commit: 3,
                rebroadcast_cooldown_ms: 25,
                contiguous_frontier: true,
                relay_backpressure: false,
                vote_queue_backlog: false,
                rbc_unresolved: false,
                expected_backoff_ms: 300,
                expected_escalated: false,
                expected_fast_window_ms: None,
            },
            Case {
                name: "fast_at_quorum",
                base_backoff_ms: 100,
                quorum_timeout_ms: 50,
                stall_age_ms: 0,
                vote_count: 3,
                min_votes_for_commit: 3,
                rebroadcast_cooldown_ms: 25,
                contiguous_frontier: true,
                relay_backpressure: false,
                vote_queue_backlog: false,
                rbc_unresolved: false,
                expected_backoff_ms: 100,
                expected_escalated: false,
                expected_fast_window_ms: None,
            },
            Case {
                name: "fast_over_quorum",
                base_backoff_ms: 100,
                quorum_timeout_ms: 50,
                stall_age_ms: 0,
                vote_count: 4,
                min_votes_for_commit: 3,
                rebroadcast_cooldown_ms: 25,
                contiguous_frontier: true,
                relay_backpressure: false,
                vote_queue_backlog: false,
                rbc_unresolved: false,
                expected_backoff_ms: 100,
                expected_escalated: false,
                expected_fast_window_ms: None,
            },
            Case {
                name: "fast_relay_backpressure",
                base_backoff_ms: 100,
                quorum_timeout_ms: 50,
                stall_age_ms: 0,
                vote_count: 2,
                min_votes_for_commit: 3,
                rebroadcast_cooldown_ms: 25,
                contiguous_frontier: true,
                relay_backpressure: true,
                vote_queue_backlog: false,
                rbc_unresolved: false,
                expected_backoff_ms: 200,
                expected_escalated: false,
                expected_fast_window_ms: None,
            },
            Case {
                name: "fast_vote_queue_backlog",
                base_backoff_ms: 100,
                quorum_timeout_ms: 50,
                stall_age_ms: 0,
                vote_count: 2,
                min_votes_for_commit: 3,
                rebroadcast_cooldown_ms: 25,
                contiguous_frontier: true,
                relay_backpressure: false,
                vote_queue_backlog: true,
                rbc_unresolved: false,
                expected_backoff_ms: 200,
                expected_escalated: false,
                expected_fast_window_ms: None,
            },
            Case {
                name: "fast_rbc_unresolved",
                base_backoff_ms: 100,
                quorum_timeout_ms: 50,
                stall_age_ms: 0,
                vote_count: 2,
                min_votes_for_commit: 3,
                rebroadcast_cooldown_ms: 25,
                contiguous_frontier: true,
                relay_backpressure: false,
                vote_queue_backlog: false,
                rbc_unresolved: true,
                expected_backoff_ms: 200,
                expected_escalated: false,
                expected_fast_window_ms: None,
            },
        ] {
            let base_backoff = Duration::from_millis(case.base_backoff_ms);
            let quorum_timeout = Duration::from_millis(case.quorum_timeout_ms);
            let rebroadcast_cooldown = Duration::from_millis(case.rebroadcast_cooldown_ms);
            let (backoff, escalated) = adaptive_quorum_reschedule_backoff(
                base_backoff,
                Duration::from_millis(case.stall_age_ms),
                quorum_timeout,
                case.vote_count,
                case.min_votes_for_commit,
            );
            assert_eq!(
                backoff,
                Duration::from_millis(case.expected_backoff_ms),
                "backoff for {}",
                case.name
            );
            assert_eq!(
                escalated, case.expected_escalated,
                "escalation flag for {}",
                case.name
            );
            assert_eq!(
                contiguous_frontier_vote_backed_resend_window(
                    rebroadcast_cooldown,
                    case.vote_count,
                    case.min_votes_for_commit,
                ),
                Duration::from_millis(case.rebroadcast_cooldown_ms.max(1)),
                "resend window for {}",
                case.name
            );
            assert_eq!(
                contiguous_frontier_vote_backed_fast_resend_window(
                    rebroadcast_cooldown,
                    case.contiguous_frontier,
                    case.vote_count,
                    case.min_votes_for_commit,
                    case.relay_backpressure,
                    case.vote_queue_backlog,
                    case.rbc_unresolved,
                ),
                case.expected_fast_window_ms.map(Duration::from_millis),
                "fast resend window for {}",
                case.name
            );
        }
    }

    fn rbc_availability_session(
        invalid: bool,
        delivered: bool,
        total_chunks: u32,
        received_chunks: u32,
        ready_signatures: usize,
    ) -> Option<RbcAvailabilityRescheduleSession> {
        Some(RbcAvailabilityRescheduleSession {
            invalid,
            malformed_chunk_shape: total_chunks == 0 || received_chunks > total_chunks,
            delivered,
            complete_delivery: delivered && total_chunks != 0 && received_chunks == total_chunks,
            total_chunks,
            received_chunks,
            ready_signatures,
            required_ready: 3,
        })
    }

    fn rbc_availability_session_unverified_complete(
        ready_signatures: usize,
    ) -> Option<RbcAvailabilityRescheduleSession> {
        Some(RbcAvailabilityRescheduleSession {
            invalid: false,
            malformed_chunk_shape: false,
            delivered: true,
            complete_delivery: false,
            total_chunks: 4,
            received_chunks: 4,
            ready_signatures,
            required_ready: 3,
        })
    }

    #[test]
    fn rbc_availability_reschedule_formal_gate_matrix() {
        struct Case {
            name: &'static str,
            da_enabled: bool,
            stall_age_ms: u64,
            availability_timeout_ms: u64,
            local_payload_available: bool,
            pending_entry: bool,
            session: Option<RbcAvailabilityRescheduleSession>,
            expected_unresolved: bool,
        }

        for case in [
            Case {
                name: "DaDisabled",
                da_enabled: false,
                stall_age_ms: 0,
                availability_timeout_ms: 100,
                local_payload_available: false,
                pending_entry: false,
                session: rbc_availability_session(false, false, 4, 4, 3),
                expected_unresolved: false,
            },
            Case {
                name: "TimeoutBoundary",
                da_enabled: true,
                stall_age_ms: 100,
                availability_timeout_ms: 100,
                local_payload_available: false,
                pending_entry: false,
                session: rbc_availability_session(false, false, 4, 4, 3),
                expected_unresolved: false,
            },
            Case {
                name: "TimeoutBelowPending",
                da_enabled: true,
                stall_age_ms: 99,
                availability_timeout_ms: 100,
                local_payload_available: false,
                pending_entry: true,
                session: None,
                expected_unresolved: true,
            },
            Case {
                name: "TimeoutZeroPending",
                da_enabled: true,
                stall_age_ms: 1_000,
                availability_timeout_ms: 0,
                local_payload_available: false,
                pending_entry: true,
                session: None,
                expected_unresolved: true,
            },
            Case {
                name: "LocalPayloadAvailable",
                da_enabled: true,
                stall_age_ms: 0,
                availability_timeout_ms: 100,
                local_payload_available: true,
                pending_entry: false,
                session: rbc_availability_session(false, false, 4, 2, 2),
                expected_unresolved: false,
            },
            Case {
                name: "PendingEntry",
                da_enabled: true,
                stall_age_ms: 0,
                availability_timeout_ms: 100,
                local_payload_available: false,
                pending_entry: true,
                session: None,
                expected_unresolved: true,
            },
            Case {
                name: "NoSession",
                da_enabled: true,
                stall_age_ms: 0,
                availability_timeout_ms: 100,
                local_payload_available: false,
                pending_entry: false,
                session: None,
                expected_unresolved: false,
            },
            Case {
                name: "InvalidSession",
                da_enabled: true,
                stall_age_ms: 0,
                availability_timeout_ms: 100,
                local_payload_available: false,
                pending_entry: false,
                session: rbc_availability_session(true, false, 4, 2, 2),
                expected_unresolved: false,
            },
            Case {
                name: "DeliveredIncompleteSession",
                da_enabled: true,
                stall_age_ms: 0,
                availability_timeout_ms: 100,
                local_payload_available: false,
                pending_entry: false,
                session: rbc_availability_session(false, true, 4, 2, 2),
                expected_unresolved: true,
            },
            Case {
                name: "DeliveredCompleteButNotReady",
                da_enabled: true,
                stall_age_ms: 0,
                availability_timeout_ms: 100,
                local_payload_available: false,
                pending_entry: false,
                session: rbc_availability_session(false, true, 4, 4, 2),
                expected_unresolved: true,
            },
            Case {
                name: "DeliveredCompleteReady",
                da_enabled: true,
                stall_age_ms: 0,
                availability_timeout_ms: 100,
                local_payload_available: false,
                pending_entry: false,
                session: rbc_availability_session(false, true, 4, 4, 3),
                expected_unresolved: false,
            },
            Case {
                name: "MalformedCompleteReady",
                da_enabled: true,
                stall_age_ms: 0,
                availability_timeout_ms: 100,
                local_payload_available: false,
                pending_entry: false,
                session: Some(RbcAvailabilityRescheduleSession {
                    invalid: false,
                    malformed_chunk_shape: true,
                    delivered: true,
                    complete_delivery: true,
                    total_chunks: 4,
                    received_chunks: 4,
                    ready_signatures: 3,
                    required_ready: 3,
                }),
                expected_unresolved: true,
            },
            Case {
                name: "DeliveredCountCompleteUnverifiedReady",
                da_enabled: true,
                stall_age_ms: 0,
                availability_timeout_ms: 100,
                local_payload_available: false,
                pending_entry: false,
                session: rbc_availability_session_unverified_complete(3),
                expected_unresolved: true,
            },
            Case {
                name: "CompleteReady",
                da_enabled: true,
                stall_age_ms: 0,
                availability_timeout_ms: 100,
                local_payload_available: false,
                pending_entry: false,
                session: rbc_availability_session(false, false, 4, 4, 3),
                expected_unresolved: false,
            },
            Case {
                name: "MissingChunks",
                da_enabled: true,
                stall_age_ms: 0,
                availability_timeout_ms: 100,
                local_payload_available: false,
                pending_entry: false,
                session: rbc_availability_session(false, false, 4, 2, 3),
                expected_unresolved: true,
            },
            Case {
                name: "ZeroTotalReady",
                da_enabled: true,
                stall_age_ms: 0,
                availability_timeout_ms: 100,
                local_payload_available: false,
                pending_entry: false,
                session: rbc_availability_session(false, false, 0, 0, 3),
                expected_unresolved: true,
            },
            Case {
                name: "ZeroTotalReadyAfterTimeout",
                da_enabled: true,
                stall_age_ms: 100,
                availability_timeout_ms: 100,
                local_payload_available: false,
                pending_entry: false,
                session: rbc_availability_session(false, false, 0, 0, 3),
                expected_unresolved: false,
            },
            Case {
                name: "NotReady",
                da_enabled: true,
                stall_age_ms: 0,
                availability_timeout_ms: 100,
                local_payload_available: false,
                pending_entry: false,
                session: rbc_availability_session(false, false, 4, 4, 2),
                expected_unresolved: true,
            },
            Case {
                name: "CompleteButNotReady",
                da_enabled: true,
                stall_age_ms: 0,
                availability_timeout_ms: 100,
                local_payload_available: false,
                pending_entry: false,
                session: rbc_availability_session(false, false, 4, 4, 2),
                expected_unresolved: true,
            },
        ] {
            assert_eq!(
                rbc_availability_unresolved_for_reschedule_decision(
                    case.da_enabled,
                    Duration::from_millis(case.stall_age_ms),
                    Duration::from_millis(case.availability_timeout_ms),
                    case.local_payload_available,
                    case.pending_entry,
                    case.session,
                ),
                case.expected_unresolved,
                "availability decision for {}",
                case.name
            );
        }
    }

    fn vote_backed_instant_before(now: Instant, age_ms: u64) -> Instant {
        now.checked_sub(Duration::from_millis(age_ms))
            .unwrap_or(now)
    }

    fn vote_backed_slot_owner_state(
        now: Instant,
        height: u64,
        view: u64,
        active_mode: bool,
        last_reason_quorum_timeout: bool,
        latest_age_ms: u64,
        older_age_ms: u64,
    ) -> VoteBackedReassemblySlotOwnerState {
        VoteBackedReassemblySlotOwnerState {
            height,
            view,
            active_mode,
            last_reason_quorum_timeout,
            lag_started_at: vote_backed_instant_before(now, older_age_ms),
            last_progress_at: vote_backed_instant_before(now, older_age_ms),
            last_fetch_at: Some(vote_backed_instant_before(now, latest_age_ms)),
            last_view_advance_at: None,
            deep_catchup_entered_at: None,
            last_vote_at: None,
            last_commit_qc_at: None,
        }
    }

    fn vote_backed_recovery_owner_state(
        now: Instant,
        height: u64,
        view: u64,
        last_cause_quorum_timeout: bool,
        latest_age_ms: u64,
        older_age_ms: u64,
    ) -> VoteBackedReassemblyRecoveryOwnerState {
        VoteBackedReassemblyRecoveryOwnerState {
            frontier_height: height,
            last_view: view,
            last_cause_quorum_timeout,
            entered_at: vote_backed_instant_before(now, older_age_ms),
            last_progress_at: vote_backed_instant_before(now, older_age_ms),
            last_dependency_progress_at: None,
            last_action_at: Some(vote_backed_instant_before(now, latest_age_ms)),
        }
    }

    #[test]
    fn vote_backed_reassembly_stall_formal_gate_matrix() {
        #[derive(Clone, Copy)]
        struct Case {
            name: &'static str,
            frontier_window_ms: u64,
            quorum_timeout_ms: u64,
            rebroadcast_cooldown_ms: u64,
            slot_exact_height: bool,
            slot: Option<VoteBackedReassemblySlotOwnerState>,
            recovery: Option<VoteBackedReassemblyRecoveryOwnerState>,
            quorum_stall_age_ms: u64,
            expected_owner_age_ms: Option<u64>,
            expected_hard_cap_ms: u64,
            expected_expired: bool,
        }

        let now = Instant::now();
        let height = 10;
        let view = 2;
        let valid_slot = |latest_age_ms, older_age_ms| {
            Some(vote_backed_slot_owner_state(
                now,
                height,
                view,
                true,
                true,
                latest_age_ms,
                older_age_ms,
            ))
        };
        let rejected_slot = |latest_age_ms, older_age_ms| {
            Some(vote_backed_slot_owner_state(
                now,
                height,
                view,
                true,
                false,
                latest_age_ms,
                older_age_ms,
            ))
        };
        let valid_recovery = |latest_age_ms, older_age_ms| {
            Some(vote_backed_recovery_owner_state(
                now,
                height,
                view,
                true,
                latest_age_ms,
                older_age_ms,
            ))
        };

        for case in [
            Case {
                name: "HardCapFrontierWindowDominates",
                frontier_window_ms: 80,
                quorum_timeout_ms: 10,
                rebroadcast_cooldown_ms: 5,
                slot_exact_height: true,
                slot: valid_slot(200, 200),
                recovery: None,
                quorum_stall_age_ms: 200,
                expected_owner_age_ms: Some(200),
                expected_hard_cap_ms: 160,
                expected_expired: true,
            },
            Case {
                name: "HardCapQuorumTimeoutDominates",
                frontier_window_ms: 20,
                quorum_timeout_ms: 90,
                rebroadcast_cooldown_ms: 5,
                slot_exact_height: true,
                slot: valid_slot(200, 200),
                recovery: None,
                quorum_stall_age_ms: 200,
                expected_owner_age_ms: Some(200),
                expected_hard_cap_ms: 180,
                expected_expired: true,
            },
            Case {
                name: "HardCapRebroadcastDominates",
                frontier_window_ms: 20,
                quorum_timeout_ms: 30,
                rebroadcast_cooldown_ms: 70,
                slot_exact_height: true,
                slot: valid_slot(200, 200),
                recovery: None,
                quorum_stall_age_ms: 200,
                expected_owner_age_ms: Some(200),
                expected_hard_cap_ms: 140,
                expected_expired: true,
            },
            Case {
                name: "HardCapOneMsFloor",
                frontier_window_ms: 0,
                quorum_timeout_ms: 0,
                rebroadcast_cooldown_ms: 0,
                slot_exact_height: true,
                slot: valid_slot(200, 200),
                recovery: None,
                quorum_stall_age_ms: 200,
                expected_owner_age_ms: Some(200),
                expected_hard_cap_ms: 2,
                expected_expired: true,
            },
            Case {
                name: "SlotOwnerActive",
                frontier_window_ms: 40,
                quorum_timeout_ms: 10,
                rebroadcast_cooldown_ms: 5,
                slot_exact_height: true,
                slot: valid_slot(100, 100),
                recovery: None,
                quorum_stall_age_ms: 200,
                expected_owner_age_ms: Some(100),
                expected_hard_cap_ms: 80,
                expected_expired: true,
            },
            Case {
                name: "SlotOwnerFinalizedRejected",
                frontier_window_ms: 40,
                quorum_timeout_ms: 10,
                rebroadcast_cooldown_ms: 5,
                slot_exact_height: true,
                slot: Some(vote_backed_slot_owner_state(
                    now, height, view, false, true, 100, 100,
                )),
                recovery: None,
                quorum_stall_age_ms: 200,
                expected_owner_age_ms: None,
                expected_hard_cap_ms: 80,
                expected_expired: false,
            },
            Case {
                name: "SlotOwnerPassiveRejected",
                frontier_window_ms: 40,
                quorum_timeout_ms: 10,
                rebroadcast_cooldown_ms: 5,
                slot_exact_height: true,
                slot: Some(vote_backed_slot_owner_state(
                    now, height, view, false, true, 100, 100,
                )),
                recovery: None,
                quorum_stall_age_ms: 200,
                expected_owner_age_ms: None,
                expected_hard_cap_ms: 80,
                expected_expired: false,
            },
            Case {
                name: "SlotOwnerWrongReasonRejected",
                frontier_window_ms: 40,
                quorum_timeout_ms: 10,
                rebroadcast_cooldown_ms: 5,
                slot_exact_height: true,
                slot: rejected_slot(100, 100),
                recovery: None,
                quorum_stall_age_ms: 200,
                expected_owner_age_ms: None,
                expected_hard_cap_ms: 80,
                expected_expired: false,
            },
            Case {
                name: "SlotOwnerWrongViewRejected",
                frontier_window_ms: 40,
                quorum_timeout_ms: 10,
                rebroadcast_cooldown_ms: 5,
                slot_exact_height: true,
                slot: Some(vote_backed_slot_owner_state(
                    now,
                    height,
                    view.saturating_add(1),
                    true,
                    true,
                    100,
                    100,
                )),
                recovery: None,
                quorum_stall_age_ms: 200,
                expected_owner_age_ms: None,
                expected_hard_cap_ms: 80,
                expected_expired: false,
            },
            Case {
                name: "SlotOwnerWrongHeightRejected",
                frontier_window_ms: 40,
                quorum_timeout_ms: 10,
                rebroadcast_cooldown_ms: 5,
                slot_exact_height: true,
                slot: Some(vote_backed_slot_owner_state(
                    now,
                    height.saturating_add(1),
                    view,
                    true,
                    true,
                    100,
                    100,
                )),
                recovery: None,
                quorum_stall_age_ms: 200,
                expected_owner_age_ms: None,
                expected_hard_cap_ms: 80,
                expected_expired: false,
            },
            Case {
                name: "SlotOwnerNotExactHeightRejected",
                frontier_window_ms: 40,
                quorum_timeout_ms: 10,
                rebroadcast_cooldown_ms: 5,
                slot_exact_height: false,
                slot: valid_slot(100, 100),
                recovery: None,
                quorum_stall_age_ms: 200,
                expected_owner_age_ms: None,
                expected_hard_cap_ms: 80,
                expected_expired: false,
            },
            Case {
                name: "SlotOwnerUsesLatestProgress",
                frontier_window_ms: 40,
                quorum_timeout_ms: 10,
                rebroadcast_cooldown_ms: 5,
                slot_exact_height: true,
                slot: valid_slot(50, 900),
                recovery: None,
                quorum_stall_age_ms: 200,
                expected_owner_age_ms: Some(50),
                expected_hard_cap_ms: 80,
                expected_expired: false,
            },
            Case {
                name: "RecoveryOwnerActive",
                frontier_window_ms: 40,
                quorum_timeout_ms: 10,
                rebroadcast_cooldown_ms: 5,
                slot_exact_height: true,
                slot: None,
                recovery: valid_recovery(100, 100),
                quorum_stall_age_ms: 200,
                expected_owner_age_ms: Some(100),
                expected_hard_cap_ms: 80,
                expected_expired: true,
            },
            Case {
                name: "RecoveryOwnerWrongCauseRejected",
                frontier_window_ms: 40,
                quorum_timeout_ms: 10,
                rebroadcast_cooldown_ms: 5,
                slot_exact_height: true,
                slot: None,
                recovery: Some(vote_backed_recovery_owner_state(
                    now, height, view, false, 100, 100,
                )),
                quorum_stall_age_ms: 200,
                expected_owner_age_ms: None,
                expected_hard_cap_ms: 80,
                expected_expired: false,
            },
            Case {
                name: "RecoveryOwnerWrongViewRejected",
                frontier_window_ms: 40,
                quorum_timeout_ms: 10,
                rebroadcast_cooldown_ms: 5,
                slot_exact_height: true,
                slot: None,
                recovery: Some(vote_backed_recovery_owner_state(
                    now,
                    height,
                    view.saturating_add(1),
                    true,
                    100,
                    100,
                )),
                quorum_stall_age_ms: 200,
                expected_owner_age_ms: None,
                expected_hard_cap_ms: 80,
                expected_expired: false,
            },
            Case {
                name: "RecoveryOwnerUsesLatestProgress",
                frontier_window_ms: 40,
                quorum_timeout_ms: 10,
                rebroadcast_cooldown_ms: 5,
                slot_exact_height: true,
                slot: None,
                recovery: valid_recovery(40, 200),
                quorum_stall_age_ms: 200,
                expected_owner_age_ms: Some(40),
                expected_hard_cap_ms: 80,
                expected_expired: false,
            },
            Case {
                name: "RecoveryAfterRejectedSlot",
                frontier_window_ms: 40,
                quorum_timeout_ms: 10,
                rebroadcast_cooldown_ms: 5,
                slot_exact_height: true,
                slot: rejected_slot(100, 100),
                recovery: valid_recovery(100, 100),
                quorum_stall_age_ms: 200,
                expected_owner_age_ms: Some(100),
                expected_hard_cap_ms: 80,
                expected_expired: true,
            },
            Case {
                name: "NoOwnerNoExpiry",
                frontier_window_ms: 40,
                quorum_timeout_ms: 10,
                rebroadcast_cooldown_ms: 5,
                slot_exact_height: true,
                slot: None,
                recovery: None,
                quorum_stall_age_ms: 200,
                expected_owner_age_ms: None,
                expected_hard_cap_ms: 80,
                expected_expired: false,
            },
            Case {
                name: "OwnerBelowCapNoExpiry",
                frontier_window_ms: 40,
                quorum_timeout_ms: 10,
                rebroadcast_cooldown_ms: 5,
                slot_exact_height: true,
                slot: valid_slot(70, 70),
                recovery: None,
                quorum_stall_age_ms: 200,
                expected_owner_age_ms: Some(70),
                expected_hard_cap_ms: 80,
                expected_expired: false,
            },
            Case {
                name: "QuorumBelowCapNoExpiry",
                frontier_window_ms: 40,
                quorum_timeout_ms: 10,
                rebroadcast_cooldown_ms: 5,
                slot_exact_height: true,
                slot: valid_slot(100, 100),
                recovery: None,
                quorum_stall_age_ms: 70,
                expected_owner_age_ms: Some(100),
                expected_hard_cap_ms: 80,
                expected_expired: false,
            },
            Case {
                name: "BothAtCapExpires",
                frontier_window_ms: 40,
                quorum_timeout_ms: 10,
                rebroadcast_cooldown_ms: 5,
                slot_exact_height: true,
                slot: valid_slot(80, 80),
                recovery: None,
                quorum_stall_age_ms: 80,
                expected_owner_age_ms: Some(80),
                expected_hard_cap_ms: 80,
                expected_expired: true,
            },
        ] {
            let hard_cap = vote_backed_frontier_reassembly_hard_cap_from_windows(
                Duration::from_millis(case.frontier_window_ms),
                Duration::from_millis(case.quorum_timeout_ms),
                Duration::from_millis(case.rebroadcast_cooldown_ms),
                2,
                3,
            );
            assert_eq!(
                hard_cap,
                Duration::from_millis(case.expected_hard_cap_ms),
                "hard cap for {}",
                case.name
            );

            let owner_stall_age = vote_backed_frontier_reassembly_owner_stall_age_from_sources(
                height,
                view,
                now,
                case.slot_exact_height,
                case.slot,
                case.recovery,
            );
            assert_eq!(
                owner_stall_age,
                case.expected_owner_age_ms.map(Duration::from_millis),
                "owner stall age for {}",
                case.name
            );

            let expired = vote_backed_frontier_reassembly_stall_expiry(
                owner_stall_age,
                Duration::from_millis(case.quorum_stall_age_ms),
                hard_cap,
            );
            assert_eq!(
                expired.is_some(),
                case.expected_expired,
                "expiry decision for {}",
                case.name
            );
            if let Some((owner_age, expired_hard_cap)) = expired {
                assert_eq!(Some(owner_age), owner_stall_age);
                assert_eq!(expired_hard_cap, hard_cap);
            }
        }
    }

    #[test]
    fn completed_quorum_view_advance_route_formal_gate_matrix() {
        struct Case {
            name: &'static str,
            input_height: u64,
            slot_height: Option<u64>,
            expected_route: CompletedQuorumViewAdvanceRoute,
        }

        let committed_height = 2;
        let frontier_height = committed_height + 1;
        for case in [
            Case {
                name: "ExactRequestedDominates",
                input_height: frontier_height,
                slot_height: Some(frontier_height),
                expected_route: CompletedQuorumViewAdvanceRoute::ExactSlot,
            },
            Case {
                name: "ExactActiveDominates",
                input_height: frontier_height,
                slot_height: Some(frontier_height),
                expected_route: CompletedQuorumViewAdvanceRoute::ExactSlot,
            },
            Case {
                name: "ExactCandidateDominates",
                input_height: frontier_height,
                slot_height: Some(frontier_height),
                expected_route: CompletedQuorumViewAdvanceRoute::ExactSlot,
            },
            Case {
                name: "ExactSaturatingIncrement",
                input_height: frontier_height,
                slot_height: Some(frontier_height),
                expected_route: CompletedQuorumViewAdvanceRoute::ExactSlot,
            },
            Case {
                name: "ExactClearsRebroadcast",
                input_height: frontier_height,
                slot_height: Some(frontier_height),
                expected_route: CompletedQuorumViewAdvanceRoute::ExactSlot,
            },
            Case {
                name: "ExactUpdatesTimestamps",
                input_height: frontier_height,
                slot_height: Some(frontier_height),
                expected_route: CompletedQuorumViewAdvanceRoute::ExactSlot,
            },
            Case {
                name: "ExactCausePreserved",
                input_height: frontier_height,
                slot_height: Some(frontier_height),
                expected_route: CompletedQuorumViewAdvanceRoute::ExactSlot,
            },
            Case {
                name: "ExactNoSlotFallback",
                input_height: frontier_height,
                slot_height: None,
                expected_route: CompletedQuorumViewAdvanceRoute::ExactFallback,
            },
            Case {
                name: "ExactStaleSlotFallback",
                input_height: frontier_height,
                slot_height: Some(frontier_height + 1),
                expected_route: CompletedQuorumViewAdvanceRoute::ExactFallback,
            },
            Case {
                name: "LowerHeightGeneric",
                input_height: committed_height,
                slot_height: Some(frontier_height),
                expected_route: CompletedQuorumViewAdvanceRoute::Generic,
            },
            Case {
                name: "FutureHeightGeneric",
                input_height: frontier_height + 1,
                slot_height: Some(frontier_height),
                expected_route: CompletedQuorumViewAdvanceRoute::Generic,
            },
            Case {
                name: "GenericPreservesSlotState",
                input_height: frontier_height + 1,
                slot_height: Some(frontier_height),
                expected_route: CompletedQuorumViewAdvanceRoute::Generic,
            },
            Case {
                name: "GenericCausePreserved",
                input_height: frontier_height + 1,
                slot_height: Some(frontier_height),
                expected_route: CompletedQuorumViewAdvanceRoute::Generic,
            },
            Case {
                name: "NonExactNoSlotGeneric",
                input_height: frontier_height + 1,
                slot_height: None,
                expected_route: CompletedQuorumViewAdvanceRoute::Generic,
            },
        ] {
            assert_eq!(
                completed_quorum_view_advance_route(
                    case.input_height,
                    committed_height,
                    case.slot_height,
                ),
                case.expected_route,
                "completed quorum route for {}",
                case.name
            );
        }
    }

    #[test]
    fn retransmit_backpressure_formal_gate_matrix() {
        struct Case {
            tx_depth: u64,
            tx_capacity: u64,
            tx_saturated: bool,
            rbc_bytes: u64,
            rbc_pressure_level: u8,
            target_count: usize,
            base_backoff_ms: u64,
            consensus_backlog: bool,
            near_backlog: bool,
            rebroadcast_cooldown_ms: u64,
            expected_pressure_score: u8,
            expected_target_limit: usize,
            expected_cooldown_multiplier: u32,
            expected_backoff_ms: u64,
            expected_timeout_ms: u64,
        }

        for case in [
            Case {
                tx_depth: 100,
                tx_capacity: 0,
                tx_saturated: false,
                rbc_bytes: 0,
                rbc_pressure_level: 0,
                target_count: 12,
                base_backoff_ms: 100,
                consensus_backlog: false,
                near_backlog: false,
                rebroadcast_cooldown_ms: 300,
                expected_pressure_score: 0,
                expected_target_limit: 12,
                expected_cooldown_multiplier: 1,
                expected_backoff_ms: 100,
                expected_timeout_ms: 600,
            },
            Case {
                tx_depth: 4,
                tx_capacity: 100,
                tx_saturated: false,
                rbc_bytes: 0,
                rbc_pressure_level: 0,
                target_count: 12,
                base_backoff_ms: 100,
                consensus_backlog: false,
                near_backlog: false,
                rebroadcast_cooldown_ms: 300,
                expected_pressure_score: 0,
                expected_target_limit: 12,
                expected_cooldown_multiplier: 1,
                expected_backoff_ms: 100,
                expected_timeout_ms: 600,
            },
            Case {
                tx_depth: 60,
                tx_capacity: 100,
                tx_saturated: false,
                rbc_bytes: 0,
                rbc_pressure_level: 0,
                target_count: 12,
                base_backoff_ms: 100,
                consensus_backlog: false,
                near_backlog: false,
                rebroadcast_cooldown_ms: 300,
                expected_pressure_score: 1,
                expected_target_limit: 12,
                expected_cooldown_multiplier: 1,
                expected_backoff_ms: 100,
                expected_timeout_ms: 600,
            },
            Case {
                tx_depth: 80,
                tx_capacity: 100,
                tx_saturated: false,
                rbc_bytes: 0,
                rbc_pressure_level: 0,
                target_count: 12,
                base_backoff_ms: 100,
                consensus_backlog: false,
                near_backlog: false,
                rebroadcast_cooldown_ms: 300,
                expected_pressure_score: 2,
                expected_target_limit: 6,
                expected_cooldown_multiplier: 2,
                expected_backoff_ms: 100,
                expected_timeout_ms: 600,
            },
            Case {
                tx_depth: 95,
                tx_capacity: 100,
                tx_saturated: false,
                rbc_bytes: 0,
                rbc_pressure_level: 0,
                target_count: 12,
                base_backoff_ms: 100,
                consensus_backlog: false,
                near_backlog: false,
                rebroadcast_cooldown_ms: 300,
                expected_pressure_score: 3,
                expected_target_limit: 6,
                expected_cooldown_multiplier: 2,
                expected_backoff_ms: 100,
                expected_timeout_ms: 600,
            },
            Case {
                tx_depth: 1,
                tx_capacity: 100,
                tx_saturated: true,
                rbc_bytes: 0,
                rbc_pressure_level: 0,
                target_count: 12,
                base_backoff_ms: 100,
                consensus_backlog: false,
                near_backlog: false,
                rebroadcast_cooldown_ms: 300,
                expected_pressure_score: 3,
                expected_target_limit: 6,
                expected_cooldown_multiplier: 2,
                expected_backoff_ms: 100,
                expected_timeout_ms: 600,
            },
            Case {
                tx_depth: 0,
                tx_capacity: 100,
                tx_saturated: false,
                rbc_bytes: 0,
                rbc_pressure_level: 1,
                target_count: 12,
                base_backoff_ms: 100,
                consensus_backlog: false,
                near_backlog: false,
                rebroadcast_cooldown_ms: 300,
                expected_pressure_score: 2,
                expected_target_limit: 6,
                expected_cooldown_multiplier: 2,
                expected_backoff_ms: 100,
                expected_timeout_ms: 600,
            },
            Case {
                tx_depth: 0,
                tx_capacity: 100,
                tx_saturated: false,
                rbc_bytes: 0,
                rbc_pressure_level: 2,
                target_count: 12,
                base_backoff_ms: 100,
                consensus_backlog: false,
                near_backlog: false,
                rebroadcast_cooldown_ms: 300,
                expected_pressure_score: 3,
                expected_target_limit: 6,
                expected_cooldown_multiplier: 2,
                expected_backoff_ms: 100,
                expected_timeout_ms: 600,
            },
            Case {
                tx_depth: 0,
                tx_capacity: 100,
                tx_saturated: false,
                rbc_bytes: RETRANSMIT_RBC_BYTES_SOFT,
                rbc_pressure_level: 0,
                target_count: 12,
                base_backoff_ms: 100,
                consensus_backlog: false,
                near_backlog: false,
                rebroadcast_cooldown_ms: 300,
                expected_pressure_score: 1,
                expected_target_limit: 12,
                expected_cooldown_multiplier: 1,
                expected_backoff_ms: 100,
                expected_timeout_ms: 600,
            },
            Case {
                tx_depth: 0,
                tx_capacity: 100,
                tx_saturated: false,
                rbc_bytes: RETRANSMIT_RBC_BYTES_HARD,
                rbc_pressure_level: 0,
                target_count: 12,
                base_backoff_ms: 100,
                consensus_backlog: false,
                near_backlog: false,
                rebroadcast_cooldown_ms: 300,
                expected_pressure_score: 2,
                expected_target_limit: 6,
                expected_cooldown_multiplier: 2,
                expected_backoff_ms: 100,
                expected_timeout_ms: 600,
            },
            Case {
                tx_depth: 80,
                tx_capacity: 100,
                tx_saturated: false,
                rbc_bytes: RETRANSMIT_RBC_BYTES_SOFT,
                rbc_pressure_level: 1,
                target_count: 12,
                base_backoff_ms: 100,
                consensus_backlog: false,
                near_backlog: false,
                rebroadcast_cooldown_ms: 300,
                expected_pressure_score: 5,
                expected_target_limit: 3,
                expected_cooldown_multiplier: 3,
                expected_backoff_ms: 100,
                expected_timeout_ms: 600,
            },
            Case {
                tx_depth: 100,
                tx_capacity: 100,
                tx_saturated: true,
                rbc_bytes: RETRANSMIT_RBC_BYTES_HARD,
                rbc_pressure_level: 2,
                target_count: 12,
                base_backoff_ms: 100,
                consensus_backlog: false,
                near_backlog: false,
                rebroadcast_cooldown_ms: 300,
                expected_pressure_score: 8,
                expected_target_limit: 1,
                expected_cooldown_multiplier: 4,
                expected_backoff_ms: 100,
                expected_timeout_ms: 600,
            },
            Case {
                tx_depth: 100,
                tx_capacity: 100,
                tx_saturated: true,
                rbc_bytes: RETRANSMIT_RBC_BYTES_HARD,
                rbc_pressure_level: 2,
                target_count: 0,
                base_backoff_ms: 100,
                consensus_backlog: false,
                near_backlog: false,
                rebroadcast_cooldown_ms: 300,
                expected_pressure_score: 8,
                expected_target_limit: 0,
                expected_cooldown_multiplier: 4,
                expected_backoff_ms: 100,
                expected_timeout_ms: 600,
            },
            Case {
                tx_depth: 0,
                tx_capacity: 100,
                tx_saturated: false,
                rbc_bytes: 0,
                rbc_pressure_level: 1,
                target_count: 5,
                base_backoff_ms: 100,
                consensus_backlog: false,
                near_backlog: false,
                rebroadcast_cooldown_ms: 300,
                expected_pressure_score: 2,
                expected_target_limit: 3,
                expected_cooldown_multiplier: 2,
                expected_backoff_ms: 100,
                expected_timeout_ms: 600,
            },
            Case {
                tx_depth: 0,
                tx_capacity: 100,
                tx_saturated: false,
                rbc_bytes: RETRANSMIT_RBC_BYTES_HARD,
                rbc_pressure_level: 1,
                target_count: 5,
                base_backoff_ms: 100,
                consensus_backlog: false,
                near_backlog: false,
                rebroadcast_cooldown_ms: 300,
                expected_pressure_score: 4,
                expected_target_limit: 2,
                expected_cooldown_multiplier: 3,
                expected_backoff_ms: 100,
                expected_timeout_ms: 600,
            },
            Case {
                tx_depth: 0,
                tx_capacity: 100,
                tx_saturated: false,
                rbc_bytes: 0,
                rbc_pressure_level: 0,
                target_count: 12,
                base_backoff_ms: 0,
                consensus_backlog: true,
                near_backlog: true,
                rebroadcast_cooldown_ms: 300,
                expected_pressure_score: 0,
                expected_target_limit: 12,
                expected_cooldown_multiplier: 1,
                expected_backoff_ms: 0,
                expected_timeout_ms: 600,
            },
            Case {
                tx_depth: 0,
                tx_capacity: 100,
                tx_saturated: false,
                rbc_bytes: 0,
                rbc_pressure_level: 0,
                target_count: 12,
                base_backoff_ms: 100,
                consensus_backlog: true,
                near_backlog: false,
                rebroadcast_cooldown_ms: 300,
                expected_pressure_score: 0,
                expected_target_limit: 12,
                expected_cooldown_multiplier: 1,
                expected_backoff_ms: 400,
                expected_timeout_ms: 600,
            },
            Case {
                tx_depth: 0,
                tx_capacity: 100,
                tx_saturated: false,
                rbc_bytes: 0,
                rbc_pressure_level: 0,
                target_count: 12,
                base_backoff_ms: 100,
                consensus_backlog: true,
                near_backlog: true,
                rebroadcast_cooldown_ms: 300,
                expected_pressure_score: 0,
                expected_target_limit: 12,
                expected_cooldown_multiplier: 1,
                expected_backoff_ms: 800,
                expected_timeout_ms: 600,
            },
            Case {
                tx_depth: 0,
                tx_capacity: 100,
                tx_saturated: false,
                rbc_bytes: 0,
                rbc_pressure_level: 0,
                target_count: 12,
                base_backoff_ms: 100,
                consensus_backlog: false,
                near_backlog: false,
                rebroadcast_cooldown_ms: 50,
                expected_pressure_score: 0,
                expected_target_limit: 12,
                expected_cooldown_multiplier: 1,
                expected_backoff_ms: 100,
                expected_timeout_ms: 200,
            },
            Case {
                tx_depth: 0,
                tx_capacity: 100,
                tx_saturated: false,
                rbc_bytes: 0,
                rbc_pressure_level: 0,
                target_count: 12,
                base_backoff_ms: 100,
                consensus_backlog: false,
                near_backlog: false,
                rebroadcast_cooldown_ms: 300,
                expected_pressure_score: 0,
                expected_target_limit: 12,
                expected_cooldown_multiplier: 1,
                expected_backoff_ms: 100,
                expected_timeout_ms: 600,
            },
            Case {
                tx_depth: 0,
                tx_capacity: 100,
                tx_saturated: false,
                rbc_bytes: 0,
                rbc_pressure_level: 0,
                target_count: 12,
                base_backoff_ms: 100,
                consensus_backlog: false,
                near_backlog: false,
                rebroadcast_cooldown_ms: 2_000,
                expected_pressure_score: 0,
                expected_target_limit: 12,
                expected_cooldown_multiplier: 1,
                expected_backoff_ms: 100,
                expected_timeout_ms: 2_000,
            },
        ] {
            let pressure_score = retransmit_pressure_score(
                case.tx_depth,
                case.tx_capacity,
                case.tx_saturated,
                case.rbc_bytes,
                case.rbc_pressure_level,
            );
            assert_eq!(pressure_score, case.expected_pressure_score);
            assert_eq!(
                retransmit_target_limit(case.target_count, pressure_score),
                case.expected_target_limit
            );
            assert_eq!(
                retransmit_cooldown_multiplier(pressure_score),
                case.expected_cooldown_multiplier
            );
            assert_eq!(
                consensus_ingress_reschedule_backoff(
                    Duration::from_millis(case.base_backoff_ms),
                    case.consensus_backlog,
                    case.near_backlog,
                ),
                Duration::from_millis(case.expected_backoff_ms)
            );
            assert_eq!(
                near_quorum_payload_timeout(Duration::from_millis(case.rebroadcast_cooldown_ms,)),
                Duration::from_millis(case.expected_timeout_ms)
            );
        }
    }

    #[test]
    fn retransmit_pressure_score_grows_with_queue_and_rbc_backlog() {
        let baseline = retransmit_pressure_score(4, 100, false, 0, 0);
        let moderate = retransmit_pressure_score(70, 100, false, RETRANSMIT_RBC_BYTES_SOFT, 1);
        let severe = retransmit_pressure_score(100, 100, true, RETRANSMIT_RBC_BYTES_HARD, 2);

        assert!(baseline < moderate);
        assert!(moderate < severe);
    }

    #[test]
    fn retransmit_target_limit_and_cooldown_scale_with_pressure() {
        let target_count = 12usize;
        assert_eq!(retransmit_target_limit(target_count, 0), target_count);
        assert_eq!(retransmit_target_limit(target_count, 2), 6);
        assert_eq!(retransmit_target_limit(target_count, 4), 3);
        assert_eq!(retransmit_target_limit(target_count, 6), 1);

        assert_eq!(retransmit_cooldown_multiplier(0), 1);
        assert_eq!(retransmit_cooldown_multiplier(2), 2);
        assert_eq!(retransmit_cooldown_multiplier(4), 3);
        assert_eq!(retransmit_cooldown_multiplier(6), 4);
    }

    #[test]
    fn near_quorum_payload_timeout_clamps_to_expected_window() {
        assert_eq!(
            near_quorum_payload_timeout(Duration::from_millis(50)),
            Duration::from_millis(200)
        );
        assert_eq!(
            near_quorum_payload_timeout(Duration::from_millis(300)),
            Duration::from_millis(600)
        );
        assert_eq!(
            near_quorum_payload_timeout(Duration::from_millis(2_000)),
            Duration::from_millis(2_000)
        );
    }

    #[test]
    fn consensus_ingress_backlog_expands_quorum_reschedule_backoff() {
        let base = Duration::from_millis(100);

        assert_eq!(
            consensus_ingress_reschedule_backoff(base, false, false),
            base
        );
        assert_eq!(
            consensus_ingress_reschedule_backoff(base, true, false),
            Duration::from_millis(400)
        );
        assert_eq!(
            consensus_ingress_reschedule_backoff(base, true, true),
            Duration::from_millis(800)
        );
    }

    #[derive(Clone, Copy, Debug)]
    enum QuorumRebroadcastDispatchCase {
        DropPendingNoLocalVote,
        EmptyTopologyNoLocalVote,
        LocalVoteEmitted,
        RelayBackpressureExit,
        NoTargetsExit,
        CooldownExit,
        BacklogLimitZeroExit,
        PacedTargetsEmptyExit,
        ForceFanoutBypassesCooldown,
        ForceFanoutBypassesLimit,
        VoteReplayOnly,
        DropPendingSuppressesPayload,
        CachedCommitQcSuppressesMissingFetch,
        MissingFetchWithVoteBacking,
        ContiguousNearQuorumBlockSync,
        BlockSyncFrameTooLarge,
        BlockSyncLocalOnlyTargets,
        BlockSyncNonSyncPayload,
        NonContiguousNoBlockSync,
        NotNearQuorumNoBlockSync,
        BlockCreatedWithVoteBacking,
        NoObservedBackingNoBlockCreated,
        AnyActionMarks,
        NoActionNoMark,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct QuorumRebroadcastDispatchOutput {
        local_vote: bool,
        votes: usize,
        block_sync: bool,
        block: bool,
        missing_block_fetch: bool,
        mark_rebroadcast: bool,
        target_count: usize,
    }

    impl QuorumRebroadcastDispatchCase {
        const ALL: [Self; 24] = [
            Self::DropPendingNoLocalVote,
            Self::EmptyTopologyNoLocalVote,
            Self::LocalVoteEmitted,
            Self::RelayBackpressureExit,
            Self::NoTargetsExit,
            Self::CooldownExit,
            Self::BacklogLimitZeroExit,
            Self::PacedTargetsEmptyExit,
            Self::ForceFanoutBypassesCooldown,
            Self::ForceFanoutBypassesLimit,
            Self::VoteReplayOnly,
            Self::DropPendingSuppressesPayload,
            Self::CachedCommitQcSuppressesMissingFetch,
            Self::MissingFetchWithVoteBacking,
            Self::ContiguousNearQuorumBlockSync,
            Self::BlockSyncFrameTooLarge,
            Self::BlockSyncLocalOnlyTargets,
            Self::BlockSyncNonSyncPayload,
            Self::NonContiguousNoBlockSync,
            Self::NotNearQuorumNoBlockSync,
            Self::BlockCreatedWithVoteBacking,
            Self::NoObservedBackingNoBlockCreated,
            Self::AnyActionMarks,
            Self::NoActionNoMark,
        ];

        fn label(self) -> &'static str {
            match self {
                Self::DropPendingNoLocalVote => "DropPendingNoLocalVote",
                Self::EmptyTopologyNoLocalVote => "EmptyTopologyNoLocalVote",
                Self::LocalVoteEmitted => "LocalVoteEmitted",
                Self::RelayBackpressureExit => "RelayBackpressureExit",
                Self::NoTargetsExit => "NoTargetsExit",
                Self::CooldownExit => "CooldownExit",
                Self::BacklogLimitZeroExit => "BacklogLimitZeroExit",
                Self::PacedTargetsEmptyExit => "PacedTargetsEmptyExit",
                Self::ForceFanoutBypassesCooldown => "ForceFanoutBypassesCooldown",
                Self::ForceFanoutBypassesLimit => "ForceFanoutBypassesLimit",
                Self::VoteReplayOnly => "VoteReplayOnly",
                Self::DropPendingSuppressesPayload => "DropPendingSuppressesPayload",
                Self::CachedCommitQcSuppressesMissingFetch => {
                    "CachedCommitQcSuppressesMissingFetch"
                }
                Self::MissingFetchWithVoteBacking => "MissingFetchWithVoteBacking",
                Self::ContiguousNearQuorumBlockSync => "ContiguousNearQuorumBlockSync",
                Self::BlockSyncFrameTooLarge => "BlockSyncFrameTooLarge",
                Self::BlockSyncLocalOnlyTargets => "BlockSyncLocalOnlyTargets",
                Self::BlockSyncNonSyncPayload => "BlockSyncNonSyncPayload",
                Self::NonContiguousNoBlockSync => "NonContiguousNoBlockSync",
                Self::NotNearQuorumNoBlockSync => "NotNearQuorumNoBlockSync",
                Self::BlockCreatedWithVoteBacking => "BlockCreatedWithVoteBacking",
                Self::NoObservedBackingNoBlockCreated => "NoObservedBackingNoBlockCreated",
                Self::AnyActionMarks => "AnyActionMarks",
                Self::NoActionNoMark => "NoActionNoMark",
            }
        }

        fn drop_pending(self) -> bool {
            matches!(
                self,
                Self::DropPendingNoLocalVote
                    | Self::VoteReplayOnly
                    | Self::DropPendingSuppressesPayload
            )
        }

        fn local_vote_can_emit(self) -> bool {
            matches!(
                self,
                Self::DropPendingNoLocalVote
                    | Self::EmptyTopologyNoLocalVote
                    | Self::LocalVoteEmitted
            )
        }

        fn topology_non_empty(self) -> bool {
            !matches!(self, Self::EmptyTopologyNoLocalVote)
        }

        fn raw_vote_count(self) -> usize {
            match self {
                Self::LocalVoteEmitted
                | Self::NoObservedBackingNoBlockCreated
                | Self::NoActionNoMark => 0,
                Self::NotNearQuorumNoBlockSync => 1,
                _ => 2,
            }
        }

        fn vote_count_input(self) -> usize {
            if matches!(self, Self::NoObservedBackingNoBlockCreated) {
                0
            } else {
                self.raw_vote_count()
            }
        }

        fn relay_backpressure(self) -> bool {
            matches!(self, Self::RelayBackpressureExit)
        }

        fn initial_target_count(self) -> usize {
            match self {
                Self::EmptyTopologyNoLocalVote | Self::NoTargetsExit | Self::NoActionNoMark => 0,
                Self::BlockSyncLocalOnlyTargets => 1,
                _ => 2,
            }
        }

        fn widen_repair_fanout(self) -> bool {
            matches!(
                self,
                Self::ForceFanoutBypassesCooldown | Self::ForceFanoutBypassesLimit
            )
        }

        fn cooldown_due(self) -> bool {
            !matches!(self, Self::CooldownExit | Self::ForceFanoutBypassesCooldown)
        }

        fn target_limit(self) -> usize {
            if matches!(
                self,
                Self::BacklogLimitZeroExit | Self::ForceFanoutBypassesLimit
            ) {
                0
            } else {
                1
            }
        }

        fn paced_target_count(self) -> usize {
            if matches!(self, Self::PacedTargetsEmptyExit) || self.target_limit() == 0 {
                0
            } else {
                self.initial_target_count().min(self.target_limit())
            }
        }

        fn has_cached_commit_qc(self) -> bool {
            matches!(self, Self::CachedCommitQcSuppressesMissingFetch)
        }

        fn contiguous_frontier(self) -> bool {
            !matches!(self, Self::NonContiguousNoBlockSync)
        }

        fn block_sync_update_available(self) -> bool {
            !matches!(self, Self::BlockSyncNonSyncPayload)
        }

        fn block_sync_update_fits_frame(self) -> bool {
            !matches!(self, Self::BlockSyncFrameTooLarge)
        }

        fn expected(self) -> QuorumRebroadcastDispatchOutput {
            use QuorumRebroadcastDispatchCase as Case;
            match self {
                Case::DropPendingNoLocalVote
                | Case::VoteReplayOnly
                | Case::DropPendingSuppressesPayload
                | Case::NoObservedBackingNoBlockCreated => QuorumRebroadcastDispatchOutput {
                    local_vote: false,
                    votes: 1,
                    block_sync: false,
                    block: false,
                    missing_block_fetch: false,
                    mark_rebroadcast: true,
                    target_count: 1,
                },
                Case::EmptyTopologyNoLocalVote
                | Case::NoTargetsExit
                | Case::BacklogLimitZeroExit
                | Case::PacedTargetsEmptyExit
                | Case::NoActionNoMark => QuorumRebroadcastDispatchOutput {
                    local_vote: false,
                    votes: 0,
                    block_sync: false,
                    block: false,
                    missing_block_fetch: false,
                    mark_rebroadcast: false,
                    target_count: 0,
                },
                Case::RelayBackpressureExit | Case::CooldownExit => {
                    QuorumRebroadcastDispatchOutput {
                        local_vote: false,
                        votes: 0,
                        block_sync: false,
                        block: false,
                        missing_block_fetch: false,
                        mark_rebroadcast: false,
                        target_count: 1,
                    }
                }
                Case::LocalVoteEmitted => QuorumRebroadcastDispatchOutput {
                    local_vote: true,
                    votes: 1,
                    block_sync: false,
                    block: true,
                    missing_block_fetch: true,
                    mark_rebroadcast: true,
                    target_count: 1,
                },
                Case::ForceFanoutBypassesCooldown | Case::ForceFanoutBypassesLimit => {
                    QuorumRebroadcastDispatchOutput {
                        local_vote: false,
                        votes: 3,
                        block_sync: true,
                        block: true,
                        missing_block_fetch: true,
                        mark_rebroadcast: true,
                        target_count: 3,
                    }
                }
                Case::CachedCommitQcSuppressesMissingFetch => QuorumRebroadcastDispatchOutput {
                    local_vote: false,
                    votes: 1,
                    block_sync: true,
                    block: true,
                    missing_block_fetch: false,
                    mark_rebroadcast: true,
                    target_count: 1,
                },
                Case::MissingFetchWithVoteBacking
                | Case::ContiguousNearQuorumBlockSync
                | Case::BlockCreatedWithVoteBacking
                | Case::AnyActionMarks => QuorumRebroadcastDispatchOutput {
                    local_vote: false,
                    votes: 1,
                    block_sync: true,
                    block: true,
                    missing_block_fetch: true,
                    mark_rebroadcast: true,
                    target_count: 1,
                },
                Case::BlockSyncFrameTooLarge
                | Case::BlockSyncLocalOnlyTargets
                | Case::BlockSyncNonSyncPayload
                | Case::NonContiguousNoBlockSync
                | Case::NotNearQuorumNoBlockSync => QuorumRebroadcastDispatchOutput {
                    local_vote: false,
                    votes: 1,
                    block_sync: false,
                    block: true,
                    missing_block_fetch: true,
                    mark_rebroadcast: true,
                    target_count: 1,
                },
            }
        }

        fn observe(self) -> QuorumRebroadcastDispatchOutput {
            const MIN_VOTES_FOR_COMMIT: usize = 3;
            let local_vote =
                !self.drop_pending() && self.topology_non_empty() && self.local_vote_can_emit();
            let vote_count = quorum_rebroadcast_observed_vote_count(
                self.raw_vote_count(),
                local_vote,
                self.vote_count_input(),
            );
            let observed_vote_backing = vote_count > 0;
            let force_full_fanout = quorum_rebroadcast_force_full_repair_fanout(
                self.widen_repair_fanout(),
                self.drop_pending(),
                observed_vote_backing,
                vote_count,
                MIN_VOTES_FOR_COMMIT,
            );
            let target_count = if force_full_fanout {
                MIN_VOTES_FOR_COMMIT
            } else {
                self.paced_target_count()
            };
            let early_exit = self.relay_backpressure()
                || self.initial_target_count() == 0
                || (!force_full_fanout && !self.cooldown_due())
                || (!force_full_fanout && self.target_limit() == 0)
                || target_count == 0;
            let votes = if early_exit { 0 } else { target_count };
            let non_local_target_count = if matches!(self, Self::BlockSyncLocalOnlyTargets) {
                0
            } else {
                target_count
            };
            let block_sync = !early_exit
                && quorum_rebroadcast_should_broadcast_vote_backed_block_sync(
                    self.drop_pending(),
                    target_count,
                    non_local_target_count,
                    observed_vote_backing,
                    self.contiguous_frontier(),
                    MIN_VOTES_FOR_COMMIT,
                    vote_count,
                    self.block_sync_update_available(),
                    self.block_sync_update_fits_frame(),
                );
            let block = !early_exit
                && quorum_rebroadcast_should_broadcast_block_created(
                    self.drop_pending(),
                    target_count,
                    observed_vote_backing,
                );
            let missing_block_fetch = !early_exit
                && quorum_rebroadcast_should_request_missing_commit_qc(
                    self.drop_pending(),
                    target_count,
                    self.has_cached_commit_qc(),
                    observed_vote_backing,
                );
            let mark_rebroadcast = !early_exit
                && quorum_rebroadcast_should_mark_precommit(
                    local_vote,
                    votes,
                    block_sync,
                    block,
                    missing_block_fetch,
                );

            QuorumRebroadcastDispatchOutput {
                local_vote,
                votes,
                block_sync,
                block,
                missing_block_fetch,
                mark_rebroadcast,
                target_count,
            }
        }
    }

    #[test]
    fn quorum_rebroadcast_dispatch_formal_gate_matrix() {
        for case in QuorumRebroadcastDispatchCase::ALL {
            assert_eq!(
                case.observe(),
                case.expected(),
                "quorum rebroadcast dispatch case {} diverged from formal gate",
                case.label()
            );
        }
    }

    #[derive(Clone, Copy, Debug)]
    enum IsolatedVoteBackedHandoffCase {
        DisabledResilience,
        ZeroVotes,
        MultipleVotes,
        AtQuorum,
        StaleHeight,
        FutureHeight,
        CachedCommitQc,
        HappyPath,
        NoSlotAfterSeed,
        WrongSlotHeight,
        WrongSlotView,
        WrongSlotHash,
        MissingBody,
        CommitQcObserved,
        NoVoteBackedOwner,
        RangePullRejected,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct IsolatedVoteBackedHandoffOutput {
        seeds_recovery: bool,
        body_event: bool,
        requests_anchor: bool,
        action: bool,
        reason_ok: bool,
    }

    impl IsolatedVoteBackedHandoffCase {
        const ALL: [Self; 16] = [
            Self::DisabledResilience,
            Self::ZeroVotes,
            Self::MultipleVotes,
            Self::AtQuorum,
            Self::StaleHeight,
            Self::FutureHeight,
            Self::CachedCommitQc,
            Self::HappyPath,
            Self::NoSlotAfterSeed,
            Self::WrongSlotHeight,
            Self::WrongSlotView,
            Self::WrongSlotHash,
            Self::MissingBody,
            Self::CommitQcObserved,
            Self::NoVoteBackedOwner,
            Self::RangePullRejected,
        ];

        fn label(self) -> &'static str {
            match self {
                Self::DisabledResilience => "DisabledResilience",
                Self::ZeroVotes => "ZeroVotes",
                Self::MultipleVotes => "MultipleVotes",
                Self::AtQuorum => "AtQuorum",
                Self::StaleHeight => "StaleHeight",
                Self::FutureHeight => "FutureHeight",
                Self::CachedCommitQc => "CachedCommitQc",
                Self::HappyPath => "HappyPath",
                Self::NoSlotAfterSeed => "NoSlotAfterSeed",
                Self::WrongSlotHeight => "WrongSlotHeight",
                Self::WrongSlotView => "WrongSlotView",
                Self::WrongSlotHash => "WrongSlotHash",
                Self::MissingBody => "MissingBody",
                Self::CommitQcObserved => "CommitQcObserved",
                Self::NoVoteBackedOwner => "NoVoteBackedOwner",
                Self::RangePullRejected => "RangePullRejected",
            }
        }

        fn resilience_enabled(self) -> bool {
            !matches!(self, Self::DisabledResilience)
        }

        fn vote_count(self) -> usize {
            match self {
                Self::ZeroVotes => 0,
                Self::MultipleVotes => 2,
                Self::AtQuorum => 3,
                _ => 1,
            }
        }

        fn height(self) -> u64 {
            match self {
                Self::StaleHeight => 10,
                Self::FutureHeight => 12,
                _ => 11,
            }
        }

        fn cached_commit_qc(self) -> bool {
            matches!(self, Self::CachedCommitQc)
        }

        fn slot_present(self) -> bool {
            !matches!(self, Self::NoSlotAfterSeed)
        }

        fn slot_height_matches(self) -> bool {
            !matches!(self, Self::WrongSlotHeight)
        }

        fn slot_view_matches(self) -> bool {
            !matches!(self, Self::WrongSlotView)
        }

        fn slot_hash_matches(self) -> bool {
            !matches!(self, Self::WrongSlotHash)
        }

        fn body_present(self) -> bool {
            !matches!(self, Self::MissingBody)
        }

        fn commit_qc_observed(self) -> bool {
            matches!(self, Self::CommitQcObserved)
        }

        fn vote_backed_owner_state(self) -> bool {
            !matches!(self, Self::NoVoteBackedOwner)
        }

        fn range_pull_succeeds(self) -> bool {
            !matches!(self, Self::RangePullRejected)
        }

        fn expected(self) -> IsolatedVoteBackedHandoffOutput {
            use IsolatedVoteBackedHandoffCase as Case;
            match self {
                Case::DisabledResilience
                | Case::ZeroVotes
                | Case::MultipleVotes
                | Case::AtQuorum
                | Case::StaleHeight
                | Case::FutureHeight
                | Case::CachedCommitQc => IsolatedVoteBackedHandoffOutput {
                    seeds_recovery: false,
                    body_event: false,
                    requests_anchor: false,
                    action: false,
                    reason_ok: true,
                },
                Case::HappyPath => IsolatedVoteBackedHandoffOutput {
                    seeds_recovery: true,
                    body_event: true,
                    requests_anchor: true,
                    action: true,
                    reason_ok: true,
                },
                Case::NoSlotAfterSeed
                | Case::WrongSlotHeight
                | Case::WrongSlotView
                | Case::WrongSlotHash
                | Case::MissingBody
                | Case::CommitQcObserved
                | Case::NoVoteBackedOwner => IsolatedVoteBackedHandoffOutput {
                    seeds_recovery: true,
                    body_event: true,
                    requests_anchor: false,
                    action: false,
                    reason_ok: true,
                },
                Case::RangePullRejected => IsolatedVoteBackedHandoffOutput {
                    seeds_recovery: true,
                    body_event: true,
                    requests_anchor: true,
                    action: false,
                    reason_ok: true,
                },
            }
        }

        fn observe(self) -> IsolatedVoteBackedHandoffOutput {
            const COMMITTED_HEIGHT: u64 = 10;
            const MIN_VOTES_FOR_COMMIT: usize = 3;

            let admission = isolated_vote_backed_handoff_admission(
                self.resilience_enabled(),
                self.vote_count(),
                MIN_VOTES_FOR_COMMIT,
                self.height(),
                COMMITTED_HEIGHT,
                self.cached_commit_qc(),
            );
            let slot_valid = isolated_vote_backed_handoff_slot_valid(
                self.slot_present(),
                self.slot_height_matches(),
                self.slot_view_matches(),
                self.slot_hash_matches(),
                self.body_present(),
                self.commit_qc_observed(),
                self.vote_backed_owner_state(),
            );
            let requests_anchor =
                isolated_vote_backed_handoff_requests_anchor(admission, slot_valid);
            let reason_ok =
                isolated_vote_backed_handoff_reason_ok(super::ISOLATED_VOTE_BACKED_HANDOFF_REASON);
            let action = isolated_vote_backed_handoff_action(
                requests_anchor,
                self.range_pull_succeeds(),
                reason_ok,
            );

            IsolatedVoteBackedHandoffOutput {
                seeds_recovery: admission,
                body_event: admission,
                requests_anchor,
                action,
                reason_ok,
            }
        }
    }

    #[test]
    fn isolated_vote_backed_handoff_formal_gate_matrix() {
        for case in IsolatedVoteBackedHandoffCase::ALL {
            assert_eq!(
                case.observe(),
                case.expected(),
                "isolated vote-backed handoff case {} diverged from formal gate",
                case.label()
            );
        }
    }

    #[derive(Clone, Copy, Debug)]
    enum PreemptiveVoteBackedRetransmitCase {
        NoWindow,
        MissingVotes,
        HasQc,
        ValidationInflight,
        MissingLocalData,
        RecoveryBlocked,
        ProgressBeforeWindow,
        ProgressAtTimeout,
        NotDue,
        NoPending,
        NoTargets,
        VoteRosterVotes,
        CommitFallbackBlockSync,
        NoOutput,
        VotesOnly,
        BlockSyncOnly,
        BlockOnly,
        MultiOutput,
        AtQuorumOutput,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct PreemptiveVoteBackedRetransmitOutput {
        candidate: bool,
        selected_source: PreemptiveVoteBackedRetransmitTargetSource,
        action: bool,
        pending_after: bool,
        near_quorum_flag: Option<bool>,
    }

    impl PreemptiveVoteBackedRetransmitCase {
        const ALL: [Self; 19] = [
            Self::NoWindow,
            Self::MissingVotes,
            Self::HasQc,
            Self::ValidationInflight,
            Self::MissingLocalData,
            Self::RecoveryBlocked,
            Self::ProgressBeforeWindow,
            Self::ProgressAtTimeout,
            Self::NotDue,
            Self::NoPending,
            Self::NoTargets,
            Self::VoteRosterVotes,
            Self::CommitFallbackBlockSync,
            Self::NoOutput,
            Self::VotesOnly,
            Self::BlockSyncOnly,
            Self::BlockOnly,
            Self::MultiOutput,
            Self::AtQuorumOutput,
        ];

        fn label(self) -> &'static str {
            match self {
                Self::NoWindow => "NoWindow",
                Self::MissingVotes => "MissingVotes",
                Self::HasQc => "HasQc",
                Self::ValidationInflight => "ValidationInflight",
                Self::MissingLocalData => "MissingLocalData",
                Self::RecoveryBlocked => "RecoveryBlocked",
                Self::ProgressBeforeWindow => "ProgressBeforeWindow",
                Self::ProgressAtTimeout => "ProgressAtTimeout",
                Self::NotDue => "NotDue",
                Self::NoPending => "NoPending",
                Self::NoTargets => "NoTargets",
                Self::VoteRosterVotes => "VoteRosterVotes",
                Self::CommitFallbackBlockSync => "CommitFallbackBlockSync",
                Self::NoOutput => "NoOutput",
                Self::VotesOnly => "VotesOnly",
                Self::BlockSyncOnly => "BlockSyncOnly",
                Self::BlockOnly => "BlockOnly",
                Self::MultiOutput => "MultiOutput",
                Self::AtQuorumOutput => "AtQuorumOutput",
            }
        }

        fn pending_before(self) -> bool {
            !matches!(self, Self::NoPending)
        }

        fn resend_window_available(self) -> bool {
            !matches!(self, Self::NoWindow)
        }

        fn has_votes(self) -> bool {
            !matches!(self, Self::MissingVotes)
        }

        fn has_qc(self) -> bool {
            matches!(self, Self::HasQc)
        }

        fn validation_inflight(self) -> bool {
            matches!(self, Self::ValidationInflight)
        }

        fn missing_local_data(self) -> bool {
            matches!(self, Self::MissingLocalData)
        }

        fn allowed_under_recovery(self) -> bool {
            !matches!(self, Self::RecoveryBlocked)
        }

        fn progress_stall_age(self) -> Duration {
            match self {
                Self::ProgressBeforeWindow => Duration::from_millis(4),
                Self::ProgressAtTimeout => Duration::from_millis(10),
                _ => Duration::from_millis(5),
            }
        }

        fn due(self) -> bool {
            !matches!(self, Self::NotDue)
        }

        fn vote_targets_available(self) -> bool {
            !matches!(self, Self::CommitFallbackBlockSync | Self::NoTargets)
        }

        fn commit_targets_available(self) -> bool {
            !matches!(self, Self::NoTargets)
        }

        fn downstream_votes(self) -> bool {
            !matches!(
                self,
                Self::CommitFallbackBlockSync
                    | Self::NoOutput
                    | Self::BlockSyncOnly
                    | Self::BlockOnly
                    | Self::AtQuorumOutput
            )
        }

        fn downstream_block_sync(self) -> bool {
            matches!(
                self,
                Self::CommitFallbackBlockSync | Self::BlockSyncOnly | Self::MultiOutput
            )
        }

        fn downstream_block(self) -> bool {
            matches!(
                self,
                Self::BlockOnly | Self::MultiOutput | Self::AtQuorumOutput
            )
        }

        fn vote_count(self) -> usize {
            match self {
                Self::MissingVotes => 0,
                Self::AtQuorumOutput => 3,
                _ => 1,
            }
        }

        fn expected(self) -> PreemptiveVoteBackedRetransmitOutput {
            use PreemptiveVoteBackedRetransmitCase as Case;
            let none = PreemptiveVoteBackedRetransmitTargetSource::NoSource;
            let vote_roster = PreemptiveVoteBackedRetransmitTargetSource::VoteRoster;
            let commit_topology = PreemptiveVoteBackedRetransmitTargetSource::CommitTopology;
            match self {
                Case::NoWindow
                | Case::MissingVotes
                | Case::HasQc
                | Case::ValidationInflight
                | Case::MissingLocalData
                | Case::RecoveryBlocked
                | Case::ProgressBeforeWindow
                | Case::ProgressAtTimeout
                | Case::NotDue => PreemptiveVoteBackedRetransmitOutput {
                    candidate: false,
                    selected_source: none,
                    action: false,
                    pending_after: true,
                    near_quorum_flag: None,
                },
                Case::NoPending => PreemptiveVoteBackedRetransmitOutput {
                    candidate: true,
                    selected_source: none,
                    action: false,
                    pending_after: false,
                    near_quorum_flag: None,
                },
                Case::NoTargets => PreemptiveVoteBackedRetransmitOutput {
                    candidate: true,
                    selected_source: none,
                    action: false,
                    pending_after: true,
                    near_quorum_flag: None,
                },
                Case::CommitFallbackBlockSync => PreemptiveVoteBackedRetransmitOutput {
                    candidate: true,
                    selected_source: commit_topology,
                    action: true,
                    pending_after: true,
                    near_quorum_flag: Some(true),
                },
                Case::NoOutput => PreemptiveVoteBackedRetransmitOutput {
                    candidate: true,
                    selected_source: vote_roster,
                    action: false,
                    pending_after: true,
                    near_quorum_flag: Some(true),
                },
                Case::AtQuorumOutput => PreemptiveVoteBackedRetransmitOutput {
                    candidate: true,
                    selected_source: vote_roster,
                    action: true,
                    pending_after: true,
                    near_quorum_flag: Some(false),
                },
                Case::VoteRosterVotes
                | Case::VotesOnly
                | Case::BlockSyncOnly
                | Case::BlockOnly
                | Case::MultiOutput => PreemptiveVoteBackedRetransmitOutput {
                    candidate: true,
                    selected_source: vote_roster,
                    action: true,
                    pending_after: true,
                    near_quorum_flag: Some(true),
                },
            }
        }

        fn observe(self) -> PreemptiveVoteBackedRetransmitOutput {
            const MIN_VOTES_FOR_COMMIT: usize = 3;
            let candidate = preemptive_vote_backed_retransmit_candidate(
                self.resend_window_available(),
                self.has_votes(),
                self.has_qc(),
                self.validation_inflight(),
                self.missing_local_data(),
                self.allowed_under_recovery(),
                self.progress_stall_age(),
                Duration::from_millis(5),
                Duration::from_millis(10),
                self.due(),
            );
            let selected_source = preemptive_vote_backed_retransmit_target_source(
                candidate,
                self.pending_before(),
                self.vote_targets_available(),
                self.commit_targets_available(),
            );
            let rebroadcast =
                selected_source != PreemptiveVoteBackedRetransmitTargetSource::NoSource;
            let action = rebroadcast
                && preemptive_vote_backed_retransmit_action(
                    usize::from(self.downstream_votes()),
                    self.downstream_block_sync(),
                    self.downstream_block(),
                );
            let near_quorum_flag = rebroadcast.then(|| {
                preemptive_vote_backed_retransmit_widen_fanout(
                    self.vote_count(),
                    MIN_VOTES_FOR_COMMIT,
                )
            });

            PreemptiveVoteBackedRetransmitOutput {
                candidate,
                selected_source,
                action,
                pending_after: self.pending_before(),
                near_quorum_flag,
            }
        }
    }

    #[test]
    fn preemptive_vote_backed_retransmit_formal_gate_matrix() {
        for case in PreemptiveVoteBackedRetransmitCase::ALL {
            assert_eq!(
                case.observe(),
                case.expected(),
                "preemptive vote-backed retransmit case {} diverged from formal gate",
                case.label()
            );
        }
    }

    #[derive(Clone, Copy, Debug)]
    enum NearQuorumPreemptiveEscalationCase {
        NoCandidates,
        BudgetExhausted,
        MissingPending,
        FreshRequestSuppresses,
        RequestHeightMismatch,
        RequestViewMismatch,
        RequestNotActionable,
        RequestBoundaryStale,
        RequestStale,
        RequestWindowZeroFresh,
        RequestCapBoundaryStale,
        InflightSuppresses,
        InflightHashMismatch,
        InflightViewMismatch,
        InflightNotInflight,
        InflightBoundaryStale,
        InflightStale,
        InflightTtlZeroFresh,
        DelegateFalse,
        DelegateTrue,
        SecondCandidateIgnored,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct NearQuorumPreemptiveEscalationOutput {
        processed: bool,
        fresh_request_suppresses: bool,
        inflight_suppresses: bool,
        escalated: bool,
        counter: usize,
        progress: bool,
        budget_exhausted: bool,
        second_candidate_processed: bool,
    }

    impl NearQuorumPreemptiveEscalationCase {
        const ALL: [Self; 21] = [
            Self::NoCandidates,
            Self::BudgetExhausted,
            Self::MissingPending,
            Self::FreshRequestSuppresses,
            Self::RequestHeightMismatch,
            Self::RequestViewMismatch,
            Self::RequestNotActionable,
            Self::RequestBoundaryStale,
            Self::RequestStale,
            Self::RequestWindowZeroFresh,
            Self::RequestCapBoundaryStale,
            Self::InflightSuppresses,
            Self::InflightHashMismatch,
            Self::InflightViewMismatch,
            Self::InflightNotInflight,
            Self::InflightBoundaryStale,
            Self::InflightStale,
            Self::InflightTtlZeroFresh,
            Self::DelegateFalse,
            Self::DelegateTrue,
            Self::SecondCandidateIgnored,
        ];

        fn label(self) -> &'static str {
            match self {
                Self::NoCandidates => "NoCandidates",
                Self::BudgetExhausted => "BudgetExhausted",
                Self::MissingPending => "MissingPending",
                Self::FreshRequestSuppresses => "FreshRequestSuppresses",
                Self::RequestHeightMismatch => "RequestHeightMismatch",
                Self::RequestViewMismatch => "RequestViewMismatch",
                Self::RequestNotActionable => "RequestNotActionable",
                Self::RequestBoundaryStale => "RequestBoundaryStale",
                Self::RequestStale => "RequestStale",
                Self::RequestWindowZeroFresh => "RequestWindowZeroFresh",
                Self::RequestCapBoundaryStale => "RequestCapBoundaryStale",
                Self::InflightSuppresses => "InflightSuppresses",
                Self::InflightHashMismatch => "InflightHashMismatch",
                Self::InflightViewMismatch => "InflightViewMismatch",
                Self::InflightNotInflight => "InflightNotInflight",
                Self::InflightBoundaryStale => "InflightBoundaryStale",
                Self::InflightStale => "InflightStale",
                Self::InflightTtlZeroFresh => "InflightTtlZeroFresh",
                Self::DelegateFalse => "DelegateFalse",
                Self::DelegateTrue => "DelegateTrue",
                Self::SecondCandidateIgnored => "SecondCandidateIgnored",
            }
        }

        fn has_candidate(self) -> bool {
            !matches!(self, Self::NoCandidates)
        }

        fn pending_block_exists(self) -> bool {
            !matches!(self, Self::MissingPending)
        }

        fn tick_budget_exhausted(self) -> bool {
            matches!(self, Self::BudgetExhausted)
        }

        fn has_request(self) -> bool {
            matches!(
                self,
                Self::FreshRequestSuppresses
                    | Self::RequestHeightMismatch
                    | Self::RequestViewMismatch
                    | Self::RequestNotActionable
                    | Self::RequestBoundaryStale
                    | Self::RequestStale
                    | Self::RequestWindowZeroFresh
                    | Self::RequestCapBoundaryStale
            )
        }

        fn request_height_matches(self) -> bool {
            !matches!(self, Self::RequestHeightMismatch)
        }

        fn request_view_matches(self) -> bool {
            !matches!(self, Self::RequestViewMismatch)
        }

        fn request_actionable(self) -> bool {
            !matches!(self, Self::RequestNotActionable)
        }

        fn rebroadcast_cooldown(self) -> Duration {
            if matches!(self, Self::RequestCapBoundaryStale) {
                Duration::from_millis(2)
            } else {
                Duration::from_millis(3)
            }
        }

        fn fetch_freshness_cap(self) -> Duration {
            (self.rebroadcast_cooldown() * 2).max(Duration::from_millis(1))
        }

        fn request_retry_window(self) -> Duration {
            match self {
                Self::RequestWindowZeroFresh => Duration::ZERO,
                Self::RequestCapBoundaryStale => Duration::from_millis(10),
                _ => Duration::from_millis(5),
            }
        }

        fn fresh_request_bound(self) -> Duration {
            self.request_retry_window()
                .max(Duration::from_millis(1))
                .min(self.fetch_freshness_cap())
        }

        fn request_age(self) -> Duration {
            match self {
                Self::RequestBoundaryStale => self.fresh_request_bound(),
                Self::RequestStale => self.fresh_request_bound() + Duration::from_millis(2),
                Self::RequestWindowZeroFresh => Duration::ZERO,
                Self::RequestCapBoundaryStale => self.fetch_freshness_cap(),
                _ => Duration::from_millis(2),
            }
        }

        fn has_recovery_budget(self) -> bool {
            matches!(
                self,
                Self::InflightSuppresses
                    | Self::InflightHashMismatch
                    | Self::InflightViewMismatch
                    | Self::InflightNotInflight
                    | Self::InflightBoundaryStale
                    | Self::InflightStale
                    | Self::InflightTtlZeroFresh
            )
        }

        fn recovery_hash_matches(self) -> bool {
            !matches!(self, Self::InflightHashMismatch)
        }

        fn recovery_view_matches(self) -> bool {
            !matches!(self, Self::InflightViewMismatch)
        }

        fn range_pull_inflight(self) -> bool {
            !matches!(self, Self::InflightNotInflight)
        }

        fn recovery_ttl(self) -> Duration {
            if matches!(self, Self::InflightTtlZeroFresh) {
                Duration::ZERO
            } else {
                Duration::from_millis(5)
            }
        }

        fn fresh_inflight_bound(self) -> Duration {
            self.recovery_ttl().max(Duration::from_millis(1))
        }

        fn inflight_age(self) -> Duration {
            match self {
                Self::InflightBoundaryStale => self.fresh_inflight_bound(),
                Self::InflightStale => self.fresh_inflight_bound() + Duration::from_millis(2),
                Self::InflightTtlZeroFresh => Duration::ZERO,
                _ => Duration::from_millis(2),
            }
        }

        fn delegate_returns(self) -> bool {
            !matches!(self, Self::DelegateFalse | Self::SecondCandidateIgnored)
        }

        fn expected(self) -> NearQuorumPreemptiveEscalationOutput {
            use NearQuorumPreemptiveEscalationCase as Case;
            match self {
                Case::NoCandidates | Case::BudgetExhausted | Case::MissingPending => {
                    NearQuorumPreemptiveEscalationOutput {
                        processed: false,
                        fresh_request_suppresses: false,
                        inflight_suppresses: false,
                        escalated: false,
                        counter: 0,
                        progress: false,
                        budget_exhausted: matches!(self, Case::BudgetExhausted),
                        second_candidate_processed: false,
                    }
                }
                Case::FreshRequestSuppresses | Case::RequestWindowZeroFresh => {
                    NearQuorumPreemptiveEscalationOutput {
                        processed: true,
                        fresh_request_suppresses: true,
                        inflight_suppresses: false,
                        escalated: false,
                        counter: 0,
                        progress: false,
                        budget_exhausted: false,
                        second_candidate_processed: false,
                    }
                }
                Case::InflightSuppresses | Case::InflightTtlZeroFresh => {
                    NearQuorumPreemptiveEscalationOutput {
                        processed: true,
                        fresh_request_suppresses: false,
                        inflight_suppresses: true,
                        escalated: false,
                        counter: 0,
                        progress: false,
                        budget_exhausted: false,
                        second_candidate_processed: false,
                    }
                }
                Case::DelegateFalse | Case::SecondCandidateIgnored => {
                    NearQuorumPreemptiveEscalationOutput {
                        processed: true,
                        fresh_request_suppresses: false,
                        inflight_suppresses: false,
                        escalated: false,
                        counter: 0,
                        progress: false,
                        budget_exhausted: false,
                        second_candidate_processed: false,
                    }
                }
                Case::RequestHeightMismatch
                | Case::RequestViewMismatch
                | Case::RequestNotActionable
                | Case::RequestBoundaryStale
                | Case::RequestStale
                | Case::RequestCapBoundaryStale
                | Case::InflightHashMismatch
                | Case::InflightViewMismatch
                | Case::InflightNotInflight
                | Case::InflightBoundaryStale
                | Case::InflightStale
                | Case::DelegateTrue => NearQuorumPreemptiveEscalationOutput {
                    processed: true,
                    fresh_request_suppresses: false,
                    inflight_suppresses: false,
                    escalated: true,
                    counter: 1,
                    progress: true,
                    budget_exhausted: false,
                    second_candidate_processed: false,
                },
            }
        }

        fn observe(self) -> NearQuorumPreemptiveEscalationOutput {
            let budget_exhausted = self.has_candidate() && self.tick_budget_exhausted();
            let processed =
                self.has_candidate() && !budget_exhausted && self.pending_block_exists();
            let fresh_request_suppresses = processed
                && self.has_request()
                && near_quorum_fresh_missing_block_request_suppresses(
                    self.request_height_matches(),
                    self.request_view_matches(),
                    self.request_actionable(),
                    self.request_age(),
                    self.request_retry_window(),
                    self.fetch_freshness_cap(),
                );
            let inflight_suppresses = processed
                && self.has_recovery_budget()
                && near_quorum_inflight_recovery_suppresses(
                    self.recovery_hash_matches(),
                    self.recovery_view_matches(),
                    self.range_pull_inflight(),
                    self.inflight_age(),
                    self.recovery_ttl(),
                );
            let escalated = processed
                && !fresh_request_suppresses
                && !inflight_suppresses
                && self.delegate_returns();
            let counter = usize::from(escalated);
            let progress = escalated;
            let second_candidate_processed = false;

            NearQuorumPreemptiveEscalationOutput {
                processed,
                fresh_request_suppresses,
                inflight_suppresses,
                escalated,
                counter,
                progress,
                budget_exhausted,
                second_candidate_processed,
            }
        }
    }

    #[test]
    fn near_quorum_preemptive_escalation_formal_gate_matrix() {
        assert_eq!(NEAR_QUORUM_PREEMPTIVE_RECOVERY_PER_TICK, 1);
        for case in NearQuorumPreemptiveEscalationCase::ALL {
            assert_eq!(
                case.observe(),
                case.expected(),
                "near-quorum preemptive escalation case {} diverged from formal gate",
                case.label()
            );
        }
    }

    #[test]
    fn vote_backed_fast_resend_is_disabled_by_consensus_backlog() {
        let window = contiguous_frontier_vote_backed_fast_resend_window(
            Duration::from_millis(25),
            true,
            1,
            13,
            false,
            true,
            false,
        );

        assert_eq!(window, None);
    }
}
