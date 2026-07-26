//! Deterministic tests for enum NCB with code-delta sequences.

use norito::columnar as ncb;

fn assert_enum_rows(
    view: &ncb::NcbU64EnumBoolView<'_>,
    expected: &[(u64, ncb::RowEnumOwned, bool)],
) {
    assert_eq!(view.len(), expected.len());
    for (idx, row) in expected.iter().enumerate() {
        assert_eq!(view.id(idx), row.0);
        assert_eq!(view.flag(idx), row.2);
        match &row.1 {
            ncb::RowEnumOwned::Name(value) => {
                assert_eq!(view.tag(idx), 0);
                match view.payload(idx).unwrap() {
                    ncb::ColEnumRef::Name(name) => assert_eq!(name, value.as_str()),
                    _ => panic!("expected name payload"),
                }
            }
            ncb::RowEnumOwned::Code(value) => {
                assert_eq!(view.tag(idx), 1);
                match view.payload(idx).unwrap() {
                    ncb::ColEnumRef::Code(code) => assert_eq!(code, *value),
                    _ => panic!("expected code payload"),
                }
            }
        }
    }
}

#[test]
fn enum_code_delta_alternating() {
    let name = "alpha";
    let rows = vec![
        (1, ncb::EnumBorrow::Name(name), true),
        (8, ncb::EnumBorrow::Code(42), false),
        (15, ncb::EnumBorrow::Name(name), false),
        (22, ncb::EnumBorrow::Code(43), true),
        (29, ncb::EnumBorrow::Name(name), false),
    ];
    let expected = vec![
        (1, ncb::RowEnumOwned::Name(name.to_owned()), true),
        (8, ncb::RowEnumOwned::Code(42), false),
        (15, ncb::RowEnumOwned::Name(name.to_owned()), false),
        (22, ncb::RowEnumOwned::Code(43), true),
        (29, ncb::RowEnumOwned::Name(name.to_owned()), false),
    ];
    let bytes = ncb::encode_ncb_u64_enum_bool(&rows, false, false, true);
    let mut prefixed = vec![0xEE, 0xEE, 0xEE];
    prefixed.extend_from_slice(&bytes);
    let view = ncb::view_ncb_u64_enum_bool(&prefixed[3..]).expect("view enum ncb");
    assert_enum_rows(&view, &expected);
}

#[test]
fn enum_code_delta_wrap() {
    let base = u32::MAX - 2;
    let step = 3_u32;
    let rows = vec![
        (5, ncb::EnumBorrow::Code(base.wrapping_add(step)), true),
        (
            16,
            ncb::EnumBorrow::Code(base.wrapping_add(step * 2)),
            false,
        ),
        (
            27,
            ncb::EnumBorrow::Code(base.wrapping_add(step * 3)),
            false,
        ),
    ];
    let expected = vec![
        (5, ncb::RowEnumOwned::Code(base.wrapping_add(step)), true),
        (
            16,
            ncb::RowEnumOwned::Code(base.wrapping_add(step * 2)),
            false,
        ),
        (
            27,
            ncb::RowEnumOwned::Code(base.wrapping_add(step * 3)),
            false,
        ),
    ];
    let bytes = ncb::encode_ncb_u64_enum_bool(&rows, false, false, true);
    let view = ncb::view_ncb_u64_enum_bool(&bytes).expect("view wrap");
    assert_enum_rows(&view, &expected);
}

#[test]
fn enum_code_and_id_delta() {
    let rows = vec![
        (9, ncb::EnumBorrow::Name("aa"), true),
        (11, ncb::EnumBorrow::Code(100), false),
        (13, ncb::EnumBorrow::Code(101), true),
        (15, ncb::EnumBorrow::Name("aa"), false),
        (17, ncb::EnumBorrow::Code(102), true),
    ];
    let expected = vec![
        (9, ncb::RowEnumOwned::Name("aa".to_owned()), true),
        (11, ncb::RowEnumOwned::Code(100), false),
        (13, ncb::RowEnumOwned::Code(101), true),
        (15, ncb::RowEnumOwned::Name("aa".to_owned()), false),
        (17, ncb::RowEnumOwned::Code(102), true),
    ];
    let bytes = ncb::encode_ncb_u64_enum_bool(&rows, true, false, true);
    let view = ncb::view_ncb_u64_enum_bool(&bytes).expect("view enum id+code delta");
    assert_enum_rows(&view, &expected);
}
