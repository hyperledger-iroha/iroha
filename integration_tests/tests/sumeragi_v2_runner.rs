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
    crypto::{Algorithm, HashOf, KeyPair},
    data_model::{
        Identifiable,
        account::{Account, AccountId},
        block::{
            BlockHeader,
            consensus_v2::{
                GlobalPhase, PROTOCOL_VERSION, QuorumCertificateRef, SumeragiV2BodyState,
            },
        },
        isi::Register,
        parameter::system::SumeragiNposParameters,
        peer::PeerId,
        prelude::FindAccountById,
        query::{
            account::prelude::FindAccounts, block::prelude::FindBlocks, prelude::QueryBuilderExt,
        },
    },
};
use iroha_test_network::{
    ConsensusMessageControlAck, ConsensusMessageControlAction, ConsensusMessageControlKind,
    ConsensusMessageControlRule, NetworkBuilder, NetworkPeer, init_instruction_registry,
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
const STATUS_TIMEOUT: Duration = Duration::from_secs(90);
const ACCOUNT_VISIBILITY_TIMEOUT: Duration = Duration::from_secs(90);
const POLL_INTERVAL: Duration = Duration::from_millis(200);
const FAST_STATUS_POLL_INTERVAL: Duration = Duration::from_millis(25);
const TAIRA_BLOCK_CADENCE: Duration = Duration::from_secs(1);
const TAIRA_RECOVERY_BOUND: Duration = Duration::from_secs(50);

#[derive(Clone, Debug)]
struct V2StatusSnapshot {
    peer: String,
    protocol_version: u64,
    node_fingerprint: Value,
    build_fingerprint: Value,
    config_fingerprint: Value,
    height_context_id: Value,
    height: u64,
    view: u64,
    leader: u64,
    phase: Value,
    body_state: SumeragiV2BodyState,
    last_timeout_view: Option<u64>,
    locked_prepare_qc: Option<PrepareQcSnapshot>,
    highest_prepare_qc: Option<PrepareQcSnapshot>,
    last_committed_height: u64,
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

fn held_distinct_sender_sequences(
    ack: &ConsensusMessageControlAck,
    height: u64,
    view: u64,
    kind: ConsensusMessageControlKind,
    allowed_senders: &BTreeSet<PeerId>,
    block_hash: Option<&HashOf<BlockHeader>>,
    require_no_block_hash: bool,
    required: usize,
) -> Option<Vec<u64>> {
    let mut senders = BTreeSet::new();
    let mut sequences = Vec::with_capacity(required);
    for message in &ack.held {
        if message.height != Some(height)
            || message.view != Some(view)
            || message.kind != kind
            || !allowed_senders.contains(&message.sender)
            || block_hash.is_some_and(|expected| message.block_hash.as_ref() != Some(expected))
            || (require_no_block_hash && message.block_hash.is_some())
            || !senders.insert(message.sender.clone())
        {
            continue;
        }
        sequences.push(message.sequence);
        if sequences.len() == required {
            return Some(sequences);
        }
    }
    None
}

fn held_prepare_vote_subject(
    ack: &ConsensusMessageControlAck,
    height: u64,
    view: u64,
    allowed_senders: &BTreeSet<PeerId>,
    rejected_hash: Option<&HashOf<BlockHeader>>,
    required: usize,
) -> Option<(HashOf<BlockHeader>, Vec<u64>)> {
    let mut subjects = BTreeMap::<HashOf<BlockHeader>, BTreeMap<PeerId, u64>>::new();
    for message in &ack.held {
        if message.height != Some(height)
            || message.view != Some(view)
            || message.kind != ConsensusMessageControlKind::PrepareVote
            || !allowed_senders.contains(&message.sender)
        {
            continue;
        }
        let block_hash = message.block_hash?;
        if rejected_hash == Some(&block_hash) {
            continue;
        }
        subjects
            .entry(block_hash)
            .or_default()
            .entry(message.sender.clone())
            .or_insert(message.sequence);
    }
    subjects.into_iter().find_map(|(block_hash, senders)| {
        (senders.len() >= required).then(|| {
            let mut sequences = senders.into_values().take(required).collect::<Vec<_>>();
            sequences.sort_unstable();
            (block_hash, sequences)
        })
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
                BlockSubject, ConsensusRound, ExecutionCommitment, HeightContext, HeightContextId,
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
                phase: GlobalPhase::Prepare,
                subject: BlockSubject {
                    parent_block_hash: Some(hash_of::<BlockHeader>(0x20)),
                    block_hash: hash_of::<BlockHeader>(subject_seed),
                    payload_hash: hash(subject_seed.wrapping_add(1)),
                },
                execution_commitment: ExecutionCommitment::without_topups(
                    hash(0x30),
                    hash(0x31),
                    hash(0x32),
                    hash(execution_seed),
                ),
            },
        }
    }

    fn classify(snapshots: &[PrepareQcSnapshot]) -> Option<LockedReproposalPrepareQcSplit<'_>> {
        let qcs = snapshots.iter().collect::<Vec<_>>();
        classify_locked_reproposal_prepare_qc_split(&qcs, HEIGHT, FIRST_VIEW, SECOND_VIEW)
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
            delivered: Vec::new(),
            dropped: 0,
            overflowed: 0,
            rejected_commands: 0,
            last_error: None,
            fatal: false,
            draining: false,
            drain_fence: None,
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
        let first_hash = hash_of::<BlockHeader>(0x70);
        let second_hash = hash_of::<BlockHeader>(0x71);
        let held = vec![
            ConsensusMessageControlHeld {
                sequence: 1,
                sender: peer_ids[0].clone(),
                kind: ConsensusMessageControlKind::PrepareVote,
                height: Some(HEIGHT),
                view: Some(FIRST_VIEW),
                block_hash: Some(first_hash),
                size_bytes: 64,
            },
            ConsensusMessageControlHeld {
                sequence: 2,
                sender: peer_ids[0].clone(),
                kind: ConsensusMessageControlKind::PrepareVote,
                height: Some(HEIGHT),
                view: Some(FIRST_VIEW),
                block_hash: Some(first_hash),
                size_bytes: 64,
            },
            ConsensusMessageControlHeld {
                sequence: 3,
                sender: peer_ids[1].clone(),
                kind: ConsensusMessageControlKind::PrepareVote,
                height: Some(HEIGHT),
                view: Some(FIRST_VIEW),
                block_hash: Some(first_hash),
                size_bytes: 64,
            },
            ConsensusMessageControlHeld {
                sequence: 4,
                sender: peer_ids[2].clone(),
                kind: ConsensusMessageControlKind::PrepareVote,
                height: Some(HEIGHT),
                view: Some(FIRST_VIEW),
                block_hash: Some(second_hash),
                size_bytes: 64,
            },
        ];
        let ack = ack(held);
        let allowed = peer_ids.iter().cloned().collect::<BTreeSet<_>>();
        assert_eq!(
            held_distinct_sender_sequences(
                &ack,
                HEIGHT,
                FIRST_VIEW,
                ConsensusMessageControlKind::PrepareVote,
                &allowed,
                Some(&first_hash),
                false,
                2,
            ),
            Some(vec![1, 3])
        );
        assert_eq!(
            held_prepare_vote_subject(&ack, HEIGHT, FIRST_VIEW, &allowed, Some(&second_hash), 2,),
            Some((first_hash, vec![1, 3]))
        );
        assert!(
            held_prepare_vote_subject(&ack, HEIGHT, FIRST_VIEW, &allowed, Some(&first_hash), 2,)
                .is_none()
        );
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

/// A four-voter v2 network must finalize across one validator outage, recover
/// the restarted validator, and keep finalizing with the full roster restored.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn authoritative_v2_finalizes_through_validator_restart() -> Result<()> {
    init_instruction_registry();

    let builder = NetworkBuilder::new()
        .with_peers(VALIDATOR_COUNT)
        .with_auto_populated_trusted_peers()
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
            wait_for_v2_statuses(&all_peers, initial_committed_floor, STATUS_TIMEOUT).await?;
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
            wait_for_v2_statuses(&all_peers, pre_restart_floor, STATUS_TIMEOUT).await?;
        validate_v2_status_set(&pre_restart_v2, VALIDATOR_COUNT)?;

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
        let outage_v2 =
            wait_for_v2_statuses(&remaining_peers, outage_floor, STATUS_TIMEOUT).await?;
        validate_v2_status_set(&outage_v2, VALIDATOR_COUNT)?;

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
            wait_for_v2_statuses(&all_peers, recovered_floor, STATUS_TIMEOUT).await?;
        validate_v2_status_set(&recovered_v2, VALIDATOR_COUNT)?;
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
        let final_v2 = wait_for_v2_statuses(&all_peers, final_floor, STATUS_TIMEOUT).await?;
        validate_v2_status_set(&final_v2, VALIDATOR_COUNT)?;
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

        let recovered = wait_for_v2_statuses(&all_peers, target_height, STATUS_TIMEOUT).await?;
        validate_v2_status_set(&recovered, VALIDATOR_COUNT)?;
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
        ensure!(
            aligned
                .iter()
                .all(|snapshot| snapshot.last_committed_height < target_height),
            "Prepare-only release unexpectedly decided the controlled height"
        );

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
        let committed_views = try_join_all(
            peers
                .iter()
                .map(|peer| committed_view_at_height(peer, target_height)),
        )
        .await?;
        ensure!(
            committed_views.iter().all(|view| *view == second_view),
            "captured Commit release finalized an unexpected view: expected={second_view}, committed={committed_views:?}"
        );
        let committed_hashes = try_join_all(
            peers
                .iter()
                .map(|peer| committed_hash_at_height(peer, target_height)),
        )
        .await?;
        ensure!(
            committed_hashes
                .iter()
                .all(|hash| hash == &canonical_subject_hash),
            "validators did not commit the exact re-proposed locked body: locked={canonical_subject_hash}, committed={committed_hashes:?}"
        );
        for ((((peer, expected), release), released_ack), committed_view) in peers
            .iter()
            .zip(&expected_rules)
            .zip(&commit_releases)
            .zip(&commit_release_acks)
            .zip(&committed_views)
        {
            let live_ack = peer
                .consensus_message_control()
                .expect("controlled peer")
                .read_ack()?;
            ensure!(
                live_ack.revision == released_ack.revision
                    && live_ack.rules.as_slice() == expected.as_slice()
                    && live_ack.delivered.as_slice() == release.as_slice(),
                "{} finalized view {committed_view} only after its partition rules changed or its captured Commit delivery lost identity",
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
                })
            },
        )
        .await?;

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
            .map(|(_, peer_id)| peer_id.clone())
            .collect::<BTreeSet<_>>();
        let (first_block_hash, first_prepare_release) = wait_for_control_selection(
            &peers[first_lock_index],
            "two distinct view-zero Prepare votes for one subject",
            STATUS_TIMEOUT,
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
        release_exact_control_sequences(
            &peers[first_lock_index],
            &expected_rules[first_lock_index],
            &first_prepare_release,
            "view-zero Prepare votes",
            CONTROL_TIMEOUT,
        )
        .await?;

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
        let first_reference = first_locked[first_lock_index]
            .locked_prepare_qc
            .as_ref()
            .expect("wait condition requires the old lock")
            .reference;

        // Queue work directly at the next leader only after subject A is
        // frozen. Its no-high-QC timeout justification must therefore produce
        // a genuinely new proposal subject rather than reload A's bytes.
        enqueue_account(peers[second_leader_index].client(), second_account.clone()).await?;

        let unlocked_senders = unlocked
            .iter()
            .map(|index| peer_ids[*index].clone())
            .collect::<BTreeSet<_>>();
        let mut timeout_releases = Vec::with_capacity(VALIDATOR_COUNT);
        for receiver_index in 0..VALIDATOR_COUNT {
            let allowed = if receiver_index == first_lock_index {
                unlocked_senders.clone()
            } else {
                unlocked
                    .iter()
                    .filter(|index| **index != receiver_index)
                    .map(|index| peer_ids[*index].clone())
                    .collect::<BTreeSet<_>>()
            };
            let release = wait_for_control_selection(
                &peers[receiver_index],
                "two no-high-QC view-zero Timeout votes from the unlocked quorum",
                DISTINCT_PREPARE_QC_VIEW_ZERO_TIMEOUT,
                |ack| {
                    held_distinct_sender_sequences(
                        ack,
                        height,
                        first_view,
                        ConsensusMessageControlKind::TimeoutVote,
                        &allowed,
                        None,
                        true,
                        LOCKED_REPROPOSAL_REMOTE_QUORUM_VOTES,
                    )
                },
            )
            .await?;
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
                        release,
                        "view-zero Timeout votes",
                        CONTROL_TIMEOUT,
                    )
                    .await
                    .map(|_| ())
                }),
        )
        .await?;

        wait_for_v2_status_condition(
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

        let (second_block_hash, second_leader_prepare_release) = wait_for_control_selection(
            &peers[second_leader_index],
            "two unlocked view-one Prepare votes for a subject distinct from the old lock",
            STATUS_TIMEOUT,
            |ack| {
                held_prepare_vote_subject(
                    ack,
                    height,
                    second_view,
                    &unlocked_senders,
                    Some(&first_block_hash),
                    LOCKED_REPROPOSAL_REMOTE_QUORUM_VOTES,
                )
            },
        )
        .await?;
        ensure!(
            second_block_hash != first_block_hash,
            "the no-high-QC view-one leader re-used the old block subject"
        );
        let second_partner_prepare_release = wait_for_control_selection(
            &peers[second_qc_partner],
            "the same two-sender view-one Prepare quorum at the second QC receiver",
            STATUS_TIMEOUT,
            |ack| {
                held_distinct_sender_sequences(
                    ack,
                    height,
                    second_view,
                    ConsensusMessageControlKind::PrepareVote,
                    &unlocked_senders,
                    Some(&second_block_hash),
                    false,
                    LOCKED_REPROPOSAL_REMOTE_QUORUM_VOTES,
                )
            },
        )
        .await?;
        let prepare_releases = [
            (
                &peers[second_leader_index],
                expected_rules[second_leader_index].as_slice(),
                second_leader_prepare_release.as_slice(),
                "view-one Prepare votes at the leader",
            ),
            (
                &peers[second_qc_partner],
                expected_rules[second_qc_partner].as_slice(),
                second_partner_prepare_release.as_slice(),
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
                    snapshots[*index]
                        .highest_prepare_qc
                        .as_ref()
                        .is_some_and(|qc| {
                            qc.reference.round.height == height
                                && qc.reference.round.view == second_view
                                && qc.reference.subject.block_hash == second_block_hash
                        })
                })
            },
        )
        .await?;
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
        ensure!(
            first_reference.subject != second_reference.subject,
            "the staged PrepareQCs did not certify distinct subjects"
        );

        let first_certificate_sender = BTreeSet::from([peer_ids[first_lock_index].clone()]);
        let old_certificate_release = wait_for_control_selection(
            &peers[first_qc_observer],
            "the authenticated view-zero PrepareQC broadcast from its sole receiver",
            STATUS_TIMEOUT,
            |ack| {
                held_distinct_sender_sequences(
                    ack,
                    height,
                    first_view,
                    ConsensusMessageControlKind::PrepareCertificate,
                    &first_certificate_sender,
                    Some(&first_block_hash),
                    false,
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

        let healed = try_join_all(peers.iter().map(|peer| async move {
            peer.consensus_message_control()
                .expect("controlled peer")
                .heal_and_release_all(CONTROL_TIMEOUT)
                .await
                .wrap_err_with(|| format!("heal and drain {} traffic", peer.mnemonic()))
        }))
        .await?;
        for (peer, ack) in peers.iter().zip(&healed) {
            ensure!(
                !ack.draining
                    && ack.drain_fence == Some(ack.revision)
                    && ack.held.is_empty()
                    && ack.release_pending.is_empty()
                    && ack.in_flight.is_none()
                    && !ack.fatal
                    && ack.overflowed == 0,
                "{} did not complete the distinct-subject drain fence",
                peer.mnemonic()
            );
        }

        network
            .ensure_blocks_with(|block_height| block_height.total >= height)
            .await
            .wrap_err("healed distinct-subject validators did not finalize")?;
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
        wait_for_accounts_visible(
            &peers,
            &[first_account, second_account],
            ACCOUNT_VISIBILITY_TIMEOUT,
        )
        .await?;
        let successor = wait_for_common_awaiting_v2_round(&peers, height, STATUS_TIMEOUT).await?;
        validate_v2_status_set(&successor, VALIDATOR_COUNT)?;
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

async fn committed_view_at_height(peer: &NetworkPeer, height: u64) -> Result<u64> {
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
            .map(|block| block.header().view_change_index())
            .ok_or_else(|| eyre!("{peer_name} has no committed block at height {height}"))
    })
    .await
    .wrap_err_with(|| format!("block-view query panicked for {}", peer.mnemonic()))?
}

async fn committed_hash_at_height(peer: &NetworkPeer, height: u64) -> Result<String> {
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
            .map(|block| block.hash().to_string())
            .ok_or_else(|| eyre!("{peer_name} has no committed block at height {height}"))
    })
    .await
    .wrap_err_with(|| format!("block-hash query panicked for {}", peer.mnemonic()))?
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
                            message.kind,
                            message.height,
                            message.view,
                            message.block_hash,
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
                        .is_none_or(|qc| qc.reference == *split.second)
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

fn parse_v2_status(peer: String, value: &Value) -> Result<V2StatusSnapshot> {
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

    Ok(V2StatusSnapshot {
        peer: peer.clone(),
        protocol_version: required_u64("protocol_version")?,
        node_fingerprint: required_value("node_fingerprint")?,
        build_fingerprint: required_value("build_fingerprint")?,
        config_fingerprint: required_value("config_fingerprint")?,
        height_context_id: required_value("height_context_id")?,
        height: required_u64("height")?,
        view: required_u64("view")?,
        leader: required_u64("leader")?,
        phase: required_value("phase")?,
        body_state,
        last_timeout_view: optional_timeout_view(object, &peer)?,
        locked_prepare_qc: optional_prepare_qc(object, &peer, "locked_prepare_qc")?,
        highest_prepare_qc: optional_prepare_qc(object, &peer, "highest_prepare_qc")?,
        last_committed_height: required_u64("last_committed_height")?,
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

fn optional_timeout_view(object: &norito::json::Map, peer: &str) -> Result<Option<u64>> {
    let Some(certificate) = object
        .get("last_timeout_certificate")
        .filter(|value| !value.is_null())
    else {
        return Ok(None);
    };
    let round = certificate
        .as_object()
        .and_then(|certificate| certificate.get("round"))
        .and_then(Value::as_object)
        .ok_or_else(|| eyre!("v2 status for {peer} has a malformed timeout-certificate round"))?;
    let view = round
        .get("view")
        .and_then(Value::as_u64)
        .ok_or_else(|| eyre!("v2 status for {peer} has a timeout certificate without a view"))?;
    Ok(Some(view))
}

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
