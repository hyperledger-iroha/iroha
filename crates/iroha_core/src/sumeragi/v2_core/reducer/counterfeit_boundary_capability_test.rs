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
