//! Targeted test: enum NCB with both id-delta and code-delta enabled on a modular Code pattern.

use norito::columnar as ncb;

#[test]
fn enum_combined_delta_modular_codes_roundtrip() {
    let mut rows_borrowed: Vec<(u64, ncb::EnumBorrow<'_>, bool)> = Vec::new();
    let mut rows_owned: Vec<(u64, ncb::RowEnumOwned, bool)> = Vec::new();
    for i in 0..128u64 {
        if i % 4 == 0 {
            rows_borrowed.push((i, ncb::EnumBorrow::Name("user"), i % 5 == 0));
            rows_owned.push((i, ncb::RowEnumOwned::Name("user".to_string()), i % 5 == 0));
        } else {
            let c = (1000 + (i % 16)) as u32;
            rows_borrowed.push((i, ncb::EnumBorrow::Code(c), i % 5 == 0));
            rows_owned.push((i, ncb::RowEnumOwned::Code(c), i % 5 == 0));
        }
    }
    // Force both id-delta and code-delta ON; no dict names
    let bytes = ncb::encode_ncb_u64_enum_bool(&rows_borrowed, true, false, true);
    let view = ncb::view_ncb_u64_enum_bool(&bytes).expect("view combined deltas");
    assert_eq!(view.len(), rows_owned.len());
    for (i, row) in rows_owned.iter().enumerate().take(view.len()) {
        assert_eq!(view.id(i), row.0, "id mismatch at {i}");
        assert_eq!(view.flag(i), row.2, "flag mismatch at {i}");
        match &row.1 {
            ncb::RowEnumOwned::Name(s) => {
                assert_eq!(view.tag(i), 0, "tag(name) mismatch at {i}");
                match view.payload(i).unwrap() {
                    ncb::ColEnumRef::Name(ns) => assert_eq!(ns, s.as_str(), "name mismatch at {i}"),
                    _ => panic!("expected name payload at {i}"),
                }
            }
            ncb::RowEnumOwned::Code(v) => {
                assert_eq!(view.tag(i), 1, "tag(code) mismatch at {i}");
                match view.payload(i).unwrap() {
                    ncb::ColEnumRef::Code(cv) => assert_eq!(cv, *v, "code mismatch at {i}"),
                    _ => panic!("expected code payload at {i}"),
                }
            }
        }
    }
}
