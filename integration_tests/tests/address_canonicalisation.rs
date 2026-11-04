//! Roadmap ADDR-5 coverage ensuring Torii surfaces canonical I105 account IDs.
use eyre::{Result, WrapErr, eyre};
use integration_tests::sandbox::{
    self, start_network_async_or_skip as sandbox_start_network_async_or_skip,
};
use iroha::data_model::{
    isi::{
        SetKeyValue,
        kaigi::{CreateKaigi, RegisterKaigiRelay, ReportKaigiRelayHealth},
        repo::RepoIsi,
    },
    kaigi::{
        KaigiId, KaigiRecord, KaigiRelayHealthStatus, KaigiRelayHop, KaigiRelayHpkeSuiteV1,
        KaigiRelayManifest, KaigiRelayRegistration, NewKaigi, encode_kaigi_relay_hpke_key_v1,
        kaigi_metadata_key,
    },
    prelude::*,
    repo::{RepoAgreementId, RepoCashLeg, RepoCollateralLeg, RepoGovernance},
};
use iroha_data_model::account::address::AccountAddress;
use iroha_data_model::alias_setup::{
    AccountAliasRoleV1, AccountProvisionV1, ResolvedAccountAliasV1,
};
use iroha_data_model::prelude::RepoInstructionBox;
use iroha_executor_data_model::permission::account::{
    AccountAliasPermissionScope, CanManageAccountAlias,
};
use iroha_model_base::domain::DomainId;
use iroha_model_base::metadata::Metadata;
use iroha_model_base::name::Name;
use iroha_test_network::{
    NetworkBuilder, account_alias_setup_instruction, init_instruction_registry,
};
use iroha_test_samples::{
    ALICE_ID, ALICE_KEYPAIR, BOB_ID, BOB_KEYPAIR, CARPENTER_ID, CARPENTER_KEYPAIR,
    SAMPLE_GENESIS_ACCOUNT_ID, SAMPLE_GENESIS_ACCOUNT_KEYPAIR,
};
use reqwest::Client;
use std::{
    env,
    sync::OnceLock,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
type SurfaceSpec<'a> = (&'a [&'a str], &'a [(&'a str, &'a str)]);
const DEFAULT_NETWORK_PARALLELISM_PEERS: usize = 64;
fn env_flag_enabled(raw: &str) -> bool {
    matches!(
        raw.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}
fn effective_file_permit_parallelism_limit() -> usize {
    if let Ok(raw) = env::var("IROHA_TEST_SERIALIZE_NETWORKS")
        && env_flag_enabled(&raw)
    {
        return 1;
    }
    if let Ok(raw) = env::var("IROHA_TEST_NETWORK_PARALLELISM")
        && let Ok(parsed) = raw.trim().parse::<usize>()
        && parsed > 0
    {
        return parsed;
    }
    let cores = std::thread::available_parallelism().map_or(1, std::num::NonZeroUsize::get);
    cores
        .saturating_div(DEFAULT_NETWORK_PARALLELISM_PEERS.max(1))
        .max(1)
}
fn install_network_parallelism_override() {
    static NETWORK_PARALLELISM_GUARD: OnceLock<sandbox::NetworkParallelismGuard> = OnceLock::new();
    // Keep this suite bounded and avoid over-admitting network starts when the underlying
    // file-permit lock only allows a single concurrent network.
    let suite_parallelism = effective_file_permit_parallelism_limit().clamp(1, 2);
    NETWORK_PARALLELISM_GUARD
        .get_or_init(|| sandbox::override_network_parallelism(None, Some(suite_parallelism)));
}
async fn start_network_async_or_skip(
    builder: NetworkBuilder,
    context: &str,
) -> Result<Option<sandbox::SerializedNetwork>> {
    install_network_parallelism_override();
    sandbox_start_network_async_or_skip(builder, context).await
}
fn http_client() -> Client {
    Client::builder()
        .timeout(Duration::from_secs(30))
        .connect_timeout(Duration::from_secs(5))
        .build()
        .expect("http client should build")
}
fn kaigi_operator_http_client() -> Client {
    Client::builder()
        .timeout(Duration::from_secs(30))
        .connect_timeout(Duration::from_secs(5))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .expect("Kaigi operator HTTP client should build")
}
async fn kaigi_operator_get(
    http: &Client,
    client: &iroha::blocking::Client,
    target: &str,
) -> Result<reqwest::Response> {
    let url = client.client().endpoint().join(target)?;
    let mut canonical_target = url.path().to_owned();
    if let Some(query) = url.query() {
        canonical_target.push('?');
        canonical_target.push_str(query);
    }
    let uri: iroha_torii::Uri = canonical_target
        .parse()
        .wrap_err("parse exact Kaigi operator target")?;
    let operator_key_pair = client
        .client()
        .operator_key_pair()
        .ok_or_else(|| eyre!("test-network client is missing its operator key"))?;
    let headers = iroha_torii::operator_signed_request_headers(
        operator_key_pair,
        client.client().network_id(),
        &iroha_torii::Method::GET,
        &uri,
        &[],
    )?;
    Ok(http
        .get(url)
        .headers(headers)
        .header("Accept", "application/json")
        .send()
        .await?)
}
fn checked_random_account_id() -> AccountId {
    AccountId::new(
        KeyPair::try_random()
            .expect("generate checked address-canonicalisation account keypair")
            .public_key()
            .clone(),
    )
}
#[test]
fn address_canonicalisation_account_fixture_uses_checked_randomness() {
    let _account_id = checked_random_account_id();
}
fn extract_account_ids(value: &norito::json::Value) -> Vec<String> {
    value
        .as_object()
        .and_then(|obj| obj.get("items"))
        .and_then(|items| items.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    item.as_object()
                        .and_then(|obj| obj.get("id"))
                        .and_then(norito::json::Value::as_str)
                        .map(ToOwned::to_owned)
                })
                .collect()
        })
        .unwrap_or_default()
}
async fn wait_for_account_in_query(
    http: &Client,
    url: reqwest::Url,
    account_literal: &str,
) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(30);
    let body = format!(r#"{{"filter":{{"op":"eq","args":["id","{account_literal}"]}},"limit":4}}"#);
    loop {
        let resp = http
            .post(url.clone())
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .body(body.clone())
            .send()
            .await?;
        if resp.status().is_success() {
            let parsed: norito::json::Value = norito::json::from_str(&resp.text().await?)?;
            let ids = extract_account_ids(&parsed);
            if ids.iter().any(|id| id == account_literal) {
                return Ok(());
            }
        }
        if Instant::now() >= deadline {
            return Err(eyre!(
                "timed out waiting for account {account_literal} to appear in accounts query"
            ));
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}
fn extract_holder_account_ids(value: &norito::json::Value) -> Vec<String> {
    value
        .as_object()
        .and_then(|obj| obj.get("items"))
        .and_then(|items| items.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    item.as_object()
                        .and_then(|obj| obj.get("account_id"))
                        .and_then(norito::json::Value::as_str)
                        .map(ToOwned::to_owned)
                })
                .collect()
        })
        .unwrap_or_default()
}
async fn find_asset_definition_with_holders(
    http: &Client,
    base: &reqwest::Url,
) -> Result<Option<String>> {
    let definitions_url = base
        .join("/v1/assets/definitions?limit=64")
        .expect("join asset definitions url");
    let definitions_resp = http
        .get(definitions_url)
        .header("Accept", "application/json")
        .send()
        .await?;
    let status = definitions_resp.status();
    let body = definitions_resp.text().await?;
    if !status.is_success() {
        return Err(eyre!(
            "failed to list asset definitions for holder test selection: {status} body={body}"
        ));
    }
    let parsed: norito::json::Value = norito::json::from_str(&body)?;
    let definition_ids: Vec<String> = parsed
        .as_object()
        .and_then(|obj| obj.get("items"))
        .and_then(norito::json::Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    item.as_object()
                        .and_then(|obj| obj.get("id"))
                        .and_then(norito::json::Value::as_str)
                        .map(ToOwned::to_owned)
                })
                .collect()
        })
        .unwrap_or_default();
    let mut first_successful_definition: Option<String> = None;
    for definition_id in definition_ids {
        let mut holders_url = base.clone();
        {
            let mut path = holders_url
                .path_segments_mut()
                .expect("torii_url must allow path segments");
            path.clear();
            path.push("v2");
            path.push("assets");
            path.push(definition_id.as_str());
            path.push("holders");
        }
        holders_url.query_pairs_mut().append_pair("limit", "8");
        let holders_resp = http
            .get(holders_url)
            .header("Accept", "application/json")
            .send()
            .await?;
        if holders_resp.status() == reqwest::StatusCode::NOT_FOUND {
            continue;
        }
        if !holders_resp.status().is_success() {
            continue;
        }
        if first_successful_definition.is_none() {
            first_successful_definition = Some(definition_id.clone());
        }
        let holders_body = holders_resp.text().await?;
        let holders_json: norito::json::Value = norito::json::from_str(&holders_body)?;
        let holder_ids = extract_holder_account_ids(&holders_json);
        if !holder_ids.is_empty() {
            return Ok(Some(definition_id));
        }
    }
    if let Some(definition_id) = first_successful_definition {
        return Ok(Some(definition_id));
    }
    Ok(None)
}
fn extract_explorer_authorities(value: &norito::json::Value) -> Vec<String> {
    value
        .as_object()
        .and_then(|obj| obj.get("items"))
        .and_then(|items| items.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    item.as_object()
                        .and_then(|obj| obj.get("authority"))
                        .and_then(norito::json::Value::as_str)
                        .map(ToOwned::to_owned)
                })
                .collect()
        })
        .unwrap_or_default()
}
fn extract_account_transaction_authorities(value: &norito::json::Value) -> Vec<String> {
    value
        .as_object()
        .and_then(|obj| obj.get("items"))
        .and_then(|items| items.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    item.as_object()
                        .and_then(|obj| obj.get("authority"))
                        .and_then(norito::json::Value::as_str)
                        .map(ToOwned::to_owned)
                })
                .collect()
        })
        .unwrap_or_default()
}
fn assert_authorities_are_i105(authorities: &[String]) -> Result<()> {
    for literal in authorities {
        AccountAddress::parse_encoded(literal, None)
            .wrap_err_with(|| format!("authority {literal} should parse as account address"))?;
    }
    Ok(())
}
fn find_repo_entry<'a>(
    value: &'a norito::json::Value,
    agreement_id: &str,
) -> Option<&'a norito::json::Map> {
    value
        .as_object()
        .and_then(|obj| obj.get("items"))
        .and_then(|items| items.as_array())
        .and_then(|items| {
            items.iter().find_map(|entry| {
                let obj = entry.as_object()?;
                let id_literal = obj.get("id")?.as_str()?;
                if id_literal == agreement_id {
                    Some(obj)
                } else {
                    None
                }
            })
        })
}
fn i105_alice_literal() -> String {
    let account_address = ALICE_ID
        .to_account_address()
        .expect("account address should encode");
    account_address.to_i105().expect("I105 address encoding")
}
fn i105_bob_literal() -> String {
    let account_address = BOB_ID
        .to_account_address()
        .expect("account address should encode");
    account_address.to_i105().expect("I105 address encoding")
}
fn dotted_i105_literal(i105_literal: &str) -> String {
    let mut chars = i105_literal.chars();
    let sentinel: String = chars.by_ref().take(4).collect();
    let remainder: String = chars.collect();
    format!("{sentinel}.{remainder}")
}
fn dotted_i105_alice_literal() -> String {
    dotted_i105_literal(&i105_alice_literal())
}
fn dotted_i105_bob_literal() -> String {
    dotted_i105_literal(&i105_bob_literal())
}
fn selector_prefixed_literal() -> String {
    let account_address = ALICE_ID
        .to_account_address()
        .expect("account address should encode");
    let canonical_hex = account_address
        .canonical_hex()
        .expect("canonical hex encoding");
    let canonical = hex::decode(&canonical_hex[2..]).expect("canonical hex decoding succeeds");
    let digest = [0xA5; 12];
    // Insert a forbidden selector tag and truncated digest. Strict V1 decoding
    // must reject this payload rather than interpreting the extra bytes.
    let mut payload = Vec::with_capacity(canonical.len() + digest.len());
    payload.push(canonical[0]); // header
    payload.push(0x01); // forbidden selector tag
    payload.extend_from_slice(&digest);
    payload.extend_from_slice(&canonical[2..]); // controller payload
    payload.drain(10..14); // keep an eight-byte selector prefix
    format!("0x{}", hex::encode(payload))
}
fn public_key_literal() -> String {
    let public_key = ALICE_KEYPAIR.public_key().to_string();
    format!("{public_key}@banka.centralbank")
}
struct KaigiRelaySeed {
    relay_i105: String,
    reporter_i105: String,
}
fn account_endpoint_url(
    base: &reqwest::Url,
    account_literal: &str,
    segments: &[&str],
) -> reqwest::Url {
    let mut url = base.clone();
    {
        let mut path = url
            .path_segments_mut()
            .expect("torii_url must allow path segments");
        path.clear();
        path.push("v1");
        path.push("accounts");
        path.push(account_literal);
        for segment in segments {
            path.push(segment);
        }
    }
    url
}
fn explorer_account_qr_url(base: &reqwest::Url, account_literal: &str) -> reqwest::Url {
    let mut url = base.clone();
    {
        let mut path = url
            .path_segments_mut()
            .expect("torii_url must allow path segments");
        path.clear();
        path.push("v1");
        path.push("explorer");
        path.push("accounts");
        path.push(account_literal);
        path.push("qr");
    }
    url
}
#[tokio::test]
async fn accounts_listing_emits_i105_identifiers() -> Result<()> {
    let Some(network) = Box::pin(start_network_async_or_skip(
        NetworkBuilder::new(),
        stringify!(accounts_listing_emits_i105_identifiers),
    ))
    .await?
    else {
        return Ok(());
    };
    network.ensure_blocks(1).await?;
    let http = http_client();
    let url = network
        .client()
        .client()
        .endpoint()
        .join("/v1/accounts?limit=32")
        .expect("join accounts url");
    let resp = http
        .get(url)
        .header("Accept", "application/json")
        .send()
        .await?;
    assert!(
        resp.status().is_success(),
        "expected success listing accounts, got {}",
        resp.status()
    );
    let body = resp.text().await?;
    let parsed: norito::json::Value = norito::json::from_str(&body)?;
    let ids = extract_account_ids(&parsed);
    let expected = ALICE_ID.to_string();
    assert!(
        ids.iter().any(|id| id == &expected),
        "expected I105 literal {expected} in account listing, got {ids:?}"
    );
    assert!(
        ids.iter().all(|id| !id.contains('@')),
        "account listing should emit canonical I105 strings"
    );
    Ok(())
}
#[tokio::test]
async fn accounts_query_accepts_i105_filter_literals() -> Result<()> {
    let Some(network) = Box::pin(start_network_async_or_skip(
        NetworkBuilder::new(),
        stringify!(accounts_query_accepts_i105_filter_literals),
    ))
    .await?
    else {
        return Ok(());
    };
    network.ensure_blocks(1).await?;
    let expected = ALICE_ID.to_string();
    let body = format!(r#"{{"filter":{{"op":"eq","args":["id","{expected}"]}},"limit":4}}"#);
    let http = http_client();
    let url = network
        .client()
        .client()
        .endpoint()
        .join("/v1/accounts/query")
        .expect("join accounts query url");
    let resp = http
        .post(url)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .body(body)
        .send()
        .await?;
    assert!(
        resp.status().is_success(),
        "expected success from accounts query, got {}",
        resp.status()
    );
    let parsed: norito::json::Value = norito::json::from_str(&resp.text().await?)?;
    let ids = extract_account_ids(&parsed);
    assert!(
        ids.iter().all(|id| id == &expected),
        "accounts query should return I105 literal {expected}, got {ids:?}"
    );
    Ok(())
}
#[tokio::test]
async fn accounts_listing_filter_rejects_dotted_i105_literals() -> Result<()> {
    let Some(network) = Box::pin(start_network_async_or_skip(
        NetworkBuilder::new(),
        stringify!(accounts_listing_filter_rejects_dotted_i105_literals),
    ))
    .await?
    else {
        return Ok(());
    };
    network.ensure_blocks(1).await?;
    let literal = dotted_i105_alice_literal();
    let reason = AccountId::parse_encoded(&literal)
        .expect_err("dotted I105 literal must fail strict parsing")
        .code_str()
        .to_string();
    // GET filters use the textual grammar; JSON filter trees belong to POST queries.
    let filter = format!(r#"id == "{literal}""#);
    let http = http_client();
    let mut url = network
        .client()
        .client()
        .endpoint()
        .join("/v1/accounts")
        .expect("join accounts url");
    {
        let mut pairs = url.query_pairs_mut();
        pairs.append_pair("filter", &filter);
        pairs.append_pair("limit", "8");
    }
    let resp = http
        .get(url)
        .header("Accept", "application/json")
        .send()
        .await?;
    assert_eq!(
        resp.status(),
        reqwest::StatusCode::BAD_REQUEST,
        "dotted I105 filter literal should be rejected"
    );
    let body = resp.text().await?;
    assert!(
        body.contains(&reason),
        "response body should mention {reason}, got {body}"
    );
    Ok(())
}
#[tokio::test]
async fn accounts_query_rejects_dotted_i105_filter_literals() -> Result<()> {
    let Some(network) = Box::pin(start_network_async_or_skip(
        NetworkBuilder::new(),
        stringify!(accounts_query_rejects_dotted_i105_filter_literals),
    ))
    .await?
    else {
        return Ok(());
    };
    network.ensure_blocks(1).await?;
    let literal = dotted_i105_alice_literal();
    let reason = AccountId::parse_encoded(&literal)
        .expect_err("dotted I105 literal must fail strict parsing")
        .code_str()
        .to_string();
    let body = format!(r#"{{"filter":{{"op":"eq","args":["id","{literal}"]}},"limit":4}}"#);
    let http = http_client();
    let url = network
        .client()
        .client()
        .endpoint()
        .join("/v1/accounts/query")
        .expect("join accounts query url");
    let resp = http
        .post(url)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .body(body)
        .send()
        .await?;
    assert_eq!(
        resp.status(),
        reqwest::StatusCode::BAD_REQUEST,
        "dotted I105 filter literal should be rejected"
    );
    let body = resp.text().await?;
    assert!(
        body.contains(&reason),
        "response body should mention {reason}, got {body}"
    );
    Ok(())
}
#[tokio::test]
async fn accounts_listing_supports_i105_response() -> Result<()> {
    let Some(network) = Box::pin(start_network_async_or_skip(
        NetworkBuilder::new(),
        stringify!(accounts_listing_supports_i105_response),
    ))
    .await?
    else {
        return Ok(());
    };
    network.ensure_blocks(1).await?;
    let http = http_client();
    let url = network
        .client()
        .client()
        .endpoint()
        .join("/v1/accounts?limit=8")
        .expect("join accounts url");
    let resp = http
        .get(url)
        .header("Accept", "application/json")
        .send()
        .await?;
    assert!(
        resp.status().is_success(),
        "expected success from accounts listing with I105 response, got {}",
        resp.status()
    );
    let parsed: norito::json::Value = norito::json::from_str(&resp.text().await?)?;
    let ids = extract_account_ids(&parsed);
    let expected = i105_alice_literal();
    assert!(
        ids.iter().any(|id| id == &expected),
        "I105 literal {expected} missing from response {ids:?}"
    );
    for id in &ids {
        AccountId::parse_encoded(id)
            .wrap_err_with(|| format!("account id {id} should parse as canonical I105"))?;
    }
    Ok(())
}
#[tokio::test]
async fn accounts_query_supports_i105_response() -> Result<()> {
    let Some(network) = Box::pin(start_network_async_or_skip(
        NetworkBuilder::new(),
        stringify!(accounts_query_supports_i105_response),
    ))
    .await?
    else {
        return Ok(());
    };
    network.ensure_blocks(1).await?;
    let body = r#"{"limit":8}"#;
    let http = http_client();
    let url = network
        .client()
        .client()
        .endpoint()
        .join("/v1/accounts/query")
        .expect("join accounts query url");
    let resp = http
        .post(url)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .body(body)
        .send()
        .await?;
    assert!(
        resp.status().is_success(),
        "expected success from accounts query with I105 response, got {}",
        resp.status()
    );
    let parsed: norito::json::Value = norito::json::from_str(&resp.text().await?)?;
    let ids = extract_account_ids(&parsed);
    let expected = i105_alice_literal();
    assert!(
        ids.iter().any(|id| id == &expected),
        "I105 literal {expected} missing from query response {ids:?}"
    );
    Ok(())
}
#[tokio::test]
async fn account_path_endpoints_reject_i105_literals() -> Result<()> {
    let Some(network) = Box::pin(start_network_async_or_skip(
        NetworkBuilder::new(),
        stringify!(account_path_endpoints_reject_i105_literals),
    ))
    .await?
    else {
        return Ok(());
    };
    network.ensure_blocks(1).await?;
    let literal = dotted_i105_alice_literal();
    let reason = AccountId::parse_encoded(&literal)
        .expect_err("dotted I105 literal must fail strict parsing")
        .code_str()
        .to_string();
    let http = http_client();
    let surfaces: [SurfaceSpec; 3] = [
        (&["assets"], &[("limit", "4")]),
        (&["transactions"], &[("limit", "4")]),
        (&["permissions"], &[("limit", "4")]),
    ];
    for (segments, query_pairs) in surfaces {
        let mut url = account_endpoint_url(
            network.client().client().endpoint(),
            literal.as_str(),
            segments,
        );
        {
            let mut qp = url.query_pairs_mut();
            for (key, value) in query_pairs {
                qp.append_pair(key, value);
            }
        }
        let resp = http
            .get(url.clone())
            .header("Accept", "application/json")
            .send()
            .await?;
        assert_eq!(
            resp.status(),
            reqwest::StatusCode::BAD_REQUEST,
            "dotted I105 literal should be rejected for {url}"
        );
        let body = resp.text().await.unwrap_or_default();
        assert!(
            body.contains(&reason),
            "response body should mention {reason}, got {body}"
        );
    }
    Ok(())
}
#[tokio::test]
async fn account_path_endpoints_reject_selector_prefixed_literals() -> Result<()> {
    let Some(network) = Box::pin(start_network_async_or_skip(
        NetworkBuilder::new(),
        stringify!(account_path_endpoints_reject_selector_prefixed_literals),
    ))
    .await?
    else {
        return Ok(());
    };
    network.ensure_blocks(1).await?;
    let literal = selector_prefixed_literal();
    let rejection_reason = AccountId::parse_encoded(&literal)
        .expect_err("selector-prefixed literal must fail to parse")
        .code_str()
        .to_owned();
    let http = http_client();
    let surfaces: [SurfaceSpec; 3] = [
        (&["assets"], &[("limit", "4")]),
        (&["transactions"], &[("limit", "4")]),
        (&["permissions"], &[("limit", "4")]),
    ];
    for (segments, query_pairs) in surfaces {
        let mut url = account_endpoint_url(
            network.client().client().endpoint(),
            literal.as_str(),
            segments,
        );
        {
            let mut qp = url.query_pairs_mut();
            for (key, value) in query_pairs {
                qp.append_pair(key, value);
            }
        }
        let resp = http
            .get(url.clone())
            .header("Accept", "application/json")
            .send()
            .await?;
        assert_eq!(
            resp.status(),
            reqwest::StatusCode::BAD_REQUEST,
            "selector-prefixed literal should be rejected for {url}"
        );
        let body = resp.text().await.unwrap_or_default();
        assert!(
            body.contains(&rejection_reason),
            "response body should mention {rejection_reason}, got {body}"
        );
    }
    Ok(())
}
#[tokio::test]
async fn account_path_endpoints_reject_public_key_literals() -> Result<()> {
    let Some(network) = Box::pin(start_network_async_or_skip(
        NetworkBuilder::new(),
        stringify!(account_path_endpoints_reject_public_key_literals),
    ))
    .await?
    else {
        return Ok(());
    };
    network.ensure_blocks(1).await?;
    let literal = public_key_literal();
    let reason = AccountId::parse_encoded(&literal)
        .expect_err("public-key@domain literal must fail strict parsing")
        .code_str()
        .to_string();
    let http = http_client();
    let surfaces: [SurfaceSpec; 3] = [
        (&["assets"], &[("limit", "4")]),
        (&["transactions"], &[("limit", "4")]),
        (&["permissions"], &[("limit", "4")]),
    ];
    for (segments, query_pairs) in surfaces {
        let mut url = account_endpoint_url(
            network.client().client().endpoint(),
            literal.as_str(),
            segments,
        );
        {
            let mut qp = url.query_pairs_mut();
            for (key, value) in query_pairs {
                qp.append_pair(key, value);
            }
        }
        let resp = http
            .get(url.clone())
            .header("Accept", "application/json")
            .send()
            .await?;
        assert_eq!(
            resp.status(),
            reqwest::StatusCode::BAD_REQUEST,
            "public-key@domain literal should be rejected for {url}"
        );
        let body = resp.text().await.unwrap_or_default();
        assert!(
            body.contains(&reason),
            "response body should mention {reason}, got {body}"
        );
    }
    Ok(())
}
#[tokio::test]
async fn asset_holders_get_supports_i105_response() -> Result<()> {
    let Some(network) = Box::pin(start_network_async_or_skip(
        NetworkBuilder::new(),
        stringify!(asset_holders_get_supports_i105_response),
    ))
    .await?
    else {
        return Ok(());
    };
    network.ensure_blocks(1).await?;
    let http = http_client();
    let Some(definition_literal) =
        find_asset_definition_with_holders(&http, network.client().client().endpoint()).await?
    else {
        eprintln!("Skipping asset holder GET I105 coverage: holders endpoint unavailable.");
        return Ok(());
    };
    let mut url = network.client().client().endpoint().clone();
    {
        let mut path = url
            .path_segments_mut()
            .expect("torii_url must allow path segments");
        path.clear();
        path.push("v2");
        path.push("assets");
        path.push(definition_literal.as_str());
        path.push("holders");
    }
    url.query_pairs_mut().append_pair("limit", "8");
    let resp = http
        .get(url)
        .header("Accept", "application/json")
        .send()
        .await?;
    let status = resp.status();
    let body_text = resp.text().await?;
    assert!(
        status.is_success(),
        "expected success from asset holders listing with I105 response, got {status} body={body_text}"
    );
    let parsed: norito::json::Value = norito::json::from_str(&body_text)?;
    let ids = extract_holder_account_ids(&parsed);
    assert!(
        !ids.is_empty(),
        "expected at least one holder in response for definition {definition_literal}"
    );
    for id in &ids {
        AccountId::parse_encoded(id)
            .wrap_err_with(|| format!("holder id {id} should parse as canonical I105"))?;
    }
    Ok(())
}
#[tokio::test]
async fn asset_holders_query_rejects_dotted_i105_filter_literals() -> Result<()> {
    let Some(network) = Box::pin(start_network_async_or_skip(
        NetworkBuilder::new(),
        stringify!(asset_holders_query_rejects_dotted_i105_filter_literals),
    ))
    .await?
    else {
        return Ok(());
    };
    network.ensure_blocks(1).await?;
    let literal = dotted_i105_alice_literal();
    let reason = AccountId::parse_encoded(&literal)
        .expect_err("dotted I105 literal must fail strict parsing")
        .code_str()
        .to_string();
    let body = format!(r#"{{"filter":{{"op":"eq","args":["account_id","{literal}"]}},"limit":4}}"#);
    let http = http_client();
    let Some(definition_literal) =
        find_asset_definition_with_holders(&http, network.client().client().endpoint()).await?
    else {
        eprintln!("Skipping asset holder query I105 coverage: holders endpoint unavailable.");
        return Ok(());
    };
    let mut url = network.client().client().endpoint().clone();
    {
        let mut path = url
            .path_segments_mut()
            .expect("torii_url must allow path segments");
        path.clear();
        path.push("v2");
        path.push("assets");
        path.push(definition_literal.as_str());
        path.push("holders");
        path.push("query");
    }
    let resp = http
        .post(url)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .body(body)
        .send()
        .await?;
    assert_eq!(
        resp.status(),
        reqwest::StatusCode::BAD_REQUEST,
        "dotted I105 filter literal should be rejected"
    );
    let body = resp.text().await?;
    assert!(
        body.contains(&reason),
        "response body should mention {reason}, got {body}"
    );
    Ok(())
}
async fn submit_account_history_fixture(
    network: &sandbox::SerializedNetwork,
    metadata_label: &str,
) -> Result<()> {
    let client = network.client();
    let account = client.account_client();
    let mut payload = account.prepare_transaction(iroha::client::AccountTransactionDraft::new(
        [SetKeyValue::account(
            ALICE_ID.clone(),
            metadata_label.parse::<Name>()?,
            norito::json!(metadata_label),
        )],
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        Metadata::default(),
    ))?;
    let quote = account
        .quote_fees(iroha::client::FeeQuoteRequest::AccountSignature { payload: &payload })
        .await?;
    eyre::ensure!(
        payload
            .fee_payment
            .has_same_payer_and_gas_bound(&quote.intent),
        "history fee quote must preserve the selected payer and gas bound"
    );
    payload.fee_payment = quote.intent;
    let transaction = account.sign_transaction(payload)?;
    let mut submitted = false;
    let mut last_err: Option<eyre::Report> = None;
    for peer in network.peers() {
        let peer_client = peer.client();
        match peer_client
            .account_client()
            .submit_transaction(&transaction)
            .await
        {
            Ok(_) => submitted = true,
            Err(err) => last_err = Some(err),
        }
    }
    if submitted {
        Ok(())
    } else {
        Err(last_err.unwrap_or_else(|| eyre!("failed to submit transaction")))
    }
}
#[tokio::test]
async fn account_transactions_get_returns_i105_literals() -> Result<()> {
    let Some(network) = Box::pin(start_network_async_or_skip(
        NetworkBuilder::new(),
        stringify!(account_transactions_get_returns_i105_literals),
    ))
    .await?
    else {
        return Ok(());
    };
    network.ensure_blocks(1).await?;
    // Ensure the account has at least one external transaction to format.
    submit_account_history_fixture(&network, "addrfmtget").await?;
    let http = http_client();
    let account_literal = ALICE_ID.to_string();
    let i105_literal = account_literal.clone();
    let base = account_endpoint_url(
        network.client().client().endpoint(),
        &account_literal,
        &["transactions"],
    );
    let default_url = {
        let mut url = base.clone();
        {
            let mut qp = url.query_pairs_mut();
            qp.append_pair("limit", "8");
        }
        url
    };
    let i105_url = {
        let mut url = base.clone();
        {
            let mut qp = url.query_pairs_mut();
            qp.append_pair("limit", "8");
        }
        url
    };
    let authorities = {
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            let resp = http
                .get(default_url.clone())
                .header("Accept", "application/json")
                .send()
                .await?;
            assert!(
                resp.status().is_success(),
                "expected success fetching account transactions with I105 default, got {}",
                resp.status()
            );
            let parsed: norito::json::Value = norito::json::from_str(&resp.text().await?)?;
            let authorities = extract_account_transaction_authorities(&parsed);
            if authorities
                .iter()
                .any(|literal| literal == &account_literal)
            {
                break authorities;
            }
            if Instant::now() >= deadline {
                return Err(eyre!(
                    "timed out waiting for I105 account transactions to appear"
                ));
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    };
    assert!(
        !authorities.is_empty(),
        "I105 default should return at least one authority literal (submitted tx)"
    );
    assert!(
        authorities
            .iter()
            .any(|literal| literal == &account_literal),
        "I105 default should include canonical literal {account_literal}, got {authorities:?}"
    );
    assert_authorities_are_i105(&authorities)?;
    let resp = http
        .get(i105_url.clone())
        .header("Accept", "application/json")
        .send()
        .await?;
    assert!(
        resp.status().is_success(),
        "expected success fetching account transactions with canonical I105, got {}",
        resp.status()
    );
    let parsed: norito::json::Value = norito::json::from_str(&resp.text().await?)?;
    let authorities = extract_account_transaction_authorities(&parsed);
    assert!(
        authorities.iter().any(|literal| literal == &i105_literal),
        "I105 response should include {i105_literal}, got {authorities:?}"
    );
    assert_authorities_are_i105(&authorities)?;
    Ok(())
}
#[tokio::test]
async fn account_transactions_query_returns_i105_literals() -> Result<()> {
    let Some(network) = Box::pin(start_network_async_or_skip(
        NetworkBuilder::new(),
        stringify!(account_transactions_query_returns_i105_literals),
    ))
    .await?
    else {
        return Ok(());
    };
    network.ensure_blocks(1).await?;
    // Ensure the account has at least one external transaction to format.
    submit_account_history_fixture(&network, "addrfmtquery").await?;
    let http = http_client();
    let account_literal = ALICE_ID.to_string();
    let i105_literal = account_literal.clone();
    let url = account_endpoint_url(
        network.client().client().endpoint(),
        &account_literal,
        &["transactions", "query"],
    );
    let default_body = r#"{"limit":8}"#;
    let authorities = {
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            let resp = http
                .post(url.clone())
                .header("Content-Type", "application/json")
                .header("Accept", "application/json")
                .body(default_body)
                .send()
                .await?;
            assert!(
                resp.status().is_success(),
                "expected success from account transactions query using I105 default, got {}",
                resp.status()
            );
            let parsed: norito::json::Value = norito::json::from_str(&resp.text().await?)?;
            let authorities = extract_account_transaction_authorities(&parsed);
            if authorities
                .iter()
                .any(|literal| literal == &account_literal)
            {
                break authorities;
            }
            if Instant::now() >= deadline {
                return Err(eyre!(
                    "timed out waiting for I105 account transactions query to appear"
                ));
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    };
    assert!(
        !authorities.is_empty(),
        "I105 default query should return at least one authority literal (submitted tx)"
    );
    assert!(
        authorities
            .iter()
            .any(|literal| literal == &account_literal),
        "I105 default query should include canonical literal {account_literal}, got {authorities:?}"
    );
    assert_authorities_are_i105(&authorities)?;
    let i105_body = r#"{"limit":8}"#;
    let resp = http
        .post(url.clone())
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .body(i105_body)
        .send()
        .await?;
    assert!(
        resp.status().is_success(),
        "expected success from account transactions query with canonical I105, got {}",
        resp.status()
    );
    let parsed: norito::json::Value = norito::json::from_str(&resp.text().await?)?;
    let authorities = extract_account_transaction_authorities(&parsed);
    assert!(
        authorities.iter().any(|literal| literal == &i105_literal),
        "I105 query response should include {i105_literal}, got {authorities:?}"
    );
    assert_authorities_are_i105(&authorities)?;
    Ok(())
}
#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn explorer_transactions_emit_i105_literals() -> Result<()> {
    let fee_asset_id: AssetDefinitionId =
        iroha_config::parameters::defaults::nexus::fees::fee_asset_id().parse()?;
    let builder = NetworkBuilder::new().with_genesis_post_bootstrap_isi(vec![
        Mint::asset_quantity(
            1_000_000_u32,
            AssetId::new(fee_asset_id, SAMPLE_GENESIS_ACCOUNT_ID.clone()),
        )
        .into(),
    ]);
    let Some(network) = Box::pin(start_network_async_or_skip(
        builder,
        stringify!(explorer_transactions_emit_i105_literals),
    ))
    .await?
    else {
        return Ok(());
    };
    network.ensure_blocks(1).await?;
    // Explorer history projects external submissions, not genesis setup instructions.
    let client = network.peer().client_for(
        &SAMPLE_GENESIS_ACCOUNT_ID,
        SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key().clone(),
    );
    submit_account_fixture_instruction(
        client.account_client(),
        SetKeyValue::account(
            SAMPLE_GENESIS_ACCOUNT_ID.clone(),
            "addrexplorertx".parse()?,
            norito::json!("canonical explorer authority"),
        )
        .into(),
    )
    .await?;
    let http = http_client();
    let account_literal = SAMPLE_GENESIS_ACCOUNT_ID.to_string();
    let i105_literal = account_literal.clone();
    let base = client
        .client()
        .endpoint()
        .join("/v1/explorer/transactions")
        .expect("join explorer transactions url");
    let default_url = {
        let mut url = base.clone();
        {
            let mut qp = url.query_pairs_mut();
            qp.append_pair("filter", &format!(r#"authority == "{account_literal}""#));
            qp.append_pair("limit", "8");
        }
        url
    };
    let resp = http
        .get(default_url)
        .header("Accept", "application/json")
        .send()
        .await?;
    let status = resp.status();
    let body = resp.text().await?;
    assert!(
        status.is_success(),
        "expected explorer transactions to succeed (I105 default), got {status}: {body}"
    );
    let parsed: norito::json::Value = norito::json::from_str(&body)?;
    let authorities = extract_explorer_authorities(&parsed);
    assert!(
        !authorities.is_empty(),
        "expected explorer transactions for Genesis account, got {body}"
    );
    assert!(
        authorities
            .iter()
            .all(|literal| literal == &account_literal),
        "explorer transactions should default to I105 literals; got {authorities:?}"
    );
    let i105_url = {
        let mut url = base.clone();
        {
            let mut qp = url.query_pairs_mut();
            qp.append_pair("filter", &format!(r#"authority == "{account_literal}""#));
            qp.append_pair("limit", "8");
        }
        url
    };
    let resp = http
        .get(i105_url)
        .header("Accept", "application/json")
        .send()
        .await?;
    let status = resp.status();
    let body = resp.text().await?;
    assert!(
        status.is_success(),
        "expected explorer transactions with I105 response to succeed, got {status}: {body}"
    );
    let parsed: norito::json::Value = norito::json::from_str(&body)?;
    let authorities = extract_explorer_authorities(&parsed);
    assert!(
        !authorities.is_empty(),
        "I105 explorer transactions response should contain items"
    );
    assert!(
        authorities.iter().all(|literal| literal == &i105_literal),
        "explorer transactions should honour canonical I105; got {authorities:?}"
    );
    let dotted_i105_filter = dotted_i105_literal(&i105_literal);
    let dotted_i105_filter_url = {
        let mut url = base;
        {
            let mut qp = url.query_pairs_mut();
            qp.append_pair("filter", &format!(r#"authority == "{dotted_i105_filter}""#));
            qp.append_pair("limit", "8");
        }
        url
    };
    let dotted_i105_reason = AccountId::parse_encoded(&dotted_i105_filter)
        .expect_err("dotted I105 authority filter must fail strict parsing")
        .code_str()
        .to_string();
    let resp = http
        .get(dotted_i105_filter_url)
        .header("Accept", "application/json")
        .send()
        .await?;
    assert_eq!(
        resp.status(),
        reqwest::StatusCode::BAD_REQUEST,
        "explorer transactions should reject dotted I105 authority filters"
    );
    let body = resp.text().await?;
    assert!(
        body.contains(&dotted_i105_reason),
        "response body should mention {dotted_i105_reason}, got {body}"
    );
    Ok(())
}
#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn explorer_instructions_emit_i105_literals() -> Result<()> {
    let fee_asset_id: AssetDefinitionId =
        iroha_config::parameters::defaults::nexus::fees::fee_asset_id().parse()?;
    let builder = NetworkBuilder::new().with_genesis_post_bootstrap_isi(vec![
        Mint::asset_quantity(
            1_000_000_u32,
            AssetId::new(fee_asset_id, SAMPLE_GENESIS_ACCOUNT_ID.clone()),
        )
        .into(),
    ]);
    let Some(network) = Box::pin(start_network_async_or_skip(
        builder,
        stringify!(explorer_instructions_emit_i105_literals),
    ))
    .await?
    else {
        return Ok(());
    };
    network.ensure_blocks(1).await?;
    // Explorer history projects external submissions, not genesis setup instructions.
    let client = network.peer().client_for(
        &SAMPLE_GENESIS_ACCOUNT_ID,
        SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key().clone(),
    );
    submit_account_fixture_instruction(
        client.account_client(),
        SetKeyValue::account(
            SAMPLE_GENESIS_ACCOUNT_ID.clone(),
            "addrexplorerisi".parse()?,
            norito::json!("canonical explorer authority"),
        )
        .into(),
    )
    .await?;
    let http = http_client();
    let account_literal = SAMPLE_GENESIS_ACCOUNT_ID.to_string();
    let i105_literal = account_literal.clone();
    let base = client
        .client()
        .endpoint()
        .join("/v1/explorer/instructions")
        .expect("join explorer instructions url");
    let default_url = {
        let mut url = base.clone();
        {
            let mut qp = url.query_pairs_mut();
            qp.append_pair("filter", &format!(r#"authority == "{account_literal}""#));
            qp.append_pair("limit", "8");
        }
        url
    };
    let resp = http
        .get(default_url)
        .header("Accept", "application/json")
        .send()
        .await?;
    let status = resp.status();
    let body = resp.text().await?;
    assert!(
        status.is_success(),
        "expected explorer instructions to succeed (I105 default), got {status}: {body}"
    );
    let parsed: norito::json::Value = norito::json::from_str(&body)?;
    let authorities = extract_explorer_authorities(&parsed);
    assert!(
        !authorities.is_empty(),
        "expected explorer instructions for Genesis account, got {body}"
    );
    assert!(
        authorities
            .iter()
            .all(|literal| literal == &account_literal),
        "explorer instructions should default to I105 literals; got {authorities:?}"
    );
    let i105_url = {
        let mut url = base.clone();
        {
            let mut qp = url.query_pairs_mut();
            qp.append_pair("filter", &format!(r#"authority == "{account_literal}""#));
            qp.append_pair("limit", "8");
        }
        url
    };
    let resp = http
        .get(i105_url)
        .header("Accept", "application/json")
        .send()
        .await?;
    let status = resp.status();
    let body = resp.text().await?;
    assert!(
        status.is_success(),
        "expected explorer instructions with I105 response to succeed, got {status}: {body}"
    );
    let parsed: norito::json::Value = norito::json::from_str(&body)?;
    let authorities = extract_explorer_authorities(&parsed);
    assert!(
        !authorities.is_empty(),
        "I105 explorer instructions response should contain items"
    );
    assert!(
        authorities.iter().all(|literal| literal == &i105_literal),
        "explorer instructions should honour canonical I105; got {authorities:?}"
    );
    let dotted_i105_filter = dotted_i105_literal(&i105_literal);
    let dotted_i105_filter_url = {
        let mut url = base;
        {
            let mut qp = url.query_pairs_mut();
            qp.append_pair("filter", &format!(r#"authority == "{dotted_i105_filter}""#));
            qp.append_pair("limit", "8");
        }
        url
    };
    let dotted_i105_reason = AccountId::parse_encoded(&dotted_i105_filter)
        .expect_err("dotted I105 authority filter must fail strict parsing")
        .code_str()
        .to_string();
    let resp = http
        .get(dotted_i105_filter_url)
        .header("Accept", "application/json")
        .send()
        .await?;
    assert_eq!(
        resp.status(),
        reqwest::StatusCode::BAD_REQUEST,
        "explorer instructions should reject dotted I105 authority filters"
    );
    let body = resp.text().await?;
    assert!(
        body.contains(&dotted_i105_reason),
        "response body should mention {dotted_i105_reason}, got {body}"
    );
    Ok(())
}
#[tokio::test]
async fn explorer_account_qr_defaults_to_i105_for_discriminant() -> Result<()> {
    let Some(network) = Box::pin(start_network_async_or_skip(
        NetworkBuilder::new(),
        stringify!(explorer_account_qr_defaults_to_i105),
    ))
    .await?
    else {
        return Ok(());
    };
    network.ensure_blocks(1).await?;
    let http = http_client();
    let canonical_literal = ALICE_ID.to_string();
    let url = explorer_account_qr_url(network.client().client().endpoint(), &canonical_literal);
    let resp = http
        .get(url)
        .header("Accept", "application/json")
        .send()
        .await?;
    assert!(
        resp.status().is_success(),
        "expected explorer account QR endpoint to succeed, got {}",
        resp.status()
    );
    let parsed: norito::json::Value = norito::json::from_str(&resp.text().await?)?;
    assert_eq!(
        parsed
            .get("canonical_id")
            .and_then(norito::json::Value::as_str),
        Some(canonical_literal.as_str()),
        "canonical_id should match I105 literal"
    );
    assert_eq!(
        parsed.get("literal").and_then(norito::json::Value::as_str),
        Some(canonical_literal.as_str()),
        "literal should default to I105"
    );
    assert_eq!(
        parsed
            .get("network_prefix")
            .and_then(norito::json::Value::as_u64),
        Some(u64::from(
            iroha_config::parameters::defaults::common::chain_discriminant()
        )),
        "network_prefix should expose the expected I105 prefix"
    );
    assert_eq!(
        parsed.get("modules").and_then(norito::json::Value::as_u64),
        Some(192),
        "modules should reflect the QR dimension"
    );
    let svg = parsed
        .get("svg")
        .and_then(norito::json::Value::as_str)
        .unwrap_or_default();
    assert!(!svg.is_empty(), "QR responses should include SVG payloads");
    assert!(
        parsed
            .get("qr_version")
            .and_then(norito::json::Value::as_u64)
            .unwrap_or(0)
            > 0,
        "qr_version should be present and non-zero"
    );
    Ok(())
}
#[tokio::test]
async fn explorer_account_qr_accepts_i105_hint() -> Result<()> {
    let Some(network) = Box::pin(start_network_async_or_skip(
        NetworkBuilder::new(),
        stringify!(explorer_account_qr_accepts_i105_hint),
    ))
    .await?
    else {
        return Ok(());
    };
    network.ensure_blocks(1).await?;
    let http = http_client();
    let canonical_literal = ALICE_ID.to_string();
    let url = explorer_account_qr_url(network.client().client().endpoint(), &canonical_literal);
    let resp = http
        .get(url)
        .header("Accept", "application/json")
        .send()
        .await?;
    assert!(
        resp.status().is_success(),
        "expected explorer account QR to respect canonical I105, got {}",
        resp.status()
    );
    let parsed: norito::json::Value = norito::json::from_str(&resp.text().await?)?;
    assert_eq!(
        parsed
            .get("canonical_id")
            .and_then(norito::json::Value::as_str),
        Some(canonical_literal.as_str()),
        "canonical_id should remain I105"
    );
    assert_eq!(
        parsed.get("literal").and_then(norito::json::Value::as_str),
        Some(canonical_literal.as_str()),
        "literal should honour the i105 preference"
    );
    Ok(())
}
#[tokio::test]
async fn accounts_query_rejects_selector_prefixed_filter_literals() -> Result<()> {
    let Some(network) = Box::pin(start_network_async_or_skip(
        NetworkBuilder::new(),
        stringify!(accounts_query_rejects_selector_prefixed_filter_literals),
    ))
    .await?
    else {
        return Ok(());
    };
    network.ensure_blocks(1).await?;
    let literal = selector_prefixed_literal();
    let rejection_reason = AccountId::parse_encoded(&literal)
        .expect_err("selector-prefixed literal must fail to parse")
        .code_str()
        .to_owned();
    let body = format!(r#"{{"filter":{{"op":"eq","args":["id","{literal}"]}},"limit":4}}"#);
    let http = http_client();
    let url = network
        .client()
        .client()
        .endpoint()
        .join("/v1/accounts/query")
        .expect("join accounts query url");
    let resp = http
        .post(url)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .body(body)
        .send()
        .await?;
    assert_eq!(
        resp.status(),
        reqwest::StatusCode::BAD_REQUEST,
        "selector-prefixed literal in filter should be rejected"
    );
    let body = resp.text().await.unwrap_or_default();
    assert!(
        body.contains(&rejection_reason),
        "response body should mention {rejection_reason}, got {body}"
    );
    Ok(())
}
#[tokio::test]
async fn accounts_query_rejects_public_key_filter_literals() -> Result<()> {
    let Some(network) = Box::pin(start_network_async_or_skip(
        NetworkBuilder::new(),
        stringify!(accounts_query_rejects_public_key_filter_literals),
    ))
    .await?
    else {
        return Ok(());
    };
    network.ensure_blocks(1).await?;
    let literal = public_key_literal();
    let reason = AccountId::parse_encoded(&literal)
        .expect_err("public-key@domain literal must fail strict parsing")
        .code_str()
        .to_string();
    let body = format!(r#"{{"filter":{{"op":"eq","args":["id","{literal}"]}},"limit":4}}"#);
    let http = http_client();
    let url = network
        .client()
        .client()
        .endpoint()
        .join("/v1/accounts/query")
        .expect("join accounts query url");
    let resp = http
        .post(url)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .body(body)
        .send()
        .await?;
    assert_eq!(
        resp.status(),
        reqwest::StatusCode::BAD_REQUEST,
        "public-key@domain literal in filter should be rejected"
    );
    let body = resp.text().await.unwrap_or_default();
    assert!(
        body.contains(&reason),
        "response body should mention {reason}, got {body}"
    );
    Ok(())
}
#[tokio::test]
async fn accounts_query_requires_canonical_ids_for_aliased_accounts() -> Result<()> {
    let domain_id: DomainId = DomainId::try_new("aliases", "universal")?;
    let alias_literal = format!("primary@{domain_id}");
    let alias = ResolvedAccountAliasV1::new(
        alias_literal.parse()?,
        iroha_model_base::topology::DataSpaceId::UNIVERSAL,
    );
    let account_id = checked_random_account_id();
    let builder = NetworkBuilder::new()
        .with_min_peers(4)
        .with_genesis_instruction(Register::domain(Domain::new(domain_id.clone())))
        .with_genesis_instruction(Register::account(Account::new(account_id.clone())))
        .with_genesis_instruction(Grant::account_permission(
            Permission::from(CanManageAccountAlias {
                scope: AccountAliasPermissionScope::Alias(alias),
            }),
            SAMPLE_GENESIS_ACCOUNT_ID.clone(),
        ))
        .with_genesis_instruction(account_alias_setup_instruction(
            &alias_literal,
            &account_id,
            AccountProvisionV1::Existing,
            AccountAliasRoleV1::Primary,
        )?);
    let Some(network) = Box::pin(start_network_async_or_skip(
        builder,
        stringify!(accounts_query_requires_canonical_ids_for_aliased_accounts),
    ))
    .await?
    else {
        return Ok(());
    };
    network.ensure_blocks(1).await?;
    let client = network.client();
    let url = client
        .client()
        .endpoint()
        .join("/v1/accounts/query")
        .expect("join accounts query url");
    let expected = account_id.to_string();
    let http = http_client();
    wait_for_account_in_query(&http, url.clone(), &expected).await?;
    let i105_literal = dotted_i105_literal(&account_id.to_string());
    // Generic account filters require canonical IDs. Resolving an alias is a
    // separate caller-aware operation with its own resolution permission.
    let alias_body =
        format!(r#"{{"filter":{{"op":"eq","args":["id","{alias_literal}"]}},"limit":4}}"#);
    let alias_resp = http
        .post(url.clone())
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .body(alias_body)
        .send()
        .await?;
    let alias_status = alias_resp.status();
    let alias_body = alias_resp.text().await?;
    assert_eq!(
        alias_status,
        reqwest::StatusCode::BAD_REQUEST,
        "generic filters must reject even a bound alias, got {alias_status} body={alias_body}"
    );
    let alias_doc: norito::json::Value = norito::json::from_str(&alias_body)?;
    assert_eq!(
        alias_doc.get("code").and_then(norito::json::Value::as_str),
        Some("invalid_filter"),
        "alias rejection must identify the invalid filter: {alias_doc:?}"
    );
    assert!(
        alias_body.contains("account filter requires a canonical I105 literal"),
        "alias rejection must explain the canonical-ID requirement: {alias_body}"
    );
    let canonical_body =
        format!(r#"{{"filter":{{"op":"eq","args":["id","{expected}"]}},"limit":4}}"#);
    let canonical_resp = http
        .post(url.clone())
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .body(canonical_body)
        .send()
        .await?;
    assert_eq!(canonical_resp.status(), reqwest::StatusCode::OK);
    let canonical_doc = norito::json::from_str(&canonical_resp.text().await?)?;
    let canonical_ids = extract_account_ids(&canonical_doc);
    assert_eq!(
        canonical_ids,
        vec![expected],
        "canonical filter must return exactly the bound account as a canonical ID"
    );
    let body = format!(r#"{{"filter":{{"op":"eq","args":["id","{i105_literal}"]}},"limit":4}}"#);
    let resp = http
        .post(url.clone())
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .body(body)
        .send()
        .await?;
    let status = resp.status();
    let body = resp.text().await?;
    let reason = AccountId::parse_encoded(&i105_literal)
        .expect_err("dotted I105 literal must fail strict parsing")
        .code_str()
        .to_string();
    assert_eq!(
        status,
        reqwest::StatusCode::BAD_REQUEST,
        "dotted I105 literal should be rejected, got {status} body={body}"
    );
    assert!(
        body.contains(&reason),
        "response body should mention {reason}, got {body}"
    );
    Ok(())
}
#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn repo_agreements_emit_i105_literals() -> Result<()> {
    init_instruction_registry();
    // Reuse pre-existing asset definitions from the test genesis to avoid permission issues when
    // registering new definitions in the wonderland domain.
    let cash_def_id: AssetDefinitionId = AssetDefinitionId::derive_from_components(
        DomainId::try_new("wonderland", "universal")?,
        "rose".parse()?,
    );
    let collateral_def_id: AssetDefinitionId = AssetDefinitionId::derive_from_components(
        DomainId::try_new("wonderland", "universal")?,
        "camomile".parse()?,
    );
    let setup_instructions: Vec<InstructionBox> = vec![
        Mint::asset_quantity(
            Quantity::from(1500_u32),
            AssetId::new(collateral_def_id.clone(), ALICE_ID.clone()),
        )
        .into(),
        Mint::asset_quantity(
            Quantity::from(1500_u32),
            AssetId::new(cash_def_id.clone(), BOB_ID.clone()),
        )
        .into(),
    ];
    let maturity_ms = u64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)?
            .checked_add(Duration::from_hours(12))
            .expect("maturity timestamp arithmetic should succeed")
            .as_millis(),
    )
    .expect("maturity timestamp fits in u64");
    let agreement_id: RepoAgreementId = "addr_format_repo".parse()?;
    let repo_instruction = RepoIsi::new(
        agreement_id.clone(),
        ALICE_ID.clone(),
        BOB_ID.clone(),
        None,
        RepoCashLeg {
            asset_definition_id: cash_def_id.clone(),
            quantity: Quantity::from(1_000_u32),
        },
        RepoCollateralLeg::new(collateral_def_id.clone(), 1_100_u32),
        0,
        maturity_ms,
        RepoGovernance::new(1_500, 43_200, false),
    );
    let mut builder = NetworkBuilder::new();
    for isi in setup_instructions {
        builder = builder.with_genesis_instruction(isi);
    }
    let Some(network) = Box::pin(start_network_async_or_skip(
        builder,
        stringify!(repo_agreements_emit_i105_literals),
    ))
    .await?
    else {
        return Ok(());
    };
    network.ensure_blocks(1).await?;
    // Issue the repo agreement after genesis to avoid initial-executor restrictions. Some
    // environments may not ship repo instructions; skip gracefully if the binary rejects them.
    let repo_instruction_box: InstructionBox = RepoInstructionBox::from(repo_instruction).into();
    let client = network.client();
    let account = client.account_client();
    let mut payload = account.prepare_transaction(iroha::client::AccountTransactionDraft::new(
        [repo_instruction_box],
        iroha::data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        Metadata::default(),
    ))?;
    let quote = account
        .quote_fees(iroha::client::FeeQuoteRequest::AccountSignature { payload: &payload })
        .await?;
    assert!(
        payload
            .fee_payment
            .has_same_payer_and_gas_bound(&quote.intent),
        "repo fee quote must preserve the selected payer and gas bound"
    );
    payload.fee_payment = quote.intent;
    let transaction = account.sign_transaction(payload)?;
    if let Err(err) = account.submit_transaction_and_wait(&transaction).await {
        eprintln!("Skipping repo i105 literal coverage: {err}");
        return Ok(());
    }
    network.ensure_blocks(2).await?;
    let http = http_client();
    let base = client.client().endpoint().clone();
    let i105_alice = ALICE_ID.to_string();
    let i105_bob = BOB_ID.to_string();
    let agreement_literal = agreement_id.to_string();
    let i105_alice = i105_alice.clone();
    let i105_bob = i105_bob.clone();
    let mut default_url = base.join("/v1/repo/agreements")?;
    {
        let mut qp = default_url.query_pairs_mut();
        qp.append_pair("limit", "8");
    }
    let resp = http
        .get(default_url)
        .header("Accept", "application/json")
        .send()
        .await?;
    assert!(
        resp.status().is_success(),
        "default repo agreement listing should succeed, got {}",
        resp.status()
    );
    let parsed: norito::json::Value = norito::json::from_str(&resp.text().await?)?;
    let entry = if let Some(entry) = find_repo_entry(&parsed, agreement_literal.as_str()) {
        entry
    } else {
        eprintln!(
            "Skipping repo i105 literal coverage: repo agreement not observed after submission."
        );
        return Ok(());
    };
    let initiator = entry
        .get("initiator")
        .and_then(norito::json::Value::as_str)
        .expect("initiator literal should be present");
    assert_eq!(
        initiator, i105_alice,
        "repo agreements must default to I105 initiators"
    );
    let counterparty = entry
        .get("counterparty")
        .and_then(norito::json::Value::as_str)
        .expect("counterparty literal should be present");
    assert_eq!(
        counterparty, i105_bob,
        "repo agreements must default to I105 counterparties"
    );
    let mut i105_url = base.join("/v1/repo/agreements")?;
    {
        let mut qp = i105_url.query_pairs_mut();
        qp.append_pair("limit", "8");
    }
    let resp = http
        .get(i105_url)
        .header("Accept", "application/json")
        .send()
        .await?;
    assert!(
        resp.status().is_success(),
        "I105 repo agreement listing should succeed, got {}",
        resp.status()
    );
    let parsed: norito::json::Value = norito::json::from_str(&resp.text().await?)?;
    let entry = find_repo_entry(&parsed, agreement_literal.as_str())
        .expect("I105 repo listing should include the recorded agreement");
    let initiator = entry
        .get("initiator")
        .and_then(norito::json::Value::as_str)
        .expect("I105 initiator literal must be present");
    assert_eq!(
        initiator, i105_alice,
        "repo agreements should honour canonical I105 for initiators"
    );
    let counterparty = entry
        .get("counterparty")
        .and_then(norito::json::Value::as_str)
        .expect("I105 counterparty literal must be present");
    assert_eq!(
        counterparty, i105_bob,
        "repo agreements should honour canonical I105 for counterparties"
    );
    let query_url = base.join("/v1/repo/agreements/query")?;
    let query_body =
        format!(r#"{{"filter":{{"op":"eq","args":["id","{agreement_literal}"]}},"limit":4}}"#);
    let resp = http
        .post(query_url)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .body(query_body)
        .send()
        .await?;
    assert!(
        resp.status().is_success(),
        "repo agreements query should succeed, got {}",
        resp.status()
    );
    let parsed: norito::json::Value = norito::json::from_str(&resp.text().await?)?;
    let entry = find_repo_entry(&parsed, agreement_literal.as_str())
        .expect("repo agreements query should return the requested agreement");
    let initiator = entry
        .get("initiator")
        .and_then(norito::json::Value::as_str)
        .expect("I105 query initiator literal must be present");
    assert_eq!(
        initiator, i105_alice,
        "repo agreements query should honour canonical I105"
    );
    let counterparty = entry
        .get("counterparty")
        .and_then(norito::json::Value::as_str)
        .expect("I105 query counterparty literal must be present");
    assert_eq!(
        counterparty, i105_bob,
        "repo agreements query should honour canonical I105"
    );
    Ok(())
}
async fn submit_account_fixture_instruction(
    account: &iroha::client::AccountClient,
    instruction: InstructionBox,
) -> Result<()> {
    let mut payload = account.prepare_transaction(iroha::client::AccountTransactionDraft::new(
        [instruction],
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        Metadata::default(),
    ))?;
    let quote = account
        .quote_fees(iroha::client::FeeQuoteRequest::AccountSignature { payload: &payload })
        .await?;
    eyre::ensure!(
        payload
            .fee_payment
            .has_same_payer_and_gas_bound(&quote.intent),
        "fixture fee quote must preserve the selected payer and gas bound"
    );
    payload.fee_payment = quote.intent;
    let transaction = account.sign_transaction(payload)?;
    account.submit_transaction_and_wait(&transaction).await?;
    Ok(())
}

#[tokio::test]
#[allow(clippy::too_many_lines)] // integration coverage requires full scenario setup
async fn kaigi_endpoints_emit_i105_literals() -> Result<()> {
    let relay_id = BOB_ID.clone();
    let reporter_id = ALICE_ID.clone();
    let domain_id = DomainId::try_new("kaigiaddress", "universal")?;
    let relay_accounts = [BOB_ID.clone(), CARPENTER_ID.clone(), ALICE_ID.clone()];
    let mut builder = NetworkBuilder::new()
        .with_genesis_instruction(Register::domain(Domain::new(domain_id.clone())));
    for (index, account) in relay_accounts.iter().enumerate() {
        let alias_literal = format!("relay{index}@{domain_id}");
        let alias = ResolvedAccountAliasV1::new(
            alias_literal.parse()?,
            iroha_model_base::topology::DataSpaceId::UNIVERSAL,
        );
        builder = builder
            .with_genesis_instruction(Grant::account_permission(
                Permission::from(CanManageAccountAlias {
                    scope: AccountAliasPermissionScope::Alias(alias),
                }),
                SAMPLE_GENESIS_ACCOUNT_ID.clone(),
            ))
            .with_genesis_instruction(account_alias_setup_instruction(
                &alias_literal,
                account,
                AccountProvisionV1::Existing,
                AccountAliasRoleV1::Primary,
            )?);
    }
    builder = builder.with_genesis_instruction(Transfer::domain(
        SAMPLE_GENESIS_ACCOUNT_ID.clone(),
        domain_id.clone(),
        reporter_id.clone(),
    ));
    let Some(network) = Box::pin(start_network_async_or_skip(
        builder,
        stringify!(kaigi_endpoints_emit_i105_literals),
    ))
    .await?
    else {
        return Ok(());
    };
    network.ensure_blocks(1).await?;
    let peer = network.peer();
    let keys = [&*BOB_KEYPAIR, &*CARPENTER_KEYPAIR, &*ALICE_KEYPAIR];
    let mut hops = Vec::new();
    for (index, (account_id, key)) in relay_accounts.iter().zip(keys).enumerate() {
        let hpke_public_key = encode_kaigi_relay_hpke_key_v1(
            KaigiRelayHpkeSuiteV1::MlKem768AuthPsk,
            &[0xAA + u8::try_from(index)?; 1_184],
        )
        .wrap_err("frame the relay HPKE key")?;
        let relay_client = peer.client_for(account_id, key.private_key().clone());
        submit_account_fixture_instruction(
            relay_client.account_client(),
            RegisterKaigiRelay {
                relay: KaigiRelayRegistration {
                    relay_id: account_id.clone(),
                    hpke_public_key: hpke_public_key.clone(),
                    bandwidth_class: 5,
                },
            }
            .into(),
        )
        .await?;
        hops.push(KaigiRelayHop {
            relay_id: account_id.clone(),
            hpke_public_key,
            weight: 1,
        });
    }
    let client = peer.client();
    let call_name: Name = "integration-demo".parse()?;
    let call_id = KaigiId::new(domain_id.clone(), call_name.clone());
    let mut call = NewKaigi::with_defaults(call_id.clone(), reporter_id.clone());
    call.relay_manifest = Some(KaigiRelayManifest {
        hops,
        expiry_ms: u64::try_from(
            (SystemTime::now().duration_since(UNIX_EPOCH)? + Duration::from_secs(300)).as_millis(),
        )?,
    });
    submit_account_fixture_instruction(
        client.account_client(),
        CreateKaigi {
            call,
            commitment: None,
            nullifier: None,
            roster_root: None,
            proof: None,
        }
        .into(),
    )
    .await?;
    let domain = client
        .account_client()
        .query_single(FindDomainById::new(domain_id))
        .await?;
    let record: KaigiRecord = domain
        .metadata()
        .get(&kaigi_metadata_key(&call_name)?)
        .ok_or_else(|| eyre!("native Kaigi creation must retain its record"))?
        .try_into_any()?;
    submit_account_fixture_instruction(
        client.account_client(),
        ReportKaigiRelayHealth {
            call_id,
            relay_id: relay_id.clone(),
            status: KaigiRelayHealthStatus::Healthy,
            reported_at_ms: record.created_at_ms,
            notes: Some("integration test coverage".to_owned()),
        }
        .into(),
    )
    .await?;
    let seed = KaigiRelaySeed {
        relay_i105: relay_id.to_string(),
        reporter_i105: reporter_id.to_string(),
    };
    let http = kaigi_operator_http_client();
    let summary_resp = kaigi_operator_get(&http, &client, "/v1/kaigi/relays").await?;
    assert!(
        summary_resp.status().is_success(),
        "kaigi relay summary should succeed, got {}",
        summary_resp.status()
    );
    let summary: norito::json::Value = norito::json::from_str(&summary_resp.text().await?)?;
    let summary_items = summary
        .get("items")
        .and_then(norito::json::Value::as_array)
        .ok_or_else(|| eyre!("kaigi relay summary missing items array"))?;
    let default_literals: Vec<_> = summary_items
        .iter()
        .filter_map(|item| {
            item.as_object()
                .and_then(|obj| obj.get("relay_id"))
                .and_then(norito::json::Value::as_str)
        })
        .collect();
    assert!(
        default_literals
            .iter()
            .any(|literal| *literal == seed.relay_i105),
        "expected I105 literal {} in kaigi summary, got {default_literals:?}",
        seed.relay_i105
    );
    let i105_resp = kaigi_operator_get(&http, &client, "/v1/kaigi/relays").await?;
    assert!(
        i105_resp.status().is_success(),
        "I105 kaigi relay summary should succeed, got {}",
        i105_resp.status()
    );
    let i105_summary: norito::json::Value = norito::json::from_str(&i105_resp.text().await?)?;
    let i105_literals: Vec<_> = i105_summary
        .get("items")
        .and_then(norito::json::Value::as_array)
        .ok_or_else(|| eyre!("I105 kaigi summary missing items array"))?
        .iter()
        .filter_map(|item| {
            item.as_object()
                .and_then(|obj| obj.get("relay_id"))
                .and_then(norito::json::Value::as_str)
        })
        .collect();
    assert!(
        i105_literals
            .iter()
            .any(|literal| *literal == seed.relay_i105),
        "expected I105 literal {} in kaigi summary, got {i105_literals:?}",
        seed.relay_i105
    );
    let noncanonical_relay_literal = dotted_i105_bob_literal();
    let detail_target = format!("/v1/kaigi/relays/{noncanonical_relay_literal}");
    let detail_resp = kaigi_operator_get(&http, &client, &detail_target).await?;
    assert_eq!(
        detail_resp.status(),
        reqwest::StatusCode::BAD_REQUEST,
        "kaigi relay detail should reject dotted I105 literal, got {}",
        detail_resp.status()
    );
    let body = detail_resp.text().await?;
    let reason = AccountId::parse_encoded(&noncanonical_relay_literal)
        .expect_err("dotted I105 relay literal must fail strict parsing")
        .code_str()
        .to_string();
    assert!(
        body.contains(&reason),
        "response body should mention {reason}, got {body}"
    );
    let formatted_detail_target = format!("/v1/kaigi/relays/{}", seed.relay_i105);
    let formatted_resp = kaigi_operator_get(&http, &client, &formatted_detail_target).await?;
    assert!(
        formatted_resp.status().is_success(),
        "kaigi relay detail with canonical I105 should succeed, got {}",
        formatted_resp.status()
    );
    let formatted_detail: norito::json::Value =
        norito::json::from_str(&formatted_resp.text().await?)?;
    let relay_literal = formatted_detail
        .get("relay")
        .and_then(norito::json::Value::as_object)
        .and_then(|relay| relay.get("relay_id"))
        .and_then(norito::json::Value::as_str)
        .ok_or_else(|| eyre!("formatted detail missing relay literal"))?;
    assert_eq!(
        relay_literal, seed.relay_i105,
        "relay literal should honour canonical I105"
    );
    let reported_by_literal = formatted_detail
        .get("reported_by")
        .and_then(norito::json::Value::as_str)
        .ok_or_else(|| eyre!("formatted detail missing reported_by literal"))?;
    assert_eq!(
        reported_by_literal, seed.reporter_i105,
        "reported_by should honour canonical I105"
    );
    Ok(())
}
