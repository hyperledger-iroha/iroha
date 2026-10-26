//! Test support for frame-identity assertions after the extraction from `iroha_core`.
//!
//! The `norito_schema` nominal names are protocol declarations that still spell the
//! original `iroha_core::zk` / `iroha_core::zk_stark` module paths, while
//! `std::any::type_name` reports this crate's Rust paths.

/// Map a recorded nominal Rust path to the path the type has in `iroha_core_zk`.
pub fn relocated_rust_path(nominal: &str) -> String {
    nominal.strip_prefix("iroha_core::zk_stark::").map_or_else(
        || {
            nominal.strip_prefix("iroha_core::zk::").map_or_else(
                || nominal.to_owned(),
                |rest| format!("iroha_core_zk::{rest}"),
            )
        },
        |rest| format!("iroha_core_zk::stark::{rest}"),
    )
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
            relocated_rust_path("iroha_core::zk::confidential_v2::ConfidentialMerklePathV2"),
            "iroha_core_zk::confidential_v2::ConfidentialMerklePathV2"
        );
        assert_eq!(
            relocated_rust_path("iroha_core::other::T"),
            "iroha_core::other::T"
        );
    }
}
