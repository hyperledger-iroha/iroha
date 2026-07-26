//! Tests for NCB enum layout and optional string column utilities.
#![allow(clippy::needless_range_loop)]

use norito::columnar as ncb;

#[test]
fn ncb_enum_roundtrip() {
    // Build a small mixed dataset
    let rows_aos: Vec<(u64, String, u32, bool)> = vec![
        (1, "alice".into(), 0, true),
        (2, "".into(), 42, false),
        (3, "carol".into(), 0, true),
        (4, "".into(), 7, false),
        (5, "dave".into(), 0, true),
    ];
    // Borrow as enum payload
    let rows_borrowed: Vec<(u64, ncb::EnumBorrow<'_>, bool)> = rows_aos
        .iter()
        .map(|(id, name, code, flag)| {
            if *code == 0 {
                (*id, ncb::EnumBorrow::Name(name.as_str()), *flag)
            } else {
                (*id, ncb::EnumBorrow::Code(*code), *flag)
            }
        })
        .collect();

    let bytes = ncb::encode_ncb_u64_enum_bool(
        &rows_borrowed,
        /*id-delta*/ false,
        /*dict*/ false,
        /*code-delta*/ false,
    );
    let view = ncb::view_ncb_u64_enum_bool(&bytes).expect("view");
    assert_eq!(view.len(), rows_borrowed.len());

    // Validate ids, tags, payloads, flags
    for i in 0..view.len() {
        assert_eq!(view.id(i), rows_borrowed[i].0);
        match (&rows_borrowed[i].1, view.payload(i).unwrap()) {
            (ncb::EnumBorrow::Name(exp), ncb::ColEnumRef::Name(got)) => assert_eq!(got, *exp),
            (ncb::EnumBorrow::Code(exp), ncb::ColEnumRef::Code(got)) => assert_eq!(got, *exp),
            _ => panic!("tag mismatch"),
        }
        assert_eq!(view.flag(i), rows_borrowed[i].2);
    }
}

#[test]
fn ncb_enum_indexed_vs_fast_iter_match() {
    // Build a mixed dataset
    let rows: Vec<(u64, ncb::EnumBorrow<'_>, bool)> = (0..1024u64)
        .map(|i| {
            let en = if i % 3 == 0 {
                ncb::EnumBorrow::Name("alice")
            } else if i % 3 == 1 {
                ncb::EnumBorrow::Name("bob")
            } else {
                ncb::EnumBorrow::Code((i % 100) as u32)
            };
            let flag = i % 5 == 0 || i % 7 == 0;
            (i * 10 + 1, en, flag)
        })
        .collect();
    let bytes = ncb::encode_ncb_u64_enum_bool(&rows, false, false, false);
    let view = ncb::view_ncb_u64_enum_bool(&bytes).expect("view");

    // Collect names where flag==true via fast popcount+tag check
    let mut a: Vec<&str> = view.iter_names_flag_true_fast().collect();
    // Collect names via indexed intersection
    let mut b: Vec<&str> = view.iter_names_flag_true_indexed().collect();
    a.sort_unstable();
    b.sort_unstable();
    assert_eq!(a, b);
}

#[test]
fn ncb_enum_codes_indexed_vs_fast_iter_match() {
    let rows: Vec<(u64, ncb::EnumBorrow<'_>, bool)> = (0..2048u64)
        .map(|i| {
            let en = if i % 2 == 0 {
                ncb::EnumBorrow::Code((i % 100) as u32)
            } else {
                ncb::EnumBorrow::Name(if i % 3 == 0 { "alice" } else { "bob" })
            };
            let flag = i % 3 == 0 || i % 11 == 0;
            (i * 5 + 7, en, flag)
        })
        .collect();
    let bytes = ncb::encode_ncb_u64_enum_bool(&rows, false, false, false);
    let view = ncb::view_ncb_u64_enum_bool(&bytes).expect("view");

    let mut a: Vec<u32> = view.iter_codes_flag_true_fast().collect();
    let mut b: Vec<u32> = view.iter_codes_flag_true_indexed().collect();
    a.sort_unstable();
    b.sort_unstable();
    assert_eq!(a, b);
}

#[test]
fn ncb_enum_delta_ids() {
    let mut rows = Vec::new();
    for i in 0..32u64 {
        rows.push((
            i * 10,
            if i % 2 == 0 {
                ncb::EnumBorrow::Name("x")
            } else {
                ncb::EnumBorrow::Code(i as u32)
            },
            i % 3 == 0,
        ));
    }
    let bytes = ncb::encode_ncb_u64_enum_bool(&rows, true, false, false);
    let view = ncb::view_ncb_u64_enum_bool(&bytes).unwrap();
    for i in 0..view.len() {
        assert_eq!(view.id(i), rows[i].0);
    }
}

#[test]
fn ncb_enum_codes_delta() {
    let mut rows = Vec::new();
    for i in 0..50u64 {
        let payload = if i % 3 == 0 {
            ncb::EnumBorrow::Code((i * 2) as u32)
        } else {
            ncb::EnumBorrow::Name("zz")
        };
        rows.push((i * 5, payload, i % 2 == 0));
    }
    let bytes = ncb::encode_ncb_u64_enum_bool(
        &rows, /*id-delta*/ false, /*dict*/ false, /*code-delta*/ true,
    );
    let view = ncb::view_ncb_u64_enum_bool(&bytes).unwrap();
    for i in 0..view.len() {
        match rows[i].1 {
            ncb::EnumBorrow::Code(v) => match view.payload(i).unwrap() {
                ncb::ColEnumRef::Code(g) => assert_eq!(g, v),
                _ => panic!("tag"),
            },
            ncb::EnumBorrow::Name(s) => match view.payload(i).unwrap() {
                ncb::ColEnumRef::Name(g) => assert_eq!(g, s),
                _ => panic!("tag"),
            },
        }
    }
}

#[test]
fn ncb_enum_dict_names() {
    // Build repeated names to exercise dictionary
    let mut rows = Vec::new();
    let names = ["alice", "bob", "alice", "carol", "bob", "alice"];
    for (i, name) in names.iter().enumerate() {
        let payload = if i % 2 == 0 {
            ncb::EnumBorrow::Name(name)
        } else {
            ncb::EnumBorrow::Code((i * 3) as u32)
        };
        rows.push(((i as u64) * 7, payload, i % 2 == 0));
    }
    let bytes = ncb::encode_ncb_u64_enum_bool(
        &rows, /*id-delta*/ false, /*dict*/ true, /*code-delta*/ false,
    );
    let view = ncb::view_ncb_u64_enum_bool(&bytes).unwrap();
    for i in 0..view.len() {
        match rows[i].1 {
            ncb::EnumBorrow::Name(s) => match view.payload(i).unwrap() {
                ncb::ColEnumRef::Name(g) => assert_eq!(g, s),
                _ => panic!("tag"),
            },
            ncb::EnumBorrow::Code(v) => match view.payload(i).unwrap() {
                ncb::ColEnumRef::Code(g) => assert_eq!(g, v),
                _ => panic!("tag"),
            },
        }
    }
}

#[test]
fn ncb_enum_dict_codes_out_of_range_rejected() {
    let rows = vec![
        (1u64, ncb::EnumBorrow::Name("alpha"), true),
        (2u64, ncb::EnumBorrow::Name("beta"), false),
    ];
    let mut bytes = ncb::encode_ncb_u64_enum_bool(
        &rows, /*id-delta*/ false, /*dict*/ true, /*code-delta*/ false,
    );
    let n = u32::from_le_bytes(bytes[0..4].try_into().unwrap()) as usize;
    assert_eq!(n, rows.len());
    let mut off = 5usize;
    let mis = off & 7;
    if mis != 0 {
        off += 8 - mis;
    }
    off += 8 * n; // ids
    off += n; // tags
    let mis4 = off & 3;
    if mis4 != 0 {
        off += 4 - mis4;
    }
    let dict_len = u32::from_le_bytes(bytes[off..off + 4].try_into().unwrap()) as usize;
    off += 4;
    let dict_offs_len = 4 * (dict_len + 1);
    off += dict_offs_len;
    let dict_last = u32::from_le_bytes(bytes[off - 4..off].try_into().unwrap()) as usize;
    off += dict_last;
    let mis4_codes = off & 3;
    if mis4_codes != 0 {
        off += 4 - mis4_codes;
    }
    bytes[off..off + 4].copy_from_slice(&(dict_len as u32).to_le_bytes());
    let res = ncb::view_ncb_u64_enum_bool(&bytes);
    assert!(res.is_err());
}

#[test]
fn ncb_enum_dict_names_codes_delta() {
    // Mix Name(Code by dict) and Code(u32) with code delta enabled
    let mut rows = Vec::new();
    for i in 0..40u32 {
        let payload = if i % 3 == 0 {
            ncb::EnumBorrow::Name("alpha")
        } else {
            ncb::EnumBorrow::Code(i * 7)
        };
        rows.push(((i as u64) * 9, payload, i % 2 == 0));
    }
    let bytes = ncb::encode_ncb_u64_enum_bool(
        &rows, /*id-delta*/ false, /*dict*/ true, /*code-delta*/ true,
    );
    let view = ncb::view_ncb_u64_enum_bool(&bytes).unwrap();
    for i in 0..view.len() {
        match rows[i].1 {
            ncb::EnumBorrow::Name(s) => match view.payload(i).unwrap() {
                ncb::ColEnumRef::Name(g) => assert_eq!(g, s),
                _ => panic!("tag"),
            },
            ncb::EnumBorrow::Code(v) => match view.payload(i).unwrap() {
                ncb::ColEnumRef::Code(g) => assert_eq!(g, v),
                _ => panic!("tag"),
            },
        }
    }
}

#[test]
fn ncb_enum_dense_iters_match() {
    // Mixed dataset
    let mut rows = Vec::new();
    for i in 0..30u32 {
        let payload = match i % 4 {
            0 | 1 => ncb::EnumBorrow::Name("x"),
            2 => ncb::EnumBorrow::Name("yy"),
            _ => ncb::EnumBorrow::Code(i),
        };
        rows.push(((i as u64) * 3, payload, i % 2 == 0));
    }
    // offsets
    let bytes = ncb::encode_ncb_u64_enum_bool(&rows, false, false, false);
    let view = ncb::view_ncb_u64_enum_bool(&bytes).unwrap();
    let dense_names: Vec<&str> = view.iter_names_dense().collect();
    let row_names: Vec<&str> = (0..view.len())
        .filter_map(|i| match view.payload(i).unwrap() {
            ncb::ColEnumRef::Name(s) => Some(s),
            _ => None,
        })
        .collect();
    assert_eq!(dense_names, row_names);
    let dense_codes: Vec<u32> = view.iter_codes_dense().collect();
    let row_codes: Vec<u32> = (0..view.len())
        .filter_map(|i| match view.payload(i).unwrap() {
            ncb::ColEnumRef::Code(v) => Some(v),
            _ => None,
        })
        .collect();
    assert_eq!(dense_codes, row_codes);
    // dict + code-delta
    let bytes2 = ncb::encode_ncb_u64_enum_bool(&rows, false, true, true);
    let view2 = ncb::view_ncb_u64_enum_bool(&bytes2).unwrap();
    let dense_names2: Vec<&str> = view2.iter_names_dense().collect();
    let row_names2: Vec<&str> = (0..view2.len())
        .filter_map(|i| match view2.payload(i).unwrap() {
            ncb::ColEnumRef::Name(s) => Some(s),
            _ => None,
        })
        .collect();
    assert_eq!(dense_names2, row_names2);
    let dense_codes2: Vec<u32> = view2.iter_codes_dense().collect();
    let row_codes2: Vec<u32> = (0..view2.len())
        .filter_map(|i| match view2.payload(i).unwrap() {
            ncb::ColEnumRef::Code(v) => Some(v),
            _ => None,
        })
        .collect();
    assert_eq!(dense_codes2, row_codes2);
}

#[test]
fn opt_str_column_basic() {
    let data = vec![Some("a"), None, Some("bb"), Some("ccc"), None];
    let (bytes, present) = ncb::encode_opt_str_column(&data);
    assert_eq!(present, 3);
    let view = ncb::view_opt_str_column(&bytes, data.len()).expect("view");
    assert_eq!(view.len(), data.len());
    for i in 0..data.len() {
        let got = view.get(i).unwrap();
        match (data[i], got) {
            (None, None) => {}
            (Some(exp), Some(got)) => assert_eq!(exp, got),
            _ => panic!("mismatch"),
        }
    }
}

#[test]
fn opt_str_column_rejects_non_monotonic_offsets() {
    let data = vec![Some("a"), Some("bb"), Some("ccc")];
    let (mut bytes, present) = ncb::encode_opt_str_column(&data);
    let n_rows = data.len();
    let bit_bytes = n_rows.div_ceil(8);
    let mut off = bit_bytes;
    let mis4 = off & 3;
    if mis4 != 0 {
        off += 4 - mis4;
    }
    let offs_start = off;
    let offs_len = 4 * (present + 1);
    assert!(offs_start + offs_len <= bytes.len());
    let off2 = u32::from_le_bytes(bytes[offs_start + 8..offs_start + 12].try_into().unwrap());
    let bad = off2.saturating_add(1);
    bytes[offs_start + 4..offs_start + 8].copy_from_slice(&bad.to_le_bytes());
    let res = ncb::view_opt_str_column(&bytes, n_rows);
    assert!(res.is_err());
}

#[test]
fn opt_u32_column_basic() {
    let data = vec![Some(1u32), None, Some(2u32), None, Some(3u32), Some(4u32)];
    let (bytes, present) = ncb::encode_opt_u32_column(&data);
    assert_eq!(present, 4);
    let view = ncb::view_opt_u32_column(&bytes, data.len()).expect("view");
    for i in 0..data.len() {
        match (data[i], view.get(i)) {
            (None, None) => {}
            (Some(a), Some(b)) => assert_eq!(a, b),
            _ => panic!("mismatch"),
        }
    }
}
