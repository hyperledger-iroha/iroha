use std::{str::FromStr as _, thread};

use eyre::Result;
use iroha_client::{
    client::{self, QueryResult},
    crypto::{KeyPair, PublicKey},
    data_model::prelude::*,
};
use iroha_config::iroha::Configuration;
use iroha_primitives::fixed::Fixed;
use serde_json::json;
use test_network::*;

#[test]
fn client_register_asset_should_add_asset_once_but_not_twice() -> Result<()> {
    let (_rt, _peer, test_client) = <PeerBuilder>::new().with_port(10_620).start_with_runtime();
    wait_for_genesis_committed(&[test_client.clone()], 0);

    // Given
    let account_id = AccountId::from_str("alice@wonderland").expect("Valid");

    let asset_definition_id = AssetDefinitionId::from_str("test_asset#wonderland").expect("Valid");
    let create_asset: InstructionBox =
        Register::asset_definition(AssetDefinition::quantity(asset_definition_id.clone())).into();
    let register_asset: InstructionBox = Register::asset(Asset::new(
        AssetId::new(asset_definition_id.clone(), account_id.clone()),
        AssetValue::Quantity(0),
    ))
    .into();

    test_client.submit_all([create_asset, register_asset.clone()])?;

    // Registering an asset to an account which doesn't have one
    // should result in asset being created
    test_client.poll_request(client::asset::by_account_id(account_id), |result| {
        let assets = result.collect::<QueryResult<Vec<_>>>().expect("Valid");

        assets.iter().any(|asset| {
            asset.id().definition_id == asset_definition_id
                && *asset.value() == AssetValue::Quantity(0)
        })
    })?;

    // But registering an asset to account already having one should fail
    assert!(test_client.submit_blocking(register_asset).is_err());

    Ok(())
}

#[test]
fn unregister_asset_should_remove_asset_from_account() -> Result<()> {
    let (_rt, _peer, test_client) = <PeerBuilder>::new().with_port(10_555).start_with_runtime();
    wait_for_genesis_committed(&[test_client.clone()], 0);

    // Given
    let account_id = AccountId::from_str("alice@wonderland").expect("Valid");

    let asset_definition_id = AssetDefinitionId::from_str("test_asset#wonderland").expect("Valid");
    let asset_id = AssetId::new(asset_definition_id.clone(), account_id.clone());
    let create_asset: InstructionBox =
        Register::asset_definition(AssetDefinition::quantity(asset_definition_id.clone())).into();
    let register_asset =
        Register::asset(Asset::new(asset_id.clone(), AssetValue::Quantity(0))).into();
    let unregister_asset = Unregister::asset(asset_id);

    test_client.submit_all([create_asset, register_asset])?;

    // Wait for asset to be registered
    test_client.poll_request(client::asset::by_account_id(account_id.clone()), |result| {
        let assets = result.collect::<QueryResult<Vec<_>>>().expect("Valid");

        assets
            .iter()
            .any(|asset| asset.id().definition_id == asset_definition_id)
    })?;

    test_client.submit(unregister_asset)?;

    // ... and check that it is removed after Unregister
    test_client.poll_request(client::asset::by_account_id(account_id), |result| {
        let assets = result.collect::<QueryResult<Vec<_>>>().expect("Valid");

        assets
            .iter()
            .all(|asset| asset.id().definition_id != asset_definition_id)
    })?;

    Ok(())
}

#[test]
fn client_add_asset_quantity_to_existing_asset_should_increase_asset_amount() -> Result<()> {
    let (_rt, _peer, test_client) = <PeerBuilder>::new().with_port(10_000).start_with_runtime();
    wait_for_genesis_committed(&[test_client.clone()], 0);

    // Given
    let account_id = AccountId::from_str("alice@wonderland").expect("Valid");
    let asset_definition_id = AssetDefinitionId::from_str("xor#wonderland").expect("Valid");
    let create_asset =
        Register::asset_definition(AssetDefinition::quantity(asset_definition_id.clone()));
    let metadata = iroha_client::data_model::metadata::UnlimitedMetadata::default();
    //When
    let quantity: u32 = 200;
    let mint = Mint::asset_quantity(
        quantity,
        AssetId::new(asset_definition_id.clone(), account_id.clone()),
    );
    let instructions: [InstructionBox; 2] = [create_asset.into(), mint.into()];
    let tx = test_client.build_transaction(instructions, metadata)?;
    test_client.submit_transaction(&tx)?;
    test_client.poll_request(client::asset::by_account_id(account_id), |result| {
        let assets = result.collect::<QueryResult<Vec<_>>>().expect("Valid");

        assets.iter().any(|asset| {
            asset.id().definition_id == asset_definition_id
                && *asset.value() == AssetValue::Quantity(quantity)
        })
    })?;
    Ok(())
}

#[test]
fn client_add_big_asset_quantity_to_existing_asset_should_increase_asset_amount() -> Result<()> {
    let (_rt, _peer, test_client) = <PeerBuilder>::new().with_port(10_510).start_with_runtime();
    wait_for_genesis_committed(&[test_client.clone()], 0);

    // Given
    let account_id = AccountId::from_str("alice@wonderland").expect("Valid");
    let asset_definition_id = AssetDefinitionId::from_str("xor#wonderland").expect("Valid");
    let create_asset =
        Register::asset_definition(AssetDefinition::big_quantity(asset_definition_id.clone()));
    let metadata = iroha_client::data_model::metadata::UnlimitedMetadata::default();
    //When
    let quantity: u128 = 2_u128.pow(65);
    let mint = Mint::asset_big_quantity(
        quantity,
        AssetId::new(asset_definition_id.clone(), account_id.clone()),
    );
    let instructions: [InstructionBox; 2] = [create_asset.into(), mint.into()];
    let tx = test_client.build_transaction(instructions, metadata)?;
    test_client.submit_transaction(&tx)?;
    test_client.poll_request(client::asset::by_account_id(account_id), |result| {
        let assets = result.collect::<QueryResult<Vec<_>>>().expect("Valid");

        assets.iter().any(|asset| {
            asset.id().definition_id == asset_definition_id
                && *asset.value() == AssetValue::BigQuantity(quantity)
        })
    })?;
    Ok(())
}

#[test]
fn client_add_asset_with_decimal_should_increase_asset_amount() -> Result<()> {
    let (_rt, _peer, test_client) = <PeerBuilder>::new().with_port(10_515).start_with_runtime();
    wait_for_genesis_committed(&[test_client.clone()], 0);

    // Given
    let account_id = AccountId::from_str("alice@wonderland").expect("Valid");
    let asset_definition_id = AssetDefinitionId::from_str("xor#wonderland").expect("Valid");
    let asset_definition = AssetDefinition::fixed(asset_definition_id.clone());
    let create_asset = Register::asset_definition(asset_definition);
    let metadata = iroha_client::data_model::metadata::UnlimitedMetadata::default();

    //When
    let quantity: Fixed = Fixed::try_from(123.456_f64).unwrap();
    let mint = Mint::asset_fixed(
        quantity,
        AssetId::new(asset_definition_id.clone(), account_id.clone()),
    );
    let instructions: [InstructionBox; 2] = [create_asset.into(), mint.into()];
    let tx = test_client.build_transaction(instructions, metadata)?;
    test_client.submit_transaction(&tx)?;
    test_client.poll_request(client::asset::by_account_id(account_id.clone()), |result| {
        let assets = result.collect::<QueryResult<Vec<_>>>().expect("Valid");

        assets.iter().any(|asset| {
            asset.id().definition_id == asset_definition_id
                && *asset.value() == AssetValue::Fixed(quantity)
        })
    })?;

    // Add some fractional part
    let quantity2: Fixed = Fixed::try_from(0.55_f64).unwrap();
    let mint = Mint::asset_fixed(
        quantity2,
        AssetId::new(asset_definition_id.clone(), account_id.clone()),
    );
    // and check that it is added without errors
    let sum = quantity
        .checked_add(quantity2)
        .map_err(|e| eyre::eyre!("{}", e))?;
    test_client.submit_till(mint, client::asset::by_account_id(account_id), |result| {
        let assets = result.collect::<QueryResult<Vec<_>>>().expect("Valid");

        assets.iter().any(|asset| {
            asset.id().definition_id == asset_definition_id
                && *asset.value() == AssetValue::Fixed(sum)
        })
    })?;
    Ok(())
}

#[test]
fn client_add_asset_with_name_length_more_than_limit_should_not_commit_transaction() -> Result<()> {
    let (_rt, _peer, test_client) = <PeerBuilder>::new().with_port(10_520).start_with_runtime();
    wait_for_genesis_committed(&[test_client.clone()], 0);
    let pipeline_time = Configuration::pipeline_time();

    // Given
    let normal_asset_definition_id = AssetDefinitionId::from_str("xor#wonderland").expect("Valid");
    let create_asset = Register::asset_definition(AssetDefinition::quantity(
        normal_asset_definition_id.clone(),
    ));
    test_client.submit(create_asset)?;
    iroha_logger::info!("Creating asset");

    let too_long_asset_name = "0".repeat(2_usize.pow(14));
    let incorrect_asset_definition_id =
        AssetDefinitionId::from_str(&(too_long_asset_name + "#wonderland")).expect("Valid");
    let create_asset = Register::asset_definition(AssetDefinition::quantity(
        incorrect_asset_definition_id.clone(),
    ));

    test_client.submit(create_asset)?;
    iroha_logger::info!("Creating another asset");
    thread::sleep(pipeline_time * 4);

    let mut asset_definition_ids = test_client
        .request(client::asset::all_definitions())
        .expect("Failed to execute request.")
        .collect::<QueryResult<Vec<_>>>()
        .expect("Failed to execute request.")
        .into_iter()
        .map(|asset| asset.id().clone());
    iroha_logger::debug!(
        "Collected asset definitions ID's: {:?}",
        &asset_definition_ids
    );

    assert!(asset_definition_ids
        .any(|asset_definition_id| asset_definition_id == normal_asset_definition_id));
    assert!(!asset_definition_ids
        .any(|asset_definition_id| asset_definition_id == incorrect_asset_definition_id));

    Ok(())
}

#[allow(unused_must_use)]
#[allow(clippy::too_many_lines)]
#[allow(clippy::expect_fun_call)]
#[test]
fn find_rate_and_make_exchange_isi_should_succeed() {
    let (_rt, _peer, test_client) = <PeerBuilder>::new().with_port(10_675).start_with_runtime();
    wait_for_genesis_committed(&[test_client.clone()], 0);

    let alice_id: AccountId = "alice@wonderland".parse().expect("Valid.");
    let seller_id: AccountId = "seller@company".parse().expect("Valid.");
    let buyer_id: AccountId = "buyer@company".parse().expect("Valid.");

    let seller_btc: AssetId = "btc#crypto#seller@company".parse().expect("Valid.");
    let buyer_eth: AssetId = "eth#crypto#buyer@company".parse().expect("Valid.");

    let seller_keypair = KeyPair::generate().expect("Failed to generate seller KeyPair.");
    let buyer_keypair = KeyPair::generate().expect("Failed to generate seller KeyPair.");

    let register_account = |account_id: AccountId, signature: PublicKey| {
        Register::account(Account::new(account_id, [signature]))
    };

    let grant_alice_asset_transfer_permission = |asset_id: AssetId, owner_keypair: KeyPair| {
        let allow_alice_to_transfer_asset = Grant::permission(
            PermissionToken::new(
                "CanTransferUserAsset".parse().unwrap(),
                &json!({ "asset_id": asset_id }),
            ),
            alice_id.clone(),
        );

        let chain_id = ChainId::new("0");
        let grant_asset_transfer_tx =
            TransactionBuilder::new(chain_id, asset_id.account_id().clone())
                .with_instructions([allow_alice_to_transfer_asset])
                .sign(owner_keypair)
                .expect("Failed to sign seller transaction");

        test_client
            .submit_transaction_blocking(&grant_asset_transfer_tx)
            .expect(&format!(
                "Failed to grant permission alice to transfer {asset_id}",
            ));
    };

    let buyer_account_id = account_id_new("buyer", "company");
    let seller_account_id = account_id_new("seller", "company");
    let asset_id = asset_id_new(
        "btc2eth_rate",
        "exchange",
        account_id_new("dex", "exchange"),
    );
    let instructions: [InstructionBox; 12] = [
        register::domain("exchange").into(),
        register::domain("company").into(),
        register::domain("crypto").into(),
        register_account(seller_id, seller_keypair.public_key().clone()).into(),
        register_account(buyer_id, buyer_keypair.public_key().clone()).into(),
        register::account("dex", "exchange").into(),
        register::asset_definition("btc", "crypto").into(),
        register::asset_definition("eth", "crypto").into(),
        register::asset_definition("btc2eth_rate", "exchange").into(),
        Mint::asset_quantity(
            200_u32,
            asset_id_new("eth", "crypto", buyer_account_id.clone()),
        )
        .into(),
        Mint::asset_quantity(
            20_u32,
            asset_id_new("btc", "crypto", seller_account_id.clone()),
        )
        .into(),
        Mint::asset_quantity(20_u32, asset_id.clone()).into(),
    ];
    test_client
        .submit_all_blocking(instructions)
        .expect("Failed to prepare accounts.");

    grant_alice_asset_transfer_permission(seller_btc, seller_keypair);
    grant_alice_asset_transfer_permission(buyer_eth, buyer_keypair);

    let to_transfer = test_client
        .request(FindAssetQuantityById::new(asset_id))
        .expect("Failed to execute query to find asset quantity by id.");
    let to_transfer = match to_transfer {
        NumericValue::U32(value) => value,
        _ => panic!("Wrong asset quantity type."),
    };
    test_client
        .submit_all_blocking([
            Transfer::asset_quantity(
                asset_id_new("btc", "crypto", seller_account_id.clone()),
                to_transfer,
                buyer_account_id.clone(),
            ),
            Transfer::asset_quantity(
                asset_id_new("eth", "crypto", buyer_account_id),
                to_transfer,
                seller_account_id,
            ),
        ])
        .expect("Failed to exchange eth for btc.");

    let expected_seller_eth = NumericValue::U32(20);
    let expected_buyer_eth = NumericValue::U32(180);
    let expected_buyer_btc = NumericValue::U32(20);

    let eth_quantity = test_client
        .request(FindAssetQuantityById::new(asset_id_new(
            "eth",
            "crypto",
            account_id_new("seller", "company"),
        )))
        .expect("Failed to execute Iroha Query");
    assert_eq!(expected_seller_eth, eth_quantity);

    // For the btc amount we expect an error, as zero assets are purged from accounts
    test_client
        .request(FindAssetQuantityById::new(asset_id_new(
            "btc",
            "crypto",
            account_id_new("seller", "company"),
        )))
        .expect_err("Query must fail");

    let buyer_eth_quantity = test_client
        .request(FindAssetQuantityById::new(asset_id_new(
            "eth",
            "crypto",
            account_id_new("buyer", "company"),
        )))
        .expect("Failed to execute Iroha Query");
    assert_eq!(expected_buyer_eth, buyer_eth_quantity);

    let buyer_btc_quantity = test_client
        .request(FindAssetQuantityById::new(asset_id_new(
            "btc",
            "crypto",
            account_id_new("buyer", "company"),
        )))
        .expect("Failed to execute Iroha Query");
    assert_eq!(expected_buyer_btc, buyer_btc_quantity);
}

#[test]
fn transfer_asset_definition() {
    let (_rt, _peer, test_client) = <PeerBuilder>::new().with_port(11_060).start_with_runtime();
    wait_for_genesis_committed(&[test_client.clone()], 0);

    let alice_id: AccountId = "alice@wonderland".parse().expect("Valid.");
    let bob_id: AccountId = "bob@wonderland".parse().expect("Valid.");
    let asset_definition_id: AssetDefinitionId = "asset#wonderland".parse().expect("Valid");

    test_client
        .submit_blocking(Register::asset_definition(AssetDefinition::quantity(
            asset_definition_id.clone(),
        )))
        .expect("Failed to submit transaction");

    let asset_definition = test_client
        .request(FindAssetDefinitionById::new(asset_definition_id.clone()))
        .expect("Failed to execute Iroha Query");
    assert_eq!(asset_definition.owned_by(), &alice_id);

    test_client
        .submit_blocking(Transfer::asset_definition(
            alice_id,
            asset_definition_id.clone(),
            bob_id.clone(),
        ))
        .expect("Failed to submit transaction");

    let asset_definition = test_client
        .request(FindAssetDefinitionById::new(asset_definition_id))
        .expect("Failed to execute Iroha Query");
    assert_eq!(asset_definition.owned_by(), &bob_id);
}

fn account_id_new(account_name: &str, account_domain: &str) -> AccountId {
    AccountId::new(
        account_domain.parse().expect("Valid"),
        account_name.parse().expect("Valid"),
    )
}

pub fn asset_id_new(
    definition_name: &str,
    definition_domain: &str,
    account_id: AccountId,
) -> AssetId {
    AssetId::new(
        AssetDefinitionId::new(
            definition_domain.parse().expect("Valid"),
            definition_name.parse().expect("Valid"),
        ),
        account_id,
    )
}

mod register {
    use super::*;

    pub fn domain(name: &str) -> Register<Domain> {
        Register::domain(Domain::new(DomainId::from_str(name).expect("Valid")))
    }

    pub fn account(account_name: &str, domain_name: &str) -> Register<Account> {
        Register::account(Account::new(
            AccountId::new(
                domain_name.parse().expect("Valid"),
                account_name.parse().expect("Valid"),
            ),
            [],
        ))
    }

    pub fn asset_definition(asset_name: &str, domain_name: &str) -> Register<AssetDefinition> {
        Register::asset_definition(AssetDefinition::quantity(AssetDefinitionId::new(
            domain_name.parse().expect("Valid"),
            asset_name.parse().expect("Valid"),
        )))
    }
}
