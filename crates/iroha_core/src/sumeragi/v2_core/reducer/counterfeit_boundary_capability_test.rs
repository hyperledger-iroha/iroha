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
