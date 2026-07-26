//! Fraud detection and risk-scoring data model.
//!
//! The types in this module capture the ledger-facing view of the PSP
//! (payment service provider) fraud pipeline. They are intentionally lean to
//! keep Norito compatibility and to unblock SDK scaffolding across languages.
//! Further fields can be added once the services mature; any breaking changes
//! must be accompanied by migrations and documented in `norito.md`.

pub mod types;

pub use types::*;
