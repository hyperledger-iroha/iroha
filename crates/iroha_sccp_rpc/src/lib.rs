//! Network I/O for SCCP v1 (spec `specs/sccp.md` §8).
//!
//! This crate talks to the outside world on behalf of the irohad light-client
//! keeper (§4.13.4), the `iroha sccp` CLI and the SCCP wallet (§7):
//!
//! - public-RPC endpoint lists with failover and owner-only secret headers;
//! - blocking HTTP over rustls, with `norito::json` parsing;
//! - EVM JSON-RPC, the beacon light-client API, TRON HTTP and a TON ADNL-TCP
//!   liteclient;
//! - builders for light-client advances, backfills, bootstraps, inbound
//!   proofs and void proofs.
//!
//! Nothing this crate returns is trusted. Every response and every evidence
//! bundle it builds is verified by `iroha_sccp`, the same network-free
//! verifier Taira runs, so a lying endpoint can only cause rejected
//! submissions.

pub mod beacon;
pub mod builders;
pub mod endpoints;
pub mod evm;
pub mod http;
pub mod ton;
pub mod tron;

pub use endpoints::{Backoff, EndpointSet, FailoverPolicy, HttpEndpointKind};
pub use http::{HttpConfig, HttpTransport, RpcError};
