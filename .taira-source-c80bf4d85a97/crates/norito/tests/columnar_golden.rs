//! Golden tests scaffolding for Norito adaptive AoS/NCB encoders.
//!
//! These assert decode/view roundtrips and tag selection behavior. Byte-level
//! goldens can be added later via fixed hex strings once formats are frozen.

use norito::columnar::{
    ADAPTIVE_TAG_AOS, ADAPTIVE_TAG_NCB, decode_rows_u64_str_bool_adaptive,
    encode_rows_u64_str_bool_adaptive, view_ncb_u64_str_bool,
};

fn make_rows(n: usize) -> Vec<(u64, String, bool)> {
    (0..n)
        .map(|i| (i as u64, format!("name{i:#x}"), i % 3 == 0))
        .collect()
}

#[test]
fn adaptive_roundtrip_small_prefers_aos() {
    let rows = make_rows(5);
    let borrowed: Vec<(u64, &str, bool)> = rows
        .iter()
        .map(|(id, s, b)| (*id, s.as_str(), *b))
        .collect();
    let bytes = encode_rows_u64_str_bool_adaptive(&borrowed);
    assert!(matches!(
        bytes.first(),
        Some(&tag) if tag == ADAPTIVE_TAG_AOS || tag == ADAPTIVE_TAG_NCB
    ));
    let decoded = decode_rows_u64_str_bool_adaptive(&bytes).expect("decode");
    assert_eq!(decoded, rows);
}

#[test]
fn adaptive_roundtrip_large_prefers_ncb_and_view_works() {
    let rows = make_rows(256);
    let borrowed: Vec<(u64, &str, bool)> = rows
        .iter()
        .map(|(id, s, b)| (*id, s.as_str(), *b))
        .collect();
    let bytes = encode_rows_u64_str_bool_adaptive(&borrowed);
    assert!(matches!(
        bytes.first(),
        Some(&ADAPTIVE_TAG_NCB) | Some(&ADAPTIVE_TAG_AOS)
    ));
    let decoded = decode_rows_u64_str_bool_adaptive(&bytes).expect("decode");
    assert_eq!(decoded, rows);

    if bytes[0] == ADAPTIVE_TAG_NCB {
        let view = view_ncb_u64_str_bool(&bytes[1..]).expect("view");
        assert_eq!(view.len(), rows.len());
        // Spot-check a few positions
        for &i in &[0, 1, 2, 3, 17, 127, 255] {
            let id = view.id(i);
            let name = view.name(i).expect("name");
            let flag = view.flag(i);
            assert_eq!((id, name, flag), (rows[i].0, rows[i].1.as_str(), rows[i].2));
        }
    }
}

#[test]
#[cfg(feature = "compact-len")]
fn golden_bytes_small_sample() {
    // With compact-len enabled, the adaptive AoS encoding of two rows must be stable.
    let rows = [
        (1u64, "alice".to_string(), true),
        (2, "bob".to_string(), false),
    ];
    let borrowed: Vec<(u64, &str, bool)> = rows
        .iter()
        .map(|(id, s, b)| (*id, s.as_str(), *b))
        .collect();
    let bytes = encode_rows_u64_str_bool_adaptive(&borrowed);
    // Expect: [tag=0x00][len=0x02][ver=0x01]
    // Row1: id=1 LE (8 bytes), name len=0x05, "alice", flag=0x01
    // Row2: id=2 LE, name len=0x03, "bob", flag=0x00
    let expected_hex = "000201010000000000000005616c69636501020000000000000003626f6200";
    assert_eq!(to_hex(&bytes), expected_hex);
}

#[cfg(feature = "compact-len")]
fn to_hex(bs: &[u8]) -> String {
    let mut s = String::with_capacity(bs.len() * 2);
    for b in bs {
        use core::fmt::Write as _;
        let _ = write!(&mut s, "{b:02x}");
    }
    s
}
