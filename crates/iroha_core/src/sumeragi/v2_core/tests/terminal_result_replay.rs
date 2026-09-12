// A generation change can retire a completion without closing its numeric view.
#[test]
fn same_view_generation_change_can_replay_validation_into_current_commit_vote() {
    let context = context();
    let local = context.leader(1);
    let subject = Subject::repeat(0xA7);
    let round = Round::new(context.height(), 1);
    let first = tc_without_high(&context, 0, &[1, 2, 3]);
    let mut reducer = Reducer::new(context.clone(), Some(local), Generation::new(12))
        .expect("four-validator reducer");
    let install = only_persist(
        reducer
            .step(Event::TimeoutCertificateReceived {
                tag: reducer.current_tag(),
                certificate: first.clone(),
            })
            .expect("enter view one"),
    );
    acknowledge(&mut reducer, &install);
    let old_tag = reducer.current_tag();
    let received = reducer
        .step(Event::ProposalReceived {
            tag: old_tag,
            proposal: proposal(&context, 1, subject, ProposalJustification::Timeout(first)),
        })
        .expect("authenticated proposal starts the original body pipeline");
    assert!(matches!(received.effects(), [Effect::FetchBody { .. }]));
    reducer
        .step(Event::BodyAvailable {
            tag: old_tag,
            round,
            subject,
        })
        .expect("original body is available");
    let stored = reducer
        .step(Event::BodyStored {
            tag: old_tag,
            round,
            subject,
        })
        .expect("original body reaches durable validation");
    assert!(matches!(stored.effects(), [Effect::ValidateBody { .. }]));

    let high = qc(&context, 0, Phase::Prepare, subject, &[1, 2, 3]);
    let upgrade = tc_with_high(&context, 0, high, &[1, 2, 3]);
    let install = only_persist(
        reducer
            .step(Event::TimeoutCertificateReceived {
                tag: old_tag,
                certificate: upgrade,
            })
            .expect("strict same-view TC upgrade"),
    );
    acknowledge(&mut reducer, &install);
    let current = reducer.current_tag();
    assert_eq!(current.view(), old_tag.view());
    assert_ne!(current.generation(), old_tag.generation());
    assert!(reducer.durable_state().timeout_intent(round).is_none());
    let retired = reducer
        .step(Event::ValidationCompleted {
            tag: old_tag,
            round,
            subject,
            valid: true,
        })
        .expect("obsolete physical completion is fenced");
    assert_eq!(
        retired.disposition(),
        StepDisposition::Ignored(IgnoreReason::StaleGeneration)
    );
    assert!(retired.effects().is_empty());

    let prepare = qc(&context, 1, Phase::Prepare, subject, &[1, 2, 3]);
    let received = reducer
        .step(Event::QuorumCertificateReceived {
            tag: current,
            certificate: prepare.clone(),
        })
        .expect("current Prepare restores the exact body work");
    assert!(received.effects().iter().any(|effect| matches!(effect,
        Effect::FetchBody { round: body_round, subject: body_subject, .. }
            if *body_round == round && *body_subject == subject)));
    let observation = received
        .effects()
        .iter()
        .find_map(|effect| match effect {
            Effect::Persist { entry, .. } => Some(entry.clone()),
            _ => None,
        })
        .expect("the exact current Prepare becomes durable before replay");
    acknowledge(&mut reducer, &observation);
    reducer
        .step(Event::BodyAvailable {
            tag: current,
            round,
            subject,
        })
        .expect("retained body is available under current authority");
    reducer
        .step(Event::BodyStored {
            tag: current,
            round,
            subject,
        })
        .expect("retained durable body reaches current validation owner");
    let vote = only_persist(
        reducer
            .step(Event::ValidationCompleted {
                tag: current,
                round,
                subject,
                valid: true,
            })
            .expect("the actual retained success may create a current vote"),
    );
    assert!(
        matches!(vote.record(), WalRecord::LockAndCommit { prepare: actual, vote }
        if actual == &prepare && vote.round() == round && vote.phase() == Phase::Commit)
    );
    let signed = acknowledge(&mut reducer, &vote);
    assert!(matches!(signed.effects(), [Effect::Sign {
        tag, message: SignableMessage::Vote(vote),
    }] if *tag == current && vote.round() == round && vote.phase() == Phase::Commit));
}

// A TC's authenticated lock survives the volatile pending-Prepare census.
#[test]
fn timeout_locked_prepare_rejection_reports_exact_certificate_once() {
    let context = context();
    let subject = Subject::repeat(0xA8);
    let locked_round = Round::new(context.height(), 0);
    let proposal_round = Round::new(context.height(), 1);
    let prepare = qc(&context, 0, Phase::Prepare, subject, &[1, 2, 3]);
    let timeout = tc_with_high(&context, 0, prepare.clone(), &[1, 2, 3]);
    let mut reducer = Reducer::new(
        context.clone(),
        Some(context.leader(1)),
        Generation::new(0),
    )
    .expect("four-validator reducer");
    let install = only_persist(
        reducer
            .step(Event::TimeoutCertificateReceived {
                tag: reducer.current_tag(),
                certificate: timeout.clone(),
            })
            .expect("the timeout carries the exact Prepare authority"),
    );
    acknowledge(&mut reducer, &install);
    let tag = reducer.current_tag();
    assert_eq!(reducer.durable_state().locked(), Some(&prepare));

    // The current ordinary re-proposal has no Prepare QC of its own. The old
    // lock cannot become a report for that different proposal occurrence.
    let received = reducer
        .step(Event::ProposalReceived {
            tag,
            proposal: proposal(
                &context,
                1,
                subject,
                ProposalJustification::Timeout(timeout),
            ),
        })
        .expect("the same immutable subject may be proposed in the new view");
    assert!(received.effects().iter().any(|effect| matches!(effect,
        Effect::FetchBody { round, subject: actual, .. }
            if *round == proposal_round && *actual == subject)));
    for round in [proposal_round, locked_round] {
        reducer
            .step(Event::BodyAvailable { tag, round, subject })
            .expect("the exact body is available");
        let stored = reducer
            .step(Event::BodyStored { tag, round, subject })
            .expect("the exact body becomes durable");
        assert!(matches!(stored.effects(), [Effect::ValidateBody { .. }]));
        let rejected = reducer
            .step(Event::ValidationCompleted {
                tag,
                round,
                subject,
                valid: false,
            })
            .expect("deterministic body rejection");
        if round == locked_round {
            assert_eq!(rejected.effects(), &[Effect::ReportInvalidCertifiedBody {
                subject,
                certificate: prepare.clone(),
            }]);
        } else {
            assert!(rejected.effects().is_empty(), "a different proposal has no matching Prepare authority");
        }
        let duplicate = reducer
            .step(Event::ValidationCompleted { tag, round, subject, valid: false })
            .expect("the terminal rejection is retained");
        assert_eq!(duplicate.disposition(), StepDisposition::Ignored(IgnoreReason::Duplicate));
        assert!(duplicate.effects().is_empty());
    }
}
