//! Local SCCP v1 wallet library (spec `specs/sccp.md` §7, §8).
//!
//! Wallets run this library against any Taira peer's Torii and the public RPC
//! endpoints in the user's `[sccp]` client configuration; no hosted service
//! is involved. It holds:
//!
//! - `pure`: network-free verification of proof bundles, rotation chains and
//!   Parliament controls against the destination's own state, and destination
//!   transaction encodings (EVM calldata and EIP-1559 signing; TRON and TON),
//!   exportable to SDK bridges;
//! - `flows`: the resumable outbound, inbound and refund flows of §7, which
//!   verify every piece of evidence before paying and journal it before
//!   submitting anything;
//! - `config`: the file-only `[sccp]` table (endpoint lists, timeouts, pinned
//!   deployments per Taira `NetworkId`), kept in its own file beside the
//!   client config until the `iroha` client config root nests it;
//! - `journal`: the resumable journal keyed by `NetworkId`, built on
//!   `iroha_wallet::operation_journal`.
//!
//! This crate is never linked into `irohad`; the `sccp_wallet` layer in
//! `ci/dependency_budget.json` enforces that boundary.

pub mod config;
pub mod flows;
pub mod journal;
pub mod pure;
