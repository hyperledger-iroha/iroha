fn assert_certified_fetch(
    reducer: &Reducer,
    outcome: &StepOutcome,
    certificate: &QuorumCertificate,
    manifest: Option<PayloadManifest>,
) {
    let sources = reducer
        .context()
        .roster()
        .iter()
        .map(|validator| validator.id())
        .collect::<Vec<_>>();
    assert_eq!(
        outcome
            .effects()
            .iter()
            .filter(|effect| matches!(effect, Effect::FetchBody { .. }))
            .count(),
        1
    );
    assert!(outcome.effects().iter().any(|effect| matches!(
        effect,
        Effect::FetchBody {
            tag,
            round,
            subject,
            manifest: fetched_manifest,
            certified_sources,
            certificate: Some(fetched_certificate),
        } if *tag == reducer.current_tag()
            && *round == certificate.round()
            && *subject == certificate.subject()
            && fetched_manifest == &manifest
            && certified_sources == &sources
            && fetched_certificate == certificate
    )));
}
fn assert_certified_fallback(reducer: &mut Reducer, certificate: &QuorumCertificate) {
    let fallback = reducer
        .step(Event::RetransmitElapsed {
            tag: reducer.current_tag(),
        })
        .expect("the retransmission boundary retries the exact certified body");
    assert_certified_fetch(reducer, &fallback, certificate, None);
}
#[test]
fn height_context_requires_bounded_three_f_plus_one_geometry() {
    assert!(matches!(
        try_context_with_powers(VotingMode::Permissioned, &[1, 1, 1]),
        Err(HeightContextError::RosterTooSmall)
    ));
    assert!(matches!(
        try_context_with_powers(VotingMode::Permissioned, &[1, 1, 1, 1, 1]),
        Err(HeightContextError::InvalidCommitteeGeometry)
    ));
    assert!(try_context_with_powers(VotingMode::Permissioned, &[1; 31]).is_ok());
}
#[test]
fn set_a_validator_fetches_the_initial_proposal_body() {
    let context = context();
    let subject = Subject::repeat(0x65);
    let mut reducer = Reducer::new(context.clone(), Some(id(1)), Generation::new(60)).unwrap();
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
        .expect("Set A accepts the current leader proposal");
    assert!(matches!(
        received.effects(),
        [Effect::FetchBody {
            subject: fetched,
            certificate: None,
            ..
        }] if *fetched == subject
    ));
}
#[test]
fn observer_does_not_fetch_an_uncertified_initial_proposal() {
    let context = context();
    let subject = Subject::repeat(0x63);
    let mut reducer = Reducer::new(context.clone(), None, Generation::new(64)).unwrap();
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
        .expect("observer retains authenticated proposal control");
    assert!(received.effects().is_empty());
}
#[test]
fn set_b_validator_defers_the_body_until_same_view_fallback() {
    let context = context();
    let subject = Subject::repeat(0x66);
    let mut reducer = Reducer::new(context.clone(), Some(id(4)), Generation::new(61)).unwrap();
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
        .expect("Set B retains the current leader proposal");
    assert!(received.effects().is_empty());
    assert!(
        reducer
            .durable_state()
            .prepare_intent(Round::new(context.height(), 0))
            .is_none()
    );
    let fallback = reducer
        .step(Event::RetransmitElapsed {
            tag: reducer.current_tag(),
        })
        .expect("the first retransmission tick activates same-view fallback");
    assert!(matches!(
        fallback.effects(),
        [Effect::FetchBody {
            subject: fetched,
            certificate: None,
            ..
        }] if *fetched == subject
    ));
}
#[test]
fn retransmit_before_a_proposal_does_not_prearm_set_b_fallback() {
    let context = context();
    let subject = Subject::repeat(0x62);
    let mut reducer = Reducer::new(context.clone(), Some(id(4)), Generation::new(65)).unwrap();
    let early_tick = reducer
        .step(Event::RetransmitElapsed {
            tag: reducer.current_tag(),
        })
        .expect("an idle retransmission tick is harmless");
    assert!(early_tick.effects().is_empty());
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
        .expect("Set B retains the proposal after the idle tick");
    assert!(
        received.effects().is_empty(),
        "fallback starts only on a proposal-scoped retransmission boundary"
    );
    let proposal_tick = reducer
        .step(Event::RetransmitElapsed {
            tag: reducer.current_tag(),
        })
        .expect("the proposal-scoped tick activates fallback");
    assert!(matches!(
        proposal_tick.effects(),
        [Effect::FetchBody {
            subject: fetched,
            certificate: None,
            ..
        }] if *fetched == subject
    ));
}
#[test]
fn set_b_validator_cannot_vote_before_same_view_fallback() {
    let context = context();
    let subject = Subject::repeat(0x64);
    let round = Round::new(context.height(), 0);
    let mut reducer = Reducer::new(context.clone(), Some(id(4)), Generation::new(63)).unwrap();
    reducer
        .step(Event::ProposalReceived {
            tag: reducer.current_tag(),
            proposal: proposal(
                &context,
                0,
                subject,
                ProposalJustification::ParentCommit(context.parent_commit()),
            ),
        })
        .expect("Set B retains the proposal without fetching it");
    let tag = reducer.current_tag();
    reducer
        .step(Event::BodyAvailable {
            tag,
            round,
            subject,
        })
        .expect("an out-of-order exact body cannot bypass the reducer gate");
    reducer
        .step(Event::BodyStored {
            tag,
            round,
            subject,
        })
        .expect("the exact body reaches deterministic validation");
    let validated = reducer
        .step(Event::ValidationCompleted {
            tag,
            round,
            subject,
            valid: true,
        })
        .expect("validating an early Set B body remains non-voting");
    assert!(validated.effects().is_empty());
    assert!(reducer.durable_state().prepare_intent(round).is_none());
    let fallback = reducer
        .step(Event::RetransmitElapsed {
            tag: reducer.current_tag(),
        })
        .expect("fallback resumes a body which arrived before the local tick");
    assert!(fallback.effects().iter().any(|effect| matches!(
        effect,
        Effect::Persist { entry, .. }
            if matches!(entry.record(), WalRecord::PrepareIntent(vote)
                if vote.round() == round && vote.subject() == subject)
    )));
}
#[test]
fn certified_view_change_resets_set_b_fallback() {
    let roster = (1_u8..=7)
        .map(|validator| Validator::new(id(validator), VotingPower::new(1)))
        .collect();
    let context = HeightContext::new(
        ContextId::repeat(0x67),
        NetworkId::repeat(0x68),
        2,
        Some(CertificateRef::new(
            ContextId::repeat(0x69),
            Round::new(1, 0),
            Phase::Commit,
            Subject::repeat(0x6a),
        )),
        7,
        roster,
        VotingMode::Permissioned,
        Digest::repeat(0x6b),
        Digest::repeat(0x6c),
        Digest::repeat(0x6d),
        Digest::repeat(0),
    )
    .expect("seven-validator production geometry");
    let local = id(7);
    for view in [0, 1] {
        assert_eq!(
            Committee::project(&context, view)
                .expect("valid committee")
                .role(6)
                .expect("stable local index"),
            CommitteeRole::SetBValidator
        );
    }
    let mut reducer = Reducer::new(context.clone(), Some(local), Generation::new(62)).unwrap();
    let first_subject = Subject::repeat(0x6e);
    let first = reducer
        .step(Event::ProposalReceived {
            tag: reducer.current_tag(),
            proposal: proposal(
                &context,
                0,
                first_subject,
                ProposalJustification::ParentCommit(context.parent_commit()),
            ),
        })
        .expect("view-zero Set B proposal is retained");
    assert!(first.effects().is_empty());
    let fallback = reducer
        .step(Event::RetransmitElapsed {
            tag: reducer.current_tag(),
        })
        .expect("view-zero fallback activates");
    assert!(matches!(fallback.effects(), [Effect::FetchBody { .. }]));
    let timeout = tc_without_high(&context, 0, &[1, 2, 3, 4, 5]);
    let install = only_persist(
        reducer
            .step(Event::TimeoutCertificateReceived {
                tag: reducer.current_tag(),
                certificate: timeout.clone(),
            })
            .expect("the timeout certificate starts its durable install"),
    );
    acknowledge(&mut reducer, &install);
    assert_eq!(reducer.current_tag().view(), 1);
    let next_subject = Subject::repeat(0x6f);
    let next = reducer
        .step(Event::ProposalReceived {
            tag: reducer.current_tag(),
            proposal: proposal(
                &context,
                1,
                next_subject,
                ProposalJustification::Timeout(timeout),
            ),
        })
        .expect("view-one Set B proposal is retained");
    assert!(
        next.effects().is_empty(),
        "a certified view transition must reset same-view fallback"
    );
}
#[test]
fn same_view_timeout_upgrade_resets_set_b_fallback() {
    let roster = (1_u8..=7)
        .map(|validator| Validator::new(id(validator), VotingPower::new(1)))
        .collect();
    let context = HeightContext::new(
        ContextId::repeat(0x70),
        NetworkId::repeat(0x71),
        2,
        Some(CertificateRef::new(
            ContextId::repeat(0x72),
            Round::new(1, 0),
            Phase::Commit,
            Subject::repeat(0x73),
        )),
        7,
        roster,
        VotingMode::Permissioned,
        Digest::repeat(0x74),
        Digest::repeat(0x75),
        Digest::repeat(0x76),
        Digest::repeat(0),
    )
    .expect("seven-validator production geometry");
    let view_one_committee = Committee::project(&context, 1).expect("valid view-one committee");
    let local_index = *view_one_committee
        .set_b()
        .first()
        .expect("seven-validator committee has Set B");
    let local = context.roster()[usize::try_from(local_index).expect("small fixture index")].id();
    let local_byte = u8::try_from(local_index + 1).expect("small fixture validator");
    let remote_quorum = (1_u8..=7)
        .filter(|validator| *validator != local_byte)
        .take(5)
        .collect::<Vec<_>>();
    assert_eq!(
        view_one_committee
            .role(local_index)
            .expect("stable local index"),
        CommitteeRole::SetBValidator
    );
    let first_timeout = tc_without_high(&context, 0, &remote_quorum);
    let mut reducer = Reducer::new(context.clone(), Some(local), Generation::new(66)).unwrap();
    let first_install = only_persist(
        reducer
            .step(Event::TimeoutCertificateReceived {
                tag: reducer.current_tag(),
                certificate: first_timeout.clone(),
            })
            .expect("the first timeout certificate enters view one"),
    );
    acknowledge(&mut reducer, &first_install);
    assert_eq!(reducer.current_tag().view(), 1);
    let first_subject = Subject::repeat(0x77);
    let first_proposal = reducer
        .step(Event::ProposalReceived {
            tag: reducer.current_tag(),
            proposal: proposal(
                &context,
                1,
                first_subject,
                ProposalJustification::Timeout(first_timeout),
            ),
        })
        .expect("Set B retains the first view-one proposal");
    assert!(first_proposal.effects().is_empty());
    let first_fallback = reducer
        .step(Event::RetransmitElapsed {
            tag: reducer.current_tag(),
        })
        .expect("the first proposal activates same-view fallback");
    assert!(first_fallback.effects().iter().any(|effect| matches!(
        effect,
        Effect::FetchBody {
            round,
            subject,
            certificate: None,
            ..
        } if *round == Round::new(context.height(), 1) && *subject == first_subject
    )));
    let replacement_subject = Subject::repeat(0x78);
    let higher_prepare = qc(
        &context,
        0,
        Phase::Prepare,
        replacement_subject,
        &remote_quorum,
    );
    let upgrade = tc_with_high(&context, 0, higher_prepare.clone(), &remote_quorum);
    let before_upgrade = reducer.current_tag();
    let upgrade_install = only_persist(
        reducer
            .step(Event::TimeoutCertificateReceived {
                tag: reducer.current_tag(),
                certificate: upgrade.clone(),
            })
            .expect("the same-round certificate upgrades the protected lock"),
    );
    acknowledge(&mut reducer, &upgrade_install);
    assert_eq!(reducer.current_tag().view(), before_upgrade.view());
    assert_ne!(
        reducer.current_tag().generation(),
        before_upgrade.generation(),
        "the upgraded certificate starts a replacement proposal generation"
    );
    let replacement_proposal = proposal(
        &context,
        1,
        replacement_subject,
        ProposalJustification::Timeout(upgrade),
    );
    let replacement = reducer
        .step(Event::ProposalReceived {
            tag: reducer.current_tag(),
            proposal: replacement_proposal,
        })
        .expect("Set B retains the replacement proposal");
    assert!(
        replacement.effects().is_empty(),
        "a same-view timeout upgrade must reset fallback for the replacement proposal"
    );
    assert_certified_fallback(&mut reducer, &higher_prepare);
    let replacement_round = Round::new(context.height(), 1);
    assert!(matches!(
        reducer
            .step(Event::BodyAvailable {
                tag: reducer.current_tag(),
                round: replacement_round,
                subject: replacement_subject,
            })
            .expect("the replacement body becomes available after fallback")
            .effects(),
        [Effect::StoreBody { round, subject, .. }]
            if *round == replacement_round && *subject == replacement_subject
    ));
    assert!(matches!(
        reducer
            .step(Event::BodyStored {
                tag: reducer.current_tag(),
                round: replacement_round,
                subject: replacement_subject,
            })
            .expect("the replacement body reaches validation after fallback")
            .effects(),
        [Effect::ValidateBody { round, subject, .. }]
            if *round == replacement_round && *subject == replacement_subject
    ));
    let prepare = only_persist(
        reducer
            .step(Event::ValidationCompleted {
                tag: reducer.current_tag(),
                round: replacement_round,
                subject: replacement_subject,
                valid: true,
            })
            .expect("fallback authorizes the replacement Prepare intent"),
    );
    assert!(matches!(
        prepare.record(),
        WalRecord::PrepareIntent(vote)
            if vote.round() == replacement_round
                && vote.phase() == Phase::Prepare
                && vote.subject() == replacement_subject
                && vote.signer() == local
    ));
}
#[test]
fn retransmit_rebinds_durable_locked_validation_after_view_change() {
    let context = context();
    let subject = Subject::repeat(0x7c);
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
                vote: commit_vote.clone(),
            },
        ),
    ];
    let mut recovered =
        Reducer::recover(context.clone(), Some(id(4)), Generation::new(25), entries).unwrap();
    let resumed = resume_after_replay(&mut recovered);
    assert!(matches!(
        resumed.effects(),
        [Effect::Sign {
            message: SignableMessage::Vote(vote),
            ..
        }] if *vote == commit_vote
    ));
    complete_signature(&mut recovered, 0x7c);
    let available = recovered
        .step(Event::BodyAvailable {
            tag: recovered.current_tag(),
            round: prepare.round(),
            subject,
        })
        .expect("the locked body enters the durable pipeline");
    assert!(matches!(available.effects(), [Effect::StoreBody { .. }]));
    let stored = recovered
        .step(Event::BodyStored {
            tag: recovered.current_tag(),
            round: prepare.round(),
            subject,
        })
        .expect("the locked body starts validation in the original view");
    assert!(matches!(stored.effects(), [Effect::ValidateBody { .. }]));
    assert_eq!(
        recovered.body_state(prepare.round(), subject),
        BodyState::Durable
    );
    let old_tag = recovered.current_tag();
    let timeout = tc_with_high(&context, 0, prepare.clone(), &[1, 2, 3]);
    let install = only_persist(
        recovered
            .step(Event::TimeoutCertificateReceived {
                tag: old_tag,
                certificate: timeout,
            })
            .expect("the timeout certificate starts a certified view change"),
    );
    let entered = acknowledge(&mut recovered, &install);
    let current_tag = recovered.current_tag();
    assert_eq!(current_tag.view(), 1);
    assert_ne!(current_tag.generation(), old_tag.generation());
    assert!(entered.effects().iter().any(|effect| matches!(
        effect,
        Effect::EnterView { tag, .. } if *tag == current_tag
    )));
    assert!(entered.effects().iter().any(|effect| matches!(
        effect,
        Effect::FetchBody {
            tag,
            round,
            subject: fetched_subject,
            certificate: Some(certificate),
            ..
        } if *tag == current_tag
            && *round == prepare.round()
            && *fetched_subject == subject
            && certificate == &prepare
    )));
    assert_eq!(
        recovered.body_state(prepare.round(), subject),
        BodyState::Missing
    );
    let signed = complete_signature(&mut recovered, 0x7d);
    assert!(signed.effects().iter().any(|effect| matches!(
        effect,
        Effect::Broadcast(ConsensusMessageV2::Vote(vote)) if vote.vote() == commit_vote
    )));
    let rebound_available = recovered
        .step(Event::BodyAvailable {
            tag: current_tag,
            round: prepare.round(),
            subject,
        })
        .expect("the current view recovers the exact locked body");
    assert!(matches!(
        rebound_available.effects(),
        [Effect::StoreBody { tag, .. }] if *tag == current_tag
    ));
    let store_retry = recovered
        .step(Event::RetransmitElapsed { tag: current_tag })
        .expect("retransmission rebinds available locked storage to the current view");
    assert!(store_retry.effects().iter().any(|effect| matches!(
        effect,
        Effect::StoreBody {
            tag,
            round,
            subject: stored_subject,
        } if *tag == current_tag
            && *round == prepare.round()
            && *stored_subject == subject
    )));
    let rebound_stored = recovered
        .step(Event::BodyStored {
            tag: current_tag,
            round: prepare.round(),
            subject,
        })
        .expect("the current view restarts locked-body validation");
    assert!(matches!(
        rebound_stored.effects(),
        [Effect::ValidateBody { tag, .. }] if *tag == current_tag
    ));
    assert_eq!(
        recovered.body_state(prepare.round(), subject),
        BodyState::Durable
    );
    let retransmitted = recovered
        .step(Event::RetransmitElapsed { tag: current_tag })
        .expect("retransmission rebinds durable locked validation to the current view");
    assert_eq!(
        retransmitted
            .effects()
            .iter()
            .filter(|effect| matches!(effect, Effect::ValidateBody { .. }))
            .count(),
        1
    );
    assert!(retransmitted.effects().iter().any(|effect| matches!(
        effect,
        Effect::ValidateBody {
            tag,
            round,
            subject: validated_subject,
        } if *tag == current_tag
            && *round == prepare.round()
            && *validated_subject == subject
    )));
    recovered
        .step(Event::ValidationCompleted {
            tag: current_tag,
            round: prepare.round(),
            subject,
            valid: true,
        })
        .expect("the current-view validation completion is accepted");
    assert_eq!(
        recovered.body_state(prepare.round(), subject),
        BodyState::Validated
    );
    assert_eq!(recovered.durable_state().locked(), Some(&prepare));
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
    let resumed = resume_after_replay(&mut reducer);
    assert_eq!(resumed.disposition(), StepDisposition::Applied);
    assert!(resumed.effects().is_empty());
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
    assert_eq!(
        reducer.volatile_prepare_counts(),
        (0, 1),
        "an old-view durable high QC has no pending body-pipeline owner"
    );
    let older = qc(&context, 0, Phase::Prepare, old_subject, &[1, 2, 3]);
    let before_older = reducer.clone();
    let ignored = reducer
        .step(Event::QuorumCertificateReceived {
            tag: reducer.current_tag(),
            certificate: older,
        })
        .expect("an old PrepareQC is valid but cannot regress progress");
    assert_eq!(
        ignored.disposition(),
        StepDisposition::Ignored(IgnoreReason::IrrelevantView)
    );
    assert!(ignored.effects().is_empty());
    assert_eq!(reducer, before_older);
    assert_eq!(reducer.volatile_prepare_counts(), (0, 1));
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
fn live_timeout_fence_retries_the_durable_lock_instead_of_a_closed_prepare() {
    let context = context();
    let locked_subject = Subject::repeat(0x7d);
    let closed_subject = Subject::repeat(0x7e);
    let locked_prepare = qc(&context, 0, Phase::Prepare, locked_subject, &[1, 2, 3]);
    let locked_vote = Vote::new(
        context.id(),
        locked_prepare.round(),
        Phase::Commit,
        locked_subject,
        id(4),
    );
    let installed_timeout = tc_with_high(&context, 0, locked_prepare.clone(), &[1, 2, 3]);
    let closed_prepare = qc(&context, 1, Phase::Prepare, closed_subject, &[1, 2, 3]);
    let entries = [
        WalEntry::new(
            PersistenceId::new(1),
            WalRecord::LockAndCommit {
                prepare: locked_prepare.clone(),
                vote: locked_vote,
            },
        ),
        WalEntry::new(
            PersistenceId::new(2),
            WalRecord::InstallTimeout(installed_timeout),
        ),
    ];
    let mut reducer = Reducer::recover(context, Some(id(4)), Generation::new(26), entries).unwrap();
    assert!(matches!(
        resume_after_replay(&mut reducer).effects(),
        [Effect::Sign {
            message: SignableMessage::Vote(vote),
            ..
        }] if vote == &locked_vote
    ));
    assert!(matches!(
        complete_signature(&mut reducer, 0x7d).effects(),
        [Effect::Broadcast(ConsensusMessageV2::Vote(vote))]
            if vote.vote() == locked_vote
    ));

    let timeout = only_persist(
        reducer
            .step(Event::TimeoutElapsed {
                tag: reducer.current_tag(),
            })
            .expect("the live current view starts its durable timeout"),
    );
    assert!(matches!(
        acknowledge(&mut reducer, &timeout).effects(),
        [Effect::Sign {
            message: SignableMessage::TimeoutVote(_),
            ..
        }]
    ));
    assert!(matches!(
        complete_signature(&mut reducer, 0x7e).effects(),
        [Effect::Broadcast(ConsensusMessageV2::TimeoutVote(_))]
    ));

    let late_prepare = reducer
        .step(Event::QuorumCertificateReceived {
            tag: reducer.current_tag(),
            certificate: closed_prepare.clone(),
        })
        .expect("the timeout-fenced PrepareQC remains valid control evidence");
    assert!(
        late_prepare
            .effects()
            .iter()
            .all(|effect| !matches!(effect, Effect::FetchBody { .. })),
        "late closed-view certification cannot recreate body acquisition"
    );
    let late_prepare = only_persist(late_prepare);
    assert!(
        acknowledge(&mut reducer, &late_prepare)
            .effects()
            .is_empty(),
        "persisting late control evidence cannot create closed-view body work"
    );

    let retransmitted = reducer
        .step(Event::RetransmitElapsed {
            tag: reducer.current_tag(),
        })
        .expect("the closed view keeps durable lock recovery live");
    assert!(retransmitted.effects().iter().any(|effect| matches!(
        effect,
        Effect::Broadcast(ConsensusMessageV2::QuorumCertificate(certificate))
            if certificate == &closed_prepare
    )));
    assert_certified_fetch(&reducer, &retransmitted, &locked_prepare, None);
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
