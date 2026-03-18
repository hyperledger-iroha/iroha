//! Bounded exhaustive state-machine tests for Sumeragi consensus.
//!
//! These tests model a finite subset of the `main_loop` behavior while reusing
//! actor-derived quorum thresholds so the model remains tied to runtime
//! semantics. The model explores bounded interleavings across:
//! - consensus phases (`Propose`/`Prepare`/`Commit`/`NewView`)
//! - bounded recovery FSM stages
//! - RBC payload progress/fault states
//! - Byzantine fault classes (equivocation, invalid signatures/QCs, withholding,
//!   stale replay/mode mismatch, and RBC corruption/conflicts)
//!
//! Coverage is bounded by configuration (max steps/views) and is exhaustive over
//! the modeled alphabet within those bounds.

use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};

use iroha_config::parameters::actual::ConsensusMode;

use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum FaultKind {
    Equivocation,
    InvalidSignature,
    InvalidQc,
    Withhold,
    StaleReplayModeMismatch,
    RbcCorruption,
    RbcConflict,
    RbcWithholding,
    RbcInsufficientDeliver,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum ConsensusStage {
    Propose,
    Prepare,
    CommitVote,
    NewView,
    Committed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum RecoveryStage {
    RefreshTopology,
    BlockSync,
    WaitingForDependencies,
    ViewChangeEscalation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum RbcState {
    Idle,
    Init,
    Chunking,
    ChunksComplete,
    ReadyPartial,
    ReadyQuorum,
    Delivered,
    Corrupted,
    Withheld,
}

#[derive(Clone, Copy, Debug)]
struct ModelConfig {
    mode: ConsensusMode,
    validator_count: u8,
    fault_budget: u8,
    honest_validators: u8,
    commit_quorum: u8,
    view_change_quorum: u8,
    ready_quorum: u8,
    max_views: u8,
    max_steps: u8,
    gst_step: u8,
    rbc_required_chunks: u8,
    da_enabled: bool,
    equivalence_reduced: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct ModelState {
    height: u8,
    view: u8,
    consensus: ConsensusStage,
    recovery: RecoveryStage,
    rbc: RbcState,
    prepare_votes: u8,
    commit_votes_honest: u8,
    commit_votes_byzantine: u8,
    stake_votes: u8,
    new_view_votes: u8,
    ready_votes: u8,
    chunk_count: u8,
    recovery_retries: u8,
    byz_equivocations: u8,
    byz_invalid_sig: u8,
    byz_invalid_qc: u8,
    byz_withholds: u8,
    committed_hash: u8,
    committed_conflict: bool,
    dependency_arrived: bool,
    gst: bool,
}

impl ModelState {
    fn initial() -> Self {
        Self {
            height: 0,
            view: 0,
            consensus: ConsensusStage::Propose,
            recovery: RecoveryStage::RefreshTopology,
            rbc: RbcState::Idle,
            prepare_votes: 0,
            commit_votes_honest: 0,
            commit_votes_byzantine: 0,
            stake_votes: 0,
            new_view_votes: 0,
            ready_votes: 0,
            chunk_count: 0,
            recovery_retries: 0,
            byz_equivocations: 0,
            byz_invalid_sig: 0,
            byz_invalid_qc: 0,
            byz_withholds: 0,
            committed_hash: 0,
            committed_conflict: false,
            dependency_arrived: false,
            gst: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum ModelEvent {
    HonestPropose,
    HonestPrepareVote,
    HonestCommitVote,
    HonestNewViewVote,
    TimeoutTick,
    DependencyArrived,
    GstElapsed,
    ByzantineEquivocatePrepare,
    ByzantineEquivocateCommit,
    ByzantineInvalidSignatureVote,
    ByzantineInvalidQcBroadcast,
    ByzantineWithholdVotes,
    ByzantineStaleReplayModeMismatch,
    RbcInit,
    RbcInitConflictingHeader,
    RbcChunkGood,
    RbcChunkReordered,
    RbcChunkCorrupt,
    RbcChunkWithhold,
    RbcReadyGood,
    RbcReadyConflicting,
    RbcDeliverGood,
    RbcDeliverInsufficientReady,
}

const ALL_EVENTS: &[ModelEvent] = &[
    ModelEvent::HonestPropose,
    ModelEvent::HonestPrepareVote,
    ModelEvent::HonestCommitVote,
    ModelEvent::HonestNewViewVote,
    ModelEvent::TimeoutTick,
    ModelEvent::DependencyArrived,
    ModelEvent::GstElapsed,
    ModelEvent::ByzantineEquivocatePrepare,
    ModelEvent::ByzantineEquivocateCommit,
    ModelEvent::ByzantineInvalidSignatureVote,
    ModelEvent::ByzantineInvalidQcBroadcast,
    ModelEvent::ByzantineWithholdVotes,
    ModelEvent::ByzantineStaleReplayModeMismatch,
    ModelEvent::RbcInit,
    ModelEvent::RbcInitConflictingHeader,
    ModelEvent::RbcChunkGood,
    ModelEvent::RbcChunkReordered,
    ModelEvent::RbcChunkCorrupt,
    ModelEvent::RbcChunkWithhold,
    ModelEvent::RbcReadyGood,
    ModelEvent::RbcReadyConflicting,
    ModelEvent::RbcDeliverGood,
    ModelEvent::RbcDeliverInsufficientReady,
];

const HONEST_PROGRESS_EVENTS: &[ModelEvent] = &[
    ModelEvent::HonestPropose,
    ModelEvent::HonestPrepareVote,
    ModelEvent::HonestCommitVote,
    ModelEvent::HonestNewViewVote,
    ModelEvent::TimeoutTick,
    ModelEvent::DependencyArrived,
    ModelEvent::RbcInit,
    ModelEvent::RbcChunkGood,
    ModelEvent::RbcReadyGood,
    ModelEvent::RbcDeliverGood,
];

#[derive(Debug, Default)]
struct ExplorationReport {
    states_explored: usize,
    transitions_explored: u64,
    commits_seen: usize,
    max_height: u8,
    recovery_escalations_seen: usize,
    rbc_deliveries_seen: usize,
    stake_gap_states_seen: usize,
    fault_kinds_seen: BTreeSet<FaultKind>,
    safety_violations: Vec<String>,
    liveness_violations: Vec<String>,
}

fn event_fault_kind(event: ModelEvent) -> Option<FaultKind> {
    match event {
        ModelEvent::ByzantineEquivocatePrepare | ModelEvent::ByzantineEquivocateCommit => {
            Some(FaultKind::Equivocation)
        }
        ModelEvent::ByzantineInvalidSignatureVote => Some(FaultKind::InvalidSignature),
        ModelEvent::ByzantineInvalidQcBroadcast => Some(FaultKind::InvalidQc),
        ModelEvent::ByzantineWithholdVotes => Some(FaultKind::Withhold),
        ModelEvent::ByzantineStaleReplayModeMismatch => Some(FaultKind::StaleReplayModeMismatch),
        ModelEvent::RbcChunkCorrupt => Some(FaultKind::RbcCorruption),
        ModelEvent::RbcInitConflictingHeader | ModelEvent::RbcReadyConflicting => {
            Some(FaultKind::RbcConflict)
        }
        ModelEvent::RbcChunkWithhold => Some(FaultKind::RbcWithholding),
        ModelEvent::RbcDeliverInsufficientReady => Some(FaultKind::RbcInsufficientDeliver),
        ModelEvent::HonestPropose
        | ModelEvent::HonestPrepareVote
        | ModelEvent::HonestCommitVote
        | ModelEvent::HonestNewViewVote
        | ModelEvent::TimeoutTick
        | ModelEvent::DependencyArrived
        | ModelEvent::GstElapsed
        | ModelEvent::RbcInit
        | ModelEvent::RbcChunkGood
        | ModelEvent::RbcChunkReordered
        | ModelEvent::RbcReadyGood
        | ModelEvent::RbcDeliverGood => None,
    }
}

fn clamp_increment(value: &mut u8, cap: u8) -> bool {
    if *value >= cap {
        return false;
    }
    *value = value.saturating_add(1);
    true
}

fn total_commit_votes(state: &ModelState) -> u8 {
    state
        .commit_votes_honest
        .saturating_add(state.commit_votes_byzantine)
}

fn commit_votes_ready(state: &ModelState, cfg: &ModelConfig) -> bool {
    let total_votes = total_commit_votes(state);
    if total_votes < cfg.commit_quorum {
        return false;
    }
    if matches!(cfg.mode, ConsensusMode::Npos) && state.stake_votes < cfg.commit_quorum {
        return false;
    }
    true
}

fn da_ready_for_commit(state: &ModelState, cfg: &ModelConfig) -> bool {
    !cfg.da_enabled || matches!(state.rbc, RbcState::Delivered)
}

fn reset_round_votes(state: &mut ModelState) {
    state.prepare_votes = 0;
    state.commit_votes_honest = 0;
    state.commit_votes_byzantine = 0;
    state.stake_votes = 0;
    state.new_view_votes = 0;
}

fn reset_rbc_round(state: &mut ModelState) {
    state.rbc = RbcState::Idle;
    state.chunk_count = 0;
    state.ready_votes = 0;
}

fn apply_commit(state: &mut ModelState) {
    state.height = state.height.saturating_add(1);
    state.consensus = ConsensusStage::Committed;
    state.committed_hash = 1;
    state.view = 0;
    state.recovery = RecoveryStage::RefreshTopology;
    state.recovery_retries = 0;
}

fn apply_event(mut state: ModelState, event: ModelEvent, cfg: &ModelConfig) -> Option<ModelState> {
    let fault_counter_cap = cfg.fault_budget.saturating_add(1);
    let before = state;

    match event {
        ModelEvent::HonestPropose => {
            if !matches!(
                state.consensus,
                ConsensusStage::Propose | ConsensusStage::NewView | ConsensusStage::Committed
            ) {
                return None;
            }
            if matches!(state.consensus, ConsensusStage::Committed) {
                state.consensus = ConsensusStage::Propose;
            }
            state.consensus = ConsensusStage::Prepare;
            state.recovery = RecoveryStage::RefreshTopology;
            state.new_view_votes = 0;
            state.prepare_votes = 0;
            if cfg.da_enabled && matches!(state.rbc, RbcState::Idle) {
                state.rbc = RbcState::Init;
            }
        }
        ModelEvent::HonestPrepareVote => {
            if !matches!(state.consensus, ConsensusStage::Prepare) {
                return None;
            }
            let _ = clamp_increment(&mut state.prepare_votes, cfg.honest_validators);
            if state.prepare_votes >= cfg.commit_quorum {
                state.consensus = ConsensusStage::CommitVote;
            }
        }
        ModelEvent::HonestCommitVote => {
            if !matches!(state.consensus, ConsensusStage::CommitVote) {
                return None;
            }
            let _ = clamp_increment(&mut state.commit_votes_honest, cfg.honest_validators);
            let _ = clamp_increment(&mut state.stake_votes, cfg.honest_validators);
            if commit_votes_ready(&state, cfg) && da_ready_for_commit(&state, cfg) {
                apply_commit(&mut state);
            }
        }
        ModelEvent::HonestNewViewVote => {
            if !matches!(state.consensus, ConsensusStage::NewView) {
                return None;
            }
            let _ = clamp_increment(&mut state.new_view_votes, cfg.honest_validators);
            if state.new_view_votes >= cfg.view_change_quorum {
                state.consensus = ConsensusStage::Propose;
                state.recovery = RecoveryStage::RefreshTopology;
                state.recovery_retries = 0;
            }
        }
        ModelEvent::TimeoutTick => {
            let _ = clamp_increment(&mut state.recovery_retries, cfg.max_steps);
            state.consensus = ConsensusStage::NewView;
            reset_round_votes(&mut state);
            state.view = state.view.saturating_add(1).min(cfg.max_views);

            state.recovery = match state.recovery {
                RecoveryStage::RefreshTopology => RecoveryStage::BlockSync,
                RecoveryStage::BlockSync => RecoveryStage::WaitingForDependencies,
                RecoveryStage::WaitingForDependencies => {
                    if state.dependency_arrived {
                        state.dependency_arrived = false;
                        state.recovery_retries = 0;
                        RecoveryStage::RefreshTopology
                    } else if state.recovery_retries >= fault_counter_cap
                        || state.view >= cfg.max_views
                    {
                        reset_rbc_round(&mut state);
                        RecoveryStage::ViewChangeEscalation
                    } else {
                        RecoveryStage::WaitingForDependencies
                    }
                }
                RecoveryStage::ViewChangeEscalation => {
                    state.dependency_arrived = false;
                    state.recovery_retries = 0;
                    RecoveryStage::RefreshTopology
                }
            };
        }
        ModelEvent::DependencyArrived => {
            state.dependency_arrived = true;
            if matches!(state.recovery, RecoveryStage::WaitingForDependencies) {
                state.recovery = RecoveryStage::RefreshTopology;
                state.recovery_retries = 0;
            }
        }
        ModelEvent::GstElapsed => {
            state.gst = true;
        }
        ModelEvent::ByzantineEquivocatePrepare => {
            if !matches!(state.consensus, ConsensusStage::Prepare) {
                return None;
            }
            let _ = clamp_increment(&mut state.byz_equivocations, fault_counter_cap);
        }
        ModelEvent::ByzantineEquivocateCommit => {
            if !matches!(state.consensus, ConsensusStage::CommitVote) {
                return None;
            }
            let _ = clamp_increment(&mut state.byz_equivocations, fault_counter_cap);
            let _ = clamp_increment(&mut state.commit_votes_byzantine, cfg.fault_budget);
            if commit_votes_ready(&state, cfg) && da_ready_for_commit(&state, cfg) {
                apply_commit(&mut state);
            }
        }
        ModelEvent::ByzantineInvalidSignatureVote => {
            let _ = clamp_increment(&mut state.byz_invalid_sig, fault_counter_cap);
        }
        ModelEvent::ByzantineInvalidQcBroadcast => {
            let _ = clamp_increment(&mut state.byz_invalid_qc, fault_counter_cap);
        }
        ModelEvent::ByzantineWithholdVotes => {
            let _ = clamp_increment(&mut state.byz_withholds, fault_counter_cap);
        }
        ModelEvent::ByzantineStaleReplayModeMismatch => {
            let _ = clamp_increment(&mut state.byz_invalid_qc, fault_counter_cap);
        }
        ModelEvent::RbcInit => {
            if !matches!(
                state.rbc,
                RbcState::Idle | RbcState::Withheld | RbcState::Corrupted
            ) {
                return None;
            }
            state.rbc = RbcState::Init;
            state.chunk_count = 0;
            state.ready_votes = 0;
        }
        ModelEvent::RbcInitConflictingHeader => {
            if !matches!(
                state.rbc,
                RbcState::Init
                    | RbcState::Chunking
                    | RbcState::ChunksComplete
                    | RbcState::ReadyPartial
                    | RbcState::ReadyQuorum
            ) {
                return None;
            }
            state.rbc = RbcState::Corrupted;
        }
        ModelEvent::RbcChunkGood | ModelEvent::RbcChunkReordered => {
            if !matches!(
                state.rbc,
                RbcState::Init | RbcState::Chunking | RbcState::Withheld
            ) {
                return None;
            }
            if matches!(state.rbc, RbcState::Withheld) && !state.gst {
                return None;
            }
            let _ = clamp_increment(&mut state.chunk_count, cfg.rbc_required_chunks);
            state.rbc = if state.chunk_count >= cfg.rbc_required_chunks {
                RbcState::ChunksComplete
            } else {
                RbcState::Chunking
            };
        }
        ModelEvent::RbcChunkCorrupt => {
            if !matches!(
                state.rbc,
                RbcState::Init
                    | RbcState::Chunking
                    | RbcState::ChunksComplete
                    | RbcState::ReadyPartial
                    | RbcState::ReadyQuorum
            ) {
                return None;
            }
            state.rbc = RbcState::Corrupted;
        }
        ModelEvent::RbcChunkWithhold => {
            if !matches!(
                state.rbc,
                RbcState::Init
                    | RbcState::Chunking
                    | RbcState::ChunksComplete
                    | RbcState::ReadyPartial
            ) {
                return None;
            }
            state.rbc = RbcState::Withheld;
        }
        ModelEvent::RbcReadyGood => {
            if !matches!(
                state.rbc,
                RbcState::ChunksComplete | RbcState::ReadyPartial | RbcState::ReadyQuorum
            ) {
                return None;
            }
            let _ = clamp_increment(&mut state.ready_votes, cfg.honest_validators);
            state.rbc = if state.ready_votes >= cfg.ready_quorum {
                RbcState::ReadyQuorum
            } else {
                RbcState::ReadyPartial
            };
        }
        ModelEvent::RbcReadyConflicting => {
            if !matches!(
                state.rbc,
                RbcState::ChunksComplete | RbcState::ReadyPartial | RbcState::ReadyQuorum
            ) {
                return None;
            }
            state.rbc = RbcState::Corrupted;
        }
        ModelEvent::RbcDeliverGood => {
            if !matches!(state.rbc, RbcState::ReadyQuorum) {
                return None;
            }
            state.rbc = RbcState::Delivered;
            if matches!(state.consensus, ConsensusStage::CommitVote)
                && commit_votes_ready(&state, cfg)
                && da_ready_for_commit(&state, cfg)
            {
                apply_commit(&mut state);
            }
        }
        ModelEvent::RbcDeliverInsufficientReady => {
            if state.ready_votes >= cfg.ready_quorum || matches!(state.rbc, RbcState::Delivered) {
                return None;
            }
            let _ = clamp_increment(&mut state.byz_invalid_qc, fault_counter_cap);
        }
    }

    if state == before {
        return None;
    }
    Some(state)
}

fn canonicalize_state(mut state: ModelState, cfg: &ModelConfig) -> ModelState {
    if !cfg.equivalence_reduced {
        return state;
    }

    let quorum_cap = cfg.commit_quorum;
    let fault_cap = cfg.fault_budget.saturating_add(1);
    state.prepare_votes = state.prepare_votes.min(quorum_cap);
    state.commit_votes_honest = state.commit_votes_honest.min(quorum_cap);
    state.commit_votes_byzantine = state.commit_votes_byzantine.min(cfg.fault_budget);
    state.stake_votes = state.stake_votes.min(quorum_cap);
    state.new_view_votes = state.new_view_votes.min(cfg.view_change_quorum);
    state.ready_votes = state.ready_votes.min(cfg.ready_quorum);
    state.chunk_count = state.chunk_count.min(cfg.rbc_required_chunks);
    state.recovery_retries = state.recovery_retries.min(cfg.max_views);
    // Byzantine fault counters do not affect transition guards; collapse them
    // to a seen/unseen marker under equivalence reduction.
    state.byz_equivocations = state.byz_equivocations.min(fault_cap).min(1);
    state.byz_invalid_sig = state.byz_invalid_sig.min(fault_cap).min(1);
    state.byz_invalid_qc = state.byz_invalid_qc.min(fault_cap).min(1);
    state.byz_withholds = state.byz_withholds.min(fault_cap).min(1);
    state
}

fn validate_safety(state: ModelState, cfg: &ModelConfig, out: &mut Vec<String>) {
    if state.committed_conflict || state.committed_hash == 2 {
        out.push(format!(
            "conflicting commit marker observed at height={} view={}",
            state.height, state.view
        ));
    }

    if matches!(state.consensus, ConsensusStage::Committed) {
        let total_votes = total_commit_votes(&state);
        if total_votes < cfg.commit_quorum {
            out.push(format!(
                "committed without commit quorum: collected={} required={}",
                total_votes, cfg.commit_quorum
            ));
        }
        if matches!(cfg.mode, ConsensusMode::Npos) && state.stake_votes < cfg.commit_quorum {
            out.push(format!(
                "NPoS committed without stake quorum: stake_votes={} required={}",
                state.stake_votes, cfg.commit_quorum
            ));
        }
        if cfg.da_enabled && !matches!(state.rbc, RbcState::Delivered) {
            out.push(format!(
                "committed in DA mode without RBC delivery (rbc={:?})",
                state.rbc
            ));
        }
    }

    if matches!(state.rbc, RbcState::Delivered) {
        if state.ready_votes < cfg.ready_quorum {
            out.push(format!(
                "RBC delivered without ready quorum: ready_votes={} required={}",
                state.ready_votes, cfg.ready_quorum
            ));
        }
        if state.chunk_count < cfg.rbc_required_chunks {
            out.push(format!(
                "RBC delivered without required chunks: chunks={} required={}",
                state.chunk_count, cfg.rbc_required_chunks
            ));
        }
    }

    if state.view > cfg.max_views {
        out.push(format!(
            "view exceeded bounded horizon: view={} max_views={}",
            state.view, cfg.max_views
        ));
    }
}

fn can_reach_commit_with_honest_schedule(
    state: ModelState,
    cfg: &ModelConfig,
    remaining_steps: u8,
    memo: &mut HashMap<(ModelState, u8), bool>,
) -> bool {
    if state.height > 0 && matches!(state.consensus, ConsensusStage::Committed) {
        return true;
    }
    if remaining_steps == 0 {
        return false;
    }

    if let Some(cached) = memo.get(&(state, remaining_steps)) {
        return *cached;
    }

    for event in HONEST_PROGRESS_EVENTS {
        let Some(mut next) = apply_event(state, *event, cfg) else {
            continue;
        };
        // Eventual synchrony assumption in liveness checks.
        next.gst = true;
        if can_reach_commit_with_honest_schedule(next, cfg, remaining_steps - 1, memo) {
            memo.insert((state, remaining_steps), true);
            return true;
        }
    }

    memo.insert((state, remaining_steps), false);
    false
}

fn validate_liveness(visited: &HashSet<ModelState>, cfg: &ModelConfig, out: &mut Vec<String>) {
    let expected_validator_count = cfg.honest_validators.saturating_add(cfg.fault_budget);
    if cfg.validator_count != expected_validator_count {
        out.push(format!(
            "invalid config: validator_count={} honest={} fault_budget={}",
            cfg.validator_count, cfg.honest_validators, cfg.fault_budget
        ));
        return;
    }

    if cfg.honest_validators < cfg.commit_quorum {
        out.push(format!(
            "invalid config for liveness: honest={} quorum={}",
            cfg.honest_validators, cfg.commit_quorum
        ));
        return;
    }

    let mut memo = HashMap::new();
    // Honest recovery requires at most: timeout/escalation handling, a full new-view
    // quorum, propose+prepare+commit quorums, and one RBC re-initialization+delivery.
    let local_horizon = cfg
        .view_change_quorum
        .saturating_add(cfg.commit_quorum.saturating_mul(2))
        .saturating_add(cfg.ready_quorum)
        .saturating_add(cfg.rbc_required_chunks)
        .saturating_add(4)
        .min(cfg.max_steps);
    for state in visited {
        if !state.gst {
            continue;
        }
        if state.height > 0 && matches!(state.consensus, ConsensusStage::Committed) {
            continue;
        }

        if !can_reach_commit_with_honest_schedule(*state, cfg, local_horizon, &mut memo) {
            out.push(format!(
                "no bounded honest-commit path from state: height={} view={} consensus={:?} recovery={:?} rbc={:?}",
                state.height, state.view, state.consensus, state.recovery, state.rbc
            ));
            if out.len() >= 12 {
                break;
            }
        }
    }
}

fn run_bounded_exhaustive(cfg: ModelConfig, events: &[ModelEvent]) -> ExplorationReport {
    let mut report = ExplorationReport::default();
    let mut queue = VecDeque::new();
    let mut visited = HashSet::new();

    let init = canonicalize_state(ModelState::initial(), &cfg);
    queue.push_back((init, 0_u8));
    visited.insert(init);

    while let Some((state, depth)) = queue.pop_front() {
        report.states_explored += 1;
        report.max_height = report.max_height.max(state.height);
        if state.height > 0 && matches!(state.consensus, ConsensusStage::Committed) {
            report.commits_seen = report.commits_seen.saturating_add(1);
        }
        if matches!(state.recovery, RecoveryStage::ViewChangeEscalation) {
            report.recovery_escalations_seen = report.recovery_escalations_seen.saturating_add(1);
        }
        if matches!(state.rbc, RbcState::Delivered) {
            report.rbc_deliveries_seen = report.rbc_deliveries_seen.saturating_add(1);
        }
        if matches!(cfg.mode, ConsensusMode::Npos)
            && total_commit_votes(&state) >= cfg.commit_quorum
            && state.stake_votes < cfg.commit_quorum
        {
            report.stake_gap_states_seen = report.stake_gap_states_seen.saturating_add(1);
        }

        validate_safety(state, &cfg, &mut report.safety_violations);

        // Treat committed states as terminal in this bounded model:
        // post-commit branching does not add safety/liveness signal but
        // explodes the state space.
        if state.height > 0 && matches!(state.consensus, ConsensusStage::Committed) {
            continue;
        }

        if depth >= cfg.max_steps {
            continue;
        }

        for event in events {
            let Some(mut next) = apply_event(state, *event, &cfg) else {
                continue;
            };
            report.transitions_explored = report.transitions_explored.saturating_add(1);
            next.gst = next.gst || depth.saturating_add(1) >= cfg.gst_step;
            if let Some(kind) = event_fault_kind(*event) {
                report.fault_kinds_seen.insert(kind);
            }
            let canonical = canonicalize_state(next, &cfg);
            if visited.insert(canonical) {
                queue.push_back((canonical, depth.saturating_add(1)));
            }
        }
    }

    validate_liveness(&visited, &cfg, &mut report.liveness_violations);
    report
}

fn apply_sequence(cfg: ModelConfig, sequence: &[ModelEvent]) -> ModelState {
    let mut state = ModelState::initial();
    for event in sequence {
        if let Some(next) = apply_event(state, *event, &cfg) {
            state = next;
        }
    }
    state
}

async fn model_config_from_actor(
    mode: ConsensusMode,
    validator_count: usize,
    equivalence_reduced: bool,
) -> (ModelConfig, TestActorHarness) {
    let mut consensus_cfg = test_sumeragi_config();
    consensus_cfg.consensus_mode = mode;
    consensus_cfg.da.enabled = true;

    let harness = test_actor_harness_with_config(validator_count, consensus_cfg, None).await;
    let mut roster = harness.actor.effective_commit_topology();
    if roster.is_empty() {
        let trusted = harness.actor.common_config.trusted_peers.value();
        roster = crate::sumeragi::filter_validators_from_trusted(trusted);
        if roster.is_empty() {
            roster = trusted.clone().into_non_empty_vec().into_iter().collect();
        }
    }
    let topology = super::network_topology::Topology::new(roster);
    let observed_len = topology.as_ref().len();
    assert_eq!(
        observed_len, validator_count,
        "actor topology mismatch for model configuration"
    );

    let n = u8::try_from(validator_count).expect("validator count fits in u8");
    let f = u8::try_from((validator_count.saturating_sub(1)) / 3).expect("fault count fits");
    assert_eq!(
        validator_count,
        usize::from(f).saturating_mul(3).saturating_add(1),
        "model currently expects 3f+1 topologies"
    );
    let honest = n.saturating_sub(f);

    let commit_quorum = u8::try_from(topology.min_votes_for_commit()).expect("quorum fits u8");
    let expected_quorum = u8::try_from(super::network_topology::commit_quorum_from_len(
        validator_count,
    ))
    .expect("helper quorum fits u8");
    assert_eq!(
        commit_quorum, expected_quorum,
        "topology quorum and helper quorum must match"
    );
    let view_change_quorum =
        u8::try_from(topology.min_votes_for_view_change()).expect("view change quorum fits u8");

    let max_steps = if validator_count <= 4 { 24 } else { 36 };
    let cfg = ModelConfig {
        mode,
        validator_count: n,
        fault_budget: f,
        honest_validators: honest,
        commit_quorum,
        view_change_quorum,
        ready_quorum: commit_quorum,
        max_views: 4,
        max_steps,
        gst_step: 4,
        rbc_required_chunks: 2,
        da_enabled: true,
        equivalence_reduced,
    };

    (cfg, harness)
}

fn assert_clean_report(report: &ExplorationReport) {
    assert!(
        report.safety_violations.is_empty(),
        "safety violations: {:?}",
        report.safety_violations
    );
    assert!(
        report.liveness_violations.is_empty(),
        "liveness violations: {:?}",
        report.liveness_violations
    );
}

#[tokio::test(flavor = "current_thread")]
async fn model_permissioned_n4_exhaustive_safety_and_liveness() {
    let (cfg, harness) = model_config_from_actor(ConsensusMode::Permissioned, 4, false).await;
    let report = run_bounded_exhaustive(cfg, ALL_EVENTS);
    assert_clean_report(&report);
    assert!(report.commits_seen > 0, "model should commit at least once");
    assert!(
        report.rbc_deliveries_seen > 0,
        "model should deliver RBC payloads"
    );
    drop(harness);
}

#[tokio::test(flavor = "current_thread")]
async fn model_permissioned_n7_equivalence_exhaustive_safety_and_liveness() {
    let (cfg, harness) = model_config_from_actor(ConsensusMode::Permissioned, 7, true).await;
    let report = run_bounded_exhaustive(cfg, ALL_EVENTS);
    assert_clean_report(&report);
    assert!(report.commits_seen > 0, "model should commit at least once");
    assert!(
        report.rbc_deliveries_seen > 0,
        "model should deliver RBC payloads"
    );
    drop(harness);
}

#[tokio::test(flavor = "current_thread")]
async fn model_npos_n4_exhaustive_safety_and_liveness() {
    let (cfg, harness) = model_config_from_actor(ConsensusMode::Npos, 4, false).await;
    let report = run_bounded_exhaustive(cfg, ALL_EVENTS);
    assert_clean_report(&report);
    assert!(report.commits_seen > 0, "model should commit at least once");
    assert!(
        report.rbc_deliveries_seen > 0,
        "model should deliver RBC payloads"
    );
    drop(harness);
}

#[tokio::test(flavor = "current_thread")]
async fn model_npos_n7_equivalence_exhaustive_safety_and_liveness() {
    let (cfg, harness) = model_config_from_actor(ConsensusMode::Npos, 7, true).await;
    let report = run_bounded_exhaustive(cfg, ALL_EVENTS);
    assert_clean_report(&report);
    assert!(report.commits_seen > 0, "model should commit at least once");
    assert!(
        report.rbc_deliveries_seen > 0,
        "model should deliver RBC payloads"
    );
    drop(harness);
}

#[tokio::test(flavor = "current_thread")]
async fn model_vote_equivocation_never_breaks_safety() {
    let (cfg, harness) = model_config_from_actor(ConsensusMode::Permissioned, 4, false).await;
    let events = [
        ModelEvent::HonestPropose,
        ModelEvent::HonestPrepareVote,
        ModelEvent::ByzantineEquivocatePrepare,
        ModelEvent::HonestPrepareVote,
        ModelEvent::ByzantineEquivocateCommit,
        ModelEvent::HonestCommitVote,
        ModelEvent::TimeoutTick,
        ModelEvent::HonestNewViewVote,
    ];
    let report = run_bounded_exhaustive(cfg, &events);
    assert!(
        report.fault_kinds_seen.contains(&FaultKind::Equivocation),
        "equivocation faults should be exercised"
    );
    assert!(
        report.safety_violations.is_empty(),
        "equivocation must not violate safety: {:?}",
        report.safety_violations
    );
    drop(harness);
}

#[tokio::test(flavor = "current_thread")]
async fn model_invalid_qc_or_signature_never_commits() {
    let (cfg, harness) = model_config_from_actor(ConsensusMode::Permissioned, 4, false).await;
    let events = [
        ModelEvent::ByzantineInvalidSignatureVote,
        ModelEvent::ByzantineInvalidQcBroadcast,
        ModelEvent::ByzantineStaleReplayModeMismatch,
        ModelEvent::TimeoutTick,
        ModelEvent::ByzantineInvalidQcBroadcast,
        ModelEvent::ByzantineInvalidSignatureVote,
    ];
    let report = run_bounded_exhaustive(cfg, &events);
    assert_eq!(
        report.commits_seen, 0,
        "invalid signatures/QCs alone must not produce commits"
    );
    assert!(report.safety_violations.is_empty());
    drop(harness);
}

#[tokio::test(flavor = "current_thread")]
async fn model_rbc_corruption_reordering_withholding_preserves_safety() {
    let (cfg, harness) = model_config_from_actor(ConsensusMode::Permissioned, 4, false).await;
    let events = [
        ModelEvent::HonestPropose,
        ModelEvent::RbcInit,
        ModelEvent::RbcChunkReordered,
        ModelEvent::RbcChunkCorrupt,
        ModelEvent::RbcChunkWithhold,
        ModelEvent::RbcReadyConflicting,
        ModelEvent::RbcDeliverInsufficientReady,
        ModelEvent::TimeoutTick,
        ModelEvent::DependencyArrived,
        ModelEvent::HonestNewViewVote,
        ModelEvent::HonestPropose,
        ModelEvent::RbcInit,
        ModelEvent::RbcChunkGood,
        ModelEvent::RbcChunkGood,
        ModelEvent::RbcReadyGood,
        ModelEvent::RbcReadyGood,
        ModelEvent::RbcReadyGood,
        ModelEvent::RbcDeliverGood,
    ];
    let report = run_bounded_exhaustive(cfg, &events);
    assert!(
        report.fault_kinds_seen.contains(&FaultKind::RbcCorruption)
            && report.fault_kinds_seen.contains(&FaultKind::RbcWithholding)
            && report.fault_kinds_seen.contains(&FaultKind::RbcConflict),
        "RBC corruption/withholding/conflict faults should be exercised"
    );
    assert!(
        report.safety_violations.is_empty(),
        "RBC faults must preserve safety: {:?}",
        report.safety_violations
    );
    drop(harness);
}

#[tokio::test(flavor = "current_thread")]
async fn model_rbc_faults_still_allow_liveness_with_honest_quorum() {
    let (cfg, harness) = model_config_from_actor(ConsensusMode::Permissioned, 7, true).await;
    let report = run_bounded_exhaustive(cfg, ALL_EVENTS);
    assert!(
        report.commits_seen > 0,
        "bounded exploration should still contain commit paths"
    );
    assert!(
        report.liveness_violations.is_empty(),
        "liveness must hold under bounded eventual synchrony: {:?}",
        report.liveness_violations
    );
    drop(harness);
}

#[tokio::test(flavor = "current_thread")]
async fn model_recovery_fsm_eventually_escalates_or_recovers() {
    let (cfg, harness) = model_config_from_actor(ConsensusMode::Permissioned, 4, false).await;
    let events = [
        ModelEvent::TimeoutTick,
        ModelEvent::TimeoutTick,
        ModelEvent::TimeoutTick,
        ModelEvent::DependencyArrived,
        ModelEvent::TimeoutTick,
        ModelEvent::TimeoutTick,
        ModelEvent::HonestNewViewVote,
    ];
    let report = run_bounded_exhaustive(cfg, &events);
    assert!(
        report.recovery_escalations_seen > 0 || report.max_height > 0,
        "recovery FSM should escalate or recover into progress"
    );
    assert!(report.safety_violations.is_empty());
    drop(harness);
}

#[tokio::test(flavor = "current_thread")]
async fn model_stale_replay_and_mode_tag_mismatch_are_non_progressing() {
    let (cfg, harness) = model_config_from_actor(ConsensusMode::Permissioned, 4, false).await;
    let events = [
        ModelEvent::ByzantineStaleReplayModeMismatch,
        ModelEvent::ByzantineStaleReplayModeMismatch,
        ModelEvent::ByzantineInvalidQcBroadcast,
        ModelEvent::ByzantineInvalidSignatureVote,
    ];
    let report = run_bounded_exhaustive(cfg, &events);
    assert_eq!(
        report.max_height, 0,
        "stale replay/mode mismatch should not commit"
    );
    assert_eq!(
        report.commits_seen, 0,
        "stale replay/mode mismatch should not commit"
    );
    assert!(report.safety_violations.is_empty());
    drop(harness);
}

#[tokio::test(flavor = "current_thread")]
async fn model_npos_stake_quorum_required_for_commit() {
    let (cfg, harness) = model_config_from_actor(ConsensusMode::Npos, 4, false).await;
    let sequence = [
        ModelEvent::HonestPropose,
        ModelEvent::RbcInit,
        ModelEvent::RbcChunkGood,
        ModelEvent::RbcChunkGood,
        ModelEvent::RbcReadyGood,
        ModelEvent::RbcReadyGood,
        ModelEvent::RbcReadyGood,
        ModelEvent::RbcDeliverGood,
        ModelEvent::HonestPrepareVote,
        ModelEvent::HonestPrepareVote,
        ModelEvent::HonestPrepareVote,
        ModelEvent::HonestCommitVote,
        ModelEvent::HonestCommitVote,
        ModelEvent::ByzantineEquivocateCommit,
    ];
    let state = apply_sequence(cfg, &sequence);
    assert!(
        total_commit_votes(&state) >= cfg.commit_quorum,
        "sequence should reach total commit quorum"
    );
    assert!(
        state.stake_votes < cfg.commit_quorum,
        "sequence should stay below stake quorum"
    );
    assert!(
        !matches!(state.consensus, ConsensusStage::Committed),
        "NPoS must not commit without stake quorum"
    );

    let report = run_bounded_exhaustive(cfg, ALL_EVENTS);
    assert!(
        report.stake_gap_states_seen > 0,
        "NPoS exploration should include stake-gap states"
    );
    assert!(report.safety_violations.is_empty());
    drop(harness);
}

#[tokio::test(flavor = "current_thread")]
async fn model_permissioned_vs_npos_mode_isolation_invariants() {
    let (permissioned_cfg, permissioned_harness) =
        model_config_from_actor(ConsensusMode::Permissioned, 4, false).await;
    let (npos_cfg, npos_harness) = model_config_from_actor(ConsensusMode::Npos, 4, false).await;

    let sequence = [
        ModelEvent::HonestPropose,
        ModelEvent::RbcInit,
        ModelEvent::RbcChunkGood,
        ModelEvent::RbcChunkGood,
        ModelEvent::RbcReadyGood,
        ModelEvent::RbcReadyGood,
        ModelEvent::RbcReadyGood,
        ModelEvent::RbcDeliverGood,
        ModelEvent::HonestPrepareVote,
        ModelEvent::HonestPrepareVote,
        ModelEvent::HonestPrepareVote,
        ModelEvent::HonestCommitVote,
        ModelEvent::HonestCommitVote,
        ModelEvent::ByzantineEquivocateCommit,
    ];

    let permissioned_state = apply_sequence(permissioned_cfg, &sequence);
    let npos_state = apply_sequence(npos_cfg, &sequence);

    assert!(
        matches!(permissioned_state.consensus, ConsensusStage::Committed),
        "permissioned path may commit with total quorum"
    );
    assert!(
        !matches!(npos_state.consensus, ConsensusStage::Committed),
        "NPoS path must not commit without stake quorum"
    );

    let permissioned_report = run_bounded_exhaustive(permissioned_cfg, ALL_EVENTS);
    let npos_report = run_bounded_exhaustive(npos_cfg, ALL_EVENTS);
    assert!(
        permissioned_report.safety_violations.is_empty()
            && npos_report.safety_violations.is_empty()
    );
    assert!(
        npos_report.stake_gap_states_seen > 0,
        "NPoS mode should preserve stake-gap distinction"
    );

    drop(permissioned_harness);
    drop(npos_harness);
}
