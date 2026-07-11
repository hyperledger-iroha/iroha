//! Unit tests for the executable Sumeragi v2 reducer.

use super::*;

fn id(byte: u8) -> ValidatorId {
    ValidatorId::repeat(byte)
}

fn signature(byte: u8) -> OpaqueSignature {
    OpaqueSignature::new(vec![byte; 8])
}

fn context_with_powers(mode: VotingMode, powers: &[u64]) -> HeightContext {
    let roster = powers
        .iter()
        .enumerate()
        .map(|(index, power)| {
            Validator::new(
                id(u8::try_from(index + 1).expect("small fixture roster")),
                VotingPower::new(*power),
            )
        })
        .collect();
    HeightContext::new(
        ContextId::repeat(0x50),
        ChainId::repeat(0x51),
        2,
        Some(CertificateRef::new(
            ContextId::repeat(0x40),
            Round::new(1, 0),
            Phase::Commit,
            Subject::repeat(0x41),
        )),
        7,
        roster,
        mode,
        Digest::repeat(0x52),
        Digest::repeat(0x53),
        Digest::repeat(0x54),
    )
    .expect("valid fixture context")
}

fn context() -> HeightContext {
    context_with_powers(VotingMode::Permissioned, &[1, 1, 1, 1])
}

fn shares(signers: &[u8]) -> Vec<SignatureShare> {
    signers
        .iter()
        .map(|signer| SignatureShare::new(id(*signer), signature(*signer)))
        .collect()
}

fn qc(
    context: &HeightContext,
    view: u64,
    phase: Phase,
    subject: Subject,
    signers: &[u8],
) -> QuorumCertificate {
    QuorumCertificate::new(
        CertificateRef::new(
            context.id(),
            Round::new(context.height(), view),
            phase,
            subject,
        ),
        shares(signers),
    )
}

fn tc_without_high(context: &HeightContext, view: u64, signers: &[u8]) -> TimeoutCertificate {
    TimeoutCertificate::new(
        context.id(),
        Round::new(context.height(), view),
        vec![TimeoutSignatureGroup::new(None, shares(signers))],
    )
}

fn tc_with_high(
    context: &HeightContext,
    view: u64,
    high: QuorumCertificate,
    signers: &[u8],
) -> TimeoutCertificate {
    TimeoutCertificate::new(
        context.id(),
        Round::new(context.height(), view),
        vec![TimeoutSignatureGroup::new(Some(high), shares(signers))],
    )
}

fn proposal(
    context: &HeightContext,
    view: u64,
    subject: Subject,
    justification: ProposalJustification,
) -> SignedProposal {
    SignedProposal::new(
        Proposal::new(
            context.id(),
            Round::new(context.height(), view),
            context.leader(view),
            PayloadManifest::new(subject, Digest::repeat(0x61), Digest::repeat(0x62), 128, 2),
            justification,
        ),
        signature(0xf0),
    )
}

fn only_persist(outcome: StepOutcome) -> WalEntry {
    let effects = outcome.into_effects();
    assert_eq!(effects.len(), 1, "expected one persistence effect");
    let Effect::Persist { entry, .. } = &effects[0] else {
        panic!("expected persistence effect, got {:?}", effects[0]);
    };
    entry.clone()
}

fn acknowledge(reducer: &mut Reducer, entry: &WalEntry) -> StepOutcome {
    reducer
        .step(Event::Persisted {
            tag: reducer.current_tag(),
            id: entry.id(),
        })
        .expect("persistence acknowledgement succeeds")
}

#[test]
fn leader_rotation_reduces_the_full_hashed_seed() {
    let mut leader_seed = [0_u8; 32];
    leader_seed[31] = 3;
    let roster = (1_u8..=4)
        .map(|index| Validator::new(id(index), VotingPower::new(1)))
        .collect();
    let context = HeightContext::new(
        ContextId::repeat(0x50),
        ChainId::repeat(0x51),
        2,
        Some(CertificateRef::new(
            ContextId::repeat(0x40),
            Round::new(1, 0),
            Phase::Commit,
            Subject::repeat(0x41),
        )),
        7,
        roster,
        VotingMode::Permissioned,
        Digest::repeat(0x52),
        Digest::repeat(0x53),
        Digest::new(leader_seed),
    )
    .expect("valid leader fixture");

    assert_eq!(context.leader(0), id(4));
    assert_eq!(context.leader(1), id(1));
    assert_eq!(context.leader(4), id(4));
}

#[test]
fn view_zero_binds_semantic_parent_finality_across_commit_views() {
    let context = context();
    let frozen = context.parent_commit().expect("fixture parent CommitQC");
    let proposal_subject = Subject::repeat(0x64);
    let mut accepts_frozen = Reducer::new(
        context.clone(),
        Some(context.leader(0)),
        Generation::new(31),
    )
    .expect("reducer");
    let accepted = accepts_frozen
        .step(Event::ProposalReceived {
            tag: accepts_frozen.current_tag(),
            proposal: proposal(
                &context,
                0,
                proposal_subject,
                ProposalJustification::ParentCommit(Some(frozen)),
            ),
        })
        .expect("the frozen parent reference is accepted");
    assert!(matches!(accepted.effects(), [Effect::FetchBody { .. }]));

    let equivalent_other_view = CertificateRef::new(
        frozen.context_id(),
        Round::new(frozen.round().height(), frozen.round().view() + 3),
        Phase::Commit,
        frozen.subject(),
    );
    assert!(frozen.same_commit_decision(equivalent_other_view));
    assert!(!frozen.same_commit_decision(CertificateRef::new(
        frozen.context_id(),
        frozen.round(),
        Phase::Prepare,
        frozen.subject(),
    )));
    let mut accepts_other_view = Reducer::new(
        context.clone(),
        Some(context.leader(0)),
        Generation::new(31),
    )
    .expect("reducer");
    let accepted = accepts_other_view
        .step(Event::ProposalReceived {
            tag: accepts_other_view.current_tag(),
            proposal: proposal(
                &context,
                0,
                proposal_subject,
                ProposalJustification::ParentCommit(Some(equivalent_other_view)),
            ),
        })
        .expect("an equivalent parent CommitQC from another view is accepted");
    assert!(matches!(accepted.effects(), [Effect::FetchBody { .. }]));

    let foreign_context = CertificateRef::new(
        ContextId::repeat(0x42),
        equivalent_other_view.round(),
        Phase::Commit,
        frozen.subject(),
    );
    let mut rejects_foreign = Reducer::new(
        context.clone(),
        Some(context.leader(0)),
        Generation::new(31),
    )
    .expect("reducer");
    assert_eq!(
        rejects_foreign.step(Event::ProposalReceived {
            tag: rejects_foreign.current_tag(),
            proposal: proposal(
                &context,
                0,
                proposal_subject,
                ProposalJustification::ParentCommit(Some(foreign_context)),
            ),
        }),
        Err(ReducerError::InvalidProposalJustification)
    );
}

#[test]
fn equivocation_effects_retain_both_exact_authenticated_artifacts() {
    let context = context();
    let parent = context.parent_commit();

    let first_proposal = proposal(
        &context,
        0,
        Subject::repeat(0x70),
        ProposalJustification::ParentCommit(parent),
    );
    let second_proposal = proposal(
        &context,
        0,
        Subject::repeat(0x71),
        ProposalJustification::ParentCommit(parent),
    );
    let mut proposal_reducer =
        Reducer::new(context.clone(), None, Generation::new(70)).expect("proposal reducer");
    proposal_reducer
        .step(Event::ProposalReceived {
            tag: proposal_reducer.current_tag(),
            proposal: first_proposal.clone(),
        })
        .expect("first proposal");
    let outcome = proposal_reducer
        .step(Event::ProposalReceived {
            tag: proposal_reducer.current_tag(),
            proposal: second_proposal.clone(),
        })
        .expect("conflicting proposal");
    assert!(matches!(
        outcome.effects(),
        [Effect::ReportEquivocation {
            evidence: EquivocationEvidence::Proposal { first, second },
        }] if first == &first_proposal && second == &second_proposal
    ));
    let same_proposal_new_signature =
        SignedProposal::new(first_proposal.proposal().clone(), signature(0x7F));
    let duplicate = proposal_reducer
        .step(Event::ProposalReceived {
            tag: proposal_reducer.current_tag(),
            proposal: same_proposal_new_signature,
        })
        .expect("same proposal statement with another signature");
    assert_eq!(
        duplicate.disposition(),
        StepDisposition::Ignored(IgnoreReason::Duplicate)
    );
    assert!(duplicate.effects().is_empty());

    let round = Round::new(context.height(), 0);
    let first_vote = SignedVote::new(
        Vote::new(
            context.id(),
            round,
            Phase::Prepare,
            Subject::repeat(0x72),
            id(2),
        ),
        signature(0x72),
    );
    let second_vote = SignedVote::new(
        Vote::new(
            context.id(),
            round,
            Phase::Prepare,
            Subject::repeat(0x73),
            id(2),
        ),
        signature(0x73),
    );
    let mut vote_reducer =
        Reducer::new(context.clone(), None, Generation::new(71)).expect("vote reducer");
    vote_reducer
        .step(Event::VoteReceived {
            tag: vote_reducer.current_tag(),
            vote: first_vote.clone(),
        })
        .expect("first vote");
    let outcome = vote_reducer
        .step(Event::VoteReceived {
            tag: vote_reducer.current_tag(),
            vote: second_vote.clone(),
        })
        .expect("conflicting vote");
    assert!(matches!(
        outcome.effects(),
        [Effect::ReportEquivocation {
            evidence: EquivocationEvidence::Vote { first, second },
        }] if first == &first_vote && second == &second_vote
    ));
    let duplicate = vote_reducer
        .step(Event::VoteReceived {
            tag: vote_reducer.current_tag(),
            vote: SignedVote::new(first_vote.vote(), signature(0x7E)),
        })
        .expect("same vote statement with another signature");
    assert_eq!(
        duplicate.disposition(),
        StepDisposition::Ignored(IgnoreReason::Duplicate)
    );
    assert!(duplicate.effects().is_empty());

    let high = qc(
        &context,
        0,
        Phase::Prepare,
        Subject::repeat(0x74),
        &[1, 2, 3],
    );
    let first_timeout = SignedTimeoutVote::new(
        TimeoutVote::new(context.id(), round, id(2), None),
        signature(0x74),
    );
    let second_timeout = SignedTimeoutVote::new(
        TimeoutVote::new(context.id(), round, id(2), Some(high.reference())),
        signature(0x75),
    );
    let mut timeout_reducer =
        Reducer::new(context, None, Generation::new(72)).expect("timeout reducer");
    timeout_reducer
        .step(Event::TimeoutVoteReceived {
            tag: timeout_reducer.current_tag(),
            vote: first_timeout.clone(),
        })
        .expect("first timeout vote");
    let outcome = timeout_reducer
        .step(Event::TimeoutVoteReceived {
            tag: timeout_reducer.current_tag(),
            vote: second_timeout.clone(),
        })
        .expect("conflicting timeout vote");
    assert!(matches!(
        outcome.effects(),
        [Effect::ReportEquivocation {
            evidence: EquivocationEvidence::Timeout { first, second },
        }] if first == &first_timeout && second == &second_timeout
    ));
    let duplicate = timeout_reducer
        .step(Event::TimeoutVoteReceived {
            tag: timeout_reducer.current_tag(),
            vote: SignedTimeoutVote::new(first_timeout.vote(), signature(0x7D)),
        })
        .expect("same timeout statement with another signature");
    assert_eq!(
        duplicate.disposition(),
        StepDisposition::Ignored(IgnoreReason::Duplicate)
    );
    assert!(duplicate.effects().is_empty());
}

#[test]
fn local_leader_persists_then_signs_broadcasts_and_prepares() {
    let context = context();
    let leader = context.leader(0);
    let mut reducer = Reducer::new(context.clone(), Some(leader), Generation::new(30)).unwrap();
    let subject = Subject::repeat(0x65);
    let manifest =
        PayloadManifest::new(subject, Digest::repeat(0x66), Digest::repeat(0x67), 256, 4);

    let proposal_entry = only_persist(
        reducer
            .step(Event::LocalProposalReady {
                tag: reducer.current_tag(),
                manifest,
            })
            .unwrap(),
    );
    assert!(matches!(
        proposal_entry.record(),
        WalRecord::ProposalIntent(proposal) if proposal.manifest() == &manifest
    ));
    assert_eq!(
        reducer.body_state(Round::new(context.height(), 0), subject),
        BodyState::Validated
    );

    let sign_proposal = acknowledge(&mut reducer, &proposal_entry);
    assert!(matches!(
        sign_proposal.effects(),
        [Effect::Sign {
            message: SignableMessage::Proposal(proposal),
            ..
        }] if proposal.manifest() == &manifest
    ));

    let signed = reducer
        .step(Event::Signed {
            tag: reducer.current_tag(),
            signature: signature(leader.as_bytes()[0]),
        })
        .unwrap();
    assert!(matches!(
        signed.effects().first(),
        Some(Effect::Broadcast(ConsensusMessageV2::Proposal(proposal)))
            if proposal.proposal().manifest() == &manifest
    ));
    let prepare_entry = signed
        .effects()
        .iter()
        .find_map(|effect| match effect {
            Effect::Persist { entry, .. } => Some(entry.clone()),
            _ => None,
        })
        .expect("local processing requests a Prepare intent");
    assert!(matches!(
        prepare_entry.record(),
        WalRecord::PrepareIntent(vote)
            if vote.phase() == Phase::Prepare && vote.subject() == subject
    ));

    let sign_prepare = acknowledge(&mut reducer, &prepare_entry);
    assert!(matches!(
        sign_prepare.effects(),
        [Effect::Sign {
            message: SignableMessage::Vote(vote),
            ..
        }] if vote.phase() == Phase::Prepare && vote.subject() == subject
    ));
}

#[test]
fn replay_resigns_proposal_with_equivalent_parent_commit_view() {
    let context = context();
    let leader = context.leader(0);
    let frozen_parent = context.parent_commit().expect("fixture parent CommitQC");
    let equivalent_parent = CertificateRef::new(
        frozen_parent.context_id(),
        Round::new(
            frozen_parent.round().height(),
            frozen_parent.round().view() + 2,
        ),
        Phase::Commit,
        frozen_parent.subject(),
    );
    let subject = Subject::repeat(0x68);
    let manifest = PayloadManifest::new(subject, Digest::repeat(0x69), Digest::repeat(0x6a), 64, 1);
    let proposal = Proposal::new(
        context.id(),
        Round::new(context.height(), 0),
        leader,
        manifest,
        ProposalJustification::ParentCommit(Some(equivalent_parent)),
    );
    let entry = WalEntry::new(
        PersistenceId::new(1),
        WalRecord::ProposalIntent(proposal.clone()),
    );
    let foreign_parent = CertificateRef::new(
        ContextId::repeat(0x7f),
        equivalent_parent.round(),
        Phase::Commit,
        equivalent_parent.subject(),
    );
    let foreign_proposal = Proposal::new(
        context.id(),
        Round::new(context.height(), 0),
        leader,
        manifest,
        ProposalJustification::ParentCommit(Some(foreign_parent)),
    );
    let foreign_error = Reducer::recover(
        context.clone(),
        Some(leader),
        Generation::new(31),
        [WalEntry::new(
            PersistenceId::new(1),
            WalRecord::ProposalIntent(foreign_proposal),
        )],
    )
    .expect_err("a foreign parent context must fail WAL replay closed");
    assert_eq!(
        foreign_error,
        ReducerError::Replay(ReplayError::InvalidProposalIntent)
    );

    let mut recovered =
        Reducer::recover(context, Some(leader), Generation::new(31), [entry]).unwrap();

    assert!(matches!(
        recovered.resume_after_replay().as_slice(),
        [Effect::Sign {
            message: SignableMessage::Proposal(value),
            ..
        }] if value == &proposal
    ));
    let completed = recovered
        .step(Event::Signed {
            tag: recovered.current_tag(),
            signature: signature(leader.as_bytes()[0]),
        })
        .unwrap();
    assert!(
        completed
            .effects()
            .iter()
            .any(|effect| matches!(effect, Effect::Broadcast(ConsensusMessageV2::Proposal(_))))
    );
    assert!(completed.effects().iter().any(|effect| matches!(
        effect,
        Effect::Persist { entry, .. }
            if matches!(entry.record(), WalRecord::PrepareIntent(_))
    )));
}

fn begin_proposal_validation(reducer: &mut Reducer, subject: Subject) -> (Round, EventTag) {
    let context = reducer.context().clone();
    let tag = reducer.current_tag();
    let round = Round::new(context.height(), 0);
    let received = reducer
        .step(Event::ProposalReceived {
            tag,
            proposal: proposal(
                &context,
                0,
                subject,
                ProposalJustification::ParentCommit(context.parent_commit()),
            ),
        })
        .expect("proposal accepted");
    assert!(matches!(received.effects(), [Effect::FetchBody { .. }]));
    let available = reducer
        .step(Event::BodyAvailable {
            tag,
            round,
            subject,
        })
        .expect("body accepted");
    assert!(matches!(available.effects(), [Effect::StoreBody { .. }]));
    let stored = reducer
        .step(Event::BodyStored {
            tag,
            round,
            subject,
        })
        .expect("body storage accepted");
    assert!(matches!(stored.effects(), [Effect::ValidateBody { .. }]));
    (round, tag)
}

#[test]
fn prepare_is_persisted_only_after_durable_body_validation() {
    let context = context();
    let mut reducer = Reducer::new(context, Some(id(1)), Generation::new(3)).unwrap();
    let subject = Subject::repeat(0x70);
    let (round, tag) = begin_proposal_validation(&mut reducer, subject);

    assert_eq!(reducer.body_state(round, subject), BodyState::Durable);
    assert!(reducer.durable_state().prepare_intent(round).is_none());
    let entry = only_persist(
        reducer
            .step(Event::ValidationCompleted {
                tag,
                round,
                subject,
                valid: true,
            })
            .unwrap(),
    );
    assert!(matches!(entry.record(), WalRecord::PrepareIntent(_)));
    assert!(reducer.durable_state().prepare_intent(round).is_none());

    let acknowledged = acknowledge(&mut reducer, &entry);
    assert!(matches!(
        acknowledged.effects(),
        [Effect::Sign {
            message: SignableMessage::Vote(vote),
            ..
        }] if vote.phase() == Phase::Prepare
    ));
    assert!(reducer.durable_state().prepare_intent(round).is_some());
}

#[test]
fn stale_generation_completion_is_rejected_after_view_change() {
    let context = context();
    let mut reducer = Reducer::new(context.clone(), Some(id(1)), Generation::new(8)).unwrap();
    let old_tag = reducer.current_tag();
    let timeout = tc_without_high(&context, 0, &[1, 2, 3]);
    let entry = only_persist(
        reducer
            .step(Event::TimeoutCertificateReceived {
                tag: old_tag,
                certificate: timeout,
            })
            .unwrap(),
    );
    let entered = acknowledge(&mut reducer, &entry);
    assert!(matches!(entered.effects(), [Effect::EnterView { .. }]));
    assert_eq!(reducer.current_tag().view(), 1);
    assert_ne!(reducer.current_tag().generation(), old_tag.generation());

    let stale = reducer
        .step(Event::ValidationCompleted {
            tag: old_tag,
            round: Round::new(context.height(), 0),
            subject: Subject::repeat(0x71),
            valid: true,
        })
        .unwrap();
    assert_eq!(
        stale.disposition(),
        StepDisposition::Ignored(IgnoreReason::StaleGeneration)
    );
}

#[test]
fn quorum_requires_both_validator_count_and_voting_power() {
    let context = context_with_powers(VotingMode::Npos, &[7, 1, 1, 1]);
    let count_only = Quorum::calculate(&context, &[id(2), id(3), id(4)]).unwrap();
    assert_eq!(count_only.signer_count(), 3);
    assert!(!count_only.satisfies(&context));

    let dual = Quorum::calculate(&context, &[id(1), id(2), id(3)]).unwrap();
    assert!(dual.satisfies(&context));
    assert!(Quorum::require(&context, &[id(1), id(2), id(3)]).is_ok());
}

#[test]
fn timeout_is_durable_before_signing_and_view_change() {
    let context = context();
    let mut reducer = Reducer::new(context.clone(), Some(id(1)), Generation::new(1)).unwrap();
    let original_tag = reducer.current_tag();
    let timeout_entry = only_persist(
        reducer
            .step(Event::TimeoutElapsed { tag: original_tag })
            .unwrap(),
    );
    assert!(matches!(
        timeout_entry.record(),
        WalRecord::TimeoutIntent(_)
    ));
    assert_eq!(reducer.current_tag().view(), 0);
    let sign = acknowledge(&mut reducer, &timeout_entry);
    assert!(matches!(
        sign.effects(),
        [Effect::Sign {
            message: SignableMessage::TimeoutVote(_),
            ..
        }]
    ));
    assert_eq!(reducer.current_tag().view(), 0);

    reducer
        .step(Event::Signed {
            tag: original_tag,
            signature: signature(1),
        })
        .unwrap();
    for signer in [2_u8, 3] {
        let vote = TimeoutVote::new(
            context.id(),
            Round::new(context.height(), 0),
            id(signer),
            None,
        );
        let outcome = reducer
            .step(Event::TimeoutVoteReceived {
                tag: original_tag,
                vote: SignedTimeoutVote::new(vote, signature(signer)),
            })
            .unwrap();
        if signer == 2 {
            assert!(outcome.effects().is_empty());
        } else {
            let install = only_persist(outcome);
            assert!(matches!(install.record(), WalRecord::InstallTimeout(_)));
            assert_eq!(reducer.current_tag().view(), 0);
            let entered = acknowledge(&mut reducer, &install);
            assert!(
                entered
                    .effects()
                    .iter()
                    .any(|effect| matches!(effect, Effect::EnterView { .. }))
            );
        }
    }
    assert_eq!(reducer.current_tag().view(), 1);
}

#[test]
fn stale_timeout_traffic_cannot_occupy_the_current_view_wal_slot() {
    let context = context();
    let mut reducer = Reducer::new(context.clone(), Some(id(1)), Generation::new(41)).unwrap();
    let stale_tc = tc_without_high(&context, 0, &[1, 2, 3]);
    let install = only_persist(
        reducer
            .step(Event::TimeoutCertificateReceived {
                tag: reducer.current_tag(),
                certificate: stale_tc.clone(),
            })
            .expect("install first timeout certificate"),
    );
    acknowledge(&mut reducer, &install);
    assert_eq!(reducer.current_tag().view(), 1);

    let stale_certificate = reducer
        .step(Event::TimeoutCertificateReceived {
            tag: reducer.current_tag(),
            certificate: stale_tc,
        })
        .expect("stale timeout certificate is safely ignored");
    assert_eq!(
        stale_certificate.disposition(),
        StepDisposition::Ignored(IgnoreReason::Duplicate)
    );
    assert!(stale_certificate.effects().is_empty());

    let old_round = Round::new(context.height(), 0);
    let stale_vote = SignedTimeoutVote::new(
        TimeoutVote::new(context.id(), old_round, id(4), None),
        signature(4),
    );
    let stale_vote = reducer
        .step(Event::TimeoutVoteReceived {
            tag: reducer.current_tag(),
            vote: stale_vote,
        })
        .expect("stale timeout vote is safely ignored");
    assert_eq!(
        stale_vote.disposition(),
        StepDisposition::Ignored(IgnoreReason::IrrelevantView)
    );
    assert!(stale_vote.effects().is_empty());

    let current_timeout = only_persist(
        reducer
            .step(Event::TimeoutElapsed {
                tag: reducer.current_tag(),
            })
            .expect("current view can still reserve the WAL slot"),
    );
    assert!(matches!(
        current_timeout.record(),
        WalRecord::TimeoutIntent(vote) if vote.round().view() == 1
    ));
}

#[test]
fn persisted_timeout_prunes_old_vote_pools_and_rejects_late_individual_votes() {
    let context = context();
    let mut reducer = Reducer::new(context.clone(), Some(id(1)), Generation::new(42)).unwrap();
    let round = Round::new(context.height(), 0);
    let subject = Subject::repeat(0x71);
    let tag = reducer.current_tag();

    reducer
        .step(Event::VoteReceived {
            tag,
            vote: SignedVote::new(
                Vote::new(context.id(), round, Phase::Prepare, subject, id(4)),
                signature(4),
            ),
        })
        .expect("current-view vote enters the bounded pool");
    reducer
        .step(Event::TimeoutVoteReceived {
            tag,
            vote: SignedTimeoutVote::new(
                TimeoutVote::new(context.id(), round, id(4), None),
                signature(4),
            ),
        })
        .expect("current-view timeout vote enters the bounded pool");
    assert_eq!(reducer.volatile_evidence_counts(), (1, 1, 0, 0));

    let install = only_persist(
        reducer
            .step(Event::TimeoutCertificateReceived {
                tag,
                certificate: tc_without_high(&context, 0, &[1, 2, 3]),
            })
            .expect("verified TC starts durable installation"),
    );
    acknowledge(&mut reducer, &install);
    assert_eq!(reducer.current_tag().view(), 1);
    assert_eq!(reducer.volatile_evidence_counts(), (0, 0, 0, 0));

    for phase in [Phase::Prepare, Phase::Commit] {
        let late = reducer
            .step(Event::VoteReceived {
                tag: reducer.current_tag(),
                vote: SignedVote::new(
                    Vote::new(context.id(), round, phase, subject, id(2)),
                    signature(2),
                ),
            })
            .expect("old individual vote is harmless after view advance");
        assert_eq!(
            late.disposition(),
            StepDisposition::Ignored(IgnoreReason::IrrelevantView)
        );
        assert!(late.effects().is_empty());
    }
    assert_eq!(reducer.volatile_evidence_counts(), (0, 0, 0, 0));
}

#[test]
fn durable_timeout_fence_blocks_delayed_prepare_and_commit_votes() {
    let context = context();
    let mut reducer = Reducer::new(context.clone(), Some(id(1)), Generation::new(5)).unwrap();
    let subject = Subject::repeat(0x72);
    let (round, tag) = begin_proposal_validation(&mut reducer, subject);

    let timeout_entry = only_persist(
        reducer
            .step(Event::TimeoutElapsed { tag })
            .expect("timeout begins while validation is outstanding"),
    );
    acknowledge(&mut reducer, &timeout_entry);
    reducer
        .step(Event::Signed {
            tag,
            signature: signature(1),
        })
        .unwrap();

    let delayed_validation = reducer
        .step(Event::ValidationCompleted {
            tag,
            round,
            subject,
            valid: true,
        })
        .unwrap();
    assert_eq!(
        delayed_validation.disposition(),
        StepDisposition::Ignored(IgnoreReason::ViewClosed)
    );
    assert!(reducer.durable_state().prepare_intent(round).is_none());

    let prepare = qc(&context, 0, Phase::Prepare, subject, &[1, 2, 3]);
    let observed = reducer
        .step(Event::QuorumCertificateReceived {
            tag,
            certificate: prepare,
        })
        .unwrap();
    assert!(observed.effects().iter().all(|effect| {
        !matches!(
            effect,
            Effect::Persist {
                entry,
                ..
            } if matches!(entry.record(), WalRecord::LockAndCommit { .. })
        )
    }));
    assert!(reducer.durable_state().commit_intent(round).is_none());
}

#[test]
fn delayed_old_view_commit_qc_still_finalizes_after_timeout_fence() {
    let context = context();
    let mut reducer = Reducer::new(context.clone(), Some(id(1)), Generation::new(5)).unwrap();
    let tag = reducer.current_tag();
    let timeout_entry = only_persist(
        reducer
            .step(Event::TimeoutElapsed { tag })
            .expect("timeout persistence requested"),
    );
    acknowledge(&mut reducer, &timeout_entry);
    reducer
        .step(Event::Signed {
            tag,
            signature: signature(1),
        })
        .unwrap();

    let subject = Subject::repeat(0x73);
    let commit = qc(&context, 0, Phase::Commit, subject, &[1, 2, 3]);
    let decision_entry = only_persist(
        reducer
            .step(Event::QuorumCertificateReceived {
                tag,
                certificate: commit,
            })
            .unwrap(),
    );
    assert!(matches!(decision_entry.record(), WalRecord::Decision(_)));
    let decided = acknowledge(&mut reducer, &decision_entry);
    assert!(decided.effects().iter().any(
        |effect| matches!(effect, Effect::FetchBody { subject: value, .. } if *value == subject)
    ));
    assert_eq!(
        reducer
            .durable_state()
            .decision()
            .map(QuorumCertificate::subject),
        Some(subject)
    );
}

#[test]
fn replay_resigns_prepare_but_timeout_fence_suppresses_old_votes() {
    let context = context();
    let round = Round::new(context.height(), 0);
    let subject = Subject::repeat(0x74);
    let prepare_vote = Vote::new(context.id(), round, Phase::Prepare, subject, id(1));
    let prepare_entry = WalEntry::new(
        PersistenceId::new(1),
        WalRecord::PrepareIntent(prepare_vote),
    );

    let mut prepared = Reducer::recover(
        context.clone(),
        Some(id(1)),
        Generation::new(11),
        [prepare_entry.clone()],
    )
    .unwrap();
    assert!(matches!(
        prepared.resume_after_replay().as_slice(),
        [Effect::Sign {
            message: SignableMessage::Vote(vote),
            ..
        }] if vote.phase() == Phase::Prepare
    ));

    let timeout = TimeoutVote::new(context.id(), round, id(1), None);
    let timeout_entry = WalEntry::new(PersistenceId::new(2), WalRecord::TimeoutIntent(timeout));
    let mut closed = Reducer::recover(
        context,
        Some(id(1)),
        Generation::new(12),
        [prepare_entry, timeout_entry],
    )
    .unwrap();
    let effects = closed.resume_after_replay();
    assert!(matches!(
        effects.as_slice(),
        [Effect::Sign {
            message: SignableMessage::TimeoutVote(_),
            ..
        }]
    ));
}

#[test]
fn replay_rejects_non_contiguous_or_post_timeout_vote_records() {
    let context = context();
    let round = Round::new(context.height(), 0);
    let timeout = TimeoutVote::new(context.id(), round, id(1), None);
    let timeout_entry = WalEntry::new(PersistenceId::new(1), WalRecord::TimeoutIntent(timeout));
    let prepare = Vote::new(
        context.id(),
        round,
        Phase::Prepare,
        Subject::repeat(0x75),
        id(1),
    );
    let late_prepare = WalEntry::new(PersistenceId::new(2), WalRecord::PrepareIntent(prepare));
    assert!(matches!(
        DurableState::replay(&context, Some(id(1)), [timeout_entry, late_prepare]),
        Err(ReplayError::ViewClosed(value)) if value == round
    ));

    let gap = WalEntry::new(PersistenceId::new(2), WalRecord::PrepareIntent(prepare));
    assert!(matches!(
        DurableState::replay(&context, Some(id(1)), [gap]),
        Err(ReplayError::NonContiguousSequence { .. })
    ));
}

#[test]
fn durable_apply_error_is_transactional() {
    let context = context();
    let mut state = DurableState::new(&context);
    let before = state.clone();
    let invalid_vote = Vote::new(
        context.id(),
        Round::new(context.height(), 0),
        Phase::Prepare,
        Subject::repeat(0x74),
        id(2),
    );
    let entry = WalEntry::new(
        PersistenceId::new(1),
        WalRecord::PrepareIntent(invalid_vote),
    );
    assert_eq!(
        state.apply(&context, Some(id(1)), &entry),
        Err(ReplayError::InvalidLocalVote)
    );
    assert_eq!(state, before);
}

#[test]
fn every_wal_boundary_replays_to_a_safe_resumable_state() {
    let context = context();
    let local = context.leader(0);
    let round = Round::new(context.height(), 0);
    let subject = Subject::repeat(0x84);
    let prepare = qc(&context, 0, Phase::Prepare, subject, &[1, 2, 3]);
    let proposal = Proposal::new(
        context.id(),
        round,
        local,
        PayloadManifest::new(subject, Digest::repeat(0x84), Digest::repeat(0x85), 128, 2),
        ProposalJustification::ParentCommit(context.parent_commit()),
    );
    let entries = [
        WalEntry::new(PersistenceId::new(1), WalRecord::ProposalIntent(proposal)),
        WalEntry::new(
            PersistenceId::new(2),
            WalRecord::PrepareIntent(Vote::new(
                context.id(),
                round,
                Phase::Prepare,
                subject,
                local,
            )),
        ),
        WalEntry::new(
            PersistenceId::new(3),
            WalRecord::ObservePrepare(prepare.clone()),
        ),
        WalEntry::new(
            PersistenceId::new(4),
            WalRecord::LockAndCommit {
                prepare: prepare.clone(),
                vote: Vote::new(context.id(), round, Phase::Commit, subject, local),
            },
        ),
        WalEntry::new(
            PersistenceId::new(5),
            WalRecord::TimeoutIntent(TimeoutVote::new(
                context.id(),
                round,
                local,
                Some(prepare.reference()),
            )),
        ),
        WalEntry::new(
            PersistenceId::new(6),
            WalRecord::InstallTimeout(tc_with_high(&context, 0, prepare, &[1, 2, 3])),
        ),
        WalEntry::new(
            PersistenceId::new(7),
            WalRecord::Decision(qc(&context, 0, Phase::Commit, subject, &[1, 2, 3])),
        ),
    ];

    for prefix_len in 0..=entries.len() {
        let mut recovered = Reducer::recover(
            context.clone(),
            Some(local),
            Generation::new(40 + u64::try_from(prefix_len).unwrap()),
            entries[..prefix_len].iter().cloned(),
        )
        .unwrap_or_else(|error| panic!("WAL prefix {prefix_len} must replay: {error}"));
        let effects = recovered.resume_after_replay();
        if prefix_len >= 7 {
            assert!(effects.iter().any(|effect| matches!(
                effect,
                Effect::FetchBody {
                    certificate: Some(_),
                    ..
                }
            )));
        } else if prefix_len == 5 {
            assert!(matches!(
                effects.as_slice(),
                [Effect::Sign {
                    message: SignableMessage::TimeoutVote(_),
                    ..
                }]
            ));
        }
    }
}

#[test]
fn tc_without_local_high_qc_retains_lock_and_rejects_another_subject() {
    let context = context();
    let subject_a = Subject::repeat(0x76);
    let subject_b = Subject::repeat(0x77);
    let prepare_a = qc(&context, 0, Phase::Prepare, subject_a, &[1, 2, 3]);
    let commit_vote = Vote::new(
        context.id(),
        Round::new(context.height(), 0),
        Phase::Commit,
        subject_a,
        id(1),
    );
    let lock = WalEntry::new(
        PersistenceId::new(1),
        WalRecord::LockAndCommit {
            prepare: prepare_a,
            vote: commit_vote,
        },
    );
    // The TC quorum did not observe validator 1's PrepareQC. Installing it
    // must not lower the local lock; the validator's full-QC timeout vote lets
    // the following view disseminate that certificate to the honest quorum.
    let timeout = tc_without_high(&context, 0, &[2, 3, 4]);
    let install = WalEntry::new(
        PersistenceId::new(2),
        WalRecord::InstallTimeout(timeout.clone()),
    );
    let mut reducer = Reducer::recover(
        context.clone(),
        Some(id(1)),
        Generation::new(20),
        [lock, install],
    )
    .unwrap();
    assert_eq!(
        reducer
            .durable_state()
            .locked()
            .map(QuorumCertificate::subject),
        Some(subject_a)
    );

    let unsafe_result = reducer.step(Event::ProposalReceived {
        tag: reducer.current_tag(),
        proposal: proposal(
            &context,
            1,
            subject_b,
            ProposalJustification::Timeout(timeout.clone()),
        ),
    });
    assert!(matches!(unsafe_result, Err(ReducerError::UnsafeProposal)));

    let accepted = reducer
        .step(Event::ProposalReceived {
            tag: reducer.current_tag(),
            proposal: proposal(
                &context,
                1,
                subject_a,
                ProposalJustification::Timeout(timeout),
            ),
        })
        .unwrap();
    assert!(matches!(accepted.effects(), [Effect::FetchBody { .. }]));
}

#[test]
fn replay_accepts_proposal_safely_unlocked_by_a_strictly_higher_prepare_qc() {
    let context = context();
    let subject_a = Subject::repeat(0x7b);
    let subject_b = Subject::repeat(0x7c);
    let local = context.leader(2);
    let lock = WalEntry::new(
        PersistenceId::new(1),
        WalRecord::LockAndCommit {
            prepare: qc(&context, 0, Phase::Prepare, subject_a, &[1, 2, 3]),
            vote: Vote::new(
                context.id(),
                Round::new(context.height(), 0),
                Phase::Commit,
                subject_a,
                local,
            ),
        },
    );
    let enter_view_one = WalEntry::new(
        PersistenceId::new(2),
        WalRecord::InstallTimeout(tc_without_high(&context, 0, &[1, 2, 3])),
    );
    let enter_view_two = WalEntry::new(
        PersistenceId::new(3),
        WalRecord::InstallTimeout(tc_without_high(&context, 1, &[1, 2, 3])),
    );

    // A different valid TC for view 1 can carry evidence learned after the
    // TC that originally advanced this node. The strictly higher PrepareQC is
    // sufficient to release the older lock, and replay must apply the same
    // predicate as the live reducer.
    let higher_prepare = qc(&context, 1, Phase::Prepare, subject_b, &[1, 2, 3]);
    let justification =
        ProposalJustification::Timeout(tc_with_high(&context, 1, higher_prepare, &[1, 2, 3]));
    let proposal = Proposal::new(
        context.id(),
        Round::new(context.height(), 2),
        local,
        PayloadManifest::new(
            subject_b,
            Digest::repeat(0x61),
            Digest::repeat(0x62),
            128,
            2,
        ),
        justification,
    );
    let proposal_intent = WalEntry::new(
        PersistenceId::new(4),
        WalRecord::ProposalIntent(proposal.clone()),
    );

    let mut reducer = Reducer::recover(
        context,
        Some(local),
        Generation::new(22),
        [lock, enter_view_one, enter_view_two, proposal_intent],
    )
    .expect("a strictly higher PrepareQC safely releases the replayed lock");
    assert_eq!(
        reducer.durable_state().proposal_intent(proposal.round()),
        Some(&proposal)
    );
    assert!(matches!(
        reducer.resume_after_replay().as_slice(),
        [Effect::Sign {
            message: SignableMessage::Proposal(value),
            ..
        }] if value == &proposal
    ));
}

#[test]
fn tc_max_preserves_potentially_committable_lock_and_forces_its_subject() {
    let context = context();
    let subject_a = Subject::repeat(0x78);
    let subject_b = Subject::repeat(0x79);
    let prepare_a = qc(&context, 0, Phase::Prepare, subject_a, &[1, 2, 3]);
    let commit_vote = Vote::new(
        context.id(),
        Round::new(context.height(), 0),
        Phase::Commit,
        subject_a,
        id(1),
    );
    let lock = WalEntry::new(
        PersistenceId::new(1),
        WalRecord::LockAndCommit {
            prepare: prepare_a.clone(),
            vote: commit_vote,
        },
    );
    let timeout = tc_with_high(&context, 0, prepare_a, &[1, 2, 3]);
    let install = WalEntry::new(
        PersistenceId::new(2),
        WalRecord::InstallTimeout(timeout.clone()),
    );
    let mut reducer = Reducer::recover(
        context.clone(),
        Some(id(1)),
        Generation::new(21),
        [lock, install],
    )
    .unwrap();
    assert_eq!(
        reducer
            .durable_state()
            .locked()
            .map(QuorumCertificate::subject),
        Some(subject_a)
    );

    let unsafe_result = reducer.step(Event::ProposalReceived {
        tag: reducer.current_tag(),
        proposal: proposal(
            &context,
            1,
            subject_b,
            ProposalJustification::Timeout(timeout.clone()),
        ),
    });
    assert!(matches!(unsafe_result, Err(ReducerError::UnsafeProposal)));

    let accepted = reducer
        .step(Event::ProposalReceived {
            tag: reducer.current_tag(),
            proposal: proposal(
                &context,
                1,
                subject_a,
                ProposalJustification::Timeout(timeout),
            ),
        })
        .unwrap();
    assert!(matches!(accepted.effects(), [Effect::FetchBody { .. }]));
}

#[test]
fn grouped_timeout_certificate_uses_union_for_dual_quorum() {
    let context = context();
    let high = qc(
        &context,
        0,
        Phase::Prepare,
        Subject::repeat(0x7a),
        &[1, 2, 3],
    );
    let certificate = TimeoutCertificate::new(
        context.id(),
        Round::new(context.height(), 1),
        vec![
            TimeoutSignatureGroup::new(None, shares(&[1])),
            TimeoutSignatureGroup::new(Some(high), shares(&[2, 3])),
        ],
    );
    assert!(certificate.validate(&context).is_ok());

    let overlap = TimeoutCertificate::new(
        context.id(),
        Round::new(context.height(), 1),
        vec![
            TimeoutSignatureGroup::new(None, shares(&[1, 2])),
            TimeoutSignatureGroup::new(
                Some(qc(
                    &context,
                    0,
                    Phase::Prepare,
                    Subject::repeat(0x7a),
                    &[1, 2, 3],
                )),
                shares(&[2, 3]),
            ),
        ],
    );
    assert!(matches!(
        overlap.validate(&context),
        Err(QuorumError::OverlappingTimeoutSigner(value)) if value == id(2)
    ));
}

#[test]
fn retransmit_repeats_a_decision_and_its_complete_certified_fetch() {
    let context = context();
    let subject = Subject::repeat(0x7d);
    let decision = qc(&context, 0, Phase::Commit, subject, &[1, 2, 3]);
    let mut reducer = Reducer::new(context, Some(id(1)), Generation::new(30)).unwrap();

    let decision_entry = only_persist(
        reducer
            .step(Event::QuorumCertificateReceived {
                tag: reducer.current_tag(),
                certificate: decision.clone(),
            })
            .unwrap(),
    );
    let initial = acknowledge(&mut reducer, &decision_entry);
    assert!(initial.effects().iter().any(|effect| matches!(
        effect,
        Effect::FetchBody {
            subject: value,
            certificate: Some(certificate),
            ..
        } if *value == subject && certificate == &decision
    )));

    // Model a pre-GST loss by dropping the first fetch. The periodic reducer
    // transition must provide both the full authorization and the decision
    // retransmission without needing the original inbound envelope.
    let retry = reducer
        .step(Event::RetransmitElapsed {
            tag: reducer.current_tag(),
        })
        .unwrap();
    assert!(retry.effects().iter().any(|effect| matches!(
        effect,
        Effect::Broadcast(ConsensusMessageV2::QuorumCertificate(certificate))
            if certificate == &decision
    )));
    assert!(retry.effects().iter().any(|effect| matches!(
        effect,
        Effect::FetchBody {
            subject: value,
            certificate: Some(certificate),
            ..
        } if *value == subject && certificate == &decision
    )));
}

#[test]
fn delayed_lower_prepare_qc_cannot_downgrade_retransmitted_progress() {
    let context = context();
    let high_subject = Subject::repeat(0x7e);
    let old_subject = Subject::repeat(0x7f);
    let mut reducer = Reducer::recover(
        context.clone(),
        Some(id(1)),
        Generation::new(31),
        [
            WalEntry::new(
                PersistenceId::new(1),
                WalRecord::InstallTimeout(tc_without_high(&context, 0, &[1, 2, 3])),
            ),
            WalEntry::new(
                PersistenceId::new(2),
                WalRecord::InstallTimeout(tc_without_high(&context, 1, &[1, 2, 3])),
            ),
        ],
    )
    .expect("recover at view two");
    assert_eq!(reducer.current_tag().view(), 2);

    let higher = qc(&context, 1, Phase::Prepare, high_subject, &[1, 2, 3]);
    let persist_high = only_persist(
        reducer
            .step(Event::QuorumCertificateReceived {
                tag: reducer.current_tag(),
                certificate: higher.clone(),
            })
            .expect("observe high PrepareQC"),
    );
    acknowledge(&mut reducer, &persist_high);

    let older = qc(&context, 0, Phase::Prepare, old_subject, &[1, 2, 3]);
    reducer
        .step(Event::QuorumCertificateReceived {
            tag: reducer.current_tag(),
            certificate: older,
        })
        .expect("an old PrepareQC is valid but cannot regress progress");

    let retry = reducer
        .step(Event::RetransmitElapsed {
            tag: reducer.current_tag(),
        })
        .expect("retransmit cached controls");
    let retained_prepare_qcs = retry
        .effects()
        .iter()
        .filter_map(|effect| match effect {
            Effect::Broadcast(ConsensusMessageV2::QuorumCertificate(certificate))
                if certificate.phase() == Phase::Prepare =>
            {
                Some(certificate)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(retained_prepare_qcs, vec![&higher]);
}

#[test]
fn timeout_certificate_cannot_advance_a_decided_height() {
    let context = context();
    let subject = Subject::repeat(0x8a);
    let decision = qc(&context, 0, Phase::Commit, subject, &[1, 2, 3]);
    let mut reducer =
        Reducer::new(context.clone(), Some(id(4)), Generation::new(40)).expect("reducer");
    let decision_entry = only_persist(
        reducer
            .step(Event::QuorumCertificateReceived {
                tag: reducer.current_tag(),
                certificate: decision,
            })
            .expect("CommitQC starts durable decision"),
    );
    acknowledge(&mut reducer, &decision_entry);

    let before = reducer.clone();
    let delayed_tc = reducer
        .step(Event::TimeoutCertificateReceived {
            tag: reducer.current_tag(),
            certificate: tc_without_high(&context, 0, &[1, 2, 3]),
        })
        .expect("a delayed TC is safely ignored after decision");
    assert_eq!(
        delayed_tc.disposition(),
        StepDisposition::Ignored(IgnoreReason::AlreadyDecided)
    );
    assert!(delayed_tc.effects().is_empty());
    assert_eq!(reducer, before);
}

#[test]
fn height_closes_only_after_apply_and_a_matching_durable_receipt() {
    let context = context();
    let subject = Subject::repeat(0x82);
    let decision = qc(&context, 0, Phase::Commit, subject, &[1, 2, 3]);
    let receipt = DurableCommitReceipt::from_trusted_storage(
        context.id(),
        context.height(),
        subject,
        decision.reference(),
    );
    let mut reducer = Reducer::new(context.clone(), Some(id(1)), Generation::new(33)).unwrap();
    assert!(!reducer.ready_to_finish());
    let decision_entry = only_persist(
        reducer
            .step(Event::QuorumCertificateReceived {
                tag: reducer.current_tag(),
                certificate: decision.clone(),
            })
            .unwrap(),
    );
    acknowledge(&mut reducer, &decision_entry);
    assert!(!reducer.ready_to_finish());
    assert_eq!(
        reducer.clone().finish_height(receipt),
        Err(ReducerError::HeightNotApplied)
    );

    let round = decision.round();
    reducer
        .step(Event::BodyAvailable {
            tag: reducer.current_tag(),
            round,
            subject,
        })
        .unwrap();
    reducer
        .step(Event::BodyStored {
            tag: reducer.current_tag(),
            round,
            subject,
        })
        .unwrap();
    let apply = reducer
        .step(Event::ValidationCompleted {
            tag: reducer.current_tag(),
            round,
            subject,
            valid: true,
        })
        .unwrap();
    assert!(matches!(apply.effects(), [Effect::Apply { .. }]));
    reducer
        .step(Event::ApplicationCompleted {
            tag: reducer.current_tag(),
            subject,
        })
        .unwrap();
    assert!(reducer.ready_to_finish());

    let mismatched = DurableCommitReceipt::from_trusted_storage(
        context.id(),
        context.height(),
        Subject::repeat(0x83),
        decision.reference(),
    );
    assert_eq!(
        reducer.clone().finish_height(mismatched),
        Err(ReducerError::DurableCommitReceiptMismatch)
    );
    let finalized = reducer
        .finish_height(receipt)
        .expect("matching receipt closes height");
    assert_eq!(finalized.context(), &context);
    assert_eq!(finalized.decision(), &decision);
}

#[test]
fn delayed_proposal_is_ignored_and_never_regresses_body_progress() {
    let context = context();
    let subject = Subject::repeat(0x7e);
    let prepare = qc(&context, 0, Phase::Prepare, subject, &[1, 2, 3]);
    let mut reducer = Reducer::new(context.clone(), Some(id(4)), Generation::new(31)).unwrap();

    let observed = reducer
        .step(Event::QuorumCertificateReceived {
            tag: reducer.current_tag(),
            certificate: prepare,
        })
        .unwrap();
    let observe_entry = observed
        .effects()
        .iter()
        .find_map(|effect| match effect {
            Effect::Persist { entry, .. } => Some(entry.clone()),
            _ => None,
        })
        .expect("observing a highest PrepareQC is durable");
    acknowledge(&mut reducer, &observe_entry);
    reducer
        .step(Event::BodyAvailable {
            tag: reducer.current_tag(),
            round: Round::new(context.height(), 0),
            subject,
        })
        .unwrap();
    assert_eq!(
        reducer.body_state(Round::new(context.height(), 0), subject),
        BodyState::Available
    );

    let received = reducer
        .step(Event::ProposalReceived {
            tag: reducer.current_tag(),
            proposal: proposal(
                &context,
                0,
                subject,
                ProposalJustification::ParentCommit(context.parent_commit()),
            ),
        })
        .unwrap();
    assert!(received.effects().is_empty());
    assert_eq!(
        reducer.body_state(Round::new(context.height(), 0), subject),
        BodyState::Available
    );

    let timeout = tc_without_high(&context, 0, &[1, 2, 3]);
    let install = only_persist(
        reducer
            .step(Event::TimeoutCertificateReceived {
                tag: reducer.current_tag(),
                certificate: timeout,
            })
            .unwrap(),
    );
    acknowledge(&mut reducer, &install);
    let old = reducer
        .step(Event::ProposalReceived {
            tag: reducer.current_tag(),
            proposal: proposal(
                &context,
                0,
                subject,
                ProposalJustification::ParentCommit(context.parent_commit()),
            ),
        })
        .unwrap();
    assert_eq!(
        old.disposition(),
        StepDisposition::Ignored(IgnoreReason::IrrelevantView)
    );
}

#[test]
fn reducer_error_is_transactional_for_conflicting_prepare_certificates() {
    let context = context();
    let mut reducer = Reducer::new(context.clone(), Some(id(4)), Generation::new(32)).unwrap();
    let first = qc(
        &context,
        0,
        Phase::Prepare,
        Subject::repeat(0x80),
        &[1, 2, 3],
    );
    let observe = reducer
        .step(Event::QuorumCertificateReceived {
            tag: reducer.current_tag(),
            certificate: first,
        })
        .unwrap();
    let entry = observe
        .effects()
        .iter()
        .find_map(|effect| match effect {
            Effect::Persist { entry, .. } => Some(entry.clone()),
            _ => None,
        })
        .expect("highest PrepareQC is persisted");
    acknowledge(&mut reducer, &entry);

    let before = reducer.clone();
    let conflicting = qc(
        &context,
        0,
        Phase::Prepare,
        Subject::repeat(0x81),
        &[1, 2, 3],
    );
    assert_eq!(
        reducer.step(Event::QuorumCertificateReceived {
            tag: reducer.current_tag(),
            certificate: conflicting,
        }),
        Err(ReducerError::ConflictingPrepareCertificates)
    );
    assert_eq!(reducer, before);
}
