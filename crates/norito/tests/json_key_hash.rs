//! Compile-time, scalar parser and runtime tape key hashing must agree.
#![cfg(all(feature = "json", feature = "crc-key-hash"))]

use norito::json::{Parser, TapeWalker, key_hash_const};

// Deliberately independent bitwise CRC32C reference. Each public update starts
// and ends complemented; the intrinsic operates on the raw internal register.
fn software_reference(key: &str) -> u64 {
    let mut state = !0_u32;
    for byte in key.bytes() {
        let mut register = !state ^ u32::from(byte);
        for _ in 0..8 {
            register = (register >> 1) ^ (0x82f6_3b78 & 0_u32.wrapping_sub(register & 1));
        }
        state = !register;
    }
    let mut mixed = u64::from(state) ^ 0x9e37_79b9_7f4a_7c15;
    mixed ^= mixed >> 33;
    mixed = mixed.wrapping_mul(0xff51_afd7_ed55_8ccd);
    mixed ^= mixed >> 33;
    mixed = mixed.wrapping_mul(0xc4ce_b9fe_1a85_ec53);
    mixed ^ (mixed >> 33)
}

fn check_key(encoded_key: &str, decoded: &str, leading_spaces: usize) {
    let document = format!("{}{{{encoded_key}:1}}", " ".repeat(leading_spaces));
    let expected = software_reference(decoded);
    assert_eq!(key_hash_const(decoded), expected, "const {decoded:?}");
    let mut parser = Parser::new(&document);
    parser.skip_ws();
    parser.expect(b'{').unwrap();
    assert_eq!(
        parser.read_key_hash().unwrap(),
        expected,
        "parser {decoded:?}"
    );
    // Hashing leaves the colon unconsumed in both reader APIs.
    parser.expect(b':').unwrap();
    let mut walker = TapeWalker::new(&document);
    walker.expect_object_start().unwrap();
    assert_eq!(
        walker.read_key_hash().unwrap(),
        expected,
        "tape {decoded:?}"
    );
    walker.expect_colon().unwrap();
}

#[test]
fn key_hash_const_matches_runtime_crc32c() {
    for key in [
        "",
        "id",
        "public_key",
        "logger",
        "network",
        "consensus",
        "transport",
    ] {
        check_key(&format!("\"{key}\""), key, 0);
    }
}

#[test]
fn key_hash_runtime_matches_software_for_all_ascii_bytes() {
    for byte in 0_u8..=127 {
        let decoded = char::from(byte).to_string();
        check_key(&format!("\"\\u{byte:04x}\""), &decoded, 0);
    }
}

#[test]
fn key_hash_runtime_matches_software_for_short_tails_long_keys_and_alignment() {
    for length in (0..=65).chain([127, 128, 129, 255, 256, 257, 1023, 1024, 1025]) {
        let key: String = (0..length)
            .map(|i| char::from(b'a' + (i % 26) as u8))
            .collect();
        for offset in 0..64 {
            check_key(&format!("\"{key}\""), &key, offset);
        }
    }
}

#[test]
fn key_hash_runtime_matches_software_for_unicode_and_escapes() {
    for (encoded, decoded) in [
        (r#""public_\u006bey""#, "public_key"),
        (r#""a\"b\\c\/d""#, "a\"b\\c/d"),
        (r#""\b\f\n\r\t""#, "\u{8}\u{c}\n\r\t"),
        (r#""日本語""#, "日本語"),
        (r#""\u65e5\u672c\u8a9e""#, "日本語"),
        (r#""\uD834\uDD1E""#, "𝄞"),
    ] {
        check_key(encoded, decoded, 3);
    }
}
