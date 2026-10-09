#[test]
fn register_restricted_asset_definition_cannot_infer_home_from_alias() {
    let state = test_state();
    let authority = (*ALICE_ID).clone();
    let paynet = DataSpaceId::new(7);
    let definition_id = AssetDefinitionId::from_uuid_bytes([
        0x8a, 0xb5, 0xec, 0x8c, 0x32, 0xdf, 0x46, 0xcf, 0x87, 0xca, 0x3e, 0xd9, 0xce, 0x36, 0xa8,
        0x19,
    ])
    .expect("opaque asset definition id");
    let alias: AssetDefinitionAlias = "unit#paynet".parse().expect("dataspace-root alias");
    let definition = AssetDefinition::numeric(
        definition_id.clone(),
        "unit".to_owned(),
        AssetBalancePolicy::DataspaceRestricted,
        None,
    )
    .with_alias(Some(alias.clone()));
    let header = BlockHeader::new(nonzero!(1_u64), None, None, 0, 0);
    let mut block = state.block(header);
    let mut tx = block.transaction();
    install_dataspace_catalog_with_lane(&mut tx, paynet, "paynet", LaneVisibility::Restricted);
    tx.current_dataspace_id = Some(paynet);
    tx.world.current_dataspace_id = Some(paynet);
    let error = Register::asset_definition(definition)
        .execute(&authority, &mut tx)
        .expect_err("a namespace alias is not immutable home authority");
    assert!(
        error
            .to_string()
            .contains("a restricted dataspace namespace cannot alias another dataspace"),
        "unexpected error: {error}"
    );
    assert!(tx.world.asset_definitions.get(&definition_id).is_none());
    assert!(tx.world.asset_definition_aliases.get(&alias).is_none());
    assert!(
        tx.world
            .asset_definition_direct_homes
            .get(&definition_id)
            .is_none()
    );
    assert!(
        tx.world
            .asset_definition_domains
            .get(&definition_id)
            .is_none()
    );
}
