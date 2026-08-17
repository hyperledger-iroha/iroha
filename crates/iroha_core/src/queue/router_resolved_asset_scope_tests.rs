// Resolved account and asset-definition scope routing regressions.

#[test]
fn account_metadata_write_with_multiple_scopes_falls_back_to_default_route() {
    let (submitter_id, submitter_keypair) = gen_account_in("wonderland");
    let (target_id, _) = gen_account_in("wonderland");
    let first_dataspace = DataSpaceId::new(1);
    let second_dataspace = DataSpaceId::new(10);
    let catalog = dataspace_catalog(&[
        (first_dataspace, "governance"),
        (second_dataspace, "restricted"),
    ]);
    let lane_catalog = catalog_with_lane_dataspaces(&[
        (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
        (LaneId::new(1), first_dataspace),
        (LaneId::new(2), second_dataspace),
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
    scope_entry.ensure_dataspace(first_dataspace);
    scope_entry.ensure_dataspace(second_dataspace);
    let state = state_with_account_scope_entries(&[(target_id, scope_entry)], catalog);
    state.nexus.write().lane_catalog = router.lane_catalog.as_ref().clone();
    assert_eq!(
        router
            .try_route_without_state(&tx)
            .expect("multi-scope account metadata writes should defer until scope is loaded"),
        None
    );
    assert_eq!(
        router
            .try_route_with_view(&tx, &state.view())
            .expect("multi-scope account metadata writes should fall back to the default route"),
        RoutingDecision::default()
    );
}

#[test]
fn opaque_asset_definition_unregister_routes_to_resolved_target_dataspace() {
    let (submitter_id, submitter_keypair) = gen_account_in("wonderland");
    let dataspace_id = DataSpaceId::new(10);
    let lane_id = LaneId::new(2);
    let catalog = dataspace_catalog(&[(dataspace_id, "restricted")]);
    let lane_catalog = catalog_with_lane_dataspaces(&[
        (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
        (lane_id, dataspace_id),
    ]);
    let router = ConfigLaneRouter::new(default_routing_policy(), catalog.clone(), lane_catalog);
    let owning_domain = DomainId::try_new("vault", "restricted").expect("domain id");
    let asset_definition = AssetDefinitionId::derive_from_components(
        owning_domain.clone(),
        "voucher".parse().expect("asset definition name"),
    );
    let opaque_asset_definition =
        AssetDefinitionId::parse_address_literal(&asset_definition.canonical_address())
            .expect("opaque canonical asset definition id");
    let tx = sample_transaction(
        &submitter_id,
        submitter_keypair.private_key(),
        vec![InstructionBox::from(Unregister::asset_definition(
            opaque_asset_definition,
        ))],
    );
    let state = state_with_asset_definitions(
        vec![
            AssetDefinition::numeric(
                asset_definition,
                "voucher".to_owned(),
                iroha_data_model::asset::AssetBalancePolicy::Global,
                Some(owning_domain),
            )
            .build(&submitter_id),
        ],
        catalog,
        router.lane_catalog.as_ref().clone(),
    );
    assert_eq!(
        router
            .try_route_without_state(&tx)
            .expect("opaque asset-definition unregisters should defer to state"),
        None
    );
    assert_eq!(
        router
            .try_route_with_view(&tx, &state.view())
            .expect("opaque asset-definition unregister should route to the resolved dataspace"),
        RoutingDecision::new(lane_id, dataspace_id)
    );
}

#[test]
fn opaque_asset_definition_metadata_set_routes_to_resolved_target_dataspace() {
    let (submitter_id, submitter_keypair) = gen_account_in("wonderland");
    let dataspace_id = DataSpaceId::new(10);
    let lane_id = LaneId::new(2);
    let catalog = dataspace_catalog(&[(dataspace_id, "restricted")]);
    let lane_catalog = catalog_with_lane_dataspaces(&[
        (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
        (lane_id, dataspace_id),
    ]);
    let router = ConfigLaneRouter::new(default_routing_policy(), catalog.clone(), lane_catalog);
    let owning_domain = DomainId::try_new("vault", "restricted").expect("domain id");
    let asset_definition = AssetDefinitionId::derive_from_components(
        owning_domain.clone(),
        "voucher".parse().expect("asset definition name"),
    );
    let opaque_asset_definition =
        AssetDefinitionId::parse_address_literal(&asset_definition.canonical_address())
            .expect("opaque canonical asset definition id");
    let tx = sample_transaction(
        &submitter_id,
        submitter_keypair.private_key(),
        vec![InstructionBox::from(SetKeyValue::asset_definition(
            opaque_asset_definition,
            "routing".parse().expect("metadata key"),
            Json::from("ok"),
        ))],
    );
    let state = state_with_asset_definitions(
        vec![
            AssetDefinition::numeric(
                asset_definition,
                "voucher".to_owned(),
                iroha_data_model::asset::AssetBalancePolicy::Global,
                Some(owning_domain),
            )
            .build(&submitter_id),
        ],
        catalog,
        router.lane_catalog.as_ref().clone(),
    );
    assert_eq!(
        router
            .try_route_without_state(&tx)
            .expect("opaque asset-definition metadata sets should defer to state"),
        None
    );
    assert_eq!(
        router
            .try_route_with_view(&tx, &state.view())
            .expect("opaque asset-definition metadata set should route to the resolved dataspace"),
        RoutingDecision::new(lane_id, dataspace_id)
    );
}

#[test]
fn opaque_global_asset_definition_metadata_set_uses_stored_alias_dataspace() {
    let (submitter_id, submitter_keypair) = gen_account_in("wonderland");
    let dataspace_id = DataSpaceId::new(10);
    let lane_id = LaneId::new(2);
    let catalog = dataspace_catalog(&[(dataspace_id, "paynet")]);
    let lane_catalog = catalog_with_lane_dataspaces(&[
        (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
        (lane_id, dataspace_id),
    ]);
    let router = ConfigLaneRouter::new(default_routing_policy(), catalog.clone(), lane_catalog);
    let transparent_asset_definition = AssetDefinitionId::derive_from_components(
        DomainId::try_new("cash", "universal").expect("domain id"),
        "pkr".parse().expect("asset definition name"),
    );
    let opaque_asset_definition =
        AssetDefinitionId::parse_address_literal(&transparent_asset_definition.canonical_address())
            .expect("opaque canonical asset definition id");
    let tx = sample_transaction(
        &submitter_id,
        submitter_keypair.private_key(),
        vec![InstructionBox::from(SetKeyValue::asset_definition(
            opaque_asset_definition.clone(),
            "routing".parse().expect("metadata key"),
            Json::from("paynet"),
        ))],
    );
    let alias: AssetDefinitionAlias = "pkr#paynet".parse().expect("asset alias");
    let mut state = state_with_asset_definitions(
        vec![
            AssetDefinition::numeric(
                opaque_asset_definition.clone(),
                "pkr".to_owned(),
                AssetBalancePolicy::Global,
                None,
            )
            .build(&submitter_id),
        ],
        catalog,
        router.lane_catalog.as_ref().clone(),
    );
    state
        .world
        .asset_definition_aliases
        .insert(alias.clone(), opaque_asset_definition.clone());
    state.world.asset_definition_alias_bindings.insert(
        opaque_asset_definition,
        crate::state::AssetDefinitionAliasBindingRecord {
            alias,
            lease_expiry_ms: None,
            grace_until_ms: None,
            bound_at_ms: 0,
        },
    );
    assert_eq!(
        router
            .try_route_without_state(&tx)
            .expect("opaque global metadata set should defer to state"),
        None
    );
    assert_eq!(
        router
            .try_route_with_view(&tx, &state.view())
            .expect("opaque global metadata set should route through the stored alias home"),
        RoutingDecision::new(lane_id, dataspace_id)
    );
}

#[test]
fn opaque_asset_definition_metadata_remove_routes_to_resolved_target_dataspace() {
    let (submitter_id, submitter_keypair) = gen_account_in("wonderland");
    let dataspace_id = DataSpaceId::new(10);
    let lane_id = LaneId::new(2);
    let catalog = dataspace_catalog(&[(dataspace_id, "restricted")]);
    let lane_catalog = catalog_with_lane_dataspaces(&[
        (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
        (lane_id, dataspace_id),
    ]);
    let router = ConfigLaneRouter::new(default_routing_policy(), catalog.clone(), lane_catalog);
    let owning_domain = DomainId::try_new("vault", "restricted").expect("domain id");
    let asset_definition = AssetDefinitionId::derive_from_components(
        owning_domain.clone(),
        "voucher".parse().expect("asset definition name"),
    );
    let opaque_asset_definition =
        AssetDefinitionId::parse_address_literal(&asset_definition.canonical_address())
            .expect("opaque canonical asset definition id");
    let tx = sample_transaction(
        &submitter_id,
        submitter_keypair.private_key(),
        vec![InstructionBox::from(RemoveKeyValue::asset_definition(
            opaque_asset_definition,
            "routing".parse().expect("metadata key"),
        ))],
    );
    let state = state_with_asset_definitions(
        vec![
            AssetDefinition::numeric(
                asset_definition,
                "voucher".to_owned(),
                iroha_data_model::asset::AssetBalancePolicy::Global,
                Some(owning_domain),
            )
            .build(&submitter_id),
        ],
        catalog,
        router.lane_catalog.as_ref().clone(),
    );
    assert_eq!(
        router
            .try_route_without_state(&tx)
            .expect("opaque asset-definition metadata removes should defer to state"),
        None
    );
    assert_eq!(
        router
            .try_route_with_view(&tx, &state.view())
            .expect("opaque asset-definition metadata remove should route to the resolved dataspace"),
        RoutingDecision::new(lane_id, dataspace_id)
    );
}

#[test]
fn opaque_global_asset_definition_unregister_uses_stored_alias_dataspace() {
    let (submitter_id, submitter_keypair) = gen_account_in("wonderland");
    let dataspace_id = DataSpaceId::new(10);
    let lane_id = LaneId::new(2);
    let catalog = dataspace_catalog(&[(dataspace_id, "paynet")]);
    let lane_catalog = catalog_with_lane_dataspaces(&[
        (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
        (lane_id, dataspace_id),
    ]);
    let router = ConfigLaneRouter::new(default_routing_policy(), catalog.clone(), lane_catalog);
    let transparent_asset_definition = AssetDefinitionId::derive_from_components(
        DomainId::try_new("cash", "universal").expect("domain id"),
        "pkr".parse().expect("asset definition name"),
    );
    let opaque_asset_definition =
        AssetDefinitionId::parse_address_literal(&transparent_asset_definition.canonical_address())
            .expect("opaque canonical asset definition id");
    let tx = sample_transaction(
        &submitter_id,
        submitter_keypair.private_key(),
        vec![InstructionBox::from(Unregister::asset_definition(
            opaque_asset_definition.clone(),
        ))],
    );
    let alias: AssetDefinitionAlias = "pkr#paynet".parse().expect("asset alias");
    let mut state = state_with_asset_definitions(
        vec![
            AssetDefinition::numeric(
                opaque_asset_definition.clone(),
                "pkr".to_owned(),
                AssetBalancePolicy::Global,
                None,
            )
            .build(&submitter_id),
        ],
        catalog,
        router.lane_catalog.as_ref().clone(),
    );
    state
        .world
        .asset_definition_aliases
        .insert(alias.clone(), opaque_asset_definition.clone());
    state.world.asset_definition_alias_bindings.insert(
        opaque_asset_definition,
        crate::state::AssetDefinitionAliasBindingRecord {
            alias,
            lease_expiry_ms: None,
            grace_until_ms: None,
            bound_at_ms: 0,
        },
    );
    assert_eq!(
        router
            .try_route_without_state(&tx)
            .expect("opaque global unregister should defer to state"),
        None
    );
    assert_eq!(
        router
            .try_route_with_view(&tx, &state.view())
            .expect("opaque global unregister should route through the stored alias home"),
        RoutingDecision::new(lane_id, dataspace_id)
    );
}
