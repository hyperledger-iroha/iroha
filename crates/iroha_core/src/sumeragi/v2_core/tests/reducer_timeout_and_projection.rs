#[test]
fn recovery_restores_tc_promoted_lock_body_before_any_retry() {
    let fixture = reducer();
    let context = fixture.context.clone();
    let locked = certificate(&context, 0, Phase::Prepare, Subject::repeat(0xd1), 0xd2);
    let entries = [WalEntry::new(
        PersistenceId::new(1),
        WalRecord::InstallTimeout(timeout_certificate(&context, 0, Some(locked.clone()))),
    )];
    let mut recovered = Reducer::recover(
        context.clone(),
        fixture.local_validator,
        Generation::INITIAL,
        entries,
    )
    .expect("restore the TC-promoted lock without a local Commit intent");
    assert_eq!(recovered.durable.locked(), Some(&locked));
    assert!(recovered.durable.commit_intent_for_lock(&locked).is_none());
    assert_eq!(recovered.body_work.len(), 1);
    assert!(recovered.pending_prepare.is_empty());
    assert_eq!(recovered.known_prepare.len(), 1);
    assert_eq!(
        recovered.body_work[&(locked.round(), locked.subject())],
        BodyWork {
            manifest: None,
            state: BodyState::Missing,
        }
    );
    let completion = Event::BodyAvailable {
        tag: recovered.current_tag(),
        round: locked.round(),
        subject: locked.subject(),
    };
    let before = recovered.clone();
    let fenced = recovered
        .step(completion.clone())
        .expect("startup is still gated");
    assert_eq!(
        fenced.disposition(),
        StepDisposition::Ignored(IgnoreReason::RecoveryPending)
    );
    assert!(fenced.effects().is_empty());
    assert_eq!(recovered, before);
    let resumed = recovered
        .step(Event::ResumeAfterReplay {
            tag: recovered.current_tag(),
        })
        .expect("resume the exact durable authority");
    assert_eq!(resumed.disposition(), StepDisposition::Applied);
    assert!(
        resumed.effects().is_empty(),
        "constructor seed must not invent a startup Fetch or signature"
    );
    for (round, subject) in [
        (Round::new(context.height(), 1), locked.subject()),
        (locked.round(), Subject::repeat(0xd3)),
    ] {
        let before = recovered.clone();
        let ignored = recovered
            .step(Event::BodyAvailable {
                tag: recovered.current_tag(),
                round,
                subject,
            })
            .expect("unrelated completion remains a stutter");
        assert_eq!(
            ignored.disposition(),
            StepDisposition::Ignored(IgnoreReason::NoMatchingWork)
        );
        assert!(ignored.effects().is_empty());
        assert_eq!(recovered, before);
    }
    let available = recovered
        .step(completion)
        .expect("the retained exact completion applies before any retry timer");
    assert_eq!(
        available.effects(),
        &[Effect::StoreBody {
            tag: recovered.current_tag(),
            round: locked.round(),
            subject: locked.subject(),
        }]
    );
    assert_eq!(
        recovered.body_state(locked.round(), locked.subject()),
        BodyState::Available
    );
    let _fetch = recovered.ensure_body_fetch(&locked);
    assert_eq!(
        recovered.body_state(locked.round(), locked.subject()),
        BodyState::Available,
        "sharing Missing insertion must not reset a live body stage"
    );
    assert_eq!(recovered.body_work.len(), 1);
    let stored = recovered
        .step(Event::BodyStored {
            tag: recovered.current_tag(),
            round: locked.round(),
            subject: locked.subject(),
        })
        .expect("the exact stored completion advances the retained pipeline");
    assert_eq!(
        stored.effects(),
        &[Effect::ValidateBody {
            tag: recovered.current_tag(),
            round: locked.round(),
            subject: locked.subject(),
        }]
    );
    assert_eq!(
        recovered.body_state(locked.round(), locked.subject()),
        BodyState::Durable
    );
    assert!(recovered.pending_prepare.is_empty());
    assert!(recovered.awaiting_signature.is_none());
}
#[test]
fn recovery_body_seeds_are_bounded_and_decision_exclusive() {
    let fixture = reducer();
    let context = fixture.context.clone();
    let locked = certificate(&context, 0, Phase::Prepare, Subject::repeat(0xd4), 0xd5);
    let current = certificate(&context, 1, Phase::Prepare, Subject::repeat(0xd6), 0xd7);
    let mut entries = vec![
        WalEntry::new(
            PersistenceId::new(1),
            WalRecord::InstallTimeout(timeout_certificate(&context, 0, Some(locked.clone()))),
        ),
        WalEntry::new(
            PersistenceId::new(2),
            WalRecord::ObservePrepare(current.clone()),
        ),
    ];
    let recovered = Reducer::recover(
        context.clone(),
        fixture.local_validator,
        Generation::INITIAL,
        entries.clone(),
    )
    .expect("restore exactly the historical lock and open-view high");
    assert_eq!(
        recovered.body_work.keys().copied().collect::<Vec<_>>(),
        vec![
            (locked.round(), locked.subject()),
            (current.round(), current.subject())
        ]
    );
    assert_eq!(recovered.pending_prepare.len(), 1);
    assert_eq!(
        recovered.pending_prepare.get(&current.reference()),
        Some(&current)
    );
    assert!(!recovered.pending_prepare.contains_key(&locked.reference()));
    let decision = certificate(&context, 1, Phase::Commit, current.subject(), 0xd8);
    entries.push(WalEntry::new(
        PersistenceId::new(3),
        WalRecord::Decision(decision.clone()),
    ));
    let mut decided = Reducer::recover(
        context,
        fixture.local_validator,
        Generation::INITIAL,
        entries,
    )
    .expect("a Decision owns the existing exclusive recovery path");
    assert!(decided.body_work.is_empty());
    assert!(decided.pending_prepare.is_empty());
    let outcome = decided
        .step(Event::ResumeAfterReplay {
            tag: decided.current_tag(),
        })
        .expect("resume only the exact Decision body");
    assert!(
        matches!(outcome.effects(), [Effect::FetchBody { round, subject, certificate: Some(actual), .. }]
        if *round == decision.proposal_round() && *subject == decision.subject() && actual == &decision)
    );
    assert_eq!(decided.body_work.len(), 1);
    assert!(
        !decided
            .body_work
            .contains_key(&(locked.round(), locked.subject()))
    );
}
#[test]
fn recovery_without_timeout_preserves_the_exact_caller_generation_seed() {
    let fixture = reducer();
    let context = fixture.context.clone();
    let local = fixture.local_validator.expect("validator fixture");
    let prepare = WalEntry::new(
        PersistenceId::new(1),
        WalRecord::PrepareIntent(Vote::new(
            context.id(),
            Round::new(context.height(), 0),
            Phase::Prepare,
            Subject::repeat(0xd9),
            local,
        )),
    );
    for seed in [
        Generation::INITIAL,
        Generation::new(37),
        Generation::new(u64::MAX),
    ] {
        for entries in [vec![], vec![prepare.clone()]] {
            let expected = DurableState::replay(&context, Some(local), entries.clone())
                .expect("valid no-TC history");
            let recovered = Reducer::recover(context.clone(), Some(local), seed, entries)
                .expect("retain caller seed");
            assert_eq!(recovered.current_tag().generation(), seed);
            assert_eq!(recovered.durable, expected);
            assert!(!recovered.replay_resumed);
        }
    }
}
#[test]
fn recovery_reconstructs_live_generation_for_each_timeout_and_non_timeout_frame() {
    let fixture = reducer();
    let context = fixture.context.clone();
    let high0 = certificate(&context, 0, Phase::Prepare, Subject::repeat(0xda), 0xdb);
    let high1 = certificate(&context, 1, Phase::Prepare, Subject::repeat(0xdc), 0xdd);
    let high2 = certificate(&context, 2, Phase::Prepare, Subject::repeat(0xde), 0xdf);
    let records = [
        (
            WalRecord::InstallTimeout(timeout_certificate(&context, 0, None)),
            1,
            0,
        ),
        (
            WalRecord::InstallTimeout(timeout_certificate(&context, 1, None)),
            2,
            0,
        ),
        (
            WalRecord::InstallTimeout(timeout_certificate(&context, 1, Some(high0))),
            2,
            1,
        ),
        (
            WalRecord::InstallTimeout(timeout_certificate(&context, 1, Some(high1))),
            2,
            2,
        ),
        (WalRecord::ObservePrepare(high2.clone()), 2, 2),
        (
            WalRecord::InstallTimeout(timeout_certificate(&context, 2, Some(high2))),
            3,
            0,
        ),
    ];
    for seed in [
        Generation::INITIAL,
        Generation::new(37),
        Generation::new(u64::MAX),
    ] {
        let mut live = Reducer::new(context.clone(), None, seed).expect("live observer");
        let mut entries = Vec::new();
        for (record, expected_view, expected_generation) in records.clone() {
            let event = match &record {
                WalRecord::InstallTimeout(certificate) => Event::TimeoutCertificateReceived {
                    tag: live.current_tag(),
                    certificate: certificate.clone(),
                },
                WalRecord::ObservePrepare(certificate) => Event::QuorumCertificateReceived {
                    tag: live.current_tag(),
                    certificate: certificate.clone(),
                },
                _ => unreachable!("fixture admits only TC and observed Prepare frames"),
            };
            let outcome = live
                .step(event)
                .expect("ordinary ingress stages the exact next durable frame");
            let mut persisted = outcome.effects().iter().filter_map(|effect| match effect {
                Effect::Persist { entry, .. } => Some(entry.clone()),
                _ => None,
            });
            let entry = persisted.next().expect("one WAL append is required");
            assert!(persisted.next().is_none());
            assert_eq!(entry.record(), &record);
            live.step(Event::Persisted {
                tag: live.current_tag(),
                id: entry.id(),
            })
            .expect("acknowledge the live transition");
            entries.push(entry);
            assert_eq!(
                live.current_tag(),
                EventTag::new(
                    context.height(),
                    expected_view,
                    Generation::new(expected_generation)
                )
            );
            let recovered = Reducer::recover(context.clone(), None, seed, entries.clone())
                .expect("recover the complete exact acknowledged prefix");
            assert_eq!(recovered.current_tag(), live.current_tag());
            assert_eq!(recovered.durable, live.durable);
            assert!(!recovered.replay_resumed);
            assert!(recovered.pending_persistence.is_none());
            assert!(recovered.awaiting_signature.is_none());
        }
    }
}
#[test]
fn recovery_generation_fold_retains_ordered_wal_rejections() {
    let fixture = reducer();
    let context = fixture.context.clone();
    let timeout = timeout_certificate(&context, 0, None);
    let first = WalEntry::new(
        PersistenceId::new(1),
        WalRecord::InstallTimeout(timeout.clone()),
    );
    let foreign = TimeoutCertificate::new(
        ContextId::repeat(0xe1),
        timeout.round(),
        timeout.groups().to_vec(),
    );
    let malformed = TimeoutCertificate::new(context.id(), timeout.round(), vec![]);
    let decision = certificate(&context, 0, Phase::Commit, Subject::repeat(0xe2), 0xe3);
    let histories = [
        vec![WalEntry::new(
            PersistenceId::new(1),
            WalRecord::InstallTimeout(foreign),
        )],
        vec![WalEntry::new(
            PersistenceId::new(1),
            WalRecord::InstallTimeout(malformed),
        )],
        vec![
            WalEntry::new(PersistenceId::new(1), WalRecord::Decision(decision)),
            WalEntry::new(
                PersistenceId::new(2),
                WalRecord::InstallTimeout(timeout.clone()),
            ),
        ],
        vec![WalEntry::new(
            PersistenceId::new(2),
            WalRecord::InstallTimeout(timeout.clone()),
        )],
        vec![
            first.clone(),
            WalEntry::new(
                PersistenceId::new(3),
                WalRecord::InstallTimeout(timeout.clone()),
            ),
        ],
        vec![
            first,
            WalEntry::new(PersistenceId::new(2), WalRecord::InstallTimeout(timeout)),
        ],
    ];
    for entries in histories {
        let expected = DurableState::replay(&context, fixture.local_validator, entries.clone())
            .expect_err("invalid history must fail the canonical durable validator");
        assert_eq!(
            Reducer::recover(
                context.clone(),
                fixture.local_validator,
                Generation::new(u64::MAX),
                entries
            ),
            Err(ReducerError::Replay(expected))
        );
    }
}
#[test]
fn view_advancing_timeout_install_resets_an_exhausted_generation() {
    let (mut pending, event) =
        pending_timeout_install_at_generation(Generation::new(u64::MAX), None);
    let outcome = pending
        .step(event)
        .expect("a view advance does not consume the same-view generation");
    assert_eq!(outcome.disposition(), StepDisposition::Applied);
    assert_eq!(pending.generation, Generation::INITIAL);
    assert_eq!(pending.durable.current_view(), 1);
    assert!(pending.pending_persistence.is_none());
}
#[test]
fn same_round_timeout_upgrade_accepts_the_last_generation() {
    let (mut pending, event) =
        pending_same_round_timeout_upgrade_at_generation(Generation::new(u64::MAX - 1));
    let outcome = pending
        .step(event)
        .expect("the final representable same-view generation remains installable");
    assert_eq!(outcome.disposition(), StepDisposition::Applied);
    assert_eq!(pending.generation, Generation::new(u64::MAX));
    assert_eq!(pending.durable.current_view(), 1);
    assert!(pending.pending_persistence.is_none());
}
#[test]
fn same_round_timeout_generation_overflow_preserves_the_complete_state() {
    let (mut pending, event) =
        pending_same_round_timeout_upgrade_at_generation(Generation::new(u64::MAX));
    let before = pending.clone();
    let error = pending
        .step(event.clone())
        .expect_err("an exhausted generation must reject the install");
    assert_eq!(error, ReducerError::GenerationOverflow);
    assert_eq!(pending, before);
    let Event::Persisted { id, .. } = event else {
        panic!("timeout-install fixture must return a persistence acknowledgement")
    };
    let mut in_place = before.clone();
    let error = in_place
        .on_persisted(id)
        .expect_err("the in-place callback must precheck generation exhaustion");
    assert_eq!(error, ReducerError::GenerationOverflow);
    assert_eq!(in_place, before);
}
fn composite_replay_reducer() -> Reducer {
    let fixture = reducer();
    let context = fixture.context.clone();
    let local = context.leader(0);
    let subject = Subject::repeat(0xc1);
    let current_round = Round::new(context.height(), 0);
    let prepare = certificate(&context, 0, Phase::Prepare, subject, 0xc2);
    let manifest =
        PayloadManifest::new(subject, Digest::repeat(0xc3), Digest::repeat(0xc4), 512, 4);
    let proposal = Proposal::new(
        context.id(),
        current_round,
        local,
        manifest,
        ProposalJustification::ParentCommit(context.parent_commit()),
    );
    let entries = [
        WalEntry::new(PersistenceId::new(1), WalRecord::ProposalIntent(proposal)),
        WalEntry::new(
            PersistenceId::new(2),
            WalRecord::PrepareIntent(Vote::new(
                context.id(),
                current_round,
                Phase::Prepare,
                subject,
                local,
            )),
        ),
        WalEntry::new(
            PersistenceId::new(3),
            WalRecord::LockAndCommit {
                prepare,
                vote: Vote::new(context.id(), current_round, Phase::Commit, subject, local),
            },
        ),
    ];
    Reducer::recover(context, Some(local), Generation::new(9), entries)
        .expect("recover proposal, Prepare, and Commit for one exact origin")
}
#[test]
fn replay_refinement_binds_the_complete_durable_fifo() {
    let before = composite_replay_reducer();
    let expected = before.expected_replay_plan();
    assert_eq!(expected.len, 3);
    assert_eq!(expected.slot0.kind, REPLAY_EFFECT_PROPOSAL);
    assert_eq!(expected.slot1.kind, REPLAY_EFFECT_PREPARE);
    assert_eq!(expected.slot2.kind, REPLAY_EFFECT_COMMIT);
    let event = Event::ResumeAfterReplay {
        tag: before.current_tag(),
    };
    let mut after = before.clone();
    let outcome = after
        .step_in_place(event.clone())
        .expect("materialize the production replay candidate");
    let projection = before.transition_projection(&event, &after, outcome.effects());
    assert!(refinement::accepts(projection));
    assert_eq!(projection.boundary_claimed.replay_plan, expected);
    assert_eq!(projection.boundary_granted.replay_plan, expected);
    let duplicate_event = Event::ResumeAfterReplay {
        tag: after.current_tag(),
    };
    let mut destructive_duplicate = after.clone();
    destructive_duplicate.signature_queue.pop_back();
    assert!(!after.transition_refines(&duplicate_event, &destructive_duplicate, &[]));
    let mut omitted = projection;
    omitted.boundary_claimed.replay_plan.len = 2;
    omitted.boundary_claimed.replay_plan.slot2 = refinement::ReplayPlanSlotProjection::none();
    assert!(!refinement::accepts(omitted));
    let mut reordered = projection;
    let replay_plan = &mut reordered.boundary_claimed.replay_plan;
    std::mem::swap(&mut replay_plan.slot1, &mut replay_plan.slot2);
    assert!(!refinement::accepts(reordered));
    let mut substituted = projection;
    substituted
        .boundary_claimed
        .replay_plan
        .slot2
        .capability
        .subject = Subject::repeat(0xc5);
    assert!(!refinement::accepts(substituted));
}
#[test]
fn replay_refinement_rejects_malformed_post_states_even_with_the_right_first_effect() {
    let before = composite_replay_reducer();
    let messages = before.expected_replay_signatures();
    assert_eq!(messages.len(), 3);
    let first = messages[0].clone();
    let effect = Effect::Sign {
        tag: before.current_tag(),
        message: first.clone(),
    };
    let event = Event::ResumeAfterReplay {
        tag: before.current_tag(),
    };
    let mut dropped_all = before.clone();
    dropped_all.replay_resumed = true;
    assert!(!before.transition_refines(&event, &dropped_all, &[]));
    let mut omitted = before.clone();
    omitted.replay_resumed = true;
    omitted.awaiting_signature = Some(first.clone());
    omitted.signature_queue.push_back(messages[2].clone());
    assert!(!before.transition_refines(&event, &omitted, std::slice::from_ref(&effect),));
    let mut reordered = before.clone();
    reordered.replay_resumed = true;
    reordered.awaiting_signature = Some(first);
    reordered.signature_queue.push_back(messages[2].clone());
    reordered.signature_queue.push_back(messages[1].clone());
    assert!(!before.transition_refines(&event, &reordered, &[effect]));
}
#[test]
fn enter_view_projection_selects_and_fetches_the_exact_post_install_lock() {
    let fixture = reducer();
    let subject = Subject::repeat(0xb1);
    let high = certificate(&fixture.context, 0, Phase::Prepare, subject, 0xb2);
    let (before, event) = pending_timeout_install(Some(high.clone()));
    let mut after = before.clone();
    let outcome = after
        .step_in_place(event.clone())
        .expect("materialize persisted-TC candidate");
    assert!(matches!(
        outcome.effects(),
        [
            Effect::EnterView {
                protected_lock: Some(protected),
                ..
            },
            Effect::FetchBody {
                certificate: Some(fetched),
                ..
            }
        ] if protected == &high && fetched == &high
    ));
    let projection = before.transition_projection(&event, &after, outcome.effects());
    assert!(refinement::accepts(projection));
    for projected in [
        projection.enter_view.pending_record_timeout.highest_prepare,
        projection
            .enter_view
            .pending_continuation_timeout
            .highest_prepare,
        projection.enter_view.durable_timeout_after.highest_prepare,
        projection.enter_view.effect_timeout.highest_prepare,
        projection.enter_view.incoming_highest_for_control,
        projection.enter_view.durable_lock_after,
        projection.enter_view.durable_highest_after,
        projection.enter_view.retained_prepare_qc_after,
        projection.enter_view.effect_protected_lock,
        projection.enter_view.following_fetch_lock,
    ] {
        assert_eq!(projected.signer_bitmap, 0b111);
        assert_eq!(projected.signer_bitmap_count, 3);
        assert_eq!(projected.signer_count, 3);
        assert_eq!(projected.voting_power, 3);
        assert_eq!(projected.evidence_class, CERTIFICATE_EVIDENCE_INCOMING);
    }
    let mut mismatched_effect_lock = projection;
    mismatched_effect_lock
        .enter_view
        .effect_protected_lock
        .subject = Reducer::subject_identity_projection(Subject::repeat(0xb3));
    assert!(!refinement::accepts(mismatched_effect_lock));
    let mut mismatched_signer_set = projection;
    mismatched_signer_set
        .enter_view
        .effect_protected_lock
        .signer_bitmap ^= 1u128 << 3;
    assert!(!refinement::accepts(mismatched_signer_set));
    let mut mismatched_signer_count = projection;
    mismatched_signer_count
        .enter_view
        .effect_protected_lock
        .signer_count += 1;
    assert!(!refinement::accepts(mismatched_signer_count));
    let mut mismatched_bitmap_count = projection;
    mismatched_bitmap_count
        .enter_view
        .effect_protected_lock
        .signer_bitmap_count += 1;
    assert!(!refinement::accepts(mismatched_bitmap_count));
    let mut mismatched_voting_power = projection;
    mismatched_voting_power
        .enter_view
        .effect_protected_lock
        .voting_power += 1;
    assert!(!refinement::accepts(mismatched_voting_power));
    let mut foreign_evidence = projection;
    foreign_evidence
        .enter_view
        .effect_protected_lock
        .evidence_class = CERTIFICATE_EVIDENCE_FOREIGN;
    assert!(!refinement::accepts(foreign_evidence));
    let mut missing_fetch = projection;
    missing_fetch.enter_view.following_fetch_lock.present = false;
    assert!(!refinement::accepts(missing_fetch));
    let mut missing_prepare_control = projection;
    missing_prepare_control.enter_view.retained_prepare_qc_after =
        CertificateIdentityProjection::default();
    assert!(!refinement::accepts(missing_prepare_control));
    let mut stale_prepare_control = projection;
    stale_prepare_control
        .enter_view
        .retained_prepare_qc_after
        .subject = Reducer::subject_identity_projection(Subject::repeat(0xba));
    assert!(!refinement::accepts(stale_prepare_control));
    let mut foreign_prepare_control = projection;
    foreign_prepare_control
        .enter_view
        .retained_prepare_qc_after
        .evidence_class = CERTIFICATE_EVIDENCE_FOREIGN;
    assert!(!refinement::accepts(foreign_prepare_control));
    let mut reordered_fetch = projection;
    reordered_fetch.enter_view.following_fetch_index = reordered_fetch.enter_view.enter_index;
    assert!(!refinement::accepts(reordered_fetch));
    let mut foreign_timeout = projection;
    foreign_timeout.enter_view.pending_record_timeout.context_id =
        Reducer::context_identity_projection(ContextId::repeat(0xb4));
    assert!(!refinement::accepts(foreign_timeout));
    let mut future_local_lock = projection;
    future_local_lock.enter_view.local_lock_before.present = true;
    future_local_lock.enter_view.local_lock_before.context_id =
        Reducer::context_identity_projection(before.context.id());
    future_local_lock.enter_view.local_lock_before.height = before.context.height();
    future_local_lock.enter_view.local_lock_before.phase = 1;
    future_local_lock.enter_view.local_lock_before.view =
        before.current_tag().view().saturating_add(1);
    future_local_lock.enter_view.local_lock_before.subject =
        Reducer::subject_identity_projection(subject);
    assert!(!refinement::accepts(future_local_lock));
    let mut missing_control_state = after.clone();
    missing_control_state
        .outbound_control
        .remove(&OutboundControlClass::PrepareQc);
    let missing_control_projection =
        before.transition_projection(&event, &missing_control_state, outcome.effects());
    assert!(!refinement::accepts(missing_control_projection));
    assert!(!before.transition_refines(&event, &missing_control_state, outcome.effects()));
    let substitute = certificate(&before.context, 0, Phase::Prepare, subject, 0xbb);
    assert_eq!(substitute.reference(), high.reference());
    assert_ne!(substitute, high);
    let mut substituted_control_state = after;
    substituted_control_state.outbound_control.insert(
        OutboundControlClass::PrepareQc,
        ConsensusMessageV2::QuorumCertificate(substitute),
    );
    let substituted_control_projection =
        before.transition_projection(&event, &substituted_control_state, outcome.effects());
    assert_eq!(
        substituted_control_projection
            .enter_view
            .retained_prepare_qc_after
            .evidence_class,
        CERTIFICATE_EVIDENCE_FOREIGN
    );
    assert!(!refinement::accepts(substituted_control_projection));
    assert!(!before.transition_refines(&event, &substituted_control_state, outcome.effects()));
}
#[test]
fn enter_view_without_a_lock_carries_and_fetches_nothing() {
    let (before, event) = pending_timeout_install(None);
    let mut after = before.clone();
    let outcome = after
        .step_in_place(event.clone())
        .expect("materialize lock-free persisted-TC candidate");
    assert!(matches!(
        outcome.effects(),
        [Effect::EnterView {
            protected_lock: None,
            ..
        }]
    ));
    let projection = before.transition_projection(&event, &after, outcome.effects());
    assert!(refinement::accepts(projection));
    let mut nonzero_absent_context = projection;
    nonzero_absent_context
        .enter_view
        .effect_protected_lock
        .context_id
        .word0 = 1;
    assert!(!refinement::accepts(nonzero_absent_context));
    let mut nonzero_absent_subject = projection;
    nonzero_absent_subject
        .enter_view
        .effect_protected_lock
        .subject
        .word3 = 1;
    assert!(!refinement::accepts(nonzero_absent_subject));
    let mut invented = projection;
    invented.enter_view.effect_protected_lock.present = true;
    invented.enter_view.effect_protected_lock.context_id =
        Reducer::context_identity_projection(before.context.id());
    invented.enter_view.effect_protected_lock.height = before.context.height();
    invented.enter_view.effect_protected_lock.phase = 1;
    invented.enter_view.effect_protected_lock.subject =
        Reducer::subject_identity_projection(Subject::repeat(0xb5));
    assert!(!refinement::accepts(invented));
    let mut invented_prepare_control_state = after;
    invented_prepare_control_state.outbound_control.insert(
        OutboundControlClass::PrepareQc,
        ConsensusMessageV2::TimeoutCertificate(timeout_certificate(&before.context, 0, None)),
    );
    let invented_prepare_control_projection =
        before.transition_projection(&event, &invented_prepare_control_state, outcome.effects());
    assert!(
        invented_prepare_control_projection
            .enter_view
            .prepare_control_slot_present_after
    );
    assert!(
        !invented_prepare_control_projection
            .enter_view
            .retained_prepare_qc_after
            .present
    );
    assert!(!refinement::accepts(invented_prepare_control_projection));
    assert!(!before.transition_refines(&event, &invented_prepare_control_state, outcome.effects()));
}
#[test]
fn local_qc_formation_projects_four_votes_to_canonical_three() {
    let mut reducer = reducer();
    let round = Round::new(reducer.context.height(), 0);
    let subject = Subject::repeat(0xd1);
    let phase = Phase::Prepare;
    let pool = [4_u8, 2, 1, 3]
        .into_iter()
        .map(|signer| {
            let validator = ValidatorId::repeat(signer);
            (
                validator,
                SignedVote::new(
                    Vote::new(reducer.context.id(), round, phase, subject, validator),
                    OpaqueSignature::new(vec![signer; 8]),
                ),
            )
        })
        .collect();
    reducer.votes.insert((round, phase, round), pool);
    let certificate = reducer
        .try_form_certificate(round, round, phase, subject)
        .expect("four valid votes are sufficient")
        .expect("certificate forms");
    assert_eq!(
        certificate
            .signatures()
            .iter()
            .map(SignatureShare::signer)
            .collect::<Vec<_>>(),
        [1_u8, 2, 3]
            .map(ValidatorId::repeat)
            .into_iter()
            .collect::<Vec<_>>()
    );
    certificate
        .validate(&reducer.context)
        .expect("locally formed QC has exact cardinality");
}
#[test]
fn local_tc_formation_projects_four_votes_to_canonical_three() {
    let mut reducer = reducer();
    let round = Round::new(reducer.context.height(), 0);
    let highest = certificate(
        &reducer.context,
        0,
        Phase::Prepare,
        Subject::repeat(0xd2),
        0xd2,
    );
    let pool = [4_u8, 2, 1, 3]
        .into_iter()
        .map(|signer| {
            let validator = ValidatorId::repeat(signer);
            let highest = (signer >= 3).then(|| highest.clone());
            (
                validator,
                SignedTimeoutVote::new(
                    TimeoutVote::new(reducer.context.id(), round, validator, highest),
                    OpaqueSignature::new(vec![signer; 8]),
                ),
            )
        })
        .collect();
    reducer.timeout_votes.insert(round, pool);
    let certificate = reducer
        .try_form_timeout_certificate(round)
        .expect("four valid timeout votes are sufficient")
        .expect("timeout certificate forms");
    let signers = certificate
        .groups()
        .iter()
        .flat_map(TimeoutSignatureGroup::signatures)
        .map(SignatureShare::signer)
        .collect::<BTreeSet<_>>();
    assert_eq!(
        signers,
        [1_u8, 2, 3].map(ValidatorId::repeat).into_iter().collect()
    );
    certificate
        .validate(&reducer.context)
        .expect("locally formed TC has exact cardinality");
}
#[test]
fn enter_view_effect_cannot_substitute_an_equal_reference_certificate() {
    let fixture = reducer();
    let subject = Subject::repeat(0xb6);
    let high = certificate(&fixture.context, 0, Phase::Prepare, subject, 0xb7);
    let substitute = certificate(&fixture.context, 0, Phase::Prepare, subject, 0xb8);
    assert_eq!(high.reference(), substitute.reference());
    assert_ne!(high, substitute);
    let (before, event) = pending_timeout_install(Some(high));
    let mut after = before.clone();
    let outcome = after
        .step_in_place(event.clone())
        .expect("materialize persisted-TC candidate");
    let mut effects = outcome.into_effects();
    let Some(Effect::EnterView { protected_lock, .. }) = effects.first_mut() else {
        panic!("first install effect must enter the view")
    };
    *protected_lock = Some(substitute);
    let projection = before.transition_projection(&event, &after, &effects);
    assert_eq!(
        projection.enter_view.effect_protected_lock.evidence_class,
        CERTIFICATE_EVIDENCE_FOREIGN
    );
    assert_eq!(
        projection.enter_view.effect_protected_lock.signer_bitmap,
        projection.enter_view.durable_lock_after.signer_bitmap
    );
    assert_eq!(
        projection.enter_view.effect_protected_lock.signer_count,
        projection.enter_view.durable_lock_after.signer_count
    );
    assert_eq!(
        projection
            .enter_view
            .effect_protected_lock
            .signer_bitmap_count,
        projection.enter_view.durable_lock_after.signer_bitmap_count
    );
    assert_eq!(
        projection.enter_view.effect_protected_lock.voting_power,
        projection.enter_view.durable_lock_after.voting_power
    );
    assert!(!refinement::accepts(projection));
    assert!(!before.transition_refines(&event, &after, &effects));
}

#[test]
fn repeated_historical_prepare_cannot_occupy_the_current_prepare_slot() {
    let (mut live, acknowledgement) = pending_timeout_install(None);
    live.step(acknowledgement)
        .expect("enter view one through the production gate");
    let old = certificate(
        &live.context,
        0,
        Phase::Prepare,
        Subject::repeat(0xd1),
        0xd2,
    );
    let observed = live
        .step(Event::QuorumCertificateReceived {
            tag: live.current_tag(),
            certificate: old.clone(),
        })
        .expect("a newly learned historical high remains observable");
    let id = observed
        .effects()
        .iter()
        .find_map(|effect| match effect {
            Effect::Persist { entry, .. } => {
                assert!(
                    matches!(entry.record(), WalRecord::ObservePrepare(record) if record == &old)
                );
                Some(entry.id())
            }
            _ => None,
        })
        .expect("historical high must cross an exact WAL boundary");
    live.step(Event::Persisted {
        tag: live.current_tag(),
        id,
    })
    .expect("persist historical high without retaining current body work");
    assert!(live.pending_prepare.is_empty());
    assert_eq!(live.durable.highest_prepare(), Some(&old));

    // Authentication may return different signature evidence for the same
    // statement. Neither exact replay nor alternate evidence recreates work.
    let mut replay_changed_state = false;
    let mut replay_was_non_duplicate = false;
    for replay in [
        old.clone(),
        certificate(&live.context, 0, Phase::Prepare, old.subject(), 0xd3),
        old.clone(),
    ] {
        let before = live.clone();
        let outcome = live
            .step(Event::QuorumCertificateReceived {
                tag: live.current_tag(),
                certificate: replay,
            })
            .expect("old durable high retransmissions remain ordinary duplicates");
        replay_was_non_duplicate |=
            outcome.disposition() != StepDisposition::Ignored(IgnoreReason::Duplicate);
        assert!(outcome.effects().is_empty());
        replay_changed_state |= live != before;
    }
    let retransmitted = live
        .step(Event::RetransmitElapsed {
            tag: live.current_tag(),
        })
        .expect("historical evidence remains available for dissemination");
    assert!(retransmitted.effects().iter().any(|effect| matches!(
        effect,
        Effect::Broadcast(ConsensusMessageV2::QuorumCertificate(record)) if record == &old
    )));
    let obsolete_pending_count = live.pending_prepare.len();

    let current = certificate(
        &live.context,
        1,
        Phase::Prepare,
        Subject::repeat(0xd4),
        0xd5,
    );
    let outcome = live
        .step(Event::QuorumCertificateReceived {
            tag: live.current_tag(),
            certificate: current.clone(),
        })
        .expect("the next current QC keeps the verified one-pending-Prepare bound");
    assert!(!replay_was_non_duplicate);
    assert!(!replay_changed_state);
    assert_eq!(obsolete_pending_count, 0);
    assert_eq!(live.pending_prepare.len(), 1);
    assert_eq!(
        live.pending_prepare.get(&current.reference()),
        Some(&current)
    );
    assert!(outcome.effects().iter().any(|effect| matches!(
        effect, Effect::Persist { entry, .. }
            if matches!(entry.record(), WalRecord::ObservePrepare(record) if record == &current)
    )));
}

#[test]
fn historical_prepare_admission_keeps_new_highs_and_rejects_conflicts() {
    let mut live = reducer();
    let timeout = timeout_certificate(&live.context, 2, None);
    let staged = live
        .step(Event::TimeoutCertificateReceived {
            tag: live.current_tag(),
            certificate: timeout,
        })
        .expect("stage a quorum-authenticated jump to view three");
    let [Effect::Persist { entry, .. }] = staged.effects() else {
        panic!("timeout install must have exactly one persistence owner");
    };
    live.step(Event::Persisted {
        tag: live.current_tag(),
        id: entry.id(),
    })
    .expect("install view three");
    for view in 0..=1 {
        let high = certificate(
            &live.context,
            view,
            Phase::Prepare,
            Subject::repeat(0xe1),
            0xe2,
        );
        let staged = live
            .step(Event::QuorumCertificateReceived {
                tag: live.current_tag(),
                certificate: high.clone(),
            })
            .expect("each strictly newer historical high remains admissible");
        let [Effect::Persist { entry, .. }] = staged.effects() else {
            panic!("historical high must have exactly one persistence owner");
        };
        assert!(matches!(entry.record(), WalRecord::ObservePrepare(record) if record == &high));
        live.step(Event::Persisted {
            tag: live.current_tag(),
            id: entry.id(),
        })
        .expect("durably retain the newer historical high");
        assert_eq!(live.durable.highest_prepare(), Some(&high));
        assert!(live.pending_prepare.is_empty());
        assert!(live.body_work.is_empty());
    }
    let before = live.clone();
    let conflicting = certificate(
        &live.context,
        1,
        Phase::Prepare,
        Subject::repeat(0xe3),
        0xe4,
    );
    assert_eq!(
        live.step(Event::QuorumCertificateReceived {
            tag: live.current_tag(),
            certificate: conflicting,
        }),
        Err(ReducerError::ConflictingPrepareCertificates)
    );
    assert_eq!(live, before);
}

#[test]
fn refinement_failure_preserves_the_rejected_predicates_for_adapter_logging() {
    let mut invalid = reducer();
    // An impossible third pool is deliberately injected below the public
    // boundary. The public step must reject it without installing any change,
    // and the adapter must receive the exact failing invariant predicates.
    for view in 0..3 {
        invalid
            .timeout_votes
            .insert(Round::new(invalid.context.height(), view), BTreeMap::new());
    }
    let before = invalid.clone();
    let error = invalid
        .step(Event::RetransmitElapsed {
            tag: invalid.current_tag(),
        })
        .expect_err("the verified volatile bound remains fail-closed");
    let rendered = error.to_string();
    let ReducerError::RefinementViolation(failure) = error else {
        panic!("the failure must retain its refinement classification");
    };
    assert!(matches!(*failure, RefinementFailure::Transition { .. }));
    assert!(rendered.contains("volatile_before_well_formed: false"));
    assert!(rendered.contains("volatile_after_well_formed: false"));
    assert!(rendered.contains("timeout_vote_pools: 3"));
    assert!(rendered.contains("event_kind: 7"));
    assert_eq!(invalid, before);
}
