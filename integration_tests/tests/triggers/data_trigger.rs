use eyre::Result;
use iroha::{client, data_model::prelude::*};
use iroha_test_network::*;
use iroha_test_samples::{gen_account_in, load_sample_wasm, ALICE_ID};

#[test]
fn two_non_intersecting_execution_paths() -> Result<()> {
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

/// # Scenario
///
/// 1. The max execution depth starts at 1.
/// 2. A trigger is registered and immediately activated by the event.
/// 3. The trigger recursively invokes itself 110 times, incrementing both the current and the maximum allowed depth on each invocation.
/// 4. After recursion completes, the maximum allowed depth remains elevated.
///
/// Note: the current execution depth cannot be inspected.
///
/// # Implications
///
/// This test illustrates a potential loophole rather than a legitimate use case.
/// Under `Repeats::Indefinitely`, the trigger would loop indefinitely.
/// Such behavior must be prevented by enforcing:
/// - permissions for executable calls (#5441) and event subscriptions (#5439)
/// - quotas or fee-based consumption (#5440)
#[test]
fn cat_depth_and_mouse_depth() -> Result<()> {
    let (network, _rt) = NetworkBuilder::new()
        .with_genesis_instruction(SetParameter::new(Parameter::SmartContract(
            iroha_data_model::parameter::SmartContractParameter::ExecutionDepth(1),
        )))
        .start_blocking()?;
    let test_client = network.client();

    let parameters = test_client.query_single(FindParameters)?;
    assert_eq!(1, parameters.smart_contract().execution_depth());

    test_client.submit_blocking(Register::trigger(Trigger::new(
        "cat_and_mouse".parse().unwrap(),
        Action::new(
            load_sample_wasm("trigger_cat_and_mouse"),
            Repeats::Exactly(110),
            ALICE_ID.clone(),
            DataEventFilter::Any,
        ),
    )))?;

    let parameters = test_client.query_single(FindParameters)?;
    assert_eq!(111, parameters.smart_contract().execution_depth());

    Ok(())
}
