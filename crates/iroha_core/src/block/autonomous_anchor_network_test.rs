#[test]
fn autonomous_anchor_admission_rejects_same_label_different_network() {
    let mut fixture = autonomous_anchor_fixture(None, 0);
    let display_name = fixture.state.chain_id.clone();
    let original_network_id = fixture.state.network_id;
    fixture.state.network_id = deterministic_test_network_id(0x7A);
    assert_eq!(fixture.state.chain_id, display_name);
    assert_ne!(fixture.state.network_id, original_network_id);
    let view = fixture.state.query_view();
    let error = ValidBlock::validate_execution_context_autonomous_lane_payloads(
        &fixture.block,
        &fixture.topology,
        &view,
        &fixture.bundle,
        fixture.profile.clone(),
    )
    .expect_err("the same display label must not authorize another genesis lineage");
    assert!(matches!(
        error,
        BlockValidationError::ExecutionContextInvalid(message)
            if message.contains("autonomous lane payload envelope")
    ));
}
