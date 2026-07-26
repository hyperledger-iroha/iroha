//! Sanity tests for documentation fixtures.

use std::convert::TryFrom;

use norito::json::{self, Value};

fn parse_hex_u64(s: &str) -> u64 {
    let s = s.trim_start_matches("0x");
    u64::from_str_radix(s, 16).expect("hex u64")
}

fn parse_hex_words(s: &str) -> Vec<u8> {
    let s = s.trim_start_matches("0x");
    assert!(s.len().is_multiple_of(2), "even-length hex");
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).expect("hex byte"))
        .collect()
}

const GOLDILOCKS_MODULUS: u64 = 0xffff_ffff_0000_0001;

fn add_mod(a: u64, b: u64) -> u64 {
    let sum = a.wrapping_add(b);
    if sum >= GOLDILOCKS_MODULUS {
        sum - GOLDILOCKS_MODULUS
    } else {
        sum
    }
}

fn mul_mod(a: u64, b: u64) -> u64 {
    let product = u128::from(a) * u128::from(b);
    let reduced = product % u128::from(GOLDILOCKS_MODULUS);
    u64::try_from(reduced).expect("product fits field element")
}

#[test]
fn lookup_grand_product_fixture_matches() {
    let contents = include_str!("fixtures/lookup_grand_product.json");
    let root: Value = json::from_str(contents).expect("parse lookup JSON");

    let gamma = parse_hex_u64(
        root.get("gamma")
            .and_then(Value::as_str)
            .expect("gamma string"),
    );
    let rows = root
        .get("rows")
        .and_then(Value::as_array)
        .expect("rows array");
    let mut z = 1u64;
    for row in rows {
        let s_perm = row.get("s_perm").and_then(Value::as_u64).expect("s_perm");
        let expected = parse_hex_u64(row.get("z_after").and_then(Value::as_str).expect("z_after"));
        if s_perm != 0 {
            let perm_hash = parse_hex_u64(
                row.get("perm_hash")
                    .and_then(Value::as_str)
                    .expect("perm_hash"),
            );
            let factor = add_mod(perm_hash, gamma);
            z = mul_mod(z, factor);
        }
        assert_eq!(z, expected, "accumulator after row");
    }
    let table_product = parse_hex_u64(
        root.get("table_product")
            .and_then(Value::as_str)
            .expect("table product"),
    );
    assert_eq!(z, table_product, "grand product matches table");
}

#[test]
fn smt_update_fixture_has_consistent_shape() {
    let contents = include_str!("fixtures/smt_update.json");
    let root: Value = json::from_str(contents).expect("parse smt JSON");
    let levels = root
        .get("levels")
        .and_then(Value::as_array)
        .expect("levels array");
    assert_eq!(levels.len(), 5, "expected 5 levels in example");
    for entry in levels {
        let path_bit = entry
            .get("path_bit")
            .and_then(Value::as_u64)
            .expect("path_bit");
        assert!(path_bit <= 1, "path bits are boolean");
        for key in ["sibling", "node_in", "node_out"] {
            let bytes = parse_hex_words(
                entry
                    .get(key)
                    .and_then(Value::as_str)
                    .unwrap_or_else(|| panic!("missing field {key}")),
            );
            assert_eq!(bytes.len(), 32, "{key} should be 32 bytes");
        }
    }
    let neighbour = root
        .get("neighbour_leaf")
        .and_then(Value::as_object)
        .expect("neighbour leaf");
    for key in ["key", "value", "hash"] {
        let bytes = parse_hex_words(
            neighbour
                .get(key)
                .and_then(Value::as_str)
                .unwrap_or_else(|| panic!("missing neighbour field {key}")),
        );
        assert_eq!(bytes.len(), 32, "neighbour {key} is 32 bytes");
    }
}
