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
use iroha_executor_data_model::permission::{
    asset::CanTransferAsset, smart_contract::CanRegisterSmartContractCode,
};
use iroha_test_network::NetworkBuilder;
use iroha_test_samples::{
    ALICE_ID, ALICE_KEYPAIR, BOB_ID, BOB_KEYPAIR, CARPENTER_ID, load_sample_ivm,
};

const TX_TIMEOUT: Duration = Duration::from_secs(60);
const CONTRACT_CALL_ADMISSION_TIMEOUT: Duration = Duration::from_secs(30);
const CONTRACT_CALL_ADMISSION_POLL: Duration = Duration::from_millis(250);
const CONTRACT_GAS_LIMIT: u64 = 100_000;
const SAMPLE_ASSET_DEFINITION_LITERAL: &str = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM";

fn sample_asset_definition_id() -> AssetDefinitionId {
    AssetDefinitionId::parse_address_literal(SAMPLE_ASSET_DEFINITION_LITERAL)
        .expect("sample asset definition literal must parse")
}

fn amount_args(amount: u64) -> norito::json::Value {
    let mut map = norito::json::Map::new();
    map.insert("amount".to_owned(), norito::json!(amount));
    norito::json::Value::Object(map)
}

fn open_escrow_args(target_amount: u64) -> norito::json::Value {
    let mut map = norito::json::Map::new();
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
) -> Result<(String, String)> {
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
                    return Ok((last_kind, last_payload));
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
    http: &reqwest::Client,
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

    if let Some(tx_hash_hex) = deploy
        .get("tx_hash_hex")
        .and_then(norito::json::Value::as_str)
    {
        let observed = wait_for_tx_terminal_status(
            http,
            &client.torii_url,
            tx_hash_hex,
            TX_TIMEOUT,
            "deploy threshold escrow",
        )
        .await?;
        if observed.0 != "Applied" {
            return Err(eyre!(
                "deploy threshold escrow: expected `Applied`, observed `{}` for tx `{tx_hash_hex}`; payload={}",
                observed.0,
                observed.1,
            ));
        }
    } else {
        wait_for_approved_txs(client, baseline, TX_TIMEOUT, "deploy threshold escrow").await?;
    }

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
    let response = submit_contract_call_json(
        client,
        http,
        authority,
        private_key,
        contract_address,
        entrypoint,
        payload.as_ref(),
        stage,
    )
    .await?;
    let tx_hash_hex = response
        .get("tx_hash_hex")
        .and_then(norito::json::Value::as_str)
        .ok_or_else(|| eyre!("{stage}: missing tx_hash_hex in response: {response:?}"))?;
    let observed =
        wait_for_tx_terminal_status(http, &client.torii_url, tx_hash_hex, TX_TIMEOUT, stage)
            .await?;
    if observed.0 != expected_status {
        return Err(eyre!(
            "{stage}: expected `{expected_status}`, observed `{}` for tx `{tx_hash_hex}`; payload={}",
            observed.0,
            observed.1,
        ));
    }
    Ok(())
}

async fn submit_contract_call_json(
    client: &Client,
    http: &reqwest::Client,
    authority: &AccountId,
    private_key: &iroha_crypto::PrivateKey,
    contract_address: &iroha_data_model::smart_contract::ContractAddress,
    entrypoint: &str,
    payload: Option<&norito::json::Value>,
    stage: &str,
) -> Result<norito::json::Value> {
    let url = client.torii_url.join("v1/contracts/call")?;
    let deadline = Instant::now() + CONTRACT_CALL_ADMISSION_TIMEOUT;

    loop {
        let mut body = norito::json::Map::new();
        body.insert("authority".into(), authority.to_string().into());
        body.insert(
            "private_key".into(),
            norito::json::to_value(&iroha_data_model::prelude::ExposedPrivateKey(
                private_key.clone(),
            ))?,
        );
        body.insert(
            "contract_address".into(),
            norito::json::to_value(contract_address)?,
        );
        body.insert("entrypoint".into(), entrypoint.into());
        if let Some(payload) = payload {
            body.insert("payload".into(), payload.clone());
        }
        body.insert("gas_limit".into(), CONTRACT_GAS_LIMIT.into());

        let response = http
            .post(url.clone())
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .body(norito::json::to_vec(&norito::json::Value::Object(body))?)
            .send()
            .await?;
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        if status.is_success() {
            return norito::json::from_str(&text).map_err(|err| {
                eyre!("{stage}: decode contract call response: {err}; body={text}")
            });
        }

        let last_response = format!("{status}: {text}");
        if Instant::now() >= deadline {
            return Err(eyre!(
                "{stage}: contract call admission did not succeed before timeout; last response {last_response}",
            ));
        }
        tokio::time::sleep(CONTRACT_CALL_ADMISSION_POLL).await;
    }
}

async fn contract_state_values(
    http: &reqwest::Client,
    torii_url: &reqwest::Url,
    contract_address: &iroha_data_model::smart_contract::ContractAddress,
    paths: &[&str],
) -> Result<std::collections::BTreeMap<String, norito::json::Value>> {
    let mut url = torii_url.join("v1/contracts/state")?;
    let contract_address = contract_address.to_string();
    url.query_pairs_mut()
        .append_pair("contract_address", contract_address.as_str())
        .append_pair("paths", &paths.join(","))
        .append_pair("decode", "json");
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
        let value = decode_contract_state_entry_json(entry)?;
        out.insert(path.to_owned(), value);
    }
    Ok(out)
}

fn decode_contract_state_entry_json(entry: &norito::json::Value) -> Result<norito::json::Value> {
    if let Some(value_json) = entry.get("value_json").cloned() {
        return Ok(value_json);
    }

    let value_b64 = entry
        .get("value_b64")
        .and_then(norito::json::Value::as_str)
        .ok_or_else(|| eyre!("contract state entry missing value_b64: {entry:?}"))?;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(value_b64)
        .map_err(|err| eyre!("decode contract state value_b64: {err}"))?;
    let tlv = ivm::pointer_abi::validate_tlv_bytes(&bytes)
        .map_err(|err| eyre!("validate contract state TLV: {err}"))?;
    let path = entry
        .get("path")
        .and_then(norito::json::Value::as_str)
        .ok_or_else(|| eyre!("contract state entry missing path: {entry:?}"))?;
    let value = match tlv.type_id {
        ivm::pointer_abi::PointerType::AccountId => {
            let account: AccountId = norito::decode_from_bytes(tlv.payload)
                .map_err(|err| eyre!("decode AccountId contract state: {err}"))?;
            norito::json::Value::from(account.to_string())
        }
        ivm::pointer_abi::PointerType::AssetDefinitionId => {
            let asset_definition: AssetDefinitionId = norito::decode_from_bytes(tlv.payload)
                .map_err(|err| eyre!("decode AssetDefinitionId contract state: {err}"))?;
            norito::json::Value::from(asset_definition.to_string())
        }
        ivm::pointer_abi::PointerType::NoritoBytes => {
            decode_contract_state_norito_bytes(path, tlv.payload)?
        }
        other => {
            let decode_error = entry
                .get("decode_error")
                .and_then(norito::json::Value::as_str)
                .unwrap_or("unknown decode error");
            return Err(eyre!(
                "unsupported contract state fallback decode for TLV type {other:?}: {decode_error}; entry={entry:?}"
            ));
        }
    };

    Ok(value)
}

fn decode_contract_state_norito_bytes(path: &str, payload: &[u8]) -> Result<norito::json::Value> {
    match path {
        "payer_account" | "recipient_account" | "escrow_account_id" => {
            let account = decode_norito_value_with_optional_inner_tlv::<AccountId>(
                payload,
                ivm::pointer_abi::PointerType::AccountId,
                "AccountId",
            )?;
            Ok(norito::json::Value::from(account.to_string()))
        }
        "escrow_asset_definition" => {
            let asset_definition = decode_norito_value_with_optional_inner_tlv::<AssetDefinitionId>(
                payload,
                ivm::pointer_abi::PointerType::AssetDefinitionId,
                "AssetDefinitionId",
            )?;
            Ok(norito::json::Value::from(asset_definition.to_string()))
        }
        "target_amount_value" | "funded_amount_value" => {
            let numeric = decode_norito_value_with_optional_inner_tlv::<Numeric>(
                payload,
                ivm::pointer_abi::PointerType::NoritoBytes,
                "Numeric",
            )?;
            Ok(norito::json::Value::from(numeric.to_string()))
        }
        "is_open" | "is_released" | "is_refunded" => {
            let flag = decode_norito_value_with_optional_inner_tlv::<i64>(
                payload,
                ivm::pointer_abi::PointerType::NoritoBytes,
                "bool flag",
            )?;
            Ok(norito::json::Value::from(flag != 0))
        }
        _ => Err(eyre!(
            "unsupported NoritoBytes contract state fallback for path `{path}`"
        )),
    }
}

fn decode_norito_value_with_optional_inner_tlv<T>(
    payload: &[u8],
    expected_inner_type: ivm::pointer_abi::PointerType,
    label: &str,
) -> Result<T>
where
    for<'de> T: norito::NoritoDeserialize<'de>,
{
    if let Ok(value) = norito::decode_from_bytes::<T>(payload) {
        return Ok(value);
    }

    let inner = ivm::pointer_abi::validate_tlv_bytes(payload)
        .map_err(|err| eyre!("decode {label} fallback inner TLV: {err}"))?;
    if inner.type_id != expected_inner_type {
        return Err(eyre!(
            "decode {label} fallback expected inner TLV type {expected_inner_type:?}, got {:?}",
            inner.type_id
        ));
    }

    norito::decode_from_bytes(inner.payload)
        .map_err(|err| eyre!("decode {label} fallback inner payload: {err}"))
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

fn account_exists(client: &Client, account_id: &AccountId) -> Result<bool> {
    match client.query_single(FindAccountById::new(account_id.clone())) {
        Ok(_) => Ok(true),
        Err(QueryError::Validation(ValidationFail::QueryFailed(
            QueryExecutionFail::Find(FindError::Account(_)) | QueryExecutionFail::NotFound,
        ))) => Ok(false),
        Err(err) => Err(eyre!(err)),
    }
}

fn asset_definition_exists(
    client: &Client,
    asset_definition_id: &AssetDefinitionId,
) -> Result<bool> {
    match client.query_single(FindAssetDefinitionById::new(asset_definition_id.clone())) {
        Ok(_) => Ok(true),
        Err(QueryError::Validation(ValidationFail::QueryFailed(
            QueryExecutionFail::Find(FindError::AssetDefinition(_)) | QueryExecutionFail::NotFound,
        ))) => Ok(false),
        Err(err) => Err(eyre!(err)),
    }
}

async fn setup_ledger_for_sample(
    client: &Client,
    asset_definition_id: &AssetDefinitionId,
    initial_amount: u32,
) -> Result<()> {
    let mut instructions: Vec<InstructionBox> = Vec::new();
    if !account_exists(client, &BOB_ID)? {
        instructions.push(Register::account(Account::new(BOB_ID.clone())).into());
    }
    if !account_exists(client, &CARPENTER_ID)? {
        instructions.push(Register::account(Account::new(CARPENTER_ID.clone())).into());
    }
    if !asset_definition_exists(client, asset_definition_id)? {
        instructions.push(
            Register::asset_definition(
                AssetDefinition::numeric(asset_definition_id.clone())
                    .with_name("threshold_escrow_asset".to_owned()),
            )
            .into(),
        );
    }
    instructions.push(
        Mint::asset_numeric(
            initial_amount,
            AssetId::new(asset_definition_id.clone(), ALICE_ID.clone()),
        )
        .into(),
    );
    tokio::task::spawn_blocking({
        let client = client.clone();
        move || client.submit_all_blocking(instructions)
    })
    .await
    .expect("setup ledger task")?;

    let escrow_asset = AssetId::new(asset_definition_id.clone(), BOB_ID.clone());
    tokio::task::spawn_blocking({
        let client = client.clone();
        move || {
            let grant_transfer = Grant::account_permission(
                CanTransferAsset {
                    asset: escrow_asset,
                },
                ALICE_ID.clone(),
            );
            let tx = TransactionBuilder::new(client.chain.clone(), BOB_ID.clone())
                .with_instructions([grant_transfer])
                .sign(BOB_KEYPAIR.private_key());
            client.submit_transaction_blocking(&tx)
        }
    })
    .await
    .expect("grant escrow transfer permission task")?;
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
    let asset_definition_id = sample_asset_definition_id();
    setup_ledger_for_sample(&client, &asset_definition_id, 20).await?;

    let contract_address = deploy_threshold_escrow(&client, &http).await?;

    call_contract_expect_status(
        &client,
        &http,
        &ALICE_ID.clone(),
        ALICE_KEYPAIR.private_key(),
        &contract_address,
        "open_escrow",
        Some(open_escrow_args(10)),
        "Applied",
        "open_escrow",
    )
    .await?;

    let opened_state = contract_state_values(
        &http,
        &client.torii_url,
        &contract_address,
        &threshold_state_paths(),
    )
    .await?;
    assert_eq!(
        opened_state["payer_account"],
        norito::json::Value::from(ALICE_ID.to_string())
    );
    assert_eq!(
        opened_state["recipient_account"],
        norito::json::Value::from(CARPENTER_ID.to_string())
    );
    assert_eq!(
        opened_state["escrow_account_id"],
        norito::json::Value::from(BOB_ID.to_string())
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
    let recipient_asset = AssetId::new(asset_definition_id.clone(), CARPENTER_ID.clone());
    let escrow_asset = AssetId::new(asset_definition_id.clone(), BOB_ID.clone());
    assert_eq!(asset_value(&client, &alice_asset)?, Some(numeric!(16)));
    assert_eq!(asset_value(&client, &escrow_asset)?, Some(numeric!(4)));
    assert_eq!(asset_value(&client, &recipient_asset)?, None);

    let partial_state = contract_state_values(
        &http,
        &client.torii_url,
        &contract_address,
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
        &contract_address,
        &["funded_amount_value", "is_open", "is_released"],
    )
    .await?;
    assert_eq!(
        early_release_state["funded_amount_value"],
        norito::json::Value::from("4")
    );
    assert_eq!(asset_value(&client, &escrow_asset)?, Some(numeric!(4)));
    assert_eq!(asset_value(&client, &recipient_asset)?, None);

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
        &contract_address,
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
        &contract_address,
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
    assert_eq!(asset_value(&client, &recipient_asset)?, Some(numeric!(10)));
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
        Some(open_escrow_args(10)),
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
    let asset_definition_id = sample_asset_definition_id();
    setup_ledger_for_sample(&client, &asset_definition_id, 20).await?;

    let contract_address = deploy_threshold_escrow(&client, &http).await?;

    call_contract_expect_status(
        &client,
        &http,
        &ALICE_ID.clone(),
        ALICE_KEYPAIR.private_key(),
        &contract_address,
        "open_escrow",
        Some(open_escrow_args(9)),
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
    let escrow_asset = AssetId::new(asset_definition_id.clone(), BOB_ID.clone());
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
        &contract_address,
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
        Some(open_escrow_args(9)),
        "Rejected",
        "reopen_after_refund",
    )
    .await?;

    Ok(())
}
