//! Owner validation and structural-wire regression tests for canonical chain labels.

use super::*;

#[derive(Encode)]
struct UncheckedChainIdWire(Box<str>);
#[derive(Encode)]
struct ChainIdEnvelope(ChainId);
#[test]
fn chain_id_from_str() {
    let id: ChainId = "test".parse().expect("valid chain id");
    assert_eq!(id, ChainId::from("test"));
}
#[test]
fn chain_id_enforces_canonical_ascii_and_byte_limit() {
    let boundary = format!("a{}z", "0".repeat(MAX_CHAIN_ID_BYTES - 2));
    assert_eq!(
        boundary
            .parse::<ChainId>()
            .expect("boundary chain id")
            .as_str(),
        boundary
    );
    for invalid in [
        String::new(),
        "-leading".to_owned(),
        "trailing-".to_owned(),
        "white space".to_owned(),
        "control\u{0000}".to_owned(),
        "unicode-é".to_owned(),
        "a".repeat(MAX_CHAIN_ID_BYTES + 1),
    ] {
        assert!(
            invalid.parse::<ChainId>().is_err(),
            "invalid chain id was accepted: {invalid:?}"
        );
        assert!(
            ChainId::try_from(invalid.clone()).is_err(),
            "owned invalid chain id was accepted: {invalid:?}"
        );
    }
}
#[test]
fn chain_id_uses_one_canonical_structural_v1_wire_layout() {
    let id = ChainId::from("test");
    let encoded = id.encode();
    assert_eq!(encoded, [5, 4, b't', b'e', b's', b't']);
    assert_eq!(id.encoded_len(), encoded.len());
    assert_eq!(
        ChainIdEnvelope(id.clone()).encode(),
        [6, 5, 4, b't', b'e', b's', b't']
    );
    let mut cursor = encoded.as_slice();
    assert_eq!(ChainId::decode(&mut cursor).expect("bare roundtrip"), id);
    assert_eq!(
        ChainId::decode_from_slice(&encoded).expect("slice roundtrip"),
        (id.clone(), encoded.len())
    );
    let framed = norito::to_bytes(&id).expect("frame ChainId");
    assert_eq!(
        norito::decode_from_bytes::<ChainId>(&framed).expect("framed roundtrip"),
        id
    );
    let transparent = "test".to_owned().encode();
    assert!(
        ChainId::decode_from_slice(&transparent).is_err(),
        "the transient transparent representation must not be accepted"
    );
    let mut truncated = encoded.clone();
    truncated.pop();
    assert!(ChainId::decode_from_slice(&truncated).is_err());
    let mut trailing = encoded;
    trailing.push(0);
    assert!(ChainId::decode_from_slice(&trailing).is_err());
}
#[test]
fn chain_id_norito_decoders_cannot_bypass_validation() {
    for invalid in [
        String::new(),
        "bidi\u{202e}".to_owned(),
        "x".repeat(MAX_CHAIN_ID_BYTES + 1),
    ] {
        // Construct the structural representation without calling the
        // validating public constructor.
        let unchecked = UncheckedChainIdWire(invalid.clone().into_boxed_str());
        let encoded = unchecked.encode();
        let mut cursor = encoded.as_slice();
        assert!(
            ChainId::decode(&mut cursor).is_err(),
            "codec accepted invalid ChainId: {invalid:?}"
        );
        assert!(
            ChainId::decode_from_slice(&encoded).is_err(),
            "slice decoder accepted invalid ChainId: {invalid:?}"
        );
        let (payload, flags) = norito::codec::encode_with_header_flags(&unchecked);
        let framed = norito::core::frame_bare_with_header_flags::<ChainId>(&payload, flags)
            .expect("frame invalid structural ChainId fixture");
        assert!(
            norito::decode_from_bytes::<ChainId>(&framed).is_err(),
            "framed decoder accepted invalid ChainId: {invalid:?}"
        );
    }
}
#[test]
fn chain_id_decoder_rejects_declared_oversize_before_body_access() {
    let mut inner = Vec::new();
    norito::core::write_len_to_vec(
        &mut inner,
        u64::try_from(MAX_CHAIN_ID_BYTES + 1).expect("chain id limit fits u64"),
    );
    let mut declared_oversize = Vec::new();
    norito::core::write_len_to_vec(
        &mut declared_oversize,
        u64::try_from(inner.len()).expect("inner header length fits u64"),
    );
    declared_oversize.extend_from_slice(&inner);
    let error = ChainId::decode_from_slice(&declared_oversize)
        .expect_err("oversized declared ChainId must fail before reading the body");
    assert!(
        error.to_string().contains("128-byte"),
        "decoder reached a generic truncation error before the ChainId limit: {error}"
    );
}

#[test]
fn chain_id_json_decoder_enforces_the_same_invariant() {
    for invalid in [
        "\"\"".to_owned(),
        "\"white space\"".to_owned(),
        format!("\"{}\"", "x".repeat(MAX_CHAIN_ID_BYTES + 1)),
    ] {
        assert!(
            norito::json::from_str::<ChainId>(&invalid).is_err(),
            "JSON accepted invalid ChainId: {invalid}"
        );
    }
}

fn valid_layouts() -> Vec<u8> {
    let flags: Vec<_> = (0..=u8::MAX)
        .filter(|flags| norito::core::validate_header_flags(*flags).is_ok())
        .collect();
    assert_eq!(flags.len(), 10, "exercise every valid V1 layout");
    flags
}

#[test]
fn chain_id_preserves_structural_wire_in_every_valid_v1_layout() {
    for label in [
        "base-fixture-1".to_owned(),
        "CHAIN.a_b:c-1".to_owned(),
        "a".repeat(MAX_CHAIN_ID_BYTES),
    ] {
        let chain: ChainId = label.parse().expect("canonical chain label");
        let canonical = norito::encode_canonical(&chain).expect("canonical chain frame");
        for flags in valid_layouts() {
            let _layout = norito::core::DecodeFlagsGuard::enter(flags);
            let (payload, actual_flags) = norito::codec::encode_with_header_flags(&chain);
            assert_eq!(
                (payload.clone(), actual_flags),
                norito::codec::encode_with_header_flags(&ChainIdWire(ChainIdText(chain.clone()))),
                "the public owner preserves its structural field layout for {flags:#04x}"
            );
            let frame =
                norito::core::frame_bare_with_header_flags::<ChainId>(&payload, actual_flags)
                    .expect("frame chain label under its declared layout");
            assert_eq!(norito::to_bytes(&chain).unwrap(), frame);
            assert_eq!(
                ChainId::decode_from_slice(&payload).expect("decode structural chain payload"),
                (chain.clone(), payload.len())
            );
            let decoded: ChainId = norito::decode_from_bytes(&frame).unwrap();
            assert_eq!(decoded, chain);
            assert_eq!(norito::to_bytes(&decoded).unwrap(), frame);
            assert_eq!(norito::encode_canonical(&decoded).unwrap(), canonical);
            assert_eq!(
                norito::decode_canonical::<ChainId>(&canonical).unwrap(),
                chain
            );
            assert_eq!(
                norito::to_bytes(&chain).unwrap(),
                frame,
                "canonical operations restore the ambient layout"
            );
        }
    }
}

#[test]
fn chain_id_rejects_unchecked_labels_in_every_valid_v1_layout() {
    let chain = ChainId::from("valid-chain");
    for flags in valid_layouts() {
        let _layout = norito::core::DecodeFlagsGuard::enter(flags);
        let valid = norito::to_bytes(&chain).expect("valid chain frame");
        for invalid in [
            String::new(),
            "white space".to_owned(),
            "x".repeat(MAX_CHAIN_ID_BYTES + 1),
        ] {
            let unchecked = UncheckedChainIdWire(invalid.into_boxed_str());
            let (payload, actual_flags) = norito::codec::encode_with_header_flags(&unchecked);
            let frame =
                norito::core::frame_bare_with_header_flags::<ChainId>(&payload, actual_flags)
                    .expect("frame unchecked chain label using the public identity");
            assert!(ChainId::decode_from_slice(&payload).is_err());
            assert!(norito::decode_from_bytes::<ChainId>(&frame).is_err());
            assert_eq!(
                norito::decode_from_bytes::<ChainId>(&valid).unwrap(),
                chain,
                "rejection preserves the enclosing decode context"
            );
        }
    }
}
