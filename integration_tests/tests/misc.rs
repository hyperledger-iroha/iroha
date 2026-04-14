#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Miscellaneous integration coverage for status endpoints and helpers.

use eyre::Result;
use integration_tests::sandbox;
use iroha::{
    client,
    data_model::{
        account::AccountAddress,
        metadata::Metadata,
        prelude::*,
        sns::{
            DOMAIN_NAME_SUFFIX_ID, NameControllerV1, NameSelectorV1, PaymentProofV1,
            RegisterNameRequestV1,
        },
    },
};
use iroha_primitives::json::Json;
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
async fn misc_status_endpoints_smoke() -> Result<()> {
    use norito::codec::DecodeAll;

    let Some(network) = start_network_async_or_skip(
        NetworkBuilder::new().with_pipeline_time(std::time::Duration::from_secs(2)),
        stringify!(misc_status_endpoints_smoke),
    )
    .await?
    else {
        return Ok(());
    };
    let client = network.client();

    // ensure_blocks_observes_genesis_progress
    network.ensure_blocks(1).await?;

    // json_and_norito_statuses_equality
    check(&client, 1).await?;
    {
        let client = client.clone();
        spawn_blocking(move || {
            let domain: DomainId = DomainId::try_new("lookingglass", "universal")?;
            let owner = client.account.clone();
            let controller = AccountAddress::from_account_id(&owner)?;
            client.sns().register(&RegisterNameRequestV1 {
                selector: NameSelectorV1::new(DOMAIN_NAME_SUFFIX_ID, domain.to_string())?,
                owner: owner.clone(),
                controllers: vec![NameControllerV1::account(&controller)],
                term_years: 1,
                pricing_class_hint: Some(0),
                payment: PaymentProofV1 {
                    asset_id: "61CtjvNd9T3THAR65GsMVHr82Bjc".to_string(),
                    gross_amount: 120,
                    net_amount: 120,
                    settlement_tx: Json::from("mock-settlement"),
                    payer: owner,
                    signature: Json::from("mock-signature"),
                },
                governance: None,
                metadata: Metadata::default(),
            })?;
            client.submit_blocking(Register::domain(Domain::new(domain)))
        })
    }
    .await??;
    if sandbox::handle_result(
        network.ensure_blocks(2).await,
        stringify!(misc_status_endpoints_smoke),
    )?
    .is_none()
    {
        return Ok(());
    }
    check(&client, 2).await?;

    // get_server_version
    let response =
        tokio::task::spawn_blocking(move || client.get_server_version().unwrap()).await?;
    let version: u64 = response
        .parse()
        .expect("server API version should be a positive integer");
    assert!(version >= 1);

    // status_with_norito_accept_header
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
