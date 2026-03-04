//! Tests for API exports

#[test]
#[allow(clippy::assertions_on_constants)]
fn transparent_flag_matches_feature() {
    #[cfg(feature = "transparent_api")]
    assert!(iroha_data_model::TRANSPARENT_API);
    #[cfg(not(feature = "transparent_api"))]
    assert!(!iroha_data_model::TRANSPARENT_API);
}

#[test]
fn account_id_reexported_at_root() {
    let _ = core::mem::size_of::<iroha_data_model::AccountId>();
}
