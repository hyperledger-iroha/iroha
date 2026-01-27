//! CoreHost JSON encode/decode and schema encode/decode helpers.

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use ivm::{CoreHost, IVM, PointerType, encoding, instruction::wide, syscalls};
mod common;

fn tlv(pty: PointerType, payload: &[u8]) -> Vec<u8> {
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

#[test]
fn json_encode_decode_roundtrip() {
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(CoreHost::new());
    let json = br#"{"a":1, "b": [2,3]}"#;
    let p_json = vm.alloc_input_tlv(&tlv(PointerType::Json, json)).unwrap();
    // ENCODE
    let enc_prog = common::assemble(
        &[
            encoding::wide::encode_sys(
                wide::system::SCALL,
                syscalls::SYSCALL_INPUT_PUBLISH_TLV as u8,
            )
            .to_le_bytes(),
            encoding::wide::encode_sys(wide::system::SCALL, syscalls::SYSCALL_JSON_ENCODE as u8)
                .to_le_bytes(),
            encoding::wide::encode_halt().to_le_bytes(),
        ]
        .concat(),
    );
    vm.set_register(10, p_json);
    vm.load_program(&enc_prog).unwrap();
    vm.run().unwrap();
    let p_blob = vm.register(10);
    let tlv_b = vm.memory.validate_tlv(p_blob).unwrap();
    assert_eq!(tlv_b.type_id, PointerType::NoritoBytes);
    // DECODE
    let dec_prog = common::assemble(
        &[
            encoding::wide::encode_sys(
                wide::system::SCALL,
                syscalls::SYSCALL_INPUT_PUBLISH_TLV as u8,
            )
            .to_le_bytes(),
            encoding::wide::encode_sys(wide::system::SCALL, syscalls::SYSCALL_JSON_DECODE as u8)
                .to_le_bytes(),
            encoding::wide::encode_halt().to_le_bytes(),
        ]
        .concat(),
    );
    vm.set_register(10, p_blob);
    vm.load_program(&dec_prog).unwrap();
    vm.run().unwrap();
    let p_out = vm.register(10);
    let tlv_j = vm.memory.validate_tlv(p_out).unwrap();
    assert_eq!(tlv_j.type_id, PointerType::Json);
}

#[test]
fn json_decode_rejects_blob() {
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(CoreHost::new());
    let json = br#"{"a":1,"b":[2,3]}"#;
    let p_blob = vm.alloc_input_tlv(&tlv(PointerType::Blob, json)).unwrap();
    let dec_prog = common::assemble(
        &[
            encoding::wide::encode_sys(
                wide::system::SCALL,
                syscalls::SYSCALL_INPUT_PUBLISH_TLV as u8,
            )
            .to_le_bytes(),
            encoding::wide::encode_sys(wide::system::SCALL, syscalls::SYSCALL_JSON_DECODE as u8)
                .to_le_bytes(),
            encoding::wide::encode_halt().to_le_bytes(),
        ]
        .concat(),
    );
    vm.set_register(10, p_blob);
    vm.load_program(&dec_prog).unwrap();
    let err = vm.run().unwrap_err();
    assert!(matches!(err, ivm::VMError::NoritoInvalid));
}

#[test]
fn schema_encode_decode_roundtrip() {
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(CoreHost::new());
    let schema = b"Order";
    let json = br#"{"qty":10, "side":"buy"}"#;
    let p_schema = vm.alloc_input_tlv(&tlv(PointerType::Name, schema)).unwrap();
    let p_json = vm.alloc_input_tlv(&tlv(PointerType::Json, json)).unwrap();
    // ENCODE (inputs are already in INPUT via alloc_input_tlv)
    let enc = common::assemble(
        &[
            encoding::wide::encode_sys(wide::system::SCALL, syscalls::SYSCALL_SCHEMA_ENCODE as u8)
                .to_le_bytes(),
            encoding::wide::encode_halt().to_le_bytes(),
        ]
        .concat(),
    );
    vm.set_register(10, p_schema);
    vm.set_register(11, p_json);
    vm.load_program(&enc).unwrap();
    vm.run().unwrap();
    let p_blob = vm.register(10);
    let tlv_b = vm.memory.validate_tlv(p_blob).unwrap();
    assert_eq!(tlv_b.type_id, PointerType::NoritoBytes);
    // DECODE (inputs are already in INPUT via alloc_input_tlv)
    let dec = common::assemble(
        &[
            encoding::wide::encode_sys(wide::system::SCALL, syscalls::SYSCALL_SCHEMA_DECODE as u8)
                .to_le_bytes(),
            encoding::wide::encode_halt().to_le_bytes(),
        ]
        .concat(),
    );
    vm.set_register(10, p_schema);
    vm.set_register(11, p_blob);
    vm.load_program(&dec).unwrap();
    vm.run().unwrap();
    let p_out = vm.register(10);
    let tlv_j = vm.memory.validate_tlv(p_out).unwrap();
    assert_eq!(tlv_j.type_id, PointerType::Json);
}

#[test]
fn schema_decode_rejects_blob() {
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(CoreHost::new());
    let schema = b"Order";
    let json = br#"{"qty":10, "side":"buy"}"#;
    let p_schema = vm.alloc_input_tlv(&tlv(PointerType::Name, schema)).unwrap();
    let p_json = vm.alloc_input_tlv(&tlv(PointerType::Json, json)).unwrap();
    let enc = common::assemble(
        &[
            encoding::wide::encode_sys(wide::system::SCALL, syscalls::SYSCALL_SCHEMA_ENCODE as u8)
                .to_le_bytes(),
            encoding::wide::encode_halt().to_le_bytes(),
        ]
        .concat(),
    );
    vm.set_register(10, p_schema);
    vm.set_register(11, p_json);
    vm.load_program(&enc).unwrap();
    vm.run().unwrap();
    let p_blob = vm.register(10);
    let encoded = vm.memory.validate_tlv(p_blob).unwrap();
    let p_blob_alt = vm
        .alloc_input_tlv(&tlv(PointerType::Blob, encoded.payload))
        .unwrap();

    let dec = common::assemble(
        &[
            encoding::wide::encode_sys(wide::system::SCALL, syscalls::SYSCALL_SCHEMA_DECODE as u8)
                .to_le_bytes(),
            encoding::wide::encode_halt().to_le_bytes(),
        ]
        .concat(),
    );
    vm.set_register(10, p_schema);
    vm.set_register(11, p_blob_alt);
    vm.load_program(&dec).unwrap();
    let err = vm.run().unwrap_err();
    assert!(matches!(err, ivm::VMError::NoritoInvalid));
}

#[test]
fn schema_decode_unknown_schema_exposes_metadata() {
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(CoreHost::new());
    let schema = b"UnknownSchema";
    let payload = [0xAB, 0xCD, 0xEF];
    let p_schema = vm.alloc_input_tlv(&tlv(PointerType::Name, schema)).unwrap();
    let p_bytes = vm
        .alloc_input_tlv(&tlv(PointerType::NoritoBytes, &payload))
        .unwrap();

    let dec = common::assemble(
        &[
            encoding::wide::encode_sys(wide::system::SCALL, syscalls::SYSCALL_SCHEMA_DECODE as u8)
                .to_le_bytes(),
            encoding::wide::encode_halt().to_le_bytes(),
        ]
        .concat(),
    );
    vm.set_register(10, p_schema);
    vm.set_register(11, p_bytes);
    vm.load_program(&dec).unwrap();
    vm.run().unwrap();

    let p_out = vm.register(10);
    let tlv_j = vm.memory.validate_tlv(p_out).unwrap();
    assert_eq!(tlv_j.type_id, PointerType::Json);

    let value: norito::json::Value = common::json_from_payload(tlv_j.payload);
    let obj = value.as_object().expect("fallback json object");

    let schema_obj = obj
        .get("schema")
        .and_then(norito::json::Value::as_object)
        .expect("schema metadata");
    let schema_name = schema_obj
        .get("name")
        .and_then(norito::json::Value::as_str)
        .expect("schema name");
    assert_eq!(schema_name, "UnknownSchema");
    assert!(matches!(
        schema_obj.get("id"),
        Some(norito::json::Value::Null)
    ));
    assert!(matches!(
        schema_obj.get("version"),
        Some(norito::json::Value::Null)
    ));

    let payload_b64 = obj
        .get("payload_base64")
        .and_then(norito::json::Value::as_str)
        .expect("payload base64");
    assert_eq!(payload_b64, BASE64_STANDARD.encode(payload));

    let len = obj
        .get("payload_len")
        .and_then(norito::json::Value::as_u64)
        .expect("payload length");
    assert_eq!(len as usize, payload.len());

    let versions = obj
        .get("known_versions")
        .and_then(norito::json::Value::as_array)
        .expect("known versions array");
    assert!(versions.is_empty());
}
