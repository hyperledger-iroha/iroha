//! Negative tests for NCB (columnar) borrowed views.
//! Exercise malformed descriptors, offsets, tags, and bounds.

use norito::core::Error;

fn corrupt_first_id_delta(body: &mut [u8]) {
    let mut off = 5usize;
    let mis = off & 7;
    if mis != 0 {
        off += 8 - mis;
    }
    off += 8; // base id
    assert!(off < body.len());
    body[off] = 3; // zigzag(-2)
}

fn overflow_varint_bytes() -> Vec<u8> {
    let mut bytes = vec![0x80; 9];
    bytes.push(0x02);
    bytes
}

#[test]
fn str_bool_invalid_descriptor_and_short() {
    // Too short
    assert!(matches!(
        norito::columnar::view_ncb_u64_str_bool(&[]),
        Err(Error::LengthMismatch)
    ));

    // Invalid descriptor byte
    let rows: Vec<(u64, &str, bool)> = vec![(1, "a", true)];
    let mut body = norito::columnar::encode_ncb_u64_str_bool(&rows);
    body[4] = 0xFF; // not one of the recognized descriptors
    let res = norito::columnar::view_ncb_u64_str_bool(&body);
    assert!(res.is_err());
    if let Err(err) = res {
        assert!(matches!(err, Error::Message(_)));
    }
}

#[test]
fn str_bool_truncated_ids_and_offsets() {
    // Truncated ids (base layout)
    let rows: Vec<(u64, &str, bool)> = vec![(1, "a", true), (2, "b", false)];
    let mut body = norito::columnar::encode_ncb_u64_str_bool(&rows);
    let n = u32::from_le_bytes(body[0..4].try_into().unwrap()) as usize;
    assert_eq!(n, 2);
    let mut off = 5usize;
    let mis = off & 7;
    if mis != 0 {
        off += 8 - mis;
    }
    // Remove one byte inside ids region to make ids slice too short
    body.remove(off); // corrupt ids length
    let res = norito::columnar::view_ncb_u64_str_bool(&body);
    assert!(res.is_err());

    // Truncated offsets array (base layout)
    let mut body = norito::columnar::encode_ncb_u64_str_bool(&rows);
    let mut off = 5usize;
    let mis = off & 7;
    if mis != 0 {
        off += 8 - mis;
    }
    off += 8 * n; // ids slice
    let mis4 = off & 3;
    if mis4 != 0 {
        off += 4 - mis4;
    }
    // Remove a byte inside the offsets slice
    body.remove(off);
    let res2 = norito::columnar::view_ncb_u64_str_bool(&body);
    assert!(res2.is_err());
}

#[test]
fn str_bool_invalid_utf8_offsets_blob() {
    // Two distinct short names → offsets layout
    let rows: Vec<(u64, &str, bool)> = vec![(1, "aa", true), (2, "bb", false)];
    let mut body = norito::columnar::encode_ncb_u64_str_bool(&rows);
    let n = u32::from_le_bytes(body[0..4].try_into().unwrap()) as usize;
    // ids
    let desc = body[4];
    let mut off = 5usize;
    let mis = off & 7;
    if mis != 0 {
        off += 8 - mis;
    }
    // handle possible id-delta
    if desc == 0x53 {
        // DESC_U64_DELTA_STR_BOOL
        off += 8;
        let mut p = off;
        for _ in 1..n {
            loop {
                let b = body[p];
                p += 1;
                if (b & 0x80) == 0 {
                    break;
                }
            }
        }
        off = p;
    } else {
        off += 8 * n;
    }
    // names offsets+blob
    let mis4 = off & 3;
    if mis4 != 0 {
        off += 4 - mis4;
    }
    let offs_len = 4 * (n + 1);
    let blob_start = off + offs_len;
    assert!(blob_start < body.len());
    body[blob_start] = 0xff; // corrupt first blob byte
    // View should fail due to InvalidUtf8 at parse time
    let res = norito::columnar::view_ncb_u64_str_bool(&body);
    assert!(res.is_err());
    if let Err(e) = res {
        assert!(matches!(e, Error::InvalidUtf8));
    }
}

#[test]
fn str_bool_invalid_utf8_dict_blob() {
    // Repeated long name triggers dict layout (avg>=8, ratio<=0.4)
    let rows: Vec<(u64, &str, bool)> = vec![
        (1, "abcdefgh", true),
        (2, "abcdefgh", false),
        (3, "abcdefgh", true),
        (4, "abcdefgh", false),
        (5, "abcdefgh", false),
    ];
    let mut body = norito::columnar::encode_ncb_u64_str_bool(&rows);
    // ids
    let n = u32::from_le_bytes(body[0..4].try_into().unwrap()) as usize;
    let mut off = 5usize;
    let mis = off & 7;
    if mis != 0 {
        off += 8 - mis;
    }
    off += 8 * n;
    // names dict (align 4)
    let mis4 = off & 3;
    if mis4 != 0 {
        off += 4 - mis4;
    }
    // dict_len
    let dict_len = u32::from_le_bytes(body[off..off + 4].try_into().unwrap()) as usize;
    let offs_start = off + 4;
    let blob_start = offs_start + 4 * (dict_len + 1);
    assert!(blob_start < body.len());
    body[blob_start] = 0xff; // corrupt dict blob
    let res = norito::columnar::view_ncb_u64_str_bool(&body);
    assert!(res.is_err());
    if let Err(e) = res {
        assert!(matches!(e, Error::InvalidUtf8));
    }
}

#[test]
fn str_bool_delta_ids_unterminated_varint() {
    // Craft rows that choose delta ids: clustered ids (1,2,3)
    let rows: Vec<(u64, &str, bool)> = vec![(1, "a", true), (2, "b", false), (3, "c", true)];
    let mut body = norito::columnar::encode_ncb_u64_str_bool(&rows);
    // Force descriptor to delta if not already (just in case)
    body[4] = 0x53; // DESC_U64_DELTA_STR_BOOL
    let _n = rows.len();
    // Find start of varint deltas after base id
    let mut off = 5usize;
    let mis = off & 7;
    if mis != 0 {
        off += 8 - mis;
    }
    off += 8; // base u64
    // Walk first varint to its end
    let mut end1 = off;
    loop {
        let b = body[end1];
        end1 += 1;
        if (b & 0x80) == 0 {
            break;
        }
    }
    // Walk second varint end
    let mut end2 = end1;
    loop {
        let b = body[end2];
        end2 += 1;
        if (b & 0x80) == 0 {
            break;
        }
    }
    // Corrupt last varint: set its last byte continuation bit and truncate buffer right after it
    body[end2 - 1] |= 0x80;
    body.truncate(end2); // end2 now points past last byte; leave as-is; parser expects one more byte
    let res = norito::columnar::view_ncb_u64_str_bool(&body);
    assert!(res.is_err());
}

#[test]
fn str_bool_delta_ids_non_canonical_varint() {
    let rows: Vec<(u64, &str, bool)> = vec![(1, "a", true), (2, "b", false)];
    let mut body = norito::columnar::encode_ncb_u64_str_bool_delta(&rows);
    // Locate first delta varint (after header padding + base id).
    let mut off = 5usize;
    let mis = off & 7;
    if mis != 0 {
        off += 8 - mis;
    }
    off += 8;
    // Replace canonical 0x02 with overlong 0x82 0x00.
    body.insert(off, 0x82);
    body[off + 1] = 0x00;
    let res = norito::columnar::view_ncb_u64_str_bool(&body);
    assert!(res.is_err());
}

#[test]
fn enum_code_delta_unterminated_varint() {
    use norito::columnar::{EnumBorrow, encode_ncb_u64_enum_bool, view_ncb_u64_enum_bool};
    // Two CODE variants to ensure n_code >= 2 for delta varints
    let rows = vec![
        (1u64, EnumBorrow::Code(5), true),
        (2u64, EnumBorrow::Code(9), false),
    ];
    let mut body = encode_ncb_u64_enum_bool(&rows, false, false, true); // code_delta=true
    // Locate codes segment; mirror view logic
    let n = u32::from_le_bytes(body[0..4].try_into().unwrap()) as usize;
    // ids
    let mut off = 5usize;
    let mis = off & 7;
    if mis != 0 {
        off += 8 - mis;
    }
    off += 8 * n;
    // tags
    off += n;
    // names: zero names; names offsets may be omitted; but for code-only rows Name count=0 so names section minimal
    let mis4 = off & 3;
    if mis4 != 0 {
        off += 4 - mis4;
    }
    // For offsets-based names and n_name=0, section size is 4 (offs array of len 1) + 0 blob
    // We need to recompute as view does; simplest approach: scan tags count
    let n_name = body[(5 + ((5 & 7 != 0) as usize * (8 - (5 & 7))) + 8 * n)
        ..(5 + ((5 & 7 != 0) as usize * (8 - (5 & 7))) + 8 * n + n)]
        .iter()
        .filter(|&&t| t == 0)
        .count();
    // Names section depends on name_dict flag (false) so Offsets case:
    let mis4_names = off & 3;
    if mis4_names != 0 {
        off += 4 - mis4_names;
    }
    off += 4 * (n_name + 1);
    let last = u32::from_le_bytes(body[off - 4..off].try_into().unwrap()) as usize;
    off += last;
    let mis4_codes = off & 3;
    if mis4_codes != 0 {
        off += 4 - mis4_codes;
    }
    // Now at codes segment start: base u32 + (n_code-1) varints
    let n_code = n - n_name;
    if n_code >= 2 {
        // Skip base u32
        off += 4;
        // First varint end
        let mut end = off;
        loop {
            let b = body[end];
            end += 1;
            if (b & 0x80) == 0 {
                break;
            }
        }
        // Corrupt: set continuation bit; truncate to force unterminated varint
        body[end - 1] |= 0x80;
        body.truncate(end);
        let res = view_ncb_u64_enum_bool(&body);
        assert!(res.is_err());
    }
}

#[test]
fn bytes_bool_offsets_out_of_bounds_and_short_bits() {
    let rows: Vec<(u64, &[u8], bool)> = vec![(1, b"xy", true), (2, b"z", false)];
    let mut body = norito::columnar::encode_ncb_u64_bytes_bool(&rows);
    // Locate start of offsets table: header(5) -> align 8 -> ids (8*n or delta) -> align 4 -> offsets
    let n = u32::from_le_bytes(body[0..4].try_into().unwrap()) as usize;
    let mut off = 5usize;
    let mis = off & 7;
    if mis != 0 {
        off += 8 - mis;
    }
    // ids: handle delta or base layout
    let desc = body[4];
    if desc == 0x54 {
        // DESC_U64_DELTA_BYTES_BOOL
        // base u64 + (n-1) varint deltas
        off += 8;
        let mut p = off;
        for _ in 1..n {
            // advance over LEB128
            loop {
                let b = body[p];
                p += 1;
                if (b & 0x80) == 0 {
                    break;
                }
            }
        }
        off = p;
    } else {
        off += 8 * n;
    }
    let mis4 = off & 3;
    if mis4 != 0 {
        off += 4 - mis4;
    }
    // Tamper last offset to exceed blob length
    let last_off_pos = off + 4 * n;
    let orig_last = u32::from_le_bytes(body[last_off_pos..last_off_pos + 4].try_into().unwrap());
    let new_last = orig_last.saturating_add(1);
    body[last_off_pos..last_off_pos + 4].copy_from_slice(&new_last.to_le_bytes());
    let res = norito::columnar::view_ncb_u64_bytes_bool(&body);
    assert!(res.is_err());
    if let Err(err) = res {
        assert!(matches!(err, Error::LengthMismatch));
    }

    // Restore and then truncate bitset to be too short
    body[last_off_pos..last_off_pos + 4].copy_from_slice(&orig_last.to_le_bytes());
    body.pop(); // drop one byte from the end (bitset area)
    let res2 = norito::columnar::view_ncb_u64_bytes_bool(&body);
    assert!(res2.is_err());
    if let Err(err2) = res2 {
        assert!(matches!(err2, Error::LengthMismatch));
    }
}

#[test]
fn enum_invalid_descriptor_and_tag_and_dict_code_oob() {
    use norito::columnar::{EnumBorrow, encode_ncb_u64_enum_bool, view_ncb_u64_enum_bool};

    // Invalid enum descriptor
    let rows = vec![(1u64, EnumBorrow::Name("n"), true)];
    let mut body = encode_ncb_u64_enum_bool(&rows, false, false, false);
    body[4] = 0x00; // invalid for enum
    let res = view_ncb_u64_enum_bool(&body);
    assert!(res.is_err());
    if let Err(err) = res {
        assert!(matches!(err, Error::Message(_)));
    }

    // Invalid tag value (neither NAME(0) nor CODE(1)) → view should reject
    let mut body = encode_ncb_u64_enum_bool(&rows, false, false, false);
    // After header align ids (8), then tags start; For n=1, ids slice at offset 5 align8 -> 8 bytes, then tags at next byte
    let mut off = 5usize;
    let mis = off & 7;
    if mis != 0 {
        off += 8 - mis;
    }
    off += 8; // ids
    body[off] = 9u8; // invalid tag for row 0
    let res_view = view_ncb_u64_enum_bool(&body);
    assert!(res_view.is_err());
    if let Err(err2) = res_view {
        assert!(matches!(err2, Error::InvalidTag { .. }));
    }

    // Dict-coded names with code out-of-bounds
    let rows2 = vec![
        (1u64, EnumBorrow::Name("aa"), true),
        (2u64, EnumBorrow::Code(7), false),
    ];
    let mut body = encode_ncb_u64_enum_bool(&rows2, false, true, false); // use_dict=true
    // Walk to names dict section and per-Name codes as in view
    let n = u32::from_le_bytes(body[0..4].try_into().unwrap()) as usize;
    let mut off = 5usize;
    let mis = off & 7;
    if mis != 0 {
        off += 8 - mis;
    }
    off += 8 * n; // ids slice
    // tags
    let _tags_off = off;
    off += n;
    // names section aligned 4
    let mut off_names = off;
    let mis4 = off_names & 3;
    if mis4 != 0 {
        off_names += 4 - mis4;
    }
    let dict_len = u32::from_le_bytes(body[off_names..off_names + 4].try_into().unwrap()) as usize;
    off_names += 4;
    let dict_offs_bytes_len = 4 * (dict_len + 1);
    off_names += dict_offs_bytes_len; // dict offs
    let dict_last = u32::from_le_bytes(body[off_names - 4..off_names].try_into().unwrap()) as usize;
    off_names += dict_last; // dict blob
    let mis4_codes = off_names & 3;
    if mis4_codes != 0 {
        off_names += 4 - mis4_codes;
    }
    // There is exactly one Name row; set its code to an out-of-bounds index (== dict_len)
    if dict_len > 0 {
        body[off_names..off_names + 4].copy_from_slice(&(dict_len as u32).to_le_bytes());
        let res_view = view_ncb_u64_enum_bool(&body);
        assert!(matches!(res_view, Err(Error::LengthMismatch)));
    }
}

#[test]
fn str_bool_dict_codes_out_of_range() {
    let rows: Vec<(u64, &str, bool)> =
        vec![(1, "alpha", true), (2, "beta", false), (3, "alpha", true)];
    let mut body = norito::columnar::encode_ncb_u64_str_bool_force_dict(&rows);
    let n = u32::from_le_bytes(body[0..4].try_into().unwrap()) as usize;
    assert_eq!(n, rows.len());
    let mut off = 5usize;
    let mis = off & 7;
    if mis != 0 {
        off += 8 - mis;
    }
    off += 8 * n;
    let mis4 = off & 3;
    if mis4 != 0 {
        off += 4 - mis4;
    }
    let dict_len = u32::from_le_bytes(body[off..off + 4].try_into().unwrap()) as usize;
    off += 4;
    let offs_start = off;
    let offs_len = 4 * (dict_len + 1);
    let last = u32::from_le_bytes(
        body[offs_start + 4 * dict_len..offs_start + 4 * dict_len + 4]
            .try_into()
            .unwrap(),
    ) as usize;
    off += offs_len;
    off += last;
    let mis4_codes = off & 3;
    if mis4_codes != 0 {
        off += 4 - mis4_codes;
    }
    body[off..off + 4].copy_from_slice(&(dict_len as u32).to_le_bytes());
    let res = norito::columnar::view_ncb_u64_str_bool(&body);
    assert!(res.is_err());
}

#[test]
fn str_bool_id_delta_underflow() {
    let rows: Vec<(u64, &str, bool)> = vec![(1, "a", true), (2, "b", false)];
    let mut body = norito::columnar::encode_ncb_u64_str_bool_delta(&rows);
    assert_eq!(body[4], 0x53);
    corrupt_first_id_delta(&mut body);
    let res = norito::columnar::view_ncb_u64_str_bool(&body);
    assert!(res.is_err());
}

#[test]
fn bytes_bool_id_delta_underflow() {
    let rows: Vec<(u64, &[u8], bool)> = vec![(1, b"aa", true), (2, b"bb", false)];
    let mut body = norito::columnar::encode_ncb_u64_bytes_bool(&rows);
    assert_eq!(body[4], 0x54);
    corrupt_first_id_delta(&mut body);
    let res = norito::columnar::view_ncb_u64_bytes_bool(&body);
    assert!(res.is_err());
}

#[test]
fn str_u32_bool_names_offsets_len_not_multiple_of_4() {
    // Non-dict names: break offsets slice length so it's not 4*(n_name+1)
    let rows: Vec<(u64, &str, u32, bool)> = vec![(1, "aa", 3, true), (2, "b", 1, false)];
    let mut body = norito::columnar::encode_ncb_u64_str_u32_bool(&rows);
    let n = u32::from_le_bytes(body[0..4].try_into().unwrap()) as usize;
    // ids
    let mut off = 5usize;
    let mis = off & 7;
    if mis != 0 {
        off += 8 - mis;
    }
    off += 8 * n;
    // tags
    off += n;
    // names offsets start (non-dict)
    let mis4 = off & 3;
    if mis4 != 0 {
        off += 4 - mis4;
    }
    // Remove one byte inside offsets slice
    body.remove(off);
    let res = norito::columnar::view_ncb_u64_str_u32_bool(&body);
    assert!(res.is_err());
}

#[test]
fn u64_u32_bool_id_delta_underflow() {
    let rows: Vec<(u64, u32, bool)> = vec![(1, 10, true), (2, 11, false)];
    let mut body = norito::columnar::encode_ncb_u64_u32_bool(&rows, true, false);
    assert_eq!(body[4], 0x23);
    corrupt_first_id_delta(&mut body);
    let res = norito::columnar::view_ncb_u64_u32_bool(&body);
    assert!(res.is_err());
}

#[test]
fn u64_u32_bool_id_delta_overflow_rejected() {
    let mut body = Vec::new();
    body.extend_from_slice(&2u32.to_le_bytes());
    body.push(0x23);
    while body.len() & 7 != 0 {
        body.push(0);
    }
    body.extend_from_slice(&1u64.to_le_bytes());
    body.extend_from_slice(&overflow_varint_bytes());
    let res = norito::columnar::view_ncb_u64_u32_bool(&body);
    assert!(matches!(res, Err(Error::LengthMismatch)));
}

#[test]
fn str_u32_bool_names_offsets_non_monotonic() {
    // Make offsets non-monotonic so s > e for some row
    let rows: Vec<(u64, &str, u32, bool)> =
        vec![(1, "ab", 3, true), (1 + (1u64 << 56), "c", 1, false)];
    let mut body = norito::columnar::encode_ncb_u64_str_u32_bool(&rows);
    let n = u32::from_le_bytes(body[0..4].try_into().unwrap()) as usize;
    // ids
    let mut off = 5usize;
    let mis = off & 7;
    if mis != 0 {
        off += 8 - mis;
    }
    off += 8 * n;
    // names offsets (non-dict)
    let mis4 = off & 3;
    if mis4 != 0 {
        off += 4 - mis4;
    }
    // Offsets slice length = 4*(n+1) = 12; Current expected offsets [0,2,3]. Make them [0,3,2] so row1 end < start
    // Write [0,3,2] little-endian
    body[off..off + 4].copy_from_slice(&0u32.to_le_bytes());
    body[off + 4..off + 8].copy_from_slice(&3u32.to_le_bytes());
    body[off + 8..off + 12].copy_from_slice(&2u32.to_le_bytes());
    let res = norito::columnar::view_ncb_u64_str_u32_bool(&body);
    assert!(res.is_err());
}

#[test]
fn str_u32_bool_id_delta_underflow() {
    let rows: Vec<(u64, &str, u32, bool)> = vec![(1, "a", 10, true), (2, "b", 11, false)];
    let policy = norito::columnar::ComboPolicy::default()
        .with_id_delta(true)
        .with_dictionary(false)
        .with_u32_delta(false);
    let mut body = norito::columnar::encode_ncb_u64_str_u32_bool_with_policy(&rows, policy);
    assert_eq!(body[4], 0x73);
    corrupt_first_id_delta(&mut body);
    let res = norito::columnar::view_ncb_u64_str_u32_bool(&body);
    assert!(res.is_err());
}

#[test]
fn bytes_u32_bool_offsets_non_monotonic() {
    // Non-monotonic offsets should error in bytes view as well
    let rows: Vec<(u64, &[u8], u32, bool)> = vec![
        (1, b"ab".as_slice(), 3, true),
        (1 + (1u64 << 56), b"c".as_slice(), 1, false),
    ];
    let mut body = norito::columnar::encode_ncb_u64_bytes_u32_bool(&rows);
    let n = u32::from_le_bytes(body[0..4].try_into().unwrap()) as usize;
    // ids
    let mut off = 5usize;
    let mis = off & 7;
    if mis != 0 {
        off += 8 - mis;
    }
    off += 8 * n;
    // bytes offsets (non-dict)
    let mis4 = off & 3;
    if mis4 != 0 {
        off += 4 - mis4;
    }
    // Write [0,3,2]
    body[off..off + 4].copy_from_slice(&0u32.to_le_bytes());
    body[off + 4..off + 8].copy_from_slice(&3u32.to_le_bytes());
    body[off + 8..off + 12].copy_from_slice(&2u32.to_le_bytes());
    let res = norito::columnar::view_ncb_u64_bytes_u32_bool(&body);
    assert!(res.is_err());
}

#[test]
fn bytes_u32_bool_id_delta_underflow() {
    let rows: Vec<(u64, &[u8], u32, bool)> = vec![(1, b"aa", 10, true), (2, b"bb", 11, false)];
    let mut body = norito::columnar::encode_ncb_u64_bytes_u32_bool(&rows);
    assert_eq!(body[4] & 0x40, 0x40);
    corrupt_first_id_delta(&mut body);
    let res = norito::columnar::view_ncb_u64_bytes_u32_bool(&body);
    assert!(res.is_err());
}

#[test]
fn str_u32_bool_invalid_utf8_offsets_blob() {
    let rows: Vec<(u64, &str, u32, bool)> = vec![(1, "aa", 3, true), (2, "bb", 7, false)];
    let mut body = norito::columnar::encode_ncb_u64_str_u32_bool(&rows);
    let n = u32::from_le_bytes(body[0..4].try_into().unwrap()) as usize;
    let desc = body[4];
    let mut off = 5usize;
    let mis = off & 7;
    if mis != 0 {
        off += 8 - mis;
    }
    // ids
    let id_delta = matches!(desc, 0x73 | 0xF3 | 0x77 | 0xF7);
    if id_delta {
        off += 8;
        let mut p = off;
        for _ in 1..n {
            loop {
                let b = body[p];
                p += 1;
                if (b & 0x80) == 0 {
                    break;
                }
            }
        }
        off = p;
    } else {
        off += 8 * n;
    }
    // names offsets (non-dict)
    let is_dict = matches!(desc, 0xB3 | 0xF3 | 0xB7 | 0xF7);
    assert!(!is_dict, "expected non-dict names for this dataset");
    let mis4 = off & 3;
    if mis4 != 0 {
        off += 4 - mis4;
    }
    let offs_len = 4 * (n + 1);
    let blob_start = off + offs_len;
    assert!(blob_start < body.len());
    body[blob_start] = 0xff;
    let res = norito::columnar::view_ncb_u64_str_u32_bool(&body);
    assert!(res.is_err());
    if let Err(e) = res {
        assert!(matches!(e, Error::InvalidUtf8));
    }
}

#[test]
fn str_u32_bool_invalid_utf8_dict_blob() {
    let rows: Vec<(u64, &str, u32, bool)> = vec![
        (1, "abcdefgh", 1, true),
        (2, "abcdefgh", 2, false),
        (3, "abcdefgh", 3, true),
        (4, "abcdefgh", 4, false),
        (5, "abcdefgh", 5, true),
    ];
    let mut body = norito::columnar::encode_ncb_u64_str_u32_bool(&rows);
    let n = u32::from_le_bytes(body[0..4].try_into().unwrap()) as usize;
    let desc = body[4];
    let mut off = 5usize;
    let mis = off & 7;
    if mis != 0 {
        off += 8 - mis;
    }
    // ids (may be delta or base)
    let id_delta = matches!(desc, 0x73 | 0xF3 | 0x77 | 0xF7);
    if id_delta {
        off += 8;
        let mut p = off;
        for _ in 1..n {
            loop {
                let b = body[p];
                p += 1;
                if (b & 0x80) == 0 {
                    break;
                }
            }
        }
        off = p;
    } else {
        off += 8 * n;
    }
    // names dict
    let mis4 = off & 3;
    if mis4 != 0 {
        off += 4 - mis4;
    }
    // dict len
    let dict_len = u32::from_le_bytes(body[off..off + 4].try_into().unwrap()) as usize;
    let offs_start = off + 4;
    let blob_start = offs_start + 4 * (dict_len + 1);
    assert!(blob_start < body.len());
    body[blob_start] = 0xff;
    let res = norito::columnar::view_ncb_u64_str_u32_bool(&body);
    assert!(res.is_err());
    if let Err(e) = res {
        assert!(matches!(e, Error::InvalidUtf8));
    }
}

#[test]
fn optstr_bool_invalid_utf8_blob() {
    // Mix of Some and None → opt-str column present>0, offsets+blob always used
    let rows: Vec<(u64, Option<&str>, bool)> = vec![
        (1, Some("aa"), true),
        (2, None, false),
        (3, Some("b"), true),
    ];
    let mut body = norito::columnar::encode_ncb_u64_optstr_bool(&rows);
    let n = u32::from_le_bytes(body[0..4].try_into().unwrap()) as usize;
    let desc = body[4];
    assert!(
        desc == 0x1B || desc == 0x5B,
        "unexpected optstr desc: {desc:02x}"
    );
    // ids
    let mut off = 5usize;
    let mis = off & 7;
    if mis != 0 {
        off += 8 - mis;
    }
    if desc == 0x5B {
        // DELTA: base u64 + (n-1) varints
        off += 8;
        let mut p = off;
        for _ in 1..n {
            loop {
                let b = body[p];
                p += 1;
                if (b & 0x80) == 0 {
                    break;
                }
            }
        }
        off = p;
    } else {
        off += 8 * n;
    }
    // opt-str column
    let opt_start = off;
    let bit_bytes = n.div_ceil(8);
    let mut p_local = bit_bytes;
    let mis4_local = p_local & 3;
    if mis4_local != 0 {
        p_local += 4 - mis4_local;
    }
    let p = opt_start + p_local;
    let present = rows.iter().filter(|(_, s, _)| s.is_some()).count();
    let offs_len = 4 * (present + 1);
    let blob_start = p + offs_len;
    assert!(blob_start < body.len());
    body[blob_start] = 0xff; // corrupt UTF-8
    let res = norito::columnar::view_ncb_u64_optstr_bool(&body);
    assert!(res.is_err());
    if let Err(e) = res {
        assert!(matches!(e, Error::InvalidUtf8));
    }
}

#[test]
fn bytes_u32_bool_bytes_offsets_len_not_multiple_of_4() {
    let rows: Vec<(u64, &[u8], u32, bool)> = vec![(1, b"xy", 3, true), (2, b"z", 0, false)];
    let mut body = norito::columnar::encode_ncb_u64_bytes_u32_bool(&rows);
    let n = u32::from_le_bytes(body[0..4].try_into().unwrap()) as usize;
    // ids
    let mut off = 5usize;
    let mis = off & 7;
    if mis != 0 {
        off += 8 - mis;
    }
    off += 8 * n;
    // bytes offsets start
    let mis4 = off & 3;
    if mis4 != 0 {
        off += 4 - mis4;
    }
    // Remove one byte inside offsets slice
    body.remove(off);
    let res = norito::columnar::view_ncb_u64_bytes_u32_bool(&body);
    assert!(res.is_err());
}

#[test]
fn enum_tags_count_mismatch_names_section_small() {
    use norito::columnar::{EnumBorrow, encode_ncb_u64_enum_bool, view_ncb_u64_enum_bool};
    // One Name and one Code
    let rows = vec![
        (1u64, EnumBorrow::Name("aaa"), true),
        (2u64, EnumBorrow::Code(7), false),
    ];
    let mut body = encode_ncb_u64_enum_bool(&rows, false, false, false);
    // Rewrite second tag from CODE(1) to NAME(0), increasing n_name from 1 to 2 but not expanding names offsets
    let n = 2usize;
    let mut off = 5usize;
    let mis = off & 7;
    if mis != 0 {
        off += 8 - mis;
    }
    off += 8 * n; // ids slice
    // tags: at [off .. off+n]
    body[off + 1] = 0u8; // set second tag to NAME
    let res = view_ncb_u64_enum_bool(&body);
    assert!(res.is_err());
}

#[test]
fn str_u32_bool_truncated_values_and_delta_unterminated() {
    // Base values truncated
    let rows: Vec<(u64, &str, u32, bool)> = vec![(1, "a", 3, true), (2, "bb", 7, false)];
    let mut body = norito::columnar::encode_ncb_u64_str_u32_bool(&rows);
    let n = u32::from_le_bytes(body[0..4].try_into().unwrap()) as usize;
    let desc = body[4];
    // Find start of values section mirroring view logic
    let mut off = 5usize;
    let mis = off & 7;
    if mis != 0 {
        off += 8 - mis;
    }
    // ids
    let id_delta = matches!(desc, 0x73 | 0xF3 | 0x77 | 0xF7); // DELTA_* variants for this shape
    if id_delta {
        // delta ids: base u64 + (n-1) varints
        off += 8;
        let mut p = off;
        for _ in 1..n {
            loop {
                let b = body[p];
                p += 1;
                if (b & 0x80) == 0 {
                    break;
                }
            }
        }
        off = p;
    } else {
        off += 8 * n;
    }
    // names section (dict or offsets)
    let is_dict = matches!(desc, 0xB3 | 0xF3 | 0xB7 | 0xF7);
    if is_dict {
        let mis4 = off & 3;
        if mis4 != 0 {
            off += 4 - mis4;
        }
        let dict_len = u32::from_le_bytes(body[off..off + 4].try_into().unwrap()) as usize;
        off += 4;
        off += 4 * (dict_len + 1);
        let dict_last = u32::from_le_bytes(body[off - 4..off].try_into().unwrap()) as usize;
        off += dict_last;
        let mis4c = off & 3;
        if mis4c != 0 {
            off += 4 - mis4c;
        }
        off += 4 * n; // codes for names
    } else {
        let mis4 = off & 3;
        if mis4 != 0 {
            off += 4 - mis4;
        }
        off += 4 * (n + 1);
        let last = u32::from_le_bytes(body[off - 4..off].try_into().unwrap()) as usize;
        off += last;
    }
    // Now at start of u32 values; align 4
    let mis4v = off & 3;
    if mis4v != 0 {
        off += 4 - mis4v;
    }
    let u32_delta = matches!(desc, 0x37 | 0xB7 | 0x77 | 0xF7);
    if !u32_delta {
        // Truncate inside values array
        let end_vals = off + 4 * n;
        body.truncate(end_vals - 1);
        let res = norito::columnar::view_ncb_u64_str_u32_bool(&body);
        assert!(res.is_err());
    } else {
        // Delta: corrupt last varint
        let mut p = off + 4; // base u32
        for _ in 1..n {
            loop {
                let b = body[p];
                p += 1;
                if (b & 0x80) == 0 {
                    break;
                }
            }
        }
        body[p - 1] |= 0x80; // set continuation
        body.truncate(p);
        let res = norito::columnar::view_ncb_u64_str_u32_bool(&body);
        assert!(res.is_err());
    }
}

#[test]
fn bytes_u32_bool_truncated_values_and_delta_unterminated() {
    // Base values truncated
    let rows: Vec<(u64, &[u8], u32, bool)> = vec![
        (1, b"abc".as_slice(), 2, true),
        (2, b"d".as_slice(), 9, false),
    ];
    let mut body = norito::columnar::encode_ncb_u64_bytes_u32_bool(&rows);
    let n = u32::from_le_bytes(body[0..4].try_into().unwrap()) as usize;
    let desc = body[4];
    let mut off = 5usize;
    let mis = off & 7;
    if mis != 0 {
        off += 8 - mis;
    }
    // ids
    let id_delta = matches!(desc, 0x74 | 0x76); // DELTA_* for bytes u32 shape
    if id_delta {
        off += 8;
        let mut p = off;
        for _ in 1..n {
            loop {
                let b = body[p];
                p += 1;
                if (b & 0x80) == 0 {
                    break;
                }
            }
        }
        off = p;
    } else {
        off += 8 * n;
    }
    // bytes offsets and blob
    let mis4 = off & 3;
    if mis4 != 0 {
        off += 4 - mis4;
    }
    off += 4 * (n + 1);
    let last = u32::from_le_bytes(body[off - 4..off].try_into().unwrap()) as usize;
    off += last;
    // values
    let mis4v = off & 3;
    if mis4v != 0 {
        off += 4 - mis4v;
    }
    let u32_delta = matches!(desc, 0x75 | 0x76);
    if !u32_delta {
        // Truncate buffer to ensure values/flags boundary is invalid
        body.pop();
        let res = norito::columnar::view_ncb_u64_bytes_u32_bool(&body);
        assert!(res.is_err());
    } else {
        // Delta: base u32 + (n-1) varints; corrupt last varint
        let mut p = off + 4;
        for _ in 1..n {
            loop {
                let b = body[p];
                p += 1;
                if (b & 0x80) == 0 {
                    break;
                }
            }
        }
        body[p - 1] |= 0x80;
        body.truncate(p);
        let res = norito::columnar::view_ncb_u64_bytes_u32_bool(&body);
        assert!(res.is_err());
    }
}

#[test]
fn bytes_u32_bool_invalid_descriptor() {
    // Build a simple valid payload then corrupt descriptor byte to 0xFF
    let rows: Vec<(u64, &[u8], u32, bool)> = vec![(1, b"a".as_slice(), 2, true)];
    let mut body = norito::columnar::encode_ncb_u64_bytes_u32_bool(&rows);
    body[4] = 0xFF;
    let res = norito::columnar::view_ncb_u64_bytes_u32_bool(&body);
    assert!(res.is_err());
}

#[test]
fn bytes_u32_bool_non_monotonic_offsets() {
    // Create two rows then set bytes offsets to a non-monotonic sequence (offs[i] > offs[i+1])
    let rows: Vec<(u64, &[u8], u32, bool)> = vec![
        (1, b"ab".as_slice(), 3, true),
        (2, b"c".as_slice(), 5, false),
    ];
    let mut body = norito::columnar::encode_ncb_u64_bytes_u32_bool(&rows);
    let n = u32::from_le_bytes(body[0..4].try_into().unwrap()) as usize;
    assert_eq!(n, 2);
    let desc = body[4];
    // Walk ids (aligned 8)
    let mut off = 5usize;
    let mis = off & 7;
    if mis != 0 {
        off += 8 - mis;
    }
    if matches!(desc, 0x54 | 0x74 | 0x78) {
        // id-delta: base + (n-1) varints
        off += 8;
        let mut p = off;
        for _ in 1..n {
            loop {
                let b = body[p];
                p += 1;
                if (b & 0x80) == 0 {
                    break;
                }
            }
        }
        off = p;
    } else {
        off += 8 * n;
    }
    // bytes offsets start (align 4)
    let mis4 = off & 3;
    if mis4 != 0 {
        off += 4 - mis4;
    }
    // Offsets array len is 3 u32s for n=2; corrupt the middle to be greater than the last
    let last_off_pos = off + 8; // position of offs[2]
    let last = u32::from_le_bytes(body[last_off_pos..last_off_pos + 4].try_into().unwrap());
    // Set offs[1] to last + 1 (non-monotonic)
    body[off + 4..off + 8].copy_from_slice(&(last + 1).to_le_bytes());
    let res = norito::columnar::view_ncb_u64_bytes_u32_bool(&body);
    assert!(res.is_err());
}

#[test]
fn enum_bool_id_delta_underflow() {
    let rows: Vec<(u64, norito::columnar::EnumBorrow<'_>, bool)> = vec![
        (1, norito::columnar::EnumBorrow::Name("alpha"), true),
        (2, norito::columnar::EnumBorrow::Code(7), false),
    ];
    let mut body = norito::columnar::encode_ncb_u64_enum_bool(
        &rows, /*id-delta*/ true, /*dict*/ false, /*code-delta*/ false,
    );
    assert_eq!(body[4], 0x63);
    corrupt_first_id_delta(&mut body);
    let res = norito::columnar::view_ncb_u64_enum_bool(&body);
    assert!(res.is_err());
}

#[test]
fn u64_u32_bool_delta_underflow_rejected() {
    let rows: Vec<(u64, u32, bool)> = vec![(10, 0, true), (20, 1, false)];
    let mut body = norito::columnar::encode_ncb_u64_u32_bool(&rows, false, true);
    let n = u32::from_le_bytes(body[0..4].try_into().unwrap()) as usize;
    assert_eq!(n, rows.len());
    let mut off = 5usize;
    let mis = off & 7;
    if mis != 0 {
        off += 8 - mis;
    }
    off += 8 * n; // ids
    let mis4 = off & 3;
    if mis4 != 0 {
        off += 4 - mis4;
    }
    off += 4; // base u32
    // Set delta to zigzag(-1) => 1, which should underflow.
    body[off] = 1;
    let res = norito::columnar::view_ncb_u64_u32_bool(&body);
    assert!(res.is_err());
}
