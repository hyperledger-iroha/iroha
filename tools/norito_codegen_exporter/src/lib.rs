//! Focused Norito metadata and fixture tooling.
//!
//! The binary exports instruction metadata, while the library exposes shared
//! implementations used by repository automation and SDK fixture checks.
mod norito_rpc;
pub use norito_rpc::{
    AliasSetupFixtureBytes, FixtureOptions, JsonOutput, generate_fixtures, run_verify,
};
