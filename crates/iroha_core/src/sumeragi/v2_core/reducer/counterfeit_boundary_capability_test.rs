#[test]
fn counterfeit_boundary_capability_cannot_invent_a_wal_transition() {
    let before = reducer();
    let after = before.clone();
    let event = Event::RetransmitElapsed {
        tag: before.current_tag(),
    };
    let mut projection = before.transition_projection(&event, &after, &[]);
    let counterfeit = BoundaryCapabilityKey {
        kind: refinement::BOUNDARY_BEGIN_WAL,
        record_kind: WAL_RECORD_PREPARE_INTENT,
        continuation: CONTINUATION_SIGN,
        persistence_id: 1,
        context_id: before.context.id(),
        tag: Reducer::tag_projection(before.current_tag()),
        ..BoundaryCapabilityKey::none()
    };
    projection.boundary_claimed = counterfeit;
    projection.boundary_granted = counterfeit;
    assert!(!refinement::accepts(projection));
}
#[test]
fn local_ready_apply_capability_requires_the_exact_manifest() {
    let subject = Subject::repeat(0xaa);
    let manifest =
        PayloadManifest::new(subject, Digest::repeat(0xab), Digest::repeat(0xac), 256, 4);
    let conflicting =
        PayloadManifest::new(subject, Digest::repeat(0xad), Digest::repeat(0xae), 256, 4);
    let (mut before, decision) = decided_reducer(subject);
    before.body_work.insert(
        (decision.round(), subject),
        BodyWork {
            manifest: None,
            state: BodyState::Missing,
        },
    );
    let mut after = before.clone();
    after.body_work.insert(
        (decision.round(), subject),
        BodyWork {
            manifest: Some(manifest),
            state: BodyState::Validated,
        },
    );
    let apply = Effect::Apply {
        tag: after.current_tag(),
        subject,
        certificate: decision,
    };
    let exact = Event::LocalProposalReady {
        tag: before.current_tag(),
        manifest,
    };
    let counterfeit = Event::LocalProposalReady {
        tag: before.current_tag(),
        manifest: conflicting,
    };
    assert!(before.transition_refines(&exact, &after, std::slice::from_ref(&apply)));
    assert!(!before.transition_refines(&counterfeit, &after, &[apply]));
}

#[test]
fn observer_certificate_evidence_cannot_mint_productive_broadcast_capability() {
    let context = reducer().context.clone();
    let prepare = certificate(&context, 0, Phase::Prepare, Subject::repeat(0xeb), 0xec);
    let decision = certificate(&context, 0, Phase::Commit, prepare.subject(), 0xed);
    let timeout = timeout_certificate(&context, 0, None);
    let cases = [
        (
            WalRecord::ObservePrepare(prepare.clone()),
            ConsensusMessageV2::QuorumCertificate(prepare),
        ),
        (
            WalRecord::Decision(decision.clone()),
            ConsensusMessageV2::QuorumCertificate(decision),
        ),
        (
            WalRecord::InstallTimeout(timeout.clone()),
            ConsensusMessageV2::TimeoutCertificate(timeout),
        ),
    ];
    for (record, message) in cases {
        for local in [None, Some(ValidatorId::repeat(1))] {
            let mut owner = Reducer::recover(
                context.clone(),
                local,
                Generation::new(19),
                [WalEntry::new(PersistenceId::new(1), record.clone())],
            )
            .expect("same certificate evidence is valid for both roles");
            owner
                .step(Event::ResumeAfterReplay {
                    tag: owner.current_tag(),
                })
                .expect("resume the role's exact recovery work");
            assert!(
                owner
                    .retained_control_messages()
                    .any(|retained| retained == &message)
            );
            let event = Event::RetransmitElapsed {
                tag: owner.current_tag(),
            };
            let effect = Effect::Broadcast(message.clone());
            let granted = owner.granted_effect_capability(&event, &owner, &effect);
            if local.is_some() {
                let requested = Reducer::effect_capability(&effect);
                assert_ne!(requested, EffectCapabilityKey::none());
                assert_eq!(
                    granted, requested,
                    "a frozen roster-local owner must grant the exact certificate broadcast key"
                );
            } else {
                assert_eq!(
                    granted,
                    EffectCapabilityKey::none(),
                    "authenticated evidence cannot grant an observer productive authority"
                );
                assert!(
                    !owner.transition_refines(&event, &owner, &[effect]),
                    "a counterfeit observer broadcast must fail the production effect gate"
                );
            }
        }
    }
}
