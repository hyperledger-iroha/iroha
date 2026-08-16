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
    let invented_prepare_control_projection = before.transition_projection(
        &event,
        &invented_prepare_control_state,
        outcome.effects(),
    );
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
    assert!(!before.transition_refines(
        &event,
        &invented_prepare_control_state,
        outcome.effects()
    ));
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
        [1_u8, 2, 3]
            .map(ValidatorId::repeat)
            .into_iter()
            .collect()
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
