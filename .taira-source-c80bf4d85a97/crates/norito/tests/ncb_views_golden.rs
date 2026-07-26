//! Golden-like tests for NCB (columnar) borrowed views.
//! Validates roundtrip and error paths for several shapes.

use norito::core::Error;

#[test]
fn ncb_view_str_bool_roundtrip() {
    let rows: Vec<(u64, &str, bool)> = vec![(1, "alice", true), (2, "bob", false)];
    let body = norito::columnar::encode_ncb_u64_str_bool(&rows);
    let view = norito::columnar::view_ncb_u64_str_bool(&body).expect("view");
    assert_eq!(view.len(), rows.len());
    for (i, row) in rows.iter().enumerate() {
        assert_eq!(view.id(i), row.0);
        assert_eq!(view.name(i).unwrap(), row.1);
        assert_eq!(view.flag(i), row.2);
    }
}

#[test]
fn ncb_view_bytes_bool_roundtrip() {
    let rows: Vec<(u64, &[u8], bool)> =
        vec![(10, b"xyz".as_slice(), true), (11, b"".as_slice(), false)];
    let body = norito::columnar::encode_ncb_u64_bytes_bool(&rows);
    let view = norito::columnar::view_ncb_u64_bytes_bool(&body).expect("view");
    assert_eq!(view.len(), rows.len());
    for (i, row) in rows.iter().enumerate() {
        assert_eq!(view.id(i), row.0);
        assert_eq!(view.data(i), row.1);
        assert_eq!(view.flag(i), row.2);
    }
}

#[test]
fn ncb_view_u32_bool_roundtrip() {
    let rows: Vec<(u64, u32, bool)> = vec![(1, 7, true), (5, 9, false), (9, 12, true)];
    // Exercise the delta variants; cover base and deltas
    for &(id_delta, val_delta) in &[(false, false), (true, false), (false, true), (true, true)] {
        let body = norito::columnar::encode_ncb_u64_u32_bool(&rows, id_delta, val_delta);
        let view = norito::columnar::view_ncb_u64_u32_bool(&body).expect("view");
        assert_eq!(view.len(), rows.len());
        for (i, row) in rows.iter().enumerate() {
            assert_eq!(view.id(i), row.0);
            assert_eq!(view.val(i), row.1);
            assert_eq!(view.flag(i), row.2);
        }
    }
}

#[test]
fn ncb_view_str_u32_bool_roundtrip() {
    let rows: Vec<(u64, &str, u32, bool)> = vec![(1, "aa", 3, true), (2, "bbb", 0, false)];
    let body = norito::columnar::encode_ncb_u64_str_u32_bool(&rows);
    let view = norito::columnar::view_ncb_u64_str_u32_bool(&body).expect("view");
    assert_eq!(view.len(), rows.len());
    for (i, row) in rows.iter().enumerate() {
        assert_eq!(view.id(i), row.0);
        assert_eq!(view.name(i).unwrap(), row.1);
        assert_eq!(view.val(i), row.2);
        assert_eq!(view.flag(i), row.3);
    }
}

#[test]
fn ncb_view_opt_str_bool_roundtrip() {
    let rows: Vec<(u64, Option<&str>, bool)> = vec![(1, Some("aa"), true), (2, None, false)];
    let body = norito::columnar::encode_ncb_u64_optstr_bool(&rows);
    let view = norito::columnar::view_ncb_u64_optstr_bool(&body).expect("view");
    assert_eq!(view.len(), rows.len());
    for (i, row) in rows.iter().enumerate() {
        assert_eq!(view.id(i), row.0);
        let got = view.name(i).unwrap();
        assert_eq!(got, row.1);
        assert_eq!(view.flag(i), row.2);
    }
}

#[test]
fn ncb_view_opt_u32_bool_roundtrip() {
    let rows: Vec<(u64, Option<u32>, bool)> = vec![(1, Some(7), true), (2, None, false)];
    let body = norito::columnar::encode_ncb_u64_optu32_bool(&rows);
    let view = norito::columnar::view_ncb_u64_optu32_bool(&body).expect("view");
    assert_eq!(view.len(), rows.len());
    for (i, row) in rows.iter().enumerate() {
        assert_eq!(view.id(i), row.0);
        assert_eq!(view.val(i), row.1);
        assert_eq!(view.flag(i), row.2);
    }
}

#[test]
fn ncb_view_enum_roundtrip() {
    use norito::columnar::{
        ColEnumRef, EnumBorrow, encode_ncb_u64_enum_bool, view_ncb_u64_enum_bool,
    };
    let rows = vec![
        (5u64, EnumBorrow::Name("aaa"), true),
        (6u64, EnumBorrow::Code(42), false),
    ];
    // Cover a few descriptor variants by toggling deltas/dict flags.
    for &(id_delta, name_dict, code_delta) in &[
        (false, false, false),
        (true, false, false),
        (false, true, false),
        (false, false, true),
    ] {
        let body = encode_ncb_u64_enum_bool(&rows, id_delta, name_dict, code_delta);
        let view = view_ncb_u64_enum_bool(&body).expect("view");
        assert_eq!(view.len(), rows.len());
        for (i, row) in rows.iter().enumerate() {
            assert_eq!(view.id(i), row.0);
            match view.payload(i).unwrap() {
                ColEnumRef::Name(s2) => {
                    if let EnumBorrow::Name(s) = row.1 {
                        assert_eq!(s, s2)
                    } else {
                        panic!("variant mismatch")
                    }
                }
                ColEnumRef::Code(v2) => {
                    if let EnumBorrow::Code(v) = row.1 {
                        assert_eq!(v, v2)
                    } else {
                        panic!("variant mismatch")
                    }
                }
            }
            assert_eq!(view.flag(i), row.2);
        }
    }
}

#[test]
fn ncb_view_str_bool_invalid_utf8_detected() {
    // Build a valid buffer then corrupt a byte inside the string blob.
    let rows: Vec<(u64, &str, bool)> = vec![(1, "alice", true), (2, "bob", false)];
    let mut body = norito::columnar::encode_ncb_u64_str_bool(&rows);
    // Locate the start of names blob similar to view parser.
    let n = u32::from_le_bytes(body[0..4].try_into().unwrap()) as usize;
    let mut off = 5usize; // header
    let mis = off & 7;
    if mis != 0 {
        off += 8 - mis;
    }
    // ids slice length depends on descriptor
    let desc = body[4];
    if desc == 0x53 {
        // DESC_U64_DELTA_STR_BOOL
        // skip base u64 and varints roughly; for simplicity, find after ids by scanning varints
        off += 8; // base
        for _ in 1..n {
            // safe: bounds checked by get in view; we keep it simple by incrementing over LEB128
            let mut p = off;
            let mut cnt = 0;
            while p < body.len() {
                let b = body[p];
                p += 1;
                cnt += 1;
                if (b & 0x80) == 0 {
                    break;
                }
            }
            off += cnt;
        }
    } else {
        off += 8 * n; // ids slice
    }
    // Align to 4 for names offsets and then skip offsets to reach blob
    let mis4 = off & 3;
    if mis4 != 0 {
        off += 4 - mis4;
    }
    off += 4 * (n + 1);
    if off < body.len() {
        // Corrupt one byte in the names blob
        body[off] = 0xFF;
    }
    let res = norito::columnar::view_ncb_u64_str_bool(&body);
    assert!(res.is_err());
    if let Err(e) = res {
        assert!(matches!(e, Error::InvalidUtf8 | Error::LengthMismatch));
    }
}
