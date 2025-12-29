//! Tests for enabling/disabling u32 delta heuristics in NCB encoders.

use norito::columnar as ncb;

#[test]
fn enum_adaptive_enables_code_delta_when_beneficial() {
    use ncb::EnumBorrow;
    // Build rows with a mix of Name/Code; names short to avoid dict; codes with small deltas
    let mut rows: Vec<(u64, EnumBorrow<'_>, bool)> = Vec::new();
    for i in 0..80u64 {
        if i % 4 == 0 {
            rows.push((i, EnumBorrow::Name("x"), i % 3 == 0));
        } else {
            rows.push((i, EnumBorrow::Code(100 + (i as u32 % 8)), i % 3 == 0));
        }
    }
    let payload = ncb::encode_rows_u64_enum_bool_adaptive(&rows);
    let expected_tag = if ncb::should_use_columnar(rows.len()) {
        ncb::ADAPTIVE_ENUM_TAG_NCB
    } else {
        ncb::ADAPTIVE_ENUM_TAG_AOS
    };
    assert_eq!(payload[0], expected_tag);
    // Roundtrip via adaptive decoder
    let decoded = ncb::decode_rows_u64_enum_bool_adaptive(&payload).expect("decode");
    let expected: Vec<(u64, ncb::RowEnumOwned, bool)> = rows
        .iter()
        .map(|(id, e, flag)| {
            let eo = match e {
                ncb::EnumBorrow::Name(s) => ncb::RowEnumOwned::Name((*s).to_string()),
                ncb::EnumBorrow::Code(v) => ncb::RowEnumOwned::Code(*v),
            };
            (*id, eo, *flag)
        })
        .collect();
    assert_eq!(decoded, expected);
}
