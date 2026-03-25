//! This file contains examples from the Rust tutorial.

use eyre::{Error, WrapErr};
use iroha::config::{Config, LoadPath};
// #region rust_config_crates
// #endregion rust_config_crates

fn main() {
    // #region rust_config_load
    let config = Config::load(LoadPath::Explicit("../../defaults/client.toml")).unwrap();
    // #endregion rust_config_load

    // Your code goes here…

    domain_registration_test(config.clone())
        .expect("Domain registration example is expected to work correctly");
    account_definition_test();
    account_registration_test(config.clone())
        .expect("Account registration example is expected to work correctly");
    asset_registration_test(config.clone())
        .expect("Asset registration example is expected to work correctly");
    asset_minting_test(config.clone())
        .expect("Asset minting example is expected to work correctly");
    asset_burning_test(config.clone())
        .expect("Asset burning example is expected to work correctly");
    // output_visualising_test(&config).expect(msg: "Visualising outputs example is expected to work correctly");
    println!("Success!");
}

fn domain_registration_test(config: Config) -> Result<(), Error> {
    // #region domain_register_example_crates
    use iroha::{
        client::Client,
        data_model::{
            metadata::Metadata,
            prelude::{Domain, DomainId, InstructionBox, Register},
        },
    };
    // #endregion domain_register_example_crates

    // #region domain_register_example_create_domain
    // Create a domain Id
    let looking_glass: DomainId = "looking_glass".parse()?;
    // #endregion domain_register_example_create_domain

    // #region domain_register_example_create_isi
    // Create an ISI
    let create_looking_glass = Register::domain(Domain::new(looking_glass));
    // #endregion domain_register_example_create_isi

    // #region rust_client_create
    // Create an Iroha client
    let client = Client::new(config);
    // #endregion rust_client_create

    // #region domain_register_example_prepare_tx
    // Prepare a transaction
    let metadata = Metadata::default();
    let instructions: Vec<InstructionBox> = vec![create_looking_glass.into()];
    let tx = client.build_transaction(instructions, metadata);
    // #endregion domain_register_example_prepare_tx

    // #region domain_register_example_submit_tx
    // Submit a prepared domain registration transaction
    client
        .submit_transaction(&tx)
        .wrap_err("Failed to submit transaction")?;
    // #endregion domain_register_example_submit_tx

    // Finish the test successfully
    Ok(())
}

fn account_definition_test() {
    // #region account_definition_comparison
    use iroha::{crypto::KeyPair, data_model::prelude::AccountId};

    // Generate a new public key for a new account
    let (public_key, _) = KeyPair::random().into_parts();
    // Materialize a scoped AccountId for `looking_glass` from the account subject's public key
    let longhand_account_id = AccountId::new(public_key.clone());
    // Create an AccountId instance by parsing the canonical I105 account address form
    let canonical_account_id = longhand_account_id
        .canonical_i105()
        .expect("Single-key account IDs can be rendered as I105");
    let account_id = AccountId::parse_encoded(&canonical_account_id)
        .map(iroha::account_address::ParsedAccountId::into_account_id)
        .expect("Valid, because the I105 payload was generated from a valid AccountId");

    // Check that two ways to define an account match
    assert_eq!(account_id, longhand_account_id);

    // #endregion account_definition_comparison
}

fn account_registration_test(config: Config) -> Result<(), Error> {
    // #region register_account_crates
    use iroha::{
        client::Client,
        crypto::KeyPair,
        data_model::{
            metadata::Metadata,
            prelude::{Account, AccountId, DomainId, InstructionBox, Register},
        },
    };
    // #endregion register_account_crates

    // Create an Iroha client
    let client = Client::new(config);

    // #region register_account_create
    // Generate a new public key for a new account
    let (public_key, _) = KeyPair::random().into_parts();
    // Materialize a scoped AccountId in the domain where this registration is performed
    let account_id = AccountId::new(public_key);
    // #endregion register_account_create

    // #region register_account_generate
    // Generate a new account
    let account_domain: DomainId = "wonderland".parse().expect("valid domain id");
    let create_account = Register::account(Account::new(account_id.to_account_id(account_domain)));
    // #endregion register_account_generate

    // #region register_account_prepare_tx
    // Prepare a transaction using the
    // Account's RegisterBox
    let metadata = Metadata::default();
    let instructions: Vec<InstructionBox> = vec![create_account.into()];
    let tx = client.build_transaction(instructions, metadata);
    // #endregion register_account_prepare_tx

    // #region register_account_submit_tx
    // Submit a prepared account registration transaction
    client.submit_transaction(&tx)?;
    // #endregion register_account_submit_tx

    // Finish the test successfully
    Ok(())
}

fn asset_registration_test(config: Config) -> Result<(), Error> {
    // #region register_asset_crates
    use iroha::{
        client::Client,
        crypto::KeyPair,
        data_model::prelude::{
            AccountId, AssetDefinition, AssetDefinitionId, AssetId, Mint, Register, numeric,
        },
    };
    // #endregion register_asset_crates

    // Create an Iroha client
    let client = Client::new(config);

    // #region register_asset_create_asset
    // Create an asset
    let asset_def_id = AssetDefinitionId::new(
        "looking_glass".parse().expect("valid domain identifier"),
        "time".parse().expect("valid asset identifier"),
    );
    // #endregion register_asset_create_asset

    // #region register_asset_init_submit
    // Initialise the registration time
    let register_time = Register::asset_definition(
        AssetDefinition::numeric(asset_def_id.clone())
            .with_name("time".to_owned())
            .mintable_once(),
    );

    // Submit a registration time
    client.submit(register_time)?;
    // #endregion register_asset_init_submit

    // Generate a new public key for a new account
    let (public_key, _) = KeyPair::random().into_parts();
    // Materialize a scoped AccountId in the domain used for this minting example
    let account_id = AccountId::new(public_key);

    // #region register_asset_mint_submit
    // Create a MintBox using a previous asset and account
    let mint = Mint::asset_numeric(numeric!(12.34), AssetId::new(asset_def_id, account_id));

    // Submit a minting transaction
    client.submit_all([mint])?;
    // #endregion register_asset_mint_submit

    // Finish the test successfully
    Ok(())
}

fn asset_minting_test(config: Config) -> Result<(), Error> {
    // #region mint_asset_crates
    use iroha::{
        client::Client,
        data_model::prelude::{AccountId, AssetDefinitionId, AssetId, Mint},
    };
    // #endregion mint_asset_crates

    // Create an Iroha client
    let client = Client::new(config);

    // Define the instances of an Asset and Account
    // #region mint_asset_define_asset_account
    let roses = AssetDefinitionId::new(
        "wonderland".parse().expect("valid domain identifier"),
        "rose".parse().expect("valid asset identifier"),
    );
    let alice = AccountId::new(
        "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03"
            .parse()
            .expect("Valid, because this is a valid public key literal"),
    );
    // #endregion mint_asset_define_asset_account

    // Mint the Asset instance
    // #region mint_asset_mint
    let mint_roses = Mint::asset_numeric(42u32, AssetId::new(roses.clone(), alice.clone()));
    // #endregion mint_asset_mint

    // #region mint_asset_submit_tx
    client
        .submit(mint_roses)
        .wrap_err("Failed to submit transaction")?;
    // #endregion mint_asset_submit_tx

    // #region mint_asset_mint_alt
    // Mint the Asset instance (alternate syntax).
    // AssetId textual representation uses the public
    // `<asset-definition-id>#<account-id>` form with optional `#dataspace:<id>` suffix.
    let alice_roses_literal = AssetId::new(roses, alice).canonical_literal();
    let alice_roses: AssetId = alice_roses_literal.parse()?;
    let mint_roses_alt = Mint::asset_numeric(10u32, alice_roses);
    // #endregion mint_asset_mint_alt

    // #region mint_asset_submit_tx_alt
    client
        .submit(mint_roses_alt)
        .wrap_err("Failed to submit transaction")?;
    // #endregion mint_asset_submit_tx_alt

    // Finish the test successfully
    Ok(())
}

fn asset_burning_test(config: Config) -> Result<(), Error> {
    // #region burn_asset_crates
    use iroha::{
        client::Client,
        data_model::prelude::{AccountId, AssetDefinitionId, AssetId, Burn},
    };
    // #endregion burn_asset_crates

    // Create an Iroha client
    let client = Client::new(config);

    // #region burn_asset_define_asset_account
    // Define the instances of an Asset and Account
    let roses = AssetDefinitionId::new(
        "wonderland".parse().expect("valid domain identifier"),
        "rose".parse().expect("valid asset identifier"),
    );
    let alice = AccountId::new(
        "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03"
            .parse()
            .expect("Valid, because this is a valid public key literal"),
    );
    // #endregion burn_asset_define_asset_account

    // #region burn_asset_burn
    // Burn the Asset instance
    let burn_roses = Burn::asset_numeric(10u32, AssetId::new(roses.clone(), alice.clone()));
    // #endregion burn_asset_burn

    // #region burn_asset_submit_tx
    client
        .submit(burn_roses)
        .wrap_err("Failed to submit transaction")?;
    // #endregion burn_asset_submit_tx

    // #region burn_asset_burn_alt
    // Burn the Asset instance (alternate syntax).
    // AssetId textual representation uses the public
    // `<asset-definition-id>#<account-id>` form with optional `#dataspace:<id>` suffix.
    let alice_roses_literal = AssetId::new(roses, alice).canonical_literal();
    let alice_roses: AssetId = alice_roses_literal.parse()?;
    let burn_roses_alt = Burn::asset_numeric(10u32, alice_roses);
    // #endregion burn_asset_burn_alt

    // #region burn_asset_submit_tx_alt
    client
        .submit(burn_roses_alt)
        .wrap_err("Failed to submit transaction")?;
    // #endregion burn_asset_submit_tx_alt

    // Finish the test successfully
    Ok(())
}
