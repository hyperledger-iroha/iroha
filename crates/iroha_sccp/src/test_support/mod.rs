//! Deterministic synthetic source chains for SCCP light-client tests (`specs/sccp.md` §11).
//!
//! Compiled under `cfg(test)` and the `test-fixtures` feature only. Each chain signs with the
//! same cryptography and domains the production verifier checks, so tests prove events from
//! local chains (EDR, a local java-tron, synthetic TON epochs) through the real light-client
//! code instead of a mock.

pub mod bsc;
pub mod ethereum;
pub mod ton;
pub mod tron;
