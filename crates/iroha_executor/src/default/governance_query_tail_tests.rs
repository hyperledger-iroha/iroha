#[test]
fn pop_registry_transparency_queries_are_public() {
    assert_allowed_without_permission(
        FindSorafsPopIssuerPolicy,
        sorafs::visit_find_sorafs_pop_issuer_policy,
    );
    assert_allowed_without_permission(
        FindSorafsPopCredentialCommitmentByDigest::new([1; 32]),
        sorafs::visit_find_sorafs_pop_credential_commitment_by_digest,
    );
    assert_allowed_without_permission(
        FindSorafsPopCommitmentRootByVersion::new(1),
        sorafs::visit_find_sorafs_pop_commitment_root_by_version,
    );
    assert_allowed_without_permission(
        FindSorafsPopRevocationPublicationByVersion::new(1),
        sorafs::visit_find_sorafs_pop_revocation_publication_by_version,
    );
    assert_allowed_without_permission(
        FindSorafsPopRevocationByNonceCommitment::new([2; 32]),
        sorafs::visit_find_sorafs_pop_revocation_by_nonce_commitment,
    );
    assert_allowed_without_permission(
        FindSorafsPopAuditDigestBySequence::new(1),
        sorafs::visit_find_sorafs_pop_audit_digest_by_sequence,
    );
    assert_allowed_without_permission(
        FindSorafsPopRegistryStatus,
        sorafs::visit_find_sorafs_pop_registry_status,
    );
}
macro_rules! orderbook_query_permission_case {
    ($name:ident, $query:expr, $visitor:path) => {
        #[test]
        fn $name() {
            let query = $query;
            assert_denied_without_permission(query.clone(), $visitor);
            assert_allowed_with_permission(
                query.clone(),
                PermissionObject::from(CanSetSorafsPricing),
                $visitor,
            );
            assert_allowed_with_permission(
                query,
                PermissionObject::from(CanCompleteSorafsReplicationOrder),
                $visitor,
            );
        }
    };
}
orderbook_query_permission_case!(
    orderbook_policy_query_requires_operator_permission,
    FindSorafsOrderbookPolicy,
    sorafs::visit_find_sorafs_orderbook_policy
);
orderbook_query_permission_case!(
    orderbook_order_query_requires_operator_permission,
    FindSorafsOrderbookOrderById::new([0x11; 32]),
    sorafs::visit_find_sorafs_orderbook_order_by_id
);
orderbook_query_permission_case!(
    orderbook_cancellation_query_requires_operator_permission,
    FindSorafsOrderbookCancellationByOrderId::new([0x12; 32]),
    sorafs::visit_find_sorafs_orderbook_cancellation_by_order_id
);
orderbook_query_permission_case!(
    orderbook_receipt_query_requires_operator_permission,
    FindSorafsOrderbookReceiptById::new([0x13; 32]),
    sorafs::visit_find_sorafs_orderbook_receipt_by_id
);
orderbook_query_permission_case!(
    orderbook_trade_query_requires_operator_permission,
    FindSorafsOrderbookTradeById::new([0x14; 32]),
    sorafs::visit_find_sorafs_orderbook_trade_by_id
);
orderbook_query_permission_case!(
    orderbook_channel_query_requires_operator_permission,
    FindSorafsOrderbookChannelById::new([0x15; 32]),
    sorafs::visit_find_sorafs_orderbook_channel_by_id
);
orderbook_query_permission_case!(
    orderbook_status_query_requires_operator_permission,
    FindSorafsOrderbookStatus,
    sorafs::visit_find_sorafs_orderbook_status
);
orderbook_query_permission_case!(
    orderbook_order_page_query_requires_operator_permission,
    FindSorafsOrderbookOrders::new(None, None, None, 10),
    sorafs::visit_find_sorafs_orderbook_orders
);
orderbook_query_permission_case!(
    orderbook_receipt_page_query_requires_operator_permission,
    FindSorafsOrderbookReceipts::new(None, None, None, 10),
    sorafs::visit_find_sorafs_orderbook_receipts
);
orderbook_query_permission_case!(
    orderbook_trade_page_query_requires_operator_permission,
    FindSorafsOrderbookTrades::new(None, None, 10),
    sorafs::visit_find_sorafs_orderbook_trades
);
orderbook_query_permission_case!(
    orderbook_channel_page_query_requires_operator_permission,
    FindSorafsOrderbookChannels::new(None, None, None, 10),
    sorafs::visit_find_sorafs_orderbook_channels
);
orderbook_query_permission_case!(
    orderbook_event_page_query_requires_operator_permission,
    FindSorafsOrderbookEvents::new(None, None, 10),
    sorafs::visit_find_sorafs_orderbook_events
);
#[test]
fn reserve_event_page_query_requires_governance_permission() {
    let query = FindSorafsReserveEvents::new(
        Some(ReserveFinalizedCursorV1 {
            height: 2,
            block_hash: [0x72; 32],
        }),
        None,
        10,
    );
    assert_denied_without_permission(query, sorafs::visit_find_sorafs_reserve_events);
    assert_allowed_with_permission(
        query,
        PermissionObject::from(CanSetSorafsReservePolicy),
        sorafs::visit_find_sorafs_reserve_events,
    );
}
#[test]
fn reserve_point_queries_require_governance_permission() {
    let permission = PermissionObject::from(CanSetSorafsReservePolicy);
    let policy = FindSorafsReservePolicy::new();
    assert_denied_without_permission(policy, sorafs::visit_find_sorafs_reserve_policy);
    assert_allowed_with_permission(
        policy,
        permission.clone(),
        sorafs::visit_find_sorafs_reserve_policy,
    );
    let provider = FindSorafsReserveProviderById::new(ProviderId::new([0x51; 32]));
    assert_denied_without_permission(provider, sorafs::visit_find_sorafs_reserve_provider_by_id);
    assert_allowed_with_permission(
        provider,
        permission.clone(),
        sorafs::visit_find_sorafs_reserve_provider_by_id,
    );
    let movement = FindSorafsReserveMovementById::new([0x61; 32]);
    assert_denied_without_permission(movement, sorafs::visit_find_sorafs_reserve_movement_by_id);
    assert_allowed_with_permission(
        movement,
        permission.clone(),
        sorafs::visit_find_sorafs_reserve_movement_by_id,
    );
    let appeal = FindSorafsReserveAppealById::new([0x71; 32]);
    assert_denied_without_permission(appeal, sorafs::visit_find_sorafs_reserve_appeal_by_id);
    assert_allowed_with_permission(
        appeal,
        permission,
        sorafs::visit_find_sorafs_reserve_appeal_by_id,
    );
}
#[test]
fn reserve_record_page_queries_require_governance_permission() {
    let permission = PermissionObject::from(CanSetSorafsReservePolicy);
    let providers = FindSorafsReserveProviders::new(None, None, 10);
    assert_denied_without_permission(providers, sorafs::visit_find_sorafs_reserve_providers);
    assert_allowed_with_permission(
        providers,
        permission.clone(),
        sorafs::visit_find_sorafs_reserve_providers,
    );
    let movements = FindSorafsReserveMovements::new(None, None, 10);
    assert_denied_without_permission(movements, sorafs::visit_find_sorafs_reserve_movements);
    assert_allowed_with_permission(
        movements,
        permission.clone(),
        sorafs::visit_find_sorafs_reserve_movements,
    );
    let appeals = FindSorafsReserveAppeals::new(None, None, 10);
    assert_denied_without_permission(appeals, sorafs::visit_find_sorafs_reserve_appeals);
    assert_allowed_with_permission(
        appeals,
        permission,
        sorafs::visit_find_sorafs_reserve_appeals,
    );
}
#[test]
fn sccp_and_generic_parameter_permissions_are_separated() {
    let sccp = custom_parameter("sccp_registry_v1");
    assert_denied_with_permission(
        sccp.clone(),
        PermissionObject::from(CanSetParameters),
        parameter::visit_set_parameter,
    );
    assert_denied_with_permission(
        sccp.clone(),
        PermissionObject::from(CanManageSccpGovernance),
        parameter::visit_set_parameter,
    );
    let mut genesis = MockExecutor::new(true);
    parameter::visit_set_parameter(&mut genesis, &sccp);
    assert!(genesis.verdict().is_err());
    let unrelated = custom_parameter("unrelated_parameter");
    assert_denied_with_permission(
        unrelated.clone(),
        PermissionObject::from(CanManageSccpGovernance),
        parameter::visit_set_parameter,
    );
    assert_allowed_with_permission(
        unrelated,
        PermissionObject::from(CanSetParameters),
        parameter::visit_set_parameter,
    );
}
#[test]
fn validation_fee_parameters_are_reserved_from_generic_set_parameter() {
    for id in [
        iroha_data_model::validation_fee::RETIRED_VALIDATION_FEE_GOVERNANCE_KEYSET_PARAMETER_ID,
        iroha_data_model::validation_fee::ValidationFeePolicyRegistryV1::PARAMETER_ID_STR,
        iroha_data_model::validation_fee::RETIRED_VALIDATION_FEE_POLICY_PARAMETER_ID,
    ] {
        let instruction = custom_parameter(id);
        assert_denied_with_permission(
            instruction.clone(),
            PermissionObject::from(CanSetParameters),
            parameter::visit_set_parameter,
        );
        let mut genesis = MockExecutor::new(true);
        parameter::visit_set_parameter(&mut genesis, &instruction);
        assert!(
            genesis.verdict().is_err(),
            "{id} must remain reserved during genesis"
        );
    }
}
#[test]
fn hijiri_parameters_require_the_exact_dedicated_permission() {
    let account_risk_id = format!(
        "{}{}",
        iroha_data_model::hijiri::HIJIRI_ACCOUNT_RISK_PARAMETER_PREFIX_V1,
        "00".repeat(32)
    );
    for id in [
        iroha_data_model::hijiri::HijiriParametersV1::PARAMETER_ID_STR,
        account_risk_id.as_str(),
    ] {
        let instruction = custom_parameter(id);
        assert_denied_with_permission(
            instruction.clone(),
            PermissionObject::from(CanSetParameters),
            parameter::visit_set_parameter,
        );
        assert_allowed_with_permission(
            instruction.clone(),
            PermissionObject::from(CanSetHijiriParameters),
            parameter::visit_set_parameter,
        );
        let mut genesis = MockExecutor::new(true);
        parameter::visit_set_parameter(&mut genesis, &instruction);
        assert!(
            genesis.verdict().is_ok(),
            "{id} must remain installable during genesis"
        );
    }
}
#[test]
fn raw_domain_registration_is_genesis_only() {
    let domain_id = DomainId::try_new("planned", "universal").expect("valid domain id");
    let instruction = Register::domain(Domain::new(domain_id));
    assert_denied_with_permission(
        instruction.clone(),
        PermissionObject::from(CanRegisterDomain),
        domain::visit_register_domain,
    );
    let mut genesis = MockExecutor::new(true);
    domain::visit_register_domain(&mut genesis, &instruction);
    assert!(genesis.verdict().is_ok());
}
#[test]
fn genesis_cannot_bypass_typed_sccp_certificate_governance() {
    let mut executor = MockExecutor::new(true);
    bridge::visit_apply_sccp_route_governance(&mut executor, &remove_sccp_route());
    assert!(executor.verdict().is_err());
}
