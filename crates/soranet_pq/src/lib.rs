//! Post-quantum cryptography helpers for the `SoraNet` networking stack.
//!
//! This crate exposes ML-KEM and ML-DSA helpers plus the hedged RNG and HKDF
//! utilities needed by the `SoraNet` handshake.

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
        MlDsaError, MlDsaKeyPair, MlDsaSignature, MlDsaSuite, generate_mldsa_keypair,
        generate_mldsa_keypair_from_os, generate_mldsa_keypair_from_seed, sign_mldsa,
        sign_mldsa_from_os, verify_mldsa,
    },
    mlkem::{
        MlKemCiphertext, MlKemKeyPair, MlKemMetadata, MlKemParameters, MlKemSharedSecret,
        MlKemSuite, SuiteParseError, decapsulate_mlkem, encapsulate_mlkem,
        encapsulate_mlkem_from_os, encapsulate_mlkem_from_seed, generate_mlkem_keypair,
        generate_mlkem_keypair_from_os, generate_mlkem_keypair_from_seed, mlkem_metadata,
        mlkem_parameters, validate_mlkem_ciphertext, validate_mlkem_public_key,
        validate_mlkem_secret_key,
    },
    rng::{
        HedgedChaCha20Rng, HedgedEntropyStatus, HedgedRngSeed, RngError, hedged_chacha20_rng,
        hedged_chacha20_rng_from_os,
    },
};
