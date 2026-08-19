#[test]
fn account_alias_dataspace_permission_grant_routes_by_scope() {
    let (submitter_id, submitter_keypair) = gen_account_in("wonderland");
    let (holder_id, _) = gen_account_in("wonderland");
    let dataspace_id = DataSpaceId::new(10);
    let lane_id = LaneId::new(3);
    let catalog = dataspace_catalog(&[(dataspace_id, "paynet")]);
    let lane_catalog = catalog_with_lane_dataspaces(&[
        (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
        (lane_id, dataspace_id),
    ]);
    let router = ConfigLaneRouter::new(default_routing_policy(), catalog, lane_catalog);
    let permission = Permission::from(CanManageAccountAlias {
        scope: AccountAliasPermissionScope::Dataspace(dataspace_id),
    });
    let tx = sample_transaction(
        &submitter_id,
        submitter_keypair.private_key(),
        vec![InstructionBox::from(Grant::account_permission(
            permission, holder_id,
        ))],
    );
    assert_eq!(
        router
            .try_route_without_state(&tx)
            .expect("dataspace alias permission should route without world state"),
        Some(RoutingDecision::new(lane_id, dataspace_id))
    );
}
#[test]
fn account_alias_domain_permission_grant_routes_by_scope() {
    let (submitter_id, submitter_keypair) = gen_account_in("wonderland");
    let (holder_id, _) = gen_account_in("wonderland");
    let dataspace_id = DataSpaceId::new(10);
    let lane_id = LaneId::new(3);
    let catalog = dataspace_catalog(&[(dataspace_id, "paynet")]);
    let lane_catalog = catalog_with_lane_dataspaces(&[
        (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
        (lane_id, dataspace_id),
    ]);
    let router = ConfigLaneRouter::new(default_routing_policy(), catalog, lane_catalog);
    let permission = Permission::from(CanResolveAccountAlias {
        scope: AccountAliasPermissionScope::Domain(
            DomainId::try_new("mibank", "paynet").expect("domain id"),
        ),
    });
    let tx = sample_transaction(
        &submitter_id,
        submitter_keypair.private_key(),
        vec![InstructionBox::from(Grant::account_permission(
            permission, holder_id,
        ))],
    );
    assert_eq!(
        router
            .try_route_without_state(&tx)
            .expect("domain alias permission state requirement should be deterministic"),
        None
    );
    let state = blank_state();
    install_router_nexus(&state, &router);
    assert_eq!(
        router
            .try_route_with_state(&tx, &state)
            .expect("domain alias permission should resolve with live state"),
        RoutingDecision::new(lane_id, dataspace_id)
    );
}
#[test]
fn account_alias_resolution_delegation_routes_by_exact_scope() {
    let (submitter_id, submitter_keypair) = gen_account_in("wonderland");
    let (holder_id, _) = gen_account_in("wonderland");
    let dataspace_id = DataSpaceId::new(10);
    let lane_id = LaneId::new(3);
    let catalog = dataspace_catalog(&[(dataspace_id, "paynet")]);
    let lane_catalog = catalog_with_lane_dataspaces(&[
        (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
        (lane_id, dataspace_id),
    ]);
    let router = ConfigLaneRouter::new(default_routing_policy(), catalog, lane_catalog);
    let permission = Permission::from(CanDelegateAccountAliasResolution {
        scope: AccountAliasPermissionScope::Domain(
            DomainId::try_new("mibank", "paynet").expect("domain id"),
        ),
    });
    let tx = sample_transaction(
        &submitter_id,
        submitter_keypair.private_key(),
        vec![InstructionBox::from(Grant::account_permission(
            permission, holder_id,
        ))],
    );
    assert_eq!(
        router
            .try_route_without_state(&tx)
            .expect("alias-resolution delegation state requirement should be deterministic"),
        None
    );
    let state = blank_state();
    install_router_nexus(&state, &router);
    assert_eq!(
        router
            .try_route_with_state(&tx, &state)
            .expect("alias-resolution delegation should resolve by its exact scope"),
        RoutingDecision::new(lane_id, dataspace_id)
    );
}
#[test]
fn asset_definition_alias_domain_permission_defers_to_sns_state() {
    let (submitter_id, submitter_keypair) = gen_account_in("wonderland");
    let (holder_id, _) = gen_account_in("wonderland");
    let dataspace_id = DataSpaceId::new(10);
    let lane_id = LaneId::new(3);
    let catalog = dataspace_catalog(&[(dataspace_id, "paynet")]);
    let lane_catalog = catalog_with_lane_dataspaces(&[
        (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
        (lane_id, dataspace_id),
    ]);
    let router = ConfigLaneRouter::new(default_routing_policy(), catalog, lane_catalog);
    let permission = Permission::from(CanManageAssetDefinitionAlias {
        scope: AssetDefinitionAliasPermissionScope::Domain(
            DomainId::try_new("assets", "paynet").expect("domain id"),
        ),
    });
    let tx = sample_transaction(
        &submitter_id,
        submitter_keypair.private_key(),
        vec![InstructionBox::from(Grant::account_permission(
            permission, holder_id,
        ))],
    );

    assert_eq!(router.try_route_without_state(&tx), Ok(None));
    let state = blank_state();
    install_router_nexus(&state, &router);
    assert_eq!(
        router
            .try_route_with_state(&tx, &state)
            .expect("asset-definition alias domain should resolve with live state"),
        RoutingDecision::new(lane_id, dataspace_id)
    );
}
#[test]
fn account_scope_directory_scope_matches_destination_account_permission_route() {
    let (submitter_id, submitter_keypair) = gen_account_in("wonderland");
    let (holder_id, _) = gen_account_in("wonderland");
    let dataspace_id = DataSpaceId::new(10);
    let catalog = dataspace_catalog(&[(dataspace_id, "paynet")]);
    let policy = LaneRoutingPolicy {
        default_lane: LaneId::SINGLE,
        default_dataspace: DataSpaceId::UNIVERSAL,
        rules: vec![LaneRoutingRule {
            lane: LaneId::new(1),
            dataspace: Some(dataspace_id),
            matcher: LaneRoutingMatcher {
                account: Some("*@hbl.paynet".to_string()),
                instruction: None,
                description: None,
            },
        }],
    };
    let lane_catalog = catalog_with_lane_dataspaces(&[
        (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
        (LaneId::new(1), dataspace_id),
    ]);
    let router = ConfigLaneRouter::new(policy, catalog.clone(), lane_catalog);
    let permission = Permission::from(CanManageAccountAlias {
        scope: AccountAliasPermissionScope::Domain(
            DomainId::try_new("hbl", "paynet").expect("domain id"),
        ),
    });
    let tx = sample_transaction(
        &submitter_id,
        submitter_keypair.private_key(),
        vec![InstructionBox::from(Grant::account_permission(
            permission,
            holder_id.clone(),
        ))],
    );
    let mut scope_entry = crate::nexus::space_directory::AccountScopeDirectoryEntry::default();
    scope_entry.ensure_dataspace(dataspace_id);
    scope_entry.bind_domain(
        dataspace_id,
        AccountAliasDomain::from("hbl".parse::<Name>().expect("domain label")),
    );
    let state = state_with_account_scope_entries(&[(holder_id.clone(), scope_entry)], catalog);
    state.nexus.write().lane_catalog = router.lane_catalog.as_ref().clone();
    let state_view = state.view();
    assert_eq!(
        state_view
            .world()
            .account_scope_hierarchy(&holder_id)
            .expect("scope hierarchy"),
        BTreeMap::from([(
            dataspace_id,
            BTreeSet::from([DomainId::try_new("hbl", "paynet").expect("domain id")]),
        )])
    );
    assert!(account_matches_alias_scope(
        "hbl.paynet",
        &holder_id,
        &state_view
    ));
    assert_eq!(
        router
            .try_route_with_view(&tx, &state_view)
            .expect("multisig scope routing should resolve"),
        RoutingDecision::new(LaneId::new(1), dataspace_id)
    );
}
#[test]
fn world_validation_routes_account_permission_holder_by_scope() {
    let (submitter_id, submitter_keypair) = gen_account_in("wonderland");
    let (holder_id, _) = gen_account_in("wonderland");
    let dataspace_id = DataSpaceId::new(10);
    let lane_id = LaneId::new(3);
    let catalog = dataspace_catalog(&[(dataspace_id, "paynet")]);
    let policy = LaneRoutingPolicy {
        default_lane: LaneId::SINGLE,
        default_dataspace: DataSpaceId::UNIVERSAL,
        rules: vec![LaneRoutingRule {
            lane: lane_id,
            dataspace: Some(dataspace_id),
            matcher: LaneRoutingMatcher {
                account: Some("*@paynet".to_string()),
                instruction: None,
                description: None,
            },
        }],
    };
    let lane_catalog = catalog_with_lane_dataspaces(&[
        (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
        (lane_id, dataspace_id),
    ]);
    let router = ConfigLaneRouter::new(policy.clone(), catalog.clone(), lane_catalog.clone());
    let permission = Permission::from(CanRegisterTrigger {
        authority: holder_id.clone(),
    });
    let tx = sample_transaction(
        &submitter_id,
        submitter_keypair.private_key(),
        vec![InstructionBox::from(Grant::account_permission(
            permission,
            holder_id.clone(),
        ))],
    );
    let mut scope_entry = crate::nexus::space_directory::AccountScopeDirectoryEntry::default();
    scope_entry.ensure_dataspace(dataspace_id);
    let state = state_with_account_scope_entries(&[(holder_id, scope_entry)], catalog);
    state.nexus.write().lane_catalog = lane_catalog.clone();
    let state_view = state.view();
    let expected = RoutingDecision::new(lane_id, dataspace_id);
    assert_eq!(
        router
            .try_route_with_view(&tx, &state_view)
            .expect("state-view routing should use account scope"),
        expected
    );
    assert_eq!(
        evaluate_policy_with_catalog_and_world(
            &policy,
            &lane_catalog,
            &state_view.nexus().dataspace_catalog,
            &tx,
            state_view.world(),
        )
        .expect("validation routing should use account scope"),
        expected
    );
}
#[test]
fn state_view_routing_uses_committed_nexus_policy_not_cached_router_policy() {
    let (submitter_id, submitter_keypair) = gen_account_in("wonderland");
    let (holder_id, _) = gen_account_in("wonderland");
    let dataspace_id = DataSpaceId::new(10);
    let lane_id = LaneId::new(3);
    let catalog = dataspace_catalog(&[(dataspace_id, "paynet")]);
    let committed_policy = LaneRoutingPolicy {
        default_lane: LaneId::SINGLE,
        default_dataspace: DataSpaceId::UNIVERSAL,
        rules: vec![LaneRoutingRule {
            lane: lane_id,
            dataspace: Some(dataspace_id),
            matcher: LaneRoutingMatcher {
                account: Some("*@paynet".to_string()),
                instruction: None,
                description: None,
            },
        }],
    };
    let lane_catalog = catalog_with_lane_dataspaces(&[
        (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
        (lane_id, dataspace_id),
    ]);
    let router = ConfigLaneRouter::new(
        LaneRoutingPolicy::default(),
        DataSpaceCatalog::default(),
        LaneCatalog::default(),
    );
    let permission = Permission::from(CanRegisterTrigger {
        authority: holder_id.clone(),
    });
    let tx = sample_transaction(
        &submitter_id,
        submitter_keypair.private_key(),
        vec![InstructionBox::from(Grant::account_permission(
            permission,
            holder_id.clone(),
        ))],
    );
    let mut scope_entry = crate::nexus::space_directory::AccountScopeDirectoryEntry::default();
    scope_entry.ensure_dataspace(dataspace_id);
    let state = state_with_account_scope_entries(&[(holder_id, scope_entry)], catalog);
    {
        let mut nexus = state.nexus.write();
        nexus.routing_policy = committed_policy;
        nexus.lane_catalog = lane_catalog;
    }
    let state_view = state.view();
    assert_eq!(
        router
            .try_route_plan_with_view(&tx, &state_view)
            .expect("state-view routing should use committed nexus policy")
            .coordinator_route(),
        RoutingDecision::new(lane_id, dataspace_id)
    );
    assert_eq!(
        router
            .try_route_plan_without_state(&tx)
            .expect("permission grant without a cached rule should defer to state"),
        None
    );
    assert_eq!(
        router
            .try_route_plan_with_state(&tx, &state)
            .expect("state routing should use committed nexus policy")
            .coordinator_route(),
        RoutingDecision::new(lane_id, dataspace_id)
    );
}
#[test]
fn account_metadata_write_routes_to_single_scope_dataspace_with_state() {
    let (submitter_id, submitter_keypair) = gen_account_in("wonderland");
    let (target_id, _) = gen_account_in("wonderland");
    let dataspace_id = DataSpaceId::new(10);
    let lane_id = LaneId::new(2);
    let catalog = dataspace_catalog(&[(dataspace_id, "restricted")]);
    let lane_catalog = catalog_with_lane_dataspaces(&[
        (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
        (lane_id, dataspace_id),
    ]);
    let router = ConfigLaneRouter::new(default_routing_policy(), catalog.clone(), lane_catalog);
    let tx = sample_transaction(
        &submitter_id,
        submitter_keypair.private_key(),
        vec![InstructionBox::from(RemoveKeyValue::account(
            target_id.clone(),
            "routing".parse().expect("metadata key"),
        ))],
    );
    let mut scope_entry = crate::nexus::space_directory::AccountScopeDirectoryEntry::default();
    scope_entry.ensure_dataspace(dataspace_id);
    let state = state_with_account_scope_entries(&[(target_id, scope_entry)], catalog);
    state.nexus.write().lane_catalog = router.lane_catalog.as_ref().clone();
    assert_eq!(
        router
            .try_route_without_state(&tx)
            .expect("account metadata writes should defer until account scope is loaded"),
        None
    );
    assert_eq!(
        router
            .try_route_with_view(&tx, &state.view())
            .expect("single-scope account metadata writes should route to that dataspace"),
        RoutingDecision::new(lane_id, dataspace_id)
    );
}
#[test]
fn register_account_with_dataspace_label_routes_without_state() {
    let (submitter_id, submitter_keypair) = gen_account_in("wonderland");
    let (target_id, _) = gen_account_in("wonderland");
    let dataspace_id = DataSpaceId::new(10);
    let lane_id = LaneId::new(2);
    let catalog = dataspace_catalog(&[(dataspace_id, "restricted")]);
    let lane_catalog = catalog_with_lane_dataspaces(&[
        (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
        (lane_id, dataspace_id),
    ]);
    let router = ConfigLaneRouter::new(default_routing_policy(), catalog.clone(), lane_catalog);
    let tx = sample_transaction(
        &submitter_id,
        submitter_keypair.private_key(),
        vec![InstructionBox::from(Register::account(
            Account::new(target_id)
                .with_label(Some(account_alias("merchant@restricted", &catalog))),
        ))],
    );
    assert_eq!(
        router
            .try_route_without_state(&tx)
            .expect("account registration with a dataspace label should route without state"),
        Some(RoutingDecision::new(lane_id, dataspace_id))
    );
}
#[test]
fn multisig_contract_trigger_proposal_routes_by_immutable_contract_dataspace() {
    let (submitter_id, submitter_keypair) = gen_account_in("submitter");
    let (multisig_id, _) = gen_account_in("multisig");
    let (policy, catalog, lane_catalog, router) = three_dataspace_contract_router();
    let contract_dataspace = DataSpaceId::new(9);
    let proposed = vec![
        sample_contract_trigger_registration(
            &multisig_id,
            "proposal_contract_call",
            contract_dataspace,
            1,
        ),
        InstructionBox::from(iroha_data_model::isi::ExecuteTrigger::new(
            "proposal_contract_call".parse().expect("trigger id"),
        )),
    ];
    let tx = sample_transaction(
        &submitter_id,
        submitter_keypair.private_key(),
        vec![InstructionBox::from(MultisigPropose::new(
            multisig_id.clone(),
            proposed,
            None,
        ))],
    );
    let state = state_with_account_scope_entries(
        &[
            (submitter_id, account_scope_entry(DataSpaceId::new(7))),
            (multisig_id, account_scope_entry(DataSpaceId::new(8))),
        ],
        catalog,
    );
    state.nexus.write().lane_catalog = lane_catalog;
    let expected_route = RoutingDecision::new(LaneId::new(4), contract_dataspace);
    let expected_plan = RoutingPlan::single(expected_route);
    assert_eq!(
        router
            .try_route_with_view(&tx, &state.view())
            .expect("proposal must route by the immutable contract address"),
        expected_route
    );
    assert_eq!(
        router
            .try_route_plan_with_view(&tx, &state.view())
            .expect("proposal plan must route by the immutable contract address"),
        expected_plan
    );
    assert_eq!(
        evaluate_policy_plan_with_catalog_and_world(
            &policy,
            router.lane_catalog.as_ref(),
            &state.view().nexus().dataspace_catalog,
            &tx,
            state.view().world(),
        )
        .expect("world-backed proposal routing must match queue routing"),
        expected_plan
    );
    assert_eq!(
        native_amx_participant_dataspaces_with_world(
            &tx,
            &state.view().nexus().dataspace_catalog,
            state.view().world(),
        ),
        vec![contract_dataspace]
    );
}
#[test]
fn multisig_contract_trigger_same_transaction_approval_keeps_contract_route() {
    let (submitter_id, submitter_keypair) = gen_account_in("submitter");
    let (multisig_id, _) = gen_account_in("multisig");
    let (policy, catalog, lane_catalog, router) = three_dataspace_contract_router();
    let contract_dataspace = DataSpaceId::new(9);
    let proposed = vec![
        sample_contract_trigger_registration(
            &multisig_id,
            "same_transaction_contract_call",
            contract_dataspace,
            2,
        ),
        InstructionBox::from(iroha_data_model::isi::ExecuteTrigger::new(
            "same_transaction_contract_call"
                .parse()
                .expect("trigger id"),
        )),
    ];
    let sibling_pair = || {
        let proposed = proposed.clone();
        let instructions_hash = HashOf::new(&proposed);
        vec![
            InstructionBox::from(MultisigPropose::new(multisig_id.clone(), proposed, None)),
            InstructionBox::from(MultisigApprove::new(multisig_id.clone(), instructions_hash)),
        ]
    };
    let executables = [
        (
            "instructions",
            Executable::Instructions(sibling_pair().into()),
        ),
        (
            "batch",
            Executable::Batch(
                sibling_pair()
                    .into_iter()
                    .map(ExecutableBatchItem::Instruction)
                    .collect::<Vec<_>>()
                    .into(),
            ),
        ),
        ("proved overlay", sample_proved_executable(sibling_pair())),
    ];
    let state = state_with_account_scope_entries(
        &[
            (
                submitter_id.clone(),
                account_scope_entry(DataSpaceId::new(7)),
            ),
            (
                multisig_id.clone(),
                account_scope_entry(DataSpaceId::new(8)),
            ),
        ],
        catalog,
    );
    state.nexus.write().lane_catalog = lane_catalog;
    let expected = RoutingPlan::single(RoutingDecision::new(LaneId::new(4), contract_dataspace));
    for (label, executable) in executables {
        let tx = sample_executable_transaction(
            &submitter_id,
            submitter_keypair.private_key(),
            executable,
        );
        assert_eq!(
            router
                .try_route_plan_with_view(&tx, &state.view())
                .unwrap_or_else(|error| panic!("{label} sibling approval failed: {error}")),
            expected,
            "{label} approval must inherit the proposal contract route",
        );
        assert_eq!(
            evaluate_policy_plan_with_catalog_and_world(
                &policy,
                router.lane_catalog.as_ref(),
                &state.view().nexus().dataspace_catalog,
                &tx,
                state.view().world(),
            )
            .unwrap_or_else(|error| {
                panic!("world-backed {label} sibling approval failed: {error}")
            }),
            expected,
        );
        assert_eq!(
            native_amx_participant_dataspaces_with_world(
                &tx,
                &state.view().nexus().dataspace_catalog,
                state.view().world(),
            ),
            vec![contract_dataspace],
            "the {label} sibling approval must not add the multisig account as a participant",
        );
    }
}
#[test]
fn multisig_contract_trigger_later_approval_reads_persisted_contract_route() {
    let (submitter_id, submitter_keypair) = gen_account_in("submitter");
    let (multisig_id, _) = gen_account_in("multisig");
    let (policy, catalog, lane_catalog, router) = three_dataspace_contract_router();
    let contract_dataspace = DataSpaceId::new(9);
    let proposed = vec![
        sample_contract_trigger_registration(
            &multisig_id,
            "persisted_contract_call",
            contract_dataspace,
            3,
        ),
        InstructionBox::from(iroha_data_model::isi::ExecuteTrigger::new(
            "persisted_contract_call".parse().expect("trigger id"),
        )),
    ];
    let instructions_hash = HashOf::new(&proposed);
    let tx = sample_transaction(
        &submitter_id,
        submitter_keypair.private_key(),
        vec![InstructionBox::from(MultisigApprove::new(
            multisig_id.clone(),
            instructions_hash,
        ))],
    );
    let mut state = state_with_account_scope_entries(
        &[
            (submitter_id, account_scope_entry(DataSpaceId::new(7))),
            (
                multisig_id.clone(),
                account_scope_entry(DataSpaceId::new(8)),
            ),
        ],
        catalog,
    );
    state.nexus.write().lane_catalog = lane_catalog;
    let proposal_state = MultisigProposalState::new(
        multisig_id.clone(),
        instructions_hash,
        proposed,
        1,
        10_000,
        BTreeSet::new(),
        None,
    );
    state.world.smart_contract_state_mut_for_testing().insert(
        multisig_proposal_state_key(&multisig_id, &instructions_hash),
        norito::to_bytes(&proposal_state).expect("proposal state should encode"),
    );
    let expected = RoutingPlan::single(RoutingDecision::new(LaneId::new(4), contract_dataspace));
    assert_eq!(
        router
            .try_route_plan_with_view(&tx, &state.view())
            .expect("persisted proposal must override the multisig account route"),
        expected
    );
    assert_eq!(
        evaluate_policy_plan_with_catalog_and_world(
            &policy,
            router.lane_catalog.as_ref(),
            &state.view().nexus().dataspace_catalog,
            &tx,
            state.view().world(),
        )
        .expect("world-backed approval must read the persisted contract route"),
        expected
    );
}
#[test]
fn nested_trigger_instruction_and_proved_overlay_route_to_contract_dataspace() {
    let (submitter_id, submitter_keypair) = gen_account_in("submitter");
    let (_policy, catalog, lane_catalog, router) = three_dataspace_contract_router();
    let contract_dataspace = DataSpaceId::new(9);
    let inner = sample_contract_trigger_registration(
        &submitter_id,
        "nested_contract_call",
        contract_dataspace,
        4,
    );
    let proved = sample_proved_executable(vec![inner]);
    let outer = sample_trigger_registration(
        &submitter_id,
        "proved_contract_wrapper",
        Executable::Instructions(
            vec![sample_trigger_registration(
                &submitter_id,
                "instruction_contract_wrapper",
                proved,
            )]
            .into(),
        ),
    );
    let tx = sample_transaction(&submitter_id, submitter_keypair.private_key(), vec![outer]);
    let state = state_with_account_scope_entries(
        &[(submitter_id, account_scope_entry(DataSpaceId::new(7)))],
        catalog,
    );
    state.nexus.write().lane_catalog = lane_catalog;
    assert_eq!(
        router
            .try_route_without_state(&tx)
            .expect("contract-only nested trigger routing must be state-free"),
        Some(RoutingDecision::new(LaneId::new(4), contract_dataspace))
    );
    assert_eq!(
        router
            .try_route_with_view(&tx, &state.view())
            .expect("nested trigger executable must resolve recursively"),
        RoutingDecision::new(LaneId::new(4), contract_dataspace)
    );
}
#[test]
fn conflicting_nested_contract_triggers_build_amx_plan_or_fail_strictly() {
    let (submitter_id, submitter_keypair) = gen_account_in("submitter");
    let (policy, catalog, lane_catalog, router) = three_dataspace_contract_router();
    let multisig_dataspace = DataSpaceId::new(8);
    let contract_dataspace = DataSpaceId::new(9);
    let nested = vec![
        sample_contract_trigger_registration(
            &submitter_id,
            "first_nested_contract",
            multisig_dataspace,
            5,
        ),
        sample_contract_trigger_registration(
            &submitter_id,
            "second_nested_contract",
            contract_dataspace,
            6,
        ),
    ];
    let outer = sample_trigger_registration(
        &submitter_id,
        "cross_dataspace_contract_wrapper",
        Executable::Instructions(nested.into()),
    );
    let state = state_with_account_scope_entries(
        &[(
            submitter_id.clone(),
            account_scope_entry(DataSpaceId::new(7)),
        )],
        catalog,
    );
    state.nexus.write().lane_catalog = lane_catalog;
    let tx = sample_transaction(
        &submitter_id,
        submitter_keypair.private_key(),
        vec![outer.clone()],
    );
    assert_eq!(
        router
            .try_route_with_view(&tx, &state.view())
            .expect("mixed nested contract targets must use the universal coordinator"),
        RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL)
    );
    let plan = router
        .try_route_plan_with_view(&tx, &state.view())
        .expect("mixed nested contract targets must build an AMX plan");
    let RoutingPlan::NativeAmx(plan) = plan else {
        panic!("mixed nested contract targets must not collapse to a single route");
    };
    assert_eq!(
        plan.coordinator.route,
        RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL)
    );
    assert_eq!(
        plan.participants
            .iter()
            .map(|participant| participant.route.dataspace_id)
            .collect::<Vec<_>>(),
        vec![multisig_dataspace, contract_dataspace]
    );
    assert_eq!(
        native_amx_participant_dataspaces_with_world(
            &tx,
            &state.view().nexus().dataspace_catalog,
            state.view().world(),
        ),
        vec![multisig_dataspace, contract_dataspace]
    );
    assert!(matches!(
        evaluate_policy_plan_with_catalog_and_world(
            &policy,
            router.lane_catalog.as_ref(),
            &state.view().nexus().dataspace_catalog,
            &tx,
            state.view().world(),
        ),
        Ok(RoutingPlan::NativeAmx(_))
    ));
    let mut strict_metadata = Metadata::default();
    strict_metadata.insert(
        AMX_POLICY_METADATA_KEY.parse().expect("amx policy key"),
        iroha_primitives::json::Json::new(AMX_POLICY_REJECT_CROSS_DATASPACE),
    );
    let strict_tx = sample_transaction_with_metadata(
        &submitter_id,
        submitter_keypair.private_key(),
        vec![outer],
        strict_metadata,
    );
    assert_eq!(
        router.try_route_plan_with_view(&strict_tx, &state.view()),
        Err(
            RoutingResolveError::ConflictingTransactionDataspaceTargets {
                first_dataspace_id: multisig_dataspace,
                second_dataspace_id: contract_dataspace,
            }
        )
    );
}
#[test]
fn non_contract_trigger_keeps_multisig_account_fallback() {
    let (submitter_id, submitter_keypair) = gen_account_in("submitter");
    let (multisig_id, _) = gen_account_in("multisig");
    let (_policy, catalog, lane_catalog, router) = three_dataspace_contract_router();
    let proposed = vec![
        sample_trigger_registration(
            &multisig_id,
            "non_contract_proved_trigger",
            sample_proved_executable(Vec::new()),
        ),
        InstructionBox::from(iroha_data_model::isi::ExecuteTrigger::new(
            "non_contract_proved_trigger".parse().expect("trigger id"),
        )),
    ];
    let tx = sample_transaction(
        &submitter_id,
        submitter_keypair.private_key(),
        vec![InstructionBox::from(MultisigPropose::new(
            multisig_id.clone(),
            proposed,
            None,
        ))],
    );
    let state = state_with_account_scope_entries(
        &[
            (submitter_id, account_scope_entry(DataSpaceId::new(7))),
            (multisig_id, account_scope_entry(DataSpaceId::new(8))),
        ],
        catalog,
    );
    state.nexus.write().lane_catalog = lane_catalog;
    assert_eq!(
        router
            .try_route_with_view(&tx, &state.view())
            .expect("targetless trigger must retain the multisig account fallback"),
        RoutingDecision::new(LaneId::new(3), DataSpaceId::new(8))
    );
}
#[test]
fn multisig_propose_routes_by_embedded_instruction_dataspace() {
    multisig_routing_fixture!(submitter_id submitter_keypair multisig_id dataspace_id lane_id catalog lane_catalog policy router proposed);
    let tx = sample_transaction(
        &submitter_id,
        submitter_keypair.private_key(),
        vec![InstructionBox::from(MultisigPropose::new(
            multisig_id,
            proposed,
            None,
        ))],
    );
    let state = state_with_account_scope_entries(&[], catalog);
    state.nexus.write().lane_catalog = lane_catalog;
    let expected_route = RoutingDecision::new(lane_id, dataspace_id);
    let expected_plan = RoutingPlan::single(expected_route);
    assert_eq!(
        router
            .try_route_without_state(&tx)
            .expect("multisig proposal should defer to state-aware routing"),
        None
    );
    assert_eq!(
        router
            .try_route_with_view(&tx, &state.view())
            .expect("embedded proposal target should route to its dataspace"),
        expected_route
    );
    assert_eq!(
        router
            .try_route_plan_with_view(&tx, &state.view())
            .expect("embedded proposal plan should route to its dataspace"),
        expected_plan
    );
    assert_eq!(
        evaluate_policy_with_catalog_and_world(
            &policy,
            router.lane_catalog.as_ref(),
            &state.view().nexus().dataspace_catalog,
            &tx,
            state.view().world(),
        )
        .expect("validation routing should match proposal routing"),
        expected_route
    );
    assert_eq!(
        evaluate_policy_plan_with_catalog_and_world(
            &policy,
            router.lane_catalog.as_ref(),
            &state.view().nexus().dataspace_catalog,
            &tx,
            state.view().world(),
        )
        .expect("validation routing plan should match proposal routing plan"),
        expected_plan
    );
}
#[test]
fn multisig_propose_plan_prefers_embedded_dataspace_over_multiscope_account() {
    multisig_routing_fixture!(submitter_id submitter_keypair multisig_id dataspace_id lane_id catalog lane_catalog policy router proposed);
    let tx = sample_transaction(
        &submitter_id,
        submitter_keypair.private_key(),
        vec![InstructionBox::from(MultisigPropose::new(
            multisig_id.clone(),
            proposed,
            None,
        ))],
    );
    let mut scope_entry = crate::nexus::space_directory::AccountScopeDirectoryEntry::default();
    scope_entry.ensure_dataspace(DataSpaceId::UNIVERSAL);
    scope_entry.ensure_dataspace(dataspace_id);
    let state = state_with_account_scope_entries(&[(multisig_id, scope_entry)], catalog);
    state.nexus.write().lane_catalog = lane_catalog;
    let expected_route = RoutingDecision::new(lane_id, dataspace_id);
    let expected_plan = RoutingPlan::single(expected_route);
    assert_eq!(
        router
            .try_route_without_state(&tx)
            .expect("multisig proposal should defer to state-aware routing"),
        None
    );
    assert_eq!(
        router
            .try_route_plan_with_view(&tx, &state.view())
            .expect("proposal plan should use the embedded write dataspace"),
        expected_plan
    );
    assert_eq!(
        evaluate_policy_plan_with_catalog_and_world(
            &policy,
            router.lane_catalog.as_ref(),
            &state.view().nexus().dataspace_catalog,
            &tx,
            state.view().world(),
        )
        .expect("validation plan should use the embedded write dataspace"),
        expected_plan
    );
}
#[test]
fn multisig_same_transaction_approve_uses_sibling_proposal_route() {
    multisig_routing_fixture!(submitter_id submitter_keypair multisig_id dataspace_id lane_id catalog lane_catalog policy router proposed);
    let proposal_hash = HashOf::new(&proposed);
    let tx = sample_transaction(
        &submitter_id,
        submitter_keypair.private_key(),
        vec![
            InstructionBox::from(MultisigPropose::new(multisig_id.clone(), proposed, None)),
            InstructionBox::from(MultisigApprove::new(multisig_id.clone(), proposal_hash)),
        ],
    );
    let mut scope_entry = crate::nexus::space_directory::AccountScopeDirectoryEntry::default();
    scope_entry.ensure_dataspace(DataSpaceId::UNIVERSAL);
    let state = state_with_account_scope_entries(&[(multisig_id, scope_entry)], catalog);
    state.nexus.write().lane_catalog = lane_catalog;
    let expected_plan = RoutingPlan::single(RoutingDecision::new(lane_id, dataspace_id));
    assert_eq!(
        router
            .try_route_plan_with_view(&tx, &state.view())
            .expect("same-transaction approval should use the sibling proposal route"),
        expected_plan
    );
    assert_eq!(
        evaluate_policy_plan_with_catalog_and_world(
            &policy,
            router.lane_catalog.as_ref(),
            &state.view().nexus().dataspace_catalog,
            &tx,
            state.view().world(),
        )
        .expect("validation plan should use the sibling proposal route"),
        expected_plan
    );
}
#[test]
fn multisig_approve_before_sibling_proposal_keeps_account_scope_target() {
    let (submitter_id, submitter_keypair) = gen_account_in("wonderland");
    let (multisig_id, _) = gen_account_in("wonderland");
    let (target_id, _) = gen_account_in("wonderland");
    let proposed_dataspace = DataSpaceId::new(10);
    let account_dataspace = DataSpaceId::new(11);
    let proposed_lane = LaneId::new(2);
    let account_lane = LaneId::new(3);
    let catalog = dataspace_catalog(&[
        (proposed_dataspace, "restricted"),
        (account_dataspace, "multisig"),
    ]);
    let lane_catalog = catalog_with_lane_dataspaces(&[
        (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
        (proposed_lane, proposed_dataspace),
        (account_lane, account_dataspace),
    ]);
    let policy = default_routing_policy();
    let router = ConfigLaneRouter::new(policy.clone(), catalog.clone(), lane_catalog.clone());
    let proposed = vec![InstructionBox::from(Register::account(
        Account::new(target_id).with_label(Some(account_alias("retail@restricted", &catalog))),
    ))];
    let proposal_hash = HashOf::new(&proposed);
    let tx = sample_transaction(
        &submitter_id,
        submitter_keypair.private_key(),
        vec![
            InstructionBox::from(MultisigApprove::new(multisig_id.clone(), proposal_hash)),
            InstructionBox::from(MultisigPropose::new(multisig_id.clone(), proposed, None)),
        ],
    );
    let mut scope_entry = crate::nexus::space_directory::AccountScopeDirectoryEntry::default();
    scope_entry.ensure_dataspace(account_dataspace);
    let state = state_with_account_scope_entries(&[(multisig_id, scope_entry)], catalog);
    state.nexus.write().lane_catalog = lane_catalog;
    let expected_plan = RoutingPlan::native_amx(
        RoutingDecision::new(proposed_lane, proposed_dataspace),
        vec![
            RouteLeg::new(
                RoutingDecision::new(proposed_lane, proposed_dataspace),
                RouteLegRole::Participant,
            ),
            RouteLeg::new(
                RoutingDecision::new(account_lane, account_dataspace),
                RouteLegRole::Participant,
            ),
        ],
    );
    assert_eq!(
        router
            .try_route_plan_with_view(&tx, &state.view())
            .expect("an approval must not correlate with a later sibling proposal"),
        expected_plan
    );
    assert_eq!(
        evaluate_policy_plan_with_catalog_and_world(
            &policy,
            router.lane_catalog.as_ref(),
            &state.view().nexus().dataspace_catalog,
            &tx,
            state.view().world(),
        )
        .expect("validation routing must preserve the approval account target"),
        expected_plan
    );
}
#[test]
fn custom_multisig_propose_defers_and_routes_by_embedded_instruction_dataspace() {
    multisig_routing_fixture!(submitter_id submitter_keypair multisig_id dataspace_id lane_id catalog lane_catalog policy router proposed);
    let tx = sample_transaction(
        &submitter_id,
        submitter_keypair.private_key(),
        vec![InstructionBox::from(CustomInstruction::new(
            iroha_primitives::json::Json::new(MultisigInstructionBox::Propose(
                MultisigPropose::new(multisig_id, proposed, None),
            )),
        ))],
    );
    let state = state_with_account_scope_entries(&[], catalog);
    state.nexus.write().lane_catalog = lane_catalog;
    assert_eq!(
        router
            .try_route_without_state(&tx)
            .expect("custom multisig proposal should defer to state-aware routing"),
        None
    );
    assert_eq!(
        router
            .try_route_with_view(&tx, &state.view())
            .expect("custom embedded proposal target should route to its dataspace"),
        RoutingDecision::new(lane_id, dataspace_id)
    );
    assert_eq!(
        evaluate_policy_with_catalog_and_world(
            &policy,
            router.lane_catalog.as_ref(),
            &state.view().nexus().dataspace_catalog,
            &tx,
            state.view().world(),
        )
        .expect("validation routing should match custom proposal routing"),
        RoutingDecision::new(lane_id, dataspace_id)
    );
}
#[test]
fn multisig_approve_routes_by_multisig_account_scope() {
    multisig_routing_fixture!(submitter_id submitter_keypair multisig_id dataspace_id lane_id catalog lane_catalog policy router proposed);
    let instructions_hash = HashOf::new(&proposed);
    let tx = sample_transaction(
        &submitter_id,
        submitter_keypair.private_key(),
        vec![InstructionBox::from(MultisigApprove::new(
            multisig_id.clone(),
            instructions_hash,
        ))],
    );
    let mut scope_entry = crate::nexus::space_directory::AccountScopeDirectoryEntry::default();
    scope_entry.ensure_dataspace(dataspace_id);
    let state = state_with_account_scope_entries(&[(multisig_id, scope_entry)], catalog);
    state.nexus.write().lane_catalog = lane_catalog;
    assert_eq!(
        router
            .try_route_without_state(&tx)
            .expect("multisig approval should defer to state-aware routing"),
        None
    );
    assert_eq!(
        router
            .try_route_with_view(&tx, &state.view())
            .expect("approval should route by multisig account scope"),
        RoutingDecision::new(lane_id, dataspace_id)
    );
    assert_eq!(
        evaluate_policy_with_catalog_and_world(
            &policy,
            router.lane_catalog.as_ref(),
            &state.view().nexus().dataspace_catalog,
            &tx,
            state.view().world(),
        )
        .expect("validation routing should match approval routing"),
        RoutingDecision::new(lane_id, dataspace_id)
    );
}
#[test]
fn custom_multisig_approve_defers_and_routes_by_multisig_account_scope() {
    multisig_routing_fixture!(submitter_id submitter_keypair multisig_id dataspace_id lane_id catalog lane_catalog policy router proposed);
    let instructions_hash = HashOf::new(&proposed);
    let tx = sample_transaction(
        &submitter_id,
        submitter_keypair.private_key(),
        vec![InstructionBox::from(CustomInstruction::new(
            iroha_primitives::json::Json::new(MultisigInstructionBox::Approve(
                MultisigApprove::new(multisig_id.clone(), instructions_hash),
            )),
        ))],
    );
    let mut scope_entry = crate::nexus::space_directory::AccountScopeDirectoryEntry::default();
    scope_entry.ensure_dataspace(dataspace_id);
    let state = state_with_account_scope_entries(&[(multisig_id, scope_entry)], catalog);
    state.nexus.write().lane_catalog = lane_catalog;
    assert_eq!(
        router
            .try_route_without_state(&tx)
            .expect("custom multisig approval should defer to state-aware routing"),
        None
    );
    assert_eq!(
        router
            .try_route_with_view(&tx, &state.view())
            .expect("custom approval should route by multisig account scope"),
        RoutingDecision::new(lane_id, dataspace_id)
    );
    assert_eq!(
        evaluate_policy_with_catalog_and_world(
            &policy,
            router.lane_catalog.as_ref(),
            &state.view().nexus().dataspace_catalog,
            &tx,
            state.view().world(),
        )
        .expect("validation routing should match custom approval routing"),
        RoutingDecision::new(lane_id, dataspace_id)
    );
}
#[test]
fn multisig_approve_routes_by_persisted_proposal_when_scope_is_missing() {
    multisig_routing_fixture!(submitter_id submitter_keypair multisig_id dataspace_id lane_id catalog lane_catalog policy router proposed);
    let instructions_hash = HashOf::new(&proposed);
    let tx = sample_transaction(
        &submitter_id,
        submitter_keypair.private_key(),
        vec![InstructionBox::from(MultisigApprove::new(
            multisig_id.clone(),
            instructions_hash,
        ))],
    );
    let mut state = state_with_account_scope_entries(&[], catalog);
    state.nexus.write().lane_catalog = lane_catalog;
    let proposal_state = MultisigProposalState::new(
        multisig_id.clone(),
        instructions_hash,
        proposed,
        1,
        10_000,
        BTreeSet::new(),
        None,
    );
    state.world.smart_contract_state_mut_for_testing().insert(
        multisig_proposal_state_key(&multisig_id, &instructions_hash),
        norito::to_bytes(&proposal_state).expect("proposal state should encode"),
    );
    let expected_route = RoutingDecision::new(lane_id, dataspace_id);
    let expected_plan = RoutingPlan::single(expected_route);
    assert_eq!(
        router
            .try_route_without_state(&tx)
            .expect("multisig approval should defer to state-aware routing"),
        None
    );
    assert_eq!(
        router.try_route_with_view(&tx, &state.view()).expect(
            "approval should route by embedded proposal target when account scope is absent"
        ),
        expected_route
    );
    assert_eq!(
        router
            .try_route_plan_with_view(&tx, &state.view())
            .expect("approval plan should route by embedded proposal target"),
        expected_plan
    );
    assert_eq!(
        evaluate_policy_with_catalog_and_world(
            &policy,
            router.lane_catalog.as_ref(),
            &state.view().nexus().dataspace_catalog,
            &tx,
            state.view().world(),
        )
        .expect("validation routing should match proposal-state fallback routing"),
        expected_route
    );
    assert_eq!(
        evaluate_policy_plan_with_catalog_and_world(
            &policy,
            router.lane_catalog.as_ref(),
            &state.view().nexus().dataspace_catalog,
            &tx,
            state.view().world(),
        )
        .expect("validation routing plan should match proposal-state fallback routing"),
        expected_plan
    );
}
#[test]
fn multisig_approve_ignores_corrupt_proposal_state_and_uses_account_scope() {
    multisig_routing_fixture!(submitter_id submitter_keypair multisig_id dataspace_id lane_id catalog lane_catalog policy router proposed);
    let instructions_hash = HashOf::new(&proposed);
    let tx = sample_transaction(
        &submitter_id,
        submitter_keypair.private_key(),
        vec![InstructionBox::from(MultisigApprove::new(
            multisig_id.clone(),
            instructions_hash,
        ))],
    );
    let mut scope_entry = crate::nexus::space_directory::AccountScopeDirectoryEntry::default();
    scope_entry.ensure_dataspace(dataspace_id);
    let mut state =
        state_with_account_scope_entries(&[(multisig_id.clone(), scope_entry)], catalog);
    state.nexus.write().lane_catalog = lane_catalog;
    state.world.smart_contract_state_mut_for_testing().insert(
        multisig_proposal_state_key(&multisig_id, &instructions_hash),
        b"not a multisig proposal state".to_vec(),
    );
    let expected_route = RoutingDecision::new(lane_id, dataspace_id);
    let expected_plan = RoutingPlan::single(expected_route);
    assert_eq!(
        router
            .try_route_without_state(&tx)
            .expect("multisig approval should defer to state-aware routing"),
        None
    );
    assert_eq!(
        router
            .try_route_with_view(&tx, &state.view())
            .expect("corrupt proposal state should fall back to multisig account scope"),
        expected_route
    );
    assert_eq!(
        router
            .try_route_plan_with_view(&tx, &state.view())
            .expect("corrupt proposal state plan should fall back to multisig account scope"),
        expected_plan
    );
    assert_eq!(
        evaluate_policy_plan_with_catalog_and_world(
            &policy,
            router.lane_catalog.as_ref(),
            &state.view().nexus().dataspace_catalog,
            &tx,
            state.view().world(),
        )
        .expect("validation routing should ignore corrupt proposal state")
        .coordinator_route(),
        expected_route
    );
}
#[test]
fn multisig_approve_ignores_unrelated_persisted_proposal_hash() {
    let (submitter_id, submitter_keypair) = gen_account_in("wonderland");
    let (multisig_id, _) = gen_account_in("wonderland");
    let (approved_target_id, _) = gen_account_in("wonderland");
    let (stale_target_id, _) = gen_account_in("wonderland");
    let account_dataspace = DataSpaceId::new(10);
    let stale_dataspace = DataSpaceId::new(11);
    let account_lane = LaneId::new(2);
    let stale_lane = LaneId::new(3);
    let catalog = dataspace_catalog(&[
        (account_dataspace, "restricted"),
        (stale_dataspace, "stale-restricted"),
    ]);
    let lane_catalog = catalog_with_lane_dataspaces(&[
        (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
        (account_lane, account_dataspace),
        (stale_lane, stale_dataspace),
    ]);
    let policy = default_routing_policy();
    let router = ConfigLaneRouter::new(policy.clone(), catalog.clone(), lane_catalog.clone());
    let approved = vec![InstructionBox::from(Register::account(
        Account::new(approved_target_id)
            .with_label(Some(account_alias("approved@restricted", &catalog))),
    ))];
    let approved_hash = HashOf::new(&approved);
    let stale_proposed = vec![InstructionBox::from(Register::account(
        Account::new(stale_target_id)
            .with_label(Some(account_alias("stale@stale-restricted", &catalog))),
    ))];
    let stale_hash = HashOf::new(&stale_proposed);
    let tx = sample_transaction(
        &submitter_id,
        submitter_keypair.private_key(),
        vec![InstructionBox::from(MultisigApprove::new(
            multisig_id.clone(),
            approved_hash,
        ))],
    );
    let mut scope_entry = crate::nexus::space_directory::AccountScopeDirectoryEntry::default();
    scope_entry.ensure_dataspace(account_dataspace);
    let mut state =
        state_with_account_scope_entries(&[(multisig_id.clone(), scope_entry)], catalog);
    state.nexus.write().lane_catalog = lane_catalog;
    let stale_state = MultisigProposalState::new(
        multisig_id.clone(),
        stale_hash,
        stale_proposed,
        1,
        10_000,
        BTreeSet::new(),
        None,
    );
    state.world.smart_contract_state_mut_for_testing().insert(
        multisig_proposal_state_key(&multisig_id, &stale_hash),
        norito::to_bytes(&stale_state).expect("stale proposal state should encode"),
    );
    let expected_route = RoutingDecision::new(account_lane, account_dataspace);
    let expected_plan = RoutingPlan::single(expected_route);
    assert_eq!(
        router
            .try_route_without_state(&tx)
            .expect("multisig approval should defer to state-aware routing"),
        None
    );
    assert_eq!(
        router
            .try_route_with_view(&tx, &state.view())
            .expect("unrelated proposal state should not route this approval"),
        expected_route
    );
    assert_eq!(
        router
            .try_route_plan_with_view(&tx, &state.view())
            .expect("unrelated proposal state plan should fall back to account scope"),
        expected_plan
    );
    assert_eq!(
        evaluate_policy_plan_with_catalog_and_world(
            &policy,
            router.lane_catalog.as_ref(),
            &state.view().nexus().dataspace_catalog,
            &tx,
            state.view().world(),
        )
        .expect("validation routing should ignore unrelated proposal state")
        .coordinator_route(),
        expected_route
    );
}
#[test]
fn multisig_approve_plan_prefers_visible_proposal_over_multiscope_account() {
    multisig_routing_fixture!(submitter_id submitter_keypair multisig_id dataspace_id lane_id catalog lane_catalog policy router proposed);
    let instructions_hash = HashOf::new(&proposed);
    let tx = sample_transaction(
        &submitter_id,
        submitter_keypair.private_key(),
        vec![InstructionBox::from(MultisigApprove::new(
            multisig_id.clone(),
            instructions_hash,
        ))],
    );
    let mut scope_entry = crate::nexus::space_directory::AccountScopeDirectoryEntry::default();
    scope_entry.ensure_dataspace(DataSpaceId::UNIVERSAL);
    scope_entry.ensure_dataspace(dataspace_id);
    let mut state =
        state_with_account_scope_entries(&[(multisig_id.clone(), scope_entry)], catalog);
    state.nexus.write().lane_catalog = lane_catalog;
    let proposal_state = MultisigProposalState::new(
        multisig_id.clone(),
        instructions_hash,
        proposed,
        1,
        10_000,
        BTreeSet::new(),
        None,
    );
    state.world.smart_contract_state_mut_for_testing().insert(
        multisig_proposal_state_key(&multisig_id, &instructions_hash),
        norito::to_bytes(&proposal_state).expect("proposal state should encode"),
    );
    let expected_route = RoutingDecision::new(lane_id, dataspace_id);
    let expected_plan = RoutingPlan::single(expected_route);
    assert_eq!(
        router
            .try_route_without_state(&tx)
            .expect("multisig approval should defer to state-aware routing"),
        None
    );
    assert_eq!(
        router
            .try_route_plan_with_view(&tx, &state.view())
            .expect("visible proposal should override multiscope account route"),
        expected_plan
    );
    assert_eq!(
        evaluate_policy_plan_with_catalog_and_world(
            &policy,
            router.lane_catalog.as_ref(),
            &state.view().nexus().dataspace_catalog,
            &tx,
            state.view().world(),
        )
        .expect("validation plan should prefer visible proposal target"),
        expected_plan
    );
}
