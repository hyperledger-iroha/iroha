//! NCB view truncation detection for combo shapes

use norito::core::Error;

#[test]
fn ncb_view_str_u32_truncation_detected() {
    let rows: Vec<(u64, &str, u32, bool)> = vec![(1, "a", 3, true), (2, "", 0, false)];
    let mut body = norito::columnar::encode_ncb_u64_str_u32_bool(&rows);
    body.pop();
    let res = norito::columnar::view_ncb_u64_str_u32_bool(&body);
    assert!(res.is_err());
    if let Err(e) = res {
        assert!(matches!(e, Error::LengthMismatch));
    }
}

#[test]
fn ncb_view_bytes_u32_truncation_detected() {
    let rows: Vec<(u64, &[u8], u32, bool)> = vec![
        (1, b"xy".as_slice(), 7, true),
        (2, b"".as_slice(), 0, false),
    ];
    let mut body = norito::columnar::encode_ncb_u64_bytes_u32_bool(&rows);
    body.pop();
    let res = norito::columnar::view_ncb_u64_bytes_u32_bool(&body);
    assert!(res.is_err());
    if let Err(e) = res {
        assert!(matches!(e, Error::LengthMismatch));
    }
}
