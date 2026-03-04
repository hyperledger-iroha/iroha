//! Shared data-availability helpers used by Torii and client SDKs.
//!
//! This module exposes deterministic sampling and assignment logic so the
//! server, CLI, and SDKs can agree on which chunks to probe for DA proofs.

pub mod sampling;
