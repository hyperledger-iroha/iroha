#[test]
fn delayed_proposal_is_ignored_and_never_regresses_body_progress() {
    let context = context();
    let subject = Subject::repeat(0x7e);
    let prepare = qc(&context, 0, Phase::Prepare, subject, &[1, 2, 3]);
    let mut reducer = Reducer::new(context.clone(), Some(id(4)), Generation::new(31)).unwrap();
    let observed = reducer
        .step(Event::QuorumCertificateReceived {
            tag: reducer.current_tag(),
            certificate: prepare.clone(),
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
    assert_certified_fallback(&mut reducer, &prepare);
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
