//! Post-quantum cryptography helpers for the `SoraNet` networking stack.
//!
//! This crate wraps the ML-KEM (CRYSTALS-Kyber) and ML-DSA (CRYSTALS-Dilithium)
//! implementations exposed via `PQClean` bindings and provides deterministic
//! hedged RNG + HKDF utilities needed by the `SoraNet` handshake.

#![cfg_attr(
    any(feature = "ffi-artifacts", soranet_pq_primary_package),
    crate_type = "cdylib"
)]
#![cfg_attr(
    any(feature = "ffi-artifacts", soranet_pq_primary_package),
    crate_type = "staticlib"
)]
#![allow(unsafe_code)]
#![deny(missing_docs)]

pub mod ffi;
mod hkdf;
mod mldsa;
mod mlkem;
mod rng;

pub use crate::{
    hkdf::{HkdfDomain, HkdfSuite, derive_labeled_hkdf},
    mldsa::{
        MlDsaError, MlDsaKeyPair, MlDsaSignature, MlDsaSuite, generate_mldsa_keypair, sign_mldsa,
        verify_mldsa,
    },
    mlkem::{
        MlKemCiphertext, MlKemKeyPair, MlKemMetadata, MlKemParameters, MlKemSharedSecret,
        MlKemSuite, SuiteParseError, decapsulate_mlkem, encapsulate_mlkem, generate_mlkem_keypair,
        mlkem_metadata, mlkem_parameters, validate_mlkem_ciphertext, validate_mlkem_public_key,
        validate_mlkem_secret_key,
    },
    rng::{HedgedRngSeed, hedged_chacha20_rng},
};
