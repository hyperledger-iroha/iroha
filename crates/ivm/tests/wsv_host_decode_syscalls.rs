//! WsvHost JSON/Name/Schema decode syscalls coverage.
#![allow(unexpected_cfgs)]

use std::str::FromStr;

use iroha_data_model::prelude::Name;
use iroha_primitives::json::Json;
use ivm::{
    IVM, PointerType,
    mock_wsv::{AccountId, MockWorldStateView, WsvHost},
    syscalls,
};
mod common;

fn make_tlv(pty: PointerType, payload: &[u8]) -> Vec<u8> {
    let payload = common::payload_for_type(pty, payload);
    let mut v = Vec::with_capacity(7 + payload.len() + 32);
    v.extend_from_slice(&(pty as u16).to_be_bytes());
    v.push(1);
    v.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    v.extend_from_slice(&payload);
    let h: [u8; 32] = iroha_crypto::Hash::new(&payload).into();
    v.extend_from_slice(&h);
    v
}

fn wsv_host() -> WsvHost {
    let wsv = MockWorldStateView::new();
    let caller: AccountId =
        "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03@wonderland"
            .parse()
            .expect("caller id");
    WsvHost::new(wsv, caller, Default::default(), Default::default())
}

#[test]
fn wsv_host_name_decode_roundtrip() {
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(wsv_host());

    let name = "wonderland";
    let name_payload =
        norito::to_bytes(&Name::from_str(name).expect("name parse")).expect("encode name");
    let p_name = vm
        .alloc_input_tlv(&make_tlv(PointerType::NoritoBytes, &name_payload))
        .expect("alloc name");

    let prog = common::assemble_syscalls(&[syscalls::SYSCALL_NAME_DECODE as u8]);
    vm.set_register(10, p_name);
    vm.load_program(&prog).expect("load program");
    vm.run().expect("name decode");

    let out_ptr = vm.register(10);
    let tlv = vm.memory.validate_tlv(out_ptr).expect("output tlv");
    assert_eq!(tlv.type_id, PointerType::Name);
    let parsed: Name = norito::decode_from_bytes(tlv.payload).expect("decode name");
    assert_eq!(parsed.as_ref(), name);
}

#[test]
fn wsv_host_json_decode_rejects_blob() {
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(wsv_host());

    let json = br#"{"a":1,"b":[2,3]}"#;
    let p_blob = vm
        .alloc_input_tlv(&make_tlv(PointerType::Blob, json))
        .expect("alloc blob");

    let prog = common::assemble_syscalls(&[syscalls::SYSCALL_JSON_DECODE as u8]);
    vm.set_register(10, p_blob);
    vm.load_program(&prog).expect("load program");
    let err = vm.run().unwrap_err();
    assert!(matches!(err, ivm::VMError::NoritoInvalid));
}

#[test]
fn wsv_host_schema_decode_roundtrip() {
    #[derive(norito::Decode, norito::Encode, Clone, Debug)]
    struct Order {
        qty: i64,
        side: String,
    }

    let mut vm = IVM::new(u64::MAX);
    vm.set_host(wsv_host());

    let schema = b"Order";
    let order = Order {
        qty: 10,
        side: "buy".to_string(),
    };
    let bytes = norito::to_bytes(&order).expect("encode order");

    let p_schema = vm
        .alloc_input_tlv(&make_tlv(PointerType::Name, schema))
        .expect("alloc schema");
    let p_bytes = vm
        .alloc_input_tlv(&make_tlv(PointerType::NoritoBytes, &bytes))
        .expect("alloc order bytes");

    let prog = common::assemble_syscalls(&[syscalls::SYSCALL_SCHEMA_DECODE as u8]);
    vm.set_register(10, p_schema);
    vm.set_register(11, p_bytes);
    vm.load_program(&prog).expect("load program");
    vm.run().expect("schema decode");

    let out_ptr = vm.register(10);
    let tlv = vm.memory.validate_tlv(out_ptr).expect("output tlv");
    assert_eq!(tlv.type_id, PointerType::Json);
    let value: norito::json::Value = common::json_from_payload(tlv.payload);
    let obj = value.as_object().expect("json object");
    assert_eq!(obj.get("qty").and_then(|v| v.as_i64()), Some(10));
    assert_eq!(obj.get("side").and_then(|v| v.as_str()), Some("buy"));
}

#[test]
fn wsv_host_schema_encode_decode_unknown_schema_roundtrip() {
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(wsv_host());

    let schema = b"UnknownSchema";
    let json = br#"{"hello":"world","n":1}"#;
    let expected =
        Json::from_str_norito(std::str::from_utf8(json).expect("json utf8")).expect("parse json");

    let p_schema = vm
        .alloc_input_tlv(&make_tlv(PointerType::Name, schema))
        .expect("alloc schema");
    let p_json = vm
        .alloc_input_tlv(&make_tlv(PointerType::Json, json))
        .expect("alloc json");

    let enc_prog = common::assemble_syscalls(&[syscalls::SYSCALL_SCHEMA_ENCODE as u8]);
    vm.set_register(10, p_schema);
    vm.set_register(11, p_json);
    vm.load_program(&enc_prog).expect("load program");
    vm.run().expect("schema encode");

    let p_blob = vm.register(10);
    let tlv_blob = vm.memory.validate_tlv(p_blob).expect("encoded blob tlv");
    assert_eq!(tlv_blob.type_id, PointerType::NoritoBytes);
    let encoded_json: Json =
        norito::decode_from_bytes(tlv_blob.payload).expect("decode norito json");
    assert_eq!(encoded_json, expected);

    let p_blob_in = vm
        .alloc_input_tlv(&make_tlv(PointerType::NoritoBytes, tlv_blob.payload))
        .expect("alloc blob");
    let dec_prog = common::assemble_syscalls(&[syscalls::SYSCALL_SCHEMA_DECODE as u8]);
    vm.set_register(10, p_schema);
    vm.set_register(11, p_blob_in);
    vm.load_program(&dec_prog).expect("load program");
    vm.run().expect("schema decode");

    let out_ptr = vm.register(10);
    let tlv_out = vm.memory.validate_tlv(out_ptr).expect("output tlv");
    assert_eq!(tlv_out.type_id, PointerType::Json);
    let decoded_json: Json =
        norito::decode_from_bytes(tlv_out.payload).expect("decode output json");
    assert_eq!(decoded_json, expected);
}
