//! Miscellaneous integration coverage for status endpoints and helpers.

use eyre::Result;
use integration_tests::sandbox;
use iroha::{client, data_model::prelude::*};
use iroha_telemetry::metrics::Status;
use iroha_test_network::*;
use sandbox::start_network_async_or_skip;
use tokio::task::spawn_blocking;

fn status_eq_excluding_uptime_and_queue(lhs: &Status, rhs: &Status) -> bool {
    lhs.peers == rhs.peers
        && lhs.blocks == rhs.blocks
        && lhs.blocks_non_empty == rhs.blocks_non_empty
        && lhs.txs_approved == rhs.txs_approved
        && lhs.txs_rejected == rhs.txs_rejected
        && lhs.view_changes == rhs.view_changes
}

async fn check(client: &client::Client, min_blocks_non_empty: u64) -> Result<()> {
    let http = reqwest::Client::new();
    let body = http
        .get(client.torii_url.join("/status").unwrap())
        .header("Accept", "application/json")
        .send()
        .await?
        .text()
        .await?;
    let status_json: Status = norito::json::from_str(&body)
        .map_err(|err| eyre::Report::msg(format!("decode status JSON: {err}")))?;

    let status_norito = {
        let client = client.clone();
        spawn_blocking(move || client.get_status()).await??
    };

    assert!(status_eq_excluding_uptime_and_queue(
        &status_json,
        &status_norito
    ));
    assert!(
        status_json.blocks_non_empty >= min_blocks_non_empty,
        "expected blocks_non_empty >= {min_blocks_non_empty}, got {}",
        status_json.blocks_non_empty
    );

    Ok(())
}

#[tokio::test]
async fn ensure_blocks_observes_genesis_progress() -> Result<()> {
    let Some(network) = start_network_async_or_skip(
        NetworkBuilder::new(),
        stringify!(ensure_blocks_observes_genesis_progress),
    )
    .await?
    else {
        return Ok(());
    };

    network.ensure_blocks(1).await?;
    Ok(())
}

#[tokio::test]
async fn json_and_norito_statuses_equality() -> Result<()> {
    let Some(network) = start_network_async_or_skip(
        NetworkBuilder::new(),
        stringify!(json_and_norito_statuses_equality),
    )
    .await?
    else {
        return Ok(());
    };
    let client = network.client();

    check(&client, 1).await?;

    {
        let client = client.clone();
        spawn_blocking(move || {
            client.submit_blocking(Register::domain(Domain::new("looking_glass".parse()?)))
        })
    }
    .await??;
    if sandbox::handle_result(
        network.ensure_blocks(2).await,
        stringify!(json_and_norito_statuses_equality),
    )?
    .is_none()
    {
        return Ok(());
    }

    check(&client, 2).await?;

    Ok(())
}

#[tokio::test]
async fn get_server_version() -> eyre::Result<()> {
    let Some(network) =
        start_network_async_or_skip(NetworkBuilder::new(), stringify!(get_server_version)).await?
    else {
        return Ok(());
    };
    let client = network.client();
    let response =
        tokio::task::spawn_blocking(move || client.get_server_version().unwrap()).await?;
    let version: u64 = response
        .parse()
        .expect("server API version should be a positive integer");
    assert!(version >= 1);
    Ok(())
}

#[tokio::test]
async fn status_with_norito_accept_header() -> eyre::Result<()> {
    use norito::codec::DecodeAll;

    let Some(network) = start_network_async_or_skip(
        NetworkBuilder::new(),
        stringify!(status_with_norito_accept_header),
    )
    .await?
    else {
        return Ok(());
    };

    let http = reqwest::Client::new();
    let url = network.client().torii_url.join("/status").unwrap();
    let resp = http
        .get(url)
        .header("Accept", "application/x-norito")
        .send()
        .await?;
    assert!(resp.status().is_success());
    let bytes = resp.bytes().await?;
    let _: Status = DecodeAll::decode_all(&mut bytes.as_ref())?;
    Ok(())
}
