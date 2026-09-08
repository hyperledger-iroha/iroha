//! Explicit executable identity shared by Torii test and benchmark fixtures.

/// Construct a fixture identity independently of shared-library build metadata.
pub fn build_identity() -> iroha_core::release_identity::BuildIdentity {
    iroha_core::release_identity::BuildIdentity::from_compiled_parts(
        "test-executable",
        Some("1111111111111111111111111111111111111111"),
        None,
        None,
        Some("test-features"),
        Some("test-target"),
    )
    .expect("valid executable identity fixture")
}
