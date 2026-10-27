//! Profile, toolchain and preliminary syntax bounds for canonical NFC scratch.

use super::*;

#[test]
fn syntax_and_ascii_demand_match_the_actual_pre_normalization_gate() {
    for raw in [
        "",
        "ascii",
        "a@b",
        "a\0b",
        "a\u{202e}b",
        "e\u{301}@b",
        "a b",
    ] {
        assert_eq!(Name::canonical_validation_scratch_bytes(raw), 0);
    }
    for raw in ["é".repeat(128), "a".repeat(256)] {
        assert!(Name::validate_str(&raw).is_err());
        assert_eq!(Name::canonical_validation_scratch_bytes(&raw), 0);
    }
    for count in 1..=127 {
        let raw = "\u{301}".repeat(count);
        Name::validate_str(&raw).unwrap();
        let bytes = Name::canonical_validation_scratch_bytes(&raw);
        assert!(bytes <= nfc_scratch::request_bytes(MAX_NAME_BYTES));
        assert_eq!(bytes, nfc_scratch::request_bytes(count));
    }
    assert_eq!(nfc_scratch::request_bytes(MAX_NAME_BYTES), 8064);
}

#[test]
fn nfc_heap_and_stable_sort_audit_is_bound_to_the_reviewed_sources() {
    let toolchain = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../rust-toolchain.toml"
    ));
    let lock = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../Cargo.lock"));
    assert!(
        toolchain
            .lines()
            .any(|line| line.trim() == "channel = \"1.99.1\""),
        "re-audit stable-sort scratch before updating the toolchain"
    );
    for package in [
        "name = \"icu_normalizer\"\nversion = \"2.4.0\"\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"\nchecksum = \"404a3b8c486f205c87fe7f23ab6894d84a4ad76d6e4671ae1981ba7def87c599\"",
        "name = \"smallvec\"\nversion = \"1.16.4\"\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"\nchecksum = \"d0d5d4b1981b8bbbb7aed204cae5726d683d5d6ac7e74ec54122ea28f315e7c8\"",
    ] {
        assert!(
            lock.split("[[package]]\n")
                .any(|entry| entry.starts_with(package)),
            "re-audit NFC allocation ownership before changing the dependency"
        );
    }
    assert!(matches!(usize::BITS, 32 | 64));
    assert!(
        MAX_NAME_BYTES * nfc_scratch::MAX_DECOMPOSITION_SCALARS * std::mem::size_of::<u32>()
            <= 4096
    );
    assert_eq!(nfc_data_sha256(), EXPECTED_NFC_DATA_SHA256);
}
