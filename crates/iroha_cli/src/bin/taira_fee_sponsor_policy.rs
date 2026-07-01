//! Upsert the public Taira default Nexus fee sponsor policy.

use std::{path::PathBuf, str::FromStr as _, time::Duration};

use clap::Parser;
use eyre::{Context, Result, bail};
use iroha::{
    client::Client,
    config::{self, AnonymityPolicy, Config},
    crypto::{ExposedPrivateKey, KeyPair},
    data_model::{
        ChainId,
        account::{AccountAddress, AccountId},
        isi::nexus::UpsertFeeSponsorPolicy,
        metadata::Metadata,
        name::Name,
        nexus::{FeeSponsorPolicy, FeeSponsorPolicyId, FeeSponsorRule, FeeSponsorRuleEffect},
    },
};
use iroha_config::parameters::{actual::SorafsRolloutPhase, defaults};
use iroha_primitives::json::Json;
use toml::Value;
use url::Url;

#[derive(Debug, Parser)]
#[command(
    about = "Create or replace the Taira default Nexus fee sponsor policy",
    version
)]
struct Args {
    #[arg(long, default_value = "https://taira.sora.org")]
    torii_url: Url,
    #[arg(long, default_value = "809574f5-fee7-5e69-bfcf-52451e42d50f")]
    chain_id: ChainId,
    #[arg(long, default_value_t = 369)]
    chain_discriminant: u16,
    #[arg(long, default_value = "default")]
    policy: Name,
    #[arg(long)]
    sponsor_account: Option<String>,
    #[arg(long, default_value = "defaults/kagami/iroha3-taira/config.toml")]
    profile_config: PathBuf,
    #[arg(long, default_value_t = 600)]
    status_timeout_secs: u64,
    #[arg(long, default_value = "6TEAJqbb8oEPmLncoNiMRbLEK6tw")]
    gas_asset_id: String,
    #[arg(long, default_value_t = 10_000_000)]
    gas_limit: u64,
}

fn table<'a>(value: &'a Value, key: &str) -> Result<&'a toml::value::Table> {
    value
        .get(key)
        .and_then(Value::as_table)
        .ok_or_else(|| eyre::eyre!("missing [{key}] table"))
}

fn string_at<'a>(table: &'a toml::value::Table, key: &str) -> Result<&'a str> {
    table
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| eyre::eyre!("missing `{key}`"))
}

fn taira_profile_signer(path: &PathBuf) -> Result<(String, String)> {
    let raw = std::fs::read_to_string(path)
        .wrap_err_with(|| format!("read Taira profile {}", path.display()))?;
    let value = toml::from_str::<Value>(&raw).wrap_err("parse Taira profile TOML")?;
    let torii = table(&value, "torii")?;
    let onboarding_value = torii
        .get("onboarding")
        .ok_or_else(|| eyre::eyre!("missing [torii.onboarding] table"))?;
    let onboarding = onboarding_value
        .as_table()
        .ok_or_else(|| eyre::eyre!("invalid [torii.onboarding] table"))?;
    let authority = string_at(onboarding, "authority")?.to_owned();
    let private_key = string_at(onboarding, "private_key")?.to_owned();
    Ok((authority, private_key))
}

fn parse_taira_account(account: &str, discriminant: u16) -> Result<AccountId> {
    if account.contains('@') {
        bail!(
            "expected an encoded Taira account address, got unsupported account literal `{account}`"
        );
    }
    AccountAddress::parse_encoded(account, Some(discriminant))
        .and_then(|address| address.to_account_id())
        .wrap_err_with(|| format!("parse Taira account address `{account}`"))
}

fn default_alias_cache_policy() -> sorafs_manifest::alias_cache::AliasCachePolicy {
    sorafs_manifest::alias_cache::AliasCachePolicy::new(
        Duration::from_secs(defaults::torii::SORAFS_ALIAS_POSITIVE_TTL_SECS),
        Duration::from_secs(defaults::torii::SORAFS_ALIAS_REFRESH_WINDOW_SECS),
        Duration::from_secs(defaults::torii::SORAFS_ALIAS_HARD_EXPIRY_SECS),
        Duration::from_secs(defaults::torii::SORAFS_ALIAS_NEGATIVE_TTL_SECS),
        Duration::from_secs(defaults::torii::SORAFS_ALIAS_REVOCATION_TTL_SECS),
        Duration::from_secs(defaults::torii::SORAFS_ALIAS_ROTATION_MAX_AGE_SECS),
        Duration::from_secs(defaults::torii::SORAFS_ALIAS_SUCCESSOR_GRACE_SECS),
        Duration::from_secs(defaults::torii::SORAFS_ALIAS_GOVERNANCE_GRACE_SECS),
    )
}

fn transaction_metadata(gas_asset_id: &str, gas_limit: u64) -> Result<Metadata> {
    let mut metadata = Metadata::default();
    metadata.insert(
        Name::from_str("gas_asset_id")?,
        Json::new(gas_asset_id.trim().to_owned()),
    );
    iroha::data_model::transaction::insert_transaction_gas_limit(&mut metadata, gas_limit);
    Ok(metadata)
}

fn main() -> Result<()> {
    let args = Args::parse();
    let (profile_account, profile_private_key) = taira_profile_signer(&args.profile_config)?;
    let sponsor_literal = args.sponsor_account.as_deref().unwrap_or(&profile_account);
    let sponsor = parse_taira_account(sponsor_literal, args.chain_discriminant)?;
    let private_key = profile_private_key
        .parse::<ExposedPrivateKey>()
        .wrap_err("parse profile private key")?
        .0;
    let key_pair = KeyPair::from_private_key(private_key).wrap_err("derive key pair")?;
    let signer = AccountId::new(key_pair.public_key().clone());
    if signer != sponsor {
        bail!("profile signer account `{signer}` does not match sponsor `{sponsor}`");
    }

    let client = Client::new(Config {
        chain: args.chain_id,
        account: sponsor.clone(),
        account_chain_discriminant: args.chain_discriminant,
        key_pair,
        basic_auth: None,
        torii_api_url: args.torii_url,
        torii_api_version: config::default_torii_api_version(),
        torii_api_min_proof_version: config::DEFAULT_TORII_API_MIN_PROOF_VERSION.to_owned(),
        torii_request_timeout: config::DEFAULT_TORII_REQUEST_TIMEOUT,
        transaction_ttl: Duration::from_secs(900),
        transaction_status_timeout: Duration::from_secs(args.status_timeout_secs),
        transaction_add_nonce: true,
        connect_queue_root: config::default_connect_queue_root(),
        soracloud_http_witness_file: None,
        sorafs_alias_cache: default_alias_cache_policy(),
        sorafs_anonymity_policy: AnonymityPolicy::GuardPq,
        sorafs_rollout_phase: SorafsRolloutPhase::default(),
    });

    let mut policy = FeeSponsorPolicy::new(FeeSponsorPolicyId::new(sponsor.clone(), args.policy));
    policy.enabled = true;
    policy
        .rules
        .push(FeeSponsorRule::new(FeeSponsorRuleEffect::Allow));

    let metadata = transaction_metadata(&args.gas_asset_id, args.gas_limit)?;
    let hash = client.submit_blocking_with_metadata(UpsertFeeSponsorPolicy { policy }, metadata)?;
    println!("{hash}");
    Ok(())
}
