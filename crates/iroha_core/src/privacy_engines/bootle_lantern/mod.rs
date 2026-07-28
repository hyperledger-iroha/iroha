//! Native fixed-profile Bootle/Lantern anonymous-credential engine.
//!
//! This implementation follows the fixed BLNS presentation relation over
//! `Z_12289[X]/(X^64 + 1)` and its Lantern/LNP22 module-linear-and-norm proof.
//! Consensus admission supplies the exact committed issuer-policy revision,
//! recomputes the transaction-intent binding, and binds the chain genesis
//! hash before this verifier is invoked.

pub mod bounds;
pub mod codec;
pub mod compression;
pub mod params;
pub mod proof;
pub mod relation;
pub mod ring;
pub mod sampling;
mod toolbox;
pub mod transcript;

pub(crate) use toolbox::application_relation_digest_v1;

/// The complete fixed-profile prover, verifier, strict codec, transparent
/// parameters, issuer-policy state, and consensus admission path are compiled.
pub const BOOTLE_LANTERN_FULL_ENGINE_AVAILABLE_V1: bool = true;
