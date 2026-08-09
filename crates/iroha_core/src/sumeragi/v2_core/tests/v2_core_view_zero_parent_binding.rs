#[test]
fn view_zero_binds_semantic_parent_decision_across_reproposal_rounds() {
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

    let redecided_round = Round::new(frozen.round().height(), frozen.round().view() + 3);
    let equivalent_other_view = CertificateRef::new_with_proposal_round(
        frozen.context_id(),
        redecided_round,
        redecided_round,
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
        .expect("an equivalent parent CommitQC after unchanged re-proposal is accepted");
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
