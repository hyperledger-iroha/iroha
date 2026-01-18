//! Regression tests for decode flag state handling and panic safety.

use std::io::{Cursor, Read};

use norito::{
    NoritoSerialize,
    core::{self, Compression, Header, VERSION_MAJOR},
};

fn packed_seq_supported() -> bool {
    cfg!(feature = "packed-seq")
}

fn encode_fixed_vec_u32(values: &[u32]) -> Vec<u8> {
    let mut body = Vec::new();
    body.extend_from_slice(&(values.len() as u64).to_le_bytes());
    for &value in values {
        let bytes = value.to_le_bytes();
        body.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
        body.extend_from_slice(&bytes);
    }
    let checksum = core::hardware_crc64(&body);
    let header = Header::new(
        <Vec<u32> as NoritoSerialize>::schema_hash(),
        body.len() as u64,
        checksum,
    );
    let mut bytes = Vec::with_capacity(Header::SIZE + body.len());
    bytes.extend_from_slice(&header.magic);
    bytes.push(header.major);
    bytes.push(header.minor);
    bytes.extend_from_slice(&header.schema);
    bytes.push(header.compression as u8);
    bytes.extend_from_slice(&header.length.to_le_bytes());
    bytes.extend_from_slice(&header.checksum.to_le_bytes());
    bytes.push(header.flags);
    bytes.extend_from_slice(&body);
    bytes
}

fn encode_fixed_tuple_u8_string(value: u8, text: &str) -> Vec<u8> {
    let mut body = Vec::new();
    body.extend_from_slice(&(1u64).to_le_bytes());
    body.push(value);

    let mut string_field = Vec::new();
    string_field.extend_from_slice(&(text.len() as u64).to_le_bytes());
    string_field.extend_from_slice(text.as_bytes());

    body.extend_from_slice(&(string_field.len() as u64).to_le_bytes());
    body.extend_from_slice(&string_field);

    let checksum = core::hardware_crc64(&body);

    let mut bytes = Vec::with_capacity(Header::SIZE + body.len());
    bytes.extend_from_slice(b"NRT0");
    bytes.push(VERSION_MAJOR);
    bytes.push(0); // default minor (no adaptive features)
    bytes.extend_from_slice(&<(u8, String)>::schema_hash());
    bytes.push(Compression::None as u8);
    bytes.extend_from_slice(&(body.len() as u64).to_le_bytes());
    bytes.extend_from_slice(&checksum.to_le_bytes());
    bytes.push(0); // baseline flags
    bytes.extend_from_slice(&body);
    bytes
}

#[test]
fn decode_mixed_flag_payloads_do_not_leak_state() {
    core::reset_decode_state();
    if !packed_seq_supported() {
        return;
    }

    let canonical = vec![1u32, 2, 3, 4, 5];
    let canonical_bytes = norito::to_bytes(&canonical).expect("encode canonical vec");

    let fixed = vec![10u32, 20, 30];
    let fixed_bytes = encode_fixed_vec_u32(&fixed);

    let decoded_canonical: Vec<u32> =
        norito::decode_from_bytes(&canonical_bytes).expect("decode canonical");
    assert_eq!(decoded_canonical, canonical);

    let err = norito::decode_from_bytes::<Vec<u32>>(&fixed_bytes)
        .expect_err("fixed layout must be rejected");
    assert!(matches!(
        err,
        norito::core::Error::LengthMismatch | norito::core::Error::DecodePanic { .. }
    ));

    // Decode flags should reset even after a failed decode.
    assert_eq!(
        core::get_decode_flags(),
        0,
        "decode flags should reset to zero"
    );

    // Subsequent canonical decodes must keep working.
    let decoded_again: Vec<u32> =
        norito::decode_from_bytes(&canonical_bytes).expect("decode canonical again");
    assert_eq!(decoded_again, canonical);
}

#[test]
fn decode_mixed_flag_payloads_reverse_order() {
    core::reset_decode_state();
    if !packed_seq_supported() {
        return;
    }

    let canonical = vec![7u32, 42, 99];
    let canonical_bytes = norito::to_bytes(&canonical).expect("encode canonical vec");

    let fixed = vec![100u32, 200, 300, 400];
    let fixed_bytes = encode_fixed_vec_u32(&fixed);

    let err =
        norito::decode_from_bytes::<Vec<u32>>(&fixed_bytes).expect_err("fixed decode must fail");
    assert!(matches!(
        err,
        norito::core::Error::LengthMismatch | norito::core::Error::DecodePanic { .. }
    ));
    assert_eq!(
        core::get_decode_flags(),
        0,
        "decode flags reset after failure"
    );

    let decoded_canonical: Vec<u32> =
        norito::decode_from_bytes(&canonical_bytes).expect("decode canonical");
    assert_eq!(decoded_canonical, canonical);
}

#[test]
fn decode_flags_guard_resets_hint_state() {
    use norito::core::{self, header_flags};

    if !packed_seq_supported() {
        return;
    }

    core::reset_decode_state();
    {
        let _guard =
            core::DecodeFlagsGuard::enter(header_flags::PACKED_SEQ | header_flags::COMPACT_LEN);
        assert!(core::use_packed_seq(), "packed seq should be active");
        assert!(core::use_compact_len(), "compact len should be active");
        assert_eq!(
            core::get_decode_flags(),
            header_flags::PACKED_SEQ | header_flags::COMPACT_LEN,
            "decode flags should ignore reserved bits"
        );
    }

    assert_eq!(
        core::get_decode_flags(),
        0,
        "decode flags should reset to zero"
    );

    let _guard = core::DecodeFlagsGuard::enter(0);
    assert!(
        !core::use_packed_seq(),
        "packed seq must remain disabled when header flags are zero"
    );
    assert!(
        !core::use_compact_len(),
        "compact len must remain disabled when header flags are zero"
    );
}

#[test]
fn header_flags_guard_mismatch_fails_fast() {
    use norito::core::header_flags;

    core::reset_decode_state();

    let values = vec![1u32, 2, 3, 4];
    let bytes = norito::to_bytes(&values).expect("encode vec");

    let _guard =
        core::DecodeFlagsGuard::enter(header_flags::PACKED_SEQ | header_flags::COMPACT_LEN);

    let err = match norito::core::from_bytes::<Vec<u32>>(&bytes) {
        Ok(_) => panic!("from_bytes accepted mismatched decode flags"),
        Err(err) => err,
    };
    assert!(matches!(
        err,
        norito::core::Error::DecodeFlagsMismatch {
            header_flags: 0,
            header_hint: 0,
            active_flags,
            active_hint
        } if active_flags
            == (header_flags::PACKED_SEQ | header_flags::COMPACT_LEN)
            && active_hint == active_flags
    ));
}

#[test]
fn tuple_decodes_do_not_leak_layout_flags() {
    core::reset_decode_state();

    let modern = (1u8, String::from("modern"));
    let modern_bytes = norito::to_bytes(&modern).expect("serialize modern tuple");
    let modern_decoded: (u8, String) =
        norito::decode_from_bytes(&modern_bytes).expect("decode modern tuple");
    assert_eq!(modern_decoded, modern);

    let fixed_bytes = encode_fixed_tuple_u8_string(7, "fixed");
    let fixed_decoded: (u8, String) =
        norito::decode_from_bytes(&fixed_bytes).expect("decode fixed tuple");
    assert_eq!(fixed_decoded, (7, "fixed".to_string()));

    assert_eq!(core::get_decode_flags(), 0, "decode flags should reset");
}

#[test]
fn decode_flags_guard_restores_previous_defaults() {
    use norito::core::{self, header_flags};

    if !packed_seq_supported() {
        return;
    }

    core::reset_decode_state();
    core::set_decode_flags(header_flags::PACKED_SEQ | header_flags::COMPACT_LEN);

    assert!(
        core::use_packed_seq(),
        "packed seq should be enabled by decode flags"
    );
    assert!(
        core::use_compact_len(),
        "compact len should be enabled by decode flags"
    );
    assert!(
        !core::use_packed_struct(),
        "packed struct should be disabled in default decode flags"
    );

    {
        let _guard = core::DecodeFlagsGuard::enter(header_flags::PACKED_STRUCT);
        assert!(
            core::use_packed_struct(),
            "packed struct should be enabled inside guard"
        );
        assert!(
            !core::use_packed_seq(),
            "packed seq should be disabled inside guard"
        );
        assert!(
            !core::use_compact_len(),
            "compact len should be disabled inside guard"
        );
    }

    assert!(
        core::use_packed_seq(),
        "packed seq should be restored after guard drop"
    );
    assert!(
        core::use_compact_len(),
        "compact len should be restored after guard drop"
    );
    assert!(
        !core::use_packed_struct(),
        "packed struct should be disabled after guard drop"
    );

    core::reset_decode_state();
    assert_eq!(
        core::get_decode_flags(),
        0,
        "decode flags should reset to zero"
    );
}

#[test]
fn decode_guard_panics_preserve_state() {
    core::reset_decode_state();
    if !packed_seq_supported() {
        return;
    }

    let canonical = vec![8u64, 9, 10];
    let canonical_bytes = norito::to_bytes(&canonical).expect("encode canonical vec");

    let result = std::panic::catch_unwind(|| {
        // Force a panic inside the decode guard using a crafted header that triggers
        // a slice OOB (payload length too small).
        let mut tampered = canonical_bytes.clone();
        tampered[Header::SIZE - 1] = norito::core::header_flags::PACKED_SEQ;
        norito::decode_from_bytes::<Vec<u64>>(&tampered).unwrap();
    });
    assert!(result.is_err(), "guarded decode must panic");

    assert_eq!(
        core::get_decode_flags(),
        0,
        "decode flags should reset even after guard panic"
    );
    core::reset_decode_state();
}

#[test]
fn decode_guard_handles_context_restoration() {
    core::reset_decode_state();
    let payload: Vec<u64> = (0..64u64).collect();
    let payload_bytes = norito::to_bytes(&payload).expect("encode");

    let mut cursor = Cursor::new(payload_bytes.as_slice());
    let mut buf = Vec::new();
    cursor.read_to_end(&mut buf).expect("read");
    let decoded: Vec<u64> = norito::decode_from_bytes(&buf).expect("decode");
    assert_eq!(decoded, payload);

    assert_eq!(
        core::get_decode_flags(),
        0,
        "decode flags should reset after context restoration"
    );
}

#[test]
fn decode_guard_handles_payload_ctx_panic() {
    core::reset_decode_state();

    let payload: Vec<u64> = (0..32u64).collect();
    let payload_bytes = norito::to_bytes(&payload).expect("encode vec");

    let mut tampered = payload_bytes.clone();
    // Corrupt the header minor byte to trigger a panic when the guard tries to combine flags.
    tampered[5] = 0xFF;

    let err =
        norito::decode_from_bytes::<Vec<u64>>(&tampered).expect_err("corrupt header must fail");
    match err {
        norito::core::Error::UnsupportedMinorVersion { .. }
        | norito::core::Error::DecodePanic { .. } => {}
        other => panic!("unexpected error: {other:?}"),
    }

    assert_eq!(
        core::get_decode_flags(),
        0,
        "decode flags should reset after payload ctx panic"
    );
}

#[test]
fn truncated_vec_payload_returns_error() {
    let payload: Vec<u32> = (0..16u32).collect();
    let mut bytes = norito::to_bytes(&payload).expect("encode");
    bytes.truncate(bytes.len().saturating_sub(3));
    let err =
        norito::decode_from_bytes::<Vec<u32>>(&bytes).expect_err("truncated payload must fail");
    match err {
        norito::core::Error::LengthMismatch | norito::core::Error::DecodePanic { .. } => {}
        other => panic!("unexpected error variant: {other:?}"),
    }
}

#[test]
fn decode_from_bytes_resets_decode_state() {
    use norito::core::{self, header_flags};

    if !packed_seq_supported() {
        return;
    }

    core::set_decode_flags(header_flags::PACKED_SEQ | header_flags::COMPACT_LEN);

    let fixed = vec![1u32, 2, 3];
    let fixed_bytes = encode_fixed_vec_u32(&fixed);

    let err =
        norito::decode_from_bytes::<Vec<u32>>(&fixed_bytes).expect_err("fixed decode must fail");
    assert!(matches!(
        err,
        norito::core::Error::LengthMismatch | norito::core::Error::DecodePanic { .. }
    ));
    assert_eq!(
        core::get_decode_flags(),
        0,
        "decode flags should reset to 0 on error"
    );
    core::reset_decode_state();
}

#[test]
fn stream_vec_collect_matches_decode() {
    let payload: Vec<u64> = (0..256u64).collect();
    let bytes = norito::to_bytes(&payload).expect("encode");
    let streamed: Vec<u64> = norito::stream_vec_collect_from_reader(Cursor::new(bytes.as_slice()))
        .expect("stream decode");
    assert_eq!(streamed, payload);

    let decoded: Vec<u64> = norito::decode_from_bytes(&bytes).expect("decode");
    assert_eq!(decoded, payload);
}

#[test]
fn stream_vec_collect_preallocates_capacity() {
    let payload: Vec<u32> = (0..128u32).collect();
    let bytes = norito::to_bytes(&payload).expect("encode");

    let streamed: Vec<u32> = norito::stream_vec_collect_from_reader(Cursor::new(bytes.as_slice()))
        .expect("stream collect");
    assert_eq!(payload, streamed);
    assert_eq!(streamed.capacity(), streamed.len());

    let folded_sum =
        norito::stream_vec_fold_from_reader(Cursor::new(bytes.as_slice()), 0u64, |acc, v: u32| {
            acc + u64::from(v)
        })
        .expect("stream fold");
    let baseline: u64 = payload.iter().copied().map(u64::from).sum();
    assert_eq!(folded_sum, baseline);
}

#[test]
fn stream_vec_collect_handles_empty_payload() {
    let payload: Vec<u32> = Vec::new();
    let bytes = norito::to_bytes(&payload).expect("encode empty vec");

    let streamed: Vec<u32> = norito::stream_vec_collect_from_reader(Cursor::new(bytes.as_slice()))
        .expect("stream empty vec");
    assert!(streamed.is_empty());

    let decoded: Vec<u32> = norito::decode_from_bytes(&bytes).expect("decode empty vec");
    assert!(decoded.is_empty());
}

#[test]
fn stream_vec_collect_handles_fixed_layout() {
    core::reset_decode_state();
    if !packed_seq_supported() {
        return;
    }

    let fixed = vec![11u32, 22, 33];
    let fixed_bytes = encode_fixed_vec_u32(&fixed);

    let err = norito::stream_vec_collect_from_reader::<_, u32>(Cursor::new(fixed_bytes.as_slice()))
        .expect_err("streaming fixed vec must fail");
    assert!(matches!(
        err,
        norito::core::Error::LengthMismatch | norito::core::Error::DecodePanic { .. }
    ));

    let decode_err = norito::decode_from_bytes::<Vec<u32>>(&fixed_bytes)
        .expect_err("decoding fixed vec must fail");
    assert!(matches!(
        decode_err,
        norito::core::Error::LengthMismatch | norito::core::Error::DecodePanic { .. }
    ));
}

#[test]
fn stream_vec_collect_handles_packed_seq() {
    if !packed_seq_supported() {
        return;
    }

    let payload: Vec<u64> = (0..2048u64).collect();
    let _guard = core::DecodeFlagsGuard::enter(norito::core::header_flags::PACKED_SEQ);
    let bytes = norito::to_bytes(&payload).expect("encode large vec");

    let flags = bytes[Header::SIZE - 1];
    assert_eq!(
        flags & norito::core::header_flags::PACKED_SEQ,
        norito::core::header_flags::PACKED_SEQ,
        "packed sequence flag should be set",
    );
    assert_eq!(
        flags & norito::core::header_flags::VARINT_OFFSETS,
        0,
        "packed layout should not advertise varint offsets in v1"
    );

    let streamed: Vec<u64> = norito::stream_vec_collect_from_reader(Cursor::new(bytes.as_slice()))
        .expect("stream decode with offsets");
    assert_eq!(streamed, payload);

    let decoded: Vec<u64> = norito::decode_from_bytes(&bytes).expect("decode with offsets");
    assert_eq!(decoded, payload);
}
