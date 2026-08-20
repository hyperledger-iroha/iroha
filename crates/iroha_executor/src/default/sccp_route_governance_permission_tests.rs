fn custom_parameter(name: &str) -> SetParameter {
    let id = iroha_smart_contract::data_model::parameter::CustomParameterId::new(
        name.parse().expect("test custom parameter id"),
    );
    SetParameter::new(Parameter::Custom(
        iroha_smart_contract::data_model::parameter::CustomParameter::new(id, Json::new(())),
    ))
}
fn remove_sccp_route() -> ApplySccpRouteGovernance {
    ApplySccpRouteGovernance::new(
        iroha_smart_contract::data_model::isi::bridge::SccpRouteGovernanceActionV1::Remove(
            iroha_smart_contract::data_model::bridge::SccpRouteKeyV1 {
                lane_id: iroha_smart_contract::data_model::bridge::SccpLaneIdV1 {
                    source:
                        iroha_smart_contract::data_model::bridge::SccpNetworkV1::EthereumMainnet,
                    target: iroha_smart_contract::data_model::bridge::SccpNetworkV1::SoraTaira,
                },
                route_id: "taira_eth_xor".to_owned(),
                asset_key: "xor".to_owned(),
                revision: 1,
            },
        ),
    )
}
#[test]
fn direct_sccp_route_governance_dispatch_is_retired_for_every_permission() {
    let instruction = remove_sccp_route();
    assert_denied_without_permission(
        instruction.clone(),
        bridge::visit_apply_sccp_route_governance,
    );
    assert_denied_with_permission(
        instruction.clone(),
        PermissionObject::from(CanSetParameters),
        bridge::visit_apply_sccp_route_governance,
    );
    assert_denied_with_permission(
        instruction.clone(),
        PermissionObject::from(CanManageSccpGovernance),
        bridge::visit_apply_sccp_route_governance,
    );
    with_mock_permissions(
        vec![PermissionObject::from(CanManageSccpGovernance)],
        || {
            let mut executor = MockExecutor::new(false);
            visit_instruction(&mut executor, &InstructionBox::from(instruction));
            assert!(
                executor.verdict().is_err(),
                "direct SCCP governance must remain closed even for legacy managers"
            );
        },
    );
}
