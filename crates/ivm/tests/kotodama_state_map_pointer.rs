//! Verify durable map path hashing and pointer Norito encoding helpers align with CoreHost.

use std::fmt::Write;

use iroha_crypto::Hash as IrohaHash;
use iroha_data_model::prelude::*;
use ivm::{CoreHost, PointerType};
use norito::to_bytes;

fn encode_pointer_tlv(ty: PointerType, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(2 + 1 + 4 + payload.len() + IrohaHash::LENGTH);
    out.extend_from_slice(&(ty as u16).to_be_bytes());
    out.push(1);
    out.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    out.extend_from_slice(payload.as_ref());
    let hash: [u8; 32] = IrohaHash::new(payload).into();
    out.extend_from_slice(&hash);
    out
}

fn parse_account_id_literal(id: &str) -> AccountId {
    AccountId::parse_encoded(id)
        .map(iroha_data_model::account::ParsedAccountId::into_account_id)
        .expect("account literal must be canonical I105")
}

fn account_pointer_tlvs(id: &str) -> (Vec<u8>, Vec<u8>) {
    let account = parse_account_id_literal(id);
    let payload = to_bytes(&account).expect("encode account id");
    let raw = encode_pointer_tlv(PointerType::AccountId, &payload);
    let norito = encode_pointer_tlv(PointerType::NoritoBytes, &raw);
    (raw, norito)
}

fn map_path(base: &str, pointer_payload: &[u8]) -> String {
    let hash: [u8; 32] = IrohaHash::new(pointer_payload).into();
    let mut s = String::with_capacity(base.len() + 1 + 64);
    let _ = write!(&mut s, "{base}/");
    for b in &hash {
        let _ = write!(&mut s, "{b:02x}");
    }
    s
}

fn encode_int_norito(value: i64) -> Vec<u8> {
    let payload = to_bytes(&value).expect("encode i64");
    encode_pointer_tlv(PointerType::NoritoBytes, &payload)
}

#[test]
fn durable_map_account_id_path_hashes() {
    const OWNER_ID: &str = "6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn";
    let (raw_ptr, _norito_ptr) = account_pointer_tlvs(OWNER_ID);
    let path = map_path("balances", &raw_ptr);
    assert!(path.starts_with("balances/"));
    assert_eq!(path.len(), "balances/".len() + 64);

    let mut host = CoreHost::new();
    host.insert_state_value(&path, encode_int_norito(5));
    let stored = host
        .state_bytes(&path)
        .expect("stored value should be present");
    let payload = to_bytes(&5_i64).expect("encode i64");
    assert_eq!(stored.len(), 7 + payload.len() + IrohaHash::LENGTH);
}

#[test]
fn durable_map_name_value_roundtrip() {
    let mut host = CoreHost::new();
    let name: Name = "wonder".parse().expect("valid name");
    let payload = to_bytes(&name).expect("encode name");
    host.insert_state_value(
        "aliases/42",
        encode_pointer_tlv(PointerType::Name, &payload),
    );
    let stored = host.state_bytes("aliases/42").expect("name pointer stored");
    assert_eq!(stored.len(), 7 + payload.len() + IrohaHash::LENGTH);
}

#[test]
fn durable_map_account_id_value_roundtrip() {
    const OWNER_ID: &str = "6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn";
    let (_raw_ptr, norito_ptr) = account_pointer_tlvs(OWNER_ID);
    let mut host = CoreHost::new();
    host.insert_state_value("owners/7", norito_ptr.clone());
    let stored = host
        .state_bytes("owners/7")
        .expect("account pointer stored");
    assert_eq!(stored, &norito_ptr[..]);
}
