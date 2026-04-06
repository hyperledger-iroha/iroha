//! CoreHost JSON encode/decode and schema encode/decode helpers.

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use iroha_data_model::{
    nexus::DataSpaceId,
    prelude::{AccountId, AssetDefinitionId},
    smart_contract::ContractAddress,
};
use iroha_primitives::numeric::Numeric;
use ivm::{CoreHost, IVM, PointerType, encoding, instruction::wide, syscalls};
mod common;

fn tlv(pty: PointerType, payload: &[u8]) -> Vec<u8> {
    let payload = common::payload_for_type(pty, payload);
    let mut v = Vec::with_capacity(7 + payload.len() + 32);
    v.extend_from_slice(&(pty as u16).to_be_bytes());
    v.push(1);
    v.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    v.extend_from_slice(payload.as_ref());
    let h: [u8; 32] = iroha_crypto::Hash::new(payload).into();
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
fn json_get_asset_definition_id_reads_address_literals() {
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(CoreHost::new());

    let json = br#"{"asset_definition_id":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM"}"#;
    let p_json = vm.alloc_input_tlv(&tlv(PointerType::Json, json)).unwrap();
    let p_key = vm
        .alloc_input_tlv(&tlv(PointerType::Name, b"asset_definition_id"))
        .unwrap();

    let prog = common::assemble_syscalls(&[syscalls::SYSCALL_JSON_GET_ASSET_DEFINITION_ID as u8]);
    vm.set_register(10, p_json);
    vm.set_register(11, p_key);
    vm.load_program(&prog).unwrap();
    vm.run().unwrap();

    let out_ptr = vm.register(10);
    let tlv_out = vm.memory.validate_tlv(out_ptr).unwrap();
    assert_eq!(tlv_out.type_id, PointerType::AssetDefinitionId);
    let asset: AssetDefinitionId = norito::decode_from_bytes(tlv_out.payload).unwrap();
    assert_eq!(
        asset,
        AssetDefinitionId::parse_address_literal("62Fk4FPcMuLvW5QjDGNF2a4jAmjM").unwrap()
    );
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

#[test]
fn json_get_numeric_reads_decimal_strings() {
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(CoreHost::new());
    let json = br#"{"amount":"0.00001"}"#;
    let p_json = vm.alloc_input_tlv(&tlv(PointerType::Json, json)).unwrap();
    let p_key = vm
        .alloc_input_tlv(&tlv(PointerType::Name, b"amount"))
        .unwrap();

    let prog = common::assemble(
        &[
            encoding::wide::encode_sys(
                wide::system::SCALL,
                syscalls::SYSCALL_JSON_GET_NUMERIC as u8,
            )
            .to_le_bytes(),
            encoding::wide::encode_halt().to_le_bytes(),
        ]
        .concat(),
    );
    vm.set_register(10, p_json);
    vm.set_register(11, p_key);
    vm.load_program(&prog).unwrap();
    vm.run().unwrap();

    let tlv = vm.memory.validate_tlv(vm.register(10)).unwrap();
    assert_eq!(tlv.type_id, PointerType::NoritoBytes);
    let value: Numeric = norito::decode_from_bytes(tlv.payload).expect("decode numeric");
    assert_eq!(value, "0.00001".parse::<Numeric>().expect("parse numeric"));
}

#[test]
fn json_object_builders_roundtrip_i64_and_account_id() {
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(CoreHost::new());

    let p_bucket_key = vm
        .alloc_input_tlv(&tlv(PointerType::Name, b"bucket_id"))
        .unwrap();
    let p_owner_key = vm
        .alloc_input_tlv(&tlv(PointerType::Name, b"owner"))
        .unwrap();
    let owner_literal = "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB";
    let p_owner = vm
        .alloc_input_tlv(&tlv(PointerType::AccountId, owner_literal.as_bytes()))
        .unwrap();

    vm.load_program(&common::assemble_syscalls(&[
        syscalls::SYSCALL_JSON_OBJECT as u8
    ]))
    .unwrap();
    vm.run().unwrap();
    let p_payload = vm.register(10);

    vm.set_register(10, p_payload);
    vm.set_register(11, p_bucket_key);
    vm.set_register(12, 2);
    vm.load_program(&common::assemble_syscalls(&[
        syscalls::SYSCALL_JSON_SET_I64 as u8,
    ]))
    .unwrap();
    vm.run().unwrap();
    let p_payload = vm.register(10);

    vm.set_register(10, p_payload);
    vm.set_register(11, p_owner_key);
    vm.set_register(12, p_owner);
    vm.load_program(&common::assemble_syscalls(&[
        syscalls::SYSCALL_JSON_SET_ACCOUNT_ID as u8,
    ]))
    .unwrap();
    vm.run().unwrap();
    let p_payload = vm.register(10);

    vm.set_register(10, p_payload);
    vm.set_register(11, p_bucket_key);
    vm.load_program(&common::assemble_syscalls(&[
        syscalls::SYSCALL_JSON_GET_I64 as u8,
    ]))
    .unwrap();
    vm.run().unwrap();
    assert_eq!(vm.register(10) as i64, 2);

    vm.set_register(10, p_payload);
    vm.set_register(11, p_owner_key);
    vm.load_program(&common::assemble_syscalls(&[
        syscalls::SYSCALL_JSON_GET_ACCOUNT_ID as u8,
    ]))
    .unwrap();
    vm.run().unwrap();

    let tlv_out = vm.memory.validate_tlv(vm.register(10)).unwrap();
    assert_eq!(tlv_out.type_id, PointerType::AccountId);
    let owner: AccountId = norito::decode_from_bytes(tlv_out.payload).expect("decode account");
    assert_eq!(
        owner,
        AccountId::parse_encoded(owner_literal)
            .expect("valid canonical account id")
            .into_account_id()
    );
}

#[test]
fn json_get_account_id_reads_contract_address_subject_literal() {
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(CoreHost::new());

    let authority = AccountId::new(iroha_crypto::KeyPair::random().public_key().clone());
    let contract_address = ContractAddress::derive(
        iroha_data_model::account::address::chain_discriminant(),
        &authority,
        3,
        DataSpaceId::GLOBAL,
    )
    .expect("derive contract address");
    let json = format!(r#"{{"controller":"{}"}}"#, contract_address);
    let p_json = vm
        .alloc_input_tlv(&tlv(PointerType::Json, json.as_bytes()))
        .unwrap();
    let p_key = vm
        .alloc_input_tlv(&tlv(PointerType::Name, b"controller"))
        .unwrap();

    let prog = common::assemble_syscalls(&[syscalls::SYSCALL_JSON_GET_ACCOUNT_ID as u8]);
    vm.set_register(10, p_json);
    vm.set_register(11, p_key);
    vm.load_program(&prog).unwrap();
    vm.run().unwrap();

    let tlv_out = vm.memory.validate_tlv(vm.register(10)).unwrap();
    assert_eq!(tlv_out.type_id, PointerType::AccountId);
    let decoded: AccountId =
        norito::decode_from_bytes(tlv_out.payload).expect("decode contract subject");
    assert_eq!(decoded, contract_address.subject_id());
}
