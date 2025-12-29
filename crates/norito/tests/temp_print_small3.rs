//! TEMP: print hex for additional offsets+code-delta small datasets.
#![cfg(feature = "json")]

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
fn print_offsets_code_delta_variant1() {
    use ncb::EnumBorrow;
    // Mix: Code, Name, Code (0 delta), Code (+5), Name
    let rows: Vec<(u64, EnumBorrow<'_>, bool)> = vec![
        (1, EnumBorrow::Code(100), false),
        (2, EnumBorrow::Name("x"), true),
        (3, EnumBorrow::Code(100), false),
        (4, EnumBorrow::Code(105), true),
        (5, EnumBorrow::Name("y"), false),
    ];
    let bytes = ncb::encode_ncb_u64_enum_bool(&rows, false, false, true);
    println!("OFFSETS_CODE_DELTA_VARIANT1:{}", to_hex(&bytes));
}

#[test]
fn print_offsets_code_delta_variant2() {
    use ncb::EnumBorrow;
    // Mix: Name, Name, Code(5), Name, Code(6)
    let rows: Vec<(u64, EnumBorrow<'_>, bool)> = vec![
        (10, EnumBorrow::Name("aa"), true),
        (11, EnumBorrow::Name("bb"), false),
        (12, EnumBorrow::Code(5), true),
        (13, EnumBorrow::Name("ccc"), false),
        (14, EnumBorrow::Code(6), true),
    ];
    let bytes = ncb::encode_ncb_u64_enum_bool(&rows, false, false, true);
    println!("OFFSETS_CODE_DELTA_VARIANT2:{}", to_hex(&bytes));
}
