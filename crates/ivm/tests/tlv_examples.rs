//! Golden-structure tests for pointer-ABI TLV envelopes.
//! These tests validate big-endian length encoding and basic layout.

use iroha_crypto::Hash;
use iroha_data_model::nexus::{DataSpaceId, LaneId};
use ivm::{
    Memory, PointerType,
    axt::{
        self, AssetHandle, AxtDescriptor, AxtTouchSpec, GroupBinding, HandleBudget, HandleSubject,
        ProofBlob,
    },
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
    // "6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn"
    let payload = b"6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn";
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
    let mut mem = Memory::new(0);
    mem.preload_input(0, &tlv).expect("preload input");
    let region = mem
        .load_region(Memory::INPUT_START, tlv.len() as u64)
        .unwrap();
    assert_eq!(region, &*tlv);
}

#[test]
fn tlv_assetdef_structure() {
    let payload = b"rose#wonderland";
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
fn tlv_asset_handle_roundtrip() {
    let descriptor = sample_descriptor();
    let binding = axt::compute_binding(&descriptor).expect("compute binding");

    let handle = AssetHandle {
        scope: vec!["transfer".into(), "withdraw".into()],
        subject: HandleSubject {
            account: "6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn".into(),
            origin_dsid: Some(DataSpaceId::new(5)),
        },
        budget: HandleBudget {
            remaining: 42,
            per_use: Some(7),
        },
        handle_era: 3,
        sub_nonce: 17,
        group_binding: GroupBinding {
            composability_group_id: vec![0xAA, 0xBB, 0xCC],
            epoch_id: 2026,
        },
        target_lane: LaneId::new(2),
        axt_binding: binding.to_vec(),
        manifest_view_root: vec![0x11; 32],
        expiry_slot: 9_999,
        max_clock_skew_ms: Some(500),
    };

    let payload = to_bytes(&handle).expect("encode handle");
    let type_id = PointerType::AssetHandle as u16;
    let tlv = make_tlv(type_id, 1, &payload);

    assert_eq!(u16::from_be_bytes(tlv[0..2].try_into().unwrap()), type_id);
    assert_eq!(
        u32::from_be_bytes(tlv[3..7].try_into().unwrap()),
        payload.len() as u32
    );
    let decoded: AssetHandle =
        decode_from_bytes(&tlv[7..7 + payload.len()]).expect("decode AssetHandle payload");
    assert_eq!(decoded, handle);

    let got_hash: [u8; 32] = tlv[7 + payload.len()..7 + payload.len() + 32]
        .try_into()
        .unwrap();
    let exp_hash: [u8; 32] = Hash::new(payload).into();
    assert_eq!(got_hash, exp_hash);
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
