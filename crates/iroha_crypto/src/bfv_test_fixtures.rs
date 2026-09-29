//! Explicit arithmetic fixtures for tests of BFV encodings and rejection boundaries.
//!
//! Enable `bfv-test-fixtures` only from a development dependency. These deterministic
//! helpers do not provide supported encryption: the exact profile has a noiseless
//! public-key equation modulo the plaintext modulus, and the rounded replacement
//! is unqualified. They do not change production policy admission or evaluation.

use crate::fhe_bfv::{
    BfvBootstrapKey, BfvCiphertext, BfvError, BfvFullBootstrapSampleExtractionSwitchKeyV1,
    BfvFullBootstrapSampleExtractionV1, BfvGaloisKey, BfvIdentifierCiphertext,
    BfvIdentifierPublicParameters, BfvParameters, BfvPublicKey, BfvRelinearizationKey,
    BfvRotationKey, BfvSecretKey,
};

/// Create exact-lift diagnostic key material.
///
/// # Errors
/// Returns [`BfvError`] when the underlying fixture inputs are invalid.
pub fn keygen_from_seed(
    params: &BfvParameters,
    seed: &[u8],
) -> Result<(BfvSecretKey, BfvPublicKey, BfvRelinearizationKey), BfvError> {
    crate::fhe_bfv::keygen_from_seed(params, seed)
}

/// Create rounded-noise diagnostic secret and public keys.
///
/// # Errors
/// Returns [`BfvError`] when the underlying fixture inputs are invalid.
pub fn keygen_bounded_noise_from_seed(
    params: &BfvParameters,
    seed: &[u8],
) -> Result<(BfvSecretKey, BfvPublicKey), BfvError> {
    crate::fhe_bfv::keygen_bounded_noise_from_seed(params, seed)
}

/// Create rounded-noise diagnostic keys including relinearization material.
///
/// # Errors
/// Returns [`BfvError`] when the underlying fixture inputs are invalid.
pub fn keygen_bounded_noise_with_relinearization_from_seed(
    params: &BfvParameters,
    seed: &[u8],
) -> Result<(BfvSecretKey, BfvPublicKey, BfvRelinearizationKey), BfvError> {
    crate::fhe_bfv::keygen_bounded_noise_with_relinearization_from_seed(params, seed)
}

/// Create an exact-lift arithmetic ciphertext fixture.
///
/// # Errors
/// Returns [`BfvError`] when the underlying fixture inputs are invalid.
pub fn encrypt_from_seed(
    params: &BfvParameters,
    public_key: &BfvPublicKey,
    plaintext: &[u64],
    seed: &[u8],
) -> Result<BfvCiphertext, BfvError> {
    crate::fhe_bfv::encrypt_from_seed(params, public_key, plaintext, seed)
}

/// Create a rounded-noise arithmetic ciphertext fixture.
///
/// # Errors
/// Returns [`BfvError`] when the underlying fixture inputs are invalid.
pub fn encrypt_bounded_noise_from_seed(
    params: &BfvParameters,
    public_key: &BfvPublicKey,
    plaintext: &[u64],
    seed: &[u8],
) -> Result<BfvCiphertext, BfvError> {
    crate::fhe_bfv::encrypt_bounded_noise_from_seed(params, public_key, plaintext, seed)
}

/// Create deterministic identifier key metadata for diagnostic tests.
///
/// # Errors
/// Returns [`BfvError`] when the underlying fixture inputs are invalid.
pub fn derive_identifier_key_material_from_seed(
    params: &BfvParameters,
    max_input_bytes: u16,
    seed: &[u8],
    associated_data: &[u8],
) -> Result<
    (
        BfvIdentifierPublicParameters,
        BfvSecretKey,
        BfvRelinearizationKey,
    ),
    BfvError,
> {
    crate::fhe_bfv::derive_identifier_key_material_from_seed(
        params,
        max_input_bytes,
        seed,
        associated_data,
    )
}

/// Create an identifier ciphertext fixture for diagnostic tests.
///
/// # Errors
/// Returns [`BfvError`] when the underlying fixture inputs are invalid.
pub fn encrypt_identifier_from_seed(
    public_parameters: &BfvIdentifierPublicParameters,
    input: &[u8],
    seed: &[u8],
) -> Result<BfvIdentifierCiphertext, BfvError> {
    crate::fhe_bfv::encrypt_identifier_from_seed(public_parameters, input, seed)
}

/// Create specialized BFV material for arithmetic diagnostics only.
///
/// # Errors
/// Returns [`BfvError`] when the underlying fixture inputs are invalid.
pub fn galois_key_from_seed(
    params: &BfvParameters,
    secret_key: &BfvSecretKey,
    automorphism_power: u32,
    seed: &[u8],
) -> Result<BfvGaloisKey, BfvError> {
    crate::fhe_bfv::galois_key_from_seed(params, secret_key, automorphism_power, seed)
}

/// Create specialized BFV material for arithmetic diagnostics only.
///
/// # Errors
/// Returns [`BfvError`] when the underlying fixture inputs are invalid.
pub fn galois_key_bounded_noise_from_seed(
    params: &BfvParameters,
    secret_key: &BfvSecretKey,
    automorphism_power: u32,
    seed: &[u8],
) -> Result<BfvGaloisKey, BfvError> {
    crate::fhe_bfv::galois_key_bounded_noise_from_seed(params, secret_key, automorphism_power, seed)
}

/// Create specialized BFV material for arithmetic diagnostics only.
///
/// # Errors
/// Returns [`BfvError`] when the underlying fixture inputs are invalid.
pub fn bootstrap_key_from_seed(
    params: &BfvParameters,
    public_key: &BfvPublicKey,
    key_id: impl Into<String>,
    seed: &[u8],
) -> Result<BfvBootstrapKey, BfvError> {
    crate::fhe_bfv::bootstrap_key_from_seed(params, public_key, key_id, seed)
}

/// Create specialized BFV material for arithmetic diagnostics only.
///
/// # Errors
/// Returns [`BfvError`] when the underlying fixture inputs are invalid.
pub fn bootstrap_key_with_max_refresh_rounds_from_seed(
    params: &BfvParameters,
    public_key: &BfvPublicKey,
    key_id: impl Into<String>,
    max_refresh_rounds: u16,
    seed: &[u8],
) -> Result<BfvBootstrapKey, BfvError> {
    crate::fhe_bfv::bootstrap_key_with_max_refresh_rounds_from_seed(
        params,
        public_key,
        key_id,
        max_refresh_rounds,
        seed,
    )
}

/// Create specialized BFV material for arithmetic diagnostics only.
///
/// # Errors
/// Returns [`BfvError`] when the underlying fixture inputs are invalid.
pub fn bootstrap_key_bounded_noise_from_seed(
    params: &BfvParameters,
    public_key: &BfvPublicKey,
    key_id: impl Into<String>,
    seed: &[u8],
) -> Result<BfvBootstrapKey, BfvError> {
    crate::fhe_bfv::bootstrap_key_bounded_noise_from_seed(params, public_key, key_id, seed)
}

/// Create specialized BFV material for arithmetic diagnostics only.
///
/// # Errors
/// Returns [`BfvError`] when the underlying fixture inputs are invalid.
pub fn bootstrap_key_bounded_noise_with_max_refresh_rounds_from_seed(
    params: &BfvParameters,
    public_key: &BfvPublicKey,
    key_id: impl Into<String>,
    max_refresh_rounds: u16,
    seed: &[u8],
) -> Result<BfvBootstrapKey, BfvError> {
    crate::fhe_bfv::bootstrap_key_bounded_noise_with_max_refresh_rounds_from_seed(
        params,
        public_key,
        key_id,
        max_refresh_rounds,
        seed,
    )
}

/// Create specialized BFV material for arithmetic diagnostics only.
///
/// # Errors
/// Returns [`BfvError`] when the underlying fixture inputs are invalid.
pub fn bfv_full_bootstrap_sample_extraction_switch_key_from_seed_v1(
    params: &BfvParameters,
    secret_key: &BfvSecretKey,
    sample_extraction: BfvFullBootstrapSampleExtractionV1,
    seed: &[u8],
) -> Result<BfvFullBootstrapSampleExtractionSwitchKeyV1, BfvError> {
    crate::fhe_bfv::bfv_full_bootstrap_sample_extraction_switch_key_from_seed_v1(
        params,
        secret_key,
        sample_extraction,
        seed,
    )
}

/// Create specialized BFV material for arithmetic diagnostics only.
///
/// # Errors
/// Returns [`BfvError`] when the underlying fixture inputs are invalid.
pub fn bfv_full_bootstrap_sample_extraction_bounded_noise_switch_key_from_seed_v1(
    params: &BfvParameters,
    secret_key: &BfvSecretKey,
    sample_extraction: BfvFullBootstrapSampleExtractionV1,
    seed: &[u8],
) -> Result<BfvFullBootstrapSampleExtractionSwitchKeyV1, BfvError> {
    crate::fhe_bfv::bfv_full_bootstrap_sample_extraction_bounded_noise_switch_key_from_seed_v1(
        params,
        secret_key,
        sample_extraction,
        seed,
    )
}

/// Create specialized BFV material for arithmetic diagnostics only.
///
/// # Errors
/// Returns [`BfvError`] when the underlying fixture inputs are invalid.
pub fn rotation_key_from_seed(
    params: &BfvParameters,
    public_key: &BfvPublicKey,
    rotation_steps: u32,
    seed: &[u8],
) -> Result<BfvRotationKey, BfvError> {
    crate::fhe_bfv::rotation_key_from_seed(params, public_key, rotation_steps, seed)
}

/// Create specialized BFV material for arithmetic diagnostics only.
///
/// # Errors
/// Returns [`BfvError`] when the underlying fixture inputs are invalid.
pub fn rotation_key_bounded_noise_from_seed(
    params: &BfvParameters,
    public_key: &BfvPublicKey,
    rotation_steps: u32,
    seed: &[u8],
) -> Result<BfvRotationKey, BfvError> {
    crate::fhe_bfv::rotation_key_bounded_noise_from_seed(params, public_key, rotation_steps, seed)
}
