//! Stateless SCCP v1 inbound light-client verification (spec `specs/sccp.md` §4.13).
//!
//! Every function here checks untrusted source-chain evidence (finality
//! updates, validator-set transitions, header segments, key-block links and
//! inclusion proofs) against light-client state passed in by the caller.
//! The modules keep no state of their own and perform no I/O, so Taira
//! execution, the irohad keeper and wallets run the same verifier.

pub mod bsc;
pub mod ethereum;
pub mod proof;
pub mod state;
pub mod ton;
pub mod tron;
