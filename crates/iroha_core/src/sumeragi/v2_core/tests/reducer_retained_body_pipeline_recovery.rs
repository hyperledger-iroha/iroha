fn retained_body_manifest(subject: Subject) -> PayloadManifest {
    PayloadManifest::new(subject, Digest::repeat(0xe1), Digest::repeat(0xe2), 512, 4)
}

#[test]
fn retained_body_custody_recovery_restores_work_without_voting_authority() {
    let fixture = reducer();
    let context = fixture.context.clone();
    let timeout = timeout_certificate(&context, 0, None);
    let mut recovered = Reducer::recover(
        context.clone(),
        fixture.local_validator,
        Generation::INITIAL,
        [WalEntry::new(
            PersistenceId::new(1),
            WalRecord::InstallTimeout(timeout),
        )],
    )
    .expect("recover a later view without proposal body work");
    let tag = recovered.current_tag();
    assert!(
        recovered
            .step(Event::ResumeAfterReplay { tag })
            .expect("resume the recovered view")
            .effects()
            .is_empty()
    );
    for view in [0, 1] {
        let round = Round::new(context.height(), view);
        let subject = Subject::repeat(0xe3 + view as u8);
        let manifest = retained_body_manifest(subject);
        let before = recovered.clone();
        recovered
            .restore_retained_body_pipeline_custody(tag, round, manifest, false)
            .expect("restore authenticated historical or current body custody");
        let mut expected = before;
        expected.body_work.insert(
            (round, subject),
            BodyWork {
                manifest: Some(manifest),
                state: BodyState::Missing,
            },
        );
        assert_eq!(
            recovered, expected,
            "custody changes no consensus authority"
        );
        assert_eq!(
            recovered
                .step(Event::BodyAvailable {
                    tag,
                    round,
                    subject
                })
                .expect("replay the retained physical body")
                .effects(),
            &[Effect::StoreBody {
                tag,
                round,
                subject
            }]
        );
        assert_eq!(
            recovered
                .step(Event::BodyStored {
                    tag,
                    round,
                    subject
                })
                .expect("replay its exact durable store acknowledgement")
                .effects(),
            &[Effect::ValidateBody {
                tag,
                round,
                subject
            }]
        );
        let validated = recovered
            .step(Event::ValidationCompleted {
                tag,
                round,
                subject,
                valid: true,
            })
            .expect("validation does not confer voting authority");
        assert!(validated.effects().is_empty());
        assert_eq!(recovered.body_state(round, subject), BodyState::Validated);
        assert!(recovered.candidate.is_none());
        assert!(recovered.pending_prepare.is_empty());
        assert!(recovered.pending_persistence.is_none());
        assert!(recovered.awaiting_signature.is_none());
    }
}

#[test]
fn retained_local_body_custody_coalesces_without_downgrading_or_revalidating() {
    let mut recovered = reducer();
    let tag = recovered.current_tag();
    let round = Round::new(tag.height(), tag.view());
    let subject = Subject::repeat(0xe5);
    let manifest = retained_body_manifest(subject);
    recovered.body_work.insert(
        (round, subject),
        BodyWork {
            manifest: None,
            state: BodyState::Missing,
        },
    );
    recovered
        .restore_retained_body_pipeline_custody(tag, round, manifest, true)
        .expect("standalone local body restores Available without claiming validation");
    assert_eq!(recovered.body_state(round, subject), BodyState::Available);
    for state in [
        BodyState::Available,
        BodyState::Durable,
        BodyState::Validated,
    ] {
        recovered
            .body_work
            .get_mut(&(round, subject))
            .unwrap()
            .state = state;
        let before = recovered.clone();
        for locally_available in [false, true] {
            recovered
                .restore_retained_body_pipeline_custody(tag, round, manifest, locally_available)
                .expect("the same authenticated origin coalesces with exact existing work");
            assert_eq!(recovered, before, "recovery never downgrades existing work");
        }
    }
    recovered
        .body_work
        .get_mut(&(round, subject))
        .unwrap()
        .state = BodyState::Invalid;
    let before = recovered.clone();
    assert_eq!(
        recovered.restore_retained_body_pipeline_custody(tag, round, manifest, true),
        Err(ReducerError::InvalidRetainedBodyPipeline)
    );
    assert_eq!(recovered, before, "recovery cannot revive an invalid body");
}

#[test]
fn retained_body_custody_recovery_rejects_foreign_identity_and_safety_debt_atomically() {
    let mut recovered = reducer();
    let tag = recovered.current_tag();
    let round = Round::new(tag.height(), tag.view());
    let subject = Subject::repeat(0xe6);
    let manifest = retained_body_manifest(subject);
    let mut pending_replay = Reducer::recover(
        recovered.context.clone(),
        recovered.local_validator,
        recovered.generation,
        std::iter::empty::<WalEntry>(),
    )
    .expect("recover without consuming the startup gate");
    let before = pending_replay.clone();
    assert_eq!(
        pending_replay.restore_retained_body_pipeline_custody(tag, round, manifest, false),
        Err(ReducerError::HeightStillBusy)
    );
    assert_eq!(pending_replay, before);
    for (invalid_tag, invalid_round) in [
        (
            EventTag::new(tag.height(), tag.view(), Generation::new(8)),
            round,
        ),
        (
            EventTag::new(tag.height() + 1, tag.view(), tag.generation()),
            round,
        ),
        (
            EventTag::new(tag.height(), tag.view() + 1, tag.generation()),
            round,
        ),
        (tag, Round::new(round.height() + 1, round.view())),
        (tag, Round::new(round.height(), round.view() + 1)),
    ] {
        let before = recovered.clone();
        assert_eq!(
            recovered.restore_retained_body_pipeline_custody(
                invalid_tag,
                invalid_round,
                manifest,
                false,
            ),
            Err(ReducerError::InvalidRetainedBodyPipeline)
        );
        assert_eq!(recovered, before);
    }
    recovered
        .restore_retained_body_pipeline_custody(tag, round, manifest, false)
        .unwrap();
    let before = recovered.clone();
    let conflicting = PayloadManifest::new(
        subject,
        Digest::repeat(0xe7),
        manifest.chunk_root(),
        manifest.byte_len(),
        manifest.chunk_count(),
    );
    assert_eq!(
        recovered.restore_retained_body_pipeline_custody(tag, round, conflicting, false),
        Err(ReducerError::InvalidRetainedBodyPipeline)
    );
    assert_eq!(recovered, before);
    let (pending, _) = pending_timeout_install(None);
    let signable = SignableMessage::Vote(Vote::new(
        recovered.context.id(),
        round,
        Phase::Prepare,
        subject,
        recovered.local_validator.unwrap(),
    ));
    for debt in 0..3 {
        let mut busy = recovered.clone();
        match debt {
            0 => busy.pending_persistence = pending.pending_persistence.clone(),
            1 => busy.awaiting_signature = Some(signable.clone()),
            _ => busy.signature_queue.push_back(signable.clone()),
        }
        let before = busy.clone();
        assert_eq!(
            busy.restore_retained_body_pipeline_custody(tag, round, manifest, true),
            Err(ReducerError::HeightStillBusy)
        );
        assert_eq!(busy, before);
    }
}

#[test]
fn retained_body_custody_recovery_respects_the_exact_durable_decision() {
    let subject = Subject::repeat(0xe8);
    let (mut recovered, decision) = decided_reducer(subject);
    let tag = recovered.current_tag();
    let round = decision.proposal_round();
    let before = recovered.clone();
    assert_eq!(
        recovered.restore_retained_body_pipeline_custody(
            tag,
            round,
            retained_body_manifest(Subject::repeat(0xe9)),
            false,
        ),
        Err(ReducerError::InvalidRetainedBodyPipeline)
    );
    assert_eq!(recovered, before);
    recovered
        .restore_retained_body_pipeline_custody(tag, round, retained_body_manifest(subject), true)
        .expect("only the exact decision body occurrence may retain custody");
    let mut expected = before;
    expected.body_work.insert(
        (round, subject),
        BodyWork {
            manifest: Some(retained_body_manifest(subject)),
            state: BodyState::Available,
        },
    );
    assert_eq!(recovered, expected);
}

#[test]
fn retained_body_custody_preserves_normal_proposal_validation_vote_authority() {
    let fixture = reducer();
    let mut recovered = Reducer::new(
        fixture.context.clone(),
        Some(fixture.context.leader(0)),
        fixture.generation,
    )
    .expect("use a committee role that may acquire the current proposal body");
    let tag = recovered.current_tag();
    let round = Round::new(tag.height(), tag.view());
    let subject = Subject::repeat(0xea);
    let manifest = retained_body_manifest(subject);
    let proposal = SignedProposal::new(
        Proposal::new(
            recovered.context.id(),
            round,
            recovered.context.leader(round.view()),
            manifest,
            ProposalJustification::ParentCommit(recovered.context.parent_commit()),
        ),
        OpaqueSignature::new(vec![0xeb; 8]),
    );
    let admitted = recovered
        .step(Event::ProposalReceived { tag, proposal })
        .expect("restore the signed current proposal through normal admission");
    assert!(matches!(admitted.effects(), [Effect::FetchBody { .. }]));
    let before = recovered.clone();
    recovered
        .restore_retained_body_pipeline_custody(tag, round, manifest, false)
        .expect("exact retained custody coalesces with authenticated candidate work");
    assert_eq!(recovered, before);
    recovered
        .step(Event::BodyAvailable {
            tag,
            round,
            subject,
        })
        .expect("body becomes available");
    recovered
        .step(Event::BodyStored {
            tag,
            round,
            subject,
        })
        .expect("body becomes durable");
    let validated = recovered
        .step(Event::ValidationCompleted {
            tag,
            round,
            subject,
            valid: true,
        })
        .expect("the actual proposal still authorizes a Prepare intent");
    assert!(matches!(
        validated.effects(),
        [Effect::Persist { entry, .. }]
            if matches!(entry.record(), WalRecord::PrepareIntent(vote)
                if vote.round() == round && vote.subject() == subject)
    ));
}
