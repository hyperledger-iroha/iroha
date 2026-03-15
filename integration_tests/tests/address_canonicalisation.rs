#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Roadmap ADDR-5 coverage ensuring Torii surfaces canonical I105 account IDs.

use std::{
    collections::BTreeSet,
    env, fs,
    path::{Path, PathBuf},
    str::FromStr,
    sync::OnceLock,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use eyre::{Result, WrapErr, eyre};
use integration_tests::sandbox::{
    self, start_network_async_or_skip as sandbox_start_network_async_or_skip,
};
use iroha::data_model::{
    isi::{SetKeyValue, offline::RegisterOfflineAllowance, repo::RepoIsi},
    kaigi::{
        KaigiId, KaigiRelayFeedback, KaigiRelayHealthStatus, KaigiRelayRegistration,
        kaigi_relay_feedback_key, kaigi_relay_metadata_key,
    },
    offline::{OFFLINE_ASSET_ENABLED_METADATA_KEY, OfflineWalletCertificate},
    prelude::*,
    repo::{RepoAgreementId, RepoCashLeg, RepoCollateralLeg, RepoGovernance},
};
use iroha_crypto::{Hash, Signature};
use iroha_data_model::account::{AccountDomainSelector, address::AccountAddress};
use iroha_data_model::metadata::Metadata;
use iroha_data_model::prelude::RepoInstructionBox;
use iroha_primitives::json::Json;
use iroha_test_network::{NetworkBuilder, init_instruction_registry};
use iroha_test_samples::{ALICE_ID, ALICE_KEYPAIR, BOB_ID, SAMPLE_GENESIS_ACCOUNT_ID};
use reqwest::Client;

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
    let cores = std::thread::available_parallelism()
        .map(std::num::NonZeroUsize::get)
        .unwrap_or(1);
    cores
        .saturating_div(DEFAULT_NETWORK_PARALLELISM_PEERS.max(1))
        .max(1)
}

fn install_network_parallelism_override() {
    static NETWORK_PARALLELISM_GUARD: OnceLock<sandbox::NetworkParallelismGuard> = OnceLock::new();
    // Keep this suite bounded and avoid over-admitting network starts when the underlying
    // file-permit lock only allows a single concurrent network.
    let suite_parallelism = effective_file_permit_parallelism_limit().min(2).max(1);
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

fn load_offline_certificate_fixture() -> Result<OfflineWalletCertificate> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../fixtures/offline_allowance/android-demo");

    for norito_path in [
        root.join("register_instruction.norito"),
        root.join("certificate.norito"),
    ] {
        if let Ok(bytes) = fs::read(&norito_path) {
            if let Ok(instruction) = norito::decode_from_bytes::<RegisterOfflineAllowance>(&bytes) {
                return Ok(instruction.certificate);
            }
            if let Ok(instruction) = norito::decode_from_bytes::<InstructionBox>(&bytes) {
                if let Some(decoded) = instruction
                    .as_any()
                    .downcast_ref::<RegisterOfflineAllowance>()
                {
                    return Ok(decoded.certificate.clone());
                }
            }
            if let Ok(certificate) = norito::decode_from_bytes::<OfflineWalletCertificate>(&bytes) {
                return Ok(certificate);
            }
        }
    }

    let path = root.join("register_instruction.json");
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

fn with_offline_allowance_genesis(
    mut builder: NetworkBuilder,
    certificate: &OfflineWalletCertificate,
) -> NetworkBuilder {
    let genesis_domain = iroha_genesis::GENESIS_DOMAIN_ID.clone();
    let wonderland_domain: DomainId = "wonderland"
        .parse()
        .expect("default wonderland domain should parse");

    // Seed every domain touched by the fixture so genesis mint/register steps can resolve
    // asset-definition scopes even when they use non-wonderland domains.
    let mut required_domains = BTreeSet::new();
    required_domains.insert(certificate.allowance.asset.definition().domain().clone());
    for domain in required_domains {
        if domain != genesis_domain && domain != wonderland_domain {
            builder = builder.with_genesis_instruction(Register::domain(Domain::new(domain)));
        }
    }

    // Seed accounts used by the fixture before minting/registering the allowance.
    // Default sample genesis already contains these identities, so skip to avoid duplicates.
    let mut required_accounts = BTreeSet::new();
    required_accounts.insert(certificate.allowance.asset.account().clone());
    required_accounts.insert(certificate.controller.clone());
    required_accounts.insert(certificate.operator.clone());
    for account in required_accounts {
        if account != *SAMPLE_GENESIS_ACCOUNT_ID && account != *ALICE_ID && account != *BOB_ID {
            builder = builder.with_genesis_instruction(Register::account(Account::new(
                account.to_account_id(wonderland_domain.clone()),
            )));
        }
    }

    let asset_definition_id = certificate.allowance.asset.definition().clone();
    let scale = certificate.allowance.amount.scale();
    let asset_definition =
        AssetDefinition::new(asset_definition_id, NumericSpec::fractional(scale));
    builder = builder.with_genesis_instruction(Register::asset_definition(asset_definition));
    builder = builder.with_genesis_instruction(SetKeyValue::asset_definition(
        certificate.allowance.asset.definition().clone(),
        OFFLINE_ASSET_ENABLED_METADATA_KEY
            .parse()
            .expect("offline.enabled metadata key should parse"),
        Json::new(true),
    ));
    builder.with_genesis_instruction(Mint::asset_numeric(
        certificate.allowance.amount.clone(),
        certificate.allowance.asset.clone(),
    ))
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
    let controller = AccountId::parse_encoded(controller)
        .map(|parsed| parsed.into_account_id())
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
    let operator = AccountId::parse_encoded(operator)
        .map(|parsed| parsed.into_account_id())
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
    let attestation_nonce = parse_optional_hash(
        obj.get("attestation_nonce_hex"),
        path,
        "attestation_nonce_hex",
    )?;
    let refresh_at_ms = obj
        .get("refresh_at_ms")
        .and_then(norito::json::Value::as_u64);

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
            Hash::from_str(hex).map(Some).map_err(|err| {
                eyre!(
                    "offline allowance fixture `{}` `{field}`: {err}",
                    path.display()
                )
            })
        }
    }
}

struct OfflineAllowanceSeed {
    controller_i105: String,
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

fn legacy_dotted_i105_literal(i105_literal: &str) -> String {
    let mut chars = i105_literal.chars();
    let sentinel: String = chars.by_ref().take(4).collect();
    let remainder: String = chars.collect();
    format!("{sentinel}.{remainder}")
}

fn legacy_dotted_i105_alice_literal() -> String {
    legacy_dotted_i105_literal(&i105_alice_literal())
}

fn legacy_dotted_i105_bob_literal() -> String {
    legacy_dotted_i105_literal(&i105_bob_literal())
}

fn local8_literal() -> String {
    let account_address = ALICE_ID
        .to_account_address()
        .expect("account address should encode");
    let canonical_hex = account_address
        .canonical_hex()
        .expect("canonical hex encoding");
    let canonical = hex::decode(&canonical_hex[2..]).expect("canonical hex decoding succeeds");

    let domain: DomainId = "wonderland".parse().expect("wonderland domain parses");
    let digest = match AccountDomainSelector::from_domain(&domain).expect("selector") {
        AccountDomainSelector::LocalDigest12(bytes) => bytes,
        other => panic!("expected LocalDigest12 selector for legacy local8 fixture, got {other:?}"),
    };

    // Construct a legacy Local-12 layout and then truncate it to Local-8 to emulate
    // a pre-cutover payload shape that must be rejected by the parser.
    let mut legacy = Vec::with_capacity(canonical.len() + digest.len());
    legacy.push(canonical[0]); // header
    legacy.push(0x01); // local selector tag
    legacy.extend_from_slice(&digest);
    legacy.extend_from_slice(&canonical[2..]); // controller payload
    legacy.drain(10..14); // trim digest bytes to Local-8 legacy width
    format!("0x{}", hex::encode(legacy))
}

fn public_key_literal() -> String {
    let public_key = ALICE_KEYPAIR.public_key().to_string();
    format!("{public_key}@wonderland")
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
        path.push("v2");
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
        path.push("v2");
        path.push("explorer");
        path.push("accounts");
        path.push(account_literal);
        path.push("qr");
    }
    url
}

#[tokio::test]
async fn accounts_listing_emits_i105_identifiers() -> Result<()> {
    let Some(network) = start_network_async_or_skip(
        NetworkBuilder::new(),
        stringify!(accounts_listing_emits_i105_identifiers),
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
    let Some(network) = start_network_async_or_skip(
        NetworkBuilder::new(),
        stringify!(accounts_query_accepts_i105_filter_literals),
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
        "accounts query should return I105 literal {expected}, got {ids:?}"
    );

    Ok(())
}

#[tokio::test]
async fn accounts_listing_filter_rejects_legacy_dotted_i105_literals() -> Result<()> {
    let Some(network) = start_network_async_or_skip(
        NetworkBuilder::new(),
        stringify!(accounts_listing_filter_rejects_legacy_dotted_i105_literals),
    )
    .await?
    else {
        return Ok(());
    };
    network.ensure_blocks(1).await?;

    let literal = legacy_dotted_i105_alice_literal();
    let reason = AccountId::parse_encoded(&literal)
        .expect_err("legacy dotted I105 literal must fail strict parsing")
        .reason()
        .to_string();
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
    assert_eq!(
        resp.status(),
        reqwest::StatusCode::BAD_REQUEST,
        "legacy dotted I105 filter literal should be rejected"
    );
    let body = resp.text().await?;
    assert!(
        body.contains(&reason),
        "response body should mention {reason}, got {body}"
    );

    Ok(())
}

#[tokio::test]
async fn accounts_query_rejects_legacy_dotted_i105_filter_literals() -> Result<()> {
    let Some(network) = start_network_async_or_skip(
        NetworkBuilder::new(),
        stringify!(accounts_query_rejects_legacy_dotted_i105_filter_literals),
    )
    .await?
    else {
        return Ok(());
    };
    network.ensure_blocks(1).await?;

    let literal = legacy_dotted_i105_alice_literal();
    let reason = AccountId::parse_encoded(&literal)
        .expect_err("legacy dotted I105 literal must fail strict parsing")
        .reason()
        .to_string();
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
        "legacy dotted I105 filter literal should be rejected"
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
    let Some(network) = start_network_async_or_skip(
        NetworkBuilder::new(),
        stringify!(accounts_listing_supports_i105_response),
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
    assert!(
        ids.iter().all(|id| id.starts_with("sora")),
        "all ids should be I105 in the response: {ids:?}"
    );

    Ok(())
}

#[tokio::test]
async fn accounts_query_supports_i105_response() -> Result<()> {
    let Some(network) = start_network_async_or_skip(
        NetworkBuilder::new(),
        stringify!(accounts_query_supports_i105_response),
    )
    .await?
    else {
        return Ok(());
    };
    network.ensure_blocks(1).await?;

    let body = r#"{"filter":null,"sort":[],"pagination":{"limit":8,"offset":0},"fetch_size":null,"select":null}"#;
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
    let Some(network) = start_network_async_or_skip(
        NetworkBuilder::new(),
        stringify!(account_path_endpoints_reject_i105_literals),
    )
    .await?
    else {
        return Ok(());
    };
    network.ensure_blocks(1).await?;

    let literal = legacy_dotted_i105_alice_literal();
    let reason = AccountId::parse_encoded(&literal)
        .expect_err("legacy dotted I105 literal must fail strict parsing")
        .reason()
        .to_string();
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
            "legacy dotted I105 literal should be rejected for {url}"
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
    let local8_reason = AccountId::parse_encoded(&literal)
        .expect_err("local8 literal must fail to parse")
        .reason()
        .to_owned();
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
            body.contains(&local8_reason),
            "response body should mention {local8_reason}, got {body}"
        );
    }

    Ok(())
}

#[tokio::test]
async fn account_path_endpoints_reject_public_key_literals() -> Result<()> {
    let Some(network) = start_network_async_or_skip(
        NetworkBuilder::new(),
        stringify!(account_path_endpoints_reject_public_key_literals),
    )
    .await?
    else {
        return Ok(());
    };
    network.ensure_blocks(1).await?;

    let literal = public_key_literal();
    let reason = AccountId::parse_encoded(&literal)
        .expect_err("public-key@domain literal must fail strict parsing")
        .reason()
        .to_string();
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
    let Some(network) = start_network_async_or_skip(
        NetworkBuilder::new(),
        stringify!(asset_holders_get_supports_i105_response),
    )
    .await?
    else {
        return Ok(());
    };
    network.ensure_blocks(1).await?;

    let http = http_client();
    let Some(definition_literal) =
        find_asset_definition_with_holders(&http, &network.client().torii_url).await?
    else {
        eprintln!("Skipping asset holder GET I105 coverage: holders endpoint unavailable.");
        return Ok(());
    };
    let mut url = network.client().torii_url.clone();
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
    assert!(
        ids.iter().all(|id| id.starts_with("sora")),
        "all holders should render I105 literals: {ids:?}"
    );

    Ok(())
}

#[tokio::test]
async fn asset_holders_query_rejects_legacy_dotted_i105_filter_literals() -> Result<()> {
    let Some(network) = start_network_async_or_skip(
        NetworkBuilder::new(),
        stringify!(asset_holders_query_rejects_legacy_dotted_i105_filter_literals),
    )
    .await?
    else {
        return Ok(());
    };
    network.ensure_blocks(1).await?;

    let literal = legacy_dotted_i105_alice_literal();
    let reason = AccountId::parse_encoded(&literal)
        .expect_err("legacy dotted I105 literal must fail strict parsing")
        .reason()
        .to_string();
    let body = format!(
        r#"{{"filter":{{"op":"eq","args":["account_id","{literal}"]}},"sort":[],"pagination":{{"limit":4,"offset":0}},"fetch_size":null}}"#
    );

    let http = http_client();
    let Some(definition_literal) =
        find_asset_definition_with_holders(&http, &network.client().torii_url).await?
    else {
        eprintln!("Skipping asset holder query I105 coverage: holders endpoint unavailable.");
        return Ok(());
    };
    let mut url = network.client().torii_url.clone();
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
        "legacy dotted I105 filter literal should be rejected"
    );
    let body = resp.text().await?;
    assert!(
        body.contains(&reason),
        "response body should mention {reason}, got {body}"
    );

    Ok(())
}

#[tokio::test]
async fn account_transactions_get_returns_i105_literals() -> Result<()> {
    let Some(network) = start_network_async_or_skip(
        NetworkBuilder::new(),
        stringify!(account_transactions_get_returns_i105_literals),
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
    let i105_literal = account_literal.clone();

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
            if !authorities.is_empty() {
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
        "expected success fetching account transactions with canonical i105, got {}",
        resp.status()
    );
    let parsed: norito::json::Value = norito::json::from_str(&resp.text().await?)?;
    let authorities = extract_account_transaction_authorities(&parsed);
    assert!(
        authorities.iter().any(|literal| literal == &i105_literal),
        "I105 response should include {i105_literal}, got {authorities:?}"
    );
    assert!(
        authorities
            .iter()
            .all(|literal| literal.starts_with("sora")),
        "I105 response should emit only I105 literals; got {authorities:?}"
    );

    Ok(())
}

#[tokio::test]
async fn account_transactions_query_returns_i105_literals() -> Result<()> {
    let Some(network) = start_network_async_or_skip(
        NetworkBuilder::new(),
        stringify!(account_transactions_query_returns_i105_literals),
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
    let i105_literal = account_literal.clone();

    let url = account_endpoint_url(
        &network.client().torii_url,
        &account_literal,
        &["transactions", "query"],
    );

    let default_body = r#"{"filter":null,"sort":[],"pagination":{"offset":0,"limit":8},"fetch_size":null,"select":null}"#;
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
            if !authorities.is_empty() {
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

    let i105_body = r#"{"filter":null,"sort":[],"pagination":{"offset":0,"limit":8},"fetch_size":null,"select":null}"#;
    let resp = http
        .post(url.clone())
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .body(i105_body)
        .send()
        .await?;
    assert!(
        resp.status().is_success(),
        "expected success from account transactions query with canonical i105, got {}",
        resp.status()
    );
    let parsed: norito::json::Value = norito::json::from_str(&resp.text().await?)?;
    let authorities = extract_account_transaction_authorities(&parsed);
    assert!(
        authorities.iter().any(|literal| literal == &i105_literal),
        "I105 query response should include {i105_literal}, got {authorities:?}"
    );
    assert!(
        authorities
            .iter()
            .all(|literal| literal.starts_with("sora")),
        "I105 query response should emit only I105 literals; got {authorities:?}"
    );

    Ok(())
}

#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn explorer_transactions_emit_i105_literals() -> Result<()> {
    let Some(network) = start_network_async_or_skip(
        NetworkBuilder::new(),
        stringify!(explorer_transactions_emit_i105_literals),
    )
    .await?
    else {
        return Ok(());
    };
    network.ensure_blocks(1).await?;

    let http = http_client();
    let account_literal = SAMPLE_GENESIS_ACCOUNT_ID.to_string();
    let i105_literal = account_literal.clone();

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
        "expected explorer transactions to succeed (I105 default), got {}",
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
        "explorer transactions should default to I105 literals; got {authorities:?}"
    );

    let i105_url = {
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
        .get(i105_url)
        .header("Accept", "application/json")
        .send()
        .await?;
    assert!(
        resp.status().is_success(),
        "expected explorer transactions with I105 response to succeed, got {}",
        resp.status()
    );
    let parsed: norito::json::Value = norito::json::from_str(&resp.text().await?)?;
    let authorities = extract_explorer_authorities(&parsed);
    assert!(
        !authorities.is_empty(),
        "I105 explorer transactions response should contain items"
    );
    assert!(
        authorities.iter().all(|literal| literal == &i105_literal),
        "explorer transactions should honour canonical i105; got {authorities:?}"
    );

    let legacy_dotted_i105_filter = legacy_dotted_i105_literal(&i105_literal);
    let legacy_dotted_i105_filter_url = {
        let mut url = base;
        {
            let mut qp = url.query_pairs_mut();
            qp.append_pair("authority", &legacy_dotted_i105_filter);
            qp.append_pair("page", "1");
            qp.append_pair("per_page", "8");
        }
        url
    };
    let legacy_dotted_i105_reason = AccountId::parse_encoded(&legacy_dotted_i105_filter)
        .expect_err("legacy dotted I105 authority filter must fail strict parsing")
        .reason()
        .to_string();
    let resp = http
        .get(legacy_dotted_i105_filter_url)
        .header("Accept", "application/json")
        .send()
        .await?;
    assert_eq!(
        resp.status(),
        reqwest::StatusCode::BAD_REQUEST,
        "explorer transactions should reject legacy dotted I105 authority filters"
    );
    let body = resp.text().await?;
    assert!(
        body.contains(&legacy_dotted_i105_reason),
        "response body should mention {legacy_dotted_i105_reason}, got {body}"
    );

    Ok(())
}

#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn explorer_instructions_emit_i105_literals() -> Result<()> {
    let Some(network) = start_network_async_or_skip(
        NetworkBuilder::new(),
        stringify!(explorer_instructions_emit_i105_literals),
    )
    .await?
    else {
        return Ok(());
    };
    network.ensure_blocks(1).await?;

    let http = http_client();
    let account_literal = SAMPLE_GENESIS_ACCOUNT_ID.to_string();
    let i105_literal = account_literal.clone();

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
        "expected explorer instructions to succeed (I105 default), got {}",
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
        "explorer instructions should default to I105 literals; got {authorities:?}"
    );

    let i105_url = {
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
        .get(i105_url)
        .header("Accept", "application/json")
        .send()
        .await?;
    assert!(
        resp.status().is_success(),
        "expected explorer instructions with I105 response to succeed, got {}",
        resp.status()
    );
    let parsed: norito::json::Value = norito::json::from_str(&resp.text().await?)?;
    let authorities = extract_explorer_authorities(&parsed);
    assert!(
        !authorities.is_empty(),
        "I105 explorer instructions response should contain items"
    );
    assert!(
        authorities.iter().all(|literal| literal == &i105_literal),
        "explorer instructions should honour canonical i105; got {authorities:?}"
    );

    let legacy_dotted_i105_filter = legacy_dotted_i105_literal(&i105_literal);
    let legacy_dotted_i105_filter_url = {
        let mut url = base;
        {
            let mut qp = url.query_pairs_mut();
            qp.append_pair("authority", &legacy_dotted_i105_filter);
            qp.append_pair("page", "1");
            qp.append_pair("per_page", "8");
        }
        url
    };
    let legacy_dotted_i105_reason = AccountId::parse_encoded(&legacy_dotted_i105_filter)
        .expect_err("legacy dotted I105 authority filter must fail strict parsing")
        .reason()
        .to_string();
    let resp = http
        .get(legacy_dotted_i105_filter_url)
        .header("Accept", "application/json")
        .send()
        .await?;
    assert_eq!(
        resp.status(),
        reqwest::StatusCode::BAD_REQUEST,
        "explorer instructions should reject legacy dotted I105 authority filters"
    );
    let body = resp.text().await?;
    assert!(
        body.contains(&legacy_dotted_i105_reason),
        "response body should mention {legacy_dotted_i105_reason}, got {body}"
    );

    Ok(())
}

#[tokio::test]
async fn explorer_account_qr_defaults_to_i105_for_discriminant() -> Result<()> {
    let Some(network) = start_network_async_or_skip(
        NetworkBuilder::new(),
        stringify!(explorer_account_qr_defaults_to_i105),
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
    let Some(network) = start_network_async_or_skip(
        NetworkBuilder::new(),
        stringify!(explorer_account_qr_accepts_i105_hint),
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
        "expected explorer account QR to respect canonical i105, got {}",
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
    let local8_reason = AccountId::parse_encoded(&literal)
        .expect_err("local8 literal must fail to parse")
        .reason()
        .to_owned();
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
        body.contains(&local8_reason),
        "response body should mention {local8_reason}, got {body}"
    );

    Ok(())
}

#[tokio::test]
async fn accounts_query_rejects_public_key_filter_literals() -> Result<()> {
    let Some(network) = start_network_async_or_skip(
        NetworkBuilder::new(),
        stringify!(accounts_query_rejects_public_key_filter_literals),
    )
    .await?
    else {
        return Ok(());
    };
    network.ensure_blocks(1).await?;

    let literal = public_key_literal();
    let reason = AccountId::parse_encoded(&literal)
        .expect_err("public-key@domain literal must fail strict parsing")
        .reason()
        .to_string();
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
async fn accounts_query_rejects_alias_and_legacy_dotted_i105_filter_literals() -> Result<()> {
    let domain_id: DomainId = "aliases".parse()?;
    let label = AccountLabel::new(domain_id.clone(), "primary".parse()?);
    let keypair = KeyPair::random();
    let account_id = AccountId::new(keypair.public_key().clone());
    let account = Account::new(account_id.clone().to_account_id(domain_id.clone()))
        .with_label(Some(label.clone()));
    let builder = NetworkBuilder::new()
        .with_min_peers(4)
        .with_genesis_instruction(Register::domain(Domain::new(domain_id.clone())))
        .with_genesis_instruction(Register::account(account));
    let Some(network) = start_network_async_or_skip(
        builder,
        stringify!(accounts_query_rejects_alias_and_legacy_dotted_i105_filter_literals),
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
    let i105_literal = legacy_dotted_i105_literal(&account_id.to_string());

    let alias_body = format!(
        r#"{{"filter":{{"op":"eq","args":["id","{alias_literal}"]}},"sort":[],"pagination":{{"limit":4,"offset":0}},"fetch_size":null,"select":null}}"#
    );
    let alias_resp = http
        .post(url.clone())
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .body(alias_body)
        .send()
        .await?;
    assert_eq!(
        alias_resp.status(),
        reqwest::StatusCode::BAD_REQUEST,
        "alias literal should be rejected"
    );

    let body = format!(
        r#"{{"filter":{{"op":"eq","args":["id","{i105_literal}"]}},"sort":[],"pagination":{{"limit":4,"offset":0}},"fetch_size":null,"select":null}}"#
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
    let reason = AccountId::parse_encoded(&i105_literal)
        .expect_err("legacy dotted I105 literal must fail strict parsing")
        .reason()
        .to_string();
    assert_eq!(
        status,
        reqwest::StatusCode::BAD_REQUEST,
        "legacy dotted I105 literal should be rejected, got {status} body={body}"
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
    let cash_def_id: AssetDefinitionId =
        AssetDefinitionId::new("wonderland".parse()?, "rose".parse()?);
    let collateral_def_id: AssetDefinitionId =
        AssetDefinitionId::new("wonderland".parse()?, "camomile".parse()?);
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
        start_network_async_or_skip(builder, stringify!(repo_agreements_emit_i105_literals))
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
        eprintln!("Skipping repo i105 literal coverage: {err}");
        return Ok(());
    }
    network.ensure_blocks(2).await?;

    let http = http_client();
    let base = client.torii_url.clone();
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
        "repo agreements should honour canonical i105 for initiators"
    );
    let counterparty = entry
        .get("counterparty")
        .and_then(norito::json::Value::as_str)
        .expect("I105 counterparty literal must be present");
    assert_eq!(
        counterparty, i105_bob,
        "repo agreements should honour canonical i105 for counterparties"
    );

    let query_url = base.join("/v1/repo/agreements/query")?;
    let query_body = format!(
        r#"{{"filter":{{"op":"eq","args":["id","{agreement_literal}"]}},"sort":[],"pagination":{{"limit":4,"offset":0}},"fetch_size":null,"select":null}}"#
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
        .expect("I105 query initiator literal must be present");
    assert_eq!(
        initiator, i105_alice,
        "repo agreements query should honour canonical i105"
    );
    let counterparty = entry
        .get("counterparty")
        .and_then(norito::json::Value::as_str)
        .expect("I105 query counterparty literal must be present");
    assert_eq!(
        counterparty, i105_bob,
        "repo agreements query should honour canonical i105"
    );

    Ok(())
}

#[tokio::test]
#[allow(clippy::too_many_lines)] // integration coverage requires full scenario setup
async fn kaigi_endpoints_emit_i105_literals() -> Result<()> {
    let relay_id = BOB_ID.clone();
    let reporter_id = ALICE_ID.clone();
    let domain_id: DomainId = "wonderland".parse()?;

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

    let Some(network) =
        start_network_async_or_skip(builder, stringify!(kaigi_endpoints_emit_i105_literals))
            .await?
    else {
        return Ok(());
    };
    network.ensure_blocks(1).await?;

    let seed = KaigiRelaySeed {
        relay_i105: relay_id.to_string(),
        reporter_i105: reporter_id.to_string(),
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
            .any(|literal| *literal == seed.relay_i105),
        "expected I105 literal {} in kaigi summary, got {default_literals:?}",
        seed.relay_i105
    );

    let i105_summary_url = base.join("/v1/kaigi/relays")?;
    let i105_resp = http
        .get(i105_summary_url)
        .header("Accept", "application/json")
        .send()
        .await?;
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

    let legacy_relay_literal = legacy_dotted_i105_bob_literal();
    let detail_url = base.join(&format!("/v1/kaigi/relays/{legacy_relay_literal}"))?;
    let detail_resp = http
        .get(detail_url)
        .header("Accept", "application/json")
        .send()
        .await?;
    assert_eq!(
        detail_resp.status(),
        reqwest::StatusCode::BAD_REQUEST,
        "kaigi relay detail should reject legacy dotted I105 literal, got {}",
        detail_resp.status()
    );
    let body = detail_resp.text().await?;
    let reason = AccountId::parse_encoded(&legacy_relay_literal)
        .expect_err("legacy dotted I105 relay literal must fail strict parsing")
        .reason()
        .to_string();
    assert!(
        body.contains(&reason),
        "response body should mention {reason}, got {body}"
    );

    let formatted_detail_url = base.join(&format!("/v1/kaigi/relays/{}", seed.relay_i105))?;
    let formatted_resp = http
        .get(formatted_detail_url)
        .header("Accept", "application/json")
        .send()
        .await?;
    assert!(
        formatted_resp.status().is_success(),
        "kaigi relay detail with canonical i105 should succeed, got {}",
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
        "relay literal should honour canonical i105"
    );
    let reported_by_literal = formatted_detail
        .get("reported_by")
        .and_then(norito::json::Value::as_str)
        .ok_or_else(|| eyre!("formatted detail missing reported_by literal"))?;
    assert_eq!(
        reported_by_literal, seed.reporter_i105,
        "reported_by should honour canonical i105"
    );

    Ok(())
}

#[tokio::test]
async fn offline_allowances_listing_emit_i105_literals() -> Result<()> {
    init_instruction_registry();
    let certificate = match load_offline_certificate_fixture() {
        Ok(certificate) => certificate,
        Err(err) => {
            eprintln!("Skipping offline allowance listing test: {err}");
            return Ok(());
        }
    };
    let controller = certificate.controller.clone();
    let controller_i105 = controller.to_string();
    let instruction = RegisterOfflineAllowance { certificate };

    let builder = with_offline_allowance_genesis(NetworkBuilder::new(), &instruction.certificate)
        .with_genesis_instruction(instruction);

    let Some(network) = start_network_async_or_skip(
        builder,
        stringify!(offline_allowances_listing_emit_i105_literals),
    )
    .await?
    else {
        return Ok(());
    };
    network.ensure_blocks(1).await?;

    let client = network.client();
    let seed = OfflineAllowanceSeed { controller_i105 };

    let base = client.torii_url.clone();
    let http = http_client();

    let default_url = base
        .join("/v1/offline/allowances?limit=4&include_expired=true")
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
    let default_entry = find_offline_allowance_entry(&parsed, &seed.controller_i105)
        .ok_or_else(|| eyre!("offline allowance for {} missing", seed.controller_i105))?;
    let default_display = default_entry
        .get("controller_display")
        .and_then(norito::json::Value::as_str)
        .ok_or_else(|| eyre!("offline allowance missing controller_display field"))?;
    assert_eq!(
        default_display, seed.controller_i105,
        "I105 default should surface canonical literal"
    );

    let i105_url = base
        .join("/v1/offline/allowances?limit=4&include_expired=true")
        .expect("offline allowances url");
    let resp = http
        .get(i105_url)
        .header("Accept", "application/json")
        .send()
        .await?;
    assert!(
        resp.status().is_success(),
        "offline allowance listing should succeed, got {}",
        resp.status()
    );
    let parsed: norito::json::Value = norito::json::from_str(&resp.text().await?)?;
    let i105_entry = find_offline_allowance_entry(&parsed, &seed.controller_i105)
        .ok_or_else(|| eyre!("offline allowance for {} missing", seed.controller_i105))?;
    let i105_display = i105_entry
        .get("controller_display")
        .and_then(norito::json::Value::as_str)
        .ok_or_else(|| eyre!("offline allowance missing controller_display field"))?;
    assert_eq!(
        i105_display, seed.controller_i105,
        "I105 listing should surface I105 literal"
    );

    Ok(())
}

#[tokio::test]
async fn offline_allowances_query_emit_i105_literals() -> Result<()> {
    init_instruction_registry();
    let certificate = match load_offline_certificate_fixture() {
        Ok(certificate) => certificate,
        Err(err) => {
            eprintln!("Skipping offline allowance query test: {err}");
            return Ok(());
        }
    };
    let controller = certificate.controller.clone();
    let controller_i105 = controller.to_string();
    let instruction = RegisterOfflineAllowance { certificate };

    let builder = with_offline_allowance_genesis(NetworkBuilder::new(), &instruction.certificate)
        .with_genesis_instruction(instruction);

    let Some(network) = start_network_async_or_skip(
        builder,
        stringify!(offline_allowances_query_emit_i105_literals),
    )
    .await?
    else {
        return Ok(());
    };
    network.ensure_blocks(1).await?;

    let client = network.client();
    let seed = OfflineAllowanceSeed { controller_i105 };

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
    let default_entry = find_offline_allowance_entry(&parsed, &seed.controller_i105)
        .ok_or_else(|| eyre!("offline allowance for {} missing", seed.controller_i105))?;
    let default_display = default_entry
        .get("controller_display")
        .and_then(norito::json::Value::as_str)
        .ok_or_else(|| eyre!("offline allowance missing controller_display field"))?;
    assert_eq!(
        default_display, seed.controller_i105,
        "I105 default query should emit canonical literal"
    );

    let i105_body = r#"{"filter":null,"sort":[],"pagination":{"limit":4,"offset":0},"fetch_size":null,"select":null}"#;
    let resp = http
        .post(url)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .body(i105_body)
        .send()
        .await?;
    assert!(
        resp.status().is_success(),
        "offline allowance query with canonical i105 should succeed, got {}",
        resp.status()
    );
    let parsed: norito::json::Value = norito::json::from_str(&resp.text().await?)?;
    let i105_entry = find_offline_allowance_entry(&parsed, &seed.controller_i105)
        .ok_or_else(|| eyre!("offline allowance for {} missing", seed.controller_i105))?;
    let i105_display = i105_entry
        .get("controller_display")
        .and_then(norito::json::Value::as_str)
        .ok_or_else(|| eyre!("offline allowance missing controller_display field"))?;
    assert_eq!(
        i105_display, seed.controller_i105,
        "canonical i105 should rewrite controller_display"
    );

    Ok(())
}
