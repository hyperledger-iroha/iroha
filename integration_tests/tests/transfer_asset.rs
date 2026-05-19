#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Tests for transferring assets between accounts.

use std::time::{Duration, Instant};

use integration_tests::{sandbox, sync::sync_after_submission};
use iroha::{
    client::Client,
    data_model::{
        Registered,
        account::{Account, AccountId},
        asset::{Asset, AssetDefinition},
        isi::{Instruction, InstructionBox},
        prelude::*,
    },
};
use iroha_test_network::*;
use iroha_test_samples::{ALICE_ID, gen_account_in};

fn start_default(
    context: &'static str,
) -> Option<(sandbox::SerializedNetwork, tokio::runtime::Runtime)> {
    sandbox::start_network_blocking_or_skip(NetworkBuilder::new(), context).unwrap()
}

#[test]
// This test suite is also covered at the UI level in the iroha_cli tests
// in test_tranfer_assets.py
fn simulate_transfer_numeric() {
    simulate_transfer(
        "simulate_transfer_numeric",
        numeric!(200),
        &numeric!(20),
        |id| {
            let name = id.name().to_string();
            AssetDefinition::numeric(id).with_name(name)
        },
        Mint::asset_numeric,
        Transfer::asset_numeric,
    )
}

fn wait_for_asset_value(
    client: &Client,
    asset_definition_id: &AssetDefinitionId,
    account_id: &AccountId,
    expected_value: &Numeric,
    context: &str,
) {
    const POLL_INTERVAL: Duration = Duration::from_millis(100);
    const TIMEOUT: Duration = Duration::from_secs(30);

    let deadline = Instant::now() + TIMEOUT;
    let mut last_observed = "assets were not queried".to_owned();

    while Instant::now() < deadline {
        match client.query(FindAssets::new()).execute_all() {
            Ok(assets) => {
                let mut matching_values = Vec::new();
                for asset in assets {
                    if asset.id().definition() == asset_definition_id
                        && asset.id().account() == account_id
                    {
                        matching_values.push(asset.value().clone());
                    }
                }
                let present = matching_values.iter().any(|value| value == expected_value);
                last_observed = format!("matching_values={matching_values:?}");
                if present {
                    return;
                }
            }
            Err(err) => {
                last_observed = format!("query failed: {err}");
            }
        }

        std::thread::sleep(POLL_INTERVAL);
    }

    panic!(
        "timed out waiting for transferred asset after {context}; expected_value={expected_value:?}; last_observed={last_observed}"
    );
}

fn simulate_transfer<T>(
    context: &'static str,
    starting_amount: T,
    amount_to_transfer: &T,
    asset_definition_ctr: impl FnOnce(AssetDefinitionId) -> <AssetDefinition as Registered>::With,
    mint_ctr: impl FnOnce(T, AssetId) -> Mint<Numeric, Asset>,
    transfer_ctr: impl FnOnce(AssetId, T, AccountId) -> Transfer<Asset, T, Account>,
) where
    T: std::fmt::Debug + Clone + Into<Numeric>,
    Mint<T, Asset>: Instruction,
    Transfer<Asset, T, Account>: Instruction,
    InstructionBox: From<Transfer<Asset, T, Account>>,
{
    let Some((network, rt)) = start_default(context) else {
        return;
    };
    let iroha = network.client();
    let mut status = iroha.get_status().expect("failed to read initial status");
    let mut last_non_empty_height = status.blocks_non_empty;

    let (alice_id, mouse_id) = generate_two_ids();
    let create_mouse = create_mouse(mouse_id.clone());
    let asset_definition_id = asset_definition_id_for(context);
    let create_asset =
        Register::asset_definition(asset_definition_ctr(asset_definition_id.clone()));
    let mint_asset = mint_ctr(
        starting_amount,
        AssetId::new(asset_definition_id.clone(), alice_id.clone()),
    );

    let instructions: [InstructionBox; 3] = [
        // create_alice.into(), We don't need to register Alice, because she is created in genesis
        create_mouse.into(),
        create_asset.into(),
        mint_asset.into(),
    ];
    iroha
        .submit_all_blocking(instructions)
        .expect("Failed to prepare state.");
    status = sync_after_submission(
        &network,
        &rt,
        &iroha,
        last_non_empty_height,
        "prepare transfer asset state",
    )
    .expect("failed to synchronize after transfer asset state preparation");
    last_non_empty_height = status.blocks_non_empty;

    //When
    let transfer_asset = transfer_ctr(
        AssetId::new(asset_definition_id.clone(), alice_id),
        amount_to_transfer.clone(),
        mouse_id.clone(),
    );
    iroha
        .submit_blocking(transfer_asset)
        .expect("Failed to transfer asset.");
    let _status = sync_after_submission(
        &network,
        &rt,
        &iroha,
        last_non_empty_height,
        "transfer asset",
    )
    .expect("failed to synchronize after asset transfer");
    let expected_value: Numeric = amount_to_transfer.clone().into();
    wait_for_asset_value(
        &iroha,
        &asset_definition_id,
        &mouse_id,
        &expected_value,
        "asset transfer",
    );
}

fn generate_two_ids() -> (AccountId, AccountId) {
    let alice_id = ALICE_ID.clone();
    let (mouse_id, _mouse_keypair) = gen_account_in("wonderland");
    (alice_id, mouse_id)
}

fn create_mouse(mouse_id: AccountId) -> Register<Account> {
    Register::account(Account::new(mouse_id.clone()))
}

fn asset_definition_id_for(context: &str) -> AssetDefinitionId {
    let name = format!("camomile_{context}");
    AssetDefinitionId::new(
        DomainId::try_new("wonderland", "universal").expect("domain id should be valid"),
        name.parse().expect("asset name should be valid"),
    )
}

#[test]
fn should_fail_if_asset_not_found() {
    let context = "should_fail_if_asset_not_found";
    let Some((network, _rt)) = start_default(context) else {
        return;
    };
    let iroha = network.client();

    let (alice_id, mouse_id) = generate_two_ids();
    let asset_definition_id = asset_definition_id_for(context);
    let create_asset_definition = Register::asset_definition(
        AssetDefinition::numeric(asset_definition_id.clone())
            .with_name(asset_definition_id.name().to_string()),
    );
    let asset_id = AssetId::new(asset_definition_id.clone(), alice_id);
    let transfer_asset = Transfer::asset_numeric(asset_id.clone(), numeric!(20), mouse_id.clone());

    let instructions: [InstructionBox; 2] = [create_asset_definition.into(), transfer_asset.into()];
    let result = iroha.submit_all_blocking(instructions);

    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .chain()
            .any(|e| e.to_string() == format!("Failed to find asset: `{asset_id}`"))
    );
}
