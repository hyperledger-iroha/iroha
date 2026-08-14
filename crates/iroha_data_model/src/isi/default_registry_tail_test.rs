#[test]
fn default_registry_roundtrip_more_instructions() {
    // Expand coverage across instruction families and variants
    let _guard = RegistryGuard::set(crate::instruction_registry::default());
    let local_registry = crate::instruction_registry::default();
    // Common fixtures
    let domain_id: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
    let fixture_account = |seed: u8| {
        let (public_key, _) =
            iroha_crypto::KeyPair::try_from_seed(vec![seed; 32], iroha_crypto::Algorithm::Ed25519)
                .expect("fixture seed derives a checked Ed25519 keypair")
                .into_parts();
        AccountId::new(public_key)
    };
    let account_a = fixture_account(0xAA);
    let account_b = fixture_account(0xBB);
    let asset_def_id: AssetDefinitionId =
        iroha_data_model::asset::AssetDefinitionId::derive_from_components(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "coin".parse().unwrap(),
        );
    let asset_id = AssetId::of(asset_def_id.clone(), account_a.clone());
    let nft_id: NftId = "n0$wonderland".parse().unwrap();
    let role_id: RoleId = "auditor".parse().unwrap();
    let key: Name = "k".parse().unwrap();
    let trig_id: TriggerId = "nightly_tick".parse().unwrap();
    // Permission token
    let perm = Permission::new("mint".parse().unwrap(), Json::new(()));
    // Upgrade executor placeholder
    let exec = crate::executor::Executor::new(
        crate::transaction::executable::IvmBytecode::from_compiled(vec![1, 2, 3]),
    );
    let cases: Vec<InstructionBox> = vec![
        // SetKeyValue and RemoveKeyValue across all owners
        SetKeyValue::account(account_a.clone(), key.clone(), Json::new(1u32)).into(),
        SetKeyValue::asset_definition(asset_def_id.clone(), key.clone(), Json::new(2u32)).into(),
        SetKeyValue::nft(nft_id.clone(), key.clone(), Json::new(3u32)).into(),
        SetKeyValue::trigger(trig_id.clone(), key.clone(), Json::new(4u32)).into(),
        RemoveKeyValue::account(account_a.clone(), key.clone()).into(),
        RemoveKeyValue::asset_definition(asset_def_id.clone(), key.clone()).into(),
        RemoveKeyValue::nft(nft_id.clone(), key.clone()).into(),
        RemoveKeyValue::trigger(trig_id.clone(), key.clone()).into(),
        // Transfers for all variants
        Transfer::domain(account_a.clone(), domain_id.clone(), account_b.clone()).into(),
        Transfer::asset_definition(account_a.clone(), asset_def_id.clone(), account_b.clone())
            .into(),
        Transfer::asset_quantity(asset_id.clone(), 7_u32, account_b.clone()).into(),
        Transfer::nft(account_a.clone(), nft_id.clone(), account_b.clone()).into(),
        // Grants and revokes for permission and role targets
        Grant::account_permission(perm.clone(), account_a.clone()).into(),
        Grant::role_permission(perm.clone(), role_id.clone()).into(),
        Revoke::account_permission(perm.clone(), account_a.clone()).into(),
        Revoke::role_permission(perm.clone(), role_id.clone()).into(),
        // ExecuteTrigger, Upgrade, CustomInstruction
        ExecuteTrigger::new(trig_id.clone())
            .with_args(norito::json!({"a": 1u32}))
            .into(),
        Upgrade::new(exec).into(),
        // Use an explicit empty JSON payload since `Json` does not implement
        // `From<()>`.
        CustomInstruction::new(Json::new(())).into(),
    ];
    for instr in cases {
        let bytes = norito::to_bytes(&instr).expect("encode");
        let (name, payload) =
            norito::decode_from_bytes::<(String, Vec<u8>)>(&bytes).expect("extract");
        let decoded = local_registry
            .decode(&name, &payload)
            .unwrap_or_else(|| panic!("instruction `{name}` is not registered"))
            .expect("decode via registry");
        assert_eq!(instr, decoded);
    }
}
