//! NCB padding, bitset, and trailing byte validation tests.

use norito::columnar::{
    EnumBorrow, encode_ncb_u64_bytes_bool, encode_ncb_u64_bytes_u32_bool,
    encode_ncb_u64_enum_bool, encode_ncb_u64_optstr_bool, encode_ncb_u64_optu32_bool,
    encode_ncb_u64_str_bool, encode_ncb_u64_str_u32_bool, encode_ncb_u64_u32_bool,
    encode_opt_str_column, encode_opt_u32_column, view_ncb_u64_bytes_bool,
    view_ncb_u64_bytes_u32_bool, view_ncb_u64_enum_bool, view_ncb_u64_optstr_bool,
    view_ncb_u64_optu32_bool, view_ncb_u64_str_bool, view_ncb_u64_str_u32_bool,
    view_ncb_u64_u32_bool, view_opt_str_column, view_opt_u32_column,
};

fn with_trailing(mut body: Vec<u8>) -> Vec<u8> {
    body.push(0xAA);
    body
}

fn set_padding_bits(mut body: Vec<u8>) -> Vec<u8> {
    if let Some(last) = body.last_mut() {
        *last |= 0b1111_1110;
    }
    body
}

#[test]
fn ncb_views_reject_trailing_bytes() {
    let rows_str = vec![(1u64, "a", true)];
    let rows_bytes = vec![(1u64, b"a".as_slice(), true)];
    let rows_u32 = vec![(1u64, 7u32, true)];
    let rows_str_u32 = vec![(1u64, "a", 7u32, true)];
    let rows_bytes_u32 = vec![(1u64, b"a".as_slice(), 7u32, true)];
    let rows_optstr = vec![(1u64, Some("a"), true)];
    let rows_optu32 = vec![(1u64, Some(7u32), true)];
    let rows_enum = vec![(1u64, EnumBorrow::Name("a"), true)];

    assert!(view_ncb_u64_str_bool(&with_trailing(encode_ncb_u64_str_bool(&rows_str))).is_err());
    assert!(view_ncb_u64_bytes_bool(&with_trailing(encode_ncb_u64_bytes_bool(&rows_bytes)))
        .is_err());
    assert!(view_ncb_u64_u32_bool(&with_trailing(encode_ncb_u64_u32_bool(
        &rows_u32, false, false
    )))
    .is_err());
    assert!(view_ncb_u64_str_u32_bool(&with_trailing(encode_ncb_u64_str_u32_bool(
        &rows_str_u32
    )))
    .is_err());
    assert!(view_ncb_u64_bytes_u32_bool(&with_trailing(encode_ncb_u64_bytes_u32_bool(
        &rows_bytes_u32
    )))
    .is_err());
    assert!(view_ncb_u64_optstr_bool(&with_trailing(encode_ncb_u64_optstr_bool(
        &rows_optstr
    )))
    .is_err());
    assert!(view_ncb_u64_optu32_bool(&with_trailing(encode_ncb_u64_optu32_bool(
        &rows_optu32
    )))
    .is_err());
    assert!(view_ncb_u64_enum_bool(&with_trailing(encode_ncb_u64_enum_bool(
        &rows_enum, false, false, false
    )))
    .is_err());
}

#[test]
fn ncb_views_reject_bitset_padding_bits() {
    let rows_str = vec![(1u64, "a", true)];
    let rows_bytes = vec![(1u64, b"a".as_slice(), true)];
    let rows_u32 = vec![(1u64, 7u32, true)];
    let rows_str_u32 = vec![(1u64, "a", 7u32, true)];
    let rows_bytes_u32 = vec![(1u64, b"a".as_slice(), 7u32, true)];
    let rows_optstr = vec![(1u64, Some("a"), true)];
    let rows_optu32 = vec![(1u64, Some(7u32), true)];
    let rows_enum = vec![(1u64, EnumBorrow::Name("a"), true)];

    assert!(view_ncb_u64_str_bool(&set_padding_bits(encode_ncb_u64_str_bool(&rows_str)))
        .is_err());
    assert!(
        view_ncb_u64_bytes_bool(&set_padding_bits(encode_ncb_u64_bytes_bool(&rows_bytes)))
            .is_err()
    );
    assert!(
        view_ncb_u64_u32_bool(&set_padding_bits(encode_ncb_u64_u32_bool(
            &rows_u32, false, false
        )))
        .is_err()
    );
    assert!(
        view_ncb_u64_str_u32_bool(&set_padding_bits(encode_ncb_u64_str_u32_bool(
            &rows_str_u32
        )))
        .is_err()
    );
    assert!(
        view_ncb_u64_bytes_u32_bool(&set_padding_bits(encode_ncb_u64_bytes_u32_bool(
            &rows_bytes_u32
        )))
        .is_err()
    );
    assert!(
        view_ncb_u64_optstr_bool(&set_padding_bits(encode_ncb_u64_optstr_bool(&rows_optstr)))
            .is_err()
    );
    assert!(
        view_ncb_u64_optu32_bool(&set_padding_bits(encode_ncb_u64_optu32_bool(&rows_optu32)))
            .is_err()
    );
    assert!(view_ncb_u64_enum_bool(&set_padding_bits(encode_ncb_u64_enum_bool(
        &rows_enum, false, false, false
    )))
    .is_err());
}

#[test]
fn ncb_views_reject_nonzero_alignment_padding() {
    let rows_str = vec![(1u64, "a", true)];
    let mut body = encode_ncb_u64_str_bool(&rows_str);
    // Header is 5 bytes; ids align to 8 so bytes 5..8 must be zero.
    assert!(body.len() >= 8);
    body[5] = 0xFF;
    assert!(view_ncb_u64_str_bool(&body).is_err());
}

#[test]
fn opt_columns_reject_padding_and_trailing() {
    let (mut bytes, _) = encode_opt_str_column(&[Some("a")]);
    bytes[0] |= 0b1111_1110;
    assert!(view_opt_str_column(&bytes, 1).is_err());

    let (mut bytes, _) = encode_opt_str_column(&[Some("a")]);
    bytes[1] = 1;
    assert!(view_opt_str_column(&bytes, 1).is_err());

    let (mut bytes, _) = encode_opt_str_column(&[Some("a")]);
    bytes.push(0);
    assert!(view_opt_str_column(&bytes, 1).is_err());

    let (mut bytes, _) = encode_opt_u32_column(&[Some(7u32)]);
    bytes[0] |= 0b1111_1110;
    assert!(view_opt_u32_column(&bytes, 1).is_err());

    let (mut bytes, _) = encode_opt_u32_column(&[Some(7u32)]);
    bytes[1] = 1;
    assert!(view_opt_u32_column(&bytes, 1).is_err());

    let (mut bytes, _) = encode_opt_u32_column(&[Some(7u32)]);
    bytes.push(0);
    assert!(view_opt_u32_column(&bytes, 1).is_err());
}
