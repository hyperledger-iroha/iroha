#![allow(dead_code)]
use std::vec::Vec;

// --- CompactProofBundle helpers via syscalls (test-only utilities) ---
use iroha_data_model::prelude::*;
use iroha_primitives::json::Json;
use ivm::{IVM, PointerType, ProgramMetadata, encoding, instruction, syscalls};

const HALT_WORD: u32 = encoding::wide::encode_halt();
pub const HALT: [u8; 4] = HALT_WORD.to_le_bytes();

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
        PointerType::DomainId => encode_from_str::<DomainId>(payload, "DomainId"),
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
