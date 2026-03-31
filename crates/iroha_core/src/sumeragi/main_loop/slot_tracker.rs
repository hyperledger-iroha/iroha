//! Same-height slot ownership tracking for `committed + 1`.

use std::{
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet},
    time::{Duration, Instant},
};

use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SlotOwnerKind {
    ProposalLed,
    BlockCreatedLed,
    ExactSlotRepair,
    CommittedEdgeSuppression,
    PassiveRetainedPayload,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FrontierBodyFetchStage {
    Leader,
    Voters,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FrontierSlotMode {
    Normal,
    DeepCatchup,
    PassiveCatchup,
    Finalized,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FrontierSlotPhase {
    AwaitBlockCreated,
    AwaitBody,
    ValidateBody,
    AwaitCommitQc,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FrontierBodyState {
    Missing,
    Available,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FrontierValidationState {
    Unknown,
    Pending,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FrontierVoteState {
    None,
    VotesObserved,
    CommitQcObserved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FrontierOwnerLockState {
    Unlocked,
    LocallyVoted,
}

#[derive(Debug, Clone)]
pub(super) struct FrontierCandidate {
    pub(super) view: u64,
    pub(super) block_hash: HashOf<BlockHeader>,
    pub(super) frontier_info: Option<super::message::BlockCreatedFrontierInfo>,
    pub(super) leader: Option<PeerId>,
    pub(super) voters: BTreeSet<PeerId>,
    pub(super) body_state: FrontierBodyState,
    pub(super) validation_state: FrontierValidationState,
    pub(super) vote_state: FrontierVoteState,
    pub(super) block_created_seen: bool,
    pub(super) exact_fetch_armed: bool,
}

#[derive(Debug, Clone, Copy, Default)]
pub(super) struct FrontierQuorumProgress {
    pub(super) votes_observed: bool,
    pub(super) commit_qc_observed: bool,
    pub(super) last_vote_at: Option<Instant>,
    pub(super) last_commit_qc_at: Option<Instant>,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct FrontierTimers {
    pub(super) observed_at: Instant,
    pub(super) last_updated_at: Instant,
    pub(super) last_progress_at: Instant,
    pub(super) last_fetch_at: Option<Instant>,
    pub(super) lag_window_started_at: Option<Instant>,
    pub(super) last_view_advance_at: Option<Instant>,
    pub(super) deep_catchup_entered_at: Option<Instant>,
}

#[derive(Debug, Clone)]
pub(super) struct FrontierRepairState {
    pub(super) fetch_stage: FrontierBodyFetchStage,
    pub(super) retry_window: Duration,
    pub(super) pending_requesters: BTreeSet<PeerId>,
    pub(super) last_reason: Option<&'static str>,
    pub(super) quorum_timeout_rebroadcasted: bool,
    pub(super) deep_catchup_reason: Option<&'static str>,
}

#[derive(Debug, Clone)]
pub(super) enum FrontierSlotEvent {
    OnBlockCreated {
        block_hash: HashOf<BlockHeader>,
        view: u64,
        frontier_info: Option<super::message::BlockCreatedFrontierInfo>,
        leader: Option<PeerId>,
        voters: BTreeSet<PeerId>,
        body_present: bool,
        requester: Option<PeerId>,
    },
    OnBodyAvailable {
        block_hash: HashOf<BlockHeader>,
        view: u64,
        sender: Option<PeerId>,
    },
    OnVoteObserved {
        block_hash: HashOf<BlockHeader>,
        view: u64,
        voter: Option<PeerId>,
    },
    OnCommitQcObserved {
        block_hash: HashOf<BlockHeader>,
        view: u64,
    },
    OnAuthoritativeSupersede {
        block_hash: HashOf<BlockHeader>,
        view: u64,
        frontier_info: Option<super::message::BlockCreatedFrontierInfo>,
        leader: Option<PeerId>,
        voters: BTreeSet<PeerId>,
        body_present: bool,
        requester: Option<PeerId>,
    },
    OnFutureGapObserved {
        block_hash: HashOf<BlockHeader>,
        view: u64,
        leader: Option<PeerId>,
        voters: BTreeSet<PeerId>,
        exact_fetch_armed: bool,
        requester: Option<PeerId>,
    },
    OnFetchRetryDue,
    OnQuorumTimeout {
        cause: ViewChangeCause,
        requested_view: u64,
    },
    OnLagWindowExpired {
        reason: &'static str,
    },
    OnViewAdvanceRequested {
        cause: ViewChangeCause,
        requested_view: u64,
    },
    OnCommittedHeightAdvanced {
        committed_height: u64,
    },
}

#[derive(Debug, Clone, Copy, Default)]
pub(super) struct FrontierSlotActions {
    pub(super) fetch_block_body: bool,
    pub(super) enter_deep_catchup: Option<&'static str>,
    pub(super) request_view_change: Option<(u64, u64, ViewChangeCause)>,
    pub(super) retire_slot: bool,
}

#[derive(Debug)]
pub(super) struct FrontierSlot {
    pub(super) height: u64,
    pub(super) active_view: u64,
    pub(super) owner_generation: u64,
    pub(super) lock_state: FrontierOwnerLockState,
    pub(super) owner_kind: SlotOwnerKind,
    pub(super) mode: FrontierSlotMode,
    pub(super) phase: FrontierSlotPhase,
    pub(super) candidate: FrontierCandidate,
    pub(super) quorum_progress: FrontierQuorumProgress,
    pub(super) timers: FrontierTimers,
    pub(super) repair_state: FrontierRepairState,
    // TODO: remove these compatibility mirrors once the remaining slot helpers read the nested
    // FSM fields directly instead of the legacy flat fields.
    pub(super) view: u64,
    pub(super) block_hash: HashOf<BlockHeader>,
    pub(super) observed_at: Instant,
    pub(super) last_updated_at: Instant,
    pub(super) body_present: bool,
    pub(super) block_created_seen: bool,
    pub(super) exact_fetch_armed: bool,
    pub(super) frontier_info: Option<super::message::BlockCreatedFrontierInfo>,
    pub(super) leader: Option<PeerId>,
    pub(super) voters: BTreeSet<PeerId>,
    pub(super) fetch_stage: FrontierBodyFetchStage,
    pub(super) last_fetch_at: Option<Instant>,
    pub(super) retry_window: Duration,
    pub(super) pending_requesters: BTreeSet<PeerId>,
}

impl FrontierSlot {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new(
        height: u64,
        view: u64,
        block_hash: HashOf<BlockHeader>,
        now: Instant,
        retry_window: Duration,
        leader: Option<PeerId>,
        voters: BTreeSet<PeerId>,
        block_created_seen: bool,
        exact_fetch_armed: bool,
        body_present: bool,
        frontier_info: Option<super::message::BlockCreatedFrontierInfo>,
        requester: Option<PeerId>,
    ) -> Self {
        let mut pending_requesters = BTreeSet::new();
        if let Some(requester) = requester {
            pending_requesters.insert(requester);
        }
        let candidate = FrontierCandidate {
            view,
            block_hash,
            frontier_info: frontier_info.clone(),
            leader: leader.clone(),
            voters: voters.clone(),
            body_state: if body_present {
                FrontierBodyState::Available
            } else {
                FrontierBodyState::Missing
            },
            validation_state: if body_present {
                FrontierValidationState::Pending
            } else {
                FrontierValidationState::Unknown
            },
            vote_state: FrontierVoteState::None,
            block_created_seen,
            exact_fetch_armed,
        };
        let timers = FrontierTimers {
            observed_at: now,
            last_updated_at: now,
            last_progress_at: now,
            last_fetch_at: None,
            lag_window_started_at: (!body_present).then_some(now),
            last_view_advance_at: None,
            deep_catchup_entered_at: None,
        };
        let repair_state = FrontierRepairState {
            fetch_stage: FrontierBodyFetchStage::Leader,
            retry_window,
            pending_requesters,
            last_reason: None,
            quorum_timeout_rebroadcasted: false,
            deep_catchup_reason: None,
        };
        let phase = if body_present {
            FrontierSlotPhase::ValidateBody
        } else if block_created_seen || exact_fetch_armed {
            FrontierSlotPhase::AwaitBody
        } else {
            FrontierSlotPhase::AwaitBlockCreated
        };
        let owner_kind = if block_created_seen {
            SlotOwnerKind::BlockCreatedLed
        } else if exact_fetch_armed || body_present {
            SlotOwnerKind::ExactSlotRepair
        } else {
            SlotOwnerKind::ProposalLed
        };
        let mut slot = Self {
            height,
            active_view: view,
            owner_generation: 0,
            lock_state: FrontierOwnerLockState::Unlocked,
            owner_kind,
            mode: FrontierSlotMode::Normal,
            phase,
            candidate,
            quorum_progress: FrontierQuorumProgress::default(),
            timers,
            repair_state,
            view,
            block_hash,
            observed_at: now,
            last_updated_at: now,
            body_present,
            block_created_seen,
            exact_fetch_armed,
            frontier_info,
            leader,
            voters,
            fetch_stage: FrontierBodyFetchStage::Leader,
            last_fetch_at: None,
            retry_window,
            pending_requesters: BTreeSet::new(),
        };
        slot.sync_compat_fields();
        slot
    }

    pub(super) fn lag_started_at(&self) -> Instant {
        self.timers
            .lag_window_started_at
            .unwrap_or(self.timers.last_progress_at)
    }

    pub(super) fn body_missing(&self) -> bool {
        matches!(self.candidate.body_state, FrontierBodyState::Missing)
    }

    pub(super) fn note_local_vote_emitted(&mut self) {
        self.lock_state = FrontierOwnerLockState::LocallyVoted;
    }

    fn matches_candidate(&self, block_hash: HashOf<BlockHeader>, view: u64) -> bool {
        self.candidate.block_hash == block_hash && self.candidate.view == view
    }

    fn current_view_for_timeout(&self, requested_view: u64) -> u64 {
        self.active_view
            .max(requested_view)
            .max(self.candidate.view)
    }

    fn record_progress(&mut self, now: Instant) {
        self.timers.last_progress_at = now;
        self.timers.last_updated_at = now;
        self.timers.lag_window_started_at = None;
        self.repair_state.quorum_timeout_rebroadcasted = false;
    }

    pub(super) fn note_lag_if_needed(&mut self, now: Instant) {
        if self.timers.lag_window_started_at.is_none() {
            self.timers.lag_window_started_at = Some(now);
        }
    }

    pub(super) fn mark_deep_catchup(&mut self, now: Instant, reason: &'static str) {
        self.owner_kind = SlotOwnerKind::ExactSlotRepair;
        self.mode = FrontierSlotMode::DeepCatchup;
        self.repair_state.deep_catchup_reason = Some(reason);
        self.repair_state.last_reason = Some(reason);
        self.repair_state.quorum_timeout_rebroadcasted = false;
        self.timers.deep_catchup_entered_at = Some(now);
        self.timers.last_updated_at = now;
    }

    pub(super) fn mark_passive_catchup(&mut self, now: Instant, reason: &'static str) {
        self.owner_kind = SlotOwnerKind::PassiveRetainedPayload;
        self.mode = FrontierSlotMode::PassiveCatchup;
        self.repair_state.deep_catchup_reason = Some(reason);
        self.repair_state.last_reason = Some(reason);
        self.repair_state.quorum_timeout_rebroadcasted = false;
        self.timers.deep_catchup_entered_at = Some(now);
        self.timers.last_updated_at = now;
    }

    pub(super) fn sync_compat_fields(&mut self) {
        self.view = self.candidate.view;
        self.block_hash = self.candidate.block_hash;
        self.observed_at = self.timers.observed_at;
        self.last_updated_at = self.timers.last_updated_at;
        self.body_present = matches!(self.candidate.body_state, FrontierBodyState::Available);
        self.block_created_seen = self.candidate.block_created_seen;
        self.exact_fetch_armed = self.candidate.exact_fetch_armed;
        self.frontier_info = self.candidate.frontier_info.clone();
        self.leader = self.candidate.leader.clone();
        self.voters = self.candidate.voters.clone();
        self.fetch_stage = self.repair_state.fetch_stage;
        self.last_fetch_at = self.timers.last_fetch_at;
        self.retry_window = self.repair_state.retry_window;
        self.pending_requesters = self.repair_state.pending_requesters.clone();
    }

    pub(super) fn step(
        &mut self,
        now: Instant,
        event: FrontierSlotEvent,
        lag_window: Duration,
    ) -> FrontierSlotActions {
        let mut actions = FrontierSlotActions::default();
        match event {
            FrontierSlotEvent::OnBlockCreated {
                block_hash,
                view,
                frontier_info,
                leader,
                voters,
                body_present,
                requester,
            } => {
                let higher_view = view > self.candidate.view;
                let same_candidate = self.matches_candidate(block_hash, view);
                if higher_view {
                    let owner_changed =
                        self.candidate.block_hash != block_hash || self.candidate.view != view;
                    self.candidate = FrontierCandidate {
                        view,
                        block_hash,
                        frontier_info: frontier_info.clone(),
                        leader: leader.clone(),
                        voters: voters.clone(),
                        body_state: if body_present {
                            FrontierBodyState::Available
                        } else {
                            FrontierBodyState::Missing
                        },
                        validation_state: if body_present {
                            FrontierValidationState::Pending
                        } else {
                            FrontierValidationState::Unknown
                        },
                        vote_state: FrontierVoteState::None,
                        block_created_seen: true,
                        exact_fetch_armed: true,
                    };
                    if owner_changed {
                        self.owner_generation = self.owner_generation.saturating_add(1);
                        self.lock_state = FrontierOwnerLockState::Unlocked;
                    }
                    self.owner_kind = SlotOwnerKind::BlockCreatedLed;
                    self.active_view = self.active_view.max(view);
                    self.phase = if body_present {
                        FrontierSlotPhase::ValidateBody
                    } else {
                        FrontierSlotPhase::AwaitBody
                    };
                    self.quorum_progress = FrontierQuorumProgress::default();
                    self.repair_state.fetch_stage = FrontierBodyFetchStage::Leader;
                    self.repair_state.quorum_timeout_rebroadcasted = false;
                    self.timers.observed_at = now;
                    self.record_progress(now);
                } else if same_candidate {
                    self.owner_kind = SlotOwnerKind::BlockCreatedLed;
                    self.candidate.block_created_seen = true;
                    self.candidate.exact_fetch_armed = true;
                    if let Some(frontier_info) = frontier_info {
                        self.candidate.frontier_info = Some(frontier_info);
                    }
                    if let Some(leader) = leader {
                        self.candidate.leader = Some(leader);
                    }
                    self.candidate.voters.extend(voters);
                    if body_present {
                        self.candidate.body_state = FrontierBodyState::Available;
                        self.candidate.validation_state = FrontierValidationState::Pending;
                        self.phase = FrontierSlotPhase::ValidateBody;
                        self.record_progress(now);
                    } else {
                        self.phase = FrontierSlotPhase::AwaitBody;
                        self.note_lag_if_needed(now);
                    }
                } else {
                    self.sync_compat_fields();
                    return actions;
                }
                if let Some(requester) = requester {
                    self.repair_state.pending_requesters.insert(requester);
                }
                self.mode = match self.mode {
                    FrontierSlotMode::Finalized => FrontierSlotMode::Normal,
                    mode => mode,
                };
            }
            FrontierSlotEvent::OnBodyAvailable {
                block_hash,
                view,
                sender,
            } => {
                if !self.matches_candidate(block_hash, view) {
                    self.sync_compat_fields();
                    return actions;
                }
                if let Some(sender) = sender {
                    self.candidate.voters.insert(sender);
                }
                self.owner_kind = SlotOwnerKind::ExactSlotRepair;
                self.candidate.body_state = FrontierBodyState::Available;
                if matches!(
                    self.candidate.validation_state,
                    FrontierValidationState::Unknown
                ) {
                    self.candidate.validation_state = FrontierValidationState::Pending;
                }
                if matches!(self.mode, FrontierSlotMode::PassiveCatchup) {
                    self.mode = FrontierSlotMode::Normal;
                }
                self.phase = FrontierSlotPhase::ValidateBody;
                self.record_progress(now);
            }
            FrontierSlotEvent::OnVoteObserved {
                block_hash,
                view,
                voter,
            } => {
                if self.matches_candidate(block_hash, view) {
                    self.owner_kind = SlotOwnerKind::ExactSlotRepair;
                    if let Some(voter) = voter {
                        self.candidate.voters.insert(voter);
                    }
                    self.candidate.vote_state = FrontierVoteState::VotesObserved;
                    self.quorum_progress.votes_observed = true;
                    self.quorum_progress.last_vote_at = Some(now);
                    self.phase = FrontierSlotPhase::AwaitCommitQc;
                    self.record_progress(now);
                } else if view >= self.candidate.view {
                    let owner_changed =
                        self.candidate.block_hash != block_hash || self.candidate.view != view;
                    self.owner_kind = SlotOwnerKind::ExactSlotRepair;
                    self.candidate.view = view;
                    self.candidate.block_hash = block_hash;
                    self.candidate.vote_state = FrontierVoteState::VotesObserved;
                    self.candidate.body_state = FrontierBodyState::Missing;
                    self.candidate.validation_state = FrontierValidationState::Unknown;
                    self.candidate.block_created_seen = false;
                    self.candidate.exact_fetch_armed = true;
                    if let Some(voter) = voter {
                        self.candidate.voters.insert(voter);
                    }
                    if owner_changed {
                        self.owner_generation = self.owner_generation.saturating_add(1);
                        self.lock_state = FrontierOwnerLockState::Unlocked;
                    }
                    self.active_view = self.active_view.max(view);
                    self.phase = FrontierSlotPhase::AwaitBody;
                    self.note_lag_if_needed(now);
                }
            }
            FrontierSlotEvent::OnCommitQcObserved { block_hash, view } => {
                if !self.matches_candidate(block_hash, view) {
                    self.sync_compat_fields();
                    return actions;
                }
                self.owner_kind = SlotOwnerKind::ExactSlotRepair;
                self.candidate.vote_state = FrontierVoteState::CommitQcObserved;
                self.quorum_progress.commit_qc_observed = true;
                self.quorum_progress.last_commit_qc_at = Some(now);
                self.phase = FrontierSlotPhase::AwaitCommitQc;
                self.record_progress(now);
            }
            FrontierSlotEvent::OnAuthoritativeSupersede {
                block_hash,
                view,
                frontier_info,
                leader,
                voters,
                body_present,
                requester,
            } => {
                let owner_changed =
                    self.candidate.block_hash != block_hash || self.candidate.view != view;
                self.candidate = FrontierCandidate {
                    view,
                    block_hash,
                    frontier_info: frontier_info.clone(),
                    leader: leader.clone(),
                    voters: voters.clone(),
                    body_state: if body_present {
                        FrontierBodyState::Available
                    } else {
                        FrontierBodyState::Missing
                    },
                    validation_state: if body_present {
                        FrontierValidationState::Pending
                    } else {
                        FrontierValidationState::Unknown
                    },
                    vote_state: FrontierVoteState::None,
                    block_created_seen: true,
                    exact_fetch_armed: true,
                };
                if owner_changed {
                    self.owner_generation = self.owner_generation.saturating_add(1);
                    self.lock_state = FrontierOwnerLockState::Unlocked;
                }
                self.owner_kind = SlotOwnerKind::BlockCreatedLed;
                self.active_view = self.active_view.max(view);
                self.phase = if body_present {
                    FrontierSlotPhase::ValidateBody
                } else {
                    FrontierSlotPhase::AwaitBody
                };
                self.quorum_progress = FrontierQuorumProgress::default();
                self.repair_state.fetch_stage = FrontierBodyFetchStage::Leader;
                self.repair_state.quorum_timeout_rebroadcasted = false;
                self.timers.observed_at = now;
                self.record_progress(now);
                if let Some(requester) = requester {
                    self.repair_state.pending_requesters.insert(requester);
                }
                self.mode = FrontierSlotMode::Normal;
            }
            FrontierSlotEvent::OnFutureGapObserved {
                block_hash,
                view,
                leader,
                voters,
                exact_fetch_armed,
                requester,
            } => {
                let higher_view = view > self.candidate.view;
                let same_candidate = self.matches_candidate(block_hash, view);
                if higher_view {
                    let owner_changed =
                        self.candidate.block_hash != block_hash || self.candidate.view != view;
                    self.candidate = FrontierCandidate {
                        view,
                        block_hash,
                        frontier_info: None,
                        leader: leader.clone(),
                        voters: voters.clone(),
                        body_state: FrontierBodyState::Missing,
                        validation_state: FrontierValidationState::Unknown,
                        vote_state: FrontierVoteState::None,
                        block_created_seen: false,
                        exact_fetch_armed,
                    };
                    if owner_changed {
                        self.owner_generation = self.owner_generation.saturating_add(1);
                        self.lock_state = FrontierOwnerLockState::Unlocked;
                    }
                    self.owner_kind = SlotOwnerKind::ExactSlotRepair;
                    self.active_view = self.active_view.max(view);
                    self.phase = FrontierSlotPhase::AwaitBody;
                    self.quorum_progress = FrontierQuorumProgress::default();
                    self.repair_state.fetch_stage = FrontierBodyFetchStage::Leader;
                    self.repair_state.quorum_timeout_rebroadcasted = false;
                    self.timers.observed_at = now;
                    self.note_lag_if_needed(now);
                } else if same_candidate {
                    self.owner_kind = SlotOwnerKind::ExactSlotRepair;
                    if let Some(leader) = leader {
                        self.candidate.leader = Some(leader);
                    }
                    self.candidate.voters.extend(voters);
                    self.candidate.exact_fetch_armed |= exact_fetch_armed;
                    self.phase = if self.body_missing() {
                        FrontierSlotPhase::AwaitBody
                    } else {
                        self.phase
                    };
                    self.note_lag_if_needed(now);
                } else {
                    self.sync_compat_fields();
                    return actions;
                }
                if let Some(requester) = requester {
                    self.repair_state.pending_requesters.insert(requester);
                }
                if matches!(self.mode, FrontierSlotMode::Normal) {
                    actions.fetch_block_body = self.candidate.exact_fetch_armed;
                }
            }
            FrontierSlotEvent::OnFetchRetryDue => {
                if matches!(self.mode, FrontierSlotMode::Normal)
                    && self.candidate.exact_fetch_armed
                    && self.body_missing()
                {
                    actions.fetch_block_body = true;
                }
            }
            FrontierSlotEvent::OnQuorumTimeout {
                cause,
                requested_view,
            } => {
                let frontier_waiting_for_body = matches!(self.phase, FrontierSlotPhase::AwaitBody)
                    && self.candidate.exact_fetch_armed
                    && self.body_missing();
                let lag_expired = lag_window != Duration::ZERO
                    && now.saturating_duration_since(self.lag_started_at()) >= lag_window;
                if frontier_waiting_for_body && !matches!(self.mode, FrontierSlotMode::DeepCatchup)
                {
                    if lag_expired {
                        self.mark_deep_catchup(now, "frontier_stall_reset");
                        actions.enter_deep_catchup = Some("frontier_stall_reset");
                    } else {
                        actions.fetch_block_body = true;
                    }
                } else if matches!(self.mode, FrontierSlotMode::PassiveCatchup) {
                    self.repair_state.last_reason = Some("frontier_stall_reset");
                } else if matches!(self.mode, FrontierSlotMode::DeepCatchup) {
                    actions.enter_deep_catchup = self.repair_state.deep_catchup_reason;
                } else {
                    if self.repair_state.quorum_timeout_rebroadcasted {
                        self.owner_kind = SlotOwnerKind::ExactSlotRepair;
                        self.active_view = self
                            .current_view_for_timeout(requested_view)
                            .saturating_add(1);
                        self.timers.last_view_advance_at = Some(now);
                        self.timers.last_updated_at = now;
                        self.repair_state.quorum_timeout_rebroadcasted = false;
                        actions.request_view_change = Some((
                            self.height,
                            self.current_view_for_timeout(requested_view),
                            cause,
                        ));
                    } else {
                        self.repair_state.quorum_timeout_rebroadcasted = true;
                        if frontier_waiting_for_body {
                            actions.fetch_block_body = true;
                        }
                    }
                }
            }
            FrontierSlotEvent::OnLagWindowExpired { reason } => {
                if matches!(self.mode, FrontierSlotMode::Normal)
                    && self.candidate.exact_fetch_armed
                    && self.body_missing()
                {
                    self.mark_deep_catchup(now, reason);
                    actions.enter_deep_catchup = Some(reason);
                } else if matches!(self.mode, FrontierSlotMode::PassiveCatchup) {
                    self.repair_state.last_reason = Some(reason);
                } else if matches!(self.mode, FrontierSlotMode::DeepCatchup) {
                    actions.enter_deep_catchup = self.repair_state.deep_catchup_reason;
                }
            }
            FrontierSlotEvent::OnViewAdvanceRequested {
                cause,
                requested_view,
            } => {
                self.owner_kind = SlotOwnerKind::ExactSlotRepair;
                let current_view = self.current_view_for_timeout(requested_view);
                self.active_view = self
                    .current_view_for_timeout(requested_view)
                    .saturating_add(1);
                self.timers.last_view_advance_at = Some(now);
                self.timers.last_updated_at = now;
                self.repair_state.quorum_timeout_rebroadcasted = false;
                actions.request_view_change = Some((self.height, current_view, cause));
            }
            FrontierSlotEvent::OnCommittedHeightAdvanced { committed_height } => {
                if committed_height >= self.height {
                    self.mode = FrontierSlotMode::Finalized;
                    actions.retire_slot = true;
                }
            }
        }

        self.sync_compat_fields();
        actions
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct FrontierPrefetchSlot {
    pub(super) height: u64,
    pub(super) view: u64,
    pub(super) block_hash: HashOf<BlockHeader>,
}

#[derive(Debug, Clone)]
pub(super) struct RetainedBranchState {
    pub(super) height: u64,
    pub(super) view: u64,
    pub(super) block_hash: HashOf<BlockHeader>,
    pub(super) frontier_info: Option<super::message::BlockCreatedFrontierInfo>,
    pub(super) payload_present: bool,
    pub(super) last_refreshed_at: Instant,
}

impl RetainedBranchState {
    fn cmp_priority(&self, other: &Self) -> Ordering {
        self.view
            .cmp(&other.view)
            .then_with(|| self.payload_present.cmp(&other.payload_present))
            .then_with(|| self.last_refreshed_at.cmp(&other.last_refreshed_at))
    }
}

#[derive(Debug, Default)]
pub(super) struct SlotTrackerState {
    pub(super) authoritative_block_slots: BTreeMap<(u64, u64), HashOf<BlockHeader>>,
    pub(super) authoritative_block_frontiers:
        BTreeMap<(u64, u64, HashOf<BlockHeader>), super::message::BlockCreatedFrontierInfo>,
    pub(super) proposals_seen: BTreeSet<(u64, u64)>,
    pub(super) retained_branches: BTreeMap<(u64, u64, HashOf<BlockHeader>), RetainedBranchState>,
}

impl SlotTrackerState {
    pub(super) fn clear(&mut self) {
        self.authoritative_block_slots.clear();
        self.authoritative_block_frontiers.clear();
        self.proposals_seen.clear();
        self.retained_branches.clear();
    }

    pub(super) fn authoritative_slot_owner_hash(
        &self,
        height: u64,
        view: u64,
    ) -> Option<HashOf<BlockHeader>> {
        self.authoritative_block_slots.get(&(height, view)).copied()
    }

    pub(super) fn note_authoritative_slot_owner(
        &mut self,
        height: u64,
        view: u64,
        block_hash: HashOf<BlockHeader>,
    ) {
        self.authoritative_block_slots
            .insert((height, view), block_hash);
        self.authoritative_block_frontiers
            .retain(|(entry_height, entry_view, entry_hash), _| {
                *entry_height != height || *entry_view != view || *entry_hash == block_hash
            });
        self.retained_branches
            .retain(|(entry_height, entry_view, entry_hash), _| {
                *entry_height != height || *entry_view != view || *entry_hash != block_hash
            });
    }

    pub(super) fn authoritative_slot_frontier_info(
        &self,
        height: u64,
        view: u64,
        block_hash: HashOf<BlockHeader>,
    ) -> Option<super::message::BlockCreatedFrontierInfo> {
        self.authoritative_block_frontiers
            .get(&(height, view, block_hash))
            .cloned()
    }

    pub(super) fn note_authoritative_slot_frontier_info(
        &mut self,
        height: u64,
        view: u64,
        block_hash: HashOf<BlockHeader>,
        frontier: super::message::BlockCreatedFrontierInfo,
    ) {
        self.authoritative_block_frontiers
            .retain(|(entry_height, entry_view, entry_hash), _| {
                *entry_height != height || *entry_view != view || *entry_hash == block_hash
            });
        self.authoritative_block_frontiers
            .insert((height, view, block_hash), frontier);
    }

    pub(super) fn note_retained_branch(
        &mut self,
        height: u64,
        view: u64,
        block_hash: HashOf<BlockHeader>,
        frontier_info: Option<super::message::BlockCreatedFrontierInfo>,
        payload_present: bool,
        now: Instant,
    ) {
        let entry = self
            .retained_branches
            .entry((height, view, block_hash))
            .or_insert_with(|| RetainedBranchState {
                height,
                view,
                block_hash,
                frontier_info: frontier_info.clone(),
                payload_present,
                last_refreshed_at: now,
            });
        entry.frontier_info = frontier_info.or_else(|| entry.frontier_info.clone());
        entry.payload_present |= payload_present;
        entry.last_refreshed_at = now;
    }

    pub(super) fn retained_branch_seed(&self, height: u64) -> Option<&RetainedBranchState> {
        self.retained_branches
            .values()
            .filter(|branch| branch.height == height && branch.payload_present)
            .max_by(|lhs, rhs| lhs.cmp_priority(rhs))
    }

    pub(super) fn remove_height(&mut self, height: u64) {
        self.proposals_seen
            .retain(|(entry_height, _)| *entry_height != height);
        self.authoritative_block_slots
            .retain(|(entry_height, _), _| *entry_height != height);
        self.authoritative_block_frontiers
            .retain(|(entry_height, _, _), _| *entry_height != height);
        self.retained_branches
            .retain(|(entry_height, _, _), _| *entry_height != height);
    }

    pub(super) fn prune_above_height(&mut self, keep_through_height: u64) {
        self.proposals_seen
            .retain(|(entry_height, _)| *entry_height <= keep_through_height);
        self.authoritative_block_slots
            .retain(|(entry_height, _), _| *entry_height <= keep_through_height);
        self.authoritative_block_frontiers
            .retain(|(entry_height, _, _), _| *entry_height <= keep_through_height);
        self.retained_branches
            .retain(|(entry_height, _, _), _| *entry_height <= keep_through_height);
    }

    pub(super) fn prune_committed(&mut self, committed_height: u64) {
        self.proposals_seen
            .retain(|(entry_height, _)| *entry_height > committed_height);
        self.authoritative_block_slots
            .retain(|(entry_height, _), _| *entry_height > committed_height);
        self.authoritative_block_frontiers
            .retain(|(entry_height, _, _), _| *entry_height > committed_height);
        self.retained_branches
            .retain(|(entry_height, _, _), _| *entry_height > committed_height);
    }
}
