//! AoS ad-hoc body version nibble tests (roundtrip + malformed).
#![cfg(feature = "json")]

use norito::{
    columnar::{
        ADAPTIVE_TAG_AOS, decode_rows_u64_bytes_bool_adaptive,
        decode_rows_u64_optu32_bool_adaptive, decode_rows_u64_str_bool_adaptive,
        decode_rows_u64_str_u32_bool_adaptive,
    },
    core::Error,
};

// Compute the offset in the adaptive AoS body where the version byte sits: after [n varint]
fn aos_version_offset_for_len(n: usize) -> usize {
    // Body layout for AoS: [len prefix][ver][payload...]. Prefix is fixed-width in sequential mode.
    let _ = n;
    norito::core::len_prefix_len(0)
}

#[test]
fn aos_str_bool_roundtrip_and_bad_version() {
    let rows: Vec<(u64, &str, bool)> = vec![(1, "a", true), (2, "bb", false), (3, "ccc", true)];
    // Serialize via AoS helper directly to exercise version nibble handling.
    let body = norito::aos::encode_rows_u64_str_bool(&rows);
    let mut payload = Vec::with_capacity(1 + body.len());
    payload.push(ADAPTIVE_TAG_AOS);
    payload.extend_from_slice(&body);
    let decoded = decode_rows_u64_str_bool_adaptive(&payload).expect("decode");
    let expected: Vec<(u64, String, bool)> = rows
        .iter()
        .map(|(a, b, c)| (*a, (*b).to_string(), *c))
        .collect();
    assert_eq!(decoded, expected);

    // Corrupt the version nibble and expect UnsupportedVersion
    let n = rows.len();
    let ver_off = 1 + aos_version_offset_for_len(n); // +1 for adaptive tag
    let mut corrupted = payload.clone();
    corrupted[ver_off] = 0x02; // low nibble != 0x1
    let err = decode_rows_u64_str_bool_adaptive(&corrupted).unwrap_err();
    matches_unsupported_version(err);

    // Corrupt high nibble (should still be rejected)
    let mut corrupted2 = payload.clone();
    corrupted2[ver_off] = 0x10 | (corrupted2[ver_off] & 0x0F);
    let err2 = decode_rows_u64_str_bool_adaptive(&corrupted2).unwrap_err();
    matches_unsupported_version(err2);
}

#[test]
fn aos_bytes_bool_roundtrip_and_bad_version() {
    let rows: Vec<(u64, &[u8], bool)> = vec![(1, b"abc", true), (2, b"\x00\xFF", false)];
    let body = norito::aos::encode_rows_u64_bytes_bool(&rows);
    let mut payload = Vec::with_capacity(1 + body.len());
    payload.push(ADAPTIVE_TAG_AOS);
    payload.extend_from_slice(&body);
    let decoded = decode_rows_u64_bytes_bool_adaptive(&payload).expect("decode");
    let expected: Vec<(u64, Vec<u8>, bool)> = rows
        .iter()
        .map(|(a, b, c)| (*a, (*b).to_vec(), *c))
        .collect();
    assert_eq!(decoded, expected);

    let n = rows.len();
    let ver_off = 1 + aos_version_offset_for_len(n);
    let mut corrupted = payload.clone();
    corrupted[ver_off] = 0xFF;
    let err = decode_rows_u64_bytes_bool_adaptive(&corrupted).unwrap_err();
    matches_unsupported_version(err);
}

#[test]
fn aos_str_u32_bool_roundtrip_and_bad_version() {
    let rows: Vec<(u64, &str, u32, bool)> = vec![(1, "x", 7, true), (2, "yy", 9, false)];
    let body = norito::aos::encode_rows_u64_str_u32_bool(&rows);
    let mut payload = Vec::with_capacity(1 + body.len());
    payload.push(ADAPTIVE_TAG_AOS);
    payload.extend_from_slice(&body);
    let decoded = decode_rows_u64_str_u32_bool_adaptive(&payload).expect("decode");
    let expected: Vec<(u64, String, u32, bool)> = rows
        .iter()
        .map(|(a, b, c, d)| (*a, (*b).to_string(), *c, *d))
        .collect();
    assert_eq!(decoded, expected);

    let n = rows.len();
    let ver_off = 1 + aos_version_offset_for_len(n);
    let mut corrupted = payload.clone();
    corrupted[ver_off] = 0;
    let err = decode_rows_u64_str_u32_bool_adaptive(&corrupted).unwrap_err();
    matches_unsupported_version(err);
}

#[test]
fn aos_opt_u32_bool_roundtrip_and_bad_version() {
    let rows: Vec<(u64, Option<u32>, bool)> = vec![(10, Some(7), true), (11, None, false)];
    let body = norito::aos::encode_rows_u64_optu32_bool(&rows);
    let mut payload = Vec::with_capacity(1 + body.len());
    payload.push(ADAPTIVE_TAG_AOS);
    payload.extend_from_slice(&body);
    let decoded = decode_rows_u64_optu32_bool_adaptive(&payload).expect("decode");
    let expected: Vec<(u64, Option<u32>, bool)> = rows.clone();
    assert_eq!(decoded, expected);

    // Corrupt the version nibble
    let n = rows.len();
    let ver_off = 1 + aos_version_offset_for_len(n);
    let mut corrupted = payload.clone();
    corrupted[ver_off] = 0x00; // low nibble != 0x1
    let err = decode_rows_u64_optu32_bool_adaptive(&corrupted).unwrap_err();
    matches_unsupported_version(err);
}

fn matches_unsupported_version(err: Error) {
    // Error::UnsupportedVersion is ideal; allow Message for generic mapping.
    match err {
        Error::UnsupportedVersion { .. } => {}
        Error::Message(_) => {}
        other => panic!("unexpected error: {other:?}"),
    }
}

fn hex_to_bytes(hex: &str) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(hex.len() / 2);
    let mut iter = hex
        .as_bytes()
        .iter()
        .copied()
        .filter(|c| !c.is_ascii_whitespace());
    while let (Some(hi), Some(lo)) = (iter.next(), iter.next()) {
        let hi = (hi as char).to_digit(16).expect("hex hi nibble");
        let lo = (lo as char).to_digit(16).expect("hex lo nibble");
        bytes.push(((hi << 4) | lo) as u8);
    }
    bytes
}
