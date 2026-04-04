#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! End-to-end coverage for the canonical threshold escrow Kotodama sample.

use std::time::{Duration, Instant};

use base64::Engine as _;
use eyre::{Result, eyre};
use integration_tests::sandbox;
use iroha::{
    client::{Client, QueryError},
    data_model::{
        ValidationFail,
        account::Account,
        asset::{AssetDefinition, AssetId},
        prelude::*,
    },
};
use iroha_data_model::query::error::{FindError, QueryExecutionFail};
use iroha_executor_data_model::permission::smart_contract::CanRegisterSmartContractCode;
use iroha_test_network::NetworkBuilder;
use iroha_test_samples::{
    ALICE_ID, ALICE_KEYPAIR, BOB_ID, BOB_KEYPAIR, gen_account_in, load_sample_ivm,
};

const TX_TIMEOUT: Duration = Duration::from_secs(60);
const CONTRACT_GAS_LIMIT: u64 = 10_000;

fn unique_asset_definition_id(test_name: &str) -> AssetDefinitionId {
    let name: Name = format!("escrow_{}", test_name.replace('-', "_"))
        .parse()
        .expect("generated asset definition name must parse");
    AssetDefinitionId::new(
        DomainId::try_new("wonderland", "universal").expect("domain id"),
        name,
    )
}

fn amount_args(amount: u64) -> norito::json::Value {
    let mut map = norito::json::Map::new();
    map.insert("amount".to_owned(), norito::json!(amount));
    norito::json::Value::Object(map)
}

fn open_escrow_args(
    recipient: &AccountId,
    escrow_id: &AccountId,
    asset_definition_id: &AssetDefinitionId,
    target_amount: u64,
) -> norito::json::Value {
    let mut map = norito::json::Map::new();
    map.insert("recipient".to_owned(), norito::json!(recipient.to_string()));
    map.insert(
        "escrow_account".to_owned(),
        norito::json!(escrow_id.to_string()),
    );
    map.insert(
        "asset_definition".to_owned(),
        norito::json!(asset_definition_id.to_string()),
    );
    map.insert("target_amount".to_owned(), norito::json!(target_amount));
    norito::json::Value::Object(map)
}

fn pipeline_status_kind(payload: &norito::json::Value) -> Option<&str> {
    let status = payload
        .get("content")
        .and_then(|content| content.get("status"))
        .or_else(|| payload.get("status"))?;
    match status {
        norito::json::Value::String(kind) => Some(kind.as_str()),
        norito::json::Value::Object(map) => map.get("kind").and_then(norito::json::Value::as_str),
        _ => None,
    }
}

async fn wait_for_approved_txs(
    client: &Client,
    baseline: u64,
    timeout: Duration,
    stage: &str,
) -> Result<()> {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        let status = tokio::task::spawn_blocking({
            let client = client.clone();
            move || client.get_status()
        })
        .await
        .expect("poll status")?;
        if status.txs_approved > baseline {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }

    Err(eyre!(
        "{stage}: timed out waiting for txs_approved to advance beyond {baseline}"
    ))
}

async fn wait_for_tx_terminal_status(
    http: &reqwest::Client,
    torii_url: &reqwest::Url,
    tx_hash_hex: &str,
    timeout: Duration,
    stage: &str,
) -> Result<String> {
    let mut status_url = torii_url.join("v1/pipeline/transactions/status")?;
    status_url
        .query_pairs_mut()
        .append_pair("hash", tx_hash_hex);
    let deadline = Instant::now() + timeout;
    let mut last_payload = String::new();
    let mut last_kind = String::from("pending");

    loop {
        let response = http.get(status_url.clone()).send().await?;
        let status = response.status();
        let bytes = response.bytes().await?;
        if status == reqwest::StatusCode::OK || status == reqwest::StatusCode::ACCEPTED {
            let payload: norito::json::Value = norito::json::from_slice(&bytes)?;
            last_payload = format!("{payload:?}");
            if let Some(kind) = pipeline_status_kind(&payload) {
                last_kind = kind.to_owned();
                if matches!(kind, "Applied" | "Rejected" | "Expired") {
                    return Ok(last_kind);
                }
            }
        }

        if Instant::now() >= deadline {
            return Err(eyre!(
                "{stage}: timed out waiting for tx `{tx_hash_hex}` to finish; last_kind={last_kind}; last_payload={last_payload}"
            ));
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

async fn deploy_threshold_escrow(
    client: &Client,
) -> Result<iroha_data_model::smart_contract::ContractAddress> {
    let baseline = client.get_status()?.txs_approved;
    let code_b64 = base64::engine::general_purpose::STANDARD
        .encode(load_sample_ivm("threshold_escrow").as_ref());
    let contract_alias = iroha_data_model::smart_contract::ContractAlias::from_components(
        "threshold_escrow",
        None,
        "universal",
    )
    .expect("threshold escrow alias");
    let deploy = tokio::task::spawn_blocking({
        let client = client.clone();
        let contract_alias = contract_alias.clone();
        move || {
            client.post_contract_deploy_json(
                &ALICE_ID.clone(),
                ALICE_KEYPAIR.private_key(),
                &code_b64,
                &contract_alias,
                None,
            )
        }
    })
    .await
    .expect("deploy threshold escrow task")?;

    wait_for_approved_txs(client, baseline, TX_TIMEOUT, "deploy threshold escrow").await?;

    deploy
        .get("contract_address")
        .and_then(norito::json::Value::as_str)
        .ok_or_else(|| eyre!("deploy response missing contract_address: {deploy:?}"))?
        .parse()
        .map_err(|err| eyre!("invalid contract address in deploy response: {err}"))
}

async fn call_contract_expect_status(
    client: &Client,
    http: &reqwest::Client,
    authority: &AccountId,
    private_key: &iroha_crypto::PrivateKey,
    contract_address: &iroha_data_model::smart_contract::ContractAddress,
    entrypoint: &str,
    payload: Option<norito::json::Value>,
    expected_status: &str,
    stage: &str,
) -> Result<()> {
    let response = tokio::task::spawn_blocking({
        let client = client.clone();
        let authority = authority.clone();
        let private_key = private_key.clone();
        let contract_address = contract_address.clone();
        let entrypoint = entrypoint.to_owned();
        move || {
            client.post_contract_call_json(
                &authority,
                Some(&private_key),
                Some(&contract_address),
                None,
                Some(entrypoint.as_str()),
                payload.as_ref(),
                None,
                None,
                None,
                CONTRACT_GAS_LIMIT,
            )
        }
    })
    .await
    .expect("contract call task")?;

    let tx_hash_hex = response
        .get("tx_hash_hex")
        .and_then(norito::json::Value::as_str)
        .ok_or_else(|| eyre!("{stage}: missing tx_hash_hex in response: {response:?}"))?;
    let observed =
        wait_for_tx_terminal_status(http, &client.torii_url, tx_hash_hex, TX_TIMEOUT, stage)
            .await?;
    if observed != expected_status {
        return Err(eyre!(
            "{stage}: expected `{expected_status}`, observed `{observed}` for tx `{tx_hash_hex}`"
        ));
    }
    Ok(())
}

async fn contract_state_values(
    http: &reqwest::Client,
    torii_url: &reqwest::Url,
    paths: &[&str],
) -> Result<std::collections::BTreeMap<String, norito::json::Value>> {
    let mut url = torii_url.join("v1/contracts/state")?;
    url.query_pairs_mut()
        .append_pair("paths", &paths.join(","))
        .append_pair("decode", "json")
        .append_pair("include_value", "false");
    let response = http
        .get(url)
        .header("Accept", "application/json")
        .send()
        .await?;
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(eyre!("contract state request returned {status}: {body}"));
    }
    let payload: norito::json::Value = norito::json::from_str(&response.text().await?)?;
    let entries = payload
        .get("entries")
        .and_then(norito::json::Value::as_array)
        .ok_or_else(|| eyre!("contract state response missing entries: {payload:?}"))?;
    let mut out = std::collections::BTreeMap::new();
    for entry in entries {
        let found = entry
            .get("found")
            .and_then(norito::json::Value::as_bool)
            .unwrap_or(false);
        if !found {
            return Err(eyre!("contract state entry not found: {entry:?}"));
        }
        let path = entry
            .get("path")
            .and_then(norito::json::Value::as_str)
            .ok_or_else(|| eyre!("contract state entry missing path: {entry:?}"))?;
        let value = entry
            .get("value_json")
            .cloned()
            .ok_or_else(|| eyre!("contract state entry missing value_json: {entry:?}"))?;
        out.insert(path.to_owned(), value);
    }
    Ok(out)
}

fn asset_value(client: &Client, asset_id: &AssetId) -> Result<Option<Numeric>> {
    match client.query_single(FindAssetById::new(asset_id.clone())) {
        Ok(asset) => Ok(Some(asset.value().clone())),
        Err(QueryError::Validation(ValidationFail::QueryFailed(
            QueryExecutionFail::Find(FindError::Asset(_)) | QueryExecutionFail::NotFound,
        ))) => Ok(None),
        Err(err) => Err(eyre!(err)),
    }
}

async fn setup_ledger_for_sample(
    client: &Client,
    escrow_id: &AccountId,
    asset_definition_id: &AssetDefinitionId,
    initial_amount: u32,
) -> Result<()> {
    let instructions: [InstructionBox; 3] = [
        Register::account(Account::new(escrow_id.clone())).into(),
        Register::asset_definition(
            AssetDefinition::numeric(asset_definition_id.clone())
                .with_name(asset_definition_id.name().to_string()),
        )
        .into(),
        Mint::asset_numeric(
            initial_amount,
            AssetId::new(asset_definition_id.clone(), ALICE_ID.clone()),
        )
        .into(),
    ];
    tokio::task::spawn_blocking({
        let client = client.clone();
        move || client.submit_all_blocking(instructions)
    })
    .await
    .expect("setup ledger task")?;
    Ok(())
}

fn threshold_state_paths() -> [&'static str; 9] {
    [
        "payer_account",
        "recipient_account",
        "escrow_account_id",
        "escrow_asset_definition",
        "target_amount_value",
        "funded_amount_value",
        "is_open",
        "is_released",
        "is_refunded",
    ]
}

#[tokio::test]
async fn threshold_escrow_releases_when_fully_funded() -> Result<()> {
    let permission: Permission = CanRegisterSmartContractCode.into();
    let builder = NetworkBuilder::new()
        .with_min_peers(4)
        .with_pipeline_time(Duration::from_secs(4))
        .with_genesis_instruction(Grant::account_permission(permission, ALICE_ID.clone()));
    let Some(network) = sandbox::start_network_async_or_skip(
        builder,
        stringify!(threshold_escrow_releases_when_fully_funded),
    )
    .await?
    else {
        return Ok(());
    };

    network.ensure_blocks(1).await?;
    let client = network.client();
    let http = reqwest::Client::new();
    let (escrow_id, _escrow_keypair) = gen_account_in("wonderland");
    let asset_definition_id = unique_asset_definition_id("release_flow");
    setup_ledger_for_sample(&client, &escrow_id, &asset_definition_id, 20).await?;

    let contract_address = deploy_threshold_escrow(&client).await?;

    call_contract_expect_status(
        &client,
        &http,
        &ALICE_ID.clone(),
        ALICE_KEYPAIR.private_key(),
        &contract_address,
        "open_escrow",
        Some(open_escrow_args(
            &BOB_ID,
            &escrow_id,
            &asset_definition_id,
            10,
        )),
        "Applied",
        "open_escrow",
    )
    .await?;

    let opened_state =
        contract_state_values(&http, &client.torii_url, &threshold_state_paths()).await?;
    assert_eq!(
        opened_state["payer_account"],
        norito::json::Value::from(ALICE_ID.to_string())
    );
    assert_eq!(
        opened_state["recipient_account"],
        norito::json::Value::from(BOB_ID.to_string())
    );
    assert_eq!(
        opened_state["escrow_account_id"],
        norito::json::Value::from(escrow_id.to_string())
    );
    assert_eq!(
        opened_state["escrow_asset_definition"],
        norito::json::Value::from(asset_definition_id.to_string())
    );
    assert_eq!(
        opened_state["target_amount_value"],
        norito::json::Value::from("10")
    );
    assert_eq!(
        opened_state["funded_amount_value"],
        norito::json::Value::from("0")
    );
    assert_eq!(opened_state["is_open"], norito::json::Value::from(true));
    assert_eq!(
        opened_state["is_released"],
        norito::json::Value::from(false)
    );
    assert_eq!(
        opened_state["is_refunded"],
        norito::json::Value::from(false)
    );

    call_contract_expect_status(
        &client,
        &http,
        &ALICE_ID.clone(),
        ALICE_KEYPAIR.private_key(),
        &contract_address,
        "deposit",
        Some(amount_args(4)),
        "Applied",
        "deposit_partial",
    )
    .await?;

    let alice_asset = AssetId::new(asset_definition_id.clone(), ALICE_ID.clone());
    let bob_asset = AssetId::new(asset_definition_id.clone(), BOB_ID.clone());
    let escrow_asset = AssetId::new(asset_definition_id.clone(), escrow_id.clone());
    assert_eq!(asset_value(&client, &alice_asset)?, Some(numeric!(16)));
    assert_eq!(asset_value(&client, &escrow_asset)?, Some(numeric!(4)));
    assert_eq!(asset_value(&client, &bob_asset)?, None);

    let partial_state = contract_state_values(
        &http,
        &client.torii_url,
        &[
            "funded_amount_value",
            "is_open",
            "is_released",
            "is_refunded",
        ],
    )
    .await?;
    assert_eq!(
        partial_state["funded_amount_value"],
        norito::json::Value::from("4")
    );
    assert_eq!(partial_state["is_open"], norito::json::Value::from(true));
    assert_eq!(
        partial_state["is_released"],
        norito::json::Value::from(false)
    );
    assert_eq!(
        partial_state["is_refunded"],
        norito::json::Value::from(false)
    );

    call_contract_expect_status(
        &client,
        &http,
        &BOB_ID.clone(),
        BOB_KEYPAIR.private_key(),
        &contract_address,
        "deposit",
        Some(amount_args(1)),
        "Rejected",
        "deposit_by_non_payer",
    )
    .await?;

    call_contract_expect_status(
        &client,
        &http,
        &ALICE_ID.clone(),
        ALICE_KEYPAIR.private_key(),
        &contract_address,
        "deposit",
        Some(amount_args(7)),
        "Rejected",
        "deposit_over_target",
    )
    .await?;

    call_contract_expect_status(
        &client,
        &http,
        &ALICE_ID.clone(),
        ALICE_KEYPAIR.private_key(),
        &contract_address,
        "release_if_ready",
        None,
        "Rejected",
        "release_too_early",
    )
    .await?;

    let early_release_state = contract_state_values(
        &http,
        &client.torii_url,
        &["funded_amount_value", "is_open", "is_released"],
    )
    .await?;
    assert_eq!(
        early_release_state["funded_amount_value"],
        norito::json::Value::from("4")
    );
    assert_eq!(asset_value(&client, &escrow_asset)?, Some(numeric!(4)));
    assert_eq!(asset_value(&client, &bob_asset)?, None);

    call_contract_expect_status(
        &client,
        &http,
        &ALICE_ID.clone(),
        ALICE_KEYPAIR.private_key(),
        &contract_address,
        "deposit",
        Some(amount_args(6)),
        "Applied",
        "deposit_remainder",
    )
    .await?;

    let funded_state = contract_state_values(
        &http,
        &client.torii_url,
        &["funded_amount_value", "is_open"],
    )
    .await?;
    assert_eq!(
        funded_state["funded_amount_value"],
        norito::json::Value::from("10")
    );
    assert_eq!(funded_state["is_open"], norito::json::Value::from(true));
    assert_eq!(asset_value(&client, &alice_asset)?, Some(numeric!(10)));
    assert_eq!(asset_value(&client, &escrow_asset)?, Some(numeric!(10)));

    call_contract_expect_status(
        &client,
        &http,
        &ALICE_ID.clone(),
        ALICE_KEYPAIR.private_key(),
        &contract_address,
        "release_if_ready",
        None,
        "Applied",
        "release_if_ready",
    )
    .await?;

    let released_state = contract_state_values(
        &http,
        &client.torii_url,
        &[
            "funded_amount_value",
            "is_open",
            "is_released",
            "is_refunded",
        ],
    )
    .await?;
    assert_eq!(
        released_state["funded_amount_value"],
        norito::json::Value::from("10")
    );
    assert_eq!(released_state["is_open"], norito::json::Value::from(false));
    assert_eq!(
        released_state["is_released"],
        norito::json::Value::from(true)
    );
    assert_eq!(
        released_state["is_refunded"],
        norito::json::Value::from(false)
    );
    assert_eq!(asset_value(&client, &alice_asset)?, Some(numeric!(10)));
    assert_eq!(asset_value(&client, &bob_asset)?, Some(numeric!(10)));
    assert_eq!(asset_value(&client, &escrow_asset)?, None);

    call_contract_expect_status(
        &client,
        &http,
        &ALICE_ID.clone(),
        ALICE_KEYPAIR.private_key(),
        &contract_address,
        "deposit",
        Some(amount_args(1)),
        "Rejected",
        "deposit_after_release",
    )
    .await?;
    call_contract_expect_status(
        &client,
        &http,
        &ALICE_ID.clone(),
        ALICE_KEYPAIR.private_key(),
        &contract_address,
        "release_if_ready",
        None,
        "Rejected",
        "release_after_release",
    )
    .await?;
    call_contract_expect_status(
        &client,
        &http,
        &ALICE_ID.clone(),
        ALICE_KEYPAIR.private_key(),
        &contract_address,
        "refund",
        None,
        "Rejected",
        "refund_after_release",
    )
    .await?;
    call_contract_expect_status(
        &client,
        &http,
        &ALICE_ID.clone(),
        ALICE_KEYPAIR.private_key(),
        &contract_address,
        "open_escrow",
        Some(open_escrow_args(
            &BOB_ID,
            &escrow_id,
            &asset_definition_id,
            10,
        )),
        "Rejected",
        "reopen_after_release",
    )
    .await?;

    Ok(())
}

#[tokio::test]
async fn threshold_escrow_refunds_when_unresolved() -> Result<()> {
    let permission: Permission = CanRegisterSmartContractCode.into();
    let builder = NetworkBuilder::new()
        .with_min_peers(4)
        .with_pipeline_time(Duration::from_secs(4))
        .with_genesis_instruction(Grant::account_permission(permission, ALICE_ID.clone()));
    let Some(network) = sandbox::start_network_async_or_skip(
        builder,
        stringify!(threshold_escrow_refunds_when_unresolved),
    )
    .await?
    else {
        return Ok(());
    };

    network.ensure_blocks(1).await?;
    let client = network.client();
    let http = reqwest::Client::new();
    let (escrow_id, _escrow_keypair) = gen_account_in("wonderland");
    let asset_definition_id = unique_asset_definition_id("refund_flow");
    setup_ledger_for_sample(&client, &escrow_id, &asset_definition_id, 20).await?;

    let contract_address = deploy_threshold_escrow(&client).await?;

    call_contract_expect_status(
        &client,
        &http,
        &ALICE_ID.clone(),
        ALICE_KEYPAIR.private_key(),
        &contract_address,
        "open_escrow",
        Some(open_escrow_args(
            &BOB_ID,
            &escrow_id,
            &asset_definition_id,
            9,
        )),
        "Applied",
        "open_escrow",
    )
    .await?;
    call_contract_expect_status(
        &client,
        &http,
        &ALICE_ID.clone(),
        ALICE_KEYPAIR.private_key(),
        &contract_address,
        "deposit",
        Some(amount_args(3)),
        "Applied",
        "deposit_partial",
    )
    .await?;

    let alice_asset = AssetId::new(asset_definition_id.clone(), ALICE_ID.clone());
    let escrow_asset = AssetId::new(asset_definition_id.clone(), escrow_id.clone());
    assert_eq!(asset_value(&client, &alice_asset)?, Some(numeric!(17)));
    assert_eq!(asset_value(&client, &escrow_asset)?, Some(numeric!(3)));

    call_contract_expect_status(
        &client,
        &http,
        &ALICE_ID.clone(),
        ALICE_KEYPAIR.private_key(),
        &contract_address,
        "refund",
        None,
        "Applied",
        "refund",
    )
    .await?;

    let refunded_state = contract_state_values(
        &http,
        &client.torii_url,
        &[
            "funded_amount_value",
            "is_open",
            "is_released",
            "is_refunded",
        ],
    )
    .await?;
    assert_eq!(
        refunded_state["funded_amount_value"],
        norito::json::Value::from("3")
    );
    assert_eq!(refunded_state["is_open"], norito::json::Value::from(false));
    assert_eq!(
        refunded_state["is_released"],
        norito::json::Value::from(false)
    );
    assert_eq!(
        refunded_state["is_refunded"],
        norito::json::Value::from(true)
    );
    assert_eq!(asset_value(&client, &alice_asset)?, Some(numeric!(20)));
    assert_eq!(asset_value(&client, &escrow_asset)?, None);

    call_contract_expect_status(
        &client,
        &http,
        &ALICE_ID.clone(),
        ALICE_KEYPAIR.private_key(),
        &contract_address,
        "deposit",
        Some(amount_args(1)),
        "Rejected",
        "deposit_after_refund",
    )
    .await?;
    call_contract_expect_status(
        &client,
        &http,
        &ALICE_ID.clone(),
        ALICE_KEYPAIR.private_key(),
        &contract_address,
        "release_if_ready",
        None,
        "Rejected",
        "release_after_refund",
    )
    .await?;
    call_contract_expect_status(
        &client,
        &http,
        &ALICE_ID.clone(),
        ALICE_KEYPAIR.private_key(),
        &contract_address,
        "refund",
        None,
        "Rejected",
        "refund_after_refund",
    )
    .await?;
    call_contract_expect_status(
        &client,
        &http,
        &ALICE_ID.clone(),
        ALICE_KEYPAIR.private_key(),
        &contract_address,
        "open_escrow",
        Some(open_escrow_args(
            &BOB_ID,
            &escrow_id,
            &asset_definition_id,
            9,
        )),
        "Rejected",
        "reopen_after_refund",
    )
    .await?;

    Ok(())
}
