// The producer-authenticated source is indivisible after its control-only anchor finalizes.
// Exercise actual carrier admission with valid signatures and canonical reservation bindings.
#[test]
fn autonomous_anchor_gas_budget_enforces_complete_source_before_anchoring() {
    let limit = 2_000_000_u64;
    for (gas_limits, expected_error) in [
        (vec![limit], None),
        (vec![limit / 2, limit / 2], None),
        (
            vec![limit, limit],
            Some("exceeds its origin block proposal gas budget"),
        ),
        (vec![0], Some("invalid proposal gas accounting")),
        (vec![u64::MAX, 1], Some("proposal gas overflows u64")),
    ] {
        let fixture = autonomous_anchor_fixture_with_gas_limits(None, 0, Some(&gas_limits));
        {
            // The origin carrier must use its committed parent policy, not a default cap
            // or the future merge carrier's policy. This is the same static path on replay.
            let mut parameters = fixture.state.world.parameters.block();
            let id = iroha_data_model::parameter::CustomParameterId::new(
                "ivm_gas_limit_per_block"
                    .parse()
                    .expect("gas parameter name"),
            );
            parameters.set_parameter(iroha_data_model::parameter::Parameter::Custom(
                iroha_data_model::parameter::CustomParameter::new(
                    id,
                    iroha_primitives::json::Json::new(limit),
                ),
            ));
            parameters.commit();
        }
        let view = fixture.state.query_view();
        let result = ValidBlock::validate_execution_context_with_state(
            &fixture.block,
            &fixture.topology,
            &view,
            fixture.profile.clone(),
        );
        match expected_error {
            None => result.expect("a complete source at the origin block cap must anchor"),
            Some(expected) => {
                let error = result.expect_err("unmergeable sources must not acquire an anchor");
                assert!(
                    matches!(&error, BlockValidationError::ExecutionContextInvalid(message)
                        if message.contains(expected)),
                    "unexpected admission error for {gas_limits:?}: {error:?}",
                );
            }
        }
    }
}
