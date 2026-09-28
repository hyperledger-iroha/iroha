//! TON liteserver access over ADNL-TCP (spec §4.13.3, §7.2 step 3, §8).
//!
//! A native, blocking liteclient built from the workspace crypto crates
//! (`curve25519-dalek`, `aes`/`ctr`, `sha2`), without an external TON SDK:
//!
//! - [`adnl`]: the ADNL-TCP handshake (ephemeral X25519 key against the
//!   liteserver's Ed25519 identity, AES-256-CTR session streams) and the
//!   SHA-256-checked packet framing, with `tcp.ping` keep-alive;
//! - [`tl`]: TL serialization primitives;
//! - [`schema`]: the `liteServer.*` requests and answers SCCP uses, with
//!   proofs, blocks and transactions kept as raw bag-of-cells bytes;
//! - [`peers`]: liteserver lists from `iroha_config` (`<ip>:<port>:<base64
//!   key>` entries, or the compiled defaults) and from `global-config.json`;
//! - [`liteclient`]: the query client with failover, timeouts and typed
//!   `liteServer.error` answers.
//!
//! Proof verification is not done here; the TON light client in `iroha_sccp`
//! verifies everything this module returns.

pub mod adnl;
pub mod liteclient;
pub mod peers;
pub mod schema;
pub mod tl;

pub use adnl::{AdnlConnection, AdnlError};
pub use liteclient::{LiteAttemptFailure, LiteClient, LiteClientConfig, LiteClientError, Stage};
pub use peers::{LiteServer, LiteServerSet, PeerError};
pub use schema::{
    AccountId, BlockId, BlockIdExt, LiteAnswer, LiteQuery, LiteServerError, LookupKey,
    WaitMasterchainSeqno,
};
