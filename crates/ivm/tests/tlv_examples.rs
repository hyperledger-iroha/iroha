//! Golden-structure tests for pointer-ABI TLV envelopes.
//! These tests validate big-endian length encoding and basic layout.
use iroha_crypto::Hash;
use iroha_data_model::nexus::AxtAnchoredSpendV1;
use iroha_model_base::topology::DataSpaceId;
use ivm::{
    Memory, PointerType,
    axt::{AxtDescriptor, AxtTouchSpec, ProofBlob},
};
use norito::{decode_from_bytes, to_bytes};
fn make_tlv(type_id: u16, version: u8, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(2 + 1 + 4 + payload.len() + 32);
    out.extend_from_slice(&type_id.to_be_bytes());
    out.push(version);
    out.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    out.extend_from_slice(payload.as_ref());
    // Iroha Hash (blake2b-32 with LSB set)
    let h: [u8; 32] = Hash::new(payload).into();
    out.extend_from_slice(&h);
    out
}
fn sample_descriptor() -> AxtDescriptor {
    let dsid_a = DataSpaceId::new(7);
    let dsid_b = DataSpaceId::new(11);
    AxtDescriptor {
        dsids: vec![dsid_a, dsid_b],
        touches: vec![
            AxtTouchSpec {
                dsid: dsid_a,
                read: vec!["balances/alice".to_string()],
                write: vec!["balances/".to_string()],
            },
            AxtTouchSpec {
                dsid: dsid_b,
                read: vec!["orders/".to_string()],
                write: vec![],
            },
        ],
    }
}
#[test]
fn tlv_account_id_structure() {
    // "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV"
    let payload = "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV".as_bytes();
    let tlv = make_tlv(0x0001, 1, payload);
    // type, version, len, payload, hash
    assert_eq!(tlv.len(), 2 + 1 + 4 + payload.len() + 32);
    assert_eq!(&tlv[0..2], &[0x00, 0x01]);
    assert_eq!(tlv[2], 0x01);
    assert_eq!(
        u32::from_be_bytes(tlv[3..7].try_into().unwrap()),
        payload.len() as u32
    );
    assert_eq!(&tlv[7..7 + payload.len()], payload);
    // Verify hash field matches
    let got_hash: [u8; 32] = tlv[7 + payload.len()..7 + payload.len() + 32]
        .try_into()
        .unwrap();
    let exp_hash: [u8; 32] = Hash::new(payload).into();
    assert_eq!(got_hash, exp_hash);
    // Preload into INPUT and read back a slice
    let mut mem = Memory::new();
    mem.preload_input(0, &tlv).expect("preload input");
    let region = mem
        .load_region(Memory::INPUT_START, tlv.len() as u64)
        .unwrap();
    assert_eq!(region, &*tlv);
}
#[test]
fn tlv_assetdef_structure() {
    let payload = b"62Fk4FPcMuLvW5QjDGNF2a4jAmjM";
    let tlv = make_tlv(0x0002, 1, payload);
    assert_eq!(u16::from_be_bytes(tlv[0..2].try_into().unwrap()), 0x0002);
    assert_eq!(
        u32::from_be_bytes(tlv[3..7].try_into().unwrap()),
        payload.len() as u32
    );
    let got_hash: [u8; 32] = tlv[7 + payload.len()..7 + payload.len() + 32]
        .try_into()
        .unwrap();
    let exp_hash: [u8; 32] = Hash::new(payload).into();
    assert_eq!(got_hash, exp_hash);
}
#[test]
fn tlv_name_structure() {
    let payload = b"cursor";
    let tlv = make_tlv(0x0003, 1, payload);
    assert_eq!(u16::from_be_bytes(tlv[0..2].try_into().unwrap()), 0x0003);
    assert_eq!(
        u32::from_be_bytes(tlv[3..7].try_into().unwrap()),
        payload.len() as u32
    );
    let got_hash: [u8; 32] = tlv[7 + payload.len()..7 + payload.len() + 32]
        .try_into()
        .unwrap();
    let exp_hash: [u8; 32] = Hash::new(payload).into();
    assert_eq!(got_hash, exp_hash);
}
#[test]
fn tlv_json_structure() {
    let payload = br#"{"query":"sc_dummy","cursor":1}"#;
    let tlv = make_tlv(0x0004, 1, payload);
    assert_eq!(u16::from_be_bytes(tlv[0..2].try_into().unwrap()), 0x0004);
    assert_eq!(
        u32::from_be_bytes(tlv[3..7].try_into().unwrap()),
        payload.len() as u32
    );
    let got_hash: [u8; 32] = tlv[7 + payload.len()..7 + payload.len() + 32]
        .try_into()
        .unwrap();
    let exp_hash: [u8; 32] = Hash::new(payload).into();
    assert_eq!(got_hash, exp_hash);
}
#[test]
fn tlv_nftid_structure() {
    let payload = b"rose:uuid:0123$wonderland";
    let tlv = make_tlv(0x0005, 1, payload);
    assert_eq!(u16::from_be_bytes(tlv[0..2].try_into().unwrap()), 0x0005);
    assert_eq!(
        u32::from_be_bytes(tlv[3..7].try_into().unwrap()),
        payload.len() as u32
    );
    let got_hash: [u8; 32] = tlv[7 + payload.len()..7 + payload.len() + 32]
        .try_into()
        .unwrap();
    let exp_hash: [u8; 32] = Hash::new(payload).into();
    assert_eq!(got_hash, exp_hash);
}
#[test]
fn tlv_dataspace_id_roundtrip() {
    let dsid = DataSpaceId::new(0xDEAD_BEEF_CAFE_BABE);
    let payload = to_bytes(&dsid).expect("encode DataSpaceId");
    let type_id = PointerType::DataSpaceId as u16;
    let tlv = make_tlv(type_id, 1, &payload);
    assert_eq!(u16::from_be_bytes(tlv[0..2].try_into().unwrap()), type_id);
    assert_eq!(
        u32::from_be_bytes(tlv[3..7].try_into().unwrap()),
        payload.len() as u32
    );
    let decoded: DataSpaceId =
        decode_from_bytes(&tlv[7..7 + payload.len()]).expect("decode DataSpaceId payload");
    assert_eq!(decoded, dsid);
    let got_hash: [u8; 32] = tlv[7 + payload.len()..7 + payload.len() + 32]
        .try_into()
        .unwrap();
    let exp_hash: [u8; 32] = Hash::new(payload).into();
    assert_eq!(got_hash, exp_hash);
}
#[test]
fn tlv_axt_descriptor_roundtrip() {
    let descriptor = sample_descriptor();
    let payload = to_bytes(&descriptor).expect("encode descriptor");
    let type_id = PointerType::AxtDescriptor as u16;
    let tlv = make_tlv(type_id, 1, &payload);
    assert_eq!(u16::from_be_bytes(tlv[0..2].try_into().unwrap()), type_id);
    assert_eq!(
        u32::from_be_bytes(tlv[3..7].try_into().unwrap()),
        payload.len() as u32
    );
    let decoded: AxtDescriptor =
        decode_from_bytes(&tlv[7..7 + payload.len()]).expect("decode AxtDescriptor payload");
    assert_eq!(decoded, descriptor);
    let got_hash: [u8; 32] = tlv[7 + payload.len()..7 + payload.len() + 32]
        .try_into()
        .unwrap();
    let exp_hash: [u8; 32] = Hash::new(payload).into();
    assert_eq!(got_hash, exp_hash);
}
#[test]
fn tlv_signed_anchored_spend_roundtrip_and_retired_handle_id_rejects() {
    let fixture: norito::json::Value = norito::json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../iroha_data_model/tests/fixtures/axt_envelope_multi_ds.json"
    )))
    .expect("current signed-spend fixture");
    let spend: AxtAnchoredSpendV1 = norito::json::from_value(fixture["spends"]["happy"][0].clone())
        .expect("signed-spend fixture");
    let payload = ivm::codec::encode_canonical_norito(&spend).expect("canonical signed spend");
    let tlv = make_tlv(PointerType::AxtAnchoredSpendV1 as u16, 1, &payload);
    let validated = ivm::pointer_abi::validate_tlv_bytes(&tlv).expect("typed TLV");
    assert_eq!(validated.type_id, PointerType::AxtAnchoredSpendV1);
    let decoded: AxtAnchoredSpendV1 =
        decode_from_bytes(validated.payload).expect("decode signed-spend payload");
    assert_eq!(decoded, spend);

    assert_eq!(PointerType::from_u16(0x000C), None);
    let retired = make_tlv(0x000C, 1, &payload);
    assert!(ivm::pointer_abi::validate_tlv_bytes(&retired).is_err());
}
#[test]
fn tlv_proof_blob_roundtrip() {
    let proof = ProofBlob {
        payload: vec![0xDE, 0xAD, 0xBE, 0xEF],
        expiry_slot: None,
    };
    let payload = to_bytes(&proof).expect("encode proof");
    let type_id = PointerType::ProofBlob as u16;
    let tlv = make_tlv(type_id, 1, &payload);
    assert_eq!(u16::from_be_bytes(tlv[0..2].try_into().unwrap()), type_id);
    assert_eq!(
        u32::from_be_bytes(tlv[3..7].try_into().unwrap()),
        payload.len() as u32
    );
    let decoded: ProofBlob =
        decode_from_bytes(&tlv[7..7 + payload.len()]).expect("decode ProofBlob payload");
    assert_eq!(decoded, proof);
    let got_hash: [u8; 32] = tlv[7 + payload.len()..7 + payload.len() + 32]
        .try_into()
        .unwrap();
    let exp_hash: [u8; 32] = Hash::new(payload).into();
    assert_eq!(got_hash, exp_hash);
}
