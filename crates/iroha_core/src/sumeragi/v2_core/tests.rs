//! Unit tests for the executable Sumeragi v2 reducer.
use super::reducer::{TimeoutPoolSnapshot, VotePoolSnapshot};
use super::*;
fn id(byte: u8) -> ValidatorId {
    ValidatorId::repeat(byte)
}
fn signature(byte: u8) -> OpaqueSignature {
    OpaqueSignature::new(vec![byte; 8])
}
fn try_context_with_powers(
    mode: VotingMode,
    powers: &[u64],
) -> Result<HeightContext, HeightContextError> {
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
        NetworkId::repeat(0x51),
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
        Digest::repeat(0x55),
        Digest::repeat(0x53),
        Digest::repeat(0x54),
    )
}
fn context_with_powers(mode: VotingMode, powers: &[u64]) -> HeightContext {
    try_context_with_powers(mode, powers).expect("valid fixture context")
}
fn context() -> HeightContext {
    context_with_powers(VotingMode::Permissioned, &[1, 1, 1, 1])
}
#[test]
fn snapshot_bootstrap_context_is_explicit_and_cannot_replace_genesis() {
    let roster = (1_u8..=4)
        .map(|validator| Validator::new(id(validator), VotingPower::new(1)))
        .collect::<Vec<_>>();
    let anchored = HeightContext::new_snapshot_bootstrap(
        ContextId::repeat(0x60),
        NetworkId::repeat(0x61),
        42,
        9,
        roster.clone(),
        VotingMode::Permissioned,
        Digest::repeat(0x62),
        Digest::repeat(0x65),
        Digest::repeat(0x63),
        Digest::repeat(0x64),
    )
    .expect("post-snapshot height accepts the explicit constructor");
    assert!(anchored.is_snapshot_bootstrap());
    assert!(anchored.parent_commit().is_none());
    assert!(matches!(
        HeightContext::new_snapshot_bootstrap(
            ContextId::repeat(0x60),
            NetworkId::repeat(0x61),
            1,
            9,
            roster,
            VotingMode::Permissioned,
            Digest::repeat(0x62),
            Digest::repeat(0x65),
            Digest::repeat(0x63),
            Digest::repeat(0x64),
        ),
        Err(HeightContextError::InvalidParentCommit)
    ));
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
fn install_decision(reducer: &mut Reducer, certificate: QuorumCertificate) -> StepOutcome {
    let entry = only_persist(
        reducer
            .step(Event::QuorumCertificateReceived {
                tag: reducer.current_tag(),
                certificate,
            })
            .expect("CommitQC starts durable decision persistence"),
    );
    acknowledge(reducer, &entry)
}
fn resume_after_replay(reducer: &mut Reducer) -> StepOutcome {
    reducer
        .step(Event::ResumeAfterReplay {
            tag: reducer.current_tag(),
        })
        .expect("replay resumption passes the production refinement gate")
}
fn complete_signature(reducer: &mut Reducer, marker: u8) -> StepOutcome {
    reducer
        .step(Event::Signed {
            tag: reducer.current_tag(),
            signature: signature(marker),
        })
        .expect("the recovered durable intent accepts its signature completion")
}
fn assert_signature_frontier(
    reducer: &Reducer,
    awaiting: Option<&SignableMessage>,
    queued: &[SignableMessage],
) {
    assert_eq!(reducer.awaiting_signature(), awaiting);
    let actual = reducer.queued_signatures().cloned().collect::<Vec<_>>();
    assert_eq!(actual.as_slice(), queued);
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
        NetworkId::repeat(0x51),
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
        Digest::repeat(0x55),
        Digest::repeat(0x53),
        Digest::new(leader_seed),
    )
    .expect("valid leader fixture");
    assert_eq!(context.leader(0), id(4));
    assert_eq!(context.leader(1), id(1));
    assert_eq!(context.leader(4), id(4));
}
#[test]
fn height_context_rejects_rosters_above_first_release_bound() {
    let oversized = (1..=super::types::MAX_VOTING_ROSTER_LEN + 1)
        .map(|index| {
            Validator::new(
                id(u8::try_from(index).expect("bounded fixture validator id")),
                VotingPower::new(1),
            )
        })
        .collect();
    let result = HeightContext::new(
        ContextId::repeat(0x50),
        NetworkId::repeat(0x51),
        2,
        Some(CertificateRef::new(
            ContextId::repeat(0x40),
            Round::new(1, 0),
            Phase::Commit,
            Subject::repeat(0x41),
        )),
        7,
        oversized,
        VotingMode::Permissioned,
        Digest::repeat(0x52),
        Digest::repeat(0x55),
        Digest::repeat(0x53),
        Digest::repeat(0x54),
    );
    assert!(matches!(result, Err(HeightContextError::RosterTooLarge)));
}
include!("tests/committee_fallback_and_retransmit.rs");
#[test]
fn height_context_requires_one_same_round_parent_commit_geometry() {
    let roster = context().roster().to_vec();
    let parent_context = ContextId::repeat(0x40);
    let parent_round = Round::new(1, 5);
    let make_context = |proposal_round| {
        HeightContext::new(
            ContextId::repeat(0x50),
            NetworkId::repeat(0x51),
            2,
            Some(CertificateRef::new_with_proposal_round(
                parent_context,
                parent_round,
                proposal_round,
                Phase::Commit,
                Subject::repeat(0x41),
            )),
            7,
            roster.clone(),
            VotingMode::Permissioned,
            Digest::repeat(0x52),
            Digest::repeat(0x55),
            Digest::repeat(0x53),
            Digest::repeat(0x54),
        )
    };
    make_context(parent_round).expect("same-round parent CommitQC is valid");
    assert!(matches!(
        make_context(Round::new(1, 2)),
        Err(HeightContextError::InvalidParentCommit)
    ));
    assert!(matches!(
        make_context(Round::new(2, 2)),
        Err(HeightContextError::InvalidParentCommit)
    ));
    assert!(matches!(
        make_context(Round::new(1, 6)),
        Err(HeightContextError::InvalidParentCommit)
    ));
}
include!("tests/v2_core_view_zero_parent_binding.rs");
#[test]
fn certificate_height_subject_identity_ignores_round_and_phase_only() {
    let context = context();
    let subject = Subject::repeat(0x65);
    let prepare = CertificateRef::new(
        context.id(),
        Round::new(context.height(), 1),
        Phase::Prepare,
        subject,
    );
    let later_commit = CertificateRef::new(
        context.id(),
        Round::new(context.height(), 4),
        Phase::Commit,
        subject,
    );
    assert!(prepare.same_height_subject(later_commit));
    assert!(!prepare.same_height_subject(CertificateRef::new(
        ContextId::repeat(0x66),
        later_commit.round(),
        Phase::Commit,
        subject,
    )));
    assert!(!prepare.same_height_subject(CertificateRef::new(
        context.id(),
        Round::new(context.height() + 1, later_commit.round().view()),
        Phase::Commit,
        subject,
    )));
    assert!(!prepare.same_height_subject(CertificateRef::new(
        context.id(),
        later_commit.round(),
        Phase::Commit,
        Subject::repeat(0x67),
    )));
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
        TimeoutVote::new(context.id(), round, id(2), Some(high)),
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
fn replay_resigns_proposal_with_equivalent_parent_reproposal_round() {
    let context = context();
    let leader = context.leader(0);
    let frozen_parent = context.parent_commit().expect("fixture parent CommitQC");
    let redecided_round = Round::new(
        frozen_parent.round().height(),
        frozen_parent.round().view() + 2,
    );
    let equivalent_parent = CertificateRef::new_with_proposal_round(
        frozen_parent.context_id(),
        redecided_round,
        redecided_round,
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
    let foreign_parent = CertificateRef::new_with_proposal_round(
        ContextId::repeat(0x7f),
        equivalent_parent.round(),
        equivalent_parent.proposal_round(),
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
        resume_after_replay(&mut recovered).effects(),
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
fn proposal_then_prepare_qc_monotonically_upgrades_the_body_fetch() {
    let context = context();
    let subject = Subject::repeat(0x6e);
    let proposal = proposal(
        &context,
        0,
        subject,
        ProposalJustification::ParentCommit(context.parent_commit()),
    );
    let manifest = *proposal.proposal().manifest();
    let prepare = qc(&context, 0, Phase::Prepare, subject, &[1, 2, 3]);
    let mut reducer = Reducer::new(context, Some(id(3)), Generation::new(33)).unwrap();
    let ordinary = reducer
        .step(Event::ProposalReceived {
            tag: reducer.current_tag(),
            proposal,
        })
        .expect("proposal starts ordinary acquisition");
    assert!(matches!(
        ordinary.effects(),
        [Effect::FetchBody {
            manifest: Some(value),
            certificate: None,
            ..
        }] if *value == manifest
    ));
    let upgraded = reducer
        .step(Event::QuorumCertificateReceived {
            tag: reducer.current_tag(),
            certificate: prepare.clone(),
        })
        .expect("PrepareQC upgrades the in-flight acquisition");
    assert!(upgraded.effects().iter().any(|effect| matches!(
        effect,
        Effect::FetchBody {
            manifest: Some(value),
            certified_sources,
            certificate: Some(value_certificate),
            ..
        } if *value == manifest
            && certified_sources == &vec![id(1), id(2), id(3)]
            && value_certificate == &prepare
    )));
}
#[test]
fn equal_prepare_qcs_with_different_quorum_subsets_reuse_first_fetch_authority() {
    let context = context();
    let subject = Subject::repeat(0x7e);
    let first = qc(&context, 0, Phase::Prepare, subject, &[1, 2, 3]);
    let second = qc(&context, 0, Phase::Prepare, subject, &[1, 2, 4]);
    assert_eq!(first.reference(), second.reference());
    assert_ne!(first, second);
    let mut reducer = Reducer::new(context, Some(id(3)), Generation::new(35)).unwrap();
    let started = reducer
        .step(Event::QuorumCertificateReceived {
            tag: reducer.current_tag(),
            certificate: first.clone(),
        })
        .expect("first PrepareQC starts certified acquisition");
    assert!(started.effects().iter().any(|effect| matches!(
        effect,
        Effect::FetchBody {
            certified_sources,
            certificate: Some(value),
            ..
        } if certified_sources == &vec![id(1), id(2), id(3)] && value == &first
    )));
    let persist = started
        .effects()
        .iter()
        .find_map(|effect| match effect {
            Effect::Persist { entry, .. } => Some(entry.clone()),
            _ => None,
        })
        .expect("first PrepareQC observation is durable");
    acknowledge(&mut reducer, &persist);
    let repeated = reducer
        .step(Event::QuorumCertificateReceived {
            tag: reducer.current_tag(),
            certificate: second,
        })
        .expect("same-subject PrepareQC with a different quorum is compatible");
    assert!(matches!(
        repeated.effects(),
        [Effect::FetchBody {
            certified_sources,
            certificate: Some(value),
            ..
        }] if certified_sources == &vec![id(1), id(2), id(3)] && value == &first
    ));
}
#[test]
fn prepare_qc_then_proposal_adds_the_manifest_without_dropping_certification() {
    let context = context();
    let subject = Subject::repeat(0x6f);
    let proposal = proposal(
        &context,
        0,
        subject,
        ProposalJustification::ParentCommit(context.parent_commit()),
    );
    let manifest = *proposal.proposal().manifest();
    let prepare = qc(&context, 0, Phase::Prepare, subject, &[1, 2, 3]);
    let mut reducer = Reducer::new(context, Some(id(3)), Generation::new(34)).unwrap();
    let certified = reducer
        .step(Event::QuorumCertificateReceived {
            tag: reducer.current_tag(),
            certificate: prepare.clone(),
        })
        .expect("PrepareQC starts certified acquisition");
    assert!(certified.effects().iter().any(|effect| matches!(
        effect,
        Effect::FetchBody {
            manifest: None,
            certificate: Some(value_certificate),
            ..
        } if value_certificate == &prepare
    )));
    let persist = certified
        .effects()
        .iter()
        .find_map(|effect| match effect {
            Effect::Persist { entry, .. } => Some(entry.clone()),
            _ => None,
        })
        .expect("PrepareQC observation is durable");
    acknowledge(&mut reducer, &persist);
    let upgraded = reducer
        .step(Event::ProposalReceived {
            tag: reducer.current_tag(),
            proposal,
        })
        .expect("proposal adds the canonical manifest");
    assert!(matches!(
        upgraded.effects(),
        [Effect::FetchBody {
            manifest: Some(value),
            certified_sources,
            certificate: Some(value_certificate),
            ..
        }] if *value == manifest
            && certified_sources == &vec![id(1), id(2), id(3)]
            && value_certificate == &prepare
    ));
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
    assert_eq!(reducer.generation(), reducer.current_tag().generation());
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
    assert!(stale.effects().is_empty());
    // Persistence completions can arrive after their owner has already been
    // retired. Their non-zero identifier is payload, not proof that they may
    // mutate the current reducer incarnation.
    let before_unowned_completion = reducer.clone();
    let unowned_id = PersistenceId::new(77);
    for completion in [
        Event::Persisted {
            tag: reducer.current_tag(),
            id: unowned_id,
        },
        Event::PersistenceFailed {
            tag: reducer.current_tag(),
            id: unowned_id,
        },
    ] {
        let ignored = reducer
            .step(completion)
            .expect("an unowned persistence completion is an accepted stutter");
        assert_eq!(
            ignored.disposition(),
            StepDisposition::Ignored(IgnoreReason::NoMatchingWork)
        );
        assert!(ignored.effects().is_empty());
        assert_eq!(reducer, before_unowned_completion);
    }
    // The stutter rule must not weaken current-owner acknowledgement checks.
    let mut pending = Reducer::new(context, Some(id(1)), Generation::new(9)).unwrap();
    let pending_entry = only_persist(
        pending
            .step(Event::TimeoutElapsed {
                tag: pending.current_tag(),
            })
            .expect("timeout starts WAL persistence"),
    );
    let wrong_id = PersistenceId::new(pending_entry.id().get() + 1);
    assert_eq!(
        pending.step(Event::Persisted {
            tag: pending.current_tag(),
            id: wrong_id,
        }),
        Err(ReducerError::PersistenceAcknowledgementMismatch {
            expected: pending_entry.id(),
            actual: wrong_id,
        })
    );
    assert_eq!(
        pending.step(Event::PersistenceFailed {
            tag: pending.current_tag(),
            id: pending_entry.id(),
        }),
        Err(ReducerError::PersistenceFailed(pending_entry.id()))
    );
}
#[test]
fn queued_timeout_tag_cannot_cross_a_timeout_certificate_view_install() {
    let context = context();
    let mut reducer = Reducer::new(context.clone(), Some(id(1)), Generation::new(8)).unwrap();
    let expired_tag = reducer.current_tag();
    let timeout = tc_without_high(&context, expired_tag.view(), &[1, 2, 3]);
    let entry = only_persist(
        reducer
            .step(Event::TimeoutCertificateReceived {
                tag: expired_tag,
                certificate: timeout,
            })
            .expect("the timeout certificate acquires the persistence slot"),
    );
    let entered = acknowledge(&mut reducer, &entry);
    assert!(matches!(entered.effects(), [Effect::EnterView { .. }]));
    assert_eq!(reducer.current_tag().view(), expired_tag.view() + 1);
    assert_ne!(reducer.current_tag().generation(), expired_tag.generation());
    let after_install = reducer.clone();
    let stale_timeout = reducer
        .step(Event::TimeoutElapsed { tag: expired_tag })
        .expect("the queued pre-install timeout is an accepted stale stutter");
    assert_eq!(
        stale_timeout.disposition(),
        StepDisposition::Ignored(IgnoreReason::StaleGeneration)
    );
    assert!(stale_timeout.effects().is_empty());
    assert_eq!(
        reducer, after_install,
        "a pre-install timeout tag cannot mint a timeout intent in the fresh view"
    );
    assert!(reducer.pending_persistence_record().is_none());
}
#[test]
fn stale_persistence_completions_stutter_while_current_append_is_pending() {
    let context = context();
    let mut reducer = Reducer::new(context.clone(), Some(id(1)), Generation::new(9)).unwrap();
    let pending_entry = only_persist(
        reducer
            .step(Event::TimeoutElapsed {
                tag: reducer.current_tag(),
            })
            .expect("current timeout starts a distinct WAL append"),
    );
    let current_tag = reducer.current_tag();
    let stale_tag = EventTag::new(context.height(), current_tag.view(), Generation::new(8));
    let stale_id = PersistenceId::new(pending_entry.id().get() + 1);
    let before = reducer.clone();
    for id in [stale_id, pending_entry.id()] {
        for completion in [
            Event::Persisted { tag: stale_tag, id },
            Event::PersistenceFailed { tag: stale_tag, id },
        ] {
            let ignored = reducer
                .step(completion)
                .expect("stale persistence completion is an accepted exact stutter");
            assert_eq!(
                ignored.disposition(),
                StepDisposition::Ignored(IgnoreReason::StaleGeneration)
            );
            assert!(ignored.effects().is_empty());
            assert_eq!(
                reducer, before,
                "stale completion must retain the exact pending entry, identifier, and state"
            );
            assert_eq!(
                reducer.pending_persistence_record(),
                Some(pending_entry.record())
            );
        }
    }
}
#[test]
fn npos_context_rejects_stake_weighted_consensus_votes() {
    assert!(matches!(
        try_context_with_powers(VotingMode::Npos, &[7, 1, 1, 1]),
        Err(HeightContextError::VotingPowerNotOne(validator)) if validator == id(1)
    ));
}
#[test]
fn quorum_rejects_duplicate_and_unordered_signers() {
    let context = context();
    assert_eq!(
        Quorum::calculate(&context, &[id(1), id(1), id(2)]),
        Err(QuorumError::SignersNotStrictlyOrdered)
    );
    assert_eq!(
        Quorum::calculate(&context, &[id(2), id(1), id(3)]),
        Err(QuorumError::SignersNotStrictlyOrdered)
    );
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
    assert!(matches!(
        reducer.pending_persistence_record(),
        Some(WalRecord::TimeoutIntent(_))
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
    assert!(matches!(
        reducer.awaiting_signature(),
        Some(SignableMessage::TimeoutVote(_))
    ));
    assert_eq!(reducer.queued_signatures().count(), 0);
    assert_eq!(reducer.current_tag().view(), 0);
    let broadcast = reducer
        .step(Event::Signed {
            tag: original_tag,
            signature: signature(1),
        })
        .unwrap();
    assert!(matches!(
        broadcast.effects(),
        [Effect::Broadcast(ConsensusMessageV2::TimeoutVote(_))]
    ));
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
            assert!(matches!(
                reducer.timeout_pool_snapshots().as_slice(),
                [TimeoutPoolSnapshot {
                    round,
                    signers,
                    signed_power,
                    certificate_formed: false,
                }] if *round == Round::new(context.height(), 0)
                    && signers == &[id(1), id(2)]
                    && *signed_power == VotingPower::new(2)
            ));
        } else {
            let install = only_persist(outcome);
            assert!(
                reducer
                    .timeout_pool_snapshots()
                    .iter()
                    .all(|pool| pool.certificate_formed)
            );
            assert!(matches!(install.record(), WalRecord::InstallTimeout(_)));
            assert_eq!(reducer.current_tag().view(), 0);
            let entered = acknowledge(&mut reducer, &install);
            assert!(matches!(
                entered.effects(),
                [
                    Effect::EnterView {
                        tag,
                        protected_lock: None,
                        ..
                    },
                    Effect::Broadcast(ConsensusMessageV2::TimeoutCertificate(certificate)),
                ] if tag.view() == 1
                    && certificate.round() == Round::new(context.height(), 0)
            ));
        }
    }
    assert_eq!(reducer.current_tag().view(), 1);
    let retransmit = reducer
        .step(Event::RetransmitElapsed {
            tag: reducer.current_tag(),
        })
        .expect("the successor view can retransmit its retained control");
    assert!(retransmit.effects().iter().all(|effect| !matches!(
        effect,
        Effect::Broadcast(ConsensusMessageV2::TimeoutVote(vote))
            if vote.vote().round() == Round::new(context.height(), 0)
    )));
}
#[test]
fn quorum_forming_local_timeout_broadcasts_only_durable_certificate() {
    let context = context();
    let mut reducer = Reducer::new(context.clone(), Some(id(1)), Generation::new(1)).unwrap();
    let tag = reducer.current_tag();
    let round = Round::new(context.height(), tag.view());
    for signer in [2_u8, 3] {
        let retained = reducer
            .step(Event::TimeoutVoteReceived {
                tag,
                vote: SignedTimeoutVote::new(
                    TimeoutVote::new(context.id(), round, id(signer), None),
                    signature(signer),
                ),
            })
            .expect("retain the remote timeout share before local signing");
        assert!(retained.effects().is_empty());
    }
    let timeout_intent = only_persist(
        reducer
            .step(Event::TimeoutElapsed { tag })
            .expect("start the local durable timeout intent"),
    );
    let sign = acknowledge(&mut reducer, &timeout_intent);
    assert!(matches!(
        sign.effects(),
        [Effect::Sign {
            message: SignableMessage::TimeoutVote(_),
            ..
        }]
    ));
    let formed = reducer
        .step(Event::Signed {
            tag,
            signature: signature(1),
        })
        .expect("the local signature forms the timeout certificate");
    let install = match formed.effects() {
        [Effect::Persist { entry, .. }]
            if matches!(entry.record(), WalRecord::InstallTimeout(_)) =>
        {
            entry.clone()
        }
        effects => panic!(
            "quorum-forming local timeout must expose only its durable TC fence: {effects:?}"
        ),
    };
    let entered = acknowledge(&mut reducer, &install);
    assert!(matches!(
        entered.effects(),
        [
            Effect::EnterView {
                tag: entered_tag,
                protected_lock: None,
                ..
            },
            Effect::Broadcast(ConsensusMessageV2::TimeoutCertificate(certificate)),
        ] if entered_tag.view() == tag.view() + 1 && certificate.round() == round
    ));
}
#[test]
fn self_contained_timeout_high_qc_cannot_poison_an_eligible_quorum() {
    let context = context();
    let round = Round::new(context.height(), 0);
    let high = qc(
        &context,
        0,
        Phase::Prepare,
        Subject::repeat(0x71),
        &[1, 2, 3],
    );
    let expected_high = high.clone();
    let mut reducer = Reducer::new(context.clone(), Some(id(1)), Generation::new(2)).unwrap();
    for (signer, highest) in [(4_u8, Some(high)), (1, None), (2, None)] {
        let vote = TimeoutVote::new(context.id(), round, id(signer), highest);
        let outcome = reducer
            .step(Event::TimeoutVoteReceived {
                tag: reducer.current_tag(),
                vote: SignedTimeoutVote::new(vote, signature(signer)),
            })
            .unwrap();
        if signer == 2 {
            let install = only_persist(outcome);
            let WalRecord::InstallTimeout(certificate) = install.record() else {
                panic!("equal-vote quorum must form a grouped timeout certificate");
            };
            assert_eq!(certificate.groups().len(), 2);
            assert_eq!(certificate.highest_prepare(), Some(&expected_high));
        } else {
            assert!(outcome.effects().is_empty());
        }
    }
}
#[test]
fn timeout_vote_rejects_an_unvalidated_embedded_high_qc() {
    let context = context();
    let round = Round::new(context.height(), 0);
    let invalid_high = qc(&context, 0, Phase::Prepare, Subject::repeat(0x72), &[1, 2]);
    let mut reducer = Reducer::new(context.clone(), Some(id(1)), Generation::new(3)).unwrap();
    let result = reducer.step(Event::TimeoutVoteReceived {
        tag: reducer.current_tag(),
        vote: SignedTimeoutVote::new(
            TimeoutVote::new(context.id(), round, id(4), Some(invalid_high)),
            signature(4),
        ),
    });
    assert!(matches!(result, Err(ReducerError::InvalidTimeoutVote)));
    assert_eq!(reducer.volatile_evidence_counts().1, 0);
}
#[test]
fn persisted_tc_starts_certified_fetch_for_a_missing_selected_lock() {
    let context = context();
    let subject = Subject::repeat(0x73);
    let high = qc(&context, 0, Phase::Prepare, subject, &[1, 2, 3]);
    let timeout = tc_with_high(&context, 0, high.clone(), &[1, 2, 3]);
    let mut reducer = Reducer::new(context, Some(id(1)), Generation::new(4)).unwrap();
    let install = only_persist(
        reducer
            .step(Event::TimeoutCertificateReceived {
                tag: reducer.current_tag(),
                certificate: timeout,
            })
            .unwrap(),
    );
    let entered = acknowledge(&mut reducer, &install);
    assert!(
        entered
            .effects()
            .iter()
            .any(|effect| matches!(effect, Effect::EnterView { .. }))
    );
    assert!(entered.effects().iter().any(|effect| matches!(
        effect,
        Effect::FetchBody {
            subject: fetched,
            certified_sources,
            certificate: Some(certificate),
            ..
        } if *fetched == subject
            && certified_sources == &vec![id(1), id(2), id(3)]
            && certificate == &high
    )));
    assert!(reducer.outbound_messages().any(|message| matches!(
        message,
        ConsensusMessageV2::QuorumCertificate(certificate)
            if certificate == &high
    )));
    let retransmit = reducer
        .step(Event::RetransmitElapsed {
            tag: reducer.current_tag(),
        })
        .expect("the TC-promoted PrepareQC remains an independent control owner");
    assert!(retransmit.effects().iter().any(|effect| matches!(
        effect,
        Effect::Broadcast(ConsensusMessageV2::QuorumCertificate(certificate))
            if certificate == &high
    )));
}
#[test]
fn strictly_ahead_install_timeout_advances_owner_and_protects_highest_prepare() {
    let context = context();
    let subject = Subject::repeat(0x74);
    let highest_prepare = qc(&context, 2, Phase::Prepare, subject, &[1, 2, 3]);
    let timeout = tc_with_high(&context, 3, highest_prepare.clone(), &[1, 2, 3]);
    let mut reducer = Reducer::new(context, Some(id(1)), Generation::new(5)).unwrap();
    let initial_tag = reducer.current_tag();
    let install = only_persist(
        reducer
            .step(Event::TimeoutCertificateReceived {
                tag: initial_tag,
                certificate: timeout.clone(),
            })
            .expect("strictly ahead TC starts a durable InstallTimeout transition"),
    );
    assert!(matches!(
        install.record(),
        WalRecord::InstallTimeout(certificate) if certificate == &timeout
    ));
    assert_eq!(reducer.current_tag(), initial_tag);
    let entered = acknowledge(&mut reducer, &install);
    assert_eq!(reducer.current_tag().view(), timeout.round().view() + 1);
    assert_ne!(reducer.current_tag().generation(), initial_tag.generation());
    assert_eq!(
        reducer
            .durable_state()
            .locked()
            .map(QuorumCertificate::subject),
        Some(subject)
    );
    assert!(entered.effects().iter().any(|effect| matches!(
        effect,
        Effect::EnterView {
            tag,
            certificate,
            protected_lock: Some(protected),
        } if *tag == reducer.current_tag()
            && certificate == &timeout
            && protected.subject() == subject
            && protected.reference() == highest_prepare.reference()
    )));
}
#[test]
fn adjacent_future_timeout_votes_form_a_catch_up_certificate() {
    let context = context();
    let mut reducer = Reducer::new(context.clone(), Some(id(4)), Generation::new(6)).unwrap();
    let current_round = Round::new(context.height(), 0);
    let future_round = Round::new(context.height(), 1);
    let current = reducer
        .step(Event::TimeoutVoteReceived {
            tag: reducer.current_tag(),
            vote: SignedTimeoutVote::new(
                TimeoutVote::new(context.id(), current_round, id(4), None),
                signature(4),
            ),
        })
        .expect("the current timeout share is retained");
    assert!(current.effects().is_empty());
    for signer in [1_u8, 2] {
        let future = reducer
            .step(Event::TimeoutVoteReceived {
                tag: reducer.current_tag(),
                vote: SignedTimeoutVote::new(
                    TimeoutVote::new(context.id(), future_round, id(signer), None),
                    signature(signer),
                ),
            })
            .expect("an adjacent future timeout share is retained");
        assert!(future.effects().is_empty());
    }
    assert_eq!(reducer.timeout_pool_snapshots().len(), 2);
    let install = only_persist(
        reducer
            .step(Event::TimeoutVoteReceived {
                tag: reducer.current_tag(),
                vote: SignedTimeoutVote::new(
                    TimeoutVote::new(context.id(), future_round, id(3), None),
                    signature(3),
                ),
            })
            .expect("future shares form a valid catch-up TC"),
    );
    assert!(matches!(
        install.record(),
        WalRecord::InstallTimeout(certificate) if certificate.round() == future_round
    ));
    let entered = acknowledge(&mut reducer, &install);
    assert_eq!(reducer.current_tag().view(), 2);
    assert!(entered.effects().iter().any(|effect| matches!(
        effect,
        Effect::EnterView { certificate, .. } if certificate.round() == future_round
    )));
    assert!(reducer.timeout_pool_snapshots().is_empty());
}
#[test]
fn timeout_install_preserves_adjacent_shares_for_the_new_current_view() {
    let context = context();
    let mut reducer = Reducer::new(context.clone(), Some(id(4)), Generation::new(7)).unwrap();
    let future_round = Round::new(context.height(), 1);
    for signer in [1_u8, 2] {
        let outcome = reducer
            .step(Event::TimeoutVoteReceived {
                tag: reducer.current_tag(),
                vote: SignedTimeoutVote::new(
                    TimeoutVote::new(context.id(), future_round, id(signer), None),
                    signature(signer),
                ),
            })
            .expect("future timeout share enters the bounded lookahead pool");
        assert!(outcome.effects().is_empty());
    }
    let current_tc = tc_without_high(&context, 0, &[1, 2, 3]);
    let install = only_persist(
        reducer
            .step(Event::TimeoutCertificateReceived {
                tag: reducer.current_tag(),
                certificate: current_tc,
            })
            .expect("the current TC advances to the prefetched round"),
    );
    acknowledge(&mut reducer, &install);
    assert_eq!(reducer.current_tag().view(), 1);
    assert!(matches!(
        reducer.timeout_pool_snapshots().as_slice(),
        [TimeoutPoolSnapshot { round, signers, .. }]
            if *round == future_round && signers == &[id(1), id(2)]
    ));
    let catch_up = only_persist(
        reducer
            .step(Event::TimeoutVoteReceived {
                tag: reducer.current_tag(),
                vote: SignedTimeoutVote::new(
                    TimeoutVote::new(context.id(), future_round, id(3), None),
                    signature(3),
                ),
            })
            .expect("the retained pool completes after view entry"),
    );
    assert!(matches!(
        catch_up.record(),
        WalRecord::InstallTimeout(certificate) if certificate.round() == future_round
    ));
}
#[test]
fn timeout_votes_beyond_adjacent_lookahead_are_ignored() {
    let context = context();
    let mut reducer = Reducer::new(context.clone(), Some(id(1)), Generation::new(8)).unwrap();
    let far_round = Round::new(context.height(), 2);
    let outcome = reducer
        .step(Event::TimeoutVoteReceived {
            tag: reducer.current_tag(),
            vote: SignedTimeoutVote::new(
                TimeoutVote::new(context.id(), far_round, id(2), None),
                signature(2),
            ),
        })
        .expect("far-future timeout traffic is bounded back");
    assert_eq!(
        outcome.disposition(),
        StepDisposition::Ignored(IgnoreReason::IrrelevantView)
    );
    assert!(reducer.timeout_pool_snapshots().is_empty());
}
#[test]
fn tc_omitting_the_local_high_keeps_its_exact_prepare_qc_retransmittable() {
    let context = context();
    let subject = Subject::repeat(0x7a);
    let round = Round::new(context.height(), 0);
    let prepare = qc(&context, 0, Phase::Prepare, subject, &[1, 2, 3]);
    let commit = Vote::new(context.id(), round, Phase::Commit, subject, id(1));
    let lock = WalEntry::new(
        PersistenceId::new(1),
        WalRecord::LockAndCommit {
            prepare: prepare.clone(),
            vote: commit,
        },
    );
    let mut reducer = Reducer::recover(context.clone(), Some(id(1)), Generation::new(6), [lock])
        .expect("recover the local high before the live TC install");
    assert!(matches!(
        resume_after_replay(&mut reducer).effects(),
        [Effect::Sign {
            message: SignableMessage::Vote(vote),
            ..
        }] if *vote == commit
    ));
    let signed = complete_signature(&mut reducer, 1);
    assert!(signed.effects().iter().any(|effect| matches!(
        effect,
        Effect::Broadcast(ConsensusMessageV2::Vote(vote)) if vote.vote() == commit
    )));
    let omitted = tc_without_high(&context, 0, &[2, 3, 4]);
    let install = only_persist(
        reducer
            .step(Event::TimeoutCertificateReceived {
                tag: reducer.current_tag(),
                certificate: omitted,
            })
            .expect("the quorum TC installs without carrying the local high"),
    );
    let entered = acknowledge(&mut reducer, &install);
    assert!(entered.effects().iter().any(|effect| matches!(
        effect,
        Effect::Sign {
            message: SignableMessage::Vote(vote),
            ..
        } if *vote == commit
    )));
    let resigned = complete_signature(&mut reducer, 2);
    assert!(resigned.effects().iter().any(|effect| matches!(
        effect,
        Effect::Broadcast(ConsensusMessageV2::Vote(vote)) if vote.vote() == commit
    )));
    assert_eq!(
        reducer
            .durable_state()
            .highest_prepare()
            .map(QuorumCertificate::reference),
        Some(prepare.reference())
    );
    assert!(reducer.outbound_messages().any(|message| matches!(
        message,
        ConsensusMessageV2::QuorumCertificate(certificate)
            if certificate == &prepare
    )));
    let retransmit = reducer
        .step(Event::RetransmitElapsed {
            tag: reducer.current_tag(),
        })
        .expect("the omitted local high remains epidemically retransmittable");
    assert!(retransmit.effects().iter().any(|effect| matches!(
        effect,
        Effect::Broadcast(ConsensusMessageV2::QuorumCertificate(certificate))
            if certificate == &prepare
    )));
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
fn same_round_timeout_upgrade_rebinds_lock_and_retains_current_timeout_vote() {
    let context = context();
    let subject = Subject::repeat(0x75);
    let prepare = qc(&context, 0, Phase::Prepare, subject, &[1, 2, 3]);
    let first = tc_without_high(&context, 0, &[1, 2, 3]);
    let upgrade = tc_with_high(&context, 0, prepare.clone(), &[1, 2, 3]);
    let mut reducer = Reducer::new(context.clone(), Some(id(4)), Generation::new(42)).unwrap();
    let first_entry = only_persist(
        reducer
            .step(Event::TimeoutCertificateReceived {
                tag: reducer.current_tag(),
                certificate: first.clone(),
            })
            .expect("the first TC enters view one without a high PrepareQC"),
    );
    acknowledge(&mut reducer, &first_entry);
    let before_upgrade = reducer.current_tag();
    assert_eq!(before_upgrade.view(), 1);
    assert!(reducer.durable_state().locked().is_none());
    let timeout_entry = only_persist(
        reducer
            .step(Event::TimeoutElapsed {
                tag: reducer.current_tag(),
            })
            .expect("the current view persists one local timeout intent"),
    );
    let timeout_sign = acknowledge(&mut reducer, &timeout_entry);
    assert!(matches!(
        timeout_sign.effects(),
        [Effect::Sign {
            message: SignableMessage::TimeoutVote(vote),
            ..
        }] if vote.round() == Round::new(context.height(), 1)
    ));
    let signed_timeout = reducer
        .step(Event::Signed {
            tag: reducer.current_tag(),
            signature: signature(4),
        })
        .expect("the durable current-view timeout intent is signed");
    let exact_timeout = signed_timeout
        .effects()
        .iter()
        .find_map(|effect| match effect {
            Effect::Broadcast(ConsensusMessageV2::TimeoutVote(vote)) => Some(vote.clone()),
            _ => None,
        })
        .expect("the signed current-view TimeoutVote is broadcast once");
    let partial_vote = SignedTimeoutVote::new(
        TimeoutVote::new(context.id(), Round::new(context.height(), 1), id(1), None),
        signature(1),
    );
    let partial = reducer
        .step(Event::TimeoutVoteReceived {
            tag: reducer.current_tag(),
            vote: partial_vote,
        })
        .expect("one responsive remote TimeoutVote enters the current pool");
    assert!(partial.effects().is_empty());
    let upgrade_entry = only_persist(
        reducer
            .step(Event::TimeoutCertificateReceived {
                tag: reducer.current_tag(),
                certificate: upgrade.clone(),
            })
            .expect("an alternate same-round TC may reveal a strictly higher Prepare origin"),
    );
    assert!(matches!(
        upgrade_entry.record(),
        WalRecord::InstallTimeout(certificate) if certificate == &upgrade
    ));
    let rebound = acknowledge(&mut reducer, &upgrade_entry);
    assert_eq!(reducer.current_tag().view(), before_upgrade.view());
    assert_ne!(
        reducer.current_tag().generation(),
        before_upgrade.generation(),
        "changing the protected lock creates a new asynchronous owner generation"
    );
    assert_eq!(
        reducer
            .durable_state()
            .locked()
            .map(QuorumCertificate::reference),
        Some(prepare.reference())
    );
    assert!(matches!(
        reducer.timeout_pool_snapshots().as_slice(),
        [TimeoutPoolSnapshot {
            round,
            signers,
            certificate_formed: false,
            ..
        }] if *round == Round::new(context.height(), 1)
            && signers == &[id(1), id(4)]
    ));
    assert!(rebound.effects().iter().any(|effect| matches!(
        effect,
        Effect::EnterView {
            tag,
            certificate,
            protected_lock: Some(lock),
        } if *tag == reducer.current_tag()
            && certificate == &upgrade
            && lock.reference() == prepare.reference()
    )));
    assert!(rebound.effects().iter().any(|effect| matches!(
        effect,
        Effect::FetchBody {
            round,
            subject: fetched_subject,
            certificate: Some(certificate),
            ..
        } if *round == prepare.proposal_round()
            && *fetched_subject == subject
            && certificate.reference() == prepare.reference()
    )));
    assert_eq!(
        reducer
            .outbound_messages()
            .filter_map(|message| match message {
                ConsensusMessageV2::TimeoutCertificate(certificate) => Some(certificate),
                _ => None,
            })
            .collect::<Vec<_>>(),
        vec![&upgrade],
        "the exact strict same-round upgrade replaces the old retained TC owner"
    );
    let retransmit = reducer
        .step(Event::RetransmitElapsed {
            tag: reducer.current_tag(),
        })
        .expect("the durable same-round upgrade remains retransmittable");
    assert!(retransmit.effects().iter().any(|effect| matches!(
        effect,
        Effect::Broadcast(ConsensusMessageV2::TimeoutCertificate(certificate))
            if certificate == &upgrade
    )));
    assert!(retransmit.effects().iter().any(|effect| matches!(
        effect,
        Effect::Broadcast(ConsensusMessageV2::TimeoutVote(vote))
            if vote == &exact_timeout
    )));
    assert!(retransmit.effects().iter().all(|effect| !matches!(
        effect,
        Effect::Broadcast(ConsensusMessageV2::TimeoutCertificate(certificate))
            if certificate == &first
    )));
    let duplicate = reducer
        .step(Event::TimeoutCertificateReceived {
            tag: reducer.current_tag(),
            certificate: upgrade.clone(),
        })
        .expect("an equal alternate TC is not a second lock upgrade");
    assert_eq!(
        duplicate.disposition(),
        StepDisposition::Ignored(IgnoreReason::Duplicate)
    );
    assert!(duplicate.effects().is_empty());
    let quorum_completion = reducer
        .step(Event::TimeoutVoteReceived {
            tag: reducer.current_tag(),
            vote: SignedTimeoutVote::new(
                TimeoutVote::new(
                    context.id(),
                    Round::new(context.height(), 1),
                    id(2),
                    Some(prepare.clone()),
                ),
                signature(2),
            ),
        })
        .expect("a post-upgrade vote completes the preserved timeout quorum");
    assert!(matches!(
        only_persist(quorum_completion).record(),
        WalRecord::InstallTimeout(certificate)
            if certificate.round() == Round::new(context.height(), 1)
    ));
    let mut recovered = Reducer::recover(
        context,
        Some(id(4)),
        Generation::new(99),
        vec![first_entry, timeout_entry, upgrade_entry],
    )
    .expect("replay selects the exact durable same-round timeout upgrade");
    assert!(matches!(
        resume_after_replay(&mut recovered).effects(),
        [Effect::Sign {
            message: SignableMessage::TimeoutVote(vote),
            ..
        }] if vote.round() == Round::new(recovered.context().height(), 1)
    ));
    let recovered_timeout = complete_signature(&mut recovered, 4)
        .effects()
        .iter()
        .find_map(|effect| match effect {
            Effect::Broadcast(ConsensusMessageV2::TimeoutVote(vote)) => Some(vote.clone()),
            _ => None,
        })
        .expect("replay reconstructs the durable current-view timeout owner");
    let replay_retransmit = recovered
        .step(Event::RetransmitElapsed {
            tag: recovered.current_tag(),
        })
        .expect("replayed timeout upgrade remains retransmittable");
    assert!(replay_retransmit.effects().iter().any(|effect| matches!(
        effect,
        Effect::Broadcast(ConsensusMessageV2::TimeoutCertificate(certificate))
            if certificate == &upgrade
    )));
    assert!(replay_retransmit.effects().iter().any(|effect| matches!(
        effect,
        Effect::Broadcast(ConsensusMessageV2::TimeoutVote(vote))
            if vote == &recovered_timeout
    )));
    assert!(replay_retransmit.effects().iter().all(|effect| !matches!(
        effect,
        Effect::Broadcast(ConsensusMessageV2::TimeoutCertificate(certificate))
            if certificate == &first
    )));
}
#[test]
fn same_round_timeout_upgrade_is_exact_local_proposal_justification() {
    let context = context();
    let proposal_view = 1;
    let subject = Subject::repeat(0xb1);
    let prepare = qc(&context, 0, Phase::Prepare, subject, &[1, 2, 3]);
    let first = tc_without_high(&context, 0, &[1, 2, 3]);
    let upgrade = tc_with_high(&context, 0, prepare.clone(), &[1, 2, 3]);
    let same_projection_foreign_evidence = tc_with_high(&context, 0, prepare, &[1, 2, 4]);
    let mut reducer = Reducer::new(
        context.clone(),
        Some(context.leader(proposal_view)),
        Generation::new(100),
    )
    .expect("reducer");
    let first_entry = only_persist(
        reducer
            .step(Event::TimeoutCertificateReceived {
                tag: reducer.current_tag(),
                certificate: first,
            })
            .expect("first timeout certificate starts installation"),
    );
    acknowledge(&mut reducer, &first_entry);
    let upgrade_entry = only_persist(
        reducer
            .step(Event::TimeoutCertificateReceived {
                tag: reducer.current_tag(),
                certificate: upgrade.clone(),
            })
            .expect("same-round high-QC upgrade starts installation"),
    );
    acknowledge(&mut reducer, &upgrade_entry);
    assert!(
        reducer
            .durable_state()
            .is_exact_local_proposal_timeout_justification(proposal_view, &upgrade,)
    );
    assert!(
        !reducer
            .durable_state()
            .is_exact_local_proposal_timeout_justification(
                proposal_view,
                &same_projection_foreign_evidence,
            ),
        "equal round/high-QC fields cannot replace the exact durable group evidence"
    );
    let manifest =
        PayloadManifest::new(subject, Digest::repeat(0xb2), Digest::repeat(0xb3), 256, 4);
    let proposal_entry = only_persist(
        reducer
            .step(Event::LocalProposalReady {
                tag: reducer.current_tag(),
                manifest,
            })
            .expect("local proposal uses the exact upgraded timeout certificate"),
    );
    assert!(matches!(
        proposal_entry.record(),
        WalRecord::ProposalIntent(proposal)
            if proposal.justification() == &ProposalJustification::Timeout(upgrade)
    ));
}
#[test]
fn recovery_uses_same_round_timeout_upgrade_as_exact_local_proposal_justification() {
    let context = context();
    let proposal_view = 1;
    let subject = Subject::repeat(0xb4);
    let prepare = qc(&context, 0, Phase::Prepare, subject, &[1, 2, 3]);
    let first = tc_without_high(&context, 0, &[1, 2, 3]);
    let upgrade = tc_with_high(&context, 0, prepare, &[1, 2, 3]);
    let manifest =
        PayloadManifest::new(subject, Digest::repeat(0xb5), Digest::repeat(0xb6), 384, 6);
    let exact_proposal = Proposal::new(
        context.id(),
        Round::new(context.height(), proposal_view),
        context.leader(proposal_view),
        manifest,
        ProposalJustification::Timeout(upgrade.clone()),
    );
    let exact_entries = vec![
        WalEntry::new(
            PersistenceId::new(1),
            WalRecord::InstallTimeout(first.clone()),
        ),
        WalEntry::new(
            PersistenceId::new(2),
            WalRecord::InstallTimeout(upgrade.clone()),
        ),
        WalEntry::new(
            PersistenceId::new(3),
            WalRecord::ProposalIntent(exact_proposal.clone()),
        ),
    ];
    let mut recovered = Reducer::recover(
        context.clone(),
        Some(context.leader(proposal_view)),
        Generation::new(101),
        exact_entries,
    )
    .expect("recovery accepts the exact latest timeout justification");
    assert_eq!(
        recovered
            .durable_state()
            .proposal_intent(Round::new(context.height(), proposal_view)),
        Some(&exact_proposal)
    );
    assert!(matches!(
        resume_after_replay(&mut recovered).effects(),
        [Effect::Sign {
            message: SignableMessage::Proposal(proposal),
            ..
        }] if proposal == &exact_proposal
    ));
    let stale_proposal = Proposal::new(
        context.id(),
        Round::new(context.height(), proposal_view),
        context.leader(proposal_view),
        manifest,
        ProposalJustification::Timeout(first.clone()),
    );
    assert_eq!(
        Reducer::recover(
            context.clone(),
            Some(context.leader(proposal_view)),
            Generation::new(102),
            [
                WalEntry::new(PersistenceId::new(1), WalRecord::InstallTimeout(first)),
                WalEntry::new(PersistenceId::new(2), WalRecord::InstallTimeout(upgrade),),
                WalEntry::new(
                    PersistenceId::new(3),
                    WalRecord::ProposalIntent(stale_proposal),
                ),
            ],
        ),
        Err(ReducerError::Replay(ReplayError::InvalidProposalIntent)),
        "replay rejects a predecessor-round TC superseded by the durable upgrade"
    );
}
#[test]
fn recovery_excludes_proposal_intent_superseded_by_same_round_timeout_upgrade() {
    let context = context();
    let proposal_view = 1;
    let round = Round::new(context.height(), proposal_view);
    let subject = Subject::repeat(0xb7);
    let prepare = qc(&context, 0, Phase::Prepare, subject, &[1, 2, 3]);
    let first = tc_without_high(&context, 0, &[1, 2, 3]);
    let upgrade = tc_with_high(&context, 0, prepare, &[1, 2, 3]);
    let manifest =
        PayloadManifest::new(subject, Digest::repeat(0xb8), Digest::repeat(0xb9), 512, 8);
    let mut live = Reducer::new(
        context.clone(),
        Some(context.leader(proposal_view)),
        Generation::new(103),
    )
    .expect("reducer");
    let first_entry = only_persist(
        live.step(Event::TimeoutCertificateReceived {
            tag: live.current_tag(),
            certificate: first,
        })
        .expect("first timeout certificate starts installation"),
    );
    acknowledge(&mut live, &first_entry);
    let proposal_entry = only_persist(
        live.step(Event::LocalProposalReady {
            tag: live.current_tag(),
            manifest,
        })
        .expect("the first durable TC initially authorizes the local proposal"),
    );
    let historical_proposal = match proposal_entry.record() {
        WalRecord::ProposalIntent(proposal) => proposal.clone(),
        record => panic!("expected ProposalIntent, got {record:?}"),
    };
    let signing = acknowledge(&mut live, &proposal_entry);
    assert!(matches!(
        signing.effects(),
        [Effect::Sign {
            message: SignableMessage::Proposal(proposal),
            ..
        }] if proposal == &historical_proposal
    ));
    assert_eq!(
        live.awaiting_signature(),
        Some(&SignableMessage::Proposal(historical_proposal.clone()))
    );
    let signed_proposal = complete_signature(&mut live, 1);
    assert!(
        signed_proposal.effects().iter().any(|effect| matches!(
            effect,
            Effect::Broadcast(ConsensusMessageV2::Proposal(proposal))
                if proposal.proposal() == &historical_proposal
        )),
        "signature completion broadcasts the proposal before the TC upgrade"
    );
    let prepare_entry = signed_proposal
        .effects()
        .iter()
        .find_map(|effect| match effect {
            Effect::Persist { entry, .. }
                if matches!(entry.record(), WalRecord::PrepareIntent(_)) =>
            {
                Some(entry.clone())
            }
            _ => None,
        })
        .expect("processing the signed proposal starts its local Prepare append");
    let prepare_signing = acknowledge(&mut live, &prepare_entry);
    assert!(
        prepare_signing.effects().iter().any(|effect| matches!(
            effect,
            Effect::Sign {
                message: SignableMessage::Vote(vote),
                ..
            } if vote.phase() == Phase::Prepare && vote.round() == round
        )),
        "acknowledging the local Prepare intent starts its signature"
    );
    let signed_prepare = complete_signature(&mut live, 2);
    assert!(
        signed_prepare.effects().iter().any(|effect| matches!(
            effect,
            Effect::Broadcast(ConsensusMessageV2::Vote(vote))
                if vote.vote().phase() == Phase::Prepare && vote.vote().round() == round
        )),
        "Prepare signature completion drains the pre-upgrade local work"
    );
    assert!(live.awaiting_signature().is_none());
    assert!(live.pending_persistence_record().is_none());
    let upgrade_entry = only_persist(
        live.step(Event::TimeoutCertificateReceived {
            tag: live.current_tag(),
            certificate: upgrade.clone(),
        })
        .expect("same-round TC upgrade supersedes proposal-signing authority"),
    );
    acknowledge(&mut live, &upgrade_entry);
    assert_eq!(
        live.durable_state().proposal_intent(round),
        Some(&historical_proposal),
        "the old intent remains durable non-equivocation evidence"
    );
    assert_eq!(live.durable_state().last_timeout(), Some(&upgrade));
    assert!(live.awaiting_signature().is_none());
    assert!(live.queued_signatures().next().is_none());
    assert!(
        live.step(Event::RetransmitElapsed {
            tag: live.current_tag(),
        })
        .expect("retransmission uses only current durable authorization")
        .effects()
        .iter()
        .all(|effect| !matches!(
            effect,
            Effect::Broadcast(ConsensusMessageV2::Proposal(proposal))
                if proposal.proposal() == &historical_proposal
        ))
    );
    let mut recovered = Reducer::recover(
        context.clone(),
        Some(context.leader(proposal_view)),
        Generation::new(104),
        [first_entry, proposal_entry, prepare_entry, upgrade_entry],
    )
    .expect("append-only replay retains the proposal followed by its TC upgrade");
    assert_eq!(
        recovered.durable_state().proposal_intent(round),
        Some(&historical_proposal),
        "the old intent remains durable non-equivocation evidence"
    );
    assert_eq!(recovered.durable_state().last_timeout(), Some(&upgrade));
    let resumed = resume_after_replay(&mut recovered);
    assert!(
        resumed.effects().iter().all(|effect| !matches!(
            effect,
            Effect::Sign {
                message: SignableMessage::Proposal(proposal),
                ..
            } if proposal == &historical_proposal
        )),
        "recovery cannot re-sign a proposal whose TC is no longer the durable latest"
    );
    assert!(!matches!(
        recovered.awaiting_signature(),
        Some(SignableMessage::Proposal(proposal)) if proposal == &historical_proposal
    ));
    assert!(recovered.queued_signatures().all(|message| !matches!(
        message,
        SignableMessage::Proposal(proposal) if proposal == &historical_proposal
    )));
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
fn prior_view_commit_votes_rebuild_the_exact_locked_round_quorum() {
    let context = context();
    let subject = Subject::repeat(0x72);
    let round = Round::new(context.height(), 0);
    let prepare = qc(&context, 0, Phase::Prepare, subject, &[1, 2, 3]);
    let local_commit = Vote::new(context.id(), round, Phase::Commit, subject, id(1));
    let lock = WalEntry::new(
        PersistenceId::new(1),
        WalRecord::LockAndCommit {
            prepare: prepare.clone(),
            vote: local_commit,
        },
    );
    let install = WalEntry::new(
        PersistenceId::new(2),
        WalRecord::InstallTimeout(tc_with_high(&context, 0, prepare, &[1, 2, 3])),
    );
    let mut reducer = Reducer::recover(
        context.clone(),
        Some(id(1)),
        Generation::new(43),
        [lock, install],
    )
    .expect("durable lock survives the view transition");
    assert_eq!(reducer.current_tag().view(), 1);
    assert_eq!(
        reducer.progress_witness_violation(),
        None,
        "recovery-pending state owns reconstruction of the durable Commit intent"
    );
    assert!(matches!(
        resume_after_replay(&mut reducer).effects(),
        [Effect::Sign {
            message: SignableMessage::Vote(vote),
            ..
        }] if *vote == local_commit
    ));
    assert_eq!(
        reducer.progress_witness_violation(),
        None,
        "the resumed signature request witnesses the durable Commit intent"
    );
    let signed = reducer
        .step(Event::Signed {
            tag: reducer.current_tag(),
            signature: signature(1),
        })
        .expect("retransmitted local CommitVote re-enters its old round pool");
    assert!(signed.effects().iter().any(|effect| matches!(
        effect,
        Effect::Broadcast(ConsensusMessageV2::Vote(vote)) if vote.vote() == local_commit
    )));
    assert_eq!(
        reducer.progress_witness_violation(),
        None,
        "the signed outbound vote and rebuilt pool replace the signature witness"
    );
    let pools = reducer.vote_pool_snapshots();
    assert!(matches!(
        pools.as_slice(),
        [VotePoolSnapshot {
            round: pooled_round,
            proposal_round: pooled_proposal_round,
            phase: Phase::Commit,
            subject: pooled_subject,
            signers,
            signed_power,
        }] if *pooled_round == round
            && *pooled_proposal_round == round
            && *pooled_subject == subject
            && signers == &[id(1)]
            && *signed_power == VotingPower::new(1)
    ));
    assert!(reducer.outbound_messages().any(
        |message| matches!(message, ConsensusMessageV2::Vote(vote) if vote.vote() == local_commit)
    ));
    for signer in [2, 3] {
        let outcome = reducer
            .step(Event::VoteReceived {
                tag: reducer.current_tag(),
                vote: SignedVote::new(
                    Vote::new(context.id(), round, Phase::Commit, subject, id(signer)),
                    signature(signer),
                ),
            })
            .expect("known locked-round CommitVote remains admissible");
        if signer == 2 {
            assert!(outcome.effects().is_empty());
        } else {
            let decision = only_persist(outcome);
            assert!(matches!(
                decision.record(),
                WalRecord::Decision(certificate)
                    if certificate.round() == round
                        && certificate.phase() == Phase::Commit
                        && certificate.subject() == subject
            ));
        }
    }
}
#[test]
fn vote_statement_identity_excludes_only_the_authenticated_signer() {
    let context = context();
    let round = Round::new(context.height(), 2);
    let subject = Subject::repeat(0x7a);
    let first = Vote::new(context.id(), round, Phase::Commit, subject, id(1));
    let second = Vote::new(context.id(), round, Phase::Commit, subject, id(2));
    assert!(first.same_statement(second));
    assert!(!first.same_statement(Vote::new(
        context.id(),
        round,
        Phase::Prepare,
        subject,
        id(2),
    )));
    assert!(!first.same_statement(Vote::new(
        context.id(),
        round,
        Phase::Commit,
        Subject::repeat(0x7b),
        id(2),
    )));
    assert!(!first.same_statement(Vote::new_with_proposal_round(
        context.id(),
        round,
        Round::new(context.height(), 1),
        Phase::Commit,
        subject,
        id(2),
    )));
}
#[test]
fn higher_tc_lock_prunes_superseded_commit_retransmission() {
    let context = context();
    let old_subject = Subject::repeat(0x91);
    let higher_subject = Subject::repeat(0x92);
    let old_round = Round::new(context.height(), 0);
    let old_prepare = qc(&context, 0, Phase::Prepare, old_subject, &[1, 2, 3]);
    let old_commit = Vote::new(context.id(), old_round, Phase::Commit, old_subject, id(1));
    let mut reducer = Reducer::recover(
        context.clone(),
        Some(id(1)),
        Generation::new(45),
        [
            WalEntry::new(
                PersistenceId::new(1),
                WalRecord::LockAndCommit {
                    prepare: old_prepare,
                    vote: old_commit,
                },
            ),
            WalEntry::new(
                PersistenceId::new(2),
                WalRecord::InstallTimeout(tc_without_high(&context, 0, &[1, 2, 3])),
            ),
        ],
    )
    .expect("recover the old active lock in view one");
    assert!(matches!(
        resume_after_replay(&mut reducer).effects(),
        [Effect::Sign {
            message: SignableMessage::Vote(vote),
            ..
        }] if *vote == old_commit
    ));
    reducer
        .step(Event::Signed {
            tag: reducer.current_tag(),
            signature: signature(1),
        })
        .expect("sign and retain the old active-lock Commit vote");
    assert!(reducer.outbound_messages().any(|message| {
        matches!(message, ConsensusMessageV2::Vote(vote) if vote.vote() == old_commit)
    }));
    let higher_prepare = qc(&context, 1, Phase::Prepare, higher_subject, &[1, 2, 3]);
    let observed = reducer
        .step(Event::QuorumCertificateReceived {
            tag: reducer.current_tag(),
            certificate: higher_prepare.clone(),
        })
        .expect("a strictly higher PrepareQC is safe above the old lock");
    let observe_entry = observed
        .effects()
        .iter()
        .find_map(|effect| match effect {
            Effect::Persist { entry, .. } => Some(entry.clone()),
            _ => None,
        })
        .expect("the higher PrepareQC is durably observed");
    acknowledge(&mut reducer, &observe_entry);
    assert_eq!(
        reducer
            .durable_state()
            .locked()
            .map(QuorumCertificate::subject),
        Some(old_subject),
        "observing a higher PrepareQC alone does not retag the lock"
    );
    let install = only_persist(
        reducer
            .step(Event::TimeoutCertificateReceived {
                tag: reducer.current_tag(),
                certificate: tc_with_high(&context, 1, higher_prepare.clone(), &[1, 2, 3]),
            })
            .expect("the next TC starts durable lock promotion"),
    );
    let entered = acknowledge(&mut reducer, &install);
    assert_eq!(reducer.current_tag().view(), 2);
    assert_eq!(
        reducer
            .durable_state()
            .locked()
            .map(QuorumCertificate::reference),
        Some(higher_prepare.reference())
    );
    assert_eq!(
        reducer.durable_state().commit_intent(old_round),
        Some(old_commit),
        "superseded Commit intent remains immutable WAL history"
    );
    assert_eq!(reducer.progress_witness_violation(), None);
    assert!(entered.effects().iter().all(|effect| {
        !matches!(
            effect,
            Effect::Sign {
                message: SignableMessage::Vote(vote),
                ..
            } if *vote == old_commit
        )
    }));
    assert!(reducer.outbound_messages().all(|message| {
        !matches!(message, ConsensusMessageV2::Vote(vote) if vote.vote() == old_commit)
    }));
    let retransmit = reducer
        .step(Event::RetransmitElapsed {
            tag: reducer.current_tag(),
        })
        .expect("superseded Commit retransmission cannot violate refinement");
    assert!(retransmit.effects().iter().all(|effect| {
        !matches!(
            effect,
            Effect::Broadcast(ConsensusMessageV2::Vote(vote)) if vote.vote() == old_commit
        )
    }));
    assert!(retransmit.effects().iter().any(|effect| {
        matches!(
            effect,
            Effect::FetchBody {
                subject,
                certificate: Some(certificate),
                ..
            } if *subject == higher_subject && certificate.reference() == higher_prepare.reference()
        )
    }));
}
#[test]
fn same_lock_tc_resigns_local_commit_and_rebuilds_quorum_without_self_delivery() {
    let context = context();
    let subject = Subject::repeat(0x95);
    let round = Round::new(context.height(), 0);
    let prepare = qc(&context, 0, Phase::Prepare, subject, &[1, 2, 3]);
    let local_commit = Vote::new(context.id(), round, Phase::Commit, subject, id(1));
    let mut reducer = Reducer::recover(
        context.clone(),
        Some(id(1)),
        Generation::new(47),
        [WalEntry::new(
            PersistenceId::new(1),
            WalRecord::LockAndCommit {
                prepare: prepare.clone(),
                vote: local_commit,
            },
        )],
    )
    .expect("recover the active locked Commit intent");
    assert!(matches!(
        resume_after_replay(&mut reducer).effects(),
        [Effect::Sign {
            message: SignableMessage::Vote(vote),
            ..
        }] if *vote == local_commit
    ));
    reducer
        .step(Event::Signed {
            tag: reducer.current_tag(),
            signature: signature(1),
        })
        .expect("place the original local Commit in its pool");
    assert!(matches!(
        reducer.vote_pool_snapshots().as_slice(),
        [VotePoolSnapshot {
            round: pooled_round,
            phase: Phase::Commit,
            subject: pooled_subject,
            signers,
            ..
        }] if *pooled_round == round && *pooled_subject == subject && signers == &[id(1)]
    ));
    let install = only_persist(
        reducer
            .step(Event::TimeoutCertificateReceived {
                tag: reducer.current_tag(),
                certificate: tc_with_high(&context, 0, prepare, &[1, 2, 3]),
            })
            .expect("begin the same-lock TC installation"),
    );
    let entered = acknowledge(&mut reducer, &install);
    assert_eq!(reducer.current_tag().view(), 1);
    assert!(reducer.vote_pool_snapshots().is_empty());
    assert!(matches!(
        entered.effects().last(),
        Some(Effect::Sign {
            message: SignableMessage::Vote(vote),
            ..
        }) if *vote == local_commit
    ));
    assert_eq!(
        reducer.progress_witness_violation(),
        None,
        "the queued signature owns reconstruction of the local pool entry"
    );
    reducer
        .step(Event::Signed {
            tag: reducer.current_tag(),
            signature: signature(9),
        })
        .expect("re-sign the exact durable Commit in the new pool generation");
    assert!(matches!(
        reducer.vote_pool_snapshots().as_slice(),
        [VotePoolSnapshot { signers, .. }] if signers == &[id(1)]
    ));
    for signer in [2_u8, 3] {
        let outcome = reducer
            .step(Event::VoteReceived {
                tag: reducer.current_tag(),
                vote: SignedVote::new(
                    Vote::new(context.id(), round, Phase::Commit, subject, id(signer)),
                    signature(signer),
                ),
            })
            .expect("responsive peer Commit enters the rebuilt locked-round pool");
        if signer == 2 {
            assert!(outcome.effects().is_empty());
        } else {
            assert!(matches!(
                only_persist(outcome).record(),
                WalRecord::Decision(certificate)
                    if certificate.round() == round
                        && certificate.phase() == Phase::Commit
                        && certificate.subject() == subject
            ));
        }
    }
}
#[test]
fn later_reproposal_commit_ack_retires_durable_old_round_commit_pool() {
    let context = context();
    let subject = Subject::repeat(0xa4);
    let original_round = Round::new(context.height(), 0);
    let reproposal_round = Round::new(context.height(), 1);
    let original_prepare = qc(&context, 0, Phase::Prepare, subject, &[1, 2, 3]);
    let old_commit = Vote::new(context.id(), original_round, Phase::Commit, subject, id(4));
    let reproposal_prepare = qc(&context, 1, Phase::Prepare, subject, &[1, 2, 3]);
    let reproposal_commit = Vote::new(
        context.id(),
        reproposal_round,
        Phase::Commit,
        subject,
        id(4),
    );
    let mut reducer = Reducer::recover(
        context.clone(),
        Some(id(4)),
        Generation::new(51),
        [WalEntry::new(
            PersistenceId::new(1),
            WalRecord::LockAndCommit {
                prepare: original_prepare.clone(),
                vote: old_commit,
            },
        )],
    )
    .expect("recover the original Commit intent");
    assert!(matches!(
        resume_after_replay(&mut reducer).effects(),
        [Effect::Sign {
            message: SignableMessage::Vote(vote),
            ..
        }] if *vote == old_commit
    ));
    reducer
        .step(Event::Signed {
            tag: reducer.current_tag(),
            signature: signature(4),
        })
        .expect("rebuild the original Commit pool");
    let timeout = tc_with_high(&context, 0, original_prepare, &[1, 2, 3]);
    let install = only_persist(
        reducer
            .step(Event::TimeoutCertificateReceived {
                tag: reducer.current_tag(),
                certificate: timeout.clone(),
            })
            .expect("install the timeout certificate that opens the reproposal round"),
    );
    acknowledge(&mut reducer, &install);
    assert_eq!(reducer.current_tag().view(), reproposal_round.view());
    reducer
        .step(Event::Signed {
            tag: reducer.current_tag(),
            signature: signature(5),
        })
        .expect("retransmit the still-authorized durable old-round Commit");
    assert!(matches!(
        reducer.vote_pool_snapshots().as_slice(),
        [VotePoolSnapshot {
            round,
            proposal_round: origin,
            phase: Phase::Commit,
            ..
        }] if *round == original_round && *origin == original_round
    ));
    let received = reducer
        .step(Event::ProposalReceived {
            tag: reducer.current_tag(),
            proposal: proposal(
                &context,
                reproposal_round.view(),
                subject,
                ProposalJustification::Timeout(timeout),
            ),
        })
        .expect("accept the unchanged locked subject at a new proposal origin");
    assert!(matches!(
        received.effects(),
        [Effect::FetchBody {
            round,
            subject: fetched_subject,
            ..
        }] if *round == reproposal_round && *fetched_subject == subject
    ));
    assert!(matches!(
        reducer
            .step(Event::BodyAvailable {
                tag: reducer.current_tag(),
                round: reproposal_round,
                subject,
            })
            .expect("recover the re-proposed locked body")
            .effects(),
        [Effect::StoreBody { .. }]
    ));
    assert!(matches!(
        reducer
            .step(Event::BodyStored {
                tag: reducer.current_tag(),
                round: reproposal_round,
                subject,
            })
            .expect("store the re-proposed locked body")
            .effects(),
        [Effect::ValidateBody { .. }]
    ));
    let prepare_entry = only_persist(
        reducer
            .step(Event::ValidationCompleted {
                tag: reducer.current_tag(),
                round: reproposal_round,
                subject,
                valid: true,
            })
            .expect("validation starts the new same-round Prepare intent"),
    );
    assert!(matches!(
        prepare_entry.record(),
        WalRecord::PrepareIntent(vote)
            if *vote == Vote::new(
                context.id(),
                reproposal_round,
                Phase::Prepare,
                subject,
                id(4),
            )
    ));
    assert!(
        reducer
            .vote_pool_snapshots()
            .iter()
            .any(|pool| pool.round == original_round)
    );
    let prepared = acknowledge(&mut reducer, &prepare_entry);
    assert!(matches!(
        prepared.effects(),
        [Effect::Sign {
            message: SignableMessage::Vote(vote),
            ..
        }] if vote.phase() == Phase::Prepare && vote.round() == reproposal_round
    ));
    reducer
        .step(Event::Signed {
            tag: reducer.current_tag(),
            signature: signature(6),
        })
        .expect("self-admit the new same-round Prepare vote");
    let commit_entry = only_persist(
        reducer
            .step(Event::QuorumCertificateReceived {
                tag: reducer.current_tag(),
                certificate: reproposal_prepare.clone(),
            })
            .expect("the new same-round PrepareQC authorizes Commit"),
    );
    assert!(matches!(
        commit_entry.record(),
        WalRecord::LockAndCommit { prepare, vote }
            if prepare.reference() == reproposal_prepare.reference()
                && *vote == reproposal_commit
    ));
    assert!(
        reducer
            .vote_pool_snapshots()
            .iter()
            .any(|pool| pool.round == original_round)
    );
    let committed = acknowledge(&mut reducer, &commit_entry);
    assert!(matches!(
        committed.effects(),
        [Effect::Sign {
            message: SignableMessage::Vote(vote),
            ..
        }] if *vote == reproposal_commit
    ));
    assert!(
        reducer
            .vote_pool_snapshots()
            .iter()
            .all(|pool| pool.round == reproposal_round),
        "the durable new lock retires the old-round Commit pool"
    );
    reducer
        .step(Event::Signed {
            tag: reducer.current_tag(),
            signature: signature(7),
        })
        .expect("self-admit the new same-round Commit vote");
    assert!(reducer.vote_pool_snapshots().iter().any(|pool| {
        pool.round == reproposal_round
            && pool.proposal_round == reproposal_round
            && pool.phase == Phase::Commit
            && pool.subject == subject
    }));
}
#[test]
fn tc_highest_prepare_missed_locally_requires_same_subject_reproposal_before_commit() {
    let context = context();
    let subject = Subject::repeat(0x96);
    let original_round = Round::new(context.height(), 0);
    let reproposal_round = Round::new(context.height(), 1);
    let original_prepare = qc(&context, 0, Phase::Prepare, subject, &[1, 2, 3]);
    let reproposal_prepare = qc(&context, 1, Phase::Prepare, subject, &[1, 2, 3]);
    let local_commit = Vote::new(
        context.id(),
        reproposal_round,
        Phase::Commit,
        subject,
        id(4),
    );
    let mut reducer = Reducer::new(context.clone(), Some(id(4)), Generation::new(48)).unwrap();
    let timeout_entry = only_persist(
        reducer
            .step(Event::TimeoutElapsed {
                tag: reducer.current_tag(),
            })
            .expect("time out without observing the PrepareQC carried by the quorum"),
    );
    acknowledge(&mut reducer, &timeout_entry);
    reducer
        .step(Event::Signed {
            tag: reducer.current_tag(),
            signature: signature(4),
        })
        .expect("finish the durable timeout vote");
    let timeout = tc_with_high(&context, 0, original_prepare.clone(), &[1, 2, 3]);
    let install = only_persist(
        reducer
            .step(Event::TimeoutCertificateReceived {
                tag: reducer.current_tag(),
                certificate: timeout.clone(),
            })
            .expect("the TC starts durable promotion of its highest PrepareQC"),
    );
    let entered = acknowledge(&mut reducer, &install);
    assert_eq!(reducer.current_tag().view(), 1);
    assert_eq!(
        reducer
            .durable_state()
            .locked()
            .map(QuorumCertificate::reference),
        Some(original_prepare.reference())
    );
    assert_eq!(
        reducer.durable_state().commit_intent(reproposal_round),
        None
    );
    assert!(entered.effects().iter().any(|effect| matches!(
        effect,
        Effect::FetchBody {
            round: fetched_round,
            subject: fetched_subject,
            certificate: Some(certificate),
            ..
        } if *fetched_round == original_round
            && *fetched_subject == subject
            && certificate.reference() == original_prepare.reference()
    )));
    assert!(matches!(
        reducer
            .step(Event::BodyAvailable {
                tag: reducer.current_tag(),
                round: original_round,
                subject,
            })
            .expect("recover the exact TC-protected body")
            .effects(),
        [Effect::StoreBody { .. }]
    ));
    assert!(matches!(
        reducer
            .step(Event::BodyStored {
                tag: reducer.current_tag(),
                round: original_round,
                subject,
            })
            .expect("durably store the exact TC-protected body")
            .effects(),
        [Effect::ValidateBody { .. }]
    ));
    let historical_validation = reducer
        .step(Event::ValidationCompleted {
            tag: reducer.current_tag(),
            round: original_round,
            subject,
            valid: true,
        })
        .expect("validating the old origin cannot manufacture a later-view Commit");
    assert_eq!(
        historical_validation.disposition(),
        StepDisposition::Ignored(IgnoreReason::IrrelevantView)
    );
    assert!(historical_validation.effects().is_empty());
    assert_eq!(
        reducer.durable_state().commit_intent(reproposal_round),
        None
    );
    let relabelled = reducer
        .step(Event::VoteReceived {
            tag: reducer.current_tag(),
            vote: SignedVote::new(
                Vote::new_with_proposal_round(
                    context.id(),
                    reproposal_round,
                    original_round,
                    Phase::Commit,
                    subject,
                    id(1),
                ),
                signature(1),
            ),
        })
        .expect_err("a later-view Commit retaining the old round is malformed");
    assert!(matches!(relabelled, ReducerError::InvalidVote));
    assert_eq!(reducer.progress_witness_violation(), None);
    let received = reducer
        .step(Event::ProposalReceived {
            tag: reducer.current_tag(),
            proposal: proposal(
                &context,
                reproposal_round.view(),
                subject,
                ProposalJustification::Timeout(timeout),
            ),
        })
        .expect("accept the exact locked subject re-proposed in the current view");
    assert!(matches!(
        received.effects(),
        [Effect::FetchBody {
            round,
            subject: fetched_subject,
            ..
        }] if *round == reproposal_round && *fetched_subject == subject
    ));
    assert!(matches!(
        reducer
            .step(Event::BodyAvailable {
                tag: reducer.current_tag(),
                round: reproposal_round,
                subject,
            })
            .expect("recover the current reproposal body")
            .effects(),
        [Effect::StoreBody { .. }]
    ));
    assert!(matches!(
        reducer
            .step(Event::BodyStored {
                tag: reducer.current_tag(),
                round: reproposal_round,
                subject,
            })
            .expect("durably store the current reproposal body")
            .effects(),
        [Effect::ValidateBody { .. }]
    ));
    let prepare_entry = only_persist(
        reducer
            .step(Event::ValidationCompleted {
                tag: reducer.current_tag(),
                round: reproposal_round,
                subject,
                valid: true,
            })
            .expect("the re-proposed body starts a same-round Prepare intent"),
    );
    assert!(matches!(
        prepare_entry.record(),
        WalRecord::PrepareIntent(vote)
            if vote.round() == reproposal_round && vote.proposal_round() == reproposal_round
    ));
    acknowledge(&mut reducer, &prepare_entry);
    reducer
        .step(Event::Signed {
            tag: reducer.current_tag(),
            signature: signature(8),
        })
        .expect("sign and self-admit the same-round Prepare");
    let commit_entry = only_persist(
        reducer
            .step(Event::QuorumCertificateReceived {
                tag: reducer.current_tag(),
                certificate: reproposal_prepare.clone(),
            })
            .expect("the same-round PrepareQC authorizes the Commit intent"),
    );
    assert!(matches!(
        commit_entry.record(),
        WalRecord::LockAndCommit { prepare, vote }
            if prepare.reference() == reproposal_prepare.reference()
                && vote.round() == reproposal_round
                && vote.proposal_round() == reproposal_round
                && *vote == local_commit
    ));
    let persisted = acknowledge(&mut reducer, &commit_entry);
    assert_eq!(
        reducer.durable_state().commit_intent(reproposal_round),
        Some(local_commit)
    );
    assert!(matches!(
        persisted.effects(),
        [Effect::Sign {
            message: SignableMessage::Vote(vote),
            ..
        }] if *vote == local_commit
    ));
    reducer
        .step(Event::Signed {
            tag: reducer.current_tag(),
            signature: signature(9),
        })
        .expect("sign and self-admit the re-proposal Commit");
    for signer in [1_u8, 2] {
        let outcome = reducer
            .step(Event::VoteReceived {
                tag: reducer.current_tag(),
                vote: SignedVote::new(
                    Vote::new(
                        context.id(),
                        reproposal_round,
                        Phase::Commit,
                        subject,
                        id(signer),
                    ),
                    signature(signer),
                ),
            })
            .expect("responsive peer Commit builds the re-proposal quorum");
        if signer == 1 {
            assert!(outcome.effects().is_empty());
        } else {
            assert!(matches!(
                only_persist(outcome).record(),
                WalRecord::Decision(certificate)
                    if certificate.round() == reproposal_round
                        && certificate.proposal_round() == reproposal_round
                        && certificate.phase() == Phase::Commit
                        && certificate.subject() == subject
            ));
        }
    }
}
#[test]
fn tc_lock_survives_closed_view_and_commits_after_later_same_subject_reproposal() {
    let context = context();
    let subject = Subject::repeat(0x9C);
    let original_round = Round::new(context.height(), 0);
    let first_reproposal_round = Round::new(context.height(), 1);
    let retry_reproposal_round = Round::new(context.height(), 2);
    let original_prepare = qc(&context, 0, Phase::Prepare, subject, &[1, 2, 3]);
    let mut reducer = Reducer::new(context.clone(), Some(id(4)), Generation::new(49)).unwrap();
    let first_timeout = tc_with_high(&context, 0, original_prepare.clone(), &[1, 2, 3]);
    let first_install = only_persist(
        reducer
            .step(Event::TimeoutCertificateReceived {
                tag: reducer.current_tag(),
                certificate: first_timeout,
            })
            .expect("the first TC durably promotes the missed PrepareQC"),
    );
    acknowledge(&mut reducer, &first_install);
    assert_eq!(reducer.current_tag().view(), first_reproposal_round.view());
    reducer
        .step(Event::BodyAvailable {
            tag: reducer.current_tag(),
            round: original_round,
            subject,
        })
        .expect("recover the exact TC-promoted body");
    reducer
        .step(Event::BodyStored {
            tag: reducer.current_tag(),
            round: original_round,
            subject,
        })
        .expect("store the exact TC-promoted body before validation finishes");
    let timeout_entry = only_persist(
        reducer
            .step(Event::TimeoutElapsed {
                tag: reducer.current_tag(),
            })
            .expect("the finality view may close while validation is in flight"),
    );
    acknowledge(&mut reducer, &timeout_entry);
    reducer
        .step(Event::Signed {
            tag: reducer.current_tag(),
            signature: signature(4),
        })
        .expect("the acknowledged timeout vote is signed");
    let validation = reducer
        .step(Event::ValidationCompleted {
            tag: reducer.current_tag(),
            round: original_round,
            subject,
            valid: true,
        })
        .expect("the old-origin validation remains a harmless recovery completion");
    assert_eq!(
        validation.disposition(),
        StepDisposition::Ignored(IgnoreReason::IrrelevantView)
    );
    assert!(validation.effects().is_empty());
    assert_eq!(reducer.progress_witness_violation(), None);
    assert_eq!(
        reducer
            .durable_state()
            .commit_intent(first_reproposal_round),
        None,
        "validating an old proposal origin never creates a later-round Commit"
    );
    let retry_timeout = tc_with_high(&context, 1, original_prepare.clone(), &[1, 2, 3]);
    let retry_install = only_persist(
        reducer
            .step(Event::TimeoutCertificateReceived {
                tag: reducer.current_tag(),
                certificate: retry_timeout.clone(),
            })
            .expect("the next TC preserves the exact locked subject"),
    );
    let entered = acknowledge(&mut reducer, &retry_install);
    assert_eq!(reducer.current_tag().view(), retry_reproposal_round.view());
    assert!(entered.effects().iter().any(|effect| matches!(
        effect,
        Effect::FetchBody {
            round,
            subject: fetched_subject,
            certificate: Some(certificate),
            ..
        } if *round == original_round
            && *fetched_subject == subject
            && certificate.reference() == original_prepare.reference()
    )));
    let received = reducer
        .step(Event::ProposalReceived {
            tag: reducer.current_tag(),
            proposal: proposal(
                &context,
                retry_reproposal_round.view(),
                subject,
                ProposalJustification::Timeout(retry_timeout),
            ),
        })
        .expect("the later leader re-proposes the locked subject unchanged");
    assert!(matches!(
        received.effects(),
        [Effect::FetchBody {
            round,
            subject: fetched_subject,
            ..
        }] if *round == retry_reproposal_round && *fetched_subject == subject
    ));
    assert!(matches!(
        reducer
            .step(Event::BodyAvailable {
                tag: reducer.current_tag(),
                round: retry_reproposal_round,
                subject,
            })
            .expect("recover the later reproposal body")
            .effects(),
        [Effect::StoreBody { .. }]
    ));
    assert!(matches!(
        reducer
            .step(Event::BodyStored {
                tag: reducer.current_tag(),
                round: retry_reproposal_round,
                subject,
            })
            .expect("store the later reproposal body")
            .effects(),
        [Effect::ValidateBody { .. }]
    ));
    let prepare_entry = only_persist(
        reducer
            .step(Event::ValidationCompleted {
                tag: reducer.current_tag(),
                round: retry_reproposal_round,
                subject,
                valid: true,
            })
            .expect("validation starts the later same-round Prepare intent"),
    );
    assert!(matches!(
        prepare_entry.record(),
        WalRecord::PrepareIntent(vote)
            if vote.round() == retry_reproposal_round
                && vote.proposal_round() == retry_reproposal_round
    ));
    acknowledge(&mut reducer, &prepare_entry);
    reducer
        .step(Event::Signed {
            tag: reducer.current_tag(),
            signature: signature(5),
        })
        .expect("sign and self-admit the later same-round Prepare");
    let later_prepare = qc(&context, 2, Phase::Prepare, subject, &[1, 2, 3]);
    let commit_entry = only_persist(
        reducer
            .step(Event::QuorumCertificateReceived {
                tag: reducer.current_tag(),
                certificate: later_prepare.clone(),
            })
            .expect("the later same-round PrepareQC authorizes Commit"),
    );
    let retry_commit = Vote::new(
        context.id(),
        retry_reproposal_round,
        Phase::Commit,
        subject,
        id(4),
    );
    assert!(matches!(
        commit_entry.record(),
        WalRecord::LockAndCommit { prepare: record, vote }
            if record.reference() == later_prepare.reference()
                && vote.proposal_round() == retry_reproposal_round
                && *vote == retry_commit
    ));
    let persisted = acknowledge(&mut reducer, &commit_entry);
    assert!(matches!(
        persisted.effects(),
        [Effect::Sign {
            message: SignableMessage::Vote(vote),
            ..
        }] if *vote == retry_commit
    ));
    assert_eq!(reducer.progress_witness_violation(), None);
}
#[test]
fn tc_promoted_missing_lock_can_acknowledge_a_successor_view_timeout() {
    let context = context();
    let subject = Subject::repeat(0x9D);
    let prepare = qc(&context, 0, Phase::Prepare, subject, &[1, 2, 3]);
    let timeout = tc_with_high(&context, 0, prepare, &[1, 2, 3]);
    let mut reducer = Reducer::new(context.clone(), Some(id(4)), Generation::new(50)).unwrap();
    let install = only_persist(
        reducer
            .step(Event::TimeoutCertificateReceived {
                tag: reducer.current_tag(),
                certificate: timeout,
            })
            .expect("the TC promotes its PrepareQC while the protected body is missing"),
    );
    acknowledge(&mut reducer, &install);
    assert_eq!(reducer.current_tag().view(), 1);
    assert_eq!(
        reducer.body_state(Round::new(context.height(), 0), subject),
        BodyState::Missing
    );
    let timeout_entry = only_persist(
        reducer
            .step(Event::TimeoutElapsed {
                tag: reducer.current_tag(),
            })
            .expect("the successor view starts its durable timeout"),
    );
    let timeout_sign = acknowledge(&mut reducer, &timeout_entry);
    assert!(matches!(
        timeout_sign.effects(),
        [Effect::Sign {
            message: SignableMessage::TimeoutVote(vote),
            ..
        }] if vote.round() == Round::new(context.height(), 1)
    ));
}
#[test]
fn retained_high_without_lock_can_acknowledge_a_successor_view_timeout() {
    let context = context();
    let subject = Subject::repeat(0x9E);
    let prepare = qc(&context, 0, Phase::Prepare, subject, &[1, 2, 3]);
    let mut reducer = Reducer::new(context.clone(), Some(id(4)), Generation::new(51)).unwrap();
    let first_timeout_entry = only_persist(
        reducer
            .step(Event::TimeoutElapsed {
                tag: reducer.current_tag(),
            })
            .expect("the initial view starts its durable timeout"),
    );
    acknowledge(&mut reducer, &first_timeout_entry);
    complete_signature(&mut reducer, 4);
    let observed = reducer
        .step(Event::QuorumCertificateReceived {
            tag: reducer.current_tag(),
            certificate: prepare.clone(),
        })
        .expect("the closed view durably retains its high PrepareQC");
    let observe_entry = observed
        .effects()
        .iter()
        .find_map(|effect| match effect {
            Effect::Persist { entry, .. } => Some(entry.clone()),
            _ => None,
        })
        .expect("observing the first PrepareQC starts one WAL append");
    acknowledge(&mut reducer, &observe_entry);
    let timeout = tc_without_high(&context, 0, &[1, 2, 3]);
    let install = only_persist(
        reducer
            .step(Event::TimeoutCertificateReceived {
                tag: reducer.current_tag(),
                certificate: timeout.clone(),
            })
            .expect("the TC without a carried high QC advances the view"),
    );
    acknowledge(&mut reducer, &install);
    assert_eq!(reducer.current_tag().view(), 1);
    assert_eq!(reducer.durable_state().highest_prepare(), Some(&prepare));
    assert_eq!(reducer.durable_state().locked(), None);
    assert_eq!(
        reducer.body_state(prepare.round(), subject),
        BodyState::Missing
    );
    let candidate = proposal(
        &context,
        1,
        subject,
        ProposalJustification::Timeout(timeout),
    );
    assert!(matches!(
        reducer
            .step(Event::ProposalReceived {
                tag: reducer.current_tag(),
                proposal: candidate,
            })
            .expect("the successor leader re-proposes while its body is unavailable")
            .effects(),
        [Effect::FetchBody {
            round,
            subject: fetched_subject,
            ..
        }] if *round == Round::new(context.height(), 1) && *fetched_subject == subject
    ));
    let successor_round = Round::new(context.height(), 1);
    assert!(matches!(
        reducer
            .step(Event::BodyAvailable {
                tag: reducer.current_tag(),
                round: successor_round,
                subject,
            })
            .expect("recover the current-generation successor body")
            .effects(),
        [Effect::StoreBody { .. }]
    ));
    assert!(matches!(
        reducer
            .step(Event::BodyStored {
                tag: reducer.current_tag(),
                round: successor_round,
                subject,
            })
            .expect("durably store the current-generation successor body")
            .effects(),
        [Effect::ValidateBody { .. }]
    ));
    let prepare_entry = only_persist(
        reducer
            .step(Event::ValidationCompleted {
                tag: reducer.current_tag(),
                round: successor_round,
                subject,
                valid: true,
            })
            .expect("validation starts the successor Prepare intent"),
    );
    assert!(matches!(
        prepare_entry.record(),
        WalRecord::PrepareIntent(vote)
            if vote.round() == successor_round && vote.subject() == subject
    ));
    acknowledge(&mut reducer, &prepare_entry);
    complete_signature(&mut reducer, 5);
    let timeout_entry = only_persist(
        reducer
            .step(Event::TimeoutElapsed {
                tag: reducer.current_tag(),
            })
            .expect("the successor view starts its durable timeout with the retained high QC"),
    );
    assert!(matches!(
        timeout_entry.record(),
        WalRecord::TimeoutIntent(vote)
            if vote.round() == Round::new(context.height(), 1)
                && vote.highest_prepare() == Some(&prepare)
    ));
    let timeout_sign = acknowledge(&mut reducer, &timeout_entry);
    assert!(matches!(
        timeout_sign.effects(),
        [Effect::Sign {
            message: SignableMessage::TimeoutVote(vote),
            ..
        }] if vote.round() == Round::new(context.height(), 1)
            && vote.highest_prepare() == Some(&prepare)
    ));
}
#[test]
fn retained_high_without_lock_can_sign_a_successor_view_timeout() {
    let context = context();
    let subject = Subject::repeat(0x9F);
    let prepare = qc(&context, 0, Phase::Prepare, subject, &[1, 2, 3]);
    let mut reducer = Reducer::new(context.clone(), Some(id(4)), Generation::new(52)).unwrap();
    let original_tag = reducer.current_tag();
    let first_timeout_entry = only_persist(
        reducer
            .step(Event::TimeoutElapsed {
                tag: reducer.current_tag(),
            })
            .expect("the initial view starts its durable timeout"),
    );
    acknowledge(&mut reducer, &first_timeout_entry);
    complete_signature(&mut reducer, 4);
    let observed = reducer
        .step(Event::QuorumCertificateReceived {
            tag: reducer.current_tag(),
            certificate: prepare.clone(),
        })
        .expect("the closed view durably retains its high PrepareQC");
    let observe_entry = observed
        .effects()
        .iter()
        .find_map(|effect| match effect {
            Effect::Persist { entry, .. } => Some(entry.clone()),
            _ => None,
        })
        .expect("observing the first PrepareQC starts one WAL append");
    acknowledge(&mut reducer, &observe_entry);
    let timeout = tc_without_high(&context, 0, &[1, 2, 3]);
    let install = only_persist(
        reducer
            .step(Event::TimeoutCertificateReceived {
                tag: reducer.current_tag(),
                certificate: timeout.clone(),
            })
            .expect("the TC without a carried high QC advances the view"),
    );
    acknowledge(&mut reducer, &install);
    let successor_round = Round::new(context.height(), 1);
    let candidate = proposal(
        &context,
        successor_round.view(),
        subject,
        ProposalJustification::Timeout(timeout.clone()),
    );
    assert!(matches!(
        reducer
            .step(Event::ProposalReceived {
                tag: reducer.current_tag(),
                proposal: candidate.clone(),
            })
            .expect("the successor proposal starts body recovery")
            .effects(),
        [Effect::FetchBody { round, .. }] if *round == successor_round
    ));
    let current_tag = reducer.current_tag();
    let ingress_matrix = [
        ("retransmit", Event::RetransmitElapsed { tag: current_tag }),
        (
            "duplicate successor proposal",
            Event::ProposalReceived {
                tag: current_tag,
                proposal: candidate,
            },
        ),
        (
            "conflicting successor proposal",
            Event::ProposalReceived {
                tag: current_tag,
                proposal: proposal(
                    &context,
                    successor_round.view(),
                    Subject::repeat(0xA0),
                    ProposalJustification::Timeout(timeout.clone()),
                ),
            },
        ),
        (
            "retained PrepareQC",
            Event::QuorumCertificateReceived {
                tag: current_tag,
                certificate: prepare.clone(),
            },
        ),
        (
            "current-view PrepareQC",
            Event::QuorumCertificateReceived {
                tag: current_tag,
                certificate: qc(&context, 1, Phase::Prepare, subject, &[1, 2, 3]),
            },
        ),
        (
            "current-view CommitQC",
            Event::QuorumCertificateReceived {
                tag: current_tag,
                certificate: qc(&context, 1, Phase::Commit, subject, &[1, 2, 3]),
            },
        ),
        (
            "installed timeout certificate",
            Event::TimeoutCertificateReceived {
                tag: current_tag,
                certificate: timeout.clone(),
            },
        ),
        (
            "current-view TimeoutVote",
            Event::TimeoutVoteReceived {
                tag: current_tag,
                vote: SignedTimeoutVote::new(
                    TimeoutVote::new(context.id(), successor_round, id(1), Some(prepare.clone())),
                    signature(1),
                ),
            },
        ),
        (
            "current-view TimeoutVote without a high QC",
            Event::TimeoutVoteReceived {
                tag: current_tag,
                vote: SignedTimeoutVote::new(
                    TimeoutVote::new(context.id(), successor_round, id(1), None),
                    signature(1),
                ),
            },
        ),
        (
            "current-generation BodyAvailable",
            Event::BodyAvailable {
                tag: current_tag,
                round: successor_round,
                subject,
            },
        ),
        (
            "premature current-generation BodyStored",
            Event::BodyStored {
                tag: current_tag,
                round: successor_round,
                subject,
            },
        ),
        (
            "premature current-generation validation",
            Event::ValidationCompleted {
                tag: current_tag,
                round: successor_round,
                subject,
                valid: true,
            },
        ),
        (
            "current-generation old-round BodyAvailable",
            Event::BodyAvailable {
                tag: current_tag,
                round: Round::new(context.height(), 0),
                subject,
            },
        ),
        (
            "current-generation old-round BodyStored",
            Event::BodyStored {
                tag: current_tag,
                round: Round::new(context.height(), 0),
                subject,
            },
        ),
        (
            "current-generation old-round validation",
            Event::ValidationCompleted {
                tag: current_tag,
                round: Round::new(context.height(), 0),
                subject,
                valid: true,
            },
        ),
        (
            "stale-generation BodyAvailable",
            Event::BodyAvailable {
                tag: original_tag,
                round: Round::new(context.height(), 0),
                subject,
            },
        ),
    ];
    for (case, event) in ingress_matrix {
        reducer
            .clone()
            .step(event)
            .unwrap_or_else(|error| panic!("{case} must pass the refinement gate: {error}"));
    }
    let mut formed_prepare = reducer.clone();
    for signer in [1_u8, 2, 3] {
        formed_prepare
            .step(Event::VoteReceived {
                tag: current_tag,
                vote: SignedVote::new(
                    Vote::new(
                        context.id(),
                        successor_round,
                        Phase::Prepare,
                        subject,
                        id(signer),
                    ),
                    signature(signer),
                ),
            })
            .unwrap_or_else(|error| {
                panic!("current-view Prepare vote {signer} must pass refinement: {error}")
            });
    }
    for carried_high in [false, true] {
        let mut formed_timeout = reducer.clone();
        let mut install = None;
        for signer in [1_u8, 2, 3] {
            let highest = (carried_high && signer != 1).then(|| prepare.clone());
            let outcome = formed_timeout
                .step(Event::TimeoutVoteReceived {
                    tag: current_tag,
                    vote: SignedTimeoutVote::new(
                        TimeoutVote::new(context.id(), successor_round, id(signer), highest),
                        signature(signer),
                    ),
                })
                .unwrap_or_else(|error| {
                    panic!(
                        "current-view Timeout vote {signer} with carried_high={carried_high} \
                         must pass refinement: {error}"
                    )
                });
            if signer == 3 {
                install = Some(only_persist(outcome));
            }
        }
        let entered = acknowledge(
            &mut formed_timeout,
            &install.expect("the third Timeout vote forms a TC"),
        );
        assert_eq!(formed_timeout.current_tag().view(), 2);
        let effects = entered.effects();
        assert!(matches!(
            effects.first(),
            Some(Effect::EnterView { tag, .. }) if tag.view() == 2
        ));
        assert!(matches!(
            effects.last(),
            Some(Effect::Broadcast(ConsensusMessageV2::TimeoutCertificate(certificate)))
                if certificate.round() == successor_round
        ));
        if carried_high {
            assert!(matches!(effects.get(1), Some(Effect::FetchBody { .. })));
        } else {
            assert_eq!(effects.len(), 2);
        }
    }
    let timeout_entry = only_persist(
        reducer
            .step(Event::TimeoutElapsed {
                tag: reducer.current_tag(),
            })
            .expect("the successor view starts its durable timeout"),
    );
    acknowledge(&mut reducer, &timeout_entry);
    let signed = complete_signature(&mut reducer, 5);
    assert!(matches!(
        signed.effects(),
        [Effect::Broadcast(ConsensusMessageV2::TimeoutVote(vote))]
            if vote.vote().round() == successor_round
                && vote.vote().highest_prepare() == Some(&prepare)
    ));
    let successor_tc = tc_with_high(
        &context,
        successor_round.view(),
        prepare.clone(),
        &[1, 2, 3],
    );
    let install = only_persist(
        reducer
            .step(Event::TimeoutCertificateReceived {
                tag: reducer.current_tag(),
                certificate: successor_tc,
            })
            .expect("a successor TC can carry the exact retained high QC"),
    );
    let entered = acknowledge(&mut reducer, &install);
    assert_eq!(reducer.current_tag().view(), 2);
    assert_eq!(reducer.durable_state().locked(), Some(&prepare));
    assert!(matches!(
        entered.effects(),
        [
            Effect::EnterView { .. },
            Effect::FetchBody {
                round,
                subject: fetched_subject,
                ..
            }
        ] if *round == prepare.round() && *fetched_subject == subject
    ));
}
#[test]
fn replay_rejects_later_view_commit_even_after_exact_tc_lock_installation() {
    let context = context();
    let subject = Subject::repeat(0x97);
    let proposal_round = Round::new(context.height(), 0);
    let finality_round = Round::new(context.height(), 1);
    let prepare = qc(&context, 0, Phase::Prepare, subject, &[1, 2, 3]);
    let local_commit = Vote::new_with_proposal_round(
        context.id(),
        finality_round,
        proposal_round,
        Phase::Commit,
        subject,
        id(4),
    );
    let timeout = TimeoutVote::new(context.id(), proposal_round, id(4), None);
    let install = tc_with_high(&context, 0, prepare.clone(), &[1, 2, 3]);
    let entries = [
        WalEntry::new(PersistenceId::new(1), WalRecord::TimeoutIntent(timeout)),
        WalEntry::new(
            PersistenceId::new(2),
            WalRecord::InstallTimeout(install.clone()),
        ),
        WalEntry::new(
            PersistenceId::new(3),
            WalRecord::LockAndCommit {
                prepare: prepare.clone(),
                vote: local_commit,
            },
        ),
    ];
    assert!(matches!(
        DurableState::replay(&context, Some(id(4)), entries.clone()),
        Err(ReplayError::InvalidLocalVote)
    ));
    let unlocked_entries = [
        entries[0].clone(),
        WalEntry::new(
            PersistenceId::new(2),
            WalRecord::InstallTimeout(tc_without_high(&context, 0, &[1, 2, 3])),
        ),
        entries[2].clone(),
    ];
    assert!(matches!(
        DurableState::replay(&context, Some(id(4)), unlocked_entries),
        Err(ReplayError::InvalidLocalVote)
    ));
    let different_subject = Subject::repeat(0x98);
    let different_prepare = qc(&context, 1, Phase::Prepare, different_subject, &[1, 2, 3]);
    let mismatched_entries = [
        entries[0].clone(),
        WalEntry::new(
            PersistenceId::new(2),
            WalRecord::InstallTimeout(install.clone()),
        ),
        WalEntry::new(
            PersistenceId::new(3),
            WalRecord::LockAndCommit {
                prepare: different_prepare,
                vote: Vote::new(context.id(), finality_round, Phase::Commit, subject, id(4)),
            },
        ),
    ];
    assert!(matches!(
        DurableState::replay(&context, Some(id(4)), mismatched_entries),
        Err(ReplayError::CommitDoesNotMatchPrepare)
    ));
    let current_prepare = qc(&context, 1, Phase::Prepare, subject, &[1, 2, 3]);
    let current_commit = Vote::new(context.id(), finality_round, Phase::Commit, subject, id(4));
    let decision_first_entries = [
        entries[0].clone(),
        WalEntry::new(PersistenceId::new(2), WalRecord::InstallTimeout(install)),
        WalEntry::new(
            PersistenceId::new(3),
            WalRecord::Decision(qc(&context, 0, Phase::Commit, subject, &[1, 2, 3])),
        ),
        WalEntry::new(
            PersistenceId::new(4),
            WalRecord::LockAndCommit {
                prepare: current_prepare,
                vote: current_commit,
            },
        ),
    ];
    assert!(matches!(
        DurableState::replay(&context, Some(id(4)), decision_first_entries),
        Err(ReplayError::InvalidLocalVote)
    ));
}
#[test]
fn replay_resigns_same_subject_reproposal_fifo_without_relabelling_old_commit() {
    let context = context();
    let subject = Subject::repeat(0x98);
    let original_round = Round::new(context.height(), 0);
    let reproposal_round = Round::new(context.height(), 1);
    let local = context.leader(reproposal_round.view());
    let original_prepare = qc(&context, 0, Phase::Prepare, subject, &[1, 2, 3]);
    let original_commit = Vote::new(context.id(), original_round, Phase::Commit, subject, local);
    let timeout = tc_with_high(
        &context,
        original_round.view(),
        original_prepare.clone(),
        &[1, 2, 3],
    );
    let reproposal = Proposal::new(
        context.id(),
        reproposal_round,
        local,
        PayloadManifest::new(subject, Digest::repeat(0x98), Digest::repeat(0x99), 128, 2),
        ProposalJustification::Timeout(timeout.clone()),
    );
    let local_prepare = Vote::new(
        context.id(),
        reproposal_round,
        Phase::Prepare,
        subject,
        local,
    );
    let reproposal_prepare = qc(&context, 1, Phase::Prepare, subject, &[1, 2, 3]);
    let reproposal_commit = Vote::new(
        context.id(),
        reproposal_round,
        Phase::Commit,
        subject,
        local,
    );
    let entries = [
        WalEntry::new(
            PersistenceId::new(1),
            WalRecord::LockAndCommit {
                prepare: original_prepare,
                vote: original_commit,
            },
        ),
        WalEntry::new(PersistenceId::new(2), WalRecord::InstallTimeout(timeout)),
        WalEntry::new(
            PersistenceId::new(3),
            WalRecord::ProposalIntent(reproposal.clone()),
        ),
        WalEntry::new(
            PersistenceId::new(4),
            WalRecord::PrepareIntent(local_prepare),
        ),
        WalEntry::new(
            PersistenceId::new(5),
            WalRecord::LockAndCommit {
                prepare: reproposal_prepare.clone(),
                vote: reproposal_commit,
            },
        ),
    ];
    let mut recovered = Reducer::recover(context, Some(local), Generation::new(50), entries)
        .expect("recover the old Commit and the complete same-subject reproposal path");
    assert_eq!(
        recovered.durable_state().commit_intent(original_round),
        Some(original_commit),
        "the old same-round Commit remains immutable WAL history"
    );
    assert_eq!(
        recovered.durable_state().commit_intent(reproposal_round),
        Some(reproposal_commit)
    );
    assert_eq!(
        recovered
            .durable_state()
            .locked()
            .map(QuorumCertificate::reference),
        Some(reproposal_prepare.reference())
    );
    let proposal_message = SignableMessage::Proposal(reproposal);
    let prepare_message = SignableMessage::Vote(local_prepare);
    let commit_message = SignableMessage::Vote(reproposal_commit);
    assert!(matches!(
        resume_after_replay(&mut recovered).effects(),
        [Effect::Sign {
            message,
            ..
        }] if message == &proposal_message
    ));
    assert_signature_frontier(
        &recovered,
        Some(&proposal_message),
        &[prepare_message, commit_message],
    );
    assert_eq!(recovered.progress_witness_violation(), None);
}
#[test]
fn higher_conflicting_prepare_intent_fences_historical_commit_reconstruction() {
    let context = context();
    let locked_subject = Subject::repeat(0x99);
    let conflicting_subject = Subject::repeat(0x9A);
    let locked_round = Round::new(context.height(), 0);
    let higher_round = Round::new(context.height(), 1);
    let finality_round = Round::new(context.height(), 2);
    let locked_prepare = qc(&context, 0, Phase::Prepare, locked_subject, &[1, 2, 3]);
    let higher_prepare_vote = Vote::new(
        context.id(),
        higher_round,
        Phase::Prepare,
        conflicting_subject,
        id(4),
    );
    let historical_commit = Vote::new_with_proposal_round(
        context.id(),
        finality_round,
        locked_round,
        Phase::Commit,
        locked_subject,
        id(4),
    );
    let prefix = [
        WalEntry::new(
            PersistenceId::new(1),
            WalRecord::InstallTimeout(tc_without_high(&context, 0, &[1, 2, 3])),
        ),
        WalEntry::new(
            PersistenceId::new(2),
            WalRecord::PrepareIntent(higher_prepare_vote),
        ),
        WalEntry::new(
            PersistenceId::new(3),
            WalRecord::InstallTimeout(tc_with_high(
                &context,
                1,
                locked_prepare.clone(),
                &[1, 2, 3],
            )),
        ),
    ];
    let mut replay = prefix.to_vec();
    replay.push(WalEntry::new(
        PersistenceId::new(4),
        WalRecord::LockAndCommit {
            prepare: locked_prepare.clone(),
            vote: historical_commit,
        },
    ));
    assert!(matches!(
        DurableState::replay(&context, Some(id(4)), replay),
        Err(ReplayError::InvalidLocalVote)
    ));
    let higher_conflicting_qc = qc(&context, 1, Phase::Prepare, conflicting_subject, &[1, 2, 3]);
    let higher_qc_replay = [
        WalEntry::new(
            PersistenceId::new(1),
            WalRecord::InstallTimeout(tc_without_high(&context, 0, &[1, 2, 3])),
        ),
        WalEntry::new(
            PersistenceId::new(2),
            WalRecord::ObservePrepare(higher_conflicting_qc),
        ),
        WalEntry::new(
            PersistenceId::new(3),
            WalRecord::InstallTimeout(tc_with_high(
                &context,
                1,
                locked_prepare.clone(),
                &[1, 2, 3],
            )),
        ),
        WalEntry::new(
            PersistenceId::new(4),
            WalRecord::LockAndCommit {
                prepare: locked_prepare.clone(),
                vote: historical_commit,
            },
        ),
    ];
    assert!(matches!(
        DurableState::replay(&context, Some(id(4)), higher_qc_replay),
        Err(ReplayError::InvalidLocalVote)
    ));
    let mut reducer = Reducer::recover(context.clone(), Some(id(4)), Generation::new(50), prefix)
        .expect("recover the higher conflicting Prepare and TC-promoted old lock");
    resume_after_replay(&mut reducer);
    let fetch = reducer
        .step(Event::RetransmitElapsed {
            tag: reducer.current_tag(),
        })
        .expect("the old lock retains body recovery even when its late Commit is fenced");
    assert!(fetch.effects().iter().any(|effect| matches!(
        effect,
        Effect::FetchBody {
            round,
            subject,
            certificate: Some(certificate),
            ..
        } if *round == locked_round
            && *subject == locked_subject
            && certificate.reference() == locked_prepare.reference()
    )));
    reducer
        .step(Event::BodyAvailable {
            tag: reducer.current_tag(),
            round: locked_round,
            subject: locked_subject,
        })
        .expect("recover the exact old locked body");
    reducer
        .step(Event::BodyStored {
            tag: reducer.current_tag(),
            round: locked_round,
            subject: locked_subject,
        })
        .expect("store the exact old locked body");
    let validation = reducer
        .step(Event::ValidationCompleted {
            tag: reducer.current_tag(),
            round: locked_round,
            subject: locked_subject,
            valid: true,
        })
        .expect("validation keeps the temporal-order fence non-fatal");
    assert_eq!(
        validation.disposition(),
        StepDisposition::Ignored(IgnoreReason::IrrelevantView)
    );
    assert!(validation.effects().is_empty());
    assert_eq!(reducer.durable_state().commit_intent(finality_round), None);
    assert_eq!(reducer.progress_witness_violation(), None);
}
#[test]
fn higher_same_subject_prepare_fences_historical_commit_reconstruction() {
    let context = context();
    let subject = Subject::repeat(0x9B);
    let locked_round = Round::new(context.height(), 0);
    let higher_round = Round::new(context.height(), 1);
    let finality_round = Round::new(context.height(), 2);
    let locked_prepare = qc(&context, 0, Phase::Prepare, subject, &[1, 2, 3]);
    let historical_commit = Vote::new_with_proposal_round(
        context.id(),
        finality_round,
        locked_round,
        Phase::Commit,
        subject,
        id(4),
    );
    let entries = [
        WalEntry::new(
            PersistenceId::new(1),
            WalRecord::InstallTimeout(tc_without_high(&context, 0, &[1, 2, 3])),
        ),
        WalEntry::new(
            PersistenceId::new(2),
            WalRecord::PrepareIntent(Vote::new(
                context.id(),
                higher_round,
                Phase::Prepare,
                subject,
                id(4),
            )),
        ),
        WalEntry::new(
            PersistenceId::new(3),
            WalRecord::ObservePrepare(qc(&context, 1, Phase::Prepare, subject, &[1, 2, 3])),
        ),
        WalEntry::new(
            PersistenceId::new(4),
            WalRecord::InstallTimeout(tc_with_high(
                &context,
                1,
                locked_prepare.clone(),
                &[1, 2, 3],
            )),
        ),
        WalEntry::new(
            PersistenceId::new(5),
            WalRecord::LockAndCommit {
                prepare: locked_prepare,
                vote: historical_commit,
            },
        ),
    ];
    assert!(matches!(
        DurableState::replay(&context, Some(id(4)), entries),
        Err(ReplayError::InvalidLocalVote)
    ));
}
#[test]
fn replay_does_not_resign_proposal_superseded_by_same_round_lock() {
    let context = context();
    let round = Round::new(context.height(), 0);
    let local = context.leader(round.view());
    let stale_subject = Subject::repeat(0x95);
    let locked_subject = Subject::repeat(0x96);
    let stale_proposal = Proposal::new(
        context.id(),
        round,
        local,
        PayloadManifest::new(
            stale_subject,
            Digest::repeat(0x61),
            Digest::repeat(0x62),
            128,
            2,
        ),
        ProposalJustification::ParentCommit(context.parent_commit()),
    );
    let locked_prepare = qc(
        &context,
        round.view(),
        Phase::Prepare,
        locked_subject,
        &[1, 2, 3],
    );
    let locked_commit = Vote::new(context.id(), round, Phase::Commit, locked_subject, local);
    let mut reducer = Reducer::recover(
        context,
        Some(local),
        Generation::new(47),
        [
            WalEntry::new(
                PersistenceId::new(1),
                WalRecord::ProposalIntent(stale_proposal.clone()),
            ),
            WalEntry::new(
                PersistenceId::new(2),
                WalRecord::LockAndCommit {
                    prepare: locked_prepare.clone(),
                    vote: locked_commit,
                },
            ),
        ],
    )
    .expect("recover a proposal intent superseded by the exact same-round lock");
    assert_eq!(
        reducer.durable_state().locked(),
        Some(&locked_prepare),
        "the PrepareQC, not the earlier proposal intent, owns replay progress"
    );
    let resumed = resume_after_replay(&mut reducer);
    assert!(resumed.effects().iter().all(|effect| {
        !matches!(
            effect,
            Effect::Sign {
                message: SignableMessage::Proposal(proposal),
                ..
            } if proposal == &stale_proposal
        )
    }));
    assert!(matches!(
        resumed.effects(),
        [Effect::Sign {
            message: SignableMessage::Vote(vote),
            ..
        }] if *vote == locked_commit
    ));
}
#[test]
fn replay_does_not_resign_commit_superseded_by_higher_tc_lock() {
    let context = context();
    let old_subject = Subject::repeat(0x93);
    let higher_subject = Subject::repeat(0x94);
    let old_prepare = qc(&context, 0, Phase::Prepare, old_subject, &[1, 2, 3]);
    let old_commit = Vote::new(
        context.id(),
        old_prepare.round(),
        Phase::Commit,
        old_subject,
        id(1),
    );
    let higher_prepare = qc(&context, 1, Phase::Prepare, higher_subject, &[1, 2, 3]);
    let mut reducer = Reducer::recover(
        context.clone(),
        Some(id(1)),
        Generation::new(46),
        [
            WalEntry::new(
                PersistenceId::new(1),
                WalRecord::LockAndCommit {
                    prepare: old_prepare,
                    vote: old_commit,
                },
            ),
            WalEntry::new(
                PersistenceId::new(2),
                WalRecord::InstallTimeout(tc_without_high(&context, 0, &[1, 2, 3])),
            ),
            WalEntry::new(
                PersistenceId::new(3),
                WalRecord::ObservePrepare(higher_prepare.clone()),
            ),
            WalEntry::new(
                PersistenceId::new(4),
                WalRecord::InstallTimeout(tc_with_high(
                    &context,
                    1,
                    higher_prepare.clone(),
                    &[1, 2, 3],
                )),
            ),
        ],
    )
    .expect("recover the TC-promoted lock");
    assert_eq!(
        reducer
            .durable_state()
            .locked()
            .map(QuorumCertificate::reference),
        Some(higher_prepare.reference())
    );
    let resumed = resume_after_replay(&mut reducer);
    assert!(
        resumed.effects().is_empty(),
        "the immutable old Commit record is no longer an active signing intent"
    );
    assert_eq!(
        reducer.durable_state().commit_intent(old_commit.round()),
        Some(old_commit)
    );
    assert!(reducer.awaiting_signature().is_none());
    assert_eq!(reducer.queued_signatures().count(), 0);
    assert_eq!(reducer.progress_witness_violation(), None);
    let retransmit = reducer
        .step(Event::RetransmitElapsed {
            tag: reducer.current_tag(),
        })
        .expect("recovered promoted lock retains a body reconstruction path");
    assert!(retransmit.effects().iter().all(|effect| {
        !matches!(
            effect,
            Effect::Broadcast(ConsensusMessageV2::Vote(vote)) if vote.vote() == old_commit
        )
    }));
    assert!(retransmit.effects().iter().any(|effect| {
        matches!(
            effect,
            Effect::FetchBody {
                subject,
                certificate: Some(certificate),
                ..
            } if *subject == higher_subject && certificate.reference() == higher_prepare.reference()
        )
    }));
}
#[test]
fn prior_view_commit_vote_for_unlocked_prepare_is_rejected() {
    let context = context();
    let locked_subject = Subject::repeat(0x73);
    let unlocked_subject = Subject::repeat(0x74);
    let locked_prepare = qc(&context, 0, Phase::Prepare, locked_subject, &[1, 2, 3]);
    let unlocked_prepare = qc(&context, 1, Phase::Prepare, unlocked_subject, &[1, 2, 3]);
    let entries = [
        WalEntry::new(
            PersistenceId::new(1),
            WalRecord::LockAndCommit {
                prepare: locked_prepare,
                vote: Vote::new(
                    context.id(),
                    Round::new(context.height(), 0),
                    Phase::Commit,
                    locked_subject,
                    id(1),
                ),
            },
        ),
        WalEntry::new(
            PersistenceId::new(2),
            WalRecord::InstallTimeout(tc_without_high(&context, 0, &[1, 2, 3])),
        ),
        WalEntry::new(
            PersistenceId::new(3),
            WalRecord::ObservePrepare(unlocked_prepare),
        ),
        WalEntry::new(
            PersistenceId::new(4),
            WalRecord::InstallTimeout(tc_without_high(&context, 1, &[1, 2, 3])),
        ),
    ];
    let mut reducer = Reducer::recover(context.clone(), Some(id(1)), Generation::new(44), entries)
        .expect("higher observed PrepareQC does not replace the durable lock");
    assert_eq!(reducer.current_tag().view(), 2);
    assert!(matches!(
        resume_after_replay(&mut reducer).effects(),
        [Effect::Sign {
            message: SignableMessage::Vote(vote),
            ..
        }] if vote.phase() == Phase::Commit && vote.subject() == locked_subject
    ));
    reducer
        .step(Event::Signed {
            tag: reducer.current_tag(),
            signature: signature(1),
        })
        .expect("finish replaying the exact durable Commit intent");
    let outcome = reducer
        .step(Event::VoteReceived {
            tag: reducer.current_tag(),
            vote: SignedVote::new(
                Vote::new(
                    context.id(),
                    Round::new(context.height(), 1),
                    Phase::Commit,
                    unlocked_subject,
                    id(2),
                ),
                signature(2),
            ),
        })
        .expect("unlocked historical CommitVote is harmless");
    assert_eq!(
        outcome.disposition(),
        StepDisposition::Ignored(IgnoreReason::IrrelevantView)
    );
    assert!(outcome.effects().is_empty());
}
#[test]
fn current_view_commit_waits_for_the_exact_durable_lock() {
    let context = context();
    let old_subject = Subject::repeat(0x9a);
    let current_subject = Subject::repeat(0x9b);
    let old_round = Round::new(context.height(), 0);
    let current_round = Round::new(context.height(), 1);
    let old_prepare = qc(&context, 0, Phase::Prepare, old_subject, &[1, 2, 3]);
    let old_commit = Vote::new(context.id(), old_round, Phase::Commit, old_subject, id(1));
    let mut reducer = Reducer::recover(
        context.clone(),
        Some(id(1)),
        Generation::new(50),
        [
            WalEntry::new(
                PersistenceId::new(1),
                WalRecord::LockAndCommit {
                    prepare: old_prepare,
                    vote: old_commit,
                },
            ),
            WalEntry::new(
                PersistenceId::new(2),
                WalRecord::InstallTimeout(tc_without_high(&context, 0, &[1, 2, 3])),
            ),
        ],
    )
    .expect("recover the historical lock in the next view");
    assert!(matches!(
        resume_after_replay(&mut reducer).effects(),
        [Effect::Sign {
            message: SignableMessage::Vote(vote),
            ..
        }] if *vote == old_commit
    ));
    reducer
        .step(Event::Signed {
            tag: reducer.current_tag(),
            signature: signature(1),
        })
        .expect("rebuild the historical locked Commit pool");
    reducer
        .step(Event::VoteReceived {
            tag: reducer.current_tag(),
            vote: SignedVote::new(
                Vote::new(
                    context.id(),
                    current_round,
                    Phase::Prepare,
                    current_subject,
                    id(2),
                ),
                signature(2),
            ),
        })
        .expect("a current-view Prepare vote enters its bounded pool");
    let premature = reducer
        .step(Event::VoteReceived {
            tag: reducer.current_tag(),
            vote: SignedVote::new(
                Vote::new(
                    context.id(),
                    current_round,
                    Phase::Commit,
                    current_subject,
                    id(2),
                ),
                signature(2),
            ),
        })
        .expect("a Commit vote without an exact durable lock is harmless");
    assert_eq!(
        premature.disposition(),
        StepDisposition::Ignored(IgnoreReason::IrrelevantView)
    );
    assert!(premature.effects().is_empty());
    let pools = reducer.vote_pool_snapshots();
    assert_eq!(pools.len(), 2);
    assert!(pools.iter().any(|pool| {
        pool.round == old_round && pool.phase == Phase::Commit && pool.subject == old_subject
    }));
    assert!(pools.iter().any(|pool| {
        pool.round == current_round
            && pool.phase == Phase::Prepare
            && pool.subject == current_subject
    }));
    let current_prepare = qc(&context, 1, Phase::Prepare, current_subject, &[1, 2, 3]);
    let observed = reducer
        .step(Event::QuorumCertificateReceived {
            tag: reducer.current_tag(),
            certificate: current_prepare.clone(),
        })
        .expect("observe the current PrepareQC before installing its lock");
    let observe_entry = observed
        .effects()
        .iter()
        .find_map(|effect| match effect {
            Effect::Persist { entry, .. } => Some(entry.clone()),
            _ => None,
        })
        .expect("the current PrepareQC is persisted");
    acknowledge(&mut reducer, &observe_entry);
    assert!(matches!(
        reducer
            .step(Event::BodyAvailable {
                tag: reducer.current_tag(),
                round: current_round,
                subject: current_subject,
            })
            .expect("make the certified body available")
            .effects(),
        [Effect::StoreBody { .. }]
    ));
    assert!(matches!(
        reducer
            .step(Event::BodyStored {
                tag: reducer.current_tag(),
                round: current_round,
                subject: current_subject,
            })
            .expect("durably store the certified body")
            .effects(),
        [Effect::ValidateBody { .. }]
    ));
    let lock_entry = only_persist(
        reducer
            .step(Event::ValidationCompleted {
                tag: reducer.current_tag(),
                round: current_round,
                subject: current_subject,
                valid: true,
            })
            .expect("validation starts the exact LockAndCommit append"),
    );
    let pre_ack_pools = reducer.vote_pool_snapshots();
    assert!(pre_ack_pools.iter().any(|pool| {
        pool.round == old_round && pool.phase == Phase::Commit && pool.subject == old_subject
    }));
    assert!(pre_ack_pools.iter().any(|pool| {
        pool.round == current_round
            && pool.phase == Phase::Prepare
            && pool.subject == current_subject
    }));
    let persisted = acknowledge(&mut reducer, &lock_entry);
    assert!(matches!(
        persisted.effects(),
        [Effect::Sign {
            message: SignableMessage::Vote(vote),
            ..
        }] if vote.round() == current_round && vote.subject() == current_subject
    ));
    assert!(
        reducer
            .vote_pool_snapshots()
            .iter()
            .all(|pool| pool.round == current_round)
    );
    reducer
        .step(Event::Signed {
            tag: reducer.current_tag(),
            signature: signature(1),
        })
        .expect("self-admit the now-authorized current Commit vote");
    let admitted = reducer
        .step(Event::VoteReceived {
            tag: reducer.current_tag(),
            vote: SignedVote::new(
                Vote::new(
                    context.id(),
                    current_round,
                    Phase::Commit,
                    current_subject,
                    id(2),
                ),
                signature(2),
            ),
        })
        .expect("the same Commit vote is admissible after exact lock persistence");
    assert_eq!(admitted.disposition(), StepDisposition::Applied);
    assert!(admitted.effects().is_empty());
    assert!(reducer.vote_pool_snapshots().iter().any(|pool| {
        pool.round == current_round
            && pool.phase == Phase::Commit
            && pool.subject == current_subject
            && pool.signers == [id(1), id(2)]
    }));
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
fn decision_persistence_fences_timeout_certificate_view_change() {
    let context = context();
    let mut reducer =
        Reducer::new(context.clone(), Some(id(1)), Generation::new(46)).expect("reducer");
    let original_tag = reducer.current_tag();
    let subject = Subject::repeat(0x92);
    let commit = qc(&context, 0, Phase::Commit, subject, &[1, 2, 3]);
    let decision_entry = only_persist(
        reducer
            .step(Event::QuorumCertificateReceived {
                tag: original_tag,
                certificate: commit.clone(),
            })
            .expect("CommitQC atomically acquires Decision persistence"),
    );
    assert!(matches!(
        reducer.pending_persistence_record(),
        Some(WalRecord::Decision(certificate)) if certificate == &commit
    ));
    let timeout = tc_without_high(&context, 0, &[1, 2, 3]);
    let before_timeout = reducer.clone();
    let busy = reducer
        .step(Event::TimeoutCertificateReceived {
            tag: original_tag,
            certificate: timeout,
        })
        .expect("TC cannot invalidate an admitted Decision persistence owner");
    assert_eq!(
        busy.disposition(),
        StepDisposition::Ignored(IgnoreReason::Busy)
    );
    assert!(busy.effects().is_empty());
    assert_eq!(reducer, before_timeout);
    assert_eq!(reducer.current_tag(), original_tag);
    let decided = acknowledge(&mut reducer, &decision_entry);
    assert!(decided.effects().iter().any(
        |effect| matches!(effect, Effect::FetchBody { subject: value, .. } if *value == subject)
    ));
    assert_eq!(reducer.durable_state().decision(), Some(&commit));
    assert_eq!(reducer.current_tag().view(), original_tag.view());
    assert_eq!(reducer.progress_witness_violation(), None);
}
#[test]
fn commit_qc_preempts_hung_timeout_signature_but_not_pending_wal() {
    let context = context();
    let round = Round::new(context.height(), 0);
    let subject = Subject::repeat(0x93);
    let commit = qc(&context, 0, Phase::Commit, subject, &[1, 2, 3]);
    let mut reducer =
        Reducer::new(context.clone(), Some(id(1)), Generation::new(47)).expect("reducer");
    let original_tag = reducer.current_tag();
    for signer in [2_u8, 3] {
        let admitted = reducer
            .step(Event::TimeoutVoteReceived {
                tag: original_tag,
                vote: SignedTimeoutVote::new(
                    TimeoutVote::new(context.id(), round, id(signer), None),
                    signature(signer),
                ),
            })
            .expect("remote timeout vote is admitted before the local timeout");
        assert_eq!(admitted.disposition(), StepDisposition::Applied);
        assert!(admitted.effects().is_empty());
    }
    let timeout_entry = only_persist(
        reducer
            .step(Event::TimeoutElapsed { tag: original_tag })
            .expect("local timeout starts its durable intent"),
    );
    assert!(matches!(
        reducer.pending_persistence_record(),
        Some(WalRecord::TimeoutIntent(_))
    ));
    let before_timeout_ack = reducer.clone();
    let busy = reducer
        .step(Event::QuorumCertificateReceived {
            tag: original_tag,
            certificate: commit.clone(),
        })
        .expect("CommitQC cannot overtake TimeoutIntent persistence");
    assert_eq!(
        busy.disposition(),
        StepDisposition::Ignored(IgnoreReason::Busy)
    );
    assert!(busy.effects().is_empty());
    assert_eq!(reducer, before_timeout_ack);
    let sign = acknowledge(&mut reducer, &timeout_entry);
    assert!(matches!(
        sign.effects(),
        [Effect::Sign {
            tag,
            message: SignableMessage::TimeoutVote(vote),
        }] if *tag == original_tag && vote.round() == round && vote.signer() == id(1)
    ));
    let prepare = qc(&context, 0, Phase::Prepare, subject, &[1, 2, 3]);
    let before_prepare = reducer.clone();
    let busy = reducer
        .step(Event::QuorumCertificateReceived {
            tag: original_tag,
            certificate: prepare,
        })
        .expect("PrepareQC remains behind the exact signing fence");
    assert_eq!(
        busy.disposition(),
        StepDisposition::Ignored(IgnoreReason::Busy)
    );
    assert!(busy.effects().is_empty());
    assert_eq!(reducer, before_prepare);
    let decision_entry = only_persist(
        reducer
            .step(Event::QuorumCertificateReceived {
                tag: original_tag,
                certificate: commit.clone(),
            })
            .expect("authenticated CommitQC supersedes the hung local signature"),
    );
    assert!(matches!(
        decision_entry.record(),
        WalRecord::Decision(certificate) if certificate == &commit
    ));
    assert!(reducer.awaiting_signature().is_none());
    assert!(reducer.queued_signatures().any(
        |message| matches!(message, SignableMessage::TimeoutVote(vote) if vote.round() == round)
    ));
    assert!(reducer.durable_state().decision().is_none());
    let before_stale_completion = reducer.clone();
    let busy = reducer
        .step(Event::Signed {
            tag: original_tag,
            signature: signature(1),
        })
        .expect("pending Decision WAL still fences the old signature completion");
    assert_eq!(
        busy.disposition(),
        StepDisposition::Ignored(IgnoreReason::Busy)
    );
    assert!(busy.effects().is_empty());
    assert_eq!(reducer, before_stale_completion);
    let decided = acknowledge(&mut reducer, &decision_entry);
    assert!(matches!(
        decided.effects(),
        [Effect::FetchBody {
            round: fetched_round,
            subject: fetched_subject,
            certificate: Some(certificate),
            ..
        }] if *fetched_round == round
            && *fetched_subject == subject
            && certificate == &commit
    ));
    assert_eq!(reducer.durable_state().decision(), Some(&commit));
    assert!(reducer.awaiting_signature().is_none());
    assert_eq!(reducer.queued_signatures().count(), 0);
    assert_eq!(reducer.progress_witness_violation(), None);
    let application_owner = reducer.current_tag();
    assert_eq!(application_owner, original_tag);
    let available = reducer
        .step(Event::BodyAvailable {
            tag: application_owner,
            round,
            subject,
        })
        .expect("the historical decided body enters the current owner's storage pipeline");
    assert!(matches!(
        available.effects(),
        [Effect::StoreBody {
            tag,
            round: stored_round,
            subject: stored_subject,
        }] if *tag == application_owner
            && *stored_round == round
            && *stored_subject == subject
    ));
    let stored = reducer
        .step(Event::BodyStored {
            tag: application_owner,
            round,
            subject,
        })
        .expect("the historical decided body enters current-owner validation");
    assert!(matches!(
        stored.effects(),
        [Effect::ValidateBody {
            tag,
            round: validated_round,
            subject: validated_subject,
        }] if *tag == application_owner
            && *validated_round == round
            && *validated_subject == subject
    ));
    let validated = reducer
        .step(Event::ValidationCompleted {
            tag: application_owner,
            round,
            subject,
            valid: true,
        })
        .expect("the historical decided body becomes actionable in the current incarnation");
    assert!(matches!(
        validated.effects(),
        [Effect::Apply {
            tag,
            subject: applied_subject,
            certificate,
        }] if *tag == application_owner
            && *applied_subject == subject
            && certificate == &commit
            && certificate.round() == round
    ));
    assert_eq!(reducer.current_tag(), application_owner);
    let completed = reducer
        .step(Event::ApplicationCompleted {
            tag: application_owner,
            subject,
        })
        .expect("the current owner accepts completion of the historical CommitQC");
    assert_eq!(completed.disposition(), StepDisposition::Applied);
    assert!(completed.effects().is_empty());
    assert_eq!(reducer.applied_subject(), Some(subject));
    assert!(reducer.ready_to_finish());
    assert_eq!(reducer.progress_witness_violation(), None);
}
#[test]
fn same_view_tc_upgrade_reissues_hung_signature_under_new_generation() {
    let context = context();
    let mut reducer =
        Reducer::new(context.clone(), Some(id(1)), Generation::new(48)).expect("reducer");
    let initial_tag = reducer.current_tag();
    let first_timeout = tc_without_high(&context, 0, &[1, 2, 3]);
    let first_entry = only_persist(
        reducer
            .step(Event::TimeoutCertificateReceived {
                tag: initial_tag,
                certificate: first_timeout,
            })
            .expect("initial TC starts the view-advance WAL transition"),
    );
    let first_install = acknowledge(&mut reducer, &first_entry);
    assert!(matches!(
        first_install.effects(),
        [Effect::EnterView {
            tag,
            protected_lock: None,
            ..
        }] if tag.view() == 1 && tag.strictly_advances(initial_tag)
    ));
    let signing_tag = reducer.current_tag();
    let timeout_round = Round::new(context.height(), signing_tag.view());
    let timeout_entry = only_persist(
        reducer
            .step(Event::TimeoutElapsed { tag: signing_tag })
            .expect("current-view timeout starts its durable intent"),
    );
    let sign = acknowledge(&mut reducer, &timeout_entry);
    assert!(matches!(
        sign.effects(),
        [Effect::Sign {
            tag,
            message: SignableMessage::TimeoutVote(vote),
        }] if *tag == signing_tag && vote.round() == timeout_round
    ));
    let locked_subject = Subject::repeat(0x95);
    let high = qc(&context, 0, Phase::Prepare, locked_subject, &[1, 2, 3]);
    let upgrade = tc_with_high(&context, 0, high.clone(), &[1, 2, 3]);
    let upgrade_entry = only_persist(
        reducer
            .step(Event::TimeoutCertificateReceived {
                tag: signing_tag,
                certificate: upgrade.clone(),
            })
            .expect("strict same-view TC upgrade supersedes the hung signing task"),
    );
    assert!(reducer.awaiting_signature().is_none());
    assert!(reducer.queued_signatures().any(
        |message| matches!(message, SignableMessage::TimeoutVote(vote) if vote.round() == timeout_round)
    ));
    let upgraded = acknowledge(&mut reducer, &upgrade_entry);
    let upgraded_tag = reducer.current_tag();
    assert_eq!(upgraded_tag.view(), signing_tag.view());
    assert!(upgraded_tag.strictly_advances(signing_tag));
    assert!(matches!(
        upgraded.effects(),
        [
            Effect::EnterView {
                tag,
                certificate,
                protected_lock: Some(locked),
            },
            Effect::FetchBody {
                tag: fetch_tag,
                round,
                subject,
                certificate: Some(fetch_certificate),
                ..
            },
            Effect::Sign {
                tag: sign_tag,
                message: SignableMessage::TimeoutVote(vote),
            },
        ] if *tag == upgraded_tag
            && certificate == &upgrade
            && locked == &high
            && *fetch_tag == upgraded_tag
            && *round == high.round()
            && *subject == locked_subject
            && fetch_certificate == &high
            && *sign_tag == upgraded_tag
            && vote.round() == timeout_round
    ));
    assert!(matches!(
        reducer.awaiting_signature(),
        Some(SignableMessage::TimeoutVote(vote)) if vote.round() == timeout_round
    ));
    let before_stale = reducer.clone();
    let stale = reducer
        .step(Event::Signed {
            tag: signing_tag,
            signature: signature(1),
        })
        .expect("old-generation signature completion is a typed stale stutter");
    assert_eq!(
        stale.disposition(),
        StepDisposition::Ignored(IgnoreReason::StaleGeneration)
    );
    assert!(stale.effects().is_empty());
    assert_eq!(reducer, before_stale);
    let completed = reducer
        .step(Event::Signed {
            tag: upgraded_tag,
            signature: signature(1),
        })
        .expect("reissued signing task completes under the new generation");
    assert!(matches!(
        completed.effects(),
        [Effect::Broadcast(ConsensusMessageV2::TimeoutVote(vote))]
            if vote.vote().round() == timeout_round
    ));
    assert!(reducer.awaiting_signature().is_none());
    assert_eq!(reducer.progress_witness_violation(), None);
}
#[test]
fn future_view_commit_qc_uses_current_owner_through_application() {
    let context = context();
    let owner_round = Round::new(context.height(), 0);
    let future_round = Round::new(context.height(), 3);
    let subject = Subject::repeat(0x94);
    let commit = qc(
        &context,
        future_round.view(),
        Phase::Commit,
        subject,
        &[1, 2, 3],
    );
    let mut reducer = Reducer::new(context, Some(id(1)), Generation::new(48)).expect("reducer");
    let application_owner = reducer.current_tag();
    assert_eq!(
        Round::new(application_owner.height(), application_owner.view()),
        owner_round
    );
    assert!(commit.round().view() > application_owner.view());
    let decision_entry = only_persist(
        reducer
            .step(Event::QuorumCertificateReceived {
                tag: application_owner,
                certificate: commit.clone(),
            })
            .expect("a valid future-view CommitQC starts durable Decision persistence"),
    );
    assert!(matches!(
        decision_entry.record(),
        WalRecord::Decision(certificate) if certificate == &commit
    ));
    let decided = acknowledge(&mut reducer, &decision_entry);
    assert_eq!(reducer.current_tag(), application_owner);
    assert_eq!(reducer.durable_state().current_view(), owner_round.view());
    assert_eq!(reducer.durable_state().decision(), Some(&commit));
    assert!(matches!(
        decided.effects(),
        [Effect::FetchBody {
            tag,
            round,
            subject: fetched_subject,
            certificate: Some(certificate),
            ..
        }] if *tag == application_owner
            && *round == future_round
            && *fetched_subject == subject
            && certificate == &commit
    ));
    let available = reducer
        .step(Event::BodyAvailable {
            tag: application_owner,
            round: future_round,
            subject,
        })
        .expect("the future-round decided body enters the current owner's storage pipeline");
    assert!(matches!(
        available.effects(),
        [Effect::StoreBody {
            tag,
            round,
            subject: stored_subject,
        }] if *tag == application_owner
            && *round == future_round
            && *stored_subject == subject
    ));
    let stored = reducer
        .step(Event::BodyStored {
            tag: application_owner,
            round: future_round,
            subject,
        })
        .expect("the future-round decided body enters current-owner validation");
    assert!(matches!(
        stored.effects(),
        [Effect::ValidateBody {
            tag,
            round,
            subject: validated_subject,
        }] if *tag == application_owner
            && *round == future_round
            && *validated_subject == subject
    ));
    let validated = reducer
        .step(Event::ValidationCompleted {
            tag: application_owner,
            round: future_round,
            subject,
            valid: true,
        })
        .expect("the future-round decided body becomes actionable under the current owner");
    assert!(matches!(
        validated.effects(),
        [Effect::Apply {
            tag,
            subject: applied_subject,
            certificate,
        }] if *tag == application_owner
            && *applied_subject == subject
            && certificate == &commit
            && certificate.round() == future_round
    ));
    let completed = reducer
        .step(Event::ApplicationCompleted {
            tag: application_owner,
            subject,
        })
        .expect("the current owner accepts completion of the future-view CommitQC");
    assert_eq!(completed.disposition(), StepDisposition::Applied);
    assert!(completed.effects().is_empty());
    assert_eq!(reducer.current_tag(), application_owner);
    assert_eq!(reducer.applied_subject(), Some(subject));
    assert!(reducer.ready_to_finish());
    assert_eq!(reducer.progress_witness_violation(), None);
}
#[test]
fn later_reproposal_commit_qc_replays_and_applies_its_exact_certified_round() {
    let context = context();
    let local = id(1);
    let origin_round = Round::new(context.height(), 0);
    let finality_round = Round::new(context.height(), 2);
    let subject = Subject::repeat(0x95);
    let prepare = qc(
        &context,
        origin_round.view(),
        Phase::Prepare,
        subject,
        &[1, 2, 3],
    );
    let commit = qc(
        &context,
        finality_round.view(),
        Phase::Commit,
        subject,
        &[1, 2, 3],
    );
    let lock_entry = WalEntry::new(
        PersistenceId::new(1),
        WalRecord::LockAndCommit {
            prepare,
            vote: Vote::new(context.id(), origin_round, Phase::Commit, subject, local),
        },
    );
    let entries = [
        lock_entry,
        WalEntry::new(PersistenceId::new(2), WalRecord::Decision(commit.clone())),
    ];
    let mut reducer = Reducer::recover(context, Some(local), Generation::new(49), entries)
        .expect("recover the old lock and strict same-round later Decision");
    let owner = reducer.current_tag();
    assert!(owner.view() < finality_round.view());
    let resumed = resume_after_replay(&mut reducer);
    assert!(matches!(
        resumed.effects(),
        [Effect::FetchBody {
            tag,
            round,
            subject: fetched_subject,
            certificate: Some(certificate),
            ..
        }] if *tag == owner
            && *round == finality_round
            && *fetched_subject == subject
            && certificate == &commit
            && certificate.round() == finality_round
    ));
    let available = reducer
        .step(Event::BodyAvailable {
            tag: owner,
            round: finality_round,
            subject,
        })
        .expect("recover the exact later-round certified body bytes");
    assert!(matches!(
        available.effects(),
        [Effect::StoreBody { round, .. }] if *round == finality_round
    ));
    let stored = reducer
        .step(Event::BodyStored {
            tag: owner,
            round: finality_round,
            subject,
        })
        .expect("store the later-round certified body");
    assert!(matches!(
        stored.effects(),
        [Effect::ValidateBody { round, .. }] if *round == finality_round
    ));
    let validated = reducer
        .step(Event::ValidationCompleted {
            tag: owner,
            round: finality_round,
            subject,
            valid: true,
        })
        .expect("validate the later-round certified body");
    assert!(matches!(
        validated.effects(),
        [Effect::Apply {
            tag,
            subject: applied_subject,
            certificate,
        }] if *tag == owner && *applied_subject == subject && certificate == &commit
    ));
    reducer
        .step(Event::ApplicationCompleted {
            tag: owner,
            subject,
        })
        .expect("complete later-view finality from its exact certified body");
    assert_eq!(reducer.applied_subject(), Some(subject));
    assert!(reducer.ready_to_finish());
    assert_eq!(reducer.progress_witness_violation(), None);
}
#[test]
fn valid_commit_qc_supersedes_different_subject_prepare_lock_live_and_replay() {
    let context = context();
    let local = id(1);
    let locked_round = Round::new(context.height(), 0);
    let decision_round = Round::new(context.height(), 1);
    let locked_subject = Subject::repeat(0x96);
    let decision_subject = Subject::repeat(0x98);
    let prepare = qc(
        &context,
        locked_round.view(),
        Phase::Prepare,
        locked_subject,
        &[1, 2, 3],
    );
    let commit_vote = Vote::new(
        context.id(),
        locked_round,
        Phase::Commit,
        locked_subject,
        local,
    );
    let lock_entry = WalEntry::new(
        PersistenceId::new(1),
        WalRecord::LockAndCommit {
            prepare,
            vote: commit_vote,
        },
    );
    let decision = qc(
        &context,
        decision_round.view(),
        Phase::Commit,
        decision_subject,
        &[1, 2, 3],
    );
    assert_eq!(decision.proposal_round(), decision.round());
    decision
        .validate(&context)
        .expect("the different-subject CommitQC is independently valid");
    let mut live = Reducer::recover(
        context.clone(),
        Some(local),
        Generation::new(50),
        [lock_entry.clone()],
    )
    .expect("recover the older Prepare lock");
    assert!(matches!(
        resume_after_replay(&mut live).effects(),
        [Effect::Sign {
            message: SignableMessage::Vote(vote),
            ..
        }] if *vote == commit_vote
    ));
    live.step(Event::Signed {
        tag: live.current_tag(),
        signature: signature(1),
    })
    .expect("restore the old lock's exact Commit owner");
    let old_lock_retry = live
        .step(Event::RetransmitElapsed {
            tag: live.current_tag(),
        })
        .expect("the old lock initially owns certified body recovery");
    assert!(old_lock_retry.effects().iter().any(|effect| matches!(
        effect,
        Effect::FetchBody {
            round,
            subject,
            certificate: Some(certificate),
            ..
        } if *round == locked_round
            && *subject == locked_subject
            && certificate.phase() == Phase::Prepare
    )));
    let decision_entry = only_persist(
        live.step(Event::QuorumCertificateReceived {
            tag: live.current_tag(),
            certificate: decision.clone(),
        })
        .expect("the first valid CommitQC supersedes a different local Prepare lock"),
    );
    assert!(matches!(
        decision_entry.record(),
        WalRecord::Decision(certificate) if certificate == &decision
    ));
    let decided = acknowledge(&mut live, &decision_entry);
    assert!(matches!(
        decided.effects(),
        [Effect::FetchBody {
            round,
            subject,
            certificate: Some(certificate),
            ..
        }] if *round == decision_round
            && *subject == decision_subject
            && certificate == &decision
    ));
    assert_eq!(live.volatile_evidence_counts(), (0, 0, 0, 0));
    let late_locked_body = live
        .step(Event::BodyAvailable {
            tag: live.current_tag(),
            round: locked_round,
            subject: locked_subject,
        })
        .expect("a completion for the superseded lock is an exact terminal stutter");
    assert_eq!(
        late_locked_body.disposition(),
        StepDisposition::Ignored(IgnoreReason::NoMatchingWork)
    );
    assert!(late_locked_body.effects().is_empty());
    let retransmitted = live
        .step(Event::RetransmitElapsed {
            tag: live.current_tag(),
        })
        .expect("only the Decision and its exact body frontier remain retransmittable");
    assert!(retransmitted.effects().iter().all(|effect| matches!(
        effect,
        Effect::Broadcast(ConsensusMessageV2::QuorumCertificate(certificate))
            if certificate == &decision
    ) || matches!(
        effect,
        Effect::FetchBody {
            round,
            subject,
            certificate: Some(certificate),
            ..
        } if *round == decision_round
            && *subject == decision_subject
            && certificate == &decision
    )));
    let available = live
        .step(Event::BodyAvailable {
            tag: live.current_tag(),
            round: decision_round,
            subject: decision_subject,
        })
        .expect("acquire the exact decided body");
    assert!(matches!(available.effects(), [Effect::StoreBody { .. }]));
    let stored = live
        .step(Event::BodyStored {
            tag: live.current_tag(),
            round: decision_round,
            subject: decision_subject,
        })
        .expect("store the exact decided body");
    assert!(matches!(stored.effects(), [Effect::ValidateBody { .. }]));
    let validated = live
        .step(Event::ValidationCompleted {
            tag: live.current_tag(),
            round: decision_round,
            subject: decision_subject,
            valid: true,
        })
        .expect("validate the exact decided body");
    assert!(matches!(
        validated.effects(),
        [Effect::Apply {
            subject,
            certificate,
            ..
        }] if *subject == decision_subject && certificate == &decision
    ));
    live.step(Event::ApplicationCompleted {
        tag: live.current_tag(),
        subject: decision_subject,
    })
    .expect("apply the quorum-authenticated superseding decision");
    assert_eq!(live.applied_subject(), Some(decision_subject));
    assert!(live.ready_to_finish());
    assert_eq!(live.progress_witness_violation(), None);
    let conflicting = qc(
        &context,
        decision_round.view() + 1,
        Phase::Commit,
        Subject::repeat(0x99),
        &[1, 2, 3],
    );
    assert!(matches!(
        live.step(Event::QuorumCertificateReceived {
            tag: live.current_tag(),
            certificate: conflicting,
        }),
        Err(ReducerError::ConflictingDecision)
    ));
    let replay_entries = [
        lock_entry,
        WalEntry::new(PersistenceId::new(2), WalRecord::Decision(decision.clone())),
    ];
    let mut replayed = Reducer::recover(
        context.clone(),
        Some(local),
        Generation::new(51),
        replay_entries,
    )
    .expect("WAL replay accepts the first valid Decision over an older lock");
    assert!(matches!(
        resume_after_replay(&mut replayed).effects(),
        [Effect::FetchBody {
            round,
            subject,
            certificate: Some(certificate),
            ..
        }] if *round == decision_round
            && *subject == decision_subject
            && certificate == &decision
    ));
    let replay_retransmit = replayed
        .step(Event::RetransmitElapsed {
            tag: replayed.current_tag(),
        })
        .expect("replay retains only terminal Decision control");
    assert!(replay_retransmit.effects().iter().all(|effect| matches!(
        effect,
        Effect::Broadcast(ConsensusMessageV2::QuorumCertificate(certificate))
            if certificate == &decision
    ) || matches!(
        effect,
        Effect::FetchBody {
            round,
            subject,
            certificate: Some(certificate),
            ..
        } if *round == decision_round
            && *subject == decision_subject
            && certificate == &decision
    )));
    let split_round = QuorumCertificate::new(
        CertificateRef::new_with_proposal_round(
            context.id(),
            decision_round,
            locked_round,
            Phase::Commit,
            decision_subject,
        ),
        shares(&[1, 2, 3]),
    );
    assert!(matches!(
        split_round.validate(&context),
        Err(QuorumError::InvalidProposalRound)
    ));
    let split_replay = DurableState::replay(
        &context,
        Some(local),
        [WalEntry::new(
            PersistenceId::new(1),
            WalRecord::Decision(split_round.clone()),
        )],
    );
    assert!(matches!(split_replay, Err(ReplayError::InvalidCertificate)));
    let mut split_live =
        Reducer::new(context, Some(local), Generation::new(52)).expect("fresh reducer");
    assert!(matches!(
        split_live.step(Event::QuorumCertificateReceived {
            tag: split_live.current_tag(),
            certificate: split_round,
        }),
        Err(ReducerError::Quorum(QuorumError::InvalidProposalRound))
    ));
}
#[test]
fn earlier_same_body_commit_qc_supersedes_a_later_reproposal_lock() {
    let context = context();
    let local = id(1);
    let subject = Subject::repeat(0x97);
    let original_round = Round::new(context.height(), 0);
    let reproposal_round = Round::new(context.height(), 1);
    let original_prepare = qc(&context, 0, Phase::Prepare, subject, &[1, 2, 3]);
    let original_commit = qc(&context, 0, Phase::Commit, subject, &[1, 2, 3]);
    let timeout = tc_with_high(&context, 0, original_prepare.clone(), &[1, 2, 3]);
    let reproposal_prepare = qc(&context, 1, Phase::Prepare, subject, &[1, 2, 3]);
    let reproposal_vote = Vote::new(
        context.id(),
        reproposal_round,
        Phase::Commit,
        subject,
        local,
    );
    let mut reducer = Reducer::recover(
        context.clone(),
        Some(local),
        Generation::new(50),
        [
            WalEntry::new(
                PersistenceId::new(1),
                WalRecord::LockAndCommit {
                    prepare: original_prepare,
                    vote: Vote::new(context.id(), original_round, Phase::Commit, subject, local),
                },
            ),
            WalEntry::new(PersistenceId::new(2), WalRecord::InstallTimeout(timeout)),
            WalEntry::new(
                PersistenceId::new(3),
                WalRecord::LockAndCommit {
                    prepare: reproposal_prepare.clone(),
                    vote: reproposal_vote,
                },
            ),
        ],
    )
    .expect("recover the later unchanged-body lock");
    assert_eq!(
        reducer
            .durable_state()
            .locked()
            .map(QuorumCertificate::round),
        Some(reproposal_round)
    );
    assert!(matches!(
        resume_after_replay(&mut reducer).effects(),
        [Effect::Sign {
            message: SignableMessage::Vote(vote),
            ..
        }] if *vote == reproposal_vote
    ));
    reducer
        .step(Event::Signed {
            tag: reducer.current_tag(),
            signature: signature(1),
        })
        .expect("restore the later lock's exact local Commit owner");
    let decision_entry = only_persist(
        reducer
            .step(Event::QuorumCertificateReceived {
                tag: reducer.current_tag(),
                certificate: original_commit.clone(),
            })
            .expect("an earlier same-body CommitQC decides beneath the later lock"),
    );
    assert!(matches!(
        decision_entry.record(),
        WalRecord::Decision(certificate) if certificate == &original_commit
    ));
    let decided = acknowledge(&mut reducer, &decision_entry);
    assert!(matches!(
        decided.effects(),
        [Effect::FetchBody {
            round,
            subject: fetched_subject,
            certificate: Some(certificate),
            ..
        }] if *round == original_round
            && *fetched_subject == subject
            && certificate == &original_commit
    ));
    let semantic_duplicate = reducer
        .step(Event::QuorumCertificateReceived {
            tag: reducer.current_tag(),
            certificate: qc(
                &context,
                reproposal_round.view(),
                Phase::Commit,
                subject,
                &[1, 2, 3],
            ),
        })
        .expect("a later same-body CommitQC is the same durable decision");
    assert_eq!(
        semantic_duplicate.disposition(),
        StepDisposition::Ignored(IgnoreReason::Duplicate)
    );
    assert!(semantic_duplicate.effects().is_empty());
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
        resume_after_replay(&mut prepared).effects(),
        [Effect::Sign {
            message: SignableMessage::Vote(vote),
            ..
        }] if vote.phase() == Phase::Prepare
    ));
    prepared
        .step(Event::Signed {
            tag: prepared.current_tag(),
            signature: signature(1),
        })
        .expect("publish the still-open Prepare intent");
    assert!(prepared.outbound_messages().any(|message| {
        matches!(
            message,
            ConsensusMessageV2::Vote(vote) if vote.vote() == prepare_vote
        )
    }));
    let timeout_entry = only_persist(
        prepared
            .step(Event::TimeoutElapsed {
                tag: prepared.current_tag(),
            })
            .expect("begin the durable timeout fence"),
    );
    let WalRecord::TimeoutIntent(expected_timeout) = timeout_entry.record() else {
        panic!("timeout fence must persist its exact local vote");
    };
    let expected_timeout = expected_timeout.clone();
    let timeout_sign = acknowledge(&mut prepared, &timeout_entry);
    assert!(matches!(
        timeout_sign.effects(),
        [Effect::Sign {
            message: SignableMessage::TimeoutVote(_),
            ..
        }]
    ));
    assert!(prepared.outbound_messages().all(|message| {
        !matches!(
            message,
            ConsensusMessageV2::Vote(vote) if vote.vote() == prepare_vote
        )
    }));
    prepared
        .step(Event::Signed {
            tag: prepared.current_tag(),
            signature: signature(2),
        })
        .expect("publish the durable timeout vote");
    assert!(prepared.outbound_messages().any(|message| {
        matches!(
            message,
            ConsensusMessageV2::TimeoutVote(vote) if vote.vote() == expected_timeout
        )
    }));
    let retry = prepared
        .step(Event::RetransmitElapsed {
            tag: prepared.current_tag(),
        })
        .expect("timeout-fenced retransmission passes refinement");
    assert!(retry.effects().iter().all(|effect| {
        !matches!(
            effect,
            Effect::Broadcast(ConsensusMessageV2::Vote(vote)) if vote.vote() == prepare_vote
        )
    }));
    assert!(retry.effects().iter().any(|effect| {
        matches!(
            effect,
            Effect::Broadcast(ConsensusMessageV2::TimeoutVote(vote))
                if vote.vote() == expected_timeout
        )
    }));
    let mut closed = Reducer::recover(
        context,
        Some(id(1)),
        Generation::new(12),
        [prepare_entry, timeout_entry],
    )
    .unwrap();
    let effects = resume_after_replay(&mut closed).into_effects();
    assert!(matches!(
        effects.as_slice(),
        [Effect::Sign {
            message: SignableMessage::TimeoutVote(_),
            ..
        }]
    ));
    assert_eq!(
        closed.queued_signatures().count(),
        0,
        "a durable timeout fence must not leave the old Prepare queued"
    );
}
#[test]
fn replay_resume_is_recovery_authenticated_tagged_and_idempotent() {
    let context = context();
    let local = id(1);
    let round = Round::new(context.height(), 0);
    let vote = Vote::new(
        context.id(),
        round,
        Phase::Prepare,
        Subject::repeat(0x75),
        local,
    );
    let entry = WalEntry::new(PersistenceId::new(1), WalRecord::PrepareIntent(vote));
    let mut fresh = Reducer::new(context.clone(), Some(local), Generation::new(10)).unwrap();
    let fresh_before = fresh.clone();
    let unavailable = fresh
        .step(Event::ResumeAfterReplay {
            tag: fresh.current_tag(),
        })
        .unwrap();
    assert_eq!(
        unavailable.disposition(),
        StepDisposition::Ignored(IgnoreReason::Duplicate)
    );
    assert!(unavailable.effects().is_empty());
    assert_eq!(
        fresh, fresh_before,
        "fresh construction cannot mint replay work"
    );
    let generation = Generation::new(11);
    let mut recovered =
        Reducer::recover(context.clone(), Some(local), generation, [entry.clone()]).unwrap();
    let recovered_before = recovered.clone();
    let premature = recovered
        .step(Event::TimeoutElapsed {
            tag: recovered.current_tag(),
        })
        .unwrap();
    assert_eq!(
        premature.disposition(),
        StepDisposition::Ignored(IgnoreReason::RecoveryPending)
    );
    assert!(premature.effects().is_empty());
    assert_eq!(recovered, recovered_before);
    for (tag, reason) in [
        (
            EventTag::new(context.height() + 1, 0, generation),
            IgnoreReason::WrongHeight,
        ),
        (
            EventTag::new(context.height(), 1, generation),
            IgnoreReason::WrongView,
        ),
        (
            EventTag::new(context.height(), 0, Generation::new(10)),
            IgnoreReason::StaleGeneration,
        ),
    ] {
        let stale = recovered
            .step(Event::ResumeAfterReplay { tag })
            .expect("stale lifecycle input is an accepted stutter");
        assert_eq!(stale.disposition(), StepDisposition::Ignored(reason));
        assert!(stale.effects().is_empty());
        assert_eq!(recovered, recovered_before);
    }
    let resumed = resume_after_replay(&mut recovered);
    assert!(matches!(
        resumed.effects(),
        [Effect::Sign {
            tag,
            message: SignableMessage::Vote(resumed_vote),
        }] if *tag == recovered.current_tag() && *resumed_vote == vote
    ));
    let after_first = recovered.clone();
    let duplicate = resume_after_replay(&mut recovered);
    assert_eq!(
        duplicate.disposition(),
        StepDisposition::Ignored(IgnoreReason::Duplicate)
    );
    assert!(duplicate.effects().is_empty());
    assert_eq!(
        recovered, after_first,
        "duplicate cannot enqueue a second signature"
    );
}
#[test]
fn replay_resume_after_a_second_crash_rejects_old_generation_work() {
    let context = context();
    let local = id(1);
    let vote = Vote::new(
        context.id(),
        Round::new(context.height(), 0),
        Phase::Prepare,
        Subject::repeat(0x75),
        local,
    );
    let entry = WalEntry::new(PersistenceId::new(1), WalRecord::PrepareIntent(vote));
    let mut first = Reducer::recover(
        context.clone(),
        Some(local),
        Generation::new(11),
        [entry.clone()],
    )
    .unwrap();
    let first_resume = resume_after_replay(&mut first);
    assert!(matches!(first_resume.effects(), [Effect::Sign { .. }]));
    // Crash before the signature completes. A new generation may replay the
    // same durable intent, while both the old completion and the old resume
    // event remain stale and cannot consume the new lifecycle transition.
    let old_tag = first.current_tag();
    let mut restarted =
        Reducer::recover(context, Some(local), Generation::new(12), [entry]).unwrap();
    for stale_event in [
        Event::ResumeAfterReplay { tag: old_tag },
        Event::Signed {
            tag: old_tag,
            signature: signature(0xee),
        },
    ] {
        let stale = restarted.step(stale_event).unwrap();
        assert_eq!(
            stale.disposition(),
            StepDisposition::Ignored(IgnoreReason::StaleGeneration)
        );
        assert!(stale.effects().is_empty());
    }
    assert!(matches!(
        resume_after_replay(&mut restarted).effects(),
        [Effect::Sign {
            tag,
            message: SignableMessage::Vote(resumed_vote),
        }] if *tag == restarted.current_tag() && *resumed_vote == vote
    ));
}
#[test]
fn decision_replay_resume_emits_one_exact_certified_fetch() {
    let context = context();
    let local = id(1);
    let subject = Subject::repeat(0x76);
    let decision = qc(&context, 0, Phase::Commit, subject, &[1, 2, 3]);
    let entry = WalEntry::new(PersistenceId::new(1), WalRecord::Decision(decision.clone()));
    let mut recovered =
        Reducer::recover(context.clone(), Some(local), Generation::new(20), [entry]).unwrap();
    let resumed = resume_after_replay(&mut recovered);
    assert_eq!(resumed.disposition(), StepDisposition::Applied);
    assert!(matches!(
        resumed.effects(),
        [Effect::FetchBody {
            tag,
            round,
            subject: fetched_subject,
            manifest: None,
            certified_sources,
            certificate: Some(certificate),
        }] if *tag == recovered.current_tag()
            && *round == decision.round()
            && *fetched_subject == subject
            && certified_sources == &vec![id(1), id(2), id(3)]
            && certificate == &decision
    ));
    assert_eq!(
        recovered.body_state(decision.round(), subject),
        BodyState::Missing
    );
    let after_first = recovered.clone();
    let duplicate = resume_after_replay(&mut recovered);
    assert_eq!(
        duplicate.disposition(),
        StepDisposition::Ignored(IgnoreReason::Duplicate)
    );
    assert!(duplicate.effects().is_empty());
    assert_eq!(recovered, after_first);
}
#[test]
fn current_view_high_prepare_replay_rearms_exact_certified_missing_body() {
    let context = context();
    let subject = Subject::repeat(0x77);
    let prepare = qc(&context, 0, Phase::Prepare, subject, &[1, 2, 4]);
    let entry = WalEntry::new(
        PersistenceId::new(1),
        WalRecord::ObservePrepare(prepare.clone()),
    );
    let mut recovered =
        Reducer::recover(context, Some(id(3)), Generation::new(21), [entry]).unwrap();
    assert_eq!(
        recovered.body_state(prepare.round(), subject),
        BodyState::Missing
    );
    let resumed = resume_after_replay(&mut recovered);
    assert_eq!(resumed.disposition(), StepDisposition::Applied);
    assert!(
        resumed.effects().is_empty(),
        "constructor reconstruction must not bypass the replay lifecycle gate"
    );
    let retransmitted = recovered
        .step(Event::RetransmitElapsed {
            tag: recovered.current_tag(),
        })
        .expect("the recovered certified acquisition is retryable");
    assert_eq!(
        retransmitted
            .effects()
            .iter()
            .filter(|effect| matches!(effect, Effect::FetchBody { .. }))
            .count(),
        1
    );
    assert!(retransmitted.effects().iter().any(|effect| matches!(
        effect,
        Effect::FetchBody {
            tag,
            round,
            subject: fetched_subject,
            manifest: None,
            certified_sources,
            certificate: Some(certificate),
        } if *tag == recovered.current_tag()
            && *round == prepare.round()
            && *fetched_subject == subject
            && certified_sources == &vec![id(1), id(2), id(4)]
            && certificate == &prepare
    )));
}
#[test]
fn old_view_high_prepare_replay_does_not_reactivate_body_work() {
    let context = context();
    let subject = Subject::repeat(0x78);
    let prepare = qc(&context, 0, Phase::Prepare, subject, &[1, 2, 3]);
    let timeout = tc_without_high(&context, 0, &[1, 2, 3]);
    let entries = [
        WalEntry::new(
            PersistenceId::new(1),
            WalRecord::ObservePrepare(prepare.clone()),
        ),
        WalEntry::new(PersistenceId::new(2), WalRecord::InstallTimeout(timeout)),
    ];
    let mut recovered =
        Reducer::recover(context, Some(id(4)), Generation::new(22), entries).unwrap();
    assert_eq!(recovered.current_tag().view(), 1);
    assert!(resume_after_replay(&mut recovered).effects().is_empty());
    let before_old_completion = recovered.clone();
    let old_completion = recovered
        .step(Event::BodyAvailable {
            tag: recovered.current_tag(),
            round: prepare.round(),
            subject,
        })
        .expect("an old-view completion is an accepted stutter");
    assert_eq!(
        old_completion.disposition(),
        StepDisposition::Ignored(IgnoreReason::NoMatchingWork)
    );
    assert!(old_completion.effects().is_empty());
    assert_eq!(recovered, before_old_completion);
    let retransmitted = recovered
        .step(Event::RetransmitElapsed {
            tag: recovered.current_tag(),
        })
        .expect("old control evidence remains retransmittable");
    assert!(
        retransmitted
            .effects()
            .iter()
            .all(|effect| !matches!(effect, Effect::FetchBody { .. })),
        "non-current high Prepare evidence must not recreate body ownership"
    );
}
#[test]
fn timed_out_current_view_high_prepare_replay_does_not_reactivate_body_work() {
    let context = context();
    let subject = Subject::repeat(0x7c);
    let prepare = qc(&context, 0, Phase::Prepare, subject, &[1, 2, 3]);
    let timeout_vote =
        TimeoutVote::new(context.id(), prepare.round(), id(4), Some(prepare.clone()));
    let entries = [
        WalEntry::new(
            PersistenceId::new(1),
            WalRecord::ObservePrepare(prepare.clone()),
        ),
        WalEntry::new(
            PersistenceId::new(2),
            WalRecord::TimeoutIntent(timeout_vote.clone()),
        ),
    ];
    let mut recovered =
        Reducer::recover(context, Some(id(4)), Generation::new(25), entries).unwrap();
    let resumed = resume_after_replay(&mut recovered);
    assert!(matches!(
        resumed.effects(),
        [Effect::Sign {
            message: SignableMessage::TimeoutVote(vote),
            ..
        }] if vote == &timeout_vote
    ));
    let signed = complete_signature(&mut recovered, 0x7c);
    assert!(matches!(
        signed.effects(),
        [Effect::Broadcast(ConsensusMessageV2::TimeoutVote(vote))]
            if vote.vote() == timeout_vote
    ));
    let before_prepare_completion = recovered.clone();
    let prepare_completion = recovered
        .step(Event::BodyAvailable {
            tag: recovered.current_tag(),
            round: prepare.round(),
            subject,
        })
        .expect("a completion behind the durable timeout fence is an accepted stutter");
    assert_eq!(
        prepare_completion.disposition(),
        StepDisposition::Ignored(IgnoreReason::NoMatchingWork)
    );
    assert!(prepare_completion.effects().is_empty());
    assert_eq!(recovered, before_prepare_completion);
    let retransmitted = recovered
        .step(Event::RetransmitElapsed {
            tag: recovered.current_tag(),
        })
        .expect("closed-view control evidence remains retransmittable");
    assert!(retransmitted.effects().iter().any(|effect| matches!(
        effect,
        Effect::Broadcast(ConsensusMessageV2::QuorumCertificate(certificate))
            if certificate == &prepare
    )));
    assert!(
        retransmitted
            .effects()
            .iter()
            .all(|effect| !matches!(effect, Effect::FetchBody { .. })),
        "a timeout-fenced non-lock Prepare cannot authorize body progress"
    );
}
#[test]
fn decision_replay_excludes_a_conflicting_current_prepare_body_owner() {
    let context = context();
    let prepare_subject = Subject::repeat(0x79);
    let decision_subject = Subject::repeat(0x7a);
    let prepare = qc(&context, 0, Phase::Prepare, prepare_subject, &[1, 2, 3]);
    let decision = qc(&context, 0, Phase::Commit, decision_subject, &[1, 3, 4]);
    let entries = [
        WalEntry::new(
            PersistenceId::new(1),
            WalRecord::ObservePrepare(prepare.clone()),
        ),
        WalEntry::new(PersistenceId::new(2), WalRecord::Decision(decision.clone())),
    ];
    let mut recovered =
        Reducer::recover(context, Some(id(2)), Generation::new(23), entries).unwrap();
    let resumed = resume_after_replay(&mut recovered);
    assert!(matches!(
        resumed.effects(),
        [Effect::FetchBody {
            round,
            subject,
            certified_sources,
            certificate: Some(certificate),
            ..
        }] if *round == decision.round()
            && *subject == decision_subject
            && certified_sources == &vec![id(1), id(3), id(4)]
            && certificate == &decision
    ));
    let before_prepare_completion = recovered.clone();
    let prepare_completion = recovered
        .step(Event::BodyAvailable {
            tag: recovered.current_tag(),
            round: prepare.round(),
            subject: prepare_subject,
        })
        .expect("the superseded Prepare completion is an accepted stutter");
    assert_eq!(
        prepare_completion.disposition(),
        StepDisposition::Ignored(IgnoreReason::NoMatchingWork)
    );
    assert!(prepare_completion.effects().is_empty());
    assert_eq!(recovered, before_prepare_completion);
}
#[test]
fn durable_lock_replay_retains_one_exact_certified_body_owner() {
    let context = context();
    let subject = Subject::repeat(0x7b);
    let prepare = qc(&context, 0, Phase::Prepare, subject, &[1, 2, 3]);
    let commit_vote = Vote::new(context.id(), prepare.round(), Phase::Commit, subject, id(4));
    let entries = [
        WalEntry::new(
            PersistenceId::new(1),
            WalRecord::ObservePrepare(prepare.clone()),
        ),
        WalEntry::new(
            PersistenceId::new(2),
            WalRecord::LockAndCommit {
                prepare: prepare.clone(),
                vote: commit_vote,
            },
        ),
    ];
    let mut recovered =
        Reducer::recover(context, Some(id(4)), Generation::new(24), entries).unwrap();
    assert_eq!(recovered.durable_state().locked(), Some(&prepare));
    assert_eq!(
        recovered.body_state(prepare.round(), subject),
        BodyState::Missing
    );
    let resumed = resume_after_replay(&mut recovered);
    assert!(matches!(
        resumed.effects(),
        [Effect::Sign {
            message: SignableMessage::Vote(vote),
            ..
        }] if *vote == commit_vote
    ));
    let signed = complete_signature(&mut recovered, 0x7b);
    assert!(matches!(
        signed.effects(),
        [Effect::Broadcast(ConsensusMessageV2::Vote(vote))]
            if vote.vote() == commit_vote
    ));
    let retransmitted = recovered
        .step(Event::RetransmitElapsed {
            tag: recovered.current_tag(),
        })
        .expect("the exact durable lock body remains retryable");
    assert_eq!(
        retransmitted
            .effects()
            .iter()
            .filter(|effect| matches!(effect, Effect::FetchBody { .. }))
            .count(),
        1
    );
    assert!(retransmitted.effects().iter().any(|effect| matches!(
        effect,
        Effect::FetchBody {
            round,
            subject: fetched_subject,
            certified_sources,
            certificate: Some(certificate),
            ..
        } if *round == prepare.round()
            && *fetched_subject == subject
            && certified_sources == &vec![id(1), id(2), id(3)]
            && certificate == &prepare
    )));
    let available = recovered
        .step(Event::BodyAvailable {
            tag: recovered.current_tag(),
            round: prepare.round(),
            subject,
        })
        .expect("the exact lock body enters the durable pipeline");
    assert!(matches!(available.effects(), [Effect::StoreBody { .. }]));
    let stored = recovered
        .step(Event::BodyStored {
            tag: recovered.current_tag(),
            round: prepare.round(),
            subject,
        })
        .expect("the exact lock body enters validation");
    assert!(matches!(stored.effects(), [Effect::ValidateBody { .. }]));
    let validated = recovered
        .step(Event::ValidationCompleted {
            tag: recovered.current_tag(),
            round: prepare.round(),
            subject,
            valid: true,
        })
        .expect("the existing durable Commit intent wins lock validation precedence");
    assert_eq!(
        validated.disposition(),
        StepDisposition::Ignored(IgnoreReason::Duplicate)
    );
    assert!(validated.effects().is_empty());
    assert_eq!(
        recovered.body_state(prepare.round(), subject),
        BodyState::Validated
    );
    assert_eq!(recovered.durable_state().locked(), Some(&prepare));
}
#[test]
fn certificate_first_decision_validates_and_applies_without_a_proposal() {
    let context = context();
    let subject = Subject::repeat(0x85);
    let decision = qc(&context, 0, Phase::Commit, subject, &[1, 2, 3]);
    let round = decision.round();
    let mut reducer = Reducer::new(context, Some(id(4)), Generation::new(35)).unwrap();
    let decision_entry = only_persist(
        reducer
            .step(Event::QuorumCertificateReceived {
                tag: reducer.current_tag(),
                certificate: decision.clone(),
            })
            .expect("CommitQC starts a certificate-first durable decision"),
    );
    let decided = acknowledge(&mut reducer, &decision_entry);
    assert!(matches!(
        decided.effects(),
        [Effect::FetchBody {
            round: fetched_round,
            subject: fetched_subject,
            manifest: None,
            certificate: Some(certificate),
            ..
        }] if *fetched_round == round
            && *fetched_subject == subject
            && certificate == &decision
    ));
    let available = reducer
        .step(Event::BodyAvailable {
            tag: reducer.current_tag(),
            round,
            subject,
        })
        .expect("the canonical certified body enters local storage");
    assert!(matches!(available.effects(), [Effect::StoreBody { .. }]));
    let stored = reducer
        .step(Event::BodyStored {
            tag: reducer.current_tag(),
            round,
            subject,
        })
        .expect("the durable certified body enters deterministic validation");
    assert!(matches!(stored.effects(), [Effect::ValidateBody { .. }]));
    let validated = reducer
        .step(Event::ValidationCompleted {
            tag: reducer.current_tag(),
            round,
            subject,
            valid: true,
        })
        .expect("a valid certificate-first body advances the durable decision");
    assert!(matches!(
        validated.effects(),
        [Effect::Apply {
            subject: applied_subject,
            certificate,
            ..
        }] if *applied_subject == subject && certificate == &decision
    ));
    assert!(reducer.durable_state().prepare_intent(round).is_none());
    assert!(reducer.durable_state().commit_intent(round).is_none());
}
include!("tests/empty_replay_resume_test.rs");
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
fn prepare_intent_replay_derives_local_vote_guards_transactionally() {
    let context = context();
    let round = Round::new(context.height(), 0);
    let subject = Subject::repeat(0x74);
    let invalid = [
        (
            Vote::new(
                ContextId::repeat(0x99),
                round,
                Phase::Prepare,
                subject,
                id(1),
            ),
            Some(id(1)),
            ReplayError::ContextMismatch,
        ),
        (
            Vote::new(
                context.id(),
                Round::new(context.height() + 1, 0),
                Phase::Prepare,
                subject,
                id(1),
            ),
            Some(id(1)),
            ReplayError::InvalidLocalVote,
        ),
        (
            Vote::new(context.id(), round, Phase::Commit, subject, id(1)),
            Some(id(1)),
            ReplayError::InvalidLocalVote,
        ),
        (
            Vote::new(context.id(), round, Phase::Prepare, subject, id(2)),
            Some(id(1)),
            ReplayError::InvalidLocalVote,
        ),
        (
            Vote::new(context.id(), round, Phase::Prepare, subject, id(9)),
            Some(id(9)),
            ReplayError::InvalidLocalVote,
        ),
        (
            Vote::new(context.id(), round, Phase::Prepare, subject, id(1)),
            None,
            ReplayError::InvalidLocalVote,
        ),
    ];
    for (vote, local_validator, expected) in invalid {
        let mut state = DurableState::new(&context);
        let before = state.clone();
        let entry = WalEntry::new(PersistenceId::new(1), WalRecord::PrepareIntent(vote));
        assert_eq!(
            state.apply(&context, local_validator, &entry),
            Err(expected)
        );
        assert_eq!(state, before);
    }
}
#[test]
fn timeout_intent_replay_derives_local_vote_and_full_high_qc_guards_transactionally() {
    let context = context();
    let round = Round::new(context.height(), 0);
    let invalid = [
        (
            TimeoutVote::new(ContextId::repeat(0x99), round, id(1), None),
            Some(id(1)),
            ReplayError::ContextMismatch,
        ),
        (
            TimeoutVote::new(
                context.id(),
                Round::new(context.height() + 1, 0),
                id(1),
                None,
            ),
            Some(id(1)),
            ReplayError::InvalidLocalVote,
        ),
        (
            TimeoutVote::new(context.id(), Round::new(context.height(), 1), id(1), None),
            Some(id(1)),
            ReplayError::InvalidLocalVote,
        ),
        (
            TimeoutVote::new(context.id(), round, id(2), None),
            Some(id(1)),
            ReplayError::InvalidLocalVote,
        ),
        (
            TimeoutVote::new(context.id(), round, id(9), None),
            Some(id(9)),
            ReplayError::InvalidLocalVote,
        ),
        (
            TimeoutVote::new(context.id(), round, id(1), None),
            None,
            ReplayError::InvalidLocalVote,
        ),
    ];
    for (vote, local_validator, expected) in invalid {
        let mut state = DurableState::new(&context);
        let before = state.clone();
        let entry = WalEntry::new(PersistenceId::new(1), WalRecord::TimeoutIntent(vote));
        assert_eq!(
            state.apply(&context, local_validator, &entry),
            Err(expected)
        );
        assert_eq!(state, before);
    }
    let subject = Subject::repeat(0x76);
    let highest = qc(&context, 0, Phase::Prepare, subject, &[1, 2, 3]);
    let observed = WalEntry::new(
        PersistenceId::new(1),
        WalRecord::ObservePrepare(highest.clone()),
    );
    let mut state = DurableState::new(&context);
    state
        .apply(&context, Some(id(1)), &observed)
        .expect("valid PrepareQC becomes the durable high QC");
    let before_timeout = state.clone();
    let same_reference_different_evidence =
        QuorumCertificate::new(highest.reference(), shares(&[1, 2, 4]));
    for carried in [None, Some(same_reference_different_evidence)] {
        let entry = WalEntry::new(
            PersistenceId::new(2),
            WalRecord::TimeoutIntent(TimeoutVote::new(context.id(), round, id(1), carried)),
        );
        assert_eq!(
            state.apply(&context, Some(id(1)), &entry),
            Err(ReplayError::TimeoutHighQcMismatch)
        );
        assert_eq!(state, before_timeout);
    }
    let exact_vote = TimeoutVote::new(context.id(), round, id(1), Some(highest));
    let exact_entry = WalEntry::new(
        PersistenceId::new(2),
        WalRecord::TimeoutIntent(exact_vote.clone()),
    );
    state
        .apply(&context, Some(id(1)), &exact_entry)
        .expect("the exact full durable high QC authorizes the timeout intent");
    assert_eq!(state.timeout_intent(round), Some(exact_vote));
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
                Some(prepare.clone()),
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
        let effects = resume_after_replay(&mut recovered).into_effects();
        if prefix_len >= 7 {
            assert!(effects.iter().any(|effect| matches!(
                effect,
                Effect::FetchBody {
                    certificate: Some(_),
                    ..
                }
            )));
        } else if prefix_len == 4 {
            // Proposal and Prepare intents from the same WAL prefix may be
            // reconstructed first. Completing that finite FIFO must expose
            // and broadcast the exact durable Commit intent as well.
            let mut pending_effects = effects;
            let mut saw_commit_sign = false;
            let mut saw_commit_broadcast = false;
            for marker in 0_u8..3 {
                let message = pending_effects
                    .iter()
                    .find_map(|effect| match effect {
                        Effect::Sign { message, .. } => Some(message.clone()),
                        _ => None,
                    })
                    .expect("each durable local intent receives a signature turn");
                saw_commit_sign |= matches!(
                    message,
                    SignableMessage::Vote(vote)
                        if vote.phase() == Phase::Commit
                            && vote.round() == round
                            && vote.subject() == subject
                            && vote.signer() == local
                );
                let signed = recovered
                    .step(Event::Signed {
                        tag: recovered.current_tag(),
                        signature: signature(0x84 + marker),
                    })
                    .expect("the recovered intent accepts its exact signature completion");
                saw_commit_broadcast |= signed.effects().iter().any(|effect| {
                    matches!(
                        effect,
                        Effect::Broadcast(ConsensusMessageV2::Vote(vote))
                            if vote.vote().phase() == Phase::Commit
                                && vote.vote().round() == round
                                && vote.vote().subject() == subject
                                && vote.vote().signer() == local
                    )
                });
                pending_effects = signed.into_effects();
            }
            assert!(saw_commit_sign);
            assert!(saw_commit_broadcast);
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
fn replay_resigns_current_proposal_prepare_then_commit_fifo() {
    let context = context();
    let local = context.leader(0);
    let current_round = Round::new(context.height(), 0);
    let subject = Subject::repeat(0x86);
    let prepare = qc(&context, 0, Phase::Prepare, subject, &[1, 2, 3]);
    let commit = Vote::new(context.id(), current_round, Phase::Commit, subject, local);
    let current_proposal = Proposal::new(
        context.id(),
        current_round,
        local,
        PayloadManifest::new(subject, Digest::repeat(0x87), Digest::repeat(0x88), 128, 2),
        ProposalJustification::ParentCommit(context.parent_commit()),
    );
    let current_prepare = Vote::new(context.id(), current_round, Phase::Prepare, subject, local);
    let entries = [
        WalEntry::new(
            PersistenceId::new(1),
            WalRecord::ProposalIntent(current_proposal.clone()),
        ),
        WalEntry::new(
            PersistenceId::new(2),
            WalRecord::PrepareIntent(current_prepare),
        ),
        WalEntry::new(
            PersistenceId::new(3),
            WalRecord::LockAndCommit {
                prepare,
                vote: commit,
            },
        ),
    ];
    let mut recovered = Reducer::recover(context, Some(local), Generation::new(60), entries)
        .expect("one exact proposal origin retains its complete signing FIFO");
    assert_eq!(recovered.current_tag().view(), 0);
    assert_eq!(
        recovered.durable_state().proposal_intent(current_round),
        Some(&current_proposal)
    );
    assert_eq!(
        recovered.durable_state().prepare_intent(current_round),
        Some(current_prepare)
    );
    assert_eq!(
        recovered.durable_state().commit_intent(current_round),
        Some(commit)
    );
    let proposal_message = SignableMessage::Proposal(current_proposal.clone());
    let prepare_message = SignableMessage::Vote(current_prepare);
    let commit_message = SignableMessage::Vote(commit);
    let resumed = resume_after_replay(&mut recovered);
    assert_eq!(
        resumed.effects(),
        [Effect::Sign {
            tag: recovered.current_tag(),
            message: proposal_message.clone(),
        }]
    );
    assert_signature_frontier(
        &recovered,
        Some(&proposal_message),
        &[prepare_message.clone(), commit_message.clone()],
    );
    let proposal_signature = signature(0x91);
    let proposal_completed = complete_signature(&mut recovered, 0x91);
    assert_eq!(
        proposal_completed.effects(),
        [
            Effect::Broadcast(ConsensusMessageV2::Proposal(SignedProposal::new(
                current_proposal.clone(),
                proposal_signature,
            ))),
            Effect::Sign {
                tag: recovered.current_tag(),
                message: prepare_message.clone(),
            },
        ]
    );
    assert_signature_frontier(
        &recovered,
        Some(&prepare_message),
        std::slice::from_ref(&commit_message),
    );
    let prepare_signature = signature(0x92);
    let prepare_completed = complete_signature(&mut recovered, 0x92);
    assert_eq!(
        prepare_completed.effects(),
        [
            Effect::Broadcast(ConsensusMessageV2::Vote(SignedVote::new(
                current_prepare,
                prepare_signature,
            ))),
            Effect::Sign {
                tag: recovered.current_tag(),
                message: commit_message.clone(),
            },
        ]
    );
    assert_signature_frontier(&recovered, Some(&commit_message), &[]);
    let commit_signature = signature(0x93);
    let commit_completed = complete_signature(&mut recovered, 0x93);
    assert_eq!(
        commit_completed.effects(),
        [Effect::Broadcast(ConsensusMessageV2::Vote(
            SignedVote::new(commit, commit_signature,)
        ))]
    );
    assert_signature_frontier(&recovered, None, &[]);
    assert_eq!(
        recovered.durable_state().proposal_intent(current_round),
        Some(&current_proposal),
        "signing must not consume the durable Proposal source"
    );
    assert_eq!(
        recovered.durable_state().prepare_intent(current_round),
        Some(current_prepare),
        "signing must not consume the durable Prepare source"
    );
    assert_eq!(
        recovered.durable_state().commit_intent(current_round),
        Some(commit),
        "signing must not consume the durable Commit source"
    );
    let after_fifo = recovered.clone();
    let duplicate = resume_after_replay(&mut recovered);
    assert_eq!(
        duplicate.disposition(),
        StepDisposition::Ignored(IgnoreReason::Duplicate)
    );
    assert!(duplicate.effects().is_empty());
    assert_eq!(
        recovered, after_fifo,
        "replay cannot enqueue the FIFO twice"
    );
}
#[test]
fn replay_resigns_current_timeout_then_durable_old_round_commit_fifo() {
    let context = context();
    let local = id(1);
    let locked_round = Round::new(context.height(), 0);
    let current_round = Round::new(context.height(), 1);
    let subject = Subject::repeat(0x89);
    let locked_prepare = qc(&context, 0, Phase::Prepare, subject, &[1, 2, 3]);
    let locked_commit = Vote::new(context.id(), locked_round, Phase::Commit, subject, local);
    let current_timeout = TimeoutVote::new(
        context.id(),
        current_round,
        local,
        Some(locked_prepare.clone()),
    );
    let entries = [
        WalEntry::new(
            PersistenceId::new(1),
            WalRecord::LockAndCommit {
                prepare: locked_prepare,
                vote: locked_commit,
            },
        ),
        WalEntry::new(
            PersistenceId::new(2),
            WalRecord::InstallTimeout(tc_without_high(&context, 0, &[1, 2, 3])),
        ),
        WalEntry::new(
            PersistenceId::new(3),
            WalRecord::TimeoutIntent(current_timeout.clone()),
        ),
    ];
    let mut recovered = Reducer::recover(context, Some(local), Generation::new(61), entries)
        .expect("a current Timeout may coexist with an already-durable old-round Commit");
    assert_eq!(
        recovered.durable_state().timeout_intent(current_round),
        Some(current_timeout.clone())
    );
    assert_eq!(
        recovered.durable_state().commit_intent(locked_round),
        Some(locked_commit)
    );
    let timeout_message = SignableMessage::TimeoutVote(current_timeout.clone());
    let commit_message = SignableMessage::Vote(locked_commit);
    let resumed = resume_after_replay(&mut recovered);
    assert_eq!(
        resumed.effects(),
        [Effect::Sign {
            tag: recovered.current_tag(),
            message: timeout_message.clone(),
        }]
    );
    assert_signature_frontier(
        &recovered,
        Some(&timeout_message),
        std::slice::from_ref(&commit_message),
    );
    let timeout_signature = signature(0x94);
    let timeout_completed = complete_signature(&mut recovered, 0x94);
    assert_eq!(
        timeout_completed.effects(),
        [
            Effect::Broadcast(ConsensusMessageV2::TimeoutVote(SignedTimeoutVote::new(
                current_timeout.clone(),
                timeout_signature
            ),)),
            Effect::Sign {
                tag: recovered.current_tag(),
                message: commit_message.clone(),
            },
        ]
    );
    assert_signature_frontier(&recovered, Some(&commit_message), &[]);
    let commit_signature = signature(0x95);
    let commit_completed = complete_signature(&mut recovered, 0x95);
    assert_eq!(
        commit_completed.effects(),
        [Effect::Broadcast(ConsensusMessageV2::Vote(
            SignedVote::new(locked_commit, commit_signature,)
        ))]
    );
    assert_signature_frontier(&recovered, None, &[]);
    assert_eq!(
        recovered.durable_state().timeout_intent(current_round),
        Some(current_timeout),
        "signing must not consume the durable Timeout source"
    );
    assert_eq!(
        recovered.durable_state().commit_intent(locked_round),
        Some(locked_commit),
        "signing must not consume the durable locked Commit source"
    );
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
    assert!(matches!(
        resume_after_replay(&mut reducer).effects(),
        [Effect::Sign {
            message: SignableMessage::Vote(vote),
            ..
        }] if vote.phase() == Phase::Commit && vote.subject() == subject_a
    ));
    reducer
        .step(Event::Signed {
            tag: reducer.current_tag(),
            signature: signature(1),
        })
        .expect("replayed Commit intent signs before new ingress");
    assert_eq!(
        reducer
            .durable_state()
            .locked()
            .map(QuorumCertificate::subject),
        Some(subject_a)
    );
    let unsafe_result = reducer
        .step(Event::ProposalReceived {
            tag: reducer.current_tag(),
            proposal: proposal(
                &context,
                1,
                subject_b,
                ProposalJustification::Timeout(timeout.clone()),
            ),
        })
        .expect("an unsafe proposal is ignored without changing state");
    assert_eq!(
        unsafe_result.disposition(),
        StepDisposition::Ignored(IgnoreReason::UnsafeProposal)
    );
    assert!(unsafe_result.effects().is_empty());
    let equal_subject_reproposal = reducer
        .step(Event::ProposalReceived {
            tag: reducer.current_tag(),
            proposal: proposal(
                &context,
                1,
                subject_a,
                ProposalJustification::Timeout(timeout),
            ),
        })
        .expect("the exact locked subject is safe in the justified later view");
    assert_eq!(
        equal_subject_reproposal.disposition(),
        StepDisposition::Applied
    );
    assert!(matches!(
        equal_subject_reproposal.effects(),
        [Effect::FetchBody { round, subject, .. }]
            if *round == Round::new(context.height(), 1) && *subject == subject_a
    ));
}
#[test]
fn replay_accepts_strictly_higher_matching_prepare_qc_proposal() {
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
    // A later valid TC for view 1 can carry evidence learned after the TC that
    // originally advanced this node. Persist that strict same-round upgrade
    // before the local proposal so its exact latest durable identity and
    // strictly higher PrepareQC both authorize the new subject.
    let higher_prepare = qc(&context, 1, Phase::Prepare, subject_b, &[1, 2, 3]);
    let higher_timeout = tc_with_high(&context, 1, higher_prepare, &[1, 2, 3]);
    let install_higher_timeout = WalEntry::new(
        PersistenceId::new(4),
        WalRecord::InstallTimeout(higher_timeout.clone()),
    );
    let justification = ProposalJustification::Timeout(higher_timeout);
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
        PersistenceId::new(5),
        WalRecord::ProposalIntent(proposal.clone()),
    );
    let recovered = Reducer::recover(
        context,
        Some(local),
        Generation::new(22),
        [
            lock,
            enter_view_one,
            enter_view_two,
            install_higher_timeout,
            proposal_intent,
        ],
    )
    .expect("replay accepts the exact strictly-higher justified subject");
    assert_eq!(
        recovered
            .durable_state()
            .proposal_intent(Round::new(recovered.context().height(), 2)),
        Some(&proposal)
    );
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
    assert!(matches!(
        resume_after_replay(&mut reducer).effects(),
        [Effect::Sign {
            message: SignableMessage::Vote(vote),
            ..
        }] if vote.phase() == Phase::Commit && vote.subject() == subject_a
    ));
    reducer
        .step(Event::Signed {
            tag: reducer.current_tag(),
            signature: signature(1),
        })
        .expect("replayed Commit intent signs before new ingress");
    assert_eq!(
        reducer
            .durable_state()
            .locked()
            .map(QuorumCertificate::subject),
        Some(subject_a)
    );
    let unsafe_result = reducer
        .step(Event::ProposalReceived {
            tag: reducer.current_tag(),
            proposal: proposal(
                &context,
                1,
                subject_b,
                ProposalJustification::Timeout(timeout.clone()),
            ),
        })
        .expect("an unsafe proposal is ignored without changing state");
    assert_eq!(
        unsafe_result.disposition(),
        StepDisposition::Ignored(IgnoreReason::UnsafeProposal)
    );
    assert!(unsafe_result.effects().is_empty());
    let equal_subject_reproposal = reducer
        .step(Event::ProposalReceived {
            tag: reducer.current_tag(),
            proposal: proposal(
                &context,
                1,
                subject_a,
                ProposalJustification::Timeout(timeout),
            ),
        })
        .expect("the TC-selected locked subject is re-proposed unchanged");
    assert_eq!(
        equal_subject_reproposal.disposition(),
        StepDisposition::Applied
    );
    assert!(matches!(
        equal_subject_reproposal.effects(),
        [Effect::FetchBody { round, subject, .. }]
            if *round == Round::new(context.height(), 1) && *subject == subject_a
    ));
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
fn decision_retains_in_flight_body_pipeline_without_duplicate_fetch() {
    let context = context();
    let subject = Subject::repeat(0x84);
    let round = Round::new(context.height(), 0);
    let mut reducer = Reducer::new(context.clone(), Some(id(3)), Generation::new(34)).unwrap();
    let proposed = reducer
        .step(Event::ProposalReceived {
            tag: reducer.current_tag(),
            proposal: proposal(
                &context,
                0,
                subject,
                ProposalJustification::ParentCommit(context.parent_commit()),
            ),
        })
        .expect("proposal starts body acquisition");
    assert!(matches!(proposed.effects(), [Effect::FetchBody { .. }]));
    let available = reducer
        .step(Event::BodyAvailable {
            tag: reducer.current_tag(),
            round,
            subject,
        })
        .expect("exact body becomes available");
    assert!(matches!(available.effects(), [Effect::StoreBody { .. }]));
    assert_eq!(reducer.body_state(round, subject), BodyState::Available);
    let decision = qc(&context, 0, Phase::Commit, subject, &[1, 2, 3]);
    let decision_entry = only_persist(
        reducer
            .step(Event::QuorumCertificateReceived {
                tag: reducer.current_tag(),
                certificate: decision.clone(),
            })
            .expect("CommitQC starts durable decision"),
    );
    let decided = acknowledge(&mut reducer, &decision_entry);
    assert!(
        decided.effects().is_empty(),
        "an in-flight StoreBody is the sole body continuation after Decision"
    );
    assert_eq!(reducer.body_state(round, subject), BodyState::Available);
    assert!(reducer.durable_state().decision().is_some());
    assert!(reducer.durable_state().prepare_intent(round).is_none());
    let store_retry = reducer
        .step(Event::RetransmitElapsed {
            tag: reducer.current_tag(),
        })
        .expect("a lost store owner is reconstructed from the decided body stage");
    assert!(store_retry.effects().iter().any(|effect| matches!(
        effect,
        Effect::StoreBody {
            round: retry_round,
            subject: retry_subject,
            ..
        } if *retry_round == round && *retry_subject == subject
    )));
    let stored = reducer
        .step(Event::BodyStored {
            tag: reducer.current_tag(),
            round,
            subject,
        })
        .expect("retained store completion advances the decided body");
    assert!(matches!(stored.effects(), [Effect::ValidateBody { .. }]));
    let validation_retry = reducer
        .step(Event::RetransmitElapsed {
            tag: reducer.current_tag(),
        })
        .expect("a lost validation owner is reconstructed from the durable body stage");
    assert!(validation_retry.effects().iter().any(|effect| matches!(
        effect,
        Effect::ValidateBody {
            round: retry_round,
            subject: retry_subject,
            ..
        } if *retry_round == round && *retry_subject == subject
    )));
    let validated = reducer
        .step(Event::ValidationCompleted {
            tag: reducer.current_tag(),
            round,
            subject,
            valid: true,
        })
        .expect("retained validation completion advances PendingApply");
    assert!(matches!(
        validated.effects(),
        [Effect::Apply {
            subject: value,
            certificate,
            ..
        }] if *value == subject && certificate == &decision
    ));
    let apply_retry = reducer
        .step(Event::RetransmitElapsed {
            tag: reducer.current_tag(),
        })
        .expect("a lost application owner is reconstructed from the validated body stage");
    assert!(apply_retry.effects().iter().any(|effect| matches!(
        effect,
        Effect::Apply {
            subject: retry_subject,
            certificate,
            ..
        } if *retry_subject == subject && certificate == &decision
    )));
    assert!(reducer.durable_state().prepare_intent(round).is_none());
    assert!(reducer.durable_state().commit_intent(round).is_none());
}
#[test]
fn exact_local_body_completion_after_decision_reconstructs_apply() {
    let context = context();
    let subject = Subject::repeat(0x8b);
    let round = Round::new(context.height(), 0);
    let manifest =
        PayloadManifest::new(subject, Digest::repeat(0x91), Digest::repeat(0x92), 384, 6);
    let decision = qc(&context, 0, Phase::Commit, subject, &[1, 2, 3]);
    let mut reducer = Reducer::new(
        context.clone(),
        Some(context.leader(0)),
        Generation::new(41),
    )
    .expect("reducer");
    let decided = install_decision(&mut reducer, decision.clone());
    assert!(matches!(decided.effects(), [Effect::FetchBody { .. }]));
    assert_eq!(reducer.body_state(round, subject), BodyState::Missing);
    let completed = reducer
        .step(Event::LocalProposalReady {
            tag: reducer.current_tag(),
            manifest,
        })
        .expect("the exact trusted local body completion passes refinement");
    assert_eq!(completed.disposition(), StepDisposition::Applied);
    assert!(matches!(
        completed.effects(),
        [Effect::Apply {
            tag,
            subject: effect_subject,
            certificate,
        }] if *tag == reducer.current_tag()
            && *effect_subject == subject
            && certificate == &decision
    ));
    assert_eq!(reducer.body_state(round, subject), BodyState::Validated);
    assert_eq!(reducer.applied_subject(), None);
    assert_eq!(reducer.progress_witness_violation(), None);
}
#[test]
fn nonmatching_local_body_completions_remain_terminal_after_decision() {
    let context = context();
    let subject = Subject::repeat(0x8c);
    let manifest =
        PayloadManifest::new(subject, Digest::repeat(0x93), Digest::repeat(0x94), 256, 4);
    let decision = qc(&context, 0, Phase::Commit, subject, &[1, 2, 3]);
    let mut decided = Reducer::new(
        context.clone(),
        Some(context.leader(0)),
        Generation::new(42),
    )
    .expect("reducer");
    install_decision(&mut decided, decision);
    for (tag, expected) in [
        (
            EventTag::new(
                context.height(),
                decided.current_tag().view(),
                Generation::new(41),
            ),
            IgnoreReason::StaleGeneration,
        ),
        (
            EventTag::new(
                context.height() + 1,
                decided.current_tag().view(),
                decided.current_tag().generation(),
            ),
            IgnoreReason::WrongHeight,
        ),
    ] {
        let mut reducer = decided.clone();
        let before = reducer.clone();
        let ignored = reducer
            .step(Event::LocalProposalReady { tag, manifest })
            .expect("a foreign completion is a refinement-safe stutter");
        assert_eq!(ignored.disposition(), StepDisposition::Ignored(expected));
        assert!(ignored.effects().is_empty());
        assert_eq!(reducer, before);
    }
    let mut wrong_subject = decided.clone();
    let before = wrong_subject.clone();
    let ignored = wrong_subject
        .step(Event::LocalProposalReady {
            tag: wrong_subject.current_tag(),
            manifest: PayloadManifest::new(
                Subject::repeat(0x8d),
                Digest::repeat(0x93),
                Digest::repeat(0x94),
                256,
                4,
            ),
        })
        .expect("a foreign subject is terminal after decision");
    assert_eq!(
        ignored.disposition(),
        StepDisposition::Ignored(IgnoreReason::AlreadyDecided)
    );
    assert!(ignored.effects().is_empty());
    assert_eq!(wrong_subject, before);
}
#[test]
fn conflicting_local_manifest_cannot_replace_decided_body_identity() {
    let context = context();
    let subject = Subject::repeat(0x8e);
    let round = Round::new(context.height(), 0);
    let mut reducer =
        Reducer::new(context.clone(), Some(id(3)), Generation::new(43)).expect("reducer");
    let admitted = reducer
        .step(Event::ProposalReceived {
            tag: reducer.current_tag(),
            proposal: proposal(
                &context,
                0,
                subject,
                ProposalJustification::ParentCommit(context.parent_commit()),
            ),
        })
        .expect("authenticated proposal fixes the exact manifest");
    assert!(matches!(admitted.effects(), [Effect::FetchBody { .. }]));
    install_decision(
        &mut reducer,
        qc(&context, 0, Phase::Commit, subject, &[1, 2, 3]),
    );
    let before = reducer.clone();
    let ignored = reducer
        .step(Event::LocalProposalReady {
            tag: reducer.current_tag(),
            manifest: PayloadManifest::new(
                subject,
                Digest::repeat(0xa1),
                Digest::repeat(0xa2),
                512,
                8,
            ),
        })
        .expect("a conflicting payload commitment is terminal");
    assert_eq!(
        ignored.disposition(),
        StepDisposition::Ignored(IgnoreReason::AlreadyDecided)
    );
    assert!(ignored.effects().is_empty());
    assert_eq!(reducer, before);
    assert_eq!(reducer.body_state(round, subject), BodyState::Missing);
}
#[test]
fn tc_interleaving_cannot_retag_an_old_local_completion_as_decided_body() {
    let context = context();
    let old_subject = Subject::repeat(0x8f);
    let decided_subject = Subject::repeat(0x90);
    let mut reducer =
        Reducer::new(context.clone(), Some(id(1)), Generation::new(44)).expect("reducer");
    let old_tag = reducer.current_tag();
    let tc_entry = only_persist(
        reducer
            .step(Event::TimeoutCertificateReceived {
                tag: old_tag,
                certificate: tc_without_high(&context, 0, &[1, 2, 3]),
            })
            .expect("TC begins installation"),
    );
    acknowledge(&mut reducer, &tc_entry);
    assert_eq!(reducer.current_tag().view(), 1);
    assert_ne!(reducer.current_tag().generation(), old_tag.generation());
    install_decision(
        &mut reducer,
        qc(&context, 1, Phase::Commit, decided_subject, &[1, 2, 3]),
    );
    let before = reducer.clone();
    let stale = reducer
        .step(Event::LocalProposalReady {
            tag: old_tag,
            manifest: PayloadManifest::new(
                old_subject,
                Digest::repeat(0xa3),
                Digest::repeat(0xa4),
                128,
                2,
            ),
        })
        .expect("old-generation local completion is rejected");
    assert_eq!(
        stale.disposition(),
        StepDisposition::Ignored(IgnoreReason::StaleGeneration)
    );
    assert!(stale.effects().is_empty());
    assert_eq!(reducer, before);
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
    let resumed = resume_after_replay(&mut reducer);
    assert!(resumed.disposition() == StepDisposition::Applied && resumed.effects().is_empty());
    let higher = qc(&context, 1, Phase::Prepare, high_subject, &[1, 2, 3]);
    let event = Event::QuorumCertificateReceived {
        tag: reducer.current_tag(),
        certificate: higher.clone(),
    };
    let persist_high = only_persist(reducer.step(event).expect("observe high PrepareQC"));
    acknowledge(&mut reducer, &persist_high);
    assert_eq!(reducer.volatile_prepare_counts(), (0, 1));
    let older = qc(&context, 0, Phase::Prepare, old_subject, &[1, 2, 3]);
    let before_older = reducer.clone();
    let event = Event::QuorumCertificateReceived {
        tag: reducer.current_tag(),
        certificate: older,
    };
    let ignored = reducer.step(event).expect("ignore old PrepareQC");
    assert!(
        ignored.disposition() == StepDisposition::Ignored(IgnoreReason::IrrelevantView)
            && ignored.effects().is_empty()
            && reducer == before_older
    );
    let event = Event::RetransmitElapsed {
        tag: reducer.current_tag(),
    };
    let retry = reducer.step(event).expect("retransmit cached controls");
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
fn timeout_elapsed_cannot_start_durable_timeout_after_decision() {
    let context = context();
    let subject = Subject::repeat(0x91);
    let mut reducer =
        Reducer::new(context.clone(), Some(id(4)), Generation::new(45)).expect("reducer");
    install_decision(
        &mut reducer,
        qc(&context, 0, Phase::Commit, subject, &[1, 2, 3]),
    );
    let before = reducer.clone();
    let ignored = reducer
        .step(Event::TimeoutElapsed {
            tag: reducer.current_tag(),
        })
        .expect("a local timeout is terminal after decision");
    assert_eq!(
        ignored.disposition(),
        StepDisposition::Ignored(IgnoreReason::AlreadyDecided)
    );
    assert!(ignored.effects().is_empty());
    assert_eq!(reducer, before);
}
#[test]
fn quorum_completing_timeout_vote_cannot_form_tc_after_decision() {
    let context = context();
    let round = Round::new(context.height(), 0);
    let subject = Subject::repeat(0x92);
    let mut reducer =
        Reducer::new(context.clone(), Some(id(4)), Generation::new(46)).expect("reducer");
    for signer in [1_u8, 2] {
        let admitted = reducer
            .step(Event::TimeoutVoteReceived {
                tag: reducer.current_tag(),
                vote: SignedTimeoutVote::new(
                    TimeoutVote::new(context.id(), round, id(signer), None),
                    signature(signer),
                ),
            })
            .expect("a partial timeout quorum is admitted before decision");
        assert!(admitted.effects().is_empty());
    }
    assert!(matches!(
        reducer.timeout_pool_snapshots().as_slice(),
        [TimeoutPoolSnapshot {
            round: pooled_round,
            signers,
            signed_power,
            certificate_formed: false,
        }] if *pooled_round == round
            && signers == &[id(1), id(2)]
            && *signed_power == VotingPower::new(2)
    ));
    install_decision(
        &mut reducer,
        qc(&context, 0, Phase::Commit, subject, &[1, 2, 3]),
    );
    let before = reducer.clone();
    let ignored = reducer
        .step(Event::TimeoutVoteReceived {
            tag: reducer.current_tag(),
            vote: SignedTimeoutVote::new(
                TimeoutVote::new(context.id(), round, id(3), None),
                signature(3),
            ),
        })
        .expect("a quorum-completing timeout vote is terminal after decision");
    assert_eq!(
        ignored.disposition(),
        StepDisposition::Ignored(IgnoreReason::AlreadyDecided)
    );
    assert!(ignored.effects().is_empty());
    assert_eq!(reducer, before);
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
    let retirement = super::wal::WalRetirementAuthorization::from_finalized_height(&finalized);
    assert!(retirement.matches_finalized_height(&finalized));
    assert_retirement_is_bound_to_finality(&retirement, &context, subject, &decision);
}
fn assert_retirement_is_bound_to_finality(
    retirement: &WalRetirementAuthorization,
    context: &HeightContext,
    subject: Subject,
    decision: &QuorumCertificate,
) {
    assert_eq!(retirement.context_id(), context.id());
    assert_eq!(retirement.height(), context.height());
    assert_eq!(retirement.subject(), subject);
    assert_eq!(retirement.certificate(), decision.reference());
    assert!(!retirement.matches_durable_decision(
        ContextId::repeat(0x99),
        context.height(),
        subject,
        decision.reference(),
    ));
    assert!(!retirement.matches_durable_decision(
        context.id(),
        context.height() + 1,
        subject,
        decision.reference(),
    ));
    assert!(!retirement.matches_durable_decision(
        context.id(),
        context.height(),
        Subject::repeat(0x99),
        decision.reference(),
    ));
    assert!(!retirement.matches_durable_decision(
        context.id(),
        context.height(),
        subject,
        CertificateRef::new(context.id(), decision.round(), Phase::Prepare, subject),
    ));
}
#[test]
fn future_prepare_qc_is_transactionally_ignored_without_retransmit_ownership() {
    let context = context();
    let future = qc(
        &context,
        1,
        Phase::Prepare,
        Subject::repeat(0x74),
        &[1, 2, 3],
    );
    let mut reducer =
        Reducer::new(context, Some(id(4)), Generation::new(42)).expect("fresh reducer");
    let before = reducer.clone();
    let ignored = reducer
        .step(Event::QuorumCertificateReceived {
            tag: reducer.current_tag(),
            certificate: future.clone(),
        })
        .expect("a valid future PrepareQC is a harmless stutter");
    assert_eq!(
        ignored.disposition(),
        StepDisposition::Ignored(IgnoreReason::IrrelevantView)
    );
    assert!(ignored.effects().is_empty());
    assert_eq!(&reducer, &before);
    assert!(reducer.durable_state().highest_prepare().is_none());
    assert!(reducer.outbound_messages().all(|message| {
        !matches!(
            message,
            ConsensusMessageV2::QuorumCertificate(certificate)
                if certificate == &future
        )
    }));
    let retransmit = reducer
        .step(Event::RetransmitElapsed {
            tag: reducer.current_tag(),
        })
        .expect("the ignored certificate created no retransmission owner");
    assert!(retransmit.effects().iter().all(|effect| {
        !matches!(
            effect,
            Effect::Broadcast(ConsensusMessageV2::QuorumCertificate(certificate))
                if certificate == &future
        )
    }));
}
include!("tests/v2_core_terminal_transactionality.rs");
