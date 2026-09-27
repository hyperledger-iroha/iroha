//! Evidence builders for SCCP v1 (spec §4.13.4, §7.2, §7.3).
//!
//! Builders fetch raw source-chain data from public RPC and assemble
//! light-client advances, backfills, bootstraps, inbound proofs and void
//! proofs. Their output is untrusted until `iroha_sccp` verifies it.

pub mod bsc;
pub mod ethereum;
pub mod ton;
pub mod tron;

// TODO(ws37): define the chain-independent builder entry points shared by the keeper and wallet.
