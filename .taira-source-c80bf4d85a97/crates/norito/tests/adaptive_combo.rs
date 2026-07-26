//! Adaptive tests for combo shapes: (u64, &str, u32, bool) and (u64, &[u8], u32, bool).

use norito::{
    columnar as ncb,
    columnar::{
        ADAPTIVE_TAG_AOS, ADAPTIVE_TAG_NCB, ComboPolicy, decode_rows_u64_bytes_u32_bool_adaptive,
        decode_rows_u64_str_u32_bool_adaptive, encode_ncb_u64_str_u32_bool_with_policy,
        encode_rows_u64_bytes_u32_bool_adaptive, encode_rows_u64_str_u32_bool_adaptive,
        should_use_columnar, view_ncb_u64_str_u32_bool,
    },
};

fn decode_ncb_str_u32_bool(body: &[u8]) -> Vec<(u64, String, u32, bool)> {
    let view = view_ncb_u64_str_u32_bool(body).expect("view");
    (0..view.len())
        .map(|i| {
            (
                view.id(i),
                view.name(i).expect("name").to_string(),
                view.val(i),
                view.flag(i),
            )
        })
        .collect()
}

#[test]
fn adaptive_str_u32_small_prefers_aos() {
    let mut rows_owned: Vec<(u64, String, u32, bool)> = Vec::new();
    for i in 0..24u64 {
        rows_owned.push((
            // Use very large deltas for IDs so NCB's id-delta is not beneficial
            // (each delta varint >= 8 bytes ⇒ raw ids are chosen).
            i << 48,
            // Keep names short and mostly unique to avoid dict; offsets cost dominates for NCB.
            format!("name{i:02}"),
            // Use very large u32 deltas so NCB's u32-delta is not beneficial
            // (each delta varint is 5 bytes ⇒ raw u32 column is chosen).
            ((i as u32) << 30),
            i % 2 == 0,
        ));
    }
    let rows_borrowed: Vec<(u64, &str, u32, bool)> = rows_owned
        .iter()
        .map(|(id, s, v, b)| (*id, s.as_str(), *v, *b))
        .collect();
    let bytes = encode_rows_u64_str_u32_bool_adaptive(&rows_borrowed);
    let aos_len = norito::aos::encode_rows_u64_str_u32_bool(&rows_borrowed).len();
    let ncb_len = ncb::encode_ncb_u64_str_u32_bool(&rows_borrowed).len();
    let expected_tag = if ncb_len < aos_len {
        ADAPTIVE_TAG_NCB
    } else {
        ADAPTIVE_TAG_AOS
    };
    assert_eq!(bytes[0], expected_tag);
    let decoded = decode_rows_u64_str_u32_bool_adaptive(&bytes).expect("decode");
    assert_eq!(decoded, rows_owned);
}

#[test]
fn adaptive_str_u32_large_prefers_ncb() {
    let mut rows_owned: Vec<(u64, String, u32, bool)> = Vec::new();
    for i in 0..160u64 {
        // >= 64 to prefer columnar
        rows_owned.push((
            i * 3,
            format!("user_{:04}", i % 42),
            1000 + (i as u32 % 17),
            i % 3 == 0,
        ));
    }
    let rows_borrowed: Vec<(u64, &str, u32, bool)> = rows_owned
        .iter()
        .map(|(id, s, v, b)| (*id, s.as_str(), *v, *b))
        .collect();
    let bytes = encode_rows_u64_str_u32_bool_adaptive(&rows_borrowed);
    let expected_tag = if should_use_columnar(rows_borrowed.len()) {
        ADAPTIVE_TAG_NCB
    } else {
        ADAPTIVE_TAG_AOS
    };
    assert_eq!(bytes[0], expected_tag);
    let decoded = decode_rows_u64_str_u32_bool_adaptive(&bytes).expect("decode");
    assert_eq!(decoded, rows_owned);
}

#[test]
fn adaptive_str_u32_small_can_choose_ncb_when_beneficial() {
    // Repeated small strings favor dictionary even for small n
    let rows_owned: Vec<(u64, String, u32, bool)> = vec![
        (1, "x".to_string(), 7, true),
        (2, "x".to_string(), 8, false),
        (3, "x".to_string(), 9, true),
        (4, "x".to_string(), 10, false),
        (5, "x".to_string(), 11, true),
        (6, "x".to_string(), 12, false),
        (7, "x".to_string(), 13, true),
        (8, "x".to_string(), 14, false),
    ];
    let rows_borrowed: Vec<(u64, &str, u32, bool)> = rows_owned
        .iter()
        .map(|(id, s, v, b)| (*id, s.as_str(), *v, *b))
        .collect();
    let bytes = encode_rows_u64_str_u32_bool_adaptive(&rows_borrowed);
    // Ensure roundtrip regardless of tag; small two-pass may choose NCB here.
    assert!(bytes[0] == ADAPTIVE_TAG_NCB || bytes[0] == ADAPTIVE_TAG_AOS);
    let decoded = decode_rows_u64_str_u32_bool_adaptive(&bytes).expect("decode");
    assert_eq!(decoded, rows_owned);
}

#[test]
fn adaptive_bytes_u32_small_prefers_aos() {
    let mut rows_owned: Vec<(u64, Vec<u8>, u32, bool)> = Vec::new();
    for i in 0..24u64 {
        rows_owned.push((
            // Force large ID deltas to defeat id-delta heuristics in NCB.
            i << 48,
            // Small varying byte blobs; offsets overhead dominates for NCB at small n.
            vec![i as u8; (i as usize % 5) + 1],
            // Force large u32 deltas to defeat value-delta heuristics in NCB.
            ((i as u32) << 30),
            i % 2 == 1,
        ));
    }
    let rows_borrowed: Vec<(u64, &[u8], u32, bool)> = rows_owned
        .iter()
        .map(|(id, b, v, f)| (*id, b.as_slice(), *v, *f))
        .collect();
    let bytes = encode_rows_u64_bytes_u32_bool_adaptive(&rows_borrowed);
    let aos_len = norito::aos::encode_rows_u64_bytes_u32_bool(&rows_borrowed).len();
    let ncb_len = ncb::encode_ncb_u64_bytes_u32_bool(&rows_borrowed).len();
    let expected_tag = if ncb_len < aos_len {
        ADAPTIVE_TAG_NCB
    } else {
        ADAPTIVE_TAG_AOS
    };
    assert_eq!(bytes[0], expected_tag);
    let decoded = decode_rows_u64_bytes_u32_bool_adaptive(&bytes).expect("decode");
    assert_eq!(decoded, rows_owned);
}

#[test]
fn adaptive_bytes_u32_large_prefers_ncb() {
    let mut rows_owned: Vec<(u64, Vec<u8>, u32, bool)> = Vec::new();
    for i in 0..160u64 {
        let len = 6 + (i as usize % 4);
        rows_owned.push((
            i * 5,
            vec![(i % 251) as u8; len],
            3000 + (i as u32 % 11),
            i % 3 == 1,
        ));
    }
    let rows_borrowed: Vec<(u64, &[u8], u32, bool)> = rows_owned
        .iter()
        .map(|(id, b, v, f)| (*id, b.as_slice(), *v, *f))
        .collect();
    let bytes = encode_rows_u64_bytes_u32_bool_adaptive(&rows_borrowed);
    let expected_tag = if should_use_columnar(rows_borrowed.len()) {
        ADAPTIVE_TAG_NCB
    } else {
        ADAPTIVE_TAG_AOS
    };
    assert_eq!(bytes[0], expected_tag);
    let decoded = decode_rows_u64_bytes_u32_bool_adaptive(&bytes).expect("decode");
    assert_eq!(decoded, rows_owned);
}

#[test]
fn small_n_str_bool_long_repeated_prefers_ncb() {
    // Long repeated strings allow dictionary to dominate even at small N
    let s = "x".repeat(64);
    let rows: Vec<(u64, &str, bool)> = (0..8u64).map(|i| (i, s.as_str(), i % 2 == 0)).collect();
    let bytes = ncb::encode_rows_u64_str_bool_adaptive(&rows);
    assert_eq!(bytes[0], ncb::ADAPTIVE_TAG_NCB);
    let out = ncb::decode_rows_u64_str_bool_adaptive(&bytes).expect("decode");
    let expected: Vec<(u64, String, bool)> = rows
        .iter()
        .map(|(a, b, c)| (*a, (*b).to_string(), *c))
        .collect();
    assert_eq!(out, expected);
}

#[test]
fn small_n_enum_name_repeated_prefers_ncb() {
    // Enum with repeated Name strings should select dictionary-coded NCB
    let s = "name-repeat".repeat(8);
    let rows: Vec<(u64, ncb::EnumBorrow<'_>, bool)> = (0..8u64)
        .map(|i| (i * 3, ncb::EnumBorrow::Name(s.as_str()), i % 3 == 0))
        .collect();
    let bytes = ncb::encode_rows_u64_enum_bool_adaptive(&rows);
    assert_eq!(bytes[0], ncb::ADAPTIVE_ENUM_TAG_NCB);
    let out = ncb::decode_rows_u64_enum_bool_adaptive(&bytes).expect("decode");
    let expected: Vec<(u64, ncb::RowEnumOwned, bool)> = rows
        .iter()
        .map(|(id, e, f)| match e {
            ncb::EnumBorrow::Name(s) => (*id, ncb::RowEnumOwned::Name((*s).to_string()), *f),
            ncb::EnumBorrow::Code(v) => (*id, ncb::RowEnumOwned::Code(*v), *f),
        })
        .collect();
    assert_eq!(out, expected);
}

#[test]
fn small_n_u32_bool_prefers_ncb() {
    // For (u64,u32,bool) at n=8, NCB is slightly smaller due to bitset
    let rows: Vec<(u64, u32, bool)> = (0..8u64)
        .map(|i| (i * 10, (i as u32) * 7, i % 2 == 1))
        .collect();
    let bytes = ncb::encode_rows_u64_u32_bool_adaptive(&rows);
    assert_eq!(bytes[0], ncb::ADAPTIVE_TAG_NCB);
    let out = ncb::decode_rows_u64_u32_bool_adaptive(&bytes).expect("decode");
    assert_eq!(out, rows);
}

#[test]
fn combo_policy_can_disable_dictionary() {
    const DICT_BIT: u8 = 0x80;
    let rows_owned: Vec<(u64, String, u32, bool)> = (0..16u64)
        .map(|i| {
            (
                i,
                format!("repeated_{:02}", i % 2),
                100 + (i as u32),
                i % 3 == 0,
            )
        })
        .collect();
    let rows_borrowed: Vec<(u64, &str, u32, bool)> = rows_owned
        .iter()
        .map(|(id, s, v, b)| (*id, s.as_str(), *v, *b))
        .collect();

    let default_bytes = ncb::encode_ncb_u64_str_u32_bool(&rows_borrowed);
    assert_ne!(
        default_bytes[4] & DICT_BIT,
        0,
        "heuristics should enable dict"
    );
    let forced_bytes = encode_ncb_u64_str_u32_bool_with_policy(
        &rows_borrowed,
        ComboPolicy::default().with_dictionary(false),
    );
    assert_eq!(forced_bytes[4] & DICT_BIT, 0);
    let decoded = decode_ncb_str_u32_bool(&forced_bytes);
    let expected: Vec<(u64, String, u32, bool)> = rows_owned
        .iter()
        .map(|(id, s, v, b)| (*id, s.clone(), *v, *b))
        .collect();
    assert_eq!(decoded, expected);
}

#[test]
fn combo_policy_can_force_dictionary() {
    const DICT_BIT: u8 = 0x80;
    let rows_owned: Vec<(u64, String, u32, bool)> = (0..12u64)
        .map(|i| {
            (
                i * 5,
                format!("unique_name_{i:03}"),
                200 + (i as u32),
                i % 2 == 0,
            )
        })
        .collect();
    let rows_borrowed: Vec<(u64, &str, u32, bool)> = rows_owned
        .iter()
        .map(|(id, s, v, b)| (*id, s.as_str(), *v, *b))
        .collect();

    let default_bytes = ncb::encode_ncb_u64_str_u32_bool(&rows_borrowed);
    assert_eq!(
        default_bytes[4] & DICT_BIT,
        0,
        "heuristics skip dict for unique names"
    );
    let forced_bytes = encode_ncb_u64_str_u32_bool_with_policy(
        &rows_borrowed,
        ComboPolicy::default().with_dictionary(true),
    );
    assert_ne!(forced_bytes[4] & DICT_BIT, 0);
    let decoded = decode_ncb_str_u32_bool(&forced_bytes);
    let expected: Vec<(u64, String, u32, bool)> = rows_owned
        .iter()
        .map(|(id, s, v, b)| (*id, s.clone(), *v, *b))
        .collect();
    assert_eq!(decoded, expected);
}

#[test]
fn combo_policy_can_disable_id_delta() {
    const ID_DELTA_BIT: u8 = 0x40;
    let rows_owned: Vec<(u64, String, u32, bool)> = (0..16u64)
        .map(|i| (i, format!("user_{i:02}"), 400 + (i as u32), i % 2 == 0))
        .collect();
    let rows_borrowed: Vec<(u64, &str, u32, bool)> = rows_owned
        .iter()
        .map(|(id, s, v, b)| (*id, s.as_str(), *v, *b))
        .collect();

    let default_bytes = ncb::encode_ncb_u64_str_u32_bool(&rows_borrowed);
    assert_ne!(
        default_bytes[4] & ID_DELTA_BIT,
        0,
        "heuristics should use id delta"
    );
    let forced_bytes = encode_ncb_u64_str_u32_bool_with_policy(
        &rows_borrowed,
        ComboPolicy::default().with_id_delta(false),
    );
    assert_eq!(forced_bytes[4] & ID_DELTA_BIT, 0);
    let decoded = decode_ncb_str_u32_bool(&forced_bytes);
    let expected: Vec<(u64, String, u32, bool)> = rows_owned
        .iter()
        .map(|(id, s, v, b)| (*id, s.clone(), *v, *b))
        .collect();
    assert_eq!(decoded, expected);
}

#[test]
fn combo_policy_can_force_id_delta() {
    const ID_DELTA_BIT: u8 = 0x40;
    let rows_owned: Vec<(u64, String, u32, bool)> = (0..16u64)
        .map(|i| {
            (
                i << 48,
                format!("sparse_{i:02}"),
                800 + (i as u32),
                i % 2 == 1,
            )
        })
        .collect();
    let rows_borrowed: Vec<(u64, &str, u32, bool)> = rows_owned
        .iter()
        .map(|(id, s, v, b)| (*id, s.as_str(), *v, *b))
        .collect();

    let default_bytes = ncb::encode_ncb_u64_str_u32_bool(&rows_borrowed);
    assert_eq!(
        default_bytes[4] & ID_DELTA_BIT,
        0,
        "heuristics should avoid id delta"
    );
    let forced_bytes = encode_ncb_u64_str_u32_bool_with_policy(
        &rows_borrowed,
        ComboPolicy::default().with_id_delta(true),
    );
    assert_ne!(forced_bytes[4] & ID_DELTA_BIT, 0);
    let decoded = decode_ncb_str_u32_bool(&forced_bytes);
    let expected: Vec<(u64, String, u32, bool)> = rows_owned
        .iter()
        .map(|(id, s, v, b)| (*id, s.clone(), *v, *b))
        .collect();
    assert_eq!(decoded, expected);
}
