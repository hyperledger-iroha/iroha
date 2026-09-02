/// Unit coverage for PrepareQC split classification and message-control evidence.
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
            execution_commitment: ExecutionCommitment::without_offline_cash_top_ups_or_merge_carrier(
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
        manifest_hash: None,
        chunk_index: None,
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
        manifest_hash: None,
        chunk_index: None,
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
        manifest_hash: None,
        chunk_index: None,
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
        manifest_hash: None,
        chunk_index: None,
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
    let split =
        classify_distinct_prepare_qc_split(&qcs, [0, 1], [2, 3], HEIGHT, FIRST_VIEW, SECOND_VIEW)
            .expect("valid distinct-subject split");
    assert_eq!(*split.first, first.reference);
    assert_eq!(*split.second, second.reference);
    assert_ne!(split.first.subject, split.second.subject);
    assert!(
        classify_distinct_prepare_qc_split(&qcs, [0, 1], [1, 2], HEIGHT, FIRST_VIEW, SECOND_VIEW,)
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
    let timeout =
        held_no_high_timeout_vote_selection(&timeout_ack, HEIGHT, FIRST_VIEW, &timeout_allowed, 2)
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
// Restart recovery uses a signed two-second cadence, not the localnet's 333 ms, so four-peer debug genesis validation retains a useful view-zero deadline under shared-CI contention.
include!("restart_timing_test.rs");
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
