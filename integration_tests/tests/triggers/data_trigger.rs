use core::panic;

use eyre::Result;
use iroha::{
    client,
    data_model::{prelude::*, Level},
};
use iroha_test_network::*;
use iroha_test_samples::{gen_account_in, ALICE_ID};

#[test]
fn must_execute_both_triggers() -> Result<()> {
    let (network, _rt) = NetworkBuilder::new().start_blocking()?;
    let test_client = network.client();

    let account_id = ALICE_ID.clone();
    let asset_definition_id = "rose#wonderland".parse()?;
    let asset_id = AssetId::new(asset_definition_id, account_id.clone());

    let get_asset_value = |iroha: &client::Client, asset_id: AssetId| -> Numeric {
        *iroha
            .query(FindAssets::new())
            .filter_with(|asset| asset.id.eq(asset_id))
            .execute_single()
            .unwrap()
            .value()
    };

    let prev_value = get_asset_value(&test_client, asset_id.clone());

    let instruction = Mint::asset_numeric(1u32, asset_id.clone());
    let register_trigger = Register::trigger(Trigger::new(
        "mint_rose_1".parse()?,
        Action::new(
            [instruction.clone()],
            Repeats::Indefinitely,
            account_id.clone(),
            AccountEventFilter::new().for_events(AccountEventSet::Created),
        ),
    ));
    test_client.submit_blocking(register_trigger)?;

    let register_trigger = Register::trigger(Trigger::new(
        "mint_rose_2".parse()?,
        Action::new(
            [instruction],
            Repeats::Indefinitely,
            account_id,
            DomainEventFilter::new().for_events(DomainEventSet::Created),
        ),
    ));
    test_client.submit_blocking(register_trigger)?;

    test_client.submit_blocking(Register::account(Account::new(
        gen_account_in("wonderland").0,
    )))?;

    let new_value = get_asset_value(&test_client, asset_id.clone());
    assert_eq!(new_value, prev_value.checked_add(numeric!(1)).unwrap());

    test_client.submit_blocking(Register::domain(Domain::new("neverland".parse()?)))?;

    let newer_value = get_asset_value(&test_client, asset_id);
    assert_eq!(newer_value, new_value.checked_add(numeric!(1)).unwrap());

    Ok(())
}

#[test]
fn airdrop_on_account_registration() -> Result<()> {
    const DEFAULT_AIRDROP: u32 = 100;

    let (network, _rt) = NetworkBuilder::new().start_blocking().unwrap();
    let test_client = network.client();

    let account_id = gen_account_in("wonderland").0;
    test_client.submit_blocking(Register::account(Account::new(account_id.clone())))?;

    test_client.submit_blocking(Log::new(Level::DEBUG, "Just ticking time".to_string()))?;

    let init_roses =
        super::get_asset_value(&test_client, format!("rose##{account_id}").parse().unwrap());
    assert_eq!(init_roses, DEFAULT_AIRDROP.into());

    Ok(())
}
