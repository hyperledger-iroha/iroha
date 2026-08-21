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
    let finished_genesis = genesis_builder.build_and_sign(&checked_genesis_fixture_keypair())?;
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
                None,
            ))
            .into()
        );
    }
    Ok(())
}
