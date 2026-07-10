//! Encode governance instructions and submit consensus-governed helper transactions.

use std::{
    fs,
    path::{Path, PathBuf},
    str::FromStr,
};

use clap::{Parser, Subcommand};
use eyre::{Result, WrapErr as _, eyre};
use iroha::{
    account_address::parse_account_address,
    client::{Client, TransactionWaitOptions, TransactionWaitTerminalStatus},
    config::{Config, LoadPath},
    data_model::{
        isi::{
            InstructionBox,
            bridge::{RecordSccpMessage, SccpRouteManifest, UpsertSccpRouteManifest},
            decode_instruction_from_pair,
            governance::RegisterCitizen,
            verifying_keys,
        },
        metadata::Metadata,
        name::Name,
        proof::VerifyingKeyId,
        transaction::{SignedTransaction, TransactionBuilder},
    },
};
use iroha_primitives::json::Json;
use iroha_sccp::{
    SccpLaneIdV1, SccpNetworkV1, SccpOutboundMessageContextV1, SccpPayloadV1, TransferPayloadV1,
    canonical_sccp_payload_bytes, hub_commitment_from_sccp_payload, verify_sccp_payload_structure,
};

const DEFAULT_LEDGER_GAS_LIMIT: u64 = 2_000_000;

#[derive(Parser, Debug)]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Encode a `RegisterCitizen` instruction.
    RegisterCitizen {
        #[arg(long)]
        owner: String,
        #[arg(long)]
        amount: u128,
        #[arg(long, default_value_t = 369)]
        chain_discriminant: u16,
    },
    /// Wrap an app-api `payload_hex` field into tx-stdin JSON.
    WrapPayloadHex {
        #[arg(long)]
        wire_id: String,
        #[arg(long)]
        payload_hex: String,
    },
    /// Encode a `RecordSccpMessage` instruction from an SCCP transfer payload.
    RecordSccpTransfer {
        /// Canonical exact SORA source profile, such as `sora-nexus`.
        #[arg(long)]
        source_profile: String,
        /// Canonical exact external destination profile, such as `ethereum-mainnet`.
        #[arg(long)]
        target_profile: String,
        /// Governed destination binding hash active for this message.
        #[arg(long)]
        destination_binding_hash: String,
        #[arg(long)]
        nonce: u64,
        #[arg(long)]
        asset_home_domain: u32,
        #[arg(long)]
        asset_id_codec: u8,
        #[arg(long)]
        asset_id: String,
        #[arg(long)]
        amount: u128,
        #[arg(long)]
        sender_codec: u8,
        #[arg(long)]
        sender: String,
        #[arg(long)]
        recipient_codec: u8,
        #[arg(long)]
        recipient: String,
        #[arg(long)]
        route_id_codec: u8,
        #[arg(long)]
        route_id: String,
    },
    /// Ensure the canonical Halo2 IPA `ivm-execution-v1` verifying key is registered.
    EnsureIvmExecutionVk {
        #[arg(long)]
        config: PathBuf,
        #[arg(long, default_value = "ivm_execution")]
        vk_name: String,
        #[arg(long)]
        gas_asset_id: Option<String>,
        #[arg(long, default_value_t = DEFAULT_LEDGER_GAS_LIMIT)]
        gas_limit: u64,
    },
    /// Publish an on-chain SCCP route manifest from a route upsert JSON artifact.
    PublishSccpRouteManifest {
        #[arg(long)]
        config: PathBuf,
        #[arg(long)]
        manifest: PathBuf,
        #[arg(long)]
        gas_asset_id: Option<String>,
        #[arg(long, default_value_t = DEFAULT_LEDGER_GAS_LIMIT)]
        gas_limit: u64,
        #[arg(long)]
        expected_route_id: Option<String>,
        #[arg(long)]
        expected_asset_key: Option<String>,
    },
}

fn print_tx_stdin_json(bytes: &[u8]) {
    use base64::{Engine as _, engine::general_purpose::STANDARD};

    let encoded = STANDARD.encode(bytes);
    println!("[\"{encoded}\"]");
}

fn print_json_value(value: &norito::json::Value) -> Result<()> {
    println!("{}", norito::json::to_string(value)?);
    Ok(())
}

fn insert_string_metadata(
    metadata: &mut Metadata,
    key: &str,
    value: impl Into<String>,
) -> Result<()> {
    metadata.insert(Name::from_str(key)?, Json::new(value.into()));
    Ok(())
}

fn tx_metadata(gas_asset_id: Option<&str>, gas_limit: u64) -> Result<Metadata> {
    let mut metadata = Metadata::default();
    if let Some(asset_id) = gas_asset_id.filter(|value| !value.trim().is_empty()) {
        insert_string_metadata(&mut metadata, "gas_asset_id", asset_id.trim().to_owned())?;
    }
    iroha::data_model::transaction::insert_transaction_gas_limit(&mut metadata, gas_limit);
    Ok(metadata)
}

fn sccp_route_manifest_value_from_artifact(
    value: norito::json::Value,
) -> Result<norito::json::Value> {
    let Some(object) = value.as_object() else {
        return Err(eyre!("SCCP route manifest artifact must be a JSON object"));
    };

    if object.contains_key("route_id") {
        return Ok(value);
    }

    if let Some(manifest) = object.get("manifest") {
        return Ok(manifest.clone());
    }

    let Some(instruction) = object
        .get("instruction")
        .and_then(norito::json::Value::as_object)
    else {
        return Err(eyre!(
            "SCCP route manifest artifact must contain `route_id`, `manifest`, or `instruction.UpsertSccpRouteManifest.manifest`"
        ));
    };
    let Some(upsert) = instruction
        .get("UpsertSccpRouteManifest")
        .and_then(norito::json::Value::as_object)
    else {
        return Err(eyre!(
            "SCCP route manifest artifact missing `instruction.UpsertSccpRouteManifest`"
        ));
    };
    upsert
        .get("manifest")
        .cloned()
        .ok_or_else(|| eyre!("SCCP route upsert artifact missing `manifest`"))
}

fn read_sccp_route_manifest_artifact(path: &Path) -> Result<SccpRouteManifest> {
    let raw = fs::read_to_string(path)
        .wrap_err_with(|| format!("failed to read SCCP route manifest `{}`", path.display()))?;
    let value: norito::json::Value =
        norito::json::from_str(&raw).wrap_err("failed to parse SCCP route manifest JSON")?;
    let manifest_value = sccp_route_manifest_value_from_artifact(value)?;
    norito::json::from_value(manifest_value).wrap_err("failed to decode SCCP route manifest")
}

fn load_config(path: &Path) -> Result<Config> {
    Config::load(LoadPath::Explicit(path.to_path_buf())).map_err(|report| {
        eyre!(
            "failed to load client config `{}`: {report}",
            path.display()
        )
    })
}

fn ivm_execution_vk_id(name: &str) -> VerifyingKeyId {
    VerifyingKeyId::new(iroha_core::zk::ZK_BACKEND_HALO2_IPA, name)
}

fn existing_compatible_ivm_execution_vk(client: &Client) -> Result<Option<VerifyingKeyId>> {
    let list = client.get_zk_vk_list_json()?;
    let Some(items) = list.as_array() else {
        return Ok(None);
    };

    for item in items {
        let id = item.get("id").and_then(norito::json::Value::as_object);
        let record = item.get("record").and_then(norito::json::Value::as_object);
        let Some(id) = id else {
            continue;
        };
        let Some(record) = record else {
            continue;
        };
        let backend = id
            .get("backend")
            .and_then(norito::json::Value::as_str)
            .unwrap_or_default();
        let name = id
            .get("name")
            .and_then(norito::json::Value::as_str)
            .unwrap_or_default();
        let status = record
            .get("status")
            .and_then(norito::json::Value::as_str)
            .unwrap_or_default();
        let circuit_id = record
            .get("circuit_id")
            .and_then(norito::json::Value::as_str)
            .unwrap_or_default();
        let gas_schedule_id = record
            .get("gas_schedule_id")
            .and_then(norito::json::Value::as_str)
            .unwrap_or_default();
        if backend == iroha_core::zk::ZK_BACKEND_HALO2_IPA
            && !name.is_empty()
            && status == "Active"
            && circuit_id == iroha_core::zk::IVM_EXECUTION_V1_CIRCUIT_ID
            && !gas_schedule_id.is_empty()
        {
            return Ok(Some(VerifyingKeyId::new(backend, name)));
        }
    }

    Ok(None)
}

fn sign_governance_transaction(
    transaction: TransactionBuilder,
    config: &Config,
    context: &'static str,
) -> Result<SignedTransaction> {
    transaction
        .try_sign(config.key_pair.private_key())
        .wrap_err(context)
}

fn ensure_ivm_execution_vk(
    client: &Client,
    config: &Config,
    vk_name: &str,
    gas_asset_id: Option<&str>,
    gas_limit: u64,
) -> Result<VerifyingKeyId> {
    let id = ivm_execution_vk_id(vk_name);
    match client.get_zk_vk_json(id.backend.as_str(), &id.name) {
        Ok(_) => return Ok(id),
        Err(err) if err.to_string().contains("HTTP status: 404") => {}
        Err(err) => return Err(err).wrap_err("failed to query existing IVM execution VK"),
    }
    if let Some(existing) = existing_compatible_ivm_execution_vk(client)
        .wrap_err("failed to list existing IVM execution VKs")?
    {
        eprintln!("ivm_execution_vk_existing={}", existing.name);
        return Ok(existing);
    }

    let record = iroha_core::zk::halo2_ipa_ivm_execution_vk_record("core", 1)
        .map_err(|err| eyre!("failed to build ivm-execution-v1 VK record: {err}"))?;
    let metadata = tx_metadata(gas_asset_id, gas_limit)?;
    let tx = TransactionBuilder::new(config.chain.clone(), config.account.clone())
        .with_metadata(metadata)
        .with_instructions([InstructionBox::from(verifying_keys::RegisterVerifyingKey {
            id: id.clone(),
            record,
        })]);
    let tx = sign_governance_transaction(
        tx,
        config,
        "failed to sign IVM execution VK registration transaction",
    )?;
    let hash = match client.submit_transaction_blocking(&tx) {
        Ok(hash) => hash,
        Err(err) => {
            let message = err.to_string();
            if message.contains("Repeated instruction")
                || message.contains("Repetition of `Register` for id `VerifyingKey")
                || message.contains("verifying key circuit/version already registered")
            {
                if let Some(existing) = existing_compatible_ivm_execution_vk(client).wrap_err(
                    "failed to list existing IVM execution VK after duplicate rejection",
                )? {
                    eprintln!("ivm_execution_vk_existing={}", existing.name);
                    return Ok(existing);
                }
                eprintln!("ivm_execution_vk_duplicate={}", id.name);
                return Ok(id);
            }
            return Err(err).wrap_err("failed to submit IVM execution VK registration");
        }
    };
    eprintln!("ivm_execution_vk_registered={hash}");
    Ok(id)
}

fn submit_sccp_route_manifest_transaction(
    client: &Client,
    config: &Config,
    tx: &SignedTransaction,
) -> Result<(String, &'static str)> {
    let tx_hash = match client.submit_transaction_blocking(tx) {
        Ok(hash) => return Ok((hash.to_string(), "single")),
        Err(err) if err.to_string().contains("length mismatch") => {
            let payload = client.prepare_transaction_payload(tx);
            let hash = payload.hash();
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .wrap_err("failed to create Tokio runtime for SCCP route batch submit")?;
            runtime
                .block_on(client.submit_prepared_transaction_payload_batch_async(&[payload]))
                .wrap_err(
                    "failed to submit SCCP route manifest upsert as one-item transaction batch",
                )?;
            let wait = client
                .wait_for_transaction_terminal_status(
                    hash,
                    TransactionWaitOptions {
                        timeout: config.transaction_status_timeout,
                        poll_interval: std::time::Duration::from_millis(500),
                        terminal_statuses: vec![TransactionWaitTerminalStatus::Applied],
                    },
                )
                .wrap_err("SCCP route manifest batch submit did not reach Applied status")?;
            if wait.terminal_kind != TransactionWaitTerminalStatus::Applied.as_str() {
                return Err(eyre!(
                    "SCCP route manifest batch submit stopped at `{}`: {}",
                    wait.terminal_kind,
                    wait.summary
                ));
            }
            hash
        }
        Err(err) => {
            return Err(err).wrap_err("failed to submit SCCP route manifest upsert transaction");
        }
    };
    Ok((tx_hash.to_string(), "batch"))
}

fn print_sccp_route_manifest_publish_output(
    tx_hash: &str,
    submit_mode: &str,
    manifest: &SccpRouteManifest,
) -> Result<()> {
    let mut output = norito::json::Map::new();
    output.insert("tx_hash".to_owned(), tx_hash.into());
    output.insert("submit_mode".to_owned(), submit_mode.into());
    output.insert("route_id".to_owned(), manifest.route_id.clone().into());
    output.insert("asset_key".to_owned(), manifest.asset_key.clone().into());
    output.insert("network".to_owned(), manifest.network.clone().into());
    output.insert("chain".to_owned(), manifest.chain.clone().into());
    output.insert(
        "production_ready".to_owned(),
        manifest.production_ready.into(),
    );
    output.insert(
        "destination_binding_hash".to_owned(),
        manifest.destination_binding_hash.clone().into(),
    );
    print_json_value(&norito::json::Value::Object(output))
}

fn publish_sccp_route_manifest(
    config_path: &Path,
    manifest_path: &Path,
    gas_asset_id: Option<&str>,
    gas_limit: u64,
    expected_route_id: Option<&str>,
    expected_asset_key: Option<&str>,
) -> Result<()> {
    let config = load_config(config_path)?;
    let client = Client::new(config.clone());
    let manifest = read_sccp_route_manifest_artifact(manifest_path)?;

    if let Some(expected) = expected_route_id
        && manifest.route_id != expected
    {
        return Err(eyre!(
            "route manifest id mismatch: expected `{expected}`, found `{}`",
            manifest.route_id
        ));
    }
    if let Some(expected) = expected_asset_key
        && manifest.asset_key != expected
    {
        return Err(eyre!(
            "route manifest asset mismatch: expected `{expected}`, found `{}`",
            manifest.asset_key
        ));
    }
    if !manifest.production_ready {
        return Err(eyre!(
            "route manifest `{}` is not marked production_ready",
            manifest.route_id
        ));
    }

    let mut metadata = tx_metadata(gas_asset_id, gas_limit)?;
    insert_string_metadata(&mut metadata, "action", "publish_sccp_route_manifest")?;
    insert_string_metadata(&mut metadata, "route_id", manifest.route_id.clone())?;
    insert_string_metadata(&mut metadata, "asset_key", manifest.asset_key.clone())?;

    let tx = TransactionBuilder::new(config.chain.clone(), config.account.clone())
        .with_metadata(metadata)
        .with_instructions([InstructionBox::from(UpsertSccpRouteManifest::new(
            manifest.clone(),
        ))]);
    let tx = sign_governance_transaction(
        tx,
        &config,
        "failed to sign SCCP route manifest upsert transaction",
    )?;
    let versioned_tx_bytes =
        <SignedTransaction as iroha_version::codec::EncodeVersioned>::encode_versioned(&tx);
    <SignedTransaction as iroha_version::codec::DecodeVersioned>::decode_all_versioned(
        &versioned_tx_bytes,
    )
    .wrap_err("locally encoded SCCP route manifest transaction does not decode")?;
    let (tx_hash, submit_mode) = submit_sccp_route_manifest_transaction(&client, &config, &tx)?;
    print_sccp_route_manifest_publish_output(&tx_hash, submit_mode, &manifest)
}

#[allow(clippy::too_many_arguments)]
fn record_sccp_transfer_payload_bytes(
    source_profile: String,
    target_profile: String,
    destination_binding_hash: String,
    nonce: u64,
    asset_home_domain: u32,
    asset_id_codec: u8,
    asset_id: String,
    amount: u128,
    sender_codec: u8,
    sender: String,
    recipient_codec: u8,
    recipient: String,
    route_id_codec: u8,
    route_id: String,
) -> Result<(String, SccpOutboundMessageContextV1, Vec<u8>)> {
    let source = SccpNetworkV1::from_profile_key(&source_profile).ok_or_else(|| {
        eyre!(
            "--source-profile must be an exact canonical SCCP profile key, got `{source_profile}`"
        )
    })?;
    let target = SccpNetworkV1::from_profile_key(&target_profile).ok_or_else(|| {
        eyre!(
            "--target-profile must be an exact canonical SCCP profile key, got `{target_profile}`"
        )
    })?;
    let binding_hex = destination_binding_hash
        .trim()
        .strip_prefix("0x")
        .or_else(|| destination_binding_hash.trim().strip_prefix("0X"))
        .unwrap_or_else(|| destination_binding_hash.trim());
    let binding_bytes = hex::decode(binding_hex)
        .wrap_err("--destination-binding-hash must be a 32-byte hex string")?;
    let destination_binding_hash: [u8; 32] =
        binding_bytes.try_into().map_err(|bytes: Vec<u8>| {
            eyre!(
                "--destination-binding-hash must be 32 bytes, got {}",
                bytes.len()
            )
        })?;
    let context = SccpOutboundMessageContextV1::new(
        SccpLaneIdV1 { source, target },
        destination_binding_hash,
    )
    .ok_or_else(|| {
        eyre!(
            "SCCP record context must be an exact SORA-to-external lane with a nonzero destination binding"
        )
    })?;
    let payload = SccpPayloadV1::Transfer(TransferPayloadV1 {
        version: 1,
        source_domain: source.domain_id(),
        dest_domain: target.domain_id(),
        nonce,
        asset_home_domain,
        asset_id_codec,
        asset_id: asset_id.into_bytes(),
        amount,
        sender_codec,
        sender: sender.into_bytes(),
        recipient_codec,
        recipient: recipient.into_bytes(),
        route_id_codec,
        route_id: route_id.into_bytes(),
    });
    if !verify_sccp_payload_structure(&payload) {
        return Err(eyre!(
            "SCCP transfer payload failed structural verification"
        ));
    }
    let commitment = hub_commitment_from_sccp_payload(context, &payload).ok_or_else(|| {
        eyre!(
            "SCCP transfer payload, exact lane, and destination binding do not form a valid commitment"
        )
    })?;
    let payload_bytes = canonical_sccp_payload_bytes(&payload);
    Ok((hex::encode(commitment.message_id), context, payload_bytes))
}

#[allow(clippy::too_many_lines)]
fn main() -> Result<()> {
    let args = Args::parse();
    match args.command {
        Command::RegisterCitizen {
            owner,
            amount,
            chain_discriminant,
        } => {
            let owner = parse_account_address(&owner, Some(chain_discriminant))
                .wrap_err("failed to parse --owner as canonical account address")?
                .address
                .to_account_id()
                .map_err(|err| eyre!(err.to_string()))
                .wrap_err("failed to decode --owner into account id")?;
            let instruction = InstructionBox::from(RegisterCitizen { owner, amount });
            let bytes = norito::to_bytes(&instruction).wrap_err("failed to encode instruction")?;
            print_tx_stdin_json(&bytes);
        }
        Command::WrapPayloadHex {
            wire_id,
            payload_hex,
        } => {
            let bytes = hex::decode(payload_hex.trim())
                .wrap_err("failed to decode --payload-hex as lowercase hex")?;
            let instruction = decode_instruction_from_pair(&wire_id, &bytes)
                .wrap_err("failed to decode instruction from --wire-id and --payload-hex")?;
            let encoded = norito::to_bytes(&instruction)
                .wrap_err("failed to encode reconstructed instruction")?;
            print_tx_stdin_json(&encoded);
        }
        Command::RecordSccpTransfer {
            source_profile,
            target_profile,
            destination_binding_hash,
            nonce,
            asset_home_domain,
            asset_id_codec,
            asset_id,
            amount,
            sender_codec,
            sender,
            recipient_codec,
            recipient,
            route_id_codec,
            route_id,
        } => {
            let (message_id, context, payload_bytes) = record_sccp_transfer_payload_bytes(
                source_profile,
                target_profile,
                destination_binding_hash,
                nonce,
                asset_home_domain,
                asset_id_codec,
                asset_id,
                amount,
                sender_codec,
                sender,
                recipient_codec,
                recipient,
                route_id_codec,
                route_id,
            )?;
            eprintln!("message_id={message_id}");
            let instruction = InstructionBox::from(RecordSccpMessage::new(context, payload_bytes));
            let bytes = norito::to_bytes(&instruction).wrap_err("failed to encode instruction")?;
            print_tx_stdin_json(&bytes);
        }
        Command::EnsureIvmExecutionVk {
            config,
            vk_name,
            gas_asset_id,
            gas_limit,
        } => {
            let config = load_config(&config)?;
            let client = Client::new(config.clone());
            let id = ensure_ivm_execution_vk(
                &client,
                &config,
                &vk_name,
                gas_asset_id.as_deref(),
                gas_limit,
            )?;
            let mut output = norito::json::Map::new();
            output.insert("backend".to_owned(), id.backend.as_str().into());
            output.insert("name".to_owned(), id.name.into());
            print_json_value(&norito::json::Value::Object(output))?;
        }
        Command::PublishSccpRouteManifest {
            config,
            manifest,
            gas_asset_id,
            gas_limit,
            expected_route_id,
            expected_asset_key,
        } => publish_sccp_route_manifest(
            &config,
            &manifest,
            gas_asset_id.as_deref(),
            gas_limit,
            expected_route_id.as_deref(),
            expected_asset_key.as_deref(),
        )?,
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    use iroha::data_model::{ChainId, account::AccountId};
    use iroha_config::parameters::{
        actual::SorafsRolloutPhase,
        defaults::{
            sorafs::gateway::{DEFAULT_ANONYMITY_POLICY, DEFAULT_ROLLOUT_PHASE},
            torii,
        },
    };
    use iroha_crypto::{Algorithm, KeyPair};
    use sorafs_manifest::alias_cache::AliasCachePolicy;
    use sorafs_orchestrator::AnonymityPolicy;
    use url::Url;

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

    fn fixture_key_pair(seed: u8) -> KeyPair {
        KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
            .expect("fixture seed must derive a valid keypair")
    }

    fn test_config_with_chain_discriminant(chain_discriminant: u16) -> Config {
        let key_pair = fixture_key_pair(42);
        let account = AccountId::new(key_pair.public_key().clone());
        Config {
            chain: ChainId::from("00000000-0000-0000-0000-000000000000"),
            account,
            account_chain_discriminant: chain_discriminant,
            key_pair,
            basic_auth: None,
            torii_api_url: Url::parse("http://127.0.0.1/").expect("valid url"),
            torii_api_version: iroha::config::default_torii_api_version(),
            torii_api_min_proof_version: iroha::config::DEFAULT_TORII_API_MIN_PROOF_VERSION
                .to_string(),
            torii_request_timeout: iroha::config::DEFAULT_TORII_REQUEST_TIMEOUT,
            transaction_ttl: iroha::config::DEFAULT_TRANSACTION_TIME_TO_LIVE,
            transaction_status_timeout: iroha::config::DEFAULT_TRANSACTION_STATUS_TIMEOUT,
            transaction_add_nonce: iroha::config::DEFAULT_TRANSACTION_NONCE,
            connect_queue_root: iroha::config::default_connect_queue_root(),
            soracloud_http_witness_file: None,
            sorafs_alias_cache: default_alias_cache_policy(),
            sorafs_anonymity_policy: default_anonymity_policy(),
            sorafs_rollout_phase: default_rollout_phase(),
        }
    }

    #[test]
    fn fixture_key_pair_uses_checked_seed_derivation() {
        assert_eq!(fixture_key_pair(42).algorithm(), Algorithm::Ed25519);
        assert!(
            KeyPair::try_from_seed(vec![0; 32], Algorithm::Ed25519).is_err(),
            "checked Ed25519 seed derivation must reject weak all-zero fixture seeds"
        );
    }

    #[test]
    fn retired_sccp_ivm_proof_commands_are_not_parseable() {
        for command in [
            "record-sccp-transfer-ivm-proved",
            "build-sccp-transfer-ivm-derive-request",
        ] {
            assert!(
                Args::try_parse_from(["gov_instruction", command]).is_err(),
                "retired SCCP IVM wrapper `{command}` must not remain in the CLI grammar"
            );
        }
    }

    #[test]
    fn sign_governance_transaction_checked_helper_verifies() -> Result<()> {
        let config = test_config_with_chain_discriminant(369);
        let tx_builder = TransactionBuilder::new(config.chain.clone(), config.account.clone())
            .with_instructions(Vec::<InstructionBox>::new());

        let tx =
            sign_governance_transaction(tx_builder, &config, "sign test governance transaction")?;

        tx.verify_signature()
            .wrap_err("verify governance helper signature")?;
        assert_eq!(tx.authority(), &config.account);
        Ok(())
    }

    #[test]
    fn record_sccp_transfer_payload_rejects_noncanonical_evm_recipient() {
        let err = record_sccp_transfer_payload_bytes(
            "sora-nexus".to_owned(),
            "ethereum-mainnet".to_owned(),
            "11".repeat(32),
            7,
            0,
            1,
            "xor#universal".to_owned(),
            42,
            1,
            "sora:bridge".to_owned(),
            2,
            "0x52908400098527886e0f7030069857d2e4169ee7".to_owned(),
            1,
            "nexus:eth:xor".to_owned(),
        )
        .expect_err("noncanonical EVM recipient should be rejected");

        assert!(err.to_string().contains("structural verification"));
    }

    #[test]
    fn record_sccp_transfer_payload_accepts_canonical_ton_recipient() {
        let (message_id, context, payload_bytes) = record_sccp_transfer_payload_bytes(
            "sora-nexus".to_owned(),
            "ton-mainnet".to_owned(),
            "22".repeat(32),
            7,
            0,
            1,
            "xor#universal".to_owned(),
            42,
            1,
            "sora:bridge".to_owned(),
            4,
            "0:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_owned(),
            1,
            "nexus:ton:xor".to_owned(),
        )
        .expect("canonical TON recipient should be accepted");

        assert_eq!(message_id.len(), 64);
        assert_eq!(context.lane.source, SccpNetworkV1::SoraNexus);
        assert_eq!(context.lane.target, SccpNetworkV1::TonMainnet);
        assert!(!payload_bytes.is_empty());
    }

    #[test]
    fn record_sccp_transfer_payload_rejects_aliases_wrong_direction_and_zero_binding() {
        let cases = [
            (
                "sora",
                "ethereum-mainnet",
                "11".repeat(32),
                "exact canonical SCCP profile key",
            ),
            (
                " sora-nexus",
                "ethereum-mainnet",
                "11".repeat(32),
                "exact canonical SCCP profile key",
            ),
            (
                "ethereum-mainnet",
                "sora-nexus",
                "11".repeat(32),
                "SORA-to-external lane",
            ),
            (
                "sora-nexus",
                "sora-taira",
                "11".repeat(32),
                "SORA-to-external lane",
            ),
            (
                "sora-nexus",
                "ethereum-mainnet",
                "00".repeat(32),
                "nonzero destination binding",
            ),
        ];

        for (source, target, binding, expected) in cases {
            let error = record_sccp_transfer_payload_bytes(
                source.to_owned(),
                target.to_owned(),
                binding,
                7,
                0,
                1,
                "xor#universal".to_owned(),
                42,
                1,
                "sora:bridge".to_owned(),
                2,
                "0x52908400098527886E0F7030069857D2E4169EE7".to_owned(),
                1,
                "nexus:eth:xor".to_owned(),
            )
            .expect_err("invalid exact SCCP context must fail before instruction construction");
            assert!(
                error.to_string().contains(expected),
                "unexpected error for {source}->{target}: {error:?}"
            );
        }
    }
}
