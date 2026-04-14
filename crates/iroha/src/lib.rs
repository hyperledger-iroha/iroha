//! Rust client library for talking to Iroha over HTTP/WebSocket.
//!
//! This crate exposes the reusable SDK surface used by applications and by the
//! workspace CLI. The `iroha` command-line binary itself is built by the
//! separate `iroha_cli` crate.

pub mod account_address;
pub mod client;
pub mod config;
pub mod da;
pub mod http;
mod http_default;
pub mod nexus;
pub mod offline;
pub mod query;
pub mod secrecy;
pub mod sns;
pub mod subscriptions;

#[cfg(feature = "sm")]
pub mod sm;

pub use iroha_crypto as crypto;
pub use iroha_data_model as data_model;
pub use iroha_executor_data_model as executor_data_model;
