//! Encode governance instructions and proof-gated governance helper transactions.

use std::{path::PathBuf, str::FromStr};

use clap::{Parser, Subcommand};
use eyre::{Result, WrapErr as _, eyre};
use iroha::{
    account_address::parse_account_address,
    client::Client,
    config::{Config, LoadPath},
    data_model::{
        account::address::ChainDiscriminantGuard,
        isi::{
            InstructionBox, bridge::RecordSccpMessage, decode_instruction_from_pair,
            governance::RegisterCitizen, verifying_keys,
        },
        metadata::Metadata,
        name::Name,
        proof::{ProofAttachment, ProofAttachmentList, VerifyingKeyId},
        transaction::{Executable, IvmBytecode, IvmProved, TransactionBuilder},
    },
};
use iroha_crypto::Hash;
use iroha_primitives::json::Json;
use iroha_sccp::{
    SccpPayloadV1, TransferPayloadV1, canonical_sccp_payload_bytes, sccp_message_id,
    verify_sccp_payload_structure,
};

const DEFAULT_LEDGER_GAS_LIMIT: u64 = 2_000_000;
const DEFAULT_IVM_GAS_LIMIT: u64 = 50_000_000;
const DEFAULT_MAX_CYCLES: u64 = 1_000_000;
const LITERAL_DATA_START: i16 = 16;
const WIDE_IMM_MIN: i64 = -128;
const WIDE_IMM_MAX: i64 = 127;

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
        #[arg(long)]
        source_domain: u32,
        #[arg(long)]
        dest_domain: u32,
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
    /// Record an SCCP transfer through proof-gated `Executable::IvmProved` admission.
    RecordSccpTransferIvmProved {
        #[arg(long)]
        config: PathBuf,
        #[arg(long, default_value = "ivm_execution")]
        vk_name: String,
        #[arg(long)]
        gas_asset_id: Option<String>,
        #[arg(long, default_value_t = DEFAULT_IVM_GAS_LIMIT)]
        gas_limit: u64,
        #[arg(long, default_value_t = DEFAULT_MAX_CYCLES)]
        max_cycles: u64,
        #[arg(long)]
        source_domain: u32,
        #[arg(long)]
        dest_domain: u32,
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
    /// Build the JSON body used for `/v1/zk/ivm/derive` without submitting it.
    BuildSccpTransferIvmDeriveRequest {
        #[arg(long)]
        config: PathBuf,
        #[arg(long, default_value = "ivm_execution")]
        vk_name: String,
        #[arg(long)]
        gas_asset_id: Option<String>,
        #[arg(long, default_value_t = DEFAULT_IVM_GAS_LIMIT)]
        gas_limit: u64,
        #[arg(long, default_value_t = DEFAULT_MAX_CYCLES)]
        max_cycles: u64,
        #[arg(long)]
        source_domain: u32,
        #[arg(long)]
        dest_domain: u32,
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

fn make_tlv(type_id: u16, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(7 + payload.len() + Hash::LENGTH);
    out.extend_from_slice(&type_id.to_be_bytes());
    out.push(1);
    out.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    out.extend_from_slice(payload);
    let h: [u8; Hash::LENGTH] = Hash::new(payload).into();
    out.extend_from_slice(&h);
    out
}

fn norito_tlv<T: norito::NoritoSerialize>(value: &T) -> Result<Vec<u8>> {
    let payload = norito::to_bytes(value)?;
    Ok(make_tlv(ivm::PointerType::NoritoBytes as u16, &payload))
}

fn push_word(code: &mut Vec<u8>, word: u32) {
    code.extend_from_slice(&word.to_le_bytes());
}

fn chunk_immediate(value: i64) -> i8 {
    if value > WIDE_IMM_MAX {
        WIDE_IMM_MAX as i8
    } else if value < WIDE_IMM_MIN {
        WIDE_IMM_MIN as i8
    } else {
        value as i8
    }
}

fn emit_addi(code: &mut Vec<u8>, rd: u8, rs1: u8, mut value: i64) {
    if rd != rs1 {
        push_word(
            code,
            ivm::encoding::wide::encode_ri(ivm::instruction::wide::arithmetic::ADDI, rd, rs1, 0),
        );
    }
    while value != 0 {
        let chunk = chunk_immediate(value);
        push_word(
            code,
            ivm::encoding::wide::encode_ri(ivm::instruction::wide::arithmetic::ADDI, rd, rd, chunk),
        );
        value -= chunk as i64;
    }
}

fn push_syscall(code: &mut Vec<u8>, syscall: u32) -> Result<()> {
    push_word(
        code,
        ivm::encoding::wide::encode_sys(
            ivm::instruction::wide::system::SCALL,
            u8::try_from(syscall).map_err(|_| eyre!("syscall id does not fit in u8"))?,
        ),
    );
    Ok(())
}

fn assemble_program_with_literals(code: &[u8], literal_data: &[u8], max_cycles: u64) -> Vec<u8> {
    let metadata = ivm::ProgramMetadata {
        max_cycles,
        mode: ivm::ivm_mode::ZK,
        vector_length: 4,
        ..Default::default()
    };
    let mut program = metadata.encode();
    if !literal_data.is_empty() {
        let unpadded_literal_len = 16 + literal_data.len();
        let post_pad = (4 - (unpadded_literal_len % 4)) % 4;
        program.extend_from_slice(b"LTLB");
        program.extend_from_slice(&0u32.to_le_bytes());
        program.extend_from_slice(&(post_pad as u32).to_le_bytes());
        program.extend_from_slice(&(literal_data.len() as u32).to_le_bytes());
        program.extend_from_slice(literal_data);
        program.extend(std::iter::repeat_n(0u8, post_pad));
    }
    program.extend_from_slice(code);
    program
}

fn build_record_instruction_program(
    instruction: &InstructionBox,
    max_cycles: u64,
) -> Result<Vec<u8>> {
    let tlv = norito_tlv(instruction)?;
    let mut code = Vec::new();
    emit_addi(&mut code, 10, 0, i64::from(LITERAL_DATA_START));
    push_syscall(&mut code, ivm::syscalls::SYSCALL_INPUT_PUBLISH_TLV)?;
    push_syscall(
        &mut code,
        ivm::syscalls::SYSCALL_SMARTCONTRACT_EXECUTE_INSTRUCTION,
    )?;
    code.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
    Ok(assemble_program_with_literals(&code, &tlv, max_cycles))
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

fn load_config(path: &PathBuf) -> Result<Config> {
    Config::load(LoadPath::Explicit(path.clone())).map_err(|report| {
        eyre!(
            "failed to load client config `{}`: {report}",
            path.display()
        )
    })
}

fn ivm_execution_vk_id(name: &str) -> VerifyingKeyId {
    VerifyingKeyId::new(iroha_core::zk::ZK_BACKEND_HALO2_IPA, name)
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

    let record = iroha_core::zk::halo2_ipa_ivm_execution_vk_record("core", 1)
        .map_err(|err| eyre!("failed to build ivm-execution-v1 VK record: {err}"))?;
    let metadata = tx_metadata(gas_asset_id, gas_limit)?;
    let tx = TransactionBuilder::new(config.chain.clone(), config.account.clone())
        .with_metadata(metadata)
        .with_instructions([InstructionBox::from(verifying_keys::RegisterVerifyingKey {
            id: id.clone(),
            record,
        })])
        .sign(config.key_pair.private_key());
    let hash = match client.submit_transaction_blocking(&tx) {
        Ok(hash) => hash,
        Err(err) => {
            let message = err.to_string();
            if message.contains("Repeated instruction")
                || message.contains("Repetition of `Register` for id `VerifyingKey")
            {
                eprintln!("ivm_execution_vk_existing={}", id.name);
                return Ok(id);
            }
            return Err(err).wrap_err("failed to submit IVM execution VK registration");
        }
    };
    eprintln!("ivm_execution_vk_registered={hash}");
    Ok(id)
}

fn ivm_request_value(
    vk_ref: &VerifyingKeyId,
    config: &Config,
    metadata: &Metadata,
    bytecode: &IvmBytecode,
) -> Result<norito::json::Value> {
    let _chain_discriminant = ChainDiscriminantGuard::enter(config.account_chain_discriminant);
    let mut object = norito::json::Map::new();
    object.insert("vk_ref".to_owned(), norito::json::to_value(vk_ref)?);
    object.insert(
        "authority".to_owned(),
        norito::json::to_value(&config.account)?,
    );
    object.insert("metadata".to_owned(), norito::json::to_value(metadata)?);
    object.insert("bytecode".to_owned(), norito::json::to_value(bytecode)?);
    Ok(norito::json::Value::Object(object))
}

fn proved_from_derive_response(value: norito::json::Value) -> Result<IvmProved> {
    let proved = value
        .as_object()
        .and_then(|object| object.get("proved"))
        .cloned()
        .ok_or_else(|| eyre!("derive response missing `proved`"))?;
    norito::json::from_value(proved).wrap_err("failed to decode derived IvmProved")
}

fn prove_ivm_execution_attachment(
    vk_ref: VerifyingKeyId,
    proved: &IvmProved,
) -> Result<ProofAttachment> {
    let parsed = ivm::ProgramMetadata::parse(proved.bytecode.as_ref())
        .map_err(|_| eyre!("invalid IVM header in derived proved payload"))?;
    let body = proved
        .bytecode
        .as_ref()
        .get(parsed.header_len..)
        .ok_or_else(|| eyre!("invalid IVM header in derived proved payload"))?;
    let code_hash = Hash::new(body);
    let overlay_bytes =
        norito::to_bytes(&proved.overlay).wrap_err("failed to encode proved overlay")?;
    let overlay_hash = Hash::new(&overlay_bytes);
    let vk_box = iroha_core::zk::halo2_ipa_ivm_execution_vk_box()
        .map_err(|err| eyre!("failed to build ivm-execution-v1 VK: {err}"))?;
    let proof = iroha_core::zk::prove_halo2_ipa_ivm_execution_envelope(
        iroha_core::zk::IVM_EXECUTION_V1_CIRCUIT_ID,
        &vk_box,
        code_hash,
        overlay_hash,
        proved.events_commitment,
        proved.gas_policy_commitment,
        None,
    )
    .map_err(|err| eyre!("failed to prove ivm-execution-v1 envelope: {err}"))?;
    Ok(ProofAttachment::new_ref(
        iroha_core::zk::ZK_BACKEND_HALO2_IPA.to_owned(),
        proof,
        vk_ref,
    ))
}

fn submit_sccp_transfer_ivm_proved(
    config_path: PathBuf,
    vk_name: String,
    gas_asset_id: Option<String>,
    gas_limit: u64,
    max_cycles: u64,
    source_domain: u32,
    dest_domain: u32,
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
) -> Result<()> {
    let config = load_config(&config_path)?;
    let client = Client::new(config.clone());
    let vk_ref = ensure_ivm_execution_vk(
        &client,
        &config,
        &vk_name,
        gas_asset_id.as_deref(),
        DEFAULT_LEDGER_GAS_LIMIT,
    )?;
    let (message_id, payload_bytes) = record_sccp_transfer_payload_bytes(
        source_domain,
        dest_domain,
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
    let instruction = InstructionBox::from(RecordSccpMessage::new(payload_bytes));
    let program = build_record_instruction_program(&instruction, max_cycles)?;
    let bytecode = IvmBytecode::from_compiled(program);
    let metadata = tx_metadata(gas_asset_id.as_deref(), gas_limit)?;
    let request = ivm_request_value(&vk_ref, &config, &metadata, &bytecode)?;
    let proved = proved_from_derive_response(
        client
            .post_zk_ivm_derive_json(&request)
            .wrap_err("failed to derive IVM proved payload via Torii")?,
    )?;
    let attachment = prove_ivm_execution_attachment(vk_ref, &proved)?;
    let tx = TransactionBuilder::new(config.chain.clone(), config.account.clone())
        .with_metadata(metadata)
        .with_executable(Executable::IvmProved(proved))
        .with_attachments(ProofAttachmentList(vec![attachment]))
        .sign(config.key_pair.private_key());
    let tx_hash = client
        .submit_transaction_blocking(&tx)
        .wrap_err("failed to submit SCCP IVM-proved transaction")?;

    let mut output = norito::json::Map::new();
    output.insert("message_id".to_owned(), message_id.into());
    output.insert("tx_hash".to_owned(), tx_hash.to_string().into());
    output.insert("vk_name".to_owned(), vk_name.into());
    print_json_value(&norito::json::Value::Object(output))
}

#[allow(clippy::too_many_arguments)]
fn build_sccp_transfer_ivm_derive_request(
    config_path: PathBuf,
    vk_name: String,
    gas_asset_id: Option<String>,
    gas_limit: u64,
    max_cycles: u64,
    source_domain: u32,
    dest_domain: u32,
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
) -> Result<()> {
    let config = load_config(&config_path)?;
    let vk_ref = ivm_execution_vk_id(&vk_name);
    let (message_id, payload_bytes) = record_sccp_transfer_payload_bytes(
        source_domain,
        dest_domain,
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
    let instruction = InstructionBox::from(RecordSccpMessage::new(payload_bytes));
    let program = build_record_instruction_program(&instruction, max_cycles)?;
    let bytecode = IvmBytecode::from_compiled(program);
    let metadata = tx_metadata(gas_asset_id.as_deref(), gas_limit)?;
    let request = ivm_request_value(&vk_ref, &config, &metadata, &bytecode)?;

    let mut output = norito::json::Map::new();
    output.insert("message_id".to_owned(), message_id.into());
    output.insert("request".to_owned(), request);
    print_json_value(&norito::json::Value::Object(output))
}

#[allow(clippy::too_many_arguments)]
fn record_sccp_transfer_payload_bytes(
    source_domain: u32,
    dest_domain: u32,
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
) -> Result<(String, Vec<u8>)> {
    let payload = SccpPayloadV1::Transfer(TransferPayloadV1 {
        version: 1,
        source_domain,
        dest_domain,
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
    let message_id = hex::encode(sccp_message_id(&payload));
    let payload_bytes = canonical_sccp_payload_bytes(&payload);
    Ok((message_id, payload_bytes))
}

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
            source_domain,
            dest_domain,
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
            let (message_id, payload_bytes) = record_sccp_transfer_payload_bytes(
                source_domain,
                dest_domain,
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
            let instruction = InstructionBox::from(RecordSccpMessage::new(payload_bytes));
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
        Command::RecordSccpTransferIvmProved {
            config,
            vk_name,
            gas_asset_id,
            gas_limit,
            max_cycles,
            source_domain,
            dest_domain,
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
        } => submit_sccp_transfer_ivm_proved(
            config,
            vk_name,
            gas_asset_id,
            gas_limit,
            max_cycles,
            source_domain,
            dest_domain,
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
        )?,
        Command::BuildSccpTransferIvmDeriveRequest {
            config,
            vk_name,
            gas_asset_id,
            gas_limit,
            max_cycles,
            source_domain,
            dest_domain,
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
        } => build_sccp_transfer_ivm_derive_request(
            config,
            vk_name,
            gas_asset_id,
            gas_limit,
            max_cycles,
            source_domain,
            dest_domain,
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

    fn test_config_with_chain_discriminant(chain_discriminant: u16) -> Config {
        let key_pair = KeyPair::from_seed(vec![42u8; 32], Algorithm::Ed25519);
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
    fn ivm_request_value_uses_config_chain_discriminant_for_authority() {
        let config = test_config_with_chain_discriminant(369);
        let metadata = tx_metadata(None, 50_000_000).expect("metadata");
        let program = ivm::ProgramMetadata {
            mode: ivm::ivm_mode::ZK,
            ..Default::default()
        }
        .encode();
        let request = ivm_request_value(
            &ivm_execution_vk_id("ivm_execution"),
            &config,
            &metadata,
            &IvmBytecode::from_compiled(program),
        )
        .expect("request");
        let authority = request
            .as_object()
            .and_then(|object| object.get("authority"))
            .and_then(norito::json::Value::as_str)
            .expect("authority string");

        assert!(
            authority.starts_with("test"),
            "expected Taira/testnet I105 prefix, got {authority}"
        );
        let _chain_discriminant = ChainDiscriminantGuard::enter(config.account_chain_discriminant);
        let parsed = AccountId::parse_encoded(authority)
            .expect("authority should parse under config discriminant")
            .into_account_id();
        assert_eq!(parsed, config.account);
    }

    #[test]
    fn record_instruction_program_publishes_literal_tlv_before_execute_instruction() {
        let instruction = InstructionBox::from(RecordSccpMessage::new(vec![0xCA, 0xFE]));
        let program =
            build_record_instruction_program(&instruction, DEFAULT_MAX_CYCLES).expect("program");
        let parsed = ivm::ProgramMetadata::parse(&program).expect("valid IVM metadata");
        let code = &program[parsed.code_offset..];

        let mut syscalls = Vec::new();
        for word_bytes in code.chunks_exact(4) {
            let word = u32::from_le_bytes(word_bytes.try_into().expect("word-sized chunk"));
            let (op, syscall) = ivm::encoding::wide::decode_sys(word);
            if op == ivm::instruction::wide::system::SCALL {
                syscalls.push(u32::from(syscall));
            }
        }

        assert_eq!(
            syscalls,
            vec![
                ivm::syscalls::SYSCALL_INPUT_PUBLISH_TLV,
                ivm::syscalls::SYSCALL_SMARTCONTRACT_EXECUTE_INSTRUCTION,
            ]
        );
        let halt = u32::from_le_bytes(
            code[code.len() - 4..]
                .try_into()
                .expect("program ends with a full word"),
        );
        assert_eq!(halt, ivm::encoding::wide::encode_halt());
    }

    #[test]
    fn record_sccp_transfer_payload_rejects_noncanonical_evm_recipient() {
        let err = record_sccp_transfer_payload_bytes(
            0,
            1,
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
        let (message_id, payload_bytes) = record_sccp_transfer_payload_bytes(
            0,
            4,
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
        assert!(!payload_bytes.is_empty());
    }
}
