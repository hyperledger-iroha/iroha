//! Fairness-aware, stake-weighted bounded model tests for Sumeragi.
//!
//! This module complements the baseline bounded model tests with:
//! - heterogeneous stake accounting for NPoS quorum checks,
//! - stronger RBC causality guards (header/chunk/ready evidence),
//! - bounded weak-fairness liveness checks under adversarial scheduling,
//! - larger topology classes (N=10/N=13) in deep ignored tests.
//!
//! The model intentionally remains finite and bounded, but ties quorum constants
//! to actor-derived topology thresholds to stay aligned with runtime semantics.

use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};

use iroha_config::parameters::actual::ConsensusMode;
use iroha_primitives::numeric::Numeric;

use super::*;

const MAX_VALIDATORS: usize = 16;
const FAIRNESS_EVENT_COUNT: usize = 10;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum FairFaultKind {
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
enum FairConsensusStage {
    Propose,
    Prepare,
    CommitVote,
    NewView,
    Committed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum FairRecoveryStage {
    RefreshTopology,
    BlockSync,
    WaitingForDependencies,
    ViewChangeEscalation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum FairRbcState {
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
struct FairModelConfig {
    mode: ConsensusMode,
    validator_count: u8,
    fault_budget: u8,
    honest_validators: u8,
    commit_quorum: u8,
    view_change_quorum: u8,
    ready_quorum: u8,
    stake_commit_quorum: u16,
    honest_stake_by_signer: [u16; MAX_VALIDATORS],
    byzantine_stake_by_signer: [u16; MAX_VALIDATORS],
    honest_total_stake: u16,
    byzantine_total_stake: u16,
    total_stake: u16,
    max_views: u8,
    max_steps: u8,
    gst_step: u8,
    rbc_required_chunks: u8,
    da_enabled: bool,
    equivalence_reduced: bool,
    enable_fair_liveness: bool,
    fairness_window: u8,
    fairness_horizon: u8,
    fairness_state_budget: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct FairModelState {
    height: u8,
    view: u8,
    consensus: FairConsensusStage,
    recovery: FairRecoveryStage,
    rbc: FairRbcState,
    prepare_votes: u8,
    commit_votes_honest: u8,
    commit_votes_byzantine: u8,
    stake_votes: u16,
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
    rbc_header_seen: bool,
    rbc_chunk_digest_valid: bool,
    rbc_ready_witnesses: u8,
}

impl FairModelState {
    fn initial() -> Self {
        Self {
            height: 0,
            view: 0,
            consensus: FairConsensusStage::Propose,
            recovery: FairRecoveryStage::RefreshTopology,
            rbc: FairRbcState::Idle,
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
            rbc_header_seen: false,
            rbc_chunk_digest_valid: false,
            rbc_ready_witnesses: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum FairModelEvent {
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

const FAIR_ALL_EVENTS: &[FairModelEvent] = &[
    FairModelEvent::HonestPropose,
    FairModelEvent::HonestPrepareVote,
    FairModelEvent::HonestCommitVote,
    FairModelEvent::HonestNewViewVote,
    FairModelEvent::TimeoutTick,
    FairModelEvent::DependencyArrived,
    FairModelEvent::GstElapsed,
    FairModelEvent::ByzantineEquivocatePrepare,
    FairModelEvent::ByzantineEquivocateCommit,
    FairModelEvent::ByzantineInvalidSignatureVote,
    FairModelEvent::ByzantineInvalidQcBroadcast,
    FairModelEvent::ByzantineWithholdVotes,
    FairModelEvent::ByzantineStaleReplayModeMismatch,
    FairModelEvent::RbcInit,
    FairModelEvent::RbcInitConflictingHeader,
    FairModelEvent::RbcChunkGood,
    FairModelEvent::RbcChunkReordered,
    FairModelEvent::RbcChunkCorrupt,
    FairModelEvent::RbcChunkWithhold,
    FairModelEvent::RbcReadyGood,
    FairModelEvent::RbcReadyConflicting,
    FairModelEvent::RbcDeliverGood,
    FairModelEvent::RbcDeliverInsufficientReady,
];

const FAIRNESS_EVENTS: [FairModelEvent; FAIRNESS_EVENT_COUNT] = [
    FairModelEvent::HonestPropose,
    FairModelEvent::HonestPrepareVote,
    FairModelEvent::HonestCommitVote,
    FairModelEvent::HonestNewViewVote,
    FairModelEvent::TimeoutTick,
    FairModelEvent::DependencyArrived,
    FairModelEvent::RbcInit,
    FairModelEvent::RbcChunkGood,
    FairModelEvent::RbcReadyGood,
    FairModelEvent::RbcDeliverGood,
];

#[derive(Debug, Default)]
struct FairExplorationReport {
    states_explored: usize,
    transitions_explored: u64,
    commits_seen: usize,
    max_height: u8,
    recovery_escalations_seen: usize,
    rbc_deliveries_seen: usize,
    stake_gap_states_seen: usize,
    fault_kinds_seen: BTreeSet<FairFaultKind>,
    safety_violations: Vec<String>,
    liveness_violations: Vec<String>,
    fairness_violations: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct FairAdversaryKey {
    state: FairModelState,
    debts: [u8; FAIRNESS_EVENT_COUNT],
    remaining_steps: u8,
}

fn weighted_stake_quorum(total_stake: u16) -> u16 {
    let total = u32::from(total_stake);
    u16::try_from((total.saturating_mul(2).saturating_add(2)) / 3).unwrap_or(u16::MAX)
}

fn build_heterogeneous_stake_profile(
    validator_count: u8,
    fault_budget: u8,
) -> ([u16; MAX_VALIDATORS], [u16; MAX_VALIDATORS], u16, u16, u16) {
    let mut honest_stake = [0_u16; MAX_VALIDATORS];
    let mut byz_stake = [0_u16; MAX_VALIDATORS];

    let honest = usize::from(validator_count.saturating_sub(fault_budget));
    let byz = usize::from(fault_budget);

    for (idx, slot) in honest_stake.iter_mut().take(honest).enumerate() {
        *slot = 2 + u16::try_from((idx * 3 + 1) % 4).unwrap_or(0);
    }
    for (idx, slot) in byz_stake.iter_mut().take(byz).enumerate() {
        *slot = 1 + u16::try_from(idx % 2).unwrap_or(0);
    }

    let mut honest_total: u16 = honest_stake.iter().take(honest).copied().sum();
    let byz_total: u16 = byz_stake.iter().take(byz).copied().sum();
    let mut total_stake = honest_total.saturating_add(byz_total);

    let required_honest = weighted_stake_quorum(total_stake);
    if honest_total < required_honest && honest > 0 {
        let delta = required_honest.saturating_sub(honest_total);
        honest_stake[0] = honest_stake[0].saturating_add(delta);
        honest_total = honest_total.saturating_add(delta);
        total_stake = honest_total.saturating_add(byz_total);
    }

    // Keep the byzantine side sub-quorum by construction in liveness-positive configs.
    if byz_total.saturating_mul(3) >= total_stake && honest > 0 {
        let deficit = byz_total
            .saturating_mul(3)
            .saturating_sub(total_stake)
            .saturating_add(1)
            / 3;
        honest_stake[0] = honest_stake[0].saturating_add(deficit);
        honest_total = honest_total.saturating_add(deficit);
        total_stake = honest_total.saturating_add(byz_total);
    }

    (
        honest_stake,
        byz_stake,
        honest_total,
        byz_total,
        weighted_stake_quorum(total_stake),
    )
}

fn signed_stake_from_counts(cfg: &FairModelConfig, honest_votes: u8, byz_votes: u8) -> u16 {
    let honest_sum: u16 = cfg
        .honest_stake_by_signer
        .iter()
        .take(usize::from(honest_votes.min(cfg.honest_validators)))
        .copied()
        .sum();
    let byz_sum: u16 = cfg
        .byzantine_stake_by_signer
        .iter()
        .take(usize::from(byz_votes.min(cfg.fault_budget)))
        .copied()
        .sum();
    honest_sum.saturating_add(byz_sum)
}

fn recompute_signed_stake(state: &mut FairModelState, cfg: &FairModelConfig) {
    state.stake_votes =
        signed_stake_from_counts(cfg, state.commit_votes_honest, state.commit_votes_byzantine);
}

fn total_commit_votes(state: &FairModelState) -> u8 {
    state
        .commit_votes_honest
        .saturating_add(state.commit_votes_byzantine)
}

fn commit_votes_ready(state: &FairModelState, cfg: &FairModelConfig) -> bool {
    if total_commit_votes(state) < cfg.commit_quorum {
        return false;
    }
    if matches!(cfg.mode, ConsensusMode::Npos) && state.stake_votes < cfg.stake_commit_quorum {
        return false;
    }
    true
}

fn da_ready_for_commit(state: &FairModelState, cfg: &FairModelConfig) -> bool {
    !cfg.da_enabled || matches!(state.rbc, FairRbcState::Delivered)
}

fn reset_round_votes(state: &mut FairModelState) {
    state.prepare_votes = 0;
    state.commit_votes_honest = 0;
    state.commit_votes_byzantine = 0;
    state.stake_votes = 0;
    state.new_view_votes = 0;
}

fn reset_rbc_round(state: &mut FairModelState) {
    state.rbc = FairRbcState::Idle;
    state.chunk_count = 0;
    state.ready_votes = 0;
    state.rbc_header_seen = false;
    state.rbc_chunk_digest_valid = false;
    state.rbc_ready_witnesses = 0;
}

fn apply_commit(state: &mut FairModelState) {
    state.height = state.height.saturating_add(1);
    state.consensus = FairConsensusStage::Committed;
    state.committed_hash = 1;
    state.view = 0;
    state.recovery = FairRecoveryStage::RefreshTopology;
    state.recovery_retries = 0;
}

fn event_fault_kind(event: FairModelEvent) -> Option<FairFaultKind> {
    match event {
        FairModelEvent::ByzantineEquivocatePrepare | FairModelEvent::ByzantineEquivocateCommit => {
            Some(FairFaultKind::Equivocation)
        }
        FairModelEvent::ByzantineInvalidSignatureVote => Some(FairFaultKind::InvalidSignature),
        FairModelEvent::ByzantineInvalidQcBroadcast => Some(FairFaultKind::InvalidQc),
        FairModelEvent::ByzantineWithholdVotes => Some(FairFaultKind::Withhold),
        FairModelEvent::ByzantineStaleReplayModeMismatch => {
            Some(FairFaultKind::StaleReplayModeMismatch)
        }
        FairModelEvent::RbcChunkCorrupt => Some(FairFaultKind::RbcCorruption),
        FairModelEvent::RbcInitConflictingHeader | FairModelEvent::RbcReadyConflicting => {
            Some(FairFaultKind::RbcConflict)
        }
        FairModelEvent::RbcChunkWithhold => Some(FairFaultKind::RbcWithholding),
        FairModelEvent::RbcDeliverInsufficientReady => Some(FairFaultKind::RbcInsufficientDeliver),
        FairModelEvent::HonestPropose
        | FairModelEvent::HonestPrepareVote
        | FairModelEvent::HonestCommitVote
        | FairModelEvent::HonestNewViewVote
        | FairModelEvent::TimeoutTick
        | FairModelEvent::DependencyArrived
        | FairModelEvent::GstElapsed
        | FairModelEvent::RbcInit
        | FairModelEvent::RbcChunkGood
        | FairModelEvent::RbcChunkReordered
        | FairModelEvent::RbcReadyGood
        | FairModelEvent::RbcDeliverGood => None,
    }
}

fn clamp_increment(value: &mut u8, cap: u8) -> bool {
    if *value >= cap {
        return false;
    }
    *value = value.saturating_add(1);
    true
}

fn apply_event(
    mut state: FairModelState,
    event: FairModelEvent,
    cfg: &FairModelConfig,
) -> Option<FairModelState> {
    let fault_counter_cap = cfg.fault_budget.saturating_add(1);
    let before = state;

    match event {
        FairModelEvent::HonestPropose => {
            if !matches!(
                state.consensus,
                FairConsensusStage::Propose
                    | FairConsensusStage::NewView
                    | FairConsensusStage::Committed
            ) {
                return None;
            }
            if matches!(state.consensus, FairConsensusStage::Committed) {
                state.consensus = FairConsensusStage::Propose;
            }
            state.consensus = FairConsensusStage::Prepare;
            state.recovery = FairRecoveryStage::RefreshTopology;
            state.new_view_votes = 0;
            state.prepare_votes = 0;
            if cfg.da_enabled && matches!(state.rbc, FairRbcState::Idle) {
                state.rbc = FairRbcState::Init;
                state.rbc_header_seen = true;
                state.rbc_chunk_digest_valid = true;
                state.chunk_count = 0;
                state.ready_votes = 0;
                state.rbc_ready_witnesses = 0;
            }
        }
        FairModelEvent::HonestPrepareVote => {
            if !matches!(state.consensus, FairConsensusStage::Prepare) {
                return None;
            }
            let _ = clamp_increment(&mut state.prepare_votes, cfg.honest_validators);
            if state.prepare_votes >= cfg.commit_quorum {
                state.consensus = FairConsensusStage::CommitVote;
            }
        }
        FairModelEvent::HonestCommitVote => {
            if !matches!(state.consensus, FairConsensusStage::CommitVote) {
                return None;
            }
            let _ = clamp_increment(&mut state.commit_votes_honest, cfg.honest_validators);
            recompute_signed_stake(&mut state, cfg);
            if commit_votes_ready(&state, cfg) && da_ready_for_commit(&state, cfg) {
                apply_commit(&mut state);
            }
        }
        FairModelEvent::HonestNewViewVote => {
            if !matches!(state.consensus, FairConsensusStage::NewView) {
                return None;
            }
            let _ = clamp_increment(&mut state.new_view_votes, cfg.honest_validators);
            if state.new_view_votes >= cfg.view_change_quorum {
                state.consensus = FairConsensusStage::Propose;
                state.recovery = FairRecoveryStage::RefreshTopology;
                state.recovery_retries = 0;
            }
        }
        FairModelEvent::TimeoutTick => {
            let _ = clamp_increment(&mut state.recovery_retries, cfg.max_steps);
            state.consensus = FairConsensusStage::NewView;
            reset_round_votes(&mut state);
            state.view = state.view.saturating_add(1).min(cfg.max_views);

            state.recovery = match state.recovery {
                FairRecoveryStage::RefreshTopology => FairRecoveryStage::BlockSync,
                FairRecoveryStage::BlockSync => FairRecoveryStage::WaitingForDependencies,
                FairRecoveryStage::WaitingForDependencies => {
                    if state.dependency_arrived {
                        state.dependency_arrived = false;
                        state.recovery_retries = 0;
                        FairRecoveryStage::RefreshTopology
                    } else if state.recovery_retries >= fault_counter_cap
                        || state.view >= cfg.max_views
                    {
                        reset_rbc_round(&mut state);
                        FairRecoveryStage::ViewChangeEscalation
                    } else {
                        FairRecoveryStage::WaitingForDependencies
                    }
                }
                FairRecoveryStage::ViewChangeEscalation => {
                    state.dependency_arrived = false;
                    state.recovery_retries = 0;
                    FairRecoveryStage::RefreshTopology
                }
            };
        }
        FairModelEvent::DependencyArrived => {
            state.dependency_arrived = true;
            if matches!(state.recovery, FairRecoveryStage::WaitingForDependencies) {
                state.recovery = FairRecoveryStage::RefreshTopology;
                state.recovery_retries = 0;
            }
        }
        FairModelEvent::GstElapsed => {
            state.gst = true;
        }
        FairModelEvent::ByzantineEquivocatePrepare => {
            if !matches!(state.consensus, FairConsensusStage::Prepare) {
                return None;
            }
            let _ = clamp_increment(&mut state.byz_equivocations, fault_counter_cap);
        }
        FairModelEvent::ByzantineEquivocateCommit => {
            if !matches!(state.consensus, FairConsensusStage::CommitVote) {
                return None;
            }
            let _ = clamp_increment(&mut state.byz_equivocations, fault_counter_cap);
            let _ = clamp_increment(&mut state.commit_votes_byzantine, cfg.fault_budget);
            recompute_signed_stake(&mut state, cfg);
            if commit_votes_ready(&state, cfg) && da_ready_for_commit(&state, cfg) {
                apply_commit(&mut state);
            }
        }
        FairModelEvent::ByzantineInvalidSignatureVote => {
            let _ = clamp_increment(&mut state.byz_invalid_sig, fault_counter_cap);
        }
        FairModelEvent::ByzantineInvalidQcBroadcast => {
            let _ = clamp_increment(&mut state.byz_invalid_qc, fault_counter_cap);
        }
        FairModelEvent::ByzantineWithholdVotes => {
            let _ = clamp_increment(&mut state.byz_withholds, fault_counter_cap);
        }
        FairModelEvent::ByzantineStaleReplayModeMismatch => {
            let _ = clamp_increment(&mut state.byz_invalid_qc, fault_counter_cap);
        }
        FairModelEvent::RbcInit => {
            if !matches!(
                state.rbc,
                FairRbcState::Idle | FairRbcState::Withheld | FairRbcState::Corrupted
            ) {
                return None;
            }
            state.rbc = FairRbcState::Init;
            state.chunk_count = 0;
            state.ready_votes = 0;
            state.rbc_header_seen = true;
            state.rbc_chunk_digest_valid = true;
            state.rbc_ready_witnesses = 0;
        }
        FairModelEvent::RbcInitConflictingHeader => {
            if !matches!(
                state.rbc,
                FairRbcState::Init
                    | FairRbcState::Chunking
                    | FairRbcState::ChunksComplete
                    | FairRbcState::ReadyPartial
                    | FairRbcState::ReadyQuorum
            ) {
                return None;
            }
            state.rbc = FairRbcState::Corrupted;
            state.rbc_header_seen = false;
            state.rbc_chunk_digest_valid = false;
        }
        FairModelEvent::RbcChunkGood | FairModelEvent::RbcChunkReordered => {
            if !matches!(
                state.rbc,
                FairRbcState::Init | FairRbcState::Chunking | FairRbcState::Withheld
            ) {
                return None;
            }
            if !state.rbc_header_seen {
                return None;
            }
            if matches!(state.rbc, FairRbcState::Withheld) && !state.gst {
                return None;
            }
            let _ = clamp_increment(&mut state.chunk_count, cfg.rbc_required_chunks);
            state.rbc = if state.chunk_count >= cfg.rbc_required_chunks {
                FairRbcState::ChunksComplete
            } else {
                FairRbcState::Chunking
            };
            state.rbc_chunk_digest_valid = true;
        }
        FairModelEvent::RbcChunkCorrupt => {
            if !matches!(
                state.rbc,
                FairRbcState::Init
                    | FairRbcState::Chunking
                    | FairRbcState::ChunksComplete
                    | FairRbcState::ReadyPartial
                    | FairRbcState::ReadyQuorum
            ) {
                return None;
            }
            state.rbc = FairRbcState::Corrupted;
            state.rbc_chunk_digest_valid = false;
        }
        FairModelEvent::RbcChunkWithhold => {
            if !matches!(
                state.rbc,
                FairRbcState::Init
                    | FairRbcState::Chunking
                    | FairRbcState::ChunksComplete
                    | FairRbcState::ReadyPartial
            ) {
                return None;
            }
            state.rbc = FairRbcState::Withheld;
        }
        FairModelEvent::RbcReadyGood => {
            if !matches!(
                state.rbc,
                FairRbcState::ChunksComplete
                    | FairRbcState::ReadyPartial
                    | FairRbcState::ReadyQuorum
            ) {
                return None;
            }
            if !state.rbc_header_seen || !state.rbc_chunk_digest_valid {
                return None;
            }
            let _ = clamp_increment(&mut state.ready_votes, cfg.honest_validators);
            let _ = clamp_increment(&mut state.rbc_ready_witnesses, cfg.honest_validators);
            state.rbc = if state.ready_votes >= cfg.ready_quorum {
                FairRbcState::ReadyQuorum
            } else {
                FairRbcState::ReadyPartial
            };
        }
        FairModelEvent::RbcReadyConflicting => {
            if !matches!(
                state.rbc,
                FairRbcState::ChunksComplete
                    | FairRbcState::ReadyPartial
                    | FairRbcState::ReadyQuorum
            ) {
                return None;
            }
            state.rbc = FairRbcState::Corrupted;
            state.rbc_chunk_digest_valid = false;
        }
        FairModelEvent::RbcDeliverGood => {
            if !matches!(state.rbc, FairRbcState::ReadyQuorum) {
                return None;
            }
            if !state.rbc_header_seen || !state.rbc_chunk_digest_valid {
                return None;
            }
            if state.rbc_ready_witnesses < cfg.ready_quorum {
                return None;
            }
            state.rbc = FairRbcState::Delivered;
            if matches!(state.consensus, FairConsensusStage::CommitVote)
                && commit_votes_ready(&state, cfg)
                && da_ready_for_commit(&state, cfg)
            {
                apply_commit(&mut state);
            }
        }
        FairModelEvent::RbcDeliverInsufficientReady => {
            if state.ready_votes >= cfg.ready_quorum || matches!(state.rbc, FairRbcState::Delivered)
            {
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

fn canonicalize_state(mut state: FairModelState, cfg: &FairModelConfig) -> FairModelState {
    if !cfg.equivalence_reduced {
        return state;
    }

    let quorum_cap = cfg.commit_quorum;
    let fault_cap = cfg.fault_budget.saturating_add(1);
    state.prepare_votes = state.prepare_votes.min(quorum_cap);
    state.commit_votes_honest = state.commit_votes_honest.min(quorum_cap);
    state.commit_votes_byzantine = state.commit_votes_byzantine.min(cfg.fault_budget);
    state.stake_votes = state.stake_votes.min(cfg.total_stake);
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
    state.rbc_ready_witnesses = state.rbc_ready_witnesses.min(cfg.ready_quorum);
    state
}

fn validate_safety(state: FairModelState, cfg: &FairModelConfig, out: &mut Vec<String>) {
    if state.committed_conflict || state.committed_hash == 2 {
        out.push(format!(
            "conflicting commit marker observed at height={} view={}",
            state.height, state.view
        ));
    }

    if matches!(state.consensus, FairConsensusStage::Committed) {
        let total_votes = total_commit_votes(&state);
        if total_votes < cfg.commit_quorum {
            out.push(format!(
                "committed without commit quorum: collected={} required={}",
                total_votes, cfg.commit_quorum
            ));
        }
        if matches!(cfg.mode, ConsensusMode::Npos) && state.stake_votes < cfg.stake_commit_quorum {
            out.push(format!(
                "NPoS committed without stake quorum: stake_votes={} required={}",
                state.stake_votes, cfg.stake_commit_quorum
            ));
        }
        if cfg.da_enabled && !matches!(state.rbc, FairRbcState::Delivered) {
            out.push(format!(
                "committed in DA mode without RBC delivery (rbc={:?})",
                state.rbc
            ));
        }
    }

    if matches!(state.rbc, FairRbcState::Delivered) {
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
        if !state.rbc_header_seen || !state.rbc_chunk_digest_valid {
            out.push("RBC delivered while header/digest validity marker was false".to_string());
        }
        if state.rbc_ready_witnesses < cfg.ready_quorum {
            out.push(format!(
                "RBC delivered without ready witnesses: ready_witnesses={} required={}",
                state.rbc_ready_witnesses, cfg.ready_quorum
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
    state: FairModelState,
    cfg: &FairModelConfig,
    remaining_steps: u8,
    memo: &mut HashMap<(FairModelState, u8), bool>,
) -> bool {
    if state.height > 0 && matches!(state.consensus, FairConsensusStage::Committed) {
        return true;
    }
    if remaining_steps == 0 {
        return false;
    }

    if let Some(cached) = memo.get(&(state, remaining_steps)) {
        return *cached;
    }

    for event in FAIRNESS_EVENTS {
        let Some(mut next) = apply_event(state, event, cfg) else {
            continue;
        };
        next.gst = true;
        if can_reach_commit_with_honest_schedule(next, cfg, remaining_steps - 1, memo) {
            memo.insert((state, remaining_steps), true);
            return true;
        }
    }

    memo.insert((state, remaining_steps), false);
    false
}

fn fairness_enabled_mask(
    state: FairModelState,
    cfg: &FairModelConfig,
    cache: &mut HashMap<FairModelState, u16>,
) -> u16 {
    if let Some(mask) = cache.get(&state) {
        return *mask;
    }

    let mut mask = 0_u16;
    for (idx, event) in FAIRNESS_EVENTS.iter().copied().enumerate() {
        if apply_event(state, event, cfg).is_some() {
            mask |= 1_u16 << idx;
        }
    }
    cache.insert(state, mask);
    mask
}

fn is_honest_progress_event(event: FairModelEvent) -> bool {
    matches!(
        event,
        FairModelEvent::HonestPropose
            | FairModelEvent::HonestPrepareVote
            | FairModelEvent::HonestCommitVote
            | FairModelEvent::HonestNewViewVote
    )
}

fn advance_fairness_debts(
    state: FairModelState,
    next: FairModelState,
    chosen: FairModelEvent,
    debts: [u8; FAIRNESS_EVENT_COUNT],
    cfg: &FairModelConfig,
    cache: &mut HashMap<FairModelState, u16>,
) -> Option<[u8; FAIRNESS_EVENT_COUNT]> {
    let enabled_before = fairness_enabled_mask(state, cfg, cache);
    let enabled_after = fairness_enabled_mask(next, cfg, cache);
    let mut next_debts = debts;

    for (idx, fairness_event) in FAIRNESS_EVENTS.iter().copied().enumerate() {
        let bit = 1_u16 << idx;
        if enabled_before & bit == 0 {
            next_debts[idx] = 0;
            continue;
        }

        if chosen == fairness_event || enabled_after & bit == 0 {
            if chosen == fairness_event {
                next_debts[idx] = 0;
            } else if chosen == FairModelEvent::TimeoutTick
                && is_honest_progress_event(fairness_event)
            {
                // Do not let timeout-driven toggling discharge obligations for
                // enabled honest progress actions.
                next_debts[idx] = next_debts[idx].saturating_add(1);
                if next_debts[idx] > cfg.fairness_window {
                    return None;
                }
            } else {
                next_debts[idx] = 0;
            }
            continue;
        }

        next_debts[idx] = next_debts[idx].saturating_add(1);
        if next_debts[idx] > cfg.fairness_window {
            return None;
        }
    }

    Some(next_debts)
}

fn fair_scheduler_allows_event(
    state: FairModelState,
    event: FairModelEvent,
    cfg: &FairModelConfig,
) -> bool {
    if event != FairModelEvent::TimeoutTick || !state.gst {
        return true;
    }

    for fair_event in FAIRNESS_EVENTS {
        if fair_event == FairModelEvent::TimeoutTick {
            continue;
        }
        if apply_event(state, fair_event, cfg).is_some() {
            return false;
        }
    }
    true
}

fn adversary_can_avoid_commit_under_fairness(
    state: FairModelState,
    cfg: &FairModelConfig,
    remaining_steps: u8,
    debts: [u8; FAIRNESS_EVENT_COUNT],
    memo: &mut HashMap<FairAdversaryKey, bool>,
    mask_cache: &mut HashMap<FairModelState, u16>,
) -> bool {
    if state.height > 0 && matches!(state.consensus, FairConsensusStage::Committed) {
        return false;
    }
    if remaining_steps == 0 {
        return true;
    }

    let key = FairAdversaryKey {
        state,
        debts,
        remaining_steps,
    };
    if let Some(cached) = memo.get(&key) {
        return *cached;
    }

    for event in FAIR_ALL_EVENTS {
        if !fair_scheduler_allows_event(state, *event, cfg) {
            continue;
        }
        let Some(mut next) = apply_event(state, *event, cfg) else {
            continue;
        };
        next.gst = true;
        let Some(next_debts) = advance_fairness_debts(state, next, *event, debts, cfg, mask_cache)
        else {
            continue;
        };
        if adversary_can_avoid_commit_under_fairness(
            next,
            cfg,
            remaining_steps - 1,
            next_debts,
            memo,
            mask_cache,
        ) {
            memo.insert(key, true);
            return true;
        }
    }

    memo.insert(key, false);
    false
}

fn adversary_can_avoid_commit_under_fairness_with_events(
    state: FairModelState,
    cfg: &FairModelConfig,
    remaining_steps: u8,
    debts: [u8; FAIRNESS_EVENT_COUNT],
    memo: &mut HashMap<FairAdversaryKey, bool>,
    events: &[FairModelEvent],
    mask_cache: &mut HashMap<FairModelState, u16>,
) -> bool {
    if state.height > 0 && matches!(state.consensus, FairConsensusStage::Committed) {
        return false;
    }
    if remaining_steps == 0 {
        return true;
    }

    let key = FairAdversaryKey {
        state,
        debts,
        remaining_steps,
    };
    if let Some(cached) = memo.get(&key) {
        return *cached;
    }

    for event in events {
        if !fair_scheduler_allows_event(state, *event, cfg) {
            continue;
        }
        let Some(mut next) = apply_event(state, *event, cfg) else {
            continue;
        };
        next.gst = true;
        let Some(next_debts) = advance_fairness_debts(state, next, *event, debts, cfg, mask_cache)
        else {
            continue;
        };
        if adversary_can_avoid_commit_under_fairness_with_events(
            next,
            cfg,
            remaining_steps - 1,
            next_debts,
            memo,
            events,
            mask_cache,
        ) {
            memo.insert(key, true);
            return true;
        }
    }

    memo.insert(key, false);
    false
}

fn adversary_can_avoid_commit_without_fairness(
    state: FairModelState,
    cfg: &FairModelConfig,
    remaining_steps: u8,
    memo: &mut HashMap<(FairModelState, u8), bool>,
) -> bool {
    if state.height > 0 && matches!(state.consensus, FairConsensusStage::Committed) {
        return false;
    }
    if remaining_steps == 0 {
        return true;
    }

    if let Some(cached) = memo.get(&(state, remaining_steps)) {
        return *cached;
    }

    for event in FAIR_ALL_EVENTS {
        let Some(mut next) = apply_event(state, *event, cfg) else {
            continue;
        };
        next.gst = true;
        if adversary_can_avoid_commit_without_fairness(next, cfg, remaining_steps - 1, memo) {
            memo.insert((state, remaining_steps), true);
            return true;
        }
    }

    memo.insert((state, remaining_steps), false);
    false
}

fn validate_liveness(
    visited: &HashSet<FairModelState>,
    cfg: &FairModelConfig,
    out: &mut Vec<String>,
) {
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

    if matches!(cfg.mode, ConsensusMode::Npos) && cfg.honest_total_stake < cfg.stake_commit_quorum {
        out.push(format!(
            "invalid NPoS config for liveness: honest_stake={} stake_quorum={}",
            cfg.honest_total_stake, cfg.stake_commit_quorum
        ));
        return;
    }
    if matches!(cfg.mode, ConsensusMode::Npos)
        && cfg.byzantine_total_stake.saturating_mul(3) >= cfg.total_stake
    {
        out.push(format!(
            "invalid NPoS config for liveness: byzantine_stake={} total_stake={}",
            cfg.byzantine_total_stake, cfg.total_stake
        ));
        return;
    }

    let mut memo = HashMap::new();
    let local_horizon = cfg
        .view_change_quorum
        .saturating_add(cfg.commit_quorum.saturating_mul(2))
        .saturating_add(cfg.ready_quorum)
        .saturating_add(cfg.rbc_required_chunks)
        .saturating_add(6)
        .min(cfg.max_steps);

    for state in visited {
        if !state.gst {
            continue;
        }
        if state.height > 0 && matches!(state.consensus, FairConsensusStage::Committed) {
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

fn validate_fair_liveness(
    visited: &HashSet<FairModelState>,
    cfg: &FairModelConfig,
    out: &mut Vec<String>,
) {
    if !cfg.enable_fair_liveness {
        return;
    }

    if cfg.fairness_window == 0 {
        out.push("fairness window must be > 0 when fair liveness is enabled".to_string());
        return;
    }

    let horizon = cfg.fairness_horizon.min(cfg.max_steps);
    let mut checked = 0usize;
    let mut memo = HashMap::new();
    let mut honest_reachability_memo = HashMap::new();
    let mut mask_cache = HashMap::new();
    for state in visited {
        if checked >= cfg.fairness_state_budget {
            break;
        }
        if !state.gst {
            continue;
        }
        if state.height > 0 && matches!(state.consensus, FairConsensusStage::Committed) {
            continue;
        }
        if matches!(state.rbc, FairRbcState::Corrupted | FairRbcState::Withheld) {
            continue;
        }
        if state.view >= cfg.max_views {
            continue;
        }
        if !can_reach_commit_with_honest_schedule(
            *state,
            cfg,
            horizon,
            &mut honest_reachability_memo,
        ) {
            continue;
        }

        if adversary_can_avoid_commit_under_fairness(
            *state,
            cfg,
            horizon,
            [0; FAIRNESS_EVENT_COUNT],
            &mut memo,
            &mut mask_cache,
        ) {
            out.push(format!(
                "fair scheduler can avoid commit in horizon from state: height={} view={} consensus={:?} recovery={:?} rbc={:?}",
                state.height, state.view, state.consensus, state.recovery, state.rbc
            ));
            if out.len() >= 12 {
                break;
            }
        }
        checked = checked.saturating_add(1);
    }
}

fn run_bounded_exhaustive(
    cfg: FairModelConfig,
    events: &[FairModelEvent],
) -> FairExplorationReport {
    let mut report = FairExplorationReport::default();
    let mut queue = VecDeque::new();
    let mut visited = HashSet::new();

    let init = canonicalize_state(FairModelState::initial(), &cfg);
    queue.push_back((init, 0_u8));
    visited.insert(init);

    while let Some((state, depth)) = queue.pop_front() {
        report.states_explored = report.states_explored.saturating_add(1);
        report.max_height = report.max_height.max(state.height);
        if state.height > 0 && matches!(state.consensus, FairConsensusStage::Committed) {
            report.commits_seen = report.commits_seen.saturating_add(1);
        }
        if matches!(state.recovery, FairRecoveryStage::ViewChangeEscalation) {
            report.recovery_escalations_seen = report.recovery_escalations_seen.saturating_add(1);
        }
        if matches!(state.rbc, FairRbcState::Delivered) {
            report.rbc_deliveries_seen = report.rbc_deliveries_seen.saturating_add(1);
        }
        if matches!(cfg.mode, ConsensusMode::Npos)
            && total_commit_votes(&state) >= cfg.commit_quorum
            && state.stake_votes < cfg.stake_commit_quorum
        {
            report.stake_gap_states_seen = report.stake_gap_states_seen.saturating_add(1);
        }

        validate_safety(state, &cfg, &mut report.safety_violations);

        // Treat committed states as terminal in this bounded model:
        // post-commit branching does not add safety/liveness signal but
        // explodes the state space.
        if state.height > 0 && matches!(state.consensus, FairConsensusStage::Committed) {
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
    validate_fair_liveness(&visited, &cfg, &mut report.fairness_violations);
    report
}

fn apply_sequence(cfg: FairModelConfig, sequence: &[FairModelEvent]) -> FairModelState {
    let mut state = FairModelState::initial();
    for event in sequence {
        if let Some(next) = apply_event(state, *event, &cfg) {
            state = next;
        }
    }
    state
}

async fn fair_model_config_from_actor(
    mode: ConsensusMode,
    validator_count: usize,
    equivalence_reduced: bool,
    enable_fair_liveness: bool,
) -> (FairModelConfig, TestActorHarness) {
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

    let (
        honest_stake_by_signer,
        byzantine_stake_by_signer,
        honest_total_stake,
        byzantine_total_stake,
        stake_commit_quorum,
    ) = build_heterogeneous_stake_profile(n, f);
    let total_stake = honest_total_stake.saturating_add(byzantine_total_stake);

    let max_steps = if validator_count <= 7 { 26 } else { 36 };
    let cfg = FairModelConfig {
        mode,
        validator_count: n,
        fault_budget: f,
        honest_validators: honest,
        commit_quorum,
        view_change_quorum,
        ready_quorum: commit_quorum,
        stake_commit_quorum,
        honest_stake_by_signer,
        byzantine_stake_by_signer,
        honest_total_stake,
        byzantine_total_stake,
        total_stake,
        max_views: 5,
        max_steps,
        gst_step: 4,
        rbc_required_chunks: 2,
        da_enabled: true,
        equivalence_reduced,
        enable_fair_liveness,
        fairness_window: 3,
        fairness_horizon: if validator_count <= 7 { 14 } else { 16 },
        fairness_state_budget: if validator_count <= 7 { 128 } else { 64 },
    };

    if matches!(mode, ConsensusMode::Npos) {
        assert!(
            u32::from(cfg.honest_total_stake).saturating_mul(3)
                >= u32::from(cfg.total_stake).saturating_mul(2),
            "fair model NPoS config must satisfy >=2/3 honest stake"
        );
    }

    (cfg, harness)
}

fn assert_clean_report(report: &FairExplorationReport) {
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
    assert!(
        report.fairness_violations.is_empty(),
        "fairness violations: {:?}",
        report.fairness_violations
    );
}

fn runtime_snapshot_from_model(
    cfg: &FairModelConfig,
) -> (
    Vec<PeerId>,
    crate::sumeragi::stake_snapshot::CommitStakeSnapshot,
    Vec<PeerId>,
    Vec<PeerId>,
) {
    let mut roster = Vec::with_capacity(usize::from(cfg.validator_count));
    for idx in 0..usize::from(cfg.validator_count) {
        let seed = format!("fair-model-stake-peer-{idx}");
        let kp = deterministic_keypair(seed.as_bytes(), Algorithm::BlsNormal);
        roster.push(PeerId::new(kp.public_key().clone()));
    }

    let byz_len = usize::from(cfg.fault_budget);
    let byz_peers = roster.iter().take(byz_len).cloned().collect::<Vec<_>>();
    let honest_peers = roster.iter().skip(byz_len).cloned().collect::<Vec<_>>();

    let mut entries = Vec::with_capacity(roster.len());
    for (idx, peer_id) in roster.iter().enumerate() {
        let stake = if idx < byz_len {
            cfg.byzantine_stake_by_signer[idx]
        } else {
            cfg.honest_stake_by_signer[idx - byz_len]
        };
        entries.push(crate::sumeragi::stake_snapshot::CommitStakeSnapshotEntry {
            peer_id: peer_id.clone(),
            stake: Numeric::from(u64::from(stake)),
        });
    }
    let snapshot = crate::sumeragi::stake_snapshot::CommitStakeSnapshot {
        validator_set_hash: HashOf::new(&roster),
        entries,
    };

    (roster, snapshot, byz_peers, honest_peers)
}

#[tokio::test(flavor = "current_thread")]
async fn fair_model_permissioned_n4_exhaustive_safety_liveness_and_fairness() {
    let (cfg, harness) =
        fair_model_config_from_actor(ConsensusMode::Permissioned, 4, false, false).await;
    let report = run_bounded_exhaustive(cfg, FAIR_ALL_EVENTS);
    assert_clean_report(&report);
    assert!(report.commits_seen > 0, "model should commit at least once");
    assert!(
        report.rbc_deliveries_seen > 0,
        "model should deliver RBC payloads"
    );
    drop(harness);
}

#[tokio::test(flavor = "current_thread")]
async fn fair_model_npos_n7_exhaustive_safety_liveness_and_fairness() {
    let (cfg, harness) = fair_model_config_from_actor(ConsensusMode::Npos, 7, true, false).await;
    let report = run_bounded_exhaustive(cfg, FAIR_ALL_EVENTS);
    assert_clean_report(&report);
    assert!(report.commits_seen > 0, "model should commit at least once");
    assert!(
        report.stake_gap_states_seen > 0,
        "NPoS exploration should include weighted stake-gap states"
    );
    drop(harness);
}

#[tokio::test(flavor = "current_thread")]
async fn fair_model_fairness_blocks_starvation_that_unfair_scheduler_allows() {
    let (cfg, harness) =
        fair_model_config_from_actor(ConsensusMode::Permissioned, 4, false, true).await;

    let mut state = FairModelState::initial();
    state.gst = true;
    state.consensus = FairConsensusStage::CommitVote;
    state.rbc = FairRbcState::Delivered;
    state.rbc_header_seen = true;
    state.rbc_chunk_digest_valid = true;
    state.chunk_count = cfg.rbc_required_chunks;
    state.ready_votes = cfg.ready_quorum;
    state.rbc_ready_witnesses = cfg.ready_quorum;
    state.commit_votes_honest = cfg.commit_quorum.saturating_sub(1);
    recompute_signed_stake(&mut state, &cfg);

    let mut unfair_memo = HashMap::new();
    assert!(
        adversary_can_avoid_commit_without_fairness(state, &cfg, 8, &mut unfair_memo),
        "without fairness constraints, adversarial scheduling can starve commit"
    );

    let mut fair_memo = HashMap::new();
    let mut mask_cache = HashMap::new();
    let constrained_events = [
        FairModelEvent::HonestCommitVote,
        FairModelEvent::ByzantineInvalidSignatureVote,
        FairModelEvent::ByzantineInvalidQcBroadcast,
    ];
    assert!(
        !adversary_can_avoid_commit_under_fairness_with_events(
            state,
            &cfg,
            8,
            [0; FAIRNESS_EVENT_COUNT],
            &mut fair_memo,
            &constrained_events,
            &mut mask_cache,
        ),
        "with weak fairness obligations, adversarial scheduling should not avoid commit"
    );

    drop(harness);
}

#[tokio::test(flavor = "current_thread")]
async fn fair_model_npos_stake_quorum_parity_matches_runtime_snapshot_checker() {
    let (cfg, harness) = fair_model_config_from_actor(ConsensusMode::Npos, 7, true, false).await;
    let (roster, snapshot, byz_peers, honest_peers) = runtime_snapshot_from_model(&cfg);

    for honest_votes in 0..=cfg.honest_validators {
        for byz_votes in 0..=cfg.fault_budget {
            let model_signed = signed_stake_from_counts(&cfg, honest_votes, byz_votes);
            let model_quorum = model_signed >= cfg.stake_commit_quorum;

            let mut signers = BTreeSet::new();
            for peer in byz_peers.iter().take(usize::from(byz_votes)) {
                signers.insert(peer.clone());
            }
            for peer in honest_peers.iter().take(usize::from(honest_votes)) {
                signers.insert(peer.clone());
            }

            let runtime_quorum =
                crate::sumeragi::stake_snapshot::stake_quorum_reached_for_snapshot(
                    &snapshot, &roster, &signers,
                )
                .expect("runtime stake quorum check should succeed");

            assert_eq!(
                model_quorum, runtime_quorum,
                "stake quorum mismatch at honest_votes={} byz_votes={}",
                honest_votes, byz_votes
            );
        }
    }

    drop(harness);
}

#[tokio::test(flavor = "current_thread")]
async fn fair_model_rbc_causality_requires_header_chunks_and_ready_witnesses() {
    let (cfg, harness) =
        fair_model_config_from_actor(ConsensusMode::Permissioned, 4, false, true).await;

    let invalid_sequence = [
        FairModelEvent::HonestPropose,
        FairModelEvent::RbcReadyGood,
        FairModelEvent::RbcDeliverInsufficientReady,
        FairModelEvent::RbcDeliverGood,
        FairModelEvent::HonestPrepareVote,
        FairModelEvent::HonestPrepareVote,
        FairModelEvent::HonestPrepareVote,
        FairModelEvent::HonestCommitVote,
        FairModelEvent::HonestCommitVote,
        FairModelEvent::ByzantineEquivocateCommit,
    ];
    let invalid_state = apply_sequence(cfg, &invalid_sequence);
    assert!(
        !matches!(invalid_state.rbc, FairRbcState::Delivered),
        "RBC must not deliver without chunk and ready evidence"
    );
    assert!(
        !matches!(invalid_state.consensus, FairConsensusStage::Committed),
        "consensus must not commit when RBC causality is unsatisfied"
    );

    let valid_sequence = [
        FairModelEvent::HonestPropose,
        FairModelEvent::RbcInit,
        FairModelEvent::RbcChunkGood,
        FairModelEvent::RbcChunkGood,
        FairModelEvent::RbcReadyGood,
        FairModelEvent::RbcReadyGood,
        FairModelEvent::RbcReadyGood,
        FairModelEvent::RbcDeliverGood,
        FairModelEvent::HonestPrepareVote,
        FairModelEvent::HonestPrepareVote,
        FairModelEvent::HonestPrepareVote,
        FairModelEvent::HonestCommitVote,
        FairModelEvent::HonestCommitVote,
        FairModelEvent::ByzantineEquivocateCommit,
    ];
    let valid_state = apply_sequence(cfg, &valid_sequence);
    assert!(
        matches!(valid_state.rbc, FairRbcState::Delivered),
        "RBC should deliver when header/chunk/ready evidence is complete"
    );
    assert!(
        matches!(valid_state.consensus, FairConsensusStage::Committed),
        "consensus should commit once RBC and quorum conditions are met"
    );

    drop(harness);
}

#[tokio::test(flavor = "current_thread")]
async fn fair_scheduler_allows_timeout_before_gst_but_blocks_it_after_gst_when_progress_is_enabled()
{
    let (cfg, harness) =
        fair_model_config_from_actor(ConsensusMode::Permissioned, 4, false, true).await;

    let mut state = FairModelState::initial();
    state.consensus = FairConsensusStage::Prepare;
    state.recovery = FairRecoveryStage::RefreshTopology;
    state.gst = false;
    assert!(
        fair_scheduler_allows_event(state, FairModelEvent::TimeoutTick, &cfg),
        "before GST, timeout should remain schedulable"
    );

    state.gst = true;
    assert!(
        !fair_scheduler_allows_event(state, FairModelEvent::TimeoutTick, &cfg),
        "after GST, timeout should be blocked while honest progress is enabled"
    );

    drop(harness);
}

#[tokio::test(flavor = "current_thread")]
async fn fair_scheduler_timeout_step_keeps_honest_progress_debt_when_it_preempts_enabled_event() {
    let (cfg, harness) =
        fair_model_config_from_actor(ConsensusMode::Permissioned, 4, false, true).await;

    let mut state = FairModelState::initial();
    state.consensus = FairConsensusStage::Prepare;
    state.recovery = FairRecoveryStage::RefreshTopology;
    state.gst = true;
    let next = apply_event(state, FairModelEvent::TimeoutTick, &cfg)
        .expect("timeout should transition state from Prepare to NewView");

    let mut mask_cache = HashMap::new();
    let debts = advance_fairness_debts(
        state,
        next,
        FairModelEvent::TimeoutTick,
        [0; FAIRNESS_EVENT_COUNT],
        &cfg,
        &mut mask_cache,
    )
    .expect("timeout transition should stay within fairness debt window");
    let prepare_idx = FAIRNESS_EVENTS
        .iter()
        .position(|event| *event == FairModelEvent::HonestPrepareVote)
        .expect("fairness events must include HonestPrepareVote");
    assert_eq!(
        debts[prepare_idx], 1,
        "timeout must not clear debt for an enabled honest progress action"
    );

    drop(harness);
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "deep topology coverage"]
async fn fair_model_permissioned_n10_equivalence_deep_safety_liveness_fairness() {
    let (cfg, harness) =
        fair_model_config_from_actor(ConsensusMode::Permissioned, 10, true, true).await;
    let report = run_bounded_exhaustive(cfg, FAIR_ALL_EVENTS);
    assert_clean_report(&report);
    assert!(report.commits_seen > 0, "deep model should still commit");
    drop(harness);
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "deep topology coverage"]
async fn fair_model_npos_n13_equivalence_deep_safety_liveness_fairness() {
    let (cfg, harness) = fair_model_config_from_actor(ConsensusMode::Npos, 13, true, true).await;
    let report = run_bounded_exhaustive(cfg, FAIR_ALL_EVENTS);
    assert_clean_report(&report);
    assert!(report.commits_seen > 0, "deep model should still commit");
    assert!(
        report.stake_gap_states_seen > 0,
        "deep NPoS run should include weighted stake-gap states"
    );
    drop(harness);
}
