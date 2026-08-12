// Included from `asset::isi::tests` to keep this policy regression in its original scope.

#[test]
fn transfer_restricted_asset_rejects_ambiguous_source_dataspace_binding() {
    let domain_id: DomainId = DomainId::try_new("wonderland", "universal").expect("domain id");
    let first_source_dataspace = DataSpaceId::new(7);
    let second_source_dataspace = DataSpaceId::new(8);
    let destination_dataspace = DataSpaceId::new(11);
    let uaid_alice = iroha_data_model::nexus::UniversalAccountId::from_hash(
        iroha_crypto::Hash::new(b"uaid::alice-ambiguous-source"),
    );
    let uaid_bob = iroha_data_model::nexus::UniversalAccountId::from_hash(iroha_crypto::Hash::new(
        b"uaid::bob-ambiguous-source",
    ));

    let domain = Domain::new(domain_id.clone()).build(&ALICE_ID);
    let alice_account = NewAccount::new(ALICE_ID.clone())
        .with_uaid(Some(uaid_alice))
        .build(&ALICE_ID);
    let bob_account = NewAccount::new(BOB_ID.clone())
        .with_uaid(Some(uaid_bob))
        .build(&BOB_ID);

    let asset_def_id: AssetDefinitionId =
        iroha_data_model::asset::AssetDefinitionId::derive_from_components(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "rose".parse().unwrap(),
        );
    let asset_def = AssetDefinition::numeric(
        asset_def_id.clone(),
        "rose".to_owned(),
        iroha_data_model::asset::AssetBalancePolicy::DataspaceRestricted,
        Some(domain_id.clone()),
    )
    .build(&ALICE_ID);
    let source_asset_id = AssetId::with_scope(
        asset_def_id.clone(),
        ALICE_ID.clone(),
        iroha_data_model::asset::AssetBalanceScope::Dataspace(first_source_dataspace),
    );
    let source_asset = Asset::new(source_asset_id.clone(), Quantity::from(10_u32));

    let mut world = World::with_assets(
        [domain],
        [alice_account, bob_account],
        [asset_def],
        [source_asset],
        [],
    );
    world.uaid_accounts.insert(uaid_alice, ALICE_ID.clone());
    world.uaid_accounts.insert(uaid_bob, BOB_ID.clone());

    let mut alice_bindings = crate::nexus::space_directory::UaidDataspaceBindings::default();
    alice_bindings.bind_account(first_source_dataspace, ALICE_ID.clone());
    alice_bindings.bind_account(second_source_dataspace, ALICE_ID.clone());
    let mut bob_bindings = crate::nexus::space_directory::UaidDataspaceBindings::default();
    bob_bindings.bind_account(destination_dataspace, BOB_ID.clone());

    let kura = Kura::blank_kura_for_testing();
    let query_store = LiveQueryStore::start_test();
    let mut state = State::new(world, kura, query_store);
    state
        .world
        .uaid_dataspaces
        .insert(uaid_alice, alice_bindings);
    state.world.uaid_dataspaces.insert(uaid_bob, bob_bindings);

    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();
    stx.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
    stx.world.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
    seed_test_call_hash(&mut stx, 0xB5);

    let err = Transfer::asset_quantity(
        AssetId::new(asset_def_id.clone(), ALICE_ID.clone()),
        1_u32,
        BOB_ID.clone(),
    )
    .execute(&ALICE_ID, &mut stx)
    .expect_err("ambiguous source binding must not pick a dataspace");
    match err {
        InstructionExecutionError::InvariantViolation(message) => {
            assert!(
                message.contains("bound to multiple dataspaces"),
                "{message}"
            )
        }
        other => panic!("unexpected error: {other:?}"),
    }

    assert_eq!(
        stx.world
            .asset(&source_asset_id)
            .expect("source balance must remain untouched")
            .value()
            .clone()
            .into_inner(),
        Quantity::from(10_u32)
    );
    let destination_asset_id = AssetId::with_scope(
        asset_def_id,
        BOB_ID.clone(),
        iroha_data_model::asset::AssetBalanceScope::Dataspace(destination_dataspace),
    );
    assert!(
        stx.world.asset(&destination_asset_id).is_err(),
        "ambiguous source transfer must not materialize a destination balance"
    );
}
