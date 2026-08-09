#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! End-to-end regressions for the authoritative Sumeragi v2 production runner.

use std::{
    collections::{BTreeMap, BTreeSet},
    time::{Duration, Instant},
};

use eyre::{Result, WrapErr, ensure, eyre};
use futures_util::future::try_join_all;
use integration_tests::sandbox;
use iroha::{
    client::Client,
    crypto::{Algorithm, Hash, HashOf, KeyPair},
    data_model::{
        Identifiable, Level,
        account::{Account, AccountId},
        block::{
            BlockHeader,
            consensus_v2::{
                BlockSubject, ConsensusMode, DualQuorum, ExecutionCommitment, GlobalPhase,
                HeightContextId, PROTOCOL_VERSION, QuorumCertificateRef, SumeragiV2BodyState,
                SumeragiV2CommitQcStatus, SumeragiV2HeightContextStatus, SumeragiV2IgnoreReason,
                SumeragiV2LivenessBlocker, SumeragiV2LivenessStatus, SumeragiV2OutboundIntentKind,
                SumeragiV2OutboundIntentStage, SumeragiV2Status, SumeragiV2VoteQuorumStatus,
                TimeoutCertificateRef, ValidatorIndex,
            },
        },
        bridge::{BridgeFinalityProof, verify_bridge_finality_proof},
        isi::{InstructionBox, Log, Register, register::RegisterBox},
        parameter::system::SumeragiNposParameters,
        peer::PeerId,
        prelude::FindAccountById,
        query::{
            account::prelude::FindAccounts, block::prelude::FindBlocks, prelude::QueryBuilderExt,
        },
        transaction::Executable,
    },
};
use iroha_test_network::{
    ConsensusMessageControlAck, ConsensusMessageControlAction, ConsensusMessageControlKind,
    ConsensusMessageControlRule, NetworkBuilder, NetworkPeer, ObserverP2pBootstrap,
    ObserverSlowReaderRelayConfig, init_instruction_registry,
};
use norito::json::Value;
use tokio::{task, time::sleep};

const VALIDATOR_COUNT: usize = 4;
const LOCKED_REPROPOSAL_HEIGHT: u64 = 2;
const LOCKED_REPROPOSAL_FIRST_VIEW: u64 = 0;
const LOCKED_REPROPOSAL_SECOND_VIEW: u64 = 1;
const LOCKED_REPROPOSAL_QUEUE_CAPACITY: usize = 256;
const LOCKED_REPROPOSAL_REMOTE_QUORUM_VOTES: usize = 2;
const DISTINCT_PREPARE_QC_QUEUE_CAPACITY: usize = 512;
const DISTINCT_PREPARE_QC_BLOCK_CADENCE: Duration = Duration::from_secs(8);
const DISTINCT_PREPARE_QC_VIEW_ZERO_TIMEOUT: Duration = Duration::from_secs(120);
const DISTINCT_PREPARE_QC_A_SELECTION_TIMEOUT: Duration = Duration::from_secs(25);
const DISTINCT_PREPARE_QC_A_RELEASE_BUDGET: Duration = Duration::from_secs(40);
// The restart scenario exercises durable recovery, not the localnet's
// accelerated 333 ms cadence. Its debug-build genesis validation runs on four
// real peers, so use a signed cadence whose view-zero deadline remains useful
// under ordinary shared-CI contention.
const RESTART_BLOCK_CADENCE: Duration = Duration::from_secs(2);
const STATUS_TIMEOUT: Duration = Duration::from_secs(90);
const ACCOUNT_VISIBILITY_TIMEOUT: Duration = Duration::from_secs(90);
const POLL_INTERVAL: Duration = Duration::from_millis(200);
const FAST_STATUS_POLL_INTERVAL: Duration = Duration::from_millis(25);
const TAIRA_BLOCK_CADENCE: Duration = Duration::from_secs(1);
const TAIRA_RECOVERY_BOUND: Duration = Duration::from_secs(50);
const SIGNED_OBSERVER_COUNT: usize = 5;
const OBSERVER_SLOW_READ_CHUNK_BYTES: usize = 1_024;
const OBSERVER_SLOW_READ_DELAY: Duration = Duration::from_millis(2);
const OBSERVER_PRESSURE_PAYLOAD_BYTES: usize = 512 * 1024;

#[derive(Clone, Debug)]
struct V2StatusSnapshot {
    peer: String,
    protocol_version: u64,
    node_fingerprint: Value,
    build_fingerprint: Value,
    config_fingerprint: Value,
    height_context_id: HeightContextId,
    height: u64,
    view: u64,
    leader: u64,
    phase: Value,
    body_state: SumeragiV2BodyState,
    last_timeout_view: Option<u64>,
    last_timeout_certificate: Option<TimeoutCertificateRef>,
    locked_prepare_qc: Option<PrepareQcSnapshot>,
    highest_prepare_qc: Option<PrepareQcSnapshot>,
    last_committed_height: u64,
    height_context: SumeragiV2HeightContextStatus,
    last_commit_qc: Option<SumeragiV2CommitQcStatus>,
    prepare_quorums: Vec<SumeragiV2VoteQuorumStatus>,
    liveness: SumeragiV2LivenessStatus,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct PrepareQcSnapshot {
    reference: QuorumCertificateRef,
}

#[derive(Clone, Copy, Debug)]
struct LockedReproposalPrepareQcSplit<'a> {
    locked: &'a QuorumCertificateRef,
    reproposed: &'a QuorumCertificateRef,
}

#[derive(Clone, Copy, Debug)]
struct DistinctPrepareQcSplit<'a> {
    first: &'a QuorumCertificateRef,
    second: &'a QuorumCertificateRef,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct HeldVoteSelection {
    subject: Option<BlockSubject>,
    execution_commitment: Option<ExecutionCommitment>,
    senders: BTreeSet<PeerId>,
    signers: BTreeSet<ValidatorIndex>,
    envelope_digests: Vec<Hash>,
    sequences: Vec<u64>,
}

fn strict_dual_quorum(
    signer_count: u32,
    min_signers: u32,
    signed_power: u64,
    total_power: u64,
) -> bool {
    min_signers > 0
        && signer_count >= min_signers
        && total_power > 0
        && signed_power >= u64::from(signer_count)
        && signed_power <= total_power
        && u128::from(signed_power) * 3 > u128::from(total_power) * 2
}

fn is_minimal_exact_prepare_quorum(
    quorum: &SumeragiV2VoteQuorumStatus,
    height_context: &SumeragiV2HeightContextStatus,
    expected: &QuorumCertificateRef,
) -> bool {
    quorum.round == expected.round
        && quorum.proposal_round == expected.proposal_round
        && quorum.subject == expected.subject
        && quorum.execution_commitment == expected.execution_commitment
        && quorum.min_signers == height_context.quorum.min_signers
        && quorum.total_power == height_context.quorum.total_power
        && quorum.signer_count == quorum.min_signers
        && strict_dual_quorum(
            quorum.signer_count,
            quorum.min_signers,
            quorum.signed_power,
            quorum.total_power,
        )
}

fn validate_minimal_exact_prepare_quorum(
    snapshot: &V2StatusSnapshot,
    expected: &QuorumCertificateRef,
) -> Result<()> {
    let matching = snapshot
        .prepare_quorums
        .iter()
        .filter(|quorum| {
            quorum.round == expected.round
                && quorum.proposal_round == expected.proposal_round
                && quorum.subject == expected.subject
                && quorum.execution_commitment == expected.execution_commitment
        })
        .collect::<Vec<_>>();
    ensure!(
        matching.len() == 1,
        "validator {} must expose exactly one Prepare pool for {expected:?}, found {matching:?}",
        snapshot.peer
    );
    let quorum = matching[0];
    ensure!(
        is_minimal_exact_prepare_quorum(quorum, &snapshot.height_context, expected),
        "validator {} reached the PrepareQC reference without the exact minimal equal-vote quorum: pool={quorum:?}, context={:?}",
        snapshot.peer,
        snapshot.height_context,
    );
    Ok(())
}

fn validate_commit_qc_dual_quorum(
    snapshot: &V2StatusSnapshot,
    minimum_committed_height: u64,
) -> Result<()> {
    let certificate = snapshot.last_commit_qc.as_ref().ok_or_else(|| {
        eyre!(
            "validator {} has no durable CommitQC summary after committing height {minimum_committed_height}",
            snapshot.peer
        )
    })?;
    ensure!(
        snapshot.last_committed_height >= minimum_committed_height
            && certificate.certificate.round.height == snapshot.last_committed_height
            && certificate.certificate.proposal_round.context_id
                == certificate.certificate.round.context_id
            && certificate.certificate.proposal_round.height
                == certificate.certificate.round.height
            && certificate.certificate.proposal_round.view <= certificate.certificate.round.view
            && certificate.certificate.phase == GlobalPhase::Commit,
        "validator {} retained the wrong durable CommitQC: {certificate:?}",
        snapshot.peer,
    );
    ensure!(
        certificate.validator_count > 0
            && DualQuorum::count_threshold(certificate.validator_count)
                == Some(certificate.min_signers)
            && certificate.total_power >= u64::from(certificate.validator_count),
        "validator {} retained a CommitQC with a malformed self-contained quorum: {certificate:?}",
        snapshot.peer,
    );
    ensure!(
        certificate.signer_count <= certificate.validator_count
            && strict_dual_quorum(
                certificate.signer_count,
                certificate.min_signers,
                certificate.signed_power,
                certificate.total_power,
            ),
        "validator {} committed without both count and power quorum: {certificate:?}",
        snapshot.peer,
    );
    if certificate.certificate.round.context_id == snapshot.height_context_id {
        ensure!(
            certificate.validator_count == snapshot.height_context.validator_count
                && certificate.min_signers == snapshot.height_context.quorum.min_signers
                && certificate.total_power == snapshot.height_context.quorum.total_power,
            "validator {} CommitQC disagrees with its matching active frozen context: certificate={certificate:?}, context={:?}",
            snapshot.peer,
            snapshot.height_context,
        );
    }
    Ok(())
}

fn locked_commit_has_exact_progress_witness(
    liveness: &SumeragiV2LivenessStatus,
    locked: &QuorumCertificateRef,
    current_height: u64,
    current_view: u64,
    last_committed_height: u64,
    last_commit_qc: Option<&SumeragiV2CommitQcStatus>,
) -> bool {
    let exact_pool = liveness.commit_quorums.iter().any(|quorum| {
        quorum.round.context_id == locked.proposal_round.context_id
            && quorum.round.height == locked.proposal_round.height
            && quorum.round.view >= locked.proposal_round.view
            && quorum.proposal_round == locked.proposal_round
            && quorum.subject == locked.subject
            && quorum.execution_commitment == locked.execution_commitment
            && quorum.signer_count > 0
            && quorum.signed_power > 0
    });
    let exact_outbound = liveness.outbound_intents.iter().any(|intent| {
        matches!(
            intent.kind,
            SumeragiV2OutboundIntentKind::CommitVote | SumeragiV2OutboundIntentKind::CommitQc
        ) && intent.round.context_id == locked.proposal_round.context_id
            && intent.round.height == locked.proposal_round.height
            && intent.round.view >= locked.proposal_round.view
            && intent.proposal_round == Some(locked.proposal_round)
            && intent.subject == Some(locked.subject)
            && intent.execution_commitment == Some(locked.execution_commitment)
    });
    let exact_timeout = current_height == locked.proposal_round.height
        && current_view > locked.proposal_round.view
        && liveness.outbound_intents.iter().any(|intent| {
            intent.kind == SumeragiV2OutboundIntentKind::TimeoutVote
                && intent.round.context_id == locked.proposal_round.context_id
                && intent.round.height == current_height
                && intent.round.view == current_view
                && intent.proposal_round.is_none()
                && intent.subject.is_none()
                && intent.execution_commitment.is_none()
                && intent.stage != SumeragiV2OutboundIntentStage::PendingPersistence
        });
    let exact_decision = last_committed_height == locked.proposal_round.height
        && last_commit_qc.is_some_and(|certificate| {
            certificate.certificate.phase == GlobalPhase::Commit
                && certificate.certificate.round.context_id == locked.proposal_round.context_id
                && certificate.certificate.round.height == locked.proposal_round.height
                && certificate.certificate.round.view >= locked.proposal_round.view
                && certificate.certificate.proposal_round == locked.proposal_round
                && certificate.certificate.subject == locked.subject
                && certificate.certificate.execution_commitment == locked.execution_commitment
        });

    exact_pool || exact_outbound || exact_timeout || exact_decision
}

fn validate_locked_commit_progress_witness(
    snapshot: &V2StatusSnapshot,
    locked: &QuorumCertificateRef,
) -> Result<()> {
    ensure!(
        snapshot
            .locked_prepare_qc
            .as_ref()
            .is_some_and(|candidate| candidate.reference == *locked),
        "validator {} does not retain the expected durable PrepareQC lock: expected={locked:?}, actual={:?}",
        snapshot.peer,
        snapshot.locked_prepare_qc,
    );
    ensure!(
        locked_commit_has_exact_progress_witness(
            &snapshot.liveness,
            locked,
            snapshot.height,
            snapshot.view,
            snapshot.last_committed_height,
            snapshot.last_commit_qc.as_ref(),
        ),
        "validator {} orphaned its exact durable locked-round Commit path: lock={locked:?}, commit_pools={:?}, outbound={:?}, last_commit_qc={:?}",
        snapshot.peer,
        snapshot.liveness.commit_quorums,
        snapshot.liveness.outbound_intents,
        snapshot.last_commit_qc,
    );
    if let Some(blocker) = snapshot.liveness.blocker {
        ensure!(
            matches!(
                blocker,
                SumeragiV2LivenessBlocker::CommitQuorumMissing
                    | SumeragiV2LivenessBlocker::TimeoutCertificateMissing
                    | SumeragiV2LivenessBlocker::SchedulerStarvation
                    | SumeragiV2LivenessBlocker::SuccessorActivationPending
                    | SumeragiV2LivenessBlocker::LocalControlPending
            ),
            "validator {} misclassified an exact validated locked-Commit delay as {blocker:?}",
            snapshot.peer,
        );
    }
    Ok(())
}

fn validate_applied_successor_witness(
    snapshot: &V2StatusSnapshot,
    minimum_committed_height: u64,
) -> Result<()> {
    validate_commit_qc_dual_quorum(snapshot, minimum_committed_height)?;
    ensure!(
        snapshot.last_committed_height >= minimum_committed_height
            && snapshot.last_committed_height.checked_add(1) == Some(snapshot.height)
            && status_is_awaiting_proposal(&snapshot.phase)
            && snapshot.body_state == SumeragiV2BodyState::Missing,
        "validator {} has a durable decision but no exact applied successor-height witness: committed={}, active={}, phase={:?}, body={:?}",
        snapshot.peer,
        snapshot.last_committed_height,
        snapshot.height,
        snapshot.phase,
        snapshot.body_state,
    );
    Ok(())
}

fn classify_locked_reproposal_prepare_qc_split<'a>(
    qcs: &[&'a PrepareQcSnapshot],
    height: u64,
    first_view: u64,
    second_view: u64,
) -> Option<LockedReproposalPrepareQcSplit<'a>> {
    if qcs.len() != VALIDATOR_COUNT {
        return None;
    }

    // Receiver-local rules admit the second-view Prepare traffic to the first
    // two peers and retain it at the final two peers. Preserve that exact
    // partition shape instead of accepting an arbitrary count-only split.
    let reproposed = &qcs[..2];
    let locked = &qcs[2..];
    if !reproposed
        .iter()
        .all(|qc| qc.reference.round.height == height && qc.reference.round.view == second_view)
        || !locked
            .iter()
            .all(|qc| qc.reference.round.height == height && qc.reference.round.view == first_view)
    {
        return None;
    }

    let reproposed_reference = &reproposed[0].reference;
    let locked_reference = &locked[0].reference;
    if reproposed
        .iter()
        .any(|qc| qc.reference != *reproposed_reference)
        || locked.iter().any(|qc| qc.reference != *locked_reference)
        || reproposed_reference == locked_reference
        || reproposed_reference.subject != locked_reference.subject
        || reproposed_reference.round.context_id != locked_reference.round.context_id
        || reproposed_reference.execution_commitment != locked_reference.execution_commitment
    {
        return None;
    }

    Some(LockedReproposalPrepareQcSplit {
        locked: locked_reference,
        reproposed: reproposed_reference,
    })
}

fn classify_distinct_prepare_qc_split<'a>(
    qcs: &[&'a PrepareQcSnapshot],
    first_group: [usize; 2],
    second_group: [usize; 2],
    height: u64,
    first_view: u64,
    second_view: u64,
) -> Option<DistinctPrepareQcSplit<'a>> {
    if qcs.len() != VALIDATOR_COUNT {
        return None;
    }
    let groups = first_group
        .into_iter()
        .chain(second_group)
        .collect::<BTreeSet<_>>();
    if groups.len() != VALIDATOR_COUNT || groups.iter().any(|index| *index >= qcs.len()) {
        return None;
    }
    let first = qcs.get(first_group[0])?.reference;
    let second = qcs.get(second_group[0])?.reference;
    if first_group
        .iter()
        .any(|index| qcs.get(*index).is_none_or(|qc| qc.reference != first))
        || second_group
            .iter()
            .any(|index| qcs.get(*index).is_none_or(|qc| qc.reference != second))
        || first.round.height != height
        || first.round.view != first_view
        || second.round.height != height
        || second.round.view != second_view
        || first.round.context_id != second.round.context_id
        || first.subject == second.subject
        || first == second
    {
        return None;
    }
    Some(DistinctPrepareQcSplit {
        first: &qcs[first_group[0]].reference,
        second: &qcs[second_group[0]].reference,
    })
}

fn held_no_high_timeout_vote_selection(
    ack: &ConsensusMessageControlAck,
    height: u64,
    view: u64,
    allowed_signers: &BTreeMap<PeerId, ValidatorIndex>,
    required: usize,
) -> Option<HeldVoteSelection> {
    let mut senders = BTreeSet::new();
    let mut signers = BTreeSet::new();
    let mut envelope_digests = Vec::with_capacity(required);
    let mut sequences = Vec::with_capacity(required);
    for message in &ack.held {
        if message.height != Some(height)
            || message.view != Some(view)
            || message.kind != ConsensusMessageControlKind::TimeoutVote
            || message.sender != message.authenticated_via
            || message.block_hash.is_some()
            || message.subject.is_some()
            || message.execution_commitment.is_some()
            || !message.certificate_signers.is_empty()
            || !senders.insert(message.sender.clone())
        {
            continue;
        }
        let signer = message.signer?;
        if allowed_signers.get(&message.sender) != Some(&signer)
            || !signers.insert(signer)
            || envelope_digests.contains(&message.envelope_digest)
        {
            continue;
        }
        envelope_digests.push(message.envelope_digest);
        sequences.push(message.sequence);
        if sequences.len() == required {
            return Some(HeldVoteSelection {
                subject: None,
                execution_commitment: None,
                senders,
                signers,
                envelope_digests,
                sequences,
            });
        }
    }
    None
}

fn held_prepare_vote_subject(
    ack: &ConsensusMessageControlAck,
    height: u64,
    view: u64,
    allowed_signers: &BTreeMap<PeerId, ValidatorIndex>,
    rejected_subject: Option<&BlockSubject>,
    required: usize,
) -> Option<HeldVoteSelection> {
    let mut subjects = BTreeMap::<
        (BlockSubject, ExecutionCommitment),
        BTreeMap<ValidatorIndex, (&PeerId, u64, Hash)>,
    >::new();
    for message in &ack.held {
        if message.height != Some(height)
            || message.view != Some(view)
            || message.kind != ConsensusMessageControlKind::PrepareVote
            || message.sender != message.authenticated_via
            || !message.certificate_signers.is_empty()
        {
            continue;
        }
        let subject = message.subject?;
        let execution_commitment = message.execution_commitment?;
        let signer = message.signer?;
        if message.block_hash != Some(subject.block_hash)
            || rejected_subject == Some(&subject)
            || allowed_signers.get(&message.sender) != Some(&signer)
        {
            continue;
        }
        subjects
            .entry((subject, execution_commitment))
            .or_default()
            .entry(signer)
            .or_insert((&message.sender, message.sequence, message.envelope_digest));
    }
    subjects
        .into_iter()
        .find_map(|((subject, execution_commitment), votes)| {
            if votes.len() < required {
                return None;
            }
            let selected = votes.into_iter().take(required).collect::<Vec<_>>();
            let senders = selected
                .iter()
                .map(|(_, (sender, _, _))| (*sender).clone())
                .collect::<BTreeSet<_>>();
            let signers = selected
                .iter()
                .map(|(signer, _)| *signer)
                .collect::<BTreeSet<_>>();
            let envelope_digests = selected
                .iter()
                .map(|(_, (_, _, digest))| *digest)
                .collect::<Vec<_>>();
            if senders.len() != required
                || signers.len() != required
                || envelope_digests
                    .iter()
                    .copied()
                    .collect::<BTreeSet<_>>()
                    .len()
                    != required
            {
                return None;
            }
            let mut sequences = selected
                .into_iter()
                .map(|(_, (_, sequence, _))| sequence)
                .collect::<Vec<_>>();
            sequences.sort_unstable();
            Some(HeldVoteSelection {
                subject: Some(subject),
                execution_commitment: Some(execution_commitment),
                senders,
                signers,
                envelope_digests,
                sequences,
            })
        })
}

fn held_prepare_certificate_sequences(
    ack: &ConsensusMessageControlAck,
    height: u64,
    view: u64,
    allowed_senders: &BTreeSet<PeerId>,
    expected_subject: &BlockSubject,
    expected_execution_commitment: &ExecutionCommitment,
    expected_signers: &[ValidatorIndex],
    required: usize,
) -> Option<Vec<u64>> {
    let mut sources_by_digest = BTreeMap::<Hash, BTreeMap<PeerId, u64>>::new();
    for message in &ack.held {
        if message.height != Some(height)
            || message.view != Some(view)
            || message.kind != ConsensusMessageControlKind::PrepareCertificate
            || message.sender != message.authenticated_via
            || !allowed_senders.contains(&message.sender)
            || message.subject.as_ref() != Some(expected_subject)
            || message.execution_commitment.as_ref() != Some(expected_execution_commitment)
            || message.block_hash.as_ref() != Some(&expected_subject.block_hash)
            || message.signer.is_some()
            || message.certificate_signers != expected_signers
        {
            continue;
        }
        sources_by_digest
            .entry(message.envelope_digest)
            .or_default()
            .entry(message.sender.clone())
            .or_insert(message.sequence);
    }
    sources_by_digest.into_values().find_map(|sources| {
        if sources.len() < required {
            return None;
        }
        let mut sequences = sources.into_values().take(required).collect::<Vec<_>>();
        if sequences.len() == required {
            sequences.sort_unstable();
            Some(sequences)
        } else {
            None
        }
    })
}

fn held_timeout_certificate_sequences(
    ack: &ConsensusMessageControlAck,
    height: u64,
    view: u64,
    allowed_senders: &BTreeSet<PeerId>,
    expected_subject: Option<&BlockSubject>,
    expected_execution_commitment: Option<&ExecutionCommitment>,
    expected_signers: &[ValidatorIndex],
    required: usize,
) -> Option<Vec<u64>> {
    let expected_block_hash = expected_subject.map(|subject| subject.block_hash);
    let mut sources_by_digest = BTreeMap::<Hash, BTreeMap<PeerId, u64>>::new();
    for message in &ack.held {
        if message.height != Some(height)
            || message.view != Some(view)
            || message.kind != ConsensusMessageControlKind::TimeoutCertificate
            || message.sender != message.authenticated_via
            || !allowed_senders.contains(&message.sender)
            || message.subject.as_ref() != expected_subject
            || message.execution_commitment.as_ref() != expected_execution_commitment
            || message.block_hash != expected_block_hash
            || message.signer.is_some()
            || message.certificate_signers != expected_signers
        {
            continue;
        }
        sources_by_digest
            .entry(message.envelope_digest)
            .or_default()
            .entry(message.sender.clone())
            .or_insert(message.sequence);
    }
    sources_by_digest.into_values().find_map(|sources| {
        if sources.len() < required {
            return None;
        }
        let mut sequences = sources.into_values().take(required).collect::<Vec<_>>();
        if sequences.len() == required {
            sequences.sort_unstable();
            Some(sequences)
        } else {
            None
        }
    })
}

fn held_quorum_evidence_sequences(
    ack: &ConsensusMessageControlAck,
    height: u64,
    view: u64,
    block_hash: &HashOf<BlockHeader>,
    vote_kind: ConsensusMessageControlKind,
    certificate_kind: ConsensusMessageControlKind,
) -> Option<Vec<u64>> {
    let matching = ack
        .held
        .iter()
        .filter(|message| {
            message.height == Some(height)
                && message.view == Some(view)
                && message.block_hash.as_ref() == Some(block_hash)
                && matches!(message.kind, kind if kind == vote_kind || kind == certificate_kind)
        })
        .collect::<Vec<_>>();
    let has_certificate = matching
        .iter()
        .any(|message| message.kind == certificate_kind);
    let vote_senders = matching
        .iter()
        .filter(|message| message.kind == vote_kind)
        .map(|message| &message.sender)
        .collect::<BTreeSet<_>>();
    if !has_certificate && vote_senders.len() < LOCKED_REPROPOSAL_REMOTE_QUORUM_VOTES {
        return None;
    }

    let mut sequences = matching
        .into_iter()
        .map(|message| message.sequence)
        .collect::<Vec<_>>();
    sequences.sort_unstable();
    (!sequences.is_empty()).then_some(sequences)
}

fn locked_reproposal_receiver_rules(
    receiver_index: usize,
    peer_ids: &[PeerId],
) -> Vec<ConsensusMessageControlRule> {
    let mut rules = Vec::new();
    for (sender_index, sender) in peer_ids.iter().enumerate() {
        if sender_index == receiver_index {
            continue;
        }
        // A Commit-certificate response can install the same decision evidence
        // as an unsolicited certificate, so keep it behind the same causal gate.
        for kind in [
            ConsensusMessageControlKind::CommitVote,
            ConsensusMessageControlKind::CommitCertificate,
            ConsensusMessageControlKind::CommitCertificateResponse,
        ] {
            rules.push(ConsensusMessageControlRule::exact(
                sender.clone(),
                kind,
                LOCKED_REPROPOSAL_HEIGHT,
                LOCKED_REPROPOSAL_FIRST_VIEW,
                ConsensusMessageControlAction::Drop,
            ));
            rules.push(ConsensusMessageControlRule::exact(
                sender.clone(),
                kind,
                LOCKED_REPROPOSAL_HEIGHT,
                LOCKED_REPROPOSAL_SECOND_VIEW,
                ConsensusMessageControlAction::Hold,
            ));
        }
        rules.push(ConsensusMessageControlRule::exact(
            sender.clone(),
            ConsensusMessageControlKind::TimeoutVote,
            LOCKED_REPROPOSAL_HEIGHT,
            LOCKED_REPROPOSAL_SECOND_VIEW,
            ConsensusMessageControlAction::Hold,
        ));
        rules.push(ConsensusMessageControlRule::exact(
            sender.clone(),
            ConsensusMessageControlKind::TimeoutCertificate,
            LOCKED_REPROPOSAL_HEIGHT,
            LOCKED_REPROPOSAL_SECOND_VIEW,
            ConsensusMessageControlAction::Hold,
        ));
        if receiver_index >= 2 {
            rules.push(ConsensusMessageControlRule::exact(
                sender.clone(),
                ConsensusMessageControlKind::PrepareVote,
                LOCKED_REPROPOSAL_HEIGHT,
                LOCKED_REPROPOSAL_SECOND_VIEW,
                ConsensusMessageControlAction::Hold,
            ));
            rules.push(ConsensusMessageControlRule::exact(
                sender.clone(),
                ConsensusMessageControlKind::PrepareCertificate,
                LOCKED_REPROPOSAL_HEIGHT,
                LOCKED_REPROPOSAL_SECOND_VIEW,
                ConsensusMessageControlAction::Hold,
            ));
        }
    }
    rules
}

fn observer_pressure_view_change_rules(
    receiver_index: usize,
    peer_ids: &[PeerId],
) -> Vec<ConsensusMessageControlRule> {
    peer_ids
        .iter()
        .enumerate()
        .filter(|(sender_index, _)| *sender_index != receiver_index)
        .flat_map(|(_, sender)| {
            [
                ConsensusMessageControlKind::CommitVote,
                ConsensusMessageControlKind::CommitCertificate,
                ConsensusMessageControlKind::CommitCertificateResponse,
            ]
            .map(|kind| {
                ConsensusMessageControlRule::exact(
                    sender.clone(),
                    kind,
                    LOCKED_REPROPOSAL_HEIGHT,
                    LOCKED_REPROPOSAL_FIRST_VIEW,
                    ConsensusMessageControlAction::Drop,
                )
            })
        })
        .collect()
}

fn distinct_prepare_qc_receiver_rules(
    receiver_index: usize,
    peer_ids: &[PeerId],
) -> Vec<ConsensusMessageControlRule> {
    let mut rules = Vec::new();
    for (sender_index, sender) in peer_ids.iter().enumerate() {
        if sender_index == receiver_index {
            continue;
        }
        for view in [LOCKED_REPROPOSAL_FIRST_VIEW, LOCKED_REPROPOSAL_SECOND_VIEW] {
            for kind in [
                ConsensusMessageControlKind::PrepareVote,
                ConsensusMessageControlKind::PrepareCertificate,
                ConsensusMessageControlKind::TimeoutVote,
                ConsensusMessageControlKind::TimeoutCertificate,
            ] {
                rules.push(ConsensusMessageControlRule::exact(
                    sender.clone(),
                    kind,
                    LOCKED_REPROPOSAL_HEIGHT,
                    view,
                    ConsensusMessageControlAction::Hold,
                ));
            }
            for kind in [
                ConsensusMessageControlKind::CommitVote,
                ConsensusMessageControlKind::CommitCertificate,
                ConsensusMessageControlKind::CommitCertificateResponse,
            ] {
                rules.push(ConsensusMessageControlRule::exact(
                    sender.clone(),
                    kind,
                    LOCKED_REPROPOSAL_HEIGHT,
                    view,
                    if view == LOCKED_REPROPOSAL_FIRST_VIEW {
                        ConsensusMessageControlAction::Drop
                    } else {
                        ConsensusMessageControlAction::Hold
                    },
                ));
            }
        }
    }
    rules
}

#[cfg(test)]
mod prepare_qc_split_tests {
    use super::*;
    use iroha::{
        crypto::{Hash, HashOf},
        data_model::block::{
            BlockHeader,
            consensus_v2::{
                BlockSubject, ConsensusRound, DualQuorum, ExecutionCommitment, HeightContext,
                HeightContextId, SumeragiV2OutboundIntentStage, SumeragiV2OutboundIntentStatus,
            },
        },
    };
    use iroha_test_network::ConsensusMessageControlHeld;

    const HEIGHT: u64 = 3;
    const FIRST_VIEW: u64 = 0;
    const SECOND_VIEW: u64 = 1;

    fn hash(seed: u8) -> Hash {
        Hash::new([seed])
    }

    fn hash_of<T>(seed: u8) -> HashOf<T> {
        HashOf::from_untyped_unchecked(hash(seed))
    }

    fn snapshot(view: u64, subject_seed: u8, execution_seed: u8) -> PrepareQcSnapshot {
        PrepareQcSnapshot {
            reference: QuorumCertificateRef {
                round: ConsensusRound {
                    context_id: HeightContextId(hash_of::<HeightContext>(0x10)),
                    height: HEIGHT,
                    view,
                },
                proposal_round: ConsensusRound {
                    context_id: HeightContextId(hash_of::<HeightContext>(0x10)),
                    height: HEIGHT,
                    view,
                },
                phase: GlobalPhase::Prepare,
                subject: BlockSubject {
                    parent_block_hash: Some(hash_of::<BlockHeader>(0x20)),
                    block_hash: hash_of::<BlockHeader>(subject_seed),
                    payload_hash: hash(subject_seed.wrapping_add(1)),
                },
                execution_commitment: ExecutionCommitment::without_topups_or_merge_carrier(
                    hash(0x30),
                    hash(0x31),
                    hash(0x32),
                    1,
                    hash(execution_seed),
                ),
            },
        }
    }

    fn classify(snapshots: &[PrepareQcSnapshot]) -> Option<LockedReproposalPrepareQcSplit<'_>> {
        let qcs = snapshots.iter().collect::<Vec<_>>();
        classify_locked_reproposal_prepare_qc_split(&qcs, HEIGHT, FIRST_VIEW, SECOND_VIEW)
    }

    fn height_context_status() -> SumeragiV2HeightContextStatus {
        SumeragiV2HeightContextStatus {
            epoch: 0,
            epoch_end_height: u64::MAX,
            mode: ConsensusMode::Npos,
            epoch_seed: [0x11; 32],
            validator_count: VALIDATOR_COUNT as u32,
            quorum: DualQuorum {
                min_signers: 3,
                total_power: 4,
            },
        }
    }

    fn quorum(reference: QuorumCertificateRef) -> SumeragiV2VoteQuorumStatus {
        SumeragiV2VoteQuorumStatus {
            round: reference.round,
            proposal_round: reference.proposal_round,
            subject: reference.subject,
            execution_commitment: reference.execution_commitment,
            signer_count: 3,
            signed_power: 3,
            min_signers: 3,
            total_power: 4,
        }
    }

    fn peer_ids() -> Vec<PeerId> {
        (0..VALIDATOR_COUNT)
            .map(|index| {
                let key = KeyPair::try_from_seed(
                    vec![u8::try_from(index + 1).expect("small peer index"); 32],
                    Algorithm::Ed25519,
                )
                .expect("deterministic peer key");
                PeerId::new(key.public_key().clone())
            })
            .collect()
    }

    fn ack(held: Vec<ConsensusMessageControlHeld>) -> ConsensusMessageControlAck {
        ConsensusMessageControlAck {
            revision: 1,
            command_digest: hash(0x60),
            rules: Vec::new(),
            queue_capacity: DISTINCT_PREPARE_QC_QUEUE_CAPACITY,
            held_bytes: held.iter().map(|message| message.size_bytes).sum(),
            held,
            release_pending: Vec::new(),
            in_flight: None,
            in_flight_bytes: 0,
            delivered: Vec::new(),
            retired: Vec::new(),
            dropped: 0,
            overflowed: 0,
            rejected_commands: 0,
            last_error: None,
            fatal: false,
            draining: false,
            drain_fence: None,
        }
    }

    fn held_prepare_vote(
        sequence: u64,
        sender: PeerId,
        signer: ValidatorIndex,
        reference: QuorumCertificateRef,
    ) -> ConsensusMessageControlHeld {
        ConsensusMessageControlHeld {
            sequence,
            authenticated_via: sender.clone(),
            sender,
            kind: ConsensusMessageControlKind::PrepareVote,
            height: Some(reference.round.height),
            view: Some(reference.round.view),
            block_hash: Some(reference.subject.block_hash),
            subject: Some(reference.subject),
            execution_commitment: Some(reference.execution_commitment),
            signer: Some(signer),
            cited_responder: None,
            certificate_signers: Vec::new(),
            envelope_digest: hash(u8::try_from(sequence).expect("small test sequence")),
            size_bytes: 64,
        }
    }

    fn held_timeout_vote(
        sequence: u64,
        sender: PeerId,
        signer: ValidatorIndex,
    ) -> ConsensusMessageControlHeld {
        ConsensusMessageControlHeld {
            sequence,
            authenticated_via: sender.clone(),
            sender,
            kind: ConsensusMessageControlKind::TimeoutVote,
            height: Some(HEIGHT),
            view: Some(FIRST_VIEW),
            block_hash: None,
            subject: None,
            execution_commitment: None,
            signer: Some(signer),
            cited_responder: None,
            certificate_signers: Vec::new(),
            envelope_digest: hash(u8::try_from(sequence).expect("small test sequence")),
            size_bytes: 64,
        }
    }

    fn held_prepare_certificate(
        sequence: u64,
        sender: PeerId,
        reference: QuorumCertificateRef,
        certificate_signers: Vec<ValidatorIndex>,
    ) -> ConsensusMessageControlHeld {
        ConsensusMessageControlHeld {
            sequence,
            authenticated_via: sender.clone(),
            sender,
            kind: ConsensusMessageControlKind::PrepareCertificate,
            height: Some(reference.round.height),
            view: Some(reference.round.view),
            block_hash: Some(reference.subject.block_hash),
            subject: Some(reference.subject),
            execution_commitment: Some(reference.execution_commitment),
            signer: None,
            cited_responder: None,
            certificate_signers,
            envelope_digest: hash(u8::try_from(sequence).expect("small test sequence")),
            size_bytes: 96,
        }
    }

    fn held_timeout_certificate(
        sequence: u64,
        sender: PeerId,
        reference: Option<QuorumCertificateRef>,
        certificate_signers: Vec<ValidatorIndex>,
    ) -> ConsensusMessageControlHeld {
        ConsensusMessageControlHeld {
            sequence,
            authenticated_via: sender.clone(),
            sender,
            kind: ConsensusMessageControlKind::TimeoutCertificate,
            height: Some(HEIGHT),
            view: Some(FIRST_VIEW),
            block_hash: reference.map(|reference| reference.subject.block_hash),
            subject: reference.map(|reference| reference.subject),
            execution_commitment: reference.map(|reference| reference.execution_commitment),
            signer: None,
            cited_responder: None,
            certificate_signers,
            envelope_digest: hash(u8::try_from(sequence).expect("small test sequence")),
            size_bytes: 96,
        }
    }

    #[test]
    fn classifies_prepare_qc_partitions_and_held_evidence() {
        let reproposed = snapshot(SECOND_VIEW, 0x40, 0x50);
        let locked = snapshot(FIRST_VIEW, 0x40, 0x50);
        let snapshots = [
            reproposed.clone(),
            reproposed.clone(),
            locked.clone(),
            locked.clone(),
        ];

        let split = classify(&snapshots).expect("valid locked-body split");
        assert_eq!(*split.locked, locked.reference);
        assert_eq!(*split.reproposed, reproposed.reference);
        assert_ne!(split.locked, split.reproposed);
        assert_eq!(split.locked.subject, split.reproposed.subject);

        let first = snapshot(FIRST_VIEW, 0x40, 0x50);
        let second = snapshot(SECOND_VIEW, 0x41, 0x51);
        let snapshots = [first.clone(), first.clone(), second.clone(), second.clone()];
        let qcs = snapshots.iter().collect::<Vec<_>>();
        let split = classify_distinct_prepare_qc_split(
            &qcs,
            [0, 1],
            [2, 3],
            HEIGHT,
            FIRST_VIEW,
            SECOND_VIEW,
        )
        .expect("valid distinct-subject split");
        assert_eq!(*split.first, first.reference);
        assert_eq!(*split.second, second.reference);
        assert_ne!(split.first.subject, split.second.subject);
        assert!(
            classify_distinct_prepare_qc_split(
                &qcs,
                [0, 1],
                [1, 2],
                HEIGHT,
                FIRST_VIEW,
                SECOND_VIEW,
            )
            .is_none()
        );
        let same_subject = [
            first.clone(),
            first,
            snapshot(SECOND_VIEW, 0x40, 0x50),
            snapshot(SECOND_VIEW, 0x40, 0x50),
        ];
        let same_subject = same_subject.iter().collect::<Vec<_>>();
        assert!(
            classify_distinct_prepare_qc_split(
                &same_subject,
                [0, 1],
                [2, 3],
                HEIGHT,
                FIRST_VIEW,
                SECOND_VIEW,
            )
            .is_none()
        );

        let peer_ids = peer_ids();
        let first_vote = snapshot(FIRST_VIEW, 0x70, 0x72).reference;
        let second_vote = snapshot(FIRST_VIEW, 0x71, 0x73).reference;
        let held = vec![
            held_prepare_vote(1, peer_ids[0].clone(), 0, first_vote),
            held_prepare_vote(2, peer_ids[0].clone(), 0, first_vote),
            held_prepare_vote(3, peer_ids[1].clone(), 1, first_vote),
            held_prepare_vote(4, peer_ids[2].clone(), 2, second_vote),
        ];
        let prepare_ack = ack(held);
        let allowed = peer_ids
            .iter()
            .cloned()
            .enumerate()
            .map(|(index, peer)| (peer, ValidatorIndex::try_from(index).expect("small roster")))
            .collect::<BTreeMap<_, _>>();
        assert_eq!(
            held_prepare_vote_subject(&prepare_ack, HEIGHT, FIRST_VIEW, &allowed, None, 2)
                .map(|selection| selection.sequences),
            Some(vec![1, 3]),
        );
        assert_eq!(
            held_prepare_vote_subject(
                &prepare_ack,
                HEIGHT,
                FIRST_VIEW,
                &allowed,
                Some(&second_vote.subject),
                2,
            )
            .map(|selection| selection.sequences),
            Some(vec![1, 3]),
        );
        assert!(
            held_prepare_vote_subject(
                &prepare_ack,
                HEIGHT,
                FIRST_VIEW,
                &allowed,
                Some(&first_vote.subject),
                2,
            )
            .is_none()
        );

        let mut duplicate_digest = held_prepare_vote(12, peer_ids[1].clone(), 1, first_vote);
        duplicate_digest.envelope_digest = hash(11);
        assert!(
            held_prepare_vote_subject(
                &ack(vec![
                    held_prepare_vote(11, peer_ids[0].clone(), 0, first_vote),
                    duplicate_digest,
                ]),
                HEIGHT,
                FIRST_VIEW,
                &allowed,
                None,
                2,
            )
            .is_none(),
            "a forged duplicate envelope digest cannot count as two signed votes"
        );

        let timeout_ack = ack(vec![
            held_timeout_vote(5, peer_ids[0].clone(), 0),
            held_timeout_vote(6, peer_ids[1].clone(), 1),
        ]);
        let timeout_allowed = allowed
            .iter()
            .filter(|(_, signer)| **signer < 2)
            .map(|(peer, signer)| (peer.clone(), *signer))
            .collect::<BTreeMap<_, _>>();
        let timeout = held_no_high_timeout_vote_selection(
            &timeout_ack,
            HEIGHT,
            FIRST_VIEW,
            &timeout_allowed,
            2,
        )
        .expect("two exact no-high timeout votes");
        assert_eq!(timeout.signers, BTreeSet::from([0, 1]));
        assert_eq!(timeout.sequences, vec![5, 6]);

        let certificate_signers = vec![0, 1, 2];
        let certificate_digest = hash(0xA0);
        let mut first_certificate = held_prepare_certificate(
            7,
            peer_ids[0].clone(),
            first_vote,
            certificate_signers.clone(),
        );
        first_certificate.envelope_digest = certificate_digest;
        let mut second_certificate = held_prepare_certificate(
            10,
            peer_ids[1].clone(),
            first_vote,
            certificate_signers.clone(),
        );
        second_certificate.envelope_digest = certificate_digest;
        let conflicting_certificate = held_prepare_certificate(
            11,
            peer_ids[2].clone(),
            first_vote,
            certificate_signers.clone(),
        );
        let certificate_ack = ack(vec![
            first_certificate,
            second_certificate,
            conflicting_certificate,
        ]);
        assert_eq!(
            held_prepare_certificate_sequences(
                &certificate_ack,
                HEIGHT,
                FIRST_VIEW,
                &BTreeSet::from([
                    peer_ids[0].clone(),
                    peer_ids[1].clone(),
                    peer_ids[2].clone(),
                ]),
                &first_vote.subject,
                &first_vote.execution_commitment,
                &certificate_signers,
                2,
            ),
            Some(vec![7, 10]),
            "one canonical PrepareQC rebroadcast by two authenticated sources is two retained source attempts"
        );
        assert!(
            held_prepare_certificate_sequences(
                &certificate_ack,
                HEIGHT,
                FIRST_VIEW,
                &BTreeSet::from([
                    peer_ids[0].clone(),
                    peer_ids[1].clone(),
                    peer_ids[2].clone(),
                ]),
                &first_vote.subject,
                &first_vote.execution_commitment,
                &certificate_signers,
                3,
            )
            .is_none(),
            "different certificate digests cannot be combined into one source set"
        );

        let timeout_certificate_digest = hash(0xA1);
        let mut first_timeout_certificate =
            held_timeout_certificate(8, peer_ids[0].clone(), None, certificate_signers.clone());
        first_timeout_certificate.envelope_digest = timeout_certificate_digest;
        let mut second_timeout_certificate =
            held_timeout_certificate(12, peer_ids[1].clone(), None, certificate_signers.clone());
        second_timeout_certificate.envelope_digest = timeout_certificate_digest;
        let mut third_timeout_certificate =
            held_timeout_certificate(13, peer_ids[2].clone(), None, certificate_signers.clone());
        third_timeout_certificate.envelope_digest = timeout_certificate_digest;
        let mut timeout_certificate_retry =
            held_timeout_certificate(15, peer_ids[0].clone(), None, certificate_signers.clone());
        timeout_certificate_retry.envelope_digest = timeout_certificate_digest;
        assert!(
            held_timeout_certificate_sequences(
                &ack(vec![
                    first_timeout_certificate.clone(),
                    timeout_certificate_retry,
                ]),
                HEIGHT,
                FIRST_VIEW,
                &BTreeSet::from([peer_ids[0].clone()]),
                None,
                None,
                &certificate_signers,
                2,
            )
            .is_none(),
            "one source's repeated certificate cannot pad the source quorum"
        );
        let locked_timeout_certificate = held_timeout_certificate(
            9,
            peer_ids[1].clone(),
            Some(first_vote),
            certificate_signers.clone(),
        );
        let timeout_certificate_ack = ack(vec![
            first_timeout_certificate,
            locked_timeout_certificate,
            second_timeout_certificate,
            third_timeout_certificate,
            held_timeout_certificate(14, peer_ids[3].clone(), None, certificate_signers.clone()),
        ]);
        assert_eq!(
            held_timeout_certificate_sequences(
                &timeout_certificate_ack,
                HEIGHT,
                FIRST_VIEW,
                &BTreeSet::from([
                    peer_ids[0].clone(),
                    peer_ids[1].clone(),
                    peer_ids[2].clone(),
                    peer_ids[3].clone(),
                ]),
                None,
                None,
                &certificate_signers,
                3,
            ),
            Some(vec![8, 12, 13]),
            "one canonical TimeoutCertificate rebroadcast by three authenticated sources is three retained source attempts"
        );
        assert!(
            held_timeout_certificate_sequences(
                &timeout_certificate_ack,
                HEIGHT,
                FIRST_VIEW,
                &BTreeSet::from([
                    peer_ids[0].clone(),
                    peer_ids[1].clone(),
                    peer_ids[2].clone(),
                    peer_ids[3].clone(),
                ]),
                None,
                None,
                &certificate_signers,
                4,
            )
            .is_none(),
            "different TimeoutCertificate digests cannot be combined into one source set"
        );
        assert_eq!(
            held_timeout_certificate_sequences(
                &timeout_certificate_ack,
                HEIGHT,
                FIRST_VIEW,
                &BTreeSet::from([peer_ids[1].clone()]),
                Some(&first_vote.subject),
                Some(&first_vote.execution_commitment),
                &certificate_signers,
                1,
            ),
            Some(vec![9]),
        );

        let mut relayed = held_timeout_vote(10, peer_ids[0].clone(), 0);
        relayed.authenticated_via = peer_ids[2].clone();
        assert!(
            held_no_high_timeout_vote_selection(
                &ack(vec![relayed, held_timeout_vote(11, peer_ids[1].clone(), 1)]),
                HEIGHT,
                FIRST_VIEW,
                &timeout_allowed,
                2,
            )
            .is_none()
        );
    }

    #[test]
    fn exact_prepare_qc_requires_both_count_and_power_quorum() {
        assert!(strict_dual_quorum(3, 3, 3, 4));
        // Three distinct signers satisfy the count threshold but their power
        // does not strictly exceed two thirds of a weighted roster.
        assert!(!strict_dual_quorum(3, 3, 3, 10));
        // Two high-power signers exceed the power threshold but cannot replace
        // the independently required third identity.
        assert!(!strict_dual_quorum(2, 3, 8, 10));
        assert!(!strict_dual_quorum(2, 3, 4, 4));
        assert!(!strict_dual_quorum(3, 3, 2, 3));
        assert!(!strict_dual_quorum(3, 3, 2, 2));
        assert!(!strict_dual_quorum(3, 3, 5, 4));
        assert!(!strict_dual_quorum(3, 3, 0, 0));

        let expected = snapshot(FIRST_VIEW, 0x40, 0x50).reference;
        let context = height_context_status();
        let valid = quorum(expected);
        assert!(is_minimal_exact_prepare_quorum(&valid, &context, &expected));

        let mut count_short = valid;
        count_short.signer_count = 2;
        assert!(!is_minimal_exact_prepare_quorum(
            &count_short,
            &context,
            &expected
        ));

        let mut power_short = valid;
        power_short.signed_power = 2;
        assert!(!is_minimal_exact_prepare_quorum(
            &power_short,
            &context,
            &expected
        ));

        let mut over_delivered = valid;
        over_delivered.signer_count = 4;
        over_delivered.signed_power = 4;
        assert!(!is_minimal_exact_prepare_quorum(
            &over_delivered,
            &context,
            &expected
        ));

        let mut wrong_subject = valid;
        wrong_subject.subject = snapshot(FIRST_VIEW, 0x41, 0x50).reference.subject;
        assert!(!is_minimal_exact_prepare_quorum(
            &wrong_subject,
            &context,
            &expected
        ));

        let mut wrong_total = valid;
        wrong_total.total_power = 3;
        assert!(!is_minimal_exact_prepare_quorum(
            &wrong_total,
            &context,
            &expected
        ));
    }

    #[test]
    fn locked_commit_progress_witness_rejects_inexact_or_empty_ownership() {
        let locked = snapshot(FIRST_VIEW, 0x40, 0x50).reference;
        let empty = SumeragiV2LivenessStatus::default();
        assert!(!locked_commit_has_exact_progress_witness(
            &empty,
            &locked,
            HEIGHT,
            SECOND_VIEW,
            HEIGHT - 1,
            None,
        ));

        let outbound = |kind, reference: QuorumCertificateRef| SumeragiV2OutboundIntentStatus {
            kind,
            round: reference.round,
            proposal_round: Some(reference.proposal_round),
            subject: Some(reference.subject),
            execution_commitment: Some(reference.execution_commitment),
            stage: SumeragiV2OutboundIntentStage::Sent,
        };
        let mut wrong_kind = empty.clone();
        wrong_kind
            .outbound_intents
            .push(outbound(SumeragiV2OutboundIntentKind::PrepareVote, locked));
        assert!(!locked_commit_has_exact_progress_witness(
            &wrong_kind,
            &locked,
            HEIGHT,
            SECOND_VIEW,
            HEIGHT - 1,
            None,
        ));

        let mut wrong_round = empty.clone();
        wrong_round.outbound_intents.push(outbound(
            SumeragiV2OutboundIntentKind::CommitVote,
            snapshot(SECOND_VIEW, 0x40, 0x50).reference,
        ));
        assert!(!locked_commit_has_exact_progress_witness(
            &wrong_round,
            &locked,
            HEIGHT,
            SECOND_VIEW,
            HEIGHT - 1,
            None,
        ));

        let mut wrong_origin = outbound(SumeragiV2OutboundIntentKind::CommitVote, locked);
        wrong_origin.proposal_round = Some(snapshot(SECOND_VIEW, 0x40, 0x50).reference.round);
        let mut wrong_origin_status = empty.clone();
        wrong_origin_status.outbound_intents.push(wrong_origin);
        assert!(!locked_commit_has_exact_progress_witness(
            &wrong_origin_status,
            &locked,
            HEIGHT,
            SECOND_VIEW,
            HEIGHT - 1,
            None,
        ));

        let mut wrong_subject = empty.clone();
        wrong_subject.outbound_intents.push(outbound(
            SumeragiV2OutboundIntentKind::CommitVote,
            snapshot(FIRST_VIEW, 0x41, 0x50).reference,
        ));
        assert!(!locked_commit_has_exact_progress_witness(
            &wrong_subject,
            &locked,
            HEIGHT,
            SECOND_VIEW,
            HEIGHT - 1,
            None,
        ));

        let mut empty_pool = empty.clone();
        let mut no_signers = quorum(locked);
        no_signers.signer_count = 0;
        no_signers.signed_power = 0;
        empty_pool.commit_quorums.push(no_signers);
        assert!(!locked_commit_has_exact_progress_witness(
            &empty_pool,
            &locked,
            HEIGHT,
            SECOND_VIEW,
            HEIGHT - 1,
            None,
        ));
    }

    #[test]
    fn locked_commit_progress_witness_accepts_each_exact_owner() {
        let locked = snapshot(FIRST_VIEW, 0x40, 0x50).reference;

        let mut outbound = SumeragiV2LivenessStatus::default();
        outbound
            .outbound_intents
            .push(SumeragiV2OutboundIntentStatus {
                kind: SumeragiV2OutboundIntentKind::CommitVote,
                round: locked.round,
                proposal_round: Some(locked.proposal_round),
                subject: Some(locked.subject),
                execution_commitment: Some(locked.execution_commitment),
                stage: SumeragiV2OutboundIntentStage::PendingSignature,
            });
        assert!(locked_commit_has_exact_progress_witness(
            &outbound,
            &locked,
            HEIGHT,
            SECOND_VIEW,
            HEIGHT - 1,
            None,
        ));

        let mut pooled = SumeragiV2LivenessStatus::default();
        let mut one_vote = quorum(locked);
        one_vote.signer_count = 1;
        one_vote.signed_power = 1;
        pooled.commit_quorums.push(one_vote);
        assert!(locked_commit_has_exact_progress_witness(
            &pooled,
            &locked,
            HEIGHT,
            SECOND_VIEW,
            HEIGHT - 1,
            None,
        ));

        let mut later_finality = outbound.clone();
        later_finality.outbound_intents[0].round.view = SECOND_VIEW;
        assert!(locked_commit_has_exact_progress_witness(
            &later_finality,
            &locked,
            HEIGHT,
            SECOND_VIEW,
            HEIGHT - 1,
            None,
        ));

        let exact_timeout = SumeragiV2OutboundIntentStatus {
            kind: SumeragiV2OutboundIntentKind::TimeoutVote,
            round: ConsensusRound {
                context_id: locked.round.context_id,
                height: HEIGHT,
                view: SECOND_VIEW,
            },
            proposal_round: None,
            subject: None,
            execution_commitment: None,
            stage: SumeragiV2OutboundIntentStage::Sent,
        };
        let mut timed_out = SumeragiV2LivenessStatus::default();
        timed_out.outbound_intents.push(exact_timeout.clone());
        assert!(locked_commit_has_exact_progress_witness(
            &timed_out,
            &locked,
            HEIGHT,
            SECOND_VIEW,
            HEIGHT - 1,
            None,
        ));
        timed_out.outbound_intents[0].stage = SumeragiV2OutboundIntentStage::PendingPersistence;
        assert!(!locked_commit_has_exact_progress_witness(
            &timed_out,
            &locked,
            HEIGHT,
            SECOND_VIEW,
            HEIGHT - 1,
            None,
        ));
        timed_out.outbound_intents[0] = exact_timeout;
        timed_out.outbound_intents[0].round.context_id =
            HeightContextId(hash_of::<HeightContext>(0x11));
        assert!(!locked_commit_has_exact_progress_witness(
            &timed_out,
            &locked,
            HEIGHT,
            SECOND_VIEW,
            HEIGHT - 1,
            None,
        ));

        let decision = SumeragiV2CommitQcStatus {
            certificate: QuorumCertificateRef {
                phase: GlobalPhase::Commit,
                ..locked
            },
            validator_count: VALIDATOR_COUNT as u32,
            signer_count: 3,
            min_signers: 3,
            signed_power: 3,
            total_power: 4,
        };
        assert!(locked_commit_has_exact_progress_witness(
            &SumeragiV2LivenessStatus::default(),
            &locked,
            HEIGHT,
            SECOND_VIEW,
            HEIGHT,
            Some(&decision),
        ));
        assert!(!locked_commit_has_exact_progress_witness(
            &SumeragiV2LivenessStatus::default(),
            &locked,
            HEIGHT,
            SECOND_VIEW,
            HEIGHT - 1,
            Some(&decision),
        ));
    }

    #[test]
    fn rejects_malformed_locked_body_reference_splits() {
        let reproposed = snapshot(SECOND_VIEW, 0x40, 0x50);
        let locked = snapshot(FIRST_VIEW, 0x40, 0x50);

        let three_by_one = [
            reproposed.clone(),
            reproposed.clone(),
            reproposed.clone(),
            locked.clone(),
        ];
        assert!(classify(&three_by_one).is_none());

        let reversed_groups = [
            locked.clone(),
            locked.clone(),
            reproposed.clone(),
            reproposed.clone(),
        ];
        assert!(classify(&reversed_groups).is_none());

        let within_group_disagreement = [
            reproposed.clone(),
            snapshot(SECOND_VIEW, 0x40, 0x51),
            locked.clone(),
            locked.clone(),
        ];
        assert!(classify(&within_group_disagreement).is_none());

        let identical_cross_group_references = [
            reproposed.clone(),
            reproposed.clone(),
            reproposed.clone(),
            reproposed.clone(),
        ];
        assert!(classify(&identical_cross_group_references).is_none());

        let different_subjects = [
            snapshot(SECOND_VIEW, 0x41, 0x50),
            snapshot(SECOND_VIEW, 0x41, 0x50),
            locked.clone(),
            locked.clone(),
        ];
        assert!(classify(&different_subjects).is_none());

        let different_execution_commitments = [
            snapshot(SECOND_VIEW, 0x40, 0x52),
            snapshot(SECOND_VIEW, 0x40, 0x52),
            locked.clone(),
            locked,
        ];
        assert!(classify(&different_execution_commitments).is_none());
    }

    #[test]
    fn initial_receiver_rules_encode_the_ordered_partition() {
        let peer_ids = peer_ids();
        for receiver_index in 0..VALIDATOR_COUNT {
            let rules = locked_reproposal_receiver_rules(receiver_index, &peer_ids);
            let expected_per_sender = if receiver_index < 2 { 8 } else { 10 };
            assert_eq!(rules.len(), (VALIDATOR_COUNT - 1) * expected_per_sender);
            assert!(rules.iter().all(|rule| {
                rule.sender != peer_ids[receiver_index]
                    && rule.height == LOCKED_REPROPOSAL_HEIGHT
                    && match rule.action {
                        ConsensusMessageControlAction::Drop => {
                            rule.view == LOCKED_REPROPOSAL_FIRST_VIEW
                                && matches!(
                                    rule.kind,
                                    ConsensusMessageControlKind::CommitVote
                                        | ConsensusMessageControlKind::CommitCertificate
                                        | ConsensusMessageControlKind::CommitCertificateResponse
                                )
                        }
                        ConsensusMessageControlAction::Hold => {
                            rule.view == LOCKED_REPROPOSAL_SECOND_VIEW
                        }
                    }
            }));
            assert_eq!(
                rules
                    .iter()
                    .filter(|rule| {
                        matches!(
                            rule.kind,
                            ConsensusMessageControlKind::PrepareVote
                                | ConsensusMessageControlKind::PrepareCertificate
                        )
                    })
                    .count(),
                if receiver_index < 2 {
                    0
                } else {
                    2 * (VALIDATOR_COUNT - 1)
                }
            );
        }
        for receiver_index in 0..VALIDATOR_COUNT {
            let rules = distinct_prepare_qc_receiver_rules(receiver_index, &peer_ids);
            assert_eq!(rules.len(), 14 * (VALIDATOR_COUNT - 1));
            assert!(rules.iter().all(|rule| {
                rule.sender != peer_ids[receiver_index]
                    && rule.height == LOCKED_REPROPOSAL_HEIGHT
                    && matches!(
                        rule.view,
                        LOCKED_REPROPOSAL_FIRST_VIEW | LOCKED_REPROPOSAL_SECOND_VIEW
                    )
                    && match rule.kind {
                        ConsensusMessageControlKind::CommitVote
                        | ConsensusMessageControlKind::CommitCertificate
                        | ConsensusMessageControlKind::CommitCertificateResponse => {
                            rule.action
                                == if rule.view == LOCKED_REPROPOSAL_FIRST_VIEW {
                                    ConsensusMessageControlAction::Drop
                                } else {
                                    ConsensusMessageControlAction::Hold
                                }
                        }
                        ConsensusMessageControlKind::PrepareVote
                        | ConsensusMessageControlKind::PrepareCertificate
                        | ConsensusMessageControlKind::TimeoutVote
                        | ConsensusMessageControlKind::TimeoutCertificate => {
                            rule.action == ConsensusMessageControlAction::Hold
                        }
                        _ => false,
                    }
            }));
        }
    }

    #[test]
    fn observer_pressure_rules_drop_only_remote_view_zero_commit_evidence() {
        let peer_ids = peer_ids();
        for receiver_index in 0..VALIDATOR_COUNT {
            let rules = observer_pressure_view_change_rules(receiver_index, &peer_ids);
            assert_eq!(rules.len(), 3 * (VALIDATOR_COUNT - 1));
            assert!(rules.iter().all(|rule| {
                rule.sender != peer_ids[receiver_index]
                    && rule.height == LOCKED_REPROPOSAL_HEIGHT
                    && rule.view == LOCKED_REPROPOSAL_FIRST_VIEW
                    && rule.action == ConsensusMessageControlAction::Drop
                    && matches!(
                        rule.kind,
                        ConsensusMessageControlKind::CommitVote
                            | ConsensusMessageControlKind::CommitCertificate
                            | ConsensusMessageControlKind::CommitCertificateResponse
                    )
            }));
        }
    }

    include!("sumeragi_v2_runner/restart_timing_test.rs");

    #[test]
    fn distinct_prepare_qc_view_zero_wait_covers_deadline_without_masking_view_one() {
        let cadence_ms = u64::try_from(DISTINCT_PREPARE_QC_BLOCK_CADENCE.as_millis())
            .expect("scenario cadence fits the canonical millisecond width");
        let (base_round_timeout_ms, _) =
            iroha_config::parameters::actual::sumeragi_v2_timing_ms(cadence_ms)
                .expect("scenario cadence derives valid v2 timing");
        let base_round_timeout = Duration::from_millis(base_round_timeout_ms);

        assert_eq!(base_round_timeout, Duration::from_secs(80));
        assert!(
            DISTINCT_PREPARE_QC_VIEW_ZERO_TIMEOUT
                >= base_round_timeout.saturating_add(Duration::from_secs(30)),
            "the staged view-zero timeout needs scheduling and control-publication margin"
        );
        assert!(
            DISTINCT_PREPARE_QC_VIEW_ZERO_TIMEOUT < base_round_timeout.saturating_mul(2),
            "the view-zero observation bound must stay below one full view-one deadline"
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn authoritative_v2_genesis_commits_on_every_validator() -> Result<()> {
    init_instruction_registry();
    let builder = NetworkBuilder::new()
        .with_peers(VALIDATOR_COUNT)
        .with_auto_populated_trusted_peers()
        .with_base_seed(stringify!(
            authoritative_v2_genesis_commits_on_every_validator
        ))
        .with_sync_timeout(Duration::from_secs(180))
        .with_peer_startup_timeout(Duration::from_secs(90));
    let context = stringify!(authoritative_v2_genesis_commits_on_every_validator);
    let network = sandbox::start_network_async_or_skip(builder, context).await?;
    let Some(network) = sandbox::enforce_network_start_requirement(network, context)? else {
        return Ok(());
    };

    let result = async {
        ensure!(
            network.peers().len() == VALIDATOR_COUNT
                && network.peers().iter().all(NetworkPeer::is_running),
            "fresh genesis must start exactly {VALIDATOR_COUNT} voting validators"
        );
        let peers = network.peers().to_vec();
        let normal = normal_statuses(&peers).await?;
        ensure!(
            normal.iter().all(|status| status.blocks >= 1),
            "fresh genesis must commit on every validator: {normal:?}"
        );
        let committed_floor = normal
            .iter()
            .map(|status| status.blocks)
            .min()
            .unwrap_or_default();
        let v2 =
            wait_for_common_awaiting_v2_round(&peers, committed_floor, STATUS_TIMEOUT).await?;
        validate_v2_status_set(&v2, VALIDATOR_COUNT)?;
        ensure!(
            v2.iter().all(|status| {
                status.last_committed_height >= 1
                    && status.last_committed_height.checked_add(1) == Some(status.height)
                    && status_is_awaiting_proposal(&status.phase)
            }),
            "durable genesis application must activate one common awaiting-proposal successor height: {v2:?}"
        );
        Ok(())
    }
    .await;
    network.shutdown_and_release().await;
    result
}

/// Four validators must retain the exact voting roster while five signed
/// observers recover through authenticated body sync after receiver-local
/// control forces a view change under bounded, byte-transparent slow-reader
/// transport delay.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "runs nine real peers with transparent slow-reader relays"]
async fn signed_observer_slow_reader_pressure_recovers_exact_successor() -> Result<()> {
    init_instruction_registry();

    let builder = NetworkBuilder::new()
        .with_peers(VALIDATOR_COUNT)
        .with_auto_populated_trusted_peers()
        .with_base_seed(stringify!(
            signed_observer_slow_reader_pressure_recovers_exact_successor
        ))
        .with_observer_p2p_bootstrap(ObserverP2pBootstrap::new(SIGNED_OBSERVER_COUNT)?)?
        .with_observer_slow_reader_relays(ObserverSlowReaderRelayConfig::new(
            OBSERVER_SLOW_READ_CHUNK_BYTES,
            OBSERVER_SLOW_READ_DELAY,
        )?)?
        .with_block_cadence(DISTINCT_PREPARE_QC_BLOCK_CADENCE)
        .with_initial_consensus_message_control_rules(
            LOCKED_REPROPOSAL_QUEUE_CAPACITY,
            observer_pressure_view_change_rules,
        )
        .with_sync_timeout(Duration::from_secs(240))
        .with_peer_startup_timeout(Duration::from_secs(180));

    let context = stringify!(signed_observer_slow_reader_pressure_recovers_exact_successor);
    let network = sandbox::start_network_async_or_skip(builder, context).await?;
    let Some(network) = sandbox::enforce_network_start_requirement(network, context)? else {
        return Ok(());
    };

    let result = async {
        let validators = network.validators().to_vec();
        let observers = network.observers().to_vec();
        let all_participants = network.all_peers().cloned().collect::<Vec<_>>();
        ensure!(
            validators.len() == VALIDATOR_COUNT
                && observers.len() == SIGNED_OBSERVER_COUNT
                && all_participants.len() == VALIDATOR_COUNT + SIGNED_OBSERVER_COUNT,
            "observer pressure scenario requires exactly four validators and five observers"
        );

        let validator_ids = validators
            .iter()
            .map(NetworkPeer::id)
            .collect::<BTreeSet<_>>();
        let validator_peer_ids = validators.iter().map(NetworkPeer::id).collect::<Vec<_>>();
        let expected_rules = validators
            .iter()
            .enumerate()
            .map(|(receiver_index, _)| {
                observer_pressure_view_change_rules(receiver_index, &validator_peer_ids)
            })
            .collect::<Vec<_>>();
        let observer_ids = observers
            .iter()
            .map(NetworkPeer::id)
            .collect::<BTreeSet<_>>();
        let topology_ids = network
            .topology_entries()
            .iter()
            .map(|entry| entry.peer.clone())
            .collect::<BTreeSet<_>>();
        ensure!(
            topology_ids == validator_ids
                && topology_ids.len() == VALIDATOR_COUNT
                && topology_ids.is_disjoint(&observer_ids),
            "signed observers entered the validator topology: validators={validator_ids:?}, observers={observer_ids:?}, topology={topology_ids:?}"
        );

        try_join_all(validators.iter().zip(&expected_rules).map(|(peer, expected)| async move {
            let ack = peer
                .consensus_message_control()
                .ok_or_else(|| eyre!("{} lacks receiver-local control", peer.mnemonic()))?
                .wait_until_ready(Duration::from_secs(20))
                .await?;
            ensure!(
                ack.revision == 1
                    && ack.rules.as_slice() == expected.as_slice()
                    && ack.queue_capacity == LOCKED_REPROPOSAL_QUEUE_CAPACITY
                    && !ack.fatal
                    && ack.overflowed == 0,
                "{} did not install its exact bounded view-zero control schedule: revision={}, queue_capacity={}, rules_match={}",
                peer.mnemonic(),
                ack.revision,
                ack.queue_capacity,
                ack.rules.as_slice() == expected.as_slice(),
            );
            Ok::<(), eyre::Report>(())
        }))
        .await?;

        let initial = wait_for_v2_status_condition(
            &validators,
            "one common open observer-pressure height-two view-zero round",
            STATUS_TIMEOUT,
            |snapshots| {
                snapshots.iter().all(|snapshot| {
                    snapshot.height == LOCKED_REPROPOSAL_HEIGHT
                        && snapshot.view == LOCKED_REPROPOSAL_FIRST_VIEW
                        && snapshot.last_committed_height < LOCKED_REPROPOSAL_HEIGHT
                        && snapshot.leader == snapshots[0].leader
                        && status_round_is_open(
                            snapshot,
                            LOCKED_REPROPOSAL_HEIGHT,
                            LOCKED_REPROPOSAL_FIRST_VIEW,
                        )
                })
            },
        )
        .await?;
        validate_v2_status_set(&initial, VALIDATOR_COUNT)?;
        validate_open_round(
            &initial,
            LOCKED_REPROPOSAL_HEIGHT,
            LOCKED_REPROPOSAL_FIRST_VIEW,
        )?;
        ensure!(
            network.set_observer_slow_reader_relays_paused(true),
            "observer slow-reader relays were unavailable at the exact open-round boundary"
        );

        let recovered_account = fixture_account(0xD5)?;
        assert_accounts_absent(&all_participants, &[recovered_account.clone()]).await?;
        let relay_stats_before = observers
            .iter()
            .map(|observer| {
                network
                    .observer_slow_reader_relay_stats_for(&observer.id())
                    .ok_or_else(|| {
                        eyre!(
                            "observer slow-reader relay stats were unavailable for {}",
                            observer.mnemonic()
                        )
                    })
            })
            .collect::<Result<Vec<_>>>()?;
        submit_pressure_account(
            network.client(),
            recovered_account.clone(),
            OBSERVER_PRESSURE_PAYLOAD_BYTES,
        )
        .await?;
        wait_for_validator_commit_before_observer_catchup(
            &validators,
            &observers,
            LOCKED_REPROPOSAL_HEIGHT,
            DISTINCT_PREPARE_QC_VIEW_ZERO_TIMEOUT,
        )
        .await?;
        ensure!(
            network.set_observer_slow_reader_relays_paused(false),
            "observer slow-reader relays disappeared before bounded recovery"
        );
        network
            .ensure_blocks_with(|height| height.total >= LOCKED_REPROPOSAL_HEIGHT)
            .await
            .wrap_err("observers did not recover the later-view body")?;
        wait_for_normal_statuses(
            &all_participants,
            LOCKED_REPROPOSAL_HEIGHT,
            STATUS_TIMEOUT,
        )
        .await?;
        wait_for_accounts_visible(
            &all_participants,
            &[recovered_account.clone()],
            ACCOUNT_VISIBILITY_TIMEOUT,
        )
        .await?;

        let committed_views =
            try_join_all(validators.iter().map(|peer| {
                committed_view_at_height(peer, LOCKED_REPROPOSAL_HEIGHT)
            }))
            .await?;
        ensure!(
            committed_views.iter().all(|view| *view > LOCKED_REPROPOSAL_FIRST_VIEW),
            "receiver-local control did not force a view change: {committed_views:?}"
        );
        let dropped = try_join_all(validators.iter().zip(&expected_rules).map(
            |(peer, expected)| async move {
                wait_for_control_selection(
                    peer,
                    "an exact rule-matched view-zero commit-evidence drop",
                    STATUS_TIMEOUT,
                    |ack| {
                        (ack.revision == 1
                            && ack.rules.as_slice() == expected.as_slice()
                            && ack.dropped > 0)
                            .then_some(ack.dropped)
                    },
                )
                .await
            },
        ))
        .await?;
        ensure!(
            dropped.iter().all(|count| *count > 0),
            "a validator changed view without proving its exact receiver-local drop rule fired: {dropped:?}"
        );

        let recovered = wait_for_common_awaiting_v2_round(
            &validators,
            LOCKED_REPROPOSAL_HEIGHT,
            STATUS_TIMEOUT,
        )
        .await?;
        validate_v2_status_set(&recovered, VALIDATOR_COUNT)?;
        for snapshot in &recovered {
            validate_applied_successor_witness(snapshot, LOCKED_REPROPOSAL_HEIGHT)?;
        }

        let proof = fetch_bridge_finality_proof(&validators[0], LOCKED_REPROPOSAL_HEIGHT).await?;
        verify_bridge_finality_proof(&proof, &network.chain_id())
            .wrap_err("recovered block finality proof failed cryptographic validation")?;
        let committed_hashes = try_join_all(all_participants.iter().map(|peer| {
            committed_hash_at_height(peer, LOCKED_REPROPOSAL_HEIGHT)
        }))
        .await?;
        let proof_hash = proof.block_header.hash().to_string();
        ensure!(
            committed_hashes.iter().all(|hash| hash == &proof_hash),
            "validators and signed observers did not converge on the exact finalized height-two block: proof={proof_hash}, committed={committed_hashes:?}"
        );
        let artifact = &proof.finality_artifact;
        let roster_ids = artifact
            .height_context
            .roster
            .iter()
            .map(|entry| entry.validator.clone())
            .collect::<BTreeSet<_>>();
        ensure!(
            roster_ids == validator_ids
                && roster_ids.len() == VALIDATOR_COUNT
                && roster_ids.is_disjoint(&observer_ids),
            "the frozen finality roster included an observer: {roster_ids:?}"
        );
        let signer_count = u32::try_from(artifact.commit_qc.signers.len())
            .wrap_err("CommitQC signer count does not fit u32")?;
        let signed_power = artifact.commit_qc.signers.iter().try_fold(
            0_u64,
            |power, signer| -> Result<u64> {
                let index = usize::try_from(*signer)
                    .wrap_err("CommitQC signer index does not fit usize")?;
                let member = artifact
                    .height_context
                    .roster
                    .get(index)
                    .ok_or_else(|| eyre!("CommitQC signer {signer} is outside the roster"))?;
                ensure!(
                    validator_ids.contains(&member.validator)
                        && !observer_ids.contains(&member.validator),
                    "CommitQC signer index {signer} resolved to an observer"
                );
                power
                    .checked_add(member.power)
                    .ok_or_else(|| eyre!("CommitQC signed power overflowed"))
            },
        )?;
        ensure!(
            artifact.height_context.roster.len() == VALIDATOR_COUNT
                && artifact.height_context.quorum.min_signers == 3
                && artifact.height_context.quorum.total_power == 4
                && strict_dual_quorum(
                    signer_count,
                    artifact.height_context.quorum.min_signers,
                    signed_power,
                    artifact.height_context.quorum.total_power,
                ),
            "recovered CommitQC lacked the exact four-validator equal-vote quorum: signers={:?}, signed_power={signed_power}, quorum={:?}",
            artifact.commit_qc.signers,
            artifact.height_context.quorum,
        );

        let observer_snapshots = wait_for_v2_status_condition(
            &observers,
            "observer convergence on the applied successor",
            STATUS_TIMEOUT,
            |snapshots| {
                snapshots.iter().all(|snapshot| {
                    snapshot.last_committed_height >= LOCKED_REPROPOSAL_HEIGHT
                        && snapshot.height_context.validator_count
                            == u32::try_from(VALIDATOR_COUNT).expect("validator count fits u32")
                })
            },
        )
        .await?;
        ensure!(
            observer_snapshots.iter().all(|snapshot| {
                snapshot
                    .last_commit_qc
                    .is_some_and(|qc| qc.validator_count == VALIDATOR_COUNT as u32)
            }),
            "an observer converged without retaining the validator-only CommitQC"
        );
        assert_account_registration_in_exact_block(
            &all_participants,
            LOCKED_REPROPOSAL_HEIGHT,
            &recovered_account,
        )
        .await?;
        for (observer, before) in observers.iter().zip(&relay_stats_before) {
            let after = network
                .observer_slow_reader_relay_stats_for(&observer.id())
                .ok_or_else(|| {
                    eyre!(
                        "observer slow-reader relay stats disappeared for {}",
                        observer.mnemonic()
                    )
                })?;
            let delayed_reads_during_pressure =
                after.delayed_reads.saturating_sub(before.delayed_reads);
            let forwarded_bytes_during_pressure = after
                .forwarded_to_observers_bytes
                .saturating_sub(before.forwarded_to_observers_bytes);
            ensure!(
                after.accepted_connections > 0
                    && after.upstream_connections > 0
                    && delayed_reads_during_pressure > 0
                    && forwarded_bytes_during_pressure
                        >= OBSERVER_PRESSURE_PAYLOAD_BYTES as u64,
                "transparent relay for {} did not carry the exact bounded pressure body after submission: before={before:?}, after={after:?}, delayed_reads_delta={delayed_reads_during_pressure}, forwarded_bytes_delta={forwarded_bytes_during_pressure}",
                observer.mnemonic(),
            );
        }
        Ok(())
    }
    .await;

    network.shutdown_and_release().await;
    result
}

/// A four-voter v2 network must finalize across one validator outage, recover
/// the restarted validator, and keep finalizing with the full roster restored.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn authoritative_v2_finalizes_through_validator_restart() -> Result<()> {
    init_instruction_registry();

    let builder = NetworkBuilder::new()
        .with_peers(VALIDATOR_COUNT)
        .with_auto_populated_trusted_peers()
        .with_block_cadence(RESTART_BLOCK_CADENCE)
        .with_sync_timeout(Duration::from_secs(180));
    let context = stringify!(authoritative_v2_finalizes_through_validator_restart);
    let network = sandbox::start_network_async_or_skip(builder, context).await?;
    let Some(network) = sandbox::enforce_network_start_requirement(network, context)? else {
        return Ok(());
    };

    let result = async {
        ensure!(
            network.peers().len() == VALIDATOR_COUNT,
            "test requires exactly {VALIDATOR_COUNT} voting validators, got {}",
            network.peers().len()
        );
        ensure!(
            network.topology_entries().len() == VALIDATOR_COUNT
                && network
                    .peers()
                    .iter()
                    .all(|peer| peer.genesis_pop().is_some()),
            "all four validators must have BLS proof-of-possession entries in fresh genesis"
        );
        ensure!(
            network.peers().iter().all(NetworkPeer::is_running),
            "all four voting validators must be running after fresh genesis"
        );

        let all_peers = network.peers().to_vec();
        let initial_statuses =
            wait_for_normal_statuses(&all_peers, 1, STATUS_TIMEOUT).await?;
        ensure!(
            initial_statuses.iter().all(|status| status.blocks >= 1),
            "fresh genesis must be committed by every validator: {initial_statuses:?}"
        );
        let initial_committed_floor = initial_statuses
            .iter()
            .map(|status| status.blocks)
            .min()
            .unwrap_or_default();
        let initial_v2 =
            wait_for_common_awaiting_v2_round(&all_peers, initial_committed_floor, STATUS_TIMEOUT)
                .await?;
        validate_v2_status_set(&initial_v2, VALIDATOR_COUNT)?;

        let before_restart_account = fixture_account(0xA1)?;
        let during_outage_account = fixture_account(0xA2)?;
        let after_restart_account = fixture_account(0xA3)?;
        assert_accounts_absent(
            &all_peers,
            &[
                before_restart_account.clone(),
                during_outage_account.clone(),
                after_restart_account.clone(),
            ],
        )
        .await?;

        let first_target_non_empty = initial_statuses
            .iter()
            .map(|status| status.blocks_non_empty)
            .max()
            .unwrap_or_default()
            .saturating_add(1);
        submit_account(network.client(), before_restart_account.clone()).await?;
        network
            .ensure_blocks_with(|height| height.non_empty >= first_target_non_empty)
            .await
            .wrap_err("all four v2 validators did not finalize the pre-restart transaction")?;
        wait_for_accounts_visible(
            &all_peers,
            &[before_restart_account.clone()],
            ACCOUNT_VISIBILITY_TIMEOUT,
        )
        .await?;

        let pre_restart_statuses = wait_for_normal_statuses(
            &all_peers,
            initial_committed_floor.saturating_add(1),
            STATUS_TIMEOUT,
        )
        .await?;
        let pre_restart_floor = pre_restart_statuses
            .iter()
            .map(|status| status.blocks)
            .min()
            .unwrap_or_default();
        ensure!(
            pre_restart_floor > initial_committed_floor,
            "the pre-restart transaction must advance committed height (initial={initial_committed_floor}, current={pre_restart_floor})"
        );
        let pre_restart_v2 =
            wait_for_common_awaiting_v2_round(&all_peers, pre_restart_floor, STATUS_TIMEOUT)
                .await?;
        validate_v2_status_set(&pre_restart_v2, VALIDATOR_COUNT)?;
        for snapshot in &pre_restart_v2 {
            validate_applied_successor_witness(snapshot, pre_restart_floor)?;
        }

        let config_layers = network
            .config_layers()
            .collect::<Vec<_>>();
        let restart_index = VALIDATOR_COUNT - 1;
        let restart_peer = network.peers()[restart_index].clone();
        let restart_node_fingerprint = pre_restart_v2[restart_index].node_fingerprint.clone();
        restart_peer.shutdown().await;

        let remaining_peers = network
            .peers()
            .iter()
            .filter(|peer| peer.is_running())
            .cloned()
            .collect::<Vec<_>>();
        ensure!(
            remaining_peers.len() == VALIDATOR_COUNT - 1,
            "exactly three voting validators must remain after one-peer shutdown, got {}",
            remaining_peers.len()
        );

        let outage_baseline = normal_statuses(&remaining_peers).await?;
        let outage_target_non_empty = outage_baseline
            .iter()
            .map(|status| status.blocks_non_empty)
            .max()
            .unwrap_or_default()
            .saturating_add(1);
        submit_account(network.client(), during_outage_account.clone()).await?;
        network
            .ensure_blocks_with(|height| height.non_empty >= outage_target_non_empty)
            .await
            .wrap_err("the three-voter quorum did not finalize while one validator was offline")?;
        wait_for_accounts_visible(
            &remaining_peers,
            &[before_restart_account.clone(), during_outage_account.clone()],
            ACCOUNT_VISIBILITY_TIMEOUT,
        )
        .await?;

        let outage_statuses = wait_for_normal_statuses(
            &remaining_peers,
            pre_restart_floor.saturating_add(1),
            STATUS_TIMEOUT,
        )
        .await?;
        let outage_floor = outage_statuses
            .iter()
            .map(|status| status.blocks)
            .min()
            .unwrap_or_default();
        ensure!(
            outage_floor > pre_restart_floor,
            "the online quorum must advance height during the outage (before={pre_restart_floor}, during={outage_floor})"
        );
        let outage_v2 = wait_for_common_awaiting_v2_round(
            &remaining_peers,
            outage_floor,
            STATUS_TIMEOUT,
        )
        .await?;
        validate_v2_status_set(&outage_v2, VALIDATOR_COUNT)?;
        for snapshot in &outage_v2 {
            validate_applied_successor_witness(snapshot, outage_floor)?;
        }

        restart_peer
            .start_checked(config_layers.iter().cloned(), None)
            .await
            .wrap_err_with(|| format!("restart v2 validator {}", restart_peer.mnemonic()))?;
        ensure!(restart_peer.is_running(), "restarted validator must be running");
        network
            .ensure_blocks_with(|height| {
                height.total >= outage_floor && height.non_empty >= outage_target_non_empty
            })
            .await
            .wrap_err("restarted v2 validator did not catch up to outage finality")?;
        wait_for_accounts_visible(
            &all_peers,
            &[before_restart_account.clone(), during_outage_account.clone()],
            ACCOUNT_VISIBILITY_TIMEOUT,
        )
        .await?;

        let recovered_statuses =
            wait_for_normal_statuses(&all_peers, outage_floor, STATUS_TIMEOUT).await?;
        let recovered_floor = recovered_statuses
            .iter()
            .map(|status| status.blocks)
            .min()
            .unwrap_or_default();
        let recovered_v2 =
            wait_for_common_awaiting_v2_round(&all_peers, recovered_floor, STATUS_TIMEOUT).await?;
        validate_v2_status_set(&recovered_v2, VALIDATOR_COUNT)?;
        for snapshot in &recovered_v2 {
            validate_applied_successor_witness(snapshot, recovered_floor)?;
        }
        ensure!(
            recovered_v2[restart_index].node_fingerprint == restart_node_fingerprint,
            "a restarted validator must retain its v2 node identity"
        );

        let post_restart_target_non_empty = recovered_statuses
            .iter()
            .map(|status| status.blocks_non_empty)
            .max()
            .unwrap_or_default()
            .saturating_add(1);
        submit_account(network.client(), after_restart_account.clone()).await?;
        network
            .ensure_blocks_with(|height| height.non_empty >= post_restart_target_non_empty)
            .await
            .wrap_err("the restored four-voter v2 network did not finalize a successor block")?;
        wait_for_accounts_visible(
            &all_peers,
            &[
                before_restart_account,
                during_outage_account,
                after_restart_account,
            ],
            ACCOUNT_VISIBILITY_TIMEOUT,
        )
        .await?;

        let final_statuses = wait_for_normal_statuses(
            &all_peers,
            recovered_floor.saturating_add(1),
            STATUS_TIMEOUT,
        )
        .await?;
        let final_floor = final_statuses
            .iter()
            .map(|status| status.blocks)
            .min()
            .unwrap_or_default();
        ensure!(
            final_floor > recovered_floor,
            "finalization must continue after restart (recovered={recovered_floor}, final={final_floor})"
        );
        let final_v2 =
            wait_for_common_awaiting_v2_round(&all_peers, final_floor, STATUS_TIMEOUT).await?;
        validate_v2_status_set(&final_v2, VALIDATOR_COUNT)?;
        for snapshot in &final_v2 {
            validate_applied_successor_witness(snapshot, final_floor)?;
        }
        for (before, after) in initial_v2.iter().zip(&final_v2) {
            ensure!(
                before.node_fingerprint == after.node_fingerprint,
                "validator {} changed v2 node fingerprint across the restart scenario",
                after.peer
            );
            ensure!(
                before.build_fingerprint == after.build_fingerprint,
                "validator {} changed build fingerprint across the restart scenario",
                after.peer
            );
            ensure!(
                before.config_fingerprint == after.config_fingerprint,
                "validator {} changed consensus-config fingerprint across the restart scenario",
                after.peer
            );
        }

        Ok(())
    }
    .await;

    network.shutdown_and_release().await;
    result
}

/// The production NPoS runner must replace an unavailable view leader through
/// a persisted timeout certificate and finalize within the Taira rollout bound.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn taira_npos_leader_timeout_commits_within_rotation_bound() -> Result<()> {
    init_instruction_registry();

    // Taira's one-second cadence is signed into genesis. The v2 round timeout
    // is derived from that immutable cadence by the protocol.
    let builder = NetworkBuilder::new()
        .with_peers(VALIDATOR_COUNT)
        .with_auto_populated_trusted_peers()
        .with_npos_genesis_bootstrap(SumeragiNposParameters::default().min_self_bond().clone())
        .with_block_cadence(TAIRA_BLOCK_CADENCE)
        .with_sync_timeout(Duration::from_secs(180));
    let context = stringify!(taira_npos_leader_timeout_commits_within_rotation_bound);
    let network = sandbox::start_network_async_or_skip(builder, context).await?;
    let Some(network) = sandbox::enforce_network_start_requirement(network, context)? else {
        return Ok(());
    };

    let result = async {
        ensure!(
            network.peers().len() == VALIDATOR_COUNT
                && network.topology_entries().len() == VALIDATOR_COUNT,
            "Taira regression requires exactly four voting validators"
        );
        ensure!(
            network.peers().iter().all(NetworkPeer::is_running),
            "all Taira validators must be running after fresh NPoS genesis"
        );

        let all_peers = network.peers().to_vec();
        let seed_account = fixture_account(0xB0)?;
        let outage_account = fixture_account(0xB1)?;
        assert_accounts_absent(
            &all_peers,
            &[seed_account.clone(), outage_account.clone()],
        )
        .await?;

        // Commit a seed transaction so the next height has just opened. The
        // view-zero leader then has the full one-second cadence remaining,
        // which makes the leader outage deterministic rather than a race with
        // an already disseminated proposal.
        let initial = normal_statuses(&all_peers).await?;
        let seed_target_non_empty = initial
            .iter()
            .map(|status| status.blocks_non_empty)
            .max()
            .unwrap_or_default()
            .saturating_add(1);
        submit_account(network.client(), seed_account.clone()).await?;
        network
            .ensure_blocks_with(|height| height.non_empty >= seed_target_non_empty)
            .await
            .wrap_err("Taira NPoS seed transaction did not finalize")?;

        let seeded = normal_statuses(&all_peers).await?;
        let seeded_floor = seeded
            .iter()
            .map(|status| status.blocks)
            .min()
            .unwrap_or_default();
        let outage_round = wait_for_common_awaiting_v2_round(
            &all_peers,
            seeded_floor,
            STATUS_TIMEOUT,
        )
        .await?;
        let target_height = outage_round[0].height;
        let initial_view = outage_round[0].view;
        let leader = outage_round[0].leader;
        let leader_peer_index = peer_index_for_validator(&all_peers, leader)?;
        let leader_peer = all_peers[leader_peer_index].clone();
        let leader_node_fingerprint = outage_round[leader_peer_index].node_fingerprint.clone();
        let config_layers = network
            .config_layers()
            .collect::<Vec<_>>();

        leader_peer.shutdown().await;
        ensure!(
            !leader_peer.is_running(),
            "the selected view leader must be offline before the proposal"
        );
        let remaining_peers = all_peers
            .iter()
            .filter(|peer| peer.is_running())
            .cloned()
            .collect::<Vec<_>>();
        ensure!(
            remaining_peers.len() == VALIDATOR_COUNT - 1,
            "exactly three NPoS voters must remain after the leader outage"
        );

        let outage_baseline = normal_statuses(&remaining_peers).await?;
        let outage_target_non_empty = outage_baseline
            .iter()
            .map(|status| status.blocks_non_empty)
            .max()
            .unwrap_or_default()
            .saturating_add(1);
        submit_account(remaining_peers[0].client(), outage_account.clone()).await?;

        let recovery_started = Instant::now();
        tokio::time::timeout(
            TAIRA_RECOVERY_BOUND,
            network.ensure_blocks_with(|height| {
                height.total >= target_height && height.non_empty >= outage_target_non_empty
            }),
        )
        .await
        .wrap_err(
            "three-voter Taira quorum exceeded one leader rotation plus one successful round",
        )??;
        let recovery_elapsed = recovery_started.elapsed();
        ensure!(
            recovery_elapsed <= TAIRA_RECOVERY_BOUND,
            "Taira recovery exceeded {:?}: elapsed={recovery_elapsed:?}",
            TAIRA_RECOVERY_BOUND
        );

        wait_for_accounts_visible(
            &remaining_peers,
            &[seed_account.clone(), outage_account.clone()],
            ACCOUNT_VISIBILITY_TIMEOUT,
        )
        .await?;

        let mut committed_views = Vec::with_capacity(remaining_peers.len());
        for peer in &remaining_peers {
            committed_views.push(committed_view_at_height(peer, target_height).await?);
        }
        ensure!(
            committed_views.iter().all(|view| *view > initial_view),
            "the outage block must be proposed after a certified view advance: height={target_height}, initial_view={initial_view}, committed_views={committed_views:?}"
        );
        ensure!(
            committed_views
                .iter()
                .all(|view| *view <= initial_view.saturating_add(VALIDATOR_COUNT as u64)),
            "the outage block exceeded one complete leader rotation: initial_view={initial_view}, committed_views={committed_views:?}"
        );
        ensure!(
            committed_views.windows(2).all(|views| views[0] == views[1]),
            "validators disagreed on the committed block view at height {target_height}: {committed_views:?}"
        );

        leader_peer
            .start_checked(config_layers.iter().cloned(), None)
            .await
            .wrap_err_with(|| format!("restart Taira leader {}", leader_peer.mnemonic()))?;
        network
            .ensure_blocks_with(|height| {
                height.total >= target_height && height.non_empty >= outage_target_non_empty
            })
            .await
            .wrap_err("restarted Taira leader did not catch up to later-view finality")?;
        wait_for_accounts_visible(
            &all_peers,
            &[seed_account, outage_account],
            ACCOUNT_VISIBILITY_TIMEOUT,
        )
        .await?;

        let recovered =
            wait_for_common_awaiting_v2_round(&all_peers, target_height, STATUS_TIMEOUT).await?;
        validate_v2_status_set(&recovered, VALIDATOR_COUNT)?;
        for snapshot in &recovered {
            validate_applied_successor_witness(snapshot, target_height)?;
        }
        ensure!(
            recovered[leader_peer_index].node_fingerprint == leader_node_fingerprint,
            "the restarted Taira leader changed its v2 node fingerprint"
        );

        Ok(())
    }
    .await;

    network.shutdown_and_release().await;
    result
}

/// Revision-1 receiver partitions staged before Sumeragi starts must retain a
/// 2+2 split of honest, round-distinct PrepareQC references for the same locked
/// subject without deciding, then converge on that exact body after ordered
/// release of captured quorum evidence.
///
/// The distinct-subject adversarial schedule is covered separately by
/// `real_network_distinct_subject_prepare_qcs_converge_after_causal_release`.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn real_network_same_subject_locked_reproposal_converges_after_ordered_quorum_release()
-> Result<()> {
    init_instruction_registry();

    const CONTROL_TIMEOUT: Duration = Duration::from_secs(20);
    let builder = NetworkBuilder::new()
        .with_peers(VALIDATOR_COUNT)
        .with_auto_populated_trusted_peers()
        .with_npos_genesis_bootstrap(SumeragiNposParameters::default().min_self_bond().clone())
        .with_block_cadence(Duration::from_secs(2))
        .with_initial_consensus_message_control_rules(
            LOCKED_REPROPOSAL_QUEUE_CAPACITY,
            locked_reproposal_receiver_rules,
        )
        .with_sync_timeout(Duration::from_secs(180));
    let context = stringify!(
        real_network_same_subject_locked_reproposal_converges_after_ordered_quorum_release
    );
    let network = sandbox::start_network_async_or_skip(builder, context).await?;
    let Some(network) = sandbox::enforce_network_start_requirement(network, context)? else {
        return Ok(());
    };

    let result = async {
        let peers = network.peers().to_vec();
        ensure!(
            peers.len() == VALIDATOR_COUNT,
            "locked-body PrepareQC split regression requires four voting validators"
        );
        let peer_ids = peers.iter().map(NetworkPeer::id).collect::<Vec<_>>();
        let expected_rules = peers
            .iter()
            .enumerate()
            .map(|(receiver_index, _)| locked_reproposal_receiver_rules(receiver_index, &peer_ids))
            .collect::<Vec<_>>();
        try_join_all(peers.iter().zip(&expected_rules).map(|(peer, expected)| async move {
            let ack = peer
                .consensus_message_control()
                .ok_or_else(|| eyre!("{} lacks the requested controller", peer.mnemonic()))?
                .wait_until_ready(CONTROL_TIMEOUT)
                .await
                .wrap_err_with(|| format!("wait for {} staged controller startup", peer.mnemonic()))?;
            ensure!(
                ack.revision == 1
                    && ack.rules.as_slice() == expected.as_slice()
                    && ack.queue_capacity == LOCKED_REPROPOSAL_QUEUE_CAPACITY,
                "{} did not acknowledge its exact pre-Sumeragi receiver rules: revision={}, queue_capacity={}, rules_match={}",
                peer.mnemonic(),
                ack.revision,
                ack.queue_capacity,
                ack.rules.as_slice() == expected.as_slice(),
            );
            ensure!(!ack.fatal, "{} controller failed closed", peer.mnemonic());
            ensure!(
                ack.overflowed == 0,
                "{} hold queue overflowed before scenario submission",
                peer.mnemonic()
            );
            Ok::<(), eyre::Report>(())
        }))
        .await?;

        let target_height = LOCKED_REPROPOSAL_HEIGHT;
        let first_view = LOCKED_REPROPOSAL_FIRST_VIEW;
        let second_view = LOCKED_REPROPOSAL_SECOND_VIEW;
        let pre_submission =
            wait_for_v2_statuses(&peers, target_height.saturating_sub(1), STATUS_TIMEOUT).await?;
        validate_v2_status_set(&pre_submission, VALIDATOR_COUNT)?;
        ensure!(
            pre_submission
                .iter()
                .all(|snapshot| snapshot.last_committed_height < target_height),
            "pre-staged receiver rules failed to keep height {target_height} undecided before scenario submission: {:?}",
            pre_submission
                .iter()
                .map(|snapshot| (
                    snapshot.peer.clone(),
                    snapshot.height,
                    snapshot.view,
                    snapshot.last_committed_height,
                ))
                .collect::<Vec<_>>()
        );

        let account = fixture_account(0xC1)?;
        assert_accounts_absent(&peers, &[account.clone()]).await?;
        enqueue_account(peers[0].client(), account.clone()).await?;

        let partitioned = wait_for_locked_reproposal_prepare_qc_split(
            &peers,
            target_height,
            first_view,
            second_view,
            STATUS_TIMEOUT,
        )
        .await?;
        validate_v2_status_set(&partitioned, VALIDATOR_COUNT)?;
        let qcs = partitioned
            .iter()
            .map(|snapshot| {
                snapshot
                    .highest_prepare_qc
                    .as_ref()
                    .expect("split helper requires every validator to expose a PrepareQC")
            })
            .collect::<Vec<_>>();
        let split = classify_locked_reproposal_prepare_qc_split(
            &qcs,
            target_height,
            first_view,
            second_view,
        )
        .expect("wait helper returned a malformed locked-body PrepareQC split");
        let locked_reference = *split.locked;
        let reproposed_reference = *split.reproposed;
        ensure!(
            locked_reference != reproposed_reference,
            "locked and re-proposed PrepareQCs must retain round-distinct references"
        );
        ensure!(
            locked_reference.subject == reproposed_reference.subject,
            "successive-view PrepareQCs must certify one identical locked subject"
        );
        ensure!(
            locked_reference.execution_commitment == reproposed_reference.execution_commitment,
            "exact locked-body re-proposal must retain its deterministic execution commitment"
        );
        let canonical_block_hash = locked_reference.subject.block_hash;
        let canonical_subject_hash = locked_reference.subject.block_hash.to_string();
        for (receiver_index, snapshot) in partitioned.iter().enumerate() {
            let highest_qc = snapshot
                .highest_prepare_qc
                .as_ref()
                .expect("every partitioned node has a valid PrepareQC");
            let locked_qc = snapshot
                .locked_prepare_qc
                .as_ref()
                .expect("every partitioned node has a durable PrepareQC lock");
            let expected_reference = if receiver_index < 2 {
                reproposed_reference
            } else {
                locked_reference
            };
            ensure!(
                highest_qc.reference == expected_reference,
                "validator {} exposed the wrong PrepareQC reference for receiver group {receiver_index}: expected={expected_reference:?}, actual={:?}",
                snapshot.peer,
                highest_qc.reference,
            );
            ensure!(
                locked_qc.reference == expected_reference,
                "validator {} did not durably lock the exact PrepareQC for receiver group {receiver_index}: expected={expected_reference:?}, actual={:?}",
                snapshot.peer,
                locked_qc.reference,
            );
            ensure!(
                highest_qc.reference.subject == locked_reference.subject,
                "validator {} exposed a PrepareQC for a different subject during locked-body re-proposal",
                snapshot.peer
            );
            ensure!(
                snapshot.body_state == SumeragiV2BodyState::Validated,
                "validator {} did not retain a validated locked body: {:?}",
                snapshot.peer,
                snapshot.body_state,
            );
            ensure!(
                snapshot.last_committed_height < target_height,
                "controlled validator {} decided before partition healing",
                snapshot.peer
            );
            validate_locked_commit_progress_witness(snapshot, &expected_reference)?;
        }

        let prepare_releases = wait_for_held_quorum_evidence(
            &peers[2..],
            target_height,
            second_view,
            &canonical_block_hash,
            ConsensusMessageControlKind::PrepareVote,
            ConsensusMessageControlKind::PrepareCertificate,
            STATUS_TIMEOUT,
        )
        .await
        .wrap_err("locked receivers did not retain a releasable view-1 Prepare quorum")?;
        ensure!(
            prepare_releases.len() == VALIDATOR_COUNT / 2
                && prepare_releases.iter().all(|release| !release.is_empty()),
            "locked receivers must expose non-empty captured Prepare sequences"
        );
        try_join_all(
            peers[2..]
                .iter()
                .zip(&expected_rules[2..])
                .zip(&prepare_releases)
                .map(|((peer, expected), release)| async move {
                    let ack = peer
                        .consensus_message_control()
                        .expect("controlled peer")
                        .apply(
                            expected,
                            release,
                            LOCKED_REPROPOSAL_QUEUE_CAPACITY,
                            CONTROL_TIMEOUT,
                        )
                        .await
                        .wrap_err_with(|| {
                            format!("release captured Prepare traffic to {}", peer.mnemonic())
                        })?;
                    ensure!(
                        ack.rules.as_slice() == expected.as_slice()
                            && ack.delivered.as_slice() == release.as_slice(),
                        "{} did not retain its partition rules while delivering the exact captured Prepare sequence: delivered={:?}, expected={release:?}",
                        peer.mnemonic(),
                        ack.delivered,
                    );
                    ensure!(!ack.fatal, "{} controller failed closed", peer.mnemonic());
                    ensure!(ack.overflowed == 0, "{} hold queue overflowed", peer.mnemonic());
                    Ok::<(), eyre::Report>(())
                }),
        )
        .await?;

        let aligned = wait_for_exact_prepare_qc_reference(
            &peers,
            &reproposed_reference,
            target_height,
            second_view,
            STATUS_TIMEOUT,
        )
        .await
        .wrap_err("captured Prepare release did not align the four locked-body references")?;
        validate_v2_status_set(&aligned, VALIDATOR_COUNT)?;
        ensure!(
            aligned
                .iter()
                .all(|snapshot| snapshot.last_committed_height < target_height),
            "Prepare-only release unexpectedly decided the controlled height"
        );
        for snapshot in &aligned {
            validate_locked_commit_progress_witness(snapshot, &reproposed_reference)?;
        }

        let commit_releases = wait_for_held_quorum_evidence(
            &peers,
            target_height,
            second_view,
            &canonical_block_hash,
            ConsensusMessageControlKind::CommitVote,
            ConsensusMessageControlKind::CommitCertificate,
            STATUS_TIMEOUT,
        )
        .await
        .wrap_err("receivers did not retain a releasable view-1 Commit quorum")?;
        ensure!(
            commit_releases.len() == VALIDATOR_COUNT
                && commit_releases.iter().all(|release| !release.is_empty()),
            "every receiver must expose a non-empty captured Commit sequence"
        );
        let commit_release_acks = try_join_all(
            peers
                .iter()
                .zip(&expected_rules)
                .zip(&commit_releases)
                .map(|((peer, expected), release)| async move {
                    let ack = peer
                        .consensus_message_control()
                        .expect("controlled peer")
                        .apply(
                            expected,
                            release,
                            LOCKED_REPROPOSAL_QUEUE_CAPACITY,
                            CONTROL_TIMEOUT,
                        )
                        .await
                        .wrap_err_with(|| {
                            format!("release captured Commit traffic to {}", peer.mnemonic())
                        })?;
                    ensure!(
                        ack.rules.as_slice() == expected.as_slice()
                            && ack.delivered.as_slice() == release.as_slice(),
                        "{} did not retain its partition rules while delivering the exact captured Commit sequence: delivered={:?}, expected={release:?}",
                        peer.mnemonic(),
                        ack.delivered,
                    );
                    ensure!(!ack.fatal, "{} controller failed closed", peer.mnemonic());
                    ensure!(ack.overflowed == 0, "{} hold queue overflowed", peer.mnemonic());
                    Ok::<_, eyre::Report>(ack)
                }),
        )
        .await?;

        network
            .ensure_blocks_with(|height| height.total >= target_height)
            .await
            .wrap_err("captured Commit release did not finalize the controlled height")?;
        let committed_metadata =
            wait_for_committed_block_metadata(&peers, target_height, STATUS_TIMEOUT)
                .await
                .wrap_err("not every validator exposed the controlled decision")?;
        let committed_header_views = committed_metadata
            .iter()
            .map(|(view, _)| *view)
            .collect::<Vec<_>>();
        // The consensus certificate advances to `second_view`, as established
        // by `reproposed_reference` and the released controller evidence. The
        // resultless block header is part of the immutable locked bytes, so an
        // exact reproposal must retain its original header view.
        ensure!(
            committed_header_views
                .iter()
                .all(|view| *view == first_view),
            "an exact later-round reproposal must not rewrite the locked block header view: expected={first_view}, committed={committed_header_views:?}"
        );
        let committed_hashes = committed_metadata
            .into_iter()
            .map(|(_, hash)| hash)
            .collect::<Vec<_>>();
        ensure!(
            committed_hashes
                .iter()
                .all(|hash| hash == &canonical_subject_hash),
            "validators did not commit the exact re-proposed locked body: locked={canonical_subject_hash}, committed={committed_hashes:?}"
        );
        for ((((peer, expected), release), released_ack), committed_header_view) in peers
            .iter()
            .zip(&expected_rules)
            .zip(&commit_releases)
            .zip(&commit_release_acks)
            .zip(&committed_header_views)
        {
            let live_ack = peer
                .consensus_message_control()
                .expect("controlled peer")
                .read_ack()?;
            ensure!(
                live_ack.revision == released_ack.revision
                    && live_ack.rules.as_slice() == expected.as_slice()
                    && live_ack.delivered.as_slice() == release.as_slice(),
                "{} finalized the immutable header view {committed_header_view} only after its partition rules changed or its captured Commit delivery lost identity",
                peer.mnemonic(),
            );
            ensure!(!live_ack.fatal, "{} controller failed closed", peer.mnemonic());
            ensure!(
                live_ack.overflowed == 0,
                "{} hold queue overflowed before the controlled decision",
                peer.mnemonic()
            );
        }

        let healed = try_join_all(peers.iter().map(|peer| async move {
            peer.consensus_message_control()
                .expect("controlled peer")
                .heal_and_release_all(CONTROL_TIMEOUT)
                .await
                .wrap_err_with(|| format!("heal and release {} traffic", peer.mnemonic()))
        }))
        .await?;
        for (peer, ack) in peers.iter().zip(&healed) {
            ensure!(
                !ack.draining && ack.drain_fence == Some(ack.revision),
                "{} did not acknowledge the completed drain fence: revision={}, fence={:?}, draining={}",
                peer.mnemonic(),
                ack.revision,
                ack.drain_fence,
                ack.draining
            );
            ensure!(
                ack.held.is_empty()
                    && ack.release_pending.is_empty()
                    && ack.in_flight.is_none(),
                "{} completed its fence with retained traffic",
                peer.mnemonic()
            );
        }

        wait_for_accounts_visible(&peers, &[account], ACCOUNT_VISIBILITY_TIMEOUT).await?;
        let final_statuses =
            wait_for_common_awaiting_v2_round(&peers, target_height, STATUS_TIMEOUT)
                .await
                .wrap_err("re-proposed locked body did not activate one common successor height")?;
        validate_v2_status_set(&final_statuses, VALIDATOR_COUNT)?;
        for snapshot in &final_statuses {
            validate_applied_successor_witness(snapshot, target_height)?;
        }
        Ok(())
    }
    .await;

    network.shutdown_and_release().await;
    result
}

/// A staged four-validator schedule must construct two honest PrepareQCs for
/// different subjects without forging traffic or letting the lower QC reach
/// the next leader, then converge on the higher-view certified subject after
/// one FIFO drain fence heals every receiver.
///
/// Each QC receiver admits exactly two distinct remote Prepare votes alongside
/// its own vote. The authoritative liveness snapshot must show that this exact
/// three-signer pool satisfies both the frozen count threshold and the strict
/// two-thirds voting-power threshold. This is the reset/dedup boundary which
/// previously let an authenticated locked-round intent remain durable while
/// its volatile reconstruction path was suppressed.
///
/// Quorum intersection means an honest 2+2 *lock* split for distinct subjects
/// is impossible: after one node locks the first QC, the other three validators
/// are the only causally valid quorum that can certify the second subject. The
/// observable 2+2 split is therefore over highest PrepareQC references; one
/// first-group node owns the old lock, while the other observes that QC only
/// after it has already voted in the next view.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn real_network_distinct_subject_prepare_qcs_converge_after_causal_release() -> Result<()> {
    init_instruction_registry();

    const CONTROL_TIMEOUT: Duration = Duration::from_secs(20);
    let builder = NetworkBuilder::new()
        .with_peers(VALIDATOR_COUNT)
        .with_auto_populated_trusted_peers()
        .with_npos_genesis_bootstrap(SumeragiNposParameters::default().min_self_bond().clone())
        .with_base_seed(stringify!(
            real_network_distinct_subject_prepare_qcs_converge_after_causal_release
        ))
        .with_block_cadence(DISTINCT_PREPARE_QC_BLOCK_CADENCE)
        .with_initial_consensus_message_control_rules(
            DISTINCT_PREPARE_QC_QUEUE_CAPACITY,
            distinct_prepare_qc_receiver_rules,
        )
        .with_sync_timeout(Duration::from_secs(180));
    let context =
        stringify!(real_network_distinct_subject_prepare_qcs_converge_after_causal_release);
    let network = sandbox::start_network_async_or_skip(builder, context).await?;
    let Some(network) = sandbox::enforce_network_start_requirement(network, context)? else {
        return Ok(());
    };

    let result = async {
        let peers = network.peers().to_vec();
        ensure!(
            peers.len() == VALIDATOR_COUNT,
            "distinct-subject PrepareQC regression requires four voting validators"
        );
        let peer_ids = peers.iter().map(NetworkPeer::id).collect::<Vec<_>>();
        let validator_by_peer = validator_indices_by_peer(&peers)?;
        let expected_rules = peers
            .iter()
            .enumerate()
            .map(|(receiver_index, _)| {
                distinct_prepare_qc_receiver_rules(receiver_index, &peer_ids)
            })
            .collect::<Vec<_>>();
        try_join_all(peers.iter().zip(&expected_rules).map(|(peer, expected)| async move {
            let ack = peer
                .consensus_message_control()
                .ok_or_else(|| eyre!("{} lacks the requested controller", peer.mnemonic()))?
                .wait_until_ready(CONTROL_TIMEOUT)
                .await
                .wrap_err_with(|| format!("wait for {} staged controller", peer.mnemonic()))?;
            ensure!(
                ack.revision == 1
                    && ack.rules.as_slice() == expected.as_slice()
                    && ack.queue_capacity == DISTINCT_PREPARE_QC_QUEUE_CAPACITY
                    && !ack.fatal
                    && ack.overflowed == 0,
                "{} did not acknowledge the exact distinct-subject freeze rules",
                peer.mnemonic()
            );
            Ok::<(), eyre::Report>(())
        }))
        .await?;

        let height = LOCKED_REPROPOSAL_HEIGHT;
        let first_view = LOCKED_REPROPOSAL_FIRST_VIEW;
        let second_view = LOCKED_REPROPOSAL_SECOND_VIEW;
        let initial = wait_for_v2_status_condition(
            &peers,
            "one common frozen view-zero height",
            STATUS_TIMEOUT,
            |snapshots| {
                snapshots.iter().all(|snapshot| {
                    snapshot.height == height
                        && snapshot.view == first_view
                        && snapshot.last_committed_height < height
                        && snapshot.leader == snapshots[0].leader
                        && status_round_is_open(snapshot, height, first_view)
                })
            },
        )
        .await?;
        validate_v2_status_set(&initial, VALIDATOR_COUNT)?;
        ensure!(
            initial.iter().all(|snapshot| {
                snapshot.height_context.mode == ConsensusMode::Npos
                    && snapshot.height_context.quorum.min_signers == 3
            }),
            "distinct-subject regression requires the canonical four-validator NPoS dual quorum: {initial:?}"
        );
        validate_open_round(&initial, height, first_view)?;
        let view_zero_release_started = Instant::now();

        let first_leader_validator = initial[0].leader;
        ensure!(
            first_leader_validator < VALIDATOR_COUNT as u64,
            "view-zero leader index is outside the four-validator roster"
        );
        let second_leader_validator = (first_leader_validator + 1) % VALIDATOR_COUNT as u64;
        let first_lock_index = peer_index_for_validator(&peers, first_leader_validator)?;
        let second_leader_index = peer_index_for_validator(&peers, second_leader_validator)?;
        ensure!(
            first_lock_index != second_leader_index,
            "successive views unexpectedly selected the same validator"
        );
        let unlocked = (0..VALIDATOR_COUNT)
            .filter(|index| *index != first_lock_index)
            .collect::<Vec<_>>();
        ensure!(
            unlocked.contains(&second_leader_index),
            "the view-one leader must remain outside the staged old lock"
        );
        let second_qc_partner = unlocked
            .iter()
            .copied()
            .find(|index| *index != second_leader_index)
            .expect("three unlocked validators include a QC partner");
        let first_qc_observer = unlocked
            .iter()
            .copied()
            .find(|index| *index != second_leader_index && *index != second_qc_partner)
            .expect("three unlocked validators include an old-QC observer");
        let first_group = [first_lock_index, first_qc_observer];
        let second_group = [second_leader_index, second_qc_partner];

        let first_account = fixture_account(0xC2)?;
        let second_account = fixture_account(0xC3)?;
        assert_accounts_absent(&peers, &[first_account.clone(), second_account.clone()]).await?;
        enqueue_account(peers[first_lock_index].client(), first_account.clone()).await?;

        let first_vote_senders = peer_ids
            .iter()
            .enumerate()
            .filter(|(index, _)| *index != first_lock_index)
            .map(|(_, peer_id)| {
                (
                    peer_id.clone(),
                    *validator_by_peer
                        .get(peer_id)
                        .expect("every network peer belongs to the frozen roster"),
                )
            })
            .collect::<BTreeMap<_, _>>();
        let first_prepare = wait_for_control_selection(
            &peers[first_lock_index],
            "two distinct view-zero Prepare votes for one subject",
            DISTINCT_PREPARE_QC_A_SELECTION_TIMEOUT,
            |ack| {
                held_prepare_vote_subject(
                    ack,
                    height,
                    first_view,
                    &first_vote_senders,
                    None,
                    LOCKED_REPROPOSAL_REMOTE_QUORUM_VOTES,
                )
            },
        )
        .await?;
        let first_subject = first_prepare
            .subject
            .expect("Prepare-vote selection carries a complete subject");
        let first_execution_commitment = first_prepare
            .execution_commitment
            .expect("Prepare-vote selection carries an execution commitment");
        let first_block_hash = first_subject.block_hash;
        ensure!(
            first_prepare.senders.len() == LOCKED_REPROPOSAL_REMOTE_QUORUM_VOTES
                && first_prepare.signers.len() == LOCKED_REPROPOSAL_REMOTE_QUORUM_VOTES
                && first_prepare.envelope_digests.len()
                    == LOCKED_REPROPOSAL_REMOTE_QUORUM_VOTES,
            "view-zero release did not bind two distinct authenticated signed envelopes: {first_prepare:?}"
        );
        let immediately_before_a = fetch_v2_status_set(&peers).await?;
        validate_v2_status_set(&immediately_before_a, VALIDATOR_COUNT)?;
        validate_open_round(&immediately_before_a, height, first_view).wrap_err(
            "view zero closed after A votes were retained but before their exact release",
        )?;
        ensure!(
            view_zero_release_started.elapsed() < DISTINCT_PREPARE_QC_A_RELEASE_BUDGET,
            "view-zero causal release exceeded its explicit pre-timeout budget before the release fence"
        );
        release_exact_control_sequences(
            &peers[first_lock_index],
            &expected_rules[first_lock_index],
            &first_prepare.sequences,
            "view-zero Prepare votes",
            CONTROL_TIMEOUT,
        )
        .await?;
        ensure!(
            view_zero_release_started.elapsed() < DISTINCT_PREPARE_QC_A_RELEASE_BUDGET,
            "view-zero causal release exceeded its explicit pre-timeout budget while crossing the release fence"
        );
        let immediately_after_a = fetch_v2_status(&peers[first_lock_index]).await?;
        ensure!(
            immediately_after_a
                .locked_prepare_qc
                .as_ref()
                .is_some_and(|qc| {
                    qc.reference.round.height == height
                        && qc.reference.round.view == first_view
                        && qc.reference.subject == first_subject
                        && qc.reference.execution_commitment == first_execution_commitment
                })
                || status_round_is_open(&immediately_after_a, height, first_view),
            "view-zero timeout raced the exact A release fence before the controlled receiver could install its QC: {immediately_after_a:?}"
        );

        let first_locked = wait_for_v2_status_condition(
            &peers,
            "one isolated durable view-zero PrepareQC lock",
            STATUS_TIMEOUT,
            |snapshots| {
                snapshots[first_lock_index]
                    .locked_prepare_qc
                    .as_ref()
                    .is_some_and(|qc| {
                        qc.reference.round.height == height
                            && qc.reference.round.view == first_view
                            && qc.reference.subject.block_hash == first_block_hash
                    })
                    && snapshots
                        .iter()
                        .enumerate()
                        .filter(|(index, _)| *index != first_lock_index)
                        .all(|(_, snapshot)| {
                            snapshot.locked_prepare_qc.is_none()
                                && snapshot.highest_prepare_qc.is_none()
                        })
            },
        )
        .await?;
        validate_v2_status_set(&first_locked, VALIDATOR_COUNT)?;
        let first_reference = first_locked[first_lock_index]
            .locked_prepare_qc
            .as_ref()
            .expect("wait condition requires the old lock")
            .reference;
        ensure!(
            first_reference.subject == first_subject
                && first_reference.execution_commitment == first_execution_commitment,
            "the first leader locked different evidence than the exact released A votes: released={first_prepare:?}, locked={first_reference:?}"
        );
        let mut first_qc_signers = first_prepare.signers.clone();
        first_qc_signers.insert(
            ValidatorIndex::try_from(first_leader_validator)
                .expect("four-validator leader index fits the wire type"),
        );
        ensure!(
            first_qc_signers.len() == 3,
            "A's QC did not consist of the leader plus the two exact remote signers: {first_qc_signers:?}"
        );
        let first_qc_signers = first_qc_signers.into_iter().collect::<Vec<_>>();
        validate_minimal_exact_prepare_quorum(
            &first_locked[first_lock_index],
            &first_reference,
        )?;
        validate_locked_commit_progress_witness(
            &first_locked[first_lock_index],
            &first_reference,
        )?;
        let first_certificate_sender = BTreeSet::from([peer_ids[first_lock_index].clone()]);
        let old_certificate_release = wait_for_control_selection(
            &peers[first_qc_observer],
            "the authenticated view-zero PrepareQC broadcast from its sole receiver",
            STATUS_TIMEOUT,
            |ack| {
                held_prepare_certificate_sequences(
                    ack,
                    height,
                    first_view,
                    &first_certificate_sender,
                    &first_subject,
                    &first_execution_commitment,
                    &first_qc_signers,
                    1,
                )
            },
        )
        .await?;
        let unsafe_proposal_before = ignore_count(
            &first_locked[first_lock_index],
            SumeragiV2IgnoreReason::UnsafeProposal,
        );

        // Queue work directly at the next leader only after subject A is
        // frozen. Its no-high-QC timeout justification must therefore produce
        // a genuinely new proposal subject rather than reload A's bytes.
        enqueue_account(peers[second_leader_index].client(), second_account.clone()).await?;

        let unlocked_senders = unlocked
            .iter()
            .map(|index| {
                let peer = peer_ids[*index].clone();
                let signer = *validator_by_peer
                    .get(&peer)
                    .expect("every unlocked peer belongs to the frozen roster");
                (peer, signer)
            })
            .collect::<BTreeMap<_, _>>();
        let unlocked_validator_indices = unlocked_senders
            .values()
            .copied()
            .collect::<BTreeSet<_>>();
        let mut timeout_releases = Vec::with_capacity(VALIDATOR_COUNT);
        for receiver_index in 0..VALIDATOR_COUNT {
            let allowed = if receiver_index == first_lock_index {
                unlocked_senders.clone()
            } else {
                unlocked
                    .iter()
                    .filter(|index| **index != receiver_index)
                    .map(|index| {
                        let peer = peer_ids[*index].clone();
                        let signer = *validator_by_peer
                            .get(&peer)
                            .expect("every unlocked peer belongs to the frozen roster");
                        (peer, signer)
                    })
                    .collect::<BTreeMap<_, _>>()
            };
            let release = wait_for_control_selection(
                &peers[receiver_index],
                "two no-high-QC view-zero Timeout votes from the unlocked quorum",
                DISTINCT_PREPARE_QC_VIEW_ZERO_TIMEOUT,
                |ack| {
                    held_no_high_timeout_vote_selection(
                        ack,
                        height,
                        first_view,
                        &allowed,
                        LOCKED_REPROPOSAL_REMOTE_QUORUM_VOTES,
                    )
                },
            )
            .await?;
            ensure!(
                release.senders.len() == LOCKED_REPROPOSAL_REMOTE_QUORUM_VOTES
                    && release.signers.len() == LOCKED_REPROPOSAL_REMOTE_QUORUM_VOTES
                    && release.envelope_digests.len()
                        == LOCKED_REPROPOSAL_REMOTE_QUORUM_VOTES,
                "receiver {} did not select two exact no-high-QC timeout envelopes: {release:?}",
                peers[receiver_index].mnemonic(),
            );
            timeout_releases.push(release);
        }
        try_join_all(
            peers
                .iter()
                .zip(&expected_rules)
                .zip(&timeout_releases)
                .map(|((peer, rules), release)| async move {
                    release_exact_control_sequences(
                        peer,
                        rules,
                        &release.sequences,
                        "view-zero Timeout votes",
                        CONTROL_TIMEOUT,
                    )
                    .await
                    .map(|_| ())
                }),
        )
        .await?;

        let view_one = wait_for_v2_status_condition(
            &peers,
            "all validators in frozen view one with the next leader selected",
            STATUS_TIMEOUT,
            |snapshots| {
                snapshots.iter().all(|snapshot| {
                    snapshot.height == height
                        && snapshot.view == second_view
                        && snapshot.leader == second_leader_validator
                        && snapshot.last_committed_height < height
                })
            },
        )
        .await?;
        validate_v2_status_set(&view_one, VALIDATOR_COUNT)?;
        for (index, snapshot) in view_one.iter().enumerate() {
            let timeout = snapshot.last_timeout_certificate.as_ref().ok_or_else(|| {
                eyre!(
                    "{} entered view one without exposing the exact installed TimeoutCertificate",
                    snapshot.peer
                )
            })?;
            ensure!(
                timeout.round.height == height && timeout.round.view == first_view,
                "{} installed the wrong timeout round: {timeout:?}",
                snapshot.peer,
            );
            if index == first_lock_index {
                ensure!(
                    timeout.highest_prepare_qc == Some(first_reference),
                    "the A-locked validator's TC did not preserve A as its safe value: {timeout:?}"
                );
            } else {
                ensure!(
                    timeout.highest_prepare_qc.is_none(),
                    "an unlocked validator's TC unexpectedly learned A: peer={}, timeout={timeout:?}",
                    snapshot.peer,
                );
            }
        }

        let unlocked_timeout_senders = unlocked
            .iter()
            .map(|index| peer_ids[*index].clone())
            .collect::<BTreeSet<_>>();
        let unlocked_timeout_signers = unlocked_validator_indices
            .iter()
            .copied()
            .collect::<Vec<_>>();
        let _no_high_timeout_certificates = wait_for_control_selection(
            &peers[first_lock_index],
            "the three no-high-QC TimeoutCertificates with the exact unlocked signer set",
            STATUS_TIMEOUT,
            |ack| {
                held_timeout_certificate_sequences(
                    ack,
                    height,
                    first_view,
                    &unlocked_timeout_senders,
                    None,
                    None,
                    &unlocked_timeout_signers,
                    unlocked_timeout_senders.len(),
                )
            },
        )
        .await?;

        let mut locked_timeout_signers = timeout_releases[first_lock_index].signers.clone();
        locked_timeout_signers.insert(
            *validator_by_peer
                .get(&peer_ids[first_lock_index])
                .expect("the A-locked peer belongs to the frozen roster"),
        );
        ensure!(
            locked_timeout_signers.len() == 3,
            "the A-preserving TC did not combine the locked validator with exactly two released remote votes: released={:?}, complete={locked_timeout_signers:?}",
            timeout_releases[first_lock_index],
        );
        let locked_timeout_signers = locked_timeout_signers.into_iter().collect::<Vec<_>>();
        let _locked_timeout_certificate = wait_for_control_selection(
            &peers[second_leader_index],
            "the A-preserving TimeoutCertificate with its exact three signer indices",
            STATUS_TIMEOUT,
            |ack| {
                held_timeout_certificate_sequences(
                    ack,
                    height,
                    first_view,
                    &BTreeSet::from([peer_ids[first_lock_index].clone()]),
                    Some(&first_subject),
                    Some(&first_execution_commitment),
                    &locked_timeout_signers,
                    1,
                )
            },
        )
        .await?;

        let second_leader_prepare = wait_for_control_selection(
            &peers[second_leader_index],
            "two unlocked view-one Prepare votes for a subject distinct from the old lock",
            STATUS_TIMEOUT,
            |ack| {
                held_prepare_vote_subject(
                    ack,
                    height,
                    second_view,
                    &unlocked_senders,
                    Some(&first_subject),
                    LOCKED_REPROPOSAL_REMOTE_QUORUM_VOTES,
                )
            },
        )
        .await?;
        let second_subject = second_leader_prepare
            .subject
            .expect("Prepare-vote selection carries a complete subject");
        let second_execution_commitment = second_leader_prepare
            .execution_commitment
            .expect("Prepare-vote selection carries an execution commitment");
        let second_block_hash = second_subject.block_hash;
        ensure!(
            second_subject != first_subject,
            "the no-high-QC view-one leader re-used the complete old block subject"
        );
        let second_partner_prepare = wait_for_control_selection(
            &peers[second_qc_partner],
            "the same two-sender view-one Prepare quorum at the second QC receiver",
            STATUS_TIMEOUT,
            |ack| {
                held_prepare_vote_subject(
                    ack,
                    height,
                    second_view,
                    &unlocked_senders,
                    Some(&first_subject),
                    LOCKED_REPROPOSAL_REMOTE_QUORUM_VOTES,
                )
            },
        )
        .await?;
        ensure!(
            second_partner_prepare.subject == Some(second_subject)
                && second_partner_prepare.execution_commitment
                    == Some(second_execution_commitment),
            "the two B-QC receivers selected different signed vote values: leader={second_leader_prepare:?}, partner={second_partner_prepare:?}"
        );
        for (receiver_index, selection) in [
            (second_leader_index, &second_leader_prepare),
            (second_qc_partner, &second_partner_prepare),
        ] {
            let local_signer = *validator_by_peer
                .get(&peer_ids[receiver_index])
                .expect("B-QC receiver belongs to the frozen roster");
            let mut complete_signers = selection.signers.clone();
            complete_signers.insert(local_signer);
            ensure!(
                complete_signers == unlocked_validator_indices
                    && selection.senders.len() == LOCKED_REPROPOSAL_REMOTE_QUORUM_VOTES
                    && selection.envelope_digests.len()
                        == LOCKED_REPROPOSAL_REMOTE_QUORUM_VOTES,
                "B-QC receiver {} did not combine its local vote with the exact other two unlocked signed envelopes: selection={selection:?}, complete={complete_signers:?}, expected={unlocked_validator_indices:?}",
                peers[receiver_index].mnemonic(),
            );
        }
        let prepare_releases = [
            (
                &peers[second_leader_index],
                expected_rules[second_leader_index].as_slice(),
                second_leader_prepare.sequences.as_slice(),
                "view-one Prepare votes at the leader",
            ),
            (
                &peers[second_qc_partner],
                expected_rules[second_qc_partner].as_slice(),
                second_partner_prepare.sequences.as_slice(),
                "view-one Prepare votes at the partner",
            ),
        ];
        try_join_all(prepare_releases.into_iter().map(
            |(peer, rules, release, description)| async move {
                release_exact_control_sequences(
                    peer,
                    rules,
                    release,
                    description,
                    CONTROL_TIMEOUT,
                )
                .await
                .map(|_| ())
            },
        ))
        .await?;

        let second_certified = wait_for_v2_status_condition(
            &peers,
            "two exact view-one PrepareQC references for subject B",
            STATUS_TIMEOUT,
            |snapshots| {
                second_group.iter().all(|index| {
                    let snapshot = &snapshots[*index];
                    snapshot.highest_prepare_qc.as_ref().is_some_and(|qc| {
                        qc.reference.round.height == height
                            && qc.reference.round.view == second_view
                            && qc.reference.subject.block_hash == second_block_hash
                            && snapshot.body_state == SumeragiV2BodyState::Validated
                            && snapshot
                                .locked_prepare_qc
                                .as_ref()
                                .is_some_and(|locked| locked.reference == qc.reference)
                    })
                })
            },
        )
        .await?;
        validate_v2_status_set(&second_certified, VALIDATOR_COUNT)?;
        let second_reference = second_certified[second_leader_index]
            .highest_prepare_qc
            .as_ref()
            .expect("wait condition requires the higher PrepareQC")
            .reference;
        ensure!(
            second_certified[second_qc_partner]
                .highest_prepare_qc
                .as_ref()
                .is_some_and(|qc| qc.reference == second_reference),
            "the two view-one receivers formed different PrepareQC references"
        );
        for receiver_index in second_group {
            validate_minimal_exact_prepare_quorum(
                &second_certified[receiver_index],
                &second_reference,
            )?;
            validate_locked_commit_progress_witness(
                &second_certified[receiver_index],
                &second_reference,
            )?;
        }
        ensure!(
            first_reference.subject != second_reference.subject,
            "the staged PrepareQCs did not certify distinct subjects"
        );
        ensure!(
            second_reference.subject == second_subject
                && second_reference.execution_commitment == second_execution_commitment,
            "the B receivers certified different evidence than the exact released votes: votes={second_leader_prepare:?}, qc={second_reference:?}"
        );
        ensure!(
            second_certified[first_lock_index]
                .locked_prepare_qc
                .as_ref()
                .is_some_and(|qc| qc.reference == first_reference)
                && second_certified[first_lock_index]
                    .highest_prepare_qc
                    .as_ref()
                    .is_some_and(|qc| qc.reference == first_reference)
                && ignore_count(
                    &second_certified[first_lock_index],
                    SumeragiV2IgnoreReason::UnsafeProposal,
                ) > unsafe_proposal_before
                && !second_certified[first_lock_index]
                    .liveness
                    .outbound_intents
                    .iter()
                    .any(|intent| {
                        intent.kind == SumeragiV2OutboundIntentKind::PrepareVote
                            && intent.round.height == height
                            && intent.round.view == second_view
                            && intent.subject == Some(second_subject)
                    }),
            "the A-locked validator did not explicitly reject B under the safe-value rule: before={unsafe_proposal_before}, after={:?}",
            second_certified[first_lock_index],
        );

        let second_certificate_senders = second_group
            .iter()
            .map(|index| peer_ids[*index].clone())
            .collect::<BTreeSet<_>>();
        let unlocked_certificate_signers = unlocked_validator_indices
            .iter()
            .copied()
            .collect::<Vec<_>>();
        let _second_certificate_evidence = wait_for_control_selection(
            &peers[first_lock_index],
            "a B PrepareQC carrying the exact three unlocked signer indices",
            STATUS_TIMEOUT,
            |ack| {
                held_prepare_certificate_sequences(
                    ack,
                    height,
                    second_view,
                    &second_certificate_senders,
                    &second_subject,
                    &second_execution_commitment,
                    &unlocked_certificate_signers,
                    1,
                )
            },
        )
        .await?;

        release_exact_control_sequences(
            &peers[first_qc_observer],
            &expected_rules[first_qc_observer],
            &old_certificate_release,
            "the stale view-zero PrepareQC",
            CONTROL_TIMEOUT,
        )
        .await?;

        let divergent = wait_for_distinct_prepare_qc_split(
            &peers,
            first_group,
            second_group,
            height,
            first_view,
            second_view,
            STATUS_TIMEOUT,
        )
        .await?;
        validate_v2_status_set(&divergent, VALIDATOR_COUNT)?;
        let qcs = divergent
            .iter()
            .map(|snapshot| {
                snapshot
                    .highest_prepare_qc
                    .as_ref()
                    .expect("split wait requires every highest PrepareQC")
            })
            .collect::<Vec<_>>();
        let split = classify_distinct_prepare_qc_split(
            &qcs,
            first_group,
            second_group,
            height,
            first_view,
            second_view,
        )
        .expect("wait helper returned a malformed distinct-subject split");
        ensure!(
            *split.first == first_reference && *split.second == second_reference,
            "the observed split lost the exact staged QC identities"
        );
        ensure!(
            divergent[first_lock_index]
                .locked_prepare_qc
                .as_ref()
                .is_some_and(|qc| qc.reference == first_reference)
                && divergent[first_qc_observer].locked_prepare_qc.is_none()
                && second_group.iter().all(|index| {
                    divergent[*index]
                        .locked_prepare_qc
                        .as_ref()
                        .is_some_and(|qc| qc.reference == second_reference)
                }),
            "the staged high-QC split did not retain the exact one-A-lock/one-A-observer/two-B-lock geometry: {divergent:?}"
        );
        validate_locked_commit_progress_witness(
            &divergent[first_lock_index],
            &first_reference,
        )?;
        for receiver_index in second_group {
            validate_locked_commit_progress_witness(
                &divergent[receiver_index],
                &second_reference,
            )?;
        }

        let controller_baselines = peers
            .iter()
            .map(|peer| {
                peer.consensus_message_control()
                    .expect("controlled peer")
                    .read_ack()
                    .wrap_err_with(|| format!("read pre-heal ACK from {}", peer.mnemonic()))
            })
            .collect::<Result<Vec<_>>>()?;

        let healed = try_join_all(peers.iter().map(|peer| async move {
            peer.consensus_message_control()
                .expect("controlled peer")
                .heal_and_release_all(CONTROL_TIMEOUT)
                .await
                .wrap_err_with(|| format!("heal and drain {} traffic", peer.mnemonic()))
        }))
        .await?;
        for ((peer, before), ack) in peers.iter().zip(&controller_baselines).zip(&healed) {
            ensure!(
                !ack.draining
                    && ack.drain_fence == Some(ack.revision)
                    && ack.rules.is_empty()
                    && ack.held.is_empty()
                    && ack.release_pending.is_empty()
                    && ack.in_flight.is_none()
                    && !ack.fatal
                    && ack.overflowed == before.overflowed
                    && ack.rejected_commands == before.rejected_commands
                    && ack.dropped == before.dropped,
                "{} did not complete the distinct-subject drain fence without controller loss: before={before:?}, after={ack:?}",
                peer.mnemonic(),
            );
        }

        network
            .ensure_blocks_with(|block_height| block_height.total >= height)
            .await
            .wrap_err("healed distinct-subject validators did not finalize")?;
        let successor = wait_for_exact_applied_successor(&peers, height, STATUS_TIMEOUT).await?;
        validate_v2_status_set(&successor, VALIDATOR_COUNT)?;
        for snapshot in &successor {
            validate_applied_successor_witness(snapshot, height)?;
            ensure!(
                snapshot.last_committed_height == height,
                "validator {} advanced beyond the exact controlled application witness: {snapshot:?}",
                snapshot.peer,
            );
        }
        let committed_hashes = try_join_all(
            peers
                .iter()
                .map(|peer| committed_hash_at_height(peer, height)),
        )
        .await?;
        let expected_hash = second_reference.subject.block_hash.to_string();
        ensure!(
            committed_hashes.iter().all(|hash| hash == &expected_hash),
            "validators did not converge on the higher-view certified subject: expected={expected_hash}, committed={committed_hashes:?}"
        );
        let committed_views =
            try_join_all(peers.iter().map(|peer| committed_view_at_height(peer, height))).await?;
        ensure!(
            committed_views.iter().all(|view| *view == second_view),
            "the finalized B headers did not retain the exact view-one decision: {committed_views:?}"
        );
        let finality_proofs = try_join_all(
            peers
                .iter()
                .map(|peer| fetch_bridge_finality_proof(peer, height)),
        )
        .await?;
        for ((peer, proof), client) in peers
            .iter()
            .zip(&finality_proofs)
            .zip(peers.iter().map(NetworkPeer::client))
        {
            validate_exact_finality_proof(
                peer,
                proof,
                &client.chain,
                height,
                second_view,
                &second_reference,
            )?;
        }
        let frozen_context = &finality_proofs[0].finality_artifact.height_context;
        validate_exact_prepare_signers_against_frozen_context(
            &first_locked[first_lock_index],
            &first_reference,
            &first_qc_signers,
            frozen_context,
        )?;
        for receiver_index in second_group {
            validate_exact_prepare_signers_against_frozen_context(
                &second_certified[receiver_index],
                &second_reference,
                &unlocked_certificate_signers,
                frozen_context,
            )?;
        }
        assert_account_registration_in_exact_block(&peers, height, &second_account).await?;
        wait_for_accounts_visible(
            &peers,
            &[first_account, second_account],
            ACCOUNT_VISIBILITY_TIMEOUT,
        )
        .await?;
        Ok(())
    }
    .await;

    network.shutdown_and_release().await;
    result
}

async fn submit_account(client: Client, account_id: AccountId) -> Result<()> {
    task::spawn_blocking(move || {
        client.submit_blocking(
            Register::account(Account::new(account_id)),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
    })
    .await
    .wrap_err("account-registration task panicked")??;
    Ok(())
}

async fn submit_pressure_account(
    client: Client,
    account_id: AccountId,
    payload_bytes: usize,
) -> Result<()> {
    task::spawn_blocking(move || {
        let instructions = vec![
            InstructionBox::from(Register::account(Account::new(account_id))),
            InstructionBox::from(Log::new(Level::INFO, "X".repeat(payload_bytes))),
        ];
        client.submit_all(
            instructions,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
    })
    .await
    .wrap_err("observer-pressure transaction task panicked")??;
    Ok(())
}

async fn enqueue_account(client: Client, account_id: AccountId) -> Result<()> {
    task::spawn_blocking(move || {
        client.submit(
            Register::account(Account::new(account_id)),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
    })
    .await
    .wrap_err("account-registration enqueue task panicked")??;
    Ok(())
}

fn fixture_account(seed_marker: u8) -> Result<AccountId> {
    let key_pair = KeyPair::try_from_seed(vec![seed_marker; 32], Algorithm::Ed25519)
        .wrap_err("derive deterministic v2-runner test account")?;
    Ok(AccountId::new(key_pair.public_key().clone()))
}

async fn normal_statuses(peers: &[NetworkPeer]) -> Result<Vec<iroha::client::Status>> {
    let mut statuses = Vec::with_capacity(peers.len());
    for peer in peers {
        statuses.push(
            peer.status()
                .await
                .wrap_err_with(|| format!("fetch /status from {}", peer.mnemonic()))?,
        );
    }
    Ok(statuses)
}

async fn wait_for_validator_commit_before_observer_catchup(
    validators: &[NetworkPeer],
    observers: &[NetworkPeer],
    height: u64,
    timeout: Duration,
) -> Result<()> {
    ensure!(
        !validators.is_empty() && !observers.is_empty(),
        "observer-lag witness requires validators and observers"
    );
    let deadline = Instant::now() + timeout;
    loop {
        let validator_statuses = normal_statuses(validators).await?;
        let observer_statuses = normal_statuses(observers).await?;
        let validators_committed = validator_statuses
            .iter()
            .all(|status| status.blocks >= height);
        let lagging_observers = observers
            .iter()
            .zip(&observer_statuses)
            .filter(|(_, status)| status.blocks < height)
            .map(|(peer, status)| (peer.mnemonic().to_owned(), status.blocks))
            .collect::<Vec<_>>();
        if validators_committed && !lagging_observers.is_empty() {
            return Ok(());
        }
        let validator_heights = validators
            .iter()
            .zip(&validator_statuses)
            .map(|(peer, status)| (peer.mnemonic().to_owned(), status.blocks))
            .collect::<Vec<_>>();
        let observer_heights = observers
            .iter()
            .zip(&observer_statuses)
            .map(|(peer, status)| (peer.mnemonic().to_owned(), status.blocks))
            .collect::<Vec<_>>();
        if validators_committed
            && observer_statuses
                .iter()
                .all(|status| status.blocks >= height)
        {
            return Err(eyre!(
                "slow-reader scenario reached height {height} everywhere without observing a validator-before-observer recovery boundary: validators={validator_heights:?}, observers={observer_heights:?}"
            ));
        }
        if Instant::now() >= deadline {
            return Err(eyre!(
                "validators did not commit height {height} ahead of at least one delayed observer within {timeout:?}: validators={validator_heights:?}, observers={observer_heights:?}"
            ));
        }
        sleep(FAST_STATUS_POLL_INTERVAL).await;
    }
}

async fn wait_for_normal_statuses(
    peers: &[NetworkPeer],
    min_blocks: u64,
    timeout: Duration,
) -> Result<Vec<iroha::client::Status>> {
    let deadline = Instant::now() + timeout;
    loop {
        let observation = match normal_statuses(peers).await {
            Ok(statuses) => {
                if statuses.iter().all(|status| status.blocks >= min_blocks) {
                    return Ok(statuses);
                }
                format!(
                    "blocks={:?}",
                    peers
                        .iter()
                        .zip(&statuses)
                        .map(|(peer, status)| (peer.mnemonic(), status.blocks))
                        .collect::<Vec<_>>()
                )
            }
            Err(error) => format!("status error: {error:#}"),
        };
        if Instant::now() >= deadline {
            return Err(eyre!(
                "normal status did not reach block height {min_blocks} on every validator within {timeout:?}: {observation}"
            ));
        }
        sleep(FAST_STATUS_POLL_INTERVAL).await;
    }
}

async fn wait_for_common_awaiting_v2_round(
    peers: &[NetworkPeer],
    min_committed_height: u64,
    timeout: Duration,
) -> Result<Vec<V2StatusSnapshot>> {
    let deadline = Instant::now() + timeout;
    loop {
        let mut snapshots = Vec::with_capacity(peers.len());
        let mut errors = Vec::new();
        for peer in peers {
            match fetch_v2_status(peer).await {
                Ok(snapshot) => snapshots.push(snapshot),
                Err(error) => errors.push(format!("{}: {error:#}", peer.mnemonic())),
            }
        }
        let observation = format!(
            "rounds={:?}, errors={errors:?}",
            snapshots
                .iter()
                .map(|snapshot| (
                    snapshot.peer.clone(),
                    snapshot.height,
                    snapshot.view,
                    snapshot.leader,
                    snapshot.phase.clone(),
                    snapshot.last_committed_height,
                    snapshot.liveness.generation,
                    snapshot
                        .liveness
                        .last_progress
                        .map(|progress| (progress.transition, progress.age_ms)),
                    snapshot.liveness.blocker,
                    snapshot.liveness.no_progress_age_ms,
                ))
                .collect::<Vec<_>>()
        );
        if snapshots.len() == peers.len() {
            validate_v2_status_set(&snapshots, VALIDATOR_COUNT)?;
            let first = &snapshots[0];
            let common_awaiting_round = snapshots.iter().all(|snapshot| {
                snapshot.height == first.height
                    && snapshot.view == first.view
                    && snapshot.leader == first.leader
                    && snapshot.height_context_id == first.height_context_id
                    && snapshot.last_committed_height >= min_committed_height
                    && snapshot.last_committed_height.checked_add(1) == Some(snapshot.height)
                    && status_is_awaiting_proposal(&snapshot.phase)
            });
            if common_awaiting_round {
                return Ok(snapshots);
            }
        }
        if Instant::now() >= deadline {
            return Err(eyre!(
                "validators did not expose a common awaiting-proposal v2 round within {timeout:?}: {observation}"
            ));
        }
        sleep(FAST_STATUS_POLL_INTERVAL).await;
    }
}

async fn wait_for_exact_applied_successor(
    peers: &[NetworkPeer],
    committed_height: u64,
    timeout: Duration,
) -> Result<Vec<V2StatusSnapshot>> {
    let successor_height = committed_height
        .checked_add(1)
        .ok_or_else(|| eyre!("controlled committed height cannot have a successor"))?;
    wait_for_v2_status_condition(
        peers,
        "the exact controlled decision applied before another height committed",
        timeout,
        |snapshots| {
            snapshots.iter().all(|snapshot| {
                snapshot.last_committed_height == committed_height
                    && snapshot.height == successor_height
                    && status_is_awaiting_proposal(&snapshot.phase)
                    && snapshot.body_state == SumeragiV2BodyState::Missing
            })
        },
    )
    .await
}

fn status_is_awaiting_proposal(phase: &Value) -> bool {
    phase
        .as_str()
        .or_else(|| {
            phase
                .as_object()
                .and_then(|object| object.get("phase"))
                .and_then(Value::as_str)
        })
        .is_some_and(|tag| {
            tag.eq_ignore_ascii_case("AwaitingProposal")
                || tag.eq_ignore_ascii_case("awaiting_proposal")
        })
}

fn status_round_is_open(snapshot: &V2StatusSnapshot, height: u64, view: u64) -> bool {
    snapshot.height == height
        && snapshot.view == view
        && snapshot
            .last_timeout_certificate
            .as_ref()
            .is_none_or(|timeout| timeout.round.height != height || timeout.round.view != view)
        && !snapshot.liveness.outbound_intents.iter().any(|intent| {
            intent.kind == SumeragiV2OutboundIntentKind::TimeoutVote
                && intent.round.height == height
                && intent.round.view == view
        })
        && !snapshot.liveness.timeout_quorums.iter().any(|quorum| {
            quorum.round.height == height
                && quorum.round.view == view
                && (quorum.signer_count > 0 || quorum.certificate_formed)
        })
}

fn validate_open_round(snapshots: &[V2StatusSnapshot], height: u64, view: u64) -> Result<()> {
    ensure!(
        snapshots
            .iter()
            .all(|snapshot| status_round_is_open(snapshot, height, view)),
        "controlled round {height}/{view} closed before the causal A release: {:?}",
        snapshots
            .iter()
            .map(|snapshot| (
                snapshot.peer.clone(),
                snapshot.height,
                snapshot.view,
                snapshot.last_timeout_certificate,
                snapshot
                    .liveness
                    .timeout_quorums
                    .iter()
                    .filter(|quorum| quorum.round.height == height && quorum.round.view == view)
                    .copied()
                    .collect::<Vec<_>>(),
                snapshot
                    .liveness
                    .outbound_intents
                    .iter()
                    .filter(|intent| {
                        intent.kind == SumeragiV2OutboundIntentKind::TimeoutVote
                            && intent.round.height == height
                            && intent.round.view == view
                    })
                    .copied()
                    .collect::<Vec<_>>(),
                ignore_count(snapshot, SumeragiV2IgnoreReason::ViewClosed),
            ))
            .collect::<Vec<_>>(),
    );
    Ok(())
}

fn ignore_count(snapshot: &V2StatusSnapshot, reason: SumeragiV2IgnoreReason) -> u64 {
    snapshot
        .liveness
        .ignore_counts
        .iter()
        .find(|entry| entry.reason == reason)
        .map_or(0, |entry| entry.count)
}

fn peer_index_for_validator(peers: &[NetworkPeer], validator: u64) -> Result<usize> {
    let mut roster = peers
        .iter()
        .enumerate()
        .map(|(index, peer)| (peer.id(), index))
        .collect::<Vec<_>>();
    roster.sort_by(|left, right| left.0.cmp(&right.0));
    let validator = usize::try_from(validator).wrap_err("validator index does not fit usize")?;
    roster
        .get(validator)
        .map(|(_, network_index)| *network_index)
        .ok_or_else(|| {
            eyre!(
                "validator index {validator} is outside the {}-peer frozen roster",
                roster.len()
            )
        })
}

fn validator_indices_by_peer(peers: &[NetworkPeer]) -> Result<BTreeMap<PeerId, ValidatorIndex>> {
    let mut roster = peers.iter().map(NetworkPeer::id).collect::<Vec<_>>();
    roster.sort();
    roster
        .into_iter()
        .enumerate()
        .map(|(index, peer)| {
            Ok((
                peer,
                ValidatorIndex::try_from(index)
                    .wrap_err("frozen validator index does not fit the wire type")?,
            ))
        })
        .collect()
}

async fn committed_view_at_height(peer: &NetworkPeer, height: u64) -> Result<u64> {
    committed_block_metadata_at_height(peer, height)
        .await
        .map(|(view, _)| view)
}

async fn committed_hash_at_height(peer: &NetworkPeer, height: u64) -> Result<String> {
    committed_block_metadata_at_height(peer, height)
        .await
        .map(|(_, hash)| hash)
}

async fn committed_block_metadata_at_height(
    peer: &NetworkPeer,
    height: u64,
) -> Result<(u64, String)> {
    let client = peer.client();
    let peer_name = peer.mnemonic().to_owned();
    task::spawn_blocking(move || {
        let blocks = client
            .query(FindBlocks)
            .execute_all()
            .wrap_err_with(|| format!("query blocks from {peer_name}"))?;
        blocks
            .iter()
            .find(|block| block.header().height().get() == height)
            .map(|block| (block.header().view_change_index(), block.hash().to_string()))
            .ok_or_else(|| eyre!("{peer_name} has no committed block at height {height}"))
    })
    .await
    .wrap_err_with(|| format!("block-metadata query panicked for {}", peer.mnemonic()))?
}

async fn wait_for_committed_block_metadata(
    peers: &[NetworkPeer],
    height: u64,
    timeout: Duration,
) -> Result<Vec<(u64, String)>> {
    let deadline = Instant::now() + timeout;
    loop {
        let mut committed = Vec::with_capacity(peers.len());
        let mut errors = Vec::new();
        for peer in peers {
            match committed_block_metadata_at_height(peer, height).await {
                Ok((view, hash)) => {
                    committed.push((peer.mnemonic().to_owned(), view, hash));
                }
                Err(error) => errors.push(format!("{}: {error:#}", peer.mnemonic())),
            }
        }
        if committed.len() == peers.len() {
            return Ok(committed
                .into_iter()
                .map(|(_, view, hash)| (view, hash))
                .collect());
        }
        if Instant::now() >= deadline {
            return Err(eyre!(
                "not every validator exposed committed block {height} within {timeout:?}: committed={committed:?}, errors={errors:?}"
            ));
        }
        sleep(FAST_STATUS_POLL_INTERVAL).await;
    }
}

async fn assert_account_registration_in_exact_block(
    peers: &[NetworkPeer],
    height: u64,
    account_id: &AccountId,
) -> Result<()> {
    try_join_all(peers.iter().map(|peer| {
        let client = peer.client();
        let peer_name = peer.mnemonic().to_owned();
        let account_id = account_id.clone();
        async move {
            task::spawn_blocking(move || {
                let blocks = client
                    .query(FindBlocks)
                    .execute_all()
                    .wrap_err_with(|| format!("query blocks from {peer_name}"))?;
                let block = blocks
                    .iter()
                    .find(|block| block.header().height().get() == height)
                    .ok_or_else(|| eyre!("{peer_name} has no committed block at height {height}"))?;
                let registered = block.external_transactions().any(|transaction| {
                    let Executable::Instructions(instructions) = transaction.instructions() else {
                        return false;
                    };
                    instructions.iter().any(|instruction| {
                        matches!(
                            instruction.as_any().downcast_ref::<RegisterBox>(),
                            Some(RegisterBox::Account(register))
                                if register.object().id() == &account_id
                        )
                    })
                });
                ensure!(
                    registered,
                    "{peer_name} height-{height} block does not contain the unique subject-B account registration {account_id}"
                );
                Ok::<(), eyre::Report>(())
            })
            .await
            .wrap_err("exact block application query panicked")?
        }
    }))
    .await?;
    Ok(())
}

async fn fetch_bridge_finality_proof(
    peer: &NetworkPeer,
    height: u64,
) -> Result<BridgeFinalityProof> {
    let client = peer.client();
    let url = client
        .torii_url
        .join(&format!("v1/bridge/finality/{height}"))
        .wrap_err("construct bridge-finality URL")?;
    let response = reqwest::Client::builder()
        .timeout(client.torii_request_timeout)
        .build()
        .wrap_err("build bridge-finality HTTP client")?
        .get(url)
        .header(reqwest::header::ACCEPT, "application/json")
        .send()
        .await
        .wrap_err_with(|| {
            format!(
                "fetch height-{height} finality proof from {}",
                peer.mnemonic()
            )
        })?;
    let status = response.status();
    let bytes = response
        .bytes()
        .await
        .wrap_err_with(|| format!("read finality proof from {}", peer.mnemonic()))?;
    ensure!(
        status.is_success(),
        "{} returned HTTP {status} for height-{height} finality proof: {}",
        peer.mnemonic(),
        String::from_utf8_lossy(&bytes),
    );
    norito::json::from_slice::<BridgeFinalityProof>(&bytes).wrap_err_with(|| {
        format!(
            "{} returned malformed height-{height} bridge finality JSON",
            peer.mnemonic()
        )
    })
}

fn validate_exact_finality_proof(
    peer: &NetworkPeer,
    proof: &BridgeFinalityProof,
    chain: &iroha::data_model::ChainId,
    height: u64,
    view: u64,
    expected_prepare: &QuorumCertificateRef,
) -> Result<()> {
    verify_bridge_finality_proof(proof, chain).wrap_err_with(|| {
        format!(
            "{} returned a cryptographically invalid height-{height} finality proof",
            peer.mnemonic()
        )
    })?;
    let artifact = &proof.finality_artifact;
    let commit_qc = &artifact.commit_qc;
    ensure!(
        artifact.height == height
            && artifact.context_id() == expected_prepare.round.context_id
            && artifact.subject == expected_prepare.subject
            && artifact.block_hash == expected_prepare.subject.block_hash
            && proof.block_header.hash() == artifact.block_hash
            && proof.block_header.height().get() == height
            && proof.block_header.view_change_index() == view
            && commit_qc.round == expected_prepare.round
            && commit_qc.phase == GlobalPhase::Commit
            && commit_qc.subject == expected_prepare.subject
            && commit_qc.execution_commitment == expected_prepare.execution_commitment,
        "{} returned finality evidence for a different height/view/context/value: expected={expected_prepare:?}, proof={proof:?}",
        peer.mnemonic(),
    );
    let signer_count = u32::try_from(commit_qc.signers.len())
        .wrap_err("CommitQC signer count does not fit the wire type")?;
    let signed_power = commit_qc.signers.iter().try_fold(0_u64, |power, signer| {
        let index = usize::try_from(*signer).wrap_err("CommitQC signer does not fit usize")?;
        let signer_power = artifact
            .height_context
            .roster
            .get(index)
            .ok_or_else(|| eyre!("CommitQC signer {signer} is outside the finality roster"))?
            .power;
        power
            .checked_add(signer_power)
            .ok_or_else(|| eyre!("CommitQC signed power overflowed"))
    })?;
    ensure!(
        strict_dual_quorum(
            signer_count,
            artifact.height_context.quorum.min_signers,
            signed_power,
            artifact.height_context.quorum.total_power,
        ),
        "{} returned a finality artifact without the exact equal-vote Commit quorum: signers={:?}, signed_power={signed_power}, quorum={:?}",
        peer.mnemonic(),
        commit_qc.signers,
        artifact.height_context.quorum,
    );
    Ok(())
}

fn validate_exact_prepare_signers_against_frozen_context(
    snapshot: &V2StatusSnapshot,
    expected: &QuorumCertificateRef,
    exact_signers: &[ValidatorIndex],
    context: &iroha::data_model::block::consensus_v2::HeightContext,
) -> Result<()> {
    ensure!(
        context.id() == expected.round.context_id && context.height == expected.round.height,
        "frozen height context does not govern the expected PrepareQC: context={:?}, expected={expected:?}",
        context.id(),
    );
    ensure!(
        !exact_signers.is_empty() && !exact_signers.windows(2).any(|pair| pair[0] >= pair[1]),
        "exact PrepareQC signer evidence is empty, duplicated, or reordered: {exact_signers:?}"
    );
    let signer_count = u32::try_from(exact_signers.len())
        .wrap_err("PrepareQC signer count does not fit the wire type")?;
    let signed_power = exact_signers.iter().try_fold(0_u64, |power, signer| {
        let index = usize::try_from(*signer).wrap_err("PrepareQC signer does not fit usize")?;
        let signer_power = context
            .roster
            .get(index)
            .ok_or_else(|| eyre!("PrepareQC signer {signer} is outside the frozen roster"))?
            .power;
        power
            .checked_add(signer_power)
            .ok_or_else(|| eyre!("PrepareQC signed power overflowed"))
    })?;
    ensure!(
        strict_dual_quorum(
            signer_count,
            context.quorum.min_signers,
            signed_power,
            context.quorum.total_power,
        ),
        "exact held Prepare envelopes do not satisfy the frozen equal-vote quorum: signers={exact_signers:?}, signed_power={signed_power}, quorum={:?}",
        context.quorum,
    );
    let matching = snapshot
        .prepare_quorums
        .iter()
        .filter(|quorum| {
            quorum.round == expected.round
                && quorum.proposal_round == expected.proposal_round
                && quorum.subject == expected.subject
                && quorum.execution_commitment == expected.execution_commitment
        })
        .collect::<Vec<_>>();
    ensure!(
        matching.len() == 1,
        "validator {} does not expose one exact Prepare pool for signer reconstruction: {matching:?}",
        snapshot.peer,
    );
    let reported = matching[0];
    ensure!(
        reported.signer_count == signer_count
            && reported.signed_power == signed_power
            && reported.min_signers == context.quorum.min_signers
            && reported.total_power == context.quorum.total_power,
        "validator {} reported Prepare quorum power that differs from the exact held certificate signers: reported={reported:?}, signers={exact_signers:?}, recomputed_power={signed_power}",
        snapshot.peer,
    );
    Ok(())
}

async fn wait_for_control_selection<T>(
    peer: &NetworkPeer,
    description: &str,
    timeout: Duration,
    select: impl Fn(&ConsensusMessageControlAck) -> Option<T>,
) -> Result<T> {
    let control = peer
        .consensus_message_control()
        .ok_or_else(|| eyre!("{} lacks the requested controller", peer.mnemonic()))?;
    let deadline = Instant::now() + timeout;
    loop {
        let observation = match control.read_ack() {
            Ok(ack) => {
                ensure!(!ack.fatal, "{} controller failed closed", peer.mnemonic());
                ensure!(
                    ack.overflowed == 0,
                    "{} hold queue overflowed while waiting for {description}",
                    peer.mnemonic()
                );
                if let Some(selected) = select(&ack) {
                    return Ok(selected);
                }
                format!(
                    "revision={}, held={}, tail={:?}",
                    ack.revision,
                    ack.held.len(),
                    ack.held
                        .iter()
                        .rev()
                        .take(16)
                        .map(|message| (
                            message.sequence,
                            message.sender.clone(),
                            message.authenticated_via.clone(),
                            message.kind,
                            message.height,
                            message.view,
                            message.block_hash,
                            message.subject,
                            message.execution_commitment,
                            message.signer,
                            message.certificate_signers.clone(),
                            message.envelope_digest,
                        ))
                        .collect::<Vec<_>>()
                )
            }
            Err(error) => format!("acknowledgement read failed: {error:#}"),
        };
        if Instant::now() >= deadline {
            return Err(eyre!(
                "{} did not retain {description} within {timeout:?}: {observation}",
                peer.mnemonic()
            ));
        }
        sleep(FAST_STATUS_POLL_INTERVAL).await;
    }
}

async fn release_exact_control_sequences(
    peer: &NetworkPeer,
    rules: &[ConsensusMessageControlRule],
    release: &[u64],
    description: &str,
    timeout: Duration,
) -> Result<ConsensusMessageControlAck> {
    let ack = peer
        .consensus_message_control()
        .ok_or_else(|| eyre!("{} lacks the requested controller", peer.mnemonic()))?
        .apply(rules, release, DISTINCT_PREPARE_QC_QUEUE_CAPACITY, timeout)
        .await
        .wrap_err_with(|| format!("release {description} to {}", peer.mnemonic()))?;
    ensure!(
        ack.rules.as_slice() == rules
            && ack.delivered.as_slice() == release
            && !ack.fatal
            && ack.overflowed == 0,
        "{} did not deliver the exact {description} sequence while retaining its partition: delivered={:?}, expected={release:?}, fatal={}, overflowed={}",
        peer.mnemonic(),
        ack.delivered,
        ack.fatal,
        ack.overflowed,
    );
    Ok(ack)
}

async fn wait_for_v2_status_condition(
    peers: &[NetworkPeer],
    description: &str,
    timeout: Duration,
    predicate: impl Fn(&[V2StatusSnapshot]) -> bool,
) -> Result<Vec<V2StatusSnapshot>> {
    let deadline = Instant::now() + timeout;
    loop {
        let mut snapshots = Vec::with_capacity(peers.len());
        let mut errors = Vec::new();
        for peer in peers {
            match fetch_v2_status(peer).await {
                Ok(snapshot) => snapshots.push(snapshot),
                Err(error) => errors.push(format!("{}: {error:#}", peer.mnemonic())),
            }
        }
        if errors.is_empty() && snapshots.len() == peers.len() && predicate(&snapshots) {
            return Ok(snapshots);
        }
        if Instant::now() >= deadline {
            return Err(eyre!(
                "validators did not expose {description} within {timeout:?}: observed={:?}, errors={errors:?}",
                snapshots
                    .iter()
                    .map(|snapshot| (
                        snapshot.peer.clone(),
                        snapshot.height,
                        snapshot.view,
                        snapshot.leader,
                        snapshot.last_committed_height,
                        snapshot.body_state,
                        snapshot.locked_prepare_qc.as_ref().map(|qc| qc.reference),
                        snapshot.highest_prepare_qc.as_ref().map(|qc| qc.reference),
                        (
                            snapshot.liveness.generation,
                            snapshot
                                .liveness
                                .last_progress
                                .map(|progress| (progress.transition, progress.age_ms)),
                            snapshot.liveness.blocker,
                            snapshot.liveness.no_progress_age_ms,
                            (
                                snapshot.liveness.prepare_quorums.len(),
                                snapshot.liveness.commit_quorums.len(),
                                snapshot.liveness.timeout_quorums.len(),
                            ),
                            snapshot
                                .liveness
                                .outbound_intents
                                .iter()
                                .map(|intent| (intent.kind, intent.round, intent.stage))
                                .collect::<Vec<_>>(),
                        ),
                    ))
                    .collect::<Vec<_>>()
            ));
        }
        sleep(FAST_STATUS_POLL_INTERVAL).await;
    }
}

async fn wait_for_distinct_prepare_qc_split(
    peers: &[NetworkPeer],
    first_group: [usize; 2],
    second_group: [usize; 2],
    height: u64,
    first_view: u64,
    second_view: u64,
    timeout: Duration,
) -> Result<Vec<V2StatusSnapshot>> {
    wait_for_v2_status_condition(
        peers,
        "the exact causally staged 2+2 distinct-subject PrepareQC split",
        timeout,
        |snapshots| {
            if !snapshots.iter().all(|snapshot| {
                snapshot.height == height
                    && snapshot.view == second_view
                    && snapshot.last_committed_height < height
            }) {
                return false;
            }
            let Some(qcs) = snapshots
                .iter()
                .map(|snapshot| snapshot.highest_prepare_qc.as_ref())
                .collect::<Option<Vec<_>>>()
            else {
                return false;
            };
            let Some(split) = classify_distinct_prepare_qc_split(
                &qcs,
                first_group,
                second_group,
                height,
                first_view,
                second_view,
            ) else {
                return false;
            };
            snapshots[first_group[0]]
                .locked_prepare_qc
                .as_ref()
                .is_some_and(|qc| qc.reference == *split.first)
                && snapshots[first_group[1]].locked_prepare_qc.is_none()
                && second_group.iter().all(|index| {
                    snapshots[*index]
                        .locked_prepare_qc
                        .as_ref()
                        .is_some_and(|qc| qc.reference == *split.second)
                })
        },
    )
    .await
}

async fn wait_for_locked_reproposal_prepare_qc_split(
    peers: &[NetworkPeer],
    height: u64,
    first_view: u64,
    second_view: u64,
    timeout: Duration,
) -> Result<Vec<V2StatusSnapshot>> {
    let deadline = Instant::now() + timeout;
    loop {
        let mut snapshots = Vec::with_capacity(peers.len());
        let mut errors = Vec::new();
        for peer in peers {
            match fetch_v2_status(peer).await {
                Ok(snapshot) => snapshots.push(snapshot),
                Err(error) => errors.push(format!("{}: {error:#}", peer.mnemonic())),
            }
        }
        let observation = format!(
            "qcs={:?}, errors={errors:?}",
            snapshots
                .iter()
                .map(|snapshot| (
                    snapshot.peer.clone(),
                    snapshot.height,
                    snapshot.view,
                    snapshot.last_committed_height,
                    snapshot.body_state,
                    snapshot.locked_prepare_qc.as_ref().map(|qc| qc.reference),
                    snapshot.highest_prepare_qc.as_ref().map(|qc| (
                        qc.reference.round.height,
                        qc.reference.round.view,
                        qc.reference.subject,
                        qc.reference,
                    )),
                ))
                .collect::<Vec<_>>()
        );
        if snapshots.len() == peers.len()
            && snapshots.iter().all(|snapshot| {
                snapshot.height == height
                    && snapshot.view == second_view
                    && snapshot.last_committed_height < height
            })
        {
            let qcs = snapshots
                .iter()
                .map(|snapshot| snapshot.highest_prepare_qc.as_ref())
                .collect::<Option<Vec<_>>>();
            let split = qcs.as_ref().and_then(|qcs| {
                classify_locked_reproposal_prepare_qc_split(qcs, height, first_view, second_view)
            });
            if split.is_some_and(|split| {
                snapshots
                    .iter()
                    .enumerate()
                    .all(|(receiver_index, snapshot)| {
                        let expected = if receiver_index < 2 {
                            split.reproposed
                        } else {
                            split.locked
                        };
                        snapshot.body_state == SumeragiV2BodyState::Validated
                            && snapshot
                                .locked_prepare_qc
                                .as_ref()
                                .is_some_and(|qc| qc.reference == *expected)
                    })
            }) {
                return Ok(snapshots);
            }
        }
        if Instant::now() >= deadline {
            return Err(eyre!(
                "real validators did not retain an exact 2+2 PrepareQC reference split for one locked subject at height {height} views {first_view}/{second_view} within {timeout:?}: {observation}"
            ));
        }
        sleep(FAST_STATUS_POLL_INTERVAL).await;
    }
}

async fn wait_for_exact_prepare_qc_reference(
    peers: &[NetworkPeer],
    expected: &QuorumCertificateRef,
    height: u64,
    view: u64,
    timeout: Duration,
) -> Result<Vec<V2StatusSnapshot>> {
    let deadline = Instant::now() + timeout;
    loop {
        let mut snapshots = Vec::with_capacity(peers.len());
        let mut errors = Vec::new();
        for peer in peers {
            match fetch_v2_status(peer).await {
                Ok(snapshot) => snapshots.push(snapshot),
                Err(error) => errors.push(format!("{}: {error:#}", peer.mnemonic())),
            }
        }
        if errors.is_empty()
            && snapshots.len() == peers.len()
            && snapshots.iter().all(|snapshot| {
                snapshot.height == height
                    && snapshot.view == view
                    && snapshot.last_committed_height < height
                    && snapshot
                        .highest_prepare_qc
                        .as_ref()
                        .is_some_and(|qc| qc.reference == *expected)
                    && snapshot
                        .locked_prepare_qc
                        .as_ref()
                        .is_some_and(|qc| qc.reference == *expected)
                    && snapshot.body_state == SumeragiV2BodyState::Validated
            })
        {
            return Ok(snapshots);
        }
        if Instant::now() >= deadline {
            return Err(eyre!(
                "validators did not converge on the exact same-subject PrepareQC reference at height {height} view {view} within {timeout:?}: expected={expected:?}, observed={:?}, errors={errors:?}",
                snapshots
                    .iter()
                    .map(|snapshot| (
                        snapshot.peer.clone(),
                        snapshot.height,
                        snapshot.view,
                        snapshot.last_committed_height,
                        snapshot.body_state,
                        snapshot.locked_prepare_qc.as_ref().map(|qc| qc.reference),
                        snapshot.highest_prepare_qc.as_ref().map(|qc| qc.reference),
                    ))
                    .collect::<Vec<_>>()
            ));
        }
        sleep(FAST_STATUS_POLL_INTERVAL).await;
    }
}

async fn wait_for_held_quorum_evidence(
    peers: &[NetworkPeer],
    height: u64,
    view: u64,
    block_hash: &HashOf<BlockHeader>,
    vote_kind: ConsensusMessageControlKind,
    certificate_kind: ConsensusMessageControlKind,
    timeout: Duration,
) -> Result<Vec<Vec<u64>>> {
    let deadline = Instant::now() + timeout;
    loop {
        let mut releases = Vec::with_capacity(peers.len());
        let mut held = Vec::with_capacity(peers.len());
        let mut errors = Vec::new();
        for peer in peers {
            let Some(control) = peer.consensus_message_control() else {
                errors.push(format!("{}: controller unavailable", peer.mnemonic()));
                continue;
            };
            match control.read_ack() {
                Ok(ack) => {
                    ensure!(!ack.fatal, "{} controller failed closed", peer.mnemonic());
                    ensure!(
                        ack.overflowed == 0,
                        "{} hold queue overflowed before release",
                        peer.mnemonic()
                    );
                    let exact_messages = ack
                        .held
                        .iter()
                        .filter(|message| {
                            message.height == Some(height)
                                && message.view == Some(view)
                                && message.block_hash.as_ref() == Some(block_hash)
                                && matches!(message.kind, kind if kind == vote_kind || kind == certificate_kind)
                        })
                        .collect::<Vec<_>>();
                    let vote_senders = exact_messages
                        .iter()
                        .filter(|message| message.kind == vote_kind)
                        .map(|message| message.sender.clone())
                        .collect::<BTreeSet<_>>()
                        .len();
                    let certificates = exact_messages
                        .iter()
                        .filter(|message| message.kind == certificate_kind)
                        .count();
                    held.push((
                        peer.mnemonic().to_owned(),
                        ack.held.len(),
                        exact_messages.len(),
                        vote_senders,
                        certificates,
                    ));
                    releases.push(held_quorum_evidence_sequences(
                        &ack,
                        height,
                        view,
                        block_hash,
                        vote_kind,
                        certificate_kind,
                    ));
                }
                Err(error) => errors.push(format!("{}: {error:#}", peer.mnemonic())),
            }
        }
        if errors.is_empty()
            && releases.len() == peers.len()
            && releases.iter().all(Option::is_some)
        {
            return Ok(releases.into_iter().flatten().collect());
        }
        if Instant::now() >= deadline {
            return Err(eyre!(
                "controlled receivers did not retain releasable {vote_kind:?}/{certificate_kind:?} quorum evidence for block {block_hash} at height {height} view {view} within {timeout:?}: held=(peer, total, exact, vote_senders, certificates)={held:?}, errors={errors:?}"
            ));
        }
        sleep(FAST_STATUS_POLL_INTERVAL).await;
    }
}

async fn assert_accounts_absent(peers: &[NetworkPeer], accounts: &[AccountId]) -> Result<()> {
    for peer in peers {
        let client = peer.client();
        let peer_name = peer.mnemonic().to_owned();
        let stored = task::spawn_blocking(move || client.query(FindAccounts).execute_all())
            .await
            .wrap_err_with(|| format!("fresh-genesis account query panicked for {peer_name}"))?
            .wrap_err_with(|| format!("query fresh-genesis accounts from {peer_name}"))?;
        for account in accounts {
            let found = stored.iter().any(|stored| stored.id() == account);
            ensure!(
                !found,
                "fresh genesis unexpectedly contained test account {account} on {peer_name}"
            );
        }
    }
    Ok(())
}

async fn wait_for_accounts_visible(
    peers: &[NetworkPeer],
    accounts: &[AccountId],
    timeout: Duration,
) -> Result<()> {
    let deadline = Instant::now() + timeout;
    let mut last_missing = Vec::new();
    loop {
        last_missing.clear();
        for peer in peers {
            for account in accounts {
                let client = peer.client();
                let account = account.clone();
                let expected = account.clone();
                let expected_label = expected.to_string();
                let peer_name = peer.mnemonic().to_owned();
                let visible = task::spawn_blocking(move || {
                    client
                        .query_single(FindAccountById::new(account))
                        .is_ok_and(|stored| stored.id() == &expected)
                })
                .await
                .wrap_err_with(|| format!("account visibility query panicked for {peer_name}"))?;
                if !visible {
                    last_missing.push(format!("{expected_label} on {peer_name}"));
                }
            }
        }
        if last_missing.is_empty() {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(eyre!(
                "accounts did not become visible on every required validator within {timeout:?}: {last_missing:?}"
            ));
        }
        sleep(POLL_INTERVAL).await;
    }
}

async fn wait_for_v2_statuses(
    peers: &[NetworkPeer],
    min_committed_height: u64,
    timeout: Duration,
) -> Result<Vec<V2StatusSnapshot>> {
    let deadline = Instant::now() + timeout;
    loop {
        let mut snapshots = Vec::with_capacity(peers.len());
        let mut errors = Vec::new();
        for peer in peers {
            match fetch_v2_status(peer).await {
                Ok(snapshot) => snapshots.push(snapshot),
                Err(error) => errors.push(format!("{}: {error:#}", peer.mnemonic())),
            }
        }
        let committed = snapshots
            .iter()
            .map(|snapshot| (snapshot.peer.clone(), snapshot.last_committed_height))
            .collect::<Vec<_>>();
        let observation = format!("committed={committed:?}, errors={errors:?}");
        if snapshots.len() == peers.len()
            && snapshots
                .iter()
                .all(|snapshot| snapshot.last_committed_height >= min_committed_height)
        {
            return Ok(snapshots);
        }
        if Instant::now() >= deadline {
            return Err(eyre!(
                "authoritative v2 status did not reach committed height {min_committed_height} on all validators within {timeout:?}: {observation}"
            ));
        }
        sleep(POLL_INTERVAL).await;
    }
}

async fn fetch_v2_status(peer: &NetworkPeer) -> Result<V2StatusSnapshot> {
    let client = peer.client();
    let peer_name = peer.mnemonic().to_owned();
    let value = task::spawn_blocking(move || client.get_sumeragi_status_json())
        .await
        .wrap_err_with(|| format!("v2 status task panicked for {peer_name}"))?
        .wrap_err_with(|| format!("fetch authoritative v2 status from {peer_name}"))?;
    parse_v2_status(peer_name, &value)
}

async fn fetch_v2_status_set(peers: &[NetworkPeer]) -> Result<Vec<V2StatusSnapshot>> {
    try_join_all(peers.iter().map(fetch_v2_status)).await
}

fn parse_v2_status(peer: String, value: &Value) -> Result<V2StatusSnapshot> {
    let typed = norito::json::from_value::<SumeragiV2Status>(value.clone())
        .wrap_err_with(|| format!("v2 status for {peer} is not the canonical typed payload"))?;
    typed
        .validate()
        .wrap_err_with(|| format!("v2 status for {peer} violates structural invariants"))?;
    ensure!(
        !typed.restart_required,
        "validator {peer} entered the consensus fail-stop state"
    );
    let object = value
        .as_object()
        .ok_or_else(|| eyre!("v2 status for {peer} is not a JSON object"))?;
    let required_u64 = |field: &str| {
        object
            .get(field)
            .and_then(Value::as_u64)
            .ok_or_else(|| eyre!("v2 status for {peer} lacks integer field `{field}`"))
    };
    let required_value = |field: &str| {
        let value = object
            .get(field)
            .filter(|value| !value.is_null())
            .cloned()
            .ok_or_else(|| eyre!("v2 status for {peer} lacks field `{field}`"))?;
        Ok::<_, eyre::Report>(value)
    };
    let body_state = norito::json::from_value::<SumeragiV2BodyState>(required_value("body_state")?)
        .wrap_err_with(|| format!("v2 status for {peer} has a malformed body state"))?;
    let height_context_id =
        norito::json::from_value::<HeightContextId>(required_value("height_context_id")?)
            .wrap_err_with(|| format!("v2 status for {peer} has a malformed height context id"))?;
    let height_context = norito::json::from_value::<SumeragiV2HeightContextStatus>(required_value(
        "height_context",
    )?)
    .wrap_err_with(|| format!("v2 status for {peer} has a malformed height context"))?;
    let liveness =
        norito::json::from_value::<SumeragiV2LivenessStatus>(required_value("liveness")?)
            .wrap_err_with(|| format!("v2 status for {peer} has malformed liveness diagnostics"))?;
    let last_commit_qc = object
        .get("last_commit_qc")
        .filter(|value| !value.is_null())
        .cloned()
        .map(norito::json::from_value::<SumeragiV2CommitQcStatus>)
        .transpose()
        .wrap_err_with(|| format!("v2 status for {peer} has a malformed durable CommitQC"))?;

    Ok(V2StatusSnapshot {
        peer: peer.clone(),
        protocol_version: required_u64("protocol_version")?,
        node_fingerprint: required_value("node_fingerprint")?,
        build_fingerprint: required_value("build_fingerprint")?,
        config_fingerprint: required_value("config_fingerprint")?,
        height_context_id,
        height: required_u64("height")?,
        view: required_u64("view")?,
        leader: required_u64("leader")?,
        phase: required_value("phase")?,
        body_state,
        last_timeout_view: optional_timeout_view(object, &peer)?,
        last_timeout_certificate: typed.last_timeout_certificate,
        locked_prepare_qc: optional_prepare_qc(object, &peer, "locked_prepare_qc")?,
        highest_prepare_qc: optional_prepare_qc(object, &peer, "highest_prepare_qc")?,
        last_committed_height: required_u64("last_committed_height")?,
        height_context,
        last_commit_qc,
        prepare_quorums: liveness.prepare_quorums.clone(),
        liveness,
    })
}

fn optional_prepare_qc(
    object: &norito::json::Map,
    peer: &str,
    field: &str,
) -> Result<Option<PrepareQcSnapshot>> {
    let Some(reference) = object.get(field).filter(|value| !value.is_null()).cloned() else {
        return Ok(None);
    };
    let typed = norito::json::from_value::<QuorumCertificateRef>(reference.clone())
        .wrap_err_with(|| format!("v2 status for {peer} has a malformed `{field}` PrepareQC"))?;
    ensure!(
        typed.phase == GlobalPhase::Prepare,
        "v2 status for {peer} exposed a non-Prepare `{field}` PrepareQC"
    );
    Ok(Some(PrepareQcSnapshot { reference: typed }))
}

include!("sumeragi_v2_runner/status_validation_helpers.rs");

fn validate_v2_status_set(
    snapshots: &[V2StatusSnapshot],
    frozen_validator_count: usize,
) -> Result<()> {
    ensure!(!snapshots.is_empty(), "v2 status set must not be empty");
    let expected_protocol = u64::from(PROTOCOL_VERSION);
    let first = &snapshots[0];
    for snapshot in snapshots {
        ensure!(
            snapshot.protocol_version == expected_protocol,
            "{} advertised protocol {}, expected authoritative v2 ({expected_protocol})",
            snapshot.peer,
            snapshot.protocol_version
        );
        ensure!(
            snapshot.height >= snapshot.last_committed_height
                && snapshot.height - snapshot.last_committed_height <= 1,
            "{} reported impossible v2 height relation: active={}, committed={}",
            snapshot.peer,
            snapshot.height,
            snapshot.last_committed_height
        );
        ensure!(
            snapshot.leader < frozen_validator_count as u64,
            "{} reported leader {} outside the frozen {frozen_validator_count}-validator roster",
            snapshot.peer,
            snapshot.leader
        );
        ensure!(
            snapshot.height_context.validator_count
                == u32::try_from(frozen_validator_count)
                    .expect("four-validator test roster fits canonical count")
                && snapshot.height_context.quorum.min_signers
                    == iroha::data_model::block::consensus_v2::DualQuorum::count_threshold(
                        snapshot.height_context.validator_count,
                    )
                    .expect("non-empty frozen roster has a quorum threshold")
                && snapshot.height_context.quorum.total_power > 0,
            "{} reported a malformed frozen equal-vote quorum: {:?}",
            snapshot.peer,
            snapshot.height_context,
        );
        if let Some(timeout_view) = snapshot.last_timeout_view {
            ensure!(
                timeout_view.checked_add(1) == Some(snapshot.view),
                "{} reported current view {} after timeout certificate view {}",
                snapshot.peer,
                snapshot.view,
                timeout_view
            );
        }
        ensure!(
            snapshot.build_fingerprint == first.build_fingerprint,
            "{} disagrees on the v2 build fingerprint",
            snapshot.peer
        );
        ensure!(
            snapshot.config_fingerprint == first.config_fingerprint,
            "{} disagrees on the v2 consensus-config fingerprint",
            snapshot.peer
        );
        ensure!(
            !snapshot.phase.is_null(),
            "{} returned an incomplete v2 reducer status",
            snapshot.peer
        );
    }

    for (index, left) in snapshots.iter().enumerate() {
        for right in &snapshots[index + 1..] {
            ensure!(
                left.node_fingerprint != right.node_fingerprint,
                "{} and {} unexpectedly share a v2 node fingerprint",
                left.peer,
                right.peer
            );
            if left.height == right.height {
                ensure!(
                    left.height_context_id == right.height_context_id,
                    "{} and {} disagree on the immutable context for height {}",
                    left.peer,
                    right.peer,
                    left.height
                );
                ensure!(
                    left.height_context == right.height_context,
                    "{} and {} disagree on the frozen equal-vote context for height {}",
                    left.peer,
                    right.peer,
                    left.height,
                );
                if left.view == right.view {
                    ensure!(
                        left.leader == right.leader,
                        "{} and {} disagree on the leader for height {} view {}",
                        left.peer,
                        right.peer,
                        left.height,
                        left.view
                    );
                }
            }
        }
    }
    Ok(())
}
