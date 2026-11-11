//! Every Norito RPC fixture transaction round-trips in the dense layout.
//!
//! The dense layout (`norito.md`, "Dense layout") must carry each real
//! transaction shape (instructions, IVM bytecode, contract calls, batches,
//! triggers, settlements) and decode it back to the same value, while the
//! current compact layout stays byte-identical to the committed fixtures.

use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use norito::{
    codec::{decode_bare_with_layout, encode_bare_with_layout},
    core::{Header, WireLayout},
};

use crate::{account::address::ChainDiscriminantGuard, transaction::SignedTransaction};

/// The chain discriminant of a fixture authority's address prefix.
fn chain_discriminant(authority: &str) -> Option<u16> {
    if authority.starts_with("sora") {
        Some(0x02F1)
    } else if authority.starts_with("test") {
        Some(0x0171)
    } else if authority.starts_with("dev") {
        Some(0x0000)
    } else {
        authority
            .strip_prefix('n')
            .map(|rest| {
                rest.chars()
                    .take_while(char::is_ascii_digit)
                    .collect::<String>()
            })
            .and_then(|digits| digits.parse().ok())
    }
}

#[test]
fn every_fixture_transaction_round_trips_in_the_dense_layout() {
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/norito_rpc/");
    let manifest: norito::json::Value = norito::json::from_str(
        &std::fs::read_to_string(format!("{root}transaction_fixtures.manifest.json"))
            .expect("fixture manifest"),
    )
    .expect("manifest JSON");
    let (mut compact_total, mut dense_total) = (0_usize, 0_usize);
    let fixtures = manifest["fixtures"].as_array().expect("fixtures");
    assert!(!fixtures.is_empty());
    for entry in fixtures {
        let name = entry["name"].as_str().expect("name");
        let authority = entry["authority"].as_str().expect("authority");
        let _guard = chain_discriminant(authority).map(ChainDiscriminantGuard::enter);
        let signed = BASE64
            .decode(entry["signed_base64"].as_str().expect("signed_base64"))
            .expect("base64");
        let transaction: SignedTransaction =
            norito::decode_canonical(&signed).unwrap_or_else(|error| panic!("{name}: {error}"));
        // The current layout is unchanged.
        assert_eq!(
            norito::encode_canonical(&transaction).unwrap(),
            signed,
            "{name}"
        );
        let compact = encode_bare_with_layout(&transaction, WireLayout::Compact).unwrap();
        assert_eq!(compact.len() + Header::SIZE, signed.len(), "{name}");
        let dense = encode_bare_with_layout(&transaction, WireLayout::Dense).unwrap();
        // No nested instruction frame survives in the dense layout.
        assert!(!dense.windows(4).any(|window| window == b"NRT0"), "{name}");
        let decoded: SignedTransaction = decode_bare_with_layout(&dense, WireLayout::Dense)
            .unwrap_or_else(|error| panic!("{name}: {error}"));
        assert_eq!(decoded, transaction, "{name}");
        assert!(decode_bare_with_layout::<SignedTransaction>(&dense, WireLayout::Compact).is_err());
        compact_total += compact.len();
        dense_total += dense.len();
    }
    // Measured on the committed fixtures: about 47% smaller.
    assert!(
        dense_total * 100 < compact_total * 56,
        "dense {dense_total} bytes vs compact {compact_total} bytes"
    );
}
