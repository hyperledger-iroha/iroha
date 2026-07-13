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
    client::Client,
    config::{Config, LoadPath},
    data_model::{
        isi::{
            InstructionBox,
            bridge::{ApplySccpRouteGovernance, RecordSccpMessage, SccpRouteGovernanceActionV1},
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
        /// Canonical first-release SORA source profile: exactly `sora-taira`.
        #[arg(long)]
        source_profile: String,
        /// Canonical exact external destination profile, such as `ethereum-mainnet`.
        #[arg(long)]
        target_profile: String,
        /// Governed destination binding hash active for this message.
        #[arg(long)]
        destination_binding_hash: String,
        /// Immutable governed route-configuration hash active for this message.
        #[arg(long)]
        route_configuration_hash: String,
        #[arg(long)]
        nonce: u64,
        /// Nonzero immutable governed route revision.
        #[arg(long)]
        route_revision: u32,
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
    /// Apply one exact on-chain SCCP route-governance action from canonical JSON.
    ApplySccpRouteGovernance {
        #[arg(long)]
        config: PathBuf,
        #[arg(long)]
        action: PathBuf,
        #[arg(long)]
        gas_asset_id: Option<String>,
        #[arg(long, default_value_t = DEFAULT_LEDGER_GAS_LIMIT)]
        gas_limit: u64,
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
    if gas_limit == 0 {
        return Err(eyre!("gas_limit must be a positive integer"));
    }
    let mut metadata = Metadata::default();
    if let Some(asset_id) = gas_asset_id.filter(|value| !value.trim().is_empty()) {
        insert_string_metadata(&mut metadata, "gas_asset_id", asset_id.trim().to_owned())?;
    }
    iroha::data_model::transaction::insert_transaction_gas_limit(&mut metadata, gas_limit);
    Ok(metadata)
}

fn read_sccp_route_governance_action(path: &Path) -> Result<SccpRouteGovernanceActionV1> {
    let raw = fs::read_to_string(path)
        .wrap_err_with(|| format!("failed to read SCCP governance action `{}`", path.display()))?;
    let action: SccpRouteGovernanceActionV1 = norito::json::from_str(&raw)
        .wrap_err("failed to decode canonical SCCP route-governance action JSON")?;
    action
        .validate_static()
        .map_err(|error| eyre!("invalid SCCP route-governance action: {error}"))?;
    Ok(action)
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

fn submit_sccp_route_governance_transaction(
    client: &Client,
    tx: &SignedTransaction,
) -> Result<String> {
    submit_sccp_route_governance_once(|| client.submit_transaction_blocking(tx))
        .map(|hash| hash.to_string())
}

fn submit_sccp_route_governance_once<T>(submit: impl FnOnce() -> Result<T>) -> Result<T> {
    submit().wrap_err("failed to submit SCCP route-governance transaction")
}

fn sccp_route_governance_action_label(action: &SccpRouteGovernanceActionV1) -> &'static str {
    match action {
        SccpRouteGovernanceActionV1::Register(_) => "register",
        SccpRouteGovernanceActionV1::SetActivation(_) => "set_activation",
        SccpRouteGovernanceActionV1::SwitchRevision(_) => "switch_revision",
        SccpRouteGovernanceActionV1::InitializeTrustAnchor(_) => "initialize_trust_anchor",
        SccpRouteGovernanceActionV1::AdvanceTrustAnchor(_) => "advance_trust_anchor",
        SccpRouteGovernanceActionV1::Remove(_) => "remove",
    }
}

fn print_sccp_route_governance_output(
    tx_hash: &str,
    submit_mode: &str,
    action: &SccpRouteGovernanceActionV1,
) -> Result<()> {
    let mut output = norito::json::Map::new();
    output.insert("tx_hash".to_owned(), tx_hash.into());
    output.insert("submit_mode".to_owned(), submit_mode.into());
    output.insert(
        "governance_action".to_owned(),
        sccp_route_governance_action_label(action).into(),
    );
    print_json_value(&norito::json::Value::Object(output))
}

fn apply_sccp_route_governance(
    config_path: &Path,
    action_path: &Path,
    gas_asset_id: Option<&str>,
    gas_limit: u64,
) -> Result<()> {
    let config = load_config(config_path)?;
    let client = Client::new(config.clone());
    let action = read_sccp_route_governance_action(action_path)?;

    let mut metadata = tx_metadata(gas_asset_id, gas_limit)?;
    insert_string_metadata(&mut metadata, "action", "apply_sccp_route_governance")?;
    insert_string_metadata(
        &mut metadata,
        "sccp_governance_action",
        sccp_route_governance_action_label(&action),
    )?;

    let tx = TransactionBuilder::new(config.chain.clone(), config.account.clone())
        .with_metadata(metadata)
        .with_instructions([InstructionBox::from(ApplySccpRouteGovernance::new(
            action.clone(),
        ))]);
    let tx = sign_governance_transaction(
        tx,
        &config,
        "failed to sign SCCP route-governance transaction",
    )?;
    let versioned_tx_bytes =
        <SignedTransaction as iroha_version::codec::EncodeVersioned>::encode_versioned(&tx);
    <SignedTransaction as iroha_version::codec::DecodeVersioned>::decode_all_versioned(
        &versioned_tx_bytes,
    )
    .wrap_err("locally encoded SCCP route-governance transaction does not decode")?;
    let tx_hash = submit_sccp_route_governance_transaction(&client, &tx)?;
    print_sccp_route_governance_output(&tx_hash, "single", &action)
}

fn parse_canonical_hex32_argument(name: &str, value: &str) -> Result<[u8; 32]> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return Err(eyre!(
            "{name} must be exactly 64 unprefixed lowercase hexadecimal characters"
        ));
    }
    let bytes = hex::decode(value).expect("validated lowercase hexadecimal input");
    bytes
        .try_into()
        .map_err(|_| eyre!("{name} must decode to exactly 32 bytes"))
}

fn parse_sccp_codec_argument(name: &str, codec: u8, value: &str) -> Result<Vec<u8>> {
    match codec {
        iroha_sccp::SCCP_CODEC_CANONICAL_TEXT => Ok(value.as_bytes().to_vec()),
        iroha_sccp::SCCP_CODEC_EVM_ADDRESS20 => {
            if value.len() != 40
                || !value
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
            {
                return Err(eyre!(
                    "{name} using evm_address20 must be exactly 40 unprefixed lowercase hexadecimal characters"
                ));
            }
            hex::decode(value).wrap_err_with(|| format!("failed to decode {name}"))
        }
        iroha_sccp::SCCP_CODEC_TRON_ADDRESS21 => {
            if value.len() != 42
                || !value
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
            {
                return Err(eyre!(
                    "{name} using tron_address21 must be exactly 42 unprefixed lowercase hexadecimal characters"
                ));
            }
            let bytes = hex::decode(value).wrap_err_with(|| format!("failed to decode {name}"))?;
            if bytes.first() != Some(&0x41) || bytes[1..].iter().all(|byte| *byte == 0) {
                return Err(eyre!(
                    "{name} using tron_address21 must start with 41 and have a nonzero 20-byte payload"
                ));
            }
            Ok(bytes)
        }
        _ => Err(eyre!("{name} uses unsupported SCCP codec {codec}")),
    }
}

#[allow(clippy::too_many_arguments)]
fn record_sccp_transfer_payload_bytes(
    source_profile: &str,
    target_profile: &str,
    destination_binding_hash: &str,
    route_configuration_hash: &str,
    nonce: u64,
    route_revision: u32,
    asset_home_domain: u32,
    asset_id_codec: u8,
    asset_id: &str,
    amount: u128,
    sender_codec: u8,
    sender: &str,
    recipient_codec: u8,
    recipient: &str,
    route_id_codec: u8,
    route_id: &str,
) -> Result<(String, SccpOutboundMessageContextV1, Vec<u8>)> {
    let source = SccpNetworkV1::from_profile_key(source_profile).ok_or_else(|| {
        eyre!(
            "--source-profile must be an exact canonical SCCP profile key, got `{source_profile}`"
        )
    })?;
    let target = SccpNetworkV1::from_profile_key(target_profile).ok_or_else(|| {
        eyre!(
            "--target-profile must be an exact canonical SCCP profile key, got `{target_profile}`"
        )
    })?;
    if source != SccpNetworkV1::SoraTaira || !target.is_external() {
        return Err(eyre!(
            "SCCP record context must select the exact sora-taira to Ethereum, BSC, or TRON lane"
        ));
    }
    let (expected_route_id, expected_recipient_codec) = match target {
        SccpNetworkV1::EthereumMainnet | SccpNetworkV1::EthereumSepolia => (
            iroha_sccp::SCCP_TAIRA_ETH_XOR_ROUTE_ID_V1,
            iroha_sccp::SCCP_CODEC_EVM_ADDRESS20,
        ),
        SccpNetworkV1::BscMainnet | SccpNetworkV1::BscTestnet => (
            iroha_sccp::SCCP_TAIRA_BSC_XOR_ROUTE_ID_V1,
            iroha_sccp::SCCP_CODEC_EVM_ADDRESS20,
        ),
        SccpNetworkV1::TronMainnet | SccpNetworkV1::TronNile | SccpNetworkV1::TronShasta => (
            iroha_sccp::SCCP_TAIRA_TRON_XOR_ROUTE_ID_V1,
            iroha_sccp::SCCP_CODEC_TRON_ADDRESS21,
        ),
        SccpNetworkV1::SoraTaira => {
            unreachable!("SORA target rejected above")
        }
    };
    if asset_home_domain != iroha_sccp::SCCP_DOMAIN_SORA
        || asset_id_codec != iroha_sccp::SCCP_CODEC_CANONICAL_TEXT
        || asset_id != iroha_sccp::SCCP_TAIRA_XOR_ASSET_KEY_V1
        || sender_codec != iroha_sccp::SCCP_CODEC_CANONICAL_TEXT
        || recipient_codec != expected_recipient_codec
        || route_id_codec != iroha_sccp::SCCP_CODEC_CANONICAL_TEXT
        || route_id != expected_route_id
    {
        return Err(eyre!(
            "SCCP record payload must use the exact Taira XOR asset, family route, sender, and recipient codecs"
        ));
    }
    let destination_binding_hash =
        parse_canonical_hex32_argument("--destination-binding-hash", destination_binding_hash)?;
    let route_configuration_hash =
        parse_canonical_hex32_argument("--route-configuration-hash", route_configuration_hash)?;
    let context = SccpOutboundMessageContextV1::new(
        SccpLaneIdV1 { source, target },
        destination_binding_hash,
        route_configuration_hash,
    )
    .ok_or_else(|| {
        eyre!(
            "SCCP record context must be an exact Taira-to-external lane with nonzero distinct destination-binding and route-configuration hashes"
        )
    })?;
    let payload = SccpPayloadV1::Transfer(TransferPayloadV1 {
        version: 1,
        source_domain: source.domain_id(),
        dest_domain: target.domain_id(),
        nonce,
        route_revision,
        asset_home_domain,
        asset_id_codec,
        asset_id: parse_sccp_codec_argument("--asset-id", asset_id_codec, asset_id)?,
        amount,
        sender_codec,
        sender: parse_sccp_codec_argument("--sender", sender_codec, sender)?,
        recipient_codec,
        recipient: parse_sccp_codec_argument("--recipient", recipient_codec, recipient)?,
        route_id_codec,
        route_id: parse_sccp_codec_argument("--route-id", route_id_codec, route_id)?,
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
    let payload_bytes = canonical_sccp_payload_bytes(&payload)?;
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
            route_configuration_hash,
            nonce,
            route_revision,
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
                &source_profile,
                &target_profile,
                &destination_binding_hash,
                &route_configuration_hash,
                nonce,
                route_revision,
                asset_home_domain,
                asset_id_codec,
                &asset_id,
                amount,
                sender_codec,
                &sender,
                recipient_codec,
                &recipient,
                route_id_codec,
                &route_id,
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
        Command::ApplySccpRouteGovernance {
            config,
            action,
            gas_asset_id,
            gas_limit,
        } => apply_sccp_route_governance(&config, &action, gas_asset_id.as_deref(), gas_limit)?,
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::Cell, time::Duration};

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
            "publish-sccp-route-manifest",
        ] {
            assert!(
                Args::try_parse_from(["gov_instruction", command]).is_err(),
                "retired SCCP IVM wrapper `{command}` must not remain in the CLI grammar"
            );
        }
    }

    #[test]
    fn transaction_metadata_rejects_zero_gas_before_signing() {
        let error = tx_metadata(Some("xor#sora"), 0)
            .expect_err("zero gas must fail before transaction construction");
        assert!(error.to_string().contains("positive"));
    }

    #[test]
    fn sccp_governance_submission_never_retries_ambiguous_errors() {
        let attempts = Cell::new(0_u8);
        let error = submit_sccp_route_governance_once::<()>(|| {
            attempts.set(attempts.get() + 1);
            Err(eyre!(
                "length mismatch after an ambiguous remote acceptance"
            ))
        })
        .expect_err("submission error must propagate without a second mutation attempt");
        assert_eq!(attempts.get(), 1);
        assert!(error.to_string().contains("failed to submit"));
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
    fn route_governance_action_reader_is_strict_and_validates_static_invariants() -> Result<()> {
        use iroha::data_model::{
            bridge::{SccpLaneIdV1, SccpNetworkV1, SccpRouteKeyV1},
            isi::bridge::SccpRouteGovernanceActionV1,
        };

        let action = SccpRouteGovernanceActionV1::Remove(SccpRouteKeyV1 {
            lane_id: SccpLaneIdV1 {
                source: SccpNetworkV1::EthereumMainnet,
                target: SccpNetworkV1::SoraTaira,
            },
            route_id: "taira_eth_xor".to_owned(),
            asset_key: "xor".to_owned(),
            revision: 1,
        });
        let canonical = norito::json::to_string(&action)?;
        let file = tempfile::NamedTempFile::new()?;
        std::fs::write(file.path(), &canonical)?;
        assert_eq!(read_sccp_route_governance_action(file.path())?, action);

        let unknown = canonical.replacen(
            "\"revision\":1",
            "\"revision\":1,\"future_authority\":true",
            1,
        );
        assert_ne!(
            unknown, canonical,
            "fixture JSON must expose route revision"
        );
        std::fs::write(file.path(), unknown)?;
        assert!(
            read_sccp_route_governance_action(file.path()).is_err(),
            "unknown governance fields must fail closed"
        );

        let invalid = canonical.replacen("\"revision\":1", "\"revision\":0", 1);
        assert_ne!(
            invalid, canonical,
            "fixture JSON must expose route revision"
        );
        std::fs::write(file.path(), invalid)?;
        assert!(
            read_sccp_route_governance_action(file.path()).is_err(),
            "statically invalid governance actions must reject before signing"
        );
        Ok(())
    }

    #[test]
    fn canonical_hash_arguments_reject_aliases_and_malformed_values() {
        let valid = "ab".repeat(32);
        assert_eq!(
            parse_canonical_hex32_argument("--hash", &valid).expect("canonical hash"),
            [0xab; 32]
        );
        for value in [
            format!("0x{valid}"),
            valid.to_uppercase(),
            format!(" {valid}"),
            format!("{valid} "),
            "ab".repeat(31),
            "ag".repeat(32),
        ] {
            assert!(
                parse_canonical_hex32_argument("--hash", &value).is_err(),
                "noncanonical hash alias `{value}` must reject"
            );
        }
    }

    #[test]
    fn record_sccp_transfer_rejects_zero_revision_and_aliased_context_commitments() {
        let build = |binding: String, configuration: String, revision| {
            record_sccp_transfer_payload_bytes(
                "sora-taira",
                "ethereum-mainnet",
                &binding,
                &configuration,
                7,
                revision,
                0,
                iroha_sccp::SCCP_CODEC_CANONICAL_TEXT,
                "xor",
                42,
                iroha_sccp::SCCP_CODEC_CANONICAL_TEXT,
                "sora:bridge",
                iroha_sccp::SCCP_CODEC_EVM_ADDRESS20,
                &"11".repeat(20),
                iroha_sccp::SCCP_CODEC_CANONICAL_TEXT,
                "taira_eth_xor",
            )
        };

        assert!(build("11".repeat(32), "12".repeat(32), 0).is_err());
        assert!(build("11".repeat(32), "11".repeat(32), 1).is_err());
        assert!(build("00".repeat(32), "12".repeat(32), 1).is_err());
        assert!(build("11".repeat(32), "00".repeat(32), 1).is_err());
    }

    #[test]
    fn record_sccp_transfer_payload_rejects_prefixed_evm_recipient() {
        let err = record_sccp_transfer_payload_bytes(
            "sora-taira",
            "ethereum-mainnet",
            &"11".repeat(32),
            &"12".repeat(32),
            7,
            1,
            0,
            1,
            "xor",
            42,
            1,
            "sora:bridge",
            2,
            "0x52908400098527886e0f7030069857d2e4169ee7",
            1,
            "taira_eth_xor",
        )
        .expect_err("prefixed EVM recipient should be rejected");

        assert!(err.to_string().contains("unprefixed lowercase hexadecimal"));
    }

    #[test]
    fn record_sccp_transfer_payload_accepts_canonical_tron_recipient() {
        let (message_id, context, payload_bytes) = record_sccp_transfer_payload_bytes(
            "sora-taira",
            "tron-mainnet",
            &"22".repeat(32),
            &"23".repeat(32),
            7,
            1,
            0,
            1,
            "xor",
            42,
            1,
            "sora:bridge",
            iroha_sccp::SCCP_CODEC_TRON_ADDRESS21,
            &format!("41{}", "12".repeat(20)),
            1,
            "taira_tron_xor",
        )
        .expect("canonical TRON recipient should be accepted");

        assert_eq!(message_id.len(), 64);
        assert_eq!(context.lane.source, SccpNetworkV1::SoraTaira);
        assert_eq!(context.lane.target, SccpNetworkV1::TronMainnet);
        assert!(!payload_bytes.is_empty());
    }

    #[test]
    fn record_sccp_transfer_payload_rejects_cross_family_or_non_xor_identity() {
        let build =
            |target: &str, asset: &str, recipient_codec: u8, recipient: String, route: &str| {
                record_sccp_transfer_payload_bytes(
                    "sora-taira",
                    target,
                    &"21".repeat(32),
                    &"22".repeat(32),
                    7,
                    1,
                    iroha_sccp::SCCP_DOMAIN_SORA,
                    iroha_sccp::SCCP_CODEC_CANONICAL_TEXT,
                    asset,
                    42,
                    iroha_sccp::SCCP_CODEC_CANONICAL_TEXT,
                    "sora:bridge",
                    recipient_codec,
                    &recipient,
                    iroha_sccp::SCCP_CODEC_CANONICAL_TEXT,
                    route,
                )
            };

        assert!(
            build(
                "ethereum-mainnet",
                "not-xor",
                iroha_sccp::SCCP_CODEC_EVM_ADDRESS20,
                "11".repeat(20),
                iroha_sccp::SCCP_TAIRA_ETH_XOR_ROUTE_ID_V1,
            )
            .is_err()
        );
        assert!(
            build(
                "ethereum-mainnet",
                "xor",
                iroha_sccp::SCCP_CODEC_TRON_ADDRESS21,
                format!("41{}", "11".repeat(20)),
                iroha_sccp::SCCP_TAIRA_ETH_XOR_ROUTE_ID_V1,
            )
            .is_err()
        );
        assert!(
            build(
                "bsc-mainnet",
                "xor",
                iroha_sccp::SCCP_CODEC_EVM_ADDRESS20,
                "11".repeat(20),
                iroha_sccp::SCCP_TAIRA_ETH_XOR_ROUTE_ID_V1,
            )
            .is_err()
        );
        assert!(
            build(
                "tron-mainnet",
                "xor",
                iroha_sccp::SCCP_CODEC_EVM_ADDRESS20,
                "11".repeat(20),
                iroha_sccp::SCCP_TAIRA_TRON_XOR_ROUTE_ID_V1,
            )
            .is_err()
        );
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
                "exact sora-taira",
            ),
            (
                "sora-nexus",
                "ethereum-mainnet",
                "11".repeat(32),
                "exact sora-taira",
            ),
            (
                "sora-taira",
                "sora-nexus",
                "11".repeat(32),
                "exact sora-taira",
            ),
            (
                "sora-taira",
                "ethereum-mainnet",
                "00".repeat(32),
                "nonzero distinct destination-binding",
            ),
        ];

        for (source, target, binding, expected) in cases {
            let error = record_sccp_transfer_payload_bytes(
                source,
                target,
                &binding,
                &"12".repeat(32),
                7,
                1,
                0,
                1,
                "xor",
                42,
                1,
                "sora:bridge",
                2,
                "52908400098527886e0f7030069857d2e4169ee7",
                1,
                "taira_eth_xor",
            )
            .expect_err("invalid exact SCCP context must fail before instruction construction");
            assert!(
                error.to_string().contains(expected),
                "unexpected error for {source}->{target}: {error:?}"
            );
        }
    }
}
