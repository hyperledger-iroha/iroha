use iroha_crypto::Hash;
use ivm::{Memory, PointerType};

fn make_tlv(type_id: u16, version: u8, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(7 + payload.len() + 32);
    out.extend_from_slice(&type_id.to_be_bytes());
    out.push(version);
    out.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    out.extend_from_slice(payload);
    let h: [u8; 32] = Hash::new(payload).into();
    out.extend_from_slice(&h);
    out
}

#[test]
fn validate_known_types_ok() {
    let cases: &[(u16, &[u8])] = &[
        (PointerType::AccountId as u16, b"alice@wonderland"),
        (PointerType::AssetDefinitionId as u16, b"rose#wonderland"),
        (PointerType::Name as u16, b"cursor"),
        (PointerType::Json as u16, br#"{"q":1}"#),
        (PointerType::NftId as u16, b"rose:uuid:0123"),
    ];
    for (type_id, payload) in cases.iter().copied() {
        let tlv = make_tlv(type_id, 1, payload);
        let mut mem = Memory::new(0);
        mem.preload_input(0, &tlv).expect("preload input");
        let v = mem.validate_tlv(Memory::INPUT_START).expect("valid tlv");
        assert_eq!(v.type_id as u16, type_id);
        assert_eq!(v.version, 1);
        assert_eq!(v.payload, payload);
    }
}

#[test]
fn reject_hash_mismatch() {
    let payload = b"alice@wonderland";
    let mut tlv = make_tlv(PointerType::AccountId as u16, 1, payload);
    // Flip one byte in the stored hash
    let off = 7 + payload.len();
    tlv[off] ^= 0xFF;
    let mut mem = Memory::new(0);
    mem.preload_input(0, &tlv).expect("preload input");
    assert!(matches!(
        mem.validate_tlv(Memory::INPUT_START),
        Err(ivm::VMError::NoritoInvalid)
    ));
}

#[test]
fn reject_oob_length() {
    // Construct a TLV that claims a very large payload length
    let _payload = b"x";
    let mut hdr = Vec::new();
    hdr.extend_from_slice(&(PointerType::AccountId as u16).to_be_bytes());
    hdr.push(1);
    hdr.extend_from_slice(&(u32::MAX).to_be_bytes());
    // No payload/hash appended to keep it small in test; bounds check should fail first
    let mut mem = Memory::new(0);
    mem.preload_input(0, &hdr).expect("preload input");
    assert!(matches!(
        mem.validate_tlv(Memory::INPUT_START),
        Err(ivm::VMError::NoritoInvalid)
    ));
}

#[test]
fn reject_unknown_type() {
    let tlv = make_tlv(0xFFFF, 1, b"x");
    let mut mem = Memory::new(0);
    mem.preload_input(0, &tlv).expect("preload input");
    assert!(matches!(
        mem.validate_tlv(Memory::INPUT_START),
        Err(ivm::VMError::NoritoInvalid)
    ));
}

#[test]
fn reject_wrong_version() {
    let tlv = make_tlv(PointerType::Name as u16, 2, b"cursor");
    let mut mem = Memory::new(0);
    mem.preload_input(0, &tlv).expect("preload input");
    assert!(matches!(
        mem.validate_tlv(Memory::INPUT_START),
        Err(ivm::VMError::NoritoInvalid)
    ));
}

#[test]
fn reject_wrong_region() {
    let tlv = make_tlv(PointerType::Name as u16, 1, b"cursor");
    let mut mem = Memory::new(0);
    // Place TLV into HEAP region (writes allowed) but validator should enforce INPUT-only
    let heap_addr = Memory::HEAP_START;
    mem.store_bytes(heap_addr, &tlv).unwrap();
    assert!(matches!(
        mem.validate_tlv(heap_addr),
        Err(ivm::VMError::NoritoInvalid)
    ));
}
