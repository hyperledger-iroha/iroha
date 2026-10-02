//! Maintained codec reconstruction over every original platform byte width; no signature grant.
use super::super::{KagemushaOrdinaryCashClockContextV1, KagemushaOrdinaryPaymentRequestBodyV1};
use super::*;
fn request() -> KagemushaOrdinaryPaymentRequestV1 {
    let mut key = [0; 32];
    key[0] = 9;
    KagemushaOrdinaryPaymentRequestV1 {
        body: KagemushaOrdinaryPaymentRequestBodyV1 {
            version: 1,
            release_id: [3; 32],
            network_id: [4; 32],
            normalized_asset_id: [5; 32],
            asset_incarnation: [6; 32],
            scale: 2,
            reserve_pool_id: [7; 32],
            recipient_account_binding: [8; 32],
            amount: (1u128 << 105) + 17,
            recipient_encryption_key: key,
            recipient_credential_digest: [9; 32],
            recipient_lane_id: [10; 32],
            request_id: [11; 32],
            clock_context: KagemushaOrdinaryCashClockContextV1 {
                version: 1,
                request_nonce: [1; 32],
                signed_observations_original_digest: [2; 32],
                lower_at_ms: 1001,
                upper_at_ms: 1002,
            },
            issued_at_ms: 1000,
            expires_at_ms: 2000,
        },
        evidence: KagemushaAppOperationApprovalEvidenceV1::AndroidKeystore {
            signature_der: vec![0x5a; 8],
        },
    }
}
fn full_original(value: &KagemushaOrdinaryPaymentRequestV1) -> Vec<u8> {
    let frame = norito::encode_canonical(value).unwrap();
    let mut bytes = ORIGINAL_DOMAIN.to_vec();
    bytes.extend((frame.len() as u64).to_le_bytes());
    bytes.extend(frame);
    bytes
}
#[test]
fn ordinary_request_canonical_stream_matches_sole_encoder_all376_platform_widths_and_semantics() {
    let base = request();
    let grammar = base.original_canonical_stream_grammar().unwrap();
    assert_eq!(grammar.variants.len(), 376);
    assert_eq!(grammar.repeated_evidence_byte_unit.first(), Some(&None));
    for (index, variant) in grammar.variants.iter().enumerate() {
        assert_eq!(variant.apple, index >= 65);
        assert_eq!(
            variant.evidence_length,
            if index < 65 { index + 8 } else { index - 64 }
        );
        let mut value = base.clone();
        // The same grammar constants must survive different full financial/request/clock fields.
        value.body.amount ^= 1u128 << 127;
        value.body.network_id[31] ^= 1;
        value.body.request_id[0] ^= 1;
        value.body.clock_context.signed_observations_original_digest[31] ^= 1;
        value.evidence = if variant.apple {
            KagemushaAppOperationApprovalEvidenceV1::AppleAppAttest {
                raw_assertion: (0..variant.evidence_length)
                    .map(|i| (i as u8).wrapping_mul(13))
                    .collect(),
            }
        } else {
            KagemushaAppOperationApprovalEvidenceV1::AndroidKeystore {
                signature_der: (0..variant.evidence_length)
                    .map(|i| (i as u8).wrapping_mul(17))
                    .collect(),
            }
        };
        let original = full_original(&value);
        assert!(original.len() <= grammar.maximum_stream_bytes);
        let native_layout = value.original_preimage_layout_for_specimen().unwrap();
        assert_eq!(native_layout.fields.len(), variant.layout.fields.len());
        for (current, fixed) in native_layout.fields.iter().zip(&variant.layout.fields) {
            assert_eq!(current.name, fixed.name);
            assert_eq!(current.positions, fixed.positions);
        }
        let mut assembled = variant.prefix.clone();
        for _ in 0..variant.evidence_length - 1 {
            assembled.extend(&grammar.repeated_evidence_byte_unit)
        }
        assembled.push(None);
        assembled.extend(&variant.suffix);
        assert_eq!(assembled, variant.layout.bytes);
        for (position, template) in assembled.into_iter().enumerate() {
            if let Some(constant) = template {
                assert_eq!(
                    constant, original[position],
                    "variant {index} syntax byte {position}"
                );
            }
        }
        let frame = &original[variant.layout.original.clone()];
        let header = norito::core::Header::read(frame).unwrap();
        assert_eq!(header.length as usize, variant.archive_payload.len());
        assert_eq!(
            variant.archive_payload.start,
            native_layout
                .original
                .start
                .checked_add(native_layout.payload_offset)
                .unwrap()
        );
        assert_eq!(variant.archive_payload.end, native_layout.original.end);
        assert!(
            original[native_layout.original.start + norito::core::Header::SIZE
                ..variant.archive_payload.start]
                .iter()
                .all(|byte| *byte == 0)
        );
        assert_eq!(
            header.checksum,
            norito::crc64_fallback(&original[variant.archive_payload.clone()])
        );
        assert_eq!(
            variant.header_payload_length_bytes.map(|p| original[p]),
            header.length.to_le_bytes()
        );
        assert_eq!(
            variant.header_crc_bytes.map(|p| original[p]),
            header.checksum.to_le_bytes()
        );
        assert_eq!(
            variant.original_length_bytes.map(|p| original[p]),
            (frame.len() as u64).to_le_bytes()
        );
        let decoded: KagemushaOrdinaryPaymentRequestV1 = norito::decode_canonical(frame).unwrap();
        assert_eq!(decoded, value);
        // These are inert codec originals. Their framing cannot upgrade to authenticated evidence.
        assert!(decoded.validate_shape().is_err());
    }
}
#[test]
fn ordinary_request_stream_body_validation_and_platform_constants_are_independent_of_specimen_width()
 {
    let base = request();
    let a = base.original_canonical_stream_grammar().unwrap();
    let mut wide = base.clone();
    wide.evidence = KagemushaAppOperationApprovalEvidenceV1::AppleAppAttest {
        raw_assertion: vec![0; 311],
    };
    let b = wide.original_canonical_stream_grammar().unwrap();
    assert_eq!(a.maximum_stream_bytes, b.maximum_stream_bytes);
    assert_eq!(a.repeated_evidence_byte_unit, b.repeated_evidence_byte_unit);
    for (a, b) in a.variants.iter().zip(&b.variants) {
        assert_eq!(a.layout.bytes, b.layout.bytes);
        assert_eq!(a.prefix, b.prefix);
        assert_eq!(a.suffix, b.suffix);
    }
    for mutation in 0..5 {
        let mut bad = base.clone();
        match mutation {
            0 => bad.body.version = 2,
            1 => bad.body.amount = 0,
            2 => bad.body.request_id = [0; 32],
            3 => bad.body.clock_context.lower_at_ms = 3000,
            _ => bad.body.expires_at_ms = bad.body.issued_at_ms,
        }
        assert!(bad.original_canonical_stream_grammar().is_err());
    }
}
