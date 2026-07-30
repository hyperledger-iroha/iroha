//! Native first-release PQ-MASP engine.
//!
//! The engine is deliberately split into a checked note relation, the fixed
//! ML-DSA/ML-KEM/XChaCha wire, and a transparent Goldilocks STARK. The verifier
//! and deterministic compiled profile are wired into typed runtime dispatch.
//! Compilation does not create or activate a lifecycle record; governed
//! deployment remains gated on the explicit release checks.

pub(crate) mod air;
mod facade;
#[cfg(any(test, feature = "privacy-release-evidence"))]
mod fixture;
pub(crate) mod relation;
pub(crate) mod stark;
pub(crate) mod wire;

pub use facade::{
    PqMaspProofErrorV1, encrypt_pq_masp_note_v1, encrypt_pq_masp_note_v1_with_rng,
    prove_pq_masp_v1, prove_pq_masp_v1_with_rng, verify_pq_masp_v1,
};
#[cfg(feature = "privacy-release-evidence")]
pub(crate) use fixture::{pq_masp_release_fixture_v1, pq_masp_release_invalid_path_fixture_v1};
pub use relation::{
    PQ_MASP_INPUT_BOUND_V1, PQ_MASP_OUTPUT_BOUND_V1, PQ_MASP_TREE_DEPTH_V1, PqMaspInputWitnessV1,
    PqMaspNotePlaintextV1, PqMaspOutputWitnessV1, PqMaspRelationErrorV1, PqMaspWitnessV1,
    derive_pq_masp_note_commitment_v1, derive_pq_masp_note_encryption_keys_digest_v1,
    derive_pq_masp_nullifier_key_digest_v1, derive_pq_masp_nullifier_v1,
};
pub use wire::{
    ML_DSA_65_PUBLIC_KEY_BYTES_V1, ML_DSA_65_SIGNATURE_BYTES_V1, ML_KEM_768_CIPHERTEXT_BYTES_V1,
    ML_KEM_768_PUBLIC_KEY_BYTES_V1, PQ_MASP_AUTHORIZATION_HEADER_BYTES_V1,
    PQ_MASP_ENCRYPTED_OUTPUT_BYTES_V1, PQ_MASP_MAX_AUTHORIZATION_PROOF_BYTES_V1,
    PQ_MASP_MAX_STARK_PROOF_BYTES_V1, PqMaspWireErrorV1, XCHACHA20_NONCE_BYTES_V1,
    decrypt_pq_masp_note_v1, derive_pq_masp_authorization_key_digest_v1,
    derive_pq_masp_recipient_id_v1, validate_pq_masp_encrypted_output_v1,
    validate_pq_masp_note_encryption_key_digest_v1,
};
