//! Compile-time controls for the retired seeded BFV construction API.
//!
//! Each import must fail independently, including the root wildcard facade.
//!
//! ```compile_fail
//! use iroha_crypto::keygen_from_seed;
//! ```
//!
//! ```compile_fail
//! use iroha_crypto::fhe_bfv::keygen_from_seed;
//! ```
//!
//! ```compile_fail
//! use iroha_crypto::keygen_bounded_noise_from_seed;
//! ```
//!
//! ```compile_fail
//! use iroha_crypto::fhe_bfv::keygen_bounded_noise_from_seed;
//! ```
//!
//! ```compile_fail
//! use iroha_crypto::keygen_bounded_noise_with_relinearization_from_seed;
//! ```
//!
//! ```compile_fail
//! use iroha_crypto::fhe_bfv::keygen_bounded_noise_with_relinearization_from_seed;
//! ```
//!
//! ```compile_fail
//! use iroha_crypto::encrypt_from_seed;
//! ```
//!
//! ```compile_fail
//! use iroha_crypto::fhe_bfv::encrypt_from_seed;
//! ```
//!
//! ```compile_fail
//! use iroha_crypto::encrypt_bounded_noise_from_seed;
//! ```
//!
//! ```compile_fail
//! use iroha_crypto::fhe_bfv::encrypt_bounded_noise_from_seed;
//! ```
//!
//! ```compile_fail
//! use iroha_crypto::derive_identifier_key_material_from_seed;
//! ```
//!
//! ```compile_fail
//! use iroha_crypto::fhe_bfv::derive_identifier_key_material_from_seed;
//! ```
//!
//! ```compile_fail
//! use iroha_crypto::encrypt_identifier_from_seed;
//! ```
//!
//! ```compile_fail
//! use iroha_crypto::fhe_bfv::encrypt_identifier_from_seed;
//! ```

//!
//! ```compile_fail
//! use iroha_crypto::galois_key_from_seed;
//! ```
//!
//! ```compile_fail
//! use iroha_crypto::fhe_bfv::galois_key_from_seed;
//! ```
//!
//! ```compile_fail
//! use iroha_crypto::galois_key_bounded_noise_from_seed;
//! ```
//!
//! ```compile_fail
//! use iroha_crypto::fhe_bfv::galois_key_bounded_noise_from_seed;
//! ```
//!
//! ```compile_fail
//! use iroha_crypto::bootstrap_key_from_seed;
//! ```
//!
//! ```compile_fail
//! use iroha_crypto::fhe_bfv::bootstrap_key_from_seed;
//! ```
//!
//! ```compile_fail
//! use iroha_crypto::bootstrap_key_with_max_refresh_rounds_from_seed;
//! ```
//!
//! ```compile_fail
//! use iroha_crypto::fhe_bfv::bootstrap_key_with_max_refresh_rounds_from_seed;
//! ```
//!
//! ```compile_fail
//! use iroha_crypto::bootstrap_key_bounded_noise_from_seed;
//! ```
//!
//! ```compile_fail
//! use iroha_crypto::fhe_bfv::bootstrap_key_bounded_noise_from_seed;
//! ```
//!
//! ```compile_fail
//! use iroha_crypto::bootstrap_key_bounded_noise_with_max_refresh_rounds_from_seed;
//! ```
//!
//! ```compile_fail
//! use iroha_crypto::fhe_bfv::bootstrap_key_bounded_noise_with_max_refresh_rounds_from_seed;
//! ```
//!
//! ```compile_fail
//! use iroha_crypto::bfv_full_bootstrap_sample_extraction_switch_key_from_seed_v1;
//! ```
//!
//! ```compile_fail
//! use iroha_crypto::fhe_bfv::bfv_full_bootstrap_sample_extraction_switch_key_from_seed_v1;
//! ```
//!
//! ```compile_fail
//! use iroha_crypto::bfv_full_bootstrap_sample_extraction_bounded_noise_switch_key_from_seed_v1;
//! ```
//!
//! ```compile_fail
//! use iroha_crypto::fhe_bfv::bfv_full_bootstrap_sample_extraction_bounded_noise_switch_key_from_seed_v1;
//! ```
//!
//! ```compile_fail
//! use iroha_crypto::rotation_key_from_seed;
//! ```
//!
//! ```compile_fail
//! use iroha_crypto::fhe_bfv::rotation_key_from_seed;
//! ```
//!
//! ```compile_fail
//! use iroha_crypto::rotation_key_bounded_noise_from_seed;
//! ```
//!
//! ```compile_fail
//! use iroha_crypto::fhe_bfv::rotation_key_bounded_noise_from_seed;
//! ```

//! The obsolete qualification-only blocker type has no compatibility export.
//!
//! ```compile_fail
//! use iroha_crypto::BfvProductionQualificationBlockerV1;
//! ```
//!
//! ```compile_fail
//! use iroha_crypto::fhe_bfv::BfvProductionQualificationBlockerV1;
//! ```

#[cfg(not(feature = "bfv-test-fixtures"))]
/// The diagnostic namespace is absent from the default shipping API.
///
/// ```compile_fail
/// use iroha_crypto::bfv_test_fixtures;
/// ```
struct DefaultApiHasNoFixtures;
