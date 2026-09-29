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
        assert!(bytes <= nfc_buffer_request_bytes(MAX_NAME_BYTES));
        assert_eq!(bytes, nfc_buffer_request_bytes(count));
    }
    assert_eq!(nfc_buffer_request_bytes(MAX_NAME_BYTES), 8064);
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
            .any(|line| line.trim() == "channel = \"1.93.1\""),
        "re-audit stable-sort scratch before updating the toolchain"
    );
    for package in [
        "name = \"icu_normalizer\"\nversion = \"2.2.0\"\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"\nchecksum = \"c56e5ee99d6e3d33bd91c5d85458b6005a22140021cc324cea84dd0e72cff3b4\"",
        "name = \"smallvec\"\nversion = \"1.15.1\"\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"\nchecksum = \"67b1b7a3b5fe4f1376887184045fcf45c69e92af734b7aaddc05fb777b6fbd03\"",
    ] {
        assert!(
            lock.split("[[package]]\n")
                .any(|entry| entry.starts_with(package)),
            "re-audit NFC allocation ownership before changing the dependency"
        );
    }
    assert!(matches!(usize::BITS, 32 | 64));
    assert!(
        MAX_NAME_BYTES * NFC_PROFILE_MAX_DECOMPOSITION_SCALARS * std::mem::size_of::<u32>() <= 4096
    );
    assert_eq!(nfc_data_sha256(), EXPECTED_NFC_DATA_SHA256);
}
