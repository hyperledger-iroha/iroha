#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Tests for sorting and pagination queries

use std::collections::HashSet;

use eyre::{Result, WrapErr as _};
use integration_tests::sandbox;
use iroha::{
    client::Client,
    crypto::KeyPair,
    data_model::{account::Account, name::Name, prelude::*, query::parameters::SortOrder},
};
use iroha_test_network::*;
use iroha_test_samples::ALICE_ID;
use nonzero_ext::nonzero;
use rand::{SeedableRng, seq::SliceRandom};
use rand_chacha::ChaCha8Rng;
use tokio::runtime::Runtime;

fn start_network(
    builder: NetworkBuilder,
    context: &'static str,
) -> Option<(sandbox::SerializedNetwork, Runtime)> {
    sandbox::start_network_blocking_or_skip(builder, context).unwrap()
}

#[test]
#[allow(clippy::cast_possible_truncation)]
fn correct_pagination_assets_after_creating_new_one() {
    const N_ASSETS: usize = 12;
    if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {
        eprintln!(
            "Skipping: pagination test gated by issue #4786. Set IROHA_RUN_IGNORED=1 to run."
        );
        return;
    }
    // 0 < pagination.start < missing_idx < pagination.end < N_ASSETS
    let missing_indices = vec![N_ASSETS / 2];
    let pagination = Pagination::new(Some(nonzero!(N_ASSETS as u64 / 3)), N_ASSETS as u64 / 3);
    let xor_filter = CompoundPredicate::<Asset>::PASS; // lightweight DSL stub

    let sort_by_metadata_key = "sort".parse::<Name>().expect("Valid");
    let sorting = Sorting::by_metadata_key(sort_by_metadata_key.clone());
    let account_id = ALICE_ID.clone();

    let builder = NetworkBuilder::new();
    let Some((network, _rt)) = start_network(
        builder,
        stringify!(correct_pagination_assets_after_creating_new_one),
    ) else {
        return;
    };
    let test_client = network.client();

    let mut tester_assets = vec![];
    let mut register_asset_definitions = vec![];
    let mut register_assets = vec![];

    let mut missing_tester_assets = vec![];
    let mut missing_register_asset_definitions = vec![];
    let mut missing_register_assets = vec![];

    for i in 0..N_ASSETS {
        let asset_definition_id = format!("xor{i}#wonderland")
            .parse::<AssetDefinitionId>()
            .expect("Valid");
        let asset_definition = AssetDefinition::numeric(asset_definition_id.clone());
        let asset = Asset::new(AssetId::new(asset_definition_id, account_id.clone()), 1u32);

        if missing_indices.contains(&i) {
            missing_tester_assets.push(asset.clone());
            missing_register_asset_definitions.push(Register::asset_definition(asset_definition));
            missing_register_assets.push(Mint::asset_numeric(1u32, asset.id().clone()));
        } else {
            tester_assets.push(asset.clone());
            register_asset_definitions.push(Register::asset_definition(asset_definition));
            register_assets.push(Mint::asset_numeric(1u32, asset.id().clone()));
        }
    }
    let mut rng = ChaCha8Rng::seed_from_u64(0x534f_5254);
    register_asset_definitions.shuffle(&mut rng);
    register_assets.shuffle(&mut rng);

    submit_chunked(&test_client, &register_asset_definitions).expect("Valid");
    submit_chunked(&test_client, &register_assets).expect("Valid");

    let queried_assets = test_client
        .query(FindAssets::new())
        .filter(xor_filter.clone())
        .with_pagination(pagination)
        .with_sorting(sorting.clone())
        .execute_all()
        .expect("Valid");

    tester_assets
        .iter()
        .skip(N_ASSETS / 3)
        .take(N_ASSETS / 3)
        .zip(queried_assets)
        .for_each(|(tester, queried)| assert_eq!(*tester, queried));

    for (i, missing_idx) in missing_indices.into_iter().enumerate() {
        tester_assets.insert(missing_idx, missing_tester_assets[i].clone());
    }
    submit_chunked(&test_client, &missing_register_asset_definitions).expect("Valid");
    submit_chunked(&test_client, &missing_register_assets).expect("Valid");

    let queried_assets = test_client
        .query(FindAssets::new())
        .filter(xor_filter)
        .with_pagination(pagination)
        .with_sorting(sorting)
        .execute_all()
        .expect("Valid");

    tester_assets
        .iter()
        .skip(N_ASSETS / 3)
        .take(N_ASSETS / 3)
        .zip(queried_assets)
        .for_each(|(tester, queried)| assert_eq!(*tester, queried));
}

const MAX_INSTRUCTIONS_PER_TX: usize = 5;

fn submit_chunked<I>(client: &Client, instructions: &[I]) -> Result<()>
where
    I: Clone + Into<InstructionBox>,
{
    for chunk in instructions.chunks(MAX_INSTRUCTIONS_PER_TX) {
        client
            .submit_all_blocking(chunk.iter().cloned())
            .wrap_err("Failed to submit instruction batch")?;
    }
    Ok(())
}

#[test]
#[allow(clippy::too_many_lines)]
fn correct_sorting_of_entities() {
    let builder = NetworkBuilder::new();
    let Some((network, _rt)) = start_network(builder, stringify!(correct_sorting_of_entities))
    else {
        return;
    };
    let test_client = network.client();

    let sort_by_metadata_key = "test_sort".parse::<Name>().expect("Valid");

    // Test sorting asset definitions

    let mut asset_definitions = vec![];
    let mut metadata_of_assets = vec![];
    let mut instructions = vec![];
    let n = 10_u32;
    for i in 0..n {
        let asset_definition_id = format!("xor_{i}#wonderland")
            .parse::<AssetDefinitionId>()
            .expect("Valid");
        let mut asset_metadata = Metadata::default();
        asset_metadata.insert(sort_by_metadata_key.clone(), n - i - 1);
        let asset_definition = AssetDefinition::numeric(asset_definition_id.clone())
            .with_metadata(asset_metadata.clone());

        metadata_of_assets.push(asset_metadata);
        asset_definitions.push(asset_definition_id);

        let create_asset_definition = Register::asset_definition(asset_definition);
        instructions.push(create_asset_definition);
    }

    submit_chunked(&test_client, &instructions).expect("Valid");

    let res = test_client
        .query(FindAssetsDefinitions::new())
        .with_sorting(Sorting::by_metadata_key(sort_by_metadata_key.clone()))
        .execute_all()
        .expect("Valid")
        .into_iter()
        .filter(|asset_definition| asset_definition.id().name().as_ref().starts_with("xor_"))
        .collect::<Vec<_>>();

    assert!(
        res.iter()
            .map(Identifiable::id)
            .eq(asset_definitions.iter().rev())
    );
    assert!(
        res.iter()
            .map(AssetDefinition::metadata)
            .eq(metadata_of_assets.iter().rev())
    );

    // Test sorting accounts

    let domain_name = "_neverland";
    let domain_id: DomainId = domain_name.parse().unwrap();
    test_client
        .submit_blocking(Register::domain(Domain::new(domain_id.clone())))
        .expect("should be committed");

    let mut accounts = vec![];
    let mut metadata_of_accounts = vec![];
    let mut instructions = vec![];

    let n = 10u32;
    let mut public_keys = (0..n)
        .map(|_| KeyPair::random().into_parts().0)
        .collect::<Vec<_>>();
    public_keys.sort_unstable();
    for i in 0..n {
        let account_id = AccountId::new(public_keys[i as usize].clone());
        let mut account_metadata = Metadata::default();
        account_metadata.insert(sort_by_metadata_key.clone(), n - i - 1);
        let account = Account::new(account_id.to_account_id(domain_id.clone()))
            .with_metadata(account_metadata.clone());

        accounts.push(account_id);
        metadata_of_accounts.push(account_metadata);

        let create_account = Register::account(account);
        instructions.push(create_account);
    }

    submit_chunked(&test_client, &instructions).expect("Valid");

    let res = test_client
        .query(FindAccounts::new())
        .with_sorting(Sorting::by_metadata_key(sort_by_metadata_key.clone()))
        .execute_all()
        .expect("Valid")
        .into_iter()
        .filter(|account| accounts.contains(account.id()))
        .collect::<Vec<_>>();

    assert!(res.iter().map(Identifiable::id).eq(accounts.iter().rev()));
    assert!(
        res.iter()
            .map(Account::metadata)
            .eq(metadata_of_accounts.iter().rev())
    );

    // Test sorting domains

    let mut domains = vec![];
    let mut metadata_of_domains = vec![];
    let mut instructions = vec![];
    let n = 10u32;
    for i in 0..n {
        let domain_id = format!("neverland{i}").parse::<DomainId>().expect("Valid");
        let mut domain_metadata = Metadata::default();
        domain_metadata.insert(sort_by_metadata_key.clone(), n - i - 1);
        let domain = Domain::new(domain_id.clone()).with_metadata(domain_metadata.clone());

        domains.push(domain_id);
        metadata_of_domains.push(domain_metadata);

        let create_account = Register::domain(domain);
        instructions.push(create_account);
    }

    submit_chunked(&test_client, &instructions).expect("Valid");

    let res = test_client
        .query(FindDomains::new())
        .with_sorting(Sorting::by_metadata_key(sort_by_metadata_key.clone()))
        .execute_all()
        .expect("Valid")
        .into_iter()
        .filter(|domain| domain.id().name().as_ref().starts_with("neverland"))
        .collect::<Vec<_>>();

    assert!(res.iter().map(Identifiable::id).eq(domains.iter().rev()));
    assert!(
        res.iter()
            .map(Domain::metadata)
            .eq(metadata_of_domains.iter().rev())
    );

    // Naive test sorting of domains
    let input = [(0_i32, 1_u32), (2, 0), (1, 2)];
    let mut domains = vec![];
    let mut metadata_of_domains = vec![];
    let mut instructions = vec![];
    for (idx, val) in input {
        let domain_id = format!("neverland_{idx}")
            .parse::<DomainId>()
            .expect("Valid");
        let mut domain_metadata = Metadata::default();
        domain_metadata.insert(sort_by_metadata_key.clone(), val);
        let domain = Domain::new(domain_id.clone()).with_metadata(domain_metadata.clone());

        domains.push(domain_id);
        metadata_of_domains.push(domain_metadata);

        let create_account = Register::domain(domain);
        instructions.push(create_account);
    }
    let _ = submit_chunked(&test_client, &instructions);

    let res = test_client
        .query(FindDomains::new())
        .with_sorting(Sorting::by_metadata_key(sort_by_metadata_key))
        .execute()
        .expect("Valid")
        .filter_map(Result::ok)
        .filter(|domain| domain.id().name().as_ref().starts_with("neverland_"))
        .collect::<Vec<_>>();

    assert_eq!(res[0].id(), &domains[1]);
    assert_eq!(res[1].id(), &domains[0]);
    assert_eq!(res[2].id(), &domains[2]);
    assert_eq!(res[0].metadata(), &metadata_of_domains[1]);
    assert_eq!(res[1].metadata(), &metadata_of_domains[0]);
    assert_eq!(res[2].metadata(), &metadata_of_domains[2]);
}

#[test]
fn metadata_sorting_descending() {
    let builder = NetworkBuilder::new();
    let Some((network, _rt)) = start_network(builder, stringify!(metadata_sorting_descending))
    else {
        return;
    };
    let test_client = network.client();

    let sort_by_metadata_key = "test_sort".parse::<Name>().expect("Valid");

    let mut asset_definitions = vec![];
    let mut metadata_of_assets = vec![];
    let mut instructions = vec![];
    let n = 10_u32;
    for i in 0..n {
        let asset_definition_id = format!("xor_{i}#wonderland")
            .parse::<AssetDefinitionId>()
            .expect("Valid");
        let mut asset_metadata = Metadata::default();
        asset_metadata.insert(sort_by_metadata_key.clone(), n - i - 1);
        let asset_definition = AssetDefinition::numeric(asset_definition_id.clone())
            .with_metadata(asset_metadata.clone());

        metadata_of_assets.push(asset_metadata);
        asset_definitions.push(asset_definition_id);

        let create_asset_definition = Register::asset_definition(asset_definition);
        instructions.push(create_asset_definition);
    }

    submit_chunked(&test_client, &instructions).expect("Valid");

    let res = test_client
        .query(FindAssetsDefinitions::new())
        .with_sorting(Sorting::new(
            Some(sort_by_metadata_key),
            Some(SortOrder::Desc),
        ))
        .execute_all()
        .expect("Valid")
        .into_iter()
        .filter(|asset_definition| asset_definition.id().name().as_ref().starts_with("xor_"))
        .collect::<Vec<_>>();

    assert!(
        res.iter()
            .map(Identifiable::id)
            .eq(asset_definitions.iter())
    );
    assert!(
        res.iter()
            .map(AssetDefinition::metadata)
            .eq(metadata_of_assets.iter())
    );
}

#[test]
fn sort_only_elements_which_have_sorting_key() -> Result<()> {
    const TEST_DOMAIN: &str = "neverland";

    let builder = NetworkBuilder::new();
    let Some((network, _rt)) = start_network(
        builder,
        stringify!(sort_only_elements_which_have_sorting_key),
    ) else {
        return Ok(());
    };
    let test_client = network.client();

    let domain_id: DomainId = TEST_DOMAIN.parse().unwrap();
    test_client
        .submit_blocking(Register::domain(Domain::new(domain_id.clone())))
        .expect("should be committed");

    let sort_by_metadata_key = "test_sort".parse::<Name>().expect("Valid");

    let mut accounts_a = vec![];
    let mut accounts_b = vec![];
    let mut instructions = vec![];

    let mut skip_set = HashSet::new();
    skip_set.insert(4);
    skip_set.insert(7);

    let n = 10u32;
    let mut public_keys = (0..n)
        .map(|_| KeyPair::random().into_parts().0)
        .collect::<Vec<_>>();
    public_keys.sort_unstable();
    for i in 0..n {
        let account_id = AccountId::new(public_keys[i as usize].clone());
        let account = if skip_set.contains(&i) {
            let account = Account::new(account_id.to_account_id(domain_id.clone()));
            accounts_b.push(account_id);
            account
        } else {
            let mut account_metadata = Metadata::default();
            account_metadata.insert(sort_by_metadata_key.clone(), n - i - 1);
            let account = Account::new(account_id.to_account_id(domain_id.clone()))
                .with_metadata(account_metadata);
            accounts_a.push(account_id);
            account
        };

        let create_account = Register::account(account);
        instructions.push(create_account);
    }

    submit_chunked(&test_client, &instructions).wrap_err("Failed to register accounts")?;

    let res = test_client
        .query(FindAccounts::new())
        .with_sorting(Sorting::by_metadata_key(sort_by_metadata_key))
        .execute_all()
        .wrap_err("Failed to submit request")?
        .into_iter()
        .filter(|account| accounts_a.contains(account.id()) || accounts_b.contains(account.id()))
        .collect::<Vec<_>>();

    let accounts = accounts_a.iter().rev().chain(accounts_b.iter());
    assert!(res.iter().map(Identifiable::id).eq(accounts));

    Ok(())
}
