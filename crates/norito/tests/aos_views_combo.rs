//! AoS view tests for combo shapes: (u64, &str, u32, bool) and (u64, &[u8], u32, bool).

use norito::columnar::{ADAPTIVE_TAG_AOS, view_aos_u64_bytes_u32_bool, view_aos_u64_str_u32_bool};

#[test]
fn aos_view_str_u32_bool() {
    let rows: Vec<(u64, &str, u32, bool)> = vec![(1, "a", 7, true), (2, "bc", 9, false)];
    let body = norito::aos::encode_rows_u64_str_u32_bool(&rows);
    let mut payload = Vec::with_capacity(1 + body.len());
    payload.push(ADAPTIVE_TAG_AOS);
    payload.extend_from_slice(&body);
    let view = view_aos_u64_str_u32_bool(&payload[1..]).expect("view");
    assert_eq!(view.len(), 2);
    assert_eq!(view.id(0), 1);
    assert_eq!(view.name(0).unwrap(), "a");
    assert_eq!(view.val(0), 7);
    assert!(view.flag(0));
    assert_eq!(view.id(1), 2);
    assert_eq!(view.name(1).unwrap(), "bc");
    assert_eq!(view.val(1), 9);
    assert!(!view.flag(1));
}

#[test]
fn aos_view_bytes_u32_bool() {
    let rows: Vec<(u64, &[u8], u32, bool)> =
        vec![(10, b"xx" as &[u8], 7, true), (11, b"", 9, false)];
    let body = norito::aos::encode_rows_u64_bytes_u32_bool(&rows);
    let mut payload = Vec::with_capacity(1 + body.len());
    payload.push(ADAPTIVE_TAG_AOS);
    payload.extend_from_slice(&body);
    let view = view_aos_u64_bytes_u32_bool(&payload[1..]).expect("view");
    assert_eq!(view.len(), 2);
    assert_eq!(view.id(0), 10);
    assert_eq!(view.data(0), b"xx");
    assert_eq!(view.val(0), 7);
    assert!(view.flag(0));
    assert_eq!(view.id(1), 11);
    assert_eq!(view.data(1), b"");
    assert_eq!(view.val(1), 9);
    assert!(!view.flag(1));
}
