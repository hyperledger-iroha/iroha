//! `CoreHost` Norito serialization helper syscall coverage.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]

use std::convert::TryFrom;

use iroha_core::smartcontracts::ivm::host::CoreHost;
use iroha_crypto::Hash;
use iroha_data_model::prelude::*;
use iroha_primitives::json::Json;
use iroha_test_samples::ALICE_ID;
use ivm::{IVM, IVMHost, PointerType, ProgramMetadata, syscalls};

fn make_tlv(pty: PointerType, payload: &[u8]) -> Vec<u8> {
    let mut v = Vec::with_capacity(7 + payload.len() + 32);
    v.extend_from_slice(&(pty as u16).to_be_bytes());
    v.push(1);
    let payload_len = u32::try_from(payload.len()).expect("payload length fits in u32");
    v.extend_from_slice(&payload_len.to_be_bytes());
    v.extend_from_slice(payload);
    let h: [u8; 32] = Hash::new(payload).into();
    v.extend_from_slice(&h);
    v
}

fn preload_input(vm: &mut IVM, _offset: u64, blob: &[u8]) -> u64 {
    vm.alloc_input_tlv(blob).expect("allocate input TLV")
}

fn load_metadata(vm: &mut IVM) {
    vm.load_program(&ProgramMetadata::default().encode())
        .expect("load metadata");
    assert_eq!(vm.abi_version(), 1, "default metadata should set ABI v1");
    assert!(
        ivm::syscalls::is_syscall_allowed(vm.syscall_policy(), syscalls::SYSCALL_JSON_ENCODE),
        "ABI policy must allow JSON helpers in tests"
    );
}

#[test]
fn json_encode_decode_roundtrip() {
    let mut host = CoreHost::new(ALICE_ID.clone());
    let mut vm = IVM::new(0);
    load_metadata(&mut vm);

    let json_value = norito::json::to_string(&norito::json!({
        "a": 1u64,
        "b": [2u64, 3u64]
    }))
    .expect("serialize json value");
    let json = Json::from_str_norito(&json_value).expect("parse json");
    let json_body = norito::to_bytes(&json).expect("encode json payload");
    let json_ptr = preload_input(&mut vm, 0, &make_tlv(PointerType::Json, &json_body));
    vm.set_register(10, json_ptr);

    host.syscall(syscalls::SYSCALL_JSON_ENCODE, &mut vm)
        .expect("json encode syscall succeeds");
    let encoded_ptr = vm.register(10);
    let encoded_tlv = vm
        .memory
        .validate_tlv(encoded_ptr)
        .expect("validate NoritoBytes TLV");
    assert_eq!(encoded_tlv.type_id, PointerType::NoritoBytes);

    host.syscall(syscalls::SYSCALL_JSON_DECODE, &mut vm)
        .expect("json decode syscall succeeds");
    let decoded_ptr = vm.register(10);
    let decoded_tlv = vm
        .memory
        .validate_tlv(decoded_ptr)
        .expect("validate Json TLV");
    assert_eq!(decoded_tlv.type_id, PointerType::Json);

    let decoded_json: Json =
        norito::decode_from_bytes(decoded_tlv.payload).expect("decode json payload");
    let roundtrip: norito::json::Value =
        norito::json::from_str(decoded_json.get()).expect("parse decoded json");
    let original: norito::json::Value =
        norito::json::from_str(json.get()).expect("parse original json");
    assert_eq!(roundtrip, original);
}

#[test]
fn schema_encode_decode_roundtrip() {
    let mut host = CoreHost::new(ALICE_ID.clone());
    let mut vm = IVM::new(0);
    load_metadata(&mut vm);

    let schema_name: Name = "Order".parse().unwrap();
    let schema_body = norito::to_bytes(&schema_name).expect("encode schema name");
    let schema_ptr = preload_input(&mut vm, 0, &make_tlv(PointerType::Name, &schema_body));

    let json_value = norito::json::to_string(&norito::json!({
        "qty": 10i64,
        "side": "buy"
    }))
    .expect("serialize schema json");
    let json = Json::from_str_norito(&json_value).expect("parse schema json");
    let json_body = norito::to_bytes(&json).expect("encode schema json");
    let json_ptr = preload_input(&mut vm, 256, &make_tlv(PointerType::Json, &json_body));

    vm.set_register(10, schema_ptr);
    vm.set_register(11, json_ptr);
    host.syscall(syscalls::SYSCALL_SCHEMA_ENCODE, &mut vm)
        .expect("schema encode succeeds");
    let encoded_ptr = vm.register(10);
    let encoded_tlv = vm
        .memory
        .validate_tlv(encoded_ptr)
        .expect("validate NoritoBytes output");
    assert_eq!(encoded_tlv.type_id, PointerType::NoritoBytes);

    vm.set_register(10, schema_ptr);
    vm.set_register(11, encoded_ptr);
    host.syscall(syscalls::SYSCALL_SCHEMA_DECODE, &mut vm)
        .expect("schema decode succeeds");
    let decoded_ptr = vm.register(10);
    let decoded_tlv = vm
        .memory
        .validate_tlv(decoded_ptr)
        .expect("validate decoded Json TLV");
    assert_eq!(decoded_tlv.type_id, PointerType::Json);

    let decoded_json: Json =
        norito::decode_from_bytes(decoded_tlv.payload).expect("decode schema json");
    let roundtrip: norito::json::Value =
        norito::json::from_str(decoded_json.get()).expect("parse decoded schema json");
    let original: norito::json::Value =
        norito::json::from_str(json.get()).expect("parse original schema json");
    assert_eq!(roundtrip, original);
}

#[test]
fn name_decode_from_norito_bytes() {
    let mut host = CoreHost::new(ALICE_ID.clone());
    let mut vm = IVM::new(0);
    load_metadata(&mut vm);

    let name: Name = "cursor".parse().expect("name");
    let raw = norito::to_bytes(&name).expect("encode name");
    let key_ptr = preload_input(&mut vm, 0, &make_tlv(PointerType::NoritoBytes, &raw));
    vm.set_register(10, key_ptr);

    host.syscall(syscalls::SYSCALL_NAME_DECODE, &mut vm)
        .expect("name decode succeeds");
    let name_ptr = vm.register(10);
    let tlv = vm.memory.validate_tlv(name_ptr).expect("validate name TLV");
    assert_eq!(tlv.type_id, PointerType::Name);
    let decoded: Name = norito::decode_from_bytes(tlv.payload).expect("decode name");
    assert_eq!(decoded.as_ref(), "cursor");
}

#[test]
fn build_path_key_norito_appends_hash() {
    let mut host = CoreHost::new(ALICE_ID.clone());
    let mut vm = IVM::new(0);
    load_metadata(&mut vm);

    let base: Name = "kv".parse().unwrap();
    let base_bytes = norito::to_bytes(&base).expect("encode base name");
    let base_ptr = preload_input(&mut vm, 0, &make_tlv(PointerType::Name, &base_bytes));
    let key = b"opaque norito payload";
    let key_ptr = preload_input(&mut vm, 256, &make_tlv(PointerType::NoritoBytes, key));

    vm.set_register(10, base_ptr);
    vm.set_register(11, key_ptr);
    host.syscall(syscalls::SYSCALL_BUILD_PATH_KEY_NORITO, &mut vm)
        .expect("path key builder succeeds");

    let out_ptr = vm.register(10);
    let out_tlv = vm
        .memory
        .validate_tlv(out_ptr)
        .expect("validate output Name TLV");
    assert_eq!(out_tlv.type_id, PointerType::Name);
    let out_name: Name = norito::decode_from_bytes(out_tlv.payload).expect("decode output Name");

    let expected_hash = Hash::new(key);
    let hex = hex::encode(expected_hash.as_ref());
    assert_eq!(out_name.as_ref(), format!("kv/{hex}"));
}

#[test]
fn schema_info_returns_registry_snapshot() {
    let mut host = CoreHost::new(ALICE_ID.clone());
    let mut vm = IVM::new(0);
    load_metadata(&mut vm);

    let schema: Name = "Order".parse().unwrap();
    let schema_bytes = norito::to_bytes(&schema).expect("encode schema name");
    let schema_ptr = preload_input(&mut vm, 0, &make_tlv(PointerType::Name, &schema_bytes));
    vm.set_register(10, schema_ptr);

    host.syscall(syscalls::SYSCALL_SCHEMA_INFO, &mut vm)
        .expect("schema info succeeds");
    let info_ptr = vm.register(10);
    let info_tlv = vm
        .memory
        .validate_tlv(info_ptr)
        .expect("validate schema info json");
    assert_eq!(info_tlv.type_id, PointerType::Json);

    let info_json: Json =
        norito::decode_from_bytes(info_tlv.payload).expect("decode schema info json");
    let info: norito::json::Value =
        norito::json::from_str(info_json.get()).expect("parse schema info json");
    let current_name = info["current"]["name"].as_str().unwrap();
    assert_eq!(current_name, "OrderByTime");
    let versions = info["versions"].as_array().unwrap();
    assert!(versions.len() >= 2);
}
