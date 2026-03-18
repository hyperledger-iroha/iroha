//! Property tests for u32-delta on combo NCB shapes.

use norito::columnar::*;
use proptest::prelude::*;

// Local helpers to force u32-delta encoding for combos (without touching crate internals)
fn pad_to(buf: &mut Vec<u8>, align: usize) {
    let mis = buf.len() & (align - 1);
    if mis != 0 {
        let pad = align - mis;
        buf.extend(std::iter::repeat_n(0u8, pad));
    }
}
fn zigzag_encode(x: i64) -> u64 {
    ((x << 1) ^ (x >> 63)) as u64
}
fn write_var_u64(buf: &mut Vec<u8>, mut v: u64) {
    while v >= 0x80 {
        buf.push((v as u8) | 0x80);
        v >>= 7;
    }
    buf.push(v as u8);
}

fn encode_str_u32_delta(rows: &[(u64, &str, u32, bool)]) -> Vec<u8> {
    // Descriptor 0x37 = DESC_U64_STR_U32DELTA_BOOL (non-delta ids, offsets names, u32 delta)
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
    let mut offs = Vec::with_capacity(n + 1);
    offs.push(0);
    for &(_, s, _, _) in rows {
        let b = s.as_bytes();
        acc = acc.wrapping_add(b.len() as u32);
        offs.push(acc);
        buf.extend_from_slice(b);
    }
    for (i, v) in offs.iter().enumerate() {
        let p = base_off + i * 4;
        buf[p..p + 4].copy_from_slice(&v.to_le_bytes());
    }
    pad_to(&mut buf, 4);
    if n > 0 {
        buf.extend_from_slice(&rows[0].2.to_le_bytes());
        let mut prev = rows[0].2 as i64;
        for &(_, _, v, _) in rows.iter().skip(1) {
            let d = (v as i64) - prev;
            prev = v as i64;
            write_var_u64(&mut buf, zigzag_encode(d));
        }
    }
    let bit_bytes = n.div_ceil(8);
    let start = buf.len();
    buf.extend(std::iter::repeat_n(0u8, bit_bytes));
    for (i, &(_, _, _, f)) in rows.iter().enumerate() {
        if f {
            buf[start + (i / 8)] |= 1u8 << (i % 8);
        }
    }
    buf
}

fn encode_bytes_u32_delta(rows: &[(u64, &[u8], u32, bool)]) -> Vec<u8> {
    // Descriptor 0x38 = DESC_U64_BYTES_U32DELTA_BOOL (non-delta ids, bytes offsets, u32 delta)
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
    let mut offs = Vec::with_capacity(n + 1);
    offs.push(0);
    for &(_, b, _, _) in rows {
        acc = acc.wrapping_add(b.len() as u32);
        offs.push(acc);
        buf.extend_from_slice(b);
    }
    for (i, v) in offs.iter().enumerate() {
        let p = base_off + i * 4;
        buf[p..p + 4].copy_from_slice(&v.to_le_bytes());
    }
    pad_to(&mut buf, 4);
    if n > 0 {
        buf.extend_from_slice(&rows[0].2.to_le_bytes());
        let mut prev = rows[0].2 as i64;
        for &(_, _, v, _) in rows.iter().skip(1) {
            let d = (v as i64) - prev;
            prev = v as i64;
            write_var_u64(&mut buf, zigzag_encode(d));
        }
    }
    let bit_bytes = n.div_ceil(8);
    let start = buf.len();
    buf.extend(std::iter::repeat_n(0u8, bit_bytes));
    for (i, &(_, _, _, f)) in rows.iter().enumerate() {
        if f {
            buf[start + (i / 8)] |= 1u8 << (i % 8);
        }
    }
    buf
}

fn small_name() -> impl Strategy<Value = String> {
    prop::collection::vec(any::<u8>(), 0..12).prop_map(|bs| {
        bs.into_iter()
            .map(|b| ((b % 26) + b'a') as char)
            .collect::<String>()
    })
}

fn small_bytes() -> impl Strategy<Value = Vec<u8>> {
    // keep short and varied lengths
    prop::collection::vec(any::<u8>(), 0..16)
}

proptest! {
    // (u64, &str, u32, bool) — force u32-delta, roundtrip via view
    #[test]
    fn prop_str_u32_delta_roundtrip(
        n in 0usize..128,
        names in prop::collection::vec(small_name(), 0..256),
        values in prop::collection::vec(any::<u32>(), 0..256),
        flags in prop::collection::vec(any::<bool>(), 0..256),
    ) {
        let n = n.min(128);
        // Build rows
        let mut rows_owned: Vec<(u64, String, u32, bool)> = Vec::with_capacity(n);
        for i in 0..n {
            let id = (i as u64) * 3 + 7;
            let s = names.get(i % names.len().max(1)).cloned().unwrap_or_default();
            let v = *values.get(i % values.len().max(1)).unwrap_or(&0);
            let f = *flags.get(i % flags.len().max(1)).unwrap_or(&false);
            rows_owned.push((id, s, v, f));
        }
        let rows_borrowed: Vec<(u64, &str, u32, bool)> = rows_owned.iter().map(|(id,s,v,f)| (*id, s.as_str(), *v, *f)).collect();
        // Encode with forced u32-delta and view
        let ncb = encode_str_u32_delta(&rows_borrowed);
        // misalignment stress
        let mut pref = vec![0xCC, 0xDD];
        pref.extend_from_slice(&ncb);
        let view = view_ncb_u64_str_u32_bool(&pref[2..]).expect("view str-u32");
        prop_assert_eq!(view.len(), n);
        for (i, row) in rows_owned.iter().enumerate().take(n) {
            prop_assert_eq!(view.id(i), row.0);
            prop_assert_eq!(view.name(i).unwrap(), row.1.as_str());
            prop_assert_eq!(view.val(i), row.2);
            prop_assert_eq!(view.flag(i), row.3);
        }
    }

    // (u64, &[u8], u32, bool) — force u32-delta, roundtrip via view
    #[test]
    fn prop_bytes_u32_delta_roundtrip(
        n in 0usize..128,
        blobs in prop::collection::vec(small_bytes(), 0..256),
        values in prop::collection::vec(any::<u32>(), 0..256),
        flags in prop::collection::vec(any::<bool>(), 0..256),
    ) {
        let n = n.min(128);
        // Build rows
        let mut rows_owned: Vec<(u64, Vec<u8>, u32, bool)> = Vec::with_capacity(n);
        for i in 0..n {
            let id = (i as u64) * 5 + 13;
            let b = blobs.get(i % blobs.len().max(1)).cloned().unwrap_or_default();
            let v = *values.get(i % values.len().max(1)).unwrap_or(&0);
            let f = *flags.get(i % flags.len().max(1)).unwrap_or(&false);
            rows_owned.push((id, b, v, f));
        }
        let rows_borrowed: Vec<(u64, &[u8], u32, bool)> = rows_owned.iter().map(|(id,b,v,f)| (*id, b.as_slice(), *v, *f)).collect();
        // Encode with forced u32-delta and view
        let ncb = encode_bytes_u32_delta(&rows_borrowed);
        // misalignment stress
        let mut pref = vec![0xAB];
        pref.extend_from_slice(&ncb);
        let view = view_ncb_u64_bytes_u32_bool(&pref[1..]).expect("view bytes-u32");
        prop_assert_eq!(view.len(), n);
        for (i, row) in rows_owned.iter().enumerate().take(n) {
            prop_assert_eq!(view.id(i), row.0);
            prop_assert_eq!(view.data(i), row.1.as_slice());
            prop_assert_eq!(view.val(i), row.2);
            prop_assert_eq!(view.flag(i), row.3);
        }
    }
}
