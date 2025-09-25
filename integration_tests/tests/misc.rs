#![allow(missing_docs)]

use eyre::Result;
use iroha::{client, data_model::prelude::*};
use iroha_telemetry::metrics::Status;
use iroha_test_network::*;
use iroha_test_samples::{sample_wasm_path, ALICE_ID};
use serde_json::json;
use tokio::task::spawn_blocking;

fn status_eq_excluding_uptime_and_queue(lhs: &Status, rhs: &Status) -> bool {
    lhs.peers == rhs.peers
        && lhs.blocks == rhs.blocks
        && lhs.blocks_non_empty == rhs.blocks_non_empty
        && lhs.txs_approved == rhs.txs_approved
        && lhs.txs_rejected == rhs.txs_rejected
        && lhs.view_changes == rhs.view_changes
}

async fn check(client: &client::Client, blocks: u64) -> Result<()> {
    let status_json = reqwest::get(client.torii_url.join("/status").unwrap())
        .await?
        .json()
        .await?;

    let status_scale = {
        let client = client.clone();
        spawn_blocking(move || client.get_status()).await??
    };

    assert!(status_eq_excluding_uptime_and_queue(
        &status_json,
        &status_scale
    ));
    assert_eq!(status_json.blocks_non_empty, blocks);

    Ok(())
}

#[tokio::test]
async fn json_and_scale_statuses_equality() -> Result<()> {
    let network = NetworkBuilder::new().start().await?;
    let client = network.client();

    check(&client, 1).await?;

    {
        let client = client.clone();
        spawn_blocking(move || {
            client.submit_blocking(Register::domain(Domain::new("looking_glass".parse()?)))
        })
    }
    .await??;
    network.ensure_blocks(2).await?;

    check(&client, 2).await?;

    Ok(())
}

#[tokio::test]
async fn get_server_version() -> eyre::Result<()> {
    let network = NetworkBuilder::new().start().await?;
    let client = network.client();
    let response =
        tokio::task::spawn_blocking(move || client.get_server_version().unwrap()).await?;
    assert!(response.version.starts_with("2.0.0"));
    assert!(!response.git_sha.is_empty());
    Ok(())
}

#[tokio::test]
async fn genesis_instructions_can_address_genesis_wasm_triggers() -> eyre::Result<()> {
    let network = NetworkBuilder::new().build();

    let peer = network.peer();
    let genesis = network.genesis();
    let configs = network.config_layers();

    // modifying the genesis
    let mut genesis_json = serde_json::to_value(genesis)?;

    // putting a trigger into `wasm_triggers`
    genesis_json
        .get_mut("wasm_triggers")
        .unwrap()
        .as_array_mut()
        .unwrap()
        .push(json!({
            "id": "test_trigger",
            "action": {
                "executable": sample_wasm_path("mint_rose_trigger"),
                "repeats": "Indefinitely",
                "authority": ALICE_ID.to_owned(),
                "filter": { "Time": { "Schedule": { "start_ms": 999_999_999_999u128 } } }
            }
        }));

    // setting this trigger metadata in `instructions`
    // NOTE: it is currently not possible to set metadata in `wasm_triggers`, since
    // `GenesisWasmAction` does not include `metadata`
    genesis_json
        .get_mut("instructions")
        .unwrap()
        .as_array_mut()
        .unwrap()
        .push(json!({
            "SetKeyValue": {
                "Trigger": {
                    "object": "test_trigger",
                    "key": "test_key",
                    "value": "test_value"
                }
            }
        }));

    let genesis = serde_json::from_value(genesis_json)?;
    peer.start_checked(configs, &genesis).await?;

    let client = peer.client();
    let metadata = spawn_blocking(move || {
        client
            .query(FindTriggers)
            .filter_with(|trigger| trigger.id.eq("test_trigger".parse().unwrap()))
            .select_with(|trigger| trigger.action.metadata)
            .execute_single()
    })
    .await??;

    expect_test::expect![[r#"
        Metadata(
            {
                "test_key": Json(
                    "\"test_value\"",
                ),
            },
        )
    "#]]
    .assert_debug_eq(&metadata);

    Ok(())
}
