//! SCCP v1 Taira-side data model.
//!
//! SCCP (SORA Cross-Chain Protocol) moves Taira XOR between the Taira Iroha network and the
//! Ethereum, BSC, TRON and TON mainnets. The normative design is `specs/sccp.md` (revision 3).
//! These modules hold the Taira-internal Norito types: consensus parameters, bridge keys and
//! roster generations, attestation subjects and signatures, outbound and inbound records, block
//! leaves and destination controls, the route registry and escrow identity, inbound
//! light-client state, events and the Parliament governance payload. The v1 instructions live
//! in [`crate::isi::sccp`].
//!
//! Network profiles reuse [`crate::bridge::SccpNetworkV1`]; contract-visible byte layouts (§3)
//! live with the code that hashes them (`iroha_sccp::v1`), never in Norito derives. Per-chain
//! proofs, advances, bootstraps and consensus sets are opaque headered Norito frames decoded by
//! `iroha_sccp::light_client`, so this crate stays independent of chain structures.

pub mod attestation;
pub mod bounded_bytes;
pub mod control;
pub mod deployment;
pub mod escrow;
pub mod events;
pub mod governance;
pub mod inbound;
pub mod keys;
pub mod keys_index;
pub mod light_client;
pub mod outbound;
pub mod params;
pub mod registry;
pub mod roster;

#[cfg(test)]
pub(crate) mod test_support;
