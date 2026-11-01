//! Canonical Name C admission against the native syntax, NFC and byte bounds.

use super::*;

fn validate(input: &str) -> c_int {
    // SAFETY: the borrowed string supplies exactly the declared readable bytes.
    unsafe { connect_norito_name_validate_v1(input.as_ptr().cast(), input.len() as c_ulong) }
}

#[test]
fn canonical_unicode_names_match_native_wire_admission() {
    for input in [
        "weekly-sync",
        "caf\u{e9}",
        "\u{56f3}\u{66f8}\u{9928}",
        "a:b",
    ] {
        assert_eq!(validate(input), 0, "{input:?}");
        let name: Name = input.parse().unwrap();
        let encoded = norito::to_bytes(&name).unwrap();
        let decoded: Name = norito::decode_from_bytes(&encoded).unwrap();
        assert_eq!(decoded.as_ref(), input);
    }
}

#[test]
fn invalid_names_are_rejected_without_normalization() {
    for input in [
        "",
        "cafe\u{301}",
        "a@b",
        "a#b",
        "a$b",
        "a b",
        "a\u{a0}b",
        "a\0b",
        "a\u{7f}b",
        "a\u{2066}b",
        " name",
        "name ",
    ] {
        assert_eq!(validate(input), 1, "{input:?}");
        assert!(Name::validate_canonical(input).is_err(), "{input:?}");
    }
}

#[test]
fn name_bounds_count_utf8_bytes() {
    assert_eq!(validate(&"a".repeat(255)), 0);
    assert_eq!(validate(&"a".repeat(256)), 1);
    assert_eq!(validate(&format!("{}a", "\u{e9}".repeat(127))), 0);
    assert_eq!(validate(&"\u{e9}".repeat(128)), 1);
}

#[test]
fn name_ffi_rejects_null_utf8_and_oversize_before_dereference() {
    // SAFETY: null is explicitly rejected, valid buffers supply their lengths,
    // and out-of-range lengths must be rejected before the one-byte read.
    unsafe {
        assert_eq!(connect_norito_name_validate_v1(std::ptr::null(), 0), -1);
        for invalid in [&[0xff_u8][..], &[0xed_u8, 0xa0, 0x80][..]] {
            assert_eq!(
                connect_norito_name_validate_v1(invalid.as_ptr().cast(), invalid.len() as c_ulong),
                -2
            );
        }
        let one_byte = b"a";
        for length in [0, 256, c_ulong::MAX] {
            assert_eq!(
                connect_norito_name_validate_v1(one_byte.as_ptr().cast(), length),
                1
            );
        }
    }
}
