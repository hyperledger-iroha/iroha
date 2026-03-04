//! TEMP: print hex for nested alternation window fixture.
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
fn print_offsets_nested_window() {
    use ncb::EnumBorrow;
    // Flag-true sequence contains: Name("aa"), Code(300), Code(300), Name("bb"), Code(300)
    let rows: Vec<(u64, EnumBorrow<'_>, bool)> = vec![
        (10, EnumBorrow::Code(299), false),
        (11, EnumBorrow::Name("aa"), true),
        (12, EnumBorrow::Code(300), true),
        (13, EnumBorrow::Code(300), true),
        (14, EnumBorrow::Name("bb"), true),
        (15, EnumBorrow::Code(300), true),
        (16, EnumBorrow::Code(301), false),
    ];
    let bytes = ncb::encode_ncb_u64_enum_bool(&rows, true, false, true);
    println!("OFFSETS_NESTED_WINDOW:{}", to_hex(&bytes));
}
