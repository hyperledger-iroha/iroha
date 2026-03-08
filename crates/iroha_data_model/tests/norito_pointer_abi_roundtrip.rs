//! Pointer-ABI Norito roundtrip tests for manifest and NFT syscalls.

use iroha_crypto::{Hash, KeyPair};
use iroha_data_model::{
    account::AccountId,
    domain::prelude::DomainId,
    name::Name,
    nft::NftId,
    smart_contract::manifest::{AccessSetHints, ContractManifest},
};
use ivm::{PointerType, validate_tlv_bytes};
use norito::{
    codec::{Decode, Encode},
    decode_from_bytes, to_bytes,
};

fn make_tlv(type_id: PointerType, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(2 + 1 + 4 + payload.len() + Hash::LENGTH);
    out.extend_from_slice(&(type_id as u16).to_be_bytes());
    out.push(1); // version
    let payload_len =
        u32::try_from(payload.len()).expect("payload length fits within TLV header encoding");
    out.extend_from_slice(&payload_len.to_be_bytes());
    out.extend_from_slice(payload);
    let digest: [u8; Hash::LENGTH] = Hash::new(payload).into();
    out.extend_from_slice(&digest);
    out
}

#[test]
fn manifest_pointer_roundtrip() {
    let account_id = AccountId::new(
        "wonderland".parse().expect("domain id"),
        KeyPair::random().public_key().clone(),
    );
    let manifest = ContractManifest {
        code_hash: Some(Hash::new(b"code-bytes")),
        abi_hash: Some(Hash::new(b"abi-policy")),
        compiler_fingerprint: Some("kotodama-1.2.3+nightly".to_owned()),
        features_bitmap: Some(0b1010_0101),
        access_set_hints: Some(AccessSetHints {
            read_keys: vec![
                format!("account:{account_id}"),
                "asset:rose#wonderland".to_owned(),
            ],
            write_keys: vec!["asset.detail:rose#wonderland:balance".to_owned()],
        }),
        entrypoints: None,
        kotoba: None,
        provenance: None,
    };

    let payload = to_bytes(&manifest).expect("encode manifest");
    let tlv = make_tlv(PointerType::NoritoBytes, &payload);

    let view = validate_tlv_bytes(&tlv).expect("well-formed TLV");
    assert_eq!(view.type_id, PointerType::NoritoBytes);

    let decoded: ContractManifest =
        decode_from_bytes(view.payload).expect("decode manifest payload");
    assert_eq!(decoded, manifest);
}

#[test]
fn nft_syscall_pointers_roundtrip() {
    let keypair = KeyPair::random();
    let (public_key, _) = keypair.into_parts();
    let domain: DomainId = "wonderland".parse().expect("domain id");
    let account_id = AccountId::new(domain.clone(), public_key);

    let nft_name: Name = "collectible".parse().expect("valid name");
    let nft_id = NftId::of(domain, nft_name);

    let account_payload = account_id.encode();
    let nft_payload = nft_id.encode();

    let account_tlv = make_tlv(PointerType::AccountId, &account_payload);
    let nft_tlv = make_tlv(PointerType::NftId, &nft_payload);

    let account_view = validate_tlv_bytes(&account_tlv).expect("account TLV must validate");
    assert_eq!(account_view.type_id, PointerType::AccountId);
    let decoded_account =
        AccountId::decode(&mut &*account_view.payload).expect("decode account id");
    assert_eq!(decoded_account, account_id);

    let nft_view = validate_tlv_bytes(&nft_tlv).expect("nft TLV must validate");
    assert_eq!(nft_view.type_id, PointerType::NftId);
    let decoded_nft = NftId::decode(&mut &*nft_view.payload).expect("decode nft id");
    assert_eq!(decoded_nft, nft_id);
}
