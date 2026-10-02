//! Canonical chain labels, domain and topology identities, names, paths and metadata.
//!
//! These owners provide state-independent validation, declared Norito identities,
//! JSON/storage keys and schema metadata without depending on the ledger model.

pub mod chain;
pub mod domain;
pub mod error;
pub mod metadata;
pub mod name;
pub mod peer;
pub mod state_path;
pub mod topology;

#[cfg(test)]
mod manual_decode_tests;
