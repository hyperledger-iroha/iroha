//! Shared runtime-governance fixture and round helpers used by SORA integration tests.

use std::{
    collections::BTreeSet,
    time::{Duration, Instant},
};

use eyre::{Result, WrapErr, eyre};
use integration_tests::{sandbox, sync};
use iroha::client::Client;
use iroha::data_model::{
    asset::AssetDefinition,
    asset::id::AssetId,
    domain::{Domain, DomainId},
    governance::types::ParliamentBody,
    isi::governance::{
        ApproveGovernanceProposal, AtWindow, CastPlainBallot, CouncilDerivationKind,
        EnactReferendum, FinalizeReferendum, PersistCouncilForEpoch, ProposeRuntimeUpgradeProposal,
        RegisterCitizen, VotingMode,
    },
    permission::Permission,
    prelude::{
        Account, AssetDefinitionId, FindAssetById, FindAssetsDefinitions, FindDomains, Grant, Mint,
        QueryBuilderExt, Register, Transfer,
    },
    query::account::prelude::FindAccounts,
    runtime::RuntimeUpgradeManifest,
};
use iroha_crypto::KeyPair;
use iroha_executor_data_model::permission::governance::{
    CanEnactGovernance, CanProposeContractDeployment, CanSubmitGovernanceBallot,
};
use iroha_test_network::NetworkBuilder;
use iroha_test_samples::{ALICE_ID, gen_account_in};

const CITIZEN_COUNT: usize = 20;
const CITIZEN_FUND: u128 = 15_000;
const CITIZEN_BOND: u128 = 10_000;
const BALLOT_LOCK: u128 = CITIZEN_FUND - CITIZEN_BOND;
const BALLOT_DURATION_BLOCKS: u64 = 20;
const THIRD_REFERENDUM_VOTERS: usize = 8;
const THIRD_REFERENDUM_APPROVE_VOTERS: usize = 5;
const GOV_MAX_CONVICTION: u64 = 6;
const GOV_DOMAIN_ID: &str = "govsmoke";
const GOV_ASSET_ID: &str = "xor#govsmoke";
const FIRST_CONTRACT_ID: &str = "parliament.lifecycle.smoke.contract";
const SECOND_CONTRACT_ID: &str = "parliament.lifecycle.smoke.reject.contract";
const RUNTIME_UPGRADE_NAME: &str = "parliament.runtime.upgrade.smoke";
const RUNTIME_UPGRADE_DESCRIPTION: &str = "runtime upgrade governance e2e smoke";
const RUNTIME_WINDOW_DURATION_BLOCKS: u64 = 40;
const TX_STATUS_TIMEOUT: Duration = Duration::from_secs(900);
const TORII_REQUEST_TIMEOUT: Duration = Duration::from_secs(120);
const TX_TTL: Duration = Duration::from_secs(1_200);
const BALANCE_WAIT_TIMEOUT: Duration = Duration::from_secs(300);

pub fn canonical_abi_hex() -> String {
    hex::encode(ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1))
}

fn governance_escrow_account_literal() -> String {
    ALICE_ID
        .canonical_i105()
        .expect("alice account id should encode to canonical i105")
}

pub fn tune_client_timeouts(client: &mut Client) {
    client.transaction_status_timeout = TX_STATUS_TIMEOUT;
    client.torii_request_timeout = TORII_REQUEST_TIMEOUT;
    client.transaction_ttl = Some(TX_TTL);
}

async fn wait_for_ready_torii_peer(
    network: &sandbox::SerializedNetwork,
    http: &reqwest::Client,
    timeout: Duration,
) -> Result<usize> {
    let deadline = Instant::now() + timeout;
    let mut last_error = String::from("no response yet");
    loop {
        // Prefer later peers first; in this localnet smoke setup, the last-started
        // peer tends to become HTTP-ready first and remain responsive.
        for idx in (0..network.peers().len()).rev() {
            let peer = &network.peers()[idx];
            let status_url = format!("{}/status", peer.torii_url());
            match http.get(&status_url).send().await {
                Ok(response) if response.status().is_success() => return Ok(idx),
                Ok(response) => {
                    last_error = format!(
                        "peer #{idx} responded with non-success {}",
                        response.status()
                    );
                }
                Err(err) => {
                    last_error = format!("peer #{idx} status request failed: {err}");
                }
            }
        }
        if Instant::now() >= deadline {
            return Err(eyre!(
                "timed out waiting for a ready Torii peer within {:?}; last error: {}",
                timeout,
                last_error
            ));
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

pub fn parse_hex32(input: &str) -> [u8; 32] {
    let bytes = hex::decode(input).expect("hex should decode");
    let mut out = [0_u8; 32];
    out.copy_from_slice(&bytes);
    out
}

pub fn compute_runtime_upgrade_proposal_id(manifest: &RuntimeUpgradeManifest) -> [u8; 32] {
    use iroha_crypto::blake2::{Blake2b512, Digest as _};

    let canonical = norito::to_bytes(manifest).expect("runtime manifest must encode");
    let manifest_len =
        u32::try_from(canonical.len()).expect("runtime manifest length should fit u32");
    let mut input = Vec::with_capacity(
        b"iroha:gov:runtime-upgrade:proposal:v1|".len()
            + core::mem::size_of::<u32>()
            + canonical.len(),
    );
    input.extend_from_slice(b"iroha:gov:runtime-upgrade:proposal:v1|");
    input.extend_from_slice(&manifest_len.to_le_bytes());
    input.extend_from_slice(&canonical);

    let digest = Blake2b512::digest(&input);
    let mut out = [0_u8; 32];
    out.copy_from_slice(&digest[..32]);
    out
}

fn json_u128(value: &norito::json::Value) -> Option<u128> {
    value
        .as_u64()
        .map(u128::from)
        .or_else(|| value.as_str().and_then(|raw| raw.parse::<u128>().ok()))
}

fn json_u64(value: &norito::json::Value) -> Option<u64> {
    value
        .as_u64()
        .or_else(|| value.as_str().and_then(|raw| raw.parse::<u64>().ok()))
}

fn max_runtime_upgrade_end_height(client: &Client) -> Result<Option<u64>> {
    let payload = client
        .get_runtime_upgrades_json()
        .wrap_err("query runtime upgrades list")?;
    let max_end_height = payload
        .get("items")
        .and_then(norito::json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| {
            item.get("record")
                .and_then(|record| record.get("manifest"))
                .and_then(|manifest| manifest.get("end_height"))
                .and_then(json_u64)
        })
        .max();
    Ok(max_end_height)
}

fn integer_sqrt_u128(value: u128) -> u128 {
    if value < 2 {
        return value;
    }
    let mut x0 = value;
    let mut x1 = u128::midpoint(x0, value / x0);
    while x1 < x0 {
        x0 = x1;
        x1 = u128::midpoint(x0, value / x0);
    }
    x0
}

fn expected_plain_total_weight(voter_count: usize) -> u128 {
    let conviction_factor = (1_u64 + BALLOT_DURATION_BLOCKS).min(GOV_MAX_CONVICTION);
    integer_sqrt_u128(BALLOT_LOCK)
        .saturating_mul(u128::from(conviction_factor))
        .saturating_mul(u128::try_from(voter_count).expect("voter count should fit u128"))
}

pub async fn wait_for_proposal_found(
    client: &Client,
    proposal_id_hex: &str,
    timeout: Duration,
) -> Result<()> {
    let deadline = Instant::now() + timeout;
    let mut last: String;
    loop {
        match client.get_gov_proposal_json(proposal_id_hex) {
            Ok(payload) => {
                if payload.get("found").and_then(norito::json::Value::as_bool) == Some(true) {
                    return Ok(());
                }
                last = format!("proposal payload did not report found=true: {payload:?}");
            }
            Err(err) => {
                last = format!("proposal query failed: {err}");
            }
        }
        if Instant::now() >= deadline {
            return Err(eyre!(
                "timed out waiting for proposal `{proposal_id_hex}` to exist; last={last}"
            ));
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

async fn wait_for_referendum_found(
    client: &Client,
    referendum_id: &str,
    timeout: Duration,
) -> Result<()> {
    let deadline = Instant::now() + timeout;
    let mut last: String;
    loop {
        match client.get_gov_referendum_json(referendum_id) {
            Ok(payload) => {
                if payload.get("found").and_then(norito::json::Value::as_bool) == Some(true) {
                    return Ok(());
                }
                last = format!("referendum payload did not report found=true: {payload:?}");
            }
            Err(err) => {
                last = format!("referendum query failed: {err}");
            }
        }
        if Instant::now() >= deadline {
            return Err(eyre!(
                "timed out waiting for referendum `{referendum_id}` to exist; last={last}"
            ));
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

async fn wait_for_proposal_status(
    client: &Client,
    proposal_id_hex: &str,
    expected_status: &str,
    timeout: Duration,
) -> Result<()> {
    let deadline = Instant::now() + timeout;
    let mut last: String;
    loop {
        match client.get_gov_proposal_json(proposal_id_hex) {
            Ok(payload) => {
                let status = payload
                    .get("proposal")
                    .and_then(|value| value.get("status"))
                    .and_then(norito::json::Value::as_str)
                    .unwrap_or_default();
                last = status.to_string();
                if status == expected_status {
                    return Ok(());
                }
            }
            Err(err) => {
                last = format!("query failed: {err}");
            }
        }
        if Instant::now() >= deadline {
            return Err(eyre!(
                "timed out waiting for proposal `{proposal_id_hex}` status `{expected_status}`; last={last}"
            ));
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

async fn wait_for_referendum_status(
    client: &Client,
    referendum_id: &str,
    expected_status: &str,
    timeout: Duration,
) -> Result<()> {
    let deadline = Instant::now() + timeout;
    let mut last: String;
    loop {
        match client.get_gov_referendum_json(referendum_id) {
            Ok(payload) => {
                let status = payload
                    .get("referendum")
                    .and_then(|value| value.get("status"))
                    .and_then(norito::json::Value::as_str)
                    .unwrap_or_default();
                last = status.to_string();
                if status == expected_status {
                    return Ok(());
                }
            }
            Err(err) => {
                last = format!("query failed: {err}");
            }
        }
        if Instant::now() >= deadline {
            return Err(eyre!(
                "timed out waiting for referendum `{referendum_id}` status `{expected_status}`; last={last}"
            ));
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

async fn wait_for_tally_total(
    client: &Client,
    referendum_id: &str,
    expected_total: u128,
    timeout: Duration,
) -> Result<()> {
    let deadline = Instant::now() + timeout;
    let mut last_total: Option<u128> = None;
    let mut last_error: Option<String>;
    loop {
        match client.get_gov_tally_json(referendum_id) {
            Ok(payload) => {
                let approve = payload
                    .get("approve")
                    .and_then(json_u128)
                    .unwrap_or_default();
                let reject = payload
                    .get("reject")
                    .and_then(json_u128)
                    .unwrap_or_default();
                let total = approve.saturating_add(reject);
                last_total = Some(total);
                if total >= expected_total {
                    return Ok(());
                }
                last_error = None;
            }
            Err(err) => {
                last_error = Some(format!("{err}"));
            }
        }
        if Instant::now() >= deadline {
            let observed_total = last_total.unwrap_or_default();
            if let Some(last_error) = last_error {
                return Err(eyre!(
                    "timed out waiting for tally total >= {expected_total}; last_total={observed_total}; last_error={last_error}"
                ));
            }
            return Err(eyre!(
                "timed out waiting for tally total >= {expected_total}; last_total={observed_total}"
            ));
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

pub async fn wait_for_runtime_upgrade_status(
    client: &Client,
    runtime_upgrade_id_hex: &str,
    expected_status_key: &str,
    timeout: Duration,
) -> Result<norito::json::Value> {
    let deadline = Instant::now() + timeout;
    loop {
        let last = match client.get_runtime_upgrades_json() {
            Ok(payload) => {
                let status = payload
                    .get("items")
                    .and_then(norito::json::Value::as_array)
                    .and_then(|items| {
                        items.iter().find_map(|item| {
                            let id_hex = item
                                .get("id_hex")
                                .and_then(norito::json::Value::as_str)
                                .unwrap_or_default();
                            if id_hex != runtime_upgrade_id_hex {
                                return None;
                            }
                            item.get("record").and_then(|record| record.get("status"))
                        })
                    });
                if let Some(status) = status {
                    if status.get(expected_status_key).is_some() {
                        return Ok(status.clone());
                    }
                    format!("observed status={status:?}")
                } else {
                    format!(
                        "runtime upgrade `{runtime_upgrade_id_hex}` missing in payload: {payload:?}"
                    )
                }
            }
            Err(err) => format!("runtime upgrades query failed: {err}"),
        };
        if Instant::now() >= deadline {
            return Err(eyre!(
                "timed out waiting for runtime upgrade `{runtime_upgrade_id_hex}` status key `{expected_status_key}`; last={last}"
            ));
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

async fn wait_for_account_registration(
    client: &Client,
    account_id: &iroha::data_model::account::AccountId,
    timeout: Duration,
) -> Result<()> {
    let deadline = Instant::now() + timeout;
    let mut last_error: String;
    loop {
        match client.query(FindAccounts::new()).execute_all() {
            Ok(accounts) => {
                if accounts
                    .iter()
                    .any(|account: &iroha::data_model::account::Account| &account.id == account_id)
                {
                    return Ok(());
                }
                last_error = format!(
                    "account not present in FindAccounts snapshot (len={})",
                    accounts.len()
                );
            }
            Err(err) => {
                last_error = format!("{err:?}");
            }
        }
        if Instant::now() >= deadline {
            return Err(eyre!(
                "timed out waiting for account `{account_id}` registration; last_error={last_error}"
            ));
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

async fn wait_for_council_member_present(
    client: &Client,
    expected_member: &iroha::data_model::account::AccountId,
    expected_epoch: u64,
    timeout: Duration,
) -> Result<()> {
    let deadline = Instant::now() + timeout;
    let expected_member_str = expected_member.to_string();
    let mut last_error: String;
    loop {
        match client.get_gov_council_json() {
            Ok(payload) => {
                let epoch = payload
                    .get("epoch")
                    .and_then(norito::json::Value::as_u64)
                    .unwrap_or_default();
                let member_found = payload
                    .get("members")
                    .and_then(norito::json::Value::as_array)
                    .is_some_and(|members| {
                        members.iter().any(|entry| {
                            entry
                                .get("account_id")
                                .and_then(norito::json::Value::as_str)
                                .is_some_and(|account| account == expected_member_str)
                        })
                    });
                if epoch == expected_epoch && member_found {
                    return Ok(());
                }
                last_error = format!(
                    "council snapshot not ready: epoch={epoch}, member_found={member_found}, payload={payload:?}"
                );
            }
            Err(err) => {
                last_error = format!("{err:?}");
            }
        }
        if Instant::now() >= deadline {
            return Err(eyre!(
                "timed out waiting for council epoch={expected_epoch} member `{expected_member}`; last_error={last_error}"
            ));
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

async fn wait_for_domain_registration(
    client: &Client,
    domain_id: &DomainId,
    timeout: Duration,
) -> Result<()> {
    let deadline = Instant::now() + timeout;
    let mut last_error: String;
    loop {
        match client.query(FindDomains::new()).execute_all() {
            Ok(domains) => {
                if domains
                    .iter()
                    .any(|domain: &Domain| &domain.id == domain_id)
                {
                    return Ok(());
                }
                last_error = format!(
                    "domain not present in FindDomains snapshot (len={})",
                    domains.len()
                );
            }
            Err(err) => {
                last_error = format!("{err:?}");
            }
        }
        if Instant::now() >= deadline {
            return Err(eyre!(
                "timed out waiting for domain `{domain_id}` registration; last_error={last_error}"
            ));
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

async fn wait_for_asset_definition_registration(
    client: &Client,
    asset_definition_id: &AssetDefinitionId,
    timeout: Duration,
) -> Result<()> {
    let deadline = Instant::now() + timeout;
    let mut last_error: String;
    loop {
        match client.query(FindAssetsDefinitions::new()).execute_all() {
            Ok(definitions) => {
                if definitions
                    .iter()
                    .any(|definition| &definition.id == asset_definition_id)
                {
                    return Ok(());
                }
                last_error = format!(
                    "asset definition not present in FindAssetsDefinitions snapshot (len={})",
                    definitions.len()
                );
            }
            Err(err) => {
                last_error = format!("{err:?}");
            }
        }
        if Instant::now() >= deadline {
            return Err(eyre!(
                "timed out waiting for asset definition `{asset_definition_id}` registration; last_error={last_error}"
            ));
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

async fn wait_for_all_citizen_balances(
    client: &Client,
    asset_def_id: &AssetDefinitionId,
    citizens: &[(iroha::data_model::account::AccountId, KeyPair)],
    expected: u128,
    timeout: Duration,
    stage: &str,
) -> Result<()> {
    let deadline = Instant::now() + timeout;
    loop {
        let mut missing = Vec::new();
        let mut first_mismatch: Option<(iroha::data_model::account::AccountId, Option<u128>)> =
            None;
        for (account_id, _) in citizens {
            let observed = numeric_asset_balance_u128(
                client,
                &AssetId::new(asset_def_id.clone(), account_id.clone()),
            )?;
            if observed != Some(expected) {
                if first_mismatch.is_none() {
                    first_mismatch = Some((account_id.clone(), observed));
                }
                missing.push(account_id.clone());
            }
        }
        if missing.is_empty() {
            return Ok(());
        }
        if Instant::now() >= deadline {
            let (first_account, first_observed) = first_mismatch
                .as_ref()
                .map_or((None, None), |(account, observed)| {
                    (Some(account.to_string()), Some(*observed))
                });
            return Err(eyre!(
                "{stage}: timed out waiting for all citizen balances={expected}; missing_count={}, first_missing={:?}, first_observed={:?}",
                missing.len(),
                first_account,
                first_observed
            ));
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

fn numeric_asset_balance_u128(client: &Client, asset_id: &AssetId) -> Result<Option<u128>> {
    let asset = match client.query_single(FindAssetById::new(asset_id.clone())) {
        Ok(asset) => asset,
        Err(err) => {
            let msg = format!("{err:?}");
            if msg.contains("Failed to find asset")
                || msg.contains("Find(Asset(")
                || msg.contains("Find(Account(")
                || msg.contains("QueryExecutionFail::Find")
                || msg.contains("QueryExecutionFail::NotFound")
            {
                return Ok(None);
            }
            return Err(eyre!("asset balance query failed for `{asset_id}`: {msg}"));
        }
    };
    if asset.value().scale() != 0 {
        return Err(eyre!(
            "expected integer numeric value for `{asset_id}`, got scale={}",
            asset.value().scale()
        ));
    }
    Ok(asset.value().try_mantissa_u128())
}

async fn wait_for_asset_balance(
    client: &Client,
    asset_id: AssetId,
    expected: u128,
    timeout: Duration,
    stage: &str,
) -> Result<()> {
    let deadline = Instant::now() + timeout;
    loop {
        let observed = numeric_asset_balance_u128(client, &asset_id)?;
        if observed == Some(expected) || (expected == 0 && observed.is_none()) {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(eyre!(
                "{stage}: timed out waiting for `{asset_id}` balance={expected}, observed={observed:?}"
            ));
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
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

pub async fn wait_for_tx_applied(
    http: &reqwest::Client,
    torii_url: &reqwest::Url,
    tx_hash_hex: &str,
    timeout: Duration,
    stage: &str,
) -> Result<()> {
    let mut status_url = torii_url.join("v2/pipeline/transactions/status")?;
    status_url
        .query_pairs_mut()
        .append_pair("hash", tx_hash_hex);
    let deadline = Instant::now() + timeout;
    let mut last_kind = String::from("unavailable");
    let mut last_payload = String::new();
    let mut last_error = String::new();

    loop {
        match http.get(status_url.clone()).send().await {
            Ok(response)
                if response.status() == reqwest::StatusCode::OK
                    || response.status() == reqwest::StatusCode::ACCEPTED =>
            {
                let status = response.status();
                let bytes = response.bytes().await?;
                if bytes.is_empty() {
                    last_kind = format!("http {status} with empty body");
                } else {
                    let payload: norito::json::Value = norito::json::from_slice(&bytes)?;
                    if let Some(kind) = pipeline_status_kind(&payload) {
                        last_kind = kind.to_string();
                        last_payload = format!("{payload:?}");
                        match kind {
                            "Applied" => return Ok(()),
                            "Rejected" => {
                                return Err(eyre!(
                                    "{stage}: tx `{tx_hash_hex}` rejected; payload={payload:?}"
                                ));
                            }
                            "Expired" => {
                                return Err(eyre!("{stage}: tx `{tx_hash_hex}` expired"));
                            }
                            _ => {}
                        }
                    } else {
                        last_kind = "missing status kind".to_string();
                        last_payload = format!("{payload:?}");
                    }
                }
            }
            Ok(response)
                if response.status() == reqwest::StatusCode::NO_CONTENT
                    || response.status() == reqwest::StatusCode::NOT_FOUND =>
            {
                last_kind = format!("http {}", response.status());
            }
            Ok(response) => {
                last_error = format!(
                    "http {} {}",
                    response.status(),
                    std::str::from_utf8(response.bytes().await?.as_ref()).unwrap_or("")
                );
            }
            Err(err) => {
                last_error = format!("{err}");
            }
        }

        if Instant::now() >= deadline {
            return Err(eyre!(
                "{stage}: timed out waiting for tx `{tx_hash_hex}` to reach Applied; last_kind={last_kind}, last_payload={last_payload}, last_error={last_error}"
            ));
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

pub async fn wait_for_tx_rejected(
    http: &reqwest::Client,
    torii_url: &reqwest::Url,
    tx_hash_hex: &str,
    timeout: Duration,
    stage: &str,
) -> Result<norito::json::Value> {
    let mut status_url = torii_url.join("v2/pipeline/transactions/status")?;
    status_url
        .query_pairs_mut()
        .append_pair("hash", tx_hash_hex);
    let deadline = Instant::now() + timeout;
    let mut last_kind = String::from("unavailable");
    let mut last_payload = String::new();
    let mut last_error = String::new();

    loop {
        match http.get(status_url.clone()).send().await {
            Ok(response)
                if response.status() == reqwest::StatusCode::OK
                    || response.status() == reqwest::StatusCode::ACCEPTED =>
            {
                let status = response.status();
                let bytes = response.bytes().await?;
                if bytes.is_empty() {
                    last_kind = format!("http {status} with empty body");
                } else {
                    let payload: norito::json::Value = norito::json::from_slice(&bytes)?;
                    if let Some(kind) = pipeline_status_kind(&payload) {
                        last_kind = kind.to_string();
                        last_payload = format!("{payload:?}");
                        match kind {
                            "Rejected" => return Ok(payload),
                            "Applied" => {
                                return Err(eyre!(
                                    "{stage}: tx `{tx_hash_hex}` unexpectedly applied; payload={payload:?}"
                                ));
                            }
                            "Expired" => {
                                return Err(eyre!("{stage}: tx `{tx_hash_hex}` expired"));
                            }
                            _ => {}
                        }
                    } else {
                        last_kind = "missing status kind".to_string();
                        last_payload = format!("{payload:?}");
                    }
                }
            }
            Ok(response)
                if response.status() == reqwest::StatusCode::NO_CONTENT
                    || response.status() == reqwest::StatusCode::NOT_FOUND =>
            {
                last_kind = format!("http {}", response.status());
            }
            Ok(response) => {
                last_error = format!(
                    "http {} {}",
                    response.status(),
                    std::str::from_utf8(response.bytes().await?.as_ref()).unwrap_or("")
                );
            }
            Err(err) => {
                last_error = format!("{err}");
            }
        }

        if Instant::now() >= deadline {
            return Err(eyre!(
                "{stage}: timed out waiting for tx `{tx_hash_hex}` to reach Rejected; last_kind={last_kind}, last_payload={last_payload}, last_error={last_error}"
            ));
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

pub type AccountId = iroha::data_model::account::AccountId;

pub struct RuntimeGovernanceFixture {
    pub network: sandbox::SerializedNetwork,
    pub http: reqwest::Client,
    pub ready_peer_idx: usize,
    pub alice: Client,
    pub citizens: Vec<(AccountId, KeyPair)>,
    pub asset_def_id: AssetDefinitionId,
    pub alice_asset_id: AssetId,
}

pub struct RuntimeRoundOutcome {
    pub runtime_upgrade_id_hex: String,
    pub activated_height: u64,
    pub scheduled_start_height: u64,
    pub scheduled_end_height: u64,
}

pub fn governance_builder_for_runtime_resilience() -> NetworkBuilder {
    let alice_escrow_account = governance_escrow_account_literal();
    NetworkBuilder::new()
        .with_peers(4)
        .with_config_layer(move |layer| {
            layer
                .write(["default_account_domain_label"], "wonderland")
                .write(["gov", "voting_asset_id"], GOV_ASSET_ID)
                .write(["gov", "citizenship_asset_id"], GOV_ASSET_ID)
                .write(
                    ["gov", "citizenship_bond_amount"],
                    i64::try_from(CITIZEN_BOND).expect("bond amount should fit i64"),
                )
                .write(["gov", "min_bond_amount"], 0)
                .write(["gov", "plain_voting_enabled"], true)
                .write(["gov", "min_enactment_delay"], 0)
                .write(["gov", "window_span"], 500)
                .write(["gov", "conviction_step_blocks"], 1)
                .write(
                    ["gov", "max_conviction"],
                    i64::try_from(GOV_MAX_CONVICTION).expect("max conviction should fit i64"),
                )
                .write(
                    ["gov", "citizenship_escrow_account"],
                    alice_escrow_account.clone(),
                )
                .write(["gov", "bond_escrow_account"], alice_escrow_account.clone())
                .write(
                    ["gov", "slash_receiver_account"],
                    alice_escrow_account.clone(),
                )
                .write(["gov", "parliament_term_blocks"], 100)
                .write(["gov", "rules_committee_size"], 1)
                .write(["gov", "agenda_council_size"], 1)
                .write(["gov", "interest_panel_size"], 1)
                .write(["gov", "review_panel_size"], 1)
                .write(["gov", "policy_jury_size"], 1)
                .write(["gov", "oversight_committee_size"], 1)
                .write(["gov", "fma_committee_size"], 1)
                .write(["gov", "parliament_alternate_size"], 1);
        })
}

#[allow(clippy::too_many_lines)]
pub async fn setup_runtime_governance_fixture(
    test_name: &'static str,
) -> Result<Option<RuntimeGovernanceFixture>> {
    let builder = governance_builder_for_runtime_resilience();
    let Some(network) = sandbox::start_network_async_or_skip(builder, test_name).await? else {
        return Ok(None);
    };

    let http = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()?;
    let ready_peer_idx = wait_for_ready_torii_peer(&network, &http, Duration::from_secs(180))
        .await
        .wrap_err("wait for ready Torii peer")?;
    let ready_peer = &network.peers()[ready_peer_idx];
    let mut alice = ready_peer.client();
    tune_client_timeouts(&mut alice);
    sync::get_status_with_retry_async(&alice)
        .await
        .wrap_err("wait for selected Torii peer status readiness")?;

    let citizens: Vec<_> = (0..CITIZEN_COUNT)
        .map(|_| gen_account_in("wonderland"))
        .collect();
    let wonderland_domain: DomainId = "wonderland".parse()?;
    let unique_citizens: BTreeSet<_> = citizens.iter().map(|(id, _)| id.clone()).collect();
    assert_eq!(
        unique_citizens.len(),
        CITIZEN_COUNT,
        "generated citizen identities must be unique"
    );

    let gov_domain_id: DomainId = GOV_DOMAIN_ID.parse()?;
    let asset_def_id: AssetDefinitionId = GOV_ASSET_ID.parse()?;
    alice
        .submit(Register::domain(Domain::new(gov_domain_id.clone())))
        .wrap_err("register governance domain")?;
    wait_for_domain_registration(&alice, &gov_domain_id, Duration::from_secs(180))
        .await
        .wrap_err("wait for governance domain registration")?;
    alice
        .submit(Register::asset_definition(
            AssetDefinition::numeric(asset_def_id.clone())
                .with_name(asset_def_id.name().to_string()),
        ))
        .wrap_err("register governance numeric asset definition")?;
    wait_for_asset_definition_registration(&alice, &asset_def_id, Duration::from_secs(180))
        .await
        .wrap_err("wait for governance asset definition registration")?;

    let enact_perm: Permission = CanEnactGovernance.into();
    let runtime_propose_perm: Permission = CanProposeContractDeployment {
        contract_id: FIRST_CONTRACT_ID.to_string(),
    }
    .into();
    let secondary_propose_perm: Permission = CanProposeContractDeployment {
        contract_id: SECOND_CONTRACT_ID.to_string(),
    }
    .into();
    alice
        .submit_all([
            Grant::account_permission(runtime_propose_perm, ALICE_ID.clone()),
            Grant::account_permission(secondary_propose_perm, ALICE_ID.clone()),
            Grant::account_permission(enact_perm, ALICE_ID.clone()),
        ])
        .wrap_err("grant governance proposal/enact permissions to alice")?;

    alice
        .submit_all(citizens.iter().map(|(account_id, _)| {
            Register::account(Account::new(
                account_id.to_account_id(wonderland_domain.clone()),
            ))
        }))
        .wrap_err("register runtime-governance citizen accounts")?;
    for (account_id, _) in &citizens {
        wait_for_account_registration(&alice, account_id, Duration::from_secs(180))
            .await
            .wrap_err_with(|| format!("wait for account registration `{account_id}`"))?;
    }

    let extra_runtime_budget = 250_000_u128;
    let total_fund = CITIZEN_FUND.saturating_mul(u128::try_from(CITIZEN_COUNT).expect("count"))
        + extra_runtime_budget;
    let alice_asset_id = AssetId::new(asset_def_id.clone(), ALICE_ID.clone());
    alice
        .submit(Mint::asset_numeric(
            u64::try_from(total_fund).expect("total fund should fit u64"),
            alice_asset_id.clone(),
        ))
        .wrap_err("mint governance balances for runtime-governance fixture setup")?;
    wait_for_asset_balance(
        &alice,
        alice_asset_id.clone(),
        total_fund,
        Duration::from_secs(180),
        "wait for fixture setup proposer funding mint",
    )
    .await?;

    alice
        .submit_all(citizens.iter().map(|(account_id, _)| {
            Transfer::asset_numeric(
                AssetId::new(asset_def_id.clone(), ALICE_ID.clone()),
                u64::try_from(CITIZEN_FUND).expect("fund amount should fit u64"),
                account_id.clone(),
            )
        }))
        .wrap_err("transfer citizen funding allocations for runtime-governance fixture")?;
    wait_for_all_citizen_balances(
        &alice,
        &asset_def_id,
        &citizens,
        CITIZEN_FUND,
        BALANCE_WAIT_TIMEOUT,
        "wait for runtime-governance fixture citizen funding distribution",
    )
    .await?;

    for (idx, (account_id, key_pair)) in citizens.iter().enumerate() {
        let mut citizen_client = ready_peer.client_for(account_id, key_pair.private_key().clone());
        tune_client_timeouts(&mut citizen_client);
        let tx_hash = citizen_client
            .submit(RegisterCitizen {
                owner: account_id.clone(),
                amount: CITIZEN_BOND,
            })
            .wrap_err_with(|| format!("register citizen bond #{idx} ({account_id})"))?;
        wait_for_tx_applied(
            &http,
            &citizen_client.torii_url,
            &hex::encode(tx_hash.as_ref()),
            Duration::from_secs(180),
            "wait for fixture citizen bond tx to be applied",
        )
        .await
        .wrap_err_with(|| {
            format!(
                "wait for runtime fixture register citizen bond tx #{idx} ({account_id}) applied"
            )
        })?;
    }
    wait_for_all_citizen_balances(
        &alice,
        &asset_def_id,
        &citizens,
        BALLOT_LOCK,
        BALANCE_WAIT_TIMEOUT,
        "wait for fixture post-bond citizen balances",
    )
    .await?;

    let council_split = CITIZEN_COUNT / 2;
    let council_members: Vec<_> = citizens
        .iter()
        .take(council_split)
        .map(|(account_id, _)| account_id.clone())
        .collect();
    let council_alternates: Vec<_> = citizens
        .iter()
        .skip(council_split)
        .map(|(account_id, _)| account_id.clone())
        .collect();
    alice
        .submit(PersistCouncilForEpoch {
            epoch: 0,
            members: council_members.clone(),
            alternates: council_alternates,
            verified: 0,
            candidates_count: u32::try_from(CITIZEN_COUNT).expect("count"),
            derived_by: CouncilDerivationKind::Fallback,
        })
        .wrap_err("persist fallback council for runtime-governance fixture")?;
    let first_council_member = council_members
        .first()
        .cloned()
        .ok_or_else(|| eyre!("runtime-governance fixture expected at least one council member"))?;
    wait_for_council_member_present(&alice, &first_council_member, 0, Duration::from_secs(180))
        .await
        .wrap_err("wait for persisted council epoch 0 member in runtime-governance fixture")?;

    Ok(Some(RuntimeGovernanceFixture {
        network,
        http,
        ready_peer_idx,
        alice,
        citizens,
        asset_def_id,
        alice_asset_id,
    }))
}

fn tuned_client_for_account(
    fixture: &RuntimeGovernanceFixture,
    account_id: &AccountId,
    key_pair: &KeyPair,
) -> Client {
    let mut client = fixture.network.peers()[fixture.ready_peer_idx]
        .client_for(account_id, key_pair.private_key().clone());
    tune_client_timeouts(&mut client);
    client
}

async fn restart_peer_and_wait_for_height(
    network: &sandbox::SerializedNetwork,
    peer_idx: usize,
    target_height: u64,
    context: &str,
) -> Result<()> {
    let peer = network.peers().get(peer_idx).ok_or_else(|| {
        eyre!(
            "{context}: peer index {peer_idx} out of range for {} peers",
            network.peers().len()
        )
    })?;

    let _ = peer.shutdown_if_started().await;
    let config_layers = network.config_layers().collect::<Vec<_>>();
    peer.start_checked(config_layers.iter().cloned(), None)
        .await
        .wrap_err_with(|| format!("{context}: restart peer {peer_idx}"))?;

    let mut restart_client = peer.client();
    tune_client_timeouts(&mut restart_client);
    sync::get_status_with_retry_async(&restart_client)
        .await
        .wrap_err_with(|| {
            format!("{context}: wait for restarted peer {peer_idx} status readiness")
        })?;

    let deadline = Instant::now() + Duration::from_secs(180);
    loop {
        match restart_client.get_status() {
            Ok(status) if status.blocks >= target_height => return Ok(()),
            Ok(_) | Err(_) => {}
        }

        if Instant::now() >= deadline {
            return Err(eyre!(
                "{context}: restarted peer {peer_idx} did not reach block height {target_height} within {:?}",
                Duration::from_secs(180)
            ));
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

#[allow(clippy::too_many_lines)]
pub async fn enact_runtime_upgrade_round(
    fixture: &mut RuntimeGovernanceFixture,
    round_label: &str,
    restart_peer_idx: Option<usize>,
) -> Result<RuntimeRoundOutcome> {
    let abi_hash_hex = canonical_abi_hex();
    let runtime_height_anchor = fixture.alice.get_status()?.blocks;
    let baseline_start_height = runtime_height_anchor.saturating_add(30);
    let chain_max_end_height = max_runtime_upgrade_end_height(&fixture.alice)
        .wrap_err("query max runtime upgrade end_height from on-chain state")?;
    let chain_start_height = chain_max_end_height.map_or(0, |height| height.saturating_add(1));
    let runtime_start_height = baseline_start_height.max(chain_start_height);
    let runtime_end_height = runtime_start_height.saturating_add(RUNTIME_WINDOW_DURATION_BLOCKS);
    let runtime_manifest = RuntimeUpgradeManifest {
        name: format!("{RUNTIME_UPGRADE_NAME}.{round_label}"),
        description: format!("{RUNTIME_UPGRADE_DESCRIPTION} ({round_label})"),
        abi_version: 1,
        abi_hash: parse_hex32(&abi_hash_hex),
        added_syscalls: Vec::new(),
        added_pointer_types: Vec::new(),
        start_height: runtime_start_height,
        end_height: runtime_end_height,
        sbom_digests: Vec::new(),
        slsa_attestation: Vec::new(),
        provenance: Vec::new(),
    };
    let runtime_proposal_id = compute_runtime_upgrade_proposal_id(&runtime_manifest);
    let runtime_proposal_id_hex = hex::encode(runtime_proposal_id);
    let runtime_referendum_id = runtime_proposal_id_hex.clone();
    let runtime_upgrade_id_hex = hex::encode(runtime_manifest.id().0);

    let runtime_propose_tx_hash = fixture
        .alice
        .submit(ProposeRuntimeUpgradeProposal {
            manifest: runtime_manifest.clone(),
            window: None,
            mode: Some(VotingMode::Plain),
        })
        .wrap_err_with(|| {
            format!("propose runtime upgrade referendum through governance ({round_label})")
        })?;
    wait_for_tx_applied(
        &fixture.http,
        &fixture.alice.torii_url,
        &hex::encode(runtime_propose_tx_hash.as_ref()),
        Duration::from_secs(180),
        "wait for runtime-upgrade proposal transaction to be applied",
    )
    .await
    .wrap_err_with(|| format!("wait for runtime-upgrade proposal tx applied ({round_label})"))?;
    wait_for_proposal_found(
        &fixture.alice,
        &runtime_proposal_id_hex,
        Duration::from_secs(180),
    )
    .await
    .wrap_err_with(|| {
        format!("wait for runtime-upgrade proposal to be queryable ({round_label})")
    })?;

    let runtime_voters: Vec<_> = fixture
        .citizens
        .iter()
        .take(THIRD_REFERENDUM_VOTERS)
        .collect();
    fixture
        .alice
        .submit_all(runtime_voters.iter().map(|(account_id, _)| {
            let ballot_perm: Permission = CanSubmitGovernanceBallot {
                referendum_id: runtime_referendum_id.clone(),
            }
            .into();
            Grant::account_permission(ballot_perm, account_id.clone())
        }))
        .wrap_err_with(|| format!("grant runtime referendum ballot permissions ({round_label})"))?;

    let runtime_transfer_amount =
        u64::try_from(BALLOT_LOCK).expect("runtime ballot lock should fit u64");
    let runtime_expected_balances: Vec<_> = runtime_voters
        .iter()
        .map(|(account_id, _)| {
            let current = numeric_asset_balance_u128(
                &fixture.alice,
                &AssetId::new(fixture.asset_def_id.clone(), account_id.clone()),
            )?
            .unwrap_or(0);
            Ok((account_id.clone(), current.saturating_add(BALLOT_LOCK)))
        })
        .collect::<Result<Vec<_>>>()?;
    fixture
        .alice
        .submit_all(runtime_voters.iter().map(|(account_id, _)| {
            Transfer::asset_numeric(
                AssetId::new(fixture.asset_def_id.clone(), ALICE_ID.clone()),
                runtime_transfer_amount,
                account_id.clone(),
            )
        }))
        .wrap_err_with(|| format!("fund runtime referendum voters ({round_label})"))?;
    for (account_id, expected_balance) in runtime_expected_balances {
        wait_for_asset_balance(
            &fixture.alice,
            AssetId::new(fixture.asset_def_id.clone(), account_id.clone()),
            expected_balance,
            BALANCE_WAIT_TIMEOUT,
            "wait for runtime referendum voter funding",
        )
        .await
        .wrap_err_with(|| {
            format!("wait for runtime referendum voter funding `{account_id}` ({round_label})")
        })?;
    }

    let (first_approver_account_id, first_approver_key_pair) =
        fixture.citizens.first().ok_or_else(|| {
            eyre!("runtime-governance fixture expected at least one citizen approver")
        })?;
    let first_approver_client =
        tuned_client_for_account(fixture, first_approver_account_id, first_approver_key_pair);
    for body in [
        ParliamentBody::RulesCommittee,
        ParliamentBody::AgendaCouncil,
    ] {
        first_approver_client
            .submit(ApproveGovernanceProposal {
                body,
                proposal_id: runtime_proposal_id,
            })
            .wrap_err_with(|| format!("submit initial {body:?} approval ({round_label})"))?;
    }
    wait_for_referendum_found(
        &fixture.alice,
        &runtime_referendum_id,
        Duration::from_secs(180),
    )
    .await
    .wrap_err_with(|| format!("wait for runtime referendum record ({round_label})"))?;
    wait_for_referendum_status(
        &fixture.alice,
        &runtime_referendum_id,
        "Proposed",
        Duration::from_secs(60),
    )
    .await
    .wrap_err_with(|| format!("wait for runtime referendum to remain proposed ({round_label})"))?;

    if let Some(peer_idx) = restart_peer_idx {
        let target_height = fixture.alice.get_status()?.blocks;
        restart_peer_and_wait_for_height(
            &fixture.network,
            peer_idx,
            target_height,
            "restart peer during runtime referendum approvals",
        )
        .await?;
    }

    for (approval_idx, (approver_account_id, approver_key_pair)) in
        fixture.citizens.iter().enumerate()
    {
        let approver_client =
            tuned_client_for_account(fixture, approver_account_id, approver_key_pair);
        for body in [
            ParliamentBody::RulesCommittee,
            ParliamentBody::AgendaCouncil,
            ParliamentBody::InterestPanel,
            ParliamentBody::ReviewPanel,
            ParliamentBody::PolicyJury,
            ParliamentBody::OversightCommittee,
            ParliamentBody::FmaCommittee,
        ] {
            approver_client
                .submit(ApproveGovernanceProposal {
                    body,
                    proposal_id: runtime_proposal_id,
                })
                .wrap_err_with(|| {
                    format!(
                        "submit runtime {body:?} approval with citizen approver #{approval_idx} ({approver_account_id}) ({round_label})"
                    )
                })?;
        }
    }
    wait_for_referendum_status(
        &fixture.alice,
        &runtime_referendum_id,
        "Open",
        Duration::from_secs(180),
    )
    .await
    .wrap_err_with(|| format!("wait for runtime referendum status to open ({round_label})"))?;

    for (idx, (account_id, key_pair)) in runtime_voters.iter().enumerate() {
        let citizen_client = tuned_client_for_account(fixture, account_id, key_pair);
        citizen_client
            .submit(CastPlainBallot {
                referendum_id: runtime_referendum_id.clone(),
                owner: account_id.clone(),
                amount: BALLOT_LOCK,
                duration_blocks: BALLOT_DURATION_BLOCKS,
                direction: u8::from(idx >= THIRD_REFERENDUM_APPROVE_VOTERS),
            })
            .wrap_err_with(|| {
                format!("cast runtime referendum ballot #{idx} ({account_id}) ({round_label})")
            })?;
    }
    wait_for_tally_total(
        &fixture.alice,
        &runtime_referendum_id,
        expected_plain_total_weight(THIRD_REFERENDUM_VOTERS),
        Duration::from_secs(180),
    )
    .await
    .wrap_err_with(|| {
        format!("wait for runtime referendum ballot tally ingestion ({round_label})")
    })?;

    let runtime_finalize_tx_hash = fixture
        .alice
        .submit(FinalizeReferendum {
            referendum_id: runtime_referendum_id.clone(),
            proposal_id: runtime_proposal_id,
        })
        .wrap_err_with(|| format!("finalize runtime referendum ({round_label})"))?;
    wait_for_tx_applied(
        &fixture.http,
        &fixture.alice.torii_url,
        &hex::encode(runtime_finalize_tx_hash.as_ref()),
        Duration::from_secs(180),
        "wait for runtime referendum finalize tx to be applied",
    )
    .await
    .wrap_err_with(|| format!("wait for runtime finalize tx applied ({round_label})"))?;
    wait_for_proposal_status(
        &fixture.alice,
        &runtime_proposal_id_hex,
        "Approved",
        Duration::from_secs(180),
    )
    .await
    .wrap_err_with(|| format!("wait for runtime proposal to become approved ({round_label})"))?;

    let runtime_enact_tx_hash = fixture
        .alice
        .submit(EnactReferendum {
            referendum_id: runtime_proposal_id,
            preimage_hash: [0; 32],
            at_window: AtWindow {
                lower: 0,
                upper: u64::MAX,
            },
        })
        .wrap_err_with(|| format!("enact runtime referendum ({round_label})"))?;
    wait_for_tx_applied(
        &fixture.http,
        &fixture.alice.torii_url,
        &hex::encode(runtime_enact_tx_hash.as_ref()),
        Duration::from_secs(180),
        "wait for runtime referendum enact tx to be applied",
    )
    .await
    .wrap_err_with(|| format!("wait for runtime enact tx applied ({round_label})"))?;
    wait_for_referendum_status(
        &fixture.alice,
        &runtime_referendum_id,
        "Closed",
        Duration::from_secs(180),
    )
    .await
    .wrap_err_with(|| format!("wait for runtime referendum to close ({round_label})"))?;
    wait_for_proposal_status(
        &fixture.alice,
        &runtime_proposal_id_hex,
        "Enacted",
        Duration::from_secs(180),
    )
    .await
    .wrap_err_with(|| format!("wait for runtime proposal to become enacted ({round_label})"))?;

    let mut runtime_status = wait_for_runtime_upgrade_status(
        &fixture.alice,
        &runtime_upgrade_id_hex,
        "ActivatedAt",
        Duration::from_secs(10),
    )
    .await
    .ok();
    if runtime_status.is_none() {
        for tick_idx in 0..40_u64 {
            let tick_tx_hash = fixture
                .alice
                .submit(Mint::asset_numeric(1_u32, fixture.alice_asset_id.clone()))
                .wrap_err_with(|| {
                    format!(
                        "submit runtime activation tick transaction #{tick_idx} ({round_label})"
                    )
                })?;
            wait_for_tx_applied(
                &fixture.http,
                &fixture.alice.torii_url,
                &hex::encode(tick_tx_hash.as_ref()),
                Duration::from_secs(180),
                "wait for runtime activation tick transaction to be applied",
            )
            .await
            .wrap_err_with(|| {
                format!(
                    "wait for runtime activation tick transaction #{tick_idx} applied ({round_label})"
                )
            })?;
            runtime_status = wait_for_runtime_upgrade_status(
                &fixture.alice,
                &runtime_upgrade_id_hex,
                "ActivatedAt",
                Duration::from_secs(5),
            )
            .await
            .ok();
            if runtime_status.is_some() {
                break;
            }
        }
    }

    let runtime_status = runtime_status.ok_or_else(|| {
        eyre!(
            "runtime upgrade `{runtime_upgrade_id_hex}` was not observed as ActivatedAt after enactment and ticking ({round_label})"
        )
    })?;
    let activated_height = runtime_status
        .get("ActivatedAt")
        .and_then(norito::json::Value::as_u64)
        .ok_or_else(|| eyre!("runtime upgrade status should include ActivatedAt height"))?;
    assert_eq!(
        activated_height, runtime_start_height,
        "runtime upgrade should activate exactly at the scheduled start height ({round_label})"
    );

    Ok(RuntimeRoundOutcome {
        runtime_upgrade_id_hex,
        activated_height,
        scheduled_start_height: runtime_start_height,
        scheduled_end_height: runtime_end_height,
    })
}
