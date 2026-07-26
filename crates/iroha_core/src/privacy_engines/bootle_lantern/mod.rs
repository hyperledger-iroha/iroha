//! Native fixed-profile Bootle/Lantern anonymous-credential engine.
//!
//! This implementation follows the fixed BLNS presentation relation over
//! `Z_12289[X]/(X^64 + 1)` and its Lantern/LNP22 module-linear-and-norm proof.
//! The public module is intentionally not registered as an activatable
//! consensus engine until committed issuer-policy lookup and verifier-side
//! transaction-intent recomputation are wired into core.

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

/// This engine remains fail-closed until its trusted runtime inputs and the
/// complete presentation prover/verifier are compiled and wired.
pub const BOOTLE_LANTERN_FULL_ENGINE_AVAILABLE_V1: bool = false;
