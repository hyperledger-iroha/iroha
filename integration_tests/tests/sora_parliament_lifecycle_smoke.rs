#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! 4-peer SORA parliament lifecycle smoke: fund, bond citizenship, approve, vote, finalize, enact.

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
        EnactReferendum, FinalizeReferendum, PersistCouncilForEpoch, ProposeDeployContract,
        RegisterCitizen, VotingMode,
    },
    permission::Permission,
    prelude::{
        Account, AssetDefinitionId, FindAssetById, FindAssetsDefinitions, FindDomains, Grant, Mint,
        QueryBuilderExt, Register, Transfer,
    },
    query::account::prelude::FindAccounts,
    smart_contract::manifest::{ContractManifest, ManifestProvenance},
};
use iroha_crypto::{Hash, KeyPair};
use iroha_executor_data_model::permission::governance::{
    CanEnactGovernance, CanProposeContractDeployment, CanSubmitGovernanceBallot,
};
use iroha_test_network::NetworkBuilder;
use iroha_test_samples::{ALICE_ID, ALICE_KEYPAIR, gen_account_in};

const CITIZEN_COUNT: usize = 20;
const CITIZEN_FUND: u128 = 15_000;
const CITIZEN_BOND: u128 = 10_000;
const BALLOT_LOCK: u128 = CITIZEN_FUND - CITIZEN_BOND;
const BALLOT_DURATION_BLOCKS: u64 = 20;
const FIRST_REFERENDUM_VOTERS: usize = 10;
const FIRST_REFERENDUM_APPROVE_VOTERS: usize = 7;
const SECOND_REFERENDUM_VOTERS: usize = CITIZEN_COUNT - FIRST_REFERENDUM_VOTERS;
const SECOND_REFERENDUM_REJECT_VOTERS: usize = 8;
const GOV_MAX_CONVICTION: u64 = 6;
const GOV_DOMAIN_ID: &str = "govsmoke";
const GOV_ASSET_ID: &str = "xor#govsmoke";
const FIRST_CONTRACT_ID: &str = "parliament.lifecycle.smoke.contract";
const SECOND_CONTRACT_ID: &str = "parliament.lifecycle.smoke.reject.contract";
const TX_STATUS_TIMEOUT: Duration = Duration::from_secs(900);
const TORII_REQUEST_TIMEOUT: Duration = Duration::from_secs(120);
const TX_TTL: Duration = Duration::from_secs(1_200);
const BALANCE_WAIT_TIMEOUT: Duration = Duration::from_secs(300);

fn canonical_abi_hex() -> String {
    hex::encode(ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1))
}

fn tune_client_timeouts(client: &mut Client) {
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

fn parse_hex32(input: &str) -> [u8; 32] {
    let bytes = hex::decode(input).expect("hex should decode");
    let mut out = [0_u8; 32];
    out.copy_from_slice(&bytes);
    out
}

fn manifest_provenance(
    code_hash_hex: &str,
    abi_hash_hex: &str,
    signer: &KeyPair,
) -> ManifestProvenance {
    let code_hash = Hash::prehashed(parse_hex32(code_hash_hex));
    let abi_hash = Hash::prehashed(parse_hex32(abi_hash_hex));
    ContractManifest {
        code_hash: Some(code_hash),
        abi_hash: Some(abi_hash),
        compiler_fingerprint: None,
        features_bitmap: None,
        access_set_hints: None,
        entrypoints: None,
        kotoba: None,
        provenance: None,
    }
    .signed(signer)
    .provenance
    .expect("manifest should contain provenance")
}

fn compute_proposal_id(
    namespace: &str,
    contract_id: &str,
    code_hash_hex: &str,
    abi_hash_hex: &str,
) -> [u8; 32] {
    use iroha_crypto::blake2::{Blake2b512, Digest as _};

    let code_hash = parse_hex32(code_hash_hex);
    let abi_hash = parse_hex32(abi_hash_hex);
    let namespace_len = u32::try_from(namespace.len()).expect("namespace length fits");
    let contract_len = u32::try_from(contract_id.len()).expect("contract length fits");

    let mut input = Vec::with_capacity(
        b"iroha:gov:proposal:v1|".len()
            + core::mem::size_of::<u32>() * 2
            + namespace.len()
            + contract_id.len()
            + code_hash.len()
            + abi_hash.len(),
    );
    input.extend_from_slice(b"iroha:gov:proposal:v1|");
    input.extend_from_slice(&namespace_len.to_le_bytes());
    input.extend_from_slice(namespace.as_bytes());
    input.extend_from_slice(&contract_len.to_le_bytes());
    input.extend_from_slice(contract_id.as_bytes());
    input.extend_from_slice(&code_hash);
    input.extend_from_slice(&abi_hash);

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

fn integer_sqrt_u128(value: u128) -> u128 {
    if value < 2 {
        return value;
    }
    let mut x0 = value;
    let mut x1 = (x0 + (value / x0)) / 2;
    while x1 < x0 {
        x0 = x1;
        x1 = (x0 + (value / x0)) / 2;
    }
    x0
}

fn expected_plain_total_weight(voter_count: usize) -> u128 {
    let conviction_factor = (1_u64 + BALLOT_DURATION_BLOCKS).min(GOV_MAX_CONVICTION);
    integer_sqrt_u128(BALLOT_LOCK)
        .saturating_mul(u128::from(conviction_factor))
        .saturating_mul(u128::try_from(voter_count).expect("voter count should fit u128"))
}

async fn wait_for_proposal_found(
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
                AssetId::new(asset_def_id.clone(), account_id.clone()),
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

fn numeric_asset_balance_u128(client: &Client, asset_id: AssetId) -> Result<Option<u128>> {
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
        let observed = numeric_asset_balance_u128(client, asset_id.clone())?;
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

async fn wait_for_tx_applied(
    http: &reqwest::Client,
    torii_url: &reqwest::Url,
    tx_hash_hex: &str,
    timeout: Duration,
    stage: &str,
) -> Result<()> {
    let mut status_url = torii_url.join("v1/pipeline/transactions/status")?;
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

async fn wait_for_tx_rejected(
    http: &reqwest::Client,
    torii_url: &reqwest::Url,
    tx_hash_hex: &str,
    timeout: Duration,
    stage: &str,
) -> Result<norito::json::Value> {
    let mut status_url = torii_url.join("v1/pipeline/transactions/status")?;
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

#[tokio::test]
async fn sora_parliament_lifecycle_smoke() -> Result<()> {
    eprintln!("sora smoke: start");
    let alice_escrow_account = format!("{}@wonderland", ALICE_KEYPAIR.public_key());
    let builder = NetworkBuilder::new()
        .with_peers(4)
        .with_config_layer(move |layer| {
            layer
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
        });

    let Some(network) =
        sandbox::start_network_async_or_skip(builder, stringify!(sora_parliament_lifecycle_smoke))
            .await?
    else {
        return Ok(());
    };

    let http = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()?;
    let ready_peer_idx = wait_for_ready_torii_peer(&network, &http, Duration::from_secs(180))
        .await
        .wrap_err("wait for a Torii peer with live /status")?;
    let ready_peer = &network.peers()[ready_peer_idx];
    let mut alice = ready_peer.client();
    tune_client_timeouts(&mut alice);
    sync::get_status_with_retry_async(&alice)
        .await
        .wrap_err("wait for selected Torii peer status readiness")?;
    eprintln!("sora smoke: ready peer selected");

    let citizens: Vec<_> = (0..CITIZEN_COUNT)
        .map(|_| gen_account_in("wonderland"))
        .collect();
    if citizens.is_empty() {
        return Err(eyre!("expected at least one generated citizen"));
    }
    assert_eq!(
        FIRST_REFERENDUM_VOTERS + SECOND_REFERENDUM_VOTERS,
        CITIZEN_COUNT,
        "referendum voter splits must cover all generated citizens"
    );
    assert!(
        FIRST_REFERENDUM_APPROVE_VOTERS < FIRST_REFERENDUM_VOTERS,
        "first referendum should include both approve and reject votes"
    );
    assert!(
        SECOND_REFERENDUM_REJECT_VOTERS < SECOND_REFERENDUM_VOTERS,
        "second referendum should include both reject and approve votes"
    );
    let (outsider_id, outsider_key_pair) = gen_account_in("wonderland");
    let unique_citizens: BTreeSet<_> = citizens.iter().map(|(id, _)| id.clone()).collect();
    assert_eq!(
        unique_citizens.len(),
        CITIZEN_COUNT,
        "generated citizen identities must be unique"
    );

    let namespace = "sora";
    let contract_id = FIRST_CONTRACT_ID;
    let reject_contract_id = SECOND_CONTRACT_ID;
    let code_hash_hex = "dd".repeat(32);
    let reject_code_hash_hex = "ee".repeat(32);
    let abi_hash_hex = canonical_abi_hex();
    let proposal_id = compute_proposal_id(namespace, contract_id, &code_hash_hex, &abi_hash_hex);
    let reject_proposal_id = compute_proposal_id(
        namespace,
        reject_contract_id,
        &reject_code_hash_hex,
        &abi_hash_hex,
    );
    let proposal_id_hex = hex::encode(proposal_id);
    let reject_proposal_id_hex = hex::encode(reject_proposal_id);
    let referendum_id = proposal_id_hex.clone();
    let reject_referendum_id = reject_proposal_id_hex.clone();

    let gov_domain_id: DomainId = GOV_DOMAIN_ID.parse()?;
    let asset_def_id: AssetDefinitionId = GOV_ASSET_ID.parse()?;
    alice
        .submit(Register::domain(Domain::new(gov_domain_id.clone())))
        .wrap_err("register governance domain")?;
    wait_for_domain_registration(&alice, &gov_domain_id, Duration::from_secs(180))
        .await
        .wrap_err("wait for governance domain registration")?;
    alice
        .submit(Register::asset_definition(AssetDefinition::numeric(
            asset_def_id.clone(),
        )))
        .wrap_err("register governance numeric asset definition")?;
    wait_for_asset_definition_registration(&alice, &asset_def_id, Duration::from_secs(180))
        .await
        .wrap_err("wait for governance asset definition registration")?;
    eprintln!("sora smoke: governance domain + asset ready");

    let propose_perm: Permission = CanProposeContractDeployment {
        contract_id: contract_id.to_string(),
    }
    .into();
    let reject_propose_perm: Permission = CanProposeContractDeployment {
        contract_id: reject_contract_id.to_string(),
    }
    .into();
    let enact_perm: Permission = CanEnactGovernance.into();
    alice
        .submit_all([
            Grant::account_permission(propose_perm, ALICE_ID.clone()),
            Grant::account_permission(reject_propose_perm, ALICE_ID.clone()),
            Grant::account_permission(enact_perm, ALICE_ID.clone()),
        ])
        .wrap_err("grant governance proposal/enact permissions to alice")?;

    alice
        .submit_all(
            citizens
                .iter()
                .map(|(account_id, _)| Register::account(Account::new(account_id.clone()))),
        )
        .wrap_err("register citizen accounts")?;
    for (account_id, _) in &citizens {
        wait_for_account_registration(&alice, account_id, Duration::from_secs(180))
            .await
            .wrap_err_with(|| format!("wait for account registration `{account_id}`"))?;
    }
    alice
        .submit(Register::account(Account::new(outsider_id.clone())))
        .wrap_err("register outsider account for negative authorization tests")?;
    wait_for_account_registration(&alice, &outsider_id, Duration::from_secs(180))
        .await
        .wrap_err("wait for outsider account registration")?;
    eprintln!("sora smoke: citizen accounts registered");

    let total_fund = CITIZEN_FUND.saturating_mul(u128::try_from(CITIZEN_COUNT).expect("count"));
    let alice_asset_id = AssetId::new(asset_def_id.clone(), ALICE_ID.clone());
    alice
        .submit(Mint::asset_numeric(
            u64::try_from(total_fund).expect("total fund should fit u64"),
            alice_asset_id.clone(),
        ))
        .wrap_err("mint total governance balances for citizen funding")?;
    if let Err(initial_mint_wait_err) = wait_for_asset_balance(
        &alice,
        alice_asset_id.clone(),
        total_fund,
        Duration::from_secs(180),
        "wait for proposer funding mint",
    )
    .await
    {
        let observed = numeric_asset_balance_u128(&alice, alice_asset_id.clone())?.unwrap_or(0);
        if observed > total_fund {
            return Err(eyre!(
                "proposer funding asset exceeded expected amount: observed={observed}, expected={total_fund}"
            ));
        }
        let missing = total_fund.saturating_sub(observed);
        if missing > 0 {
            alice
                .submit(Mint::asset_numeric(
                    u64::try_from(missing).expect("missing fund should fit u64"),
                    alice_asset_id.clone(),
                ))
                .wrap_err(
                    "retry minting missing governance balances for citizen funding after timeout",
                )?;
        }
        wait_for_asset_balance(
            &alice,
            alice_asset_id.clone(),
            total_fund,
            Duration::from_secs(180),
            "wait for proposer funding mint after retry",
        )
        .await
        .wrap_err_with(|| format!("initial proposer mint wait failed: {initial_mint_wait_err}"))?;
    }
    eprintln!("sora smoke: proposer funding minted");

    alice
        .submit_all(citizens.iter().flat_map(|(account_id, _)| {
            let first_ballot_perm: Permission = CanSubmitGovernanceBallot {
                referendum_id: referendum_id.clone(),
            }
            .into();
            let second_ballot_perm: Permission = CanSubmitGovernanceBallot {
                referendum_id: reject_referendum_id.clone(),
            }
            .into();
            [
                Grant::account_permission(first_ballot_perm, account_id.clone()),
                Grant::account_permission(second_ballot_perm, account_id.clone()),
            ]
        }))
        .wrap_err("grant ballot permissions for both referenda")?;

    alice
        .submit_all(citizens.iter().map(|(account_id, _)| {
            Transfer::asset_numeric(
                AssetId::new(asset_def_id.clone(), ALICE_ID.clone()),
                u64::try_from(CITIZEN_FUND).expect("fund amount should fit u64"),
                account_id.clone(),
            )
        }))
        .wrap_err("transfer citizen funding allocations")?;

    wait_for_all_citizen_balances(
        &alice,
        &asset_def_id,
        &citizens,
        CITIZEN_FUND,
        BALANCE_WAIT_TIMEOUT,
        "wait for citizen funding distribution",
    )
    .await?;
    eprintln!("sora smoke: citizen funding distributed");
    wait_for_asset_balance(
        &alice,
        alice_asset_id.clone(),
        0,
        BALANCE_WAIT_TIMEOUT,
        "wait for proposer balance after funding",
    )
    .await?;
    eprintln!("sora smoke: proposer post-funding balance settled");

    let mut outsider_client =
        ready_peer.client_for(&outsider_id, outsider_key_pair.private_key().clone());
    tune_client_timeouts(&mut outsider_client);
    let outsider_bond_tx_hash = outsider_client
        .submit(RegisterCitizen {
            owner: outsider_id.clone(),
            amount: CITIZEN_BOND,
        })
        .wrap_err("submit outsider underfunded citizenship bond (negative path)")?;
    wait_for_tx_rejected(
        &http,
        &outsider_client.torii_url,
        &hex::encode(outsider_bond_tx_hash.as_ref()),
        Duration::from_secs(180),
        "wait for outsider underfunded citizenship bond to be rejected",
    )
    .await
    .wrap_err("outsider underfunded citizenship bond should be rejected")?;
    eprintln!("sora smoke: outsider underfunded bond rejected");

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
            "wait for citizen bond tx to be applied",
        )
        .await
        .wrap_err_with(|| {
            format!("wait for register citizen bond tx #{idx} ({account_id}) applied")
        })?;
    }
    eprintln!("sora smoke: citizen bond txs submitted");

    wait_for_all_citizen_balances(
        &alice,
        &asset_def_id,
        &citizens,
        BALLOT_LOCK,
        BALANCE_WAIT_TIMEOUT,
        "wait for post-bond citizen balances",
    )
    .await?;
    eprintln!("sora smoke: citizen post-bond balances settled");

    let expected_escrow_delta =
        CITIZEN_BOND.saturating_mul(u128::try_from(CITIZEN_COUNT).expect("count"));
    wait_for_asset_balance(
        &alice,
        alice_asset_id.clone(),
        expected_escrow_delta,
        BALANCE_WAIT_TIMEOUT,
        "wait for escrowed citizenship bond total",
    )
    .await?;
    eprintln!("sora smoke: escrow bond total settled");

    let (council_member_id, _) = &citizens[0];
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
        .wrap_err("persist fallback council for epoch 0")?;
    wait_for_council_member_present(&alice, council_member_id, 0, Duration::from_secs(180))
        .await
        .wrap_err("wait for persisted council epoch 0 member")?;
    eprintln!("sora smoke: council persisted");

    alice
        .submit(ProposeDeployContract {
            namespace: namespace.to_string(),
            contract_id: contract_id.to_string(),
            code_hash_hex: code_hash_hex.clone(),
            abi_hash_hex: abi_hash_hex.clone(),
            abi_version: "1".to_string(),
            window: None,
            mode: Some(VotingMode::Plain),
            manifest_provenance: Some(manifest_provenance(
                &code_hash_hex,
                &abi_hash_hex,
                &ALICE_KEYPAIR,
            )),
        })
        .wrap_err("propose governance contract deployment referendum")?;
    wait_for_proposal_found(&alice, &proposal_id_hex, Duration::from_secs(180))
        .await
        .wrap_err("wait for governance proposal to be queryable")?;
    eprintln!("sora smoke: proposal submitted");

    let mut unauthorized_approver_client =
        ready_peer.client_for(&outsider_id, outsider_key_pair.private_key().clone());
    tune_client_timeouts(&mut unauthorized_approver_client);
    let unauthorized_approval_tx_hash = unauthorized_approver_client
        .submit(ApproveGovernanceProposal {
            body: ParliamentBody::RulesCommittee,
            proposal_id,
        })
        .wrap_err("submit unauthorized proposal approval attempt from outsider account")?;
    wait_for_tx_rejected(
        &http,
        &unauthorized_approver_client.torii_url,
        &hex::encode(unauthorized_approval_tx_hash.as_ref()),
        Duration::from_secs(180),
        "wait for unauthorized proposal approval attempt to be rejected",
    )
    .await
    .wrap_err("unauthorized proposal approval attempt should be rejected")?;

    let first_council_account_id = council_members
        .first()
        .cloned()
        .expect("at least one council member is required");
    let (_, first_council_key_pair) = citizens
        .iter()
        .find(|(account_id, _)| account_id == &first_council_account_id)
        .expect("first council account should have a generated key pair");
    let mut first_council_client = ready_peer.client_for(
        &first_council_account_id,
        first_council_key_pair.private_key().clone(),
    );
    tune_client_timeouts(&mut first_council_client);
    first_council_client
        .submit(ApproveGovernanceProposal {
            body: ParliamentBody::RulesCommittee,
            proposal_id,
        })
        .wrap_err("submit first council rules committee approval")?;
    wait_for_referendum_found(&alice, &referendum_id, Duration::from_secs(180))
        .await
        .wrap_err("wait for referendum record to appear")?;
    wait_for_referendum_status(&alice, &referendum_id, "Proposed", Duration::from_secs(60))
        .await
        .wrap_err("wait for referendum to remain proposed before agenda council quorum")?;
    eprintln!("sora smoke: referendum remained proposed after partial council approvals");

    for (approval_idx, council_account_id) in council_members.iter().enumerate() {
        let (_, council_key_pair) = citizens
            .iter()
            .find(|(account_id, _)| account_id == council_account_id)
            .expect("council account should have a generated key pair");
        let mut council_member_client =
            ready_peer.client_for(council_account_id, council_key_pair.private_key().clone());
        tune_client_timeouts(&mut council_member_client);
        council_member_client
            .submit(ApproveGovernanceProposal {
                body: ParliamentBody::AgendaCouncil,
                proposal_id,
            })
            .wrap_err_with(|| {
                format!(
                    "submit agenda council approval with council member #{approval_idx} ({council_account_id})"
                )
            })?;
        if approval_idx > 0 {
            council_member_client
                .submit(ApproveGovernanceProposal {
                    body: ParliamentBody::RulesCommittee,
                    proposal_id,
                })
                .wrap_err_with(|| {
                    format!(
                        "submit rules committee approval with council member #{approval_idx} ({council_account_id})"
                    )
                })?;
        }
    }
    wait_for_referendum_status(&alice, &referendum_id, "Open", Duration::from_secs(180))
        .await
        .wrap_err("wait for referendum status to open before ballots")?;
    eprintln!("sora smoke: referendum open");

    for (idx, (account_id, key_pair)) in citizens.iter().take(FIRST_REFERENDUM_VOTERS).enumerate() {
        let mut citizen_client = ready_peer.client_for(account_id, key_pair.private_key().clone());
        tune_client_timeouts(&mut citizen_client);
        citizen_client
            .submit(CastPlainBallot {
                referendum_id: referendum_id.clone(),
                owner: account_id.clone(),
                amount: BALLOT_LOCK,
                duration_blocks: BALLOT_DURATION_BLOCKS,
                direction: if idx < FIRST_REFERENDUM_APPROVE_VOTERS {
                    0
                } else {
                    1
                },
            })
            .wrap_err_with(|| format!("cast citizen ballot #{idx} ({account_id})"))?;
    }
    let expected_total_weight = expected_plain_total_weight(FIRST_REFERENDUM_VOTERS);
    wait_for_tally_total(
        &alice,
        &referendum_id,
        expected_total_weight,
        Duration::from_secs(180),
    )
    .await
    .wrap_err("wait for full ballot tally ingestion")?;
    eprintln!("sora smoke: tally reached expected total");

    let premature_enact_tx_hash = alice
        .submit(EnactReferendum {
            referendum_id: proposal_id,
            preimage_hash: [0; 32],
            at_window: AtWindow {
                lower: 0,
                upper: u64::MAX,
            },
        })
        .wrap_err("submit premature enact referendum before finalize (negative path)")?;
    wait_for_tx_rejected(
        &http,
        &alice.torii_url,
        &hex::encode(premature_enact_tx_hash.as_ref()),
        Duration::from_secs(180),
        "wait for premature enact referendum tx to be rejected",
    )
    .await
    .wrap_err("premature enact referendum should be rejected before finalize")?;
    eprintln!("sora smoke: premature enact rejected");

    let finalize_tx_hash = alice
        .submit(FinalizeReferendum {
            referendum_id: referendum_id.clone(),
            proposal_id,
        })
        .wrap_err("finalize referendum")?;
    wait_for_tx_applied(
        &http,
        &alice.torii_url,
        &hex::encode(finalize_tx_hash.as_ref()),
        Duration::from_secs(180),
        "wait for finalize referendum tx to be applied",
    )
    .await
    .wrap_err("wait for finalize referendum tx applied")?;
    wait_for_proposal_status(
        &alice,
        &proposal_id_hex,
        "Approved",
        Duration::from_secs(180),
    )
    .await
    .wrap_err("wait for approved proposal status before enactment")?;

    let enact_tx_hash = alice
        .submit(EnactReferendum {
            referendum_id: proposal_id,
            preimage_hash: [0; 32],
            at_window: AtWindow {
                lower: 0,
                upper: u64::MAX,
            },
        })
        .wrap_err("enact referendum")?;
    wait_for_tx_applied(
        &http,
        &alice.torii_url,
        &hex::encode(enact_tx_hash.as_ref()),
        Duration::from_secs(180),
        "wait for enact referendum tx to be applied",
    )
    .await
    .wrap_err("wait for enact referendum tx applied")?;
    wait_for_referendum_status(&alice, &referendum_id, "Closed", Duration::from_secs(180))
        .await
        .wrap_err("wait for closed referendum status")?;
    wait_for_proposal_status(
        &alice,
        &proposal_id_hex,
        "Enacted",
        Duration::from_secs(180),
    )
    .await
    .wrap_err("wait for enacted proposal status")?;
    eprintln!("sora smoke: enactment observed for first referendum");

    let referendum_payload = alice.get_gov_referendum_json(&referendum_id)?;
    assert_eq!(
        referendum_payload
            .get("found")
            .and_then(norito::json::Value::as_bool),
        Some(true),
        "referendum endpoint should report existing record"
    );
    let referendum_status = referendum_payload
        .get("referendum")
        .and_then(|value| value.get("status"))
        .and_then(norito::json::Value::as_str)
        .unwrap_or_default();
    assert_eq!(referendum_status, "Closed");

    let proposal_payload = alice.get_gov_proposal_json(&proposal_id_hex)?;
    assert_eq!(
        proposal_payload
            .get("found")
            .and_then(norito::json::Value::as_bool),
        Some(true),
        "proposal endpoint should report existing record"
    );
    let proposal_status = proposal_payload
        .get("proposal")
        .and_then(|value| value.get("status"))
        .and_then(norito::json::Value::as_str)
        .unwrap_or_default();
    assert_eq!(proposal_status, "Enacted");

    let tally_payload = alice.get_gov_tally_json(&referendum_id)?;
    let approve = tally_payload
        .get("approve")
        .and_then(json_u128)
        .unwrap_or_default();
    let reject = tally_payload
        .get("reject")
        .and_then(json_u128)
        .unwrap_or_default();
    assert!(
        approve > reject,
        "approval votes should exceed rejection votes"
    );

    let instances_payload = alice.get_gov_instances_by_ns_filtered_json(
        namespace,
        Some(contract_id),
        None,
        Some(0),
        Some(50),
        None,
    )?;
    let deployed_instance = instances_payload
        .get("instances")
        .and_then(norito::json::Value::as_array)
        .and_then(|instances| {
            instances.iter().find(|instance| {
                instance
                    .get("contract_id")
                    .and_then(norito::json::Value::as_str)
                    .is_some_and(|cid| cid == contract_id)
            })
        })
        .ok_or_else(|| eyre!("expected deployed contract instance for `{contract_id}`"))?;
    assert_eq!(
        deployed_instance
            .get("code_hash_hex")
            .and_then(norito::json::Value::as_str),
        Some(code_hash_hex.as_str()),
        "deployed instance should be bound to the enacted code hash"
    );

    let manifest_payload = alice
        .get_contract_manifest_json(&code_hash_hex)
        .wrap_err("fetch contract manifest for enacted code hash")?;
    assert_eq!(
        manifest_payload
            .get("manifest")
            .and_then(|manifest| manifest.get("code_hash"))
            .and_then(norito::json::Value::as_str),
        Some(code_hash_hex.as_str()),
        "manifest code hash should match enacted contract hash"
    );
    assert_eq!(
        manifest_payload
            .get("manifest")
            .and_then(|manifest| manifest.get("abi_hash"))
            .and_then(norito::json::Value::as_str),
        Some(abi_hash_hex.as_str()),
        "manifest abi hash should match proposed ABI hash"
    );
    eprintln!("sora smoke: deployment side effects verified");

    alice
        .submit(ProposeDeployContract {
            namespace: namespace.to_string(),
            contract_id: reject_contract_id.to_string(),
            code_hash_hex: reject_code_hash_hex.clone(),
            abi_hash_hex: abi_hash_hex.clone(),
            abi_version: "1".to_string(),
            window: None,
            mode: Some(VotingMode::Plain),
            manifest_provenance: Some(manifest_provenance(
                &reject_code_hash_hex,
                &abi_hash_hex,
                &ALICE_KEYPAIR,
            )),
        })
        .wrap_err("propose second governance contract deployment referendum for rejection path")?;
    wait_for_proposal_found(&alice, &reject_proposal_id_hex, Duration::from_secs(180))
        .await
        .wrap_err("wait for second governance proposal to be queryable")?;
    eprintln!("sora smoke: rejection-path proposal submitted");

    for (approval_idx, council_account_id) in council_members.iter().enumerate() {
        let (_, council_key_pair) = citizens
            .iter()
            .find(|(account_id, _)| account_id == council_account_id)
            .expect("council account should have a generated key pair");
        let mut council_member_client =
            ready_peer.client_for(council_account_id, council_key_pair.private_key().clone());
        tune_client_timeouts(&mut council_member_client);
        council_member_client
            .submit_all([
                ApproveGovernanceProposal {
                    body: ParliamentBody::RulesCommittee,
                    proposal_id: reject_proposal_id,
                },
                ApproveGovernanceProposal {
                    body: ParliamentBody::AgendaCouncil,
                    proposal_id: reject_proposal_id,
                },
            ])
            .wrap_err_with(|| {
                format!(
                    "approve rejection-path proposal in required parliament bodies with council member #{approval_idx} ({council_account_id})"
                )
            })?;
    }
    wait_for_referendum_found(&alice, &reject_referendum_id, Duration::from_secs(180))
        .await
        .wrap_err("wait for rejection-path referendum record to appear")?;
    wait_for_referendum_status(
        &alice,
        &reject_referendum_id,
        "Open",
        Duration::from_secs(180),
    )
    .await
    .wrap_err("wait for rejection-path referendum status to open")?;

    for (idx, (account_id, key_pair)) in citizens
        .iter()
        .skip(FIRST_REFERENDUM_VOTERS)
        .take(SECOND_REFERENDUM_VOTERS)
        .enumerate()
    {
        let mut citizen_client = ready_peer.client_for(account_id, key_pair.private_key().clone());
        tune_client_timeouts(&mut citizen_client);
        citizen_client
            .submit(CastPlainBallot {
                referendum_id: reject_referendum_id.clone(),
                owner: account_id.clone(),
                amount: BALLOT_LOCK,
                duration_blocks: BALLOT_DURATION_BLOCKS,
                direction: if idx < SECOND_REFERENDUM_REJECT_VOTERS {
                    1
                } else {
                    0
                },
            })
            .wrap_err_with(|| {
                format!("cast rejection-path citizen ballot #{idx} ({account_id})")
            })?;
    }
    wait_for_tally_total(
        &alice,
        &reject_referendum_id,
        expected_plain_total_weight(SECOND_REFERENDUM_VOTERS),
        Duration::from_secs(180),
    )
    .await
    .wrap_err("wait for rejection-path ballot tally ingestion")?;

    let reject_finalize_tx_hash = alice
        .submit(FinalizeReferendum {
            referendum_id: reject_referendum_id.clone(),
            proposal_id: reject_proposal_id,
        })
        .wrap_err("finalize rejection-path referendum")?;
    wait_for_tx_applied(
        &http,
        &alice.torii_url,
        &hex::encode(reject_finalize_tx_hash.as_ref()),
        Duration::from_secs(180),
        "wait for rejection-path finalize tx to be applied",
    )
    .await
    .wrap_err("wait for rejection-path finalize tx applied")?;
    wait_for_proposal_status(
        &alice,
        &reject_proposal_id_hex,
        "Rejected",
        Duration::from_secs(180),
    )
    .await
    .wrap_err("wait for rejection-path proposal status")?;

    let rejected_enact_tx_hash = alice
        .submit(EnactReferendum {
            referendum_id: reject_proposal_id,
            preimage_hash: [0; 32],
            at_window: AtWindow {
                lower: 0,
                upper: u64::MAX,
            },
        })
        .wrap_err("submit enactment for rejected proposal (negative path)")?;
    wait_for_tx_rejected(
        &http,
        &alice.torii_url,
        &hex::encode(rejected_enact_tx_hash.as_ref()),
        Duration::from_secs(180),
        "wait for rejected proposal enactment tx to be rejected",
    )
    .await
    .wrap_err("enactment of rejected proposal should be rejected")?;

    let reject_tally_payload = alice.get_gov_tally_json(&reject_referendum_id)?;
    let reject_path_approve = reject_tally_payload
        .get("approve")
        .and_then(json_u128)
        .unwrap_or_default();
    let reject_path_reject = reject_tally_payload
        .get("reject")
        .and_then(json_u128)
        .unwrap_or_default();
    assert!(
        reject_path_reject > reject_path_approve,
        "rejection-path votes should reject the proposal"
    );

    let reject_proposal_payload = alice.get_gov_proposal_json(&reject_proposal_id_hex)?;
    let reject_proposal_status = reject_proposal_payload
        .get("proposal")
        .and_then(|value| value.get("status"))
        .and_then(norito::json::Value::as_str)
        .unwrap_or_default();
    assert_eq!(
        reject_proposal_status, "Rejected",
        "second proposal should finish as rejected"
    );
    eprintln!("sora smoke: rejection-path referendum verified");

    Ok(())
}
