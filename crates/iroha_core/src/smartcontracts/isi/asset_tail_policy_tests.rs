#[test]
fn transfer_rejects_when_issuer_policy_requires_binding_for_destination() {
    let domain_id: DomainId = DomainId::try_new("wonderland", "universal").expect("domain id");
    let domain = Domain::new(domain_id.clone()).build(&ALICE_ID);
    let alice_account = build_account_in_domain(&ALICE_ID, &domain_id);
    let bob_account = build_account_in_domain(&BOB_ID, &domain_id);
    let asset_def_id: AssetDefinitionId =
        iroha_data_model::asset::AssetDefinitionId::derive_from_components(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "rose".parse().unwrap(),
        );
    let mut asset_def = AssetDefinition::numeric(
        asset_def_id.clone(),
        "rose".to_owned(),
        iroha_data_model::asset::AssetBalancePolicy::Global,
        None,
    )
    .build(&ALICE_ID);
    let issuer_policy = AssetIssuerUsagePolicyV1 {
        require_subject_binding: true,
        subject_bindings: BTreeMap::from([(ALICE_ID.clone(), AssetSubjectBindingV1::default())]),
    };
    asset_def.metadata_mut().insert(
        ASSET_ISSUER_USAGE_POLICY_METADATA_KEY
            .parse()
            .expect("metadata key"),
        Json::new(issuer_policy),
    );
    let source_asset_id = AssetId::new(asset_def_id.clone(), ALICE_ID.clone());
    let source_asset = Asset::new(source_asset_id.clone(), Quantity::from(10_u32));
    let world = World::with_assets(
        [domain],
        [alice_account, bob_account],
        [asset_def],
        [source_asset],
        [],
    );
    let kura = Kura::blank_kura_for_testing();
    let query_store = LiveQueryStore::start_test();
    let state = State::new(world, kura, query_store);
    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();
    let err = Transfer::asset_quantity(source_asset_id, 1_u32, BOB_ID.clone())
        .execute(&ALICE_ID, &mut stx)
        .expect_err("unbound destination must be rejected");
    assert!(
        err.to_string()
            .contains("requires explicit subject binding"),
        "unexpected error: {err}"
    );
}
#[test]
fn transfer_accepts_any_matching_allowed_domain_membership() {
    let denied_domain_id: DomainId =
        DomainId::try_new("wonderland", "universal").expect("domain id");
    let allowed_domain_id: DomainId = DomainId::try_new("oasis", "universal").expect("domain id");
    let asset_def_id: AssetDefinitionId =
        iroha_data_model::asset::AssetDefinitionId::derive_from_components(
            denied_domain_id.clone(),
            "rose".parse().unwrap(),
        );
    let mut denied_domain_metadata = Metadata::default();
    denied_domain_metadata.insert(
        DOMAIN_ASSET_USAGE_POLICY_METADATA_KEY
            .parse()
            .expect("metadata key"),
        Json::new(DomainAssetUsagePolicyV1 {
            allowed_assets: BTreeSet::new(),
            denied_assets: BTreeSet::from([asset_def_id.clone()]),
        }),
    );
    let denied_domain = Domain::new(denied_domain_id.clone())
        .with_metadata(denied_domain_metadata)
        .build(&ALICE_ID);
    let allowed_domain = Domain::new(allowed_domain_id.clone()).build(&ALICE_ID);
    let allowed_dataspace_id = DataSpaceId::UNIVERSAL;
    let alice_alias = AccountAlias::new(
        "alice".parse().expect("account alias label"),
        Some(AccountAliasDomain::new(allowed_domain_id.name().clone())),
        allowed_dataspace_id,
    );
    let alice_account = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
    let bob_alias = AccountAlias::new(
        "bob".parse().expect("account alias label"),
        Some(AccountAliasDomain::new(allowed_domain_id.name().clone())),
        allowed_dataspace_id,
    );
    let bob_account = Account::new(BOB_ID.clone()).build(&BOB_ID);
    let mut asset_def = AssetDefinition::numeric(
        asset_def_id.clone(),
        "rose".to_owned(),
        iroha_data_model::asset::AssetBalancePolicy::Global,
        None,
    )
    .build(&ALICE_ID);
    let binding = AssetSubjectBindingV1 {
        allowed_domains: BTreeSet::from([denied_domain_id.clone(), allowed_domain_id.clone()]),
        allowed_dataspaces: BTreeSet::new(),
    };
    let issuer_policy = AssetIssuerUsagePolicyV1 {
        require_subject_binding: true,
        subject_bindings: BTreeMap::from([
            (ALICE_ID.clone(), binding.clone()),
            (BOB_ID.clone(), binding),
        ]),
    };
    asset_def.metadata_mut().insert(
        ASSET_ISSUER_USAGE_POLICY_METADATA_KEY
            .parse()
            .expect("metadata key"),
        Json::new(issuer_policy),
    );
    let source_asset_id = AssetId::new(asset_def_id.clone(), ALICE_ID.clone());
    let source_asset = Asset::new(source_asset_id.clone(), Quantity::from(10_u32));
    let world = World::with_assets(
        [denied_domain, allowed_domain],
        [alice_account, bob_account],
        [asset_def],
        [source_asset],
        [],
    );
    let kura = Kura::blank_kura_for_testing();
    let query_store = LiveQueryStore::start_test();
    let state = State::new(world, kura, query_store);
    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();
    seed_test_call_hash(&mut stx, 0xB4);
    seed_test_account_alias_binding(&mut stx, &ALICE_ID, &alice_alias);
    seed_test_account_alias_binding(&mut stx, &BOB_ID, &bob_alias);
    seed_test_account_alias_lease(&mut stx, &ALICE_ID, &alice_alias);
    seed_test_account_alias_lease(&mut stx, &BOB_ID, &bob_alias);
    Transfer::asset_quantity(source_asset_id, 1_u32, BOB_ID.clone())
        .execute(&ALICE_ID, &mut stx)
        .expect("one matching allowed domain membership should authorize transfer");
    let destination_asset_id = AssetId::new(asset_def_id, BOB_ID.clone());
    assert_eq!(
        stx.world
            .asset(&destination_asset_id)
            .expect("destination asset created")
            .value()
            .clone()
            .into_inner(),
        Quantity::from(1_u32)
    );
}
#[test]
fn transfer_rejects_when_bound_domain_policy_denies_asset() {
    let domain_id: DomainId = DomainId::try_new("wonderland", "universal").expect("domain id");
    let asset_def_id: AssetDefinitionId =
        iroha_data_model::asset::AssetDefinitionId::derive_from_components(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "rose".parse().unwrap(),
        );
    let mut domain_metadata = Metadata::default();
    let domain_policy = DomainAssetUsagePolicyV1 {
        allowed_assets: BTreeSet::new(),
        denied_assets: BTreeSet::from([asset_def_id.clone()]),
    };
    domain_metadata.insert(
        DOMAIN_ASSET_USAGE_POLICY_METADATA_KEY
            .parse()
            .expect("metadata key"),
        Json::new(domain_policy),
    );
    let domain = Domain::new(domain_id.clone())
        .with_metadata(domain_metadata)
        .build(&ALICE_ID);
    let domain_dataspace_id = DataSpaceId::UNIVERSAL;
    let alice_alias = AccountAlias::new(
        "alice".parse().expect("account alias label"),
        Some(AccountAliasDomain::new(domain_id.name().clone())),
        domain_dataspace_id,
    );
    let alice_account = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
    let bob_alias = AccountAlias::new(
        "bob".parse().expect("account alias label"),
        Some(AccountAliasDomain::new(domain_id.name().clone())),
        domain_dataspace_id,
    );
    let bob_account = Account::new(BOB_ID.clone()).build(&BOB_ID);
    let mut asset_def = AssetDefinition::numeric(
        asset_def_id.clone(),
        "rose".to_owned(),
        iroha_data_model::asset::AssetBalancePolicy::Global,
        None,
    )
    .build(&ALICE_ID);
    let binding = AssetSubjectBindingV1 {
        allowed_domains: BTreeSet::from([domain_id.clone()]),
        allowed_dataspaces: BTreeSet::new(),
    };
    let issuer_policy = AssetIssuerUsagePolicyV1 {
        require_subject_binding: true,
        subject_bindings: BTreeMap::from([
            (ALICE_ID.clone(), binding.clone()),
            (BOB_ID.clone(), binding),
        ]),
    };
    asset_def.metadata_mut().insert(
        ASSET_ISSUER_USAGE_POLICY_METADATA_KEY
            .parse()
            .expect("metadata key"),
        Json::new(issuer_policy),
    );
    let source_asset_id = AssetId::new(asset_def_id.clone(), ALICE_ID.clone());
    let source_asset = Asset::new(source_asset_id.clone(), Quantity::from(10_u32));
    let world = World::with_assets(
        [domain],
        [alice_account, bob_account],
        [asset_def],
        [source_asset],
        [],
    );
    let kura = Kura::blank_kura_for_testing();
    let query_store = LiveQueryStore::start_test();
    let state = State::new(world, kura, query_store);
    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();
    seed_test_account_alias_binding(&mut stx, &ALICE_ID, &alice_alias);
    seed_test_account_alias_binding(&mut stx, &BOB_ID, &bob_alias);
    seed_test_account_alias_lease(&mut stx, &ALICE_ID, &alice_alias);
    seed_test_account_alias_lease(&mut stx, &BOB_ID, &bob_alias);
    let err = Transfer::asset_quantity(source_asset_id, 1_u32, BOB_ID.clone())
        .execute(&ALICE_ID, &mut stx)
        .expect_err("domain deny policy must reject transfer");
    assert!(
        err.to_string().contains("domain policy"),
        "unexpected error: {err}"
    );
}
#[test]
fn transfer_rejects_when_dataspace_manifest_denies_bound_asset() {
    let domain_id: DomainId = DomainId::try_new("wonderland", "universal").expect("domain id");
    let dsid = DataSpaceId::new(7);
    let uaid_alice = iroha_data_model::nexus::UniversalAccountId::from_hash(
        iroha_crypto::Hash::new(b"uaid:alice"),
    );
    let uaid_bob = iroha_data_model::nexus::UniversalAccountId::from_hash(iroha_crypto::Hash::new(
        b"uaid:bob",
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
    let mut asset_def = AssetDefinition::numeric(
        asset_def_id.clone(),
        "rose".to_owned(),
        iroha_data_model::asset::AssetBalancePolicy::DataspaceRestricted,
        Some(domain_id.clone()),
    )
    .build(&ALICE_ID);
    let binding = AssetSubjectBindingV1 {
        allowed_domains: BTreeSet::new(),
        allowed_dataspaces: BTreeSet::from([dsid]),
    };
    let issuer_policy = AssetIssuerUsagePolicyV1 {
        require_subject_binding: true,
        subject_bindings: BTreeMap::from([
            (ALICE_ID.clone(), binding.clone()),
            (BOB_ID.clone(), binding),
        ]),
    };
    asset_def.metadata_mut().insert(
        ASSET_ISSUER_USAGE_POLICY_METADATA_KEY
            .parse()
            .expect("metadata key"),
        Json::new(issuer_policy),
    );
    let source_asset_id = AssetId::with_scope(
        asset_def_id.clone(),
        ALICE_ID.clone(),
        iroha_data_model::asset::AssetBalanceScope::Dataspace(dsid),
    );
    let source_asset = Asset::new(source_asset_id.clone(), Quantity::from(10_u32));
    let world = World::with_assets(
        [domain],
        [alice_account, bob_account],
        [asset_def],
        [source_asset],
        [],
    );
    let kura = Kura::blank_kura_for_testing();
    let query_store = LiveQueryStore::start_test();
    let state = State::new(world, kura, query_store);
    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();
    stx.current_dataspace_id = Some(dsid);
    stx.world.current_dataspace_id = Some(dsid);
    let mut alice_manifest_record =
        crate::nexus::space_directory::SpaceDirectoryManifestRecord::new(AssetPermissionManifest {
            version: iroha_data_model::nexus::ManifestVersion::default(),
            uaid: uaid_alice,
            dataspace: dsid,
            issued_ms: 1,
            activation_epoch: 0,
            expiry_epoch: None,
            entries: Vec::new(),
        });
    alice_manifest_record.lifecycle.mark_activated(0);
    let mut bob_manifest_record =
        crate::nexus::space_directory::SpaceDirectoryManifestRecord::new(AssetPermissionManifest {
            version: iroha_data_model::nexus::ManifestVersion::default(),
            uaid: uaid_bob,
            dataspace: dsid,
            issued_ms: 1,
            activation_epoch: 0,
            expiry_epoch: None,
            entries: Vec::new(),
        });
    bob_manifest_record.lifecycle.mark_activated(0);
    let mut alice_set = crate::nexus::space_directory::SpaceDirectoryManifestSet::default();
    alice_set.upsert(alice_manifest_record);
    let mut bob_set = crate::nexus::space_directory::SpaceDirectoryManifestSet::default();
    bob_set.upsert(bob_manifest_record);
    stx.world
        .space_directory_manifests
        .insert(uaid_alice, alice_set);
    stx.world
        .space_directory_manifests
        .insert(uaid_bob, bob_set);
    let err = Transfer::asset_quantity(source_asset_id, 1_u32, BOB_ID.clone())
        .execute(&ALICE_ID, &mut stx)
        .expect_err("manifest without matching allow should deny");
    assert!(
        err.to_string().contains("dataspace policy denied"),
        "unexpected error: {err}"
    );
}
