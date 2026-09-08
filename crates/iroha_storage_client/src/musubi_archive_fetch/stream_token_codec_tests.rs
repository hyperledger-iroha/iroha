//! Bounded canonical token transport tests; decoded signature bytes remain untrusted claims.
use super::*;
use sorafs_manifest::StreamTokenBodyV1;

fn token() -> StreamTokenV1 {
    StreamTokenV1 {
        body: StreamTokenBodyV1 {
            token_id: "aa".repeat(16),
            manifest_cid: vec![1; 128],
            provider_id: [0x31; 32],
            profile_handle: "a".repeat(128),
            max_streams: 1024,
            ttl_epoch: 3601,
            rate_limit_bytes: 1_073_741_824,
            issued_at: 1,
            requests_per_minute: 10_000,
            token_pk_version: u32::MAX,
        },
        signature: vec![0x32; 64],
    }
}

fn layouts() -> [u8; 10] {
    use norito::core::header_flags::{COMPACT_LEN, FIELD_BITSET, PACKED_SEQ, PACKED_STRUCT};
    [
        0,
        COMPACT_LEN,
        PACKED_SEQ,
        PACKED_SEQ | COMPACT_LEN,
        PACKED_STRUCT,
        PACKED_STRUCT | COMPACT_LEN,
        PACKED_SEQ | PACKED_STRUCT,
        PACKED_SEQ | PACKED_STRUCT | COMPACT_LEN,
        PACKED_STRUCT | COMPACT_LEN | FIELD_BITSET,
        PACKED_SEQ | PACKED_STRUCT | COMPACT_LEN | FIELD_BITSET,
    ]
}

fn assert_invalid(encoded: &str) {
    let error = decode_stream_token_exact(encoded).unwrap_err();
    assert_eq!(error.class(), MusubiArchiveRuntimeFailureClassV1::Integrity);
    assert_eq!(error.code(), "MUSUBI_ARCHIVE_TOKEN_RESPONSE_INVALID");
}

#[test]
fn maximum_token_claim_is_canonical_and_bounded_in_all_ten_layouts() {
    let token = token();
    let bytes = norito::encode_canonical(&token).unwrap();
    assert!(bytes.len() < STREAM_TOKEN_MAX_WIRE_BYTES_V1);
    let encoded = STANDARD.encode(&bytes);
    for flags in layouts() {
        let _guard = norito::core::DecodeFlagsGuard::enter(flags);
        let (result, usage) = norito::core::with_decode_limits_measured(
            norito::DecodeLimits::new(2048, 2048, 4096, 8192, 32),
            || decode_stream_token_exact(&encoded),
        );
        assert_eq!(result.unwrap(), token);
        assert!(usage.total_allocated_bytes() > 0);
        assert!(usage.total_allocated_bytes() <= 8192);
        assert_eq!(norito::encode_canonical(&token).unwrap(), bytes);
    }
}

#[test]
fn alternate_token_frames_never_gain_authority_from_ambient_encoding_flags() {
    let token = token();
    let canonical = norito::encode_canonical(&token).unwrap();
    let mut alternate_count = 0;
    for flags in layouts() {
        let _guard = norito::core::DecodeFlagsGuard::enter(flags);
        let alternate = norito::to_bytes(&token).unwrap();
        assert_eq!(
            norito::decode_from_bytes::<StreamTokenV1>(&alternate).unwrap(),
            token
        );
        assert_eq!(
            decode_stream_token_exact(&STANDARD.encode(&canonical)).unwrap(),
            token
        );
        if alternate != canonical {
            alternate_count += 1;
            assert_invalid(&STANDARD.encode(alternate));
        }
    }
    assert!(
        alternate_count > 0,
        "reject genuinely different ordinary-decodable frames"
    );
}

#[test]
fn forbidden_compression_and_oversized_headers_fail_before_norito_allocation() {
    let bytes = norito::encode_canonical(&token()).unwrap();
    let header = norito::core::Header::read(bytes.as_slice()).unwrap();
    assert_eq!(header.compression, norito::Compression::None);
    let mut compressed = bytes;
    compressed[header.magic.len() + 2 + header.schema.len()] = norito::Compression::Zstd as u8;
    assert_eq!(
        norito::core::Header::read(compressed.as_slice())
            .unwrap()
            .compression,
        norito::Compression::Zstd
    );
    for invalid in [
        STANDARD.encode(&compressed),
        STANDARD.encode(&compressed[..norito::core::Header::SIZE]),
        STANDARD.encode(vec![0; STREAM_TOKEN_MAX_WIRE_BYTES_V1 + 1]),
        "a".repeat(STREAM_TOKEN_MAX_BASE64_BYTES_V1 + 1),
        String::new(),
    ] {
        let (result, usage) = norito::core::with_decode_limits_measured(
            norito::DecodeLimits::new(2048, 2048, 4096, 8192, 32),
            || decode_stream_token_exact(&invalid),
        );
        assert!(result.is_err());
        assert_eq!(usage.total_allocated_bytes(), 0);
        assert_invalid(&invalid);
    }
}

#[test]
fn canonical_transport_preserves_body_and_base64_rejection_policy() {
    let token = token();
    let bytes = norito::encode_canonical(&token).unwrap();
    assert_eq!(
        decode_stream_token_exact(&STANDARD.encode(&bytes)).unwrap(),
        token
    );
    for value in [
        format!(" {}", STANDARD.encode(&bytes)),
        format!("{}\n", STANDARD.encode(&bytes)),
        "_bad".to_owned(),
    ] {
        assert_invalid(&value);
    }
    let mut suffix = bytes.clone();
    suffix.push(0);
    assert_invalid(&STANDARD.encode(suffix));
    assert_invalid(&STANDARD.encode(&bytes[..bytes.len() - 1]));
    for fault in 0..3 {
        let mut wrong = token.clone();
        match fault {
            0 => wrong.body.requests_per_minute = 0,
            1 => wrong.body.ttl_epoch = wrong.body.issued_at,
            _ => wrong.body.ttl_epoch = 0,
        }
        assert_invalid(&STANDARD.encode(norito::encode_canonical(&wrong).unwrap()));
    }
}
