#![allow(dead_code)]
use std::vec::Vec;

// --- CompactProofBundle helpers via syscalls (test-only utilities) ---
use iroha_data_model::prelude::*;
use iroha_primitives::json::Json;
use ivm::{IVM, PointerType, ProgramMetadata, encoding, instruction, syscalls};
use ivm_abi::metadata::LITERAL_SECTION_MAGIC;
use ivm_abi::state_value::{
    StateValueAtomV1, StateValueKindV1, StateValueNodeV1, StateValueRecordV1, StateValueSchemaV1,
    state_value_schema_hash_v1,
};

const HALT_WORD: u32 = encoding::wide::encode_halt();
pub const HALT: [u8; 4] = HALT_WORD.to_le_bytes();

/// Select a named Kotodama V1 entrypoint after loading its artifact.
///
/// V1 artifacts deliberately begin with a non-dispatching `HALT`; raw VM tests
/// must exercise the same CNTR selector-to-PC mapping used by production hosts
/// instead of relying on source declaration order.
pub fn select_kotodama_entrypoint(vm: &mut IVM, program: &[u8], name: &str) {
    let parsed = ProgramMetadata::parse(program).expect("parse Kotodama V1 artifact");
    let entrypoint = parsed
        .contract_interface
        .as_ref()
        .expect("Kotodama V1 artifact must embed CNTR")
        .entrypoints
        .iter()
        .find(|entrypoint| entrypoint.name == name)
        .unwrap_or_else(|| panic!("missing Kotodama V1 entrypoint `{name}`"));
    let pc =
        u64::try_from(parsed.prefix_len()).expect("program prefix fits u64") + entrypoint.entry_pc;
    vm.set_program_counter(pc)
        .unwrap_or_else(|error| panic!("select Kotodama V1 entrypoint `{name}`: {error:?}"));
}

fn i64_state_schema() -> StateValueSchemaV1 {
    StateValueSchemaV1 {
        nodes: vec![StateValueNodeV1::Leaf(StateValueKindV1::Int)],
    }
}

/// Encode an `i64` using the schema-bound Kotodama V1 durable-value record.
pub fn encode_i64_state_value(value: i64) -> Vec<u8> {
    let schema = i64_state_schema();
    let schema_bytes = norito::to_bytes(&schema).expect("encode i64 state schema");
    norito::to_bytes(&StateValueRecordV1 {
        schema_hash: state_value_schema_hash_v1(&schema_bytes),
        atoms: vec![StateValueAtomV1::Int(value)],
    })
    .expect("encode i64 state record")
}

/// Decode and validate an `i64` Kotodama V1 durable-value record.
pub fn decode_i64_state_value(payload: &[u8]) -> i64 {
    let schema = i64_state_schema();
    let schema_bytes = norito::to_bytes(&schema).expect("encode i64 state schema");
    let record: StateValueRecordV1 =
        norito::decode_from_bytes(payload).expect("decode i64 state record");
    assert_eq!(
        record.schema_hash,
        state_value_schema_hash_v1(&schema_bytes),
        "i64 state schema hash"
    );
    assert!(schema.validate_atoms(&record.atoms));
    let [StateValueAtomV1::Int(value)] = record.atoms.as_slice() else {
        panic!("i64 state record must contain exactly one integer atom");
    };
    *value
}

/// Encode one pointer-backed value using the schema-bound Kotodama V1 record.
pub fn encode_pointer_state_value(
    kind: StateValueKindV1,
    pointer_type: PointerType,
    payload: &[u8],
) -> Vec<u8> {
    assert!(kind.is_pointer(), "state kind must use a pointer word");
    let schema = StateValueSchemaV1 {
        nodes: vec![StateValueNodeV1::Leaf(kind)],
    };
    let schema_bytes = norito::to_bytes(&schema).expect("encode pointer state schema");
    let mut envelope = Vec::with_capacity(7 + payload.len() + iroha_crypto::Hash::LENGTH);
    envelope.extend_from_slice(&(pointer_type as u16).to_be_bytes());
    envelope.push(1);
    envelope.extend_from_slice(
        &u32::try_from(payload.len())
            .expect("test pointer payload fits u32")
            .to_be_bytes(),
    );
    envelope.extend_from_slice(payload);
    envelope.extend_from_slice(iroha_crypto::Hash::new(payload).as_ref());
    norito::to_bytes(&StateValueRecordV1 {
        schema_hash: state_value_schema_hash_v1(&schema_bytes),
        atoms: vec![StateValueAtomV1::Pointer(envelope)],
    })
    .expect("encode pointer state record")
}

/// Decode and structurally validate one pointer-backed Kotodama V1 state record.
pub fn decode_pointer_state_value(payload: &[u8], kind: StateValueKindV1) -> Vec<u8> {
    assert!(kind.is_pointer(), "state kind must use a pointer word");
    let schema = StateValueSchemaV1 {
        nodes: vec![StateValueNodeV1::Leaf(kind)],
    };
    let schema_bytes = norito::to_bytes(&schema).expect("encode pointer state schema");
    let record: StateValueRecordV1 =
        norito::decode_from_bytes(payload).expect("decode pointer state record");
    assert_eq!(
        record.schema_hash,
        state_value_schema_hash_v1(&schema_bytes),
        "pointer state schema hash"
    );
    assert!(schema.validate_atoms(&record.atoms));
    let [StateValueAtomV1::Pointer(envelope)] = record.atoms.as_slice() else {
        panic!("pointer state record must contain exactly one pointer atom");
    };
    envelope.clone()
}

fn assemble_words(words: &[u32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(words.len() * 4);
    for &word in words {
        bytes.extend_from_slice(&word.to_le_bytes());
    }
    assemble(&bytes)
}

fn syscall_prog(syscall: u8) -> Vec<u8> {
    assemble_words(&[
        encoding::wide::encode_sys(instruction::wide::system::SCALL, syscall),
        HALT_WORD,
    ])
}

/// Issue GET_MERKLE_COMPACT via SCALL and decode into a CompactProofBundle.
pub fn syscall_memory_compact_bundle(
    vm: &mut IVM,
    addr: u64,
    depth_cap: Option<usize>,
) -> ivm::merkle_utils::CompactProofBundle {
    let out_ptr = ivm::Memory::OUTPUT_START;
    let root_out = out_ptr + 8192;
    vm.set_register(10, addr);
    vm.set_register(11, out_ptr);
    vm.set_register(12, depth_cap.unwrap_or(0) as u64);
    vm.set_register(13, root_out);
    let prog = syscall_prog(syscalls::SYSCALL_GET_MERKLE_COMPACT as u8);
    vm.load_program(&prog).expect("load program");
    vm.run().expect("syscall");

    // Parse header and decode typed compact proof
    let mut hdr = [0u8; 1 + 4 + 4];
    vm.memory.load_bytes(out_ptr, &mut hdr).expect("hdr");
    let depth = hdr[0] as usize;
    let total = 1 + 4 + 4 + depth * 32;
    let mut buf = vec![0u8; total];
    vm.memory.load_bytes(out_ptr, &mut buf).expect("body");
    let (cp, _) = ivm::merkle_utils::decode_compact_proof_bytes(&buf).expect("decode");
    // Read root
    let mut root = [0u8; 32];
    vm.memory.load_bytes(root_out, &mut root).expect("root");
    // Build bundle
    let siblings: Vec<[u8; 32]> = cp
        .siblings()
        .iter()
        .map(|opt| opt.map(|h| *h.as_ref()).unwrap_or([0u8; 32]))
        .collect();
    ivm::merkle_utils::CompactProofBundle {
        depth: cp.depth(),
        dirs: cp.dirs(),
        siblings,
        root,
    }
}

/// Issue GET_REGISTER_MERKLE_COMPACT via SCALL and decode into a CompactProofBundle.
pub fn syscall_registers_compact_bundle(
    vm: &mut IVM,
    idx: usize,
    depth_cap: Option<usize>,
) -> ivm::merkle_utils::CompactProofBundle {
    let out_ptr = ivm::Memory::OUTPUT_START;
    let root_out = out_ptr + 12288;
    vm.set_register(10, idx as u64);
    vm.set_register(11, out_ptr);
    vm.set_register(12, depth_cap.unwrap_or(0) as u64);
    vm.set_register(13, root_out);
    let prog = syscall_prog(syscalls::SYSCALL_GET_REGISTER_MERKLE_COMPACT as u8);
    vm.load_program(&prog).expect("load program");
    vm.run().expect("syscall");

    // Parse header and decode typed compact proof
    let mut hdr = [0u8; 1 + 4 + 4];
    vm.memory.load_bytes(out_ptr, &mut hdr).expect("hdr");
    let depth = hdr[0] as usize;
    let total = 1 + 4 + 4 + depth * 32;
    let mut buf = vec![0u8; total];
    vm.memory.load_bytes(out_ptr, &mut buf).expect("body");
    let (cp, _) = ivm::merkle_utils::decode_compact_proof_bytes(&buf).expect("decode");
    // Read root
    let mut root = [0u8; 32];
    vm.memory.load_bytes(root_out, &mut root).expect("root");
    // Build bundle
    let siblings: Vec<[u8; 32]> = cp
        .siblings()
        .iter()
        .map(|opt| opt.map(|h| *h.as_ref()).unwrap_or([0u8; 32]))
        .collect();
    ivm::merkle_utils::CompactProofBundle {
        depth: cp.depth(),
        dirs: cp.dirs(),
        siblings,
        root,
    }
}

pub const MODE_VECTOR: u8 = 0x02;
pub const MODE_ZK: u8 = ivm::ivm_mode::ZK;

pub fn assemble_with_mode(code: &[u8], mode: u8) -> Vec<u8> {
    let vector_length = if (mode & MODE_VECTOR) != 0 { 4 } else { 0 };
    let meta = ProgramMetadata {
        mode,
        vector_length,
        max_cycles: 0,
        abi_version: 1,
        ..ProgramMetadata::default()
    };
    let mut v = meta.encode();
    v.extend_from_slice(code);
    v
}

pub fn assemble(code: &[u8]) -> Vec<u8> {
    assemble_with_mode(code, 0)
}

/// Assemble a program with an `LTLB` literal section and return the program
/// bytes plus the literal addresses inside the loaded code region.
pub fn assemble_with_literal_section(code: &[u8], literals: &[&[u8]]) -> (Vec<u8>, Vec<u64>) {
    let mut program = ProgramMetadata::default().encode();
    let offsets_len = literals.len() * 8;
    let data_start = 16 + offsets_len;
    let data_len: usize = literals.iter().map(|literal| literal.len()).sum();
    let mut offsets = Vec::with_capacity(literals.len());
    let mut data = Vec::with_capacity(data_len);
    let mut cursor = data_start as u64;
    for literal in literals {
        offsets.push(cursor);
        data.extend_from_slice(literal);
        cursor += literal.len() as u64;
    }
    program.extend_from_slice(&LITERAL_SECTION_MAGIC);
    program.extend_from_slice(&(literals.len() as u32).to_le_bytes());
    let post_pad = (4 - ((16 + offsets_len + data.len()) % 4)) % 4;
    program.extend_from_slice(&(post_pad as u32).to_le_bytes());
    program.extend_from_slice(&(data.len() as u32).to_le_bytes());
    for offset in &offsets {
        program.extend_from_slice(&offset.to_le_bytes());
    }
    program.extend_from_slice(&data);
    program.extend(std::iter::repeat_n(0u8, post_pad));
    let literal_addrs = offsets.into_iter().collect();
    program.extend_from_slice(code);
    (program, literal_addrs)
}

/// Assemble a program that consists of one or more SCALL instructions followed by HALT.
pub fn assemble_syscalls(syscalls: &[u8]) -> Vec<u8> {
    let mut code = Vec::with_capacity((syscalls.len() + 1) * 4);
    for &num in syscalls {
        let word = encoding::wide::encode_sys(instruction::wide::system::SCALL, num);
        code.extend_from_slice(&word.to_le_bytes());
    }
    code.extend_from_slice(&HALT);
    assemble(&code)
}

/// Assemble a SCALL/HALT program with a literal-table prefix and return the
/// program bytes plus literal addresses inside the loaded code region.
pub fn assemble_syscalls_with_literal_section(
    syscalls: &[u8],
    literals: &[&[u8]],
) -> (Vec<u8>, Vec<u64>) {
    let mut code = Vec::with_capacity((syscalls.len() + 1) * 4);
    for &num in syscalls {
        let word = encoding::wide::encode_sys(instruction::wide::system::SCALL, num);
        code.extend_from_slice(&word.to_le_bytes());
    }
    code.extend_from_slice(&HALT);
    assemble_with_literal_section(&code, literals)
}

pub fn assemble_zk(code: &[u8], max_cycles: u64) -> Vec<u8> {
    let mut header = assemble_with_mode(code, MODE_ZK);
    // overwrite max_cycles in header (bytes 8..16)
    header[8..16].copy_from_slice(&max_cycles.to_le_bytes());
    header
}

pub fn payload_for_type(pointer_type: PointerType, payload: &[u8]) -> Vec<u8> {
    match pointer_type {
        PointerType::AccountId => encode_account_id_payload(payload),
        PointerType::AssetDefinitionId => {
            encode_from_str::<AssetDefinitionId>(payload, "AssetDefinitionId")
        }
        PointerType::AssetId => encode_from_str::<AssetId>(payload, "AssetId"),
        PointerType::DomainId => encode_domain_id_payload(payload),
        PointerType::Name => encode_name_payload(payload),
        PointerType::NftId => encode_from_str::<NftId>(payload, "NftId"),
        PointerType::Json => encode_json_payload(payload),
        _ => payload.to_vec(),
    }
}

pub fn json_from_payload(payload: &[u8]) -> norito::json::Value {
    let json: Json = norito::decode_from_bytes(payload).expect("decode Json payload");
    norito::json::from_str(json.get()).expect("parse Json payload")
}

fn encode_from_str<T>(payload: &[u8], label: &str) -> Vec<u8>
where
    T: core::str::FromStr + norito::NoritoSerialize,
    <T as core::str::FromStr>::Err: core::fmt::Display,
{
    let raw = core::str::from_utf8(payload).expect("payload must be utf-8");
    let value: T = raw
        .parse()
        .unwrap_or_else(|e| panic!("{label} literal `{raw}` failed to parse: {e}"));
    norito::to_bytes(&value).expect("encode payload")
}

fn encode_account_id_payload(payload: &[u8]) -> Vec<u8> {
    // Some tests already provide Norito-encoded AccountId payload bytes.
    if norito::decode_from_bytes::<AccountId>(payload).is_ok() {
        return payload.to_vec();
    }

    let raw = core::str::from_utf8(payload).expect("payload must be utf-8");
    let account = AccountId::parse_encoded(raw)
        .map(iroha_data_model::account::ParsedAccountId::into_account_id)
        .unwrap_or_else(|err| panic!("AccountId literal `{raw}` failed to parse: {err}"));
    norito::to_bytes(&account).expect("encode payload")
}

fn encode_json_payload(payload: &[u8]) -> Vec<u8> {
    let raw = core::str::from_utf8(payload).expect("json payload must be utf-8");
    let json = Json::from_str_norito(raw).expect("parse json payload");
    norito::to_bytes(&json).expect("encode json payload")
}

fn encode_name_payload(payload: &[u8]) -> Vec<u8> {
    // Some tests already provide Norito-encoded Name payload bytes.
    if norito::decode_from_bytes::<Name>(payload).is_ok() {
        return payload.to_vec();
    }

    let raw = core::str::from_utf8(payload).expect("payload must be utf-8");
    match raw.parse::<Name>() {
        Ok(name) => norito::to_bytes(&name).expect("encode payload"),
        // Permission token literals like `mint_asset:rose#wonder` are intentionally
        // not `Name`; pass them through as raw bytes so host-side parsing decides.
        Err(_) => payload.to_vec(),
    }
}

fn encode_domain_id_payload(payload: &[u8]) -> Vec<u8> {
    if norito::decode_from_bytes::<DomainId>(payload).is_ok() {
        return payload.to_vec();
    }

    let raw = core::str::from_utf8(payload).expect("payload must be utf-8");
    // Older IVM pointer-TLV tests still pass bare domain labels. Canonicalize
    // those onto the universal dataspace so the helper matches the checked-in
    // TLV examples and existing fixture usage.
    let domain = if raw.contains('.') {
        DomainId::parse_fully_qualified(raw)
    } else {
        DomainId::try_new(raw, "universal")
    }
    .unwrap_or_else(|err| panic!("DomainId literal `{raw}` failed to parse: {err}"));
    norito::to_bytes(&domain).expect("encode payload")
}
