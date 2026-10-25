//! State-free native privacy engines, compiled profiles, verified effects and execution proofs.
//!
//! Core owns ledger storage and mounts these modules internally. Module names,
//! explicit `iroha_core::` schema identities and logging targets remain stable.
//! `iroha_core` `EnvFilter` directives also prefix-match this owner crate's targets.
#![cfg_attr(
    test,
    expect(
        clippy::large_stack_arrays,
        reason = "The compiler-generated test harness reference array exceeds 16 KiB; source modules deny this lint below"
    )
)]

/// Native transparent execution proofs and bounded deterministic race relations.
#[cfg_attr(test, deny(clippy::large_stack_arrays))]
pub mod execution_proofs;
#[cfg(test)]
#[cfg_attr(test, deny(clippy::large_stack_arrays))]
mod ivm_test_support;
/// Typed Exact12 dispatch registry: verification, effect class and delivery per role.
#[cfg_attr(test, deny(clippy::large_stack_arrays))]
pub mod privacy_dispatch;
/// Native transparent privacy protocol engines.
#[cfg_attr(test, deny(clippy::large_stack_arrays))]
pub mod privacy_engines;
/// Deterministic compiled manifests for executable privacy engines.
#[cfg_attr(test, deny(clippy::large_stack_arrays))]
pub mod privacy_profiles;
/// Durable records produced by verified first-release privacy actions.
#[cfg_attr(test, deny(clippy::large_stack_arrays))]
pub mod privacy_state;
/// Exhaustive native proof verification and verified-effect derivation.
#[doc(hidden)]
#[cfg_attr(test, deny(clippy::large_stack_arrays))]
pub mod privacy_verifier;
#[cfg(test)]
#[cfg_attr(test, deny(clippy::large_stack_arrays))]
pub(crate) mod json_macros {
    pub use norito::derive::JsonDeserialize;
}

#[cfg(test)]
#[cfg_attr(test, deny(clippy::large_stack_arrays))]
mod test_support;
