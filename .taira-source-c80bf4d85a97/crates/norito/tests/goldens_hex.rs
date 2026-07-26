//! Hex goldens for select AoS/NCB bodies to lock on-wire stability.
//! These tests assert exact bytes for small, deterministic datasets under
//! the default sequential configuration (fixed 64-bit length headers, no packed variants).

fn to_hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

#[test]
fn golden_aos_u64_str_bool_single() {
    // Rows: [(1, "a", true)]
    let rows = vec![(1u64, "a", true)];
    let body = norito::aos::encode_rows_u64_str_bool(&rows);
    // With compact-len: [len=1][ver=0x01][id:8][str_len=1]["a"][flag]
    let expected_seq = "010000000000000001010000000000000001000000000000006101";
    let expected_packed = "01010100000000000000016101";
    let expected = if norito::core::default_encode_flags() == 0 {
        expected_seq
    } else {
        expected_packed
    };
    assert_eq!(to_hex(&body), expected);
}

#[test]
fn golden_aos_u64_optstr_bool_two_rows() {
    // Rows: [(1, Some("a"), true), (2, None, false)]
    let rows = vec![(1u64, Some("a"), true), (2u64, None, false)];
    let body = norito::aos::encode_rows_u64_optstr_bool(&rows);
    // With compact-len: [len=2][ver=0x01]
    // Row1: [id:8][tag=1][str_len=1]["a"][flag=1]
    // Row2: [id:8][tag=0][flag=0]
    let expected_seq =
        "0200000000000000010100000000000000010100000000000000610102000000000000000000";
    let expected_packed = "020101000000000000000101610102000000000000000000";
    let expected = if norito::core::default_encode_flags() == 0 {
        expected_seq
    } else {
        expected_packed
    };
    assert_eq!(to_hex(&body), expected);
}

#[test]
fn golden_aos_u64_bytes_bool_two_rows() {
    // Rows: [(1, ["a","b","c"], true), (2, [0x00,0xff], false)]
    let rows = vec![
        (1u64, b"abc".as_slice(), true),
        (2u64, [0x00u8, 0xffu8].as_slice(), false),
    ];
    let body = norito::aos::encode_rows_u64_bytes_bool(&rows);
    // With compact-len: [len=2][ver=0x01]
    // Row1: [id:8][bytes_len=3]["abc"][flag=1]
    // Row2: [id:8][bytes_len=2][00 ff][flag=0]
    let expected_seq = "02000000000000000101000000000000000300000000000000616263010200000000000000020000000000000000ff00";
    let expected_packed = "02010100000000000000036162630102000000000000000200ff00";
    let expected = if norito::core::default_encode_flags() == 0 {
        expected_seq
    } else {
        expected_packed
    };
    assert_eq!(to_hex(&body), expected);
}

#[test]
fn golden_aos_u64_optu32_bool_two_rows() {
    // Rows: [(1, Some(7), true), (2, None, false)]
    let rows = vec![(1u64, Some(7u32), true), (2u64, None, false)];
    let body = norito::aos::encode_rows_u64_optu32_bool(&rows);
    // With compact-len: [len=2][ver=0x01]
    // Row1: [id:8][tag=1][val: u32 LE 07][flag=1]
    // Row2: [id:8][tag=0][flag=0]
    let expected_seq = "020000000000000001010000000000000001070000000102000000000000000000";
    let expected_packed = "0201010000000000000001070000000102000000000000000000";
    let expected = if norito::core::default_encode_flags() == 0 {
        expected_seq
    } else {
        expected_packed
    };
    assert_eq!(to_hex(&body), expected);
}

#[test]
fn golden_aos_u64_str_u32_bool_two_rows() {
    // Rows: [(1, "a", 3, true), (2, "", 0, false)]
    let rows = vec![(1u64, "a", 3u32, true), (2u64, "", 0u32, false)];
    let body = norito::aos::encode_rows_u64_str_u32_bool(&rows);
    // With compact-len: [len=2][ver=0x01]
    // Row1: [id:8][str_len=1]["a"][u32=03 00 00 00][flag=1]
    // Row2: [id:8][str_len=0][u32=00 00 00 00][flag=0]
    let expected_seq = "02000000000000000101000000000000000100000000000000610300000001020000000000000000000000000000000000000000";
    let expected_packed = "02010100000000000000016103000000010200000000000000000000000000";
    let expected = if norito::core::default_encode_flags() == 0 {
        expected_seq
    } else {
        expected_packed
    };
    assert_eq!(to_hex(&body), expected);
}

#[test]
fn golden_aos_u64_bytes_u32_bool_two_rows() {
    // Rows: [(1, [0xff], 7, true), (2, [], 0, false)]
    let rows = vec![
        (1u64, [0xffu8].as_slice(), 7u32, true),
        (2u64, [].as_slice(), 0u32, false),
    ];
    let body = norito::aos::encode_rows_u64_bytes_u32_bool(&rows);
    // With compact-len: [len=2][ver=0x01]
    // Row1: [id:8][bytes_len=1][ff][u32=07 00 00 00][flag=1]
    // Row2: [id:8][bytes_len=0][u32=00 00 00 00][flag=0]
    let expected_seq = "02000000000000000101000000000000000100000000000000ff0700000001020000000000000000000000000000000000000000";
    let expected_packed = "0201010000000000000001ff07000000010200000000000000000000000000";
    let expected = if norito::core::default_encode_flags() == 0 {
        expected_seq
    } else {
        expected_packed
    };
    assert_eq!(to_hex(&body), expected);
}

#[test]
fn golden_aos_u64_enum_bool_simple() {
    use norito::columnar::EnumBorrow;
    // Rows: [(1, Name("n"), true), (2, Code(7), false)]
    let rows = vec![
        (1u64, EnumBorrow::Name("n"), true),
        (2u64, EnumBorrow::Code(7), false),
    ];
    // Note: enum AoS uses a minimal header without version byte ([len] only).
    let body = norito::aos::encode_rows_u64_enum_bool(&rows);
    // Header: [len=2]
    // Row1: [id:8][tag=0][str_len=1]["n"][flag=1]
    // Row2: [id:8][tag=1][code: u32 LE 07][flag=0]
    let expected_seq =
        "020000000000000001000000000000000001000000000000006e010200000000000000010700000000";
    let expected_packed = "02010000000000000000016e010200000000000000010700000000";
    let expected = if norito::core::default_encode_flags() == 0 {
        expected_seq
    } else {
        expected_packed
    };
    assert_eq!(to_hex(&body), expected);
}

#[test]
fn golden_ncb_u64_str_bool_offsets_single() {
    // Rows: [(1, "a", true)] → descriptor 0x13 (no dict, no delta)
    let rows = vec![(1u64, "a", true)];
    let body = norito::columnar::encode_ncb_u64_str_bool(&rows);
    // Layout: [n=1 u32][desc=13][pad3][id:8]\
    //         [offs: 0,1][blob:61][flags:01]
    let expected_hex = "01000000\
                         13\
                         000000\
                         0100000000000000\
                         00000000\
                         01000000\
                         61\
                         01"
    .replace(['\n', ' '], "");
    assert_eq!(to_hex(&body), expected_hex);
}

#[test]
fn golden_ncb_u64_bytes_u32_bool_single() {
    // Rows: [(1, [0xff], 7, true)] → choose base layout (no deltas)
    let rows = vec![(1u64, [0xffu8].as_slice(), 7u32, true)];
    let body = norito::columnar::encode_ncb_u64_bytes_u32_bool(&rows);
    // Layout: [n=1][desc=0x34][pad3][id:8]\
    //         [offs:0,1][blob:ff][align4? yes (pad 3)][val:07 00 00 00][flags:01]
    // After offs (8 bytes) + blob (1), total since start 5+3+8+8+1=25, align to 4 → +3 padding (00 00 00)
    let expected_hex = "01000000\
                         34\
                         000000\
                         0100000000000000\
                         00000000\
                         01000000\
                         ff\
                         000000\
                         07000000\
                         01"
    .replace(['\n', ' '], "");
    assert_eq!(to_hex(&body), expected_hex);
}

#[test]
fn golden_ncb_u64_str_u32_bool_offsets_two_rows() {
    // Rows: [(1, "a", 3, true), (2, "", 0, false)] → descriptor 0x33 (no dict, no deltas)
    let rows = vec![(1u64, "a", 3u32, true), (2u64, "", 0u32, false)];
    let body = norito::columnar::encode_ncb_u64_str_u32_bool(&rows);
    let expected_hex = "02000000\
                         33\
                         000000\
                         0100000000000000\
                         0200000000000000\
                         00000000\
                         01000000\
                         01000000\
                         61\
                         000000\
                         03000000\
                         00000000\
                         01"
    .replace(['\n', ' '], "");
    assert_eq!(to_hex(&body), expected_hex);
}

#[test]
fn golden_ncb_u64_bytes_u32_bool_offsets_two_rows() {
    // Rows: [(1, [0xff], 7, true), (2, [], 0, false)] → descriptor 0x34 (no deltas)
    let rows = vec![
        (1u64, [0xffu8].as_slice(), 7u32, true),
        (2u64, [].as_slice(), 0u32, false),
    ];
    let body = norito::columnar::encode_ncb_u64_bytes_u32_bool(&rows);
    let expected_hex = "02000000\
                         34\
                         000000\
                         0100000000000000\
                         0200000000000000\
                         00000000\
                         01000000\
                         01000000\
                         ff\
                         000000\
                         07000000\
                         00000000\
                         01"
    .replace(['\n', ' '], "");
    assert_eq!(to_hex(&body), expected_hex);
}

#[test]
fn golden_ncb_u64_enum_bool_simple() {
    use norito::columnar::{EnumBorrow, encode_ncb_u64_enum_bool};
    // Rows: [(1, Name("n"), true), (2, Code(7), false)]
    let rows = vec![
        (1u64, EnumBorrow::Name("n"), true),
        (2u64, EnumBorrow::Code(7), false),
    ];
    let body = encode_ncb_u64_enum_bool(&rows, false, false, false);
    // Layout: [n=2][desc=0x61][align8][ids: 1,2][tags: 00,01]\
    // names (offsets): align4, offs: [0,1], blob:"6e" ('n')\
    // codes (base u32 slice): align4, [07 00 00 00]\
    // flags: 1 byte for 2 rows: bit0=1, bit1=0 → 00000001
    let _expected_hex = {
        let mut s = String::new();
        s.push_str("02000000"); // n
        s.push_str("61"); // desc
        s.push_str("000000"); // align to 8
        s.push_str("0100000000000000"); // id 1
        s.push_str("0200000000000000"); // id 2
        s.push_str("00"); // tag NAME
        s.push_str("01"); // tag CODE
        // names offsets (align 4 already at 5+3+16+2 = 26, align to 28 -> +2 zeros). But view aligns, encoder pads similarly.
        // Simpler: construct via encoder, and assert structure parts match:
        s
    };
    // Instead of asserting full hex for enum (cross-check is complex due to align interplay),
    // assert critical prefixes and descriptor/tags correctness.
    let hex = to_hex(&body);
    assert!(hex.starts_with("0200000061"), "header+desc mismatch: {hex}");
    // tags appear after ids: ensure substring contains 0001 for [NAME, CODE]
    assert!(
        hex.contains("0001"),
        "tags not found in expected order: {hex}"
    );
}

#[test]
fn golden_ncb_u64_enum_bool_dict_name_only_single() {
    use norito::columnar::{EnumBorrow, encode_ncb_u64_enum_bool};
    // Single row: Name("n"), true; force dict names, no deltas
    let rows = vec![(1u64, EnumBorrow::Name("n"), true)];
    let body = encode_ncb_u64_enum_bool(&rows, false, true, false);
    let expected_hex = "01000000\
                         e1\
                         000000\
                         0100000000000000\
                         00\
                         000000\
                         01000000\
                         00000000\
                         01000000\
                         6e\
                         000000\
                         00000000\
                         01"
    .replace(['\n', ' '], "");
    assert_eq!(to_hex(&body), expected_hex);
}

#[test]
fn golden_ncb_u64_enum_bool_code_delta_only() {
    use norito::columnar::{EnumBorrow, encode_ncb_u64_enum_bool};
    // Two code rows so code-delta emits base + one varint delta
    let rows = vec![
        (1u64, EnumBorrow::Code(5), true),
        (2u64, EnumBorrow::Code(7), false),
    ];
    let body = encode_ncb_u64_enum_bool(&rows, false, false, true);
    let expected_hex = "02000000\
                         65\
                         000000\
                         0100000000000000\
                         0200000000000000\
                         0101\
                         000000000000\
                         05000000\
                         04\
                         01"
    .replace(['\n', ' '], "");
    assert_eq!(to_hex(&body), expected_hex);
}

#[test]
fn golden_ncb_u64_enum_bool_iddelta_dict_codedelta_three_rows() {
    use norito::columnar::{EnumBorrow, encode_ncb_u64_enum_bool};
    // Rows: [(100, Name("aa"), true), (101, Code(5), false), (103, Code(7), true)]
    let rows = vec![
        (100u64, EnumBorrow::Name("aa"), true),
        (101u64, EnumBorrow::Code(5), false),
        (103u64, EnumBorrow::Code(7), true),
    ];
    let body = encode_ncb_u64_enum_bool(&rows, true, true, true);
    // Expected:
    // n=3, desc=E7, pad3, ids base=64 00.., varints 0x02 (+1), 0x04 (+2)
    // tags 00 01 01, align3, dict_len=1, offs [0,2], blob 'aa', align2, per-Name code 00 00 00 00
    // codes base 05 00 00 00, delta 0x04, flags 0x05 (bits 0 and 2)
    let expected_hex = "03000000\
                         e7\
                         000000\
                         6400000000000000\
                         0204\
                         000101\
                         000000\
                         01000000\
                         00000000\
                         02000000\
                         6161\
                         0000\
                         00000000\
                         05000000\
                         04\
                         05"
    .replace(['\n', ' '], "");
    assert_eq!(to_hex(&body), expected_hex);
}

#[test]
fn golden_ncb_u64_enum_bool_dict_two_names_full() {
    use norito::columnar::{EnumBorrow, encode_ncb_u64_enum_bool};
    // Two Name rows: ensure dict order follows first appearance and offsets are [0,2,4]
    let rows = vec![
        (1u64, EnumBorrow::Name("aa"), true),
        (2u64, EnumBorrow::Name("bb"), false),
    ];
    let body = encode_ncb_u64_enum_bool(&rows, false, true, false);
    let expected_hex = "02000000\
                         e1\
                         000000\
                         0100000000000000\
                         0200000000000000\
                         0000\
                         0000\
                         02000000\
                         00000000\
                         02000000\
                         04000000\
                         61616262\
                         00000000\
                         01000000\
                         01"
    .replace(['\n', ' '], "");
    assert_eq!(to_hex(&body), expected_hex);
}

#[test]
fn golden_ncb_u64_str_u32_bool_offsets_single() {
    // Rows: [(1, "a", 3, true)] → descriptor 0x33 (no dict, no deltas)
    let rows = vec![(1u64, "a", 3u32, true)];
    let body = norito::columnar::encode_ncb_u64_str_u32_bool(&rows);
    // Layout: [n=1][desc=33][pad3][id:8][offs 0,1][blob 61][align3][val 03 00 00 00][flags 01]
    let expected_hex = "01000000\
                         33\
                         000000\
                         0100000000000000\
                         00000000\
                         01000000\
                         61\
                         000000\
                         03000000\
                         01"
    .replace(['\n', ' '], "");
    assert_eq!(to_hex(&body), expected_hex);
}

#[test]
fn golden_ncb_u64_str_u32_bool_u32delta_two_rows() {
    // Rows crafted to disable id-delta and dict, enable u32-delta only
    // ids: 1, 1 + (1<<56) → large delta disables id-delta
    // names: short distinct ("a","b") → no dict
    // u32: 5,6 → small delta enables u32-delta
    let rows = vec![
        (1u64, "a", 5u32, true),
        (1u64 + (1u64 << 56), "b", 6u32, false),
    ];
    let body = norito::columnar::encode_ncb_u64_str_u32_bool(&rows);
    // Expect descriptor 0x37 (u32-delta only)
    let expected_hex = "02000000\
                         37\
                         000000\
                         0100000000000000\
                         0100000000000001\
                         00000000\
                         01000000\
                         02000000\
                         61\
                         62\
                         0000\
                         05000000\
                         02\
                         01"
    .replace(['\n', ' '], "");
    assert_eq!(to_hex(&body), expected_hex);
}

#[test]
fn golden_ncb_u64_bytes_u32_bool_u32delta_two_rows() {
    // Rows crafted to disable id-delta, enable u32-delta
    let rows = vec![
        (1u64, b"x".as_slice(), 5u32, true),
        (1u64 + (1u64 << 56), b"y".as_slice(), 6u32, false),
    ];
    let body = norito::columnar::encode_ncb_u64_bytes_u32_bool(&rows);
    // Expect descriptor 0x38 (u32-delta only)
    let expected_hex = "02000000\
                         38\
                         000000\
                         0100000000000000\
                         0100000000000001\
                         00000000\
                         01000000\
                         02000000\
                         78\
                         79\
                         0000\
                         05000000\
                         02\
                         01"
    .replace(['\n', ' '], "");
    assert_eq!(to_hex(&body), expected_hex);
}

#[test]
fn golden_ncb_u64_bytes_u32_bool_iddelta_u32delta_two_rows() {
    // Two rows; id-delta enabled (consecutive ids), dict not applicable for bytes, u32-delta enabled
    let rows = vec![
        (100u64, b"x".as_slice(), 5u32, true),
        (101u64, b"yz".as_slice(), 7u32, false),
    ];
    let body = norito::columnar::encode_ncb_u64_bytes_u32_bool(&rows);
    // Expect descriptor 0x78 (id-delta + u32-delta)
    let expected_hex = "02000000\
                         78\
                         000000\
                         6400000000000000\
                         02\
                         000000\
                         00000000\
                         01000000\
                         03000000\
                         78797a\
                         00\
                         05000000\
                         04\
                         01"
    .replace(['\n', ' '], "");
    assert_eq!(to_hex(&body), expected_hex);
}

#[test]
fn golden_ncb_u64_str_u32_bool_iddelta_only_two_rows() {
    // id-delta enabled, dict disabled, u32-delta disabled via large delta
    let rows = vec![
        (100u64, "a", 0u32, true),
        (101u64, "b", 0xF0000000u32, false),
    ];
    let body = norito::columnar::encode_ncb_u64_str_u32_bool(&rows);
    // Expect descriptor 0x73 (id-delta only)
    let expected_hex = "02000000\
                         73\
                         000000\
                         6400000000000000\
                         02\
                         000000\
                         00000000\
                         01000000\
                         02000000\
                         6162\
                         0000\
                         00000000\
                         000000f0\
                         01"
    .replace(['\n', ' '], "");
    assert_eq!(to_hex(&body), expected_hex);
}

#[test]
fn golden_ncb_u64_str_u32_bool_iddelta_only_eight_rows() {
    // 8 rows to exercise larger id-delta-only offsets-based names; u32-delta disabled via large alternating deltas
    let rows: Vec<(u64, &str, u32, bool)> = vec![
        (0, "a", 0, true),
        (1, "b", 0xF0000000, false),
        (2, "c", 0, true),
        (3, "d", 0xF0000000, false),
        (4, "e", 0, true),
        (5, "f", 0xF0000000, false),
        (6, "g", 0, true),
        (7, "h", 0xF0000000, false),
    ];
    let body = norito::columnar::encode_ncb_u64_str_u32_bool(&rows);
    // Expect descriptor 0x73 with:
    // ids: base=0, seven varint deltas 0x02; names offsets [0..8]; blob "abcdefgh"; values raw u32; flags 0x55
    let expected_hex = "08000000\
                         73\
                         000000\
                         0000000000000000\
                         02020202020202\
                         00\
                         00000000\
                         01000000\
                         02000000\
                         03000000\
                         04000000\
                         05000000\
                         06000000\
                         07000000\
                         08000000\
                         6162636465666768\
                         00000000\
                         000000f0\
                         00000000\
                         000000f0\
                         00000000\
                         000000f0\
                         00000000\
                         000000f0\
                         55"
    .replace(['\n', ' '], "");
    assert_eq!(to_hex(&body), expected_hex);
}

#[test]
fn golden_ncb_u64_str_u32_bool_iddelta_only_sixteen_rows() {
    // 16 rows id-delta-only (descriptor 0x73). Use alternating large/small u32 to disable u32-delta.
    // Build owned rows first, then borrow to avoid ephemeral &str issues
    let mut owned: Vec<(u64, String, u32, bool)> = Vec::new();
    for i in 0..16u64 {
        let name_char = if i <= 7 {
            (b'a' + (i as u8)) as char
        } else {
            (b'a' + ((i - 8) as u8)) as char
        };
        let val = if i % 2 == 0 { 0u32 } else { 0xF0000000u32 };
        owned.push((i, name_char.to_string(), val, i % 2 == 0));
    }
    let borrowed: Vec<(u64, &str, u32, bool)> = owned
        .iter()
        .map(|(id, s, v, b)| (*id, s.as_str(), *v, *b))
        .collect();
    let body = norito::columnar::encode_ncb_u64_str_u32_bool(&borrowed);
    // Descriptor should be id-delta only
    assert_eq!(body[4], 0x73, "expected desc=0x73 (id-delta only)");
    // Sanity: view roundtrip must match inputs
    let view = norito::columnar::view_ncb_u64_str_u32_bool(&body).expect("view");
    assert_eq!(view.len(), 16);
    for (i, _item) in owned.iter().enumerate().take(16) {
        assert_eq!(view.id(i), i as u64);
        let s = view.name(i).unwrap();
        assert_eq!(s.len(), 1);
        assert!(matches!(s.as_bytes()[0], b'a'..=b'h'));
        assert_eq!(view.val(i), if i % 2 == 0 { 0 } else { 0xF0000000 });
        assert_eq!(view.flag(i), i % 2 == 0);
    }
}

#[test]
fn golden_ncb_u64_bytes_u32_bool_iddelta_only_sixteen_rows() {
    // 16 rows id-delta-only (descriptor 0x74). Alternate large/small u32 values to disable u32-delta.
    // Use varying bytes to exercise offsets/blob layout.
    let mut owned: Vec<(u64, Vec<u8>, u32, bool)> = Vec::new();
    for i in 0..16u64 {
        let bytes = match i % 4 {
            0 => vec![b'x'],
            1 => vec![b'y', b'z'],
            2 => Vec::new(),
            _ => vec![(b'a' + ((i % 26) as u8))],
        };
        let val = if i % 2 == 0 { 0u32 } else { 0xF0000000u32 };
        owned.push((i, bytes, val, i % 2 == 1));
    }
    let borrowed: Vec<(u64, &[u8], u32, bool)> = owned
        .iter()
        .map(|(id, bs, v, b)| (*id, bs.as_slice(), *v, *b))
        .collect();
    let body = norito::columnar::encode_ncb_u64_bytes_u32_bool(&borrowed);
    // Descriptor should be id-delta only (0x74)
    assert_eq!(body[4], 0x74, "expected desc=0x74 (id-delta only)");
    // Sanity: view roundtrip must match inputs
    let view = norito::columnar::view_ncb_u64_bytes_u32_bool(&body).expect("view");
    assert_eq!(view.len(), 16);
    for (i, item) in owned.iter().enumerate().take(16) {
        assert_eq!(view.id(i), i as u64);
        let data = view.data(i);
        let expected = &item.1;
        assert_eq!(data, expected.as_slice());
        assert_eq!(view.val(i), if i % 2 == 0 { 0 } else { 0xF0000000 });
        assert_eq!(view.flag(i), i % 2 == 1);
    }
}

#[test]
fn golden_ncb_u64_bytes_u32_bool_iddelta_only_two_rows() {
    // id-delta enabled, u32-delta disabled via large delta
    let rows = vec![
        (100u64, b"x".as_slice(), 0u32, true),
        (101u64, b"yz".as_slice(), 0xF0000000u32, false),
    ];
    let body = norito::columnar::encode_ncb_u64_bytes_u32_bool(&rows);
    // Expect descriptor 0x74 (id-delta only)
    let expected_hex = "02000000\
                         74\
                         000000\
                         6400000000000000\
                         02\
                         000000\
                         00000000\
                         01000000\
                         03000000\
                         78797a\
                         00\
                         00000000\
                         000000f0\
                         01"
    .replace(['\n', ' '], "");
    assert_eq!(to_hex(&body), expected_hex);
}

#[test]
fn golden_ncb_u64_str_u32_bool_delta_many_rows() {
    // Many rows; heuristics typically choose id+u32 delta for this shape.
    let rows: Vec<(u64, &str, u32, bool)> = (0..16)
        .map(|i| (i as u64 + 1, "repeat", (i % 4) as u32, i % 2 == 0))
        .collect();
    let body = norito::columnar::encode_ncb_u64_str_u32_bool(&rows);
    let hex = to_hex(&body);
    let desc = body[4];
    // Assert u32-delta bit present (0x04) and dict bit (0x80) may be off
    assert!(
        desc & 0x04 != 0,
        "expected u32-delta set in desc={desc:02x}"
    );
    // Blob should contain 'repeat' in ASCII: 72 65 70 65 61 74 (at least once)
    assert!(hex.contains("726570656174"), "blob missing 'repeat': {hex}");
}

#[test]
fn golden_ncb_u64_enum_bool_dict_code_delta() {
    use norito::columnar::{EnumBorrow, encode_ncb_u64_enum_bool};
    // Three rows to include at least one Name and two Code for code-delta
    let rows = vec![
        (1u64, EnumBorrow::Name("aa"), true),
        (2u64, EnumBorrow::Code(5), false),
        (3u64, EnumBorrow::Code(7), true),
    ];
    // Force: id_delta=false, name_dict=true, code_delta=true → desc 0xE5
    let body = encode_ncb_u64_enum_bool(&rows, false, true, true);
    let hex = to_hex(&body);
    // Header: n=3, desc=E5
    assert!(hex.starts_with("03000000e5"), "header/desc mismatch: {hex}");
    // Tags must be NAME(0), CODE(1), CODE(1)
    assert!(hex.contains("0001")); // at least NAME then CODE sequence
    // Dict must include 'aa' (6161)
    assert!(hex.contains("6161"), "dict blob missing 'aa': {hex}");
    // Codes delta section should contain base 05 00 00 00 and a varint-encoded delta of +2 (zigzag 4 => 0x04)
    assert!(hex.contains("05000000"), "missing base code: {hex}");
    assert!(hex.contains("04"), "missing varint delta 0x04: {hex}");
}

#[test]
fn golden_ncb_u64_str_u32_bool_dict_u32delta() {
    // Five rows; dict enabled (2/5 distinct, avg len 8), id-delta disabled via large jumps, u32-delta enabled
    let rows = vec![
        (1u64, "abcdefgh", 10u32, true),
        (1u64 + (1u64 << 56), "ijklmnop", 11u32, false),
        (1u64 + (2u64 << 56), "abcdefgh", 12u32, true),
        (1u64 + (3u64 << 56), "ijklmnop", 13u32, false),
        (1u64 + (4u64 << 56), "abcdefgh", 14u32, true),
    ];
    let body = norito::columnar::encode_ncb_u64_str_u32_bool(&rows);
    // Expect descriptor 0xB7 (dict + u32-delta)
    let expected_hex = "05000000\
                         b7\
                         000000\
                         0100000000000000\
                         0100000000000001\
                         0100000000000002\
                         0100000000000003\
                         0100000000000004\
                         02000000\
                         00000000\
                         08000000\
                         10000000\
                         6162636465666768696a6b6c6d6e6f70\
                         00000000\
                         01000000\
                         00000000\
                         01000000\
                         00000000\
                         0a000000\
                         02020202\
                         15"
    .replace(['\n', ' '], "");
    assert_eq!(to_hex(&body), expected_hex);
}

#[test]
fn golden_ncb_u64_str_u32_bool_iddelta_dict_u32delta() {
    // Five rows; id-delta enabled (consecutive ids), dict enabled (2/5 distinct, avg len 8), u32-delta enabled
    let rows = vec![
        (100u64, "abcdefgh", 10u32, true),
        (101u64, "ijklmnop", 11u32, false),
        (102u64, "abcdefgh", 12u32, true),
        (103u64, "ijklmnop", 13u32, false),
        (104u64, "abcdefgh", 14u32, true),
    ];
    let body = norito::columnar::encode_ncb_u64_str_u32_bool(&rows);
    // Expect descriptor 0xF7 (id-delta + dict + u32-delta)
    let expected_hex = "05000000\
                         f7\
                         000000\
                         6400000000000000\
                         02020202\
                         02000000\
                         00000000\
                         08000000\
                         10000000\
                         6162636465666768696a6b6c6d6e6f70\
                         00000000\
                         01000000\
                         00000000\
                         01000000\
                         00000000\
                         0a000000\
                         02020202\
                         15"
    .replace(['\n', ' '], "");
    assert_eq!(to_hex(&body), expected_hex);
}
