//! Lightweight, consensus-visible zero-knowledge primitives.
//!
//! [`poseidon`] is the canonical BN254 Poseidon2 permutation shared by the data
//! model, IVM, FASTPQ and the Halo2 engine; [`vega_constants`] holds the Vega
//! mDL wire-shape constants the data model validates. Both are dependency-light
//! so `iroha_data_model` does not depend on the full `iroha_zkp_halo2` engine.
#![deny(missing_docs)]
#![deny(unsafe_code)]
pub mod pasta_keys;
pub mod poseidon;
pub mod vega_constants;
