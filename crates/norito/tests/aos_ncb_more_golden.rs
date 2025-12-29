//! Additional golden vectors for Adaptive AoS/NCB formats
//!
//! Covers:
//! - AoS small payloads for (u64, bytes, bool) and (u64, str, u32, bool)
//! - NCB payloads for (u64, str, bool), (u64, Option<str>, bool), (u64, Option<u32>, bool)
//! - NCB payload for enum(Name|Code) column without deltas/dicts
//!
//! The expected bytes are computed from the documented layouts and should be
//! stable across architectures. Compact-len is enabled by default for varints.

use norito::columnar as ncb;

fn to_hex(bs: &[u8]) -> String {
    let mut s = String::with_capacity(bs.len() * 2);
    for b in bs {
        use core::fmt::Write as _;
        let _ = write!(&mut s, "{b:02x}");
    }
    s
}

#[test]
fn aos_bytes_bool_small_golden() {
    // Rows: (1, b"abc", true), (2, b"\x00\xff", false)
    let rows: Vec<(u64, &[u8], bool)> = vec![(1, b"abc", true), (2, b"\x00\xff", false)];
    let bytes = ncb::encode_rows_u64_bytes_bool_adaptive(&rows);
    // Sequential layout encodes [tag=0x01][len:u32][rows...], each row prefixed with length.
    let expected = "01020000005400000001000000000000000200000000000000030000000500000061626300ff01";
    assert_eq!(to_hex(&bytes), expected);
}

#[test]
fn aos_str_u32_bool_small_golden() {
    // Rows: (1, "x", 7, true), (2, "yy", 9, false)
    let rows: Vec<(u64, &str, u32, bool)> = vec![(1, "x", 7, true), (2, "yy", 9, false)];
    let bytes = ncb::encode_rows_u64_str_u32_bool_adaptive(&rows);
    // Sequential layout encodes [tag=0x01][len:u32][rows...], each field prefixed with length.
    let expected =
        "01020000007700000001000000000000000200000000000000010000000300000078797900070000000401";
    assert_eq!(to_hex(&bytes), expected);
}

#[test]
fn ncb_str_bool_golden_small() {
    // NCB (u64, str, bool) with two rows (no delta, no dict)
    let rows: Vec<(u64, &str, bool)> = vec![(1, "alice", true), (2, "bob", false)];
    let bytes = ncb::encode_ncb_u64_str_bool(&rows);
    // Layout: [n:2][desc:0x13][pad→8][ids][offs(3)][blob][flags]
    // Delta-coded ids are enabled for this dataset; descriptor 0x53 and base+varint deltas
    let expected =
        "0200000053000000010000000000000002000000000000000500000008000000616c696365626f6201";
    assert_eq!(to_hex(&bytes), expected);
}

#[test]
fn ncb_str_bool_golden_medium() {
    // NCB (u64, str, bool) with eight rows, id-delta path, 1-byte names
    let rows: Vec<(u64, &str, bool)> = vec![
        (0, "a", true),
        (1, "b", false),
        (2, "c", true),
        (3, "d", false),
        (4, "e", true),
        (5, "f", false),
        (6, "g", true),
        (7, "h", false),
    ];
    let bytes = ncb::encode_ncb_u64_str_bool(&rows);
    // Layout: [n:8][desc:0x53][pad→8][base id=0][7×varint 0x02]
    //         [pad→4][offs 0..8][blob "abcdefgh"][flags 0x55]
    let expected = "080000005300000000000000000000000202020202020200000000000100000002000000030000000400000005000000060000000700000008000000616263646566676855";
    assert_eq!(to_hex(&bytes), expected);
}

#[test]
fn ncb_str_bool_dict_golden_small() {
    // Force dictionary encoding for repeated names
    let rows: Vec<(u64, &str, bool)> = vec![
        (1, "ab", true),
        (2, "cd", false),
        (3, "ab", false),
        (4, "cd", true),
    ];
    let bytes = ncb::encode_ncb_u64_str_bool_force_dict(&rows);
    // Layout: [n:4][desc:0x93][pad→8][ids]
    //         [pad→4][dict_len=2][offs 0,2,4][blob "abcd"]
    //         [pad→4][codes per row 0,1,0,1][flags 0x09]
    let expected = "0400000093000000010000000000000002000000000000000300000000000000040000000000000002000000000000000200000004000000616263640000000001000000000000000100000009";
    assert_eq!(to_hex(&bytes), expected);
}

#[test]
fn ncb_enum_bool_dict_code_delta_golden() {
    // Enum(Name|Code) with dict-coded names and delta-coded codes
    use ncb::EnumBorrow;
    let rows: Vec<(u64, EnumBorrow<'_>, bool)> = vec![
        (10, EnumBorrow::Name("x"), false),
        (12, EnumBorrow::Name("y"), true),
        (14, EnumBorrow::Code(7), false),
        (15, EnumBorrow::Code(8), true),
    ];
    let bytes = ncb::encode_ncb_u64_enum_bool(&rows, false, true, true);
    // Layout: [n:4][desc:0xE5][pad→8][ids]
    //         [tags 0,0,1,1][pad→4]
    //         names dict: [len=2][offs 0,1,2][blob "xy"][pad→4][per-Name codes 0,1]
    //         codes subcol (delta): [base 7][varint zigzag(1) = 0x02]
    //         flags 0x0A
    let expected = "04000000e50000000a000000000000000c000000000000000e000000000000000f00000000000000000001010200000000000000010000000200000078790000000000000100000007000000020a";
    assert_eq!(to_hex(&bytes), expected);
}

#[test]
fn ncb_opt_str_bool_golden_small() {
    // (u64, Option<str>, bool): [1, Some("a"), false], [2, None, true], [3, Some("bc"), false]
    let rows: Vec<(u64, Option<&str>, bool)> = vec![
        (1, Some("a"), false),
        (2, None, true),
        (3, Some("bc"), false),
    ];
    let bytes = ncb::encode_ncb_u64_optstr_bool(&rows);
    // Layout: [n:3][desc:0x1B][pad→8][ids]
    //         [bitset 0b101][pad→4][offs(3) 0,1,3][blob "abc"] [flags 0b010]
    // Delta-coded ids (desc 0x5b), followed by presence bitset 0b101 and offsets/blob
    let expected = "030000005b000000010000000000000002020500000000000000010000000300000061626302";
    assert_eq!(to_hex(&bytes), expected);
}

#[test]
fn ncb_opt_u32_bool_golden_small() {
    // (u64, Option<u32>, bool): [1, Some(7), false], [2, None, true], [3, Some(9), false]
    let rows: Vec<(u64, Option<u32>, bool)> =
        vec![(1, Some(7), false), (2, None, true), (3, Some(9), false)];
    let bytes = ncb::encode_ncb_u64_optu32_bool(&rows);
    // Layout: [n:3][desc:0x1C][pad→8][ids]
    //         [bitset 0b101][pad→4][u32 present {7,9}] [flags 0b010]
    // Delta-coded ids (desc 0x5c), presence bitset 0b101 then dense u32 values {7,9}
    let expected = "030000005c0000000100000000000000020205000000070000000900000002";
    assert_eq!(to_hex(&bytes), expected);
}

#[test]
fn ncb_enum_bool_golden_small() {
    // (u64, enum(Name|Code), bool): [1, Name("a"), false], [2, Code(7), true], [3, Name("bc"), false]
    // Force: no id delta, no name dict, no code delta for a stable descriptor/shape.
    let rows: Vec<(u64, ncb::EnumBorrow<'_>, bool)> = vec![
        (1, ncb::EnumBorrow::Name("a"), false),
        (2, ncb::EnumBorrow::Code(7), true),
        (3, ncb::EnumBorrow::Name("bc"), false),
    ];
    let bytes = ncb::encode_ncb_u64_enum_bool(&rows, false, false, false);
    // Layout: [n:3][desc:0x61][pad→8][ids]
    //         [tags 0,1,0][pad→4]
    //         names: [offs 0,1,3][blob "abc"]
    //         [pad→4]
    //         codes: [7]
    //         flags: 0b010
    // Descriptor 0x61, tags [0,1,0], names offsets [0,1,3], blob "abc", code[7], flags 0b010
    // Note: a 4-byte align is inserted after tags and after blob.
    let expected = "030000006100000001000000000000000200000000000000030000000000000000010000000000000100000003000000616263000700000002";
    assert_eq!(to_hex(&bytes), expected);
}

#[test]
fn ncb_enum_offsets_iddelta_golden_small() {
    // NCB enum with offsets-based names and id-delta enabled
    use ncb::EnumBorrow;
    let rows: Vec<(u64, EnumBorrow<'_>, bool)> = vec![
        (100, EnumBorrow::Name("aa"), true),
        (101, EnumBorrow::Code(7), false),
        (105, EnumBorrow::Name("bbb"), true),
        (112, EnumBorrow::Code(9), false),
    ];
    let bytes = ncb::encode_ncb_u64_enum_bool(&rows, true, false, false);
    // Layout: [n:4][desc:0x63][pad→8][base=100][varint deltas {+1,+4,+7}]
    //         [tags 0,1,0,1][pad→4]
    //         names offsets [0,2,5] + blob "aabbb"
    //         [pad→4]
    //         codes [7,9]
    //         flags 0b0101
    let expected = "0400000063000000640000000000000002080e00010001000000000002000000050000006161626262000000070000000900000005";
    assert_eq!(to_hex(&bytes), expected);
}

#[test]
fn aos_enum_small_golden() {
    // Adaptive AoS for enum rows: small input should select AoS
    use ncb::EnumBorrow;
    let rows: Vec<(u64, EnumBorrow<'_>, bool)> = vec![
        (1, EnumBorrow::Name("x"), false),
        (2, EnumBorrow::Code(7), true),
    ];
    let bytes = ncb::encode_rows_u64_enum_bool_adaptive(&rows);
    // Sequential layout encodes [tag=0x01][len:u32][rows...], enum payloads include per-field lengths.
    let expected = "0102000000630000000100000000000000020001000000000001000000780000000700000002";
    assert_eq!(to_hex(&bytes), expected);
}

fn read_hex_fixture(rel: &str) -> Vec<u8> {
    use std::path::Path;
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(rel);
    let hex = std::fs::read_to_string(&path).expect("read fixture");
    let hex = hex.trim();
    let mut out = Vec::with_capacity(hex.len() / 2);
    let mut i = 0;
    while i < hex.len() {
        let b = u8::from_str_radix(&hex[i..i + 2], 16).expect("hex");
        out.push(b);
        i += 2;
    }
    out
}

#[test]
fn ncb_enum_offsets_code_delta_small_fixture() {
    use ncb::EnumBorrow;
    let rows: Vec<(u64, EnumBorrow<'_>, bool)> = vec![
        (10, EnumBorrow::Name("aa"), true),
        (12, EnumBorrow::Code(7), false),
        (14, EnumBorrow::Code(9), true),
        (15, EnumBorrow::Name("bbb"), false),
    ];
    let bytes = ncb::encode_ncb_u64_enum_bool(&rows, false, false, true);
    let fix = read_hex_fixture("tests/data/enum_offsets_code_delta_small.hex");
    assert_eq!(bytes, fix, "offsets+code-delta small bytes mismatch");
}

#[test]
fn ncb_enum_dict_id_code_delta_small_fixture() {
    use ncb::EnumBorrow;
    let rows: Vec<(u64, EnumBorrow<'_>, bool)> = vec![
        (100, EnumBorrow::Name("ab"), true),
        (109, EnumBorrow::Name("cd"), false),
        (110, EnumBorrow::Code(1), true),
        (119, EnumBorrow::Name("ab"), false),
        (128, EnumBorrow::Code(3), true),
    ];
    let bytes = ncb::encode_ncb_u64_enum_bool(&rows, true, true, true);
    let fix = read_hex_fixture("tests/data/enum_dict_id_code_delta_small.hex");
    assert_eq!(bytes, fix, "dict+id+code-delta small bytes mismatch");
}

#[test]
fn ncb_enum_offsets_id_delta_small_fixture() {
    use ncb::EnumBorrow;
    let rows: Vec<(u64, EnumBorrow<'_>, bool)> = vec![
        (100, EnumBorrow::Name("aa"), true),
        (101, EnumBorrow::Code(7), false),
        (105, EnumBorrow::Name("bbb"), true),
        (112, EnumBorrow::Code(9), false),
    ];
    let bytes = ncb::encode_ncb_u64_enum_bool(&rows, true, false, false);
    let fix = read_hex_fixture("tests/data/enum_offsets_id_delta_small.hex");
    assert_eq!(bytes, fix, "offsets+id-delta small bytes mismatch");
}

#[test]
fn ncb_enum_dict_code_delta_small_fixture() {
    use ncb::EnumBorrow;
    let rows: Vec<(u64, EnumBorrow<'_>, bool)> = vec![
        (10, EnumBorrow::Name("x"), false),
        (12, EnumBorrow::Name("y"), true),
        (14, EnumBorrow::Code(7), false),
        (15, EnumBorrow::Code(8), true),
    ];
    let bytes = ncb::encode_ncb_u64_enum_bool(&rows, false, true, true);
    let fix = read_hex_fixture("tests/data/enum_dict_code_delta_small.hex");
    assert_eq!(bytes, fix, "dict+code-delta small bytes mismatch");
}

#[test]
fn ncb_enum_offsets_code_delta_variant1_fixture() {
    use ncb::EnumBorrow;
    let rows: Vec<(u64, EnumBorrow<'_>, bool)> = vec![
        (1, EnumBorrow::Code(100), false),
        (2, EnumBorrow::Name("x"), true),
        (3, EnumBorrow::Code(100), false),
        (4, EnumBorrow::Code(105), true),
        (5, EnumBorrow::Name("y"), false),
    ];
    let bytes = ncb::encode_ncb_u64_enum_bool(&rows, false, false, true);
    let fix = read_hex_fixture("tests/data/enum_offsets_code_delta_variant1.hex");
    assert_eq!(bytes, fix, "offsets+code-delta variant1 bytes mismatch");
}

#[test]
fn ncb_enum_offsets_code_delta_variant2_fixture() {
    use ncb::EnumBorrow;
    let rows: Vec<(u64, EnumBorrow<'_>, bool)> = vec![
        (10, EnumBorrow::Name("aa"), true),
        (11, EnumBorrow::Name("bb"), false),
        (12, EnumBorrow::Code(5), true),
        (13, EnumBorrow::Name("ccc"), false),
        (14, EnumBorrow::Code(6), true),
    ];
    let bytes = ncb::encode_ncb_u64_enum_bool(&rows, false, false, true);
    let fix = read_hex_fixture("tests/data/enum_offsets_code_delta_variant2.hex");
    assert_eq!(bytes, fix, "offsets+code-delta variant2 bytes mismatch");
}

#[test]
fn ncb_enum_offsets_id_code_delta_small_fixture() {
    use ncb::EnumBorrow;
    let rows: Vec<(u64, EnumBorrow<'_>, bool)> = vec![
        (10, EnumBorrow::Name("a"), false),
        (12, EnumBorrow::Code(100), true),
        (15, EnumBorrow::Code(102), false),
        (18, EnumBorrow::Name("bbb"), true),
        (21, EnumBorrow::Code(101), false),
    ];
    let bytes = ncb::encode_ncb_u64_enum_bool(&rows, true, false, true);
    let fix = read_hex_fixture("tests/data/enum_offsets_id_code_delta_small.hex");
    assert_eq!(bytes, fix, "offsets+id+code-delta small bytes mismatch");
}

#[test]
fn ncb_enum_dict_id_delta_small_fixture() {
    use ncb::EnumBorrow;
    let rows: Vec<(u64, EnumBorrow<'_>, bool)> = vec![
        (100, EnumBorrow::Name("ab"), true),
        (109, EnumBorrow::Name("cd"), false),
        (110, EnumBorrow::Name("ab"), true),
        (119, EnumBorrow::Name("ef"), false),
    ];
    let bytes = ncb::encode_ncb_u64_enum_bool(&rows, true, true, false);
    let fix = read_hex_fixture("tests/data/enum_dict_id_delta_small.hex");
    assert_eq!(bytes, fix, "dict+id-delta small bytes mismatch");
}

#[test]
fn ncb_enum_offsets_alternating_aa_300_fixture() {
    use ncb::EnumBorrow;
    let mut rows: Vec<(u64, EnumBorrow<'_>, bool)> = Vec::new();
    let mut id = 10u64;
    for i in 0..16u64 {
        if i % 2 == 0 {
            rows.push((id, EnumBorrow::Name("aa"), true));
        } else {
            rows.push((id, EnumBorrow::Code(300), true));
        }
        id += 1;
    }
    let bytes = ncb::encode_ncb_u64_enum_bool(&rows, true, false, true);
    let fix = read_hex_fixture("tests/data/enum_offsets_alternating_aa_300.hex");
    assert_eq!(bytes, fix, "offsets alternating aa/300 bytes mismatch");
}

#[test]
fn ncb_enum_dict_alternating_zz_77_fixture() {
    use ncb::EnumBorrow;
    let mut rows: Vec<(u64, EnumBorrow<'_>, bool)> = Vec::new();
    let mut id = 1u64;
    for i in 0..10u64 {
        if i % 2 == 0 {
            rows.push((id, EnumBorrow::Name("zz"), true));
        } else {
            rows.push((id, EnumBorrow::Code(77), true));
        }
        id += 1;
    }
    let bytes = ncb::encode_ncb_u64_enum_bool(&rows, true, true, true);
    let fix = read_hex_fixture("tests/data/enum_dict_alternating_zz_77.hex");
    assert_eq!(bytes, fix, "dict alternating zz/77 bytes mismatch");
}
