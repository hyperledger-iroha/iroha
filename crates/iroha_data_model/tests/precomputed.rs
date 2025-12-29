//! Tests for precomputed keywords

use iroha_data_model::PRECOMPUTED_KEYWORDS;

#[test]
fn precomputed_contains_expected_items() {
    assert!(PRECOMPUTED_KEYWORDS.contains(&"account"));
    assert!(PRECOMPUTED_KEYWORDS.contains(&"domain"));
}
