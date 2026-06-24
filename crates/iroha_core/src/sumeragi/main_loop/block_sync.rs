//! Block sync and missing-block request handlers.

use std::collections::{BTreeMap, BTreeSet, btree_map::Entry};
use std::sync::Arc;
use std::time::Instant;

use iroha_logger::prelude::*;
use norito::codec::Encode as _;

use crate::sumeragi::message::BlockMessageWire;

use super::locked_qc::qc_satisfies_locked_with_lookup;
use super::message::FetchPendingBlockPriority;
use super::proposal_handlers::BlockSyncRecoveryMode;
use super::*;

fn allow_uncertified_block_sync_roster(
    block_height: u64,
    local_height: u64,
    requested_missing_block: bool,
) -> bool {
    requested_missing_block || block_height == local_height.saturating_add(1)
}

fn should_mark_block_sync_implicit_recovery(
    da_enabled: bool,
    requested_missing_block: bool,
    block_known_locally: bool,
    block_height: u64,
    local_height: u64,
    implicit_frontier_recovery_allowed: bool,
) -> bool {
    da_enabled
        && !requested_missing_block
        && !block_known_locally
        && block_height <= local_height.saturating_add(1)
        && implicit_frontier_recovery_allowed
}

fn should_note_block_sync_vote_placeholder(
    has_commit_votes: bool,
    incoming_qc_present: bool,
    validator_checkpoint_present: bool,
    exact_contiguous_frontier: bool,
    block_known_locally: bool,
    requested_missing_block: bool,
) -> bool {
    has_commit_votes
        && !incoming_qc_present
        && !validator_checkpoint_present
        && exact_contiguous_frontier
        && !block_known_locally
        && !requested_missing_block
}

fn block_sync_stale_view_has_commit_evidence(
    incoming_qc_present: bool,
    validator_checkpoint_present: bool,
    has_commit_votes: bool,
) -> bool {
    incoming_qc_present || validator_checkpoint_present || has_commit_votes
}

fn block_sync_stale_view_should_drop(
    stale_view: bool,
    requested_missing_block: bool,
    block_known_locally: bool,
    has_commit_evidence: bool,
) -> bool {
    stale_view && !requested_missing_block && !block_known_locally && !has_commit_evidence
}

fn block_sync_stale_view_drop_record(
    drop_stale_view: bool,
) -> Option<(
    super::status::ConsensusMessageKind,
    super::status::ConsensusMessageOutcome,
    super::status::ConsensusMessageReason,
)> {
    drop_stale_view.then_some((
        super::status::ConsensusMessageKind::BlockSyncUpdate,
        super::status::ConsensusMessageOutcome::Dropped,
        super::status::ConsensusMessageReason::StaleView,
    ))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BlockSyncFetchResponseDeferralMessage {
    BlockCreated,
    BlockSyncUpdate {
        commit_qc_present: bool,
        validator_checkpoint_present: bool,
    },
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BlockSyncFetchResponseDeferralCommittedHash {
    Matches,
    Mismatch,
    Unknown,
}

fn block_sync_fetch_response_deferral_committed_hash(
    committed_hash: Option<HashOf<BlockHeader>>,
    block_hash: HashOf<BlockHeader>,
) -> BlockSyncFetchResponseDeferralCommittedHash {
    match committed_hash {
        Some(committed_hash) if committed_hash == block_hash => {
            BlockSyncFetchResponseDeferralCommittedHash::Matches
        }
        Some(_) => BlockSyncFetchResponseDeferralCommittedHash::Mismatch,
        None => BlockSyncFetchResponseDeferralCommittedHash::Unknown,
    }
}

fn block_sync_fetch_response_deferral_message(
    msg: &BlockMessage,
) -> BlockSyncFetchResponseDeferralMessage {
    match msg {
        BlockMessage::BlockCreated(_) => BlockSyncFetchResponseDeferralMessage::BlockCreated,
        BlockMessage::BlockSyncUpdate(update) => {
            BlockSyncFetchResponseDeferralMessage::BlockSyncUpdate {
                commit_qc_present: update.commit_qc.is_some(),
                validator_checkpoint_present: update.validator_checkpoint.is_some(),
            }
        }
        _ => BlockSyncFetchResponseDeferralMessage::Other,
    }
}

fn should_defer_canonical_committed_fetch_response_shape(
    block_height: u64,
    local_committed_height: u64,
    committed_hash: BlockSyncFetchResponseDeferralCommittedHash,
    message: BlockSyncFetchResponseDeferralMessage,
) -> bool {
    block_height == local_committed_height
        && matches!(
            committed_hash,
            BlockSyncFetchResponseDeferralCommittedHash::Matches
        )
        && match message {
            BlockSyncFetchResponseDeferralMessage::BlockCreated => true,
            BlockSyncFetchResponseDeferralMessage::BlockSyncUpdate {
                commit_qc_present,
                validator_checkpoint_present,
            } => !commit_qc_present && !validator_checkpoint_present,
            BlockSyncFetchResponseDeferralMessage::Other => false,
        }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BlockSyncFetchBlockBodyHandleDecision {
    dispatch: bool,
    pending_stash: bool,
    frontier_stash: bool,
    remove_requester: bool,
    deferred_record: bool,
    dedup_release_count: u8,
    dispatch_uses_plain_fallback_helper: bool,
}

fn block_sync_fetch_block_body_handle_decision(
    local_block_found: bool,
    identity_matches: bool,
    should_defer_exact_local: bool,
    frontier_matches: bool,
    window_allows: bool,
) -> BlockSyncFetchBlockBodyHandleDecision {
    let exact_local = local_block_found && identity_matches;
    let dispatch = exact_local && !should_defer_exact_local;
    let pending_stash = if exact_local && should_defer_exact_local {
        true
    } else {
        !exact_local && !frontier_matches && window_allows
    };
    let frontier_stash = !exact_local && frontier_matches;
    BlockSyncFetchBlockBodyHandleDecision {
        dispatch,
        pending_stash,
        frontier_stash,
        remove_requester: dispatch,
        deferred_record: !dispatch,
        dedup_release_count: 1,
        dispatch_uses_plain_fallback_helper: dispatch,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BlockBodyResponsePayloadIdentity {
    block_hash: HashOf<BlockHeader>,
    height: u64,
    view: u64,
    payload_hash: Hash,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BlockBodyRepairGateDecision {
    identity_matches_response: bool,
    payload_hash_matches_expected: bool,
    allow: bool,
}

fn block_body_repair_gate_decision(
    runtime_da_enabled: bool,
    frontier_slot_exact: bool,
    session_exists: bool,
    session_metadata_matches: bool,
    session_has_authoritative_payload: bool,
    expected_payload_hash: Option<Hash>,
    response_block_hash: HashOf<BlockHeader>,
    response_height: u64,
    response_view: u64,
    body_identity: BlockBodyResponsePayloadIdentity,
) -> BlockBodyRepairGateDecision {
    let identity_matches_response = body_identity.block_hash == response_block_hash
        && body_identity.height == response_height
        && body_identity.view == response_view;
    let payload_hash_matches_expected =
        expected_payload_hash.is_some_and(|expected| body_identity.payload_hash == expected);
    let payload_hash_acceptable = expected_payload_hash.is_none() || payload_hash_matches_expected;
    BlockBodyRepairGateDecision {
        identity_matches_response,
        payload_hash_matches_expected,
        allow: runtime_da_enabled
            && frontier_slot_exact
            && session_exists
            && session_metadata_matches
            && !session_has_authoritative_payload
            && identity_matches_response
            && payload_hash_acceptable,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BlockBodyRequestStashWindowDecision {
    effective_margin: u64,
    lower_bound: u64,
    upper_bound: u64,
    stash: bool,
}

fn block_body_request_stash_window_decision(
    committed_height: u64,
    raw_margin: u64,
    request_height: u64,
) -> BlockBodyRequestStashWindowDecision {
    let effective_margin = raw_margin.max(1);
    let lower_bound = committed_height.saturating_add(1);
    let upper_bound = committed_height.saturating_add(effective_margin);
    BlockBodyRequestStashWindowDecision {
        effective_margin,
        lower_bound,
        upper_bound,
        stash: request_height >= lower_bound && request_height <= upper_bound,
    }
}

fn same_height_block_body_repair_source_matches(
    source_exists: bool,
    phase_is_commit: bool,
    block_hash_matches: bool,
    height_matches: bool,
    view_matches: bool,
    actionable_dependency: bool,
) -> bool {
    source_exists
        && phase_is_commit
        && block_hash_matches
        && height_matches
        && view_matches
        && actionable_dependency
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SameHeightBlockBodyRepairDecision {
    pending_source: bool,
    deferred_source: bool,
    active_commit_qc_repair: bool,
    allow: bool,
}

fn same_height_block_body_repair_decision(
    frontier_slot_exact: bool,
    pending_source: bool,
    deferred_source: bool,
    active_commit_qc_repair: bool,
) -> SameHeightBlockBodyRepairDecision {
    SameHeightBlockBodyRepairDecision {
        pending_source,
        deferred_source,
        active_commit_qc_repair,
        allow: frontier_slot_exact
            && (pending_source || deferred_source || active_commit_qc_repair),
    }
}

fn block_body_repair_epoch_deferred_source(
    source_exists: bool,
    phase_is_commit: bool,
    block_hash_matches: bool,
    height_matches: bool,
    view_matches: bool,
    epoch: u64,
) -> Option<u64> {
    (source_exists && phase_is_commit && block_hash_matches && height_matches && view_matches)
        .then_some(epoch)
}

fn block_body_repair_epoch_pending_source(
    source_exists: bool,
    commit_qc_observed: bool,
    commit_qc_epoch: Option<u64>,
) -> Option<u64> {
    (source_exists && commit_qc_observed)
        .then_some(commit_qc_epoch)
        .flatten()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BlockBodyRepairEpochSource {
    Cache,
    Deferred,
    Pending,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BlockBodyRepairEpochDecision {
    source: BlockBodyRepairEpochSource,
    epoch: Option<u64>,
}

fn block_body_repair_epoch_decision(
    cache_epoch: Option<u64>,
    deferred_epoch: Option<u64>,
    pending_epoch: Option<u64>,
) -> BlockBodyRepairEpochDecision {
    if let Some(epoch) = cache_epoch {
        BlockBodyRepairEpochDecision {
            source: BlockBodyRepairEpochSource::Cache,
            epoch: Some(epoch),
        }
    } else if let Some(epoch) = deferred_epoch {
        BlockBodyRepairEpochDecision {
            source: BlockBodyRepairEpochSource::Deferred,
            epoch: Some(epoch),
        }
    } else if let Some(epoch) = pending_epoch {
        BlockBodyRepairEpochDecision {
            source: BlockBodyRepairEpochSource::Pending,
            epoch: Some(epoch),
        }
    } else {
        BlockBodyRepairEpochDecision {
            source: BlockBodyRepairEpochSource::None,
            epoch: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DirectCommitQcTopologySource {
    Primary,
    Fallback,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DirectCommitQcForBlockResult {
    Cache,
    World,
    Formed,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DirectCommitQcForBlockDecision {
    world_consulted: bool,
    topology_source: DirectCommitQcTopologySource,
    try_form: bool,
    try_phase_commit: bool,
    try_subject_block: bool,
    result: DirectCommitQcForBlockResult,
}

fn direct_commit_qc_for_block_decision(
    cache_available: bool,
    world_available: bool,
    primary_topology_available: bool,
    fallback_topology_available: bool,
    pending_commit_votes: usize,
    min_votes_for_commit: usize,
    formed_qc_available: bool,
) -> DirectCommitQcForBlockDecision {
    let world_consulted = !cache_available;
    let topology_source = if cache_available || world_available {
        DirectCommitQcTopologySource::None
    } else if primary_topology_available {
        DirectCommitQcTopologySource::Primary
    } else if fallback_topology_available {
        DirectCommitQcTopologySource::Fallback
    } else {
        DirectCommitQcTopologySource::None
    };
    let votes_meet_floor = pending_commit_votes >= min_votes_for_commit.max(1);
    let try_form = !cache_available
        && !world_available
        && matches!(
            topology_source,
            DirectCommitQcTopologySource::Primary | DirectCommitQcTopologySource::Fallback
        )
        && votes_meet_floor;
    let result = if cache_available {
        DirectCommitQcForBlockResult::Cache
    } else if world_available {
        DirectCommitQcForBlockResult::World
    } else if try_form && formed_qc_available {
        DirectCommitQcForBlockResult::Formed
    } else {
        DirectCommitQcForBlockResult::None
    };
    DirectCommitQcForBlockDecision {
        world_consulted,
        topology_source,
        try_form,
        try_phase_commit: try_form,
        try_subject_block: try_form,
        result,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BlockBodyDirectCommitQcSource {
    Embedded,
    Checkpoint,
    Local,
    None,
}

fn block_body_direct_commit_qc_update_source(
    identity_matches: bool,
    embedded_commit_qc: bool,
    checkpoint_commit_qc: bool,
    local_direct_qc: bool,
) -> BlockBodyDirectCommitQcSource {
    if !identity_matches {
        BlockBodyDirectCommitQcSource::None
    } else if embedded_commit_qc {
        BlockBodyDirectCommitQcSource::Embedded
    } else if checkpoint_commit_qc {
        BlockBodyDirectCommitQcSource::Checkpoint
    } else if local_direct_qc {
        BlockBodyDirectCommitQcSource::Local
    } else {
        BlockBodyDirectCommitQcSource::None
    }
}

fn block_body_direct_commit_qc_created_source(
    identity_matches: bool,
    local_direct_qc: bool,
) -> BlockBodyDirectCommitQcSource {
    if identity_matches && local_direct_qc {
        BlockBodyDirectCommitQcSource::Local
    } else {
        BlockBodyDirectCommitQcSource::None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DetachedBlockBodyCommitQcDecision {
    handle_qc: bool,
    clear_missing_commit_qc: bool,
}

fn detached_block_body_commit_qc_decision(
    has_qc: bool,
    cached_before: bool,
    cached_after_handle: bool,
) -> DetachedBlockBodyCommitQcDecision {
    let handle_qc = has_qc && !cached_before;
    DetachedBlockBodyCommitQcDecision {
        handle_qc,
        clear_missing_commit_qc: has_qc && (cached_before || (handle_qc && cached_after_handle)),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BlockBodyResponseDispatchDecision {
    created_companion: bool,
    plain_fallback: bool,
    response: bool,
    qc_companion: bool,
    pos_created: u8,
    pos_plain: u8,
    pos_response: u8,
    pos_qc: u8,
    all_bypass: bool,
}

fn block_body_response_dispatch_decision(
    is_sync: bool,
    created_companion_under_cap: bool,
    direct_qc_available: bool,
) -> BlockBodyResponseDispatchDecision {
    let pos_created = if created_companion_under_cap { 1 } else { 0 };
    let pos_plain = if is_sync {
        if created_companion_under_cap { 2 } else { 1 }
    } else {
        0
    };
    let pos_response = 1 + u8::from(created_companion_under_cap) + u8::from(is_sync);
    let pos_qc = if direct_qc_available {
        pos_response + 1
    } else {
        0
    };
    BlockBodyResponseDispatchDecision {
        created_companion: created_companion_under_cap,
        plain_fallback: is_sync,
        response: true,
        qc_companion: direct_qc_available,
        pos_created,
        pos_plain,
        pos_response,
        pos_qc,
        all_bypass: true,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FetchPendingResponsePayloadKind {
    BlockSyncUpdate,
    BlockCreated,
    EagerRbcPayload,
    Other,
}

impl FetchPendingResponsePayloadKind {
    fn from_message(msg: &BlockMessage) -> Self {
        match msg {
            BlockMessage::BlockSyncUpdate(_) => Self::BlockSyncUpdate,
            BlockMessage::BlockCreated(_) => Self::BlockCreated,
            BlockMessage::RbcInit(_)
            | BlockMessage::RbcChunk(_)
            | BlockMessage::RbcChunkCompact(_)
            | BlockMessage::RbcReady(_)
            | BlockMessage::RbcDeliver(_) => Self::EagerRbcPayload,
            _ => Self::Other,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FetchPendingResponsePreflightDecision {
    hintless_allowed: bool,
    downgrade_hintless: bool,
    message_after_hintless_gate: FetchPendingResponsePayloadKind,
    apply_cached_qc: bool,
    trim_update: bool,
    bypass_queue: bool,
}

fn fetch_pending_response_preflight_decision(
    initial_kind: FetchPendingResponsePayloadKind,
    hintless_block_sync: bool,
    force_bypass_queue: bool,
    priority: FetchPendingBlockPriority,
    targets_highest_qc: bool,
    allow_highest_qc_bypass: bool,
    allow_hintless_block_sync_bypass: bool,
    requester_roster_proof_known: bool,
) -> FetchPendingResponsePreflightDecision {
    let hintless_allowed =
        hintless_block_sync && allow_hintless_block_sync_bypass && requester_roster_proof_known;
    let downgrade_hintless = hintless_block_sync && !hintless_allowed;
    let message_after_hintless_gate = if downgrade_hintless {
        FetchPendingResponsePayloadKind::BlockCreated
    } else {
        initial_kind
    };
    let apply_cached_qc = matches!(
        message_after_hintless_gate,
        FetchPendingResponsePayloadKind::BlockSyncUpdate
    );
    let bypass_queue = force_bypass_queue
        || matches!(priority, FetchPendingBlockPriority::Consensus)
        || (allow_highest_qc_bypass && targets_highest_qc)
        || matches!(
            message_after_hintless_gate,
            FetchPendingResponsePayloadKind::BlockCreated
                | FetchPendingResponsePayloadKind::EagerRbcPayload
        )
        || (allow_hintless_block_sync_bypass && hintless_allowed);
    FetchPendingResponsePreflightDecision {
        hintless_allowed,
        downgrade_hintless,
        message_after_hintless_gate,
        apply_cached_qc,
        trim_update: apply_cached_qc,
        bypass_queue,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FetchPendingResponseFinalPayload {
    Original(FetchPendingResponsePayloadKind),
    FallbackBlockCreated,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FetchPendingResponseFrameDecision {
    final_payload: FetchPendingResponseFinalPayload,
    payload_sent: bool,
    direct_qc_companion: bool,
    companion_before_payload: bool,
}

fn fetch_pending_response_frame_decision(
    message_after_hintless_gate: FetchPendingResponsePayloadKind,
    trim_fits: bool,
    fallback_fits: bool,
    direct_qc_available: bool,
) -> FetchPendingResponseFrameDecision {
    let final_payload = if matches!(
        message_after_hintless_gate,
        FetchPendingResponsePayloadKind::BlockSyncUpdate
    ) && !trim_fits
    {
        if fallback_fits {
            FetchPendingResponseFinalPayload::FallbackBlockCreated
        } else {
            FetchPendingResponseFinalPayload::None
        }
    } else {
        FetchPendingResponseFinalPayload::Original(message_after_hintless_gate)
    };
    let payload_sent = !matches!(final_payload, FetchPendingResponseFinalPayload::None);
    FetchPendingResponseFrameDecision {
        final_payload,
        payload_sent,
        direct_qc_companion: direct_qc_available,
        companion_before_payload: direct_qc_available && payload_sent,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FetchPendingResponsesBatchCommitDecision {
    dispatch_commit_qc_only: bool,
    restash: bool,
    restash_commit_qc_only: bool,
}

fn fetch_pending_responses_batch_commit_decision(
    commit_qc_only: bool,
    commit_qc_dispatch_succeeds: bool,
) -> FetchPendingResponsesBatchCommitDecision {
    let restash = commit_qc_only && !commit_qc_dispatch_succeeds;
    FetchPendingResponsesBatchCommitDecision {
        dispatch_commit_qc_only: commit_qc_only,
        restash,
        restash_commit_qc_only: restash,
    }
}

fn fetch_pending_responses_batch_should_build_payload(payload_peer_count: usize) -> bool {
    payload_peer_count > 0
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FetchPendingResponsesBatchPayloadKind {
    HintlessBlockSyncUpdate,
    RosterBlockSyncUpdate,
    BlockCreated,
    Other,
}

impl FetchPendingResponsesBatchPayloadKind {
    fn from_message(msg: &BlockMessage, hintless_block_sync: bool) -> Self {
        match msg {
            BlockMessage::BlockSyncUpdate(_) if hintless_block_sync => {
                Self::HintlessBlockSyncUpdate
            }
            BlockMessage::BlockSyncUpdate(_) => Self::RosterBlockSyncUpdate,
            BlockMessage::BlockCreated(_) => Self::BlockCreated,
            _ => Self::Other,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FetchPendingResponsesBatchPayloadMessage {
    BlockSyncUpdate,
    BlockCreated,
    Other,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FetchPendingResponsesBatchPayloadDecision {
    payload_peer: bool,
    exact_body_companion: bool,
    hintless_allowed: bool,
    payload_sent: bool,
    payload_message: FetchPendingResponsesBatchPayloadMessage,
    created_companion: bool,
    payload_pos: u8,
    created_companion_pos: u8,
    created_companion_before_payload: bool,
    payload_force_bypass_arg: bool,
    payload_allow_hintless_arg: bool,
    payload_roster_proof_arg: bool,
    payload_consensus_priority_arg: bool,
}

fn fetch_pending_responses_batch_payload_decision(
    payload_peer: bool,
    payload_kind: FetchPendingResponsesBatchPayloadKind,
    force_bypass_queue: bool,
    allow_hintless_block_sync_bypass: bool,
    requester_roster_proof_known: bool,
    priority: FetchPendingBlockPriority,
    created_companion_fits: bool,
) -> FetchPendingResponsesBatchPayloadDecision {
    let consensus_priority = matches!(priority, FetchPendingBlockPriority::Consensus);
    let exact_body_companion = payload_peer && consensus_priority;
    let hintless_allowed = payload_peer
        && matches!(
            payload_kind,
            FetchPendingResponsesBatchPayloadKind::HintlessBlockSyncUpdate
        )
        && allow_hintless_block_sync_bypass
        && requester_roster_proof_known;
    let payload_sent = payload_peer;
    let payload_message = if !payload_sent {
        FetchPendingResponsesBatchPayloadMessage::None
    } else {
        match payload_kind {
            FetchPendingResponsesBatchPayloadKind::HintlessBlockSyncUpdate => {
                if hintless_allowed {
                    FetchPendingResponsesBatchPayloadMessage::BlockSyncUpdate
                } else {
                    FetchPendingResponsesBatchPayloadMessage::BlockCreated
                }
            }
            FetchPendingResponsesBatchPayloadKind::RosterBlockSyncUpdate => {
                FetchPendingResponsesBatchPayloadMessage::BlockSyncUpdate
            }
            FetchPendingResponsesBatchPayloadKind::BlockCreated => {
                FetchPendingResponsesBatchPayloadMessage::BlockCreated
            }
            FetchPendingResponsesBatchPayloadKind::Other => {
                FetchPendingResponsesBatchPayloadMessage::Other
            }
        }
    };
    let created_companion = payload_peer
        && matches!(
            payload_kind,
            FetchPendingResponsesBatchPayloadKind::RosterBlockSyncUpdate
        )
        && created_companion_fits;
    let payload_pos = if payload_sent {
        1 + u8::from(exact_body_companion) + u8::from(created_companion)
    } else {
        0
    };
    let created_companion_pos = if created_companion {
        1 + u8::from(exact_body_companion)
    } else {
        0
    };
    let payload_force_bypass_arg = payload_sent
        && if matches!(
            payload_kind,
            FetchPendingResponsesBatchPayloadKind::HintlessBlockSyncUpdate
        ) {
            force_bypass_queue
        } else {
            force_bypass_queue
                || (allow_hintless_block_sync_bypass
                    && matches!(
                        payload_kind,
                        FetchPendingResponsesBatchPayloadKind::BlockCreated
                    ))
        };
    let payload_allow_hintless_arg = payload_sent
        && if matches!(
            payload_kind,
            FetchPendingResponsesBatchPayloadKind::HintlessBlockSyncUpdate
        ) {
            hintless_allowed
        } else {
            allow_hintless_block_sync_bypass
        };
    FetchPendingResponsesBatchPayloadDecision {
        payload_peer,
        exact_body_companion,
        hintless_allowed,
        payload_sent,
        payload_message,
        created_companion,
        payload_pos,
        created_companion_pos,
        created_companion_before_payload: !created_companion || created_companion_pos < payload_pos,
        payload_force_bypass_arg,
        payload_allow_hintless_arg,
        payload_roster_proof_arg: payload_sent && requester_roster_proof_known,
        payload_consensus_priority_arg: payload_sent && consensus_priority,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PendingResponseFlushKind {
    Fetch,
    Body,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PendingResponseFlushDecision {
    returns_ready: bool,
    build_payload: bool,
    fetch_removed: bool,
    body_removed: bool,
    fetch_batch_called: bool,
    fetch_batch_force_arg: bool,
    fetch_batch_allow_highest_arg: bool,
    fetch_batch_allow_hintless_arg: bool,
    body_response_constructed: bool,
    body_response_hash_bound: bool,
    body_response_height_bound: bool,
    body_response_view_bound: bool,
    body_response_payload_bound: bool,
    body_dispatches_use_plain_fallback: bool,
}

fn pending_response_flush_decision(
    kind: PendingResponseFlushKind,
    pending_key_present: bool,
    canonical_response_deferred: bool,
) -> PendingResponseFlushDecision {
    let returns_ready = pending_key_present && !canonical_response_deferred;
    let fetch = matches!(kind, PendingResponseFlushKind::Fetch);
    let body = matches!(kind, PendingResponseFlushKind::Body);
    let body_response_constructed = body && returns_ready;
    PendingResponseFlushDecision {
        returns_ready,
        build_payload: pending_key_present,
        fetch_removed: fetch && returns_ready,
        body_removed: body && returns_ready,
        fetch_batch_called: fetch && returns_ready,
        fetch_batch_force_arg: false,
        fetch_batch_allow_highest_arg: false,
        fetch_batch_allow_hintless_arg: false,
        body_response_constructed,
        body_response_hash_bound: body_response_constructed,
        body_response_height_bound: body_response_constructed,
        body_response_view_bound: body_response_constructed,
        body_response_payload_bound: body_response_constructed,
        body_dispatches_use_plain_fallback: true,
    }
}

fn pending_response_flush_targets_requester(
    decision: PendingResponseFlushDecision,
    requester_recorded: bool,
) -> bool {
    decision.returns_ready && requester_recorded
}

#[cfg(debug_assertions)]
fn block_body_response_body_matches_payload(
    body: &super::message::BlockBodyData,
    payload: &BlockMessage,
) -> bool {
    match (body, payload) {
        (
            super::message::BlockBodyData::BlockCreated(body),
            BlockMessage::BlockCreated(payload),
        ) => body.encode() == payload.encode(),
        (
            super::message::BlockBodyData::BlockSyncUpdate(body),
            BlockMessage::BlockSyncUpdate(payload),
        ) => body.encode() == payload.encode(),
        _ => false,
    }
}

fn block_sync_consensus_mode_tag(consensus_mode: ConsensusMode) -> &'static str {
    match consensus_mode {
        ConsensusMode::Permissioned => PERMISSIONED_TAG,
        ConsensusMode::Npos => NPOS_TAG,
    }
}

fn block_sync_commit_conflict_detected(
    height_convertible: bool,
    nonzero_height: bool,
    committed_present: bool,
    committed_hash_matches: bool,
) -> bool {
    height_convertible && nonzero_height && committed_present && !committed_hash_matches
}

fn block_sync_commit_conflict_should_validate_qc(
    commit_conflict: bool,
    incoming_qc_present: bool,
) -> bool {
    commit_conflict && incoming_qc_present
}

fn block_sync_commit_conflict_should_emit_evidence(
    commit_conflict: bool,
    incoming_qc_present: bool,
    qc_valid: bool,
) -> bool {
    commit_conflict && incoming_qc_present && qc_valid
}

fn block_sync_commit_conflict_should_clear_missing(commit_conflict: bool) -> bool {
    commit_conflict
}

fn block_sync_commit_conflict_drop_record(
    commit_conflict: bool,
) -> Option<(
    super::status::ConsensusMessageKind,
    super::status::ConsensusMessageOutcome,
    super::status::ConsensusMessageReason,
)> {
    commit_conflict.then_some((
        super::status::ConsensusMessageKind::BlockSyncUpdate,
        super::status::ConsensusMessageOutcome::Dropped,
        super::status::ConsensusMessageReason::CommitConflict,
    ))
}

fn deferred_block_sync_validation_pending_conflicts(
    pending_height: Option<u64>,
    block_height: u64,
) -> bool {
    pending_height.is_none_or(|pending_height| pending_height <= block_height)
}

fn deferred_block_sync_validation_inflight_blocks(
    validation_inflight_empty: bool,
    contiguous_frontier: bool,
    blocking_pending_conflict: bool,
) -> bool {
    !validation_inflight_empty && (!contiguous_frontier || blocking_pending_conflict)
}

fn deferred_block_sync_update_deferral_reason(
    commit_inflight: bool,
    validation_blocks: bool,
    pending_processing: bool,
    allow_certified_exact_frontier_bypass: bool,
) -> Option<&'static str> {
    if allow_certified_exact_frontier_bypass {
        return None;
    }
    if commit_inflight {
        return Some("commit_inflight");
    }
    if validation_blocks {
        return Some("validation_inflight");
    }
    pending_processing.then_some("pending_processing")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DeferredBlockSyncMergeDecision {
    take_incoming_commit_qc: bool,
    take_incoming_validator_checkpoint: bool,
    take_incoming_stake_snapshot: bool,
    replace_sender: bool,
    final_commit_qc_present: bool,
    final_validator_checkpoint_present: bool,
    final_stake_snapshot_present: bool,
    final_sender_present: bool,
}

fn deferred_block_sync_merge_decision(
    existing_commit_qc_present: bool,
    incoming_commit_qc_present: bool,
    existing_validator_checkpoint_present: bool,
    incoming_validator_checkpoint_present: bool,
    existing_stake_snapshot_present: bool,
    incoming_stake_snapshot_present: bool,
    existing_sender_present: bool,
    incoming_sender_present: bool,
) -> DeferredBlockSyncMergeDecision {
    let take_incoming_commit_qc = !existing_commit_qc_present && incoming_commit_qc_present;
    let take_incoming_validator_checkpoint =
        !existing_validator_checkpoint_present && incoming_validator_checkpoint_present;
    let take_incoming_stake_snapshot =
        !existing_stake_snapshot_present && incoming_stake_snapshot_present;
    DeferredBlockSyncMergeDecision {
        take_incoming_commit_qc,
        take_incoming_validator_checkpoint,
        take_incoming_stake_snapshot,
        replace_sender: incoming_sender_present,
        final_commit_qc_present: existing_commit_qc_present || incoming_commit_qc_present,
        final_validator_checkpoint_present: existing_validator_checkpoint_present
            || incoming_validator_checkpoint_present,
        final_stake_snapshot_present: existing_stake_snapshot_present
            || incoming_stake_snapshot_present,
        final_sender_present: existing_sender_present || incoming_sender_present,
    }
}

fn deferred_block_sync_commit_evidence_present(
    commit_qc_present: bool,
    validator_checkpoint_present: bool,
    stake_snapshot_present: bool,
) -> bool {
    commit_qc_present || validator_checkpoint_present || stake_snapshot_present
}

fn deferred_block_sync_cap_should_evict(cap: usize, len: usize) -> bool {
    cap > 0 && len > cap
}

fn deferred_block_sync_cap_eviction_count(cap: usize, len: usize) -> usize {
    if deferred_block_sync_cap_should_evict(cap, len) {
        len - cap
    } else {
        0
    }
}

fn deferred_block_sync_eviction_rank(
    has_commit_evidence: bool,
    height: u64,
    view: u64,
    hash: HashOf<BlockHeader>,
) -> (u8, u64, u64, HashOf<BlockHeader>) {
    (u8::from(has_commit_evidence), view, height, hash)
}

fn deferred_block_sync_cache_key(
    block_height: u64,
    block_view: u64,
    block_hash: HashOf<BlockHeader>,
) -> (u64, u64, HashOf<BlockHeader>) {
    (block_height, block_view, block_hash)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DeferredBlockSyncCacheDecision {
    cache_called: bool,
    commit_votes_cleared: bool,
    key_matched: bool,
    inserted: bool,
    cap_called: bool,
    len_before_cap: usize,
    eviction_count: usize,
    final_len: usize,
}

fn deferred_block_sync_cache_decision(
    initial_len: usize,
    existing_same_full_key: bool,
    cap: usize,
) -> DeferredBlockSyncCacheDecision {
    let inserted = !existing_same_full_key;
    let len_before_cap = initial_len + usize::from(inserted);
    let eviction_count = deferred_block_sync_cap_eviction_count(cap, len_before_cap);
    DeferredBlockSyncCacheDecision {
        cache_called: true,
        commit_votes_cleared: true,
        key_matched: existing_same_full_key,
        inserted,
        cap_called: true,
        len_before_cap,
        eviction_count,
        final_len: len_before_cap.saturating_sub(eviction_count),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DeferredBlockSyncDeferRecordDecision {
    cache_called: bool,
    record_called: bool,
    record_after_cache: bool,
    recorded_kind: super::status::ConsensusMessageKind,
    recorded_outcome: super::status::ConsensusMessageOutcome,
    recorded_reason: super::status::ConsensusMessageReason,
}

fn deferred_block_sync_defer_record_decision() -> DeferredBlockSyncDeferRecordDecision {
    DeferredBlockSyncDeferRecordDecision {
        cache_called: true,
        record_called: true,
        record_after_cache: true,
        recorded_kind: super::status::ConsensusMessageKind::BlockSyncUpdate,
        recorded_outcome: super::status::ConsensusMessageOutcome::Deferred,
        recorded_reason: super::status::ConsensusMessageReason::CommitPipelineActive,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DeferredBlockSyncReplayDecision {
    returns_progress: bool,
    select_key: bool,
    remove_before_handle: bool,
    handle_called: bool,
    update_forwarded: bool,
    sender_forwarded: bool,
    warn_on_error: bool,
    final_len: usize,
    later_entries_preserved: bool,
}

fn deferred_block_sync_replay_decision(
    initial_len: usize,
    commit_inflight: bool,
    validation_inflight: bool,
    remove_succeeds: bool,
    handler_errors: bool,
) -> DeferredBlockSyncReplayDecision {
    let ready = initial_len > 0 && !commit_inflight && !validation_inflight;
    let returns_progress = ready && remove_succeeds;
    DeferredBlockSyncReplayDecision {
        returns_progress,
        select_key: ready,
        remove_before_handle: returns_progress,
        handle_called: returns_progress,
        update_forwarded: returns_progress,
        sender_forwarded: returns_progress,
        warn_on_error: returns_progress && handler_errors,
        final_len: initial_len - usize::from(returns_progress),
        later_entries_preserved: initial_len != 2 || returns_progress,
    }
}

fn block_sync_future_window_requested_margin(raw_margin: u64) -> u64 {
    raw_margin.max(1)
}

fn block_sync_future_window_far_ahead(
    height: u64,
    local_height: u64,
    requested_margin: u64,
) -> bool {
    height > local_height.saturating_add(requested_margin)
}

fn block_sync_future_window_lower_unresolved(missing_height: Option<u64>, height: u64) -> bool {
    missing_height.is_some_and(|missing_height| missing_height < height)
}

fn block_sync_future_window_pre_generic_drop(
    known_block: bool,
    requested_missing_block: bool,
    far_ahead_by_committed: bool,
    lower_unresolved_missing: bool,
    parent_available: bool,
) -> Option<bool> {
    if known_block {
        return Some(false);
    }
    if requested_missing_block {
        return Some(far_ahead_by_committed);
    }
    if lower_unresolved_missing && far_ahead_by_committed {
        return Some(true);
    }
    if parent_available {
        return Some(false);
    }
    None
}

fn block_sync_future_window_drop_decision(
    known_block: bool,
    requested_missing_block: bool,
    far_ahead_by_committed: bool,
    lower_unresolved_missing: bool,
    parent_available: bool,
    generic_drop: bool,
) -> bool {
    block_sync_future_window_pre_generic_drop(
        known_block,
        requested_missing_block,
        far_ahead_by_committed,
        lower_unresolved_missing,
        parent_available,
    )
    .unwrap_or(generic_drop)
}

fn block_sync_commit_conflict_allow_genesis_stub(block_height: u64, block_view: u64) -> bool {
    block_height == 1 && block_view == 0
}

const BLOCK_SYNC_COMMIT_CONFLICT_EVIDENCE_REASON: &str = "commit_conflict_finality";

fn block_sync_commit_conflict_invalid_qc_evidence(
    commit_qc: Qc,
) -> crate::sumeragi::consensus::Evidence {
    crate::sumeragi::consensus::Evidence {
        kind: crate::sumeragi::consensus::EvidenceKind::InvalidQc,
        payload: crate::sumeragi::consensus::EvidencePayload::InvalidQc {
            certificate: commit_qc,
            reason: BLOCK_SYNC_COMMIT_CONFLICT_EVIDENCE_REASON.to_owned(),
        },
    }
}

fn block_sync_vote_placeholder_matches(
    vote: &crate::sumeragi::consensus::Vote,
    block_hash: HashOf<BlockHeader>,
    block_height: u64,
    block_view: u64,
    expected_epoch: u64,
) -> bool {
    vote.phase == crate::sumeragi::consensus::Phase::Commit
        && vote.block_hash == block_hash
        && vote.height == block_height
        && vote.view == block_view
        && vote.epoch == expected_epoch
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BlockSyncSnapshotHintFilter {
    snapshot_present: bool,
    qc_after: bool,
    qc_revalidated: bool,
    checkpoint_after: bool,
    stake_after: bool,
}

fn block_sync_snapshot_hint_filter(
    snapshot_present: bool,
    incoming_qc: bool,
    qc_hash_matches: bool,
    qc_same_validator_set: bool,
    incoming_checkpoint: bool,
    checkpoint_hash_matches: bool,
    incoming_stake: bool,
    local_stake_present: bool,
    stake_hash_matches: bool,
) -> BlockSyncSnapshotHintFilter {
    let (qc_after, qc_revalidated) = if !incoming_qc {
        (false, false)
    } else if !snapshot_present || qc_hash_matches {
        (true, false)
    } else if qc_same_validator_set {
        (true, true)
    } else {
        (false, false)
    };
    let checkpoint_after = if !incoming_checkpoint {
        false
    } else if !snapshot_present {
        true
    } else {
        checkpoint_hash_matches
    };
    let stake_after = if !incoming_stake {
        false
    } else if !snapshot_present {
        true
    } else {
        local_stake_present && stake_hash_matches
    };
    BlockSyncSnapshotHintFilter {
        snapshot_present,
        qc_after,
        qc_revalidated,
        checkpoint_after,
        stake_after,
    }
}

fn block_sync_snapshot_roster_selection(
    snapshot: &crate::commit_roster_journal::CommitRosterSnapshot,
) -> Option<BlockSyncRosterSelection> {
    let roster = snapshot.commit_qc.validator_set.clone();
    if roster.is_empty() {
        return None;
    }
    let stake_snapshot = snapshot
        .stake_snapshot
        .as_ref()
        .filter(|snapshot| snapshot.matches_roster(&roster))
        .cloned();
    Some(BlockSyncRosterSelection {
        roster,
        source: BlockSyncRosterSource::CommitRosterJournal,
        commit_qc: Some(snapshot.commit_qc.clone()),
        checkpoint: Some(snapshot.validator_checkpoint.clone()),
        stake_snapshot,
    })
}

fn block_sync_no_roster_known_vote_only(
    block_known: bool,
    has_commit_votes: bool,
    cert_hint_present: bool,
    checkpoint_hint_present: bool,
    stake_hint_present: bool,
) -> bool {
    block_known
        && has_commit_votes
        && !cert_hint_present
        && !checkpoint_hint_present
        && !stake_hint_present
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BlockSyncNoRosterFallbackSource {
    None,
    Effective,
    Trusted,
}

fn block_sync_no_roster_fallback_roster(
    effective_roster: Vec<PeerId>,
    trusted_roster: Vec<PeerId>,
) -> (BlockSyncNoRosterFallbackSource, Vec<PeerId>) {
    if !effective_roster.is_empty() {
        (BlockSyncNoRosterFallbackSource::Effective, effective_roster)
    } else if !trusted_roster.is_empty() {
        (BlockSyncNoRosterFallbackSource::Trusted, trusted_roster)
    } else {
        (BlockSyncNoRosterFallbackSource::None, Vec::new())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BlockSyncKnownRosterCandidateQcSource {
    Incoming,
    Selection,
    Checkpoint,
}

fn block_sync_known_roster_candidate_qc(
    incoming_qc: Option<Qc>,
    selection_commit_qc: Option<Qc>,
    checkpoint_qc: Option<Qc>,
) -> Option<(BlockSyncKnownRosterCandidateQcSource, Qc)> {
    incoming_qc
        .map(|qc| (BlockSyncKnownRosterCandidateQcSource::Incoming, qc))
        .or_else(|| {
            selection_commit_qc.map(|qc| (BlockSyncKnownRosterCandidateQcSource::Selection, qc))
        })
        .or_else(|| checkpoint_qc.map(|qc| (BlockSyncKnownRosterCandidateQcSource::Checkpoint, qc)))
}

fn block_sync_selected_signatures_should_cache_validated_signers(
    cache_key_available: bool,
) -> bool {
    cache_key_available
}

fn block_sync_selected_signatures_ahead_of_frontier(block_height: u64, local_height: u64) -> bool {
    block_height > local_height.saturating_add(1)
}

fn block_sync_selected_signatures_error_is_deferable(
    err: crate::block::SignatureVerificationError,
) -> bool {
    matches!(
        err,
        crate::block::SignatureVerificationError::UnknownSignature
            | crate::block::SignatureVerificationError::UnknownSignatory
            | crate::block::SignatureVerificationError::MissingPop
    )
}

fn block_sync_selected_signatures_should_defer(
    parent_missing: bool,
    ahead: bool,
    err: crate::block::SignatureVerificationError,
) -> bool {
    parent_missing && ahead && block_sync_selected_signatures_error_is_deferable(err)
}

fn block_sync_selected_signatures_should_request_gap(
    block_height: u64,
    expected_height: u64,
) -> bool {
    block_height > expected_height.saturating_add(1)
}

fn block_sync_selected_signatures_has_roster_evidence(
    incoming_qc_present: bool,
    selection_commit_qc_present: bool,
    checkpoint_present: bool,
) -> bool {
    incoming_qc_present || selection_commit_qc_present || checkpoint_present
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BlockSyncSelectedQcSource {
    Incoming,
    Selection,
    Checkpoint,
    World,
    Cached,
}

fn block_sync_selected_qc_candidate(
    incoming_qc: Option<Qc>,
    selection_commit_qc: Option<Qc>,
    checkpoint_qc: Option<Qc>,
    world_qc: Option<Qc>,
    cached_qc: Option<Qc>,
) -> Option<(BlockSyncSelectedQcSource, Qc)> {
    incoming_qc
        .map(|qc| (BlockSyncSelectedQcSource::Incoming, qc))
        .or_else(|| selection_commit_qc.map(|qc| (BlockSyncSelectedQcSource::Selection, qc)))
        .or_else(|| checkpoint_qc.map(|qc| (BlockSyncSelectedQcSource::Checkpoint, qc)))
        .or_else(|| world_qc.map(|qc| (BlockSyncSelectedQcSource::World, qc)))
        .or_else(|| cached_qc.map(|qc| (BlockSyncSelectedQcSource::Cached, qc)))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BlockSyncSelectedQcShape {
    Valid,
    HeightMismatch,
    HashMismatch,
    EpochMismatch,
    PhaseMismatch,
}

fn block_sync_selected_qc_shape(
    qc: &Qc,
    block_hash: HashOf<BlockHeader>,
    block_height: u64,
    expected_epoch: u64,
) -> BlockSyncSelectedQcShape {
    if qc.height != block_height {
        BlockSyncSelectedQcShape::HeightMismatch
    } else if qc.subject_block_hash != block_hash {
        BlockSyncSelectedQcShape::HashMismatch
    } else if qc.epoch != expected_epoch {
        BlockSyncSelectedQcShape::EpochMismatch
    } else if !matches!(qc.phase, crate::sumeragi::consensus::Phase::Commit) {
        BlockSyncSelectedQcShape::PhaseMismatch
    } else {
        BlockSyncSelectedQcShape::Valid
    }
}

fn block_sync_selected_qc_aggregate_ok(
    cached_qc_match: bool,
    selection_commit_qc_match: bool,
) -> Option<bool> {
    (cached_qc_match || selection_commit_qc_match).then_some(true)
}

fn block_sync_selected_qc_should_derive_cached(
    candidate_kept: bool,
    candidate_validated: bool,
    had_incoming_qc: bool,
) -> bool {
    !candidate_kept || (!candidate_validated && had_incoming_qc)
}

fn block_sync_selected_qc_should_attempt_aggregate_fallback(
    had_incoming_qc: bool,
    qc_evidence_available: bool,
) -> bool {
    had_incoming_qc && !qc_evidence_available
}

fn block_sync_selected_qc_should_accept_aggregate_fallback(
    fallback_attempted: bool,
    original_candidate_present: bool,
    aggregate_fallback_ok: bool,
) -> bool {
    fallback_attempted && original_candidate_present && aggregate_fallback_ok
}

fn block_sync_selected_qc_should_drop_invalid_payload(
    invalid_qc_present: bool,
    block_quorum_met: bool,
    commit_cert_present: bool,
    checkpoint_present: bool,
) -> bool {
    invalid_qc_present && !block_quorum_met && !commit_cert_present && !checkpoint_present
}

fn block_sync_selected_quorum_sparse_exact_frontier_request(
    requested_missing_block: bool,
    exact_contiguous_frontier: bool,
    qc_evidence_present: bool,
    checkpoint_present: bool,
    has_commit_votes: bool,
) -> bool {
    requested_missing_block
        && exact_contiguous_frontier
        && !qc_evidence_present
        && !checkpoint_present
        && !has_commit_votes
}

fn block_sync_selected_quorum_should_maybe_request_missing_qc(
    quorum_available: bool,
    qc_evidence_present: bool,
    commit_cert_present: bool,
    checkpoint_present: bool,
    block_signer_count: usize,
    commit_quorum: usize,
    requested_missing_block: bool,
) -> bool {
    !quorum_available
        && !qc_evidence_present
        && !commit_cert_present
        && !checkpoint_present
        && block_signer_count < commit_quorum
        && !requested_missing_block
}

fn block_sync_selected_quorum_should_defer_npos_vote_only(
    npos_mode: bool,
    vote_only_frontier_update: bool,
    explicit_requested_missing_block: bool,
) -> bool {
    npos_mode && vote_only_frontier_update && !explicit_requested_missing_block
}

fn block_sync_selected_quorum_should_call_repair(quorum_available: bool) -> bool {
    !quorum_available
}

fn block_sync_selected_apply_allow_nonextending_qc(
    selection_commit_qc_present: bool,
    incoming_qc_validated_by_roster: bool,
    incoming_qc_usable: bool,
) -> bool {
    selection_commit_qc_present || incoming_qc_validated_by_roster || incoming_qc_usable
}

fn block_sync_selected_apply_same_height_frontier_conflict(
    block_quorum_met: bool,
    incoming_qc_usable: bool,
    commit_cert_present: bool,
    checkpoint_present: bool,
    local_conflicting_frontier_vote: bool,
) -> bool {
    block_quorum_met
        && !incoming_qc_usable
        && !commit_cert_present
        && !checkpoint_present
        && local_conflicting_frontier_vote
}

fn block_sync_selected_apply_preserve_on_payload_mismatch(
    incoming_qc_usable: bool,
    commit_cert_present: bool,
    checkpoint_present: bool,
) -> bool {
    !incoming_qc_usable && !commit_cert_present && !checkpoint_present
}

fn block_sync_selected_apply_authoritative_supersede(
    incoming_qc_usable: bool,
    commit_cert_present: bool,
    checkpoint_present: bool,
    block_quorum_met: bool,
    same_height_frontier_conflict: bool,
) -> bool {
    incoming_qc_usable
        || commit_cert_present
        || checkpoint_present
        || (block_quorum_met && !same_height_frontier_conflict)
}

fn block_sync_selected_apply_recovery_mode(
    has_commit_votes: bool,
    incoming_qc_usable: bool,
    commit_cert_present: bool,
    checkpoint_present: bool,
    observed_incoming_qc_epoch: Option<u64>,
    expected_epoch: u64,
    authoritative_supersede: bool,
) -> BlockSyncRecoveryMode {
    if has_commit_votes || incoming_qc_usable || commit_cert_present || checkpoint_present {
        BlockSyncRecoveryMode::CommitEvidenceRepair {
            observed_commit_qc_epoch: observed_incoming_qc_epoch
                .or_else(|| checkpoint_present.then_some(expected_epoch)),
            allow_aborted_revival_without_local_commit_qc: has_commit_votes
                || commit_cert_present
                || checkpoint_present,
        }
    } else if authoritative_supersede {
        BlockSyncRecoveryMode::SignedQuorumFrontierRepair
    } else {
        BlockSyncRecoveryMode::PayloadOnly
    }
}

#[derive(Clone, Copy, Debug)]
struct BlockSyncSelectedApplySignedQuorumRepair {
    creation_ok: bool,
    block_known_after_creation: bool,
    signature_quorum_met: bool,
    exact_contiguous_frontier: bool,
    qc_evidence_present: bool,
    commit_cert_present: bool,
    checkpoint_present: bool,
    missing_commit_qc_repair_active: bool,
}

fn block_sync_selected_apply_signed_quorum_commit_repair_active(
    input: BlockSyncSelectedApplySignedQuorumRepair,
) -> bool {
    input.creation_ok
        && input.block_known_after_creation
        && input.signature_quorum_met
        && input.exact_contiguous_frontier
        && !input.qc_evidence_present
        && !input.commit_cert_present
        && !input.checkpoint_present
        && input.missing_commit_qc_repair_active
}

fn block_sync_selected_apply_pending_commit_qc_observed(
    signed_quorum_commit_repair_active: bool,
    pending_block_matches_non_invalid: bool,
) -> bool {
    signed_quorum_commit_repair_active && pending_block_matches_non_invalid
}

#[derive(Clone, Copy, Debug)]
struct BlockSyncSelectedApplySparseRecovery {
    block_known_before: bool,
    block_known_after_creation: bool,
    next_height: bool,
    block_signer_count: usize,
    commit_quorum: usize,
    incoming_qc_usable: bool,
    commit_cert_present: bool,
    checkpoint_present: bool,
}

fn block_sync_selected_apply_sparse_next_height_payload_recovered(
    input: BlockSyncSelectedApplySparseRecovery,
) -> bool {
    !input.block_known_before
        && input.block_known_after_creation
        && input.next_height
        && input.block_signer_count < input.commit_quorum
        && !input.incoming_qc_usable
        && !input.commit_cert_present
        && !input.checkpoint_present
}

fn block_sync_selected_apply_payload_unapplied_drop(ready_for_qc: bool) -> bool {
    !ready_for_qc
}

fn block_sync_selected_apply_qc_to_apply(ready_for_qc: bool, qc_evidence_present: bool) -> bool {
    ready_for_qc && qc_evidence_present
}

fn block_sync_selected_qc_prefilter_topology_recovery(topology_empty: bool) -> bool {
    topology_empty
}

fn block_sync_selected_qc_prefilter_hash_mismatch(hash_matches: bool) -> bool {
    !hash_matches
}

fn block_sync_selected_qc_prefilter_height_mismatch(height_matches: bool) -> bool {
    !height_matches
}

fn block_sync_selected_qc_prefilter_epoch_mismatch(epoch_matches: bool) -> bool {
    !epoch_matches
}

fn block_sync_selected_qc_prefilter_phase_mismatch(commit_phase: bool) -> bool {
    !commit_phase
}

fn block_sync_selected_qc_prefilter_same_height_locked_drop(
    same_height_conflict: bool,
    same_height_recoverable: bool,
) -> bool {
    same_height_conflict && !same_height_recoverable
}

fn block_sync_selected_qc_prefilter_stale_locked_drop(stale_against_lock: bool) -> bool {
    stale_against_lock
}

fn block_sync_selected_qc_prefilter_nonextending_needs_resolution(
    extends_locked: bool,
    allow_nonextending_qc: bool,
) -> bool {
    !extends_locked && !allow_nonextending_qc
}

fn block_sync_selected_qc_prefilter_nonextending_defer(
    needs_resolution: bool,
    deferred_missing_locked_payload: bool,
) -> bool {
    needs_resolution && deferred_missing_locked_payload
}

fn block_sync_selected_qc_prefilter_nonextending_locked_drop(
    needs_resolution: bool,
    deferred_missing_locked_payload: bool,
) -> bool {
    needs_resolution && !deferred_missing_locked_payload
}

fn block_sync_selected_qc_prefilter_retain_nonextending(
    extends_locked: bool,
    allow_nonextending_qc: bool,
) -> bool {
    !extends_locked && allow_nonextending_qc
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BlockSyncSelectedQcProcessTallySource {
    Cached,
    Fresh,
}

fn block_sync_selected_qc_process_tally_source(
    cached_tally_available: bool,
) -> BlockSyncSelectedQcProcessTallySource {
    if cached_tally_available {
        BlockSyncSelectedQcProcessTallySource::Cached
    } else {
        BlockSyncSelectedQcProcessTallySource::Fresh
    }
}

fn block_sync_selected_qc_process_block_known_for_commit(
    pending_block_valid: bool,
    inflight_block_active: bool,
    kura_block_known: bool,
) -> bool {
    pending_block_valid || inflight_block_active || kura_block_known
}

fn block_sync_selected_qc_process_commit_qc_accepted(process_ok: bool) -> bool {
    process_ok
}

fn block_sync_selected_qc_process_apply_commit_qc(
    commit_qc_accepted: bool,
    block_known_for_commit: bool,
) -> bool {
    commit_qc_accepted && block_known_for_commit
}

fn block_sync_selected_qc_process_clean_rbc_sessions(
    apply_commit_qc: bool,
    runtime_da_enabled: bool,
) -> bool {
    apply_commit_qc && runtime_da_enabled
}

fn block_sync_selected_qc_process_observe_pending_epoch(
    commit_qc_accepted: bool,
    block_known_for_commit: bool,
    pending_entry_exists: bool,
) -> bool {
    commit_qc_accepted && !block_known_for_commit && pending_entry_exists
}

fn block_sync_selected_qc_process_cache_unknown_block_qc(
    creation_ok: bool,
    block_known_after_creation: bool,
    incoming_qc_present: bool,
) -> bool {
    creation_ok && !block_known_after_creation && incoming_qc_present
}

fn block_sync_selected_qc_cache_update_locked_qc(
    allow_nonextending_qc: bool,
    incoming_newer_than_lock: bool,
) -> bool {
    allow_nonextending_qc && incoming_newer_than_lock
}

fn block_sync_selected_qc_cache_missing_context_quarantine(missing_context_error: bool) -> bool {
    missing_context_error
}

fn block_sync_selected_qc_cache_final_validation_drop(missing_context_error: bool) -> bool {
    !missing_context_error
}

pub(super) fn block_sync_qc_aggregate_fallback_ok(
    qc: &crate::sumeragi::consensus::Qc,
    topology: &super::network_topology::Topology,
    pops: &BTreeMap<PublicKey, Vec<u8>>,
    chain_id: &ChainId,
    consensus_mode: ConsensusMode,
    stake_snapshot: Option<&CommitStakeSnapshot>,
    mode_tag: &str,
) -> bool {
    if qc.phase != crate::sumeragi::consensus::Phase::Commit {
        return false;
    }
    if qc.highest_qc.is_some() {
        return false;
    }
    if !super::qc_aggregate_consistent(qc, topology, pops, chain_id, mode_tag) {
        return false;
    }
    let roster_len = topology.as_ref().len();
    let Ok(parsed) = super::qc_signer_indices(qc, roster_len, roster_len) else {
        return false;
    };
    match consensus_mode {
        ConsensusMode::Permissioned => {
            parsed.voting.len() >= topology.min_votes_for_commit().max(1)
        }
        ConsensusMode::Npos => {
            let Some(snapshot) = stake_snapshot else {
                return false;
            };
            let Ok(signer_peers) = super::signer_peers_for_topology(&parsed.voting, topology)
            else {
                return false;
            };
            super::stake_snapshot::stake_quorum_reached_for_snapshot(
                snapshot,
                topology.as_ref(),
                &signer_peers,
            )
            .unwrap_or(false)
        }
    }
}

impl Actor {
    fn should_defer_canonical_committed_fetch_response(
        &self,
        block: &SignedBlock,
        msg: &BlockMessage,
    ) -> bool {
        let block_hash = block.hash();
        let block_height = block.header().height().get();
        let local_committed_height = self.committed_height_snapshot();
        should_defer_canonical_committed_fetch_response_shape(
            block_height,
            local_committed_height,
            block_sync_fetch_response_deferral_committed_hash(
                self.committed_block_hash_for_height(block_height),
                block_hash,
            ),
            block_sync_fetch_response_deferral_message(msg),
        )
    }

    pub(super) fn commit_qc_from_validator_checkpoint(
        &self,
        block_hash: HashOf<BlockHeader>,
        block_height: u64,
        block_view: u64,
        checkpoint: &ValidatorSetCheckpoint,
        stake_snapshot: Option<&CommitStakeSnapshot>,
    ) -> Option<crate::sumeragi::consensus::Qc> {
        if checkpoint.block_hash != block_hash {
            warn!(
                expected = %block_hash,
                actual = %checkpoint.block_hash,
                "ignoring validator checkpoint that does not match block hash"
            );
            return None;
        }
        if checkpoint.height != block_height {
            warn!(
                expected = block_height,
                actual = checkpoint.height,
                block = %block_hash,
                "ignoring validator checkpoint that does not match block height"
            );
            return None;
        }
        if checkpoint.view != block_view {
            warn!(
                expected = block_view,
                actual = checkpoint.view,
                block = %block_hash,
                height = block_height,
                "ignoring validator checkpoint that does not match block view"
            );
            return None;
        }
        let (consensus_mode, mode_tag, _prf_seed) = self.consensus_context_for_height(block_height);
        let expected_epoch = self.epoch_for_height(block_height);
        let checkpoint_stake_snapshot = match consensus_mode {
            ConsensusMode::Permissioned => None,
            ConsensusMode::Npos => self
                .roster_validation_cache
                .inputs_for_roster(&checkpoint.validator_set, consensus_mode, stake_snapshot)
                .stake_snapshot
                .or_else(|| {
                    self.roster_validation_cache
                        .stake_snapshot_for_roster(&checkpoint.validator_set)
                }),
        };
        let inputs = match consensus_mode {
            ConsensusMode::Permissioned => self.roster_validation_cache.inputs_for_roster(
                &checkpoint.validator_set,
                consensus_mode,
                None,
            ),
            ConsensusMode::Npos => self.roster_validation_cache.inputs_for_roster(
                &checkpoint.validator_set,
                consensus_mode,
                checkpoint_stake_snapshot.as_ref(),
            ),
        };
        let allow_genesis_stub = block_height == 1 && block_view == 0;
        if let Err(err) = super::validate_checkpoint_roster_cached(
            &self.roster_validation_cache,
            checkpoint,
            block_hash,
            block_height,
            Some(block_view),
            consensus_mode,
            &self.common_config.chain,
            mode_tag,
            expected_epoch,
            Some((checkpoint.parent_state_root, checkpoint.post_state_root)),
            allow_genesis_stub,
            &inputs,
        ) {
            warn!(
                ?err,
                block = %block_hash,
                height = block_height,
                view = block_view,
                "ignoring uncertified validator checkpoint sidecar"
            );
            return None;
        }
        Some(crate::sumeragi::consensus::Qc {
            phase: crate::sumeragi::consensus::Phase::Commit,
            subject_block_hash: checkpoint.block_hash,
            parent_state_root: checkpoint.parent_state_root,
            post_state_root: checkpoint.post_state_root,
            height: checkpoint.height,
            view: checkpoint.view,
            epoch: expected_epoch,
            chain_order_hash: checkpoint.chain_order_hash,
            rechain_seq: checkpoint.rechain_seq,
            mode_tag: mode_tag.to_string(),
            highest_qc: None,
            validator_set_hash: checkpoint.validator_set_hash,
            validator_set_hash_version: checkpoint.validator_set_hash_version,
            validator_set: checkpoint.validator_set.clone(),
            aggregate: crate::sumeragi::consensus::QcAggregate {
                signers_bitmap: checkpoint.signers_bitmap.clone(),
                bls_aggregate_signature: checkpoint.bls_aggregate_signature.clone(),
            },
        })
    }

    pub(super) fn block_sync_qc_is_missing_context_error(err: &QcValidationError) -> bool {
        matches!(
            err,
            QcValidationError::MissingVotes { .. }
                | QcValidationError::StakeSnapshotUnavailable
                // Block-sync QC hints can arrive before sidecar/roster convergence. Treat
                // aggregate mismatches as retryable to avoid churny drop/refetch loops.
                | QcValidationError::AggregateMismatch
        )
    }

    fn maybe_cache_frontier_payload_sender_vote_roster(
        &mut self,
        block: &SignedBlock,
        block_hash: HashOf<BlockHeader>,
        block_height: u64,
        block_view: u64,
        consensus_mode: ConsensusMode,
        sender: Option<&PeerId>,
    ) -> Option<Vec<PeerId>> {
        if !matches!(consensus_mode, ConsensusMode::Npos)
            || self.vote_roster_cache.contains_key(&block_hash)
        {
            return None;
        }
        let sender = sender?;
        let roster = {
            let world = self.state.world_view();
            let sender_lane_ids = crate::state::validator_lane_ids_for_peer(&world, sender);
            if sender_lane_ids.is_empty() {
                None
            } else {
                let roster = super::roster::canonicalize_roster_for_mode(
                    super::roster::filter_roster_with_live_consensus_keys_at_height_world(
                        &world,
                        super::roster::stake_active_validator_roster_for_lanes_from_world(
                            &world,
                            &sender_lane_ids,
                        ),
                        block_height,
                    ),
                    consensus_mode,
                );
                if roster.is_empty() {
                    None
                } else {
                    let topology = super::network_topology::Topology::new(roster.clone());
                    let (_, mode_tag, prf_seed) = self.consensus_context_for_height(block_height);
                    match super::validated_block_signers_from_world(
                        block, &topology, &world, mode_tag, prf_seed,
                    ) {
                        Ok(_) => Some(roster),
                        Err(err) => {
                            debug!(
                                ?err,
                                height = block_height,
                                view = block_view,
                                block = %block_hash,
                                sender = %sender,
                                "ignoring sender-lane roster candidate for frontier payload-only block sync"
                            );
                            None
                        }
                    }
                }
            }
        };
        if let Some(roster) = roster.as_ref() {
            self.cache_vote_roster(block_hash, block_height, block_view, roster.clone());
            debug!(
                height = block_height,
                view = block_view,
                block = %block_hash,
                sender = %sender,
                roster_len = roster.len(),
                "cached sender-lane vote roster before frontier payload-only block sync validation"
            );
        }
        roster
    }

    pub(super) fn has_missing_block_request_for_height(&self, height: u64) -> bool {
        self.pending
            .missing_block_requests
            .values()
            .any(|request| request.height == height)
    }

    fn block_sync_qc_final_drop(&mut self, reason: &'static str) {
        super::status::inc_blocksync_qc_final_drop(reason);
        #[cfg(feature = "telemetry")]
        if let Some(telemetry) = self.telemetry_handle() {
            telemetry.inc_blocksync_qc_final_drop(reason);
        }
    }

    fn quarantine_block_sync_qc_candidate(
        &mut self,
        qc: crate::sumeragi::consensus::Qc,
        reason: &'static str,
        target: QuarantinedQcTarget,
    ) {
        let key = Self::qc_tally_key(&qc);
        let now = Instant::now();
        if let Some(existing) = self.quarantined_block_sync_qcs.get_mut(&key) {
            existing.qc = qc;
            existing.reason = reason;
            existing.last_attempt = now;
            return;
        }
        if self.quarantined_block_sync_qcs.len() >= QUARANTINED_BLOCK_SYNC_QC_CAP {
            let oldest = self
                .quarantined_block_sync_qcs
                .iter()
                .min_by_key(|(key, entry)| (entry.first_seen, **key))
                .map(|(key, _)| *key);
            if let Some(oldest) = oldest {
                self.quarantined_block_sync_qcs.remove(&oldest);
                self.block_sync_qc_final_drop("capacity");
            }
        }
        self.quarantined_block_sync_qcs.insert(
            key,
            QuarantinedQcCandidate {
                qc,
                first_seen: now,
                last_attempt: now,
                attempts: 0,
                escalated_fetch: false,
                reason,
                target,
            },
        );
        super::status::inc_blocksync_qc_quarantine();
        #[cfg(feature = "telemetry")]
        if let Some(telemetry) = self.telemetry_handle() {
            telemetry.inc_blocksync_qc_quarantine();
        }
    }

    fn force_block_sync_fetch_for_qc(
        &mut self,
        qc: &crate::sumeragi::consensus::Qc,
        reason: &'static str,
    ) {
        let mut roster = qc.validator_set.clone();
        if roster.is_empty() {
            let (consensus_mode, _, _) = self.consensus_context_for_height(qc.height);
            roster = self.roster_for_vote_with_mode(
                qc.subject_block_hash,
                qc.height,
                qc.view,
                consensus_mode,
            );
        }
        if roster.is_empty() {
            return;
        }
        let topology = super::network_topology::Topology::new(roster);
        let signer_set =
            super::qc_signer_indices(qc, topology.as_ref().len(), topology.as_ref().len())
                .map(|parsed| parsed.voting.into_iter().collect::<BTreeSet<_>>())
                .unwrap_or_default();
        let targets = Self::build_fetch_targets(&signer_set, &topology);
        if targets.is_empty() {
            return;
        }
        if matches!(qc.phase, crate::sumeragi::consensus::Phase::Commit) {
            let sent = self.request_certified_block_for_qc(qc, &topology, &signer_set, reason);
            debug!(
                height = qc.height,
                view = qc.view,
                block = %qc.subject_block_hash,
                targets = sent,
                reason,
                "forcing certified block fetch for quarantined commit QC"
            );
            return;
        }
        self.request_missing_block(
            qc.subject_block_hash,
            qc.height,
            qc.view,
            super::MissingBlockPriority::Consensus,
            &targets,
        );
        debug!(
            height = qc.height,
            view = qc.view,
            block = %qc.subject_block_hash,
            targets = targets.len(),
            reason,
            "forcing block-sync fetch for quarantined QC"
        );
    }

    pub(super) fn keep_exact_frontier_block_sync_repair_in_slot(
        &mut self,
        block_hash: HashOf<BlockHeader>,
        block_height: u64,
        block_view: u64,
        signers: &BTreeSet<crate::sumeragi::consensus::ValidatorIndex>,
        topology: &super::network_topology::Topology,
        reason: &'static str,
    ) -> bool {
        let now = Instant::now();
        if !self.handle_frontier_body_gap_with_topology(
            block_hash,
            block_height,
            block_view,
            signers,
            topology,
            true,
            now,
        ) {
            return false;
        }
        debug!(
            height = block_height,
            view = block_view,
            block = %block_hash,
            signer_count = signers.len(),
            roster_len = topology.as_ref().len(),
            reason,
            "routing contiguous frontier block sync recovery through exact body repair"
        );
        true
    }

    #[allow(clippy::too_many_arguments)]
    fn maybe_request_pending_block_for_missing_qc(
        &mut self,
        block_hash: HashOf<BlockHeader>,
        block_height: u64,
        block_view: u64,
        block_signer_count: usize,
        commit_quorum: usize,
        block_signers: &BTreeSet<crate::sumeragi::consensus::ValidatorIndex>,
        topology: &super::network_topology::Topology,
    ) -> bool {
        let now = Instant::now();
        let retry_window = self.rebroadcast_cooldown();
        let aggressive_after_attempts = self.recovery_missing_fetch_aggressive_after_attempts();
        let existing_attempts = self
            .pending
            .missing_block_requests
            .get(&block_hash)
            .map_or(0, |stats| stats.attempts);
        let fetch_mode = if existing_attempts >= aggressive_after_attempts {
            super::MissingBlockFetchMode::AggressiveTopology
        } else {
            super::MissingBlockFetchMode::Default
        };
        let signer_fallback_attempts = self.recovery_signer_fallback_attempts();
        let decision = super::plan_missing_block_fetch_with_mode(
            &mut self.pending.missing_block_requests,
            block_hash,
            block_height,
            block_view,
            crate::sumeragi::consensus::Phase::Commit,
            super::MissingBlockPriority::Background,
            block_signers,
            topology,
            now,
            retry_window,
            None,
            signer_fallback_attempts,
            fetch_mode,
            false,
        );
        let dwell = self
            .pending
            .missing_block_requests
            .get(&block_hash)
            .map(|stats| now.saturating_duration_since(stats.first_seen))
            .unwrap_or_default();
        let targets_len = match &decision {
            super::MissingBlockFetchDecision::Requested { targets, .. } => targets.len(),
            _ => 0,
        };
        self.note_missing_block_fetch_metrics(&decision, retry_window, targets_len, dwell);

        // Invariant A: every sparse missing-QC signal must either advance request state in-place
        // or be explicitly backoff-suppressed with an existing request.
        let no_targets = matches!(&decision, super::MissingBlockFetchDecision::NoTargets);
        match &decision {
            super::MissingBlockFetchDecision::Requested {
                targets,
                target_kind,
            } => {
                self.request_missing_block(
                    block_hash,
                    block_height,
                    block_view,
                    super::MissingBlockPriority::Background,
                    &targets,
                );
                info!(
                    height = block_height,
                    view = block_view,
                    block = %block_hash,
                    block_signers = block_signer_count,
                    commit_quorum,
                    target_kind = target_kind.label(),
                    retry_window_ms = retry_window.as_millis(),
                    "requesting pending block to recover missing QC"
                );
            }
            super::MissingBlockFetchDecision::Backoff => {
                trace!(
                    height = block_height,
                    view = block_view,
                    block = %block_hash,
                    retry_window_ms = retry_window.as_millis(),
                    "suppressing duplicate pending-block fetch during missing-block backoff"
                );
            }
            super::MissingBlockFetchDecision::NoTargets => {
                warn!(
                    height = block_height,
                    view = block_view,
                    block = %block_hash,
                    block_signers = block_signer_count,
                    commit_quorum,
                    retry_window_ms = retry_window.as_millis(),
                    "missing-block recovery tracked but no fetch targets available"
                );
            }
        }
        let tracked = self
            .pending
            .missing_block_requests
            .contains_key(&block_hash);
        debug_assert!(
            tracked || no_targets,
            "sparse missing-QC recovery must track request state unless no targets are available"
        );
        tracked
    }

    pub(super) fn try_replay_quarantined_block_sync_qcs(
        &mut self,
        now: Instant,
        tick_deadline: Option<Instant>,
    ) -> bool {
        if self.quarantined_block_sync_qcs.is_empty() {
            return false;
        }

        enum ReplayAction {
            Replay {
                key: QcVoteKey,
                qc: crate::sumeragi::consensus::Qc,
                reason: &'static str,
                target: QuarantinedQcTarget,
            },
            Escalate {
                key: QcVoteKey,
                qc: crate::sumeragi::consensus::Qc,
                reason: &'static str,
                target: QuarantinedQcTarget,
            },
            Expire {
                key: QcVoteKey,
                qc: crate::sumeragi::consensus::Qc,
                reason: &'static str,
                target: QuarantinedQcTarget,
            },
        }

        let mut actions = Vec::new();
        let keys: Vec<_> = self
            .quarantined_block_sync_qcs
            .keys()
            .cloned()
            .take(QUARANTINED_BLOCK_SYNC_QC_PER_TICK)
            .collect();
        for key in keys {
            if Self::tick_budget_exhausted(tick_deadline, Instant::now()) {
                break;
            }
            let Some(entry) = self.quarantined_block_sync_qcs.get(&key) else {
                continue;
            };
            if self.block_known_locally(entry.qc.subject_block_hash) {
                actions.push(ReplayAction::Replay {
                    key,
                    qc: entry.qc.clone(),
                    reason: entry.reason,
                    target: entry.target,
                });
                continue;
            }
            let age = now.saturating_duration_since(entry.first_seen);
            if age < self.recovery_deferred_qc_ttl() {
                continue;
            }
            if entry.escalated_fetch || entry.attempts >= QUARANTINED_BLOCK_SYNC_QC_MAX_ATTEMPTS {
                actions.push(ReplayAction::Expire {
                    key,
                    qc: entry.qc.clone(),
                    reason: entry.reason,
                    target: entry.target,
                });
            } else {
                actions.push(ReplayAction::Escalate {
                    key,
                    qc: entry.qc.clone(),
                    reason: entry.reason,
                    target: entry.target,
                });
            }
        }

        if actions.is_empty() {
            return false;
        }

        let mut progress = false;
        for action in actions {
            match action {
                ReplayAction::Replay {
                    key,
                    qc,
                    reason,
                    target,
                } => {
                    self.quarantined_block_sync_qcs.remove(&key);
                    match self.handle_qc(qc.clone()) {
                        Ok(()) => {
                            super::status::inc_blocksync_qc_revalidated();
                            #[cfg(feature = "telemetry")]
                            if let Some(telemetry) = self.telemetry_handle() {
                                telemetry.inc_blocksync_qc_revalidated();
                            }
                            progress = true;
                        }
                        Err(err) => {
                            warn!(
                                ?err,
                                reason,
                                ?target,
                                "failed to replay quarantined block-sync QC"
                            );
                            self.block_sync_qc_final_drop("replay_error");
                            progress = true;
                        }
                    }
                }
                ReplayAction::Escalate {
                    key,
                    qc,
                    reason,
                    target,
                } => {
                    if let Some(entry) = self.quarantined_block_sync_qcs.get_mut(&key) {
                        entry.escalated_fetch = true;
                        entry.attempts = entry.attempts.saturating_add(1);
                        entry.first_seen = now;
                        entry.last_attempt = now;
                    }
                    self.force_block_sync_fetch_for_qc(&qc, reason);
                    debug!(?target, reason, "escalated quarantined block-sync QC fetch");
                    progress = true;
                }
                ReplayAction::Expire {
                    key,
                    qc,
                    reason,
                    target,
                } => {
                    self.quarantined_block_sync_qcs.remove(&key);
                    self.force_block_sync_fetch_for_qc(&qc, reason);
                    self.block_sync_qc_final_drop("expired");
                    warn!(?target, reason, "quarantined block-sync QC expired");
                    let current_view = self.phase_tracker.current_view(qc.height).unwrap_or(0);
                    let _ = self.handle_roster_unavailable_recovery(
                        qc.height,
                        current_view,
                        Some(qc.subject_block_hash),
                        self.queue.queued_len(),
                        now,
                        super::ProposalDeferWarningKind::EmptyCommitTopologyProposal,
                        "quarantined_block_sync_qc_expired_empty_commit_topology",
                    );
                    progress = true;
                }
            }
        }

        progress
    }

    pub(super) fn enqueue_fetch_pending_block_response(&mut self, peer: PeerId, msg: BlockMessage) {
        let mut msg = BlockMessageWire::new(msg);
        if !self.prepare_background_block_message(&mut msg) {
            return;
        }
        let request = BackgroundRequest::Post { peer, msg };
        if self.config.debug.disable_background_worker {
            self.dispatch_background_inline(request);
            return;
        }
        let dispatched = {
            #[cfg(feature = "telemetry")]
            {
                background::dispatch_background_request(
                    self.background_post_tx.as_ref(),
                    request,
                    &self.telemetry,
                )
            }
            #[cfg(not(feature = "telemetry"))]
            {
                background::dispatch_background_request(self.background_post_tx.as_ref(), request)
            }
        };
        if let Err(request) = dispatched {
            self.dispatch_background_fallback(*request);
        }
    }

    fn dispatch_fetch_pending_block_response(
        &mut self,
        peer: PeerId,
        msg: BlockMessage,
        bypass_queue: bool,
    ) {
        if bypass_queue {
            let mut msg = BlockMessageWire::new(msg);
            if !self.prepare_background_block_message(&mut msg) {
                return;
            }
            #[cfg(test)]
            self.record_background_request(&BackgroundRequest::Post {
                peer: peer.clone(),
                msg: msg.clone(),
            });
            self.dispatch_background_fallback(BackgroundRequest::Post { peer, msg });
            return;
        }
        self.enqueue_fetch_pending_block_response(peer, msg);
    }

    pub(super) fn block_body_response_from_payload(
        block_hash: HashOf<BlockHeader>,
        height: u64,
        view: u64,
        body: BlockMessage,
    ) -> Option<super::message::BlockBodyResponse> {
        let body = match body {
            BlockMessage::BlockCreated(created) => {
                super::message::BlockBodyData::BlockCreated(created)
            }
            BlockMessage::BlockSyncUpdate(update) => {
                super::message::BlockBodyData::BlockSyncUpdate(update)
            }
            other => {
                iroha_logger::warn!(
                    height,
                    view,
                    block = %block_hash,
                    payload_kind = Self::block_message_kind(&other),
                    "exact body fetch payload builder returned unexpected variant; dropping response"
                );
                return None;
            }
        };
        Some(super::message::BlockBodyResponse {
            block_hash,
            height,
            view,
            body,
        })
    }

    pub(super) fn block_body_response_for_wire(
        &self,
        block: &SignedBlock,
    ) -> super::message::BlockBodyResponse {
        let block_hash = block.hash();
        let height = block.header().height().get();
        let view = block.header().view_change_index();
        let body = self.build_fetch_pending_block_payload(block);
        Self::block_body_response_from_payload(block_hash, height, view, body)
            .unwrap_or_else(|| self.plain_block_body_response_for_wire(block))
    }

    pub(super) fn plain_block_body_response_for_wire(
        &self,
        block: &SignedBlock,
    ) -> super::message::BlockBodyResponse {
        let block_hash = block.hash();
        let height = block.header().height().get();
        let view = block.header().view_change_index();
        super::message::BlockBodyResponse {
            block_hash,
            height,
            view,
            body: super::message::BlockBodyData::BlockCreated(
                self.frontier_block_created_for_wire(block),
            ),
        }
    }

    pub(super) fn dispatch_block_body_response_with_plain_fallback(
        &mut self,
        peer: PeerId,
        block: &SignedBlock,
        response: super::message::BlockBodyResponse,
    ) {
        let direct_commit_qc = self.direct_commit_qc_from_block_body_response(&response);
        let is_sync = matches!(
            &response.body,
            super::message::BlockBodyData::BlockSyncUpdate(_)
        );
        let created_companion_sent =
            self.dispatch_block_created_companion_for_body_response(peer.clone(), block);
        let dispatch_decision = block_body_response_dispatch_decision(
            is_sync,
            created_companion_sent,
            direct_commit_qc.is_some(),
        );
        debug_assert!(dispatch_decision.all_bypass);
        debug_assert_eq!(dispatch_decision.created_companion, created_companion_sent);
        debug_assert_eq!(dispatch_decision.plain_fallback, is_sync);
        debug_assert!(dispatch_decision.response);
        debug_assert_eq!(dispatch_decision.qc_companion, direct_commit_qc.is_some());

        let mut dispatch_position = 0_u8;
        if created_companion_sent {
            dispatch_position += 1;
            debug_assert_eq!(dispatch_decision.pos_created, dispatch_position);
        } else {
            debug_assert_eq!(dispatch_decision.pos_created, 0);
        }
        if is_sync {
            dispatch_position += 1;
            debug_assert_eq!(dispatch_decision.pos_plain, dispatch_position);
            let plain = self.plain_block_body_response_for_wire(block);
            self.dispatch_fetch_pending_block_response(
                peer.clone(),
                BlockMessage::BlockBodyResponse(plain),
                /*bypass_queue*/ true,
            );
        } else {
            debug_assert_eq!(dispatch_decision.pos_plain, 0);
        }
        dispatch_position += 1;
        debug_assert_eq!(dispatch_decision.pos_response, dispatch_position);
        self.dispatch_fetch_pending_block_response(
            peer.clone(),
            BlockMessage::BlockBodyResponse(response),
            /*bypass_queue*/ true,
        );
        if let Some(qc) = direct_commit_qc {
            dispatch_position += 1;
            debug_assert_eq!(dispatch_decision.pos_qc, dispatch_position);
            let header = block.header();
            self.dispatch_direct_commit_qc_companion(
                peer,
                qc,
                block.hash(),
                header.height().get(),
                header.view_change_index(),
                "exact_block_body_response",
            );
        } else {
            debug_assert_eq!(dispatch_decision.pos_qc, 0);
        }
    }

    fn dispatch_block_created_companion_for_body_response(
        &mut self,
        peer: PeerId,
        block: &SignedBlock,
    ) -> bool {
        let created = BlockMessage::BlockCreated(self.frontier_block_created_for_wire(block));
        let created_len = super::consensus_block_wire_len(self.common_config.peer.id(), &created);
        let header = block.header();
        if created_len > self.consensus_payload_frame_cap {
            warn!(
                height = header.height().get(),
                view = header.view_change_index(),
                block = %block.hash(),
                cap = self.consensus_payload_frame_cap,
                created_len,
                "skipping BlockCreated companion for exact body response; payload exceeds frame cap"
            );
            return false;
        }
        debug!(
            height = header.height().get(),
            view = header.view_change_index(),
            block = %block.hash(),
            peer = %peer,
            "sending BlockCreated companion for exact body response"
        );
        self.dispatch_fetch_pending_block_response(peer, created, /*bypass_queue*/ true);
        true
    }

    fn direct_commit_qc_for_block(
        &mut self,
        block: &SignedBlock,
    ) -> Option<crate::sumeragi::consensus::Qc> {
        let block_hash = block.hash();
        let height = block.header().height().get();
        let view = block.header().view_change_index();
        if let Some(qc) = self.cached_commit_qc_for_block(block_hash, height, view) {
            debug_assert_eq!(
                direct_commit_qc_for_block_decision(true, false, false, false, 0, 1, false).result,
                DirectCommitQcForBlockResult::Cache
            );
            return Some(qc);
        }

        {
            let world = self.state.world_view();
            if let Some(qc) = crate::block_sync::BlockSynchronizer::block_sync_qc_for_world(
                &world,
                self.config.consensus_mode,
                block,
            ) {
                let decision =
                    direct_commit_qc_for_block_decision(false, true, false, false, 0, 1, false);
                debug_assert!(decision.world_consulted);
                debug_assert_eq!(decision.result, DirectCommitQcForBlockResult::World);
                return Some(qc);
            }
        }

        let (consensus_mode, _, _) = self.consensus_context_for_height(height);
        let primary_commit_topology =
            self.roster_for_vote_with_mode(block_hash, height, view, consensus_mode);
        let primary_topology_available = !primary_commit_topology.is_empty();
        let (commit_topology, fallback_topology_available) = if primary_topology_available {
            (primary_commit_topology, false)
        } else {
            let fallback = self.effective_commit_topology();
            let fallback_available = !fallback.is_empty();
            (fallback, fallback_available)
        };
        if commit_topology.is_empty() {
            let decision = direct_commit_qc_for_block_decision(
                false,
                false,
                primary_topology_available,
                fallback_topology_available,
                0,
                1,
                false,
            );
            debug_assert_eq!(decision.topology_source, DirectCommitQcTopologySource::None);
            return None;
        }

        let topology = super::network_topology::Topology::new(commit_topology);
        let pending_commit_votes = self.pending_block_commit_votes_count(block_hash, height, view);
        let preform_decision = direct_commit_qc_for_block_decision(
            false,
            false,
            primary_topology_available,
            fallback_topology_available,
            pending_commit_votes,
            topology.min_votes_for_commit(),
            false,
        );
        if !preform_decision.try_form {
            return None;
        }
        debug_assert!(preform_decision.try_phase_commit);
        debug_assert!(preform_decision.try_subject_block);

        self.try_form_qc_from_votes(
            crate::sumeragi::consensus::Phase::Commit,
            block_hash,
            height,
            view,
            self.epoch_for_height(height),
            &topology,
        );
        let formed = self.cached_commit_qc_for_block(block_hash, height, view);
        let final_decision = direct_commit_qc_for_block_decision(
            false,
            false,
            primary_topology_available,
            fallback_topology_available,
            pending_commit_votes,
            topology.min_votes_for_commit(),
            formed.is_some(),
        );
        debug_assert_eq!(
            final_decision.result,
            if formed.is_some() {
                DirectCommitQcForBlockResult::Formed
            } else {
                DirectCommitQcForBlockResult::None
            }
        );
        if formed.is_some() {
            debug!(
                height,
                view,
                block = %block_hash,
                "formed direct commit QC from cached votes for fetch response"
            );
        }
        formed
    }

    fn validator_checkpoint_from_commit_qc(
        qc: &crate::sumeragi::consensus::Qc,
    ) -> ValidatorSetCheckpoint {
        ValidatorSetCheckpoint::new_with_chain_order(
            qc.height,
            qc.view,
            qc.subject_block_hash,
            qc.chain_order_hash,
            qc.rechain_seq,
            qc.parent_state_root,
            qc.post_state_root,
            qc.validator_set.clone(),
            qc.aggregate.signers_bitmap.clone(),
            qc.aggregate.bls_aggregate_signature.clone(),
            qc.validator_set_hash_version,
            None,
        )
    }

    pub(super) fn certified_block_fetch_response_for_block_with_qc(
        &self,
        block: &SignedBlock,
        commit_qc: crate::sumeragi::consensus::Qc,
    ) -> Option<super::message::CertifiedBlockFetchResponse> {
        let block_hash = block.hash();
        let height = block.header().height().get();
        let view = block.header().view_change_index();
        if commit_qc.subject_block_hash != block_hash
            || commit_qc.height != height
            || commit_qc.view != view
            || !matches!(commit_qc.phase, crate::sumeragi::consensus::Phase::Commit)
        {
            warn!(
                height,
                view,
                block = %block_hash,
                qc_height = commit_qc.height,
                qc_view = commit_qc.view,
                qc_block = %commit_qc.subject_block_hash,
                "skipping certified block fetch response with mismatched commit QC"
            );
            return None;
        }
        let (consensus_mode, _, _) = self.consensus_context_for_height(height);
        let stake_snapshot = match consensus_mode {
            ConsensusMode::Permissioned => None,
            ConsensusMode::Npos => self
                .state
                .commit_roster_snapshot_for_block(height, block_hash)
                .and_then(|snapshot| snapshot.stake_snapshot)
                .filter(|snapshot| snapshot.matches_roster(&commit_qc.validator_set))
                .or_else(|| {
                    self.roster_validation_cache
                        .stake_snapshot_for_roster(&commit_qc.validator_set)
                }),
        };
        Some(super::message::CertifiedBlockFetchResponse {
            height,
            view,
            block: block.clone(),
            validator_checkpoint: Self::validator_checkpoint_from_commit_qc(&commit_qc),
            commit_qc,
            stake_snapshot,
        })
    }

    pub(super) fn certified_block_fetch_response_for_block(
        &mut self,
        block: &SignedBlock,
    ) -> Option<super::message::CertifiedBlockFetchResponse> {
        let commit_qc = self.direct_commit_qc_for_block(block)?;
        self.certified_block_fetch_response_for_block_with_qc(block, commit_qc)
    }

    fn certified_block_fetch_proof_for_response(
        response: &super::message::CertifiedBlockFetchResponse,
    ) -> super::message::CertifiedBlockFetchProof {
        super::message::CertifiedBlockFetchProof {
            height: response.height,
            view: response.view,
            block_hash: response.block.hash(),
            commit_qc: response.commit_qc.clone(),
            validator_checkpoint: response.validator_checkpoint.clone(),
            stake_snapshot: response.stake_snapshot.clone(),
        }
    }

    pub(super) fn dispatch_certified_block_fetch_proof(
        &mut self,
        peer: PeerId,
        response: &super::message::CertifiedBlockFetchResponse,
        source: &'static str,
    ) -> bool {
        let proof = Self::certified_block_fetch_proof_for_response(response);
        let msg =
            BlockMessage::CertifiedBlockFetch(super::message::CertifiedBlockFetch::Proof(proof));
        let origin = self.common_config.peer.id().clone();
        let cap = self.block_message_frame_cap(&msg);
        let proof_len = super::consensus_block_wire_len(&origin, &msg);
        let block_hash = response.block.hash();
        if proof_len > cap {
            warn!(
                height = response.height,
                view = response.view,
                block = %block_hash,
                cap,
                proof_len,
                source,
                "dropping oversized certified commit proof companion"
            );
            return false;
        }
        info!(
            height = response.height,
            view = response.view,
            block = %block_hash,
            peer = %peer,
            source,
            "sending certified commit proof companion"
        );
        self.dispatch_fetch_pending_block_response(peer, msg, /*bypass_queue*/ true);
        true
    }

    fn certified_block_fetch_body_for_response(
        response: &super::message::CertifiedBlockFetchResponse,
    ) -> super::message::CertifiedBlockFetchBody {
        super::message::CertifiedBlockFetchBody {
            height: response.height,
            view: response.view,
            block: response.block.clone(),
        }
    }

    pub(super) fn dispatch_certified_block_fetch_response(
        &mut self,
        peer: PeerId,
        response: super::message::CertifiedBlockFetchResponse,
    ) -> bool {
        let full = BlockMessage::CertifiedBlockFetch(
            super::message::CertifiedBlockFetch::Response(response.clone()),
        );
        let origin = self.common_config.peer.id().clone();
        let cap = self.block_message_frame_cap(&full);
        let full_len = super::consensus_block_wire_len(&origin, &full);
        if full_len <= cap {
            self.dispatch_fetch_pending_block_response(peer, full, /*bypass_queue*/ true);
            return true;
        }

        let block = response.block.clone();
        let block_hash = block.hash();
        let height = response.height;
        let view = response.view;
        let proof = Self::certified_block_fetch_proof_for_response(&response);
        let proof_msg =
            BlockMessage::CertifiedBlockFetch(super::message::CertifiedBlockFetch::Proof(proof));
        let proof_len = super::consensus_block_wire_len(&origin, &proof_msg);
        if proof_len > cap {
            warn!(
                height,
                view,
                block = %block_hash,
                cap,
                full_len,
                proof_len,
                "dropping oversized certified block fetch response; proof companion also exceeds frame cap"
            );
            return false;
        }

        let body = Self::certified_block_fetch_body_for_response(&response);
        let body_msg =
            BlockMessage::CertifiedBlockFetch(super::message::CertifiedBlockFetch::Body(body));
        let body_len = super::consensus_block_wire_len(&origin, &body_msg);
        self.dispatch_fetch_pending_block_response(
            peer.clone(),
            proof_msg,
            /*bypass_queue*/ true,
        );
        if body_len <= cap {
            self.dispatch_fetch_pending_block_response(
                peer.clone(),
                body_msg,
                /*bypass_queue*/ true,
            );
        } else {
            let body_response =
                BlockMessage::BlockBodyResponse(self.plain_block_body_response_for_wire(&block));
            let body_response_len = super::consensus_block_wire_len(&origin, &body_response);
            if body_response_len <= cap {
                self.dispatch_fetch_pending_block_response(
                    peer.clone(),
                    body_response,
                    /*bypass_queue*/ true,
                );
            } else {
                let created =
                    BlockMessage::BlockCreated(self.frontier_block_created_for_wire(&block));
                let created_len = super::consensus_block_wire_len(&origin, &created);
                if created_len <= cap {
                    self.dispatch_fetch_pending_block_response(
                        peer.clone(),
                        created,
                        /*bypass_queue*/ true,
                    );
                } else {
                    warn!(
                        height,
                        view,
                        block = %block_hash,
                        cap,
                        full_len,
                        body_len,
                        body_response_len,
                        created_len,
                        "skipping certified block fetch body split; block body exceeds frame cap"
                    );
                }
            }
        }
        info!(
            height,
            view,
            block = %block_hash,
            cap,
            full_len,
            proof_len,
            body_len,
            "split oversized certified block fetch response into proof and body"
        );
        true
    }

    fn materialize_certified_block_fetch_response(
        &mut self,
        response: super::message::CertifiedBlockFetchResponse,
    ) -> bool {
        let block = response.block;
        let block_hash = block.hash();
        let height = block.header().height().get();
        let view = block.header().view_change_index();
        let payload_bytes = super::proposals::block_payload_bytes(&block);
        let payload_hash = Hash::new(&payload_bytes);
        if let Some(inflight) = self.subsystems.commit.inflight.as_mut()
            && inflight.block_hash == block_hash
        {
            if matches!(
                inflight.pending.validation_status,
                ValidationStatus::Invalid
            ) {
                warn!(
                    height,
                    view,
                    block = %block_hash,
                    "dropping certified block fetch response for invalid inflight block"
                );
                return false;
            }
            inflight
                .pending
                .note_commit_qc_observed(response.commit_qc.epoch);
            self.maybe_cache_rehydrated_kura_body(&block);
            self.clear_missing_block_request(
                &block_hash,
                MissingBlockClearReason::PayloadAvailable,
            );
            self.clear_missing_block_view_change(&block_hash);
            self.request_commit_pipeline_for_pending(
                block_hash,
                super::status::RoundEventCauseTrace::BlockAvailable,
                None,
            );
            return true;
        }
        match self.pending.pending_blocks.entry(block_hash) {
            Entry::Occupied(mut entry) => {
                let pending = entry.get_mut();
                if matches!(pending.validation_status, ValidationStatus::Invalid) {
                    warn!(
                        height,
                        view,
                        block = %block_hash,
                        "dropping certified block fetch response for invalid pending block"
                    );
                    return false;
                }
                if pending.is_retry_aborted() {
                    pending.revive_after_abort_with_payload_bytes(
                        block.clone(),
                        payload_hash,
                        height,
                        view,
                        payload_bytes,
                    );
                } else {
                    pending.replace_block_with_payload_bytes(
                        block.clone(),
                        payload_hash,
                        height,
                        view,
                        payload_bytes,
                    );
                }
                pending.note_commit_qc_observed(response.commit_qc.epoch);
            }
            Entry::Vacant(entry) => {
                let mut pending = PendingBlock::new_with_payload_bytes(
                    block.clone(),
                    payload_hash,
                    height,
                    view,
                    payload_bytes,
                );
                pending.note_commit_qc_observed(response.commit_qc.epoch);
                entry.insert(pending);
            }
        }
        self.maybe_cache_rehydrated_kura_body(&block);
        self.deferred_missing_payload_qcs
            .remove(&Self::qc_tally_key(&response.commit_qc));
        self.deferred_block_sync_updates
            .remove(&(height, view, block_hash));
        self.flush_frontier_body_requesters(&block);
        self.flush_pending_block_body_requests_if_ready(&block);
        self.flush_pending_fetch_requests(&block);
        self.clear_missing_block_request(&block_hash, MissingBlockClearReason::PayloadAvailable);
        self.clear_missing_block_view_change(&block_hash);
        let _ = self.try_replay_deferred_qcs();
        let _ = self.try_replay_deferred_missing_payload_qcs(Instant::now());
        self.request_commit_pipeline_for_pending(
            block_hash,
            super::status::RoundEventCauseTrace::BlockAvailable,
            None,
        );
        info!(
            height,
            view,
            block = %block_hash,
            "materialized certified block fetch response as pending block"
        );
        true
    }

    pub(super) fn handle_certified_block_fetch(
        &mut self,
        fetch: super::message::CertifiedBlockFetch,
        sender: Option<PeerId>,
    ) -> Result<()> {
        match fetch {
            super::message::CertifiedBlockFetch::Request(request) => {
                self.handle_certified_block_fetch_request(request, sender);
                Ok(())
            }
            super::message::CertifiedBlockFetch::Response(response) => {
                self.handle_certified_block_fetch_response(response)
            }
            super::message::CertifiedBlockFetch::Proof(proof) => {
                self.handle_certified_block_fetch_proof(proof)
            }
            super::message::CertifiedBlockFetch::Body(body) => {
                self.handle_certified_block_fetch_body(body)
            }
        }
    }

    fn handle_certified_block_fetch_request(
        &mut self,
        request: super::message::CertifiedBlockFetchRequest,
        sender: Option<PeerId>,
    ) {
        if let Some(sender) = sender.as_ref()
            && sender != &request.requester
        {
            warn!(
                authenticated_sender = %sender,
                claimed_requester = %request.requester,
                height = request.height,
                view = request.view,
                block = %request.block_hash,
                "dropping certified block fetch request with mismatched requester"
            );
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::CertifiedBlockFetch,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::InvalidPayload,
            );
            return;
        }
        let Some(block) = self.local_signed_block_for_body_repair(request.block_hash) else {
            debug!(
                height = request.height,
                view = request.view,
                block = %request.block_hash,
                requester = %request.requester,
                "unable to serve certified block fetch request: local block missing"
            );
            return;
        };
        if block.header().height().get() != request.height
            || block.header().view_change_index() != request.view
            || block.hash() != request.block_hash
        {
            warn!(
                height = request.height,
                view = request.view,
                block = %request.block_hash,
                local_height = block.header().height().get(),
                local_view = block.header().view_change_index(),
                local_block = %block.hash(),
                "dropping certified block fetch request with mismatched local subject"
            );
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::CertifiedBlockFetch,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::InvalidPayload,
            );
            return;
        }
        let Some(response) = self.certified_block_fetch_response_for_block(block.as_ref()) else {
            debug!(
                height = request.height,
                view = request.view,
                block = %request.block_hash,
                requester = %request.requester,
                "unable to serve certified block fetch request: commit QC unavailable"
            );
            return;
        };
        info!(
            height = request.height,
            view = request.view,
            block = %request.block_hash,
            requester = %request.requester,
            "sending certified block fetch response"
        );
        self.dispatch_certified_block_fetch_response(request.requester, response);
    }

    fn accept_certified_block_fetch_proof(
        &mut self,
        proof: &super::message::CertifiedBlockFetchProof,
    ) -> Result<bool> {
        if let Err(err) = proof.validate_subject() {
            warn!(
                ?err,
                height = proof.height,
                view = proof.view,
                block = %proof.block_hash,
                "dropping malformed certified block fetch proof"
            );
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::CertifiedBlockFetch,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::InvalidPayload,
            );
            return Ok(false);
        }
        let block_hash = proof.block_hash;
        let height = proof.height;
        let view = proof.view;
        let commit_qc = proof.commit_qc.clone();
        self.handle_qc_with_stake_snapshot(commit_qc.clone(), proof.stake_snapshot.clone())?;
        if self
            .cached_commit_qc_for_block(block_hash, height, view)
            .is_none()
        {
            warn!(
                height,
                view,
                block = %block_hash,
                "dropping certified block fetch response whose commit QC was not accepted locally"
            );
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::CertifiedBlockFetch,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::InvalidPayload,
            );
            return Ok(false);
        }
        self.state.record_commit_roster(
            &commit_qc,
            &proof.validator_checkpoint,
            proof.stake_snapshot.clone(),
        );
        super::status::record_commit_qc(commit_qc.clone());
        self.clear_missing_commit_qc_request(&block_hash, MissingBlockClearReason::Obsolete);
        Ok(true)
    }

    fn handle_certified_block_fetch_proof(
        &mut self,
        proof: super::message::CertifiedBlockFetchProof,
    ) -> Result<()> {
        let _ = self.accept_certified_block_fetch_proof(&proof)?;
        Ok(())
    }

    fn handle_certified_block_fetch_body(
        &mut self,
        body: super::message::CertifiedBlockFetchBody,
    ) -> Result<()> {
        if let Err(err) = body.validate_subject() {
            warn!(
                ?err,
                height = body.height,
                view = body.view,
                block = %body.block.hash(),
                "dropping malformed certified block fetch body"
            );
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::CertifiedBlockFetch,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::InvalidPayload,
            );
            return Ok(());
        }
        let block_hash = body.block.hash();
        let height = body.height;
        let view = body.view;
        let Some(commit_qc) = self.cached_commit_qc_for_block(block_hash, height, view) else {
            warn!(
                height,
                view,
                block = %block_hash,
                "dropping certified block fetch body without an accepted commit proof"
            );
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::CertifiedBlockFetch,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::QuorumMissing,
            );
            return Ok(());
        };
        let snapshot = self
            .state
            .commit_roster_snapshot_for_block(height, block_hash);
        let validator_checkpoint = snapshot
            .as_ref()
            .map(|snapshot| snapshot.validator_checkpoint.clone())
            .unwrap_or_else(|| Self::validator_checkpoint_from_commit_qc(&commit_qc));
        let stake_snapshot = snapshot
            .and_then(|snapshot| snapshot.stake_snapshot)
            .filter(|snapshot| snapshot.matches_roster(&commit_qc.validator_set))
            .or_else(|| match self.consensus_context_for_height(height).0 {
                ConsensusMode::Permissioned => None,
                ConsensusMode::Npos => self
                    .roster_validation_cache
                    .stake_snapshot_for_roster(&commit_qc.validator_set),
            });
        let response = super::message::CertifiedBlockFetchResponse {
            height,
            view,
            block: body.block,
            commit_qc,
            validator_checkpoint,
            stake_snapshot,
        };
        self.materialize_certified_block_fetch_response(response);
        Ok(())
    }

    fn handle_certified_block_fetch_response(
        &mut self,
        response: super::message::CertifiedBlockFetchResponse,
    ) -> Result<()> {
        if let Err(err) = response.validate_subject() {
            warn!(
                ?err,
                height = response.height,
                view = response.view,
                block = %response.block.hash(),
                "dropping malformed certified block fetch response"
            );
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::CertifiedBlockFetch,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::InvalidPayload,
            );
            return Ok(());
        }
        let proof = Self::certified_block_fetch_proof_for_response(&response);
        if !self.accept_certified_block_fetch_proof(&proof)? {
            return Ok(());
        }
        self.materialize_certified_block_fetch_response(response);
        Ok(())
    }

    fn direct_commit_qc_from_block_sync_update(
        &self,
        block_hash: HashOf<BlockHeader>,
        height: u64,
        view: u64,
        update: &super::message::BlockSyncUpdate,
    ) -> Option<crate::sumeragi::consensus::Qc> {
        update.commit_qc.clone().or_else(|| {
            update.validator_checkpoint.as_ref().and_then(|checkpoint| {
                self.commit_qc_from_validator_checkpoint(
                    block_hash,
                    height,
                    view,
                    checkpoint,
                    update.stake_snapshot.as_ref(),
                )
            })
        })
    }

    fn direct_commit_qc_from_block_body_response(
        &mut self,
        response: &super::message::BlockBodyResponse,
    ) -> Option<crate::sumeragi::consensus::Qc> {
        match &response.body {
            super::message::BlockBodyData::BlockSyncUpdate(update) => {
                let header = update.block.header();
                let identity_matches = update.block.hash() == response.block_hash
                    && header.height().get() == response.height
                    && header.view_change_index() == response.view;
                if !identity_matches {
                    debug_assert_eq!(
                        block_body_direct_commit_qc_update_source(false, false, false, false),
                        BlockBodyDirectCommitQcSource::None
                    );
                    return None;
                }
                let embedded_qc = update.commit_qc.clone();
                let checkpoint_qc = if embedded_qc.is_none() {
                    update.validator_checkpoint.as_ref().and_then(|checkpoint| {
                        self.commit_qc_from_validator_checkpoint(
                            response.block_hash,
                            response.height,
                            response.view,
                            checkpoint,
                            update.stake_snapshot.as_ref(),
                        )
                    })
                } else {
                    None
                };
                let local_qc = if embedded_qc.is_none() && checkpoint_qc.is_none() {
                    self.direct_commit_qc_for_block(&update.block)
                } else {
                    None
                };
                let decision = block_body_direct_commit_qc_update_source(
                    true,
                    embedded_qc.is_some(),
                    checkpoint_qc.is_some(),
                    local_qc.is_some(),
                );
                debug_assert_eq!(
                    decision,
                    if embedded_qc.is_some() {
                        BlockBodyDirectCommitQcSource::Embedded
                    } else if checkpoint_qc.is_some() {
                        BlockBodyDirectCommitQcSource::Checkpoint
                    } else if local_qc.is_some() {
                        BlockBodyDirectCommitQcSource::Local
                    } else {
                        BlockBodyDirectCommitQcSource::None
                    }
                );
                embedded_qc.or(checkpoint_qc).or(local_qc)
            }
            super::message::BlockBodyData::BlockCreated(created) => {
                let header = created.block.header();
                let identity_matches = created.block.hash() == response.block_hash
                    && header.height().get() == response.height
                    && header.view_change_index() == response.view;
                if !identity_matches {
                    debug_assert_eq!(
                        block_body_direct_commit_qc_created_source(false, false),
                        BlockBodyDirectCommitQcSource::None
                    );
                    return None;
                }
                let local_qc = self.direct_commit_qc_for_block(&created.block);
                debug_assert_eq!(
                    block_body_direct_commit_qc_created_source(true, local_qc.is_some()),
                    if local_qc.is_some() {
                        BlockBodyDirectCommitQcSource::Local
                    } else {
                        BlockBodyDirectCommitQcSource::None
                    }
                );
                local_qc
            }
        }
    }

    fn dispatch_direct_commit_qc_companion(
        &mut self,
        peer: PeerId,
        qc: crate::sumeragi::consensus::Qc,
        block_hash: HashOf<BlockHeader>,
        height: u64,
        view: u64,
        source: &'static str,
    ) {
        info!(
            height,
            view,
            block = %block_hash,
            peer = %peer,
            source,
            "sending direct commit QC companion"
        );
        self.dispatch_fetch_pending_block_response(
            peer,
            BlockMessage::Qc(qc),
            /*bypass_queue*/ true,
        );
    }

    pub(super) fn dispatch_commit_qc_only_fetch_response(
        &mut self,
        peer: PeerId,
        block: &SignedBlock,
        priority: FetchPendingBlockPriority,
        requester_roster_proof_known: bool,
    ) -> bool {
        let block_hash = block.hash();
        let header = block.header();
        let height = header.height().get();
        let view = header.view_change_index();
        let Some(qc) = self.direct_commit_qc_for_block(block) else {
            let mut replay_targets =
                self.known_block_commit_qc_recovery_targets(block_hash, height, view, &[]);
            replay_targets.push(peer.clone());
            replay_targets.sort();
            replay_targets.dedup();
            let replayed_votes = self.rebroadcast_block_votes_to_targets_with_backpressure(
                crate::sumeragi::consensus::Phase::Commit,
                block_hash,
                height,
                view,
                &replay_targets,
                true,
                "commit_qc_only_fetch_response",
            );
            if replayed_votes > 0 {
                info!(
                    height,
                    view,
                    block = %block_hash,
                    peer = %peer,
                    targets = replay_targets.len(),
                    replayed_votes,
                    "sending cached commit votes for commit-QC-only fetch response"
                );
            }
            if self.committed_signed_quorum_fetch_fallback_available(block) {
                let update = self.signed_quorum_fetch_fallback_update(block);
                info!(
                    height,
                    view,
                    block = %block_hash,
                    peer = %peer,
                    replayed_votes,
                    "sending signed-quorum block sync fallback for commit-QC-only fetch response"
                );
                self.send_fetch_pending_block_response(
                    peer,
                    BlockMessage::BlockSyncUpdate(update),
                    priority,
                    /*force_bypass_queue*/ true,
                    /*allow_highest_qc_bypass*/ true,
                    /*allow_hintless_block_sync_bypass*/ true,
                    requester_roster_proof_known,
                );
                return true;
            }
            debug!(
                height,
                view,
                block = %block_hash,
                peer = %peer,
                "deferring commit-QC-only fetch response: commit QC unavailable"
            );
            return false;
        };
        info!(
            height,
            view,
            block = %block_hash,
            peer = %peer,
            "sending commit-QC-only fetch response"
        );
        if let Some(response) =
            self.certified_block_fetch_response_for_block_with_qc(block, qc.clone())
        {
            let _ = self.dispatch_certified_block_fetch_proof(
                peer.clone(),
                &response,
                "fetch_pending_block_commit_qc_only",
            );
        }
        self.dispatch_direct_commit_qc_companion(
            peer,
            qc,
            block_hash,
            height,
            view,
            "fetch_pending_block_commit_qc_only",
        );
        true
    }

    pub(super) fn committed_signed_quorum_fetch_fallback_available(
        &self,
        block: &SignedBlock,
    ) -> bool {
        let block_hash = block.hash();
        let height = block.header().height().get();
        if self.committed_block_hash_for_height(height) != Some(block_hash) {
            return false;
        }

        self.signed_commit_quorum_signer_count(block).is_some()
    }

    pub(super) fn signed_commit_quorum_signer_count(&self, block: &SignedBlock) -> Option<usize> {
        let block_hash = block.hash();
        let height = block.header().height().get();
        let view = block.header().view_change_index();
        let (consensus_mode, mode_tag, prf_seed) = self.consensus_context_for_height(height);
        let mut commit_roster =
            self.roster_for_vote_with_mode(block_hash, height, view, consensus_mode);
        if commit_roster.is_empty() {
            commit_roster = self.effective_commit_topology();
        }
        if commit_roster.is_empty() {
            return None;
        }

        let topology = super::network_topology::Topology::new(commit_roster);
        let world = self.state.world_view();
        let Ok(block_signers) =
            super::validated_block_signers_from_world(block, &topology, &world, mode_tag, prf_seed)
        else {
            return None;
        };
        match consensus_mode {
            ConsensusMode::Permissioned => (block_signers.len()
                >= topology.min_votes_for_commit().max(1))
            .then_some(block_signers.len()),
            ConsensusMode::Npos => {
                let stake_snapshot = self
                    .state
                    .commit_roster_snapshot_for_block(height, block_hash)
                    .and_then(|snapshot| snapshot.stake_snapshot)
                    .filter(|snapshot| snapshot.matches_roster(topology.as_ref()))
                    .or_else(|| {
                        self.roster_validation_cache
                            .stake_snapshot_for_roster(topology.as_ref())
                    });
                let Some(stake_snapshot) = stake_snapshot.as_ref() else {
                    return None;
                };
                let Ok(signer_peers) = super::signer_peers_for_topology(&block_signers, &topology)
                else {
                    return None;
                };
                super::stake_snapshot::stake_quorum_reached_for_snapshot(
                    stake_snapshot,
                    topology.as_ref(),
                    &signer_peers,
                )
                .unwrap_or(false)
                .then_some(block_signers.len())
            }
        }
    }

    fn signed_quorum_fetch_fallback_update(
        &self,
        block: &SignedBlock,
    ) -> super::message::BlockSyncUpdate {
        super::block_sync_update_with_roster(
            block,
            self.state.as_ref(),
            self.kura.as_ref(),
            self.config.consensus_mode,
            self.common_config.trusted_peers.value(),
            self.common_config.peer.id(),
            &self.roster_validation_cache,
        )
    }

    pub(super) fn send_block_body_response(&mut self, peer: PeerId, block: &SignedBlock) {
        let header = block.header();
        debug!(
            height = header.height().get(),
            view = header.view_change_index(),
            block = %block.hash(),
            peer = %peer,
            "sending exact BlockBodyResponse"
        );
        let response = self.block_body_response_for_wire(block);
        self.dispatch_block_body_response_with_plain_fallback(peer, block, response);
    }

    pub(super) fn flush_frontier_body_requesters(&mut self, block: &SignedBlock) {
        let block_hash = block.hash();
        let height = block.header().height().get();
        let view = block.header().view_change_index();
        let Some(slot) = self.frontier_slot.as_mut() else {
            return;
        };
        if slot.block_hash != block_hash || slot.height != height || slot.view != view {
            return;
        }
        let now = Instant::now();
        slot.mark_body_available(now);
        let requesters = slot.take_pending_requesters();
        for peer in requesters {
            self.send_block_body_response(peer, block);
        }
    }

    fn fetch_response_targets_highest_qc(&self, msg: &BlockMessage) -> bool {
        let Some(highest) = self.highest_qc else {
            return false;
        };
        let (block_hash, height, view) = match msg {
            BlockMessage::BlockSyncUpdate(update) => {
                let header = update.block.header();
                (
                    update.block.hash(),
                    header.height().get(),
                    header.view_change_index(),
                )
            }
            BlockMessage::BlockCreated(created) => {
                let header = created.block.header();
                (
                    created.block.hash(),
                    header.height().get(),
                    header.view_change_index(),
                )
            }
            BlockMessage::RbcInit(init) => (init.block_hash, init.height, init.view),
            BlockMessage::RbcChunk(chunk) => (chunk.block_hash, chunk.height, chunk.view),
            BlockMessage::RbcChunkCompact(chunk) => (
                chunk.block_hash,
                u64::from(chunk.height),
                u64::from(chunk.view),
            ),
            _ => return false,
        };
        highest.subject_block_hash == block_hash && highest.height == height && highest.view == view
    }

    fn fetch_response_should_bypass_queue(
        &self,
        msg: &BlockMessage,
        allow_highest_qc_bypass: bool,
    ) -> bool {
        (allow_highest_qc_bypass && self.fetch_response_targets_highest_qc(msg))
            || matches!(
                msg,
                // Missing-block recovery must deliver the block payload eagerly; otherwise
                // peers can receive RBC chunks first and stall waiting for BlockCreated.
                BlockMessage::BlockCreated(_)
                    | BlockMessage::RbcInit(_)
                    | BlockMessage::RbcChunk(_)
                    | BlockMessage::RbcChunkCompact(_)
                    | BlockMessage::RbcReady(_)
                    | BlockMessage::RbcDeliver(_)
            )
    }

    fn send_fetch_pending_block_response(
        &mut self,
        peer: PeerId,
        mut msg: BlockMessage,
        priority: FetchPendingBlockPriority,
        force_bypass_queue: bool,
        allow_highest_qc_bypass: bool,
        allow_hintless_block_sync_bypass: bool,
        requester_roster_proof_known: bool,
    ) {
        let mut direct_commit_qc = None;
        let initial_kind = FetchPendingResponsePayloadKind::from_message(&msg);
        let mut hintless_block_sync = matches!(
            &msg,
            BlockMessage::BlockSyncUpdate(update)
                if update.commit_qc.is_none() && update.validator_checkpoint.is_none()
        );
        let preflight = fetch_pending_response_preflight_decision(
            initial_kind,
            hintless_block_sync,
            force_bypass_queue,
            priority,
            self.fetch_response_targets_highest_qc(&msg),
            allow_highest_qc_bypass,
            allow_hintless_block_sync_bypass,
            requester_roster_proof_known,
        );
        debug_assert_eq!(
            preflight.hintless_allowed,
            matches!(
                super::decide_hintless_block_sync_response_policy(
                    requester_roster_proof_known,
                    allow_hintless_block_sync_bypass,
                ),
                super::HintlessBlockSyncResponsePolicy::AllowHintlessBlockSync
            ) && hintless_block_sync
        );
        if preflight.downgrade_hintless {
            if let BlockMessage::BlockSyncUpdate(update) = &msg {
                let header = update.block.header();
                debug!(
                    height = header.height().get(),
                    view = header.view_change_index(),
                    block = %update.block.hash(),
                    peer = %peer,
                    "enforcing hintless BlockSyncUpdate send gate: downgrading to BlockCreated"
                );
                msg = BlockMessage::BlockCreated(super::message::BlockCreated {
                    block: update.block.clone(),
                    frontier: None,
                });
            }
            hintless_block_sync = false;
        }
        debug_assert_eq!(
            FetchPendingResponsePayloadKind::from_message(&msg),
            preflight.message_after_hintless_gate
        );
        let payload_bypasses_queue =
            self.fetch_response_should_bypass_queue(&msg, allow_highest_qc_bypass);
        let bypass_queue = preflight.bypass_queue;
        debug_assert_eq!(
            bypass_queue,
            force_bypass_queue
                || matches!(priority, FetchPendingBlockPriority::Consensus)
                || payload_bypasses_queue
                || (allow_hintless_block_sync_bypass && hintless_block_sync)
        );
        if let BlockMessage::BlockSyncUpdate(update) = &mut msg {
            debug_assert!(preflight.apply_cached_qc);
            debug_assert!(preflight.trim_update);
            let block_hash = update.block.hash();
            let height = update.block.header().height().get();
            let view = update.block.header().view_change_index();
            let expected_epoch = self.epoch_for_height(height);
            Self::apply_cached_qcs_to_block_sync_update(
                update,
                &self.qc_cache,
                &self.vote_log,
                block_hash,
                height,
                view,
                expected_epoch,
                self.state.as_ref(),
                self.config.consensus_mode,
            );
            direct_commit_qc = self
                .direct_commit_qc_from_block_sync_update(block_hash, height, view, update)
                .or_else(|| self.direct_commit_qc_for_block(&update.block))
                .map(|qc| (qc, block_hash, height, view));
            if !self.trim_block_sync_update_for_frame_cap(update) {
                let fallback = BlockMessage::BlockCreated(super::message::BlockCreated {
                    block: update.block.clone(),
                    frontier: None,
                });
                let fallback_len =
                    super::consensus_block_wire_len(self.common_config.peer.id(), &fallback);
                let frame_decision = fetch_pending_response_frame_decision(
                    preflight.message_after_hintless_gate,
                    false,
                    fallback_len <= self.consensus_payload_frame_cap,
                    direct_commit_qc.is_some(),
                );
                if matches!(
                    frame_decision.final_payload,
                    FetchPendingResponseFinalPayload::None
                ) {
                    warn!(
                        height,
                        view,
                        block = %block_hash,
                        cap = self.consensus_payload_frame_cap,
                        fallback_len,
                        "dropping oversized block sync response; BlockCreated still exceeds cap"
                    );
                    if let Some((qc, block_hash, height, view)) = direct_commit_qc.take() {
                        debug_assert!(frame_decision.direct_qc_companion);
                        debug_assert!(!frame_decision.companion_before_payload);
                        self.dispatch_direct_commit_qc_companion(
                            peer,
                            qc,
                            block_hash,
                            height,
                            view,
                            "fetch_pending_block_response_oversized",
                        );
                    }
                    return;
                }
                debug_assert!(matches!(
                    frame_decision.final_payload,
                    FetchPendingResponseFinalPayload::FallbackBlockCreated
                ));
                warn!(
                    height,
                    view,
                    block = %block_hash,
                    cap = self.consensus_payload_frame_cap,
                    fallback_len,
                    "block sync response exceeds frame cap; sending BlockCreated instead"
                );
                if let Some((qc, block_hash, height, view)) = direct_commit_qc.take() {
                    debug_assert!(frame_decision.companion_before_payload);
                    self.dispatch_direct_commit_qc_companion(
                        peer.clone(),
                        qc,
                        block_hash,
                        height,
                        view,
                        "fetch_pending_block_response_fallback",
                    );
                }
                debug_assert!(frame_decision.payload_sent);
                self.dispatch_fetch_pending_block_response(peer, fallback, bypass_queue);
                return;
            }
        } else {
            debug_assert!(!preflight.apply_cached_qc);
            debug_assert!(!preflight.trim_update);
        }
        if let BlockMessage::BlockCreated(created) = &msg {
            let header = created.block.header();
            let block_hash = created.block.hash();
            let height = header.height().get();
            let view = header.view_change_index();
            direct_commit_qc = self
                .direct_commit_qc_for_block(&created.block)
                .map(|qc| (qc, block_hash, height, view));
        }
        let final_payload_kind = FetchPendingResponsePayloadKind::from_message(&msg);
        let frame_decision = fetch_pending_response_frame_decision(
            final_payload_kind,
            true,
            false,
            direct_commit_qc.is_some(),
        );
        debug_assert!(matches!(
            frame_decision.final_payload,
            FetchPendingResponseFinalPayload::Original(kind) if kind == final_payload_kind
        ));
        if let Some((qc, block_hash, height, view)) = direct_commit_qc.take() {
            debug_assert!(frame_decision.direct_qc_companion);
            debug_assert!(frame_decision.companion_before_payload);
            self.dispatch_direct_commit_qc_companion(
                peer.clone(),
                qc,
                block_hash,
                height,
                view,
                "fetch_pending_block_response",
            );
        } else {
            debug_assert!(!frame_decision.direct_qc_companion);
        }
        debug_assert!(frame_decision.payload_sent);
        self.dispatch_fetch_pending_block_response(peer, msg, bypass_queue);
    }

    pub(super) fn build_fetch_pending_block_payload(&self, block: &SignedBlock) -> BlockMessage {
        let block_hash = block.hash();
        let block_height = block.header().height().get();
        let block_view = block.header().view_change_index();
        let update = super::block_sync_update_with_roster(
            block,
            self.state.as_ref(),
            self.kura.as_ref(),
            self.config.consensus_mode,
            self.common_config.trusted_peers.value(),
            self.common_config.peer.id(),
            &self.roster_validation_cache,
        );
        let mut update = update;
        let expected_epoch = self.epoch_for_height(block_height);
        Self::apply_cached_qcs_to_block_sync_update(
            &mut update,
            &self.qc_cache,
            &self.vote_log,
            block_hash,
            block_height,
            block_view,
            expected_epoch,
            self.state.as_ref(),
            self.config.consensus_mode,
        );
        let (consensus_mode, _, _) = self.consensus_context_for_height(block_height);
        let has_roster = super::block_sync_update_has_roster(&update, consensus_mode);
        let has_cached_qc = update.commit_qc.is_some() || !update.commit_votes.is_empty();
        let tracked_missing_payload_recovery = self
            .pending
            .missing_block_requests
            .contains_key(&block_hash)
            || self.deferred_missing_payload_qcs.values().any(|entry| {
                entry.qc.subject_block_hash == block_hash
                    && entry.qc.height == block_height
                    && entry.qc.view == block_view
            })
            || self.missing_commit_qc_repair_active_for_round(
                block_hash,
                block_height,
                block_view,
                self.committed_height_snapshot(),
                Instant::now(),
            );
        let send_block_sync = match consensus_mode {
            ConsensusMode::Permissioned => has_roster || has_cached_qc,
            // Missing-block recovery in NPoS must stay on the BlockSyncUpdate path even when
            // roster sidecars/hints are unavailable, otherwise responders fall back to
            // BlockCreated and receivers can livelock on lock-conflicting hintless payloads.
            ConsensusMode::Npos => has_roster || has_cached_qc || tracked_missing_payload_recovery,
        };
        if !send_block_sync {
            BlockMessage::BlockCreated(self.frontier_block_created_for_wire(block))
        } else {
            BlockMessage::BlockSyncUpdate(update)
        }
    }

    pub(super) fn block_sync_qc_extends_locked_chain(
        &self,
        qc: &crate::sumeragi::consensus::Qc,
    ) -> bool {
        let Some(lock) = self.locked_qc else {
            return true;
        };
        if !self.block_known_for_lock(lock.subject_block_hash) {
            return qc.view > lock.view;
        }
        let candidate = Self::qc_to_header_ref(qc);
        qc_satisfies_locked_with_lookup(lock, candidate, |hash, lookup_height| {
            self.parent_hash_for(hash, lookup_height)
        })
    }

    pub(super) fn block_sync_qc_same_height_conflict(
        lock: crate::sumeragi::consensus::QcHeaderRef,
        qc: &crate::sumeragi::consensus::Qc,
    ) -> bool {
        qc.view <= lock.view
            && qc.height == lock.height
            && qc.subject_block_hash != lock.subject_block_hash
    }

    pub(super) fn block_sync_qc_same_height_recoverable(
        lock: crate::sumeragi::consensus::QcHeaderRef,
        qc: &crate::sumeragi::consensus::Qc,
        allow_nonextending_qc: bool,
    ) -> bool {
        allow_nonextending_qc
            && matches!(qc.phase, crate::sumeragi::consensus::Phase::Commit)
            && qc.height == lock.height
            && qc.subject_block_hash != lock.subject_block_hash
    }

    pub(super) fn defer_block_sync_qc_while_locked_payload_missing(
        &mut self,
        qc: &crate::sumeragi::consensus::Qc,
        context: &'static str,
    ) -> bool {
        let Some(lock) = self.locked_qc else {
            return false;
        };
        if self.block_known_locally(lock.subject_block_hash) {
            return false;
        }
        if qc.view > lock.view {
            return false;
        }
        if qc.height == lock.height && qc.subject_block_hash == lock.subject_block_hash {
            return false;
        }
        self.drop_missing_lock_if_unknown(qc);
        debug!(
            context,
            height = qc.height,
            view = qc.view,
            incoming_hash = %qc.subject_block_hash,
            locked_height = lock.height,
            locked_view = lock.view,
            locked_hash = %lock.subject_block_hash,
            "deferring block sync QC while locked payload remains unavailable"
        );
        self.quarantine_block_sync_qc_candidate(
            qc.clone(),
            "locked_payload_missing",
            QuarantinedQcTarget::LockedPayload,
        );
        self.record_consensus_message_handling(
            super::status::ConsensusMessageKind::Qc,
            super::status::ConsensusMessageOutcome::Dropped,
            super::status::ConsensusMessageReason::LockedQc,
        );
        true
    }

    pub(super) fn block_sync_qc_is_stale_against_lock(
        &self,
        qc: &crate::sumeragi::consensus::Qc,
    ) -> bool {
        self.locked_qc
            .is_some_and(|lock| qc.view <= lock.view && qc.height < lock.height)
    }

    fn log_block_sync_locked_qc_conflict(
        &mut self,
        qc: &crate::sumeragi::consensus::Qc,
        lock: crate::sumeragi::consensus::QcHeaderRef,
        context: &'static str,
    ) {
        let warn_cooldown = self
            .rebroadcast_cooldown()
            .max(super::BLOCK_SYNC_WARN_COOLDOWN_FLOOR);
        let warn_now = Instant::now();
        if let Some(suppressed_since_last) = self.block_sync_warning_log.allow(
            super::BlockSyncWarningKind::LockedQcConflict,
            qc.subject_block_hash,
            qc.height,
            qc.view,
            warn_now,
            warn_cooldown,
            super::BLOCK_SYNC_WARN_BURST_WINDOW,
            super::BLOCK_SYNC_WARN_BURST_CAP,
        ) {
            self.hotspot_log_summary.record_block_sync_warn();
            if suppressed_since_last > 0 {
                self.hotspot_log_summary
                    .record_block_sync_suppressed(suppressed_since_last);
            }
            info!(
                context,
                height = qc.height,
                view = qc.view,
                phase = ?qc.phase,
                incoming_hash = %qc.subject_block_hash,
                locked_height = lock.height,
                locked_view = lock.view,
                locked_hash = %lock.subject_block_hash,
                suppressed_since_last,
                warn_cooldown_ms = warn_cooldown.as_millis(),
                "dropping block sync QC that conflicts with locked chain"
            );
        } else {
            self.hotspot_log_summary.record_block_sync_suppressed(1);
        }
        self.hotspot_log_summary.emit_if_due(warn_now);
    }

    fn take_pending_fetch_requesters(
        &mut self,
        block_hash: &HashOf<BlockHeader>,
    ) -> BTreeMap<PeerId, super::PendingFetchRequestMeta> {
        self.pending
            .pending_fetch_requests
            .remove(block_hash)
            .unwrap_or_default()
    }

    fn take_pending_block_body_requesters(
        &mut self,
        block_hash: &HashOf<BlockHeader>,
    ) -> BTreeSet<PeerId> {
        self.pending
            .pending_block_body_requests
            .remove(block_hash)
            .unwrap_or_default()
    }

    fn stash_pending_fetch_request(
        &mut self,
        block_hash: HashOf<BlockHeader>,
        peer: PeerId,
        priority: FetchPendingBlockPriority,
        requester_roster_proof_known: bool,
        commit_qc_only: bool,
    ) {
        let meta = super::PendingFetchRequestMeta {
            priority,
            requester_roster_proof_known,
            commit_qc_only,
        };
        let entry = self
            .pending
            .pending_fetch_requests
            .entry(block_hash)
            .or_default();
        entry
            .entry(peer)
            .and_modify(|stored| {
                stored.priority = stored.priority.max(meta.priority);
                stored.requester_roster_proof_known |= meta.requester_roster_proof_known;
                stored.commit_qc_only |= meta.commit_qc_only;
            })
            .or_insert(meta);
    }

    fn stash_pending_block_body_request(&mut self, block_hash: HashOf<BlockHeader>, peer: PeerId) {
        self.pending
            .pending_block_body_requests
            .entry(block_hash)
            .or_default()
            .insert(peer);
    }

    fn should_stash_pending_block_body_request(&self, height: u64) -> bool {
        let committed_height = self.committed_height_snapshot();
        let raw_margin = self.recovery_missing_request_stale_height_margin();
        block_body_request_stash_window_decision(committed_height, raw_margin, height).stash
    }

    fn remove_pending_block_body_requester(
        &mut self,
        block_hash: &HashOf<BlockHeader>,
        peer: &PeerId,
    ) {
        if let Some(requesters) = self.pending.pending_block_body_requests.get_mut(block_hash) {
            requesters.remove(peer);
            if requesters.is_empty() {
                self.pending.pending_block_body_requests.remove(block_hash);
            }
        }
    }

    fn send_fetch_pending_block_responses(
        &mut self,
        peers: BTreeMap<PeerId, super::PendingFetchRequestMeta>,
        block: &SignedBlock,
        force_bypass_queue: bool,
        allow_highest_qc_bypass: bool,
        allow_hintless_block_sync_bypass: bool,
    ) {
        if peers.is_empty() {
            return;
        }
        let block_hash = block.hash();
        let mut payload_peers = BTreeMap::new();
        for (peer, meta) in peers {
            if meta.commit_qc_only {
                let commit_qc_sent = self.dispatch_commit_qc_only_fetch_response(
                    peer.clone(),
                    block,
                    meta.priority,
                    meta.requester_roster_proof_known,
                );
                let decision = fetch_pending_responses_batch_commit_decision(true, commit_qc_sent);
                debug_assert!(decision.dispatch_commit_qc_only);
                if decision.restash {
                    debug_assert!(decision.restash_commit_qc_only);
                    self.stash_pending_fetch_request(
                        block_hash,
                        peer,
                        meta.priority,
                        meta.requester_roster_proof_known,
                        decision.restash_commit_qc_only,
                    );
                }
            } else {
                let decision = fetch_pending_responses_batch_commit_decision(false, false);
                debug_assert!(!decision.dispatch_commit_qc_only);
                debug_assert!(!decision.restash);
                payload_peers.insert(peer, meta);
            }
        }
        let peers = payload_peers;
        if !fetch_pending_responses_batch_should_build_payload(peers.len()) {
            return;
        }
        let msg = self.build_fetch_pending_block_payload(block);
        let hintless_block_sync = matches!(
            &msg,
            BlockMessage::BlockSyncUpdate(update)
                if update.commit_qc.is_none() && update.validator_checkpoint.is_none()
        );
        let payload_kind =
            FetchPendingResponsesBatchPayloadKind::from_message(&msg, hintless_block_sync);
        let consensus_body_targets = peers
            .iter()
            .filter_map(|(peer, meta)| {
                let decision = fetch_pending_responses_batch_payload_decision(
                    true,
                    payload_kind,
                    force_bypass_queue,
                    allow_hintless_block_sync_bypass,
                    meta.requester_roster_proof_known,
                    meta.priority,
                    false,
                );
                decision.exact_body_companion.then(|| peer.clone())
            })
            .collect::<Vec<_>>();
        if !consensus_body_targets.is_empty() {
            let header = block.header();
            info!(
                height = header.height().get(),
                view = header.view_change_index(),
                block = %block.hash(),
                targets = ?consensus_body_targets,
                "sending exact BlockBodyResponse companion for consensus missing-block fetch"
            );
            let response = self.block_body_response_for_wire(block);
            for peer in consensus_body_targets {
                self.dispatch_block_body_response_with_plain_fallback(
                    peer,
                    block,
                    response.clone(),
                );
            }
        }

        if hintless_block_sync {
            let created = BlockMessage::BlockCreated(self.frontier_block_created_for_wire(block));
            let header = block.header();
            for (peer, meta) in peers {
                let decision = fetch_pending_responses_batch_payload_decision(
                    true,
                    payload_kind,
                    force_bypass_queue,
                    allow_hintless_block_sync_bypass,
                    meta.requester_roster_proof_known,
                    meta.priority,
                    false,
                );
                debug_assert_eq!(decision.payload_peer, decision.payload_sent);
                debug_assert!(decision.created_companion_before_payload);
                let peer_msg = match decision.payload_message {
                    FetchPendingResponsesBatchPayloadMessage::BlockSyncUpdate => msg.clone(),
                    FetchPendingResponsesBatchPayloadMessage::BlockCreated => {
                        debug!(
                            height = header.height().get(),
                            view = header.view_change_index(),
                            block = %block.hash(),
                            peer = %peer,
                            requester_priority = ?meta.priority,
                            requester_roster_proof_known = meta.requester_roster_proof_known,
                            "downgrading hintless BlockSyncUpdate to BlockCreated: requester roster proof not confirmed"
                        );
                        created.clone()
                    }
                    FetchPendingResponsesBatchPayloadMessage::Other
                    | FetchPendingResponsesBatchPayloadMessage::None => {
                        debug_assert!(false, "hintless payload peer must send update or created");
                        continue;
                    }
                };
                self.send_fetch_pending_block_response(
                    peer.clone(),
                    peer_msg,
                    meta.priority,
                    decision.payload_force_bypass_arg,
                    allow_highest_qc_bypass,
                    decision.payload_allow_hintless_arg,
                    decision.payload_roster_proof_arg,
                );
            }
            return;
        }

        let bypass_rosterless_created =
            allow_hintless_block_sync_bypass && matches!(msg, BlockMessage::BlockCreated(_));
        if matches!(msg, BlockMessage::BlockSyncUpdate(_)) {
            let created = BlockMessage::BlockCreated(self.frontier_block_created_for_wire(block));
            let created_len =
                super::consensus_block_wire_len(self.common_config.peer.id(), &created);
            // For roster-hinted updates, include a companion BlockCreated copy so peers can
            // recover payload bytes even if they defer BlockSyncUpdate processing.
            let send_created_copy = true;
            let created_companion_fits =
                send_created_copy && created_len <= self.consensus_payload_frame_cap;
            if send_created_copy && created_len <= self.consensus_payload_frame_cap {
                for (peer, meta) in peers.iter() {
                    let decision = fetch_pending_responses_batch_payload_decision(
                        true,
                        payload_kind,
                        force_bypass_queue,
                        allow_hintless_block_sync_bypass,
                        meta.requester_roster_proof_known,
                        meta.priority,
                        created_companion_fits,
                    );
                    debug_assert!(decision.created_companion);
                    debug_assert!(decision.created_companion_before_payload);
                    self.send_fetch_pending_block_response(
                        peer.clone(),
                        created.clone(),
                        meta.priority,
                        force_bypass_queue || bypass_rosterless_created,
                        allow_highest_qc_bypass,
                        allow_hintless_block_sync_bypass,
                        meta.requester_roster_proof_known,
                    );
                }
            } else if send_created_copy {
                let header = block.header();
                warn!(
                    height = header.height().get(),
                    view = header.view_change_index(),
                    block = %block.hash(),
                    cap = self.consensus_payload_frame_cap,
                    created_len,
                    "skipping BlockCreated fetch response; payload exceeds frame cap"
                );
            }
        }
        for (peer, meta) in peers {
            let decision = fetch_pending_responses_batch_payload_decision(
                true,
                payload_kind,
                force_bypass_queue,
                allow_hintless_block_sync_bypass,
                meta.requester_roster_proof_known,
                meta.priority,
                false,
            );
            debug_assert_eq!(decision.payload_peer, decision.payload_sent);
            debug_assert!(decision.created_companion_before_payload);
            debug_assert_eq!(
                decision.payload_consensus_priority_arg,
                matches!(meta.priority, FetchPendingBlockPriority::Consensus)
            );
            self.send_fetch_pending_block_response(
                peer.clone(),
                msg.clone(),
                meta.priority,
                decision.payload_force_bypass_arg,
                allow_highest_qc_bypass,
                decision.payload_allow_hintless_arg,
                decision.payload_roster_proof_arg,
            );
            debug_assert_eq!(
                decision.payload_force_bypass_arg,
                force_bypass_queue || bypass_rosterless_created
            );
        }
    }

    pub(super) fn flush_pending_fetch_requests(&mut self, block: &SignedBlock) {
        let block_hash = block.hash();
        let requesters = self.take_pending_fetch_requesters(&block_hash);
        self.send_fetch_pending_block_responses(
            requesters, block, /*force_bypass_queue*/ false,
            /*allow_highest_qc_bypass*/ false,
            /*allow_hintless_block_sync_bypass*/ false,
        );
    }

    pub(super) fn flush_pending_fetch_requests_if_ready(&mut self, block: &SignedBlock) -> bool {
        let block_hash = block.hash();
        let pending_key_present = self
            .pending
            .pending_fetch_requests
            .contains_key(&block_hash);
        let preflight = pending_response_flush_decision(
            PendingResponseFlushKind::Fetch,
            pending_key_present,
            false,
        );
        if !preflight.build_payload {
            debug_assert!(!preflight.returns_ready);
            debug_assert!(!preflight.fetch_removed);
            debug_assert!(!preflight.fetch_batch_called);
            return false;
        }
        let msg = self.build_fetch_pending_block_payload(block);
        let decision = pending_response_flush_decision(
            PendingResponseFlushKind::Fetch,
            pending_key_present,
            self.should_defer_canonical_committed_fetch_response(block, &msg),
        );
        if !decision.returns_ready {
            debug_assert!(decision.build_payload);
            debug_assert!(!decision.fetch_removed);
            debug_assert!(!decision.fetch_batch_called);
            return false;
        }
        debug_assert!(decision.fetch_removed);
        debug_assert!(decision.fetch_batch_called);
        debug_assert!(!decision.fetch_batch_force_arg);
        debug_assert!(!decision.fetch_batch_allow_highest_arg);
        debug_assert!(!decision.fetch_batch_allow_hintless_arg);
        self.flush_pending_fetch_requests(block);
        true
    }

    pub(super) fn flush_pending_block_body_requests_if_ready(
        &mut self,
        block: &SignedBlock,
    ) -> bool {
        let block_hash = block.hash();
        let pending_key_present = self
            .pending
            .pending_block_body_requests
            .contains_key(&block_hash);
        let preflight = pending_response_flush_decision(
            PendingResponseFlushKind::Body,
            pending_key_present,
            false,
        );
        if !preflight.build_payload {
            debug_assert!(!preflight.returns_ready);
            debug_assert!(!preflight.body_removed);
            debug_assert!(!preflight.body_response_constructed);
            return false;
        }
        let height = block.header().height().get();
        let view = block.header().view_change_index();
        let payload = self.build_fetch_pending_block_payload(block);
        let decision = pending_response_flush_decision(
            PendingResponseFlushKind::Body,
            pending_key_present,
            self.should_defer_canonical_committed_fetch_response(block, &payload),
        );
        if !decision.returns_ready {
            debug_assert!(decision.build_payload);
            debug_assert!(!decision.body_removed);
            debug_assert!(!decision.body_response_constructed);
            return false;
        }
        debug_assert!(decision.body_removed);
        debug_assert!(decision.body_response_constructed);
        debug_assert!(decision.body_response_hash_bound);
        debug_assert!(decision.body_response_height_bound);
        debug_assert!(decision.body_response_view_bound);
        debug_assert!(decision.body_response_payload_bound);
        #[cfg(debug_assertions)]
        let payload_for_debug = payload.clone();
        let Some(response) =
            Self::block_body_response_from_payload(block_hash, height, view, payload)
        else {
            return false;
        };
        debug_assert_eq!(response.block_hash, block_hash);
        debug_assert_eq!(response.height, height);
        debug_assert_eq!(response.view, view);
        #[cfg(debug_assertions)]
        debug_assert!(block_body_response_body_matches_payload(
            &response.body,
            &payload_for_debug
        ));
        let requesters = self.take_pending_block_body_requesters(&block_hash);
        for peer in requesters {
            debug_assert!(pending_response_flush_targets_requester(decision, true));
            debug_assert!(decision.body_dispatches_use_plain_fallback);
            self.dispatch_block_body_response_with_plain_fallback(peer, block, response.clone());
        }
        true
    }

    pub(super) fn should_drop_future_block_sync_update(
        &self,
        block_hash: &HashOf<BlockHeader>,
        parent_hash: Option<HashOf<BlockHeader>>,
        height: u64,
        view: u64,
        requested_missing_block: bool,
        has_commit_evidence: bool,
    ) -> bool {
        let known_block = self.block_known_locally(*block_hash);
        let local_height = u64::try_from(self.state.committed_height()).unwrap_or(u64::MAX);
        let active_frontier_range_pull = self
            .active_frontier_range_pull_accepts_future_block_sync_update(
                height,
                local_height,
                has_commit_evidence,
            );
        if active_frontier_range_pull {
            return false;
        }
        let requested_margin = block_sync_future_window_requested_margin(
            self.recovery_missing_request_stale_height_margin(),
        );
        let far_ahead_by_committed =
            block_sync_future_window_far_ahead(height, local_height, requested_margin);
        let lower_unresolved_missing = self
            .lowest_unresolved_missing_block_height(local_height)
            .filter(|missing_height| {
                block_sync_future_window_lower_unresolved(Some(*missing_height), height)
            });
        let parent_available =
            parent_hash.is_some_and(|hash| self.block_payload_available_locally(hash));
        if let Some(drop) = block_sync_future_window_pre_generic_drop(
            known_block,
            requested_missing_block,
            far_ahead_by_committed,
            lower_unresolved_missing.is_some(),
            parent_available,
        ) {
            return drop;
        }
        block_sync_future_window_drop_decision(
            known_block,
            requested_missing_block,
            far_ahead_by_committed,
            lower_unresolved_missing.is_some(),
            parent_available,
            self.should_drop_future_consensus_message(height, view, "BlockSyncUpdate"),
        )
    }

    fn active_frontier_range_pull_accepts_future_block_sync_update(
        &self,
        height: u64,
        local_height: u64,
        has_commit_evidence: bool,
    ) -> bool {
        if !has_commit_evidence || height <= local_height.saturating_add(1) {
            return false;
        }
        let frontier_height = local_height.saturating_add(1);
        let now = Instant::now();
        self.range_pull_escalation_cooldowns.iter().any(
            |((_, pull_local_height, canonical_height, reason), expires)| {
                *pull_local_height == local_height
                    && *canonical_height == frontier_height
                    && now < *expires
                    && (Self::reason_is_canonical_frontier_reanchor(reason)
                        || matches!(
                            *reason,
                            "rbc_far_future_missing_block" | "block_sync_future_gap"
                        ))
            },
        )
    }

    fn validation_inflight_blocks_block_sync_update(
        &self,
        block_height: u64,
        parent_hash: Option<HashOf<BlockHeader>>,
    ) -> bool {
        let validation_inflight_empty = self.subsystems.validation.inflight.is_empty();
        let contiguous_frontier = block_height
            == self.committed_height_snapshot().saturating_add(1)
            && parent_hash == self.state.latest_block_hash_fast();
        let blocking_pending_conflict = !validation_inflight_empty
            && contiguous_frontier
            && self.subsystems.validation.inflight.keys().any(|hash| {
                let pending_height = self
                    .pending
                    .pending_blocks
                    .get(hash)
                    .map(|pending| pending.height);
                deferred_block_sync_validation_pending_conflicts(pending_height, block_height)
            });
        deferred_block_sync_validation_inflight_blocks(
            validation_inflight_empty,
            contiguous_frontier,
            blocking_pending_conflict,
        )
    }

    fn block_sync_update_deferral_reason(
        &self,
        block_height: u64,
        parent_hash: Option<HashOf<BlockHeader>>,
        allow_certified_exact_frontier_bypass: bool,
    ) -> Option<&'static str> {
        deferred_block_sync_update_deferral_reason(
            self.subsystems.commit.inflight.is_some(),
            self.validation_inflight_blocks_block_sync_update(block_height, parent_hash),
            self.pending.pending_processing.get().is_some(),
            allow_certified_exact_frontier_bypass,
        )
    }

    fn merge_deferred_block_sync_update(
        existing: &mut super::DeferredBlockSyncUpdate,
        mut incoming: super::DeferredBlockSyncUpdate,
    ) {
        let decision = deferred_block_sync_merge_decision(
            existing.update.commit_qc.is_some(),
            incoming.update.commit_qc.is_some(),
            existing.update.validator_checkpoint.is_some(),
            incoming.update.validator_checkpoint.is_some(),
            existing.update.stake_snapshot.is_some(),
            incoming.update.stake_snapshot.is_some(),
            existing.sender.is_some(),
            incoming.sender.is_some(),
        );
        if decision.take_incoming_commit_qc {
            existing.update.commit_qc = incoming.update.commit_qc.take();
        }
        if decision.take_incoming_validator_checkpoint {
            existing.update.validator_checkpoint = incoming.update.validator_checkpoint.take();
        }
        if decision.take_incoming_stake_snapshot {
            existing.update.stake_snapshot = incoming.update.stake_snapshot.take();
        }
        if decision.replace_sender {
            existing.sender = incoming.sender;
        }
        debug_assert_eq!(
            existing.update.commit_qc.is_some(),
            decision.final_commit_qc_present
        );
        debug_assert_eq!(
            existing.update.validator_checkpoint.is_some(),
            decision.final_validator_checkpoint_present
        );
        debug_assert_eq!(
            existing.update.stake_snapshot.is_some(),
            decision.final_stake_snapshot_present
        );
        debug_assert_eq!(existing.sender.is_some(), decision.final_sender_present);
    }

    fn deferred_block_sync_has_commit_evidence(entry: &super::DeferredBlockSyncUpdate) -> bool {
        deferred_block_sync_commit_evidence_present(
            entry.update.commit_qc.is_some(),
            entry.update.validator_checkpoint.is_some(),
            entry.update.stake_snapshot.is_some(),
        )
    }

    fn enforce_deferred_block_sync_cap(&mut self) {
        let cap = self.recovery_pending_block_sync_cap();
        if !deferred_block_sync_cap_should_evict(cap, self.deferred_block_sync_updates.len()) {
            return;
        }
        let expected_evictions =
            deferred_block_sync_cap_eviction_count(cap, self.deferred_block_sync_updates.len());
        let mut evictions = 0u64;
        while deferred_block_sync_cap_should_evict(cap, self.deferred_block_sync_updates.len()) {
            let candidate = self
                .deferred_block_sync_updates
                .iter()
                .min_by_key(|(key, entry)| {
                    let &(height, view, hash) = *key;
                    deferred_block_sync_eviction_rank(
                        Self::deferred_block_sync_has_commit_evidence(entry),
                        height,
                        view,
                        hash,
                    )
                })
                .map(|(key, _)| *key);
            let Some((height, view, hash)) = candidate else {
                break;
            };
            if self
                .deferred_block_sync_updates
                .remove(&(height, view, hash))
                .is_some()
            {
                evictions = evictions.saturating_add(1);
                debug!(
                    height,
                    view,
                    block = %hash,
                    deferred = self.deferred_block_sync_updates.len(),
                    cap,
                    "evicting deferred block sync update due to bounded queue cap"
                );
            }
        }
        debug_assert_eq!(usize::try_from(evictions).ok(), Some(expected_evictions));
        if evictions > 0 {
            super::status::inc_pending_queue_evictions_total(evictions);
        }
    }

    pub(super) fn cache_deferred_block_sync_update(
        &mut self,
        mut update: super::message::BlockSyncUpdate,
        sender: Option<PeerId>,
        block_hash: HashOf<BlockHeader>,
        block_height: u64,
        block_view: u64,
        reason: &'static str,
    ) {
        let key = deferred_block_sync_cache_key(block_height, block_view, block_hash);
        let decision = deferred_block_sync_cache_decision(
            self.deferred_block_sync_updates.len(),
            self.deferred_block_sync_updates.contains_key(&key),
            self.recovery_pending_block_sync_cap(),
        );
        debug_assert!(decision.cache_called);
        update.commit_votes.clear();
        debug_assert!(decision.commit_votes_cleared);
        debug_assert!(update.commit_votes.is_empty());
        let entry = super::DeferredBlockSyncUpdate { update, sender };
        if let Some(existing) = self.deferred_block_sync_updates.get_mut(&key) {
            debug_assert!(decision.key_matched);
            debug_assert!(!decision.inserted);
            Self::merge_deferred_block_sync_update(existing, entry);
        } else {
            debug_assert!(!decision.key_matched);
            debug_assert!(decision.inserted);
            self.deferred_block_sync_updates.insert(key, entry);
        }
        debug_assert_eq!(
            self.deferred_block_sync_updates.len(),
            decision.len_before_cap
        );
        self.enforce_deferred_block_sync_cap();
        debug_assert!(decision.cap_called);
        debug_assert_eq!(self.deferred_block_sync_updates.len(), decision.final_len);
        debug!(
            height = block_height,
            view = block_view,
            block = %block_hash,
            deferred = self.deferred_block_sync_updates.len(),
            reason,
            "cached deferred block sync payload for later replay"
        );
    }

    pub(super) fn defer_block_sync_update(
        &mut self,
        update: super::message::BlockSyncUpdate,
        sender: Option<PeerId>,
        block_hash: HashOf<BlockHeader>,
        block_height: u64,
        block_view: u64,
        reason: &'static str,
    ) {
        let decision = deferred_block_sync_defer_record_decision();
        debug_assert!(decision.cache_called);
        self.cache_deferred_block_sync_update(
            update,
            sender,
            block_hash,
            block_height,
            block_view,
            reason,
        );
        debug_assert!(decision.record_after_cache);
        debug_assert!(decision.record_called);
        debug_assert_eq!(
            decision.recorded_kind,
            super::status::ConsensusMessageKind::BlockSyncUpdate
        );
        debug_assert_eq!(
            decision.recorded_outcome,
            super::status::ConsensusMessageOutcome::Deferred
        );
        debug_assert_eq!(
            decision.recorded_reason,
            super::status::ConsensusMessageReason::CommitPipelineActive
        );
        self.record_consensus_message_handling(
            super::status::ConsensusMessageKind::BlockSyncUpdate,
            super::status::ConsensusMessageOutcome::Deferred,
            super::status::ConsensusMessageReason::CommitPipelineActive,
        );
        debug!(
            height = block_height,
            view = block_view,
            block = %block_hash,
            deferred = self.deferred_block_sync_updates.len(),
            reason,
            commit_inflight = self.subsystems.commit.inflight.is_some(),
            validation_inflight = self.subsystems.validation.inflight.len(),
            "deferring block sync update while commit/validation work is in flight"
        );
    }

    /// Replay deferred block-sync updates once commit/validation work is idle.
    pub(super) fn try_replay_deferred_block_sync_updates(&mut self) -> bool {
        let initial_len = self.deferred_block_sync_updates.len();
        if self.deferred_block_sync_updates.is_empty() {
            let decision =
                deferred_block_sync_replay_decision(initial_len, false, false, false, false);
            debug_assert!(!decision.returns_progress);
            debug_assert!(!decision.select_key);
            debug_assert_eq!(self.deferred_block_sync_updates.len(), decision.final_len);
            return false;
        }
        let commit_inflight = self.subsystems.commit.inflight.is_some();
        let validation_inflight = !self.subsystems.validation.inflight.is_empty();
        if commit_inflight || validation_inflight {
            let decision = deferred_block_sync_replay_decision(
                initial_len,
                commit_inflight,
                validation_inflight,
                false,
                false,
            );
            debug_assert!(!decision.returns_progress);
            debug_assert!(!decision.remove_before_handle);
            debug_assert!(!decision.handle_called);
            debug_assert_eq!(self.deferred_block_sync_updates.len(), decision.final_len);
            return false;
        }
        let ready_decision =
            deferred_block_sync_replay_decision(initial_len, false, false, true, false);
        debug_assert!(ready_decision.select_key);
        let Some(key) = self.deferred_block_sync_updates.keys().next().cloned() else {
            let decision =
                deferred_block_sync_replay_decision(initial_len, false, false, false, false);
            debug_assert!(!decision.returns_progress);
            debug_assert!(!decision.handle_called);
            return false;
        };
        let (height, view, block_hash) = key;
        let Some(entry) = self.deferred_block_sync_updates.remove(&key) else {
            let decision =
                deferred_block_sync_replay_decision(initial_len, false, false, false, false);
            debug_assert!(!decision.returns_progress);
            debug_assert!(!decision.handle_called);
            debug_assert_eq!(self.deferred_block_sync_updates.len(), decision.final_len);
            return false;
        };
        debug_assert!(ready_decision.remove_before_handle);
        debug_assert_eq!(
            self.deferred_block_sync_updates.len(),
            ready_decision.final_len
        );
        debug_assert!(ready_decision.later_entries_preserved);
        debug!(
            height,
            view,
            block = %block_hash,
            deferred = self.deferred_block_sync_updates.len(),
            "replaying deferred block sync update"
        );
        let result = self.handle_block_sync_update(entry.update, entry.sender);
        let handler_errors = result.is_err();
        let decision =
            deferred_block_sync_replay_decision(initial_len, false, false, true, handler_errors);
        debug_assert!(decision.returns_progress);
        debug_assert!(decision.handle_called);
        debug_assert!(decision.update_forwarded);
        debug_assert!(decision.sender_forwarded);
        if let Err(err) = result {
            debug_assert!(decision.warn_on_error);
            warn!(
                ?err,
                height,
                view,
                block = %block_hash,
                "failed to replay deferred block sync update"
            );
        } else {
            debug_assert!(!decision.warn_on_error);
        }
        true
    }

    #[allow(clippy::too_many_lines, clippy::needless_pass_by_value)]
    pub(super) fn handle_block_sync_update(
        &mut self,
        update: super::message::BlockSyncUpdate,
        sender: Option<PeerId>,
    ) -> Result<()> {
        let dedup_key = super::block_sync_update_dedup_key(&update);
        self.release_block_payload_dedup(&dedup_key);
        if crate::sumeragi::status::local_peer_removed() {
            debug!(
                ?sender,
                "allowing BlockSyncUpdate while local peer removed from world to permit catch-up"
            );
        }
        let super::message::BlockSyncUpdate {
            block,
            commit_votes,
            commit_qc: incoming_qc,
            validator_checkpoint,
            stake_snapshot,
        } = update;
        let mut incoming_qc = incoming_qc;
        let mut validator_checkpoint = validator_checkpoint;
        let mut stake_snapshot = stake_snapshot;
        let block_hash = block.hash();
        let block_height = block.header().height().get();
        let block_view = block.header().view_change_index();
        self.maybe_cache_rehydrated_kura_body(&block);
        let parent_hash = block.header().prev_block_hash();
        let local_height = u64::try_from(self.state.committed_height()).unwrap_or(u64::MAX);
        let expected_epoch = self.epoch_for_height(block_height);
        let requested_missing_block_by_hash = self
            .pending
            .missing_block_requests
            .contains_key(&block_hash);
        let requested_missing_block_by_height = !requested_missing_block_by_hash
            && self.has_missing_block_request_for_height(block_height);
        let explicit_requested_missing_block =
            requested_missing_block_by_hash || requested_missing_block_by_height;
        let mut requested_missing_block = explicit_requested_missing_block;
        if requested_missing_block_by_height {
            debug!(
                height = block_height,
                view = block_view,
                block = %block_hash,
                "treating block sync update as missing-block recovery traffic due to same-height request"
            );
        }
        let block_known_locally = self.block_known_locally(block_hash);
        let has_commit_votes = !commit_votes.is_empty();
        let has_commit_evidence = block_sync_stale_view_has_commit_evidence(
            incoming_qc.is_some(),
            validator_checkpoint.is_some(),
            has_commit_votes,
        );
        let exact_contiguous_frontier = block_height == local_height.saturating_add(1)
            && parent_hash == self.state.latest_block_hash_fast();
        let implicit_recovery_consensus_mode = self.consensus_context_for_height(block_height).0;
        let vote_only_frontier_update =
            has_commit_votes && incoming_qc.is_none() && validator_checkpoint.is_none();
        let implicit_frontier_recovery_allowed =
            !(matches!(implicit_recovery_consensus_mode, ConsensusMode::Npos)
                && vote_only_frontier_update);
        if self.runtime_da_enabled()
            && exact_contiguous_frontier
            && !block_known_locally
            && !requested_missing_block
            && implicit_frontier_recovery_allowed
        {
            requested_missing_block = true;
            debug!(
                height = block_height,
                view = block_view,
                block = %block_hash,
                "treating exact contiguous frontier block sync update as recovery traffic"
            );
        }
        self.prune_frontier_slot_state();
        let certified_exact_contiguous_frontier =
            exact_contiguous_frontier && (incoming_qc.is_some() || validator_checkpoint.is_some());
        let sparse_exact_contiguous_frontier_repair = exact_contiguous_frontier
            && requested_missing_block
            && !block_known_locally
            && incoming_qc.is_none()
            && validator_checkpoint.is_none()
            && !has_commit_votes;
        let allow_exact_contiguous_frontier_bypass =
            certified_exact_contiguous_frontier || sparse_exact_contiguous_frontier_repair;
        let would_defer_exact_frontier = allow_exact_contiguous_frontier_bypass
            && self
                .block_sync_update_deferral_reason(block_height, parent_hash, false)
                .is_some();
        let entry_deferral_reason = self.block_sync_update_deferral_reason(
            block_height,
            parent_hash,
            allow_exact_contiguous_frontier_bypass,
        );
        if would_defer_exact_frontier && entry_deferral_reason.is_none() {
            info!(
                height = block_height,
                view = block_view,
                block = %block_hash,
                commit_inflight = self.subsystems.commit.inflight.is_some(),
                validation_inflight = self.subsystems.validation.inflight.len(),
                pending_processing = self.pending.pending_processing.get().is_some(),
                certified = certified_exact_contiguous_frontier,
                sparse_payload_repair = sparse_exact_contiguous_frontier_repair,
                "bypassing block sync deferral for exact-frontier recovery update"
            );
        }
        if sparse_exact_contiguous_frontier_repair {
            let now = Instant::now();
            let retry_window = self.rebroadcast_cooldown();
            let view_change_window = Some(self.quorum_timeout(self.runtime_da_enabled()));
            let _ = super::touch_missing_block_request(
                &mut self.pending.missing_block_requests,
                block_hash,
                block_height,
                block_view,
                crate::sumeragi::consensus::Phase::Commit,
                super::MissingBlockPriority::Consensus,
                now,
                retry_window,
                view_change_window,
            );
        }
        let exact_frontier_body_repair = self.frontier_slot.as_ref().is_some_and(|slot| {
            slot.block_hash == block_hash && slot.height == block_height && slot.view == block_view
        });
        let frontier_lane_owned = (local_height.saturating_add(1)..=local_height.saturating_add(2))
            .contains(&block_height);
        let frontier_lane_locked = self
            .frontier_slot
            .as_ref()
            .is_some_and(|slot| slot.height == local_height.saturating_add(1))
            || self
                .next_slot_prefetch
                .as_ref()
                .is_some_and(|slot| slot.height == local_height.saturating_add(2));
        let frontier_lane_deep_catchup = block_height > local_height.saturating_add(1)
            && frontier_lane_owned
            && has_commit_evidence
            && (requested_missing_block || block_known_locally);
        let block_known_sidecar_fast_path =
            block_known_locally && (incoming_qc.is_some() || validator_checkpoint.is_some());
        let frontier_lane_fast_path = frontier_lane_owned
            && !frontier_lane_deep_catchup
            && (block_known_sidecar_fast_path
                || (entry_deferral_reason.is_none()
                    && (block_known_locally
                        || (explicit_requested_missing_block
                            && (incoming_qc.is_some() || validator_checkpoint.is_some()))
                        || (exact_frontier_body_repair
                            && (incoming_qc.is_some() || validator_checkpoint.is_some())))));
        if frontier_lane_fast_path {
            let mut processed_votes = 0usize;
            let mut dropped_votes = 0usize;
            for vote in commit_votes {
                if vote.phase != crate::sumeragi::consensus::Phase::Commit
                    || vote.block_hash != block_hash
                    || vote.height != block_height
                    || vote.view != block_view
                    || vote.epoch != expected_epoch
                {
                    dropped_votes = dropped_votes.saturating_add(1);
                    continue;
                }
                self.handle_vote(vote);
                processed_votes = processed_votes.saturating_add(1);
            }
            if dropped_votes > 0 {
                debug!(
                    height = block_height,
                    view = block_view,
                    block = %block_hash,
                    dropped_votes,
                    "dropping mismatched commit votes from contiguous frontier block sync update"
                );
            }
            if block_known_locally {
                debug!(
                        height = block_height,
                        view = block_view,
                        block = %block_hash,
                    processed_votes,
                    has_commit_qc = incoming_qc.is_some(),
                    has_checkpoint = validator_checkpoint.is_some(),
                    "ignoring frontier-lane BlockSyncUpdate sidecars for a locally known block"
                );
                let mut sidecar_qc = incoming_qc.take().or_else(|| {
                    validator_checkpoint.as_ref().and_then(|checkpoint| {
                        self.commit_qc_from_validator_checkpoint(
                            block_hash,
                            block_height,
                            block_view,
                            checkpoint,
                            stake_snapshot.as_ref(),
                        )
                    })
                });
                let signed_quorum_repair_signers = sidecar_qc
                    .is_none()
                    .then(|| {
                        (exact_contiguous_frontier
                            && self.missing_commit_qc_repair_active_for_round(
                                block_hash,
                                block_height,
                                block_view,
                                local_height,
                                Instant::now(),
                            ))
                        .then(|| self.signed_commit_quorum_signer_count(&block))
                        .flatten()
                    })
                    .flatten();
                if sidecar_qc.is_some() {
                    let qc = sidecar_qc
                        .take()
                        .expect("sidecar QC presence checked above");
                    let world_view = self.state.world_view();
                    let consensus_mode = super::effective_consensus_mode_for_height_from_world(
                        &world_view,
                        block_height,
                        self.config.consensus_mode,
                    );
                    let mode_tag = match consensus_mode {
                        ConsensusMode::Permissioned => PERMISSIONED_TAG,
                        ConsensusMode::Npos => NPOS_TAG,
                    };
                    let prf_seed = Some(super::prf_seed_for_height_from_world(
                        &world_view,
                        &self.common_config.chain,
                        block_height,
                    ));
                    drop(world_view);
                    let checkpoint = validator_checkpoint.clone().unwrap_or_else(|| {
                        ValidatorSetCheckpoint::new_with_chain_order(
                            qc.height,
                            qc.view,
                            qc.subject_block_hash,
                            qc.chain_order_hash,
                            qc.rechain_seq,
                            qc.parent_state_root,
                            qc.post_state_root,
                            qc.validator_set.clone(),
                            qc.aggregate.signers_bitmap.clone(),
                            qc.aggregate.bls_aggregate_signature.clone(),
                            qc.validator_set_hash_version,
                            None,
                        )
                    });
                    self.state
                        .record_commit_roster(&qc, &checkpoint, stake_snapshot.clone());
                    let topology = super::network_topology::Topology::new(qc.validator_set.clone());
                    if let Some(work) = self.prepare_known_block_qc_work(
                        qc,
                        Arc::new(block),
                        topology,
                        stake_snapshot.clone(),
                        consensus_mode,
                        mode_tag,
                        prf_seed,
                        true,
                    ) {
                        let buffered_local_block =
                            self.pending
                                .pending_blocks
                                .get(&block_hash)
                                .is_some_and(|pending| !pending.is_retry_aborted())
                                || self.subsystems.commit.inflight.as_ref().is_some_and(
                                    |inflight| {
                                        inflight.block_hash == block_hash
                                            && !inflight.pending.aborted
                                    },
                                )
                                || self
                                    .pending
                                    .pending_processing
                                    .get()
                                    .is_some_and(|pending| pending == block_hash);
                        if buffered_local_block {
                            let _ = self.apply_known_block_qc_work(work);
                        } else {
                            self.enqueue_known_block_qc_work(work);
                        }
                    }
                    let _ = self.try_replay_deferred_qcs();
                    let _ = self.try_replay_deferred_missing_payload_qcs(Instant::now());
                    self.request_commit_pipeline_for_pending(
                        block_hash,
                        super::status::RoundEventCauseTrace::BlockSyncUpdated,
                        None,
                    );
                } else if let Some(block_signers) = signed_quorum_repair_signers {
                    if let Some(pending) = self.pending.pending_blocks.get_mut(&block_hash)
                        && pending.height == block_height
                        && pending.view == block_view
                        && pending.validation_status != ValidationStatus::Invalid
                    {
                        // A canonical committed peer may no longer have a portable commit QC,
                        // but its committed block still carries a verified commit-signature
                        // quorum. Treat that quorum as the local commit evidence needed to run
                        // the finalization path.
                        pending.note_commit_qc_observed(expected_epoch);
                    }
                    self.note_frontier_commit_qc_observed(
                        block_hash,
                        block_height,
                        block_view,
                        Instant::now(),
                    );
                    self.clear_missing_commit_qc_request(
                        &block_hash,
                        MissingBlockClearReason::Obsolete,
                    );
                    self.request_commit_pipeline_for_pending(
                        block_hash,
                        super::status::RoundEventCauseTrace::BlockSyncUpdated,
                        None,
                    );
                    info!(
                        height = block_height,
                        view = block_view,
                        block = %block_hash,
                        block_signers,
                        "accepted signed-quorum block sync fallback as commit evidence for known block"
                    );
                }
                return Ok(());
            }
            info!(
                height = block_height,
                view = block_view,
                block = %block_hash,
                processed_votes,
                has_commit_qc = incoming_qc.is_some(),
                has_checkpoint = validator_checkpoint.is_some(),
                "routing frontier-lane BlockSyncUpdate through BlockCreated owner"
            );
            let recovery_block = block.clone();
            let recovery_mode =
                if has_commit_votes || incoming_qc.is_some() || validator_checkpoint.is_some() {
                    BlockSyncRecoveryMode::CommitEvidenceRepair {
                        observed_commit_qc_epoch: incoming_qc
                            .as_ref()
                            .map(|qc| qc.epoch)
                            .or_else(|| validator_checkpoint.as_ref().map(|_| expected_epoch)),
                        allow_aborted_revival_without_local_commit_qc: true,
                    }
                } else {
                    BlockSyncRecoveryMode::PayloadOnly
                };
            let result = self.handle_block_created_from_block_sync(
                super::message::BlockCreated {
                    block,
                    frontier: None,
                },
                sender.clone(),
                incoming_qc.is_none() && validator_checkpoint.is_none(),
                recovery_mode,
            );
            let payload_materialized = result.is_ok()
                && self.materialize_frontier_block_sync_payload_for_qc_recovery(
                    &recovery_block,
                    incoming_qc.as_ref().map(|qc| qc.epoch),
                );
            let mut local_block_for_qc = result
                .is_ok()
                .then(|| self.local_signed_block_for_hash(block_hash))
                .flatten();
            if payload_materialized || local_block_for_qc.is_some() {
                let _ = self.try_replay_deferred_qcs();
                let _ = self.try_replay_deferred_missing_payload_qcs(Instant::now());
                self.request_commit_pipeline_for_pending(
                    block_hash,
                    super::status::RoundEventCauseTrace::BlockSyncUpdated,
                    None,
                );
                if local_block_for_qc.is_none() {
                    local_block_for_qc = self.local_signed_block_for_hash(block_hash);
                }
            }
            if result.is_ok()
                && let Some(local_block) = local_block_for_qc
                && let Some(qc) = incoming_qc.take().or_else(|| {
                    validator_checkpoint.as_ref().and_then(|checkpoint| {
                        self.commit_qc_from_validator_checkpoint(
                            block_hash,
                            block_height,
                            block_view,
                            checkpoint,
                            stake_snapshot.as_ref(),
                        )
                    })
                })
            {
                let world_view = self.state.world_view();
                let consensus_mode = super::effective_consensus_mode_for_height_from_world(
                    &world_view,
                    block_height,
                    self.config.consensus_mode,
                );
                let mode_tag = match consensus_mode {
                    ConsensusMode::Permissioned => PERMISSIONED_TAG,
                    ConsensusMode::Npos => NPOS_TAG,
                };
                let prf_seed = Some(super::prf_seed_for_height_from_world(
                    &world_view,
                    &self.common_config.chain,
                    block_height,
                ));
                drop(world_view);
                let checkpoint = validator_checkpoint.clone().unwrap_or_else(|| {
                    ValidatorSetCheckpoint::new_with_chain_order(
                        qc.height,
                        qc.view,
                        qc.subject_block_hash,
                        qc.chain_order_hash,
                        qc.rechain_seq,
                        qc.parent_state_root,
                        qc.post_state_root,
                        qc.validator_set.clone(),
                        qc.aggregate.signers_bitmap.clone(),
                        qc.aggregate.bls_aggregate_signature.clone(),
                        qc.validator_set_hash_version,
                        None,
                    )
                });
                self.state
                    .record_commit_roster(&qc, &checkpoint, stake_snapshot.clone());
                let topology = super::network_topology::Topology::new(qc.validator_set.clone());
                if let Some(work) = self.prepare_known_block_qc_work(
                    qc,
                    local_block,
                    topology,
                    stake_snapshot.clone(),
                    consensus_mode,
                    mode_tag,
                    prf_seed,
                    true,
                ) {
                    self.enqueue_known_block_qc_work(work);
                }
            }
            return result;
        }
        if frontier_lane_deep_catchup {
            info!(
                height = block_height,
                view = block_view,
                block = %block_hash,
                block_known_locally,
                has_commit_qc = incoming_qc.is_some(),
                has_checkpoint = validator_checkpoint.is_some(),
                has_commit_votes,
                "processing contiguous frontier BlockSyncUpdate as deep catch-up"
            );
        }
        let has_commit_votes = !commit_votes.is_empty();
        let has_commit_evidence = block_sync_stale_view_has_commit_evidence(
            incoming_qc.is_some(),
            validator_checkpoint.is_some(),
            has_commit_votes,
        );
        if !block_known_locally
            && !requested_missing_block
            && frontier_lane_locked
            && block_height > local_height.saturating_add(2)
            && !self.active_frontier_range_pull_accepts_future_block_sync_update(
                block_height,
                local_height,
                has_commit_evidence,
            )
        {
            debug!(
                height = block_height,
                view = block_view,
                block = %block_hash,
                local_height,
                frontier_slot_height = self.frontier_slot.as_ref().map(|slot| slot.height),
                next_slot_prefetch_height =
                    self.next_slot_prefetch.as_ref().map(|slot| slot.height),
                "dropping block sync update beyond the active frontier lanes"
            );
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::BlockSyncUpdate,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::FutureWindow,
            );
            return Ok(());
        }
        if self.should_drop_future_block_sync_update(
            &block_hash,
            parent_hash,
            block_height,
            block_view,
            requested_missing_block,
            has_commit_evidence,
        ) {
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::BlockSyncUpdate,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::FutureWindow,
            );
            if let Some(parent_hash) = parent_hash {
                let local_height = u64::try_from(self.state.committed_height()).unwrap_or(u64::MAX);
                let expected_height = local_height.saturating_add(1);
                let commit_topology = self.effective_commit_topology();
                let expected_usize = usize::try_from(expected_height).ok();
                let actual_usize = usize::try_from(block_height).ok();
                self.request_missing_parent(
                    block_hash,
                    block_height,
                    block_view,
                    parent_hash,
                    &commit_topology,
                    None,
                    expected_usize,
                    actual_usize,
                    "block_sync_future_window",
                );
                if block_height > expected_height.saturating_add(1) {
                    self.request_missing_parents_for_gap(
                        &commit_topology,
                        None,
                        "block_sync_future_gap",
                    );
                }
            }
            return Ok(());
        }
        let parent_missing_for_evidence_ahead = !block_known_locally
            && block_height > local_height.saturating_add(1)
            && parent_hash.is_some_and(|hash| !self.block_known_locally(hash))
            && has_commit_evidence;
        if parent_missing_for_evidence_ahead {
            let expected_height = local_height.saturating_add(1);
            let expected_usize = usize::try_from(expected_height).ok();
            let actual_usize = usize::try_from(block_height).ok();
            if let Some(parent_hash) = parent_hash {
                let commit_topology = self.effective_commit_topology();
                self.request_missing_parent(
                    block_hash,
                    block_height,
                    block_view,
                    parent_hash,
                    &commit_topology,
                    None,
                    expected_usize,
                    actual_usize,
                    "block_sync_evidence_parent_gap",
                );
                if block_height > expected_height.saturating_add(1) {
                    self.request_missing_parents_for_gap(
                        &commit_topology,
                        None,
                        "block_sync_evidence_parent_gap",
                    );
                }
            }
            self.cache_deferred_block_sync_update(
                super::message::BlockSyncUpdate {
                    block,
                    commit_votes: Vec::new(),
                    commit_qc: incoming_qc,
                    validator_checkpoint,
                    stake_snapshot,
                },
                sender,
                block_hash,
                block_height,
                block_view,
                "evidence_parent_gap",
            );
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::BlockSyncUpdate,
                super::status::ConsensusMessageOutcome::Deferred,
                super::status::ConsensusMessageReason::FutureWindow,
            );
            info!(
                height = block_height,
                view = block_view,
                block = %block_hash,
                local_height,
                parent = ?parent_hash,
                deferred = self.deferred_block_sync_updates.len(),
                "deferred evidence-bearing block sync update until parent is locally available"
            );
            return Ok(());
        }
        if let Some(local_view) = self.stale_view(block_height, block_view) {
            let drop_stale_view = block_sync_stale_view_should_drop(
                true,
                requested_missing_block,
                block_known_locally,
                has_commit_evidence,
            );
            if let Some((kind, outcome, reason)) =
                block_sync_stale_view_drop_record(drop_stale_view)
            {
                debug!(
                    height = block_height,
                    view = block_view,
                    local_view,
                    kind = "BlockSyncUpdate",
                    "dropping consensus message for stale view without missing request"
                );
                self.record_consensus_message_handling(kind, outcome, reason);
                return Ok(());
            }
            let da_enabled = self.runtime_da_enabled();
            debug!(
                height = block_height,
                view = block_view,
                local_view,
                da_enabled,
                block_known_locally,
                has_commit_evidence,
                missing_request = requested_missing_block,
                "accepting BlockSyncUpdate for stale view"
            );
        }
        if let Some(qc) = incoming_qc.as_ref() {
            if let Some(lock) = self.locked_qc {
                let same_height_conflict = Self::block_sync_qc_same_height_conflict(lock, qc);
                if same_height_conflict {
                    let exact_frontier_commit_cert = exact_contiguous_frontier
                        && qc.subject_block_hash == block_hash
                        && qc.height == block_height
                        && qc.view == block_view
                        && qc.epoch == expected_epoch
                        && Self::block_sync_qc_same_height_recoverable(lock, qc, true);
                    if exact_frontier_commit_cert {
                        debug!(
                            height = block_height,
                            view = block_view,
                            block = %block_hash,
                            locked_height = lock.height,
                            locked_view = lock.view,
                            locked_block = %lock.subject_block_hash,
                            "allowing exact-frontier commit QC to supersede same-height locked branch after validation"
                        );
                    } else if self.defer_block_sync_qc_while_locked_payload_missing(
                        qc,
                        "block_sync_update.prefilter.missing_locked_payload",
                    ) {
                        return Ok(());
                    } else {
                        self.log_block_sync_locked_qc_conflict(
                            qc,
                            lock,
                            "block_sync_update.prefilter.height_conflict",
                        );
                        crate::sumeragi::status::inc_block_sync_locked_qc_prefilter_drop();
                        self.record_consensus_message_handling(
                            super::status::ConsensusMessageKind::Qc,
                            super::status::ConsensusMessageOutcome::Dropped,
                            super::status::ConsensusMessageReason::LockedQc,
                        );
                        incoming_qc = None;
                    }
                }
            }
        }
        let (consensus_mode, mode_tag, prf_seed, local_height) = {
            let world_view = self.state.world_view();
            let consensus_mode = super::effective_consensus_mode_for_height_from_world(
                &world_view,
                block_height,
                self.config.consensus_mode,
            );
            let mode_tag = block_sync_consensus_mode_tag(consensus_mode);
            let prf_seed = Some(super::prf_seed_for_height_from_world(
                &world_view,
                &self.common_config.chain,
                block_height,
            ));
            let local_height = u64::try_from(self.state.committed_height()).unwrap_or(u64::MAX);
            (consensus_mode, mode_tag, prf_seed, local_height)
        };
        let kura_committed_start = Instant::now();
        if let Ok(height_usize) = usize::try_from(block_height)
            && let Some(nz_height) = NonZeroUsize::new(height_usize)
        {
            if let Some(committed) = self.kura.get_block(nz_height) {
                let committed_hash = committed.hash();
                let commit_conflict = block_sync_commit_conflict_detected(
                    true,
                    true,
                    true,
                    committed_hash == block_hash,
                );
                if commit_conflict {
                    let Some(commit_qc) = incoming_qc.take() else {
                        info!(
                            committed_height = height_usize,
                            committed_hash = %committed_hash,
                            incoming_hash = %block_hash,
                            "dropping block sync update that conflicts with committed block without commit QC"
                        );
                        if let Some((kind, outcome, reason)) =
                            block_sync_commit_conflict_drop_record(commit_conflict)
                        {
                            self.record_consensus_message_handling(kind, outcome, reason);
                        }
                        if block_sync_commit_conflict_should_clear_missing(commit_conflict) {
                            self.clear_missing_block_request(
                                &block_hash,
                                MissingBlockClearReason::Obsolete,
                            );
                        }
                        return Ok(());
                    };
                    let inputs = self.roster_validation_cache.inputs_for_roster(
                        &commit_qc.validator_set,
                        consensus_mode,
                        stake_snapshot.as_ref(),
                    );
                    let allow_genesis_stub =
                        block_sync_commit_conflict_allow_genesis_stub(block_height, block_view);
                    let should_validate_qc =
                        block_sync_commit_conflict_should_validate_qc(commit_conflict, true);
                    let qc_valid = if should_validate_qc {
                        match super::validate_commit_qc_roster_cached(
                            &self.roster_validation_cache,
                            &commit_qc,
                            block_hash,
                            block_height,
                            Some(block_view),
                            consensus_mode,
                            expected_epoch,
                            &self.common_config.chain,
                            mode_tag,
                            allow_genesis_stub,
                            &inputs,
                        ) {
                            Ok(_) => true,
                            Err(err) => {
                                warn!(
                                    ?err,
                                    committed_height = height_usize,
                                    committed_hash = %committed_hash,
                                    incoming_hash = %block_hash,
                                    "dropping commit-conflict block sync update with invalid commit QC"
                                );
                                if let Some((kind, outcome, reason)) =
                                    block_sync_commit_conflict_drop_record(commit_conflict)
                                {
                                    self.record_consensus_message_handling(kind, outcome, reason);
                                }
                                if block_sync_commit_conflict_should_clear_missing(commit_conflict)
                                {
                                    self.clear_missing_block_request(
                                        &block_hash,
                                        MissingBlockClearReason::Obsolete,
                                    );
                                }
                                return Ok(());
                            }
                        }
                    } else {
                        false
                    };
                    if block_sync_commit_conflict_should_emit_evidence(
                        commit_conflict,
                        true,
                        qc_valid,
                    ) {
                        info!(
                            committed_height = height_usize,
                            committed_hash = %committed_hash,
                            incoming_hash = %block_hash,
                            view = block_view,
                            "rejecting conflicting commit QC at committed height; enforcing finality"
                        );
                        #[cfg(feature = "telemetry")]
                        {
                            self.telemetry.inc_commit_conflict_detected();
                        }
                        let evidence = block_sync_commit_conflict_invalid_qc_evidence(commit_qc);
                        if let Err(err) = self.record_and_broadcast_evidence(evidence) {
                            warn!(
                                ?err,
                                committed_height = height_usize,
                                committed_hash = %committed_hash,
                                incoming_hash = %block_hash,
                                "failed to record commit-conflict evidence"
                            );
                        }
                    }
                    if let Some((kind, outcome, reason)) =
                        block_sync_commit_conflict_drop_record(commit_conflict)
                    {
                        self.record_consensus_message_handling(kind, outcome, reason);
                    }
                    if block_sync_commit_conflict_should_clear_missing(commit_conflict) {
                        self.clear_missing_block_request(
                            &block_hash,
                            MissingBlockClearReason::Obsolete,
                        );
                    }
                    return Ok(());
                }
            }
        }
        let kura_committed_ms =
            u64::try_from(kura_committed_start.elapsed().as_millis()).unwrap_or(u64::MAX);
        let kura_known_start = Instant::now();
        let block_known = self.kura.get_block_height_by_hash(block_hash).is_some();
        let kura_known_ms =
            u64::try_from(kura_known_start.elapsed().as_millis()).unwrap_or(u64::MAX);
        let has_roster_hint = incoming_qc.is_some()
            || validator_checkpoint.is_some()
            || stake_snapshot.is_some()
            || !commit_votes.is_empty();
        if block_known && !has_roster_hint {
            info!(
                hash = ?block_hash,
                height = block_height,
                "skipping block sync update for already known block"
            );
            self.clear_missing_block_request(
                &block_hash,
                MissingBlockClearReason::PayloadAvailable,
            );
            return Ok(());
        }
        if should_mark_block_sync_implicit_recovery(
            self.runtime_da_enabled(),
            requested_missing_block,
            block_known_locally,
            block_height,
            local_height,
            implicit_frontier_recovery_allowed,
        ) {
            // Aborted pending payloads are retained for recovery but must still be treated as
            // missing for consensus progression, otherwise sparse next-height block-sync updates
            // can be dropped before they revive the pending entry.
            requested_missing_block = true;
        }
        if should_note_block_sync_vote_placeholder(
            has_commit_votes,
            incoming_qc.is_some(),
            validator_checkpoint.is_some(),
            exact_contiguous_frontier,
            block_known_locally,
            requested_missing_block,
        ) {
            let now = Instant::now();
            for vote in commit_votes.iter().filter(|vote| {
                block_sync_vote_placeholder_matches(
                    vote,
                    block_hash,
                    block_height,
                    block_view,
                    expected_epoch,
                )
            }) {
                let _ = self.note_frontier_vote_placeholder(
                    vote.block_hash,
                    vote.height,
                    vote.view,
                    None,
                    now,
                );
            }
        }
        let mut commit_votes = Some(commit_votes);
        let mut process_commit_votes = |actor: &mut Actor| {
            let Some(commit_votes) = commit_votes.take() else {
                return (0usize, 0usize);
            };
            let mut processed_votes = 0usize;
            let mut dropped_votes = 0usize;
            for vote in commit_votes {
                if vote.phase != crate::sumeragi::consensus::Phase::Commit
                    || vote.block_hash != block_hash
                    || vote.height != block_height
                    || vote.view != block_view
                    || vote.epoch != expected_epoch
                {
                    dropped_votes = dropped_votes.saturating_add(1);
                    continue;
                }
                actor.handle_vote(vote);
                processed_votes = processed_votes.saturating_add(1);
            }
            if dropped_votes > 0 {
                debug!(
                    height = block_height,
                    view = block_view,
                    block = %block_hash,
                    dropped_votes,
                    "dropping mismatched commit votes from block sync update"
                );
            }
            (processed_votes, dropped_votes)
        };
        let commit_votes_start = Instant::now();
        let (commit_votes_processed, commit_votes_dropped) = process_commit_votes(self);
        let commit_votes_pre_ms =
            u64::try_from(commit_votes_start.elapsed().as_millis()).unwrap_or(u64::MAX);
        // Commit votes can arm missing-block recovery before we reach the roster/payload path.
        // Keep the local gate in sync with the tracked request state so vote-backed stale-view
        // recovery does not get dropped as if no recovery request existed.
        let vote_requested_missing_block = self
            .pending
            .missing_block_requests
            .contains_key(&block_hash);
        if vote_requested_missing_block
            && (implicit_frontier_recovery_allowed || explicit_requested_missing_block)
        {
            requested_missing_block = true;
        }
        let vote_only_known_block_fast_path = block_known
            && has_commit_votes
            && incoming_qc.is_none()
            && validator_checkpoint.is_none()
            && stake_snapshot.is_none();
        if vote_only_known_block_fast_path {
            debug!(
                height = block_height,
                view = block_view,
                block = %block_hash,
                commit_votes_pre_ms,
                commit_votes_processed,
                commit_votes_dropped,
                "processed known-block vote-only block sync update via fast-path"
            );
            self.clear_missing_block_request(
                &block_hash,
                MissingBlockClearReason::PayloadAvailable,
            );
            return Ok(());
        }
        if let Some(reason) = entry_deferral_reason {
            self.defer_block_sync_update(
                super::message::BlockSyncUpdate {
                    block,
                    commit_votes: Vec::new(),
                    commit_qc: incoming_qc,
                    validator_checkpoint,
                    stake_snapshot,
                },
                sender,
                block_hash,
                block_height,
                block_view,
                reason,
            );
            return Ok(());
        }
        let cached_frontier_qc = cached_qc_for(
            &self.qc_cache,
            crate::sumeragi::consensus::Phase::Commit,
            block_hash,
            block_height,
            block_view,
            expected_epoch,
        )
        .is_some();
        let parent_known_locally = parent_hash.is_some_and(|hash| self.block_known_locally(hash));
        let unsolicited_npos_vote_only_frontier = matches!(consensus_mode, ConsensusMode::Npos)
            && has_commit_votes
            && !requested_missing_block
            && incoming_qc.is_none()
            && validator_checkpoint.is_none()
            && !cached_frontier_qc;
        let frontier_lane_payload_only = frontier_lane_owned
            && !frontier_lane_deep_catchup
            && !block_known
            && incoming_qc.is_none()
            && validator_checkpoint.is_none()
            && !cached_frontier_qc
            && !unsolicited_npos_vote_only_frontier
            && (requested_missing_block
                || parent_known_locally
                || (matches!(consensus_mode, ConsensusMode::Npos)
                    && block_height == local_height.saturating_add(1)));
        if frontier_lane_payload_only {
            info!(
                height = block_height,
                view = block_view,
                block = %block_hash,
                processed_votes = commit_votes_processed,
                "routing payload-only frontier-lane BlockSyncUpdate through BlockCreated owner"
            );
            let _ = self.maybe_cache_frontier_payload_sender_vote_roster(
                &block,
                block_hash,
                block_height,
                block_view,
                consensus_mode,
                sender.as_ref(),
            );
            let creation_result = self.handle_block_created_from_block_sync(
                super::message::BlockCreated {
                    block: block.clone(),
                    frontier: None,
                },
                sender.clone(),
                true,
                if requested_missing_block {
                    BlockSyncRecoveryMode::RequestedPayloadRepair
                } else {
                    BlockSyncRecoveryMode::PayloadOnly
                },
            );
            let mut recovery_targets = Vec::new();
            let aborted_pending_without_commit_evidence = self
                .pending
                .pending_blocks
                .get(&block_hash)
                .is_some_and(|pending| pending.is_retry_aborted())
                && !has_commit_evidence
                && !cached_frontier_qc;
            let payload_materialized = creation_result.is_ok()
                && !aborted_pending_without_commit_evidence
                && self.materialize_frontier_block_sync_payload_for_qc_recovery(&block, None);
            if creation_result.is_ok() {
                let mut roster = self
                    .vote_roster_cache
                    .get(&block_hash)
                    .map(|entry| entry.roster.clone())
                    .unwrap_or_default();
                if roster.is_empty() {
                    roster = self.roster_for_vote_with_mode(
                        block_hash,
                        block_height,
                        block_view,
                        consensus_mode,
                    );
                }
                if roster.is_empty() {
                    roster = self.effective_commit_topology();
                }
                if roster.is_empty() {
                    roster = self.trusted_topology();
                }
                recovery_targets = roster.clone();
                self.cache_vote_roster(block_hash, block_height, block_view, roster);
            }
            let block_known_after_creation = self.block_known_locally(block_hash);
            if payload_materialized || block_known_after_creation {
                let _ = self.try_replay_deferred_qcs();
                let _ = self.try_replay_deferred_missing_payload_qcs(Instant::now());
                self.request_commit_pipeline_for_pending(
                    block_hash,
                    super::status::RoundEventCauseTrace::BlockSyncUpdated,
                    None,
                );
                if block_height == local_height.saturating_add(1) {
                    let recovery_targets = self.known_block_commit_qc_recovery_targets(
                        block_hash,
                        block_height,
                        block_view,
                        &recovery_targets,
                    );
                    let _ = self.maybe_request_known_block_commit_qc_recovery(
                        block_hash,
                        block_height,
                        block_view,
                        &recovery_targets,
                        None,
                        "block_sync_update_payload_only_frontier_lane",
                    );
                }
            }
            return creation_result;
        }
        // For known blocks, prefer the locally recorded commit roster snapshot and ignore
        // mismatching hints to avoid re-validating rosters on the main loop.
        let snapshot = block_known
            .then(|| {
                self.state
                    .commit_roster_snapshot_for_block(block_height, block_hash)
            })
            .flatten();
        let qc_hash_matches = snapshot
            .as_ref()
            .zip(incoming_qc.as_ref())
            .is_some_and(|(snapshot, qc)| HashOf::new(qc) == HashOf::new(&snapshot.commit_qc));
        let qc_same_validator_set =
            snapshot
                .as_ref()
                .zip(incoming_qc.as_ref())
                .is_some_and(|(snapshot, qc)| {
                    qc.validator_set_hash_version == snapshot.commit_qc.validator_set_hash_version
                        && qc.validator_set_hash == snapshot.commit_qc.validator_set_hash
                        && qc.validator_set == snapshot.commit_qc.validator_set
                });
        let checkpoint_hash_matches = snapshot
            .as_ref()
            .zip(validator_checkpoint.as_ref())
            .is_some_and(|(snapshot, checkpoint)| {
                HashOf::new(checkpoint) == HashOf::new(&snapshot.validator_checkpoint)
            });
        let local_stake_present = snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.stake_snapshot.as_ref())
            .is_some();
        let stake_hash_matches = snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.stake_snapshot.as_ref())
            .zip(stake_snapshot.as_ref())
            .is_some_and(|(local, stake)| HashOf::new(local) == HashOf::new(stake));
        let snapshot_filter = block_sync_snapshot_hint_filter(
            snapshot.is_some(),
            incoming_qc.is_some(),
            qc_hash_matches,
            qc_same_validator_set,
            validator_checkpoint.is_some(),
            checkpoint_hash_matches,
            stake_snapshot.is_some(),
            local_stake_present,
            stake_hash_matches,
        );
        if snapshot_filter.snapshot_present {
            let snapshot = snapshot.as_ref().expect("snapshot present");
            if let Some(qc) = incoming_qc.as_ref() {
                if snapshot_filter.qc_revalidated {
                    info!(
                        height = block_height,
                        view = block_view,
                        block = %block_hash,
                        incoming_signers = qc_signer_count(qc),
                        local_signers = qc_signer_count(&snapshot.commit_qc),
                        "incoming block sync QC differs from local snapshot; revalidating"
                    );
                } else if !snapshot_filter.qc_after {
                    info!(
                        height = block_height,
                        view = block_view,
                        block = %block_hash,
                        "dropping block sync QC: does not match local commit roster snapshot"
                    );
                    incoming_qc = None;
                }
            }
            if validator_checkpoint.is_some() && !snapshot_filter.checkpoint_after {
                info!(
                    height = block_height,
                    view = block_view,
                    block = %block_hash,
                    "dropping block sync validator checkpoint: does not match local snapshot"
                );
                validator_checkpoint = None;
            }
            if stake_snapshot.is_some() && !snapshot_filter.stake_after {
                info!(
                    height = block_height,
                    view = block_view,
                    block = %block_hash,
                    "dropping block sync stake snapshot: does not match local snapshot"
                );
                stake_snapshot = None;
            }
        }
        let snapshot_selection = snapshot.as_ref().and_then(|snapshot| {
            let selection = block_sync_snapshot_roster_selection(snapshot)?;
            if let Some(key) = BlockSyncRosterCacheKey::from_hints(
                block_hash,
                block_height,
                block_view,
                consensus_mode,
                selection.commit_qc.as_ref(),
                selection.checkpoint.as_ref(),
                selection.stake_snapshot.as_ref(),
            ) {
                self.block_sync_roster_cache.insert(key, selection.clone());
            }
            Some(selection)
        });
        let roster_start = Instant::now();
        let persisted_roster_start = Instant::now();
        let allow_sidecar = !self.sidecar_quarantined_for_height(block_height);
        let persisted_roster = snapshot_selection.or_else(|| {
            persisted_roster_for_block(
                self.state.as_ref(),
                &self.kura,
                consensus_mode,
                block_height,
                block_hash,
                Some(block_view),
                &self.roster_validation_cache,
                Some(&mut self.block_sync_roster_cache),
                allow_sidecar,
            )
        });
        let roster_persisted_ms =
            u64::try_from(persisted_roster_start.elapsed().as_millis()).unwrap_or(u64::MAX);
        let cert_hint = incoming_qc.as_ref();
        let checkpoint_hint = validator_checkpoint.as_ref();
        let roster_cache_key = super::BlockSyncRosterCacheKey::from_hints(
            block_hash,
            block_height,
            block_view,
            consensus_mode,
            cert_hint,
            checkpoint_hint,
            stake_snapshot.as_ref(),
        );
        // Allow next-height block sync updates without roster artifacts; missing-block requests
        // already opt into the uncertified path.
        let allow_uncertified = allow_uncertified_block_sync_roster(
            block_height,
            local_height,
            requested_missing_block,
        );
        let selection_start = Instant::now();
        let selection = if let Some(selection) = persisted_roster {
            Some(selection)
        } else if let Some(selection) = roster_cache_key
            .as_ref()
            .and_then(|key| self.block_sync_roster_cache.get(key))
        {
            debug!(
                height = block_height,
                view = block_view,
                block = %block_hash,
                source = selection.source.as_str(),
                "block sync roster cache hit"
            );
            Some(selection)
        } else {
            let selection = select_block_sync_roster(
                &block,
                block_hash,
                block_height,
                None,
                cert_hint,
                checkpoint_hint,
                stake_snapshot.as_ref(),
                self.state.as_ref(),
                self.common_config.trusted_peers.value(),
                self.common_config.peer.id(),
                consensus_mode,
                mode_tag,
                allow_uncertified,
                &self.roster_validation_cache,
            );
            if let (Some(selection), Some(key)) = (selection.as_ref(), roster_cache_key.as_ref()) {
                if selection.commit_qc.is_some() || selection.checkpoint.is_some() {
                    self.block_sync_roster_cache
                        .insert(key.clone(), selection.clone());
                }
            }
            selection
        };
        let roster_select_ms =
            u64::try_from(selection_start.elapsed().as_millis()).unwrap_or(u64::MAX);
        let roster_ms = u64::try_from(roster_start.elapsed().as_millis()).unwrap_or(u64::MAX);
        let roster_validate_ms = roster_ms;
        let roster_snapshot = self
            .state
            .commit_roster_snapshot_for_block(block_height, block_hash);
        let Some(selection) = selection else {
            if block_sync_no_roster_known_vote_only(
                block_known,
                has_commit_votes,
                cert_hint.is_some(),
                checkpoint_hint.is_some(),
                stake_snapshot.is_some(),
            ) {
                if roster_snapshot.is_some() {
                    debug!(
                        height = block_height,
                        view = block_view,
                        block = %block_hash,
                        "dropping vote-only block sync update for known block with local commit roster snapshot"
                    );
                } else {
                    info!(
                        height = block_height,
                        view = block_view,
                        block = %block_hash,
                        "processing commit votes without roster hints for known block"
                    );
                    process_commit_votes(self);
                }
                self.clear_missing_block_request(
                    &block_hash,
                    MissingBlockClearReason::PayloadAvailable,
                );
                return Ok(());
            }
            if !block_known {
                let (_, fallback_roster) = block_sync_no_roster_fallback_roster(
                    self.effective_commit_topology(),
                    self.trusted_topology(),
                );
                if !fallback_roster.is_empty() {
                    let fallback_topology = super::network_topology::Topology::new(fallback_roster);
                    let empty_signers =
                        BTreeSet::<crate::sumeragi::consensus::ValidatorIndex>::new();
                    if self.keep_exact_frontier_block_sync_repair_in_slot(
                        block_hash,
                        block_height,
                        block_view,
                        &empty_signers,
                        &fallback_topology,
                        "block_sync_update_missing_roster",
                    ) {
                        self.record_consensus_message_handling(
                            super::status::ConsensusMessageKind::BlockSyncUpdate,
                            super::status::ConsensusMessageOutcome::Deferred,
                            super::status::ConsensusMessageReason::RosterMissing,
                        );
                        return Ok(());
                    }
                    if self.maybe_request_pending_block_for_missing_qc(
                        block_hash,
                        block_height,
                        block_view,
                        block.signatures().count(),
                        fallback_topology.min_votes_for_commit().max(1),
                        &empty_signers,
                        &fallback_topology,
                    ) {
                        requested_missing_block = true;
                    }
                }
                if requested_missing_block {
                    let failover_requested = self.force_tracked_missing_height_sidecar_failover(
                        block_height,
                        block_hash,
                        "block_sync_update_missing_roster",
                    );
                    if failover_requested {
                        debug!(
                            height = block_height,
                            view = block_view,
                            block = %block_hash,
                            "forced deterministic sidecar failover for tracked missing block without verifiable roster"
                        );
                    }
                }
            }
            warn!(
                height = block_height,
                view = block_view,
                block = %block_hash,
                cert_hint = cert_hint.is_some(),
                checkpoint_hint = checkpoint_hint.is_some(),
                requested_missing_block,
                roster_snapshot = roster_snapshot.is_some(),
                roster_validate_ms = roster_ms,
                "dropping block sync update: no verifiable roster available"
            );
            super::status::inc_block_sync_drop_invalid_signatures();
            super::status::inc_block_sync_roster_drop_missing();
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::BlockSyncUpdate,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::RosterMissing,
            );
            #[cfg(feature = "telemetry")]
            if let Some(telemetry) = self.telemetry_handle() {
                telemetry.note_block_sync_roster_drop("missing");
            }
            if block_known {
                self.clear_missing_block_request(
                    &block_hash,
                    MissingBlockClearReason::PayloadAvailable,
                );
            }
            return Ok(());
        };
        super::status::inc_block_sync_roster_source(selection.source.as_str());
        #[cfg(feature = "telemetry")]
        if let Some(telemetry) = self.telemetry_handle() {
            telemetry.note_block_sync_roster_source(selection.source.as_str());
        }
        info!(
            height = block_height,
            view = block_view,
            block = %block_hash,
            source = selection.source.as_str(),
            "block sync roster selected"
        );
        self.cache_vote_roster(
            block_hash,
            block_height,
            block_view,
            selection.roster.clone(),
        );
        let topology = super::network_topology::Topology::new(selection.roster.clone());
        if let Some(checkpoint) = selection.checkpoint.clone() {
            super::status::record_validator_checkpoint(checkpoint);
        }
        // Persist commit rosters only once the block is known locally.
        let commit_roster_record = selection.commit_qc.as_ref().map(|cert| {
            let checkpoint = selection.checkpoint.clone().unwrap_or_else(|| {
                ValidatorSetCheckpoint::new_with_chain_order(
                    cert.height,
                    cert.view,
                    cert.subject_block_hash,
                    cert.chain_order_hash,
                    cert.rechain_seq,
                    cert.parent_state_root,
                    cert.post_state_root,
                    cert.validator_set.clone(),
                    cert.aggregate.signers_bitmap.clone(),
                    cert.aggregate.bls_aggregate_signature.clone(),
                    cert.validator_set_hash_version,
                    None,
                )
            });
            (cert.clone(), checkpoint, selection.stake_snapshot.clone())
        });
        if block_known {
            if let Some((cert, checkpoint, stake_snapshot)) = commit_roster_record.as_ref() {
                self.state
                    .record_commit_roster(cert, checkpoint, stake_snapshot.clone());
            }
            info!(
                hash = ?block_hash,
                height = block_height,
                "skipping block sync update for already known block"
            );
            process_commit_votes(self);
            // Known blocks may still be waiting on a commit QC (e.g., persisted before QC arrival).
            let checkpoint_qc = selection.checkpoint.as_ref().and_then(|checkpoint| {
                self.commit_qc_from_validator_checkpoint(
                    block_hash,
                    block_height,
                    block_view,
                    checkpoint,
                    selection
                        .stake_snapshot
                        .as_ref()
                        .or(stake_snapshot.as_ref()),
                )
            });
            if let Some((candidate_qc_source, qc)) = block_sync_known_roster_candidate_qc(
                incoming_qc.take(),
                selection.commit_qc.clone(),
                checkpoint_qc,
            ) {
                let qc_hash = HashOf::new(&qc);
                let cached_qc_match = cached_qc_for(
                    &self.qc_cache,
                    crate::sumeragi::consensus::Phase::Commit,
                    block_hash,
                    block_height,
                    block_view,
                    expected_epoch,
                )
                .is_some_and(|cached| HashOf::new(&cached) == qc_hash);
                let local_snapshot_qc_match = roster_snapshot
                    .as_ref()
                    .is_some_and(|snapshot| HashOf::new(&snapshot.commit_qc) == qc_hash);
                if cached_qc_match && local_snapshot_qc_match {
                    debug!(
                        height = block_height,
                        view = block_view,
                        block = %block_hash,
                        "skipping redundant known-block QC replay: commit QC already cached locally"
                    );
                } else {
                    let commit_qc_match =
                        candidate_qc_source == BlockSyncKnownRosterCandidateQcSource::Selection;
                    let work = self.prepare_known_block_qc_work(
                        qc,
                        Arc::new(block),
                        topology.clone(),
                        stake_snapshot.clone(),
                        consensus_mode,
                        mode_tag,
                        prf_seed,
                        commit_qc_match,
                    );
                    if let Some(work) = work {
                        self.enqueue_known_block_qc_work(work);
                    }
                }
            }
            if self
                .cached_commit_qc_for_block(block_hash, block_height, block_view)
                .is_some()
            {
                self.clear_missing_commit_qc_request(
                    &block_hash,
                    MissingBlockClearReason::Obsolete,
                );
            }
            self.clear_missing_block_request(
                &block_hash,
                MissingBlockClearReason::PayloadAvailable,
            );
            return Ok(());
        }
        let had_incoming_qc = incoming_qc.is_some();
        let signer_cache_key = BlockSignerCacheKey::new(
            block_hash,
            selection.roster.as_slice(),
            consensus_mode,
            prf_seed,
        );
        let signature_start = Instant::now();
        let cached_block_signers = signer_cache_key
            .as_ref()
            .and_then(|key| self.block_signer_cache.get(key));
        let block_signers = if let Some(signers) = cached_block_signers {
            debug!(
                height = block_height,
                view = block_view,
                block = %block_hash,
                signers = signers.len(),
                "block sync signer cache hit"
            );
            signers
        } else {
            let block_signers_result = {
                let world_view = self.state.world_view();
                validated_block_signers_from_world(
                    &block,
                    &topology,
                    &world_view,
                    mode_tag,
                    prf_seed,
                )
            };
            match block_signers_result {
                Ok(signers) => {
                    if block_sync_selected_signatures_should_cache_validated_signers(
                        signer_cache_key.is_some(),
                    ) && let Some(key) = signer_cache_key.clone()
                    {
                        self.block_signer_cache.insert(key, signers.clone());
                    }
                    signers
                }
                Err(err) => {
                    let local_height =
                        u64::try_from(self.state.committed_height()).unwrap_or(u64::MAX);
                    let parent_missing = block
                        .header()
                        .prev_block_hash()
                        .is_some_and(|hash| !self.block_known_locally(hash));
                    let ahead = block_sync_selected_signatures_ahead_of_frontier(
                        block_height,
                        local_height,
                    );
                    if block_sync_selected_signatures_should_defer(parent_missing, ahead, err) {
                        let expected_height = local_height.saturating_add(1);
                        let expected_usize = usize::try_from(expected_height).ok();
                        let actual_usize = usize::try_from(block_height).ok();
                        if let Some(parent_hash) = block.header().prev_block_hash() {
                            let commit_topology = self.effective_commit_topology();
                            self.request_missing_parent(
                                block_hash,
                                block_height,
                                block_view,
                                parent_hash,
                                &commit_topology,
                                Some(&selection.roster),
                                expected_usize,
                                actual_usize,
                                "block_sync_signatures",
                            );
                            if block_sync_selected_signatures_should_request_gap(
                                block_height,
                                expected_height,
                            ) {
                                self.request_missing_parents_for_gap(
                                    &commit_topology,
                                    Some(&selection.roster),
                                    "block_sync_gap",
                                );
                            }
                        }
                        info!(
                            ?err,
                            height = block_height,
                            view = block_view,
                            block = %block_hash,
                            local_height,
                            "deferring block sync update due to signature mismatch while behind"
                        );
                        self.record_consensus_message_handling(
                            super::status::ConsensusMessageKind::BlockSyncUpdate,
                            super::status::ConsensusMessageOutcome::Deferred,
                            super::status::ConsensusMessageReason::SignatureMismatchDeferred,
                        );
                        let created = super::message::BlockCreated {
                            block,
                            frontier: None,
                        };
                        let _ = self.handle_block_created_from_block_sync(
                            created,
                            sender.clone(),
                            true,
                            BlockSyncRecoveryMode::PayloadOnly,
                        );
                        return Ok(());
                    }
                    let has_roster_evidence = block_sync_selected_signatures_has_roster_evidence(
                        incoming_qc.is_some(),
                        selection.commit_qc.is_some(),
                        selection.checkpoint.is_some(),
                    );
                    if has_roster_evidence {
                        warn!(
                            ?err,
                            hash = ?block_hash,
                            height = block_height,
                            view = block_view,
                            has_incoming_qc = incoming_qc.is_some(),
                            has_commit_qc = selection.commit_qc.is_some(),
                            has_checkpoint = selection.checkpoint.is_some(),
                            "continuing block sync update despite signature mismatch because roster/QC evidence is available"
                        );
                        BTreeSet::new()
                    } else {
                        super::status::inc_block_sync_drop_invalid_signatures();
                        warn!(
                            ?err,
                            hash = ?block_hash,
                            height = block_height,
                            view = block_view,
                            "dropping block sync update with invalid or insufficient signatures"
                        );
                        self.record_consensus_message_handling(
                            super::status::ConsensusMessageKind::BlockSyncUpdate,
                            super::status::ConsensusMessageOutcome::Dropped,
                            super::status::ConsensusMessageReason::InvalidSignature,
                        );
                        return Ok(());
                    }
                }
            }
        };
        let signature_verify_ms =
            u64::try_from(signature_start.elapsed().as_millis()).unwrap_or(u64::MAX);
        let qc_candidate_start = Instant::now();
        let commit_quorum = topology.min_votes_for_commit().max(1);
        let candidate_qc = {
            let world_view = self.state.world_view();
            let checkpoint_qc = selection.checkpoint.as_ref().and_then(|checkpoint| {
                self.commit_qc_from_validator_checkpoint(
                    block_hash,
                    block_height,
                    block_view,
                    checkpoint,
                    selection
                        .stake_snapshot
                        .as_ref()
                        .or(stake_snapshot.as_ref()),
                )
            });
            let world_qc = crate::block_sync::BlockSynchronizer::block_sync_qc_for_world(
                &world_view,
                self.config.consensus_mode,
                &block,
            );
            let cached_qc = cached_qc_for(
                &self.qc_cache,
                crate::sumeragi::consensus::Phase::Commit,
                block_hash,
                block_height,
                block_view,
                expected_epoch,
            );
            block_sync_selected_qc_candidate(
                incoming_qc,
                selection.commit_qc.clone(),
                checkpoint_qc,
                world_qc,
                cached_qc,
            )
            .map(|(_, qc)| qc)
        };
        let candidate_qc = candidate_qc.and_then(|qc| {
            match block_sync_selected_qc_shape(&qc, block_hash, block_height, expected_epoch) {
                BlockSyncSelectedQcShape::Valid => Some(qc),
                BlockSyncSelectedQcShape::HeightMismatch => {
                    warn!(
                        height = block_height,
                        view = block_view,
                        hash = %block_hash,
                        qc_height = qc.height,
                        "dropping block sync QC with mismatched height"
                    );
                    None
                }
                BlockSyncSelectedQcShape::HashMismatch => {
                    warn!(
                        height = block_height,
                        view = block_view,
                        hash = %block_hash,
                        qc_hash = %qc.subject_block_hash,
                        "dropping block sync QC with mismatched block hash"
                    );
                    None
                }
                BlockSyncSelectedQcShape::EpochMismatch => {
                    warn!(
                        height = block_height,
                        view = block_view,
                        hash = %block_hash,
                        expected_epoch,
                        qc_epoch = qc.epoch,
                        "dropping block sync QC with mismatched epoch"
                    );
                    None
                }
                BlockSyncSelectedQcShape::PhaseMismatch => {
                    warn!(
                        height = block_height,
                        view = block_view,
                        hash = %block_hash,
                        phase = ?qc.phase,
                        "dropping block sync QC with non-precommit phase"
                    );
                    None
                }
            }
        });
        let original_candidate_qc = candidate_qc.clone();
        let qc_candidate_ms =
            u64::try_from(qc_candidate_start.elapsed().as_millis()).unwrap_or(u64::MAX);

        let qc_validate_start = Instant::now();
        let commit_cert_hint_present = selection.commit_qc.is_some();
        let checkpoint_present = selection.checkpoint.is_some();
        let candidate_qc_present = candidate_qc.is_some();
        let candidate_qc_signers = candidate_qc.as_ref().map(qc_signer_count);
        let block_signer_count = block_signers.len();
        let signature_quorum_met = match consensus_mode {
            ConsensusMode::Permissioned => block_signer_count >= commit_quorum,
            ConsensusMode::Npos => {
                let signature_topology = super::topology_for_view(
                    &topology,
                    block_height,
                    block_view,
                    mode_tag,
                    prf_seed,
                );
                let mut signer_peers = BTreeSet::new();
                for signer in &block_signers {
                    let Ok(idx) = usize::try_from(*signer) else {
                        continue;
                    };
                    let Some(peer) = signature_topology.as_ref().get(idx) else {
                        continue;
                    };
                    signer_peers.insert(peer.clone());
                }
                if let Some(snapshot) = selection.stake_snapshot.as_ref() {
                    super::stake_snapshot::stake_quorum_reached_for_snapshot(
                        snapshot,
                        &selection.roster,
                        &signer_peers,
                    )
                    .unwrap_or(false)
                } else {
                    false
                }
            }
        };
        let local_height = u64::try_from(self.state.committed_height()).unwrap_or(u64::MAX);
        let mut cached_qc_tally: Option<QcSignerTally> = None;
        let cached_qc_match = candidate_qc.as_ref().and_then(|qc| {
            cached_qc_for(
                &self.qc_cache,
                qc.phase,
                qc.subject_block_hash,
                qc.height,
                qc.view,
                qc.epoch,
            )
            .filter(|cached| HashOf::new(cached) == HashOf::new(qc))
        });
        // Reuse prior validation to skip expensive aggregate verification for identical QCs.
        let aggregate_ok = candidate_qc.as_ref().and_then(|qc| {
            block_sync_selected_qc_aggregate_ok(
                cached_qc_match.is_some(),
                selection
                    .commit_qc
                    .as_ref()
                    .is_some_and(|cert| HashOf::new(cert) == HashOf::new(qc)),
            )
        });
        let validated_qc = candidate_qc.as_ref().and_then(|qc| {
            let world_view = self.state.world_view();
            match validate_block_sync_qc(
                qc,
                &topology,
                &world_view,
                &block_signers,
                block_view,
                &self.roster_validation_cache.pops,
                &self.common_config.chain,
                consensus_mode,
                stake_snapshot.as_ref(),
                mode_tag,
                prf_seed,
                aggregate_ok,
            ) {
                Ok((signers, present_signers)) => {
                    cached_qc_tally = Some(QcSignerTally {
                        voting_signers: signers,
                        present_signers,
                    });
                    Some(qc.clone())
                }
                Err(err) => {
                    record_qc_validation_error(self.telemetry_handle(), &err);
                    if had_incoming_qc {
                        super::status::inc_block_sync_qc_replaced();
                    }
                    let reason = qc_validation_reason(&err);
                    if Self::block_sync_qc_is_missing_context_error(&err) {
                        self.quarantine_block_sync_qc_candidate(
                            qc.clone(),
                            reason,
                            QuarantinedQcTarget::BlockSync,
                        );
                        warn!(
                            ?err,
                            reason,
                            hash = ?block_hash,
                            height = block_height,
                            view = block_view,
                            block_signers = block_signer_count,
                            candidate_qc_signers,
                            had_incoming_qc,
                            "quarantining block sync QC while dependencies are unresolved"
                        );
                    } else {
                        self.block_sync_qc_final_drop(reason);
                        warn!(
                            ?err,
                            reason,
                            hash = ?block_hash,
                            height = block_height,
                            view = block_view,
                            block_signers = block_signer_count,
                            candidate_qc_signers,
                            had_incoming_qc,
                            "dropping block sync QC after validation failure"
                        );
                    }
                    None
                }
            }
        });

        let derive_valid_qc = || {
            cached_qc_for(
                &self.qc_cache,
                crate::sumeragi::consensus::Phase::Commit,
                block_hash,
                block_height,
                block_view,
                expected_epoch,
            )
            .and_then(|qc| {
                let world_view = self.state.world_view();
                validate_block_sync_qc(
                    &qc,
                    &topology,
                    &world_view,
                    &block_signers,
                    block_view,
                    &self.roster_validation_cache.pops,
                    &self.common_config.chain,
                    consensus_mode,
                    stake_snapshot.as_ref(),
                    mode_tag,
                    prf_seed,
                    Some(true),
                )
                .ok()
                .map(|_| qc)
            })
        };

        let candidate_kept = candidate_qc.is_some();
        let candidate_validated = validated_qc.is_some();
        let (mut incoming_qc, incoming_qc_validated) = if let Some(qc) = validated_qc {
            (Some(qc), true)
        } else if block_sync_selected_qc_should_derive_cached(
            candidate_kept,
            candidate_validated,
            had_incoming_qc,
        ) {
            let derived = derive_valid_qc();
            let derived_validated = derived.is_some();
            (derived, derived_validated)
        } else {
            (None, false)
        };
        if incoming_qc_validated {
            if let (Some(qc), Some(tally)) = (incoming_qc.as_ref(), cached_qc_tally.take()) {
                self.note_validated_qc_tally(qc, tally);
            }
        }
        let aggregate_fallback_attempted = block_sync_selected_qc_should_attempt_aggregate_fallback(
            had_incoming_qc,
            incoming_qc.is_some(),
        );
        let qc_fallback_ms = if aggregate_fallback_attempted {
            let qc_fallback_start = Instant::now();
            if let Some(qc) = original_candidate_qc {
                let aggregate_fallback_ok = block_sync_qc_aggregate_fallback_ok(
                    &qc,
                    &topology,
                    &self.roster_validation_cache.pops,
                    &self.common_config.chain,
                    consensus_mode,
                    stake_snapshot.as_ref(),
                    mode_tag,
                );
                if block_sync_selected_qc_should_accept_aggregate_fallback(
                    aggregate_fallback_attempted,
                    true,
                    aggregate_fallback_ok,
                ) {
                    let qc_signers = qc_signer_count(&qc);
                    info!(
                        hash = %block_hash,
                        height = block_height,
                        view = block_view,
                        qc_signers,
                        "accepting block sync QC validated from aggregate signature despite local validation failure"
                    );
                    incoming_qc = Some(qc);
                }
            }
            u64::try_from(qc_fallback_start.elapsed().as_millis()).unwrap_or(u64::MAX)
        } else {
            0
        };
        let qc_validate_ms =
            u64::try_from(qc_validate_start.elapsed().as_millis()).unwrap_or(u64::MAX);
        let hard_locked_conflict = incoming_qc.as_ref().is_some_and(|qc| {
            self.locked_qc.is_some_and(|lock| {
                Self::block_sync_qc_same_height_conflict(lock, qc)
                    && !Self::block_sync_qc_same_height_recoverable(lock, qc, true)
            })
        });
        if hard_locked_conflict
            && let (Some(qc), Some(lock)) = (incoming_qc.as_ref(), self.locked_qc)
        {
            self.log_block_sync_locked_qc_conflict(qc, lock, "block_sync_update.prefetch_cache");
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::Qc,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::LockedQc,
            );
            // Treat a locked-chain conflict as unusable QC evidence in this update path.
            incoming_qc = None;
        }
        let incoming_qc_usable = incoming_qc_validated && !hard_locked_conflict;
        if incoming_qc_usable {
            if let Some(qc) = incoming_qc.as_ref() {
                self.quarantined_block_sync_qcs
                    .remove(&Self::qc_tally_key(qc));
                self.qc_cache.insert(Self::qc_tally_key(qc), qc.clone());
            }
        }
        let qc_evidence_present = incoming_qc.is_some();
        let commit_cert_present = super::block_sync_commit_cert_present(
            commit_cert_hint_present,
            incoming_qc_validated,
            hard_locked_conflict,
        );
        let invalid_qc_present = had_incoming_qc && !incoming_qc_validated && !qc_evidence_present;
        let block_quorum_met = block_signer_count >= commit_quorum;
        if block_sync_selected_qc_should_drop_invalid_payload(
            invalid_qc_present,
            block_quorum_met,
            commit_cert_present,
            checkpoint_present,
        ) {
            warn!(
                hash = ?block_hash,
                height = block_height,
                view = block_view,
                "dropping block sync update with invalid QC and insufficient quorum"
            );
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::BlockSyncUpdate,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::InvalidPayload,
            );
            return Ok(());
        }
        let sparse_exact_frontier_recovery_requested =
            block_sync_selected_quorum_sparse_exact_frontier_request(
                requested_missing_block,
                exact_contiguous_frontier,
                qc_evidence_present,
                checkpoint_present,
                has_commit_votes,
            );
        let mut quorum_available = block_sync_quorum_available(
            block_signer_count,
            commit_quorum,
            signature_quorum_met,
            qc_evidence_present,
            commit_cert_present,
            checkpoint_present,
            explicit_requested_missing_block || sparse_exact_frontier_recovery_requested,
            block_height,
            local_height,
        );
        if block_sync_selected_quorum_should_maybe_request_missing_qc(
            quorum_available,
            qc_evidence_present,
            commit_cert_present,
            checkpoint_present,
            block_signer_count,
            commit_quorum,
            requested_missing_block,
        ) {
            if self.maybe_request_pending_block_for_missing_qc(
                block_hash,
                block_height,
                block_view,
                block_signer_count,
                commit_quorum,
                &block_signers,
                &topology,
            ) {
                if block_sync_selected_quorum_should_defer_npos_vote_only(
                    matches!(consensus_mode, ConsensusMode::Npos),
                    vote_only_frontier_update,
                    explicit_requested_missing_block,
                ) {
                    self.record_consensus_message_handling(
                        super::status::ConsensusMessageKind::BlockSyncUpdate,
                        super::status::ConsensusMessageOutcome::Deferred,
                        super::status::ConsensusMessageReason::QuorumMissing,
                    );
                    return Ok(());
                }
                // Invariant A: sparse missing-QC updates must transition request state in this
                // same event step (or stay explicitly suppressed via backoff).
                requested_missing_block = true;
                quorum_available = block_sync_quorum_available(
                    block_signer_count,
                    commit_quorum,
                    signature_quorum_met,
                    qc_evidence_present,
                    commit_cert_present,
                    checkpoint_present,
                    requested_missing_block,
                    block_height,
                    local_height,
                );
            }
        }
        if block_sync_selected_quorum_should_call_repair(quorum_available) {
            if !qc_evidence_present
                && !commit_cert_present
                && !checkpoint_present
                && block_signer_count < commit_quorum
                && !requested_missing_block
            {
                debug!(
                    hash = ?block_hash,
                    height = block_height,
                    view = block_view,
                    "sparse block sync update remained unrequested after recovery planning"
                );
            }
            if self.keep_exact_frontier_block_sync_repair_in_slot(
                block_hash,
                block_height,
                block_view,
                &block_signers,
                &topology,
                "block_sync_update_missing_commit_quorum",
            ) {
                self.record_consensus_message_handling(
                    super::status::ConsensusMessageKind::BlockSyncUpdate,
                    super::status::ConsensusMessageOutcome::Deferred,
                    super::status::ConsensusMessageReason::QuorumMissing,
                );
                return Ok(());
            }
            super::status::inc_block_sync_drop_invalid_signatures();
            let warn_cooldown = self
                .rebroadcast_cooldown()
                .max(super::BLOCK_SYNC_WARN_COOLDOWN_FLOOR);
            let warn_now = Instant::now();
            if let Some(suppressed_since_last) = self.block_sync_warning_log.allow(
                super::BlockSyncWarningKind::MissingCommitRoleQuorum,
                block_hash,
                block_height,
                block_view,
                warn_now,
                warn_cooldown,
                super::BLOCK_SYNC_WARN_BURST_WINDOW,
                super::BLOCK_SYNC_WARN_BURST_CAP,
            ) {
                self.hotspot_log_summary.record_block_sync_warn();
                if suppressed_since_last > 0 {
                    self.hotspot_log_summary
                        .record_block_sync_suppressed(suppressed_since_last);
                }
                warn!(
                    hash = ?block_hash,
                    height = block_height,
                    view = block_view,
                    block_signers = block_signer_count,
                    signatures = block.signatures().count(),
                    commit_quorum,
                    candidate_qc_present,
                    candidate_qc_signers,
                    qc_evidence_present,
                    incoming_qc_validated,
                    missing_request = requested_missing_block,
                    local_height,
                    suppressed_since_last,
                    warn_cooldown_ms = warn_cooldown.as_millis(),
                    "dropping block sync update missing commit-role quorum"
                );
            } else {
                self.hotspot_log_summary.record_block_sync_suppressed(1);
            }
            self.hotspot_log_summary.emit_if_due(warn_now);
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::BlockSyncUpdate,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::QuorumMissing,
            );
            return Ok(());
        }
        let incoming_qc_signers = incoming_qc.as_ref().map(qc_signer_count);
        let selection_commit_qc_present = selection.commit_qc.is_some();
        let incoming_qc_validated_by_roster = !selection_commit_qc_present
            && incoming_qc.as_ref().is_some_and(|cert| {
                let inputs = self.roster_validation_cache.inputs_for_roster(
                    &cert.validator_set,
                    consensus_mode,
                    stake_snapshot.as_ref(),
                );
                super::validate_commit_qc_roster_cached(
                    &self.roster_validation_cache,
                    cert,
                    block_hash,
                    block_height,
                    Some(block_view),
                    consensus_mode,
                    expected_epoch,
                    &self.common_config.chain,
                    mode_tag,
                    false,
                    &inputs,
                )
                .is_ok()
            });
        let allow_nonextending_qc = block_sync_selected_apply_allow_nonextending_qc(
            selection_commit_qc_present,
            incoming_qc_validated_by_roster,
            incoming_qc_usable,
        );
        info!(
            hash = ?block_hash,
            height = block_height,
            block_signers = block_signer_count,
            candidate_qc_present,
            candidate_qc_signers,
            incoming_qc_signers,
            "applying block sync update"
        );
        let quorum_only_same_height_frontier_conflict =
            block_sync_selected_apply_same_height_frontier_conflict(
                block_quorum_met,
                incoming_qc_usable,
                commit_cert_present,
                checkpoint_present,
                self.local_conflicting_frontier_vote(block_height, block_hash)
                    .is_some(),
            );
        // Raw block-signature quorum is enough to hydrate the payload locally and keep stale-view
        // catch-up moving, but it is not authoritative enough to steal same-height frontier
        // ownership from a branch that this validator already voted on. Only certified evidence
        // may bypass the passive retained branch path in that exact conflict case.
        let allow_frontier_owner_preserve_on_payload_mismatch =
            block_sync_selected_apply_preserve_on_payload_mismatch(
                incoming_qc_usable,
                commit_cert_present,
                checkpoint_present,
            );
        let allow_authoritative_frontier_owner_supersede =
            block_sync_selected_apply_authoritative_supersede(
                incoming_qc_usable,
                commit_cert_present,
                checkpoint_present,
                block_quorum_met,
                quorum_only_same_height_frontier_conflict,
            );
        let recovery_mode = block_sync_selected_apply_recovery_mode(
            has_commit_votes,
            incoming_qc_usable,
            commit_cert_present,
            checkpoint_present,
            incoming_qc.as_ref().map(|qc| qc.epoch),
            expected_epoch,
            allow_authoritative_frontier_owner_supersede,
        );
        let created = super::message::BlockCreated {
            block,
            frontier: None,
        };
        let block_apply_start = Instant::now();
        let creation_result = self.handle_block_created_from_block_sync(
            created,
            sender.clone(),
            allow_frontier_owner_preserve_on_payload_mismatch,
            recovery_mode,
        );
        let block_apply_ms =
            u64::try_from(block_apply_start.elapsed().as_millis()).unwrap_or(u64::MAX);
        let block_known_after_creation = self.block_known_locally(block_hash);
        let creation_ok = creation_result.is_ok();
        let missing_commit_qc_repair_active = creation_ok
            && block_known_after_creation
            && signature_quorum_met
            && exact_contiguous_frontier
            && !qc_evidence_present
            && !commit_cert_present
            && !checkpoint_present
            && self.missing_commit_qc_repair_active_for_round(
                block_hash,
                block_height,
                block_view,
                local_height,
                Instant::now(),
            );
        let signed_quorum_commit_repair_active =
            block_sync_selected_apply_signed_quorum_commit_repair_active(
                BlockSyncSelectedApplySignedQuorumRepair {
                    creation_ok,
                    block_known_after_creation,
                    signature_quorum_met,
                    exact_contiguous_frontier,
                    qc_evidence_present,
                    commit_cert_present,
                    checkpoint_present,
                    missing_commit_qc_repair_active,
                },
            );
        if signed_quorum_commit_repair_active {
            if let Some(pending) = self.pending.pending_blocks.get_mut(&block_hash)
                && block_sync_selected_apply_pending_commit_qc_observed(
                    signed_quorum_commit_repair_active,
                    pending.height == block_height
                        && pending.view == block_view
                        && pending.validation_status != ValidationStatus::Invalid,
                )
            {
                // A canonical committed peer may no longer have a portable commit QC, but its
                // committed block still carries a verified commit-signature quorum. Treat that
                // quorum as the local commit evidence needed to run the finalization path.
                pending.note_commit_qc_observed(expected_epoch);
            }
            self.note_frontier_commit_qc_observed(
                block_hash,
                block_height,
                block_view,
                Instant::now(),
            );
            self.clear_missing_commit_qc_request(&block_hash, MissingBlockClearReason::Obsolete);
            self.request_commit_pipeline_for_pending(
                block_hash,
                super::status::RoundEventCauseTrace::BlockSyncUpdated,
                None,
            );
            info!(
                height = block_height,
                view = block_view,
                block = %block_hash,
                block_signers = block_signer_count,
                "accepted signed-quorum block sync fallback as commit evidence"
            );
        }
        let recovered_sparse_next_height_payload =
            block_sync_selected_apply_sparse_next_height_payload_recovered(
                BlockSyncSelectedApplySparseRecovery {
                    block_known_before: block_known,
                    block_known_after_creation,
                    next_height: block_height == local_height.saturating_add(1),
                    block_signer_count,
                    commit_quorum,
                    incoming_qc_usable,
                    commit_cert_present,
                    checkpoint_present,
                },
            );
        if recovered_sparse_next_height_payload {
            let recovery_targets = self.known_block_commit_qc_recovery_targets(
                block_hash,
                block_height,
                block_view,
                topology.as_ref(),
            );
            let _ = self.maybe_request_known_block_commit_qc_recovery(
                block_hash,
                block_height,
                block_view,
                &recovery_targets,
                None,
                "block_sync_update_sparse_next_height_payload",
            );
        }
        let ready_for_qc = block_sync_ready_for_qc(block_known_after_creation, &creation_result);
        if block_known_after_creation {
            if let Some((cert, checkpoint, stake_snapshot)) = commit_roster_record.as_ref() {
                self.state
                    .record_commit_roster(cert, checkpoint, stake_snapshot.clone());
            }
        }
        if block_sync_selected_apply_payload_unapplied_drop(ready_for_qc) {
            if let Err(err) = &creation_result {
                warn!(
                    ?err,
                    height = block_height,
                    view = block_view,
                    block = %block_hash,
                    "dropping block sync update: failed to apply block payload"
                );
            } else {
                warn!(
                    height = block_height,
                    view = block_view,
                    block = %block_hash,
                    "dropping block sync update: block not accepted locally"
                );
            }
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::BlockSyncUpdate,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::PayloadUnapplied,
            );
        }
        let commit_votes_post_start = Instant::now();
        process_commit_votes(self);
        let commit_votes_post_ms =
            u64::try_from(commit_votes_post_start.elapsed().as_millis()).unwrap_or(u64::MAX);

        let qc_to_apply =
            if block_sync_selected_apply_qc_to_apply(ready_for_qc, incoming_qc.is_some()) {
                incoming_qc.take()
            } else {
                None
            };

        let mut qc_apply_tally_ms = 0;
        let mut qc_apply_process_ms = 0;
        let mut qc_apply_commit_ms = 0;
        let qc_apply_start = Instant::now();
        let qc_apply_result = block_sync_apply_qc_after_block(
            creation_result,
            block_known_after_creation,
            qc_to_apply,
            |qc| {
                if block_sync_selected_qc_prefilter_topology_recovery(topology.as_ref().is_empty())
                {
                    let _ = self.handle_roster_unavailable_recovery(
                        block_height,
                        block_view,
                        Some(block_hash),
                        self.queue.queued_len(),
                        Instant::now(),
                        super::ProposalDeferWarningKind::EmptyCommitTopologyProposal,
                        "block_sync_update_qc_empty_commit_topology",
                    );
                    warn!(
                        height = block_height,
                        view = block_view,
                        "dropping block sync QC: empty commit topology"
                    );
                    return Ok(());
                }
                let qc_hash_matches = qc.subject_block_hash == block_hash;
                if block_sync_selected_qc_prefilter_hash_mismatch(qc_hash_matches) {
                    warn!(
                        incoming_hash = %block_hash,
                        qc_hash = %qc.subject_block_hash,
                        "ignoring block sync QC that does not match block hash"
                    );
                    return Ok(());
                }
                let qc_height_matches = qc.height == block_height;
                if block_sync_selected_qc_prefilter_height_mismatch(qc_height_matches) {
                    warn!(
                        incoming_hash = %block_hash,
                        height = block_height,
                        qc_height = qc.height,
                        "ignoring block sync QC that does not match block height"
                    );
                    return Ok(());
                }
                let expected_epoch = self.epoch_for_height(block_height);
                let qc_epoch_matches = qc.epoch == expected_epoch;
                if block_sync_selected_qc_prefilter_epoch_mismatch(qc_epoch_matches) {
                    warn!(
                        incoming_hash = %block_hash,
                        height = block_height,
                        expected_epoch,
                        qc_epoch = qc.epoch,
                        "ignoring block sync QC with mismatched epoch"
                    );
                    return Ok(());
                }
                let qc_commit_phase = matches!(qc.phase, crate::sumeragi::consensus::Phase::Commit);
                if block_sync_selected_qc_prefilter_phase_mismatch(qc_commit_phase) {
                    warn!(
                        incoming_hash = %block_hash,
                        phase = ?qc.phase,
                        "ignoring block sync QC with non-precommit phase"
                    );
                    return Ok(());
                }
                if let Some(lock) = self.locked_qc {
                    let same_height_conflict = Self::block_sync_qc_same_height_conflict(lock, &qc);
                    let same_height_recoverable = same_height_conflict
                        && Self::block_sync_qc_same_height_recoverable(
                            lock,
                            &qc,
                            allow_nonextending_qc,
                        );
                    if block_sync_selected_qc_prefilter_same_height_locked_drop(
                        same_height_conflict,
                        same_height_recoverable,
                    ) {
                        crate::sumeragi::status::inc_block_sync_locked_qc_prefilter_drop();
                        self.log_block_sync_locked_qc_conflict(
                            &qc,
                            lock,
                            "block_sync_update.height_conflict",
                        );
                        self.record_consensus_message_handling(
                            super::status::ConsensusMessageKind::Qc,
                            super::status::ConsensusMessageOutcome::Dropped,
                            super::status::ConsensusMessageReason::LockedQc,
                        );
                        return Ok(());
                    }
                }
                if block_sync_selected_qc_prefilter_stale_locked_drop(
                    self.block_sync_qc_is_stale_against_lock(&qc),
                ) {
                    debug!(
                        height = qc.height,
                        view = qc.view,
                        incoming_hash = %qc.subject_block_hash,
                        locked_height = self.locked_qc.map(|lock| lock.height),
                        "dropping stale block sync QC below locked height"
                    );
                    self.record_consensus_message_handling(
                        super::status::ConsensusMessageKind::Qc,
                        super::status::ConsensusMessageOutcome::Dropped,
                        super::status::ConsensusMessageReason::LockedQc,
                    );
                    return Ok(());
                }
                let extends_locked = self.block_sync_qc_extends_locked_chain(&qc);
                let nonextending_needs_resolution =
                    block_sync_selected_qc_prefilter_nonextending_needs_resolution(
                        extends_locked,
                        allow_nonextending_qc,
                    );
                if nonextending_needs_resolution {
                    let deferred_missing_locked_payload = self
                        .defer_block_sync_qc_while_locked_payload_missing(
                            &qc,
                            "block_sync_update.non_extending.missing_locked_payload",
                        );
                    if block_sync_selected_qc_prefilter_nonextending_defer(
                        nonextending_needs_resolution,
                        deferred_missing_locked_payload,
                    ) {
                        return Ok(());
                    }
                    if block_sync_selected_qc_prefilter_nonextending_locked_drop(
                        nonextending_needs_resolution,
                        deferred_missing_locked_payload,
                    ) {
                        if self.block_sync_qc_is_stale_against_lock(&qc) {
                            debug!(
                                height = qc.height,
                                view = qc.view,
                                incoming_hash = %qc.subject_block_hash,
                                locked_height = self.locked_qc.map(|lock| lock.height),
                                "dropping stale block sync QC below locked height"
                            );
                        } else if let Some(lock) = self.locked_qc {
                            self.log_block_sync_locked_qc_conflict(
                                &qc,
                                lock,
                                "block_sync_update.non_extending",
                            );
                        } else {
                            info!(
                                height = qc.height,
                                view = qc.view,
                                incoming_hash = %qc.subject_block_hash,
                                "dropping block sync QC that does not extend locked chain"
                            );
                        }
                        self.record_consensus_message_handling(
                            super::status::ConsensusMessageKind::Qc,
                            super::status::ConsensusMessageOutcome::Dropped,
                            super::status::ConsensusMessageReason::LockedQc,
                        );
                        return Ok(());
                    }
                }
                if block_sync_selected_qc_prefilter_retain_nonextending(
                    extends_locked,
                    allow_nonextending_qc,
                ) {
                    debug!(
                        height = qc.height,
                        view = qc.view,
                        incoming_hash = %qc.subject_block_hash,
                        "retaining non-extending block sync QC for lock realignment"
                    );
                }
                let qc_signers = qc_signer_count(&qc);
                let tally_start = Instant::now();
                let cached_tally = self.qc_signer_tally.get(&Self::qc_tally_key(&qc)).cloned();
                let tally_result =
                    match block_sync_selected_qc_process_tally_source(cached_tally.is_some()) {
                        BlockSyncSelectedQcProcessTallySource::Cached => {
                            Ok(cached_tally.expect("cached tally checked as present"))
                        }
                        BlockSyncSelectedQcProcessTallySource::Fresh => {
                            let world_view = self.state.world_view();
                            tally_qc_against_block_signers(
                                &qc,
                                &topology,
                                &world_view,
                                &block_signers,
                                block_view,
                                &self.roster_validation_cache.pops,
                                &self.common_config.chain,
                                consensus_mode,
                                stake_snapshot.as_ref(),
                                mode_tag,
                                prf_seed,
                                None,
                            )
                        }
                    };
                qc_apply_tally_ms =
                    u64::try_from(tally_start.elapsed().as_millis()).unwrap_or(u64::MAX);
                match tally_result {
                    Ok(tally) => {
                        crate::sumeragi::status::record_precommit_signers(
                            crate::sumeragi::status::PrecommitSignerRecord {
                                block_hash,
                                height: qc.height,
                                view: qc.view,
                                epoch: qc.epoch,
                                chain_order_hash: qc.chain_order_hash,
                                rechain_seq: qc.rechain_seq,
                                parent_state_root: qc.parent_state_root,
                                post_state_root: qc.post_state_root,
                                signers: tally.voting_signers.clone(),
                                bls_aggregate_signature: qc
                                    .aggregate
                                    .bls_aggregate_signature
                                    .clone(),
                                roster_len: topology.as_ref().len(),
                                mode_tag: mode_tag.to_string(),
                                validator_set: topology.as_ref().to_vec(),
                                stake_snapshot: stake_snapshot.clone(),
                            },
                        );
                        self.note_validated_qc_tally(&qc, tally.clone());
                        let pending_block_valid = self
                            .pending
                            .pending_blocks
                            .get(&block_hash)
                            .is_some_and(|pending| {
                                !pending.is_retry_aborted()
                                    && pending.validation_status == ValidationStatus::Valid
                            });
                        let inflight_block_active = self
                            .subsystems
                            .commit
                            .inflight
                            .as_ref()
                            .is_some_and(|inflight| {
                                inflight.block_hash == block_hash && !inflight.pending.aborted
                            });
                        let kura_block_known =
                            self.kura.get_block_height_by_hash(block_hash).is_some();
                        let block_known_for_commit =
                            block_sync_selected_qc_process_block_known_for_commit(
                                pending_block_valid,
                                inflight_block_active,
                                kura_block_known,
                            );
                        let process_start = Instant::now();
                        let process_ok = self.process_precommit_qc(
                            &qc,
                            block_known_for_commit,
                            allow_nonextending_qc,
                        );
                        qc_apply_process_ms =
                            u64::try_from(process_start.elapsed().as_millis()).unwrap_or(u64::MAX);
                        let commit_qc_accepted =
                            block_sync_selected_qc_process_commit_qc_accepted(process_ok);
                        if !commit_qc_accepted {
                            if self.block_sync_qc_is_stale_against_lock(&qc) {
                                debug!(
                                    height = qc.height,
                                    view = qc.view,
                                    incoming_hash = %qc.subject_block_hash,
                                    locked_height = self.locked_qc.map(|lock| lock.height),
                                    "dropping stale block sync QC below locked height"
                                );
                            } else if let Some(lock) = self.locked_qc {
                                self.log_block_sync_locked_qc_conflict(
                                    &qc,
                                    lock,
                                    "block_sync_update.precommit_reject",
                                );
                            }
                            return Ok(());
                        }
                        self.qc_cache.insert(Self::qc_tally_key(&qc), qc.clone());
                        if block_known_for_commit {
                            super::status::record_commit_qc(qc.clone());
                        } else {
                            let sent = self.request_certified_block_for_qc(
                                &qc,
                                &topology,
                                &tally.voting_signers,
                                "block_sync_update_qc_missing_commit_ready_payload",
                            );
                            info!(
                                incoming_hash = %block_hash,
                                height = block_height,
                                view = block_view,
                                targets = sent,
                                "cached block sync QC and requested certified block before publishing commit status"
                            );
                        }
                        #[cfg(feature = "telemetry")]
                        if let Some(telemetry) = self.telemetry_handle() {
                            telemetry.note_qc_signer_counts(
                                "precommit",
                                tally.present_signers,
                                tally.voting_signers.len(),
                            );
                        }
                        debug!(
                            incoming_hash = %block_hash,
                            signers = tally.voting_signers.len(),
                            qc_signers,
                            "applied block sync QC after validation"
                        );
                        let apply_commit_qc_now = block_sync_selected_qc_process_apply_commit_qc(
                            commit_qc_accepted,
                            block_known_for_commit,
                        );
                        if apply_commit_qc_now {
                            let commit_start = Instant::now();
                            self.apply_commit_qc(
                                &qc,
                                topology.as_ref(),
                                block_hash,
                                block_height,
                                block_view,
                            );
                            if block_sync_selected_qc_process_clean_rbc_sessions(
                                apply_commit_qc_now,
                                self.runtime_da_enabled(),
                            ) {
                                self.clean_rbc_sessions_for_committed_block_if_settled(
                                    block_hash,
                                    block_height,
                                );
                            }
                            qc_apply_commit_ms = u64::try_from(commit_start.elapsed().as_millis())
                                .unwrap_or(u64::MAX);
                            self.request_commit_pipeline_for_round(
                                block_height,
                                block_view,
                                super::status::RoundPhaseTrace::WaitCommitQc,
                                super::status::RoundEventCauseTrace::BlockSyncUpdated,
                                None,
                            );
                        } else {
                            if let Some(pending) = self.pending.pending_blocks.get_mut(&block_hash)
                                && block_sync_selected_qc_process_observe_pending_epoch(
                                    commit_qc_accepted,
                                    block_known_for_commit,
                                    true,
                                )
                            {
                                pending.note_commit_qc_observed(qc.epoch);
                            }
                            debug!(
                                incoming_hash = %block_hash,
                                height = block_height,
                                view = block_view,
                                "deferring commit apply for block sync QC until block is validated"
                            );
                        }
                    }
                    Err(err) => {
                        record_qc_validation_error(self.telemetry_handle(), &err);
                        warn!(
                            ?err,
                            reason = qc_validation_reason(&err),
                            incoming_hash = %block_hash,
                            height = block_height,
                            view = block_view,
                            qc_signers,
                            block_signers = block_signer_count,
                            "dropping block sync QC after validation failure"
                        );
                    }
                }
                Ok(())
            },
        );
        let qc_apply_ms = u64::try_from(qc_apply_start.elapsed().as_millis()).unwrap_or(u64::MAX);
        qc_apply_result?;
        if block_sync_selected_qc_process_cache_unknown_block_qc(
            creation_ok,
            block_known_after_creation,
            incoming_qc.is_some(),
        ) {
            if let Some(qc) = incoming_qc.take() {
                // Cache the QC so we can reuse it once the block becomes available locally.
                self.cache_block_sync_qc_for_unknown_block(
                    qc,
                    block_hash,
                    block_height,
                    block_view,
                    &topology,
                    &block_signers,
                    allow_nonextending_qc,
                    consensus_mode,
                    stake_snapshot.clone(),
                    mode_tag,
                    prf_seed,
                );
            }
        }

        debug!(
            height = block_height,
            view = block_view,
            block = %block_hash,
            kura_committed_ms,
            kura_known_ms,
            roster_validate_ms,
            roster_persisted_ms,
            roster_select_ms,
            signature_verify_ms,
            commit_votes_pre_ms,
            commit_votes_post_ms,
            qc_candidate_ms,
            qc_validate_ms,
            qc_fallback_ms,
            block_apply_ms,
            qc_apply_ms,
            qc_apply_tally_ms,
            qc_apply_process_ms,
            qc_apply_commit_ms,
            "block sync update substep timings"
        );

        Ok(())
    }

    /// Cache a validated precommit QC from block sync when the block payload is not ready yet.
    #[allow(clippy::too_many_arguments)]
    #[allow(clippy::too_many_lines)]
    #[allow(clippy::needless_pass_by_value)]
    pub(super) fn cache_block_sync_qc_for_unknown_block(
        &mut self,
        qc: crate::sumeragi::consensus::Qc,
        block_hash: HashOf<BlockHeader>,
        block_height: u64,
        block_view: u64,
        topology: &super::network_topology::Topology,
        block_signers: &BTreeSet<crate::sumeragi::consensus::ValidatorIndex>,
        allow_nonextending_qc: bool,
        consensus_mode: ConsensusMode,
        stake_snapshot: Option<super::stake_snapshot::CommitStakeSnapshot>,
        mode_tag: &str,
        prf_seed: Option<[u8; 32]>,
    ) {
        if block_sync_selected_qc_prefilter_topology_recovery(topology.as_ref().is_empty()) {
            let _ = self.handle_roster_unavailable_recovery(
                block_height,
                block_view,
                Some(block_hash),
                self.queue.queued_len(),
                Instant::now(),
                super::ProposalDeferWarningKind::EmptyCommitTopologyProposal,
                "cache_block_sync_qc_empty_commit_topology",
            );
            warn!(
                height = block_height,
                view = block_view,
                "dropping cached block sync QC: empty commit topology"
            );
            return;
        }
        let qc_hash_matches = qc.subject_block_hash == block_hash;
        if block_sync_selected_qc_prefilter_hash_mismatch(qc_hash_matches) {
            warn!(
                incoming_hash = %block_hash,
                qc_hash = %qc.subject_block_hash,
                "ignoring cached block sync QC that does not match block hash"
            );
            return;
        }
        let qc_height_matches = qc.height == block_height;
        if block_sync_selected_qc_prefilter_height_mismatch(qc_height_matches) {
            warn!(
                incoming_hash = %block_hash,
                height = block_height,
                qc_height = qc.height,
                "ignoring cached block sync QC that does not match block height"
            );
            return;
        }
        let expected_epoch = self.epoch_for_height(block_height);
        let qc_epoch_matches = qc.epoch == expected_epoch;
        if block_sync_selected_qc_prefilter_epoch_mismatch(qc_epoch_matches) {
            warn!(
                incoming_hash = %block_hash,
                height = block_height,
                expected_epoch,
                qc_epoch = qc.epoch,
                "ignoring cached block sync QC with mismatched epoch"
            );
            return;
        }
        let qc_commit_phase = matches!(qc.phase, crate::sumeragi::consensus::Phase::Commit);
        if block_sync_selected_qc_prefilter_phase_mismatch(qc_commit_phase) {
            warn!(
                incoming_hash = %block_hash,
                phase = ?qc.phase,
                "ignoring cached block sync QC with non-precommit phase"
            );
            return;
        }
        let qc_ref = crate::sumeragi::consensus::QcHeaderRef {
            phase: qc.phase,
            subject_block_hash: qc.subject_block_hash,
            height: qc.height,
            view: qc.view,
            epoch: qc.epoch,
        };
        if let Some(lock) = self.locked_qc {
            let same_height_conflict = Self::block_sync_qc_same_height_conflict(lock, &qc);
            let same_height_recoverable = same_height_conflict
                && Self::block_sync_qc_same_height_recoverable(lock, &qc, allow_nonextending_qc);
            if block_sync_selected_qc_prefilter_same_height_locked_drop(
                same_height_conflict,
                same_height_recoverable,
            ) {
                crate::sumeragi::status::inc_block_sync_locked_qc_prefilter_drop();
                self.log_block_sync_locked_qc_conflict(
                    &qc,
                    lock,
                    "cached_block_sync_qc.height_conflict",
                );
                self.record_consensus_message_handling(
                    super::status::ConsensusMessageKind::Qc,
                    super::status::ConsensusMessageOutcome::Dropped,
                    super::status::ConsensusMessageReason::LockedQc,
                );
                return;
            }
        }
        if block_sync_selected_qc_prefilter_stale_locked_drop(
            self.block_sync_qc_is_stale_against_lock(&qc),
        ) {
            debug!(
                height = qc.height,
                view = qc.view,
                incoming_hash = %qc.subject_block_hash,
                locked_height = self.locked_qc.map(|lock| lock.height),
                "dropping stale cached block sync QC below locked height"
            );
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::Qc,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::LockedQc,
            );
            return;
        }
        let extends_locked = self.block_sync_qc_extends_locked_chain(&qc);
        let nonextending_needs_resolution =
            block_sync_selected_qc_prefilter_nonextending_needs_resolution(
                extends_locked,
                allow_nonextending_qc,
            );
        if nonextending_needs_resolution {
            let deferred_missing_locked_payload = self
                .defer_block_sync_qc_while_locked_payload_missing(
                    &qc,
                    "cached_block_sync_qc.non_extending.missing_locked_payload",
                );
            if block_sync_selected_qc_prefilter_nonextending_defer(
                nonextending_needs_resolution,
                deferred_missing_locked_payload,
            ) {
                return;
            }
            if block_sync_selected_qc_prefilter_nonextending_locked_drop(
                nonextending_needs_resolution,
                deferred_missing_locked_payload,
            ) {
                if self.block_sync_qc_is_stale_against_lock(&qc) {
                    debug!(
                        height = qc.height,
                        view = qc.view,
                        incoming_hash = %qc.subject_block_hash,
                        locked_height = self.locked_qc.map(|lock| lock.height),
                        "dropping stale block sync QC below locked height"
                    );
                } else if let Some(lock) = self.locked_qc {
                    self.log_block_sync_locked_qc_conflict(
                        &qc,
                        lock,
                        "cached_block_sync_qc.non_extending",
                    );
                }
                self.record_consensus_message_handling(
                    super::status::ConsensusMessageKind::Qc,
                    super::status::ConsensusMessageOutcome::Dropped,
                    super::status::ConsensusMessageReason::LockedQc,
                );
                return;
            }
        }
        if block_sync_selected_qc_prefilter_retain_nonextending(
            extends_locked,
            allow_nonextending_qc,
        ) {
            debug!(
                height = qc.height,
                view = qc.view,
                incoming_hash = %qc.subject_block_hash,
                "retaining cached non-extending block sync QC for lock realignment"
            );
        }
        let qc_signers = qc_signer_count(&qc);
        let tally_result = {
            let world_view = self.state.world_view();
            tally_qc_against_block_signers(
                &qc,
                topology,
                &world_view,
                block_signers,
                block_view,
                &self.roster_validation_cache.pops,
                &self.common_config.chain,
                consensus_mode,
                stake_snapshot.as_ref(),
                mode_tag,
                prf_seed,
                None,
            )
        };
        match tally_result {
            Ok(tally) => {
                crate::sumeragi::status::record_precommit_signers(
                    crate::sumeragi::status::PrecommitSignerRecord {
                        block_hash,
                        height: qc.height,
                        view: qc.view,
                        epoch: qc.epoch,
                        chain_order_hash: qc.chain_order_hash,
                        rechain_seq: qc.rechain_seq,
                        parent_state_root: qc.parent_state_root,
                        post_state_root: qc.post_state_root,
                        signers: tally.voting_signers.clone(),
                        bls_aggregate_signature: qc.aggregate.bls_aggregate_signature.clone(),
                        roster_len: topology.as_ref().len(),
                        mode_tag: mode_tag.to_string(),
                        validator_set: topology.as_ref().to_vec(),
                        stake_snapshot: stake_snapshot.clone(),
                    },
                );
                self.note_validated_qc_tally(&qc, tally.clone());
                let process_ok = self.process_precommit_qc(&qc, false, allow_nonextending_qc);
                let commit_qc_accepted =
                    block_sync_selected_qc_process_commit_qc_accepted(process_ok);
                if !commit_qc_accepted {
                    if self.block_sync_qc_is_stale_against_lock(&qc) {
                        debug!(
                            height = qc.height,
                            view = qc.view,
                            incoming_hash = %qc.subject_block_hash,
                            locked_height = self.locked_qc.map(|lock| lock.height),
                            "dropping stale block sync QC below locked height"
                        );
                    } else if let Some(lock) = self.locked_qc {
                        if Self::block_sync_qc_same_height_conflict(lock, &qc) {
                            crate::sumeragi::status::inc_block_sync_locked_qc_prefilter_drop();
                        }
                        self.log_block_sync_locked_qc_conflict(
                            &qc,
                            lock,
                            "cached_block_sync_qc.precommit_reject",
                        );
                    }
                    return;
                }
                let incoming_newer_than_lock = self
                    .locked_qc
                    .is_none_or(|lock| (qc.height, qc.view) > (lock.height, lock.view));
                if block_sync_selected_qc_cache_update_locked_qc(
                    allow_nonextending_qc,
                    incoming_newer_than_lock,
                ) {
                    super::status::set_locked_qc(qc.height, qc.view, Some(qc.subject_block_hash));
                    self.locked_qc = Some(qc_ref);
                    self.prune_precommit_votes_conflicting_with_lock(qc_ref);
                }
                self.quarantined_block_sync_qcs
                    .remove(&Self::qc_tally_key(&qc));
                let sent = self.request_certified_block_for_qc(
                    &qc,
                    topology,
                    &tally.voting_signers,
                    "cached_block_sync_qc_missing_payload",
                );
                self.qc_cache.insert(Self::qc_tally_key(&qc), qc);
                debug!(
                    incoming_hash = %block_hash,
                    signers = tally.voting_signers.len(),
                    qc_signers,
                    targets = sent,
                    "cached block sync QC and requested certified block before block payload is ready"
                );
            }
            Err(err) => {
                record_qc_validation_error(self.telemetry_handle(), &err);
                let reason = qc_validation_reason(&err);
                let missing_context_error = Self::block_sync_qc_is_missing_context_error(&err);
                if block_sync_selected_qc_cache_missing_context_quarantine(missing_context_error) {
                    self.quarantine_block_sync_qc_candidate(
                        qc.clone(),
                        reason,
                        QuarantinedQcTarget::BlockSync,
                    );
                    warn!(
                        ?err,
                        reason,
                        incoming_hash = %block_hash,
                        height = block_height,
                        view = block_view,
                        qc_signers,
                        block_signers = block_signers.len(),
                        "quarantining cached block sync QC after transient validation failure"
                    );
                } else if block_sync_selected_qc_cache_final_validation_drop(missing_context_error)
                {
                    self.block_sync_qc_final_drop(reason);
                    warn!(
                        ?err,
                        reason,
                        incoming_hash = %block_hash,
                        height = block_height,
                        view = block_view,
                        qc_signers,
                        block_signers = block_signers.len(),
                        "dropping cached block sync QC after validation failure"
                    );
                }
            }
        }
    }

    #[allow(clippy::too_many_lines)]
    #[allow(clippy::unnecessary_wraps)]
    pub(super) fn handle_fetch_pending_block(
        &mut self,
        request: super::message::FetchPendingBlock,
    ) -> Result<()> {
        let block_hash = request.block_hash;
        let peer = request.requester;
        let request_priority = request
            .priority
            .unwrap_or(FetchPendingBlockPriority::Background);
        let dedup_key = super::BlockPayloadDedupKey::FetchPendingBlock {
            height: request.height,
            view: request.view,
            block_hash,
            requester_hash: CryptoHash::new(peer.encode()),
            priority: request_priority,
            commit_qc_only: request.commit_qc_only.unwrap_or(false),
        };
        let requester_roster_proof_known = request.requester_roster_proof_known.unwrap_or(false);
        let commit_qc_only = request.commit_qc_only.unwrap_or(false);
        let request_meta = super::PendingFetchRequestMeta {
            priority: request_priority,
            requester_roster_proof_known,
            commit_qc_only,
        };
        let force_bypass_queue = false;
        let mut invalid_payload = false;

        let inflight_response = if let Some(inflight) = self
            .subsystems
            .commit
            .inflight
            .as_ref()
            .filter(|inflight| inflight.block_hash == block_hash)
        {
            if matches!(
                inflight.pending.validation_status,
                ValidationStatus::Invalid
            ) {
                debug!(
                    hash = %block_hash,
                    "skipping fetch response for invalid inflight pending block"
                );
                invalid_payload = true;
                None
            } else {
                Some(inflight.pending.block.clone())
            }
        } else {
            None
        };
        if let Some(block) = inflight_response {
            let mut requesters = self.take_pending_fetch_requesters(&block_hash);
            requesters
                .entry(peer.clone())
                .and_modify(|stored| {
                    stored.priority = stored.priority.max(request_meta.priority);
                    stored.requester_roster_proof_known |=
                        request_meta.requester_roster_proof_known;
                    stored.commit_qc_only |= request_meta.commit_qc_only;
                })
                .or_insert(request_meta);
            self.send_fetch_pending_block_responses(
                requesters,
                &block,
                force_bypass_queue,
                /*allow_highest_qc_bypass*/ true,
                /*allow_hintless_block_sync_bypass*/ false,
            );
            self.release_block_payload_dedup(&dedup_key);
            return Ok(());
        }

        let pending_response = if let Some(pending) = self.pending.pending_blocks.get(&block_hash) {
            if matches!(pending.validation_status, ValidationStatus::Invalid) {
                debug!(
                    hash = %block_hash,
                    "skipping fetch response for invalid pending block"
                );
                invalid_payload = true;
                None
            } else {
                Some(pending.block.clone())
            }
        } else {
            None
        };
        if let Some(block) = pending_response {
            let mut requesters = self.take_pending_fetch_requesters(&block_hash);
            requesters
                .entry(peer.clone())
                .and_modify(|stored| {
                    stored.priority = stored.priority.max(request_meta.priority);
                    stored.requester_roster_proof_known |=
                        request_meta.requester_roster_proof_known;
                    stored.commit_qc_only |= request_meta.commit_qc_only;
                })
                .or_insert(request_meta);
            self.send_fetch_pending_block_responses(
                requesters,
                &block,
                force_bypass_queue,
                /*allow_highest_qc_bypass*/ true,
                /*allow_hintless_block_sync_bypass*/ false,
            );
            self.release_block_payload_dedup(&dedup_key);
            return Ok(());
        }

        let deferred_response =
            self.deferred_block_sync_updates
                .iter()
                .find_map(|((height, view, hash), entry)| {
                    (*hash == block_hash)
                        .then(|| (*height, *view, Arc::new(entry.update.block.clone())))
                });
        if let Some((_, _, block)) = deferred_response {
            let mut requesters = self.take_pending_fetch_requesters(&block_hash);
            requesters
                .entry(peer.clone())
                .and_modify(|stored| {
                    stored.priority = stored.priority.max(request_meta.priority);
                    stored.requester_roster_proof_known |=
                        request_meta.requester_roster_proof_known;
                    stored.commit_qc_only |= request_meta.commit_qc_only;
                })
                .or_insert(request_meta);
            self.send_fetch_pending_block_responses(
                requesters,
                block.as_ref(),
                force_bypass_queue,
                /*allow_highest_qc_bypass*/ true,
                /*allow_hintless_block_sync_bypass*/ false,
            );
            self.release_block_payload_dedup(&dedup_key);
            return Ok(());
        }

        if let Some(height) = self.kura.get_block_height_by_hash(block_hash) {
            if let Some(block) = self.kura.get_block(height) {
                let block = block.as_ref();
                let mut requesters = self.take_pending_fetch_requesters(&block_hash);
                requesters
                    .entry(peer.clone())
                    .and_modify(|stored| {
                        stored.priority = stored.priority.max(request_meta.priority);
                        stored.requester_roster_proof_known |=
                            request_meta.requester_roster_proof_known;
                        stored.commit_qc_only |= request_meta.commit_qc_only;
                    })
                    .or_insert(request_meta);
                let response = self.build_fetch_pending_block_payload(block);
                if !commit_qc_only
                    && self.should_defer_canonical_committed_fetch_response(block, &response)
                {
                    debug!(
                        height = block.header().height().get(),
                        view = block.header().view_change_index(),
                        block = %block_hash,
                        peer = %peer,
                        "deferring exact-tip fetch response until commit proof is available"
                    );
                    self.pending
                        .pending_fetch_requests
                        .insert(block_hash, requesters);
                    self.record_consensus_message_handling(
                        super::status::ConsensusMessageKind::FetchPendingBlock,
                        super::status::ConsensusMessageOutcome::Deferred,
                        super::status::ConsensusMessageReason::NotFound,
                    );
                    self.release_block_payload_dedup(&dedup_key);
                    return Ok(());
                }
                self.send_fetch_pending_block_responses(
                    requesters,
                    block,
                    force_bypass_queue,
                    /*allow_highest_qc_bypass*/ true,
                    /*allow_hintless_block_sync_bypass*/ true,
                );
                self.release_block_payload_dedup(&dedup_key);
                return Ok(());
            }
        }

        if !invalid_payload {
            self.stash_pending_fetch_request(
                block_hash,
                peer,
                request_priority,
                requester_roster_proof_known,
                commit_qc_only,
            );
        }

        self.record_consensus_message_handling(
            super::status::ConsensusMessageKind::FetchPendingBlock,
            super::status::ConsensusMessageOutcome::Deferred,
            super::status::ConsensusMessageReason::NotFound,
        );
        self.release_block_payload_dedup(&dedup_key);
        Ok(())
    }

    #[allow(clippy::unnecessary_wraps)]
    pub(super) fn handle_fetch_block_body(
        &mut self,
        request: super::message::FetchBlockBody,
    ) -> Result<()> {
        let dedup_key = super::BlockPayloadDedupKey::FetchBlockBody {
            height: request.height,
            view: request.view,
            block_hash: request.block_hash,
            requester_hash: CryptoHash::new(request.requester.encode()),
        };
        let block_hash = request.block_hash;
        let peer = request.requester;
        let mut local_block_found = false;
        let mut local_identity_matches = false;
        if let Some(block) = self.local_signed_block_for_body_repair(block_hash) {
            local_block_found = true;
            let header = block.header();
            local_identity_matches = header.height().get() == request.height
                && header.view_change_index() == request.view;
            if local_identity_matches {
                let payload = self.build_fetch_pending_block_payload(block.as_ref());
                let should_defer =
                    self.should_defer_canonical_committed_fetch_response(block.as_ref(), &payload);
                let decision = block_sync_fetch_block_body_handle_decision(
                    local_block_found,
                    local_identity_matches,
                    should_defer,
                    false,
                    false,
                );
                if decision.pending_stash {
                    debug!(
                        height = request.height,
                        view = request.view,
                        block = %block_hash,
                        peer = %peer,
                        "deferring exact body response until commit proof is available"
                    );
                    self.stash_pending_block_body_request(block_hash, peer);
                    debug_assert_eq!(decision.dedup_release_count, 1);
                    self.release_block_payload_dedup(&dedup_key);
                    if decision.deferred_record {
                        self.record_consensus_message_handling(
                            super::status::ConsensusMessageKind::FetchBlockBody,
                            super::status::ConsensusMessageOutcome::Deferred,
                            super::status::ConsensusMessageReason::NotFound,
                        );
                    }
                    return Ok(());
                }
                debug_assert!(decision.dispatch);
                let Some(response) = Self::block_body_response_from_payload(
                    block_hash,
                    request.height,
                    request.view,
                    payload,
                ) else {
                    if decision.remove_requester {
                        self.remove_pending_block_body_requester(&block_hash, &peer);
                    }
                    debug_assert_eq!(decision.dedup_release_count, 1);
                    self.release_block_payload_dedup(&dedup_key);
                    return Ok(());
                };
                if decision.remove_requester {
                    self.remove_pending_block_body_requester(&block_hash, &peer);
                }
                if decision.dispatch_uses_plain_fallback_helper {
                    self.dispatch_block_body_response_with_plain_fallback(
                        peer,
                        block.as_ref(),
                        response,
                    );
                }
                debug_assert_eq!(decision.dedup_release_count, 1);
                self.release_block_payload_dedup(&dedup_key);
                return Ok(());
            }
        }
        let frontier_matches = self.frontier_slot.as_ref().is_some_and(|slot| {
            slot.block_hash == block_hash
                && slot.height == request.height
                && slot.view == request.view
        });
        let window_allows = self.should_stash_pending_block_body_request(request.height);
        let decision = block_sync_fetch_block_body_handle_decision(
            local_block_found,
            local_identity_matches,
            false,
            frontier_matches,
            window_allows,
        );
        debug_assert!(!decision.dispatch);
        if decision.frontier_stash
            && let Some(slot) = self.frontier_slot.as_mut()
            && slot.block_hash == block_hash
            && slot.height == request.height
            && slot.view == request.view
        {
            slot.repair_state.pending_requesters.insert(peer.clone());
        }
        if decision.pending_stash {
            debug!(
                height = request.height,
                view = request.view,
                block = %block_hash,
                peer = %peer,
                "stashing exact body requester until local payload becomes available"
            );
            self.stash_pending_block_body_request(block_hash, peer);
        }
        debug_assert_eq!(decision.dedup_release_count, 1);
        self.release_block_payload_dedup(&dedup_key);
        if decision.deferred_record {
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::FetchBlockBody,
                super::status::ConsensusMessageOutcome::Deferred,
                super::status::ConsensusMessageReason::NotFound,
            );
        }
        Ok(())
    }

    fn allow_same_height_block_body_repair(
        &self,
        response: &super::message::BlockBodyResponse,
    ) -> bool {
        let frontier_slot_exact = self.frontier_slot_is_exact_height(response.height);
        if !frontier_slot_exact {
            return same_height_block_body_repair_decision(false, false, false, false).allow;
        }
        let committed_height = self.committed_height_snapshot();
        let now = Instant::now();
        let pending_source = self
            .pending
            .missing_block_requests
            .get(&response.block_hash)
            .is_some_and(|request| {
                let phase_is_commit = request.phase == crate::sumeragi::consensus::Phase::Commit;
                let height_matches = request.height == response.height;
                let view_matches = request.view == response.view;
                let actionable_dependency = phase_is_commit
                    && height_matches
                    && view_matches
                    && self.missing_block_request_has_actionable_dependency(
                        response.block_hash,
                        request,
                        committed_height,
                        now,
                    );
                same_height_block_body_repair_source_matches(
                    true,
                    phase_is_commit,
                    true,
                    height_matches,
                    view_matches,
                    actionable_dependency,
                )
            });
        let deferred_source = !pending_source
            && self.deferred_missing_payload_qcs.values().any(|entry| {
                let phase_is_commit = entry.qc.phase == crate::sumeragi::consensus::Phase::Commit;
                let block_hash_matches = entry.qc.subject_block_hash == response.block_hash;
                let height_matches = entry.qc.height == response.height;
                let view_matches = entry.qc.view == response.view;
                let actionable_dependency = phase_is_commit
                    && block_hash_matches
                    && height_matches
                    && view_matches
                    && self.deferred_missing_payload_qc_has_actionable_dependency(
                        entry,
                        committed_height,
                        now,
                    );
                same_height_block_body_repair_source_matches(
                    true,
                    phase_is_commit,
                    block_hash_matches,
                    height_matches,
                    view_matches,
                    actionable_dependency,
                )
            });
        let active_commit_qc_repair = !pending_source
            && !deferred_source
            && self.missing_commit_qc_repair_active_for_round(
                response.block_hash,
                response.height,
                response.view,
                committed_height,
                now,
            );
        same_height_block_body_repair_decision(
            frontier_slot_exact,
            pending_source,
            deferred_source,
            active_commit_qc_repair,
        )
        .allow
    }

    fn block_body_response_payload_identity(
        response: &super::message::BlockBodyResponse,
    ) -> BlockBodyResponsePayloadIdentity {
        let block = match &response.body {
            super::message::BlockBodyData::BlockCreated(created) => &created.block,
            super::message::BlockBodyData::BlockSyncUpdate(update) => &update.block,
        };
        let header = block.header();
        let payload_hash = Hash::new(super::proposals::block_payload_bytes(block));
        BlockBodyResponsePayloadIdentity {
            block_hash: block.hash(),
            height: header.height().get(),
            view: header.view_change_index(),
            payload_hash,
        }
    }

    fn allow_rbc_session_block_body_repair(
        &self,
        response: &super::message::BlockBodyResponse,
    ) -> bool {
        let runtime_da_enabled = self.runtime_da_enabled();
        let frontier_slot_exact = self.frontier_slot_is_exact_height(response.height);
        let key = Actor::session_key(&response.block_hash, response.height, response.view);
        let session = self.subsystems.da_rbc.rbc.sessions.get(&key);
        let session_metadata_matches = session
            .is_some_and(|session| self.rbc_session_metadata_matches_progress_slot(key, session));
        let session_has_authoritative_payload = session.is_some_and(|session| {
            self.rbc_session_has_verified_or_local_payload_for_progress(key, session)
        });
        let expected_payload_hash = session.and_then(|session| session.payload_hash());
        let body_identity = Self::block_body_response_payload_identity(response);

        block_body_repair_gate_decision(
            runtime_da_enabled,
            frontier_slot_exact,
            session.is_some(),
            session_metadata_matches,
            session_has_authoritative_payload,
            expected_payload_hash,
            response.block_hash,
            response.height,
            response.view,
            body_identity,
        )
        .allow
    }

    fn observed_commit_qc_epoch_for_body_repair(
        &self,
        block_hash: HashOf<BlockHeader>,
        height: u64,
        view: u64,
    ) -> Option<u64> {
        let cache_epoch = self
            .cached_commit_qc_for_block(block_hash, height, view)
            .map(|qc| qc.epoch);
        let deferred_epoch = cache_epoch.is_none().then(|| {
            self.deferred_missing_payload_qcs
                .values()
                .find_map(|entry| {
                    block_body_repair_epoch_deferred_source(
                        true,
                        matches!(entry.qc.phase, crate::sumeragi::consensus::Phase::Commit),
                        entry.qc.subject_block_hash == block_hash,
                        entry.qc.height == height,
                        entry.qc.view == view,
                        entry.qc.epoch,
                    )
                })
        });
        let deferred_epoch = deferred_epoch.flatten();
        let pending_epoch = if cache_epoch.is_none() && deferred_epoch.is_none() {
            self.pending
                .pending_blocks
                .get(&block_hash)
                .and_then(|pending| {
                    block_body_repair_epoch_pending_source(
                        true,
                        pending.commit_qc_observed(),
                        pending.commit_qc_epoch,
                    )
                })
        } else {
            None
        };
        block_body_repair_epoch_decision(cache_epoch, deferred_epoch, pending_epoch).epoch
    }

    #[allow(clippy::unnecessary_wraps)]
    pub(super) fn handle_block_body_response(
        &mut self,
        response: super::message::BlockBodyResponse,
        sender: Option<PeerId>,
    ) -> Result<()> {
        let dedup_key = super::BlockPayloadDedupKey::BlockBodyResponse {
            height: response.height,
            view: response.view,
            block_hash: response.block_hash,
            evidence_hash: super::block_body_response_evidence_hash(&response),
        };
        let mut detached_commit_qc = self.direct_commit_qc_from_block_body_response(&response);
        let response_has_commit_evidence = detached_commit_qc.is_some()
            || matches!(
                &response.body,
                super::message::BlockBodyData::BlockSyncUpdate(update)
                    if !update.commit_votes.is_empty()
            );
        if !self.frontier_slot_is_exact_height(response.height) {
            if let super::message::BlockBodyData::BlockSyncUpdate(update) = response.body {
                let header = update.block.header();
                if update.block.hash() != response.block_hash
                    || header.height().get() != response.height
                    || header.view_change_index() != response.view
                {
                    self.record_consensus_message_handling(
                        super::status::ConsensusMessageKind::BlockBodyResponse,
                        super::status::ConsensusMessageOutcome::Dropped,
                        super::status::ConsensusMessageReason::InvalidPayload,
                    );
                    self.release_block_payload_dedup(&dedup_key);
                    return Ok(());
                }
                if update.commit_qc.is_some() || update.validator_checkpoint.is_some() {
                    debug!(
                        height = response.height,
                        view = response.view,
                        block = %response.block_hash,
                        committed_height = self.committed_height_snapshot(),
                        "routing non-exact QC-bearing BlockBodyResponse through block-sync update path"
                    );
                    self.release_block_payload_dedup(&dedup_key);
                    return self.handle_block_sync_update(update, sender);
                }
            }
            self.handle_detached_block_body_commit_qc(
                detached_commit_qc,
                response.block_hash,
                response.height,
                response.view,
                "non_frontier_height",
            );
            self.release_block_payload_dedup(&dedup_key);
            return Ok(());
        }
        let slot_matches = self.frontier_slot.as_ref().is_some_and(|slot| {
            slot.block_hash == response.block_hash
                && slot.height == response.height
                && slot.view == response.view
        });
        let allow_rbc_body_repair =
            !slot_matches && self.allow_rbc_session_block_body_repair(&response);
        let allow_same_height_repair = !slot_matches
            && (allow_rbc_body_repair || self.allow_same_height_block_body_repair(&response));
        if !slot_matches && !allow_same_height_repair {
            self.handle_detached_block_body_commit_qc(
                detached_commit_qc,
                response.block_hash,
                response.height,
                response.view,
                "body_not_requested",
            );
            self.release_block_payload_dedup(&dedup_key);
            return Ok(());
        }
        let sender_for_slot = sender
            .clone()
            .filter(|peer| peer != self.common_config.peer.id());
        let plain_body_response = matches!(
            &response.body,
            super::message::BlockBodyData::BlockCreated(_)
        );
        let repairs_missing_highest_qc_dependency = self
            .subsystems
            .propose
            .highest_qc_missing_defer_markers
            .iter()
            .any(|(_, _, hash)| *hash == response.block_hash);
        if allow_same_height_repair {
            info!(
                height = response.height,
                view = response.view,
                block = %response.block_hash,
                active_frontier = ?self
                    .frontier_slot
                    .as_ref()
                    .map(|slot| (slot.height, slot.view, slot.block_hash)),
                allow_rbc_body_repair,
                "accepting BlockBodyResponse for exact-height same-slot repair after frontier ownership moved"
            );
        }
        let result = match response.body {
            super::message::BlockBodyData::BlockCreated(block_created) => {
                let header = block_created.block.header();
                if block_created.block.hash() != response.block_hash
                    || header.height().get() != response.height
                    || header.view_change_index() != response.view
                {
                    self.record_consensus_message_handling(
                        super::status::ConsensusMessageKind::BlockBodyResponse,
                        super::status::ConsensusMessageOutcome::Dropped,
                        super::status::ConsensusMessageReason::InvalidPayload,
                    );
                    self.release_block_payload_dedup(&dedup_key);
                    return Ok(());
                }
                if allow_same_height_repair {
                    let observed_commit_qc_epoch = self.observed_commit_qc_epoch_for_body_repair(
                        response.block_hash,
                        response.height,
                        response.view,
                    );
                    self.handle_block_created_from_block_sync(
                        block_created,
                        sender,
                        /*allow_frontier_owner_preserve_on_payload_mismatch*/ true,
                        if observed_commit_qc_epoch.is_some() {
                            BlockSyncRecoveryMode::CommitEvidenceRepair {
                                observed_commit_qc_epoch,
                                allow_aborted_revival_without_local_commit_qc: true,
                            }
                        } else {
                            BlockSyncRecoveryMode::PayloadOnly
                        },
                    )
                } else {
                    self.handle_block_created(block_created, sender)
                }
            }
            super::message::BlockBodyData::BlockSyncUpdate(update) => {
                let header = update.block.header();
                if update.block.hash() != response.block_hash
                    || header.height().get() != response.height
                    || header.view_change_index() != response.view
                {
                    self.record_consensus_message_handling(
                        super::status::ConsensusMessageKind::BlockBodyResponse,
                        super::status::ConsensusMessageOutcome::Dropped,
                        super::status::ConsensusMessageReason::InvalidPayload,
                    );
                    self.release_block_payload_dedup(&dedup_key);
                    return Ok(());
                }
                self.handle_block_sync_update(update, sender)
            }
        };
        let body_materialized = self.frontier_block_materialized_locally(response.block_hash);
        if body_materialized
            && repairs_missing_highest_qc_dependency
            && self
                .cached_commit_qc_for_block(response.block_hash, response.height, response.view)
                .is_none()
        {
            let targets = self.known_block_commit_qc_recovery_targets(
                response.block_hash,
                response.height,
                response.view,
                &[],
            );
            let requested = self.maybe_request_known_block_commit_qc_recovery(
                response.block_hash,
                response.height,
                response.view,
                &targets,
                None,
                "highest_qc_dependency_body_materialized",
            );
            debug!(
                height = response.height,
                view = response.view,
                block = %response.block_hash,
                requested,
                "tracked known-block commit-QC recovery after highest-QC dependency body repair"
            );
        }
        let slot_matches_after = self.frontier_slot.as_ref().is_some_and(|slot| {
            slot.block_hash == response.block_hash
                && slot.height == response.height
                && slot.view == response.view
        });
        let missing_commit_qc_repair_pending = self.missing_commit_qc_request_pending_for_round(
            response.block_hash,
            response.height,
            response.view,
        );
        if body_materialized
            && missing_commit_qc_repair_pending
            && self
                .cached_commit_qc_for_block(response.block_hash, response.height, response.view)
                .is_none()
        {
            self.handle_detached_block_body_commit_qc(
                detached_commit_qc.take(),
                response.block_hash,
                response.height,
                response.view,
                "materialized_body",
            );
        }
        if body_materialized {
            let queue_depths = super::status::worker_queue_depth_snapshot();
            info!(
                height = response.height,
                view = response.view,
                block = %response.block_hash,
                sender = ?sender_for_slot.as_ref(),
                slot_matches = slot_matches_after,
                allow_same_height_repair,
                allow_rbc_body_repair,
                vote_rx_depth = queue_depths.vote_rx,
                block_payload_rx_depth = queue_depths.block_payload_rx,
                rbc_chunk_rx_depth = queue_depths.rbc_chunk_rx,
                block_rx_depth = queue_depths.block_rx,
                "materialized block body from BlockBodyResponse"
            );
            let _ = self.maybe_start_validation_for_pending_after_cached_new_view_qc(
                response.height,
                response.view,
                "block_body_response_payload_ready",
            );
        }
        let materialized_at = Instant::now();
        if body_materialized
            && response_has_commit_evidence
            && let Some(pending) = self.pending.pending_blocks.get_mut(&response.block_hash)
            && pending.height == response.height
            && pending.view == response.view
            && !pending.is_retry_aborted()
        {
            pending.touch_progress(materialized_at);
            debug!(
                height = response.height,
                view = response.view,
                block = %response.block_hash,
                "refreshed pending progress after evidence-bearing BlockBodyResponse materialized local body"
            );
        }
        if body_materialized && slot_matches_after {
            let _ = self.handle_frontier_slot_event(
                materialized_at,
                super::FrontierSlotEvent::OnBodyAvailable {
                    block_hash: response.block_hash,
                    view: response.view,
                    sender: sender_for_slot,
                },
            );
            let _ = self.try_replay_deferred_qcs();
            let _ = self.try_replay_deferred_missing_payload_qcs(Instant::now());
            self.request_commit_pipeline_for_pending(
                response.block_hash,
                super::status::RoundEventCauseTrace::BlockAvailable,
                None,
            );
            // Plain exact-body fallbacks carry no certificate evidence. While
            // commit-QC repair is pending, release their dedup key so repeated
            // plain payload repair cannot suppress a later evidence-bearing retry.
            if plain_body_response
                && (missing_commit_qc_repair_pending
                    || self.missing_commit_qc_request_pending_for_round(
                        response.block_hash,
                        response.height,
                        response.view,
                    ))
            {
                self.release_block_payload_dedup(&dedup_key);
            }
        } else {
            if body_materialized && allow_same_height_repair {
                self.request_commit_pipeline_for_pending(
                    response.block_hash,
                    super::status::RoundEventCauseTrace::BlockAvailable,
                    None,
                );
            }
            self.release_block_payload_dedup(&dedup_key);
        }
        result
    }

    fn handle_detached_block_body_commit_qc(
        &mut self,
        qc: Option<crate::sumeragi::consensus::Qc>,
        block_hash: HashOf<BlockHeader>,
        height: u64,
        view: u64,
        context: &'static str,
    ) {
        let Some(qc) = qc else {
            debug_assert_eq!(
                detached_block_body_commit_qc_decision(false, false, false),
                DetachedBlockBodyCommitQcDecision {
                    handle_qc: false,
                    clear_missing_commit_qc: false,
                }
            );
            return;
        };
        let cached_before = self
            .cached_commit_qc_for_block(block_hash, height, view)
            .is_some();
        let before_decision = detached_block_body_commit_qc_decision(true, cached_before, false);
        debug_assert_eq!(
            before_decision,
            DetachedBlockBodyCommitQcDecision {
                handle_qc: !cached_before,
                clear_missing_commit_qc: cached_before,
            }
        );
        if cached_before {
            self.clear_missing_commit_qc_request(&block_hash, MissingBlockClearReason::Obsolete);
            return;
        }
        info!(
            height,
            view,
            block = %block_hash,
            context,
            "processing commit QC from ignored BlockBodyResponse"
        );
        debug_assert!(before_decision.handle_qc);
        if let Err(err) = self.handle_qc(qc) {
            warn!(
                ?err,
                height,
                view,
                block = %block_hash,
                context,
                "failed to process commit QC from ignored BlockBodyResponse"
            );
        }
        let cached_after_handle = self
            .cached_commit_qc_for_block(block_hash, height, view)
            .is_some();
        let after_decision =
            detached_block_body_commit_qc_decision(true, false, cached_after_handle);
        debug_assert_eq!(
            after_decision,
            DetachedBlockBodyCommitQcDecision {
                handle_qc: true,
                clear_missing_commit_qc: cached_after_handle,
            }
        );
        if after_decision.clear_missing_commit_qc {
            self.clear_missing_commit_qc_request(&block_hash, MissingBlockClearReason::Obsolete);
        }
    }

    pub(super) fn materialize_frontier_block_sync_payload_for_qc_recovery(
        &mut self,
        block: &SignedBlock,
        observed_commit_qc_epoch: Option<u64>,
    ) -> bool {
        let block_hash = block.hash();
        let block_height = block.header().height().get();
        let block_view = block.header().view_change_index();
        if self.block_known_locally(block_hash) || !self.frontier_slot_is_exact_height(block_height)
        {
            return false;
        }

        let authoritative_owner = self.authoritative_slot_owner_hash(block_height, block_view);
        let deferred_commit_qc_epoch =
            self.deferred_missing_payload_qcs
                .values()
                .find_map(|entry| {
                    (entry.qc.subject_block_hash == block_hash
                        && entry.qc.height == block_height
                        && entry.qc.view == block_view
                        && matches!(entry.qc.phase, crate::sumeragi::consensus::Phase::Commit))
                    .then_some(entry.qc.epoch)
                });
        if authoritative_owner != Some(block_hash) && deferred_commit_qc_epoch.is_none() {
            return false;
        }

        let payload_bytes = super::proposals::block_payload_bytes(block);
        let payload_hash = Hash::new(&payload_bytes);
        let mut pending = PendingBlock::new_with_payload_bytes(
            block.clone(),
            payload_hash,
            block_height,
            block_view,
            payload_bytes,
        );
        if let Some(epoch) = observed_commit_qc_epoch.or(deferred_commit_qc_epoch) {
            pending.note_commit_qc_observed(epoch);
        }

        self.pending.pending_blocks.insert(block_hash, pending);
        self.deferred_block_sync_updates
            .remove(&(block_height, block_view, block_hash));
        self.flush_frontier_body_requesters(block);
        self.flush_pending_block_body_requests_if_ready(block);
        self.flush_pending_fetch_requests(block);
        self.clear_missing_block_request(&block_hash, MissingBlockClearReason::PayloadAvailable);
        self.clear_missing_block_view_change(&block_hash);
        info!(
            height = block_height,
            view = block_view,
            block = %block_hash,
            commit_qc_observed = observed_commit_qc_epoch
                .or(deferred_commit_qc_epoch)
                .is_some(),
            "materialized frontier block-sync payload for deferred QC recovery"
        );
        true
    }

    pub(super) fn prepare_known_block_qc_work(
        &mut self,
        qc: crate::sumeragi::consensus::Qc,
        block: Arc<SignedBlock>,
        topology: super::network_topology::Topology,
        stake_snapshot: Option<CommitStakeSnapshot>,
        consensus_mode: ConsensusMode,
        mode_tag: &'static str,
        prf_seed: Option<[u8; 32]>,
        commit_qc_match: bool,
    ) -> Option<KnownBlockQcWork> {
        let block_hash = block.hash();
        let block_height = block.header().height().get();
        let block_view = block.header().view_change_index();
        if topology.as_ref().is_empty() {
            let _ = self.handle_roster_unavailable_recovery(
                block_height,
                block_view,
                Some(block_hash),
                self.queue.queued_len(),
                Instant::now(),
                super::ProposalDeferWarningKind::EmptyCommitTopologyProposal,
                "prepare_known_block_qc_work_empty_commit_topology",
            );
            warn!(
                height = block_height,
                view = block_view,
                "dropping block sync QC: empty commit topology"
            );
            return None;
        }
        if qc.subject_block_hash != block_hash {
            warn!(
                incoming_hash = %block_hash,
                qc_hash = %qc.subject_block_hash,
                "ignoring block sync QC that does not match block hash"
            );
            return None;
        }
        if qc.height != block_height {
            warn!(
                incoming_hash = %block_hash,
                height = block_height,
                qc_height = qc.height,
                "ignoring block sync QC that does not match block height"
            );
            return None;
        }
        let expected_epoch = self.epoch_for_height(block_height);
        if qc.epoch != expected_epoch {
            warn!(
                incoming_hash = %block_hash,
                height = block_height,
                expected_epoch,
                qc_epoch = qc.epoch,
                "ignoring block sync QC with mismatched epoch"
            );
            return None;
        }
        if !matches!(qc.phase, crate::sumeragi::consensus::Phase::Commit) {
            warn!(
                incoming_hash = %block_hash,
                phase = ?qc.phase,
                "ignoring block sync QC with non-commit phase"
            );
            return None;
        }
        if let Some(lock) = self.locked_qc {
            let same_height_conflict = Self::block_sync_qc_same_height_conflict(lock, &qc);
            if same_height_conflict {
                if !Self::block_sync_qc_same_height_recoverable(lock, &qc, true) {
                    if self.defer_block_sync_qc_while_locked_payload_missing(
                        &qc,
                        "known_block_qc.height_conflict.missing_locked_payload",
                    ) {
                        return None;
                    }
                    crate::sumeragi::status::inc_block_sync_locked_qc_prefilter_drop();
                    self.log_block_sync_locked_qc_conflict(
                        &qc,
                        lock,
                        "known_block_qc.height_conflict",
                    );
                    self.record_consensus_message_handling(
                        super::status::ConsensusMessageKind::Qc,
                        super::status::ConsensusMessageOutcome::Dropped,
                        super::status::ConsensusMessageReason::LockedQc,
                    );
                    return None;
                }
            }
        }
        if self.block_sync_qc_is_stale_against_lock(&qc) {
            debug!(
                height = qc.height,
                view = qc.view,
                incoming_hash = %qc.subject_block_hash,
                locked_height = self.locked_qc.map(|lock| lock.height),
                "dropping stale known-block QC below locked height"
            );
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::Qc,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::LockedQc,
            );
            return None;
        }
        if !self.block_sync_qc_extends_locked_chain(&qc) {
            if self.defer_block_sync_qc_while_locked_payload_missing(
                &qc,
                "known_block_qc.non_extending.missing_locked_payload",
            ) {
                return None;
            }
            debug!(
                height = qc.height,
                view = qc.view,
                incoming_hash = %qc.subject_block_hash,
                "retaining known-block non-extending QC for lock realignment"
            );
        }
        Some(KnownBlockQcWork {
            qc,
            block,
            topology,
            stake_snapshot,
            consensus_mode,
            mode_tag,
            prf_seed,
            commit_qc_match,
            aggregate_ok: None,
        })
    }

    pub(super) fn enqueue_known_block_qc_work(&mut self, work: KnownBlockQcWork) {
        let key = Self::qc_tally_key(&work.qc);
        if self.known_block_qc_work.contains_key(&key) {
            debug!(
                phase = ?work.qc.phase,
                height = work.qc.height,
                view = work.qc.view,
                block = %work.qc.subject_block_hash,
                "dropping duplicate known-block QC work item"
            );
            return;
        }
        self.known_block_qc_work.insert(key, work);
        self.record_consensus_message_handling(
            super::status::ConsensusMessageKind::Qc,
            super::status::ConsensusMessageOutcome::Deferred,
            super::status::ConsensusMessageReason::AggregateVerifyDeferred,
        );
        debug!(
            height = key.2,
            view = key.3,
            block = %key.1,
            queued = self.known_block_qc_work.len(),
            "deferred known-block QC processing off payload queue"
        );
        if let Some(wake) = self.wake_tx.as_ref() {
            let _ = wake.try_send(());
        }
    }

    pub(super) fn drain_known_block_qc_work(&mut self, tick_deadline: Option<Instant>) -> bool {
        if self.known_block_qc_work.is_empty() {
            return false;
        }
        let mut progress = false;
        let mut processed = 0usize;
        while processed < KNOWN_BLOCK_QC_WORK_PER_TICK {
            if Self::tick_budget_exhausted(tick_deadline, Instant::now()) {
                break;
            }
            let key = match self.known_block_qc_work.keys().next().cloned() {
                Some(key) => key,
                None => break,
            };
            let Some(work) = self.known_block_qc_work.remove(&key) else {
                continue;
            };
            if self.apply_known_block_qc_work(work) {
                progress = true;
            }
            processed = processed.saturating_add(1);
        }
        if processed > 0 {
            debug!(
                processed,
                remaining = self.known_block_qc_work.len(),
                "drained known-block QC work items"
            );
        }
        progress
    }

    fn dispatch_known_block_qc_verify(
        &mut self,
        work: KnownBlockQcWork,
    ) -> Option<KnownBlockQcWork> {
        if self.subsystems.qc_verify.work_txs.is_empty() {
            return Some(work);
        }
        let canonical_roster = super::roster::canonicalize_roster(work.topology.as_ref().to_vec());
        let canonical_topology = super::network_topology::Topology::new(canonical_roster);
        let Some(inputs) = super::qc_aggregate_inputs(
            &work.qc,
            &canonical_topology,
            &self.roster_validation_cache.pops,
            &self.common_config.chain,
            work.mode_tag,
        ) else {
            return Some(work);
        };
        let key = super::QcVerifyKey::from_qc(&work.qc);
        if self.subsystems.qc_verify.inflight.contains_key(&key) {
            debug!(
                height = work.qc.height,
                view = work.qc.view,
                phase = ?work.qc.phase,
                block = %work.qc.subject_block_hash,
                "known-block QC verify already in flight"
            );
            return None;
        }
        let id = self.subsystems.qc_verify.next_id();
        let mut verify_work = super::qc_verify::QcVerifyWork {
            id,
            key: key.clone(),
            inputs,
        };
        let mut dispatched = false;
        let mut disconnected = Vec::new();
        let total = self.subsystems.qc_verify.work_txs.len();
        for _ in 0..total {
            let idx = self.subsystems.qc_verify.next_worker % total;
            self.subsystems.qc_verify.next_worker =
                self.subsystems.qc_verify.next_worker.saturating_add(1);
            let work_tx = &self.subsystems.qc_verify.work_txs[idx];
            match work_tx.try_send(verify_work) {
                Ok(()) => {
                    dispatched = true;
                    break;
                }
                Err(std::sync::mpsc::TrySendError::Full(returned)) => {
                    verify_work = returned;
                }
                Err(std::sync::mpsc::TrySendError::Disconnected(returned)) => {
                    verify_work = returned;
                    disconnected.push(idx);
                }
            }
        }
        if !disconnected.is_empty() {
            disconnected.sort_unstable();
            disconnected.dedup();
            for idx in disconnected.into_iter().rev() {
                if idx < self.subsystems.qc_verify.work_txs.len() {
                    self.subsystems.qc_verify.work_txs.swap_remove(idx);
                }
            }
            if self.subsystems.qc_verify.next_worker >= self.subsystems.qc_verify.work_txs.len() {
                self.subsystems.qc_verify.next_worker = 0;
            }
        }
        if dispatched {
            self.subsystems.qc_verify.inflight.insert(
                key,
                super::QcVerifyInFlight {
                    id,
                    target: super::QcVerifyTarget::KnownBlock(work),
                },
            );
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::Qc,
                super::status::ConsensusMessageOutcome::Deferred,
                super::status::ConsensusMessageReason::AggregateVerifyDeferred,
            );
            return None;
        }
        if self.subsystems.qc_verify.work_txs.is_empty() {
            warn!(
                height = work.qc.height,
                view = work.qc.view,
                phase = ?work.qc.phase,
                block = %work.qc.subject_block_hash,
                "QC verify workers unavailable for known-block QC; running aggregate verification inline"
            );
            self.subsystems.qc_verify.result_rx = None;
            self.subsystems.qc_verify.inflight.clear();
        } else {
            warn!(
                height = work.qc.height,
                view = work.qc.view,
                phase = ?work.qc.phase,
                block = %work.qc.subject_block_hash,
                "QC verify worker queue full for known-block QC; running aggregate verification inline"
            );
        }
        Some(work)
    }

    pub(super) fn apply_known_block_qc_work(&mut self, work: KnownBlockQcWork) -> bool {
        if self.block_sync_qc_is_stale_against_lock(&work.qc) {
            debug!(
                height = work.qc.height,
                view = work.qc.view,
                incoming_hash = %work.qc.subject_block_hash,
                locked_height = self.locked_qc.map(|lock| lock.height),
                "dropping stale known-block QC below locked height before verify dispatch"
            );
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::Qc,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::LockedQc,
            );
            return true;
        }
        let cached_qc_match = cached_qc_for(
            &self.qc_cache,
            work.qc.phase,
            work.qc.subject_block_hash,
            work.qc.height,
            work.qc.view,
            work.qc.epoch,
        )
        .filter(|cached| HashOf::new(cached) == HashOf::new(&work.qc));
        let cached_tally = if cached_qc_match.is_some() || work.commit_qc_match {
            self.qc_signer_tally
                .get(&Self::qc_tally_key(&work.qc))
                .cloned()
        } else {
            None
        };
        let mut work = work;
        if cached_tally.is_none() && work.aggregate_ok.is_none() {
            if let Some(pending) = self.dispatch_known_block_qc_verify(work) {
                work = pending;
            } else {
                return false;
            }
        }
        let KnownBlockQcWork {
            qc,
            block,
            topology,
            stake_snapshot,
            consensus_mode,
            mode_tag,
            prf_seed,
            commit_qc_match: _,
            aggregate_ok,
        } = work;
        let block_hash = block.hash();
        let block_height = block.header().height().get();
        let block_view = block.header().view_change_index();
        let qc_signers = qc_signer_count(&qc);
        let tally = if let Some(tally) = cached_tally {
            Ok(tally)
        } else {
            let signer_cache_key =
                BlockSignerCacheKey::new(block_hash, topology.as_ref(), consensus_mode, prf_seed);
            let cached_block_signers = signer_cache_key
                .as_ref()
                .and_then(|key| self.block_signer_cache.get(key));
            let block_signers = if let Some(signers) = cached_block_signers {
                signers
            } else {
                let block_signers = {
                    let world_view = self.state.world_view();
                    validated_block_signers_from_world(
                        &block,
                        &topology,
                        &world_view,
                        mode_tag,
                        prf_seed,
                    )
                };
                match block_signers {
                    Ok(signers) => {
                        if let Some(key) = signer_cache_key {
                            self.block_signer_cache.insert(key, signers.clone());
                        }
                        signers
                    }
                    Err(err) => {
                        warn!(
                            ?err,
                            height = block_height,
                            view = block_view,
                            block = %block_hash,
                            "block sync QC received for known block with invalid signatures; proceeding without signer subset check"
                        );
                        BTreeSet::new()
                    }
                }
            };
            let world_view = self.state.world_view();
            tally_qc_against_block_signers(
                &qc,
                &topology,
                &world_view,
                &block_signers,
                block_view,
                &self.roster_validation_cache.pops,
                &self.common_config.chain,
                consensus_mode,
                stake_snapshot.as_ref(),
                mode_tag,
                prf_seed,
                aggregate_ok,
            )
        };
        let tally = match tally {
            Ok(tally) => tally,
            Err(err) => {
                let reason = qc_validation_reason(&err);
                if Self::block_sync_qc_is_missing_context_error(&err) {
                    self.quarantine_block_sync_qc_candidate(
                        qc.clone(),
                        reason,
                        QuarantinedQcTarget::BlockSync,
                    );
                } else {
                    self.block_sync_qc_final_drop(reason);
                }
                warn!(
                    ?err,
                    height = block_height,
                    view = block_view,
                    block = %block_hash,
                    qc_signers,
                    "dropping block sync QC: tally validation failed"
                );
                return false;
            }
        };
        crate::sumeragi::status::record_precommit_signers(
            crate::sumeragi::status::PrecommitSignerRecord {
                block_hash,
                height: qc.height,
                view: qc.view,
                epoch: qc.epoch,
                chain_order_hash: qc.chain_order_hash,
                rechain_seq: qc.rechain_seq,
                parent_state_root: qc.parent_state_root,
                post_state_root: qc.post_state_root,
                signers: tally.voting_signers.clone(),
                bls_aggregate_signature: qc.aggregate.bls_aggregate_signature.clone(),
                roster_len: topology.as_ref().len(),
                mode_tag: mode_tag.to_string(),
                validator_set: topology.as_ref().to_vec(),
                stake_snapshot: stake_snapshot.clone(),
            },
        );
        self.note_validated_qc_tally(&qc, tally.clone());
        let mut block_known_for_commit =
            self.pending
                .pending_blocks
                .get(&block_hash)
                .is_some_and(|pending| {
                    !pending.is_retry_aborted()
                        && pending.validation_status == ValidationStatus::Valid
                })
                || self
                    .subsystems
                    .commit
                    .inflight
                    .as_ref()
                    .is_some_and(|inflight| {
                        inflight.block_hash == block_hash && !inflight.pending.aborted
                    })
                || self.kura.get_block_height_by_hash(block_hash).is_some();
        if block_known_for_commit {
            block_known_for_commit = self.rehydrate_pending_from_kura_for_qc(&qc);
        }
        let process_ok = self.process_precommit_qc(&qc, block_known_for_commit, true);
        if !process_ok {
            if self.block_sync_qc_is_stale_against_lock(&qc) {
                debug!(
                    height = qc.height,
                    view = qc.view,
                    incoming_hash = %qc.subject_block_hash,
                    locked_height = self.locked_qc.map(|lock| lock.height),
                    "dropping stale block sync QC below locked height"
                );
            } else if let Some(lock) = self.locked_qc {
                if Self::block_sync_qc_same_height_conflict(lock, &qc) {
                    crate::sumeragi::status::inc_block_sync_locked_qc_prefilter_drop();
                }
                self.log_block_sync_locked_qc_conflict(
                    &qc,
                    lock,
                    "known_block_qc.apply.precommit_reject",
                );
            }
            return true;
        }
        let checkpoint = ValidatorSetCheckpoint::new_with_chain_order(
            qc.height,
            qc.view,
            qc.subject_block_hash,
            qc.chain_order_hash,
            qc.rechain_seq,
            qc.parent_state_root,
            qc.post_state_root,
            qc.validator_set.clone(),
            qc.aggregate.signers_bitmap.clone(),
            qc.aggregate.bls_aggregate_signature.clone(),
            qc.validator_set_hash_version,
            None,
        );
        if self
            .state
            .record_commit_roster(&qc, &checkpoint, stake_snapshot.clone())
        {
            debug!(
                incoming_hash = %block_hash,
                height = block_height,
                view = block_view,
                "recorded commit roster from block sync QC"
            );
        }
        let qc_key = Self::qc_tally_key(&qc);
        self.deferred_missing_payload_qcs.remove(&qc_key);
        self.quarantined_block_sync_qcs.remove(&qc_key);
        super::status::record_commit_qc(qc.clone());
        self.qc_cache.insert(Self::qc_tally_key(&qc), qc.clone());
        self.clear_missing_commit_qc_request(&block_hash, MissingBlockClearReason::Obsolete);
        debug!(
            incoming_hash = %block_hash,
            signers = tally.voting_signers.len(),
            qc_signers,
            "applied block sync QC for known block"
        );
        if block_known_for_commit {
            self.apply_commit_qc(&qc, topology.as_ref(), block_hash, block_height, block_view);
            self.qc_cache
                .entry(Self::qc_tally_key(&qc))
                .or_insert_with(|| qc.clone());
            self.request_commit_pipeline_for_round(
                block_height,
                block_view,
                super::status::RoundPhaseTrace::WaitCommitQc,
                super::status::RoundEventCauseTrace::BlockSyncUpdated,
                None,
            );
        } else {
            if let Some(pending) = self.pending.pending_blocks.get_mut(&block_hash) {
                pending.note_commit_qc_observed(qc.epoch);
            }
            debug!(
                incoming_hash = %block_hash,
                height = block_height,
                view = block_view,
                "deferring commit apply for block sync QC until block is validated"
            );
        }
        true
    }
}

#[cfg(test)]
mod allow_uncertified_block_sync_roster_tests {
    use iroha_config::parameters::actual::ConsensusMode;
    use iroha_crypto::{Hash, HashOf, KeyPair};
    use iroha_data_model::{
        block::BlockHeader,
        consensus::{VALIDATOR_SET_HASH_VERSION_V1, ValidatorSetCheckpoint},
        peer::PeerId,
    };
    use iroha_primitives::numeric::Numeric;

    use crate::{
        commit_roster_journal::CommitRosterSnapshot,
        sumeragi::{
            consensus::{NPOS_TAG, PERMISSIONED_TAG, Phase, Qc, QcAggregate, ValidatorIndex, Vote},
            stake_snapshot::{CommitStakeSnapshot, CommitStakeSnapshotEntry},
        },
    };

    use super::super::message::FetchPendingBlockPriority;
    use super::super::proposal_handlers::BlockSyncRecoveryMode;
    use super::super::{
        FutureConsensusMessageDropDecision, future_consensus_message_drop_decision,
    };
    use super::{
        BLOCK_SYNC_COMMIT_CONFLICT_EVIDENCE_REASON, BlockBodyDirectCommitQcSource,
        BlockBodyRepairEpochSource, BlockBodyResponseDispatchDecision,
        BlockBodyResponsePayloadIdentity, BlockSyncFetchBlockBodyHandleDecision,
        BlockSyncFetchResponseDeferralCommittedHash, BlockSyncFetchResponseDeferralMessage,
        BlockSyncKnownRosterCandidateQcSource, BlockSyncNoRosterFallbackSource,
        BlockSyncRosterSource, BlockSyncSelectedApplySignedQuorumRepair,
        BlockSyncSelectedApplySparseRecovery, BlockSyncSelectedQcProcessTallySource,
        BlockSyncSelectedQcShape, BlockSyncSelectedQcSource, BlockSyncSnapshotHintFilter,
        DetachedBlockBodyCommitQcDecision, DirectCommitQcForBlockResult,
        DirectCommitQcTopologySource, FetchPendingResponseFinalPayload,
        FetchPendingResponsePayloadKind, FetchPendingResponsesBatchPayloadKind,
        FetchPendingResponsesBatchPayloadMessage, PendingResponseFlushKind,
        allow_uncertified_block_sync_roster, block_body_direct_commit_qc_created_source,
        block_body_direct_commit_qc_update_source, block_body_repair_epoch_decision,
        block_body_repair_epoch_deferred_source, block_body_repair_epoch_pending_source,
        block_body_repair_gate_decision, block_body_request_stash_window_decision,
        block_body_response_dispatch_decision, block_sync_commit_conflict_allow_genesis_stub,
        block_sync_commit_conflict_detected, block_sync_commit_conflict_drop_record,
        block_sync_commit_conflict_invalid_qc_evidence,
        block_sync_commit_conflict_should_clear_missing,
        block_sync_commit_conflict_should_emit_evidence,
        block_sync_commit_conflict_should_validate_qc, block_sync_consensus_mode_tag,
        block_sync_fetch_block_body_handle_decision, block_sync_future_window_drop_decision,
        block_sync_future_window_far_ahead, block_sync_future_window_lower_unresolved,
        block_sync_future_window_pre_generic_drop, block_sync_future_window_requested_margin,
        block_sync_known_roster_candidate_qc, block_sync_no_roster_fallback_roster,
        block_sync_no_roster_known_vote_only, block_sync_selected_apply_allow_nonextending_qc,
        block_sync_selected_apply_authoritative_supersede,
        block_sync_selected_apply_payload_unapplied_drop,
        block_sync_selected_apply_pending_commit_qc_observed,
        block_sync_selected_apply_preserve_on_payload_mismatch,
        block_sync_selected_apply_qc_to_apply, block_sync_selected_apply_recovery_mode,
        block_sync_selected_apply_same_height_frontier_conflict,
        block_sync_selected_apply_signed_quorum_commit_repair_active,
        block_sync_selected_apply_sparse_next_height_payload_recovered,
        block_sync_selected_qc_aggregate_ok, block_sync_selected_qc_cache_final_validation_drop,
        block_sync_selected_qc_cache_missing_context_quarantine,
        block_sync_selected_qc_cache_update_locked_qc, block_sync_selected_qc_candidate,
        block_sync_selected_qc_prefilter_epoch_mismatch,
        block_sync_selected_qc_prefilter_hash_mismatch,
        block_sync_selected_qc_prefilter_height_mismatch,
        block_sync_selected_qc_prefilter_nonextending_defer,
        block_sync_selected_qc_prefilter_nonextending_locked_drop,
        block_sync_selected_qc_prefilter_nonextending_needs_resolution,
        block_sync_selected_qc_prefilter_phase_mismatch,
        block_sync_selected_qc_prefilter_retain_nonextending,
        block_sync_selected_qc_prefilter_same_height_locked_drop,
        block_sync_selected_qc_prefilter_stale_locked_drop,
        block_sync_selected_qc_prefilter_topology_recovery,
        block_sync_selected_qc_process_apply_commit_qc,
        block_sync_selected_qc_process_block_known_for_commit,
        block_sync_selected_qc_process_cache_unknown_block_qc,
        block_sync_selected_qc_process_clean_rbc_sessions,
        block_sync_selected_qc_process_commit_qc_accepted,
        block_sync_selected_qc_process_observe_pending_epoch,
        block_sync_selected_qc_process_tally_source, block_sync_selected_qc_shape,
        block_sync_selected_qc_should_accept_aggregate_fallback,
        block_sync_selected_qc_should_attempt_aggregate_fallback,
        block_sync_selected_qc_should_derive_cached,
        block_sync_selected_qc_should_drop_invalid_payload,
        block_sync_selected_quorum_should_call_repair,
        block_sync_selected_quorum_should_defer_npos_vote_only,
        block_sync_selected_quorum_should_maybe_request_missing_qc,
        block_sync_selected_quorum_sparse_exact_frontier_request,
        block_sync_selected_signatures_ahead_of_frontier,
        block_sync_selected_signatures_has_roster_evidence,
        block_sync_selected_signatures_should_cache_validated_signers,
        block_sync_selected_signatures_should_defer,
        block_sync_selected_signatures_should_request_gap, block_sync_snapshot_hint_filter,
        block_sync_snapshot_roster_selection, block_sync_stale_view_drop_record,
        block_sync_stale_view_has_commit_evidence, block_sync_stale_view_should_drop,
        block_sync_vote_placeholder_matches, deferred_block_sync_cache_decision,
        deferred_block_sync_cache_key, deferred_block_sync_cap_eviction_count,
        deferred_block_sync_cap_should_evict, deferred_block_sync_commit_evidence_present,
        deferred_block_sync_defer_record_decision, deferred_block_sync_eviction_rank,
        deferred_block_sync_merge_decision, deferred_block_sync_replay_decision,
        deferred_block_sync_update_deferral_reason, deferred_block_sync_validation_inflight_blocks,
        deferred_block_sync_validation_pending_conflicts, detached_block_body_commit_qc_decision,
        direct_commit_qc_for_block_decision, fetch_pending_response_frame_decision,
        fetch_pending_response_preflight_decision, fetch_pending_responses_batch_commit_decision,
        fetch_pending_responses_batch_payload_decision,
        fetch_pending_responses_batch_should_build_payload, pending_response_flush_decision,
        pending_response_flush_targets_requester, same_height_block_body_repair_decision,
        same_height_block_body_repair_source_matches,
        should_defer_canonical_committed_fetch_response_shape,
        should_mark_block_sync_implicit_recovery, should_note_block_sync_vote_placeholder,
    };

    fn snapshot_roster_test_keypair() -> KeyPair {
        KeyPair::try_random()
            .expect("block-sync snapshot roster fixture key generation should succeed")
    }

    fn snapshot_roster_test_peer() -> PeerId {
        PeerId::new(snapshot_roster_test_keypair().public_key().clone())
    }

    fn snapshot_roster_test_stake_snapshot(roster: &[PeerId]) -> CommitStakeSnapshot {
        CommitStakeSnapshot {
            validator_set_hash: HashOf::new(&roster.to_vec()),
            entries: roster
                .iter()
                .cloned()
                .map(|peer_id| CommitStakeSnapshotEntry {
                    peer_id,
                    stake: Numeric::new(1, 0),
                })
                .collect(),
        }
    }

    fn snapshot_roster_test_snapshot(
        roster: Vec<PeerId>,
        stake_snapshot: Option<CommitStakeSnapshot>,
    ) -> CommitRosterSnapshot {
        let block_hash =
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x61; Hash::LENGTH]));
        let parent_state_root = Hash::prehashed([0x62; Hash::LENGTH]);
        let post_state_root = Hash::prehashed([0x63; Hash::LENGTH]);
        let signers_bitmap = if roster.is_empty() {
            Vec::new()
        } else {
            vec![0b0000_0001]
        };
        let qc = Qc {
            phase: Phase::Commit,
            subject_block_hash: block_hash,
            parent_state_root,
            post_state_root,
            height: 5,
            view: 2,
            epoch: 0,
            chain_order_hash: crate::sumeragi::consensus::default_chain_order_hash(),
            rechain_seq: 0,
            mode_tag: PERMISSIONED_TAG.to_owned(),
            highest_qc: None,
            validator_set_hash: HashOf::new(&roster),
            validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
            validator_set: roster.clone(),
            aggregate: QcAggregate {
                signers_bitmap: signers_bitmap.clone(),
                bls_aggregate_signature: Vec::new(),
            },
        };
        let checkpoint = ValidatorSetCheckpoint::new(
            qc.height,
            qc.view,
            block_hash,
            parent_state_root,
            post_state_root,
            roster,
            signers_bitmap,
            Vec::new(),
            VALIDATOR_SET_HASH_VERSION_V1,
            None,
        );
        CommitRosterSnapshot {
            commit_qc: qc,
            validator_checkpoint: checkpoint,
            stake_snapshot,
        }
    }

    #[test]
    fn allows_next_height_without_explicit_request() {
        assert!(allow_uncertified_block_sync_roster(11, 10, false));
    }

    #[test]
    fn rejects_farther_height_without_explicit_request() {
        assert!(!allow_uncertified_block_sync_roster(12, 10, false));
    }

    #[test]
    fn allows_any_height_when_missing_block_is_requested() {
        assert!(allow_uncertified_block_sync_roster(25, 10, true));
    }

    #[test]
    fn formal_gate_matrix_matches_requested_and_next_height_policy() {
        for (block_height, local_height, label) in [
            (2, 3, "requested stale"),
            (3, 3, "requested same height"),
            (4, 3, "requested next height"),
            (5, 3, "requested future"),
        ] {
            assert!(
                allow_uncertified_block_sync_roster(block_height, local_height, true),
                "{label} missing-block requests should allow uncertified roster selection"
            );
        }

        for (block_height, local_height, expected, label) in [
            (1, 0, true, "zero to next height"),
            (4, 3, true, "ordinary next height"),
            (u64::MAX, u64::MAX, true, "saturated next height"),
            (3, 3, false, "same height"),
            (2, 3, false, "stale height"),
            (5, 3, false, "future height"),
        ] {
            assert_eq!(
                allow_uncertified_block_sync_roster(block_height, local_height, false),
                expected,
                "unrequested {label} policy mismatch"
            );
        }
    }

    #[test]
    fn block_sync_implicit_recovery_formal_gate_matrix() {
        for (case, initial_requested, expected_after) in [
            ("already requested", true, true),
            ("da disabled", false, false),
            ("known local", false, false),
            ("above frontier bound", false, false),
            ("implicit disallowed", false, false),
            ("same height implicit", false, true),
            ("next height implicit", false, true),
            ("saturated boundary implicit", false, true),
        ] {
            let (da_enabled, known_local, block_height, local_height, implicit_allowed) = match case
            {
                "already requested" => (true, false, 6, 5, true),
                "da disabled" => (false, false, 6, 5, true),
                "known local" => (true, true, 6, 5, true),
                "above frontier bound" => (true, false, 7, 5, true),
                "implicit disallowed" => (true, false, 6, 5, false),
                "same height implicit" => (true, false, 5, 5, true),
                "next height implicit" => (true, false, 6, 5, true),
                "saturated boundary implicit" => (true, false, u64::MAX, u64::MAX, true),
                _ => unreachable!("covered cases"),
            };
            let should_mark = should_mark_block_sync_implicit_recovery(
                da_enabled,
                initial_requested,
                known_local,
                block_height,
                local_height,
                implicit_allowed,
            );
            assert_eq!(
                initial_requested || should_mark,
                expected_after,
                "{case} requested flag mismatch"
            );
        }
    }

    #[test]
    fn block_sync_snapshot_roster_formal_gate_matrix() {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        enum Source {
            Snapshot,
            Persisted,
            Cache,
            Fresh,
            None,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        enum SidecarArg {
            NotCalled,
            Allowed,
            Blocked,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        struct Decision {
            selected_source: Source,
            snapshot_roster_origin: bool,
            snapshot_commit_qc_included: bool,
            snapshot_checkpoint_included: bool,
            snapshot_stake_included: bool,
            snapshot_cache_insert: bool,
            persisted_lookup_called: bool,
            allow_sidecar_arg: SidecarArg,
            cache_lookup_called: bool,
            fresh_selector_called: bool,
            fresh_cache_insert: bool,
        }

        #[derive(Debug, Clone, Copy)]
        struct Case {
            label: &'static str,
            has_snapshot: bool,
            snapshot_roster_nonempty: bool,
            snapshot_stake_present: bool,
            snapshot_stake_matches: bool,
            snapshot_cache_key: bool,
            persisted_available: bool,
            cache_hit: bool,
            fallback_cache_key: bool,
            fresh_available: bool,
            fresh_certified: bool,
            sidecar_quarantined: bool,
            expected: Decision,
        }

        let snapshot = Decision {
            selected_source: Source::Snapshot,
            snapshot_roster_origin: true,
            snapshot_commit_qc_included: true,
            snapshot_checkpoint_included: true,
            snapshot_stake_included: true,
            snapshot_cache_insert: true,
            persisted_lookup_called: false,
            allow_sidecar_arg: SidecarArg::NotCalled,
            cache_lookup_called: false,
            fresh_selector_called: false,
            fresh_cache_insert: false,
        };
        let persisted = Decision {
            selected_source: Source::Persisted,
            snapshot_roster_origin: false,
            snapshot_commit_qc_included: false,
            snapshot_checkpoint_included: false,
            snapshot_stake_included: false,
            snapshot_cache_insert: false,
            persisted_lookup_called: true,
            allow_sidecar_arg: SidecarArg::Allowed,
            cache_lookup_called: false,
            fresh_selector_called: false,
            fresh_cache_insert: false,
        };
        let cache = Decision {
            selected_source: Source::Cache,
            cache_lookup_called: true,
            ..persisted
        };
        let fresh_certified = Decision {
            selected_source: Source::Fresh,
            cache_lookup_called: true,
            fresh_selector_called: true,
            fresh_cache_insert: true,
            ..persisted
        };
        let fresh_uncertified = Decision {
            selected_source: Source::Fresh,
            cache_lookup_called: true,
            fresh_selector_called: true,
            fresh_cache_insert: false,
            ..persisted
        };
        let no_selection = Decision {
            selected_source: Source::None,
            cache_lookup_called: true,
            fresh_selector_called: true,
            ..persisted
        };
        let cases = [
            Case {
                label: "snapshot_matching_stake",
                has_snapshot: true,
                snapshot_roster_nonempty: true,
                snapshot_stake_present: true,
                snapshot_stake_matches: true,
                snapshot_cache_key: true,
                persisted_available: false,
                cache_hit: false,
                fallback_cache_key: true,
                fresh_available: false,
                fresh_certified: false,
                sidecar_quarantined: false,
                expected: snapshot,
            },
            Case {
                label: "snapshot_no_stake",
                has_snapshot: true,
                snapshot_roster_nonempty: true,
                snapshot_stake_present: false,
                snapshot_stake_matches: false,
                snapshot_cache_key: true,
                persisted_available: false,
                cache_hit: false,
                fallback_cache_key: true,
                fresh_available: false,
                fresh_certified: false,
                sidecar_quarantined: false,
                expected: Decision {
                    snapshot_stake_included: false,
                    ..snapshot
                },
            },
            Case {
                label: "snapshot_wrong_stake",
                has_snapshot: true,
                snapshot_roster_nonempty: true,
                snapshot_stake_present: true,
                snapshot_stake_matches: false,
                snapshot_cache_key: true,
                persisted_available: false,
                cache_hit: false,
                fallback_cache_key: true,
                fresh_available: false,
                fresh_certified: false,
                sidecar_quarantined: false,
                expected: Decision {
                    snapshot_stake_included: false,
                    ..snapshot
                },
            },
            Case {
                label: "snapshot_no_key",
                has_snapshot: true,
                snapshot_roster_nonempty: true,
                snapshot_stake_present: false,
                snapshot_stake_matches: false,
                snapshot_cache_key: false,
                persisted_available: false,
                cache_hit: false,
                fallback_cache_key: true,
                fresh_available: false,
                fresh_certified: false,
                sidecar_quarantined: false,
                expected: Decision {
                    snapshot_stake_included: false,
                    snapshot_cache_insert: false,
                    ..snapshot
                },
            },
            Case {
                label: "snapshot_preempts_persisted",
                has_snapshot: true,
                snapshot_roster_nonempty: true,
                snapshot_stake_present: true,
                snapshot_stake_matches: true,
                snapshot_cache_key: true,
                persisted_available: true,
                cache_hit: false,
                fallback_cache_key: true,
                fresh_available: false,
                fresh_certified: false,
                sidecar_quarantined: false,
                expected: snapshot,
            },
            Case {
                label: "snapshot_preempts_cache",
                has_snapshot: true,
                snapshot_roster_nonempty: true,
                snapshot_stake_present: true,
                snapshot_stake_matches: true,
                snapshot_cache_key: true,
                persisted_available: false,
                cache_hit: true,
                fallback_cache_key: true,
                fresh_available: false,
                fresh_certified: false,
                sidecar_quarantined: false,
                expected: snapshot,
            },
            Case {
                label: "snapshot_preempts_fresh",
                has_snapshot: true,
                snapshot_roster_nonempty: true,
                snapshot_stake_present: true,
                snapshot_stake_matches: true,
                snapshot_cache_key: true,
                persisted_available: false,
                cache_hit: false,
                fallback_cache_key: true,
                fresh_available: true,
                fresh_certified: true,
                sidecar_quarantined: false,
                expected: snapshot,
            },
            Case {
                label: "snapshot_empty_persisted",
                has_snapshot: true,
                snapshot_roster_nonempty: false,
                snapshot_stake_present: false,
                snapshot_stake_matches: false,
                snapshot_cache_key: true,
                persisted_available: true,
                cache_hit: false,
                fallback_cache_key: true,
                fresh_available: false,
                fresh_certified: false,
                sidecar_quarantined: false,
                expected: persisted,
            },
            Case {
                label: "snapshot_empty_none",
                has_snapshot: true,
                snapshot_roster_nonempty: false,
                snapshot_stake_present: false,
                snapshot_stake_matches: false,
                snapshot_cache_key: true,
                persisted_available: false,
                cache_hit: false,
                fallback_cache_key: true,
                fresh_available: false,
                fresh_certified: false,
                sidecar_quarantined: false,
                expected: no_selection,
            },
            Case {
                label: "no_snapshot_persisted_allowed",
                has_snapshot: false,
                snapshot_roster_nonempty: false,
                snapshot_stake_present: false,
                snapshot_stake_matches: false,
                snapshot_cache_key: true,
                persisted_available: true,
                cache_hit: false,
                fallback_cache_key: true,
                fresh_available: false,
                fresh_certified: false,
                sidecar_quarantined: false,
                expected: persisted,
            },
            Case {
                label: "no_snapshot_persisted_quarantined",
                has_snapshot: false,
                snapshot_roster_nonempty: false,
                snapshot_stake_present: false,
                snapshot_stake_matches: false,
                snapshot_cache_key: true,
                persisted_available: true,
                cache_hit: false,
                fallback_cache_key: true,
                fresh_available: false,
                fresh_certified: false,
                sidecar_quarantined: true,
                expected: Decision {
                    allow_sidecar_arg: SidecarArg::Blocked,
                    ..persisted
                },
            },
            Case {
                label: "no_snapshot_persisted_and_cache",
                has_snapshot: false,
                snapshot_roster_nonempty: false,
                snapshot_stake_present: false,
                snapshot_stake_matches: false,
                snapshot_cache_key: true,
                persisted_available: true,
                cache_hit: true,
                fallback_cache_key: true,
                fresh_available: false,
                fresh_certified: false,
                sidecar_quarantined: false,
                expected: persisted,
            },
            Case {
                label: "no_snapshot_cache_hit",
                has_snapshot: false,
                snapshot_roster_nonempty: false,
                snapshot_stake_present: false,
                snapshot_stake_matches: false,
                snapshot_cache_key: true,
                persisted_available: false,
                cache_hit: true,
                fallback_cache_key: true,
                fresh_available: false,
                fresh_certified: false,
                sidecar_quarantined: false,
                expected: cache,
            },
            Case {
                label: "no_snapshot_cache_and_fresh",
                has_snapshot: false,
                snapshot_roster_nonempty: false,
                snapshot_stake_present: false,
                snapshot_stake_matches: false,
                snapshot_cache_key: true,
                persisted_available: false,
                cache_hit: true,
                fallback_cache_key: true,
                fresh_available: true,
                fresh_certified: true,
                sidecar_quarantined: false,
                expected: cache,
            },
            Case {
                label: "no_snapshot_fresh_qc",
                has_snapshot: false,
                snapshot_roster_nonempty: false,
                snapshot_stake_present: false,
                snapshot_stake_matches: false,
                snapshot_cache_key: true,
                persisted_available: false,
                cache_hit: false,
                fallback_cache_key: true,
                fresh_available: true,
                fresh_certified: true,
                sidecar_quarantined: false,
                expected: fresh_certified,
            },
            Case {
                label: "no_snapshot_fresh_checkpoint",
                has_snapshot: false,
                snapshot_roster_nonempty: false,
                snapshot_stake_present: false,
                snapshot_stake_matches: false,
                snapshot_cache_key: true,
                persisted_available: false,
                cache_hit: false,
                fallback_cache_key: true,
                fresh_available: true,
                fresh_certified: true,
                sidecar_quarantined: false,
                expected: fresh_certified,
            },
            Case {
                label: "no_snapshot_fresh_uncertified",
                has_snapshot: false,
                snapshot_roster_nonempty: false,
                snapshot_stake_present: false,
                snapshot_stake_matches: false,
                snapshot_cache_key: true,
                persisted_available: false,
                cache_hit: false,
                fallback_cache_key: true,
                fresh_available: true,
                fresh_certified: false,
                sidecar_quarantined: false,
                expected: fresh_uncertified,
            },
            Case {
                label: "no_snapshot_fresh_no_key",
                has_snapshot: false,
                snapshot_roster_nonempty: false,
                snapshot_stake_present: false,
                snapshot_stake_matches: false,
                snapshot_cache_key: true,
                persisted_available: false,
                cache_hit: false,
                fallback_cache_key: false,
                fresh_available: true,
                fresh_certified: true,
                sidecar_quarantined: false,
                expected: Decision {
                    cache_lookup_called: false,
                    fresh_cache_insert: false,
                    ..fresh_certified
                },
            },
            Case {
                label: "no_snapshot_none",
                has_snapshot: false,
                snapshot_roster_nonempty: false,
                snapshot_stake_present: false,
                snapshot_stake_matches: false,
                snapshot_cache_key: true,
                persisted_available: false,
                cache_hit: false,
                fallback_cache_key: true,
                fresh_available: false,
                fresh_certified: false,
                sidecar_quarantined: false,
                expected: no_selection,
            },
        ];

        let roster = vec![snapshot_roster_test_peer(), snapshot_roster_test_peer()];
        let matching_stake = snapshot_roster_test_stake_snapshot(&roster);
        let wrong_stake = snapshot_roster_test_stake_snapshot(&[snapshot_roster_test_peer()]);

        for case in cases {
            let snapshot_selected = case.has_snapshot && case.snapshot_roster_nonempty;
            let selected_source = if snapshot_selected {
                Source::Snapshot
            } else if case.persisted_available {
                Source::Persisted
            } else if case.cache_hit && case.fallback_cache_key {
                Source::Cache
            } else if case.fresh_available {
                Source::Fresh
            } else {
                Source::None
            };
            let actual = Decision {
                selected_source,
                snapshot_roster_origin: selected_source == Source::Snapshot,
                snapshot_commit_qc_included: selected_source == Source::Snapshot,
                snapshot_checkpoint_included: selected_source == Source::Snapshot,
                snapshot_stake_included: selected_source == Source::Snapshot
                    && case.snapshot_stake_present
                    && case.snapshot_stake_matches,
                snapshot_cache_insert: snapshot_selected && case.snapshot_cache_key,
                persisted_lookup_called: !snapshot_selected,
                allow_sidecar_arg: if snapshot_selected {
                    SidecarArg::NotCalled
                } else if case.sidecar_quarantined {
                    SidecarArg::Blocked
                } else {
                    SidecarArg::Allowed
                },
                cache_lookup_called: !snapshot_selected
                    && !case.persisted_available
                    && case.fallback_cache_key,
                fresh_selector_called: !snapshot_selected
                    && !case.persisted_available
                    && !(case.cache_hit && case.fallback_cache_key),
                fresh_cache_insert: selected_source == Source::Fresh
                    && case.fallback_cache_key
                    && case.fresh_certified,
            };
            assert_eq!(
                actual, case.expected,
                "{} abstract decision mismatch",
                case.label
            );

            if case.has_snapshot {
                let snapshot_roster = if case.snapshot_roster_nonempty {
                    roster.clone()
                } else {
                    Vec::new()
                };
                let stake_snapshot = match (
                    case.snapshot_stake_present,
                    case.snapshot_stake_matches,
                    case.snapshot_roster_nonempty,
                ) {
                    (true, true, true) => Some(matching_stake.clone()),
                    (true, false, true) => Some(wrong_stake.clone()),
                    _ => None,
                };
                let snapshot = snapshot_roster_test_snapshot(snapshot_roster, stake_snapshot);
                let selection = block_sync_snapshot_roster_selection(&snapshot);
                assert_eq!(
                    selection.is_some(),
                    snapshot_selected,
                    "{} snapshot helper selection mismatch",
                    case.label
                );
                if let Some(selection) = selection {
                    assert_eq!(selection.source, BlockSyncRosterSource::CommitRosterJournal);
                    assert_eq!(selection.roster, roster);
                    assert_eq!(selection.commit_qc.as_ref(), Some(&snapshot.commit_qc));
                    assert_eq!(
                        selection.checkpoint.as_ref(),
                        Some(&snapshot.validator_checkpoint)
                    );
                    assert_eq!(
                        selection.stake_snapshot.is_some(),
                        case.snapshot_stake_present && case.snapshot_stake_matches,
                        "{} snapshot stake inclusion mismatch",
                        case.label
                    );
                }
            }
        }
    }

    #[test]
    fn block_sync_no_roster_formal_gate_matrix() {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        enum Outcome {
            KnownVoteOnly,
            Deferred,
            Dropped,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        enum StatusOutcome {
            None,
            Deferred,
            Dropped,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        struct Decision {
            known_vote_only: bool,
            process_votes: bool,
            clear_missing: bool,
            clear_reason_payload_available: bool,
            fallback_source: BlockSyncNoRosterFallbackSource,
            keep_repair_called: bool,
            deferred: bool,
            maybe_request_called: bool,
            requested_missing: bool,
            failover_called: bool,
            outcome: Outcome,
            status_outcome: StatusOutcome,
            status_reason_roster_missing: bool,
            drop_metrics: bool,
            warn_drop: bool,
            returns_ok: bool,
            continues: bool,
        }

        #[derive(Debug, Clone, Copy)]
        struct Case {
            label: &'static str,
            block_known: bool,
            has_commit_votes: bool,
            cert_hint: bool,
            checkpoint_hint: bool,
            stake_hint: bool,
            roster_snapshot: bool,
            effective_fallback: bool,
            trusted_fallback: bool,
            keep_exact_frontier_repair: bool,
            maybe_request_missing_qc: bool,
            initial_requested: bool,
            expected: Decision,
        }

        let known_vote = Decision {
            known_vote_only: true,
            process_votes: true,
            clear_missing: true,
            clear_reason_payload_available: true,
            fallback_source: BlockSyncNoRosterFallbackSource::None,
            keep_repair_called: false,
            deferred: false,
            maybe_request_called: false,
            requested_missing: false,
            failover_called: false,
            outcome: Outcome::KnownVoteOnly,
            status_outcome: StatusOutcome::None,
            status_reason_roster_missing: false,
            drop_metrics: false,
            warn_drop: false,
            returns_ok: true,
            continues: false,
        };
        let known_drop = Decision {
            known_vote_only: false,
            process_votes: false,
            clear_missing: true,
            clear_reason_payload_available: true,
            fallback_source: BlockSyncNoRosterFallbackSource::None,
            keep_repair_called: false,
            deferred: false,
            maybe_request_called: false,
            requested_missing: false,
            failover_called: false,
            outcome: Outcome::Dropped,
            status_outcome: StatusOutcome::Dropped,
            status_reason_roster_missing: true,
            drop_metrics: true,
            warn_drop: true,
            returns_ok: true,
            continues: false,
        };
        let unknown_deferred_effective = Decision {
            known_vote_only: false,
            process_votes: false,
            clear_missing: false,
            clear_reason_payload_available: false,
            fallback_source: BlockSyncNoRosterFallbackSource::Effective,
            keep_repair_called: true,
            deferred: true,
            maybe_request_called: false,
            requested_missing: false,
            failover_called: false,
            outcome: Outcome::Deferred,
            status_outcome: StatusOutcome::Deferred,
            status_reason_roster_missing: true,
            drop_metrics: false,
            warn_drop: false,
            returns_ok: true,
            continues: false,
        };
        let unknown_deferred_trusted = Decision {
            fallback_source: BlockSyncNoRosterFallbackSource::Trusted,
            ..unknown_deferred_effective
        };
        let unknown_drop_no_fallback = Decision {
            known_vote_only: false,
            process_votes: false,
            clear_missing: false,
            clear_reason_payload_available: false,
            fallback_source: BlockSyncNoRosterFallbackSource::None,
            keep_repair_called: false,
            deferred: false,
            maybe_request_called: false,
            requested_missing: false,
            failover_called: false,
            outcome: Outcome::Dropped,
            status_outcome: StatusOutcome::Dropped,
            status_reason_roster_missing: true,
            drop_metrics: true,
            warn_drop: true,
            returns_ok: true,
            continues: false,
        };
        let unknown_request_effective = Decision {
            fallback_source: BlockSyncNoRosterFallbackSource::Effective,
            keep_repair_called: true,
            maybe_request_called: true,
            requested_missing: true,
            failover_called: true,
            ..unknown_drop_no_fallback
        };
        let unknown_request_trusted = Decision {
            fallback_source: BlockSyncNoRosterFallbackSource::Trusted,
            ..unknown_request_effective
        };
        let unknown_fallback_no_request = Decision {
            fallback_source: BlockSyncNoRosterFallbackSource::Effective,
            keep_repair_called: true,
            maybe_request_called: true,
            ..unknown_drop_no_fallback
        };
        let cases = [
            Case {
                label: "known_vote_no_snapshot",
                block_known: true,
                has_commit_votes: true,
                cert_hint: false,
                checkpoint_hint: false,
                stake_hint: false,
                roster_snapshot: false,
                effective_fallback: false,
                trusted_fallback: false,
                keep_exact_frontier_repair: false,
                maybe_request_missing_qc: false,
                initial_requested: false,
                expected: known_vote,
            },
            Case {
                label: "known_vote_with_snapshot",
                roster_snapshot: true,
                expected: Decision {
                    process_votes: false,
                    ..known_vote
                },
                ..Case {
                    label: "known_vote_no_snapshot",
                    block_known: true,
                    has_commit_votes: true,
                    cert_hint: false,
                    checkpoint_hint: false,
                    stake_hint: false,
                    roster_snapshot: false,
                    effective_fallback: false,
                    trusted_fallback: false,
                    keep_exact_frontier_repair: false,
                    maybe_request_missing_qc: false,
                    initial_requested: false,
                    expected: known_vote,
                }
            },
            Case {
                label: "known_vote_with_qc",
                block_known: true,
                has_commit_votes: true,
                cert_hint: true,
                checkpoint_hint: false,
                stake_hint: false,
                roster_snapshot: false,
                effective_fallback: false,
                trusted_fallback: false,
                keep_exact_frontier_repair: false,
                maybe_request_missing_qc: false,
                initial_requested: false,
                expected: known_drop,
            },
            Case {
                label: "known_vote_with_checkpoint",
                checkpoint_hint: true,
                expected: known_drop,
                ..Case {
                    label: "known_vote_with_qc",
                    block_known: true,
                    has_commit_votes: true,
                    cert_hint: false,
                    checkpoint_hint: false,
                    stake_hint: false,
                    roster_snapshot: false,
                    effective_fallback: false,
                    trusted_fallback: false,
                    keep_exact_frontier_repair: false,
                    maybe_request_missing_qc: false,
                    initial_requested: false,
                    expected: known_drop,
                }
            },
            Case {
                label: "known_vote_with_stake",
                stake_hint: true,
                expected: known_drop,
                ..Case {
                    label: "known_vote_with_qc",
                    block_known: true,
                    has_commit_votes: true,
                    cert_hint: false,
                    checkpoint_hint: false,
                    stake_hint: false,
                    roster_snapshot: false,
                    effective_fallback: false,
                    trusted_fallback: false,
                    keep_exact_frontier_repair: false,
                    maybe_request_missing_qc: false,
                    initial_requested: false,
                    expected: known_drop,
                }
            },
            Case {
                label: "known_no_votes",
                block_known: true,
                has_commit_votes: false,
                cert_hint: false,
                checkpoint_hint: false,
                stake_hint: false,
                roster_snapshot: false,
                effective_fallback: false,
                trusted_fallback: false,
                keep_exact_frontier_repair: false,
                maybe_request_missing_qc: false,
                initial_requested: false,
                expected: known_drop,
            },
            Case {
                label: "unknown_defer_effective",
                block_known: false,
                has_commit_votes: false,
                cert_hint: false,
                checkpoint_hint: false,
                stake_hint: false,
                roster_snapshot: false,
                effective_fallback: true,
                trusted_fallback: false,
                keep_exact_frontier_repair: true,
                maybe_request_missing_qc: false,
                initial_requested: false,
                expected: unknown_deferred_effective,
            },
            Case {
                label: "unknown_defer_trusted",
                effective_fallback: false,
                trusted_fallback: true,
                expected: unknown_deferred_trusted,
                ..Case {
                    label: "unknown_defer_effective",
                    block_known: false,
                    has_commit_votes: false,
                    cert_hint: false,
                    checkpoint_hint: false,
                    stake_hint: false,
                    roster_snapshot: false,
                    effective_fallback: true,
                    trusted_fallback: false,
                    keep_exact_frontier_repair: true,
                    maybe_request_missing_qc: false,
                    initial_requested: false,
                    expected: unknown_deferred_effective,
                }
            },
            Case {
                label: "unknown_request_effective_failover",
                block_known: false,
                has_commit_votes: false,
                cert_hint: false,
                checkpoint_hint: false,
                stake_hint: false,
                roster_snapshot: false,
                effective_fallback: true,
                trusted_fallback: false,
                keep_exact_frontier_repair: false,
                maybe_request_missing_qc: true,
                initial_requested: false,
                expected: unknown_request_effective,
            },
            Case {
                label: "unknown_request_trusted_no_failover",
                effective_fallback: false,
                trusted_fallback: true,
                expected: unknown_request_trusted,
                ..Case {
                    label: "unknown_request_effective_failover",
                    block_known: false,
                    has_commit_votes: false,
                    cert_hint: false,
                    checkpoint_hint: false,
                    stake_hint: false,
                    roster_snapshot: false,
                    effective_fallback: true,
                    trusted_fallback: false,
                    keep_exact_frontier_repair: false,
                    maybe_request_missing_qc: true,
                    initial_requested: false,
                    expected: unknown_request_effective,
                }
            },
            Case {
                label: "unknown_initial_requested_no_fallback",
                block_known: false,
                has_commit_votes: false,
                cert_hint: false,
                checkpoint_hint: false,
                stake_hint: false,
                roster_snapshot: false,
                effective_fallback: false,
                trusted_fallback: false,
                keep_exact_frontier_repair: false,
                maybe_request_missing_qc: false,
                initial_requested: true,
                expected: Decision {
                    requested_missing: true,
                    failover_called: true,
                    ..unknown_drop_no_fallback
                },
            },
            Case {
                label: "unknown_no_fallback",
                block_known: false,
                has_commit_votes: false,
                cert_hint: false,
                checkpoint_hint: false,
                stake_hint: false,
                roster_snapshot: false,
                effective_fallback: false,
                trusted_fallback: false,
                keep_exact_frontier_repair: false,
                maybe_request_missing_qc: false,
                initial_requested: false,
                expected: unknown_drop_no_fallback,
            },
            Case {
                label: "unknown_fallback_no_request",
                block_known: false,
                has_commit_votes: false,
                cert_hint: false,
                checkpoint_hint: false,
                stake_hint: false,
                roster_snapshot: false,
                effective_fallback: true,
                trusted_fallback: false,
                keep_exact_frontier_repair: false,
                maybe_request_missing_qc: false,
                initial_requested: false,
                expected: unknown_fallback_no_request,
            },
            Case {
                label: "unknown_with_qc_drop",
                cert_hint: true,
                expected: unknown_fallback_no_request,
                ..Case {
                    label: "unknown_fallback_no_request",
                    block_known: false,
                    has_commit_votes: false,
                    cert_hint: false,
                    checkpoint_hint: false,
                    stake_hint: false,
                    roster_snapshot: false,
                    effective_fallback: true,
                    trusted_fallback: false,
                    keep_exact_frontier_repair: false,
                    maybe_request_missing_qc: false,
                    initial_requested: false,
                    expected: unknown_fallback_no_request,
                }
            },
            Case {
                label: "unknown_with_votes_drop",
                has_commit_votes: true,
                expected: unknown_fallback_no_request,
                ..Case {
                    label: "unknown_fallback_no_request",
                    block_known: false,
                    has_commit_votes: false,
                    cert_hint: false,
                    checkpoint_hint: false,
                    stake_hint: false,
                    roster_snapshot: false,
                    effective_fallback: true,
                    trusted_fallback: false,
                    keep_exact_frontier_repair: false,
                    maybe_request_missing_qc: false,
                    initial_requested: false,
                    expected: unknown_fallback_no_request,
                }
            },
        ];

        let peer = snapshot_roster_test_peer();
        for case in cases {
            let known_vote_only = block_sync_no_roster_known_vote_only(
                case.block_known,
                case.has_commit_votes,
                case.cert_hint,
                case.checkpoint_hint,
                case.stake_hint,
            );
            let effective_roster = case
                .effective_fallback
                .then(|| vec![peer.clone()])
                .unwrap_or_default();
            let trusted_roster = case
                .trusted_fallback
                .then(|| vec![peer.clone()])
                .unwrap_or_default();
            let (fallback_source, fallback_roster) =
                block_sync_no_roster_fallback_roster(effective_roster, trusted_roster);
            assert_eq!(
                fallback_roster.is_empty(),
                fallback_source == BlockSyncNoRosterFallbackSource::None,
                "{} fallback roster/source mismatch",
                case.label
            );
            let fallback_source = if case.block_known {
                BlockSyncNoRosterFallbackSource::None
            } else {
                fallback_source
            };
            let keep_repair_called = fallback_source != BlockSyncNoRosterFallbackSource::None;
            let deferred = keep_repair_called && case.keep_exact_frontier_repair;
            let maybe_request_called = keep_repair_called && !deferred;
            let requested_missing =
                case.initial_requested || (maybe_request_called && case.maybe_request_missing_qc);
            let outcome = if known_vote_only {
                Outcome::KnownVoteOnly
            } else if deferred {
                Outcome::Deferred
            } else {
                Outcome::Dropped
            };
            let status_outcome = match outcome {
                Outcome::KnownVoteOnly => StatusOutcome::None,
                Outcome::Deferred => StatusOutcome::Deferred,
                Outcome::Dropped => StatusOutcome::Dropped,
            };
            let actual = Decision {
                known_vote_only,
                process_votes: known_vote_only && !case.roster_snapshot,
                clear_missing: case.block_known,
                clear_reason_payload_available: case.block_known,
                fallback_source,
                keep_repair_called,
                deferred,
                maybe_request_called,
                requested_missing,
                failover_called: !case.block_known && requested_missing,
                outcome,
                status_outcome,
                status_reason_roster_missing: status_outcome != StatusOutcome::None,
                drop_metrics: outcome == Outcome::Dropped,
                warn_drop: outcome == Outcome::Dropped,
                returns_ok: true,
                continues: false,
            };
            assert_eq!(actual, case.expected, "{} mismatch", case.label);
        }
    }

    #[test]
    fn block_sync_known_roster_formal_gate_matrix() {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        enum CandidateSource {
            None,
            Incoming,
            Selection,
            Checkpoint,
            Later,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        enum CommitRosterCheckpointKind {
            None,
            SelectionCheckpoint,
            SynthFromCommitQc,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        enum PrepareCommitQcMatchArg {
            NotCalled,
            True,
            False,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        enum ReturnKind {
            Ok,
            Continue,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        struct Decision {
            source_metric_recorded: bool,
            vote_roster_cached: bool,
            checkpoint_recorded: bool,
            commit_roster_prepared: bool,
            commit_roster_persisted: bool,
            commit_roster_checkpoint_kind: CommitRosterCheckpointKind,
            commit_roster_stake_included: bool,
            process_votes: bool,
            candidate_qc_source: CandidateSource,
            redundant_replay: bool,
            prepare_known_qc_work: bool,
            prepare_commit_qc_match_arg: PrepareCommitQcMatchArg,
            enqueue_known_qc_work: bool,
            clear_missing_commit_qc: bool,
            clear_missing_block: bool,
            clear_missing_block_payload_available: bool,
            return_kind: ReturnKind,
            continues: bool,
        }

        #[derive(Debug, Clone, Copy)]
        struct Case {
            label: &'static str,
            block_known: bool,
            incoming_qc: bool,
            selection_commit_qc: bool,
            selection_checkpoint: bool,
            selection_stake: bool,
            checkpoint_converts: bool,
            cached_qc_match: bool,
            local_snapshot_qc_match: bool,
            prepare_work_some: bool,
            cached_commit_qc_available: bool,
            expected: Decision,
        }

        let known_no_qc = Decision {
            source_metric_recorded: true,
            vote_roster_cached: true,
            checkpoint_recorded: false,
            commit_roster_prepared: false,
            commit_roster_persisted: false,
            commit_roster_checkpoint_kind: CommitRosterCheckpointKind::None,
            commit_roster_stake_included: false,
            process_votes: true,
            candidate_qc_source: CandidateSource::None,
            redundant_replay: false,
            prepare_known_qc_work: false,
            prepare_commit_qc_match_arg: PrepareCommitQcMatchArg::NotCalled,
            enqueue_known_qc_work: false,
            clear_missing_commit_qc: false,
            clear_missing_block: true,
            clear_missing_block_payload_available: true,
            return_kind: ReturnKind::Ok,
            continues: false,
        };
        let known_selection_qc = Decision {
            commit_roster_prepared: true,
            commit_roster_persisted: true,
            commit_roster_checkpoint_kind: CommitRosterCheckpointKind::SynthFromCommitQc,
            candidate_qc_source: CandidateSource::Selection,
            prepare_known_qc_work: true,
            prepare_commit_qc_match_arg: PrepareCommitQcMatchArg::True,
            enqueue_known_qc_work: true,
            ..known_no_qc
        };
        let known_incoming_qc = Decision {
            candidate_qc_source: CandidateSource::Incoming,
            prepare_known_qc_work: true,
            prepare_commit_qc_match_arg: PrepareCommitQcMatchArg::False,
            enqueue_known_qc_work: true,
            ..known_no_qc
        };
        let known_checkpoint_only = Decision {
            checkpoint_recorded: true,
            candidate_qc_source: CandidateSource::Checkpoint,
            prepare_known_qc_work: true,
            prepare_commit_qc_match_arg: PrepareCommitQcMatchArg::False,
            enqueue_known_qc_work: true,
            ..known_no_qc
        };
        let cases = [
            Case {
                label: "known_no_qc",
                block_known: true,
                incoming_qc: false,
                selection_commit_qc: false,
                selection_checkpoint: false,
                selection_stake: false,
                checkpoint_converts: false,
                cached_qc_match: false,
                local_snapshot_qc_match: false,
                prepare_work_some: true,
                cached_commit_qc_available: false,
                expected: known_no_qc,
            },
            Case {
                label: "known_incoming_qc",
                incoming_qc: true,
                expected: known_incoming_qc,
                ..Case {
                    label: "known_no_qc",
                    block_known: true,
                    incoming_qc: false,
                    selection_commit_qc: false,
                    selection_checkpoint: false,
                    selection_stake: false,
                    checkpoint_converts: false,
                    cached_qc_match: false,
                    local_snapshot_qc_match: false,
                    prepare_work_some: true,
                    cached_commit_qc_available: false,
                    expected: known_no_qc,
                }
            },
            Case {
                label: "known_selection_qc",
                selection_commit_qc: true,
                expected: known_selection_qc,
                ..Case {
                    label: "known_no_qc",
                    block_known: true,
                    incoming_qc: false,
                    selection_commit_qc: false,
                    selection_checkpoint: false,
                    selection_stake: false,
                    checkpoint_converts: false,
                    cached_qc_match: false,
                    local_snapshot_qc_match: false,
                    prepare_work_some: true,
                    cached_commit_qc_available: false,
                    expected: known_no_qc,
                }
            },
            Case {
                label: "known_checkpoint_only",
                selection_checkpoint: true,
                checkpoint_converts: true,
                expected: known_checkpoint_only,
                ..Case {
                    label: "known_no_qc",
                    block_known: true,
                    incoming_qc: false,
                    selection_commit_qc: false,
                    selection_checkpoint: false,
                    selection_stake: false,
                    checkpoint_converts: false,
                    cached_qc_match: false,
                    local_snapshot_qc_match: false,
                    prepare_work_some: true,
                    cached_commit_qc_available: false,
                    expected: known_no_qc,
                }
            },
            Case {
                label: "known_incoming_preempts_selection",
                incoming_qc: true,
                selection_commit_qc: true,
                expected: Decision {
                    commit_roster_prepared: true,
                    commit_roster_persisted: true,
                    commit_roster_checkpoint_kind: CommitRosterCheckpointKind::SynthFromCommitQc,
                    ..known_incoming_qc
                },
                ..Case {
                    label: "known_no_qc",
                    block_known: true,
                    incoming_qc: false,
                    selection_commit_qc: false,
                    selection_checkpoint: false,
                    selection_stake: false,
                    checkpoint_converts: false,
                    cached_qc_match: false,
                    local_snapshot_qc_match: false,
                    prepare_work_some: true,
                    cached_commit_qc_available: false,
                    expected: known_no_qc,
                }
            },
            Case {
                label: "known_selection_preempts_checkpoint",
                selection_commit_qc: true,
                selection_checkpoint: true,
                checkpoint_converts: true,
                expected: Decision {
                    checkpoint_recorded: true,
                    commit_roster_checkpoint_kind: CommitRosterCheckpointKind::SelectionCheckpoint,
                    ..known_selection_qc
                },
                ..Case {
                    label: "known_no_qc",
                    block_known: true,
                    incoming_qc: false,
                    selection_commit_qc: false,
                    selection_checkpoint: false,
                    selection_stake: false,
                    checkpoint_converts: false,
                    cached_qc_match: false,
                    local_snapshot_qc_match: false,
                    prepare_work_some: true,
                    cached_commit_qc_available: false,
                    expected: known_no_qc,
                }
            },
            Case {
                label: "known_checkpoint_conversion_fails",
                selection_checkpoint: true,
                checkpoint_converts: false,
                expected: Decision {
                    checkpoint_recorded: true,
                    ..known_no_qc
                },
                ..Case {
                    label: "known_no_qc",
                    block_known: true,
                    incoming_qc: false,
                    selection_commit_qc: false,
                    selection_checkpoint: false,
                    selection_stake: false,
                    checkpoint_converts: false,
                    cached_qc_match: false,
                    local_snapshot_qc_match: false,
                    prepare_work_some: true,
                    cached_commit_qc_available: false,
                    expected: known_no_qc,
                }
            },
            Case {
                label: "known_redundant_qc",
                selection_commit_qc: true,
                cached_qc_match: true,
                local_snapshot_qc_match: true,
                expected: Decision {
                    redundant_replay: true,
                    prepare_known_qc_work: false,
                    prepare_commit_qc_match_arg: PrepareCommitQcMatchArg::NotCalled,
                    enqueue_known_qc_work: false,
                    ..known_selection_qc
                },
                ..Case {
                    label: "known_selection_qc",
                    block_known: true,
                    incoming_qc: false,
                    selection_commit_qc: true,
                    selection_checkpoint: false,
                    selection_stake: false,
                    checkpoint_converts: false,
                    cached_qc_match: false,
                    local_snapshot_qc_match: false,
                    prepare_work_some: true,
                    cached_commit_qc_available: false,
                    expected: known_selection_qc,
                }
            },
            Case {
                label: "known_cached_only_replays",
                selection_commit_qc: true,
                cached_qc_match: true,
                local_snapshot_qc_match: false,
                expected: known_selection_qc,
                ..Case {
                    label: "known_selection_qc",
                    block_known: true,
                    incoming_qc: false,
                    selection_commit_qc: true,
                    selection_checkpoint: false,
                    selection_stake: false,
                    checkpoint_converts: false,
                    cached_qc_match: false,
                    local_snapshot_qc_match: false,
                    prepare_work_some: true,
                    cached_commit_qc_available: false,
                    expected: known_selection_qc,
                }
            },
            Case {
                label: "known_snapshot_only_replays",
                selection_commit_qc: true,
                cached_qc_match: false,
                local_snapshot_qc_match: true,
                expected: known_selection_qc,
                ..Case {
                    label: "known_selection_qc",
                    block_known: true,
                    incoming_qc: false,
                    selection_commit_qc: true,
                    selection_checkpoint: false,
                    selection_stake: false,
                    checkpoint_converts: false,
                    cached_qc_match: false,
                    local_snapshot_qc_match: false,
                    prepare_work_some: true,
                    cached_commit_qc_available: false,
                    expected: known_selection_qc,
                }
            },
            Case {
                label: "known_prepare_none",
                selection_commit_qc: true,
                prepare_work_some: false,
                expected: Decision {
                    enqueue_known_qc_work: false,
                    ..known_selection_qc
                },
                ..Case {
                    label: "known_selection_qc",
                    block_known: true,
                    incoming_qc: false,
                    selection_commit_qc: true,
                    selection_checkpoint: false,
                    selection_stake: false,
                    checkpoint_converts: false,
                    cached_qc_match: false,
                    local_snapshot_qc_match: false,
                    prepare_work_some: true,
                    cached_commit_qc_available: false,
                    expected: known_selection_qc,
                }
            },
            Case {
                label: "known_cached_commit_qc",
                selection_commit_qc: true,
                cached_commit_qc_available: true,
                expected: Decision {
                    clear_missing_commit_qc: true,
                    ..known_selection_qc
                },
                ..Case {
                    label: "known_selection_qc",
                    block_known: true,
                    incoming_qc: false,
                    selection_commit_qc: true,
                    selection_checkpoint: false,
                    selection_stake: false,
                    checkpoint_converts: false,
                    cached_qc_match: false,
                    local_snapshot_qc_match: false,
                    prepare_work_some: true,
                    cached_commit_qc_available: false,
                    expected: known_selection_qc,
                }
            },
            Case {
                label: "known_synth_checkpoint_record",
                selection_commit_qc: true,
                expected: known_selection_qc,
                ..Case {
                    label: "known_selection_qc",
                    block_known: true,
                    incoming_qc: false,
                    selection_commit_qc: true,
                    selection_checkpoint: false,
                    selection_stake: false,
                    checkpoint_converts: false,
                    cached_qc_match: false,
                    local_snapshot_qc_match: false,
                    prepare_work_some: true,
                    cached_commit_qc_available: false,
                    expected: known_selection_qc,
                }
            },
            Case {
                label: "known_checkpoint_record",
                selection_commit_qc: true,
                selection_checkpoint: true,
                expected: Decision {
                    checkpoint_recorded: true,
                    commit_roster_checkpoint_kind: CommitRosterCheckpointKind::SelectionCheckpoint,
                    ..known_selection_qc
                },
                ..Case {
                    label: "known_selection_qc",
                    block_known: true,
                    incoming_qc: false,
                    selection_commit_qc: true,
                    selection_checkpoint: false,
                    selection_stake: false,
                    checkpoint_converts: false,
                    cached_qc_match: false,
                    local_snapshot_qc_match: false,
                    prepare_work_some: true,
                    cached_commit_qc_available: false,
                    expected: known_selection_qc,
                }
            },
            Case {
                label: "known_stake_record",
                selection_commit_qc: true,
                selection_stake: true,
                expected: Decision {
                    commit_roster_stake_included: true,
                    ..known_selection_qc
                },
                ..Case {
                    label: "known_selection_qc",
                    block_known: true,
                    incoming_qc: false,
                    selection_commit_qc: true,
                    selection_checkpoint: false,
                    selection_stake: false,
                    checkpoint_converts: false,
                    cached_qc_match: false,
                    local_snapshot_qc_match: false,
                    prepare_work_some: true,
                    cached_commit_qc_available: false,
                    expected: known_selection_qc,
                }
            },
            Case {
                label: "unknown_selected",
                block_known: false,
                selection_commit_qc: true,
                selection_checkpoint: true,
                selection_stake: true,
                expected: Decision {
                    checkpoint_recorded: true,
                    commit_roster_prepared: true,
                    process_votes: false,
                    candidate_qc_source: CandidateSource::Later,
                    clear_missing_block: false,
                    clear_missing_block_payload_available: false,
                    return_kind: ReturnKind::Continue,
                    continues: true,
                    ..known_no_qc
                },
                ..Case {
                    label: "known_no_qc",
                    block_known: true,
                    incoming_qc: false,
                    selection_commit_qc: false,
                    selection_checkpoint: false,
                    selection_stake: false,
                    checkpoint_converts: false,
                    cached_qc_match: false,
                    local_snapshot_qc_match: false,
                    prepare_work_some: true,
                    cached_commit_qc_available: false,
                    expected: known_no_qc,
                }
            },
        ];

        let incoming_qc =
            snapshot_roster_test_snapshot(vec![snapshot_roster_test_peer()], None).commit_qc;
        let selection_qc =
            snapshot_roster_test_snapshot(vec![snapshot_roster_test_peer()], None).commit_qc;
        let checkpoint_qc =
            snapshot_roster_test_snapshot(vec![snapshot_roster_test_peer()], None).commit_qc;

        for case in cases {
            let helper_result = case.block_known.then(|| {
                block_sync_known_roster_candidate_qc(
                    case.incoming_qc.then(|| incoming_qc.clone()),
                    case.selection_commit_qc.then(|| selection_qc.clone()),
                    case.checkpoint_converts.then(|| checkpoint_qc.clone()),
                )
            });
            let candidate_qc_source = match helper_result.as_ref().and_then(Option::as_ref) {
                Some((BlockSyncKnownRosterCandidateQcSource::Incoming, qc)) => {
                    assert_eq!(HashOf::new(qc), HashOf::new(&incoming_qc));
                    CandidateSource::Incoming
                }
                Some((BlockSyncKnownRosterCandidateQcSource::Selection, qc)) => {
                    assert_eq!(HashOf::new(qc), HashOf::new(&selection_qc));
                    CandidateSource::Selection
                }
                Some((BlockSyncKnownRosterCandidateQcSource::Checkpoint, qc)) => {
                    assert_eq!(HashOf::new(qc), HashOf::new(&checkpoint_qc));
                    CandidateSource::Checkpoint
                }
                None if case.block_known => CandidateSource::None,
                None => CandidateSource::Later,
            };
            let commit_roster_persisted = case.block_known && case.selection_commit_qc;
            let commit_roster_checkpoint_kind = if !commit_roster_persisted {
                CommitRosterCheckpointKind::None
            } else if case.selection_checkpoint {
                CommitRosterCheckpointKind::SelectionCheckpoint
            } else {
                CommitRosterCheckpointKind::SynthFromCommitQc
            };
            let redundant_replay = case.block_known
                && candidate_qc_source != CandidateSource::None
                && candidate_qc_source != CandidateSource::Later
                && case.cached_qc_match
                && case.local_snapshot_qc_match;
            let prepare_known_qc_work = case.block_known
                && candidate_qc_source != CandidateSource::None
                && candidate_qc_source != CandidateSource::Later
                && !redundant_replay;
            let prepare_commit_qc_match_arg = if !prepare_known_qc_work {
                PrepareCommitQcMatchArg::NotCalled
            } else if candidate_qc_source == CandidateSource::Selection {
                PrepareCommitQcMatchArg::True
            } else {
                PrepareCommitQcMatchArg::False
            };
            let actual = Decision {
                source_metric_recorded: true,
                vote_roster_cached: true,
                checkpoint_recorded: case.selection_checkpoint,
                commit_roster_prepared: case.selection_commit_qc,
                commit_roster_persisted,
                commit_roster_checkpoint_kind,
                commit_roster_stake_included: commit_roster_persisted && case.selection_stake,
                process_votes: case.block_known,
                candidate_qc_source,
                redundant_replay,
                prepare_known_qc_work,
                prepare_commit_qc_match_arg,
                enqueue_known_qc_work: prepare_known_qc_work && case.prepare_work_some,
                clear_missing_commit_qc: case.block_known && case.cached_commit_qc_available,
                clear_missing_block: case.block_known,
                clear_missing_block_payload_available: case.block_known,
                return_kind: if case.block_known {
                    ReturnKind::Ok
                } else {
                    ReturnKind::Continue
                },
                continues: !case.block_known,
            };
            assert_eq!(actual, case.expected, "{} mismatch", case.label);
        }
    }

    #[test]
    fn block_sync_known_selected_roster_formal_gate_matrix() {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        enum CandidateSource {
            None,
            Incoming,
            Selection,
            Checkpoint,
            Later,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        enum CommitRosterCheckpointKind {
            None,
            SelectionCheckpoint,
            SynthFromCommitQc,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        enum ReturnKind {
            Ok,
            Continue,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        struct Decision {
            source_metric_recorded: bool,
            vote_roster_cached: bool,
            checkpoint_recorded: bool,
            commit_roster_prepared: bool,
            commit_roster_persisted: bool,
            commit_roster_checkpoint_kind: CommitRosterCheckpointKind,
            commit_roster_stake_included: bool,
            process_votes: bool,
            candidate_qc_source: CandidateSource,
            redundant_replay: bool,
            prepare_known_qc_work: bool,
            prepare_commit_qc_match_arg: bool,
            enqueue_known_qc_work: bool,
            clear_missing_commit_qc: bool,
            clear_missing_block: bool,
            clear_missing_block_payload_available: bool,
            return_kind: ReturnKind,
            continues: bool,
        }

        #[derive(Debug, Clone, Copy)]
        struct Case {
            label: &'static str,
            block_known: bool,
            incoming_qc: bool,
            selection_commit_qc: bool,
            selection_checkpoint: bool,
            selection_stake: bool,
            checkpoint_converts: bool,
            cached_qc_match: bool,
            local_snapshot_qc_match: bool,
            prepare_work_some: bool,
            cached_commit_qc_available: bool,
            expected: Decision,
        }

        let known_no_qc = Decision {
            source_metric_recorded: true,
            vote_roster_cached: true,
            checkpoint_recorded: false,
            commit_roster_prepared: false,
            commit_roster_persisted: false,
            commit_roster_checkpoint_kind: CommitRosterCheckpointKind::None,
            commit_roster_stake_included: false,
            process_votes: true,
            candidate_qc_source: CandidateSource::None,
            redundant_replay: false,
            prepare_known_qc_work: false,
            prepare_commit_qc_match_arg: false,
            enqueue_known_qc_work: false,
            clear_missing_commit_qc: false,
            clear_missing_block: true,
            clear_missing_block_payload_available: true,
            return_kind: ReturnKind::Ok,
            continues: false,
        };
        let known_selection_qc = Decision {
            commit_roster_prepared: true,
            commit_roster_persisted: true,
            commit_roster_checkpoint_kind: CommitRosterCheckpointKind::SynthFromCommitQc,
            candidate_qc_source: CandidateSource::Selection,
            prepare_known_qc_work: true,
            prepare_commit_qc_match_arg: true,
            enqueue_known_qc_work: true,
            ..known_no_qc
        };
        let known_incoming_qc = Decision {
            candidate_qc_source: CandidateSource::Incoming,
            prepare_known_qc_work: true,
            prepare_commit_qc_match_arg: false,
            enqueue_known_qc_work: true,
            ..known_no_qc
        };
        let known_checkpoint_only = Decision {
            checkpoint_recorded: true,
            candidate_qc_source: CandidateSource::Checkpoint,
            prepare_known_qc_work: true,
            prepare_commit_qc_match_arg: false,
            enqueue_known_qc_work: true,
            ..known_no_qc
        };
        let base = Case {
            label: "known_no_qc",
            block_known: true,
            incoming_qc: false,
            selection_commit_qc: false,
            selection_checkpoint: false,
            selection_stake: false,
            checkpoint_converts: false,
            cached_qc_match: false,
            local_snapshot_qc_match: false,
            prepare_work_some: true,
            cached_commit_qc_available: false,
            expected: known_no_qc,
        };
        let cases = [
            Case {
                label: "unknown_selected",
                block_known: false,
                selection_commit_qc: true,
                selection_checkpoint: true,
                selection_stake: true,
                expected: Decision {
                    checkpoint_recorded: true,
                    commit_roster_prepared: true,
                    process_votes: false,
                    candidate_qc_source: CandidateSource::Later,
                    clear_missing_block: false,
                    clear_missing_block_payload_available: false,
                    return_kind: ReturnKind::Continue,
                    continues: true,
                    ..known_no_qc
                },
                ..base
            },
            base,
            Case {
                label: "known_incoming_qc",
                incoming_qc: true,
                expected: known_incoming_qc,
                ..base
            },
            Case {
                label: "known_selection_qc",
                selection_commit_qc: true,
                expected: known_selection_qc,
                ..base
            },
            Case {
                label: "known_checkpoint_only",
                selection_checkpoint: true,
                checkpoint_converts: true,
                expected: known_checkpoint_only,
                ..base
            },
            Case {
                label: "known_incoming_preempts_selection",
                incoming_qc: true,
                selection_commit_qc: true,
                expected: Decision {
                    commit_roster_prepared: true,
                    commit_roster_persisted: true,
                    commit_roster_checkpoint_kind: CommitRosterCheckpointKind::SynthFromCommitQc,
                    ..known_incoming_qc
                },
                ..base
            },
            Case {
                label: "known_selection_preempts_checkpoint",
                selection_commit_qc: true,
                selection_checkpoint: true,
                checkpoint_converts: true,
                expected: Decision {
                    checkpoint_recorded: true,
                    commit_roster_checkpoint_kind: CommitRosterCheckpointKind::SelectionCheckpoint,
                    ..known_selection_qc
                },
                ..base
            },
            Case {
                label: "known_checkpoint_conversion_fails",
                selection_checkpoint: true,
                checkpoint_converts: false,
                expected: Decision {
                    checkpoint_recorded: true,
                    ..known_no_qc
                },
                ..base
            },
            Case {
                label: "known_redundant_qc",
                selection_commit_qc: true,
                cached_qc_match: true,
                local_snapshot_qc_match: true,
                expected: Decision {
                    redundant_replay: true,
                    prepare_known_qc_work: false,
                    prepare_commit_qc_match_arg: false,
                    enqueue_known_qc_work: false,
                    clear_missing_commit_qc: true,
                    ..known_selection_qc
                },
                ..base
            },
            Case {
                label: "known_cached_only_replays",
                selection_commit_qc: true,
                cached_qc_match: true,
                expected: Decision {
                    clear_missing_commit_qc: true,
                    ..known_selection_qc
                },
                ..base
            },
            Case {
                label: "known_snapshot_only_replays",
                selection_commit_qc: true,
                local_snapshot_qc_match: true,
                expected: Decision {
                    clear_missing_commit_qc: true,
                    ..known_selection_qc
                },
                ..base
            },
            Case {
                label: "known_prepare_none",
                selection_commit_qc: true,
                prepare_work_some: false,
                expected: Decision {
                    enqueue_known_qc_work: false,
                    ..known_selection_qc
                },
                ..base
            },
            Case {
                label: "known_cached_commit_qc",
                cached_commit_qc_available: true,
                expected: Decision {
                    clear_missing_commit_qc: true,
                    ..known_no_qc
                },
                ..base
            },
            Case {
                label: "known_synth_checkpoint_record",
                selection_commit_qc: true,
                expected: known_selection_qc,
                ..base
            },
            Case {
                label: "known_checkpoint_record",
                selection_commit_qc: true,
                selection_checkpoint: true,
                checkpoint_converts: true,
                expected: Decision {
                    checkpoint_recorded: true,
                    commit_roster_checkpoint_kind: CommitRosterCheckpointKind::SelectionCheckpoint,
                    ..known_selection_qc
                },
                ..base
            },
            Case {
                label: "known_stake_record",
                selection_commit_qc: true,
                selection_stake: true,
                expected: Decision {
                    commit_roster_stake_included: true,
                    ..known_selection_qc
                },
                ..base
            },
        ];

        let incoming_qc =
            snapshot_roster_test_snapshot(vec![snapshot_roster_test_peer()], None).commit_qc;
        let selection_qc =
            snapshot_roster_test_snapshot(vec![snapshot_roster_test_peer()], None).commit_qc;
        let checkpoint_qc =
            snapshot_roster_test_snapshot(vec![snapshot_roster_test_peer()], None).commit_qc;

        for case in cases {
            let candidate_qc_source = if case.block_known {
                match block_sync_known_roster_candidate_qc(
                    case.incoming_qc.then(|| incoming_qc.clone()),
                    case.selection_commit_qc.then(|| selection_qc.clone()),
                    case.checkpoint_converts.then(|| checkpoint_qc.clone()),
                ) {
                    Some((BlockSyncKnownRosterCandidateQcSource::Incoming, qc)) => {
                        assert_eq!(HashOf::new(&qc), HashOf::new(&incoming_qc));
                        CandidateSource::Incoming
                    }
                    Some((BlockSyncKnownRosterCandidateQcSource::Selection, qc)) => {
                        assert_eq!(HashOf::new(&qc), HashOf::new(&selection_qc));
                        CandidateSource::Selection
                    }
                    Some((BlockSyncKnownRosterCandidateQcSource::Checkpoint, qc)) => {
                        assert_eq!(HashOf::new(&qc), HashOf::new(&checkpoint_qc));
                        CandidateSource::Checkpoint
                    }
                    None => CandidateSource::None,
                }
            } else {
                CandidateSource::Later
            };
            let commit_roster_prepared = case.selection_commit_qc;
            let commit_roster_persisted = case.block_known && commit_roster_prepared;
            let commit_roster_checkpoint_kind = if !commit_roster_persisted {
                CommitRosterCheckpointKind::None
            } else if case.selection_checkpoint {
                CommitRosterCheckpointKind::SelectionCheckpoint
            } else {
                CommitRosterCheckpointKind::SynthFromCommitQc
            };
            let has_known_qc_candidate = matches!(
                candidate_qc_source,
                CandidateSource::Incoming
                    | CandidateSource::Selection
                    | CandidateSource::Checkpoint
            );
            let redundant_replay = case.block_known
                && has_known_qc_candidate
                && case.cached_qc_match
                && case.local_snapshot_qc_match;
            let prepare_known_qc_work =
                case.block_known && has_known_qc_candidate && !redundant_replay;
            let actual = Decision {
                source_metric_recorded: true,
                vote_roster_cached: true,
                checkpoint_recorded: case.selection_checkpoint,
                commit_roster_prepared,
                commit_roster_persisted,
                commit_roster_checkpoint_kind,
                commit_roster_stake_included: commit_roster_persisted && case.selection_stake,
                process_votes: case.block_known,
                candidate_qc_source,
                redundant_replay,
                prepare_known_qc_work,
                prepare_commit_qc_match_arg: prepare_known_qc_work
                    && candidate_qc_source == CandidateSource::Selection,
                enqueue_known_qc_work: prepare_known_qc_work && case.prepare_work_some,
                clear_missing_commit_qc: case.block_known
                    && (case.cached_qc_match
                        || case.local_snapshot_qc_match
                        || case.cached_commit_qc_available),
                clear_missing_block: case.block_known,
                clear_missing_block_payload_available: case.block_known,
                return_kind: if case.block_known {
                    ReturnKind::Ok
                } else {
                    ReturnKind::Continue
                },
                continues: !case.block_known,
            };
            assert_eq!(actual, case.expected, "{} mismatch", case.label);
        }
    }

    #[test]
    fn block_sync_selected_signatures_formal_gate_matrix() {
        use crate::block::SignatureVerificationError;

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        struct Decision {
            uses_cache: bool,
            validates_signers: bool,
            caches_validated_signers: bool,
            signer_set_cached: bool,
            signer_set_validated: bool,
            signer_set_empty: bool,
            signer_set_invalid: bool,
            deferred: bool,
            requests_missing_parent: bool,
            requests_gap: bool,
            request_uses_effective_topology: bool,
            request_carries_selected_roster: bool,
            records_deferred_status: bool,
            records_dropped_status: bool,
            reason_signature_deferred: bool,
            reason_invalid_signature: bool,
            forwards_block_created: bool,
            recovery_payload_only: bool,
            drop_invalid_signature: bool,
            drop_invalid_signature_metric: bool,
            continues: bool,
            proceeds_to_qc_candidate: bool,
            returns_ok: bool,
            clears_missing: bool,
        }

        #[derive(Debug, Clone, Copy)]
        struct Case {
            label: &'static str,
            cache_hit: bool,
            validation_ok: bool,
            cache_key_available: bool,
            parent_missing: bool,
            block_height: u64,
            local_height: u64,
            err: SignatureVerificationError,
            incoming_qc: bool,
            selection_commit_qc: bool,
            checkpoint: bool,
            expected: Decision,
        }

        let empty = Decision {
            uses_cache: false,
            validates_signers: false,
            caches_validated_signers: false,
            signer_set_cached: false,
            signer_set_validated: false,
            signer_set_empty: false,
            signer_set_invalid: false,
            deferred: false,
            requests_missing_parent: false,
            requests_gap: false,
            request_uses_effective_topology: false,
            request_carries_selected_roster: false,
            records_deferred_status: false,
            records_dropped_status: false,
            reason_signature_deferred: false,
            reason_invalid_signature: false,
            forwards_block_created: false,
            recovery_payload_only: false,
            drop_invalid_signature: false,
            drop_invalid_signature_metric: false,
            continues: false,
            proceeds_to_qc_candidate: false,
            returns_ok: false,
            clears_missing: false,
        };
        let cache_hit = Decision {
            uses_cache: true,
            signer_set_cached: true,
            continues: true,
            proceeds_to_qc_candidate: true,
            ..empty
        };
        let validated_without_key = Decision {
            validates_signers: true,
            signer_set_validated: true,
            continues: true,
            proceeds_to_qc_candidate: true,
            ..empty
        };
        let validated_with_key = Decision {
            caches_validated_signers: true,
            ..validated_without_key
        };
        let deferred_parent_only = Decision {
            validates_signers: true,
            deferred: true,
            requests_missing_parent: true,
            request_uses_effective_topology: true,
            request_carries_selected_roster: true,
            records_deferred_status: true,
            reason_signature_deferred: true,
            forwards_block_created: true,
            recovery_payload_only: true,
            returns_ok: true,
            ..empty
        };
        let invalid_drop = Decision {
            validates_signers: true,
            records_dropped_status: true,
            reason_invalid_signature: true,
            drop_invalid_signature: true,
            drop_invalid_signature_metric: true,
            returns_ok: true,
            ..empty
        };
        let roster_evidence_continue = Decision {
            validates_signers: true,
            signer_set_empty: true,
            continues: true,
            proceeds_to_qc_candidate: true,
            ..empty
        };
        let base = Case {
            label: "no_evidence_drop",
            cache_hit: false,
            validation_ok: false,
            cache_key_available: false,
            parent_missing: false,
            block_height: 12,
            local_height: 10,
            err: SignatureVerificationError::Other,
            incoming_qc: false,
            selection_commit_qc: false,
            checkpoint: false,
            expected: invalid_drop,
        };
        let cases = [
            Case {
                label: "cache_hit",
                cache_hit: true,
                cache_key_available: true,
                expected: cache_hit,
                ..base
            },
            Case {
                label: "validated_with_key",
                validation_ok: true,
                cache_key_available: true,
                expected: validated_with_key,
                ..base
            },
            Case {
                label: "validated_without_key",
                validation_ok: true,
                cache_key_available: false,
                expected: validated_without_key,
                ..base
            },
            Case {
                label: "defer_parent_only",
                parent_missing: true,
                block_height: 12,
                local_height: 10,
                err: SignatureVerificationError::UnknownSignature,
                expected: deferred_parent_only,
                ..base
            },
            Case {
                label: "defer_gap",
                parent_missing: true,
                block_height: 13,
                local_height: 10,
                err: SignatureVerificationError::MissingPop,
                expected: Decision {
                    requests_gap: true,
                    ..deferred_parent_only
                },
                ..base
            },
            Case {
                label: "parent_known_invalid",
                parent_missing: false,
                block_height: 12,
                local_height: 10,
                err: SignatureVerificationError::UnknownSignatory,
                expected: invalid_drop,
                ..base
            },
            Case {
                label: "not_ahead_invalid",
                parent_missing: true,
                block_height: 11,
                local_height: 10,
                err: SignatureVerificationError::UnknownSignature,
                expected: invalid_drop,
                ..base
            },
            Case {
                label: "nondefer_error_invalid",
                parent_missing: true,
                block_height: 12,
                local_height: 10,
                err: SignatureVerificationError::Other,
                expected: invalid_drop,
                ..base
            },
            Case {
                label: "incoming_qc_evidence",
                incoming_qc: true,
                expected: roster_evidence_continue,
                ..base
            },
            Case {
                label: "selection_qc_evidence",
                selection_commit_qc: true,
                expected: roster_evidence_continue,
                ..base
            },
            Case {
                label: "checkpoint_evidence",
                checkpoint: true,
                expected: roster_evidence_continue,
                ..base
            },
            base,
        ];

        for case in cases {
            let ahead = block_sync_selected_signatures_ahead_of_frontier(
                case.block_height,
                case.local_height,
            );
            let expected_height = case.local_height.saturating_add(1);
            let error_path = !case.cache_hit && !case.validation_ok;
            let deferred = error_path
                && block_sync_selected_signatures_should_defer(
                    case.parent_missing,
                    ahead,
                    case.err,
                );
            let has_roster_evidence = block_sync_selected_signatures_has_roster_evidence(
                case.incoming_qc,
                case.selection_commit_qc,
                case.checkpoint,
            );
            let invalid_drop = error_path && !deferred && !has_roster_evidence;
            let signer_set_empty = error_path && !deferred && has_roster_evidence;
            let continues = case.cache_hit || case.validation_ok || signer_set_empty;
            let actual = Decision {
                uses_cache: case.cache_hit,
                validates_signers: !case.cache_hit,
                caches_validated_signers: case.validation_ok
                    && block_sync_selected_signatures_should_cache_validated_signers(
                        case.cache_key_available,
                    ),
                signer_set_cached: case.cache_hit,
                signer_set_validated: case.validation_ok,
                signer_set_empty,
                signer_set_invalid: false,
                deferred,
                requests_missing_parent: deferred,
                requests_gap: deferred
                    && block_sync_selected_signatures_should_request_gap(
                        case.block_height,
                        expected_height,
                    ),
                request_uses_effective_topology: deferred,
                request_carries_selected_roster: deferred,
                records_deferred_status: deferred,
                records_dropped_status: invalid_drop,
                reason_signature_deferred: deferred,
                reason_invalid_signature: invalid_drop,
                forwards_block_created: deferred,
                recovery_payload_only: deferred,
                drop_invalid_signature: invalid_drop,
                drop_invalid_signature_metric: invalid_drop,
                continues,
                proceeds_to_qc_candidate: continues,
                returns_ok: deferred || invalid_drop,
                clears_missing: false,
            };
            assert_eq!(actual, case.expected, "{} mismatch", case.label);
        }
    }

    #[test]
    fn block_sync_selected_qc_formal_gate_matrix() {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        enum ShapeCase {
            Valid,
            Height,
            Hash,
            Epoch,
            Phase,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        struct Decision {
            source_incoming: bool,
            source_selection: bool,
            source_checkpoint: bool,
            source_world: bool,
            source_cached: bool,
            candidate_kept: bool,
            validates_candidate: bool,
            aggregate_ok_cached: bool,
            aggregate_ok_selection: bool,
            quarantines_missing_context: bool,
            final_drops_qc: bool,
            qc_replaced_metric: bool,
            derives_cached_qc: bool,
            incoming_qc_validated: bool,
            aggregate_fallback_attempted: bool,
            aggregate_fallback_accepted: bool,
            locked_conflict_drop: bool,
            qc_evidence_present: bool,
            usable_qc_cached: bool,
            quarantine_cleared: bool,
            commit_cert_present: bool,
            invalid_qc_present: bool,
            drops_invalid_payload: bool,
            invalid_payload_returns_ok: bool,
            clears_missing: bool,
        }

        #[derive(Debug, Clone, Copy)]
        struct Case {
            label: &'static str,
            incoming_hint: bool,
            selection_hint: bool,
            checkpoint_hint: bool,
            checkpoint_converts: bool,
            world_available: bool,
            cached_source_available: bool,
            cached_recovery_available: bool,
            shape: ShapeCase,
            validation_ok: bool,
            missing_context_error: bool,
            final_validation_error: bool,
            cached_match: bool,
            selection_match: bool,
            aggregate_fallback_ok: bool,
            hard_lock_conflict: bool,
            block_quorum_met: bool,
        }

        let base = Case {
            label: "incoming_preempts_selection",
            incoming_hint: false,
            selection_hint: false,
            checkpoint_hint: false,
            checkpoint_converts: false,
            world_available: false,
            cached_source_available: false,
            cached_recovery_available: false,
            shape: ShapeCase::Valid,
            validation_ok: false,
            missing_context_error: false,
            final_validation_error: false,
            cached_match: false,
            selection_match: false,
            aggregate_fallback_ok: false,
            hard_lock_conflict: false,
            block_quorum_met: false,
        };
        let cases = [
            Case {
                label: "incoming_preempts_selection",
                incoming_hint: true,
                selection_hint: true,
                validation_ok: true,
                ..base
            },
            Case {
                label: "selection_preempts_checkpoint",
                selection_hint: true,
                checkpoint_hint: true,
                checkpoint_converts: true,
                validation_ok: true,
                selection_match: true,
                ..base
            },
            Case {
                label: "checkpoint_preempts_world",
                checkpoint_hint: true,
                checkpoint_converts: true,
                world_available: true,
                validation_ok: true,
                ..base
            },
            Case {
                label: "world_preempts_cached",
                world_available: true,
                cached_source_available: true,
                validation_ok: true,
                ..base
            },
            Case {
                label: "cached_valid",
                cached_source_available: true,
                validation_ok: true,
                ..base
            },
            Case {
                label: "no_source_cached_recovery",
                cached_recovery_available: true,
                ..base
            },
            Case {
                label: "incoming_shape_height",
                incoming_hint: true,
                shape: ShapeCase::Height,
                ..base
            },
            Case {
                label: "incoming_shape_hash",
                incoming_hint: true,
                shape: ShapeCase::Hash,
                ..base
            },
            Case {
                label: "incoming_shape_epoch",
                incoming_hint: true,
                shape: ShapeCase::Epoch,
                ..base
            },
            Case {
                label: "incoming_shape_phase",
                incoming_hint: true,
                shape: ShapeCase::Phase,
                ..base
            },
            Case {
                label: "incoming_missing_context",
                incoming_hint: true,
                missing_context_error: true,
                ..base
            },
            Case {
                label: "incoming_final_invalid_cached_recovery",
                incoming_hint: true,
                cached_recovery_available: true,
                final_validation_error: true,
                ..base
            },
            Case {
                label: "incoming_final_invalid_aggregate_fallback",
                incoming_hint: true,
                final_validation_error: true,
                aggregate_fallback_ok: true,
                ..base
            },
            Case {
                label: "incoming_final_invalid_no_recovery_drop",
                incoming_hint: true,
                final_validation_error: true,
                ..base
            },
            Case {
                label: "selection_final_invalid_no_recovery",
                selection_hint: true,
                final_validation_error: true,
                ..base
            },
            Case {
                label: "cached_match_skips_aggregate",
                incoming_hint: true,
                validation_ok: true,
                cached_match: true,
                ..base
            },
            Case {
                label: "selection_match_skips_aggregate",
                selection_hint: true,
                validation_ok: true,
                selection_match: true,
                ..base
            },
            Case {
                label: "hard_lock_conflict",
                incoming_hint: true,
                validation_ok: true,
                hard_lock_conflict: true,
                ..base
            },
            Case {
                label: "invalid_qc_block_quorum",
                incoming_hint: true,
                shape: ShapeCase::Height,
                block_quorum_met: true,
                ..base
            },
            Case {
                label: "invalid_qc_checkpoint",
                incoming_hint: true,
                checkpoint_hint: true,
                shape: ShapeCase::Height,
                ..base
            },
        ];

        let block_hash =
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x61; Hash::LENGTH]));
        let other_hash =
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x7a; Hash::LENGTH]));
        let block_height = 5;
        let expected_epoch = 0;
        let mut incoming_qc =
            snapshot_roster_test_snapshot(vec![snapshot_roster_test_peer()], None).commit_qc;
        let selection_qc =
            snapshot_roster_test_snapshot(vec![snapshot_roster_test_peer()], None).commit_qc;
        let checkpoint_qc =
            snapshot_roster_test_snapshot(vec![snapshot_roster_test_peer()], None).commit_qc;
        let world_qc =
            snapshot_roster_test_snapshot(vec![snapshot_roster_test_peer()], None).commit_qc;
        let cached_qc =
            snapshot_roster_test_snapshot(vec![snapshot_roster_test_peer()], None).commit_qc;

        for case in cases {
            incoming_qc.height = block_height;
            incoming_qc.subject_block_hash = block_hash;
            incoming_qc.epoch = expected_epoch;
            incoming_qc.phase = Phase::Commit;
            let mut shaped_incoming_qc = incoming_qc.clone();
            match case.shape {
                ShapeCase::Valid => {}
                ShapeCase::Height => shaped_incoming_qc.height = block_height + 1,
                ShapeCase::Hash => shaped_incoming_qc.subject_block_hash = other_hash,
                ShapeCase::Epoch => shaped_incoming_qc.epoch = expected_epoch + 1,
                ShapeCase::Phase => shaped_incoming_qc.phase = Phase::Prepare,
            }

            let expected_source_incoming = case.incoming_hint;
            let expected_source_selection = !case.incoming_hint && case.selection_hint;
            let expected_source_checkpoint =
                !case.incoming_hint && !case.selection_hint && case.checkpoint_converts;
            let expected_source_world = !case.incoming_hint
                && !case.selection_hint
                && !case.checkpoint_converts
                && case.world_available;
            let expected_source_cached = !case.incoming_hint
                && !case.selection_hint
                && !case.checkpoint_converts
                && !case.world_available
                && case.cached_source_available;
            let expected_any_source = expected_source_incoming
                || expected_source_selection
                || expected_source_checkpoint
                || expected_source_world
                || expected_source_cached;
            let expected_shape_valid = case.shape == ShapeCase::Valid;
            let expected_candidate_kept = expected_any_source && expected_shape_valid;
            let expected_candidate_validated = expected_candidate_kept && case.validation_ok;
            let expected_derives_cached_qc = (!expected_candidate_kept
                || (expected_candidate_kept && !case.validation_ok && case.incoming_hint))
                && case.cached_recovery_available;
            let expected_incoming_qc_validated =
                expected_candidate_validated || expected_derives_cached_qc;
            let expected_aggregate_fallback_attempted =
                case.incoming_hint && !expected_incoming_qc_validated;
            let expected_aggregate_fallback_accepted = expected_aggregate_fallback_attempted
                && expected_candidate_kept
                && case.aggregate_fallback_ok;
            let expected_evidence_before_lock =
                expected_incoming_qc_validated || expected_aggregate_fallback_accepted;
            let expected_locked_conflict_drop =
                expected_evidence_before_lock && case.hard_lock_conflict;
            let expected_qc_evidence_present =
                expected_evidence_before_lock && !case.hard_lock_conflict;
            let expected_usable_qc_cached =
                expected_incoming_qc_validated && !case.hard_lock_conflict;
            let expected_commit_cert_present = super::super::block_sync_commit_cert_present(
                case.selection_hint,
                expected_incoming_qc_validated,
                case.hard_lock_conflict,
            );
            let expected_invalid_qc_present = case.incoming_hint
                && !expected_incoming_qc_validated
                && !expected_qc_evidence_present;
            let expected_drops_invalid_payload = expected_invalid_qc_present
                && !case.block_quorum_met
                && !expected_commit_cert_present
                && !case.checkpoint_hint;
            let expected = Decision {
                source_incoming: expected_source_incoming,
                source_selection: expected_source_selection,
                source_checkpoint: expected_source_checkpoint,
                source_world: expected_source_world,
                source_cached: expected_source_cached,
                candidate_kept: expected_candidate_kept,
                validates_candidate: expected_candidate_kept,
                aggregate_ok_cached: expected_candidate_kept && case.cached_match,
                aggregate_ok_selection: expected_candidate_kept
                    && !case.cached_match
                    && case.selection_match,
                quarantines_missing_context: expected_candidate_kept && case.missing_context_error,
                final_drops_qc: expected_candidate_kept && case.final_validation_error,
                qc_replaced_metric: case.incoming_hint
                    && (case.missing_context_error || case.final_validation_error),
                derives_cached_qc: expected_derives_cached_qc,
                incoming_qc_validated: expected_incoming_qc_validated,
                aggregate_fallback_attempted: expected_aggregate_fallback_attempted,
                aggregate_fallback_accepted: expected_aggregate_fallback_accepted,
                locked_conflict_drop: expected_locked_conflict_drop,
                qc_evidence_present: expected_qc_evidence_present,
                usable_qc_cached: expected_usable_qc_cached,
                quarantine_cleared: expected_usable_qc_cached,
                commit_cert_present: expected_commit_cert_present,
                invalid_qc_present: expected_invalid_qc_present,
                drops_invalid_payload: expected_drops_invalid_payload,
                invalid_payload_returns_ok: expected_drops_invalid_payload,
                clears_missing: false,
            };

            let candidate = block_sync_selected_qc_candidate(
                case.incoming_hint.then(|| shaped_incoming_qc.clone()),
                case.selection_hint.then(|| selection_qc.clone()),
                case.checkpoint_converts.then(|| checkpoint_qc.clone()),
                case.world_available.then(|| world_qc.clone()),
                case.cached_source_available.then(|| cached_qc.clone()),
            );
            let (source, candidate_qc) = match candidate {
                Some((source, qc)) => (Some(source), Some(qc)),
                None => (None, None),
            };
            let candidate_kept = candidate_qc.as_ref().is_some_and(|qc| {
                block_sync_selected_qc_shape(qc, block_hash, block_height, expected_epoch)
                    == BlockSyncSelectedQcShape::Valid
            });
            let validates_candidate = candidate_kept;
            let candidate_validated = candidate_kept && case.validation_ok;
            let derives_cached_qc = block_sync_selected_qc_should_derive_cached(
                candidate_kept,
                candidate_validated,
                case.incoming_hint,
            ) && case.cached_recovery_available;
            let incoming_qc_validated = candidate_validated || derives_cached_qc;
            let aggregate_fallback_attempted =
                block_sync_selected_qc_should_attempt_aggregate_fallback(
                    case.incoming_hint,
                    incoming_qc_validated,
                );
            let aggregate_fallback_accepted =
                block_sync_selected_qc_should_accept_aggregate_fallback(
                    aggregate_fallback_attempted,
                    candidate_kept,
                    case.aggregate_fallback_ok,
                );
            let evidence_before_lock = incoming_qc_validated || aggregate_fallback_accepted;
            let qc_evidence_present = evidence_before_lock && !case.hard_lock_conflict;
            let commit_cert_present = super::super::block_sync_commit_cert_present(
                case.selection_hint,
                incoming_qc_validated,
                case.hard_lock_conflict,
            );
            let invalid_qc_present =
                case.incoming_hint && !incoming_qc_validated && !qc_evidence_present;
            let drops_invalid_payload = block_sync_selected_qc_should_drop_invalid_payload(
                invalid_qc_present,
                case.block_quorum_met,
                commit_cert_present,
                case.checkpoint_hint,
            );
            let aggregate_ok_cached = candidate_kept
                && block_sync_selected_qc_aggregate_ok(case.cached_match, false).is_some();
            let aggregate_ok_selection = candidate_kept
                && !case.cached_match
                && block_sync_selected_qc_aggregate_ok(false, case.selection_match).is_some();
            let actual = Decision {
                source_incoming: source == Some(BlockSyncSelectedQcSource::Incoming),
                source_selection: source == Some(BlockSyncSelectedQcSource::Selection),
                source_checkpoint: source == Some(BlockSyncSelectedQcSource::Checkpoint),
                source_world: source == Some(BlockSyncSelectedQcSource::World),
                source_cached: source == Some(BlockSyncSelectedQcSource::Cached),
                candidate_kept,
                validates_candidate,
                aggregate_ok_cached,
                aggregate_ok_selection,
                quarantines_missing_context: candidate_kept && case.missing_context_error,
                final_drops_qc: candidate_kept && case.final_validation_error,
                qc_replaced_metric: case.incoming_hint
                    && (case.missing_context_error || case.final_validation_error),
                derives_cached_qc,
                incoming_qc_validated,
                aggregate_fallback_attempted,
                aggregate_fallback_accepted,
                locked_conflict_drop: evidence_before_lock && case.hard_lock_conflict,
                qc_evidence_present,
                usable_qc_cached: incoming_qc_validated && !case.hard_lock_conflict,
                quarantine_cleared: incoming_qc_validated && !case.hard_lock_conflict,
                commit_cert_present,
                invalid_qc_present,
                drops_invalid_payload,
                invalid_payload_returns_ok: drops_invalid_payload,
                clears_missing: false,
            };
            assert_eq!(actual, expected, "{} mismatch", case.label);
        }
    }

    #[test]
    fn block_sync_selected_quorum_formal_gate_matrix() {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        struct Decision {
            quorum_initial: bool,
            maybe_request_called: bool,
            requested_after_maybe: bool,
            quorum_after_maybe: bool,
            npos_vote_only_deferred: bool,
            repair_called: bool,
            repair_deferred: bool,
            drop_quorum_missing: bool,
            invalid_qc_drop: bool,
            record_deferred_quorum: bool,
            record_dropped_invalid: bool,
            record_dropped_quorum: bool,
            record_reason_invalid_payload: bool,
            record_reason_quorum_missing: bool,
            drop_invalid_signature_metric: bool,
            returns_ok: bool,
            continues_to_apply: bool,
            clears_missing: bool,
        }

        #[derive(Debug, Clone, Copy)]
        struct Case {
            label: &'static str,
            qc_evidence: bool,
            commit_cert: bool,
            signature_quorum: bool,
            checkpoint: bool,
            invalid_qc: bool,
            explicit_requested: bool,
            requested_at_entry: bool,
            exact_contiguous_frontier: bool,
            frontier_next_height: bool,
            has_block_signer: bool,
            has_commit_votes: bool,
            maybe_request_would_track: bool,
            npos_mode: bool,
            vote_only_frontier: bool,
            repair_kept: bool,
        }

        let base = Case {
            label: "quorum_drop",
            qc_evidence: false,
            commit_cert: false,
            signature_quorum: false,
            checkpoint: false,
            invalid_qc: false,
            explicit_requested: false,
            requested_at_entry: false,
            exact_contiguous_frontier: false,
            frontier_next_height: false,
            has_block_signer: true,
            has_commit_votes: false,
            maybe_request_would_track: false,
            npos_mode: false,
            vote_only_frontier: false,
            repair_kept: false,
        };
        let cases = [
            Case {
                label: "qc_evidence_quorum",
                qc_evidence: true,
                ..base
            },
            Case {
                label: "commit_cert_quorum",
                commit_cert: true,
                ..base
            },
            Case {
                label: "signature_quorum",
                signature_quorum: true,
                ..base
            },
            Case {
                label: "checkpoint_quorum",
                checkpoint: true,
                ..base
            },
            Case {
                label: "explicit_frontier_sparse",
                explicit_requested: true,
                frontier_next_height: true,
                ..base
            },
            Case {
                label: "tracked_frontier_sparse",
                requested_at_entry: true,
                exact_contiguous_frontier: true,
                frontier_next_height: true,
                ..base
            },
            Case {
                label: "frontier_sparse_with_commit_votes",
                requested_at_entry: true,
                exact_contiguous_frontier: true,
                frontier_next_height: true,
                has_commit_votes: true,
                ..base
            },
            Case {
                label: "sparse_no_signers",
                requested_at_entry: true,
                exact_contiguous_frontier: true,
                frontier_next_height: true,
                has_block_signer: false,
                ..base
            },
            Case {
                label: "requested_nonfrontier_no_quorum",
                requested_at_entry: true,
                ..base
            },
            Case {
                label: "unrequested_sparse_no_quorum",
                frontier_next_height: true,
                ..base
            },
            Case {
                label: "missing_qc_request_npos_vote_only",
                frontier_next_height: true,
                maybe_request_would_track: true,
                npos_mode: true,
                vote_only_frontier: true,
                ..base
            },
            Case {
                label: "missing_qc_request_classic_continue",
                frontier_next_height: true,
                maybe_request_would_track: true,
                ..base
            },
            Case {
                label: "missing_qc_request_npos_non_vote_continue",
                frontier_next_height: true,
                maybe_request_would_track: true,
                npos_mode: true,
                ..base
            },
            Case {
                label: "missing_qc_request_nonfrontier_drop",
                maybe_request_would_track: true,
                ..base
            },
            Case {
                label: "missing_qc_request_zero_signer_drop",
                frontier_next_height: true,
                has_block_signer: false,
                maybe_request_would_track: true,
                ..base
            },
            Case {
                label: "missing_qc_request_backoff_repair",
                frontier_next_height: true,
                repair_kept: true,
                ..base
            },
            Case {
                label: "repair_deferred",
                requested_at_entry: true,
                repair_kept: true,
                ..base
            },
            base,
            Case {
                label: "invalid_qc_drop",
                invalid_qc: true,
                ..base
            },
            Case {
                label: "invalid_qc_block_quorum",
                invalid_qc: true,
                signature_quorum: true,
                ..base
            },
            Case {
                label: "invalid_qc_checkpoint",
                invalid_qc: true,
                checkpoint: true,
                ..base
            },
        ];

        let commit_quorum = 2;
        let local_height = 10;

        for case in cases {
            let block_signer_count = if case.signature_quorum {
                commit_quorum
            } else if case.has_block_signer {
                1
            } else {
                0
            };
            let block_height = if case.frontier_next_height {
                local_height + 1
            } else {
                local_height + 2
            };

            let spec_invalid_qc_drop =
                case.invalid_qc && !case.signature_quorum && !case.commit_cert && !case.checkpoint;
            let spec_can_check_quorum = !spec_invalid_qc_drop;
            let spec_sparse_exact_frontier_request = case.requested_at_entry
                && case.exact_contiguous_frontier
                && !case.qc_evidence
                && !case.checkpoint
                && !case.has_commit_votes;
            let spec_initial_missing_request_arg =
                case.explicit_requested || spec_sparse_exact_frontier_request;
            let spec_initial_sparse_quorum = spec_initial_missing_request_arg
                && case.frontier_next_height
                && case.has_block_signer;
            let spec_quorum_initial = case.qc_evidence
                || case.commit_cert
                || case.signature_quorum
                || case.checkpoint
                || spec_initial_sparse_quorum;
            let spec_maybe_request_called = spec_can_check_quorum
                && !spec_quorum_initial
                && !case.qc_evidence
                && !case.commit_cert
                && !case.checkpoint
                && !case.signature_quorum
                && !case.requested_at_entry;
            let spec_maybe_request_tracked =
                spec_maybe_request_called && case.maybe_request_would_track;
            let spec_npos_vote_only_deferred = spec_maybe_request_tracked
                && case.npos_mode
                && case.vote_only_frontier
                && !case.explicit_requested;
            let spec_requested_after_maybe =
                spec_maybe_request_tracked && !spec_npos_vote_only_deferred;
            let spec_quorum_after_maybe = spec_quorum_initial
                || (spec_requested_after_maybe
                    && case.frontier_next_height
                    && case.has_block_signer);
            let spec_repair_called =
                spec_can_check_quorum && !spec_npos_vote_only_deferred && !spec_quorum_after_maybe;
            let spec_repair_deferred = spec_repair_called && case.repair_kept;
            let spec_drop_quorum_missing = spec_repair_called && !case.repair_kept;
            let spec_record_deferred_quorum = spec_npos_vote_only_deferred || spec_repair_deferred;
            let spec_record_dropped_quorum = spec_drop_quorum_missing;
            let expected = Decision {
                quorum_initial: spec_quorum_initial,
                maybe_request_called: spec_maybe_request_called,
                requested_after_maybe: spec_requested_after_maybe,
                quorum_after_maybe: spec_quorum_after_maybe,
                npos_vote_only_deferred: spec_npos_vote_only_deferred,
                repair_called: spec_repair_called,
                repair_deferred: spec_repair_deferred,
                drop_quorum_missing: spec_drop_quorum_missing,
                invalid_qc_drop: spec_invalid_qc_drop,
                record_deferred_quorum: spec_record_deferred_quorum,
                record_dropped_invalid: spec_invalid_qc_drop,
                record_dropped_quorum: spec_record_dropped_quorum,
                record_reason_invalid_payload: spec_invalid_qc_drop,
                record_reason_quorum_missing: spec_record_deferred_quorum
                    || spec_record_dropped_quorum,
                drop_invalid_signature_metric: spec_drop_quorum_missing,
                returns_ok: spec_invalid_qc_drop
                    || spec_npos_vote_only_deferred
                    || spec_repair_deferred
                    || spec_drop_quorum_missing,
                continues_to_apply: spec_can_check_quorum
                    && !spec_npos_vote_only_deferred
                    && spec_quorum_after_maybe,
                clears_missing: false,
            };

            let invalid_qc_drop = block_sync_selected_qc_should_drop_invalid_payload(
                case.invalid_qc,
                case.signature_quorum,
                case.commit_cert,
                case.checkpoint,
            );
            let sparse_exact_frontier_request =
                block_sync_selected_quorum_sparse_exact_frontier_request(
                    case.requested_at_entry,
                    case.exact_contiguous_frontier,
                    case.qc_evidence,
                    case.checkpoint,
                    case.has_commit_votes,
                );
            let initial_missing_request_arg =
                case.explicit_requested || sparse_exact_frontier_request;
            let quorum_initial = super::super::block_sync_quorum_available(
                block_signer_count,
                commit_quorum,
                case.signature_quorum,
                case.qc_evidence,
                case.commit_cert,
                case.checkpoint,
                initial_missing_request_arg,
                block_height,
                local_height,
            );
            let requested_missing_block = case.requested_at_entry || case.explicit_requested;
            let maybe_request_called = !invalid_qc_drop
                && block_sync_selected_quorum_should_maybe_request_missing_qc(
                    quorum_initial,
                    case.qc_evidence,
                    case.commit_cert,
                    case.checkpoint,
                    block_signer_count,
                    commit_quorum,
                    requested_missing_block,
                );
            let maybe_request_tracked = maybe_request_called && case.maybe_request_would_track;
            let npos_vote_only_deferred = maybe_request_tracked
                && block_sync_selected_quorum_should_defer_npos_vote_only(
                    case.npos_mode,
                    case.vote_only_frontier,
                    case.explicit_requested,
                );
            let requested_after_maybe = maybe_request_tracked && !npos_vote_only_deferred;
            let quorum_after_maybe = quorum_initial
                || (requested_after_maybe
                    && super::super::block_sync_quorum_available(
                        block_signer_count,
                        commit_quorum,
                        case.signature_quorum,
                        case.qc_evidence,
                        case.commit_cert,
                        case.checkpoint,
                        true,
                        block_height,
                        local_height,
                    ));
            let repair_called = !invalid_qc_drop
                && !npos_vote_only_deferred
                && block_sync_selected_quorum_should_call_repair(quorum_after_maybe);
            let repair_deferred = repair_called && case.repair_kept;
            let drop_quorum_missing = repair_called && !case.repair_kept;
            let record_deferred_quorum = npos_vote_only_deferred || repair_deferred;
            let record_dropped_quorum = drop_quorum_missing;
            let actual = Decision {
                quorum_initial,
                maybe_request_called,
                requested_after_maybe,
                quorum_after_maybe,
                npos_vote_only_deferred,
                repair_called,
                repair_deferred,
                drop_quorum_missing,
                invalid_qc_drop,
                record_deferred_quorum,
                record_dropped_invalid: invalid_qc_drop,
                record_dropped_quorum,
                record_reason_invalid_payload: invalid_qc_drop,
                record_reason_quorum_missing: record_deferred_quorum || record_dropped_quorum,
                drop_invalid_signature_metric: drop_quorum_missing,
                returns_ok: invalid_qc_drop
                    || npos_vote_only_deferred
                    || repair_deferred
                    || drop_quorum_missing,
                continues_to_apply: !invalid_qc_drop
                    && !npos_vote_only_deferred
                    && quorum_after_maybe,
                clears_missing: false,
            };
            assert_eq!(actual, expected, "{} mismatch", case.label);
        }
    }

    #[test]
    fn block_sync_selected_apply_formal_gate_matrix() {
        #[derive(Debug, Clone, Copy)]
        enum Candidate {
            SelectionCommitQcAllowsNonextending,
            IncomingQcValidAllowsNonextending,
            IncomingQcUsableAllowsNonextending,
            NoQcDisallowsNonextending,
            SameHeightSignatureConflict,
            SignatureQuorumSupersedesWithoutConflict,
            IncomingQcSupersedes,
            CommitCertSupersedes,
            CheckpointSupersedes,
            PayloadOnlyNoAuthority,
            CommitVotesRecovery,
            IncomingQcRecovery,
            CommitCertRecovery,
            CheckpointRecovery,
            SignedQuorumFrontierRepair,
            PreservePayloadMismatch,
            SignedQuorumCommitRepairActive,
            SignedQuorumRepairCreationError,
            SignedQuorumRepairUnknownAfter,
            SignedQuorumRepairNoSignatureQuorum,
            SignedQuorumRepairNotFrontier,
            SignedQuorumRepairHasQc,
            SparseNextHeightRecovery,
            SparseKnownBefore,
            SparseUnknownAfter,
            SparseHasCommitQuorum,
            ReadyForQc,
            NotReadyCreationError,
            NotReadyUnknownAfter,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        struct Decision {
            allow_nonextending_qc: bool,
            same_height_frontier_conflict: bool,
            preserve_on_payload_mismatch: bool,
            authoritative_supersede: bool,
            recovery_commit_evidence: bool,
            recovery_signed_quorum: bool,
            recovery_payload_only: bool,
            observed_epoch_incoming: bool,
            observed_epoch_checkpoint: bool,
            observed_epoch_none: bool,
            allow_aborted_revival: bool,
            handle_created_called: bool,
            pass_preserve_flag: bool,
            pass_commit_evidence_mode: bool,
            pass_signed_quorum_mode: bool,
            pass_payload_only_mode: bool,
            signed_quorum_commit_repair_active: bool,
            pending_commit_qc_observed: bool,
            frontier_commit_qc_observed: bool,
            clear_missing_commit_qc_request: bool,
            request_commit_pipeline: bool,
            sparse_next_height_payload_recovered: bool,
            request_known_block_commit_qc_recovery: bool,
            ready_for_qc: bool,
            record_payload_unapplied_drop: bool,
            process_commit_votes: bool,
            qc_to_apply: bool,
        }

        fn selection_commit_qc(candidate: Candidate) -> bool {
            matches!(candidate, Candidate::SelectionCommitQcAllowsNonextending)
        }

        fn incoming_qc_validated(candidate: Candidate) -> bool {
            matches!(candidate, Candidate::IncomingQcValidAllowsNonextending)
        }

        fn incoming_qc_usable(candidate: Candidate) -> bool {
            matches!(
                candidate,
                Candidate::IncomingQcUsableAllowsNonextending
                    | Candidate::IncomingQcSupersedes
                    | Candidate::IncomingQcRecovery
            )
        }

        fn has_commit_votes(candidate: Candidate) -> bool {
            matches!(candidate, Candidate::CommitVotesRecovery)
        }

        fn commit_cert_present(candidate: Candidate) -> bool {
            matches!(
                candidate,
                Candidate::CommitCertSupersedes | Candidate::CommitCertRecovery
            )
        }

        fn checkpoint_present(candidate: Candidate) -> bool {
            matches!(
                candidate,
                Candidate::CheckpointSupersedes | Candidate::CheckpointRecovery
            )
        }

        fn incoming_qc_object(candidate: Candidate) -> bool {
            incoming_qc_validated(candidate)
                || incoming_qc_usable(candidate)
                || commit_cert_present(candidate)
        }

        fn qc_evidence_present(candidate: Candidate) -> bool {
            incoming_qc_object(candidate)
                || matches!(
                    candidate,
                    Candidate::ReadyForQc | Candidate::SignedQuorumRepairHasQc
                )
        }

        fn block_quorum_met(candidate: Candidate) -> bool {
            matches!(
                candidate,
                Candidate::SameHeightSignatureConflict
                    | Candidate::SignatureQuorumSupersedesWithoutConflict
                    | Candidate::SignedQuorumFrontierRepair
            )
        }

        fn local_conflicting_frontier_vote(candidate: Candidate) -> bool {
            matches!(candidate, Candidate::SameHeightSignatureConflict)
        }

        fn signature_quorum_met(candidate: Candidate) -> bool {
            matches!(
                candidate,
                Candidate::SignedQuorumFrontierRepair
                    | Candidate::SignedQuorumCommitRepairActive
                    | Candidate::SignedQuorumRepairCreationError
                    | Candidate::SignedQuorumRepairUnknownAfter
                    | Candidate::SignedQuorumRepairNotFrontier
                    | Candidate::SignedQuorumRepairHasQc
            )
        }

        fn exact_contiguous_frontier(candidate: Candidate) -> bool {
            matches!(
                candidate,
                Candidate::SignedQuorumFrontierRepair
                    | Candidate::SignedQuorumCommitRepairActive
                    | Candidate::SignedQuorumRepairCreationError
                    | Candidate::SignedQuorumRepairUnknownAfter
                    | Candidate::SignedQuorumRepairNoSignatureQuorum
                    | Candidate::SignedQuorumRepairHasQc
                    | Candidate::SparseNextHeightRecovery
                    | Candidate::SparseKnownBefore
                    | Candidate::SparseUnknownAfter
                    | Candidate::SparseHasCommitQuorum
            )
        }

        fn missing_commit_qc_repair_active(candidate: Candidate) -> bool {
            matches!(
                candidate,
                Candidate::SignedQuorumCommitRepairActive
                    | Candidate::SignedQuorumRepairCreationError
                    | Candidate::SignedQuorumRepairUnknownAfter
                    | Candidate::SignedQuorumRepairNoSignatureQuorum
                    | Candidate::SignedQuorumRepairNotFrontier
                    | Candidate::SignedQuorumRepairHasQc
            )
        }

        fn creation_ok(candidate: Candidate) -> bool {
            !matches!(
                candidate,
                Candidate::SignedQuorumRepairCreationError | Candidate::NotReadyCreationError
            )
        }

        fn block_known_before(candidate: Candidate) -> bool {
            matches!(candidate, Candidate::SparseKnownBefore)
        }

        fn block_known_after(candidate: Candidate) -> bool {
            !matches!(
                candidate,
                Candidate::SignedQuorumRepairUnknownAfter
                    | Candidate::SparseUnknownAfter
                    | Candidate::NotReadyUnknownAfter
            )
        }

        fn next_height(candidate: Candidate) -> bool {
            matches!(
                candidate,
                Candidate::SparseNextHeightRecovery
                    | Candidate::SparseKnownBefore
                    | Candidate::SparseUnknownAfter
                    | Candidate::SparseHasCommitQuorum
            )
        }

        fn block_signer_below_commit_quorum(candidate: Candidate) -> bool {
            !matches!(candidate, Candidate::SparseHasCommitQuorum)
        }

        fn pending_block_matches_non_invalid(candidate: Candidate) -> bool {
            matches!(candidate, Candidate::SignedQuorumCommitRepairActive)
        }

        let candidates = [
            Candidate::SelectionCommitQcAllowsNonextending,
            Candidate::IncomingQcValidAllowsNonextending,
            Candidate::IncomingQcUsableAllowsNonextending,
            Candidate::NoQcDisallowsNonextending,
            Candidate::SameHeightSignatureConflict,
            Candidate::SignatureQuorumSupersedesWithoutConflict,
            Candidate::IncomingQcSupersedes,
            Candidate::CommitCertSupersedes,
            Candidate::CheckpointSupersedes,
            Candidate::PayloadOnlyNoAuthority,
            Candidate::CommitVotesRecovery,
            Candidate::IncomingQcRecovery,
            Candidate::CommitCertRecovery,
            Candidate::CheckpointRecovery,
            Candidate::SignedQuorumFrontierRepair,
            Candidate::PreservePayloadMismatch,
            Candidate::SignedQuorumCommitRepairActive,
            Candidate::SignedQuorumRepairCreationError,
            Candidate::SignedQuorumRepairUnknownAfter,
            Candidate::SignedQuorumRepairNoSignatureQuorum,
            Candidate::SignedQuorumRepairNotFrontier,
            Candidate::SignedQuorumRepairHasQc,
            Candidate::SparseNextHeightRecovery,
            Candidate::SparseKnownBefore,
            Candidate::SparseUnknownAfter,
            Candidate::SparseHasCommitQuorum,
            Candidate::ReadyForQc,
            Candidate::NotReadyCreationError,
            Candidate::NotReadyUnknownAfter,
        ];

        let expected_epoch = 7;
        let observed_incoming_epoch = 42;
        let commit_quorum = 2;

        for candidate in candidates {
            let spec_allow_nonextending_qc = selection_commit_qc(candidate)
                || incoming_qc_validated(candidate)
                || incoming_qc_usable(candidate);
            let spec_same_height_frontier_conflict = block_quorum_met(candidate)
                && !incoming_qc_usable(candidate)
                && !commit_cert_present(candidate)
                && !checkpoint_present(candidate)
                && local_conflicting_frontier_vote(candidate);
            let spec_preserve_on_payload_mismatch = !incoming_qc_usable(candidate)
                && !commit_cert_present(candidate)
                && !checkpoint_present(candidate);
            let spec_authoritative_supersede = incoming_qc_usable(candidate)
                || commit_cert_present(candidate)
                || checkpoint_present(candidate)
                || (block_quorum_met(candidate) && !spec_same_height_frontier_conflict);
            let spec_recovery_commit_evidence = has_commit_votes(candidate)
                || incoming_qc_usable(candidate)
                || commit_cert_present(candidate)
                || checkpoint_present(candidate);
            let spec_recovery_signed_quorum =
                !spec_recovery_commit_evidence && spec_authoritative_supersede;
            let spec_recovery_payload_only =
                !spec_recovery_commit_evidence && !spec_recovery_signed_quorum;
            let spec_observed_epoch_incoming =
                spec_recovery_commit_evidence && incoming_qc_object(candidate);
            let spec_observed_epoch_checkpoint = spec_recovery_commit_evidence
                && !incoming_qc_object(candidate)
                && checkpoint_present(candidate);
            let spec_observed_epoch_none = spec_recovery_commit_evidence
                && !incoming_qc_object(candidate)
                && !checkpoint_present(candidate);
            let spec_allow_aborted_revival = spec_recovery_commit_evidence
                && (has_commit_votes(candidate)
                    || commit_cert_present(candidate)
                    || checkpoint_present(candidate));
            let spec_signed_quorum_commit_repair_active = creation_ok(candidate)
                && block_known_after(candidate)
                && signature_quorum_met(candidate)
                && exact_contiguous_frontier(candidate)
                && !qc_evidence_present(candidate)
                && !commit_cert_present(candidate)
                && !checkpoint_present(candidate)
                && missing_commit_qc_repair_active(candidate);
            let spec_pending_commit_qc_observed = spec_signed_quorum_commit_repair_active
                && pending_block_matches_non_invalid(candidate);
            let spec_sparse_next_height_payload_recovered = !block_known_before(candidate)
                && block_known_after(candidate)
                && next_height(candidate)
                && block_signer_below_commit_quorum(candidate)
                && !incoming_qc_usable(candidate)
                && !commit_cert_present(candidate)
                && !checkpoint_present(candidate);
            let spec_ready_for_qc = creation_ok(candidate) && block_known_after(candidate);
            let expected = Decision {
                allow_nonextending_qc: spec_allow_nonextending_qc,
                same_height_frontier_conflict: spec_same_height_frontier_conflict,
                preserve_on_payload_mismatch: spec_preserve_on_payload_mismatch,
                authoritative_supersede: spec_authoritative_supersede,
                recovery_commit_evidence: spec_recovery_commit_evidence,
                recovery_signed_quorum: spec_recovery_signed_quorum,
                recovery_payload_only: spec_recovery_payload_only,
                observed_epoch_incoming: spec_observed_epoch_incoming,
                observed_epoch_checkpoint: spec_observed_epoch_checkpoint,
                observed_epoch_none: spec_observed_epoch_none,
                allow_aborted_revival: spec_allow_aborted_revival,
                handle_created_called: true,
                pass_preserve_flag: spec_preserve_on_payload_mismatch,
                pass_commit_evidence_mode: spec_recovery_commit_evidence,
                pass_signed_quorum_mode: spec_recovery_signed_quorum,
                pass_payload_only_mode: spec_recovery_payload_only,
                signed_quorum_commit_repair_active: spec_signed_quorum_commit_repair_active,
                pending_commit_qc_observed: spec_pending_commit_qc_observed,
                frontier_commit_qc_observed: spec_signed_quorum_commit_repair_active,
                clear_missing_commit_qc_request: spec_signed_quorum_commit_repair_active,
                request_commit_pipeline: spec_signed_quorum_commit_repair_active,
                sparse_next_height_payload_recovered: spec_sparse_next_height_payload_recovered,
                request_known_block_commit_qc_recovery: spec_sparse_next_height_payload_recovered,
                ready_for_qc: spec_ready_for_qc,
                record_payload_unapplied_drop: !spec_ready_for_qc,
                process_commit_votes: true,
                qc_to_apply: spec_ready_for_qc && qc_evidence_present(candidate),
            };

            let same_height_frontier_conflict =
                block_sync_selected_apply_same_height_frontier_conflict(
                    block_quorum_met(candidate),
                    incoming_qc_usable(candidate),
                    commit_cert_present(candidate),
                    checkpoint_present(candidate),
                    local_conflicting_frontier_vote(candidate),
                );
            let preserve_on_payload_mismatch =
                block_sync_selected_apply_preserve_on_payload_mismatch(
                    incoming_qc_usable(candidate),
                    commit_cert_present(candidate),
                    checkpoint_present(candidate),
                );
            let authoritative_supersede = block_sync_selected_apply_authoritative_supersede(
                incoming_qc_usable(candidate),
                commit_cert_present(candidate),
                checkpoint_present(candidate),
                block_quorum_met(candidate),
                same_height_frontier_conflict,
            );
            let recovery_mode = block_sync_selected_apply_recovery_mode(
                has_commit_votes(candidate),
                incoming_qc_usable(candidate),
                commit_cert_present(candidate),
                checkpoint_present(candidate),
                incoming_qc_object(candidate).then_some(observed_incoming_epoch),
                expected_epoch,
                authoritative_supersede,
            );
            let (
                recovery_commit_evidence,
                recovery_signed_quorum,
                recovery_payload_only,
                observed_commit_qc_epoch,
                allow_aborted_revival,
            ) = match recovery_mode {
                BlockSyncRecoveryMode::CommitEvidenceRepair {
                    observed_commit_qc_epoch,
                    allow_aborted_revival_without_local_commit_qc,
                } => (
                    true,
                    false,
                    false,
                    observed_commit_qc_epoch,
                    allow_aborted_revival_without_local_commit_qc,
                ),
                BlockSyncRecoveryMode::SignedQuorumFrontierRepair => {
                    (false, true, false, None, false)
                }
                BlockSyncRecoveryMode::PayloadOnly => (false, false, true, None, false),
                BlockSyncRecoveryMode::RequestedPayloadRepair => (false, false, false, None, false),
            };
            let signed_quorum_commit_repair_active =
                block_sync_selected_apply_signed_quorum_commit_repair_active(
                    BlockSyncSelectedApplySignedQuorumRepair {
                        creation_ok: creation_ok(candidate),
                        block_known_after_creation: block_known_after(candidate),
                        signature_quorum_met: signature_quorum_met(candidate),
                        exact_contiguous_frontier: exact_contiguous_frontier(candidate),
                        qc_evidence_present: qc_evidence_present(candidate),
                        commit_cert_present: commit_cert_present(candidate),
                        checkpoint_present: checkpoint_present(candidate),
                        missing_commit_qc_repair_active: missing_commit_qc_repair_active(candidate),
                    },
                );
            let block_signer_count = if block_signer_below_commit_quorum(candidate) {
                1
            } else {
                commit_quorum
            };
            let sparse_next_height_payload_recovered =
                block_sync_selected_apply_sparse_next_height_payload_recovered(
                    BlockSyncSelectedApplySparseRecovery {
                        block_known_before: block_known_before(candidate),
                        block_known_after_creation: block_known_after(candidate),
                        next_height: next_height(candidate),
                        block_signer_count,
                        commit_quorum,
                        incoming_qc_usable: incoming_qc_usable(candidate),
                        commit_cert_present: commit_cert_present(candidate),
                        checkpoint_present: checkpoint_present(candidate),
                    },
                );
            let creation_ok_result: super::super::Result<()> = Ok(());
            let creation_err_result: super::super::Result<()> =
                Err(eyre::eyre!("block sync apply gate test error"));
            let creation_result = if creation_ok(candidate) {
                &creation_ok_result
            } else {
                &creation_err_result
            };
            let ready_for_qc = super::super::block_sync_ready_for_qc(
                block_known_after(candidate),
                creation_result,
            );
            let actual = Decision {
                allow_nonextending_qc: block_sync_selected_apply_allow_nonextending_qc(
                    selection_commit_qc(candidate),
                    incoming_qc_validated(candidate),
                    incoming_qc_usable(candidate),
                ),
                same_height_frontier_conflict,
                preserve_on_payload_mismatch,
                authoritative_supersede,
                recovery_commit_evidence,
                recovery_signed_quorum,
                recovery_payload_only,
                observed_epoch_incoming: recovery_commit_evidence
                    && observed_commit_qc_epoch == Some(observed_incoming_epoch),
                observed_epoch_checkpoint: recovery_commit_evidence
                    && observed_commit_qc_epoch == Some(expected_epoch),
                observed_epoch_none: recovery_commit_evidence && observed_commit_qc_epoch.is_none(),
                allow_aborted_revival,
                handle_created_called: true,
                pass_preserve_flag: preserve_on_payload_mismatch,
                pass_commit_evidence_mode: recovery_commit_evidence,
                pass_signed_quorum_mode: recovery_signed_quorum,
                pass_payload_only_mode: recovery_payload_only,
                signed_quorum_commit_repair_active,
                pending_commit_qc_observed: block_sync_selected_apply_pending_commit_qc_observed(
                    signed_quorum_commit_repair_active,
                    pending_block_matches_non_invalid(candidate),
                ),
                frontier_commit_qc_observed: signed_quorum_commit_repair_active,
                clear_missing_commit_qc_request: signed_quorum_commit_repair_active,
                request_commit_pipeline: signed_quorum_commit_repair_active,
                sparse_next_height_payload_recovered,
                request_known_block_commit_qc_recovery: sparse_next_height_payload_recovered,
                ready_for_qc,
                record_payload_unapplied_drop: block_sync_selected_apply_payload_unapplied_drop(
                    ready_for_qc,
                ),
                process_commit_votes: true,
                qc_to_apply: block_sync_selected_apply_qc_to_apply(
                    ready_for_qc,
                    qc_evidence_present(candidate),
                ),
            };

            assert_eq!(actual, expected, "{candidate:?} mismatch");
        }
    }

    #[test]
    fn block_sync_stale_view_formal_gate_matrix() {
        #[derive(Debug, Clone, Copy)]
        enum Candidate {
            FreshView,
            StaleUnrequestedUnknownNoEvidence,
            StaleRequested,
            StaleKnownBlock,
            StaleWithQc,
            StaleWithCheckpoint,
            StaleWithVotes,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        struct Decision {
            has_commit_evidence: bool,
            drop: bool,
            record_kind: Option<super::super::status::ConsensusMessageKind>,
            record_outcome: Option<super::super::status::ConsensusMessageOutcome>,
            record_reason: Option<super::super::status::ConsensusMessageReason>,
            clear_missing_request: bool,
            continue_after_gate: bool,
            return_ok: bool,
        }

        fn stale_view(candidate: Candidate) -> bool {
            !matches!(candidate, Candidate::FreshView)
        }

        fn requested_missing(candidate: Candidate) -> bool {
            matches!(candidate, Candidate::StaleRequested)
        }

        fn block_known_locally(candidate: Candidate) -> bool {
            matches!(candidate, Candidate::StaleKnownBlock)
        }

        fn incoming_qc(candidate: Candidate) -> bool {
            matches!(candidate, Candidate::StaleWithQc)
        }

        fn validator_checkpoint(candidate: Candidate) -> bool {
            matches!(candidate, Candidate::StaleWithCheckpoint)
        }

        fn has_commit_votes(candidate: Candidate) -> bool {
            matches!(candidate, Candidate::StaleWithVotes)
        }

        let candidates = [
            Candidate::FreshView,
            Candidate::StaleUnrequestedUnknownNoEvidence,
            Candidate::StaleRequested,
            Candidate::StaleKnownBlock,
            Candidate::StaleWithQc,
            Candidate::StaleWithCheckpoint,
            Candidate::StaleWithVotes,
        ];

        for candidate in candidates {
            let spec_has_commit_evidence = incoming_qc(candidate)
                || validator_checkpoint(candidate)
                || has_commit_votes(candidate);
            let spec_drop = stale_view(candidate)
                && !requested_missing(candidate)
                && !block_known_locally(candidate)
                && !spec_has_commit_evidence;
            let spec_record = spec_drop.then_some((
                super::super::status::ConsensusMessageKind::BlockSyncUpdate,
                super::super::status::ConsensusMessageOutcome::Dropped,
                super::super::status::ConsensusMessageReason::StaleView,
            ));
            let expected = Decision {
                has_commit_evidence: spec_has_commit_evidence,
                drop: spec_drop,
                record_kind: spec_record.map(|(kind, _, _)| kind),
                record_outcome: spec_record.map(|(_, outcome, _)| outcome),
                record_reason: spec_record.map(|(_, _, reason)| reason),
                clear_missing_request: false,
                continue_after_gate: !spec_drop,
                return_ok: true,
            };

            let has_commit_evidence = block_sync_stale_view_has_commit_evidence(
                incoming_qc(candidate),
                validator_checkpoint(candidate),
                has_commit_votes(candidate),
            );
            let drop = block_sync_stale_view_should_drop(
                stale_view(candidate),
                requested_missing(candidate),
                block_known_locally(candidate),
                has_commit_evidence,
            );
            let record = block_sync_stale_view_drop_record(drop);
            let actual = Decision {
                has_commit_evidence,
                drop,
                record_kind: record.map(|(kind, _, _)| kind),
                record_outcome: record.map(|(_, outcome, _)| outcome),
                record_reason: record.map(|(_, _, reason)| reason),
                clear_missing_request: false,
                continue_after_gate: !drop,
                return_ok: true,
            };

            assert_eq!(actual, expected, "{candidate:?} mismatch");
        }
    }

    #[test]
    fn block_sync_commit_conflict_formal_gate_matrix() {
        #[derive(Debug, Clone, Copy)]
        enum Candidate {
            HeightZeroSkips,
            CommittedAbsent,
            CommittedSameHash,
            ConflictNoQc,
            ConflictInvalidQc,
            ConflictValidQc,
            ConflictValidQcEvidenceError,
            ConflictValidQcWithStake,
            ConflictValidQcNpos,
            ConflictValidQcGenesisStub,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        struct Decision {
            validate_called: bool,
            validation_args_bound: bool,
            validation_uses_stake: bool,
            validation_mode: Option<ConsensusMode>,
            validation_mode_tag: Option<&'static str>,
            validation_allow_genesis_stub: bool,
            drop: bool,
            clear_missing: bool,
            record_kind: Option<super::super::status::ConsensusMessageKind>,
            record_outcome: Option<super::super::status::ConsensusMessageOutcome>,
            record_reason: Option<super::super::status::ConsensusMessageReason>,
            evidence_emitted: bool,
            evidence_kind_invalid_qc: bool,
            evidence_reason_matches: bool,
            evidence_certificate_matches_incoming: bool,
            falls_through: bool,
            return_ok: bool,
        }

        fn height_convertible(candidate: Candidate) -> bool {
            !matches!(candidate, Candidate::HeightZeroSkips)
        }

        fn nonzero_height(candidate: Candidate) -> bool {
            !matches!(candidate, Candidate::HeightZeroSkips)
        }

        fn committed_present(candidate: Candidate) -> bool {
            nonzero_height(candidate) && !matches!(candidate, Candidate::CommittedAbsent)
        }

        fn committed_hash_matches(candidate: Candidate) -> bool {
            matches!(candidate, Candidate::CommittedSameHash)
        }

        fn incoming_qc(candidate: Candidate) -> bool {
            matches!(
                candidate,
                Candidate::ConflictInvalidQc
                    | Candidate::ConflictValidQc
                    | Candidate::ConflictValidQcEvidenceError
                    | Candidate::ConflictValidQcWithStake
                    | Candidate::ConflictValidQcNpos
                    | Candidate::ConflictValidQcGenesisStub
            )
        }

        fn qc_valid(candidate: Candidate) -> bool {
            matches!(
                candidate,
                Candidate::ConflictValidQc
                    | Candidate::ConflictValidQcEvidenceError
                    | Candidate::ConflictValidQcWithStake
                    | Candidate::ConflictValidQcNpos
                    | Candidate::ConflictValidQcGenesisStub
            )
        }

        fn stake_snapshot_present(candidate: Candidate) -> bool {
            matches!(candidate, Candidate::ConflictValidQcWithStake)
        }

        fn consensus_mode(candidate: Candidate) -> ConsensusMode {
            if matches!(candidate, Candidate::ConflictValidQcNpos) {
                ConsensusMode::Npos
            } else {
                ConsensusMode::Permissioned
            }
        }

        fn height(candidate: Candidate) -> u64 {
            if matches!(candidate, Candidate::ConflictValidQcGenesisStub) {
                1
            } else {
                4
            }
        }

        fn view(candidate: Candidate) -> u64 {
            if matches!(candidate, Candidate::ConflictValidQcGenesisStub) {
                0
            } else {
                2
            }
        }

        let candidates = [
            Candidate::HeightZeroSkips,
            Candidate::CommittedAbsent,
            Candidate::CommittedSameHash,
            Candidate::ConflictNoQc,
            Candidate::ConflictInvalidQc,
            Candidate::ConflictValidQc,
            Candidate::ConflictValidQcEvidenceError,
            Candidate::ConflictValidQcWithStake,
            Candidate::ConflictValidQcNpos,
            Candidate::ConflictValidQcGenesisStub,
        ];

        for candidate in candidates {
            let spec_conflict = height_convertible(candidate)
                && nonzero_height(candidate)
                && committed_present(candidate)
                && !committed_hash_matches(candidate);
            let spec_validate_called = spec_conflict && incoming_qc(candidate);
            let spec_evidence_emitted =
                spec_conflict && incoming_qc(candidate) && qc_valid(candidate);
            let spec_record = spec_conflict.then_some((
                super::super::status::ConsensusMessageKind::BlockSyncUpdate,
                super::super::status::ConsensusMessageOutcome::Dropped,
                super::super::status::ConsensusMessageReason::CommitConflict,
            ));
            let expected = Decision {
                validate_called: spec_validate_called,
                validation_args_bound: spec_validate_called,
                validation_uses_stake: spec_validate_called && stake_snapshot_present(candidate),
                validation_mode: spec_validate_called.then_some(consensus_mode(candidate)),
                validation_mode_tag: spec_validate_called.then(|| {
                    if consensus_mode(candidate) == ConsensusMode::Npos {
                        NPOS_TAG
                    } else {
                        PERMISSIONED_TAG
                    }
                }),
                validation_allow_genesis_stub: spec_validate_called
                    && height(candidate) == 1
                    && view(candidate) == 0,
                drop: spec_conflict,
                clear_missing: spec_conflict,
                record_kind: spec_record.map(|(kind, _, _)| kind),
                record_outcome: spec_record.map(|(_, outcome, _)| outcome),
                record_reason: spec_record.map(|(_, _, reason)| reason),
                evidence_emitted: spec_evidence_emitted,
                evidence_kind_invalid_qc: spec_evidence_emitted,
                evidence_reason_matches: spec_evidence_emitted,
                evidence_certificate_matches_incoming: spec_evidence_emitted,
                falls_through: !spec_conflict,
                return_ok: true,
            };

            let conflict = block_sync_commit_conflict_detected(
                height_convertible(candidate),
                nonzero_height(candidate),
                committed_present(candidate),
                committed_hash_matches(candidate),
            );
            let validate_called =
                block_sync_commit_conflict_should_validate_qc(conflict, incoming_qc(candidate));
            let evidence_emitted = block_sync_commit_conflict_should_emit_evidence(
                conflict,
                incoming_qc(candidate),
                qc_valid(candidate),
            );
            let record = block_sync_commit_conflict_drop_record(conflict);
            let (
                evidence_kind_invalid_qc,
                evidence_reason_matches,
                evidence_certificate_matches_incoming,
            ) = if evidence_emitted {
                let incoming =
                    snapshot_roster_test_snapshot(vec![snapshot_roster_test_peer()], None)
                        .commit_qc;
                let evidence = block_sync_commit_conflict_invalid_qc_evidence(incoming.clone());
                let kind_invalid_qc = matches!(
                    evidence.kind,
                    crate::sumeragi::consensus::EvidenceKind::InvalidQc
                );
                match evidence.payload {
                    crate::sumeragi::consensus::EvidencePayload::InvalidQc {
                        certificate,
                        reason,
                    } => (
                        kind_invalid_qc,
                        reason == BLOCK_SYNC_COMMIT_CONFLICT_EVIDENCE_REASON,
                        certificate == incoming,
                    ),
                    _ => (kind_invalid_qc, false, false),
                }
            } else {
                (false, false, false)
            };
            let actual = Decision {
                validate_called,
                validation_args_bound: validate_called,
                validation_uses_stake: validate_called && stake_snapshot_present(candidate),
                validation_mode: validate_called.then_some(consensus_mode(candidate)),
                validation_mode_tag: validate_called
                    .then_some(block_sync_consensus_mode_tag(consensus_mode(candidate))),
                validation_allow_genesis_stub: validate_called
                    && block_sync_commit_conflict_allow_genesis_stub(
                        height(candidate),
                        view(candidate),
                    ),
                drop: conflict,
                clear_missing: block_sync_commit_conflict_should_clear_missing(conflict),
                record_kind: record.map(|(kind, _, _)| kind),
                record_outcome: record.map(|(_, outcome, _)| outcome),
                record_reason: record.map(|(_, _, reason)| reason),
                evidence_emitted,
                evidence_kind_invalid_qc,
                evidence_reason_matches,
                evidence_certificate_matches_incoming,
                falls_through: !conflict,
                return_ok: true,
            };

            assert_eq!(actual, expected, "{candidate:?} mismatch");
        }
    }

    #[test]
    fn block_sync_fetch_response_deferral_formal_gate_matrix() {
        #[derive(Debug, Clone, Copy)]
        enum Candidate {
            CanonicalBlockCreated,
            CanonicalBareUpdate,
            CanonicalUpdateWithCommitQc,
            CanonicalUpdateWithCheckpoint,
            CanonicalUpdateWithBothProofs,
            CanonicalOtherMessage,
            NextHeightBlockCreated,
            HistoricalBlockCreated,
            SameHeightHashMismatch,
            SameHeightHashUnknown,
        }

        fn block_height(candidate: Candidate) -> u64 {
            match candidate {
                Candidate::NextHeightBlockCreated => 4,
                Candidate::HistoricalBlockCreated => 2,
                _ => 3,
            }
        }

        fn local_committed_height(_: Candidate) -> u64 {
            3
        }

        fn committed_hash(candidate: Candidate) -> BlockSyncFetchResponseDeferralCommittedHash {
            match candidate {
                Candidate::SameHeightHashMismatch => {
                    BlockSyncFetchResponseDeferralCommittedHash::Mismatch
                }
                Candidate::SameHeightHashUnknown => {
                    BlockSyncFetchResponseDeferralCommittedHash::Unknown
                }
                _ => BlockSyncFetchResponseDeferralCommittedHash::Matches,
            }
        }

        fn message(candidate: Candidate) -> BlockSyncFetchResponseDeferralMessage {
            match candidate {
                Candidate::CanonicalOtherMessage => BlockSyncFetchResponseDeferralMessage::Other,
                Candidate::CanonicalBareUpdate => {
                    BlockSyncFetchResponseDeferralMessage::BlockSyncUpdate {
                        commit_qc_present: false,
                        validator_checkpoint_present: false,
                    }
                }
                Candidate::CanonicalUpdateWithCommitQc => {
                    BlockSyncFetchResponseDeferralMessage::BlockSyncUpdate {
                        commit_qc_present: true,
                        validator_checkpoint_present: false,
                    }
                }
                Candidate::CanonicalUpdateWithCheckpoint => {
                    BlockSyncFetchResponseDeferralMessage::BlockSyncUpdate {
                        commit_qc_present: false,
                        validator_checkpoint_present: true,
                    }
                }
                Candidate::CanonicalUpdateWithBothProofs => {
                    BlockSyncFetchResponseDeferralMessage::BlockSyncUpdate {
                        commit_qc_present: true,
                        validator_checkpoint_present: true,
                    }
                }
                _ => BlockSyncFetchResponseDeferralMessage::BlockCreated,
            }
        }

        let candidates = [
            Candidate::CanonicalBlockCreated,
            Candidate::CanonicalBareUpdate,
            Candidate::CanonicalUpdateWithCommitQc,
            Candidate::CanonicalUpdateWithCheckpoint,
            Candidate::CanonicalUpdateWithBothProofs,
            Candidate::CanonicalOtherMessage,
            Candidate::NextHeightBlockCreated,
            Candidate::HistoricalBlockCreated,
            Candidate::SameHeightHashMismatch,
            Candidate::SameHeightHashUnknown,
        ];

        for candidate in candidates {
            let spec_defer = block_height(candidate) == local_committed_height(candidate)
                && matches!(
                    committed_hash(candidate),
                    BlockSyncFetchResponseDeferralCommittedHash::Matches
                )
                && match message(candidate) {
                    BlockSyncFetchResponseDeferralMessage::BlockCreated => true,
                    BlockSyncFetchResponseDeferralMessage::BlockSyncUpdate {
                        commit_qc_present,
                        validator_checkpoint_present,
                    } => !commit_qc_present && !validator_checkpoint_present,
                    BlockSyncFetchResponseDeferralMessage::Other => false,
                };
            let actual = should_defer_canonical_committed_fetch_response_shape(
                block_height(candidate),
                local_committed_height(candidate),
                committed_hash(candidate),
                message(candidate),
            );

            assert_eq!(actual, spec_defer, "{candidate:?} mismatch");
        }
    }

    #[test]
    fn block_sync_fetch_block_body_handle_formal_gate_matrix() {
        #[derive(Debug, Clone, Copy)]
        enum Candidate {
            ExactCreatedDispatch,
            ExactProofDispatch,
            ExactCanonicalDefer,
            LocalHeightMismatchFrontier,
            LocalViewMismatchWindow,
            LocalIdentityMismatchOutside,
            NoLocalFrontierStash,
            NoLocalFrontierOverWindow,
            NoLocalWindowStash,
            NoLocalOutsideWindow,
        }

        fn local_block_found(candidate: Candidate) -> bool {
            matches!(
                candidate,
                Candidate::ExactCreatedDispatch
                    | Candidate::ExactProofDispatch
                    | Candidate::ExactCanonicalDefer
                    | Candidate::LocalHeightMismatchFrontier
                    | Candidate::LocalViewMismatchWindow
                    | Candidate::LocalIdentityMismatchOutside
            )
        }

        fn identity_matches(candidate: Candidate) -> bool {
            matches!(
                candidate,
                Candidate::ExactCreatedDispatch
                    | Candidate::ExactProofDispatch
                    | Candidate::ExactCanonicalDefer
            )
        }

        fn should_defer_exact_local(candidate: Candidate) -> bool {
            matches!(candidate, Candidate::ExactCanonicalDefer)
        }

        fn frontier_matches(candidate: Candidate) -> bool {
            matches!(
                candidate,
                Candidate::LocalHeightMismatchFrontier
                    | Candidate::NoLocalFrontierStash
                    | Candidate::NoLocalFrontierOverWindow
            )
        }

        fn window_allows(candidate: Candidate) -> bool {
            matches!(
                candidate,
                Candidate::LocalViewMismatchWindow
                    | Candidate::NoLocalFrontierOverWindow
                    | Candidate::NoLocalWindowStash
            )
        }

        let candidates = [
            Candidate::ExactCreatedDispatch,
            Candidate::ExactProofDispatch,
            Candidate::ExactCanonicalDefer,
            Candidate::LocalHeightMismatchFrontier,
            Candidate::LocalViewMismatchWindow,
            Candidate::LocalIdentityMismatchOutside,
            Candidate::NoLocalFrontierStash,
            Candidate::NoLocalFrontierOverWindow,
            Candidate::NoLocalWindowStash,
            Candidate::NoLocalOutsideWindow,
        ];

        for candidate in candidates {
            let spec_exact_local = local_block_found(candidate) && identity_matches(candidate);
            let spec_dispatch = spec_exact_local && !should_defer_exact_local(candidate);
            let spec_pending_stash = if spec_exact_local && should_defer_exact_local(candidate) {
                true
            } else {
                !spec_exact_local && !frontier_matches(candidate) && window_allows(candidate)
            };
            let spec_frontier_stash = !spec_exact_local && frontier_matches(candidate);
            let expected = BlockSyncFetchBlockBodyHandleDecision {
                dispatch: spec_dispatch,
                pending_stash: spec_pending_stash,
                frontier_stash: spec_frontier_stash,
                remove_requester: spec_dispatch,
                deferred_record: !spec_dispatch,
                dedup_release_count: 1,
                dispatch_uses_plain_fallback_helper: spec_dispatch,
            };

            let actual = block_sync_fetch_block_body_handle_decision(
                local_block_found(candidate),
                identity_matches(candidate),
                should_defer_exact_local(candidate),
                frontier_matches(candidate),
                window_allows(candidate),
            );

            assert_eq!(actual, expected, "{candidate:?} mismatch");
        }
    }

    #[test]
    fn block_body_repair_formal_gate_matrix() {
        #[derive(Debug, Clone, Copy)]
        struct Case {
            label: &'static str,
            runtime_da_enabled: bool,
            frontier_slot_exact: bool,
            session_exists: bool,
            session_metadata_matches: bool,
            session_has_authoritative_payload: bool,
            expected_payload_hash_present: bool,
            body_block_hash_matches_response: bool,
            body_height_matches_response: bool,
            body_view_matches_response: bool,
            body_payload_hash_matches_expected: bool,
        }

        fn happy_case(label: &'static str) -> Case {
            Case {
                label,
                runtime_da_enabled: true,
                frontier_slot_exact: true,
                session_exists: true,
                session_metadata_matches: true,
                session_has_authoritative_payload: false,
                expected_payload_hash_present: true,
                body_block_hash_matches_response: true,
                body_height_matches_response: true,
                body_view_matches_response: true,
                body_payload_hash_matches_expected: true,
            }
        }

        let block_hash =
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x71; Hash::LENGTH]));
        let other_block_hash =
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x72; Hash::LENGTH]));
        let payload_hash = Hash::prehashed([0x73; Hash::LENGTH]);
        let other_payload_hash = Hash::prehashed([0x74; Hash::LENGTH]);
        let response_height = 3;
        let response_view = 1;

        let cases = [
            happy_case("happy_block_created"),
            happy_case("happy_block_sync_update"),
            Case {
                label: "da_disabled",
                runtime_da_enabled: false,
                ..happy_case("da_disabled")
            },
            Case {
                label: "not_frontier_exact",
                frontier_slot_exact: false,
                ..happy_case("not_frontier_exact")
            },
            Case {
                label: "session_missing",
                session_exists: false,
                session_metadata_matches: false,
                ..happy_case("session_missing")
            },
            Case {
                label: "metadata_mismatch",
                session_metadata_matches: false,
                ..happy_case("metadata_mismatch")
            },
            Case {
                label: "authoritative_payload_known",
                session_has_authoritative_payload: true,
                ..happy_case("authoritative_payload_known")
            },
            Case {
                label: "missing_expected_payload_hash",
                expected_payload_hash_present: false,
                body_payload_hash_matches_expected: false,
                ..happy_case("missing_expected_payload_hash")
            },
            Case {
                label: "response_block_hash_mismatch",
                body_block_hash_matches_response: false,
                ..happy_case("response_block_hash_mismatch")
            },
            Case {
                label: "response_height_mismatch",
                body_height_matches_response: false,
                ..happy_case("response_height_mismatch")
            },
            Case {
                label: "response_view_mismatch",
                body_view_matches_response: false,
                ..happy_case("response_view_mismatch")
            },
            Case {
                label: "response_payload_hash_mismatch",
                body_payload_hash_matches_expected: false,
                ..happy_case("response_payload_hash_mismatch")
            },
        ];

        for case in cases {
            let body_identity = BlockBodyResponsePayloadIdentity {
                block_hash: if case.body_block_hash_matches_response {
                    block_hash
                } else {
                    other_block_hash
                },
                height: if case.body_height_matches_response {
                    response_height
                } else {
                    response_height + 1
                },
                view: if case.body_view_matches_response {
                    response_view
                } else {
                    response_view + 1
                },
                payload_hash: if case.body_payload_hash_matches_expected {
                    payload_hash
                } else {
                    other_payload_hash
                },
            };
            let expected_payload_hash = case.expected_payload_hash_present.then_some(payload_hash);
            let spec_identity_matches = case.body_block_hash_matches_response
                && case.body_height_matches_response
                && case.body_view_matches_response;
            let spec_payload_hash_matches = expected_payload_hash
                .as_ref()
                .is_some_and(|expected| &body_identity.payload_hash == expected);
            let spec_payload_hash_acceptable =
                expected_payload_hash.is_none() || spec_payload_hash_matches;
            let spec_allow = case.runtime_da_enabled
                && case.frontier_slot_exact
                && case.session_exists
                && case.session_metadata_matches
                && !case.session_has_authoritative_payload
                && spec_identity_matches
                && spec_payload_hash_acceptable;

            let actual = block_body_repair_gate_decision(
                case.runtime_da_enabled,
                case.frontier_slot_exact,
                case.session_exists,
                case.session_metadata_matches,
                case.session_has_authoritative_payload,
                expected_payload_hash,
                block_hash,
                response_height,
                response_view,
                body_identity,
            );

            assert_eq!(
                actual.identity_matches_response, spec_identity_matches,
                "{} identity_matches_response mismatch",
                case.label
            );
            assert_eq!(
                actual.payload_hash_matches_expected, spec_payload_hash_matches,
                "{} payload_hash_matches_expected mismatch",
                case.label
            );
            assert_eq!(actual.allow, spec_allow, "{} allow mismatch", case.label);
        }
    }

    #[test]
    fn block_body_request_stash_formal_gate_matrix() {
        #[derive(Debug, Clone, Copy)]
        struct Case {
            label: &'static str,
            committed_height: u64,
            raw_margin: u64,
            request_height: u64,
            expected_effective_margin: u64,
            expected_lower_bound: u64,
            expected_upper_bound: u64,
        }

        let cases = [
            Case {
                label: "zero_margin_next",
                committed_height: 3,
                raw_margin: 0,
                request_height: 4,
                expected_effective_margin: 1,
                expected_lower_bound: 4,
                expected_upper_bound: 4,
            },
            Case {
                label: "one_margin_next",
                committed_height: 3,
                raw_margin: 1,
                request_height: 4,
                expected_effective_margin: 1,
                expected_lower_bound: 4,
                expected_upper_bound: 4,
            },
            Case {
                label: "within_margin",
                committed_height: 3,
                raw_margin: 3,
                request_height: 5,
                expected_effective_margin: 3,
                expected_lower_bound: 4,
                expected_upper_bound: 6,
            },
            Case {
                label: "upper_boundary",
                committed_height: 3,
                raw_margin: 2,
                request_height: 5,
                expected_effective_margin: 2,
                expected_lower_bound: 4,
                expected_upper_bound: 5,
            },
            Case {
                label: "beyond_margin",
                committed_height: 3,
                raw_margin: 1,
                request_height: 5,
                expected_effective_margin: 1,
                expected_lower_bound: 4,
                expected_upper_bound: 4,
            },
            Case {
                label: "same_height",
                committed_height: 3,
                raw_margin: 3,
                request_height: 3,
                expected_effective_margin: 3,
                expected_lower_bound: 4,
                expected_upper_bound: 6,
            },
            Case {
                label: "stale_height",
                committed_height: 3,
                raw_margin: 3,
                request_height: 2,
                expected_effective_margin: 3,
                expected_lower_bound: 4,
                expected_upper_bound: 6,
            },
            Case {
                label: "zero_committed_next",
                committed_height: 0,
                raw_margin: 0,
                request_height: 1,
                expected_effective_margin: 1,
                expected_lower_bound: 1,
                expected_upper_bound: 1,
            },
            Case {
                label: "saturated_committed_boundary",
                committed_height: u64::MAX,
                raw_margin: 0,
                request_height: u64::MAX,
                expected_effective_margin: 1,
                expected_lower_bound: u64::MAX,
                expected_upper_bound: u64::MAX,
            },
            Case {
                label: "saturated_upper_boundary",
                committed_height: u64::MAX - 1,
                raw_margin: 3,
                request_height: u64::MAX,
                expected_effective_margin: 3,
                expected_lower_bound: u64::MAX,
                expected_upper_bound: u64::MAX,
            },
            Case {
                label: "saturated_lower_below",
                committed_height: u64::MAX,
                raw_margin: 3,
                request_height: u64::MAX - 1,
                expected_effective_margin: 3,
                expected_lower_bound: u64::MAX,
                expected_upper_bound: u64::MAX,
            },
        ];

        for case in cases {
            let spec_stash = case.request_height >= case.expected_lower_bound
                && case.request_height <= case.expected_upper_bound;
            let actual = block_body_request_stash_window_decision(
                case.committed_height,
                case.raw_margin,
                case.request_height,
            );

            assert_eq!(
                actual.effective_margin, case.expected_effective_margin,
                "{} effective_margin mismatch",
                case.label
            );
            assert_eq!(
                actual.lower_bound, case.expected_lower_bound,
                "{} lower_bound mismatch",
                case.label
            );
            assert_eq!(
                actual.upper_bound, case.expected_upper_bound,
                "{} upper_bound mismatch",
                case.label
            );
            assert_eq!(actual.stash, spec_stash, "{} stash mismatch", case.label);
        }
    }

    #[test]
    fn same_height_block_body_repair_formal_gate_matrix() {
        #[derive(Debug, Clone, Copy)]
        struct Source {
            exists: bool,
            phase_is_commit: bool,
            block_hash_matches: bool,
            height_matches: bool,
            view_matches: bool,
            actionable_dependency: bool,
        }

        impl Source {
            fn absent() -> Self {
                Self {
                    exists: false,
                    phase_is_commit: false,
                    block_hash_matches: false,
                    height_matches: false,
                    view_matches: false,
                    actionable_dependency: false,
                }
            }

            fn actionable() -> Self {
                Self {
                    exists: true,
                    phase_is_commit: true,
                    block_hash_matches: true,
                    height_matches: true,
                    view_matches: true,
                    actionable_dependency: true,
                }
            }
        }

        #[derive(Debug, Clone, Copy)]
        struct Case {
            label: &'static str,
            frontier_slot_exact: bool,
            pending: Source,
            deferred: Source,
            active_commit_qc_repair: bool,
        }

        fn pending_case(label: &'static str) -> Case {
            Case {
                label,
                frontier_slot_exact: true,
                pending: Source::actionable(),
                deferred: Source::absent(),
                active_commit_qc_repair: false,
            }
        }

        fn deferred_case(label: &'static str) -> Case {
            Case {
                label,
                frontier_slot_exact: true,
                pending: Source::absent(),
                deferred: Source::actionable(),
                active_commit_qc_repair: false,
            }
        }

        let cases = [
            pending_case("pending_actionable"),
            deferred_case("deferred_qc_actionable"),
            Case {
                label: "active_commit_repair",
                frontier_slot_exact: true,
                pending: Source::absent(),
                deferred: Source::absent(),
                active_commit_qc_repair: true,
            },
            Case {
                label: "multiple_sources",
                frontier_slot_exact: true,
                pending: Source::actionable(),
                deferred: Source::actionable(),
                active_commit_qc_repair: true,
            },
            Case {
                label: "not_frontier_pending_actionable",
                frontier_slot_exact: false,
                ..pending_case("not_frontier_pending_actionable")
            },
            Case {
                label: "pending_wrong_phase",
                pending: Source {
                    phase_is_commit: false,
                    ..Source::actionable()
                },
                ..pending_case("pending_wrong_phase")
            },
            Case {
                label: "pending_hash_mismatch",
                pending: Source {
                    block_hash_matches: false,
                    ..Source::actionable()
                },
                ..pending_case("pending_hash_mismatch")
            },
            Case {
                label: "pending_height_mismatch",
                pending: Source {
                    height_matches: false,
                    ..Source::actionable()
                },
                ..pending_case("pending_height_mismatch")
            },
            Case {
                label: "pending_view_mismatch",
                pending: Source {
                    view_matches: false,
                    ..Source::actionable()
                },
                ..pending_case("pending_view_mismatch")
            },
            Case {
                label: "pending_not_actionable",
                pending: Source {
                    actionable_dependency: false,
                    ..Source::actionable()
                },
                ..pending_case("pending_not_actionable")
            },
            Case {
                label: "deferred_wrong_phase",
                deferred: Source {
                    phase_is_commit: false,
                    ..Source::actionable()
                },
                ..deferred_case("deferred_wrong_phase")
            },
            Case {
                label: "deferred_hash_mismatch",
                deferred: Source {
                    block_hash_matches: false,
                    ..Source::actionable()
                },
                ..deferred_case("deferred_hash_mismatch")
            },
            Case {
                label: "deferred_height_mismatch",
                deferred: Source {
                    height_matches: false,
                    ..Source::actionable()
                },
                ..deferred_case("deferred_height_mismatch")
            },
            Case {
                label: "deferred_view_mismatch",
                deferred: Source {
                    view_matches: false,
                    ..Source::actionable()
                },
                ..deferred_case("deferred_view_mismatch")
            },
            Case {
                label: "deferred_not_actionable",
                deferred: Source {
                    actionable_dependency: false,
                    ..Source::actionable()
                },
                ..deferred_case("deferred_not_actionable")
            },
            Case {
                label: "no_sources",
                frontier_slot_exact: true,
                pending: Source::absent(),
                deferred: Source::absent(),
                active_commit_qc_repair: false,
            },
        ];

        for case in cases {
            let pending_source = same_height_block_body_repair_source_matches(
                case.pending.exists,
                case.pending.phase_is_commit,
                case.pending.block_hash_matches,
                case.pending.height_matches,
                case.pending.view_matches,
                case.pending.actionable_dependency,
            );
            let deferred_source = same_height_block_body_repair_source_matches(
                case.deferred.exists,
                case.deferred.phase_is_commit,
                case.deferred.block_hash_matches,
                case.deferred.height_matches,
                case.deferred.view_matches,
                case.deferred.actionable_dependency,
            );
            let spec_allow = case.frontier_slot_exact
                && (pending_source || deferred_source || case.active_commit_qc_repair);

            let actual = same_height_block_body_repair_decision(
                case.frontier_slot_exact,
                pending_source,
                deferred_source,
                case.active_commit_qc_repair,
            );

            assert_eq!(
                actual.pending_source, pending_source,
                "{} pending_source mismatch",
                case.label
            );
            assert_eq!(
                actual.deferred_source, deferred_source,
                "{} deferred_source mismatch",
                case.label
            );
            assert_eq!(
                actual.active_commit_qc_repair, case.active_commit_qc_repair,
                "{} active_commit_qc_repair mismatch",
                case.label
            );
            assert_eq!(actual.allow, spec_allow, "{} allow mismatch", case.label);
        }
    }

    #[test]
    fn block_body_repair_epoch_formal_gate_matrix() {
        #[derive(Debug, Clone, Copy)]
        struct DeferredSource {
            exists: bool,
            phase_is_commit: bool,
            block_hash_matches: bool,
            height_matches: bool,
            view_matches: bool,
        }

        impl DeferredSource {
            fn absent() -> Self {
                Self {
                    exists: false,
                    phase_is_commit: false,
                    block_hash_matches: false,
                    height_matches: false,
                    view_matches: false,
                }
            }

            fn matching() -> Self {
                Self {
                    exists: true,
                    phase_is_commit: true,
                    block_hash_matches: true,
                    height_matches: true,
                    view_matches: true,
                }
            }
        }

        #[derive(Debug, Clone, Copy)]
        struct PendingSource {
            exists: bool,
            commit_qc_observed: bool,
            epoch_present: bool,
        }

        impl PendingSource {
            fn absent() -> Self {
                Self {
                    exists: false,
                    commit_qc_observed: false,
                    epoch_present: false,
                }
            }

            fn matching() -> Self {
                Self {
                    exists: true,
                    commit_qc_observed: true,
                    epoch_present: true,
                }
            }
        }

        #[derive(Debug, Clone, Copy)]
        struct Case {
            label: &'static str,
            cache_present: bool,
            deferred: DeferredSource,
            pending: PendingSource,
            expected_source: BlockBodyRepairEpochSource,
        }

        fn cache_case(label: &'static str) -> Case {
            Case {
                label,
                cache_present: true,
                deferred: DeferredSource::absent(),
                pending: PendingSource::absent(),
                expected_source: BlockBodyRepairEpochSource::Cache,
            }
        }

        fn deferred_case(label: &'static str) -> Case {
            Case {
                label,
                cache_present: false,
                deferred: DeferredSource::matching(),
                pending: PendingSource::absent(),
                expected_source: BlockBodyRepairEpochSource::Deferred,
            }
        }

        fn pending_case(label: &'static str) -> Case {
            Case {
                label,
                cache_present: false,
                deferred: DeferredSource::absent(),
                pending: PendingSource::matching(),
                expected_source: BlockBodyRepairEpochSource::Pending,
            }
        }

        let cache_epoch = 11;
        let deferred_epoch_value = 22;
        let pending_epoch_value = 33;
        let cases = [
            cache_case("cache_only"),
            Case {
                label: "cache_over_deferred",
                deferred: DeferredSource::matching(),
                ..cache_case("cache_over_deferred")
            },
            Case {
                label: "cache_over_pending",
                pending: PendingSource::matching(),
                ..cache_case("cache_over_pending")
            },
            deferred_case("deferred_only"),
            Case {
                label: "deferred_over_pending",
                pending: PendingSource::matching(),
                ..deferred_case("deferred_over_pending")
            },
            pending_case("pending_only"),
            Case {
                label: "deferred_wrong_phase",
                deferred: DeferredSource {
                    phase_is_commit: false,
                    ..DeferredSource::matching()
                },
                expected_source: BlockBodyRepairEpochSource::None,
                ..deferred_case("deferred_wrong_phase")
            },
            Case {
                label: "deferred_hash_mismatch",
                deferred: DeferredSource {
                    block_hash_matches: false,
                    ..DeferredSource::matching()
                },
                expected_source: BlockBodyRepairEpochSource::None,
                ..deferred_case("deferred_hash_mismatch")
            },
            Case {
                label: "deferred_height_mismatch",
                deferred: DeferredSource {
                    height_matches: false,
                    ..DeferredSource::matching()
                },
                expected_source: BlockBodyRepairEpochSource::None,
                ..deferred_case("deferred_height_mismatch")
            },
            Case {
                label: "deferred_view_mismatch",
                deferred: DeferredSource {
                    view_matches: false,
                    ..DeferredSource::matching()
                },
                expected_source: BlockBodyRepairEpochSource::None,
                ..deferred_case("deferred_view_mismatch")
            },
            Case {
                label: "pending_not_observed",
                pending: PendingSource {
                    commit_qc_observed: false,
                    ..PendingSource::matching()
                },
                expected_source: BlockBodyRepairEpochSource::None,
                ..pending_case("pending_not_observed")
            },
            Case {
                label: "pending_epoch_missing",
                pending: PendingSource {
                    epoch_present: false,
                    ..PendingSource::matching()
                },
                expected_source: BlockBodyRepairEpochSource::None,
                ..pending_case("pending_epoch_missing")
            },
            Case {
                label: "no_sources",
                cache_present: false,
                deferred: DeferredSource::absent(),
                pending: PendingSource::absent(),
                expected_source: BlockBodyRepairEpochSource::None,
            },
        ];

        for case in cases {
            let cache_epoch_value = case.cache_present.then_some(cache_epoch);
            let deferred_epoch = block_body_repair_epoch_deferred_source(
                case.deferred.exists,
                case.deferred.phase_is_commit,
                case.deferred.block_hash_matches,
                case.deferred.height_matches,
                case.deferred.view_matches,
                deferred_epoch_value,
            );
            let pending_epoch = block_body_repair_epoch_pending_source(
                case.pending.exists,
                case.pending.commit_qc_observed,
                case.pending.epoch_present.then_some(pending_epoch_value),
            );
            let expected_epoch = match case.expected_source {
                BlockBodyRepairEpochSource::Cache => Some(cache_epoch),
                BlockBodyRepairEpochSource::Deferred => Some(deferred_epoch_value),
                BlockBodyRepairEpochSource::Pending => Some(pending_epoch_value),
                BlockBodyRepairEpochSource::None => None,
            };

            let actual =
                block_body_repair_epoch_decision(cache_epoch_value, deferred_epoch, pending_epoch);

            assert_eq!(
                actual.source, case.expected_source,
                "{} source mismatch",
                case.label
            );
            assert_eq!(
                actual.epoch, expected_epoch,
                "{} epoch mismatch",
                case.label
            );
        }
    }

    #[test]
    fn direct_commit_qc_for_block_formal_gate_matrix() {
        #[derive(Debug, Clone, Copy)]
        struct Case {
            label: &'static str,
            cache_available: bool,
            world_available: bool,
            primary_topology_available: bool,
            fallback_topology_available: bool,
            pending_commit_votes: usize,
            min_votes_for_commit: usize,
            formed_qc_available: bool,
        }

        let cases = [
            Case {
                label: "cached_only",
                cache_available: true,
                world_available: false,
                primary_topology_available: false,
                fallback_topology_available: false,
                pending_commit_votes: 0,
                min_votes_for_commit: 1,
                formed_qc_available: false,
            },
            Case {
                label: "cached_world_votes",
                cache_available: true,
                world_available: true,
                primary_topology_available: true,
                fallback_topology_available: false,
                pending_commit_votes: 2,
                min_votes_for_commit: 2,
                formed_qc_available: true,
            },
            Case {
                label: "world_only",
                cache_available: false,
                world_available: true,
                primary_topology_available: false,
                fallback_topology_available: false,
                pending_commit_votes: 0,
                min_votes_for_commit: 1,
                formed_qc_available: false,
            },
            Case {
                label: "world_votes",
                cache_available: false,
                world_available: true,
                primary_topology_available: true,
                fallback_topology_available: false,
                pending_commit_votes: 2,
                min_votes_for_commit: 2,
                formed_qc_available: true,
            },
            Case {
                label: "primary_enough_forms",
                cache_available: false,
                world_available: false,
                primary_topology_available: true,
                fallback_topology_available: false,
                pending_commit_votes: 2,
                min_votes_for_commit: 2,
                formed_qc_available: true,
            },
            Case {
                label: "primary_enough_no_form",
                cache_available: false,
                world_available: false,
                primary_topology_available: true,
                fallback_topology_available: false,
                pending_commit_votes: 2,
                min_votes_for_commit: 2,
                formed_qc_available: false,
            },
            Case {
                label: "primary_under",
                cache_available: false,
                world_available: false,
                primary_topology_available: true,
                fallback_topology_available: false,
                pending_commit_votes: 1,
                min_votes_for_commit: 2,
                formed_qc_available: false,
            },
            Case {
                label: "primary_under_fallback_available",
                cache_available: false,
                world_available: false,
                primary_topology_available: true,
                fallback_topology_available: true,
                pending_commit_votes: 1,
                min_votes_for_commit: 2,
                formed_qc_available: true,
            },
            Case {
                label: "fallback_enough_forms",
                cache_available: false,
                world_available: false,
                primary_topology_available: false,
                fallback_topology_available: true,
                pending_commit_votes: 2,
                min_votes_for_commit: 2,
                formed_qc_available: true,
            },
            Case {
                label: "fallback_enough_no_form",
                cache_available: false,
                world_available: false,
                primary_topology_available: false,
                fallback_topology_available: true,
                pending_commit_votes: 2,
                min_votes_for_commit: 2,
                formed_qc_available: false,
            },
            Case {
                label: "fallback_under",
                cache_available: false,
                world_available: false,
                primary_topology_available: false,
                fallback_topology_available: true,
                pending_commit_votes: 1,
                min_votes_for_commit: 2,
                formed_qc_available: false,
            },
            Case {
                label: "no_topology",
                cache_available: false,
                world_available: false,
                primary_topology_available: false,
                fallback_topology_available: false,
                pending_commit_votes: 2,
                min_votes_for_commit: 2,
                formed_qc_available: true,
            },
            Case {
                label: "zero_min_zero_votes",
                cache_available: false,
                world_available: false,
                primary_topology_available: false,
                fallback_topology_available: true,
                pending_commit_votes: 0,
                min_votes_for_commit: 0,
                formed_qc_available: true,
            },
            Case {
                label: "zero_min_one_vote_forms",
                cache_available: false,
                world_available: false,
                primary_topology_available: false,
                fallback_topology_available: true,
                pending_commit_votes: 1,
                min_votes_for_commit: 0,
                formed_qc_available: true,
            },
        ];

        for case in cases {
            let spec_world_consulted = !case.cache_available;
            let spec_topology_source = if case.cache_available || case.world_available {
                DirectCommitQcTopologySource::None
            } else if case.primary_topology_available {
                DirectCommitQcTopologySource::Primary
            } else if case.fallback_topology_available {
                DirectCommitQcTopologySource::Fallback
            } else {
                DirectCommitQcTopologySource::None
            };
            let spec_try_form = !case.cache_available
                && !case.world_available
                && matches!(
                    spec_topology_source,
                    DirectCommitQcTopologySource::Primary | DirectCommitQcTopologySource::Fallback
                )
                && case.pending_commit_votes >= case.min_votes_for_commit.max(1);
            let spec_result = if case.cache_available {
                DirectCommitQcForBlockResult::Cache
            } else if case.world_available {
                DirectCommitQcForBlockResult::World
            } else if spec_try_form && case.formed_qc_available {
                DirectCommitQcForBlockResult::Formed
            } else {
                DirectCommitQcForBlockResult::None
            };

            let actual = direct_commit_qc_for_block_decision(
                case.cache_available,
                case.world_available,
                case.primary_topology_available,
                case.fallback_topology_available,
                case.pending_commit_votes,
                case.min_votes_for_commit,
                case.formed_qc_available,
            );

            assert_eq!(
                actual.world_consulted, spec_world_consulted,
                "{} world_consulted mismatch",
                case.label
            );
            assert_eq!(
                actual.topology_source, spec_topology_source,
                "{} topology_source mismatch",
                case.label
            );
            assert_eq!(
                actual.try_form, spec_try_form,
                "{} try_form mismatch",
                case.label
            );
            assert_eq!(
                actual.try_phase_commit, spec_try_form,
                "{} try_phase_commit mismatch",
                case.label
            );
            assert_eq!(
                actual.try_subject_block, spec_try_form,
                "{} try_subject_block mismatch",
                case.label
            );
            assert_eq!(actual.result, spec_result, "{} result mismatch", case.label);
        }
    }

    #[test]
    fn block_body_direct_commit_qc_formal_gate_matrix() {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        enum BodyKind {
            Update,
            Created,
        }

        #[derive(Debug, Clone, Copy)]
        struct Case {
            label: &'static str,
            body_kind: BodyKind,
            identity_matches: bool,
            embedded_commit_qc: bool,
            checkpoint_commit_qc: bool,
            local_direct_qc: bool,
            expected_source: BlockBodyDirectCommitQcSource,
        }

        let cases = [
            Case {
                label: "update_embedded_qc",
                body_kind: BodyKind::Update,
                identity_matches: true,
                embedded_commit_qc: true,
                checkpoint_commit_qc: false,
                local_direct_qc: false,
                expected_source: BlockBodyDirectCommitQcSource::Embedded,
            },
            Case {
                label: "update_embedded_over_checkpoint",
                body_kind: BodyKind::Update,
                identity_matches: true,
                embedded_commit_qc: true,
                checkpoint_commit_qc: true,
                local_direct_qc: false,
                expected_source: BlockBodyDirectCommitQcSource::Embedded,
            },
            Case {
                label: "update_checkpoint_qc",
                body_kind: BodyKind::Update,
                identity_matches: true,
                embedded_commit_qc: false,
                checkpoint_commit_qc: true,
                local_direct_qc: false,
                expected_source: BlockBodyDirectCommitQcSource::Checkpoint,
            },
            Case {
                label: "update_checkpoint_over_local",
                body_kind: BodyKind::Update,
                identity_matches: true,
                embedded_commit_qc: false,
                checkpoint_commit_qc: true,
                local_direct_qc: true,
                expected_source: BlockBodyDirectCommitQcSource::Checkpoint,
            },
            Case {
                label: "update_local_qc",
                body_kind: BodyKind::Update,
                identity_matches: true,
                embedded_commit_qc: false,
                checkpoint_commit_qc: false,
                local_direct_qc: true,
                expected_source: BlockBodyDirectCommitQcSource::Local,
            },
            Case {
                label: "update_no_qc",
                body_kind: BodyKind::Update,
                identity_matches: true,
                embedded_commit_qc: false,
                checkpoint_commit_qc: false,
                local_direct_qc: false,
                expected_source: BlockBodyDirectCommitQcSource::None,
            },
            Case {
                label: "update_hash_mismatch",
                body_kind: BodyKind::Update,
                identity_matches: false,
                embedded_commit_qc: true,
                checkpoint_commit_qc: false,
                local_direct_qc: false,
                expected_source: BlockBodyDirectCommitQcSource::None,
            },
            Case {
                label: "update_height_mismatch",
                body_kind: BodyKind::Update,
                identity_matches: false,
                embedded_commit_qc: true,
                checkpoint_commit_qc: false,
                local_direct_qc: false,
                expected_source: BlockBodyDirectCommitQcSource::None,
            },
            Case {
                label: "update_view_mismatch",
                body_kind: BodyKind::Update,
                identity_matches: false,
                embedded_commit_qc: true,
                checkpoint_commit_qc: false,
                local_direct_qc: false,
                expected_source: BlockBodyDirectCommitQcSource::None,
            },
            Case {
                label: "created_local_qc",
                body_kind: BodyKind::Created,
                identity_matches: true,
                embedded_commit_qc: false,
                checkpoint_commit_qc: false,
                local_direct_qc: true,
                expected_source: BlockBodyDirectCommitQcSource::Local,
            },
            Case {
                label: "created_no_qc",
                body_kind: BodyKind::Created,
                identity_matches: true,
                embedded_commit_qc: false,
                checkpoint_commit_qc: false,
                local_direct_qc: false,
                expected_source: BlockBodyDirectCommitQcSource::None,
            },
            Case {
                label: "created_hash_mismatch",
                body_kind: BodyKind::Created,
                identity_matches: false,
                embedded_commit_qc: false,
                checkpoint_commit_qc: false,
                local_direct_qc: true,
                expected_source: BlockBodyDirectCommitQcSource::None,
            },
            Case {
                label: "created_height_mismatch",
                body_kind: BodyKind::Created,
                identity_matches: false,
                embedded_commit_qc: false,
                checkpoint_commit_qc: false,
                local_direct_qc: true,
                expected_source: BlockBodyDirectCommitQcSource::None,
            },
            Case {
                label: "created_view_mismatch",
                body_kind: BodyKind::Created,
                identity_matches: false,
                embedded_commit_qc: false,
                checkpoint_commit_qc: false,
                local_direct_qc: true,
                expected_source: BlockBodyDirectCommitQcSource::None,
            },
        ];

        for case in cases {
            let actual = match case.body_kind {
                BodyKind::Update => block_body_direct_commit_qc_update_source(
                    case.identity_matches,
                    case.embedded_commit_qc,
                    case.checkpoint_commit_qc,
                    case.local_direct_qc,
                ),
                BodyKind::Created => block_body_direct_commit_qc_created_source(
                    case.identity_matches,
                    case.local_direct_qc,
                ),
            };

            assert_eq!(
                actual, case.expected_source,
                "{} source mismatch",
                case.label
            );
        }
    }

    #[test]
    fn detached_block_body_commit_qc_formal_gate_matrix() {
        #[derive(Debug, Clone, Copy)]
        struct Case {
            label: &'static str,
            has_qc: bool,
            cached_before: bool,
            cached_after_handle: bool,
            expected: DetachedBlockBodyCommitQcDecision,
        }

        let cases = [
            Case {
                label: "no_qc",
                has_qc: false,
                cached_before: false,
                cached_after_handle: false,
                expected: DetachedBlockBodyCommitQcDecision {
                    handle_qc: false,
                    clear_missing_commit_qc: false,
                },
            },
            Case {
                label: "no_qc_cached",
                has_qc: false,
                cached_before: true,
                cached_after_handle: false,
                expected: DetachedBlockBodyCommitQcDecision {
                    handle_qc: false,
                    clear_missing_commit_qc: false,
                },
            },
            Case {
                label: "cached_before",
                has_qc: true,
                cached_before: true,
                cached_after_handle: false,
                expected: DetachedBlockBodyCommitQcDecision {
                    handle_qc: false,
                    clear_missing_commit_qc: true,
                },
            },
            Case {
                label: "handle_success_caches",
                has_qc: true,
                cached_before: false,
                cached_after_handle: true,
                expected: DetachedBlockBodyCommitQcDecision {
                    handle_qc: true,
                    clear_missing_commit_qc: true,
                },
            },
            Case {
                label: "handle_success_no_cache",
                has_qc: true,
                cached_before: false,
                cached_after_handle: false,
                expected: DetachedBlockBodyCommitQcDecision {
                    handle_qc: true,
                    clear_missing_commit_qc: false,
                },
            },
            Case {
                label: "handle_error_caches",
                has_qc: true,
                cached_before: false,
                cached_after_handle: true,
                expected: DetachedBlockBodyCommitQcDecision {
                    handle_qc: true,
                    clear_missing_commit_qc: true,
                },
            },
            Case {
                label: "handle_error_no_cache",
                has_qc: true,
                cached_before: false,
                cached_after_handle: false,
                expected: DetachedBlockBodyCommitQcDecision {
                    handle_qc: true,
                    clear_missing_commit_qc: false,
                },
            },
        ];

        for case in cases {
            let actual = detached_block_body_commit_qc_decision(
                case.has_qc,
                case.cached_before,
                case.cached_after_handle,
            );

            assert_eq!(actual, case.expected, "{} mismatch", case.label);
        }
    }

    #[test]
    fn block_body_response_dispatch_formal_gate_matrix() {
        #[derive(Debug, Clone, Copy)]
        struct Case {
            label: &'static str,
            is_sync: bool,
            under_cap: bool,
            direct_qc: bool,
            expected: BlockBodyResponseDispatchDecision,
        }

        let cases = [
            Case {
                label: "created_under_no_qc",
                is_sync: false,
                under_cap: true,
                direct_qc: false,
                expected: BlockBodyResponseDispatchDecision {
                    created_companion: true,
                    plain_fallback: false,
                    response: true,
                    qc_companion: false,
                    pos_created: 1,
                    pos_plain: 0,
                    pos_response: 2,
                    pos_qc: 0,
                    all_bypass: true,
                },
            },
            Case {
                label: "created_under_qc",
                is_sync: false,
                under_cap: true,
                direct_qc: true,
                expected: BlockBodyResponseDispatchDecision {
                    created_companion: true,
                    plain_fallback: false,
                    response: true,
                    qc_companion: true,
                    pos_created: 1,
                    pos_plain: 0,
                    pos_response: 2,
                    pos_qc: 3,
                    all_bypass: true,
                },
            },
            Case {
                label: "created_over_no_qc",
                is_sync: false,
                under_cap: false,
                direct_qc: false,
                expected: BlockBodyResponseDispatchDecision {
                    created_companion: false,
                    plain_fallback: false,
                    response: true,
                    qc_companion: false,
                    pos_created: 0,
                    pos_plain: 0,
                    pos_response: 1,
                    pos_qc: 0,
                    all_bypass: true,
                },
            },
            Case {
                label: "created_over_qc",
                is_sync: false,
                under_cap: false,
                direct_qc: true,
                expected: BlockBodyResponseDispatchDecision {
                    created_companion: false,
                    plain_fallback: false,
                    response: true,
                    qc_companion: true,
                    pos_created: 0,
                    pos_plain: 0,
                    pos_response: 1,
                    pos_qc: 2,
                    all_bypass: true,
                },
            },
            Case {
                label: "sync_under_no_qc",
                is_sync: true,
                under_cap: true,
                direct_qc: false,
                expected: BlockBodyResponseDispatchDecision {
                    created_companion: true,
                    plain_fallback: true,
                    response: true,
                    qc_companion: false,
                    pos_created: 1,
                    pos_plain: 2,
                    pos_response: 3,
                    pos_qc: 0,
                    all_bypass: true,
                },
            },
            Case {
                label: "sync_under_qc",
                is_sync: true,
                under_cap: true,
                direct_qc: true,
                expected: BlockBodyResponseDispatchDecision {
                    created_companion: true,
                    plain_fallback: true,
                    response: true,
                    qc_companion: true,
                    pos_created: 1,
                    pos_plain: 2,
                    pos_response: 3,
                    pos_qc: 4,
                    all_bypass: true,
                },
            },
            Case {
                label: "sync_over_no_qc",
                is_sync: true,
                under_cap: false,
                direct_qc: false,
                expected: BlockBodyResponseDispatchDecision {
                    created_companion: false,
                    plain_fallback: true,
                    response: true,
                    qc_companion: false,
                    pos_created: 0,
                    pos_plain: 1,
                    pos_response: 2,
                    pos_qc: 0,
                    all_bypass: true,
                },
            },
            Case {
                label: "sync_over_qc",
                is_sync: true,
                under_cap: false,
                direct_qc: true,
                expected: BlockBodyResponseDispatchDecision {
                    created_companion: false,
                    plain_fallback: true,
                    response: true,
                    qc_companion: true,
                    pos_created: 0,
                    pos_plain: 1,
                    pos_response: 2,
                    pos_qc: 3,
                    all_bypass: true,
                },
            },
        ];

        for case in cases {
            let actual =
                block_body_response_dispatch_decision(case.is_sync, case.under_cap, case.direct_qc);

            assert_eq!(actual, case.expected, "{} mismatch", case.label);
        }
    }

    #[test]
    fn fetch_pending_response_send_formal_gate_matrix() {
        #[derive(Debug, Clone, Copy)]
        struct Expected {
            hintless_allowed: bool,
            downgrade_hintless: bool,
            after_hintless: FetchPendingResponsePayloadKind,
            apply_cached_qc: bool,
            trim_update: bool,
            bypass_queue: bool,
            final_payload: FetchPendingResponseFinalPayload,
            payload_sent: bool,
            direct_qc_companion: bool,
            companion_before_payload: bool,
            bypass_used_for_payload: bool,
        }

        #[derive(Debug, Clone, Copy)]
        struct Case {
            label: &'static str,
            initial_kind: FetchPendingResponsePayloadKind,
            hintless_block_sync: bool,
            force_bypass_queue: bool,
            priority: FetchPendingBlockPriority,
            targets_highest_qc: bool,
            allow_highest_qc_bypass: bool,
            allow_hintless_block_sync_bypass: bool,
            requester_roster_proof_known: bool,
            trim_fits: bool,
            fallback_fits: bool,
            direct_qc_available: bool,
            expected: Expected,
        }

        let background_update = Expected {
            hintless_allowed: false,
            downgrade_hintless: false,
            after_hintless: FetchPendingResponsePayloadKind::BlockSyncUpdate,
            apply_cached_qc: true,
            trim_update: true,
            bypass_queue: false,
            final_payload: FetchPendingResponseFinalPayload::Original(
                FetchPendingResponsePayloadKind::BlockSyncUpdate,
            ),
            payload_sent: true,
            direct_qc_companion: false,
            companion_before_payload: false,
            bypass_used_for_payload: false,
        };
        let update_bypass = Expected {
            bypass_queue: true,
            bypass_used_for_payload: true,
            ..background_update
        };
        let hintless_allowed = Expected {
            hintless_allowed: true,
            bypass_queue: true,
            bypass_used_for_payload: true,
            ..background_update
        };
        let downgraded_hintless = Expected {
            hintless_allowed: false,
            downgrade_hintless: true,
            after_hintless: FetchPendingResponsePayloadKind::BlockCreated,
            apply_cached_qc: false,
            trim_update: false,
            bypass_queue: true,
            final_payload: FetchPendingResponseFinalPayload::Original(
                FetchPendingResponsePayloadKind::BlockCreated,
            ),
            payload_sent: true,
            direct_qc_companion: false,
            companion_before_payload: false,
            bypass_used_for_payload: true,
        };
        let update_with_direct_qc = Expected {
            direct_qc_companion: true,
            companion_before_payload: true,
            ..background_update
        };
        let fallback_with_direct_qc = Expected {
            final_payload: FetchPendingResponseFinalPayload::FallbackBlockCreated,
            payload_sent: true,
            direct_qc_companion: true,
            companion_before_payload: true,
            ..background_update
        };
        let oversized_with_direct_qc = Expected {
            final_payload: FetchPendingResponseFinalPayload::None,
            payload_sent: false,
            direct_qc_companion: true,
            companion_before_payload: false,
            bypass_used_for_payload: false,
            ..background_update
        };
        let created_with_direct_qc = Expected {
            hintless_allowed: false,
            downgrade_hintless: false,
            after_hintless: FetchPendingResponsePayloadKind::BlockCreated,
            apply_cached_qc: false,
            trim_update: false,
            bypass_queue: true,
            final_payload: FetchPendingResponseFinalPayload::Original(
                FetchPendingResponsePayloadKind::BlockCreated,
            ),
            payload_sent: true,
            direct_qc_companion: true,
            companion_before_payload: true,
            bypass_used_for_payload: true,
        };
        let created_without_direct_qc = Expected {
            direct_qc_companion: false,
            companion_before_payload: false,
            ..created_with_direct_qc
        };
        let rbc_ready_payload = Expected {
            hintless_allowed: false,
            downgrade_hintless: false,
            after_hintless: FetchPendingResponsePayloadKind::EagerRbcPayload,
            apply_cached_qc: false,
            trim_update: false,
            bypass_queue: true,
            final_payload: FetchPendingResponseFinalPayload::Original(
                FetchPendingResponsePayloadKind::EagerRbcPayload,
            ),
            payload_sent: true,
            direct_qc_companion: false,
            companion_before_payload: false,
            bypass_used_for_payload: true,
        };

        let cases = [
            Case {
                label: "background_update",
                initial_kind: FetchPendingResponsePayloadKind::BlockSyncUpdate,
                hintless_block_sync: false,
                force_bypass_queue: false,
                priority: FetchPendingBlockPriority::Background,
                targets_highest_qc: false,
                allow_highest_qc_bypass: false,
                allow_hintless_block_sync_bypass: false,
                requester_roster_proof_known: false,
                trim_fits: true,
                fallback_fits: false,
                direct_qc_available: false,
                expected: background_update,
            },
            Case {
                label: "force_update",
                force_bypass_queue: true,
                expected: update_bypass,
                ..Case {
                    label: "background_update",
                    initial_kind: FetchPendingResponsePayloadKind::BlockSyncUpdate,
                    hintless_block_sync: false,
                    force_bypass_queue: false,
                    priority: FetchPendingBlockPriority::Background,
                    targets_highest_qc: false,
                    allow_highest_qc_bypass: false,
                    allow_hintless_block_sync_bypass: false,
                    requester_roster_proof_known: false,
                    trim_fits: true,
                    fallback_fits: false,
                    direct_qc_available: false,
                    expected: background_update,
                }
            },
            Case {
                label: "consensus_update",
                priority: FetchPendingBlockPriority::Consensus,
                expected: update_bypass,
                ..cases_background_update()
            },
            Case {
                label: "highest_update_allowed",
                targets_highest_qc: true,
                allow_highest_qc_bypass: true,
                expected: update_bypass,
                ..cases_background_update()
            },
            Case {
                label: "highest_update_disallowed",
                targets_highest_qc: true,
                expected: background_update,
                ..cases_background_update()
            },
            Case {
                label: "hintless_allowed",
                hintless_block_sync: true,
                allow_hintless_block_sync_bypass: true,
                requester_roster_proof_known: true,
                expected: hintless_allowed,
                ..cases_background_update()
            },
            Case {
                label: "hintless_no_roster",
                hintless_block_sync: true,
                allow_hintless_block_sync_bypass: true,
                expected: downgraded_hintless,
                ..cases_background_update()
            },
            Case {
                label: "hintless_no_allow",
                hintless_block_sync: true,
                expected: downgraded_hintless,
                ..cases_background_update()
            },
            Case {
                label: "update_trim_fits_qc",
                direct_qc_available: true,
                expected: update_with_direct_qc,
                ..cases_background_update()
            },
            Case {
                label: "update_trim_fails_fallback_fits_qc",
                trim_fits: false,
                fallback_fits: true,
                direct_qc_available: true,
                expected: fallback_with_direct_qc,
                ..cases_background_update()
            },
            Case {
                label: "update_trim_fails_fallback_oversized_qc",
                trim_fits: false,
                fallback_fits: false,
                direct_qc_available: true,
                expected: oversized_with_direct_qc,
                ..cases_background_update()
            },
            Case {
                label: "created_with_qc",
                initial_kind: FetchPendingResponsePayloadKind::BlockCreated,
                direct_qc_available: true,
                expected: created_with_direct_qc,
                ..cases_background_update()
            },
            Case {
                label: "created_no_qc",
                initial_kind: FetchPendingResponsePayloadKind::BlockCreated,
                expected: created_without_direct_qc,
                ..cases_background_update()
            },
            Case {
                label: "rbc_ready_payload",
                initial_kind: FetchPendingResponsePayloadKind::EagerRbcPayload,
                expected: rbc_ready_payload,
                ..cases_background_update()
            },
        ];

        for case in cases {
            let preflight = fetch_pending_response_preflight_decision(
                case.initial_kind,
                case.hintless_block_sync,
                case.force_bypass_queue,
                case.priority,
                case.targets_highest_qc,
                case.allow_highest_qc_bypass,
                case.allow_hintless_block_sync_bypass,
                case.requester_roster_proof_known,
            );
            assert_eq!(
                preflight.hintless_allowed, case.expected.hintless_allowed,
                "{} hintless_allowed mismatch",
                case.label
            );
            assert_eq!(
                preflight.downgrade_hintless, case.expected.downgrade_hintless,
                "{} downgrade_hintless mismatch",
                case.label
            );
            assert_eq!(
                preflight.message_after_hintless_gate, case.expected.after_hintless,
                "{} after_hintless mismatch",
                case.label
            );
            assert_eq!(
                preflight.apply_cached_qc, case.expected.apply_cached_qc,
                "{} apply_cached_qc mismatch",
                case.label
            );
            assert_eq!(
                preflight.trim_update, case.expected.trim_update,
                "{} trim_update mismatch",
                case.label
            );
            assert_eq!(
                preflight.bypass_queue, case.expected.bypass_queue,
                "{} bypass_queue mismatch",
                case.label
            );

            let frame = fetch_pending_response_frame_decision(
                preflight.message_after_hintless_gate,
                case.trim_fits,
                case.fallback_fits,
                case.direct_qc_available,
            );
            assert_eq!(
                frame.final_payload, case.expected.final_payload,
                "{} final_payload mismatch",
                case.label
            );
            assert_eq!(
                frame.payload_sent, case.expected.payload_sent,
                "{} payload_sent mismatch",
                case.label
            );
            assert_eq!(
                frame.direct_qc_companion, case.expected.direct_qc_companion,
                "{} direct_qc_companion mismatch",
                case.label
            );
            assert_eq!(
                frame.companion_before_payload, case.expected.companion_before_payload,
                "{} companion_before_payload mismatch",
                case.label
            );
            assert_eq!(
                frame.payload_sent && preflight.bypass_queue,
                case.expected.bypass_used_for_payload,
                "{} bypass_used_for_payload mismatch",
                case.label
            );
        }

        fn cases_background_update() -> Case {
            Case {
                label: "background_update",
                initial_kind: FetchPendingResponsePayloadKind::BlockSyncUpdate,
                hintless_block_sync: false,
                force_bypass_queue: false,
                priority: FetchPendingBlockPriority::Background,
                targets_highest_qc: false,
                allow_highest_qc_bypass: false,
                allow_hintless_block_sync_bypass: false,
                requester_roster_proof_known: false,
                trim_fits: true,
                fallback_fits: false,
                direct_qc_available: false,
                expected: Expected {
                    hintless_allowed: false,
                    downgrade_hintless: false,
                    after_hintless: FetchPendingResponsePayloadKind::BlockSyncUpdate,
                    apply_cached_qc: true,
                    trim_update: true,
                    bypass_queue: false,
                    final_payload: FetchPendingResponseFinalPayload::Original(
                        FetchPendingResponsePayloadKind::BlockSyncUpdate,
                    ),
                    payload_sent: true,
                    direct_qc_companion: false,
                    companion_before_payload: false,
                    bypass_used_for_payload: false,
                },
            }
        }
    }

    #[test]
    fn fetch_pending_responses_batch_formal_gate_matrix() {
        #[derive(Debug, Clone, Copy)]
        struct ExpectedPayload {
            exact_body_companion: bool,
            hintless_allowed: bool,
            payload_message: FetchPendingResponsesBatchPayloadMessage,
            created_companion: bool,
            payload_pos: u8,
            created_companion_pos: u8,
            created_companion_before_payload: bool,
            payload_force_bypass_arg: bool,
            payload_allow_hintless_arg: bool,
            payload_roster_proof_arg: bool,
            payload_consensus_priority_arg: bool,
        }

        fn assert_commit_case(
            label: &str,
            commit_qc_only: bool,
            dispatch_succeeds: bool,
            expected_dispatch: bool,
            expected_restash: bool,
            expected_restash_flag: bool,
        ) {
            let decision =
                fetch_pending_responses_batch_commit_decision(commit_qc_only, dispatch_succeeds);
            assert_eq!(
                decision.dispatch_commit_qc_only, expected_dispatch,
                "{label} dispatch_commit_qc_only mismatch"
            );
            assert_eq!(
                decision.restash, expected_restash,
                "{label} restash mismatch"
            );
            assert_eq!(
                decision.restash_commit_qc_only, expected_restash_flag,
                "{label} restash_commit_qc_only mismatch"
            );
        }

        fn assert_payload_case(
            label: &str,
            payload_kind: FetchPendingResponsesBatchPayloadKind,
            force_bypass_queue: bool,
            allow_hintless_block_sync_bypass: bool,
            requester_roster_proof_known: bool,
            priority: FetchPendingBlockPriority,
            created_companion_fits: bool,
            expected: ExpectedPayload,
        ) {
            let decision = fetch_pending_responses_batch_payload_decision(
                true,
                payload_kind,
                force_bypass_queue,
                allow_hintless_block_sync_bypass,
                requester_roster_proof_known,
                priority,
                created_companion_fits,
            );
            assert!(decision.payload_peer, "{label} should be payload peer");
            assert!(decision.payload_sent, "{label} should send payload");
            assert_eq!(
                decision.exact_body_companion, expected.exact_body_companion,
                "{label} exact_body_companion mismatch"
            );
            assert_eq!(
                decision.hintless_allowed, expected.hintless_allowed,
                "{label} hintless_allowed mismatch"
            );
            assert_eq!(
                decision.payload_message, expected.payload_message,
                "{label} payload_message mismatch"
            );
            assert_eq!(
                decision.created_companion, expected.created_companion,
                "{label} created_companion mismatch"
            );
            assert_eq!(
                decision.payload_pos, expected.payload_pos,
                "{label} payload_pos mismatch"
            );
            assert_eq!(
                decision.created_companion_pos, expected.created_companion_pos,
                "{label} created_companion_pos mismatch"
            );
            assert_eq!(
                decision.created_companion_before_payload,
                expected.created_companion_before_payload,
                "{label} created_companion_before_payload mismatch"
            );
            assert_eq!(
                decision.payload_force_bypass_arg, expected.payload_force_bypass_arg,
                "{label} force_bypass arg mismatch"
            );
            assert_eq!(
                decision.payload_allow_hintless_arg, expected.payload_allow_hintless_arg,
                "{label} allow_hintless arg mismatch"
            );
            assert_eq!(
                decision.payload_roster_proof_arg, expected.payload_roster_proof_arg,
                "{label} requester_roster arg mismatch"
            );
            assert_eq!(
                decision.payload_consensus_priority_arg, expected.payload_consensus_priority_arg,
                "{label} consensus priority arg mismatch"
            );
        }

        fn expected_other(
            exact_body_companion: bool,
            force_bypass: bool,
            consensus_priority: bool,
        ) -> ExpectedPayload {
            ExpectedPayload {
                exact_body_companion,
                hintless_allowed: false,
                payload_message: FetchPendingResponsesBatchPayloadMessage::Other,
                created_companion: false,
                payload_pos: 1 + u8::from(exact_body_companion),
                created_companion_pos: 0,
                created_companion_before_payload: true,
                payload_force_bypass_arg: force_bypass,
                payload_allow_hintless_arg: false,
                payload_roster_proof_arg: true,
                payload_consensus_priority_arg: consensus_priority,
            }
        }

        assert!(!fetch_pending_responses_batch_should_build_payload(0));
        assert!(fetch_pending_responses_batch_should_build_payload(1));

        assert_commit_case(
            "only_commit_qc_direct_success",
            true,
            true,
            true,
            false,
            false,
        );
        assert_commit_case("only_commit_qc_deferred", true, false, true, true, true);
        assert_commit_case("payload_peer", false, false, false, false, false);

        let non_payload = fetch_pending_responses_batch_payload_decision(
            false,
            FetchPendingResponsesBatchPayloadKind::Other,
            false,
            false,
            true,
            FetchPendingBlockPriority::Background,
            false,
        );
        assert!(!non_payload.payload_peer);
        assert!(!non_payload.payload_sent);
        assert_eq!(
            non_payload.payload_message,
            FetchPendingResponsesBatchPayloadMessage::None
        );

        assert_payload_case(
            "mixed_commit_qc_and_payload",
            FetchPendingResponsesBatchPayloadKind::Other,
            false,
            false,
            true,
            FetchPendingBlockPriority::Background,
            false,
            expected_other(false, false, false),
        );
        assert_payload_case(
            "consensus_payload_companion",
            FetchPendingResponsesBatchPayloadKind::Other,
            false,
            false,
            true,
            FetchPendingBlockPriority::Consensus,
            false,
            expected_other(true, false, true),
        );
        assert_payload_case(
            "background_payload_no_companion",
            FetchPendingResponsesBatchPayloadKind::Other,
            false,
            false,
            true,
            FetchPendingBlockPriority::Background,
            false,
            expected_other(false, false, false),
        );
        assert_payload_case(
            "hintless_allowed_peer",
            FetchPendingResponsesBatchPayloadKind::HintlessBlockSyncUpdate,
            false,
            true,
            true,
            FetchPendingBlockPriority::Background,
            false,
            ExpectedPayload {
                exact_body_companion: false,
                hintless_allowed: true,
                payload_message: FetchPendingResponsesBatchPayloadMessage::BlockSyncUpdate,
                created_companion: false,
                payload_pos: 1,
                created_companion_pos: 0,
                created_companion_before_payload: true,
                payload_force_bypass_arg: false,
                payload_allow_hintless_arg: true,
                payload_roster_proof_arg: true,
                payload_consensus_priority_arg: false,
            },
        );
        assert_payload_case(
            "hintless_downgraded_no_roster",
            FetchPendingResponsesBatchPayloadKind::HintlessBlockSyncUpdate,
            false,
            true,
            false,
            FetchPendingBlockPriority::Background,
            false,
            ExpectedPayload {
                exact_body_companion: false,
                hintless_allowed: false,
                payload_message: FetchPendingResponsesBatchPayloadMessage::BlockCreated,
                created_companion: false,
                payload_pos: 1,
                created_companion_pos: 0,
                created_companion_before_payload: true,
                payload_force_bypass_arg: false,
                payload_allow_hintless_arg: false,
                payload_roster_proof_arg: false,
                payload_consensus_priority_arg: false,
            },
        );
        assert_payload_case(
            "hintless_downgraded_no_allow",
            FetchPendingResponsesBatchPayloadKind::HintlessBlockSyncUpdate,
            false,
            false,
            true,
            FetchPendingBlockPriority::Background,
            false,
            ExpectedPayload {
                exact_body_companion: false,
                hintless_allowed: false,
                payload_message: FetchPendingResponsesBatchPayloadMessage::BlockCreated,
                created_companion: false,
                payload_pos: 1,
                created_companion_pos: 0,
                created_companion_before_payload: true,
                payload_force_bypass_arg: false,
                payload_allow_hintless_arg: false,
                payload_roster_proof_arg: true,
                payload_consensus_priority_arg: false,
            },
        );
        assert_payload_case(
            "hintless_mixed_two_peers_a",
            FetchPendingResponsesBatchPayloadKind::HintlessBlockSyncUpdate,
            false,
            true,
            true,
            FetchPendingBlockPriority::Background,
            false,
            ExpectedPayload {
                exact_body_companion: false,
                hintless_allowed: true,
                payload_message: FetchPendingResponsesBatchPayloadMessage::BlockSyncUpdate,
                created_companion: false,
                payload_pos: 1,
                created_companion_pos: 0,
                created_companion_before_payload: true,
                payload_force_bypass_arg: false,
                payload_allow_hintless_arg: true,
                payload_roster_proof_arg: true,
                payload_consensus_priority_arg: false,
            },
        );
        assert_payload_case(
            "hintless_mixed_two_peers_b",
            FetchPendingResponsesBatchPayloadKind::HintlessBlockSyncUpdate,
            false,
            true,
            false,
            FetchPendingBlockPriority::Background,
            false,
            ExpectedPayload {
                exact_body_companion: false,
                hintless_allowed: false,
                payload_message: FetchPendingResponsesBatchPayloadMessage::BlockCreated,
                created_companion: false,
                payload_pos: 1,
                created_companion_pos: 0,
                created_companion_before_payload: true,
                payload_force_bypass_arg: false,
                payload_allow_hintless_arg: false,
                payload_roster_proof_arg: false,
                payload_consensus_priority_arg: false,
            },
        );
        assert_payload_case(
            "hintless_consensus_companion",
            FetchPendingResponsesBatchPayloadKind::HintlessBlockSyncUpdate,
            false,
            true,
            true,
            FetchPendingBlockPriority::Consensus,
            false,
            ExpectedPayload {
                exact_body_companion: true,
                hintless_allowed: true,
                payload_message: FetchPendingResponsesBatchPayloadMessage::BlockSyncUpdate,
                created_companion: false,
                payload_pos: 2,
                created_companion_pos: 0,
                created_companion_before_payload: true,
                payload_force_bypass_arg: false,
                payload_allow_hintless_arg: true,
                payload_roster_proof_arg: true,
                payload_consensus_priority_arg: true,
            },
        );
        assert_payload_case(
            "roster_update_companion_fits",
            FetchPendingResponsesBatchPayloadKind::RosterBlockSyncUpdate,
            false,
            true,
            true,
            FetchPendingBlockPriority::Background,
            true,
            ExpectedPayload {
                exact_body_companion: false,
                hintless_allowed: false,
                payload_message: FetchPendingResponsesBatchPayloadMessage::BlockSyncUpdate,
                created_companion: true,
                payload_pos: 2,
                created_companion_pos: 1,
                created_companion_before_payload: true,
                payload_force_bypass_arg: false,
                payload_allow_hintless_arg: true,
                payload_roster_proof_arg: true,
                payload_consensus_priority_arg: false,
            },
        );
        assert_payload_case(
            "roster_update_companion_oversized",
            FetchPendingResponsesBatchPayloadKind::RosterBlockSyncUpdate,
            false,
            true,
            true,
            FetchPendingBlockPriority::Background,
            false,
            ExpectedPayload {
                exact_body_companion: false,
                hintless_allowed: false,
                payload_message: FetchPendingResponsesBatchPayloadMessage::BlockSyncUpdate,
                created_companion: false,
                payload_pos: 1,
                created_companion_pos: 0,
                created_companion_before_payload: true,
                payload_force_bypass_arg: false,
                payload_allow_hintless_arg: true,
                payload_roster_proof_arg: true,
                payload_consensus_priority_arg: false,
            },
        );
        assert_payload_case(
            "roster_update_no_roster_proof_keeps_hintless_flag_but_marks_unproven_requester",
            FetchPendingResponsesBatchPayloadKind::RosterBlockSyncUpdate,
            false,
            true,
            false,
            FetchPendingBlockPriority::Background,
            false,
            ExpectedPayload {
                exact_body_companion: false,
                hintless_allowed: false,
                payload_message: FetchPendingResponsesBatchPayloadMessage::BlockSyncUpdate,
                created_companion: false,
                payload_pos: 1,
                created_companion_pos: 0,
                created_companion_before_payload: true,
                payload_force_bypass_arg: false,
                payload_allow_hintless_arg: true,
                payload_roster_proof_arg: false,
                payload_consensus_priority_arg: false,
            },
        );
        assert_payload_case(
            "plain_created_with_hintless_bypass",
            FetchPendingResponsesBatchPayloadKind::BlockCreated,
            false,
            true,
            true,
            FetchPendingBlockPriority::Background,
            false,
            ExpectedPayload {
                exact_body_companion: false,
                hintless_allowed: false,
                payload_message: FetchPendingResponsesBatchPayloadMessage::BlockCreated,
                created_companion: false,
                payload_pos: 1,
                created_companion_pos: 0,
                created_companion_before_payload: true,
                payload_force_bypass_arg: true,
                payload_allow_hintless_arg: true,
                payload_roster_proof_arg: true,
                payload_consensus_priority_arg: false,
            },
        );
        assert_payload_case(
            "plain_created_with_hintless_bypass_without_roster_proof_marks_unproven_requester",
            FetchPendingResponsesBatchPayloadKind::BlockCreated,
            false,
            true,
            false,
            FetchPendingBlockPriority::Background,
            false,
            ExpectedPayload {
                exact_body_companion: false,
                hintless_allowed: false,
                payload_message: FetchPendingResponsesBatchPayloadMessage::BlockCreated,
                created_companion: false,
                payload_pos: 1,
                created_companion_pos: 0,
                created_companion_before_payload: true,
                payload_force_bypass_arg: true,
                payload_allow_hintless_arg: true,
                payload_roster_proof_arg: false,
                payload_consensus_priority_arg: false,
            },
        );
        assert_payload_case(
            "plain_created_without_hintless_bypass",
            FetchPendingResponsesBatchPayloadKind::BlockCreated,
            false,
            false,
            true,
            FetchPendingBlockPriority::Background,
            false,
            ExpectedPayload {
                exact_body_companion: false,
                hintless_allowed: false,
                payload_message: FetchPendingResponsesBatchPayloadMessage::BlockCreated,
                created_companion: false,
                payload_pos: 1,
                created_companion_pos: 0,
                created_companion_before_payload: true,
                payload_force_bypass_arg: false,
                payload_allow_hintless_arg: false,
                payload_roster_proof_arg: true,
                payload_consensus_priority_arg: false,
            },
        );
        assert_payload_case(
            "plain_other_payload",
            FetchPendingResponsesBatchPayloadKind::Other,
            false,
            false,
            true,
            FetchPendingBlockPriority::Background,
            false,
            expected_other(false, false, false),
        );
        assert_payload_case(
            "force_plain_other_payload",
            FetchPendingResponsesBatchPayloadKind::Other,
            true,
            false,
            true,
            FetchPendingBlockPriority::Background,
            false,
            expected_other(false, true, false),
        );
    }

    #[test]
    fn pending_response_flush_formal_gate_matrix() {
        #[derive(Debug, Clone, Copy)]
        enum Candidate {
            FetchAbsent,
            FetchDeferred,
            FetchReadyOne,
            FetchReadyEmptyEntry,
            BodyAbsent,
            BodyDeferred,
            BodyReadyTwo,
            BodyReadyEmptyEntry,
        }

        fn kind(candidate: Candidate) -> PendingResponseFlushKind {
            match candidate {
                Candidate::FetchAbsent
                | Candidate::FetchDeferred
                | Candidate::FetchReadyOne
                | Candidate::FetchReadyEmptyEntry => PendingResponseFlushKind::Fetch,
                Candidate::BodyAbsent
                | Candidate::BodyDeferred
                | Candidate::BodyReadyTwo
                | Candidate::BodyReadyEmptyEntry => PendingResponseFlushKind::Body,
            }
        }

        fn has_pending_key(candidate: Candidate) -> bool {
            !matches!(candidate, Candidate::FetchAbsent | Candidate::BodyAbsent)
        }

        fn deferred(candidate: Candidate) -> bool {
            matches!(
                candidate,
                Candidate::FetchDeferred | Candidate::BodyDeferred
            )
        }

        fn fetch_requester(candidate: Candidate, peer: &str) -> bool {
            matches!(
                candidate,
                Candidate::FetchDeferred | Candidate::FetchReadyOne
            ) && peer == "a"
        }

        fn body_requester(candidate: Candidate, peer: &str) -> bool {
            matches!(candidate, Candidate::BodyDeferred | Candidate::BodyReadyTwo)
                && matches!(peer, "a" | "b")
        }

        let candidates = [
            Candidate::FetchAbsent,
            Candidate::FetchDeferred,
            Candidate::FetchReadyOne,
            Candidate::FetchReadyEmptyEntry,
            Candidate::BodyAbsent,
            Candidate::BodyDeferred,
            Candidate::BodyReadyTwo,
            Candidate::BodyReadyEmptyEntry,
        ];

        for candidate in candidates {
            let decision = pending_response_flush_decision(
                kind(candidate),
                has_pending_key(candidate),
                deferred(candidate),
            );
            let ready = has_pending_key(candidate) && !deferred(candidate);
            let fetch = matches!(kind(candidate), PendingResponseFlushKind::Fetch);
            let body = matches!(kind(candidate), PendingResponseFlushKind::Body);

            assert_eq!(
                decision.returns_ready, ready,
                "{candidate:?} return mismatch"
            );
            assert_eq!(
                decision.build_payload,
                has_pending_key(candidate),
                "{candidate:?} build-payload mismatch"
            );
            assert_eq!(
                decision.fetch_removed,
                fetch && ready,
                "{candidate:?} fetch removal mismatch"
            );
            assert_eq!(
                decision.body_removed,
                body && ready,
                "{candidate:?} body removal mismatch"
            );
            assert_eq!(
                decision.fetch_batch_called,
                fetch && ready,
                "{candidate:?} fetch batch call mismatch"
            );
            assert!(
                !decision.fetch_batch_force_arg,
                "{candidate:?} force bypass must remain disabled"
            );
            assert!(
                !decision.fetch_batch_allow_highest_arg,
                "{candidate:?} highest-QC bypass must remain disabled"
            );
            assert!(
                !decision.fetch_batch_allow_hintless_arg,
                "{candidate:?} hintless bypass must remain disabled"
            );
            assert_eq!(
                decision.body_response_constructed,
                body && ready,
                "{candidate:?} body response construction mismatch"
            );
            assert_eq!(
                decision.body_response_hash_bound, decision.body_response_constructed,
                "{candidate:?} body hash binding mismatch"
            );
            assert_eq!(
                decision.body_response_height_bound, decision.body_response_constructed,
                "{candidate:?} body height binding mismatch"
            );
            assert_eq!(
                decision.body_response_view_bound, decision.body_response_constructed,
                "{candidate:?} body view binding mismatch"
            );
            assert_eq!(
                decision.body_response_payload_bound, decision.body_response_constructed,
                "{candidate:?} body payload binding mismatch"
            );
            assert!(
                decision.body_dispatches_use_plain_fallback,
                "{candidate:?} body dispatches must use the fallback helper"
            );

            for peer in ["a", "b", "c"] {
                let requester = if fetch {
                    fetch_requester(candidate, peer)
                } else {
                    body_requester(candidate, peer)
                };
                assert_eq!(
                    pending_response_flush_targets_requester(decision, requester),
                    ready && requester,
                    "{candidate:?} peer {peer} targeting mismatch"
                );
            }
        }
    }

    #[test]
    fn deferred_block_sync_helper_formal_gate_matrix() {
        fn reason_label(reason: Option<&'static str>) -> &'static str {
            reason.unwrap_or("none")
        }

        fn hash(byte: u8) -> HashOf<BlockHeader> {
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([byte; Hash::LENGTH]))
        }

        assert!(!deferred_block_sync_validation_inflight_blocks(
            true, true, false
        ));
        assert!(deferred_block_sync_validation_inflight_blocks(
            false, false, false
        ));
        assert!(deferred_block_sync_validation_pending_conflicts(None, 10));
        assert!(deferred_block_sync_validation_pending_conflicts(
            Some(9),
            10
        ));
        assert!(deferred_block_sync_validation_pending_conflicts(
            Some(10),
            10
        ));
        assert!(!deferred_block_sync_validation_pending_conflicts(
            Some(11),
            10
        ));
        assert!(deferred_block_sync_validation_inflight_blocks(
            false, true, true
        ));
        assert!(!deferred_block_sync_validation_inflight_blocks(
            false, true, false
        ));

        assert_eq!(
            reason_label(deferred_block_sync_update_deferral_reason(
                true, true, true, false
            )),
            "commit_inflight"
        );
        assert_eq!(
            reason_label(deferred_block_sync_update_deferral_reason(
                false, true, true, false
            )),
            "validation_inflight"
        );
        assert_eq!(
            reason_label(deferred_block_sync_update_deferral_reason(
                false, false, true, false
            )),
            "pending_processing"
        );
        assert_eq!(
            reason_label(deferred_block_sync_update_deferral_reason(
                true, true, true, true
            )),
            "none"
        );
        assert_eq!(
            reason_label(deferred_block_sync_update_deferral_reason(
                false, false, false, false
            )),
            "none"
        );

        let fill_missing =
            deferred_block_sync_merge_decision(false, true, false, true, false, true, false, false);
        assert!(fill_missing.take_incoming_commit_qc);
        assert!(fill_missing.take_incoming_validator_checkpoint);
        assert!(fill_missing.take_incoming_stake_snapshot);
        assert!(fill_missing.final_commit_qc_present);
        assert!(fill_missing.final_validator_checkpoint_present);
        assert!(fill_missing.final_stake_snapshot_present);

        let preserve_existing =
            deferred_block_sync_merge_decision(true, true, true, true, true, true, false, false);
        assert!(!preserve_existing.take_incoming_commit_qc);
        assert!(!preserve_existing.take_incoming_validator_checkpoint);
        assert!(!preserve_existing.take_incoming_stake_snapshot);
        assert!(preserve_existing.final_commit_qc_present);
        assert!(preserve_existing.final_validator_checkpoint_present);
        assert!(preserve_existing.final_stake_snapshot_present);

        let sender_none_preserves = deferred_block_sync_merge_decision(
            false, false, false, false, false, false, true, false,
        );
        assert!(!sender_none_preserves.replace_sender);
        assert!(sender_none_preserves.final_sender_present);
        let sender_some_replaces = deferred_block_sync_merge_decision(
            false, false, false, false, false, false, true, true,
        );
        assert!(sender_some_replaces.replace_sender);
        assert!(sender_some_replaces.final_sender_present);

        assert!(deferred_block_sync_commit_evidence_present(
            true, false, false
        ));
        assert!(deferred_block_sync_commit_evidence_present(
            false, true, false
        ));
        assert!(deferred_block_sync_commit_evidence_present(
            false, false, true
        ));
        assert!(!deferred_block_sync_commit_evidence_present(
            false, false, false
        ));

        assert!(!deferred_block_sync_cap_should_evict(0, 3));
        assert_eq!(deferred_block_sync_cap_eviction_count(0, 3), 0);
        assert!(!deferred_block_sync_cap_should_evict(3, 2));
        assert_eq!(deferred_block_sync_cap_eviction_count(3, 2), 0);
        assert!(deferred_block_sync_cap_should_evict(2, 3));
        assert_eq!(deferred_block_sync_cap_eviction_count(2, 3), 1);
        assert_eq!(deferred_block_sync_cap_eviction_count(1, 3), 2);

        #[derive(Debug, Clone, Copy)]
        struct Entry {
            label: &'static str,
            evidence: bool,
            height: u64,
            view: u64,
            hash: HashOf<BlockHeader>,
        }

        fn first_evicted(entries: &[Entry]) -> &'static str {
            entries
                .iter()
                .min_by_key(|entry| {
                    deferred_block_sync_eviction_rank(
                        entry.evidence,
                        entry.height,
                        entry.view,
                        entry.hash,
                    )
                })
                .expect("non-empty entries")
                .label
        }

        assert_eq!(
            first_evicted(&[
                Entry {
                    label: "no_evidence",
                    evidence: false,
                    height: 10,
                    view: 10,
                    hash: hash(0x20),
                },
                Entry {
                    label: "evidence",
                    evidence: true,
                    height: 1,
                    view: 1,
                    hash: hash(0x10),
                },
            ]),
            "no_evidence"
        );
        assert_eq!(
            first_evicted(&[
                Entry {
                    label: "old_view",
                    evidence: false,
                    height: 10,
                    view: 1,
                    hash: hash(0x20),
                },
                Entry {
                    label: "new_view",
                    evidence: false,
                    height: 1,
                    view: 2,
                    hash: hash(0x10),
                },
            ]),
            "old_view"
        );
        assert_eq!(
            first_evicted(&[
                Entry {
                    label: "old_height",
                    evidence: false,
                    height: 1,
                    view: 2,
                    hash: hash(0x20),
                },
                Entry {
                    label: "new_height",
                    evidence: false,
                    height: 2,
                    view: 2,
                    hash: hash(0x10),
                },
            ]),
            "old_height"
        );
        assert_eq!(
            first_evicted(&[
                Entry {
                    label: "low_hash",
                    evidence: false,
                    height: 2,
                    view: 2,
                    hash: hash(0x10),
                },
                Entry {
                    label: "high_hash",
                    evidence: false,
                    height: 2,
                    view: 2,
                    hash: hash(0x20),
                },
            ]),
            "low_hash"
        );
        assert!(deferred_block_sync_cap_eviction_count(2, 3) > 0);
        assert_eq!(deferred_block_sync_cap_eviction_count(2, 2), 0);
    }

    #[test]
    fn deferred_block_sync_cache_formal_gate_matrix() {
        fn hash(byte: u8) -> HashOf<BlockHeader> {
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([byte; Hash::LENGTH]))
        }

        fn assert_cache_case(
            label: &str,
            initial_len: usize,
            existing_same_full_key: bool,
            cap: usize,
            expected_len_before_cap: usize,
            expected_eviction_count: usize,
            expected_final_len: usize,
        ) {
            let decision =
                deferred_block_sync_cache_decision(initial_len, existing_same_full_key, cap);
            assert!(decision.cache_called, "{label} cache_called mismatch");
            assert!(
                decision.commit_votes_cleared,
                "{label} commit votes must be cleared"
            );
            assert_eq!(
                decision.key_matched, existing_same_full_key,
                "{label} key_matched mismatch"
            );
            assert_eq!(
                decision.inserted, !existing_same_full_key,
                "{label} inserted mismatch"
            );
            assert!(decision.cap_called, "{label} cap must be enforced");
            assert_eq!(
                decision.len_before_cap, expected_len_before_cap,
                "{label} len_before_cap mismatch"
            );
            assert_eq!(
                decision.eviction_count, expected_eviction_count,
                "{label} eviction_count mismatch"
            );
            assert_eq!(
                decision.final_len, expected_final_len,
                "{label} final_len mismatch"
            );
        }

        let base = deferred_block_sync_cache_key(7, 3, hash(0x10));
        assert_ne!(base, deferred_block_sync_cache_key(8, 3, hash(0x10)));
        assert_ne!(base, deferred_block_sync_cache_key(7, 4, hash(0x10)));
        assert_ne!(base, deferred_block_sync_cache_key(7, 3, hash(0x11)));

        assert_cache_case("cache_new_entry", 0, false, 0, 1, 0, 1);
        assert_cache_case("cache_new_sender_none", 0, false, 0, 1, 0, 1);
        assert_cache_case("cache_same_key_fills_missing_qc", 1, true, 0, 1, 0, 1);
        assert_cache_case("cache_same_key_preserves_existing_qc", 1, true, 0, 1, 0, 1);
        assert_cache_case("cache_distinct_height_inserts", 1, false, 0, 2, 0, 2);
        assert_cache_case("cache_distinct_view_inserts", 1, false, 0, 2, 0, 2);
        assert_cache_case("cache_distinct_hash_inserts", 1, false, 0, 2, 0, 2);
        assert_cache_case("cache_cap_after_insert", 1, false, 1, 2, 1, 1);
        assert_cache_case("cache_cap_after_merge", 2, true, 1, 2, 1, 1);

        let fill_missing = deferred_block_sync_merge_decision(
            false, true, false, false, false, false, false, false,
        );
        assert!(fill_missing.take_incoming_commit_qc);
        assert!(fill_missing.final_commit_qc_present);

        let preserve_existing = deferred_block_sync_merge_decision(
            true, true, false, false, false, false, false, false,
        );
        assert!(!preserve_existing.take_incoming_commit_qc);
        assert!(preserve_existing.final_commit_qc_present);

        let sender_none_preserves = deferred_block_sync_merge_decision(
            false, false, false, false, false, false, true, false,
        );
        assert!(!sender_none_preserves.replace_sender);
        assert!(sender_none_preserves.final_sender_present);

        let sender_some_replaces = deferred_block_sync_merge_decision(
            false, false, false, false, false, false, true, true,
        );
        assert!(sender_some_replaces.replace_sender);
        assert!(sender_some_replaces.final_sender_present);

        let defer_record = deferred_block_sync_defer_record_decision();
        assert!(defer_record.cache_called);
        assert!(defer_record.record_called);
        assert!(defer_record.record_after_cache);
        assert_eq!(
            defer_record.recorded_kind,
            super::status::ConsensusMessageKind::BlockSyncUpdate
        );
        assert_eq!(
            defer_record.recorded_outcome,
            super::status::ConsensusMessageOutcome::Deferred
        );
        assert_eq!(
            defer_record.recorded_reason,
            super::status::ConsensusMessageReason::CommitPipelineActive
        );
    }

    #[test]
    fn deferred_block_sync_replay_formal_gate_matrix() {
        fn hash(byte: u8) -> HashOf<BlockHeader> {
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([byte; Hash::LENGTH]))
        }

        fn assert_replay_case(
            label: &str,
            initial_len: usize,
            commit_inflight: bool,
            validation_inflight: bool,
            remove_succeeds: bool,
            handler_errors: bool,
            expected_return: bool,
            expected_final_len: usize,
            expected_warn: bool,
        ) {
            let decision = deferred_block_sync_replay_decision(
                initial_len,
                commit_inflight,
                validation_inflight,
                remove_succeeds,
                handler_errors,
            );
            assert_eq!(
                decision.returns_progress, expected_return,
                "{label} return mismatch"
            );
            assert_eq!(
                decision.select_key,
                initial_len > 0 && !commit_inflight && !validation_inflight,
                "{label} selected-key admission mismatch"
            );
            assert_eq!(
                decision.remove_before_handle, expected_return,
                "{label} remove-before-handle mismatch"
            );
            assert_eq!(
                decision.handle_called, expected_return,
                "{label} handle-called mismatch"
            );
            assert_eq!(
                decision.update_forwarded, expected_return,
                "{label} update-forwarded mismatch"
            );
            assert_eq!(
                decision.sender_forwarded, expected_return,
                "{label} sender-forwarded mismatch"
            );
            assert_eq!(
                decision.warn_on_error, expected_warn,
                "{label} warn-on-error mismatch"
            );
            assert_eq!(
                decision.final_len, expected_final_len,
                "{label} final-len mismatch"
            );
            assert!(
                decision.later_entries_preserved,
                "{label} later entries should be preserved when present"
            );
        }

        assert_replay_case(
            "empty_queue",
            0,
            false,
            false,
            false,
            false,
            false,
            0,
            false,
        );
        assert_replay_case(
            "commit_inflight",
            1,
            true,
            false,
            false,
            false,
            false,
            1,
            false,
        );
        assert_replay_case(
            "validation_inflight",
            1,
            false,
            true,
            false,
            false,
            false,
            1,
            false,
        );
        assert_replay_case(
            "single_success",
            1,
            false,
            false,
            true,
            false,
            true,
            0,
            false,
        );
        assert_replay_case("single_error", 1, false, false, true, true, true, 0, true);
        assert_replay_case(
            "remove_missing",
            1,
            false,
            false,
            false,
            false,
            false,
            1,
            false,
        );
        assert_replay_case(
            "multiple_select_first",
            2,
            false,
            false,
            true,
            false,
            true,
            1,
            false,
        );

        let early = deferred_block_sync_cache_key(7, 1, hash(0x01));
        let late = deferred_block_sync_cache_key(7, 2, hash(0x02));
        let mut keys = std::collections::BTreeMap::new();
        keys.insert(late, ());
        keys.insert(early, ());
        assert_eq!(
            keys.keys().next().copied(),
            Some(early),
            "replay must select the first ordered deferred key"
        );
    }

    #[test]
    fn block_sync_future_window_formal_gate_matrix() {
        #[derive(Debug, Clone, Copy)]
        struct Case {
            label: &'static str,
            known_block: bool,
            requested_missing_block: bool,
            parent_available: bool,
            local_height: u64,
            raw_margin: u64,
            height: u64,
            view: u64,
            lower_missing_height: Option<u64>,
            base_height: u64,
            height_window: u64,
            view_window: u64,
            base_view: Option<u64>,
            view_age_expired: bool,
            expected_drop: bool,
        }

        fn generic_drop(case: Case) -> bool {
            !matches!(
                future_consensus_message_drop_decision(
                    case.height,
                    case.view,
                    "BlockSyncUpdate",
                    case.base_height,
                    case.base_view,
                    case.height_window,
                    case.view_window,
                    case.view_age_expired,
                ),
                FutureConsensusMessageDropDecision::Allow
            )
        }

        let default = Case {
            label: "default",
            known_block: false,
            requested_missing_block: false,
            parent_available: false,
            local_height: 3,
            raw_margin: 1,
            height: 6,
            view: 0,
            lower_missing_height: None,
            base_height: 3,
            height_window: 1,
            view_window: 0,
            base_view: Some(0),
            view_age_expired: false,
            expected_drop: true,
        };

        let cases = [
            Case {
                label: "known_block",
                known_block: true,
                expected_drop: false,
                ..default
            },
            Case {
                label: "requested_within_margin",
                requested_missing_block: true,
                raw_margin: 3,
                expected_drop: false,
                ..default
            },
            Case {
                label: "requested_far",
                requested_missing_block: true,
                expected_drop: true,
                ..default
            },
            Case {
                label: "requested_far_known_parent",
                requested_missing_block: true,
                parent_available: true,
                expected_drop: true,
                ..default
            },
            Case {
                label: "requested_saturated_boundary",
                requested_missing_block: true,
                local_height: 9,
                raw_margin: 0,
                height: 9,
                expected_drop: false,
                ..default
            },
            Case {
                label: "unrequested_lower_missing_far",
                lower_missing_height: Some(4),
                expected_drop: true,
                ..default
            },
            Case {
                label: "unrequested_lower_missing_far_known_parent",
                parent_available: true,
                lower_missing_height: Some(4),
                expected_drop: true,
                ..default
            },
            Case {
                label: "unrequested_lower_missing_same_height",
                raw_margin: 8,
                lower_missing_height: Some(6),
                height_window: 8,
                expected_drop: false,
                ..default
            },
            Case {
                label: "unrequested_known_parent_far",
                parent_available: true,
                expected_drop: false,
                ..default
            },
            Case {
                label: "unrequested_parent_before_view_gate",
                parent_available: true,
                raw_margin: 8,
                height: 3,
                view: 2,
                height_window: 0,
                view_window: 1,
                expected_drop: false,
                ..default
            },
            Case {
                label: "unrequested_generic_windows_disabled",
                raw_margin: 8,
                height_window: 0,
                expected_drop: false,
                ..default
            },
            Case {
                label: "unrequested_generic_height_drop",
                raw_margin: 8,
                expected_drop: true,
                ..default
            },
            Case {
                label: "unrequested_generic_height_boundary",
                raw_margin: 8,
                height: 4,
                expected_drop: false,
                ..default
            },
            Case {
                label: "unrequested_generic_view_drop",
                raw_margin: 8,
                height: 3,
                view: 2,
                height_window: 0,
                view_window: 1,
                expected_drop: true,
                ..default
            },
            Case {
                label: "unrequested_generic_view_boundary",
                raw_margin: 8,
                height: 3,
                view: 1,
                height_window: 0,
                view_window: 1,
                expected_drop: false,
                ..default
            },
            Case {
                label: "unrequested_generic_view_age_expired",
                raw_margin: 8,
                height: 3,
                view: 2,
                height_window: 0,
                view_window: 1,
                view_age_expired: true,
                expected_drop: false,
                ..default
            },
            Case {
                label: "unrequested_generic_no_phase_view",
                raw_margin: 8,
                height: 3,
                view: 2,
                height_window: 0,
                view_window: 1,
                base_view: None,
                expected_drop: false,
                ..default
            },
        ];

        for case in cases {
            let margin = block_sync_future_window_requested_margin(case.raw_margin);
            let far_ahead =
                block_sync_future_window_far_ahead(case.height, case.local_height, margin);
            let lower_unresolved =
                block_sync_future_window_lower_unresolved(case.lower_missing_height, case.height);
            let pre_generic = block_sync_future_window_pre_generic_drop(
                case.known_block,
                case.requested_missing_block,
                far_ahead,
                lower_unresolved,
                case.parent_available,
            );
            let actual = block_sync_future_window_drop_decision(
                case.known_block,
                case.requested_missing_block,
                far_ahead,
                lower_unresolved,
                case.parent_available,
                generic_drop(case),
            );
            assert_eq!(actual, case.expected_drop, "{} drop mismatch", case.label);
            if pre_generic.is_none() {
                assert_eq!(
                    actual,
                    generic_drop(case),
                    "{} generic fallback mismatch",
                    case.label
                );
            }
        }

        assert!(matches!(
            future_consensus_message_drop_decision(3, 10, "NewViewVote", 3, Some(0), 0, 1, false),
            FutureConsensusMessageDropDecision::Allow
        ));
    }

    #[test]
    fn block_sync_selected_qc_prefilter_formal_gate_matrix() {
        #[derive(Debug, Clone, Copy)]
        enum Candidate {
            EmptyTopology,
            HashMismatch,
            HeightMismatch,
            EpochMismatch,
            PhaseMismatch,
            SameHeightConflictDrop,
            SameHeightConflictRecoverable,
            StaleLockDrop,
            NonextendingDefer,
            NonextendingDropWithLock,
            NonextendingDropWithoutLock,
            NonextendingAllowedRetain,
            ExtendingContinues,
            NoLockContinues,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        struct Decision {
            topology_recovery: bool,
            shape_ignored: bool,
            same_height_locked_drop: bool,
            locked_prefilter_metric: bool,
            log_locked_conflict: bool,
            stale_locked_drop: bool,
            extends_computed: bool,
            nonextending_defer: bool,
            nonextending_locked_drop: bool,
            quarantine_locked_payload: bool,
            record_locked_drop: bool,
            retain_nonextending: bool,
            tally_attempted: bool,
            process_precommit_attempted: bool,
            returns_ok_before_tally: bool,
            returns_ok: bool,
        }

        fn topology_empty(candidate: Candidate) -> bool {
            matches!(candidate, Candidate::EmptyTopology)
        }

        fn hash_matches(candidate: Candidate) -> bool {
            !matches!(candidate, Candidate::HashMismatch)
        }

        fn height_matches(candidate: Candidate) -> bool {
            !matches!(candidate, Candidate::HeightMismatch)
        }

        fn epoch_matches(candidate: Candidate) -> bool {
            !matches!(candidate, Candidate::EpochMismatch)
        }

        fn commit_phase(candidate: Candidate) -> bool {
            !matches!(candidate, Candidate::PhaseMismatch)
        }

        fn shape_ok(candidate: Candidate) -> bool {
            !topology_empty(candidate)
                && hash_matches(candidate)
                && height_matches(candidate)
                && epoch_matches(candidate)
                && commit_phase(candidate)
        }

        fn same_height_conflict(candidate: Candidate) -> bool {
            matches!(
                candidate,
                Candidate::SameHeightConflictDrop | Candidate::SameHeightConflictRecoverable
            )
        }

        fn allow_nonextending(candidate: Candidate) -> bool {
            matches!(
                candidate,
                Candidate::SameHeightConflictRecoverable | Candidate::NonextendingAllowedRetain
            )
        }

        fn stale_against_lock(candidate: Candidate) -> bool {
            matches!(candidate, Candidate::StaleLockDrop)
        }

        fn extends_locked(candidate: Candidate) -> bool {
            matches!(
                candidate,
                Candidate::ExtendingContinues | Candidate::NoLockContinues
            )
        }

        fn defer_missing_locked_payload(candidate: Candidate) -> bool {
            matches!(candidate, Candidate::NonextendingDefer)
        }

        let candidates = [
            Candidate::EmptyTopology,
            Candidate::HashMismatch,
            Candidate::HeightMismatch,
            Candidate::EpochMismatch,
            Candidate::PhaseMismatch,
            Candidate::SameHeightConflictDrop,
            Candidate::SameHeightConflictRecoverable,
            Candidate::StaleLockDrop,
            Candidate::NonextendingDefer,
            Candidate::NonextendingDropWithLock,
            Candidate::NonextendingDropWithoutLock,
            Candidate::NonextendingAllowedRetain,
            Candidate::ExtendingContinues,
            Candidate::NoLockContinues,
        ];

        for candidate in candidates {
            let spec_topology_recovery = topology_empty(candidate);
            let spec_shape_ignored = !topology_empty(candidate)
                && (!hash_matches(candidate)
                    || !height_matches(candidate)
                    || !epoch_matches(candidate)
                    || !commit_phase(candidate));
            let spec_same_height_recoverable =
                same_height_conflict(candidate) && allow_nonextending(candidate);
            let spec_same_height_locked_drop = shape_ok(candidate)
                && same_height_conflict(candidate)
                && !spec_same_height_recoverable;
            let spec_stale_locked_drop = shape_ok(candidate)
                && !spec_same_height_locked_drop
                && stale_against_lock(candidate);
            let spec_extends_computed =
                shape_ok(candidate) && !spec_same_height_locked_drop && !spec_stale_locked_drop;
            let spec_nonextending_needs_resolution = spec_extends_computed
                && !extends_locked(candidate)
                && !allow_nonextending(candidate);
            let spec_nonextending_defer =
                spec_nonextending_needs_resolution && defer_missing_locked_payload(candidate);
            let spec_nonextending_locked_drop =
                spec_nonextending_needs_resolution && !defer_missing_locked_payload(candidate);
            let spec_record_locked_drop = spec_same_height_locked_drop
                || spec_stale_locked_drop
                || spec_nonextending_defer
                || spec_nonextending_locked_drop;
            let spec_retain_nonextending = spec_extends_computed
                && !extends_locked(candidate)
                && allow_nonextending(candidate);
            let spec_tally_attempted = spec_extends_computed
                && (extends_locked(candidate) || allow_nonextending(candidate));
            let spec_returns_ok_before_tally = spec_topology_recovery
                || spec_shape_ignored
                || spec_same_height_locked_drop
                || spec_stale_locked_drop
                || spec_nonextending_defer
                || spec_nonextending_locked_drop;
            let expected = Decision {
                topology_recovery: spec_topology_recovery,
                shape_ignored: spec_shape_ignored,
                same_height_locked_drop: spec_same_height_locked_drop,
                locked_prefilter_metric: spec_same_height_locked_drop,
                log_locked_conflict: spec_same_height_locked_drop
                    || matches!(candidate, Candidate::NonextendingDropWithLock),
                stale_locked_drop: spec_stale_locked_drop,
                extends_computed: spec_extends_computed,
                nonextending_defer: spec_nonextending_defer,
                nonextending_locked_drop: spec_nonextending_locked_drop,
                quarantine_locked_payload: spec_nonextending_defer,
                record_locked_drop: spec_record_locked_drop,
                retain_nonextending: spec_retain_nonextending,
                tally_attempted: spec_tally_attempted,
                process_precommit_attempted: spec_tally_attempted,
                returns_ok_before_tally: spec_returns_ok_before_tally,
                returns_ok: spec_returns_ok_before_tally,
            };

            let topology_recovery =
                block_sync_selected_qc_prefilter_topology_recovery(topology_empty(candidate));
            let hash_mismatch =
                block_sync_selected_qc_prefilter_hash_mismatch(hash_matches(candidate));
            let height_mismatch =
                block_sync_selected_qc_prefilter_height_mismatch(height_matches(candidate));
            let epoch_mismatch =
                block_sync_selected_qc_prefilter_epoch_mismatch(epoch_matches(candidate));
            let phase_mismatch =
                block_sync_selected_qc_prefilter_phase_mismatch(commit_phase(candidate));
            let shape_ignored = !topology_recovery
                && (hash_mismatch || height_mismatch || epoch_mismatch || phase_mismatch);
            let actual_shape_ok = !topology_recovery && !shape_ignored;
            let same_height_recoverable =
                same_height_conflict(candidate) && allow_nonextending(candidate);
            let same_height_locked_drop = actual_shape_ok
                && block_sync_selected_qc_prefilter_same_height_locked_drop(
                    same_height_conflict(candidate),
                    same_height_recoverable,
                );
            let stale_locked_drop = actual_shape_ok
                && !same_height_locked_drop
                && block_sync_selected_qc_prefilter_stale_locked_drop(stale_against_lock(
                    candidate,
                ));
            let extends_computed =
                actual_shape_ok && !same_height_locked_drop && !stale_locked_drop;
            let nonextending_needs_resolution = extends_computed
                && block_sync_selected_qc_prefilter_nonextending_needs_resolution(
                    extends_locked(candidate),
                    allow_nonextending(candidate),
                );
            let nonextending_defer = block_sync_selected_qc_prefilter_nonextending_defer(
                nonextending_needs_resolution,
                defer_missing_locked_payload(candidate),
            );
            let nonextending_locked_drop =
                block_sync_selected_qc_prefilter_nonextending_locked_drop(
                    nonextending_needs_resolution,
                    defer_missing_locked_payload(candidate),
                );
            let retain_nonextending = extends_computed
                && block_sync_selected_qc_prefilter_retain_nonextending(
                    extends_locked(candidate),
                    allow_nonextending(candidate),
                );
            let tally_attempted = extends_computed
                && !nonextending_defer
                && !nonextending_locked_drop
                && (extends_locked(candidate) || allow_nonextending(candidate));
            let returns_ok_before_tally = topology_recovery
                || shape_ignored
                || same_height_locked_drop
                || stale_locked_drop
                || nonextending_defer
                || nonextending_locked_drop;
            let actual = Decision {
                topology_recovery,
                shape_ignored,
                same_height_locked_drop,
                locked_prefilter_metric: same_height_locked_drop,
                log_locked_conflict: same_height_locked_drop
                    || (nonextending_locked_drop
                        && matches!(candidate, Candidate::NonextendingDropWithLock)),
                stale_locked_drop,
                extends_computed,
                nonextending_defer,
                nonextending_locked_drop,
                quarantine_locked_payload: nonextending_defer,
                record_locked_drop: same_height_locked_drop
                    || stale_locked_drop
                    || nonextending_defer
                    || nonextending_locked_drop,
                retain_nonextending,
                tally_attempted,
                process_precommit_attempted: tally_attempted,
                returns_ok_before_tally,
                returns_ok: returns_ok_before_tally,
            };

            assert_eq!(actual, expected, "{candidate:?} mismatch");
        }
    }

    #[test]
    fn block_sync_selected_qc_process_formal_gate_matrix() {
        #[derive(Debug, Clone, Copy)]
        enum Candidate {
            CachedTallyHitKnownPending,
            FreshTallyKnownPending,
            FreshTallyKnownInflight,
            FreshTallyKnownKura,
            FreshTallyUnknownPending,
            FreshTallyUnknownNoPending,
            ProcessReject,
            TallyError,
            RuntimeDaCleanup,
            RuntimeDaDisabled,
            AllowNonextendingForwarded,
            ReadyWithoutQc,
            CreationOkUnknownCache,
            CreationErrorNoCache,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        struct Decision {
            cached_tally_reused: bool,
            fresh_tally_called: bool,
            record_tally_validation_error: bool,
            record_precommit_signers: bool,
            note_validated_tally: bool,
            process_precommit_attempted: bool,
            process_block_known_arg: bool,
            process_allow_nonextending_arg: bool,
            record_commit_qc: bool,
            insert_qc_cache: bool,
            apply_commit_qc: bool,
            clean_rbc_sessions: bool,
            request_commit_pipeline: bool,
            observe_pending_epoch: bool,
            unknown_block_cache_called: bool,
            wrapper_returns_err: bool,
        }

        fn ready_apply_case(candidate: Candidate) -> bool {
            matches!(
                candidate,
                Candidate::CachedTallyHitKnownPending
                    | Candidate::FreshTallyKnownPending
                    | Candidate::FreshTallyKnownInflight
                    | Candidate::FreshTallyKnownKura
                    | Candidate::FreshTallyUnknownPending
                    | Candidate::FreshTallyUnknownNoPending
                    | Candidate::ProcessReject
                    | Candidate::TallyError
                    | Candidate::RuntimeDaCleanup
                    | Candidate::RuntimeDaDisabled
                    | Candidate::AllowNonextendingForwarded
            )
        }

        fn cached_tally_case(candidate: Candidate) -> bool {
            matches!(candidate, Candidate::CachedTallyHitKnownPending)
        }

        fn tally_error_case(candidate: Candidate) -> bool {
            matches!(candidate, Candidate::TallyError)
        }

        fn process_reject_case(candidate: Candidate) -> bool {
            matches!(candidate, Candidate::ProcessReject)
        }

        fn block_known_for_commit_case(candidate: Candidate) -> bool {
            matches!(
                candidate,
                Candidate::CachedTallyHitKnownPending
                    | Candidate::FreshTallyKnownPending
                    | Candidate::FreshTallyKnownInflight
                    | Candidate::FreshTallyKnownKura
                    | Candidate::ProcessReject
                    | Candidate::RuntimeDaCleanup
                    | Candidate::RuntimeDaDisabled
                    | Candidate::AllowNonextendingForwarded
            )
        }

        fn pending_block_valid(candidate: Candidate) -> bool {
            matches!(
                candidate,
                Candidate::CachedTallyHitKnownPending | Candidate::FreshTallyKnownPending
            )
        }

        fn inflight_block_active(candidate: Candidate) -> bool {
            matches!(candidate, Candidate::FreshTallyKnownInflight)
        }

        fn kura_block_known(candidate: Candidate) -> bool {
            matches!(
                candidate,
                Candidate::FreshTallyKnownKura
                    | Candidate::ProcessReject
                    | Candidate::RuntimeDaCleanup
                    | Candidate::RuntimeDaDisabled
                    | Candidate::AllowNonextendingForwarded
            )
        }

        fn pending_entry_exists(candidate: Candidate) -> bool {
            matches!(candidate, Candidate::FreshTallyUnknownPending)
        }

        fn runtime_da_enabled(candidate: Candidate) -> bool {
            matches!(candidate, Candidate::RuntimeDaCleanup)
        }

        fn allow_nonextending_input(candidate: Candidate) -> bool {
            matches!(candidate, Candidate::AllowNonextendingForwarded)
        }

        let candidates = [
            Candidate::CachedTallyHitKnownPending,
            Candidate::FreshTallyKnownPending,
            Candidate::FreshTallyKnownInflight,
            Candidate::FreshTallyKnownKura,
            Candidate::FreshTallyUnknownPending,
            Candidate::FreshTallyUnknownNoPending,
            Candidate::ProcessReject,
            Candidate::TallyError,
            Candidate::RuntimeDaCleanup,
            Candidate::RuntimeDaDisabled,
            Candidate::AllowNonextendingForwarded,
            Candidate::ReadyWithoutQc,
            Candidate::CreationOkUnknownCache,
            Candidate::CreationErrorNoCache,
        ];

        for candidate in candidates {
            let spec_tally_ok = ready_apply_case(candidate) && !tally_error_case(candidate);
            let spec_process_ok = spec_tally_ok && !process_reject_case(candidate);
            let spec_apply_commit_qc = spec_process_ok && block_known_for_commit_case(candidate);
            let expected = Decision {
                cached_tally_reused: cached_tally_case(candidate),
                fresh_tally_called: ready_apply_case(candidate) && !cached_tally_case(candidate),
                record_tally_validation_error: tally_error_case(candidate),
                record_precommit_signers: spec_tally_ok,
                note_validated_tally: spec_tally_ok,
                process_precommit_attempted: spec_tally_ok,
                process_block_known_arg: spec_tally_ok && block_known_for_commit_case(candidate),
                process_allow_nonextending_arg: spec_tally_ok
                    && allow_nonextending_input(candidate),
                record_commit_qc: spec_process_ok,
                insert_qc_cache: spec_process_ok,
                apply_commit_qc: spec_apply_commit_qc,
                clean_rbc_sessions: spec_apply_commit_qc && runtime_da_enabled(candidate),
                request_commit_pipeline: spec_apply_commit_qc,
                observe_pending_epoch: spec_process_ok
                    && !block_known_for_commit_case(candidate)
                    && pending_entry_exists(candidate),
                unknown_block_cache_called: matches!(candidate, Candidate::CreationOkUnknownCache),
                wrapper_returns_err: matches!(candidate, Candidate::CreationErrorNoCache),
            };

            let qc_present_for_apply = ready_apply_case(candidate);
            let tally_source = qc_present_for_apply
                .then(|| block_sync_selected_qc_process_tally_source(cached_tally_case(candidate)));
            let cached_tally_reused =
                tally_source == Some(BlockSyncSelectedQcProcessTallySource::Cached);
            let fresh_tally_called =
                tally_source == Some(BlockSyncSelectedQcProcessTallySource::Fresh);
            let tally_ok = qc_present_for_apply && !tally_error_case(candidate);
            let block_known_for_commit = block_sync_selected_qc_process_block_known_for_commit(
                pending_block_valid(candidate),
                inflight_block_active(candidate),
                kura_block_known(candidate),
            );
            assert_eq!(
                block_known_for_commit,
                block_known_for_commit_case(candidate),
                "{candidate:?} block-known projection mismatch"
            );
            let process_precommit_attempted = tally_ok;
            let process_ok = tally_ok && !process_reject_case(candidate);
            let commit_qc_accepted = block_sync_selected_qc_process_commit_qc_accepted(process_ok);
            let apply_commit_qc = block_sync_selected_qc_process_apply_commit_qc(
                commit_qc_accepted,
                block_known_for_commit,
            );
            let unknown_qc_present = matches!(
                candidate,
                Candidate::CreationOkUnknownCache | Candidate::CreationErrorNoCache
            );
            let creation_ok = !matches!(candidate, Candidate::CreationErrorNoCache);
            let unknown_block_cache_called = block_sync_selected_qc_process_cache_unknown_block_qc(
                creation_ok,
                false,
                unknown_qc_present,
            );
            let actual = Decision {
                cached_tally_reused,
                fresh_tally_called,
                record_tally_validation_error: tally_error_case(candidate),
                record_precommit_signers: tally_ok,
                note_validated_tally: tally_ok,
                process_precommit_attempted,
                process_block_known_arg: process_precommit_attempted && block_known_for_commit,
                process_allow_nonextending_arg: process_precommit_attempted
                    && allow_nonextending_input(candidate),
                record_commit_qc: commit_qc_accepted,
                insert_qc_cache: commit_qc_accepted,
                apply_commit_qc,
                clean_rbc_sessions: block_sync_selected_qc_process_clean_rbc_sessions(
                    apply_commit_qc,
                    runtime_da_enabled(candidate),
                ),
                request_commit_pipeline: apply_commit_qc,
                observe_pending_epoch: block_sync_selected_qc_process_observe_pending_epoch(
                    commit_qc_accepted,
                    block_known_for_commit,
                    pending_entry_exists(candidate),
                ),
                unknown_block_cache_called,
                wrapper_returns_err: matches!(candidate, Candidate::CreationErrorNoCache),
            };

            assert_eq!(actual, expected, "{candidate:?} mismatch");
        }
    }

    #[test]
    fn block_sync_selected_qc_cache_formal_gate_matrix() {
        #[derive(Debug, Clone, Copy)]
        enum Candidate {
            EmptyTopology,
            HashMismatch,
            HeightMismatch,
            EpochMismatch,
            PhaseMismatch,
            SameHeightConflictDrop,
            SameHeightConflictRecoverable,
            StaleLockDrop,
            NonextendingDefer,
            NonextendingDrop,
            NonextendingAllowedRetain,
            ExtendingProcessOk,
            NoLockProcessOk,
            ProcessRejectLocked,
            AllowUpdateNoLock,
            AllowUpdateNewer,
            AllowNoUpdateOlder,
            AllowFalseNoUpdate,
            TallyMissingContext,
            TallyFinalError,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        struct Decision {
            topology_recovery: bool,
            shape_ignored: bool,
            same_height_locked_drop: bool,
            locked_prefilter_metric: bool,
            stale_locked_drop: bool,
            extends_computed: bool,
            nonextending_defer: bool,
            nonextending_drop: bool,
            quarantine_locked_payload: bool,
            record_locked_drop: bool,
            retain_nonextending: bool,
            tally_attempted: bool,
            tally_validation_error_recorded: bool,
            missing_context_quarantined: bool,
            final_drop: bool,
            final_error_quarantined: bool,
            record_precommit_signers: bool,
            note_validated_tally: bool,
            process_precommit_attempted: bool,
            process_block_known_false: bool,
            process_allow_nonextending_arg: bool,
            process_rejected: bool,
            process_reject_logs_conflict: bool,
            process_ok: bool,
            update_locked_qc: bool,
            prune_precommit_votes: bool,
            highest_qc_unchanged: bool,
            remove_quarantined_qc: bool,
            record_commit_qc: bool,
            insert_qc_cache: bool,
        }

        fn topology_empty(candidate: Candidate) -> bool {
            matches!(candidate, Candidate::EmptyTopology)
        }

        fn hash_matches(candidate: Candidate) -> bool {
            !matches!(candidate, Candidate::HashMismatch)
        }

        fn height_matches(candidate: Candidate) -> bool {
            !matches!(candidate, Candidate::HeightMismatch)
        }

        fn epoch_matches(candidate: Candidate) -> bool {
            !matches!(candidate, Candidate::EpochMismatch)
        }

        fn commit_phase(candidate: Candidate) -> bool {
            !matches!(candidate, Candidate::PhaseMismatch)
        }

        fn shape_ok(candidate: Candidate) -> bool {
            !topology_empty(candidate)
                && hash_matches(candidate)
                && height_matches(candidate)
                && epoch_matches(candidate)
                && commit_phase(candidate)
        }

        fn lock_present(candidate: Candidate) -> bool {
            matches!(
                candidate,
                Candidate::SameHeightConflictDrop
                    | Candidate::SameHeightConflictRecoverable
                    | Candidate::StaleLockDrop
                    | Candidate::NonextendingDefer
                    | Candidate::NonextendingDrop
                    | Candidate::NonextendingAllowedRetain
                    | Candidate::ExtendingProcessOk
                    | Candidate::ProcessRejectLocked
                    | Candidate::AllowUpdateNewer
                    | Candidate::AllowNoUpdateOlder
                    | Candidate::AllowFalseNoUpdate
            )
        }

        fn same_height_conflict(candidate: Candidate) -> bool {
            matches!(
                candidate,
                Candidate::SameHeightConflictDrop | Candidate::SameHeightConflictRecoverable
            )
        }

        fn allow_nonextending(candidate: Candidate) -> bool {
            matches!(
                candidate,
                Candidate::SameHeightConflictRecoverable
                    | Candidate::NonextendingAllowedRetain
                    | Candidate::AllowUpdateNoLock
                    | Candidate::AllowUpdateNewer
                    | Candidate::AllowNoUpdateOlder
            )
        }

        fn stale_against_lock(candidate: Candidate) -> bool {
            matches!(candidate, Candidate::StaleLockDrop)
        }

        fn extends_locked(candidate: Candidate) -> bool {
            matches!(
                candidate,
                Candidate::ExtendingProcessOk
                    | Candidate::NoLockProcessOk
                    | Candidate::ProcessRejectLocked
                    | Candidate::AllowUpdateNoLock
                    | Candidate::AllowUpdateNewer
                    | Candidate::AllowNoUpdateOlder
                    | Candidate::AllowFalseNoUpdate
                    | Candidate::TallyMissingContext
                    | Candidate::TallyFinalError
            )
        }

        fn defer_missing_locked_payload(candidate: Candidate) -> bool {
            matches!(candidate, Candidate::NonextendingDefer)
        }

        fn tally_error_case(candidate: Candidate) -> bool {
            matches!(
                candidate,
                Candidate::TallyMissingContext | Candidate::TallyFinalError
            )
        }

        fn missing_context_error(candidate: Candidate) -> bool {
            matches!(candidate, Candidate::TallyMissingContext)
        }

        fn final_validation_error(candidate: Candidate) -> bool {
            matches!(candidate, Candidate::TallyFinalError)
        }

        fn process_reject_case(candidate: Candidate) -> bool {
            matches!(candidate, Candidate::ProcessRejectLocked)
        }

        fn should_update_lock(candidate: Candidate) -> bool {
            matches!(
                candidate,
                Candidate::AllowUpdateNoLock | Candidate::AllowUpdateNewer
            )
        }

        let candidates = [
            Candidate::EmptyTopology,
            Candidate::HashMismatch,
            Candidate::HeightMismatch,
            Candidate::EpochMismatch,
            Candidate::PhaseMismatch,
            Candidate::SameHeightConflictDrop,
            Candidate::SameHeightConflictRecoverable,
            Candidate::StaleLockDrop,
            Candidate::NonextendingDefer,
            Candidate::NonextendingDrop,
            Candidate::NonextendingAllowedRetain,
            Candidate::ExtendingProcessOk,
            Candidate::NoLockProcessOk,
            Candidate::ProcessRejectLocked,
            Candidate::AllowUpdateNoLock,
            Candidate::AllowUpdateNewer,
            Candidate::AllowNoUpdateOlder,
            Candidate::AllowFalseNoUpdate,
            Candidate::TallyMissingContext,
            Candidate::TallyFinalError,
        ];

        for candidate in candidates {
            let spec_topology_recovery = topology_empty(candidate);
            let spec_shape_ignored = !topology_empty(candidate)
                && (!hash_matches(candidate)
                    || !height_matches(candidate)
                    || !epoch_matches(candidate)
                    || !commit_phase(candidate));
            let spec_same_height_recoverable =
                same_height_conflict(candidate) && allow_nonextending(candidate);
            let spec_same_height_locked_drop = shape_ok(candidate)
                && same_height_conflict(candidate)
                && !spec_same_height_recoverable;
            let spec_stale_locked_drop = shape_ok(candidate)
                && !spec_same_height_locked_drop
                && stale_against_lock(candidate);
            let spec_extends_computed =
                shape_ok(candidate) && !spec_same_height_locked_drop && !spec_stale_locked_drop;
            let spec_nonextending_needs_resolution = spec_extends_computed
                && !extends_locked(candidate)
                && !allow_nonextending(candidate);
            let spec_nonextending_defer =
                spec_nonextending_needs_resolution && defer_missing_locked_payload(candidate);
            let spec_nonextending_drop =
                spec_nonextending_needs_resolution && !defer_missing_locked_payload(candidate);
            let spec_record_locked_drop = spec_same_height_locked_drop
                || spec_stale_locked_drop
                || spec_nonextending_defer
                || spec_nonextending_drop;
            let spec_retain_nonextending = spec_extends_computed
                && !extends_locked(candidate)
                && allow_nonextending(candidate);
            let spec_tally_attempted = spec_extends_computed
                && (extends_locked(candidate) || allow_nonextending(candidate));
            let spec_tally_ok = spec_tally_attempted && !tally_error_case(candidate);
            let spec_process_rejected = spec_tally_ok && process_reject_case(candidate);
            let spec_process_ok = spec_tally_ok && !process_reject_case(candidate);
            let spec_update_locked_qc =
                spec_process_ok && allow_nonextending(candidate) && should_update_lock(candidate);
            let expected = Decision {
                topology_recovery: spec_topology_recovery,
                shape_ignored: spec_shape_ignored,
                same_height_locked_drop: spec_same_height_locked_drop,
                locked_prefilter_metric: spec_same_height_locked_drop,
                stale_locked_drop: spec_stale_locked_drop,
                extends_computed: spec_extends_computed,
                nonextending_defer: spec_nonextending_defer,
                nonextending_drop: spec_nonextending_drop,
                quarantine_locked_payload: spec_nonextending_defer,
                record_locked_drop: spec_record_locked_drop,
                retain_nonextending: spec_retain_nonextending,
                tally_attempted: spec_tally_attempted,
                tally_validation_error_recorded: spec_tally_attempted
                    && tally_error_case(candidate),
                missing_context_quarantined: spec_tally_attempted
                    && missing_context_error(candidate),
                final_drop: spec_tally_attempted && final_validation_error(candidate),
                final_error_quarantined: false,
                record_precommit_signers: spec_tally_ok,
                note_validated_tally: spec_tally_ok,
                process_precommit_attempted: spec_tally_ok,
                process_block_known_false: spec_tally_ok,
                process_allow_nonextending_arg: spec_tally_ok && allow_nonextending(candidate),
                process_rejected: spec_process_rejected,
                process_reject_logs_conflict: spec_process_rejected && lock_present(candidate),
                process_ok: spec_process_ok,
                update_locked_qc: spec_update_locked_qc,
                prune_precommit_votes: spec_update_locked_qc,
                highest_qc_unchanged: spec_process_ok,
                remove_quarantined_qc: spec_process_ok,
                record_commit_qc: spec_process_ok,
                insert_qc_cache: spec_process_ok,
            };

            let topology_recovery =
                block_sync_selected_qc_prefilter_topology_recovery(topology_empty(candidate));
            let shape_ignored = !topology_recovery
                && (block_sync_selected_qc_prefilter_hash_mismatch(hash_matches(candidate))
                    || block_sync_selected_qc_prefilter_height_mismatch(height_matches(candidate))
                    || block_sync_selected_qc_prefilter_epoch_mismatch(epoch_matches(candidate))
                    || block_sync_selected_qc_prefilter_phase_mismatch(commit_phase(candidate)));
            let actual_shape_ok = !topology_recovery && !shape_ignored;
            let same_height_recoverable =
                same_height_conflict(candidate) && allow_nonextending(candidate);
            let same_height_locked_drop = actual_shape_ok
                && block_sync_selected_qc_prefilter_same_height_locked_drop(
                    same_height_conflict(candidate),
                    same_height_recoverable,
                );
            let stale_locked_drop = actual_shape_ok
                && !same_height_locked_drop
                && block_sync_selected_qc_prefilter_stale_locked_drop(stale_against_lock(
                    candidate,
                ));
            let extends_computed =
                actual_shape_ok && !same_height_locked_drop && !stale_locked_drop;
            let nonextending_needs_resolution = extends_computed
                && block_sync_selected_qc_prefilter_nonextending_needs_resolution(
                    extends_locked(candidate),
                    allow_nonextending(candidate),
                );
            let nonextending_defer = block_sync_selected_qc_prefilter_nonextending_defer(
                nonextending_needs_resolution,
                defer_missing_locked_payload(candidate),
            );
            let nonextending_drop = block_sync_selected_qc_prefilter_nonextending_locked_drop(
                nonextending_needs_resolution,
                defer_missing_locked_payload(candidate),
            );
            let retain_nonextending = extends_computed
                && block_sync_selected_qc_prefilter_retain_nonextending(
                    extends_locked(candidate),
                    allow_nonextending(candidate),
                );
            let tally_attempted = extends_computed
                && !nonextending_defer
                && !nonextending_drop
                && (extends_locked(candidate) || allow_nonextending(candidate));
            let tally_ok = tally_attempted && !tally_error_case(candidate);
            let process_precommit_attempted = tally_ok;
            let process_rejected = tally_ok && process_reject_case(candidate);
            let process_ok = tally_ok && !process_reject_case(candidate);
            let commit_qc_accepted = block_sync_selected_qc_process_commit_qc_accepted(process_ok);
            let update_locked_qc = commit_qc_accepted
                && block_sync_selected_qc_cache_update_locked_qc(
                    allow_nonextending(candidate),
                    should_update_lock(candidate),
                );
            let actual = Decision {
                topology_recovery,
                shape_ignored,
                same_height_locked_drop,
                locked_prefilter_metric: same_height_locked_drop,
                stale_locked_drop,
                extends_computed,
                nonextending_defer,
                nonextending_drop,
                quarantine_locked_payload: nonextending_defer,
                record_locked_drop: same_height_locked_drop
                    || stale_locked_drop
                    || nonextending_defer
                    || nonextending_drop,
                retain_nonextending,
                tally_attempted,
                tally_validation_error_recorded: tally_attempted && tally_error_case(candidate),
                missing_context_quarantined: tally_attempted
                    && block_sync_selected_qc_cache_missing_context_quarantine(
                        missing_context_error(candidate),
                    ),
                final_drop: tally_attempted
                    && tally_error_case(candidate)
                    && block_sync_selected_qc_cache_final_validation_drop(missing_context_error(
                        candidate,
                    )),
                final_error_quarantined: false,
                record_precommit_signers: tally_ok,
                note_validated_tally: tally_ok,
                process_precommit_attempted,
                process_block_known_false: process_precommit_attempted,
                process_allow_nonextending_arg: process_precommit_attempted
                    && allow_nonextending(candidate),
                process_rejected,
                process_reject_logs_conflict: process_rejected && lock_present(candidate),
                process_ok: commit_qc_accepted,
                update_locked_qc,
                prune_precommit_votes: update_locked_qc,
                highest_qc_unchanged: commit_qc_accepted,
                remove_quarantined_qc: commit_qc_accepted,
                record_commit_qc: commit_qc_accepted,
                insert_qc_cache: commit_qc_accepted,
            };

            assert_eq!(actual, expected, "{candidate:?} mismatch");
        }
    }

    #[test]
    fn block_sync_snapshot_hint_formal_gate_matrix() {
        struct Case {
            label: &'static str,
            block_known: bool,
            snapshot_exists: bool,
            incoming_qc: bool,
            qc_hash_matches: bool,
            qc_same_validator_set: bool,
            incoming_checkpoint: bool,
            checkpoint_hash_matches: bool,
            incoming_stake: bool,
            local_stake_present: bool,
            stake_hash_matches: bool,
            expected: BlockSyncSnapshotHintFilter,
        }

        let none = BlockSyncSnapshotHintFilter {
            snapshot_present: true,
            qc_after: false,
            qc_revalidated: false,
            checkpoint_after: false,
            stake_after: false,
        };
        let cases = [
            Case {
                label: "unknown_snapshot_hints",
                block_known: false,
                snapshot_exists: true,
                incoming_qc: true,
                qc_hash_matches: false,
                qc_same_validator_set: false,
                incoming_checkpoint: true,
                checkpoint_hash_matches: false,
                incoming_stake: true,
                local_stake_present: false,
                stake_hash_matches: false,
                expected: BlockSyncSnapshotHintFilter {
                    snapshot_present: false,
                    qc_after: true,
                    qc_revalidated: false,
                    checkpoint_after: true,
                    stake_after: true,
                },
            },
            Case {
                label: "known_no_snapshot_hints",
                block_known: true,
                snapshot_exists: false,
                incoming_qc: true,
                qc_hash_matches: false,
                qc_same_validator_set: false,
                incoming_checkpoint: true,
                checkpoint_hash_matches: false,
                incoming_stake: true,
                local_stake_present: false,
                stake_hash_matches: false,
                expected: BlockSyncSnapshotHintFilter {
                    snapshot_present: false,
                    qc_after: true,
                    qc_revalidated: false,
                    checkpoint_after: true,
                    stake_after: true,
                },
            },
            Case {
                label: "known_no_hints",
                block_known: true,
                snapshot_exists: true,
                incoming_qc: false,
                qc_hash_matches: false,
                qc_same_validator_set: false,
                incoming_checkpoint: false,
                checkpoint_hash_matches: false,
                incoming_stake: false,
                local_stake_present: false,
                stake_hash_matches: false,
                expected: none,
            },
            Case {
                label: "known_matching_qc",
                block_known: true,
                snapshot_exists: true,
                incoming_qc: true,
                qc_hash_matches: true,
                qc_same_validator_set: true,
                incoming_checkpoint: false,
                checkpoint_hash_matches: false,
                incoming_stake: false,
                local_stake_present: false,
                stake_hash_matches: false,
                expected: BlockSyncSnapshotHintFilter {
                    qc_after: true,
                    ..none
                },
            },
            Case {
                label: "known_same_roster_diff_qc",
                block_known: true,
                snapshot_exists: true,
                incoming_qc: true,
                qc_hash_matches: false,
                qc_same_validator_set: true,
                incoming_checkpoint: false,
                checkpoint_hash_matches: false,
                incoming_stake: false,
                local_stake_present: false,
                stake_hash_matches: false,
                expected: BlockSyncSnapshotHintFilter {
                    qc_after: true,
                    qc_revalidated: true,
                    ..none
                },
            },
            Case {
                label: "known_diff_roster_qc",
                block_known: true,
                snapshot_exists: true,
                incoming_qc: true,
                qc_hash_matches: false,
                qc_same_validator_set: false,
                incoming_checkpoint: false,
                checkpoint_hash_matches: false,
                incoming_stake: false,
                local_stake_present: false,
                stake_hash_matches: false,
                expected: none,
            },
            Case {
                label: "known_same_hash_diff_roster_qc",
                block_known: true,
                snapshot_exists: true,
                incoming_qc: true,
                qc_hash_matches: true,
                qc_same_validator_set: false,
                incoming_checkpoint: false,
                checkpoint_hash_matches: false,
                incoming_stake: false,
                local_stake_present: false,
                stake_hash_matches: false,
                expected: BlockSyncSnapshotHintFilter {
                    qc_after: true,
                    ..none
                },
            },
            Case {
                label: "known_matching_checkpoint",
                block_known: true,
                snapshot_exists: true,
                incoming_qc: false,
                qc_hash_matches: false,
                qc_same_validator_set: false,
                incoming_checkpoint: true,
                checkpoint_hash_matches: true,
                incoming_stake: false,
                local_stake_present: false,
                stake_hash_matches: false,
                expected: BlockSyncSnapshotHintFilter {
                    checkpoint_after: true,
                    ..none
                },
            },
            Case {
                label: "known_mismatch_checkpoint",
                block_known: true,
                snapshot_exists: true,
                incoming_qc: false,
                qc_hash_matches: false,
                qc_same_validator_set: false,
                incoming_checkpoint: true,
                checkpoint_hash_matches: false,
                incoming_stake: false,
                local_stake_present: false,
                stake_hash_matches: false,
                expected: none,
            },
            Case {
                label: "known_matching_stake",
                block_known: true,
                snapshot_exists: true,
                incoming_qc: false,
                qc_hash_matches: false,
                qc_same_validator_set: false,
                incoming_checkpoint: false,
                checkpoint_hash_matches: false,
                incoming_stake: true,
                local_stake_present: true,
                stake_hash_matches: true,
                expected: BlockSyncSnapshotHintFilter {
                    stake_after: true,
                    ..none
                },
            },
            Case {
                label: "known_no_local_stake",
                block_known: true,
                snapshot_exists: true,
                incoming_qc: false,
                qc_hash_matches: false,
                qc_same_validator_set: false,
                incoming_checkpoint: false,
                checkpoint_hash_matches: false,
                incoming_stake: true,
                local_stake_present: false,
                stake_hash_matches: false,
                expected: none,
            },
            Case {
                label: "known_mismatch_stake",
                block_known: true,
                snapshot_exists: true,
                incoming_qc: false,
                qc_hash_matches: false,
                qc_same_validator_set: false,
                incoming_checkpoint: false,
                checkpoint_hash_matches: false,
                incoming_stake: true,
                local_stake_present: true,
                stake_hash_matches: false,
                expected: none,
            },
            Case {
                label: "known_all_matching",
                block_known: true,
                snapshot_exists: true,
                incoming_qc: true,
                qc_hash_matches: true,
                qc_same_validator_set: true,
                incoming_checkpoint: true,
                checkpoint_hash_matches: true,
                incoming_stake: true,
                local_stake_present: true,
                stake_hash_matches: true,
                expected: BlockSyncSnapshotHintFilter {
                    snapshot_present: true,
                    qc_after: true,
                    qc_revalidated: false,
                    checkpoint_after: true,
                    stake_after: true,
                },
            },
            Case {
                label: "known_all_mismatch",
                block_known: true,
                snapshot_exists: true,
                incoming_qc: true,
                qc_hash_matches: false,
                qc_same_validator_set: false,
                incoming_checkpoint: true,
                checkpoint_hash_matches: false,
                incoming_stake: true,
                local_stake_present: true,
                stake_hash_matches: false,
                expected: none,
            },
        ];

        for case in cases {
            let filter = block_sync_snapshot_hint_filter(
                case.block_known && case.snapshot_exists,
                case.incoming_qc,
                case.qc_hash_matches,
                case.qc_same_validator_set,
                case.incoming_checkpoint,
                case.checkpoint_hash_matches,
                case.incoming_stake,
                case.local_stake_present,
                case.stake_hash_matches,
            );
            assert_eq!(filter, case.expected, "{} mismatch", case.label);
        }
    }

    #[test]
    fn block_sync_vote_placeholder_formal_gate_matrix() {
        let block_hash =
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x44; Hash::LENGTH]));
        let other_hash =
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x45; Hash::LENGTH]));
        let height = 7_u64;
        let view = 3_u64;
        let epoch = 2_u64;
        let chain_order_hash = crate::sumeragi::consensus::default_chain_order_hash();
        let make_vote = |phase: Phase,
                         block_hash: HashOf<BlockHeader>,
                         height: u64,
                         view: u64,
                         epoch: u64,
                         signer: ValidatorIndex| {
            Vote {
                phase,
                block_hash,
                parent_state_root: Hash::prehashed([0; Hash::LENGTH]),
                post_state_root: Hash::prehashed([1; Hash::LENGTH]),
                height,
                view,
                epoch,
                chain_order_hash,
                rechain_seq: 0,
                highest_qc: None,
                signer,
                bls_sig: Vec::new(),
            }
        };
        let valid_vote = make_vote(Phase::Commit, block_hash, height, view, epoch, 0);

        #[allow(clippy::type_complexity)]
        let cases: Vec<(&str, Vec<Vote>, bool, bool, bool, bool, bool, bool, usize)> = vec![
            (
                "no_votes",
                Vec::new(),
                false,
                false,
                true,
                false,
                false,
                false,
                0,
            ),
            (
                "valid_vote",
                vec![valid_vote.clone()],
                false,
                false,
                true,
                false,
                false,
                true,
                1,
            ),
            (
                "two_valid_votes",
                vec![
                    valid_vote.clone(),
                    make_vote(Phase::Commit, block_hash, height, view, epoch, 1),
                ],
                false,
                false,
                true,
                false,
                false,
                true,
                2,
            ),
            (
                "invalid_phase",
                vec![make_vote(
                    Phase::Prepare,
                    block_hash,
                    height,
                    view,
                    epoch,
                    0,
                )],
                false,
                false,
                true,
                false,
                false,
                true,
                0,
            ),
            (
                "invalid_hash",
                vec![make_vote(Phase::Commit, other_hash, height, view, epoch, 0)],
                false,
                false,
                true,
                false,
                false,
                true,
                0,
            ),
            (
                "invalid_height",
                vec![make_vote(
                    Phase::Commit,
                    block_hash,
                    height.saturating_add(1),
                    view,
                    epoch,
                    0,
                )],
                false,
                false,
                true,
                false,
                false,
                true,
                0,
            ),
            (
                "invalid_view",
                vec![make_vote(
                    Phase::Commit,
                    block_hash,
                    height,
                    view.saturating_add(1),
                    epoch,
                    0,
                )],
                false,
                false,
                true,
                false,
                false,
                true,
                0,
            ),
            (
                "invalid_epoch",
                vec![make_vote(
                    Phase::Commit,
                    block_hash,
                    height,
                    view,
                    epoch.saturating_add(1),
                    0,
                )],
                false,
                false,
                true,
                false,
                false,
                true,
                0,
            ),
            (
                "mixed_votes",
                vec![
                    valid_vote.clone(),
                    make_vote(Phase::Commit, other_hash, height, view, epoch, 1),
                ],
                false,
                false,
                true,
                false,
                false,
                true,
                1,
            ),
            (
                "with_qc_sidecar",
                vec![valid_vote.clone()],
                true,
                false,
                true,
                false,
                false,
                false,
                0,
            ),
            (
                "with_checkpoint_sidecar",
                vec![valid_vote.clone()],
                false,
                true,
                true,
                false,
                false,
                false,
                0,
            ),
            (
                "with_stake_sidecar",
                vec![valid_vote.clone()],
                false,
                false,
                true,
                false,
                false,
                true,
                1,
            ),
            (
                "not_exact_frontier",
                vec![valid_vote.clone()],
                false,
                false,
                false,
                false,
                false,
                false,
                0,
            ),
            (
                "known_local",
                vec![valid_vote.clone()],
                false,
                false,
                true,
                true,
                false,
                false,
                0,
            ),
            (
                "already_requested",
                vec![valid_vote],
                false,
                false,
                true,
                false,
                true,
                false,
                0,
            ),
        ];

        for (
            label,
            votes,
            incoming_qc_present,
            validator_checkpoint_present,
            exact_contiguous_frontier,
            block_known_locally,
            requested_missing_block,
            expected_gate,
            expected_placeholders,
        ) in cases
        {
            let gate = should_note_block_sync_vote_placeholder(
                !votes.is_empty(),
                incoming_qc_present,
                validator_checkpoint_present,
                exact_contiguous_frontier,
                block_known_locally,
                requested_missing_block,
            );
            assert_eq!(gate, expected_gate, "{label} gate mismatch");
            let placeholders = gate
                .then(|| {
                    votes
                        .iter()
                        .filter(|vote| {
                            block_sync_vote_placeholder_matches(
                                vote, block_hash, height, view, epoch,
                            )
                        })
                        .count()
                })
                .unwrap_or_default();
            assert_eq!(
                placeholders, expected_placeholders,
                "{label} placeholder count mismatch"
            );
        }
    }
}
