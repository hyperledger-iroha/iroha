//! Deterministic tests for u32-delta on combo NCB shapes.

use norito::columnar::*;

type StrRow<'a> = (u64, &'a str, u32, bool);
type BytesRow<'a> = (u64, &'a [u8], u32, bool);

fn pad_to(buf: &mut Vec<u8>, align: usize) {
    let mis = buf.len() & (align - 1);
    if mis != 0 {
        let pad = align - mis;
        buf.extend(std::iter::repeat_n(0u8, pad));
    }
}

fn zigzag_encode(value: i64) -> u64 {
    ((value << 1) ^ (value >> 63)) as u64
}

fn write_var_u64(buf: &mut Vec<u8>, mut value: u64) {
    while value >= 0x80 {
        buf.push((value as u8) | 0x80);
        value >>= 7;
    }
    buf.push(value as u8);
}

fn encode_str_u32_delta(rows: &[StrRow<'_>]) -> Vec<u8> {
    const DESC: u8 = 0x37;
    let n = rows.len();
    let mut buf = Vec::new();
    buf.extend_from_slice(&(n as u32).to_le_bytes());
    buf.push(DESC);
    pad_to(&mut buf, 8);
    for &(id, _, _, _) in rows {
        buf.extend_from_slice(&id.to_le_bytes());
    }
    pad_to(&mut buf, 4);
    let base_off = buf.len();
    buf.extend(std::iter::repeat_n(0u8, 4 * (n + 1)));
    let mut acc: u32 = 0;
    let mut offsets = Vec::with_capacity(n + 1);
    offsets.push(0);
    for &(_, value, _, _) in rows {
        acc = acc.wrapping_add(value.len() as u32);
        offsets.push(acc);
        buf.extend_from_slice(value.as_bytes());
    }
    for (idx, value) in offsets.iter().enumerate() {
        let pos = base_off + idx * 4;
        buf[pos..pos + 4].copy_from_slice(&value.to_le_bytes());
    }
    pad_to(&mut buf, 4);
    if n > 0 {
        buf.extend_from_slice(&rows[0].2.to_le_bytes());
        let mut prev = rows[0].2 as i64;
        for &(_, _, value, _) in rows.iter().skip(1) {
            let delta = value as i64 - prev;
            prev = value as i64;
            write_var_u64(&mut buf, zigzag_encode(delta));
        }
    }
    let bit_bytes = n.div_ceil(8);
    let start = buf.len();
    buf.extend(std::iter::repeat_n(0u8, bit_bytes));
    for (idx, &(_, _, _, flag)) in rows.iter().enumerate() {
        if flag {
            buf[start + (idx / 8)] |= 1u8 << (idx % 8);
        }
    }
    buf
}

fn encode_bytes_u32_delta(rows: &[BytesRow<'_>]) -> Vec<u8> {
    const DESC: u8 = 0x38;
    let n = rows.len();
    let mut buf = Vec::new();
    buf.extend_from_slice(&(n as u32).to_le_bytes());
    buf.push(DESC);
    pad_to(&mut buf, 8);
    for &(id, _, _, _) in rows {
        buf.extend_from_slice(&id.to_le_bytes());
    }
    pad_to(&mut buf, 4);
    let base_off = buf.len();
    buf.extend(std::iter::repeat_n(0u8, 4 * (n + 1)));
    let mut acc: u32 = 0;
    let mut offsets = Vec::with_capacity(n + 1);
    offsets.push(0);
    for &(_, value, _, _) in rows {
        acc = acc.wrapping_add(value.len() as u32);
        offsets.push(acc);
        buf.extend_from_slice(value);
    }
    for (idx, value) in offsets.iter().enumerate() {
        let pos = base_off + idx * 4;
        buf[pos..pos + 4].copy_from_slice(&value.to_le_bytes());
    }
    pad_to(&mut buf, 4);
    if n > 0 {
        buf.extend_from_slice(&rows[0].2.to_le_bytes());
        let mut prev = rows[0].2 as i64;
        for &(_, _, value, _) in rows.iter().skip(1) {
            let delta = value as i64 - prev;
            prev = value as i64;
            write_var_u64(&mut buf, zigzag_encode(delta));
        }
    }
    let bit_bytes = n.div_ceil(8);
    let start = buf.len();
    buf.extend(std::iter::repeat_n(0u8, bit_bytes));
    for (idx, &(_, _, _, flag)) in rows.iter().enumerate() {
        if flag {
            buf[start + (idx / 8)] |= 1u8 << (idx % 8);
        }
    }
    buf
}

#[test]
fn str_u32_delta_roundtrip() {
    let cases: Vec<Vec<StrRow<'_>>> = vec![
        Vec::new(),
        vec![(7, "", 0, false)],
        vec![
            (7, "aa", 0, true),
            (10, "bb", 1, false),
            (13, "cc", u32::MAX, true),
        ],
    ];

    for rows in cases {
        let ncb = encode_str_u32_delta(&rows);
        let mut prefixed = vec![0xCC, 0xDD];
        prefixed.extend_from_slice(&ncb);
        let view = view_ncb_u64_str_u32_bool(&prefixed[2..]).expect("view str-u32");
        assert_eq!(view.len(), rows.len());
        for (idx, row) in rows.iter().enumerate() {
            assert_eq!(view.id(idx), row.0);
            assert_eq!(view.name(idx).unwrap(), row.1);
            assert_eq!(view.val(idx), row.2);
            assert_eq!(view.flag(idx), row.3);
        }
    }
}

#[test]
fn bytes_u32_delta_roundtrip() {
    let empty: &[u8] = &[];
    let short: &[u8] = &[0, 1, 2];
    let maxish: &[u8] = &[255, 254, 253, 252];
    let cases: Vec<Vec<BytesRow<'_>>> = vec![
        Vec::new(),
        vec![(13, empty, 0, false)],
        vec![
            (13, short, 0, true),
            (18, empty, 1, false),
            (23, maxish, u32::MAX, true),
        ],
    ];

    for rows in cases {
        let ncb = encode_bytes_u32_delta(&rows);
        let mut prefixed = vec![0xAB];
        prefixed.extend_from_slice(&ncb);
        let view = view_ncb_u64_bytes_u32_bool(&prefixed[1..]).expect("view bytes-u32");
        assert_eq!(view.len(), rows.len());
        for (idx, row) in rows.iter().enumerate() {
            assert_eq!(view.id(idx), row.0);
            assert_eq!(view.data(idx), row.1);
            assert_eq!(view.val(idx), row.2);
            assert_eq!(view.flag(idx), row.3);
        }
    }
}
