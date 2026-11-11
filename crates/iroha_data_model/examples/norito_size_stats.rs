//! Compare transaction sizes in the current compact layout and the dense layout.
//!
//! Reads the signed transactions in `fixtures/norito_rpc`, re-encodes each bare
//! payload (the Torii submission wire, after its version byte) in both layouts
//! and reports the totals. The dense encoding is also decoded back to check that
//! the data model round-trips.
//!
//! The dense instruction envelope carries no nested Norito frame; the report
//! flags any embedded frame that remains.
//!
//! Run with `cargo run -p iroha_data_model --example norito_size_stats`.

use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use iroha_data_model::{account::address::ChainDiscriminantGuard, transaction::SignedTransaction};
use norito::{
    codec::{decode_bare_with_layout, encode_bare_with_layout},
    core::{Header, WireLayout},
};

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

/// Embedded Norito frames (one per instruction) found in a bare payload.
fn embedded_frames(bytes: &[u8]) -> usize {
    bytes.windows(4).filter(|window| *window == b"NRT0").count()
}

fn main() {
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/norito_rpc/");
    let manifest: norito::json::Value = norito::json::from_str(
        &std::fs::read_to_string(format!("{root}transaction_fixtures.manifest.json"))
            .expect("fixture manifest"),
    )
    .expect("manifest JSON");
    let (mut count, mut compact_total, mut dense_total, mut frames) = (0, 0, 0, 0);
    let mut round_trip_failures = Vec::new();
    for entry in manifest["fixtures"].as_array().expect("fixtures") {
        let name = entry["name"].as_str().expect("name");
        let authority = entry["authority"].as_str().expect("authority");
        let _guard = chain_discriminant(authority).map(ChainDiscriminantGuard::enter);
        let signed_bytes = BASE64
            .decode(entry["signed_base64"].as_str().expect("signed_base64"))
            .expect("base64");
        let transaction: SignedTransaction = norito::decode_canonical(&signed_bytes)
            .unwrap_or_else(|error| panic!("{name}: {error}"));
        let compact = encode_bare_with_layout(&transaction, WireLayout::Compact)
            .unwrap_or_else(|error| panic!("{name}: {error}"));
        assert_eq!(
            compact.len() + Header::SIZE,
            signed_bytes.len(),
            "{name}: the compact payload is the fixture frame without its header"
        );
        let dense = encode_bare_with_layout(&transaction, WireLayout::Dense)
            .unwrap_or_else(|error| panic!("{name}: {error}"));
        match decode_bare_with_layout::<SignedTransaction>(&dense, WireLayout::Dense) {
            Ok(decoded) if decoded == transaction => {}
            Ok(_) => round_trip_failures.push(format!("{name}: decoded a different value")),
            Err(error) => round_trip_failures.push(format!("{name}: {error}")),
        }
        count += 1;
        compact_total += compact.len() + 1;
        dense_total += dense.len() + 1;
        frames += embedded_frames(&dense);
    }
    // Tenths of a percent, in integer arithmetic.
    let saved = (compact_total - dense_total) * 1000 / compact_total;
    println!("{count} signed transactions, submission wire (version byte + bare payload):");
    println!("  compact layout: {compact_total} bytes");
    println!(
        "  dense layout:   {dense_total} bytes ({}.{}% smaller)",
        saved / 10,
        saved % 10
    );
    if frames != 0 {
        println!("  {frames} embedded Norito frames remain in the dense payloads");
    }
    if round_trip_failures.is_empty() {
        println!("  every dense payload decodes back to the same transaction");
    } else {
        println!(
            "  {} dense payloads did not round-trip:",
            round_trip_failures.len()
        );
        for failure in &round_trip_failures {
            println!("    {failure}");
        }
    }
}
