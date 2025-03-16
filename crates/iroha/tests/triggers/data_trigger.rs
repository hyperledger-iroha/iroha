use eyre::Result;
use iroha::{
    client,
    data_model::{parameter::SumeragiParameter, prelude::*},
};
use iroha_test_network::*;
use iroha_test_samples::{gen_account_in, ALICE_ID};

use super::*;

/// # Scenario
///
/// 0. Transaction: [register Carol]
/// 0. Trigger execution: account created (Carol) -> mint roses for Carol
/// 0. Transaction: [burn one of Carol's roses] ... Depends on the previous trigger execution
/// 0. Block commit
#[test]
#[ignore = "enable in #4937"]
fn executes_on_every_transaction() -> Result<()> {
    let carol = gen_account_in("wonderland");
    let rose_carol: AssetId = format!("rose##{}", carol.0).parse().unwrap();
    let mint_roses_on_carol_creation = Trigger::new(
        "mint_roses_on_carol_creation".parse().unwrap(),
        Action::new(
            vec![Mint::asset_numeric(2_u32, rose_carol.clone())],
            Repeats::Indefinitely,
            ALICE_ID.clone(),
            AccountEventFilter::new()
                .for_account(carol.0.clone())
                .for_events(AccountEventSet::Created),
        ),
    );
    let (network, _rt) = NetworkBuilder::new()
        .with_genesis_instruction(SetParameter::new(Parameter::Sumeragi(
            // This reset to the default matters for some reason
            SumeragiParameter::BlockTimeMs(2_000),
        )))
        .with_genesis_instruction(Register::trigger(mint_roses_on_carol_creation))
        .start_blocking()?;
    let test_client = network.client();

    test_client.submit(Register::account(Account::new(carol.0.clone())))?;
    test_client.submit_blocking(Burn::asset_numeric(1_u32, rose_carol.clone()))?;
    assert_eq!(2, test_client.get_status().unwrap().blocks);
    assert_eq!(numeric!(1), get_asset_value(&test_client, rose_carol));

    Ok(())
}

mod matches_a_batch_of_events {
    use std::collections::BTreeMap;

    use iroha_data_model::isi::Instruction;
    use iroha_test_samples::load_sample_wasm;

    use super::*;

    /// # Scenario
    ///
    /// 0. Transaction: [mint a rose, mint another rose]
    /// 0. Trigger execution: asset minted (some roses) -> burn both roses
    #[test]
    #[ignore = "enable in #4937"]
    fn accumulation() -> Result<()> {
        let carol = gen_account_in("wonderland");
        let mint_a_rose = Mint::asset_numeric(1_u32, format!("rose##{}", carol.0).parse().unwrap());

        test((0..2).map(|_| mint_a_rose.clone()), |_roses| todo!())
    }

    /// # Scenario
    ///
    /// 0. Transaction: [register Carol, register Dave]
    /// 0. Trigger execution: account created (Carol and Dave) -> mint a rose for each
    #[test]
    #[ignore = "enable in #4937"]
    fn union() -> Result<()> {
        todo!()
    }

    fn test(
        when: impl Iterator<Item = impl Instruction>,
        predicate: impl Fn(BTreeMap<AssetId, Numeric>) -> bool,
    ) -> Result<()> {
        let matches_a_batch_of_events = Trigger::new(
            "matches_a_batch_of_events".parse().unwrap(),
            Action::new(
                load_sample_wasm("matches_a_batch_of_events"),
                Repeats::Indefinitely,
                ALICE_ID.clone(),
                DomainEventFilter::new().for_domain("wonderland".parse().unwrap()),
            ),
        );
        let (network, _rt) = NetworkBuilder::new()
            .with_genesis_instruction(Register::trigger(matches_a_batch_of_events))
            .start_blocking()?;
        let test_client = network.client();

        test_client.submit_all_blocking(when)?;
        let roses = test_client
            .query(FindAssets)
            .filter_with(|asset| asset.id.definition.eq("rose#wonderland".parse().unwrap()))
            .select_with(|asset| (asset.id, asset.value))
            .execute()?
            .collect::<Result<BTreeMap<_, _>, _>>()?;
        assert!(predicate(roses));

        Ok(())
    }
}

/// # Scenario
///
/// 0. Register `trigger_1` with `filter_1`
/// 0. Register `trigger_2` with `filter_2`
/// 0. Emit an event that matches both `filter_1` and `filter_2`
/// 0. Both `trigger_1` and `trigger_2` execute
#[test]
fn subscribe_events() -> Result<()> {
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
