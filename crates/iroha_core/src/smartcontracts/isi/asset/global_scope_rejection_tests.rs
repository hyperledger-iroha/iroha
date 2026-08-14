// Same-scope regression coverage extracted to keep the parent source budget bounded.
#[test]
fn mint_global_asset_rejects_explicit_dataspace_scope_on_universal_route() {
    let domain_id: DomainId = DomainId::try_new("wonderland", "universal").expect("domain id");
    let domain = Domain::new(domain_id.clone()).build(&ALICE_ID);
    let account = build_account_in_domain(&ALICE_ID, &domain_id);
    let asset_def_id: AssetDefinitionId =
        iroha_data_model::asset::AssetDefinitionId::derive_from_components(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "xor".parse().unwrap(),
        );
    let asset_def = AssetDefinition::numeric(
        asset_def_id.clone(),
        "xor".to_owned(),
        iroha_data_model::asset::AssetBalancePolicy::Global,
        None,
    )
    .build(&ALICE_ID);
    let world = World::with([domain], [account], [asset_def]);
    let kura = Kura::blank_kura_for_testing();
    let query_store = LiveQueryStore::start_test();
    let state = State::new(world, kura, query_store);
    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();
    stx.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
    stx.world.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
    let scoped_id = AssetId::with_scope(
        asset_def_id,
        ALICE_ID.clone(),
        iroha_data_model::asset::AssetBalanceScope::Dataspace(DataSpaceId::new(7)),
    );
    let err = Mint::asset_quantity(5_u32, scoped_id)
        .execute(&ALICE_ID, &mut stx)
        .expect_err("global assets must reject explicit dataspace-scoped ids");
    match err {
        InstructionExecutionError::InvariantViolation(message) => {
            assert!(
                message.contains("global assets cannot be addressed with dataspace scope"),
                "unexpected invariant message: {message}"
            );
        }
        other => panic!("unexpected error: {other:?}"),
    }
}
#[test]
fn burn_global_asset_rejects_explicit_dataspace_scope_on_universal_route() {
    let domain_id: DomainId = DomainId::try_new("wonderland", "universal").expect("domain id");
    let domain = Domain::new(domain_id.clone()).build(&ALICE_ID);
    let account = build_account_in_domain(&ALICE_ID, &domain_id);
    let asset_def_id: AssetDefinitionId =
        iroha_data_model::asset::AssetDefinitionId::derive_from_components(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "xor".parse().unwrap(),
        );
    let asset_def = AssetDefinition::numeric(
        asset_def_id.clone(),
        "xor".to_owned(),
        iroha_data_model::asset::AssetBalancePolicy::Global,
        None,
    )
    .build(&ALICE_ID);
    let world = World::with([domain], [account], [asset_def]);
    let kura = Kura::blank_kura_for_testing();
    let query_store = LiveQueryStore::start_test();
    let state = State::new(world, kura, query_store);
    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();
    stx.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
    stx.world.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
    let scoped_id = AssetId::with_scope(
        asset_def_id,
        ALICE_ID.clone(),
        iroha_data_model::asset::AssetBalanceScope::Dataspace(DataSpaceId::new(7)),
    );
    let err = Burn::asset_quantity(5_u32, scoped_id)
        .execute(&ALICE_ID, &mut stx)
        .expect_err("global assets must reject explicit dataspace-scoped ids");
    match err {
        InstructionExecutionError::InvariantViolation(message) => {
            assert!(
                message.contains("global assets cannot be addressed with dataspace scope"),
                "unexpected invariant message: {message}"
            );
        }
        other => panic!("unexpected error: {other:?}"),
    }
}
