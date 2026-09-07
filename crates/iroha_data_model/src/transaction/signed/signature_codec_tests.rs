//! Transaction signature tuple framing and exact-consumption regressions.

use super::{Signature, SignatureOf, TransactionSignature};
use norito::core::{DecodeFlagsGuard, DecodeFromSlice, header_flags};

fn signature() -> TransactionSignature {
    TransactionSignature(SignatureOf::from_signature(Signature::from_bytes(
        &(1..=64).collect::<Vec<u8>>(),
    )))
}

fn layouts() -> [u8; 6] {
    [
        0,
        header_flags::COMPACT_LEN,
        header_flags::PACKED_SEQ,
        header_flags::PACKED_SEQ | header_flags::COMPACT_LEN,
        header_flags::PACKED_STRUCT | header_flags::COMPACT_LEN,
        header_flags::PACKED_STRUCT
            | header_flags::PACKED_SEQ
            | header_flags::COMPACT_LEN
            | header_flags::FIELD_BITSET,
    ]
}

#[test]
fn transaction_signature_slice_matches_declared_tuple_layouts() {
    let signature = signature();
    for requested in layouts() {
        let (payload, flags) = {
            let _flags = DecodeFlagsGuard::enter(requested);
            norito::codec::encode_with_header_flags(&signature)
        };
        let framed =
            norito::core::frame_bare_with_header_flags::<TransactionSignature>(&payload, flags)
                .expect("frame signature tuple");
        let decoded = norito::core::decode_from_bytes::<TransactionSignature>(&framed)
            .expect("decode advertised signature tuple");
        assert_eq!(decoded, signature, "layout {flags:#x}");

        let _flags = DecodeFlagsGuard::enter(flags);
        let (decoded, used) = TransactionSignature::decode_from_slice(&payload)
            .expect("slice decode advertised signature tuple");
        assert_eq!(used, payload.len());
        assert_eq!(decoded, signature);
        assert_eq!(
            norito::to_bytes(&decoded).expect("re-encode signature tuple"),
            framed,
            "decoding must preserve exact frame bytes for layout {flags:#x}"
        );
    }
}

#[test]
fn transaction_signature_slice_rejects_truncated_and_trailing_payloads() {
    for requested in layouts() {
        let (payload, flags) = {
            let _flags = DecodeFlagsGuard::enter(requested);
            norito::codec::encode_with_header_flags(&signature())
        };
        let _flags = DecodeFlagsGuard::enter(flags);
        for end in 0..payload.len() {
            assert!(
                TransactionSignature::decode_from_slice(&payload[..end]).is_err(),
                "accepted truncated tuple at {end} for layout {flags:#x}"
            );
        }
        let mut trailing = payload;
        trailing.push(0);
        assert!(
            TransactionSignature::decode_from_slice(&trailing).is_err(),
            "accepted trailing tuple bytes for layout {flags:#x}"
        );
    }
}

#[test]
fn transaction_signature_rejects_unwrapped_inner_signature() {
    let signature = signature();
    for requested in layouts() {
        let (inner, flags) = {
            let _flags = DecodeFlagsGuard::enter(requested);
            norito::codec::encode_with_header_flags(&signature.0)
        };
        let framed =
            norito::core::frame_bare_with_header_flags::<TransactionSignature>(&inner, flags)
                .expect("frame deliberately unwrapped signature");
        assert!(
            norito::core::decode_from_bytes::<TransactionSignature>(&framed).is_err(),
            "accepted unwrapped signature for tuple layout {flags:#x}"
        );
    }
}
