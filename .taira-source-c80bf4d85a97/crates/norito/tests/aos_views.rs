use norito::columnar::{AosEnumRef, *};

#[test]
fn aos_view_str_bool() {
    let rows: Vec<(u64, &str, bool)> = vec![(1, "a", true), (2, "bc", false)];
    let body = norito::aos::encode_rows_u64_str_bool(&rows);
    let mut payload = Vec::with_capacity(1 + body.len());
    payload.push(ADAPTIVE_TAG_AOS);
    payload.extend_from_slice(&body);
    let view = view_aos_u64_str_bool(&payload[1..]).expect("view");
    assert_eq!(view.len(), 2);
    assert_eq!(view.id(0), 1);
    assert_eq!(view.name(0).unwrap(), "a");
    assert!(view.flag(0));
    assert_eq!(view.id(1), 2);
    assert_eq!(view.name(1).unwrap(), "bc");
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

#[test]
fn aos_view_enum_bool() {
    let rows: Vec<(u64, EnumBorrow<'_>, bool)> = vec![
        (1, EnumBorrow::Name("alice"), true),
        (2, EnumBorrow::Code(42), false),
    ];
    let body = norito::aos::encode_rows_u64_enum_bool(&rows);
    let mut payload = Vec::with_capacity(1 + body.len());
    payload.push(ADAPTIVE_ENUM_TAG_AOS);
    payload.extend_from_slice(&body);
    let view = view_aos_u64_enum_bool(&payload[1..]).expect("view");
    assert_eq!(view.len(), 2);
    assert_eq!(view.id(0), 1);
    match view.payload(0).unwrap() {
        AosEnumRef::Name(s) => assert_eq!(s, "alice"),
        _ => panic!("expected name"),
    }
    assert!(view.flag(0));
    assert_eq!(view.id(1), 2);
    match view.payload(1).unwrap() {
        AosEnumRef::Code(v) => assert_eq!(v, 42),
        _ => panic!("expected code"),
    }
    assert!(!view.flag(1));
}

#[test]
fn aos_view_opt_str_bool() {
    let rows: Vec<(u64, Option<&str>, bool)> = vec![(1, Some("x"), true), (2, None, false)];
    let body = norito::aos::encode_rows_u64_optstr_bool(&rows);
    let mut payload = Vec::with_capacity(1 + body.len());
    payload.push(ADAPTIVE_TAG_AOS);
    payload.extend_from_slice(&body);
    let view = view_aos_u64_optstr_bool(&payload[1..]).expect("view");
    assert_eq!(view.len(), 2);
    assert_eq!(view.id(0), 1);
    assert_eq!(view.name(0).unwrap(), Some("x"));
    assert!(view.flag(0));
    assert_eq!(view.id(1), 2);
    assert_eq!(view.name(1).unwrap(), None);
    assert!(!view.flag(1));
}

#[test]
fn aos_view_opt_u32_bool() {
    let rows: Vec<(u64, Option<u32>, bool)> = vec![(10, Some(7), true), (11, None, false)];
    let body = norito::aos::encode_rows_u64_optu32_bool(&rows);
    let mut payload = Vec::with_capacity(1 + body.len());
    payload.push(ADAPTIVE_TAG_AOS);
    payload.extend_from_slice(&body);
    let view = view_aos_u64_optu32_bool(&payload[1..]).expect("view");
    assert_eq!(view.len(), 2);
    assert_eq!(view.id(0), 10);
    assert_eq!(view.val(0), Some(7));
    assert!(view.flag(0));
    assert_eq!(view.id(1), 11);
    assert_eq!(view.val(1), None);
    assert!(!view.flag(1));
}
