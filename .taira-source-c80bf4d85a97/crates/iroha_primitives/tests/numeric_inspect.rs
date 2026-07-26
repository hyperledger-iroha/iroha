//! Tests for inspecting the encoded representation of numeric values.

use core::fmt::Write;

use iroha_primitives::numeric::Numeric;

#[test]
fn print_numeric_encoding() {
    let value = Numeric::new(1_234_567_890, 4);
    let bytes = value.encode();

    let mut hex_bytes = String::with_capacity(bytes.len() * 2);
    for byte in &bytes {
        write!(&mut hex_bytes, "{byte:02x}").expect("writing hex byte");
    }

    println!("numeric bytes: {hex_bytes}");
}
