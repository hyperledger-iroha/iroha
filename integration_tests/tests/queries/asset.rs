//! Asset query regression scenarios.
use eyre::{Result, eyre};
use integration_tests::sandbox;
use iroha::{
    blocking::Client,
    collections::{Collection, ListQuery, field},
    data_model::prelude::*,
};
use iroha_model_base::domain::DomainId;
use iroha_test_network::submit_ensure_domain_for_network;
use iroha_test_samples::{ALICE_ID, gen_account_in};
use std::{thread, time::Duration};
#[derive(norito::JsonDeserialize)]
struct DefinitionQuantity {
    id: AssetDefinitionId,
    total_quantity: Quantity,
}
#[derive(norito::JsonDeserialize)]
struct HolderQuantity {
    id: AssetId,
    account_id: AccountId,
    asset: AssetDefinitionId,
    quantity: Quantity,
}
const UNREGISTER_ATTEMPTS: usize = 30;
const UNREGISTER_DELAY: Duration = Duration::from_millis(250);
#[test]
#[allow(clippy::too_many_lines)]
fn find_asset_total_quantity() -> Result<()> {
    let Some((network, _rt)) = sandbox::start_network_blocking_or_skip(
        super::query_network_builder(),
        stringify!(find_asset_total_quantity),
    )
    .unwrap() else {
        return Ok(());
    };
    let test_client = super::query_client(&network);
    let result: Result<()> = (|| {
        // Register new domain
        let domain_id: DomainId = DomainId::try_new("looking-glass", "universal")?;
        submit_ensure_domain_for_network(&network, &test_client, Domain::new(domain_id))?;
        let accounts: [AccountId; 5] = [
            ALICE_ID.clone(),
            gen_account_in("wonderland").0,
            gen_account_in("wonderland").0,
            gen_account_in("wonderland").0,
            gen_account_in("looking-glass").0,
        ];
        let wonderland_domain: DomainId = DomainId::try_new("wonderland", "universal")?;
        let quantity_definition = AssetDefinitionId::derive_from_components(
            wonderland_domain.clone(),
            "quantity".parse()?,
        );
        let fixed_definition =
            AssetDefinitionId::derive_from_components(wonderland_domain.clone(), "fixed".parse()?);
        // Registering accounts
        let register_accounts = accounts
            .iter()
            .enumerate()
            .skip(1) // Alice has already been registered in genesis
            .map(|(_index, account_id)| Register::account(Account::new(account_id.clone())))
            .collect::<Vec<_>>();
        test_client.submit_all(
            register_accounts,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )?;
        // Test asset quantity values.
        test_total_quantity(
            &test_client,
            &accounts,
            &quantity_definition,
            "quantity".to_owned(),
            NumericSpec::default(),
            &Quantity::one(),
            &Quantity::from(10_u32),
            &Quantity::from(5_u32),
            &Quantity::from(30_u32),
        )?;
        test_total_quantity(
            &test_client,
            &accounts,
            &fixed_definition,
            "fixed".to_owned(),
            NumericSpec::default(),
            &Quantity::one(),
            &Quantity::from(10_u32),
            &Quantity::from(5_u32),
            &Quantity::from(30_u32),
        )?;
        Ok(())
    })();
    if sandbox::handle_result(result, stringify!(find_asset_total_quantity))?.is_none() {
        return Ok(());
    }
    Ok(())
}
#[allow(clippy::too_many_arguments)]
#[allow(clippy::too_many_lines)]
fn test_total_quantity(
    test_client: &Client,
    accounts: &[AccountId; 5],
    definition_id: &AssetDefinitionId,
    definition_name: String,
    asset_spec: NumericSpec,
    initial_value: &Quantity,
    to_mint: &Quantity,
    to_burn: &Quantity,
    expected_total_asset_quantity: &Quantity,
) -> Result<()> {
    let context = stringify!(find_asset_total_quantity);
    // Registering new asset definition
    let asset_definition = {
        let __asset_definition_id = definition_id.clone();
        AssetDefinition::new(
            __asset_definition_id.clone(),
            definition_name,
            asset_spec,
            iroha_data_model::asset::AssetBalancePolicy::Global,
            Some(DomainId::try_new("wonderland", "universal")?),
        )
    };
    test_client.submit(
        Register::asset_definition(asset_definition),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )?;
    let account_count = accounts.len();
    let expected_initial_total = sum_value(initial_value, account_count)?;
    let expected_minted_total = sum_value(to_mint, account_count)?;
    let expected_burned_total = sum_value(to_burn, account_count)?;
    let expected_after_mint = expected_initial_total
        .checked_add(&expected_minted_total)
        .map_err(|_| eyre!("quantity overflow while summing minted totals"))?;
    let expected_after_burn = expected_after_mint
        .checked_sub(&expected_burned_total)
        .map_err(|_| eyre!("quantity underflow while subtracting burned totals"))?;
    assert_eq!(
        expected_total_asset_quantity, &expected_after_burn,
        "expected_total_asset_quantity should match computed totals"
    );
    let asset_ids = accounts
        .iter()
        .cloned()
        .map(|account_id| AssetId::new(definition_id.clone(), account_id))
        .collect::<Vec<_>>();
    let get_quantity = || -> Result<Option<Quantity>> {
        const MAX_ATTEMPTS: usize = 5;
        let mut last_err = None;
        for attempt in 1..=MAX_ATTEMPTS {
            let observation = (|| -> Result<Option<Quantity>> {
                let page = test_client.list_page(
                    &Collection::AssetDefinitions,
                    &ListQuery::new()
                        .limit(64)
                        .filter(field("id").eq(definition_id.to_string())),
                )?;
                eyre::ensure!(
                    page.next_cursor.is_none(),
                    "definition lookup must be complete"
                );
                eyre::ensure!(
                    page.total.is_none(),
                    "definition lookup did not request a total"
                );
                eyre::ensure!(page.items.len() <= 1, "definition ID must be unique");
                page.items
                    .into_iter()
                    .next()
                    .map(|row| {
                        let definition: DefinitionQuantity = norito::json::from_value(row)?;
                        eyre::ensure!(
                            &definition.id == definition_id,
                            "unexpected definition in exact lookup"
                        );
                        Ok(definition.total_quantity)
                    })
                    .transpose()
            })();
            match observation {
                Ok(value) => return Ok(value),
                Err(error) => last_err = Some(error),
            }
            if attempt < MAX_ATTEMPTS {
                thread::sleep(Duration::from_millis(250 * attempt as u64));
            }
        }
        Err(last_err.expect("every observation failed"))
    };
    // Assert that initial total quantity before any burns and mints is zero
    let initial_total_asset_quantity =
        wait_for_quantity(&get_quantity, &Quantity::zero(), context)?;
    assert!(initial_total_asset_quantity.is_zero());
    let mut mint_and_burn_assets: Vec<InstructionBox> = Vec::new();
    for asset_id in asset_ids.iter().cloned() {
        mint_and_burn_assets
            .push(Mint::asset_quantity(initial_value.clone(), asset_id.clone()).into());
        mint_and_burn_assets.push(Mint::asset_quantity(to_mint.clone(), asset_id.clone()).into());
        mint_and_burn_assets.push(Burn::asset_quantity(to_burn.clone(), asset_id).into());
    }
    test_client.submit_all(
        mint_and_burn_assets,
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )?;
    let observed_after_burn = wait_for_quantity(&get_quantity, &expected_after_burn, "mint+burn")?;
    // Assert that total asset quantity is equal to: `n_accounts * (initial_value + to_mint - to_burn)`
    let total_asset_quantity = observed_after_burn;
    let log_balances = |stage: &str, definition_total: &Quantity| -> Result<Quantity> {
        let page = test_client.list_page(
            &Collection::AssetHolders(definition_id.clone()),
            &ListQuery::new().limit(64),
        )?;
        eyre::ensure!(
            page.next_cursor.is_none(),
            "holder balances must fit one coherent page"
        );
        eyre::ensure!(
            page.total.is_none() && page.items.len() <= 64,
            "holder page exceeded its envelope"
        );
        let mut assets = std::collections::BTreeMap::new();
        for row in page.items {
            let holder: HolderQuantity = norito::json::from_value(row)?;
            eyre::ensure!(&holder.asset == definition_id, "holder definition mismatch");
            eyre::ensure!(
                holder.id == AssetId::new(holder.asset, holder.account_id),
                "holder identity mismatch"
            );
            eyre::ensure!(
                assets.insert(holder.id, holder.quantity).is_none(),
                "duplicate holder"
            );
        }
        let mut manual_total = Quantity::zero();
        let mut missing_accounts = Vec::new();
        println!(
            "[{stage}] balances for asset definition {definition_id} (definition total {definition_total})"
        );
        for (account_id, asset_id) in accounts.iter().zip(&asset_ids) {
            let balance = assets.get(asset_id).map_or_else(
                || {
                    missing_accounts.push(account_id.clone());
                    Quantity::zero()
                },
                Clone::clone,
            );
            println!("    account {account_id}: balance {balance}");
            manual_total = manual_total
                .checked_add(&balance)
                .map_err(|_| eyre!("quantity overflow while summing balances"))?;
        }
        if !missing_accounts.is_empty() {
            println!("[{stage}] accounts missing asset {definition_id}: {missing_accounts:?}");
        }
        println!(
            "[{stage}] manual total for {definition_id}: {manual_total} (definition total {definition_total})"
        );
        Ok(manual_total)
    };
    let manual_total =
        match sandbox::handle_result(log_balances("after burn", &total_asset_quantity), context)? {
            Some(value) => value,
            None => return Ok(()),
        };
    if manual_total != total_asset_quantity {
        println!(
            "[after burn] discrepancy for {definition_id}: manual sum {manual_total} vs aggregate {total_asset_quantity}"
        );
    }
    assert_eq!(expected_total_asset_quantity, &total_asset_quantity);
    // Unregister asset definition
    test_client.submit(
        Unregister::asset_definition(definition_id.clone()),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )?;
    let mut removed = false;
    let mut last_value: Option<Quantity> = None;
    for _ in 0..UNREGISTER_ATTEMPTS {
        match get_quantity() {
            Ok(None) => {
                removed = true;
                break;
            }
            Err(err) => {
                if let Some(reason) = sandbox::sandbox_reason(&eyre::Report::msg(err.to_string())) {
                    return Err(eyre::Report::msg(format!(
                        "sandboxed network restriction detected while running {context}: {reason}"
                    )));
                }
                return Err(err);
            }
            Ok(Some(value)) => {
                last_value = Some(value);
            }
        }
        thread::sleep(UNREGISTER_DELAY);
    }
    if !removed {
        return Err(eyre!(
            "expected no aggregate after unregister but observed {:?} for {definition_id}",
            last_value
        ));
    }
    let remaining_assets = test_client.list_page(
        &Collection::ExplorerAssets,
        &ListQuery::new()
            .limit(64)
            .filter(field("definition_id").eq(definition_id.to_string())),
    )?;
    assert!(
        remaining_assets.next_cursor.is_none(),
        "asset absence requires a complete page"
    );
    assert!(remaining_assets.total.is_none());
    assert!(
        remaining_assets.items.is_empty(),
        "expected assets for {definition_id} to be removed after unregister"
    );
    Ok(())
}
fn wait_for_quantity<F>(get_quantity: &F, expected: &Quantity, context: &str) -> Result<Quantity>
where
    F: Fn() -> Result<Option<Quantity>>,
{
    const MAX_ATTEMPTS: usize = 30; // 7.5 seconds at 250ms
    const DELAY: Duration = Duration::from_millis(250);
    let mut last_err: Option<eyre::Report> = None;
    for attempt in 1..=MAX_ATTEMPTS {
        match get_quantity() {
            Ok(Some(value)) if value == *expected => return Ok(value),
            Ok(value) => {
                last_err = Some(eyre!(
                    "{context}: expected {expected}, got {value:?} (attempt {attempt}/{MAX_ATTEMPTS})"
                ));
            }
            Err(err) => last_err = Some(err),
        }
        thread::sleep(DELAY);
    }
    Err(last_err.unwrap_or_else(|| {
        eyre!(
            "{context}: total quantity did not reach expected {expected} after {MAX_ATTEMPTS} attempts"
        )
    }))
}
fn sum_value(value: &Quantity, count: usize) -> Result<Quantity> {
    let mut total = Quantity::zero();
    for _ in 0..count {
        total = total
            .checked_add(value)
            .map_err(|_| eyre!("quantity overflow while summing quantities"))?;
    }
    Ok(total)
}
