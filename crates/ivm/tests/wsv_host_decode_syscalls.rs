//! WsvHost JSON/Name/Schema decode syscalls coverage.
#![allow(unexpected_cfgs)]

use std::str::FromStr;

use iroha_data_model::prelude::Name;
use ivm::{
    IVM, PointerType,
    mock_wsv::{AccountId, MockWorldStateView, WsvHost},
    syscalls,
};
mod common;

fn make_tlv(pty: PointerType, payload: &[u8]) -> Vec<u8> {
    let mut v = Vec::with_capacity(7 + payload.len() + 32);
    v.extend_from_slice(&(pty as u16).to_be_bytes());
    v.push(1);
    v.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    v.extend_from_slice(payload);
    let h: [u8; 32] = iroha_crypto::Hash::new(payload).into();
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
    let p_name = vm
        .alloc_input_tlv(&make_tlv(PointerType::NoritoBytes, name.as_bytes()))
        .expect("alloc name");

    let prog = common::assemble_syscalls(&[syscalls::SYSCALL_NAME_DECODE as u8]);
    vm.set_register(10, p_name);
    vm.load_program(&prog).expect("load program");
    vm.run().expect("name decode");

    let out_ptr = vm.register(10);
    let tlv = vm.memory.validate_tlv(out_ptr).expect("output tlv");
    assert_eq!(tlv.type_id, PointerType::Name);
    let parsed =
        Name::from_str(std::str::from_utf8(tlv.payload).expect("name utf8")).expect("name parse");
    assert_eq!(parsed.as_ref(), name);
}

#[test]
fn wsv_host_json_decode_accepts_blob() {
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(wsv_host());

    let json = br#"{"a":1,"b":[2,3]}"#;
    let p_blob = vm
        .alloc_input_tlv(&make_tlv(PointerType::Blob, json))
        .expect("alloc blob");

    let prog = common::assemble_syscalls(&[syscalls::SYSCALL_JSON_DECODE as u8]);
    vm.set_register(10, p_blob);
    vm.load_program(&prog).expect("load program");
    vm.run().expect("json decode");

    let out_ptr = vm.register(10);
    let tlv = vm.memory.validate_tlv(out_ptr).expect("output tlv");
    assert_eq!(tlv.type_id, PointerType::Json);
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
    let value: norito::json::Value = norito::json::from_slice(tlv.payload).expect("json decode");
    let obj = value.as_object().expect("json object");
    assert_eq!(obj.get("qty").and_then(|v| v.as_i64()), Some(10));
    assert_eq!(obj.get("side").and_then(|v| v.as_str()), Some("buy"));
}
