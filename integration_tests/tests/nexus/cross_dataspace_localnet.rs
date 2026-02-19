#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Localnet cross-dataspace atomic swap regression test.

use std::{
    thread,
    time::{Duration, Instant},
};

use eyre::{Result, eyre};
use integration_tests::sandbox;
use iroha::{
    client::Client,
    data_model::{
        Level,
        asset::{AssetDefinition, AssetDefinitionId, AssetId},
        block::consensus::SumeragiStatusWire,
        isi::{
            Grant, InstructionBox, Log, Mint, Register,
            settlement::{
                DvpIsi, SettlementAtomicity, SettlementExecutionOrder, SettlementLeg,
                SettlementPlan,
            },
        },
        nexus::DataSpaceId,
        prelude::{FindAssets, Numeric, QueryBuilderExt},
    },
};
use iroha_executor_data_model::permission::{
    asset::CanTransferAssetWithDefinition, asset_definition::CanRegisterAssetDefinition,
};
use iroha_test_network::NetworkBuilder;
use iroha_test_samples::{ALICE_ID, BOB_ID, BOB_KEYPAIR};
use toml::{Table, Value};

const DS1_ALIAS: &str = "ds1";
const DS2_ALIAS: &str = "ds2";
const DS1_ID_U64: u64 = 1;
const DS2_ID_U64: u64 = 2;
const STATUS_WAIT_TIMEOUT: Duration = Duration::from_secs(45);
const STATUS_POLL_INTERVAL: Duration = Duration::from_millis(200);

fn localnet_builder() -> NetworkBuilder {
    NetworkBuilder::new()
        .with_peers(4)
        .with_config_layer(|layer| {
            let mut lane_ds1 = Table::new();
            lane_ds1.insert("index".into(), Value::Integer(0));
            lane_ds1.insert("alias".into(), Value::String("lane-ds1".to_owned()));
            lane_ds1.insert("dataspace".into(), Value::String(DS1_ALIAS.to_owned()));
            lane_ds1.insert("visibility".into(), Value::String("restricted".to_owned()));
            lane_ds1.insert("metadata".into(), Value::Table(Table::new()));

            let mut lane_ds2 = Table::new();
            lane_ds2.insert("index".into(), Value::Integer(1));
            lane_ds2.insert("alias".into(), Value::String("lane-ds2".to_owned()));
            lane_ds2.insert("dataspace".into(), Value::String(DS2_ALIAS.to_owned()));
            lane_ds2.insert("visibility".into(), Value::String("restricted".to_owned()));
            lane_ds2.insert("metadata".into(), Value::Table(Table::new()));

            let mut ds1 = Table::new();
            ds1.insert("alias".into(), Value::String(DS1_ALIAS.to_owned()));
            ds1.insert("id".into(), Value::Integer(DS1_ID_U64 as i64));
            ds1.insert(
                "description".into(),
                Value::String("private dataspace one".to_owned()),
            );
            ds1.insert("fault_tolerance".into(), Value::Integer(1));

            let mut ds2 = Table::new();
            ds2.insert("alias".into(), Value::String(DS2_ALIAS.to_owned()));
            ds2.insert("id".into(), Value::Integer(DS2_ID_U64 as i64));
            ds2.insert(
                "description".into(),
                Value::String("private dataspace two".to_owned()),
            );
            ds2.insert("fault_tolerance".into(), Value::Integer(1));

            let mut matcher_alice = Table::new();
            matcher_alice.insert("account".into(), Value::String(ALICE_ID.to_string()));
            let mut rule_alice = Table::new();
            rule_alice.insert("lane".into(), Value::Integer(0));
            rule_alice.insert("dataspace".into(), Value::String(DS1_ALIAS.to_owned()));
            rule_alice.insert("matcher".into(), Value::Table(matcher_alice));

            let mut matcher_bob = Table::new();
            matcher_bob.insert("account".into(), Value::String(BOB_ID.to_string()));
            let mut rule_bob = Table::new();
            rule_bob.insert("lane".into(), Value::Integer(1));
            rule_bob.insert("dataspace".into(), Value::String(DS2_ALIAS.to_owned()));
            rule_bob.insert("matcher".into(), Value::Table(matcher_bob));

            let mut policy = Table::new();
            policy.insert("default_lane".into(), Value::Integer(0));
            policy.insert(
                "default_dataspace".into(),
                Value::String(DS1_ALIAS.to_owned()),
            );
            policy.insert(
                "rules".into(),
                Value::Array(vec![Value::Table(rule_alice), Value::Table(rule_bob)]),
            );

            layer
                .write(["nexus", "enabled"], true)
                .write(["nexus", "lane_count"], 2_i64)
                .write(
                    ["nexus", "lane_catalog"],
                    Value::Array(vec![Value::Table(lane_ds1), Value::Table(lane_ds2)]),
                )
                .write(
                    ["nexus", "dataspace_catalog"],
                    Value::Array(vec![Value::Table(ds1), Value::Table(ds2)]),
                )
                .write(["nexus", "routing_policy"], Value::Table(policy))
                .write(
                    ["nexus", "staking", "restricted_validator_mode"],
                    "stake_elected",
                );
        })
}

fn wait_for_height(
    client: &Client,
    target_height: u64,
    context: &str,
) -> Result<SumeragiStatusWire> {
    let started = Instant::now();
    let mut last_height = 0;
    while started.elapsed() <= STATUS_WAIT_TIMEOUT {
        let status = client
            .get_sumeragi_status_wire()
            .map_err(|err| eyre!(err))?;
        last_height = status.commit_qc.height;
        if status.commit_qc.height >= target_height {
            return Ok(status);
        }
        thread::sleep(STATUS_POLL_INTERVAL);
    }
    Err(eyre!(
        "{context}: timed out waiting for block height >= {target_height}; last observed {last_height}"
    ))
}

fn asset_balance(client: &Client, asset_id: &AssetId) -> Result<Numeric> {
    let assets = client
        .query(FindAssets::new())
        .execute_all()
        .map_err(|err| eyre!(err))?;
    Ok(assets
        .into_iter()
        .find(|asset| &asset.id == asset_id)
        .map_or_else(Numeric::zero, |asset| asset.value().clone()))
}

fn has_dataspace_commitment(status: &SumeragiStatusWire, dataspace_id: u64) -> bool {
    let expected = DataSpaceId::new(dataspace_id);
    status
        .dataspace_commitments
        .iter()
        .any(|entry| entry.dataspace_id == expected && entry.tx_count > 0)
}

fn wait_for_dataspace_commitment(
    client: &Client,
    min_height: u64,
    dataspace_id: u64,
    context: &str,
) -> Result<SumeragiStatusWire> {
    let started = Instant::now();
    let mut last_height = 0;
    let mut last_commitments = String::new();
    while started.elapsed() <= STATUS_WAIT_TIMEOUT {
        let status = client
            .get_sumeragi_status_wire()
            .map_err(|err| eyre!(err))?;
        last_height = status.commit_qc.height;
        last_commitments = format!("{:?}", status.dataspace_commitments);
        if status.commit_qc.height >= min_height && has_dataspace_commitment(&status, dataspace_id)
        {
            return Ok(status);
        }
        thread::sleep(STATUS_POLL_INTERVAL);
    }
    Err(eyre!(
        "{context}: timed out waiting for dataspace commitment {dataspace_id} at or above height {min_height}; last height {last_height}, last commitments {last_commitments}"
    ))
}

fn wait_for_expected_balances(
    client: &Client,
    expectations: &[(&AssetId, Numeric)],
    context: &str,
) -> Result<()> {
    let started = Instant::now();
    let mut last_observed = Vec::with_capacity(expectations.len());
    while started.elapsed() <= STATUS_WAIT_TIMEOUT {
        last_observed.clear();
        let mut all_match = true;
        for (asset_id, expected) in expectations {
            let observed = asset_balance(client, asset_id)?;
            if observed != *expected {
                all_match = false;
            }
            last_observed.push(((*asset_id).clone(), observed));
        }
        if all_match {
            return Ok(());
        }
        thread::sleep(STATUS_POLL_INTERVAL);
    }
    Err(eyre!(
        "{context}: timed out waiting for expected balances; last observed {last_observed:?}"
    ))
}

#[test]
fn cross_dataspace_atomic_swap_is_all_or_nothing() -> Result<()> {
    let context = stringify!(cross_dataspace_atomic_swap_is_all_or_nothing);
    let Some((network, _rt)) =
        sandbox::start_network_blocking_or_skip(localnet_builder(), context)?
    else {
        return Ok(());
    };

    let alice = network.client();
    let bob = network
        .peer()
        .client_for(&BOB_ID, BOB_KEYPAIR.private_key().clone());

    let initial_height = alice
        .get_sumeragi_status_wire()
        .map_err(|err| eyre!(err))?
        .commit_qc
        .height;

    alice.submit_blocking(Log::new(Level::INFO, "route probe ds1".to_string()))?;
    let ds1_status =
        wait_for_dataspace_commitment(&alice, initial_height + 1, DS1_ID_U64, "ds1 route probe")?;
    let _ds1_status_bob = wait_for_height(&bob, ds1_status.commit_qc.height, "ds1 probe on bob")?;

    bob.submit_blocking(Log::new(Level::INFO, "route probe ds2".to_string()))?;
    let _ds2_status = wait_for_dataspace_commitment(
        &alice,
        ds1_status.commit_qc.height + 1,
        DS2_ID_U64,
        "ds2 route probe",
    )?;

    let ds1_asset_def: AssetDefinitionId = "ds1coin#wonderland".parse().expect("asset definition");
    let ds2_asset_def: AssetDefinitionId = "ds2coin#wonderland".parse().expect("asset definition");
    let wonderland_domain = "wonderland".parse().expect("domain id");

    alice.submit_blocking(Grant::account_permission(
        CanRegisterAssetDefinition {
            domain: wonderland_domain,
        },
        BOB_ID.clone(),
    ))?;

    alice.submit_blocking(Register::asset_definition(AssetDefinition::numeric(
        ds1_asset_def.clone(),
    )))?;
    bob.submit_blocking(Register::asset_definition(AssetDefinition::numeric(
        ds2_asset_def.clone(),
    )))?;

    let alice_ds1_asset = AssetId::new(ds1_asset_def.clone(), ALICE_ID.clone());
    let bob_ds1_asset = AssetId::new(ds1_asset_def.clone(), BOB_ID.clone());
    let alice_ds2_asset = AssetId::new(ds2_asset_def.clone(), ALICE_ID.clone());
    let bob_ds2_asset = AssetId::new(ds2_asset_def.clone(), BOB_ID.clone());

    alice.submit_blocking(Grant::account_permission(
        CanTransferAssetWithDefinition {
            asset_definition: ds1_asset_def.clone(),
        },
        BOB_ID.clone(),
    ))?;

    alice.submit_blocking(Mint::asset_numeric(100_u32, alice_ds1_asset.clone()))?;
    bob.submit_blocking(Mint::asset_numeric(200_u32, bob_ds2_asset.clone()))?;

    let successful_swap = DvpIsi::new(
        "ds1ds2swapok".parse().expect("settlement id"),
        SettlementLeg::new(
            ds1_asset_def.clone(),
            Numeric::from(30_u32),
            ALICE_ID.clone(),
            BOB_ID.clone(),
        ),
        SettlementLeg::new(
            ds2_asset_def.clone(),
            Numeric::from(45_u32),
            BOB_ID.clone(),
            ALICE_ID.clone(),
        ),
        SettlementPlan::new(
            SettlementExecutionOrder::DeliveryThenPayment,
            SettlementAtomicity::AllOrNothing,
        ),
    );
    alice.submit_blocking(InstructionBox::from(successful_swap))?;
    let synced_after_success = alice.get_sumeragi_status_wire().map_err(|err| eyre!(err))?;
    let _synced_after_success_bob = wait_for_height(
        &bob,
        synced_after_success.commit_qc.height,
        "successful swap propagation to bob",
    )?;
    wait_for_expected_balances(
        &alice,
        &[
            (&alice_ds1_asset, Numeric::from(70_u32)),
            (&bob_ds1_asset, Numeric::from(30_u32)),
            (&alice_ds2_asset, Numeric::from(45_u32)),
            (&bob_ds2_asset, Numeric::from(155_u32)),
        ],
        "successful swap balances",
    )?;

    assert_eq!(
        asset_balance(&alice, &alice_ds1_asset)?,
        Numeric::from(70_u32)
    );
    assert_eq!(
        asset_balance(&alice, &bob_ds1_asset)?,
        Numeric::from(30_u32)
    );
    assert_eq!(
        asset_balance(&alice, &alice_ds2_asset)?,
        Numeric::from(45_u32)
    );
    assert_eq!(
        asset_balance(&alice, &bob_ds2_asset)?,
        Numeric::from(155_u32)
    );

    let reverse_successful_swap = DvpIsi::new(
        "ds2ds1swapok".parse().expect("settlement id"),
        SettlementLeg::new(
            ds2_asset_def.clone(),
            Numeric::from(20_u32),
            BOB_ID.clone(),
            ALICE_ID.clone(),
        ),
        SettlementLeg::new(
            ds1_asset_def.clone(),
            Numeric::from(10_u32),
            ALICE_ID.clone(),
            BOB_ID.clone(),
        ),
        SettlementPlan::new(
            SettlementExecutionOrder::DeliveryThenPayment,
            SettlementAtomicity::AllOrNothing,
        ),
    );
    bob.submit_blocking(InstructionBox::from(reverse_successful_swap))?;
    let synced_after_reverse = bob.get_sumeragi_status_wire().map_err(|err| eyre!(err))?;
    let _synced_after_reverse_bob = wait_for_height(
        &alice,
        synced_after_reverse.commit_qc.height,
        "reverse swap propagation to alice",
    )?;
    wait_for_expected_balances(
        &alice,
        &[
            (&alice_ds1_asset, Numeric::from(60_u32)),
            (&bob_ds1_asset, Numeric::from(40_u32)),
            (&alice_ds2_asset, Numeric::from(65_u32)),
            (&bob_ds2_asset, Numeric::from(135_u32)),
        ],
        "reverse swap balances",
    )?;

    assert_eq!(
        asset_balance(&alice, &alice_ds1_asset)?,
        Numeric::from(60_u32)
    );
    assert_eq!(
        asset_balance(&alice, &bob_ds1_asset)?,
        Numeric::from(40_u32)
    );
    assert_eq!(
        asset_balance(&alice, &alice_ds2_asset)?,
        Numeric::from(65_u32)
    );
    assert_eq!(
        asset_balance(&alice, &bob_ds2_asset)?,
        Numeric::from(135_u32)
    );

    let failing_swap = DvpIsi::new(
        "ds1ds2swapfail".parse().expect("settlement id"),
        SettlementLeg::new(
            ds1_asset_def,
            Numeric::from(10_u32),
            ALICE_ID.clone(),
            BOB_ID.clone(),
        ),
        SettlementLeg::new(
            ds2_asset_def,
            Numeric::from(10_000_u32),
            BOB_ID.clone(),
            ALICE_ID.clone(),
        ),
        SettlementPlan::new(
            SettlementExecutionOrder::DeliveryThenPayment,
            SettlementAtomicity::AllOrNothing,
        ),
    );
    let failure = alice
        .submit_blocking(InstructionBox::from(failing_swap))
        .expect_err("underfunded counter-leg must reject all-or-nothing settlement");
    let failure_text = failure.to_string();
    assert!(
        failure_text.contains("settlement leg requires 10000")
            || failure_text.contains("requires 10000"),
        "unexpected failure message: {failure_text}"
    );
    wait_for_expected_balances(
        &alice,
        &[
            (&alice_ds1_asset, Numeric::from(60_u32)),
            (&bob_ds1_asset, Numeric::from(40_u32)),
            (&alice_ds2_asset, Numeric::from(65_u32)),
            (&bob_ds2_asset, Numeric::from(135_u32)),
        ],
        "rollback balances after failing swap",
    )?;

    assert_eq!(
        asset_balance(&alice, &alice_ds1_asset)?,
        Numeric::from(60_u32)
    );
    assert_eq!(
        asset_balance(&alice, &bob_ds1_asset)?,
        Numeric::from(40_u32)
    );
    assert_eq!(
        asset_balance(&alice, &alice_ds2_asset)?,
        Numeric::from(65_u32)
    );
    assert_eq!(
        asset_balance(&alice, &bob_ds2_asset)?,
        Numeric::from(135_u32)
    );

    Ok(())
}
