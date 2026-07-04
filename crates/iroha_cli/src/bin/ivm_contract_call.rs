//! Submit a deployed IVM contract call as a raw signed transaction.
#![allow(clippy::too_many_arguments)]

use std::{
    fs,
    path::{Path, PathBuf},
    str::FromStr,
    time::{Duration, Instant},
};

use clap::Parser;
use eyre::{Result, WrapErr as _, eyre};
use iroha::{
    account_address::parse_account_address,
    client::Client,
    config::{self, Config},
    data_model::{
        metadata::Metadata,
        name::Name,
        prelude::*,
        smart_contract::{ContractAddress, ContractAlias},
        transaction::{Executable, TransactionBuilder, executable::ContractInvocation},
    },
};
use iroha_config::parameters::{
    actual::SorafsRolloutPhase,
    defaults::{
        sorafs::gateway::{DEFAULT_ANONYMITY_POLICY, DEFAULT_ROLLOUT_PHASE},
        torii,
    },
};
use iroha_crypto::{KeyPair, PrivateKey};
use iroha_primitives::json::Json;
use sorafs_manifest::alias_cache::AliasCachePolicy;
use sorafs_orchestrator::AnonymityPolicy;
use url::Url;

const DEFAULT_CHAIN_DISCRIMINANT_TAIRA: u16 = 369;
const DEFAULT_IVM_GAS_LIMIT: u64 = 1_000_000;

#[derive(Parser, Debug)]
struct Args {
    #[arg(long)]
    torii_url: String,
    #[arg(long)]
    chain_id: String,
    #[arg(long)]
    authority: String,
    #[arg(long)]
    private_key: String,
    #[arg(long)]
    contract_address: Option<String>,
    #[arg(long)]
    contract_alias: Option<String>,
    #[arg(long, default_value = "main")]
    entrypoint: String,
    #[arg(long)]
    payload_json: Option<String>,
    #[arg(long)]
    payload_file: Option<PathBuf>,
    #[arg(long, default_value_t = DEFAULT_CHAIN_DISCRIMINANT_TAIRA)]
    chain_discriminant: u16,
    #[arg(long)]
    gas_asset_id: Option<String>,
    #[arg(long)]
    fee_sponsor: Option<String>,
    #[arg(long = "gov-manifest-approver", value_name = "ACCOUNT")]
    gov_manifest_approvers: Vec<String>,
    #[arg(long, default_value_t = DEFAULT_IVM_GAS_LIMIT)]
    gas_limit: u64,
    #[arg(long, default_value_t = 300_000)]
    status_timeout_ms: u64,
    #[arg(long)]
    transaction_ttl_ms: Option<u64>,
    #[arg(long, default_value_t = 300_000)]
    torii_request_timeout_ms: u64,
}

fn default_alias_cache_policy() -> AliasCachePolicy {
    AliasCachePolicy::new(
        Duration::from_secs(torii::SORAFS_ALIAS_POSITIVE_TTL_SECS),
        Duration::from_secs(torii::SORAFS_ALIAS_REFRESH_WINDOW_SECS),
        Duration::from_secs(torii::SORAFS_ALIAS_HARD_EXPIRY_SECS),
        Duration::from_secs(torii::SORAFS_ALIAS_NEGATIVE_TTL_SECS),
        Duration::from_secs(torii::SORAFS_ALIAS_REVOCATION_TTL_SECS),
        Duration::from_secs(torii::SORAFS_ALIAS_ROTATION_MAX_AGE_SECS),
        Duration::from_secs(torii::SORAFS_ALIAS_SUCCESSOR_GRACE_SECS),
        Duration::from_secs(torii::SORAFS_ALIAS_GOVERNANCE_GRACE_SECS),
    )
}

fn default_anonymity_policy() -> AnonymityPolicy {
    AnonymityPolicy::parse(DEFAULT_ANONYMITY_POLICY).unwrap_or(AnonymityPolicy::GuardPq)
}

fn default_rollout_phase() -> SorafsRolloutPhase {
    SorafsRolloutPhase::parse(DEFAULT_ROLLOUT_PHASE).unwrap_or_default()
}

fn make_client(
    torii_url: &str,
    chain_id: &str,
    authority: AccountId,
    chain_discriminant: u16,
    key_pair: KeyPair,
    status_timeout_ms: u64,
    transaction_ttl_ms: Option<u64>,
    torii_request_timeout_ms: u64,
) -> Result<Client> {
    let config = Config {
        chain: ChainId::from(chain_id),
        account: authority,
        account_chain_discriminant: chain_discriminant,
        key_pair,
        basic_auth: None,
        torii_api_url: Url::parse(torii_url).wrap_err("invalid --torii-url")?,
        torii_api_version: config::default_torii_api_version(),
        torii_api_min_proof_version: config::DEFAULT_TORII_API_MIN_PROOF_VERSION.to_string(),
        torii_request_timeout: Duration::from_millis(torii_request_timeout_ms),
        transaction_ttl: transaction_ttl_ms.map_or(
            config::DEFAULT_TRANSACTION_TIME_TO_LIVE,
            Duration::from_millis,
        ),
        transaction_status_timeout: Duration::from_millis(status_timeout_ms),
        transaction_add_nonce: false,
        connect_queue_root: config::default_connect_queue_root(),
        soracloud_http_witness_file: None,
        sorafs_alias_cache: default_alias_cache_policy(),
        sorafs_anonymity_policy: default_anonymity_policy(),
        sorafs_rollout_phase: default_rollout_phase(),
    };
    Ok(Client::new(config))
}

fn insert_string_metadata(
    metadata: &mut Metadata,
    key: &str,
    value: impl Into<String>,
) -> Result<()> {
    metadata.insert(Name::from_str(key)?, Json::new(value.into()));
    Ok(())
}

fn insert_gas_asset_id(metadata: &mut Metadata, gas_asset_id: Option<&str>) -> Result<()> {
    if let Some(asset_id) = gas_asset_id.filter(|value| !value.trim().is_empty()) {
        insert_string_metadata(metadata, "gas_asset_id", asset_id.trim().to_owned())?;
    }
    Ok(())
}

fn insert_gov_manifest_approvers(metadata: &mut Metadata, approvers: &[String]) -> Result<()> {
    let mut accounts = Vec::new();
    for (index, raw) in approvers.iter().enumerate() {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(eyre!("--gov-manifest-approver[{index}] must not be blank"));
        }
        accounts.push(trimmed.to_owned());
    }
    if !accounts.is_empty() {
        metadata.insert(
            Name::from_str("gov_manifest_approvers")?,
            Json::new(accounts),
        );
    }
    Ok(())
}

fn parse_payload(payload_json: Option<&str>, payload_file: Option<&Path>) -> Result<Option<Json>> {
    match (payload_json, payload_file) {
        (Some(raw), None) => norito::json::from_str::<norito::json::Value>(raw)
            .map(Json::new)
            .map(Some)
            .wrap_err("invalid --payload-json"),
        (None, Some(path)) => {
            let contents =
                fs::read_to_string(path).wrap_err_with(|| format!("read {}", path.display()))?;
            norito::json::from_str::<norito::json::Value>(&contents)
                .map(Json::new)
                .map(Some)
                .wrap_err_with(|| format!("invalid JSON in {}", path.display()))
        }
        (None, None) => Ok(None),
        (Some(_), Some(_)) => Err(eyre!(
            "--payload-json and --payload-file are mutually exclusive"
        )),
    }
}

fn resolve_contract_target(
    client: &Client,
    contract_address: Option<&str>,
    contract_alias: Option<&str>,
    alias_resolve_timeout_ms: u64,
) -> Result<(ContractAddress, Option<ContractAlias>)> {
    match (contract_address, contract_alias) {
        (Some(raw), None) => Ok((
            raw.parse()
                .wrap_err("invalid --contract-address canonical literal")?,
            None,
        )),
        (None, Some(raw_alias)) => {
            let alias: ContractAlias = raw_alias.parse().wrap_err("invalid --contract-alias")?;
            let deadline = Instant::now() + Duration::from_millis(alias_resolve_timeout_ms);
            let body = loop {
                let response = client
                    .post_contract_alias_resolve(&alias)
                    .wrap_err("failed to call `/v1/contracts/aliases/resolve`")?;
                let status = response.status();
                let body = response.into_body();
                if status.as_u16() == 200 {
                    break body;
                }
                if status.as_u16() == 404 && Instant::now() < deadline {
                    std::thread::sleep(Duration::from_secs(1));
                    continue;
                }
                if status.as_u16() == 404 {
                    return Err(eyre!("contract alias `{alias}` not found"));
                }
                return Err(eyre!(
                    "contract alias resolve request failed with HTTP {}: {}",
                    status,
                    std::str::from_utf8(&body).unwrap_or("")
                ));
            };
            let value: norito::json::Value =
                norito::json::from_slice(&body).wrap_err("decode contract alias response")?;
            let resolved = value
                .get("contract_address")
                .and_then(norito::json::Value::as_str)
                .ok_or_else(|| eyre!("contract alias response missing `contract_address`"))?;
            Ok((
                resolved
                    .parse()
                    .wrap_err("resolved contract address is invalid")?,
                Some(alias),
            ))
        }
        (Some(_), Some(_)) | (None, None) => Err(eyre!(
            "provide exactly one contract target via --contract-address or --contract-alias"
        )),
    }
}

fn contract_call_metadata(
    contract_address: &ContractAddress,
    contract_alias: Option<&ContractAlias>,
    entrypoint: &str,
    payload: Option<&Json>,
    gas_asset_id: Option<&str>,
    fee_sponsor: Option<&str>,
    gas_limit: u64,
    gov_manifest_approvers: &[String],
) -> Result<Metadata> {
    let mut metadata = Metadata::default();
    insert_string_metadata(
        &mut metadata,
        "contract_address",
        contract_address.to_string(),
    )?;
    if let Some(alias) = contract_alias {
        insert_string_metadata(&mut metadata, "contract_alias", alias.to_string())?;
    }
    insert_string_metadata(&mut metadata, "contract_entrypoint", entrypoint.to_owned())?;
    if let Some(payload) = payload {
        metadata.insert(Name::from_str("contract_payload")?, payload.clone());
    }
    insert_gas_asset_id(&mut metadata, gas_asset_id)?;
    if let Some(sponsor) = fee_sponsor.filter(|value| !value.trim().is_empty()) {
        insert_string_metadata(&mut metadata, "fee_sponsor", sponsor.trim().to_owned())?;
    }
    metadata.insert(Name::from_str("gas_limit")?, Json::new(gas_limit));
    insert_gov_manifest_approvers(&mut metadata, gov_manifest_approvers)?;
    Ok(metadata)
}

fn sign_contract_call_transaction(
    chain_id: &ChainId,
    authority: &AccountId,
    private_key: &PrivateKey,
    transaction_ttl: Option<Duration>,
    metadata: Metadata,
    contract_address: ContractAddress,
    entrypoint: String,
    payload: Option<Json>,
) -> Result<SignedTransaction> {
    let mut builder = TransactionBuilder::new(chain_id.clone(), authority.clone());
    if let Some(transaction_ttl) = transaction_ttl {
        builder.set_ttl(transaction_ttl);
    }
    builder
        .with_metadata(metadata)
        .with_executable(Executable::ContractCall(ContractInvocation {
            contract_address,
            entrypoint,
            payload,
        }))
        .try_sign(private_key)
        .wrap_err("failed to sign contract call transaction")
}

fn payload_digest_hex(payload: Option<&Json>) -> String {
    let payload_json = payload.map_or("", |payload| payload.get().as_str());
    hex::encode(blake3::hash(payload_json.as_bytes()).as_bytes())
}

fn main() -> Result<()> {
    let args = Args::parse();

    let authority = parse_account_address(&args.authority, Some(args.chain_discriminant))
        .wrap_err("failed to parse --authority as canonical account address")?
        .address
        .to_account_id()
        .map_err(|err| eyre!(err.to_string()))
        .wrap_err("failed to decode --authority")?;
    let private_key: PrivateKey = args
        .private_key
        .parse()
        .wrap_err("failed to parse --private-key")?;
    let key_pair = KeyPair::from(private_key.clone());
    let client = make_client(
        &args.torii_url,
        &args.chain_id,
        authority.clone(),
        args.chain_discriminant,
        key_pair,
        args.status_timeout_ms,
        args.transaction_ttl_ms,
        args.torii_request_timeout_ms,
    )?;
    let (contract_address, contract_alias) = resolve_contract_target(
        &client,
        args.contract_address.as_deref(),
        args.contract_alias.as_deref(),
        args.status_timeout_ms,
    )?;
    let payload = parse_payload(args.payload_json.as_deref(), args.payload_file.as_deref())?;
    let payload_digest_hex = payload_digest_hex(payload.as_ref());
    let transaction_ttl = args.transaction_ttl_ms.map(Duration::from_millis);
    let metadata = contract_call_metadata(
        &contract_address,
        contract_alias.as_ref(),
        &args.entrypoint,
        payload.as_ref(),
        args.gas_asset_id.as_deref(),
        args.fee_sponsor.as_deref(),
        args.gas_limit,
        &args.gov_manifest_approvers,
    )?;
    let tx = sign_contract_call_transaction(
        &client.chain,
        &authority,
        &private_key,
        transaction_ttl,
        metadata,
        contract_address.clone(),
        args.entrypoint.clone(),
        payload,
    )?;
    let tx_hash = tx.hash();
    let entrypoint_hash = tx.hash_as_entrypoint();
    client.submit_transaction_blocking(&tx)?;

    let operation_receipt = norito::json!({
        "operation_kind": ("contract_call"),
        "status": ("committed"),
        "transport": ("ivm-contract-call-helper"),
        "dataspace": (""),
        "contract_alias": (contract_alias.as_ref().map(ToString::to_string)),
        "contract_address": (contract_address.to_string()),
        "tx_hash_hex": (tx_hash.to_string()),
        "entrypoint": (args.entrypoint.clone()),
        "entrypoint_hash_hex": (entrypoint_hash.to_string()),
        "gas_limit": (args.gas_limit),
        "gas_asset_id": (args.gas_asset_id),
        "fee_sponsor": (args.fee_sponsor),
        "payload_digest_hex": (payload_digest_hex),
    });
    let result = norito::json!({
        "ok": true,
        "submitted": true,
        "torii_url": (args.torii_url),
        "chain_id": (args.chain_id),
        "authority": (authority),
        "contract_address": (contract_address),
        "contract_alias": (contract_alias),
        "entrypoint": (args.entrypoint.clone()),
        "tx_hash_hex": (tx_hash),
        "entrypoint_hash_hex": (entrypoint_hash),
        "operation_receipt": (operation_receipt),
        "terminal_kind": ("Committed"),
        "final": (norito::json!({
            "kind": ("Committed"),
            "hash": (tx_hash),
        })),
    });
    println!("{}", norito::json::to_json_pretty(&result)?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payload_digest_hex_hashes_empty_payload_when_absent() {
        assert_eq!(
            payload_digest_hex(None),
            hex::encode(blake3::hash(b"").as_bytes())
        );
    }

    #[test]
    fn payload_digest_hex_hashes_json_payload_contents() {
        let payload = Json::new(norito::json!({"action": "call"}));

        assert_eq!(
            payload_digest_hex(Some(&payload)),
            hex::encode(blake3::hash(payload.get().as_bytes()).as_bytes())
        );
    }
}
