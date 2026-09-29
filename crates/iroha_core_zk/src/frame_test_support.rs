//! Test support for frame-identity assertions after the extraction from `iroha_core`.
//!
//! The `norito_schema` nominal names are protocol declarations that still spell the
//! original `iroha_core::zk` / `iroha_core::zk_stark` module paths, while
//! `std::any::type_name` reports this crate's Rust paths.

/// Map a recorded nominal Rust path to the path the type has in `iroha_core_zk`.
pub(crate) fn relocated_rust_path(nominal: &str) -> String {
    if let Some(rest) = nominal.strip_prefix("iroha_core::zk_stark::") {
        format!("iroha_core_zk::stark::{rest}")
    } else if let Some(rest) = nominal.strip_prefix("iroha_core::zk::") {
        format!("iroha_core_zk::{rest}")
    } else {
        nominal.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::relocated_rust_path;

    #[test]
    fn relocated_rust_path_maps_both_original_prefixes() {
        assert_eq!(
            relocated_rust_path("iroha_core::zk_stark::StarkProofV1"),
            "iroha_core_zk::stark::StarkProofV1"
        );
        assert_eq!(
            relocated_rust_path("iroha_core::zk::kagemusha_v1_state::LaneBinding"),
            "iroha_core_zk::kagemusha_v1_state::LaneBinding"
        );
        assert_eq!(
            relocated_rust_path("iroha_core::other::T"),
            "iroha_core::other::T"
        );
    }
}
