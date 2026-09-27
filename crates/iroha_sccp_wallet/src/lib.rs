//! Local SCCP v1 wallet library (spec `specs/sccp.md` §7, §8).
//!
//! Wallets run this library against any Taira peer's Torii and the public RPC
//! endpoints in the user's `[sccp]` client configuration; no hosted service
//! is involved. It holds:
//!
//! - `pure`: network-free bundle and rotation verification and destination
//!   transaction encodings (EVM, TRON and TON), exportable to SDK bridges;
//! - `flows`: the resumable outbound, inbound and refund flows of §7, which
//!   verify every piece of evidence before paying and journal it through
//!   `iroha_wallet::operation_journal` before submitting anything;
//! - `config` and `journal`: the file-only `[sccp]` client configuration and
//!   the SCCP journal records.
//!
//! This crate is never linked into `irohad`; the `sccp_wallet` layer in
//! `ci/dependency_budget.json` enforces that boundary.

pub mod config;
pub mod flows;
pub mod journal;
pub mod pure;
