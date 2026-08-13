#[test]
fn primitive_projection_cannot_hide_a_safety_violation() {
    let before = reducer();
    let after = before.clone();
    let event = Event::RetransmitElapsed {
        tag: before.current_tag(),
    };
    let mut projection = before.transition_projection(&event, &after, &[]);
    assert!(refinement::accepts(projection));
    projection.safety_before.invalid_pending_append = 1;
    assert!(!refinement::accepts(projection));
}
#[test]
fn enter_view_accepts_incoming_equal_projection_with_distinct_full_evidence() {
    let fixture = reducer();
    let context = fixture.context.clone();
    let local = ValidatorId::repeat(1);
    let subject = Subject::repeat(0xbc);
    let round = Round::new(context.height(), 0);
    let local_prepare = certificate(&context, 0, Phase::Prepare, subject, 0xbd);
    let incoming_prepare = certificate(&context, 0, Phase::Prepare, subject, 0xbe);
    assert_eq!(local_prepare.reference(), incoming_prepare.reference());
    assert_ne!(local_prepare, incoming_prepare);
    let commit = Vote::new(context.id(), round, Phase::Commit, subject, local);
    let initial_timeout = timeout_certificate(&context, 0, None);
    let mut pending = Reducer::recover(
        context.clone(),
        Some(local),
        Generation::new(7),
        [
            WalEntry::new(
                PersistenceId::new(1),
                WalRecord::LockAndCommit {
                    prepare: local_prepare.clone(),
                    vote: commit,
                },
            ),
            WalEntry::new(
                PersistenceId::new(2),
                WalRecord::InstallTimeout(initial_timeout),
            ),
        ],
    )
    .expect("recover a local-evidence lock in view one");
    let resume = pending
        .step(Event::ResumeAfterReplay {
            tag: pending.current_tag(),
        })
        .expect("resume the exact durable Commit intent");
    assert!(matches!(
        resume.effects(),
        [Effect::Sign {
            message: SignableMessage::Vote(vote),
            ..
        }] if *vote == commit
    ));
    pending
        .step(Event::Signed {
            tag: pending.current_tag(),
            signature: OpaqueSignature::new(vec![0xbf; 8]),
        })
        .expect("complete the recovered Commit signature");
    let incoming_timeout = timeout_certificate(&context, 1, Some(incoming_prepare.clone()));
    let begin = pending
        .step(Event::TimeoutCertificateReceived {
            tag: pending.current_tag(),
            certificate: incoming_timeout,
        })
        .expect("begin the distinct-evidence timeout install");
    let id = match begin.effects() {
        [Effect::Persist { entry, .. }] => entry.id(),
        effects => panic!("expected one timeout persistence effect, got {effects:?}"),
    };
    let event = Event::Persisted {
        tag: pending.current_tag(),
        id,
    };
    let mut after = pending.clone();
    let outcome = after
        .step_in_place(event.clone())
        .expect("materialize the persisted timeout candidate");
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
            },
            Effect::Sign {
                message: SignableMessage::Vote(vote),
                ..
            }
        ] if protected == &local_prepare && fetched == &local_prepare && *vote == commit
    ));
    let projection = pending.transition_projection(&event, &after, outcome.effects());
    for incoming in [
        projection.enter_view.pending_record_timeout.highest_prepare,
        projection
            .enter_view
            .pending_continuation_timeout
            .highest_prepare,
        projection.enter_view.durable_timeout_after.highest_prepare,
        projection.enter_view.effect_timeout.highest_prepare,
        projection.enter_view.incoming_highest_for_control,
    ] {
        assert_eq!(incoming.signer_bitmap, 0b111);
        assert_eq!(incoming.signer_bitmap_count, 3);
        assert_eq!(incoming.signer_count, 3);
        assert_eq!(incoming.voting_power, 3);
        assert_eq!(incoming.evidence_class, CERTIFICATE_EVIDENCE_INCOMING);
    }
    for local_projection in [
        projection.enter_view.local_lock_before,
        projection.enter_view.local_highest_before,
        projection.enter_view.durable_lock_after,
        projection.enter_view.durable_highest_after,
        projection.enter_view.retained_prepare_qc_after,
        projection.enter_view.effect_protected_lock,
        projection.enter_view.following_fetch_lock,
    ] {
        assert_eq!(local_projection.signer_bitmap, 0b111);
        assert_eq!(local_projection.signer_bitmap_count, 3);
        assert_eq!(local_projection.signer_count, 3);
        assert_eq!(local_projection.voting_power, 3);
        assert_eq!(local_projection.evidence_class, CERTIFICATE_EVIDENCE_LOCAL);
    }
    assert!(projection.enter_view.prepare_control_slot_present_after);
    assert!(
        refinement::accepts(projection),
        "a valid incoming QC may share the fixed-width signer projection without being the exact local evidence"
    );
    let mut false_local_claim = projection;
    false_local_claim
        .enter_view
        .incoming_highest_for_control
        .evidence_class = CERTIFICATE_EVIDENCE_LOCAL;
    for local_projection in [
        &mut false_local_claim.enter_view.local_highest_before,
        &mut false_local_claim.enter_view.durable_highest_after,
        &mut false_local_claim.enter_view.retained_prepare_qc_after,
    ] {
        local_projection.signer_bitmap = 0b1011;
    }
    assert!(!refinement::accepts(false_local_claim));
    let mut foreign_incoming = projection;
    foreign_incoming
        .enter_view
        .incoming_highest_for_control
        .evidence_class = CERTIFICATE_EVIDENCE_FOREIGN;
    assert!(!refinement::accepts(foreign_incoming));
    let mut production = pending;
    let applied = production
        .step(event)
        .expect("the checked reducer must commit the accepted candidate");
    assert_eq!(applied.effects(), outcome.effects());
    assert_eq!(production, after);
}
