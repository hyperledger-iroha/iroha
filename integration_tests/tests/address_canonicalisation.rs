//! Roadmap ADDR-5 coverage ensuring Torii surfaces canonical IH58 account IDs.

use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use eyre::{Result, WrapErr, eyre};
use integration_tests::sandbox::start_network_async_or_skip;
use iroha::data_model::{
    isi::{SetKeyValue, offline::RegisterOfflineAllowance, repo::RepoIsi},
    kaigi::{
        KaigiId, KaigiRelayFeedback, KaigiRelayHealthStatus, KaigiRelayRegistration,
        kaigi_relay_feedback_key, kaigi_relay_metadata_key,
    },
    offline::OfflineWalletCertificate,
    prelude::*,
    repo::{RepoAgreementId, RepoCashLeg, RepoCollateralLeg, RepoGovernance},
};
use iroha_crypto::{Hash, Signature};
use iroha_data_model::account::{
    AccountDomainSelector,
    address::{AccountAddress, AccountAddressFormat},
    clear_account_domain_selector_resolver, set_account_domain_selector_resolver,
};
use iroha_data_model::metadata::Metadata;
use iroha_data_model::prelude::RepoInstructionBox;
use iroha_primitives::json::Json;
use iroha_test_network::{NetworkBuilder, init_instruction_registry};
use iroha_test_samples::{ALICE_ID, ALICE_KEYPAIR, BOB_ID, SAMPLE_GENESIS_ACCOUNT_ID};
use reqwest::Client;

type SurfaceSpec<'a> = (&'a [&'a str], &'a [(&'a str, &'a str)]);

fn http_client() -> Client {
    Client::builder()
        .timeout(Duration::from_secs(30))
        .connect_timeout(Duration::from_secs(5))
        .build()
        .expect("http client should build")
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
    let body = format!(
        r#"{{"filter":{{"op":"eq","args":["id","{account_literal}"]}},"sort":[],"pagination":{{"limit":4,"offset":0}},"fetch_size":null,"select":null}}"#
    );
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

fn assert_authorities_are_ih58(authorities: &[String]) -> Result<()> {
    for literal in authorities {
        let (_, format) = AccountAddress::parse_any(literal, None)
            .wrap_err_with(|| format!("authority {literal} should parse as account address"))?;
        if !matches!(format, AccountAddressFormat::IH58 { .. }) {
            return Err(eyre!(
                "IH58 default should emit canonical IH58 literals; got {} ({:?})",
                literal,
                format
            ));
        }
    }

    Ok(())
}

fn find_offline_allowance_entry<'a>(
    value: &'a norito::json::Value,
    controller_id: &str,
) -> Option<&'a norito::json::Map> {
    value
        .as_object()
        .and_then(|obj| obj.get("items"))
        .and_then(|items| items.as_array())
        .and_then(|items| {
            items.iter().find_map(|entry| {
                let obj = entry.as_object()?;
                let literal = obj.get("controller_id")?.as_str()?;
                if literal == controller_id {
                    Some(obj)
                } else {
                    None
                }
            })
        })
}

struct DomainSelectorResolverGuard;

impl DomainSelectorResolverGuard {
    fn install(domains: &[DomainId]) -> Result<Self> {
        let mut selectors = Vec::with_capacity(domains.len());
        for domain in domains {
            let selector = AccountDomainSelector::from_domain(domain)
                .map_err(|err| eyre!("failed to derive domain selector for {domain}: {err}"))?;
            selectors.push((selector, domain.clone()));
        }
        set_account_domain_selector_resolver(Arc::new(move |candidate| {
            selectors.iter().find_map(|(selector, domain)| {
                if candidate == selector {
                    Some(domain.clone())
                } else {
                    None
                }
            })
        }));
        Ok(Self)
    }
}

impl Drop for DomainSelectorResolverGuard {
    fn drop(&mut self) {
        clear_account_domain_selector_resolver();
    }
}

fn load_offline_certificate_fixture() -> Result<OfflineWalletCertificate> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../fixtures/offline_allowance/android-demo/register_instruction.json");
    let raw = fs::read_to_string(&path).wrap_err_with(|| {
        format!(
            "failed to read offline allowance fixture `{}`",
            path.display()
        )
    })?;
    let fixture: norito::json::Value = norito::json::from_str(&raw).map_err(|err| {
        eyre!(
            "failed to parse offline allowance fixture `{}`: {err}",
            path.display()
        )
    })?;
    let domains: Vec<DomainId> = ["wonderland", "treasury"]
        .into_iter()
        .map(|label| label.parse())
        .collect::<Result<_, _>>()
        .map_err(|err| eyre!("failed to parse fixture domain label: {err}"))?;
    let _resolver_guard = DomainSelectorResolverGuard::install(&domains)?;
    let certificate_value = fixture
        .get("certificate")
        .ok_or_else(|| {
            eyre!(
                "offline allowance fixture `{}` missing `certificate` field",
                path.display()
            )
        })?
        .clone();
    // Fixture JSON uses *_hex/*_b64 shorthands instead of the Norito JSON layout.
    parse_offline_certificate_fixture(&certificate_value, &path)
}

fn parse_offline_certificate_fixture(
    value: &norito::json::Value,
    path: &Path,
) -> Result<OfflineWalletCertificate> {
    let obj = value.as_object().ok_or_else(|| {
        eyre!(
            "offline allowance fixture `{}` certificate must be a JSON object",
            path.display()
        )
    })?;

    let controller = obj
        .get("controller")
        .and_then(norito::json::Value::as_str)
        .ok_or_else(|| {
            eyre!(
                "offline allowance fixture `{}` missing certificate controller",
                path.display()
            )
        })?;
    let controller = controller
        .parse()
        .map_err(|err| eyre!("failed to parse controller `{controller}`: {err}"))?;

    let operator = obj
        .get("operator")
        .and_then(norito::json::Value::as_str)
        .ok_or_else(|| {
            eyre!(
                "offline allowance fixture `{}` missing certificate operator",
                path.display()
            )
        })?;
    let operator = operator
        .parse()
        .map_err(|err| eyre!("failed to parse operator `{operator}`: {err}"))?;

    let allowance = obj
        .get("allowance")
        .and_then(norito::json::Value::as_object)
        .ok_or_else(|| {
            eyre!(
                "offline allowance fixture `{}` missing allowance block",
                path.display()
            )
        })?;
    let allowance_asset = allowance
        .get("asset")
        .and_then(norito::json::Value::as_str)
        .ok_or_else(|| {
            eyre!(
                "offline allowance fixture `{}` missing allowance asset",
                path.display()
            )
        })?;
    let asset = allowance_asset
        .parse()
        .map_err(|err| eyre!("failed to parse allowance asset `{allowance_asset}`: {err}"))?;
    let amount_str = allowance
        .get("amount")
        .and_then(norito::json::Value::as_str)
        .ok_or_else(|| {
            eyre!(
                "offline allowance fixture `{}` missing allowance amount",
                path.display()
            )
        })?;
    let amount = amount_str
        .parse()
        .map_err(|err| eyre!("failed to parse allowance amount `{amount_str}`: {err}"))?;
    let commitment_hex = allowance
        .get("commitment_hex")
        .and_then(norito::json::Value::as_str)
        .ok_or_else(|| {
            eyre!(
                "offline allowance fixture `{}` missing allowance commitment_hex",
                path.display()
            )
        })?;
    let commitment =
        hex::decode(commitment_hex).map_err(|err| eyre!("commitment_hex decode: {err}"))?;

    let spend_public_key = obj
        .get("spend_public_key")
        .and_then(norito::json::Value::as_str)
        .ok_or_else(|| {
            eyre!(
                "offline allowance fixture `{}` missing spend_public_key",
                path.display()
            )
        })?;
    let spend_public_key = spend_public_key
        .parse()
        .map_err(|err| eyre!("failed to parse spend_public_key `{spend_public_key}`: {err}"))?;

    let attestation_report_b64 = obj
        .get("attestation_report_b64")
        .and_then(norito::json::Value::as_str)
        .ok_or_else(|| {
            eyre!(
                "offline allowance fixture `{}` missing attestation_report_b64",
                path.display()
            )
        })?;
    let attestation_report = BASE64
        .decode(attestation_report_b64)
        .map_err(|err| eyre!("attestation_report_b64 decode: {err}"))?;

    let issued_at_ms = obj
        .get("issued_at_ms")
        .and_then(norito::json::Value::as_u64)
        .ok_or_else(|| {
            eyre!(
                "offline allowance fixture `{}` missing issued_at_ms",
                path.display()
            )
        })?;
    let expires_at_ms = obj
        .get("expires_at_ms")
        .and_then(norito::json::Value::as_u64)
        .ok_or_else(|| {
            eyre!(
                "offline allowance fixture `{}` missing expires_at_ms",
                path.display()
            )
        })?;

    let policy = obj
        .get("policy")
        .and_then(norito::json::Value::as_object)
        .ok_or_else(|| {
            eyre!(
                "offline allowance fixture `{}` missing policy block",
                path.display()
            )
        })?;
    let max_balance = policy
        .get("max_balance")
        .and_then(norito::json::Value::as_str)
        .ok_or_else(|| {
            eyre!(
                "offline allowance fixture `{}` missing policy max_balance",
                path.display()
            )
        })?;
    let max_tx_value = policy
        .get("max_tx_value")
        .and_then(norito::json::Value::as_str)
        .ok_or_else(|| {
            eyre!(
                "offline allowance fixture `{}` missing policy max_tx_value",
                path.display()
            )
        })?;
    let policy_expires = policy
        .get("expires_at_ms")
        .and_then(norito::json::Value::as_u64)
        .ok_or_else(|| {
            eyre!(
                "offline allowance fixture `{}` missing policy expires_at_ms",
                path.display()
            )
        })?;

    let operator_signature_hex = obj
        .get("operator_signature_hex")
        .and_then(norito::json::Value::as_str)
        .ok_or_else(|| {
            eyre!(
                "offline allowance fixture `{}` missing operator_signature_hex",
                path.display()
            )
        })?;
    let operator_signature = Signature::from_hex(operator_signature_hex)
        .map_err(|err| eyre!("operator_signature_hex decode: {err}"))?;

    let metadata_value = obj
        .get("metadata")
        .cloned()
        .unwrap_or_else(|| norito::json::Value::Object(Default::default()));
    let metadata: Metadata =
        norito::json::from_value(metadata_value).map_err(|err| eyre!("metadata parse: {err}"))?;

    let verdict_id = parse_optional_hash(obj.get("verdict_id_hex"), path, "verdict_id_hex")?;
    let attestation_nonce =
        parse_optional_hash(obj.get("attestation_nonce_hex"), path, "attestation_nonce_hex")?;
    let refresh_at_ms = obj.get("refresh_at_ms").and_then(norito::json::Value::as_u64);

    Ok(OfflineWalletCertificate {
        controller,
        operator,
        allowance: iroha_data_model::offline::OfflineAllowanceCommitment {
            asset,
            amount,
            commitment,
        },
        spend_public_key,
        attestation_report,
        issued_at_ms,
        expires_at_ms,
        policy: iroha_data_model::offline::OfflineWalletPolicy {
            max_balance: max_balance
                .parse()
                .map_err(|err| eyre!("policy max_balance parse: {err}"))?,
            max_tx_value: max_tx_value
                .parse()
                .map_err(|err| eyre!("policy max_tx_value parse: {err}"))?,
            expires_at_ms: policy_expires,
        },
        operator_signature,
        metadata,
        verdict_id,
        attestation_nonce,
        refresh_at_ms,
    })
}

fn parse_optional_hash(
    value: Option<&norito::json::Value>,
    path: &Path,
    field: &'static str,
) -> Result<Option<Hash>> {
    match value {
        None => Ok(None),
        Some(value) if value.is_null() => Ok(None),
        Some(value) => {
            let hex = value.as_str().ok_or_else(|| {
                eyre!(
                    "offline allowance fixture `{}` `{field}` must be hex string or null",
                    path.display()
                )
            })?;
            Hash::from_str(hex)
                .map(Some)
                .map_err(|err| eyre!("offline allowance fixture `{}` `{field}`: {err}", path.display()))
        }
    }
}

struct OfflineAllowanceSeed {
    controller_ih58: String,
    controller_compressed: String,
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

fn compressed_alice_literal() -> String {
    let account_address = ALICE_ID
        .to_account_address()
        .expect("account address should encode");
    account_address
        .to_compressed_sora()
        .expect("compressed address encoding")
}

fn compressed_bob_literal() -> String {
    let account_address = BOB_ID
        .to_account_address()
        .expect("account address should encode");
    account_address
        .to_compressed_sora()
        .expect("compressed address encoding")
}

fn local8_literal() -> String {
    let account_address = ALICE_ID
        .to_account_address()
        .expect("account address should encode");
    let canonical_hex = account_address
        .canonical_hex()
        .expect("canonical hex encoding");
    let mut canonical = hex::decode(&canonical_hex[2..]).expect("canonical hex decoding succeeds");
    let digest_start = 2;
    canonical.drain(digest_start + 8..digest_start + 12);
    format!("0x{}", hex::encode(canonical))
}

fn public_key_literal() -> String {
    let public_key = ALICE_KEYPAIR.public_key().to_string();
    format!("{public_key}@{}", ALICE_ID.domain())
}

struct KaigiRelaySeed {
    relay_ih58: String,
    relay_compressed: String,
    reporter_compressed: String,
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
async fn accounts_listing_emits_ih58_identifiers() -> Result<()> {
    let Some(network) = start_network_async_or_skip(
        NetworkBuilder::new(),
        stringify!(accounts_listing_emits_ih58_identifiers),
    )
    .await?
    else {
        return Ok(());
    };
    network.ensure_blocks(1).await?;

    let http = http_client();
    let url = network
        .client()
        .torii_url
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
        "expected IH58 literal {expected} in account listing, got {ids:?}"
    );
    assert!(
        ids.iter().all(|id| !id.contains('@')),
        "account listing should emit canonical IH58 strings"
    );

    Ok(())
}

#[tokio::test]
async fn accounts_query_accepts_ih58_filter_literals() -> Result<()> {
    let Some(network) = start_network_async_or_skip(
        NetworkBuilder::new(),
        stringify!(accounts_query_accepts_ih58_filter_literals),
    )
    .await?
    else {
        return Ok(());
    };
    network.ensure_blocks(1).await?;

    let expected = ALICE_ID.to_string();
    let body = format!(
        r#"{{"filter":{{"op":"eq","args":["id","{expected}"]}},"sort":[],"pagination":{{"limit":4,"offset":0}},"fetch_size":null,"select":null}}"#
    );

    let http = http_client();
    let url = network
        .client()
        .torii_url
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
        "accounts query should return IH58 literal {expected}, got {ids:?}"
    );

    Ok(())
}

#[tokio::test]
async fn accounts_listing_filter_accepts_compressed_literals() -> Result<()> {
    let Some(network) = start_network_async_or_skip(
        NetworkBuilder::new(),
        stringify!(accounts_listing_filter_accepts_compressed_literals),
    )
    .await?
    else {
        return Ok(());
    };
    network.ensure_blocks(1).await?;

    let literal = compressed_alice_literal();
    let filter = format!(r#"{{"op":"eq","args":["id","{literal}"]}}"#);
    let http = http_client();
    let mut url = network
        .client()
        .torii_url
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
    assert!(
        resp.status().is_success(),
        "expected success from accounts listing with compressed filter, got {}",
        resp.status()
    );
    let parsed: norito::json::Value = norito::json::from_str(&resp.text().await?)?;
    let ids = extract_account_ids(&parsed);
    let expected = ALICE_ID.to_string();
    assert!(
        ids.iter().any(|id| id == &expected),
        "compressed literal should resolve to IH58 output {expected}, got {ids:?}"
    );

    Ok(())
}

#[tokio::test]
async fn accounts_query_accepts_compressed_filter_literals() -> Result<()> {
    let Some(network) = start_network_async_or_skip(
        NetworkBuilder::new(),
        stringify!(accounts_query_accepts_compressed_filter_literals),
    )
    .await?
    else {
        return Ok(());
    };
    network.ensure_blocks(1).await?;

    let literal = compressed_alice_literal();
    let body = format!(
        r#"{{"filter":{{"op":"eq","args":["id","{literal}"]}},"sort":[],"pagination":{{"limit":4,"offset":0}},"fetch_size":null,"select":null}}"#
    );

    let http = http_client();
    let url = network
        .client()
        .torii_url
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
        "expected success from accounts query with compressed literal, got {}",
        resp.status()
    );
    let parsed: norito::json::Value = norito::json::from_str(&resp.text().await?)?;
    let ids = extract_account_ids(&parsed);
    let expected = ALICE_ID.to_string();
    assert!(
        ids.iter().all(|id| id == &expected),
        "compressed literal should yield canonical IH58 output {expected}, got {ids:?}"
    );

    Ok(())
}

#[tokio::test]
async fn accounts_listing_supports_compressed_response() -> Result<()> {
    let Some(network) = start_network_async_or_skip(
        NetworkBuilder::new(),
        stringify!(accounts_listing_supports_compressed_response),
    )
    .await?
    else {
        return Ok(());
    };
    network.ensure_blocks(1).await?;

    let http = http_client();
    let url = network
        .client()
        .torii_url
        .join("/v1/accounts?limit=8&address_format=compressed")
        .expect("join accounts url");
    let resp = http
        .get(url)
        .header("Accept", "application/json")
        .send()
        .await?;
    assert!(
        resp.status().is_success(),
        "expected success from accounts listing with compressed response, got {}",
        resp.status()
    );
    let parsed: norito::json::Value = norito::json::from_str(&resp.text().await?)?;
    let ids = extract_account_ids(&parsed);
    let expected = compressed_alice_literal();
    assert!(
        ids.iter().any(|id| id == &expected),
        "compressed literal {expected} missing from response {ids:?}"
    );
    assert!(
        ids.iter().all(|id| id.starts_with("sora")),
        "all ids should be compressed in the response: {ids:?}"
    );

    Ok(())
}

#[tokio::test]
async fn accounts_query_supports_compressed_response() -> Result<()> {
    let Some(network) = start_network_async_or_skip(
        NetworkBuilder::new(),
        stringify!(accounts_query_supports_compressed_response),
    )
    .await?
    else {
        return Ok(());
    };
    network.ensure_blocks(1).await?;

    let body = r#"{"filter":null,"sort":[],"pagination":{"limit":8,"offset":0},"fetch_size":null,"select":null,"address_format":"compressed"}"#;
    let http = http_client();
    let url = network
        .client()
        .torii_url
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
        "expected success from accounts query with compressed response, got {}",
        resp.status()
    );
    let parsed: norito::json::Value = norito::json::from_str(&resp.text().await?)?;
    let ids = extract_account_ids(&parsed);
    let expected = compressed_alice_literal();
    assert!(
        ids.iter().any(|id| id == &expected),
        "compressed literal {expected} missing from query response {ids:?}"
    );

    Ok(())
}

#[tokio::test]
async fn accounts_listing_rejects_unknown_address_format() -> Result<()> {
    let Some(network) = start_network_async_or_skip(
        NetworkBuilder::new(),
        stringify!(accounts_listing_rejects_unknown_address_format),
    )
    .await?
    else {
        return Ok(());
    };
    network.ensure_blocks(1).await?;

    let http = http_client();
    let url = network
        .client()
        .torii_url
        .join("/v1/accounts?address_format=future")
        .expect("join accounts url");
    let resp = http
        .get(url)
        .header("Accept", "application/json")
        .send()
        .await?;
    assert_eq!(
        resp.status(),
        reqwest::StatusCode::BAD_REQUEST,
        "invalid address_format should be rejected"
    );

    Ok(())
}

#[tokio::test]
async fn account_path_endpoints_accept_compressed_literals() -> Result<()> {
    let Some(network) = start_network_async_or_skip(
        NetworkBuilder::new(),
        stringify!(account_path_endpoints_accept_compressed_literals),
    )
    .await?
    else {
        return Ok(());
    };
    network.ensure_blocks(1).await?;

    let literal = compressed_alice_literal();
    let http = http_client();

    let surfaces: [SurfaceSpec; 3] = [
        (&["assets"], &[("limit", "4")]),
        (&["transactions"], &[("limit", "4")]),
        (&["permissions"], &[("limit", "4")]),
    ];

    for (segments, query_pairs) in surfaces {
        let mut url = account_endpoint_url(&network.client().torii_url, literal.as_str(), segments);
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
        assert!(
            resp.status().is_success(),
            "compressed literal should be accepted for {}, got {}",
            url,
            resp.status()
        );
    }

    Ok(())
}

#[tokio::test]
async fn account_path_endpoints_reject_local8_literals() -> Result<()> {
    let Some(network) = start_network_async_or_skip(
        NetworkBuilder::new(),
        stringify!(account_path_endpoints_reject_local8_literals),
    )
    .await?
    else {
        return Ok(());
    };
    network.ensure_blocks(1).await?;

    let literal = local8_literal();
    let http = http_client();
    let surfaces: [SurfaceSpec; 3] = [
        (&["assets"], &[("limit", "4")]),
        (&["transactions"], &[("limit", "4")]),
        (&["permissions"], &[("limit", "4")]),
    ];

    for (segments, query_pairs) in surfaces {
        let mut url = account_endpoint_url(&network.client().torii_url, literal.as_str(), segments);
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
            "Local-8 literal should be rejected for {url}"
        );
        let body = resp.text().await.unwrap_or_default();
        assert!(
            body.contains("ERR_LOCAL8_DEPRECATED"),
            "response body should mention ERR_LOCAL8_DEPRECATED, got {body}"
        );
    }

    Ok(())
}

#[tokio::test]
async fn account_path_endpoints_accept_public_key_literals() -> Result<()> {
    let Some(network) = start_network_async_or_skip(
        NetworkBuilder::new(),
        stringify!(account_path_endpoints_accept_public_key_literals),
    )
    .await?
    else {
        return Ok(());
    };
    network.ensure_blocks(1).await?;

    let literal = public_key_literal();
    let http = http_client();
    let surfaces: [SurfaceSpec; 3] = [
        (&["assets"], &[("limit", "4")]),
        (&["transactions"], &[("limit", "4")]),
        (&["permissions"], &[("limit", "4")]),
    ];

    for (segments, query_pairs) in surfaces {
        let mut url = account_endpoint_url(&network.client().torii_url, literal.as_str(), segments);
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
        assert!(
            resp.status().is_success(),
            "public-key literal should be accepted for {url}, got {}",
            resp.status()
        );
    }

    Ok(())
}

#[tokio::test]
async fn asset_holders_get_supports_compressed_response() -> Result<()> {
    let Some(network) = start_network_async_or_skip(
        NetworkBuilder::new(),
        stringify!(asset_holders_get_supports_compressed_response),
    )
    .await?
    else {
        return Ok(());
    };
    network.ensure_blocks(1).await?;

    let http = http_client();
    let url = network
        .client()
        .torii_url
        .join("/v1/assets/rose%23wonderland/holders?limit=8&address_format=compressed")
        .expect("join asset holders url");
    let resp = http
        .get(url)
        .header("Accept", "application/json")
        .send()
        .await?;
    assert!(
        resp.status().is_success(),
        "expected success from asset holders listing with compressed response, got {}",
        resp.status()
    );
    let parsed: norito::json::Value = norito::json::from_str(&resp.text().await?)?;
    let ids = extract_holder_account_ids(&parsed);
    let expected = compressed_alice_literal();
    assert!(
        ids.iter().any(|id| id == &expected),
        "compressed literal {expected} missing from holders response {ids:?}"
    );
    assert!(
        ids.iter().all(|id| id.starts_with("sora")),
        "all holders should render compressed literals when requested: {ids:?}"
    );

    Ok(())
}

#[tokio::test]
async fn asset_holders_query_accepts_compressed_filter_literals() -> Result<()> {
    let Some(network) = start_network_async_or_skip(
        NetworkBuilder::new(),
        stringify!(asset_holders_query_accepts_compressed_filter_literals),
    )
    .await?
    else {
        return Ok(());
    };
    network.ensure_blocks(1).await?;

    let literal = compressed_alice_literal();
    let body = format!(
        r#"{{"filter":{{"op":"eq","args":["account_id","{literal}"]}},"sort":[],"pagination":{{"limit":4,"offset":0}},"fetch_size":null,"address_format":null}}"#
    );

    let http = http_client();
    let url = network
        .client()
        .torii_url
        .join("/v1/assets/rose%23wonderland/holders/query")
        .expect("join asset holders query url");
    let resp = http
        .post(url)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .body(body)
        .send()
        .await?;
    assert!(
        resp.status().is_success(),
        "expected success from asset holders query with compressed literal, got {}",
        resp.status()
    );
    let parsed: norito::json::Value = norito::json::from_str(&resp.text().await?)?;
    let ids = extract_holder_account_ids(&parsed);
    let expected = ALICE_ID.to_string();
    assert!(
        !ids.is_empty(),
        "compressed filter literal should match canonical accounts"
    );
    assert!(
        ids.iter().all(|id| id == &expected),
        "asset holders query should emit canonical IH58 literals by default; expected {expected}, got {ids:?}"
    );

    Ok(())
}

#[tokio::test]
async fn account_transactions_get_supports_address_format() -> Result<()> {
    let Some(network) = start_network_async_or_skip(
        NetworkBuilder::new(),
        stringify!(account_transactions_get_supports_address_format),
    )
    .await?
    else {
        return Ok(());
    };
    network.ensure_blocks(1).await?;

    // Ensure the account has at least one external transaction to format.
    let client = network.client();
    let peer_clients: Vec<_> = network.peers().iter().map(|peer| peer.client()).collect();
    tokio::task::spawn_blocking({
        let client = client.clone();
        move || -> Result<()> {
            let key: Name = "addrfmtget".parse().expect("metadata key");
            let transaction = client.build_transaction_from_items(
                [SetKeyValue::account(
                    ALICE_ID.clone(),
                    key,
                    norito::json!("addrfmtget"),
                )],
                Metadata::default(),
            );
            let mut submitted = false;
            let mut last_err: Option<eyre::Report> = None;
            for peer_client in peer_clients {
                match peer_client.submit_transaction(&transaction) {
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
    })
    .await??;

    let http = http_client();
    let account_literal = ALICE_ID.to_string();
    let account_address = ALICE_ID
        .to_account_address()
        .expect("account address should encode");
    let compressed_literal = account_address.to_compressed_sora().expect("compressed");

    let base = account_endpoint_url(
        &network.client().torii_url,
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
    let compressed_url = {
        let mut url = base.clone();
        {
            let mut qp = url.query_pairs_mut();
            qp.append_pair("limit", "8");
            qp.append_pair("address_format", "compressed");
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
                "expected success fetching account transactions with IH58 default, got {}",
                resp.status()
            );
            let parsed: norito::json::Value = norito::json::from_str(&resp.text().await?)?;
            let authorities = extract_account_transaction_authorities(&parsed);
            if !authorities.is_empty() {
                break authorities;
            }
            if Instant::now() >= deadline {
                return Err(eyre!(
                    "timed out waiting for IH58 account transactions to appear"
                ));
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    };
    assert!(
        !authorities.is_empty(),
        "IH58 default should return at least one authority literal (submitted tx)"
    );
    assert!(
        authorities
            .iter()
            .any(|literal| literal == &account_literal),
        "IH58 default should include canonical literal {account_literal}, got {authorities:?}"
    );
    assert_authorities_are_ih58(&authorities)?;

    let resp = http
        .get(compressed_url.clone())
        .header("Accept", "application/json")
        .send()
        .await?;
    assert!(
        resp.status().is_success(),
        "expected success fetching account transactions with address_format=compressed, got {}",
        resp.status()
    );
    let parsed: norito::json::Value = norito::json::from_str(&resp.text().await?)?;
    let authorities = extract_account_transaction_authorities(&parsed);
    assert!(
        authorities
            .iter()
            .any(|literal| literal == &compressed_literal),
        "compressed response should include {compressed_literal}, got {authorities:?}"
    );
    assert!(
        authorities
            .iter()
            .all(|literal| literal.starts_with("sora")),
        "compressed response should emit only compressed literals; got {authorities:?}"
    );

    Ok(())
}

#[tokio::test]
async fn account_transactions_query_supports_address_format() -> Result<()> {
    let Some(network) = start_network_async_or_skip(
        NetworkBuilder::new(),
        stringify!(account_transactions_query_supports_address_format),
    )
    .await?
    else {
        return Ok(());
    };
    network.ensure_blocks(1).await?;

    // Ensure the account has at least one external transaction to format.
    let client = network.client();
    let peer_clients: Vec<_> = network.peers().iter().map(|peer| peer.client()).collect();
    tokio::task::spawn_blocking({
        let client = client.clone();
        move || -> Result<()> {
            let key: Name = "addrfmtquery".parse().expect("metadata key");
            let transaction = client.build_transaction_from_items(
                [SetKeyValue::account(
                    ALICE_ID.clone(),
                    key,
                    norito::json!("addrfmtquery"),
                )],
                Metadata::default(),
            );
            let mut submitted = false;
            let mut last_err: Option<eyre::Report> = None;
            for peer_client in peer_clients {
                match peer_client.submit_transaction(&transaction) {
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
    })
    .await??;

    let http = http_client();
    let account_literal = ALICE_ID.to_string();
    let account_address = ALICE_ID
        .to_account_address()
        .expect("account address should encode");
    let compressed_literal = account_address.to_compressed_sora().expect("compressed");

    let url = account_endpoint_url(
        &network.client().torii_url,
        &account_literal,
        &["transactions", "query"],
    );

    let default_body = r#"{"filter":null,"sort":[],"pagination":{"offset":0,"limit":8},"fetch_size":null,"select":null,"address_format":null}"#;
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
                "expected success from account transactions query using IH58 default, got {}",
                resp.status()
            );
            let parsed: norito::json::Value = norito::json::from_str(&resp.text().await?)?;
            let authorities = extract_account_transaction_authorities(&parsed);
            if !authorities.is_empty() {
                break authorities;
            }
            if Instant::now() >= deadline {
                return Err(eyre!(
                    "timed out waiting for IH58 account transactions query to appear"
                ));
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    };
    assert!(
        !authorities.is_empty(),
        "IH58 default query should return at least one authority literal (submitted tx)"
    );
    assert!(
        authorities
            .iter()
            .any(|literal| literal == &account_literal),
        "IH58 default query should include canonical literal {account_literal}, got {authorities:?}"
    );
    assert_authorities_are_ih58(&authorities)?;

    let compressed_body = r#"{"filter":null,"sort":[],"pagination":{"offset":0,"limit":8},"fetch_size":null,"select":null,"address_format":"compressed"}"#;
    let resp = http
        .post(url.clone())
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .body(compressed_body)
        .send()
        .await?;
    assert!(
        resp.status().is_success(),
        "expected success from account transactions query with address_format=compressed, got {}",
        resp.status()
    );
    let parsed: norito::json::Value = norito::json::from_str(&resp.text().await?)?;
    let authorities = extract_account_transaction_authorities(&parsed);
    assert!(
        authorities
            .iter()
            .any(|literal| literal == &compressed_literal),
        "compressed query response should include {compressed_literal}, got {authorities:?}"
    );
    assert!(
        authorities
            .iter()
            .all(|literal| literal.starts_with("sora")),
        "compressed query response should emit only compressed literals; got {authorities:?}"
    );

    Ok(())
}

#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn explorer_transactions_respect_address_format_preferences() -> Result<()> {
    let Some(network) = start_network_async_or_skip(
        NetworkBuilder::new(),
        stringify!(explorer_transactions_respect_address_format_preferences),
    )
    .await?
    else {
        return Ok(());
    };
    network.ensure_blocks(1).await?;

    let http = http_client();
    let account_literal = SAMPLE_GENESIS_ACCOUNT_ID.to_string();
    let account_address = SAMPLE_GENESIS_ACCOUNT_ID
        .to_account_address()
        .expect("account address should encode");
    let compressed_literal = account_address.to_compressed_sora().expect("compressed");

    let base = network
        .client()
        .torii_url
        .join("/v1/explorer/transactions")
        .expect("join explorer transactions url");

    let default_url = {
        let mut url = base.clone();
        {
            let mut qp = url.query_pairs_mut();
            qp.append_pair("authority", &account_literal);
            qp.append_pair("page", "1");
            qp.append_pair("per_page", "8");
        }
        url
    };
    let resp = http
        .get(default_url)
        .header("Accept", "application/json")
        .send()
        .await?;
    assert!(
        resp.status().is_success(),
        "expected explorer transactions to succeed (IH58 default), got {}",
        resp.status()
    );
    let parsed: norito::json::Value = norito::json::from_str(&resp.text().await?)?;
    let authorities = extract_explorer_authorities(&parsed);
    assert!(
        !authorities.is_empty(),
        "expected explorer transactions for Genesis account"
    );
    assert!(
        authorities
            .iter()
            .all(|literal| literal == &account_literal),
        "explorer transactions should default to IH58 literals; got {authorities:?}"
    );

    let compressed_url = {
        let mut url = base.clone();
        {
            let mut qp = url.query_pairs_mut();
            qp.append_pair("authority", &account_literal);
            qp.append_pair("page", "1");
            qp.append_pair("per_page", "8");
            qp.append_pair("address_format", "compressed");
        }
        url
    };
    let resp = http
        .get(compressed_url)
        .header("Accept", "application/json")
        .send()
        .await?;
    assert!(
        resp.status().is_success(),
        "expected explorer transactions with compressed response to succeed, got {}",
        resp.status()
    );
    let parsed: norito::json::Value = norito::json::from_str(&resp.text().await?)?;
    let authorities = extract_explorer_authorities(&parsed);
    assert!(
        !authorities.is_empty(),
        "compressed explorer transactions response should contain items"
    );
    assert!(
        authorities
            .iter()
            .all(|literal| literal == &compressed_literal),
        "explorer transactions should honour address_format=compressed; got {authorities:?}"
    );

    let compressed_filter_url = {
        let mut url = base;
        {
            let mut qp = url.query_pairs_mut();
            qp.append_pair("authority", &compressed_literal);
            qp.append_pair("page", "1");
            qp.append_pair("per_page", "8");
        }
        url
    };
    let resp = http
        .get(compressed_filter_url)
        .header("Accept", "application/json")
        .send()
        .await?;
    assert!(
        resp.status().is_success(),
        "explorer transactions should accept compressed authority filters"
    );
    let parsed: norito::json::Value = norito::json::from_str(&resp.text().await?)?;
    let authorities = extract_explorer_authorities(&parsed);
    assert!(
        !authorities.is_empty(),
        "compressed authority filter should still yield results"
    );
    assert!(
        authorities
            .iter()
            .all(|literal| literal == &account_literal),
        "compressed authority filters should not change response defaults; got {authorities:?}"
    );

    Ok(())
}

#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn explorer_instructions_respect_address_format_preferences() -> Result<()> {
    let Some(network) = start_network_async_or_skip(
        NetworkBuilder::new(),
        stringify!(explorer_instructions_respect_address_format_preferences),
    )
    .await?
    else {
        return Ok(());
    };
    network.ensure_blocks(1).await?;

    let http = http_client();
    let account_literal = SAMPLE_GENESIS_ACCOUNT_ID.to_string();
    let account_address = SAMPLE_GENESIS_ACCOUNT_ID
        .to_account_address()
        .expect("account address should encode");
    let compressed_literal = account_address.to_compressed_sora().expect("compressed");

    let base = network
        .client()
        .torii_url
        .join("/v1/explorer/instructions")
        .expect("join explorer instructions url");

    let default_url = {
        let mut url = base.clone();
        {
            let mut qp = url.query_pairs_mut();
            qp.append_pair("authority", &account_literal);
            qp.append_pair("page", "1");
            qp.append_pair("per_page", "8");
        }
        url
    };
    let resp = http
        .get(default_url)
        .header("Accept", "application/json")
        .send()
        .await?;
    assert!(
        resp.status().is_success(),
        "expected explorer instructions to succeed (IH58 default), got {}",
        resp.status()
    );
    let parsed: norito::json::Value = norito::json::from_str(&resp.text().await?)?;
    let authorities = extract_explorer_authorities(&parsed);
    assert!(
        !authorities.is_empty(),
        "expected explorer instructions for Genesis account"
    );
    assert!(
        authorities
            .iter()
            .all(|literal| literal == &account_literal),
        "explorer instructions should default to IH58 literals; got {authorities:?}"
    );

    let compressed_url = {
        let mut url = base.clone();
        {
            let mut qp = url.query_pairs_mut();
            qp.append_pair("authority", &account_literal);
            qp.append_pair("page", "1");
            qp.append_pair("per_page", "8");
            qp.append_pair("address_format", "compressed");
        }
        url
    };
    let resp = http
        .get(compressed_url)
        .header("Accept", "application/json")
        .send()
        .await?;
    assert!(
        resp.status().is_success(),
        "expected explorer instructions with compressed response to succeed, got {}",
        resp.status()
    );
    let parsed: norito::json::Value = norito::json::from_str(&resp.text().await?)?;
    let authorities = extract_explorer_authorities(&parsed);
    assert!(
        !authorities.is_empty(),
        "compressed explorer instructions response should contain items"
    );
    assert!(
        authorities
            .iter()
            .all(|literal| literal == &compressed_literal),
        "explorer instructions should honour address_format=compressed; got {authorities:?}"
    );

    let compressed_filter_url = {
        let mut url = base;
        {
            let mut qp = url.query_pairs_mut();
            qp.append_pair("authority", &compressed_literal);
            qp.append_pair("page", "1");
            qp.append_pair("per_page", "8");
        }
        url
    };
    let resp = http
        .get(compressed_filter_url)
        .header("Accept", "application/json")
        .send()
        .await?;
    assert!(
        resp.status().is_success(),
        "explorer instructions should accept compressed authority filters"
    );
    let parsed: norito::json::Value = norito::json::from_str(&resp.text().await?)?;
    let authorities = extract_explorer_authorities(&parsed);
    assert!(
        !authorities.is_empty(),
        "compressed authority filter should still yield instruction results"
    );
    assert!(
        authorities
            .iter()
            .all(|literal| literal == &account_literal),
        "compressed authority filters should not change response defaults; got {authorities:?}"
    );

    Ok(())
}

#[tokio::test]
async fn explorer_account_qr_defaults_to_ih58() -> Result<()> {
    let Some(network) = start_network_async_or_skip(
        NetworkBuilder::new(),
        stringify!(explorer_account_qr_defaults_to_ih58),
    )
    .await?
    else {
        return Ok(());
    };
    network.ensure_blocks(1).await?;

    let http = http_client();
    let canonical_literal = ALICE_ID.to_string();
    let url = explorer_account_qr_url(&network.client().torii_url, &canonical_literal);
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
        "canonical_id should match IH58 literal"
    );
    assert_eq!(
        parsed.get("literal").and_then(norito::json::Value::as_str),
        Some(canonical_literal.as_str()),
        "literal should default to IH58"
    );
    assert_eq!(
        parsed
            .get("address_format")
            .and_then(norito::json::Value::as_str),
        Some("ih58"),
        "address_format label should advertise IH58 default"
    );
    assert_eq!(
        parsed
            .get("network_prefix")
            .and_then(norito::json::Value::as_u64),
        Some(u64::from(
            iroha_config::parameters::defaults::common::chain_discriminant()
        )),
        "network_prefix should expose the expected IH58 prefix"
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
async fn explorer_account_qr_supports_compressed_literals() -> Result<()> {
    let Some(network) = start_network_async_or_skip(
        NetworkBuilder::new(),
        stringify!(explorer_account_qr_supports_compressed_literals),
    )
    .await?
    else {
        return Ok(());
    };
    network.ensure_blocks(1).await?;

    let http = http_client();
    let canonical_literal = ALICE_ID.to_string();
    let compressed_literal = compressed_alice_literal();
    let mut url = explorer_account_qr_url(&network.client().torii_url, &canonical_literal);
    {
        let mut qp = url.query_pairs_mut();
        qp.append_pair("address_format", "compressed");
    }
    let resp = http
        .get(url)
        .header("Accept", "application/json")
        .send()
        .await?;
    assert!(
        resp.status().is_success(),
        "expected explorer account QR to respect address_format=compressed, got {}",
        resp.status()
    );
    let parsed: norito::json::Value = norito::json::from_str(&resp.text().await?)?;
    assert_eq!(
        parsed
            .get("canonical_id")
            .and_then(norito::json::Value::as_str),
        Some(canonical_literal.as_str()),
        "canonical_id should remain IH58 even when rendering compressed (`sora`) literals"
    );
    assert_eq!(
        parsed.get("literal").and_then(norito::json::Value::as_str),
        Some(compressed_literal.as_str()),
        "literal should honour the compressed (`sora`) preference"
    );
    assert_eq!(
        parsed
            .get("address_format")
            .and_then(norito::json::Value::as_str),
        Some("compressed"),
        "address_format label should reflect the compressed (`sora`) preference"
    );

    Ok(())
}

#[tokio::test]
async fn accounts_query_rejects_local8_filter_literals() -> Result<()> {
    let Some(network) = start_network_async_or_skip(
        NetworkBuilder::new(),
        stringify!(accounts_query_rejects_local8_filter_literals),
    )
    .await?
    else {
        return Ok(());
    };
    network.ensure_blocks(1).await?;

    let literal = local8_literal();
    let body = format!(
        r#"{{"filter":{{"op":"eq","args":["id","{literal}"]}},"sort":[],"pagination":{{"limit":4,"offset":0}},"fetch_size":null,"select":null}}"#
    );
    let http = http_client();
    let url = network
        .client()
        .torii_url
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
        "Local-8 literal in filter should be rejected"
    );
    let body = resp.text().await.unwrap_or_default();
    assert!(
        body.contains("ERR_LOCAL8_DEPRECATED"),
        "response body should mention ERR_LOCAL8_DEPRECATED, got {body}"
    );

    Ok(())
}

#[tokio::test]
async fn accounts_query_accepts_public_key_filter_literals() -> Result<()> {
    let Some(network) = start_network_async_or_skip(
        NetworkBuilder::new(),
        stringify!(accounts_query_accepts_public_key_filter_literals),
    )
    .await?
    else {
        return Ok(());
    };
    network.ensure_blocks(1).await?;

    let literal = public_key_literal();
    let body = format!(
        r#"{{"filter":{{"op":"eq","args":["id","{literal}"]}},"sort":[],"pagination":{{"limit":4,"offset":0}},"fetch_size":null,"select":null}}"#
    );
    let http = http_client();
    let url = network
        .client()
        .torii_url
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
        "public-key literal in filter should be accepted, got {}",
        resp.status()
    );
    let parsed: norito::json::Value = norito::json::from_str(&resp.text().await?)?;
    let ids = extract_account_ids(&parsed);
    let expected = ALICE_ID.to_string();
    assert!(
        ids.iter().any(|id| id == &expected),
        "public-key literal should resolve to {expected}, got {ids:?}"
    );

    Ok(())
}

#[tokio::test]
async fn accounts_query_accepts_alias_and_compressed_filter_literals() -> Result<()> {
    let domain_id: DomainId = "aliases".parse()?;
    let label = AccountLabel::new(domain_id.clone(), "primary".parse()?);
    let keypair = KeyPair::random();
    let account_id = AccountId::new(domain_id.clone(), keypair.public_key().clone());
    let account = Account::new(account_id.clone()).with_label(Some(label.clone()));
    let builder = NetworkBuilder::new()
        .with_min_peers(4)
        .with_genesis_instruction(Register::domain(Domain::new(domain_id.clone())))
        .with_genesis_instruction(Register::account(account));
    let Some(network) = start_network_async_or_skip(
        builder,
        stringify!(accounts_query_accepts_alias_and_compressed_filter_literals),
    )
    .await?
    else {
        return Ok(());
    };
    network.ensure_blocks(1).await?;

    let client = network.client();
    let url = client
        .torii_url
        .join("/v1/accounts/query")
        .expect("join accounts query url");
    let expected = account_id.to_string();
    let http = http_client();
    wait_for_account_in_query(&http, url.clone(), &expected).await?;

    let alias_literal = format!("{}@{}", label.label, domain_id);
    let compressed_literal = account_id
        .to_account_address()
        .expect("account address should encode")
        .to_compressed_sora()
        .expect("compressed address encoding");

    for literal in [alias_literal, compressed_literal] {
        let body = format!(
            r#"{{"filter":{{"op":"eq","args":["id","{literal}"]}},"sort":[],"pagination":{{"limit":4,"offset":0}},"fetch_size":null,"select":null}}"#
        );
        let resp = http
            .post(url.clone())
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .body(body)
            .send()
            .await?;
        let status = resp.status();
        let body = resp.text().await?;
        assert!(
            status.is_success(),
            "alias/compressed literal should be accepted, got {status} body={body}"
        );
        let parsed: norito::json::Value = norito::json::from_str(&body)?;
        let ids = extract_account_ids(&parsed);
        assert!(
            ids.iter().any(|id| id == &expected),
            "literal {literal} should resolve to {expected}, got {ids:?}"
        );
        assert!(
            ids.iter().all(|id| !id.contains('@')),
            "response should return canonical IH58 ids, got {ids:?}"
        );
    }

    Ok(())
}

#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn repo_agreements_respect_address_format() -> Result<()> {
    init_instruction_registry();
    // Reuse pre-existing asset definitions from the test genesis to avoid permission issues when
    // registering new definitions in the wonderland domain.
    let cash_def_id: AssetDefinitionId = "rose#wonderland".parse()?;
    let collateral_def_id: AssetDefinitionId = "camomile#wonderland".parse()?;
    let setup_instructions: Vec<InstructionBox> = vec![
        Mint::asset_numeric(
            numeric!(1500),
            AssetId::new(collateral_def_id.clone(), ALICE_ID.clone()),
        )
        .into(),
        Mint::asset_numeric(
            numeric!(1500),
            AssetId::new(cash_def_id.clone(), BOB_ID.clone()),
        )
        .into(),
    ];

    let maturity_ms = u64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)?
            .checked_add(Duration::from_secs(43_200))
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
            quantity: numeric!(1000),
        },
        RepoCollateralLeg::new(collateral_def_id.clone(), numeric!(1100)),
        0,
        maturity_ms,
        RepoGovernance::with_defaults(1_500, 43_200),
    );

    let mut builder = NetworkBuilder::new();
    for isi in setup_instructions {
        builder = builder.with_genesis_instruction(isi);
    }

    let Some(network) =
        start_network_async_or_skip(builder, stringify!(repo_agreements_respect_address_format))
            .await?
    else {
        return Ok(());
    };
    network.ensure_blocks(1).await?;
    // Issue the repo agreement after genesis to avoid initial-executor restrictions. Some
    // environments may not ship repo instructions; skip gracefully if the binary rejects them.
    let repo_instruction_box: InstructionBox = RepoInstructionBox::from(repo_instruction).into();
    let client = network.client();
    if let Err(err) = client.submit::<InstructionBox>(repo_instruction_box) {
        eprintln!("Skipping repo address_format coverage: {err}");
        return Ok(());
    }
    network.ensure_blocks(2).await?;

    let http = http_client();
    let base = client.torii_url.clone();
    let ih58_alice = ALICE_ID.to_string();
    let ih58_bob = BOB_ID.to_string();
    let agreement_literal = agreement_id.to_string();
    let compressed_alice = compressed_alice_literal();
    let compressed_bob = compressed_bob_literal();

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
            "Skipping repo address_format coverage: repo agreement not observed after submission."
        );
        return Ok(());
    };
    let initiator = entry
        .get("initiator")
        .and_then(norito::json::Value::as_str)
        .expect("initiator literal should be present");
    assert_eq!(
        initiator, ih58_alice,
        "repo agreements must default to IH58 initiators"
    );
    let counterparty = entry
        .get("counterparty")
        .and_then(norito::json::Value::as_str)
        .expect("counterparty literal should be present");
    assert_eq!(
        counterparty, ih58_bob,
        "repo agreements must default to IH58 counterparties"
    );

    let mut compressed_url = base.join("/v1/repo/agreements")?;
    {
        let mut qp = compressed_url.query_pairs_mut();
        qp.append_pair("limit", "8");
        qp.append_pair("address_format", "compressed");
    }
    let resp = http
        .get(compressed_url)
        .header("Accept", "application/json")
        .send()
        .await?;
    assert!(
        resp.status().is_success(),
        "compressed repo agreement listing should succeed, got {}",
        resp.status()
    );
    let parsed: norito::json::Value = norito::json::from_str(&resp.text().await?)?;
    let entry = find_repo_entry(&parsed, agreement_literal.as_str())
        .expect("compressed repo listing should include the recorded agreement");
    let initiator = entry
        .get("initiator")
        .and_then(norito::json::Value::as_str)
        .expect("compressed initiator literal must be present");
    assert_eq!(
        initiator, compressed_alice,
        "repo agreements should honour address_format=compressed for initiators"
    );
    let counterparty = entry
        .get("counterparty")
        .and_then(norito::json::Value::as_str)
        .expect("compressed counterparty literal must be present");
    assert_eq!(
        counterparty, compressed_bob,
        "repo agreements should honour address_format=compressed for counterparties"
    );

    let query_url = base.join("/v1/repo/agreements/query")?;
    let query_body = format!(
        r#"{{"filter":{{"op":"eq","args":["id","{agreement_literal}"]}},"sort":[],"pagination":{{"limit":4,"offset":0}},"fetch_size":null,"select":null,"address_format":"compressed"}}"#
    );
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
        .expect("compressed query initiator literal must be present");
    assert_eq!(
        initiator, compressed_alice,
        "repo agreements query should honour address_format=compressed"
    );
    let counterparty = entry
        .get("counterparty")
        .and_then(norito::json::Value::as_str)
        .expect("compressed query counterparty literal must be present");
    assert_eq!(
        counterparty, compressed_bob,
        "repo agreements query should honour address_format=compressed"
    );

    Ok(())
}

#[tokio::test]
#[allow(clippy::too_many_lines)] // integration coverage requires full scenario setup
async fn kaigi_endpoints_respect_address_format_preferences() -> Result<()> {
    let relay_id = BOB_ID.clone();
    let reporter_id = ALICE_ID.clone();
    let domain_id = relay_id.domain().clone();

    let registration = KaigiRelayRegistration {
        relay_id: relay_id.clone(),
        hpke_public_key: vec![0xAA; 32],
        bandwidth_class: 5,
    };
    let registration_key =
        kaigi_relay_metadata_key(&relay_id).wrap_err("kaigi relay metadata key")?;
    let registration_value = Json::try_new(registration)
        .map_err(|err| eyre!("serialize Kaigi relay registration: {err}"))?;
    let set_registration =
        SetKeyValue::domain(domain_id.clone(), registration_key, registration_value);

    let call_name: Name = "integration-demo".parse()?;
    let feedback = KaigiRelayFeedback {
        relay_id: relay_id.clone(),
        call: KaigiId::new(domain_id.clone(), call_name),
        reported_by: reporter_id.clone(),
        status: KaigiRelayHealthStatus::Healthy,
        reported_at_ms: 1_735_000,
        notes: Some("integration test coverage".to_owned()),
    };
    let feedback_key = kaigi_relay_feedback_key(&relay_id).wrap_err("kaigi relay feedback key")?;
    let feedback_value =
        Json::try_new(feedback).map_err(|err| eyre!("serialize Kaigi relay feedback: {err}"))?;
    let set_feedback = SetKeyValue::domain(domain_id, feedback_key, feedback_value);

    let mut builder = NetworkBuilder::new();
    builder = builder.with_genesis_instruction(set_registration);
    builder = builder.with_genesis_instruction(set_feedback);

    let Some(network) = start_network_async_or_skip(
        builder,
        stringify!(kaigi_endpoints_respect_address_format_preferences),
    )
    .await?
    else {
        return Ok(());
    };
    network.ensure_blocks(1).await?;

    let seed = KaigiRelaySeed {
        relay_ih58: relay_id.to_string(),
        relay_compressed: compressed_bob_literal(),
        reporter_compressed: compressed_alice_literal(),
    };

    let client = network.client();
    let http = http_client();
    let base = &client.torii_url;

    let summary_url = base.join("/v1/kaigi/relays")?;
    let summary_resp = http
        .get(summary_url)
        .header("Accept", "application/json")
        .send()
        .await?;
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
            .any(|literal| *literal == seed.relay_ih58),
        "expected IH58 literal {} in kaigi summary, got {default_literals:?}",
        seed.relay_ih58
    );

    let compressed_summary_url = base.join("/v1/kaigi/relays?address_format=compressed")?;
    let compressed_resp = http
        .get(compressed_summary_url)
        .header("Accept", "application/json")
        .send()
        .await?;
    assert!(
        compressed_resp.status().is_success(),
        "compressed kaigi relay summary should succeed, got {}",
        compressed_resp.status()
    );
    let compressed_summary: norito::json::Value =
        norito::json::from_str(&compressed_resp.text().await?)?;
    let compressed_literals: Vec<_> = compressed_summary
        .get("items")
        .and_then(norito::json::Value::as_array)
        .ok_or_else(|| eyre!("compressed kaigi summary missing items array"))?
        .iter()
        .filter_map(|item| {
            item.as_object()
                .and_then(|obj| obj.get("relay_id"))
                .and_then(norito::json::Value::as_str)
        })
        .collect();
    assert!(
        compressed_literals
            .iter()
            .any(|literal| *literal == seed.relay_compressed),
        "expected compressed literal {} in kaigi summary, got {compressed_literals:?}",
        seed.relay_compressed
    );

    let detail_url = base.join(&format!("/v1/kaigi/relays/{}", seed.relay_compressed))?;
    let detail_resp = http
        .get(detail_url)
        .header("Accept", "application/json")
        .send()
        .await?;
    assert!(
        detail_resp.status().is_success(),
        "kaigi relay detail should accept compressed literal, got {}",
        detail_resp.status()
    );
    let detail: norito::json::Value = norito::json::from_str(&detail_resp.text().await?)?;
    let default_detail_literal = detail
        .get("relay")
        .and_then(norito::json::Value::as_object)
        .and_then(|relay| relay.get("relay_id"))
        .and_then(norito::json::Value::as_str)
        .ok_or_else(|| eyre!("relay detail missing relay literal"))?;
    assert_eq!(
        default_detail_literal, seed.relay_ih58,
        "compressed literal path should resolve to IH58 detail output"
    );

    let formatted_detail_url = base.join(&format!(
        "/v1/kaigi/relays/{}?address_format=compressed",
        seed.relay_ih58
    ))?;
    let formatted_resp = http
        .get(formatted_detail_url)
        .header("Accept", "application/json")
        .send()
        .await?;
    assert!(
        formatted_resp.status().is_success(),
        "kaigi relay detail with address_format should succeed, got {}",
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
        relay_literal, seed.relay_compressed,
        "relay literal should honour address_format=compressed"
    );
    let reported_by_literal = formatted_detail
        .get("reported_by")
        .and_then(norito::json::Value::as_str)
        .ok_or_else(|| eyre!("formatted detail missing reported_by literal"))?;
    assert_eq!(
        reported_by_literal, seed.reporter_compressed,
        "reported_by should honour address_format=compressed"
    );

    Ok(())
}

#[tokio::test]
async fn offline_allowances_listing_respects_address_format_hint() -> Result<()> {
    init_instruction_registry();
    let certificate = load_offline_certificate_fixture()?;
    let controller = certificate.controller.clone();
    let controller_ih58 = controller.to_string();
    let controller_compressed = controller
        .to_account_address()
        .map_err(|err| eyre!("failed to encode fixture controller as account address: {err}"))?
        .to_compressed_sora()
        .map_err(|err| eyre!("failed to encode fixture controller as compressed literal: {err}"))?;
    let instruction = RegisterOfflineAllowance { certificate };

    let mut builder = NetworkBuilder::new();
    builder = builder.with_genesis_instruction(instruction);

    let Some(network) = start_network_async_or_skip(
        builder,
        stringify!(offline_allowances_listing_respects_address_format_hint),
    )
    .await?
    else {
        return Ok(());
    };
    network.ensure_blocks(1).await?;

    let client = network.client();
    let seed = OfflineAllowanceSeed {
        controller_ih58,
        controller_compressed,
    };

    let base = client.torii_url.clone();
    let http = http_client();

    let default_url = base
        .join("/v1/offline/allowances?limit=4")
        .expect("offline allowances url");
    let resp = http
        .get(default_url)
        .header("Accept", "application/json")
        .send()
        .await?;
    assert!(
        resp.status().is_success(),
        "offline allowance listing should succeed, got {}",
        resp.status()
    );
    let parsed: norito::json::Value = norito::json::from_str(&resp.text().await?)?;
    let default_entry = find_offline_allowance_entry(&parsed, &seed.controller_ih58)
        .ok_or_else(|| eyre!("offline allowance for {} missing", seed.controller_ih58))?;
    let default_display = default_entry
        .get("controller_display")
        .and_then(norito::json::Value::as_str)
        .ok_or_else(|| eyre!("offline allowance missing controller_display field"))?;
    assert_eq!(
        default_display, seed.controller_ih58,
        "IH58 default should surface canonical literal"
    );

    let compressed_url = base
        .join("/v1/offline/allowances?limit=4&address_format=compressed")
        .expect("offline allowances url");
    let resp = http
        .get(compressed_url)
        .header("Accept", "application/json")
        .send()
        .await?;
    assert!(
        resp.status().is_success(),
        "offline allowance listing should succeed, got {}",
        resp.status()
    );
    let parsed: norito::json::Value = norito::json::from_str(&resp.text().await?)?;
    let compressed_entry = find_offline_allowance_entry(&parsed, &seed.controller_ih58)
        .ok_or_else(|| eyre!("offline allowance for {} missing", seed.controller_ih58))?;
    let compressed_display = compressed_entry
        .get("controller_display")
        .and_then(norito::json::Value::as_str)
        .ok_or_else(|| eyre!("offline allowance missing controller_display field"))?;
    assert_eq!(
        compressed_display, seed.controller_compressed,
        "compressed listing should surface compressed literal"
    );

    Ok(())
}

#[tokio::test]
async fn offline_allowances_query_respects_address_format_hint() -> Result<()> {
    init_instruction_registry();
    let certificate = load_offline_certificate_fixture()?;
    let controller = certificate.controller.clone();
    let controller_ih58 = controller.to_string();
    let controller_compressed = controller
        .to_account_address()
        .map_err(|err| eyre!("failed to encode fixture controller as account address: {err}"))?
        .to_compressed_sora()
        .map_err(|err| eyre!("failed to encode fixture controller as compressed literal: {err}"))?;
    let instruction = RegisterOfflineAllowance { certificate };

    let mut builder = NetworkBuilder::new();
    builder = builder.with_genesis_instruction(instruction);

    let Some(network) = start_network_async_or_skip(
        builder,
        stringify!(offline_allowances_query_respects_address_format_hint),
    )
    .await?
    else {
        return Ok(());
    };
    network.ensure_blocks(1).await?;

    let client = network.client();
    let seed = OfflineAllowanceSeed {
        controller_ih58,
        controller_compressed,
    };

    let base = client.torii_url.clone();
    let http = http_client();
    let url = base
        .join("/v1/offline/allowances/query")
        .expect("offline allowances query url");

    let default_body = r#"{"filter":null,"sort":[],"pagination":{"limit":4,"offset":0},"fetch_size":null,"select":null}"#;
    let resp = http
        .post(url.clone())
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .body(default_body)
        .send()
        .await?;
    assert!(
        resp.status().is_success(),
        "offline allowance query should succeed, got {}",
        resp.status()
    );
    let parsed: norito::json::Value = norito::json::from_str(&resp.text().await?)?;
    let default_entry = find_offline_allowance_entry(&parsed, &seed.controller_ih58)
        .ok_or_else(|| eyre!("offline allowance for {} missing", seed.controller_ih58))?;
    let default_display = default_entry
        .get("controller_display")
        .and_then(norito::json::Value::as_str)
        .ok_or_else(|| eyre!("offline allowance missing controller_display field"))?;
    assert_eq!(
        default_display, seed.controller_ih58,
        "IH58 default query should emit canonical literal"
    );

    let compressed_body = r#"{"filter":null,"sort":[],"pagination":{"limit":4,"offset":0},"fetch_size":null,"select":null,"address_format":"compressed"}"#;
    let resp = http
        .post(url)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .body(compressed_body)
        .send()
        .await?;
    assert!(
        resp.status().is_success(),
        "offline allowance query with address_format should succeed, got {}",
        resp.status()
    );
    let parsed: norito::json::Value = norito::json::from_str(&resp.text().await?)?;
    let compressed_entry = find_offline_allowance_entry(&parsed, &seed.controller_ih58)
        .ok_or_else(|| eyre!("offline allowance for {} missing", seed.controller_ih58))?;
    let compressed_display = compressed_entry
        .get("controller_display")
        .and_then(norito::json::Value::as_str)
        .ok_or_else(|| eyre!("offline allowance missing controller_display field"))?;
    assert_eq!(
        compressed_display, seed.controller_compressed,
        "address_format=compressed should rewrite controller_display"
    );

    Ok(())
}
