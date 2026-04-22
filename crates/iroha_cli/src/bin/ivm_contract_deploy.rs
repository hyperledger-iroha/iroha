//! Deploy a contract by registering code/manifest through an IVM transaction,
//! then activating and alias-binding it through a plain instruction batch.

use std::{fs, path::{Path, PathBuf}, str::FromStr, time::Duration};

use clap::Parser;
use eyre::{Result, WrapErr as _, eyre};
use iroha::{
    account_address::parse_account_address,
    client::Client,
    config::{self, Config},
    data_model::{
        account::Account,
        isi::{
            contract_alias::SetContractAlias,
            smart_contract_code::{
                ActivateContractInstance, RegisterSmartContractBytes, RegisterSmartContractCode,
            },
        },
        metadata::Metadata,
        name::Name,
        prelude::*,
        query::account::prelude::FindAccountById,
        smart_contract::{CONTRACT_DEPLOY_NONCE_METADATA_KEY, ContractAlias},
        transaction::{Executable, IvmBytecode, TransactionBuilder},
    },
};
use iroha_config::parameters::{
    actual::SorafsRolloutPhase,
    defaults::{
        sorafs::gateway::{DEFAULT_ANONYMITY_POLICY, DEFAULT_ROLLOUT_PHASE},
        torii,
    },
};
use iroha_crypto::{Hash, KeyPair, PrivateKey};
use iroha_primitives::json::Json;
use iroha_version::codec::EncodeVersioned;
use sorafs_manifest::alias_cache::AliasCachePolicy;
use sorafs_orchestrator::AnonymityPolicy;
use url::Url;

const DEFAULT_CHAIN_DISCRIMINANT_TAIRA: u16 = 369;
const DEFAULT_IVM_GAS_LIMIT: u64 = 1_000_000;
const DEFAULT_MAX_CYCLES: u64 = 1_000_000;
const MAX_DIRECT_REGISTER_BYTES_TX_BYTES: usize = 40_000;
const STAGED_REGISTER_CHUNK_BYTES: usize = 24_000;
const COPY_WORD_BYTES: usize = 8;
const LITERAL_DATA_START: i16 = 16;
const WIDE_IMM_MIN: i64 = -128;
const WIDE_IMM_MAX: i64 = 127;

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
    code_file: PathBuf,
    #[arg(long)]
    contract_alias: String,
    #[arg(long, default_value_t = DEFAULT_CHAIN_DISCRIMINANT_TAIRA)]
    chain_discriminant: u16,
    #[arg(long)]
    gas_asset_id: Option<String>,
    #[arg(long, default_value_t = DEFAULT_IVM_GAS_LIMIT)]
    gas_limit: u64,
    #[arg(long)]
    out_dir: Option<PathBuf>,
    #[arg(long, default_value_t = false)]
    emit_only: bool,
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
        torii_request_timeout: config::DEFAULT_TORII_REQUEST_TIMEOUT,
        transaction_ttl: config::DEFAULT_TRANSACTION_TIME_TO_LIVE,
        transaction_status_timeout: config::DEFAULT_TRANSACTION_STATUS_TIMEOUT,
        transaction_add_nonce: false,
        connect_queue_root: config::default_connect_queue_root(),
        soracloud_http_witness_file: None,
        sorafs_alias_cache: default_alias_cache_policy(),
        sorafs_anonymity_policy: default_anonymity_policy(),
        sorafs_rollout_phase: default_rollout_phase(),
    };
    Ok(Client::new(config))
}

fn insert_string_metadata(metadata: &mut Metadata, key: &str, value: impl Into<String>) -> Result<()> {
    metadata.insert(Name::from_str(key)?, Json::new(value.into()));
    Ok(())
}

fn insert_gas_asset_id(metadata: &mut Metadata, gas_asset_id: Option<&str>) -> Result<()> {
    if let Some(asset_id) = gas_asset_id.filter(|value| !value.trim().is_empty()) {
        insert_string_metadata(metadata, "gas_asset_id", asset_id.trim().to_owned())?;
    }
    Ok(())
}

fn ivm_transaction_metadata(
    gas_asset_id: Option<&str>,
    gas_limit: u64,
    contract_address: &iroha::data_model::smart_contract::ContractAddress,
) -> Result<Metadata> {
    let mut metadata = Metadata::default();
    insert_gas_asset_id(&mut metadata, gas_asset_id)?;
    iroha::data_model::transaction::insert_transaction_gas_limit(&mut metadata, gas_limit);
    insert_string_metadata(
        &mut metadata,
        "gov_contract_address",
        contract_address.to_string(),
    )?;
    insert_string_metadata(
        &mut metadata,
        "contract_address",
        contract_address.to_string(),
    )?;
    Ok(metadata)
}

fn instruction_transaction_metadata(
    gas_asset_id: Option<&str>,
    contract_address: &iroha::data_model::smart_contract::ContractAddress,
) -> Result<Metadata> {
    let mut metadata = Metadata::default();
    insert_gas_asset_id(&mut metadata, gas_asset_id)?;
    insert_string_metadata(
        &mut metadata,
        "gov_contract_address",
        contract_address.to_string(),
    )?;
    insert_string_metadata(
        &mut metadata,
        "contract_address",
        contract_address.to_string(),
    )?;
    Ok(metadata)
}

fn current_deploy_nonce(client: &Client, authority: &AccountId) -> Result<u64> {
    let account: Account = client
        .query_single(FindAccountById::new(authority.clone()))
        .wrap_err_with(|| format!("account `{authority}` not found"))?;
    let nonce_key =
        Name::from_str(CONTRACT_DEPLOY_NONCE_METADATA_KEY).expect("static metadata key is valid");
    account
        .metadata()
        .get(&nonce_key)
        .map(|value| {
            value
                .try_into_any_norito::<u64>()
                .map_err(|_| eyre!("`{CONTRACT_DEPLOY_NONCE_METADATA_KEY}` metadata is not a u64"))
        })
        .transpose()
        .map(|value| value.unwrap_or(0))
}

fn resolve_alias_dataspace(alias: &ContractAlias) -> Result<DataSpaceId> {
    match alias.dataspace_segment() {
        "universal" => Ok(DataSpaceId::UNIVERSAL),
        "governance" => Ok(DataSpaceId::new(1)),
        "zk" => Ok(DataSpaceId::new(2)),
        other => Err(eyre!(
            "unsupported dataspace alias `{other}`; only universal/governance/zk are supported by this helper"
        )),
    }
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

fn typed_tlv<T: norito::NoritoSerialize>(pointer_type: ivm::PointerType, value: &T) -> Result<Vec<u8>> {
    let payload = norito::to_bytes(value)?;
    Ok(make_tlv(pointer_type as u16, &payload))
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
            ivm::encoding::wide::encode_ri(
                ivm::instruction::wide::arithmetic::ADDI,
                rd,
                rs1,
                0,
            ),
        );
    }
    while value != 0 {
        let chunk = chunk_immediate(value);
        push_word(
            code,
            ivm::encoding::wide::encode_ri(
                ivm::instruction::wide::arithmetic::ADDI,
                rd,
                rd,
                chunk,
            ),
        );
        value -= chunk as i64;
    }
}

fn norito_tlv<T: norito::NoritoSerialize>(value: &T) -> Result<Vec<u8>> {
    let payload = norito::to_bytes(value)?;
    Ok(make_tlv(ivm::PointerType::NoritoBytes as u16, &payload))
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

fn emit_load64(code: &mut Vec<u8>, rd: u8, base: u8, imm: i8) {
    push_word(
        code,
        ivm::encoding::wide::encode_load(ivm::instruction::wide::memory::LOAD64, rd, base, imm),
    );
}

fn emit_store64(code: &mut Vec<u8>, base: u8, rs: u8, imm: i8) {
    push_word(
        code,
        ivm::encoding::wide::encode_store(ivm::instruction::wide::memory::STORE64, base, rs, imm),
    );
}

fn emit_rr(code: &mut Vec<u8>, opcode: u8, rd: u8, rs1: u8, rs2: u8) {
    push_word(code, ivm::encoding::wide::encode_rr(opcode, rd, rs1, rs2));
}

fn assemble_program_with_literals(code: &[u8], literal_data: &[u8]) -> Vec<u8> {
    let mut program = Vec::new();
    program.extend_from_slice(b"IVM\0");
    program.extend_from_slice(&[1, 1, 0, 4]);
    program.extend_from_slice(&DEFAULT_MAX_CYCLES.to_le_bytes());
    program.push(1);
    if !literal_data.is_empty() {
        program.extend_from_slice(b"LTLB");
        program.extend_from_slice(&0u32.to_le_bytes());
        program.extend_from_slice(&0u32.to_le_bytes());
        program.extend_from_slice(&(literal_data.len() as u32).to_le_bytes());
        program.extend_from_slice(literal_data);
    }
    program.extend_from_slice(code);
    program
}

fn literal_ptr(offset: usize) -> Result<i16> {
    i16::try_from(offset).map_err(|_| eyre!("literal section grew beyond i16 addressable range"))
}

fn emit_syscall_for_literal(code: &mut Vec<u8>, ptr: i16, syscall: u32) -> Result<()> {
    emit_addi(code, 10, 0, i64::from(ptr));
    code.extend_from_slice(
        &ivm::encoding::wide::encode_sys(
            ivm::instruction::wide::system::SCALL,
            u8::try_from(ivm::syscalls::SYSCALL_INPUT_PUBLISH_TLV)
                .map_err(|_| eyre!("input publish syscall id does not fit in u8"))?,
        )
        .to_le_bytes(),
    );
    code.extend_from_slice(
        &ivm::encoding::wide::encode_sys(
            ivm::instruction::wide::system::SCALL,
            u8::try_from(syscall).map_err(|_| eyre!("syscall id does not fit in u8"))?,
        )
        .to_le_bytes(),
    );
    Ok(())
}

fn build_single_register_program<T: norito::NoritoSerialize>(
    payload: &T,
    syscall: u32,
) -> Result<Vec<u8>> {
    let tlv = norito_tlv(payload)?;
    let ptr = literal_ptr(LITERAL_DATA_START as usize)?;
    let mut code = Vec::new();
    emit_syscall_for_literal(&mut code, ptr, syscall)?;
    code.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
    Ok(assemble_program_with_literals(&code, &tlv))
}

fn build_state_set_program(path: &Name, value: &[u8]) -> Result<Vec<u8>> {
    let path_tlv = typed_tlv(ivm::PointerType::Name, path)?;
    let value_tlv = make_tlv(ivm::PointerType::NoritoBytes as u16, value);
    let path_ptr = literal_ptr(LITERAL_DATA_START as usize)?;
    let value_ptr = literal_ptr(LITERAL_DATA_START as usize + path_tlv.len())?;
    let mut literal_data = Vec::with_capacity(path_tlv.len() + value_tlv.len());
    literal_data.extend_from_slice(&path_tlv);
    literal_data.extend_from_slice(&value_tlv);

    let mut code = Vec::new();
    emit_addi(&mut code, 10, 0, i64::from(path_ptr));
    push_syscall(&mut code, ivm::syscalls::SYSCALL_INPUT_PUBLISH_TLV)?;
    emit_addi(&mut code, 12, 10, 0);
    emit_addi(&mut code, 10, 0, i64::from(value_ptr));
    push_syscall(&mut code, ivm::syscalls::SYSCALL_INPUT_PUBLISH_TLV)?;
    emit_addi(&mut code, 11, 10, 0);
    emit_addi(&mut code, 10, 12, 0);
    push_syscall(&mut code, ivm::syscalls::SYSCALL_STATE_SET)?;
    push_word(&mut code, ivm::encoding::wide::encode_halt());
    Ok(assemble_program_with_literals(&code, &literal_data))
}

fn build_staged_register_program_inner(
    paths: &[Name],
    chunk_sizes: &[usize],
    emit_register: bool,
    emit_cleanup: bool,
) -> Result<Vec<u8>> {
    if paths.len() != chunk_sizes.len() {
        return Err(eyre!("chunk path count and size count must match"));
    }
    if chunk_sizes.is_empty() {
        return Err(eyre!("staged register requires at least one chunk"));
    }

    let mut literal_data = Vec::new();
    let mut path_ptrs = Vec::with_capacity(paths.len());
    for path in paths {
        path_ptrs.push(literal_ptr(LITERAL_DATA_START as usize + literal_data.len())?);
        literal_data.extend_from_slice(&typed_tlv(ivm::PointerType::Name, path)?);
    }

    let total_len: usize = chunk_sizes.iter().sum();
    let alloc_len = total_len.next_multiple_of(COPY_WORD_BYTES);
    let mut code = Vec::new();

    emit_addi(&mut code, 10, 0, alloc_len as i64);
    push_syscall(&mut code, ivm::syscalls::SYSCALL_ALLOC)?;
    emit_addi(&mut code, 1, 10, 0);
    emit_addi(&mut code, 8, 10, 0);
    emit_addi(&mut code, 5, 0, 8);
    emit_addi(&mut code, 6, 0, 56);

    for (path_ptr, chunk_size) in path_ptrs.into_iter().zip(chunk_sizes.iter().copied()) {
        let iterations = chunk_size.div_ceil(COPY_WORD_BYTES);
        if iterations == 0 {
            continue;
        }

        emit_addi(&mut code, 10, 0, i64::from(path_ptr));
        push_syscall(&mut code, ivm::syscalls::SYSCALL_INPUT_PUBLISH_TLV)?;
        push_syscall(&mut code, ivm::syscalls::SYSCALL_STATE_GET)?;
        emit_addi(&mut code, 2, 10, 0);
        emit_addi(&mut code, 4, 0, iterations as i64);

        emit_load64(&mut code, 3, 2, 0);
        emit_load64(&mut code, 7, 2, 8);
        emit_rr(
            &mut code,
            ivm::instruction::wide::arithmetic::SRL,
            9,
            3,
            6,
        );
        emit_rr(
            &mut code,
            ivm::instruction::wide::arithmetic::SLL,
            12,
            7,
            5,
        );
        emit_rr(
            &mut code,
            ivm::instruction::wide::arithmetic::OR,
            9,
            9,
            12,
        );
        emit_store64(&mut code, 1, 9, 0);
        emit_addi(&mut code, 2, 2, COPY_WORD_BYTES as i64);
        emit_addi(&mut code, 1, 1, COPY_WORD_BYTES as i64);
        emit_addi(&mut code, 4, 4, -1);
        push_word(
            &mut code,
            ivm::encoding::wide::encode_branch(
                ivm::instruction::wide::control::BNE,
                4,
                0,
                -9,
            ),
        );
    }

    if emit_register {
        emit_addi(&mut code, 10, 8, 0);
        push_syscall(&mut code, ivm::syscalls::SYSCALL_REGISTER_SMART_CONTRACT_BYTES)?;
    }

    if emit_cleanup {
        for path_ptr in paths.iter().scan(0usize, |offset, path| {
            let ptr = literal_ptr(LITERAL_DATA_START as usize + *offset).ok()?;
            *offset += typed_tlv(ivm::PointerType::Name, path).ok()?.len();
            Some(ptr)
        }) {
            emit_addi(&mut code, 10, 0, i64::from(path_ptr));
            push_syscall(&mut code, ivm::syscalls::SYSCALL_INPUT_PUBLISH_TLV)?;
            push_syscall(&mut code, ivm::syscalls::SYSCALL_STATE_DEL)?;
        }
    }

    push_word(&mut code, ivm::encoding::wide::encode_halt());
    Ok(assemble_program_with_literals(&code, &literal_data))
}

fn build_staged_register_program(paths: &[Name], chunk_sizes: &[usize]) -> Result<Vec<u8>> {
    build_staged_register_program_inner(paths, chunk_sizes, true, true)
}

#[cfg(test)]
fn build_staged_copy_program(paths: &[Name], chunk_sizes: &[usize]) -> Result<Vec<u8>> {
    build_staged_register_program_inner(paths, chunk_sizes, false, false)
}

#[cfg(test)]
fn build_staged_register_only_program(paths: &[Name], chunk_sizes: &[usize]) -> Result<Vec<u8>> {
    build_staged_register_program_inner(paths, chunk_sizes, true, false)
}

fn staged_chunk_paths(code_hash: Hash, chunk_count: usize) -> Result<Vec<Name>> {
    let code_hash_hex = hex::encode(<[u8; 32]>::from(code_hash));
    let prefix = &code_hash_hex[..12];
    (0..chunk_count)
        .map(|index| Name::from_str(&format!("cd_{prefix}_{index:03}")).map_err(Into::into))
        .collect()
}

fn split_bytes(bytes: &[u8], chunk_size: usize) -> Vec<Vec<u8>> {
    bytes
        .chunks(chunk_size)
        .map(|chunk| chunk.to_vec())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn staged_copy_program_reconstructs_register_request_tlv() {
        let request = RegisterSmartContractBytes {
            code_hash: Hash::new(b"stage-test"),
            code: (0..59_201).map(|index| (index % 251) as u8).collect(),
        };
        let register_request_tlv = norito_tlv(&request).expect("encode register request tlv");
        let chunks = split_bytes(&register_request_tlv, STAGED_REGISTER_CHUNK_BYTES);
        let chunk_sizes: Vec<_> = chunks.iter().map(Vec::len).collect();
        let chunk_paths =
            staged_chunk_paths(Hash::new(b"stage-copy-test"), chunks.len()).expect("chunk paths");
        let program =
            build_staged_copy_program(&chunk_paths, &chunk_sizes).expect("build staged copy");

        let mut vm = ivm::IVM::new(DEFAULT_MAX_CYCLES);
        vm.set_host(ivm::CoreHost::new());
        {
            let host = vm
                .host_mut_any()
                .and_then(|any| any.downcast_mut::<ivm::CoreHost>())
                .expect("downcast core host");
            for (path, chunk) in chunk_paths.iter().zip(&chunks) {
                host.insert_state_value(path.as_ref(), chunk);
            }
        }
        vm.load_program(&program).expect("load copy program");
        vm.run().expect("run staged copy");

        let heap_ptr = vm.register(8);
        let copied = vm
            .memory
            .load_region(heap_ptr, register_request_tlv.len() as u64)
            .expect("read reconstructed heap bytes");
        assert_eq!(copied, register_request_tlv);
        ivm::pointer_abi::validate_tlv_bytes(&copied).expect("reconstructed bytes must form a TLV");
    }

    #[test]
    fn staged_register_program_runs_under_contract_runtime_host() {
        let authority = AccountId::of(iroha_crypto::KeyPair::random().public_key().clone());
        let request = RegisterSmartContractBytes {
            code_hash: Hash::new(b"stage-runtime"),
            code: (0..59_201).map(|index| (index % 251) as u8).collect(),
        };
        let register_request_tlv = norito_tlv(&request).expect("encode register request tlv");
        let chunks = split_bytes(&register_request_tlv, STAGED_REGISTER_CHUNK_BYTES);
        let chunk_sizes: Vec<_> = chunks.iter().map(Vec::len).collect();
        let chunk_paths =
            staged_chunk_paths(Hash::new(b"stage-runtime-test"), chunks.len()).expect("chunk paths");
        let copy_program =
            build_staged_copy_program(&chunk_paths, &chunk_sizes).expect("build staged copy");
        let register_program = build_staged_register_only_program(&chunk_paths, &chunk_sizes)
            .expect("build staged register");

        let mut snapshot = BTreeMap::new();
        for (path, chunk) in chunk_paths.iter().zip(&chunks) {
            snapshot.insert(
                path.clone(),
                make_tlv(ivm::PointerType::NoritoBytes as u16, chunk),
            );
        }

        let mut copy_vm = ivm::IVM::new(DEFAULT_MAX_CYCLES);
        let mut copy_host = iroha_core::smartcontracts::ivm::host::CoreHost::new(authority.clone());
        copy_host.set_durable_state_snapshot(snapshot.clone());
        copy_vm.set_host(copy_host);
        copy_vm
            .load_program(&copy_program)
            .expect("load staged copy program");
        copy_vm.run().expect("run staged copy program");
        let heap_ptr = copy_vm.register(8);
        let copied = copy_vm
            .memory
            .load_region(heap_ptr, register_request_tlv.len() as u64)
            .expect("read reconstructed heap bytes");
        assert_eq!(copied, register_request_tlv);

        let mut register_vm = ivm::IVM::new(DEFAULT_MAX_CYCLES);
        let mut register_host = iroha_core::smartcontracts::ivm::host::CoreHost::new(authority);
        register_host.set_durable_state_snapshot(snapshot);
        register_vm.set_host(register_host);
        register_vm
            .load_program(&register_program)
            .expect("load staged register program");
        register_vm
            .run()
            .expect("run staged register program");
    }
}

fn sign_ivm_transaction(
    chain_id: &ChainId,
    authority: &AccountId,
    private_key: &PrivateKey,
    metadata: Metadata,
    program: Vec<u8>,
) -> SignedTransaction {
    TransactionBuilder::new(chain_id.clone(), authority.clone())
        .with_metadata(metadata)
        .with_executable(Executable::Ivm(IvmBytecode::from_compiled(program)))
        .sign(private_key)
}

fn sign_instruction_transaction(
    chain_id: &ChainId,
    authority: &AccountId,
    private_key: &PrivateKey,
    metadata: Metadata,
    instructions: impl IntoIterator<Item = InstructionBox>,
) -> SignedTransaction {
    TransactionBuilder::new(chain_id.clone(), authority.clone())
        .with_metadata(metadata)
        .with_instructions(instructions)
        .sign(private_key)
}

fn write_tx(out_dir: &Path, stem: &str, tx: &SignedTransaction) -> Result<(PathBuf, usize)> {
    fs::create_dir_all(out_dir)
        .wrap_err_with(|| format!("create output directory {}", out_dir.display()))?;
    let path = out_dir.join(format!("{stem}.norito"));
    let bytes = tx.encode_versioned();
    fs::write(&path, &bytes).wrap_err_with(|| format!("write {}", path.display()))?;
    Ok((path, bytes.len()))
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
        key_pair.clone(),
    )?;

    let contract_alias: ContractAlias = args
        .contract_alias
        .parse()
        .wrap_err("failed to parse --contract-alias")?;
    let dataspace_id = resolve_alias_dataspace(&contract_alias)?;
    let deploy_nonce = current_deploy_nonce(&client, &authority)?;
    let next_nonce = deploy_nonce
        .checked_add(1)
        .ok_or_else(|| eyre!("deploy nonce overflow"))?;
    let contract_address = iroha::data_model::smart_contract::ContractAddress::derive(
        args.chain_discriminant,
        &authority,
        deploy_nonce,
        dataspace_id,
    )
    .map_err(|err| eyre!(err.to_string()))
    .wrap_err("failed to derive contract address")?;

    let code = fs::read(&args.code_file)
        .wrap_err_with(|| format!("read {}", args.code_file.display()))?;
    let verified = ivm::verify_contract_artifact(&code)
        .map_err(|err| eyre!("verify contract artifact: {err}"))?;
    let manifest = verified.manifest.signed(&key_pair);
    let code_hash = verified.code_hash;
    let tx_metadata = ivm_transaction_metadata(
        args.gas_asset_id.as_deref(),
        args.gas_limit,
        &contract_address,
    )?;
    let register_request = RegisterSmartContractBytes {
        code_hash,
        code: code.clone(),
    };
    let register_bytes_program = build_single_register_program(
        &register_request,
        ivm::syscalls::SYSCALL_REGISTER_SMART_CONTRACT_BYTES,
    )?;
    let direct_register_bytes_tx = sign_ivm_transaction(
        &client.chain,
        &authority,
        &private_key,
        tx_metadata.clone(),
        register_bytes_program,
    );
    let direct_register_bytes_tx_size = direct_register_bytes_tx.encode_versioned().len();
    let use_staged_register =
        direct_register_bytes_tx_size > MAX_DIRECT_REGISTER_BYTES_TX_BYTES;

    let mut register_stage_tx_hashes = Vec::new();
    let mut register_plans: Vec<(String, String, SignedTransaction)> = Vec::new();
    let register_bytes_tx_hash;
    let register_bytes_tx_strategy;
    if use_staged_register {
        let register_request_tlv = norito_tlv(&register_request)?;
        let chunks = split_bytes(&register_request_tlv, STAGED_REGISTER_CHUNK_BYTES);
        let chunk_paths = staged_chunk_paths(code_hash, chunks.len())?;
        for (index, (path, chunk)) in chunk_paths.iter().zip(chunks.iter()).enumerate() {
            let chunk_program = build_state_set_program(path, chunk)?;
            let chunk_tx = sign_ivm_transaction(
                &client.chain,
                &authority,
                &private_key,
                tx_metadata.clone(),
                chunk_program,
            );
            register_stage_tx_hashes.push(chunk_tx.hash().to_string());
            register_plans.push((
                format!("stage_register_bytes_chunk_{index:03}"),
                format!("stage-register-bytes-chunk-{index:03}"),
                chunk_tx,
            ));
        }
        let chunk_sizes = chunks.iter().map(Vec::len).collect::<Vec<_>>();
        let staged_register_program = build_staged_register_program(&chunk_paths, &chunk_sizes)?;
        let staged_register_tx = sign_ivm_transaction(
            &client.chain,
            &authority,
            &private_key,
            tx_metadata.clone(),
            staged_register_program,
        );
        register_bytes_tx_hash = staged_register_tx.hash();
        register_bytes_tx_strategy = "staged_state_chunks";
        register_plans.push((
            "register_bytes_via_ivm".to_owned(),
            "register-bytes-via-ivm".to_owned(),
            staged_register_tx,
        ));
    } else {
        register_bytes_tx_hash = direct_register_bytes_tx.hash();
        register_bytes_tx_strategy = "direct";
        register_plans.push((
            "register_bytes_via_ivm".to_owned(),
            "register-bytes-via-ivm".to_owned(),
            direct_register_bytes_tx,
        ));
    }
    let register_manifest_program = build_single_register_program(
        &RegisterSmartContractCode {
            manifest: manifest.clone(),
        },
        ivm::syscalls::SYSCALL_REGISTER_SMART_CONTRACT_CODE,
    )?;
    let register_manifest_tx = sign_ivm_transaction(
        &client.chain,
        &authority,
        &private_key,
        tx_metadata,
        register_manifest_program,
    );

    let nonce_key =
        Name::from_str(CONTRACT_DEPLOY_NONCE_METADATA_KEY).expect("static metadata key is valid");
    let activate_tx = sign_instruction_transaction(
        &client.chain,
        &authority,
        &private_key,
        instruction_transaction_metadata(args.gas_asset_id.as_deref(), &contract_address)?,
        [
            InstructionBox::from(ActivateContractInstance {
                contract_address: contract_address.clone(),
                code_hash,
            }),
            InstructionBox::from(SetContractAlias::bind(
                contract_address.clone(),
                contract_alias.clone(),
                None,
            )),
            InstructionBox::from(SetKeyValue::account(
                authority.clone(),
                nonce_key,
                Json::new(next_nonce),
            )),
        ],
    );

    let register_manifest_tx_hash = register_manifest_tx.hash();
    let activate_tx_hash = activate_tx.hash();
    let mut planned_txs = register_plans;
    planned_txs.push((
        "register_manifest_via_ivm".to_owned(),
        "register-manifest-via-ivm".to_owned(),
        register_manifest_tx,
    ));
    planned_txs.push(("activate".to_owned(), "activate".to_owned(), activate_tx));
    let written = if let Some(out_dir) = args.out_dir.as_deref() {
        Some(
            planned_txs
                .iter()
                .enumerate()
                .map(|(index, (name, slug, tx))| {
                    let stem = format!("{:02}-{slug}", index + 1);
                    Ok((name.as_str(), write_tx(out_dir, &stem, tx)?))
                })
                .collect::<Result<Vec<_>>>()?,
        )
    } else {
        None
    };
    if !args.emit_only {
        for (_, _, tx) in &planned_txs {
            client.submit_transaction_blocking(tx)?;
        }
    }

    let result = norito::json!({
        "ok": true,
        "submitted": (!args.emit_only),
        "torii_url": (args.torii_url),
        "chain_id": (args.chain_id),
        "authority": (authority),
        "contract_alias": (contract_alias),
        "contract_address": (contract_address),
        "deploy_nonce": (deploy_nonce),
        "next_deploy_nonce": (next_nonce),
        "code_hash_hex": (hex::encode(<[u8; 32]>::from(code_hash))),
        "register_bytes_tx_strategy": (register_bytes_tx_strategy),
        "direct_register_bytes_tx_size": (direct_register_bytes_tx_size as u64),
        "register_bytes_via_ivm_tx_hash": (register_bytes_tx_hash),
        "register_bytes_stage_tx_hashes": (register_stage_tx_hashes),
        "register_manifest_via_ivm_tx_hash": (register_manifest_tx_hash),
        "activate_tx_hash": (activate_tx_hash),
    });
    let mut result = result.as_object().cloned().ok_or_else(|| eyre!("expected object"))?;
    if let Some(written) = written {
        let files = written
            .into_iter()
            .map(|(name, (path, size))| {
                norito::json!({
                    "name": (name),
                    "path": (path.display().to_string()),
                    "size": (size as u64),
                })
            })
            .collect();
        result.insert("files".to_owned(), norito::json::Value::Array(files));
    }
    println!(
        "{}",
        norito::json::to_json_pretty(&norito::json::Value::Object(result))?
    );
    Ok(())
}
