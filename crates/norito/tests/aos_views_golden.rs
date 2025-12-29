//! Golden-like tests for AoS ad-hoc bodies and borrowed views.
//! Validates roundtrip and truncation detection for common shapes.

use norito::core::Error;

#[test]
fn aos_view_str_bool_roundtrip() {
    let rows: Vec<(u64, &str, bool)> = vec![(1, "alice", true), (2, "bob", false)];
    let body = norito::aos::encode_rows_u64_str_bool(&rows);
    let view = norito::columnar::view_aos_u64_str_bool(&body).expect("view");
    assert_eq!(view.len(), rows.len());
    for (i, row) in rows.iter().enumerate() {
        assert_eq!(view.id(i), row.0);
        assert_eq!(view.name(i).unwrap(), row.1);
        assert_eq!(view.flag(i), row.2);
    }
}

#[test]
fn aos_view_bytes_u32_bool_roundtrip() {
    let rows: Vec<(u64, &[u8], u32, bool)> = vec![
        (10, b"xyz".as_slice(), 7, true),
        (11, b"".as_slice(), 0, false),
    ];
    let body = norito::aos::encode_rows_u64_bytes_u32_bool(&rows);
    let view = norito::columnar::view_aos_u64_bytes_u32_bool(&body).expect("view");
    assert_eq!(view.len(), rows.len());
    for (i, row) in rows.iter().enumerate() {
        assert_eq!(view.id(i), row.0);
        assert_eq!(view.data(i), row.1);
        assert_eq!(view.val(i), row.2);
        assert_eq!(view.flag(i), row.3);
    }
}

#[test]
fn aos_view_opt_str_bool_roundtrip() {
    let rows: Vec<(u64, Option<&str>, bool)> = vec![(1, Some("aa"), true), (2, None, false)];
    let body = norito::aos::encode_rows_u64_optstr_bool(&rows);
    let view = norito::columnar::view_aos_u64_optstr_bool(&body).expect("view");
    assert_eq!(view.len(), rows.len());
    for (i, row) in rows.iter().enumerate() {
        assert_eq!(view.id(i), row.0);
        let got = view.name(i).unwrap();
        assert_eq!(got, row.1);
        assert_eq!(view.flag(i), row.2);
    }
}

#[test]
fn aos_enum_view_roundtrip() {
    use norito::columnar::AosEnumRef;
    let rows: Vec<(u64, norito::columnar::EnumBorrow<'_>, bool)> = vec![
        (5, norito::columnar::EnumBorrow::Name("aaa"), true),
        (6, norito::columnar::EnumBorrow::Code(42), false),
    ];
    // For enum AoS, use columnar helper which accounts for the minimal header policy for enums.
    let body = norito::aos::encode_rows_u64_enum_bool(&rows);
    let view = norito::columnar::view_aos_u64_enum_bool(&body).expect("view");
    assert_eq!(view.len(), rows.len());
    for (i, row) in rows.iter().enumerate() {
        assert_eq!(view.id(i), row.0);
        match view.payload(i).unwrap() {
            AosEnumRef::Name(s) => {
                assert!(matches!(row.1, norito::columnar::EnumBorrow::Name(_)) && s == "aaa")
            }
            AosEnumRef::Code(v) => {
                assert!(matches!(row.1, norito::columnar::EnumBorrow::Code(_)) && v == 42)
            }
        }
        assert_eq!(view.flag(i), row.2);
    }
}

#[test]
fn aos_view_truncation_detected() {
    let rows: Vec<(u64, &str, bool)> = vec![(1, "alice", true), (2, "bob", false)];
    let mut body = norito::aos::encode_rows_u64_str_bool(&rows);
    // Truncate the last byte; view parse should fail with LengthMismatch
    body.pop();
    let res = norito::columnar::view_aos_u64_str_bool(&body);
    assert!(res.is_err());
    if let Err(e) = res {
        assert!(matches!(e, Error::LengthMismatch));
    }
}

#[test]
fn aos_bytes_view_truncation_detected() {
    // Two rows bytes body; then truncate
    let rows: Vec<(u64, &[u8], bool)> =
        vec![(1, b"ab".as_slice(), true), (2, b"".as_slice(), false)];
    let mut body = norito::aos::encode_rows_u64_bytes_bool(&rows);
    body.pop();
    let res = norito::columnar::view_aos_u64_bytes_bool(&body);
    assert!(res.is_err());
    if let Err(e) = res {
        assert!(matches!(e, Error::LengthMismatch));
    }
}

#[test]
fn aos_view_optstr_truncation_detected() {
    let rows: Vec<(u64, Option<&str>, bool)> = vec![(1, Some("aa"), true), (2, None, false)];
    let mut body = norito::aos::encode_rows_u64_optstr_bool(&rows);
    // Truncate the last byte; view parse should fail with LengthMismatch
    body.pop();
    let res = norito::columnar::view_aos_u64_optstr_bool(&body);
    assert!(res.is_err());
    if let Err(e) = res {
        assert!(matches!(e, Error::LengthMismatch));
    }
}

#[test]
fn aos_enum_view_truncation_detected() {
    use norito::columnar::EnumBorrow;
    let rows: Vec<(u64, norito::columnar::EnumBorrow<'_>, bool)> = vec![
        (5, EnumBorrow::Name("aaa"), true),
        (6, EnumBorrow::Code(42), false),
    ];
    let mut body = norito::aos::encode_rows_u64_enum_bool(&rows);
    // Truncate a byte; enum AoS view must detect mismatch as well
    body.pop();
    let res = norito::columnar::view_aos_u64_enum_bool(&body);
    assert!(res.is_err());
    if let Err(e) = res {
        assert!(matches!(e, Error::LengthMismatch));
    }
}

#[test]
fn aos_enum_view_invalid_tag_detected() {
    use norito::columnar::EnumBorrow;
    // Start with a valid single-row AoS enum body with a Code payload
    let rows: Vec<(u64, norito::columnar::EnumBorrow<'_>, bool)> =
        vec![(1, EnumBorrow::Code(7), true)];
    let mut body = norito::aos::encode_rows_u64_enum_bool(&rows);
    // Locate the tag byte and set it to an invalid value (2)
    let mut off = 0usize;
    #[cfg(feature = "compact-len")]
    let (_n, used) = norito::core::read_len_from_slice(&body[off..]).expect("len");
    #[cfg(not(feature = "compact-len"))]
    let (_n, used) = {
        assert!(body.len() >= 8);
        (1usize, 8usize)
    };
    off += used; // after [len]
    off += 8; // id
    assert!(off < body.len());
    body[off] = 2; // invalid tag
    // View must reject with InvalidTag
    let res = norito::columnar::view_aos_u64_enum_bool(&body);
    assert!(res.is_err());
    if let Err(e) = res {
        assert!(matches!(e, Error::InvalidTag { .. }));
    }
}

#[test]
fn aos_enum_view_invalid_utf8_detected() {
    use norito::columnar::EnumBorrow;
    // Start with a valid single-row AoS enum body with a Name payload
    let rows: Vec<(u64, norito::columnar::EnumBorrow<'_>, bool)> =
        vec![(1, EnumBorrow::Name("n"), true)];
    let mut body = norito::aos::encode_rows_u64_enum_bool(&rows);

    // Walk the AoS enum body to locate the string byte and corrupt it
    let mut off = 0usize;
    #[cfg(feature = "compact-len")]
    let (_n, used) = norito::core::read_len_from_slice(&body[off..]).expect("len");
    #[cfg(not(feature = "compact-len"))]
    let (_n, used) = {
        assert!(body.len() >= 8);
        (1usize, 8usize)
    };
    off += used; // after [len]
    assert!(off + 8 < body.len());
    off += 8; // id
    assert_eq!(body[off], 0u8, "expected NAME tag");
    off += 1; // tag
    #[cfg(feature = "compact-len")]
    let (slen, used) = norito::core::read_len_from_slice(&body[off..]).expect("slen");
    #[cfg(not(feature = "compact-len"))]
    let (slen, used) = {
        assert!(off + 8 <= body.len());
        let mut lb = [0u8; 8];
        lb.copy_from_slice(&body[off..off + 8]);
        (u64::from_le_bytes(lb) as usize, 8usize)
    };
    off += used; // start of string bytes
    assert!(slen >= 1 && off + slen <= body.len());
    // Corrupt first byte to invalid UTF-8 (0xFF)
    body[off] = 0xff;

    // Build view and ensure payload() returns InvalidUtf8
    let view = norito::columnar::view_aos_u64_enum_bool(&body).expect("view builds");
    let res = view.payload(0);
    assert!(res.is_err());
    if let Err(e) = res {
        assert!(matches!(e, Error::InvalidUtf8));
    }
}

#[test]
fn aos_view_str_invalid_utf8_detected() {
    // Single row: (1, "a", true)
    let rows: Vec<(u64, &str, bool)> = vec![(1, "a", true)];
    let mut body = norito::aos::encode_rows_u64_str_bool(&rows);
    // Locate start of string bytes and corrupt first byte
    let mut off = 0usize;
    #[cfg(feature = "compact-len")]
    let (_n, used) = norito::core::read_len_from_slice(&body[off..]).expect("len");
    #[cfg(not(feature = "compact-len"))]
    let (_n, used) = (1usize, 8usize);
    off += used; // after [len]
    off += 1; // version byte
    off += 8; // id
    #[cfg(feature = "compact-len")]
    let (_slen, used) = norito::core::read_len_from_slice(&body[off..]).expect("slen");
    #[cfg(not(feature = "compact-len"))]
    let (_slen, used) = (1usize, 8usize);
    off += used; // start of str bytes
    assert!(off < body.len());
    body[off] = 0xff; // invalidate utf8
    let view = norito::columnar::view_aos_u64_str_bool(&body).expect("view builds");
    let res = view.name(0);
    assert!(res.is_err());
    if let Err(e) = res {
        assert!(matches!(e, Error::InvalidUtf8));
    }
}

#[test]
fn aos_view_str_u32_invalid_utf8_detected() {
    // Single row: (1, "a", 3, true)
    let rows: Vec<(u64, &str, u32, bool)> = vec![(1, "a", 3, true)];
    let mut body = norito::aos::encode_rows_u64_str_u32_bool(&rows);
    // Locate start of string bytes and corrupt first byte
    let mut off = 0usize;
    #[cfg(feature = "compact-len")]
    let (_n, used) = norito::core::read_len_from_slice(&body[off..]).expect("len");
    #[cfg(not(feature = "compact-len"))]
    let (_n, used) = (1usize, 8usize);
    off += used; // after [len]
    off += 1; // version byte
    off += 8; // id
    #[cfg(feature = "compact-len")]
    let (_slen, used) = norito::core::read_len_from_slice(&body[off..]).expect("slen");
    #[cfg(not(feature = "compact-len"))]
    let (_slen, used) = (1usize, 8usize);
    off += used; // start of str bytes
    assert!(off < body.len());
    body[off] = 0xff; // invalidate utf8
    // Use the correct AoS str_u32 view helper
    let view = norito::columnar::view_aos_u64_str_u32_bool(&body).expect("view str-u32 ok");
    let res = view.name(0);
    assert!(res.is_err());
    if let Err(e) = res {
        assert!(matches!(e, Error::InvalidUtf8));
    }
}

#[test]
fn aos_optstr_view_invalid_utf8_detected() {
    // Start with a valid single-row AoS opt-str body with Some("a")
    let rows: Vec<(u64, Option<&str>, bool)> = vec![(1, Some("a"), true)];
    let mut body = norito::aos::encode_rows_u64_optstr_bool(&rows);

    // Walk the AoS opt-str body to locate the string byte and corrupt it
    let mut off = 0usize;
    #[cfg(feature = "compact-len")]
    let (_n, used) = norito::core::read_len_from_slice(&body[off..]).expect("len");
    #[cfg(not(feature = "compact-len"))]
    let (_n, used) = {
        assert!(body.len() >= 8);
        (1usize, 8usize)
    };
    off += used; // after [len]
    // Skip version nibble for opt-str AoS
    assert!(off < body.len());
    off += 1;
    // Row fields
    assert!(off + 8 < body.len());
    off += 8; // id
    assert_eq!(body[off], 1u8, "expected present=Some tag");
    off += 1; // tag
    #[cfg(feature = "compact-len")]
    let (slen, used) = norito::core::read_len_from_slice(&body[off..]).expect("slen");
    #[cfg(not(feature = "compact-len"))]
    let (slen, used) = {
        assert!(off + 8 <= body.len());
        let mut lb = [0u8; 8];
        lb.copy_from_slice(&body[off..off + 8]);
        (u64::from_le_bytes(lb) as usize, 8usize)
    };
    off += used; // start of string bytes
    assert!(slen >= 1 && off + slen <= body.len());
    // Corrupt first byte to invalid UTF-8 (0xFF)
    body[off] = 0xff;

    // Build view and ensure name(0) returns InvalidUtf8
    let view = norito::columnar::view_aos_u64_optstr_bool(&body).expect("view builds");
    let res = view.name(0);
    assert!(res.is_err());
    if let Err(e) = res {
        assert!(matches!(e, Error::InvalidUtf8));
    }
}

#[test]
fn aos_view_str_u32_truncation_detected() {
    use norito::core::Error;
    let rows: Vec<(u64, &str, u32, bool)> = vec![(1, "a", 3, true), (2, "", 0, false)];
    let mut body = norito::aos::encode_rows_u64_str_u32_bool(&rows);
    body.pop();
    let res = norito::columnar::view_aos_u64_str_u32_bool(&body);
    assert!(res.is_err());
    if let Err(e) = res {
        assert!(matches!(e, Error::LengthMismatch));
    }
}

#[test]
fn aos_view_bytes_u32_truncation_detected() {
    use norito::core::Error;
    let rows: Vec<(u64, &[u8], u32, bool)> = vec![
        (1, b"xy".as_slice(), 3, true),
        (2, b"".as_slice(), 0, false),
    ];
    let mut body = norito::aos::encode_rows_u64_bytes_u32_bool(&rows);
    body.pop();
    let res = norito::columnar::view_aos_u64_bytes_u32_bool(&body);
    assert!(res.is_err());
    if let Err(e) = res {
        assert!(matches!(e, Error::LengthMismatch));
    }
}

#[test]
fn aos_optu32_view_truncation_detected() {
    // Build a valid body for rows: (10, Some(7), true), (11, None, false)
    let rows: Vec<(u64, Option<u32>, bool)> = vec![(10, Some(7), true), (11, None, false)];
    let mut body = norito::aos::encode_rows_u64_optu32_bool(&rows);
    // Truncate the last byte; view should fail with LengthMismatch
    body.pop();
    let res = norito::columnar::view_aos_u64_optu32_bool(&body);
    assert!(res.is_err());
    if let Err(e) = res {
        assert!(matches!(e, Error::LengthMismatch));
    }
}
