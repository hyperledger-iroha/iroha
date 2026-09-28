#[test]
fn genesis_domain_builder_assets_retain_registered_owning_domains() -> Result<()> {
    use iroha_data_model::isi::register::RegisterBox;

    init_instruction_registry();
    let domains = [
        DomainId::try_new("wonderland", "universal")?,
        DomainId::try_new("garden", "universal")?,
    ];
    let mut builder =
        GenesisBuilder::new_without_executor(ChainId::from("genesis-domain-asset-ownership"), ".");
    for domain in &domains {
        builder = builder
            .domain(domain.clone())
            .asset("coin".parse()?, NumericSpec::fractional(2))
            .finish_domain();
    }
    let manifest = builder.build_raw_for_test();
    let json = norito::json::to_json(&manifest)?;
    let decoded: RawGenesisTransaction = norito::json::from_str(&json)?;
    let mut registered_domains = std::collections::BTreeSet::new();
    let mut registered_assets = std::collections::BTreeSet::new();
    for instruction in decoded.instructions() {
        match instruction.as_any().downcast_ref::<RegisterBox>() {
            Some(RegisterBox::Domain(register)) => {
                registered_domains.insert(register.object.id.clone());
            }
            Some(RegisterBox::AssetDefinition(register)) => {
                let definition = &register.object;
                let domain = definition
                    .owning_domain
                    .as_ref()
                    .expect("domain-builder assets require explicit ownership");
                assert!(
                    registered_domains.contains(domain),
                    "owning domain must be registered first"
                );
                assert_eq!(
                    definition.id,
                    AssetDefinitionId::derive_from_components(domain.clone(), "coin".parse()?),
                );
                assert_eq!(definition.spec, NumericSpec::fractional(2));
                assert_eq!(
                    definition.balance_scope_policy,
                    iroha_data_model::asset::AssetBalancePolicy::Global,
                );
                registered_assets.insert(definition.id.clone());
            }
            _ => {}
        }
    }
    assert_eq!(registered_domains, domains.into_iter().collect());
    assert_eq!(registered_assets.len(), 2);
    Ok(())
}

#[test]
#[allow(clippy::too_many_lines)]
fn genesis_block_builder_example() -> Result<()> {
    let public_key: std::collections::HashMap<&'static str, PublicKey> = [
        ("alice", ALICE_KEYPAIR.public_key().clone()),
        ("bob", BOB_KEYPAIR.public_key().clone()),
        (
            "cheshire_cat",
            checked_genesis_fixture_keypair().into_parts().0,
        ),
        (
            "mad_hatter",
            checked_genesis_fixture_keypair().into_parts().0,
        ),
    ]
    .into_iter()
    .collect();
    let (_tmp_dir, mut genesis_builder) = test_builder();
    let _executor_path = genesis_builder.executor.clone();
    genesis_builder = genesis_builder
        .domain(DomainId::try_new("wonderland", "universal").unwrap())
        .account(public_key["alice"].clone())
        .account(public_key["bob"].clone())
        .finish_domain()
        .domain(DomainId::try_new("tulgey_wood", "universal").unwrap())
        .account(public_key["cheshire_cat"].clone())
        .finish_domain()
        .domain(DomainId::try_new("meadow", "universal").unwrap())
        .account(public_key["mad_hatter"].clone())
        .asset("hats".parse().unwrap(), NumericSpec::default())
        .finish_domain();
    // In real cases executor should be constructed from an IVM bytecode blob
    let finished_genesis = genesis_builder
        .set_topology(deterministic_test_genesis_topology_entries())
        .build_and_sign(&checked_genesis_fixture_keypair())?;
    let transactions = &finished_genesis
        .0
        .external_transactions()
        .collect::<Vec<_>>();
    // First transaction
    {
        let transaction = transactions[0];
        let instructions = transaction.instructions();
        let Executable::Instructions(instructions) = instructions else {
            panic!("Expected instructions");
        };
        assert_eq!(instructions.len(), 1);
    }
    // Second transaction
    let transaction = transactions[1];
    let instructions = transaction.instructions();
    let Executable::Instructions(instructions) = instructions else {
        panic!("Expected instructions");
    };
    let parameter_count = instructions
        .iter()
        .take_while(|instruction| instruction.as_any().is::<SetParameter>())
        .count();
    assert!(
        parameter_count > 0,
        "genesis must install its parameter snapshot"
    );
    let registrations = &instructions[parameter_count..];
    assert_eq!(registrations.len(), 8 + 4);
    let (instructions, topology) = registrations.split_at(8);
    let expected_topology: Vec<InstructionBox> = deterministic_test_genesis_topology_entries()
        .into_iter()
        .map(|entry| {
            RegisterPeerWithPop::new(entry.peer.clone(), entry.pop_bytes().unwrap().unwrap()).into()
        })
        .collect();
    assert_eq!(topology, expected_topology);
    {
        let domain_id: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
        assert_eq!(
            instructions[0],
            Register::domain(Domain::new(domain_id.clone())).into()
        );
        assert_eq!(
            instructions[1],
            Register::account(Account::new(
                AccountId::new(public_key["alice"].clone()).clone()
            ))
            .into()
        );
        assert_eq!(
            instructions[2],
            Register::account(Account::new(
                AccountId::new(public_key["bob"].clone()).clone()
            ))
            .into()
        );
    }
    {
        let domain_id: DomainId = DomainId::try_new("tulgey_wood", "universal").unwrap();
        assert_eq!(
            instructions[3],
            Register::domain(Domain::new(domain_id.clone())).into()
        );
        assert_eq!(
            instructions[4],
            Register::account(Account::new(
                AccountId::new(public_key["cheshire_cat"].clone()).clone()
            ))
            .into()
        );
    }
    {
        let domain_id: DomainId = DomainId::try_new("meadow", "universal").unwrap();
        assert_eq!(
            instructions[5],
            Register::domain(Domain::new(domain_id.clone())).into()
        );
        assert_eq!(
            instructions[6],
            Register::account(Account::new(
                AccountId::new(public_key["mad_hatter"].clone()).clone()
            ))
            .into()
        );
        assert_eq!(
            instructions[7],
            Register::asset_definition(AssetDefinition::numeric(
                iroha_data_model::asset::AssetDefinitionId::derive_from_components(
                    DomainId::try_new("meadow", "universal").unwrap(),
                    "hats".parse().unwrap(),
                ),
                "hats".to_owned(),
                iroha_data_model::asset::AssetBalancePolicy::Global,
                Some(domain_id),
            ))
            .into()
        );
    }
    Ok(())
}
