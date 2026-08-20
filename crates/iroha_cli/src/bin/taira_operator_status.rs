//! Minimal native client for one protected Taira Sumeragi status read.
//!
//! This binary deliberately avoids a normal client TOML and account signer. The reset controller
//! supplies only an exact loopback Torii URL, the signed genesis NetworkId, and one owner-only
//! operator key file. No mutation or non-operator route is exposed.

use std::{path::PathBuf, time::Duration};

use clap::Parser as _;
use eyre::{Result, WrapErr as _, bail};
use iroha::{
    client::Client,
    config::{Config, DEFAULT_TRANSACTION_NONCE, default_connect_queue_root},
    data_model::{ChainId, NetworkId, account::AccountId, block::BlockHeader},
};
use iroha_config::parameters::{actual::SorafsRolloutPhase, defaults};
use iroha_crypto::{Algorithm, HashOf, KeyPair};
use sorafs_manifest::alias_cache::AliasCachePolicy;
use sorafs_orchestrator::AnonymityPolicy;
use url::Url;

#[path = "../operator_key.rs"]
mod operator_key;

const MIN_TIMEOUT_MS: u64 = 250;
const MAX_TIMEOUT_MS: u64 = 10_000;

#[derive(clap::Parser, Debug)]
#[command(name = "taira_operator_status")]
struct Args {
    /// Exact credential-free loopback Torii root, including the port.
    #[arg(long)]
    torii_url: Url,
    /// Exact signed-genesis NetworkId admitted by the selected validator.
    #[arg(long, value_parser = parse_network_id_literal)]
    network_id: NetworkId,
    /// Absolute owner-only operator private-key file.
    #[arg(long)]
    operator_private_key_file: PathBuf,
    /// Bounded HTTP timeout for this one read.
    #[arg(long, default_value_t = 2_000)]
    timeout_ms: u64,
}

fn parse_network_id_literal(value: &str) -> Result<NetworkId, String> {
    let genesis_hash = norito::json::from_value::<HashOf<BlockHeader>>(
        norito::json::Value::String(value.to_owned()),
    )
    .map_err(|_| "NetworkId must be one canonical genesis hash literal".to_owned())?;
    let canonical = norito::json::to_value(&genesis_hash)
        .map_err(|_| "NetworkId could not be canonically encoded".to_owned())?;
    if canonical.as_str() != Some(value) {
        return Err("NetworkId does not use its canonical literal spelling".to_owned());
    }
    Ok(NetworkId::from_genesis_hash(genesis_hash))
}

fn validate_args(args: &Args) -> Result<()> {
    let url = &args.torii_url;
    if url.scheme() != "http"
        || url.host_str() != Some("127.0.0.1")
        || url.port().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || !matches!(url.path(), "" | "/")
    {
        bail!("--torii-url must be an absolute credential-free http://127.0.0.1:<port>/ root");
    }
    if !(MIN_TIMEOUT_MS..=MAX_TIMEOUT_MS).contains(&args.timeout_ms) {
        bail!("--timeout-ms must be between {MIN_TIMEOUT_MS} and {MAX_TIMEOUT_MS}");
    }
    Ok(())
}

fn alias_cache_policy() -> AliasCachePolicy {
    AliasCachePolicy::new(
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

fn operator_client(args: &Args, operator_key_pair: KeyPair) -> Result<Client> {
    // This deterministic account key is deliberately inert: the only exposed request is signed
    // by `operator_key_pair`, and the reset controller never submits an account-authenticated
    // request or transaction through this binary.
    let account_key_pair = KeyPair::try_from_seed(
        b"iroha:taira:operator-status:inert-account:v1".to_vec(),
        Algorithm::Ed25519,
    )
    .wrap_err("failed to derive inert operator-status account context")?;
    let account = AccountId::new(account_key_pair.public_key().clone());
    let timeout = Duration::from_millis(args.timeout_ms);
    let config = Config {
        chain: ChainId::from("taira"),
        network_id: args.network_id.clone(),
        account,
        account_chain_discriminant: defaults::common::chain_discriminant(),
        key_pair: account_key_pair,
        basic_auth: None,
        torii_api_url: args.torii_url.clone(),
        torii_request_timeout: timeout,
        transaction_ttl: timeout,
        transaction_status_timeout: timeout,
        transaction_add_nonce: DEFAULT_TRANSACTION_NONCE,
        connect_queue_root: default_connect_queue_root(),
        soracloud_http_witness_file: None,
        sorafs_alias_cache: alias_cache_policy(),
        sorafs_anonymity_policy: AnonymityPolicy::GuardPq,
        sorafs_rollout_phase: SorafsRolloutPhase::Default,
    };
    let mut client = Client::new(config);
    client.set_operator_key_pair(operator_key_pair);
    Ok(client)
}

fn run(args: Args) -> Result<()> {
    validate_args(&args)?;
    let operator_key_pair = operator_key::load_operator_key_pair(&args.operator_private_key_file)
        .wrap_err("failed to load runtime operator signing key")?;
    let status = operator_client(&args, operator_key_pair)?
        .get_sumeragi_status_json()
        .wrap_err("protected Sumeragi status read failed")?;
    let rendered = norito::json::to_json(&status)
        .wrap_err("failed to encode protected Sumeragi status JSON")?;
    println!("{rendered}");
    Ok(())
}

fn main() {
    if let Err(error) = run(Args::parse()) {
        eprintln!("error: {error:#}");
        std::process::exit(70);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NETWORK_ID: &str =
        "hash:82531CE8EAE8BFF6BEECA4698BFD13A3BC8BEC5F0EE0D23D428C97FC17AB0F3B#3E94";

    fn parse_url(value: &str) -> Args {
        Args::try_parse_from([
            "taira_operator_status",
            "--torii-url",
            value,
            "--network-id",
            NETWORK_ID,
            "--operator-private-key-file",
            "/private/operator.key",
        ])
        .expect("parse fixture arguments")
    }

    #[test]
    fn accepts_only_one_exact_loopback_torii_root() {
        let args = parse_url("http://127.0.0.1:29080/");
        validate_args(&args).expect("accept exact loopback root");
        assert_eq!(args.timeout_ms, 2_000);
        for rejected in [
            "https://127.0.0.1:29080/",
            "http://localhost:29080/",
            "http://user:secret@127.0.0.1:29080/",
            "http://127.0.0.1/",
            "http://127.0.0.1:29080/v1/sumeragi/status",
            "http://127.0.0.1:29080/?query=1",
            "http://127.0.0.1:29080/#fragment",
        ] {
            assert!(
                validate_args(&parse_url(rejected)).is_err(),
                "accepted forbidden Torii URL {rejected}"
            );
        }
    }

    #[test]
    fn rejects_unbounded_timeouts() {
        for timeout_ms in [0, MIN_TIMEOUT_MS - 1, MAX_TIMEOUT_MS + 1] {
            let mut args = parse_url("http://127.0.0.1:29080/");
            args.timeout_ms = timeout_ms;
            assert!(validate_args(&args).is_err());
        }
    }
}
